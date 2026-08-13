//! # asr_streaming（高层 API）— 用 Qwen3 ASR 做流式语音识别
//!
//! 演示流式 ASR：`start()` → 分块 `process_audio()` → `finish()`，并通过
//! [`Session::set_event_callback`] 接收窗口边界处的部分转录（`partial_text`），
//! `finish()` 返回完整最终文本。Qwen3 ASR 同时支持离线（`RunMode::Offline`）
//! 与流式（`RunMode::Streaming`）两种模式。
//!
//! 运行前需要：
//! 1. 用按需编译的 feature 构建（Qwen3 ASR 不在默认 core-models 集）：
//!    ```bash
//!    cargo build -p audio-cpp --features model-qwen3-asr
//!    ```
//! 2. 下载 Qwen3 ASR Q8_0 GGUF：
//!    `https://huggingface.co/audio-cpp/audio.cpp-gguf` → `Qwen3-ASR-GGUF/qwen3-asr-q8_0.gguf`
//!
//! 运行方式（本机已用 `F:\models\qwen3-asr-0.6b-q8_0.gguf` + 内置
//! `sample_16k.wav` 验证通过，输出逐窗口部分转录 + 最终文本）：
//! ```bash
//! cargo run -p audio-cpp --features model-qwen3-asr --example asr_streaming -- `
//!     F:\models\qwen3-asr-0.6b-q8_0.gguf audio-cpp-sys/audio.cpp/assets/resources/sample_16k.wav
//! ```
//!
//! 说明：
//! - 输入任意采样率的 WAV（引擎内部线性重采样为 16kHz）；建议英文或中文普通话。
//! - 流式按"窗口"处理：`start` 请求里可用 `options.audio_chunk_seconds` 指定
//!   窗口秒数（默认 30s），窗口排满才触发一次 `partial_text` 事件；音频比窗口
//!   短时不会有任何逐块事件，最终文本在 `finish()` 的结果里，属正常。

use std::sync::{Arc, Mutex};

use audio_cpp::{load_wav, Backend, ModelFamily, Registry, RunMode, StreamEvent, TaskKind};

fn main() -> Result<(), audio_cpp::Error> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("用法: asr_streaming <qwen3-asr-q8_0.gguf> <input.wav>");
        std::process::exit(1);
    }
    let model_path = &args[1];
    let wav_path = &args[2];

    // 1. 加载模型与音频。Qwen3 ASR 的 GGUF 无法自动探测族别，须显式指定。
    let registry = Registry::new()?;
    println!("模型族: {:?}", registry.families()?);
    let model = registry.load(model_path, Some(ModelFamily::Qwen3Asr), None)?;
    let wav = load_wav(wav_path)?;
    println!("音频: {}Hz {}ch {}采样", wav.sample_rate, wav.channels, wav.samples.len());

    // 2. 创建流式 ASR 会话。
    let mut session = model.create_task_session(
        TaskKind::Asr,
        RunMode::Streaming,
        Backend::Cpu,
        0,   // device
        4,   // threads
        None,
    )?;
    println!("会话: family={} task={} mode={}", session.family(), session.task_kind(), session.run_mode());
    let policy = session.streaming_policy()?;
    println!("流式策略: {:?}", policy);

    // 3. 注册事件回调：收集窗口边界的部分转录文本。
    let partial = Arc::new(Mutex::new(Vec::<String>::new()));
    let collector = Arc::clone(&partial);
    session.set_event_callback(Some(move |event: StreamEvent| {
        if let Some(t) = &event.partial_text {
            collector.lock().unwrap().push(t.text.clone());
        }
    }));

    // 4. 开流，按窗口秒数输出部分转录（窗口越短事件越频繁，单窗口计算量更小）。
    //    Qwen3 ASR 的 streaming 要求 prepare 时拿到音频契约：start 请求传
    //    audio_path（或 audio 对象）即可，示例里引擎用它建立契约、随后我们再
    //    逐块 process_audio 送入同样的音频。Windows 路径需转义反斜杠。
    let request = format!(
        r#"{{"audio_path":"{}","options":{{"audio_chunk_seconds":3.0}}}}"#,
        wav_path.replace('\\', "\\\\").replace('"', "\\\"")
    );
    println!("请求: {request}");
    session.start(Some(&request))?;

    // 5. 分块送入音频。策略给出推荐块大小（qwen3_asr 只给秒数不给采样数，
    //    此时按 sample_rate × 秒数换算）；引擎内部会缓冲合并，分块大小不敏感。
    let chunk = if policy.preferred_audio_chunk_samples > 0 {
        policy.preferred_audio_chunk_samples
    } else if policy.preferred_audio_chunk_seconds > 0.0 {
        (policy.preferred_audio_chunk_seconds * wav.sample_rate as f64).round() as usize
    } else {
        512
    };
    let mut pos = 0usize;
    let mut start_sample = 0i64;
    while pos < wav.samples.len() {
        let end = (pos + chunk).min(wav.samples.len());
        session.process_audio(&wav.samples[pos..end], wav.sample_rate, wav.channels, start_sample)?;
        let block_len = (end - pos) as i64;
        pos = end;
        start_sample += block_len;
    }

    // 6. 结束流式会话，取回最终完整结果。
    let result = session.finish()?;
    session.reset();

    let partial = partial.lock().unwrap();
    println!("=== 部分转录 ===");
    for t in partial.iter() {
        println!("  {t}");
    }
    println!("=== 最终转录 ===");
    match &result.text_output {
        Some(t) => println!("  {}（语言: {}）", t.text, t.language),
        None => println!("  (无文本输出；报告信息: {result:?})"),
    }
    Ok(())
}