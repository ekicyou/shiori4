# ホスト側プロトコル仕様（v4）

## 0. この文書について

この文書は、shiori4 の v4 で SHIORI とホスト（ベースウェア）が交わすメッセージの語彙の正本である。SHIORI 実装者・ベースウェア実装者・shiori4 は、この文書を参照し、語を写すときに変えない。

- **当てはまる版**: shiori4 0.0.2 から。0.0.1 の語との違いは付録 A に置く。`main` の文書には未公開の版の語が載りうるので、公開された版の語は、その版のタグの文書で読む。
- **運び方に依存しない**: 語彙はメッセージごとに閉じていて、ABI（`shiori4_abi`）という運び方に依存しない。双方向の通信路（パイプ・IPC など）にもそのまま載せてよい。この文書は通信用のプロトコルそのもの（接続・フレーミングなど）を定めない。
- **芯と実装の分担**: shiori4 の芯（ABI の口と runtime。`shiori4`・`shiori4_host`）は、運ぶ文字列が UTF-8 であることだけを求め、中身を解釈しない。この文書の語を守らせるのはホストと SHIORI の実装である。
- **便利関数は任意**: 語に合わせた解析と組み立ての関数（`shiori4::protocol`。`shiori4_host::protocol` からも使える）は、使いたい人が使うものである。使わなくてもこの文書に従える。
- **ukadoc の扱い**: SHIORI/3.0・SSTP・プロパティシステムの語をそのまま使う所では、[ukadoc](https://ssp.shillest.net/ukadoc/manual/) を参照先として示す。その語の意味は ukadoc のとおりで、この文書は書き換えない。参照先は付録 B にまとめる。
- **言い方**: 規範の文は「〜する」（必ずそうする）、「〜してよい」（してもしなくてもよい）、「〜しない」（してはいけない）の 3 つで書く。

## 1. 書式

- メッセージはヘッダ部分だけである。各行は `名前: 値` で、行は CRLF で区切る。書式は [SHIORI/3.0](https://ssp.shillest.net/ukadoc/manual/spec_shiori3.html) のヘッダと同じで、開始行（`GET SHIORI/3.0` など）と終端の空行は無い。
- 文字コードは UTF-8 に固定する。送り手は `Charset` ヘッダを付けない。付いていても受け手は読まない（この文書の決め。ABI の規約（`shiori4_abi.h`）では、ホストは要求に `Charset` を付けなくてよく、付いていても SHIORI は無視し、SHIORI は応答に付けない）。
- ヘッダ名の受け手は、大文字小文字を区別せずに読む。送り手は、ukadoc の綴り（この文書の綴り）で書く。
- 値は `:` の後の空白を除いたものである。値は CR・LF を含まない。

## 2. v4 の要求と応答

### 2.1 SHIORI/3.0 と同じ語

v4 のヘッダの名前と意味、ホストが送るイベント（`ID` と `Reference*` の並び）、SHIORI の応答のヘッダ（`Value`・`ValueNotify`・`Marker`・`ErrorLevel`・`ErrorDescription` など）の名前と意味、ステータスコードは、ukadoc の [SHIORI/3.0](https://ssp.shillest.net/ukadoc/manual/spec_shiori3.html) と[イベント一覧](https://ssp.shillest.net/ukadoc/manual/list_shiori_event.html)のとおりとする。SHIORI/3.0 との違いは、開始行が無いことと `Charset` が無いことの 2 つだけである。

### 2.2 ホストが必ず付けるヘッダ

ホストは v4 の要求（request・notify）に次の 5 つを必ず付ける。並びは ukadoc どおりとする（`SecurityLevel` は少なくとも `ID` より前）。

| ヘッダ | 値 |
|--------|----|
| `ID` | イベント名・要求の名前 |
| `Sender` | ベースウェアの名前 |
| `SenderType` | ukadoc の語（カンマ区切り） |
| `SecurityLevel` | `local` か `external` の 2 語だけ |
| `SecurityOrigin` | ukadoc の形。`null` か起点（`<scheme>://<host>[:<port>]`） |

### 2.3 状況によって付けるヘッダ

| ヘッダ | 付けるとき |
|--------|------------|
| `Status` | 伝えるベースウェアの状態があるときだけ。値は ukadoc の語のカンマ区切り |
| `BaseID` | ukadoc が `BaseID` を定めるイベントだけ |
| `Reference*` | そのイベント・要求が ukadoc またはこの文書で定める並び |

### 2.4 post から起きた要求

post（第 4 章）から起きた要求には、ホストは先頭の語が `internal` の `SenderType` と、`SecurityLevel: local` を付ける。

| 要求 | `SenderType` | `SecurityLevel` |
|------|--------------|-----------------|
| SEND の台本の中の `\![raise]` で起きた要求 | `internal,raise`（ukadoc どおり `raise` を足す） | `local` |
| NOTIFY で起こしたイベント | `internal` | `local` |
| post の完了を知らせる `OnHostComplete` | `internal` | `local` |

### 2.5 SHIORI の名前と版

SHIORI の名前と版は SHIORI/3.0 と同じく、ホストが `ID: version`（ほかに `ID: name`・`ID: craftman`・`ID: craftmanw`）を要求し、SHIORI は 200 ＋ `Value: <値>` で答える。答えない SHIORI は 204 を返す。

### 2.6 ABI の版

ABI の版はヘッダに載せない。版は `shiori_get_api` の引数と、返るテーブルの `abi_version` で分かる。

## 3. プロパティ

SHIORI はホスト（ベースウェア）のプロパティを、`host->request` の同期の要求で読み書きする。
拒否とホストの中の失敗の返し方は、読み（3.1）と書き込み（3.2）のどちらも第 7 章に従う。

### 3.1 読み（getproperty）

| | ヘッダ |
|-|--------|
| 要求 | `ID: getproperty` ／ `Reference0: <名前>` |
| 応答 | 200 ＋ `Value: <値>`（プロパティが存在する）／ 204（存在しない） |

### 3.2 書き込み（setproperty）

| | ヘッダ |
|-|--------|
| 要求 | `ID: setproperty` ／ `Reference0: <名前>` ／ `Reference1: <値>` |
| 応答 | 204（書き込んだ）／ 400（不正） |

### 3.3 0.0.1 からの改名

0.0.1 の `ID: property` は `ID: getproperty` に改めた。ホストは `ID: property` を受ける互換の扱いを持たない（ほかの知らない `ID` と同じに扱う）。

### 3.4 名前の決まり

- プロパティの名前は、ukadoc の[プロパティシステム](https://ssp.shillest.net/ukadoc/manual/list_propertysystem.html)の名前をそのまま使う。
- ベースウェア固有のプロパティは、ベースウェアの接頭辞（例 `areka.`）を付けた名前で分ける。
- 名前の大文字小文字は区別しない。

### 3.5 値の改行

ホストは値の CR・LF を取り除いて返す（エスケープして残さない）。改行を含む値が要るプロパティは、別の名前で足す。

### 3.6 一覧

この文書はプロパティの一覧を持たない。一覧は ukadoc の[プロパティシステム](https://ssp.shillest.net/ukadoc/manual/list_propertysystem.html)を参照する。

## 4. post

### 4.1 同じプロセスからの SSTP

post は「同じプロセスからの [SSTP](https://ssp.shillest.net/ukadoc/manual/spec_sstp.html)」として扱う。コマンドは `Command` ヘッダに SSTP の語で書く。語の意味は ukadoc の SSTP のとおりで、この章はその上の v4 での決めだけを書く。

### 4.2 SEND と NOTIFY

| コマンド | ヘッダ | 意味 |
|----------|--------|------|
| `SEND` | `Command: SEND` ／ `Script: <さくらスクリプト>` | ホストはその台本を再生する |
| `NOTIFY` | `Command: NOTIFY` ／ `Event: <イベント名>` ／ `Reference*` | ホストはそのイベントを SHIORI に送る |

- NOTIFY のイベントの名前に制限は掛けない（台本の `\![raise]` と同じ）。
- ホストは安全上の理由で特定のイベントの NOTIFY を拒否してよい（第 7 章）。

### 4.3 宛先

post の宛先は「post したインスタンス」に固定する。ホストは SSTP の宛先指定（`IfGhost` など）を読まない。

### 4.4 Option

SEND と NOTIFY は SSTP の `Option` を読む。

| 語 | 扱い |
|----|------|
| `nobreak`・`notranslate`・`strict` | ukadoc の [SSTP](https://ssp.shillest.net/ukadoc/manual/spec_sstp.html) どおりに効く（`nobreak` の扱いは第 5 章） |
| `nodescript` | 読まない |
| 知らない語 | 無視する |

語が 2 つ以上なら、ukadoc どおりカンマで区切る（例 `Option: nobreak,notranslate`）。

### 4.5 戻り値と結果

post の戻り値は受付の可否だけである（`shiori4_abi.h` の「post の完了通知（v4）」）。結果は、`Tag` を付けた post にだけ、`OnHostComplete`（第 6 章）で返る。`Tag` を付けない post は「結果は不要」を意味する。

### 4.6 Tag

- `Tag` の値は、中身を解釈しない文字列である。CR・LF を含まない。数でなくてよい。
- 値は `:` の後の空白を除いたものである（第 1 章）。送り手は値の前後に空白を置かない。
- ホストは受け取った値を一字も変えずに `OnHostComplete` の `Tag` で返す。

### 4.7 要るヘッダ

post に要るヘッダは `Command` と、そのコマンドのヘッダ（`Script`・`Event`・`Reference*`・`Option`・`Tag`）だけである。SSTP の `Sender`・`Charset` は要らない。付いていてもホストは読まない。

## 5. 割り込みと待ち

### 5.1 既定は割り込み

- post の SEND は、既定で今のスクリプトに割り込む。
- `Option: nobreak` のときだけ、今のスクリプトが終わるまで待ってから再生する。
- 割り込みと待ちを言うための新しいヘッダ（`Interrupt` など）は作らない。

### 5.2 優先度

SEND の優先度は Owned SSTP と同じ（SHIORI 自身の依頼）とする。今のスクリプトに `\t` が入っていても割り込める。

### 5.3 待つための列

`nobreak` の SEND を待たせる列はホストの仕事である。

### 5.4 割り込まれた側への知らせ

| 切られたトーク | 知らせ |
|----------------|--------|
| `Tag` 付きの SEND から始まったトークが SEND に切られた | その post の `OnHostComplete` が `Reference1: interrupted` を運ぶ（第 6 章） |
| 通常のイベントの応答として出たトークが SEND に切られた | 何も出さない |
| 利用者が中断した | 今どおり `OnBalloonBreak` を出す（`Tag` 付きの SEND のトークなら第 6.7 節も） |

### 5.5 OnBalloonBreak を使わない理由

SEND による割り込みの知らせに [`OnBalloonBreak`](https://ssp.shillest.net/ukadoc/manual/list_shiori_event.html) を使わない。ukadoc では `OnBalloonBreak` は「SSTP 以外でブレイクされた際」のイベントであり、post は SSTP（第 4.1 節）だからである。

### 5.6 NOTIFY への適用

NOTIFY で起こしたイベントへの応答の台本にも、5.1・5.2 を当てはめる（既定は割り込み、`Option: nobreak` なら待つ）。ukadoc の SSTP で SEND と NOTIFY は同じ扱いだからである。

## 6. OnHostComplete

### 6.1 届け方

ホストは `Tag` 付きの post の結果を、`ID: OnHostComplete` のイベントとして post したインスタンスに届ける。届け方の既定は request とする。ホストは notify で届けてもよい。SHIORI は request と notify の両方で `OnHostComplete` を受けられるようにする。

### 6.2 共通のヘッダ

第 2.2 節の必須ヘッダ（`SenderType: internal`・`SecurityLevel: local`。第 2.4 節）に加えて、次を付ける。

| ヘッダ | 値 |
|--------|----|
| `ID` | `OnHostComplete` |
| `Tag` | post の `Tag` の値（第 4.6 節。一字も変えない） |
| `Status` | 結果のステータスコード（200・204・400 など） |
| `Reference0` | コマンド名（`SEND`・`NOTIFY` など。post の `Command` の値） |

例（SEND が最後まで再生された）:

```
Sender: areka
SenderType: internal
SecurityLevel: local
SecurityOrigin: null
ID: OnHostComplete
Tag: 42
Status: 200
Reference0: SEND
Reference1: completed
```

### 6.3 SEND の結果

SEND の完了は、再生が終わった時に出す。

| `Status` | 意味 | `Reference1` |
|----------|------|--------------|
| 200 | 再生した | `completed`（最後まで）か `interrupted`（途中で切られた） |
| 204 | 一度も再生せずに捨てた | 無し |
| 400 | 不正。拒否なら `Reason: forbidden` を付ける（第 7 章） | 無し |

204 になる場合の例:

- `nobreak` で待っている間に、終了・ゴーストの切り替えが来た。
- ホストが台本を受けられない状態だった。

### 6.4 NOTIFY の結果

NOTIFY の完了の `Reference1` は無しとする。

| `Status` | 意味 |
|----------|------|
| 200 | イベントを起こし、SHIORI がその要求に答えた。応答の台本の再生の終わりは待たない |
| 204 | 起こさずに捨てた（受けられない状態・終了・切り替え） |
| 400 | 不正。拒否なら `Reason: forbidden` を付ける（第 7 章） |

### 6.5 OnHostComplete への応答の台本

ホストは `OnHostComplete` への応答のスクリプトを、`Option: nobreak` の SEND と同じに扱う。再生中なら終わるまで待ち、待つ間に終了・切り替えが来たら捨てる。

理由: `interrupted` の知らせは、割り込んだトークの再生中に届く。通常の応答と同じに扱うと、その応答が割り込んだ側のトークをさらに置き換えてしまう。

### 6.6 Status の意味の別

`OnHostComplete` の `Status` は、結果のステータスコードを運ぶ。第 2.3 節の `Status`（ukadoc の、ベースウェアの状態）とは別の意味である。名前は 3 者で合意した並びのため変えない。ホストは `OnHostComplete` にベースウェアの状態の `Status` を付けない。

### 6.7 1 つの post に 1 つ

ホストは `Tag` 付きの post 1 つに、`OnHostComplete` をちょうど 1 つ出す。利用者が `Tag` 付きの SEND のトークを中断したときは、`OnBalloonBreak` と、その post の `OnHostComplete`（200 ＋ `Reference1: interrupted`）の両方を出す。

### 6.8 終了・切り替えとの順

終了・切り替えで post を捨てたときの `OnHostComplete`（204）は、終了のイベント（`OnClose`・`OnGhostChanging` など）より前に出す。それに間に合わない unload では出さなくてよい（unload を始めたら未配送のイベントを捨てる ABI の規約どおり）。

## 7. 拒否とエラー

### 7.1 拒否は 400 ＋ Reason

ホストは拒否を 400 ＋ `Reason: forbidden` で返す。「存在しない」の 204（第 3.1 節など）とは区別する。

### 7.2 403 を足さない

403 は足さない。ステータスを [SHIORI/3.0](https://ssp.shillest.net/ukadoc/manual/spec_shiori3.html) の集合に収め、v3 と v4 で要求と応答のコードを分けないためである。

### 7.3 Reason の読み分け

`Reason` を付けるかは任意である。`Reason` の語は当面 `forbidden` だけとする。受け手は 400 を次のとおり読む。

| 400 の `Reason` | 読み |
|-----------------|------|
| 無し | 不正（その ID・コマンドを扱えない未対応を含む） |
| `forbidden` | 拒否 |
| 知らない語 | 理由不明の拒否 |

### 7.4 当てはまる所

この章の返し方は、`host->request` の応答（`getproperty`・`setproperty` など）と、`OnHostComplete` の 400 の両方に当てはまる。

### 7.5 ホストの中の失敗は 500

ホストは、ホストの中の失敗（IPC が切れた、など答えを作れないとき）を 500 で返してよい。400 は SHIORI の誤り（不正・拒否）を表し、500 とは区別する。

ホストの実装が要求に答えないときの shiori4_host の芯の既定の応答は 400 のままとする（このホストでは扱えない）。

## 8. 語の一覧

この文書の語（ID・ヘッダ名・値の語）と、便利関数 `shiori4::protocol`（`shiori4_host::protocol` からも使える）の定数名の対応である。定数の値は、この表の語と同じ綴りである。便利関数を使わない実装は、この表の語をそのまま書けばよい。

| 種類 | 語 | 定数名 |
|------|----|--------|
| ID | `getproperty` | `ID_GETPROPERTY` |
| ID | `setproperty` | `ID_SETPROPERTY` |
| ID | `OnHostComplete` | `ID_HOST_COMPLETE` |
| ヘッダ名 | `ID` | `HEADER_ID` |
| ヘッダ名 | `Sender` | `HEADER_SENDER` |
| ヘッダ名 | `SenderType` | `HEADER_SENDER_TYPE` |
| ヘッダ名 | `SecurityLevel` | `HEADER_SECURITY_LEVEL` |
| ヘッダ名 | `SecurityOrigin` | `HEADER_SECURITY_ORIGIN` |
| ヘッダ名 | `Value` | `HEADER_VALUE` |
| ヘッダ名 | `Command` | `HEADER_COMMAND` |
| ヘッダ名 | `Tag` | `HEADER_TAG` |
| ヘッダ名 | `Script` | `HEADER_SCRIPT` |
| ヘッダ名 | `Event` | `HEADER_EVENT` |
| ヘッダ名 | `Option` | `HEADER_OPTION` |
| ヘッダ名 | `Status` | `HEADER_STATUS` |
| ヘッダ名 | `Reason` | `HEADER_REASON` |
| `Command` の値 | `SEND` | `COMMAND_SEND` |
| `Command` の値 | `NOTIFY` | `COMMAND_NOTIFY` |
| `Option` の値 | `nobreak` | `OPTION_NOBREAK` |
| `Option` の値 | `notranslate` | `OPTION_NOTRANSLATE` |
| `Option` の値 | `strict` | `OPTION_STRICT` |
| `Reason` の値 | `forbidden` | `REASON_FORBIDDEN` |
| `OnHostComplete` の `Reference1` の値 | `completed` | `OUTCOME_COMPLETED` |
| `OnHostComplete` の `Reference1` の値 | `interrupted` | `OUTCOME_INTERRUPTED` |
| `SenderType` の値 | `internal` | `SENDER_TYPE_INTERNAL` |
| `SecurityLevel` の値 | `local` | `SECURITY_LEVEL_LOCAL` |

- 0.0.1 の `ID: property` の定数（`ID_PROPERTY`）は無い（第 3.3 節）。
- `Reference*`・`SecurityOrigin: null`・`SenderType` の `raise` など、この表に無い語にも定数は無い。綴りは本文と ukadoc のとおりに書く。

## 付録 A. 0.0.1 からの移行

shiori4 0.0.1 から 0.0.2 への変更の一覧である。語の変更は 1 つで、残りは API の変更である。

### A.1 語の変更

| 0.0.1 | 0.0.2 |
|-------|-------|
| `ID: property` | `ID: getproperty`（`property` は受けない。第 3.3 節） |

### A.2 API の変更

| 0.0.1 | 0.0.2 |
|-------|-------|
| `shiori4::{Headers, HeaderWriter, Properties, Lookup, PendingTags}` | `shiori4::protocol::{…}` |
| `shiori4::host_protocol::ID_PROPERTY` | `shiori4::protocol::ID_GETPROPERTY`（と `ID_SETPROPERTY` ほかの新しい定数。第 8 章） |
| `Ghost::request(&mut self, &Headers)`／`notify(&mut self, &Headers)` | `request(&mut self, &str)`／`notify(&mut self, &str)`。解析は `protocol::Headers::parse(req)` |
| `shiori4::v3::parse_request -> (Method, Headers)` | `-> (Method, &str)` |
| `Host::property(&self, &str) -> Option<String>` | 削除。`Host::request` で `getproperty` に答える（`protocol::getproperty_response`） |
| `Host::request(&self, &Headers) -> Option<ShioriResponse>`（None で組み込みへ） | `Host::request(&self, &str) -> ShioriResponse`（既定 400） |
| `Host::accept_post(&self, &Headers) -> bool`（既定は `Command` の有無） | `accept_post(&self, &str) -> bool`（既定 true） |
| `Post::headers`・`command`・`tag`・`wants_result` | 削除。`protocol::Headers::parse(post.text())` |
| `Completer::tag`・`is_wanted`・`complete(delivery, status, refs)` | 削除。`protocol::host_complete(..)` → `Completer::complete_with` |
| `Completer::complete_with`（Tag を自動で先頭に足す。Tag 無しは送らない） | 全文をそのまま届ける（何も足さない・検査しない） |
| `Event::parsed`・`Event::id` | 削除。`protocol::Headers::parse(&ev.headers)` |
| `shiori4_host::{HeaderWriter, Headers, host_protocol}` の再エクスポート | `shiori4_host::protocol::{…}` |
| （無し） | `Host::name(&self) -> &str`（必須） |

## 付録 B. ukadoc の参照先

この文書が語をそのまま使う ukadoc の頁である。語の意味はこれらの頁のとおりとする。

| 頁 | URL | この文書で使う所 |
|----|-----|------------------|
| SHIORI/3.0 | <https://ssp.shillest.net/ukadoc/manual/spec_shiori3.html> | 書式・ヘッダの名前と意味・ステータスコード（第 1・2・7 章）。要求は `#request`、応答は `#response` の節。`SecurityLevel` の位置、`SenderType` の語、`SecurityOrigin` の形 |
| SSTP | <https://ssp.shillest.net/ukadoc/manual/spec_sstp.html> | post の語（第 4・5 章）。SEND は `#method_send`、NOTIFY は `#method_notify` の節。`Option` の `nobreak`・`notranslate`・`nodescript`・`strict` |
| プロパティシステム | <https://ssp.shillest.net/ukadoc/manual/list_propertysystem.html> | プロパティの名前と一覧（第 3 章） |
| イベント一覧 | <https://ssp.shillest.net/ukadoc/manual/list_shiori_event.html> | ホストが送るイベント（第 2 章）、`OnBalloonBreak`（第 5 章） |
