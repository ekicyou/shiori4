# Roadmap

## Overview

完成形は、ワークスペースの 3 クレート（`shiori4_abi`・`shiori4`・`shiori4_host`）を **crates.io の公開クレートとして公開すること**である。
公開は pasta と同じく **CI（タグ push で走る `release.yml`）が行い**、開発者は `/release` スキルで版の更新・PR・リリースタグの push・CI の見届けを行う。手元からは `cargo publish` しない（初回の 1 回を除く）。

pasta（`C:\home\maz\git\pasta`）のリリースの仕組みを下敷きにし、バイナリ・ゴースト・VSCode 拡張・Azure・マニュアルなど pasta 固有の部分を落として 3 クレート用に縮める。

## Approach Decision
- **Chosen**: pasta の仕組みの移植（縮小版）。`build.yml`（PR の CI、`workflow_call` でリリースの gate にも使う）＋ `release.yml`（verify → gate → publish-crates → github-release → report）＋ 冪等な公開スクリプト ＋ `/release` スキル
- **Why**: 開発者が pasta で運用に慣れている。crates.io の Trusted Publishing（`rust-lang/crates-io-auth-action`）で秘密のトークンを持たずに公開でき、同じ版の再実行は `skipped` で冪等になる。pasta で実際に公開できている
- **Rejected alternatives**:
  - release-plz / cargo-release: 開発者が pasta と同じ運用を望んだため
  - 手元から `cargo publish`: 公開の前の検査（タグと版の一致・main からの到達・CI 通過）を人の注意に頼ることになるため

## Scope
- **In**: 公開できる形への Cargo.toml・README・ライセンスの整備、PR の CI、タグで動くリリース CI、GitHub Release とリリースノート、`/release` スキル、初回セットアップの手順書
- **Out**: バイナリの配布、docs.rs 以外のドキュメントサイト、crates.io 以外のレジストリ、リリースの自動化（タグ push は `/release` で人が起こす）

## Constraints
- 初版は **0.0.1**。3 クレートは**同じ版**（`[workspace.package] version`）で揃えて出す
- ライセンスは **MIT だけ**（`[workspace.package] license = "MIT"`。各クレートにルートの LICENSE の写しを同梱する）
- PR の CI は **ubuntu / windows / macos** の 3 OS で test と clippy を通す（`shiori4_host` は .dll / .so / .dylib を読む前提のため）
- 公開の順は依存の順: `shiori4_abi` → `shiori4` → `shiori4_host`
- Trusted Publishing はクレートを新規に作れない。**初回の 1 回だけ、開発者が手元からトークンで公開する**（手順書に書く。スキルや CI は代行しない）
- main への直接 push はしない。push してよいのは作業ブランチとリリースタグだけ
- リポジトリは公開（`ekicyou/shiori4`）。GitHub の environment の保護規則は公開リポジトリなので GitHub Free で使える

## Boundary Strategy
- **Why this split**: 「公開できる形のクレートと、それを守る PR の CI」と「タグから公開までの仕組み」で責務が分かれる。前者が終われば、手元で `cargo publish --dry-run` が通り、PR ごとに 3 OS で検査される状態になる。後者はその上に載るだけで、クレートの中身に触れない
- **Shared seams to watch**:
  - `build.yml` は ① が作り、② の `release.yml` が `workflow_call` で gate として呼ぶ。① は `on: workflow_call: {}` を最初から入れておく
  - 版の置き場所（`[workspace.package] version` と `[workspace.dependencies]` の内部依存の `version`）は ① が決め、② の `/release` が書き換える。② は ① の決めた場所と個数を前提にする
  - `.kiro/steering/workflow.md` の「前提: 完成の形」と DoD・リモート同期の節は ② が公開の運用に合わせて書き直す

## Specs (dependency order)
- [x] crate-publish-ready -- 3 クレートを crates.io に出せる形にし（版の一本化・path+version 依存・メタデータ・各クレートの README・ライセンス整合・`cargo package` の通過）、PR の CI（build.yml、3 OS・cargo-deny）で守る。Dependencies: none
- [ ] release-workflow -- タグ push で検査・公開・GitHub Release を行う release.yml と公開スクリプト、初回セットアップの手順書、リリースノートの約束、`/release` スキル、workflow.md の更新。Dependencies: crate-publish-ready
- [ ] release-bootstrap-cleanup -- v0.0.1 を 4/4 で出し終えた後、`/release` と手順書から初版のためだけの判定・経路を削る。Dependencies: release-workflow（と v0.0.1 のリリースの実施）
