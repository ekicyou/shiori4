//! 正本の一致の検査（教科書の検査）
//!
//! 規約の正本は C ヘッダ（`include/shiori4_abi.h`）の先頭の注釈と、`src/lib.rs` の `//!` の 2 つ。
//! 両方の 9 つの節が一字一句一致すること、各節の最上位の項目が 1 から連番であることを確かめる。
//! std だけを使い、リポジトリの中だけで走る（`Cargo.toml` の `exclude` でパッケージから外す）。

const HEADER: &str = include_str!("../include/shiori4_abi.h");
const CRATE_SRC: &str = include_str!("../src/lib.rs");

/// 規約の 9 つの節の見出し（正本の順）。上流が節を足したら、一覧に無い見出しとして落ちる
const SECTIONS: [&str; 9] = [
    "版（abi_version）",
    "ABI 規約",
    "戻り値（ShioriStatus）",
    "スレッド規約",
    "load の規約",
    "ホストのプロセス",
    "DLL の口と発見の順",
    "post の完了通知（v4）",
    "配布の決まり（DLL の選び方）",
];

// ==== 抽出と正規化（教科書の章の検査でも使う）

/// CRLF を LF に、各行の前後の空白を除き、空の行を除く
fn normalize(text: &str) -> Vec<String> {
    text.replace("\r\n", "\n")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

/// 規約の比べ方: 正規化に加え、C ヘッダの `host->` を `//!` の `host.` に揃える
fn normalize_rules(lines: &[String]) -> Vec<String> {
    normalize(&lines.join("\n"))
        .into_iter()
        .map(|l| l.replace("host->", "host."))
        .collect()
}

/// C ヘッダの最初の `/*` から `*/` までの行から、先頭の ` *`（と続く空白 1 つ）を除いたもの
fn header_comment(src: &str) -> Vec<String> {
    let src = src.replace("\r\n", "\n");
    let start = src.find("/*").expect("C ヘッダに /* が無い") + 2;
    let end = start + src[start..].find("*/").expect("C ヘッダに */ が無い");
    src[start..end]
        .lines()
        .map(|l| {
            let l = l.strip_prefix(" *").unwrap_or(l);
            l.strip_prefix(' ').unwrap_or(l).to_owned()
        })
        .collect()
}

/// `//!` の行から、行頭の `//!`（と続く空白 1 つ）を除いたもの。節の見出しの `# ` も除く
fn crate_doc(src: &str) -> Vec<String> {
    src.replace("\r\n", "\n")
        .lines()
        .filter_map(|l| l.strip_prefix("//!"))
        .map(|l| {
            let l = l.strip_prefix(' ').unwrap_or(l);
            l.strip_prefix("# ").unwrap_or(l).to_owned()
        })
        .collect()
}

/// 行頭が `<数字>. ` なら、その数
fn item_number(line: &str) -> Option<u32> {
    let (num, _) = line.split_once(". ")?;
    if num.is_empty() || !num.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    num.parse().ok()
}

/// 最初の正本の見出しから先を節に切り分ける（見出しより前の行は捨てる）。
/// 節の中で字下げの無い行は、番号の付いた項目か、次の節の見出しでなければならない
fn split_sections(lines: &[String]) -> Result<Vec<(String, Vec<String>)>, String> {
    let mut sections: Vec<(String, Vec<String>)> = Vec::new();
    for line in lines {
        let top_level = !line.is_empty() && !line.starts_with(char::is_whitespace);
        if sections.is_empty() {
            if SECTIONS.contains(&line.as_str()) {
                sections.push((line.clone(), Vec::new()));
            }
            continue;
        }
        if top_level && item_number(line).is_none() {
            if !SECTIONS.contains(&line.as_str()) {
                return Err(format!(
                    "正本の一覧に無い見出し、または番号の無い最上位の行: {line:?}"
                ));
            }
            sections.push((line.clone(), Vec::new()));
            continue;
        }
        sections.last_mut().unwrap().1.push(line.clone());
    }
    Ok(sections)
}

/// 節の見出しが、正本の 9 つの節をこの順ですべて持つことを確かめる
fn assert_all_sections(sections: &[(String, Vec<String>)], what: &str) {
    let names: Vec<&str> = sections.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, SECTIONS, "{what} の節の並びが正本の 9 つの節と違う");
}

fn header_sections() -> Vec<(String, Vec<String>)> {
    let s = split_sections(&header_comment(HEADER)).expect("C ヘッダの注釈");
    assert_all_sections(&s, "C ヘッダ");
    s
}

fn crate_doc_sections() -> Vec<(String, Vec<String>)> {
    let s = split_sections(&crate_doc(CRATE_SRC)).expect("lib.rs の //!");
    assert_all_sections(&s, "lib.rs の //!");
    s
}

// ==== 抽出と正規化の自己テスト（テストが黙って通らないため）

#[test]
fn extraction_and_normalization_on_handwritten_input() {
    let header = "/*\r\n * 題\r\n *\r\n * ABI 規約\r\n * 1. host->request を呼ぶ  \r\n *    続きの行\r\n *\r\n * load の規約\r\n * 1. 一つ目\r\n * 2. 二つ目\r\n */\r\n#define X 1\r\n";
    let doc = "//! 題\n//!\n//! # ABI 規約\n//! 1. host.request を呼ぶ\n//!    続きの行\n//!\n//! # load の規約\n//! 1. 一つ目\n//! 2. 二つ目\nfn f() {}\n";

    assert_eq!(
        normalize(" a \r\n\r\n  b\r\n"),
        ["a", "b"],
        "CRLF・前後の空白・空の行"
    );

    let h = split_sections(&header_comment(header)).unwrap();
    let d = split_sections(&crate_doc(doc)).unwrap();
    let names: Vec<&str> = h.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["ABI 規約", "load の規約"]);
    assert_eq!(h[0].1[1], "   続きの行", "続きの行の字下げを残す");
    assert_eq!(
        normalize_rules(&h[0].1),
        ["1. host.request を呼ぶ", "続きの行"]
    );
    assert_eq!(normalize_rules(&h[1].1), ["1. 一つ目", "2. 二つ目"]);
    for ((hn, hl), (dn, dl)) in h.iter().zip(&d) {
        assert_eq!(hn, dn);
        assert_eq!(normalize_rules(hl), normalize_rules(dl));
    }
    assert_eq!(item_number("12. x"), Some(12));
    assert_eq!(item_number("- x"), None);
    assert_eq!(item_number("1.5 x"), None);

    // 一覧に無い見出し・番号の無い最上位の行は落とす
    let bad = "//! # ABI 規約\n//! 1. a\n//! # 新しい節\n".to_owned();
    assert!(split_sections(&crate_doc(&bad)).is_err());
    let bad = "/*\n * ABI 規約\n * - a\n */".to_owned();
    assert!(split_sections(&header_comment(&bad)).is_err());
}

// ==== 正本の検査

#[test]
fn header_and_crate_doc_rules_agree() {
    for ((name, h), (_, d)) in header_sections().iter().zip(&crate_doc_sections()) {
        let (h, d) = (normalize_rules(h), normalize_rules(d));
        for (i, (hl, dl)) in h.iter().zip(&d).enumerate() {
            assert_eq!(
                hl,
                dl,
                "「{name}」の {} 行目が C ヘッダと //! で違う（両方を直す）",
                i + 1
            );
        }
        assert_eq!(
            h.len(),
            d.len(),
            "「{name}」の行の数が C ヘッダと //! で違う（両方を直す）"
        );
    }
}

#[test]
fn rule_numbers_are_sequential() {
    for (what, sections) in [
        ("C ヘッダ", header_sections()),
        ("lib.rs の //!", crate_doc_sections()),
    ] {
        for (name, lines) in &sections {
            let numbers: Vec<u32> = lines.iter().filter_map(|l| item_number(l)).collect();
            let expected: Vec<u32> = (1..=numbers.len() as u32).collect();
            assert!(
                !numbers.is_empty(),
                "{what} の「{name}」に番号の付いた項目が無い"
            );
            assert_eq!(
                numbers, expected,
                "{what} の「{name}」の番号が 1 からの連番でない"
            );
        }
    }
}
