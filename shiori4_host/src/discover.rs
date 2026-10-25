//! 口の発見: DLL の口を `shiori_get_api`（4 → 3）→ `loadu` → `load` の順に、許可された口だけ探し、見つかった口の種類と開いたライブラリを返す（`load_ghost`／`load_ghost_with`）

use std::{ffi::OsStr, path::Path};

use crate::{GhostRuntime, Host, LoadError, V3Runtime, normalize_dir};

/// 許可する口の種類。発見の順は並びによらず固定（V4 → V3 → Loadu → Load）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntryKind {
    /// `shiori_get_api(4)`
    V4,
    /// `shiori_get_api(3)`（v3 互換モード）
    V3,
    /// 従来の `loadu`
    Loadu,
    /// 従来の `load`
    Load,
}

impl EntryKind {
    /// すべての口（既定）
    pub const ALL: [EntryKind; 4] = [Self::V4, Self::V3, Self::Loadu, Self::Load];
}

/// 解決した口ごとの結果
pub enum LoadedGhost<H: Host> {
    /// v4 で読んだ
    V4(GhostRuntime<H>),
    /// v3 互換モードで読んだ
    V3(V3Runtime),
    /// 従来の口で解決した。entry は Loadu か Load。この spec では初期化しない（`loadu`・`load` は呼んでいない）
    Legacy {
        entry: EntryKind,
        library: libloading::Library,
    },
}

/// すべての口を許可して読む（[`load_ghost_with`] に [`EntryKind::ALL`] を渡すのと同じ）
pub fn load_ghost<H: Host>(
    lib_path: impl AsRef<OsStr>,
    ghost_dir: impl AsRef<Path>,
    host: H,
) -> Result<LoadedGhost<H>, LoadError> {
    load_ghost_with(lib_path, ghost_dir, host, &EntryKind::ALL)
}

/// 許可する口を渡して読む。allowed の並びと重複は無視する（発見の順は V4 → V3 → Loadu → Load で固定）
///
/// 手順は dir の検査（`InvalidPath`。ライブラリを開く前）→ ライブラリを開く（`Library`）→ v4 → v3 →
/// `loadu` → `load`。許可されていない口は DLL にあっても無いものとして扱う。
/// - `shiori_get_api` があって V4・V3 のどちらかを許可しているなら、結果（`Unsupported`・`LoadFailed`）に
///   かかわらず `loadu`・`load` へは進まない。V4・V3 のどちらも許可していなければ `shiori_get_api` は無いものとする
/// - 従来の口は書き出しの有無だけを見て、呼ばない。開いたライブラリを [`LoadedGhost::Legacy`] で返す
/// - 許可された口が 1 つも見つからなければ `NotShiori4`（allowed が空なら必ずこれ）
///
/// 機械の種類は確かめない（合わない DLL は `Library` になる）。ホストは [`crate::machine_matches`] で
/// 確かめてから渡すこと。ライブラリを開くと DLL の初期化コードが動く。信頼できる DLL だけを渡すこと
pub fn load_ghost_with<H: Host>(
    lib_path: impl AsRef<OsStr>,
    ghost_dir: impl AsRef<Path>,
    host: H,
    allowed: &[EntryKind],
) -> Result<LoadedGhost<H>, LoadError> {
    let dir = ghost_dir.as_ref();
    normalize_dir(dir)?;
    let allows = |k: EntryKind| allowed.contains(&k);
    #[cfg(all(windows, target_arch = "x86"))]
    let mut lib = crate::legacy::open_library(lib_path.as_ref())?;
    // SAFETY: ライブラリの初期化コードが実行される。信頼できる SHIORI だけを読み込むこと
    #[cfg(not(all(windows, target_arch = "x86")))]
    let mut lib = unsafe { libloading::Library::new(lib_path) }.map_err(LoadError::Library)?;

    if allows(EntryKind::V4) {
        match GhostRuntime::from_library(lib, dir, host) {
            Ok(rt) => return Ok(LoadedGhost::V4(rt)),
            Err(e) => match e.into_parts() {
                // shiori_get_api が無い: v3 も無いので従来の口へ
                (LoadError::NotShiori4, Some(l)) => return legacy(l, &allows),
                (LoadError::Unsupported(_), Some(l)) if allows(EntryKind::V3) => lib = l,
                (e, _) => return Err(e),
            },
        }
    }
    if allows(EntryKind::V3) {
        match V3Runtime::from_library(lib, dir) {
            Ok(rt) => return Ok(LoadedGhost::V3(rt)),
            Err(e) => match e.into_parts() {
                (LoadError::NotShiori4, Some(l)) => lib = l,
                (e, _) => return Err(e),
            },
        }
    }
    legacy(lib, &allows)
}

/// 従来の口を有無だけ見る（呼ばない）
fn legacy<H: Host>(
    lib: libloading::Library,
    allows: &impl Fn(EntryKind) -> bool,
) -> Result<LoadedGhost<H>, LoadError> {
    for (entry, sym) in [
        (EntryKind::Loadu, &b"loadu\0"[..]),
        (EntryKind::Load, b"load\0"),
    ] {
        // SAFETY: 書き出しの有無を見るだけで、呼ばない
        if allows(entry) && unsafe { lib.get::<unsafe extern "C" fn()>(sym) }.is_ok() {
            return Ok(LoadedGhost::Legacy {
                entry,
                library: lib,
            });
        }
    }
    Err(LoadError::NotShiori4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Post, test_dll};

    #[cfg(windows)]
    const GHOST_DIR: &str = r"C:\ghost";
    #[cfg(not(windows))]
    const GHOST_DIR: &str = "/ghost";

    struct NoHost;

    impl Host for NoHost {
        fn name(&self) -> &str {
            "nohost"
        }

        fn on_post(&self, _post: Post) {}
    }

    fn with(allowed: &[EntryKind]) -> Result<LoadedGhost<NoHost>, LoadError> {
        load_ghost_with(test_dll::dll_path(), GHOST_DIR, NoHost, allowed)
    }

    /// モードを設定して f を動かし、既定に戻す（ロックとモード用の別のハンドルを持つ）
    fn in_mode(mode: u32, f: impl FnOnce()) {
        let _g = test_dll::lock();
        let mode_lib = unsafe { libloading::Library::new(test_dll::dll_path()) }
            .expect("テスト用 DLL を開けない");
        test_dll::set_mode(&mode_lib, mode);
        f();
        test_dll::set_mode(&mode_lib, test_dll::MODE_DEFAULT);
    }

    #[test]
    fn default_is_v4() {
        in_mode(test_dll::MODE_DEFAULT, || {
            let r = load_ghost(test_dll::dll_path(), GHOST_DIR, NoHost);
            assert!(matches!(r, Ok(LoadedGhost::V4(_))));
        });
    }

    #[test]
    fn no_v4_is_v3() {
        in_mode(test_dll::MODE_V3, || {
            let r = load_ghost(test_dll::dll_path(), GHOST_DIR, NoHost);
            assert!(matches!(r, Ok(LoadedGhost::V3(_))));
        });
    }

    #[test]
    fn no_table_is_unsupported_without_legacy() {
        // loadu・load はあるが、shiori_get_api があるので進まない
        in_mode(0, || {
            assert!(matches!(
                with(&EntryKind::ALL),
                Err(LoadError::Unsupported(_))
            ));
        });
    }

    #[test]
    fn load_failed_stops() {
        let _g = test_dll::lock();
        let dir = Path::new(GHOST_DIR).join("fail");
        let r = load_ghost(test_dll::dll_path(), &dir, NoHost);
        assert!(matches!(r, Err(LoadError::LoadFailed)));
    }

    #[test]
    fn allowed_kinds() {
        let _g = test_dll::lock();
        assert!(matches!(with(&[EntryKind::V3]), Ok(LoadedGhost::V3(_))));
        assert!(matches!(
            with(&[EntryKind::Loadu, EntryKind::Load]),
            Ok(LoadedGhost::Legacy {
                entry: EntryKind::Loadu,
                ..
            })
        ));
        assert!(matches!(
            with(&[EntryKind::Load]),
            Ok(LoadedGhost::Legacy {
                entry: EntryKind::Load,
                ..
            })
        ));
        // shiori_get_api があっても V4・V3 を許可しなければ無いものとして従来の口へ
        assert!(matches!(
            with(&[EntryKind::Loadu]),
            Ok(LoadedGhost::Legacy {
                entry: EntryKind::Loadu,
                ..
            })
        ));
        assert!(matches!(with(&[]), Err(LoadError::NotShiori4)));
    }

    #[test]
    fn allowed_version_missing_is_unsupported_without_legacy() {
        // 許可した版が無くても shiori_get_api があるので従来の口へ進まない
        in_mode(test_dll::MODE_V3, || {
            assert!(matches!(
                with(&[EntryKind::V4, EntryKind::Loadu]),
                Err(LoadError::Unsupported(_))
            ));
        });
        in_mode(test_dll::MODE_V4, || {
            assert!(matches!(
                with(&[EntryKind::V3, EntryKind::Load]),
                Err(LoadError::Unsupported(_))
            ));
        });
    }

    #[test]
    fn os_library_is_not_shiori4() {
        // x86 の Windows の発見の口は相対のライブラリのパスを断るので、絶対パスで渡す
        #[cfg(windows)]
        let os_lib = std::path::PathBuf::from(std::env::var_os("SystemRoot").expect("SystemRoot"))
            .join(r"System32\kernel32.dll");
        #[cfg(target_os = "macos")]
        let os_lib = "/usr/lib/libSystem.B.dylib";
        #[cfg(all(unix, not(target_os = "macos")))]
        let os_lib = "libc.so.6";
        assert!(matches!(
            load_ghost(os_lib, GHOST_DIR, NoHost),
            Err(LoadError::NotShiori4)
        ));
    }

    #[cfg(all(windows, target_arch = "x86"))]
    #[test]
    fn relative_library_path_is_invalid_path() {
        for rel in [
            "legacyghost.dll",
            "kernel32.dll",
            r"\Windows\System32\kernel32.dll",
            "C:kernel32.dll",
        ] {
            assert!(
                matches!(
                    load_ghost(rel, GHOST_DIR, NoHost),
                    Err(LoadError::InvalidPath)
                ),
                "{rel}"
            );
        }
    }

    #[cfg(all(windows, target_arch = "x86"))]
    #[test]
    fn missing_absolute_library_is_library() {
        assert!(matches!(
            load_ghost(r"C:\no-such-dir\no-such-library.dll", GHOST_DIR, NoHost),
            Err(LoadError::Library(_))
        ));
    }

    #[test]
    fn relative_dir_is_invalid_path() {
        // ライブラリのパスは存在しない。dir の検査が先なので Library ではなく InvalidPath になる
        assert!(matches!(
            load_ghost("no-such-library", "ghost", NoHost),
            Err(LoadError::InvalidPath)
        ));
    }
}
