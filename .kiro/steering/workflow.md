# 開発ワークフロー

Kiro 仕様駆動開発における作業フローと完了基準。`kiro-complete` スキルはこのファイルを権威として読み、ここに書いたゲートと手順を順に実行する。

> **関連ドキュメント**: [CLAUDE.md](../../CLAUDE.md) - プロジェクト指示、コマンド一覧

## 前提: このリポジトリの完成の形

完成形は、ワークスペースの 3 クレート（`shiori4_abi`・`shiori4`・`shiori4_host`）を **crates.io の公開クレートとして CI から公開すること**である（[roadmap.md](roadmap.md)）。
公開は**リリースタグ（`vX.Y.Z`）の push** を合図に CI（`.github/workflows/release.yml`）が行う。開発者は `/release`（`.claude/skills/release/`）で、版の決定と更新・版の更新の PR・タグの作成と push・CI の結果の見届けまでを行う。手元から `cargo publish` しない（初回の手動公開と緊急時の手順は `.github/release-ci-setup.md`）。
各 spec の完成は、公開とは切り離して「3 クレートがビルドでき、テストと clippy が通ること」（下の DoD）で判定する。

---

## 仕様フェーズ

```
requirements → design → tasks → implementation → implementation-complete
```

| コマンド                            | 用途       |
| ----------------------------------- | ---------- |
| `/kiro-spec-init "description"`     | 仕様初期化 |
| `/kiro-spec-requirements {feature}` | 要件定義   |
| `/kiro-spec-design {feature} [-y]`  | 設計生成   |
| `/kiro-spec-tasks {feature} [-y]`   | タスク分解 |
| `/kiro-impl {feature} [tasks]`      | 実装       |
| `/kiro-spec-status {feature}`       | 進捗確認   |
| `/kiro-complete {feature}`          | 完了承認後のアーカイブ・PR・マージ |

---

## 完了基準（DoD）

すべて同時に満たすこと：

1. **Spec Gate**: requirements / design / tasks が承認済みで、`tasks.md` の全タスクが `[x]`
2. **Test Gate**: ワークスペースのルートで次の 2 つがともに成功する
   ```powershell
   cargo test --all                                    # ビルド＋全テスト
   cargo clippy --all-targets --workspace -- -D warnings
   ```
   リリースの補助スクリプト（`.github/scripts/release/`・`.claude/skills/release/scripts/`）を変えたときは、次も成功すること
   ```powershell
   pwsh -NoProfile -File .github/scripts/release/release-scripts-test.ps1
   ```
3. **Doc Gate**: 仕様差分を下の「ドキュメント保守」の該当ドキュメントに反映した
4. **Steering Gate**: 変更が既存ステアリング（`product.md`・`tech.md`・`structure.md`・本ファイル）と矛盾しない。矛盾するなら、ステアリングを直すか実装を直す

いずれかが失敗したら完了を中断し、開発者に報告する。

---

## 実装完了時アクション

### 1. コミット
```powershell
git add -A; git commit -m "<type>(<scope>): <summary>"
```
- タイプ: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`
- スコープ: spec の名前、またはクレート名（`abi`・`host` など）
- 本文は日本語の箇条書き

### 2. スキルドキュメント更新検討

当該 spec の変更領域が下の表に当たるときは、対象スキルの記述と食い違いが無いかを確かめて直す。当たらなければこの手順はスキップする。

| 変更領域 | 対象スキル |
| -------- | ---------- |
| リリースの仕組みの変更（release.yml・補助スクリプト・公開対象のクレート・版の置き場所・見どころの約束） | `.claude/skills/release/`（SKILL.md・procedure.md・scripts） |
| 開発フローの変更（本ファイル・`CLAUDE.md`） | 影響を受ける `kiro-*` スキル |

- ドメイン固有のスキルを足したら、この表に行を足す

### 3. リモート同期（ブランチ戦略）

リモート同期は **PR（Pull Request）ベース**で行う。これが唯一の手順実体であり、各スキルはこのフローを複製せず本セクションを参照する。

> フィーチャーブランチ／ワークツリーは Claude Code（ハーネス）のワークツリー機能が供給する。スキルは自前でブランチ／ワークツリーを作らず、「commit → PR 作成 → squash マージ → ブランチ削除」だけを担う。1 feature = 1 ブランチ = 1 PR。**デフォルトブランチ（main）への直接 push は行わない。**

```powershell
$branch = git rev-parse --abbrev-ref HEAD   # ハーネス供給の作業ブランチ
gh pr create --base {default-branch} --head $branch --title "<subject>" --body "<body>"
gh pr merge --squash --delete-branch --subject "<subject>" --body "<body>"
```

- `{remote}`（`origin` → 単一リモート → none）と `{default-branch}`（`git symbolic-ref` → `main` → `master` → 現ブランチ）は決定的に解決し、ハードコードしない
- **CI を待たない**: PR では CI（`.github/workflows/build.yml`）が走るが、その結果は待たない。マージ前の関門は DoD の Test Gate（ローカルの `cargo test --all` と clippy、補助スクリプトを変えたときはそのテスト）である
- **版の更新の PR**: `/release` が作る版の更新の PR（`chore(release): vX.Y.Z`）も、同じ流れ（PR・squash・CI を待たない）で統合する
- **直接 push の禁止の例外**: リリースタグの ref の push（`git push origin vX.Y.Z`）だけである。`.claude/settings.json` の allow にあるのはリリースタグの push（`git push origin v*`）と作業ブランチの push（`git push -u origin HEAD`）だけで、main への push・強制 push・タグの付け直しの項目は無い。タグの push の glob は後ろのオプションや refspec まで合うので、強制 push（`-f`・`--force`・`+`）・削除（`-d`・`--delete`）・refspec の push（`:refs/`・`:…main…`・`:v…`）・タグの付け直し（`git tag … -f`・`-d`）は `ask` の規則で毎回確認させる（`ask` は `allow` より先に当たる）。規則は書かれたコマンドの文字列に当たるだけなので、別の形の呼び出しまでは縛らない
- **ブランチ削除**: リモートの feature ブランチは `--delete-branch` がマージ成功**後に** API で消す。ローカルブランチ・ワークツリーの後始末はハーネスに委ねる（ローカル削除の警告は非致命）
- **フォールバック（PR 不可）**: デフォルトブランチ上 / `{remote}` が none / `gh` 未認証のときは、警告して PR・push をスキップし、ローカルコミットを保持する
- **中断条件**: PR の作成またはマージ（API）が失敗したら、ブランチを消さずに中断して報告する

#### squash コミットメッセージの生成方針

1. `git log --no-merges --pretty=format:"%h %s%n%b" {default-branch}..HEAD` で分岐点以降のコミットを集める
2. 対象 spec の `requirements.md` / `design.md` の概要で意図を補う
3. subject は `<type>(<scope>): <機能全体を 1 文で>`、body は「何を・なぜ」の開発単位で 3〜7 項目。fixup / typo / WIP は畳む

### 4. 仕様アーカイブ

spec ディレクトリを `.kiro/specs/completed/` へ移し、**移した後で** `spec.json` の `phase` を `"completed"` にする（移動前に編集すると、エディタがファイルを元の場所に復活させることがある）。

繰り返し実行型の spec（`/kiro-impl` のたびにタスクをリセットするもの）は移動しない。

### 5. ロードマップ

`.kiro/steering/roadmap.md` があり、完了した spec がその一覧に載っていれば `- [x]` にする。roadmap.md がまだ無ければスキップする（`/kiro-discovery` で複数 spec の計画を立てたときに作られる）。

---

## タスク生成ルール

`/kiro-spec-tasks` は、すべての実装タスクの後に次の最終タスクを**必ず**加える（削除・省略禁止）。

```markdown
**Task: ドキュメント整合性の確認と更新**

1. [ ] クレート冒頭の `//!` - 規約・設計方針の変更を反映（該当する場合）
2. [ ] `shiori4_abi/include/shiori4_abi.h` - ABI の型・定数・規約の変更を Rust 側と揃える（該当する場合）
3. [ ] README.md - クレートの役割・使い方の入口の変更を反映（該当する場合）
4. [ ] steering/* - 該当領域のステアリング更新
```

- 実装タスクは具体的かつテスト可能な粒度に分け、各タスクに完了条件を書く
- 依存関係の順に並べる（テストファースト推奨）

---

## 回帰責任（Regression-First Fix）

- **同一 PR で修正**: 既存テストが落ちたらマージ前に直す
- **原因特定**: 最小再現を特定し、根本原因を直す
- **テスト更新**: 挙動変更が正当なら、テストを先に更新して理由を書く

---

## 禁止事項

### MVP 禁止
完成宣言に「MVP」「部分実装」「スキャフォールドのみ」「とりあえず動く」を使わない。
使う表現: 「全テスト合格」「DoD Gate 通過」「追加タスク待ち（未完成）」

### 危険な Git 操作の禁止

複数セッションが同じリポジトリで並走するため、他の作業を巻き込む破壊的操作をしない。

| 禁止コマンド             | 理由                                   | 代替手段                                       |
| ------------------------ | -------------------------------------- | ---------------------------------------------- |
| `git revert <commit>`    | 他セッションの未コミット作業を巻き込む | `git show <commit>` で差分確認後、手で逆変更してコミット |
| `git reset --hard`       | 未コミット変更を完全消去               | `git status` で確認し、WIP コミットで退避      |
| `git checkout -- <file>` | ファイル単位の強制破棄                 | `git diff <file>` で確認後、エディタで手動修正 |
| `git clean -fd`          | 未追跡ファイルの一括削除               | `git clean -fdn` で確認後、個別削除            |

- コミット前に必ず `git status` を確かめる
- 疑問があれば開発者に確認する

---

## ドキュメント保守

### 更新チェックリスト

| 変更種別                     | 更新対象ドキュメント |
| ---------------------------- | -------------------- |
| ABI の規約・型・定数の変更   | `shiori4_abi/src/lib.rs` 冒頭の `//!`、`shiori4_abi/include/shiori4_abi.h`、steering/tech.md（ABI 規約） |
| スレッド規約の変更           | 各クレート冒頭の `//!`（規約番号の参照も）、steering/tech.md |
| 公開 API の変更              | 該当クレート冒頭の `//!` の使用例、README.md（使い方の入口） |
| クレートの追加・層の変更     | README.md（クレート表）、steering/structure.md、steering/tech.md |
| 依存関係の変更               | steering/tech.md |
| 開発フローの変更             | steering/workflow.md、CLAUDE.md |
| リリースの仕組みの変更（release.yml・補助スクリプト・公開対象のクレート・版の置き場所・見どころの約束） | `.claude/skills/release/`（SKILL.md・procedure.md・scripts）、`.github/release-ci-setup.md`、`doc/release-notes/README.md`（見どころの約束）、steering/tech.md（公開とバージョン・CI） |

### 保守ルール

1. **ABI を変えたら**: Rust 側の `//!` と C ヘッダのコメントを**両方**直す。片方だけの更新は Doc Gate 不通過
2. **既存の利用者を壊す変更**（公開 API・送受信するバイト列・ABI のレイアウト）は、開発者の裁定を得てから行う
3. **PR 時確認**: ドキュメント更新漏れがないかを DoD チェックリストで確かめる
