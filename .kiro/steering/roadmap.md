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
- [x] release-workflow -- タグ push で検査・公開・GitHub Release を行う release.yml と公開スクリプト、初回セットアップの手順書、リリースノートの約束、`/release` スキル、workflow.md の更新。仕組みを main に統合した後、開発者の準備（environment・初版の手動公開・Trusted Publisher・後片付け）と `/release` による v0.0.1 のリリースまでを、この spec の中で行ってから閉じる。Dependencies: crate-publish-ready
- [x] release-bootstrap-cleanup -- v0.0.1 を 4/4 で出し終えた後、`/release` と手順書から初版のためだけの判定・経路を削る。Dependencies: release-workflow（v0.0.1 のリリースを含む）
- [ ] abi-conventions -- shiori4_abi.h の規約を、SHIORI 実装者（pasta）とベースウェア実装者（areka）の意見で締める（スレッドの意味・`SHIORI_E_WRONG_THREAD`・unload の結果の約束・log と release のスレッド・notify の後回し・load の失敗理由と 2 つ目のインスタンス・load の dir の形・v4 の Charset）。Rust 側の型と高水準層をそれに合わせる。Dependencies: none
- [ ] host-protocol -- 暫定の `host_protocol` を、ホスト側プロトコル仕様として確定させる（getproperty / setproperty・post の語彙 SEND / NOTIFY と宛先・割り込み・拒否の返し方・プロパティ名の決まり・v4 の要求ヘッダの語彙・version・OnHostComplete の届け方と Reference の並び）。pasta と areka が上流として参照する正本。Dependencies: none

## Phase 2: v4 の規約とホスト側プロトコルの確定

v0.0.1 の公開の後、SHIORI 実装者（pasta、`SHIORI4:pasta` セッション）とベースウェア実装者（areka、`SHIORI4:areka` セッション）が main 72cff0b の案を読み、意見を出した（2026-10-23）。合意した語は **shiori4 を 3 者の正本** とし、areka の spec `shiori4-api`（areka リポジトリ）は host-protocol を上流として参照する。

- **Why this split**: ABI 層（バイト列の運搬と、スレッド・所有権・寿命の規約）とプロトコル層（運ぶ中身の語彙）は、product.md の方針どおり分ける。ABI を小さく安定に保ち、語彙はその上で増やす。abi-conventions は `shiori4_abi`（と各クレートの規約の守り方）、host-protocol は `shiori4::host_protocol` と `shiori4_host` の組み込みの処理を持つ
- **Shared seams to watch**:
  - 拒否の返し方で 403 を ABI の定数に足すと決まったら、定数の追加は abi-conventions の `shiori4_abi` に入れる（host-protocol は使う側）
  - どちらも `shiori4/src/lib.rs`・`shiori4_host/src/lib.rs` に触れる。並べて進めるときは片方を先に統合する
  - property の改名（`property` → `getproperty`）は 0.0.1 の動きを壊す唯一の変更。0.0.x のうちに入れる
- **Release**: 2 本を統合した後、`/release` で 0.0.2 を出す（spec ではない）。0.0.2 は CI からの初めての公開で、1 回の認証で 3 クレートを出せるかと 401・403 の分類も確かめる
