# Technology Stack

## Architecture

3 層のクレート構成。下の層は上の層を知らない。

```
shiori4_abi   … C ABI（関数テーブル・ShioriBytes・版・規約）。ペイロードは解釈しない
   ↑
shiori4       … SHIORI 側の高水準層（芯: Ghost, Adapter, v3 変換。便利関数: protocol（Headers, Properties, PendingTags, 語の定数と組み立て・読み分け））
   ↑
shiori4_host  … ホスト側の高水準層（Host, GhostRuntime, V3Runtime, 読み込みの口 load_ghost, post キュー, pump, 従来の SHIORI/3.0 の口 LegacyRuntime（x86 の Windows だけ））
```

- ホストと SHIORI が共有する語（`ID: getproperty`, `Command: SEND`, `Tag`, `OnHostComplete` など）は `doc/host-protocol.md`（ホスト側プロトコル仕様）が定める。その定数と組み立て・読み分けは便利関数 `shiori4::protocol` に置き、ホスト側は `shiori4_host::protocol` として再エクスポートして使う
- 各層は「ABI 層の trait（`Shiori` / `ShioriHost`）」と「高水準 trait（`Ghost` / `Host`）」の2段。実装者は通常、高水準 trait だけを実装する

## Core Technologies

- **Language**: Rust 1.85 以上、edition 2024（`[workspace.package]` で一括指定）
- **FFI**: `extern "C"` + `#[repr(C)]`。C ヘッダ `shiori4_abi/include/shiori4_abi.h` は Rust 側と同一レイアウトを手で保つ
- **動的読み込み**: `libloading`（`shiori4_host` のみ）

## Key Libraries

外部依存は最小に保つ。`shiori4_abi` と `shiori4` は依存ゼロ（std のみ）、`shiori4_host` も `libloading` だけ（x86 の Windows の従来の口が使う `GlobalAlloc` などは kernel32 を直接宣言し、依存を足さない）。

## Development Standards

### ABI 規約（破ると互換が壊れる）
- DLL 境界を越えるのは `#[repr(C)]` 構造体と `extern "C"` 関数ポインタだけ
- ペイロードは UTF-8 の (ptr, len)。NUL 終端にしない。引数は呼び出し中だけの借用
- 所有権が移るのは `out` の `ShioriBytes` だけ。`out` は常にホストが作り、ホストの `release` で解放する（SHIORI は応答を `host.copy_from_slice` で写す。v3 互換モードにも `copy_from_slice` だけのテーブルを渡す）
- 構造体は末尾にだけ追加する。追加する関数ポインタは `Option<unsafe extern "C" fn(..)>`。受け手は `struct_size` で判定する
- 戻り値: 0 以上は SHIORI/3.0 のステータスコード、負は ABI エラー（`SHIORI_E_*`）
- エクスポートする関数は、例外・パニックを境界の外へ出さず `SHIORI_E_PANIC` を返す（Rust では `catch_unwind` で包む）
- 規約を変えたら、`shiori4_abi/src/lib.rs` 冒頭の `//!` と C ヘッダのコメントを**両方**直す（同じ文・同じ番号にする。食い違いは `shiori4_abi/tests/textbook.rs` が落とす）
- 規約の番号は各節の最上位の項目にだけ付け、「<節の名前> N」（スレッド規約は「スレッド規約 N」）で引く。番号は使い回さず、新しい規約は節の末尾に足し、やめた規約は番号を残して「（廃止）」と書く（`doc/textbook/rules.md` の冒頭）

### スレッド規約
- `load` / `request` / `notify` / `unload` はホストがインスタンスごとに決めた単一スレッドから。`GhostRuntime` は `!Send`、呼び出しは `&mut self` で型として強制する
- `host.request` を呼べるのは、load / request / notify を呼ばれているスレッドから、その呼び出しの間だけ（v4）。違反には `SHIORI_E_WRONG_THREAD`（-7）を返してよい
- `host.request` の処理中に SHIORI を呼び返さない（`SHIORI_E_REENTRANT`）。`Host` のメソッドは `&self` だけでランタイムに触れない
- `post` と `log` は任意スレッドから。ホストはコピーしてすぐ返る（post はキューに積み、イベントループ（`pump`）から届ける）
- `ShioriBytes.release` は常にホストの関数で、いつでも・どのスレッドから呼んでもよい（unload の後でもよい）
- unload は `Drop` で表す。unload が返った後、SHIORI はホストを呼ばない。高水準層は unload を始めた時点で Poster を無効にするので、join は要らない

### Unsafe
- `unsafe` は ABI 層に閉じ込め、上位層には安全な型（`ShioriOwnedBytes`, `ShioriInstance`, `ShioriHostHandle` など）だけを見せる
- 生ポインタを受ける関数は `shiori_slice_arg` / `shiori_str_arg` で検証してから使う

### Code Quality
- `cargo clippy --all-targets --workspace` を `-D warnings` で通す
- コメントとドキュメントは日本語。規約はクレート冒頭の `//!` に番号付きで書き、実装側から「規約 N」として参照する

### Testing
- 各ファイル末尾の `#[cfg(test)] mod tests` に置く（`shiori4_host` は役割ごとのファイルにそれぞれ持つ）
- ホスト側のテストは `GhostRuntime::from_static` でテスト用 `Ghost` を DLL なしに直結して、往復を検証する
- 教科書（`doc/textbook/`）とルートの README の検査: 正本の写し（規約・配布の決まり・語の表・C の断片）の一致は `shiori4_abi/tests/textbook.rs`（`exclude` でパッケージから外す。`cargo package` の「含まれないテストを無視する」の警告 1 つは受け入れる）。Rust の例は `shiori4_host/src/lib.rs` の `#[cfg(doctest)]` の項目が `include_str!` で doctest に載せる。C の例のビルドと往復は CI の job `textbook`
- DLL を通すテストは `shiori4_host/src/test_dll.rs` の `dll_path()` が作るテスト用 cdylib（`testdata/testghost/`）を読む。DLL の状態はプロセスに 1 つなので `lock()` を取ってから使う
- 従来の口（`LegacyRuntime`）のテストは x86 の Windows でだけ走り、従来の SHIORI/3.0 の口を書き出すテスト用 cdylib（`testdata/legacyghost/`）を読む。i686 では `cargo test --all --target i686-pc-windows-msvc` で走らせ、DLL も `--target i686-pc-windows-msvc` で作る（例: `cargo build --manifest-path testdata/legacyghost/Cargo.toml --target-dir target/legacyghost --target i686-pc-windows-msvc`）

### 公開とバージョン
- 版はルート `Cargo.toml` の **3 か所**だけに書く: `[workspace.package] version` と、`[workspace.dependencies]` の `shiori4_abi`・`shiori4` の `version`。3 つは常に同じ値にする（`0.0.x` の版要求は完全一致なので、直し忘れると依存解決で落ちる）
- 版は `/release` で初めて決まる。まだ出ていない版を文書・spec・注釈に書くときは、番号を予想して書かず、`doc/release-notes/README.md` の「次の版の印」を書く（`/release` の段 4 が版に置き換える）。印の綴りはここに写さない（写すと、ここも置き換えの対象になる）
- 共通メタデータ（`edition`・`rust-version`・`authors`・`license`・`repository`・`homepage`・`keywords`）も `[workspace.package]` に置き、各クレートは `<field>.workspace = true` で受け継ぐ。クレート固有なのは `name` と `description` だけ
- ライセンスは **MIT だけ**。各クレートのディレクトリにルート `LICENSE` の写しを置く（`license-file` は使わない）
- 公開はリリースタグ `vX.Y.Z` の push で動くリリース CI（`.github/workflows/release.yml`）だけが行う。段は `verify` → `gate` → `publish-crates` → `github-release` → `report`。開発者は `/release` で版の更新からタグの push・見届けまでを行う
- crates.io への公開の順は依存順の `shiori4_abi` → `shiori4` → `shiori4_host`（1 つずつ。公開済みの版は飛ばす）
- 認証は crates.io の Trusted Publishing（OIDC）。environment `release` の job で、1 回の実行につき 1 回だけ auth してトークン（30 分）を取り、3 クレートの公開に使う。API トークンは持たない
- 手元から公開しない（`cargo publish` を手で打たない）。例外は初回の手動公開と緊急時の手動公開だけで、手順は `.github/release-ci-setup.md` に従う

### CI
`.github/workflows/build.yml` が `main` への push・PR・手動（`workflow_dispatch`）で走り、`workflow_call` で他のワークフローからも呼べる。ジョブは 6 種:
- **test**: ubuntu / windows / macos で `cargo test --all` と `cargo clippy --all-targets --workspace -- -D warnings`
- **deny**: `cargo deny check`（設定はルートの `deny.toml`。ライセンスの許可・勧告・取得元）
- **package**: `cargo package --workspace` と中身の検査（`shiori4_abi` に C ヘッダが入るか、各クレートの `LICENSE` がルートと同じか）
- **release-scripts**: リリースの補助スクリプトのテスト（`pwsh -NoProfile -File .github/scripts/release/release-scripts-test.ps1`）
- **textbook**: ubuntu / windows / macos で、testghost・試験用のホスト（`shiori4_host/examples/shiori4-testhost.rs`）・`doc/textbook/c/` の C の例（Windows は MSVC で SHIORI の DLL を `/MT`、ほかは `cc`）をビルドし、終了の状態で往復を確かめる
- **test-i686**（`test-i686-windows`）: windows で、i686（`i686-pc-windows-msvc`）の `cargo test --all`（doctest を含む）と clippy（`-D warnings`）、legacyghost 自身の test と clippy（自前の workspace なので manifest を指定）。testghost・legacyghost・試験用のホストを i686 でビルドし、従来の口の往復（`--entry legacy` を含む）を終了の状態で確かめる

`.github/workflows/release.yml` はリリースタグ `v[0-9]+.[0-9]+.[0-9]+` の push だけで走る。job は次の 5 つで、上流が失敗すると後続は走らない（`report` だけは常に走る）:
- **verify**: タグの形・タグと `[workspace.package] version` の一致・タグのコミットが main から到達できること・見どころ `doc/release-notes/vX.Y.Z.md` の有無
- **gate**: build.yml を `workflow_call` で呼び、全検査をタグのコミットで通す
- **publish-crates**: crates.io への公開（environment `release`・Trusted Publishing）
- **github-release**: GitHub Release を作る（本文は見どころ＋詳しいリリースノート。書き込みの権限はこの job だけ）
- **report**: 公開先ごとの結果を 1 枚の表にまとめる

## Development Environment

### Required Tools
- Rust 1.85+（stable）

### Common Commands
```bash
# Build: cargo build
# Test:  cargo test --all
# Lint:  cargo clippy --all-targets --workspace -- -D warnings
# Package: cargo package --workspace
```
- 単独の `cargo package -p <crate>` は、下位クレートが crates.io に初めて公開されるまで依存を解決できず使えない。`--workspace` で検証する

## Key Technical Decisions

- **芯は中身を解釈しない**: 3 クレートの芯は運ぶ文字列を解析せず原文のまま渡す（求めるのは UTF-8 であることだけ）。解析と組み立ては芯から独立した便利関数 `shiori4::protocol` に置く。中身の形が替わっても芯を変えずに済むようにするため
- **ABI はバイト列の運搬だけ**: SHIORI/3.0 の解析・v3/v4 変換・プロパティは上位層の責務。ABI を小さく保ち、言語非依存にするため
- **v3/v4 の差のうち透過にするのは文字列形式だけ**: ホストを呼べるか（`Option<ShioriHostHandle>`）は `Ghost` に見せる。v3 で同期プロパティ取得ができない事実を隠さない
- **完了通知は通常のイベント**: post の結果は専用 API ではなく、`Tag` 付きの request / notify として後から届ける。届けるのは `pump` の中だけ（post 中や host.request 中には届けない）
- **`Cargo.lock` はコミットしない**（ライブラリのワークスペースのため）: ライブラリの lock は利用者のビルドでは使われない。lock が無いので CI は利用者と同じく毎回最新の依存で検査する。公開にも `--locked` を使わない

---
_Document standards and patterns, not every dependency_
