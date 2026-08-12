//! # audio-cpp
//!
//! audio.cpp（基于 ggml 的本地音频推理引擎）的高层安全 Rust 封装。
//!
//! 底层 FFI 位于 [`audio_cpp_sys`]；本 crate 在其之上提供类型安全的
//! 注册表 / 模型 / 会话 API，并将所有跨 C 边界的资源管理（句柄释放、
//! 字符串所有权、事件回调）封装在安全的 `Drop` 与类型系统中。
//!
//! ## 快速上手
//!
//! 枚举引擎能力：
//! ```no_run
//! use audio_cpp::Registry;
//! let registry = Registry::new()?;
//! println!("模型族: {:?}", registry.families()?);
//! println!("设备: {:?}", Registry::devices()?);
//! # Ok::<(), audio_cpp::Error>(())
//! ```
//!
//! 加载模型并做一次离线 VAD（需 silero 权重与 wav 文件）：
//! ```no_run
//! use audio_cpp::{Backend, Registry, RunMode, TaskKind};
//!
//! let registry = Registry::new()?;
//! let model = registry.load("./silero_vad_16k.safetensors", None, None)?;
//! let session = model.create_task_session(
//!     TaskKind::Vad,
//!     RunMode::Offline,
//!     Backend::Cpu,
//!     0,
//!     4,
//!     None,
//! )?;
//! let request = r#"{"audio_path":"./sample.wav","options":{"vad_threshold":0.5}}"#;
//! let result = session.run_offline(request)?;
//! for seg in &result.speech_segments {
//!     println!("语音: {:?}..{:?} 置信度={}", seg.span.start_sample, seg.span.end_sample, seg.confidence);
//! }
//! # Ok::<(), audio_cpp::Error>(())
//! ```
//!
//! ## 资源生命周期
//!
//! - [`Registry`]、[`Model`]、[`Session`] 各自持有 C 句柄并在 `Drop` 中释放；
//! - `Model` 不管理 `Registry` 的生命周期：注册表应存活于所有派生模型的
//!   使用期之内；
//! - 流式会话的事件回调要求 `Send` 闭包，回调可来自 C++ 侧线程。

pub use audio_cpp_sys;

mod error;
pub use error::Error;

mod ffi;

mod types;
pub use types::{
    AudioBufferInfo, Backend, Capabilities, Device, LoaderInfo, ModelMetadata, NamedAudioOutput,
    RunMode, SpeakerTurn, SpeechSegment, StreamEvent, StreamingPolicy, SupportedTask, TaskKind,
    TaskResult, TextOutput, TimeSpan, VoiceActivityEvent,
};

mod registry;
pub use registry::Registry;

mod model;
pub use model::Model;

mod session;
pub use session::Session;

mod audio;
pub use audio::{load_wav, WavAudio};
