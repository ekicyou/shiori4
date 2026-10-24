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
