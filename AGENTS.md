# AGENTS.md

本文件为在此仓库中工作的 AI 代理提供项目上下文与约定。

## 项目概览

`audio-cpp-rs` 是把 C++ 音频推理框架 [audio.cpp](https://github.com/0xShug0/audio.cpp) 封装为 Rust 库的项目。目标是**共享 C++ 运行时实现**，而非重写。

工作区（resolver = 2）：

- **`audio-cpp-sys/`** — 底层 FFI crate。`build.rs` 负责全部本地构建：
  1. CMake + **Ninja** 构建上游 `engine_runtime` 静态库（vendored at `audio-cpp-sys/audio.cpp`）；
  2. `cc` 编译 C shim（`capi.h` / `capi.cpp`）；
  3. `bindgen` 生成绑定，输出到 `OUT_DIR/bindings.rs`，由 `src/lib.rs` 用 `include!` 引入。
- **`audio-cpp/`** — 高层安全封装 crate（当前为骨架，仅有模块占位）。

`links = "audio-cpp"` 声明在 `audio-cpp-sys/Cargo.toml`，防止同一程序出现两份本地运行时。

## 关键约束

### 1. vendored 上游源码不入库
`audio-cpp-sys/audio.cpp` 是上游的普通目录拷贝（约 200MB），**已被 `.gitignore` 排除**。任何操作都不应假定该目录存在且保持内容；不要在编辑 `git` 版本历史时删除/改动它。`build.rs` 会在启动时断言其存在（`audio.cpp/CMakeLists.txt`），缺失时报错提示。

### 2. C ABI 边界不可破坏
跨 C/Rust 的契约全部定义在 `audio-cpp-sys/capi.h`。修改结构或语义时，必须**同步修改**：
- `capi.h`（声明）、`capi.cpp`（实现）、build.rs bindgen allowlist（`audiocpp_.*`）；
- 高层 `audio-cpp` crate 的消耗代码。

约定：结构化数据走 JSON 字符串；音频走 `float*`；返回值为非 NULL 的 `char*`/`float*`/句柄必须用对应 `free()` 释放；异常在 shim 内捕获为错误码 + `audiocpp_last_error()`。

### 3. 注释语言
项目约定中文注释。`capi.h`、`capi.cpp`、`build.rs`、Cargo.toml 的说明性注释为中文；代码本身保持原有语言（C++/Rust）。

### 4. 构建环境（win32）
- 工具：PowerShell 7（`pwsh`）、CMake、Ninja（build.rs 强制 `generator("Ninja")`）、MSVC 或 GCC/Clang、bindgen 需要 clang 与 MSVC `INCLUDE` 环境。
- 构建命令：
  - `cargo build`（默认 `core-models` + CPU）
  - `cargo build --features full-models,openmp`
  - 验证：从仓库根目录运行 `cargo check` / `cargo build`。build.rs 有 `BUILD_DEBUG=1` 调试日志。

### 5. feature → audio.cpp 映射
| Cargo feature | CMake 选项 |
|---|---|
| `core-models`（默认） | `AUDIOCPP_MODEL_SET=core` |
| `full-models` | `AUDIOCPP_MODEL_SET=full` |
| `cuda` | `ENGINE_ENABLE_CUDA=ON`（与 `hip` 互斥） |
| `hip` | `ENGINE_ENABLE_HIP=ON` |
| `vulkan` | `ENGINE_ENABLE_VULKAN=ON` |
| `metal` | `ENGINE_ENABLE_METAL=ON`（Apple 默认） |
| `openmp` | `ENGINE_ENABLE_OPENMP=ON` |
| `native` | `ENGINE_ENABLE_NATIVE_CPU=ON` |

## 已知状态
- C ABI 设计/实现（capi.h/capi.cpp）与构建脚本已完成；
- **端到端 `cargo build --workspace` 已在 win32/MSVC 验证通过**。构建要点：
  - CMake 使用 Ninja 生成器，`CMAKE_ARCHIVE_OUTPUT_DIRECTORY=OUT_DIR/lib` 把归档统一收集；
  - MSVC 需注入 INCLUDE/LIB（build.rs 用 cc 探测）+ `/utf-8` `/EHsc` 编译选项（audio.cpp 部分含中文源码，缺 `/utf-8` 会报 C2001）；
  - shim 编译需 `/std:c++17`（MSVC）而非 `-std=c++17`。
- `audio-cpp` 高层安全 API 已实现（Registry / Model / Session，离线 + 流式），
  并有 3 个示例验证运行通过：`audio-cpp-sys/examples/inspect`、
  `audio-cpp-sys/examples/vad_offline_ffi`、`audio-cpp/examples/vad_offline` 与
  `audio-cpp/examples/vad_streaming`。注意：两个 crate 的示例输出文件名会在
  `target/debug/examples` 冲突，不要给不同 crate 的示例取同名。
- 流式会话（silero_vad）每块必须恰好 `preferred_audio_chunk_samples`（512）个采样，
  末尾不足块需补零；其流式事件经 `is_final` 汇总，single-chunk 调试时无逐块事件也正常。

## 常用命令速查
```bash
git clone https://github.com/0xShug0/audio.cpp.git audio-cpp-sys/audio.cpp   # 补 vendored 源码
cargo build -p audio-cpp-sys                                                  # 单独构建底层
BUILD_DEBUG=1 cargo build                                                      # 调试构建脚本
```