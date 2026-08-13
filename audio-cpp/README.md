# audio-cpp

> audio.cpp（基于 ggml 的本地音频推理引擎）的**高层安全 Rust 封装**。

底层 FFI 位于 `audio-cpp-sys`；本 crate 在其之上提供类型安全的
**注册表 / 模型 / 会话** API，并把所有跨 C 边界的资源管理（句柄释放、
字符串所有权、事件回调）封装进安全的 `Drop` 与类型系统。两者共享同一份
C++ `engine_runtime`，**不重写 audio.cpp**。

## 依赖与构建

```toml
[dependencies]
audio-cpp = { path = "../audio-cpp" }   # 或版本号
```

首次构建前需先补齐 git submodule（见工作区 README）：

```bash
git submodule update --init --recursive
cargo build   # 默认 core-models + CPU
```

模型组合与计算后端通过 features 选择，全部转发给 `audio-cpp-sys`（含义见
[该 crate 的 Cargo.toml](../audio-cpp-sys/Cargo.toml)）：

```powershell
cargo build                                   # core-models（内置 VAD，开箱即用）
cargo build --features model-qwen3-asr        # 按需编译单个模型族（无需环境变量）
cargo build --features model-citrinet-asr,model-moss,openmp   # 多族 + 后端
cargo build --features full-models,cuda       # 全量 + GPU 后端
```

> 模型族 feature 命名约定 `model-<上游 target 名>`（`-`/`_` 等价，如
> `model-qwen3-asr` 或 `model-qwen3_asr`）。已内置一组常用族（ASR/TTS/分离，
> 见 `[features]` 表）；未覆盖的族仍可 `$env:AUDIOCPP_MODELS="..."` 配合
> `--features custom-models` 使用，两者可混用（取并集）。

## 架构：一次调用的完整链路

```
Registry::new()                # 枚举已编译的模型族/loader/设备
 └─ registry.load(path, family_hint, options) → Model
     └─ model.create_task_session(task, mode, backend, device, threads, opts) → Session
         ├─ 离线:     session.run_offline(request_json) → TaskResult
         └─ 流式:     session.set_event_callback(cb)
                      session.start(json) → process_audio(&[f32], ...) → ... → finish() → TaskResult
                      session.reset()     # 复用会话开始新一轮
```

各对象持有 C 句柄并在 `Drop` 中释放；`Model` 不管理 `Registry` 的生命周期，
注册表应存活于所有派生模型的使用期之内。

## 基本用法

### 1. 枚举引擎能力

```rust
use audio_cpp::Registry;

let registry = Registry::new()?;
println!("模型族: {:?}", registry.families()?);      // ["silero_vad","marblenet_vad",...]
println!("设备:   {:?}", Registry::devices()?);      // [Device{ backend:"CPU", ... }]
for loader in registry.loaders()? {
    println!("{} 任务: {:?}", loader.family, loader.capabilities.supported_tasks);
}
```

### 2. 离线 VAD（silero_vad，内置权重开箱即用）

```rust
use audio_cpp::{Backend, Registry, RunMode, TaskKind};

let registry = Registry::new()?;
let model = registry.load("./silero_vad_16k.safetensors", None, None)?;

let session = model.create_task_session(
    TaskKind::Vad, RunMode::Offline, Backend::Cpu, 0, 4, None,
)?;
let request = r#"{"audio_path":"./sample.wav","options":{"vad_threshold":0.5}}"#;
let result = session.run_offline(request)?;

for seg in &result.speech_segments {
    println!("语音 {}..{} 置信度={}", seg.span.start_sample, seg.span.end_sample, seg.confidence);
}
```

### 3. 流式 VAD（分块送入 + 事件回调）

```rust
use std::sync::{Arc, Mutex};
use audio_cpp::{load_wav, Backend, Registry, RunMode, StreamEvent, TaskKind};

let registry = Registry::new()?;
let model = registry.load("./silero_vad_16k.safetensors", None, None)?;
let wav = load_wav("./sample.wav")?;

let mut session = model.create_task_session(
    TaskKind::Vad, RunMode::Streaming, Backend::Cpu, 0, 4, None,
)?;
let policy = session.streaming_policy()?;   // 推荐分块大小
let chunk = policy.preferred_audio_chunk_samples.max(1);

let events = Arc::new(Mutex::new(Vec::new()));
let collector = Arc::clone(&events);
session.set_event_callback(Some(move |ev: StreamEvent| {
    for act in &ev.voice_activity {
        collector.lock().unwrap().push(format!("{} @{}", act.kind, act.sample));
    }
}));

session.start(None)?;
for block in wav.samples.chunks(chunk) {
    // 流式会话要求每块恰好 chunk 个采样：末尾不足补零。
    let mut padded = vec![0f32; chunk];
    padded[..block.len()].copy_from_slice(block);
    session.process_audio(&padded, wav.sample_rate, wav.channels, 0)?;
}
let result = session.finish()?;   // 最终语音片段
session.reset();                  // 复用会话重新开始
```

> **注意**：silero_vad 流式要求每块**恰好** `preferred_audio_chunk_samples`
> （512）个采样，末尾不足块必须补零。回调可能来自 C++ 侧线程，回调内不得
> 再调用本会话的方法。

### 4. 离线 ASR（Citrinet，需按需编译）

```rust
use audio_cpp::{Backend, ModelFamily, Registry, RunMode, TaskKind};

let registry = Registry::new()?;
// GGUF 无法自动探测族别（会误判为 silero_vad），必须显式 family_hint。
let model = registry.load("./citrinet-asr-q8_0.gguf", Some(ModelFamily::CitrinetAsr), None)?;
let session = model.create_task_session(
    TaskKind::Asr, RunMode::Offline, Backend::Cpu, 0, 4, None,
)?;

let request = r#"{"audio_path":"./speech.wav"}"#;
let result = session.run_offline(request)?;
if let Some(text) = &result.text_output {
    println!("转录: {}", text.text);
}
```

### 5. 离线 TTS（MOSS-TTS-Nano，需 custom-models 构建）

```rust
use audio_cpp::{Backend, ModelFamily, Registry, RunMode, TaskKind};

let registry = Registry::new()?;
let model = registry.load("./moss-tts-nano-100m-q8_0.gguf", Some(ModelFamily::MossTtsNano), None)?;
let session = model.create_task_session(
    TaskKind::Tts, RunMode::Offline, Backend::Cpu, 0, 4, None,
)?;

let request = r#"{"text":"Hello from Rust and audio.cpp!"}"#;
let result = session.run_offline(request)?;

// 合成音频在 audio_output.samples（f32，交错存放），值域 -1..1。
let audio = result.audio_output.expect("TTS 应返回音频");
let samples = audio.samples.expect("应携带采样数据");
println!("{}Hz {}ch {} 采样", audio.sample_rate, audio.channels, samples.len());
```

## 关键类型速查

| 类型 | 说明 |
|---|---|
| [`Registry`](src/registry.rs) | 枚举模型族 / loader / 设备；`load()` 加载模型 |
| [`Model`](src/model.rs) | 已加载模型：`metadata()` / `capabilities()` / `create_task_session()` |
| [`Session`](src/session.rs) | 任务会话：离线 `run_offline()`；流式 `start`/`process_audio`/`finish`/`reset` |
| [`TaskKind`](src/types.rs) | `Vad` / `Asr` / `Tts` / `Diar` / `SourceSeparation` |
| [`ModelFamily`](src/types.rs) | 模型族枚举（`Qwen3Asr` / `CitrinetAsr` / `Htdemucs` / …；未收录族用 `Custom(String)`） |
| [`RunMode`](src/types.rs) | `Offline` / `Streaming` |
| [`Backend`](src/types.rs) | `Cpu` / `Cuda` / `Hip` / `Vulkan` / `Metal` / `Best` |
| [`TaskResult`](src/types.rs) | `speech_segments` / `text_output` / `audio_output` / `named_audio_outputs` |
| [`StreamEvent`](src/types.rs) | 流式事件：`voice_activity` / `partial_text` / `audio_output` / `is_final` |
| [`load_wav`](src/audio.rs) | 读 WAV 为 `WavAudio { sample_rate, channels, samples }` |

所有枚举的 `as_str()` 返回传给 C 边界的字符串；结构化数据一律走 JSON
（`request_json` 为任意 JSON 对象，如 `{"audio_path":...,"options":{...}}`）。

## 注意事项

- **family_hint 必填场景**：NeMo safetensors（如 marblenet_vad）与 GGUF
  （如 citrinet_asr / moss_tts_nano / htdemucs / sortformer_diar / qwen3_asr）
  无法被引擎自动探测族别，会误判为 silero_vad，必须显式传 `family_hint`。
  用 [`ModelFamily`](src/types.rs) 枚举代替裸字符串（如
  `Some(ModelFamily::Qwen3Asr)`）可避免拼写错误；内置 silero_vad 可省略。
- **阈值选项键**：silero_vad 用 `vad_threshold`，marblenet_vad 用 `threshold`。
- **Windows 路径**：请求 JSON 中的反斜杠必须转义（`\\`），建议改用正斜杠；
  `\a` 等非法转义会让 shim 解析失败。
- **线程**：`Session` 为 `Send`；事件回调要求 `Send` 闭包，可能从 C++ 线程调用。
- **错误**：所有方法返回 [`Error`](src/error.rs)，底层错误信息经
  `audiocpp_last_error()` 透传，为类型化枚举，可用 `?` 传播。

完整可运行示例见 [`examples/`](examples/)（vad_offline / vad_streaming /
asr_offline / tts_offline / diar_offline / sep_offline，均在本机 win32/MSVC 验证运行）。
