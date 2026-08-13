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

### 使用预编译库（跳过 CMake 构建）

构建时默认会编译 `audio.cpp` 的 `engine_runtime` 及其依赖（耗时较长）。有以下两种方式
使用预编译静态库，跳过 **整个 CMake 构建**：

1. **显式指定目录**（任何情况都可用）：

```bash
# 目录内需包含 engine_runtime 及依赖库（*.lib / *.a），可放在
#   <dir>、<dir>/lib、<dir>/lib64 或 <dir>/bin
export AUDIOCPP_PREBUILT_DIR=/path/to/prebuilt

cargo build
```

2. **自动下载**（`prebuilt` feature，需发布对应 GitHub Release 资产）：

```toml
[dependencies]
audio-cpp-sys = { version = "0.1", features = ["prebuilt"] }
```

```bash
cargo build --features prebuilt
```

按当前平台/后端/模型组合自动拼资产名并下载，缓存到
`target/audio-cpp-prebuilt-cache/<tag>/`。资产命名
`audio-cpp-prebuilt-{linux|macos|windows}-{target}-{backend}-{modelset}-static.tar.gz`。

> **只发布 `full` 全模型资产**（full 是任何 model 组合的超集，见下）。core /
> `custom-<族>` 组合会自动回退下载 full 资产（体积较大但保证可用），仍失败才
> 回落源码构建。

下载/缓存行为可用环境变量定制（适合内网或访问不了 GitHub 的环境）：

| 环境变量 | 作用 |
|---|---|
| `AUDIOCPP_PREBUILT_URL` | 下载地址。可含 `{tag}` / `{asset}` 占位符（镜像站场景：`https://mirror.example.com/audio-cpp/{asset}`）；不含占位符视为完整地址；`file://` 前缀表示本地归档，直接复制不走网络 |
| `AUDIOCPP_PREBUILT_TAG` | Release tag，默认 `v{version}` |
| `AUDIOCPP_PREBUILT_REPO` | GitHub 仓库，默认 `launcher-rs/audio-cpp-rs` |
| `AUDIOCPP_PREBUILT_DIR` | 显式本地库目录（不下载，直接链接） |
| `AUDIOCPP_PREBUILT_OFF` | 设为 1/true 禁用自动下载，强制源码构建 |

网络下载失败会自动重试 3 次（带退避）；HTTP 4xx（资源不存在等）为确定性
失败不重试，直接回落源码构建。

要点：

- **`full` 资产是任何模型组合的超集**：无论开启哪些 `model-*` feature（或
  `AUDIOCPP_MODELS` 指定多族），下载端都会先尝试精确资产名（`core` /
  `custom-<族1>-<族2>...`），404 时自动回退下载 `full` 资产，仍失败才回落
  源码构建；
- CUDA / Vulkan 仍需要本地 SDK 参与链接（静态库不传导其运行时依赖），只省编译；
- C shim（`capi.cpp`）与 Rust 绑定仍从源码编译，因此上游 `audio.cpp` 源码树依然
  需要（`prebuilt` 旁路会自动获取）；
- 归档内 `metadata.json` 记录 `audio_commit`（打包时 audio.cpp submodule HEAD）与
  `msvc_ver`（打包工具链的 `_MSC_VER`）。下载后自动校验：`audio_commit` 与本地
  submodule 不一致，或本地 MSVC 版本低于归档的 `msvc_ver`，则删缓存回落源码构建
  （避免 ABI 错配；MSVC 静态库绑定工具集版本）。

> 资产由 CI（`.github/workflows/prebuilt-audio-cpp.yml`）在打 `v*` tag 时生成并上传；
> 设计见 [docs/prebuilt_pattern_report.md](../../docs/prebuilt_pattern_report.md)。

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
