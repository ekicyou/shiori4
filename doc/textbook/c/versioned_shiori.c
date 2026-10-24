/* versioned_shiori.c — 版ごとに別の静的なテーブルを返す C の SHIORI（v4 と v3 互換モード。OnBoot に応答する） */
#define SHIORI_BUILD_MODULE /* shiori_get_api を書き出す側でだけ定義する */
#include "shiori4_abi.h"
#include <stdlib.h>
#include <string.h>

typedef struct { const ShioriHostApi* host; } Inst; /* 状態はインスタンスごとに持つ（load の規約 5） */

/* req は NUL 終端ではない（ABI 規約 3）ので、len の中で行の頭を探す */
static int has_line(const uint8_t* req, size_t len, const char* key) {
    size_t n = strlen(key);
    for (size_t i = 0; i + n <= len; i++)
        if ((i == 0 || req[i - 1] == '\n') && memcmp(req + i, key, n) == 0) return 1;
    return 0;
}
static int starts_with(const uint8_t* req, size_t len, const char* head) {
    size_t n = strlen(head);
    return len >= n && memcmp(req, head, n) == 0;
}
/* out はホストが作る（ABI 規約 5）。呼ばれた側は out を最後に書く（ABI 規約 6） */
static ShioriStatus reply(void* inst, ShioriStatus status, const char* text, ShioriBytes* out) {
    const ShioriHostApi* h = ((Inst*)inst)->host;
    h->copy_from_slice(h->ctx, (const uint8_t*)text, strlen(text), out);
    return status;
}
static Inst* new_inst(const ShioriHostApi* host) {
    Inst* in = malloc(sizeof *in);
    if (in) in->host = host; /* unload が返るまで有効（スレッド規約 5） */
    return in;
}
static void unload(void* inst) { free(inst); }

/* ---- v4: ペイロードはヘッダ部分だけ（版 1）。応答に Charset を付けない（版 3） ---- */
static void* load_v4(const ShioriHostApi* host, const uint8_t* dir, size_t dir_len) {
    (void)dir; (void)dir_len;
    if (!host || !SHIORI_HOST_API_IS_VALID(host)) return NULL;
    int ok = SHIORI_HAS_FIELD(host, ShioriHostApi, copy_from_slice) && host->copy_from_slice;
    Inst* in = ok ? new_inst(host) : NULL;
    if (!in && host->log) /* NULL を返す前に log で理由を出す（load の規約 3） */
        host->log(host->ctx, SHIORI_LOG_ERROR, (const uint8_t*)"load failed", 11);
    return in;
}
static ShioriStatus request_v4(void* inst, const uint8_t* req, size_t len, ShioriBytes* out) {
    if (!has_line(req, len, "ID: OnBoot\r\n")) return SHIORI_204_NO_CONTENT;
    return reply(inst, SHIORI_200_OK, "Value: \\0\\s[0]こんにちは（v4）。\\e\r\n", out);
}
static ShioriStatus notify_v4(void* inst, const uint8_t* req, size_t len) {
    (void)inst; (void)req; (void)len;
    return SHIORI_204_NO_CONTENT;
}

/* ---- v3 互換モード: 要求も応答も SHIORI/3.0 の全文（版 2） ---- */
static void* load_v3(const ShioriHostApi* host, const uint8_t* dir, size_t dir_len) {
    (void)dir; (void)dir_len;
    /* host は copy_from_slice だけを持つテーブル。log は無く、この例は自身のログも持たないので、失敗は NULL を返すだけ（load の規約 3） */
    if (!host || !SHIORI_HAS_FIELD(host, ShioriHostApi, copy_from_slice) || !host->copy_from_slice)
        return NULL;
    return new_inst(host);
}
static ShioriStatus request_v3(void* inst, const uint8_t* req, size_t len, ShioriBytes* out) {
    /* 戻り値は開始行と同じステータスコード。文字コードは UTF-8 固定（版 2） */
    if (starts_with(req, len, "GET SHIORI/3.0\r\n") && has_line(req, len, "ID: OnBoot\r\n"))
        return reply(inst, SHIORI_200_OK,
                     "SHIORI/3.0 200 OK\r\nCharset: UTF-8\r\n"
                     "Value: \\0\\s[0]こんにちは（v3）。\\e\r\n\r\n", out);
    if (starts_with(req, len, "GET SHIORI/3.0\r\n") || starts_with(req, len, "NOTIFY SHIORI/3.0\r\n"))
        return reply(inst, SHIORI_204_NO_CONTENT, "SHIORI/3.0 204 No Content\r\nCharset: UTF-8\r\n\r\n", out);
    /* SHIORI/2.x などには 400（版 2） */
    return reply(inst, SHIORI_400_BAD_REQUEST, "SHIORI/3.0 400 Bad Request\r\nCharset: UTF-8\r\n\r\n", out);
}
static ShioriStatus notify_v3(void* inst, const uint8_t* req, size_t len) {
    (void)inst; /* notify の req は NOTIFY の全文。GET なら 400（版 2） */
    return starts_with(req, len, "NOTIFY SHIORI/3.0\r\n") ? SHIORI_204_NO_CONTENT : SHIORI_400_BAD_REQUEST;
}

/* 版ごとに別の静的なテーブル。abi_version は要求された版と一致させる（版 4） */
static const ShioriApi API_V4 = { sizeof(ShioriApi), SHIORI_ABI_VERSION, load_v4, unload, request_v4, notify_v4 };
static const ShioriApi API_V3 = { sizeof(ShioriApi), SHIORI_ABI_VERSION_3, load_v3, unload, request_v3, notify_v3 };
SHIORI_EXPORT const ShioriApi* shiori_get_api(uint32_t v) { /* ほかの版には NULL（ABI 規約 1） */
    if (v == SHIORI_ABI_VERSION) return &API_V4;
    if (v == SHIORI_ABI_VERSION_3) return &API_V3;
    return NULL;
}
