# 開発ワークフロー

Kiro 仕様駆動開発における作業フローと完了基準。`kiro-complete` スキルはこのファイルを権威として読み、ここに書いたゲートと手順を順に実行する。

> **関連ドキュメント**: [CLAUDE.md](../../CLAUDE.md) - プロジェクト指示、コマンド一覧

## 前提: このリポジトリの完成の形

完成形は、ワークスペースの 3 クレート（`shiori4_abi`・`shiori4`・`shiori4_host`）を **crates.io の公開クレートとして CI から公開すること**である（[roadmap.md](roadmap.md)）。
公開の仕組み（release.yml・`/release`）ができるまでは、各 spec の完成は「3 クレートがビルドでき、テストと clippy が通ること」で判定する。公開の運用の節は release-workflow spec がこのファイルに足す。

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

このリポジトリには、実装内容に追従させるドメイン固有のスキル（ゴースト作成ガイドなど）は**まだ無い**。`.claude/skills/` にあるのは Kiro ワークフローのスキルだけなので、通常はこの手順を**スキップ**する。

- **実施する場合**: 当該 spec が開発フロー（本ファイル・`CLAUDE.md`）を変えたときだけ、影響を受ける `kiro-*` スキルの記述と食い違いが無いかを確かめて直す
- ドメイン固有のスキルを足したら、ここに「変更領域 → 対象スキル」の表を足す

### 3. リモート同期（ブランチ戦略）

リモート同期は **PR（Pull Request）ベース**で行う。これが唯一の手順実体であり、各スキルはこのフローを複製せず本セクションを参照する。

> フィーチャーブランチ／ワークツリーは Claude Code（ハーネス）のワークツリー機能が供給する。スキルは自前でブランチ／ワークツリーを作らず、「commit → PR 作成 → squash マージ → ブランチ削除」だけを担う。1 feature = 1 ブランチ = 1 PR。**デフォルトブランチ（main）への直接 push は行わない。**

```powershell
$branch = git rev-parse --abbrev-ref HEAD   # ハーネス供給の作業ブランチ
gh pr create --base {default-branch} --head $branch --title "<subject>" --body "<body>"
gh pr merge --squash --delete-branch --subject "<subject>" --body "<body>"
```

- `{remote}`（`origin` → 単一リモート → none）と `{default-branch}`（`git symbolic-ref` → `main` → `master` → 現ブランチ）は決定的に解決し、ハードコードしない
- **CI を待たない**: PR では CI（`.github/workflows/build.yml`）が走るが、その結果は待たない。マージ前の関門は DoD の Test Gate（ローカルの `cargo test --all` と clippy）だけである
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

### 保守ルール

1. **ABI を変えたら**: Rust 側の `//!` と C ヘッダのコメントを**両方**直す。片方だけの更新は Doc Gate 不通過
2. **既存の利用者を壊す変更**（公開 API・送受信するバイト列・ABI のレイアウト）は、開発者の裁定を得てから行う
3. **PR 時確認**: ドキュメント更新漏れがないかを DoD チェックリストで確かめる
