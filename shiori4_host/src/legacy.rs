//! SHIORI/3.0 の従来の口（`load`・`loadu`・`request`・`unload`）を持つ DLL を読む部分（x86 の Windows だけ）
//!
//! HGLOBAL の持ち主の決まり: 入力はホストが確保して DLL が解放し、応答は DLL が確保してホストが解放する。

use std::ffi::c_void;
#[cfg(test)]
use std::sync::atomic::{AtomicU32, Ordering};

use crate::{EntryKind, LoadError};

const GMEM_FIXED: u32 = 0;
const CP_ACP: u32 = 0;
const CP_UTF8: u32 = 65001;
const WC_NO_BEST_FIT_CHARS: u32 = 0x400;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GlobalAlloc(flags: u32, bytes: usize) -> *mut c_void;
    fn GlobalFree(h: *mut c_void) -> *mut c_void;
    fn GetACP() -> u32;
    fn WideCharToMultiByte(
        code_page: u32,
        flags: u32,
        wide: *const u16,
        wide_len: i32,
        out: *mut u8,
        out_len: i32,
        default_char: *const u8,
        used_default_char: *mut i32,
    ) -> i32;
}

/// 従来の DLL の要求の失敗
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyRequestError {
    /// 要求の入力のメモリ（GlobalAlloc）を確保できなかった。DLL は呼んでいない
    AllocFailed,
    /// request が NULL を返した
    NullResponse,
    /// request が負の長さを返した（応答のメモリは解放した）
    NegativeLength(i32),
    /// 文字列の口で、応答（末尾の NUL を落とした後）が UTF-8 でない
    InvalidUtf8,
}

/// global_alloc_copy が確保した数
#[cfg(test)]
pub(crate) static INPUTS_ALLOCATED: AtomicU32 = AtomicU32::new(0);
/// take_response が解放した数
#[cfg(test)]
pub(crate) static RESPONSES_FREED: AtomicU32 = AtomicU32::new(0);

/// GMEM_FIXED で max(bytes.len(), 1) バイトを確保し、bytes をちょうど len バイトだけ書く（NUL を付けない）。
/// 返したハンドルの持ち主は呼んだ先の DLL。確保できなければ None
#[allow(dead_code)] // LegacyRuntime（task 3.2・3.4）が使う
fn global_alloc_copy(bytes: &[u8]) -> Option<*mut c_void> {
    // GlobalAlloc(GMEM_FIXED, 0) の振る舞いは文書で保証されないので、最低 1 バイトを確保する
    let h = unsafe { GlobalAlloc(GMEM_FIXED, bytes.len().max(1)) };
    if h.is_null() {
        return None;
    }
    // GMEM_FIXED のハンドルは先頭のポインタ
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), h.cast::<u8>(), bytes.len()) };
    #[cfg(test)]
    INPUTS_ALLOCATED.fetch_add(1, Ordering::SeqCst);
    Some(h)
}

/// request の戻り値を受け取る。NULL は NullResponse。len < 0 は GlobalFree して NegativeLength。
/// それ以外は len バイトだけ写し（NUL を探さない）、GlobalFree して返す
///
/// # Safety
/// p は DLL が GMEM_FIXED で確保した、少なくとも len バイトの領域か NULL
#[allow(dead_code)] // LegacyRuntime（task 3.4）が使う
unsafe fn take_response(p: *mut c_void, len: i32) -> Result<Vec<u8>, LegacyRequestError> {
    if p.is_null() {
        return Err(LegacyRequestError::NullResponse);
    }
    let out = if len < 0 {
        Err(LegacyRequestError::NegativeLength(len))
    } else {
        Ok(unsafe { std::slice::from_raw_parts(p.cast::<u8>(), len as usize) }.to_vec())
    };
    unsafe { GlobalFree(p) };
    #[cfg(test)]
    RESPONSES_FREED.fetch_add(1, Ordering::SeqCst);
    out
}

/// entry が Loadu なら (dir の UTF-8, false)。Load なら to_acp(dir)。変換の API の失敗は LoadFailed
#[allow(dead_code)] // LegacyRuntime（task 3.2）が使う
fn dir_bytes(entry: EntryKind, dir: &str) -> Result<(Vec<u8>, bool), LoadError> {
    match entry {
        EntryKind::Load => to_acp(dir).ok_or(LoadError::LoadFailed),
        _ => Ok((dir.as_bytes().to_vec(), false)),
    }
}

/// CP_ACP のバイト列と「表せない文字があった」。GetACP() が 65001 なら (s の UTF-8, false)。
/// それ以外は WC_NO_BEST_FIT_CHARS で best-fit をせず、表せない文字は既定の文字（`?`）にして
/// used_default_char で知る。長さの問い合わせと変換の 2 回で呼び、NUL は出さない。0 が返れば None
#[allow(dead_code)] // dir_bytes と LegacyRuntime::dir_lossy（task 3.2）が使う
fn to_acp(s: &str) -> Option<(Vec<u8>, bool)> {
    // 65001 では WC_NO_BEST_FIT_CHARS と used_default_char が使えない。UTF-8 はそのまま CP_ACP のバイト列
    if unsafe { GetACP() } == CP_UTF8 {
        return Some((s.as_bytes().to_vec(), false));
    }
    // 長さ 0 では WideCharToMultiByte が失敗する
    if s.is_empty() {
        return Some((Vec::new(), false));
    }
    let wide: Vec<u16> = s.encode_utf16().collect();
    let wide_len = i32::try_from(wide.len()).ok()?;
    let convert = |out: *mut u8, out_len: i32, used: *mut i32| unsafe {
        WideCharToMultiByte(
            CP_ACP,
            WC_NO_BEST_FIT_CHARS,
            wide.as_ptr(),
            wide_len,
            out,
            out_len,
            std::ptr::null(),
            used,
        )
    };
    let n = convert(std::ptr::null_mut(), 0, std::ptr::null_mut());
    if n <= 0 {
        return None;
    }
    let mut out = vec![0u8; n as usize];
    let mut used = 0i32;
    let written = convert(out.as_mut_ptr(), n, &mut used);
    if written <= 0 {
        return None;
    }
    out.truncate(written as usize);
    Some((out, used != 0))
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard};

    use super::*;

    // 数を読むテストどうしを並べない
    static COUNTERS: Mutex<()> = Mutex::new(());
    fn counters() -> MutexGuard<'static, ()> {
        COUNTERS.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// DLL の代わりに応答を GMEM_FIXED で確保する（末尾に NUL を置く）
    fn dll_response(bytes: &[u8]) -> *mut c_void {
        let p = unsafe { GlobalAlloc(GMEM_FIXED, bytes.len() + 1) };
        assert!(!p.is_null());
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), p.cast::<u8>(), bytes.len());
            *p.cast::<u8>().add(bytes.len()) = 0;
        }
        p
    }

    #[test]
    fn alloc_copy_writes_exact_bytes_and_counts() {
        let _g = counters();
        let before = INPUTS_ALLOCATED.load(Ordering::SeqCst);
        let h = global_alloc_copy(b"abc").expect("alloc");
        let got = unsafe { std::slice::from_raw_parts(h.cast::<u8>(), 3) }.to_vec();
        assert_eq!(got, b"abc");
        assert_eq!(INPUTS_ALLOCATED.load(Ordering::SeqCst), before + 1);
        // 持ち主は DLL。ここでは DLL の代わりに解放する
        assert!(unsafe { GlobalFree(h) }.is_null());
    }

    #[test]
    fn alloc_copy_empty_returns_a_handle() {
        let _g = counters();
        let before = INPUTS_ALLOCATED.load(Ordering::SeqCst);
        let h = global_alloc_copy(b"").expect("alloc of 0 bytes still returns a handle");
        assert!(!h.is_null());
        assert_eq!(INPUTS_ALLOCATED.load(Ordering::SeqCst), before + 1);
        assert!(unsafe { GlobalFree(h) }.is_null());
    }

    #[test]
    fn take_response_copies_len_bytes_and_frees() {
        let _g = counters();
        let before = RESPONSES_FREED.load(Ordering::SeqCst);
        // NUL を探さない: 途中の NUL も len までは写す
        let p = dll_response(b"ab\0cd");
        assert_eq!(unsafe { take_response(p, 5) }, Ok(b"ab\0cd".to_vec()));
        assert_eq!(RESPONSES_FREED.load(Ordering::SeqCst), before + 1);
    }

    #[test]
    fn take_response_len_shorter_than_buffer_copies_only_len() {
        let _g = counters();
        let p = dll_response(b"abcdef");
        assert_eq!(unsafe { take_response(p, 2) }, Ok(b"ab".to_vec()));
    }

    #[test]
    fn take_response_zero_len_is_empty_and_frees() {
        let _g = counters();
        let before = RESPONSES_FREED.load(Ordering::SeqCst);
        let p = dll_response(b"");
        assert_eq!(unsafe { take_response(p, 0) }, Ok(Vec::new()));
        assert_eq!(RESPONSES_FREED.load(Ordering::SeqCst), before + 1);
    }

    #[test]
    fn take_response_null_is_error_and_frees_nothing() {
        let _g = counters();
        let before = RESPONSES_FREED.load(Ordering::SeqCst);
        assert_eq!(
            unsafe { take_response(std::ptr::null_mut(), 3) },
            Err(LegacyRequestError::NullResponse)
        );
        assert_eq!(RESPONSES_FREED.load(Ordering::SeqCst), before);
    }

    #[test]
    fn take_response_negative_len_is_error_and_frees() {
        let _g = counters();
        let before = RESPONSES_FREED.load(Ordering::SeqCst);
        let p = dll_response(b"x");
        assert_eq!(
            unsafe { take_response(p, -1) },
            Err(LegacyRequestError::NegativeLength(-1))
        );
        assert_eq!(RESPONSES_FREED.load(Ordering::SeqCst), before + 1);
    }

    #[test]
    fn to_acp_ascii_is_utf8_and_not_lossy() {
        let s = r"C:\ghost\master\";
        assert_eq!(to_acp(s), Some((s.as_bytes().to_vec(), false)));
    }

    #[test]
    fn to_acp_empty_is_empty() {
        assert_eq!(to_acp(""), Some((Vec::new(), false)));
    }

    #[test]
    fn to_acp_surrogate_pair_emoji_is_question_and_lossy() {
        let s = "a\u{1F600}b";
        let (bytes, lossy) = to_acp(s).expect("convert");
        if unsafe { GetACP() } == CP_UTF8 {
            assert_eq!((bytes, lossy), (s.as_bytes().to_vec(), false));
            return;
        }
        assert!(lossy);
        // 写った所は `?` だけ（best-fit の似た文字にならない）
        let (first, rest) = bytes.split_first().expect("non-empty");
        let (last, middle) = rest.split_last().expect("at least 2 bytes");
        assert_eq!((*first, *last), (b'a', b'b'));
        assert!(
            !middle.is_empty() && middle.iter().all(|&c| c == b'?'),
            "{bytes:?}"
        );
    }

    #[test]
    fn dir_bytes_loadu_is_utf8_itself() {
        let s = "C:\\ghost\\\u{1F600}\u{3042}\\";
        assert_eq!(
            dir_bytes(EntryKind::Loadu, s).ok(),
            Some((s.as_bytes().to_vec(), false))
        );
    }

    #[test]
    fn dir_bytes_load_is_to_acp() {
        let s = "C:\\ghost\\\u{1F600}\\";
        assert_eq!(dir_bytes(EntryKind::Load, s).ok(), to_acp(s));
        assert_eq!(
            dir_bytes(EntryKind::Load, "C:\\g\\").ok(),
            Some((b"C:\\g\\".to_vec(), false))
        );
    }
}
