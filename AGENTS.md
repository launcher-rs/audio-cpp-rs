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
- 高层 `audio-cpp` crate 的消耗代码；
- `bindings.docsrs.rs`（docs.rs 预生成绑定：改 `capi.h` 后本地完整构建一次，
  把 `target/debug/build/audio-cpp-sys-*/out/bindings.rs` 拷回；否则下次发布的
  docs.rs 文档与新 ABI 脱节）。

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
   计数口径三件事要分别核：`audiocpp_add_model` 的 **target** 数、`make_*_loader` 的
   **去重 loader 名**数、`model_specs/*.json` 的 **spec** 数。自 `cc21c9ea` 起
   「loader 数 = spec 数」不再恒成立（`earshot_vad` 为无 spec 的手写 loader）：
   新增此类族时除了同步 `ModelFamily`，还要在 `scripts/gen-models-doc.ps1` 的
   `$specless` 表补登记，否则 `docs/models.md` 会静默漏族。
   另：升级后务必确认 submodule **工作树**已切到新 commit
   （`git -C audio-cpp-sys/audio.cpp log -1` 与父仓库 `git ls-tree HEAD audio-cpp-sys/audio.cpp`
   一致，`git status` 不应长期挂 ` M audio-cpp-sys/audio.cpp`）——只改指针不切工作树时，
   构建验证跑的是旧源码，结论无效。
2. **新增模型 loader → 同步 `ModelFamily`**：查 `audio.cpp/CMakeLists.txt` 是否新增
   `make_*_loader`。有则同步 `audio-cpp/src/types.rs`：`ModelFamily` 枚举（`as_str()`
   返回的字符串须与上游 loader 族名一致）+ `from_path()` 关键词表 + Cargo.toml 新增
   `model-*` feature（如适用）。若只是现有 loader 的行为/选项变化（如 dots_tts 新增
   edit 推理），枚举无需动，但需评估高层类型/示例是否要暴露新能力。
   无论新增还是改名 loader 族，都要重跑 `pwsh -File scripts/gen-models-doc.ps1`
   刷新 `docs/models.md`，并在 `scripts/model-desc-zh.tsv` 补/改对应中文简介。
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
- 当前 submodule HEAD = `4de392e0`（origin/main，`5ec688c6` 之后 19 个提交）。
  crate 版本为 **0.11.0**（workspace 统一；历史见下条与逐条升级记录）。
  ⚠️ **`e9303b76`→`cc21c9ea` 那次升级的工作树/gitlink 不一致（当时已纠正，记录以防再犯）**：
  `5a5b7a1` 把
  gitlink 提交为 `e9303b76`，但 submodule **工作树始终停在 `ccdbe769`**（submodule
  reflog 只有 `22cad6af → ccdbe769` 一次 checkout，无 `e9303b76` 记录）。因此那条
  记录里「cargo build 4m14s 全量重编」的验证实际是对 `ccdbe769` 源码跑的，并非
  `e9303b76`。推进到 `cc21c9ea`（包含 `e9303b76`）时已一并覆盖该校验。
  教训：升级 submodule 后必须 `git -C audio-cpp-sys/audio.cpp log -1` 确认**工作树**
  与父仓库 gitlink 一致（`git status` 里 ` M audio-cpp-sys/audio.cpp` 长期挂着即为不一致），
  只看 `git ls-tree HEAD` 的指针不足以证明源码已切换。
  注：当时已按 `e9303b76` 实际文件复核更早的 130 计数（`audiocpp_add_model` 125 /
  去重 loader 130 / spec 130 全部吻合），Rust 侧同步内容本身无误。
- **升级 `5ec688c6`→`4de392e0`（main）审查结论**：
  - diff 共 182 文件（+24.8k/-5.4k，19 个提交）：新增 **3 个 `audiocpp_add_model`
    target**（139→142）`tiny_voca`（Tiny Voca：4.5M 参数英文 TTS，自带 dizione
    音素化前端（src/community_models/tiny_voca/dizione/*，含 g2p/同形异义词/
    形态学/规整化），无需外部 phonemizer；task `tts` 仅离线，内置 M2/F2 两音色，
    支持长文本；请求选项 `voice_id`/`speed`/`semitone_shift`/`pitch_shift`/
    `language`）/ `rosvot`（ROSVOT 歌声转符号谱：音符音高/时值与词对齐 MIDI，
    ROSVOT+RWBD+RMVPE 三检查点打包单 GGUF；task `midi` 仅离线，语言 `music`；
    请求选项 `output_format`（midi/json）/`note_boundary_threshold`/
    `word_boundary_threshold`/`word_durations_file`）/ `soulx_singer`
    （SoulX-Singer：参考音频条件的歌唱合成（SVS）与歌唱 voice conversion
    （SVC）；task `music`（即 `gen` 别名）/`svc` 仅离线，zh/en/yue；
    请求选项 `reference_metadata_file`/`target_metadata_file`/`control`/
    `pitch_shift`/`auto_pitch_shift`/`guidance_scale`/`num_steps`/`seed`，
    会话选项 `weight_type`/`attention`）。去重 loader 名 147→**150**，
    spec 文件 146→**149**（`earshot_vad` 仍无 spec）。无新增 ALIASES。
    上游权重名：`tiny-voca-GGUF/tiny-voca-orig.gguf`（24MB 单文件，音素化器
    内嵌）、`ROSVOT-GGUF/rosvot-f32.gguf`、`SoulX-Singer-GGUF/
    soulx-singer-{q8_0,f16}.gguf`。
  - 其余为 PocketTTS / PocketTTS Small 重构到共享 Mimi latent 解码器运行时
    （`framework/codecs/mimi_latent_decoder_runtime`，修流式历史与解码器生命
    周期）/ zonos2 长文本续写（`long_form_continuation`，chunk 默认 300→150）/
    cosyvoice3 新增 `speed` + HiFT 元数据 arena 缩小（1024→64 / 2048→4）/
    RMVPE CUDA 运行时（`ENGINE_HAS_CUDA_RMVPE`）/ 中文规整化日文保留（audio8/
    voxcpm1 描述更新）/ ace_step `reference_audio` 音色条件 / CLI YAML 配置
    （app/cli/args.cpp +286）/ dizione 拉丁语支持 / HIP rocBLAS 兼容包（CI 层）
    等。
  - **C ABI 边界无需改动**：capi.cpp 实际包含的 6 个头（`capi.h`、
    `framework/core/backend.h`、`framework/io/json.h`、
    `framework/runtime/{model,registry,session}.h`）本次 diff **零改动**
    （另核查的 `errors.h`/`spec_backed_model.h`/`task_vocabulary.{h,cpp}` 亦
    零改动）。`capi.h` / `capi.cpp` / build.rs bindgen allowlist（`audiocpp_.*`）
    保持原样，`bindings.docsrs.rs` 无需重拷。
  - 无新 task 类型 / 输出字段：`svc` 是**既有** `VoiceTaskKind::Svc` token
    （`task_vocabulary.cpp:27`，`TaskKind::Svc` 高层早已存在）；`midi`/`music`
    均既有（`sheetsage2` 先例）。3 个新 spec 的 options `type` 全为
    `enum`/`float`/`int`/`string`/`bool`，无 `*_list` 数组类型（kokoro
    `phonemes`、ming3_tts `speaker_embedding_file` 两处既有缺口依旧）；
    5 个既有 spec 的改动全是加法选项/描述与默认值（无删键改名）。高层
    `types.rs` serde 结构无需改。
  - 已同步 `audio-cpp/src/types.rs` 的 `ModelFamily`：新增 3 个枚举变体
    `TinyVoca`（`as_str()` → `"tiny_voca"`）/ `Rosvot`（→ `"rosvot"`）/
    `SoulxSinger`（→ `"soulx_singer"`，均与上游 `kFamily`/`family()` 一致）+
    `from_path()` 关键词表（**排序陷阱**：`soulx_singer`/`soulx-singer`/
    `soulxsinger` 三条必须排在裸 `"soulx"`（SoulX-FlashHead）之前——上游权重
    目录 `SoulX-Singer-GGUF` 与文件名 `soulx-singer-q8_0.gguf` 均含 `"soulx"`，
    会被抢先吞掉；`tiny_voca`/`rosvot` 与既有关键词无前缀冲突）+ `From<&str>`；
    并在两个 Cargo.toml 新增 `model-tiny-voca` / `model-rosvot` /
    `model-soulx-singer` 三个 feature（build.rs 自动映射 CMake target）。
    `model_family_roundtrip` / `model_family_from_path`（含
    `SoulX-Singer-GGUF/soulx-singer-q8_0.gguf`、`soulx-singer-f16.gguf` 与
    `SoulX-FlashHead-Lite-Q8_0.gguf` 仍判 SoulxFlashhead 的反向断言）测试已
    覆盖。
  - 3 新族 session 均自持资产：`tiny_voca` 持 `shared_ptr<const TinyVocaAssets>`；
    `rosvot` 持 `RosvotRuntime`（值成员，内部持 `shared_ptr<const RosvotAssets>`）
    + contract（spec-backed）；`soulx_singer` 持 `shared_ptr<const ModelAssets>`
    + contract（spec-backed）。`Session` 独立于 `Model` 存活的保证成立。
  - 计数同步：`README.md`（3 处）/ `audio-cpp-sys/README.md` / 两个 Cargo.toml
    的族数注释统一 147→**150**；重跑 `pwsh -File scripts/gen-models-doc.ps1`
    刷新 `docs/models.md`（**150** 族 + 2 内置 VAD，with upstream doc 125，
    missing zh desc = 0），`scripts/model-desc-zh.tsv` 补 3 条中文简介。
    注意 `audiocpp_add_model` 的 **target** 数是 142，去重 **loader** 名才是 150
    （一个 target 可挂多个 loader；`earshot_vad` 无 spec，故 spec 149）。
  - **版本 0.10.0 → 0.11.0**（`workspace.package.version` +
    `workspace.dependencies.audio-cpp-sys` 版本 + `audio-cpp-sys/README.md`
    四处 + 本文件；`Cargo.lock` 随构建同步）。
  - 验证：`cargo fmt --all -- --check` + `cargo build --workspace`（2m10s，
    build.rs 跟踪 submodule HEAD 正常触发 CMake 重编与 `engine_runtime.lib`
    重链接）+ `cargo test --workspace`（32 lib + 12 doc 全过）+
    `cargo clippy --workspace --all-targets` 零代码警告（4 条 prebuilt 下载
    依赖的 manifest unused 警告为既有）+
    `cargo check -p audio-cpp-sys --features custom-models,model-tiny-voca,
    model-rosvot,model-soulx-singer`（CMakeCache `AUDIOCPP_MODELS=rosvot,
    soulx_singer,tiny_voca`，dizione 全部 obj 已编译，映射通过）。
  - 注意：`metadata.json.audio_commit` 与新 submodule HEAD 不一致会强制回落
    源码构建（预编译自动下载被跳过），属预期；`cargo publish` / tag /
    Release / 预编译资产上传均属第 6 节发布操作，须用户显式下达，本次未执行。
- **升级 `d38518a0`→`5ec688c6`（main，v0.9.5 之后）审查结论**：
  - diff 共 62 文件（+5.2k/-0.9k，13 个提交）：新增 **1 个 `audiocpp_add_model`
    target**（138→139）`m5_tts`（M5-TTS：Qwen3 speech codec 解码的多语自回归 TTS
    + 参考音频克隆；task `tts`/`clone`（即 `clon`），离线+流式；语言
    zh/yue/zh-Sichuan/en/de/fr/ja/ko；请求选项 `language`/`temperature`/`top_k`/
    `max_tokens`/`voice_id`（仅 CustomVoice 检查点）/`reference_text`/`instruct`/
    `seed`/`text_chunk_size`/`text_chunk_mode`，会话选项
    `weight_type`/`codec_weight_type`/`attention`）。去重 loader 名 146→**147**，
    spec 文件 145→**146**（`earshot_vad` 仍无 spec）。无新增 ALIASES。
  - 其余为共享 MoE FFN + Gated DeltaNet 内核重构、chatterbox T3 音色跨语言保留、
    campplus 常量共享、PocketTTS 流式修复、omnivoice RKNN zeroop、上游 **opt-in
    C API** 暴露 custom-schema 结果（`src/capi/audiocpp.cpp` + `include/audiocpp.h`，
    与本仓 shim 无关；本仓 `dump_task_result` 早已导出 `custom_schema_output`）
    与新增 `AUDIOCPP_BUILD_C_API_STATIC`、Windows Xet 下载校验等。
  - **C ABI 边界无需改动**：capi.cpp 依赖的 8 个头
    （`framework/core/backend.h`、`framework/io/json.h`、
    `framework/runtime/{model,registry,session}.h`、`runtime/task_vocabulary.h`、
    `core/errors.h`、`runtime/spec_backed_model.h`）本次 diff **零改动**。
    `capi.h` / `capi.cpp` / build.rs bindgen allowlist（`audiocpp_.*`）保持原样，
    `bindings.docsrs.rs` 无需重拷。
  - 无新 task 类型 / 输出字段：`clone` 是既有 `clon` 别名（经通用任务字符串透传）；
    spec diff 无 `*_list` 数组类型、无删除/改名键（仅新增 `m5_tts.json`）。
    高层 `types.rs` serde 结构无需改。
  - 已同步 `audio-cpp/src/types.rs` 的 `ModelFamily`：新增枚举变体 `M5Tts`
    （`as_str()` → `"m5_tts"`，与上游 loader 族名一致）+ `from_path()` 关键词
    （`m5_tts`/`m5-tts`/`m5tts`；上游权重目录 `M5-TTS-GGUF`、文件名
    `m5-tts-50m-{voiceclone,customvoice}-{f16,q8_0}.gguf` 均可命中）+ `From<&str>`
    （含 `m5-tts`/`m5tts`）；并在两个 Cargo.toml 新增 `model-m5-tts` feature
    （build.rs 自动映射 CMake target）。`model_family_roundtrip` /
    `model_family_from_path`（含 `M5-TTS-GGUF/m5-tts-50m-voiceclone-q8_0.gguf`
    真实路径断言）已覆盖。
  - 新族 session 自持资产：`m5_tts` 持 `shared_ptr<const ModelAssets>` + contract
    （spec-backed），`Session` 独立于 `Model` 存活的保证成立。
  - 计数同步：`README.md`（3 处）/ `audio-cpp-sys/README.md` / 两个 Cargo.toml
    的族数注释统一 146→**147**；重跑 `pwsh -File scripts/gen-models-doc.ps1`
    刷新 `docs/models.md`（**147** 族 + 2 内置 VAD，missing zh desc = 0），
    `scripts/model-desc-zh.tsv` 补 1 条中文简介。注意 `audiocpp_add_model` 的
    **target** 数是 139，去重 **loader** 名才是 147。
  - 验证：`cargo fmt --all -- --check` + `cargo build --workspace`（34s，
    build.rs 跟踪 submodule HEAD 正常触发 CMake 增量重编与 `engine_runtime.lib`
    重链接）+ `cargo test --workspace`（32 lib + 12 doc 全过）+
    `cargo clippy --workspace --all-targets` 零警告 +
    `cargo check -p audio-cpp-sys --features custom-models,model-m5-tts`
    （`engine_model_m5_tts` 的 ar/assets/session 三个 obj 已编译，映射通过）。
  - 注意：`metadata.json.audio_commit` 与新 submodule HEAD 不一致会强制回落
    源码构建（预编译自动下载被跳过），属预期；`cargo publish` / tag /
    Release / 预编译资产上传均属第 6 节发布操作，须用户显式下达，本次未执行。
    上游 `v0.9.5` 在本范围起点之前（`51fa5e61`），本仓按惯例跟 origin/main HEAD。
- **升级 `d2da530b`→`d38518a0`（main）审查结论**：
  - diff 共 526 文件（+64.2k/-9.7k，63 个提交）：新增 **5 个 `audiocpp_add_model`
    target**（133→138）`edda_asr`（社区 Edda v0.2 large：丹麦语 Whisper-large-v3
    微调，可选 DFlash2 无损投机解码 drafter（edda-v0.2-large-hurtig），约 3~7 倍
    解码加速；task `asr`，离线+流式（每块出完整结果），da；请求选项
    `language`/`speculative`）/ `moss_soundeffect`（MOSS-SoundEffect v2：
    en/zh 文本转音效，Qwen3 文本编码器 + Wan 音频 DiT + 连续 DAC 解码器；
    task `sfx`（即 `gen`）仅离线，请求选项 `duration_sec`（0.1~30s）/
    `num_steps`/会话选项 `moss_soundeffect.weight_type`）/
    `mulacover`（MuLaCover 翻唱歌曲生成，实验性：参考音频或旋律/和弦 MIDI +
    新歌词与风格；task `music`（即 `gen`）仅离线，请求选项 `melody_file`/
    `chord_file`/`drum_file` 等 MIDI 路径，另含转写器子模型）/
    `zonos2`（Zyphra ZONOS2 社区 TTS：28 层 GQA/MoE 主干 + 9 延迟
    1024-entry DAC codebook，Descript Audio Codec 解码 44.1kHz，ECAPA
    说话人嵌入克隆；task `tts`/`clon`，离线+流式，en/fr/de/es/it/pt/ja/zh/ko；
    主干权重 `zonos2-q8_0.gguf`）/ `zonos3`（Zyphra ZONOS3-Mini 社区 TTS：
    GDN/attention latent-MoE 主干 + 10 延迟 4096-entry codebook，MR-iSTFT
    解码 44.1kHz，说话人嵌入与原始参考两种克隆；同 9 语，离线+流式；
    主干权重名为 `backbone-ptq.gguf`，族名只能靠目录 `ZONOS3-Mini-GGUF` 识别）。
    去重 loader 名 141→**146**，spec 文件 140→**145**（`earshot_vad` 仍无 spec，
    故 loader 数比 spec 多 1）。无新增 `ALIASES`。
  - 其余为上游 **SYCL 后端**（`ENGINE_ENABLE_SYCL`，Intel oneAPI；本仓与 WebGPU
    一样不接 feature，仅源码可编）/ 上游 opt-in C API 新增日志回调
    `audiocpp_log_set`（`src/capi/audiocpp.cpp`，与本仓 shim 无关）/ MoE 专家
    卸载统一到共享框架资源（drama3、nemotron_transcribe、ming3_audio 重构）/
    会话自持 PreparedGraph 生命周期 / 共享 DiT 运行时迁移 4 族 / Qwen3 音频
    tokenizer 提升为框架 codecs / Kroko XL 离线 ASR（仍属既有 `kroko_asr` 族，
    无新 loader）/ `sense_asr` 输出词级时间戳（走既有 `word_timestamps`）/
    `audio8_tts` 新增 Realistic 0.6B 并改为默认包（原 Preview 保留）/
    `kugelaudio` 新增参考音频克隆（tasks 变为 `tts`+`clone`，新选项
    `reference_encoder_path`）/ `pocket_tts` 支持 `flow_lm.z_proj` 维度不等 /
    `chatterbox` 语音转换目标缓存 / 任务请求接受内联 `voice_ref`（WAV 数据，
    非 WAV 返回 400；本仓高层未加构造器，属既有 JSON 透传可达面）。
  - **C ABI 边界无需改动**：capi.cpp 依赖的 8 个头
    （`framework/core/backend.h`、`framework/io/json.h`、
    `framework/runtime/{model,registry,session}.h`、`runtime/task_vocabulary.h`、
    `core/errors.h`、`runtime/spec_backed_model.h`）本次 diff **零改动**
    （`task_vocabulary` 的 17 个规范 token 与别名不变）。
    `capi.h` / `capi.cpp` / build.rs bindgen allowlist（`audiocpp_.*`）保持原样，
    `bindings.docsrs.rs` 无需重拷。
  - 无新 task 类型 / 输出字段：5 新族复用既有 `asr`/`sfx`/`music`/`tts`/`clone`
    （`sfx`/`music`/`clone` 均为既有别名，经通用任务字符串透传）；
    `sense_asr` 时间戳走既有 `word_timestamps`。高层 `types.rs` serde 结构无需改。
  - **无数组选项缺口**：12 个改动 spec 的 options `type` 全为
    `bool`/`int`/`enum`/`float`/`string`，无 `*_list` 类型（kokoro
    `phonemes`、ming3_tts `speaker_embedding_file` 两处既有缺口依旧未接）。
  - spec diff 无破坏性删除：既有族仅描述/包清单/新增选项（audio8_tts 默认包
    由 Preview 改为 Realistic、kugelaudio 增 `clone`），无删键改名；
    本仓 Rust 侧无需适配。
  - 已同步 `audio-cpp/src/types.rs` 的 `ModelFamily`：新增 5 个枚举变体
    `EddaAsr`（`as_str()` → `"edda_asr"`）/ `MossSoundeffect`
    （→ `"moss_soundeffect"`）/ `Mulacover`（→ `"mulacover"`）/ `Zonos2`
    （→ `"zonos2"`）/ `Zonos3`（→ `"zonos3"`，均与上游 loader 族名一致）+
    `from_path()` 关键词表（`moss_soundeffect` 系必须排在裸 `moss` 之前；
    `mulacover` 只收完整族名，**不收裸 `mula`**（否则 `heartmula` 会被误吞）；
    `zonos2`/`zonos-2`、`zonos3`/`zonos3_mini`/`zonos3-mini`/`zonos-3`；
    `edda_asr`/`edda-asr`/`edda`）+ `From<&str>`；并在两个 Cargo.toml 新增
    `model-edda-asr` / `model-moss-soundeffect` / `model-mulacover` /
    `model-zonos2` / `model-zonos3` 五个 feature（build.rs 自动映射 CMake
    target，无需改 build.rs）。`model_family_roundtrip` /
    `model_family_from_path`（含上游真实路径断言：
    `MOSS-SoundEffect-v2-GGUF/moss-soundeffect-v2-q8_0.gguf` /
    `MuLaCover-GGUF/mulacover-q8_0.gguf` / `ZONOS2-GGUF/zonos2-q8_0.gguf` /
    `ZONOS3-Mini-GGUF/backbone-ptq.gguf`，以及 `heartmula-q8_0.gguf` 不被
    `mulacover` 影响的回归断言）测试已覆盖。
  - 5 新族 session 均自持资产：`edda_asr` 持 `shared_ptr<const EddaAssets>`、
    `moss_soundeffect` 持 `shared_ptr<const Assets>`、`mulacover` 持
    `shared_ptr<const MuLaCoverAssets>`、`zonos2`/`zonos3` 分别持
    `shared_ptr<const Zonos2Assets>`/`shared_ptr<const Zonos3Assets>`
    （均为 spec-backed，另持 contract）；`Session` 独立于 `Model` 存活的保证成立。
  - 计数同步：`README.md`（3 处）/ `audio-cpp-sys/README.md` / 两个 Cargo.toml
    的族数注释统一 141→**146**；重跑 `pwsh -File scripts/gen-models-doc.ps1`
    刷新 `docs/models.md`（**146** 族 + 2 内置 VAD，missing zh desc = 0），
    `scripts/model-desc-zh.tsv` 补 5 条中文简介。注意 `audiocpp_add_model` 的
    **target** 数是 138，去重 **loader** 名才是 146。
  - **版本 0.9.0 → 0.10.0**（`workspace.package.version` +
    `workspace.dependencies.audio-cpp-sys` 版本 + `audio-cpp-sys/README.md`
    四处 + 本文件；`Cargo.lock` 随构建同步）。`prebuilt_download.rs` 测试 fixture
    里的旧版本号是纯逻辑测试数据，未改。
  - 验证：`cargo fmt --all -- --check` + `cargo build --workspace`（2m08s，
    build.rs 跟踪 submodule HEAD 正常触发重编）+ `cargo test --workspace`
    （32 lib + 12 doc 全过）+ `cargo clippy --workspace --all-targets` 零警告 +
    `cargo check -p audio-cpp-sys --features custom-models,model-edda-asr,
    model-zonos2,model-zonos3,model-moss-soundeffect,model-mulacover`
    （5 个 feature→CMake target 映射全部通过）。
  - 注意：`metadata.json.audio_commit` 与新 submodule HEAD 不一致会强制回落
    源码构建（预编译自动下载被跳过），属预期；`cargo publish` / tag /
    Release / 预编译资产上传均属第 6 节发布操作，须用户显式下达，本次未执行。
    上游 main 已过 `v0.9.5` 清理提交（`51fa5e61`），但 tag 仍停在 `v0.9.4`
    （`v0.9.5` 未打 tag）；本仓按惯例跟 origin/main HEAD。
- **升级 `cc21c9ea`→`d2da530b`（main）审查结论**：
  - diff 共 368 文件（+27.6k/-9.1k）：新增 **3 个 `audiocpp_add_model` target**
    （130→133）`lfm_d1_omni`（Liquid AI d1-Omni-2B 音频决策：对音频+具名问题输出
    校准的是/否、选项、评分与数值估计，12 语种；task `cls`（spec 写 `classify`）
    仅离线，结果走既有 `custom_schema_output`，schema `audio_decisions`）/
    `realtime_reuse`（RE-USE 的实时流式变体，因果通用语音修复，语言无关；
    task `s2s` 离线+流式，会话选项 `realtime_reuse.exit_layer`/`lookahead_frames`/
    `weight_type`）/ `pocket_tts_small`（PocketTTS Small 英文 18m/7m/2m 及 ReFL
    变体，实验性；task `tts`/`clon` 离线+流式，默认音色 `alba`，克隆走 voice
    embedding），另有 **3 个 `parakeet_tdt` target 的 ALIASES loader**
    （`phonon2_asr` Fermion Phonon-2 英文 / `underdog_bark3` Conway Underdog Bark 3
    Standard·Plus·Pro 英文 / `haas_asr` Haas-ja-v1.0 日文，均复用 Parakeet 实现、
    task `asr` 离线+流式）。去重 loader 名 135→**141**，spec 文件 134→**140**
    （仍只有 `earshot_vad` 无 spec，`gen-models-doc.ps1` 的 `$specless` 无需补）。
    其余为 lfm3_audio 模块重构（backbone/blocks/chat/encoder/published_gguf 等
    删除、提升为框架共享 `lfm_decoder_runtime` + `lfm3_audio_encoder_runtime`）/
    nemotron_transcribe `repetition_guard` / index_tts2 流式化 / soulx_flashhead
    自 `community_models` 迁至 `models` 命名空间（target 与 loader 名不变）/
    ggml ssm-scan Vulkan·Metal·CUDA 内核与 WebGPU 后端等。
  - **C ABI 边界无需改动**：capi.cpp 依赖的 5 个头（`framework/core/backend.h` /
    `framework/io/json.h` / `framework/runtime/{model,registry,session}.h`）与
    `task_vocabulary.{h,cpp}`、`errors.h`、`spec_backed_model.h` 本次 diff
    **零改动**；`runtime/cache_slots.h` 仅 +5 行内部加法（不经 C ABI）。
    `capi.h` / `capi.cpp` / build.rs bindgen allowlist（`audiocpp_.*`）保持原样，
    `bindings.docsrs.rs` 无需重拷。
  - 无新 task 类型 / 输出字段：6 新族复用既有 `asr`/`cls`/`tts`/`clon`/`s2s`
    （spec 的 `classify` 是既有 `cls` 别名，经通用任务字符串透传）；
    `lfm_d1_omni` 结果走既有 `custom_schema_output`（shim 早已导出）。
    高层 `types.rs` serde 结构无需改。
  - **无数组选项缺口**：6 个新 spec 的 options `type` 全为
    `bool`/`int`/`enum`/`float`/`string`/`path`，无 `*_list` 类型。
  - 破坏性 spec 变更（上游层）：`parakeet_tdt.json` **删除** `hotwords` /
    `hotwords_score`（连同 Phonon-2/Bark 描述一并迁入 `phonon2_asr.json` 与
    `underdog_bark3.json`，`haas_asr` 无此选项）——传旧键的 parakeet_tdt 请求
    会被上游未知选项校验拒绝；本仓 Rust 侧全仓 grep 无 `hotwords` 硬编码，
    无需代码改动。`index_tts2` modes 增 `streaming`（能力增强）+ 新选项
    `text_chunk_tokens`/`text_normalization`；`fish_drama3` 增 `skip_empty_chunks`；
    `nemotron_transcribe` 增 `repetition_guard`；`muscriptor` 增 medium/large
    权重包。全部走通用 options 字符串透传，高层无需改。
  - 已同步 `audio-cpp/src/types.rs` 的 `ModelFamily`：新增 6 个枚举变体
    `LfmD1Omni`（`as_str()` → `"lfm_d1_omni"`）/ `Phonon2Asr`（→ `"phonon2_asr"`）/
    `UnderdogBark3`（→ `"underdog_bark3"`）/ `HaasAsr`（→ `"haas_asr"`）/
    `PocketTtsSmall`（→ `"pocket_tts_small"`）/ `RealtimeReuse`
    （→ `"realtime_reuse"`，均与上游 loader 族名一致：3 个 parakeet ALIASES
    的 `kFamily` 即其族名，3 个新 target 同名）+ `from_path()` 关键词表
    （**排序陷阱**：`pocket_tts_small`/`pocket-tts-small`/`pockettts-small`
    必须排在裸 `pocket_tts` 之前；`realtime_reuse`/`realtime-reuse`/
    `real-time-reuse`/`realtimereuse` 必须排在 `reuse`/`re-use` 之前——官方目录名
    `RE-USE-GGUF` 小写后含 `re-use`、文件名 `realtime-reuse-f32.gguf` 含 `reuse`；
    phonon2 系/underdog 系/haas 系关键词置于 `parakeet` 关键词之前；
    `lfm_d1_omni`/`lfm-d1-omni`/`d1_omni`/`d1-omni` 不收裸 `d1`）+ `From<&str>`
    （含 `"phonon2-asr"`/`"underdog-bark3"`/`"haas-asr"` 连字符写法）；
    并在两个 Cargo.toml 新增 `model-phonon2-asr` / `model-underdog-bark3` /
    `model-haas-asr` / `model-lfm-d1-omni` / `model-pocket-tts-small` /
    `model-realtime-reuse` 六个 feature（build.rs 自动映射 CMake target/alias，
    无需改 build.rs）。`model_family_roundtrip` / `model_family_from_path`
    （含按上游真实权重文件名的 6 组防吞断言：`phonon-2-mixed-q8.gguf` /
    `Phonon-2-GGUF/phonon-2-mixed-q8.gguf` /
    `PocketTTS-Small-GGUF/english-18m-refl/english-18m-refl-f32.gguf` /
    `RE-USE-GGUF/realtime-reuse-f32.gguf` /
    `Haas-ja-v1.0-GGUF/haas-ja-v1.0-q8_0.gguf` /
    `Underdog-Bark-3.0-GGUF/underdog-bark-3.0-mixed-f16.gguf` /
    `d1-Omni-2B-GGUF/d1-Omni-2B-Q8_0.gguf` /
    `d1-Omni-2B-GGUF/mmproj-d1-Omni-2B-F16.gguf`）测试已覆盖。
  - 6 新族 session 自持资产：`pocket_tts_small` 持 `shared_ptr<const Assets>
    manifest_` + `shared_ptr<const BackendWeights> weights_`；`lfm_d1_omni` 持
    `shared_ptr<const LfmD1OmniAssets> assets_` + contract；3 个 parakeet ALIASES
    共享 `ParakeetTDTSession`（持 `shared_ptr<const ParakeetTDTAssets> assets_`）。
    `realtime_reuse` **例外但自足**：`Session` 把 assets `std::move` 进
    `std::unique_ptr<Runtime> runtime_`，而 `Runtime::Impl` 构造时仅把张量搬进
    自身成员 `weights`/`store` 后即 `assets->tensors->release_storage()`，
    **不保留任何 Assets 引用**（`Assets` 仅 `resources`+`tensors`，两者均只在
    `load_assets` 与 `Impl` 构造期使用）；`StreamingFrontend` 只依赖 `Runtime&`。
    `Session` 独立于 `Model` 存活的保证成立。
  - 计数同步：`README.md`（3 处）/ `audio-cpp-sys/README.md` / 两个 Cargo.toml 的
    族数注释统一 135→**141**；重跑 `pwsh -File scripts/gen-models-doc.ps1` 刷新
    `docs/models.md`（**141** 族 + 2 内置 VAD，missing zh desc = 0），
    `scripts/model-desc-zh.tsv` 补 6 条中文简介。
    注意 `audiocpp_add_model` 的 **target** 数是 133，去重 **loader** 名才是 141
    （一个 target 可挂多个 loader；`earshot_vad` 无 spec，故 spec 140）。
  - 已知小瑕疵（不阻塞）：上游把 `haas` 的说明页命名为 `docs/models/haas.md`，
    与 family 名 `haas_asr` 不一致，而 `gen-models-doc.ps1` 按 family 名匹配
    上游文档，故 `haas_asr` 行缺上游文档链接（同因既有：`qwen3`/`r2t2`/
    `vocalrender` 等 orphan doc）。
  - **版本 0.8.1 → 0.9.0**（`workspace.package.version` + `workspace.dependencies.
    audio-cpp-sys` 版本 + `audio-cpp-sys/README.md` 四处 + 本文件；`Cargo.lock`
    随 `cargo check` 同步）。`prebuilt_download.rs` 测试 fixture 里的 `0.8.1`
    是纯逻辑测试数据，未改。
  - 验证：`cargo fmt --all -- --check` + `cargo build --workspace`（1m43s，
    build.rs 跟踪 submodule HEAD 正常触发重编）+ `cargo test --workspace`
    （32 lib + 12 doc 全过）+ `cargo clippy --workspace --all-targets` 零警告 +
    `cargo check -p audio-cpp-sys --features custom-models,model-phonon2-asr,
    model-underdog-bark3,model-haas-asr,model-lfm-d1-omni,model-pocket-tts-small,
    model-realtime-reuse`（1m39s；CMakeLists 的 `ALIASES phonon2_asr
    underdog_bark3 haas_asr` 已核实，别名被上游接受）。
  - **发布（用户显式授权执行，第 6 节流程）**：
    ① `cargo publish --registry crates-io` 已发布 **`audio-cpp-sys 0.9.0`**
    （2026-11-06 07:15 UTC）与 **`audio-cpp 0.9.0`**（07:27 UTC），两者在
    crates.io API 可查且未 yanked。必须**按依赖序分两步**发：`audio-cpp` 依赖
    `audio-cpp-sys ^0.9.0`，先发 sys 并等索引可见，再发 cpp（`--workspace`
    的 dry-run 会在 cpp 打包阶段因找不到 `^0.9.0` 而失败，属预期）。
    本机 `~/.cargo/config.toml` 把 crates-io 替换为 `rsproxy-sparse` 镜像，
    镜像同步有延迟（sys 刚发时 cpp 解析不到 0.9.0，几分钟后自动可用）；
    另遇一次 crates.io 上传 `SSL connect error (schannel ... handshake)`
    瞬时错误，重试即过。`chacha20 v0.10.1`（Cargo.lock 传递依赖）被 yank 的
    warning 与本次发布无关。
    ② git tag **`v0.9.0`**（指向 `b8a352b`）已推送，触发
    `.github/workflows/prebuilt-audio-cpp.yml` run `42181033963`
    （9 cell：linux 4 / windows 4 / macos 1，full × cpu/vulkan/metal，
    历史同类约 26-27 分钟）。workflow 各 cell 自行 `gh release create`
    `v0.9.0`（若不存在）再 `gh release upload --clobber`，**无需预先建 Release**；
    资产名里的 `AUDIO_COMMIT` 取 `git ls-tree HEAD` 的 gitlink，故必须先 commit
    再推 tag，否则 CI 会按旧 gitlink 打包。
    **结果**：run `42181033963` 9/9 job 全绿（macOS 13m22s / linux 11m50s~20m13s /
    windows 19m38s~30m8s），Release `v0.9.0` 资产 **9/9 齐全**，文件名均含
    `d2da530bf04f`。已下载实测两个 Windows 资产（`cpu-md` 与 `vulkan-mt`）：
    `engine_runtime.lib`（889MB / 900MB）、`ggml*.lib`（`vulkan-mt` 另含
    `ggml-vulkan.lib` 67MB）、`sentencepiece*.lib`、`cjson_vendor.lib`、
    `yaml_vendor.lib` 全部在包内，`metadata.json` 的 `crate_version=0.9.0` /
    `audio_commit=d2da530bf04f` / `msvc_ver=1944` / `crt` 均正确 → **资产完好**。
  - ⚠️ **发现 CI 断言失效（本次发布资产未受影响，建议后续修复）**：4 个 Windows
    cell 都报了 `Missing key archive: engine_runtime.lib|sentencepiece*.lib|
    ggml*.lib` 的 `::error::` annotation，但 job 仍成功、实测资产也完好——
    两个缺陷叠加：
    ① `collect-windows-prebuilt.sh:64` 用 `compgen -G "$ROOT/lib/$need"` 断言，
    而 Windows runner 的 `$RUNNER_TEMP` 是反斜杠路径（`D:\a\_temp`），bash glob
    把 `\a`/`\t` 当转义符吃掉，模式必然匹配失败 → **断言恒误报**；
    `find`/`cp` 不走 glob 所以收集本身正常。Linux/macOS 的 `$RUNNER_TEMP` 是
    正斜杠，故 `collect-unix-prebuilt.sh` 无此问题（与只在 Windows 报错吻合）。
    ② workflow 的 `Collect prebuilt libraries` 步骤是 `shell: pwsh` 且内联调用
    `bash .github/scripts/...`，pwsh 对 native 命令非零退出**不会**因
    `$ErrorActionPreference='Stop'` 而失败，脚本的 `exit 1`（第 73 行）被吞掉，
    步骤照样走完 `tar` 打包 → **真缺文件时也会发坏包**，恰是该断言想防的事。
    本次因 ① 恒误报 + ② 恒吞错，等于断言完全不生效，但包内容实测无误。
    若要修：断言改 `find "$ROOT/lib" -name "$need" -print -quit | grep -q .`
    （或先 `cd` 到 `$ROOT/lib` 再用相对 glob），并在 pwsh 步骤把
    `bash ...` 改为 `if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }`。
    ③ 预编译 tag 默认 `v{version}`，即 `v0.9.0`；registry 消费端按
    `CARGO_PKG_VERSION` 查该 tag 的 Release 资产。
    上游 main 在 `v0.9.3`（`41c1f8ff`）之后；本仓按惯例跟 origin/main HEAD。
- **升级 `e9303b76`→`cc21c9ea`（main）审查结论**：
  - diff 共 579 文件（+77.4k/-21.5k，91 个提交）：新增 5 个 loader 族
    （`audiocpp_add_model` 125→130，去重 loader 名 130→135，spec 文件 130→134）：
    `earshot_vad`（Earshot VAD 原生 C++ 移植，alias `earshot`；task `vad` 离线+流式，
    16kHz mono、16ms 帧，权重为 `earshot_vad.safetensors`（**非 GGUF**，文件名不等于
    该名的须显式 `family_hint`）；选项 `threshold`/`neg_threshold`/`smoothing_factor`/
    `speech_pad_ms`/`min_speech_duration_ms`/`min_silence_duration_ms`/`start_delay_ms`/
    `keep_trailing_segment`，会话选项 `exec_plan`）/
    `nemotron_transcribe`（NVIDIA Nemotron 3.5 Transcribe 30B-A4B，实验性；task `asr`
    仅离线；输出含 `word_timestamps` + `speaker_turns` + `custom_schema_output`；
    请求选项 `template_name`/`return_timestamps`/`diarization` 等，会话选项为
    `disk_experts`/`pin_experts`/`expert_cache_size` 等 CPU 专家卸载系）/
    `lfm3_audio`（Liquid AI LFM3-Audio-2B，实验性；task `asr`/`tts`/`s2s` 离线+流式，
    18 语种且**无 auto**；S2S 多轮历史经 `output_artifacts` 的 `lfm3_audio.reply`
    产物回传）/ `magenta_realtime_2`（Google Magenta RealTime 2 帧自回归立体声音乐
    生成，实验性；spec task 写 `music`（即 `gen` 别名），离线+流式；`pianoroll_file`
    是**字符串**（内含 JSON）而非数组选项）/ `soulx_flashhead`（SoulX-FlashHead Lite
    音频驱动人像视频生成，实验性；spec task 写 `sfx`（即 `gen` 别名），离线+流式，
    请求选项 `reference_image_file`（单个 `path`，必填）+ 尺寸/步数系，视频帧经
    `output_artifacts`）。其余为 WebGPU 后端集成（`fb09bde9`/`dcec826b`/`611131c8`，
    仅上游 browser/native WebGPU，本仓未接 feature）/ Metal IM2COL_3D·SSM kernel /
    HIP·CUDA lowering / irodori_tts v4 Large（同族既有 loader）/ Ming3 TTS 流式化 /
    Nemotron ASR CPU 专家卸载 / LoRA 与 package 解析重构（`model_package.h` +
    `resolve_package_load`，不经本 shim）/ Earshot SIMD 内核 / server 与 WebUI 等。
  - **不变式首次被打破（后续升级注意）**：`earshot_vad` 是**纯手写 loader、无
    `model_specs/*.json`**，故「去重 loader 名 = spec 文件数 = `ModelFamily` 非内置族数」
    不再成立（135 vs 134）。`scripts/gen-models-doc.ps1` 已新增 `$specless` 手工登记表
    （含幂等的 family 去重），把这类无 spec 的族也收进 `docs/models.md`；新增此类族时
    需在 `$specless` 补一条（family/display/cat/status/tasks/modes/langs），
    否则文档会静默漏族、族数与枚举对不上。
  - **C ABI 边界无需改动**：capi.cpp 依赖的 5 个头中 `framework/io/json.h` 与
    `framework/runtime/session.h` 本次 diff **零改动**（`TaskResult` / `StreamEvent`
    结构体无新字段，`dump_task_result` / `dump_stream_event` / `dump_audio_buffer`
    无需补字段）；`backend.h` +40/-9 为源码兼容的加法（`BackendConfig::impl` 带默认值、
    `BackendDeviceInfo` 新增 `backend_option`/`device_name`/`description`/`pci_bus_id`/
    `memory`/`caps`、`list_backend_devices(bool query_properties = false)` 加默认参数、
    新增 `print_backend_devices_json`/`is_hip_buffer`/`set_backend_round_bf16_fusion`
    自由函数，`BackendMemorySnapshot` 仅文件内移位；`name` 字段取值口径未变），
    shim 不调用任一新项；`model.h` +12（`PackageLoadTarget` + 带默认实现的
    `IVoiceModelLoader::resolve_package_load`，我们不自实现 loader，够不着）、
    `registry.h` +10（`ModelRegistry::resolve_package_load` + 自由函数
    `load_vad_model`），均为加法且不进 C ABI。`task_vocabulary.{h,cpp}` 与 `errors.h`
    **零改动**（17 个规范 token + 别名不变，无新异常类型），故 `TaskKind` 无需动。
    `capi.h` / `capi.cpp` / build.rs bindgen allowlist（`audiocpp_.*`）保持原样，
    `bindings.docsrs.rs` 无需重拷。
  - 无新 task 类型 / 输出字段：5 新族复用既有 `vad`/`asr`/`tts`/`s2s`/`gen`
    （spec 的 `music`/`sfx` 是既有别名，经通用任务字符串透传；文档生成器已换算为
    `gen`）；`custom_schema_output` / `speaker_turns` / `word_timestamps` /
    `output_artifacts` / `speech_segments` 全部为 shim 早已导出的字段。
    高层 `types.rs` serde 结构无需改。
  - **数组选项缺口未变**：本次 4 个新 spec 无任何 `*_list`/`path_list` 类型（已逐个
    核对 options `type`），`kokoro phonemes`（string_list）与 `ming3_tts
    speaker_embedding_file`（path_list）两处既有缺口依旧未接。
  - 破坏性改名（上游 spec 层，无兼容保留）：`liveavatar` 请求选项
    `fused_cfg` → `batched_cfg`（`model_specs/liveavatar.json:98`，同时改写
    `denoiser_layerwise` 描述里的引用）。本仓 Rust 侧未硬编码旧键（全仓 grep 无
    `fused_cfg`），无需代码改动；手写旧键的调用方需自行更新（传旧键会被上游
    未知选项校验拒绝，经 `audiocpp_last_error` 报错，非静默）。
    本次 spec diff 中唯一的 `- "name"` 行即此处，其余全是加法。
  - 已同步 `audio-cpp/src/types.rs` 的 `ModelFamily`：新增 5 个枚举变体 `EarshotVad`
    （`as_str()` → `"earshot_vad"`）/ `NemotronTranscribe`（→ `"nemotron_transcribe"`）/
    `Lfm3Audio`（→ `"lfm3_audio"`）/ `MagentaRealtime2`（→ `"magenta_realtime_2"`）/
    `SoulxFlashhead`（→ `"soulx_flashhead"`，均与上游 loader 族名一致：
    每个族的 `kFamily` 常量都等于其 CMake target 名，无改名）+ `from_path()` 关键词表
    （`earshot_vad`/`earshot-vad`/`earshot`；`nemotron_transcribe`/`nemotron-transcribe`/
    `nemotron_3.5_transcribe`/`nemotron-3.5-transcribe`/`nemotron-3.5`/`nemotron_3.5`；
    `lfm3_audio`/`lfm3-audio`/`lfm3`；`magenta_realtime_2`/`magenta-realtime-2`/
    `magenta_rt2`/`magenta-rt2`/`magenta`；`soulx_flashhead`/`soulx-flashhead`/`soulx`/
    `flashhead`）+ `From<&str>`（含上游 `family_aliases()` 声明的 `earshot`）；
    并在两个 Cargo.toml 新增 `model-earshot-vad` / `model-nemotron-transcribe` /
    `model-lfm3-audio` / `model-magenta-realtime-2` / `model-soulx-flashhead`
    五个 feature（build.rs 自动映射 CMake target，无需改 build.rs）。
    - **`nemotron_transcribe` 的 from_path 排序陷阱（已加回归断言）**：上游权重目录名
      `NVIDIA-Nemotron-3.5-Transcribe-30B-A4B-GGUF` 与 encoder 侧 `mmproj-*` 文件小写后
      都含子串 **`nemotron-3`**，会被既有 `("nemotron-3", Nemotron3Diar)` 关键词抢先吞掉，
      故 Transcribe 的 6 条关键词必须**整体排在 diar 的 4 条之前**；反向断言
      `nemotron-3-diarization-bf16.gguf` 仍判 `Nemotron3Diar`、
      `lfm2.5-audio-1.5b-q8_0.gguf` 仍判 `Lfm2Audio` 一并锁定。
      另按上游真实文件名补了 13 条 `model_family_from_path` 断言
      （`earshot_vad.safetensors` / `nemotron-transcribe-30b-a4b-experts-iq4_nl.gguf` /
      `LFM3-Audio-2B-Q8_0.gguf` / `mmproj-LFM3-Audio-2B-F16.gguf` /
      `magenta-rt2-small-f16.gguf` / `SoulX-FlashHead-Lite-Q8_0.gguf` 等），
      `model_family_roundtrip` 的 `push![...]` 变体表已补 5 项。
  - 计数同步：`docs/models.md` 重跑生成器刷新（**135** 族 + 2 内置 VAD；
    ASR 32→34 / 语音分析 7→8 / 音频与音乐生成 9→11，TTS 51 不变，
    missing zh desc = 0），`scripts/model-desc-zh.tsv` 补 5 条中文简介（135 行数据）；
    `README.md`（3 处）/ `audio-cpp-sys/README.md` / 两个 Cargo.toml 的族数注释
    统一 130→**135**。注意 `audiocpp_add_model` 的 **target** 数是 130，
    去重 **loader** 名才是 135（一个 target 可挂多个 loader，`earshot_vad` 无 spec）。
  - 5 新族 session 均自持资产：earshot_vad 持 `shared_ptr<const EarshotWeights>`、
    nemotron_transcribe 持 `shared_ptr<const TranscribeAssets>`、
    magenta_realtime_2 持 `shared_ptr<const MagentaAssets>`；
    soulx_flashhead **间接**自持（session 存 `Pipeline`，`Pipeline::State` 持
    `shared_ptr<const Assets>`）；lfm3_audio 亦间接（session 存 `Model` 结构体，
    其 `assets` 为 `shared_ptr<const Lfm3AudioAssets>`，另持 `contract` shared_ptr）。
    `Session` 独立于 `Model` 存活的保证成立。
  - 验证：`cargo fmt --all -- --check` + `cargo test --workspace`（32 lib + 12 doc 全过，
    含本次新增断言；build.rs 跟踪 submodule HEAD 正常触发重编，2m24s）+
    `cargo clippy --workspace --all-targets` 零警告 + `cargo check -p audio-cpp-sys
    --features custom-models,model-earshot-vad,model-nemotron-transcribe,
    model-lfm3-audio,model-magenta-realtime-2,model-soulx-flashhead`
    （5 个 feature→CMake target 映射全部通过）。
  - 注意：`metadata.json.audio_commit` 与新 submodule HEAD 不一致会强制回落
    源码构建（预编译自动下载被跳过），属预期；`cargo publish` / tag /
    Release / 预编译资产上传均属第 6 节发布操作，须用户显式下达，本次未执行。
    上游本次范围内已发布 `v0.9.3` tag（`41c1f8ff`，2026-10-26），main 在其后 61 个
    提交；本仓按惯例跟 origin/main HEAD，不跟随 tag。
- **升级 `ccdbe769`→`e9303b76`（main）审查结论**：
  - diff 共 472 文件（+34.7k/-2.0k，77 个提交）：新增 5 个 loader 族
    （`audiocpp_add_model` 121→125，去重 loader 名 125→130 = spec 文件数 =
    `ModelFamily` 非内置族数，各处计数已统一改为 **130**）：
    `gradium_phonon`（Gradium Phonon 31M 英文 TTS，复用 PocketTTS 运行时：
    Mimi codec + flow 声学模型，预置 `default` 音色，支持导出/复用 voice
    state；task `tts` 离线+流式；**与 `pocket_tts` 同属 `pocket_tts`
    CMake target**，经 `ALIASES gradium_phonon` 作为 AUDIOCPP_MODELS 条目）/
    `latentsync`（LatentSync 1.6 音频驱动唇形同步，实验性；task `sfx`
    即 `gen` 仅离线，请求选项 `reference_video_file` 必填、需 FFmpeg）/
    `ming3_audio`（Ming 3.0 Audio Tiny：Whisper 编码器 + 混合 KDA/MLA MoE
    Thinker + 连续 latent Talker，zh/en；task `asr`/`s2s`/`tts`/`clon`
    仅离线，300+ 内置 Talker 音色）/ `ming3_tts`（Ming 3.0 TTS 0.5B：
    Qwen 主干 + flow matching 声学头 + CAMPPlus + 音频 VAE，7 语；
    task `tts`/`clon`/`vdes` 仅离线）/ `xtts_v2`（Coqui XTTS v2 跨语 TTS +
    克隆，17 语；task `tts` 仅离线，参考音频即说话人条件）。
    其余为 LoRA 合并工具 / RKNN sidecar 扩展 / 各 TTS 的 CUDA·Metal 性能
    选项与 `cuda_perf` 预设 / CJK 文本分块默认值调整 / GGUF 数组元数据批量
    读取 / server/WebUI（LatentSync 面板等）等内部改进。
  - **C ABI 边界无需改动**：capi.cpp 依赖的 5 个头中
    `framework/core/backend.h` / `framework/io/json.h` /
    `framework/runtime/session.h` 本次 diff **零改动**
    （`task_vocabulary.{h,cpp}` / `errors.h` / `options.h` 亦零改动，
    14 种 task 不变）。`model.h` 纯加法（`ModelEngineRequirement` /
    `ModelResourcePlan` + `IVoiceModelLoader::model_resources` 带默认实现
    的虚函数）、`registry.h` 纯加法（`Registry::model_resources`）、
    `spec_backed_model.h` 纯加法（可选 `model_resources` 回调 + override），
    均不经 C ABI；`runtime/kv_cache.h` 仅内部加法。`capi.h` / `capi.cpp` /
    build.rs bindgen allowlist（`audiocpp_.*`）保持原样，
    `bindings.docsrs.rs` 无需重拷。
  - 无新 task 类型 / 输出字段：5 新族复用既有 `tts`/`clon`/`vdes`/`asr`/
    `s2s`/`gen`（spec 的 `sfx`/`clone`/`design` 是既有别名），全部新选项
    （latentsync 视频/步数系、ming3 模板/音色/flow 系、gradium voice state/
    graph capacity 系、xtts 采样系）走通用 options 字符串透传。
    高层 `types.rs` serde 结构无需改。
  - **新增数组选项缺口（记录，不实现）**：`ming3_tts` 的
    `speaker_embedding_file` 类型为 `path_list`（上游 spec 层要求数组，
    走 `TaskRequest::option_arrays`）；本仓 shim 仍只暴露字符串 options
    （未接上游 opt-in C API 的 `audiocpp_request_set_option_array`），
    故该选项暂不可达。kokoro `phonemes`（`string_list`）缺口依旧。
  - 已同步 `audio-cpp/src/types.rs` 的 `ModelFamily`：新增 5 个枚举变体
    `GradiumPhonon`（`as_str()` → `"gradium_phonon"`）/ `Latentsync`
    （→ `"latentsync"`）/ `Ming3Audio`（→ `"ming3_audio"`）/ `Ming3Tts`
    （→ `"ming3_tts"`）/ `XttsV2`（→ `"xtts_v2"`，均与上游 loader 族名
    一致）+ `from_path()` 关键词表（`gradium_phonon`/`gradium`；
    `latentsync`/`latent-sync`；`ming3_audio`/`ming-3.0-audio` 置于裸
    `ming3` 之前；`ming3_tts`/`ming-3.0-tts` + 裸 `ming3` 兜底；
    `xtts_v2`/`xtts-v2`/`xtts`；`phonon-31m`/`phonon-40m` 对应打包权重名，
    **不收裸 `phonon`/`phonon-2`**，避免与 Parakeet Phonon-2 手动转换权重
    误判）+ `From<&str>`；并在两个 Cargo.toml 新增
    `model-gradium-phonon`（alias，等价 `model-pocket-tts`）/`model-latentsync`/
    `model-ming3-audio`/`model-ming3-tts`/`model-xtts-v2` 五个 feature
    （build.rs 自动映射 CMake target/alias）。`model_family_roundtrip` /
    `model_family_from_path`（含 `phonon-*` 返回 None、ming3_audio 不被
    `ming3` 吞掉的回归断言）测试已覆盖。
  - **顺带修补**：`audio-cpp/Cargo.toml` 缺 `model-plapre` /
    `model-open-wake-word` 的高层转发（sys 侧上次已加），本次补齐。
  - 5 新族 session 均自持资产（`shared_ptr<const *Assets>` + contract），
    `Session` 独立于 `Model` 存活的保证成立。
  - 验证：`cargo fmt --check` + `cargo build --workspace`（build.rs 跟踪
    submodule HEAD 正常触发全量重编，4m14s）+ `cargo test --workspace`
    （32 lib + 12 doc 全过）+ `cargo clippy --workspace --all-targets`
    零警告 + `cargo check -p audio-cpp-sys --features custom-models,
    model-gradium-phonon,model-latentsync,model-ming3-audio,
    model-ming3-tts,model-xtts-v2`（5 个 feature→CMake target/alias 映射
    全部通过，3m44s）。
  - 重跑 `pwsh -File scripts/gen-models-doc.ps1` 刷新 `docs/models.md`
    （130 族 + 2 内置 VAD；ASR 31→32 / TTS 48→51 / 音频与音乐生成 8→9），
    并在 `scripts/model-desc-zh.tsv` 补 5 条中文简介（missing zh desc = 0）。
    顺带修脚本 feature 归因：同 target 多 loader 时，主 loader（族名 = target
    名，如 `pocket_tts`）不再显示同 target 其它 loader 的专用 feature
    （`model-gradium-phonon`），次级 loader（如 `vibevoice_asr_streaming`、
    `htdemucs_6stems`、`moss_tts_nano`）仍显示 target 级 feature。
  - 注意：`metadata.json.audio_commit` 与新 submodule HEAD 不一致会强制回落
    源码构建（预编译自动下载被跳过），属预期；`cargo publish` / tag /
    Release / 预编译资产上传均属第 6 节发布操作，须用户显式下达，本次未执行。
- **升级 `22cad6af`→`ccdbe769`（main）审查结论**：
  - diff 共 260 文件（+18.7k/-2.1k，49 个提交）：新增 2 个 CMake target
    （`audiocpp_add_model` 119→121，去重 loader 名 123→125 = spec 文件数 =
    `ModelFamily` 非内置族数，各处计数已统一改为 **125**）：
    `plapre`（族 `plapre`，Plapre 3 丹麦语 TTS：Llama 自回归解码 + Kanade
    codec，内置 5 音色 `tor/ida/liv/ask/kaj`；task 为 `tts`/`clon`（spec 写
    `clone` 别名），离线+流式（流式仅出完整文本块）；克隆需 `reference_text`，
    另有 `voice_id`/`context_text`/`pronunciations`/`edit_start_frame`/
    `edit_end_frame` 等选项，依赖 `utf8proc_vendor`）/
    `open_wake_word`（族 `open_wake_word`，OpenWakeWord 唤醒词检测：共享语音
    嵌入 CNN + 转换分类器，英文 "alexa"，FF32 GGUF；task 为 `wake`（别名
    `wake_word`），离线+流式，结果走既有 `speech_segments`，请求选项
    `threshold`，会话选项 `exec_plan`（NEON/GGML 执行计划，x86 上不生效））。
    其余为 LoRA 适配器重构（`framework/assets/weight_adapter.{h,cpp}` 取代
    `lora_tensor_source.h`，ComfyUI/LoKr 支持 YuE2/VibeVoice/ACE-Step）/
    NEON 执行基础设施（`framework/neon/*`，仅 AArch64）/ fish_drama3 克隆与
    VRAM 优化（磁盘专家权重量化 `disk_expert_weight_store`、流式 Studio 面板）/
    i-quant（IQ）GGUF 张量存储类型 / server 每模型后端设备选择与
    `default_session_options` / 语音预设携带请求选项 / kitten_tts2 新 BF16/Q8/Q4
    权重 / RKNN 与 WebUI（越南语界面）等。
  - **C ABI 边界无需改动**：capi.cpp 依赖的 5 个头（`framework/core/backend.h` /
    `framework/io/json.h` / `framework/runtime/{model,registry,session}.h`）本次
    diff **零改动**（`task_vocabulary.{h,cpp}` 亦零改动，14 种 task 不变；
    `runtime/kv_cache.h` 仅加法 +22 行、`io/binary.h` 为内部实现，均不经 C ABI；
    新增 `framework/neon/*`、`framework/rknn/engine.h` 改动也不经 C ABI）。
    `capi.h` / `capi.cpp` / build.rs bindgen allowlist（`audiocpp_.*`）保持原样，
    `bindings.docsrs.rs` 无需重拷。
  - 无新 task 类型 / 输出字段：`open_wake_word` 复用既有 `wake` + `speech_segments`
    （shim 早已导出），`plapre` 复用既有 `tts`/`clon`（spec 的 `clone` 是
    `task_vocabulary` 里的既有别名，经通用任务字符串透传）；14 个改动 spec
    diff **无 `*_list`/array 类型**（`word_duration_frames`/`pronunciations`
    是 JSON 字符串选项），全部新选项走通用 options 字符串透传。
    高层 `types.rs` serde 结构无需改。
  - 已同步 `audio-cpp/src/types.rs` 的 `ModelFamily`：新增 2 个枚举变体
    `Plapre`（`as_str()` → `"plapre"`）/ `OpenWakeWord`（→ `"open_wake_word"`，
    与上游 loader 族名一致）+ `from_path()` 关键词（`plapre_3`/`plapre-3`/
    `plapre`；`open_wake_word`/`open-wake-word`/`openwake`，与 `micro_wake_word`
    系关键词互不干扰，已用 287+ 关键词全表回放验证新文件名命中顺序）+
    `From<&str>`；并在 `audio-cpp-sys/Cargo.toml` 与 `audio-cpp/Cargo.toml`
    新增 `model-plapre` / `model-open-wake-word` feature（build.rs 自动映射
    CMake target）；`model_family_roundtrip` / `model_family_from_path`
    （含 `open-wake-word` vs `microwake` 防吞断言）测试已覆盖。
  - 两族 session 均自持资产（`shared_ptr<const Assets>` / `shared_ptr<const
    PlapreAssets>`，spec-backed 含 contract），`Session` 独立于 `Model` 存活的
    保证成立。
  - 验证：`cargo fmt --check` + `cargo build --workspace`（build.rs 跟踪
    submodule HEAD 正常触发重编，48s）+ `cargo test --workspace`（32 lib +
    12 doc 全过）+ `clippy --workspace --all-targets` 零警告 +
    `cargo check -p audio-cpp-sys --features custom-models,model-plapre,
    model-open-wake-word`（CMake 日志 `custom selected [open_wake_word;plapre]
    linked [...]`，`engine_model_plapre` 6 obj / `engine_model_open_wake_word`
    4 obj 编译通过；`neon_embedding.cpp` 因 x86 无 `ENGINE_ENABLE_NEON` 不编，
    符合上游条件编译）。
  - 重跑 `pwsh -File scripts/gen-models-doc.ps1` 刷新 `docs/models.md`
    （125 族 + 2 内置 VAD，TTS 47→48 / 音频工具 22→23），并在
    `scripts/model-desc-zh.tsv` 补 `open_wake_word` / `plapre` 两条中文简介
    （missing zh desc = 0）。
  - 注意：`metadata.json.audio_commit` 与新 submodule HEAD 不一致会强制回落
    源码构建（预编译自动下载被跳过），属预期；`cargo publish` / tag /
    Release / 预编译资产上传均属第 6 节发布操作，须用户显式下达，本次未执行。
- **升级 `26b89278`→`22cad6af`（main，v0.9.2）审查结论**：
  - diff 共 144 文件（+33.3k/-1.3k，20 个提交）：新增 1 个 CMake target
    `fish_drama3`（族 `fish_drama3`，Fish Audio Drama 3 原生 MoE 语音生成/
    声音克隆/续写，社区模型；task 为 `tts` 长文（`long_form`）+ `clon`
    （`speaker_reference`），离线+流式；语言 `auto`；克隆需参考音频及其转写
    `reference_text`，多参考/续写走 `multi_reference_cond`/`references`/
    `previous_takes`/`previous_takes_file` JSON 字符串选项；`encode_reference_only`
    可仅编码参考，产出 `drama3_reference_v1` 声学 token 产物，走既有
    `output_artifacts`）。其余为 RKNN NPU sidecar 基础设施（`framework/rknn/
    engine.h` + `options.h` 新增 `ComponentExecPlan`/`parse_exec_plan`；
    qwen3_tts/kokoro_tts/piper_tts/inflect_v2 的 opt-in NPU 解码/生成 sidecar，
    仅上游可选构建）/ granite5asr CTC 词级时间戳 / miotts 重复惩罚按 token
    去重 / Vulkan 上取整溢出修复 / server 配置示例独立发布资产 / WebUI
    Voice Library + Arena 等。上游 model target 118→119 个（`audiocpp_add_model`
    计数；含 `fish_drama3`）。
  - **C ABI 边界无需改动**：capi.cpp 依赖的 5 个头（`framework/core/backend.h` /
    `framework/io/json.h` / `framework/runtime/{model,registry,session}.h`）本次
    diff **零改动**（`task_vocabulary.h` / `session.h` 亦零改动；framework 下仅
    `runtime/options.h` 新增 RKNN 组件执行计划解析 + `runtime/spec_backed_model.h`
    的未知选项报错信息增强（附加拼写提示与上游链接），均不经 C ABI）。
    `capi.h` / `capi.cpp` / build.rs bindgen allowlist 保持原样。
  - 无新 task 类型 / 输出字段：新族用既有 `tts`/`clon`（`drama3_reference_v1`
    产物走既有 `output_artifacts`）；granite5asr 时间戳走既有 `word_timestamps`；
    全部新选项（fish_drama3 系、RKNN `exec_plan` 等）走通用 options 字符串透传，
    spec diff 无 `*_list`/array 类型，无数组选项缺口。高层 `types.rs` serde
    结构无需改。
  - 已同步 `audio-cpp/src/types.rs` 的 `ModelFamily`：新增 1 个枚举变体
    `FishDrama3`（`as_str()` → `"fish_drama3"`，与上游 loader 族名一致）+
    `from_path()` 关键词（`fish_drama3`/`fish-drama3`，置于 `fish_audio` 之前）
    + `From<&str>`；并在 `audio-cpp-sys/Cargo.toml` 与 `audio-cpp/Cargo.toml`
    新增 `model-fish-drama3` feature（build.rs 自动映射 CMake target）。
    `model_family_roundtrip` / `model_family_from_path`（含 fish_drama3 与
    fish_audio 互不误判的回归断言）测试已覆盖。
  - **顺带修补历史遗漏**：`voxtral_tts` 是上游独立 loader 族（`audiocpp_add_model
    (voxtral_tts ... make_voxtral_tts_loader)`，spec `model_specs/voxtral_tts.json`，
    Mistral Voxtral 4B TTS，task `tts` 离线+流式）却从未收进 `ModelFamily`——本次
    新增 `VoxtralTts`（`as_str()` → `"voxtral_tts"`）+ `from_path()` 关键词
    （`voxtral_tts`/`voxtral-tts`，**置于裸 `"voxtral"` 之前**，否则会被误吞判成
    `VoxtralRealtime`）+ `From<&str>` + 回归断言。`voxtral_tts` 无专用 feature
    （与 `voxtral_realtime` 一致），用 `custom-models` + `AUDIOCPP_MODELS`。
  - **计数修正**：此前 `full-models` 注释 / sys README 的 loader 族计数（117）
    陈旧且有偏差；按上游 CMakeLists 实际统计，去重 loader 数 122→123（= spec
    文件数 = `ModelFamily` 非内置族数），已把 Cargo.toml 注释与 sys README 统一
    改为 **123**。
  - 新增 `docs/models.md`：由上游 `model_specs/*.json` 生成的全模型目录
    （123 族 + 2 内置 VAD，按任务分类，每族给出 family / 显示名 / 运行时任务
    token / 离线·流式 / 语言 / 上游状态 / 专用 feature / 中文一句话简介 + 上游文档
    链接）；README 增加「模型目录」入口与链接。生成器与中文简介表已入库：
    `scripts/gen-models-doc.ps1` + `scripts/model-desc-zh.tsv`（升级 submodule 后
    重跑即可刷新，缺简介的族会有 warning 提示）。
  - 新族 session 自持资产（`shared_ptr<const Drama3Assets>`，spec-backed），
    `Session` 独立于 `Model` 存活的保证成立。
  - 验证：`cargo fmt --check` + `cargo build --workspace`（build.rs 跟踪
    submodule HEAD 正常触发重编，约 1.5 分钟）+ `cargo test --workspace`
    （32 lib + 12 doc 全过）+ `clippy --workspace --all-targets` 零警告 +
    `cargo check -p audio-cpp-sys --features custom-models,model-fish-drama3`
    （新 feature→CMake target 映射抽查通过，`engine_model_fish_drama3` 及其
    utf8proc 依赖对象均已编译）。
  - 注意：`metadata.json.audio_commit` 与新 submodule HEAD 不一致会强制回落
    源码构建（预编译自动下载被跳过），属预期；`cargo publish` / tag /
    Release / 预编译资产上传均属第 6 节发布操作，须用户显式下达，本次未执行。
- **升级 `886076fc`→`26b89278`（main）审查结论**：
  - diff 共 291 文件（+9.0k/-6.3k，23 个提交）：**无新增 `make_*_loader`**
    （新旧 CMakeLists 提取的 loader 名单比对一致）；其余为 server 大重构
    （`runtime.cpp` / `parallel_runtime.cpp` 逻辑搬入新增 `server_protocol.cpp` /
    `server_ui.cpp` / `request_dispatch.h`，纯 HTTP 层，不经 C ABI）/
    irodori_tts 新增流式模式（同族既有 loader，shim 流式通用支持）/
    moss_tts_nano 增量解码 + KV cache + 参考音色缓存（内部性能）/
    Vevo2 与 ACE-Step 重构 / 模型选项规范化（下述破坏性改名）/
    parakeet Vulkan 传输优化 / 文档与示例补全。
  - **C ABI 边界无需改动**：capi.cpp 依赖的 5 个头（`framework/core/backend.h` /
    `framework/io/json.h` / `framework/runtime/{model,registry,session}.h`）本次
    diff **零改动**（`errors.h` / `task_vocabulary` 亦零改动；framework 下唯一
    改动是 `text/chunking.cpp` 的整数严格校验，抛 `std::runtime_error`，
    shim 统一 `catch (const std::exception&)` 捕获）。`capi.h` / `capi.cpp` /
    build.rs bindgen allowlist 保持原样。
  - 无新 task 类型 / 输出字段：`TaskResult` / `StreamEvent` 结构体不变
    （`dump_*` 无需补字段）；spec diff 无 `*_list` 类型，无数组选项缺口；
    全部新选项（ace_step `route` / `bpm` 系、irodori `stream_frames_per_event`
    系等）走通用 options 字符串透传。高层 `types.rs` / `options.rs` serde
    结构无需改（`options.rs` 未引用任何已删键，已 grep 核实）。
  - 破坏性改名（上游 spec 层，无兼容保留，手写旧键的调用方需自行更新）：
    ace_step 删 `duration_sec` / `lm_guidance_scale` / `enable_thinking`；
    liveavatar 删 `support_gguf` / `vae_gguf` / `denoiser_gguf`（改为 load-only）；
    vevo2 删 6 个 `*_graph_arena_mb`；voxcpm1 删 `weight_type` /
    `audiovae_weight_type`（改为 session-only）。本仓 Rust 侧无硬编码旧键，
    无需代码改动。
  - 改动族 session 抽查（irodori/vevo2/moss_nano）均 `shared_ptr` 自持资产，
    `Session` 独立于 `Model` 存活的保证成立。
  - 验证：`cargo fmt --check` + `cargo build --workspace`（build.rs 跟踪
    submodule HEAD 正常触发增量重编，约 2.5 分钟）+ `cargo test --workspace`
    （32 lib + 12 doc 全过）+ `clippy --workspace --all-targets` 零警告。
  - 注意：`metadata.json.audio_commit` 与新 submodule HEAD 不一致会强制回落
    源码构建（预编译自动下载被跳过），属预期；`cargo publish` /
    tag / Release / 预编译资产上传均属第 6 节发布操作，须用户显式下达，
    本次未执行。
- **补记**：`73c55549`→`886076fc` 的指针前移由 `8679345`
 （fix(options)：会话选项改嵌套形状）顺带提交，未留审查记录；
  本次已将已知状态链补齐，crate 版本 **0.8.0**（`c6a026d` 升版）。
- **升级 `d3ab9df2`→`73c55549`（main，v0.9.1-33）审查结论**：
  - diff 共 506 文件（+47.9k/-5.3k，66 个提交）：新增 9 个 CMake target
    （上游 model target 108→117 个）：`mossformer2`（MossFormer2 重叠语音分离，
    16kHz，task `sep` 仅离线）/ `tf_gridnet`（时频 GridNet 重叠语音分离，
    task `sep` 仅离线）/ `firered_vad`（FireRed DFSMN VAD，16kHz mono，
    task `vad` 离线+流式，流式需 Stream-VAD checkpoint）/ `ced`
    （CED ViT 音频分类，527 个 AudioSet 类别，task `cls` 仅离线，结果走
    `custom_schema_output` schema `ced.classification.v1`）/ `ast_audioset`
    （AST 多标签分类，527 类，task `cls` 仅离线，schema
    `ast_audioset.classification.v1`）/ `micro_wake_word`（microWakeWord
    MixedNet 唤醒词检测，16kHz mono，task `wake` 离线+流式，结果走既有
    `speech_segments`）/ `sherpa_kws`（sherpa-onnx Zipformer transducer
    关键词检测，16kHz mono，task `wake` 离线+流式，结果走既有
    `speech_segments`）/ `hviske_asr_v6`（丹麦语 rotary-Whisper 编码器 +
    Qwen3 解码器，task `asr` 仅离线）/ `whistle_asr`（社区离线 ASR，
    task `asr` 仅离线）。其余为 parakeet Phonon-2 热词/流式/时间戳 +
    `partial_text_snapshot` / MOSS-TTS-Nano 与 VibeVoice/Fish 流式化 /
    Kokoro/NeuTTS/VoxCPM2 流式改进 / sanotts ARM NEON CPU 解码选项 /
    server 模型参数透出（`GET /v1/models?include_params=true`）等。
  - **C ABI 边界有加法改动**：`backend.h` 新增 CPU extra-buffer 自由函数
    （shim 不用）/`json.h` 零改动；`model.h`/`registry.h` 零改动（registry 仅
    新增 `finish_model_load_trace` 自由函数，shim 不调用）；`session.h` 新增
    `VoiceTaskKind::WakeWord` / `VoiceTaskKind::AudioClassification`
    （shim 经 `parse_voice_task_kind`/`to_string` 字符串透传，自动兼容）+
    `StreamEvent::partial_text_snapshot`（可选可修正整段假设，server
    `task_stream_format` 已透出）。`errors.h` 新增 `InvalidRequestError`
    （派生自 `std::exception`，shim 统一 `catch (const std::exception&)`
    捕获；server 侧映射 400，本仓 shim 无 HTTP 码，错误字符串透传即可）。
    已同步 `capi.cpp` `dump_stream_event` 导出 `partial_text_snapshot`
    （与 `partial_text` 同形 `{text,language}`，缺省不输出）；`capi.h` /
    build.rs bindgen allowlist（`audiocpp_.*`）不变（无签名改动，
    `bindings.docsrs.rs` 无需重拷）。
  - 无其余新输出字段：9 新族中 7 个复用既有 `sep`/`vad`/`asr`/`wake`
    （`speech_segments` 早已导出）；ced/ast 走既有 `custom_schema_output`
    （shim 早已导出）；全部新选项（mossformer2/tf_gridnet 分块、
    firered_vad 阈值系、micro/sherpa 阈值系、hviske/whistle 解码系）走通用
    options 字符串透传（新 spec 无 `*_list` 类型，无数组选项缺口）。
  - 已同步 `audio-cpp/src/types.rs` 的 `TaskKind`：新增 `WakeWord`
    （`as_str()` → `"wake"`，`Deserialize` 兼收别名 `"wake_word"`）/
    `AudioClassification`（→ `"cls"`，兼收 `"classify"`，与上游
    `task_vocabulary` 规范名/别名一致）+ `task_kind_aliases` 测试；
    `StreamEvent` 新增 `partial_text_snapshot: Option<TextOutput>`
    （`#[serde(default)]` 向后兼容）+ 反序列化测试覆盖。
    已同步 `ModelFamily`：新增 9 个枚举变体（`as_str()` 与上游族名一致）+
    `from_path()` 关键词表（`hviske_asr_v6` 置于裸 `"hviske"` 之前；
    `mossformer2` 置于裸 `"moss"` 之前；`ced` 置于 `"forced-aligner"`
    之后——`ced` 是 for**ced** 的子串；`ast` 仅收 `ast_audioset`/
    `ast-audioset`/`audioset` 不收裸 `ast` 防误伤）+ `From<&str>`
    （下划线规范名 + 连字符别名）；并在两个 Cargo.toml 新增 9 个
    `model-*` feature（build.rs 自动映射 CMake target）；full-models
    注释 108→117，sys README 计数字段 108→117。`model_family_roundtrip` /
    `model_family_from_path`（含防吞断言：hviske/mossformer2/ced/forced-aligner）
    测试已覆盖。
  - 9 新族 session 均自持资产（`shared_ptr<const …>`，spec-backed），
    `Session` 独立于 `Model` 存活的保证成立。
  - 验证：`cargo fmt --check` + `cargo build --workspace`（build.rs 跟踪
    submodule HEAD 正常触发重编，约 3 分钟）+ `cargo test --workspace`
    （32 lib + 12 doc 全过）+ `clippy --workspace --all-targets` 零警告 +
    `cargo check -p audio-cpp-sys --features custom-models,model-sherpa-kws`
    （新 feature→CMake target 映射抽查通过）。
  - 注意：`metadata.json.audio_commit` 与新 submodule HEAD 不一致会强制回落
    源码构建（预编译自动下载被跳过），属预期；`cargo publish` /
    tag / Release / 预编译资产上传均属第 6 节发布操作，须用户显式下达，
    本次未执行。
- **升级 `fc24c99e`→`d3ab9df2`（main）审查结论**：
  - diff 共 873 文件（+54.7k/-5.3k，约 74 个提交）：新增 17 个 CMake target
    （上游 model target 91→108 个；loader 94→112 个，另 `demucs` target 内
    新增第二 loader `make_htdemucs_6stems_loader`）：`audio_flamingo`
    （Audio Flamingo 3/Next，转写/描述/问答，task `asr` 仅离线）/
    `crisperwhisper`（CrisperWhisper 2.0 逐字/意图转写，task `asr`/`align`，
    离线+流式）/ `gigaam_asr`（GigaAM v3/多语 Conformer，CTC/RNN-T，
    task `asr` 仅离线）/ `index_echo`（Index-Echo-S2TT 翻译+双语时间戳字幕，
    task `asr` 仅离线）/ `kitten_tts2`（Kitten TTS 2 多语 Qwen3+克隆，
    task `tts`/`clon` 仅离线）/ `kugelaudio`（KugelAudio-0-Open 预置音色，
    task `tts` 离线+流式）/ `lfm2_audio`（LFM2.5-Audio，task `asr` 仅离线）/
    `maya1`（Maya1 3B 表现力英文 TTS，task `tts` 仅离线）/ `moss_ttsd`
    （MOSS-TTSD 8B 对话 TTS 多说话人单遍+逐人克隆，task `tts`/`clon` 仅离线）/
    `owsm` / `owsm_ctc`（ESPnet 多语 ASR/翻译，task `asr` 离线+流式）/
    `reuse`（RE-USE 语音修复双向 Mamba，task `s2s` 仅离线）/ `sam_audio`
    （prompt 条件音频分离，task `s2s` 仅离线）/ `samsone`（紧凑音频语言模型，
    task `asr` 仅离线）/ `sidon`（单说话人修复 48kHz，task `s2s` 仅离线）/
    `smart_turn`（Smart Turn v3.2 轮次完成检测，task `turn` 仅离线，
    16kHz mono，结果走新增 `custom_schema_output`）/ `tone_color_vc`
    （参考说话人 VC，task `vc` 仅离线）；另 `htdemucs_6stems`
    （族 `htdemucs_6stems` 别名 `htdemucs_6s`，鼓/贝斯/人声/其他/吉他/钢琴六轨，
    task `sep` 仅离线，与 `htdemucs` 同属 `demucs` target）。
    其余为 server 并行运行时（`parallel_runtime` + `/v1/tasks/batch` +
    `audio_base64` 内联音频 + CORS 错误透传）/ nemotron_asr 说话人标签与
    masked ASR / demucs 6-stem 管线 / 框架 decoder/encoder 重命名
    （qwen→causal/greedy/generic）/ espeak 移至 `framework/text` /
    音频工具移至 `framework/audio/utilities` / SNAC 解码器 / host_memory 等。
  - **C ABI 边界有加法改动**：`backend.h`/`model.h`/`registry.h` 零改动；
    `json.h` 仅新增 `enable_serialized_json_parsing()`（shim 不用）；
    `session.h` 新增 `VoiceTaskKind::TurnDetection`（shim 经
    `parse_voice_task_kind`/`to_string` 字符串透传，自动兼容）+
    `CustomSchemaOutput` 与 `TaskResult::custom_schema_output`
    （smart_turn 唯一输出，shim 原先会丢弃）。已同步 `capi.cpp`
    `dump_task_result` 导出 `custom_schema_output`
    （`{"schema","data"}`，与 server `runtime.cpp` 同形）；build.rs bindgen
    allowlist（`audiocpp_.*`）不变。高层新增 `TaskKind::TurnDetection`
    （`"turn"`）+ `CustomSchemaOutput`（`schema` + `serde_json::Value data`）+
    `TaskResult::custom_schema_output`（`Option`，两端 serde 向后兼容）。
    server 批量端点（`IBatchedOfflineVoiceTaskSession`/`run_batch`，上次已评估）
    本仓 shim 仍未暴露，属既有契约外能力，未接。
  - 无其余新 task 类型 / 输出字段：17 新族中 16 个复用既有 `asr`/`tts`/
    `clon`/`vc`/`s2s`/`sep`/`align`；全部新选项走通用 options 字符串透传。
    高层其余 serde 结构无需改。
  - 已同步 `audio-cpp/src/types.rs` 的 `ModelFamily`：新增 18 个枚举变体
    （17 新 target 各一 + `Htdemucs6Stems`；`as_str()` 与上游族名一致）+
    `from_path()` 关键词表（`kitten_tts2`/`index_echo`/`moss_ttsd`/
    `owsm_ctc`/`htdemucs_6stems` 各置于其前缀家族之前；既有
    `htdemucs-6s` 断言改判新变体）+ `From<&str>`（含 `htdemucs_6s` 别名）；
    并在两个 Cargo.toml 新增 17 个 `model-*` feature（build.rs 自动映射
    CMake target；`htdemucs_6stems` 与 `htdemucs` 同 target，**不**建 feature，
    用既有 `model-demucs` 即可，否则 CMake 报 Unknown entry——沿用
    `vibevoice_asr_streaming` 先例，文档已注明）；full-models 注释 91→108，
    sys README 计数字段 91→108。`model_family_roundtrip` /
    `model_family_from_path`（含 5 组防吞断言）/ `task_kind_*` 测试已覆盖。
  - 18 新族 session 均自持资产（`shared_ptr<const …>`，spec-backed 含
    smart_turn 在内），`Session` 独立于 `Model` 存活的保证成立。
  - 验证：`cargo fmt --check` + `cargo build --workspace`（build.rs 跟踪
    submodule HEAD 正常触发全量重编，约 5 分钟）+ `cargo test --workspace`
    （31 lib + 12 doc 全过）+ `clippy --workspace --all-targets` 零警告 +
    `cargo check -p audio-cpp-sys --features custom-models,model-smart-turn`
    （新 feature→CMake target 映射抽查通过）。
  - 注意：`metadata.json.audio_commit` 与新 submodule HEAD 不一致会强制回落
    源码构建（预编译自动下载被跳过），属预期；`cargo publish` /
    tag / Release / 预编译资产上传均属第 6 节发布操作，须用户显式下达，
    本次未执行。
- **升级 `eb8e21bd`→`fc24c99e`（main / v0.8.2）审查结论**：
  - diff 共 160 文件（+13.2k/-1.9k，约 35 个提交）：新增 1 个 CMake target
    `nemotron_3_diar`（族 `nemotron_3_diar`，NVIDIA Nemotron 3 Diarization
    八说话人分离，arrival-order 身份，task `diar`，离线+原生批+流式，
    16kHz 输入，BF16 GGUF 需本地从 `.nemo` 转换，OpenMDW-1.1 许可）；
    另 1 个 target **改名** `vietneu_tts`→`vieneu_v3_turbo`（族名同步改，
    旧名保留为上游别名；VieNeu-TTS v3 Turbo，vi/en 48kHz TTS+即时克隆，
    task `tts`/`clon` 仅离线，文本须 SEA-G2P 音素）；上游 model target
    共 91 个（+1，loader 清单 94 个）。
    其余为 server 新增原生批量转录端点（multipart 多文件 + `run_batch`
    回调式进度，SSE diarization 事件）/ yue2 `semantic_prefix`/
    `semantic_prefix_file`（从给定 semantic token 续写歌曲）+ `lyrics`
    改为可空（纯器乐）/ moonshine_asr `audio_chunk_mode`/
    `audio_chunk_duration_sec` 长音频分块（vad_chunking）/
    moss_tts_v15 spec 补 text chunking 选项 / auk 模型包下载元数据 /
    seed-vc V1 F0 条件修复 / pocket_tts UTF-8 文本分块 + `*_24l` flow
    depth 加载修复 / audio8_tts Falcon-H1（Mamba2+attention）支持 +
    KV-cache / higgs CUDA KV 过渡优化 / BS-RoFormer CUDA 加速 /
    gguf 量化按行并行 / Windows engine core UTF-8 编译 /
    moss codec Nano 解码器窗口修复。
  - **C ABI 边界无需改动**：capi.cpp 依赖的 5 个头中 4 个
    （`backend.h`/`json.h`/`model.h`/`registry.h`）diff 零改动；
    `session.h` 仅**纯加法**新增 `IBatchedOfflineVoiceTaskSession`
    （server 批量端点专用虚接口，shim 不实现不依赖，现有
    `IOfflineVoiceTaskSession` 不受影响）；`task_vocabulary` /
    `spec_backed_model.h` / `app/server/runtime.cpp` 的 shim 依赖面
    零改动（runtime.cpp 虽 +302 行但仅 server HTTP 层）。
    `capi.h` / `capi.cpp` / build.rs bindgen allowlist 保持原样。
  - 无新 task 类型 / 输出字段：新 diar 族复用既有 `diar` +
    `speaker_turns`（shim 早已导出）；yue2/moonshine/moss/auk 新选项
    全部走通用 options 字符串透传；server 批量端点不经 C ABI（本仓
    shim 未暴露批量接口，属既有单请求契约外的新能力，未接）。
    高层 `types.rs` serde 结构无需改。
  - 已同步 `audio-cpp/src/types.rs` 的 `ModelFamily`：
    ① 新增 `Nemotron3Diar`（`as_str()` → `"nemotron_3_diar"`，与上游
    loader 族名一致）+ `from_path()` 关键词（`nemotron_3_diar` 等 4 条
    置于裸 `"nemotron"` 之前避免吞掉）+ `From<&str>`；
    ② **改名** `VietneuTts` → `VieneuV3Turbo`（`as_str()` →
    `"vieneu_v3_turbo"` 与上游新族名一致；`From<&str>` 同时接受新名
    与旧名 `vietneu_tts`；`from_path` 关键词 `vieneu`/`vietneu` 并存；
    枚举变体改名属 0.6.0 semver 次要破坏性变更，已在版本号体现）；
    并在两个 Cargo.toml 新增 `model-nemotron-3-diar` /
    `model-vieneu-v3-turbo` feature（build.rs 自动映射 CMake target，
    改名 target 用新名 feature）；full-models 注释 90→91，README
    计数 90→91。`model_family_roundtrip` / `model_family_from_path`
    测试已覆盖新变体与改名。
  - 两族 session 均自持资产（nemotron_3_diar 持
    `shared_ptr<const Assets>` + contract；vieneu 持
    `shared_ptr<const VieNeuTTSAssets>`），`Session` 独立于 `Model`
    存活的保证成立。
  - 版本：`workspace.package.version` 0.5.0 → **0.6.0**，
    `workspace.dependencies.audio-cpp-sys` 版本同步 0.6.0，
    `audio-cpp-sys/README.md` 版本引用四处同步。
  - 验证：`cargo fmt --check` + `cargo build --workspace` +
    `cargo test --workspace` + `clippy --workspace --all-targets` 零警告 +
    `cargo check -p audio-cpp-sys --features custom-models,model-nemotron-3-diar`
    （新 feature→CMake target 映射抽查）。
  - 注意：`metadata.json.audio_commit` 与新 submodule HEAD 不一致会强制
    回落源码构建（预编译自动下载被跳过），属预期；`cargo publish` /
    tag / Release / 预编译资产上传均属第 6 节发布操作，须用户显式下达，
    本次未执行。
- **升级 `e3de8e3f`→`eb8e21bd`（main）审查结论**：
  - diff 共 110 文件（+7.0k/-3.2k，13 个提交）：新增 1 个 CMake target
    `moss_tts_v15`（族 `moss_tts_v15`，MOSS-TTS-v1.5：8B delay-pattern
    零样本声音克隆 TTS，Qwen3-8B backbone + 32 codebook + MOSS-Audio-Tokenizer
    v1 24kHz codec，en/zh，task `tts`/`clon` 仅离线；克隆需参考音频，
    请求选项 `tokens`/`instruct`/`language`/`moss_tts_v15.weight_type`）；
    其余为 yue2 `export_semantic`/`stop_after`（abc|semantic|audio）+
    `yue2.attention_tile_rows`（eager NAR 注意力分块，长曲适配 4GiB buffer）
    + semantic token 流经既有 `output_artifacts` 导出（kind Custom id
    `semantic`，mime `application/vnd.yue2.semantic+json`）/ kokoro_tts
    `return_timestamps`（音素组级 word timings，走既有 `word_timestamps`）/
    共享 mel 与参考音频前端（`framework/audio/{mel_spectrogram_frontend,
    reference_audio_frontend}`，chatterbox/confucius4/dots/index_tts2/seed_vc
    等 audio_features 迁移，内部重构）/ nemotron ASR 流式改进 /
    roformer CUDA chunk 流水线 / moss_tts_delay 解码器从 moss_voicegen
    提升为框架共享层（`framework/decoders/moss_tts_delay`）/
    vulkan-shaders-gen 编译器启动失败重试 + 内存压力退避。
    上游 model target 共 90 个（+1）。
  - **C ABI 边界无需改动**：capi.cpp 依赖的 5 个头（`framework/core/backend.h` /
    `framework/io/json.h` / `framework/runtime/{model,registry,session}.h`）本次
    diff **零改动**（`task_vocabulary` 未动；`spec_backed_model.h` 未动；
    `app/server/runtime.cpp` 未动）。`capi.h` / `capi.cpp` / build.rs bindgen
    allowlist 保持原样。
  - 无新 task 类型 / 输出字段：新族用既有 `tts`/`clon`；yue2 semantic 走
    既有 `output_artifacts`（shim 早已导出），kokoro 时间戳走既有
    `word_timestamps`；全部新选项（yue2/kokoro/moss_tts_v15 系）经通用
    options 字符串透传。高层 `types.rs` serde 结构无需改。
  - 已同步 `audio-cpp/src/types.rs` 的 `ModelFamily`：新增 1 个枚举变体
    `MossTtsV15`（`as_str()` → `"moss_tts_v15"`，与上游 loader 族名一致）+
    `from_path()` 关键词表（`moss_tts_v15`/`moss-tts-v15`，置于裸 `"moss"`
    之前避免子串误判）+ `From<&str>`；并在 `audio-cpp-sys/Cargo.toml` 与
    `audio-cpp/Cargo.toml` 新增 `model-moss-tts-v15` feature（build.rs 自动
    映射 CMake target）；full-models 注释 89→90，README 计数 89→90。
    `model_family_roundtrip` / `model_family_from_path` 测试已覆盖新变体。
  - 新族 session 自持资产（`std::shared_ptr<const Assets> assets_`），
    `Session` 独立于 `Model` 存活的保证成立。
  - 验证：`cargo fmt --check` + `cargo build --workspace`（build.rs 跟踪
    submodule HEAD 正常触发重编，56.5s）+ `cargo test --workspace`
    （31 lib + 12 doc 全过）+ `clippy --workspace --all-targets` 零警告 +
    `cargo check -p audio-cpp-sys --features custom-models,model-moss-tts-v15`
    （新 feature→CMake target 映射抽查通过）。
  - 注意：`metadata.json.audio_commit` 与新 submodule HEAD 不一致会强制回落
    源码构建（预编译自动下载被跳过），属预期；重新发布预编译资产须用户显式
    下达发布命令（第 6 节），本次未执行。
- **升级 `219a7e0e`→`e3de8e3f`（main）审查结论**：
  - diff 共 93 文件（+10.6k/-1.1k，11 个提交）：新增 2 个 CMake target
    `piper_tts`（族 `piper_tts`，Piper VITS 社区 TTS，首包英文
    `en_US-lessac-medium`，22.05kHz mono，task `tts` 仅离线，eSpeak-ng
    前端，请求选项 `speed`/`variation`/`duration_variation`/`seed`）/
    `kitten_tts`（族 `kitten_tts`，KittenTTS Mini 0.8 英文 TTS，80M 参数，
    8 个内置声音，24kHz mono，task `tts` 仅离线，请求选项 `speed`/`seed`，
    `--voice-id` 选声音，`--speaking-rate` 通用语速）；其余为全 TTS 语速
    支持（supertonic `speaking_rate` 改名 `speed` 并保留别名、kokoro 新增
    `speed` 选项兼容旧 contract、server `build_speech_request` 统一接受
    `speed`/`speaking_rate` 并写入 voice style）/ NeMo mel 前端共享化
    （canary/cohere/citrinet/parakeet/sortformer/nemotron/hviske/granite5asr
    等 ASR 前端迁移到 `framework/audio/nemo_mel_frontend`，内部重构）/
    ASR 编码器图过大自动重建（`asr_graph_capacity_usable`，10% 容差）/
    voxcpm2 流式左上下文（`stream_left_context`，默认 3，修 patch 边界咔哒声）
    + Vulkan 也用 stateful 流式解码 / voxcpm1 Vulkan stateful 解码修 /
    live 转录路由新增 `prompt`（热词）查询参数 + `language` 改 URL 解码 /
    vulkan-shaders-gen 空着色器编译重试 / WebUI bundle 可复现 + svelte
    config / CI 本地运行脚本。上游 model target 共 89 个（+2）。
  - **C ABI 边界无需改动**：capi.cpp 依赖的 5 个头（`framework/core/backend.h` /
    `framework/io/json.h` / `framework/runtime/{model,registry,session}.h`）本次
    diff **零改动**（`task_vocabulary` 未动；`spec_backed_model.h` 未动）。
    `capi.h` / `capi.cpp` / build.rs bindgen allowlist 保持原样。
  - 无新 task 类型 / 输出字段：2 新族均用既有 `tts`（仅离线）；新选项
    （piper/kitten/kokoro 的 `speed`、supertonic 的 `speed` 别名、
    voxcpm2 的 `stream_left_context`、kitten 的权重/图容量会话选项）全部走
    通用 options 字符串透传，无需 Rust 改动。`speaking_rate` 在
    `VoiceCondition` style 字段与 server 侧均保留兼容。高层 `types.rs`
    serde 结构无需改。
  - 已同步 `audio-cpp/src/types.rs` 的 `ModelFamily`：新增 2 个枚举变体
    `PiperTts`（`as_str()` → `"piper_tts"`）/ `KittenTts`（→ `"kitten_tts"`，
    与上游 loader 族名一致）+ `from_path()` 关键词表
    （`piper_tts`/`piper-tts`/裸 `piper`，`kitten_tts`/`kitten-tts`/裸 `kitten`）+
    `From<&str>`；并在 `audio-cpp-sys/Cargo.toml` 与 `audio-cpp/Cargo.toml`
    新增 2 个 `model-*` feature（build.rs 自动映射 CMake target）；
    full-models 注释 87→89，README 计数字段 87→89。
    `model_family_roundtrip` / `model_family_from_path` 测试已覆盖新变体。
  - 验证：`cargo fmt --check` + `cargo build --workspace`（build.rs 跟踪
    submodule HEAD 正常触发增量重编）+ `cargo test --workspace`
    （31 lib + 12 doc 全过）+ `clippy --workspace --all-targets` 零警告 +
    `cargo check -p audio-cpp-sys --features custom-models,model-piper-tts`
    （新 feature→CMake target 映射抽查通过）。
    2 新族 session 均自持资产（piper 持 `shared_ptr<const PiperTtsAssets>`；
    kitten 持 `shared_ptr<const KittenAssets>`），`Session` 独立于 `Model`
    存活的保证成立。
  - 注意：`metadata.json.audio_commit` 与新 submodule HEAD 不一致会强制回落
    源码构建（预编译自动下载被跳过），属预期；重新发布预编译资产须用户显式
    下达发布命令（第 6 节），本次未执行。
- **升级 `c0b26a50`→`219a7e0e`（main）审查结论**：
  - diff 共 208 文件（+34.3k/-1.5k，16 个提交）：新增 4 个 CMake target
    `zipvoice`（族 `zipvoice`，k2-fsa ZipVoice / ZipVoice-Distill 流匹配
    TTS，task `tts`+`clon` 仅离线，en/zh，零样本克隆需 `reference_text`）/
    `liveavatar`（族 `liveavatar`，Wan S2V 音频驱动数字人，task `sfx`
    即 `gen`，离线+流式）/ `confucius4_r2t2`（族 `confucius4_r2t2`，
    网易 Confucius4-R2T2 实时流式 ASR，task `asr` 离线+流式）/ `auk`
    （族 `auk`，腾讯 AuK Base / AuK-Flash 语音生成与编辑，task `tts`+
    `edit` 仅离线，链接 `yaml_vendor`）；其余为 vibevoice_asr_streaming
    新增 1.5B 尺寸（同 loader 已兼容，无新族）/ breeze_tts 新增会话选项
    `bf16_activations`（`auto`/`on`/`off`）/ CLI `--out-format` /
    CUDA lowering 与 SSM gate fusion / yue2 NAR parity probe 等。
    上游 model target 共 87 个（+4）。
  - **C ABI 边界无需改动**：capi.cpp 依赖的 5 个头（`framework/core/backend.h` /
    `framework/io/json.h` / `framework/runtime/{model,registry,session}.h`）本次
    diff **零改动**（`task_vocabulary` 未动，仍 14 种；`spec_backed_model.h` 未动）。
    `capi.h` / `capi.cpp` / build.rs bindgen allowlist 保持原样。
  - 无新 task 类型 / 输出字段：4 新族均用既有 `tts` / `clon` / `edit`（`gen`
    别名）/ `asr` / `sfx`（`gen` 别名）；高层 `types.rs` serde 结构无需改。
    新选项（zipvoice `guidance_scale`/`num_inference_steps`/`t_shift`/`speed` 系、
    breeze `bf16_activations`、liveavatar `denoiser_weight_streaming` 系）全部走
    通用 options 字符串透传，无需 Rust 改动。
  - 已同步 `audio-cpp/src/types.rs` 的 `ModelFamily`：新增 4 个枚举变体
    `Zipvoice`（`as_str()` → `"zipvoice"`）/ `Auk`（→ `"auk"`）/
    `Confucius4R2t2`（→ `"confucius4_r2t2"`）/ `Liveavatar`（→ `"liveavatar"`，
    与上游 loader 族名一致）+ `from_path()` 关键词表
    （`confucius4_r2t2` 三条置于裸 `"confucius"` 之前，避免判成
    `Confucius4Tts`，另加 `confucius4-tts` 回归断言锁定）+
    `From<&str>`；并在 `audio-cpp-sys/Cargo.toml` 与 `audio-cpp/Cargo.toml`
    新增 4 个 `model-*` feature（build.rs 自动映射 CMake target）；
    full-models 注释 83→87，README 计数字段 77→87。
    `model_family_roundtrip` / `model_family_from_path` 测试已覆盖新变体。
  - 验证：`cargo fmt --check` + `cargo build --workspace`（增量重编约 1 分钟，
    新框架 TU 已编入，`engine_runtime.lib` 重链接）+ `cargo test --workspace`
    （31 lib + 12 doc 全过）+ `clippy --workspace --all-targets` 零警告 +
    `cargo check -p audio-cpp-sys --features custom-models,model-zipvoice`
    （新 feature→CMake target 映射抽查通过，`engine_model_zipvoice` 已编译）。
    4 新族 session 均自持资产（zipvoice/confucius/liveavatar 持
    `shared_ptr<const *Assets>`；auk 经 `select_component_assets` 持
    `shared_ptr<const AukAssets>`），`Session` 独立于 `Model` 存活的保证成立。
  - 注意：`metadata.json.audio_commit` 与新 submodule HEAD 不一致会强制回落
    源码构建（预编译自动下载被跳过），属预期；重新发布预编译资产须用户显式
    下达发布命令（第 6 节），本次未执行。
- **升级 `048c9a0c`→`c0b26a50`（main）审查结论**：
  - diff 共 95 文件（+8.2k/-179，12 个提交）：新增 6 个 CMake target
    `apollo`（族 `apollo`，44.1kHz 压缩音频音乐修复，task `s2s` 仅离线）/
    `universr`（族 `universr`，复数 STFT 流匹配超分至 48kHz，task `s2s`
    仅离线）/ `canary_asr`（族 `canary_asr`，NVIDIA Canary 180M Flash
    多语 ASR/翻译，task `asr` 仅离线，en/de/es/fr）/ `cohere_asr`
    （族 `cohere_asr`，Cohere Transcribe 多语转写，task `asr` 仅离线，
    14 语种）/ `moss_transcribe_diarize`（族 `moss_transcribe_diarize`，
    Whisper 编码器 + Qwen3 解码器，转写+说话人分离+时间戳联合输出，
    task `asr` 离线+流式）/ `pulsevad`（族 `pulsevad`，16kHz 单声道
    VAD，task `vad` 仅离线）；其余为 YuE2 AR/NAR 独立 LoRA（下述破坏性
    改名）/ kokoro `phonemes` 直供音素流（下述）/ 转录详情端点支持无文本
    diar（server `runtime.cpp` 仅改，`text_output` 为空时 diar 不再抛错）/
    GGUF 内嵌 sidecar 携带与校验 / RoFormer HIP 注意力布局优化 /
    CUDA 默认架构提示与 opt-in ccache / fish Q6_K/Q4_K 量化文档 / WebUI
    新增 ASR 与音频工具模型面板。上游 model target 共 83 个（+6）。
  - **C ABI 边界无需改动**：capi.cpp 依赖的 5 个头（`framework/core/backend.h` /
    `framework/io/json.h` / `framework/runtime/{model,registry,session}.h`）本次
    diff **零改动**（`src/capi/`、`session.h`、`spec_backed_model.h` 均无变化；
    上游 opt-in C API 与本仓 shim 无关）。`capi.h` / `capi.cpp` / build.rs bindgen
    allowlist 保持原样。
  - 无新 task 类型 / 输出字段：6 新族均用既有 `s2s` / `asr` / `vad`
    （moss_transcribe_diarize 的 segments+speaker_turns 复用既有输出字段）；
    高层 `types.rs` serde 结构无需改。新选项（canary `target_language`/`pnc`、
    cohere `pnc`、apollo/universr `audio_chunk_duration_sec` 系、universr
    `input_sample_rate`/`sampler_mode`/`num_inference_steps`/`guidance_scale`/
    `seed`、pulsevad `threshold`/`hop_size_samples`/`min_speech_duration_ms`/
    `min_silence_duration_ms`）全部走通用 options 字符串透传，无需 Rust 改动。
  - 已同步 `audio-cpp/src/types.rs` 的 `ModelFamily`：新增 6 个枚举变体
    `Apollo`（`as_str()` → `"apollo"`）/ `Universr`（→ `"universr"`）/
    `CanaryAsr`（→ `"canary_asr"`）/ `CohereAsr`（→ `"cohere_asr"`）/
    `MossTranscribeDiarize`（→ `"moss_transcribe_diarize"`）/ `Pulsevad`
    （→ `"pulsevad"`，与上游 loader 族名一致）+ `from_path()` 关键词表
    （`moss_transcribe_diarize` 两条置于裸 `"moss"` 之前，避免子串误判）+
    `From<&str>`；并在 `audio-cpp-sys/Cargo.toml` 与 `audio-cpp/Cargo.toml`
    新增 6 个 `model-*` feature（build.rs 自动映射 CMake target）；
    full-models 注释 77→83。`model_family_roundtrip` /
    `model_family_from_path` 测试已覆盖新变体。
  - 破坏性改名（上游，无兼容保留）：yue2 会话选项 `yue2.lora` /
    `yue2.lora_scale` 改名为 `yue2.ar_lora` / `yue2.ar_lora_scale`，另新增
    `yue2.nar_lora` / `yue2.nar_lora_scale`（NAR 声学渲染适配器，可与 AR
    独立或组合使用）。本仓 Rust 侧未硬编码旧键（仅 AGENTS.md 历史记录提及），
    无需代码改动；调用方手写旧键需自行更新。
  - 未暴露的新能力（记录，不实现）：kokoro `phonemes`（`string_list` 类型，
    需上游 opt-in C API `audiocpp_request_set_option_array`，本仓 shim 未暴露
    数组选项，暂经通用字符串 options 不可达）。
  - 验证：`cargo fmt --check` + `cargo build --workspace`（增量重编约 1.5 分钟，
    `engine_runtime` 链接通过；6 新族不在 core 集，默认构建不编译它们，
    与既往升级一致）+ `cargo test --workspace`（31 lib + 12 doc 全过）+
    `clippy --workspace --all-targets` 零警告 + `cargo check -p audio-cpp-sys
    --features custom-models,model-pulsevad`（新 feature→CMake target 映射抽查通过）。
    6 新族 session 均自持资产（canary/cohere/apollo/universr 持
    `shared_ptr<const *Assets>`；moss_transcribe_diarize/pulsevad 走
    spec-backed `shared_ptr`），`Session` 独立于 `Model` 存活的保证成立。
  - 注意：`metadata.json.audio_commit` 与新 submodule HEAD 不一致会强制回落
    源码构建（预编译自动下载被跳过），属预期；重新发布预编译资产须用户显式
    下达发布命令（第 6 节），本次未执行。
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
    model targets）按需编译，避免 full 全量 91 个 loader 族的编译成本；引擎核心 +
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
- **`prebuilt` 的 registry（crates.io）场景修复**（此前对 registry 消费者恒不生效：
  `.crate` 内无 submodule，`local_audio_commit()` 恒为 `None`，旧 `ensure_prebuilt`
  直接回落源码构建，`AUDIOCPP_PREBUILT_URL` 等覆盖也够不着）：`local_audio_commit()`
  为 `None` 时改走“版本锚定 + Release 资产清单发现”路径——按 `v{CARGO_PKG_VERSION}`
  拉 `api.github.com/.../releases/tags/{tag}` 的 `assets[]`，以前缀
  （`asset_name_of` 口径，commit 段通配）+ `.tar.gz` 后缀选资产（精确 modelset
  先、full 回退；CI 只发 full），再下载校验。身份改由版本锚定判定：
  新资产的 `metadata.json:crate_version`（CI 已同步写入，见下）须等于
  `CARGO_PKG_VERSION`；无该字段的老资产（v0.8.0 及之前）默认信任“官方 Release +
  自身版本 tag”放行，显式覆盖 TAG / REPO / URL 时视为用户自担风险放行（均有告警）；
  `msvc_ver` 门槛两条路径通用。任何存疑一律回落源码（git 检出路径零改动，
  仍走 commit 精确寻址、无需 API）。新增 env：`AUDIOCPP_PREBUILT_API`
  （清单 API 模板，GHES 可覆盖，纯传输层）/`GITHUB_TOKEN` / `GH_TOKEN`
  （API 限流配额，token 变化触发重跑）。注意 registry + prebuilt 混用释放版 lib
  与 clone 到 main HEAD 的 shim 头文件：`crate_version` 保证 lib 与本 crate 的
  capi 同源，但上游 framework 头若在发版后发生破坏性变更，链接期会 loud 失败，
  此时设 `AUDIOCPP_PREBUILT_OFF=1` 强制源码构建。验证：`prebuilt_download.rs`
  末尾 `#[cfg(test)]` 10 用例（`rustc --test` 直编本文件跑，含 v0.8.0 真实资产名
  回归，全过）+ 解包 `.crate` 到 repo 外实测 registry 全链通过
  （v0.8.0 老资产 cpu-md-full：API 发现→下载→legacy 放行→clone 头文件→链接成功，
  共 4m50s；此前同场景恒回落源码构建）。**要让下游用上此修复
  须发新版 crate + 跑 prebuilt workflow（均属第 6 节发布操作，须用户显式下达）。**
- **CI 预编译资产**：`.github/workflows/prebuilt-audio-cpp.yml` 在 `v*` tag、
  `workflow_dispatch` 时构建（linux 4 / windows 4（cpu/vulkan 各×md/mt）/ macos 1，
  共 9 cell，full × cpu/vulkan/metal）。**main 开发期推送不触发，submodule 指针变动也不触发**——
  重新发布预编译资产属「发布 / Release 操作」，须由用户显式下达发布命令（见第 6 节）。
  用 `.github/scripts/collect-unix-prebuilt.sh` 与
  `collect-windows-prebuilt.sh` 从 `target/**/out`（及 Windows 长路径重定向的
  `%TEMP%\acb*`）收集静态库打包，写 `metadata.json`（含 `audio_commit` 完整 SHA 前 12 位 +
  `msvc_ver` + `crate_version`（打包时的 crate 版本，供 registry 版本锚定校验）），
  经 `gh release upload --clobber` 上传到 `v{CARGO_PKG_VERSION}`
   Release（同一版本 Release 下按 commit 累积多个资产，天然按版本+commit 双重隔离）。
   消费端按自身 crate 版本查对应 tag，**旧版 Release 长期保留即可被旧版消费者寻址**，
   `--clobber` 只覆盖同名资产、不删旧 Release，故不存在“新版覆盖旧预编译”问题。
  Windows vulkan 用 `.github/actions/setup-vulkan-sdk-windows`（LunarG SDK）。资产命名
  须与 `prebuilt_download.rs::asset_name_of()` 保持一致（含 commit 段）。
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
pwsh -File scripts/gen-models-doc.ps1                                     # 由上游 model_specs 重新生成 docs/models.md（升级 submodule 后）
```