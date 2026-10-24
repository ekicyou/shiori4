# shiori4 workspace

SHIORI とホスト（ベースウェア）を C ABI でつなぐ、SHIORI 次世代 ABI のワークスペースです。

| クレート | 役割 | リンク |
|---|---|---|
| [`shiori4_abi`](shiori4_abi/README.md) | ABI 定義（C ABI の関数テーブル、ShioriBytes、版、規約）。C ヘッダ `include/shiori4_abi.h` 付き | [crates.io](https://crates.io/crates/shiori4_abi) |
| [`shiori4`](shiori4/README.md) | SHIORI 側の高水準層（`Ghost` trait、v3/v4 変換）と、語に合わせた便利関数のモジュール `shiori4::protocol`（ヘッダの型、プロパティの取得、Tag の待ち合わせ、語の定数） | [crates.io](https://crates.io/crates/shiori4) |
| [`shiori4_host`](shiori4_host/README.md) | ホスト側の高水準層（`Host` trait、`GhostRuntime`、post キュー、完了通知、pump） | [crates.io](https://crates.io/crates/shiori4_host) |

## 全体図

誰が誰を、どのスレッドから、いつ呼べるかを 1 つの表にまとめます。「根拠」は[規約の一覧](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/rules.md)の番号で、規約の文はそちらにあります。`host.` はホストが load に渡すテーブル（`ShioriHostApi`）の関数です。

| 関数 | 呼ぶ側 → 呼ばれる側 | 呼べるスレッド | 呼べる期間 | 外れたときの結果 | 根拠 |
|------|------|------|------|------|------|
| `load` | ホスト → SHIORI | ホストがインスタンスごとに決めた 1 つのスレッド | インスタンスの始まり | 失敗は NULL。v4 は NULL を返す前に `host.log` で理由を出す | スレッド規約 1、load の規約 3 |
| `request` | ホスト → SHIORI | load と同じスレッド | load が返ってから unload を呼ぶまで | — | スレッド規約 1 |
| `notify` | ホスト → SHIORI | load と同じスレッド | load が返ってから unload を呼ぶまで | — | スレッド規約 1・7 |
| `host.request` | SHIORI → ホスト | load・request・notify を呼ばれているスレッド | その呼び出しが返るまで（v4） | ホストは `SHIORI_E_WRONG_THREAD`（-7）を返してよい（out は空）。処理中にホストは SHIORI を呼び返さない（`SHIORI_E_REENTRANT`（-4）） | スレッド規約 2・3 |
| `host.post` | SHIORI → ホスト | 任意 | unload が返るまで | unload を始めたホストは `SHIORI_E_SHUTDOWN`（-5）を返してよい | スレッド規約 4・5、post の完了通知 2 |
| `host.log` | SHIORI → ホスト | 任意 | unload が返るまで | — | スレッド規約 4・5 |
| `host.copy_from_slice` | SHIORI → ホスト | 任意 | unload が返るまで | 写せなかったら、ホストが自分で決めた印を `out` に残す（SHIORI には知らせない） | ABI 規約 5、スレッド規約 5・6 |
| `ShioriBytes.release` | `out` を受け取った側 → ホスト（`out` の `ShioriBytes` は常にホストが作る） | 任意 | いつでも（インスタンスの unload の後でもよい） | — | ABI 規約 5・6、スレッド規約 5・6 |
| `unload` | ホスト → SHIORI | load と同じスレッド | インスタンスの終わり。長く止まらない | 返った後、SHIORI はホストのどの関数も呼ばない（`release` だけは例外） | スレッド規約 1・5 |

- **v3 互換モード**（`shiori_get_api(3)`）では、load に渡す host は `copy_from_slice` だけを持ち（`request`・`post`・`log` は NULL）、SHIORI は `copy_from_slice` があるかだけを確かめます。`host.request`・`host.post`・`host.log` の行は使えません（版 2）。load の失敗の理由は、host に log が無いので SHIORI 自身のログに出します（load の規約 3）。
- **unload を始めた後と、返った後**: 始めた後は、ホストは `host.post` に `SHIORI_E_SHUTDOWN` を返してよく（スレッド規約 5）、そのインスタンス宛ての未配送のイベントを破棄します（post の完了通知 3）。返った後は、SHIORI は `ShioriHostApi` のどの関数も呼びません。例外は `ShioriBytes.release` だけで、いつでも呼べます（スレッド規約 5・6）。

## 最小の例

### Rust の SHIORI

`OnBoot` に `Value` で答え、`export_ghost!` で `shiori_get_api` を書き出します。v4 と v3 互換モードの両方のテーブルを返します（`Ghost::SUPPORTS_V3` は既定で true）。クレートの種類は `crate-type = ["cdylib"]` にします。この例は偽のホストで v4 と v3 の往復を実行して確かめています。

```rust
use shiori4::protocol::{HeaderWriter, Headers, HEADER_VALUE};
use shiori4::{Ghost, ShioriHostHandle, ShioriResponse, ShioriStatus};

struct MyGhost; // 状態は static に置かず Self に持つ（load の規約 5）

impl Ghost for MyGhost {
    fn load(_host: Option<ShioriHostHandle>, _dir: &str) -> Result<Self, ShioriStatus> {
        Ok(Self)
    }

    // req はヘッダ部分の原文（v3 でも、Adapter が SHIORI/3.0 の全文から取り出して渡す）
    fn request(&mut self, req: &str) -> ShioriResponse {
        match Headers::parse(req).id() {
            Some("OnBoot") => HeaderWriter::new()
                .push(HEADER_VALUE, "\\0\\s[0]こんにちは。\\e")
                .into_response(),
            _ => ShioriResponse::no_content(),
        }
    }
}

shiori4::export_ghost!(MyGhost);
# fn main() {
#     struct FakeHost;
#     impl shiori4::ShioriHost for FakeHost {
#         fn request(&self, _req: &[u8]) -> ShioriResponse {
#             ShioriResponse::no_content()
#         }
#         fn post(&self, _msg: &[u8]) -> ShioriStatus {
#             shiori4::SHIORI_204_NO_CONTENT
#         }
#     }
#     static API: shiori4::ShioriApi =
#         shiori4::ShioriApi::new::<shiori4::Adapter<MyGhost>, { shiori4::SHIORI_ABI_VERSION }>();
#     static API_V3: shiori4::ShioriApi =
#         shiori4::ShioriApi::new::<shiori4::Adapter<MyGhost>, { shiori4::SHIORI_ABI_VERSION_3 }>();
#     let host = shiori4::ShioriHostBox::new(FakeHost);
#     let mut v4 = API.load(&host, "/ghost").expect("load v4");
#     let r = v4.request("ID: OnBoot\r\n").expect("abi");
#     assert_eq!(r.status, shiori4::SHIORI_200_OK);
#     assert_eq!(r.body.as_str().unwrap(), "Value: \\0\\s[0]こんにちは。\\e\r\n");
#     let mut v3 = API_V3.load_v3("/ghost").expect("load v3");
#     let r = v3.request("GET SHIORI/3.0\r\nCharset: UTF-8\r\nID: OnBoot\r\n\r\n").expect("abi");
#     assert_eq!(
#         r.body.as_str().unwrap(),
#         "SHIORI/3.0 200 OK\r\nCharset: UTF-8\r\nValue: \\0\\s[0]こんにちは。\\e\r\n\r\n"
#     );
# }
```

得られる応答（v4。ステータス 200、改行は CRLF）:

```text
Value: \0\s[0]こんにちは。\e
```

v3 互換モードで `GET SHIORI/3.0` の全文を送ると、SHIORI/3.0 の応答の全文になります。

```text
SHIORI/3.0 200 OK
Charset: UTF-8
Value: \0\s[0]こんにちは。\e

```

### Rust のホスト

SHIORI を読む → `OnBoot` を要求する → 応答を表示する → Drop で unload する、の順です。要求には[ホスト側プロトコル仕様](https://github.com/ekicyou/shiori4/blob/main/doc/host-protocol.md)の第 2.2 節の必須ヘッダを付けます。この例は DLL を要するので、ビルドだけを検査します（実行しない）。同じ流れを[試験用のホスト](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/testhost.md)が CI の 3 つの OS で実行しています。

```rust,no_run
use shiori4_host::{GhostRuntime, Host, Post};

struct MyHost;

impl Host for MyHost {
    fn name(&self) -> &str {
        "MyBase"
    }
    // post は pump の中で渡される。この例は pump を呼ばないので、呼ばれない
    fn on_post(&self, _post: Post) {}
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 使い方: <SHIORI のファイル> <ゴーストのディレクトリ>（絶対パス。相対パスは LoadError::InvalidPath）
    let mut args = std::env::args_os().skip(1);
    let lib = args.next().ok_or("SHIORI のファイルが無い")?;
    let dir = args.next().ok_or("ゴーストのディレクトリが無い")?;
    let mut rt = GhostRuntime::load(lib, dir, MyHost)?; // 読んで v4 で load する
    let req = "SecurityLevel: local\r\nID: OnBoot\r\nSender: MyBase\r\n\
               SenderType: internal\r\nSecurityOrigin: null\r\n";
    let r = rt.request(req).map_err(|s| format!("ABI error: {s}"))?;
    println!("status {}", r.status);
    print!("{}", r.body.as_str()?);
    drop(rt); // unload して、ライブラリを放す
    Ok(())
}
```

上の Rust の SHIORI を読ませると、次を表示します。

```text
status 200
Value: \0\s[0]こんにちは。\e
```

### C の SHIORI

v4 のテーブルだけを返す最小の SHIORI です（v3 も返す形は [C の書き手向けの説明](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/c.md)の「版の判定」）。CI が 3 つの OS でビルドし、試験用のホストで読ませています。得られる応答はステータス 200 と次の 1 行（改行は CRLF）です。

```text
Value: \0\s[0]こんにちは。\e
```

<details>
<summary><code>doc/textbook/c/minimal_shiori.c</code> の全文</summary>

```c
/* minimal_shiori.c — 最小の C の SHIORI（v4 のテーブルだけを返す。OnBoot に応答する） */
#define SHIORI_BUILD_MODULE /* shiori_get_api を書き出す側でだけ定義する */
#include "shiori4_abi.h"
#include <stdlib.h>
#include <string.h>

typedef struct { const ShioriHostApi* host; } Inst; /* 状態はインスタンスごとに持つ（load の規約 5） */

static void* load(const ShioriHostApi* host, const uint8_t* dir, size_t dir_len) {
    (void)dir; (void)dir_len;
    if (!host || !SHIORI_HOST_API_IS_VALID(host)) return NULL;
    int ok = SHIORI_HAS_FIELD(host, ShioriHostApi, copy_from_slice) && host->copy_from_slice;
    Inst* in = ok ? malloc(sizeof *in) : NULL;
    if (!in) { /* NULL を返す前に log で理由を出す（load の規約 3） */
        if (host->log) host->log(host->ctx, SHIORI_LOG_ERROR, (const uint8_t*)"load failed", 11);
        return NULL;
    }
    in->host = host; /* unload が返るまで有効（スレッド規約 5） */
    return in;
}
static void unload(void* inst) { free(inst); }

static ShioriStatus request(void* inst, const uint8_t* req, size_t len, ShioriBytes* out) {
    static const char key[] = "ID: OnBoot\r\n", res[] = "Value: \\0\\s[0]こんにちは。\\e\r\n";
    int boot = 0; /* req は NUL 終端ではない（ABI 規約 3）ので、len の中で行の頭を探す */
    for (size_t i = 0; !boot && i + sizeof key - 1 <= len; i++)
        boot = (i == 0 || req[i - 1] == '\n') && memcmp(req + i, key, sizeof key - 1) == 0;
    if (!boot) return SHIORI_204_NO_CONTENT;
    /* out はホストが作る（ABI 規約 5）。呼ばれた側は out を最後に書く（ABI 規約 6） */
    const ShioriHostApi* h = ((Inst*)inst)->host;
    h->copy_from_slice(h->ctx, (const uint8_t*)res, sizeof res - 1, out);
    return SHIORI_200_OK;
}
static ShioriStatus notify(void* inst, const uint8_t* req, size_t len) {
    (void)inst; (void)req; (void)len;
    return SHIORI_204_NO_CONTENT;
}

static const ShioriApi API = { sizeof(ShioriApi), SHIORI_ABI_VERSION, load, unload, request, notify };
SHIORI_EXPORT const ShioriApi* shiori_get_api(uint32_t v) { /* ほかの版には NULL（ABI 規約 1） */
    return v == SHIORI_ABI_VERSION ? &API : NULL;
}
```

</details>

### C のホスト

ライブラリを開く → `shiori_get_api(4)`（無ければ 3）→ load → `OnBoot` を要求する → 応答を表示する → unload する、の順です。使い方は `minimal_host <SHIORI のファイル> <ゴーストのディレクトリ>` です。CI は試験用の SHIORI（`testdata/testghost`）を読ませ、終了の状態 0 と `Value: testghost` の行を確かめています。表示は次のとおりです。

```text
abi 4
status 200
Value: testghost
```

<details>
<summary><code>doc/textbook/c/minimal_host.c</code> の全文</summary>

```c
/* minimal_host.c — 最小の C のホスト（開く → shiori_get_api(4)、無ければ 3 → load → OnBoot → unload）
 * 使い方: minimal_host <SHIORI のファイル> <ゴーストのディレクトリ>
 *   ディレクトリは絶対パス・OS の区切り・末尾に区切り・UTF-8 で与える（load の規約 1。この例は確かめずに渡す）
 *   引数は UTF-8 とみなす。Windows では、パスが ASCII だけかコードページが UTF-8 のときに限って成り立つ
 * 表示: 1 行目に abi <版>、2 行目に status <戻り値>、続けて応答の全文。終了の状態は 2xx で 0、ほかは 1
 * v3 の枝はビルドだけを検査する（CI の testghost は v4 を返すので、v3 の枝は通らない）
 */
#ifndef _WIN32
#define _POSIX_C_SOURCE 200809L /* dlopen を使うため */
#endif
#include "shiori4_abi.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#include <windows.h>
static void* open_lib(const char* path) { /* 引数は UTF-8 とみなし、UTF-16 に変えて開く */
    wchar_t w[4096];
    return MultiByteToWideChar(CP_UTF8, 0, path, -1, w, 4096) ? (void*)LoadLibraryW(w) : NULL;
}
static ShioriGetApiFn find_get_api(void* lib) {
    return (ShioriGetApiFn)GetProcAddress((HMODULE)lib, SHIORI_GET_API_SYMBOL);
}
static void close_lib(void* lib) { FreeLibrary((HMODULE)lib); }
#else
#include <dlfcn.h>
static void* open_lib(const char* path) { return dlopen(path, RTLD_NOW | RTLD_LOCAL); }
static ShioriGetApiFn find_get_api(void* lib) { return (ShioriGetApiFn)dlsym(lib, SHIORI_GET_API_SYMBOL); }
static void close_lib(void* lib) { dlclose(lib); }
#endif

/* ---- ホストのテーブル。out の ShioriBytes は常にホストが作り、ホストの release で解放する（ABI 規約 5） ---- */
static char copy_failed; /* 写せなかったときに out に残す、このホストが決めた印 */

static void release(ShioriBytes* b) { /* いつでも・どのスレッドから呼ばれてもよい（スレッド規約 6） */
    free(b->owner);
    *b = (ShioriBytes)SHIORI_BYTES_EMPTY;
}
static void copy_from_slice(void* ctx, const uint8_t* ptr, size_t len, ShioriBytes* out) {
    (void)ctx;
    uint8_t* p = malloc(len ? len : 1);
    if (!p) { /* SHIORI には知らせず、印を残す（ABI 規約 5） */
        *out = (ShioriBytes){ NULL, 0, &copy_failed, NULL };
        return;
    }
    if (len) memcpy(p, ptr, len);
    *out = (ShioriBytes){ p, len, p, release };
}
static ShioriStatus host_request(void* ctx, const uint8_t* req, size_t len, ShioriBytes* out) {
    (void)ctx; (void)req; (void)len; (void)out;
    return SHIORI_400_BAD_REQUEST; /* この例は SHIORI からの要求に答えない */
}
static ShioriStatus host_post(void* ctx, const uint8_t* msg, size_t len) {
    (void)ctx; (void)msg; (void)len;
    return SHIORI_204_NO_CONTENT; /* 受け付けるだけで実行しない（post の完了通知 2） */
}
static void host_log(void* ctx, int32_t level, const uint8_t* msg, size_t len) {
    (void)ctx;
    fprintf(stderr, "log %d: %.*s\n", (int)level, (int)len, (const char*)msg);
}

int main(int argc, char** argv) {
    if (argc != 3) {
        fprintf(stderr, "usage: minimal_host <SHIORI のファイル> <ゴーストのディレクトリ>\n");
        return 1;
    }
    void* lib = open_lib(argv[1]);
    if (!lib) {
        fprintf(stderr, "cannot open %s\n", argv[1]);
        return 1;
    }
    ShioriGetApiFn get_api = find_get_api(lib);
    if (!get_api) {
        fprintf(stderr, "no %s in %s\n", SHIORI_GET_API_SYMBOL, argv[1]);
        close_lib(lib);
        return 1;
    }
    uint32_t v = SHIORI_ABI_VERSION; /* 4 が NULL なら 3 を試す（DLL の口と発見の順 2） */
    const ShioriApi* api = get_api(v);
    if (!api) api = get_api(v = SHIORI_ABI_VERSION_3);
    /* 版が要求と一致し（版 4）、基本のフィールドが揃っているか。
     * 将来足されたフィールドは struct_size を使って同じ形で確かめる（ABI 規約 7）。ここでは notify で示す */
    if (!api || api->abi_version != v || !SHIORI_API_IS_VALID(api) || !SHIORI_HAS_FIELD(api, ShioriApi, notify)) {
        fprintf(stderr, "no usable table\n");
        close_lib(lib);
        return 1;
    }
    /* host は unload が返るまで有効にしておく（スレッド規約 5）。v3 は copy_from_slice だけ（版 2） */
    ShioriHostApi host = v == SHIORI_ABI_VERSION
        ? (ShioriHostApi){ sizeof host, v, NULL, host_request, host_post, host_log, copy_from_slice }
        : (ShioriHostApi){ sizeof host, v, NULL, NULL, NULL, NULL, copy_from_slice };
    void* inst = api->load(&host, (const uint8_t*)argv[2], strlen(argv[2]));
    if (!inst) {
        fprintf(stderr, "load failed\n");
        close_lib(lib);
        return 1;
    }
    /* v4 はヘッダ部分だけ（版 1）、v3 は SHIORI/3.0 の要求全文（版 2）。
     * ホストが必ず付けるヘッダと並びは doc/host-protocol.md 第 2.2 節 */
#define HEADERS "SecurityLevel: local\r\nID: OnBoot\r\nSender: minimal_host\r\n" \
                "SenderType: internal\r\nSecurityOrigin: null\r\n"
    const char* req = v == SHIORI_ABI_VERSION ? HEADERS : "GET SHIORI/3.0\r\nCharset: UTF-8\r\n" HEADERS "\r\n";
    ShioriBytes out = SHIORI_BYTES_EMPTY;
    ShioriStatus st = api->request(inst, (const uint8_t*)req, strlen(req), &out);
    printf("abi %u\nstatus %d\n", (unsigned)v, (int)st);
    int copied = out.owner != &copy_failed;
    if (!copied) fprintf(stderr, "copy_from_slice failed\n");
    if (out.len) fwrite(out.ptr, 1, out.len, stdout);
    shiori_bytes_release(&out); /* 戻り値に関わらず解放する（ABI 規約 6） */
    api->unload(inst);
    close_lib(lib);
    return SHIORI_IS_SUCCESS(st) && copied ? 0 : 1;
}
```

</details>

## 道案内

読む順は次のとおりです。

1. 全体図と最小の例（この README）
2. [規約の一覧と理由](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/rules.md): 規約の文と番号、各規約の理由
3. [語の表と確定の範囲](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/vocabulary.md): SHIORI とホストが交わすメッセージの語
4. 細部
   - [docs.rs の `shiori4`](https://docs.rs/shiori4)（SHIORI 側の例）・[`shiori4_host`](https://docs.rs/shiori4_host)（ホスト側の例）・[`shiori4_abi`](https://docs.rs/shiori4_abi)（ABI の型）
   - [C の書き手向けの説明](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/c.md)
   - [移行の手引き](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/migration.md)（SHIORI/3.0 から）
   - [配布の決まり](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/distribution.md)（DLL の名前と選び方）
   - 補い: [別のプロセスの注意](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/process.md)・[試験用のホストの使い方](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/testhost.md)

教科書の目次は [doc/textbook/README.md](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/README.md) です。正本は 2 つで、食い違ったら正本が正しいです。

- 規約: C ヘッダ [`shiori4_abi/include/shiori4_abi.h`](https://github.com/ekicyou/shiori4/blob/main/shiori4_abi/include/shiori4_abi.h) の冒頭の注釈（同じ文が `shiori4_abi` の[クレートのドキュメント](https://docs.rs/shiori4_abi)にあります）
- メッセージの語（getproperty・post の Command・OnHostComplete など）: [ホスト側プロトコル仕様](https://github.com/ekicyou/shiori4/blob/main/doc/host-protocol.md)。芯（`Ghost`・`Host` と runtime）は中身を解釈せず原文を渡し、`shiori4::protocol`（`shiori4_host::protocol` からも使える）は使いたい人が使う便利関数です
