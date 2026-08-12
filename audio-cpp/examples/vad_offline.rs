//! # vad_offline（高层 API）— 用 silero_vad 对音频做离线语音活动检测
//!
//! 与 `audio-cpp-sys` 的对应示例功能相同，但全部经由 `audio-cpp` 的
//! 安全高层 API，无需手动管理 C 句柄与字符串。
//!
//! 调用链：`Registry::new()` → `Registry::load()` → `Model::create_task_session()`
//! → `Session::run_offline()` → 解析 `TaskResult`。
//!
//! 运行方式：
//! ```bash
//! cargo run -p audio-cpp --example vad_offline -- \
//!     audio-cpp-sys/audio.cpp/assets/framework/models/silero_vad/silero_vad_16k.safetensors \
//!     audio-cpp-sys/audio.cpp/assets/resources/sample_16k.wav
//! ```

use audio_cpp::{Backend, Registry, RunMode, TaskKind};

fn main() -> Result<(), audio_cpp::Error> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("用法: vad_offline <silero_vad_16k.safetensors> <input.wav>");
        std::process::exit(1);
    }
    let model_path = &args[1];
    let wav_path = &args[2];

    // 1. 创建默认注册表，枚举模型族与设备（验证 FFI 链路）。
    let registry = Registry::new()?;
    println!("模型族: {:?}", registry.families()?);
    println!("设备: {:?}", Registry::devices()?);

    // 2. 加载模型（所有权由 Model 的 Drop 管理，无需手动释放）。
    let model = registry.load(model_path, None, None)?;
    println!("模型加载成功: {model_path}");

    // 3. 创建离线 VAD 会话。
    let session = model.create_task_session(
        TaskKind::Vad,
        RunMode::Offline,
        Backend::Cpu,
        0,   // device
        4,   // threads
        None,
    )?;
    println!("会话: family={} task={} mode={}", session.family(), session.task_kind(), session.run_mode());

    // 4. 构造请求并离线执行。
    let request = format!(
        r#"{{"audio_path":"{}","options":{{"vad_threshold":0.5}}}}"#,
        wav_path.replace('\\', "\\\\").replace('"', "\\\"")
    );
    let result = session.run_offline(&request)?;

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
