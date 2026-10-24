# Project Structure

## Organization Philosophy

**層ごとのクレート分割**（ABI → SHIORI 側 → ホスト側）。クレートの中心は `src/lib.rs` で、ファイル内を `// ====` の区切りコメントでセクション分けする。
`shiori4_host` だけは、読み込みの口（v3 互換モード・口の発見）と純粋な関数（配布の決まり）を役割ごとの別ファイル（`mod`）に分け、テスト専用の補助も別ファイル（`#[cfg(test)] mod`）に置く。
ファイルが大きくなって分割するときも、層の境界（どのクレートに置くか）を先に決める。

## Directory Patterns

### ABI 層
**Location**: `/shiori4_abi/`
**Purpose**: C ABI の型・定数・関数テーブル、ABI 層の trait（`Shiori`, `ShioriHost`）、`export_shiori!`。中身を解釈するコードは置かない
**Example**: `ShioriBytes`, `ShioriApi`, `SHIORI_E_PANIC`, `include/shiori4_abi.h`

### SHIORI 側の高水準層
**Location**: `/shiori4/`
**Purpose**: ゴースト開発者向け API。芯（`Ghost`・`Adapter`、v3 変換 `mod v3`）と、芯から独立した便利関数（`mod protocol`: ヘッダ処理・語の定数・組み立てと読み分け・プロパティの取得）
**Example**: `Ghost`, `export_ghost!`, `protocol::Headers`, `protocol::Properties`, `protocol::PendingTags`

### ホスト側の高水準層
**Location**: `/shiori4_host/`
**Purpose**: ベースウェア開発者向け API。読み込み、ホスト要求の振り分け（`Bridge`）、post キュー、イベント配送
**Example**: `Host`, `GhostRuntime`, `Post`, `Completer`, `Event`
試験用のホストは `examples/shiori4-testhost.rs`（終了の状態が下流の CI の契約。使い方は `doc/textbook/testhost.md`）

### 教科書
**Location**: `/doc/textbook/`
**Purpose**: 目次（`README.md`）と章（規約の一覧と理由・語・C・別のプロセス・移行・配布・試験用のホスト）。C の例は `c/` に全文を置く。正本（C ヘッダと `//!`・`doc/host-protocol.md`）を写すときは一字一句写し、理由は教科書にだけ置く。全体図と最小の例はルートの README に置き、目次から導く
**Example**: `rules.md`, `c/minimal_shiori.c`

## Naming Conventions

- **クレート**: `shiori4` + `_abi` / `_host` の接尾辞
- **ABI 層の型・定数**: C 側と揃えるため `Shiori` / `SHIORI_` 接頭辞（`ShioriBytes`, `SHIORI_200_OK`, `SHIORI_E_SHUTDOWN`, `SHIORI_LOG_INFO`）
- **高水準層の型**: 接頭辞なしの短い名前（`Ghost`, `Host`, `Headers`, `Event`）
- **ステータス定数**: `SHIORI_<コード>_<理由>`、ABI エラーは `SHIORI_E_<名前>`
- **プロトコル語彙**: `protocol` 内で `ID_*` / `HEADER_*` / `COMMAND_*` / `OPTION_*` / `REASON_*` / `OUTCOME_*` / `SENDER_TYPE_*` / `SECURITY_LEVEL_*`
- **所有の区別**: 借用・未所有は `ShioriBytes` / `ShioriResponse`、解放責任を持つものは `ShioriOwned*`

## Import Organization

上位クレートは下位クレートを再エクスポートし、利用者は1クレートだけ use すれば済むようにする。

```rust
// shiori4: ABI 層を丸ごと再エクスポート
pub use shiori4_abi;
pub use shiori4_abi::*;

// shiori4_host: 必要なものだけ明示的に再エクスポート
pub use shiori4;
pub use shiori4::protocol;
```

クレート間の参照は、ルートの `[workspace.dependencies]` に `path`（手元のソース）と `version`（公開時の版要求）の両方を書き、各クレートが `<dep>.workspace = true` で受け継ぐ。

## Code Organization Principles

- 依存は下向きだけ（`shiori4_host` → `shiori4` → `shiori4_abi`）。`shiori4_abi` は何にも依存しない
- 層をまたぐ語彙（メッセージ名など）は、両側が依存できる最下位の高水準層（`shiori4`）に置く
- 規約・設計方針はクレート冒頭の `//!` に書き、README は使い方の入口だけにする。読み物は `doc/textbook/` に置き、README と crate doc から GitHub の絶対 URL で導く
- 各クレートの `README.md` は crates.io の顔（そのクレートの役割・利用者・使い方）。ルートの `README.md` はワークスペース全体の入口で、各クレートへ導く
- テストは同じファイルの `mod tests` に置く

---
_Document patterns, not file trees. New files following patterns shouldn't require updates_
