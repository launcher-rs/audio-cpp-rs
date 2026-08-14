//! 内部工具：把原始 C ABI 调用包装成安全的 `Result`。
//!
//! `audio-cpp-sys` 的约定（见 `capi.h`）：
//! - 函数返回 0 成功、非 0 出错；
//! - 出错时可用 `audiocpp_last_error()` 取得错误信息；
//! - 所有返回的 `char*` 归调用方所有，用 `audiocpp_free_string` 释放。

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use audio_cpp_sys::*;

use crate::error::Error;

/// 将 shim 返回的 `char*` 取出为 UTF-8 字符串并立即释放。
///
/// # Safety
///
/// `ptr` 必须是由 `audiocpp_*` 函数返回且尚未释放的 `char*`。
pub(crate) unsafe fn take_string(ptr: *mut c_char) -> Result<String, Error> {
    if ptr.is_null() {
        return Ok(String::new());
    }
    // SAFETY: 调用方保证 ptr 是 shim 返回的、尚未释放的 char*（见函数 Safety 说明）。
    let s = unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned();
    // SAFETY: 同上，shim 要求返回的 char* 用 audiocpp_free_string 释放。
    unsafe { audiocpp_free_string(ptr) };
    Ok(s)
}

/// 获取最近一次错误的描述信息（由 shim 持有，无需释放）。
pub(crate) fn last_error() -> String {
    unsafe {
        let p = audiocpp_last_error();
        if p.is_null() {
            String::from("(无错误信息)")
        } else {
            CStr::from_ptr(p).to_string_lossy().into_owned()
        }
    }
}

/// 检查 C ABI 返回值，非 0 时附加错误信息并返回 `Err`。
pub(crate) fn check_rc(rc: i32) -> Result<(), Error> {
    if rc == 0 {
        Ok(())
    } else {
        Err(Error::Ffi(last_error()))
    }
}

/// 构造 CString，用于把 UTF-8 字符串传给 C 边界。
pub(crate) fn cstring(s: &str) -> Result<CString, Error> {
    CString::new(s).map_err(Error::from)
}
