# audio-cpp-rs

> 用 Rust 封装 [audio.cpp](https://github.com/0xShug0/audio.cpp) —— 基于 ggml 的高性能本地音频推理引擎。

audio.cpp 是一个纯 C++ 音频推理框架，基于 ggml，支持 TTS、STT（ASR）、VAD、说话人分离、音源分离、声音克隆、编解码与音乐生成等 44+ 模型族任务，后端覆盖 CPU / CUDA / HIP / Vulkan / Metal，可加载 GGUF 与 safetensors 模型。

本项目**不重写 audio.cpp**，而是为 Rust 开发者提供一条**安全、易用、与 C++ 运行时共享实现**的绑定路径。

## 工作区结构

| Crate | 说明 |
|---|---|
| [`audio-cpp-sys`](audio-cpp-sys/) | **底层 FFI**：用 CMake 构建 `engine_runtime` 静态库，bindgen 生成 C ABI 绑定 |
| [`audio-cpp`](audio-cpp/) | **高层安全封装**：类型安全的注册表 / 模型 / 会话 API（离线 + 流式） |

```
C:\code\audio-cpp-rs\
├── audio-cpp-sys\        # 底层绑定 crate
│   ├── build.rs          # CMake(Ninja) 构建 engine_runtime + cc 编译 shim + bindgen
│   ├── capi.h            # C ABI 头文件（跨边界契约的全部内容）
│   ├── capi.cpp          # C++ shim：把 audio.cpp 的异常转换为 C 错误码
│   └── audio.cpp\        # ⚠️ vendored 上游源码（约 200MB，.gitignore 排除，不入库）
├── audio-cpp\            # 高层安全封装 crate（骨架）
└── docs\research_report.md  # 调研报告
```

### 关于 vendored 上游源码

上游仓库通过普通目录拷贝引入：`audio.cpp/` → `audio-cpp-sys/audio.cpp`，由 `.gitignore` 排除，**不提交进 git**（参考 llama-cpp-rs 用 git submodule 引入 llama.cpp 的做法，本仓库先采用拷贝方式，后续可平滑切换为 submodule）。克隆本项目后需自行获取音频工具链：

```bash
git clone https://github.com/0xShug0/audio.cpp.git audio-cpp-sys/audio.cpp
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

| Feature | audio.cpp 映射 | 说明 |
|---|---|---|
| `core-models`（默认） | `AUDIOCPP_MODEL_SET=core` | 引擎核心 + 内置 VAD（silero/marblenet） |
| `custom-models` | `AUDIOCPP_MODEL_SET=custom` | 只编译指定模型族（设 `AUDIOCPP_MODELS`，见下） |
| `full-models` | `AUDIOCPP_MODEL_SET=full` | 全部 44+ 模型族 |
| `cuda` | `ENGINE_ENABLE_CUDA=ON` | 需要 CUDA 工具链 |
| `hip` | `ENGINE_ENABLE_HIP=ON` | 需要 HIP/ROCm |
| `vulkan` | `ENGINE_ENABLE_VULKAN=ON` | Vulkan SDK |
| `metal` | `ENGINE_ENABLE_METAL=ON` | Apple 平台默认开启 |
| `openmp` | `ENGINE_ENABLE_OPENMP=ON` | OpenMP 并行 |
| `native` | `ENGINE_ENABLE_NATIVE_CPU=ON` | 为本地 CPU 生成优化指令 |

> `custom-models` 按需编译指定模型族，避免 `full-models` 全量 44+ 的编译成本；
> 引擎核心与内置 VAD 始终编入。用法：`$env:AUDIOCPP_MODELS="citrinet_asr"; cargo build --features custom-models`（逗号分隔多个族）。
> `cuda` 与 `hip` 互斥（audio.cpp 的 CMake 会校验）；二者可用环境变量精确控制（见 `build.rs` 中透传的 `GGML_*` / `CMAKE_*`）。

## 环境变量

| 变量 | 作用 |
|---|---|
| `BUILD_DEBUG=1` | 开启 build.rs 调试日志（`cargo:warning`） |
| `CMAKE_VERBOSE` | 开启 CMake 详细输出 |
| `AUDIOCPP_LIB_PROFILE` | 覆盖 CMake 构建 profile（默认 `Release`） |
| `GGML_*` / `CMAKE_*` | 透传给 CMake 缓存，微调 ggml 选项 |

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

# 5) 离线 ASR（须先 custom-models 构建并下载 Citrinet GGUF，见 asr_offline.rs 头注）
$env:AUDIOCPP_MODELS="citrinet_asr"
cargo run -p audio-cpp --features custom-models --example asr_offline -- `
  ./citrinet-asr-q8_0.gguf audio-cpp-sys/audio.cpp/assets/resources/sample_16k.wav

# 4) 流式 VAD（事件回调 + 分块 process_audio）
cargo run -p audio-cpp --example vad_streaming -- `
  audio-cpp-sys/audio.cpp/assets/framework/models/silero_vad/silero_vad_16k.safetensors `
  audio-cpp-sys/audio.cpp/assets/resources/sample_16k.wav
```

> 内置 VAD 权重（silero / marblenet）随上游源码 vendored 在 `audio.cpp/assets/framework/models/`，无需另行下载。

## 状态

- [x] 工作区骨架（workspace、audio-cpp-sys、audio-cpp）
- [x] C ABI 设计与实现（capi.h/capi.cpp）
- [x] 构建脚本（CMake + cc + bindgen）
- [x] 首次端到端构建验证（win32/MSVC，`cargo build --workspace` 通过）
- [x] 示例（inspect、vad_offline_ffi、高层 vad_offline / vad_streaming，均在本机运行验证）
- [x] 高层安全 API（Registry / Model / Session，离线 + 流式）
- [x] 内置 VAD 两种模型端到端验证：silero_vad（离线+流式）、marblenet_vad（离线）
- [x] ASR 端到端验证：Citrinet ASR Q8_0 GGUF 离线转录（sample_16k.wav → Nature 台词）
- [x] `custom-models` feature：按需编译指定模型族，避免 full 全量成本
- [ ] 更多模型族端到端验证（TTS 等）

## 参考

- [audio.cpp 官方仓库](https://github.com/0xShug0/audio.cpp)
- [`temp/llama-cpp-rs`](temp/llama-cpp-rs) —— llama-cpp-sys-2 / llama-cpp-2 参考实现（submodule 方式引入上游）
- [docs/research_report.md](docs/research_report.md) —— 调研报告

## 许可

MIT OR Apache-2.0