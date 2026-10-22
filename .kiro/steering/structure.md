# Project Structure

## Organization Philosophy

**層ごとのクレート分割**（ABI → SHIORI 側 → ホスト側）。1クレートは現状 `src/lib.rs` 1ファイルで、ファイル内を `// ====` の区切りコメントでセクション分けする。
ファイルが大きくなって分割するときも、層の境界（どのクレートに置くか）を先に決める。

## Directory Patterns

### ABI 層
**Location**: `/shiori4_abi/`
**Purpose**: C ABI の型・定数・関数テーブル、ABI 層の trait（`Shiori`, `ShioriHost`）、`export_shiori!`。中身を解釈するコードは置かない
**Example**: `ShioriBytes`, `ShioriApi`, `SHIORI_E_PANIC`, `include/shiori4_abi.h`

### SHIORI 側の高水準層
**Location**: `/shiori4/`
**Purpose**: ゴースト開発者向け API。ヘッダ処理、v3 変換（`mod v3`）、ホストとの共有語彙（`mod host_protocol`）
**Example**: `Ghost`, `export_ghost!`, `Headers`, `Properties`, `PendingTags`

### ホスト側の高水準層
**Location**: `/shiori4_host/`
**Purpose**: ベースウェア開発者向け API。読み込み、ホスト要求の振り分け（`Bridge`）、post キュー、イベント配送
**Example**: `Host`, `GhostRuntime`, `Post`, `Completer`, `Event`

## Naming Conventions

- **クレート**: `shiori4` + `_abi` / `_host` の接尾辞
- **ABI 層の型・定数**: C 側と揃えるため `Shiori` / `SHIORI_` 接頭辞（`ShioriBytes`, `SHIORI_200_OK`, `SHIORI_E_SHUTDOWN`, `SHIORI_LOG_INFO`）
- **高水準層の型**: 接頭辞なしの短い名前（`Ghost`, `Host`, `Headers`, `Event`）
- **ステータス定数**: `SHIORI_<コード>_<理由>`、ABI エラーは `SHIORI_E_<名前>`
- **プロトコル語彙**: `host_protocol` 内で `ID_*` / `HEADER_*`
- **所有の区別**: 借用・未所有は `ShioriBytes` / `ShioriResponse`、解放責任を持つものは `ShioriOwned*`

## Import Organization

上位クレートは下位クレートを再エクスポートし、利用者は1クレートだけ use すれば済むようにする。

```rust
// shiori4: ABI 層を丸ごと再エクスポート
pub use shiori4_abi;
pub use shiori4_abi::*;

// shiori4_host: 必要なものだけ明示的に再エクスポート
pub use shiori4;
pub use shiori4::{host_protocol, HeaderWriter, Headers};
```

クレート間の参照は、ルートの `[workspace.dependencies]` に `path`（手元のソース）と `version`（公開時の版要求）の両方を書き、各クレートが `<dep>.workspace = true` で受け継ぐ。

## Code Organization Principles

- 依存は下向きだけ（`shiori4_host` → `shiori4` → `shiori4_abi`）。`shiori4_abi` は何にも依存しない
- 層をまたぐ語彙（メッセージ名など）は、両側が依存できる最下位の高水準層（`shiori4`）に置く
- 規約・設計方針はクレート冒頭の `//!` に書き、README は使い方の入口だけにする
- 各クレートの `README.md` は crates.io の顔（そのクレートの役割・利用者・使い方）。ルートの `README.md` はワークスペース全体の入口で、各クレートへ導く
- テストは同じファイルの `mod tests` に置く

---
_Document patterns, not file trees. New files following patterns shouldn't require updates_
