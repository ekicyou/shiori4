//! shiori4_host のテスト用 cdylib（SHIORI/3.0 の従来の口）
//!
//! - `loadu`・`load`: 入力を記録して `GlobalFree`。dir に `fail` を含むと 0、それ以外は設定した戻り値（既定 1）
//! - `request`: 入力を記録して `GlobalFree`。応答の種類（OK・NULL・負の長さ・ECHO）で返し分ける。
//!   応答は長さ＋1 バイトを `GMEM_FIXED` で確保して末尾に NUL を置き、`*len` は NUL を含めない
//! - `unload`: 数えて 1 を返す
//! - `legacyghost_*`: 数・最後の入力を読む／状態を既定に戻すテスト用の書き出し
//! - `shiori_get_api` は持たない（既定の発見の順で `loadu` に解決させる）

use std::{
    ffi::c_void,
    sync::{
        Mutex,
        atomic::{AtomicI32, AtomicU32, Ordering},
    },
};

const GMEM_FIXED: u32 = 0;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GlobalAlloc(flags: u32, bytes: usize) -> *mut c_void;
    fn GlobalFree(h: *mut c_void) -> *mut c_void;
}

/// 応答の種類: 既定の SHIORI/3.0 200 OK
pub const RESPONSE_OK: u32 = 0;
/// 応答の種類: NULL を返す
pub const RESPONSE_NULL: u32 = 1;
/// 応答の種類: 応答を確保して `*len = -1`
pub const RESPONSE_NEGATIVE: u32 = 2;
/// 応答の種類: 受けた要求をそのまま返す
pub const RESPONSE_ECHO: u32 = 3;

const OK_RESPONSE: &[u8] = b"SHIORI/3.0 200 OK\r\nCharset: UTF-8\r\nValue: legacyghost\r\n\r\n";

#[repr(C)]
pub struct LegacyStats {
    pub loadu: u32,
    pub load: u32,
    pub request: u32,
    pub unload: u32,
    pub inputs_freed: u32,
    pub responses_allocated: u32,
}

static LOADU: AtomicU32 = AtomicU32::new(0);
static LOAD: AtomicU32 = AtomicU32::new(0);
static REQUEST: AtomicU32 = AtomicU32::new(0);
static UNLOAD: AtomicU32 = AtomicU32::new(0);
static INPUTS_FREED: AtomicU32 = AtomicU32::new(0);
static RESPONSES_ALLOCATED: AtomicU32 = AtomicU32::new(0);
static RESPONSE_KIND: AtomicU32 = AtomicU32::new(RESPONSE_OK);
static LOAD_RESULT: AtomicI32 = AtomicI32::new(1);
static LAST_INPUT: Mutex<Vec<u8>> = Mutex::new(Vec::new());

/// 入力の HGLOBAL（GMEM_FIXED なのでハンドルがそのまま先頭）を写して記録し、DLL の側で解放する
///
/// # Safety
/// `h` は NULL か、`len` バイト以上を読める `GlobalAlloc` のメモリ
unsafe fn take_input(h: *mut c_void, len: i32) -> Vec<u8> {
    let bytes = if h.is_null() || len <= 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(h as *const u8, len as usize) }.to_vec()
    };
    if !h.is_null() {
        unsafe { GlobalFree(h) };
        INPUTS_FREED.fetch_add(1, Ordering::SeqCst);
    }
    *LAST_INPUT.lock().unwrap() = bytes.clone();
    bytes
}

fn load_common(h: *mut c_void, len: i32) -> i32 {
    let dir = unsafe { take_input(h, len) };
    if dir.windows(4).any(|w| w == b"fail") {
        0
    } else {
        LOAD_RESULT.load(Ordering::SeqCst)
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn loadu(h: *mut c_void, len: i32) -> i32 {
    LOADU.fetch_add(1, Ordering::SeqCst);
    load_common(h, len)
}

#[unsafe(no_mangle)]
pub extern "C" fn load(h: *mut c_void, len: i32) -> i32 {
    LOAD.fetch_add(1, Ordering::SeqCst);
    load_common(h, len)
}

/// # Safety
/// `h` は NULL か `*len` バイトの `GlobalAlloc` のメモリ、`len` は読み書きできる i32
#[unsafe(no_mangle)]
pub unsafe extern "C" fn request(h: *mut c_void, len: *mut i32) -> *mut c_void {
    REQUEST.fetch_add(1, Ordering::SeqCst);
    let in_len = if len.is_null() { 0 } else { unsafe { *len } };
    let input = unsafe { take_input(h, in_len) };
    let kind = RESPONSE_KIND.load(Ordering::SeqCst);
    if kind == RESPONSE_NULL {
        return std::ptr::null_mut();
    }
    let body: &[u8] = if kind == RESPONSE_ECHO {
        &input
    } else {
        OK_RESPONSE
    };
    let p = unsafe { GlobalAlloc(GMEM_FIXED, body.len() + 1) } as *mut u8;
    if p.is_null() {
        return std::ptr::null_mut();
    }
    RESPONSES_ALLOCATED.fetch_add(1, Ordering::SeqCst);
    unsafe {
        std::ptr::copy_nonoverlapping(body.as_ptr(), p, body.len());
        *p.add(body.len()) = 0;
        if !len.is_null() {
            *len = if kind == RESPONSE_NEGATIVE {
                -1
            } else {
                body.len() as i32
            };
        }
    }
    p as *mut c_void
}

#[unsafe(no_mangle)]
pub extern "C" fn unload() -> i32 {
    UNLOAD.fetch_add(1, Ordering::SeqCst);
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn legacyghost_reset() {
    for c in [
        &LOADU,
        &LOAD,
        &REQUEST,
        &UNLOAD,
        &INPUTS_FREED,
        &RESPONSES_ALLOCATED,
    ] {
        c.store(0, Ordering::SeqCst);
    }
    RESPONSE_KIND.store(RESPONSE_OK, Ordering::SeqCst);
    LOAD_RESULT.store(1, Ordering::SeqCst);
    LAST_INPUT.lock().unwrap().clear();
}

#[unsafe(no_mangle)]
pub extern "C" fn legacyghost_set_response(kind: u32) {
    RESPONSE_KIND.store(kind, Ordering::SeqCst);
}

#[unsafe(no_mangle)]
pub extern "C" fn legacyghost_set_load_result(v: i32) {
    LOAD_RESULT.store(v, Ordering::SeqCst);
}

/// # Safety
/// `out` は書き込める `LegacyStats`
#[unsafe(no_mangle)]
pub unsafe extern "C" fn legacyghost_stats(out: *mut LegacyStats) {
    let s = LegacyStats {
        loadu: LOADU.load(Ordering::SeqCst),
        load: LOAD.load(Ordering::SeqCst),
        request: REQUEST.load(Ordering::SeqCst),
        unload: UNLOAD.load(Ordering::SeqCst),
        inputs_freed: INPUTS_FREED.load(Ordering::SeqCst),
        responses_allocated: RESPONSES_ALLOCATED.load(Ordering::SeqCst),
    };
    unsafe { out.write(s) };
}

/// 最後に受けた入力（load・loadu・request）の長さを返し、`cap` バイトまで `out` に写す
///
/// # Safety
/// `out` は `cap` バイト書き込める（`cap` が 0 なら NULL でよい）
#[unsafe(no_mangle)]
pub unsafe extern "C" fn legacyghost_last_input(out: *mut u8, cap: usize) -> usize {
    let last = LAST_INPUT.lock().unwrap();
    let n = last.len().min(cap);
    if n > 0 {
        unsafe { std::ptr::copy_nonoverlapping(last.as_ptr(), out, n) };
    }
    last.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(s: &[u8]) -> *mut c_void {
        let h = unsafe { GlobalAlloc(GMEM_FIXED, s.len()) };
        unsafe { std::ptr::copy_nonoverlapping(s.as_ptr(), h as *mut u8, s.len()) };
        h
    }

    fn stats() -> LegacyStats {
        let mut s = std::mem::MaybeUninit::uninit();
        unsafe {
            legacyghost_stats(s.as_mut_ptr());
            s.assume_init()
        }
    }

    // 状態はプロセスに 1 つなので 1 つのテストで通す
    #[test]
    fn round_trip_counts_and_kinds() {
        legacyghost_reset();
        assert_eq!(loadu(input(b"C:\\fail\\"), 8), 0);
        legacyghost_set_load_result(2);
        assert_eq!(load(input(b"C:\\g\\"), 5), 2);

        let req = b"GET SHIORI/3.0\r\n\r\n";
        let mut len = req.len() as i32;
        let r = unsafe { request(input(req), &mut len) };
        assert_eq!(len as usize, OK_RESPONSE.len());
        assert_eq!(unsafe { *(r as *const u8).add(OK_RESPONSE.len()) }, 0);
        unsafe { GlobalFree(r) };

        legacyghost_set_response(RESPONSE_ECHO);
        let mut len = req.len() as i32;
        let r = unsafe { request(input(req), &mut len) };
        assert_eq!(
            unsafe { std::slice::from_raw_parts(r as *const u8, len as usize) },
            req
        );
        unsafe { GlobalFree(r) };

        legacyghost_set_response(RESPONSE_NEGATIVE);
        let r = unsafe { request(input(req), &mut len) };
        assert_eq!(len, -1);
        unsafe { GlobalFree(r) };

        legacyghost_set_response(RESPONSE_NULL);
        let mut len = req.len() as i32;
        assert!(unsafe { request(input(req), &mut len) }.is_null());
        assert_eq!(unload(), 1);

        let mut buf = [0u8; 4];
        assert_eq!(
            unsafe { legacyghost_last_input(buf.as_mut_ptr(), 4) },
            req.len()
        );
        assert_eq!(&buf, b"GET ");

        let s = stats();
        assert_eq!((s.loadu, s.load, s.request, s.unload), (1, 1, 4, 1));
        assert_eq!((s.inputs_freed, s.responses_allocated), (6, 3));

        legacyghost_reset();
        let s = stats();
        assert_eq!((s.request, s.inputs_freed), (0, 0));
        assert_eq!(
            unsafe { legacyghost_last_input(std::ptr::null_mut(), 0) },
            0
        );
    }
}
