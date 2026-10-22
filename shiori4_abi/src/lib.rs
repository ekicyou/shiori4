//! shiori4_abi — SHIORI 次世代 ABI 定義（ホスト・SHIORI 共通）
//!
//! このクレートは「バイト列をどう運ぶか」だけを扱い、ペイロードの中身は解釈しない。
//! SHIORI/3.0 の解析・生成、v3/v4 の変換、プロパティ管理は上位クレート（shiori4）の役割。
//!
//! # 版（abi_version）
//! - `SHIORI_ABI_VERSION`（4）: 既定。host 必須。ペイロードはヘッダ部分だけ（開始行なし）
//! - `SHIORI_ABI_VERSION_3`（3）: SHIORI3 互換モード。異なる OS の互換ベースウェア向け
//!   - load の host は NULL（渡されても SHIORI は無視する）。ホストの呼び出しは使えない
//!   - request の req は SHIORI/3.0 の要求全文（"GET SHIORI/3.0" / "NOTIFY SHIORI/3.0" で始まる）
//!   - request の out は SHIORI/3.0 の応答全文（"SHIORI/3.0 200 OK" で始まり空行で終わる）
//!   - 戻り値は開始行と同じステータスコード（正本は文字列）
//!   - notify の req は "NOTIFY SHIORI/3.0" で始まる全文。GET なら 400
//!   - 文字コードは UTF-8 固定。SHIORI/2.x の要求には 400 を返す
//!   - v3 は v4 と同じ機能を出すことを保証しない
//! - ホストは shiori_get_api(v) で版を選ぶ。返るテーブルの abi_version は v と一致する
//!
//! # ABI 規約
//! - 公開シンボルは `shiori_get_api` の1つだけ。対応できない版なら null を返す
//!   （ホストは「shiori_get_api があれば新 ABI、なければ旧 SHIORI」と判別できる）
//! - DLL 境界を越えるのは #[repr(C)] の構造体と extern "C" の関数ポインタだけ
//! - ペイロードは UTF-8 のバイト列（ptr と len）。NUL 終端にはしない
//! - 引数の (ptr, len) は呼び出しの間だけ有効な借用。保持する場合は受け取った側がコピーする
//! - 所有権が移るのは out に書き込まれる ShioriBytes だけ。確保した側の release で解放する
//! - 呼び出された側は out を最後に書く。呼び出した側は戻り値に関係なく out を解放する
//! - 構造体は末尾にだけフィールドを追加する。受け取った側は struct_size を見て判定する
//!   追加する関数ポインタは Option<unsafe extern "C" fn(..)>（C の NULL 許容と同じレイアウト）にする
//!
//! # 戻り値（ShioriStatus）
//! - 0 以上: SHIORI/3.0 のステータスコード（200, 204, 400, 500 など）
//! - 負の値: ABI 層のエラー（SHIORI_E_*）。out は空。SHIORI_E_PANIC ならホストはインスタンスを unload する
//!
//! # スレッド規約
//! 1. ShioriApi の request / notify は、ホストが決めた単一スレッドからだけ呼ぶ
//! 2. SHIORI は load / request / notify の処理中に host.request を同期で呼んでよい（v4）
//! 3. host.request の処理中に、ホストから SHIORI を呼び返してはならない（SHIORI_E_REENTRANT）
//! 4. host.post は任意のスレッドから呼べる。ホストは msg をコピーしてキューに積み、すぐ返る
//! 5. ShioriHostApi は unload が返るまで有効。unload の後の post は SHIORI_E_SHUTDOWN
//!    SHIORI は unload の中で、post を呼ぶ自分のスレッドをすべて止めてから返る
//! 6. エクスポートする関数は catch_unwind で包み、パニックを SHIORI_E_PANIC に変換する
//!
//! # post の完了通知（v4）
//! - 受付 ID は SHIORI が発行し、`Tag` ヘッダとしてメッセージに入れる（登録してから post する）
//! - post の戻り値は受付可否だけ（204: 受付、400: 不正、SHIORI_E_SHUTDOWN: 終了中）
//! - 完了はホストが通常のイベント（request または notify）として、post したインスタンスに届ける
//!   - post の呼び出し中や host.request の処理中には届けない（イベントループから届ける）
//!   - unload を始めたら、そのインスタンス宛ての未配送イベントは破棄する
//! - Tag を付けない post は「結果は不要」を意味する

use std::{
    borrow::Cow,
    ffi::c_void,
    fmt,
    mem::{offset_of, size_of},
    ops::Deref,
    panic::{catch_unwind, AssertUnwindSafe},
    ptr::{self, NonNull},
    str::Utf8Error,
    string::FromUtf8Error,
};

// ============================================================
// 定数
// ============================================================

/// 既定の版
pub const SHIORI_ABI_VERSION: u32 = 4;
/// SHIORI3 互換モード（host は渡されない。ペイロードは SHIORI/3.0 の全文）
pub const SHIORI_ABI_VERSION_3: u32 = 3;
/// この ABI 定義が扱う最小の版
pub const SHIORI_ABI_VERSION_MIN: u32 = 3;

/// 唯一のエクスポートシンボル名（NUL 終端。libloading にそのまま渡せる）
pub const SHIORI_GET_API_SYMBOL: &[u8] = b"shiori_get_api\0";

pub type ShioriStatus = i32;

// ---- SHIORI/3.0 のステータスコード（0 以上）。このクレートでは数値として持つだけ ----
pub const SHIORI_200_OK: ShioriStatus = 200;
pub const SHIORI_204_NO_CONTENT: ShioriStatus = 204;
/// 互換用。新規実装では非推奨
pub const SHIORI_311_NOT_ENOUGH: ShioriStatus = 311;
/// 互換用。新規実装では非推奨
pub const SHIORI_312_ADVICE: ShioriStatus = 312;
pub const SHIORI_400_BAD_REQUEST: ShioriStatus = 400;
pub const SHIORI_500_INTERNAL_SERVER_ERROR: ShioriStatus = 500;

// ---- ABI 層のエラー（負の値）。SHIORI のステータスとは別物 ----
pub const SHIORI_E_INVALID_ARG: ShioriStatus = -1;
pub const SHIORI_E_UNSUPPORTED: ShioriStatus = -2;
/// ホストはこのインスタンスを unload すべき
pub const SHIORI_E_PANIC: ShioriStatus = -3;
pub const SHIORI_E_REENTRANT: ShioriStatus = -4;
pub const SHIORI_E_SHUTDOWN: ShioriStatus = -5;
pub const SHIORI_E_INVALID_UTF8: ShioriStatus = -6;

// ---- ログレベル ----
pub const SHIORI_LOG_ERROR: i32 = 1;
pub const SHIORI_LOG_WARN: i32 = 2;
pub const SHIORI_LOG_INFO: i32 = 3;
pub const SHIORI_LOG_DEBUG: i32 = 4;

/// 200 番台なら成功
pub const fn shiori_is_success(s: ShioriStatus) -> bool {
    200 <= s && s < 300
}

/// 負の値なら ABI 層のエラー
pub const fn shiori_is_abi_error(s: ShioriStatus) -> bool {
    s < 0
}

// ============================================================
// 所有権付きバイト列
// ============================================================

/// 境界を越えるバイト列
///
/// 不変条件: len > 0 なら、ptr は release が呼ばれるまで len バイト読める。
/// この不変条件を守るため、フィールドは非公開にしている
/// （#[repr(C)] のレイアウトはフィールドの可視性に影響されない）。
///
/// ShioriBytes 自体は Drop を持たない。受け取った値は ShioriOwnedBytes で包んで扱うこと。
#[repr(C)]
pub struct ShioriBytes {
    ptr: *const u8,
    len: usize,
    /// 確保した側だけが解釈する不透明なポインタ
    owner: *mut c_void,
    /// 確保した側の解放関数。None なら解放は不要
    release: Option<unsafe extern "C" fn(bytes: *mut ShioriBytes)>,
}

impl ShioriBytes {
    /// 空の値。out 引数の初期値に使う
    pub const fn empty() -> Self {
        Self {
            ptr: ptr::null(),
            len: 0,
            owner: ptr::null_mut(),
            release: None,
        }
    }

    /// Vec をコピーせずに渡す（容量ごと owner に保持する）
    pub fn from_vec(v: Vec<u8>) -> Self {
        // Vec をムーブしてもヒープ上のバッファの位置は変わらない
        let ptr = v.as_ptr();
        let len = v.len();
        let owner = Box::into_raw(Box::new(v));
        Self {
            ptr,
            len,
            owner: owner.cast(),
            release: Some(release_vec),
        }
    }

    /// String をコピーせずに渡す（into_bytes は O(1)）
    pub fn from_string(s: String) -> Self {
        Self::from_vec(s.into_bytes())
    }

    /// 借用している &str はコピーしてから渡す
    pub fn copy_from_str(s: &str) -> Self {
        Self::from_vec(s.as_bytes().to_vec())
    }

    /// 借用している &[u8] はコピーしてから渡す
    pub fn copy_from_slice(b: &[u8]) -> Self {
        Self::from_vec(b.to_vec())
    }

    /// Rust 以外で確保したバッファや静的なバッファを包む
    ///
    /// # Safety
    /// - len > 0 なら、ptr は release が呼ばれるまで len バイト読めること
    ///   （release が None なら、受け取った側が使い終わるまで読めること）
    /// - release は、この値の owner を正しく解放できる関数であること
    pub const unsafe fn from_raw_parts(
        ptr: *const u8,
        len: usize,
        owner: *mut c_void,
        release: Option<unsafe extern "C" fn(bytes: *mut ShioriBytes)>,
    ) -> Self {
        Self { ptr, len, owner, release }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// コピーせずに借用する
    pub fn as_slice(&self) -> &[u8] {
        if self.len == 0 || self.ptr.is_null() {
            &[]
        } else {
            // SAFETY: 型の不変条件により、ptr は len バイト読める
            unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
        }
    }

    /// コピーせずに借用する（UTF-8 として検証する。O(n)）
    pub fn as_str(&self) -> Result<&str, Utf8Error> {
        std::str::from_utf8(self.as_slice())
    }

    /// 不正なバイトは U+FFFD に置き換える。UTF-8 として正しければコピーしない
    pub fn to_str_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(self.as_slice())
    }

    /// 検証せずに &str として借用する
    ///
    /// # Safety
    /// 中身が UTF-8 として正しいことを呼び出し側が保証すること
    pub unsafe fn as_str_unchecked(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(self.as_slice()) }
    }

    /// 自分のアロケータへコピーする
    pub fn to_vec(&self) -> Vec<u8> {
        self.as_slice().to_vec()
    }
}

impl Default for ShioriBytes {
    fn default() -> Self {
        Self::empty()
    }
}

impl fmt::Debug for ShioriBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShioriBytes")
            .field("len", &self.len)
            .field("has_release", &self.release.is_some())
            .finish_non_exhaustive()
    }
}

/// from_vec で確保したバッファの解放関数
unsafe extern "C" fn release_vec(b: *mut ShioriBytes) {
    if b.is_null() {
        return;
    }
    unsafe {
        let b = &mut *b;
        if !b.owner.is_null() {
            drop(Box::from_raw(b.owner as *mut Vec<u8>));
        }
        *b = ShioriBytes::empty(); // 二重解放を防ぐ
    }
}

impl From<Vec<u8>> for ShioriBytes {
    fn from(v: Vec<u8>) -> Self {
        Self::from_vec(v)
    }
}

impl From<String> for ShioriBytes {
    fn from(s: String) -> Self {
        Self::from_string(s)
    }
}

impl From<&str> for ShioriBytes {
    fn from(s: &str) -> Self {
        Self::copy_from_str(s)
    }
}

impl From<&[u8]> for ShioriBytes {
    fn from(b: &[u8]) -> Self {
        Self::copy_from_slice(b)
    }
}

// ============================================================
// 受け取った ShioriBytes の RAII ラッパ
// ============================================================

/// 受け取った ShioriBytes を、Drop で相手側の release に返す
///
/// 参照系のメソッド（as_slice / as_str / to_str_lossy など）は Deref を通して
/// ShioriBytes のものが使える。返る &str の寿命は ShioriOwnedBytes に縛られる。
pub struct ShioriOwnedBytes(ShioriBytes);

impl ShioriOwnedBytes {
    /// # Safety
    /// ABI 呼び出しで受け取ったばかりで、まだ解放されていないこと
    /// （自分で生成してまだ誰にも渡していない ShioriBytes も可）
    pub unsafe fn from_raw(b: ShioriBytes) -> Self {
        Self(b)
    }

    /// 自分のアロケータへコピーして String にする
    /// 相手のバッファは Drop で解放される（相手のバッファを引き取ってはいけない）
    pub fn into_string(self) -> Result<String, FromUtf8Error> {
        String::from_utf8(self.0.to_vec())
    }

    /// 自分のアロケータへコピーして Vec にする
    pub fn into_vec(self) -> Vec<u8> {
        self.0.to_vec()
    }
}

impl Deref for ShioriOwnedBytes {
    type Target = ShioriBytes;
    fn deref(&self) -> &ShioriBytes {
        &self.0
    }
}

impl Drop for ShioriOwnedBytes {
    fn drop(&mut self) {
        if let Some(f) = self.0.release {
            unsafe { f(&mut self.0) }
        }
    }
}

impl fmt::Debug for ShioriOwnedBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ShioriOwnedBytes").field(&self.0).finish()
    }
}

// ============================================================
// 応答（Rust 側でだけ使い、境界は越えない）
// ============================================================

/// SHIORI やホストの実装者が返す応答
/// body の形式は版による（v4: ヘッダ部分だけ / v3: SHIORI/3.0 の応答全文）
#[derive(Debug)]
pub struct ShioriResponse {
    pub status: ShioriStatus,
    pub body: ShioriBytes,
}

impl ShioriResponse {
    pub fn ok(body: impl Into<ShioriBytes>) -> Self {
        Self { status: SHIORI_200_OK, body: body.into() }
    }

    pub fn no_content() -> Self {
        Self { status: SHIORI_204_NO_CONTENT, body: ShioriBytes::empty() }
    }

    pub fn bad_request() -> Self {
        Self { status: SHIORI_400_BAD_REQUEST, body: ShioriBytes::empty() }
    }

    pub fn error() -> Self {
        Self { status: SHIORI_500_INTERNAL_SERVER_ERROR, body: ShioriBytes::empty() }
    }

    pub fn with_status(status: ShioriStatus, body: impl Into<ShioriBytes>) -> Self {
        Self { status, body: body.into() }
    }

    /// 応答を分解し、body を RAII ラッパに包んで返す（body を読んでから捨てる場合に使う）
    pub fn into_owned(self) -> ShioriOwnedResponse {
        // SAFETY: body はまだ誰にも渡していない
        ShioriOwnedResponse { status: self.status, body: unsafe { ShioriOwnedBytes::from_raw(self.body) } }
    }
}

/// 呼び出した側が受け取る応答
#[derive(Debug)]
pub struct ShioriOwnedResponse {
    /// SHIORI/3.0 のステータスコード（0 以上）
    pub status: ShioriStatus,
    pub body: ShioriOwnedBytes,
}

impl ShioriOwnedResponse {
    pub fn is_success(&self) -> bool {
        shiori_is_success(self.status)
    }
}

// ============================================================
// 引数 (ptr, len) の受け取り
// ============================================================

/// # Safety
/// p は len バイト読めること（len == 0 なら null でもよい）。
/// 返る参照は、その FFI 呼び出しの間だけ有効
pub unsafe fn shiori_slice_arg<'a>(p: *const u8, len: usize) -> Result<&'a [u8], ShioriStatus> {
    match (p.is_null(), len) {
        (_, 0) => Ok(&[]),
        (true, _) => Err(SHIORI_E_INVALID_ARG),
        (false, _) => Ok(unsafe { std::slice::from_raw_parts(p, len) }),
    }
}

/// # Safety
/// shiori_slice_arg と同じ
pub unsafe fn shiori_str_arg<'a>(p: *const u8, len: usize) -> Result<&'a str, ShioriStatus> {
    let b = unsafe { shiori_slice_arg(p, len)? };
    std::str::from_utf8(b).map_err(|_| SHIORI_E_INVALID_UTF8)
}

// ============================================================
// 内部：パニックを境界で止める
// ============================================================

/// Ok はステータスコード、Err は ABI 層のエラー（負の値）
fn guard(f: impl FnOnce() -> Result<ShioriStatus, ShioriStatus>) -> ShioriStatus {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(s)) | Ok(Err(s)) => s,
        Err(_) => SHIORI_E_PANIC,
    }
}

// ============================================================
// ShioriHostApi（ホスト → SHIORI に渡すテーブル。v4 のみ）
// ============================================================

#[repr(C)]
pub struct ShioriHostApi {
    struct_size: u32,
    abi_version: u32,
    ctx: *mut c_void,
    /// 同期リクエスト。SHIORI のスレッドから、load / request / notify の処理中に限り呼べる
    request: unsafe extern "C" fn(
        ctx: *mut c_void,
        req: *const u8,
        len: usize,
        out: *mut ShioriBytes,
    ) -> ShioriStatus,
    /// 非同期通知。任意のスレッドから呼べる。msg は呼び出し中だけ有効
    post: unsafe extern "C" fn(ctx: *mut c_void, msg: *const u8, len: usize) -> ShioriStatus,
    log: unsafe extern "C" fn(ctx: *mut c_void, level: i32, msg: *const u8, len: usize),
    // 将来のフィールドはここに Option<unsafe extern "C" fn(..)> で追加する
}

impl ShioriHostApi {
    /// 基本フィールドの末尾（フィールドを追加しても変わらない）
    pub const BASE_SIZE: usize = offset_of!(Self, log) + size_of::<usize>();

    pub fn struct_size(&self) -> u32 {
        self.struct_size
    }

    pub fn abi_version(&self) -> u32 {
        self.abi_version
    }

    /// ホストのテーブルは v4 以上でのみ存在する
    pub fn is_valid(&self) -> bool {
        self.abi_version >= SHIORI_ABI_VERSION && self.struct_size as usize >= Self::BASE_SIZE
    }
}

/// ホストの実装者が実装する trait（ABI 層）
/// post は任意のスレッドから &self で呼ばれるので Sync を要求する（規約 4）
///
/// 通常は上位クレート（shiori4_host）の Host trait を実装する
pub trait ShioriHost: Sync + 'static {
    /// SHIORI からの同期リクエスト（SHIORI のスレッドから呼ばれる）
    /// この中から SHIORI を呼び返してはならない（SHIORI_E_REENTRANT を返す）
    fn request(&self, req: &[u8]) -> ShioriResponse;

    /// 非同期通知。msg は呼び出しの間だけ有効なので、コピーしてからキューに積むこと
    /// 受け付けたら 204、不正なら 400、終了中なら SHIORI_E_SHUTDOWN を返す
    fn post(&self, msg: &[u8]) -> ShioriStatus;

    fn log(&self, _level: i32, _msg: &str) {}
}

/// ShioriHostApi と ShioriHost の実体をまとめてヒープに固定する
/// SHIORI はこのアドレスを保持するので、ShioriInstance より長く生かす（借用で強制される）
pub struct ShioriHostBox<H: ShioriHost> {
    raw: NonNull<HostBoxInner<H>>,
}

struct HostBoxInner<H> {
    api: ShioriHostApi,
    host: H,
}

impl<H: ShioriHost> ShioriHostBox<H> {
    pub fn new(host: H) -> Self {
        let raw = Box::into_raw(Box::new(HostBoxInner {
            api: ShioriHostApi {
                struct_size: size_of::<ShioriHostApi>() as u32,
                abi_version: SHIORI_ABI_VERSION,
                ctx: ptr::null_mut(),
                request: host_request::<H>,
                post: host_post::<H>,
                log: host_log::<H>,
            },
            host,
        }));
        // ctx は ShioriHost の実体を指す（Box::into_raw 後なのでアドレスは動かない）
        unsafe { (*raw).api.ctx = ptr::addr_of_mut!((*raw).host).cast() };
        // SAFETY: Box::into_raw の戻り値は null にならない
        Self { raw: unsafe { NonNull::new_unchecked(raw) } }
    }

    pub fn host(&self) -> &H {
        unsafe { &(*self.raw.as_ptr()).host }
    }

    fn api_ptr(&self) -> NonNull<ShioriHostApi> {
        // SAFETY: raw は有効なので、そのフィールドへのポインタも null にならない
        unsafe { NonNull::new_unchecked(ptr::addr_of_mut!((*self.raw.as_ptr()).api)) }
    }
}

impl<H: ShioriHost> Drop for ShioriHostBox<H> {
    fn drop(&mut self) {
        unsafe { drop(Box::from_raw(self.raw.as_ptr())) }
    }
}

unsafe extern "C" fn host_request<H: ShioriHost>(
    ctx: *mut c_void,
    req: *const u8,
    len: usize,
    out: *mut ShioriBytes,
) -> ShioriStatus {
    guard(|| {
        if ctx.is_null() || out.is_null() {
            return Err(SHIORI_E_INVALID_ARG);
        }
        let host = unsafe { &*(ctx as *const H) };
        let req = unsafe { shiori_slice_arg(req, len)? };
        let ShioriResponse { status, body } = host.request(req);
        unsafe { out.write(body) }; // 最後に書く。パニック時の out は空のまま
        Ok(status)
    })
}

unsafe extern "C" fn host_post<H: ShioriHost>(
    ctx: *mut c_void,
    msg: *const u8,
    len: usize,
) -> ShioriStatus {
    guard(|| {
        if ctx.is_null() {
            return Err(SHIORI_E_INVALID_ARG);
        }
        let host = unsafe { &*(ctx as *const H) };
        let msg = unsafe { shiori_slice_arg(msg, len)? };
        Ok(host.post(msg))
    })
}

unsafe extern "C" fn host_log<H: ShioriHost>(
    ctx: *mut c_void,
    level: i32,
    msg: *const u8,
    len: usize,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if ctx.is_null() {
            return;
        }
        let host = unsafe { &*(ctx as *const H) };
        let msg = unsafe { shiori_slice_arg(msg, len) }.unwrap_or(&[]);
        host.log(level, &String::from_utf8_lossy(msg));
    }));
}

// ============================================================
// ShioriApi（SHIORI → ホスト に返すテーブル）
// ============================================================

#[repr(C)]
pub struct ShioriApi {
    struct_size: u32,
    /// shiori_get_api で要求された版と同じ値
    abi_version: u32,
    /// インスタンスを生成する。失敗したら null
    /// host は v4 では必須、v3 では None。dir は UTF-8 のゴーストディレクトリ
    load: unsafe extern "C" fn(
        host: Option<NonNull<ShioriHostApi>>,
        dir: *const u8,
        dir_len: usize,
    ) -> *mut c_void,
    unload: unsafe extern "C" fn(inst: *mut c_void),
    /// GET に相当する（v3 では NOTIFY 全文も受け付ける）。戻り値はステータスコード
    request: unsafe extern "C" fn(
        inst: *mut c_void,
        req: *const u8,
        len: usize,
        out: *mut ShioriBytes,
    ) -> ShioriStatus,
    /// NOTIFY に相当する。本文は返さない
    notify: unsafe extern "C" fn(inst: *mut c_void, req: *const u8, len: usize) -> ShioriStatus,
    // 将来のフィールドはここに Option<unsafe extern "C" fn(..)> で追加する
}

/// load 時に渡される動作モード
pub enum ShioriLoadMode {
    /// v4：ペイロードはヘッダ部分だけ。ホストを呼べる
    V4(ShioriHostHandle),
    /// v3：ペイロードは SHIORI/3.0 の全文。ホストは使えない
    V3,
}

impl ShioriLoadMode {
    pub fn abi_version(&self) -> u32 {
        match self {
            Self::V4(_) => SHIORI_ABI_VERSION,
            Self::V3 => SHIORI_ABI_VERSION_3,
        }
    }
}

/// ABI 層の trait。req と応答は、モードに応じた形式のまま受け渡す（このクレートでは解釈しない）
/// unload は Drop で表す。Drop では、ShioriHostPoster を渡した別スレッドを止めて join すること
///
/// 通常は上位クレート（shiori4）のアダプタを通して実装する
pub trait Shiori: Sized + 'static {
    /// v3 のテーブルも返すか。true にするなら v3 の全文形式を自分で扱うこと
    const SUPPORTS_V3: bool = false;

    fn load(mode: ShioriLoadMode, dir: &str) -> Result<Self, ShioriStatus>;

    fn request(&mut self, req: &str) -> ShioriResponse;

    fn notify(&mut self, _req: &str) -> ShioriStatus {
        SHIORI_204_NO_CONTENT
    }
}

impl ShioriApi {
    /// 基本フィールドの末尾（フィールドを追加しても変わらない）
    pub const BASE_SIZE: usize = offset_of!(Self, notify) + size_of::<usize>();

    /// trait 実装からテーブルを生成する（安全）。V は SHIORI_ABI_VERSION か SHIORI_ABI_VERSION_3
    pub const fn new<S: Shiori, const V: u32>() -> Self {
        Self {
            struct_size: size_of::<Self>() as u32,
            abi_version: V,
            load: shiori_load::<S, V>,
            unload: shiori_unload::<S>,
            request: shiori_request::<S>,
            notify: shiori_notify::<S>,
        }
    }

    /// DLL から受け取ったテーブルを検証して参照にする
    ///
    /// # Safety
    /// p は shiori_get_api(requested) の戻り値であり、そのライブラリが 'lib の間ロードされていること。
    /// また、相手の実装がこの ABI 規約を守っていること
    pub unsafe fn from_raw<'lib>(
        p: *const ShioriApi,
        requested: u32,
    ) -> Result<&'lib Self, ShioriStatus> {
        if p.is_null() {
            return Err(SHIORI_E_UNSUPPORTED);
        }
        // 参照を作る前に struct_size を生ポインタ経由で確認する
        let size = unsafe { ptr::addr_of!((*p).struct_size).read() } as usize;
        if size < Self::BASE_SIZE {
            return Err(SHIORI_E_UNSUPPORTED);
        }
        let api = unsafe { &*p };
        // 要求した版と違うテーブルが返ってきたら拒否する
        if api.is_valid() && api.abi_version == requested {
            Ok(api)
        } else {
            Err(SHIORI_E_UNSUPPORTED)
        }
    }

    pub fn struct_size(&self) -> u32 {
        self.struct_size
    }

    pub fn abi_version(&self) -> u32 {
        self.abi_version
    }

    pub fn is_valid(&self) -> bool {
        self.abi_version >= SHIORI_ABI_VERSION_MIN && self.struct_size as usize >= Self::BASE_SIZE
    }

    /// v4 モードでインスタンスを生成する。返るインスタンスは self と host の両方を借用する
    pub fn load<'a, H: ShioriHost>(
        &'a self,
        host: &'a ShioriHostBox<H>,
        dir: &str,
    ) -> Option<ShioriInstance<'a>> {
        if self.abi_version < SHIORI_ABI_VERSION {
            return None;
        }
        let p = unsafe { (self.load)(Some(host.api_ptr()), dir.as_ptr(), dir.len()) };
        NonNull::new(p).map(|inst| ShioriInstance { api: self, inst })
    }

    /// v3 モードでインスタンスを生成する（host なし）
    pub fn load_v3<'a>(&'a self, dir: &str) -> Option<ShioriInstance<'a>> {
        if self.abi_version != SHIORI_ABI_VERSION_3 {
            return None;
        }
        let p = unsafe { (self.load)(None, dir.as_ptr(), dir.len()) };
        NonNull::new(p).map(|inst| ShioriInstance { api: self, inst })
    }
}

unsafe extern "C" fn shiori_load<S: Shiori, const V: u32>(
    host: Option<NonNull<ShioriHostApi>>,
    dir: *const u8,
    dir_len: usize,
) -> *mut c_void {
    let r = catch_unwind(AssertUnwindSafe(|| -> Result<*mut c_void, ShioriStatus> {
        let mode = if V >= SHIORI_ABI_VERSION {
            let api: &ShioriHostApi = unsafe { host.ok_or(SHIORI_E_INVALID_ARG)?.as_ref() };
            if !api.is_valid() {
                return Err(SHIORI_E_UNSUPPORTED);
            }
            ShioriLoadMode::V4(unsafe { ShioriHostHandle::new(api) })
        } else {
            ShioriLoadMode::V3 // host が渡されても無視する
        };
        let dir = unsafe { shiori_str_arg(dir, dir_len)? };
        let s = S::load(mode, dir)?;
        Ok(Box::into_raw(Box::new(s)).cast())
    }));
    match r {
        Ok(Ok(p)) => p,
        _ => ptr::null_mut(),
    }
}

unsafe extern "C" fn shiori_unload<S: Shiori>(inst: *mut c_void) {
    if inst.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        drop(Box::from_raw(inst as *mut S));
    }));
}

unsafe extern "C" fn shiori_request<S: Shiori>(
    inst: *mut c_void,
    req: *const u8,
    len: usize,
    out: *mut ShioriBytes,
) -> ShioriStatus {
    guard(|| {
        if inst.is_null() || out.is_null() {
            return Err(SHIORI_E_INVALID_ARG);
        }
        let me = unsafe { &mut *(inst as *mut S) };
        let req = unsafe { shiori_str_arg(req, len)? };
        let ShioriResponse { status, body } = me.request(req);
        unsafe { out.write(body) }; // 最後に書く。パニック時の out は空のまま
        Ok(status)
    })
}

unsafe extern "C" fn shiori_notify<S: Shiori>(
    inst: *mut c_void,
    req: *const u8,
    len: usize,
) -> ShioriStatus {
    guard(|| {
        if inst.is_null() {
            return Err(SHIORI_E_INVALID_ARG);
        }
        let me = unsafe { &mut *(inst as *mut S) };
        let req = unsafe { shiori_str_arg(req, len)? };
        Ok(me.notify(req))
    })
}

/// エクスポート関数 `shiori_get_api` の型
pub type ShioriGetApiFn =
    unsafe extern "C" fn(requested_abi_version: u32) -> *const ShioriApi;

/// エクスポート関数 `shiori_get_api` を生成する
///
/// ```ignore
/// shiori4_abi::export_shiori!(MyShiori);
/// ```
#[macro_export]
macro_rules! export_shiori {
    ($t:ty) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn shiori_get_api(requested: u32) -> *const $crate::ShioriApi {
            static API_V4: $crate::ShioriApi =
                $crate::ShioriApi::new::<$t, { $crate::SHIORI_ABI_VERSION }>();
            static API_V3: $crate::ShioriApi =
                $crate::ShioriApi::new::<$t, { $crate::SHIORI_ABI_VERSION_3 }>();
            match requested {
                $crate::SHIORI_ABI_VERSION => &API_V4,
                $crate::SHIORI_ABI_VERSION_3 if <$t as $crate::Shiori>::SUPPORTS_V3 => &API_V3,
                _ => ::core::ptr::null(),
            }
        }
    };
}

// ============================================================
// ホスト側：SHIORI インスタンス（安全なラッパ）
// ============================================================

/// NonNull を含むので !Send かつ !Sync。規約 1（単一スレッド）を型で強制する
/// Drop で unload を呼ぶ
pub struct ShioriInstance<'a> {
    api: &'a ShioriApi,
    inst: NonNull<c_void>,
}

impl ShioriInstance<'_> {
    pub fn abi_version(&self) -> u32 {
        self.api.abi_version
    }

    /// Err になるのは ABI 層のエラーだけ。400 や 500 は Ok(ShioriOwnedResponse) で返る
    /// v3 では body に SHIORI/3.0 の応答全文が入る
    pub fn request(&mut self, req: &str) -> Result<ShioriOwnedResponse, ShioriStatus> {
        let mut out = ShioriBytes::empty();
        let st = unsafe {
            (self.api.request)(self.inst.as_ptr(), req.as_ptr(), req.len(), &mut out)
        };
        let body = unsafe { ShioriOwnedBytes::from_raw(out) }; // どの場合でも確実に解放する
        if shiori_is_abi_error(st) {
            Err(st)
        } else {
            Ok(ShioriOwnedResponse { status: st, body })
        }
    }

    /// Err になるのは ABI 層のエラーだけ
    pub fn notify(&mut self, req: &str) -> Result<ShioriStatus, ShioriStatus> {
        let st = unsafe { (self.api.notify)(self.inst.as_ptr(), req.as_ptr(), req.len()) };
        if shiori_is_abi_error(st) {
            Err(st)
        } else {
            Ok(st)
        }
    }
}

impl Drop for ShioriInstance<'_> {
    fn drop(&mut self) {
        unsafe { (self.api.unload)(self.inst.as_ptr()) }
    }
}

// ============================================================
// SHIORI 側：ホストハンドル（安全なラッパ）
// ============================================================

/// 同期 request 用。生成はクレート内（shiori_load）だけ
/// NonNull を含むので !Send かつ !Sync。複製しても有効期間は同じ（unload まで）
#[derive(Clone)]
pub struct ShioriHostHandle {
    api: NonNull<ShioriHostApi>,
}

/// post 専用。任意のスレッドへ持ち出せる
/// unload の後は使ってはならない（Shiori の Drop で、持ち出したスレッドを join すること）
#[derive(Clone, Copy)]
pub struct ShioriHostPoster {
    ctx: *mut c_void,
    post: unsafe extern "C" fn(*mut c_void, *const u8, usize) -> ShioriStatus,
}

// SAFETY: ABI 規約 4 により、post は任意のスレッドから呼べる
unsafe impl Send for ShioriHostPoster {}
unsafe impl Sync for ShioriHostPoster {}

impl ShioriHostHandle {
    /// # Safety
    /// api は is_valid() を満たし、unload が返るまで有効であること
    pub(crate) unsafe fn new(api: &ShioriHostApi) -> Self {
        Self { api: NonNull::from(api) }
    }

    fn api(&self) -> &ShioriHostApi {
        // SAFETY: new の前提条件による
        unsafe { self.api.as_ref() }
    }

    /// Err になるのは ABI 層のエラーだけ。400 や 500 は Ok(ShioriOwnedResponse) で返る
    pub fn request(&self, req: &[u8]) -> Result<ShioriOwnedResponse, ShioriStatus> {
        let api = self.api();
        let mut out = ShioriBytes::empty();
        let st = unsafe { (api.request)(api.ctx, req.as_ptr(), req.len(), &mut out) };
        let body = unsafe { ShioriOwnedBytes::from_raw(out) }; // どの場合でも確実に解放する
        if shiori_is_abi_error(st) {
            Err(st)
        } else {
            Ok(ShioriOwnedResponse { status: st, body })
        }
    }

    pub fn request_str(&self, req: &str) -> Result<ShioriOwnedResponse, ShioriStatus> {
        self.request(req.as_bytes())
    }

    pub fn log(&self, level: i32, msg: &str) {
        let api = self.api();
        unsafe { (api.log)(api.ctx, level, msg.as_ptr(), msg.len()) }
    }

    pub fn poster(&self) -> ShioriHostPoster {
        let api = self.api();
        ShioriHostPoster { ctx: api.ctx, post: api.post }
    }
}

impl ShioriHostPoster {
    /// 204: 受付、400: 不正、SHIORI_E_SHUTDOWN: 終了中
    pub fn post(&self, msg: &[u8]) -> ShioriStatus {
        unsafe { (self.post)(self.ctx, msg.as_ptr(), msg.len()) }
    }

    pub fn post_str(&self, msg: &str) -> ShioriStatus {
        self.post(msg.as_bytes())
    }
}

// ============================================================
// レイアウトをコンパイル時に検査する（C 版ヘッダとの一致確認）
// ============================================================

const _: () = {
    let w = size_of::<usize>();

    // ShioriBytes: ptr, len, owner, release
    assert!(offset_of!(ShioriBytes, ptr) == 0);
    assert!(offset_of!(ShioriBytes, len) == w);
    assert!(offset_of!(ShioriBytes, owner) == 2 * w);
    assert!(offset_of!(ShioriBytes, release) == 3 * w);
    assert!(size_of::<ShioriBytes>() == 4 * w);

    // Option<fn> / Option<NonNull<T>> は C の NULL 許容ポインタと同じサイズ
    assert!(size_of::<Option<unsafe extern "C" fn(*mut ShioriBytes)>>() == w);
    assert!(size_of::<Option<NonNull<ShioriHostApi>>>() == w);

    // ShioriHostApi: u32, u32, ctx, request, post, log
    assert!(offset_of!(ShioriHostApi, ctx) == 8);
    assert!(offset_of!(ShioriHostApi, request) == 8 + w);
    assert!(offset_of!(ShioriHostApi, post) == 8 + 2 * w);
    assert!(offset_of!(ShioriHostApi, log) == 8 + 3 * w);
    assert!(size_of::<ShioriHostApi>() == 8 + 4 * w);

    // ShioriApi: u32, u32, load, unload, request, notify
    assert!(offset_of!(ShioriApi, load) == 8);
    assert!(offset_of!(ShioriApi, unload) == 8 + w);
    assert!(offset_of!(ShioriApi, request) == 8 + 2 * w);
    assert!(offset_of!(ShioriApi, notify) == 8 + 3 * w);
    assert!(size_of::<ShioriApi>() == 8 + 4 * w);
};
