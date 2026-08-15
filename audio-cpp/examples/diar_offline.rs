//! # diar_offline（高层 API）— 用 SortFormer 做说话人分离
//!
//! 演示 `model-sortformer-diar` feature 构建的说话人分离模型族。运行前需要：
//! 1. 用按需编译的 feature 构建 audio-cpp-sys（SortFormer 不在默认 core-models 集）：
//!    ```bash
//!    cargo build -p audio-cpp --features model-sortformer-diar
//!    ```
//! 2. 下载 SortFormer Diar 4spk Q8_0 GGUF（约 176MB）：
//!    `https://huggingface.co/audio-cpp/audio.cpp-gguf` → `Sortformer-Diar-4spk-v1-GGUF/sortformer-diar-4spk-v1-q8_0.gguf`
//!
//! 运行方式：
//! ```bash
//! cargo run -p audio-cpp --features model-sortformer-diar --example diar_offline -- \
//!     ./sortformer-diar-4spk-v1-q8_0.gguf ./two_speakers.wav
//! ```
//! 音频要求 16kHz 单声道 WAV（SortFormer 的 processor 采样率，模型不隐式重采样）。
//! 测试音频可用上游自带的 `audio-cpp-sys/audio.cpp/assets/resources/four_speaker_short.wav`
//! （24kHz mono 四说话人），需先重采样到 16kHz。输出中的 confidence 恒为 0
//! （上游 postprocess 未填充，SortFormer 本身不输出逐段置信度）。

use audio_cpp::{Backend, ModelFamily, Registry, Request, RunMode, TaskKind};

fn main() -> Result<(), audio_cpp::Error> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("用法: diar_offline <sortformer-diar-4spk-v1-q8_0.gguf> <input.wav>");
        std::process::exit(1);
    }
    let model_path = &args[1];
    let wav_path = &args[2];

    // 1. 创建默认注册表，确认说话人分离模型族已编译进引擎。
    let registry = Registry::new()?;
    let families = registry.families()?;
    println!("模型族: {families:?}");
    if !families
        .iter()
        .any(|f| f == ModelFamily::SortformerDiar.as_str())
    {
        eprintln!(
            "警告: sortformer_diar 未编译进引擎。请用 `--features model-sortformer-diar` 重新构建。"
        );
    }

    // 2. 加载 SortFormer Diar 模型（GGUF 无法自动探测，须显式指定家族）。
    let model = registry.load(model_path, Some(ModelFamily::SortformerDiar), None)?;
    println!("模型加载成功: {model_path}");
    println!("元数据: {:?}", model.metadata()?);

    // 3. 创建离线说话人分离会话（SortFormer 仅支持 offline）。
    let session = model.create_task_session(
        TaskKind::Diar,
        RunMode::Offline,
        Backend::Cpu,
        0, // device
        4, // threads
        None,
    )?;
    println!(
        "会话: family={} task={} mode={}",
        session.family(),
        session.task_kind(),
        session.run_mode()
    );

    // 4. 离线执行：请求里用 audio_path 指向 WAV 文件（须 16kHz mono）。
    let result = session.run_offline(Request::diar(wav_path))?;

    // 5. 打印每个说话人的发言时间段。
    println!("=== 说话人分离结果 ===");
    if result.speaker_turns.is_empty() {
        println!("(无 speaker_turns 输出；报告信息: {result:?})");
    }
    for turn in &result.speaker_turns {
        let start_sec = turn.span.start_sample as f64 / 16000.0;
        let end_sec = turn.span.end_sample as f64 / 16000.0;
        println!(
            "{}  {:6.2}s..{:6.2}s  采样 {}..{}  置信度={:.3}",
            turn.speaker_id,
            start_sec,
            end_sec,
            turn.span.start_sample,
            turn.span.end_sample,
            turn.confidence
        );
    }
    Ok(())
}
