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
- **本地测试权重**可放在 `F:\models\`（qwen3-asr-0.6b-q8_0.gguf、
  fun-asr-nano-2512-q8_0.gguf、voxcpm2-q8_0.gguf、
  qwen3-tts-12hz-0.6b-base-q8_0.gguf 等），示例文档统一引用该目录。

## 常用命令速查
```bash
git submodule update --init --recursive                                  # 补齐 audio-cpp-sys/audio.cpp（克隆后必需）
git -C audio-cpp-sys/audio.cpp pull --ff-only                             # 单独更新 submodule 到上游最新
cargo build -p audio-cpp-sys                                              # 单独构建底层
BUILD_DEBUG=1 cargo build                                                 # 调试构建脚本
```