# 预编译资产路线图（0.4.0+）

> 记录 audio-cpp-rs 预编译资产机制的未来优化目标。当前状态（0.4.0）已实现的
> 方案见 [prebuilt_pattern_report.md](prebuilt_pattern_report.md) 与
> [gh-workflow-guide.md](gh-workflow-guide.md)。

## 现状（0.4.0）

- **只发布 `full` 全模型资产**（linux 4 + windows 2 + macos 1，共 7 个 cell）；
- 资产名含 audio.cpp submodule 完整 SHA 前 12 位（如
  `audio-cpp-prebuilt-...-full-static-<commit>.tar.gz`），消费端按自身 commit
  精确请求，404 即立即回落，不再下载解压后才校验；
- CI 触发：`v*` tag、`workflow_dispatch`，**以及 main 上 submodule 指针变动**
  （开发期子模块一 bump 即构建对应 commit 的预编译，无需发版）；
- 下载端 superset 回退：core / `custom-<族>` 组合先尝试精确资产名，404 时
  自动回退下载 `full-<commit>` 资产，仍失败才回落源码构建；
- 归档 `metadata.json` 记 `audio_commit`（前 12 位）+ `msvc_ver`，下载端兜底校验防 ABI 错配。

### 实测数据（win32/MSVC，full-models+vulkan）

| 场景 | 二进制 | 下载 | 冷构建耗时 |
|---|---|---|---|
| full prebuilt | 68.2 MB | 77 MB | ~54s |
| qwen3_asr（custom）源码 | 57.8 MB | 0 | ~270s |

## 问题与权衡

1. **二进制体积**：custom 组合用 prebuilt 会回退到 full，二进制 +10MB（loader
   按需加载，运行期内存差异很小，但发布产物体积增大）；
2. **下载体积**：仅要 qwen3_asr 的用户需下载 77MB full 包（一次性的，好网络几秒）；
3. **编译提速收益显著**：270s → 54s（约 5 倍），且每次增量构建秒级。

## 目标（0.4.0+）：双层资产方案

**思路**：在 full 之外，为常用模型族额外发布"单族小包"（custom-<族>），
下载端按需匹配，未命中再回退 full。

### 具体设计

1. **资产层级**：
   - `full`：全模型（现有，兜底）；
   - `custom-<族>`：常用族单族包（qwen3_asr、moss_tts_nano、citrinet_asr、
     demucs、sortformer_diar 等），每个 = 引擎核心 + 内置 VAD + 该族，体积小；
2. **下载匹配优先级**：精确 `custom-<族1>-<族2>...` → 任一单族 `custom-<族>`
   → `full` → 源码。多族组合先试精确组合名，失败后**逐个尝试已启用的族**，
   命中第一个单族包即可（单族包是纯超集组合的近似，功能上族已包含）；
3. **CI 矩阵扩展**：给常用族各加一个 cell（约 5–8 个新 cell），`fail-fast: false`
   保证个别族失败不影响其他；
4. **体积收益**：单族包预计 15–30MB，custom 用户免下 full 大包、二进制更小；
5. **兜底不变**：未收录族 / 组合仍回退 full / 源码，绝不因缺小包而失败。

### 验收标准

- `model-qwen3-asr + vulkan + prebuilt` 下载 `custom-qwen3_asr` 包（<35MB）
  而非 full（77MB），二进制 <60MB；
- `model-qwen3-asr + model-moss + vulkan + prebuilt` 命中单族回退逻辑，不下载 full；
- 单族资产与 full 资产在相同 backend 下 ABI 一致（同一 audio.cpp commit）。

### 暂不做的原因（避免过度设计）

- 逐族全量发布（20+ 族 × 任意组合）仍不可行——组合爆炸未解决，单族小包
  只覆盖"最常用单族"这一可穷举子集；
- 二进制体积差异（10MB）对多数用户可接受，双层方案优先级低于功能性改进。
