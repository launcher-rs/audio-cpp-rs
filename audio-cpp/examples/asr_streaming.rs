// 示例代码中的 unwrap/expect 是惯用法：失败即程序结束，无需展开错误链。
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! # asr_streaming（高层 API）— 用 Qwen3 ASR 做流式语音识别
//!
//! 演示流式 ASR 的便捷封装 [`StreamingSession`]：`start()` → 分块
//! `push_audio()`（返回 `Vec<StreamEvent>`，含窗口边界的 `partial_text`）
//! → `finish()`（完整最终文本）。Qwen3 ASR 同时支持离线
//! （`RunMode::Offline`）与流式（`RunMode::Streaming`）两种模式。
//!
//! 运行前需要：
//! 1. 用按需编译的 feature 构建（Qwen3 ASR 不在默认 core-models 集）：
//!    ```bash
//!    cargo build -p audio-cpp --features model-qwen3-asr
//!    ```
//! 2. 下载 Qwen3 ASR Q8_0 GGUF：
//!    `https://huggingface.co/audio-cpp/audio.cpp-gguf` → `Qwen3-ASR-0.6B-GGUF/qwen3-asr-0.6b-q8_0.gguf`
//!
//! 运行方式（已用 `./qwen3-asr-q8_0.gguf` + 内置 `sample_16k.wav` 验证通过，
//! 输出逐窗口部分转录 + 最终文本）：
//! ```bash
//! cargo run -p audio-cpp --features model-qwen3-asr --example asr_streaming -- `
//!     ./qwen3-asr-q8_0.gguf audio-cpp-sys/audio.cpp/assets/resources/sample_16k.wav
//! ```
//!
//! 说明：
//! - 输入任意采样率的 WAV（引擎内部线性重采样为 16kHz）；建议英文或中文普通话。
//! - 流式按"窗口"处理：`start` 请求里可用 `options.audio_chunk_seconds` 指定
//!   窗口秒数（默认 30s），窗口排满才触发一次 `partial_text` 事件；音频比窗口
//!   短时不会有任何逐块事件，最终文本在 `finish()` 的结果里，属正常。

use audio_cpp::{Backend, Registry, Request, StreamingSession, TaskKind, load_wav};

fn main() -> Result<(), audio_cpp::Error> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("用法: asr_streaming <qwen3-asr-q8_0.gguf> <input.wav>");
        std::process::exit(1);
    }
    let model_path = &args[1];
    let wav_path = &args[2];

    // 1. 加载模型与音频。族别按文件名自动推断（from_path），无需手写。
    let registry = Registry::new()?;
    println!("模型族: {:?}", registry.families()?);
    let model = registry.load(model_path, audio_cpp::ModelFamily::from_path(model_path), None)?;
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

    // 3. 开流：请求携带 audio_path 建立音频契约（Qwen3 ASR 的 streaming
    //    prepare 需要契约），随后再逐块 process_audio 送入同样的音频。
    //    纯 options 场景（如仅传 language、无需契约的模型族）可用
    //    `Request::stream()` 构造。
    stream.start(Request::asr(wav_path).option("audio_chunk_seconds", 3.0))?;

    // 4. 分块送入音频，每块取回该块触发的全部事件（partial_text / is_final）。
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
    stream.reset();

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
