# Research & Design Decisions — crate-publish-ready

## Summary
- **Feature**: `crate-publish-ready`
- **Discovery Scope**: Extension（既存の Cargo.toml の書き換え）＋ Simple Addition（README・CI・deny.toml の新規）
- **Key Findings**:
  - 手元の写し（スクラッチ）で版の一本化と `[workspace.dependencies]`（path＋version）に書き換えると、`cargo package --workspace` と `cargo publish --workspace --dry-run` は **3 クレートとも未公開のまま**依存の順に通る（cargo 1.99.1。一時のローカルレジストリで下位クレートを解決する）。一方 `cargo package -p shiori4` を単独で実行すると、`shiori4_abi` が crates.io に無いので検証が失敗する。要件 4.2 を満たすには「ワークスペース単位で」パッケージ化する必要がある
  - C ヘッダ `include/shiori4_abi.h` は、`include`/`exclude` を書かなくても既定でパッケージに入る（`cargo package --list -p shiori4_abi` で確認）。ただし **ルートの LICENSE はどのパッケージにも入らない**（`license = "MIT"` だけでは複製されない）
  - pasta の `deny.toml` をそのまま当てると `advisories ok, bans ok, licenses ok, sources ok` で通るが、使われない許可ライセンス（BSD・Zlib・BSL など）ごとに `license-not-encountered` の警告が出る。現在の依存のライセンスは MIT・Apache-2.0・ISC（libloading）だけ
  - 手元（Windows）では現状のまま `cargo test --all` と `cargo clippy --all-targets --workspace -- -D warnings` が通る。ubuntu / macOS での結果は未確認

## Research Log

### 現状の資産（Current State）
- **Context**: 要件と既存コードの差を洗う
- **Sources Consulted**: `Cargo.toml`（ルート・各クレート）、`README.md`、`LICENSE`、`.gitignore`、`shiori4*/src/lib.rs`、`.kiro/steering/*`、pasta の `Cargo.toml`・`.github/workflows/build.yml`・`deny.toml`
- **Findings**:
  - ルート `[workspace.package]` は `edition`・`rust-version`・`license = "MIT OR Apache-2.0"` だけ。`[workspace.dependencies]` は無い
  - 各クレートは `version = "0.1.0"` 直書き、`description` あり、`edition`・`rust-version`・`license` を `.workspace = true` で受けている
  - 内部依存は `shiori4_abi = { path = "../shiori4_abi" }`・`shiori4 = { path = "../shiori4" }`（version なし）。外部依存は `shiori4_host` の `libloading = "0.8"` だけ
  - README はルートの 1 つだけ（クレート表と使い方の入口）。各クレートの `lib.rs` 冒頭に `//!` の詳しいドキュメントがある（docs.rs の顔になる）。`#![doc = include_str!("../README.md")]` のような README の取り込みは無い
  - `.gitignore` は `Cargo.lock` と `target` を無視。`.github/` は無い
  - OS 固有のコードは `lib.rs` に無い（`cfg(windows)` などの分岐なし）。ホスト側のテストは `GhostRuntime::from_static` で DLL なしに往復するので、3 OS で同じテストが走る見込み。doctest は `ignore` 指定で実行されない
- **Implications**: 変更は Cargo.toml 4 本の書き換えと、README 3 本・CI 1 本・deny.toml 1 本の新規、steering 2 本の更新に収まる。Rust のソースには触れない（要件 7）

### パッケージ化の可否（実験）
- **Context**: 要件 4.1・4.2（未公開のまま依存の順に通るか）
- **Sources Consulted**: スクラッチにワークスペースを写して（`.git` なし）Cargo.toml を書き換え、cargo 1.99.1 で実行
- **Findings**:
  - 現状のまま `cargo package -p shiori4`: `all dependencies must have a version requirement specified when packaging. dependency shiori4_abi does not specify a version` で失敗（brief の記述どおり）
  - 書き換え後 `cargo package --workspace`: 3 クレートを Packaging → Verifying し、`shiori4` の検証では `target/package/tmp-registry` から `shiori4_abi 0.0.1` を展開して成功
  - 書き換え後 `cargo publish --workspace --dry-run`: 3 クレートとも `aborting upload due to dry run` まで到達
  - 書き換え後 `cargo package -p shiori4`（単独）: `no matching package named shiori4_abi found` で失敗
  - パッケージの中身: `shiori4_abi` は `Cargo.lock`・`Cargo.toml`・`Cargo.toml.orig`・`include/shiori4_abi.h`・`src/lib.rs`（README を足せば加わる）。`repository` 等が無いと `manifest has no documentation, homepage or repository` の警告
- **Implications**:
  - CI と手元の検証は「ワークスペース単位」（`--workspace`）で行う必要がある。単独のクレートでの dry-run は初回公開まで使えない（release-workflow の公開スクリプトが 1 クレートずつ `cargo publish -p` するなら、下位から順に公開すれば問題ない）
  - 実験は `.git` の無い写しで行った。実リポジトリでは cargo が git の追跡ファイルを基準にするため、未追跡の README を作ってコミットしないまま package すると入らない／dirty 扱いになる点に注意（CI は clean checkout なので問題ない）

### 依存検査（cargo-deny）
- **Context**: 要件 5.4・5.5
- **Sources Consulted**: pasta の `deny.toml`、手元の `cargo-deny 0.20.2` でスクラッチに対して `cargo deny check`・`cargo deny list`
- **Findings**:
  - 結果は全項目 ok。使われていない許可ライセンス 8 件について `license-not-encountered` の警告（失敗ではない）
  - 依存のライセンス: `cfg-if`・`windows-link` が MIT OR Apache-2.0、`libloading` が ISC、自クレートは MIT
  - pasta の `[bans] wildcards = "allow"` は path だけの依存を許すための設定。内部依存に version を付ければ `deny` にもできる
  - `Cargo.lock` が無い場合、cargo-deny はその場で lock を作って検査する（検査の時点の最新の依存で判定される）
- **Implications**: 許可リストは「MIT・Apache-2.0・ISC」に絞れば警告ゼロ、pasta のままなら将来の依存追加に強いが警告が出る。要件ディスカッションで「MIT・Apache-2.0・ISC」に絞ると確定した（要件 5.4）

### pasta の build.yml からの縮小
- **Context**: 要件 5・6
- **Findings**:
  - 残すもの: `on:`（push main・pull_request main・`workflow_dispatch`・`workflow_call: {}`）、`actions/checkout@v6`、`dtolnay/rust-toolchain@stable`、`Swatinem/rust-cache@v2`、`deny` job（`EmbarkStudios/cargo-deny-action@v2`、ubuntu）、`fail-fast: false`
  - 落とすもの: x86/x64 の target matrix、ロケール設定、`pasta_shiori` の release ビルド、LongPaths の FFI テスト、artifact の upload、luacheck・wasm・release-scripts の job
  - 変えるもの: matrix を `os: [ubuntu-latest, windows-latest, macos-latest]` に。clippy は pasta では x64 だけだが、本 spec の要件 5.1 は 3 OS で clippy を回す（cfg の差で警告が変わり得るため）
  - 足すもの: パッケージ化の検証の step / job（pasta には無い）
- **Implications**: job 構成（パッケージ化をどの OS で回すか、test と clippy を同じ job にするか）は設計で決める

## Requirement-to-Asset Map

| 要件 | 既存の資産 | ギャップ | 種別 |
|---|---|---|---|
| 1 版の一本化 | `[workspace.package]` あり（version なし）、各クレートに `0.1.0` 直書き | `version = "0.0.1"` の追加、各クレートの `version.workspace = true`、`[workspace.dependencies]` の新設と内部依存の移行 | Missing |
| 1.6 版の食い違いの検出 | なし | caret 要求のため互換の範囲の食い違いは検出できない。厳密な一致の検査は release-workflow の verify に委ねるか、設計で `=0.0.1` 指定などを検討 | Constraint |
| 2 メタデータ・ライセンス | `description`・`edition`・`rust-version`・`license`（`MIT OR Apache-2.0`） | `license = "MIT"` への修正、`authors`・`repository`・`homepage`・`documentation` の追加 | Missing |
| 2.6 ライセンス文の同梱 | ルートの `LICENSE`（MIT） | パッケージに入らない（確認済み）。各クレートに写す、`license-file` で指す、など手段が要る | Missing（入れると確定。手段は設計） |
| 3 各クレートの README | ルート `README.md` のみ、各 `lib.rs` 冒頭の `//!` | 3 本の新規作成。ルート README からのリンク | Missing |
| 4 パッケージ化 | なし | `--workspace` でなら未公開のまま通ることを確認済み。C ヘッダは既定で入る | Constraint（単独 `-p` は不可） |
| 5 CI | なし（`.github/` 無し） | `.github/workflows/build.yml` と `deny.toml` の新規。ubuntu / macOS でのテスト結果は未確認 | Missing / Unknown |
| 6 gate 呼び出し | なし | `on: workflow_call: {}`（入力なし）。呼び出し側は release-workflow | Missing |
| 7 変えないもの | 既存テスト（Windows で全通過）、clippy 警告ゼロ | Rust ソースに触れない方針で満たせる | — |
| 8 steering | `tech.md`（Cargo.lock はコミットしない、lint コマンドに `--workspace` なし）、`structure.md`（「path 依存で参照」） | 版の置き場所・CI・lock 方針・内部依存の書き方・README の位置づけの追記と修正 | Missing |

## Implementation Approach Options

### Option A: 既存の Cargo.toml を拡張し、CI・設定・README は pasta の写しを縮める（Extend + 写し）
- ルート Cargo.toml に `version`・メタデータ・`[workspace.dependencies]`、各クレートは `.workspace = true` に置き換え
- README は各クレートのディレクトリに置く（cargo の既定で拾われる）
- build.yml・deny.toml は pasta から必要な job / 設定だけを残す
- ✅ brief の Approach そのもの。差分が小さく、開発者が pasta で慣れた形
- ✅ release-workflow が pasta の公開スクリプトを移植するときに前提がそろう
- ❌ pasta の deny.toml をそのまま持ち込むと未使用ライセンスの警告が残る（絞れば解消）

### Option B: README をルートの 1 本で共有する（`readme = "../README.md"`）
- 各クレートが同じルート README を crates.io の顔に使う
- ✅ ファイルが増えない
- ❌ brief の Scope（各クレートの README）と要件 3.2（クレートごとの役割と入口）に合わない。3 クレートで同じページになり、どれを使うべきか分かりにくい

### Option C: Option A ＋ ライセンス文・C ヘッダの同梱を明示（Hybrid）
- Option A に加え、各クレートに LICENSE を置く（または `license-file`・`include` で明示）し、CI のパッケージ化の検証で中身（C ヘッダ・README・LICENSE）を確かめる
- ✅ 要件 2.6・4.3 を既定の挙動に頼らず保証できる
- ❌ LICENSE の写しが 3 つに増える（本文の同期が要る）。`license` と `license-file` を両方書くと cargo が警告する点に注意

**推奨（参考）**: Option C。要件ディスカッションで LICENSE の同梱（要件 2.6）と C ヘッダの CI 検査（要件 5.6）が確定したため、Option A に Option C の部分を足す。

## Implementation Complexity & Risk
- **Effort**: S（1〜3 日）— 既存の pasta の形を写すだけで、Rust のソースに触れない。新しい技術要素なし
- **Risk**: Low〜Medium — 手元でパッケージ化と依存検査の通過を確認済み。未確認なのは ubuntu / macOS でのテストと clippy、CI 上の挙動（キャッシュ・lock なしでの cargo-deny）だけ

## Design Decisions（設計に持ち越す）

### Decision: `Cargo.lock` をコミットするか
- **Context**: tech.md は「コミットしない（ライブラリのため）」。release-workflow が pasta と同じく `--locked` で公開するなら lock が要る（brief の Constraints）
- **Alternatives Considered**:
  1. コミットしない（現状維持）— CI と cargo-deny は毎回その時点の最新の依存で検査する。再現性は低いが、利用者が実際に引く依存で検査できる
  2. コミットする — CI・cargo-deny・公開が同じ依存で再現する。`.gitignore` から外し、`/release` の版の更新が lock も書き換える必要がある（release-workflow の brief は「方針しだいで Cargo.lock」と書いている）
- **Follow-up**: 決めた結果を tech.md（要件 8.2）と `.gitignore` に反映し、release-workflow に申し送る

### Decision: パッケージ化の検証を CI のどこで回すか
- **Context**: 要件 5.6。`cargo package --workspace` は OS に依らない検査だが、ビルドを伴う
- **Alternatives**: (1) 3 OS の matrix の各 job で回す (2) ubuntu の別 job で 1 回だけ回す
- **Follow-up**: 要件 5.7（失敗の見分け）を満たす job / step 名にする

### Decision: LICENSE の本文をパッケージに入れる方法
- **Context**: 要件 2.6（入れると確定）。`license = "MIT"` だけではルートの LICENSE はパッケージに入らない
- **Alternatives**: (1) 各クレートのディレクトリに LICENSE を写す（本文の同期が要る） (2) `license-file` でルートの LICENSE を指す（`license` との併記は警告） (3) `include` などでルートの LICENSE を取り込む
- **Follow-up**: 要件 4.4 の「パッケージの中身」の確認に LICENSE を含める

### Decision: 内部依存の版要求の書き方
- **Context**: 要件 1.6。`version = "0.0.1"`（caret）は 0.0.x では実質厳密一致になるが、0.1 以上では互換の範囲を受け入れる
- **Alternatives**: (1) pasta と同じ caret (2) `=0.0.1` の厳密指定
- **Follow-up**: release-workflow の `/release` が書き換える個数（3 か所）は変わらない

## Risks & Mitigations
- ubuntu / macOS でテストや clippy が初めて落ちる — 最初の CI 実行で確かめ、落ちたら本 spec の中で直す（ソースの修正が要るなら要件 7.1 に触れないか開発者に確認）
- 依存検査が、変更と無関係な新しい脆弱性勧告で PR を落とす — 想定どおりの挙動として受け入れる。必要なら `deny.toml` の ignore に理由付きで足す
- `cargo package -p <crate>` を単独で使う手順が混ざる — 手元・CI・steering のコマンドを `--workspace` で統一する
- 実リポジトリで未コミットの README を package すると dirty 扱いになる — 検証はコミット後に行う（CI は clean checkout）

## Research Needed（設計フェーズへ）
設計フェーズで解決した（下の「設計フェーズの調査」）。残るのは最後の 1 件だけ。
- ~~crates.io が `documentation` 未設定のときに docs.rs へのリンクを自動で出すか~~ → 出す（Cargo Book）
- ~~`license` と `license-file` の併記の扱い、LICENSE を置く最小の書き方~~ → 併記は警告。各クレートに写しを置く
- ~~rust-cache と cargo-deny-action の、`Cargo.lock` が無いときの挙動~~ → lock をコミットしないので、どちらも lock なしで動かす（記録は下）
- ubuntu-latest / macos-latest での `cargo test --all` と clippy の結果（最初の CI 実行で確認。設計でも未確認のまま）

---

## 設計フェーズの調査（kiro-spec-design, 2026-10-22）

- **Discovery Scope**: Extension（light discovery）。新しいライブラリ・Action は足さないので外部調査は Cargo Book と 4 つの Action の README・Cargo チームの指針に絞った
- **使ったスキル・ガイド**: `kiro-spec-design`（rules: design-principles・design-discovery-light・design-synthesis・design-review-gate）、`kiro-validate-design`（rules: design-review）。ドメイン固有のスキルはこのリポジトリに無い
- **実験の場所**: スクラッチに 3 クレートを写し（`git init` して追跡ファイルの挙動を再現）、design.md のマニフェストの契約どおりに書き換えた写し（`exp`）。cargo 1.99.1・cargo-deny 0.20.2・Windows

### 実験 A: 設計どおりのマニフェスト＋各クレートの README・LICENSE の写し
- `cargo package --workspace`: 3 クレートとも Packaging → Verifying で成功し、**warning は 0 件**（authors・homepage・repository・keywords あり、documentation なし）。要件 2.5 を満たす
- `cargo package --list -p <crate>`: `.cargo_vcs_info.json`・`Cargo.lock`・`Cargo.toml`・`Cargo.toml.orig`・`LICENSE`・`README.md`・`src/lib.rs`（`shiori4_abi` はさらに `include/shiori4_abi.h`）。`readme`・`include` を書かなくても README とヘッダは入る
- `cargo test --all --locked` と `cargo clippy --all-targets --workspace --locked -- -D warnings` が通る（Windows）

### 実験 B / B2 / C: ルートの LICENSE を入れる別の方法
- B（`license = "MIT"` ＋ `license-file = "../LICENSE"`）: パッケージには `LICENSE` が入るが、`cargo package` でも `cargo build` でも毎回 `warning: only one of license or license-file is necessary` が出る → 要件 2.5 の趣旨（警告を出さない）に反する
- B2（`license-file = "../LICENSE"` だけ）: パッケージの `Cargo.toml` に `license` が無くなる → crates.io で MIT と表示されず要件 2.1 に反する
- C（`include = ["../LICENSE", …]`）: パッケージの外のファイルは入らない（`LICENSE` が一覧に出ない）
- **結論**: 各クレートのディレクトリに LICENSE を写す（実験 A）。写しのずれは CI で `cmp` する

### 実験 E2: 版要求の食い違い
- `[workspace.package] version = "0.0.2"`、版要求は `0.0.1` のまま → `cargo build` が `failed to select a version for the requirement shiori4_abi = "^0.0.1"`（`candidate versions found which didn't match: 0.0.2`）で終了コード 101。要件 1.6 は caret のままで満たせる

### 実験 E3: `Cargo.lock` と `--locked`
- a) lock なしで `cargo test --all --locked` → `cannot create the lock file … because --locked was passed` で失敗。lock をコミットしないと `--locked` は使えない（`cargo package --no-verify --locked` は lock なしでも通ったが、検証のビルドや `cargo publish --locked` はリポジトリの lock を前提にする）
- b) lock が最新なら `cargo package --workspace --locked` は通る
- c) 版を 3 か所とも上げて lock を直さない → `cargo package --workspace --locked` も `cargo clippy … --locked` も `cannot update the lock file … because --locked was passed` で失敗。CI で lock の直し忘れを検出できる
- d) `cargo update --workspace` で lock の 3 クレートの版だけが更新される（外部依存は動かない）。release-workflow の `/release` はこれを使える
- 補足: resolver 3 は `rust-version = "1.85"` を見て `libloading 0.8.9` を選ぶ（`available: v0.9.0, requires Rust 1.88.0`）

### 実験 D: 絞った `deny.toml`
- `allow = ["MIT", "Apache-2.0", "ISC"]`、`wildcards = "deny"`、`unknown-registry`・`unknown-git = "deny"` で `advisories ok, bans ok, licenses ok, sources ok`、警告なし。lock の依存は `cfg-if`・`libloading`・`windows-link` と自クレート 3 つ

### 外部の資料
- **Cargo Book（manifest、手元の rust-docs）**: documentation 未設定なら crates.io が docs.rs へ自動でリンクする（「If no URL is specified in the manifest file, crates.io will automatically link your crate…」）。`readme` 未設定でも package root の `README.md` を使う。keywords は 5 個まで・20 字まで・ASCII。`license-file` は非標準のライセンス用
- **Swatinem/rust-cache**: キーに `Cargo.lock` / `Cargo.toml` のハッシュを含む（あれば）。lock の無いライブラリは効きが限られると明記
- **EmbarkStudios/cargo-deny-action**: 既定は `check`・`--all-features`、既定の Rust は 1.85.0。lock の無い場合の挙動は書かれていない（「Cargo.lock files checked into your repository」を前提にする記述がある）
- **Rust blog「Change in Guidance on Committing Lockfiles」（2023-08-29）**: ライブラリでも「プロジェクトに最善のことをする」。`cargo new` はライブラリでも lock を無視しなくなった。最新の依存での定期的な検査は勧める
- **pasta**: `Cargo.lock` をコミットし、`publish-crate.ps1` は `cargo publish -p $Crate --locked`。release.yml は lock が書き換わっていないことを `git diff --exit-code -- Cargo.lock` で確かめる

## 設計の決定（確定版）

### Decision: `Cargo.lock` をコミットしない（設計ディスカッションで改めた）
- **Selected**: コミットしない（tech.md の今の方針のまま）。どのジョブにも `--locked` を付けない
- **Rationale**: ライブラリの lock は利用者のビルドで使われず、効くのはこのリポジトリの CI だけ。外部依存は `libloading` 1 つで再現性の利点が小さい。lock が無ければ CI は利用者と同じく最新の依存で検査する。版の更新が 3 か所だけで済む。pasta が `--locked` で公開するのは DLL を配るためで、純粋なライブラリの shiori4 には当てはまらない
- **Trade-offs**: CI の結果に再現性が無い（依存の新しい版で変わり得る）。実験 E3 の lock の直し忘れの検出は使わない
- **History**: 設計の初稿は実験 E3 と pasta に合わせて「コミットする」を選んだが、開発者の指摘（ライブラリの lock は利用者に効かない）を受けて改めた
- **Follow-up**: tech.md の「コミットしない」に理由を添える。release-workflow の brief に「lock は無く、`--locked` を使わずに公開する」と申し送る（pasta の release.yml の `git diff --exit-code -- Cargo.lock` も移植しない）

### Decision: LICENSE は各クレートに写す
- **Selected**: 実験 A。`license-file` は使わない
- **Rationale**: 実験 B・B2・C のとおり、他の方法は警告か表示の誤りか不採用になる
- **Trade-offs**: 写しが 3 つ。本文は年が変わるくらいしか変わらず、CI の `cmp` でずれを検出する

### Decision: パッケージ化の検証は ubuntu の別ジョブで 1 回
- **Selected**: `package` ジョブ（ubuntu）。ジョブ名で見分けられる（要件 5.7）
- **Rationale**: 中身は OS に依らない。検証のビルドは `test` ジョブの 3 OS で済んでいる。CRLF の影響なく `cmp` できる

### Decision: 内部依存の版要求は caret（pasta と同じ）
- **Selected**: `version = "0.0.1"`。`=` の厳密指定はしない
- **Rationale**: 0.0.x では caret が実質厳密一致で、実験 E2 のとおり直し忘れを検出する。0.1 以上の検出漏れは要件 1.6 の但し書きで受け入れ済み。`/release` が書き換える個数は変わらない

### Decision: `libloading` は `[workspace.dependencies]` に移さない
- **Selected**: `shiori4_host/Cargo.toml` に `libloading = "0.8"` のまま
- **Rationale**: 要件 1.3 は内部依存だけを求める。外部依存は 1 つだけで、移しても版の管理は楽にならない（最小の差分）

### Decision: `deny.toml` は pasta から絞って厳しくする
- **Selected**: 許可ライセンス 3 つ、`wildcards = "deny"`、`unknown-registry`・`unknown-git = "deny"`、`db-path`・`db-urls` は既定に任せて省く
- **Rationale**: 要件 5.4・5.5（許可していない取得元で失敗）。pasta の `warn` では失敗しない

## Synthesis（design-synthesis）
- **Generalization**: 「PR・push・手動・gate」の 4 つの入口は、トリガを 1 つのワークフローに束ね、`if:` で分岐しないことで 1 つの検査に一般化した（要件 6.4）。release-workflow はジョブを足すだけで済む
- **Build vs. Adopt**: すべて既存のものを採用（cargo の既定の同梱・README 自動検出・`--workspace` の一時レジストリ、cargo-deny、pasta と同じ 4 つの Action）。自作するのは中身の検査の数行の bash だけ（cargo に「このファイルが入っているか」を検査する機能が無いため）
- **Simplification**: `readme`・`include`・`exclude`・`license-file`・`documentation` を書かない。パッケージ化を 3 OS で回さない。`--locked` を使わない。`libloading` を動かさない。MSRV の CI・Dependabot は入れない

## References
- `C:\home\maz\git\pasta\Cargo.toml` — `[workspace.package]` と `[workspace.dependencies]`（path＋version）の流儀
- `C:\home\maz\git\pasta\.github\workflows\build.yml` — `workflow_call: {}` と cargo-deny job
- `C:\home\maz\git\pasta\deny.toml` — 許可ライセンス・advisories・sources の設定
- [The Cargo Book: Publishing on crates.io](https://doc.rust-lang.org/cargo/reference/publishing.html) — path 依存に version が要ること
- [The Cargo Book: The Manifest Format](https://doc.rust-lang.org/cargo/reference/manifest.html#package-metadata) — メタデータの警告
