//! 配布の決まり: 対象の OS・arch から SHIORI のファイル名の候補を並べ、ファイルの先頭のヘッダから機械の種類が合うかを確かめる（ライブラリとして読み込まない）。決まりの本文は C ヘッダの「配布の決まり」

use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};

/// 候補を並べる対象の OS
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetOs {
    Windows,
    Linux,
    MacOs,
}

/// 候補を並べる・機械の種類を確かめる対象の arch。x86 は表に無い（接尾辞なしの Windows の DLL が担う）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetArch {
    X64,
    Arm64,
}

impl TargetOs {
    /// 今のプロセスの OS。表に無い OS なら None
    pub const CURRENT: Option<TargetOs> = if cfg!(windows) {
        Some(TargetOs::Windows)
    } else if cfg!(target_os = "linux") {
        Some(TargetOs::Linux)
    } else if cfg!(target_os = "macos") {
        Some(TargetOs::MacOs)
    } else {
        None
    };
}

impl TargetArch {
    /// 今のプロセスの arch。x86 など表に無い arch なら None
    pub const CURRENT: Option<TargetArch> = if cfg!(target_arch = "x86_64") {
        Some(TargetArch::X64)
    } else if cfg!(target_arch = "aarch64") {
        Some(TargetArch::Arm64)
    } else {
        None
    };
}

/// 名前の幹（または descript.txt の `shiori,` の値）から、探す順にファイル名を並べる。
///
/// 値を最後の `/` か `\` で「ディレクトリの部分」と「名前の部分」に分け、名前の部分から
/// 末尾の `.dll`（ASCII の大文字小文字を区別しない）を 1 つ落とした残りを幹とする。
/// 候補はディレクトリの部分をそのまま前に付け、名前の部分にだけ規則を当てる
/// （例 `bin/pasta.dll` → `bin/pasta.x64.dll`・`bin/pasta.dll`）。
/// `can_spawn_x64` は Windows arm64 でだけ効き、macOS では arch を見ない。
///
/// ファイルにもライブラリにも触れない。順と規則の詳しくは C ヘッダ（`shiori4_abi.h`）の「配布の決まり」。
///
/// ```
/// use shiori4_host::{shiori_file_candidates, TargetArch, TargetOs};
/// assert_eq!(
///     shiori_file_candidates("pasta.dll", TargetOs::Linux, TargetArch::X64, false),
///     ["libpasta.x64.so", "libpasta.so"]
/// );
/// ```
pub fn shiori_file_candidates(
    name: &str,
    os: TargetOs,
    arch: TargetArch,
    can_spawn_x64: bool,
) -> Vec<String> {
    let split = name.rfind(['/', '\\']).map_or(0, |i| i + 1);
    let (dir, file) = name.split_at(split);
    let stem = match file
        .len()
        .checked_sub(4)
        .and_then(|i| file.get(i..).map(|ext| (i, ext)))
    {
        Some((i, ext)) if ext.eq_ignore_ascii_case(".dll") => &file[..i],
        _ => file,
    };
    let names = match (os, arch) {
        (TargetOs::Windows, TargetArch::X64) => {
            vec![format!("{stem}.x64.dll"), format!("{stem}.dll")]
        }
        (TargetOs::Windows, TargetArch::Arm64) => {
            let mut v = vec![format!("{stem}.arm64.dll")];
            if can_spawn_x64 {
                v.push(format!("{stem}.x64.dll"));
            }
            v.push(format!("{stem}.dll"));
            v
        }
        (TargetOs::Linux, TargetArch::X64) => {
            vec![format!("lib{stem}.x64.so"), format!("lib{stem}.so")]
        }
        (TargetOs::Linux, TargetArch::Arm64) => {
            vec![format!("lib{stem}.arm64.so"), format!("lib{stem}.so")]
        }
        (TargetOs::MacOs, _) => vec![format!("lib{stem}.dylib")],
    };
    names.into_iter().map(|n| format!("{dir}{n}")).collect()
}

/// 先頭から読む量の上限。PE だけは、さらに `e_lfanew` の先の 6 バイトを読む
const HEAD_LIMIT: u64 = 4096;

/// ファイル先頭のヘッダ（PE・ELF・Mach-O の単体とユニバーサル）の機械の種類が `arch` と合うか。
///
/// ライブラリとして読み込まず、初期化コードも動かさない。読むのは先頭の最大 4 KiB と、
/// PE の `e_lfanew` の先の 6 バイトだけ。すべての読み取りは範囲を確かめ、パニックしない。
/// macOS のユニバーサルバイナリは、中に `arch` が含まれていれば合う。
///
/// - 形式が違う・壊れている・短い → `Ok(false)`
/// - 開けない・読めない → `Err`
///
/// # 2 つの例外（ヘッダの表すとおりに扱い、特別扱いしない）
/// - ARM64EC の DLL は Machine が x64（0x8664）なので `X64` に合う。x64 の実機では「合う」と出ても
///   読めない（`LoadError::Library` になる）
/// - ARM64X の DLL は Machine が arm64（0xAA64）なので `Arm64` に合う。ARM の Windows で x64 として
///   動くプロセスが `X64` で確かめると「合わない」と出る（実際には読める）
pub fn machine_matches(path: impl AsRef<Path>, arch: TargetArch) -> io::Result<bool> {
    let mut file = File::open(path)?;
    let mut head = Vec::new();
    (&mut file).take(HEAD_LIMIT).read_to_end(&mut head)?;

    if head.starts_with(b"MZ") {
        let Some(e_lfanew) = u32_at(&head, 0x3C, true) else {
            return Ok(false);
        };
        file.seek(SeekFrom::Start(u64::from(e_lfanew)))?;
        let mut pe = [0u8; 6];
        match file.read_exact(&mut pe) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(false),
            Err(e) => return Err(e),
        }
        if &pe[..4] != b"PE\0\0" {
            return Ok(false);
        }
        let want: u16 = match arch {
            TargetArch::X64 => 0x8664,
            TargetArch::Arm64 => 0xAA64,
        };
        return Ok(u16::from_le_bytes([pe[4], pe[5]]) == want);
    }

    if head.starts_with(b"\x7FELF") {
        // EI_CLASS = 2（64bit）だけ。EI_DATA: 1 = LE、2 = BE
        let le = match (head.get(4), head.get(5)) {
            (Some(2), Some(1)) => true,
            (Some(2), Some(2)) => false,
            _ => return Ok(false),
        };
        let want: u16 = match arch {
            TargetArch::X64 => 62,
            TargetArch::Arm64 => 183,
        };
        return Ok(u16_at(&head, 18, le) == Some(want));
    }

    let want: u32 = match arch {
        TargetArch::X64 => 0x0100_0007,
        TargetArch::Arm64 => 0x0100_000C,
    };
    if u32_at(&head, 0, true) == Some(0xFEED_FACF) {
        return Ok(u32_at(&head, 4, true) == Some(want));
    }
    let entry = match u32_at(&head, 0, false) {
        Some(0xCAFE_BABE) => 20,
        Some(0xCAFE_BABF) => 32,
        _ => return Ok(false),
    };
    // Java の class ファイルも 0xCAFEBABE で始まるが、続く値（版）は 45 以上なので 1〜30 で見分ける
    let nfat = match u32_at(&head, 4, false) {
        Some(n @ 1..=30) => n as usize,
        _ => return Ok(false),
    };
    Ok((0..nfat).any(|i| u32_at(&head, 8 + i * entry, false) == Some(want)))
}

fn u16_at(b: &[u8], off: usize, le: bool) -> Option<u16> {
    let a: [u8; 2] = b.get(off..off.checked_add(2)?)?.try_into().ok()?;
    Some(if le {
        u16::from_le_bytes(a)
    } else {
        u16::from_be_bytes(a)
    })
}

fn u32_at(b: &[u8], off: usize, le: bool) -> Option<u32> {
    let a: [u8; 4] = b.get(off..off.checked_add(4)?)?.try_into().ok()?;
    Some(if le {
        u32::from_le_bytes(a)
    } else {
        u32::from_be_bytes(a)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use TargetArch::*;
    use TargetOs::*;

    #[test]
    fn candidates_follow_the_table() {
        for name in ["pasta", "pasta.dll", "PASTA.DLL"] {
            let stem = &name[..5];
            let c = |os, arch, spawn| shiori_file_candidates(name, os, arch, spawn);
            assert_eq!(
                c(Windows, X64, false),
                [format!("{stem}.x64.dll"), format!("{stem}.dll")]
            );
            assert_eq!(
                c(Windows, X64, true),
                [format!("{stem}.x64.dll"), format!("{stem}.dll")]
            );
            assert_eq!(
                c(Windows, Arm64, false),
                [format!("{stem}.arm64.dll"), format!("{stem}.dll")]
            );
            assert_eq!(
                c(Windows, Arm64, true),
                [
                    format!("{stem}.arm64.dll"),
                    format!("{stem}.x64.dll"),
                    format!("{stem}.dll")
                ]
            );
            for spawn in [false, true] {
                assert_eq!(
                    c(Linux, X64, spawn),
                    [format!("lib{stem}.x64.so"), format!("lib{stem}.so")]
                );
                assert_eq!(
                    c(Linux, Arm64, spawn),
                    [format!("lib{stem}.arm64.so"), format!("lib{stem}.so")]
                );
                assert_eq!(c(MacOs, X64, spawn), [format!("lib{stem}.dylib")]);
                assert_eq!(c(MacOs, Arm64, spawn), [format!("lib{stem}.dylib")]);
            }
        }
    }

    #[test]
    fn candidates_keep_the_directory_part() {
        assert_eq!(
            shiori_file_candidates("bin/pasta.dll", Windows, X64, false),
            ["bin/pasta.x64.dll", "bin/pasta.dll"]
        );
        assert_eq!(
            shiori_file_candidates("bin/pasta.dll", Linux, X64, false),
            ["bin/libpasta.x64.so", "bin/libpasta.so"]
        );
        // 区切りは書き換えず、最後の区切りで分ける
        assert_eq!(
            shiori_file_candidates("a/b\\pasta.DLL", MacOs, Arm64, false),
            ["a/b\\libpasta.dylib"]
        );
        // 落とすのは名前の部分の末尾の .dll を 1 つだけ。ディレクトリの .dll は残す
        assert_eq!(
            shiori_file_candidates("x.dll/pasta.dll.dll", Linux, Arm64, false),
            ["x.dll/libpasta.dll.arm64.so", "x.dll/libpasta.dll.so"]
        );
        // 短い名前・多バイト文字でもパニックしない
        assert_eq!(shiori_file_candidates("", MacOs, X64, false), ["lib.dylib"]);
        assert_eq!(
            shiori_file_candidates("ぱすた", Windows, X64, false),
            ["ぱすた.x64.dll", "ぱすた.dll"]
        );
    }

    /// バイト列を一時ファイルに書いて、両方の arch で確かめる
    fn check(tag: &str, bytes: &[u8]) -> (bool, bool) {
        let path = std::env::temp_dir().join(format!("shiori4_dist_{}_{tag}", std::process::id()));
        std::fs::write(&path, bytes).unwrap();
        let r = (
            machine_matches(&path, X64).unwrap(),
            machine_matches(&path, Arm64).unwrap(),
        );
        std::fs::remove_file(&path).unwrap();
        r
    }

    fn pe(machine: u16) -> Vec<u8> {
        let mut b = vec![0u8; 0x80];
        b[..2].copy_from_slice(b"MZ");
        b[0x3C..0x40].copy_from_slice(&0x40u32.to_le_bytes());
        b[0x40..0x44].copy_from_slice(b"PE\0\0");
        b[0x44..0x46].copy_from_slice(&machine.to_le_bytes());
        b
    }

    fn elf(class: u8, data: u8, machine: u16) -> Vec<u8> {
        let mut b = vec![0u8; 64];
        b[..4].copy_from_slice(b"\x7FELF");
        b[4] = class;
        b[5] = data;
        let m = if data == 2 {
            machine.to_be_bytes()
        } else {
            machine.to_le_bytes()
        };
        b[18..20].copy_from_slice(&m);
        b
    }

    fn fat(magic: u32, cputypes: &[u32]) -> Vec<u8> {
        let entry = if magic == 0xCAFE_BABF { 32 } else { 20 };
        let mut b = magic.to_be_bytes().to_vec();
        b.extend((cputypes.len() as u32).to_be_bytes());
        for c in cputypes {
            let mut e = vec![0u8; entry];
            e[..4].copy_from_slice(&c.to_be_bytes());
            b.extend(e);
        }
        b
    }

    #[test]
    fn pe_machine() {
        assert_eq!(check("pe_x64", &pe(0x8664)), (true, false));
        assert_eq!(check("pe_arm64", &pe(0xAA64)), (false, true));
        assert_eq!(check("pe_x86", &pe(0x014C)), (false, false));
        // e_lfanew が 4 KiB の先でも読む
        let mut far = pe(0x8664);
        far[0x3C..0x40].copy_from_slice(&0x2000u32.to_le_bytes());
        far.resize(0x2006, 0);
        far[0x2000..0x2004].copy_from_slice(b"PE\0\0");
        far[0x2004..0x2006].copy_from_slice(&0x8664u16.to_le_bytes());
        assert_eq!(check("pe_far", &far), (true, false));
        // 署名が違う
        let mut bad = pe(0x8664);
        bad[0x40] = b'X';
        assert_eq!(check("pe_badsig", &bad), (false, false));
    }

    #[test]
    fn elf_machine() {
        assert_eq!(check("elf_x64", &elf(2, 1, 62)), (true, false));
        assert_eq!(check("elf_arm64", &elf(2, 1, 183)), (false, true));
        assert_eq!(check("elf_be", &elf(2, 2, 183)), (false, true));
        assert_eq!(check("elf_32", &elf(1, 1, 62)), (false, false));
        assert_eq!(check("elf_baddata", &elf(2, 3, 62)), (false, false));
    }

    #[test]
    fn macho_machine() {
        let mut thin = 0xFEED_FACFu32.to_le_bytes().to_vec();
        thin.extend(0x0100_000Cu32.to_le_bytes());
        thin.extend([0u8; 24]);
        assert_eq!(check("macho_arm64", &thin), (false, true));
        thin[4..8].copy_from_slice(&0x0100_0007u32.to_le_bytes());
        assert_eq!(check("macho_x64", &thin), (true, false));

        assert_eq!(
            check("fat_both", &fat(0xCAFE_BABE, &[0x0100_0007, 0x0100_000C])),
            (true, true)
        );
        assert_eq!(
            check("fat64_arm64", &fat(0xCAFE_BABF, &[0x0100_000C])),
            (false, true)
        );
        assert_eq!(
            check("fat_other", &fat(0xCAFE_BABE, &[0x0000_0007])),
            (false, false)
        );
        // nfat_arch が多いのに表が足りない
        let mut short = fat(0xCAFE_BABE, &[0x0100_0007]);
        short[4..8].copy_from_slice(&3u32.to_be_bytes());
        short[8..12].copy_from_slice(&0u32.to_be_bytes());
        assert_eq!(check("fat_short", &short), (false, false));
    }

    #[test]
    fn not_a_library_or_broken() {
        // Java の class: 0xCAFEBABE、minor 0、major 52
        let mut class = 0xCAFE_BABEu32.to_be_bytes().to_vec();
        class.extend([0, 0, 0, 52]);
        class.extend(0x0100_0007u32.to_be_bytes());
        class.extend([0u8; 32]);
        assert_eq!(check("java_class", &class), (false, false));
        assert_eq!(check("empty", b""), (false, false));
        assert_eq!(check("short_mz", b"MZ"), (false, false));
        assert_eq!(check("short_elf", b"\x7FELF\x02\x01"), (false, false));
        assert_eq!(
            check("short_fat", &[0xCA, 0xFE, 0xBA, 0xBE, 0, 0]),
            (false, false)
        );
        assert_eq!(check("text", b"hello, world"), (false, false));
        // e_lfanew がファイルの外・u32 の最大
        let mut out = pe(0x8664);
        out[0x3C..0x40].copy_from_slice(&0x1000u32.to_le_bytes());
        assert_eq!(check("pe_out", &out), (false, false));
        out[0x3C..0x40].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(check("pe_max", &out), (false, false));
    }

    #[test]
    fn missing_file_is_err() {
        let path =
            std::env::temp_dir().join(format!("shiori4_dist_{}_missing", std::process::id()));
        assert!(machine_matches(&path, X64).is_err());
    }

    #[test]
    fn current_exe_matches_current_arch() {
        let exe = std::env::current_exe().unwrap();
        if let Some(arch) = TargetArch::CURRENT {
            assert!(
                machine_matches(&exe, arch).unwrap(),
                "{exe:?} should match {arch:?}"
            );
        }
        assert!(
            TargetOs::CURRENT.is_some()
                || !cfg!(any(windows, target_os = "linux", target_os = "macos"))
        );
    }
}
