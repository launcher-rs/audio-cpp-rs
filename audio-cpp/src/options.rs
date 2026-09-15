//! 常用选项键常量与会话选项构造器。
//!
//! 上游选项是无类型的字符串键值（`options: { "key": "value" }`），配错键名
//! 不会报错、只会被静默忽略（教训：`vad_threshold` 在上游从未存在，
//! silero/marblenet 的阈值键都是 [`request::THRESHOLD`]）。本模块把源码级
//! 核实过的键收敛为常量，拼写错误在编译期暴露；族名前缀别名（如
//! `sortformer_diar.session_len_sec`）与裸键等价，shim 照单全收。
//!
//! 键分两类，传参位置不同不可混用：
//! - [`request`]：随请求走（[`crate::Request::option`]），如 `language`；
//! - [`session`]：随会话走（[`SessionOptions`] →
//!   [`crate::Model::create_task_session`] 的 `session_options`），如
//!   `session_len_sec`。
//!
//! # 示例
//!
//! ```
//! use audio_cpp::options::{SessionOptions, request, session};
//! use audio_cpp::Request;
//!
//! // 请求选项：直接用常量当键
//! let req = Request::asr("./speech.wav").option(request::LANGUAGE, "zh");
//! // 会话选项：builder 拼出 create_task_session 要的 JSON 字符串
//! let opts = SessionOptions::new()
//!     .session_len_sec(100.0)
//!     .graph_capacity_mode(session::graph_capacity::GROW);
//! assert_eq!(
//!     opts.to_json(),
//!     r#"{"graph_capacity_mode":"grow","session_len_sec":100.0}"#
//! );
//! ```

use std::collections::BTreeMap;

/// 请求选项键（随 [`crate::Request::option`] 走）。
pub mod request {
    /// 转写/合成语言（如 `"zh"` / `"en"`）。
    ///
    /// 注意：部分族只读顶层 `language`（对齐器要求 `text_input.language`
    /// 非空），部分只读本选项（如 Qwen3 TTS 的声音克隆参考）；
    /// [`crate::TtsRequest::language`] 与 [`crate::Request::align`] 写的是
    /// 顶层 `language`。parakeet_tdt 会严格校验请求选项——它未声明的键
    /// （含本键）会直接拒绝，此时只用顶层 `language`。
    pub const LANGUAGE: &str = "language";
    /// Qwen3 ASR 是否回填词级时间戳（`"true"` 开启）。
    ///
    /// 开启后还需会话选项配好对齐模型（见
    /// [`super::session::QWEN3_ASR_FORCED_ALIGNER_MODEL_PATH`]），否则上游
    /// 抛错；流式模式不支持本选项（直接抛错）。无词戳输出时先检查这两处，
    /// 而非假设上游未填充。
    pub const RETURN_TIMESTAMPS: &str = "return_timestamps";
    /// 词级时间戳钳制到音频范围内（`"true"` 开启）。
    ///
    /// Qwen3 ASR 与 Qwen3 对齐器均读取本键（后者在
    /// `model_specs/qwen3_forced_aligner.json` 声明，默认 `false`）。
    pub const CLAMP_TIMESTAMPS_TO_AUDIO: &str = "clamp_timestamps_to_audio";
    /// 音频分块时长（秒，浮点），如 Qwen3 ASR 流式的窗口／离线分块。
    ///
    /// 别名（等价）：`audio_chunk_duration_sec` / `audio_chunk_duration_seconds`
    /// / `audio_chunk_duration`。
    pub const AUDIO_CHUNK_SECONDS: &str = "audio_chunk_seconds";
    /// 音频分块模式：`auto` / `fixed` / `quiet_energy` / `vad` / `none`
    /// （见 [`super::session::audio_chunk_mode`]）。
    pub const AUDIO_CHUNK_MODE: &str = "audio_chunk_mode";
    /// VAD 阈值（silero_vad 与 marblenet_vad 均用本键）。
    ///
    /// 注意：`vad_threshold` 在上游从未存在，传它会被静默忽略。
    pub const THRESHOLD: &str = "threshold";
    /// Qwen3 TTS 声音克隆：参考音频的文本转写（越贴合内容音色越准）。
    ///
    /// [`crate::TtsRequest::reference_text`] 已封装本键，无需手写。
    pub const REFERENCE_TEXT: &str = "reference_text";
    /// 流式 TTS 坏样本重试开关（`"false"` 关闭；VoxCPM2 流式硬性要求关闭）。
    pub const RETRY_BADCASE: &str = "retry_badcase";
    /// YuE2 歌曲风格／标签（必填，非空）。
    pub const STYLE: &str = "style";
    /// YuE2 歌词（必填；与请求顶层 `text` 等价，见 `Request::tts`）。
    pub const LYRICS: &str = "lyrics";
    /// 采样种子（YuE2 等生成族）。
    pub const SEED: &str = "seed";
    /// 解码最大长度（如 SheetSage2、Moonshine）。
    pub const MAX_TOKENS: &str = "max_tokens";
    /// MMS 对齐器文本归一化：`latin`（默认，31 符号拉丁 CTC）/
    /// `pre_romanized`（调用方自备 ASCII 罗马化文本）。
    pub const TEXT_NORMALIZATION: &str = "text_normalization";
}

/// 会话选项键（随 [`SessionOptions`] 走，建会话时一次传入）。
pub mod session {
    /// 长音频会话图长度（秒），如 SortFormer 说话人分离默认仅 ~20s/90s，
    /// 超长音频需调大（如 `100.0`）配合 [`GRAPH_CAPACITY_MODE`] 使用。
    ///
    /// 族名前缀别名（等价）：`sortformer_diar.session_len_sec`。
    pub const SESSION_LEN_SEC: &str = "session_len_sec";
    /// 会话图容量模式（见 [`graph_capacity`] 取值）。
    pub const GRAPH_CAPACITY_MODE: &str = "graph_capacity_mode";
    /// Qwen3 ASR 词戳回填所需的对齐模型路径（Qwen3-ForcedAligner 权重）。
    ///
    /// 不配而请求开 [`super::request::RETURN_TIMESTAMPS`] 时上游抛错。
    /// 别名（等价）：`qwen3_asr.aligner_model_path`。
    pub const QWEN3_ASR_FORCED_ALIGNER_MODEL_PATH: &str = "qwen3_asr.forced_aligner_model_path";

    /// [`GRAPH_CAPACITY_MODE`](self::GRAPH_CAPACITY_MODE) 的合法取值。
    pub mod graph_capacity {
        /// 固定容量，超限即报 `request exceeds fixed graph capacity`。
        pub const FIXED: &str = "fixed";
        /// 分档容量。
        pub const TIERED: &str = "tiered";
        /// 按需增长（长音频 diar 常用）。
        pub const GROW: &str = "grow";
        /// 翻倍增长。
        pub const DOUBLE: &str = "double";
    }

    /// [`super::request::AUDIO_CHUNK_MODE`] 的合法取值。
    pub mod audio_chunk_mode {
        /// 自动选择（默认）。
        pub const AUTO: &str = "auto";
        /// 固定窗口切分。
        pub const FIXED: &str = "fixed";
        /// 静音能量切分。
        pub const QUIET_ENERGY: &str = "quiet_energy";
        /// VAD 切分。
        pub const VAD: &str = "vad";
        /// 不切分。
        pub const NONE: &str = "none";
    }
}

/// 会话选项构造器：拼出 [`crate::Model::create_task_session`] 的
/// `session_options` 参数要的 JSON 字符串。
///
/// JSON 形如 `{"session_len_sec":100.0,"graph_capacity_mode":"grow"}`；
/// 空选项序列化为 `{}`。未知键用 [`SessionOptions::option`] 自由扩展
/// （shim 对未知键同样静默忽略，键名请核对 [`session`] 常量）。
///
/// # 示例
///
/// ```no_run
/// use audio_cpp::{Backend, Registry, RunMode, TaskKind, options::SessionOptions};
///
/// let registry = Registry::new()?;
/// let model = registry.load("./sortformer-diar-4spk-q8_0.gguf", None, None)?;
/// let opts = SessionOptions::new()
///     .session_len_sec(100.0)
///     .graph_capacity_mode(audio_cpp::options::session::graph_capacity::GROW);
/// let opts_json = opts.to_json();
/// let session = model.create_task_session(
///     TaskKind::Diar, RunMode::Offline, Backend::Cpu, 0, 4, Some(&opts_json),
/// )?;
/// # Ok::<(), audio_cpp::Error>(())
/// ```
#[derive(Debug, Clone, Default)]
pub struct SessionOptions {
    options: BTreeMap<String, String>,
}

impl SessionOptions {
    /// 空选项。
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置一个选项键值（值为任意可转字符串类型）。
    pub fn option(mut self, key: impl Into<String>, value: impl ToString) -> Self {
        self.options.insert(key.into(), value.to_string());
        self
    }

    /// 会话图长度（秒），见 [`session::SESSION_LEN_SEC`]。
    pub fn session_len_sec(mut self, seconds: f64) -> Self {
        self.options
            .insert(session::SESSION_LEN_SEC.to_owned(), format!("{seconds:?}"));
        self
    }

    /// 会话图容量模式（取值见 [`session::graph_capacity`]），见
    /// [`session::GRAPH_CAPACITY_MODE`]。
    pub fn graph_capacity_mode(mut self, mode: impl Into<String>) -> Self {
        self.options
            .insert(session::GRAPH_CAPACITY_MODE.to_owned(), mode.into());
        self
    }

    /// Qwen3 ASR 词戳回填的对齐模型路径，见
    /// [`session::QWEN3_ASR_FORCED_ALIGNER_MODEL_PATH`]。
    pub fn qwen3_aligner_model_path(mut self, path: impl Into<String>) -> Self {
        self.options.insert(
            session::QWEN3_ASR_FORCED_ALIGNER_MODEL_PATH.to_owned(),
            path.into(),
        );
        self
    }

    /// 序列化为 JSON 字符串（传给 `create_task_session`）。
    ///
    /// 数值型值（[`SessionOptions::session_len_sec`] 写入的）按 JSON 数字
    /// 还原，其余按字符串；空选项为 `{}`。
    pub fn to_json(&self) -> String {
        let mut obj = serde_json::Map::new();
        for (k, v) in &self.options {
            // session_len_sec 写入的是 Rust float Debug 格式，还原为数字。
            let value = if k == session::SESSION_LEN_SEC {
                v.parse::<f64>().map_or_else(
                    |_| serde_json::Value::String(v.clone()),
                    serde_json::Value::from,
                )
            } else {
                serde_json::Value::String(v.clone())
            };
            obj.insert(k.clone(), value);
        }
        serde_json::Value::Object(obj).to_string()
    }
}

#[cfg(test)]
mod tests {
    // 测试断言中的 unwrap/expect 是惯用法：失败即测试失败，展开错误链无意义。
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn session_options_json_shape() {
        let opts = SessionOptions::new()
            .session_len_sec(100.0)
            .graph_capacity_mode(session::graph_capacity::GROW);
        let v: serde_json::Value = serde_json::from_str(&opts.to_json()).unwrap();
        assert_eq!(
            v,
            serde_json::json!({
                "session_len_sec": 100.0,
                "graph_capacity_mode": "grow",
            })
        );
    }

    #[test]
    fn session_options_empty_and_custom() {
        assert_eq!(SessionOptions::new().to_json(), "{}");
        let opts = SessionOptions::new()
            .qwen3_aligner_model_path("./align.gguf")
            .option("custom_key", "v");
        let v: serde_json::Value = serde_json::from_str(&opts.to_json()).unwrap();
        assert_eq!(
            v,
            serde_json::json!({
                "qwen3_asr.forced_aligner_model_path": "./align.gguf",
                "custom_key": "v",
            })
        );
    }
}
