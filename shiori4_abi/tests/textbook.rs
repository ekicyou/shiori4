//! 正本の一致の検査（教科書の検査）
//!
//! 規約の正本は C ヘッダ（`include/shiori4_abi.h`）の先頭の注釈と、`src/lib.rs` の `//!` の 2 つ。
//! 両方の 9 つの節が一字一句一致すること、各節の最上位の項目が 1 から連番であることを確かめる。
//! 教科書（`doc/textbook/`）とルートの README に写した規約・C の断片・語の表が正本と一致することも確かめる。
//! std だけを使い、リポジトリの中だけで走る（`Cargo.toml` の `exclude` でパッケージから外す）。

const HEADER: &str = include_str!("../include/shiori4_abi.h");
const CRATE_SRC: &str = include_str!("../src/lib.rs");
const RULES_MD: &str = include_str!("../../doc/textbook/rules.md");
const DISTRIBUTION_MD: &str = include_str!("../../doc/textbook/distribution.md");
const VOCABULARY_MD: &str = include_str!("../../doc/textbook/vocabulary.md");
const C_MD: &str = include_str!("../../doc/textbook/c.md");
const HOST_PROTOCOL_MD: &str = include_str!("../../doc/host-protocol.md");
const README_MD: &str = include_str!("../../README.md");
/// C の例のファイル（名前と中身）
const C_FILES: [(&str, &str); 3] = [
    (
        "minimal_shiori.c",
        include_str!("../../doc/textbook/c/minimal_shiori.c"),
    ),
    (
        "minimal_host.c",
        include_str!("../../doc/textbook/c/minimal_host.c"),
    ),
    (
        "versioned_shiori.c",
        include_str!("../../doc/textbook/c/versioned_shiori.c"),
    ),
];

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

// ==== 教科書の章からの抽出（章ごとの区切りを持つ。`split_sections` は正本の注釈だけに使う）

/// Markdown の `## <見出し>` ごとに、引用の行（`>` で始まる行）から `>` を除いたものを集める。
/// 最初の `## ` より前の引用は、見出しを空の文字列として集める
fn quotes_by_section(md: &str) -> Vec<(String, Vec<String>)> {
    let mut out = vec![(String::new(), Vec::new())];
    for line in md.replace("\r\n", "\n").lines() {
        if let Some(name) = line.strip_prefix("## ") {
            out.push((name.trim().to_owned(), Vec::new()));
        } else if let Some(q) = line.strip_prefix('>') {
            out.last_mut().unwrap().1.push(q.to_owned());
        }
    }
    out
}

/// Markdown の ```` ```<lang> ```` のフェンスの塊の中身を、出てくる順に集める（言語の書き方は完全一致）
fn fenced_blocks(md: &str, lang: &str) -> Vec<String> {
    let mut out = Vec::new();
    // フェンスの中なら Some。中身は、言語が合う塊なら Some(行の並び)、他の言語なら None
    let mut cur: Option<Option<Vec<&str>>> = None;
    let md = md.replace("\r\n", "\n");
    for line in md.lines() {
        let fence = line.trim_start().strip_prefix("```");
        match (&mut cur, fence) {
            (None, Some(info)) => cur = Some((info.trim() == lang).then(Vec::new)),
            (Some(b), Some(_)) => {
                if let Some(b) = b {
                    out.push(b.join("\n"));
                }
                cur = None;
            }
            (Some(Some(b)), None) => b.push(line),
            _ => {}
        }
    }
    assert!(cur.is_none(), "閉じていないフェンスがある");
    out
}

/// `needle` が `hay` の中に、連続した行の並びとしてそのままあるか（空の `needle` は無いものとする）
fn contains_run(hay: &[String], needle: &[String]) -> bool {
    !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle)
}

/// Markdown の表の行（区切りの行を除く）の、先頭の 3 つのセル
fn table_rows(md: &str) -> Vec<[String; 3]> {
    md.replace("\r\n", "\n")
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with('|') && !l.chars().all(|c| "|-: ".contains(c)))
        .filter_map(|l| {
            let mut cells = l.trim_matches('|').split('|').map(|c| c.trim().to_owned());
            Some([cells.next()?, cells.next()?, cells.next()?])
        })
        .collect()
}

/// `host-protocol.md` の第 8 章（`## 8.` の行から、次の `## ` の行の前まで）
fn chapter8(md: &str) -> String {
    let md = md.replace("\r\n", "\n");
    let start = md
        .find("\n## 8.")
        .expect("host-protocol.md に第 8 章が無い")
        + 1;
    let rest = &md[start..];
    let end = rest.find("\n## ").map_or(rest.len(), |i| i + 1);
    rest[..end].to_owned()
}

// ==== 抽出と正規化の自己テスト（テストが黙って通らないため）

#[test]
fn chapter_extraction_on_handwritten_input() {
    let md = "# 題\r\n> 前の引用\r\n## 読み方\r\n本文\r\n## ABI 規約\r\n> 1. host->request\r\n>    続き\r\n### 小見出し\r\n> 2. 二つ目\r\n## 後\r\n";
    let q = quotes_by_section(md);
    let names: Vec<&str> = q.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        ["", "読み方", "ABI 規約", "後"],
        "`###` では切らない"
    );
    assert_eq!(q[0].1, [" 前の引用"]);
    assert!(q[1].1.is_empty());
    assert_eq!(
        normalize_rules(&q[2].1),
        ["1. host.request", "続き", "2. 二つ目"]
    );

    let md = "```rust\nfn f() {}\n```\n```c\nint a;\n\nint b;\n```\n  ```text\nint z;\n  ```\n```c,ignore\nint y;\n```\n```c\r\nint c;\r\n```\r\n";
    assert_eq!(
        fenced_blocks(md, "c"),
        ["int a;\n\nint b;", "int c;"],
        "他の言語の塊を拾わない"
    );

    let hay = normalize("a\nb\nc\nd\n");
    assert!(contains_run(&hay, &normalize("b\n\n  c")));
    assert!(!contains_run(&hay, &normalize("b\nd")), "連続していない");
    assert!(!contains_run(&hay, &normalize("b\ncx")), "1 字の違い");
    assert!(!contains_run(&hay, &[]), "空の塊");

    let doc = "## 7. 前\n| x | y | z |\n## 8. 語\n| 種類 | 語 | 定数名 |\n|------|----|:---:|\n| ID | `a` | `A` |\n|ID|`b`|`B`| 節 |\n| 短い | 行 |\n## 付録\n| p | q | r |\n";
    let rows = table_rows(&chapter8(doc));
    assert_eq!(
        rows,
        [
            ["種類", "語", "定数名"].map(String::from),
            ["ID", "`a`", "`A`"].map(String::from),
            ["ID", "`b`", "`B`"].map(String::from),
        ],
        "第 8 章の表の行だけ・区切りの行と 3 つに満たない行は除く"
    );
}

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

// ==== 教科書の写しの検査

/// 行の並びが一致することを、最初に違う行を示して確かめる
fn assert_lines_eq(got: &[String], want: &[String], what: &str) {
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        assert_eq!(g, w, "{what} の {} 行目が正本と違う", i + 1);
    }
    assert_eq!(got.len(), want.len(), "{what} の行の数が正本と違う");
}

/// 正本（C ヘッダ）の節の、規約の比べ方で正規化した行
fn canon_section(name: &str) -> Vec<String> {
    let sections = header_sections();
    let (_, lines) = sections.iter().find(|(n, _)| n == name).unwrap();
    normalize_rules(lines)
}

#[test]
fn rules_catalog_copies_every_rule() {
    let quotes = quotes_by_section(RULES_MD);
    for (name, lines) in &quotes {
        assert!(
            SECTIONS.contains(&name.as_str()) || lines.is_empty(),
            "rules.md の「{name}」は正本の節でないのに引用の行がある"
        );
    }
    let canon: Vec<&(String, Vec<String>)> = quotes
        .iter()
        .filter(|(n, _)| SECTIONS.contains(&n.as_str()))
        .collect();
    let names: Vec<&str> = canon.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names, SECTIONS,
        "rules.md の節の並びが正本の 9 つの節と違う"
    );
    for (name, lines) in canon {
        assert_lines_eq(
            &normalize_rules(lines),
            &canon_section(name),
            &format!("rules.md の「{name}」の引用"),
        );
    }
}

#[test]
fn distribution_chapter_quotes_canon() {
    let quotes: Vec<String> = quotes_by_section(DISTRIBUTION_MD)
        .into_iter()
        .flat_map(|(_, lines)| lines)
        .collect();
    assert_lines_eq(
        &normalize_rules(&quotes),
        &canon_section("配布の決まり（DLL の選び方）"),
        "distribution.md の引用",
    );
}

#[test]
fn c_blocks_are_excerpts() {
    let sources: Vec<(&str, Vec<String>)> = C_FILES
        .iter()
        .map(|(n, t)| (*n, normalize(t)))
        .chain([("shiori4_abi.h", normalize(HEADER))])
        .collect();

    // README の C の塊 2 つは、最小の例の全文
    let readme = fenced_blocks(README_MD, "c");
    assert_eq!(readme.len(), 2, "README の ```c の塊は 2 つ");
    for (block, file) in readme.iter().zip(["minimal_shiori.c", "minimal_host.c"]) {
        let (_, want) = sources.iter().find(|(n, _)| *n == file).unwrap();
        assert_lines_eq(&normalize(block), want, &format!("README の {file}"));
    }

    // c.md の C の塊は、C の例のファイルか C ヘッダの連続した行の並び
    let blocks = fenced_blocks(C_MD, "c");
    assert!(!blocks.is_empty(), "c.md に ```c の塊が無い");
    for (i, block) in blocks.iter().enumerate() {
        let block = normalize(block);
        assert!(
            sources.iter().any(|(_, s)| contains_run(s, &block)),
            "c.md の {} 番目の ```c の塊が、C の例のファイルにも C ヘッダにもそのまま無い: {:?}",
            i + 1,
            block.first()
        );
    }
}

#[test]
fn vocabulary_table_keeps_chapter8() {
    let rows = table_rows(&chapter8(HOST_PROTOCOL_MD));
    let (head, want) = rows.split_first().expect("第 8 章に表が無い");
    assert_eq!(head, &["種類", "語", "定数名"], "第 8 章の表の見出しの行");
    assert!(!want.is_empty(), "第 8 章の表に語の行が無い");
    let have = table_rows(VOCABULARY_MD);
    for row in want {
        assert!(
            have.contains(row),
            "vocabulary.md の語の表に、第 8 章の行が綴りのまま無い: {row:?}"
        );
    }
}
