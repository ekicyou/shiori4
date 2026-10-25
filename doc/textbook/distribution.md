# 配布の決まり

配布されたゴーストから、ホストの OS と arch に合う SHIORI のファイルを、DLL を読まずに選ぶ手順を 1 つの章にまとめる。前半はベースウェアの作者に向けた手順と Rust の例、後半は SHIORI の作者に向けた「どの名前でファイルを置くか」である。教科書の目次は [doc/textbook/README.md](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/README.md) にある。

## この章の読み方

- 規約の正本は C ヘッダ [`shiori4_abi/include/shiori4_abi.h`](https://github.com/ekicyou/shiori4/blob/main/shiori4_abi/include/shiori4_abi.h) の冒頭の注釈と、`shiori4_abi` のクレート冒頭の `//!` である。この章は正本を言い直したもので、食い違ったら正本が正しい。
- この章の引用（`>` で始まる行）は、正本の「配布の決まり（DLL の選び方）」の節を一字一句そのまま写したものである。写しが正本と一致することは CI が確かめる（`shiori4_abi/tests/textbook.rs`）。
- 「配布の決まり 3」「DLL の口と発見の順 2」などの引き方は、[規約の一覧](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/rules.md)の「番号の付け方と引用の仕方」に従う。各規約の理由も規約の一覧にある。
- Rust の例は `shiori4_host` の公開 API（`shiori_file_candidates`・`machine_matches`・`TargetOs`・`TargetArch`）を使う。API の詳しくは [docs.rs の `shiori4_host`](https://docs.rs/shiori4_host) にある。

## 配布の決まり（DLL の選び方）

> 1. descript.txt の shiori,<名前> から名前の幹を取る（例 shiori,pasta.dll → pasta）
> 2. ホストは、ホストの OS と arch ごとに次の順で探す
>    - Windows x64: <幹>.x64.dll → <幹>.dll
>    - Windows arm64: <幹>.arm64.dll →（x64 のプロセスを起こせるホストだけ）<幹>.x64.dll → <幹>.dll
>    - Linux x64: lib<幹>.x64.so → lib<幹>.so
>    - Linux arm64: lib<幹>.arm64.so → lib<幹>.so
>    - macOS: lib<幹>.dylib（ユニバーサルバイナリを推奨）
> 3. 接尾辞は x64・arm64 の 2 つだけ。x86 は作らない。接尾辞なしの Windows の DLL が、
>    32bit の SHIORI/3.0 を担う
> 4. Linux と macOS は lib の付いた名前だけを探す。macOS は arch の接尾辞を持たない
> 5. ホストは選んだファイルの機械の種類をヘッダ（PE・ELF・Mach-O）で確かめ、合わなければ次の候補へ
>    進み、候補が尽きたらそのゴーストを読めなかったものとして扱う。これは SHIORI/3.0 と v4 の両方に使える

## ホストの手順

ホストは、次の順に進めてファイルを 1 つ選ぶ。どの段も DLL を読まない。

1. descript.txt の `shiori,<名前>` から名前の幹を取る（配布の決まり 1）。`shiori,pasta.dll` なら幹は `pasta` である。
2. ホストの OS と arch を決める。arch は `x64` と `arm64` の 2 つだけである（配布の決まり 3）。
3. OS と arch ごとの順で、候補のファイル名を並べる（配布の決まり 2）。Windows arm64 で `<幹>.x64.dll` を候補に入れるのは、x64 のプロセスを起こせるホストだけである。Linux と macOS の候補は、どれも `lib` の付いた名前である（配布の決まり 4）。
4. 候補を前から順に見て、選んだファイルの機械の種類をヘッダ（PE・ELF・Mach-O）で確かめる。合わなければ次の候補へ進む（配布の決まり 5）。
5. 合ったファイルを読む。候補が尽きたら、そのゴーストを読めなかったものとして扱う（配布の決まり 5）。

## Rust で行う例

### 今のプロセスの OS と arch（手順 2）

`TargetOs::CURRENT`・`TargetArch::CURRENT` は、今のプロセスの OS と arch である。表に無い OS や、x86 など表に無い arch では `None` になる。

```rust
use shiori4_host::{TargetArch, TargetOs};

match (TargetOs::CURRENT, TargetArch::CURRENT) {
    (Some(os), Some(arch)) => println!("{os:?} {arch:?} の候補を探す"),
    _ => println!("配布の決まりの表に無い OS か arch"),
}
```

### 候補を並べる（手順 1・3）

`shiori_file_candidates` は、descript.txt の `shiori,` の値（または名前の幹）から、探す順にファイル名を並べる。名前の部分の末尾の `.dll`（ASCII の大文字小文字を区別しない）を 1 つ落とした残りを幹とする。ファイルにもライブラリにも触れない。

```rust
use shiori4_host::{shiori_file_candidates, TargetArch, TargetOs};

// Windows x64
assert_eq!(
    shiori_file_candidates("pasta.dll", TargetOs::Windows, TargetArch::X64, false),
    ["pasta.x64.dll", "pasta.dll"],
);
// Windows arm64: 4 つ目の引数（x64 のプロセスを起こせるか）で <幹>.x64.dll が入るかが決まる
assert_eq!(
    shiori_file_candidates("pasta.dll", TargetOs::Windows, TargetArch::Arm64, false),
    ["pasta.arm64.dll", "pasta.dll"],
);
assert_eq!(
    shiori_file_candidates("pasta.dll", TargetOs::Windows, TargetArch::Arm64, true),
    ["pasta.arm64.dll", "pasta.x64.dll", "pasta.dll"],
);
// Linux は lib の付いた名前だけ
assert_eq!(
    shiori_file_candidates("pasta.dll", TargetOs::Linux, TargetArch::X64, false),
    ["libpasta.x64.so", "libpasta.so"],
);
assert_eq!(
    shiori_file_candidates("pasta.dll", TargetOs::Linux, TargetArch::Arm64, false),
    ["libpasta.arm64.so", "libpasta.so"],
);
// macOS は arch の接尾辞を持たない
assert_eq!(
    shiori_file_candidates("pasta.dll", TargetOs::MacOs, TargetArch::Arm64, false),
    ["libpasta.dylib"],
);
// 値にディレクトリの部分があれば、そのまま前に付く
assert_eq!(
    shiori_file_candidates("bin/pasta.dll", TargetOs::Linux, TargetArch::X64, false),
    ["bin/libpasta.x64.so", "bin/libpasta.so"],
);
```

4 つ目の引数 `can_spawn_x64` は Windows arm64 でだけ効き、macOS では arch を見ない。

### 機械の種類を確かめる（手順 4）

`machine_matches` は、ファイルの先頭のヘッダ（PE・ELF・Mach-O の単体とユニバーサル）の機械の種類が `arch` と合うかを返す。ライブラリとして読み込まず、初期化コードも動かさない。形式が違う・壊れている・短いファイルは `Ok(false)`、開けない・読めないファイルは `Err` になる。macOS のユニバーサルバイナリは、中に `arch` が含まれていれば合う。

次の例は、今の実行ファイルを確かめる。今のプロセスの arch には合い、もう一方の arch には合わない。

```rust
use shiori4_host::{machine_matches, TargetArch};

let exe = std::env::current_exe().unwrap();
if let Some(arch) = TargetArch::CURRENT {
    assert!(machine_matches(&exe, arch).unwrap());
    let other = match arch {
        TargetArch::X64 => TargetArch::Arm64,
        TargetArch::Arm64 => TargetArch::X64,
    };
    assert!(!machine_matches(&exe, other).unwrap());
}
```

`machine_matches` の doc は、ヘッダの表すとおりに扱う 2 つの例外を挙げている。ARM64EC の DLL は Machine が x64 なので `X64` に合うが、x64 の実機では読めない。ARM64X の DLL は Machine が arm64 なので、ARM の Windows で x64 として動くプロセスが `X64` で確かめると合わないと出るが、実際には読める。

### 候補から 1 つ選ぶ（手順 3〜5）

候補を順に見て、在って機械の種類が合う最初のファイルを選ぶ。この例では、開けない（無い）ファイルも、合わないファイルと同じく次の候補へ進む。`None` は候補が尽きたことで、そのゴーストを読めなかったものとして扱う。

次の例は、一時ディレクトリの 1 つ目の候補に合わないファイル（ただの文字列）を、最後の候補に今の実行ファイルの写しを置いて、最後の候補が選ばれることを確かめる。

```rust
use shiori4_host::{machine_matches, shiori_file_candidates, TargetArch, TargetOs};
use std::path::{Path, PathBuf};

/// 在って機械の種類が合う最初の候補。候補が尽きたら None（読めなかったゴースト）
fn select_shiori(dir: &Path, shiori: &str, os: TargetOs, arch: TargetArch, can_spawn_x64: bool) -> Option<PathBuf> {
    shiori_file_candidates(shiori, os, arch, can_spawn_x64)
        .into_iter()
        .map(|name| dir.join(name))
        .find(|p| machine_matches(p, arch).unwrap_or(false))
}

let (Some(os), Some(arch)) = (TargetOs::CURRENT, TargetArch::CURRENT) else { return };
let dir = std::env::temp_dir().join(format!("shiori4_distribution_{}", std::process::id()));
std::fs::create_dir_all(&dir).unwrap();

// まだ何も無い: 候補が尽きる
assert_eq!(select_shiori(&dir, "pasta.dll", os, arch, false), None);

let names = shiori_file_candidates("pasta.dll", os, arch, false);
std::fs::write(dir.join(&names[0]), "not a library").unwrap();
let last = dir.join(names.last().unwrap());
std::fs::copy(std::env::current_exe().unwrap(), &last).unwrap();
assert_eq!(select_shiori(&dir, "pasta.dll", os, arch, false), Some(last));

std::fs::remove_dir_all(&dir).unwrap();
```

macOS では候補が 1 つだけなので、1 つ目の候補に書いた文字列を今の実行ファイルの写しで上書きして、その候補が選ばれる。

### 選んだファイルを読む（手順 5）

選んだファイルを `load_ghost_with` で読む。`load_ghost_with` は機械の種類を確かめないので、`machine_matches` で確かめてから渡す。読み込みの口（v4・v3 互換モード・旧来の SHIORI/3.0）は、選んだ後に DLL の口と発見の順で決まる（DLL の口と発見の順 2）。

この例はゴーストの DLL を要するので、ビルドだけを検査する（実行しない）。

```rust,no_run
use shiori4_host::*;
# struct MyHost;
# impl Host for MyHost { fn name(&self) -> &str { "MyBase" } fn on_post(&self, _post: Post) {} }

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let dir = std::env::current_exe()?.parent().expect("exe dir").join("ghost").join("master");
let (os, arch) = (TargetOs::CURRENT.expect("os"), TargetArch::CURRENT.expect("arch"));
let Some(path) = shiori_file_candidates("pasta.dll", os, arch, false)
    .into_iter()
    .map(|name| dir.join(name))
    .find(|p| machine_matches(p, arch).unwrap_or(false))
else {
    // 候補が尽きた: 読めなかったゴースト（配布の決まり 5）
    return Err("no shiori".into());
};
match load_ghost_with(&path, &dir, MyHost, &[EntryKind::V4, EntryKind::V3])? {
    LoadedGhost::V4(mut rt) => { rt.request("ID: OnBoot\r\n").map_err(|s| format!("ABI error: {s}"))?; }
    LoadedGhost::V3(mut rt) => {
        let res = rt.request("GET SHIORI/3.0\r\nID: OnBoot\r\n\r\n").map_err(|s| format!("ABI error: {s}"))?;
        # drop(res);
    }
    // x86 の Windows では LegacyRuntime::from_library で読める（この例では許可していない）
    LoadedGhost::Legacy { entry, .. } => println!("従来の口 {entry:?} は許可していない"),
}
# Ok(())
# }
```

## SHIORI の作者へ: どの名前で置くか

descript.txt には、どの OS でも `shiori,pasta.dll` のように 1 つの名前を書く。ホストはその幹 `pasta` から、OS と arch ごとのファイル名を探す（配布の決まり 1・2）。SHIORI の作者は、作ったファイルを幹に合わせた名前で置く。

Rust の cdylib は、既定でパッケージ名（`-` は `_` になる）から名前を作る。たとえばパッケージ名 `my-ghost` の cdylib は、Windows で `my_ghost.dll`、Linux で `libmy_ghost.so`、macOS で `libmy_ghost.dylib` になる。cargo はこの名前に `.x64` などの接尾辞を付けないので、置くときに幹と接尾辞に合わせて付け替える。

次の表は、パッケージ名 `my-ghost` の cdylib を、幹 `pasta`（descript.txt は `shiori,pasta.dll`）のゴーストに置くときの対応である。

| ホストの OS・arch | ビルドの target の例 | cdylib が既定で作る名前 | 置く名前 |
|-------------------|----------------------|-------------------------|----------|
| Windows x64 | `x86_64-pc-windows-msvc` | `my_ghost.dll` | `pasta.x64.dll` |
| Windows arm64 | `aarch64-pc-windows-msvc` | `my_ghost.dll` | `pasta.arm64.dll` |
| Linux x64 | `x86_64-unknown-linux-gnu` | `libmy_ghost.so` | `libpasta.x64.so` |
| Linux arm64 | `aarch64-unknown-linux-gnu` | `libmy_ghost.so` | `libpasta.arm64.so` |
| macOS | `x86_64-apple-darwin`・`aarch64-apple-darwin` | `libmy_ghost.dylib` | `libpasta.dylib`（ユニバーサルバイナリを推奨） |

- 接尾辞は `x64`・`arm64` の 2 つだけで、x86 は作らない（配布の決まり 3）。
- 接尾辞なしの Windows の DLL（`pasta.dll`）は、32bit の SHIORI/3.0 を担う（配布の決まり 3）。Windows の x64 と arm64 のファイルには接尾辞を付けて、この名前と分ける。
- Linux の `libpasta.so` は、x64 と arm64 のどちらのホストも最後の候補として探す（配布の決まり 2）。ホストは機械の種類をヘッダで確かめるので（配布の決まり 5）、合わない arch のホストは次の候補へ進み、候補が尽きて読めなかったものとして扱う。両方の arch を配るときは、同じ名前に 2 つは置けないので接尾辞を付ける。
- Linux と macOS では、`lib` の付いた名前だけが探される（配布の決まり 4）。`pasta.so` のように `lib` の無い名前は見つからない。
- macOS は arch の接尾辞を持たない（配布の決まり 4）。x64 と arm64 の両方を 1 つの `libpasta.dylib` に入れたユニバーサルバイナリを推奨する（配布の決まり 2）。`machine_matches` は、ユニバーサルバイナリの中にホストの arch が含まれていれば合うと判定する。

## SHIORI/3.0 と v4 の両方に使える

この手順は、SHIORI/3.0 と v4 の両方のファイルに使える（配布の決まり 5）。選んだファイルが v4・v3 互換モード・旧来の SHIORI/3.0 のどの口で読まれるかは、選んだ後に DLL の口と発見の順で決まる（DLL の口と発見の順 2）。1 つの DLL が、SHIORI/3.0 の書き出しと `shiori_get_api` の両方を持ってもよい（DLL の口と発見の順 1）。

旧来の DLL を読む口は、x86 の Windows でだけ `shiori4_host` の `LegacyRuntime` にある（入口は `load_ghost_with` → `LoadedGhost::Legacy` → `LegacyRuntime::from_library`）。ほかの環境では現れない。従来の口（`loadu`・`load`）で解決したときは、`load_ghost_with` は開いたライブラリを `LoadedGhost::Legacy` で返すだけで `loadu`・`load` を呼ばず、x86 の Windows では `LegacyRuntime::from_library` がその口で初期化する。SHIORI/3.0 と v4 の違いは[移行の手引き](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/migration.md)にある。
