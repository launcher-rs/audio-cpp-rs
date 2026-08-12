//! # inspect — 枚举 audio.cpp 注册表与后端设备
//!
//! 该示例演示最底层的 C ABI 用法，无需任何模型文件即可运行，用于验证
//! `audio-cpp-sys` 的 FFI 链路（bindgen 绑定 + C shim + engine_runtime）完好：
//!
//! 1. 创建默认注册表；
//! 2. 枚举注册的模型族（families）与 loader 声明（loaders）；
//! 3. 枚举各后端可用的计算设备（devices）；
//! 4. 释放所有句柄与字符串。
//!
//! 运行方式：
//! ```bash
//! cargo run -p audio-cpp-sys --example inspect
//! ```

use std::ffi::CStr;
use std::os::raw::c_char;

use audio_cpp_sys::*;

/// 把 shim 返回的 `char*` 转换为 UTF-8 字符串并立即释放。
///
/// 约定：所有返回的 `char*` 归调用方所有，使用后必须调用
/// `audiocpp_free_string` 释放。
unsafe fn take_string(ptr: *mut c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let s = CStr::from_ptr(ptr).to_string_lossy().into_owned();
    audiocpp_free_string(ptr);
    s
}

fn main() {
    // 创建默认注册表（含所有编译进 engine_runtime 的模型族 loader）。
    let registry = unsafe { audiocpp_registry_default() };
    assert!(!registry.is_null(), "audiocpp_registry_default() 返回空指针");

    // 错误信息示例：注册表句柄为空时应捕获错误。
    let mut out = std::ptr::null_mut();
    let rc = unsafe { audiocpp_registry_families_json(std::ptr::null(), &mut out) };
    assert!(rc != 0, "空注册表调用应返回非 0 错误码");
    let last_error = unsafe {
        let p = audiocpp_last_error();
        if p.is_null() {
            String::from("(无错误信息)")
        } else {
            CStr::from_ptr(p).to_string_lossy().into_owned()
        }
    };
    println!("[错误路径验证] rc={rc} last_error={last_error}");

    // 枚举模型族。
    let mut families_json = std::ptr::null_mut();
    let rc = unsafe { audiocpp_registry_families_json(registry, &mut families_json) };
    assert_eq!(rc, 0, "audiocpp_registry_families_json 失败");
    let families = unsafe { take_string(families_json) };
    println!("=== 已注册模型族 ===\n{families}");

    // 枚举 loader 声明（模型族、能力、端点）。
    let mut loaders_json = std::ptr::null_mut();
    let rc = unsafe { audiocpp_registry_loaders_json(registry, &mut loaders_json) };
    assert_eq!(rc, 0, "audiocpp_registry_loaders_json 失败");
    let loaders = unsafe { take_string(loaders_json) };
    println!("=== loader 声明 ===\n{loaders}");

    // 枚举后端设备。
    let mut devices_json = std::ptr::null_mut();
    let rc = unsafe { audiocpp_registry_devices_json(&mut devices_json) };
    assert_eq!(rc, 0, "audiocpp_registry_devices_json 失败");
    let devices = unsafe { take_string(devices_json) };
    println!("=== 后端设备 ===\n{devices}");

    // 释放注册表。
    unsafe { audiocpp_registry_free(registry) };

    println!("\ninspect 完成");
}
