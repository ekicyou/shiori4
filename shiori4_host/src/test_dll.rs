//! テスト専用: テスト用 cdylib（`testdata/testghost/`）を作って場所を返す・DLL の状態を守るロック
//!
//! DLL のモードは DLL の static でプロセスに 1 つなので、DLL を使うテストは [`lock`] を取ってから動かし、
//! モードを既定に戻してから終える。

use std::{
    env::consts::{DLL_PREFIX, DLL_SUFFIX},
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, MutexGuard, OnceLock},
};

/// テスト用 DLL を（1 テストバイナリにつき 1 回だけ）作り、その場所を返す
pub(crate) fn dll_path() -> &'static Path {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("ワークスペースのルート");
        let target_dir = root.join("target").join("testghost");
        let out = Command::new(env!("CARGO"))
            .arg("build")
            .arg("--manifest-path")
            .arg(root.join("testdata").join("testghost").join("Cargo.toml"))
            .arg("--target-dir")
            .arg(&target_dir)
            // 外の cargo test の --target を引き継ぐと出力の場所がずれる
            .env_remove("CARGO_BUILD_TARGET")
            .output()
            .expect("cargo を起動できない");
        assert!(
            out.status.success(),
            "テスト用 DLL のビルドに失敗した:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
        target_dir.join("debug").join(format!("{DLL_PREFIX}testghost{DLL_SUFFIX}"))
    })
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
    let f = unsafe { lib.get::<unsafe extern "C" fn(u32)>(b"testghost_set_mode") }.expect("testghost_set_mode が無い");
    unsafe { f(mode) };
}

/// DLL のモードを守るロック。前のテストのパニックで毒になっていても使う
pub(crate) fn lock() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[test]
fn dll_has_four_exports() {
    let _g = lock();
    let lib = unsafe { libloading::Library::new(dll_path()) }.expect("テスト用 DLL を開けない");
    for name in [&b"shiori_get_api"[..], b"testghost_set_mode", b"loadu", b"load"] {
        let sym = unsafe { lib.get::<unsafe extern "C" fn()>(name) };
        assert!(sym.is_ok(), "{} が無い", String::from_utf8_lossy(name));
    }
}
