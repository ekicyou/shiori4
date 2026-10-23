# Brief: abi-textbook

## Problem
shiori4 の ABI を使う人は 2 種類いる。SHIORI の作者（pasta など）と、ベースウェアの作者（areka など）である。どちらも、今の文書だけでは規約を一通りに読めない。3 者会談（2026-10-23）では、いちばん時間を使ったのが「誰が誰を、どのスレッドから、いつ呼べるか」の解釈だった。Rust 以外で書く人には、C ヘッダの注釈のほかに手がかりが無い。SHIORI/3.0 からの移行の手引きも無い。

## Current State
- 規約の本文は、C ヘッダの冒頭の注釈と、各クレートの doc に散らばっている。
- 各クレートの README は、crates.io に出すための最小のものしか無い。
- abi-conventions と host-protocol が、規約と語彙の本文を確定させる（この spec の上流）。
- 2 者に聞いた要望は `hearing.md` にある（この spec の入力）。

## Desired Outcome
shiori4 リポジトリが「ABI の教科書」を持っている。

- SHIORI の作者とベースウェアの作者が、Rust でも C でも、迷わずに実装できる。
- 規約を番号で引用でき、各規約に理由が添えてある。
- DLL だけを試せる、試験用のホストがある。

## Approach
読む順は、2 者の要望（hearing.md の 1）に従う。

1. 全体図: 8 つの関数を、誰がどのスレッドからいつ呼べるかの表 1 つにまとめる。
2. 最小の例: Rust と C、SHIORI とホスト。
3. 番号付きの規約の一覧（理由つき）。
4. host-protocol の語の表。
5. 細部（docs.rs の例・C ヘッダの説明・移行の手引き・ゴーストの配布の決まり）。

README は 1 と 2 と道案内だけにする。規約の本文は上流の 2 本の spec が確定させたものを引き、この spec では語を変えない。

## Scope
- **In**:
  - ルートと各クレートの README.md。
  - docs.rs の例。SHIORI 側（`shiori4`）とホスト側（`shiori4_host`）で、hearing.md の 2 と 3 に当たる。
  - C ヘッダの説明。所有権・静的 CRT・版の判定・最小の C のホスト・`SHIORI_HAS_FIELD`・言語ごとの例外の扱い。
  - 規約の理由。
  - 別のプロセスでホストを動かすときの注意（hearing.md の 4）。
  - 移行の手引き（SHIORI 側とホスト側）。
  - host_protocol の確定の範囲の一覧。
  - ゴーストの配布の決まりの章。
  - 試験用のホスト。DLL だけを読み、要求を投げて応答を見る、例か小さな CLI。Linux と macOS の CI で、DLL の境界まで試すために使う。
- **Out**:
  - 規約と語彙そのものの変更（abi-conventions と host-protocol が持つ）。
  - docs.rs 以外のドキュメントサイト。
  - pasta や areka の文書。

## Boundary Candidates
- 読み物（README・全体図・規約の一覧・移行の手引き・配布の決まり）
- API の doc と例（docs.rs）
- C の書き手向けの説明と、C の最小の例
- 試験用のホスト

## Out of Boundary
- ABI の関数・構造体・規約の文言の決定
- host-protocol の語の決定

## Upstream / Downstream
- **Upstream**: abi-conventions、host-protocol（どちらも統合済みであること）。hearing.md
- **Downstream**: pasta の v4 対応（0.5.0）、areka の `shiori4-api`（v0.2.0）、ほかの SHIORI とベースウェアの作者

## Existing Spec Touchpoints
- **Extends**: completed の crate-publish-ready が作った各クレートの README を書き直す。
- **Adjacent**: abi-conventions と host-protocol。両者が書く doc の更新と重ならないよう、この spec は教科書としての構成と例を持つ。

## Constraints
- 文書は日本語で書く。
- 規約の番号は、実装側が引用し続けられるように固定する。
- 例は `cargo test --doc` か CI で動かし、腐らないようにする。
- 試験用のホストは、3 OS の CI で動かせるものにする。

## 申し送り（abi-conventions より）
- 規約の本文の節は、C ヘッダ（`shiori4_abi/include/shiori4_abi.h` 冒頭）と `shiori4_abi` の `//!` で同文になった。節の順: 版／ABI 規約／戻り値／スレッド規約 1〜7／load の規約／ホストのプロセス／DLL の口と発見の順／post の完了通知／配布の決まり。教科書はこの番号と言葉を参照する。
- `ShioriHostPoster` は `Copy` でなくなり（`.clone()` で複製）、任意のスレッドから呼べる `log` を持つ。unload を始めた時点で Poster は無効になり（post は `SHIORI_E_SHUTDOWN`、log は何もしない）、join は要らない。unload の後まで残った `ShioriHostHandle` も、request は `SHIORI_E_SHUTDOWN`、log は何もしない。
- 新しいエラー `SHIORI_E_WRONG_THREAD`（-7）: 規約 2 の期間（load・request・notify の処理中・同じスレッド）の外の `host->request`。unload の中の request も -7 になる。
