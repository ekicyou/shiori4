//! SHIORI ⇔ ホスト間のメッセージ（v4）の解析と組み立て
//!
//! - ヘッダの解析と生成（Headers / HeaderWriter）
//! - メッセージ名の定数
//! - プロパティ取得の抽象化（Properties / Lookup）
//! - post の受付 ID（Tag）の発行と待ち合わせ（PendingTags）
//!
//! メッセージ形式
//!
//! - プロパティ取得（host.request）
//!   要求: `ID: property` / `Reference0: <名前>`
//!   応答: 200 + `Value: <値>`、存在しなければ 204
//! - 非同期依頼（host.post）
//!   `Command: <コマンド名>`（必須）/ `Tag: <受付 ID>`（結果が必要な場合）/ 任意のヘッダ
//! - 完了通知（ホスト → SHIORI の request または notify）
//!   `ID: OnHostComplete` / `Tag` / `Status: <ステータス>` /
//!   `Reference0: <コマンド名>` / `Reference1..: <結果>`
//!
//! 使う型は shiori4_abi のものだけ。ホスト側クレート shiori4_host と共有する

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, MutexGuard,
    },
};

use shiori4_abi::{
    shiori_is_success, ShioriHostHandle, ShioriHostPoster, ShioriResponse, ShioriStatus, SHIORI_200_OK,
    SHIORI_204_NO_CONTENT, SHIORI_400_BAD_REQUEST,
};

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
// ホストとの間のメッセージ名
// ============================================================

// 文書（doc/host-protocol.md）の第 8 章と同じ綴り
pub const ID_PROPERTY: &str = "property";
pub const ID_GETPROPERTY: &str = "getproperty";
pub const ID_SETPROPERTY: &str = "setproperty";
pub const ID_HOST_COMPLETE: &str = "OnHostComplete";
pub const HEADER_ID: &str = "ID";
pub const HEADER_SENDER: &str = "Sender";
pub const HEADER_SENDER_TYPE: &str = "SenderType";
pub const HEADER_SECURITY_LEVEL: &str = "SecurityLevel";
pub const HEADER_SECURITY_ORIGIN: &str = "SecurityOrigin";
pub const HEADER_VALUE: &str = "Value";
pub const HEADER_COMMAND: &str = "Command";
pub const HEADER_TAG: &str = "Tag";
pub const HEADER_SCRIPT: &str = "Script";
pub const HEADER_EVENT: &str = "Event";
pub const HEADER_OPTION: &str = "Option";
pub const HEADER_STATUS: &str = "Status";
pub const HEADER_REASON: &str = "Reason";
pub const COMMAND_SEND: &str = "SEND";
pub const COMMAND_NOTIFY: &str = "NOTIFY";
pub const OPTION_NOBREAK: &str = "nobreak";
pub const OPTION_NOTRANSLATE: &str = "notranslate";
pub const OPTION_STRICT: &str = "strict";
pub const REASON_FORBIDDEN: &str = "forbidden";
pub const OUTCOME_COMPLETED: &str = "completed";
pub const OUTCOME_INTERRUPTED: &str = "interrupted";
pub const SENDER_TYPE_INTERNAL: &str = "internal";
pub const SECURITY_LEVEL_LOCAL: &str = "local";

// ============================================================
// SHIORI 側: 要求と post の組み立て
// ============================================================

/// `ID: getproperty` / `Reference0: <名前>`
pub fn getproperty_request(name: &str) -> String {
    HeaderWriter::new().push(HEADER_ID, ID_GETPROPERTY).push("Reference0", name).finish()
}

/// `ID: setproperty` / `Reference0: <名前>` / `Reference1: <値>`
pub fn setproperty_request(name: &str, value: &str) -> String {
    HeaderWriter::new()
        .push(HEADER_ID, ID_SETPROPERTY)
        .push("Reference0", name)
        .push("Reference1", value)
        .finish()
}

/// `Command: SEND` / [`Tag`] / `Script` / [`Option: a,b`]
pub fn send_post(script: &str, options: &[&str], tag: Option<&str>) -> String {
    let w = post_head(COMMAND_SEND, tag).push(HEADER_SCRIPT, script);
    post_options(w, options).finish()
}

/// `Command: NOTIFY` / [`Tag`] / `Event` / `Reference0..` / [`Option: a,b`]
pub fn notify_post(event: &str, refs: &[&str], options: &[&str], tag: Option<&str>) -> String {
    let w = post_head(COMMAND_NOTIFY, tag).push(HEADER_EVENT, event);
    let w = refs.iter().enumerate().fold(w, |w, (i, r)| w.push(&format!("Reference{i}"), r));
    post_options(w, options).finish()
}

fn post_head(command: &str, tag: Option<&str>) -> HeaderWriter {
    let w = HeaderWriter::new().push(HEADER_COMMAND, command);
    match tag {
        Some(t) => w.push(HEADER_TAG, t),
        None => w,
    }
}

/// 語は判定せずにそのまま載せる（知らない語も含む）
fn post_options(w: HeaderWriter, options: &[&str]) -> HeaderWriter {
    if options.is_empty() {
        w
    } else {
        w.push(HEADER_OPTION, &options.join(","))
    }
}

// ============================================================
// SHIORI 側: 応答と完了通知の読み分け
// ============================================================

/// 400 の `Reason` の読み分け（拒否の判定ではない。知らない語も読み分けて返す）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    /// Reason が無い 400: 不正（未対応を含む）
    Invalid,
    /// Reason: forbidden
    Forbidden,
    /// 未知の Reason: 理由不明の拒否
    Unknown,
}

impl Rejection {
    /// 前後の空白を除いて `forbidden` と完全に一致したときだけ Forbidden
    pub fn from_reason(reason: Option<&str>) -> Self {
        match reason.map(str::trim) {
            None => Self::Invalid,
            Some(REASON_FORBIDDEN) => Self::Forbidden,
            Some(_) => Self::Unknown,
        }
    }

    /// status が 400 のときだけ Some。headers は応答または完了通知のヘッダ部分
    pub fn of(status: ShioriStatus, headers: &Headers<'_>) -> Option<Self> {
        (status == SHIORI_400_BAD_REQUEST).then(|| Self::from_reason(headers.get(HEADER_REASON)))
    }
}

/// `OnHostComplete` の中身
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostComplete<'a> {
    /// Tag の値（変えない）
    pub tag: &'a str,
    /// Status（結果のステータスコード）
    pub status: ShioriStatus,
    /// Reference0（無ければ ""）
    pub command: &'a str,
    /// Reference1（completed / interrupted / 知らない語もそのまま）
    pub outcome: Option<&'a str>,
    /// status が 400 のときだけ
    pub rejection: Option<Rejection>,
}

impl<'a> HostComplete<'a> {
    /// ID が OnHostComplete で、Tag と数として読める Status があるときだけ Some。
    /// ID の値はヘッダ名ではないので大文字小文字を区別して比べる
    pub fn parse(h: &Headers<'a>) -> Option<Self> {
        if h.id()? != ID_HOST_COMPLETE {
            return None;
        }
        let tag = h.get(HEADER_TAG)?;
        let status = h.get(HEADER_STATUS)?.trim().parse::<ShioriStatus>().ok()?;
        Some(Self {
            tag,
            status,
            command: h.reference(0).unwrap_or(""),
            outcome: h.reference(1),
            rejection: Rejection::of(status, h),
        })
    }
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
                .push("ID", ID_PROPERTY)
                .push("Reference0", name)
                .finish();
            match h.request_str(&req) {
                Ok(r) if r.status == SHIORI_200_OK => {
                    let value = r
                        .body
                        .as_str()
                        .ok()
                        .and_then(|s| Headers::parse(s).get(HEADER_VALUE).map(str::to_owned));
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
        let st = poster.post_str(&format!("{}: {tag}\r\n{headers}", HEADER_TAG));
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
        let tag: u64 = h.get(HEADER_TAG)?.trim().parse().ok()?;
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
// テスト
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use shiori4_abi::{
        Shiori, ShioriApi, ShioriHost, ShioriHostBox, ShioriInstance, ShioriLoadMode, SHIORI_ABI_VERSION,
        SHIORI_E_UNSUPPORTED,
    };

    #[test]
    fn headers_parse() {
        let h = Headers::parse("ID: OnBoot\r\nreference0: a: b\r\n\r\n");
        assert_eq!(h.id(), Some("OnBoot"));
        assert_eq!(h.reference(0), Some("a: b"));
    }

    #[test]
    fn header_writer_strips_newlines() {
        let s = HeaderWriter::new().push("Value", "a\r\nb").push_raw("X: 1\r\n\r\n").finish();
        assert_eq!(s, "Value: ab\r\nX: 1\r\n");
    }

    #[test]
    fn property_requests() {
        assert_eq!(getproperty_request("baseware.name"), "ID: getproperty\r\nReference0: baseware.name\r\n");
        assert_eq!(
            setproperty_request("a.b", "x\r\ny"),
            "ID: setproperty\r\nReference0: a.b\r\nReference1: xy\r\n"
        );
    }

    #[test]
    fn send_posts() {
        assert_eq!(send_post(r"\0hi\e", &[], None), "Command: SEND\r\nScript: \\0hi\\e\r\n");
        assert_eq!(
            send_post("a\r\nb\nc", &[OPTION_NOBREAK], Some("42")),
            "Command: SEND\r\nTag: 42\r\nScript: abc\r\nOption: nobreak\r\n"
        );
        // 知らない語もそのまま載せる
        assert_eq!(
            send_post("x", &[OPTION_NOBREAK, "unknownword"], Some("t-1")),
            "Command: SEND\r\nTag: t-1\r\nScript: x\r\nOption: nobreak,unknownword\r\n"
        );
    }

    #[test]
    fn notify_posts() {
        assert_eq!(notify_post("OnX", &[], &[], None), "Command: NOTIFY\r\nEvent: OnX\r\n");
        assert_eq!(
            notify_post("OnX", &["a", "b\nc"], &[OPTION_NOTRANSLATE, OPTION_STRICT], Some("7")),
            "Command: NOTIFY\r\nTag: 7\r\nEvent: OnX\r\nReference0: a\r\nReference1: bc\r\nOption: notranslate,strict\r\n"
        );
    }

    #[test]
    fn shiori_side_builders_have_no_charset() {
        let all = [
            getproperty_request("a"),
            setproperty_request("a", "b"),
            send_post("s", &[OPTION_NOBREAK], Some("1")),
            notify_post("e", &["r"], &[OPTION_STRICT], Some("1")),
        ];
        for s in all {
            assert!(!s.is_empty());
            assert!(!s.to_ascii_lowercase().contains("charset"), "{s}");
        }
    }

    #[test]
    fn rejection_reads_reason() {
        assert_eq!(Rejection::from_reason(None), Rejection::Invalid);
        assert_eq!(Rejection::from_reason(Some(REASON_FORBIDDEN)), Rejection::Forbidden);
        assert_eq!(Rejection::from_reason(Some(" forbidden ")), Rejection::Forbidden);
        assert_eq!(Rejection::from_reason(Some("Forbidden")), Rejection::Unknown);
        assert_eq!(Rejection::from_reason(Some("busy")), Rejection::Unknown);

        let none = Headers::parse("");
        let forbidden = Headers::parse("Reason: forbidden\r\n");
        assert_eq!(Rejection::of(SHIORI_400_BAD_REQUEST, &none), Some(Rejection::Invalid));
        assert_eq!(Rejection::of(SHIORI_400_BAD_REQUEST, &forbidden), Some(Rejection::Forbidden));
        assert_eq!(Rejection::of(SHIORI_200_OK, &forbidden), None);
        assert_eq!(Rejection::of(SHIORI_204_NO_CONTENT, &none), None);
        assert_eq!(Rejection::of(500, &forbidden), None);
    }

    const COMPLETE_HEAD: &str =
        "Sender: areka\r\nSenderType: internal\r\nSecurityLevel: local\r\nSecurityOrigin: null\r\nID: OnHostComplete\r\n";

    fn complete(rest: &str) -> String {
        format!("{COMPLETE_HEAD}{rest}")
    }

    #[test]
    fn host_complete_send_results() {
        let t = complete("Tag: 42\r\nStatus: 200\r\nReference0: SEND\r\nReference1: completed\r\n");
        assert_eq!(
            HostComplete::parse(&Headers::parse(&t)),
            Some(HostComplete {
                tag: "42",
                status: SHIORI_200_OK,
                command: COMMAND_SEND,
                outcome: Some(OUTCOME_COMPLETED),
                rejection: None,
            })
        );

        let t = complete("Tag: abc-01\r\nStatus: 204\r\nReference0: SEND\r\n");
        assert_eq!(
            HostComplete::parse(&Headers::parse(&t)),
            Some(HostComplete {
                tag: "abc-01",
                status: SHIORI_204_NO_CONTENT,
                command: COMMAND_SEND,
                outcome: None,
                rejection: None,
            })
        );

        let t = complete("Tag: 7\r\nStatus: 400\r\nReference0: SEND\r\nReason: forbidden\r\n");
        assert_eq!(
            HostComplete::parse(&Headers::parse(&t)),
            Some(HostComplete {
                tag: "7",
                status: SHIORI_400_BAD_REQUEST,
                command: COMMAND_SEND,
                outcome: None,
                rejection: Some(Rejection::Forbidden),
            })
        );
    }

    #[test]
    fn host_complete_passes_unknown_words() {
        let t = complete("Tag: 1\r\nStatus: 200\r\nReference0: FUTURECMD\r\nReference1: someday\r\n");
        let c = HostComplete::parse(&Headers::parse(&t)).unwrap();
        assert_eq!((c.command, c.outcome), ("FUTURECMD", Some("someday")));
        // Reference0 が無ければ ""
        let c =
            HostComplete::parse(&Headers::parse("ID: OnHostComplete\r\nTag: 1\r\nStatus: 204\r\n"))
                .unwrap();
        assert_eq!(c.command, "");
    }

    #[test]
    fn host_complete_rejects_malformed() {
        let bad = [
            // ID 違い（値は大文字小文字を区別する）
            "ID: OnBoot\r\nTag: 1\r\nStatus: 200\r\nReference0: SEND\r\n",
            "ID: onhostcomplete\r\nTag: 1\r\nStatus: 200\r\nReference0: SEND\r\n",
            "Tag: 1\r\nStatus: 200\r\nReference0: SEND\r\n",
            // Status 無し・数でない
            "ID: OnHostComplete\r\nTag: 1\r\nReference0: SEND\r\n",
            "ID: OnHostComplete\r\nTag: 1\r\nStatus: ok\r\nReference0: SEND\r\n",
            // Tag 無し
            "ID: OnHostComplete\r\nStatus: 200\r\nReference0: SEND\r\n",
        ];
        for t in bad {
            assert_eq!(HostComplete::parse(&Headers::parse(t)), None, "{t}");
        }
    }

    /// Reference0 が "a" なら 200 + Value、それ以外は 204。post は全文を覚える
    #[derive(Default)]
    struct PropHost {
        posted: Mutex<String>,
    }

    impl ShioriHost for PropHost {
        fn request(&self, req: &[u8]) -> ShioriResponse {
            let h = Headers::parse(std::str::from_utf8(req).unwrap());
            match (h.id(), h.reference(0)) {
                (Some(ID_PROPERTY), Some("a")) => HeaderWriter::new().push(HEADER_VALUE, "1").into_response(),
                _ => ShioriResponse::no_content(),
            }
        }
        fn post(&self, msg: &[u8]) -> ShioriStatus {
            *self.posted.lock().unwrap() = String::from_utf8(msg.to_vec()).unwrap();
            SHIORI_204_NO_CONTENT
        }
    }

    /// 要求の中（host.request を使える期間）で Properties と PendingTags を使い、結果を Value に並べる
    struct Probe {
        props: Properties,
        poster: ShioriHostPoster,
        tags: PendingTags<&'static str>,
    }

    impl Shiori for Probe {
        fn load(mode: ShioriLoadMode, _dir: &str) -> Result<Self, ShioriStatus> {
            let ShioriLoadMode::V4(host) = mode else { return Err(SHIORI_E_UNSUPPORTED) };
            Ok(Self { poster: host.poster(), props: Properties::new(Some(host)), tags: PendingTags::new() })
        }
        fn request(&mut self, req: &str) -> ShioriResponse {
            let h = Headers::parse(req);
            let v = match h.id() {
                Some("post") => format!("{:?}", self.tags.post(&self.poster, "ctx", "Command: test\r\n")),
                Some(ID_HOST_COMPLETE) => format!("{:?}", self.tags.take_from(&h)),
                _ => {
                    let a = format!("{:?}", self.props.get("a"));
                    format!("{a} {:?}", self.props.get("b"))
                }
            };
            HeaderWriter::new().push(HEADER_VALUE, &v).into_response()
        }
    }

    static API: ShioriApi = ShioriApi::new::<Probe, { SHIORI_ABI_VERSION }>();

    fn value(inst: &mut ShioriInstance<'_>, req: &str) -> String {
        inst.request(req).expect("abi").body.as_str().unwrap().to_owned()
    }

    #[test]
    fn properties_get() {
        let host = ShioriHostBox::new(PropHost::default());
        let mut inst = API.load(&host, "/ghost").expect("load");
        assert_eq!(value(&mut inst, "ID: get\r\n"), "Value: Ready(\"1\") Missing\r\n");

        // v3 はホストに問い合わせず、取り込んだ値だけを返す
        let mut v3 = Properties::new(None);
        assert_eq!(v3.get("a"), Lookup::Pending);
        v3.set("a", "2");
        assert_eq!(v3.get("a"), Lookup::Ready("2"));
    }

    #[test]
    fn pending_tags_roundtrip() {
        let host = ShioriHostBox::new(PropHost::default());
        let mut inst = API.load(&host, "/ghost").expect("load");
        assert_eq!(value(&mut inst, "ID: post\r\n"), "Value: Ok(1)\r\n");
        assert_eq!(*host.host().posted.lock().unwrap(), "Tag: 1\r\nCommand: test\r\n");

        let done = "ID: OnHostComplete\r\nTag: 1\r\nStatus: 200\r\n";
        assert_eq!(value(&mut inst, done), "Value: Some((1, \"ctx\"))\r\n");
        assert_eq!(value(&mut inst, done), "Value: None\r\n");
    }
}
