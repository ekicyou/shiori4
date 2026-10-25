//! SHIORI/2.x の文字列の便利関数: 要求の組み立て、応答の読み分け、開始行の版の見分け（どれも状態を持たない）
//!
//! 文字コードの変換・Charset の方針・出来事の移し方・DLL の読み込みと HGLOBAL は持たない
//! （`LegacyRuntime` とホストが受け持つ）。

// ============================================================
// 要求の組み立て
// ============================================================

use shiori4::protocol::{HEADER_EVENT, HEADER_ID, HeaderWriter};

/// SHIORI/2.x の要求の全文を作る。
///
/// 開始行は `<method> <command> SHIORI/<version>`（`command` が `None` なら
/// `<method> SHIORI/<version>`）で、続けて `headers` を渡した順に `名前: 値` の行で書き、
/// 終端の空行で閉じる。行の区切りは CRLF。値に含まれる CR・LF は取り除く。
/// 渡されないヘッダ（`Charset` を含む）は書かず、文字コードも変えない。
///
/// 開始行の語（`method`・`command`・`version`）に空白・CR・LF が含まれるか、語が空なら
/// 要求を作らずに `None` を返す。ヘッダの名前は検査しない。
///
/// ```
/// use shiori4_host::shiori2::request;
///
/// let req = request("GET", Some("Sentence"), "2.2", &[("Charset", "Shift_JIS"), ("Event", "OnBoot")]);
/// assert_eq!(
///     req.as_deref(),
///     Some("GET Sentence SHIORI/2.2\r\nCharset: Shift_JIS\r\nEvent: OnBoot\r\n\r\n")
/// );
/// assert_eq!(request("GET", Some("Sen tence"), "2.2", &[]), None);
/// ```
pub fn request(
    method: &str,
    command: Option<&str>,
    version: &str,
    headers: &[(&str, &str)],
) -> Option<String> {
    let words_ok = is_word(method) && command.is_none_or(is_word) && is_word(version);
    words_ok.then(|| build(method, command, version, headers))
}

/// 開始行の語として書ける（空でなく、ASCII の空白・CR・LF を含まない）
fn is_word(s: &str) -> bool {
    !s.is_empty() && !s.bytes().any(|b| b.is_ascii_whitespace())
}

/// 検査を済ませた語とヘッダから全文を作る（命令ごとの関数もここを通る）
fn build(method: &str, command: Option<&str>, version: &str, headers: &[(&str, &str)]) -> String {
    let mut out = String::from(method);
    if let Some(c) = command {
        out.push(' ');
        out.push_str(c);
    }
    out.push_str(" SHIORI/");
    out.push_str(version);
    out.push_str("\r\n");
    let w = headers
        .iter()
        .fold(HeaderWriter::new(), |w, &(n, v)| w.push(n, v));
    out.push_str(&w.finish());
    out.push_str("\r\n");
    out
}

/// 共通のヘッダを先に、命令に固有のヘッダを後に書いて全文を作る（語は定数なので検査しない）
fn build_with(
    method: &str,
    command: Option<&str>,
    version: &str,
    common: &[(&str, &str)],
    own: &[(&str, &str)],
) -> String {
    build(method, command, version, &[common, own].concat())
}

/// `Reference0` から順の名前を `refs` の数だけ作る
fn reference_names(refs: &[&str]) -> Vec<String> {
    (0..refs.len()).map(|i| format!("Reference{i}")).collect()
}

/// 名前の並びと値の並びを `(名前, 値)` の並びにする
fn pairs<'a>(names: &'a [String], values: &[&'a str]) -> Vec<(&'a str, &'a str)> {
    names
        .iter()
        .map(String::as_str)
        .zip(values.iter().copied())
        .collect()
}

/// 版の問い合わせ `GET Version SHIORI/2.6` の全文を作る（SSP が最初に送る要求）。
///
/// `common`（`Charset`・`Sender` など）だけを渡した順に書く。命令に固有のヘッダは無い。
/// 既定の値は持たず、失敗しない。
///
/// ```
/// use shiori4_host::shiori2::get_version;
///
/// assert_eq!(
///     get_version(&[("Charset", "UTF-8"), ("Sender", "SSP")]),
///     "GET Version SHIORI/2.6\r\nCharset: UTF-8\r\nSender: SSP\r\n\r\n"
/// );
/// ```
pub fn get_version(common: &[(&str, &str)]) -> String {
    build_with("GET", Some("Version"), "2.6", common, &[])
}

/// 出来事 `GET Sentence SHIORI/2.2` の全文を作る。
///
/// `common` の後に `Event: <event>` と、`references` を `Reference0` から順に書く。
/// 出来事の名前と `Reference` は変えずに書き、SHIORI/3.0 の出来事からの移し方は持たない。
/// 失敗しない。
pub fn get_sentence(common: &[(&str, &str)], event: &str, references: &[&str]) -> String {
    let names = reference_names(references);
    let own = [&[(HEADER_EVENT, event)][..], &pairs(&names, references)].concat();
    build_with("GET", Some("Sentence"), "2.2", common, &own)
}

/// 語の問い合わせ `GET Word SHIORI/2.0` の全文を作る。
///
/// `common` の後に `Type: <kind>`（`\ms` など）を書く。失敗しない。
pub fn get_word(common: &[(&str, &str)], kind: &str) -> String {
    build_with("GET", Some("Word"), "2.0", common, &[("Type", kind)])
}

/// 状態の問い合わせ `GET Status SHIORI/2.0` の全文を作る。
///
/// `common` だけを書く。命令に固有のヘッダは無い。失敗しない。
pub fn get_status(common: &[(&str, &str)]) -> String {
    build_with("GET", Some("Status"), "2.0", common, &[])
}

/// 文字列の資源 `GET String SHIORI/2.5` の全文を作る。
///
/// `common` の後に `ID: <id>`（`homeurl` など）を書く。失敗しない。
pub fn get_string(common: &[(&str, &str)], id: &str) -> String {
    build_with("GET", Some("String"), "2.5", common, &[(HEADER_ID, id)])
}

/// 主のゴーストの名前 `NOTIFY OwnerGhostName SHIORI/2.3` の全文を作る。
///
/// `common` の後に `Ghost: <ghost>` を書く。失敗しない。
pub fn notify_owner_ghost_name(common: &[(&str, &str)], ghost: &str) -> String {
    build_with(
        "NOTIFY",
        Some("OwnerGhostName"),
        "2.3",
        common,
        &[("Ghost", ghost)],
    )
}

/// ほかのゴーストの名前 `NOTIFY OtherGhostName SHIORI/2.3` の全文を作る。
///
/// `common` の後に、`ghosts` の `(名前, 本体のサーフィス, 相方のサーフィス)` ごとに
/// `GhostEx: 名前\x01本体のサーフィス\x01相方のサーフィス` の 1 行を渡した順に書く。失敗しない。
pub fn notify_other_ghost_name(common: &[(&str, &str)], ghosts: &[(&str, &str, &str)]) -> String {
    let values: Vec<String> = ghosts
        .iter()
        .map(|(name, sakura, kero)| format!("{name}\x01{sakura}\x01{kero}"))
        .collect();
    let own: Vec<(&str, &str)> = values.iter().map(|v| ("GhostEx", v.as_str())).collect();
    build_with("NOTIFY", Some("OtherGhostName"), "2.3", common, &own)
}

/// 教える `TEACH SHIORI/2.4` の全文を作る（命令の語は無い）。
///
/// `common` の後に `Word: <word>` と、`references` を `Reference0` から順に書く。失敗しない。
pub fn teach(common: &[(&str, &str)], word: &str, references: &[&str]) -> String {
    let names = reference_names(references);
    let own = [&[("Word", word)][..], &pairs(&names, references)].concat();
    build_with("TEACH", None, "2.4", common, &own)
}

/// スクリプトの変換 `TRANSLATE Sentence SHIORI/2.6` の全文を作る。
///
/// `common` の後に `Sentence: <sentence>` を書く。失敗しない。
pub fn translate_sentence(common: &[(&str, &str)], sentence: &str) -> String {
    build_with(
        "TRANSLATE",
        Some("Sentence"),
        "2.6",
        common,
        &[("Sentence", sentence)],
    )
}

// ============================================================
// 応答の読み分け
// ============================================================

use shiori4::protocol::{HEADER_STATUS, HEADER_VALUE, Headers};
use shiori4::shiori4_abi::ShioriStatus;

/// SHIORI/2.x の応答を版・ステータス・ヘッダに読み分けたもの（ヘッダは応答の全文の借用）
#[derive(Debug, Clone)]
pub struct Response<'a> {
    /// 開始行の版
    pub version: Version,
    /// 開始行のステータスの数（311 は `SHIORI_311_NOT_ENOUGH`、312 は `SHIORI_312_ADVICE` と比べる）
    pub status: ShioriStatus,
    /// 空行までのヘッダ（`Charset` もここから読む）
    pub headers: Headers<'a>,
}

impl<'a> Response<'a> {
    /// 応答の全文（文字コードを変えた後の UTF-8）を読み分ける。
    ///
    /// 開始行は最初の LF までの 1 行（末尾の CR を除く）で、先頭を [`version_of`] と同じ決まりで
    /// 版に読み、ASCII の空白で区切った 2 つ目の語を ASCII の数字だけのステータスの数として読む。
    /// 理由句は読まず、知らない数（299 など）もそのまま返す。ヘッダは開始行の次の行から
    /// 最初の空行の手前まで（CRLF と LF のどちらも）を読み、空行より後ろは読まない。
    /// 文字コードは変えず、状態を持たない。
    ///
    /// 版が分からないか、ステータスの語が無い・数でない・溢れるなら `None` を返す。
    ///
    /// ```
    /// use shiori4_host::shiori2::{Response, Version};
    ///
    /// let res = Response::parse("SHIORI/2.6 200 OK\r\nID: KAWARI.kdt\r\n\r\n").unwrap();
    /// assert_eq!(res.version, Version::V2);
    /// assert_eq!(res.status, 200);
    /// assert_eq!(res.headers.get("ID"), Some("KAWARI.kdt"));
    /// assert!(Response::parse("SHIORI/2.6 OK\r\n\r\n").is_none());
    /// ```
    pub fn parse(text: &'a str) -> Option<Self> {
        let (line, rest) = text.split_once('\n').unwrap_or((text, ""));
        let line = line.strip_suffix('\r').unwrap_or(line);
        let version = start_line_version(line.as_bytes())?;
        let word = line.split_ascii_whitespace().nth(1)?;
        if !word.bytes().all(|b| b.is_ascii_digit()) {
            return None; // `+200`・`-1` を数に読まない
        }
        let status = word.parse().ok()?;
        let end = rest
            .split_inclusive('\n')
            .take_while(|l| *l != "\n" && *l != "\r\n")
            .map(str::len)
            .sum();
        Some(Self {
            version,
            status,
            headers: Headers::parse(&rest[..end]),
        })
    }

    /// `GET Sentence`・`TRANSLATE Sentence` の応答の `Sentence`（スクリプト）。無ければ `Value`、どちらも無ければ `None`。
    ///
    /// ステータスを見ずに（311・312 の応答からも）取り出す。値はヘッダの借用のままで、
    /// 文字コードの変換もさくらスクリプトのエスケープの解釈もしない。
    pub fn sentence(&self) -> Option<&'a str> {
        self.fixed_or_value("Sentence")
    }

    /// `GET Word` の応答の `Word`。無ければ `Value`、どちらも無ければ `None`（ステータスは見ない）
    pub fn word(&self) -> Option<&'a str> {
        self.fixed_or_value("Word")
    }

    /// `GET String` の応答の `String`。無ければ `Value`、どちらも無ければ `None`（ステータスは見ない）
    pub fn string(&self) -> Option<&'a str> {
        self.fixed_or_value("String")
    }

    /// `GET Status` の応答の `Status`（`,` で分けずに原文のまま）。無ければ `Value`、どちらも無ければ `None`。
    ///
    /// 欄の [`status`](Self::status)（ステータスの数）とは別のもの。
    pub fn status_header(&self) -> Option<&'a str> {
        self.fixed_or_value(HEADER_STATUS)
    }

    /// `GET Version` の応答の `ID`（名前）。無ければ `None`（`Value` に代えない）
    pub fn id(&self) -> Option<&'a str> {
        self.headers.get(HEADER_ID)
    }

    /// `GET Version` の応答の `Craftman`（作者）。無ければ `None`（`Value` に代えない）
    pub fn craftman(&self) -> Option<&'a str> {
        self.headers.get("Craftman")
    }

    /// `GET Version` の応答の `Version`（SHIORI の版の文字列）。無ければ `Value`、どちらも無ければ `None`。
    ///
    /// 欄の [`version`](Self::version)（開始行の版）とは別のもの。
    pub fn version_header(&self) -> Option<&'a str> {
        self.fixed_or_value("Version")
    }

    /// 決まったヘッダ、無ければ `Value`（SHIORI/3.0 の書き方で答える SHIORI/2.x の SHIORI のため）
    fn fixed_or_value(&self, name: &str) -> Option<&'a str> {
        self.headers
            .get(name)
            .or_else(|| self.headers.get(HEADER_VALUE))
    }
}

// ============================================================
// 版の見分け
// ============================================================

/// 応答の開始行の版（SHIORI/2.x か SHIORI/3.x か）。小さい方の番号は持たない
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    /// SHIORI/3.x（開始行が `SHIORI/3` で始まる）。この後は SHIORI/3.0 の形で話す
    V3,
    /// SHIORI/2.x（開始行が `SHIORI/2` で始まる）。この後は SHIORI/2.x で話す
    V2,
}

/// 応答の開始行の版を見分ける。分からなければ `None`（空の応答も `None`）。
///
/// 応答の先頭の 8 バイトだけを読み、ASCII の `SHIORI/2` なら [`Version::V2`]、
/// `SHIORI/3` なら [`Version::V3`] を返す。それ以外（8 バイトに足りない・ASCII でない・
/// 小文字・`SHIORI/1`・`SHIORI/4` など）は `None`。ステータス・ヘッダ・`Charset` は読まず、
/// 文字コードも仮定しないので、`&str` と `&[u8]` で同じ結果になる。
/// どの要求への応答にも当てられ、状態を持たず、DLL を呼ばない。
///
/// ```
/// use shiori4_host::shiori2::{Version, version_of};
///
/// assert_eq!(version_of("SHIORI/2.6 200 OK\r\n\r\n"), Some(Version::V2));
/// assert_eq!(version_of(b"SHIORI/3.0 200 OK\r\n\r\n"), Some(Version::V3));
/// assert_eq!(version_of(""), None);
/// ```
pub fn version_of(response: impl AsRef<[u8]>) -> Option<Version> {
    start_line_version(response.as_ref())
}

/// 開始行の先頭の 8 バイトを版に読む（応答の読み分けもここを通る）
fn start_line_version(line: &[u8]) -> Option<Version> {
    match line.get(..8)? {
        b"SHIORI/2" => Some(Version::V2),
        b"SHIORI/3" => Some(Version::V3),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- 要求の組み立て（汎用の形） ----

    #[test]
    fn request_writes_start_line_headers_in_order_and_blank_line() {
        let req = request(
            "GET",
            Some("Sentence"),
            "2.2",
            &[("Charset", "Shift_JIS"), ("Event", "OnBoot")],
        );
        assert_eq!(
            req.as_deref(),
            Some("GET Sentence SHIORI/2.2\r\nCharset: Shift_JIS\r\nEvent: OnBoot\r\n\r\n")
        );
    }

    #[test]
    fn request_without_command_omits_the_word() {
        let req = request("TEACH", None, "2.4", &[("Word", "x")]);
        assert_eq!(req.as_deref(), Some("TEACH SHIORI/2.4\r\nWord: x\r\n\r\n"));
    }

    #[test]
    fn request_writes_version_as_given() {
        for v in ["2.0", "2.1", "2.2", "2.3", "2.4", "2.5", "2.6"] {
            let req = request("GET", Some("Version"), v, &[]).unwrap();
            assert_eq!(req, format!("GET Version SHIORI/{v}\r\n\r\n"));
        }
    }

    #[test]
    fn request_strips_cr_lf_from_values() {
        let req = request("GET", Some("Sentence"), "2.2", &[("Event", "a\r\nb")]).unwrap();
        assert_eq!(req, "GET Sentence SHIORI/2.2\r\nEvent: ab\r\n\r\n");
        assert_eq!(req.matches("\r\n").count(), 3);
    }

    #[test]
    fn request_rejects_broken_or_empty_words() {
        for bad in ["", "a b", "a\rb", "a\nb", " ", "\r", "\n"] {
            assert_eq!(
                request(bad, Some("Sentence"), "2.2", &[]),
                None,
                "method {bad:?}"
            );
            assert_eq!(
                request("GET", Some(bad), "2.2", &[]),
                None,
                "command {bad:?}"
            );
            assert_eq!(
                request("GET", Some("Sentence"), bad, &[]),
                None,
                "version {bad:?}"
            );
        }
    }

    #[test]
    fn request_without_headers_writes_no_charset() {
        let req = request("GET", Some("Version"), "2.6", &[]).unwrap();
        assert_eq!(req, "GET Version SHIORI/2.6\r\n\r\n");
        assert!(!req.contains("Charset"));
    }

    // ---- 要求の組み立て（命令ごと） ----

    const C: &[(&str, &str)] = &[("Charset", "Shift_JIS"), ("Sender", "SSP")];

    #[test]
    fn get_version_matches_ssp_log() {
        assert_eq!(
            get_version(&[("Charset", "UTF-8"), ("Sender", "SSP")]),
            "GET Version SHIORI/2.6\r\nCharset: UTF-8\r\nSender: SSP\r\n\r\n"
        );
    }

    #[test]
    fn each_command_matches_ssp_log_form() {
        let lv = &[
            ("Charset", "Shift_JIS"),
            ("SecurityLevel", "local"),
            ("Sender", "SSP"),
        ];
        assert_eq!(
            get_sentence(lv, "OnBoot", &["0"]),
            "GET Sentence SHIORI/2.2\r\nCharset: Shift_JIS\r\nSecurityLevel: local\r\nSender: SSP\r\nEvent: OnBoot\r\nReference0: 0\r\n\r\n"
        );
        assert_eq!(
            get_word(C, "\\ms"),
            "GET Word SHIORI/2.0\r\nCharset: Shift_JIS\r\nSender: SSP\r\nType: \\ms\r\n\r\n"
        );
        assert_eq!(
            get_status(C),
            "GET Status SHIORI/2.0\r\nCharset: Shift_JIS\r\nSender: SSP\r\n\r\n"
        );
        let origin = &[
            ("Charset", "Shift_JIS"),
            ("SecurityLevel", "local"),
            ("SecurityOrigin", "null"),
            ("Sender", "SSP"),
        ];
        assert_eq!(
            get_string(origin, "homeurl"),
            "GET String SHIORI/2.5\r\nCharset: Shift_JIS\r\nSecurityLevel: local\r\nSecurityOrigin: null\r\nSender: SSP\r\nID: homeurl\r\n\r\n"
        );
        assert_eq!(
            notify_owner_ghost_name(C, "さくら"),
            "NOTIFY OwnerGhostName SHIORI/2.3\r\nCharset: Shift_JIS\r\nSender: SSP\r\nGhost: さくら\r\n\r\n"
        );
        assert_eq!(
            notify_other_ghost_name(&[], &[("まゆら", "0", "10")]),
            "NOTIFY OtherGhostName SHIORI/2.3\r\nGhostEx: まゆら\x010\x0110\r\n\r\n"
        );
        assert_eq!(
            teach(C, "りんご", &["x"]),
            "TEACH SHIORI/2.4\r\nCharset: Shift_JIS\r\nSender: SSP\r\nWord: りんご\r\nReference0: x\r\n\r\n"
        );
        assert_eq!(
            translate_sentence(origin, "\\h\\s[0]"),
            "TRANSLATE Sentence SHIORI/2.6\r\nCharset: Shift_JIS\r\nSecurityLevel: local\r\nSecurityOrigin: null\r\nSender: SSP\r\nSentence: \\h\\s[0]\r\n\r\n"
        );
    }

    #[test]
    fn commands_without_common_write_no_charset() {
        assert_eq!(get_status(&[]), "GET Status SHIORI/2.0\r\n\r\n");
    }

    #[test]
    fn references_are_numbered_from_zero_in_order() {
        assert_eq!(
            get_sentence(C, "OnBoot", &["a", "b"]),
            "GET Sentence SHIORI/2.2\r\nCharset: Shift_JIS\r\nSender: SSP\r\nEvent: OnBoot\r\nReference0: a\r\nReference1: b\r\n\r\n"
        );
        assert_eq!(
            teach(&[], "w", &["a", "b"]),
            "TEACH SHIORI/2.4\r\nWord: w\r\nReference0: a\r\nReference1: b\r\n\r\n"
        );
    }

    #[test]
    fn ghost_ex_is_one_line_per_ghost_in_order() {
        let req = notify_other_ghost_name(C, &[("A", "0", "10"), ("B", "3", "11")]);
        assert_eq!(
            req,
            "NOTIFY OtherGhostName SHIORI/2.3\r\nCharset: Shift_JIS\r\nSender: SSP\r\nGhostEx: A\x010\x0110\r\nGhostEx: B\x013\x0111\r\n\r\n"
        );
    }

    #[test]
    fn event_name_is_written_as_given() {
        // SHIORI/3.0 の名前でも移さずに書く
        let req = get_sentence(&[], "OnSecondChange", &["1"]);
        assert!(req.contains("\r\nEvent: OnSecondChange\r\nReference0: 1\r\n"));
    }

    #[test]
    fn each_command_equals_generic_request() {
        let r = |m, c, v, own: &[(&str, &str)]| request(m, c, v, &[C, own].concat()).unwrap();
        assert_eq!(get_version(C), r("GET", Some("Version"), "2.6", &[]));
        assert_eq!(
            get_sentence(C, "OnBoot", &["a", "b"]),
            r(
                "GET",
                Some("Sentence"),
                "2.2",
                &[
                    ("Event", "OnBoot"),
                    ("Reference0", "a"),
                    ("Reference1", "b")
                ]
            )
        );
        assert_eq!(
            get_word(C, "\\ms"),
            r("GET", Some("Word"), "2.0", &[("Type", "\\ms")])
        );
        assert_eq!(get_status(C), r("GET", Some("Status"), "2.0", &[]));
        assert_eq!(
            get_string(C, "homeurl"),
            r("GET", Some("String"), "2.5", &[("ID", "homeurl")])
        );
        assert_eq!(
            notify_owner_ghost_name(C, "g"),
            r("NOTIFY", Some("OwnerGhostName"), "2.3", &[("Ghost", "g")])
        );
        assert_eq!(
            notify_other_ghost_name(C, &[("A", "0", "10"), ("B", "3", "11")]),
            r(
                "NOTIFY",
                Some("OtherGhostName"),
                "2.3",
                &[("GhostEx", "A\x010\x0110"), ("GhostEx", "B\x013\x0111")]
            )
        );
        assert_eq!(
            teach(C, "w", &["a"]),
            r("TEACH", None, "2.4", &[("Word", "w"), ("Reference0", "a")])
        );
        assert_eq!(
            translate_sentence(C, "s"),
            r("TRANSLATE", Some("Sentence"), "2.6", &[("Sentence", "s")])
        );
    }

    // ---- 版の見分け ----

    #[test]
    fn version_of_reads_bytes_with_shift_jis_body() {
        let res: &[u8] = b"SHIORI/2.6 200 OK\r\nSentence: \x82\xa0\r\n\r\n";
        assert_eq!(version_of(res), Some(Version::V2));
    }

    #[test]
    fn version_of_str_and_bytes_agree() {
        for s in [
            "SHIORI/2.6 200 OK\r\n\r\n",
            "SHIORI/3.0 200 OK\r\n\r\n",
            "HTTP/1.0 200 OK\r\n\r\n",
            "",
        ] {
            assert_eq!(version_of(s), version_of(s.as_bytes()), "{s:?}");
        }
    }

    #[test]
    fn version_of_reads_only_the_first_line() {
        let res: &[u8] = b"SHIORI/2.6 200 OK\r\nSHIORI/3.0 200 OK\r\n\xff\xfe\x00\r\n\r\n";
        assert_eq!(version_of(res), Some(Version::V2));
        assert_eq!(version_of(b"X\r\nSHIORI/2.6 200 OK\r\n"), None);
    }

    #[test]
    fn version_of_reads_only_the_major_number() {
        for s in ["SHIORI/2", "SHIORI/2.", "SHIORI/2.300", "SHIORI/2.6200 OK"] {
            assert_eq!(version_of(s), Some(Version::V2), "{s:?}");
        }
        for s in ["SHIORI/3.0", "SHIORI/3.1 200 OK"] {
            assert_eq!(version_of(s), Some(Version::V3), "{s:?}");
        }
    }

    #[test]
    fn version_of_unknown_is_none() {
        let cases: &[&[u8]] = &[
            b"",
            b"SHIORI/",
            "SHIORI/２.６ 200 OK".as_bytes(),
            b"shiori/2.6 200 OK",
            b"\x82\xa0SHIORI/2.6",
            b" SHIORI/2.6 200 OK",
            b"HTTP/1.0 200 OK",
            b"SHIORI/1.0 200 OK",
            b"SHIORI/4.0 200 OK",
        ];
        for &c in cases {
            assert_eq!(version_of(c), None, "{c:?}");
        }
    }

    #[test]
    fn version_of_ignores_status() {
        assert_eq!(
            version_of("SHIORI/2.6 400 Bad Request\r\n\r\n"),
            Some(Version::V2)
        );
        // shiori4 の SHIORI の側が SHIORI/2.x の要求に返す答え
        assert_eq!(
            version_of("SHIORI/3.0 400 Bad Request\r\n\r\n"),
            Some(Version::V3)
        );
    }

    // ---- 応答の読み分け ----

    const KAWARI_VERSION: &str = "SHIORI/2.6 200 OK\r\nCharset: Shift_JIS\r\nCraftman: KawariDevelopmentTeam\r\nID: KAWARI.kdt\r\nVersion: 7.4.0\r\n\r\n";

    fn headers_of<'a>(r: &Response<'a>) -> Vec<(&'a str, &'a str)> {
        r.headers.iter().collect()
    }

    #[test]
    fn parse_reads_kawari_get_version() {
        let r = Response::parse(KAWARI_VERSION).unwrap();
        assert_eq!((r.version, r.status), (Version::V2, 200));
        assert_eq!(
            headers_of(&r),
            [
                ("Charset", "Shift_JIS"),
                ("Craftman", "KawariDevelopmentTeam"),
                ("ID", "KAWARI.kdt"),
                ("Version", "7.4.0"),
            ]
        );
    }

    #[test]
    fn parse_reads_lf_only_the_same() {
        let lf = KAWARI_VERSION.replace("\r\n", "\n");
        let a = Response::parse(KAWARI_VERSION).unwrap();
        let b = Response::parse(&lf).unwrap();
        assert_eq!((a.version, a.status), (b.version, b.status));
        assert_eq!(headers_of(&a), headers_of(&b));
    }

    #[test]
    fn parse_stops_at_blank_line() {
        for s in [
            "SHIORI/2.2 200 OK\r\nCharset: UTF-8\r\n\r\nSentence: x\r\n",
            "SHIORI/2.2 200 OK\nCharset: UTF-8\n\nSentence: x\n",
        ] {
            let r = Response::parse(s).unwrap();
            assert_eq!(headers_of(&r), [("Charset", "UTF-8")], "{s:?}");
        }
        // 空行が無ければ末尾まで
        let r = Response::parse("SHIORI/2.2 200 OK\r\nSentence: x").unwrap();
        assert_eq!(r.headers.get("Sentence"), Some("x"));
        let r = Response::parse("SHIORI/2.2 204").unwrap();
        assert_eq!(r.headers.iter().count(), 0);
    }

    #[test]
    fn parse_ignores_reason_phrase() {
        for s in [
            "SHIORI/2.6 200\r\n\r\n",
            "SHIORI/2.6 200 OK\r\n\r\n",
            "SHIORI/2.6 200 Whatever\r\n\r\n",
            "SHIORI/2.6 200 Not Found\r\n\r\n",
        ] {
            let r = Response::parse(s).unwrap();
            assert_eq!((r.version, r.status), (Version::V2, 200), "{s:?}");
        }
    }

    #[test]
    fn parse_returns_unknown_status_as_number() {
        let r = Response::parse("SHIORI/2.6 299 X\r\n\r\n").unwrap();
        assert_eq!(r.status, 299);
        let r = Response::parse("SHIORI/2.6 999\r\n\r\n").unwrap();
        assert_eq!(r.status, 999);
    }

    #[test]
    fn parse_rejects_unreadable_start_line() {
        for s in [
            "",
            "HTTP/1.0 200 OK\r\n\r\n",
            "SHIORI/2.6 OK\r\n\r\n",
            "SHIORI/x.y 200 OK\r\n\r\n",
            "SHIORI/2.6\r\n\r\n",
            "SHIORI/2.6 \r\n\r\n",
            "SHIORI/2.6 +200 OK\r\n\r\n",
            "SHIORI/2.6 -1 OK\r\n\r\n",
            "SHIORI/2.6 ２００ OK\r\n\r\n",
            "SHIORI/2.6 99999999999 OK\r\n\r\n",
            "SHIORI/1.0 200 OK\r\n\r\n",
            "SHIORI/4.0 200 OK\r\n\r\n",
            "X\r\nSHIORI/2.6 200 OK\r\n\r\n",
        ] {
            assert!(Response::parse(s).is_none(), "{s:?}");
        }
    }

    #[test]
    fn parse_reads_shiori3_as_v3() {
        for s in ["SHIORI/3.0 200 OK\r\n\r\n", "SHIORI/3.1 200 OK\r\n\r\n"] {
            assert_eq!(Response::parse(s).unwrap().version, Version::V3, "{s:?}");
        }
    }

    #[test]
    fn parse_311_312_match_reexported_constants() {
        let r =
            Response::parse("SHIORI/2.4 311 Not Enough\r\nSentence: \\h\\s[0]x\r\n\r\n").unwrap();
        assert_eq!(r.status, crate::SHIORI_311_NOT_ENOUGH);
        assert_eq!(r.headers.get("Sentence"), Some("\\h\\s[0]x"));
        let r = Response::parse("SHIORI/2.4 312 Advice\r\n\r\n").unwrap();
        assert_eq!(r.status, crate::SHIORI_312_ADVICE);
    }

    #[test]
    fn parse_version_agrees_with_version_of() {
        let lf = KAWARI_VERSION.replace("\r\n", "\n");
        let cases = [
            KAWARI_VERSION,
            &lf,
            "SHIORI/2.6 200 OK\r\n\r\n",
            "SHIORI/2.6 299 X\r\n\r\n",
            "SHIORI/2.6 400 Bad Request\r\n\r\n",
            "SHIORI/3.0 200 OK\r\n\r\n",
            "SHIORI/3.1 200 OK\r\n\r\n",
            "SHIORI/3.0 400 Bad Request\r\n\r\n",
            "SHIORI/2.4 311 Not Enough\r\n\r\n",
            "SHIORI/2.4 312 Advice\r\n\r\n",
            "",
            "HTTP/1.0 200 OK\r\n\r\n",
            "SHIORI/1.0 200 OK\r\n\r\n",
            "SHIORI/4.0 200 OK\r\n\r\n",
        ];
        for s in cases {
            let parsed = Response::parse(s).map(|r| r.version);
            assert!(parsed.is_none() || parsed == version_of(s), "{s:?}");
            // 版が読めない例は読み分けも失敗する
            if version_of(s).is_none() {
                assert!(parsed.is_none(), "{s:?}");
            }
        }
        // 例の集まりが両方の版を含み、読み分けが空振りしていない
        let got: Vec<_> = cases
            .iter()
            .filter_map(|s| Response::parse(s).map(|r| r.version))
            .collect();
        assert_eq!(got.len(), 10);
        assert!(got.contains(&Version::V2) && got.contains(&Version::V3));
    }

    // ---- 決まったヘッダの取り出し ----

    #[test]
    fn version_extractors_read_kawari_get_version() {
        let r = Response::parse(KAWARI_VERSION).unwrap();
        assert_eq!(r.id(), Some("KAWARI.kdt"));
        assert_eq!(r.craftman(), Some("KawariDevelopmentTeam"));
        assert_eq!(r.version_header(), Some("7.4.0"));
    }

    #[test]
    fn extractors_fall_back_to_value_only_where_allowed() {
        type Get = for<'a> fn(&Response<'a>) -> Option<&'a str>;
        let cases: [(&str, Get, bool); 7] = [
            ("Sentence", |r| r.sentence(), true),
            ("Word", |r| r.word(), true),
            ("String", |r| r.string(), true),
            ("Status", |r| r.status_header(), true),
            ("Version", |r| r.version_header(), true),
            ("ID", |r| r.id(), false),
            ("Craftman", |r| r.craftman(), false),
        ];
        for (name, get, falls_back) in cases {
            let fixed = format!("SHIORI/2.6 200 OK\r\n{name}: fixed\r\n\r\n");
            let value = "SHIORI/2.6 200 OK\r\nValue: value\r\n\r\n";
            let both = format!("SHIORI/2.6 200 OK\r\nValue: value\r\n{name}: fixed\r\n\r\n");
            let none = "SHIORI/2.6 200 OK\r\nCharset: UTF-8\r\n\r\n";
            assert_eq!(
                get(&Response::parse(&fixed).unwrap()),
                Some("fixed"),
                "{name}"
            );
            let expect = falls_back.then_some("value");
            assert_eq!(get(&Response::parse(value).unwrap()), expect, "{name}");
            assert_eq!(
                get(&Response::parse(&both).unwrap()),
                Some("fixed"),
                "{name}"
            );
            assert_eq!(get(&Response::parse(none).unwrap()), None, "{name}");
        }
    }

    #[test]
    fn extractors_return_raw_values() {
        let r = Response::parse(
            "SHIORI/2.6 200 OK\r\nStatus: neuron,neuronm\r\nSentence: \\h\\s[0]x\\e\r\n\r\n",
        )
        .unwrap();
        assert_eq!(r.status_header(), Some("neuron,neuronm"));
        assert_eq!(r.sentence(), Some("\\h\\s[0]x\\e"));
    }

    #[test]
    fn extractors_ignore_status() {
        let r =
            Response::parse("SHIORI/2.4 311 Not Enough\r\nSentence: \\h\\s[0]x\r\n\r\n").unwrap();
        assert_eq!(r.sentence(), Some("\\h\\s[0]x"));
        let r = Response::parse("SHIORI/2.4 312 Advice\r\nValue: \\h\\s[0]y\r\n\r\n").unwrap();
        assert_eq!(r.sentence(), Some("\\h\\s[0]y"));
    }
}
