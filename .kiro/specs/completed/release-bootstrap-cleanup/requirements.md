# Requirements Document

## Project Description (Input)
v0.0.1 を 4/4（crates.io の 3 クレート ＋ GitHub Release）で出し終えたので、release-workflow が初版 0.0.1 のためだけに持たせた判定と経路を、`/release` スキルと一度だけの準備の手順書から削る。詳しくは同じフォルダの brief.md（Problem・Approach・Scope・Out of Boundary と「申し送り（release-workflow より）」）による。撤去の対象は `git grep -n '【初版のみ】'` で見つかる 5 か所（procedure.md の判定表の行 S2 と節「初版の経路【初版のみ】」、SKILL.md の段 2 の入口の 1 行、release-ci-setup.md の 2 つの節）と、印の無い 0.0.1 に固有の記述（release-ci-setup.md の全体の流れの手順 2・3・5 など）である。release.yml・公開スクリプト・定型コマンド・補助スクリプトのテストは初版の判定を持たないので変えない。

## Introduction
release-workflow は、手で公開した初版 0.0.1 にリリースタグと GitHub Release をそろえるため、`/release` の状態の判定に「初版の状態」の行と専用の経路を持たせ、一度だけの準備の手順書に初版のための節を持たせた。v0.0.1 は 2026-10-23 に 4/4 で出し終えたので、これらは二度と使われない。本仕様はそれらを取り除き、以後のリリースで開発者とエージェントが初版のための記述を読まずに済むようにし、「タグが無いのに公開済みのものがある」状態を例外なく止める。

## Boundary Context
- **In scope**: `/release` スキル（SKILL.md・procedure.md）の初版の判定と経路の削除、一度だけの準備の手順書（`.github/release-ci-setup.md`）の初版の節と 0.0.1 に固有の記述の削除・書き直し、撤去の前の v0.0.1 の 4/4 の確認
- **Out of scope**: リリース CI（`release.yml`）・公開スクリプト・定型コマンド A〜D・補助スクリプトのテストの変更（初版の判定を持たない）、`doc/release-notes/v0.0.1.md`（その版の見どころとして残す）、前のリリースタグが無い版の詳しいリリースノートの扱い（初版に限らない一般の振る舞い）、「Trusted Publishing のみ」の有効化とトークンの失効（release-workflow で開発者が済ませた）、新しいクレートの初回公開の手順の変更
- **Adjacent expectations**: release-workflow の要件のうち、初版の経路（要件 14.2・14.4〜14.6）を除く振る舞いはそのまま保つ

## Requirements

### Requirement 1: 撤去の前の確認
**Objective:** As a 開発者, I want 初版の経路を削る前に v0.0.1 が出し終わっていることを確かめたい, so that まだ要る経路を誤って消さない

#### Acceptance Criteria
1. When 撤去の作業を始めるとき, the 作業者 shall 状態の照会（定型コマンド A）で、v0.0.1 のリリースタグがあり、crates.io の 3 クレートの 0.0.1 と GitHub Release がそろって公開済み 4/4 で、`不明` が 0 であることを確かめる。
2. If 公開済みが 4/4 でないか `不明` が 1 以上であれば, the 作業者 shall 何も削らずに止まり、照会の出力を開発者に示す。

### Requirement 2: リリーススキルから初版の判定と経路を除く
**Objective:** As a 開発者, I want `/release` の判定と手順に初版のためのものを持たせたくない, so that 以後のリリースで読む必要のない行と節が無く、異常な状態が例外なく止まる

#### Acceptance Criteria
1. The Release Skill shall 状態の判定表に初版の状態の行を持たず、ほかの行（S0・S1・S3〜S9）の条件と次の手を変えない。
2. When リリースタグが無く、公開済みが 0/4 でないとき, the Release Skill shall 初版かどうかによらず「タグが無いのに公開済みのものがある」状態と判定して止まり、公開済みの行を開発者に示す。
3. The Release Skill shall 手順書（procedure.md）に初版の経路の節を持たず、SKILL.md の段 2 の要約に初版の入口を持たない。
4. The Release Skill shall 初版の経路を指していた記述（判定表のあとの注記・段の要約・節の参照）を、残らないように除き、ほかの段と節が成り立つ形を保つ。

### Requirement 3: 一度だけの準備の手順書から 0.0.1 に固有の流れを除く
**Objective:** As a 開発者, I want 手順書を、設定をやり直すときと新しいクレートを足すときに使える形にしたい, so that 済んだ初版の手順を読まずに済む

#### Acceptance Criteria
1. The Setup Guide shall 初版 0.0.1 の手動公開の節と、初版 0.0.1 のリリースの節を持たない。
2. The Setup Guide shall 全体の流れを、0.0.1 と本仕様より前の PR に触れない形で書き、設定をやり直すとき・新しいクレートを足すときに上から順に使える順に並べる。
3. The Setup Guide shall 初回の後片付けの節を、手で公開した後にいつも行う後片付けとして残し、初版を前提にした言い回しを持たない。
4. The Setup Guide shall 新しいクレートの初回公開・緊急時の手動公開・2 つ目以降のクレートの認証だけが失敗したとき・名前の対応表の節を残し、リリース CI・公開スクリプト・リリーススキルが見出しの名前で指す節を、すべて同じ見出しで持ち続ける。
5. The Setup Guide shall 本文の中で、削った節を指す参照を持たない。

### Requirement 4: 撤去の後の整合と回帰
**Objective:** As a 開発者, I want 撤去の後もリリースの仕組みが変わらず動くことを確かめたい, so that 次のリリース（0.0.2）を安心して始められる

#### Acceptance Criteria
1. When 撤去が終わったとき, the repository shall spec の文書を除くリポジトリのどこにも、初版のためだけのものを示す印 `【初版のみ】` を持たない。
2. The repository shall リリース CI・公開スクリプト・定型コマンド・補助スクリプトのテスト・`doc/release-notes/v0.0.1.md` を、本仕様の変更に含めない。
3. When 撤去が終わったとき, the repository shall 補助スクリプトのテスト・ワークスペースの全テスト・clippy（警告をエラーとして扱う）がすべて通る状態である。
4. When 撤去が終わったとき, the Release Skill shall 現在の実際の状態（v0.0.1 が 4/4）で状態の照会を判定表に当てると、「リリース済み」の行に当たり、次の版の決定から始める。
