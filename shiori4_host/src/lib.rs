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
//! ```ignore
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
//! let mut rt = GhostRuntime::load("ghost/master/shiori.dll", "ghost/master", MyHost)?;
//! let r = rt.request("ID: OnBoot\r\n")?;
//! // ... wake されたら:
//! for d in rt.pump() { /* d.outcome の応答（スクリプト）を再生する */ }
//! ```
//!
//! # 規約との対応
//! - 規約 1（単一スレッド）: `GhostRuntime` が `!Send` で、呼び出しは `&mut self`
//! - 規約 3（再入禁止）: `Host` のメソッドは `&self` しか持たず、ランタイムに触れない。
//!   `Host` の中から `GhostRuntime` を呼べるような共有（Rc<RefCell<..>> 等）はしないこと
//! - 規約 4（post）: ホスト側では post をコピーしてキューに積むだけ。`on_post` は pump の中で呼ばれる
//! - 規約 5（unload）: Drop で、shutdown → 未配送の破棄 → unload → ライブラリ解放 の順に行う

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
    SHIORI_500_INTERNAL_SERVER_ERROR, SHIORI_ABI_VERSION, SHIORI_E_INVALID_ARG, SHIORI_E_PANIC,
    SHIORI_E_REENTRANT, SHIORI_E_SHUTDOWN, SHIORI_E_UNSUPPORTED, SHIORI_LOG_DEBUG,
    SHIORI_LOG_ERROR, SHIORI_LOG_INFO, SHIORI_LOG_WARN,
};
use shiori4::shiori4_abi::{
    ShioriGetApiFn, ShioriHost, ShioriHostBox, ShioriInstance, SHIORI_GET_API_SYMBOL,
};

mod v3;
mod discover;
mod dist;
#[cfg(test)]
mod test_dll;

// ============================================================
// ホスト実装者が実装する trait
// ============================================================

/// ホスト（ベースウェア）の実装者が実装する trait
///
/// すべて `&self` で呼ばれる。呼ばれるスレッドはメソッドごとに異なるので注意:
/// - `property` / `request` / `log` : SHIORI のスレッド（= GhostRuntime を使っているスレッド）
/// - `accept_post` / `wake`         : 任意のスレッド（SHIORI のワーカーからも呼ばれる）
/// - `on_post`                      : GhostRuntime::pump を呼んだスレッド
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
    /// 既定では `Command` ヘッダがあれば受け付ける。false なら SHIORI に 400 を返す
    fn accept_post(&self, req: &Headers<'_>) -> bool {
        req.get(host_protocol::HEADER_COMMAND).is_some()
    }

    /// 受け付けた post を処理する（pump の中で呼ばれる）
    /// 結果を返すときは `post.into_completer()` を使う。Tag のない post では完了通知は捨てられる
    fn on_post(&self, post: Post);

    /// SHIORI からのログ
    fn log(&self, _level: i32, _msg: &str) {}

    /// キューに何かが積まれた（任意のスレッドから呼ばれる）
    /// イベントループに「GhostRuntime::pump を呼んで」と依頼する
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
}

impl<H: Host> ShioriHost for Bridge<H> {
    fn request(&self, req: &[u8]) -> ShioriResponse {
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
    /// ゴーストディレクトリが UTF-8 で表せない
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
            Self::InvalidPath => f.write_str("ghost directory is not valid UTF-8"),
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
        let dir = ghost_dir.as_ref().to_str().ok_or(LoadError::InvalidPath)?.to_owned();

        // SAFETY: ライブラリの初期化コードが実行される。信頼できる SHIORI だけを読み込むこと
        let lib = unsafe { libloading::Library::new(lib_path) }.map_err(LoadError::Library)?;
        let get: ShioriGetApiFn = unsafe { lib.get::<ShioriGetApiFn>(SHIORI_GET_API_SYMBOL) }
            .map(|s| *s)
            .map_err(|_| LoadError::NotShiori4)?;

        let p = unsafe { get(SHIORI_ABI_VERSION) };
        // SAFETY: lib は GhostRuntime の最後のフィールドとして保持し、instance より後に解放する。
        // 'static はこの構造体の外へ漏らさない
        let api: &'static ShioriApi =
            unsafe { ShioriApi::from_raw(p, SHIORI_ABI_VERSION) }.map_err(LoadError::Unsupported)?;

        Self::with_api(api, &dir, host, Some(lib))
    }

    /// 同じプロセス内のテーブル（ShioriApi::new で作った static）を使う。テスト用
    pub fn from_static(api: &'static ShioriApi, ghost_dir: &str, host: H) -> Result<Self, LoadError> {
        if api.abi_version() < SHIORI_ABI_VERSION {
            return Err(LoadError::Unsupported(SHIORI_E_UNSUPPORTED));
        }
        Self::with_api(api, ghost_dir, host, None)
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
        });

        let inst = api.load(&host_box, dir).ok_or(LoadError::LoadFailed)?;
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
        let r = self.instance.request(headers);
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
        let r = self.instance.notify(headers);
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
        // 3. unload（SHIORI は自分のスレッドを止めてから返る）
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

    struct TestGhost {
        props: Properties,
        poster: Option<ShioriHostPoster>,
        pending: PendingTags<&'static str>,
    }

    impl Ghost for TestGhost {
        fn load(host: Option<ShioriHostHandle>, _dir: &str) -> Result<Self, ShioriStatus> {
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
        GhostRuntime::from_static(&API, "/ghost", TestHost { woken: AtomicUsize::new(0) })
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
        };
        let r = ShioriHost::request(&b, b"ID: unknown\r\n");
        assert_eq!(r.status, 400);
        let _ = r.into_owned();
        // Command のない post は 400
        assert_eq!(ShioriHost::post(&b, b"Tag: 1\r\n"), SHIORI_400_BAD_REQUEST);
    }
}
