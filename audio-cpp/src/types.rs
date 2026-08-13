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
    /// 音频源分离（Source Separation，如 Demucs 分离人声/鼓/贝斯/其他）
    SourceSeparation,
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
            TaskKind::SourceSeparation => "sep",
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

/// 模型族（映射 `Registry::load` 的 `family_hint` 参数）。
///
/// 枚举覆盖上游 `engine_runtime` 的全部 loader 族，`as_str()` 返回传给
/// C 边界的字符串。GGUF / NeMo safetensors 无法被引擎自动探测族别，加载
/// 时必须显式指定；用枚举代替裸字符串可避免拼写错误。
///
/// 上游社区持续新增模型，本枚举可能滞后：unknown 族可直接用
/// [`ModelFamily::Custom`] 传任意字符串，`From<&str>` 也会把未收录的名字
/// 自动映射到 `Custom`。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ModelFamily {
    // ---- VAD ----
    /// Silero VAD（内置权重，通常可省略 family_hint）
    SileroVad,
    /// MarbleNet VAD（NeMo 格式，须显式指定）
    MarblenetVad,

    // ---- ASR ----
    /// Qwen3 ASR
    Qwen3Asr,
    /// Citrinet ASR
    CitrinetAsr,
    /// Fun ASR Nano
    FunAsrNano,
    /// Higgs Audio STT
    HiggsAudioStt,
    /// Hviske ASR
    HviskeAsr,
    /// Kroko ASR
    KrokoAsr,
    /// Nemotron ASR
    NemotronAsr,
    /// Parakeet TDT
    ParakeetTdt,
    /// Vibevoice ASR
    VibevoiceAsr,

    // ---- TTS ----
    /// Qwen3 TTS
    Qwen3Tts,
    /// Confucius4 TTS
    Confucius4Tts,
    /// Dots TTS
    DotsTts,
    /// Fish Audio
    FishAudio,
    /// GLM TTS
    GlmTts,
    /// Higgs Audio TTS
    HiggsAudioTts,
    /// Index TTS2
    IndexTts2,
    /// Irodori TTS
    IrodoriTts,
    /// MOSS TTS Local
    MossTtsLocal,
    /// MOSS TTS Nano
    MossTtsNano,
    /// Neutts
    Neutts,
    /// Outetts
    Outetts,
    /// Pocket TTS
    PocketTts,
    /// Vietneu TTS
    VietneuTts,
    /// MiniMax H3
    MinimaxH3,

    // ---- 说话人分离 / 语音转换 / 音乐生成 ----
    /// SortFormer 说话人分离（Diar）
    SortformerDiar,
    /// Seed VC（语音转换）
    SeedVc,
    /// RVC（语音转换）
    Rvc,
    /// Chatterbox（说话人还原 / TTS）
    Chatterbox,
    /// Vevo2（零样本 TTS / VC / 语音编辑）
    Vevo2,
    /// VoxCPM2
    Voxcpm2,
    /// Complex Inversion（多任务音频）
    AceStep,

    // ---- 音乐 / 音频生成与分离 ----
    /// HTDemucs 音乐源分离
    Htdemucs,
    /// Mel Band Roformer（乐音建模）
    MelBandRoformer,
    /// BSRoformer
    BsRoformer,
    /// Muscriptor（乐理描述）
    Muscriptor,
    /// Omnivoice（条件自回归生成）
    Omnivoice,
    /// Stable Audio（音频生成）
    StableAudio,
    /// Supertonic（音乐生成）
    Supertonic,
    /// Voxtral Realtime
    VoxtralRealtime,

    // ---- 其他 ----
    /// Miocodec / Miotts（小米语音）
    Miocodec,
    /// Miotts（小米 TTS）
    Miotts,
    /// Vibevoice（语音转换）
    Vibevoice,
    /// Dramabox（剧本播客）
    Dramabox,
    /// Heartmula
    Heartmula,
    /// Inflect V2
    InflectV2,
    /// Qwen3 Forced Aligner（强制对齐）
    Qwen3ForcedAligner,

    /// 未收录的模型族名（原样传给 C 边界）。
    Custom(String),
}

impl ModelFamily {
    /// 转换为传给 C 边界的字符串。
    pub fn as_str(&self) -> &str {
        match self {
            ModelFamily::SileroVad => "silero_vad",
            ModelFamily::MarblenetVad => "marblenet_vad",
            ModelFamily::Qwen3Asr => "qwen3_asr",
            ModelFamily::CitrinetAsr => "citrinet_asr",
            ModelFamily::FunAsrNano => "fun_asr_nano",
            ModelFamily::HiggsAudioStt => "higgs_audio_stt",
            ModelFamily::HviskeAsr => "hviske_asr",
            ModelFamily::KrokoAsr => "kroko_asr",
            ModelFamily::NemotronAsr => "nemotron_asr",
            ModelFamily::ParakeetTdt => "parakeet_tdt",
            ModelFamily::VibevoiceAsr => "vibevoice_asr",
            ModelFamily::Qwen3Tts => "qwen3_tts",
            ModelFamily::Confucius4Tts => "confucius4_tts",
            ModelFamily::DotsTts => "dots_tts",
            ModelFamily::FishAudio => "fish_audio",
            ModelFamily::GlmTts => "glm_tts",
            ModelFamily::HiggsAudioTts => "higgs_audio_tts",
            ModelFamily::IndexTts2 => "index_tts2",
            ModelFamily::IrodoriTts => "irodori_tts",
            ModelFamily::MossTtsLocal => "moss_tts_local",
            ModelFamily::MossTtsNano => "moss_tts_nano",
            ModelFamily::Neutts => "neutts",
            ModelFamily::Outetts => "outetts",
            ModelFamily::PocketTts => "pocket_tts",
            ModelFamily::VietneuTts => "vietneu_tts",
            ModelFamily::MinimaxH3 => "minimax_h3",
            ModelFamily::SortformerDiar => "sortformer_diar",
            ModelFamily::SeedVc => "seed_vc",
            ModelFamily::Rvc => "rvc",
            ModelFamily::Chatterbox => "chatterbox",
            ModelFamily::Vevo2 => "vevo2",
            ModelFamily::Voxcpm2 => "voxcpm2",
            ModelFamily::AceStep => "ace_step",
            ModelFamily::Htdemucs => "htdemucs",
            ModelFamily::MelBandRoformer => "mel_band_roformer",
            ModelFamily::BsRoformer => "bs_roformer",
            ModelFamily::Muscriptor => "muscriptor",
            ModelFamily::Omnivoice => "omnivoice",
            ModelFamily::StableAudio => "stable_audio",
            ModelFamily::Supertonic => "supertonic",
            ModelFamily::VoxtralRealtime => "voxtral_realtime",
            ModelFamily::Miocodec => "miocodec",
            ModelFamily::Miotts => "miotts",
            ModelFamily::Vibevoice => "vibevoice",
            ModelFamily::Dramabox => "dramabox",
            ModelFamily::Heartmula => "heartmula",
            ModelFamily::InflectV2 => "inflect_v2",
            ModelFamily::Qwen3ForcedAligner => "qwen3_forced_aligner",
            ModelFamily::Custom(s) => s,
        }
    }
}

impl From<&str> for ModelFamily {
    /// 从字符串构造：收录的名字映射到对应变体，未收录的落入 `Custom`。
    fn from(s: &str) -> Self {
        match s {
            "silero_vad" => ModelFamily::SileroVad,
            "marblenet_vad" => ModelFamily::MarblenetVad,
            "qwen3_asr" => ModelFamily::Qwen3Asr,
            "citrinet_asr" => ModelFamily::CitrinetAsr,
            "fun_asr_nano" => ModelFamily::FunAsrNano,
            "higgs_audio_stt" => ModelFamily::HiggsAudioStt,
            "hviske_asr" => ModelFamily::HviskeAsr,
            "kroko_asr" => ModelFamily::KrokoAsr,
            "nemotron_asr" => ModelFamily::NemotronAsr,
            "parakeet_tdt" => ModelFamily::ParakeetTdt,
            "vibevoice_asr" => ModelFamily::VibevoiceAsr,
            "qwen3_tts" => ModelFamily::Qwen3Tts,
            "confucius4_tts" => ModelFamily::Confucius4Tts,
            "dots_tts" => ModelFamily::DotsTts,
            "fish_audio" => ModelFamily::FishAudio,
            "glm_tts" => ModelFamily::GlmTts,
            "higgs_audio_tts" => ModelFamily::HiggsAudioTts,
            "index_tts2" => ModelFamily::IndexTts2,
            "irodori_tts" => ModelFamily::IrodoriTts,
            "moss_tts_local" => ModelFamily::MossTtsLocal,
            "moss_tts_nano" => ModelFamily::MossTtsNano,
            "neutts" => ModelFamily::Neutts,
            "outetts" => ModelFamily::Outetts,
            "pocket_tts" => ModelFamily::PocketTts,
            "vietneu_tts" => ModelFamily::VietneuTts,
            "minimax_h3" => ModelFamily::MinimaxH3,
            "sortformer_diar" => ModelFamily::SortformerDiar,
            "seed_vc" => ModelFamily::SeedVc,
            "rvc" => ModelFamily::Rvc,
            "chatterbox" => ModelFamily::Chatterbox,
            "vevo2" => ModelFamily::Vevo2,
            "voxcpm2" => ModelFamily::Voxcpm2,
            "ace_step" => ModelFamily::AceStep,
            "htdemucs" => ModelFamily::Htdemucs,
            "mel_band_roformer" => ModelFamily::MelBandRoformer,
            "bs_roformer" => ModelFamily::BsRoformer,
            "muscriptor" => ModelFamily::Muscriptor,
            "omnivoice" => ModelFamily::Omnivoice,
            "stable_audio" => ModelFamily::StableAudio,
            "supertonic" => ModelFamily::Supertonic,
            "voxtral_realtime" => ModelFamily::VoxtralRealtime,
            "miocodec" => ModelFamily::Miocodec,
            "miotts" => ModelFamily::Miotts,
            "vibevoice" => ModelFamily::Vibevoice,
            "dramabox" => ModelFamily::Dramabox,
            "heartmula" => ModelFamily::Heartmula,
            "inflect_v2" => ModelFamily::InflectV2,
            "qwen3_forced_aligner" => ModelFamily::Qwen3ForcedAligner,
            other => ModelFamily::Custom(other.to_owned()),
        }
    }
}

impl From<String> for ModelFamily {
    fn from(s: String) -> Self {
        ModelFamily::from(s.as_str())
    }
}

impl AsRef<str> for ModelFamily {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for ModelFamily {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
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
    /// 命名音频输出列表（如流式 TTS 的逐块 `chunk_N`）。
    #[serde(default)]
    pub named_audio_outputs: Vec<NamedAudioOutput>,
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
