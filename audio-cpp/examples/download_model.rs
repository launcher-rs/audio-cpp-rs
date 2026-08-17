//! # download_model — 模型不存在时用 hf-hub 从 Hugging Face 下载
//!
//! 引擎本身只认本地路径，不负责获取模型。本示例演示如何用 [hf-hub] 在模型
//! 不存在时自动下载到本地缓存，再交给 `Registry::load` 加载——解决"下载指引
//! 要用户手动去 Hugging Face 页面点"的痛点。
//!
//! `hf-hub` 仅是示例用的 dev-dependency，**不进入库的运行时依赖**；实际项目
//! 中可换成任何你偏好的方式（`huggingface-cli`、curl、自建镜像、对象存储等），
//! 只需保证最终 `Registry::load` 拿到一个本地路径即可。
//!
//! 模型下载地址约定：上游官方 GGUF 仓库
//! `https://huggingface.co/audio-cpp/audio.cpp-gguf`，各模型在仓库内按子目录
//! 存放（如 `Qwen3-ASR-0.6B-GGUF/qwen3-asr-0.6b-q8_0.gguf`）；示例的第二个参数
//! 就是这个仓库内路径。下载走 hf-hub 的标准缓存（自动命中已下载文件），
//! 缓存位置用 `HF_HOME` 环境变量控制（默认 `~/.cache/huggingface`）。
//!
//! 运行方式：
//! ```bash
//! # 仅下载：从官方仓库拉 Qwen3 ASR GGUF 到本地缓存，打印缓存路径
//! cargo run -p audio-cpp --example download_model -- \
//!     audio-cpp/audio.cpp-gguf Qwen3-ASR-0.6B-GGUF/qwen3-asr-0.6b-q8_0.gguf
//!
//! # 下载后接着跑 VAD（第 3 个参数给 family_hint，示例用 silero_vad 验证链路）：
//! cargo run -p audio-cpp --example download_model -- \
//!     audio-cpp/audio.cpp-gguf Qwen3-ASR-0.6B-GGUF/qwen3-asr-0.6b-q8_0.gguf \
//!     silero_vad
//! ```
//!
//! [hf-hub]: https://crates.io/crates/hf-hub

use std::path::{Path, PathBuf};

use hf_hub::HFClientSync;

/// 解析 `repo_id`（`owner/name` 形式）。
fn parse_repo(repo_id: &str) -> (&str, &str) {
    let (owner, name) = repo_id
        .split_once('/')
        .unwrap_or_else(|| panic!("repo_id 必须是 owner/name 形式，收到: {repo_id}"));
    (owner, name)
}

/// 下载模型文件到 hf-hub 缓存，返回本地路径。已缓存时直接复用，不重复下载。
fn download_or_get(repo_id: &str, path_in_repo: &str) -> Result<PathBuf, String> {
    // 若本地文件已存在，直接使用。
    let local = Path::new(path_in_repo);
    if local.is_file() {
        println!("本地已存在，直接使用: {}", local.display());
        return Ok(local.to_path_buf());
    }

    // 用 hf-hub 下载（blocking API）。走标准缓存：首次下载到
    // `HF_HOME`（默认 ~/.cache/huggingface），再次运行自动命中。
    println!("本地不存在 {path_in_repo}，从 {repo_id} 下载…");
    let client = HFClientSync::new().map_err(|e| format!("创建 HF 客户端失败: {e}"))?;
    let (owner, name) = parse_repo(repo_id);
    let repo = client.model(owner, name);
    let path = repo
        .download_file()
        .filename(path_in_repo.to_string())
        .send()
        .map_err(|e| format!("下载失败: {e}"))?;
    println!("已下载: {}", path.display());
    Ok(path)
}

fn main() -> Result<(), audio_cpp::Error> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("用法: download_model <repo_id owner/name> <仓库内模型路径> [family_hint]");
        eprintln!(
            "  例: download_model audio-cpp/audio.cpp-gguf Qwen3-ASR-0.6B-GGUF/qwen3-asr-0.6b-q8_0.gguf"
        );
        eprintln!(
            "  例: download_model audio-cpp/audio.cpp-gguf Qwen3-ASR-0.6B-GGUF/qwen3-asr-0.6b-q8_0.gguf qwen3_asr"
        );
        eprintln!(
            "缓存位置用 HF_HOME 环境变量控制；模型族需用对应 feature 编译（如 qwen3_asr → model-qwen3-asr）"
        );
        std::process::exit(1);
    }
    let repo_id = &args[1];
    let path_in_repo = &args[2];
    let family_hint = args
        .get(3)
        .filter(|s| !s.starts_with("--"))
        .map(String::as_str)
        .map(audio_cpp::ModelFamily::from);

    // 1. 下载（本地不存在时）→ 拿到本地路径。
    let model_path = download_or_get(repo_id, path_in_repo).map_err(audio_cpp::Error::Other)?;

    // 2. 加载模型。
    let registry = audio_cpp::Registry::new()?;
    let model = registry.load(&model_path.to_string_lossy(), family_hint.clone(), None)?;
    println!(
        "模型加载成功: {} family_hint={family_hint:?}",
        model_path.display()
    );

    // 3. 若给了 family_hint 且是 VAD 族，跑一次离线 VAD 验证链路。
    if let Some(family) = family_hint {
        let family_str = family.as_str();
        if matches!(family_str, "silero_vad" | "marblenet_vad") {
            let session = model.create_task_session(
                audio_cpp::TaskKind::Vad,
                audio_cpp::RunMode::Offline,
                audio_cpp::Backend::Cpu,
                0, // device
                4, // threads
                None,
            )?;
            // 用引擎自带的 sample_16k.wav 验证。
            let wav = "audio-cpp-sys/audio.cpp/assets/resources/sample_16k.wav";
            let threshold_key = if session.family() == "marblenet_vad" {
                "threshold"
            } else {
                "vad_threshold"
            };
            let result =
                session.run_offline(audio_cpp::Request::vad(wav).option(threshold_key, 0.5))?;
            println!(
                "VAD 验证（{wav}）：检测到 {} 段语音",
                result.speech_segments.len()
            );
        } else {
            println!("family_hint={family_str} 非 VAD 族，跳过 VAD 验证（仅验证下载+加载）");
        }
    }

    Ok(())
}
