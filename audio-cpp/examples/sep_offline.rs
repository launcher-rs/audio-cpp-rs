//! # sep_offline（高层 API）— 用 HTDemucs 做音乐源分离
//!
//! 演示 `model-demucs` feature 构建的音频分离模型族。运行前需要：
//! 1. 用按需编译的 feature 构建 audio-cpp-sys（Demucs 不在默认 core-models 集）：
//!    ```bash
//!    cargo build -p audio-cpp --features model-demucs
//!    ```
//! 2. 下载 HTDemucs Q8_0 GGUF（约 59MB）：
//!    `https://huggingface.co/audio-cpp/audio.cpp-gguf` → `HTDemucs-GGUF/htdemucs-q8_0.gguf`
//!
//! 运行方式：
//! ```bash
//! cargo run -p audio-cpp --features model-demucs --example sep_offline -- \
//!     ./htdemucs-q8_0.gguf ./song.wav ./out_dir
//! ```
//! 音频要求 44100Hz 双声道（立体声）WAV（模型不隐式重采样）。输出四个
//! 独立音轨：`drums.wav` / `bass.wav` / `other.wav` / `vocals.wav`。

use std::io::Write;

use audio_cpp::{Backend, Registry, RunMode, TaskKind};

/// 把交错 f32 采样写入 16-bit PCM WAV 文件。
fn write_wav_pcm16(path: &str, samples: &[f32], sample_rate: i32, channels: u16) -> std::io::Result<()> {
    let bytes_per_sample = 2u32;
    let block_align = bytes_per_sample * channels as u32;
    let byte_rate = sample_rate as u32 * block_align;
    let data_len = samples.len() as u32 * bytes_per_sample;
    let mut f = std::fs::File::create(path)?;
    f.write_all(b"RIFF")?;
    f.write_all(&(36u32 + data_len).to_le_bytes())?;
    f.write_all(b"WAVE")?;
    f.write_all(b"fmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;
    f.write_all(&channels.to_le_bytes())?;
    f.write_all(&(sample_rate as u32).to_le_bytes())?;
    f.write_all(&byte_rate.to_le_bytes())?;
    f.write_all(&(block_align as u16).to_le_bytes())?;
    f.write_all(&(bytes_per_sample as u16 * 8).to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&data_len.to_le_bytes())?;
    for &s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        f.write_all(&v.to_le_bytes())?;
    }
    Ok(())
}

fn main() -> Result<(), audio_cpp::Error> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("用法: sep_offline <htdemucs-q8_0.gguf> <input.wav> <out_dir>");
        std::process::exit(1);
    }
    let model_path = &args[1];
    let wav_path = &args[2];
    let out_dir = &args[3];
    std::fs::create_dir_all(out_dir)
        .map_err(|e| audio_cpp::Error::Ffi(format!("创建输出目录失败: {e}")))?;

    // 1. 创建默认注册表，确认分离模型族已编译进引擎。
    let registry = Registry::new()?;
    let families = registry.families()?;
    println!("模型族: {families:?}");
    if !families.iter().any(|f| f == "htdemucs") {
        eprintln!(
            "警告: htdemucs 未编译进引擎。请用 `--features model-demucs` 重新构建。"
        );
    }

    // 2. 加载 HTDemucs 模型（GGUF 无法自动探测，须显式指定家族）。
    let model = registry.load(model_path, Some("htdemucs"), None)?;
    println!("模型加载成功: {model_path}");
    println!("元数据: {:?}", model.metadata()?);

    // 3. 创建离线源分离会话（Demucs 仅支持 offline + source_separation）。
    let session = model.create_task_session(
        TaskKind::SourceSeparation,
        RunMode::Offline,
        Backend::Cpu,
        0,   // device
        4,   // threads
        None,
    )?;
    println!("会话: family={} task={} mode={}", session.family(), session.task_kind(), session.run_mode());

    // 4. 离线执行：请求里用 audio_path 指向 WAV 文件（须 44100Hz stereo）。
    let request = format!(
        r#"{{"audio_path":"{}"}}"#,
        wav_path.replace('\\', "\\\\").replace('"', "\\\"")
    );
    println!("请求: {request}");
    let result = session.run_offline(&request)?;

    // 5. 把每个分离源写入独立 WAV。
    println!("=== 分离结果 ===");
    if result.named_audio_outputs.is_empty() {
        println!("(无命名音频输出；报告信息: {result:?})");
    }
    for named in &result.named_audio_outputs {
        let out_path = format!("{out_dir}\\{}.wav", named.id);
        let samples = named.audio.samples.as_deref().unwrap_or(&[]);
        let channels = named.audio.channels.max(1) as u16;
        write_wav_pcm16(&out_path, samples, named.audio.sample_rate, channels)
            .map_err(|e| audio_cpp::Error::Ffi(format!("写 {out_path} 失败: {e}")))?;
        println!(
            "  {} → {} ({}Hz {}ch {} 采样, {}s)",
            named.id,
            out_path,
            named.audio.sample_rate,
            named.audio.channels,
            samples.len(),
            samples.len() as f64 / named.audio.sample_rate.max(1) as f64,
        );
    }
    Ok(())
}
