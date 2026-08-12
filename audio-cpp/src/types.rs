//! 与 `capi.cpp` JSON 输出一一对应的结构化类型。
//!
//! 这些类型由 serde 从 shim 返回的 JSON 反序列化而来，字段名与
//! `capi.cpp` 的 `dump_*` 系列函数保持一致。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// 任务种类（映射 `audiocpp_model_create_task_session` 的 `task` 参数）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskKind {
    /// 语音活动检测（VAD）
    Vad,
    /// 语音识别（ASR）
    Asr,
    /// 说话人分离（Diarization）
    Diar,
    /// 语音合成（TTS）
    Tts,
}

impl TaskKind {
    /// 转换为传给 C 边界的字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            TaskKind::Vad => "vad",
            TaskKind::Asr => "asr",
            TaskKind::Diar => "diar",
            TaskKind::Tts => "tts",
        }
    }
}

/// 会话运行模式（离线 / 流式）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunMode {
    /// 离线：一次请求 → 一次结果。
    Offline,
    /// 流式：持续送入音频、持续产出事件。
    Streaming,
}

impl RunMode {
    /// 转换为传给 C 边界的字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            RunMode::Offline => "offline",
            RunMode::Streaming => "streaming",
        }
    }
}

/// 计算后端（映射 `audiocpp_model_create_task_session` 的 `backend` 参数）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Backend {
    /// CPU
    Cpu,
    /// CUDA
    Cuda,
    /// HIP / ROCm
    Hip,
    /// Vulkan
    Vulkan,
    /// Metal
    Metal,
    /// 自动选择最佳可用后端
    Best,
}

impl Backend {
    /// 转换为传给 C 边界的字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            Backend::Cpu => "cpu",
            Backend::Cuda => "cuda",
            Backend::Hip => "hip",
            Backend::Vulkan => "vulkan",
            Backend::Metal => "metal",
            Backend::Best => "best",
        }
    }
}

/// 计算设备（`audiocpp_registry_devices_json` 的一项）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Device {
    /// 后端名，如 `"CPU"`。
    pub backend: String,
    /// 设备序号。
    pub index: i32,
    /// 设备名。
    pub name: String,
    /// 设备类型，如 `"CPU"`。
    #[serde(rename = "type")]
    pub kind: String,
}

/// 模型族 loader 的声明信息（`audiocpp_registry_loaders_json` 的一项）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoaderInfo {
    /// 模型族名。
    pub family: String,
    /// 能力集合。
    pub capabilities: Capabilities,
    /// 指令策略描述。
    pub instructions_policy: String,
    /// 可用的 API 端点。
    pub api_endpoints: Vec<String>,
}

/// 能力集合（`audiocpp_model_capabilities_json` / loader 声明）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Capabilities {
    /// 支持的任务及其模式。
    pub supported_tasks: Vec<SupportedTask>,
    /// 支持的语言。
    pub languages: Vec<String>,
    /// 是否支持说话人参考音频。
    pub supports_speaker_reference: bool,
    /// 是否支持风格条件。
    pub supports_style_condition: bool,
    /// 是否支持时间戳。
    pub supports_timestamps: bool,
}

/// 一个受支持的任务及其可用模式。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SupportedTask {
    /// 任务名，如 `"vad"`。
    pub task: String,
    /// 可用模式，如 `["offline","streaming"]`。
    pub modes: Vec<String>,
}

/// 已加载模型的元数据（`audiocpp_model_metadata_json`）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelMetadata {
    /// 模型族。
    pub family: String,
    /// 变体。
    pub variant: String,
    /// 描述。
    pub description: String,
    /// 候选配置。
    pub config_candidates: Vec<String>,
    /// 候选权重。
    pub weight_candidates: Vec<String>,
}

/// 一段音频在采样轴上的时间范围。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TimeSpan {
    /// 起始采样点（含）。
    pub start_sample: i64,
    /// 结束采样点（不含）。
    pub end_sample: i64,
}

/// 语音活动片段。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpeechSegment {
    /// 时间范围。
    pub span: TimeSpan,
    /// 置信度（0..1）。
    pub confidence: f32,
    /// 附带文本（如 ASR 片段）。
    pub text: String,
}

/// 说话人分离中的一段发言（谁在什么时间说话）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpeakerTurn {
    /// 说话人 id，如 `"speaker_0"`。
    pub speaker_id: String,
    /// 发言时间范围。
    pub span: TimeSpan,
    /// 置信度（0..1）。
    pub confidence: f32,
}

/// 文本输出（如 ASR / TTS 的文本）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextOutput {
    /// 文本内容。
    pub text: String,
    /// 语言。
    pub language: String,
}

/// 音频输出的描述信息（采样率 / 声道 / 长度）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioBufferInfo {
    /// 采样率。
    pub sample_rate: i32,
    /// 声道数。
    pub channels: i32,
    /// 采样数。
    pub sample_count: usize,
    /// 实际采样数据（f32，-1..1）。仅当生成音频需要回传时存在
    /// （如 TTS 的 `audio_output`）；VAD / ASR 通常不携带。
    #[serde(default)]
    pub samples: Option<Vec<f32>>,
}

/// 带名字的音频输出（如 TTS 的多段输出）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NamedAudioOutput {
    /// 输出 id。
    pub id: String,
    /// 音频描述。
    pub audio: AudioBufferInfo,
    /// 附加元数据。
    pub meta: BTreeMap<String, String>,
}

/// 一次任务执行的完整结果（`audiocpp_session_run_offline` / `finish`）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskResult {
    /// 检测到的语音片段。
    pub speech_segments: Vec<SpeechSegment>,
    /// 说话人分离的发言分段（存在时）。
    pub speaker_turns: Vec<SpeakerTurn>,
    /// 文本输出（存在时）。
    pub text_output: Option<TextOutput>,
    /// 音频输出（存在时）。
    pub audio_output: Option<AudioBufferInfo>,
    /// 命名音频输出列表。
    pub named_audio_outputs: Vec<NamedAudioOutput>,
}

/// 流式会话产出的单个事件。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamEvent {
    /// 语音活动事件列表。
    pub voice_activity: Vec<VoiceActivityEvent>,
    /// 部分文本（流式 ASR 等）。
    pub partial_text: Option<TextOutput>,
    /// 音频输出（存在时）。
    pub audio_output: Option<AudioBufferInfo>,
    /// 是否为最终事件。
    pub is_final: bool,
}

/// 语音活动事件（`speech_start` / `speech_end` / `speech_segment`）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VoiceActivityEvent {
    /// 事件类型。
    pub kind: String,
    /// 触发位置（采样点）。
    pub sample: i64,
    /// 语音概率。
    pub probability: f32,
    /// 片段（`speech_segment` 事件携带）。
    pub segment: Option<SpeechSegment>,
}

/// 流式策略描述（`audiocpp_session_streaming_policy_json`）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamingPolicy {
    /// 输入类型，如 `"audio_chunks"`。
    pub input: String,
    /// 输出类型，如 `"pull_events"` / `"final_result"`。
    pub output: String,
    /// 推荐的音频分块采样数。
    pub preferred_audio_chunk_samples: usize,
    /// 推荐的音频分块秒数。
    pub preferred_audio_chunk_seconds: f64,
}
