# 試験用のホストの使い方

ベースウェア無しで SHIORI のライブラリを読み、要求を 1 回投げて、結果を表示して unload する試験用のホスト `shiori4-testhost` の使い方と、他のリポジトリの CI への取り込み方を示す。教科書の目次は [doc/textbook/README.md](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/README.md) にある。

## この章の読み方

- 試験用のホストは `shiori4_host` の example 1 本 [`shiori4_host/examples/shiori4-testhost.rs`](https://github.com/ekicyou/shiori4/blob/main/shiori4_host/examples/shiori4-testhost.rs) である。この章の表はそのコードの写しで、食い違ったらコードが正しい。
- 名前・引数・終了の状態は、他のリポジトリの CI が頼る契約である。変えるときはこの章も直す（example の冒頭の `//!` にも同じことが書いてある）。
- 「DLL の口と発見の順」の引き方は、[規約の一覧](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/rules.md)の「番号の付け方と引用の仕方」に従う。

## 注意: 信頼できる SHIORI だけを渡す

ライブラリを開くと、その初期化のコードが動く。試験用のホストには、信頼できる SHIORI だけを渡す。同じ注意は使い方の表示（終了の状態 64 のとき）にも出る。

## 呼び方

```text
shiori4-testhost [--entry v3] [-H "名前: 値"]... <SHIORI のファイル> <ゴーストのディレクトリ> [ID]
```

x86 の Windows では `--entry legacy` も受け付け、SHIORI のファイルを絶対パスで渡す（「x86 の Windows の従来の口」の節）。

```text
shiori4-testhost [--entry v3|legacy] [-H "名前: 値"]... <SHIORI のファイル（絶対パス）> <ゴーストのディレクトリ> [ID]
```

| 引数 | 意味 |
|------|------|
| `<SHIORI のファイル>` | 読むライブラリのパス（Windows の `.dll`・Linux の `.so`・macOS の `.dylib`）。x86 の Windows では絶対パスに限る（相対のパスは `InvalidPath`） |
| `<ゴーストのディレクトリ>` | SHIORI の load に渡すディレクトリ。絶対パスで UTF-8 で表せるものに限る（相対パスは `InvalidPath`）。区切りは `shiori4_host` が OS の区切りと末尾の区切り 1 つに整える |
| `[ID]` | 要求の `ID` の値。省くと `OnBoot` |
| `--entry v3` | v3 互換モード（`shiori_get_api(3)`）の口だけを試す。省くと、すべての口を発見の順（v4 → v3 → `loadu` → `load`）で探す |
| `--entry legacy` | 従来の口（`loadu`・`load`）だけを試す。x86 の Windows だけで、ほかの環境では引数の誤り（「x86 の Windows の従来の口」の節） |
| `-H "名前: 値"` | 要求に足すヘッダ。何度でも与えられ、与えた順に足す。最初の `:` で名前と値に分け、それぞれの前後の空白を除く |

- オプションは位置の引数の前後どこに置いてもよい。
- 次のときは引数の誤りとして、理由と使い方を stderr に出して終了の状態 64 で終わる: `<SHIORI のファイル>` か `<ゴーストのディレクトリ>` が無い／位置の引数が 4 つ以上ある／`--entry` の後が `v3` でない（x86 の Windows では `v3`・`legacy` でない）／`-H` の後にヘッダが無い・UTF-8 でない・`:` が無い・名前が空か改行を含む／`ID` が UTF-8 でない／`-` で始まる 2 文字以上の知らないオプション。

## 送る要求

- v4 では、次の 5 つのヘッダ（`SecurityLevel` は `ID` より前）に `-H` のヘッダを続けたものを要求にする。

  ```text
  SecurityLevel: local
  ID: <ID>
  Sender: shiori4-testhost
  SenderType: internal
  SecurityOrigin: null
  ```

- v3 互換モードでは、`GET SHIORI/3.0` と `Charset: UTF-8` の 2 行の後に同じヘッダを続け、空行で終えた全文を要求にする。
- 各行は CRLF で終わる。

## 表示

stdout に、次の順で表示する。

| 行 | 内容 |
|----|------|
| `entry: <口>` | 読んだ口。`v4`・`v3`・`loadu`・`load` のどれか |
| `request:` | 続く行に、送った要求の全文 |
| `status: <値>` | 戻り値のステータスコード。v3 互換モードでは応答の開始行（`SHIORI/3.0 <コード> …`）から取り、読めなければ `0` |
| `abi error: <値> <定数名>` | `status:` の代わりに、ABI エラー（負の戻り値）とその定数名（`SHIORI_E_INVALID_ARG`・`SHIORI_E_UNSUPPORTED`・`SHIORI_E_PANIC`・`SHIORI_E_REENTRANT`・`SHIORI_E_SHUTDOWN`・`SHIORI_E_INVALID_UTF8`・`SHIORI_E_WRONG_THREAD`、どれでもなければ `(unknown)`）。このときは `response:` を出さない |
| `request error: <名前>: <説明>` | `status:` の代わりに、従来の口の要求の失敗（x86 の Windows だけ。「x86 の Windows の従来の口」の節）。このときは `response:` を出さない |
| `response:` | 続く行に、応答の全文 |
| `post: <原文>` | v4 で、要求の後に 1 回だけ pump して届いた post |
| `unloaded` | SHIORI を unload した |

- SHIORI が `log` を呼ぶと、そのときに `log[<level>]: <msg>` の行を stdout に出す。
- post は表示するだけで、中身を実行しない。完了通知も返さない。pump の後に届いた post は unload で捨てる。
- x86 の Windows でない環境で従来の口（`loadu`・`load`）だけが見つかったときは、`entry: loadu`（または `entry: load`）の後に `従来の口は読まない（shiori3-legacy-host が足す）` を出す。従来の口は呼ばず、`unloaded` も出さない。x86 の Windows では従来の口で往復する（「x86 の Windows の従来の口」の節）。
- 読み込みに失敗したときは、stderr に `load error: <LoadError の値の名前>: <その説明>` を出す。値の名前は `Library`・`NotShiori4`・`Unsupported`・`InvalidPath`・`LoadFailed` のどれかである。

## 終了の状態

| 終了の状態 | 意味 |
|------------|------|
| 0 | 往復が成り立ち、ステータスが 2xx（200 以上 300 未満） |
| 1 | 往復は成り立ったが、ステータスが 2xx 以外（v3 互換モードで開始行が読めなかったときを含む） |
| 2 | ABI エラー（負の戻り値）が返った。x86 の Windows の従来の口では、要求が失敗した（`request error:` を出す） |
| 3 | 読み込みに失敗した（`load error:` を出す） |
| 4 | 従来の SHIORI/3.0 の口だけが見つかった（x86 の Windows でない環境だけ。x86 の Windows では出ない） |
| 64 | 引数の誤り（理由と使い方を stderr に出す） |

## 出力の例

テスト用の SHIORI [`testdata/testghost`](https://github.com/ekicyou/shiori4/tree/main/testdata/testghost) を Windows で読んだときの出力を写す。`<dir>` はゴーストのディレクトリの絶対パスである。

v4 で読み、`OnBoot` を要求した（終了の状態 0）。

```text
> shiori4-testhost testghost.dll <dir>
entry: v4
request:
SecurityLevel: local
ID: OnBoot
Sender: shiori4-testhost
SenderType: internal
SecurityOrigin: null

status: 200
response:
Value: testghost

unloaded
```

`--entry v3` で v3 互換モードだけを試した（終了の状態 0）。

```text
> shiori4-testhost --entry v3 testghost.dll <dir>
entry: v3
request:
GET SHIORI/3.0
Charset: UTF-8
SecurityLevel: local
ID: OnBoot
Sender: shiori4-testhost
SenderType: internal
SecurityOrigin: null


status: 200
response:
SHIORI/3.0 200 OK
Charset: UTF-8
Value: testghost


unloaded
```

testghost はディレクトリに `fail` を含むと load に失敗する。そのときの出力（終了の状態 3）。`log[1]:` の行は SHIORI 側の `shiori4_abi`（`shiori4` が使う層） が load の失敗を書いた log で stdout に、`load error:` の行は stderr に出る。

```text
> shiori4-testhost testghost.dll <dir>fail\
log[1]: shiori load failed: status 500
load error: LoadFailed: SHIORI load failed
```

ゴーストのディレクトリを省いた（終了の状態 64。すべて stderr）。

```text
> shiori4-testhost testghost.dll
SHIORI のファイルとゴーストのディレクトリが要る
usage: shiori4-testhost [--entry v3] [-H "名前: 値"]... <SHIORI のファイル> <ゴーストのディレクトリ> [ID]
信頼できる SHIORI だけを渡す（ライブラリを開くと、その初期化のコードが動く）
```

## x86 の Windows の従来の口

x86 の Windows（`i686-pc-windows-msvc` などでビルドした試験用のホスト）では、従来の SHIORI/3.0 の口（`loadu`・`load`）を `shiori4_host` の `LegacyRuntime` で読んで往復させる。ほかの環境の振る舞いは前の節までのとおりで、この節は x86 の Windows だけのことである。

- SHIORI のファイルは絶対パスで渡す。相対のパスは `load error: InvalidPath: …` で終了の状態 3 になる（`load_ghost_with` が x86 の Windows では絶対パスだけを開くため）。使い方の表示も `<SHIORI のファイル（絶対パス）>` になる。
- `--entry legacy` で従来の口（`loadu` → `load` の順）だけを許可する。`shiori_get_api` と従来の口の両方を持つ DLL の、従来の口を試すときに使う。`shiori_get_api` を持つ DLL は、省くと v4・v3 互換モードで解決される（DLL の口と発見の順 2）。従来の口を持たない DLL に `--entry legacy` を付けると、読み込みの失敗（終了の状態 3）になる。
- 従来の口に解決すると、`entry: loadu`（または `entry: load`）を出し、その口で初期化する。初期化の失敗（`loadu`・`load` が 0 を返した、など）は stderr に `load error:` を出して終了の状態 3 で終わる。
- 要求は v3 互換モードと同じ全文（`GET SHIORI/3.0`・`Charset: UTF-8`・5 つのヘッダ・`-H` のヘッダ・空行）で、1 回だけ往復させる。
- 応答は末尾に続く NUL を落とし、UTF-8 として読めないバイトを置換文字にして `response:` に出す（UTF-8 でなくても往復は失敗にしない）。`status:` は応答の開始行から取り、読めなければ `0` で、終了の状態は v3 互換モードと同じ（2xx は 0、それ以外は 1）。
- 要求の失敗は、stdout に `request error: <名前>: <説明>` を出して終了の状態 2 で終わる。名前は `AllocFailed`（要求のメモリを確保できなかった）・`NullResponse`（`request` が NULL を返した）・`NegativeLength`（`request` が負の長さを書き戻した）のどれかである。
- 最後に unload して `unloaded` を出す。終了の状態 4 は出ない。

テスト用の従来の SHIORI [`testdata/legacyghost`](https://github.com/ekicyou/shiori4/tree/main/testdata/legacyghost) を x86 の Windows で読んだときの出力を写す。`<lib>` は `legacyghost.dll` の絶対パス、`<dir>` はゴーストのディレクトリの絶対パスである。

`loadu` で読み、`OnBoot` を要求した（終了の状態 0。`--entry legacy` を付けても同じ）。

```text
> shiori4-testhost <lib> <dir>
entry: loadu
request:
GET SHIORI/3.0
Charset: UTF-8
SecurityLevel: local
ID: OnBoot
Sender: shiori4-testhost
SenderType: internal
SecurityOrigin: null


status: 200
response:
SHIORI/3.0 200 OK
Charset: UTF-8
Value: legacyghost


unloaded
```

legacyghost はディレクトリに `fail` を含むと `loadu`・`load` が 0 を返す。そのときの出力（終了の状態 3。`load error:` の行は stderr）。

```text
> shiori4-testhost <lib> <dir>fail\
entry: loadu
load error: LoadFailed: SHIORI load failed
```

SHIORI のファイルを相対のパスで渡した（終了の状態 3。stderr）。

```text
> shiori4-testhost legacyghost.dll <dir>
load error: InvalidPath: ghost directory or library path is not an absolute path, or the directory is not valid UTF-8
```

## 取り込み方

試験用のホストは `shiori4_host` の example なので、`cargo install` に `--example` を付けて取り込む。`--tag` で版を固定する。`vX.Y.Z` には、この example を含む版のタグを入れる（この example は `v0.1.0` から含まれる。`v0.0.2` のタグには無い）。

```bash
cargo install --git https://github.com/ekicyou/shiori4 --tag vX.Y.Z shiori4_host --example shiori4-testhost
```

crates.io に公開された版から取り込むときは、次の形になる（`X.Y.Z` はこの example を含む版。この形はまだ確かめていない）。

```bash
cargo install shiori4_host --version X.Y.Z --example shiori4-testhost
```

どちらも `shiori4-testhost`（Windows では `shiori4-testhost.exe`）を cargo の bin のディレクトリに置く。

## CI の例

GitHub Actions の 1 段の例を示す。**この例は検査しない**（他のリポジトリの CI の中でしか動かせないため）。Linux のランナーで、SHIORI の `libmy_shiori.so` を `target/release` にビルドした後に置く想定で、`vX.Y.Z` と名前は自分のものに置き換える。

```yaml
- name: shiori4-testhost で往復を確かめる
  shell: bash
  run: |
    cargo install --git https://github.com/ekicyou/shiori4 --tag vX.Y.Z shiori4_host --example shiori4-testhost
    shiori4-testhost "$GITHUB_WORKSPACE/target/release/libmy_shiori.so" "$GITHUB_WORKSPACE/"
```

- 試験用のホストが 0 以外で終わると、この段が失敗する。
- ゴーストのディレクトリは絶対パスで渡す。Windows のランナーでは OS の形のパスにする。shiori4 自身の CI がその書き方の例で、[`.github/workflows/build.yml`](https://github.com/ekicyou/shiori4/blob/main/.github/workflows/build.yml) の job `textbook` の「Round trips」の段が、3 OS で試験用のホストを呼んで終了の状態を確かめている。x86 の Windows の従来の口は、job `test-i686`（`test-i686-windows`）の「Round trips」の段が legacyghost で確かめている。
