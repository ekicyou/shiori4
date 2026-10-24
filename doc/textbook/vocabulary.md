# 語の表と確定の範囲

0.0.x の間は、版ごとに互換を壊す変更がありうる。

ホスト側プロトコルの語（ID・ヘッダ名・値の語・`Reason`）とステータスの一覧と、shiori4 の版ごとにどの語と API が確定しているかの一覧である。教科書の目次は [doc/textbook/README.md](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/README.md) にある。

## この表の読み方

- 語の正本は [`doc/host-protocol.md`](https://github.com/ekicyou/shiori4/blob/main/doc/host-protocol.md)（ホスト側プロトコル仕様）である。この表は語の綴りを変えずに写し、語の意味を書かない。語の意味は「正本の節」の列の節と、ukadoc の参照先（[`doc/host-protocol.md`](https://github.com/ekicyou/shiori4/blob/main/doc/host-protocol.md) の付録 B）で読む。
- 語の表の「種類」「語」「定数名」の 3 列は、`doc/host-protocol.md` 第 8 章「語の一覧」の表の行をそのまま写したものである。「正本の節」の列だけをこの表で足した。節の番号は `doc/host-protocol.md` の章と節である。
- 写しが第 8 章と一致することは CI が確かめる（`shiori4_abi/tests/textbook.rs`）。
- 定数名は、便利関数 `shiori4::protocol`（`shiori4_host::protocol` からも使える）の定数の名前である。定数の値は、表の語と同じ綴りである（第 8 章）。
- 第 8 章に行の無いステータスは、ステータスの表に `shiori4_abi` の定数から足した。
- 「スレッド規約 2」「戻り値 1」などの引き方は、[規約の一覧](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/rules.md)の「番号の付け方と引用の仕方」に従う。

## 語の表

| 種類 | 語 | 定数名 | 正本の節 |
|------|----|--------|----------|
| ID | `getproperty` | `ID_GETPROPERTY` | 第 3.1・3.3 節 |
| ID | `setproperty` | `ID_SETPROPERTY` | 第 3.2 節 |
| ID | `OnHostComplete` | `ID_HOST_COMPLETE` | 第 6 章 |
| ヘッダ名 | `ID` | `HEADER_ID` | 第 2.2 節 |
| ヘッダ名 | `Sender` | `HEADER_SENDER` | 第 2.2 節 |
| ヘッダ名 | `SenderType` | `HEADER_SENDER_TYPE` | 第 2.2・2.4 節 |
| ヘッダ名 | `SecurityLevel` | `HEADER_SECURITY_LEVEL` | 第 2.2・2.4 節 |
| ヘッダ名 | `SecurityOrigin` | `HEADER_SECURITY_ORIGIN` | 第 2.2 節 |
| ヘッダ名 | `Value` | `HEADER_VALUE` | 第 2.1・2.5・3.1 節 |
| ヘッダ名 | `Command` | `HEADER_COMMAND` | 第 4.1・4.2 節 |
| ヘッダ名 | `Tag` | `HEADER_TAG` | 第 4.6・6.2 節 |
| ヘッダ名 | `Script` | `HEADER_SCRIPT` | 第 4.2 節 |
| ヘッダ名 | `Event` | `HEADER_EVENT` | 第 4.2 節 |
| ヘッダ名 | `Option` | `HEADER_OPTION` | 第 4.4 節 |
| ヘッダ名 | `Status` | `HEADER_STATUS` | 第 2.3・6.2・6.6 節 |
| ヘッダ名 | `Reason` | `HEADER_REASON` | 第 7.1・7.3 節 |
| `Command` の値 | `SEND` | `COMMAND_SEND` | 第 4.2・6.3 節 |
| `Command` の値 | `NOTIFY` | `COMMAND_NOTIFY` | 第 4.2・6.4 節 |
| `Option` の値 | `nobreak` | `OPTION_NOBREAK` | 第 4.4 節・第 5 章 |
| `Option` の値 | `notranslate` | `OPTION_NOTRANSLATE` | 第 4.4 節 |
| `Option` の値 | `strict` | `OPTION_STRICT` | 第 4.4 節 |
| `Reason` の値 | `forbidden` | `REASON_FORBIDDEN` | 第 7.1・7.3 節 |
| `OnHostComplete` の `Reference1` の値 | `completed` | `OUTCOME_COMPLETED` | 第 6.3 節 |
| `OnHostComplete` の `Reference1` の値 | `interrupted` | `OUTCOME_INTERRUPTED` | 第 5.4・6.3 節 |
| `SenderType` の値 | `internal` | `SENDER_TYPE_INTERNAL` | 第 2.4 節 |
| `SecurityLevel` の値 | `local` | `SECURITY_LEVEL_LOCAL` | 第 2.2・2.4 節 |

- `Status` のヘッダは 2 つの意味で使われる。第 2.3 節はベースウェアの状態、第 6.2 節は `OnHostComplete` の結果のステータスコードで、その別は第 6.6 節にある。
- 第 8 章のとおり、`Reference*`・`SecurityOrigin: null`・`SenderType` の `raise` など、この表に無い語には定数が無い。綴りは `doc/host-protocol.md` の本文と ukadoc のとおりに書く。0.0.1 の `ID: property` の定数（`ID_PROPERTY`）は無い（第 3.3 節）。

## ステータスの表

第 8 章に行の無いステータスである。「定数名」は `shiori4_abi` の定数（C ヘッダ [`shiori4_abi/include/shiori4_abi.h`](https://github.com/ekicyou/shiori4/blob/main/shiori4_abi/include/shiori4_abi.h) の `#define` と同じ名前と値）である。「正本の節」の列の「第 N 節」は `doc/host-protocol.md` の節、「戻り値 1」などは C ヘッダ冒頭の規約（[規約の一覧](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/rules.md)）である。

0 以上のステータス（戻り値 1）の意味は ukadoc の [SHIORI/3.0](https://ssp.shillest.net/ukadoc/manual/spec_shiori3.html) のとおりである（`doc/host-protocol.md` 第 2.1 節・付録 B）。負の値（戻り値 2）の意味は、「正本の節」の列の規約で読む。

| 種類 | 語 | 定数名 | 正本の節 |
|------|----|--------|----------|
| ステータス（SHIORI/3.0 のステータスコード） | `200` | `SHIORI_200_OK` | 第 2.5・3.1・6.2・6.3・6.4 節、戻り値 1 |
| ステータス（SHIORI/3.0 のステータスコード） | `204` | `SHIORI_204_NO_CONTENT` | 第 2.5・3.1・3.2・4.5・6.2・6.3・6.4 節、戻り値 1、post の完了通知 2 |
| ステータス（SHIORI/3.0 のステータスコード） | `311` | `SHIORI_311_NOT_ENOUGH` | 戻り値 1（定数の注釈に「互換用。新規実装では非推奨」） |
| ステータス（SHIORI/3.0 のステータスコード） | `312` | `SHIORI_312_ADVICE` | 戻り値 1（定数の注釈に「互換用。新規実装では非推奨」） |
| ステータス（SHIORI/3.0 のステータスコード） | `400` | `SHIORI_400_BAD_REQUEST` | 第 3.2・6.2・6.3・6.4 節・第 7 章、戻り値 1、post の完了通知 2 |
| ステータス（SHIORI/3.0 のステータスコード） | `500` | `SHIORI_500_INTERNAL_SERVER_ERROR` | 第 7.5 節、戻り値 1、版 2 |
| ステータス（ABI 層のエラー） | `-1` | `SHIORI_E_INVALID_ARG` | 戻り値 2 |
| ステータス（ABI 層のエラー） | `-2` | `SHIORI_E_UNSUPPORTED` | 戻り値 2 |
| ステータス（ABI 層のエラー） | `-3` | `SHIORI_E_PANIC` | 戻り値 2、ABI 規約 8 |
| ステータス（ABI 層のエラー） | `-4` | `SHIORI_E_REENTRANT` | 戻り値 2、スレッド規約 3 |
| ステータス（ABI 層のエラー） | `-5` | `SHIORI_E_SHUTDOWN` | 戻り値 2、スレッド規約 5、post の完了通知 2 |
| ステータス（ABI 層のエラー） | `-6` | `SHIORI_E_INVALID_UTF8` | 戻り値 2 |
| ステータス（ABI 層のエラー） | `-7` | `SHIORI_E_WRONG_THREAD` | 戻り値 2、スレッド規約 2 |

- `doc/host-protocol.md` は 403 を足さない（第 7.2 節）。

## 確定の範囲

### 決め方

- `doc/host-protocol.md` に書かれた語は、その文書が当てはまる版で確定である。後の版で変えるときは、付録 A と同じく移行を書く。
- 各版で公開された API は、その版で確定である。
- `doc/host-protocol.md` に無い語（定数の無い `Reference*` の並びの拡張など）は、決まっていない。
- 語ごとの安定度は、この教科書では決めない。
- `main` の文書には未公開の版の語が載りうる。公開された版の語は、その版のタグの文書で読む（`doc/host-protocol.md` 第 0 章）。

### 版ごとの表

| 版 | 確定の語 | 確定の API | 決まっていない |
|----|----------|------------|----------------|
| 0.0.1 | 語の文書は無い（`doc/host-protocol.md` は 0.0.2 から当てはまる）。0.0.1 の語のうち 0.0.2 で変わったものは付録 A の A.1 にある | 0.0.1 で公開した API（[`v0.0.1` のタグ](https://github.com/ekicyou/shiori4/tree/v0.0.1)） | 文書に書かれていない語のすべて |
| 0.0.2 | [`v0.0.2` のタグの `doc/host-protocol.md`](https://github.com/ekicyou/shiori4/blob/v0.0.2/doc/host-protocol.md) に書かれた語 | 0.0.2 で公開した API（[`v0.0.2` のタグ](https://github.com/ekicyou/shiori4/tree/v0.0.2)） | その文書に無い語（定数の無い `Reference*` の並びの拡張など） |
| 0.1.0 | [`v0.1.0` のタグの `doc/host-protocol.md`](https://github.com/ekicyou/shiori4/blob/v0.1.0/doc/host-protocol.md) に書かれた語 | 0.1.0 で公開した API（[`v0.1.0` のタグ](https://github.com/ekicyou/shiori4/tree/v0.1.0)） | その文書に無い語（定数の無い `Reference*` の並びの拡張など） |
| 未公開の `main` | 無い（版が公開されたとき、その版のタグの文書に書かれた語が確定になる） | 無い（版が公開されたとき、その版の API が確定になる） | `main` の文書と API のすべて。公開までに変わりうる |

## 0.0.1 から 0.0.2 への変更

正本は [`doc/host-protocol.md`](https://github.com/ekicyou/shiori4/blob/main/doc/host-protocol.md)の付録 A「0.0.1 からの移行」である。語の変更は 1 つで、残りは API の変更である。次の表はその要約で、各行の詳しい形は付録 A で読む。

| 変わったもの | 0.0.1 | 0.0.2 | 出典 |
|--------------|-------|-------|------|
| 語 | `ID: property` | `ID: getproperty`（`property` は受けない） | 付録 A の A.1・第 3.3 節 |
| 語の定数と解析の置き場所 | `shiori4::{Headers, HeaderWriter, Properties, Lookup, PendingTags}`・`shiori4::host_protocol` | `shiori4::protocol::{…}`（`shiori4_host::protocol` からも使える） | 付録 A の A.2 |
| SHIORI 側の受け取り | `Ghost::request`・`notify` が `&Headers` を受ける | 原文の `&str` を受ける。解析は `protocol::Headers::parse(req)` | 付録 A の A.2 |
| ホスト側の `Host` | `Host::property` がある。`Host::request` は `Option` を返す | `Host::property` を削除。`Host::request` は `ShioriResponse` を返し既定は 400。`Host::name` が必須 | 付録 A の A.2 |
| post と完了通知 | `Post::headers`・`Completer::tag`・`Event::parsed` などがある。`Completer::complete_with` は Tag を先頭に足す | それらを削除し、解析は `protocol::Headers::parse` で行う。`Completer::complete_with` は全文をそのまま届ける（何も足さない） | 付録 A の A.2 |

付録 A に無い、0.0.2 の ABI の規約と読み込みの口の変更（`ShioriHostPoster` が `Copy` でなくなったこと、絶対パスでない dir を `LoadError::InvalidPath` で断ること、呼び出しの外からの `host->request` が `SHIORI_E_WRONG_THREAD` を返すことなど）は、[0.0.2 の見どころ](https://github.com/ekicyou/shiori4/blob/main/doc/release-notes/v0.0.2.md)の「動きが変わる点」にある。
