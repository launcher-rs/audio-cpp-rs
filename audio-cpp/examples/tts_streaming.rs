// 示例代码中的 unwrap/expect 是惯用法：失败即程序结束，无需展开错误链。
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! # tts_streaming（高层 API）— 用 VoxCPM2 做流式语音合成
//!
//! 演示流式 TTS：`start()` 一次性启动合成，期间每个音频块经事件回调
//! （`named_audio_outputs`，id 为 `chunk_0 / chunk_1 / …`）送出，`finish()`
//! 返回合并的完整音频。VoxCPM2 的流式输入取自 `start` 请求里的 `text`
//! （`streaming_policy().input` 为 `none`），**不需要**再调用 `process_audio`。
//!
//! 运行前需要：
//! 1. 用按需编译的 custom-models 构建（VoxCPM2 无专用 feature）：
//!    ```bash
//!    $env:AUDIOCPP_MODELS="voxcpm2"; cargo build --features custom-models
//!    ```
//! 2. 下载 VoxCPM2 Q8_0 GGUF：
//!    `https://huggingface.co/audio-cpp/audio.cpp-gguf` → `VoxCPM2-GGUF/voxcpm2-q8_0.gguf`
//!
//! 运行方式（模型需自行下载放 F:\models，如 `F:\models\voxcpm2-q8_0.gguf`）：
//! ```bash
//! cargo run -p audio-cpp --features custom-models --example tts_streaming -- `
//!     F:\models\voxcpm2-q8_0.gguf out.wav "你好，我是流式语音合成。"
//! ```
//!
//! 说明：
//! - VoxCPM2 支持中英文合成；流式合成在 `start` 时整段运行（CPU 上耗时较久），
//!   事件经回调逐块回传，合成完毕后再 `finish()` 聚合。
//! - 输出各块也写入独立 WAV（`<out>_chunkN.wav`），便于观察逐块产出。

use std::io::Write;
use std::sync::{Arc, Mutex};

use audio_cpp::{Backend, ModelFamily, Registry, Request, RunMode, StreamEvent, TaskKind};

/// 把交错 f32 采样写入 16-bit PCM WAV 文件。
fn write_wav_pcm16(
    path: &str,
    samples: &[f32],
    sample_rate: i32,
    channels: u16,
) -> std::io::Result<()> {
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
        eprintln!("用法: tts_streaming <voxcpm2-q8_0.gguf> <out.wav> <text>");
        std::process::exit(1);
    }
    let model_path = &args[1];
    let out_path = &args[2];
    let text = &args[3];

    // 1. 加载模型。VoxCPM2 的 GGUF 无法自动探测族别，须显式指定。
    let registry = Registry::new()?;
    println!("模型族: {:?}", registry.families()?);
    let model = registry.load(model_path, Some(ModelFamily::Voxcpm2), None)?;
    println!("模型加载成功: {model_path}");
    println!("元数据: {:?}", model.metadata()?);

    // 2. 创建流式 TTS 会话。
    let mut session = model.create_task_session(
        TaskKind::Tts,
        RunMode::Streaming,
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
    let policy = session.streaming_policy()?;
    println!("流式策略: {:?}（输入 none → 无需 process_audio）", policy);

    // 3. 注册事件回调：收集每个合成音频块（named_audio_outputs，id 为 chunk_N）。
    let chunks = Arc::new(Mutex::new(Vec::<StreamEvent>::new()));
    let collector = Arc::clone(&chunks);
    session.set_event_callback(Some(move |ev: StreamEvent| {
        if !ev.named_audio_outputs.is_empty() {
            collector.lock().unwrap().push(ev);
        }
    }));

    // 4. 启动流式合成。文本经 text 字段送入；合成在 start 内整段跑完。
    //    流式要求 retry_badcase=false（上游限制），其余参数可选。
    session.start(Request::tts(text).option("retry_badcase", false))?;
    println!("请求文本: {text}");

    // 5. 聚合最终结果。
    let result = session.finish()?;
    session.reset();

    // 6. 写入完整音频（audio_output 为合并结果）与各音频块。
    if let Some(audio) = &result.audio_output {
        let channels = audio.channels.max(1) as u16;
        let samples = audio.samples.as_deref().unwrap_or(&[]);
        write_wav_pcm16(out_path, samples, audio.sample_rate, channels)
            .map_err(|e| audio_cpp::Error::Ffi(format!("写 {out_path} 失败: {e}")))?;
        println!(
            "已写入 {out_path}: {}Hz {}ch {}采样（{} 秒）",
            audio.sample_rate,
            channels,
            samples.len(),
            samples.len() as f64 / audio.sample_rate.max(1) as f64
        );
    } else {
        println!("(无合并音频输出；报告信息: {result:?})");
    }

    let chunks = chunks.lock().unwrap();
    println!("=== 逐块事件 ({} 个) ===", chunks.len());
    for (i, ev) in chunks.iter().enumerate() {
        for named in &ev.named_audio_outputs {
            let samples = named.audio.samples.as_deref().unwrap_or(&[]);
            let chunk_path = format!("{out_path}_chunk{i}.wav");
            let channels = named.audio.channels.max(1) as u16;
            write_wav_pcm16(&chunk_path, samples, named.audio.sample_rate, channels)
                .map_err(|e| audio_cpp::Error::Ffi(format!("写 {chunk_path} 失败: {e}")))?;
            println!("  {} → {} ({} 采样)", named.id, chunk_path, samples.len());
        }
    }
    Ok(())
}
