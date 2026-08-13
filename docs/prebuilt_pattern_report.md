# 研究报告：audio-cpp-rs 能否借鉴 llama-cpp-rs 的预编译库模式

> 目标：评估 `eugenehp/llama-cpp-rs`（crate `llama-cpp-sys-4`，即 `temp/llama.cpp-plug`）的
> 预编译 native 库机制能否移植到本项目 audio-cpp-rs，并给出落地路线。
> 日期：2026-08-13 · 代码基线：llama-cpp-rs main 分支（prebuilt 机制已随 0.5.x 上线）

---

## 1. 结论先行

**可以借鉴，且收益显著；核心机制（目录式消费 + CI 打资产 + 自动下载 + 身份校验回落）几乎可整套移植。**
需做三处定制：

1. 资产标识从「crate 版本」改为「audio.cpp submodule commit + 模型组合」；
2. 变体矩阵从「仅后端」扩展为「后端 × 模型集合」；
3. CUDA 预编译仍无法消除对本地 Toolkit 的链接依赖。

建议按「手动目录 → CI 资产 → 自动下载」三阶段落地，第一阶段风险极低。

---

## 2. llama-cpp-rs 预编译机制全貌（已核实的实现）

研究基于 `https://github.com/eugenehp/llama-cpp-rs`。其预编译机制由三件套组成：

### 2.1 资产生产：`.github/workflows/prebuilt-llama.yml`（tag `v*` 触发 + workflow_dispatch）

- 矩阵定义在 `.github/prebuilt-matrix.json`：
  **linux(x64/arm64)×{cpu,vulkan,blas}、windows×{cpu,vulkan}、macos(arm64)×{cpu,metal}、android(arm64)×{cpu}，
  各自再分 static/dynamic**，共约 22 个 cell。
- `plan` job 先用 `filter-prebuilt-matrix.py` 产出子矩阵（dispatch 时支持单选平台/变体/库类型）。
- 每个 cell 流程：
  1. `git checkout --recursive`（拉 submodule）；
  2. 装依赖（vulkan 装 libvulkan-dev/glslc，blas 装 openblas，Windows 配 Vulkan SDK action）；
  3. `cargo build -p llama-cpp-4 --no-default-features --features "<variant>,mtmd"`（dynamic 追加 `,dynamic-link`）；
  4. 跑收集脚本（`.github/scripts/collect-linux-prebuilt.sh` / `collect-windows-prebuilt.sh`）；
  5. 写 `metadata.json`：`{"target":..., "features":..., "library_type":..., "sha":...}`；
  6. `tar.gz` 归档 → `gh release upload --clobber`（release 不存在则 `gh release create`）。
- 资产命名：`llama-prebuilt-{linux|macos|windows|android}-{target}-{variant}-{static|dynamic}.tar.gz`。

### 2.2 收集脚本（归档内层布局）

- 按 `is_llama_lib_name`（`libllama* | libggml* | libcommon* | libmtmd*`）过滤；
- static：从 CMake install 树取 `*.a`；
- dynamic：从 CMake install 的 `lib/lib64` 取版本化 soname 链（`libllama.so -> libllama.so.0 -> libllama.so.0.0.N`，
  含符号链接），**跳过 target/**/deps 里的破损软链**；
- Windows：static 收 `llama*.lib|ggml*.lib|common*.lib|mtmd*.lib`（来源含 `$LOCALAPPDATA/llcb` 短路径缓存与 OUT_DIR）；
  dynamic 收 `.dll` 到 `bin/`。
- 归档结构 = `{<dir>/lib, <dir>/lib64, <dir>/bin}` 灵活布局，消费端四种路径全支持。

### 2.3 消费端 build.rs（跳过 CMake 的核心）

- `resolve_prebuilt_directory()`：读 `LLAMA_PREBUILT_DIR`（显式目录，最老信封）→
  否则 `#[cfg(feature="prebuilt")]` 调 `prebuilt_download::ensure_prebuilt()` 自动下载。
- 命中路径后：
  - 对 `<dir>, <dir>/lib, <dir>/lib64, <dir>/bin` 各发 `cargo:rustc-link-search`；
  - `extract_prebuilt_lib_names()` 探测库名 → 逐个 `cargo:rustc-link-lib=static|dylib=<lib>`；
  - 再补平台运行时库（linux `stdc++`、mac framework Foundation/Metal/Accelerate + `c++`/`clang_rt.osx`、win `advapi32`）。
- dynamic 模式下 `extract_prebuilt_shared_assets()` 把 `.so/.dll/.dylib` **按 soname 跟随符号链接**
  复制到 `target/{,examples/,deps/}` 下，保证 loader 能按版本名加载。
- 之后仍 `compile_mtp_shim / compile_ext_shim`（C shim 恒从源码编译）+ 继续跑 bindgen，
  **只有大头 C++ 库是预编译的**。
- 附加工程点：Windows 有 `%LOCALAPPDATA%\llcb\<8hex>` MAX_PATH 规避；非 Windows 有
  「源码提交 + 后端 flags」为 key 的共享 CMake cache 避免 feature 切换全量重编。

### 2.4 `prebuilt_download.rs` 自动下载模块

- `asset_name(target, shared)` 按当前 target/feature → 资产名；
- `variant_suffix()`：`cuda/hip/webgpu/opencl/q1 → None(不发布)`，`metal/vulkan/blas → 对应后缀`，否则 `cpu`；
- `ensure_prebuilt()`：`LLAMA_PREBUILT_OFF` 则 disabled → `LLAMA_PREBUILT_DIR` 已设则跳过下载 →
  计算 `target/llama-prebuilt-cache/<tag>/<asset>` 缓存目录，已有效则复用 →
  否则 `ureq` 下载 + `flate2+tar` 解压 → **解压后无库则删缓存回落源码编译**；
- 环境变量：`LLAMA_PREBUILT_DIR / TAG(默认 v{CARGO_PKG_VERSION}) / REPO(默认 eugenehp/llama-cpp-rs)
  / URL(整包覆盖) / OFF / SHARED`；
- `is_valid_prebuilt_root()` 用 `is_llama_lib_name()`（`llama|ggml|ggml-base|ggml-cpu|common|mtmd` 或 `ggml-*` 前缀）校验目录有效。

### 2.5 身份校验 / 兜底（关键设计）

- llama-cpp-rs 有 `patches/` 目录（构建时对 llama.cpp 打补丁）。
- **只要 patches 激活（`stage_active_patches` 非空）：显式 `LLAMA_PREBUILT_DIR` 直接 panic**，
  `prebuilt` feature 自动下载则**回落到已验证的源码编译**——因为归档没有"补丁身份信封"。
- `llama_src_version = submodule commit + patches hash` 用于缓存失效与陈旧检测。

### 2.6 已知的效果数据（README 基准）

- Debug 静态链接 + prebuilt：11.99s→11.01s（约 **8%**）；
- 收益主要在**调试迭代**与 **CI/CD 复用**（预期 50–80% 提速）；Release 差异不大；dynamic 反而更慢。

---

## 3. audio-cpp-rs 现状（对照基线）

`audio-cpp-sys/build.rs`（588 行）当前纯源码构建：

1. `ensure_audio_src()` 断言/回填 submodule `audio.cpp`；
2. CMake + Ninja 构建 `engine_runtime` 目标，`CMAKE_ARCHIVE_OUTPUT_DIRECTORY` 收敛到 `OUT_DIR/lib`；
   `ENGINE_ENABLE_*` 与 feature 映射；`AUDIOCPP_MODEL_SET` 三态（full/core/custom），
   custom 由 `model-*` feature 与 `AUDIOCPP_MODELS` env 取并集；`always_configure(true)`；
3. `extract_static_lib_names()` glob 收集 `.lib/.a`（跳过 CMakeFiles），断言含 `engine_runtime`，
   逐个 `cargo:rustc-link-lib=static=`；
4. 平台链接（win `advapi32`）+ `emit_cuda_links`（cudart/cublas/cublasLt/cufft/cuda，
   定位 CUDA_PATH/nvcc/常见目录）+ `emit_vulkan_links`（`vulkan-1`/`vulkan`，VULKAN_SDK/C:/VulkanSDK/v*）；
5. `cc` 编译 `capi.cpp`（shim，含 `/utf-8 /EHsc /std:c++17`）；6. bindgen 生成 `bindings.rs`。

### 结构差异（直接影响移植设计）

- llama.cpp 产出**多帧分离库**（libllama/libggml...）；audio.cpp 的模型族是
  **OBJECT 库 `engine_model_<family>` 全部折叠进单一静态库 `engine_runtime`**
  （`AUDIOCPP_RUNTIME_OBJECTS` = engine_core + silero_vad + marblenet_vad + 已选族）。
  所以**归档只需一个 `engine_runtime` + 少量依赖**（ggml/ggml-cpu/ggml-base/sentencepiece/
  cjson_vendor/yaml_vendor），收集比 llama 简单得多。
- 但**模型组合烙在 `engine_runtime` 里**：core/full/custom 是同一库名的不同二进制——这是与
  llama（模型集固定）最大的维度差异。
- audio-cpp 的 C API 由 Rust 侧 `cc` 编译的 `capi.cpp` 提供，不属引擎库；绑定点都在 engine 头。

---

## 4. 可移植性逐项分析

| llama-cpp-rs 机制 | audio-cpp-rs 可移植性 | 备注 |
|---|---|---|
| `*_PREBUILT_DIR` 目录消费 + 四种子路径 link-search | ✅ 直接搬运 | 现有 extract/link 逻辑已集中在 build.rs 一个函数，旁路即可 |
| 跳过 CMake、仍编译 shim + bindgen | ✅ 直接搬运 | capi 恒源码编译，头文件来自 submodule（AGENTS 约束下 submodule 本就在） |
| `prebuilt_download.rs` 自动下载/解压/缓存/回落 | ✅ 可复制改前缀 | 库名校验改为 `engine_runtime`/`ggml*`/`sentencepiece`/`cjson`/`yaml` |
| `prebuilt-llama.yml` tag 触发 + matrix + collect + `gh release upload` | ✅ 模板可复用 | 矩阵按 4.1-B 定制 |
| 资产命名 | ✅ 可复用 | 后缀需含模型集合维度 |
| patches 身份校验 fail-closed | ⚠️ 简化 | audio-cpp 无 patches；**身份 = submodule commit**，需从"patch 信封"转为"commit + 模型集 + 后端"校验 |
| 自动下载 tag = v{crate 版本} | ❌ 不可用 | crate 版本不锁 submodule；tag 必须绑定 audio.cpp commit |
| dynamic-link 双产物 | ⚠️ 二期可选 | 需给 engine_runtime 加 C ABI DLL 导出方案；一期 static-only |
| Windows MAX_PATH / 共享 cache | ✅ 预编译天然消解 | 预编译路径无 CMake 树，vulkan ExternalProject 深嵌套问题消失 |

### 4.1 三个必须定制的点

**A. 资产命名维度 = commit + 模型集 + 后端 + 平台**（不由 crate 版本决定）：

```text
audio-cpp-prebuilt-{os}-{target}-{backend}-{modelset}-static.tar.gz
# modelset 例：core / full / custom-qwen3_asr / custom-moss_tts_nano …
```

- release tag 建议绑定 audio.cpp 的 submodule commit（如 `audio-<shorthash>` 或每个 commit 建 tag），
  `metadata.json` 记 `audio_commit/features/models/sha`；
- 消费端比对 `submodule HEAD == metadata.audio_commit`，不符 → warning + 源码回落（对应 llama 的 fail-closed）；
- 让同 crate 版本在不同子模块提交下各自拿到匹配归档，避免静默 ABI 错配。

**B. 矩阵爆炸治理**：full = 44+ 模型族，实测编译极久、归档大；core 很小。若只发 full，
默认 core 用户被迫下载巨包。建议：

- 基准发布 **core（小，VAD 即用）** 与 **full（全家桶）**；
- 对常用族（qwen3_asr、moss_tts、citrinet_asr、demucs、sortformer_diar…）**按族单独发布 custom-<族>**
  （每族即 core+一族，归档小）；
- 下载模块据启用的 `model-*` feature 与 `AUDIOCPP_MODELS` env 拼 `modelset`，未收录组合 → 源码回落；
- 保留 `AUDIOCPP_PREBUILT_URL` 整包覆盖，配合 `AUDIOCPP_MODELS` 即可让任何人自产自用任意组合。

> **最终采用（2026-08-13 落地）**：上面"逐族发布 custom-<族>"的方案最终**未采用**。
> 逐族发布会组合爆炸，且开启多个 `model-*` feature 时无法用一个族资产覆盖。
> 实际落地为**只发布 `full` 全模型资产** + 下载端 superset 回退：
> 无论开启哪些模型组合 feature，先尝试精确资产名（`core` / `custom-<族1>-<族2>...`），
> 404 时自动回退下载 `full` 资产（它是任何组合的超集，功能全可用，仅体积较大），
> 仍失败才回落源码。CI 矩阵因此精简为 full × cpu/vulkan/metal。

**C. CUDA 预编译的边界**：`emit_cuda_links` 依赖本地 Toolkit（cudart/cublas/... import lib 需在消费机上存在），
所以 **CUDA 预编译只省编译、不省 SDK**。同理 Vulkan 仍需 `vulkan-1.lib` 链接。文档须写明；
这也是 llama-cpp-rs 把 cuda 归档标成"experimental/不发布"的原因。
**预编译最大收益反而是 vulkan（消解 vulkan-shaders-gen 的深链与编译）与 cpu。**

### 4.2 与 llama 一致的收益预期

- 下游 debug 迭代提速（llama 实测约 8%）；
- **CI/CD 复用的预期 50–80% 提速**：下游仓库（或本库 examples、跨平台 app 流水线）设
  `AUDIOCPP_PREBUILT_DIR` 指向已发布归档即秒装，跳过最耗时的 ggml/engine 编译；
- Windows vulkan/cuda 场景免去本地 C++ 工具链与路径长度地狱。

---

## 5. 建议落地路线（三阶段，每阶段可独立验收）

**阶段 0 —— 重构 build.rs 便于旁路（小改）**
把 main() 拆为「prebuilt 分支 vs 源码分支」：先 `resolve_prebuilt_dir()`，命中即走新 link 逻辑并
`return`，CMake 块整体跳过；提取 `emit_platform_links` 等为两分支共用。不改变默认行为。

**阶段 1 —— `AUDIOCPP_PREBUILT_DIR` 手动目录 + CI 打资产（风险最低，先做）**
- 新增 `AUDIOCPP_PREBUILT_DIR`（含 lib/lib64/bin 子路径），复用现有 extract/assert/link；capi 与 bindgen 照常；
- 移植 `prebuilt-llama.yml` + matrix：**linux(x64/arm64)×{cpu,vulkan}、windows(x64)×{cpu,vulkan}、
  macos(arm64)×{cpu,metal}，static 为主**（dynamic 二期）；
- 移植 collect 脚本，过滤文件名改为 `engine_runtime|ggml|sentencepiece|cjson_vendor|yaml_vendor`；
- 资产 tag = audio.cpp submodule commit，`metadata.json` 记 commit/models；
- 验收：`AUDIOCPP_PREBUILT_DIR=<解压目录> cargo build -p audio-cpp-sys` 在 win32 通过且跳过 CMake 输出。

**阶段 2 —— `prebuilt` cargo feature 自动下载**
- 复制 `prebuilt_download.rs`（改前缀 AUDIOCPP、`asset_name` 拼 modelset/backend、`variant_suffix` 映射音讯后端集合）；
- 缓存到 `target/audio-cpp-prebuilt-cache/<commit>/<asset>`；`LLAMA_*`→`AUDIOCPP_*` 系列 env；解压后无库/404/离线 → 源码回落；
- 消费端 `metadata.audio_commit != submodule HEAD` → warning + 回落（对应 llama 的 patches fail-closed）；
- 验收：win32 `--features prebuilt` 直接拉归档秒装。

**阶段 3（可选）—— 动态链接**
如需 DLL/.so 化，需给 `engine_runtime` 设计 C ABI 导出（capi 已具备），再在 CI 增 `dynamic` cell；
或保持 static-only（对 release 收益有限，llama 数据亦显示 dynamic 更慢）。

> **落地状态（2026-08-13）**：阶段 0–2 均已完成并验证：
> - 阶段 1：`AUDIOCPP_PREBUILT_DIR` 手动目录旁路 + CI 资产（见 `.github/workflows/prebuilt-audio-cpp.yml`）；
> - 阶段 2：`prebuilt` feature 自动下载 + superset 回退（只发布 full 资产，见上）；
> - 资产 `metadata.json` 记录 `audio_commit` 与 `msvc_ver`（MSVC 工具集版本），下载端
>   双向校验不符即回落源码，规避 ABI 错配；
> - 实测 win32/MSVC：full-models+vulkan 冷构建普通编译约 274s，预编译约 54s（约 5 倍加速），
>   二次构建 1s。

---

## 6. 风险与注意事项

1. **ABI 身份**：必须校验 submodule commit + 模型集，否则 bindings 来自当前头、库来自旧 commit → 静默崩溃。
2. **MSVC CRT**：预编译 .lib 为默认 `/MD`；消费端若开 `crt-static` 会链接错位，文档注明需源码构建。
3. **build.rs 内联网下载**（非托管的构建副作用）：离线/404 回落逻辑务必保留（llama 已示范）。
4. **GitHub Actions 成本**：约 22→（audio 简化后）~10 个 cell/tag；可用 `workflow_dispatch` 按需重跑。
5. **每个 commit 建 tag 会 tags 膨胀**：可改为只对稳定音频版打 tag + 公开 `metadata`；消费端用 `AUDIOCPP_PREBUILT_TAG` 覆盖。

---

## 7. 最终建议

先做 **阶段 0 + 1**（手动目录 + CI 资产，投入 1–2 天，风险最小、收益立刻可测），验证 win32/MSVC 与
Linux 各 1–2 个 cell 后再决定是否上 `prebuilt` 自动下载 feature。

**架构上 audio-cpp-rs 比 llama-cpp-rs 更适合预编译**——单一 `engine_runtime` 归档小、无 patches、
shim 恒源码，唯一要自创的是「commit + 模型组合」的资产寻址。