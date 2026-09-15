//! 模型注册表（Registry）。
//!
//! 注册表负责枚举已编译进引擎的模型族 / loader / 设备，并加载模型。
//! 一个 `Registry` 持有 C 侧的不透明句柄，可被多个 `Model` 共享，直到
//! 所有派生句柄释放后才可释放（见各类型的 `Drop`）。

use std::cell::UnsafeCell;
use std::ffi::c_char;
use std::marker::PhantomData;
use std::ptr;
use std::sync::Arc;

use audio_cpp_sys::*;

use crate::error::Error;
use crate::ffi;
use crate::model::Model;
use crate::types::{Device, LoaderInfo, ModelFamily, ModelInspection};

/// C 注册表所有权的共享持有者。
///
/// `Registry::load` 返回的每个 [`Model`] 都会克隆持有它，因此调用方在
/// 加载完成后即可释放 `Registry`：C 注册表存活到最后一个派生句柄释放，
/// 不存在"先 drop registry 即悬垂"。`Send` 而非 `Sync`（C 侧 registry
/// 内部无锁，并发共享 `&Registry` 仍是未定义行为，与之前一致）。
#[derive(Clone)]
// `Arc` 在此仅作共享所有权计数（跨线程只转移所有权、从不并发访问，
// 与 `Registry: Send + !Sync` 的既有语义一致）。
pub(crate) struct RegistryGuard {
    // 仅作所有权计数（Clone/Drop），从不读内容：C 注册表由最后一个
    // guard 释放，这是刻意设计而非冗余字段。
    #[allow(dead_code)]
    inner: Arc<RegistryInner>,
}

struct RegistryInner {
    raw: *mut audiocpp_registry,
    // `*mut` 本身是 Send+Sync，加一个 !Sync 标记保住 `!Sync` 语义。
    _no_sync: PhantomData<UnsafeCell<()>>,
}

// SAFETY: guard 只做所有权计数与释放（`Drop` 调 `audiocpp_registry_free`），
// 不提供并发访问 C 注册表的路径；C 句柄的跨线程转移与之前 `Registry: Send` 一致。
unsafe impl Send for RegistryGuard {}

impl Drop for RegistryInner {
    fn drop(&mut self) {
        unsafe {
            audiocpp_registry_free(self.raw);
        }
    }
}

/// 模型注册表。
pub struct Registry {
    raw: *mut audiocpp_registry,
    // 自持有：`Clone` 出去给 Model，Registry 先 drop 不影响已加载模型。
    guard: RegistryGuard,
}

// C ABI 句柄本身不要求 Send/Sync。C 侧 registry 内部无锁，多个线程共享
// 同一注册表调用 load() 属于并发访问，行为未定义。因此只实现 Send
// （所有权可跨线程转移），不实现 Sync（禁止 & 共享跨线程使用）。
unsafe impl Send for Registry {}

impl Registry {
    /// 创建默认注册表，包含所有编译进本库的模型族 loader。
    ///
    /// # Errors
    ///
    /// 底层 C ABI 无法创建注册表时返回 [`Error::NullHandle`]。
    // `Arc` 仅作所有权计数（见 `RegistryGuard`），`!Send` 属刻意设计。
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn new() -> Result<Self, Error> {
        let raw = unsafe { audiocpp_registry_default() };
        if raw.is_null() {
            return Err(Error::NullHandle(ffi::last_error()));
        }
        let guard = RegistryGuard {
            inner: Arc::new(RegistryInner {
                raw,
                _no_sync: PhantomData,
            }),
        };
        Ok(Self { raw, guard })
    }

    /// 已注册的模型族列表，例如 `["silero_vad","qwen3_asr"]`。
    ///
    /// # Errors
    ///
    /// C ABI 调用失败或返回的 JSON 无法解析时返回对应 [`Error`] 变体。
    pub fn families(&self) -> Result<Vec<String>, Error> {
        let mut out: *mut c_char = ptr::null_mut();
        ffi::check_rc(unsafe { audiocpp_registry_families_json(self.raw, &mut out) })?;
        let json = unsafe { ffi::take_string(out)? };
        serde_json::from_str(&json).map_err(Error::from)
    }

    /// 所有 loader 的声明信息（模型族、任务、端点）。
    ///
    /// # Errors
    ///
    /// C ABI 调用失败或返回的 JSON 无法解析时返回对应 [`Error`] 变体。
    pub fn loaders(&self) -> Result<Vec<LoaderInfo>, Error> {
        let mut out: *mut c_char = ptr::null_mut();
        ffi::check_rc(unsafe { audiocpp_registry_loaders_json(self.raw, &mut out) })?;
        let json = unsafe { ffi::take_string(out)? };
        let root: serde_json::Value = serde_json::from_str(&json)?;
        let loaders = root.get("loaders").ok_or_else(|| {
            Error::Other(format!(
                "注册表 loaders JSON 缺少 `loaders` 顶层字段: {json}"
            ))
        })?;
        serde_json::from_value(loaders.clone()).map_err(Error::from)
    }

    /// 枚举所有后端可用的计算设备。
    ///
    /// # Errors
    ///
    /// C ABI 调用失败或返回的 JSON 无法解析时返回对应 [`Error`] 变体。
    pub fn devices() -> Result<Vec<Device>, Error> {
        let mut out: *mut c_char = ptr::null_mut();
        ffi::check_rc(unsafe { audiocpp_registry_devices_json(&mut out) })?;
        let json = unsafe { ffi::take_string(out)? };
        serde_json::from_str(&json).map_err(Error::from)
    }

    /// 判断某个模型族是否已编译进当前引擎。
    ///
    /// 用于加载前的预检：返回 `false` 说明该族未启（需对应 `model-*` feature
    /// 或 `full-models` 重新编译），直接 `load` 会失败。
    ///
    /// 注意：`false` 同时涵盖“内部出错”（此时详情见 C 侧 `last_error`，但本
    /// 签名无法传出）；需区分时请用 [`Registry::try_supports_family`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use audio_cpp::Registry;
    /// let registry = Registry::new().unwrap();
    /// // 默认 core 构建恒定包含 silero_vad；随机字符串必然为 false。
    /// assert!(registry.supports_family("silero_vad"));
    /// assert!(!registry.supports_family("definitely_not_a_family"));
    /// ```
    pub fn supports_family(&self, family: &str) -> bool {
        self.try_supports_family(family).unwrap_or(false)
    }

    /// 与 [`Registry::supports_family`] 同义，但区分“不支持”与“出错”。
    ///
    /// C ABI 在传参非法或内部异常时返回 `-1`（`supports_family` 把它折成
    /// `false`）；本方法此时返回 `Err` 并携带 `last_error` 文本。
    ///
    /// # Errors
    ///
    /// 字符串含 NUL，或 C ABI 内部出错时返回对应 [`Error`] 变体。
    pub fn try_supports_family(&self, family: &str) -> Result<bool, Error> {
        let family_c = ffi::cstring(family)?;
        let rc = unsafe {
            audiocpp_registry_supports_family(self.raw, family_c.as_ptr() as *const c_char)
        };
        if rc < 0 {
            return Err(Error::Ffi(ffi::last_error()));
        }
        Ok(rc != 0)
    }

    /// 预检模型文件：无需真正加载即可获得 metadata / capabilities / 支持的
    /// CLI 选项 / 发现的配置与权重资产。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use audio_cpp::Registry;
    /// let registry = Registry::new().unwrap();
    /// let info = registry.inspect("./qwen3-asr-q8_0.gguf").unwrap();
    /// println!("族: {}，变体: {}", info.metadata.family, info.metadata.variant);
    /// for opt in &info.cli.request_options {
    ///     println!("  请求选项 {}: {}", opt.name, opt.description);
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// C ABI 调用失败、路径非法或返回 JSON 无法解析时返回对应 [`Error`] 变体。
    /// 若文件不存在或无法被任何 loader 识别，底层会报错（如 [`Error::NullHandle`]）。
    pub fn inspect(&self, model_path: &str) -> Result<ModelInspection, Error> {
        let path_c = ffi::cstring(model_path)?;
        let mut out: *mut c_char = ptr::null_mut();
        ffi::check_rc(unsafe {
            audiocpp_registry_inspect_json(self.raw, path_c.as_ptr() as *const c_char, &mut out)
        })?;
        let json = unsafe { ffi::take_string(out)? };
        serde_json::from_str(&json).map_err(Error::from)
    }

    /// 加载一个模型。
    ///
    /// `model_path` 为权重文件路径（如 `.safetensors` / `.gguf`）；
    /// `family_hint` 可选，用于指定模型族（GGUF / NeMo safetensors 无法
    /// 自动探测族别，须显式指定，见 [`crate::ModelFamily`]）；`load_options`
    /// 可选，例如 `{"weight_id":"..."}`。
    ///
    /// 返回的 [`Model`] 共享持有 C 注册表的所有权：本 `Registry` 释放后
    /// 模型仍可继续使用，无需 `_registry` 三件套持有。
    ///
    /// # Errors
    ///
    /// 路径含 NUL / 非 UTF-8 返回 [`Error::Nul`] 或 [`Error::NonUtf8Path`]；
    /// 底层加载失败返回 [`Error::NullHandle`]。
    pub fn load(
        &self,
        model_path: &str,
        family_hint: Option<ModelFamily>,
        load_options: Option<&str>,
    ) -> Result<Model, Error> {
        let path_c = ffi::cstring(model_path)?;
        let hint_c = family_hint
            .as_ref()
            .map(|f| ffi::cstring(f.as_str()))
            .transpose()?;
        let options_c = load_options.map(ffi::cstring).transpose()?;

        let raw = unsafe {
            audiocpp_registry_load(
                self.raw,
                path_c.as_ptr() as *const c_char,
                hint_c
                    .as_ref()
                    .map_or(ptr::null(), |s| s.as_ptr() as *const c_char),
                options_c
                    .as_ref()
                    .map_or(ptr::null(), |s| s.as_ptr() as *const c_char),
            )
        };
        if raw.is_null() {
            return Err(Error::NullHandle(ffi::last_error()));
        }
        Ok(Model::from_raw(raw, self.guard.clone()))
    }
}

// 注意：`Registry` 没有手动 `Drop` 实现——字段析构即语义：`raw` 是普通指针
// 无动作，`guard` 的 Arc 计数减一；C 注册表由最后一个存活的 `RegistryGuard`
// （Registry / Model / Session 任一持有）释放。

#[cfg(test)]
mod tests {
    // 测试断言中的 unwrap/expect 是惯用法：失败即测试失败，展开错误链无意义。
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn supports_family_known_and_unknown() {
        let reg = Registry::new().unwrap();
        // core 模型集始终包含 silero_vad；明显不存在的族应返回 false 且不报错。
        assert!(reg.supports_family("silero_vad"));
        assert!(!reg.supports_family("definitely_not_a_real_family_xyz"));
    }

    #[test]
    fn model_outlives_registry() {
        // G5：Model 共享持有 C 注册表，Registry 先释放后模型仍可继续使用。
        // 用上游自带的 silero 内置权重（构建本 crate 必需 submodule，路径稳定）。
        let weight = "../audio-cpp-sys/audio.cpp/assets/framework/models/silero_vad/silero_vad_16k.safetensors";
        assert!(
            std::path::Path::new(weight).exists(),
            "内置权重缺失，submodule 未补齐？{weight}"
        );
        let model = {
            let reg = Registry::new().unwrap();
            reg.load(weight, None, None).unwrap()
        };
        // Registry 已释放：metadata 走 C 侧 loaded-model，与注册表无关。
        let meta = model.metadata().unwrap();
        assert_eq!(meta.family, "silero_vad");
    }
}
