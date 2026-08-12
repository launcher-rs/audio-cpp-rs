//! # audio-cpp-sys
//!
//! audio.cpp（基于 ggml 的本地音频推理引擎）的底层 FFI 绑定。
//!
//! 本 crate 只做两件事：
//! 1. 在 `build.rs` 里用 CMake 构建上游 `engine_runtime` 静态库，并用
//!    bindgen 从 `capi.h` 生成绑定；
//! 2. 在 `lib.rs` 里把生成的绑定导出为 `audio_cpp_sys::*`。
//!
//! 高层、安全的 API 位于 `audio-cpp` crate。

#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

// 由 build.rs 生成的 C ABI 绑定（来自 capi.h）。
//
// 句柄类型：
// - `audiocpp_registry` —— 模型注册表；
// - `audiocpp_model`    —— 已加载的模型；
// - `audiocpp_session`  —— 一次任务会话（离线或流式）。
//
// 约定：函数返回 0 成功 / 非 0 出错；错误信息由 `audiocpp_last_error()`
// 获取；`char*` / `float*` 返回值非 NULL 时需用对应的 free() 释放。
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));