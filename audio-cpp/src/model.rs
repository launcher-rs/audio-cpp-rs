//! 已加载的模型（Model）。

use std::ffi::c_char;
use std::os::raw::c_int;
use std::ptr;

use audio_cpp_sys::*;

use crate::error::Error;
use crate::ffi;
use crate::session::Session;
use crate::types::{Backend, Capabilities, ModelMetadata, RunMode, TaskKind};

/// 已加载的模型。
///
/// 由 [`crate::Registry::load`] 创建，持有一个 C 句柄。模型不会自行
/// 释放注册表；注册表的生命周期通常覆盖所用模型的全部使用时段。
pub struct Model {
    raw: *mut audiocpp_model,
}

unsafe impl Send for Model {}
unsafe impl Sync for Model {}

impl Model {
    /// 从原始 C 句柄包装（仅内部使用）。
    pub(crate) fn from_raw(raw: *mut audiocpp_model) -> Self {
        Self { raw }
    }

    /// 模型元数据（family / variant / description / 候选配置与权重）。
    ///
    /// # Errors
    ///
    /// C ABI 调用失败或返回的 JSON 无法解析时返回对应 [`Error`] 变体。
    pub fn metadata(&self) -> Result<ModelMetadata, Error> {
        let mut out: *mut c_char = ptr::null_mut();
        ffi::check_rc(unsafe { audiocpp_model_metadata_json(self.raw, &mut out) })?;
        let json = unsafe { ffi::take_string(out)? };
        serde_json::from_str(&json).map_err(Error::from)
    }

    /// 模型能力集合。
    ///
    /// # Errors
    ///
    /// C ABI 调用失败或返回的 JSON 无法解析时返回对应 [`Error`] 变体。
    pub fn capabilities(&self) -> Result<Capabilities, Error> {
        let mut out: *mut c_char = ptr::null_mut();
        ffi::check_rc(unsafe { audiocpp_model_capabilities_json(self.raw, &mut out) })?;
        let json = unsafe { ffi::take_string(out)? };
        serde_json::from_str(&json).map_err(Error::from)
    }

    /// 在模型上创建一次任务会话。
    ///
    /// 可从 `metadata()` / `capabilities()` 确认模型是否支持目标任务与模式。
    ///
    /// # Errors
    ///
    /// 参数含 NUL / 非 UTF-8 返回 [`Error::Nul`] 或 [`Error::NonUtf8Path`]；
    /// 底层无法创建会话返回 [`Error::NullHandle`]。
    pub fn create_task_session(
        &self,
        task: TaskKind,
        mode: RunMode,
        backend: Backend,
        device: i32,
        threads: i32,
        session_options: Option<&str>,
    ) -> Result<Session, Error> {
        let task_c = ffi::cstring(task.as_str())?;
        let mode_c = ffi::cstring(mode.as_str())?;
        let backend_c = ffi::cstring(backend.as_str())?;
        let options_c = session_options.map(ffi::cstring).transpose()?;

        let raw = unsafe {
            audiocpp_model_create_task_session(
                self.raw,
                task_c.as_ptr() as *const c_char,
                mode_c.as_ptr() as *const c_char,
                backend_c.as_ptr() as *const c_char,
                device as c_int,
                threads as c_int,
                options_c
                    .as_ref()
                    .map_or(ptr::null(), |s| s.as_ptr() as *const c_char),
            )
        };
        if raw.is_null() {
            return Err(Error::NullHandle(ffi::last_error()));
        }
        Ok(Session::from_raw(raw))
    }
}

impl Drop for Model {
    fn drop(&mut self) {
        unsafe {
            audiocpp_model_free(self.raw);
        }
    }
}
