# Requirements Document

## Project Description (Input)
SHIORI 実装者（pasta）とベースウェア実装者（areka）が、v4 で SHIORI とホストが交わすメッセージの語彙を必要としている。今の語彙は `shiori4::host_protocol` に「暫定」として置かれているだけで、プロパティの読み（`ID: property`）・post の `Command` と `Tag`・完了通知 `OnHostComplete` の名前しかない。プロパティの書き込み、post のコマンド、拒否の返し方、ホストが必ず付けるヘッダ、SHIORI の名前と版をホストが知る手段が決まっていない。pasta にとって v4 の最大の利点である「SHIORI から今すぐ台本を出す」ことを表す語も無い。areka は、自分の spec `shiori4-api` の上流として参照できる正本を必要としている。

いま、`shiori4_host` の `Bridge` は組み込みで `property` に 200 ＋ `Value` か 204 で答え、`accept_post` の既定は `Command` の有無を見る。完了通知は `Completer` が `OnHostComplete` の並びを組み立てる。つまり芯（ABI の口と runtime）が中身を解釈している。

2026-10-23 に pasta・areka・shiori4 の 3 者で論点 1〜12 を合意し、開発者は「芯は中身を解釈しない。パースは使いたい人が使う便利関数として独立したモジュールで出す」と裁定した（brief.md）。この spec では、合意した語をホスト側プロトコル仕様の文書として shiori4 リポジトリに置いて 3 者の正本にし、芯から中身の解釈を外し、語に合わせた便利関数を独立したモジュールとして提供する。

## Introduction
この spec は 3 つのものを持つ（brief.md の Boundary Candidates）。

1. **ホスト側プロトコル仕様の文書**（語彙の正本）: brief.md の合意した論点 1〜12 を、pasta・areka・shiori4 が同じ一通りの意味に読める形で書く。runtime の振る舞いとしては持たない。
2. **芯から中身の解釈を外すこと**: `shiori4`・`shiori4_host` の芯は、やり取りされる文字列が UTF-8 であることだけを求め、中身（ID・Command・Tag など）を読まない。
3. **パースのための便利関数のモジュール**: 仕様の語に合わせた解析・組み立ての関数を、芯から呼ばれない独立したモジュールとして提供する。使いたい人だけが使う。

要件の番号と brief.md の論点の対応は、各要件の目的の末尾に「（論点 N）」で示す。brief.md で決め切れていない所は、要件ディスカッションで確定させた（末尾の「要件ディスカッションの結果」）。

## Boundary Context
- **In scope**:
  - ホスト側プロトコル仕様の文書（論点 1〜12）と、各クレートの doc・README からその文書への導線。
  - 芯から中身の解釈を外すこと: `shiori4_host` の `Bridge`（組み込みの `property` の応答をやめる）・`Host::accept_post` の既定（全部を受け付ける）・完了通知（ヘッダの全文を渡す口だけにする）。SHIORI 側の芯（`Ghost`・`Adapter`）が v4 の中身を解釈しないこと。
  - 便利関数のモジュール（移すもの: `Headers`・`HeaderWriter`・`host_protocol` の定数・`OnHostComplete` の組み立て・`PendingTags`。足すもの: `getproperty`／`setproperty` と `SEND`／`NOTIFY` の組み立て、新しい語の定数）。
  - 上の振る舞いのテスト。
  - pasta・areka への正本の場所の連絡。
- **Out of scope**:
  - ABI の規約（スレッド・寿命・log・load の dir・Charset）。abi-conventions が確定させた。
  - SSTP そのもの（ネットワーク越しの SSTP の受け口）。
  - ukadoc のプロパティの一覧を shiori4 が持つこと（名前の決まりだけを持つ）。
  - ベースウェアの再生キューの実装、語の守らせ方（ホストが SEND を割り込ませる・待たせる・`Reason` を返す・必須ヘッダを付ける）。ホストの実装が持つ。
  - pasta のシーン再生の仕組み、areka の SSTP の受け口・安全度の扱い。
  - 知らない `Command` の post の扱いの決め事（芯は中身を見ず、便利関数も判定しない）。
  - JSON-RPC など別の語の便利関数（将来、別のモジュールとして並べる）。
  - 版を上げること（0.0.2 は `/release` が出す）。
- **Adjacent expectations**:
  - abi-conventions（完了済み）が `Bridge` の `ShioriHost::request` の先頭に置いた規約 2 の門（期間の外・別スレッドの `host->request` に `Host` を呼ばず -7 と空の out を返す）は、振り分けを書き替えても先頭に残す。
  - abi-conventions のテスト `builtin_responses_have_no_charset` は組み込みの応答を前提にしているので、組み込みの応答をやめるときに「芯と便利関数が作るものに Charset が無い」ことを守るテストとして作り直す。
  - `Host::log`・`Host::accept_post`・`Host::wake` は任意のスレッドから呼ばれ、`GhostRuntime` を Drop するスレッドを同期で待ってはいけない（abi-conventions の申し送り）。
  - areka の spec `shiori4-api`・pasta の v4 対応（0.5.0）・abi-textbook（語の表）は、この spec の文書を上流として参照し、語を写して変えない。areka は、この spec が入った shiori4 の版が crates.io に出てから着手する。

## Requirements

### Requirement 1: ホスト側プロトコル仕様の文書と正本
**Objective:** As a SHIORI 実装者（pasta）・ベースウェア実装者（areka）, I want v4 の語彙が shiori4 リポジトリの 1 つの文書に、一通りの意味で書かれていること, so that 3 者が同じ正本を参照し、互いの文書に語を写して変えずに済む（全論点）

#### Acceptance Criteria
1. The ホスト側プロトコル仕様 shall shiori4 リポジトリの中の 1 つの文書として、日本語で、要件 2〜7 の語彙（brief.md の論点 1〜12）をすべて持つ。
2. The ホスト側プロトコル仕様 shall 「shiori4 の芯（ABI の口と runtime）は中身を解釈せず、語を守らせるのはホストと SHIORI の実装である」ことと、「語に合わせた便利関数は使いたい人が使うもので、使わなくても仕様に従える」ことを述べる。
3. The ホスト側プロトコル仕様 shall ヘッダの書式が SHIORI/3.0 と同じ（`名前: 値`、CRLF 区切り、ヘッダ部分だけで開始行と終端の空行が無い）ことと、Charset を付けないこと（abi-conventions）を述べる。
4. The ホスト側プロトコル仕様 shall その語彙が当てはまる shiori4 の版（この spec が入る版）を述べる。
5. The shiori4・shiori4_host の doc と README、および `host_protocol` に当たる便利関数のモジュールの doc shall ホスト側プロトコル仕様の文書を指し、語彙を「暫定」と書かない。
6. The ホスト側プロトコル仕様 shall ukadoc（SHIORI/3.0・SSTP・プロパティシステム）の語をそのまま使う所では、ukadoc を参照先として示し、意味を書き換えない。
7. The ホスト側プロトコル仕様 shall ヘッダ名について、受け手は大文字小文字を区別せずに読み、送り手は ukadoc の綴りで書くと定める。

### Requirement 2: プロパティの読み書きと名前の決まり
**Objective:** As a SHIORI 実装者, I want プロパティの読みと書きの要求と応答が get / set の対で決まっていること, so that ベースウェアの状態を同期で読み、変えられる（論点 1・4）

#### Acceptance Criteria
1. The ホスト側プロトコル仕様 shall プロパティの読みを、要求 `ID: getproperty` ／ `Reference0: <名前>`、応答 200 ＋ `Value: <値>`（存在する）・204（存在しない）と定める。
2. The ホスト側プロトコル仕様 shall プロパティの書き込みを、要求 `ID: setproperty` ／ `Reference0: <名前>` ／ `Reference1: <値>`、応答 204（成功）・400（不正）と定める。
3. The ホスト側プロトコル仕様 shall 0.0.1 の `ID: property` を `getproperty` に改めたことと、`property` を受ける互換の扱いを持たないことを述べる。
4. The ホスト側プロトコル仕様 shall プロパティの名前に ukadoc のプロパティシステムの名前をそのまま使い、ベースウェア固有の名前は接頭辞（例 `areka.`）で分け、名前の大文字小文字を区別しないと定める。
5. The ホスト側プロトコル仕様 shall 値の改行は取り除いて返し（エスケープして残さない）、改行を含む値が要るプロパティは別の名前で足すと定める。
6. The ホスト側プロトコル仕様 shall プロパティの一覧を持たず、一覧は ukadoc を参照すると述べる。

### Requirement 3: post の語彙と宛先
**Objective:** As a SHIORI 実装者, I want post で「今すぐ台本を出す」と「イベントを起こす」を SSTP と同じ語で言えること, so that v4 の利点を、ukadoc と areka の既存の裁定と同じ語で使える（論点 2・3・10・11）

#### Acceptance Criteria
1. The ホスト側プロトコル仕様 shall post を「同じプロセスからの SSTP」として扱い、コマンドを `Command` ヘッダの SSTP の語で表すと述べる。
2. The ホスト側プロトコル仕様 shall `Command: SEND` ＋ `Script: <さくらスクリプト>` を「その台本を再生する」と定める。
3. The ホスト側プロトコル仕様 shall `Command: NOTIFY` ＋ `Event: <イベント名>` ＋ `Reference*` を「ホストがそのイベントを SHIORI に送る」と定め、イベントの名前に制限を掛けないこと（台本の `\![raise]` と同じ）と、ホストが安全上の理由で特定のイベントを拒否（要件 7）してよいことを述べる。
4. The ホスト側プロトコル仕様 shall post の宛先を「post したインスタンス」に固定し、SSTP の宛先指定（`IfGhost` など）を読まないと定める。
5. The ホスト側プロトコル仕様 shall SEND と NOTIFY が読む SSTP の `Option` を、`nobreak`・`notranslate`・`strict` は ukadoc どおりに効き、`nodescript` は読まず、知らない語は無視し、2 つ以上なら ukadoc どおりカンマで区切ると定める。
6. The ホスト側プロトコル仕様 shall post の戻り値は受付の可否だけ（abi-conventions の C ヘッダの「post の完了通知」）であり、結果は `Tag` を付けた post の `OnHostComplete`（要件 6）で返ると述べる。
7. The ホスト側プロトコル仕様 shall `Tag` を中身を解釈しない文字列（CR・LF を含まない。数でなくてよい）とし、ホストは受け取った値を一字も変えずに `OnHostComplete` で返すと定める。値は `:` の後の空白を除いたものとし、送り手は値の前後に空白を置かない（設計ディスカッションで足した）。
8. The ホスト側プロトコル仕様 shall post に要るヘッダを `Command` と、そのコマンドのヘッダ（`Script`・`Event`・`Reference*`・`Option`・`Tag`）だけとし、SSTP の `Sender`・`Charset` は要らない（付いていてもホストは読まない）と定める。

### Requirement 4: SEND の割り込みと待ち
**Objective:** As a SHIORI 実装者・ベースウェア実装者, I want SEND が今のトークを切るのか待つのかが SSTP と同じ既定で決まっていること, so that 3 者が同じ再生の順を前提にできる（論点 9）

#### Acceptance Criteria
1. The ホスト側プロトコル仕様 shall post の SEND は既定で今のスクリプトに割り込み、`Option: nobreak` のときだけ今のスクリプトが終わるまで待つと定め、新しいヘッダ（`Interrupt` など）を作らない。
2. The ホスト側プロトコル仕様 shall SEND の優先度を Owned SSTP と同じ（SHIORI 自身の依頼）とし、今のスクリプトに `\t` が入っていても割り込めると定める。
3. The ホスト側プロトコル仕様 shall 待つための列はホストの仕事であると述べる。
4. The ホスト側プロトコル仕様 shall 割り込まれた側への知らせを次のとおり定める: 切られたトークが `Tag` 付きの SEND から始まったものなら、その `OnHostComplete` が `interrupted` を運ぶ。通常のイベントの応答として出たトークが SEND に切られたときは何も出さない。利用者の中断なら、今どおり `OnBalloonBreak` を出す。
5. The ホスト側プロトコル仕様 shall SEND による割り込みの知らせに `OnBalloonBreak` を使わないと述べ、その理由（ukadoc では「SSTP 以外でブレイクされた際」のイベントであること）を書く。
6. The ホスト側プロトコル仕様 shall NOTIFY で起こしたイベントへの応答の台本にも、要件 4.1〜4.2 の割り込みと待ち（既定は割り込み、`Option: nobreak` なら待つ）を当てはめると定める（ukadoc の SSTP で SEND と NOTIFY は同じ扱いのため）。

### Requirement 5: v4 の要求と応答のヘッダの語彙、SHIORI の名前と版
**Objective:** As a SHIORI 実装者・ベースウェア実装者, I want v4 の要求・応答のヘッダとイベントの語彙が SHIORI/3.0 と同じだと決まっていること, so that 既存の SHIORI/3.0 の知識と ukadoc をそのまま使える（論点 5・6）

#### Acceptance Criteria
1. The ホスト側プロトコル仕様 shall v4 のヘッダの名前と意味、ホストが送るイベント（ID と Reference の並び）、SHIORI の応答のヘッダ（`Value`・`ValueNotify`・`Marker`・`ErrorLevel`・`ErrorDescription` など）の名前と意味が ukadoc の SHIORI/3.0 と同じで、違いは開始行が無いことと Charset が無いことだけだと定める。
2. The ホスト側プロトコル仕様 shall ホストが v4 の要求に必ず付けるヘッダを `ID`・`Sender`（ベースウェアの名前）・`SenderType`（ukadoc の語）・`SecurityLevel`（`local` か `external` の 2 語だけ）・`SecurityOrigin`（ukadoc の形。`null` か起点）と定める。
3. The ホスト側プロトコル仕様 shall 状況によって付けるヘッダを、`Status`（伝える状態があるときだけ。ukadoc の語のカンマ区切り）・`BaseID`（ukadoc が定めるイベントだけ）・`Reference*` と定める。
4. The ホスト側プロトコル仕様 shall post から起きた要求（SEND の中の `\![raise]`、NOTIFY で起こしたイベント、post の完了を知らせる `OnHostComplete`）に、先頭の語が `internal` の `SenderType` と `SecurityLevel: local` を付けると定める。SEND の中の `\![raise]` で起きた要求は ukadoc どおり `raise` を足し（`SenderType: internal,raise`）、NOTIFY で起こしたイベントと `OnHostComplete` は `internal` だけとする。
5. The ホスト側プロトコル仕様 shall SHIORI の名前と版を SHIORI/3.0 と同じく、`ID: version`（ほかに `name`・`craftman`・`craftmanw`）の要求に 200 ＋ `Value:` で答え、答えなければ 204 とすると定める。
6. The ホスト側プロトコル仕様 shall ABI の版をヘッダに載せないこと（`shiori_get_api` の引数と `abi_version` で分かるため）を述べる。

### Requirement 6: OnHostComplete の届け方と並び
**Objective:** As a SHIORI 実装者, I want post の結果がどの届け方で、どのヘッダの並びで来るかが決まっていること, so that 待っている処理を `Tag` で再開し、結果に応じた台本を返せる（論点 7）

#### Acceptance Criteria
1. The ホスト側プロトコル仕様 shall `OnHostComplete` の届け方の既定を request とし、SHIORI は request と notify の両方で `OnHostComplete` を受けられなければならないと定める。
2. The ホスト側プロトコル仕様 shall `OnHostComplete` の共通のヘッダを `ID: OnHostComplete`・`Tag`（post の `Tag` の値）・`Status`（結果のステータスコード）・`Reference0: <コマンド名>` と定める。
3. The ホスト側プロトコル仕様 shall SEND の完了を「再生が終わった時」に出し、200（再生した。`Reference1` は `completed` か `interrupted`）・204（一度も再生せずに捨てた。`Reference1` は無し）・400（不正。拒否なら `Reason: forbidden` を付ける。要件 7）と定める。
4. The ホスト側プロトコル仕様 shall 204 になる場合の例（`nobreak` で待っている間に終了・切り替えが来た、ホストが受けられない状態だった）を挙げる。
5. The ホスト側プロトコル仕様 shall NOTIFY の完了を、`Reference1` は無しで、200（イベントを起こし、SHIORI がその要求に答えた。応答の台本の再生の終わりは待たない）・204（起こさずに捨てた。受けられない状態・終了・切り替え）・400（不正。拒否なら `Reason: forbidden` を付ける。要件 7）と定める。
6. The ホスト側プロトコル仕様 shall `OnHostComplete` への応答のスクリプトを `Option: nobreak` の SEND と同じに扱う（再生中なら終わるまで待ち、待つ間に終了・切り替えが来たら捨てる）と定め、その理由（`interrupted` の通知は割り込んだトークの再生中に届くので、通常の応答と同じに扱うと割り込んだ側をさらに置き換えてしまう）を書く。
7. The ホスト側プロトコル仕様 shall `OnHostComplete` の `Status` は結果のステータスコードを運び、要件 5.3 の ukadoc の `Status`（ベースウェアの状態）とは別の意味であることを明記する。名前は変えない（3 者で合意した並びのため）。ホストは `OnHostComplete` にベースウェアの状態の `Status` を付けない。
8. The ホスト側プロトコル仕様 shall `Tag` 付きの post 1 つに `OnHostComplete` をちょうど 1 つ出すと定める。利用者が `Tag` 付きの SEND のトークを中断したときは、`OnBalloonBreak` と、その post の `OnHostComplete`（200 ＋ `interrupted`）の両方を出す。
9. The ホスト側プロトコル仕様 shall 終了・切り替えで post を捨てたときの `OnHostComplete`（204）を、終了のイベント（`OnClose`・`OnGhostChanging` など）より前に出し、それに間に合わない unload では出さなくてよい（unload で未配送を捨てる規約どおり）と定める。

### Requirement 7: 拒否の返し方
**Objective:** As a SHIORI 実装者・ベースウェア実装者, I want 「拒否」と「存在しない」と「不正」を応答で見分けられること, so that SHIORI が拒否に応じた振る舞いを選べる（論点 8・12）

#### Acceptance Criteria
1. The ホスト側プロトコル仕様 shall 拒否を 400 ＋ `Reason: forbidden` で返すと定め、「存在しない」の 204 と区別する。
2. The ホスト側プロトコル仕様 shall 403 を足さないことと、その理由（ステータスを SHIORI/3.0 の集合に収め、v3 と v4 で要求と応答のコードを分けないため）を書く。
3. The ホスト側プロトコル仕様 shall `Reason` を付けるかは任意とし、`Reason` の無い 400 は「不正」（その ID・コマンドを扱えない未対応を含む）、`Reason: forbidden` は「拒否」、未知の語は「理由不明の拒否」と読むと定める。`Reason` の語は当面 `forbidden` だけとする。
4. The ホスト側プロトコル仕様 shall 拒否の返し方が、host.request の応答（`getproperty`・`setproperty` など）と `OnHostComplete` の 400 の両方に当てはまることを述べる。
5. The ホスト側プロトコル仕様 shall ホストの中の失敗（IPC が切れた、など答えを作れないとき）を 500 で返してよいと定め、SHIORI の誤りを表す 400 と区別する。芯の既定の応答（要件 8.3）は 400 のままとする。

### Requirement 8: ホスト側の芯から中身の解釈を外す
**Objective:** As a ベースウェア実装者, I want `shiori4_host` の芯が SHIORI からの要求と post の中身を読まずに `Host` へ渡すこと, so that 語の扱いをすべて自分の `Host` の実装で決められ、将来中身が別の語（JSON-RPC など）に替わっても芯が邪魔をしない（方針）

#### Acceptance Criteria
1. When SHIORI が規約 2 の期間の中で `host->request` を呼ぶ, the shiori4_host shall 要求の文字列を解析せずにそのまま `Host` に渡し、`Host` の応答をそのまま SHIORI に返す。
2. The shiori4_host shall 組み込みの `property`（`getproperty` を含む）の応答を持たず、どの `ID` にも芯が独自に答えない。
3. Where `Host` の実装が同期の要求に答えない, the shiori4_host shall どの要求にも 400（このホストでは扱えない）を返す。0.0.1 が知らない ID に返していた応答と同じにする。
4. The shiori4_host shall abi-conventions の規約 2 の門（別スレッド・期間の外の `host->request` には `Host` を呼ばず `SHIORI_E_WRONG_THREAD` と空の out を返す）を、振り分けの前に残す。
5. If `host->request` の要求、または post のメッセージが UTF-8 でない, the shiori4_host shall それを `Host` に渡さず 400 を返す（芯が求めるのは UTF-8 であることだけ）。
6. When SHIORI が post する, the shiori4_host shall 既定では中身（`Command` の有無を含む）を見ずに受け付け、`Host` の実装が受付の判定を持つときだけその判定に従う。
7. The shiori4_host shall 完了通知の芯として、`Host` が与えたヘッダの全文を、指定された届け方（request か notify）で post したインスタンスに届ける口を持ち、芯はそのヘッダに何も足さず、何も検査しない。
8. The shiori4_host の芯（`Host` trait の同期要求と post の受付の判定・受け付けた post・SHIORI に届けるイベント・完了通知の口） shall 原文（文字列）を受け渡し、中身を解析して返すメソッド（`Command`・`Tag`・`ID` を読むもの、`OnHostComplete` を組み立てるもの）を持たない。解析と組み立ては便利関数のモジュール（要件 10）が受け持つ。
9. The shiori4_host shall 芯から中身の解釈を外した後も、post のキュー・`pump` での配送・unload での破棄・`Host::wake` の呼び出しの振る舞いを変えない。

### Requirement 9: SHIORI 側の芯から中身の解釈を外す
**Objective:** As a SHIORI 実装者, I want `shiori4` の芯が v4 のヘッダの中身を解釈しないこと, so that 自分の SHIORI で好きな解析を使え、芯が便利関数のモジュールに縛られない（方針）

#### Acceptance Criteria
1. While v4 で動いている, the shiori4 の芯（`Ghost` と、それを ABI につなぐ層） shall ホストからの request・notify の文字列を解析せずに原文（文字列）のまま Ghost 実装へ渡す。解析は実装者が便利関数のモジュールで行う。
2. While v3 互換モードで動いている, the shiori4 の芯 shall SHIORI/3.0 の全文とヘッダ部分の変換（開始行の読み書き・Charset の検査と補い）を、インターフェースの変換としてこれまでどおり行う。
3. The shiori4・shiori4_host shall 便利関数のクレートに依存しない（`[dependencies]` に置かない。テストと doc の例のための `[dev-dependencies]` は可）。
4. The shiori4 の芯 shall `host->request`・post・log を、文字列を解析せずに運ぶ口（`ShioriHostHandle`・`ShioriHostPoster`）をこれまでどおり提供する。

### Requirement 10: パースのための便利関数のモジュール
**Objective:** As a SHIORI 実装者・ベースウェア実装者, I want 仕様の語に合わせた解析と組み立ての関数を、必要なら使えること, so that 仕様どおりのメッセージを手で組み立てずに済み、使わない人は芯だけで済む（方針・論点 1〜12）

#### Acceptance Criteria
1. The 便利関数のモジュール shall `shiori4_abi` だけに依存する独立したクレート `shiori4_helper` として提供され、使わない利用者の振る舞いに影響しない（設計ディスカッションで確定）。
2. The 便利関数のモジュール shall ヘッダ部分の解析（名前の大文字小文字を区別しない取得、`Reference<n>` の取得、並びの列挙）と組み立て（値の CR・LF を取り除く）を提供する。
3. The 便利関数のモジュール shall ホスト側プロトコル仕様が定める名前と語（`getproperty`・`setproperty`・`Value`・`Command`・`Tag`・`SEND`・`NOTIFY`・`Script`・`Event`・`Option`・`OnHostComplete`・`Status`・`Reason`・`forbidden`・`completed`・`interrupted` と、要件 5.2 の必須ヘッダの名前）を定数として提供し、`property` を残さない。
4. The 便利関数のモジュール shall SHIORI 側のために、`getproperty`・`setproperty` の要求と、`SEND`・`NOTIFY` の post を仕様どおりに組み立てる関数を提供する。
5. The 便利関数のモジュール shall ホスト側のために、`getproperty`・`setproperty` の応答（200 ＋ `Value`（値の改行を取り除く）・204・400・400 ＋ `Reason: forbidden`）を組み立てる関数を提供する（要求を読むのは 10.2 の解析で足りる）。
6. The 便利関数のモジュール shall ホスト側のために、受け付けた post の原文から `Tag` とコマンド名を引き継ぎ、`ID: OnHostComplete`・`Tag`・`Status`・`Reference0: <コマンド名>`・`Reference1..` を並べた完了通知のヘッダを組み立てる関数を提供する。`Tag` の無い post からは組み立てない。
7. The 便利関数のモジュール shall SHIORI 側のために、`Tag` を発行して登録してから post し、`OnHostComplete` の `Tag` で登録を取り出す待ち合わせ（`PendingTags`）を、今と同じ使い方で提供する。
8. The 便利関数のモジュール shall v4 でホストに同期で問い合わせるプロパティの取得（今の `Properties`／`Lookup`。芯から移す）を `getproperty` を使う形で提供し、`setproperty` の送信はキャッシュへの取り込みと紛れない別の関数で提供する。
9. The 便利関数のモジュール shall 知らない `Command`・知らない `Reason`・知らない `Option` の語を判定・拒否しない。
10. The 便利関数のモジュール shall 組み立てるヘッダに Charset を付けない。

### Requirement 11: 互換性・検証・連絡
**Objective:** As a shiori4 の開発者, I want 0.0.1 の API の変更が 0.0.x のうちに 1 度で済み、振る舞いがテストで確かめられ、下流に正本の場所が伝わること, so that pasta・areka が 0.0.2 から安心して使い始められる（Constraints）

#### Acceptance Criteria
1. The 各クレート shall 0.0.1 の公開 API の変更（`property` の改名・組み込みの応答の撤去・`accept_post` の既定・完了通知の口・便利関数への移動）を、この spec の中でまとめて行い、移行の仕方をリリースノートに書ける形で残す。
2. The 各クレート shall ABI（C ヘッダの構造体のレイアウト・関数テーブル・定数の値・公開シンボル）を変えない。
3. The 各クレート shall 版を上げない（0.0.2 は `/release` が上げる）。
4. The 各クレート shall この spec の振る舞い（芯が要求・post を解析せずに渡すこと、既定で post を全部受け付けること、既定の応答、規約 2 の門が先頭に残ること、完了通知の全文の配送、芯と便利関数が Charset を付けないこと、便利関数の組み立てと解析が仕様どおりであること、`PendingTags` の往復）をテストで確かめる。
5. The 各クレート shall ubuntu・windows・macos の CI（`cargo test --all` と `cargo clippy --all-targets --workspace -- -D warnings`）を通す。
6. The shiori4 の開発者 shall pasta と areka の両セッションへ 2 回知らせる: 要件の承認の後に確定した語（論点 1〜12 と、要件ディスカッションで足した `Reason` の読み分け・`OnHostComplete` のヘッダ・`Ghost`／`Host` が原文を受けること）を、設計で置き場所が決まった後に正本の場所と、pasta・areka の照合で足した語（下の「pasta・areka の照合で足したもの」）を。送るのは開発者の明示の指示による。
7. The steering（`tech.md`・`structure.md`・`workflow.md`・`roadmap.md`） shall `host_protocol` の置き場所と「芯は中身を解釈しない」方針について、この spec の結果と食い違う記述を持たない。
8. The shiori4 リポジトリ shall `shiori4_helper` を 4 つ目の公開クレートとして、ビルドとリリースの CI・`/release` のスキル・`.github/release-ci-setup.md` の対象に加え、公開の順を `shiori4_abi` → `shiori4_helper` → `shiori4` → `shiori4_host` とする。初回の公開と Trusted Publisher の登録は開発者が手元から行い、その手順を `.github/release-ci-setup.md` に書く。

## 要件ディスカッションの結果（2026-10-24 確定）
brief.md だけでは決め切れなかった所を、開発者が全議題とも推奨案で確定した。

1. **`OnHostComplete` の `Status`**（要件 6.7）: 名前は変えず、`OnHostComplete` の中だけは結果のステータスコードを運ぶと明記する。
2. **`Host` が答えないときの既定の応答**（要件 8.3）: どの要求にも 400。
3. **芯は原文だけを運ぶ**（要件 8.8・9.1・10.8）: `Ghost`・`Host` は原文（文字列）を受ける。`Post`・`Event`・`Completer` の解析つきメソッドと `Properties`／`Lookup` は便利関数のモジュールへ移す。
4. **400 と `Reason` の読み分け**（要件 6.3・7.3）: `Reason` は任意。無ければ「不正」、`forbidden` は「拒否」、未知の語は「理由不明の拒否」。すべての 400 に共通。3 者の合意に足した決め事。
5. **`OnHostComplete` のヘッダ**（要件 5.4）: post から起きた要求に含め、`SenderType: internal`・`SecurityLevel: local` を付ける。3 者の合意に足した決め事。
6. **pasta・areka への連絡**（要件 11.6）: 要件の承認の後に語を、設計の後に置き場所を知らせる。送るのは開発者の明示の指示による。1 回目は 2026-10-24 に両セッションへ送った。

### pasta・areka の照合で足したもの（2026-10-24 確定）
1 回目の連絡に両者が返した要望を、開発者が推奨案で確定した。1〜3 は 3 者の合意に足した語。

1. **NOTIFY の完了に 204**（要件 6.5。areka）: 起こさずに捨てたら 204。SEND の 200／204／400 と同じ形にする。
2. **`SenderType` は先頭の語が `internal`**（要件 5.4。両者）: `\![raise]` なら `internal,raise`。NOTIFY と `OnHostComplete` は `internal` だけ。
3. **ホストの中の失敗は 500**（要件 7.5。areka）: 400（SHIORI の誤り）と区別する。芯の既定は 400 のまま。
4. **`Reason` の無い 400 は未対応を含む**（要件 7.3。pasta）。
5. **ヘッダ名の大文字小文字**（要件 1.7。pasta）: 受け手は区別しない。送り手は ukadoc の綴り。
6. **`Tag` は解釈しない文字列で、一字も変えずに返す**（要件 3.7。pasta）。
7. **`Tag` 付きの post 1 つに `OnHostComplete` は 1 つ**（要件 6.8。areka）: 利用者の中断では `OnBalloonBreak` と `interrupted` の両方。
8. **終了・切り替えの 204 は終了のイベントより前**（要件 6.9。areka）: 間に合わない unload では出さない。
9. **NOTIFY の 200 は SHIORI が答えた時。応答の台本も `nobreak` を読む**（要件 6.5・4.6・3.5。areka）。
10. **post に要るヘッダは `Command` とそのコマンドのヘッダだけ**（要件 3.8。pasta）。
11. **`Option` が 2 つ以上ならカンマ区切り**（要件 3.5。pasta）。

知らない `Command` の扱いの推奨を注記で書く案（areka）は、brief の裁定（決め事は要らない）に従い見送った。

設計フェーズへ回した事項（モジュールの名前と置き場所・文書の置き場所・C ヘッダの 1 行・v3 の私的な解析・完了通知の口の形・`setproperty` の関数名・`Option` の語の定数）は research.md の「Recommendations for Design Phase」にある。
