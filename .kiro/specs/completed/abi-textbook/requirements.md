# Requirements Document

## Project Description (Input)
shiori4 の ABI を使う人は 2 種類いる。SHIORI の作者（pasta など）と、ベースウェアの作者（areka など）である。どちらも、今の文書だけでは規約を一通りに読めない。2026-10-23 の 3 者会談では、いちばん時間を使ったのが「誰が誰を、どのスレッドから、いつ呼べるか」（スレッド規約 2・5）の解釈だった。

いま、規約の本文は C ヘッダ `shiori4_abi/include/shiori4_abi.h` の冒頭の注釈と `shiori4_abi` の `//!` に同文で置かれ、語彙の正本は `doc/host-protocol.md` にある。各クレートの README は crates.io に出すための最小のもので、docs.rs の例は各クレートに 1〜数本しかない。Rust 以外で書く人には C ヘッダの注釈のほかに手がかりが無く、SHIORI/3.0 からの移行の手引きも無い。DLL だけを読んで要求を投げる手段も無い。

この spec では、shiori4 リポジトリに「ABI の教科書」を持たせる。SHIORI の作者とベースウェアの作者が、Rust でも C でも迷わずに実装でき、規約を番号で引用でき、各規約に理由が添えてあり、DLL だけを試せる試験用のホストがある状態にする。規約と語彙の本文は上流の abi-conventions と host-protocol が確定させたものを引き、この spec では語を変えない。要望の出どころは `hearing.md`（pasta と areka へのヒアリング）である。

## Introduction
この spec は 4 つのものを持つ（brief.md の Boundary Candidates）。

1. **読み物**: README（ルートと各クレート）・全体図・番号付きの規約の一覧と理由・host-protocol の語の表と確定の範囲・別のプロセスでホストを動かすときの注意・移行の手引き・ゴーストの配布の決まりの章。
2. **API の doc と例**（docs.rs）: SHIORI 側（`shiori4`）とホスト側（`shiori4_host`）の使い方の例（hearing.md の 2 と 3）。
3. **C の書き手向けの説明と、C の最小の例**: 所有権・静的 CRT・版の判定・最小の C のホスト・`SHIORI_HAS_FIELD`・言語ごとの例外の扱い・呼ぶスレッドの規約（hearing.md の 5）。
4. **試験用のホスト**: ベースウェア無しで SHIORI の DLL を読み、要求を投げて応答を見るもの（hearing.md の 8）。

読む順は hearing.md の 1 に従う（全体図 → 最小の例 → 規約の一覧 → 語の表 → 細部）。各要件の目的の末尾に、対応する hearing.md の節を「（hearing N）」で示す。

brief.md で決め切れていなかった所は、要件ディスカッション（2026-10-24）で決めた。**【決定 Qn】** の Qn は末尾の「決定事項」の番号である。

## Boundary Context
- **In scope**:
  - ルートと各クレート（`shiori4_abi`・`shiori4`・`shiori4_host`）の README.md の書き直し。
  - 全体図、番号付きの規約の一覧と各規約の理由、host-protocol の語の表、host_protocol の確定の範囲の一覧。
  - docs.rs の例（SHIORI 側とホスト側。hearing.md の 2 と 3）。
  - C の書き手向けの説明と C の最小の例（hearing.md の 5）。
  - 別のプロセスでホストを動かすときの注意（hearing.md の 4）。
  - 移行の手引き（SHIORI 側とホスト側。hearing.md の 7）。
  - ゴーストの配布の決まりの章（hearing.md の 8）。
  - 試験用のホストと、それを使って DLL の境界まで試す CI の検査。
  - 教科書の例（Rust と C）が腐らないための検査。
  - C ヘッダ冒頭と `shiori4_abi` の `//!` の規約の節に、規約の番号だけを書き入れること（文言は変えない。要件 4.3）。
  - 設計の議論（2026-10-24）で見つかった正本の穴を埋める ABI の変更（要件 15）: `load`・`unload` を呼ぶスレッド、`ShioriBytes` を作る側と `release` を呼べる期間、それに合わせた 3 クレートと testghost の直し。
- **Out of scope**:
  - 規約と語彙そのものの決定・変更（ABI の関数・構造体・規約の文言は abi-conventions、host-protocol の語は host-protocol が持つ）。番号を書き入れることは文言の変更に当たらない。例外は要件 15 の変更だけである。
  - ホストが使うメモリ確保の簡易版（C ヘッダの既定の実装や、別のプロセスのホストが写しを減らす工夫）。問題になったら別の spec に切り出す。
  - docs.rs 以外のドキュメントサイト。
  - pasta や areka の文書。
  - 従来の SHIORI/3.0 の DLL（HGLOBAL の `load`・`loadu`・`request`・`unload`）を読むこと（shiori3-legacy-host が持つ。試験用のホストへのつなぎも shiori3-legacy-host が持つ）。
  - 版を上げること・公開すること（`/release` が行う）。
- **Adjacent expectations**:
  - abi-conventions と host-protocol は統合済みで、C ヘッダ冒頭と `shiori4_abi` の `//!` の規約の節（版／ABI 規約／戻り値／スレッド規約 1〜7／load の規約／ホストのプロセス／DLL の口と発見の順／post の完了通知／配布の決まり）と、`doc/host-protocol.md`（第 0〜8 章・付録 A・付録 B）が正本である。教科書はこれらの番号と言葉を引き、写すときに変えない。
  - 教科書の例は、上流が決めた今の API の形で書く（`Ghost::request`／`notify` は原文の `&str` を受ける・`Host::name()` が必須・`ShioriHostPoster` は `.clone()` で複製し `log` を持つ・unload を始めた時点で Poster は無効・`SHIORI_E_WRONG_THREAD`（-7）・`GhostRuntime::from_library`・`load_ghost`／`load_ghost_with`・`protocol::getproperty_response`／`rejection_response`／`host_complete` と `Completer::complete_with`）。
  - shiori3-legacy-host は、試験用のホストが従来の DLL も読めるようにするつなぎを、この spec の試験用のホストの上に足す。
  - 下流の pasta の v4 対応（0.5.0）・areka の `shiori4-api`（v0.2.0）・ほかの作者は、教科書の規約の番号を引用し続ける。

## Requirements

### Requirement 1: 教科書の構成と、正本との関係
**Objective:** As a SHIORI の作者・ベースウェアの作者, I want 教科書が決まった読む順で並び、規約と語の正本をそのまま引いていること, so that どこから読み始めても同じ一通りの意味にたどり着ける（hearing 1）

#### Acceptance Criteria
1. The ABI の教科書 shall 日本語で書かれ、全体図 → 最小の例 → 番号付きの規約の一覧 → host-protocol の語の表 → 細部（docs.rs の例・C ヘッダの説明・移行の手引き・配布の決まり）の順に読めるよう道案内を持つ。
2. The ABI の教科書 shall 規約の文を、C ヘッダ冒頭と `shiori4_abi` の `//!` の文言のまま引き、語を、`doc/host-protocol.md` の綴りのまま引く。
3. If 教科書を書く中で、正本の規約や語に欠け・食い違い・二通りに読める所が見つかったら, the ABI の教科書 shall その場で規約や語を決めず、未決のものとして記録して開発者の裁定へ回す（上流の abi-conventions・host-protocol は完了済みなので、直すときは新しい spec か修正の PR になる）。
4. The ABI の教科書 shall それが当てはまる shiori4 の版を述べ、公開された版の内容はその版のタグの文書で読むことを述べる（`doc/host-protocol.md` 第 0 章と同じ扱い）。
5. The ABI の教科書 shall 読み物を、docs.rs 以外のドキュメントサイトを使わずに、shiori4 リポジトリの中の文書（GitHub で読める形）と docs.rs のクレートのドキュメントで提供する。読み物の本文はリポジトリの `doc/` の文書に置き、README と docs.rs のクレートのドキュメントから GitHub の URL でそこへ導く。【決定 Q1】

### Requirement 2: 全体図（誰が誰を、どのスレッドから、いつ呼べるか）
**Objective:** As a SHIORI の作者・ベースウェアの作者, I want 8 つの関数の呼び出しの決まりが 1 つの表にまとまっていること, so that 会談でいちばん時間を使った規約 2・5 の解釈を、表を見るだけで一通りに読める（hearing 1）

#### Acceptance Criteria
1. The 全体図 shall `load`・`request`・`notify`・`host.request`・`post`・`log`・`release`・`unload` の 8 つを 1 つの表にまとめ、関数ごとに「呼ぶ側」「呼ばれる側」「呼べるスレッド」「呼べる期間」を示す。
2. The 全体図 shall 各行に、根拠になるスレッド規約の番号（1〜7）または規約の節の名前を添える。
3. The 全体図 shall 期間やスレッドの外で呼んだときに返りうる結果（`SHIORI_E_WRONG_THREAD`・`SHIORI_E_REENTRANT`・`SHIORI_E_SHUTDOWN` など）と、unload を始めた後と返った後の扱いを示す。
4. Where v3 互換モード（`shiori_get_api(3)`）を使うとき, the 全体図 shall host が `copy_from_slice` だけを持ち、ほかのホストの関数（`host.request`・`post`・`log`）を呼べないことを示す（要件 15）。

### Requirement 3: 最小の例
**Objective:** As a 初めて shiori4 を使う作者, I want 自分の言語と立場に合った最小の動く例があること, so that 全体を読む前に、往復の 1 回を自分の手で動かせる（hearing 1）

#### Acceptance Criteria
1. The ABI の教科書 shall Rust の SHIORI・C の SHIORI・Rust のホスト・C のホストの 4 つの最小の例を持つ。
2. The 最小の SHIORI の例 shall `shiori_get_api` で関数テーブルを返して `OnBoot` の要求に応答することを、30 行ほどで示す。Rust の例は `export_ghost!` で v3 と v4 の両方を出し、C の例は v4 のテーブルだけを返す（v3 も返す形は要件 9.4 の版の判定の例で示す）。【決定 Q2】
3. The 最小のホストの例 shall SHIORI を読む → `OnBoot` を要求する → 応答を受け取る → unload する、の順を示す。
4. The 最小の例 shall それぞれ、得られる応答（または表示）を例の中か直後に示す。

### Requirement 4: 番号付きの規約の一覧と、規約の理由
**Objective:** As a SHIORI の作者・ベースウェアの作者, I want すべての規約に変わらない番号と短い理由が付いていること, so that 実装や議論から「規約 N」で引用でき、理由を知らずに規約を緩めたり「親切に」破ったりせずに済む（hearing 1・6）

#### Acceptance Criteria
1. The 規約の一覧 shall C ヘッダ冒頭の規約の節（版／ABI 規約／戻り値／スレッド規約 1〜7／load の規約／ホストのプロセス／DLL の口と発見の順／post の完了通知／配布の決まり）の規約を、節の順のまま、1 つ残らず、正本の文をそのまま写して並べる。【決定 Q4】
2. The 規約の一覧 shall スレッド規約 1〜7 の番号を、C ヘッダと `shiori4_abi` の `//!` の番号のまま使い、「規約 N」はこれまでどおりスレッド規約 N を指す。
3. The 規約の一覧 shall スレッド規約以外の各規約に「節の名前 ＋ 番号」（例「load の規約 1」）の番号を付け、同じ番号を C ヘッダ冒頭と `shiori4_abi` の `//!` の両方の規約の節に書き入れる。書き入れるのは番号だけで、規約の文言は変えない。【決定 Q3】
4. When 一度公開した版で規約に番号を付けたら, the 規約の一覧 shall 後の版でその番号を別の規約に使い回さず、規約が足されたときは新しい番号を足す。
5. The 規約の一覧 shall 各規約に 1〜2 文の理由を添え、長い経緯は別の所に置いてそこへ導く。理由は教科書にだけ置き、C ヘッダと `//!` には足さない。【決定 Q4】
8. If 完了済みの spec（abi-conventions・host-protocol ほか）の記録から規約の理由を引けなかったら, the 規約の一覧 shall その理由を、記録に無いことを示す印を付けた草案として書き、統合の前に開発者の確認を受ける。【決定 Q4】
6. The 規約の一覧 shall 少なくとも次の規約の理由を述べる: ABI を緩めない理由（`host.request` をホストの UI スレッドで待つとデッドロックするなど）、再入の禁止、post の完了を post の中で届けないこと、unload で未配送を捨てること、SEND が既定で割り込むこと（SSTP に揃えた）。
7. The 全体図・C の書き手向けの説明・docs.rs の例 shall 規約に触れる所で、この一覧の番号を引用する。

### Requirement 5: host-protocol の語の表と、確定の範囲の一覧
**Objective:** As a ベースウェアの作者, I want ID・ヘッダ・ステータス・Reason の語の表と、どの語が確定でどれが変わりうるかの一覧があること, so that 自分の実装が依存してよい語を、版ごとに見分けられる（hearing 1・8）

#### Acceptance Criteria
1. The 語の表 shall ID・ヘッダ名・ステータス・`Reason` の語を、`doc/host-protocol.md` 第 8 章の語と定数名の対応に沿って一覧にし、各語から正本の章・節へ導く。第 8 章に行の無いステータスは、`shiori4_abi` の定数名（`SHIORI_<コード>_<理由>`・`SHIORI_E_*`）と、それを使う `doc/host-protocol.md` の節から引く。
2. The 語の表 shall 語を写すときに綴りを変えず、語の意味は `doc/host-protocol.md` と ukadoc の参照先（付録 B）に委ねる。
3. The 確定の範囲の一覧 shall shiori4 の版ごとに、どの語と API が確定で、どれが変わりうるかを示す。
4. The 確定の範囲の一覧 shall 0.0.1 から 0.0.2 への変更を、`doc/host-protocol.md` 付録 A を起点にして示す。
5. The 確定の範囲の一覧 shall `doc/host-protocol.md` に書かれた語を、その文書が当てはまる版で確定とし（後の版で変えるときは付録 A と同じく移行を書く）、文書に無い語（定数の無い `Reference*` の並びの拡張など）を「決まっていない」として示す。語ごとの安定度は教科書では決めない。【決定 Q5】
6. The 確定の範囲の一覧 shall 冒頭で、0.0.x の間は版ごとに互換を壊す変更がありうることを述べる。【決定 Q5】

### Requirement 6: README（ルートと各クレート）
**Objective:** As a crates.io や GitHub で shiori4 を見つけた作者, I want README が全体図と最小の例と道案内だけを持つこと, so that 最初の画面で全体を掴み、次に読む文書へ迷わず進める（hearing 1）

#### Acceptance Criteria
1. The ルートの README shall 全体図（要件 2）・最小の例（要件 3）・各文書への道案内だけを持つ。
2. The 各クレートの README shall そのクレートの役割・想定する利用者・使い始めの入口・他の 2 クレートとの関係・詳しいドキュメントへの道案内・ライセンスを持つ。
3. The 各クレートの README shall crates.io の頁に表示されたときにも切れないリンクで、教科書の各文書と docs.rs へ導く。
4. Where README がコードの例を持つとき, the README shall その例を要件 14 の検査の対象にする。
5. The ルートと各クレートの README shall 規約の本文を写さず、規約は要件 4 の一覧へ導く（structure.md の「README は使い方の入口だけ」に従う）。

### Requirement 7: docs.rs の例（SHIORI 側 `shiori4`）
**Objective:** As a SHIORI の作者（pasta）, I want 規約の難しい所を守る書き方の例が docs.rs にあること, so that 別スレッド・post・unload・複数インスタンス・v3 と v4 の両対応を、規約どおりに書ける（hearing 2）

#### Acceptance Criteria
1. The `shiori4` のクレートのドキュメント shall 処理を別のスレッド（アクター）で動かす SHIORI の例を持ち、アクターからの `host.request` を、応答を待っている SHIORI のスレッドへ中継して規約 2 の期間の中で呼ぶ形を示す。
2. The `shiori4` のクレートのドキュメント shall post → `Tag` → `OnHostComplete` で待っている処理を再開する例を持ち、`PendingTags` の使い方と、登録してから post する理由を示す。
3. The `shiori4` のクレートのドキュメント shall join しない unload の例を持ち、unload の開始で Poster が無効になる（post は `SHIORI_E_SHUTDOWN`・log は何もしない）ことと、固まったスレッドがあっても規約 5 を守れることを示す。
4. The `shiori4` のクレートのドキュメント shall 状態をインスタンスごとに持ち、グローバルな状態を置かない書き方の例を持つ（load の規約の「複数のインスタンスを受けるべき」）。
5. The `shiori4` のクレートのドキュメント shall v3 と v4 の両方で動く `Ghost` の例を持ち、host が `None` のときの代わりの動きを示す。
6. The `shiori4` の例 shall 上流が決めた今の API の形（Boundary Context の Adjacent expectations）で書かれる。

### Requirement 8: docs.rs の例（ホスト側 `shiori4_host`）
**Objective:** As a ベースウェアの作者（areka）, I want 読み込み・post・完了通知・プロパティ・unload・テストの書き方の例が docs.rs にあること, so that ホストの責任を一通り規約どおりに果たせる（hearing 3）

#### Acceptance Criteria
1. The `shiori4_host` のクレートのドキュメント shall `LoadError` の 5 つの値（`Library`・`NotShiori4`・`Unsupported`・`InvalidPath`・`LoadFailed`）のそれぞれについて、ホストがすべきことを示す（`NotShiori4` は旧来の SHIORI/3.0 として読む候補に回す、`LoadFailed` は読めなかったゴーストとして扱い SHIORI/3.0 として読み直さない、`Unsupported` は別の版で読み直せる、を含む）。
2. The `shiori4_host` のクレートのドキュメント shall `GhostRuntime::load` が `NotShiori4` のときに読んだライブラリを手放すこと（別の口で読み直すと DLL が 2 回読まれること）を明記し、2 回読まずに済む `GhostRuntime::from_library` と `FromLibraryError` からライブラリを取り戻す道を示す。
3. The `shiori4_host` のクレートのドキュメント shall `Host::wake` から `GhostRuntime::pump` へのつなぎ方を、Win32 のメッセージの輪と、std の channel を使うスレッドの 2 通りで示す。Win32 のメッセージの輪の例は、外部の crate を足さずに使う Win32 の関数を例の中で宣言し、Windows でビルドまで検査する。【決定 Q6】
4. The `shiori4_host` のクレートのドキュメント shall `Completer` で `OnHostComplete` を返す例を、SEND の completed と interrupted、NOTIFY のそれぞれについて持つ。
5. The `shiori4_host` のクレートのドキュメント shall nobreak で待たせた SEND を、待つ間の終了で捨てて 204 の `OnHostComplete` を返す例を持つ（`doc/host-protocol.md` 第 6.3・6.8 節）。
6. The `shiori4_host` のクレートのドキュメント shall getproperty と setproperty に答える `Host` の例と、拒否を 400 ＋ `Reason: forbidden` で返す例を持つ。
7. The `shiori4_host` のクレートのドキュメント shall unload の順序（shutdown → 未配送を捨てる → unload → ライブラリを放す）と、`GhostRuntime` の Drop の中で起きることを示す。
8. The `shiori4_host` のクレートのドキュメント shall `GhostRuntime::from_static` で DLL なしに往復を通す、決定論のテストの書き方の例を持つ。
9. The `shiori4_host` の例 shall 上流が決めた今の API の形（Boundary Context の Adjacent expectations）で書かれる。

### Requirement 9: C の書き手向けの説明と、C の最小の例
**Objective:** As a Rust 以外で SHIORI やホストを書く作者, I want 所有権・版の判定・スレッド・例外の扱いが C の書き手の言葉で説明され、動く C の例があること, so that C ヘッダの注釈だけに頼らずに正しく実装できる（hearing 5）

#### Acceptance Criteria
1. The C の書き手向けの説明 shall `out` の `ShioriBytes` を誰が作り（常にホスト。SHIORI は `host->copy_from_slice` で作る）、誰がいつ `release` するかの所有権の表と図を持つ（要件 15）。
2. The C の書き手向けの説明 shall `release` がどのスレッドからも、unload の後にも呼ばれうること（規約 6）と、戻り値が負のときも呼んだ側が `out` を解放することを、例を付けて示す。
3. The C の書き手向けの説明 shall 静的 CRT で作る DLL について、確保も解放もホストのメモリとホストのコードだけで起きるので CRT が別でも関係ないことを述べる。
4. The C の書き手向けの説明 shall 版の判定の例として、ホスト側の手順（`shiori_get_api(4)` が NULL なら 3 を試す）と、SHIORI 側の書き方（版ごとに別の静的なテーブルを返す）を示す。
5. The C の書き手向けの説明 shall 最小の C のホストの例として、ライブラリを開く（Windows は `LoadLibrary`、それ以外は `dlopen`）→ `shiori_get_api(4)` → 無ければ 3 → load → request → unload の流れを示す。
6. The C の書き手向けの説明 shall `struct_size` で足されたフィールドの有無を確かめる例（`SHIORI_HAS_FIELD`）を示す。
7. The C の書き手向けの説明 shall スレッド規約 1〜7 を、C の書き手の言葉（どの関数をどのスレッドから呼ぶか・どの関数ポインタがいつまで有効か）で述べ直し、各文に規約の番号を添える。
8. The C の書き手向けの説明 shall パニックや例外を境界の外へ出さない書き方を、C++ の例外・Zig の panic などの言語ごとに一言ずつ述べ、境界では `SHIORI_E_PANIC` を返すことを示す。

### Requirement 10: ホストを別のプロセスで動かすときの注意
**Objective:** As a SHIORI を専用のプロセスで読むベースウェアの作者, I want 別のプロセスにしたときに起きる止まり方と時点のずれが短く書かれていること, so that 親子のプロセスが互いに待って止まる作りを避けられる（hearing 4）

#### Acceptance Criteria
1. The 別のプロセスの注意 shall `host.request` の中継が同期の往復になり、親は SHIORI の応答を待つ間にも子からの問い合わせに答えられる待ち方にしないと互いに待って止まることを述べる。
2. The 別のプロセスの注意 shall post は子のプロセスで受けてから親へ送るので、受付（204）と完了が別の時点になることを述べる。
3. The 別のプロセスの注意 shall `SHIORI_E_PANIC` や子のプロセスの異常終了を、インスタンスの終わりとして扱うことを述べる。
4. The 別のプロセスの注意 shall 根拠になる規約（「ホストのプロセス」の節・スレッド規約）の番号を引用し、短くまとめる。

### Requirement 11: 移行の手引き
**Objective:** As a SHIORI/3.0 の SHIORI やホストを持つ作者, I want 今のものから shiori4 へ移る手順が差分の表で示されていること, so that 何を変えれば動くかを短い時間で掴める（hearing 7）

#### Acceptance Criteria
1. The 移行の手引き（SHIORI 側） shall SHIORI/3.0 の DLL → v3 互換モード → v4 の 3 段の差分を表で示し、少なくとも Charset が UTF-8 だけであること（Shift_JIS の要求は来ない）・HGLOBAL が (ptr, len) と、ホストの `copy_from_slice` で作る応答に変わること・load の dir の形が変わることを含む。
2. The 移行の手引き（ホスト側） shall 要求は開始行と Charset を外すだけでヘッダの語は同じであること・応答の状態は戻り値で受けること・旧来の DLL との見分け方（`shiori_get_api` の有無）・両方を持つ DLL の扱い（「DLL の口と発見の順」）を示す。
3. The 移行の手引き（ホスト側） shall v3 互換モードは Windows 以外のホスト向けであり、Windows のホストは旧来の DLL を今までどおり読めばよいことを述べる。shiori3-legacy-host が統合される前は「旧来の DLL を読む口は shiori4_host にまだ無い」と書き、統合後の書き換えは shiori3-legacy-host の brief に申し送る。【決定 Q7】
4. The 移行の手引き shall shiori4 0.0.1 から 0.0.2 への移行について、`doc/host-protocol.md` 付録 A へ導く。

### Requirement 12: ゴーストの配布の決まりの章
**Objective:** As a ベースウェアの作者, I want DLL を読まずに、ホストの OS と arch に合う SHIORI のファイルを選ぶ手順が 1 つの章にあること, so that 配布されたゴーストから正しいファイルを安全に選べる（hearing 8）

#### Acceptance Criteria
1. The 配布の決まりの章 shall descript.txt の `shiori,<名前>` から名前の幹を取り、OS と arch ごとの順で候補を探し、機械の種類をヘッダで確かめ、候補が尽きたら読めなかったものとして扱う手順を、C ヘッダの「配布の決まり」の言葉のまま示す。
2. The 配布の決まりの章 shall その手順を Rust で行う例（`shiori_file_candidates`・`machine_matches`・`TargetOs`・`TargetArch`）を示す。
3. The 配布の決まりの章 shall SHIORI の作者向けに、OS と arch ごとにどの名前でファイルを置けばよいか（Rust の cdylib が既定で作る名前との関係を含む）を示す。
4. The 配布の決まりの章 shall その手順が SHIORI/3.0 と v4 の両方のファイルに使えることを述べる。

### Requirement 13: 試験用のホスト
**Objective:** As a SHIORI の作者（pasta）・shiori4 の開発者, I want ベースウェア無しで SHIORI の DLL を読み、要求を投げて応答を見られる試験用のホストがあること, so that Linux と macOS を含む CI で、DLL の境界まで試せる（hearing 8）

#### Acceptance Criteria
1. When SHIORI のライブラリのパスとゴーストのディレクトリを与えられたら, the 試験用のホスト shall 「DLL の口と発見の順」に従って v4 → v3 互換モードの順に口を探して読み、どの口で読んだかを表示する。
2. When 読み込みが成功したら, the 試験用のホスト shall 与えられた要求（既定は `OnBoot`）を SHIORI に投げ、戻り値（ステータスコードまたは ABI エラー）と応答の全文を表示する。
3. When 要求を終えたら, the 試験用のホスト shall SHIORI を unload して終わる。
4. If 口が見つからない・load が失敗する・ABI エラーが返るなど、試験が成り立たなかったら, the 試験用のホスト shall 原因の種類（`LoadError` の値や ABI エラーの値）を表示し、成功と区別できる終了の状態で終わる。
5. While SHIORI を読んでいる間, the 試験用のホスト shall SHIORI からの `host.request` に最小の `Host` として答え（`name` を返し、ほかは正本の既定どおり）、要求の後に 1 回 pump して、受けた `post` と `log` を表示する。試験用のホストは post の中身を実行せず、`Tag` の付いた post にも完了通知を返さず、届けていない post は unload で捨てる。【決定 Q8】
6. If 読んだファイルが従来の SHIORI/3.0 の口（`loadu`・`load`）だけを持つなら, the 試験用のホスト shall 従来の口を見つけたことを表示し、読めないものとして終わる（従来の DLL を読むのは shiori3-legacy-host が足す）。
7. The 試験用のホスト shall Windows・Linux・macOS の 3 OS で動く。
8. The shiori4 の CI shall 3 OS で試験用のホストにテスト用の SHIORI のライブラリを読ませ、要求と応答の往復が成功することを確かめる。
9. The 試験用のホスト shall `shiori4_host` の example 1 本として提供され、公開するクレートを増やさない。他のリポジトリ（pasta など）の CI が `cargo install --git`（`--example` 付き）などで取り込める形にし、その取り込み方を教科書に書く。【決定 Q9】

### Requirement 14: 例が腐らないための検査
**Objective:** As a shiori4 の開発者, I want 教科書の例が CI で動かされ、API や規約が変わったら壊れて知らせること, so that 読者が古い例を写して迷うことが無い（brief.md の Constraints）

#### Acceptance Criteria
1. The shiori4 の CI shall docs.rs と README の Rust の例を、3 OS でビルドし、DLL やベースウェアを要しない例は実行して結果を確かめる。
2. The shiori4 の CI shall C の最小の例（SHIORI とホスト）・版の判定の例・`SHIORI_HAS_FIELD` の例を 3 OS でビルドし、言語を入れ替えた組で実行して往復が成功することを確かめる: C のホストの例 × テスト用の Rust の SHIORI（`testdata/testghost`）と、試験用のホスト（要件 13）× C の SHIORI の例。【決定 Q10】
3. If 検査の対象から外す例（検査しない例）を置くなら, the ABI の教科書 shall その例が検査されないことと理由を、例の隣に明示する。
4. When 例の検査が失敗したら, the shiori4 の CI shall PR の検査を失敗にする。
5. The 例の検査 shall 今の DoD（`cargo test --all` と `cargo clippy --all-targets --workspace -- -D warnings`）とクレートの公開の検査（`cargo package --workspace`）を壊さない。
6. The shiori4 の CI shall 規約の一覧（要件 4.1）に写した規約の文と番号が、C ヘッダ冒頭と `shiori4_abi` の `//!` の文と番号に一字一句一致することを確かめ、ずれたら PR の検査を失敗にする。【決定 Q4】

### Requirement 15: 全体図の穴を埋める ABI の変更
**Objective:** As a SHIORI の作者・ベースウェアの作者, I want 全体図のすべての行が正本の規約で埋まり、外れた DLL のコードを呼ぶ道が ABI から無くなること, so that 全体図に「規約に定めが無い」が残らず、Rust の高水準層が型の上で UB を生まない（設計の議論 #1）

#### Acceptance Criteria
1. The スレッド規約 1 shall `load`・`request`・`notify`・`unload` を、ホストがインスタンスごとに決めた同じ 1 つのスレッドから呼ぶと定める（`Ghost` は `Send` を求めないため）。
2. The ABI 規約 shall `out` の `ShioriBytes` を常にホストが作ると定め、SHIORI は応答を `ShioriHostApi` の末尾に足す `copy_from_slice`（`ptr` から `len` バイトをホストのメモリへ写し、ホストが作った `ShioriBytes` を `out` に入れる。戻り値は持たない）で作る。If 写せなかったら, the ホスト shall 自分で決めた印を `out` に残して SHIORI には知らせず、SHIORI は `out` をそのまま返し、印を見て扱うのはホストとする（設計の議論 #3）。
3. The スレッド規約 6 shall `release` が常にホストの関数であること、`release` と `copy_from_slice` をどのスレッドから呼んでもよいこと、`release` はインスタンスの unload の後に呼ばれてもよくホストはそれに耐えることを定め、スレッド規約 5 の「unload の後にホストを呼ばない」の例外が `release` だけであることを明記する。
4. Where v3 互換モードを使うとき, the 版の規約 shall load の host に `copy_from_slice` だけを持つテーブル（`request`・`post`・`log` は NULL）を渡すと定める。`Ghost::load` に渡す host は、v3 ではこれまでどおり `None` とする。
5. If ホストのテーブルに `copy_from_slice` が無い（`struct_size` が足りない・NULL・v3 で host が NULL）なら, the 新しい SHIORI shall load を失敗させる（自分のメモリで作る昔の形へ戻らない）。
6. The 規約の変更 shall C ヘッダと `shiori4_abi` の `//!` の両方に同じ文で入り、`shiori4_abi`・`shiori4`（`export_shiori!`・`Adapter`）・`shiori4_host`（`Bridge`・`V3Runtime`・`from_static`）・`testdata/testghost` がそれに合わせて直され、既存のテストが通る。
7. The 0.0.3 の見どころ shall 0.0.2 以前と二進互換でないこと（関数の形は変わらないが、`out` を作る側が変わる）を述べ、pasta と areka への知らせに含める。

## 決定事項（要件ディスカッション 2026-10-24）
brief.md で決め切れず、要件ディスカッションで開発者がすべて推奨案で確定した所である。

1. **Q1 読み物の置き場所**: 本文はリポジトリの `doc/` に置き、README と docs.rs から GitHub の URL で導く（要件 1.5）。`doc/host-protocol.md` と同じ扱いで、C ヘッダと `//!` の同文の約束を崩さない。
2. **Q2 最小の SHIORI の例の版**: hearing の「v3 だけで 30 行ほど」は「30 行ほどで足りる」と読む。Rust の例は `export_ghost!` で v3・v4 の両方を出し、C の例は v4 だけを返す。v3 も返す形は版の判定の例で示す（要件 3.2）。
3. **Q3 規約の番号**: 番号を C ヘッダと `//!` の両方に書き入れる（番号だけで文言は変えない）。スレッド規約 1〜7 はそのままで、「規約 N」はスレッド規約を指し続ける。ほかの節は「節の名前 ＋ 番号」。教科書だけに付けると上流が項目を足したときにずれるため（要件 4.2・4.3）。
4. **Q4 規約の一覧の中身と理由**: 一覧は正本の文を写し、写しが正本と一致することを CI で確かめる。理由は教科書にだけ置く。記録から引けない理由は印を付けた草案にして、統合の前に開発者が確かめる（要件 4.1・4.5・4.8・14.6）。
5. **Q5 確定の範囲**: 文書に書かれた語はその版で確定、文書に無いものは「決まっていない」。冒頭に 0.0.x の間は版ごとに互換を壊す変更がありうることを 1 行書く。語ごとの安定度は決めない（要件 5.5・5.6）。
6. **Q6 Win32 のメッセージの輪の例**: 外部の crate を足さず、使う Win32 の関数を例の中で宣言して、Windows でビルドまで検査する（要件 8.3）。
7. **Q7 移行の手引きと shiori3-legacy-host**: 今は「旧来の DLL を読む口は shiori4_host にまだ無い」と書き、統合後の書き換えを shiori3-legacy-host の brief に申し送る（要件 11.3）。
8. **Q8 試験用のホストの post**: 要求の後に 1 回 pump して post と log を表示する。完了通知は返さず、届けていない post は unload で捨てる（要件 13.5）。
9. **Q9 試験用のホストの形と配り方**: `shiori4_host` の example 1 本。公開するクレートは増やさない。pasta などは `cargo install --git`（`--example` 付き）で取り込む。引数の形と、`--example` での取り込みが実際に動くかは設計で決め・確かめる（要件 13.9）。
10. **Q10 C の例の検査**: ビルドと実行まで行う。言語を入れ替えた組（C のホスト × `testdata/testghost`、試験用のホスト × C の SHIORI の例）で往復を確かめる。CI を変える PR なので CI を見届けてから統合する（要件 14.2）。
11. **Q11 全体図の穴と ABI の所有権**（設計の議論 #1、2026-10-24）: スレッド規約 1 を `load`・`unload` まで広げる。`ShioriBytes` は常にホストが作り（SHIORI は `host->copy_from_slice` で作る）、`release` は常にホストの関数で、いつでも・どのスレッドからでも呼べる。v3 にも `copy_from_slice` だけを持つテーブルを渡す。Rust の型に lifetime は足さない。メモリ確保の簡易版は保留し、問題になったら別の spec に切り出す（要件 15）。
