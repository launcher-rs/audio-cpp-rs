//! # asr_offline（高层 API）— 用 Citrinet ASR 转录一段英文语音
//!
//! 演示 `full-models` / `custom-models` 构建的 ASR 模型族。运行前需要：
//! 1. 用 `custom-models`（推荐）或 `full-models` 构建 audio-cpp-sys
//!    （Citrinet 不在默认的 core-models 集内）：
//!    ```bash
//!    # PowerShell
//!    $env:AUDIOCPP_MODELS="citrinet_asr"; cargo build --features custom-models
//!    ```
//! 2. 下载 Citrinet ASR Q8_0 GGUF（约 40MB）：
//!    `https://huggingface.co/audio-cpp/audio.cpp-gguf` → `Citrinet-ASR-GGUF/citrinet-asr-q8_0.gguf`
//!
//! 运行方式：
//! ```bash
//! cargo run -p audio-cpp --features custom-models --example asr_offline -- \
//!     ./citrinet-asr-q8_0.gguf ./speech.wav
//! ```
//! 音频建议 16k 单声道 WAV（英文）。

use audio_cpp::{Backend, Registry, RunMode, TaskKind};

fn main() -> Result<(), audio_cpp::Error> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("用法: asr_offline <citrinet-asr-q8_0.gguf> <input.wav>");
        std::process::exit(1);
    }
    let model_path = &args[1];
    let wav_path = &args[2];

    // 1. 创建默认注册表，确认 ASR 模型族已编译进引擎。
    let registry = Registry::new()?;
    let families = registry.families()?;
    println!("模型族: {families:?}");
    let has_asr = families.iter().any(|f| f == "citrinet_asr");
    if !has_asr {
        eprintln!(
            "警告: citrinet_asr 未编译进引擎。请用 `--features custom-models`，\
             并设置 AUDIOCPP_MODELS=citrinet_asr 重新构建。"
        );
    }

    // 2. 加载 Citrinet ASR 模型（GGUF 文件）。
    //    GGUF 内容无法被自动探测识别（会误判为 silero_vad），须显式指定家族。
    let model = registry.load(model_path, Some("citrinet_asr"), None)?;
    println!("模型加载成功: {model_path}");
    println!("元数据: {:?}", model.metadata()?);

    // 3. 创建离线 ASR 会话。
    let session = model.create_task_session(
        TaskKind::Asr,
        RunMode::Offline,
        Backend::Cpu,
        0,   // device
        4,   // threads
        None,
    )?;
    println!("会话: family={} task={} mode={}", session.family(), session.task_kind(), session.run_mode());

    // 4. 离线转录：请求里直接用 audio_path 指向 WAV 文件。
    let request = format!(
        r#"{{"audio_path":"{}"}}"#,
        wav_path.replace('\\', "\\\\").replace('"', "\\\"")
    );
    let result = session.run_offline(&request)?;

    // 5. 打印转录文本。
    match &result.text_output {
        Some(text) => println!("=== 转录结果 ===\n{}\n语言: {}", text.text, text.language),
        None => println!("=== 转录结果 ===\n(无文本输出；报告信息: {result:?})"),
    }
    for seg in &result.speech_segments {
        println!(
            "  片段 {}..{} 置信度={} 文本=\"{}\"",
            seg.span.start_sample, seg.span.end_sample, seg.confidence, seg.text
        );
    }
    Ok(())
}