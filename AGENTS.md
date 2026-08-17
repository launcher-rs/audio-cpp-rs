# AGENTS.md

本文件为在此仓库中工作的 AI 代理提供项目上下文与约定。

## 项目概览

`audio-cpp-rs` 是把 C++ 音频推理框架 [audio.cpp](https://github.com/0xShug0/audio.cpp) 封装为 Rust 库的项目。目标是**共享 C++ 运行时实现**，而非重写。

工作区（resolver = 3，edition 2024）：

- **`audio-cpp-sys/`** — 底层 FFI crate。`build.rs` 负责全部本地构建：
  1. CMake + **Ninja** 构建上游 `engine_runtime` 静态库（submodule at `audio-cpp-sys/audio.cpp`）；
  2. `cc` 编译 C shim（`capi.h` / `capi.cpp`）；
  3. `bindgen` 生成绑定，输出到 `OUT_DIR/bindings.rs`，由 `src/lib.rs` 用 `include!` 引入。
- **`audio-cpp/`** — 高层安全封装 crate（当前为骨架，仅有模块占位）。

`links = "audio-cpp"` 声明在 `audio-cpp-sys/Cargo.toml`，防止同一程序出现两份本地运行时。

## 关键约束

### 1. 上游源码以 git submodule 引入
`audio-cpp-sys/audio.cpp` 是 git submodule（`.gitmodules` → `https://github.com/0xShug0/audio.cpp.git`），
**内容不入库**。克隆本项目后必须执行 `git submodule update --init --recursive` 补齐。
任何操作都不应假定该目录存在且保持内容；不要在编辑 `git` 版本历史时删除/改动它。
`build.rs` 会在启动时断言其存在（`audio.cpp/CMakeLists.txt`），缺失时报错提示。

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
- **cuda feature 链接**：engine_runtime/ggml-cuda 是静态库，它们 PRIVATE 的 CUDA
  依赖不会传导到最终可执行文件。build.rs 在启用 `cuda` 时会用
  `CUDA_PATH`/`nvcc`/常见安装目录定位 Toolkit 的 `lib/x64`（或 `lib64`），显式输出
  `cudart/cublas/cublasLt/cufft/cuda` 链接。`cuda.lib`（驱动 import lib，11+ 的
  Toolkit 自带；旧版叫 `nvcuda.lib`，本机 12.4 用 `cuda`）。运行 CUDA 程序需
  `bin/cudart64_*.dll` 在 PATH。另注意：**CMake 会在 build_dir/CMakeFiles/.../
  CompilerIdCUDA 生成 `a.lib` 探测产物，递归收集静态库时须跳过 CMakeFiles
  目录**，否则报 "could not find native static library `a`"。
- **vulkan feature 两处构建坑**（均已修复并在 win32/MSVC 端到端验证，RTX 4060 +
  qwen3_asr 转录通过）：
  1. **Windows 路径长度**：ggml 的 vulkan-shaders-gen ExternalProject 把构建目录
     嵌套到 `out/build/ggml/src/ggml-vulkan/vulkan-shaders-gen-prefix/src/...`，
     叠加 OUT_DIR 前缀后路径超 ~250 字符，MSVC `cl.exe` 报 C1083、`rc.exe` 在
     manifest 嵌入环节报 `RC2136: missing '=' in EXSTYLE=<flags>`。build.rs 会在
     `OUT_DIR/build` + 嵌套深度超过 240 时，用 cmake crate 的 `out_dir()` 把 CMake
     构建目录重定向到系统临时目录的 `acb<12hex>`（按 OUT_DIR 哈希唯一），下游无需
     手动设置 `CARGO_TARGET_DIR`；
  2. **Vulkan loader 链接**：ggml-vulkan 是静态库，PRIVATE 的 `Vulkan::Vulkan`
     依赖不传导到最终可执行文件（与 CUDA 同理），否则链接报 LNK2019 无法解析的
     `vkGetInstanceProcAddr` 等符号。build.rs 在启用 `vulkan` 时用
     `VULKAN_SDK` 环境变量或 `C:/VulkanSDK/v*` 定位 SDK `Lib`，显式输出
     `vulkan-1`（Linux 为 `vulkan`）链接。
- `audio-cpp` 高层安全 API 已实现（Registry / Model / Session，离线 + 流式），
  并有 3 个示例验证运行通过：`audio-cpp-sys/examples/inspect`、
  `audio-cpp-sys/examples/vad_offline_ffi`、`audio-cpp/examples/vad_offline` 与
  `audio-cpp/examples/vad_streaming`。注意：两个 crate 的示例输出文件名会在
  `target/debug/examples` 冲突，不要给不同 crate 的示例取同名。
- 流式会话（silero_vad）每块必须恰好 `preferred_audio_chunk_samples`（512）个采样，
  末尾不足块需补零；其流式事件经 `is_final` 汇总，single-chunk 调试时无逐块事件也正常。
- 内置 VAD 已双模型验证：silero_vad（离线+流式）与 marblenet_vad（仅离线）。
  marblenet_vad 是 NeMo checkpoint，引擎自动探测会误判成 silero_vad（报
  "missing tensor: stft_conv.weight"），必须显式传 `family_hint="marblenet_vad"`；
  其阈值选项键是 `threshold`（silero 用 `vad_threshold`）。
- ASR 已用 Citrinet ASR Q8_0 GGUF 验证（`audio-cpp/examples/asr_offline` 跑通，
  sample_16k.wav 转录为 Nature 台词）。Citrinet 不在默认 core-models 集，需
  `custom-models` 或 `full-models`；custom 只需 `$env:AUDIOCPP_MODELS="citrinet_asr"`。
  **GGUF 同样无法自动探测族别**，须显式 `family_hint="citrinet_asr"`（否则误判
  silero_vad 报 missing tensor）。
- 上游 CMake 支持 `AUDIOCPP_MODEL_SET=custom` + `AUDIOCPP_MODELS`（逗号分隔
  model targets）按需编译，避免 full 全量 44+ 模型族的编译成本；引擎核心 +
  内置 VAD 始终编入。build.rs 的 `custom-models` feature 透传该机制。
- 请求 JSON 里的 `audio_path` 若为 Windows 路径，反斜杠必须转义（`\\`），
  `\a` 等非法转义会导致 shim 解析失败（"failed to parse json"），改用正斜杠最省事。
- TTS 已用 MOSS-TTS-Nano-100M Q8_0 GGUF 验证（`audio-cpp/examples/tts_offline` 跑通，
  合成 ~8s 语音写入 WAV）。MOSS 由 CMake target `moss` 提供，custom 用
  `AUDIOCPP_MODELS=moss_tts_nano`（或 moss_tts_local，同 target）；其 GGUF 同样须
  显式 `family_hint="moss_tts_nano"`；输出音频 48kHz 2ch 交错 f32。
- **C ABI 音频回传**：`dump_audio_buffer` 现把实际采样以 `samples`（f32 数组）字段
  并入 JSON 的 `audio_output` / `named_audio_outputs`；高层 `AudioBufferInfo.samples`
  为 `Option<Vec<f32>>`，VAD/ASR 不携带（空数组）。改此 ABI 需同步 capi.cpp /
  高层 types.rs，两端 serde 均向后兼容（未知字段忽略）。
- **模型组合切换需重配置**：build.rs 用 `always_configure(true)`，切换
  `AUDIOCPP_MODELS` 后重新 configure 以更新 registry loader 集合；否则会沿用旧的
  model 组合（如刚才 citrinet 组合不含 moss）。
- 说话人分离已用 SortFormer Diar 4spk Q8_0 GGUF 验证（`audio-cpp/examples/diar_offline`
  跑通，four_speaker_short.wav 正确分出 SPEAKER_00..03 各约 4-5 秒发言）。
  SortFormer 由 CMake target `sortformer_diar` 提供，feature 为 `model-sortformer-diar`
  （无需 env）；GGUF 须显式 `family_hint="sortformer_diar"`；**采样率以 GGUF 内嵌
  `processor_config.json` 为准（16000Hz）**，与 GGUF 顶层 feature KV（24000）可能不一致，
  引擎用前者校验、不重采样，24kHz 的 four_speaker_short.wav 需先重采样到 16kHz；
  输出在 `TaskResult.speaker_turns: Vec<SpeakerTurn>`（speaker_id/span/confidence），
  上游 postprocess 恒填 confidence=0.0（SortFormer 无逐段置信度），属正常。
  C ABI `dump_task_result` 已导出 speaker_turns。
- 音乐源分离已用 HTDemucs Q8_0 GGUF 验证（`audio-cpp/examples/sep_offline` 跑通，
  6s 合成立体声混音正确分离出 drums/bass/other/vocals 四轨，能量分布合理）。
  HTDemucs 由 CMake target `demucs`（alias `htdemucs`）提供，feature 为 `model-demucs`
  （无需 env）；GGUF 须显式 `family_hint="htdemucs"`；**输入须 44100Hz 立体声**
  （不隐式重采样，mono 会自动复制为双声道）；task 字符串为 `sep`（C ABI 透传，
  Rust `TaskKind::SourceSeparation`，**不是** `source_separation`）；输出在
  `TaskResult.named_audio_outputs`（id 为 drums/bass/other/vocals，f32 交错）；
  输出时长会比输入略长（htdemucs 的 overlap-add chunk 对齐扩展）。
- **family_hint 类型化**：`Registry::load` 的 `family_hint` 参数已改为
  `Option<ModelFamily>`（`audio-cpp/src/types.rs`），枚举收录全部上游 loader 族，
  `as_str()` 给出传给 C 边界的字符串；未收录族用 `ModelFamily::Custom(String)`
  兜底，`From<&str>` 自动收编已知名字。改枚举需与上游 `CMakeLists.txt` 的
  `make_*_loader` 清单保持同步。
- **流式 ASR 已用 Qwen3 ASR Q8_0 GGUF 验证**（`audio-cpp/examples/asr_streaming`
  跑通，sample_16k.wav 输出逐 3s 窗口部分转录 + 最终文本）。Qwen3 ASR 由 CMake
  target `qwen3_asr` 提供，feature 为 `model-qwen3-asr`；GGUF 须显式
  `family_hint="qwen3_asr"`。**流式 `start` 请求必须带 `audio_path`（或 `audio`
  对象）建立音频契约**，否则 prepare 报 "Qwen3 ASR prepare() requires an audio
  contract"；其 `streaming_policy` 的 `preferred_audio_chunk_samples` 为 0，分块
  大小应按 `preferred_audio_chunk_seconds × sample_rate` 换算；窗口边界经
  `audio_chunk_seconds` 选项控制（默认 30s），短音频无逐块事件属正常。
- **流式 TTS（VoxCPM2）示例**：`audio-cpp/examples/tts_streaming`，模型族
  `voxcpm2`（custom 用 `AUDIOCPP_MODELS=voxcpm2`）。VoxCPM2 流式输入为 `none`
  （不消费音频块），`start` 带 text 即整段合成，逐块音频经事件回调送出、
  `finish()` 返回合并音频；**C ABI `dump_stream_event` 已补 `named_audio_outputs`
  字段**（流式 TTS 逐块事件），高层 `StreamEvent.named_audio_outputs` 同步新增。
  **流式 `start` 请求必须带 `options.retry_badcase=false`**（上游硬约束，缺省会报
  "VoxCPM2 streaming generation requires retry_badcase=false"）。
- **离线 Qwen3 TTS 声音克隆示例**：`audio-cpp/examples/tts_offline_qwen3`，模型族
  `qwen3_tts`（custom 用 `AUDIOCPP_MODELS=qwen3_tts`）。Qwen3 TTS **base 变体
  必须做 voice-clone**——上游 `make_request` 会把请求里的 `audio_path`（或 `audio`
  对象）读作参考人声（无需额外 C ABI），参考音频文本转写经
  `options.reference_text` 传入，越贴合内容音色越准；GGUF 须显式
  `family_hint="qwen3_tts"`。已验证：`qwen3-tts-12hz-0.6b-base-q8_0.gguf` + upstream
  `sample_16k.wav`（Nature 台词做参考）合成 ~4s 语音，输出 24000Hz 1ch。
- **本地测试权重**可放在任意本地目录（qwen3-asr-0.6b-q8_0.gguf、
  fun-asr-nano-2512-q8_0.gguf、voxcpm2-q8_0.gguf、
  qwen3-tts-12hz-0.6b-base-q8_0.gguf 等），文档中一律用相对路径占位，
  **不要写入真实的本地绝对路径**（如盘符/用户名），避免泄露个人电脑信息。
- **预编译旁路**：设置 `AUDIOCPP_PREBUILT_DIR=<目录>` 后 build.rs 跳过整个 CMake
  构建，直接链接该目录下的 `engine_runtime` 及依赖静态库（布局 `<dir>`、`<dir>/lib`、
  `<dir>/lib64`、`<dir>/bin` 均可，与 llama-cpp-rs 的 `LLAMA_PREBUILT_DIR` 一致）。
  C shim 与绑定仍从源码编译，故仍需 `ensure_audio_src()` 源码树。目录须与当前
  feature 组合（模型集 + 后端）匹配；CUDA/Vulkan 预编译仍要本地 SDK 链接。
  capi/bindgen/平台链接逻辑已抽为 `compile_capi_shim` / `generate_bindings` /
  `emit_platform_links` 供两路径共用。设计见 docs/prebuilt_pattern_report.md。
- **`prebuilt` feature 自动下载**：`audio-cpp-sys/prebuilt_download.rs` 按当前
  平台/后端/模型组合拼资产名（`audio-cpp-prebuilt-{os}-{target}-{backend}-{modelset}-static.tar.gz`），
  从 GitHub Releases 下载并缓存到 `target/audio-cpp-prebuilt-cache/<tag>/`。
  backend 由 feature 推导（cpu/vulkan/metal；cuda/hip 不发预编译）。**只发布
  `full` 全模型资产**（full 是任何 model 组合的超集）：core / custom-<族> 资产
  404 时自动回退下载 full 资产（`ensure_prebuilt` 的 superset 回退），仍失败才
  回落源码。归档 `metadata.json.audio_commit` 与本地 submodule HEAD 不符则删
  缓存回落源码（防 ABI 错配）；`metadata.json.msvc_ver` 为 CI 构建时的
  `_MSC_VER`，本地工具集版本偏低则回落源码（MSVC 静态库绑定工具集版本）。
  env：`AUDIOCPP_PREBUILT_TAG`（tag，默认 `v{version}`）、
  `AUDIOCPP_PREBUILT_REPO`、`AUDIOCPP_PREBUILT_URL`（完整地址覆盖，也可含
  `{tag}`/`{asset}` 占位符供内网镜像，`file://` 前缀指本地归档直接复制）、
  `AUDIOCPP_PREBUILT_OFF`（禁用自动下载）。网络抖动重试 3 次带退避（404 等
  4xx 为确定性失败不重试），仍失败回落源码。`AUDIOCPP_PREBUILT_DIR` 显式目录
  优先级高于自动下载。
- **CI 预编译资产**：`.github/workflows/prebuilt-audio-cpp.yml` 在 `v*` tag 或
  workflow_dispatch 时构建（linux 3 / windows 2 / macos 1 矩阵，full ×
  cpu/vulkan/metal），用 `.github/scripts/collect-unix-prebuilt.sh` 与
  `collect-windows-prebuilt.sh` 从 `target/**/out`（及 Windows 长路径重定向的
  `%TEMP%\acb*`）收集静态库打包，写 `metadata.json`（含 `msvc_ver`），经
  `gh release upload` 上传。Windows vulkan 用
  `.github/actions/setup-vulkan-sdk-windows`（LunarG SDK）。资产命名须与
  `prebuilt_download.rs::asset_name()` 保持一致。
- **MSVC `crt-static`（静态 CRT）**：消费端开 `-C target-feature=+crt-static`
  后，Rust 侧 std 与 cc 编译的 C shim（capi.o）均为 `/MT`，而 CMake 默认（及
  预编译资产）是 `/MD`，混链接报 LNK2038（RuntimeLibrary 不匹配）+ LNK2019
  （`__imp_*` 无法解析）。build.rs 检测 `CARGO_CFG_TARGET_FEATURE` 含
  `crt-static` 时：跳过预编译（资产为 `/MD`，强制回退源码构建），并给 CMake
  注入 `CMAKE_POLICY_DEFAULT_CMP0091=NEW` + `CMAKE_MSVC_RUNTIME_LIBRARY`
  （`MultiThreaded`/`MultiThreadedDebug`，按 `AUDIOCPP_LIB_PROFILE` 取）全目标
  `/MT`——注意 sentencepiece 等子目录 cmake_minimum_required 低（3.5），
  不显式设 `CMAKE_POLICY_DEFAULT_CMP0091=NEW` 不会继承 `/MT`。已用
  `aduio_cpp_ceshi`（`model-qwen3-asr,vulkan`）验证：源码构建 + crt-static 跑通。
  **注意：MSVC 的 OpenMP 运行时只有 DLL 版（`vcomp140.dll`，无静态库），且
  ggml 的 `GGML_OPENMP` 默认 ON 且未接到 `ENGINE_ENABLE_OPENMP`**——不显式设
  `GGML_OPENMP=OFF` 时即便 cargo 未开 `openmp` feature，ggml 内部仍会 `/openmp`
  编译并让产物依赖 vcomp140.dll（在无 MSVC 运行库的沙箱/目标机直接报
  "vcomp140.dll 找不到"）。build.rs 已让 `GGML_OPENMP` 与 `ENGINE_ENABLE_OPENMP`
  同步，并在 crt-static 下无条件关闭 OpenMP（开 `openmp` feature + crt-static
  会警告并强制关闭）。crt-static + vulkan 的静态二进制仅剩 `vulkan-1.dll`（后端
  必需）与系统 DLL。注意：crt-static 路径无预编译加速，需完整源码构建。

## 常用命令速查
```bash
git submodule update --init --recursive                                  # 补齐 audio-cpp-sys/audio.cpp（克隆后必需）
git -C audio-cpp-sys/audio.cpp pull --ff-only                             # 单独更新 submodule 到上游最新
cargo build -p audio-cpp-sys                                              # 单独构建底层
BUILD_DEBUG=1 cargo build                                                 # 调试构建脚本
AUDIOCPP_PREBUILT_DIR=<预编译目录> cargo build                            # 跳过 CMake，直接用预编译静态库
```