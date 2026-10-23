# Brief: release-bootstrap-cleanup

## Problem
release-workflow は、手で公開した初版 0.0.1 にタグと GitHub Release を付けるため、`/release` に「初版の状態」の判定と経路を持たせる（release-workflow の要件 14）。この経路が使われるのは 0.0.1 の 1 回だけである。そのまま残ると、以後のすべてのリリースで、開発者もエージェントも読む必要のない判定表の行・スクリプトの分岐・テストの項目を読み続けることになる。また「タグが無いのに公開済みがある」状態を止めるはずの判定に、例外が 1 つ残り続ける。

## Current State
- release-workflow の完了後、初版の経路は次の場所にある（release-workflow の要件 14.6 により、1 か所にまとめられ、初版のためだけのものと明記されている）:
  - `.claude/skills/release/procedure.md` の状態の判定表の初版の行と、その説明
  - `.claude/skills/release/scripts/release-state.ps1` の初版の判定
  - 補助スクリプトのテストのうち、初版の判定の項目
  - 要件 12.6 にあたる「どの判定にも当たらない状態」の、初版の例外
  - `.github/release-ci-setup.md` の、0.0.1 に固有の記述（初版の手動公開の後にタグを push する流れ、初版の確認）
- 撤去してよいのは、v0.0.1 が 4/4（3 クレート ＋ GitHub Release）で公開済みになった後である

## 申し送り（release-workflow より）
- 上の Current State は release-workflow の着手前の見込みで、実装とずれている。実際の初版の経路の置き場所は次のとおり（`git grep -n '【初版のみ】'` で見つかる）:
  - `.claude/skills/release/procedure.md` の判定表 S の行 `S2【初版のみ】` と、ファイルの末尾の節「初版の経路【初版のみ】」
  - `.claude/skills/release/SKILL.md` の段 2 の入口の 1 行
  - `.github/release-ci-setup.md` の節「【初版のみ】初版 0.0.1 の手動公開」と「【初版のみ】初版 0.0.1 のリリース」
- 印の無い 0.0.1 に固有の記述もある。`.github/release-ci-setup.md` の「位置づけと全体の流れ」の手順 2・3・5、冒頭の「初版の手動公開に使う API トークン」の言い回し、節「後片付け（必須）」の冒頭、Trusted Publisher の節の「手動公開の後に行う」。grep では出ないので読んで直す
- `release-state.ps1`（定型 A）と補助スクリプトのテストは初版の判定を持たない（設計どおり）。削る対象にならない
- 撤去の後は S2 の状態が S9 に当たって止まる（release-workflow の design.md「初版の経路【初版のみ】」の撤去の後）
- v0.0.1 は 2026-10-23 に 4/4（3 クレート skipped・GitHub Release published、リリース CI 39888963308）で出し終えた。撤去の前提は満たしている

## Desired Outcome
- `/release` の判定と経路に初版のためのものが無く、「タグが無いのに公開済みがある」状態は例外なく止まる
- 手順書は、新しいクレートを足すときの初回の手動公開（release-workflow の要件 15.7）を残したまま、0.0.1 に固有の流れを持たない
- 撤去の後も、補助スクリプトのテスト・PR の CI・`cargo test --all`・clippy が通る

## Approach
release-workflow が 1 か所にまとめた初版の経路を、その印を頼りに削る。新しい仕組みは足さない。v0.0.1 が 4/4 で公開済みであることを、作業の前に確かめる（GitHub Release・crates.io の 3 クレート・タグ）。

## Scope
- **In**:
  - リリーススキル（SKILL.md・procedure.md・scripts）から初版の判定と経路を削る
  - 補助スクリプトのテストから初版の項目を削る
  - 手順書から 0.0.1 に固有の流れを削り、新しいクレートを足すときの手動公開の節は残す
  - 撤去の前に v0.0.1 が 4/4 で公開済みであることを確かめる
- **Out**:
  - `doc/release-notes/v0.0.1.md`（その版の見どころとして残す）
  - release.yml と公開スクリプト（初版の経路を持たない）
  - 「Trusted Publishing のみ」の有効化と API トークンの失効（release-workflow の手順書の後片付けで、開発者が行う）

## Boundary Candidates
- リリーススキルの判定と経路の削除
- 手順書の初版の記述の整理

## Out of Boundary
- リリースの仕組みの挙動の変更（初版の経路を削るほかは、release-workflow の要件のとおりに保つ）
- 新しいクレートの追加の手順の変更

## Upstream / Downstream
- **Upstream**: release-workflow（初版の経路を 1 か所にまとめて作る。要件 14.6）。v0.0.1 のリリースの実施（開発者が手順書と `/release` で行う）
- **Downstream**: 0.0.2 以降のリリース

## Existing Spec Touchpoints
- **Extends**: なし（release-workflow が完了した後の、その成果物の整理）
- **Adjacent**: release-workflow（初版の経路の置き場所と印が継ぎ目）

## Constraints
- v0.0.1 が 4/4 で公開済みになる前に始めない
- 小さい spec。新しいファイル・仕組み・依存を足さない
