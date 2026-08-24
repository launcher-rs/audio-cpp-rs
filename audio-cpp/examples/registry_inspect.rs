//! # registry_inspect（高层 API）— 枚举注册表：模型族 / loader / 设备
//!
//! 无需下载任何权重即可运行，用于查看当前构建（feature / AUDIOCPP_MODELS）
//! 实际编译进了哪些模型族，以及每个族支持的任务与运行模式——例如哪个族支持
//! 流式（`modes` 含 `streaming`）、支持哪些语言。
//!
//! 运行方式：
//! ```bash
//! cargo run -p audio-cpp --example registry_inspect
//! # 带权重路径时额外演示 inspect 预检：
//! cargo run -p audio-cpp --example registry_inspect ./qwen3-asr-q8_0.gguf
//! ```
//!
//! 也可以验证 [`ModelFamily`] 枚举与引擎返回字符串的往返一致性：
//! 本示例会把每个枚举已知族找出来，检查其 `as_str()` 是否被引擎注册。

use audio_cpp::{ModelFamily, Registry};

fn main() -> Result<(), audio_cpp::Error> {
    // 1. 枚举已编译的模型族。
    let registry = Registry::new()?;
    let families = registry.families()?;
    println!("=== 模型族 ({}) ===", families.len());
    for f in &families {
        println!("  {f}");
    }

    // 2. 枚举计算设备。
    println!("\n=== 计算设备 ===");
    for d in Registry::devices()? {
        println!("  [{}] #{} {}", d.backend, d.index, d.name);
    }

    // 3. 遍历 loader 声明：任务 + 运行模式 + 语言 + 端点。
    println!("\n=== Loader 明细 ===");
    let loaders = registry.loaders()?;
    for l in &loaders {
        let tasks = l
            .capabilities
            .supported_tasks
            .iter()
            .map(|t| format!("{}[{}]", t.task, t.modes.join("/")))
            .collect::<Vec<_>>()
            .join(", ");
        println!("{}", l.family);
        println!("  任务: {tasks}");
        if !l.capabilities.languages.is_empty() {
            println!("  语言: {}", l.capabilities.languages.join(", "));
        }
        if !l.api_endpoints.is_empty() {
            println!("  端点: {}", l.api_endpoints.join(", "));
        }
    }

    // 4. 用 ModelFamily 枚举替代裸字符串：枚举收录的名字应都能被引擎解析。
    println!("\n=== ModelFamily 往返校验 ===");
    for f in families.iter().map(std::string::String::as_str) {
        let model_family = ModelFamily::from(f);
        println!("  {} → {:?} → {}", f, model_family, model_family.as_str());
    }

    // 5. 演示过滤：找出支持流式 ASR 的族（对应 asr_streaming 示例）。
    println!("\n=== 支持流式 ASR 的族 ===");
    for l in &loaders {
        let streaming_asr = l
            .capabilities
            .supported_tasks
            .iter()
            .any(|t| t.task == "asr" && t.modes.iter().any(|m| m == "streaming"));
        if streaming_asr {
            println!(
                "  {}（用 ModelFamily::from(\"{}\") → {:?}）",
                l.family,
                l.family,
                ModelFamily::from(l.family.as_str())
            );
        }
    }

    // 6. 加载前预检：supports_family 判断某族是否已编译进引擎（无需权重）。
    println!("\n=== supports_family 预检 ===");
    for probe in ["silero_vad", "qwen3_asr", "definitely_not_a_family"] {
        println!(
            "  {probe:30} → {}",
            if registry.supports_family(probe) {
                "已编译"
            } else {
                "未编译"
            }
        );
    }

    // 7. 预检具体权重文件：inspect 不真正加载即可给出 metadata / 能力 / 选项 / 资产。
    //    仅当命令行传入路径时演示（无权重也能跑上面 1~6 步）。
    if let Some(path) = std::env::args().nth(1) {
        println!("\n=== inspect 预检：{path} ===");
        match registry.inspect(&path) {
            Ok(info) => {
                println!("  族: {}", info.metadata.family);
                println!("  变体: {}", info.metadata.variant);
                println!("  描述: {}", info.metadata.description);
                let tasks = info
                    .capabilities
                    .supported_tasks
                    .iter()
                    .map(|t| format!("{}[{}]", t.task, t.modes.join("/")))
                    .collect::<Vec<_>>()
                    .join(", ");
                println!("  能力: {tasks}");
                if !info.discovered_weights.is_empty() {
                    println!("  发现权重:");
                    for w in &info.discovered_weights {
                        println!("    - {}  {}", w.id, w.path);
                    }
                }
                if !info.cli.request_options.is_empty() {
                    println!(
                        "  请求选项: {}",
                        info.cli
                            .request_options
                            .iter()
                            .map(|o| o.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
            }
            Err(e) => println!("  inspect 失败: {e}（文件可能不存在或无法被任何 loader 识别）"),
        }
    } else {
        println!("\n提示: 传入权重路径可演示 inspect，例如");
        println!("  cargo run -p audio-cpp --example registry_inspect ./qwen3-asr-q8_0.gguf");
    }

    println!("\n提示: registry.families() 反映当前编译的模型集；切换 feature 或");
    println!("$env:AUDIOCPP_MODELS 并重新构建后，这里列出的族会随之变化。");
    Ok(())
}
