//! # vad_offline_ffi — 用 silero_vad 模型对一段音频做离线语音活动检测（VAD）
//!
//! 该示例演示完整的 registry → model → session → run 调用链：
//!
//! 1. 创建默认注册表，加载 silero_vad 模型（需要 `silero_vad_16k.safetensors`
//!    权重文件）；
//! 2. 在模型上创建离线（offline）VAD 任务会话；
//! 3. 通过 JSON 请求传入 WAV 文件（shim 内部会读取为 float 采样）；
//! 4. 打印返回的 `speech_segments`（每个语音片段的起止采样点与置信度）。
//!
//! 运行方式（第二个参数为模型权重路径，第三个为音频路径）：
//! ```bash
//! cargo run -p audio-cpp-sys --example vad_offline_ffi -- \
//!     ./silero_vad_16k.safetensors ./speech.wav
//! ```
//!
//! 权重下载：<https://huggingface.co/audio-cpp/audio.cpp-gguf>（或官方
//! audio.cpp 仓库的 model 说明）。音频建议 16k 单声道 WAV。

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

use audio_cpp_sys::*;

/// 把 shim 返回的 `char*` 转换为 UTF-8 字符串并立即释放。
unsafe fn take_string(ptr: *mut c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let s = CStr::from_ptr(ptr).to_string_lossy().into_owned();
    audiocpp_free_string(ptr);
    s
}

/// 读取最近一次错误信息。
unsafe fn last_error(default: &str) -> String {
    let p = audiocpp_last_error();
    if p.is_null() {
        default.to_string()
    } else {
        CStr::from_ptr(p).to_string_lossy().into_owned()
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!(
            "用法: vad_offline <silero_vad_16k.safetensors> <input.wav>\n\
             例如: cargo run -p audio-cpp-sys --example vad_offline -- \
             ./silero_vad_16k.safetensors ./speech.wav"
        );
        std::process::exit(1);
    }
    let model_path = &args[1];
    let wav_path = &args[2];

    // 1. 创建默认注册表。
    let registry = unsafe { audiocpp_registry_default() };
    assert!(!registry.is_null(), "创建默认注册表失败: {}", unsafe { last_error("未知错误") });

    // 2. 加载 silero_vad 模型。
    let model_path_c = CString::new(model_path.as_str()).expect("模型路径含 NUL");
    let model = unsafe { audiocpp_registry_load(registry, model_path_c.as_ptr(), ptr::null(), ptr::null()) };
    if model.is_null() {
        let msg = unsafe { last_error("未知错误") };
        unsafe { audiocpp_registry_free(registry) };
        panic!("加载模型失败: {msg}");
    }
    println!("模型加载成功: {model_path}");

    // 3. 创建离线 VAD 会话。
    //    task="vad", mode="offline", backend="cpu", device=0, threads=4。
    let task = CString::new("vad").unwrap();
    let mode = CString::new("offline").unwrap();
    let backend = CString::new("cpu").unwrap();
    let session = unsafe {
        audiocpp_model_create_task_session(
            model,
            task.as_ptr(),
            mode.as_ptr(),
            backend.as_ptr(),
            0, // device
            4, // threads
            ptr::null(),
        )
    };
    if session.is_null() {
        let msg = unsafe { last_error("未知错误") };
        unsafe {
            audiocpp_model_free(model);
            audiocpp_registry_free(registry);
        }
        panic!("创建 VAD 会话失败: {msg}");
    }
    println!("会话已创建（family={}）", unsafe {
        let p = audiocpp_session_family(session);
        CStr::from_ptr(p).to_string_lossy().into_owned()
    });

    // 4. 构造请求 JSON：通过 audio_path 传入 WAV 文件。
    let request_json = format!(
        r#"{{"audio_path":"{}","options":{{"vad_threshold":0.5}}}}"#,
        wav_path.replace('\\', "\\\\").replace('"', "\\\"")
    );
    let request_c = CString::new(request_json).expect("请求含 NUL");

    // 5. 离线执行，取回 TaskResult JSON。
    let mut result_json = ptr::null_mut();
    let rc = unsafe { audiocpp_session_run_offline(session, request_c.as_ptr(), &mut result_json) };
    if rc != 0 {
        let msg = unsafe { last_error("未知错误") };
        unsafe {
            audiocpp_session_free(session);
            audiocpp_model_free(model);
            audiocpp_registry_free(registry);
        }
        panic!("离线运行失败: {msg}");
    }
    let result = unsafe { take_string(result_json) };
    println!("=== VAD 结果 ===\n{result}");

    // 6. 释放所有句柄。
    unsafe {
        audiocpp_session_free(session);
        audiocpp_model_free(model);
        audiocpp_registry_free(registry);
    }
    println!("\nvad_offline 完成");
}
