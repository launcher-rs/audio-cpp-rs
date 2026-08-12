# audio-cpp-sys

[audio.cpp](https://github.com/0xShug0/audio.cpp)（基于 ggml 的本地音频推理引擎）的**底层 FFI 绑定**。

本 crate 是 `audio-cpp-rs` 工作区的一部分，只负责把 C++ 运行时构建出来并导出原始 C ABI：

1. `build.rs` 用 **CMake + Ninja** 构建上游 `engine_runtime` 静态库，用 `cc` 编译 C shim（`capi.cpp`）；
2. `bindgen` 从 [`capi.h`](capi.h) 生成 Rust 绑定，`lib.rs` 以 `audio_cpp_sys::*` 导出。

> 需要**高层、类型安全的 API** 请使用 [`audio-cpp`](https://crates.io/crates/audio-cpp)。

## 依赖

- **上游源码**：`audio.cpp` 以 **git submodule** 引入。构建脚本会断言 `audio.cpp/CMakeLists.txt` 存在，缺失时报错提示。
- **工具链**（win32 实测）：CMake、Ninja、MSVC（或 GCC/Clang）、bindgen 所需的 clang 与 MSVC `INCLUDE`/`LIB` 环境。

## 使用

把本 crate 加入依赖：

```toml
[dependencies]
audio-cpp-sys = "0.1"
```

构建默认的 `core-models`（引擎核心 + 内置 VAD）：

```bash
cargo build
```

### features

**模型组合**（互斥，默认 `core-models`）：

| feature | 说明 |
|---|---|
| `core-models`（默认） | 引擎核心 + 内置 VAD（silero_vad / marblenet_vad），权重随上游 vendored，开箱即用 |
| `custom-models` | 按需编译：配合 `AUDIOCPP_MODELS` 环境变量（逗号分隔 model alias） |
| `model-<族>` | 常用模型族的专用开关（如 `model-qwen3-asr`、`model-moss`、`model-demucs`、`model-sortformer-diar`），无需环境变量 |
| `full-models` | 全量 44+ 模型族（编译慢，且仍要自行下载权重） |

**计算后端**（可叠加，默认 CPU）：

| feature | 说明 |
|---|---|
| `cuda` | NVIDIA CUDA（需 CUDA Toolkit；与 `hip` 互斥） |
| `hip` | AMD HIP/ROCm（与 `cuda` 互斥） |
| `vulkan` | Vulkan 通用后端（需 Vulkan SDK） |
| `metal` | Apple Metal（macOS 默认开启） |
| `openmp` | OpenMP 并行（需对应运行时） |
| `native` | 针对本机 CPU 优化（不移植） |

## C ABI 约定

所有绑定函数遵循同一套契约（详见 [`capi.h`](capi.h)）：

- 结构化数据以 **JSON 字符串**（UTF-8、`\0` 结尾）跨边界传递；
- 音频采样以 **`float` 数组**传递；
- 返回 `0` 成功、非 `0` 出错，错误信息用 `audiocpp_last_error()` 获取；
- 返回的 `char*` / `float*` / 句柄若非空，必须用对应的 `free()` 释放（`audiocpp_free_string` / `audiocpp_audio_free` / `audiocpp_*_free`）。

三个核心句柄：

- `audiocpp_registry` —— 模型注册表（`audiocpp_registry_default` / `..._load`）；
- `audiocpp_model` —— 已加载的模型（`audiocpp_model_metadata_json` / `..._create_task_session`）；
- `audiocpp_session` —— 一次任务会话，离线（`run_offline`）或流式（`start` / `process_audio` / `finish`）。

## 示例

```bash
# 枚举注册表与设备（无需任何模型文件）
cargo run -p audio-cpp-sys --example inspect

# 离线 VAD（需要 silero_vad_16k.safetensors 权重）
cargo run -p audio-cpp-sys --example vad_offline_ffi -- \
  ./silero_vad_16k.safetensors ./speech.wav
```

## License

Apache-2.0
