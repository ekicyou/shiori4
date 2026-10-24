//! shiori4_abi — SHIORI 次世代 ABI 定義（ホスト・SHIORI 共通）
//!
//! このクレートは「バイト列をどう運ぶか」だけを扱い、ペイロードの中身は解釈しない。
//! SHIORI/3.0 の解析・生成、v3/v4 の変換、プロパティ管理は上位クレート（shiori4）の役割。
//!
//! # 版（abi_version）
//! - SHIORI_ABI_VERSION（4）: 既定。host 必須。ペイロードはヘッダ部分だけ（開始行なし）
//! - SHIORI_ABI_VERSION_3（3）: SHIORI3 互換モード。異なる OS の互換ベースウェア向け
//!   - load の host は copy_from_slice だけを持つテーブル（abi_version は 3。request・post・log は NULL）。
//!     SHIORI は copy_from_slice があるかだけを確かめる。ほかのホストの呼び出しは使えない
//!   - request の req は SHIORI/3.0 の要求全文（"GET SHIORI/3.0" / "NOTIFY SHIORI/3.0" で始まる）
//!   - request の out は SHIORI/3.0 の応答全文（"SHIORI/3.0 200 OK" で始まり空行で終わる）
//!   - 戻り値は開始行と同じステータスコード（正本は文字列）。負の値は ABI エラー（out は空。500 として扱う）
//!   - notify の req は "NOTIFY SHIORI/3.0" で始まる全文。GET なら 400
//!   - 文字コードは UTF-8 固定（Charset: UTF-8）。SHIORI/2.x の要求には 400 を返す
//!   - v3 は v4 と同じ機能を出すことを保証しない
//! - v4 の文字コードは UTF-8 固定。ホストは要求に Charset ヘッダを付けなくてよい。付いていても
//!   SHIORI は無視する。SHIORI は応答に Charset を付けない
//! - ホストは shiori_get_api(v) で版を選ぶ。返るテーブルの abi_version は v と一致する
//!
//! # ABI 規約
//! - shiori4 の書き出しは shiori_get_api の 1 つだけ。対応できない版なら NULL を返す
//!   （SHIORI/3.0 の書き出しと並べてよい。「DLL の口と発見の順」を参照）
//! - DLL 境界を越えるのは C のレイアウトの構造体と関数ポインタだけ
//! - ペイロードは UTF-8 のバイト列（ptr と len）。NUL 終端にはしない
//! - 引数の (ptr, len) は呼び出しの間だけ有効な借用。保持する場合は受け取った側がコピーする
//! - 所有権が移るのは out に書き込まれる ShioriBytes だけ。out の ShioriBytes は常にホストが作り、
//!   ホストの release で解放する（SHIORI は応答を host.copy_from_slice で写して作る）。
//!   写せなかったときは、ホストが自分で決めた印を out に残し（SHIORI には知らせない）、
//!   SHIORI は out をそのまま返す
//! - 呼び出された側は out を最後に書く。呼び出した側は戻り値に関係なく out を解放する
//! - 構造体は末尾にだけフィールドを追加する（追加する関数ポインタは NULL 許容）。
//!   受け取った側は struct_size を見て、フィールドがあるかを判定する
//! - エクスポートする関数は、例外・パニックを境界の外へ出さず SHIORI_E_PANIC を返す
//!
//! # 戻り値（ShioriStatus）
//! - 0 以上: SHIORI/3.0 のステータスコード（200, 204, 400, 500 など）
//! - 負の値: ABI 層のエラー（SHIORI_E_*）。out は空。SHIORI_E_PANIC ならホストはインスタンスを unload する
//!
//! # スレッド規約
//! 1. ShioriApi の load / request / notify / unload は、ホストが決めた単一のスレッドからだけ呼ぶ。
//!    このスレッドはインスタンスごとに決まり、プロセスで 1 つではない。
//!    同じ DLL の 2 つのインスタンスは、別々のスレッドから呼ばれうる
//! 2. SHIORI が host.request を呼べるのは、load / request / notify を呼ばれているスレッドから、
//!    その呼び出しが返るまでの間だけ（v4）。
//!    ホストは、これに反する呼び出しを見分けたら SHIORI_E_WRONG_THREAD を返してよい（out は空）。
//!    判定はインスタンスごとに行う
//! 3. host.request の処理中に、ホストから SHIORI を呼び返してはならない（SHIORI_E_REENTRANT）
//! 4. host.post と host.log は任意のスレッドから呼べる。ホストは msg をコピーしてすぐ返る
//!    （post はキューに積む）
//! 5. ShioriHostApi は unload が返るまで有効。SHIORI は、unload が返った後に ShioriHostApi の
//!    どの関数も呼ばない（例外は ShioriBytes.release だけ。規約 6）。
//!    unload を始めたホストは post に SHIORI_E_SHUTDOWN を返してよい。
//!    unload は長く止まらないこと。ホストは OS の終了の猶予の中で、各インスタンスの unload を
//!    呼ぶことがある
//! 6. ShioriBytes.release は常にホストの関数で、いつでも・どのスレッドから呼んでもよい
//!    （インスタンスの unload の後でもよい）。host.copy_from_slice も、どのスレッドから呼んでもよい。
//!    ホストはそれに耐えること
//! 7. notify を即 204 で返し、処理を後で行う実装は合法である。後回しにした処理の中では
//!    host.request を使えない（規約 2 の期間の外であるため）。ホストへの依頼には post を使う
//!
//! # load の規約
//! - dir は、v4・v3 互換モードとも、絶対パス・OS の区切り・末尾に区切りあり・UTF-8 の文字列
//!   （呼び出し中だけ有効）
//! - カレントディレクトリはホスト次第である。SHIORI はカレントディレクトリに頼らず、load の dir を使う
//! - v4 の load が失敗するときは、NULL を返す前に host.log で理由を出す。v3 は host に log が無いので、
//!   SHIORI 自身のログに出す。v3 の load は失敗の理由を返さない（NULL を返すだけ）
//! - SHIORI は同じプロセスでの 2 つ目のインスタンスを拒否してよい（NULL ＋ log）。拒否されたホストは、
//!   そのゴーストを読めなかったものとして扱い、ゴーストごとに別のプロセスで読むか、拒否を受け入れる
//! - SHIORI は、できれば複数のインスタンスを受けるべきである（状態をインスタンスごとに持ち、
//!   グローバルな状態を避ける）
//!
//! # ホストのプロセス
//! - ホストは SHIORI を専用のプロセスで読むことがある。SHIORI は、ホストと同じプロセスにいること
//!   （ホストの窓・プロセスの状態を共有すること）を前提にしない
//! - host.request は IPC を往復することがあり、遅くなりうる
//!
//! # DLL の口と発見の順
//! - 1 つの DLL が、SHIORI/3.0 の書き出し（load・request・unload）と shiori_get_api の両方を持ってよい
//! - ホストは書き出しを shiori_get_api → loadu → load の順に探し、最初に見つかった口で解決する。
//!   shiori_get_api が見つかったら 4、だめなら 3 を試す。どちらも NULL なら読めなかったものとして扱い、
//!   loadu・load へは進まない。解決した口の load が失敗しても、ほかの口へは進まない
//!
//! # post の完了通知（v4）
//! - 受付 ID は SHIORI が発行し、Tag ヘッダとしてメッセージに入れる（登録してから post する）
//! - post の戻り値は受付可否だけ（204: 受付、400: 不正、SHIORI_E_SHUTDOWN: 終了中）
//! - 完了はホストが通常のイベント（request または notify）として、post したインスタンスに届ける
//!   - post の呼び出し中や host.request の処理中には届けない（イベントループから届ける）
//!   - unload を始めたら、そのインスタンス宛ての未配送イベントは破棄する
//! - Tag を付けない post は「結果は不要」を意味する
//! - 運ぶ中身の語（Command・Tag・OnHostComplete の並び・拒否の返し方など）は、shiori4 リポジトリの doc/host-protocol.md（ホスト側プロトコル仕様）が定める
//!
//! # 配布の決まり（DLL の選び方）
//! - descript.txt の shiori,<名前> から名前の幹を取る（例 shiori,pasta.dll → pasta）
//! - ホストは、ホストの OS と arch ごとに次の順で探す
//!   - Windows x64: <幹>.x64.dll → <幹>.dll
//!   - Windows arm64: <幹>.arm64.dll →（x64 のプロセスを起こせるホストだけ）<幹>.x64.dll → <幹>.dll
//!   - Linux x64: lib<幹>.x64.so → lib<幹>.so
//!   - Linux arm64: lib<幹>.arm64.so → lib<幹>.so
//!   - macOS: lib<幹>.dylib（ユニバーサルバイナリを推奨）
//! - 接尾辞は x64・arm64 の 2 つだけ。x86 は作らない。接尾辞なしの Windows の DLL が、
//!   32bit の SHIORI/3.0 を担う
//! - Linux と macOS は lib の付いた名前だけを探す。macOS は arch の接尾辞を持たない
//! - ホストは選んだファイルの機械の種類をヘッダ（PE・ELF・Mach-O）で確かめ、合わなければ次の候補へ
//!   進み、候補が尽きたらそのゴーストを読めなかったものとして扱う。これは SHIORI/3.0 と v4 の両方に使える

use std::{
    borrow::Cow,
    ffi::c_void,
    fmt,
    mem::{offset_of, size_of},
    ops::Deref,
    panic::{AssertUnwindSafe, catch_unwind},
    ptr::{self, NonNull},
    str::Utf8Error,
    string::FromUtf8Error,
    sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError},
};

// ============================================================
// 定数
// ============================================================

/// 既定の版
pub const SHIORI_ABI_VERSION: u32 = 4;
/// SHIORI3 互換モード（host は copy_from_slice だけを持つ。ペイロードは SHIORI/3.0 の全文）
pub const SHIORI_ABI_VERSION_3: u32 = 3;
/// この ABI 定義が扱う最小の版
pub const SHIORI_ABI_VERSION_MIN: u32 = 3;

/// shiori4 の唯一のエクスポートシンボル名（ABI 規約）（NUL 終端。libloading にそのまま渡せる）
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
/// 規約 2 に反する host.request の呼び出し（out は空）
pub const SHIORI_E_WRONG_THREAD: ShioriStatus = -7;

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
        Self {
            ptr,
            len,
            owner,
            release,
        }
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

// SAFETY: スレッド規約 6 により、release はどのスレッドから呼んでもよい。
// Sync は付けない（共有して読む用途は要件に無い）。
// ShioriOwnedResponse は status と ShioriOwnedBytes だけなので自動で Send になる。
unsafe impl Send for ShioriOwnedBytes {}

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
        Self {
            status: SHIORI_200_OK,
            body: body.into(),
        }
    }

    pub fn no_content() -> Self {
        Self {
            status: SHIORI_204_NO_CONTENT,
            body: ShioriBytes::empty(),
        }
    }

    pub fn bad_request() -> Self {
        Self {
            status: SHIORI_400_BAD_REQUEST,
            body: ShioriBytes::empty(),
        }
    }

    pub fn error() -> Self {
        Self {
            status: SHIORI_500_INTERNAL_SERVER_ERROR,
            body: ShioriBytes::empty(),
        }
    }

    pub fn with_status(status: ShioriStatus, body: impl Into<ShioriBytes>) -> Self {
        Self {
            status,
            body: body.into(),
        }
    }

    /// 応答を分解し、body を RAII ラッパに包んで返す（body を読んでから捨てる場合に使う）
    pub fn into_owned(self) -> ShioriOwnedResponse {
        // SAFETY: body はまだ誰にも渡していない
        ShioriOwnedResponse {
            status: self.status,
            body: unsafe { ShioriOwnedBytes::from_raw(self.body) },
        }
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
// ShioriHostApi（ホスト → SHIORI に渡すテーブル。v3 では copy_from_slice だけ）
// ============================================================

#[repr(C)]
pub struct ShioriHostApi {
    struct_size: u32,
    abi_version: u32,
    ctx: *mut c_void,
    /// 同期リクエスト。load / request / notify を呼ばれているスレッドから、その呼び出しが
    /// 返るまでの間だけ呼べる（スレッド規約 2）。外れた呼び出しには SHIORI_E_WRONG_THREAD が返りうる。
    /// v3 のテーブルでは None
    request: Option<
        unsafe extern "C" fn(
            ctx: *mut c_void,
            req: *const u8,
            len: usize,
            out: *mut ShioriBytes,
        ) -> ShioriStatus,
    >,
    /// 非同期通知。任意のスレッドから呼べる（スレッド規約 4）。msg は呼び出し中だけ有効。
    /// v3 のテーブルでは None
    post:
        Option<unsafe extern "C" fn(ctx: *mut c_void, msg: *const u8, len: usize) -> ShioriStatus>,
    /// 任意のスレッドから呼べる（スレッド規約 4）。msg は呼び出し中だけ有効。v3 のテーブルでは None
    log: Option<unsafe extern "C" fn(ctx: *mut c_void, level: i32, msg: *const u8, len: usize)>,
    /// SHIORI の応答を作る。ptr から len バイトをホストのメモリへ写し、ホストが作った ShioriBytes を
    /// out に入れる（ABI 規約）。任意のスレッドから呼べる（スレッド規約 6）。v3 のテーブルにもある。
    /// 戻り値は無い。写せなかったときは、ホストが自分で決めた印を out に残す（SHIORI には知らせない）
    copy_from_slice: Option<
        unsafe extern "C" fn(ctx: *mut c_void, ptr: *const u8, len: usize, out: *mut ShioriBytes),
    >,
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

    /// v4 のテーブルの検査（v3 のテーブルは copy_from_slice があるかだけを確かめる）
    pub fn is_valid(&self) -> bool {
        self.abi_version >= SHIORI_ABI_VERSION && self.struct_size as usize >= Self::BASE_SIZE
    }

    /// copy_from_slice の欄があれば ctx と一緒に返す（C の SHIORI_HAS_FIELD に当たる）
    ///
    /// # Safety
    /// p は struct_size が読めるテーブルを指すこと。欄を読むのは struct_size を確かめてから
    unsafe fn copy_from_slice_of(p: NonNull<Self>) -> Option<(*mut c_void, CopyFromSliceFn)> {
        let p = p.as_ptr();
        let size = unsafe { ptr::addr_of!((*p).struct_size).read() } as usize;
        if size < offset_of!(Self, copy_from_slice) + size_of::<usize>() {
            return None;
        }
        let f = unsafe { ptr::addr_of!((*p).copy_from_slice).read() }?;
        Some((unsafe { ptr::addr_of!((*p).ctx).read() }, f))
    }
}

type CopyFromSliceFn =
    unsafe extern "C" fn(ctx: *mut c_void, ptr: *const u8, len: usize, out: *mut ShioriBytes);

/// ホストの copy_from_slice（ShioriHostBox と load_v3 のテーブルが使う）。
/// ctx は見ないので、ShioriHostBox を捨てた後に release しても安全（スレッド規約 6）
unsafe extern "C" fn host_copy_from_slice(
    _ctx: *mut c_void,
    ptr: *const u8,
    len: usize,
    out: *mut ShioriBytes,
) {
    if out.is_null() {
        return;
    }
    // 写せなかったときの印は空の ShioriBytes（パニックも境界の外へ出さない）
    let b = catch_unwind(AssertUnwindSafe(|| {
        unsafe { shiori_slice_arg(ptr, len) }.map(ShioriBytes::copy_from_slice)
    }));
    unsafe { out.write(b.ok().and_then(Result::ok).unwrap_or_default()) }
}

/// load_v3 が渡す copy_from_slice だけのテーブル
struct V3HostApi(ShioriHostApi);

// SAFETY: ctx は null で、中身は関数ポインタだけ。SHIORI は読むだけ
unsafe impl Sync for V3HostApi {}

static V3_HOST: V3HostApi = V3HostApi(ShioriHostApi {
    struct_size: size_of::<ShioriHostApi>() as u32,
    abi_version: SHIORI_ABI_VERSION_3,
    ctx: ptr::null_mut(),
    request: None,
    post: None,
    log: None,
    copy_from_slice: Some(host_copy_from_slice),
});

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
                request: Some(host_request::<H>),
                post: Some(host_post::<H>),
                log: Some(host_log::<H>),
                copy_from_slice: Some(host_copy_from_slice),
            },
            host,
        }));
        // ctx は ShioriHost の実体を指す（Box::into_raw 後なのでアドレスは動かない）
        unsafe { (*raw).api.ctx = ptr::addr_of_mut!((*raw).host).cast() };
        // SAFETY: Box::into_raw の戻り値は null にならない
        Self {
            raw: unsafe { NonNull::new_unchecked(raw) },
        }
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
    /// インスタンスを生成する。失敗したら null（v4 は先に host.log で理由を出す。load の規約）
    /// host は v4 では必須、v3 では copy_from_slice だけを持つテーブル。dir は絶対パス・OS の区切り・末尾に区切りあり・UTF-8 の
    /// ゴーストディレクトリ（呼び出し中だけ有効。load の規約）
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
/// unload は Drop で表す。unload を始めた時点で、そのインスタンスの ShioriHostPoster は無効になり
/// post は SHIORI_E_SHUTDOWN を返す（log は何もしない）ので、持ち出したスレッドを join しなくてよい。
/// Drop の中で使えるホストの口は ShioriHostHandle::log だけ（request は SHIORI_E_WRONG_THREAD になりうる）
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

    /// v3 モードでインスタンスを生成する（host は copy_from_slice だけのテーブル）
    pub fn load_v3<'a>(&'a self, dir: &str) -> Option<ShioriInstance<'a>> {
        if self.abi_version != SHIORI_ABI_VERSION_3 {
            return None;
        }
        let host = NonNull::from(&V3_HOST.0);
        let p = unsafe { (self.load)(Some(host), dir.as_ptr(), dir.len()) };
        NonNull::new(p).map(|inst| ShioriInstance { api: self, inst })
    }
}

/// shiori_load が返すインスタンスの実体。v4 は門を持ち、v3 は持たない
/// 応答はこの層がホストの copy_from_slice で写す（S はテーブルを見ない）
struct Instance<S> {
    gate: Option<Arc<PosterGate>>,
    ctx: *mut c_void,
    copy_from_slice: CopyFromSliceFn,
    shiori: S,
}

unsafe extern "C" fn shiori_load<S: Shiori, const V: u32>(
    host: Option<NonNull<ShioriHostApi>>,
    dir: *const u8,
    dir_len: usize,
) -> *mut c_void {
    // 失敗の 1 行を出せる host（v4 で有効なものだけ）
    let log_host = match host {
        Some(h) if V >= SHIORI_ABI_VERSION => Some(unsafe { h.as_ref() }).filter(|a| a.is_valid()),
        _ => None,
    };
    let r = catch_unwind(AssertUnwindSafe(|| -> Result<*mut c_void, ShioriStatus> {
        let host = host.ok_or(SHIORI_E_INVALID_ARG)?;
        // 応答を作る口が無いホストでは load を失敗させる（自分のメモリで作る形へ戻らない）
        let (ctx, copy_from_slice) =
            unsafe { ShioriHostApi::copy_from_slice_of(host) }.ok_or(SHIORI_E_UNSUPPORTED)?;
        let (mode, gate) = if V >= SHIORI_ABI_VERSION {
            let api: &ShioriHostApi = unsafe { host.as_ref() };
            if !api.is_valid() {
                return Err(SHIORI_E_UNSUPPORTED);
            }
            let gate = Arc::new(PosterGate::default());
            (
                ShioriLoadMode::V4(unsafe { ShioriHostHandle::new(api, gate.clone()) }),
                Some(gate),
            )
        } else {
            (ShioriLoadMode::V3, None) // v3 の S にはホストを見せない（copy_from_slice はこの層が使う）
        };
        let dir = unsafe { shiori_str_arg(dir, dir_len)? };
        let shiori = S::load(mode, dir)?;
        let inst = Instance {
            gate,
            ctx,
            copy_from_slice,
            shiori,
        };
        Ok(Box::into_raw(Box::new(inst)).cast())
    }));
    let msg = match r {
        Ok(Ok(p)) => return p,
        Ok(Err(st)) => format!("shiori load failed: status {st}"),
        Err(_) => format!("shiori load failed: panic (status {SHIORI_E_PANIC})"),
    };
    if let Some((api, log)) = log_host.and_then(|a| Some((a, a.log?))) {
        // ホストの log のパニックも境界の外へ出さない
        let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
            log(api.ctx, SHIORI_LOG_ERROR, msg.as_ptr(), msg.len())
        }));
    }
    ptr::null_mut()
}

unsafe extern "C" fn shiori_unload<S: Shiori>(inst: *mut c_void) {
    if inst.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let inst = unsafe { Box::from_raw(inst as *mut Instance<S>) };
        // 門を閉じて実行中の post・log を待ってから S を Drop する（S の Drop の中の post も止まる）
        let gate = inst.gate.clone();
        if let Some(g) = &gate {
            g.close_and_wait();
        }
        drop(inst);
        // S の中から外へ持ち出された ShioriHostHandle も、ここから先はホストに触れない
        if let Some(g) = &gate {
            g.lock().unloaded = true;
        }
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
        let inst = unsafe { &mut *(inst as *mut Instance<S>) };
        let req = unsafe { shiori_str_arg(req, len)? };
        let ShioriResponse { status, body } = inst.shiori.request(req);
        let body = unsafe { ShioriOwnedBytes::from_raw(body) }; // 写したら自分の側で解放する
        let b = body.as_slice();
        // 最後に書く（ホストが作る）。パニック時の out は空のまま
        unsafe { (inst.copy_from_slice)(inst.ctx, b.as_ptr(), b.len(), out) };
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
        let me = unsafe { &mut (*(inst as *mut Instance<S>)).shiori };
        let req = unsafe { shiori_str_arg(req, len)? };
        Ok(me.notify(req))
    })
}

/// エクスポート関数 `shiori_get_api` の型
pub type ShioriGetApiFn = unsafe extern "C" fn(requested_abi_version: u32) -> *const ShioriApi;

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
        let st =
            unsafe { (self.api.request)(self.inst.as_ptr(), req.as_ptr(), req.len(), &mut out) };
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
/// NonNull を含むので !Send かつ !Sync。複製しても有効期間は同じ（unload まで）。
/// unload が返った後に残っていても、request は SHIORI_E_SHUTDOWN を返し、log は何もしない
/// unload（Shiori の Drop）の中で使えるのは log だけ。unload は load / request / notify の
/// 期間に入らないので、request は SHIORI_E_WRONG_THREAD になりうる（規約 2）
#[derive(Clone)]
pub struct ShioriHostHandle {
    api: NonNull<ShioriHostApi>,
    gate: Arc<PosterGate>,
}

/// post と log 用。任意のスレッドへ持ち出せる（複製は .clone() で書く）
/// unload を始めた時点で無効になり、post は SHIORI_E_SHUTDOWN を返し、log は何もしない。
/// そのため unload の後も動き続けるスレッドから呼んでも安全で、join は要らない
#[derive(Clone)]
pub struct ShioriHostPoster {
    ctx: *mut c_void,
    post: unsafe extern "C" fn(*mut c_void, *const u8, usize) -> ShioriStatus,
    log: unsafe extern "C" fn(*mut c_void, i32, *const u8, usize),
    gate: Arc<PosterGate>,
}

/// unload を始めたら閉じ、それまでに通った post・log が返るのを待つ門（インスタンスごとに 1 つ）
#[derive(Default)]
struct PosterGate {
    state: Mutex<GateState>,
    idle: Condvar,
}

#[derive(Default)]
struct GateState {
    /// 一度 true になったら戻らない
    closed: bool,
    /// 門を通ってホストの関数を呼んでいる最中の数
    in_flight: usize,
    /// unload が S を Drop し終えた。これより後、ShioriHostHandle はホストの表に触れない
    unloaded: bool,
}

/// Drop で in_flight を 1 つ減らす
struct GateTicket<'a>(&'a PosterGate);

impl PosterGate {
    fn lock(&self) -> MutexGuard<'_, GateState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 閉じていれば None
    fn enter(&self) -> Option<GateTicket<'_>> {
        let mut st = self.lock();
        if st.closed {
            return None;
        }
        st.in_flight += 1;
        Some(GateTicket(self))
    }

    /// shiori_unload からだけ呼ぶ。先に閉じるので、待ちの間に新しい post は入らない
    fn close_and_wait(&self) {
        let mut st = self.lock();
        st.closed = true;
        while st.in_flight > 0 {
            st = self.idle.wait(st).unwrap_or_else(PoisonError::into_inner);
        }
    }

    fn is_unloaded(&self) -> bool {
        self.lock().unloaded
    }
}

impl Drop for GateTicket<'_> {
    fn drop(&mut self) {
        let mut st = self.0.lock();
        st.in_flight -= 1;
        if st.in_flight == 0 {
            self.0.idle.notify_all();
        }
    }
}

// SAFETY: スレッド規約 4 により、post は任意のスレッドから呼べる
unsafe impl Send for ShioriHostPoster {}
unsafe impl Sync for ShioriHostPoster {}

impl ShioriHostHandle {
    /// # Safety
    /// api は is_valid() を満たし、unload が返るまで有効であること
    pub(crate) unsafe fn new(api: &ShioriHostApi, gate: Arc<PosterGate>) -> Self {
        Self {
            api: NonNull::from(api),
            gate,
        }
    }

    fn api(&self) -> &ShioriHostApi {
        // SAFETY: new の前提条件による
        unsafe { self.api.as_ref() }
    }

    /// Err になるのは ABI 層のエラーだけ。400 や 500 は Ok(ShioriOwnedResponse) で返る
    /// unload が返った後は SHIORI_E_SHUTDOWN（ホストに触れない）
    pub fn request(&self, req: &[u8]) -> Result<ShioriOwnedResponse, ShioriStatus> {
        if self.gate.is_unloaded() {
            return Err(SHIORI_E_SHUTDOWN);
        }
        let api = self.api();
        let request = api.request.ok_or(SHIORI_E_UNSUPPORTED)?;
        let mut out = ShioriBytes::empty();
        let st = unsafe { request(api.ctx, req.as_ptr(), req.len(), &mut out) };
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

    /// unload が返った後は何もしない
    pub fn log(&self, level: i32, msg: &str) {
        if self.gate.is_unloaded() {
            return;
        }
        let api = self.api();
        if let Some(log) = api.log {
            unsafe { log(api.ctx, level, msg.as_ptr(), msg.len()) }
        }
    }

    pub fn poster(&self) -> ShioriHostPoster {
        // 門が閉じた後と、テーブルに post・log が無いときに使う
        unsafe extern "C" fn dead_post(_: *mut c_void, _: *const u8, _: usize) -> ShioriStatus {
            SHIORI_E_SHUTDOWN
        }
        unsafe extern "C" fn dead_log(_: *mut c_void, _: i32, _: *const u8, _: usize) {}
        if self.gate.is_unloaded() {
            // 門は閉じているので、この Poster は ctx にも関数にも触れない
            return ShioriHostPoster {
                ctx: std::ptr::null_mut(),
                post: dead_post,
                log: dead_log,
                gate: self.gate.clone(),
            };
        }
        let api = self.api();
        ShioriHostPoster {
            ctx: api.ctx,
            post: api.post.unwrap_or(dead_post),
            log: api.log.unwrap_or(dead_log),
            gate: self.gate.clone(),
        }
    }
}

impl ShioriHostPoster {
    /// 204: 受付、400: 不正、SHIORI_E_SHUTDOWN: 終了中（unload を始めた後を含む）
    pub fn post(&self, msg: &[u8]) -> ShioriStatus {
        // 門を見てから ctx に触れる（閉じた後の ctx はホストが解放しているかもしれない）
        let Some(_ticket) = self.gate.enter() else {
            return SHIORI_E_SHUTDOWN;
        };
        unsafe { (self.post)(self.ctx, msg.as_ptr(), msg.len()) }
    }

    pub fn post_str(&self, msg: &str) -> ShioriStatus {
        self.post(msg.as_bytes())
    }

    /// 任意のスレッドから呼べる（規約 4）。unload を始めた後は何もしない
    pub fn log(&self, level: i32, msg: &str) {
        let Some(_ticket) = self.gate.enter() else {
            return;
        };
        unsafe { (self.log)(self.ctx, level, msg.as_ptr(), msg.len()) }
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

    // ShioriHostApi: u32, u32, ctx, request, post, log, copy_from_slice
    assert!(offset_of!(ShioriHostApi, ctx) == 8);
    assert!(offset_of!(ShioriHostApi, request) == 8 + w);
    assert!(offset_of!(ShioriHostApi, post) == 8 + 2 * w);
    assert!(offset_of!(ShioriHostApi, log) == 8 + 3 * w);
    assert!(offset_of!(ShioriHostApi, copy_from_slice) == 8 + 4 * w);
    assert!(size_of::<ShioriHostApi>() == 8 + 5 * w);

    // ShioriApi: u32, u32, load, unload, request, notify
    assert!(offset_of!(ShioriApi, load) == 8);
    assert!(offset_of!(ShioriApi, unload) == 8 + w);
    assert!(offset_of!(ShioriApi, request) == 8 + 2 * w);
    assert!(offset_of!(ShioriApi, notify) == 8 + 3 * w);
    assert!(size_of::<ShioriApi>() == 8 + 4 * w);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_keep_their_values() {
        assert_eq!(SHIORI_E_INVALID_ARG, -1);
        assert_eq!(SHIORI_E_UNSUPPORTED, -2);
        assert_eq!(SHIORI_E_PANIC, -3);
        assert_eq!(SHIORI_E_REENTRANT, -4);
        assert_eq!(SHIORI_E_SHUTDOWN, -5);
        assert_eq!(SHIORI_E_INVALID_UTF8, -6);
        assert_eq!(SHIORI_E_WRONG_THREAD, -7);
    }

    #[test]
    fn owned_bytes_can_be_released_on_another_thread() {
        fn assert_send<T: Send>() {}
        assert_send::<ShioriOwnedBytes>();
        assert_send::<ShioriOwnedResponse>();

        // 自分が確保したバイト列（release_vec）を別スレッドへ送り、そこで解放する
        let owned = unsafe { ShioriOwnedBytes::from_raw(ShioriBytes::copy_from_str("hello")) };
        let resp = ShioriOwnedResponse {
            status: SHIORI_200_OK,
            body: owned,
        };
        let got = std::thread::spawn(move || {
            let s = resp.body.as_str().map(str::to_owned);
            drop(resp);
            s
        })
        .join()
        .unwrap();
        assert_eq!(got.as_deref(), Ok("hello"));
    }
    // ---- unload と Poster の門 ----

    use std::cell::RefCell;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::Duration;

    thread_local! {
        // load したスレッドで Poster を受け取る（テストは別々のスレッドで並ぶので混ざらない）
        static LOADED: RefCell<Option<ShioriHostPoster>> = const { RefCell::new(None) };
        static POST_IN_DROP: RefCell<Option<ShioriStatus>> = const { RefCell::new(None) };
    }

    struct GateShiori {
        poster: ShioriHostPoster,
    }

    impl Shiori for GateShiori {
        fn load(mode: ShioriLoadMode, _dir: &str) -> Result<Self, ShioriStatus> {
            let ShioriLoadMode::V4(h) = mode else {
                return Err(SHIORI_E_UNSUPPORTED);
            };
            let poster = h.poster();
            LOADED.with(|l| *l.borrow_mut() = Some(poster.clone()));
            Ok(Self { poster })
        }
        fn request(&mut self, _req: &str) -> ShioriResponse {
            ShioriResponse::no_content()
        }
    }

    impl Drop for GateShiori {
        fn drop(&mut self) {
            let st = self.poster.post_str("in drop");
            POST_IN_DROP.with(|d| *d.borrow_mut() = Some(st));
        }
    }

    static GATE_API: ShioriApi = ShioriApi::new::<GateShiori, { SHIORI_ABI_VERSION }>();

    #[derive(Default)]
    struct Counters {
        posts: AtomicUsize,
        logs: AtomicUsize,
        block: AtomicBool,
        entered: AtomicBool,
        release: AtomicBool,
        post_returned: AtomicBool,
    }

    struct GateHost(Arc<Counters>);

    impl ShioriHost for GateHost {
        fn request(&self, _req: &[u8]) -> ShioriResponse {
            ShioriResponse::no_content()
        }
        fn post(&self, _msg: &[u8]) -> ShioriStatus {
            let c = &self.0;
            c.posts.fetch_add(1, Ordering::SeqCst);
            // 止めるのは最初の 1 回だけ（Drop の中の post などは止めない）
            if c.block.swap(false, Ordering::SeqCst) {
                c.entered.store(true, Ordering::SeqCst);
                while !c.release.load(Ordering::SeqCst) {
                    std::thread::yield_now();
                }
                c.post_returned.store(true, Ordering::SeqCst);
            }
            SHIORI_204_NO_CONTENT
        }
        fn log(&self, _level: i32, _msg: &str) {
            self.0.logs.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn load_gate(host: &ShioriHostBox<GateHost>) -> (ShioriInstance<'_>, ShioriHostPoster) {
        let inst = GATE_API.load(host, "/ghost").expect("load");
        let poster = LOADED.with(|l| l.borrow_mut().take()).expect("poster");
        (inst, poster)
    }

    #[test]
    fn post_after_unload_returns_shutdown_without_touching_host() {
        let c = Arc::new(Counters::default());
        let host = ShioriHostBox::new(GateHost(c.clone()));
        let (inst, poster) = load_gate(&host);
        assert_eq!(poster.post_str("before"), SHIORI_204_NO_CONTENT);
        assert_eq!(c.posts.load(Ordering::SeqCst), 1);
        drop(inst);
        // Shiori の Drop の中の post も門で止まる
        assert_eq!(
            POST_IN_DROP.with(|d| d.borrow_mut().take()),
            Some(SHIORI_E_SHUTDOWN)
        );
        assert_eq!(poster.post_str("after"), SHIORI_E_SHUTDOWN);
        assert_eq!(poster.clone().post(b"after"), SHIORI_E_SHUTDOWN);
        assert_eq!(c.posts.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn unload_waits_for_post_in_flight() {
        let c = Arc::new(Counters::default());
        c.block.store(true, Ordering::SeqCst);
        let host = ShioriHostBox::new(GateHost(c.clone()));
        let (inst, poster) = load_gate(&host);
        std::thread::scope(|s| {
            let t = s.spawn(move || poster.post_str("slow"));
            while !c.entered.load(Ordering::SeqCst) {
                std::thread::yield_now();
            }
            let c2 = c.clone();
            s.spawn(move || {
                std::thread::sleep(Duration::from_millis(100));
                c2.release.store(true, Ordering::SeqCst);
            });
            drop(inst); // unload
            // unload が返った時点で、unload の前に入った post はホストから返っている
            assert!(c.post_returned.load(Ordering::SeqCst));
            assert_eq!(t.join().unwrap(), SHIORI_204_NO_CONTENT);
        });
    }

    #[test]
    fn thread_outliving_unload_is_safe() {
        let c = Arc::new(Counters::default());
        let host = ShioriHostBox::new(GateHost(c.clone()));
        let (inst, poster) = load_gate(&host);
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        // join しないスレッド（unload の後も post・log を呼び続ける）
        let t = std::thread::spawn(move || {
            while !stop2.load(Ordering::SeqCst) {
                poster.post_str("tick");
                poster.log(0, "tick");
                std::thread::yield_now();
            }
            // stop は unload の後に立つので、この post は必ず unload の後に走る
            let last = poster.post_str("tick");
            poster.log(0, "tick");
            last
        });
        std::thread::sleep(Duration::from_millis(20));
        drop(inst);
        let posts = c.posts.load(Ordering::SeqCst);
        let logs = c.logs.load(Ordering::SeqCst);
        drop(host); // ctx を解放した後も、スレッドは ctx に触れない
        std::thread::sleep(Duration::from_millis(20));
        stop.store(true, Ordering::SeqCst);
        assert_eq!(t.join().unwrap(), SHIORI_E_SHUTDOWN);
        assert_eq!(c.posts.load(Ordering::SeqCst), posts);
        assert_eq!(c.logs.load(Ordering::SeqCst), logs);
    }

    #[test]
    fn poster_log_does_not_reach_host_after_unload() {
        let c = Arc::new(Counters::default());
        let host = ShioriHostBox::new(GateHost(c.clone()));
        let (inst, poster) = load_gate(&host);
        std::thread::scope(|s| {
            s.spawn(|| poster.log(0, "from worker"));
        });
        assert_eq!(c.logs.load(Ordering::SeqCst), 1);
        drop(inst);
        poster.log(0, "after unload");
        assert_eq!(c.logs.load(Ordering::SeqCst), 1);
    }

    thread_local! {
        static HANDLE: RefCell<Option<ShioriHostHandle>> = const { RefCell::new(None) };
    }

    struct HandleShiori;

    impl Shiori for HandleShiori {
        fn load(mode: ShioriLoadMode, _dir: &str) -> Result<Self, ShioriStatus> {
            let ShioriLoadMode::V4(h) = mode else {
                return Err(SHIORI_E_UNSUPPORTED);
            };
            HANDLE.with(|s| *s.borrow_mut() = Some(h.clone()));
            Ok(Self)
        }
        fn request(&mut self, _req: &str) -> ShioriResponse {
            ShioriResponse::no_content()
        }
    }

    impl Drop for HandleShiori {
        fn drop(&mut self) {
            // unload の中ではまだ log が届く
            HANDLE.with(|s| s.borrow().as_ref().unwrap().log(0, "in drop"));
        }
    }

    static HANDLE_API: ShioriApi = ShioriApi::new::<HandleShiori, { SHIORI_ABI_VERSION }>();

    #[test]
    fn handle_kept_past_unload_does_not_touch_host() {
        let c = Arc::new(Counters::default());
        let host = ShioriHostBox::new(GateHost(c.clone()));
        let inst = HANDLE_API.load(&host, "/ghost").expect("load");
        drop(inst);
        assert_eq!(c.logs.load(Ordering::SeqCst), 1); // Drop の中の 1 行
        let h = HANDLE.with(|s| s.borrow_mut().take()).expect("handle");
        drop(host); // ホストの表を解放した後も、h はそれに触れない
        h.log(0, "after unload");
        assert_eq!(h.request_str("GET\r\n").err(), Some(SHIORI_E_SHUTDOWN));
        assert_eq!(h.poster().post_str("after unload"), SHIORI_E_SHUTDOWN);
        assert_eq!(c.logs.load(Ordering::SeqCst), 1);
        assert_eq!(c.posts.load(Ordering::SeqCst), 0);
    }

    // ---- load の失敗の 1 行 ----

    thread_local! {
        static LOGGED: RefCell<Vec<(i32, String)>> = const { RefCell::new(Vec::new()) };
    }

    struct FailShiori;

    impl Shiori for FailShiori {
        const SUPPORTS_V3: bool = true;
        fn load(_mode: ShioriLoadMode, dir: &str) -> Result<Self, ShioriStatus> {
            if dir.contains("panic") {
                panic!("load panic");
            }
            Err(SHIORI_E_INVALID_ARG)
        }
        fn request(&mut self, _req: &str) -> ShioriResponse {
            ShioriResponse::no_content()
        }
    }

    struct LogHost;

    impl ShioriHost for LogHost {
        fn request(&self, _req: &[u8]) -> ShioriResponse {
            ShioriResponse::no_content()
        }
        fn post(&self, _msg: &[u8]) -> ShioriStatus {
            SHIORI_204_NO_CONTENT
        }
        fn log(&self, level: i32, msg: &str) {
            LOGGED.with(|l| l.borrow_mut().push((level, msg.to_owned())));
        }
    }

    static FAIL_V4: ShioriApi = ShioriApi::new::<FailShiori, { SHIORI_ABI_VERSION }>();
    static FAIL_V3: ShioriApi = ShioriApi::new::<FailShiori, { SHIORI_ABI_VERSION_3 }>();

    fn take_logged() -> Vec<(i32, String)> {
        LOGGED.with(|l| std::mem::take(&mut *l.borrow_mut()))
    }

    #[test]
    fn v4_load_failure_logs_one_line() {
        let host = ShioriHostBox::new(LogHost);
        assert!(FAIL_V4.load(&host, "/ghost").is_none());
        assert_eq!(
            take_logged(),
            vec![(SHIORI_LOG_ERROR, "shiori load failed: status -1".to_owned())]
        );
        assert!(FAIL_V4.load(&host, "/panic").is_none());
        assert_eq!(
            take_logged(),
            vec![(
                SHIORI_LOG_ERROR,
                "shiori load failed: panic (status -3)".to_owned()
            )]
        );
        // dir の検査の失敗（UTF-8 でない）も 1 行
        let p = unsafe { (FAIL_V4.load)(Some(host.api_ptr()), [0xffu8].as_ptr(), 1) };
        assert!(p.is_null());
        assert_eq!(take_logged().len(), 1);
    }

    #[test]
    fn v3_load_failure_does_not_log() {
        let host = ShioriHostBox::new(LogHost);
        assert!(FAIL_V3.load_v3("/ghost").is_none());
        // v3 のテーブルに host を渡されても log しない
        let p = unsafe { (FAIL_V3.load)(Some(host.api_ptr()), "/panic".as_ptr(), 6) };
        assert!(p.is_null());
        assert!(take_logged().is_empty());
    }

    // ---- 応答はホストの copy_from_slice で作る ----

    struct CopyShiori;

    impl Shiori for CopyShiori {
        const SUPPORTS_V3: bool = true;
        fn load(_mode: ShioriLoadMode, _dir: &str) -> Result<Self, ShioriStatus> {
            Ok(Self)
        }
        fn request(&mut self, req: &str) -> ShioriResponse {
            ShioriResponse::ok(format!("Value: {req}"))
        }
    }

    static COPY_V4: ShioriApi = ShioriApi::new::<CopyShiori, { SHIORI_ABI_VERSION }>();
    static COPY_V3: ShioriApi = ShioriApi::new::<CopyShiori, { SHIORI_ABI_VERSION_3 }>();

    /// 生のテーブルで load する（host を自分で組んだテーブルにする）
    fn load_raw<'a>(
        api: &'a ShioriApi,
        host: Option<&'a ShioriHostApi>,
    ) -> Option<ShioriInstance<'a>> {
        let p = unsafe { (api.load)(host.map(NonNull::from), "/ghost".as_ptr(), 6) };
        NonNull::new(p).map(|inst| ShioriInstance { api, inst })
    }

    /// ShioriHostBox のテーブルの写し（ctx は host の実体を指したまま）
    fn table_of<H: ShioriHost>(host: &ShioriHostBox<H>) -> ShioriHostApi {
        // SAFETY: ShioriHostApi は Drop を持たないので、ビットの写しでよい
        unsafe { host.api_ptr().as_ptr().read() }
    }

    #[test]
    fn load_is_null_without_copy_from_slice() {
        let host = ShioriHostBox::new(LogHost);
        // v4: 欄の手前で終わる（古いホストの）テーブル。失敗の 1 行は出る
        let mut old = table_of(&host);
        old.struct_size = offset_of!(ShioriHostApi, copy_from_slice) as u32;
        assert!(load_raw(&COPY_V4, Some(&old)).is_none());
        assert_eq!(take_logged().len(), 1);
        // v4: 欄はあるが NULL
        let mut none = table_of(&host);
        none.copy_from_slice = None;
        assert!(load_raw(&COPY_V4, Some(&none)).is_none());
        assert_eq!(take_logged().len(), 1);
        // v3: host が NULL・欄の無いテーブル
        assert!(load_raw(&COPY_V3, None).is_none());
        old.abi_version = SHIORI_ABI_VERSION_3;
        assert!(load_raw(&COPY_V3, Some(&old)).is_none());
        assert!(take_logged().is_empty());
        // 欄があれば読める
        assert!(COPY_V4.load(&host, "/ghost").is_some());
        assert!(COPY_V3.load_v3("/ghost").is_some());
    }

    #[derive(Default)]
    struct CopyCounts {
        copies: AtomicUsize,
        releases: AtomicUsize,
    }

    /// ホストの copy_from_slice（ctx は CopyCounts。owner に ctx を入れ、release で数える）
    unsafe extern "C" fn counting_copy(
        ctx: *mut c_void,
        ptr: *const u8,
        len: usize,
        out: *mut ShioriBytes,
    ) {
        let c = unsafe { &*(ctx as *const CopyCounts) };
        c.copies.fetch_add(1, Ordering::SeqCst);
        let buf: Box<[u8]> = unsafe { shiori_slice_arg(ptr, len) }.unwrap().into();
        let buf = Box::into_raw(buf);
        let b = unsafe {
            ShioriBytes::from_raw_parts(buf as *const u8, len, ctx, Some(counting_release))
        };
        unsafe { out.write(b) }
    }

    unsafe extern "C" fn counting_release(b: *mut ShioriBytes) {
        let b = unsafe { &mut *b };
        let c = unsafe { &*(b.owner as *const CopyCounts) };
        c.releases.fetch_add(1, Ordering::SeqCst);
        drop(unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(b.ptr as *mut u8, b.len)) });
        *b = ShioriBytes::empty();
    }

    fn counting_table(c: &CopyCounts, abi_version: u32) -> ShioriHostApi {
        ShioriHostApi {
            struct_size: size_of::<ShioriHostApi>() as u32,
            abi_version,
            ctx: c as *const CopyCounts as *mut c_void,
            request: None,
            post: None,
            log: None,
            copy_from_slice: Some(counting_copy),
        }
    }

    #[test]
    fn response_is_allocated_and_released_only_by_host_functions() {
        for (api, v) in [
            (&COPY_V4, SHIORI_ABI_VERSION),
            (&COPY_V3, SHIORI_ABI_VERSION_3),
        ] {
            let c = CopyCounts::default();
            let t = counting_table(&c, v);
            let mut inst = load_raw(api, Some(&t)).expect("load");
            let r = inst.request("x").expect("abi");
            assert_eq!(r.body.as_str(), Ok("Value: x"));
            assert_eq!(c.copies.load(Ordering::SeqCst), 1);
            assert_eq!(c.releases.load(Ordering::SeqCst), 0);
            drop(inst);
            drop(r); // unload の後に捨ててもホストの release で解放される
            assert_eq!(c.releases.load(Ordering::SeqCst), 1);
        }
    }
}
