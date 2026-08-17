//! # load_any_audio — 用 symphonia 解码非 WAV 音频并喂给引擎
//!
//! 引擎只接受 f32 采样（采样率 + 声道数 + 交错数组），内置 `load_wav` 仅支持
//! WAV。本示例演示如何用 [symphonia] 把任意常见格式（mp3 / flac / ogg / m4a /
//! wav …）解码为 f32 采样，构造 `WavAudio` 后经 `AudioInput::Buffer` 交给请求，
//! 从而绕过 WAV 限制——`symphonia` 仅是示例用的 dev-dependency，**不进入库的
//! 运行时依赖**；实际项目中可换成任何你偏好的 Rust 解码库（[rodio]、[hound]、
//! [claxon] 等），只需填充 `WavAudio` 的三个字段即可。
//!
//! 运行方式（示例用 silero_vad 验证整条链路；音频文件可以是任意格式）：
//! ```bash
//! cargo run -p audio-cpp --example load_any_audio -- \
//!     audio-cpp-sys/audio.cpp/assets/framework/models/silero_vad/silero_vad_16k.safetensors \
//!     <你的音频文件，如 song.flac> \
//!     [family_hint]
//! ```
//!
//! [symphonia]: https://crates.io/crates/symphonia
//! [rodio]: https://crates.io/crates/rodio
//! [hound]: https://crates.io/crates/hound
//! [claxon]: https://crates.io/crates/claxon

use std::fs::File;
use std::path::Path;

use audio_cpp::{AudioInput, Backend, ModelFamily, Registry, Request, RunMode, TaskKind, WavAudio};

/// 用 symphonia 把任意格式音频解码为交错 f32 采样。
fn decode_any_audio(path: &str) -> Result<WavAudio, String> {
    use symphonia::core::codecs::audio::AudioDecoderOptions;
    use symphonia::core::formats::probe::Hint;
    use symphonia::core::formats::{FormatOptions, TrackType};
    use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
    use symphonia::core::meta::MetadataOptions;

    let file = File::open(path).map_err(|e| format!("打开文件失败: {e}"))?;
    let mss = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());

    // 1. 探测容器格式并获取音频轨。
    let mut format = symphonia::default::get_probe()
        .probe(
            &Hint::new(),
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| format!("探测格式失败: {e}"))?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| "文件中没有音频轨".to_string())?;

    // 2. 创建解码器。
    let codec_params = track
        .codec_params
        .as_ref()
        .and_then(|c| c.audio())
        .ok_or_else(|| "缺少音频编码参数".to_string())?;
    let sample_rate = codec_params.sample_rate.unwrap_or(0) as i32;
    let channels = codec_params
        .channels
        .as_ref()
        .map(|c| c.count() as i32)
        .unwrap_or(0);
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(codec_params, &AudioDecoderOptions::default())
        .map_err(|e| format!("创建解码器失败: {e}"))?;

    // 3. 逐包解码，累积到 f32 交错缓冲。
    let mut samples: Vec<f32> = Vec::new();
    while let Some(packet) = format
        .next_packet()
        .map_err(|e| format!("读取数据包失败: {e}"))?
    {
        if packet.track_id != track_id {
            continue;
        }
        let decoded = decoder
            .decode(&packet)
            .map_err(|e| format!("解码失败: {e}"))?;
        let n = decoded.samples_interleaved();
        if n == 0 {
            continue;
        }
        let start = samples.len();
        samples.resize(start + n, 0.0);
        decoded.copy_to_slice_interleaved(&mut samples[start..]);
    }

    Ok(WavAudio {
        sample_rate,
        channels,
        samples,
    })
}

fn main() -> Result<(), audio_cpp::Error> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("用法: load_any_audio <权重.safetensors> <input 任意格式> [family_hint]");
        std::process::exit(1);
    }
    let model_path = &args[1];
    let audio_path = &args[2];
    let family_hint = args.get(3).map(String::as_str).map(ModelFamily::from);

    // 1. 用 symphonia 解码任意格式 → f32 采样（不经引擎的 WAV 限制）。
    let wav = decode_any_audio(audio_path).map_err(audio_cpp::Error::Other)?;
    println!(
        "解码成功: {} 采样率={}Hz 声道={} 采样数={}",
        Path::new(audio_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(audio_path),
        wav.sample_rate,
        wav.channels,
        wav.samples.len()
    );

    // 2. 创建注册表并加载模型。
    let registry = Registry::new()?;
    let model = registry.load(model_path, family_hint.clone(), None)?;
    println!("模型加载成功: {model_path} family_hint={family_hint:?}");

    // 3. 创建离线 VAD 会话。
    let session = model.create_task_session(
        TaskKind::Vad,
        RunMode::Offline,
        Backend::Cpu,
        0, // device
        4, // threads
        None,
    )?;

    // 4. 把解码出的采样经 AudioInput::Buffer 交给请求。
    //    若音频采样率非 16k，VAD 模型可能不适用（silero 固定 16k）；
    //    用其它模型族时可先自行重采样。
    let threshold_key = if session.family() == "marblenet_vad" {
        "threshold"
    } else {
        "vad_threshold"
    };
    let result =
        session.run_offline(Request::vad(AudioInput::Buffer(wav)).option(threshold_key, 0.5))?;

    // 5. 打印语音片段。
    println!("=== 语音片段 ===");
    for seg in &result.speech_segments {
        println!(
            "  {}..{} 置信度={}",
            seg.span.start_sample, seg.span.end_sample, seg.confidence
        );
    }
    Ok(())
}
