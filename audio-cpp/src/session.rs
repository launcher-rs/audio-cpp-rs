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

use std::ffi::{CStr, c_char, c_void};
use std::os::raw::c_int;
use std::ptr;
use std::sync::{Arc, Mutex};

use audio_cpp_sys::*;

use crate::error::Error;
use crate::ffi;
use crate::model::Model;
use crate::request::IntoRequest;
use crate::types::{Backend, RunMode, StreamEvent, StreamingPolicy, TaskKind, TaskResult};

/// 事件回调的底层存储（box 在堆上，地址稳定，可跨线程）。
struct EventSinkInner {
    cb: Mutex<Box<dyn FnMut(StreamEvent) + Send>>,
}

/// shim 事件回调的 C 桥接函数。
///
/// `user_data` 指向 `EventSinkInner`。事件 JSON 由 shim 一次性构造，转为
/// Rust 类型后交给用户闭包。
unsafe extern "C" fn stream_event_cb(
    user_data: *mut c_void,
    event_json: *const c_char,
    _is_final: c_int,
) {
    if user_data.is_null() || event_json.is_null() {
        return;
    }
    // SAFETY: shim 保证 user_data 是 EventSinkInner 指针、event_json 是 NUL 结尾 JSON。
    let inner = user_data.cast::<EventSinkInner>();
    // SAFETY: 同上，event_json 是 NUL 结尾的 UTF-8 JSON。
    let json = unsafe { CStr::from_ptr(event_json) }
        .to_string_lossy()
        .into_owned();
    let Ok(event) = serde_json::from_str::<StreamEvent>(&json) else {
        return; // 事件 JSON 契约不符：忽略
    };
    // SAFETY: 同上，inner 在会话生命周期内有效（EventSink 析构前回调已解绑）。
    let mut guard =
        unsafe { (*inner).cb.lock() }.unwrap_or_else(std::sync::PoisonError::into_inner);
    (guard)(event);
}

/// 任务会话。
pub struct Session {
    raw: *mut audiocpp_session,
    /// 事件回调的持有者；析构时负责释放并确保 C 侧不再引用。
    /// 用 `Mutex` 保护指针本身，使 `set_event_callback` 能以 `&self` 调用
    /// （内部可变性），从而在持有 `&Session` 的线程上注册回调。
    event_sink: Mutex<Option<*mut EventSinkInner>>,
}

// Session 持有回调 box（要求 Send）与 C 句柄。跨线程转移所有权是安全的，
// 但用户应避免同时对同一会话做流式调用与回调锁内嵌套调用。
unsafe impl Send for Session {}

impl Session {
    /// 从原始 C 句柄包装（仅内部使用）。
    pub(crate) fn from_raw(raw: *mut audiocpp_session) -> Self {
        Self {
            raw,
            event_sink: Mutex::new(None),
        }
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
    ///
    /// # Errors
    ///
    /// C ABI 调用失败或返回的 JSON 无法解析时返回对应 [`Error`] 变体。
    pub fn streaming_policy(&self) -> Result<StreamingPolicy, Error> {
        let mut out: *mut c_char = ptr::null_mut();
        ffi::check_rc(unsafe { audiocpp_session_streaming_policy_json(self.raw, &mut out) })?;
        let json = unsafe { ffi::take_string(out)? };
        serde_json::from_str(&json).map_err(Error::from)
    }

    /// 为一次请求做准备（离线与流式共用）。
    ///
    /// `request` 为 [`crate::request::IntoRequest`]，可用类型化的
    /// [`crate::request::Request`] 枚举（推荐，每个任务一种变体），或任意
    /// JSON 字符串。携带 `audio` / `text` / `voice` 等输入，或用 `audio_path`
    /// 指向本地 WAV 文件。空请求可传 `()`。
    ///
    /// # Errors
    ///
    /// 请求序列化失败、路径含 NUL，或 C ABI 调用失败时返回对应 [`Error`] 变体。
    pub fn prepare<R: IntoRequest>(&self, request: R) -> Result<(), Error> {
        let json = request.into_request()?.to_json()?;
        let req_c = ffi::cstring(&json)?;
        ffi::check_rc(unsafe {
            audiocpp_session_prepare(self.raw, req_c.as_ptr() as *const c_char)
        })
    }

    /// 注册流式事件回调；调用 `None` 清除。
    ///
    /// 回调会在每次流事件产生时被调用（可能来自 C++ 侧线程），其内容是
    /// 解析为 [`StreamEvent`] 的对象。回调内不得再调用本会话的方法。
    ///
    /// 以 `&self` 提供（内部有 `Mutex` 保护），因此可在持有 `&Session`
    /// 的任意线程上注册 / 更换回调。同一个会话只保留一份回调。
    pub fn set_event_callback<F>(&self, cb: Option<F>)
    where
        F: FnMut(StreamEvent) + Send + 'static,
    {
        // 先清理旧回调，保证 C 侧不再引用旧的 user_data。
        let old = self.event_sink_take();
        if let Some(old) = old {
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
                audiocpp_session_set_event_sink(
                    self.raw,
                    Some(stream_event_cb),
                    inner.cast::<c_void>(),
                );
            }
            *self
                .event_sink
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(inner);
        }
    }

    /// 开始一次流式任务（先 `prepare` 再进入流式读取循环）。
    ///
    /// `request` 同 [`Session::prepare`]，类型化枚举或 JSON 字符串均可；
    /// 空请求传 `()`。
    ///
    /// # Errors
    ///
    /// 请求序列化失败、路径含 NUL，或 C ABI 调用失败时返回对应 [`Error`] 变体。
    pub fn start<R: IntoRequest>(&self, request: R) -> Result<(), Error> {
        let json = request.into_request()?.to_json()?;
        let req_c = ffi::cstring(&json)?;
        ffi::check_rc(unsafe { audiocpp_session_start(self.raw, req_c.as_ptr() as *const c_char) })
    }

    /// 将一段音频送入流式会话，返回该块触发的第一个事件（若有）。
    ///
    /// `samples` 为 `float` 采样（-1..1），`start_sample` 为该块在输入流中的
    /// 起始采样点。若同时注册了事件回调，事件会经回调到达；返回值是
    /// shim 同步返回的事件 JSON。
    ///
    /// # Errors
    ///
    /// C ABI 调用失败或返回的 JSON 无法解析时返回对应 [`Error`] 变体。
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
    ///
    /// # Errors
    ///
    /// C ABI 调用失败或返回的 JSON 无法解析时返回对应 [`Error`] 变体。
    pub fn finish(&self) -> Result<TaskResult, Error> {
        let mut out: *mut c_char = ptr::null_mut();
        ffi::check_rc(unsafe { audiocpp_session_finish(self.raw, &mut out) })?;
        if out.is_null() {
            return Ok(TaskResult {
                speech_segments: Vec::new(),
                speaker_turns: Vec::new(),
                text_output: None,
                audio_output: None,
                named_audio_outputs: Vec::new(),
                word_timestamps: Vec::new(),
                artifact_output: None,
                output_artifacts: Vec::new(),
            });
        }
        let json = unsafe { ffi::take_string(out)? };
        serde_json::from_str(&json).map_err(Error::from)
    }

    /// 离线执行一次请求，返回 TaskResult。
    ///
    /// 相当于 `prepare` + 一次完整执行（shim 内部已做 prepare）。`request`
    /// 同 [`Session::prepare`]，类型化枚举或 JSON 字符串均可。
    ///
    /// # Errors
    ///
    /// 请求序列化失败、路径含 NUL，或 C ABI 调用失败时返回对应 [`Error`] 变体。
    pub fn run_offline<R: IntoRequest>(&self, request: R) -> Result<TaskResult, Error> {
        let json = request.into_request()?.to_json()?;
        let req_c = ffi::cstring(&json)?;
        let mut out: *mut c_char = ptr::null_mut();
        ffi::check_rc(unsafe {
            audiocpp_session_run_offline(self.raw, req_c.as_ptr() as *const c_char, &mut out)
        })?;
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

    /// 取出并归还事件回调指针（内部用；调用方负责释放）。
    fn event_sink_take(&self) -> Option<*mut EventSinkInner> {
        self.event_sink
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // 先清除事件回调，避免 C 侧在会话析构后再引用 user_data。
        if let Some(inner) = self.event_sink_take() {
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

/// 流式会话的便捷封装。
///
/// 包装一个 [`Session`]，自动注册内部事件收集器，把 shim 的逐事件回调
/// 缓冲为 Vec，使调用方无需手动管理回调 + 锁。典型用法（流式 ASR）：
///
/// ```
/// use audio_cpp::{Backend, Model, Registry, Request, RunMode, TaskKind};
/// use audio_cpp::session::StreamingSession;
/// # fn f() -> Result<(), audio_cpp::Error> {
/// # let registry = Registry::new()?;
/// # let model = registry.load("./qwen3-asr-0.6b-q8_0.gguf", None, None)?;
/// let session = model.create_task_session(
///     TaskKind::Asr, RunMode::Streaming, Backend::Cpu, 0, 1, None,
/// )?;
/// let mut stream = StreamingSession::from_session(session);
///
/// stream.start(Request::stream().option("language", "auto"))?;
/// // 每块音频送入后，取出该块触发的全部事件（partial_text / is_final）
/// let samples: Vec<f32> = vec![0.0; 512];
/// let events = stream.push_audio(&samples, 16000, 1, 0)?;
/// for ev in &events {
///     if let Some(t) = &ev.partial_text {
///         println!("增量: {}", t.text);
///     }
/// }
/// let result = stream.finish()?;
/// # Ok(()) }
/// ```
///
/// 与底层 [`Session::process_audio`] 不同，`push_audio` 返回该块触发的
/// **所有**事件（经内部回调收集），而不是单个事件。不消费音频块的模型族
/// （如流式 TTS）可跳过 `push_audio`，直接 `start` → `finish`。
pub struct StreamingSession {
    session: Session,
    events: Arc<Mutex<Vec<StreamEvent>>>,
}

unsafe impl Send for StreamingSession {}

impl StreamingSession {
    /// 从现有会话包装（流式任务）。
    pub fn from_session(session: Session) -> Self {
        let events: Arc<Mutex<Vec<StreamEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        session.set_event_callback(Some(move |ev| {
            if let Ok(mut v) = sink.lock() {
                v.push(ev);
            }
        }));
        Self { session, events }
    }

    /// 从模型创建流式会话并包装（等价于
    /// `create_task_session(Streaming)` + [`Self::from_session`]）。
    ///
    /// # Errors
    ///
    /// 底层无法创建流式会话时返回对应 [`Error`] 变体。
    pub fn from_model(
        model: &Model,
        task: TaskKind,
        backend: Backend,
        device: i32,
        threads: i32,
        session_options: Option<&str>,
    ) -> Result<Self, Error> {
        let session = model.create_task_session(
            task,
            RunMode::Streaming,
            backend,
            device,
            threads,
            session_options,
        )?;
        Ok(Self::from_session(session))
    }

    /// 开始流式任务（等价于 [`Session::start`]）。
    ///
    /// # Errors
    ///
    /// 请求序列化失败、路径含 NUL，或 C ABI 调用失败时返回对应 [`Error`] 变体。
    pub fn start<R: IntoRequest>(&self, request: R) -> Result<(), Error> {
        self.session.start(request)
    }

    /// 送入一段音频，返回该块触发的全部流事件。
    ///
    /// 等价于先清空内部缓冲、调用 [`Session::process_audio`]、再取出缓冲
    /// 中的全部事件。`samples` 为 `float` 采样，`start_sample` 为相对输入
    /// 流的起始采样点。
    ///
    /// # Errors
    ///
    /// C ABI 调用失败时返回对应 [`Error`] 变体。
    pub fn push_audio(
        &self,
        samples: &[f32],
        sample_rate: i32,
        channels: i32,
        start_sample: i64,
    ) -> Result<Vec<StreamEvent>, Error> {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
        self.session
            .process_audio(samples, sample_rate, channels, start_sample)?;
        Ok(self
            .events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .drain(..)
            .collect())
    }

    /// 结束流式会话，返回最终结果（等价于 [`Session::finish`]）。
    ///
    /// # Errors
    ///
    /// C ABI 调用失败时返回对应 [`Error`] 变体。
    pub fn finish(&self) -> Result<TaskResult, Error> {
        self.session.finish()
    }

    /// 重置会话（等价于 [`Session::reset`]），并清空内部缓冲。
    pub fn reset(&self) {
        self.session.reset();
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }

    /// 底层会话引用。
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// 底层会话可变引用。
    pub fn session_mut(&mut self) -> &mut Session {
        &mut self.session
    }
}
