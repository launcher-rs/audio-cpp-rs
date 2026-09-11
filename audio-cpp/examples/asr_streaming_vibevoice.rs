// 示例代码中的 unwrap/expect 是惯用法：失败即程序结束，无需展开错误链.
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! # asr_streaming_vibevoice（高层 API）— 用 VibeVoice ASR Streaming 7B 做流式语音识别
//!
//! 演示流式 ASR 的便捷封装 [`StreamingSession`]：`start()` → 按策略分块
//! `push_audio()`（返回 `Vec<StreamEvent>`，窗口解满时的 `partial_text`
//! 为增量转录片段）→ `finish()`（完整最终文本）。VibeVoice ASR Streaming
//! 同时支持离线（`RunMode::Offline`）与流式（`RunMode::Streaming`）两种模式。
//!
//! 运行前需要：
//! 1. 用按需编译的 feature 构建（`vibevoice_asr_streaming` 与 `vibevoice_asr`
//!    同属 `vibevoice_asr` CMake target，用既有 feature 即可）：
//!    ```bash
//!    cargo build -p audio-cpp --features model-vibevoice-asr
//!    ```
//! 2. 下载 VibeVoice ASR Streaming 7B Q8_0 GGUF：
//!    `https://huggingface.co/audio-cpp/VibeVoice-ASR-Streaming-7B-GGUF` →
//!    `vibevoice-asr-streaming-7b-q8_0.gguf`
//!
//! 运行方式：
//! ```bash
//! cargo run -p audio-cpp --features model-vibevoice-asr --example asr_streaming_vibevoice -- `
//!     ./vibevoice-asr-streaming-7b-q8_0.gguf audio-cpp-sys/audio.cpp/assets/resources/sample_16k.wav
//! ```
//!
//! 说明：
//! - 输入任意采样率的 WAV（引擎内部重采样为模型处理采样率）；该流式族支持
//!   en/zh/es/pt/de/ja/ko/fr/ru/it 十种语言标签。
//! - 流式偏好 0.25s 音频块（`streaming_policy` 给出，远小于窗口的会攒批）：
//!   内部攒满一个解码窗口才触发一次 `partial_text` 事件；短音频可能没有任何
//!   逐块事件，最终文本在 `finish()` 的结果里，属正常。
//! - 该流式族不输出时间戳（无 segments/words），但可输出说话人归属文本
//!   （`speaker_turns`，见离线模式 `--turns-out`）。

use audio_cpp::{Backend, Registry, Request, StreamingSession, TaskKind, load_wav};

fn main() -> Result<(), audio_cpp::Error> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!(
            "用法: asr_streaming_vibevoice <vibevoice-asr-streaming-7b-q8_0.gguf> <input.wav>"
        );
        std::process::exit(1);
    }
    let model_path = &args[1];
    let wav_path = &args[2];

    // 1. 加载模型与音频。族别按文件名自动推断（from_path 识别
    //    vibevoice-asr-streaming 前缀，不会误判为 vibevoice_asr），无需手写。
    let registry = Registry::new()?;
    println!("模型族: {:?}", registry.families()?);
    let model = registry.load(
        model_path,
        audio_cpp::ModelFamily::from_path(model_path),
        None,
    )?;
    let wav = load_wav(wav_path)?;
    println!(
        "音频: {}Hz {}ch {}采样",
        wav.sample_rate,
        wav.channels,
        wav.samples.len()
    );

    // 2. 创建流式会话并包装为 StreamingSession（自动收集事件回调）。
    let stream = StreamingSession::from_model(
        &model,
        TaskKind::Asr,
        Backend::Cpu,
        0, // device
        4, // threads
        None,
    )?;
    println!(
        "会话: family={} task={} mode={}",
        stream.session().family(),
        stream.session().task_kind(),
        stream.session().run_mode()
    );
    let policy = stream.session().streaming_policy()?;
    println!("流式策略: {:?}", policy);

    // 3. 开流：请求携带 audio_path 建立音频契约（VibeVoice ASR 的 streaming
    //    prepare 需要契约，与 Qwen3 ASR 同模式），随后再逐块 process_audio
    //    送入同样的音频。
    stream.start(Request::asr(wav_path).option("language", "en"))?;

    // 4. 分块送入音频，每块取回该块触发的全部事件（partial_text / is_final）。
    //    samples 为 0 时按 seconds × 采样率换算（本族偏好 0.25s）。
    let chunk = if policy.preferred_audio_chunk_samples > 0 {
        policy.preferred_audio_chunk_samples
    } else if policy.preferred_audio_chunk_seconds > 0.0 {
        (policy.preferred_audio_chunk_seconds * wav.sample_rate as f64).round() as usize
    } else {
        512
    };
    let mut partial = Vec::<String>::new();
    let mut pos = 0usize;
    let mut start_sample = 0i64;
    while pos < wav.samples.len() {
        let end = (pos + chunk).min(wav.samples.len());
        let events = stream.push_audio(
            &wav.samples[pos..end],
            wav.sample_rate,
            wav.channels,
            start_sample,
        )?;
        for ev in &events {
            if let Some(t) = &ev.partial_text {
                partial.push(t.text.clone());
            }
        }
        let block_len = (end - pos) as i64;
        pos = end;
        start_sample += block_len;
    }

    // 5. 结束流式会话，取回最终完整结果。
    let result = stream.finish()?;
    stream.reset()?;

    println!("=== 部分转录 ===");
    for t in &partial {
        println!("  {t}");
    }
    println!("=== 最终转录 ===");
    match &result.text_output {
        Some(t) => println!("  {}（语言: {}）", t.text, t.language),
        None => println!("  (无文本输出；报告信息: {result:?})"),
    }
    Ok(())
}
