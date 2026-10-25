//! SHIORI/3.0 の従来の口（`load`・`loadu`・`request`・`unload`）を持つ DLL を読む部分（x86 の Windows だけ）
//!
//! HGLOBAL の持ち主の決まり: 入力はホストが確保して DLL が解放し、応答は DLL が確保してホストが解放する。

#[cfg(test)]
use std::sync::atomic::{AtomicU32, Ordering};
use std::{ffi::c_void, marker::PhantomData, path::Path};

use crate::{EntryKind, LoadError, normalize_dir};

const GMEM_FIXED: u32 = 0;
const CP_ACP: u32 = 0;
const CP_UTF8: u32 = 65001;
const WC_NO_BEST_FIT_CHARS: u32 = 0x400;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GlobalAlloc(flags: u32, bytes: usize) -> *mut c_void;
    fn GlobalFree(h: *mut c_void) -> *mut c_void;
    fn GetACP() -> u32;
    fn WideCharToMultiByte(
        code_page: u32,
        flags: u32,
        wide: *const u16,
        wide_len: i32,
        out: *mut u8,
        out_len: i32,
        default_char: *const u8,
        used_default_char: *mut i32,
    ) -> i32;
}

// 従来の DLL の口（ukadoc の DLL 共通仕様。x86 の __cdecl は Rust の extern "C"、long は i32）
type LoadFn = unsafe extern "C" fn(h: *mut c_void, len: i32) -> i32;
type RequestFn = unsafe extern "C" fn(h: *mut c_void, len: *mut i32) -> *mut c_void;
type UnloadFn = unsafe extern "C" fn() -> i32;

/// 従来の口（`loadu`・`load`・`request`・`unload`）の DLL を 1 回だけ初期化した実行時の値
///
/// `!Send`・`!Sync`。作ったスレッドで使うこと（[`crate::V3Runtime`] と同じ）。
/// Drop で `unload` を 1 回呼び（戻り値は見ない）、その後でライブラリを解放する。手放した後は DLL の関数を呼ばない。
///
/// - 1 つのモジュールに 1 インスタンスだけ。同じファイルを 2 回開くと同じモジュールになり、従来の ABI にはハンドルが無い。
///   2 体目は別のファイルか別のプロセスで読む。
/// - 渡すライブラリは、発見の口（`load_ghost`・`load_ghost_with`）が `LoadedGhost::Legacy` で返したものにする。
///   開いた時点で DLL の初期化のコードが動いているので、信頼できる DLL だけを読むこと。自分で `Library::new` すると
///   ダイアログの抑止と絶対パスの検査が効かない。
/// - `ghost_dir` には、発見の口に渡したのと同じ dir を渡す（`LoadedGhost::Legacy` は dir を持たない）。
/// - ホストに残すもの: プロセスの起こし方・IPC・待ち時間・強制終了・カレントディレクトリ・本文の文字コードの方針。
///   この型は要求と応答の中身を解釈しない。
///
/// 別のスレッドへは送れない:
///
/// ```compile_fail,E0277
/// fn f<T: Send>() {}
/// f::<shiori4_host::LegacyRuntime>();
/// ```
pub struct LegacyRuntime {
    #[allow(dead_code)] // request（task 3.4）が使う
    request: RequestFn,
    unload: UnloadFn,
    _not_send: PhantomData<*mut ()>,
    // Drop の本体で unload を呼んだ後、宣言の順に落ちる。ライブラリは最後
    _library: libloading::Library,
}

impl Drop for LegacyRuntime {
    fn drop(&mut self) {
        // SAFETY: 初期化は済んでいて、ライブラリはまだ開いている。以後 DLL の関数は呼ばない
        unsafe { (self.unload)() };
    }
}

impl LegacyRuntime {
    /// `LoadedGhost::Legacy` の entry と library を受け、entry の口で 1 回だけ初期化する。
    /// ghost_dir には発見の口に渡したのと同じ dir を渡す
    ///
    /// 口・`unload`・`request` の解決、dir の検査と変換、入力の確保を済ませてから口を呼ぶ。
    /// entry が Loadu・Load でないか書き出しが揃わなければ `NotShiori4`、dir が不正なら `InvalidPath`、
    /// 変換・確保の失敗と口が 0 を返したときは `LoadFailed`。どの失敗でも `unload` は呼ばずにライブラリを解放する
    pub fn from_library(
        entry: EntryKind,
        library: libloading::Library,
        ghost_dir: impl AsRef<Path>,
    ) -> Result<Self, LoadError> {
        let name: &[u8] = match entry {
            EntryKind::Loadu => b"loadu\0",
            EntryKind::Load => b"load\0",
            _ => return Err(LoadError::NotShiori4),
        };
        // SAFETY: 型は従来の口の ABI。関数のポインタは library を持つ間だけ使う
        let (init, unload, request) = unsafe {
            (
                *library
                    .get::<LoadFn>(name)
                    .map_err(|_| LoadError::NotShiori4)?,
                *library
                    .get::<UnloadFn>(b"unload\0")
                    .map_err(|_| LoadError::NotShiori4)?,
                *library
                    .get::<RequestFn>(b"request\0")
                    .map_err(|_| LoadError::NotShiori4)?,
            )
        };
        let dir = normalize_dir(ghost_dir.as_ref())?;
        let (bytes, _) = dir_bytes(entry, &dir)?;
        let h = global_alloc_copy(&bytes).ok_or(LoadError::LoadFailed)?;
        // h は渡した時点で DLL のもの。x86 のスライスの長さは i32::MAX 以下
        if unsafe { init(h, bytes.len() as i32) } == 0 {
            return Err(LoadError::LoadFailed);
        }
        Ok(Self {
            request,
            unload,
            _not_send: PhantomData,
            _library: library,
        })
    }

    /// dir を load に渡すと CP_ACP で表せない文字が置き換わるか（loadu には関わらない）。
    /// 値を作らずに呼べるので、初期化の前にも、load が失敗した後にも確かめられる。
    /// dir の検査は from_library と同じ（`InvalidPath`）。変換の API の失敗は `LoadFailed`
    pub fn dir_lossy(ghost_dir: impl AsRef<Path>) -> Result<bool, LoadError> {
        Ok(dir_bytes(EntryKind::Load, &normalize_dir(ghost_dir.as_ref())?)?.1)
    }
}

/// 従来の DLL の要求の失敗
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyRequestError {
    /// 要求の入力のメモリ（GlobalAlloc）を確保できなかった。DLL は呼んでいない
    AllocFailed,
    /// request が NULL を返した
    NullResponse,
    /// request が負の長さを返した（応答のメモリは解放した）
    NegativeLength(i32),
    /// 文字列の口で、応答（末尾の NUL を落とした後）が UTF-8 でない
    InvalidUtf8,
}

/// global_alloc_copy が確保した数
#[cfg(test)]
pub(crate) static INPUTS_ALLOCATED: AtomicU32 = AtomicU32::new(0);
/// take_response が解放した数
#[cfg(test)]
pub(crate) static RESPONSES_FREED: AtomicU32 = AtomicU32::new(0);

/// GMEM_FIXED で max(bytes.len(), 1) バイトを確保し、bytes をちょうど len バイトだけ書く（NUL を付けない）。
/// 返したハンドルの持ち主は呼んだ先の DLL。確保できなければ None
fn global_alloc_copy(bytes: &[u8]) -> Option<*mut c_void> {
    // GlobalAlloc(GMEM_FIXED, 0) の振る舞いは文書で保証されないので、最低 1 バイトを確保する
    let h = unsafe { GlobalAlloc(GMEM_FIXED, bytes.len().max(1)) };
    if h.is_null() {
        return None;
    }
    // GMEM_FIXED のハンドルは先頭のポインタ
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), h.cast::<u8>(), bytes.len()) };
    #[cfg(test)]
    INPUTS_ALLOCATED.fetch_add(1, Ordering::SeqCst);
    Some(h)
}

/// request の戻り値を受け取る。NULL は NullResponse。len < 0 は GlobalFree して NegativeLength。
/// それ以外は len バイトだけ写し（NUL を探さない）、GlobalFree して返す
///
/// # Safety
/// p は DLL が GMEM_FIXED で確保した、少なくとも len バイトの領域か NULL
#[allow(dead_code)] // LegacyRuntime（task 3.4）が使う
unsafe fn take_response(p: *mut c_void, len: i32) -> Result<Vec<u8>, LegacyRequestError> {
    if p.is_null() {
        return Err(LegacyRequestError::NullResponse);
    }
    let out = if len < 0 {
        Err(LegacyRequestError::NegativeLength(len))
    } else {
        Ok(unsafe { std::slice::from_raw_parts(p.cast::<u8>(), len as usize) }.to_vec())
    };
    unsafe { GlobalFree(p) };
    #[cfg(test)]
    RESPONSES_FREED.fetch_add(1, Ordering::SeqCst);
    out
}

/// entry が Loadu なら (dir の UTF-8, false)。Load なら to_acp(dir)。変換の API の失敗は LoadFailed
fn dir_bytes(entry: EntryKind, dir: &str) -> Result<(Vec<u8>, bool), LoadError> {
    match entry {
        EntryKind::Load => to_acp(dir).ok_or(LoadError::LoadFailed),
        _ => Ok((dir.as_bytes().to_vec(), false)),
    }
}

/// CP_ACP のバイト列と「表せない文字があった」。GetACP() が 65001 なら (s の UTF-8, false)。
/// それ以外は WC_NO_BEST_FIT_CHARS で best-fit をせず、表せない文字は既定の文字（`?`）にして
/// used_default_char で知る。長さの問い合わせと変換の 2 回で呼び、NUL は出さない。0 が返れば None
fn to_acp(s: &str) -> Option<(Vec<u8>, bool)> {
    // 65001 では WC_NO_BEST_FIT_CHARS と used_default_char が使えない。UTF-8 はそのまま CP_ACP のバイト列
    if unsafe { GetACP() } == CP_UTF8 {
        return Some((s.as_bytes().to_vec(), false));
    }
    // 長さ 0 では WideCharToMultiByte が失敗する
    if s.is_empty() {
        return Some((Vec::new(), false));
    }
    let wide: Vec<u16> = s.encode_utf16().collect();
    let wide_len = i32::try_from(wide.len()).ok()?;
    let convert = |out: *mut u8, out_len: i32, used: *mut i32| unsafe {
        WideCharToMultiByte(
            CP_ACP,
            WC_NO_BEST_FIT_CHARS,
            wide.as_ptr(),
            wide_len,
            out,
            out_len,
            std::ptr::null(),
            used,
        )
    };
    let n = convert(std::ptr::null_mut(), 0, std::ptr::null_mut());
    if n <= 0 {
        return None;
    }
    let mut out = vec![0u8; n as usize];
    let mut used = 0i32;
    let written = convert(out.as_mut_ptr(), n, &mut used);
    if written <= 0 {
        return None;
    }
    out.truncate(written as usize);
    Some((out, used != 0))
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard};

    use super::*;

    // 数を読むテストどうしを並べない
    static COUNTERS: Mutex<()> = Mutex::new(());
    fn counters() -> MutexGuard<'static, ()> {
        COUNTERS.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// DLL の代わりに応答を GMEM_FIXED で確保する（末尾に NUL を置く）
    fn dll_response(bytes: &[u8]) -> *mut c_void {
        let p = unsafe { GlobalAlloc(GMEM_FIXED, bytes.len() + 1) };
        assert!(!p.is_null());
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), p.cast::<u8>(), bytes.len());
            *p.cast::<u8>().add(bytes.len()) = 0;
        }
        p
    }

    #[test]
    fn alloc_copy_writes_exact_bytes_and_counts() {
        let _g = counters();
        let before = INPUTS_ALLOCATED.load(Ordering::SeqCst);
        let h = global_alloc_copy(b"abc").expect("alloc");
        let got = unsafe { std::slice::from_raw_parts(h.cast::<u8>(), 3) }.to_vec();
        assert_eq!(got, b"abc");
        assert_eq!(INPUTS_ALLOCATED.load(Ordering::SeqCst), before + 1);
        // 持ち主は DLL。ここでは DLL の代わりに解放する
        assert!(unsafe { GlobalFree(h) }.is_null());
    }

    #[test]
    fn alloc_copy_empty_returns_a_handle() {
        let _g = counters();
        let before = INPUTS_ALLOCATED.load(Ordering::SeqCst);
        let h = global_alloc_copy(b"").expect("alloc of 0 bytes still returns a handle");
        assert!(!h.is_null());
        assert_eq!(INPUTS_ALLOCATED.load(Ordering::SeqCst), before + 1);
        assert!(unsafe { GlobalFree(h) }.is_null());
    }

    #[test]
    fn take_response_copies_len_bytes_and_frees() {
        let _g = counters();
        let before = RESPONSES_FREED.load(Ordering::SeqCst);
        // NUL を探さない: 途中の NUL も len までは写す
        let p = dll_response(b"ab\0cd");
        assert_eq!(unsafe { take_response(p, 5) }, Ok(b"ab\0cd".to_vec()));
        assert_eq!(RESPONSES_FREED.load(Ordering::SeqCst), before + 1);
    }

    #[test]
    fn take_response_len_shorter_than_buffer_copies_only_len() {
        let _g = counters();
        let p = dll_response(b"abcdef");
        assert_eq!(unsafe { take_response(p, 2) }, Ok(b"ab".to_vec()));
    }

    #[test]
    fn take_response_zero_len_is_empty_and_frees() {
        let _g = counters();
        let before = RESPONSES_FREED.load(Ordering::SeqCst);
        let p = dll_response(b"");
        assert_eq!(unsafe { take_response(p, 0) }, Ok(Vec::new()));
        assert_eq!(RESPONSES_FREED.load(Ordering::SeqCst), before + 1);
    }

    #[test]
    fn take_response_null_is_error_and_frees_nothing() {
        let _g = counters();
        let before = RESPONSES_FREED.load(Ordering::SeqCst);
        assert_eq!(
            unsafe { take_response(std::ptr::null_mut(), 3) },
            Err(LegacyRequestError::NullResponse)
        );
        assert_eq!(RESPONSES_FREED.load(Ordering::SeqCst), before);
    }

    #[test]
    fn take_response_negative_len_is_error_and_frees() {
        let _g = counters();
        let before = RESPONSES_FREED.load(Ordering::SeqCst);
        let p = dll_response(b"x");
        assert_eq!(
            unsafe { take_response(p, -1) },
            Err(LegacyRequestError::NegativeLength(-1))
        );
        assert_eq!(RESPONSES_FREED.load(Ordering::SeqCst), before + 1);
    }

    #[test]
    fn to_acp_ascii_is_utf8_and_not_lossy() {
        let s = r"C:\ghost\master\";
        assert_eq!(to_acp(s), Some((s.as_bytes().to_vec(), false)));
    }

    #[test]
    fn to_acp_empty_is_empty() {
        assert_eq!(to_acp(""), Some((Vec::new(), false)));
    }

    #[test]
    fn to_acp_surrogate_pair_emoji_is_question_and_lossy() {
        let s = "a\u{1F600}b";
        let (bytes, lossy) = to_acp(s).expect("convert");
        if unsafe { GetACP() } == CP_UTF8 {
            assert_eq!((bytes, lossy), (s.as_bytes().to_vec(), false));
            return;
        }
        assert!(lossy);
        // 写った所は `?` だけ（best-fit の似た文字にならない）
        let (first, rest) = bytes.split_first().expect("non-empty");
        let (last, middle) = rest.split_last().expect("at least 2 bytes");
        assert_eq!((*first, *last), (b'a', b'b'));
        assert!(
            !middle.is_empty() && middle.iter().all(|&c| c == b'?'),
            "{bytes:?}"
        );
    }

    #[test]
    fn dir_bytes_loadu_is_utf8_itself() {
        let s = "C:\\ghost\\\u{1F600}\u{3042}\\";
        assert_eq!(
            dir_bytes(EntryKind::Loadu, s).ok(),
            Some((s.as_bytes().to_vec(), false))
        );
    }

    #[test]
    fn dir_bytes_load_is_to_acp() {
        let s = "C:\\ghost\\\u{1F600}\\";
        assert_eq!(dir_bytes(EntryKind::Load, s).ok(), to_acp(s));
        assert_eq!(
            dir_bytes(EntryKind::Load, "C:\\g\\").ok(),
            Some((b"C:\\g\\".to_vec(), false))
        );
    }

    // ---- LegacyRuntime の初期化（legacyghost を読む）----

    use crate::{Host, LoadedGhost, Post, load_ghost, load_ghost_with, test_dll};

    /// ASCII でない文字を含む dir と、その正規化の後
    const DIR: &str = "C:\\ghost\\\u{e9}";
    const DIR_NORMALIZED: &str = "C:\\ghost\\\u{e9}\\";

    struct NoHost;

    impl Host for NoHost {
        fn name(&self) -> &str {
            "nohost"
        }

        fn on_post(&self, _post: Post) {}
    }

    /// ロックを取って legacyghost の状態を戻し、数を読むための別のハンドルを f に渡す
    fn with_legacy(f: impl FnOnce(&libloading::Library)) {
        let _g = test_dll::lock();
        let stats = open_legacy();
        test_dll::legacy_reset(&stats);
        f(&stats);
        test_dll::legacy_reset(&stats);
    }

    fn open_legacy() -> libloading::Library {
        unsafe { libloading::Library::new(test_dll::legacy_dll_path()) }
            .expect("legacyghost を開けない")
    }

    fn legacy_of(r: Result<LoadedGhost<NoHost>, LoadError>) -> (EntryKind, libloading::Library) {
        match r {
            Ok(LoadedGhost::Legacy { entry, library }) => (entry, library),
            _ => panic!("Legacy に解決しない"),
        }
    }

    #[test]
    fn default_discovery_picks_loadu_with_utf8_dir() {
        with_legacy(|stats| {
            let (entry, library) = legacy_of(load_ghost(test_dll::legacy_dll_path(), DIR, NoHost));
            assert_eq!(entry, EntryKind::Loadu);
            assert!(LegacyRuntime::from_library(entry, library, DIR).is_ok());
            let s = test_dll::legacy_stats(stats);
            // 値は is_ok の後すぐ落ちて unload が 1 回
            assert_eq!((s.loadu, s.load, s.unload), (1, 0, 1));
            assert_eq!(
                test_dll::legacy_last_input(stats),
                DIR_NORMALIZED.as_bytes()
            );
        });
    }

    #[test]
    fn load_only_picks_load_with_acp_dir() {
        with_legacy(|stats| {
            let (entry, library) = legacy_of(load_ghost_with(
                test_dll::legacy_dll_path(),
                DIR,
                NoHost,
                &[EntryKind::Load],
            ));
            assert_eq!(entry, EntryKind::Load);
            assert!(LegacyRuntime::from_library(entry, library, DIR).is_ok());
            let s = test_dll::legacy_stats(stats);
            assert_eq!((s.loadu, s.load, s.unload), (0, 1, 1));
            let (acp, _) = to_acp(DIR_NORMALIZED).expect("convert");
            assert_eq!(test_dll::legacy_last_input(stats), acp);
        });
    }

    #[test]
    fn loadu_failure_does_not_fall_back_to_load() {
        with_legacy(|stats| {
            let r = LegacyRuntime::from_library(EntryKind::Loadu, open_legacy(), r"C:\ghost\fail");
            assert!(matches!(r, Err(LoadError::LoadFailed)));
            let s = test_dll::legacy_stats(stats);
            assert_eq!((s.loadu, s.load, s.unload), (1, 0, 0));
        });
    }

    #[test]
    fn bool_is_32bit_nonzero() {
        with_legacy(|stats| {
            // 0x100 は下位 1 バイトが 0
            for v in [2, 0x100, -1] {
                test_dll::legacy_set_load_result(stats, v);
                let r = LegacyRuntime::from_library(EntryKind::Loadu, open_legacy(), DIR);
                assert!(r.is_ok(), "{v}");
            }
            test_dll::legacy_set_load_result(stats, 0);
            let r = LegacyRuntime::from_library(EntryKind::Load, open_legacy(), DIR);
            assert!(matches!(r, Err(LoadError::LoadFailed)));
            let s = test_dll::legacy_stats(stats);
            assert_eq!(
                (s.loadu, s.load, s.unload),
                (3, 1, 3),
                "失敗した 1 回は unload しない"
            );
        });
    }

    #[test]
    fn non_legacy_entry_or_missing_exports_is_not_shiori4() {
        with_legacy(|stats| {
            for entry in [EntryKind::V4, EntryKind::V3] {
                let r = LegacyRuntime::from_library(entry, open_legacy(), DIR);
                assert!(matches!(r, Err(LoadError::NotShiori4)));
            }
            assert_eq!(
                test_dll::legacy_stats(stats),
                test_dll::LegacyStats::default()
            );
        });
        // testghost には unload・request が無い。その loadu は 0 を返すので、呼ばれていれば LoadFailed になる
        let _g = test_dll::lock();
        let lib = unsafe { libloading::Library::new(test_dll::dll_path()) }
            .expect("testghost を開けない");
        let r = LegacyRuntime::from_library(EntryKind::Loadu, lib, DIR);
        assert!(matches!(r, Err(LoadError::NotShiori4)));
    }

    #[test]
    fn invalid_dir_is_invalid_path_without_calling() {
        with_legacy(|stats| {
            let r = LegacyRuntime::from_library(EntryKind::Loadu, open_legacy(), "ghost");
            assert!(matches!(r, Err(LoadError::InvalidPath)));
            assert_eq!(
                test_dll::legacy_stats(stats),
                test_dll::LegacyStats::default()
            );
        });
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    }

    /// legacyghost がプロセスに読まれているか
    fn legacy_module_loaded() -> bool {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = test_dll::legacy_dll_path()
            .as_os_str()
            .encode_wide()
            .chain([0])
            .collect();
        !unsafe { GetModuleHandleW(wide.as_ptr()) }.is_null()
    }

    #[test]
    fn drop_calls_unload_once() {
        with_legacy(|stats| {
            let rt =
                LegacyRuntime::from_library(EntryKind::Loadu, open_legacy(), DIR).expect("init");
            assert_eq!(test_dll::legacy_stats(stats).unload, 0);
            drop(rt);
            let s = test_dll::legacy_stats(stats);
            assert_eq!((s.loadu, s.load, s.unload), (1, 0, 1));
        });
    }

    #[test]
    fn drop_frees_the_library() {
        let _g = test_dll::lock();
        let rt = LegacyRuntime::from_library(EntryKind::Loadu, open_legacy(), DIR).expect("init");
        assert!(legacy_module_loaded());
        drop(rt);
        assert!(!legacy_module_loaded());
    }

    #[test]
    fn dir_lossy_checks_like_from_library() {
        assert_eq!(
            LegacyRuntime::dir_lossy(r"C:\ghost\master").ok(),
            Some(false)
        );
        assert!(matches!(
            LegacyRuntime::dir_lossy("ghost"),
            Err(LoadError::InvalidPath)
        ));
        let lossy = unsafe { GetACP() } != CP_UTF8;
        assert_eq!(
            LegacyRuntime::dir_lossy("C:\\ghost\\\u{1F600}").ok(),
            Some(lossy)
        );
    }
}
