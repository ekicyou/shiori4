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
- 文字コードは UTF-8 に固定する。送り手は `Charset` ヘッダを付けない。付いていても受け手は読まない（`shiori4_abi.h` の規約）。
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

拒否とホストの中の失敗の返し方は第 7 章に従う。

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
