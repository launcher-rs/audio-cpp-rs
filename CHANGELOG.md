# 更新日志（CHANGELOG）

本文件记录 audio-cpp-rs 每个版本面向使用者的变化。格式参照 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)：

- 新增条目写入「未发布」段，随代码一起提交；
- 发布时把「未发布」段改名对应版本号并补日期（版本号与工作区 `Cargo.toml` 一致），再打 `v<版本>` 标签；
- 打标签后预编译工作流会自动提取 CHANGELOG 中对应版本段作为 Release 说明，条目请面向使用者描述变化，不写内部实现细节；
- 0.10.0 之前的历史版本未维护本文件，不补记。

## [未发布]

## [0.10.0] - 2026-11-20

### 新增

- 跟进上游 audio.cpp（submodule 从 `d2da530b` 前进到 `5e7273db`），新增 5 个模型族（全量 loader 族 147 → 151，完整清单见 [模型目录](docs/models.md)）：
  - `m5_tts`：M5-TTS 多语自回归 TTS + 参考音频克隆（Qwen3 speech codec 解码；zh/yue/zh-Sichuan/en/de/fr/ja/ko；离线+流式；feature `model-m5-tts`）。
  - `tiny_voca`：Tiny Voca 4.5M 参数英文 TTS，自带 dizione 音素化前端、无需外部 phonemizer，内置 M2/F2 两音色，支持长文本（仅离线；feature `model-tiny-voca`）。
  - `rosvot`：ROSVOT 歌声转符号谱——音符音高/时值与词对齐 MIDI 转写，输出 MIDI 或 JSON 音符事件（仅离线；feature `model-rosvot`）。
  - `soulx_singer`：SoulX-Singer 参考音频条件的歌唱合成（SVS）与歌唱 voice conversion（SVC），zh/en/yue（仅离线；task `music`/`svc`；feature `model-soulx-singer`）。
  - `granite_speech5`：IBM Granite Speech 5.0 2B 自回归多语 ASR + 语音翻译，en/fr/de/es/pt/ja，翻译目标另支持 it/zh（仅离线；feature `model-granite-speech5`）。
- `ModelFamily` 新增对应枚举变体与 `from_path()` 识别，`TaskKind` 无需变化（`svc` 为既有任务类型）。

### 变更

- 上游模型行为随 submodule 升级带入：`zonos2` 新增长文本续写选项 `long_form_continuation` 且分块默认值 300→150；`cosyvoice3` 新增 `speed` 语速选项；`ace_step` 新增 `reference_audio` 音色条件；`granite5asr` 编码器图 arena 默认值调整。全部经通用 options 字符串透传，`audio-cpp` 的 serde 结构向后兼容。
- 推送 `v*` 标签触发的预编译工作流现在会校验标签与 `Cargo.toml` 版本一致，并从本文件提取对应版本段作为 GitHub Release 说明（缺失时退回通用文案并告警）。
