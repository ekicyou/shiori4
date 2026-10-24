//! shiori4_host — shiori4 のホスト（ベースウェア）側の高水準層
//!
//! 教科書: <https://github.com/ekicyou/shiori4/blob/main/doc/textbook/README.md>
//!
//! `Host` trait を実装すれば、SHIORI（v4）を読み込んで動かすホストが作れる。
//!
//! - SHIORI の DLL / .so / .dylib を読み込み、ライフタイムを一括管理する（`GhostRuntime`）
//! - SHIORI からの同期の要求（プロパティの取得など）を、原文のまま `Host::request` に渡す
//! - 任意のスレッドから来る post を原文のままキューに積み、イベントループで `Host::on_post` に渡す
//! - 非同期処理の完了や任意のイベントを、規約どおりのタイミングで SHIORI に届ける（`pump`）
//! - unload 時に未配送のイベントを破棄し、以後の post を SHIORI_E_SHUTDOWN にする
//!
//! 芯は要求・post・完了通知の中身を解釈しない。運ぶ中身の語（`getproperty`・post の `Command`・`Tag`・
//! `OnHostComplete`・拒否の返し方など）は、ホスト側プロトコル仕様
//! <https://github.com/ekicyou/shiori4/blob/main/doc/host-protocol.md> が定める。
//! 解析と組み立ては便利関数のモジュール `protocol`（`shiori4::protocol` の再エクスポート）で行う。
//! 下の例では `getproperty` に `protocol::getproperty_response` で答え、post の完了通知を
//! `protocol::host_complete` で組み立てて、post に付いてきた `Completer` で返す。
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
//!     fn name(&self) -> &str {
//!         "MyBase"
//!     }
//!     fn request(&self, req: &str) -> ShioriResponse {
//!         let h = protocol::Headers::parse(req);
//!         match (h.id(), h.reference(0)) {
//!             (Some(protocol::ID_GETPROPERTY), Some("baseware.name")) => protocol::getproperty_response(Some("MyBase")),
//!             (Some(protocol::ID_GETPROPERTY), _) => protocol::getproperty_response(None),
//!             _ => ShioriResponse::bad_request(),
//!         }
//!     }
//!     fn on_post(&self, post: Post) {
//!         // 重い処理は別スレッドへ。Completer は Send なので持ち出せる
//!         let (text, done) = post.into_parts();
//!         std::thread::spawn(move || {
//!             // Tag の無い post は結果不要（None）
//!             if let Some(h) = protocol::host_complete(&text, "MyBase", SHIORI_200_OK, &[protocol::OUTCOME_COMPLETED], None) {
//!                 done.complete_with(Delivery::Request, &h);
//!             }
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
//! // 実際は文書の第 2.2 節の必須ヘッダ（Sender など）も付ける
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
//!   間だけ `Host::request` に渡す。外れた呼び出しには SHIORI_E_WRONG_THREAD を返す
//! - 規約 3（再入禁止）: `Host` のメソッドは `&self` しか持たず、ランタイムに触れない。
//!   `Host` の中から `GhostRuntime` を呼べるような共有（Rc<RefCell<..>> 等）はしないこと
//! - 規約 4（post・log）: ホスト側では post をコピーしてキューに積むだけ。`on_post` は pump の中で
//!   呼ばれる。`Host::log` は任意のスレッドから呼ばれる
//! - 規約 5（unload）: Drop で、shutdown → 未配送の破棄 → unload → ライブラリ解放 の順に行う。
//!   SHIORI に join は求めない。unload が返った後、SHIORI はホストを呼ばない
//! - load の規約（dir）: dir を検査し、絶対パスでないか UTF-8 で表せなければ `LoadError::InvalidPath`。
//!   通るときは見た目だけ正規化し（OS の区切り・末尾に区切り）、SHIORI に渡す
//! - Charset: v4 は UTF-8 固定なので、芯が作る応答と便利関数が作るヘッダに Charset を付けない
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
//!
//! # DLL の選び方と読み方
//! - 配布の決まり: `shiori_file_candidates` が descript.txt の `shiori,` の値から探すファイル名を順に並べ、
//!   `machine_matches` がファイルの機械の種類を確かめる（どちらも DLL を読まない）。
//!   今のプロセスの OS・arch は `TargetOs::CURRENT`・`TargetArch::CURRENT`。決まりの本文は C ヘッダの「配布の決まり」
//! - 発見の順と許可: `load_ghost` は `shiori_get_api`（4 → 3）→ `loadu` → `load` の順に口を探し、
//!   `LoadedGhost` で結果を返す。`load_ghost_with` は許可する口（`EntryKind`）を選べる
//! - 読み終えたライブラリからの入口: `GhostRuntime::from_library`。`NotShiori4`・`Unsupported`・`InvalidPath`
//!   では `FromLibraryError` がライブラリを返すので、別の版で読み直せる
//! - v3 互換モードの口: `V3Runtime`（SHIORI/3.0 の全文を文字列で渡し、応答の全文を受け取る。post は無い）
//!
//! ```
//! use shiori4_host::{shiori_file_candidates, TargetArch, TargetOs};
//!
//! assert_eq!(
//!     shiori_file_candidates("pasta.dll", TargetOs::Windows, TargetArch::Arm64, true),
//!     ["pasta.arm64.dll", "pasta.x64.dll", "pasta.dll"],
//! );
//! ```
//!
//! ```no_run
//! use shiori4_host::*;
//! # struct MyHost;
//! # impl Host for MyHost { fn name(&self) -> &str { "MyBase" } fn on_post(&self, _post: Post) {} }
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let dir = std::env::current_exe()?.parent().expect("exe dir").join("ghost").join("master");
//! let (os, arch) = (TargetOs::CURRENT.expect("os"), TargetArch::CURRENT.expect("arch"));
//! // 候補のうち、在って機械の種類が合う最初のファイルを選ぶ
//! let path = shiori_file_candidates("shiori.dll", os, arch, false)
//!     .into_iter()
//!     .map(|name| dir.join(name))
//!     .find(|p| machine_matches(p, arch).unwrap_or(false))
//!     .ok_or("no shiori")?;
//! match load_ghost_with(&path, &dir, MyHost, &[EntryKind::V4, EntryKind::V3])? {
//!     LoadedGhost::V4(mut rt) => { rt.request("ID: OnBoot\r\n").map_err(|s| format!("ABI error: {s}"))?; }
//!     LoadedGhost::V3(mut rt) => {
//!         let res = rt.request("GET SHIORI/3.0\r\nID: OnBoot\r\n\r\n").map_err(|s| format!("ABI error: {s}"))?;
//!         # drop(res);
//!     }
//!     LoadedGhost::Legacy { entry, .. } => println!("従来の口 {entry:?} はこのクレートでは読まない"),
//! }
//! # Ok(())
//! # }
//!
//! // GhostRuntime::from_library が Unsupported で返したライブラリを、v3 互換モードで読み直す
//! fn retry_v3(e: FromLibraryError, dir: &std::path::Path) -> Result<V3Runtime, LoadError> {
//!     match e.into_parts() {
//!         (LoadError::Unsupported(_), Some(lib)) => Ok(V3Runtime::from_library(lib, dir)?),
//!         (e, _) => Err(e),
//!     }
//! }
//! ```

use std::{
    ffi::OsStr,
    fmt,
    mem::ManuallyDrop,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

pub use shiori4;
// from_library が受ける Library の型を、利用者が版を合わせずに使えるようにする
pub use libloading;
pub use shiori4::protocol;
pub use shiori4::shiori4_abi::{
    SHIORI_200_OK, SHIORI_204_NO_CONTENT, SHIORI_400_BAD_REQUEST, SHIORI_500_INTERNAL_SERVER_ERROR,
    SHIORI_ABI_VERSION, SHIORI_E_INVALID_ARG, SHIORI_E_INVALID_UTF8, SHIORI_E_PANIC,
    SHIORI_E_REENTRANT, SHIORI_E_SHUTDOWN, SHIORI_E_UNSUPPORTED, SHIORI_E_WRONG_THREAD,
    SHIORI_LOG_DEBUG, SHIORI_LOG_ERROR, SHIORI_LOG_INFO, SHIORI_LOG_WARN, ShioriApi,
    ShioriOwnedBytes, ShioriOwnedResponse, ShioriResponse, ShioriStatus, shiori_is_abi_error,
    shiori_is_success,
};
use shiori4::shiori4_abi::{
    SHIORI_GET_API_SYMBOL, ShioriBytes, ShioriGetApiFn, ShioriHost, ShioriHostBox, ShioriInstance,
};

mod v3;
pub use v3::V3Runtime;
mod discover;
pub use discover::{EntryKind, LoadedGhost, load_ghost, load_ghost_with};
mod dist;
pub use dist::{TargetArch, TargetOs, machine_matches, shiori_file_candidates};
#[cfg(test)]
mod test_dll;

// ============================================================
// ホスト実装者が実装する trait
// ============================================================

/// ホスト（ベースウェア）の実装者が実装する trait
///
/// すべて `&self` で呼ばれる。呼ばれるスレッドはメソッドごとに異なるので注意:
/// - `request`                      : SHIORI のスレッド（= GhostRuntime を使っているスレッド）
/// - `accept_post` / `wake` / `log` : 任意のスレッド（SHIORI のワーカーからも呼ばれる）
/// - `on_post`                      : GhostRuntime::pump を呼んだスレッド
///
/// 任意のスレッドから呼ばれる `accept_post`・`wake`・`log` は、`GhostRuntime` を Drop するスレッドを
/// 同期で待たないこと（規約 4: コピーしてすぐ返る。待つと unload の門の待ちと互いに待ち合って止まる）
pub trait Host: Send + Sync + 'static {
    /// ホストの名前（要求の `Sender` に載せる名前。例 "areka"）。芯は使わない
    fn name(&self) -> &str;

    /// 同期の要求（SHIORI のスレッド、規約 2 の期間）に答える。req は原文（ヘッダ部分）。
    /// 戻り値はそのまま SHIORI に返る。既定はどの要求にも 400（このホストでは扱えない）
    ///
    /// # 例: getproperty・setproperty に答え、拒否は 400 ＋ `Reason: forbidden`（文書 3.1・3.2・3.4・7.1）
    ///
    /// 読みは 200 ＋ `Value`（在る）か 204（無い）、書き込みは 204（書き込んだ）か 400（不正）で答える。
    /// 書き込ませないプロパティは、「無い」の 204 や「不正」の Reason の無い 400 と分けて、
    /// 400 ＋ `Reason: forbidden` で拒否する。この例のホストは `baseware.` で始まる名前を書き込ませない。
    /// この例は DLL なしに [`GhostRuntime::from_static`] で往復を実行して確かめている。
    ///
    /// ```
    /// use shiori4_host::*;
    /// use std::collections::HashMap;
    /// use std::sync::Mutex;
    /// # use shiori4::protocol::Properties;
    /// # struct PropGhost {
    /// #     props: Properties,
    /// # }
    /// # impl shiori4::Ghost for PropGhost {
    /// #     fn load(host: Option<shiori4::ShioriHostHandle>, _dir: &str) -> Result<Self, ShioriStatus> {
    /// #         Ok(Self { props: Properties::new(host) })
    /// #     }
    /// #     fn request(&mut self, _req: &str) -> ShioriResponse {
    /// #         // Properties で読み書きを試し、結果を Value に並べて返す
    /// #         let p = &mut self.props;
    /// #         let a = format!("{:?}", p.get("baseware.name"));
    /// #         let b = format!("{:?}", p.set_on_host("MyBase.Volume", "3"));
    /// #         let c = format!("{:?}", p.get("mybase.volume"));
    /// #         let d = format!("{:?}", p.set_on_host("baseware.name", "x"));
    /// #         let e = format!("{:?}", p.get("no.such"));
    /// #         let v = [a, b, c, d, e].join(" ");
    /// #         protocol::HeaderWriter::new().push(protocol::HEADER_VALUE, &v).into_response()
    /// #     }
    /// # }
    /// # static API: ShioriApi = ShioriApi::new::<shiori4::Adapter<PropGhost>, { SHIORI_ABI_VERSION }>();
    ///
    /// struct PropHost {
    ///     /// 名前は小文字にして持つ（名前の大文字小文字は区別しない。文書 3.4）
    ///     props: Mutex<HashMap<String, String>>,
    /// }
    ///
    /// impl Host for PropHost {
    ///     fn name(&self) -> &str {
    ///         "MyBase"
    ///     }
    ///     fn request(&self, req: &str) -> ShioriResponse {
    ///         let h = protocol::Headers::parse(req);
    ///         let name = h.reference(0).map(str::to_lowercase);
    ///         let mut props = self.props.lock().unwrap();
    ///         match (h.id(), name, h.reference(1)) {
    ///             // 在れば 200 ＋ Value、無ければ 204（文書 3.1）
    ///             (Some(protocol::ID_GETPROPERTY), Some(n), _) => {
    ///                 protocol::getproperty_response(props.get(&n).map(String::as_str))
    ///             }
    ///             // 書き込ませない名前は拒否（文書 7.1）
    ///             (Some(protocol::ID_SETPROPERTY), Some(n), Some(_)) if n.starts_with("baseware.") => {
    ///                 protocol::rejection_response(Some(protocol::REASON_FORBIDDEN))
    ///             }
    ///             // 書き込んだら 204（文書 3.2）
    ///             (Some(protocol::ID_SETPROPERTY), Some(n), Some(v)) => {
    ///                 props.insert(n, v.to_owned());
    ///                 ShioriResponse::no_content()
    ///             }
    ///             // 名前や値の欠けた要求・知らない要求は、Reason の無い 400（不正。文書 7.3）
    ///             _ => protocol::rejection_response(None),
    ///         }
    ///     }
    ///     fn on_post(&self, _post: Post) {}
    /// }
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let props = HashMap::from([("baseware.name".to_owned(), "MyBase".to_owned())]);
    /// let host = PropHost { props: Mutex::new(props) };
    /// // dir は絶対パス（load の規約 1）
    /// let dir = if cfg!(windows) { r"C:\ghost" } else { "/ghost" };
    /// let mut rt = GhostRuntime::from_static(&API, dir, host)?;
    /// // 例の偽の SHIORI は、shiori4::protocol::Properties で読み書きを試した結果を Value に並べて返す
    /// let r = rt.request("ID: OnBoot\r\n").map_err(|s| format!("ABI error: {s}"))?;
    /// assert_eq!(
    ///     r.body.as_str()?,
    ///     "Value: Ready(\"MyBase\") Ok(()) Ready(\"3\") Err(Rejected(Forbidden)) Missing\r\n"
    /// );
    /// # Ok(())
    /// # }
    /// ```
    fn request(&self, _req: &str) -> ShioriResponse {
        ShioriResponse::bad_request()
    }

    /// post を受け付けるかを、その場で判定する（任意のスレッドから呼ばれる。軽く保つこと）。msg は原文
    /// `GhostRuntime` を Drop するスレッドを同期で待たないこと（規約 4）
    /// 既定ではすべて受け付ける。false なら SHIORI に 400 を返す
    fn accept_post(&self, _msg: &str) -> bool {
        true
    }

    /// 受け付けた post を処理する（pump の中で呼ばれる）
    /// 結果を返すときは `post.into_completer()` の `complete_with` を使う（全文はホストが組み立てる）
    fn on_post(&self, post: Post);

    /// SHIORI からのログ（任意のスレッドから呼ばれる。規約 4）
    /// `GhostRuntime` を Drop するスレッドを同期で待たないこと（UI スレッドへの同期の送信など）
    fn log(&self, _level: i32, _msg: &str) {}

    /// キューに何かが積まれた（任意のスレッドから呼ばれる）
    /// イベントループに「GhostRuntime::pump を呼んで」と依頼する。依頼は非同期で行い、
    /// `GhostRuntime` を Drop するスレッドを同期で待たないこと（規約 4）
    ///
    /// # 例: std の channel で待つイベントループ（スレッド規約 1・4）
    ///
    /// `wake` は channel に「pump して」を積むだけで、待たずに返る。`GhostRuntime` を作ったスレッドが
    /// その channel を待ち、受けたら [`GhostRuntime::pump`] を呼ぶ。`on_post` はその pump の中で呼ばれる。
    /// この例は DLL なしに [`GhostRuntime::from_static`] で往復を実行して確かめている。
    ///
    /// ```
    /// use shiori4_host::*;
    /// use std::sync::mpsc::{channel, Sender};
    /// use std::time::Duration;
    /// # struct WorkerGhost {
    /// #     poster: shiori4::ShioriHostPoster,
    /// # }
    /// # impl shiori4::Ghost for WorkerGhost {
    /// #     fn load(host: Option<shiori4::ShioriHostHandle>, _dir: &str) -> Result<Self, ShioriStatus> {
    /// #         Ok(Self { poster: host.ok_or(SHIORI_E_UNSUPPORTED)?.poster() })
    /// #     }
    /// #     fn request(&mut self, _req: &str) -> ShioriResponse {
    /// #         let p = self.poster.clone();
    /// #         std::thread::spawn(move || p.post_str(&protocol::send_post("\\0hi\\e", &[], None)));
    /// #         ShioriResponse::no_content()
    /// #     }
    /// # }
    /// # static API: ShioriApi = ShioriApi::new::<shiori4::Adapter<WorkerGhost>, { SHIORI_ABI_VERSION }>();
    ///
    /// struct ChannelHost {
    ///     wake: Sender<()>,
    ///     scripts: Sender<String>,
    /// }
    ///
    /// impl Host for ChannelHost {
    ///     fn name(&self) -> &str {
    ///         "MyBase"
    ///     }
    ///     // pump を呼んだスレッドで呼ばれる。この例では台本を再生する代わりに送り出す
    ///     fn on_post(&self, post: Post) {
    ///         if let Some(s) = protocol::Headers::parse(post.text()).get(protocol::HEADER_SCRIPT) {
    ///             let _ = self.scripts.send(s.to_owned());
    ///         }
    ///     }
    ///     // 任意のスレッドから呼ばれる。積むだけで待たない（スレッド規約 4）
    ///     fn wake(&self) {
    ///         let _ = self.wake.send(());
    ///     }
    /// }
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let (wake, wake_rx) = channel();
    /// let (scripts, scripts_rx) = channel();
    /// // dir は絶対パス（load の規約 1）
    /// let dir = if cfg!(windows) { r"C:\ghost" } else { "/ghost" };
    /// let mut rt = GhostRuntime::from_static(&API, dir, ChannelHost { wake, scripts })?;
    /// // 例の偽の SHIORI は、OnBoot でワーカーのスレッドを起こし、そこから SEND を post する
    /// rt.request("ID: OnBoot\r\n").map_err(|s| format!("ABI error: {s}"))?;
    /// // イベントループの 1 回り: wake を待ち、rt を作ったこのスレッドで pump する（スレッド規約 1）
    /// wake_rx.recv_timeout(Duration::from_secs(5))?;
    /// rt.pump();
    /// assert_eq!(scripts_rx.try_recv()?, "\\0hi\\e");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # 例: Win32 のメッセージの輪（スレッド規約 1・4）
    ///
    /// `wake` は `PostThreadMessageW` で UI スレッドにメッセージを積むだけで、待たずに返る。
    /// UI スレッドのメッセージの輪がそのメッセージを受けたら [`GhostRuntime::pump`] を呼ぶ。
    /// 使う Win32 の関数は、外部の crate を足さずに例の中で宣言する。
    /// この例は DLL と Windows を要するので、Windows でビルドだけを検査する（実行しない）。ほかの OS では空の例になる。
    ///
    /// ```no_run
    /// # #[cfg(windows)]
    /// # mod example {
    /// use shiori4_host::*;
    /// use std::ffi::c_void;
    ///
    /// #[repr(C)]
    /// struct Point {
    ///     x: i32,
    ///     y: i32,
    /// }
    ///
    /// #[repr(C)]
    /// struct Msg {
    ///     hwnd: *mut c_void,
    ///     message: u32,
    ///     w_param: usize,
    ///     l_param: isize,
    ///     time: u32,
    ///     pt: Point,
    /// }
    ///
    /// #[link(name = "user32")]
    /// unsafe extern "system" {
    ///     fn PostThreadMessageW(thread: u32, msg: u32, w_param: usize, l_param: isize) -> i32;
    ///     fn GetMessageW(msg: *mut Msg, hwnd: *mut c_void, min: u32, max: u32) -> i32;
    ///     fn TranslateMessage(msg: *const Msg) -> i32;
    ///     fn DispatchMessageW(msg: *const Msg) -> isize;
    /// }
    ///
    /// #[link(name = "kernel32")]
    /// unsafe extern "system" {
    ///     fn GetCurrentThreadId() -> u32;
    /// }
    ///
    /// /// pump を頼むメッセージ（WM_APP）
    /// const WM_APP_PUMP: u32 = 0x8000;
    ///
    /// struct Win32Host {
    ///     ui_thread: u32,
    /// }
    ///
    /// impl Host for Win32Host {
    ///     fn name(&self) -> &str {
    ///         "MyBase"
    ///     }
    ///     fn on_post(&self, _post: Post) { /* 台本を再生する */ }
    ///     // 任意のスレッドから呼ばれる。積むだけで待たない（スレッド規約 4）
    ///     fn wake(&self) {
    ///         unsafe { PostThreadMessageW(self.ui_thread, WM_APP_PUMP, 0, 0) };
    ///     }
    /// }
    ///
    /// pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let dir = std::env::current_exe()?.parent().expect("exe dir").join("ghost").join("master");
    /// // rt を作るこのスレッドが UI スレッドで、メッセージの輪を回す（スレッド規約 1）。
    /// // 実際はメッセージの列を先に作っておく（PeekMessageW）。列が無いうちの PostThreadMessageW は失敗する
    /// let host = Win32Host { ui_thread: unsafe { GetCurrentThreadId() } };
    /// let mut rt = GhostRuntime::load(dir.join("shiori.dll"), &dir, host)?;
    /// rt.request("ID: OnBoot\r\n").map_err(|s| format!("ABI error: {s}"))?;
    /// let mut msg: Msg = unsafe { std::mem::zeroed() };
    /// // GetMessageW は WM_QUIT で 0、失敗で -1 を返す
    /// while unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } > 0 {
    ///     if msg.hwnd.is_null() && msg.message == WM_APP_PUMP {
    ///         for _d in rt.pump() { /* _d.outcome の応答（スクリプト）を再生する */ }
    ///         continue;
    ///     }
    ///     unsafe {
    ///         TranslateMessage(&msg);
    ///         DispatchMessageW(&msg);
    ///     }
    /// }
    /// Ok(())
    /// }
    /// # }
    /// # #[cfg(windows)]
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> { example::main() }
    /// # #[cfg(not(windows))]
    /// # fn main() {}
    /// ```
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
        Self {
            delivery: Delivery::Request,
            headers: headers.into(),
        }
    }

    pub fn notify(headers: impl Into<String>) -> Self {
        Self {
            delivery: Delivery::Notify,
            headers: headers.into(),
        }
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
        f.debug_struct("EventSender")
            .field("closed", &self.is_closed())
            .finish()
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
        Self {
            text,
            completer: Completer { sender },
        }
    }

    /// ヘッダ部分の原文
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn into_completer(self) -> Completer {
        self.completer
    }

    /// 原文と Completer に分ける（原文を別スレッドへ持ち出す場合に使う）
    pub fn into_parts(self) -> (String, Completer) {
        (self.text, self.completer)
    }
}

/// post したインスタンスへ完了通知を届ける口（Send。別スレッドへ持ち出せる。1 回だけ使える）
/// 使わずに捨てた場合、完了通知は届かない
///
/// # 例: SEND の completed・interrupted と、nobreak で待たせた SEND の 204（post の完了通知 3・4、文書 5.1・5.4・6.3・6.8）
///
/// 再生中の SEND と、`Option: nobreak` で待たせた SEND を、原文と `Completer` の組で持つ。
/// - 新しい SEND は既定で今の台本に割り込み、切られた側には 200 ＋ `interrupted` を返す（文書 5.1・5.4）
/// - 最後まで再生したら 200 ＋ `completed` を返し、待たせた SEND があれば次を再生する（文書 6.3）
/// - 終了では、待たせた SEND を一度も再生せずに捨てて 204 を返し、それを終了のイベントより前に届ける（文書 6.3・6.8）
/// - `Tag` の無い post には返さない（`protocol::host_complete` が None。post の完了通知 4）
///
/// 完了は `complete_with` でキューに積まれ、pump が SHIORI に届ける（post の完了通知 3）。`on_post` の中で
/// 積んだ完了は、その同じ pump の中で届く。
/// この例は DLL なしに [`GhostRuntime::from_static`] で往復を実行して確かめている。
///
/// ```
/// use shiori4_host::*;
/// use std::sync::Mutex;
/// # struct TalkGhost {
/// #     poster: shiori4::ShioriHostPoster,
/// # }
/// # impl shiori4::Ghost for TalkGhost {
/// #     fn load(host: Option<shiori4::ShioriHostHandle>, _dir: &str) -> Result<Self, ShioriStatus> {
/// #         Ok(Self { poster: host.ok_or(SHIORI_E_UNSUPPORTED)?.poster() })
/// #     }
/// #     fn request(&mut self, req: &str) -> ShioriResponse {
/// #         let h = protocol::Headers::parse(req);
/// #         match h.id() {
/// #             Some("OnTalk") => {
/// #                 let options: Vec<&str> = h.reference(2).into_iter().collect();
/// #                 let post = protocol::send_post(h.reference(1).unwrap_or(""), &options, h.reference(0));
/// #                 assert_eq!(self.poster.post_str(&post), SHIORI_204_NO_CONTENT);
/// #                 ShioriResponse::no_content()
/// #             }
/// #             Some(protocol::ID_HOST_COMPLETE) => {
/// #                 let c = protocol::HostComplete::parse(&h).expect("OnHostComplete");
/// #                 let v = match c.outcome {
/// #                     Some(o) => format!("{} {} {o}", c.tag, c.status),
/// #                     None => format!("{} {}", c.tag, c.status),
/// #                 };
/// #                 protocol::HeaderWriter::new().push(protocol::HEADER_VALUE, &v).into_response()
/// #             }
/// #             _ => ShioriResponse::no_content(),
/// #         }
/// #     }
/// # }
/// # static API: ShioriApi = ShioriApi::new::<shiori4::Adapter<TalkGhost>, { SHIORI_ABI_VERSION }>();
///
/// /// 再生中の SEND と、nobreak で待たせた SEND（原文と Completer）
/// #[derive(Default)]
/// struct Balloon {
///     playing: Option<(String, Completer)>,
///     waiting: Vec<(String, Completer)>,
/// }
///
/// #[derive(Default)]
/// struct PlayerHost {
///     balloon: Mutex<Balloon>,
/// }
///
/// impl PlayerHost {
///     /// 完了を返す。Tag の無い post は結果が不要なので何もしない（post の完了通知 4）
///     fn complete(&self, (text, done): (String, Completer), status: ShioriStatus, refs: &[&str]) {
///         if let Some(h) = protocol::host_complete(&text, self.name(), status, refs, None) {
///             done.complete_with(Delivery::Request, &h);
///         }
///     }
///
///     /// 台本を最後まで再生した（実際は再生の終わりから呼ぶ）
///     fn finish(&self) {
///         let mut b = self.balloon.lock().unwrap();
///         if let Some(p) = b.playing.take() {
///             self.complete(p, SHIORI_200_OK, &[protocol::OUTCOME_COMPLETED]);
///         }
///         if !b.waiting.is_empty() {
///             b.playing = Some(b.waiting.remove(0));
///         }
///     }
///
///     /// 終了: 待たせた SEND を一度も再生せずに捨てる（文書 6.3）
///     fn close(&self) {
///         let waiting = std::mem::take(&mut self.balloon.lock().unwrap().waiting);
///         for w in waiting {
///             self.complete(w, SHIORI_204_NO_CONTENT, &[]);
///         }
///     }
/// }
///
/// impl Host for PlayerHost {
///     fn name(&self) -> &str {
///         "MyBase"
///     }
///     fn on_post(&self, post: Post) {
///         let (text, done) = post.into_parts();
///         let h = protocol::Headers::parse(&text);
///         let send = h.get(protocol::HEADER_COMMAND) == Some(protocol::COMMAND_SEND);
///         let nobreak = h
///             .get(protocol::HEADER_OPTION)
///             .is_some_and(|o| o.split(',').any(|w| w.trim() == protocol::OPTION_NOBREAK));
///         if !send {
///             // この例のホストは SEND だけを扱う。ほかは Reason の無い 400（不正。文書 7.3）
///             return self.complete((text, done), SHIORI_400_BAD_REQUEST, &[]);
///         }
///         let mut b = self.balloon.lock().unwrap();
///         if nobreak && b.playing.is_some() {
///             b.waiting.push((text, done));
///         } else if let Some(cut) = b.playing.replace((text, done)) {
///             self.complete(cut, SHIORI_200_OK, &[protocol::OUTCOME_INTERRUPTED]);
///         }
///     }
/// }
///
/// /// 例の偽の SHIORI は OnTalk で、Reference0 を Tag に、Reference1 を台本に、Reference2 を Option にして SEND を post する
/// fn talk(rt: &mut GhostRuntime<PlayerHost>, tag: &str, script: &str, option: Option<&str>) {
///     let option = option.map(|o| format!("Reference2: {o}\r\n")).unwrap_or_default();
///     let req = format!("ID: OnTalk\r\nReference0: {tag}\r\nReference1: {script}\r\n{option}");
///     rt.request(&req).expect("abi");
/// }
///
/// /// pump で届けた OnHostComplete への応答（例の偽の SHIORI は `Value: <Tag> <Status> <Reference1>` を返す）
/// fn pump_values(rt: &mut GhostRuntime<PlayerHost>) -> Vec<String> {
///     let value = |d: Delivered| match d.outcome {
///         Outcome::Response(r) => {
///             let text = r.body.as_str().expect("utf-8").to_owned();
///             protocol::Headers::parse(&text).get(protocol::HEADER_VALUE).expect("Value").to_owned()
///         }
///         o => panic!("{o:?}"),
///     };
///     rt.pump().into_iter().map(value).collect::<Vec<_>>()
/// }
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // dir は絶対パス（load の規約 1）
/// let dir = if cfg!(windows) { r"C:\ghost" } else { "/ghost" };
/// let mut rt = GhostRuntime::from_static(&API, dir, PlayerHost::default())?;
///
/// // 2 が 1 に割り込む
/// talk(&mut rt, "1", "\\0a\\e", None);
/// talk(&mut rt, "2", "\\0b\\e", None);
/// assert_eq!(pump_values(&mut rt), ["1 200 interrupted"]);
/// rt.host().finish();
/// assert_eq!(pump_values(&mut rt), ["2 200 completed"]);
///
/// // 4 は nobreak なので 3 の再生の終わりを待つ。待つ間に終了が来たら捨てて 204
/// talk(&mut rt, "3", "\\0c\\e", None);
/// talk(&mut rt, "4", "\\0d\\e", Some(protocol::OPTION_NOBREAK));
/// assert!(pump_values(&mut rt).is_empty());
/// rt.host().close();
/// rt.host().finish(); // 再生中の 3 は捨てずに完了を 1 つ返す（文書 6.7）
/// assert_eq!(pump_values(&mut rt), ["4 204", "3 200 completed"]);
/// // 204 を届けてから終了のイベントを送る（文書 6.8）。実際は文書の第 2.2 節の必須ヘッダも付ける
/// rt.request("ID: OnClose\r\n").map_err(|s| format!("ABI error: {s}"))?;
/// # Ok(())
/// # }
/// ```
///
/// # 例: NOTIFY の完了（post の完了通知 3、文書 2.4・6.4）
///
/// NOTIFY の post は `on_post` で預かり、pump を呼んだイベントループがそのイベントを SHIORI に起こす
/// （`on_post` は `&self` しか持たず、ランタイムに触れない）。SHIORI が答えたら 200 を返す。
/// NOTIFY の完了は `Reference1` を持たない（文書 6.4）。
/// この例は DLL なしに [`GhostRuntime::from_static`] で往復を実行して確かめている。
///
/// ```
/// use shiori4_host::*;
/// use std::sync::Mutex;
/// # struct NotifyGhost {
/// #     poster: shiori4::ShioriHostPoster,
/// # }
/// # impl shiori4::Ghost for NotifyGhost {
/// #     fn load(host: Option<shiori4::ShioriHostHandle>, _dir: &str) -> Result<Self, ShioriStatus> {
/// #         Ok(Self { poster: host.ok_or(SHIORI_E_UNSUPPORTED)?.poster() })
/// #     }
/// #     fn request(&mut self, req: &str) -> ShioriResponse {
/// #         let h = protocol::Headers::parse(req);
/// #         let v = match h.id() {
/// #             Some("OnBoot") => {
/// #                 let post = protocol::notify_post("OnMyEvent", &["x"], &[], Some("7"));
/// #                 assert_eq!(self.poster.post_str(&post), SHIORI_204_NO_CONTENT);
/// #                 return ShioriResponse::no_content();
/// #             }
/// #             Some("OnMyEvent") => format!(
/// #                 "{} {}",
/// #                 h.get(protocol::HEADER_SENDER_TYPE).unwrap_or(""),
/// #                 h.reference(0).unwrap_or("")
/// #             ),
/// #             Some(protocol::ID_HOST_COMPLETE) => {
/// #                 let c = protocol::HostComplete::parse(&h).expect("OnHostComplete");
/// #                 format!("{} {} {} {:?}", c.tag, c.status, c.command, c.outcome)
/// #             }
/// #             _ => return ShioriResponse::no_content(),
/// #         };
/// #         protocol::HeaderWriter::new().push(protocol::HEADER_VALUE, &v).into_response()
/// #     }
/// # }
/// # static API: ShioriApi = ShioriApi::new::<shiori4::Adapter<NotifyGhost>, { SHIORI_ABI_VERSION }>();
///
/// #[derive(Default)]
/// struct NotifyHost {
///     /// イベントループで起こすまで預かる NOTIFY（原文と Completer）
///     raised: Mutex<Vec<(String, Completer)>>,
/// }
///
/// impl Host for NotifyHost {
///     fn name(&self) -> &str {
///         "MyBase"
///     }
///     fn on_post(&self, post: Post) {
///         let (text, done) = post.into_parts();
///         if protocol::Headers::parse(&text).get(protocol::HEADER_COMMAND) == Some(protocol::COMMAND_NOTIFY) {
///             self.raised.lock().unwrap().push((text, done));
///         } else if let Some(h) = protocol::host_complete(&text, self.name(), SHIORI_400_BAD_REQUEST, &[], None) {
///             // この例のホストは NOTIFY だけを扱う。ほかは Reason の無い 400（不正。文書 7.3）
///             done.complete_with(Delivery::Request, &h);
///         }
///     }
/// }
///
/// /// post の Event と Reference* から、起こすイベントのヘッダを組む（必須ヘッダは文書 2.2、
/// /// post から起きた要求の SenderType と SecurityLevel は文書 2.4）。Event が無ければ None
/// fn raise_headers(post: &str, sender: &str) -> Option<String> {
///     let h = protocol::Headers::parse(post);
///     let w = protocol::HeaderWriter::new()
///         .push(protocol::HEADER_SENDER, sender)
///         .push(protocol::HEADER_SENDER_TYPE, protocol::SENDER_TYPE_INTERNAL)
///         .push(protocol::HEADER_SECURITY_LEVEL, protocol::SECURITY_LEVEL_LOCAL)
///         .push(protocol::HEADER_SECURITY_ORIGIN, "null")
///         .push(protocol::HEADER_ID, h.get(protocol::HEADER_EVENT)?);
///     let refs = (0..).map_while(|i| h.reference(i).map(|r| (i, r)));
///     Some(refs.fold(w, |w, (i, r)| w.push(&format!("Reference{i}"), r)).finish())
/// }
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // dir は絶対パス（load の規約 1）
/// let dir = if cfg!(windows) { r"C:\ghost" } else { "/ghost" };
/// let mut rt = GhostRuntime::from_static(&API, dir, NotifyHost::default())?;
/// // 例の偽の SHIORI は、OnBoot で Tag: 7 の NOTIFY（Event: OnMyEvent・Reference0: x）を post する
/// rt.request("ID: OnBoot\r\n").map_err(|s| format!("ABI error: {s}"))?;
/// rt.pump(); // on_post が NOTIFY を預かる
///
/// let raised = std::mem::take(&mut *rt.host().raised.lock().unwrap());
/// for (text, done) in raised {
///     let status = match raise_headers(&text, "MyBase") {
///         Some(ev) => {
///             // 例の偽の SHIORI は、SenderType と Reference0 を Value にして答える
///             let r = rt.request(&ev).map_err(|s| format!("ABI error: {s}"))?;
///             assert_eq!(r.body.as_str()?, "Value: internal x\r\n");
///             SHIORI_200_OK // 起こし、SHIORI が答えた（文書 6.4）
///         }
///         None => SHIORI_400_BAD_REQUEST, // Event の無い NOTIFY は不正
///     };
///     if let Some(h) = protocol::host_complete(&text, "MyBase", status, &[], None) {
///         done.complete_with(Delivery::Request, &h);
///     }
/// }
///
/// // 完了は次の pump で届く。例の偽の SHIORI は Tag・Status・Reference0・Reference1 を Value にして答える
/// let ds = rt.pump();
/// match &ds[..] {
///     [Delivered { outcome: Outcome::Response(r), .. }] => {
///         assert_eq!(r.body.as_str()?, "Value: 7 200 NOTIFY None\r\n");
///     }
///     other => panic!("{other:?}"),
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Completer {
    sender: EventSender,
}

impl Completer {
    /// headers を何も足さず、何も検査せず、delivery で届ける。ランタイムが破棄された後は false
    pub fn complete_with(self, delivery: Delivery, headers: &str) -> bool {
        self.sender.send(Event {
            delivery,
            headers: headers.to_owned(),
        })
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
        self.host.request(text)
    }

    fn post(&self, msg: &[u8]) -> ShioriStatus {
        if self.shutdown.load(Ordering::Acquire) {
            return SHIORI_E_SHUTDOWN;
        }
        let Ok(text) = std::str::from_utf8(msg) else {
            return SHIORI_400_BAD_REQUEST;
        };
        if !self.host.accept_post(text) {
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

/// 読み込みの失敗。値ごとにホストがすべきことが違う
///
/// 次の扱いは、[`GhostRuntime::load`]・[`GhostRuntime::from_library`] で版を決めて読んだときのもの。
/// [`load_ghost`]・[`load_ghost_with`] の Err は発見の順を終えた結果なので、`NotShiori4`・`Unsupported` も
/// 読めなかったゴーストとして扱い、読み直さない（DLL の口と発見の順 2）。DLL を 1 回だけ読んで発見の順を
/// 辿るには、まず [`load_ghost`] を使う
///
/// - `Library`: ライブラリを開けなかった。機械の種類が合わないファイルなら次の候補へ進む
///   （配布の決まり 5。[`machine_matches`] で開く前に確かめられる）
/// - `NotShiori4`: `shiori_get_api` が無い。旧来の SHIORI/3.0 として読む候補に回す
///   （DLL の口と発見の順 2）。[`GhostRuntime::load`] はこのときライブラリを手放すので、
///   別の口で読み直すと DLL が 2 回読まれる。2 回読まずに済ませるには [`GhostRuntime::from_library`] を使う
/// - `Unsupported`: `shiori_get_api(4)` が NULL か、版に合わないテーブル（ABI 規約 1・版 4）。別の版（v3 互換モードの [`V3Runtime`]）で
///   読み直せる（版 4）
/// - `InvalidPath`: dir が絶対パスでないか UTF-8 で表せない（load の規約 1）。ライブラリを開く前に断るので、
///   dir を直して読み直せばよい
/// - `LoadFailed`: SHIORI の load が失敗した。読めなかったゴーストとして扱い、SHIORI/3.0 として読み直さない
///   （DLL の口と発見の順 2: 解決した口の load が失敗しても、ほかの口へは進まない）。理由は `Host::log` に
///   届いている（load の規約 3）。2 つ目のインスタンスの拒否もこれになり、ゴーストごとに別のプロセスで読むか、
///   拒否を受け入れる（load の規約 4）
///
/// # 例: 値ごとの扱い（DLL の口と発見の順 2・load の規約 3）
///
/// この例は DLL を要するので、ビルドだけを検査する（実行しない）。
///
/// ```no_run
/// use shiori4_host::*;
/// # struct MyHost;
/// # impl Host for MyHost { fn name(&self) -> &str { "MyBase" } fn on_post(&self, _post: Post) {} }
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let dir = std::env::current_exe()?.parent().expect("exe dir").join("ghost").join("master");
/// let path = dir.join("shiori.dll");
/// match GhostRuntime::load(&path, &dir, MyHost) {
///     Ok(mut rt) => { rt.request("ID: OnBoot\r\n").map_err(|s| format!("ABI error: {s}"))?; }
///     // 開けなかった。機械の種類が合わないなら次の候補へ（配布の決まり 5）
///     Err(LoadError::Library(e)) => println!("次の候補へ: {e}"),
///     // 旧来の SHIORI/3.0 として読む候補に回す（ここで DLL が 2 回目に読まれる）
///     Err(LoadError::NotShiori4) => println!("SHIORI/3.0 の読み込みへ回す"),
///     // 別の版で読み直せる
///     Err(LoadError::Unsupported(_)) => {
///         let mut rt = V3Runtime::load(&path, &dir)?;
///         let res = rt.request("GET SHIORI/3.0\r\nID: OnBoot\r\n\r\n").map_err(|s| format!("ABI error: {s}"))?;
///         # drop(res);
///     }
///     // ホストの不具合。dir を絶対パスに直す
///     Err(LoadError::InvalidPath) => return Err("ghost dir must be absolute".into()),
///     // 読めなかったゴースト。SHIORI/3.0 として読み直さない（理由は Host::log に届いている）
///     Err(LoadError::LoadFailed) => println!("読めなかったゴースト"),
/// }
/// # Ok(())
/// # }
/// ```
///
/// # 例: `InvalidPath` の枝（load の規約 1）
///
/// dir の検査はライブラリを開くより先なので、在らないライブラリでも相対パスの dir は `InvalidPath` になる。
/// この例は DLL なしに実行して確かめている。
///
/// ```
/// use shiori4_host::*;
/// # struct MyHost;
/// # impl Host for MyHost { fn name(&self) -> &str { "MyBase" } fn on_post(&self, _post: Post) {} }
///
/// let r = GhostRuntime::load("no-such-library", "ghost/master", MyHost);
/// assert!(matches!(r, Err(LoadError::InvalidPath)));
/// ```
#[derive(Debug)]
pub enum LoadError {
    /// ライブラリを開けなかった
    Library(libloading::Error),
    /// 許可された口が見つからない。版を決めて読む口では shiori_get_api がない（旧 SHIORI の可能性）。
    /// 発見の口（[`load_ghost_with`]）では、許可された口が DLL に 1 つも無い
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
            Self::NotShiori4 => f.write_str("no allowed SHIORI entry point found"),
            Self::Unsupported(s) => write!(f, "unsupported SHIORI ABI version (status {s})"),
            Self::InvalidPath => {
                f.write_str("ghost directory is not an absolute path or not valid UTF-8")
            }
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
    let mut s = if cfg!(windows) && !s.starts_with(r"\\?\") {
        s.replace('/', "\\")
    } else {
        s.to_owned()
    };
    let sep = std::path::MAIN_SEPARATOR;
    s.truncate(s.trim_end_matches(sep).len());
    s.push(sep);
    Ok(s)
}

/// 読み終えたライブラリの入口の共通部分: dir の検査 → `shiori_get_api` を探す → 版 version のテーブルの検査
///
/// 失敗は `InvalidPath`・`NotShiori4`・`Unsupported` だけ（どれも SHIORI の初期化の前。ライブラリを返してよい）
///
/// # Safety
/// 返るテーブルは lib が開いている間だけ有効。呼び出し側は lib をインスタンスより後に解放すること
unsafe fn library_entry(
    lib: &libloading::Library,
    ghost_dir: &Path,
    version: u32,
) -> Result<(String, &'static ShioriApi), LoadError> {
    let dir = normalize_dir(ghost_dir)?;
    let get: ShioriGetApiFn = *unsafe { lib.get::<ShioriGetApiFn>(SHIORI_GET_API_SYMBOL) }
        .map_err(|_| LoadError::NotShiori4)?;
    let p = unsafe { get(version) };
    let api = unsafe { ShioriApi::from_raw(p, version) }.map_err(LoadError::Unsupported)?;
    Ok((dir, api))
}

// ============================================================
// ランタイム
// ============================================================

/// 1つの SHIORI インスタンスと、それを動かすホスト側の状態をまとめて所有する
///
/// `!Send`（中の ShioriInstance が !Send）。作ったスレッドで使うこと。
/// Drop で、shutdown → 未配送イベントの破棄 → unload → ライブラリ解放 の順に後始末する。
///
/// # 例: unload の順序と Drop の中で起きること（スレッド規約 5・6、post の完了通知 3、ABI 規約 5）
///
/// Drop は次の順に進む。
///
/// 1. shutdown: 以後の [`EventSender::send`] は false を返し、SHIORI の post には
///    `SHIORI_E_SHUTDOWN` を返す（スレッド規約 5）
/// 2. まだ [`pump`](GhostRuntime::pump) していないイベントと post を捨てる。SHIORI には届かない（post の完了通知 3）
/// 3. unload。その中で SHIORI が出した log はまだ `Host::log` に届く（ホストのテーブルは unload が返るまで有効。スレッド規約 5）
/// 4. ホストのテーブルを放し、最後にライブラリを放す（[`from_static`](GhostRuntime::from_static) にはライブラリが無い）
///
/// 応答（[`ShioriOwnedResponse`]）はランタイムより長く持ってよい。応答はホストが
/// `copy_from_slice` で作り、`release` は常にホストの関数なので、unload やライブラリの解放の後でも
/// 読んで捨てられる（ABI 規約 5・スレッド規約 6）。
/// この例は DLL なしに [`GhostRuntime::from_static`] で往復を実行して確かめている。
///
/// ```
/// use shiori4_host::*;
/// use std::sync::{Arc, Mutex};
/// # struct UnloadGhost {
/// #     host: shiori4::ShioriHostHandle,
/// # }
/// # impl shiori4::Ghost for UnloadGhost {
/// #     fn load(host: Option<shiori4::ShioriHostHandle>, _dir: &str) -> Result<Self, ShioriStatus> {
/// #         Ok(Self { host: host.ok_or(SHIORI_E_UNSUPPORTED)? })
/// #     }
/// #     fn request(&mut self, req: &str) -> ShioriResponse {
/// #         let id = protocol::Headers::parse(req).get(protocol::HEADER_ID).unwrap_or("").to_owned();
/// #         self.host.log(0, &format!("request {id}"));
/// #         protocol::HeaderWriter::new().push(protocol::HEADER_VALUE, "\\0hi\\e").into_response()
/// #     }
/// # }
/// # impl Drop for UnloadGhost {
/// #     fn drop(&mut self) {
/// #         self.host.log(0, "unload");
/// #     }
/// # }
/// # static API: ShioriApi = ShioriApi::new::<shiori4::Adapter<UnloadGhost>, { SHIORI_ABI_VERSION }>();
///
/// // SHIORI が log に出した行を、ランタイムの外から読めるように集める
/// struct LogHost {
///     lines: Arc<Mutex<Vec<String>>>,
/// }
///
/// impl Host for LogHost {
///     fn name(&self) -> &str {
///         "MyBase"
///     }
///     fn on_post(&self, _post: Post) {}
///     fn log(&self, _level: i32, msg: &str) {
///         self.lines.lock().unwrap().push(msg.to_owned());
///     }
/// }
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let lines = Arc::new(Mutex::new(Vec::new()));
/// let dir = if cfg!(windows) { r"C:\ghost" } else { "/ghost" };
/// let mut rt = GhostRuntime::from_static(&API, dir, LogHost { lines: Arc::clone(&lines) })?;
/// let res = rt.request("ID: OnBoot\r\n").map_err(|s| format!("ABI error: {s}"))?;
/// let sender = rt.event_sender();
/// // 積んだが pump しないまま Drop する
/// assert!(sender.send(Event::request("ID: OnClose\r\n")));
/// drop(rt);
/// // OnClose は捨てられ、SHIORI には届かずに unload された
/// assert_eq!(*lines.lock().unwrap(), ["request OnBoot", "unload"]);
/// // Drop の後の send は false
/// assert!(sender.is_closed());
/// assert!(!sender.send(Event::request("ID: OnBoot\r\n")));
/// // 応答はランタイムより長く持てる（スレッド規約 6）
/// assert_eq!(res.body.as_str()?, "Value: \\0hi\\e\r\n");
/// drop(res);
/// # Ok(())
/// # }
/// ```
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
    ///
    /// 失敗したときは読んだライブラリを手放す（`NotShiori4`・`Unsupported` でも）。そのあと同じファイルを
    /// SHIORI/3.0 や別の版として読み直すと、DLL が 2 回読まれる（初期化コードも 2 回動く）。
    /// 読み直すつもりなら、ライブラリを自分で開いて [`GhostRuntime::from_library`] に渡し、
    /// [`FromLibraryError`] から取り戻す。値ごとの扱いは [`LoadError`]
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
    ///
    /// # 例: ライブラリを取り戻して読み直す（DLL の口と発見の順 2・版 4・load の規約 1・3）
    ///
    /// ライブラリを 1 回だけ開き、`NotShiori4` なら SHIORI/3.0 の読み込みへ、`Unsupported` なら
    /// v3 互換モードへ、同じライブラリを渡す。`LoadFailed` ではライブラリが返らないので読み直さない。
    /// この例は DLL を要するので、ビルドだけを検査する（実行しない）。
    ///
    /// ```no_run
    /// use shiori4_host::*;
    /// # struct MyHost;
    /// # impl Host for MyHost { fn name(&self) -> &str { "MyBase" } fn on_post(&self, _post: Post) {} }
    /// // ホストの SHIORI/3.0 の読み込み（このクレートの外）。開いたライブラリを受け取る
    /// fn load_shiori3(_lib: libloading::Library, _dir: &std::path::Path) {}
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let dir = std::env::current_exe()?.parent().expect("exe dir").join("ghost").join("master");
    /// // SAFETY: ライブラリの初期化コードが動く。信頼できる SHIORI だけを開く
    /// let lib = unsafe { libloading::Library::new(dir.join("shiori.dll")) }?;
    /// match GhostRuntime::from_library(lib, &dir, MyHost) {
    ///     Ok(mut rt) => { rt.request("ID: OnBoot\r\n").map_err(|s| format!("ABI error: {s}"))?; }
    ///     Err(e) => match e.into_parts() {
    ///         // 旧来の SHIORI/3.0 の候補へ、開いたライブラリのまま回す（DLL の口と発見の順 2）
    ///         (LoadError::NotShiori4, Some(lib)) => load_shiori3(lib, &dir),
    ///         // 別の版で読み直す（版 4）
    ///         (LoadError::Unsupported(_), Some(lib)) => {
    ///             let mut rt = V3Runtime::from_library(lib, &dir)?;
    ///             let res = rt.request("GET SHIORI/3.0\r\nID: OnBoot\r\n\r\n").map_err(|s| format!("ABI error: {s}"))?;
    ///             # drop(res);
    ///         }
    ///         // dir を直せば同じライブラリで読み直せる（load の規約 1）
    ///         (LoadError::InvalidPath, Some(_lib)) => return Err("ghost dir must be absolute".into()),
    ///         // LoadFailed: ライブラリは返らない。読めなかったゴーストとして扱う（load の規約 3）
    ///         (e, _) => return Err(e.into()),
    ///     },
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_library(
        lib: libloading::Library,
        ghost_dir: impl AsRef<Path>,
        host: H,
    ) -> Result<Self, FromLibraryError> {
        // SAFETY: lib は GhostRuntime の最後のフィールドとして保持し、instance より後に解放する。
        // 'static はこの構造体の外へ漏らさない
        let (dir, api) =
            match unsafe { library_entry(&lib, ghost_dir.as_ref(), SHIORI_ABI_VERSION) } {
                Ok(v) => v,
                Err(error) => {
                    return Err(FromLibraryError {
                        error,
                        library: Some(lib),
                    });
                }
            };
        // with_api が失敗したとき lib は中で捨てられる（LoadFailed は読み直さない）
        Self::with_api(api, &dir, host, Some(lib)).map_err(|error| FromLibraryError {
            error,
            library: None,
        })
    }

    /// 同じプロセス内のテーブル（ShioriApi::new で作った static）を使う。テスト用
    ///
    /// dir は `load` と同じ検査と正規化を経て SHIORI に届く
    ///
    /// # 例: DLL なしの決定論のテスト（load の規約 1・スレッド規約 1）
    ///
    /// SHIORI のテーブルを同じプロセスの `static` に置けば、DLL もベースウェアも無しに往復を通せる。
    /// dir は存在しなくてよい（絶対パスで UTF-8 でありさえすればよい。load の規約 1）。ワーカーのスレッドも時計も使わず、
    /// テストのスレッドでランタイムを作って呼ぶので（スレッド規約 1）、毎回同じ結果になる。
    /// テストの関数は `tests/` などに置き、`#[test]` を付ける。この例はその関数を `fn main` から呼んで
    /// 実行して確かめている。doctest では `#[test]` の関数が消えるので、`cfg_attr(test, test)` で包んでいる。
    ///
    /// ```
    /// use shiori4_host::*;
    /// # struct EchoGhost;
    /// # impl shiori4::Ghost for EchoGhost {
    /// #     fn load(_host: Option<shiori4::ShioriHostHandle>, _dir: &str) -> Result<Self, ShioriStatus> {
    /// #         Ok(Self)
    /// #     }
    /// #     fn request(&mut self, req: &str) -> ShioriResponse {
    /// #         match protocol::Headers::parse(req).get(protocol::HEADER_ID) {
    /// #             Some("OnBoot") => protocol::HeaderWriter::new().push(protocol::HEADER_VALUE, "\\0hi\\e").into_response(),
    /// #             _ => ShioriResponse::no_content(),
    /// #         }
    /// #     }
    /// # }
    /// // テストする SHIORI のテーブル。DLL を作らずに同じプロセスに置く
    /// static API: ShioriApi = ShioriApi::new::<shiori4::Adapter<EchoGhost>, { SHIORI_ABI_VERSION }>();
    ///
    /// struct TestHost;
    ///
    /// impl Host for TestHost {
    ///     fn name(&self) -> &str {
    ///         "TestBase"
    ///     }
    ///     fn on_post(&self, _post: Post) {}
    /// }
    ///
    /// #[cfg_attr(test, test)] // tests/ では #[test] だけでよい
    /// fn on_boot_says_hi() {
    ///     let dir = if cfg!(windows) { r"C:\ghost" } else { "/ghost" };
    ///     let mut rt = GhostRuntime::from_static(&API, dir, TestHost).expect("load");
    ///     let res = rt.request("ID: OnBoot\r\n").expect("abi");
    ///     assert_eq!(res.status, SHIORI_200_OK);
    ///     assert_eq!(res.body.as_str().unwrap(), "Value: \\0hi\\e\r\n");
    ///     // 知らないイベントには 204
    ///     let res = rt.request("ID: OnUnknown\r\n").expect("abi");
    ///     assert_eq!(res.status, SHIORI_204_NO_CONTENT);
    /// }
    ///
    /// fn main() {
    ///     on_boot_says_hi();
    /// }
    /// ```
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
        let sender = EventSender {
            tx: tx.clone(),
            wake,
            shutdown: Arc::clone(&shutdown),
        };
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
        let instance =
            unsafe { std::mem::transmute::<ShioriInstance<'_>, ShioriInstance<'static>>(inst) };

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
    use shiori4::protocol::{HeaderWriter, Headers, Lookup, PendingTags, Properties};
    use shiori4::shiori4_abi::{ShioriHostHandle, ShioriHostPoster};
    use shiori4::{Adapter, Ghost};
    use std::ffi::c_void;
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
            Ok(Self {
                props: Properties::new(host),
                poster,
                pending: PendingTags::new(),
            })
        }

        fn request(&mut self, req: &str) -> ShioriResponse {
            let req = Headers::parse(req);
            match req.id() {
                Some("OnBoot") => {
                    let name = match self.props.get("name") {
                        Lookup::Ready(v) => v.to_owned(),
                        _ => "?".to_owned(),
                    };
                    if let Some(p) = &self.poster {
                        let _ = self.pending.post(
                            p,
                            "dl",
                            &protocol::send_post(r"\0\s[0]hi\e", &[], None),
                        );
                    }
                    HeaderWriter::new().push("Value", &name).into_response()
                }
                Some(protocol::ID_HOST_COMPLETE) => match self.pending.take_from(&req) {
                    Some((_, kind)) => HeaderWriter::new()
                        .push(
                            "Value",
                            &format!(
                                "done:{kind}:{}:{}",
                                req.get(protocol::HEADER_STATUS).unwrap_or(""),
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

    /// getproperty に便利関数で答える。value が値を引く（None なら 204）。getproperty 以外は 400
    fn answer_getproperty(req: &str, value: impl FnOnce(&str) -> Option<String>) -> ShioriResponse {
        let h = Headers::parse(req);
        match (h.id(), h.reference(0)) {
            (Some(protocol::ID_GETPROPERTY), Some(name)) => {
                protocol::getproperty_response(value(name).as_deref())
            }
            _ => protocol::rejection_response(None),
        }
    }

    impl Host for TestHost {
        fn name(&self) -> &str {
            "test"
        }

        fn request(&self, req: &str) -> ShioriResponse {
            answer_getproperty(req, |name| (name == "name").then(|| "さくら".to_owned()))
        }

        fn on_post(&self, post: Post) {
            let (text, done) = post.into_parts();
            assert_eq!(
                Headers::parse(&text).get(protocol::HEADER_COMMAND),
                Some(protocol::COMMAND_SEND)
            );
            let headers = protocol::host_complete(&text, self.name(), SHIORI_200_OK, &["ok"], None)
                .expect("Tag 付き");
            assert!(done.complete_with(Delivery::Request, &headers));
        }

        fn wake(&self) {
            self.woken.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn runtime() -> GhostRuntime<TestHost> {
        GhostRuntime::from_static(
            &API,
            GHOST_DIR,
            TestHost {
                woken: AtomicUsize::new(0),
            },
        )
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

    /// 規約 2 の期間の中の Bridge と、post が積まれるキュー
    fn bridge<H: Host>(host: H, in_call: bool) -> (Bridge<H>, mpsc::Receiver<Msg>) {
        let (tx, rx) = mpsc::channel();
        let b = Bridge {
            host: Arc::new(host),
            tx,
            shutdown: Arc::new(AtomicBool::new(false)),
            owner: std::thread::current().id(),
            in_call: AtomicBool::new(in_call),
        };
        (b, rx)
    }

    /// request と accept_post に届いた原文を覚える。request には 500 + `X: y` で答える
    #[derive(Default)]
    struct RecordHost {
        reject: bool,
        requests: std::sync::Mutex<Vec<String>>,
        accepts: std::sync::Mutex<Vec<String>>,
    }

    impl Host for RecordHost {
        fn name(&self) -> &str {
            "record"
        }

        fn request(&self, req: &str) -> ShioriResponse {
            self.requests.lock().unwrap().push(req.to_owned());
            ShioriResponse::with_status(SHIORI_500_INTERNAL_SERVER_ERROR, "X: y\r\n")
        }

        fn accept_post(&self, msg: &str) -> bool {
            self.accepts.lock().unwrap().push(msg.to_owned());
            !self.reject
        }

        fn on_post(&self, _post: Post) {}
    }

    /// name と on_post だけを実装する（ほかは既定）
    struct DefaultHost;

    impl Host for DefaultHost {
        fn name(&self) -> &str {
            "default"
        }

        fn on_post(&self, _post: Post) {}
    }

    #[test]
    fn bridge_passes_request_verbatim() {
        let (b, _rx) = bridge(RecordHost::default(), true);
        let reqs = [
            "ID: x-unknown\r\nfoo:  bar \r\nReference0: a\r\n",
            "ID: getproperty\r\nReference0: name\r\n",
            "",
        ];
        for req in reqs {
            let r = ShioriHost::request(&b, req.as_bytes()).into_owned();
            // 芯は getproperty にも答えず、Host の応答をそのまま返す
            assert_eq!(r.status, SHIORI_500_INTERNAL_SERVER_ERROR);
            assert_eq!(r.body.as_str().unwrap(), "X: y\r\n");
        }
        assert_eq!(*b.host.requests.lock().unwrap(), reqs);
    }

    #[test]
    fn default_host_request_is_400_for_any_id() {
        let (b, _rx) = bridge(DefaultHost, true);
        for req in [
            PROP_REQ,
            b"ID: property\r\nReference0: name\r\n",
            b"ID: unknown\r\n",
        ] {
            let r = ShioriHost::request(&b, req).into_owned();
            assert_eq!(r.status, SHIORI_400_BAD_REQUEST);
            assert!(r.body.is_empty());
        }
    }

    #[test]
    fn post_without_command_is_accepted_by_default() {
        let (b, rx) = bridge(DefaultHost, true);
        for msg in ["Tag: 1\r\n", ""] {
            assert_eq!(ShioriHost::post(&b, msg.as_bytes()), SHIORI_204_NO_CONTENT);
            let Ok(Msg::Post(text)) = rx.try_recv() else {
                panic!("post が積まれない")
            };
            assert_eq!(text, msg);
        }
    }

    #[test]
    fn rejected_post_is_400() {
        let (b, rx) = bridge(
            RecordHost {
                reject: true,
                ..Default::default()
            },
            true,
        );
        assert_eq!(
            ShioriHost::post(&b, b"Command: SEND\r\nTag: 1\r\n"),
            SHIORI_400_BAD_REQUEST
        );
        assert_eq!(
            *b.host.accepts.lock().unwrap(),
            ["Command: SEND\r\nTag: 1\r\n"]
        );
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn non_utf8_request_and_post_are_400_without_host() {
        let (b, rx) = bridge(RecordHost::default(), true);
        let r = ShioriHost::request(&b, b"ID: \xff\r\n").into_owned();
        assert_eq!(r.status, SHIORI_400_BAD_REQUEST);
        assert!(r.body.is_empty());
        assert_eq!(
            ShioriHost::post(&b, b"Tag: \xff\r\n"),
            SHIORI_400_BAD_REQUEST
        );
        assert!(b.host.requests.lock().unwrap().is_empty());
        assert!(b.host.accepts.lock().unwrap().is_empty());
        assert!(rx.try_recv().is_err());
    }

    /// 送り口と、その送り口が積むキュー
    fn test_sender() -> (EventSender, mpsc::Receiver<Msg>) {
        let (tx, rx) = mpsc::channel();
        (
            EventSender {
                tx,
                wake: Arc::new(|| {}),
                shutdown: Arc::new(AtomicBool::new(false)),
            },
            rx,
        )
    }

    #[test]
    fn completer_delivers_headers_verbatim() {
        let (sender, rx) = test_sender();
        // Tag 付きの post でも Tag の無い post（空文字列を含む）でも、引数をそのまま積む
        let cases = [
            (
                "Command: SEND\r\nTag: 1\r\n",
                Delivery::Request,
                "ID: x-any\r\nfoo:  bar \r\n",
            ),
            (
                "Command: SEND\r\n",
                Delivery::Notify,
                "ID: OnHostComplete\r\nStatus: 200\r\n",
            ),
            ("", Delivery::Request, ""),
        ];
        for (post, delivery, headers) in cases {
            let post = Post::new(post.to_owned(), sender.clone());
            assert!(post.into_completer().complete_with(delivery, headers));
            let Ok(Msg::Event(ev)) = rx.try_recv() else {
                panic!("event が積まれない")
            };
            assert_eq!(ev.delivery, delivery);
            assert_eq!(ev.headers, headers);
        }
        assert!(rx.try_recv().is_err());
    }

    // ---- Charset を付けない（10.10） ----

    #[test]
    fn no_charset_in_core_and_helpers() {
        let no_charset = |s: &str| assert!(!s.to_ascii_lowercase().contains("charset"), "{s:?}");
        let (b, _rx) = bridge(DefaultHost, true);
        let r = ShioriHost::request(&b, PROP_REQ).into_owned();
        assert_eq!(r.status, SHIORI_400_BAD_REQUEST);
        no_charset(r.body.as_str().unwrap());
        assert_eq!(
            ShioriHost::post(&b, b"Command: SEND\r\nTag: 1\r\n"),
            SHIORI_204_NO_CONTENT
        );

        let post = protocol::send_post(r"\0hi\e", &["nobreak"], Some("1"));
        for s in [
            protocol::getproperty_request("baseware.name"),
            protocol::setproperty_request("currentghost.name", "x"),
            protocol::notify_post("OnTest", &["a"], &["nobreak"], Some("2")),
            protocol::host_complete(&post, "test", SHIORI_200_OK, &["ok"], None).expect("Tag 付き"),
            protocol::host_complete(
                &post,
                "test",
                SHIORI_400_BAD_REQUEST,
                &[],
                Some("forbidden"),
            )
            .expect("Tag 付き"),
            post,
        ] {
            no_charset(&s);
        }
        for r in [
            protocol::getproperty_response(Some("v")),
            protocol::getproperty_response(None),
            protocol::rejection_response(None),
            protocol::rejection_response(Some("forbidden")),
        ] {
            no_charset(r.into_owned().body.as_str().unwrap());
        }
    }

    // ---- 規約 2（ThreadGuard） ----

    /// request が呼ばれた回数と、log で届いた文を数える。getproperty には `v:<名前>` で答える
    #[derive(Default)]
    struct CountHost {
        calls: AtomicUsize,
        logs: std::sync::Mutex<Vec<String>>,
    }

    impl Host for CountHost {
        fn name(&self) -> &str {
            "count"
        }

        fn request(&self, req: &str) -> ShioriResponse {
            self.calls.fetch_add(1, Ordering::Relaxed);
            answer_getproperty(req, |name| Some(format!("v:{name}")))
        }

        fn on_post(&self, _post: Post) {}

        fn log(&self, _level: i32, msg: &str) {
            self.logs.lock().unwrap().push(msg.to_owned());
        }
    }

    const PROP_REQ: &[u8] = b"ID: getproperty\r\nReference0: name\r\n";

    fn count_bridge(in_call: bool) -> Bridge<CountHost> {
        bridge(CountHost::default(), in_call).0
    }

    #[test]
    fn request_from_other_thread_is_wrong_thread() {
        // 持ち主のスレッドが処理中でも、別スレッドからの request は -7 で Host に渡らない
        let b = count_bridge(true);
        let r = std::thread::scope(|s| {
            s.spawn(|| ShioriHost::request(&b, PROP_REQ).into_owned())
                .join()
                .unwrap()
        });
        assert_eq!(r.status, SHIORI_E_WRONG_THREAD);
        assert!(r.body.is_empty());
        assert_eq!(b.host.calls.load(Ordering::Relaxed), 0);
        // 持ち主のスレッドからなら通る
        let r = ShioriHost::request(&b, PROP_REQ).into_owned();
        assert_eq!(r.status, 200);
        assert_eq!(b.host.calls.load(Ordering::Relaxed), 1);
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
            Ok(Self {
                props,
                poster,
                at_load,
            })
        }

        fn request(&mut self, req: &str) -> ShioriResponse {
            match Headers::parse(req).id() {
                Some("OnLog") => {
                    let p = self.poster.clone().unwrap();
                    std::thread::spawn(move || p.log(SHIORI_LOG_INFO, "from worker"))
                        .join()
                        .unwrap();
                    ShioriResponse::no_content()
                }
                _ => {
                    let v = format!("{}|{}", self.at_load, value(&mut self.props, "req"));
                    HeaderWriter::new().push("Value", &v).into_response()
                }
            }
        }

        fn notify(&mut self, _req: &str) -> ShioriStatus {
            if value(&mut self.props, "notify") == "v:notify" {
                SHIORI_200_OK
            } else {
                SHIORI_500_INTERNAL_SERVER_ERROR
            }
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
        assert!(
            r.body.as_str().unwrap().contains("Value: v:load|v:req"),
            "{:?}",
            r.body.as_str()
        );
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
        let ok =
            |input: &str, want: &str| assert_eq!(norm(input).as_deref(), Some(want), "{input}");
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
        let ok =
            |input: &str, want: &str| assert_eq!(norm(input).as_deref(), Some(want), "{input}");
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
        let messy = format!(
            "{}//",
            GHOST_DIR.trim_end_matches(std::path::MAIN_SEPARATOR)
        );
        let _rt = GhostRuntime::from_static(
            &API,
            &messy,
            TestHost {
                woken: AtomicUsize::new(0),
            },
        )
        .expect("load");
        let got = LOADED_DIR.lock().unwrap().clone();
        assert_eq!(got, GHOST_DIR_NORMALIZED);
    }

    #[test]
    fn from_static_rejects_relative_dir() {
        for bad in ["ghost", "ghost/master"] {
            let r = GhostRuntime::from_static(
                &API,
                bad,
                TestHost {
                    woken: AtomicUsize::new(0),
                },
            );
            assert!(matches!(r, Err(LoadError::InvalidPath)), "{bad}");
        }
    }

    #[test]
    fn load_rejects_relative_dir_before_opening_library() {
        // ライブラリのパスは存在しない。dir の検査が先なので Library ではなく InvalidPath になる
        let r = GhostRuntime::load(
            "no-such-shiori.dll",
            "ghost",
            TestHost {
                woken: AtomicUsize::new(0),
            },
        );
        assert!(matches!(r, Err(LoadError::InvalidPath)));
    }

    #[test]
    fn invalid_path_display_mentions_absolute() {
        assert!(LoadError::InvalidPath.to_string().contains("absolute"));
    }

    #[test]
    fn not_shiori4_display_is_no_allowed_entry() {
        assert_eq!(
            LoadError::NotShiori4.to_string(),
            "no allowed SHIORI entry point found"
        );
    }

    // ---- from_library（要件 7.6・12.1〜12.4） ----

    struct FailGhost;

    impl Ghost for FailGhost {
        fn load(_host: Option<ShioriHostHandle>, _dir: &str) -> Result<Self, ShioriStatus> {
            Err(SHIORI_500_INTERNAL_SERVER_ERROR)
        }

        fn request(&mut self, _req: &str) -> ShioriResponse {
            ShioriResponse::no_content()
        }
    }

    static FAIL_API: ShioriApi = ShioriApi::new::<Adapter<FailGhost>, { SHIORI_ABI_VERSION }>();

    #[test]
    fn from_static_load_null_is_load_failed() {
        let r = GhostRuntime::from_static(
            &FAIL_API,
            GHOST_DIR,
            TestHost {
                woken: AtomicUsize::new(0),
            },
        );
        assert!(matches!(r, Err(LoadError::LoadFailed)));
    }

    struct FailLogHost(std::sync::Arc<std::sync::Mutex<Vec<(i32, String)>>>);

    impl Host for FailLogHost {
        fn name(&self) -> &str {
            "faillog"
        }

        fn on_post(&self, _post: Post) {}

        fn log(&self, level: i32, msg: &str) {
            self.0.lock().unwrap().push((level, msg.to_owned()));
        }
    }

    #[test]
    fn load_failure_reason_reaches_host_log() {
        let logs = std::sync::Arc::default();
        let r = GhostRuntime::from_static(
            &FAIL_API,
            GHOST_DIR,
            FailLogHost(std::sync::Arc::clone(&logs)),
        );
        assert!(matches!(r, Err(LoadError::LoadFailed)));
        let logs = logs.lock().unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].0, shiori4::SHIORI_LOG_ERROR);
        assert!(logs[0].1.contains("500"), "{:?}", logs[0].1);
    }

    fn test_lib() -> libloading::Library {
        unsafe { libloading::Library::new(crate::test_dll::dll_path()) }
            .expect("テスト用 DLL を開けない")
    }

    fn test_host() -> TestHost {
        TestHost {
            woken: AtomicUsize::new(0),
        }
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
        let e = GhostRuntime::from_library(lib, GHOST_DIR, test_host())
            .err()
            .expect("失敗するはず");
        assert!(matches!(e.error(), LoadError::NotShiori4));
        assert!(e.into_library().is_some());
    }

    #[test]
    fn from_library_unsupported_returns_library() {
        let _g = crate::test_dll::lock();
        let lib = test_lib();
        crate::test_dll::set_mode(&lib, crate::test_dll::MODE_V3);
        let e = GhostRuntime::from_library(lib, GHOST_DIR, test_host())
            .err()
            .expect("失敗するはず");
        assert!(matches!(e.error(), LoadError::Unsupported(_)));
        // 返ったライブラリでやり直せる
        let lib = e.into_library().expect("ライブラリが返る");
        crate::test_dll::set_mode(&lib, crate::test_dll::MODE_DEFAULT);
        assert!(GhostRuntime::from_library(lib, GHOST_DIR, test_host()).is_ok());
    }

    #[test]
    fn from_library_invalid_path_returns_library() {
        let _g = crate::test_dll::lock();
        let e = GhostRuntime::from_library(test_lib(), "ghost", test_host())
            .err()
            .expect("失敗するはず");
        assert!(matches!(e.error(), LoadError::InvalidPath));
        let (err, lib) = e.into_parts();
        assert!(matches!(err, LoadError::InvalidPath));
        // dir を直して同じライブラリでやり直せる
        assert!(
            GhostRuntime::from_library(lib.expect("ライブラリが返る"), GHOST_DIR, test_host())
                .is_ok()
        );
    }

    #[test]
    fn from_library_load_failed_drops_library() {
        let _g = crate::test_dll::lock();
        let dir = std::path::Path::new(GHOST_DIR).join("fail");
        let e = GhostRuntime::from_library(test_lib(), &dir, test_host())
            .err()
            .expect("失敗するはず");
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

    // ---- 応答はホストが作り、ホストの release で解放する（要件 15.3・15.5・15.6） ----

    #[test]
    fn response_dropped_after_runtime_is_safe() {
        // from_static: unload と ShioriHostBox の解放の後に読んで捨てる
        let mut rt = runtime();
        let r = rt.request("ID: OnBoot\r\n").expect("abi");
        drop(rt);
        assert!(r.body.as_str().unwrap().contains("Value: さくら"));
        drop(r);
        // DLL: ライブラリを外した後でも、release はホストの関数なので呼べる
        let _g = crate::test_dll::lock();
        let mut rt = GhostRuntime::from_library(test_lib(), GHOST_DIR, test_host()).expect("load");
        let r = rt.request("ID: OnBoot\r\n").expect("abi");
        drop(rt);
        assert!(r.body.as_str().unwrap().contains("Value: testghost"));
        drop(r);
    }

    type CopyFn = unsafe extern "C" fn(*mut c_void, *const u8, usize, *mut ShioriBytes);

    /// shiori4_abi の ShioriHostApi と同じ配置（欄は shiori4_abi の外から触れないので写す）
    #[repr(C)]
    struct RawHostApi {
        struct_size: u32,
        abi_version: u32,
        ctx: *mut c_void,
        request: Option<unsafe extern "C" fn()>,
        post: Option<unsafe extern "C" fn()>,
        log: Option<unsafe extern "C" fn()>,
        copy_from_slice: Option<CopyFn>,
    }

    /// shiori4_abi の ShioriApi と同じ配置
    #[repr(C)]
    struct RawApi {
        struct_size: u32,
        abi_version: u32,
        load: unsafe extern "C" fn(*const RawHostApi, *const u8, usize) -> *mut c_void,
        unload: unsafe extern "C" fn(*mut c_void),
        request:
            unsafe extern "C" fn(*mut c_void, *const u8, usize, *mut ShioriBytes) -> ShioriStatus,
        notify: unsafe extern "C" fn(*mut c_void, *const u8, usize) -> ShioriStatus,
    }

    const _: () =
        assert!(size_of::<RawHostApi>() == size_of::<shiori4::shiori4_abi::ShioriHostApi>());
    const _: () = assert!(size_of::<RawApi>() == size_of::<ShioriApi>());

    unsafe extern "C" fn plain_copy(
        _ctx: *mut c_void,
        ptr: *const u8,
        len: usize,
        out: *mut ShioriBytes,
    ) {
        let b = unsafe { std::slice::from_raw_parts(ptr, len) };
        unsafe { out.write(ShioriBytes::copy_from_slice(b)) }
    }

    fn raw_table(abi_version: u32, struct_size: usize, copy: Option<CopyFn>) -> RawHostApi {
        RawHostApi {
            struct_size: struct_size as u32,
            abi_version,
            ctx: std::ptr::null_mut(),
            request: None,
            post: None,
            log: None,
            copy_from_slice: copy,
        }
    }

    #[test]
    fn new_shiori_load_fails_without_copy_from_slice() {
        let _g = crate::test_dll::lock();
        let lib = test_lib();
        let get_api =
            unsafe { lib.get::<unsafe extern "C" fn(u32) -> *const RawApi>(b"shiori_get_api") }
                .expect("shiori_get_api");
        let full = size_of::<RawHostApi>();
        let old = std::mem::offset_of!(RawHostApi, copy_from_slice); // 欄の手前で終わる古いホスト
        for v in [
            SHIORI_ABI_VERSION,
            shiori4::shiori4_abi::SHIORI_ABI_VERSION_3,
        ] {
            let api = unsafe { &*get_api(v) };
            let load = |host: *const RawHostApi| unsafe {
                (api.load)(
                    host,
                    GHOST_DIR_NORMALIZED.as_ptr(),
                    GHOST_DIR_NORMALIZED.len(),
                )
            };
            assert!(
                load(&raw_table(v, old, Some(plain_copy))).is_null(),
                "v{v}: 大きさが足りない"
            );
            assert!(load(&raw_table(v, full, None)).is_null(), "v{v}: 欄が NULL");
            assert!(load(std::ptr::null()).is_null(), "v{v}: host が NULL");
            // 欄があれば読める（上の失敗が別の理由でないことの確かめ）
            let inst = load(&raw_table(v, full, Some(plain_copy)));
            assert!(!inst.is_null(), "v{v}");
            unsafe { (api.unload)(inst) };
        }
    }

    /// PROBE の load が受け取ったテーブル: (版, 大きさ, request・post・log・copy_from_slice があるか)
    static PROBED: std::sync::Mutex<Vec<(u32, u32, [bool; 4])>> = std::sync::Mutex::new(Vec::new());

    /// 受け取ったテーブルを PROBED に残し、応答はその copy_from_slice で作る
    unsafe extern "C" fn probe_load(h: *const RawHostApi, _d: *const u8, _l: usize) -> *mut c_void {
        let h = unsafe { &*h };
        let has = [
            h.request.is_some(),
            h.post.is_some(),
            h.log.is_some(),
            h.copy_from_slice.is_some(),
        ];
        PROBED
            .lock()
            .unwrap()
            .push((h.abi_version, h.struct_size, has));
        // 欄が無ければ load を失敗させる（extern "C" の中でパニックしない）
        match h.copy_from_slice {
            Some(copy) => Box::into_raw(Box::new((h.ctx, copy))).cast(),
            None => std::ptr::null_mut(),
        }
    }

    unsafe extern "C" fn probe_unload(inst: *mut c_void) {
        drop(unsafe { Box::from_raw(inst as *mut (*mut c_void, CopyFn)) });
    }

    unsafe extern "C" fn probe_request(
        inst: *mut c_void,
        _req: *const u8,
        _len: usize,
        out: *mut ShioriBytes,
    ) -> ShioriStatus {
        let (ctx, copy) = unsafe { *(inst as *const (*mut c_void, CopyFn)) };
        let body = b"Value: probe";
        unsafe { copy(ctx, body.as_ptr(), body.len(), out) };
        SHIORI_200_OK
    }

    unsafe extern "C" fn probe_notify(_i: *mut c_void, _r: *const u8, _l: usize) -> ShioriStatus {
        SHIORI_200_OK
    }

    const fn probe(abi_version: u32) -> RawApi {
        RawApi {
            struct_size: size_of::<RawApi>() as u32,
            abi_version,
            load: probe_load,
            unload: probe_unload,
            request: probe_request,
            notify: probe_notify,
        }
    }

    static PROBE_V4: RawApi = probe(SHIORI_ABI_VERSION);
    static PROBE_V3: RawApi = probe(shiori4::shiori4_abi::SHIORI_ABI_VERSION_3);

    fn as_api(raw: &'static RawApi) -> &'static ShioriApi {
        // SAFETY: 同じ配置（上の assert）
        unsafe { &*(raw as *const RawApi).cast::<ShioriApi>() }
    }

    #[test]
    fn runtimes_pass_tables_with_copy_from_slice() {
        let full = size_of::<RawHostApi>() as u32;
        // v3: copy_from_slice だけのテーブル
        let mut v3 = V3Runtime::from_static(as_api(&PROBE_V3), GHOST_DIR).expect("load");
        assert_eq!(v3.request("x"), Ok("Value: probe".to_owned()));
        drop(v3);
        // v4: 全部の欄があるテーブル。応答はランタイムを捨てた後にも読めて、捨てられる
        let mut rt =
            GhostRuntime::from_static(as_api(&PROBE_V4), GHOST_DIR, test_host()).expect("load");
        let r = rt.request("x").expect("abi");
        drop(rt);
        assert_eq!(r.body.as_str(), Ok("Value: probe"));
        drop(r);
        assert_eq!(
            *PROBED.lock().unwrap(),
            [
                (
                    shiori4::shiori4_abi::SHIORI_ABI_VERSION_3,
                    full,
                    [false, false, false, true]
                ),
                (SHIORI_ABI_VERSION, full, [true, true, true, true]),
            ]
        );
    }

    // ---- 芯は便利関数を参照しない（9.3・10.1） ----

    /// 芯のコードの行（`mod tests {` より前の、空でなくコメントでない行）。
    /// 境目は `#[cfg(test)]` でなく `mod tests {`（芯の途中に `#[cfg(test)] mod test_dll;` があるため）
    fn code_lines_before_tests(src: &str) -> impl Iterator<Item = &str> {
        src.lines()
            .map(str::trim)
            .take_while(|l| *l != "mod tests {")
            .filter(|l| !l.is_empty() && !l.starts_with("//"))
    }

    /// code_lines_before_tests のうち `protocol` を語として含む行
    fn protocol_lines(src: &str) -> Vec<&str> {
        code_lines_before_tests(src)
            .filter(|l| {
                l.split(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .any(|w| w == "protocol")
            })
            .collect()
    }

    #[test]
    fn protocol_lines_counts_only_code_before_tests() {
        // 途中の `#[cfg(test)] mod test_dll;` は境目でない。その後の芯の参照も数える
        let src = "//! protocol の doc\n/// protocol::Headers を使ってよい\npub use shiori4::protocol;\n#[cfg(test)]\nmod test_dll;\nlet protocols = my_protocol;\n    use protocol::Headers;\n#[cfg(test)]\nmod tests {\n    use protocol::Headers;\n}\n";
        assert_eq!(
            protocol_lines(src),
            ["pub use shiori4::protocol;", "use protocol::Headers;"]
        );

        // 境目が無ければテストの中の参照も数え、検査は黙って通らない
        let no_boundary = "pub use shiori4::protocol;\nmod test {\n    use protocol::Headers;\n}\n";
        assert_eq!(protocol_lines(no_boundary).len(), 2);
    }

    #[test]
    fn core_refers_to_protocol_only_by_its_reexport() {
        assert_eq!(
            protocol_lines(include_str!("lib.rs")),
            ["pub use shiori4::protocol;"]
        );
    }
}
