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
    /// 音频生成（Audio Generation，如 MiniMax Music3 音乐生成）
    AudioGeneration,
    /// 语音合成（TTS）
    Tts,
    /// 语音克隆（Voice Cloning）
    VoiceCloning,
    /// 语音转换（Voice Conversion）
    VoiceConversion,
    /// 语音到语音（Speech-to-Speech）
    SpeechToSpeech,
    /// 强制对齐（Alignment）
    Alignment,
    /// 声音设计（Voice Design）
    VoiceDesign,
    /// 说话人识别（Speaker Recognition）
    SpeakerRecognition,
    /// 歌唱语音转换（SVC）
    Svc,
    /// MIDI 生成
    Midi,
}

impl TaskKind {
    /// 转换为传给 C 边界的字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            TaskKind::Vad => "vad",
            TaskKind::Asr => "asr",
            TaskKind::Diar => "diar",
            TaskKind::SourceSeparation => "sep",
            TaskKind::AudioGeneration => "gen",
            TaskKind::Tts => "tts",
            TaskKind::VoiceCloning => "clon",
            TaskKind::VoiceConversion => "vc",
            TaskKind::SpeechToSpeech => "s2s",
            TaskKind::Alignment => "align",
            TaskKind::VoiceDesign => "vdes",
            TaskKind::SpeakerRecognition => "spk",
            TaskKind::Svc => "svc",
            TaskKind::Midi => "midi",
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
    /// SenseVoice ASR（阿里，社区模型）
    SenseAsr,
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
    /// FireRed Audio（语音理解 / 生成，含 Qwen3.5 运行时）
    FireredAudio,
    /// Granite 5 ASR（IBM Granite Speech 5.0 470M TurboCTC ASR，社区模型）
    Granite5Asr,
    /// Audio8 ASR（社区 ASR 模型）
    Audio8Asr,
    /// Audio8 TTS（社区 TTS 模型）
    Audio8Tts,

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
    /// MOSS VoiceGen（语音生成）
    MossVoicegen,
    /// Neutts
    Neutts,
    /// Outetts
    Outetts,
    /// Pocket TTS
    PocketTts,
    /// Vietneu TTS
    VietneuTts,
    /// FireRed TTS3（零样本语音合成）
    Fireredtts3,
    /// F5 TTS
    F5Tts,
    /// Magpie TTS
    MagpieTts,
    /// PersonaPlex（多角色对话语音）
    Personaplex,
    /// MiniMax H3
    MinimaxH3,
    /// Soprano TTS
    SopranoTts,
    /// Echo TTS
    EchoTts,
    /// VoxCPM1 TTS
    Voxcpm1,

    // ---- 说话人分离 / 语音转换 / 音乐生成 ----
    /// SortFormer 说话人分离（Diar）
    SortformerDiar,
    /// Seed VC（语音转换）
    SeedVc,
    /// RVC（语音转换）
    Rvc,
    /// MeanVC2（语音转换）
    Meanvc2,
    /// Chatterbox（说话人还原 / TTS）
    Chatterbox,
    /// Chatterbox Turbo TTS（快速推理变体）
    ChatterboxTurbo,
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
    /// MiniMax Music3（音乐生成）
    MinimaxMusic3,
    /// Supertonic（音乐生成）
    Supertonic,
    /// Voxtral Realtime
    VoxtralRealtime,
    /// AudioSR（语音/音频超分辨率）
    Audiosr,
    /// ControlFoley（可控电影音效生成）
    ControlFoley,
    /// MidoAudio / MIdashengLM 生成式音频
    MidashEnglmGen,

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
    /// MMS Forced Aligner（强制对齐，社区模型）
    MmsForcedAligner,

    /// 未收录的模型族名（原样传给 C 边界）。
    Custom(String),
}

impl ModelFamily {
    /// 按文件名关键词推断模型族。
    ///
    /// GGUF / NeMo safetensors 无法被引擎自动探测族别，加载时必须显式指定。
    /// 上游社区习惯以模型族名命名权重文件（如 `qwen3-asr-0.6b-q8_0.gguf`、
    /// `sortformer-diar-4spk-v1-q8_0.gguf`），本函数从文件名中的关键词
    /// 匹配到对应 [`ModelFamily`]；无法识别时返回 `None`，调用方应回退到
    /// 显式 hint 或 [`ModelFamily::Custom`]。
    ///
    /// 匹配规则按关键词**首次命中**，文件名需包含明确的族名（如 `qwen3`、
    /// `citrinet`、`moss-tts-nano`）。`#family=` 片段（`path#family=qwen3_asr`）
    /// 会被剥离后参与匹配，同时优先于关键词命中。
    ///
    /// # 示例
    ///
    /// ```
    /// use audio_cpp::ModelFamily;
    ///
    /// assert_eq!(
    ///     ModelFamily::from_path("models/qwen3-asr-0.6b-q8_0.gguf"),
    ///     Some(ModelFamily::Qwen3Asr),
    /// );
    /// assert_eq!(
    ///     ModelFamily::from_path("model.gguf#family=citrinet_asr"),
    ///     Some(ModelFamily::CitrinetAsr),
    /// );
    /// assert_eq!(ModelFamily::from_path("my-weights.gguf"), None);
    /// ```
    pub fn from_path(path: &str) -> Option<ModelFamily> {
        let lower = path.to_ascii_lowercase();
        // `#family=xxx` 片段：显式指定优先，直接映射后返回。
        if let Some((_, f)) = lower.rsplit_once("#family=") {
            return Some(ModelFamily::from(f.trim()));
        }
        // 关键词 → 族：按文件名常用命名约定匹配。
        const KEYWORDS: &[(&str, ModelFamily)] = &[
            ("silero_vad", ModelFamily::SileroVad),
            ("silero-vad", ModelFamily::SileroVad),
            ("marblenet_vad", ModelFamily::MarblenetVad),
            ("marblenet-vad", ModelFamily::MarblenetVad),
            ("marblenet", ModelFamily::MarblenetVad),
            ("qwen3_asr", ModelFamily::Qwen3Asr),
            ("qwen3-asr", ModelFamily::Qwen3Asr),
            ("qwen3", ModelFamily::Qwen3Asr),
            ("citrinet", ModelFamily::CitrinetAsr),
            ("sense_asr", ModelFamily::SenseAsr),
            ("sense-asr", ModelFamily::SenseAsr),
            ("sensevoice", ModelFamily::SenseAsr),
            ("fun-asr-nano", ModelFamily::FunAsrNano),
            ("fun_asr_nano", ModelFamily::FunAsrNano),
            ("funasr", ModelFamily::FunAsrNano),
            ("higgs_audio_stt", ModelFamily::HiggsAudioStt),
            ("higgs_audio_tts", ModelFamily::HiggsAudioTts),
            ("higgs", ModelFamily::HiggsAudioStt),
            ("hviske", ModelFamily::HviskeAsr),
            ("kroko", ModelFamily::KrokoAsr),
            ("nemotron", ModelFamily::NemotronAsr),
            ("parakeet", ModelFamily::ParakeetTdt),
            ("vibevoice_asr", ModelFamily::VibevoiceAsr),
            ("firered_audio", ModelFamily::FireredAudio),
            ("firered-audio", ModelFamily::FireredAudio),
            ("granite5asr", ModelFamily::Granite5Asr),
            ("granite-speech", ModelFamily::Granite5Asr),
            ("granite_speech", ModelFamily::Granite5Asr),
            ("granite", ModelFamily::Granite5Asr),
            ("audio8_asr", ModelFamily::Audio8Asr),
            ("audio8-asr", ModelFamily::Audio8Asr),
            ("audio8asr", ModelFamily::Audio8Asr),
            ("arkasr", ModelFamily::Audio8Asr),
            ("audio8_tts", ModelFamily::Audio8Tts),
            ("audio8-tts", ModelFamily::Audio8Tts),
            ("audio8tts", ModelFamily::Audio8Tts),
            ("fireredtts3", ModelFamily::Fireredtts3),
            ("firered_tts3", ModelFamily::Fireredtts3),
            ("firered-tts3", ModelFamily::Fireredtts3),
            ("vibevoice", ModelFamily::Vibevoice),
            ("qwen3_tts", ModelFamily::Qwen3Tts),
            ("qwen3-tts", ModelFamily::Qwen3Tts),
            ("confucius", ModelFamily::Confucius4Tts),
            ("dots_tts", ModelFamily::DotsTts),
            ("dots-tts", ModelFamily::DotsTts),
            ("fish_audio", ModelFamily::FishAudio),
            ("fish-audio", ModelFamily::FishAudio),
            ("glm_tts", ModelFamily::GlmTts),
            ("glm-tts", ModelFamily::GlmTts),
            ("index_tts2", ModelFamily::IndexTts2),
            ("index-tts2", ModelFamily::IndexTts2),
            ("irodori", ModelFamily::IrodoriTts),
            ("moss-tts-nano", ModelFamily::MossTtsNano),
            ("moss_tts_nano", ModelFamily::MossTtsNano),
            ("moss-tts-local", ModelFamily::MossTtsLocal),
            ("moss_tts_local", ModelFamily::MossTtsLocal),
            ("moss_voicegen", ModelFamily::MossVoicegen),
            ("moss-voicegen", ModelFamily::MossVoicegen),
            ("moss", ModelFamily::MossTtsNano),
            ("neutts", ModelFamily::Neutts),
            ("outetts", ModelFamily::Outetts),
            ("pocket_tts", ModelFamily::PocketTts),
            ("pocket-tts", ModelFamily::PocketTts),
            ("vietneu", ModelFamily::VietneuTts),
            ("f5_tts", ModelFamily::F5Tts),
            ("f5-tts", ModelFamily::F5Tts),
            ("magpie", ModelFamily::MagpieTts),
            ("personaplex", ModelFamily::Personaplex),
            ("soprano_tts", ModelFamily::SopranoTts),
            ("soprano-tts", ModelFamily::SopranoTts),
            ("soprano", ModelFamily::SopranoTts),
            ("echo_tts", ModelFamily::EchoTts),
            ("echo-tts", ModelFamily::EchoTts),
            ("echo", ModelFamily::EchoTts),
            ("voxcpm1", ModelFamily::Voxcpm1),
            ("voxcpm-1", ModelFamily::Voxcpm1),
            ("minimax_h3", ModelFamily::MinimaxH3),
            ("minimax-h3", ModelFamily::MinimaxH3),
            ("sortformer-diar", ModelFamily::SortformerDiar),
            ("sortformer_diar", ModelFamily::SortformerDiar),
            ("sortformer", ModelFamily::SortformerDiar),
            ("seed_vc", ModelFamily::SeedVc),
            ("seed-vc", ModelFamily::SeedVc),
            ("seedvc", ModelFamily::SeedVc),
            ("rvc", ModelFamily::Rvc),
            ("meanvc2", ModelFamily::Meanvc2),
            ("mean-vc2", ModelFamily::Meanvc2),
            ("chatterbox_turbo", ModelFamily::ChatterboxTurbo),
            ("chatterbox-turbo", ModelFamily::ChatterboxTurbo),
            ("chatterbox", ModelFamily::Chatterbox),
            ("vevo2", ModelFamily::Vevo2),
            ("voxcpm2", ModelFamily::Voxcpm2),
            ("ace_step", ModelFamily::AceStep),
            ("ace-step", ModelFamily::AceStep),
            ("htdemucs", ModelFamily::Htdemucs),
            ("demucs", ModelFamily::Htdemucs),
            ("mel-band-roformer", ModelFamily::MelBandRoformer),
            ("mel_band_roformer", ModelFamily::MelBandRoformer),
            ("bs-roformer", ModelFamily::BsRoformer),
            ("bs_roformer", ModelFamily::BsRoformer),
            ("muscriptor", ModelFamily::Muscriptor),
            ("omnivoice", ModelFamily::Omnivoice),
            ("stable_audio", ModelFamily::StableAudio),
            ("stable-audio", ModelFamily::StableAudio),
            ("minimax_music3", ModelFamily::MinimaxMusic3),
            ("minimax-music3", ModelFamily::MinimaxMusic3),
            ("supertonic", ModelFamily::Supertonic),
            ("voxtral-realtime", ModelFamily::VoxtralRealtime),
            ("voxtral", ModelFamily::VoxtralRealtime),
            ("audiosr", ModelFamily::Audiosr),
            ("audio-sr", ModelFamily::Audiosr),
            ("controlfoley", ModelFamily::ControlFoley),
            ("control-foley", ModelFamily::ControlFoley),
            ("midashenglm_gen", ModelFamily::MidashEnglmGen),
            ("midashenglm-gen", ModelFamily::MidashEnglmGen),
            ("midashen", ModelFamily::MidashEnglmGen),
            ("miocodec", ModelFamily::Miocodec),
            ("miotts", ModelFamily::Miotts),
            ("dramabox", ModelFamily::Dramabox),
            ("heartmula", ModelFamily::Heartmula),
            ("inflect", ModelFamily::InflectV2),
            ("qwen3_forced_aligner", ModelFamily::Qwen3ForcedAligner),
            ("mms_forced_aligner", ModelFamily::MmsForcedAligner),
            ("mms", ModelFamily::MmsForcedAligner),
            ("forced-aligner", ModelFamily::Qwen3ForcedAligner),
        ];
        KEYWORDS
            .iter()
            .find(|(k, _)| lower.contains(k))
            .map(|(_, family)| family.clone())
    }

    /// 转换为传给 C 边界的字符串。
    pub fn as_str(&self) -> &str {
        match self {
            ModelFamily::SileroVad => "silero_vad",
            ModelFamily::MarblenetVad => "marblenet_vad",
            ModelFamily::Qwen3Asr => "qwen3_asr",
            ModelFamily::CitrinetAsr => "citrinet_asr",
            ModelFamily::SenseAsr => "sense_asr",
            ModelFamily::FunAsrNano => "fun_asr_nano",
            ModelFamily::HiggsAudioStt => "higgs_audio_stt",
            ModelFamily::HviskeAsr => "hviske_asr",
            ModelFamily::KrokoAsr => "kroko_asr",
            ModelFamily::NemotronAsr => "nemotron_asr",
            ModelFamily::ParakeetTdt => "parakeet_tdt",
            ModelFamily::VibevoiceAsr => "vibevoice_asr",
            ModelFamily::FireredAudio => "firered_audio",
            ModelFamily::Granite5Asr => "granite5asr",
            ModelFamily::Audio8Asr => "audio8_asr",
            ModelFamily::Audio8Tts => "audio8_tts",
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
            ModelFamily::MossVoicegen => "moss_voicegen",
            ModelFamily::Neutts => "neutts",
            ModelFamily::Outetts => "outetts",
            ModelFamily::PocketTts => "pocket_tts",
            ModelFamily::VietneuTts => "vietneu_tts",
            ModelFamily::Fireredtts3 => "fireredtts3",
            ModelFamily::F5Tts => "f5_tts",
            ModelFamily::MagpieTts => "magpie_tts",
            ModelFamily::Personaplex => "personaplex",
            ModelFamily::SopranoTts => "soprano_tts",
            ModelFamily::EchoTts => "echo_tts",
            ModelFamily::Voxcpm1 => "voxcpm1",
            ModelFamily::MinimaxH3 => "minimax_h3",
            ModelFamily::SortformerDiar => "sortformer_diar",
            ModelFamily::SeedVc => "seed_vc",
            ModelFamily::Rvc => "rvc",
            ModelFamily::Meanvc2 => "meanvc2",
            ModelFamily::Chatterbox => "chatterbox",
            ModelFamily::ChatterboxTurbo => "chatterbox_turbo",
            ModelFamily::Vevo2 => "vevo2",
            ModelFamily::Voxcpm2 => "voxcpm2",
            ModelFamily::AceStep => "ace_step",
            ModelFamily::Htdemucs => "htdemucs",
            ModelFamily::MelBandRoformer => "mel_band_roformer",
            ModelFamily::BsRoformer => "bs_roformer",
            ModelFamily::Muscriptor => "muscriptor",
            ModelFamily::Omnivoice => "omnivoice",
            ModelFamily::StableAudio => "stable_audio",
            ModelFamily::MinimaxMusic3 => "minimax_music3",
            ModelFamily::Supertonic => "supertonic",
            ModelFamily::VoxtralRealtime => "voxtral_realtime",
            ModelFamily::Audiosr => "audiosr",
            ModelFamily::ControlFoley => "controlfoley",
            ModelFamily::MidashEnglmGen => "midashenglm_gen",
            ModelFamily::Miocodec => "miocodec",
            ModelFamily::Miotts => "miotts",
            ModelFamily::Vibevoice => "vibevoice",
            ModelFamily::Dramabox => "dramabox",
            ModelFamily::Heartmula => "heartmula",
            ModelFamily::InflectV2 => "inflect_v2",
            ModelFamily::Qwen3ForcedAligner => "qwen3_forced_aligner",
            ModelFamily::MmsForcedAligner => "mms_forced_aligner",
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
            "sense_asr" => ModelFamily::SenseAsr,
            "fun_asr_nano" => ModelFamily::FunAsrNano,
            "higgs_audio_stt" => ModelFamily::HiggsAudioStt,
            "hviske_asr" => ModelFamily::HviskeAsr,
            "kroko_asr" => ModelFamily::KrokoAsr,
            "nemotron_asr" => ModelFamily::NemotronAsr,
            "parakeet_tdt" => ModelFamily::ParakeetTdt,
            "vibevoice_asr" => ModelFamily::VibevoiceAsr,
            "firered_audio" => ModelFamily::FireredAudio,
            "granite5asr" => ModelFamily::Granite5Asr,
            "granite_speech5_asr" => ModelFamily::Granite5Asr,
            "granite_speech" => ModelFamily::Granite5Asr,
            "granite_speech5_ctc" => ModelFamily::Granite5Asr,
            "audio8_asr" => ModelFamily::Audio8Asr,
            "arkasr" => ModelFamily::Audio8Asr,
            "audio8_tts" => ModelFamily::Audio8Tts,
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
            "moss_voicegen" => ModelFamily::MossVoicegen,
            "neutts" => ModelFamily::Neutts,
            "outetts" => ModelFamily::Outetts,
            "pocket_tts" => ModelFamily::PocketTts,
            "vietneu_tts" => ModelFamily::VietneuTts,
            "fireredtts3" => ModelFamily::Fireredtts3,
            "f5_tts" => ModelFamily::F5Tts,
            "magpie_tts" => ModelFamily::MagpieTts,
            "personaplex" => ModelFamily::Personaplex,
            "soprano_tts" => ModelFamily::SopranoTts,
            "echo_tts" => ModelFamily::EchoTts,
            "voxcpm1" => ModelFamily::Voxcpm1,
            "minimax_h3" => ModelFamily::MinimaxH3,
            "sortformer_diar" => ModelFamily::SortformerDiar,
            "seed_vc" => ModelFamily::SeedVc,
            "rvc" => ModelFamily::Rvc,
            "meanvc2" => ModelFamily::Meanvc2,
            "chatterbox" => ModelFamily::Chatterbox,
            "chatterbox_turbo" => ModelFamily::ChatterboxTurbo,
            "vevo2" => ModelFamily::Vevo2,
            "voxcpm2" => ModelFamily::Voxcpm2,
            "ace_step" => ModelFamily::AceStep,
            "htdemucs" => ModelFamily::Htdemucs,
            "mel_band_roformer" => ModelFamily::MelBandRoformer,
            "bs_roformer" => ModelFamily::BsRoformer,
            "muscriptor" => ModelFamily::Muscriptor,
            "omnivoice" => ModelFamily::Omnivoice,
            "stable_audio" => ModelFamily::StableAudio,
            "minimax_music3" => ModelFamily::MinimaxMusic3,
            "supertonic" => ModelFamily::Supertonic,
            "voxtral_realtime" => ModelFamily::VoxtralRealtime,
            "audiosr" => ModelFamily::Audiosr,
            "controlfoley" => ModelFamily::ControlFoley,
            "midashenglm_gen" => ModelFamily::MidashEnglmGen,
            "miocodec" => ModelFamily::Miocodec,
            "miotts" => ModelFamily::Miotts,
            "vibevoice" => ModelFamily::Vibevoice,
            "dramabox" => ModelFamily::Dramabox,
            "heartmula" => ModelFamily::Heartmula,
            "inflect_v2" => ModelFamily::InflectV2,
            "qwen3_forced_aligner" => ModelFamily::Qwen3ForcedAligner,
            "mms_forced_aligner" => ModelFamily::MmsForcedAligner,
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
    /// 该段文本（部分模型产出，如带转写的说话人分离）。
    #[serde(default)]
    pub text: String,
}

/// 词级时间戳（ASR 逐词对齐）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WordTimestamp {
    /// 时间范围（采样点）。
    pub span: TimeSpan,
    /// 词文本。
    pub word: String,
    /// 置信度（0..1）。
    pub confidence: f32,
}

/// 模型产出的产物（embedding / 状态 / 对齐结果等）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VoiceArtifact {
    /// 产物种类（如 `speaker_embedding` / `transcript_alignment`）。
    pub kind: String,
    /// 产物 id。
    pub id: String,
    /// 二进制载荷的 base64 编码（无载荷时为空字符串）。
    #[serde(default)]
    pub payload_base64: String,
    /// 附加元数据。
    #[serde(default)]
    pub meta: BTreeMap<String, String>,
}

/// CLI 选项信息（模型支持的请求 / 会话 / 加载选项）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CliOption {
    /// 选项名。
    pub name: String,
    /// 值名称（帮助文本）。
    pub value_name: String,
    /// 描述。
    pub description: String,
    /// 是否必填。
    pub required: bool,
    /// 默认值（可选）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
    /// 最小值（可选，字符串形式）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_value: Option<String>,
    /// 最大值（可选，字符串形式）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_value: Option<String>,
}

/// 模型预检时发现的配置 / 权重资产。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NamedAsset {
    /// 资产 id。
    pub id: String,
    /// 资产路径。
    pub path: String,
}

/// 模型的三类 CLI 选项（请求 / 会话 / 加载）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CliInterface {
    /// 请求级选项（如 `audio_chunk_seconds`）。
    #[serde(default)]
    pub request_options: Vec<CliOption>,
    /// 会话级选项。
    #[serde(default)]
    pub session_options: Vec<CliOption>,
    /// 加载级选项（如 `weight_id`）。
    #[serde(default)]
    pub load_options: Vec<CliOption>,
}

/// 模型预检结果（`Registry::inspect`）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelInspection {
    /// 模型元数据。
    pub metadata: ModelMetadata,
    /// 能力集合。
    pub capabilities: Capabilities,
    /// CLI 选项（请求 / 会话 / 加载）。
    pub cli: CliInterface,
    /// 发现的配置文件。
    #[serde(default)]
    pub discovered_configs: Vec<NamedAsset>,
    /// 发现的权重文件。
    #[serde(default)]
    pub discovered_weights: Vec<NamedAsset>,
    /// 模型根目录。
    #[serde(default)]
    pub model_root: String,
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
    /// 词级时间戳（ASR 逐词对齐，存在时）。
    #[serde(default)]
    pub word_timestamps: Vec<WordTimestamp>,
    /// 单个产物输出（存在时，如 voice-clone 的 cached_voice_id）。
    #[serde(default)]
    pub artifact_output: Option<VoiceArtifact>,
    /// 多个产物输出（存在时）。
    #[serde(default)]
    pub output_artifacts: Vec<VoiceArtifact>,
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
    /// 说话人分段（流式说话人分离等，存在时）。
    #[serde(default)]
    pub speaker_turns: Vec<SpeakerTurn>,
    /// 词级时间戳（流式 ASR 逐词对齐，存在时）。
    #[serde(default)]
    pub word_timestamps: Vec<WordTimestamp>,
    /// 产物输出（流式产出 embedding / 状态等，存在时）。
    #[serde(default)]
    pub output_artifacts: Vec<VoiceArtifact>,
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

#[cfg(test)]
mod tests {
    // 测试断言中的 unwrap/expect 是惯用法：失败即测试失败，展开错误链无意义。
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    /// 所有非 Custom 变体的 `as_str()` 必须能被 `From<&str>` 精确回收到
    /// 原变体（而不是落入 `Custom`），且 `as_str` 与 `From` 两张 match
    /// 表完全一致——防止新增模型族时漏改其一。
    #[test]
    fn model_family_roundtrip() {
        let mut variants: Vec<ModelFamily> = Vec::new();
        macro_rules! push {
            ($($v:ident),* $(,)?) => {
                $(variants.push(ModelFamily::$v);)*
            };
        }
        push![
            // VAD
            SileroVad,
            MarblenetVad,
            // ASR
            Qwen3Asr,
            CitrinetAsr,
            SenseAsr,
            FunAsrNano,
            HiggsAudioStt,
            HviskeAsr,
            KrokoAsr,
            NemotronAsr,
            ParakeetTdt,
            VibevoiceAsr,
            FireredAudio,
            Granite5Asr,
            Audio8Asr,
            Audio8Tts,
            // TTS
            Qwen3Tts,
            Confucius4Tts,
            DotsTts,
            FishAudio,
            GlmTts,
            HiggsAudioTts,
            IndexTts2,
            IrodoriTts,
            MossTtsLocal,
            MossTtsNano,
            MossVoicegen,
            Neutts,
            Outetts,
            PocketTts,
            VietneuTts,
            Fireredtts3,
            F5Tts,
            MagpieTts,
            Personaplex,
            SopranoTts,
            EchoTts,
            Voxcpm1,
            MinimaxH3,
            // 分离 / 转换 / 音乐
            SortformerDiar,
            SeedVc,
            Rvc,
            Meanvc2,
            Chatterbox,
            ChatterboxTurbo,
            Vevo2,
            Voxcpm2,
            AceStep,
            Htdemucs,
            MelBandRoformer,
            BsRoformer,
            Muscriptor,
            Omnivoice,
            StableAudio,
            Supertonic,
            VoxtralRealtime,
            Audiosr,
            ControlFoley,
            MidashEnglmGen,
            // 其他
            Miocodec,
            Miotts,
            Vibevoice,
            Dramabox,
            Heartmula,
            InflectV2,
            Qwen3ForcedAligner,
            MmsForcedAligner,
        ];
        assert!(
            variants.len() >= 40,
            "ModelFamily 应至少覆盖上游全部 40+ loader 族，当前 {}",
            variants.len()
        );
        for v in variants {
            let name = v.as_str();
            assert_eq!(
                ModelFamily::from(name),
                v,
                "as_str() 与 From<&str> 不一致：{}",
                name
            );
            // Display / AsRef 应等价于 as_str。
            assert_eq!(format!("{v}"), name);
            assert_eq!(v.as_ref(), name);
        }
    }

    #[test]
    fn model_family_custom_fallback() {
        // 未收录的名字落入 Custom 并原样保留。
        let m = ModelFamily::from("brand_new_family");
        match &m {
            ModelFamily::Custom(s) => assert_eq!(s, "brand_new_family"),
            other => panic!("未收录名字应落入 Custom，得到 {other:?}"),
        }
        assert_eq!(m.as_str(), "brand_new_family");
        // Custom 值本身不再被 From 回收。
        assert_eq!(
            ModelFamily::from("brand_new_family"),
            ModelFamily::Custom("brand_new_family".to_owned())
        );
        // From<String> 与 AsRef 一致。
        assert_eq!(
            ModelFamily::from("qwen3_asr".to_owned()),
            ModelFamily::Qwen3Asr
        );
    }

    #[test]
    fn model_family_from_path() {
        // 关键词命中（常见 GGUF 命名）
        assert_eq!(
            ModelFamily::from_path("models/qwen3-asr-0.6b-q8_0.gguf"),
            Some(ModelFamily::Qwen3Asr)
        );
        assert_eq!(
            ModelFamily::from_path("sortformer-diar-4spk-v1-q8_0.gguf"),
            Some(ModelFamily::SortformerDiar)
        );
        assert_eq!(
            ModelFamily::from_path("citrinet-asr-q8_0.gguf"),
            Some(ModelFamily::CitrinetAsr)
        );
        assert_eq!(
            ModelFamily::from_path("moss-tts-nano-q8_0.gguf"),
            Some(ModelFamily::MossTtsNano)
        );
        assert_eq!(
            ModelFamily::from_path("htdemucs-6s-q8_0.gguf"),
            Some(ModelFamily::Htdemucs)
        );
        // 大小写不敏感
        assert_eq!(
            ModelFamily::from_path("Qwen3-ASR.Q8_0.GGUF"),
            Some(ModelFamily::Qwen3Asr)
        );
        // #family= 显式覆盖优先于关键词
        assert_eq!(
            ModelFamily::from_path("model.gguf#family=citrinet_asr"),
            Some(ModelFamily::CitrinetAsr)
        );
        assert_eq!(
            ModelFamily::from_path("model.gguf#family=qwen3_tts"),
            Some(ModelFamily::Qwen3Tts)
        );
        // 无法识别
        assert_eq!(ModelFamily::from_path("my-weights.gguf"), None);
        assert_eq!(ModelFamily::from_path(""), None);
    }

    #[test]
    fn task_kind_as_str() {
        let cases = [
            (TaskKind::Vad, "vad"),
            (TaskKind::Asr, "asr"),
            (TaskKind::Diar, "diar"),
            (TaskKind::SourceSeparation, "sep"),
            (TaskKind::AudioGeneration, "gen"),
            (TaskKind::Tts, "tts"),
            (TaskKind::VoiceCloning, "clon"),
            (TaskKind::VoiceConversion, "vc"),
            (TaskKind::SpeechToSpeech, "s2s"),
            (TaskKind::Alignment, "align"),
            (TaskKind::VoiceDesign, "vdes"),
            (TaskKind::SpeakerRecognition, "spk"),
            (TaskKind::Svc, "svc"),
            (TaskKind::Midi, "midi"),
        ];
        for (k, want) in cases {
            assert_eq!(k.as_str(), want);
        }
    }

    #[test]
    fn run_mode_as_str() {
        assert_eq!(RunMode::Offline.as_str(), "offline");
        assert_eq!(RunMode::Streaming.as_str(), "streaming");
    }

    #[test]
    fn backend_as_str() {
        let cases = [
            (Backend::Cpu, "cpu"),
            (Backend::Cuda, "cuda"),
            (Backend::Hip, "hip"),
            (Backend::Vulkan, "vulkan"),
            (Backend::Metal, "metal"),
            (Backend::Best, "best"),
        ];
        for (b, want) in cases {
            assert_eq!(b.as_str(), want);
        }
    }

    /// 结构化类型应能从 C 侧 `dump_*` 产出的 JSON 反序列化（serde 契约）。
    #[test]
    fn structured_types_deserialize() {
        let result: TaskResult = serde_json::from_str(
            r#"{
                "speech_segments": [
                    {"span": {"start_sample": 0, "end_sample": 1600}, "confidence": 0.95, "text": ""}
                ],
                "text_output": {"text": "hi", "language": "en"},
                "audio_output": {"sample_rate": 24000, "channels": 1, "sample_count": 0, "samples": []},
                "named_audio_outputs": [],
                "speaker_turns": [
                    {"speaker_id": "speaker_0", "span": {"start_sample": 0, "end_sample": 1600}, "confidence": 0.9, "text": "hello"}
                ],
                "word_timestamps": [
                    {"span": {"start_sample": 0, "end_sample": 320}, "word": "hello", "confidence": 0.99}
                ],
                "artifact_output": {"kind": "speaker_embedding", "id": "spk1", "payload_base64": "", "meta": {}},
                "output_artifacts": [
                    {"kind": "custom", "id": "a", "payload_base64": "AQID", "meta": {"k": "v"}}
                ]
            }"#,
        )
        .unwrap();
        assert_eq!(result.speech_segments.len(), 1);
        assert_eq!(result.speech_segments[0].span.start_sample, 0);
        assert_eq!(result.speech_segments[0].confidence, 0.95);
        assert_eq!(result.text_output.as_ref().unwrap().text, "hi");
        assert_eq!(result.audio_output.unwrap().sample_rate, 24000);
        assert_eq!(result.speaker_turns[0].text, "hello");
        assert_eq!(result.word_timestamps[0].word, "hello");
        assert_eq!(
            result.artifact_output.as_ref().unwrap().kind,
            "speaker_embedding"
        );
        assert_eq!(result.output_artifacts[0].payload_base64, "AQID");

        let ev: StreamEvent = serde_json::from_str(
            r#"{
                "voice_activity": [
                    {"kind": "speech_start", "sample": 100, "probability": 0.8, "segment": null}
                ],
                "partial_text": null,
                "audio_output": null,
                "named_audio_outputs": [{"id": "chunk_0", "audio": {"sample_rate": 48000, "channels": 2, "sample_count": 320, "samples": []}, "meta": {}}],
                "speaker_turns": [],
                "word_timestamps": [],
                "output_artifacts": [],
                "is_final": true
            }"#,
        )
        .unwrap();
        assert_eq!(ev.voice_activity[0].kind, "speech_start");
        assert_eq!(ev.named_audio_outputs[0].id, "chunk_0");
        assert!(ev.is_final);
    }

    #[test]
    fn model_inspection_deserialize() {
        // 验证 inspect 返回的 ModelInspection 结构与上游 dump_inspection 对齐，
        // 且缺失字段（无 meta / 无发现资产）能向后兼容。
        let info: ModelInspection = serde_json::from_str(
            r#"{
                "metadata": {
                    "family": "qwen3_asr",
                    "variant": "q8_0",
                    "description": "Qwen3 ASR",
                    "config_candidates": ["config.json"],
                    "weight_candidates": ["qwen3-asr-q8_0.gguf"]
                },
                "capabilities": {
                    "supported_tasks": [{"task": "asr", "modes": ["offline", "streaming"]}],
                    "languages": ["zh", "en"],
                    "supports_speaker_reference": false,
                    "supports_style_condition": false,
                    "supports_timestamps": true
                },
                "discovered_weights": [
                    {"id": "model", "path": "/models/qwen3-asr-q8_0.gguf"}
                ],
                "cli": {
                    "request_options": [
                        {"name": "language", "value_name": "CODE", "description": "语言", "required": false, "default_value": "auto"}
                    ],
                    "session_options": [],
                    "load_options": []
                }
            }"#,
        )
        .unwrap();
        assert_eq!(info.metadata.family, "qwen3_asr");
        assert!(
            info.capabilities
                .supported_tasks
                .iter()
                .any(|t| t.task == "asr" && t.modes.contains(&"streaming".to_string()))
        );
        assert_eq!(info.discovered_weights[0].id, "model");
        assert_eq!(info.cli.request_options[0].name, "language");
        assert_eq!(
            info.cli.request_options[0].default_value.as_deref(),
            Some("auto")
        );
    }
}
