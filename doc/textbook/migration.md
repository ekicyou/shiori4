# 移行の手引き

SHIORI/3.0 の SHIORI やホストを持つ作者に向けて、shiori4 へ移るときに変わる所を差分の表で示す。教科書の目次は [doc/textbook/README.md](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/README.md) にある。

## この章の読み方

- 規約の正本は C ヘッダ [`shiori4_abi/include/shiori4_abi.h`](https://github.com/ekicyou/shiori4/blob/main/shiori4_abi/include/shiori4_abi.h) の冒頭の注釈と、`shiori4_abi` のクレート冒頭の `//!` である。この章は正本を言い直したもので、食い違ったら正本が正しい。
- 「版 2」「load の規約 1」「DLL の口と発見の順 2」などの引き方は、[規約の一覧](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/rules.md)の「番号の付け方と引用の仕方」に従う。各規約の理由も規約の一覧にある。
- 「SHIORI/3.0 の DLL」は、`load`・`loadu`・`request`・`unload` を書き出す従来の DLL である。その列は ukadoc の [DLL の仕様](https://ssp.shillest.net/ukadoc/manual/spec_dll.html)と [SHIORI/3.0](https://ssp.shillest.net/ukadoc/manual/spec_shiori3.html) から写した。その意味は ukadoc のとおりで、この章は書き換えない。
- 「v3 互換モード」は `shiori_get_api(3)` が返すテーブル（版 2）、「v4」は `shiori_get_api(4)` が返すテーブル（版 1）である。

## SHIORI 側の差分

| 項目 | SHIORI/3.0 の DLL | v3 互換モード | v4 |
|------|-------------------|---------------|----|
| 書き出し | `load`・`loadu`・`request`・`unload`（ukadoc の DLL の仕様） | `shiori_get_api` の 1 つだけ。`shiori_get_api(3)` にテーブルを返し、対応しないなら NULL（ABI 規約 1） | `shiori_get_api` の 1 つだけ。`shiori_get_api(4)` にテーブルを返し、対応しないなら NULL（ABI 規約 1） |
| ホストからの見分けられ方 | `shiori_get_api` が無い。ホストは `loadu` → `load` の順に探す（DLL の口と発見の順 2） | `shiori_get_api(4)` が NULL で、`shiori_get_api(3)` がテーブルを返す。テーブルの `abi_version` は 3（版 4） | `shiori_get_api(4)` がテーブルを返す。テーブルの `abi_version` は 4（版 4） |
| load の dir の形 | `HGLOBAL` と長さで渡る。`loadu` は UTF-8、`load` は既定の OEM コードページ（日本語の環境では CP932）のディレクトリのパス（ukadoc の DLL の仕様） | 絶対パス・OS の区切り・末尾に区切りあり・UTF-8 の文字列。呼び出し中だけ有効（load の規約 1） | v3 互換モードと同じ（load の規約 1） |
| load の host | 無い（`load` の引数は `HGLOBAL` と長さだけ） | `copy_from_slice` だけを持つテーブル（`abi_version` は 3。`request`・`post`・`log` は NULL）。SHIORI は `copy_from_slice` があるかだけを確かめる（版 2） | 必須（版 1） |
| ホストを呼べるか | 呼ぶ口が無い | `copy_from_slice` のほかは使えない（版 2） | `host.request`（`load`・`request`・`notify` を呼ばれているスレッドから、その呼び出しが返るまでの間だけ。スレッド規約 2）・`host.post`・`host.log`（任意のスレッドから。スレッド規約 4）・`host.copy_from_slice`（どのスレッドからでも。スレッド規約 6） |
| 要求の形 | 開始行（`GET SHIORI/3.0` など）で始まり、空行で終わる（ukadoc の SHIORI/3.0） | SHIORI/3.0 の要求全文（`GET SHIORI/3.0`／`NOTIFY SHIORI/3.0` で始まる）。`notify` の req は `NOTIFY SHIORI/3.0` で始まる全文で、GET なら 400（版 2） | ヘッダ部分だけ（開始行なし。版 1） |
| 応答の形 | `SHIORI/3.0 200 OK` などの開始行で始まり、空行で終わる（ukadoc の SHIORI/3.0） | SHIORI/3.0 の応答全文（`SHIORI/3.0 200 OK` で始まり空行で終わる。版 2） | ヘッダ部分だけ（開始行なし。版 1） |
| Charset | `Charset` ヘッダが文字コードを示す（ukadoc の SHIORI/3.0） | UTF-8 固定（`Charset: UTF-8`）。SHIORI/2.x の要求には 400 を返す（版 2）。UTF-8 固定なので Shift_JIS の要求は来ない | UTF-8 固定。ホストは要求に `Charset` を付けなくてよく、付いていても SHIORI は無視する。SHIORI は応答に `Charset` を付けない（版 3）。UTF-8 固定なので Shift_JIS の要求は来ない |
| 要求の受け取り | `HGLOBAL` と長さ。ホストが `GlobalAlloc` で確保し、SHIORI が読んで `GlobalFree` で解放する（ukadoc の DLL の仕様） | `(ptr, len)` の UTF-8 のバイト列で、NUL 終端にしない（ABI 規約 3）。呼び出しの間だけ有効な借用で、保持するなら SHIORI がコピーする（ABI 規約 4） | v3 互換モードと同じ（ABI 規約 3・4） |
| 応答の作り方 | SHIORI が `GlobalAlloc` で確保して書き、長さを書き戻す。ホストが `GlobalFree` で解放する（ukadoc の DLL の仕様） | `out` の `ShioriBytes` は常にホストが作り、ホストの `release` で解放する。SHIORI は応答を `host.copy_from_slice` で写して作る（ABI 規約 5）。写せなかったときは、SHIORI は `out` をそのまま返す（ABI 規約 5）。`out` は最後に書く（ABI 規約 6） | v3 互換モードと同じ（ABI 規約 5・6） |
| 状態の返し方 | 応答の開始行（ukadoc の SHIORI/3.0） | 戻り値は開始行と同じステータスコード（正本は文字列）。負の値は ABI エラー（out は空。500 として扱う。版 2） | 戻り値。0 以上は SHIORI/3.0 のステータスコード、負の値は ABI 層のエラー（戻り値 1・2） |
| load の失敗 | `FALSE` を返す（ukadoc の DLL の仕様） | NULL を返すだけで、理由を返さない。理由は SHIORI 自身のログに出す（load の規約 3） | NULL を返す前に `host.log` で理由を出す（load の規約 3） |
| 機能 | — | v4 と同じ機能を出すことを保証しない（版 2） | — |

- 1 つの DLL が、SHIORI/3.0 の書き出しと `shiori_get_api` の両方を持ってよい（DLL の口と発見の順 1）。ただし `shiori_get_api` を持つ DLL は、そちらで解決される（DLL の口と発見の順 2）。

## ホスト側の差分

| 項目 | SHIORI/3.0 の DLL を読むホスト | v4・v3 互換モードを読むホスト |
|------|--------------------------------|-------------------------------|
| 見分け方 | `shiori_get_api` が無く、`loadu` か `load` がある（DLL の口と発見の順 2） | `shiori_get_api` がある。`shiori_get_api(4)` を試し、だめなら 3 を試す。どちらも NULL なら読めなかったものとして扱い、`loadu`・`load` へは進まない（DLL の口と発見の順 2）。返ったテーブルの `abi_version` は要求した版と一致する（版 4） |
| 両方を持つ DLL | — | 書き出しを `shiori_get_api` → `loadu` → `load` の順に探し、最初に見つかった口で解決する。解決した口の load が失敗しても、ほかの口へは進まない（DLL の口と発見の順 1・2） |
| load の渡し方 | dir を `HGLOBAL` に写して `loadu`（UTF-8）か `load`（既定の OEM コードページ）に渡す（ukadoc の DLL の仕様） | `host` と dir を渡す。dir は絶対パス・OS の区切り・末尾に区切りあり・UTF-8（load の規約 1）。v3 互換モードの `host` は `copy_from_slice` だけを持つテーブル（版 2） |
| 要求の作り方 | SHIORI/3.0 の要求全文 | v4 は開始行と `Charset` を外すだけで、ヘッダの語は同じ（[`doc/host-protocol.md`](https://github.com/ekicyou/shiori4/blob/main/doc/host-protocol.md) 第 2.1 節。終端の空行も無い。第 1 章）。v3 互換モードは SHIORI/3.0 の要求全文のまま（版 2） |
| 応答の状態 | 応答の開始行を読む | 戻り値で受ける（戻り値 1・2）。v3 互換モードの戻り値は開始行と同じステータスコードで、正本は文字列（版 2） |
| 応答のメモリ | SHIORI が確保した `HGLOBAL` を `GlobalFree` で解放する（ukadoc の DLL の仕様） | `copy_from_slice` を用意し、`out` を自分のメモリで作る。写せなかったときは、自分で決めた印を `out` に残す（SHIORI には知らせない。ABI 規約 5）。戻り値に関係なく `out` を解放する（ABI 規約 6） |
| 負の戻り値 | — | ABI 層のエラーで、`out` は空。`SHIORI_E_PANIC` ならインスタンスを unload する（戻り値 2） |
| SHIORI からの呼び出し | 無い | v4 では `host.request`・`host.post`・`host.log` に答える（スレッド規約 2・4）。v3 互換モードでは `copy_from_slice` だけ（版 2） |

- `shiori4_host` の `load_ghost`（[`shiori4_host/src/discover.rs`](https://github.com/ekicyou/shiori4/blob/main/shiori4_host/src/discover.rs)）は、この順に口を探す。`shiori_get_api` で読めたら `LoadedGhost::V4`・`LoadedGhost::V3` を返す。従来の口（`loadu`・`load`）は書き出しの有無だけを見て呼ばず、開いたライブラリを `LoadedGhost::Legacy` で返す。

## v3 互換モードと旧来の DLL

- v3 互換モードは、Windows 以外のベースウェアを組みたい人のために、`HGLOBAL` に依存しない SHIORI/3.0 の口を用意したものである。従来の SHIORI/3.0 の DLL を置き換えるものではない（[roadmap.md](https://github.com/ekicyou/shiori4/blob/main/.kiro/steering/roadmap.md) の「v3 互換モードの位置づけ」。版 2 の「異なる OS の互換ベースウェア向け」）。
- Windows のホストは、旧来の DLL を今までどおり読めばよい（roadmap.md の「v3 互換モードの位置づけ」）。
- 旧来の DLL を読む口は `shiori4_host` にまだ無い。`load_ghost` は従来の口を見つけても呼ばない（前の節）。従来の SHIORI/3.0 の DLL を読む口は、spec の shiori3-legacy-host が x86 の Windows に向けて足す（roadmap.md）。

## 0.0.2 以前の shiori4 から

- 応答の `ShioriBytes` を常にホストが作る形（ABI 規約 5）は、0.0.2 以前と二進互換でない。関数の形は変わらず、`ShioriHostApi` の末尾に `copy_from_slice` が足された。新しい SHIORI は、`copy_from_slice` を持たない古いホストでは load に失敗する。古い SHIORI は新しいホストで昔のまま動くが、新しい規約の外なので、ライブラリを外した後に応答を捨ててはいけない（abi-textbook の research.md「二進互換の申し送り」）。SHIORI の DLL とホストを両方とも作り直す。
- shiori4 0.0.1 から 0.0.2 への語と API の変更は、[`doc/host-protocol.md` 付録 A](https://github.com/ekicyou/shiori4/blob/main/doc/host-protocol.md#付録-a-001-からの移行) にある。語の変更は `ID: property` から `ID: getproperty` への 1 つで、残りは API の変更である（付録 A）。
