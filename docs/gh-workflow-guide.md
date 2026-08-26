# 用 gh CLI 控制预编译 CI（GitHub Actions）

本文档说明如何用 [GitHub CLI](https://cli.github.com)（`gh`）手动触发、跟踪与排查
`prebuilt-audio-cpp.yml` 工作流。该工作流负责构建并发布 audio-cpp 的预编译静态库资产。

> 前置：已安装 gh 并登录（`gh auth login`），对仓库有写权限。本文以
> `launcher-rs/audio-cpp-rs` 为例，仓库名可用环境变量覆盖（见下文）。

## 工作流总览

- **文件**：`.github/workflows/prebuilt-audio-cpp.yml`
- **触发**：
  - `v*` tag 推送：自动构建全部平台并上传到对应 tag 的 release；
  - `workflow_dispatch`（手动）：可指定平台与是否上传，用于按需重跑。
- **矩阵**（`fail-fast: false`，单个 cell 失败不影响其他）：
  | 平台 | cells |
  |---|---|
  | Linux | x64 cpu full / x64 vulkan full / arm64 cpu full / arm64 vulkan full |
  | Windows | x64 cpu full / x64 vulkan full |
  | macOS | arm64 metal full |
- **手动输入参数**：
  - `platform`：`all` / `linux` / `windows` / `macos`（空 = all）
  - `upload_to_release`：是否上传资产到 GitHub Release
  - `tag`：上传目标 release 的 tag（仅当 `upload_to_release` 开启时生效）

## 常用命令

### 1. 触发构建（不发布）

只编译 + 收集资产，验证矩阵能否通过，不上传 release：

```bash
# 全部平台
gh workflow run prebuilt-audio-cpp.yml --repo launcher-rs/audio-cpp-rs \
  -f platform=all -f upload_to_release=false

# 只测 Linux
gh workflow run prebuilt-audio-cpp.yml --repo launcher-rs/audio-cpp-rs \
  -f platform=linux -f upload_to_release=false

# 只测某个后端组合（通过 feature 矩阵 cell 名区分）
# 注意：workflow_dispatch 的 platform 只到平台粒度，无法单选 cell。
```

### 2. 构建并发布到 release

```bash
# 发布全部平台资产到 v0.3.1
gh workflow run prebuilt-audio-cpp.yml --repo launcher-rs/audio-cpp-rs \
  -f platform=all -f upload_to_release=true -f tag=v0.3.1

# 只发布 Linux（含新补的 arm64 vulkan full）
gh workflow run prebuilt-audio-cpp.yml --repo launcher-rs/audio-cpp-rs \
  -f platform=linux -f upload_to_release=true -f tag=v0.3.1
```

> `upload_to_release=true` 时若目标 tag 的 release 不存在，workflow 会自动创建；
> 资产用 `--clobber` 上传，同名资产会被覆盖，可安全重跑。

### 3. 查看运行状态

```bash
# 列出最近运行
gh run list --repo launcher-rs/audio-cpp-rs \
  --workflow prebuilt-audio-cpp.yml -L 5

# 查看某个 run 的 job 级结论
gh run view <run-id> --repo launcher-rs/audio-cpp-rs \
  --json status,conclusion,jobs \
  --jq '.status, (.jobs[] | "\(.name): \(.conclusion // "running")")'

# 阻塞式等待完成（成功返回 0，失败返回非 0）
gh run watch <run-id> --repo launcher-rs/audio-cpp-rs --exit-status --interval 120
```

### 4. 查看失败日志

```bash
# 只输出失败步骤的日志
gh run view <run-id> --repo launcher-rs/audio-cpp-rs --log-failed

# 过滤关键字（如 CMake 错误、缺库）
gh run view <run-id> --repo launcher-rs/audio-cpp-rs --log-failed |
  Select-String -Pattern "error|Could NOT find|missing:"
```

### 5. 管理 release 资产

```bash
# 列出 v0.3.1 的资产
gh release view v0.3.1 --repo launcher-rs/audio-cpp-rs \
  --json assets --jq '.assets[].name'

# 删除误上传的资产
gh release delete-asset v0.3.1 <asset-name> --repo launcher-rs/audio-cpp-rs \
  --yes

# 手动下载资产
gh release download v0.3.1 --repo launcher-rs/audio-cpp-rs \
  --pattern "audio-cpp-prebuilt-windows-*"
```

## 资产命名与下载

- 命名：`audio-cpp-prebuilt-{linux|windows|macos}-{target}-{backend}[-{crt}]-{modelset}-static-{commit}.tar.gz`（`crt` 仅 Windows；`commit` 为 audio.cpp submodule 完整 SHA 前 12 位）。消费端按自身 commit 精确请求，命中才下载、404 即回落。
- 只发布 `full` modelset（任何 model 组合的超集）。消费端 core / `custom-<族>`
  组合 404 时自动回退下载 `full-{commit}` 资产。
- 下载与校验由 `audio-cpp-sys/prebuilt_download.rs` 完成：
  - 校验 `metadata.json.audio_commit`（打包时 submodule HEAD）与本地一致，否则回落源码；
  - 校验 `metadata.json.msvc_ver`（Windows 打包工具链的 `_MSC_VER`），本地 MSVC
    版本偏低则回落源码。
- 相关环境变量：`AUDIOCPP_PREBUILT_TAG` / `_REPO` / `_URL` / `_DIR` / `_OFF`。

## 常见排查

| 现象 | 处理 |
|---|---|
| run 显示 `success` 但 release 没有新资产 | 触发时 `upload_to_release` 用了 false，或 tag 传错；重跑带 `-f upload_to_release=true` |
| 单个 cell 失败 | `fail-fast: false`，其余 cell 正常；用 `--log-failed` 定位该 cell 日志 |
| 某个平台根本没跑 | `platform` 参数过滤了该平台；`gh run view` 里该 job 显示 `skipped` |
| Linux 找不到 glslc | Ubuntu 包名是 `glslc`（不是 `shaderc`）；已写入 workflow |
| 本地下载后链接报 `__std_unique_8` 等 LNK2019 | MSVC 工具集版本低于打包机；本地升级 VS 或改用源码构建 |
