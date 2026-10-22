//! shiori4 — shiori4_abi の上に載る高水準層（SHIORI 側）
//!
//! - ヘッダの解析と生成（Headers / HeaderWriter）
//! - v3（SHIORI/3.0 全文）と v4（ヘッダ部分だけ）の文字列形式の変換（Adapter）
//! - プロパティ取得の抽象化（Properties / Lookup）
//! - post の受付 ID（Tag）の発行と待ち合わせ（PendingTags）
//! - ホストとの間のメッセージ名（host_protocol）。ホスト側クレート shiori4_host と共有する
//!
//! v3 と v4 の差のうち、このクレートが透過にするのは「文字列形式」だけ。
//! ホストを呼べるかどうか（プロパティを同期で取れるか）は Ghost の実装に見せる。
//!
//! ```ignore
//! struct MyGhost { props: shiori4::Properties }
//!
//! impl shiori4::Ghost for MyGhost {
//!     fn load(host: Option<shiori4::ShioriHostHandle>, _dir: &str) -> Result<Self, shiori4::ShioriStatus> {
//!         Ok(Self { props: shiori4::Properties::new(host) })
//!     }
//!     fn request(&mut self, req: &shiori4::Headers<'_>) -> shiori4::ShioriResponse {
//!         match req.id() {
//!             Some("OnBoot") => match self.props.get("currentghost.name") {
//!                 shiori4::Lookup::Ready(name) => shiori4::HeaderWriter::new()
//!                     .push("Value", &format!("\\0{name}だよ\\e"))
//!                     .into_response(),
//!                 _ => shiori4::HeaderWriter::new().push("Value", "\\0……\\e").into_response(),
//!             },
//!             _ => shiori4::ShioriResponse::no_content(),
//!         }
//!     }
//! }
//!
//! shiori4::export_ghost!(MyGhost);
//! ```

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, MutexGuard,
    },
};

pub use shiori4_abi;
pub use shiori4_abi::*;

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
// ヘッダの解析と生成
// ============================================================

/// "Name: Value" の並びを借用で解析したもの（開始行は含まない）
#[derive(Debug, Default, Clone)]
pub struct Headers<'a> {
    items: Vec<(&'a str, &'a str)>,
}

impl<'a> Headers<'a> {
    /// CRLF / LF のどちらの改行でも読む。空行と ':' を含まない行は無視する
    pub fn parse(text: &'a str) -> Self {
        let items = text
            .lines()
            .filter_map(|line| {
                let (k, v) = line.split_once(':')?;
                let k = k.trim();
                if k.is_empty() {
                    None
                } else {
                    Some((k, v.trim_start()))
                }
            })
            .collect();
        Self { items }
    }

    /// 名前は大文字小文字を区別しない。同名が複数あれば最初のもの
    pub fn get(&self, name: &str) -> Option<&'a str> {
        self.items
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|&(_, v)| v)
    }

    pub fn id(&self) -> Option<&'a str> {
        self.get("ID")
    }

    pub fn reference(&self, n: usize) -> Option<&'a str> {
        self.get(&format!("Reference{n}"))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&'a str, &'a str)> + '_ {
        self.items.iter().copied()
    }
}

/// ヘッダ部分（開始行なし、終端の空行なし）を組み立てる
#[derive(Debug, Default, Clone)]
pub struct HeaderWriter {
    buf: String,
}

impl HeaderWriter {
    pub fn new() -> Self {
        Self::default()
    }

    /// 値に含まれる CR / LF は取り除く（ヘッダの改行注入を防ぐ）
    pub fn push(mut self, name: &str, value: &str) -> Self {
        self.buf.push_str(name);
        self.buf.push_str(": ");
        self.buf.extend(value.chars().filter(|&c| c != '\r' && c != '\n'));
        self.buf.push_str("\r\n");
        self
    }

    /// 組み立て済みのヘッダ部分をそのまま後ろに足す（改行の正規化はしない）
    pub fn push_raw(mut self, headers: &str) -> Self {
        let h = headers.trim_end_matches(['\r', '\n']);
        if !h.is_empty() {
            self.buf.push_str(h);
            self.buf.push_str("\r\n");
        }
        self
    }

    pub fn finish(self) -> String {
        self.buf
    }

    /// 200 OK の応答にする
    pub fn into_response(self) -> ShioriResponse {
        ShioriResponse::ok(self.buf)
    }
}

// ============================================================
// SHIORI 実装者向けの高水準 trait
// ============================================================

/// SHIORI の実装者が実装する trait。どちらのモードでもヘッダ部分だけを扱う
/// unload は Drop で表す（ShioriHostPoster を渡したスレッドは Drop で止めて join すること）
pub trait Ghost: Sized + 'static {
    /// v3（SHIORI3 互換モード）にも対応するか
    const SUPPORTS_V3: bool = true;

    /// v4 なら host は Some、v3 なら None
    fn load(host: Option<ShioriHostHandle>, dir: &str) -> Result<Self, ShioriStatus>;

    /// GET に相当する。応答の body はヘッダ部分だけ
    fn request(&mut self, req: &Headers<'_>) -> ShioriResponse;

    /// NOTIFY に相当する。既定の実装は 204
    fn notify(&mut self, _req: &Headers<'_>) -> ShioriStatus {
        SHIORI_204_NO_CONTENT
    }
}

/// Ghost を ABI 層の Shiori に変換するアダプタ
/// モードは load の時点で記録し、v3 のときだけ文字列形式を変換する
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
// ホストとの間のメッセージ名（暫定。ホスト側プロトコル仕様で確定させる）
// ============================================================

/// SHIORI ⇔ ホスト間のメッセージ形式（v4）
///
/// - プロパティ取得（host.request）
///   要求: `ID: property` / `Reference0: <名前>`
///   応答: 200 + `Value: <値>`、存在しなければ 204
/// - 非同期依頼（host.post）
///   `Command: <コマンド名>`（必須）/ `Tag: <受付 ID>`（結果が必要な場合）/ 任意のヘッダ
/// - 完了通知（ホスト → SHIORI の request または notify）
///   `ID: OnHostComplete` / `Tag` / `Status: <ステータス>` /
///   `Reference0: <コマンド名>` / `Reference1..: <結果>`
pub mod host_protocol {
    pub const ID_PROPERTY: &str = "property";
    pub const HEADER_VALUE: &str = "Value";

    pub const HEADER_COMMAND: &str = "Command";
    pub const HEADER_TAG: &str = "Tag";

    pub const ID_HOST_COMPLETE: &str = "OnHostComplete";
    pub const HEADER_STATUS: &str = "Status";
}

// ============================================================
// プロパティ
// ============================================================

/// プロパティ取得の結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lookup<'a> {
    /// 値がある
    Ready(&'a str),
    /// v4 でホストが「存在しない」と答えた
    Missing,
    /// v3 で未取得。代替動作にし、後続イベントで set されるのを待つ
    Pending,
}

/// v4 ではホストに同期で問い合わせ、v3 では受動的に埋まるキャッシュとして振る舞う
///
/// v3 でプロパティを得る手順（さくらスクリプトでの取得指示や、届くイベントの形式）は
/// ベースウェアに依存するため、このクレートでは扱わない。
/// 届いた値は Ghost 側で解釈して set で取り込む。
pub struct Properties {
    host: Option<ShioriHostHandle>,
    cache: HashMap<String, String>,
}

impl Properties {
    pub fn new(host: Option<ShioriHostHandle>) -> Self {
        Self { host, cache: HashMap::new() }
    }

    /// ホストに問い合わせられるか（v4 なら true）
    pub fn is_live(&self) -> bool {
        self.host.is_some()
    }

    pub fn get(&mut self, name: &str) -> Lookup<'_> {
        if let Some(h) = &self.host {
            let req = HeaderWriter::new()
                .push("ID", host_protocol::ID_PROPERTY)
                .push("Reference0", name)
                .finish();
            match h.request_str(&req) {
                Ok(r) if r.status == SHIORI_200_OK => {
                    let value = r
                        .body
                        .as_str()
                        .ok()
                        .and_then(|s| Headers::parse(s).get(host_protocol::HEADER_VALUE).map(str::to_owned));
                    match value {
                        Some(v) => {
                            self.cache.insert(name.to_owned(), v);
                        }
                        None => return Lookup::Missing,
                    }
                }
                Ok(r) if r.status == SHIORI_204_NO_CONTENT => {
                    self.cache.remove(name);
                    return Lookup::Missing;
                }
                // ホスト側のエラーや ABI エラーのときは、キャッシュの値で代替する
                _ => {}
            }
        }
        match self.cache.get(name) {
            Some(v) => Lookup::Ready(v.as_str()),
            None if self.host.is_some() => Lookup::Missing,
            None => Lookup::Pending,
        }
    }

    /// キャッシュだけを見る（ホストに問い合わせない）
    pub fn cached(&self, name: &str) -> Option<&str> {
        self.cache.get(name).map(String::as_str)
    }

    /// v3 などで受け取った値を取り込む
    pub fn set(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.cache.insert(name.into(), value.into());
    }

    pub fn invalidate(&mut self, name: &str) {
        self.cache.remove(name);
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }
}

// ============================================================
// post の受付 ID（Tag）
// ============================================================

/// SHIORI が Tag を発行し、登録してから post する（完了イベントとの競合を防ぐ）
/// 任意のスレッドから使える（T: Send なら Sync）
pub struct PendingTags<T> {
    next: AtomicU64,
    map: Mutex<HashMap<u64, T>>,
}

impl<T> Default for PendingTags<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> PendingTags<T> {
    pub fn new() -> Self {
        Self { next: AtomicU64::new(1), map: Mutex::new(HashMap::new()) }
    }

    fn map(&self) -> MutexGuard<'_, HashMap<u64, T>> {
        self.map.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Tag を付けて post する。受け付けられなければ登録を取り消して Err を返す
    /// headers はヘッダ部分（Tag 以外。Command を含めること）
    pub fn post(&self, poster: &ShioriHostPoster, context: T, headers: &str) -> Result<u64, ShioriStatus> {
        let tag = self.next.fetch_add(1, Ordering::Relaxed);
        self.map().insert(tag, context);
        let st = poster.post_str(&format!("{}: {tag}\r\n{headers}", host_protocol::HEADER_TAG));
        if shiori_is_success(st) {
            Ok(tag)
        } else {
            self.map().remove(&tag);
            Err(st)
        }
    }

    pub fn take(&self, tag: u64) -> Option<T> {
        self.map().remove(&tag)
    }

    /// 完了イベントの Tag ヘッダから登録を取り出す
    pub fn take_from(&self, h: &Headers<'_>) -> Option<(u64, T)> {
        let tag: u64 = h.get(host_protocol::HEADER_TAG)?.trim().parse().ok()?;
        self.take(tag).map(|c| (tag, c))
    }

    pub fn len(&self) -> usize {
        self.map().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&self) {
        self.map().clear();
    }
}

// ============================================================
// テスト（DLL を介さず同一プロセスでテーブルを通す）
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(text.ends_with("\r\n\r\n"));

        let r = inst.request("GET SHIORI/2.6\r\n\r\n").expect("abi");
        assert_eq!(r.status, 400);
        assert_eq!(inst.notify("GET SHIORI/3.0\r\nID: x\r\n\r\n"), Ok(400));
        assert_eq!(inst.notify("NOTIFY SHIORI/3.0\r\nID: x\r\n\r\n"), Ok(204));
    }

    #[test]
    fn headers_parse() {
        let h = Headers::parse("ID: OnBoot\r\nreference0: a: b\r\n\r\n");
        assert_eq!(h.id(), Some("OnBoot"));
        assert_eq!(h.reference(0), Some("a: b"));
    }
}
