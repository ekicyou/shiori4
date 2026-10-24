//! shiori4 — shiori4_abi の上に載る高水準層（SHIORI 側）
//!
//! - v3（SHIORI/3.0 全文）と v4（ヘッダ部分だけ）の文字列形式の変換（Adapter）
//! - ホストとの間のメッセージの解析と組み立て（protocol）。ホスト側クレート shiori4_host と共有する
//!
//! v3 と v4 の差のうち、このクレートが透過にするのは「文字列形式」だけ。
//! ホストを呼べるかどうか（プロパティを同期で取れるか）は Ghost の実装に見せる。
//!
//! ```ignore
//! struct MyGhost { props: shiori4::protocol::Properties }
//!
//! impl shiori4::Ghost for MyGhost {
//!     fn load(host: Option<shiori4::ShioriHostHandle>, _dir: &str) -> Result<Self, shiori4::ShioriStatus> {
//!         Ok(Self { props: shiori4::protocol::Properties::new(host) })
//!     }
//!     fn request(&mut self, req: &shiori4::protocol::Headers<'_>) -> shiori4::ShioriResponse {
//!         match req.id() {
//!             Some("OnBoot") => match self.props.get("currentghost.name") {
//!                 shiori4::protocol::Lookup::Ready(name) => shiori4::protocol::HeaderWriter::new()
//!                     .push("Value", &format!("\\0{name}だよ\\e"))
//!                     .into_response(),
//!                 _ => shiori4::protocol::HeaderWriter::new().push("Value", "\\0……\\e").into_response(),
//!             },
//!             _ => shiori4::ShioriResponse::no_content(),
//!         }
//!     }
//! }
//!
//! shiori4::export_ghost!(MyGhost);
//! ```

pub use shiori4_abi;
pub use shiori4_abi::*;

pub mod protocol;

use protocol::Headers;

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

/// SHIORI の実装者が実装する trait。どちらのモードでもヘッダ部分だけを扱う
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

    /// GET に相当する。応答の body はヘッダ部分だけ
    fn request(&mut self, req: &Headers<'_>) -> ShioriResponse;

    /// NOTIFY に相当する。既定の実装は 204
    ///
    /// 即 204 で返して処理を後回しにしてよい（スレッド規約 7）。ただし後回しにした処理の中では
    /// host.request（Properties::get など）を使えない（スレッド規約 2 の期間の外）。
    /// ホストへの依頼には ShioriHostPoster の post を使う
    fn notify(&mut self, _req: &Headers<'_>) -> ShioriStatus {
        SHIORI_204_NO_CONTENT
    }
}

/// Ghost を ABI 層の Shiori に変換するアダプタ
/// モードは load の時点で記録し、v3 のときだけ文字列形式を変換する
/// - v4: Charset は UTF-8 固定。要求の Charset ヘッダは見ず（値で拒否しない）、応答に Charset を付けない
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
        Ok(Self { ghost: G::load(host, dir)?, v3 })
    }

    fn request(&mut self, req: &str) -> ShioriResponse {
        if !self.v3 {
            return self.ghost.request(&Headers::parse(req));
        }
        let (status, text) = match v3::parse_request(req) {
            Err(st) => (st, v3::response(st, b"")),
            Ok((v3::Method::Get, h)) => {
                let r = self.ghost.request(&h).into_owned();
                (r.status, v3::response(r.status, r.body.as_slice()))
            }
            Ok((v3::Method::Notify, h)) => {
                let st = self.ghost.notify(&h);
                (st, v3::response(st, b""))
            }
        };
        ShioriResponse::with_status(status, text)
    }

    fn notify(&mut self, req: &str) -> ShioriStatus {
        if !self.v3 {
            return self.ghost.notify(&Headers::parse(req));
        }
        match v3::parse_request(req) {
            Ok((v3::Method::Notify, h)) => self.ghost.notify(&h),
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

    /// 開始行を読み、メソッドとヘッダ部分に分ける
    /// 不正な開始行（SHIORI/2.x を含む）と UTF-8 以外の Charset は 400
    pub fn parse_request(req: &str) -> Result<(Method, Headers<'_>), ShioriStatus> {
        let (line, rest) = req.split_once('\n').unwrap_or((req, ""));
        let method = match line.trim_end_matches('\r').trim_end() {
            "GET SHIORI/3.0" => Method::Get,
            "NOTIFY SHIORI/3.0" => Method::Notify,
            _ => return Err(SHIORI_400_BAD_REQUEST),
        };
        let headers = Headers::parse(rest);
        if let Some(cs) = headers.get("Charset") {
            if !cs.trim().eq_ignore_ascii_case("UTF-8") {
                return Err(SHIORI_400_BAD_REQUEST);
            }
        }
        Ok((method, headers))
    }

    /// ステータスとヘッダ部分から SHIORI/3.0 の応答全文を組み立てる
    /// Charset がなければ "Charset: UTF-8" を補う
    pub fn response(status: ShioriStatus, body: &[u8]) -> Vec<u8> {
        let status = if status < 0 { SHIORI_500_INTERNAL_SERVER_ERROR } else { status };
        let mut v = format!("SHIORI/3.0 {} {}\r\n", status, shiori_reason_phrase(status)).into_bytes();

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
    use protocol::HeaderWriter;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Echo;

    impl Ghost for Echo {
        fn load(_host: Option<ShioriHostHandle>, _dir: &str) -> Result<Self, ShioriStatus> {
            Ok(Echo)
        }
        fn request(&mut self, req: &Headers<'_>) -> ShioriResponse {
            match req.id() {
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
            Ok(Counter { count: 0, poster: host.poster() })
        }
        fn request(&mut self, req: &Headers<'_>) -> ShioriResponse {
            let v = if req.id() == Some("post") {
                let p = self.poster.clone();
                std::thread::spawn(move || p.post_str("Command: test")).join().unwrap()
            } else {
                self.count += 1;
                self.count as ShioriStatus
            };
            HeaderWriter::new().push("Value", &v.to_string()).into_response()
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
        let r = inst.request("GET SHIORI/3.0\r\nCharset: Shift_JIS\r\nID: OnBoot\r\n\r\n").expect("abi");
        assert_eq!(r.status, 400);

        let r = inst.request("GET SHIORI/2.6\r\n\r\n").expect("abi");
        assert_eq!(r.status, 400);
        assert_eq!(inst.notify("GET SHIORI/3.0\r\nID: x\r\n\r\n"), Ok(400));
        assert_eq!(inst.notify("NOTIFY SHIORI/3.0\r\nID: x\r\n\r\n"), Ok(204));
    }


    // ---- v4 の Charset・複数インスタンス・Poster の複製 ----

    #[test]
    fn v4_ignores_charset_and_adds_none() {
        let host = ShioriHostBox::new(CountHost::default());
        let mut inst = V4_API.load(&host, "/ghost").expect("load");
        let r = inst.request("Charset: Shift_JIS\r\nID: OnBoot\r\n").expect("abi");
        assert_eq!(r.status, 200);
        let text = r.body.as_str().unwrap();
        assert_eq!(text, "Value: 1\r\n");
        assert!(!text.to_ascii_lowercase().contains("charset"));
        assert_eq!(inst.notify("Charset: Shift_JIS\r\nID: OnSecondChange\r\n"), Ok(204));
    }

    #[test]
    fn two_instances_from_one_table_keep_separate_state() {
        let host = ShioriHostBox::new(CountHost::default());
        let mut a = V4_API.load(&host, "/a").expect("load a");
        let mut b = V4_API.load(&host, "/b").expect("load b");
        let value = |i: &mut ShioriInstance<'_>| i.request("ID: x\r\n").unwrap().body.as_str().unwrap().to_owned();
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
}
