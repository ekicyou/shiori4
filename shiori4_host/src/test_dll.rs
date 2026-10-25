//! テスト専用: テスト用 cdylib（`testdata/testghost/`・x86 の Windows では `testdata/legacyghost/` も）を作って場所を返す・DLL の状態を守るロック
//!
//! DLL のモードは DLL の static でプロセスに 1 つなので、DLL を使うテストは [`lock`] を取ってから動かし、
//! モードを既定に戻してから終える。

use std::{
    env::consts::{DLL_PREFIX, DLL_SUFFIX},
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, MutexGuard, OnceLock},
};

/// x86 の Windows のテストでは DLL も同じ三つ組で作る（gnu の x86 はテストの対象にしない）
#[cfg(all(windows, target_arch = "x86"))]
const TARGET: Option<&str> = Some("i686-pc-windows-msvc");
#[cfg(not(all(windows, target_arch = "x86")))]
const TARGET: Option<&str> = None;

/// `testdata/<name>` を `target/<name>` に作り、DLL の場所を返す（testghost・legacyghost で共通）
fn build(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("ワークスペースのルート");
    let target_dir = root.join("target").join(name);
    let mut cmd = Command::new(env!("CARGO"));
    cmd.arg("build")
        .arg("--manifest-path")
        .arg(root.join("testdata").join(name).join("Cargo.toml"))
        .arg("--target-dir")
        .arg(&target_dir)
        // 外の cargo test の --target を引き継ぐと出力の場所がずれる
        .env_remove("CARGO_BUILD_TARGET");
    let mut out_dir = target_dir;
    if let Some(t) = TARGET {
        cmd.arg("--target").arg(t);
        out_dir = out_dir.join(t);
    }
    let out = cmd.output().expect("cargo を起動できない");
    assert!(
        out.status.success(),
        "テスト用 DLL（{name}）のビルドに失敗した:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out_dir
        .join("debug")
        .join(format!("{DLL_PREFIX}{name}{DLL_SUFFIX}"))
}

/// テスト用 DLL を（1 テストバイナリにつき 1 回だけ）作り、その場所を返す
pub(crate) fn dll_path() -> &'static Path {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| build("testghost"))
}

/// v4 のテーブルを出す（testdata/testghost/src/lib.rs と同じ値）
pub(crate) const MODE_V4: u32 = 1;
/// v3 のテーブルを出す
pub(crate) const MODE_V3: u32 = 2;
/// 既定: v4 と v3 の両方を出す
pub(crate) const MODE_DEFAULT: u32 = MODE_V4 | MODE_V3;

/// DLL のモードを設定する。[`lock`] を取ってから呼ぶこと
///
/// モードは DLL の static なので、lib を全部閉じると DLL が外れて既定に戻る。開いたままの lib に掛ける
pub(crate) fn set_mode(lib: &libloading::Library, mode: u32) {
    let f = unsafe { lib.get::<unsafe extern "C" fn(u32)>(b"testghost_set_mode") }
        .expect("testghost_set_mode が無い");
    unsafe { f(mode) };
}

/// DLL のモードを守るロック。前のテストのパニックで毒になっていても使う
pub(crate) fn lock() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// 従来の口のテスト用 DLL（`testdata/legacyghost/`）を（1 テストバイナリにつき 1 回だけ）作り、その場所を返す
#[cfg(all(windows, target_arch = "x86"))]
pub(crate) fn legacy_dll_path() -> &'static Path {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| build("legacyghost"))
}

/// legacyghost の数（testdata/legacyghost/src/lib.rs の `LegacyStats` と同じ形）
#[cfg(all(windows, target_arch = "x86"))]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct LegacyStats {
    pub loadu: u32,
    pub load: u32,
    pub request: u32,
    pub unload: u32,
    pub inputs_freed: u32,
    pub responses_allocated: u32,
}

// 応答の種類（testdata/legacyghost/src/lib.rs と同じ値）
/// 既定の SHIORI/3.0 200 OK
#[cfg(all(windows, target_arch = "x86"))]
pub(crate) const RESPONSE_OK: u32 = 0;
/// NULL を返す
#[cfg(all(windows, target_arch = "x86"))]
pub(crate) const RESPONSE_NULL: u32 = 1;
/// 長さに -1 を返す
#[cfg(all(windows, target_arch = "x86"))]
pub(crate) const RESPONSE_NEGATIVE: u32 = 2;
/// 受けた要求をそのまま返す
#[cfg(all(windows, target_arch = "x86"))]
pub(crate) const RESPONSE_ECHO: u32 = 3;

/// legacyghost の書き出しを引く。[`lock`] を取ってから使うこと
#[cfg(all(windows, target_arch = "x86"))]
fn legacy_sym<'a, T>(lib: &'a libloading::Library, name: &[u8]) -> libloading::Symbol<'a, T> {
    unsafe { lib.get::<T>(name) }
        .unwrap_or_else(|_| panic!("{} が無い", String::from_utf8_lossy(name)))
}

/// 数・最後の入力・応答の種類・初期化の戻り値を既定に戻す
#[cfg(all(windows, target_arch = "x86"))]
pub(crate) fn legacy_reset(lib: &libloading::Library) {
    unsafe { legacy_sym::<unsafe extern "C" fn()>(lib, b"legacyghost_reset")() };
}

/// 応答の種類を設定する（`RESPONSE_*`）
#[cfg(all(windows, target_arch = "x86"))]
pub(crate) fn legacy_set_response(lib: &libloading::Library, kind: u32) {
    unsafe { legacy_sym::<unsafe extern "C" fn(u32)>(lib, b"legacyghost_set_response")(kind) };
}

/// `load`・`loadu` の戻り値を設定する
#[cfg(all(windows, target_arch = "x86"))]
pub(crate) fn legacy_set_load_result(lib: &libloading::Library, v: i32) {
    unsafe { legacy_sym::<unsafe extern "C" fn(i32)>(lib, b"legacyghost_set_load_result")(v) };
}

/// 数を読む
#[cfg(all(windows, target_arch = "x86"))]
pub(crate) fn legacy_stats(lib: &libloading::Library) -> LegacyStats {
    let mut s = LegacyStats::default();
    unsafe {
        legacy_sym::<unsafe extern "C" fn(*mut LegacyStats)>(lib, b"legacyghost_stats")(&mut s)
    };
    s
}

/// 最後に受けた入力（load・loadu・request）を読む
#[cfg(all(windows, target_arch = "x86"))]
pub(crate) fn legacy_last_input(lib: &libloading::Library) -> Vec<u8> {
    let f =
        legacy_sym::<unsafe extern "C" fn(*mut u8, usize) -> usize>(lib, b"legacyghost_last_input");
    let n = unsafe { f(std::ptr::null_mut(), 0) };
    let mut buf = vec![0u8; n];
    let m = unsafe { f(buf.as_mut_ptr(), n) };
    buf.truncate(m.min(n));
    buf
}

#[test]
fn dll_has_four_exports() {
    let _g = lock();
    let lib = unsafe { libloading::Library::new(dll_path()) }.expect("テスト用 DLL を開けない");
    for name in [
        &b"shiori_get_api"[..],
        b"testghost_set_mode",
        b"loadu",
        b"load",
    ] {
        let sym = unsafe { lib.get::<unsafe extern "C" fn()>(name) };
        assert!(sym.is_ok(), "{} が無い", String::from_utf8_lossy(name));
    }
}

#[cfg(all(windows, target_arch = "x86"))]
#[test]
fn legacy_dll_is_i686_and_helpers_work() {
    let _g = lock();
    assert!(
        legacy_dll_path()
            .to_string_lossy()
            .contains("i686-pc-windows-msvc")
    );
    assert!(
        dll_path()
            .to_string_lossy()
            .contains("i686-pc-windows-msvc")
    );
    let lib =
        unsafe { libloading::Library::new(legacy_dll_path()) }.expect("legacyghost を開けない");
    legacy_reset(&lib);
    legacy_set_load_result(&lib, 2);
    legacy_set_response(&lib, RESPONSE_ECHO);
    let loadu =
        unsafe { lib.get::<unsafe extern "C" fn(*mut std::ffi::c_void, i32) -> i32>(b"loadu") }
            .expect("loadu が無い");
    // NULL の入力は解放されず、長さ 0 の入力として記録される
    assert_eq!(unsafe { loadu(std::ptr::null_mut(), 0) }, 2);
    let s = legacy_stats(&lib);
    assert_eq!((s.loadu, s.load, s.inputs_freed), (1, 0, 0));
    assert!(legacy_last_input(&lib).is_empty());
    legacy_reset(&lib);
    assert_eq!(legacy_stats(&lib), LegacyStats::default());
}
