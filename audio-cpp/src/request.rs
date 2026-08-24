//! 类型化任务请求（[`Request`]）构造器。
//!
//! 上层 API 的所有请求入口（离线 [`crate::Session::run_offline`]、流式
//! [`crate::Session::start`] / [`crate::Session::prepare`]）都接受
//! [`IntoRequest`]：既可以是本模块的类型化 [`Request`] 枚举，也可以是任意
//! JSON 字符串（透传给 C 边界）。用 [`Request`] 构造时无需手工拼接 / 转义
//! JSON，Windows 路径也无需手动转义反斜杠。
//!
//! 每个任务一种变体，携带各自参数：
//! ```rust
//! use audio_cpp::Request;
//!
//! // 离线 VAD：音频 + 阈值选项
//! let r1 = Request::vad("./speech.wav").option("vad_threshold", 0.5);
//! // 离线 / 流式 ASR
//! let r2 = Request::asr("./speech.wav");
//! let r3 = Request::asr("./speech.wav").option("audio_chunk_seconds", 3.0);
//! // TTS：文本（可选说话人参考，如 Qwen3 TTS 声音克隆）
//! let r4 = Request::tts("Hello!");
//! let r5 = Request::tts("Hello!").reference("./ref.wav").reference_text("参考文本");
//! // 原始 JSON 透传
//! let r6 = Request::json(r#"{"audio_path":"./speech.wav"}"#);
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::audio::WavAudio;
use crate::error::Error;

/// 请求里的音频输入：本地文件路径，或内嵌采样数据。
#[derive(Debug, Clone)]
pub enum AudioInput {
    /// 从本地 WAV 文件读取（等价于 JSON 顶层 `audio_path`）。
    Path(String),
    /// 内嵌音频数据（等价于 JSON 顶层 `audio` 对象）。
    Buffer(WavAudio),
}

impl From<&str> for AudioInput {
    fn from(s: &str) -> Self {
        AudioInput::Path(s.to_owned())
    }
}

impl From<String> for AudioInput {
    fn from(s: String) -> Self {
        AudioInput::Path(s)
    }
}

impl From<&String> for AudioInput {
    fn from(s: &String) -> Self {
        AudioInput::Path(s.clone())
    }
}

impl From<WavAudio> for AudioInput {
    fn from(a: WavAudio) -> Self {
        AudioInput::Buffer(a)
    }
}

impl From<&Path> for AudioInput {
    fn from(p: &Path) -> Self {
        AudioInput::Path(p.to_string_lossy().into_owned())
    }
}

impl From<PathBuf> for AudioInput {
    fn from(p: PathBuf) -> Self {
        AudioInput::Path(p.to_string_lossy().into_owned())
    }
}

/// 音频类任务（VAD / ASR / 说话人分离 / 源分离）的请求参数。
#[derive(Debug, Clone, Default)]
pub struct AudioRequest {
    /// 音频输入（文件路径或内嵌数据）。
    pub audio: Option<AudioInput>,
    /// 附加选项键值（如 VAD 的 `vad_threshold` / `threshold`、流式 ASR 的
    /// `audio_chunk_seconds`）。非字符串值由 shim 字符串化后交给上游。
    pub options: BTreeMap<String, Value>,
}

impl AudioRequest {
    /// 以音频输入构造请求。
    pub fn new(audio: impl Into<AudioInput>) -> Self {
        Self {
            audio: Some(audio.into()),
            options: BTreeMap::new(),
        }
    }

    /// 构造仅含选项的请求（无音频输入）。
    ///
    /// 用于流式 `start` / `prepare` 阶段：此时音频经
    /// [`crate::Session::process_audio`] 逐块送入，请求本身只需携带
    /// `options`（如 `language`、`audio_chunk_seconds`）。
    pub fn options_only() -> Self {
        Self {
            audio: None,
            options: BTreeMap::new(),
        }
    }

    /// 设置一个选项键值（等价于 JSON `options` 里的一个字段）。
    pub fn option<V: Into<Value>>(mut self, key: impl Into<String>, value: V) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }

    /// 批量设置选项。
    pub fn options<K, V>(mut self, opts: impl IntoIterator<Item = (K, V)>) -> Self
    where
        K: Into<String>,
        V: Into<Value>,
    {
        self.options
            .extend(opts.into_iter().map(|(k, v)| (k.into(), v.into())));
        self
    }
}

/// 说话人参考 / 风格条件（映射到上游请求 JSON 的 `voice` 字段）。
///
/// 比把参考音频塞进顶层 `audio` 更语义化：支持「用参考音频克隆音色」、
/// 「用 `cached_voice_id` 复用已注册音色」、以及风格/语速/音高控制。
/// 通过 [`TtsRequest::voice`] 设置。
#[derive(Debug, Clone, Default)]
pub struct VoiceCondition {
    /// 说话人参考音频（声音克隆）。
    pub speaker_audio: Option<AudioInput>,
    /// 已缓存的音色 id（复用之前克隆得到的音色，免去再次上传参考音频）。
    pub cached_voice_id: Option<String>,
    /// 风格：语言。
    pub language: Option<String>,
    /// 风格：情绪。
    pub emotion: Option<String>,
    /// 风格：语速缩放。
    pub speaking_rate: Option<f32>,
    /// 风格：音高偏移。
    pub pitch_shift: Option<f32>,
    /// 风格：能量缩放。
    pub energy_scale: Option<f32>,
    /// 风格：自由标签。
    pub tags: BTreeMap<String, String>,
}

impl VoiceCondition {
    /// 以说话人参考音频（声音克隆）构造。
    pub fn speaker(audio: impl Into<AudioInput>) -> Self {
        Self {
            speaker_audio: Some(audio.into()),
            ..Default::default()
        }
    }

    /// 以已缓存音色 id 构造（复用音色）。
    pub fn cached(id: impl Into<String>) -> Self {
        Self {
            cached_voice_id: Some(id.into()),
            ..Default::default()
        }
    }

    /// 设置语言。
    pub fn language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// 设置情绪。
    pub fn emotion(mut self, emotion: impl Into<String>) -> Self {
        self.emotion = Some(emotion.into());
        self
    }

    /// 设置语速缩放。
    pub fn speaking_rate(mut self, rate: f32) -> Self {
        self.speaking_rate = Some(rate);
        self
    }

    /// 设置音高偏移。
    pub fn pitch_shift(mut self, shift: f32) -> Self {
        self.pitch_shift = Some(shift);
        self
    }

    /// 设置能量缩放。
    pub fn energy_scale(mut self, scale: f32) -> Self {
        self.energy_scale = Some(scale);
        self
    }

    /// 设置一个风格标签。
    pub fn tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.insert(key.into(), value.into());
        self
    }
}

/// 语音合成（TTS）的请求参数。
#[derive(Debug, Clone)]
pub struct TtsRequest {
    /// 待合成文本。
    pub text: String,
    /// 文本语言（可选）。
    pub language: Option<String>,
    /// 说话人参考音频（声音克隆，如 Qwen3 TTS base 变体）。
    pub reference_audio: Option<AudioInput>,
    /// 参考音频的文本转写（经 `options.reference_text` 交给上游）。
    pub reference_text: Option<String>,
    /// 说话人参考 / 风格条件（映射到上游 `voice` 字段）。
    pub voice: Option<VoiceCondition>,
    /// 附加选项键值（如流式 TTS 的 `retry_badcase`）。
    pub options: BTreeMap<String, Value>,
}

impl TtsRequest {
    /// 以待合成文本构造请求。
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            language: None,
            reference_audio: None,
            reference_text: None,
            voice: None,
            options: BTreeMap::new(),
        }
    }

    /// 设置文本语言。
    pub fn language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// 设置说话人参考音频（声音克隆）。
    pub fn reference(mut self, audio: impl Into<AudioInput>) -> Self {
        self.reference_audio = Some(audio.into());
        self
    }

    /// 设置参考音频的文本转写。
    pub fn reference_text(mut self, text: impl Into<String>) -> Self {
        self.reference_text = Some(text.into());
        self
    }

    /// 设置说话人参考 / 风格条件（映射到上游 `voice` 字段）。
    ///
    /// 例：`Request::tts("Hi").voice(VoiceCondition::speaker("./ref.wav").emotion("happy"))`。
    pub fn voice(mut self, voice: VoiceCondition) -> Self {
        self.voice = Some(voice);
        self
    }

    /// 设置一个选项键值（等价于 JSON `options` 里的一个字段）。
    pub fn option<V: Into<Value>>(mut self, key: impl Into<String>, value: V) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }

    /// 批量设置选项。
    pub fn options<K, V>(mut self, opts: impl IntoIterator<Item = (K, V)>) -> Self
    where
        K: Into<String>,
        V: Into<Value>,
    {
        self.options
            .extend(opts.into_iter().map(|(k, v)| (k.into(), v.into())));
        self
    }
}

/// 一次任务请求。
///
/// 对应 capi.cpp `parse_task_request` 读取的顶层 JSON 字段（`text` /
/// `language` / `audio` / `audio_path` / `options`）。按任务种类区分变体，
/// 每个变体携带自己的参数；序列化时统一展开为上述 JSON 形状。
#[derive(Debug, Clone)]
pub enum Request {
    /// 语音活动检测：需音频输入，可配 `vad_threshold` / `threshold` 等。
    Vad(AudioRequest),
    /// 语音识别：需音频输入，流式时用 `audio_chunk_seconds` 控制窗口。
    Asr(AudioRequest),
    /// 说话人分离：需音频输入。
    Diar(AudioRequest),
    /// 音乐源分离（Demucs 等）：需音频输入。
    SourceSeparation(AudioRequest),
    /// 语音合成：需文本，可带说话人参考音频（声音克隆）。
    Tts(TtsRequest),
    /// 原始 JSON 字符串透传（不经任何序列化改动）。
    Json(String),
}

impl Request {
    /// VAD 请求：以音频输入构造。
    pub fn vad(audio: impl Into<AudioInput>) -> Self {
        Request::Vad(AudioRequest::new(audio))
    }

    /// ASR 请求：以音频输入构造。
    pub fn asr(audio: impl Into<AudioInput>) -> Self {
        Request::Asr(AudioRequest::new(audio))
    }

    /// 流式 ASR 请求：仅含选项，无音频输入。
    ///
    /// 用于流式会话的 `start` / `prepare`（音频经
    /// [`crate::Session::process_audio`] 逐块送入，无需 `audio_path`）。
    /// 例如：
    /// ```
    /// use audio_cpp::Request;
    /// let req = Request::stream().option("language", "auto").option("audio_chunk_seconds", 3.0);
    /// assert_eq!(req.to_json().unwrap(), r#"{"options":{"audio_chunk_seconds":3.0,"language":"auto"}}"#);
    /// ```
    pub fn stream() -> Self {
        Request::Asr(AudioRequest::options_only())
    }

    /// 说话人分离请求：以音频输入构造。
    pub fn diar(audio: impl Into<AudioInput>) -> Self {
        Request::Diar(AudioRequest::new(audio))
    }

    /// 音乐源分离请求：以音频输入构造。
    pub fn source_separation(audio: impl Into<AudioInput>) -> Self {
        Request::SourceSeparation(AudioRequest::new(audio))
    }

    /// TTS 请求：以待合成文本构造。
    pub fn tts(text: impl Into<String>) -> Self {
        Request::Tts(TtsRequest::new(text))
    }

    /// 原始 JSON 字符串透传（不经序列化改动，直接交给 C 边界）。
    pub fn json(s: impl Into<String>) -> Self {
        Request::Json(s.into())
    }

    /// 设置一个选项键值（等价于 JSON `options` 里的一个字段）。
    ///
    /// 对 [`Request::Json`] 无效（原始 JSON 不经序列化改动）。
    pub fn option<V: Into<Value>>(self, key: impl Into<String>, value: V) -> Self {
        let key = key.into();
        let value = value.into();
        match self {
            Request::Vad(r) => Request::Vad(r.option(key, value)),
            Request::Asr(r) => Request::Asr(r.option(key, value)),
            Request::Diar(r) => Request::Diar(r.option(key, value)),
            Request::SourceSeparation(r) => Request::SourceSeparation(r.option(key, value)),
            Request::Tts(r) => Request::Tts(r.option(key, value)),
            Request::Json(_) => self,
        }
    }

    /// 批量设置选项。对 [`Request::Json`] 无效。
    pub fn options<K, V>(self, opts: impl IntoIterator<Item = (K, V)>) -> Self
    where
        K: Into<String>,
        V: Into<Value>,
    {
        let opts: Vec<(String, Value)> = opts
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();
        match self {
            Request::Vad(r) => Request::Vad(r.options(opts)),
            Request::Asr(r) => Request::Asr(r.options(opts)),
            Request::Diar(r) => Request::Diar(r.options(opts)),
            Request::SourceSeparation(r) => Request::SourceSeparation(r.options(opts)),
            Request::Tts(r) => Request::Tts(r.options(opts)),
            Request::Json(_) => self,
        }
    }

    /// 设置说话人参考音频（仅对 [`Request::Tts`] 有意义，其余变体忽略）。
    pub fn reference(self, audio: impl Into<AudioInput>) -> Self {
        match self {
            Request::Tts(r) => Request::Tts(r.reference(audio)),
            other => other,
        }
    }

    /// 设置参考音频的文本转写（仅对 [`Request::Tts`] 有意义，其余变体忽略）。
    pub fn reference_text(self, text: impl Into<String>) -> Self {
        match self {
            Request::Tts(r) => Request::Tts(r.reference_text(text)),
            other => other,
        }
    }

    /// 设置说话人参考 / 风格条件（仅对 [`Request::Tts`] 有意义，其余变体忽略）。
    pub fn voice(self, voice: VoiceCondition) -> Self {
        match self {
            Request::Tts(r) => Request::Tts(r.voice(voice)),
            other => other,
        }
    }

    /// 设置文本语言（仅对 [`Request::Tts`] 有意义，其余变体忽略）。
    pub fn language(self, language: impl Into<String>) -> Self {
        match self {
            Request::Tts(r) => Request::Tts(r.language(language)),
            other => other,
        }
    }

    /// 序列化为 JSON 字符串（传给底层 C ABI）。[`Request::Json`] 原样返回。
    ///
    /// # Errors
    ///
    /// 当内部对象无法序列化为合法 JSON 时返回 [`Error::Json`]。
    pub fn to_json(&self) -> Result<String, Error> {
        let mut obj = serde_json::Map::new();
        match self {
            Request::Json(s) => return Ok(s.clone()),
            Request::Vad(r) | Request::Asr(r) | Request::Diar(r) | Request::SourceSeparation(r) => {
                if let Some(audio) = &r.audio {
                    write_audio(&mut obj, audio);
                }
                if !r.options.is_empty() {
                    obj.insert(
                        "options".into(),
                        Value::Object(r.options.clone().into_iter().collect()),
                    );
                }
            }
            Request::Tts(r) => {
                obj.insert("text".into(), Value::String(r.text.clone()));
                if let Some(lang) = &r.language {
                    obj.insert("language".into(), Value::String(lang.clone()));
                }
                if let Some(audio) = &r.reference_audio {
                    write_audio(&mut obj, audio);
                }
                let mut options = r.options.clone();
                if let Some(rt) = &r.reference_text {
                    options.insert("reference_text".into(), Value::String(rt.clone()));
                }
                if !options.is_empty() {
                    obj.insert(
                        "options".into(),
                        Value::Object(options.into_iter().collect()),
                    );
                }
                if let Some(voice) = &r.voice {
                    obj.insert("voice".into(), serialize_voice(voice));
                }
            }
        }
        Ok(serde_json::to_string(&Value::Object(obj))?)
    }
}

/// 把音频输入写入请求根对象（`audio_path` 或 `audio` 对象）。
fn write_audio(obj: &mut serde_json::Map<String, Value>, input: &AudioInput) {
    match input {
        AudioInput::Path(p) => {
            obj.insert("audio_path".into(), Value::String(p.clone()));
        }
        AudioInput::Buffer(buf) => {
            obj.insert(
                "audio".into(),
                serde_json::json!({
                    "sample_rate": buf.sample_rate,
                    "channels": buf.channels,
                    "samples": buf.samples,
                }),
            );
        }
    }
}

/// 把 [`VoiceCondition`] 序列化为上游请求的 `voice` 对象。
fn serialize_voice(voice: &VoiceCondition) -> Value {
    let mut speaker = serde_json::Map::new();
    if let Some(id) = &voice.cached_voice_id {
        speaker.insert("cached_voice_id".into(), Value::String(id.clone()));
    }
    if let Some(audio) = &voice.speaker_audio {
        match audio {
            AudioInput::Path(p) => {
                speaker.insert("audio_path".into(), Value::String(p.clone()));
            }
            AudioInput::Buffer(buf) => {
                speaker.insert(
                    "audio".into(),
                    serde_json::json!({
                        "sample_rate": buf.sample_rate,
                        "channels": buf.channels,
                        "samples": buf.samples,
                    }),
                );
            }
        }
    }
    let mut style = serde_json::Map::new();
    if let Some(v) = &voice.language {
        style.insert("language".into(), Value::String(v.clone()));
    }
    if let Some(v) = &voice.emotion {
        style.insert("emotion".into(), Value::String(v.clone()));
    }
    if let Some(v) = &voice.speaking_rate {
        style.insert("speaking_rate".into(), serde_json::json!(v));
    }
    if let Some(v) = &voice.pitch_shift {
        style.insert("pitch_shift".into(), serde_json::json!(v));
    }
    if let Some(v) = &voice.energy_scale {
        style.insert("energy_scale".into(), serde_json::json!(v));
    }
    if !voice.tags.is_empty() {
        let mut tags = serde_json::Map::new();
        for (k, v) in &voice.tags {
            tags.insert(k.clone(), Value::String(v.clone()));
        }
        style.insert("tags".into(), Value::Object(tags));
    }

    let mut voice_obj = serde_json::Map::new();
    voice_obj.insert("speaker".into(), Value::Object(speaker));
    voice_obj.insert("style".into(), Value::Object(style));
    Value::Object(voice_obj)
}

/// 可转换为一次任务请求的参数。
///
/// 已为以下类型实现：
/// - [`Request`] / `&Request`：类型化请求；
/// - `&str` / `String`：任意 JSON 字符串（透传给 C 边界）；
/// - `()`：空请求（等价于 `{}`）。
pub trait IntoRequest {
    /// 转换为请求对象。
    ///
    /// # Errors
    ///
    /// 永远成功（各实现均为 infallible），保留 `Result` 以便后续扩展。
    fn into_request(self) -> Result<Request, Error>;
}

impl IntoRequest for Request {
    fn into_request(self) -> Result<Request, Error> {
        Ok(self)
    }
}

impl IntoRequest for &Request {
    fn into_request(self) -> Result<Request, Error> {
        Ok(self.clone())
    }
}

impl IntoRequest for () {
    fn into_request(self) -> Result<Request, Error> {
        Ok(Request::Json("{}".into()))
    }
}

impl IntoRequest for &str {
    fn into_request(self) -> Result<Request, Error> {
        Ok(Request::Json(self.to_string()))
    }
}

impl IntoRequest for String {
    fn into_request(self) -> Result<Request, Error> {
        Ok(Request::Json(self))
    }
}

#[cfg(test)]
mod tests {
    // 测试断言中的 unwrap/expect 是惯用法：失败即测试失败，展开错误链无意义。
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn json(s: &str) -> serde_json::Value {
        serde_json::from_str(s).expect("合法 JSON")
    }

    #[test]
    fn stream_start_options_only() {
        let req = Request::stream()
            .option("language", "auto")
            .option("audio_chunk_seconds", 3.0);
        assert_eq!(
            json(&req.to_json().unwrap()),
            json(r#"{"options":{"audio_chunk_seconds":3.0,"language":"auto"}}"#)
        );
    }

    #[test]
    fn stream_start_empty() {
        // 无音频、无选项的流式 start 请求等价于空对象。
        let req = Request::stream();
        assert_eq!(json(&req.to_json().unwrap()), json(r#"{}"#));
    }

    #[test]
    fn vad_offline() {
        let req = Request::vad("./a.wav").option("vad_threshold", 0.5);
        assert_eq!(
            json(&req.to_json().unwrap()),
            json(r#"{"audio_path":"./a.wav","options":{"vad_threshold":0.5}}"#)
        );
    }

    #[test]
    fn asr_streaming_window() {
        let req = Request::asr("./a.wav").option("audio_chunk_seconds", 3.0);
        assert_eq!(
            json(&req.to_json().unwrap()),
            json(r#"{"audio_path":"./a.wav","options":{"audio_chunk_seconds":3.0}}"#)
        );
    }

    #[test]
    fn tts_text() {
        let req = Request::tts("Hello!");
        assert_eq!(json(&req.to_json().unwrap()), json(r#"{"text":"Hello!"}"#));
    }

    #[test]
    fn tts_voice_clone() {
        let req = Request::tts("Hi")
            .reference("./ref.wav")
            .reference_text("参考文本")
            .language("zh");
        assert_eq!(
            json(&req.to_json().unwrap()),
            json(
                r#"{"text":"Hi","language":"zh","audio_path":"./ref.wav","options":{"reference_text":"参考文本"}}"#
            )
        );
    }

    #[test]
    fn windows_path_no_manual_escaping() {
        // 反斜杠与引号由序列化自动转义。
        let req = Request::asr("C:\\dir\\spe\"ch.wav");
        assert_eq!(
            json(&req.to_json().unwrap()),
            json(r#"{"audio_path":"C:\\dir\\spe\"ch.wav"}"#)
        );
    }

    #[test]
    fn diar_offline() {
        let req = Request::diar("./meeting.wav").option("num_speakers", 4);
        assert_eq!(
            json(&req.to_json().unwrap()),
            json(r#"{"audio_path":"./meeting.wav","options":{"num_speakers":4}}"#)
        );
    }

    #[test]
    fn source_separation_offline() {
        let req = Request::source_separation("./song.wav").options([("stem", "vocals")]);
        assert_eq!(
            json(&req.to_json().unwrap()),
            json(r#"{"audio_path":"./song.wav","options":{"stem":"vocals"}}"#)
        );
    }

    #[test]
    fn tts_voice_condition() {
        // 说话人参考 + 风格控制映射到上游 `voice` 字段。
        let req = Request::tts("Hi").voice(
            VoiceCondition::speaker("./ref.wav")
                .emotion("happy")
                .speaking_rate(1.1)
                .tag("gender", "female"),
        );
        assert_eq!(
            json(&req.to_json().unwrap()),
            json(
                r#"{"text":"Hi","voice":{"speaker":{"audio_path":"./ref.wav"},"style":{"emotion":"happy","speaking_rate":1.100000023841858,"tags":{"gender":"female"}}}}"#
            )
        );
    }

    #[test]
    fn tts_voice_cached_id() {
        // 复用已缓存音色：仅传 cached_voice_id。
        let req = Request::tts("Hi").voice(VoiceCondition::cached("spk_abc"));
        assert_eq!(
            json(&req.to_json().unwrap()),
            json(r#"{"text":"Hi","voice":{"speaker":{"cached_voice_id":"spk_abc"},"style":{}}}"#)
        );
    }

    #[test]
    fn embedded_audio_buffer() {
        let buf = WavAudio {
            sample_rate: 16000,
            channels: 1,
            samples: vec![0.0, 0.5, -0.5],
        };
        let req = Request::vad(AudioInput::Buffer(buf));
        assert_eq!(
            json(&req.to_json().unwrap()),
            json(r#"{"audio":{"sample_rate":16000,"channels":1,"samples":[0.0,0.5,-0.5]}}"#)
        );
    }

    #[test]
    fn raw_json_pass_through() {
        let s = r#"{"text":"hi","options":{"a":1}}"#;
        assert_eq!(Request::json(s).to_json().unwrap(), s);
        assert_eq!(
            <&str as IntoRequest>::into_request(s)
                .unwrap()
                .to_json()
                .unwrap(),
            s
        );
    }

    #[test]
    fn empty_request() {
        assert_eq!(
            IntoRequest::into_request(()).unwrap().to_json().unwrap(),
            "{}"
        );
    }
}
