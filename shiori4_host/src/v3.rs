//! v3 互換モードの口: `shiori_get_api(3)` で読んだ SHIORI に、SHIORI/3.0 の要求の全文を文字列で渡し、応答の全文を文字列で受け取る（`V3Runtime`）

use std::{ffi::OsStr, mem::ManuallyDrop, path::Path};

use shiori4::shiori4_abi::{ShioriInstance, SHIORI_ABI_VERSION_3};

use crate::{
    library_entry, normalize_dir, FromLibraryError, LoadError, ShioriApi, ShioriStatus,
    SHIORI_E_INVALID_UTF8, SHIORI_E_PANIC, SHIORI_E_UNSUPPORTED,
};

/// v3 互換モード（`shiori_get_api(3)`）の SHIORI 1 つを所有し、SHIORI/3.0 の全文を文字列で往復させる
///
/// `!Send`（中の ShioriInstance が !Send）。作ったスレッドで使うこと。
/// load に host を渡さないので、post と完了通知は無い（SHIORI からホストは呼べない）。
/// dir は [`crate::GhostRuntime`] と同じ検査と正規化を経て SHIORI に届く。
/// Drop で unload → ライブラリ解放 の順に後始末する（`GhostRuntime` と同じ）。
///
/// DLL を読む口（`load`・`from_library`）は DLL の初期化コードを動かす。信頼できる SHIORI だけを渡すこと
pub struct V3Runtime {
    // 解放順序は Drop で明示する。_lib は必ず最後
    instance: ManuallyDrop<ShioriInstance<'static>>,
    poisoned: bool,
    _lib: Option<libloading::Library>,
}

impl V3Runtime {
    /// SHIORI のライブラリを読み込み、v3 で load する
    pub fn load(lib_path: impl AsRef<OsStr>, ghost_dir: impl AsRef<Path>) -> Result<Self, LoadError> {
        // dir の検査をライブラリを開くより先にする（GhostRuntime::load と同じ）
        normalize_dir(ghost_dir.as_ref())?;
        // SAFETY: ライブラリの初期化コードが実行される。信頼できる SHIORI だけを読み込むこと
        let lib = unsafe { libloading::Library::new(lib_path) }.map_err(LoadError::Library)?;
        Self::from_library(lib, ghost_dir).map_err(LoadError::from)
    }

    /// 読み終えたライブラリから v3 で作る
    ///
    /// ライブラリの返し方は [`crate::GhostRuntime::from_library`] と同じ: `InvalidPath`・`NotShiori4`・
    /// `Unsupported`（`shiori_get_api(3)` が NULL）ではライブラリを返し、`LoadFailed`（v3 の load が
    /// NULL。理由は付かない）では返さない。`GhostRuntime::from_library` が `Unsupported` で返した
    /// ライブラリをそのまま渡せる
    pub fn from_library(lib: libloading::Library, ghost_dir: impl AsRef<Path>) -> Result<Self, FromLibraryError> {
        // SAFETY: lib は V3Runtime の最後のフィールドとして保持し、instance より後に解放する
        let (dir, api) = match unsafe { library_entry(&lib, ghost_dir.as_ref(), SHIORI_ABI_VERSION_3) } {
            Ok(v) => v,
            Err(error) => return Err(FromLibraryError { error, library: Some(lib) }),
        };
        Self::with_api(api, &dir, Some(lib)).map_err(|error| FromLibraryError { error, library: None })
    }

    /// 同じプロセス内の v3 のテーブル（ShioriApi::new で作った static）を使う。テスト用
    pub fn from_static(api: &'static ShioriApi, ghost_dir: impl AsRef<Path>) -> Result<Self, LoadError> {
        let dir = normalize_dir(ghost_dir.as_ref())?;
        if api.abi_version() != SHIORI_ABI_VERSION_3 {
            return Err(LoadError::Unsupported(SHIORI_E_UNSUPPORTED));
        }
        Self::with_api(api, &dir, None)
    }

    fn with_api(api: &'static ShioriApi, dir: &str, lib: Option<libloading::Library>) -> Result<Self, LoadError> {
        let instance = api.load_v3(dir).ok_or(LoadError::LoadFailed)?;
        Ok(Self { instance: ManuallyDrop::new(instance), poisoned: false, _lib: lib })
    }

    /// SHIORI がパニックした（SHIORI_E_PANIC を返した）か。true なら破棄して読み直すこと
    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    /// req は SHIORI/3.0 の要求の全文（GET も NOTIFY も）。Ok は応答の全文
    ///
    /// Err は ABI 層のエラー。応答が UTF-8 でなければ SHIORI_E_INVALID_UTF8（壊れた文字列は作らない）、
    /// poisoned の後は常に SHIORI_E_PANIC
    pub fn request(&mut self, req: &str) -> Result<String, ShioriStatus> {
        if self.poisoned {
            return Err(SHIORI_E_PANIC);
        }
        let r = self.instance.request(req);
        if matches!(r, Err(SHIORI_E_PANIC)) {
            self.poisoned = true;
        }
        // 相手のバッファは r の Drop で release される
        r?.body.as_str().map(str::to_owned).map_err(|_| SHIORI_E_INVALID_UTF8)
    }
}

impl Drop for V3Runtime {
    fn drop(&mut self) {
        // unload。SAFETY: 以後 instance には触れない
        unsafe { ManuallyDrop::drop(&mut self.instance) };
        // 残りのフィールドは宣言順に解放され、_lib（ライブラリ）が最後になる
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        test_dll, GhostRuntime, Host, Post, ShioriResponse, SHIORI_200_OK, SHIORI_204_NO_CONTENT,
        SHIORI_500_INTERNAL_SERVER_ERROR,
    };
    use shiori4::shiori4_abi::{Shiori, ShioriLoadMode};
    use shiori4::{Adapter, Ghost, HeaderWriter, Headers, ShioriHostHandle};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[cfg(windows)]
    const GHOST_DIR: &str = r"C:\ghost";
    #[cfg(not(windows))]
    const GHOST_DIR: &str = "/ghost";

    /// NOTIFY で受けた ID
    static NOTIFIED: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
    /// EchoGhost の Drop（= unload）の回数
    static UNLOADS: AtomicUsize = AtomicUsize::new(0);

    /// GET の ID を Value で返し、NOTIFY の ID を NOTIFIED に残す
    struct EchoGhost;

    impl Ghost for EchoGhost {
        fn load(host: Option<ShioriHostHandle>, dir: &str) -> Result<Self, ShioriStatus> {
            assert!(host.is_none(), "v3 では host を渡さない");
            assert!(dir.ends_with(std::path::MAIN_SEPARATOR), "dir は正規化されて届く");
            Ok(Self)
        }
        fn request(&mut self, req: &Headers<'_>) -> ShioriResponse {
            HeaderWriter::new().push("Value", req.id().unwrap_or("")).into_response()
        }
        fn notify(&mut self, req: &Headers<'_>) -> ShioriStatus {
            *NOTIFIED.lock().unwrap() = req.id().unwrap_or("").to_owned();
            SHIORI_204_NO_CONTENT
        }
    }

    static ECHO_V3: ShioriApi = ShioriApi::new::<Adapter<EchoGhost>, { SHIORI_ABI_VERSION_3 }>();

    /// Drop を数えるだけの Ghost（UNLOADS を他のテストと共有しないよう専用にする）
    struct CountGhost;

    impl Ghost for CountGhost {
        fn load(_host: Option<ShioriHostHandle>, _dir: &str) -> Result<Self, ShioriStatus> {
            Ok(Self)
        }
        fn request(&mut self, _req: &Headers<'_>) -> ShioriResponse {
            ShioriResponse::no_content()
        }
    }

    impl Drop for CountGhost {
        fn drop(&mut self) {
            UNLOADS.fetch_add(1, Ordering::SeqCst);
        }
    }

    static COUNT_V3: ShioriApi = ShioriApi::new::<Adapter<CountGhost>, { SHIORI_ABI_VERSION_3 }>();

    struct FailGhost;

    impl Ghost for FailGhost {
        fn load(_host: Option<ShioriHostHandle>, _dir: &str) -> Result<Self, ShioriStatus> {
            Err(SHIORI_500_INTERNAL_SERVER_ERROR)
        }
        fn request(&mut self, _req: &Headers<'_>) -> ShioriResponse {
            ShioriResponse::no_content()
        }
    }

    static FAIL_V3: ShioriApi = ShioriApi::new::<Adapter<FailGhost>, { SHIORI_ABI_VERSION_3 }>();

    /// ABI 層の trait を直接実装する。"bad" で UTF-8 でない応答、"panic" でパニック
    struct RawShiori;

    impl Shiori for RawShiori {
        fn load(_mode: ShioriLoadMode, _dir: &str) -> Result<Self, ShioriStatus> {
            Ok(Self)
        }
        fn request(&mut self, req: &str) -> ShioriResponse {
            match req {
                "bad" => ShioriResponse::with_status(SHIORI_200_OK, vec![b'S', 0xff, 0xfe]),
                "panic" => panic!("テストのパニック"),
                _ => ShioriResponse::with_status(SHIORI_200_OK, "SHIORI/3.0 200 OK\r\n\r\n"),
            }
        }
    }

    static RAW_V3: ShioriApi = ShioriApi::new::<RawShiori, { SHIORI_ABI_VERSION_3 }>();

    #[test]
    fn get_roundtrip() {
        let mut rt = V3Runtime::from_static(&ECHO_V3, GHOST_DIR).expect("load");
        let text = rt.request("GET SHIORI/3.0\r\nCharset: UTF-8\r\nID: OnBoot\r\n\r\n").expect("abi");
        assert!(text.starts_with("SHIORI/3.0 200 OK\r\n"), "{text}");
        assert!(text.contains("Value: OnBoot\r\n"), "{text}");
        assert!(text.ends_with("\r\n\r\n"), "{text}");
    }

    #[test]
    fn notify_full_text_through_request() {
        let mut rt = V3Runtime::from_static(&ECHO_V3, GHOST_DIR).expect("load");
        let text = rt.request("NOTIFY SHIORI/3.0\r\nCharset: UTF-8\r\nID: OnNotifyX\r\n\r\n").expect("abi");
        assert!(text.starts_with("SHIORI/3.0 204 "), "{text}");
        assert_eq!(*NOTIFIED.lock().unwrap(), "OnNotifyX");
    }

    #[test]
    fn load_null_is_load_failed() {
        assert!(matches!(V3Runtime::from_static(&FAIL_V3, GHOST_DIR), Err(LoadError::LoadFailed)));
    }

    #[test]
    fn invalid_dir_and_wrong_table() {
        assert!(matches!(V3Runtime::from_static(&ECHO_V3, "ghost"), Err(LoadError::InvalidPath)));
        static ECHO_V4: ShioriApi = ShioriApi::new::<Adapter<EchoGhost>, { crate::SHIORI_ABI_VERSION }>();
        assert!(matches!(V3Runtime::from_static(&ECHO_V4, GHOST_DIR), Err(LoadError::Unsupported(_))));
    }

    #[test]
    fn non_utf8_response_is_error() {
        let mut rt = V3Runtime::from_static(&RAW_V3, GHOST_DIR).expect("load");
        assert_eq!(rt.request("bad"), Err(SHIORI_E_INVALID_UTF8));
        assert!(!rt.is_poisoned());
        assert!(rt.request("ok").is_ok());
    }

    #[test]
    fn panic_poisons() {
        let mut rt = V3Runtime::from_static(&RAW_V3, GHOST_DIR).expect("load");
        assert_eq!(rt.request("panic"), Err(SHIORI_E_PANIC));
        assert!(rt.is_poisoned());
        assert_eq!(rt.request("ok"), Err(SHIORI_E_PANIC));
    }

    #[test]
    fn drop_unloads_once() {
        let rt = V3Runtime::from_static(&COUNT_V3, GHOST_DIR).expect("load");
        assert_eq!(UNLOADS.load(Ordering::SeqCst), 0);
        drop(rt);
        assert_eq!(UNLOADS.load(Ordering::SeqCst), 1);
    }

    struct NoHost;

    impl Host for NoHost {
        fn on_post(&self, _post: Post) {}
    }

    fn test_lib() -> libloading::Library {
        unsafe { libloading::Library::new(test_dll::dll_path()) }.expect("テスト用 DLL を開けない")
    }

    #[test]
    fn from_library_takes_unsupported_library() {
        let _g = test_dll::lock();
        // モードを戻すための別のハンドル（V3Runtime がライブラリを持って行くため）
        let mode_lib = test_lib();
        test_dll::set_mode(&mode_lib, test_dll::MODE_V3);
        let e = GhostRuntime::from_library(test_lib(), GHOST_DIR, NoHost).err().expect("v4 は無い");
        assert!(matches!(e.error(), LoadError::Unsupported(_)));
        let mut rt = V3Runtime::from_library(e.into_library().expect("ライブラリが返る"), GHOST_DIR).expect("v3 で読める");
        let text = rt.request("GET SHIORI/3.0\r\nID: OnBoot\r\n\r\n").expect("abi");
        assert!(text.starts_with("SHIORI/3.0 200 OK\r\n"), "{text}");
        assert!(text.contains("Value: testghost\r\n"), "{text}");
        drop(rt);
        test_dll::set_mode(&mode_lib, test_dll::MODE_DEFAULT);
    }

    #[test]
    fn from_library_returns_library() {
        let _g = test_dll::lock();
        let mode_lib = test_lib();
        // v3 が無い → Unsupported とライブラリ
        test_dll::set_mode(&mode_lib, test_dll::MODE_V4);
        let e = V3Runtime::from_library(test_lib(), GHOST_DIR).err().expect("v3 は無い");
        assert!(matches!(e.error(), LoadError::Unsupported(_)));
        assert!(e.into_library().is_some());
        test_dll::set_mode(&mode_lib, test_dll::MODE_DEFAULT);
        // 相対の dir → InvalidPath とライブラリ
        let e = V3Runtime::from_library(test_lib(), "ghost").err().expect("相対");
        assert!(matches!(e.error(), LoadError::InvalidPath));
        assert!(e.into_library().is_some());
        // load が NULL → LoadFailed、ライブラリは返さない
        let dir = std::path::Path::new(GHOST_DIR).join("fail");
        let e = V3Runtime::from_library(test_lib(), &dir).err().expect("load の失敗");
        assert!(matches!(e.error(), LoadError::LoadFailed));
        assert!(e.into_library().is_none());
        // パスの入口
        assert!(V3Runtime::load(test_dll::dll_path(), GHOST_DIR).is_ok());
    }
}
