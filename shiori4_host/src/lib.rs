//! shiori4_host — shiori4 のホスト（ベースウェア）側の高水準層
//!
//! `Host` trait を実装すれば、SHIORI（v4）を読み込んで動かすホストが作れる。
//!
//! - SHIORI の DLL / .so / .dylib を読み込み、ライフタイムを一括管理する（`GhostRuntime`）
//! - SHIORI からの同期リクエスト（プロパティ取得など）を `Host` に振り分ける
//! - 任意のスレッドから来る post をキューに積み、イベントループで `Host::on_post` に渡す
//! - 非同期処理の完了や任意のイベントを、規約どおりのタイミングで SHIORI に届ける（`pump`）
//! - unload 時に未配送のイベントを破棄し、以後の post を SHIORI_E_SHUTDOWN にする
//!
//! # スレッドモデル
//! `GhostRuntime` は `!Send`。作ったスレッド（UI スレッドなど）で request / notify / pump を呼ぶ。
//! SHIORI が別スレッドから post すると `Host::wake` が呼ばれるので、そこでイベントループに
//! 「pump を呼んで」と依頼する（Windows なら DispatcherQueue.TryEnqueue、など）。
//!
//! ```no_run
//! use shiori4_host::*;
//!
//! struct MyHost;
//!
//! impl Host for MyHost {
//!     fn property(&self, name: &str) -> Option<String> {
//!         match name {
//!             "baseware.name" => Some("MyBase".into()),
//!             _ => None,
//!         }
//!     }
//!     fn on_post(&self, post: Post) {
//!         // 重い処理は別スレッドへ。Completer は Send なので持ち出せる
//!         let done = post.into_completer();
//!         std::thread::spawn(move || {
//!             done.complete(Delivery::Request, SHIORI_200_OK, &["result"]);
//!         });
//!     }
//!     fn wake(&self) { /* UI スレッドに pump を依頼する */ }
//! }
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // dir は絶対パスで渡す（相対パスは LoadError::InvalidPath）。ここでは実行ファイルの隣から組み立てる
//! let exe = std::env::current_exe()?;
//! let dir = exe.parent().expect("exe dir").join("ghost").join("master");
//! let mut rt = GhostRuntime::load(dir.join("shiori.dll"), &dir, MyHost)?;
//! let r = rt.request("ID: OnBoot\r\n").map_err(|s| format!("ABI error: {s}"))?;
//! // ... wake されたら:
//! for d in rt.pump() { /* d.outcome の応答（スクリプト）を再生する */ }
//! # drop(r);
//! # Ok(())
//! # }
//! ```
//!
//! # 規約との対応
//! - 規約 1（単一スレッド）: `GhostRuntime` が `!Send` で、呼び出しは `&mut self`
//! - 規約 2（host.request の期間）: load / request / notify を呼んでいるスレッドで、その呼び出しの
//!   間だけ `Host::property`・`Host::request` に渡す。外れた呼び出しには SHIORI_E_WRONG_THREAD を返す
//! - 規約 3（再入禁止）: `Host` のメソッドは `&self` しか持たず、ランタイムに触れない。
//!   `Host` の中から `GhostRuntime` を呼べるような共有（Rc<RefCell<..>> 等）はしないこと
//! - 規約 4（post・log）: ホスト側では post をコピーしてキューに積むだけ。`on_post` は pump の中で
//!   呼ばれる。`Host::log` は任意のスレッドから呼ばれる
//! - 規約 5（unload）: Drop で、shutdown → 未配送の破棄 → unload → ライブラリ解放 の順に行う。
//!   SHIORI に join は求めない。unload が返った後、SHIORI はホストを呼ばない
//! - load の規約（dir）: dir を検査し、絶対パスでないか UTF-8 で表せなければ `LoadError::InvalidPath`。
//!   通るときは見た目だけ正規化し（OS の区切り・末尾に区切り）、SHIORI に渡す
//! - Charset: v4 は UTF-8 固定なので、組み込みの応答と完了通知の見出しに Charset を付けない
//!
//! # エラー定数
//! `shiori4_host` を use するだけで、ABI 層のエラー（`SHIORI_E_*`）がすべて使える。
//!
//! ```
//! use shiori4_host::{
//!     SHIORI_E_INVALID_ARG, SHIORI_E_INVALID_UTF8, SHIORI_E_PANIC, SHIORI_E_REENTRANT,
//!     SHIORI_E_SHUTDOWN, SHIORI_E_UNSUPPORTED, SHIORI_E_WRONG_THREAD,
//! };
//! assert_eq!(
//!     [SHIORI_E_INVALID_ARG, SHIORI_E_UNSUPPORTED, SHIORI_E_PANIC, SHIORI_E_REENTRANT,
//!      SHIORI_E_SHUTDOWN, SHIORI_E_INVALID_UTF8, SHIORI_E_WRONG_THREAD],
//!     [-1, -2, -3, -4, -5, -6, -7],
//! );
//! ```

use std::{
    ffi::OsStr,
    fmt,
    mem::ManuallyDrop,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
};

pub use shiori4;
pub use shiori4::{host_protocol, HeaderWriter, Headers};
pub use shiori4::shiori4_abi::{
    shiori_is_abi_error, shiori_is_success, ShioriApi, ShioriOwnedBytes, ShioriOwnedResponse,
    ShioriResponse, ShioriStatus, SHIORI_200_OK, SHIORI_204_NO_CONTENT, SHIORI_400_BAD_REQUEST,
    SHIORI_500_INTERNAL_SERVER_ERROR, SHIORI_ABI_VERSION, SHIORI_E_INVALID_ARG,
    SHIORI_E_INVALID_UTF8, SHIORI_E_PANIC, SHIORI_E_REENTRANT, SHIORI_E_SHUTDOWN,
    SHIORI_E_UNSUPPORTED, SHIORI_E_WRONG_THREAD, SHIORI_LOG_DEBUG, SHIORI_LOG_ERROR,
    SHIORI_LOG_INFO, SHIORI_LOG_WARN,
};
use shiori4::shiori4_abi::{
    ShioriBytes, ShioriGetApiFn, ShioriHost, ShioriHostBox, ShioriInstance, SHIORI_GET_API_SYMBOL,
};

mod v3;
mod discover;
mod dist;
pub use dist::{machine_matches, shiori_file_candidates, TargetArch, TargetOs};
#[cfg(test)]
mod test_dll;

// ============================================================
// ホスト実装者が実装する trait
// ============================================================

/// ホスト（ベースウェア）の実装者が実装する trait
///
/// すべて `&self` で呼ばれる。呼ばれるスレッドはメソッドごとに異なるので注意:
/// - `property` / `request`         : SHIORI のスレッド（= GhostRuntime を使っているスレッド）
/// - `accept_post` / `wake` / `log` : 任意のスレッド（SHIORI のワーカーからも呼ばれる）
/// - `on_post`                      : GhostRuntime::pump を呼んだスレッド
///
/// 任意のスレッドから呼ばれる `accept_post`・`wake`・`log` は、`GhostRuntime` を Drop するスレッドを
/// 同期で待たないこと（規約 4: コピーしてすぐ返る。待つと unload の門の待ちと互いに待ち合って止まる）
pub trait Host: Send + Sync + 'static {
    /// プロパティ取得（`ID: property` / `Reference0: <名前>`）に答える
    /// None なら 204（存在しない）。値に含まれる改行は取り除かれる
    fn property(&self, _name: &str) -> Option<String> {
        None
    }

    /// 組み込み以外の同期リクエストを処理する。None なら組み込みの処理に回す
    /// （組み込み: property。それ以外は 400）
    fn request(&self, _req: &Headers<'_>) -> Option<ShioriResponse> {
        None
    }

    /// post を受け付けるかを、その場で判定する（任意のスレッドから呼ばれる。軽く保つこと）
    /// `GhostRuntime` を Drop するスレッドを同期で待たないこと（規約 4）
    /// 既定では `Command` ヘッダがあれば受け付ける。false なら SHIORI に 400 を返す
    fn accept_post(&self, req: &Headers<'_>) -> bool {
        req.get(host_protocol::HEADER_COMMAND).is_some()
    }

    /// 受け付けた post を処理する（pump の中で呼ばれる）
    /// 結果を返すときは `post.into_completer()` を使う。Tag のない post では完了通知は捨てられる
    fn on_post(&self, post: Post);

    /// SHIORI からのログ（任意のスレッドから呼ばれる。規約 4）
    /// `GhostRuntime` を Drop するスレッドを同期で待たないこと（UI スレッドへの同期の送信など）
    fn log(&self, _level: i32, _msg: &str) {}

    /// キューに何かが積まれた（任意のスレッドから呼ばれる）
    /// イベントループに「GhostRuntime::pump を呼んで」と依頼する。依頼は非同期で行い、
    /// `GhostRuntime` を Drop するスレッドを同期で待たないこと（規約 4）
    fn wake(&self) {}
}

// ============================================================
// イベント
// ============================================================

/// SHIORI への届け方
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// request（GET 相当）。応答（スクリプトなど）を受け取る
    Request,
    /// notify（NOTIFY 相当）。応答本文はない
    Notify,
}

/// SHIORI に届けるイベント（ヘッダ部分だけ。開始行なし）
#[derive(Debug, Clone)]
pub struct Event {
    pub delivery: Delivery,
    pub headers: String,
}

impl Event {
    pub fn request(headers: impl Into<String>) -> Self {
        Self { delivery: Delivery::Request, headers: headers.into() }
    }

    pub fn notify(headers: impl Into<String>) -> Self {
        Self { delivery: Delivery::Notify, headers: headers.into() }
    }

    pub fn parsed(&self) -> Headers<'_> {
        Headers::parse(&self.headers)
    }

    pub fn id(&self) -> Option<&str> {
        Headers::parse(&self.headers).id()
    }
}

/// 届けた結果
#[derive(Debug)]
pub enum Outcome {
    /// request で届けた（400 や 500 もここ）
    Response(ShioriOwnedResponse),
    /// notify で届けた（ステータス）
    Notified(ShioriStatus),
    /// ABI 層のエラー（負の値）。SHIORI_E_PANIC ならランタイムは poisoned になる
    Failed(ShioriStatus),
}

/// pump で届けたイベントとその結果
#[derive(Debug)]
pub struct Delivered {
    pub event: Event,
    pub outcome: Outcome,
}

enum Msg {
    Post(String),
    Event(Event),
}

type WakeFn = Arc<dyn Fn() + Send + Sync>;

/// 任意のスレッドから SHIORI 宛てのイベントを積む送り口（Send + Sync + Clone）
/// ランタイムが破棄された後は send が false を返す
#[derive(Clone)]
pub struct EventSender {
    tx: mpsc::Sender<Msg>,
    wake: WakeFn,
    shutdown: Arc<AtomicBool>,
}

impl EventSender {
    pub fn send(&self, ev: Event) -> bool {
        if self.shutdown.load(Ordering::Acquire) {
            return false;
        }
        if self.tx.send(Msg::Event(ev)).is_err() {
            return false;
        }
        (self.wake)();
        true
    }

    pub fn is_closed(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }
}

impl fmt::Debug for EventSender {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventSender").field("closed", &self.is_closed()).finish()
    }
}

// ============================================================
// post と完了通知
// ============================================================

/// SHIORI から受け付けた post（ホスト側でコピー済み）
#[derive(Debug)]
pub struct Post {
    text: String,
    completer: Completer,
}

impl Post {
    fn new(text: String, sender: EventSender) -> Self {
        let (tag, command) = {
            let h = Headers::parse(&text);
            (
                h.get(host_protocol::HEADER_TAG).map(|s| s.trim().to_owned()),
                h.get(host_protocol::HEADER_COMMAND).map(str::to_owned),
            )
        };
        Self { text, completer: Completer { tag, command, sender } }
    }

    /// ヘッダ部分の原文
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn headers(&self) -> Headers<'_> {
        Headers::parse(&self.text)
    }

    pub fn command(&self) -> Option<&str> {
        self.completer.command.as_deref()
    }

    pub fn tag(&self) -> Option<&str> {
        self.completer.tag.as_deref()
    }

    /// 結果を返す必要があるか（Tag 付きか）
    pub fn wants_result(&self) -> bool {
        self.completer.tag.is_some()
    }

    pub fn into_completer(self) -> Completer {
        self.completer
    }

    /// 原文と Completer に分ける（原文を別スレッドへ持ち出す場合に使う）
    pub fn into_parts(self) -> (String, Completer) {
        (self.text, self.completer)
    }
}

/// post の完了を SHIORI に通知する（Send。別スレッドへ持ち出せる）
/// 使わずに捨てた場合、完了通知は届かない
#[derive(Debug)]
pub struct Completer {
    tag: Option<String>,
    command: Option<String>,
    sender: EventSender,
}

impl Completer {
    pub fn tag(&self) -> Option<&str> {
        self.tag.as_deref()
    }

    pub fn is_wanted(&self) -> bool {
        self.tag.is_some()
    }

    /// 標準形式（`ID: OnHostComplete`）で完了を通知する
    /// `Reference0` はコマンド名、`Reference1..` は refs。Tag のない post なら何もせず false
    pub fn complete(self, delivery: Delivery, status: ShioriStatus, refs: &[&str]) -> bool {
        let Some(tag) = self.tag else { return false };
        let mut w = HeaderWriter::new()
            .push("ID", host_protocol::ID_HOST_COMPLETE)
            .push(host_protocol::HEADER_TAG, &tag)
            .push(host_protocol::HEADER_STATUS, &status.to_string())
            .push("Reference0", self.command.as_deref().unwrap_or(""));
        for (i, r) in refs.iter().enumerate() {
            w = w.push(&format!("Reference{}", i + 1), r);
        }
        self.sender.send(Event { delivery, headers: w.finish() })
    }

    /// 任意のヘッダで完了を通知する（Tag は自動で先頭に付く）
    pub fn complete_with(self, delivery: Delivery, headers: &str) -> bool {
        let Some(tag) = self.tag else { return false };
        let h = HeaderWriter::new()
            .push(host_protocol::HEADER_TAG, &tag)
            .push_raw(headers)
            .finish();
        self.sender.send(Event { delivery, headers: h })
    }
}

// ============================================================
// ABI 層の ShioriHost への橋渡し（非公開）
// ============================================================

struct Bridge<H> {
    host: Arc<H>,
    tx: mpsc::Sender<Msg>,
    shutdown: Arc<AtomicBool>,
    /// 持ち主のスレッド（with_api を呼んだスレッド）
    owner: std::thread::ThreadId,
    /// SHIORI の load / request / notify を呼んでいる間だけ true（規約 2 の期間）
    in_call: AtomicBool,
}

/// 規約 2 の期間の印を立て、Drop で下ろす
struct CallScope<'a>(&'a AtomicBool);

impl<'a> CallScope<'a> {
    fn new(flag: &'a AtomicBool) -> Self {
        // 他のスレッドは owner との比較で必ず -7 になるので、印の順序は Relaxed で足りる
        flag.store(true, Ordering::Relaxed);
        Self(flag)
    }
}

impl Drop for CallScope<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Relaxed);
    }
}

impl<H: Host> ShioriHost for Bridge<H> {
    fn request(&self, req: &[u8]) -> ShioriResponse {
        // 規約 2: 持ち主のスレッドで、load / request / notify の処理中だけ受ける
        if std::thread::current().id() != self.owner || !self.in_call.load(Ordering::Relaxed) {
            return ShioriResponse::with_status(SHIORI_E_WRONG_THREAD, ShioriBytes::empty());
        }
        let Ok(text) = std::str::from_utf8(req) else {
            return ShioriResponse::bad_request();
        };
        let h = Headers::parse(text);
        if let Some(r) = self.host.request(&h) {
            return r;
        }
        match h.id() {
            Some(id) if id == host_protocol::ID_PROPERTY => match h.reference(0) {
                Some(name) => match self.host.property(name) {
                    Some(v) => HeaderWriter::new()
                        .push(host_protocol::HEADER_VALUE, &v)
                        .into_response(),
                    None => ShioriResponse::no_content(),
                },
                None => ShioriResponse::bad_request(),
            },
            _ => ShioriResponse::bad_request(),
        }
    }

    fn post(&self, msg: &[u8]) -> ShioriStatus {
        if self.shutdown.load(Ordering::Acquire) {
            return SHIORI_E_SHUTDOWN;
        }
        let Ok(text) = std::str::from_utf8(msg) else {
            return SHIORI_400_BAD_REQUEST;
        };
        if !self.host.accept_post(&Headers::parse(text)) {
            return SHIORI_400_BAD_REQUEST;
        }
        // msg は呼び出し中だけ有効なのでコピーして積む
        if self.tx.send(Msg::Post(text.to_owned())).is_err() {
            return SHIORI_E_SHUTDOWN;
        }
        self.host.wake();
        SHIORI_204_NO_CONTENT
    }

    fn log(&self, level: i32, msg: &str) {
        self.host.log(level, msg);
    }
}

// ============================================================
// 読み込みエラー
// ============================================================

#[derive(Debug)]
pub enum LoadError {
    /// ライブラリを開けなかった
    Library(libloading::Error),
    /// shiori_get_api がない（旧 SHIORI の可能性）
    NotShiori4,
    /// 要求した版に対応していない
    Unsupported(ShioriStatus),
    /// ゴーストディレクトリが絶対パスでない、または UTF-8 で表せない
    InvalidPath,
    /// SHIORI の load が失敗した
    LoadFailed,
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Library(e) => write!(f, "failed to open SHIORI library: {e}"),
            Self::NotShiori4 => f.write_str("shiori_get_api not found (legacy SHIORI?)"),
            Self::Unsupported(s) => write!(f, "unsupported SHIORI ABI version (status {s})"),
            Self::InvalidPath => f.write_str("ghost directory is not an absolute path or not valid UTF-8"),
            Self::LoadFailed => f.write_str("SHIORI load failed"),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Library(e) => Some(e),
            _ => None,
        }
    }
}

/// [`GhostRuntime::from_library`] の失敗。読み直せる失敗ではライブラリを返す
///
/// ライブラリが返るのは `InvalidPath`・`NotShiori4`・`Unsupported`（SHIORI の初期化の前の失敗）
/// のときだけ。`LoadFailed` では返さない（v4 の SHIORI が初期化に失敗したので読み直さない。
/// `shiori_get_api` を持つ DLL は v4 だと宣言しているので、旧来の口で取り繕うと二重の初期化か
/// 作者の意図と違う動きになる）。フィールドは非公開で、この組み合わせは型の中で守る
#[derive(Debug)]
pub struct FromLibraryError {
    error: LoadError,
    library: Option<libloading::Library>,
}

impl FromLibraryError {
    pub fn error(&self) -> &LoadError {
        &self.error
    }

    /// `NotShiori4`・`Unsupported`・`InvalidPath` のときだけ Some
    pub fn into_library(self) -> Option<libloading::Library> {
        self.library
    }

    pub fn into_parts(self) -> (LoadError, Option<libloading::Library>) {
        (self.error, self.library)
    }
}

impl fmt::Display for FromLibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}

impl std::error::Error for FromLibraryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// ライブラリを捨てて LoadError にする
impl From<FromLibraryError> for LoadError {
    fn from(e: FromLibraryError) -> Self {
        e.error
    }
}

/// dir を検査し、見た目だけ正規化する（絶対パス・OS の区切り・末尾に区切り 1 つの UTF-8）
///
/// ファイルシステムには触れない（存在の確認・リンクや `..` の解決・`\\?\` の付加をしない）。
/// 相対パスはカレントを基準に解決せず `InvalidPath` で断る（Windows の `\ghost`・`C:ghost` も相対）
fn normalize_dir(dir: &Path) -> Result<String, LoadError> {
    if !dir.is_absolute() {
        return Err(LoadError::InvalidPath);
    }
    let s = dir.to_str().ok_or(LoadError::InvalidPath)?;
    // `\\?\` の形では `/` は区切りではないので置き換えない。Unix では `\` は名前の文字
    let mut s = if cfg!(windows) && !s.starts_with(r"\\?\") { s.replace('/', "\\") } else { s.to_owned() };
    let sep = std::path::MAIN_SEPARATOR;
    s.truncate(s.trim_end_matches(sep).len());
    s.push(sep);
    Ok(s)
}

// ============================================================
// ランタイム
// ============================================================

/// 1つの SHIORI インスタンスと、それを動かすホスト側の状態をまとめて所有する
///
/// `!Send`（中の ShioriInstance が !Send）。作ったスレッドで使うこと。
/// Drop で、shutdown → 未配送イベントの破棄 → unload → ライブラリ解放 の順に後始末する。
pub struct GhostRuntime<H: Host> {
    // 解放順序は Drop で明示する。_lib は必ず最後
    instance: ManuallyDrop<ShioriInstance<'static>>,
    host_box: ManuallyDrop<ShioriHostBox<Bridge<H>>>,
    rx: mpsc::Receiver<Msg>,
    host: Arc<H>,
    sender: EventSender,
    shutdown: Arc<AtomicBool>,
    poisoned: bool,
    _lib: Option<libloading::Library>,
}

impl<H: Host> GhostRuntime<H> {
    /// pump 1回で処理するメッセージ数の既定上限
    pub const DEFAULT_PUMP_LIMIT: usize = 256;

    /// SHIORI のライブラリを読み込み、v4 で load する
    pub fn load(
        lib_path: impl AsRef<OsStr>,
        ghost_dir: impl AsRef<Path>,
        host: H,
    ) -> Result<Self, LoadError> {
        // dir の検査をライブラリを開くより先にする（開けないライブラリでも InvalidPath が先に出る）
        normalize_dir(ghost_dir.as_ref())?;

        // SAFETY: ライブラリの初期化コードが実行される。信頼できる SHIORI だけを読み込むこと
        let lib = unsafe { libloading::Library::new(lib_path) }.map_err(LoadError::Library)?;
        Self::from_library(lib, ghost_dir, host).map_err(LoadError::from)
    }

    /// 読み終えたライブラリから v4 で作る
    ///
    /// 手順は dir の検査 → `shiori_get_api` を探す → 版 4 のテーブルの検査 → load。
    /// `InvalidPath`・`NotShiori4`・`Unsupported` で失敗したときは、エラーと一緒にライブラリを返す
    /// （[`FromLibraryError::into_library`]）。どれも SHIORI の初期化の前なので、dir を直したり、
    /// 別の版や SHIORI/3.0 として同じライブラリで読み直したりできる。
    ///
    /// `LoadFailed`（v4 の load が NULL を返した）ではライブラリを返さない。v4 の SHIORI が
    /// 初期化に失敗したので読み直さない。`shiori_get_api` を持つ DLL は v4 だと宣言しているので、
    /// 旧来の口で取り繕うと二重の初期化か作者の意図と違う動きになる。理由は `Host::log` に届いている。
    ///
    /// lib は信頼できる SHIORI であること（`shiori_get_api` と load で DLL のコードが動く）
    pub fn from_library(
        lib: libloading::Library,
        ghost_dir: impl AsRef<Path>,
        host: H,
    ) -> Result<Self, FromLibraryError> {
        let load_failed = |error| FromLibraryError { error, library: None };
        let dir = match normalize_dir(ghost_dir.as_ref()) {
            Ok(d) => d,
            Err(error) => return Err(FromLibraryError { error, library: Some(lib) }),
        };
        let get: ShioriGetApiFn = match unsafe { lib.get::<ShioriGetApiFn>(SHIORI_GET_API_SYMBOL) } {
            Ok(s) => *s,
            Err(_) => return Err(FromLibraryError { error: LoadError::NotShiori4, library: Some(lib) }),
        };

        let p = unsafe { get(SHIORI_ABI_VERSION) };
        // SAFETY: lib は GhostRuntime の最後のフィールドとして保持し、instance より後に解放する。
        // 'static はこの構造体の外へ漏らさない
        let api: &'static ShioriApi = match unsafe { ShioriApi::from_raw(p, SHIORI_ABI_VERSION) } {
            Ok(api) => api,
            Err(s) => return Err(FromLibraryError { error: LoadError::Unsupported(s), library: Some(lib) }),
        };

        // with_api が失敗したとき lib は中で捨てられる（LoadFailed は読み直さない）
        Self::with_api(api, &dir, host, Some(lib)).map_err(load_failed)
    }

    /// 同じプロセス内のテーブル（ShioriApi::new で作った static）を使う。テスト用
    ///
    /// dir は `load` と同じ検査と正規化を経て SHIORI に届く
    pub fn from_static(
        api: &'static ShioriApi,
        ghost_dir: impl AsRef<Path>,
        host: H,
    ) -> Result<Self, LoadError> {
        let dir = normalize_dir(ghost_dir.as_ref())?;
        if api.abi_version() < SHIORI_ABI_VERSION {
            return Err(LoadError::Unsupported(SHIORI_E_UNSUPPORTED));
        }
        Self::with_api(api, &dir, host, None)
    }

    fn with_api(
        api: &'static ShioriApi,
        dir: &str,
        host: H,
        lib: Option<libloading::Library>,
    ) -> Result<Self, LoadError> {
        let host = Arc::new(host);
        let (tx, rx) = mpsc::channel();
        let shutdown = Arc::new(AtomicBool::new(false));
        let wake: WakeFn = {
            let h = Arc::clone(&host);
            Arc::new(move || h.wake())
        };
        let sender = EventSender { tx: tx.clone(), wake, shutdown: Arc::clone(&shutdown) };
        let host_box = ShioriHostBox::new(Bridge {
            host: Arc::clone(&host),
            tx,
            shutdown: Arc::clone(&shutdown),
            owner: std::thread::current().id(),
            in_call: AtomicBool::new(false),
        });

        let inst = {
            let _scope = CallScope::new(&host_box.host().in_call);
            api.load(&host_box, dir)
        }
        .ok_or(LoadError::LoadFailed)?;
        // SAFETY: instance が参照するのは api（lib が生きている間有効）と host_box の中身（ヒープ上で
        // 位置が変わらない）。どちらも Drop で instance より後に解放する
        let instance = unsafe { std::mem::transmute::<ShioriInstance<'_>, ShioriInstance<'static>>(inst) };

        Ok(Self {
            instance: ManuallyDrop::new(instance),
            host_box: ManuallyDrop::new(host_box),
            rx,
            host,
            sender,
            shutdown,
            poisoned: false,
            _lib: lib,
        })
    }

    pub fn host(&self) -> &H {
        &self.host
    }

    /// 任意のスレッドからイベントを積むための送り口
    pub fn event_sender(&self) -> EventSender {
        self.sender.clone()
    }

    /// SHIORI がパニックした（SHIORI_E_PANIC を返した）か。true なら破棄して読み直すこと
    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    /// request（GET 相当）。headers はヘッダ部分だけ
    /// Err は ABI 層のエラー。poisoned の後は常に Err(SHIORI_E_PANIC)
    pub fn request(&mut self, headers: &str) -> Result<ShioriOwnedResponse, ShioriStatus> {
        if self.poisoned {
            return Err(SHIORI_E_PANIC);
        }
        let r = {
            let _scope = CallScope::new(&self.host_box.host().in_call);
            self.instance.request(headers)
        };
        if matches!(r, Err(SHIORI_E_PANIC)) {
            self.poisoned = true;
        }
        r
    }

    /// notify（NOTIFY 相当）
    pub fn notify(&mut self, headers: &str) -> Result<ShioriStatus, ShioriStatus> {
        if self.poisoned {
            return Err(SHIORI_E_PANIC);
        }
        let r = {
            let _scope = CallScope::new(&self.host_box.host().in_call);
            self.instance.notify(headers)
        };
        if matches!(r, Err(SHIORI_E_PANIC)) {
            self.poisoned = true;
        }
        r
    }

    /// イベントを1つ、指定された方法で届ける
    pub fn deliver(&mut self, ev: &Event) -> Outcome {
        match ev.delivery {
            Delivery::Request => match self.request(&ev.headers) {
                Ok(r) => Outcome::Response(r),
                Err(e) => Outcome::Failed(e),
            },
            Delivery::Notify => match self.notify(&ev.headers) {
                Ok(s) => Outcome::Notified(s),
                Err(e) => Outcome::Failed(e),
            },
        }
    }

    /// キューを処理する（Host::wake を受けたら、イベントループから呼ぶ）
    /// post は Host::on_post に渡し、イベントは SHIORI に届けて結果を返す
    pub fn pump(&mut self) -> Vec<Delivered> {
        self.pump_limit(Self::DEFAULT_PUMP_LIMIT)
    }

    /// 処理数の上限を指定して pump する。上限に達したら Host::wake を呼び直す
    pub fn pump_limit(&mut self, limit: usize) -> Vec<Delivered> {
        let mut out = Vec::new();
        let mut processed = 0;
        while processed < limit {
            let Ok(msg) = self.rx.try_recv() else { break };
            processed += 1;
            match msg {
                Msg::Post(text) => {
                    let post = Post::new(text, self.sender.clone());
                    self.host.on_post(post);
                }
                Msg::Event(ev) => {
                    let outcome = self.deliver(&ev);
                    out.push(Delivered { event: ev, outcome });
                }
            }
        }
        if processed == limit {
            // まだ残っている可能性がある。イベントループに再度 pump を依頼する
            self.host.wake();
        }
        out
    }
}

impl<H: Host> Drop for GhostRuntime<H> {
    fn drop(&mut self) {
        // 1. 以後の post / イベント送信を拒否する
        self.shutdown.store(true, Ordering::Release);
        // 2. 未配送のメッセージを破棄する
        while self.rx.try_recv().is_ok() {}
        // 3. unload（unload が返った後、SHIORI はホストを呼ばない。規約 5）
        // SAFETY: 以後 instance には触れない
        unsafe { ManuallyDrop::drop(&mut self.instance) };
        // 4. ホストのテーブルを解放する
        unsafe { ManuallyDrop::drop(&mut self.host_box) };
        // 5. 残りのフィールドは宣言順に解放され、_lib（ライブラリ）が最後になる
    }
}

// ============================================================
// テスト（DLL を介さず、同一プロセスのゴーストで往復させる）
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use shiori4::shiori4_abi::{ShioriHostHandle, ShioriHostPoster};
    use shiori4::{Adapter, Ghost, Lookup, PendingTags, Properties};
    use std::sync::atomic::AtomicUsize;

    /// テストの dir。OS ごとの絶対パス（Windows では "/ghost" は絶対でない）
    #[cfg(windows)]
    const GHOST_DIR: &str = r"C:\ghost";
    #[cfg(not(windows))]
    const GHOST_DIR: &str = "/ghost";
    /// GHOST_DIR を正規化したもの（SHIORI に届く形）
    #[cfg(windows)]
    const GHOST_DIR_NORMALIZED: &str = r"C:\ghost\";
    #[cfg(not(windows))]
    const GHOST_DIR_NORMALIZED: &str = "/ghost/";

    /// TestGhost の load が受け取った dir（どのテストも GHOST_DIR を正規化した形を渡す）
    static LOADED_DIR: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

    struct TestGhost {
        props: Properties,
        poster: Option<ShioriHostPoster>,
        pending: PendingTags<&'static str>,
    }

    impl Ghost for TestGhost {
        fn load(host: Option<ShioriHostHandle>, dir: &str) -> Result<Self, ShioriStatus> {
            *LOADED_DIR.lock().unwrap() = dir.to_owned();
            let poster = host.as_ref().map(|h| h.poster());
            Ok(Self { props: Properties::new(host), poster, pending: PendingTags::new() })
        }

        fn request(&mut self, req: &Headers<'_>) -> ShioriResponse {
            match req.id() {
                Some("OnBoot") => {
                    let name = match self.props.get("name") {
                        Lookup::Ready(v) => v.to_owned(),
                        _ => "?".to_owned(),
                    };
                    if let Some(p) = &self.poster {
                        let _ = self.pending.post(p, "dl", "Command: test.echo\r\n");
                    }
                    HeaderWriter::new().push("Value", &name).into_response()
                }
                Some(host_protocol::ID_HOST_COMPLETE) => match self.pending.take_from(req) {
                    Some((_, kind)) => HeaderWriter::new()
                        .push(
                            "Value",
                            &format!(
                                "done:{kind}:{}:{}",
                                req.get(host_protocol::HEADER_STATUS).unwrap_or(""),
                                req.reference(1).unwrap_or("")
                            ),
                        )
                        .into_response(),
                    None => ShioriResponse::bad_request(),
                },
                _ => ShioriResponse::no_content(),
            }
        }
    }

    static API: ShioriApi = ShioriApi::new::<Adapter<TestGhost>, { SHIORI_ABI_VERSION }>();

    struct TestHost {
        woken: AtomicUsize,
    }

    impl Host for TestHost {
        fn property(&self, name: &str) -> Option<String> {
            (name == "name").then(|| "さくら".to_owned())
        }

        fn on_post(&self, post: Post) {
            assert_eq!(post.command(), Some("test.echo"));
            assert!(post.wants_result());
            post.into_completer().complete(Delivery::Request, SHIORI_200_OK, &["ok"]);
        }

        fn wake(&self) {
            self.woken.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn runtime() -> GhostRuntime<TestHost> {
        GhostRuntime::from_static(&API, GHOST_DIR, TestHost { woken: AtomicUsize::new(0) })
            .expect("load")
    }

    #[test]
    fn property_post_and_completion() {
        let mut rt = runtime();

        let r = rt.request("ID: OnBoot\r\n").expect("abi");
        assert_eq!(r.status, 200);
        assert!(r.body.as_str().unwrap().contains("Value: さくら"));
        drop(r);
        assert!(rt.host().woken.load(Ordering::Relaxed) >= 1);

        // post → on_post → 完了イベント → SHIORI に配送
        let delivered = rt.pump();
        assert_eq!(delivered.len(), 1);
        match &delivered[0].outcome {
            Outcome::Response(r) => {
                assert!(r.body.as_str().unwrap().contains("Value: done:dl:200:ok"));
            }
            o => panic!("unexpected outcome: {o:?}"),
        }
        assert!(rt.pump().is_empty());
    }

    #[test]
    fn sender_after_drop() {
        let rt = runtime();
        let s = rt.event_sender();
        assert!(s.send(Event::notify("ID: OnTest\r\n")));
        drop(rt);
        assert!(s.is_closed());
        assert!(!s.send(Event::notify("ID: OnTest\r\n")));
    }

    #[test]
    fn unknown_host_request_is_400() {
        let b = Bridge {
            host: Arc::new(TestHost { woken: AtomicUsize::new(0) }),
            tx: mpsc::channel().0,
            shutdown: Arc::new(AtomicBool::new(false)),
            owner: std::thread::current().id(),
            in_call: AtomicBool::new(true),
        };
        let r = ShioriHost::request(&b, b"ID: unknown\r\n");
        assert_eq!(r.status, 400);
        let _ = r.into_owned();
        // Command のない post は 400
        assert_eq!(ShioriHost::post(&b, b"Tag: 1\r\n"), SHIORI_400_BAD_REQUEST);
    }

    // ---- Charset を付けない（9.2） ----

    #[test]
    fn builtin_responses_have_no_charset() {
        let b = count_bridge(true);
        for req in [PROP_REQ, b"ID: property\r\n", b"ID: unknown\r\n"] {
            let r = ShioriHost::request(&b, req).into_owned();
            assert!(r.status >= 0);
            assert!(!r.body.as_str().unwrap().to_ascii_lowercase().contains("charset"), "{:?}", r.body.as_str());
        }
    }

    #[test]
    fn completion_headers_have_no_charset() {
        let (tx, rx) = mpsc::channel();
        let sender = EventSender { tx, wake: Arc::new(|| {}), shutdown: Arc::new(AtomicBool::new(false)) };
        let post = Post::new("Command: test.echo\r\nTag: 1\r\n".to_owned(), sender.clone());
        assert!(post.into_completer().complete(Delivery::Request, SHIORI_200_OK, &["ok"]));
        let post = Post::new("Command: test.echo\r\nTag: 2\r\n".to_owned(), sender);
        assert!(post.into_completer().complete_with(Delivery::Notify, "X: y\r\n"));
        for _ in 0..2 {
            let Ok(Msg::Event(ev)) = rx.try_recv() else { panic!("event") };
            assert!(ev.parsed().get(host_protocol::HEADER_TAG).is_some());
            assert!(!ev.headers.to_ascii_lowercase().contains("charset"), "{}", ev.headers);
        }
    }

    // ---- 規約 2（ThreadGuard） ----

    /// property と request が呼ばれた回数と、log で届いた文を数える
    #[derive(Default)]
    struct CountHost {
        calls: AtomicUsize,
        logs: std::sync::Mutex<Vec<String>>,
    }

    impl Host for CountHost {
        fn property(&self, name: &str) -> Option<String> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Some(format!("v:{name}"))
        }

        fn request(&self, _req: &Headers<'_>) -> Option<ShioriResponse> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            None
        }

        fn on_post(&self, _post: Post) {}

        fn log(&self, _level: i32, msg: &str) {
            self.logs.lock().unwrap().push(msg.to_owned());
        }
    }

    const PROP_REQ: &[u8] = b"ID: property\r\nReference0: name\r\n";

    fn count_bridge(in_call: bool) -> Bridge<CountHost> {
        Bridge {
            host: Arc::new(CountHost::default()),
            tx: mpsc::channel().0,
            shutdown: Arc::new(AtomicBool::new(false)),
            owner: std::thread::current().id(),
            in_call: AtomicBool::new(in_call),
        }
    }

    #[test]
    fn request_from_other_thread_is_wrong_thread() {
        // 持ち主のスレッドが処理中でも、別スレッドからの request は -7 で Host に渡らない
        let b = count_bridge(true);
        let r = std::thread::scope(|s| s.spawn(|| ShioriHost::request(&b, PROP_REQ).into_owned()).join().unwrap());
        assert_eq!(r.status, SHIORI_E_WRONG_THREAD);
        assert!(r.body.is_empty());
        assert_eq!(b.host.calls.load(Ordering::Relaxed), 0);
        // 持ち主のスレッドからなら通る
        let r = ShioriHost::request(&b, PROP_REQ).into_owned();
        assert_eq!(r.status, 200);
        assert_eq!(b.host.calls.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn request_outside_call_is_wrong_thread() {
        let b = count_bridge(false);
        let r = ShioriHost::request(&b, PROP_REQ).into_owned();
        assert_eq!(r.status, SHIORI_E_WRONG_THREAD);
        assert!(r.body.is_empty());
        assert_eq!(b.host.calls.load(Ordering::Relaxed), 0);
    }

    /// load・request・notify の中で property を取り、別スレッドから log を送るゴースト
    struct GuardGhost {
        props: Properties,
        poster: Option<ShioriHostPoster>,
        at_load: String,
    }

    fn value(props: &mut Properties, name: &str) -> String {
        match props.get(name) {
            Lookup::Ready(v) => v.to_owned(),
            _ => "?".to_owned(),
        }
    }

    impl Ghost for GuardGhost {
        fn load(host: Option<ShioriHostHandle>, _dir: &str) -> Result<Self, ShioriStatus> {
            let poster = host.as_ref().map(|h| h.poster());
            let mut props = Properties::new(host);
            let at_load = value(&mut props, "load");
            Ok(Self { props, poster, at_load })
        }

        fn request(&mut self, req: &Headers<'_>) -> ShioriResponse {
            match req.id() {
                Some("OnLog") => {
                    let p = self.poster.clone().unwrap();
                    std::thread::spawn(move || p.log(SHIORI_LOG_INFO, "from worker")).join().unwrap();
                    ShioriResponse::no_content()
                }
                _ => {
                    let v = format!("{}|{}", self.at_load, value(&mut self.props, "req"));
                    HeaderWriter::new().push("Value", &v).into_response()
                }
            }
        }

        fn notify(&mut self, _req: &Headers<'_>) -> ShioriStatus {
            if value(&mut self.props, "notify") == "v:notify" { SHIORI_200_OK } else { SHIORI_500_INTERNAL_SERVER_ERROR }
        }
    }

    static GUARD_API: ShioriApi = ShioriApi::new::<Adapter<GuardGhost>, { SHIORI_ABI_VERSION }>();

    fn guard_runtime() -> GhostRuntime<CountHost> {
        GhostRuntime::from_static(&GUARD_API, GHOST_DIR, CountHost::default()).expect("load")
    }

    #[test]
    fn property_inside_load_request_notify() {
        let mut rt = guard_runtime();
        let r = rt.request("ID: OnBoot\r\n").expect("abi");
        assert!(r.body.as_str().unwrap().contains("Value: v:load|v:req"), "{:?}", r.body.as_str());
        drop(r);
        assert_eq!(rt.notify("ID: OnTest\r\n"), Ok(SHIORI_200_OK));
        // 呼び出しが返った後は印が下りている
        let r = ShioriHost::request(rt.host_box.host(), PROP_REQ).into_owned();
        assert_eq!(r.status, SHIORI_E_WRONG_THREAD);
    }

    #[test]
    fn two_runtimes_on_two_threads() {
        let barrier = std::sync::Barrier::new(2);
        std::thread::scope(|s| {
            for _ in 0..2 {
                s.spawn(|| {
                    let mut rt = guard_runtime();
                    barrier.wait();
                    for _ in 0..100 {
                        let r = rt.request("ID: OnBoot\r\n").expect("abi");
                        assert!(r.body.as_str().unwrap().contains("Value: v:load|v:req"));
                    }
                });
            }
        });
    }

    #[test]
    fn poster_log_from_other_thread_reaches_host() {
        let mut rt = guard_runtime();
        let r = rt.request("ID: OnLog\r\n").expect("abi");
        assert_eq!(r.status, 204);
        drop(r);
        assert_eq!(*rt.host().logs.lock().unwrap(), ["from worker"]);
    }

    // ---- dir の検査と正規化（要件 8.3〜8.8） ----

    /// 正規化の結果。InvalidPath は None
    fn norm(dir: impl AsRef<Path>) -> Option<String> {
        match normalize_dir(dir.as_ref()) {
            Ok(s) => Some(s),
            Err(LoadError::InvalidPath) => None,
            Err(e) => panic!("想定外のエラー: {e}"),
        }
    }

    #[cfg(windows)]
    #[test]
    fn normalize_dir_table_windows() {
        let ok = |input: &str, want: &str| assert_eq!(norm(input).as_deref(), Some(want), "{input}");
        ok(r"C:\a\b", r"C:\a\b\");
        ok(r"C:\a\b\", r"C:\a\b\");
        ok(r"C:\a\b\\", r"C:\a\b\");
        ok("C:/a/b", r"C:\a\b\");
        ok("C:/a/b/", r"C:\a\b\");
        ok(r"C:\", r"C:\");
        ok(r"\\server\share", r"\\server\share\");
        ok(r"\\server\share\", r"\\server\share\");
        ok("//server/share", r"\\server\share\");
        // \\?\ の形では / は区切りではないので置き換えない。\\?\ を足しもしない
        ok(r"\\?\C:\a/b", r"\\?\C:\a/b\");
        ok(r"\\?\C:\a\", r"\\?\C:\a\");
        // ファイルシステムに触れない（.. も解決しない）
        ok(r"C:\no\such\..\dir", r"C:\no\such\..\dir\");
        for bad in ["ghost", r"ghost\master", r"\ghost", "/ghost", "C:ghost", ""] {
            assert_eq!(norm(bad), None, "{bad}");
        }
        use std::os::windows::ffi::OsStringExt;
        // 対でないサロゲート
        let w: Vec<u16> = r"C:\a".encode_utf16().chain([0xD800]).collect();
        assert_eq!(norm(std::ffi::OsString::from_wide(&w)), None);
    }

    #[cfg(unix)]
    #[test]
    fn normalize_dir_table_unix() {
        let ok = |input: &str, want: &str| assert_eq!(norm(input).as_deref(), Some(want), "{input}");
        ok("/a/b", "/a/b/");
        ok("/a/b/", "/a/b/");
        ok("/a/b//", "/a/b/");
        ok("/", "/");
        // Unix では \ は名前の文字
        ok(r"/a\b", r"/a\b/");
        // ファイルシステムに触れない（.. も解決しない）
        ok("/no/such/../dir", "/no/such/../dir/");
        for bad in ["ghost", "ghost/master", "./ghost", r"C:\ghost", ""] {
            assert_eq!(norm(bad), None, "{bad}");
        }
        use std::os::unix::ffi::OsStrExt;
        assert_eq!(norm(OsStr::from_bytes(b"/a/\xff")), None);
    }

    #[test]
    fn from_static_passes_normalized_dir_to_shiori() {
        // 末尾の区切りを二重にして渡す（Windows では `/` を混ぜることにもなる）
        let messy = format!("{}//", GHOST_DIR.trim_end_matches(std::path::MAIN_SEPARATOR));
        let _rt = GhostRuntime::from_static(&API, &messy, TestHost { woken: AtomicUsize::new(0) })
            .expect("load");
        let got = LOADED_DIR.lock().unwrap().clone();
        assert_eq!(got, GHOST_DIR_NORMALIZED);
    }

    #[test]
    fn from_static_rejects_relative_dir() {
        for bad in ["ghost", "ghost/master"] {
            let r = GhostRuntime::from_static(&API, bad, TestHost { woken: AtomicUsize::new(0) });
            assert!(matches!(r, Err(LoadError::InvalidPath)), "{bad}");
        }
    }

    #[test]
    fn load_rejects_relative_dir_before_opening_library() {
        // ライブラリのパスは存在しない。dir の検査が先なので Library ではなく InvalidPath になる
        let r = GhostRuntime::load("no-such-shiori.dll", "ghost", TestHost { woken: AtomicUsize::new(0) });
        assert!(matches!(r, Err(LoadError::InvalidPath)));
    }

    #[test]
    fn invalid_path_display_mentions_absolute() {
        assert!(LoadError::InvalidPath.to_string().contains("absolute"));
    }

    // ---- from_library（要件 7.6・12.1〜12.4） ----

    struct FailGhost;

    impl Ghost for FailGhost {
        fn load(_host: Option<ShioriHostHandle>, _dir: &str) -> Result<Self, ShioriStatus> {
            Err(SHIORI_500_INTERNAL_SERVER_ERROR)
        }

        fn request(&mut self, _req: &Headers<'_>) -> ShioriResponse {
            ShioriResponse::no_content()
        }
    }

    static FAIL_API: ShioriApi = ShioriApi::new::<Adapter<FailGhost>, { SHIORI_ABI_VERSION }>();

    #[test]
    fn from_static_load_null_is_load_failed() {
        let r = GhostRuntime::from_static(&FAIL_API, GHOST_DIR, TestHost { woken: AtomicUsize::new(0) });
        assert!(matches!(r, Err(LoadError::LoadFailed)));
    }

    fn test_lib() -> libloading::Library {
        unsafe { libloading::Library::new(crate::test_dll::dll_path()) }.expect("テスト用 DLL を開けない")
    }

    fn test_host() -> TestHost {
        TestHost { woken: AtomicUsize::new(0) }
    }

    #[test]
    fn from_library_ok() {
        let _g = crate::test_dll::lock();
        let mut rt = GhostRuntime::from_library(test_lib(), GHOST_DIR, test_host()).expect("load");
        let r = rt.request("ID: OnBoot\r\n").expect("abi");
        assert!(r.body.as_str().unwrap().contains("Value: testghost"));
    }

    #[test]
    fn from_library_not_shiori4_returns_library() {
        #[cfg(windows)]
        const OS_LIB: &str = "kernel32.dll";
        #[cfg(target_os = "macos")]
        const OS_LIB: &str = "/usr/lib/libSystem.B.dylib";
        #[cfg(all(unix, not(target_os = "macos")))]
        const OS_LIB: &str = "libc.so.6";
        let lib = unsafe { libloading::Library::new(OS_LIB) }.expect("OS のライブラリを開けない");
        let e = GhostRuntime::from_library(lib, GHOST_DIR, test_host()).err().expect("失敗するはず");
        assert!(matches!(e.error(), LoadError::NotShiori4));
        assert!(e.into_library().is_some());
    }

    #[test]
    fn from_library_unsupported_returns_library() {
        let _g = crate::test_dll::lock();
        let lib = test_lib();
        crate::test_dll::set_mode(&lib, crate::test_dll::MODE_V3);
        let e = GhostRuntime::from_library(lib, GHOST_DIR, test_host()).err().expect("失敗するはず");
        assert!(matches!(e.error(), LoadError::Unsupported(_)));
        // 返ったライブラリでやり直せる
        let lib = e.into_library().expect("ライブラリが返る");
        crate::test_dll::set_mode(&lib, crate::test_dll::MODE_DEFAULT);
        assert!(GhostRuntime::from_library(lib, GHOST_DIR, test_host()).is_ok());
    }

    #[test]
    fn from_library_invalid_path_returns_library() {
        let _g = crate::test_dll::lock();
        let e = GhostRuntime::from_library(test_lib(), "ghost", test_host()).err().expect("失敗するはず");
        assert!(matches!(e.error(), LoadError::InvalidPath));
        let (err, lib) = e.into_parts();
        assert!(matches!(err, LoadError::InvalidPath));
        // dir を直して同じライブラリでやり直せる
        assert!(GhostRuntime::from_library(lib.expect("ライブラリが返る"), GHOST_DIR, test_host()).is_ok());
    }

    #[test]
    fn from_library_load_failed_drops_library() {
        let _g = crate::test_dll::lock();
        let dir = std::path::Path::new(GHOST_DIR).join("fail");
        let e = GhostRuntime::from_library(test_lib(), &dir, test_host()).err().expect("失敗するはず");
        assert!(matches!(e.error(), LoadError::LoadFailed));
        assert_eq!(e.to_string(), LoadError::LoadFailed.to_string());
        assert!(std::error::Error::source(&e).is_some());
        assert!(matches!(LoadError::from(e), LoadError::LoadFailed));
    }

    #[test]
    fn load_goes_through_from_library() {
        let _g = crate::test_dll::lock();
        let rt = GhostRuntime::load(crate::test_dll::dll_path(), GHOST_DIR, test_host());
        assert!(rt.is_ok());
        let dir = std::path::Path::new(GHOST_DIR).join("fail");
        let r = GhostRuntime::load(crate::test_dll::dll_path(), &dir, test_host());
        assert!(matches!(r, Err(LoadError::LoadFailed)));
    }
}
