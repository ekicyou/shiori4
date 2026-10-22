# Research & Gap Analysis — release-workflow

## Summary
- **Feature**: `release-workflow`
- **Discovery Scope**: Complex Integration（GitHub Actions・crates.io Trusted Publishing・GitHub Release・エージェントのスキル）。ただし pasta に実際に動いている同じ仕組みがあり、主な作業は「移植と縮小」
- **Key Findings**:
  - shiori4 側には、リリースに関わる資産は build.yml（`workflow_call: {}` あり）とルート `Cargo.toml` の版 3 か所だけ。`.github/scripts/`・`.github/release-ci-setup.md`・`doc/`・`.claude/skills/release/`・`.claude/settings.json` はいずれも無い。要件 1〜16 のうち、既存の資産を「使う」だけで済むのは関門（要件 2.1）と版の置き場所（要件 9.1）だけで、残りは新規
  - pasta の一式（release.yml 574 行・`.github/scripts/release/` 6 本・スキル SKILL.md 169 行＋procedure.md 504 行＋scripts 4 本・手順書 235 行・見どころの約束 73 行）のうち、pasta 固有の部分（build job・publish-vsce・Azure・Marketplace・package.json・マニュアル・Claudia の口調）が半分近くを占める。数え方は 7/7 → 4/4、版の更新は 6 ファイル → 2 ファイル（`Cargo.toml`・見どころ）に縮む
  - pasta から**そのままでは持ってこられない**前提が 3 つある: (1) `publish-crate.ps1` が `cargo publish --locked` を使う（shiori4 は lock をコミットしないので外す）、(2) 版の更新の確認が `Cargo.lock` と `cargo metadata --locked` に依存する（shiori4 では不要）、(3) 状態の判定表 S9「タグが無いのに公開済みがある → 止まる」が、shiori4 の初版 0.0.1（手動で公開済み・タグ無し）にそのまま当たる
  - crates.io の調査（2026-10-22）: Trusted Publishing のトークンは 30 分で、1 つのトークンで「設定が一致するすべてのクレート」を公開できる。新しいクレートは Trusted Publishing で作れない（初回の手動公開が必須）。`trustpub_only`（「Require trusted publishing for all new versions」）はクレートの設定画面にある。`cargo publish --workspace`（1.90 で安定化）は公開済みの版が 1 つでもあると全体が失敗し、skip しない

## Research Log

### 現状の資産（Current State）
- **Context**: 要件と既存の資産の差を洗う
- **Sources Consulted**: `Cargo.toml`、`.gitignore`、`.github/workflows/build.yml`、`.claude/`、`.kiro/steering/*`、`.kiro/specs/completed/crate-publish-ready/{design,research}.md`、`git log`
- **Findings**:
  - `Cargo.toml`: `[workspace.package] version = "0.0.1"`、`[workspace.dependencies]` の `shiori4_abi`・`shiori4` に `version = "0.0.1"`。`repository = "https://github.com/ekicyou/shiori4"`
  - `.gitignore` に `Cargo.lock`（lock はコミットしない。tech.md「公開にも `--locked` を使わない」）
  - `build.yml`: `on: push(main)・pull_request(main)・workflow_dispatch・workflow_call: {}`、トップの `permissions: contents: read`、job は `test`（3 OS）・`deny`・`package`。release-scripts の job は無い
  - `.claude/` には `skills/` だけ（kiro-* スキル）。`settings.json` は無い
  - `.github/` には `workflows/build.yml` だけ。`doc/` は無い
  - コミットの件名: `feat(crate-publish-ready): … (#3)`・`docs(steering): … (#2)`・`chore(kiro): … (#1)`・`feat(host): …`（PR 番号なし）・`Initial commit`（接頭辞なし）
  - 手元に pwsh 7.6 がある。GitHub のホストランナー（ubuntu・windows・macos）にも pwsh は入っている
- **Implications**: ほぼすべてが新規ファイル。既存ファイルへの変更は build.yml（job の追加）・steering 2 本だけ。Rust のソースには触れない

### pasta の一式と、縮める箇所
- **Context**: brief の Approach（pasta の移植と縮小）を、ファイル単位で具体化する
- **Sources Consulted**: pasta の `.github/workflows/release.yml`・`.github/scripts/release/*.ps1`・`.github/release-ci-setup.md`・`.claude/skills/release/{SKILL.md,procedure.md,scripts/*.ps1}`・`doc/release-notes/README.md`・`.claude/settings.json`・`.kiro/specs/completed/release-workflow/requirements.md`
- **Findings**（ファイルごと。◎ ほぼそのまま／○ 名前と数を直す／△ 中身を作り直す／× 持ってこない）:

  | pasta のファイル | 扱い | shiori4 で変えること |
  |---|---|---|
  | `release.yml` の verify | ○ | `package.json` の検査を外す（verify-tag.ps1 側） |
  | `release.yml` の gate | ◎ | `uses: ./.github/workflows/build.yml` のまま |
  | `release.yml` の build | × | 配布物が無い。`git diff --exit-code -- Cargo.lock` も持ってこない（brief） |
  | `release.yml` の publish-crates | ○ | 5 → 3 クレート、`needs: [verify, gate]`、outputs・集約 step の一覧、runs-on（windows である理由が無い） |
  | `release.yml` の publish-vsce | × | — |
  | `release.yml` の github-release | ○ | artifact のダウンロードを外す。添付を持たず、詳しいリリースノートを本文に置く（要件 4.5。要件ディスカッションで確定）。Title `shiori4 vX.Y.Z` |
  | `release.yml` の report | ○ | 行を 4 つに。`build`・`publish-vsce` を needs から外す |
  | `verify-tag.ps1` | ○ | 拡張の版（package.json）の検査を外す。5 条件 → 4 条件 |
  | `publish-crate.ps1` | ○ | `--locked` を外す（DryRun の表示も）。User-Agent を shiori4 に |
  | `release-notes.ps1` | ○ | 内部の作業のスコープに `kiro` を足す（要件 6.5）。`-BodyFile` の本文の形 |
  | `github-release.ps1` | △ | 添付の判定を外し「下書き → 公開」だけにする。既定の Title |
  | `publish-vsix.ps1` | × | — |
  | `release-scripts-test.ps1` | ○ | 見どころの見本から Claudia を外す。定型コマンド D の項目を 4 行に。`package.json` を作る準備を外す |
  | `release-ci-setup.md` | △ | 2〜7 節（Azure・Marketplace・確認ワークフロー）を落とす。8 節を 3 クレートに。初回の手動公開（11 節）を「初版」の節として前に出す。10 節の後片付けから `VSCE_PAT` を外す |
  | `doc/release-notes/README.md` | △ | 書き方 4〜6（Claudia の口調・出典リンク）を外す。`book/` へのリンクを外す。内部の作業の表 |
  | `.claude/skills/release/SKILL.md` | ○ | 7/7 → 4/4、6 ファイル → 2 ファイル、`release.ps1`・`npm`・`vsce` の記述を外す。「まだ実地で確かめていない読み方」の節を引き継ぐか |
  | `procedure.md` | △ | 段 4 の手順 1〜7（版の表記 5 ファイル・`npm version`・`cargo update --workspace`・`cargo metadata --locked`・`verify-content.mjs`）を `Cargo.toml` の 3 か所だけに。判定表 S0〜S9 に初版の行を足す。F5 の「配布物」を「詳しいリリースノート」に。`RELEASE.md` への参照を外す |
  | `release-state.ps1`（定型 A） | ○ | 出どころから `package.json`・Marketplace を外し、クレートの一覧を 3 つに、集計を n/4 に、GitHub Release の添付の判定を外す |
  | `ci-result.ps1`（定型 B） | ○ | `Publish pasta_*` → `Publish shiori4*` |
  | `pretag-check.ps1`（定型 C） | ○ | `package.json` の行を外す（5 行 → 4 行） |
  | `highlights-check.ps1`（定型 D） | ○ | Claudia の出典リンクの行を外す（5 行 → 4 行）。`-Repo ekicyou/pasta` |
  | `.claude/settings.json` | ○ | `mcp__github__merge_pull_request`・`git fetch`・`git merge origin/main`・`git merge --abort`・`git push -u origin HEAD`・`git push origin v*`・`gh pr create/merge`・`gh run list/view/rerun` と autoMode の説明文。「vsce publish」の文言を外す |

- **Implications**: ファイルの数は約 15 本。いずれも pasta に動く元があり、「直すべき箇所の一覧」を設計で漏れなく固定できれば、実装はほぼ機械的になる。逆に、漏れ（`--locked`・`package.json`・`pasta_*` の文字列）が残ると、初回の CI の実行で初めて気づく。設計では「pasta 固有の語の残りを grep で検出する」検査を完了条件に入れる価値がある

### crates.io Trusted Publishing と cargo publish（外部の調査）
- **Context**: brief の Constraint「1 つの job で crates-io-auth-action を複数回呼ぶことは公式文書に無い。落とし先を設計で持つ」、要件 3.2・3.3・14・15
- **Sources Consulted**（サブエージェントによる調査。2026-10-22 時点の rust-lang/crates.io・crates-io-auth-action・cargo のソースと公式ブログ）:
  - https://github.com/rust-lang/crates-io-auth-action
  - https://github.com/rust-lang/crates.io/blob/main/src/controllers/trustpub/tokens/exchange/mod.rs
  - https://github.com/rust-lang/crates.io/blob/main/src/controllers/krate/publish.rs
  - https://blog.rust-lang.org/2026/01/21/crates-io-development-update
  - https://blog.rust-lang.org/2025/09/18/Rust-1.90.0/
  - https://github.com/rust-lang/cargo/issues/13397
- **Findings**:
  - トークンの有効期限は 30 分（サーバの `ACCESS_TOKEN_LIFETIME`）。action の post step が job の終わりにトークンを失効させる
  - 交換したトークンは、リポジトリ・ワークフロー名・environment が一致する Trusted Publishing の設定を持つ**すべてのクレート**に効く。つまり 3 クレートを 1 つのトークンで公開できる
  - 同じ job で複数回呼ぶことは、文書には無い（UNVERIFIED）。ソースからは、呼ぶたびに新しい OIDC トークンを取り、それぞれの post step が自分のトークンを失効させる作りに見える（pasta も 5 回呼んで動いている）
  - 新しいクレートは Trusted Publishing では作れない（403「Trusted Publishing tokens do not support creating new crates. Publish the crate manually, first」）。Trusted Publisher の設定も、既にあるクレートにしか足せない
  - `trustpub_only` はクレートの設定画面の「Require trusted publishing for all new versions」。有効にするとトークンでの公開を拒む。Trusted Publisher が 1 つも無いと公開できなくなる。変えるたびに監査ログと owner へのメールが出る
  - ワークフローのファイル名は完全一致で照合される。environment は設定に入れたときだけ照合される（入れなければどの environment でも通る）
  - `cargo publish --workspace` は Rust 1.90 で安定化。公開済みの版が 1 つでもあると、何も上げずに全体が失敗する（skip しない。`--idempotent` の要望は未解決）
- **Implications**:
  - 認証は「job の先頭で 1 回だけ取り、3 つの publish に渡す」形で足りる見込み。3 クレートは小さく、30 分の内に収まる見込みが高い（ただし publish-crate.ps1 の待ちの上限は「索引の反映待ち 5 分＋再試行 3 分＋公開後の確認 5 分」でクレートあたり最大 13 分ほどになり、3 クレートの最悪は 30 分を超え得る）。時間切れは「失敗した job の再実行」で新しいトークンを取り直して続けられる（公開済みは `skipped`）。どちらを採るか（pasta どおりクレートごとに取る／1 回だけ取る）は設計で決める（Research Needed）
  - `cargo publish --workspace` は再実行で skip しないので、要件 3.4（公開済みは `skipped`）を満たすには、pasta の 1 クレートずつの形（`-p` と、事前の公開済み判定）を残す
  - 初版の CI の実行では 3 クレートとも `skipped` になる。publish-crate.ps1 は公開済みの判定を認証なしの API で行うので、`skipped` の経路では crates.io に対する「公開の権限」は試されない。試されるのはトークンの交換（auth step）だけ。要件 14.5（確定事項 3）はこの事実に基づく

### 初版 0.0.1 の流れ
- **Context**: brief の Constraint「初版 0.0.1 は手動で出し、CI の最初の実行は全部 skipped の確認になる見込み（手順書で扱う）」、要件 14
- **Findings**:
  - main の版はすでに 0.0.1。版の更新（`Cargo.toml` の書き換え）は要らない。必要なのは `doc/release-notes/v0.0.1.md` を main に入れることと、タグ `v0.0.1` の push だけ
  - pasta の判定表で当たる行: 手動公開の前なら S7（タグ無し・0/7）だが、手動公開の後は「3/4 公開済み・タグ無し」で S9（止まる）。初版の行を足す必要がある
  - 手動公開と Trusted Publisher の登録の順: 設定は既にあるクレートにしか足せないので、「手動公開 → Trusted Publisher の登録 → タグの push」の順が必須。`trustpub_only` は Trusted Publisher の登録の後でないと出ない
  - 手動公開の順も依存の順（`shiori4_abi` → `shiori4` → `shiori4_host`）。`cargo publish -p shiori4` は、`shiori4_abi 0.0.1` が索引に現れるまで解決できない（crate-publish-ready の実験 E2）。cargo 1.90 以上なら `cargo publish --workspace` 1 回で依存の順に出せる（初回は全部未公開なので skip の問題は無い）
  - release-notes.ps1 は前のタグが無いとき全履歴を使い、`**Full Changelog**` の行を出さない。pasta の段 4 H1 の確認（「最後の行が compare のリンク」）は初版では当たらないので、初版の扱いを procedure に書く必要がある
  - 初版の材料には `Initial commit`（接頭辞なし → 「そのほか」）と `feat(host): …`（PR 番号なし）が入る。見どころの PR 番号の検査（定型 D）が PR 番号の無い見どころを許すか、確かめる必要がある
- **Implications**: 初版は「版の更新を飛ばし、見どころだけを PR で入れてタグを押す」特別な経路になる。要件ディスカッションで、スキルに初版の判定を 1 か所にまとめて持たせ（要件 14.6）、`v0.0.1.md` は実装の PR に含めると確定した。初版の経路は v0.0.1 の完了後に release-bootstrap-cleanup が撤去する

## Requirement-to-Asset Map

| 要件 | 既存の資産（shiori4） | 移植元（pasta） | ギャップ | 種別 |
|---|---|---|---|---|
| 1 起動とタグの検査 | なし | release.yml verify・verify-tag.ps1 | 新規。`package.json` の検査を外す | Missing |
| 2.1–2.2 関門 | build.yml（`workflow_call: {}`） | release.yml gate | 呼ぶだけ。build.yml の top-level `permissions: contents: read` と呼び出し側の権限の整合を確かめる | Constraint |
| 2.3–2.4 補助スクリプトのテスト | なし | build.yml の release-scripts job・release-scripts-test.ps1 | build.yml に job を 1 つ足す（既存 job・トリガは変えない）。テストの項目を縮める | Missing |
| 3 crates.io への公開 | `[workspace.dependencies]` の版要求 | publish-crates・publish-crate.ps1 | 新規。`--locked` を外す。認証を 1 回にするか 3 回にするか | Missing / Unknown |
| 3.3 environment の制限 | なし | 手順書 4 節 | GitHub の設定（開発者が行う） | Missing（手順書） |
| 4 GitHub Release | なし | github-release・github-release.ps1・release-notes.ps1 | 添付を持たない（確定）。添付の判定を外す | Missing / Unknown |
| 5 結果の報告 | なし | report job・各 job の集約 step | 行を 4 つに | Missing |
| 6 見どころの約束 | なし（`doc/` が無い） | doc/release-notes/README.md | 口調の決まりを外す。内部の作業のスコープ（未決事項 5） | Missing |
| 7–13 リリーススキル | kiro-* スキルのみ | `.claude/skills/release/` 一式 | 新規。7/7 → 4/4、版の更新 2 ファイル、判定表に初版の行 | Missing |
| 9.1 版の 3 か所 | ルート `Cargo.toml` の 3 か所（crate-publish-ready の契約） | procedure 段 4 | `Cargo.lock`・`cargo metadata --locked` の確認は不要。3 か所が同じ版であることの確認に置き換える | Constraint |
| 14 初版 0.0.1 | 版はすでに 0.0.1 | なし（pasta は既存クレートに CI を足した） | 判定表の新しい行。見どころだけの PR | Missing / Unknown |
| 15 手順書 | なし | release-ci-setup.md | Azure・Marketplace の節を落とし、初版の手動公開を前に出す | Missing |
| 16.1–16.5 steering | workflow.md（「公開の運用の節は release-workflow が足す」）・tech.md（公開とバージョン・CI） | pasta の workflow.md のリリースの節 | 書き換え | Missing |
| 16.6 操作の許可 | `.claude/settings.json` 無し | pasta の settings.json | 新規作成。`kiro-complete` の `gh pr merge` の許可も同じファイルに入る点に注意 | Missing |

## Implementation Approach Options

### Option A: pasta の一式をファイルごとに写し、pasta 固有の部分を削る（Port & Trim）
- 各ファイルを pasta から写し、上の表の「変えること」だけを直す。release.yml の job 構成（verify → gate → publish-crates → github-release → report）、status 契約（`status=`・`reason=`・`url=`）、スクリプトの引数、定型コマンド A〜D の出力の形を pasta と同じに保つ
- ✅ 開発者が pasta で慣れた形・読み方のまま運用できる。pasta で実際に公開できている
- ✅ procedure.md と release.yml の step 名・出力の形の対応（スキルの「このスキルを直すとき」）が pasta と同じなので、片方を直したときにもう片方へ移しやすい
- ❌ pasta の 5 クレート・3 配布物を前提にした作り（クレートごとの auth・outputs の列挙・添付 3/3）が、3 クレート・添付 1 つには過剰に残る
- ❌ 写し漏れ（`--locked`・`package.json`・`pasta_*`）が初回の CI まで見つからない危険

### Option B: 仕組みを shiori4 用に作り直す（Rewrite）
- 要件から job・スクリプトを設計し直す。例: 認証は 1 回だけ、publish はクレートの一覧をループする 1 step、GitHub Release は添付なしで本文に詳しいリリースノートを直接置く
- ✅ 3 クレートに合った小さな形になり、読む量が減る
- ❌ pasta で実地に確かめた挙動（冪等性・status 契約・再実行の読み方）を作り直すことになり、確かめ直しが要る
- ❌ 開発者が pasta と shiori4 で 2 つの形を覚えることになる（brief は「pasta と同じく」を望んでいる）

### Option C: 骨格は pasta を写し、3 クレート用に効くところだけ単純にする（Hybrid）
- job 構成・status 契約・スクリプトの引数・定型コマンドの出力の形・判定表の構造は pasta のまま
- 単純にする候補（設計で 1 つずつ判断する）: 認証を job の先頭の 1 回だけにする（brief の「1 回の認証を共有する形」の落とし先を最初から採る）、publish-crates の runs-on を ubuntu にする、添付の判定を 1 つにする（または添付をやめる）
- 足すもの: 判定表に初版の行、手順書に初版の節
- ✅ pasta との対応を保ちつつ、shiori4 で意味の無い部分を持ち込まない
- ❌ どこを単純にしたかを design.md に明記しないと、pasta との差分が追えなくなる

## Implementation Complexity & Risk

- **Effort: M（3〜7 日）** — ファイル数は約 15 本と多いが、すべてに動く元がある。新しく考えるのは初版の経路と、縮めたときの整合（判定表・status 契約・テスト項目）
- **Risk: Medium** — 外部サービス（crates.io・GitHub）との結合は、実際にタグを push しないと確かめられない。初版の CI の実行は全部 `skipped` なので、CI からの公開の経路（auth の後の `cargo publish`）が初めて走るのは 0.0.2。そこで失敗しても、`failed` → 再実行で回復でき、取り消しの要る事故にはなりにくい

## Recommendations for Design Phase

- **推奨**: Option C（Hybrid）。brief の「pasta の移植と縮小」に沿い、pasta との対応表（pasta のファイル → shiori4 のファイル → 変えた点）を design.md に持つ
- **設計で決めること**:
  1. 認証を 1 回だけにするか、pasta どおりクレートごとに取るか（brief が設計に委ねた点。トークンは 3 クレート共通で効く。待ちの上限の合計と 30 分の関係）
  2. publish-crates の runs-on（pasta は配布物のビルドの都合で windows。shiori4 では理由が無い）
  3. 添付を持たない形での「公開済み」の判定（定型 A・github-release.ps1・F5 の縮め方）
  4. 判定表 S0〜S9 への初版の行の入れ方と、初版の見どころの PR（版の更新を伴わないコミットの件名・段 1 の「版の更新のコミットだけが残っている場合」の判定との関係）
  5. 補助スクリプトのテストの項目（pasta の 9 項目から、口調・`package.json` の項目を外し、初版（前のタグ無し）の項目を足すか）
  6. pasta 固有の語（`pasta`・`package.json`・`vsce`・`Claudia`・`--locked`・`Cargo.lock`）の残りを検出する完了条件
  7. `.claude/settings.json` に kiro-complete 用の `gh pr merge` の許可も入れるか（pasta は同じファイルに入れている）
- **Research Needed**:
  - 同じ job での crates-io-auth-action の複数回の呼び出し（UNVERIFIED。1 回にするなら不要）
  - environment の名前の大文字小文字の照合（GitHub の経路は完全一致の見込み。手順書では小文字の `release` に統一すれば足りる）
  - 定型 D の「PR の番号」の検査が、PR を通らなかった変更（`feat(host): …`・`Initial commit`）だけを材料にする初版で、どう判定されるか
