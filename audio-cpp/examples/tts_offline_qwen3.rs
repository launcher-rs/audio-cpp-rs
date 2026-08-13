//! # tts_offline_qwen3（高层 API）— 用 Qwen3 TTS base 做声音克隆合成
//!
//! 演示 Qwen3 TTS（`qwen3_tts` 族）离线合成。Qwen3 TTS **base 变体需要
//! voice-clone 参考音频**：引擎用参考音频提取说话人特征，再按合成文本产出语音。
//! 参考音频经请求里的 `audio_path`（或 `audio` 对象）传入，参考音频的文本转写
//! 经 `options.reference_text` 提供——上游 `make_request` 会把读到的音频当作
//! voice-clone 参考（见 `qwen3_tts/session.cpp`），无需额外 C ABI。
//!
//! 运行前需要：
//! 1. 用按需编译的 feature 构建（Qwen3 TTS 不在默认 core-models 集）：
//!    ```bash
//!    cargo build -p audio-cpp --features model-qwen3-tts
//!    ```
//! 2. 下载 Qwen3 TTS base Q8_0 GGUF：
//!    `https://huggingface.co/audio-cpp/audio.cpp-gguf` → `Qwen3-TTS-GGUF/qwen3-tts-12hz-0.6b-base-q8_0.gguf`
//!    以及一段 3~5 秒的参考人声 WAV（如上游的 `sample_16k.wav`）。
//!
//! 运行方式：
//! ```bash
//! cargo run -p audio-cpp --features model-qwen3-tts --example tts_offline_qwen3 -- `
//!     F:\models\qwen3-tts-12hz-0.6b-base-q8_0.gguf `
//!     audio-cpp-sys/audio.cpp/assets/resources/sample_16k.wav `
//!     "Some call me nature. Others call me Mother Nature." `
//!     out.wav "Hello from Rust and Qwen3 TTS!"
//! ```
//!
//! 说明：
//! - 参考音频任意采样率均可（引擎内部重采样到模型 `input_sample_rate`）；
//! - `reference_text` 应与参考音频内容一致，越贴合，克隆音色越准；
//! - 输出音频采样率为模型 `output_sample_rate`，写入 16-bit PCM WAV。

use std::io::Write;

use audio_cpp::{Backend, ModelFamily, Registry, RunMode, TaskKind};

/// 把交错 f32 采样写入 16-bit PCM WAV 文件。
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
    if args.len() < 4 {
        eprintln!("用法: tts_offline_qwen3 <model.gguf> <reference.wav> <reference_text> <out.wav> [text]");
        std::process::exit(1);
    }
    let model_path = &args[1];
    let reference_path = &args[2];
    let reference_text = &args[3];
    let out_path = &args[4];
    let text = args
        .get(5)
        .cloned()
        .unwrap_or_else(|| "Hello from Rust and Qwen3 TTS!".to_string());

    // 1. 加载模型。Qwen3 TTS 的 GGUF 无法自动探测族别，须显式指定。
    let registry = Registry::new()?;
    println!("模型族: {:?}", registry.families()?);
    let model = registry.load(model_path, Some(ModelFamily::Qwen3Tts), None)?;
    println!("模型加载成功: {model_path}");
    println!("元数据: {:?}", model.metadata()?);

    // 2. 创建离线 TTS 会话（Qwen3 TTS 仅支持 offline）。
    let session = model.create_task_session(
        TaskKind::Tts,
        RunMode::Offline,
        Backend::Cpu,
        0,   // device
        4,   // threads
        None,
    )?;
    println!("会话: family={} task={} mode={}", session.family(), session.task_kind(), session.run_mode());

    // 3. 构造请求：text 为待合成文本；audio_path 指向参考人声；
    //    options.reference_text 为参考音频文本转写。
    let request = format!(
        r#"{{"text":"{}","audio_path":"{}","options":{{"reference_text":"{}"}}}}"#,
        text.replace('"', "\\\""),
        reference_path.replace('\\', "\\\\").replace('"', "\\\""),
        reference_text.replace('"', "\\\"")
    );
    println!("参考音频: {reference_path}");
    println!("参考文本: {reference_text}");
    println!("合成文本: {text}");
    let result = session.run_offline(&request)?;

    // 4. 取出合成的音频并写入 WAV 文件。
    let audio = result
        .audio_output
        .as_ref()
        .expect("Qwen3 TTS 应返回 audio_output");
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