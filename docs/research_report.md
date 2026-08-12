# audio.cpp → Rust 绑定研究报告

> 目标：将 [audio.cpp](https://github.com/0xShug0/audio.cpp)（C++ 音频推理框架）封装为 Rust 库，供 Rust 生态直接使用。
> 参考：`temp/llama-cpp-rs`（llama-cpp-sys-2 / llama-cpp-2）、`temp/llama-cpp-rs4`（llama-cpp-sys-4 / llama-cpp-4）。
> 日期：2026-08-12 · 版本基线：audio.cpp `last-docker-build-31-g55eab3c`（Release 0.6 WIP）

---

## 1. 项目背景与目标

audio.cpp 是一个基于 `ggml` 的高性能 C++ 音频推理框架，当前已支持 **44+ 模型族**，覆盖 TTS、ASR、VAD、说话人分离、音源分离、声音克隆/转换、编解码、音乐生成等任务，后端支持 CPU / CUDA / HIP / Vulkan / Metal，且可加载 GGUF 与 safetensors 模型。

本项目目标不是"重写"audio.cpp，而是为 Rust 开发者提供一条 **安全、易用、与 C++ 运行时共享实现的绑定路径**，让 Rust 程序能够：

- 枚举并加载 audio.cpp 支持的所有模型族；
- 创建离线（offline）与流式（streaming）推理会话；
- 传输音频/文本/声音条件等输入，获取转录、合成音频、说话人片段等输出；
- 选择后端（CPU/CUDA/HIP/Vulkan/Metal）与设备。

---

## 2. audio.cpp 架构剖析

### 2.1 代码规模

| 度量 | 数值 |
|---|---|
| `src/` 下 .cpp/.h | 522 个文件，约 **23.5 万行** |
| `include/` 公共头文件 | 539 个 |
| 模型族 | 44+（`full` 组合），另有 core / custom 裁剪 |

### 2.2 构建系统（CMake）

顶层 `CMakeLists.txt` 的关键结构：

- **外部依赖**：`external/ggml`、`external/sentencepiece`、`external/llama_tokenizer`、`external/cJSON`、`external/libyaml`。
- **产物**：
  - `engine_core`（OBJECT 库）：框架核心（runtime/session/model/registry、模块库、音频 DSP、分词器、IO/JSON/YAML、采样等）；
  - 每个模型一个 `engine_model_<family>`（OBJECT 库，`EXCLUDE_FROM_ALL`）；
  - `engine_runtime`（STATIC 库）：`registry.cpp` + 选中的模型 OBJECT 库聚合；
  - `cjson_vendor` / `yaml_vendor` / `sentencepiece` / `ggml`。
- **代码生成**：从 `model_specs/*.json` 生成 `model_specs.inc`、`model_registry_includes.inc`、`model_registry_loaders.inc`，运行时据此装配 loader 注册表。
- **模型裁剪**：`AUDIOCPP_MODEL_SET=full|core|custom` + `AUDIOCPP_MODELS=...`；`full` 默认全量链接。
- **后端开关**：`ENGINE_ENABLE_CUDA/HIP/VULKAN/METAL/LLAMAFILE/OPENMP/NATIVE_CPU/CPU_ALL_VARIANTS` 等，映射到 `GGML_*` 选项。

### 2.3 公共 C++ API 面（绑定目标）

头文件在 `include/engine/`，命名空间 `engine::runtime`。核心类型（见 `runtime/model.h`、`runtime/session.h`、`runtime/registry.h`）：

```cpp
// 注册表（入口）
ModelRegistry make_default_registry(optional<path> config_path);
class ModelRegistry {
    std::vector<std::string> families();
    ModelInspection  inspect(const ModelLoadRequest&);
    std::unique_ptr<ILoadedVoiceModel> load(const ModelLoadRequest&);
};

// 模型
class ILoadedVoiceModel {
    const ModelMetadata &  metadata();
    const CapabilitySet &  capabilities();
    std::unique_ptr<IVoiceTaskSession> create_task_session(const TaskSpec&, const SessionOptions&);
};

// 会话：离线 / 流式
class IOfflineVoiceTaskSession : IVoiceTaskSession {
    void prepare(const SessionPreparationRequest&);
    TaskResult run(const TaskRequest&);
};
class IStreamingVoiceTaskSession : IVoiceTaskSession {
    StreamingPolicy streaming_policy();
    void start_stream(const TaskRequest&);
    StreamEvent process_audio_chunk(const AudioChunk&);
    TaskResult finalize();
    void reset();
};
```

数据对象（大量 `std::string`/`std::vector<float>`/`std::unordered_map<std::string,std::string>`/`std::optional`）：

- `TaskSpec`（task + mode）、`SessionOptions`（`BackendConfig` + options map）；
- `AudioBuffer`、`AudioChunk`、`Transcript`、`SpeechSegment`、`SpeakerTurn`、`WordTimestamp`；
- `VoiceReference`/`StyleCondition`/`VoiceCondition`；
- `TaskRequest`（text/audio/voice/artifacts/options）、`TaskResult`、`StreamEvent`；
- `ModelLoadRequest`、`ModelInspection`、`ModelMetadata`、`CapabilitySet`、`LoaderAdvertisement`。

### 2.4 关键结论

1. **audio.cpp 没有任何面向外部的 C ABI**（`extern "C"` 只存在于 `external/` 依赖与 `tools/omtd`），公共 API 全部是 C++ 类 + STL 容器 + 虚函数 + 异常。bindgen 无法直接绑定，**必须自建一层 C 包装（shim）**。
2. 注册表–模型–会话三级对象模型与 llama.cpp 的 `llama_model / llama_context` 相比层级更多，但抽象非常规整，**封装成 Rust 的 Engine → ModelLoader → Session 三层非常自然**。
3. 所有模型共用一套请求/响应结构，且 audio.cpp 自带 `engine::io::json` 与 HTTP server（`app/server`），**JSON 已在 C++ 侧被用作通用编组格式**——FFI 边界复用它可大幅减少手写编组代码。
4. 每个模型一个 OBJECT 库、注册表由生成代码装配，意味着**"模型子集"是一个编译期选项**，与 Cargo feature 天然对应。

---

## 3. 参考项目分析

### 3.1 llama-cpp-rs（llama-cpp-sys-2 + llama-cpp-2，utilityai 维护）

**sys 层（`llama-cpp-sys-2`）**
- `build.rs`：
  - `bindgen` 对 `wrapper.h` 生成绑定，`allowlist_function("llama_.*"/"ggml_.*"/"gguf_.*")`；
  - `cmake::Config` 编译 vendored `llama.cpp/`，关闭 examples/tests/tools；
  - 通过 `extract_lib_names()` glob `OUT_DIR/lib*/*.a|.so` 推断静态库名，`println!("cargo:rustc-link-search=...")`；
  - **Cargo feature ↔ CMake 选项映射**：`cuda/metal/vulkan/opencl/openmp/rocm/mkl` → `GGML_*`；
  - 转发 `GGML_*`、`CMAKE_*` 环境变量给 CMake（下游可覆盖任意选项）；
  - 把 `CARGO_CFG_TARGET_FEATURE` 里的 `avx/avx2/fma/...` 映射成 `GGML_AVX*`（未开 `target-cpu=native` 时逐特性打开）；
  - MSVC 下通过 `cc::Build` 提取 INCLUDE 环境喂给 bindgen，并补 `/O2 /DNDEBUG /Ob2` 修复 debug 下 CMake Release 回归；
  - Android NDK 工具链探测（`ANDROID_NDK` 等）。
- 包装 C++ 侧仅有的"纯 C++ 无法 bindgen"部分：`wrapper_common.cpp` 用 `extern "C"` 导出 `llama_rs_*` 辅助函数（`common` feature）。

**安全层（`llama-cpp-2`）**
- 结构体镜像：`LlamaModelParams`、`LlamaContextParams` 等用 `Default`/字段级默认值，`#[repr(C)]` 或逐字段转换后调用 sys；
- 指针包装：`LlamaModel`/`LlamaContext` 持 `NonNull` + 手动 `Drop`（`llama_free_*`）；
- 错误：`thiserror`；日志桥接 `tracing`；feature 透传到 sys 层；
- 提供 `Sampler`、回调进度 `progress_callback` 等安全封装；示例 `examples/simple`。

### 3.2 llama-cpp-rs4（llama-cpp-sys-4 + llama-cpp-4，eugenehp fork）

在上一代基础上增强，值得借鉴的点：

- **ext_shim 模式**（`llama-cpp-sys-4/ext_shim/`）：对纯 C++ 的 API（如 `common_get_device_memory_data` 返回嵌套结构、`llama_get_memory_breakdown`）手写 `extern "C"` 扁平化函数——入参 `opaque* + out 数组 + max_out`，返回写入条数。**这正是我们为 audio.cpp 会话/注册表写 C 包装的标准范式**。
- **build 期补丁**：`patch-apply` 对 OUT_DIR 中的 C++ 源码打 patch（如 MTP shim），避免污染 vendored 源码。
- **prebuilt 下载**：`prebuilt_download.rs` 用 `ureq + tar + flate2` 按 tri 下载预编译产物，`prebuilt` feature 可跳过本地编译（CI 出包 + 发布时用）。
- **细粒度 feature**：`q1/mtmd/mtp/rpc` 等按功能拆分；`links = "llama"` 声明 GL，避免双份 ggml 符号冲突。
- 安全层模块化：`model/context/token/sampling/ggml/common.rs` 等按子系统拆文件。

### 3.3 可迁移结论

| audio-cpp-rs 需求 | 借鉴来源 |
|---|---|
| C++/STL/虚函数类无法 bindgen → 需要 C shim | llama-cpp-rs4 `ext_shim` 范式 |
| Cargo feature ↔ CMake 选项映射、静态库发现 | llama-cpp-sys-2 `build.rs` |
| 编译器特性（AVX/AVX2…）自动映射 | llama-cpp-sys-2 `build.rs` |
| 转发环境变量让下游定制构建 | llama-cpp-sys-2 `build.rs` |
| 避免双份 ggml | `links = "..."` GL 机制（两个参考项目都用） |
| 预编译加速安装 | llama-cpp-rs4 `prebuilt` |
| 安全封装：错误类型、指针 RAII、回调桥接 | llama-cpp-2 安全层 |

---

## 4. 可行性分析

### 4.1 技术可行性：**高**

1. **绑定路径成熟**：audio.cpp 的运行时抽象（Registry → Model → Session）是稳定的虚接口层，不依赖 STL 容器的 ABI（我们在 FFI 侧用 JSON 字符串/浮点数组跨边界），不存在 ABI 不稳定的问题。
2. **已有 JSON 通用编组**：server 端已把请求/响应定义成 JSON 形态，Rust 侧直接映射该 JSON（可先用 `serde_json`，后续可逐类型强类型化）。
3. **ggml 由 audio.cpp 自己构建**：我们只需把整个 CMake 工程当黑盒静态库构建，符号冲突风险低（Rust crate 之间靠 `links` 声明）。
4. **最小验证路径现成**：`silero_vad`/`marblenet_vad` 是 **Bundled** 模型（无外部下载），可在最早阶段端到端跑通。

### 4.2 工作量评估（相对）

- 纯 C shim（约 1000–2000 行 C++，覆盖 registry/model/session/audio 类型）：可控；
- bindgen 生成 + `build.rs` 移植自 llama-cpp-sys-2：主要是 feature 矩阵与库发现逻辑，工作量中等；
- 安全层 Rust：Engine/Model/Session 三个核心对象 + JSON 结构 + 错误 + 回调，中等；
- **主要风险在构建期**：全量 44 模型编译时间长、产物大；需提供 `core`/`custom` 模型子集 feature（对齐 `AUDIOCPP_MODEL_SET`）。

### 4.3 结论

可行。**"C shim + bindgen + 安全层" 是唯一现实路径**；纯 Rust 重写 23.5 万行 C++（含 ggml 算子、44 个模型、DSP）不现实；`cxx`/`autocxx` 对本项目这种"大量自定义值类型 + 虚函数回调"的场景反而更难维护（需大量 opaque/桥接，且宏生成调试困难）。详见 §5。

---

## 5. 方案选型与论证

| 方案 | 优点 | 缺点 | 结论 |
|---|---|---|---|
| **A. C shim + bindgen（推荐）** | 与两个参考项目完全同构；绑定面完全可控；逐功能演进 | 需手写 ~1-2k 行 C 包装与编组 | ✅ 采用 |
| B. `cxx` crate 双向绑定 | 类型映射自动化度高 | 音频 C++ 库公开 API 大量使用 STL/虚函数/回调，`cxx` 需要大量 `ExternType`/opaque 声明，复杂度反而上升；构建耦合 `cxxbridge` | ❌ |
| C. 纯 Rust 重写 | 无 FFI、内存安全 | 23.5 万行 + ggml + 44 模型，不可行；且失去与上游性能/模型同步 | ❌ |
| D. 直接 bindgen 绑定 C++ 头 | 无额外层 | bindgen 不生成 C++ 类方法绑定，`std::vector/std::string/std::function` 等无 ABI 保证 | ❌ |

**方案 A 细化**：新增一个小 C++ 翻译单元（如 `audio-cpp-sys/src/capi.cpp` + `capi.h`），`extern "C"` 导出扁平函数，**和 `engine_runtime` 一起编进同一个静态库**（或单独编译再链）。Rust 侧 bindgen 只吃 `capi.h`（纯 C 可解析），安全层消费 `capi.h` 绑定。

---

## 6. 总体架构设计

### 6.1 工作区布局（对齐 llama-cpp-rs 双 crate 惯例）

```
audio-cpp-rs/                      # workspace 根
├─ Cargo.toml                      # [workspace] members
├─ audio-cpp-sys/                  # ① FFI 层（links = "audiocpp"）
│  ├─ audio.cpp/                   #    vendored 源码（git submodule 或 vendor 脚本）
│  ├─ capi.h / capi.cpp            #    手写 C shim（extern "C"）
│  ├─ build.rs                     #    cmake + bindgen(capi.h) + 静态库发现
│  └─ src/lib.rs                   #    include!("bindings.rs") + 少量 FFI helper
├─ audio-cpp/                      # ② 安全层
│  ├─ src/error.rs  engine.rs  model.rs  session.rs  task.rs  audio.rs  json.rs
│  └─ src/lib.rs
├─ examples/                       # asr / tts / streaming / list-loaders
└─ docs/
```

### 6.2 FFI 边界原则

- **Opaque 句柄**：`AudioCppRegistry*`、`AudioCppModel*`、`AudioCppSession*`，创建/销毁成对导出；
- **编组格式**：结构化输入输出统一用 **JSON 字符串**（`AudioCppResult` 结构携带 `char*` + `size_t` + `error_code`/`error_msg`）；音频波形用 `float* + size_t`（避免 JSON 编码 44.1kHz 音频的爆炸式开销）；
- **异常边界**：所有导出函数 `try/catch(...)`，把异常转成错误码 + `last_error()` 字符串，绝不跨 FFI 抛异常；
- **回调**：流式事件回调采用 `(void* user_data, const char* json_event) -> void` C 函数指针 + Rust 侧 `Box<dyn Fn>` 桥接（同 llama.cpp `progress_callback` 模式）；
- **导出符号白名单**：`allowlist_function("audiocpp_.*")`，不让 bindgen 把 ggml 全部符号暴露给用户。

### 6.3 C shim 导出函数草案（示意）

```c
// 注册表
audiocpp_registry*   audiocpp_registry_default(const char* config_path);
void                 audiocpp_registry_free(audiocpp_registry*);
size_t               audiocpp_registry_families(const audiocpp_registry*, char*** out);
const char*          audiocpp_registry_list_loaders(const audiocpp_registry*); // JSON
audiocpp_result      audiocpp_registry_inspect(const audiocpp_registry*, const char* model_path, const char* options_json);
audiocpp_model*      audiocpp_registry_load(const audiocpp_registry*, const char* load_request_json); // 异常→result

// 模型
const char*          audiocpp_model_metadata(audiocpp_model*);   // JSON
audiocpp_session*    audiocpp_model_create_session(audiocpp_model*, const char* task_spec_json, const char* options_json);

// 会话（离线）
audiocpp_result      audiocpp_session_prepare(audiocpp_session*, const char* prep_json);
audiocpp_result      audiocpp_session_run(audiocpp_session*, const char* request_json);

// 会话（流式）
audiocpp_stream_policy audiocpp_session_streaming_policy(audiocpp_session*);
audiocpp_result      audiocpp_session_start(audiocpp_session*, const char* request_json);
audiocpp_result      audiocpp_session_process_chunk(audiocpp_session*, const float* samples, size_t n, int sr, int ch, int64_t start);
// 流式事件走回调注册：audiocpp_session_set_event_sink(session, cb, user_data);
audiocpp_result      audiocpp_session_finalize(audiocpp_session*);
void                 audiocpp_session_reset(audiocpp_session*);

// 音频工具（可选二期）：resample / denoise / stft 等
```

### 6.4 feature 矩阵（草案）

```toml
[features]
default = ["core-models"]                     # 对齐 AUDIOCPP_MODEL_SET=core
full-models   = []                            # AUDIOCPP_MODEL_SET=full
core-models   = []                            # AUDIOCPP_MODEL_SET=core
# 单模型族开关（对齐 AUDIOCPP_MODELS=custom）：
silero-vad = []   marblenet-vad = []
qwen3-asr  = []   qwen3-tts  = []   qwen3-forced-aligner = []
fish-audio = []   higgs-audio-tts = []   ...（44 族逐一映射）
# 后端：
cuda = []   hip = []   vulkan = []   metal = []
openmp = []   native = []   cuda-graphs = []   llamafile = []
# 发布：
prebuilt = []
```

`build.rs` 把这些 feature 翻译成 CMake `-DAUDIOCPP_MODEL_SET=... -DAUDIOCPP_MODELS=... -DENGINE_ENABLE_*`。

### 6.5 安全层 API 形态（草案）

```rust
// audio-cpp
pub struct AudioEngine { registry: sys::Registry }     // 可 Clone? 内部 Arc
pub struct Model { _inner: sys::Model, meta: ModelMetadata }
pub enum Session { Offline(OfflineSession), Streaming(StreamingSession) }

impl AudioEngine {
    pub fn new() -> Result<Self>;
    pub fn families(&self) -> Vec<String>;
    pub fn list_loaders(&self) -> Result<Vec<LoaderAdvertisement>>;
    pub fn inspect(&self, model_path: impl AsRef<Path>, opts: &LoadOptions) -> Result<ModelInspection>;
    pub fn load(&self, model_path: impl AsRef<Path>, opts: &LoadOptions) -> Result<Model>;
}

impl Model {
    pub fn metadata(&self) -> &ModelMetadata;
    pub fn capabilities(&self) -> &CapabilitySet;
    pub fn offline_session(&self, task: TaskSpec, opts: SessionOptions) -> Result<OfflineSession>;
    pub fn streaming_session(&self, task: TaskSpec, opts: SessionOptions) -> Result<StreamingSession>;
}

// 离线
impl OfflineSession {
    pub fn run(&mut self, request: TaskRequest) -> Result<TaskResult>;
}

// 流式
impl StreamingSession {
    pub fn set_event_sink(&mut self, cb: impl FnMut(StreamEvent) + Send + 'static);
    pub fn start(&mut self, request: TaskRequest) -> Result<()>;
    pub fn push_audio(&mut self, chunk: AudioChunk) -> Result<()>;
    pub fn finish(&mut self) -> Result<TaskResult>;
}
```

---

## 7. 关键难点与对策

| 难点 | 对策 |
|---|---|
| C++ 异常跨 FFI | shim 全部 `try/catch` 兜底，返回错误码 + 缓存错误串，`audiocpp_last_error()` 读取 |
| STL/虚函数无法 bindgen | 一律 opaque 句柄 + C 扁平函数；值类型用 JSON 字符串跨边界（JSON 由 C++ 侧 `engine::io::json` 生成） |
| 流式事件回调 | C 函数指针 + `user_data`，Rust 侧闭包桥接；回调只做"拷出 JSON + 唤醒 channel"，不在 FFI 线程里做重活 |
| MSVC OpenMP `#pragma omp simd`（longformer C7660） | CMake 已用 `/openmp:experimental`；我们直接沿用 C++ 侧 CMake，不做二次编译 C++，规避此坑 |
| 模型裁剪与二进制体积 | feature 映射 `AUDIOCPP_MODEL_SET`/`AUDIOCPP_MODELS`；`full` 仅按需开启 |
| 构建时间长 | 增量 `cmake::Config` 复用 OUT_DIR；CI 缓存；后续 `prebuilt` feature 走预编译下载（llama-cpp-rs4 模式） |
| ggml 双份符号 | `links = "audiocpp"` + 参考 llama-cpp-rs 的 `ggml_cmake_dir` 导出，声明唯一 |
| 上游 API 变动快（0.6 WIP） | vendored 源码固定 commit（`55eab3c`）；shim 只依赖稳定的 Registry/Model/Session 接口，升上游时 diff 化评估 |
| 会话对象线程安全 | Rust 侧 `Session` 持 `UnsafeCell`/`Mutex`，`!Sync` 语义或显式 `&mut self`；engine/model 用 `Arc` |
| 模型规格（`model_specs/*.json`）随 C++ 一起生成 | 由 CMake 生成步骤照常执行，shim 不干预；Rust 侧通过 `ModelInspection`（JSON）读取 |

---

## 8. 路线图

> 原则：**先通后优，先最小模型（Bundled VAD）打通全链路，再铺模型矩阵，再做质量与发布。**

### 阶段 0 — 环境与可行性验证（0.5–1 周）
- [ ] 确认本机 CMake/编译器/CUDA 可用；`temp/audio.cpp` 独立 CMake 构建出 `engine_runtime` 静态库；
- [ ] 用 `silero_vad`（Bundled）跑通 `audiocpp_cli`，确认模型加载路径；
- [ ] 手写 ~200 行最小 C shim（registry default → load silero_vad → run），用一个小 C 程序验证；
- **验收**：C 程序能对一段 wav 输出 VAD 片段。

### 阶段 1 — `audio-cpp-sys` FFI 层（2–3 周）
- [ ] workspace 化，`audio-cpp-sys/audio.cpp` vendored 固定 commit；
- [ ] `build.rs`：`cmake` 构建 + 静态库发现 + `bindgen(capi.h)` + feature→CMake 映射 + `GGML_*`/`CMAKE_*` 转发 + `links="audiocpp"`；
- [ ] 完整 `capi.h/capi.cpp`：registry / model / offline session / streaming session / 音频缓冲 / JSON 结果 / 错误码 / 回调；
- [ ] Windows MSVC 与 Linux CI 冒烟构建；
- **验收**：Rust 能通过 sys 层加载 silero_vad 并跑离线 VAD。

### 阶段 2 — `audio-cpp` 安全层（2–3 周）
- [ ] `Error`（thiserror）、`AudioEngine`、`Model`、`OfflineSession`/`StreamingSession`；
- [ ] `serde` 结构：`TaskRequest`/`TaskResult`/`StreamEvent`/`ModelInspection` 等 ↔ C++ JSON 对齐（参考 server 端 JSON 契约）；
- [ ] 流式事件通道（callback → `std::sync::mpsc`/`crossbeam`）；
- **验收**：Rust 程序端到端跑通 ASR（`qwen3_asr`）与 TTS（`qwen3_tts`）示例。

### 阶段 3 — 模型矩阵与后端特性（3–4 周）
- [ ] 全量 44 族 feature 映射 + 依赖关系（`DEPENDS`，如 `qwen3_asr ↔ qwen3_forced_aligner`）；
- [ ] `cuda`/`hip`/`vulkan`/`metal`/`openmp`/`native` 等后端 feature 验证；
- [ ] 与 `audiocpp_cli`/server 输出的 **parity 测试**（同一 wav/文本，比较转录文本与音频 RMS/时长）；
- **验收**：CI 矩阵（win/linux/mac × cpu/cuda）绿。

### 阶段 4 — 工程化（2–3 周）
- [ ] 示例集（`examples/`：list-loaders、asr、tts、streaming、voice-clone）；
- [ ] 文档（README、API docs、模型支持表）；benches（RTF/时延对比 C++ CLI）；
- [ ] GitHub Actions 缓存/制品；`prebuilt` 预编译下载（可选，参考 llama-cpp-rs4）；
- [ ] crates.io 发布 `audio-cpp-sys` + `audio-cpp`。

### 阶段 5 — 进阶（持续，视需求）
- [ ] 强类型编组（JSON → typed struct，serde 直连，避免字符串中转）；
- [ ] `audio-cpp-utils`（DSP/重采样/去噪的 Rust 侧 API 透出）；
- [ ] 音频工具：WAV 读写、自动设备枚举（`list_backend_devices` 已存在，直接透出）。

---

## 9. 风险与缓解

| 风险 | 等级 | 缓解 |
|---|---|---|
| 上游快速演进、API 变动 | 中 | vendored 固定 commit；shim 只依赖核心虚接口；升级走 diff review |
| 构建时间/体积（44 模型） | 中 | 模型子集 feature；CI 缓存；prebuilt 下载 |
| MSVC/OpenMP/编译器差异 | 低 | 完全复用 audio.cpp CMake；不二次编译 C++；CI 三平台矩阵 |
| FFI 回调/内存泄漏 | 中 | 统一 RAII 包装、opaque 销毁函数；`-Z sanitizer`/valgrind 在 CI 冒烟 |
| JSON 编组性能（长音频/大 batch） | 低 | 波形走 `float*`，仅元数据走 JSON；流式按 chunk 拉取 |
| 模型文件下载/规格依赖 | 低 | `ModelInspection` 透出 `config/weights candidates`，Rust 侧复用 audio.cpp model_manager 逻辑 |

---

## 10. 结论与建议

1. **技术上完全可行**，且是"顺着音频生态往 Rust 走的正路"：llama.cpp 已在 Rust 生态被成功绑定两代，audio.cpp 与其同构（同为 ggml + C++ 运行时 + JSON server）。
2. **采用 C shim + bindgen + 安全层双 crate 架构**，直接借鉴：
   - `llama-cpp-sys-2/build.rs`（feature→CMake、静态库发现、环境变量转发）；
   - `llama-cpp-rs4` 的 `ext_shim` 范式（纯 C++ 类的 C 扁平化包装）；
   - 两者共用的 `links` 机制防 ggml 双份。
3. **从 Bundled VAD 起步**，最快 1 周可验证全链路；ASR/TTS 随后补齐，最终形成 44 族模型 + 5 种后端的完整矩阵。
4. 建议仓库结构按 §6.1 的 workspace 落地，首阶段交付 `audio-cpp-sys`（FFI 层）与 `audio-cpp`（安全层）两个 crate。
