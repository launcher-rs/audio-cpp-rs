//! # tts_offline（高层 API）— 用 MOSS-TTS-Nano 合成一段语音
//!
//! 演示 `custom-models` 构建的 TTS 模型族。运行前需要：
//! 1. 用 `custom-models` 构建：（MOSS-TTS-Nano 由 `moss` target 提供）
//!    ```bash
//!    $env:AUDIOCPP_MODELS="moss_tts_nano"; cargo build --features custom-models
//!    ```
//! 2. 下载 MOSS-TTS-Nano-100M Q8_0 GGUF（约 184MB）：`audio-cpp/audio.cpp-gguf`
//!    仓库的 `MOSS-TTS-Nano-100M-GGUF/moss-tts-nano-100m-q8_0.gguf`。
//!
//! 运行方式：
//! ```bash
//! cargo run -p audio-cpp --features custom-models --example tts_offline -- \
//!     ./moss-tts-nano-100m-q8_0.gguf out.wav "Hello from Rust and audio.cpp!"
//! ```
//!
//! 默认带一句示例英文文本（语音越长合成越耗时）。

use std::io::Write;

use audio_cpp::{Backend, Registry, RunMode, TaskKind};

/// 把 (-1..1) 的 f32 采样写入 16-bit PCM WAV 文件。
///
/// `samples` 为交错存放（每帧 = `channels` 个采样），`sample_rate` 为采样率。
fn write_wav_pcm16(path: &str, samples: &[f32], sample_rate: i32, channels: u16) -> std::io::Result<()> {
    let bytes_per_sample = 2u32; // 16-bit
    let block_align = bytes_per_sample * channels as u32;
    let byte_rate = sample_rate as u32 * block_align;
    let data_len = samples.len() as u32 * bytes_per_sample;
    let mut f = std::fs::File::create(path)?;
    f.write_all(b"RIFF")?;
    f.write_all(&(36u32 + data_len).to_le_bytes())?;
    f.write_all(b"WAVE")?;
    f.write_all(b"fmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?; // PCM
    f.write_all(&channels.to_le_bytes())?;
    f.write_all(&(sample_rate as u32).to_le_bytes())?;
    f.write_all(&byte_rate.to_le_bytes())?;
    f.write_all(&(block_align as u16).to_le_bytes())?;
    f.write_all(&(bytes_per_sample as u16 * 8).to_le_bytes())?; // bits per sample
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
    if args.len() < 3 {
        eprintln!("用法: tts_offline <moss-tts-nano-100m-q8_0.gguf> <out.wav> [text]");
        std::process::exit(1);
    }
    let model_path = &args[1];
    let out_path = &args[2];
    let text = args
        .get(3)
        .cloned()
        .unwrap_or_else(|| "Hello from Rust and audio.cpp!".to_string());

    // 1. 创建默认注册表，确认 TTS 模型族已编译进引擎。
    let registry = Registry::new()?;
    let families = registry.families()?;
    println!("模型族: {families:?}");
    if !families.iter().any(|f| f == "moss_tts_nano") {
        eprintln!(
            "警告: moss_tts_nano 未编译进引擎。请用 `--features custom-models`，\
             并设置 AUDIOCPP_MODELS=moss_tts_nano 重新构建。"
        );
    }

    // 2. 加载 TTS 模型（GGUF 文件，须显式指定模型族）。
    let model = registry.load(model_path, Some("moss_tts_nano"), None)?;
    println!("模型加载成功: {model_path}");

    // 3. 创建离线 TTS 会话。
    let session = model.create_task_session(
        TaskKind::Tts,
        RunMode::Offline,
        Backend::Cpu,
        0,   // device
        4,   // threads
        None,
    )?;
    println!("会话: family={} task={} mode={}", session.family(), session.task_kind(), session.run_mode());

    // 4. 合成：请求带 text 输入（当前 C ABI 对 voice 参考支持有限，用默认音色）。
    let request = format!(r#"{{"text":"{}"}}"#, text.replace('"', "\\\""));
    println!("请求文本: {text}");
    let result = session.run_offline(&request)?;

    // 5. 取出合成的音频并写入 WAV 文件。
    let audio = result
        .audio_output
        .as_ref()
        .expect("TTS 应返回 audio_output");
    let samples = audio
        .samples
        .as_deref()
        .expect("audio_output 应携带 samples 数据");
    if samples.is_empty() {
        return Err(audio_cpp::Error::Ffi("合成音频为空".to_string()));
    }
    let channels = audio.channels.max(1) as u16;
    write_wav_pcm16(out_path, samples, audio.sample_rate, channels)
        .map_err(|e| audio_cpp::Error::Ffi(format!("写 WAV 失败: {e}")))?;
    println!(
        "已写入 {out_path}: {}Hz {}ch {} 采样（{} 秒）",
        audio.sample_rate,
        audio.channels,
        samples.len(),
        samples.len() as f64 / audio.sample_rate as f64
    );
    Ok(())
}