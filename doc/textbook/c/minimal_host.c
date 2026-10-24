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
