//! shiori4_host のテスト用 cdylib
//!
//! - `shiori_get_api`: モードのビットで v4／v3 のテーブルを返し分ける（立っていない版は NULL）
//! - `testghost_set_mode`: モードを設定する（既定は [`MODE_DEFAULT`]）
//! - `loadu`・`load`: SHIORI/3.0 の従来の口のダミー。呼ばれない（有無だけを見る）
//! - Ghost の load は dir に `fail` を含むと失敗する

use std::{
    ffi::c_void,
    sync::atomic::{AtomicU32, Ordering},
};

use shiori4::protocol::HeaderWriter;
use shiori4::{
    Adapter, Ghost, ShioriApi, ShioriHostHandle, ShioriResponse,
    ShioriStatus, SHIORI_500_INTERNAL_SERVER_ERROR, SHIORI_ABI_VERSION, SHIORI_ABI_VERSION_3,
};

/// v4 のテーブルを出す
pub const MODE_V4: u32 = 1;
/// v3 のテーブルを出す
pub const MODE_V3: u32 = 2;
/// 既定: v4 と v3 の両方を出す
pub const MODE_DEFAULT: u32 = MODE_V4 | MODE_V3;

static MODE: AtomicU32 = AtomicU32::new(MODE_DEFAULT);

struct TestGhost;

impl Ghost for TestGhost {
    fn load(_host: Option<ShioriHostHandle>, dir: &str) -> Result<Self, ShioriStatus> {
        if dir.contains("fail") {
            return Err(SHIORI_500_INTERNAL_SERVER_ERROR);
        }
        Ok(Self)
    }

    fn request(&mut self, _req: &str) -> ShioriResponse {
        HeaderWriter::new().push("Value", "testghost").into_response()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn shiori_get_api(requested: u32) -> *const ShioriApi {
    static API_V4: ShioriApi = ShioriApi::new::<Adapter<TestGhost>, { SHIORI_ABI_VERSION }>();
    static API_V3: ShioriApi = ShioriApi::new::<Adapter<TestGhost>, { SHIORI_ABI_VERSION_3 }>();
    let mode = MODE.load(Ordering::SeqCst);
    match requested {
        SHIORI_ABI_VERSION if mode & MODE_V4 != 0 => &API_V4,
        SHIORI_ABI_VERSION_3 if mode & MODE_V3 != 0 => &API_V3,
        _ => std::ptr::null(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn testghost_set_mode(mode: u32) {
    MODE.store(mode, Ordering::SeqCst);
}

#[unsafe(no_mangle)]
pub extern "C" fn loadu(_h: *mut c_void, _len: isize) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn load(_h: *mut c_void, _len: isize) -> i32 {
    0
}
