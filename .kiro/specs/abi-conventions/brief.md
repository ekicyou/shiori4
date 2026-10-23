# Brief: abi-conventions

## Problem
SHIORI 実装者（pasta）とベースウェア実装者（areka）が、`shiori4_abi.h`（main 72cff0b）の規約を読んで v4 対応を設計しようとしている。ところが、いくつかの規約は意味が一通りに読めない。手段を指定していて守れない規約もある。何も書いていない所もある。このまま実装すると、両者が別の読み方をしてデッドロックや解放後の呼び出しを起こす。

## Current State
`shiori4_abi/include/shiori4_abi.h` の冒頭の「ABI 規約」「スレッド規約」「post の完了通知」と、Rust 版 `shiori4_abi` の型。高水準層（`shiori4` の `ShioriHostPoster`・`export_ghost!`、`shiori4_host` の `GhostRuntime`）は今の規約で作られている。2026-10-23 に、2 つのセッションの意見に shiori4 側の推奨を返して合意した（下の「合意した論点」）。

## Desired Outcome
- C ヘッダ・Rust の doc・高水準層の 3 つが、同じ規約を同じ言葉で述べている。
- 合意した論点がすべてそこに入っている。
- 高水準層を使う Ghost 実装者・Host 実装者は、join や自前のスレッド検査なしで規約を守れる。

## 合意した論点（pasta = P、areka = A）
1. **スレッド規約 2 の意味**（P1）: SHIORI が `host->request` を呼べるのは「load / request / notify を呼ばれているスレッドから、その呼び出しが返るまでの間だけ」。ABI は厳しいまま。違反を見分けるため `SHIORI_E_WRONG_THREAD (-7)` を足す。ホストはスレッド ID を見て -7 を返してよい。
2. **unload の約束**（P2）: 規約 5 の「post するスレッドをすべて止めてから返る」は手段の指定なのでやめる。代わりに結果を約束する:「unload が返った後に ShioriHostApi のどの関数も呼ばない」。`shiori4` の `ShioriHostPoster` は、unload を始めた時点で無効になり、以後は host に触れず `SHIORI_E_SHUTDOWN` を返す。こうすれば、Ghost 実装者が join しなくても守れる（今の doc の「Drop で止めて join すること」を改める）。
3. **log と release のスレッド**（P3）: `host->log` は post と同じく任意のスレッドから呼べる（ホストはコピーしてすぐ返る）。`ShioriBytes.release` も、どのスレッドから呼んでもよい。
4. **notify の後回し**（P5）: notify を即 204 で返し、処理を後で行う実装は合法とする。後回しにした処理の中では `host->request` を使えない（post を使う）、と注記する。
5. **load の失敗**（P6）: 失敗するときは、NULL を返す前に `host->log` で理由を出す。SHIORI は 2 つ目のインスタンスを拒否してよい（NULL ＋ log）。拒否されたホストは、ゴーストごとに別のプロセスで読むか、拒否を受け入れる。
6. **load の dir の形**（A5）: v4 も v3 互換も「絶対パス・OS の区切り・末尾に区切りあり・UTF-8」とする（SSP が SHIORI/3.0 の load に渡す形に合わせる）。 pasta も 3 者会談で問題なしと確認した（PathBuf で使い、末尾の区切りの有無に関係なく動く）。
7. **v4 の Charset**（A6）: v4 は UTF-8 固定なので、ホストは Charset ヘッダを付けなくてよい。付いていても SHIORI は無視する。応答にも付けない。

## 3 者会談で出た追加の論点（areka の提案。3 者とも同意し、2026-10-23 に確定）
8. **複数のインスタンス**（areka A）: areka は 1 つのプロセスで複数のゴーストを動かす（`multi-ghost-runtime`）。同じ DLL のファイルを 2 体が指すと、load が 2 回呼ばれる。
   - 論点 5 の拒否（NULL ＋ log。ホストは読めなかったゴーストとして扱う）は保つ。
   - 加えて「SHIORI はできれば複数のインスタンスを受けるべき」と推奨として書く。
   - pasta は、複数のインスタンスを受けられる見込み。Lua VM はすでにアクタースレッドごとに持っており、プロセスに 1 つの mailbox などはインスタンスの中へ移す。どの版で入れるかは pasta が決める。
9. **単一スレッドの単位**（areka B）: 「ホストが決めた単一のスレッド」は、インスタンスごとであって、プロセスで 1 つではない。
   - 同じ DLL の 2 つのインスタンスは、別々のスレッドから呼ばれうる。
   - `SHIORI_E_WRONG_THREAD` もインスタンスごとに判定する。
10. **unload の時間**（areka F）: 「unload は長く止まらないこと。ホストは OS の終了の猶予の中で、各インスタンスの unload を呼ぶことがある」と注意を一行足す。
    - 時間の数字は書かない。
    - 論点 2（SHIORI に join を求めない）と食い違わない言い回しにする。

11. **SHIORI を読むプロセス**（areka の提案。2026-10-23。pasta も同意）: 注意を一行足す。
    - 文言:「ホストは SHIORI を専用のプロセスで読むことがある。SHIORI は、ホストと同じプロセスにいること（ホストの窓・プロセスの状態を共有すること）を前提にしない。」
    - `host->request` は IPC を往復することがあり、遅くなりうる、とも書く。
    - ABI の形は変わらない。
    - 背景: areka は v4 の DLL も同じプロセスでは読まず、SHIORI ごとに helper のプロセスを起こして、その中で shiori4_host を使う。
    - 論点 8（同じプロセスでの 2 つ目の load）は、ほかのホストのための規約として残す。

## Approach
規約の本文は C ヘッダの冒頭の注釈を正本にする。Rust 版の doc と高水準層の doc はそれに合わせる。

- 足すもの: 定数（-7）と、`ShioriHostPoster` の無効化。どちらも末尾追加・挙動の追加だけで、既存の利用者を壊さない。
- `shiori4_host` の `GhostRuntime` / `Bridge` は、次を規約どおりに作る:
  - 規約 2 の違反（別スレッドからの `host->request`）に -7 を返す
  - load の dir を規約の形で渡す
  - Charset を付けない

## Scope
- **In**:
  - `shiori4_abi`: C ヘッダの規約の注釈、`SHIORI_E_WRONG_THREAD` の定数（C と Rust）、Rust 版の doc。
  - `shiori4`: `ShioriHostPoster` の無効化と、その doc。
  - `shiori4_host`: スレッドの検査と -7、load の dir の正規化、log と release の任意スレッド対応の確認。
  - 上の 3 つのテスト。
- **Out**:
  - 運ぶ中身の語彙（property・post のコマンド・必ず付けるヘッダ・version・OnHostComplete）。これは host-protocol。
  - x86 の v4 DLL を別プロセス越しに読むこと（areka は当面しない）。
  - SHIORI/3.0 側の HGLOBAL の入口。

## Boundary Candidates
- C ヘッダの規約文（正本）と Rust の型・定数の同期
- SHIORI 側の高水準層が規約を守る仕組み（poster の無効化）
- ホスト側の高水準層が規約違反を見分ける仕組み（スレッド検査・-7）と、ホストが渡す値の形（dir・Charset）

## Out of Boundary
- host_protocol の名前と語彙（host-protocol が持つ）
- ベースウェア・SHIORI の個別の実装（pasta のアクタースレッド・areka の専用スレッド）

## Upstream / Downstream
- **Upstream**: completed の crate-publish-ready（3 クレートの形）。v0.0.1 の ABI
- **Downstream**:
  - host-protocol（同じファイルに触れる）。
  - pasta の v4 対応。
  - areka の spec `shiori4-api`（areka リポジトリ）。
  - 0.0.2 のリリース。

## Existing Spec Touchpoints
- **Extends**: なし（ABI の初版は spec 外で作られた）
- **Adjacent**: host-protocol（`shiori4/src/lib.rs`・`shiori4_host/src/lib.rs` を同時に触る。拒否の返し方は 400 ＋ `Reason` に決まり、ABI の定数は足さない）

## Constraints
- 構造体のレイアウト（`SHIORI_STATIC_ASSERT`）を変えない。足すのは定数と注釈だけで、関数テーブルは末尾追加の規則に従う。
- 3 クレートは同じ版で出す。この spec は版を上げない（`/release` が上げる）。
- 規約の文は日本語。C ヘッダは Rust 以外の実装者も読む正本である。
