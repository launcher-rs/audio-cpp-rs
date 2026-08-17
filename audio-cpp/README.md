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
         ├─ 离线:     session.run_offline(Request) → TaskResult
         └─ 流式:     session.set_event_callback(cb)
                      session.start(Request) → process_audio(&[f32], ...) → ... → finish() → TaskResult
                      session.reset()     # 复用会话开始新一轮
```

各对象持有 C 句柄并在 `Drop` 中释放；`Model` 不管理 `Registry` 的生命周期，
注册表应存活于所有派生模型的使用期之内。

所有请求都用类型化的 [`Request`](src/request.rs) 枚举构造（也可直接传 JSON
字符串透传）：每个任务一种变体、携带各自参数，无需手工拼接或转义 JSON，
Windows 路径也不需要转义反斜杠。

```rust
use audio_cpp::Request;

let r1 = Request::vad(".\\speech.wav").option("vad_threshold", 0.5); // VAD：音频 + 阈值
let r2 = Request::asr(".\\speech.wav");                              // ASR / 分离等：音频
let r3 = Request::asr(".\\speech.wav").option("audio_chunk_seconds", 3.0); // 流式窗口
let r4 = Request::tts("Hello!");                                     // TTS：文本
let r5 = Request::tts("Hello!").reference(".\\ref.wav")              // TTS 声音克隆
                                  .reference_text("参考文本转写");
let r6 = Request::diar(".\\speech.wav");       // 说话人分离
let r7 = Request::source_separation(".\\song.wav"); // 音乐源分离
let r8 = Request::json(r#"{"audio_path":"x.wav"}"#); // 原始 JSON 透传

let json = r1.to_json()?;   // 序列化为 JSON 字符串
```

常用链式方法：`option(key, value)` / `options([...])`（任意任务）、
`reference(audio)` / `reference_text(text)` / `language(lang)`（TTS）。

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
use audio_cpp::{Backend, Registry, Request, RunMode, TaskKind};

let registry = Registry::new()?;
let model = registry.load("./silero_vad_16k.safetensors", None, None)?;

let session = model.create_task_session(
    TaskKind::Vad, RunMode::Offline, Backend::Cpu, 0, 4, None,
)?;
let result = session.run_offline(
    Request::vad("./sample.wav").option("vad_threshold", 0.5),
)?;

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

session.start(())?;   // VAD 流式无需请求参数
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
use audio_cpp::{Backend, ModelFamily, Registry, Request, RunMode, TaskKind};

let registry = Registry::new()?;
// GGUF 无法自动探测族别（会误判为 silero_vad），必须显式 family_hint。
let model = registry.load("./citrinet-asr-q8_0.gguf", Some(ModelFamily::CitrinetAsr), None)?;
let session = model.create_task_session(
    TaskKind::Asr, RunMode::Offline, Backend::Cpu, 0, 4, None,
)?;

let result = session.run_offline(Request::asr("./speech.wav"))?;
if let Some(text) = &result.text_output {
    println!("转录: {}", text.text);
}
```

### 5. 流式 ASR（Qwen3 ASR，需按需编译）

Qwen3 ASR 同时支持离线与流式。流式会话与 VAD 类似：
`start(请求)` → 分块 `process_audio()` → `finish()`；窗口边界会经事件回调
产出 `partial_text` 部分转录。**streaming 的 `start` 请求需带 `audio_path`（或
`audio` 对象）以建立音频契约**，否则 `prepare` 会报错。

```rust
use std::sync::{Arc, Mutex};
use audio_cpp::{Backend, ModelFamily, Registry, Request, RunMode, StreamEvent, TaskKind, load_wav};

let registry = Registry::new()?;
let model = registry.load("./qwen3-asr-q8_0.gguf", Some(ModelFamily::Qwen3Asr), None)?;
let wav = load_wav("./speech.wav")?;

let mut session = model.create_task_session(
    TaskKind::Asr, RunMode::Streaming, Backend::Cpu, 0, 4, None,
)?;
let policy = session.streaming_policy()?;

let partial = Arc::new(Mutex::new(Vec::<String>::new()));
let collector = Arc::clone(&partial);
session.set_event_callback(Some(move |ev: StreamEvent| {
    if let Some(t) = &ev.partial_text {
        collector.lock().unwrap().push(t.text.clone());
    }
}));

// streaming 的 start 请求须含音频契约（audio_path 或 audio 对象）。
let request = Request::asr("./speech.wav").option("audio_chunk_seconds", 3.0);
session.start(request)?;

let chunk = (policy.preferred_audio_chunk_seconds * wav.sample_rate as f64).round() as usize;
for block in wav.samples.chunks(chunk) {
    session.process_audio(block, wav.sample_rate, wav.channels, 0)?;
}
let result = session.finish()?;   // 最终完整文本
session.reset();
```

> **注意**：`preferred_audio_chunk_samples` 可能为 0，Qwen3 ASR 只填
> `preferred_audio_chunk_seconds`，分块大小按 `秒数 × 采样率` 换算即可。

### 6. 离线 TTS（MOSS-TTS-Nano，需 custom-models 构建）

```rust
use audio_cpp::{Backend, ModelFamily, Registry, Request, RunMode, TaskKind};

let registry = Registry::new()?;
let model = registry.load("./moss-tts-nano-100m-q8_0.gguf", Some(ModelFamily::MossTtsNano), None)?;
let session = model.create_task_session(
    TaskKind::Tts, RunMode::Offline, Backend::Cpu, 0, 4, None,
)?;

let result = session.run_offline(Request::tts("Hello from Rust and audio.cpp!"))?;

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
| [`Request`](src/request.rs) | 类型化请求枚举：`Request::vad` / `asr` / `diar` / `source_separation` / `tts` / `json`；`option(...)` / `reference(...)` |
| [`TaskKind`](src/types.rs) | `Vad` / `Asr` / `Tts` / `Diar` / `SourceSeparation` |
| [`ModelFamily`](src/types.rs) | 模型族枚举（`Qwen3Asr` / `CitrinetAsr` / `Htdemucs` / …；未收录族用 `Custom(String)`） |
| [`RunMode`](src/types.rs) | `Offline` / `Streaming` |
| [`Backend`](src/types.rs) | `Cpu` / `Cuda` / `Hip` / `Vulkan` / `Metal` / `Best` |
| [`TaskResult`](src/types.rs) | `speech_segments` / `text_output` / `audio_output` / `named_audio_outputs` |
| [`StreamEvent`](src/types.rs) | 流式事件：`voice_activity` / `partial_text` / `audio_output` / `named_audio_outputs` / `is_final` |
| [`load_wav`](src/audio.rs) | 读 WAV 为 `WavAudio { sample_rate, channels, samples }` |

所有枚举的 `as_str()` 返回传给 C 边界的字符串；结构化数据一律走 JSON。请求用
类型化 [`Request`](src/request.rs) 构造（也可传任意 JSON 字符串透传，如
`session.run_offline(r#"{"audio_path":...}"#)`）。

## 注意事项

- **family_hint 必填场景**：NeMo safetensors（如 marblenet_vad）与 GGUF
  （如 citrinet_asr / moss_tts_nano / htdemucs / sortformer_diar / qwen3_asr）
  无法被引擎自动探测族别，会误判为 silero_vad，必须显式传 `family_hint`。
  用 [`ModelFamily`](src/types.rs) 枚举代替裸字符串（如
  `Some(ModelFamily::Qwen3Asr)`）可避免拼写错误；内置 silero_vad 可省略。
- **阈值选项键**：silero_vad 用 `vad_threshold`，marblenet_vad 用 `threshold`。
- **Windows 路径**：用 [`Request`](src/request.rs) 构造器时反斜杠无需转义；
  若直接传 JSON 字符串，则反斜杠必须转义（`\\`），`\a` 等非法转义会让 shim
  解析失败。
- **线程**：`Session` 为 `Send`；事件回调要求 `Send` 闭包，可能从 C++ 线程调用。
- **错误**：所有方法返回 [`Error`](src/error.rs)，底层错误信息经
  `audiocpp_last_error()` 透传，为类型化枚举，可用 `?` 传播。

完整可运行示例见 [`examples/`](examples/)（vad_offline / vad_streaming /
asr_offline / asr_streaming / tts_offline / tts_streaming / diar_offline /
sep_offline / registry_inspect）。其中 vad_streaming / asr_streaming /
tts_streaming / registry_inspect 已在本机 win32/MSVC 验证运行；其余离线示例
此前已验证。测试权重文件请自行放置（不要提交到仓库）。
