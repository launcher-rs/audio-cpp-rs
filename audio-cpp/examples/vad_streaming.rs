//! # vad_streaming（高层 API）— 用 silero_vad 对音频做流式语音活动检测
//!
//! 演示流式会话：`start()` → 分块 `process_audio()` → `finish()`，并通过
//! [`Session::set_event_callback`] 接收 C++ 侧回调的流事件。
//!
//! 运行方式：
//! ```bash
//! cargo run -p audio-cpp --example vad_streaming -- \
//!     audio-cpp-sys/audio.cpp/assets/framework/models/silero_vad/silero_vad_16k.safetensors \
//!     audio-cpp-sys/audio.cpp/assets/resources/sample_16k.wav
//! ```

use std::sync::{Arc, Mutex};

use audio_cpp::{load_wav, Backend, Registry, RunMode, TaskKind};

fn main() -> Result<(), audio_cpp::Error> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("用法: vad_streaming <silero_vad_16k.safetensors> <input.wav>");
        std::process::exit(1);
    }

    // 1. 加载权重并读取音频。
    let registry = Registry::new()?;
    let model = registry.load(&args[1], None, None)?;
    let wav = load_wav(&args[2])?;
    println!("音频: {}Hz {}ch {}采样", wav.sample_rate, wav.channels, wav.samples.len());

    // 2. 创建流式 VAD 会话。
    let session = model.create_task_session(
        TaskKind::Vad,
        RunMode::Streaming,
        Backend::Cpu,
        0,
        4,
        None,
    )?;
    println!("会话: family={} task={} mode={}", session.family(), session.task_kind(), session.run_mode());
    println!("流式策略: {:?}", session.streaming_policy()?);

    // 3. 注册事件回调：记录 speech_start / speech_end 事件。
    use audio_cpp::StreamEvent;
    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    let collector = Arc::clone(&events);
    let mut session = session;
    session.set_event_callback(Some(move |event: StreamEvent| {
        for act in &event.voice_activity {
            collector.lock().unwrap().push(format!(
                "{} @{} (p={:.3})",
                act.kind, act.sample, act.probability
            ));
        }
    }));

    // 4. 分块送入音频（按策略推荐的块大小）。silero_vad 流式要求每块恰好
    //    512 个采样，因此末尾不足一块时以静音补足（不影响 VAD 判定）。
    let policy = session.streaming_policy()?;
    let chunk = if policy.preferred_audio_chunk_samples > 0 {
        policy.preferred_audio_chunk_samples
    } else {
        512
    };

    session.start(None)?;
    let mut pos = 0usize;
    let mut start_sample = 0i64;
    while pos < wav.samples.len() {
        let end = (pos + chunk).min(wav.samples.len());
        let block = &wav.samples[pos..end];
        if block.len() == chunk {
            session.process_audio(block, wav.sample_rate, wav.channels, start_sample)?;
        } else {
            // 补零到完整块再送入。
            let mut padded = vec![0f32; chunk];
            padded[..block.len()].copy_from_slice(block);
            session.process_audio(&padded, wav.sample_rate, wav.channels, start_sample)?;
        }
        pos = end;
        start_sample += block.len() as i64;
    }

    // 5. 结束流式会话，取回最终结果。
    let result = session.finish()?;
    session.reset();

    let events = events.lock().unwrap();
    println!("=== 流式事件 ===");
    for e in events.iter() {
        println!("  {e}");
    }
    println!("=== 最终语音片段 ===");
    for seg in &result.speech_segments {
        println!(
            "  {}..{} 置信度={}",
            seg.span.start_sample, seg.span.end_sample, seg.confidence
        );
    }
    Ok(())
}
