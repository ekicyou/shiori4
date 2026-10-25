//! SHIORI/2.x の文字列の便利関数: 要求の組み立て、応答の読み分け、開始行の版の見分け（どれも状態を持たない）
//!
//! 文字コードの変換・Charset の方針・出来事の移し方・DLL の読み込みと HGLOBAL は持たない
//! （`LegacyRuntime` とホストが受け持つ）。

// ============================================================
// 要求の組み立て
// ============================================================

use shiori4::protocol::HeaderWriter;

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

// ============================================================
// 応答の読み分け
// ============================================================

// ============================================================
// 版の見分け
// ============================================================

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
}
