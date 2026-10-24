//! shiori4 — shiori4_abi の上に載る高水準層（SHIORI 側）
//!
//! - 芯: [`Ghost`] と、それを ABI につなぐ [`Adapter`]。v4 ではホストからの要求の原文（ヘッダ部分）を
//!   解析せずにそのまま `Ghost` に渡し、v3 では SHIORI/3.0 の全文とヘッダ部分の文字列形式だけを変換する
//! - 便利関数: [`protocol`]。ホスト側プロトコルの語に合わせたヘッダの解析と組み立て、プロパティの取得など。
//!   芯はこれを使わず、使わなくても仕様に従える。ホスト側クレート shiori4_host からも再エクスポートで使える
//!
//! 運ぶ中身の語（`getproperty`・`setproperty`・post の `Command`・`Tag`・`OnHostComplete`・拒否の返し方など）は、
//! ホスト側プロトコル仕様
//! <https://github.com/ekicyou/shiori4/blob/main/doc/host-protocol.md> が定める。
//!
//! ホストを呼べるかどうか（プロパティを同期で取れるか）は `Ghost` の実装に見せる（v4 なら `load` の host が `Some`）。
//!
//! ```no_run
//! use shiori4::protocol::{HeaderWriter, Headers, Lookup, Properties, HEADER_VALUE};
//!
//! struct MyGhost {
//!     props: Properties,
//! }
//!
//! impl shiori4::Ghost for MyGhost {
//!     fn load(host: Option<shiori4::ShioriHostHandle>, _dir: &str) -> Result<Self, shiori4::ShioriStatus> {
//!         Ok(Self { props: Properties::new(host) })
//!     }
//!
//!     // req はヘッダ部分の原文。解析は便利関数の Headers で行う
//!     fn request(&mut self, req: &str) -> shiori4::ShioriResponse {
//!         match Headers::parse(req).id() {
//!             Some("OnBoot") => {
//!                 let script = match self.props.get("currentghost.name") {
//!                     Lookup::Ready(name) => format!("\\0{name}だよ\\e"),
//!                     _ => "\\0……\\e".to_owned(),
//!                 };
//!                 HeaderWriter::new().push(HEADER_VALUE, &script).into_response()
//!             }
//!             _ => shiori4::ShioriResponse::no_content(),
//!         }
//!     }
//! }
//!
//! shiori4::export_ghost!(MyGhost);
//! # fn main() {}
//! ```

pub use shiori4_abi;
pub use shiori4_abi::*;

pub mod protocol;

// ============================================================
// SHIORI/3.0 の理由句
// ============================================================

pub const fn shiori_reason_phrase(s: ShioriStatus) -> &'static str {
    match s {
        200 => "OK",
        204 => "No Content",
        311 => "Not Enough",
        312 => "Advice",
        400 => "Bad Request",
        _ => "Internal Server Error",
    }
}

// ============================================================
// SHIORI 実装者向けの高水準 trait
// ============================================================

/// SHIORI の実装者が実装する trait。どちらのモードでもヘッダ部分の原文を受ける（解析は実装者が行う。
/// 便利関数の protocol::Headers::parse を使ってよい）
/// unload は Drop で表す。unload を始めた時点で、このインスタンスから取り出した ShioriHostPoster は
/// すべて無効になり、post は SHIORI_E_SHUTDOWN を返し、log は何もせず、どちらもホストに触れない（スレッド規約 5）。
/// そのため Poster を渡したスレッドを Drop の中で止めて join する必要はない
pub trait Ghost: Sized + 'static {
    /// v3（SHIORI3 互換モード）にも対応するか
    const SUPPORTS_V3: bool = true;

    /// v4 なら host は Some、v3 なら None
    ///
    /// v4 で Err を返す（またはパニックする）と、NULL を返す前に失敗のステータスを含む 1 行が
    /// 自動で host.log に出る（load の規約）。詳しい理由は自分で log すること。
    /// 状態は Self（インスタンス）ごとに持ち、グローバルな状態を避ける。
    /// export_ghost! で作った SHIORI は、同じプロセスで複数のインスタンスを受け付ける（load の規約）
    fn load(host: Option<ShioriHostHandle>, dir: &str) -> Result<Self, ShioriStatus>;

    /// GET に相当する。req はヘッダ部分の原文。v4 はホストが渡したまま、v3 は開始行と終端の空行を除いたもの。
    /// 応答の body はヘッダ部分だけ
    fn request(&mut self, req: &str) -> ShioriResponse;

    /// NOTIFY に相当する。既定の実装は 204
    ///
    /// 即 204 で返して処理を後回しにしてよい（スレッド規約 7）。ただし後回しにした処理の中では
    /// host.request（protocol::Properties::get など）を使えない（スレッド規約 2 の期間の外）。
    /// ホストへの依頼には ShioriHostPoster の post を使う
    fn notify(&mut self, _req: &str) -> ShioriStatus {
        SHIORI_204_NO_CONTENT
    }
}

/// Ghost を ABI 層の Shiori に変換するアダプタ
/// モードは load の時点で記録し、v3 のときだけ文字列形式を変換する
/// - v4: 要求を解析せずに原文のまま Ghost に渡す。Charset は UTF-8 固定。要求の Charset ヘッダは見ず（値で拒否しない）、応答に Charset を付けない
/// - v3: 従来どおり。UTF-8 以外の Charset は 400、応答に Charset がなければ "Charset: UTF-8" を補う
pub struct Adapter<G> {
    ghost: G,
    v3: bool,
}

impl<G: Ghost> Shiori for Adapter<G> {
    const SUPPORTS_V3: bool = G::SUPPORTS_V3;

    fn load(mode: ShioriLoadMode, dir: &str) -> Result<Self, ShioriStatus> {
        let (host, v3) = match mode {
            ShioriLoadMode::V4(h) => (Some(h), false),
            ShioriLoadMode::V3 => (None, true),
        };
        Ok(Self {
            ghost: G::load(host, dir)?,
            v3,
        })
    }

    fn request(&mut self, req: &str) -> ShioriResponse {
        if !self.v3 {
            return self.ghost.request(req);
        }
        let (status, text) = match v3::parse_request(req) {
            Err(st) => (st, v3::response(st, b"")),
            Ok((v3::Method::Get, h)) => {
                let r = self.ghost.request(h).into_owned();
                (r.status, v3::response(r.status, r.body.as_slice()))
            }
            Ok((v3::Method::Notify, h)) => {
                let st = self.ghost.notify(h);
                (st, v3::response(st, b""))
            }
        };
        ShioriResponse::with_status(status, text)
    }

    fn notify(&mut self, req: &str) -> ShioriStatus {
        if !self.v3 {
            return self.ghost.notify(req);
        }
        match v3::parse_request(req) {
            Ok((v3::Method::Notify, h)) => self.ghost.notify(h),
            Ok((v3::Method::Get, _)) => SHIORI_400_BAD_REQUEST,
            Err(st) => st,
        }
    }
}

/// エクスポート関数 shiori_get_api を生成する
#[macro_export]
macro_rules! export_ghost {
    ($t:ty) => {
        $crate::shiori4_abi::export_shiori!($crate::Adapter<$t>);
    };
}

// ============================================================
// v3（SHIORI/3.0 全文）の変換
// ============================================================

pub mod v3 {
    use super::*;

    pub enum Method {
        Get,
        Notify,
    }

    /// 開始行を読み、メソッドとヘッダ部分の原文に分ける
    /// ヘッダ部分は req の部分文字列で、開始行と終端の空行（とそれより後）を除き、各行の改行は残す
    /// 不正な開始行（SHIORI/2.x を含む）と UTF-8 以外の Charset は 400
    pub fn parse_request(req: &str) -> Result<(Method, &str), ShioriStatus> {
        let (line, rest) = req.split_once('\n').unwrap_or((req, ""));
        let method = match line.trim_end_matches('\r').trim_end() {
            "GET SHIORI/3.0" => Method::Get,
            "NOTIFY SHIORI/3.0" => Method::Notify,
            _ => return Err(SHIORI_400_BAD_REQUEST),
        };
        let end = rest
            .split_inclusive('\n')
            .take_while(|l| *l != "\n" && *l != "\r\n")
            .map(str::len)
            .sum();
        let headers = &rest[..end];
        if let Some(cs) = charset(headers) {
            if !cs.trim().eq_ignore_ascii_case("UTF-8") {
                return Err(SHIORI_400_BAD_REQUEST);
            }
        }
        Ok((method, headers))
    }

    /// Charset ヘッダの値。protocol::Headers と同じ規則（名前は大文字小文字を区別しない・
    /// 最初の `:` で分ける・値の先頭の空白を除く・同名が複数あれば最初のもの）で読む
    fn charset(headers: &str) -> Option<&str> {
        headers.lines().find_map(|line| {
            let (k, v) = line.split_once(':')?;
            k.trim()
                .eq_ignore_ascii_case("Charset")
                .then(|| v.trim_start())
        })
    }

    /// ステータスとヘッダ部分から SHIORI/3.0 の応答全文を組み立てる
    /// Charset がなければ "Charset: UTF-8" を補う
    pub fn response(status: ShioriStatus, body: &[u8]) -> Vec<u8> {
        let status = if status < 0 {
            SHIORI_500_INTERNAL_SERVER_ERROR
        } else {
            status
        };
        let mut v =
            format!("SHIORI/3.0 {} {}\r\n", status, shiori_reason_phrase(status)).into_bytes();

        let mut b = body;
        while b.ends_with(b"\r\n") {
            b = &b[..b.len() - 2]; // 末尾の改行と空行は付け直す
        }
        while b.ends_with(b"\n") {
            b = &b[..b.len() - 1];
        }
        let has_charset = b
            .split(|&c| c == b'\n')
            .any(|l| l.len() >= 8 && l[..8].eq_ignore_ascii_case(b"charset:"));
        if !has_charset {
            v.extend_from_slice(b"Charset: UTF-8\r\n");
        }
        if !b.is_empty() {
            v.extend_from_slice(b);
            v.extend_from_slice(b"\r\n");
        }
        v.extend_from_slice(b"\r\n"); // 終端の空行
        v
    }
}

// ============================================================
// テスト（DLL を介さず同一プロセスでテーブルを通す）
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::{HeaderWriter, Headers};
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Echo;

    impl Ghost for Echo {
        fn load(_host: Option<ShioriHostHandle>, _dir: &str) -> Result<Self, ShioriStatus> {
            Ok(Echo)
        }
        fn request(&mut self, req: &str) -> ShioriResponse {
            match Headers::parse(req).id() {
                Some(id) => HeaderWriter::new().push("Value", id).into_response(),
                None => ShioriResponse::bad_request(),
            }
        }
    }

    static API_V3: ShioriApi = ShioriApi::new::<Adapter<Echo>, { SHIORI_ABI_VERSION_3 }>();

    /// 要求ごとに数を数え、ID: post なら Poster を複製して別スレッドから post する
    struct Counter {
        count: u32,
        poster: ShioriHostPoster,
    }

    impl Ghost for Counter {
        const SUPPORTS_V3: bool = false;
        fn load(host: Option<ShioriHostHandle>, _dir: &str) -> Result<Self, ShioriStatus> {
            let host = host.ok_or(SHIORI_E_UNSUPPORTED)?;
            Ok(Counter {
                count: 0,
                poster: host.poster(),
            })
        }
        fn request(&mut self, req: &str) -> ShioriResponse {
            let v = if Headers::parse(req).id() == Some("post") {
                let p = self.poster.clone();
                std::thread::spawn(move || p.post_str("Command: test"))
                    .join()
                    .unwrap()
            } else {
                self.count += 1;
                self.count as ShioriStatus
            };
            HeaderWriter::new()
                .push("Value", &v.to_string())
                .into_response()
        }
    }

    static V4_API: ShioriApi = ShioriApi::new::<Adapter<Counter>, { SHIORI_ABI_VERSION }>();

    #[derive(Default)]
    struct CountHost {
        posts: AtomicU64,
    }

    impl ShioriHost for CountHost {
        fn request(&self, _req: &[u8]) -> ShioriResponse {
            ShioriResponse::no_content()
        }
        fn post(&self, _msg: &[u8]) -> ShioriStatus {
            self.posts.fetch_add(1, Ordering::SeqCst);
            SHIORI_204_NO_CONTENT
        }
    }

    #[test]
    fn v3_roundtrip() {
        let mut inst = API_V3.load_v3("/tmp").expect("load");
        let r = inst
            .request("GET SHIORI/3.0\r\nCharset: UTF-8\r\nID: OnBoot\r\n\r\n")
            .expect("abi");
        assert_eq!(r.status, 200);
        let text = r.body.as_str().unwrap();
        assert!(text.starts_with("SHIORI/3.0 200 OK\r\n"));
        assert!(text.contains("Value: OnBoot\r\n"));
        assert!(text.contains("Charset: UTF-8\r\n")); // v3 は応答に Charset を補う
        assert!(text.ends_with("\r\n\r\n"));

        // v3 は UTF-8 以外の Charset を 400 で拒む
        let r = inst
            .request("GET SHIORI/3.0\r\nCharset: Shift_JIS\r\nID: OnBoot\r\n\r\n")
            .expect("abi");
        assert_eq!(r.status, 400);

        let r = inst.request("GET SHIORI/2.6\r\n\r\n").expect("abi");
        assert_eq!(r.status, 400);
        assert_eq!(inst.notify("GET SHIORI/3.0\r\nID: x\r\n\r\n"), Ok(400));
        assert_eq!(inst.notify("NOTIFY SHIORI/3.0\r\nID: x\r\n\r\n"), Ok(204));
    }

    /// v3 の Ghost が受けた原文を残す（この Ghost を使うテストは 1 本だけ）
    static RECEIVED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

    struct Record;

    impl Ghost for Record {
        fn load(_host: Option<ShioriHostHandle>, _dir: &str) -> Result<Self, ShioriStatus> {
            Ok(Record)
        }
        fn request(&mut self, req: &str) -> ShioriResponse {
            RECEIVED.lock().unwrap().push(req.to_owned());
            ShioriResponse::no_content()
        }
        fn notify(&mut self, req: &str) -> ShioriStatus {
            RECEIVED.lock().unwrap().push(req.to_owned());
            SHIORI_204_NO_CONTENT
        }
    }

    static RECORD_V3: ShioriApi = ShioriApi::new::<Adapter<Record>, { SHIORI_ABI_VERSION_3 }>();

    #[test]
    fn v3_ghost_receives_headers_without_start_line_and_blank_line() {
        let mut inst = RECORD_V3.load_v3("/tmp").expect("load");
        assert_eq!(
            inst.request("GET SHIORI/3.0\r\nCharset: UTF-8\r\nID: OnBoot\r\n\r\n")
                .expect("abi")
                .status,
            204
        );
        assert_eq!(
            inst.notify("NOTIFY SHIORI/3.0\r\ncharset :  utf-8\r\nID: x\r\n\r\n"),
            Ok(204)
        );
        assert_eq!(
            inst.request("GET SHIORI/3.0\nID: lf\n\n")
                .expect("abi")
                .status,
            204
        );
        // 名前の大文字小文字・値の先頭の空白は Headers と同じ規則で読む
        assert_eq!(
            inst.request("GET SHIORI/3.0\r\nCHARSET:Shift_JIS\r\nID: x\r\n\r\n")
                .expect("abi")
                .status,
            400
        );
        // ヘッダ部分は最初の空行で終わる。その後ろは渡さない。終端の空行が無ければ最後の行まで
        assert_eq!(
            inst.request("GET SHIORI/3.0\r\nID: a\r\n\r\nID: after\r\n")
                .expect("abi")
                .status,
            204
        );
        assert_eq!(
            inst.request("GET SHIORI/3.0\r\nID: b\r\n")
                .expect("abi")
                .status,
            204
        );
        assert_eq!(
            *RECEIVED.lock().unwrap(),
            [
                "Charset: UTF-8\r\nID: OnBoot\r\n",
                "charset :  utf-8\r\nID: x\r\n",
                "ID: lf\n",
                "ID: a\r\n",
                "ID: b\r\n",
            ]
        );
    }

    // ---- v3 の host（copy_from_slice だけのテーブル） ----

    use std::ffi::c_void;
    use std::ptr::NonNull;

    /// Ghost::load が受けた host が None だったか（この Ghost を使うテストは 1 本だけ）
    static V3_HOST_WAS_NONE: std::sync::Mutex<Option<bool>> = std::sync::Mutex::new(None);

    struct SeeHost;

    impl Ghost for SeeHost {
        fn load(host: Option<ShioriHostHandle>, _dir: &str) -> Result<Self, ShioriStatus> {
            *V3_HOST_WAS_NONE.lock().unwrap() = Some(host.is_none());
            Ok(SeeHost)
        }
        fn request(&mut self, _req: &str) -> ShioriResponse {
            HeaderWriter::new().push("Value", "v3").into_response()
        }
    }

    static SEE_HOST_V3: ShioriApi = ShioriApi::new::<Adapter<SeeHost>, { SHIORI_ABI_VERSION_3 }>();

    /// C ヘッダの ShioriHostApi と同じ並び（shiori4_abi の大きさの assert が一致を保つ）
    #[repr(C)]
    struct RawHostApi {
        struct_size: u32,
        abi_version: u32,
        ctx: *mut c_void,
        request: Option<unsafe extern "C" fn()>,
        post: Option<unsafe extern "C" fn()>,
        log: Option<unsafe extern "C" fn()>,
        copy_from_slice:
            Option<unsafe extern "C" fn(*mut c_void, *const u8, usize, *mut ShioriBytes)>,
    }

    /// C ヘッダの ShioriApi と同じ並び
    #[repr(C)]
    struct RawApi {
        struct_size: u32,
        abi_version: u32,
        load: unsafe extern "C" fn(Option<NonNull<ShioriHostApi>>, *const u8, usize) -> *mut c_void,
        unload: unsafe extern "C" fn(*mut c_void),
        request:
            unsafe extern "C" fn(*mut c_void, *const u8, usize, *mut ShioriBytes) -> ShioriStatus,
    }

    static COPIES: AtomicU64 = AtomicU64::new(0);
    static RELEASES: AtomicU64 = AtomicU64::new(0);

    /// ホストの copy_from_slice。写した数と、host_release で解放した数を数える
    unsafe extern "C" fn host_copy(
        _ctx: *mut c_void,
        ptr: *const u8,
        len: usize,
        out: *mut ShioriBytes,
    ) {
        COPIES.fetch_add(1, Ordering::SeqCst);
        let buf: Box<[u8]> = unsafe { std::slice::from_raw_parts(ptr, len) }.into();
        let p = Box::into_raw(buf) as *const u8;
        unsafe {
            out.write(ShioriBytes::from_raw_parts(
                p,
                len,
                std::ptr::null_mut(),
                Some(host_release),
            ))
        }
    }

    unsafe extern "C" fn host_release(b: *mut ShioriBytes) {
        RELEASES.fetch_add(1, Ordering::SeqCst);
        let b = unsafe { &mut *b };
        let len = b.len();
        let p = b.as_slice().as_ptr() as *mut u8;
        drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(p, len)) });
        *b = ShioriBytes::empty();
    }

    #[test]
    fn v3_ghost_load_gets_none_and_response_is_made_by_copy_from_slice() {
        // 写しの並びがずれたら UB にせず、ここで落とす
        assert_eq!(size_of::<RawHostApi>(), size_of::<ShioriHostApi>());
        // RawApi は request までの頭の部分だけを写す
        assert_eq!(
            size_of::<RawApi>() + size_of::<usize>(),
            size_of::<ShioriApi>()
        );
        let table = RawHostApi {
            struct_size: size_of::<RawHostApi>() as u32,
            abi_version: SHIORI_ABI_VERSION_3,
            ctx: std::ptr::null_mut(),
            request: None,
            post: None,
            log: None,
            copy_from_slice: Some(host_copy),
        };
        // SAFETY: RawApi は ShioriApi と同じ並び
        let api = unsafe { &*(&SEE_HOST_V3 as *const ShioriApi as *const RawApi) };
        let host = NonNull::from(&table).cast::<ShioriHostApi>();
        let inst = unsafe { (api.load)(Some(host), "/ghost".as_ptr(), 6) };
        assert!(!inst.is_null());
        assert_eq!(*V3_HOST_WAS_NONE.lock().unwrap(), Some(true));

        let req = "GET SHIORI/3.0\r\nID: OnBoot\r\n\r\n";
        let mut out = ShioriBytes::empty();
        let st = unsafe { (api.request)(inst, req.as_ptr(), req.len(), &mut out) };
        let body = unsafe { ShioriOwnedBytes::from_raw(out) };
        assert_eq!(st, 200);
        assert!(body.as_str().unwrap().contains("Value: v3\r\n"));
        assert_eq!(COPIES.load(Ordering::SeqCst), 1);
        unsafe { (api.unload)(inst) };
        drop(body);
        assert_eq!(RELEASES.load(Ordering::SeqCst), 1);
    }

    // ---- v4 の Charset・複数インスタンス・Poster の複製 ----

    #[test]
    fn v4_ignores_charset_and_adds_none() {
        let host = ShioriHostBox::new(CountHost::default());
        let mut inst = V4_API.load(&host, "/ghost").expect("load");
        let r = inst
            .request("Charset: Shift_JIS\r\nID: OnBoot\r\n")
            .expect("abi");
        assert_eq!(r.status, 200);
        let text = r.body.as_str().unwrap();
        assert_eq!(text, "Value: 1\r\n");
        assert!(!text.to_ascii_lowercase().contains("charset"));
        assert_eq!(
            inst.notify("Charset: Shift_JIS\r\nID: OnSecondChange\r\n"),
            Ok(204)
        );
    }

    #[test]
    fn two_instances_from_one_table_keep_separate_state() {
        let host = ShioriHostBox::new(CountHost::default());
        let mut a = V4_API.load(&host, "/a").expect("load a");
        let mut b = V4_API.load(&host, "/b").expect("load b");
        let value = |i: &mut ShioriInstance<'_>| {
            i.request("ID: x\r\n")
                .unwrap()
                .body
                .as_str()
                .unwrap()
                .to_owned()
        };
        assert_eq!(value(&mut a), "Value: 1\r\n");
        assert_eq!(value(&mut a), "Value: 2\r\n");
        assert_eq!(value(&mut b), "Value: 1\r\n");
        drop(a);
        assert_eq!(value(&mut b), "Value: 2\r\n");
    }

    #[test]
    fn cloned_poster_posts_from_another_thread() {
        let host = ShioriHostBox::new(CountHost::default());
        let mut inst = V4_API.load(&host, "/ghost").expect("load");
        let r = inst.request("ID: post\r\n").expect("abi");
        assert_eq!(r.body.as_str().unwrap(), "Value: 204\r\n");
        assert_eq!(host.host().posts.load(Ordering::SeqCst), 1);
    }

    // ---- 芯が便利関数を参照しないことの検査（要件 9.3・10.1） ----

    /// テストより前（`mod tests {` の行の手前まで）の、空でなく `//` で始まらない行。
    /// 途中に `#[cfg(test)]` の項目が足されても、その後の芯の行を見落とさない
    fn code_lines_before_tests(src: &str) -> impl Iterator<Item = &str> {
        src.lines()
            .map(str::trim)
            .take_while(|l| *l != "mod tests {")
            .filter(|l| !l.is_empty() && !l.starts_with("//"))
    }

    /// code_lines_before_tests のうち `protocol` を語として含む行
    fn protocol_lines(src: &str) -> Vec<&str> {
        code_lines_before_tests(src)
            .filter(|l| {
                l.split(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .any(|w| w == "protocol")
            })
            .collect()
    }

    #[test]
    fn protocol_lines_counts_only_code_before_tests() {
        let allowed = "//! protocol の doc\n/// protocol::Headers を使ってよい\npub mod protocol;\nlet protocols = my_protocol;\n#[cfg(test)]\nmod tests {\n    use protocol::Headers;\n}\n";
        assert_eq!(protocol_lines(allowed), ["pub mod protocol;"]);

        let violated = "//! protocol の doc\npub mod protocol;\n    use crate::protocol::Headers;\n#[cfg(test)]\nmod tests {\n    use protocol::Headers;\n}\n";
        assert_eq!(
            protocol_lines(violated),
            ["pub mod protocol;", "use crate::protocol::Headers;"]
        );

        // 途中の `#[cfg(test)]` の項目は境目でない。その後の芯の参照も数える
        let early_cfg = "pub mod protocol;\n#[cfg(test)]\nmod helper;\n    use crate::protocol::Headers;\n#[cfg(test)]\nmod tests {\n    use protocol::Headers;\n}\n";
        assert_eq!(
            protocol_lines(early_cfg),
            ["pub mod protocol;", "use crate::protocol::Headers;"]
        );

        // 境目が無ければテストの中の参照も数え、検査は黙って通らない
        let no_boundary = "pub mod protocol;\nmod test {\n    use protocol::Headers;\n}\n";
        assert_eq!(protocol_lines(no_boundary).len(), 2);
    }

    #[test]
    fn core_refers_to_protocol_only_by_its_declaration() {
        assert_eq!(
            protocol_lines(include_str!("lib.rs")),
            ["pub mod protocol;"]
        );
    }

    #[test]
    fn protocol_module_does_not_refer_to_core() {
        let found: Vec<&str> = code_lines_before_tests(include_str!("protocol.rs"))
            .filter(|l| l.contains("Ghost") || l.contains("Adapter") || l.contains("v3::"))
            .collect();
        assert!(found.is_empty(), "{found:?}");
    }
}
