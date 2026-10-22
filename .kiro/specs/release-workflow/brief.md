# Brief: release-workflow

## Problem
3 クレートを crates.io に公開したいが、手元からの `cargo publish` は検査（タグと版の一致・main からの到達・CI 通過・依存の順）を人の注意に頼ることになる。開発者（ekicyou）は pasta と同じく、CI に公開させ、`/release` スキル 1 つで版の更新からリリースの見届けまでを済ませたい。

## Current State
- crate-publish-ready が終われば、3 クレートは `cargo package` が通り、build.yml が PR で 3 OS の検査をしている
- タグで動くワークフロー・公開スクリプト・`/release` スキル・リリースノートの置き場所は無い
- pasta（`C:\home\maz\git\pasta`）には実際に動いている同じ仕組みがある:
  - `.github/workflows/release.yml`: verify → gate（build.yml を `workflow_call`）→ build → publish-crates ∥ publish-vsce → github-release → report
  - `.github/scripts/release/`: `verify-tag.ps1`・`publish-crate.ps1`（冪等。published / skipped / failed と reason）・`release-notes.ps1`・`github-release.ps1`（下書き → 添付 → 公開、冪等）・`release-scripts-test.ps1`
  - `.github/release-ci-setup.md`: environment `release`、crates.io の Trusted Publisher 登録、初回の手動公開、`trustpub_only`
  - `.claude/skills/release/`: `SKILL.md`・`procedure.md`（段 1〜9、状態表 S0〜S9、失敗表 F0〜F7）・`scripts/`（`release-state.ps1`・`ci-result.ps1`・`pretag-check.ps1`・`highlights-check.ps1`）
  - `doc/release-notes/`（README.md の約束と `vX.Y.Z.md` の見どころ）、`.claude/settings.json` のリリース用の許可
- `.kiro/steering/workflow.md` は「公開・タグ・CI は持たない」と書いている（この spec で書き直す）

## Desired Outcome
- `vX.Y.Z` のタグ push で release.yml が走り、タグと `[workspace.package] version` の一致・main からの到達・見どころファイルの存在を検査し、build.yml を gate として通したうえで、`shiori4_abi` → `shiori4` → `shiori4_host` の順に Trusted Publishing で公開し、GitHub Release を作る
- 同じタグでの再実行は、公開済みのクレートを `skipped` にして残りだけを進める
- `/release [X.Y.Z]` で、版の決定 → 版の更新の PR（squash）→ タグの push → CI の見届け・失敗した job の再実行 → 報告までを、実際の状態から続きを判定しながら行える
- 初回セットアップ（environment・Trusted Publisher・初回の手動公開・`trustpub_only`）の手順書があり、開発者がその通りに一度だけ行えば、以後は `/release` だけで公開できる

## Approach
pasta の仕組みを移植し、3 クレート用に縮める。落とすもの: build job（dll・nar・vsix）、publish-vsce と Azure 一式、release-setup-check.yml、manual.yml、版の出どころのうち package.json・book・Marketplace、見どころのクラウディア口調の検査。数え方は pasta の 7/7 から **3 クレート ＋ GitHub Release = 4/4** に変わる。版の更新が触るファイルは Cargo.toml・（方針しだいで）Cargo.lock・`doc/release-notes/vX.Y.Z.md`。

## Scope
- **In**:
  - `.github/workflows/release.yml` と `.github/scripts/release/*.ps1`（スクリプトのテストを build.yml の job に足すことを含む）
  - `.github/release-ci-setup.md`（初回セットアップの手順書。初回の手動公開の手順を含む）
  - `doc/release-notes/README.md`（見どころの約束）と、初版 `v0.0.1.md` の書き方
  - `.claude/skills/release/`（SKILL.md・procedure.md・scripts）と `.claude/settings.json` のリリース用の許可
  - `.kiro/steering/workflow.md` の「前提: 完成の形」・DoD・リモート同期・ドキュメント保守の更新、`tech.md` のリリースの記述
- **Out**:
  - Cargo.toml のメタデータ・README・build.yml の本体（crate-publish-ready が持つ）
  - 初回セットアップと初回の手動公開の**実行**（開発者が手順書に従って行う）
  - リリースの自動化（定期・main への push での自動リリース）

## Boundary Candidates
- CI 側: タグからの検査・公開・GitHub Release（release.yml と scripts）
- 人の側: 版の決定から見届けまでの手順（`/release` スキル）
- 一度だけの準備（release-ci-setup.md）

## Out of Boundary
- クレートの API・中身
- build.yml の検査項目そのもの（呼ぶだけ。足すのはスクリプトのテストの job だけ）

## Upstream / Downstream
- **Upstream**: crate-publish-ready（build.yml の `workflow_call`、版の置き場所、`cargo package` が通ること）、pasta の同名ファイル一式
- **Downstream**: 以後のすべてのリリース

## Existing Spec Touchpoints
- **Extends**: なし
- **Adjacent**: crate-publish-ready（build.yml と版の置き場所が継ぎ目）

## Constraints
- 初版 0.0.1。3 クレートは同じ版
- 公開は Trusted Publishing（`rust-lang/crates-io-auth-action@v1`、environment `release`、ワークフロー名 `release.yml`）。リポジトリに秘密のトークンを置かない
- Trusted Publishing はクレートを新規に作れないので、3 クレートとも初回だけ開発者がトークンで手動公開する。そのため初版 0.0.1 は手動で出し、CI の最初の実行は「全部 skipped」の確認になる見込み（手順書で扱う）
- 1 つの job で crates-io-auth-action を複数回呼ぶことは公式文書に無い（pasta も注記している）。落とし先（1 回の認証を共有する形）を設計で持つ
- main へ直接 push しない。手元から `cargo publish`（初回を除く）・`gh release create`・`cargo yank` をしない
- スクリプトは pasta と同じく PowerShell（pwsh）
