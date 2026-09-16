# AGENTS.md

本文件为在此仓库中工作的 AI 代理提供项目上下文与约定。

## 项目概览

`audio-cpp-rs` 是把 C++ 音频推理框架 [audio.cpp](https://github.com/0xShug0/audio.cpp) 封装为 Rust 库的项目。目标是**共享 C++ 运行时实现**，而非重写。

工作区（resolver = 3，edition 2024）：

- **`audio-cpp-sys/`** — 底层 FFI crate。`build.rs` 负责全部本地构建：
  1. CMake + **Ninja** 构建上游 `engine_runtime` 静态库（submodule at `audio-cpp-sys/audio.cpp`）；
  2. `cc` 编译 C shim（`capi.h` / `capi.cpp`）；
  3. `bindgen` 生成绑定，输出到 `OUT_DIR/bindings.rs`，由 `src/lib.rs` 用 `include!` 引入。
- **`audio-cpp/`** — 高层安全封装 crate（Registry / Model / Session，离线 + 流式，
  类型化请求与 serde 类型，见 `audio-cpp/README.md`）。

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
- **中文乱码约定**：本机 pwsh 的 `[Console]::OutputEncoding` 默认是系统代码页（936/GB2312），而 opencode 等工具读 pwsh 输出时按 UTF-8 解码——含中文的命令输出（如 `git log`、`rg`、`cargo` 的警告、`Get-Content`、`Get-ChildItem`）会乱码。**执行含中文输出的命令前**，先设置 UTF-8（四行都设，`$PSStyle.OutputRendering='PlainText'` 可避免 pwsh 7 的 ANSI 样式在管道时干扰编码）：
  ```powershell
  [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
  [Console]::InputEncoding = [System.Text.Encoding]::UTF8
  $OutputEncoding = [System.Text.Encoding]::UTF8
  $PSStyle.OutputRendering = 'PlainText'
  ```
  也可把上述四行写入 pwsh profile（`$PROFILE`）让所有会话自动生效；`Get-Content` 读中文文件建议加 `-Encoding UTF8`。
- 构建命令：
  - `cargo build`（默认 `core-models` + CPU）
  - `cargo build --features full-models,openmp`
  - 验证：从仓库根目录运行 `cargo check` / `cargo build`。build.rs 有 `BUILD_DEBUG=1` 调试日志。

### 5. feature → audio.cpp 映射
| Cargo feature | CMake 选项 |
|---|---|
| `core-models`（默认） | `AUDIOCPP_MODEL_SET=core` |
| `full-models` | `AUDIOCPP_MODEL_SET=full` |
| `custom-models` | `AUDIOCPP_MODEL_SET=custom`（与 `model-*` feature / `AUDIOCPP_MODELS` 取并集） |
| `model-<族>`（40+ 个） | custom 并集中的一项（build.rs 扫描 `CARGO_FEATURE_MODEL_*` 自动映射，无需改 build.rs） |
| `prebuilt` | 启用预编译自动下载（tag 默认 `v{version}`，见预编译节） |
| `cuda` | `ENGINE_ENABLE_CUDA=ON`（与 `hip` 互斥，build.rs 已硬校验） |
| `hip` | `ENGINE_ENABLE_HIP=ON` |
| `vulkan` | `ENGINE_ENABLE_VULKAN=ON` |
| `metal` | `ENGINE_ENABLE_METAL=ON`（Apple 默认；非 Apple 可用 `AUDIOCPP_FORCE_METAL=1` 强制） |
| `openmp` | `ENGINE_ENABLE_OPENMP=ON`（同步 `GGML_OPENMP`；crt-static 下强制关闭） |
| `native` | `ENGINE_ENABLE_NATIVE_CPU=ON` |

### 6. 发布 / 标签 / Release 操作需显式授权
AI 代理**不得擅自**执行以下“对外发布”类操作，除非用户在对话中**明确命令**：
- 发布到 crates.io（`cargo publish`）；
- 创建 / 删除 GitHub Release（含 `gh release create` / `delete` / `upload`）；
- 创建 / 删除 / 推送 git tag（含 `git tag`、`git push --delete <tag>`，以及 `gh release create` 顺带建出的 tag）。
以上操作多为**不可逆**（crates.io 版本无法撤回；tag / release 直接决定预编译资产的寻址与下游消费端），
即便推断“应该”这么做（如版本号已变、需要发布预编译资产等），代理也必须先停下、向用户确认，
由用户决定是否执行。预编译资产归属哪个 release / tag 以用户指定为准，代理不得自行拍板。

## audio.cpp 升级检查清单

每次把 `audio-cpp-sys/audio.cpp` submodule 更新到上游新 commit 后，按下表顺序审查，缺一不可：

1. **定位 diff**：`git -C audio-cpp-sys/audio.cpp diff <旧commit>..HEAD --stat`，先看整体改动面。
2. **新增模型 loader → 同步 `ModelFamily`**：查 `audio.cpp/CMakeLists.txt` 是否新增
   `make_*_loader`。有则同步 `audio-cpp/src/types.rs`：`ModelFamily` 枚举（`as_str()`
   返回的字符串须与上游 loader 族名一致）+ `from_path()` 关键词表 + Cargo.toml 新增
   `model-*` feature（如适用）。若只是现有 loader 的行为/选项变化（如 dots_tts 新增
   edit 推理），枚举无需动，但需评估高层类型/示例是否要暴露新能力。
3. **C ABI 边界**：capi.cpp 只依赖 5 个头（`engine/framework/core/backend.h` /
   `engine/framework/io/json.h` /
   `engine/framework/runtime/{model,registry,session}.h`）。diff 这些头文件：
   公共 API（session/request/types/registry）改了就要同步 capi.h / capi.cpp / build.rs
   bindgen allowlist（`audiocpp_.*`）。新增异常类型必须派生自 `std::exception`
   （shim 统一 `catch (const std::exception&)` 转 `audiocpp_last_error`）；上游新增的
   `engine::runtime::CapacityError`（请求过大应归 400 而非 500）按需在 shim 里映射。
   新 loader 须确认其 session 自持资产（`shared_ptr`），否则高层 `Session` 独立于
   `Model` 存活的保证不成立（见 `audio-cpp/src/lib.rs` 资源生命周期节）。
4. **请求/响应 JSON 结构**：上游 `app/server/runtime.cpp` 或模型 `request.cpp` 新增
   task 类型 / 选项键 / 输出字段时，检查 C ABI `dump_task_result` / `dump_stream_event`
   / `dump_audio_buffer` 是否要补字段，以及高层 `types.rs` 的 serde 结构（两端 serde
   均向后兼容，未知字段忽略，但新增字段要显式加）。
5. **构建验证（必跑）**：
   - `cargo fmt --all -- --check`——CI 卡得最频繁的一项，示例里换行/长行经常不过；
   - `cargo build --workspace`——验证 shim 对新引擎编译链接通过。build.rs 已跟踪
     submodule HEAD 指针会自动触发重编；若输出仍是 0.x 秒未重编，说明跟踪失效需排查；
   - 涉及后端/模型行为变化时按需跑相关示例验证。
 6. **预编译资产**：`metadata.json.audio_commit` 与 submodule HEAD 不一致会强制回落
    源码构建（预编译自动下载被跳过），下游自动退化为源码构建即可，无需代理干预。
    `.github/workflows/prebuilt-audio-cpp.yml` **仅**在推送 `v*` tag 或手动
    `workflow_dispatch` 时构建并上传预编译资产（main 开发期推送**不**触发，submodule
    指针变动也不会触发）。因此「升级 submodule 后重新发布预编译资产」属于第 6 节
    定义的**发布 / Release 操作**，必须由用户显式下达发布命令才处理，代理不得擅自
    触发或执行。已在“已知状态”记录的验证结论随版本变化需复核。

## 已知状态
- 当前 submodule HEAD = `048c9a0c`（origin/main；从
  `e9ff200` 快进）。历史升级记录见下方逐条。
- **升级 `e9ff200`→`048c9a0c`（main）审查结论**：
  - diff 共 66 文件（+3k/-1.5k，21 个提交）：**无新增 `make_*_loader`**
    （81→81，上游 model target 数不变）；其余为 YuE2 LoRA（框架
    `lora_tensor_source` + AR/NAR 运行时 + WebUI 选项）/ 流式 partial 按增量
    发布（新增 `partial_text` 共享发布器，kroko/parakeet/higgs/vibevoice 等
    对齐）/ greedy Qwen decoder 与 Qwen BPE bundle 框架提升（audio8 thinker
    瘦身约 689 行）/ kokoro 英文 G2P 补完 / builtin_audio_utils 显式权重路径
    （下述）/ yue2 ABC 乐谱产物（下述）/ sheetsage2 放宽加载检查 /
    CUDA Turing 原生 SASS / vulkan col2im 修 / server config 与 WebUI 小修 +
    `v0.8.0` 发版。
  - **C ABI 边界无需改动**：`framework/core/backend.h` /
    `framework/io/json.h` / `framework/runtime/{model,registry}.h` 本次
    diff **零改动**；`framework/runtime/session.h` 仅加法字段
    `TaskRequest::option_arrays` / `SessionPreparationRequest::option_arrays`
    （上游 opt-in C API `audiocpp_request_set_option_array` 的运行时载体，
    上游明确尚无 spec 使用 `*_list` 选项类型）；`spec_backed_model.h` 新增
    数组键校验重载，既有调用点不动。`capi.h` / `capi.cpp` / build.rs bindgen
    allowlist 保持原样。
  - 无新 task 类型 / 输出字段：`task_vocabulary` 未动；yue2 新增 `score`
    ABC 文本产物（kind `Custom` + `meta`：mime/format/extension/source/
    truncated）走既有 `output_artifacts`（shim 与高层 `VoiceArtifact.meta`
    早已支持）；流式 `partial_text` 改增量语义（同名字段，客户端按追加拼接，
    此前 parakeet 发全量导致二次拼接错误）。高层 `types.rs` serde 结构无需改。
  - 本仓同步：`ModelFamily` 无需动（无新 loader 族）；仅修
    `BuiltinAudioUtils` 文档——`model_path` 改传权重文件或其所在目录
    （权重另行下载，非内嵌）+ 必经 `load_options` 传
    `{"utility":"<工具 ID>"}`（`Registry::load` 早已支持 load_options 透传；
    `from_path()` 关键词对目录路径仍有效）。另 yue2 请求/会话选项经通用
    字符串透传，无需 Rust 改动（`guidance_scale` 新增、`cfg_scale` 保留兼容；
    删 `semantic_codes_file`；新增会话选项 `yue2.lora` / `yue2.lora_scale` /
    `yue2.attention`；默认 seed/步数变更；yue2 与 sheetsage2 状态升为
    `supported`）。
  - 验证：`cargo fmt --check` + `cargo build --workspace`（build.rs 跟踪
    submodule HEAD 正常触发增量重编，新 TU `lora_tensor_source` /
    `partial_text` / `greedy_qwen_decoder` / `qwen_bpe_bundle` 已编入，
    `engine_runtime.lib` 重链接）+ `cargo test --workspace`
    （31 lib + 12 doc 全过）+ `clippy --workspace --all-targets` 零警告。
    新旧 session 均自持资产（builtin session 按值持有 `UtilityRuntime` 不变；
    yue2 持 `shared_ptr<const Yue2Assets>` 不变），`Session` 独立于 `Model`
    存活的保证成立。
  - 注意：`metadata.json.audio_commit` 与新 submodule HEAD 不一致会强制回落
    源码构建（预编译自动下载被跳过），属预期；重新发布预编译资产须用户显式
    下达发布命令（第 6 节），本次未执行。
- **升级 `1bab32a`→`e9ff200`（main）审查结论**：
  - diff 共 106 文件（+13.7k/-303，约 20 个提交）：新增 2 个 CMake target
    `yue2`（族 `yue2`，YuE2 音乐生成，task `gen` 仅离线，纯文本请求
    lyrics+style，不消费音频，输出 `audio_output`）/ `sheetsage2`
    （族 `sheetsage2`，SheetSage2 音频转 ABC 谱，task `midi` 仅离线，
    音频输入，输出复用既有 `artifact_output`/`output_artifacts`）；其余为
    Yue2 AR/NAR 运行时与 Oobleck VAE 框架 codec、Qwen chunked prefill +
    后端 KV 复用、encoder-decoder 框架模块、task vocabulary 收敛（下述）、
    各模型 safetensors 文件清单补全、server/UI（Yue2 面板）等。上游 model
    target 共 77 个（+2），`make_*_loader` 78→80。
  - **C ABI 边界无需改动**：capi.cpp 依赖的 5 个头（`framework/core/backend.h` /
    `framework/io/json.h` / `framework/runtime/{model,registry,session}.h`）本次
    diff **零改动**（`session.cpp` 的 `to_string`/`parse_voice_task_kind` 改调
    新表，`kv_cache.h` 加批内状态查询，均为内部加法）。`capi.h` / `capi.cpp` /
    build.rs bindgen allowlist 保持原样。
  - 无新 task 类型：14 种不变，仅把四处手写 task 名单收敛为
    `framework/runtime/task_vocabulary.{h,cpp}` 单一真源（顺带修了
    `miocodec.json` 的非法 `"codec"` 与 `moss_voicegen.json` 的 `"vdes"`→
    `"design"` 别名；上游 opt-in C API `include/audiocpp.h` 新增任务枚举与
    `audiocpp_request_set_text_language`，与本仓 shim 无关）。
    高层 `types.rs` 的 `TaskKind` / serde 结构无需改。
  - 已同步 `audio-cpp/src/types.rs` 的 `ModelFamily`：新增 2 个枚举变体
    `Yue2`（`as_str()` → `"yue2"`）/ `Sheetsage2`（→ `"sheetsage2"`）+
    `from_path()` 关键词表 + `From<&str>`；并在 `audio-cpp-sys/Cargo.toml` 与
    `audio-cpp/Cargo.toml` 新增 `model-yue2` / `model-sheetsage2` feature
    （build.rs 自动映射 CMake target），另补上此前漏建的
    `model-qwen3-forced-aligner`（G3 关闭：Qwen3 对齐族早有 CMake target，
    只是缺单选 feature）；full-models 注释 75→77。
    `model_family_roundtrip` 等测试已覆盖新变体。
  - 本次顺带关闭 voxkit 缺口 G1–G6（证据均在上游源码核实，非推测）：
    G1 `Request::align(audio, text, language)`（shim 早已解析顶层
    `text`/`language`→`text_input`，纯 Rust 侧新增，无需动 C ABI）；
    G2 `TaskResult::word_timestamps` 文档写明 Qwen3 ASR 需
    `return_timestamps=true` + 会话选项 `qwen3_asr.forced_aligner_model_path`
    （流式不支持），空数组先查选项；
    G4 新增 `audio_cpp::options` 模块（核实过的请求/会话选项键常量 +
    `SessionOptions` builder），并修掉传了很久的死键 `vad_threshold`
    （上游从未存在，silero/marblenet 阈值键都是 `threshold`）；
    G5 `Registry`→`Model`→`Session` 改为内部 `Arc` 共享所有权
    （`Send` 不变、`!Sync` 不变），调用方可及时释放上游持有者
    （`model_outlives_registry` 回归测试锁定）；
    G6 对齐语言码写入 `Request::align` 文档（Qwen3 11 语种 / MMS 仅 nl-en
    拉丁 + `pre_romanized`，上游无 `auto` 档）。
  - 验证：`cargo fmt --check` + `cargo build --workspace`（build.rs 跟踪
    submodule HEAD 正常触发增量重编）+ `cargo test --workspace`
    （31 lib + 12 doc 全过）+ `clippy --workspace --all-targets` 零警告 +
    `vad_offline`（silero 内置权重 + sample_16k.wav）端到端通过。
    新族 session 均自持资产（yue2 持 `shared_ptr<const Yue2Assets>`；
    sheetsage2 持 assets + contract），`Session` 独立于 `Model` 存活的
    保证成立。
- **升级 `78d4770`→`1bab32a`（main）审查结论**：
  - diff 共 116 文件（+18.4k/-162，29 个提交）：新增 4 个 CMake target
    `kokoro_tts`（族 `kokoro_tts`，Kokoro 82M 多语 TTS，仅离线）/
    `moonshine_asr`（族 `moonshine_asr`，Moonshine 流式 ASR，离线+流式）/
    `niagara_asr`（族 `niagara_asr`，ABR Niagara 英文批处理 ASR，仅离线）/
    `builtin_audio_utils`（族 `builtin_audio_utils`，内置降噪/增强/超分工具，
    task `s2s` 仅离线）；其余为 Orukeet r3 权重变体（仍属 `parakeet_tdt` 族，
    无新 loader）、parakeet 长音频 session 修复、VibeVoice 流式 flash 注意力
    性能优化、server 前端模块解耦（`server_frontends.cmake` + 可选 HTTPS
    前端监听器）/转录端点调试日志/Sortformer v2 本地包路径修复、上游 opt-in
    共享库 C API（`include/audiocpp.h` + `src/capi`，`AUDIOCPP_BUILD_C_API`
    默认 OFF，与本仓 `capi.h` shim 无关）等。上游 model target 共 75 个（+4）。
  - **C ABI 边界无需改动**：capi.cpp 依赖的 5 个头（`framework/core/backend.h` /
    `framework/io/json.h` / `framework/runtime/{model,registry,session}.h`）本次
    diff **零改动**。`framework` 下其余变更均为内部加法：`runtime/kv_cache.h`
    新增 ring 模式选项（`ring_mode` / `ring_pinned_steps`，默认关闭）+
    `slot_for_position()` + 自由函数 `ring_stored_position()`；
    `modules/weight_binding.h` 新增 `linear_from_transposed_named_source`
    模板重载（原 `hf_conv1d_linear_from_source` 转调它，行为不变）；
    `audio/utility_api.h` 新增内置工具清单 API（`list/find/require/
    resolve_builtin_audio_utility` + `default_audio_utility_assets_root`），
    shim 从不依赖它。`capi.h` / `capi.cpp` / build.rs bindgen allowlist 保持原样。
  - 无新 task 类型 / 输出字段：4 新族均用既有 `tts` / `asr` / `s2s`
    （`builtin_audio_utils` capabilities 为 `s2s/audio_enhancement`）；
    server `runtime.cpp` diff 仅为前端转接层 + 转录调试日志，无新端点/字段；
    高层 `types.rs` serde 结构无需改。新选项（kokoro `weight_type` /
    `text_chunk_size` / `seed`、moonshine `max_tokens`、niagara `language` /
    `audio_chunk_mode`）全部走通用 options 字符串透传，无需 Rust 改动。
  - 已同步 `audio-cpp/src/types.rs` 的 `ModelFamily`：新增 4 个枚举变体
    `KokoroTts`（`as_str()` → `"kokoro_tts"`）/ `MoonshineAsr`
    （→ `"moonshine_asr"`）/ `NiagaraAsr`（→ `"niagara_asr"`）/
    `BuiltinAudioUtils`（→ `"builtin_audio_utils"`，文档注明 task `s2s` +
    `model_path` 直接传工具 ID）+ `from_path()` 关键词表（含工具 ID
    `deepfilternet` / `rnnoise` / `zipenhancer` / `gtcrn` / `flashsr` →
    `BuiltinAudioUtils`，`Registry::load(model_path: &str)` 传 ID 字符串即可，
    Rust 侧无需改）+ `From<&str>`；并在 `audio-cpp-sys/Cargo.toml` 与
    `audio-cpp/Cargo.toml` 新增 `model-kokoro-tts` / `model-moonshine-asr` /
    `model-niagara-asr` / `model-builtin-audio-utils` 四个 `model-*` feature
    （build.rs 自动映射 CMake target）；full-models 注释 72→75。
    `model_family_roundtrip` 等测试已覆盖新变体（26 lib + 9 doc 全过）。
  - 验证：`cargo fmt --check` + `cargo build --workspace`（增量重编约 1.5 分钟，
    `engine_runtime` 链接通过；4 新族不在 core 集，默认构建不编译它们，
    与既往升级一致）+ `cargo test --workspace`（26 lib + 9 doc 全过）+
    `clippy --workspace --all-targets` 零警告。新族 session 均自持资产
    （kokoro/moonshine/niagara 持 `shared_ptr` assets；builtin session 按值
    持有 `UtilityRuntime`），`Session` 独立于 `Model` 存活的保证成立。
  - 注意：`registry.cpp` 的 `validate_request` 放宽为 family_hint 为
    `builtin_audio_utils` 且 `model_path` 为已知工具 ID 时免路径存在性校验，
    属上游配合该族“ID 即路径”的设计，无 ABI 影响。
- **升级 `f6277c1`→`78d4770`（main）审查结论**：
  - diff 共 136 文件（+10.8k/-0.8k，26 个提交）：新增 1 个 CMake target
    `sortformer_diar_v2`（族 `sortformer_diar_v2`，Sortformer v2.1 流式说话人分离，
    离线+流式）+ `vibevoice_asr` target 新增第二 loader
    `make_vibevoice_asr_streaming_loader`（族 `vibevoice_asr_streaming`，
    VibeVoice ASR Streaming 7B，离线+流式）；其余为 sanotts 增声/mira 加速/
    BreezeTTS 流式收紧/VeVo2 音频分块/VoxCPM2 无状态分块策略/Fish HIP fast-sampler
    改独立 HIP target/GTCRN 降噪工具/eSpeak-ng 共享音素器/server 分块传输体解析等内部改进。
    上游 model target 共 71 个（+1）。
  - **C ABI 边界无需改动**：`framework/runtime/*`、`framework/io/json.h`、
    `framework/core/backend.h` 本次 diff **零改动**（framework 下仅新增
    `audio/{espeak_phonemizer,espeak_data,gtcrn}.{h,cpp}` 内部工具 +
    `utility_api.cpp` 的 GTCRN 降噪模型分发（字符串透传，无 ABI 影响））。
    `capi.h` / `capi.cpp` / build.rs bindgen allowlist 保持原样。
  - 无新 task 类型 / 输出字段：两新族均用既有 `diar` / `asr`（及既有流式 ABI）；
    高层 `types.rs` serde 结构无需改。
  - 已同步 `audio-cpp/src/types.rs` 的 `ModelFamily`：新增 2 个枚举变体
    `SortformerDiarV2`（`as_str()` → `"sortformer_diar_v2"`）/
    `VibevoiceAsrStreaming`（`as_str()` → `"vibevoice_asr_streaming"`）+
    `from_path()` 关键词表（流式/新变体置于其前缀家族之前，避免子串误判）+
    `From<&str>`；并在 `audio-cpp-sys/Cargo.toml` 与 `audio-cpp/Cargo.toml`
    新增 `model-sortformer-diar-v2` feature（build.rs 自动映射 CMake target）。
    注意：`vibevoice_asr_streaming` **不**是独立 CMake target/alias（与
    `vibevoice_asr` 同 target），故**不**新增 `model-vibevoice-asr-streaming`
    feature——用既有 `model-vibevoice-asr` 即可编入其 loader（否则 CMake 报
    Unknown AUDIOCPP_MODELS entry），枚举变体文档已注明。
  - 验证：`cargo fmt --check` + `cargo build --workspace`（build.rs 跟踪
    submodule HEAD 正常触发 CMake 重编，`engine_runtime` 链接通过）+
    `cargo test --workspace`（26 lib + 9 doc 全过）+ `clippy --workspace
    --all-targets` 零警告。新增 `audio-cpp/examples/asr_streaming_vibevoice.rs`
   （仿 `asr_streaming`，`model-vibevoice-asr` feature，0.25s 分块偏好 + start
    带 `audio_path` 建契约 + `language` 选项；编译 + clippy 通过，无本地权重，
    端到端待补）。`sortformer_diar_v2` 暂无示例（无公开 GGUF，需本地转 `.nemo`）。
  - 全面复核（26 commit 逐项）确认无遗漏：`make_*_loader` 新增确为上述 2 个；
    sanotts/mira/pocket/qwen/attention/flashsr 均无新选项键；Vevo2 音频分块
    （`audio_chunk_duration_sec`/`cross_fade_duration_sec`）、VoxCPM2
    `chunk_strategy=stateless`、BreezeTTS 流式（`stream_frames_per_event`/
    `stream_lookahead_margin`，且 streaming 增量默认开）的新选项全部走通用
    options 字符串透传，无需 Rust 改动；vibevoice `audio_chunk_seconds` 改名
    框架侧保留旧键兼容（`chunking.cpp` 别名链），无破坏；eSpeak 共享化后默认
    仍动态加载（`AUDIOCPP_STATIC_ESPEAK` 默认 OFF 且涉 GPL 静态链接义务，不启用）；
    Fish HIP 独立 target 仅影响 HIP 构建（`emit_hip_links` 已覆盖运行时链接）。
    新能力（需权重，均经既有 ABI 可达）：`sortformer_diar_v2` 离线+流式
    diar（mono 16kHz，无公开 GGUF，需本地转 `.nemo`，NVIDIA 许可）/
    `vibevoice_asr_streaming` 离线+流式 ASR（公开 GGUF
    `audio-cpp/VibeVoice-ASR-Streaming-7B-GGUF`，流式偏好 0.25s 分块，
    `prepare` 需 audio contract，无 timestamps 输出）/ `breeze_tts` 新增
    streaming 模式。GTCRN 降噪仅 server/CLI 可达，capi 从未暴露 audio-utility
    API（deepfilternet/rnnoise 同理），属既有缺口非本次回归，未来要暴露需新 C ABI。
- **升级 `2269821`→`f6277c1`（main）审查结论**：
  - diff 共 214 文件（+33.9k/-1k，34 个提交），涉及 6 个新 loader 族 +
    PocketTTS 流式 + server 转录详情端点 + ggml INT8/ternary（VibeASR）：
    `audio.cpp/CMakeLists.txt` 新增 `make_sanotts_loader`（sanoTTS 社区 TTS，
    7 声音 en/vi/id）/ `make_sopro_tts_loader`（Sopro V2 Turbo TTS，离线+流式）
    / `make_mira_tts_loader`（MiraTTS 社区 TTS）/ `make_cosyvoice3_loader`
    / `make_breeze_tts_loader`（BreezeTTS，含 voice design）/
    `make_vibeasr_loader`（VibeASR 社区 INT8/ternary ASR）。上游 loader 清单共 72 个。
  - **C ABI 边界无需改动**：capi.cpp 依赖的 5 个头（`framework/core/backend.h` /
    `framework/io/json.h` / `framework/runtime/{model,registry,session}.h`）本次
    diff **零改动**（framework 下仅新增内部 `core/attention_fallback.h`）。
    `capi.h` / `capi.cpp` / build.rs bindgen allowlist 保持原样。
  - 无新 task 类型 / 输出字段：新模型均用既有 `tts` / `asr`（breeze 另用既有
    `clon` / `vdes`）；server 新增 `/v1/audio/transcriptions/details` 只是把
    既有 segments/speaker_turns/words 字段复用到新 HTTP 端点，`dump_task_result`
    早已导出这些字段；PocketTTS 新增流式走既有流式 ABI。
  - 已同步 `audio-cpp/src/types.rs` 的 `ModelFamily`：新增 6 个枚举变体
    `Sanotts` / `SoproTts` / `MiraTts` / `Cosyvoice3` / `BreezeTts` /
    `Vibeasr`（`as_str()` → `sanotts` / `sopro_tts` / `mira_tts` /
    `cosyvoice3` / `breeze_tts` / `vibeasr`，与上游 loader 族名一致）+
    `from_path()` 关键词表 + `From<&str>`（含上游别名 `sopro` /
    `sopro_v2` / `sopro_v2_turbo` / `mira` / `MiraTTS`）；并在
    `audio-cpp-sys/Cargo.toml` 与 `audio-cpp/Cargo.toml` 新增
    `model-sanotts` / `model-sopro-tts` / `model-mira-tts` /
    `model-cosyvoice3` / `model-breeze-tts` /     `model-vibeasr` 六个
    `model-*` feature（build.rs 按 `model-<target>` 约定自动映射 CMake
    target，无需改 build.rs）。`model_family_roundtrip` 等测试已覆盖新变体
    （23 项全过）。
- **补记（历史漏记）**：`Audio8Asr`（`as_str()` → `"audio8_asr"`，含 `arkasr`
  别名）+ `model-audio8-asr` feature 在代码侧早已收录，但从未出现在升级历史中。
  经核对上游 `eb82b18 feat: Add Audio8-ASR-0.1B community model port (#337)`
  早于 `ee7be93` 即已合入，系 `d2ff370`→`ee7be93`（或更早）审查时漏记，功能本身
  无缺失，本次核对确认无误。
- **全仓审查修复**（f6277c1 之后未动 submodule，纯本仓改动）：4 组并行审查共
  报告 30+ 项，已复核并全部修复。含 3 个高严重度：流回调加 `catch_unwind`
  （`session.rs`）、`audiocpp_audio_load_wav` 全函数 try/catch + 格式/大小校验
  （`capi.cpp`）、macOS 默认构建请求 `metal` 预编译资产（`prebuilt_download.rs`）；
  破坏性变更仅 2 处：`Session::reset()` / `StreamingSession::reset()` 改返回
  `Result<(), Error>`、`Request::stream()` 改名 `stream_asr()`（旧名 deprecated
  保留）；`supports_family` 的 `bool` 语义不变（出错仍为 false，另新增
  `try_supports_family` 区分出错）。
  验证：`cargo fmt --check` + `clippy --all-targets` 零警告 + `cargo build --workspace`
  + `cargo test --workspace`（26 lib + 9 doc）+ `vad_offline` / `vad_streaming`
  示例端到端通过。另实测 `prebuilt` 下载链路：404 即标 `[non-retryable]` 直接
  回落（无重试拖延），资产名拼写正确（v0.4.0 尚无 Release，故回落源码构建，符合预期）。
- **升级 `3497b7c`→`2269821`（main）审查结论**：
  - diff 共 45 文件（+3.7k/-84），涉及 1 个新 loader 族 + Chatterbox Turbo TTS
    社区模型 + Qwen3 ASR 标点输出 + Fish Audio HIP Fast-AR + alignment 端点：
    `audio.cpp/CMakeLists.txt` 新增 `make_chatterbox_turbo_loader`（社区 Chatterbox
    Turbo TTS）。上游 loader 清单共 65 个。
  - **C ABI 边界无需改动**：`framework/runtime/*` 与 `framework/io/json.h` 本次 diff
    **零改动**。`capi.h` / `capi.cpp` / build.rs bindgen allowlist 保持原样。
  - 已同步 `audio-cpp/src/types.rs` 的 `ModelFamily`：新增 1 个枚举变体
    `ChatterboxTurbo`（`as_str()` → `"chatterbox_turbo"`，与上游 loader 族名一致）+
    `from_path()` 关键词 + `From<&str>`；并在 `audio-cpp-sys/Cargo.toml`
    与 `audio-cpp/Cargo.toml` 新增 `model-chatterbox-turbo` feature。`model_family_roundtrip`
    等测试已覆盖新变体（23 项全过）。
- **升级 `ee7be93`→`3497b7c`（release-0.7.1）审查结论**：
  - diff 共 32 文件（+9.4k/-34），涉及 1 个新 loader 族 + chinese_variant 框架
    层新增 + granite5asr/chatterbox 小修复：
    `audio.cpp/CMakeLists.txt` 新增 `make_audio8_tts_loader`（社区 Audio8 TTS）。
    `engine_core` 新增 `chinese_variant.cpp`（中文变体处理）。上游 loader 清单共 64 个。
  - **C ABI 边界无需改动**：`framework/runtime/*` 与 `framework/io/json.h` 本次 diff
    **零改动**。`capi.h` / `capi.cpp` / build.rs bindgen allowlist 保持原样。
  - 已同步 `audio-cpp/src/types.rs` 的 `ModelFamily`：新增 1 个枚举变体
    `Audio8Tts`（`as_str()` → `"audio8_tts"`，与上游 loader 族名一致）+
    `from_path()` 关键词 + `From<&str>`；并在 `audio-cpp-sys/Cargo.toml`
    与 `audio-cpp/Cargo.toml` 新增 `model-audio8-tts` feature。`model_family_roundtrip`
    等测试已覆盖新变体（23 项全过）。
- **升级 `d2ff370`→`ee7be93`（main）审查结论**：
  - diff 共 230 文件（+31k/-19k），涉及 3 个新 loader 族 + 大量社区模型实现 +
    ggml CUDA 重写 + `model_specs_v1/*` 整体迁移到 `model_specs/*`：
    `audio.cpp/CMakeLists.txt` 新增 `make_echo_tts_loader` / `make_soprano_tts_loader`
    / `make_voxcpm1_loader`（上游 loader 清单共 63 个）。已同步 `audio-cpp/src/types.rs`
    的 `ModelFamily`：新增 3 个枚举变体 `SopranoTts` / `EchoTts` / `Voxcpm1`（`as_str()`
    → `soprano_tts` / `echo_tts` / `voxcpm1`，与上游 loader 族名一致）+ `from_path()`
    关键词表 + `From<&str>`；并在 `audio-cpp-sys/Cargo.toml` 与 `audio-cpp/Cargo.toml`
    新增 `model-soprano-tts` / `model-echo-tts` / `model-voxcpm1` 三个 `model-*` feature
    （build.rs 按 `model-<target>` 约定自动映射 CMake target，无需改 build.rs）。
    `model_family_roundtrip` 等测试已覆盖这 3 个新变体（23 项全过）。
  - **C ABI 边界无需改动**：capi.cpp 依赖的 `engine/framework/runtime/*` 与
    `engine/framework/io/json.h` 公共 API 本次 diff **零改动**（仅 `framework/core/
    backend.h`、`framework/sampling/torch_random.h` 等新增/内部文件变更）。`capi.h` /
    `capi.cpp` / build.rs bindgen allowlist 保持原样。
  - 内部行为变更（不触及 ABI，高层 `types.rs` 无需改）：ggml CUDA 后端大幅重写；
    `wav_reader.cpp`、`framework/core/backend.cpp`、`qwen_causal_decode_runtime.cpp`
    等核心文件更新；新增 `moss_audio_tokenizer_codec_runtime`（替代部分 moss shared
    音频 tokenizer 代码）、`torch_random` 采样运行时的 CPU/CUDA 实现；`model_specs_v1`
    目录被 `model_specs` 取代（仅影响上游自带 CLI/WebUI，不影响本 FFI 的 CMake 构建，
    构建仍由 `AUDIOCPP_MODEL_SET` / `AUDIOCPP_MODELS` 控制）。
- **升级 `da16c1b`→`d2ff370`（v0.7.0）审查结论**：
  - diff 涉及 1 个新 loader 族 + Fish Audio codec 内部重构 + server 改进：
    `audio.cpp/CMakeLists.txt` 新增 `make_granite5asr_loader`（IBM Granite Speech
    5.0 470M TurboCTC ASR 社区模型，aliases: `granite_speech5_asr` /
    `granite_speech` / `granite_speech5_ctc`），上游 loader 清单共 60 个。
    Fish Audio 的 `FishAudioCodes` / `FishAudioCodecRuntime` 从模型层
    `include/engine/models/fish_audio/` 提升为框架层共享 codec
    `include/engine/framework/codecs/fish_dac_codec_runtime.h`，原 `codec.h`
    删除、`codec.cpp` 由 `fish_dac_codec_runtime.cpp`（编入 `engine_core`）替代。
    server 新增 `model_memory.cpp`（模型内存预估）。**未触碰
    `engine/framework/runtime/*` 与 `engine/framework/io/json.h` 公共 API**：
    C ABI 边界（capi.h/capi.cpp/build.rs bindgen allowlist）无需改动。
  - 已同步 `audio-cpp/src/types.rs` 的 `ModelFamily`：新增 1 个枚举变体
    `Granite5Asr`（`as_str()` → `"granite5asr"`，与上游 loader 族名一致）+
    `from_path()` 关键词（`granite5asr` / `granite-speech` / `granite_speech` /
    `granite`）+ `From<&str>`（含 4 个别名）；并在 `audio-cpp-sys/Cargo.toml`
    与 `audio-cpp/Cargo.toml` 新增 `model-granite5asr` feature（build.rs 按
    `model-<target>` 约定自动映射 CMake target，无需改 build.rs）。`as_str` 与
    `From` 一致性测试（`model_family_roundtrip`）已覆盖新变体。
  - 注意：`granite5asr` 不在 `core` 模型集，默认 `core-models` 构建不会编译它；
    需用时用 `custom-models` + `model-granite5asr` feature 或
    `AUDIOCPP_MODELS` 环境变量。
- **升级 `c79e588`→`da16c1b` 审查结论**：
  - diff 涉及大量模型族新增（`audio.cpp/CMakeLists.txt` 新增 5 个 `make_*_loader`：
    `audiosr` / `controlfoley` / `firered_audio` / `fireredtts3` / `midashenglm_gen`，
    上游 loader 清单共 59 个）。**未触碰 `engine/framework/runtime/*` 与
    `engine/framework/io/json.h` 公共 API**：C ABI 边界（capi.h/capi.cpp/build.rs
    bindgen allowlist）无需改动。
  - 已同步 `audio-cpp/src/types.rs` 的 `ModelFamily`：新增 5 个枚举变体
    （`Audiosr` / `ControlFoley` / `FireredAudio` / `Fireredtts3` / `MidashEnglmGen`）
    + `as_str()`（`audiosr` / `controlfoley` / `firered_audio` / `fireredtts3` /
    `midashenglm_gen`，与上游 loader 族名一致）+ `from_path()` 关键词表 +
    `From<&str>`；并在 `audio-cpp-sys/Cargo.toml` 与 `audio-cpp/Cargo.toml` 新增
    `model-audiosr` / `model-controlfoley` / `model-firered-audio` /
    `model-fireredtts3` / `model-midashenglm-gen` 五个 `model-*` feature（build.rs
    按 `model-<target>` 约定自动映射 CMake target，无需改 build.rs）。`as_str` 与
    `From` 一致性测试（`model_family_roundtrip`）已覆盖这 5 个新变体。
  - 注意：这 5 个新族均不在 `core` 模型集，默认 `core-models` 构建不会编译它们；
    需用时用 `custom-models` + 对应 `model-*` feature 或 `AUDIOCPP_MODELS` 环境变量。
- **升级 `288a271`→`c79e588` 审查结论**：
  - diff 仅涉及 `src/framework/audio/chunking.cpp`、`src/models/index_tts2/{gpt,session}.cpp`、
    `src/models/supertonic/session.cpp` 与 CI/文档，**未触碰 `engine/framework/runtime/*`
    与 `engine/framework/io/json.h` 公共 API**：C ABI 边界（capi.h/capi.cpp/build.rs
    bindgen allowlist）无需改动；`index_tts2` / `supertonic` 两族此前已收录于
    `ModelFamily`，无新增 loader 族。
  - 行为变更均为内部修复/优化：chunking 把越界 speech metadata 的抛错改为 Warning
    并丢弃（不再中断整段）；index_tts2 在 HIP 后端把 KV/conv 权重缓存默认改 F16（与
    CUDA 一致，规避 HIP 上 F32 的 flash-attn 转换开销）；supertonic 新增从请求
    `voice` / `supertonic.voice` 选项回退取声线预设（WebUI model_params 兼容）。
    均未引入新 ABI 字段或新 task 类型，高层 `types.rs` 无需改动。
- 历史升级记录（仍作参考）：
  - 升级（`980bd41`→`288a271`）审查结论：
  - **C ABI 边界无需改动**：capi.cpp 依赖的 `engine/framework/runtime/*` 与
    `engine/framework/io/json.h` 公共 API 均未破坏性变更（backend.h/model.h 仅
    新增带默认实现的虚函数与自由函数）。include 路径一直是 `include/engine/...`，
    build.rs 已把 `audio.cpp/include` 加入搜索路径，故 shim 直接编译通过。
  - **新增 8 个 VoiceTaskKind**：`VoiceCloning`(`clon`)/`VoiceConversion`(`vc`)/
    `SpeechToSpeech`(`s2s`)/`Alignment`(`align`)/`VoiceDesign`(`vdes`)/
    `SpeakerRecognition`(`spk`)/`Svc`(`svc`)/`Midi`(`midi`)，已同步到
    `types.rs` 的 `TaskKind`（as_str / 测试）。
  - **新增 6 个 loader 族**（上游 `make_*_loader` 清单共 54 个）：`f5_tts`、
    `magpie_tts`、`personaplex`、`moss_voicegen`、`meanvc2`、`mms_forced_aligner`，
    已同步 `ModelFamily`（枚举/as_str/from_path/From<&str>/测试）+ 对应 `model-*`
     feature（sys 与高层转发）。其余 48 个族此前已收录。
  - **补全未绑定的高价值 API**（上游运行时新增能力）：
    - C ABI 新增 `audiocpp_registry_supports_family`（加载前判断某族是否已编译
      进引擎）与 `audiocpp_registry_inspect_json`（预检模型：metadata /
      capabilities / CLI 选项 / 发现资产）；高层 `Registry::supports_family` /
      `Registry::inspect`（返回 `ModelInspection`：`CliInterface` / `NamedAsset` 等）。
    - `dump_*` 系列补齐新字段：TaskResult/StreamEvent 的 `word_timestamps`（词级
      时间戳）、`speaker_turns`（流式说话人分段）、`artifact_output` /
      `output_artifacts`（`VoiceArtifact`，二进制载荷 base64 编码）；`SpeakerTurn`
      补 `text`。高层 `TaskResult` / `StreamEvent` / `SpeakerTurn` 及新增
      `WordTimestamp` / `VoiceArtifact` 类型同步。
    - `parse_task_request` 解析 `voice` 条件对象（参考音频 / `cached_voice_id` +
      风格 `language`/`emotion`/`speaking_rate`/`pitch_shift`/`energy_scale`/`tags`）；
      高层新增 `VoiceCondition` 构造器（经 `Request::tts(...).voice(...)` 使用），
      比把参考音频塞进顶层 `audio` 更语义化。
  - 默认 `core-models` 构建约 1.5 分钟（win32/MSVC，增量）编译链接通过；22 个
    types/registry/request 单元测试全部通过。
- **bindgen 已升级到 0.72**，crate 版本升至 **0.4.0**（workspace 统一；0.3.0 / 0.3.1 均已发布到 crates.io，预编译资产随 0.3.1 发布到 `v0.3.1`，旧版 Release 保留可寻址）。
- **build.rs 已跟踪 submodule HEAD 指针**：`cargo build` 的 rerun-if-changed 加入
  父仓库 `.git/modules/audio-cpp-sys/audio.cpp/HEAD`（cargo 无法精准跟踪整个
  submodule 目录，对目录会退化为总是重跑、每次多花几分钟）。`git submodule update`
  / `git -C audio.cpp checkout|pull` 改写该文件即自动触发 build.rs 重跑与 C++
  增量重编，不再需要手动 touch capi.cpp；非常规 .git 布局兜底跟踪目录本身。
  升级 submodule 后直接跑 `cargo build --workspace` 即可，若输出仍是 0.x 秒
  未重编说明跟踪失效（可 `BUILD_DEBUG=1` 排查）。
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
  两模型的阈值选项键统一为 `threshold`（`vad_threshold` 在上游从未存在，
  传它会被静默忽略，见 `audio_cpp::options::request::THRESHOLD`）。
- ASR 已用 Citrinet ASR Q8_0 GGUF 验证（`audio-cpp/examples/asr_offline` 跑通，
  sample_16k.wav 转录为 Nature 台词）。Citrinet 不在默认 core-models 集，需
  `custom-models` 或 `full-models`；custom 只需 `$env:AUDIOCPP_MODELS="citrinet_asr"`。
  **GGUF 同样无法自动探测族别**，须显式 `family_hint="citrinet_asr"`（否则误判
  silero_vad 报 missing tensor）。
- 上游 CMake 支持 `AUDIOCPP_MODEL_SET=custom` + `AUDIOCPP_MODELS`（逗号分隔
  model targets）按需编译，避免 full 全量 77 个 loader 族的编译成本；引擎核心 +
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
  平台/后端/模型组合 **+ 本地 audio.cpp 的 commit（完整 SHA 前 12 位）** 拼资产名
 （`audio-cpp-prebuilt-{os}-{target}-{backend}[-{crt}]-{modelset}-static-{commit}.tar.gz`，
  `crt` 仅 Windows）。commit 直接编入文件名，消费端按自身 commit 精确请求：命中才
  下载、404 即立即回落源码构建，**不会在 commit 不符时白下整包**（旧逻辑是下载解压后
  才由 `metadata.json` 校验）。从 GitHub Releases 下载并缓存到
  `target/audio-cpp-prebuilt-cache/<tag>/<asset>`。`local_audio_commit()` 取父仓库
  记录的 submodule gitlink（取完整 SHA 前 12 位），并要求子模块工作树 HEAD 等于它且**干净
  （无未提交改动）**，否则返回 None 强制源码构建——保证资产名里的 commit 逐字节等价于
  本地将编译的 audio.cpp 内容。`ensure_prebuilt` 的 superset 回退：`custom-*` / `core`
  资产 404 时回退下载 `full-{commit}` 资产，仍失败才回落源码。下载后 `metadata.json`
  仍做兜底校验（`audio_commit` 须等于本地 gitlink、`msvc_ver` 不高于本地工具集）。
  backend 由 feature 推导（cpu/vulkan/metal；cuda/hip 不发预编译）。env：
  `AUDIOCPP_PREBUILT_TAG`（tag，默认 `v{version}`）、`AUDIOCPP_PREBUILT_REPO`、
  `AUDIOCPP_PREBUILT_URL`（完整地址覆盖，也可含 `{tag}`/`{asset}` 占位符供内网镜像，
  `file://` 前缀指本地归档直接复制）、`AUDIOCPP_PREBUILT_OFF`（禁用自动下载）。网络
  抖动重试 3 次带退避（404 等 4xx 为确定性失败不重试），仍失败回落源码。
  `AUDIOCPP_PREBUILT_DIR` 显式目录优先级高于自动下载。
- **CI 预编译资产**：`.github/workflows/prebuilt-audio-cpp.yml` 在 `v*` tag、
  `workflow_dispatch` 时构建（linux 4 / windows 4（cpu/vulkan 各×md/mt）/ macos 1，
  共 9 cell，full × cpu/vulkan/metal）。**main 开发期推送不触发，submodule 指针变动也不触发**——
  重新发布预编译资产属「发布 / Release 操作」，须由用户显式下达发布命令（见第 6 节）。
  用 `.github/scripts/collect-unix-prebuilt.sh` 与
  `collect-windows-prebuilt.sh` 从 `target/**/out`（及 Windows 长路径重定向的
  `%TEMP%\acb*`）收集静态库打包，写 `metadata.json`（含 `audio_commit` 完整 SHA 前 12 位 +
  `msvc_ver`），经 `gh release upload --clobber` 上传到 `v{CARGO_PKG_VERSION}`
   Release（同一版本 Release 下按 commit 累积多个资产，天然按版本+commit 双重隔离）。
   消费端按自身 crate 版本查对应 tag，**旧版 Release 长期保留即可被旧版消费者寻址**，
   `--clobber` 只覆盖同名资产、不删旧 Release，故不存在“新版覆盖旧预编译”问题。
  Windows vulkan 用 `.github/actions/setup-vulkan-sdk-windows`（LunarG SDK）。资产命名
  须与 `prebuilt_download.rs::asset_name()` 保持一致（含 commit 段）。
- **MSVC `crt-static`（静态 CRT）**：消费端开 `-C target-feature=+crt-static`
  后，Rust 侧 std 与 cc 编译的 C shim（capi.o）均为 `/MT`，而 CMake 默认（及
  预编译资产）是 `/MD`，混链接报 LNK2038（RuntimeLibrary 不匹配）+ LNK2019
  （`__imp_*` 无法解析）。build.rs 检测 `CARGO_CFG_TARGET_FEATURE` 含
   `crt-static` 时：跳过 `/MD` 预编译资产，改请求 `-mt` 资产（CI windows 矩阵含
   mt 构建），无匹配才回落源码构建；并给 CMake
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

- **基于 voxkit 消费反馈的新 API**（voxkit 是 crates.io audio-cpp 0.2 的真实
  消费者，ASR/VAD/TTS/Diar/流式全链路）：
  - `ModelFamily::from_path(path)`：按文件名关键词推断模型族（GGUF 加载时
    免手写家族匹配表），支持 `#family=xxx` 片段显式覆盖优先；
  - `Request::stream()`：纯 options 的流式请求构造器（无音频输入，用于
    流式 `start`/`prepare`，音频经 `process_audio` 逐块送），配合
    `.option("language", ..)` / `.option("audio_chunk_seconds", ..)`，
    免手拼 JSON；
  - `StreamingSession`（`audio_cpp::StreamingSession`，session 模块公开）：
    流式会话便捷封装，自动注册事件收集器，`push_audio` 返回 `Vec<StreamEvent>`
    （该块全部事件），消除手动回调 + `Arc<Mutex<Vec>>` 样板；
  - **`Session` 独立于 `Model` 存活**（上游 session 持权重/资产 `shared_ptr`
    共享所有权，silero_vad 与 spec_backed 均如此）：会话创建后可释放
    Model/Registry，lib.rs 生命周期文档已记录并给 `no_run` 示例。
  - 上游 `v0.2.0` git tag 与 crates.io 发布版不一致（发布后未补 tag，tag 仅
    服务预编译库）——是预期行为，勿改。

## 常用命令速查
```bash
git submodule update --init --recursive                                  # 补齐 audio-cpp-sys/audio.cpp（克隆后必需）
git -C audio-cpp-sys/audio.cpp pull --ff-only                             # 单独更新 submodule 到上游最新
git -C audio-cpp-sys/audio.cpp diff <旧commit>..HEAD --stat                 # 升级审查：定位改动面
cargo build -p audio-cpp-sys                                              # 单独构建底层
BUILD_DEBUG=1 cargo build                                                 # 调试构建脚本
AUDIOCPP_PREBUILT_DIR=<预编译目录> cargo build                            # 跳过 CMake，直接用预编译静态库
```