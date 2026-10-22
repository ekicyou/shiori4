/*
 * shiori4_abi.h — SHIORI 次世代 ABI 定義（Rust 版 shiori4_abi と同一レイアウト）
 *
 * 版（abi_version）
 *  - SHIORI_ABI_VERSION (4): 既定。host 必須。ペイロードはヘッダ部分だけ（開始行なし）
 *  - SHIORI_ABI_VERSION_3 (3): SHIORI3 互換モード。異なる OS の互換ベースウェア向け
 *     - load の host は NULL（渡されても SHIORI は無視する）。ホストの呼び出しは使えない
 *     - request: req は SHIORI/3.0 の要求全文（"GET SHIORI/3.0" / "NOTIFY SHIORI/3.0" で始まる）
 *                out は SHIORI/3.0 の応答全文（"SHIORI/3.0 200 OK" で始まり空行で終わる）
 *                戻り値は開始行と同じステータスコード（正本は文字列）
 *                負の値は ABI エラー（out は空。500 として扱う）
 *     - notify : req は "NOTIFY SHIORI/3.0" で始まる全文。GET なら 400
 *     - 文字コードは UTF-8 固定（Charset: UTF-8）。SHIORI/2.x の要求には 400
 *     - v3 は v4 と同じ機能を出すことを保証しない
 *  - ホストは shiori_get_api(v) で版を選ぶ。返るテーブルの abi_version は v と一致する
 *
 * ABI 規約
 *  - 公開シンボルは shiori_get_api の1つだけ。対応できない版なら NULL を返す
 *  - ペイロードは UTF-8 のバイト列（ptr と len）。NUL 終端にはしない
 *  - 引数の (ptr, len) は呼び出しの間だけ有効な借用。保持する場合は受け取った側がコピーする
 *  - 所有権が移るのは out に書き込まれる ShioriBytes だけ。確保した側の release で解放する
 *  - 呼び出された側は out を最後に書く。呼び出した側は戻り値に関係なく out を解放する
 *  - 構造体は末尾にだけフィールドを追加する（追加する関数ポインタは NULL 許容）
 *
 * 戻り値（ShioriStatus）
 *  - 0 以上: SHIORI/3.0 のステータスコード
 *  - 負の値: ABI 層のエラー（SHIORI_E_*）。SHIORI_E_PANIC ならホストはインスタンスを unload する
 *
 * スレッド規約
 *  1. ShioriApi の request / notify は、ホストが決めた単一スレッドからだけ呼ぶ
 *  2. SHIORI は load / request / notify の処理中に host->request を同期で呼んでよい（v4）
 *  3. host->request の処理中に、ホストから SHIORI を呼び返してはならない（SHIORI_E_REENTRANT）
 *  4. host->post は任意のスレッドから呼べる。ホストは msg をコピーしてキューに積み、すぐ返る
 *  5. ShioriHostApi は unload が返るまで有効。unload の後の post は SHIORI_E_SHUTDOWN
 *     SHIORI は unload の中で、post を呼ぶ自分のスレッドをすべて止めてから返る
 *
 * post の完了通知（v4）
 *  - 受付 ID は SHIORI が発行し、Tag ヘッダとしてメッセージに入れる（登録してから post する）
 *  - post の戻り値は受付可否だけ（204: 受付、400: 不正、SHIORI_E_SHUTDOWN: 終了中）
 *  - 完了はホストが通常のイベント（request または notify）として、post したインスタンスに届ける
 *    post の呼び出し中や host->request の処理中には届けない
 *    unload を始めたら、そのインスタンス宛ての未配送イベントは破棄する
 *  - Tag を付けない post は「結果は不要」を意味する
 */
#ifndef SHIORI4_ABI_H
#define SHIORI4_ABI_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

#if defined(_WIN32)
#  define SHIORI_EXPORT __declspec(dllexport)
#else
#  define SHIORI_EXPORT __attribute__((visibility("default")))
#endif

/* ============ 定数 ============ */
#define SHIORI_ABI_VERSION      4u   /* 既定 */
#define SHIORI_ABI_VERSION_3    3u   /* SHIORI3 互換モード（host は NULL） */
#define SHIORI_ABI_VERSION_MIN  3u
#define SHIORI_GET_API_SYMBOL   "shiori_get_api"

typedef int32_t ShioriStatus;

/* SHIORI/3.0 のステータスコード（0 以上） */
#define SHIORI_200_OK                     200
#define SHIORI_204_NO_CONTENT             204
#define SHIORI_311_NOT_ENOUGH             311  /* 互換用。新規実装では非推奨 */
#define SHIORI_312_ADVICE                 312  /* 互換用。新規実装では非推奨 */
#define SHIORI_400_BAD_REQUEST            400
#define SHIORI_500_INTERNAL_SERVER_ERROR  500

/* ABI 層のエラー（負の値） */
#define SHIORI_E_INVALID_ARG   (-1)
#define SHIORI_E_UNSUPPORTED   (-2)
#define SHIORI_E_PANIC         (-3)  /* ホストはこのインスタンスを unload すべき */
#define SHIORI_E_REENTRANT     (-4)
#define SHIORI_E_SHUTDOWN      (-5)
#define SHIORI_E_INVALID_UTF8  (-6)

#define SHIORI_IS_SUCCESS(s)    ((s) >= 200 && (s) < 300)
#define SHIORI_IS_ABI_ERROR(s)  ((s) < 0)

/* ログレベル */
#define SHIORI_LOG_ERROR 1
#define SHIORI_LOG_WARN  2
#define SHIORI_LOG_INFO  3
#define SHIORI_LOG_DEBUG 4

/* ============ 所有権付きバイト列 ============
 * ptr/len : UTF-8 のバイト列（NUL 終端ではない）
 * owner   : 確保した側だけが解釈する不透明なポインタ
 * release : 確保した側の解放関数。受け取った側が使い終わったら必ず呼ぶ（NULL なら不要）
 *           release は呼び出し後に *bytes を空（全フィールド 0）にする
 */
typedef struct ShioriBytes {
    const uint8_t* ptr;
    size_t         len;
    void*          owner;
    void         (*release)(struct ShioriBytes* bytes);
} ShioriBytes;

#define SHIORI_BYTES_EMPTY { NULL, 0, NULL, NULL }

static inline void shiori_bytes_release(ShioriBytes* b) {
    if (b && b->release) b->release(b);
}

/* ============ ホスト → SHIORI に渡すテーブル（v4 のみ） ============ */
typedef struct ShioriHostApi {
    uint32_t struct_size;
    uint32_t abi_version;   /* 4 以上 */
    void*    ctx;

    /* 同期リクエスト。SHIORI のスレッドから、load / request / notify の処理中に限り呼べる
     * 戻り値はステータスコード。out にはヘッダ部分。どの戻り値でも out は解放すること */
    ShioriStatus (*request)(void* ctx, const uint8_t* req, size_t len, ShioriBytes* out);

    /* 非同期通知。任意のスレッドから呼べる
     * msg は呼び出し中のみ有効。ホストは保持する場合コピーする
     * 戻り値: 204 受付 / 400 不正 / SHIORI_E_SHUTDOWN 終了中 */
    ShioriStatus (*post)(void* ctx, const uint8_t* msg, size_t len);

    /* msg は呼び出し中のみ有効 */
    void         (*log)(void* ctx, int32_t level, const uint8_t* msg, size_t len);

    /* 将来のフィールドはここに追加する（NULL 許容） */
} ShioriHostApi;

/* ============ SHIORI → ホスト に返すテーブル ============ */
typedef struct ShioriApi {
    uint32_t struct_size;
    uint32_t abi_version;   /* shiori_get_api で要求した版と同じ値 */

    /* インスタンスを生成する。失敗したら NULL
     * host: v4 では必須（unload が返るまで有効）。v3 では NULL
     * dir : UTF-8 のゴーストディレクトリ（呼び出し中のみ有効） */
    void*        (*load)(const ShioriHostApi* host /* nullable */,
                         const uint8_t* dir, size_t dir_len);

    void         (*unload)(void* inst);

    /* GET に相当する（v3 では NOTIFY 全文も受け付ける）。戻り値はステータスコード
     * out の形式は版による。どの戻り値でも out は解放すること */
    ShioriStatus (*request)(void* inst, const uint8_t* req, size_t len, ShioriBytes* out);

    /* NOTIFY に相当する。本文は返さない */
    ShioriStatus (*notify)(void* inst, const uint8_t* req, size_t len);

    /* 将来のフィールドはここに追加する（NULL 許容） */
} ShioriApi;

typedef const ShioriApi* (*ShioriGetApiFn)(uint32_t requested_abi_version);

/* 唯一のエクスポート。要求された版に対応できなければ NULL を返す
 * SHIORI 側でビルドするときだけ SHIORI_BUILD_MODULE を定義する */
#ifdef SHIORI_BUILD_MODULE
SHIORI_EXPORT const ShioriApi* shiori_get_api(uint32_t requested_abi_version);
#endif

/* 追加フィールドが存在するかの判定
 * 例: SHIORI_HAS_FIELD(api, ShioriApi, notify) */
#define SHIORI_HAS_FIELD(p, T, f) \
    ((size_t)(p)->struct_size >= offsetof(T, f) + sizeof(((T*)0)->f))

/* 基本フィールドが揃っているか */
#define SHIORI_API_IS_VALID(p) \
    ((p)->abi_version >= SHIORI_ABI_VERSION_MIN && SHIORI_HAS_FIELD(p, ShioriApi, notify))
#define SHIORI_HOST_API_IS_VALID(p) \
    ((p)->abi_version >= SHIORI_ABI_VERSION && SHIORI_HAS_FIELD(p, ShioriHostApi, log))

/* ============ レイアウト検査（Rust 版と一致すること） ============ */
#if defined(__cplusplus)
#  define SHIORI_STATIC_ASSERT(c, m) static_assert(c, m)
#else
#  define SHIORI_STATIC_ASSERT(c, m) _Static_assert(c, m)
#endif

SHIORI_STATIC_ASSERT(sizeof(ShioriBytes) == 4 * sizeof(void*), "ShioriBytes size");
SHIORI_STATIC_ASSERT(offsetof(ShioriHostApi, ctx) == 8, "ShioriHostApi.ctx offset");
SHIORI_STATIC_ASSERT(offsetof(ShioriHostApi, log) == 8 + 3 * sizeof(void*), "ShioriHostApi.log offset");
SHIORI_STATIC_ASSERT(sizeof(ShioriHostApi) == 8 + 4 * sizeof(void*), "ShioriHostApi size");
SHIORI_STATIC_ASSERT(offsetof(ShioriApi, load) == 8, "ShioriApi.load offset");
SHIORI_STATIC_ASSERT(offsetof(ShioriApi, notify) == 8 + 3 * sizeof(void*), "ShioriApi.notify offset");
SHIORI_STATIC_ASSERT(sizeof(ShioriApi) == 8 + 4 * sizeof(void*), "ShioriApi size");

#ifdef __cplusplus
}
#endif
#endif /* SHIORI4_ABI_H */
