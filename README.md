# audio-cpp-rs

> 用 Rust 封装 [audio.cpp](https://github.com/0xShug0/audio.cpp) —— 基于 ggml 的高性能本地音频推理引擎。

audio.cpp 是一个纯 C++ 音频推理框架，基于 ggml，支持 TTS、STT（ASR）、VAD、说话人分离、音源分离、声音克隆、编解码与音乐生成等 44+ 模型族任务，后端覆盖 CPU / CUDA / HIP / Vulkan / Metal，可加载 GGUF 与 safetensors 模型。

本项目**不重写 audio.cpp**，而是为 Rust 开发者提供一条**安全、易用、与 C++ 运行时共享实现**的绑定路径。

## 工作区结构

| Crate | 说明 |
|---|---|
| [`audio-cpp-sys`](audio-cpp-sys/) | **底层 FFI**：用 CMake 构建 `engine_runtime` 静态库，bindgen 生成 C ABI 绑定 |
| [`audio-cpp`](audio-cpp/) | **高层安全封装**：类型安全的注册表 / 模型 / 会话 API（离线 + 流式）。使用指南见 [`audio-cpp/README.md`](audio-cpp/README.md) |

```
audio-cpp-rs/
├── audio-cpp-sys/        # 底层绑定 crate
│   ├── build.rs          # CMake(Ninja) 构建 engine_runtime + cc 编译 shim + bindgen
│   ├── capi.h            # C ABI 头文件（跨边界契约的全部内容）
│   ├── capi.cpp          # C++ shim：把 audio.cpp 的异常转换为 C 错误码
│   └── audio.cpp/        # ⚠️ git submodule 引入的上游源码（内容不入库）
├── audio-cpp/            # 高层安全封装 crate
└── docs/                 # 文档：调研报告、预编译报告、使用指南等
```

### 关于上游源码（git submodule）

上游 `audio.cpp` 以 **git submodule** 引入：`.gitmodules` → `https://github.com/0xShug0/audio.cpp.git`，内容**不提交进本仓库**。克隆本项目后需先补齐子模块：

```bash
git submodule update --init --recursive
```

## 设计约定（C ABI）

跨边界契约固定在 `capi.h` / `capi.cpp`：

- **结构化数据**一律以 JSON 字符串（UTF-8，`\0` 结尾）跨过 ABI 边界；
- **音频采样**以 `float` 数组跨过边界；
- **所有权**：C++ 侧持有对象，`audiocpp_*` 返回的句柄 / 字符串 / 采样缓冲都必须用对应的 `free()` 释放；
- **错误**：函数返回 0 成功、非 0 失败，最近一次错误信息可用 `audiocpp_last_error()` 获取；audio.cpp 抛出的异常在 shim 边界被捕获，绝不会泄漏进 C。

### 对象模型

```
Registry（注册表：枚举/加载模型族）
  └─ audiocpp_registry_load() → Model（已加载的模型：元数据/能力）
       └─ audiocpp_model_create_task_session() → Session（离线/流式任务会话）
            ├─ Offline:  prepare() → run_offline() → TaskResult(JSON)
            └─ Streaming: set_event_sink() → start() → process_audio() → finish()
```

## 构建

依赖：CMake ≥ 3.20、Ninja、C/C++ 编译器（MSVC 或 GCC/Clang）、（bindgen 需要）LLVM/clang 与 MSVC 环境。

首次构建前先拉取上游源码（见上），随后：

```bash
cargo build            # 默认：core-models + CPU
cargo build --release
```

可选的 Cargo features：

> 所有 feature 在两个 crate 上均可使用（`audio-cpp` 会原样转发给 `audio-cpp-sys`）。

### 模型组合（互斥，决定编译哪些模型族）

| Feature | audio.cpp 映射 | 作用 |
|---|---|---|
| `core-models`（默认） | `AUDIOCPP_MODEL_SET=core` | **最小编译集**：引擎核心 + 内置 VAD（silero_vad / marblenet_vad）。内置 VAD 权重随上游 vendored，开箱即用 |
| `model-<族>` | `AUDIOCPP_MODEL_SET=custom` | **按需编译单个模型族**（最推荐，无需设环境变量） |
| `custom-models` | `AUDIOCPP_MODEL_SET=custom` | **按需编译**：引擎核心 + 内置 VAD + `AUDIOCPP_MODELS` 指定的模型族（兜底，配合环境变量使用） |
| `full-models` | `AUDIOCPP_MODEL_SET=full` | **全量**：audio.cpp 全部 44+ 模型族（编译极慢、产物很大，权重仍需自行下载） |

**按需编译单个模型族**：启用对应的 `model-<族>` feature 即可，无需设置环境变量：

```powershell
cargo build --features model-qwen3-asr        # 只编译 Qwen3 ASR + 内置 VAD
cargo run -p audio-cpp --features model-citrinet-asr --example asr_offline   # ASR 示例
```

已内置的常用族 feature（完整列表见各 crate 的 Cargo.toml）：

| Feature | 模型族（上游 target/alias） | 用途 |
|---|---|---|
| `model-qwen3-asr` | qwen3_asr | ASR（已验证） |
| `model-citrinet-asr` | citrinet_asr | ASR（已验证） |
| `model-fun-asr-nano` | fun_asr_nano | ASR |
| `model-hviske-asr` / `model-kroko-asr` / `model-nemotron-asr` / `model-parakeet-tdt` / `model-vibevoice-asr` | 对应 asr 族 | ASR |
| `model-moss` | moss（moss_tts_nano / moss_tts_local） | TTS（已验证） |
| `model-qwen3-tts` / `model-irodori-tts` | qwen3_tts / irodori_tts | TTS（qwen3 已验证） |
| `model-fish-audio` | fish_audio | TTS / 声音克隆 |
| `model-demucs` | demucs（htdemucs） | 音频分离 |
| `model-roformer` | roformer（mel_band_roformer） | 音频分离 |

- build.rs 会自动收集所有启用的 `model-<族>` feature（经 `CARGO_FEATURE_MODEL_*`
  环境变量），并把它们与 `AUDIOCPP_MODELS` 环境变量取**并集**——两者可混用；
  新增族 feature 只要在 Cargo.toml 声明名字与上游 target/alias 一致即可，无需改 build.rs。
- 其余未列出的模型族（roformer、htdemucs、outetts、glm_tts、pocket_tts、
  sortformer_diar 等约 30 个）仍用 `custom-models` + `AUDIOCPP_MODELS`：

```powershell
$env:AUDIOCPP_MODELS = "citrinet_asr,qwen3_asr,moss_tts_nano"; cargo build --features custom-models
```

> `AUDIOCPP_MODELS` 接受上游 CMake 的 **alias/target 名**（逗号分隔），引擎核心与
> 内置 VAD 始终编入。build.rs 对 custom-models 强制 re-configure，切换组合后直接
> 重新构建即生效，无需 clean。

### 计算后端（可叠加，不启用任何 GPU 后端时即 CPU 推理）

| Feature | audio.cpp 映射 | 作用 | 前提条件 |
|---|---|---|---|
| `cuda` | `ENGINE_ENABLE_CUDA=ON` | NVIDIA GPU 加速 | 已装 CUDA Toolkit（nvcc/cudart） |
| `hip` | `ENGINE_ENABLE_HIP=ON` | AMD GPU 加速（HIP/ROCm） | 已装 ROCm 工具链；**与 `cuda` 互斥** |
| `vulkan` | `ENGINE_ENABLE_VULKAN=ON` | Vulkan 通用计算后端 | Vulkan SDK 与驱动；跨厂商通用，性能/算子覆盖通常不如专属后端 |
| `metal` | `ENGINE_ENABLE_METAL=ON` | Apple Metal | **macOS 默认即开启**；win32 上默认关闭，需设 `AUDIOCPP_FORCE_METAL` 才强制开启 |
| `openmp` | `ENGINE_ENABLE_OPENMP=ON` | 多线程并行加速 | OpenMP 运行时（MSVC `/openmp`、GCC/Clang `libgomp`）；未装则链接期报错，此时应关掉而非忽略 |
| `native` | `ENGINE_ENABLE_NATIVE_CPU=ON` | 针对本机 CPU 生成优化指令（AVX2/FMA 等） | 产物不可移植到旧 CPU；目标机与构建机不同时勿开 |

```powershell
cargo build                                     # 默认 core-models + CPU
cargo build --features core-models,openmp       # CPU 多线程
cargo build --features full-models,cuda         # 全量模型 + NVIDIA GPU
cargo build --features custom-models,vulkan,openmp
```

## 环境变量

| 变量 | 作用 |
|---|---|
| `BUILD_DEBUG=1` | 开启 build.rs 调试日志（`cargo:warning`） |
| `CMAKE_VERBOSE` | 开启 CMake 详细输出 |
| `AUDIOCPP_LIB_PROFILE` | 覆盖 CMake 构建 profile（默认 `Release`） |
| `GGML_*` / `CMAKE_*` | 透传给 CMake 缓存，微调 ggml 选项 |

## 模型下载

音频模型权重**不随本仓库提供**（除内置 VAD），需要从 Hugging Face 下载：

- **官方 GGUF 包**：[`audio-cpp/audio.cpp-gguf`](https://huggingface.co/audio-cpp/audio.cpp-gguf)
  —— audio.cpp 官方发布的现成 GGUF 模型（Q8_0 / Q16 / F32 等精度，按 `Family-GGUF/<name>.gguf` 组织），大多数模型族开箱即用；
- **社区模型包**：[`mirek190/audio.cpp`](https://huggingface.co/mirek190/audio.cpp)
  —— 社区维护的补充模型；
- **官方下载工具**：上游提供 `tools/model_manager_v2.py`，按 `model_specs/*.json` 下载各模型族的默认包，优先选取现成 GGUF（见上游 [docs/model_manager.md](https://github.com/0xShug0/audio.cpp/blob/main/docs/model_manager.md)）；
- **内置权重**：silero_vad / marblenet_vad 随上游 vendored（`assets/framework/models`），无需下载。

> 各示例头注里均标注了对应模型的下载地址与所需 feature；GGUF 模型加载时须显式
> 传 `family_hint`（见 `audio-cpp/README.md`）。
>
> 想自动下载而非手动点页面：`audio-cpp/examples/download_model` 示例用
> [hf-hub](https://crates.io/crates/hf-hub)（仅示例用 dev-dependency）在本地文件
> 不存在时直接从 Hugging Face 拉到缓存再加载，详见该示例头注。

## 示例

`audio-cpp-sys/examples/` 提供直接使用 FFI 的端到端演示（已在本机 win32/MSVC 验证运行），`audio-cpp/examples/` 则展示安全高层 API：

```bash
# 底层 FFI：
# 1) 枚举注册表、设备与错误路径（验证 FFI 链路）
cargo run -p audio-cpp-sys --example inspect

# 2) silero_vad 离线 VAD：对 wav 检测语音片段，输出 JSON
cargo run -p audio-cpp-sys --example vad_offline_ffi -- `
  audio-cpp-sys/audio.cpp/assets/framework/models/silero_vad/silero_vad_16k.safetensors `
  audio-cpp-sys/audio.cpp/assets/resources/sample_16k.wav

# 高层 API（类型安全，无需手动管理 C 句柄）：
# 3) 离线 VAD（silero_vad；marblenet_vad 需传第 3 个 family_hint）
cargo run -p audio-cpp --example vad_offline -- `
  audio-cpp-sys/audio.cpp/assets/framework/models/silero_vad/silero_vad_16k.safetensors `
  audio-cpp-sys/audio.cpp/assets/resources/sample_16k.wav

# marblenet_vad（NeMo 格式，引擎自动探测易误判，须显式指定模型族）
cargo run -p audio-cpp --example vad_offline -- `
  audio-cpp-sys/audio.cpp/assets/framework/models/marblenet_vad/marblenet_vad.safetensors `
  audio-cpp-sys/audio.cpp/assets/resources/sample_16k.wav marblenet_vad

# 4) 流式 VAD（事件回调 + 分块 process_audio）
cargo run -p audio-cpp --example vad_streaming -- `
  audio-cpp-sys/audio.cpp/assets/framework/models/silero_vad/silero_vad_16k.safetensors `
  audio-cpp-sys/audio.cpp/assets/resources/sample_16k.wav

# 5) 离线 ASR（Citrinet GGUF 需自行下载，见 asr_offline.rs 头注）
#   用 feature 方式按需编译（推荐，无需环境变量）：
cargo run -p audio-cpp --features model-citrinet-asr --example asr_offline -- `
  ./citrinet-asr-q8_0.gguf audio-cpp-sys/audio.cpp/assets/resources/sample_16k.wav

# 6) 离线 TTS（MOSS-TTS-Nano GGUF 需自行下载，见 tts_offline.rs 头注）
cargo run -p audio-cpp --features model-moss --example tts_offline -- `
  ./moss-tts-nano-100m-q8_0.gguf out.wav "Hello from Rust and audio.cpp!"

# 6b) 离线 TTS 声音克隆（Qwen3 TTS base 变体须做 voice-clone，见 tts_offline_qwen3.rs 头注）
cargo run -p audio-cpp --features model-qwen3-tts --example tts_offline_qwen3 -- `
  ./qwen3-tts-12hz-0.6b-base-q8_0.gguf `
  audio-cpp-sys/audio.cpp/assets/resources/sample_16k.wav `
  "Some call me nature. Others call me Mother Nature." `
  out.wav "Hello from Rust and Qwen3 TTS!"

# 7) 离线说话人分离（SortFormer GGUF 需自行下载并重采样到 16kHz，见 diar_offline.rs 头注）
cargo run -p audio-cpp --features model-sortformer-diar --example diar_offline -- `
  ./sortformer-diar-4spk-v1-q8_0.gguf ./four_speaker_16k.wav

# 8) 离线音乐源分离（HTDemucs GGUF 需自行下载，输入须 44.1kHz 立体声，见 sep_offline.rs 头注）
cargo run -p audio-cpp --features model-demucs --example sep_offline -- `
  ./htdemucs-q8_0.gguf ./song.wav ./sep_out

# 9) 流式 ASR（Qwen3 ASR：窗口部分转录 + 最终文本）
cargo run -p audio-cpp --features model-qwen3-asr --example asr_streaming -- `
  ./qwen3-asr-q8_0.gguf audio-cpp-sys/audio.cpp/assets/resources/sample_16k.wav

# 10) 流式 TTS（VoxCPM2：逐块音频事件 + 合并输出）
$env:AUDIOCPP_MODELS="voxcpm2"; cargo build --features custom-models
cargo run -p audio-cpp --features custom-models --example tts_streaming -- `
  ./voxcpm2-q8_0.gguf out.wav "你好，我是流式语音合成。"

# 11) 注册表内省（无需下载权重；枚举模型族/loader/设备并校验 ModelFamily 往返）
cargo run -p audio-cpp --example registry_inspect

# 12) 非 WAV 音频输入：用 symphonia 解码 mp3/flac/ogg/m4a 等任意格式喂给引擎
#    （symphonia 仅示例用 dev-dependency，不进入库依赖；用户可用任何解码库）
cargo run -p audio-cpp --example load_any_audio -- `
  audio-cpp-sys/audio.cpp/assets/framework/models/silero_vad/silero_vad_16k.safetensors `
  ./song.mp3

# 13) 模型自动下载：本地不存在时用 hf-hub 从 Hugging Face 拉取到缓存再加载
#    （hf-hub 仅示例用 dev-dependency；缓存走 HF_HOME，再次运行自动命中）
cargo run -p audio-cpp --example download_model -- `
  audio-cpp/audio.cpp-gguf Qwen3-ASR-0.6B-GGUF/qwen3-asr-0.6b-q8_0.gguf
```


## 状态

- [x] 工作区骨架（workspace、audio-cpp-sys、audio-cpp）
- [x] C ABI 设计与实现（capi.h/capi.cpp）
- [x] 构建脚本（CMake + cc + bindgen）
- [x] 首次端到端构建验证（win32/MSVC，`cargo build --workspace` 通过）
- [x] 示例（inspect、vad_offline_ffi、高层 vad_offline / vad_streaming，均在本机运行验证）
- [x] 高层安全 API（Registry / Model / Session，离线 + 流式）
- [x] 模型族端到端验证（VAD、ASR、TTS、说话人分离、音乐源分离，见 AGENTS.md）
- [x] 内置 VAD 两种模型端到端验证：silero_vad（离线+流式）、marblenet_vad（离线）
- [x] ASR 端到端验证：Citrinet ASR Q8_0 GGUF 离线转录（sample_16k.wav → Nature 台词）
- [x] TTS 端到端验证：MOSS-TTS-Nano-100M Q8_0 GGUF 离线合成（~8s 语音写入 WAV）
- [x] Qwen3 TTS 声音克隆端到端验证：qwen3-tts-12hz-0.6b-base Q8_0 GGUF（sample_16k.wav 做参考 → ~4s 克隆语音）
- [x] C ABI 音频回传：`audio_output.samples` 携带生成音频的 f32 采样
- [x] `custom-models` feature：按需编译指定模型族，避免 full 全量成本
- [x] 说话人分离端到端验证：SortFormer Diar 4spk Q8_0 GGUF（four_speaker_short.wav 重采样到 16kHz）
- [x] 音乐源分离端到端验证：HTDemucs Q8_0 GGUF（44.1kHz 立体声 → drums/bass/other/vocals）
- [x] `ModelFamily` 枚举族名替代裸字符串 hint（并给出 `Custom(String)` 兜底）
- [x] 流式 ASR 端到端验证：Qwen3 ASR Q8_0 GGUF（sample_16k.wav → 逐窗口部分转录 + 最终文本）
- [x] 注册表内省示例（registry_inspect）与流式 TTS 示例（tts_streaming，VoxCPM2）
- [x] C ABI 流事件回传：`StreamEvent.named_audio_outputs`（流式 TTS 逐块音频）
- [x] 非 WAV 音频输入示例（load_any_audio）：symphonia 解码 mp3/flac 实测跑通（symphonia 为 dev-dependency，不进入库依赖）
- [x] 模型自动下载示例（download_model）：hf-hub 从官方仓库实测下载 citrinet GGUF 并缓存复用（hf-hub 为 dev-dependency，不进入库依赖）

## 参考

- [audio.cpp 官方仓库](https://github.com/0xShug0/audio.cpp)
- [llama-cpp-rs（utilityai，v2：llama-cpp-sys-2 / llama-cpp-2）](https://github.com/utilityai/llama-cpp-rs) —— 本地参考实现（submodule 方式引入上游）
- [llama-cpp-rs（eugenehp，v4）](https://github.com/eugenehp/llama-cpp-rs) —— 新一代 llama-cpp-rs（llama-cpp-rs4）
- [docs/research_report.md](docs/research_report.md) —— 调研报告

## 许可

Apache-2.0