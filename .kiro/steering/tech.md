# Technology Stack

## Architecture

3 層のクレート構成。下の層は上の層を知らない。

```
shiori4_abi   … C ABI（関数テーブル・ShioriBytes・版・規約）。ペイロードは解釈しない
   ↑
shiori4       … SHIORI 側の高水準層（Ghost, Adapter, Headers, Properties, PendingTags, host_protocol）
   ↑
shiori4_host  … ホスト側の高水準層（Host, GhostRuntime, post キュー, pump）
```

- ホストと SHIORI が共有するメッセージ名（`ID: property`, `Tag`, `OnHostComplete` など）は `shiori4::host_protocol` に置き、ホスト側はそれを再エクスポートして使う
- 各層は「ABI 層の trait（`Shiori` / `ShioriHost`）」と「高水準 trait（`Ghost` / `Host`）」の2段。実装者は通常、高水準 trait だけを実装する

## Core Technologies

- **Language**: Rust 1.85 以上、edition 2024（`[workspace.package]` で一括指定）
- **FFI**: `extern "C"` + `#[repr(C)]`。C ヘッダ `shiori4_abi/include/shiori4_abi.h` は Rust 側と同一レイアウトを手で保つ
- **動的読み込み**: `libloading`（`shiori4_host` のみ）

## Key Libraries

外部依存は最小に保つ。`shiori4_abi` と `shiori4` は依存ゼロ（std のみ）、`shiori4_host` も `libloading` だけ。

## Development Standards

### ABI 規約（破ると互換が壊れる）
- DLL 境界を越えるのは `#[repr(C)]` 構造体と `extern "C"` 関数ポインタだけ
- ペイロードは UTF-8 の (ptr, len)。NUL 終端にしない。引数は呼び出し中だけの借用
- 所有権が移るのは `out` の `ShioriBytes` だけ。確保した側の `release` で解放する
- 構造体は末尾にだけ追加する。追加する関数ポインタは `Option<unsafe extern "C" fn(..)>`。受け手は `struct_size` で判定する
- 戻り値: 0 以上は SHIORI/3.0 のステータスコード、負は ABI エラー（`SHIORI_E_*`）
- エクスポートする関数は `catch_unwind` で包み、パニックを `SHIORI_E_PANIC` にする
- 規約を変えたら、`shiori4_abi/src/lib.rs` 冒頭の `//!` と C ヘッダのコメントを**両方**直す

### スレッド規約
- `request` / `notify` はホストが決めた単一スレッドから。`GhostRuntime` は `!Send`、呼び出しは `&mut self` で型として強制する
- `host.request` の処理中に SHIORI を呼び返さない（`SHIORI_E_REENTRANT`）。`Host` のメソッドは `&self` だけでランタイムに触れない
- `post` は任意スレッドから。ホストはコピーしてキューに積み、イベントループ（`pump`）から届ける
- unload は `Drop` で表す。SHIORI は post するスレッドを止めて join してから返る

### Unsafe
- `unsafe` は ABI 層に閉じ込め、上位層には安全な型（`ShioriOwnedBytes`, `ShioriInstance`, `ShioriHostHandle` など）だけを見せる
- 生ポインタを受ける関数は `shiori_slice_arg` / `shiori_str_arg` で検証してから使う

### Code Quality
- `cargo clippy --all-targets --workspace` を `-D warnings` で通す
- コメントとドキュメントは日本語。規約はクレート冒頭の `//!` に番号付きで書き、実装側から「規約 N」として参照する

### Testing
- 各クレートの `lib.rs` 末尾の `#[cfg(test)] mod tests` に置く
- ホスト側のテストは `GhostRuntime::from_static` でテスト用 `Ghost` を DLL なしに直結して、往復を検証する

### 公開とバージョン
- 版はルート `Cargo.toml` の **3 か所**だけに書く: `[workspace.package] version` と、`[workspace.dependencies]` の `shiori4_abi`・`shiori4` の `version`。3 つは常に同じ値にする（`0.0.x` の版要求は完全一致なので、直し忘れると依存解決で落ちる）
- 共通メタデータ（`edition`・`rust-version`・`authors`・`license`・`repository`・`homepage`・`keywords`）も `[workspace.package]` に置き、各クレートは `<field>.workspace = true` で受け継ぐ。クレート固有なのは `name` と `description` だけ
- ライセンスは **MIT だけ**。各クレートのディレクトリにルート `LICENSE` の写しを置く（`license-file` は使わない）
- 公開はリリースタグ `vX.Y.Z` の push で動くリリース CI（`.github/workflows/release.yml`）だけが行う。段は `verify` → `gate` → `publish-crates` → `github-release` → `report`。開発者は `/release` で版の更新からタグの push・見届けまでを行う
- crates.io への公開の順は依存順の `shiori4_abi` → `shiori4` → `shiori4_host`（1 つずつ。公開済みの版は飛ばす）
- 認証は crates.io の Trusted Publishing（OIDC）。environment `release` の job で、1 回の実行につき 1 回だけ auth してトークン（30 分）を取り、3 クレートの公開に使う。API トークンは持たない
- 手元から公開しない（`cargo publish` を手で打たない）。例外は初回の手動公開と緊急時の手動公開だけで、手順は `.github/release-ci-setup.md` に従う

### CI
`.github/workflows/build.yml` が `main` への push・PR・手動（`workflow_dispatch`）で走り、`workflow_call` で他のワークフローからも呼べる。ジョブは 4 種:
- **test**: ubuntu / windows / macos で `cargo test --all` と `cargo clippy --all-targets --workspace -- -D warnings`
- **deny**: `cargo deny check`（設定はルートの `deny.toml`。ライセンスの許可・勧告・取得元）
- **package**: `cargo package --workspace` と中身の検査（`shiori4_abi` に C ヘッダが入るか、各クレートの `LICENSE` がルートと同じか）
- **release-scripts**: リリースの補助スクリプトのテスト（`pwsh -NoProfile -File .github/scripts/release/release-scripts-test.ps1`）

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

- **ABI はバイト列の運搬だけ**: SHIORI/3.0 の解析・v3/v4 変換・プロパティは上位層の責務。ABI を小さく保ち、言語非依存にするため
- **v3/v4 の差のうち透過にするのは文字列形式だけ**: ホストを呼べるか（`Option<ShioriHostHandle>`）は `Ghost` に見せる。v3 で同期プロパティ取得ができない事実を隠さない
- **完了通知は通常のイベント**: post の結果は専用 API ではなく、`Tag` 付きの request / notify として後から届ける。届けるのは `pump` の中だけ（post 中や host.request 中には届けない）
- **`Cargo.lock` はコミットしない**（ライブラリのワークスペースのため）: ライブラリの lock は利用者のビルドでは使われない。lock が無いので CI は利用者と同じく毎回最新の依存で検査する。公開にも `--locked` を使わない

---
_Document standards and patterns, not every dependency_
