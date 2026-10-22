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
- `cargo clippy` を `-D warnings` で通す
- コメントとドキュメントは日本語。規約はクレート冒頭の `//!` に番号付きで書き、実装側から「規約 N」として参照する

### Testing
- 各クレートの `lib.rs` 末尾の `#[cfg(test)] mod tests` に置く
- ホスト側のテストは `GhostRuntime::from_static` でテスト用 `Ghost` を DLL なしに直結して、往復を検証する

## Development Environment

### Required Tools
- Rust 1.85+（stable）

### Common Commands
```bash
# Build: cargo build
# Test:  cargo test --all
# Lint:  cargo clippy --all-targets -- -D warnings
```

## Key Technical Decisions

- **ABI はバイト列の運搬だけ**: SHIORI/3.0 の解析・v3/v4 変換・プロパティは上位層の責務。ABI を小さく保ち、言語非依存にするため
- **v3/v4 の差のうち透過にするのは文字列形式だけ**: ホストを呼べるか（`Option<ShioriHostHandle>`）は `Ghost` に見せる。v3 で同期プロパティ取得ができない事実を隠さない
- **完了通知は通常のイベント**: post の結果は専用 API ではなく、`Tag` 付きの request / notify として後から届ける。届けるのは `pump` の中だけ（post 中や host.request 中には届けない）
- **`Cargo.lock` はコミットしない**（ライブラリのワークスペースのため）

---
_Document standards and patterns, not every dependency_
