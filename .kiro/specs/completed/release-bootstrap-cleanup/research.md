# Research & Design Decisions — release-bootstrap-cleanup

## Summary
- **Feature**: release-bootstrap-cleanup
- **Discovery Scope**: Simple Addition（削除だけ。形式の調べは省き、置き場所の確認だけを行った）
- **Key Findings**:
  - 初版の経路は `【初版のみ】` の印で 5 か所に見つかる（procedure.md 2・SKILL.md 1・release-ci-setup.md 2）
  - 印の無い 0.0.1 に固有の記述は release-ci-setup.md の冒頭の箇条・全体の流れ・後片付けにある（release-workflow からの申し送りのとおり）
  - スクリプト・ワークフロー・テストには初版の判定が無い。`release-notes.ps1` とテストの「前のタグが無い版」は初版に限らない一般の振る舞いで、残す

## Research Log

### 初版の経路の置き場所
- **Context**: brief の Current State は着手前の見込みで、実装とずれていた
- **Sources Consulted**: `git grep -n '【初版のみ】'`、`git grep -n -E '初版|0\.0\.1'`（`.claude/skills/release`・`.github`・`.kiro/steering`・`doc/release-notes/README.md`）
- **Findings**: 削る対象は文書の 3 ファイルだけ。定型コマンドの使い方の例（`-Version 0.0.1`）と `release-state.ps1` のコメント（「初版の前」）は初版の経路ではないので残す
- **Implications**: 変更は Markdown に限られ、補助スクリプトのテストの項目は変わらない

## Design Decisions

### Decision: 手順書の全体の流れの形
- **Context**: 初版の手順を消すと、全体の流れの手順 2・3・5 が無くなる
- **Alternatives Considered**:
  1. 全体の流れの節ごと消す
  2. 設定のやり直しと新しいクレートの追加に使う 4 手順に書き直す
- **Selected Approach**: 2
- **Rationale**: 手順書は改名・移管・新しいクレートの追加のときにも使う（冒頭の箇条）。流れが無いと、どの節から読むかが分からない
- **Trade-offs**: 「節の順が実施の順」の決まりと、新しいクレートの初回公開の節が後ろにあることが少しずれる。流れの手順で節を名前で指して補う

### Decision: 後片付けの節を残す
- **Context**: 後片付けは初版のために書かれたが、新しいクレートの初回公開の手順 4 と緊急時の手動公開が同じ設定（Trusted Publishing のみ・トークンの失効）を使う
- **Selected Approach**: 見出し「後片付け（必須）」を変えずに残し、冒頭と手順 1 を「手で公開した後」の言い回しにする
- **Rationale**: 見出しは procedure.md と新しいクレートの節から名前で指されている（3.4）

## Synthesis Outcomes
- 一般化: なし（削るだけ）
- 作るか使うか: 新しい仕組みは作らない。確認には既存の定型コマンド A とテストを使う
- 簡素化: S9 の条件を変えずに、元の S2 の状態を受けさせる

## Risks & Mitigations
- 手順書の見出しを誤って変える — 参照先の実在の確認（設計の Testing Strategy）で見つける
