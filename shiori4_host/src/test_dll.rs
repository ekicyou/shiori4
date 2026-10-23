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
