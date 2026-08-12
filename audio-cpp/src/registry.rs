//! 模型注册表（Registry）。
//!
//! 注册表负责枚举已编译进引擎的模型族 / loader / 设备，并加载模型。
//! 一个 `Registry` 持有 C 侧的不透明句柄，可被多个 `Model` 共享，直到
//! 所有派生句柄释放后才可释放（见各类型的 `Drop`）。

use std::ffi::c_char;
use std::ptr;

use audio_cpp_sys::*;

use crate::error::Error;
use crate::ffi;
use crate::model::Model;
use crate::types::{Device, LoaderInfo};

/// 模型注册表。
pub struct Registry {
    raw: *mut audiocpp_registry,
}

// C ABI 句柄本身不要求 Send/Sync；但上层不透传 & 引用跨线程使用句柄。
// Registry 自身为唯一持有者，跨线程转移所有权是安全的（内部有锁/无共享可变态）。
unsafe impl Send for Registry {}
unsafe impl Sync for Registry {}

impl Registry {
    /// 创建默认注册表，包含所有编译进本库的模型族 loader。
    pub fn new() -> Result<Self, Error> {
        let raw = unsafe { audiocpp_registry_default() };
        if raw.is_null() {
            return Err(Error::NullHandle(ffi::last_error()));
        }
        Ok(Self { raw })
    }

    /// 已注册的模型族列表，例如 `["silero_vad","qwen3_asr"]`。
    pub fn families(&self) -> Result<Vec<String>, Error> {
        let mut out: *mut c_char = ptr::null_mut();
        ffi::check_rc(unsafe { audiocpp_registry_families_json(self.raw, &mut out) })?;
        let json = unsafe { ffi::take_string(out)? };
        serde_json::from_str(&json).map_err(Error::from)
    }

    /// 所有 loader 的声明信息（模型族、任务、端点）。
    pub fn loaders(&self) -> Result<Vec<LoaderInfo>, Error> {
        let mut out: *mut c_char = ptr::null_mut();
        ffi::check_rc(unsafe { audiocpp_registry_loaders_json(self.raw, &mut out) })?;
        let json = unsafe { ffi::take_string(out)? };
        let root: serde_json::Value = serde_json::from_str(&json)?;
        Ok(serde_json::from_value(root["loaders"].clone())?)
    }

    /// 枚举所有后端可用的计算设备。
    pub fn devices() -> Result<Vec<Device>, Error> {
        let mut out: *mut c_char = ptr::null_mut();
        ffi::check_rc(unsafe { audiocpp_registry_devices_json(&mut out) })?;
        let json = unsafe { ffi::take_string(out)? };
        serde_json::from_str(&json).map_err(Error::from)
    }

    /// 加载一个模型。
    ///
    /// `model_path` 为权重文件路径（如 `.safetensors` / `.gguf`）；
    /// `family_hint` 可选，用于指定模型族；`load_options` 可选，例如
    /// `{"weight_id":"..."}`。
    pub fn load(
        &self,
        model_path: &str,
        family_hint: Option<&str>,
        load_options: Option<&str>,
    ) -> Result<Model, Error> {
        let path_c = ffi::cstring(model_path)?;
        let hint_c = family_hint.map(ffi::cstring).transpose()?;
        let options_c = load_options.map(ffi::cstring).transpose()?;

        let raw = unsafe {
            audiocpp_registry_load(
                self.raw,
                path_c.as_ptr() as *const c_char,
                hint_c.as_ref().map_or(ptr::null(), |s| s.as_ptr() as *const c_char),
                options_c.as_ref().map_or(ptr::null(), |s| s.as_ptr() as *const c_char),
            )
        };
        if raw.is_null() {
            return Err(Error::NullHandle(ffi::last_error()));
        }
        Ok(Model::from_raw(raw))
    }
}

impl Drop for Registry {
    fn drop(&mut self) {
        unsafe {
            audiocpp_registry_free(self.raw);
        }
    }
}