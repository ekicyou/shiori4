# 規約の一覧と理由

shiori4 の ABI の規約を、正本の節の順に 1 つ残らず写し、各規約に番号と短い理由を添えた一覧である。教科書の目次は [doc/textbook/README.md](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/README.md) にある。

## この一覧の読み方

### 正本と写し

- 規約の正本は、C ヘッダ [`shiori4_abi/include/shiori4_abi.h`](https://github.com/ekicyou/shiori4/blob/main/shiori4_abi/include/shiori4_abi.h) の冒頭の注釈と、`shiori4_abi` のクレート冒頭の `//!`（[docs.rs](https://docs.rs/shiori4_abi)）の 2 つで、同じ文を持つ。食い違ったら正本が正しい。
- この一覧の引用（`>` で始まる行）は、正本の文を一字一句そのまま写したものである。綴りは `//!` の形（`host.request`）で写す。C ヘッダの `host->request` と同じものを指す。
- 写しが正本と一致することは CI が確かめる（`shiori4_abi/tests/textbook.rs`）。正本が変わって写しがずれると、PR の検査が失敗する。
- 理由は、この一覧にだけ置く。C ヘッダと `//!` には足さない。

### 番号の付け方と引用の仕方

- スレッド規約は「スレッド規約 N」と引く。短く「規約 N」と書いたら、スレッド規約 N を指す。
- ほかの 8 つの節は「<節の名前> N」と引く。節の名前は見出しから括弧の部分を除いたもので、「版 N」「ABI 規約 N」「戻り値 N」「load の規約 N」「ホストのプロセス N」「DLL の口と発見の順 N」「post の完了通知 N」「配布の決まり N」になる。
- 番号は各節の最上位の項目にだけ付いている。字下げした下位の項目（`- ` の行）は、親の番号で引く。たとえば「post の完了通知 3」は、post の呼び出し中に届けないことと、unload で未配送を捨てることを含む。

### 番号を使い回さない

- 一度公開した版で付けた番号は、後の版で別の規約に使わない。
- 規約が足されたら、その節の末尾に新しい番号を足す。
- 規約をやめるときは、番号を残して「（廃止）」と書く。

### 理由と出典

- 各規約の直後の **理由** は 1〜2 文で、括弧の中に出典を添える。長い経緯は出典の側にある。
- 出典の略し方は次のとおり。
  - 「abi-conventions 論点 N」: [`.kiro/specs/completed/abi-conventions/brief.md`](https://github.com/ekicyou/shiori4/blob/main/.kiro/specs/completed/abi-conventions/brief.md) の論点 N。「abi-conventions 要件 N」は同じ spec の [`requirements.md`](https://github.com/ekicyou/shiori4/blob/main/.kiro/specs/completed/abi-conventions/requirements.md) の要件 N、「abi-conventions の設計」は [`design.md`](https://github.com/ekicyou/shiori4/blob/main/.kiro/specs/completed/abi-conventions/design.md)。
  - 「host-protocol 論点 N」: [`.kiro/specs/completed/host-protocol/brief.md`](https://github.com/ekicyou/shiori4/blob/main/.kiro/specs/completed/host-protocol/brief.md) の論点 N。
  - 「abi-textbook 設計の議論 #N」: この教科書の spec（`.kiro/specs/abi-textbook/design.md`）の「設計の議論の決定」の N（理由の詳しい文は同じ spec の `research.md` の「設計の議論 #1 の決定」「#2〜#11 の決定」にある）。「hearing N」は同じ spec の `hearing.md` の節 N。
  - 「tech.md」「product.md」「roadmap.md」: [`.kiro/steering/`](https://github.com/ekicyou/shiori4/tree/main/.kiro/steering) の各文書。
  - 「host-protocol.md 第 N 節」: [`doc/host-protocol.md`](https://github.com/ekicyou/shiori4/blob/main/doc/host-protocol.md)（ホスト側プロトコル仕様）の節。
- 記録から引けなかった理由は **理由（草案・記録に無い）** と書く。開発者が確かめたら印を外す。

## 版（abi_version）

> 1. SHIORI_ABI_VERSION（4）: 既定。host 必須。ペイロードはヘッダ部分だけ（開始行なし）
>    - SHIORI は copy_from_slice があるかを確かめ、無ければ load で NULL を返す

**理由**: v4 の要は SHIORI からホストを呼べること（同期の `host.request` と非同期の `post`）なので、host を必ず渡す（product.md の「Core Capabilities」）。応答はホストの `copy_from_slice` でしか作れない（ABI 規約 5）ので、それが無いテーブルでは load を続けられない（abi-textbook の要件 15.5）。

**理由**: 開始行を持たないのは、要求の種類（GET・NOTIFY）を関数の口（`request`・`notify`）が、版を `shiori_get_api` の引数と `abi_version` が受け持ち、開始行に載せるものが残らないためである。

> 2. SHIORI_ABI_VERSION_3（3）: SHIORI3 互換モード。異なる OS の互換ベースウェア向け
>    - load の host は copy_from_slice だけを持つテーブル（abi_version は 3。request・post・log は NULL）。
>      SHIORI は copy_from_slice があるかだけを確かめる。ほかのホストの呼び出しは使えない
>    - request の req は SHIORI/3.0 の要求全文（"GET SHIORI/3.0" / "NOTIFY SHIORI/3.0" で始まる）
>    - request の out は SHIORI/3.0 の応答全文（"SHIORI/3.0 200 OK" で始まり空行で終わる）
>    - 戻り値は開始行と同じステータスコード（正本は文字列）。負の値は ABI エラー（out は空。500 として扱う）
>    - notify の req は "NOTIFY SHIORI/3.0" で始まる全文。GET なら 400
>    - 文字コードは UTF-8 固定（Charset: UTF-8）。SHIORI/2.x の要求には 400 を返す
>    - v3 は v4 と同じ機能を出すことを保証しない

**理由**: v3 互換モードは、Windows 以外のベースウェアを組みたい人のために、HGLOBAL に依存しない SHIORI/3.0 の口を用意したもので、従来の SHIORI/3.0 の DLL を置き換えるものではない（roadmap.md、abi-conventions 論点 16）。v3 にも `copy_from_slice` だけを持つテーブルを渡すのは、v3 でも応答の `ShioriBytes` をホストが作るためである（abi-textbook 設計の議論 #1）。

> 3. v4 の文字コードは UTF-8 固定。ホストは要求に Charset ヘッダを付けなくてよい。付いていても
>    SHIORI は無視する。SHIORI は応答に Charset を付けない

**理由**: v4 は UTF-8 しか運ばないので、両者が余計な Charset を付け合ったり、その値で要求を断ったりしないようにした（abi-conventions 論点 7・要件 9）。

> 4. ホストは shiori_get_api(v) で版を選ぶ。返るテーブルの abi_version は v と一致する

**理由**: ABI の版はヘッダに載せず、`shiori_get_api` の引数と、返るテーブルの `abi_version` で分かるようにした（host-protocol 論点 6、host-protocol.md 第 2.6 節）。

## ABI 規約

> 1. shiori4 の書き出しは shiori_get_api の 1 つだけ。対応できない版なら NULL を返す
>    （SHIORI/3.0 の書き出しと並べてよい。「DLL の口と発見の順」を参照）

**理由**: 公開するシンボルを `shiori_get_api` 1 つと関数テーブルに絞り、所有権・スレッド・版の決まりをそのテーブルで固定する（product.md）。旧来の SHIORI/3.0 の DLL とは `shiori_get_api` の有無で見分けるので、SHIORI/3.0 の書き出しと並べても読み分けられる（product.md の「Target Use Cases」、abi-conventions 論点 13）。

> 2. DLL 境界を越えるのは C のレイアウトの構造体と関数ポインタだけ

**理由**: ABI はバイト列の運搬だけを受け持ち、小さく、言語に依存しない形に保つ（tech.md の「ABI はバイト列の運搬だけ」）。C ヘッダが正本なので、Rust 以外の言語でも同じ形を作れる（product.md）。

> 3. ペイロードは UTF-8 のバイト列（ptr と len）。NUL 終端にはしない

**理由**: 芯が運ぶ文字列に求めるのは UTF-8 であることだけで、中身は解釈しない（tech.md の「芯は中身を解釈しない」）。中身の形が替わっても、芯を変えずに済む（同じ所）。

> 4. 引数の (ptr, len) は呼び出しの間だけ有効な借用。保持する場合は受け取った側がコピーする

**理由**: 所有権が移るのを `out` の 1 か所だけにし（ABI 規約 5）、ほかの引数は呼んだ側のメモリを貸すだけにすれば、誰がいつ解放するかで迷わない。呼び出しが返った後のメモリに頼る実装は、ホストの作り方しだいで壊れる。

> 5. 所有権が移るのは out に書き込まれる ShioriBytes だけ。out の ShioriBytes は常にホストが作り、
>    ホストの release で解放する（SHIORI は応答を host.copy_from_slice で写して作る）。
>    写せなかったときは、ホストが自分で決めた印を out に残し（SHIORI には知らせない）、
>    SHIORI は out をそのまま返す

**理由**: ホストが `ShioriBytes` を作れば `release` が常にホストのコードになり、所有権の決まりが 1 通りになって、外れた DLL の `release` を呼ぶ道が ABI から無くなる（abi-textbook 設計の議論 #1）。写せなかったときの扱いはホストの仕事で、SHIORI は写せたかを知らなくてよいので、`copy_from_slice` は戻り値を持たない（abi-textbook 設計の議論 #3）。

> 6. 呼び出された側は out を最後に書く。呼び出した側は戻り値に関係なく out を解放する

**理由**: 解放の手順を戻り値で分けないので、負の戻り値（`out` は空）のときも同じ後始末の道を通り、漏れも二重の解放も起きにくい。呼ばれた側が `out` を最後に書けば、途中で失敗したときに書きかけの `out` が残らない。

> 7. 構造体は末尾にだけフィールドを追加する（追加する関数ポインタは NULL 許容）。
>    受け取った側は struct_size を見て、フィールドがあるかを判定する

**理由**: 構造体を末尾にだけ足し、`struct_size` で有無を見分ければ、版を上げずに後方互換を保てる（product.md の「Value Proposition」）。

> 8. エクスポートする関数は、例外・パニックを境界の外へ出さず SHIORI_E_PANIC を返す

**理由**: 例外やパニックが C の境界を越えたときの振る舞いは言語ごとに定まらず、呼んだ側のプロセスを壊しうる。規約を言語によらない形で書いたのは、Rust 以外の実装者も同じ規約を守れるようにするためである。

## 戻り値（ShioriStatus）

> 1. 0 以上: SHIORI/3.0 のステータスコード（200, 204, 400, 500 など）

**理由**: v4 のステータスコードは SHIORI/3.0（ukadoc）のとおりとし（host-protocol.md 第 2.1 節）、v3 と v4 で応答のコードを分けない。拒否の返し方で 403 を足さなかったのも、ステータスを SHIORI/3.0 の集合に収めるためである（host-protocol 論点 8）。

> 2. 負の値: ABI 層のエラー（SHIORI_E_*）。out は空。SHIORI_E_PANIC ならホストはインスタンスを unload する

**理由**: 負の値を ABI 層に取っておけば、運搬の失敗と SHIORI/3.0 のステータスが混ざらない。パニックの後のインスタンスは状態が壊れているかもしれないので、使い続けずに unload する。

## スレッド規約

> 1. ShioriApi の load / request / notify / unload は、ホストが決めた単一のスレッドからだけ呼ぶ。
>    このスレッドはインスタンスごとに決まり、プロセスで 1 つではない。
>    同じ DLL の 2 つのインスタンスは、別々のスレッドから呼ばれうる

**理由**: Rust の `Ghost` は `Send` を求めないので、`load`・`unload` を別のスレッドで呼ぶと Rust の SHIORI で UB になる（abi-textbook 設計の議論 #1）。スレッドをインスタンスごとにしたのは、areka が 1 つのプロセスで複数のゴーストを別々のスレッドで動かすためである（abi-conventions 論点 8・9）。

> 2. SHIORI が host.request を呼べるのは、load / request / notify を呼ばれているスレッドから、
>    その呼び出しが返るまでの間だけ（v4）。
>    ホストは、これに反する呼び出しを見分けたら SHIORI_E_WRONG_THREAD を返してよい（out は空）。
>    判定はインスタンスごとに行う

**理由**: ホストの UI スレッドが `request` の応答を待っている間に、SHIORI がワーカースレッドから `host.request` を呼ぶと、その問い合わせに UI スレッドで答えようとするホストは互いに待って止まる。ABI は厳しいまま保ち、違反は `SHIORI_E_WRONG_THREAD` で見分けられるようにした（abi-conventions 論点 1・要件 2、hearing 6）。

> 3. host.request の処理中に、ホストから SHIORI を呼び返してはならない（SHIORI_E_REENTRANT）

**理由**: `host.request` の処理中、SHIORI は自分の `load`・`request`・`notify` の途中で止まっている。そこへ呼び返すと、1 つのインスタンスが 2 つの呼び出しを同時に抱え、作りかけの状態のまま次の要求を受けることになる（Rust の `Ghost::request` は `&mut self` を取るので、その前提が崩れる）。

> 4. host.post と host.log は任意のスレッドから呼べる。ホストは msg をコピーしてすぐ返る
>    （post はキューに積む）

**理由**: SHIORI がワーカースレッドからもログや依頼を出せるよう、`log` も `post` と同じく任意のスレッドから呼べるようにした（abi-conventions 論点 3・要件 5）。ホストがすぐ返らずに `GhostRuntime` を Drop するスレッドを同期で待つと、実行中の `post`・`log` が返るのを待つ unload と互いに待ち合って止まるので、すぐ返ることをホストの義務にした（abi-conventions の設計の `ShioriHostPoster` と `shiori4_host` の項）。

> 5. ShioriHostApi は unload が返るまで有効。SHIORI は、unload が返った後に ShioriHostApi の
>    どの関数も呼ばない（例外は ShioriBytes.release だけ。規約 6）。
>    unload を始めたホストは post に SHIORI_E_SHUTDOWN を返してよい。
>    unload は長く止まらないこと。ホストは OS の終了の猶予の中で、各インスタンスの unload を
>    呼ぶことがある

**理由**: 以前の「post するスレッドをすべて止めてから返る」は手段の指定で守りにくかったので、「unload が返った後にホストを呼ばない」という結果の約束に替えた。高水準層の `ShioriHostPoster` が unload の開始で無効になるので、Ghost の作者は join を書かずに守れる（abi-conventions 論点 2・10・要件 4）。

> 6. ShioriBytes.release は常にホストの関数で、いつでも・どのスレッドから呼んでもよい
>    （インスタンスの unload の後でもよい）。host.copy_from_slice も、どのスレッドから呼んでもよい。
>    ホストはそれに耐えること

**理由**: 応答を別のスレッドで捨てられるよう、`release` をどのスレッドからも呼べるようにした（abi-conventions 論点 3・要件 5）。`release` を常にホストの関数にしたので、unload の後やライブラリを外した後に応答を捨てても、外れた DLL のコードを呼ばない（abi-textbook 設計の議論 #1）。

> 7. notify を即 204 で返し、処理を後で行う実装は合法である。後回しにした処理の中では
>    host.request を使えない（規約 2 の期間の外であるため）。ホストへの依頼には post を使う

**理由**: ホストのスレッドを止めずに重い処理ができるよう、notify の後回しを合法とした（abi-conventions 論点 4・要件 6）。後回しにした処理はスレッド規約 2 の期間の外なので、ホストへの依頼は任意のスレッドから呼べる `post` で行う（同じ所）。

## load の規約

> 1. dir は、v4・v3 互換モードとも、絶対パス・OS の区切り・末尾に区切りあり・UTF-8 の文字列
>    （呼び出し中だけ有効）

**理由**: SSP が SHIORI/3.0 の load に渡す形に合わせた（abi-conventions 論点 6）。形が決まっていれば、`dir + "file"` のような書き方でも、どのホストでも同じに動く（abi-conventions 要件 8）。

> 2. カレントディレクトリはホスト次第である。SHIORI はカレントディレクトリに頼らず、load の dir を使う

**理由**: カレントディレクトリの扱いはホストごとに違い（areka は helper のカレントディレクトリをゴーストの ghost/master にする）、規約では決めなかった（abi-conventions 論点 14、hearing の論点 d）。カレントディレクトリはプロセスに 1 つなので、複数のゴーストを別々のスレッドで読むと競争になる（abi-conventions 要件 8.4）。

> 3. v4 の load が失敗するときは、NULL を返す前に host.log で理由を出す。v3 は host に log が無いので、
>    SHIORI 自身のログに出す。v3 の load は失敗の理由を返さない（NULL を返すだけ）

**理由**: NULL だけではホストに失敗の理由が分からず、読めなかったゴーストを正しく扱えないので、v4 では NULL の前に `log` で理由を残す（abi-conventions 論点 5・要件 7）。v3 の load は今の規約のまま理由を返さず、v3 の host には `log` が無いので、理由は SHIORI 自身のログに出す（abi-conventions 論点 16）。

> 4. SHIORI は同じプロセスでの 2 つ目のインスタンスを拒否してよい（NULL ＋ log）。拒否されたホストは、
>    そのゴーストを読めなかったものとして扱い、ゴーストごとに別のプロセスで読むか、拒否を受け入れる

**理由**: areka は 1 つのプロセスで複数のゴーストを動かし、同じ DLL を 2 体が指すと load が 2 回呼ばれる（abi-conventions 論点 8）。2 つ目を受けられない SHIORI のために、NULL ＋ log で断る道を残した（abi-conventions 論点 5・8）。

> 5. SHIORI は、できれば複数のインスタンスを受けるべきである（状態をインスタンスごとに持ち、
>    グローバルな状態を避ける）

**理由**: 1 つのプロセスで複数のゴーストを動かすホスト（areka）のために、断る道（load の規約 4）を保ったうえで推奨として書いた（abi-conventions 論点 8）。pasta もこの推奨に合わせて、状態をインスタンスごとに持つ作りにすると決めた（同じ所）。

## ホストのプロセス

> 1. ホストは SHIORI を専用のプロセスで読むことがある。SHIORI は、ホストと同じプロセスにいること
>    （ホストの窓・プロセスの状態を共有すること）を前提にしない

**理由**: areka は v4 の DLL も同じプロセスでは読まず、SHIORI ごとに helper のプロセスを起こして、その中で `shiori4_host` を使う（abi-conventions 論点 11）。ABI の形は変わらない（同じ所）。

> 2. host.request は IPC を往復することがあり、遅くなりうる

**理由**: ホストのプロセス 1 のとおり、ホストが SHIORI を別のプロセスに置くと、`host.request` は親のプロセスまで往復する（abi-conventions 論点 11）。

## DLL の口と発見の順

> 1. 1 つの DLL が、SHIORI/3.0 の書き出し（load・request・unload）と shiori_get_api の両方を持ってよい

**理由**: pasta は 1 つの DLL で両方の口を出す（hearing の論点 b）。`shiori_get_api` を知らない古いホストは、今までどおり従来の口で読める（abi-conventions 論点 13・要件 11.9）。

> 2. ホストは書き出しを shiori_get_api → loadu → load の順に探し、最初に見つかった口で解決する。
>    shiori_get_api が見つかったら 4、だめなら 3 を試す。どちらも NULL なら読めなかったものとして扱い、
>    loadu・load へは進まない。解決した口の load が失敗しても、ほかの口へは進まない

**理由**: 同じ DLL を 2 つの口で初期化しないためである（abi-conventions 論点 13）。`shiori_get_api` を持つ DLL は v4 だと宣言しているので、その失敗を旧来の口で取り繕うと、二重の初期化か作者の意図と違う動きになる（abi-conventions 論点 15）。

## post の完了通知（v4）

> 1. 受付 ID は SHIORI が発行し、Tag ヘッダとしてメッセージに入れる（登録してから post する）

**理由**: 完了は post とは別の時点でイベントループから届くので、登録より先に post すると、完了が先に届いたときに文脈が見つからない（`shiori4::protocol::PendingTags` の doc、abi-textbook の設計の ShioriExamples）。SHIORI が ID を発行すれば、post より前に登録できる（同じ所）。

> 2. post の戻り値は受付可否だけ（204: 受付、400: 不正、SHIORI_E_SHUTDOWN: 終了中）

**理由**: post の結果は専用の API ではなく、`Tag` の付いた request・notify として後から届ける（tech.md の「完了通知は通常のイベント」）。post の呼び出しはキューに積んですぐ返る（スレッド規約 4）ので、戻り値が伝えるのは受け付けたかどうかだけになる。

> 3. 完了はホストが通常のイベント（request または notify）として、post したインスタンスに届ける
>    - post の呼び出し中や host.request の処理中には届けない（イベントループから届ける）
>    - unload を始めたら、そのインスタンス宛ての未配送イベントは破棄する

**理由**: post はどのスレッドからも、SHIORI の処理の途中からも呼ばれるので、その中で完了を届けると、決まったスレッドの外から SHIORI を呼んだり（スレッド規約 1）、処理中の SHIORI を呼び返したり（スレッド規約 3）することになる。そのためホストのイベントループから届け、unload を始めたインスタンスにはもう届けられないので、未配送のイベントは捨てる（host-protocol.md 第 6.8 節も、この規約に従って unload に間に合わない完了を出さなくてよいとする）。

> 4. Tag を付けない post は「結果は不要」を意味する

**理由**: 結果を待たない依頼に受付 ID の発行と登録を求めず、ホストにも届け先の無い完了通知を作らせずに済む。

> 5. 運ぶ中身の語（Command・Tag・OnHostComplete の並び・拒否の返し方など）は、shiori4 リポジトリの doc/host-protocol.md（ホスト側プロトコル仕様）が定める

**理由**: ABI と芯は運ぶ文字列の中身を解釈しないので、語は別の文書に置く。中身の形が替わっても（JSON-RPC など）、芯を変えずに済む（tech.md の「芯は中身を解釈しない」、host-protocol の brief の「方針」）。

## 配布の決まり（DLL の選び方）

> 1. descript.txt の shiori,<名前> から名前の幹を取る（例 shiori,pasta.dll → pasta）

**理由**: descript.txt の `shiori,pasta.dll` に対して、Windows 以外のホストがどのファイルを探すかが決まっていなかった（hearing の論点 c）。名前の幹から、OS と arch ごとのファイル名を決める（abi-conventions 論点 12）。

> 2. ホストは、ホストの OS と arch ごとに次の順で探す
>    - Windows x64: <幹>.x64.dll → <幹>.dll
>    - Windows arm64: <幹>.arm64.dll →（x64 のプロセスを起こせるホストだけ）<幹>.x64.dll → <幹>.dll
>    - Linux x64: lib<幹>.x64.so → lib<幹>.so
>    - Linux arm64: lib<幹>.arm64.so → lib<幹>.so
>    - macOS: lib<幹>.dylib（ユニバーサルバイナリを推奨）

**理由**: arm64 の Windows で x64 の DLL を候補にするのが x64 のプロセスを起こせるホストだけなのは、arm64 のプロセスが x64 の DLL を読めないためである（ARM64EC は除く。abi-conventions 論点 12）。

> 3. 接尾辞は x64・arm64 の 2 つだけ。x86 は作らない。接尾辞なしの Windows の DLL が、
>    32bit の SHIORI/3.0 を担う

**理由**: 接尾辞なしの `pasta.dll` が、SSP の読む 32bit の SHIORI/3.0 を担うので、x86 の接尾辞は要らない（abi-conventions 論点 12）。x86 の v4 の DLL を別のプロセス越しに読むことは、この規約の外にある（abi-conventions の brief の Out）。

> 4. Linux と macOS は lib の付いた名前だけを探す。macOS は arch の接尾辞を持たない

**理由**: `lib` の付いた名前は共有ライブラリの大原則で、Rust の cdylib もそう作る（abi-conventions 論点 12）。macOS はユニバーサルバイナリ 1 つを前提にし、arch ごとに分けたい作者が出たら `lib<幹>.<arch>.dylib` を足せる（今の規則とぶつからない。同じ所）。

> 5. ホストは選んだファイルの機械の種類をヘッダ（PE・ELF・Mach-O）で確かめ、合わなければ次の候補へ
>    進み、候補が尽きたらそのゴーストを読めなかったものとして扱う。これは SHIORI/3.0 と v4 の両方に使える

**理由**: DLL を読まずに（初期化のコードを動かさずに）、どの arch のどのファイルを読むかを決めるためである。areka では DLL を選ぶのが helper のプロセスを起こす前の本体の側なので、ヘッダを見て決める（abi-conventions 論点 12・要件 14）。

## ホスト側プロトコルの決まりの理由

ここは C ヘッダの規約ではなく、ホスト側プロトコル仕様の決まりの理由である。番号は付けない。

### SEND は既定で割り込む

host-protocol.md 第 5.1 節は、post の SEND が既定で今のスクリプトに割り込み、`Option: nobreak` のときだけ今のスクリプトが終わるまで待つ、と定める。

**理由**: post は「同じプロセスからの SSTP」として扱うので、既定と待ち方を SSTP に揃えれば、語・既定・ukadoc の参照・areka の既存の裁定を 1 つにできる。shiori4 の当初の推奨は「既定は待つ」だったが、3 者会談でこの理由から改めた（host-protocol 論点 9）。
