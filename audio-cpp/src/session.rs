//! 任务会话（Session）：离线执行 + 流式处理。
//!
//! 会话由 [`crate::Model::create_task_session`] 创建，按其任务与运行模式
//! 分为：
//! - 离线（`RunMode::Offline`）：`run()` 一次执行、一次结果；
//! - 流式（`RunMode::Streaming`）：`start()` → `process_audio()` → `finish()`，
//!   期间通过事件回调或 `process_audio` 的返回值接收流事件。
//!
//! # 事件回调
//!
//! 流式会话可在创建时调用 [`Session::set_event_callback`] 注册一个 Rust 闭包，
//! shim 会在每个流事件发生时回传给该闭包（详见 `capi.h` 的
//! `audiocpp_stream_event_cb`）。回调数据经 `Mutex` 保护，可从 C++ 侧线程调用。

use std::ffi::{c_char, c_void, CString};
use std::os::raw::{c_int, c_long};
use std::ptr;
use std::sync::Mutex;

use audio_cpp_sys::*;

use crate::error::Error;
use crate::ffi;
use crate::types::{StreamEvent, StreamingPolicy, TaskResult};

/// 事件回调的底层存储（box 在堆上，地址稳定，可跨线程）。
struct EventSinkInner {
    cb: Mutex<Box<dyn FnMut(StreamEvent) + Send>>,
}

/// shim 事件回调的 C 桥接函数。
///
/// `user_data` 指向 `EventSinkInner`。事件 JSON 由 shim 一次性构造，转为
/// Rust 类型后交给用户闭包。
unsafe extern "C" fn stream_event_cb(user_data: *mut c_void, event_json: *const c_char, _is_final: c_int) {
    if user_data.is_null() || event_json.is_null() {
        return;
    }
    let inner = user_data.cast::<EventSinkInner>();
    let json = std::ffi::CStr::from_ptr(event_json).to_string_lossy().into_owned();
    let event = match serde_json::from_str::<StreamEvent>(&json) {
        Ok(e) => e,
        Err(_) => return, // 事件 JSON 契约不符：忽略
    };
    let mut guard = match (*inner).cb.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    (guard)(event);
}

/// 任务会话。
pub struct Session {
    raw: *mut audiocpp_session,
    /// 事件回调的持有者；析构时负责释放并确保 C 侧不再引用。
    event_sink: Option<*mut EventSinkInner>,
}

// Session 持有回调 box（要求 Send）与 C 句柄。跨线程转移所有权是安全的，
// 但用户应避免同时对同一会话做流式调用与回调锁内嵌套调用。
unsafe impl Send for Session {}

impl Session {
    /// 从原始 C 句柄包装（仅内部使用）。
    pub(crate) fn from_raw(raw: *mut audiocpp_session) -> Self {
        Self { raw, event_sink: None }
    }

    /// 模型族名（借用的 C 字符串，即刻拷贝为 Rust 字符串）。
    pub fn family(&self) -> String {
        unsafe {
            let p = audiocpp_session_family(self.raw);
            if p.is_null() {
                String::new()
            } else {
                std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned()
            }
        }
    }

    /// 任务种类（如 `"vad"`）。
    pub fn task_kind(&self) -> String {
        unsafe {
            let p = audiocpp_session_task_kind(self.raw);
            if p.is_null() {
                String::new()
            } else {
                std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned()
            }
        }
    }

    /// 运行模式（如 `"offline"` / `"streaming"`）。
    pub fn run_mode(&self) -> String {
        unsafe {
            let p = audiocpp_session_run_mode(self.raw);
            if p.is_null() {
                String::new()
            } else {
                std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned()
            }
        }
    }

    /// 流式策略描述（输入/输出类型、推荐分块大小）。
    pub fn streaming_policy(&self) -> Result<StreamingPolicy, Error> {
        let mut out: *mut c_char = ptr::null_mut();
        ffi::check_rc(unsafe { audiocpp_session_streaming_policy_json(self.raw, &mut out) })?;
        let json = unsafe { ffi::take_string(out)? };
        serde_json::from_str(&json).map_err(Error::from)
    }

    /// 为一次请求做准备（离线与流式共用）。
    ///
    /// `request_json` 为 JSON 对象，携带 `audio` / `text` / `voice` 等输入，
    /// 或用 `audio_path` 指向本地 WAV 文件。若为 `None` 则传入空 JSON `{}`。
    pub fn prepare(&self, request_json: Option<&str>) -> Result<(), Error> {
        let req_c = match request_json {
            Some(s) => ffi::cstring(s)?,
            None => CString::new("{}").expect("'{}' 不含 NUL"),
        };
        ffi::check_rc(unsafe { audiocpp_session_prepare(self.raw, req_c.as_ptr() as *const c_char) })
    }

    /// 注册流式事件回调；调用 `None` 清除。
    ///
    /// 回调会在每次流事件产生时被调用（可能来自 C++ 侧线程），其内容是
    /// 解析为 [`StreamEvent`] 的对象。回调内不得再调用本会话的方法。
    pub fn set_event_callback<F>(&mut self, cb: Option<F>)
    where
        F: FnMut(StreamEvent) + Send + 'static,
    {
        // 先清理旧回调，保证 C 侧不再引用旧的 user_data。
        if let Some(old) = self.event_sink.take() {
            unsafe {
                audiocpp_session_set_event_sink(self.raw, None, ptr::null_mut());
                drop(Box::from_raw(old));
            }
        }
        if let Some(cb) = cb {
            let inner = Box::into_raw(Box::new(EventSinkInner {
                cb: Mutex::new(Box::new(cb)),
            }));
            unsafe {
                audiocpp_session_set_event_sink(self.raw, Some(stream_event_cb), inner.cast::<c_void>());
            }
            self.event_sink = Some(inner);
        } else {
            self.event_sink = None;
        }
    }

    /// 开始一次流式任务（先 `prepare` 再进入流式读取循环）。
    pub fn start(&self, request_json: Option<&str>) -> Result<(), Error> {
        let req_c = match request_json {
            Some(s) => ffi::cstring(s)?,
            None => CString::new("{}").expect("'{}' 不含 NUL"),
        };
        ffi::check_rc(unsafe { audiocpp_session_start(self.raw, req_c.as_ptr() as *const c_char) })
    }

    /// 将一段音频送入流式会话，返回该块触发的第一个事件（若有）。
    ///
    /// `samples` 为 `float` 采样（-1..1），`start_sample` 为该块在输入流中的
    /// 起始采样点。若同时注册了事件回调，事件会经回调到达；返回值是
    /// shim 同步返回的事件 JSON。
    pub fn process_audio(
        &self,
        samples: &[f32],
        sample_rate: i32,
        channels: i32,
        start_sample: i64,
    ) -> Result<Option<StreamEvent>, Error> {
        let mut out: *mut c_char = ptr::null_mut();
        ffi::check_rc(unsafe {
            audiocpp_session_process_audio(
                self.raw,
                samples.as_ptr(),
                samples.len(),
                sample_rate,
                channels,
                start_sample,
                &mut out,
            )
        })?;
        if out.is_null() {
            return Ok(None);
        }
        let json = unsafe { ffi::take_string(out)? };
        Ok(Some(serde_json::from_str(&json).map_err(Error::from)?))
    }

    /// 结束流式会话，返回最终 TaskResult。
    pub fn finish(&self) -> Result<TaskResult, Error> {
        let mut out: *mut c_char = ptr::null_mut();
        ffi::check_rc(unsafe { audiocpp_session_finish(self.raw, &mut out) })?;
        if out.is_null() {
            return Ok(TaskResult {
                speech_segments: Vec::new(),
                text_output: None,
                audio_output: None,
                named_audio_outputs: Vec::new(),
            });
        }
        let json = unsafe { ffi::take_string(out)? };
        serde_json::from_str(&json).map_err(Error::from)
    }

    /// 离线执行一次请求，返回 TaskResult。
    ///
    /// 相当于 `prepare` + 一次完整执行（shim 内部已做 prepare）。
    pub fn run_offline(&self, request_json: &str) -> Result<TaskResult, Error> {
        let req_c = ffi::cstring(request_json)?;
        let mut out: *mut c_char = ptr::null_mut();
        ffi::check_rc(unsafe { audiocpp_session_run_offline(self.raw, req_c.as_ptr() as *const c_char, &mut out) })?;
        if out.is_null() {
            return Err(Error::Ffi(ffi::last_error()));
        }
        let json = unsafe { ffi::take_string(out)? };
        serde_json::from_str(&json).map_err(Error::from)
    }

    /// 重置流式会话内部状态（可复用会话对象重新开始）。
    pub fn reset(&self) {
        unsafe {
            audiocpp_session_reset(self.raw);
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // 先清除事件回调，避免 C 侧在会话析构后再引用 user_data。
        if let Some(inner) = self.event_sink.take() {
            unsafe {
                audiocpp_session_set_event_sink(self.raw, None, ptr::null_mut());
                drop(Box::from_raw(inner));
            }
        }
        unsafe {
            audiocpp_session_free(self.raw);
        }
    }
}

// 供 process_audio / finish 内部使用：`c_long` 的别名保持与 C ABI 一致。
#[allow(dead_code)]
type RawLong = c_long;