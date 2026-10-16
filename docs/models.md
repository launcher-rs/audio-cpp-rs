# 模型目录（Model Catalog）

本页按任务分类列出 **audio-cpp-rs**（经上游 [audio.cpp](https://github.com/0xShug0/audio.cpp)）支持的全部模型族。每条给出：

- **族**：上游 family 名，即加载 GGUF 时传给 `family_hint` / `Registry::load` 的字符串，也是 `ModelFamily` 枚举的规范名；
- **专用 feature**：本仓库 `Cargo.toml` 里可直接启用的 `model-<族>` Cargo feature（`—` 表示无专用 feature，改用 `custom-models` + `AUDIOCPP_MODELS=<族>`）；
- **任务**：运行时的**规范 token**，即传给 `Request` / `TaskKind` 的值。上游 `model_specs` 对个别任务用了不同拼写，本页已换算：`music`/`sfx`/`edit`/`audio_generation`→`gen`、`clone`→`clon`、`design`→`vdes`、`speaker`→`spk`、`wake_word`→`wake`、`classify`→`cls`；
- **模式 / 语言 / 状态**：取自上游 `model_specs/<族>.json`，模式中的“流式”指支持流式会话（`streaming`）；**简介**为该模型的中文一句话说明。

> ⚠️ 模型权重**不随本仓库提供**（内置 VAD 除外），下载方式见 [README 的「模型下载」](../README.md#模型下载)。
> GGUF 加载时通常须**显式传 `family_hint`**（引擎自动探测可能误判），用法见 [audio-cpp/README.md](../audio-cpp/README.md)。
> 上游“状态”为 audio.cpp 的成熟度标注：稳定（`supported`）/ 实验（`experimental`）/ 社区（`community`）/ 测试（`testing`）/ 开发中（`wip`），仅供参考。

共收录 **123** 个上游模型族，另有 **2** 个内置 VAD 模型（随引擎编译、权重已 vendored，无需下载）。

## 内置模型（开箱即用，无需下载权重）

| 族 | 模型 | 任务 | 模式 | 语言 | 状态 | 专用 feature | 简介 |
|---|---|---|---|---|---|---|---|
| `silero_vad` | Silero VAD | `vad` | 离线 + 流式 | auto / 多语 | 稳定（内置） | `core-models`（默认） | Silero 语音活动检测，16kHz 单声道，随引擎编译，权重 vendored。 |
| `marblenet_vad` | MarbleNet VAD | `vad` | 离线 | auto / 多语 | 稳定（内置） | `core-models`（默认） | NVIDIA NeMo MarbleNet VAD；NeMo checkpoint 自动探测易误判，须显式传 `family_hint="marblenet_vad"`。 |

## 分类速览

| 分类 | 数量 | 跳转 |
|---|---|---|
| 语音识别（ASR） | 31 | [跳转](#cat-asr) |
| 语音合成（TTS） | 47 | [跳转](#cat-tts) |
| 语音转换 / 声音克隆（VC） | 5 | [跳转](#cat-voice_conversion) |
| 语音分析（VAD · 说话人分离 · 对齐 · 分类 · 唤醒 · 轮次检测） | 7 | [跳转](#cat-speech_analysis) |
| 音频工具（降噪 / 修复 / 超分 / 源分离 / 转谱） | 22 | [跳转](#cat-audio_tools) |
| 音频与音乐生成 | 8 | [跳转](#cat-audio_generation) |
| 其他社区模型 | 3 | [跳转](#cat-community) |

<a id="cat-asr"></a>

## 语音识别（ASR）

把语音转成文本（部分模型同时给时间戳 / 说话人标签）。

| 族 | 模型 | 任务 | 模式 | 语言 | 状态 | 专用 feature | 简介 |
|---|---|---|---|---|---|---|---|
| `audio_flamingo` | Audio Flamingo 3 / Next | `asr` | 离线 | en | 稳定（supported） | `model-audio-flamingo` | Audio Flamingo 3 / Next，支持语音、音乐与环境声的转写、描述与问答。 |
| `audio8_asr` | Audio8-ASR-0.1B | `asr` | 离线 | en, zh, yue, ja, ko, fr, de | 社区（community） | `model-audio8-asr` | Audio8-ASR-0.1B 紧凑多语识别（en/zh/yue/ja/ko/fr/de）：Qwen3-ASR 音频编码器 + MLP-tower 适配 + 8 层 Qwen2 风格解码器，30 秒离线窗口。权重 CC-BY-NC-4.0，GGUF 仅本地转换、不公开分发。 |
| `canary_asr` | Canary 180M Flash | `asr` | 离线 | en, de, es, fr | 稳定（supported） | `model-canary-asr` | NVIDIA Canary 多语语音识别与语音翻译。 |
| `citrinet_asr` | Citrinet ASR | `asr` | 离线 | en | 稳定（supported） | `model-citrinet-asr` | NVIDIA Citrinet 端到端英文 ASR，卷积 CTC 结构，适合短语音转写。 |
| `cohere_asr` | Cohere Transcribe | `asr` | 离线 | en, fr, de, es, it, pt, nl, pl …（共 14 种） | 稳定（supported） | `model-cohere-asr` | Cohere 多语语音转写，支持标点控制。 |
| `confucius4_r2t2` | Confucius4-R2T2 | `asr` | 离线 + 流式 | zh, en, yue, ar, de, fr, es, pt …（共 30 种） | 社区（community） | `model-confucius4-r2t2` | 网易有道 Confucius4-R2T2 实时 ASR：Qwen3-ASR 微调 + 最长稳定前缀（LSP）流式，80ms–2s 分块，支持上下文/热词提示，覆盖 30 语。 |
| `crisperwhisper` | CrisperWhisper 2.0 | `asr` `align` | 离线 + 流式 | af, am, ar, as, az, ba, be, bg …（共 99 种） | 稳定（supported） | `model-crisperwhisper` | 基于 Whisper 的逐字（verbatim）与意图转写。 |
| `fun_asr_nano` | Fun-ASR-Nano | `asr` | 离线 | 自动检测, zh, en, ja | 开发中（wip） | `model-fun-asr-nano` | FunAudioLLM Fun-ASR-Nano-2512 离线多语识别。 |
| `gigaam_asr` | GigaAM ASR | `asr` | 离线 | ru, kk, ky, uz, en | 稳定（supported） | `model-gigaam-asr` | GigaAM v3 / 多语 Conformer 识别，CTC 或 RNN-T 解码。 |
| `granite5asr` | Granite Speech 5.0 470M TurboCTC | `asr` | 离线 + 流式 | en | 稳定（supported） | `model-granite5asr` | IBM Granite Speech 5.0 TurboCTC 470M：紧凑高速英文 ASR，Conformer CTC + 分块自注意力 + 自条件。 |
| `higgs_audio_stt` | Higgs Audio v3 STT | `asr` | 离线 + 流式 | en | 稳定（supported） | — | Boson AI 英文语音转写：Whisper Large v3 编码器 + Qwen 解码器。 |
| `hviske_asr` | Hviske ASR | `asr` | 离线 | ar, da, de, el, en, es, fr, it …（共 15 种） | 稳定（supported） | `model-hviske-asr` | 面向丹麦语优化的 Conformer 编解码 ASR（Hviske v5 微调），可选 Cohere ASR 语种提示。 |
| `hviske_asr_v6` | Hviske v6 | `asr` | 离线 | da | 稳定（supported） | `model-hviske-asr-v6` | 丹麦语识别：旋转位置 Whisper 编码器 + Qwen3 解码器。 |
| `index_echo` | Index-Echo-S2TT | `asr` | 离线 | zh, en, es, ja | 实验（experimental） | `model-index-echo` | Index-Echo 语音转文本翻译，输出双语时间戳字幕。 |
| `kroko_asr` | Kroko Community ASR | `asr` | 离线 + 流式 | de, en, es, fr, it, he, nl, pt, sv, tr | 社区（community） | `model-kroko-asr` | Kroko Community 单语包原生 Zipformer2 RNN-T 识别：离线 + 有状态流式，greedy / 改进 beam 解码、热词、端点分段、部分结果与词级时间戳。 |
| `lfm2_audio` | LFM2.5-Audio | `asr` `tts` `s2s` | 离线 + 流式 | en, ja | 实验（experimental） | `model-lfm2-audio` | Liquid AI LFM2.5-Audio（加载 llama.cpp 格式 GGUF）：ASR 用 FastConformer 编码器接 LFM2 混合主干；TTS 经 depthformer 生成音频帧再由 ISTFT 还原 24kHz；S2S 输入语音问题、输出文字+音频交错。 |
| `moonshine_asr` | Moonshine Streaming ASR | `asr` | 离线 + 流式 | en | 实验（experimental） | `model-moonshine-asr` | Moonshine tiny/small/medium 流式英文识别。 |
| `moss_transcribe_diarize` | MOSS-Transcribe-Diarize | `asr` | 离线 + 流式 | 自动检测, 50+ 种语言 | 稳定（supported） | `model-moss-transcribe-diarize` | Whisper 编码器 + Qwen3 解码器，联合输出 50+ 语言的转写、说话人分离与时间戳。 |
| `nemotron_asr` | Nemotron 3.5 ASR | `asr` | 离线 + 流式 | ar-AR, bg-BG, cs-CZ, da-DK, de-DE, el-GR, en-GB, en-US …（共 40 种） | 稳定（supported） | `model-nemotron-asr` | NVIDIA 600M 流式 ASR，覆盖 40 种语言区域，支持低延迟与批处理、原生标点/大小写、自动语种检测与可配置分块。 |
| `niagara_asr` | Niagara ASR | `asr` | 离线 | en | 稳定（supported） | `model-niagara-asr` | ABR Niagara 英文批处理 ASR（注意力状态空间模型），greedy CTC 解码 + SentencePiece 分词。 |
| `owsm` | OWSM | `asr` | 离线 + 流式 | abk, afr, amh, ara, asm, ast, aze, bak …（共 151 种） | 稳定（supported） | `model-owsm` | ESPnet Open Whisper 风格多语识别与翻译。 |
| `owsm_ctc` | OWSM-CTC | `asr` | 离线 + 流式 | abk, afr, amh, ara, asm, ast, aze, bak …（共 151 种） | 稳定（supported） | `model-owsm-ctc` | ESPnet 非自回归多语识别与翻译，自条件 CTC。 |
| `parakeet_tdt` | Parakeet-TDT 0.6B v3 | `asr` | 离线 + 流式 | bg, cs, da, de, el, en, es, et …（共 25 种） | 社区（community） | `model-parakeet-tdt` | NVIDIA Parakeet-TDT 0.6B v3 FastConformer-TDT，覆盖 25 种欧洲语言并自动语种检测；支持上游 safetensors 包与独立 GGUF，提供离线全上下文、有界窗口长音频与缓冲流式；权重为无界双向注意力，非原生 cache-aware 流式模型。 |
| `qwen3_asr` | Qwen3-ASR | `asr` | 离线 + 流式 | zh, en, yue, ar, de, fr, es, pt …（共 31 种） | 稳定（supported） | `model-qwen3-asr` | Qwen ASR 系列：覆盖 30 语、22 种中文方言与多种英语口音的语种识别与识别，对噪声、长音频与歌唱鲁棒。 |
| `samsone` | SAMSONE | `asr` | 离线 | en | 稳定（supported） | `model-samsone` | SAMSONE 紧凑音频-语言模型：音频描述、问答与声音/音乐/语音理解。 |
| `sense_asr` | SenseVoice-Small | `asr` | 离线 + 流式 | 自动检测, zh, en, yue, ja, ko, pt, ru …（共 23 种） | 社区（community） | `model-sense-asr` | SenseVoice-Small 多语识别，经 SAN-M 编码器 + CTC 头输出丰富的事件/情感/ITN 标签。 |
| `vibeasr` | VibeVoice-ASR-BitNet | `asr` | 离线 | en, zh, fr, it, ko, pt, vi | 社区（community） | `model-vibeasr` | VibeASR.cpp 的 CPU 优先 VibeVoice ASR 移植：INT8（I8_S）音频 VAE 编码器 + 三值（I2_S）Qwen2 解码器。 |
| `vibevoice_asr` | VibeVoice ASR | `asr` | 离线 | 自动检测, 51 种语言 | 稳定（supported） | `model-vibevoice-asr` | 微软长音频转写：一次处理最长 60 分钟音频，输出含说话人、时间戳、内容、热词的结构化转写，覆盖 50+ 语言。 |
| `vibevoice_asr_streaming` | VibeVoice ASR Streaming | `asr` | 离线 + 流式 | en, zh, es, pt, de, ja, ko, fr, ru, it | 稳定（supported） | `model-vibevoice-asr` | 微软 VibeVoice ASR 流式模型（7B 与 1.5B），分块流式转写并保持解码器状态。 |
| `voxtral_realtime` | Voxtral Mini 4B Realtime | `asr` | 离线 + 流式 | en, zh, hi, es, ar, fr, pt, ru …（共 13 种） | 稳定（supported） | — | Mistral 13 语实时 ASR，原生流式因果音频编码器，可配置低延迟转写，精度接近离线开源系统。 |
| `whistle_asr` | Whistle ASR | `asr` | 离线 | en, de, fr, es, it, nl, pl | 实验（experimental） | `model-whistle-asr` | Whistle 离线识别，本地转换 FP32 GGUF，运行于 CPU 或 Metal。 |

<a id="cat-tts"></a>

## 语音合成（TTS）

把文本合成为语音（含声音克隆 / 语音设计等变体）。

| 族 | 模型 | 任务 | 模式 | 语言 | 状态 | 专用 feature | 简介 |
|---|---|---|---|---|---|---|---|
| `audio8_tts` | Audio8 TTS Preview 0.6B | `tts` `clon` | 离线 + 流式 | 自动检测, yue, zh, nl, en, fr, de, it …（共 12 种） | 社区（community） | `model-audio8-tts` | Audio8 TTS Preview 0.6B DualAR 语音合成，覆盖粤/中/荷/英/法/德/意/日/韩/波/西共 11 语，自动语种处理，支持短参考快速克隆。 |
| `auk` | AuK | `tts` `clon` `gen` | 离线 | 自动检测 | 实验（experimental） | `model-auk` | 腾讯 AuK base / AuK-Flash 语音生成与编辑模型。 |
| `breeze_tts` | BreezeTTS 2 | `tts` `clon` `vdes` | 离线 + 流式 | zh, en | 稳定（supported） | `model-breeze-tts` | BreezeTTS 2 原生 GGUF：指令条件 TTS + 提示音频克隆，T5Gemma 文本条件、Qwen 风格声学码生成、深度码本解码、Mimi 波形还原。 |
| `chatterbox` | Chatterbox | `tts` `clon` `vc` | 离线 | ar, da, de, el, en, es, fi, fr …（共 19 种） | 稳定（supported） | — | 开源 Chatterbox 系列：情感夸张可控的表现力 TTS 与声音转换，支持快速生成、零样本克隆与多语 TTS 路径。 |
| `chatterbox_turbo` | Chatterbox Turbo | `tts` | 离线 | en | 测试（testing） | `model-chatterbox-turbo` | Resemble AI 蒸馏版 350M Chatterbox Turbo：GPT2 T3 主干 + GPT2 BPE 分词 + 2 步 meanflow S3Gen 解码，内置音色的快速英文 TTS。 |
| `confucius4_tts` | Confucius4-TTS | `tts` `clon` | 离线 + 流式 | zh, en, ja, ko, de, fr, es, id …（共 14 种） | 实验（experimental） | — | Confucius4-TTS 多语克隆 TTS（离线+流式）：参考音色 + 语种感知文本规范化、T2S 语义生成、S2A 流匹配、风格编码、语义音频特征与 BigVGAN 声码。 |
| `cosyvoice3` | CosyVoice3 | `tts` `clon` | 离线 | zh, en, ja, ko, de, es, fr, it, ru, yue | 稳定（supported） | `model-cosyvoice3` | Fun-CosyVoice3 原生 GGUF：零样本 / 跨语种 / 指令条件 TTS，CosyVoice3 语音 token 自回归 + 因果掩码流 mel 解码 + CAM++ 说话人条件 + 因果 HiFT 波形解码。 |
| `dots_tts` | DotTTS | `tts` `clon` | 离线 + 流式 | 多语 | 稳定（supported） | — | DotTTS 多语零样本 TTS / 克隆 / 语音编辑：SOAR + MeanFlow、提示音频+文本预填充、说话人参考条件、语言标签、多模板、长文分块与真流式输出。 |
| `dramabox` | DramaBox | `tts` `clon` | 离线 | en | 实验（experimental） | — | DramaBox 英文表现力 TTS 与克隆（独立 GGUF）：Gemma 文本条件 + 扩散采样 + 参考音频条件 + 长文分块，48kHz 立体声输出。 |
| `echo_tts` | Echo-TTS | `tts` `clon` | 离线 | en | 实验（experimental） | `model-echo-tts` | Echo-TTS 英文零样本克隆 TTS：2.8B 扩散 Transformer 在 PCA 空间生成 80 维潜变量，经 Fish S1-DAC 解码为 44.1kHz；生成为固定 29.72s（640 潜帧）窗口。 |
| `f5_tts` | F5-TTS | `tts` `clon` | 离线 | en, ar | 社区（community） | `model-f5-tts` | 社区 F5-TTS 流匹配扩散 Transformer，零样本克隆（含 Habibi-TTS 阿拉伯语微调）：文本卷积条件器 + RoPE DiT + CFM 采样 + Vocos 声码。 |
| `firered_audio` | FireRedAudio | `asr` `tts` `clon` `vdes` `gen` | 离线 | zh, en | 实验（experimental） | `model-firered-audio` | FireRedAudio 原生 GGUF：Qwen3.5 混合主干 + FireRed RedAE 条件 + RedPatchEncoder + RedDiT 潜流 + RedAE 波形解码。 |
| `fireredtts3` | FireRedTTS3 | `tts` `clon` `vdes` | 离线 | 中文, 英语, 粤语, 日语, 韩语, 西班牙语, 法语, 俄语 …（共 45 种） | 实验（experimental） | `model-fireredtts3` | FireRedTTS3 系列：Base 零样本克隆与 Instruct 指令 TTS、声音设计、语义/声学编辑，Qwen3 自回归隐状态 + FireRed DiT 潜 patch 流 + RedAE 波形解码。 |
| `fish_audio` | Fish Audio S2 Pro | `tts` `clon` | 离线 + 流式 | 80+ 种语言 | 稳定（supported） | `model-fish-audio` | Fish Audio S2 Pro 语音合成，覆盖 80+ 语言，自动语种处理，支持行内韵律/情感控制与短参考快速克隆。 |
| `fish_drama3` | Fish Audio Drama 3 | `tts` `clon` | 离线 + 流式 | 自动检测 | 实验（experimental） | `model-fish-drama3` | Fish Audio Drama 3 原生 MoE 语音生成、声音克隆与续写。 |
| `glm_tts` | GLM-TTS | `tts` `clon` | 离线 | zh, en | 社区（community） | — | 社区中英零样本合成与克隆：原生 Llama + Whisper-VQ + Flow/DiT + CAMPPlus + HiFT。 |
| `higgs_audio_tts` | Higgs Audio v3 TTS | `tts` `clon` | 离线 + 流式 | 100+ 种语言 | 稳定（supported） | — | Boson AI 对话式 TTS，覆盖 100+ 语言，零样本克隆，行内控制情感、风格、韵律、停顿与音效。 |
| `index_tts2` | IndexTTS2 | `tts` `clon` | 离线 | zh, en, ja, es, ar | 稳定（supported） | — | IndexTTS2 零样本 TTS：克隆、情感-说话人解耦、文本或音频情感控制、显式时长控制；v2 覆盖中英，v2.5 增加日/西/阿等多语 tiktoken 分词。 |
| `inflect_v2` | Inflect Micro v2 | `tts` | 离线 | en | 社区（community） | — | 紧凑英文 VITS TTS，原生 GGML 推理 + 外部 eSpeak-ng 音素化。 |
| `irodori_tts` | Irodori-TTS | `tts` `clon` `vdes` | 离线 + 流式 | ja | 稳定（supported） | `model-irodori-tts` | 日语 TTS：基于 RF-DiT 连续音频潜表示，支持零样本克隆、自动时长预测、多模态声音设计与 emoji 风格控制。 |
| `kitten_tts` | KittenTTS Mini 0.8 | `tts` | 离线 | en | 社区（community） | `model-kitten-tts` | KittenTTS Mini 0.8 英文 TTS，8 个内置音色，共享 eSpeak-ng 音素化。 |
| `kitten_tts2` | Kitten TTS 2 | `tts` `clon` | 离线 | en, ar, zh, fr, de, hi, it, pt, ru, es | 实验（experimental） | `model-kitten-tts2` | Kitten TTS 2 多语生成：Qwen3 语音生成 + S3 meanflow 解码 + 参考音频克隆。 |
| `kokoro_tts` | Kokoro 82M | `tts` | 离线 + 流式 | en-us, en-gb, es, fr-fr, hi, it, ja, pt-br, zh | 社区（community） | `model-kokoro-tts` | Kokoro 82M 多语 TTS，54 个预置音色，优化 CPU 推理，共享 eSpeak-ng 音素化。 |
| `kugelaudio` | KugelAudio-0-Open | `tts` | 离线 + 流式 | en, de, fr, es, it, pt, nl, pl …（共 23 种） | 实验（experimental） | `model-kugelaudio` | KugelAudio TTS，内置预置音色，支持增量音频输出。 |
| `magpie_tts` | MagpieTTS Multilingual 357M | `tts` | 离线 | ar-AE, ar-MSA, ar-SA, de, en, es, fr, hi …（共 13 种） | 稳定（supported） | `model-magpie-tts` | NVIDIA MagpieTTS 多语 TTS：由文本生成 NanoCodec 音频 token（内置说话人上下文嵌入 + 纯解码器 Transformer + 自回归局部 Transformer），再解码为波形。 |
| `maya1` | Maya1 | `tts` `vdes` | 离线 | en | 稳定（supported） | `model-maya1` | Maya Research 3B 表现力英文 TTS，支持自然语言声音设计与行内情感控制。 |
| `miotts` | MioTTS | `tts` `clon` | 离线 | en, ja | 稳定（supported） | — | 基于 MioCodec 的轻量 LLM 英文/日文 TTS，支持低延迟生成与短参考零样本克隆。 |
| `mira_tts` | MiraTTS | `tts` `clon` | 离线 + 流式 | en | 实验（experimental） | `model-mira-tts` | MiraTTS 社区克隆 TTS：Qwen2 自回归语音 token 生成 + ECAPA/Perceiver 说话人分词 + 条件声学处理 + DAC 解码。 |
| `moss_tts_local` | MOSS-TTS-Local | `tts` `clon` | 离线 | ar, cs, da, de, el, en, es, fa …（共 31 种） | 稳定（supported） | `model-moss` | MOSS-TTS 旗舰：高保真 31 语与语码混合语音、零样本克隆、长文生成，支持拼音/音素/时长细粒度控制。 |
| `moss_tts_nano` | MOSS-TTS-Nano | `tts` `clon` | 离线 + 流式 | ar, cs, da, de, el, en, es, fa …（共 19 种） | 稳定（supported） | `model-moss` | MOSS-TTS-Nano 紧凑部署优先，实时多语生成、轻量集成与零样本克隆。 |
| `neutts` | NeuTTS | `tts` | 离线 + 流式 | en | 稳定（supported） | — | Neuphonic 英文 TTS 系列；当前 2E 包用 Qwen3 风格自回归 token 主干 + NeuCodec 波形解码 + 内置音色提示与情感 token 控制。 |
| `omnivoice` | OmniVoice | `tts` `clon` `vdes` | 离线 + 流式 | 600+ 种语言 | 稳定（supported） | — | k2-fsa 大规模多语零样本 TTS，覆盖 600+ 语言，支持短参考克隆、属性化声音设计、发音控制与非语言标签。 |
| `outetts` | Llama-OuteTTS 1.0 | `tts` `clon` | 离线 | ar, be, bn, de, en, es, fa, fr …（共 23 种） | 社区（community） | — | 基于 Llama 的开源权重 TTS，23 语合成，短参考单样本克隆并支持自动词对齐。 |
| `personaplex` | PersonaPlex | `s2s` | 离线 + 流式 | en | 稳定（supported） | `model-personaplex` | PersonaPlex：Moshi 风格全双工语音对话模型，Mimi 音频分词，内置说话人/人设提示，流式音频输入 + 文本生成 + 合成回复语音。 |
| `piper_tts` | Piper TTS | `tts` | 离线 | en | 社区（community） | `model-piper-tts` | Piper TTS 音色的原生 VITS 推理，使用共享 eSpeak-ng 前端。 |
| `pocket_tts` | PocketTTS | `tts` `clon` | 离线 + 流式 | en, fr, de, it, pt, es | 稳定（supported） | — | Kyutai 100M 参数 CPU 友好 TTS 包：英/法/德/意/葡/西的实时本地合成与小体积克隆。 |
| `qwen3_tts` | Qwen3-TTS | `tts` `clon` `vdes` | 离线 + 流式 | zh, en, ja, ko, de, fr, ru, pt, es, it | 稳定（supported） | `model-qwen3-tts` | Qwen TTS 系列：10 语可控合成，3 秒克隆、CustomVoice 指令控制预置音色、VoiceDesign 由自然语言描述设计音色。 |
| `sanotts` | sanoTTS Nano | `tts` | 离线 | en, vi, id, cs, de, es, fr, it …（共 14 种） | 社区（community） | `model-sanotts` | sanoTTS Nano 超小多语 TTS（14 语）：nano 系（时长学生 + 上下文声学学生 → mel-100 + 噪声驱动 ConvNeXt-1D 解码 + iSTFT，音色 heart / heart-nano，24kHz）与确定性 piperlite 系（时长/声学学生 → 192 通道潜表示（可选校准适配）→ 3 级 ConvTranspose1d + 空洞残差，音色 amy / hfc / kristin，22.05kHz）；使用外部 eSpeak-ng 音素化。 |
| `soprano_tts` | Soprano | `tts` | 离线 + 流式 | en | 社区（community） | `model-soprano-tts` | Soprano 超轻量（约 80M）纯英文 TTS：17 层 Qwen3 风格因果 LM 自回归输出每帧 512 维特征，非迭代 Vocos 式解码器还原 32kHz 音频，无扩散细化。 |
| `sopro_tts` | Sopro V2 Turbo | `tts` `clon` | 离线 + 流式 | en, pt, fr, de | 社区（community） | `model-sopro-tts` | 社区 Sopro V2 Turbo（samuel-vitorino/sopro-v2-turbo）120M 零样本克隆 TTS：SentencePiece 分词 + 风格前缀条件的 FSQ 语义 LM + rectified-flow 声学 DiT + Vocos iSTFT 声码，24kHz。 |
| `supertonic` | Supertonic 3 | `tts` | 离线 + 流式 | en, ko, ja, ar, bg, cs, da, de …（共 31 种） | 稳定（supported） | — | Supertone 端侧 TTS，覆盖 31 语，预置音色，面向浏览器/移动/桌面的紧凑快速本地合成。 |
| `vibevoice` | VibeVoice | `tts` | 离线 + 流式 | en, zh | 稳定（supported） | — | 微软长音频多说话人 TTS，面向播客等对话音频，最长 90 分钟、最多 4 说话人。 |
| `vieneu_v3_turbo` | VieNeu-TTS v3 Turbo | `tts` `clon` | 离线 | vi, en | 社区（community） | `model-vieneu-v3-turbo` | VieNeu-TTS v3 Turbo 端侧越/英 TTS（48kHz），3–5 秒参考即时克隆；文本输入为 SEA-G2P 音素。 |
| `voxcpm1` | VoxCPM1 | `tts` `clon` | 离线 + 流式 | zh, en, ja, ko | 稳定（supported） | `model-voxcpm1` | OpenBMB VoxCPM 0.5B 无分词器 TTS，支持短参考克隆与流式输出（16kHz）。 |
| `voxcpm2` | VoxCPM2 | `tts` `clon` `vdes` | 离线 + 流式 | ar, my, zh, 中文方言, da, nl, en, fi …（共 31 种） | 稳定（supported） | — | OpenBMB 无分词器 TTS：覆盖 30 语与 9 种中文方言，48kHz 输出，支持自然语言声音设计、可控短参考克隆与表现力风格引导。 |
| `voxtral_tts` | Voxtral 4B TTS | `tts` | 离线 + 流式 | en, fr, es, de, it, pt, nl, ar, hi | 稳定（supported） | — | Mistral 基于 Ministral 的自回归 + 流匹配 TTS，内置 20 个预置音色。 |
| `zipvoice` | ZipVoice | `tts` `clon` | 离线 + 流式 | en, zh | 社区（community） | `model-zipvoice` | 社区 ZipVoice / ZipVoice-Distill 流匹配 TTS：TTSZipformer 主干、按提示比例预测时长、带 t-shift 与无分类器引导的 Euler 求解器、Vocos mel-24kHz 声码。 |

<a id="cat-voice_conversion"></a>

## 语音转换 / 声音克隆（VC）

在保留内容的前提下改变音色，或参考说话人的音色转换。

| 族 | 模型 | 任务 | 模式 | 语言 | 状态 | 专用 feature | 简介 |
|---|---|---|---|---|---|---|---|
| `meanvc2` | MeanVC2 | `vc` | 离线 + 流式 | 语言无关 | 稳定（supported） | `model-meanvc2` | MeanVC2 轻量流式零样本声音转换；当前包覆盖推荐的 120ms/40ms 质量路径。 |
| `rvc` | RVC | `vc` | 离线 | 语言无关 | 实验（experimental） | — | RVC 离线检索式声音转换：原生 HuBERT 内容特征 + RMVPE 音高提取 + 可选 IVF 检索混合，内置 v1/v2 音色并支持自备 RVC 权重。 |
| `seed_vc` | Seed-VC | `vc` `svc` | 离线 | 语言无关 | 稳定（supported） | — | Seed-VC 零样本声音/歌声转换：从参考音频迁移音色与风格，支持低延迟实时转换与可选轻量微调。 |
| `tone_color_vc` | Tone Color VC | `vc` | 离线 | 语言无关 | 实验（experimental） | `model-tone-color-vc` | Tone Color VC 独立声音转换，使用参考说话人录音。 |
| `vevo2` | Vevo2 | `tts` `gen` `vc` `svc` `s2s` | 离线 | en, zh | 稳定（supported） | — | Vevo2 统一可控框架：中英语音与歌声生成、声音转换与编辑，分词器解耦内容、韵律、旋律、风格与音色。 |

<a id="cat-speech_analysis"></a>

## 语音分析（VAD · 说话人分离 · 对齐 · 分类 · 唤醒 · 轮次检测）

不做转录，而是分析音频结构（人声活动、说话人、对齐、事件分类等）。

| 族 | 模型 | 任务 | 模式 | 语言 | 状态 | 专用 feature | 简介 |
|---|---|---|---|---|---|---|---|
| `ast_audioset` | AST AudioSet | `cls` | 离线 | 语言无关 | 稳定（supported） | `model-ast-audioset` | MIT 音频频谱图 Transformer，对 527 个 AudioSet 声音类别做多标签分类。 |
| `ced` | CED Audio Tagging | `cls` | 离线 | 语言无关 | 稳定（supported） | `model-ced` | CED（一致性集成蒸馏 ViT）音频分类，覆盖 527 个 AudioSet 类别。 |
| `mms_forced_aligner` | Meta MMS-300M Forced Aligner | `align` | 离线 | 多语 | 社区（community） | `model-mms-forced-aligner` | Meta MMS-300M 多语 CTC 强制对齐（Wav2Vec2ForCTC）：把精确文本对齐到音频并给出词级时间戳。权重覆盖 1130 语，但本移植的原生文本规范化仅支持荷兰语与英语拉丁字母；可用 text_normalization=pre_romanized 传入 ASCII 罗马化，不支持的非拉丁输入会被拒绝。权重 CC-BY-NC-4.0，GGUF 仅本地转换、不公开分发。 |
| `nemotron_3_diar` | Nemotron 3 Diarization | `diar` | 离线 + 流式 | 多语 | 稳定（supported） | `model-nemotron-3-diar` | NVIDIA 八说话人流式分离，采用到达顺序身份与可配置延迟档位。 |
| `qwen3_forced_aligner` | Qwen3 Forced Aligner | `align` | 离线 | zh, en, yue, fr, de, it, ja, ko …（共 11 种） | 稳定（supported） | `model-qwen3-forced-aligner` | Qwen3 非自回归强制对齐：把文本对齐到语音，11 语下给出词级/字级时间戳。 |
| `sortformer_diar` | Sortformer Diarization | `diar` | 离线 | en | 稳定（supported） | `model-sortformer-diar` | NVIDIA 基于 Transformer 的端到端说话人分离，主要英语训练，直接从音频预测说话人标签并按到达时间排序，最多四说话人。 |
| `sortformer_diar_v2` | Sortformer Diarization v2.1 | `diar` | 离线 + 流式 | 多语 | 社区（community） | `model-sortformer-diar-v2` | NVIDIA Sortformer v2.1 流式说话人分离，四说话人通道、AOSC 状态、稳定到达顺序身份，支持有界上下文离线与流式；无官方语种白名单（以英语为主，含 AISHELL-4/AliMeeting 等非英语会议数据，其他语言可能退化）。权重受 NVIDIA Open Model License 约束，获再分发批准前仅限本地使用。 |

<a id="cat-audio_tools"></a>

## 音频工具（降噪 / 修复 / 超分 / 源分离 / 转谱）

通用音频处理：降噪、语音修复、超分、音源分离、录音转谱等。

| 族 | 模型 | 任务 | 模式 | 语言 | 状态 | 专用 feature | 简介 |
|---|---|---|---|---|---|---|---|
| `apollo` | Apollo | `s2s` | 离线 | 语言无关 | 稳定（supported） | `model-apollo` | 面向 44.1kHz 压缩音频的频带序列音乐修复模型。 |
| `audiosr` | AudioSR | `s2s` | 离线 | 自动检测 | 实验（experimental） | `model-audiosr` | AudioSR：基于潜扩散的音频超分辨率（输入波形 → 高清波形）。 |
| `bs_roformer` | BS-RoFormer | `sep` | 离线 | 语言无关 | 稳定（supported） | `model-roformer` | BS-RoFormer 音乐分离：在显式非重叠频带上估计人声，并从混音推导伴奏。 |
| `builtin_audio_utils` | Built-in Audio Utilities | `s2s` | 离线 | 自动检测 | 实验（experimental） | `model-builtin-audio-utils` | 内置降噪 / 增强 / 超分工具（权重需另行下载）。 |
| `controlfoley` | ControlFoley | `gen` | 离线 | 自动检测 | 实验（experimental） | `model-controlfoley` | 多模态 Foley 音效生成模型。 |
| `firered_vad` | FireRed VAD | `vad` | 离线 + 流式 | 语言无关 | 实验（experimental） | `model-firered-vad` | FireRed DFSMN 语音活动检测，含离线与因果 Stream-VAD 两种权重。 |
| `htdemucs` | HTDemucs | `sep` | 离线 | 语言无关 | 稳定（supported） | `model-demucs` | HTDemucs 音乐分离：波形+频谱 Transformer，分为人声 / 鼓 / 贝斯 / 伴奏。 |
| `htdemucs_6stems` | HTDemucs 6-stem | `sep` | 离线 | 语言无关 | 稳定（supported） | `model-demucs` | HTDemucs 6 轨分离：波形+频谱 Transformer，分为鼓 / 贝斯 / 人声 / 其他 / 吉他 / 钢琴。 |
| `mel_band_roformer` | Mel-Band RoFormer | `sep` | 离线 | 语言无关 | 稳定（supported） | `model-roformer` | Mel-Band RoFormer 分离：mel 尺度重叠频带 + RoFormer 注意力，估计人声 / 鼓 / 伴奏等。 |
| `micro_wake_word` | microWakeWord | `wake` | 离线 + 流式 | 随模型而定 | 实验（experimental） | `model-micro-wake-word` | microWakeWord MixedNet（TFLite 转 GGUF）流式唤醒词检测。 |
| `miocodec` | MioCodec | `vc` `s2s` | 离线 | 语言无关 | 稳定（supported） | — | MioCodec 轻量神经音频编解码：把 44.1kHz 音频编码为 25Hz 低码率离散 token，再带说话人条件解码。 |
| `mossformer2` | MossFormer2 | `sep` | 离线 | 语言无关 | 稳定（supported） | `model-mossformer2` | MossFormer2 16kHz 重叠语音分离。 |
| `muscriptor` | MuScriptor | `midi` | 离线 + 流式 | 音乐 | 稳定（supported） | — | MuScriptor 音频转符号：把音乐音频转为符号音符事件或 MIDI，支持乐器约束、采样、beam 搜索、批量分块与流式最终结果输出。 |
| `pulsevad` | PulseVAD | `vad` | 离线 | 语言无关 | 稳定（supported） | `model-pulsevad` | 16kHz 单声道语音活动检测，提供 2.1K 学生与 81K 教师权重。 |
| `reuse` | RE-USE | `s2s` | 离线 | 语言无关 | 稳定（supported） | `model-reuse` | RE-USE 采样率无关语音修复，双向 Mamba。 |
| `sam_audio` | SAM Audio | `s2s` | 离线 | 语言无关 | 稳定（supported） | `model-sam-audio` | 提示条件音频分离。 |
| `sheetsage2` | SheetSage2 | `midi` | 离线 | 音乐 | 稳定（supported） | `model-sheetsage2` | SheetSage2 音频转符号：从自包含 GGUF 读取录音并输出 ABC 乐谱产物。 |
| `sherpa_kws` | sherpa-onnx Zipformer KWS | `wake` | 离线 + 流式 | 随模型而定 | 实验（experimental） | `model-sherpa-kws` | sherpa-onnx Zipformer transducer 流式关键词检测。 |
| `sidon` | Sidon v0.1 | `s2s` | 离线 | 语言无关 | 稳定（supported） | `model-sidon` | Sidon v0.1 单说话人语音修复，48kHz 输出。 |
| `smart_turn` | Smart Turn v3.2 | `turn` | 离线 | 自动检测 | 稳定（supported） | `model-smart-turn` | Smart Turn v3.2 音频原生轮次完成检测（Whisper-Tiny 编码器 + 注意力池化分类）。 |
| `tf_gridnet` | TF-GridNet | `sep` | 离线 | 语言无关 | 稳定（supported） | `model-tf-gridnet` | TF-GridNet 时频重叠语音分离。 |
| `universr` | UniverSR | `s2s` | 离线 | 语言无关 | 稳定（supported） | `model-universr` | UniverSR：复数 STFT 流匹配音频超分至 48kHz。 |

<a id="cat-audio_generation"></a>

## 音频与音乐生成

从文本或音频条件生成音乐 / 音效 / 更长的音频。

| 族 | 模型 | 任务 | 模式 | 语言 | 状态 | 专用 feature | 简介 |
|---|---|---|---|---|---|---|---|
| `ace_step` | ACE-Step | `gen` | 离线 | 50+ 种语言 | 稳定（supported） | — | 开源 ACE-Step 1.5 音乐生成模型，支持 50+ 语言的文生音乐与歌词转歌，并提供翻唱、重绘、续写、分轨、人声转伴奏等编辑路径。 |
| `heartmula` | HeartMuLa | `gen` | 离线 | 多语 | 稳定（supported） | — | 开源音乐基础模型：由文本、歌词提示与风格标签生成完整歌曲，结构可控、多语歌词条件。 |
| `liveavatar` | LiveAvatar | `gen` | 离线 + 流式 | 自动检测 | 实验（experimental） | `model-liveavatar` | LiveAvatar 音频驱动数字人视频生成（Wan2.2 S2V 架构）。 |
| `midashenglm_gen` | MiDashengLM-Gen | `gen` | 离线 | 自动检测 | 实验（experimental） | `model-midashenglm-gen` | DashengLM 生成式音频模型，支持语音、音乐、音效与环境声提示生成。 |
| `minimax_h3` | MiniMax-H3 | `tts` `gen` | 离线 | 自动检测 | 实验（experimental） | — | MiniMax-H3 文生音视频（官方 T2VA 管线）：Qwen3-VL 文本条件 + 音视频联合 DiT 去噪 + MiniMax-H3 音视频 VAE 解码。 |
| `minimax_music3` | MiniMax Music 3 | `gen` | 离线 | 自动检测 | 实验（experimental） | `model-minimax-music3` | MiniMax Music 3 文生音乐，支持歌词条件。 |
| `stable_audio` | Stable Audio 3 | `gen` | 离线 | en | 稳定（supported） | — | Stability AI 生成式音频系列：文生音乐/音效、音频到音频编辑、修复、续写、变长生成与 LoRA 个性化。 |
| `yue2` | YuE2 | `gen` | 离线 | en | 稳定（supported） | `model-yue2` | YuE2 音乐生成：符号 ABC 规划 + 语义 codec 生成 + NAR 声学流合成 + Oobleck VAE 解码。 |

<a id="cat-community"></a>

## 其他社区模型

未归入上述类别的社区贡献模型。

| 族 | 模型 | 任务 | 模式 | 语言 | 状态 | 专用 feature | 简介 |
|---|---|---|---|---|---|---|---|
| `moss_tts_v15` | MOSS-TTS-v1.5 | `tts` `clon` | 离线 | en, zh | 实验（experimental） | `model-moss-tts-v15` | 8B delay-pattern MOSS-TTS：短参考零样本克隆，24kHz，带显式时长预算。 |
| `moss_ttsd` | MOSS-TTSD | `tts` `clon` | 离线 | en, zh | 实验（experimental） | `model-moss-ttsd` | 8B delay-pattern 对话 TTS：一遍生成带说话人标记的对话，逐说话人零样本克隆，24kHz、16 码本。 |
| `moss_voicegen` | MOSS-VoiceGenerator | `vdes` | 离线 | en, zh | 社区（community） | `model-moss-voicegen` | MOSS 声音设计：用文字指令而非参考录音创建音色，再用该音色朗读文本。 |

## 上游文档索引

下列模型族在上游 `docs/models/` 有专门的说明页（模型结构、转换脚本、权重来源与效果评测）：

- [`ace_step`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/ace_step.md) — ACE-Step
- [`apollo`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/apollo.md) — Apollo
- [`ast_audioset`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/ast_audioset.md) — AST AudioSet
- [`audio_flamingo`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/audio_flamingo.md) — Audio Flamingo 3 / Next
- [`audio8_asr`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/audio8_asr.md) — Audio8-ASR-0.1B
- [`audio8_tts`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/audio8_tts.md) — Audio8 TTS Preview 0.6B
- [`audiosr`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/audiosr.md) — AudioSR
- [`auk`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/auk.md) — AuK
- [`breeze_tts`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/breeze_tts.md) — BreezeTTS 2
- [`canary_asr`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/canary_asr.md) — Canary 180M Flash
- [`ced`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/ced.md) — CED Audio Tagging
- [`chatterbox_turbo`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/chatterbox_turbo.md) — Chatterbox Turbo
- [`cohere_asr`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/cohere_asr.md) — Cohere Transcribe
- [`controlfoley`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/controlfoley.md) — ControlFoley
- [`cosyvoice3`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/cosyvoice3.md) — CosyVoice3
- [`crisperwhisper`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/crisperwhisper.md) — CrisperWhisper 2.0
- [`dots_tts`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/dots_tts.md) — DotTTS
- [`echo_tts`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/echo_tts.md) — Echo-TTS
- [`f5_tts`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/f5_tts.md) — F5-TTS
- [`firered_audio`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/firered_audio.md) — FireRedAudio
- [`firered_vad`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/firered_vad.md) — FireRed VAD
- [`fireredtts3`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/fireredtts3.md) — FireRedTTS3
- [`fish_audio`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/fish_audio.md) — Fish Audio S2 Pro
- [`fish_drama3`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/fish_drama3.md) — Fish Audio Drama 3
- [`fun_asr_nano`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/fun_asr_nano.md) — Fun-ASR-Nano
- [`gigaam_asr`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/gigaam_asr.md) — GigaAM ASR
- [`glm_tts`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/glm_tts.md) — GLM-TTS
- [`granite5asr`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/granite5asr.md) — Granite Speech 5.0 470M TurboCTC
- [`higgs_audio_stt`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/higgs_audio_stt.md) — Higgs Audio v3 STT
- [`higgs_audio_tts`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/higgs_audio_tts.md) — Higgs Audio v3 TTS
- [`hviske_asr_v6`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/hviske_asr_v6.md) — Hviske v6
- [`index_echo`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/index_echo.md) — Index-Echo-S2TT
- [`index_tts2`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/index_tts2.md) — IndexTTS2
- [`inflect_v2`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/inflect_v2.md) — Inflect Micro v2
- [`irodori_tts`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/irodori_tts.md) — Irodori-TTS
- [`kitten_tts`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/kitten_tts.md) — KittenTTS Mini 0.8
- [`kitten_tts2`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/kitten_tts2.md) — Kitten TTS 2
- [`kokoro_tts`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/kokoro_tts.md) — Kokoro 82M
- [`kroko_asr`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/kroko_asr.md) — Kroko Community ASR
- [`kugelaudio`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/kugelaudio.md) — KugelAudio-0-Open
- [`lfm2_audio`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/lfm2_audio.md) — LFM2.5-Audio
- [`liveavatar`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/liveavatar.md) — LiveAvatar
- [`magpie_tts`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/magpie_tts.md) — MagpieTTS Multilingual 357M
- [`maya1`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/maya1.md) — Maya1
- [`meanvc2`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/meanvc2.md) — MeanVC2
- [`micro_wake_word`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/micro_wake_word.md) — microWakeWord
- [`midashenglm_gen`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/midashenglm_gen.md) — MiDashengLM-Gen
- [`minimax_h3`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/minimax_h3.md) — MiniMax-H3
- [`minimax_music3`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/minimax_music3.md) — MiniMax Music 3
- [`miotts`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/miotts.md) — MioTTS
- [`mira_tts`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/mira_tts.md) — MiraTTS
- [`mms_forced_aligner`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/mms_forced_aligner.md) — Meta MMS-300M Forced Aligner
- [`moonshine_asr`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/moonshine_asr.md) — Moonshine Streaming ASR
- [`moss_transcribe_diarize`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/moss_transcribe_diarize.md) — MOSS-Transcribe-Diarize
- [`moss_tts_local`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/moss_tts_local.md) — MOSS-TTS-Local
- [`moss_tts_nano`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/moss_tts_nano.md) — MOSS-TTS-Nano
- [`moss_tts_v15`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/moss_tts_v15.md) — MOSS-TTS-v1.5
- [`moss_ttsd`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/moss_ttsd.md) — MOSS-TTSD
- [`moss_voicegen`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/moss_voicegen.md) — MOSS-VoiceGenerator
- [`mossformer2`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/mossformer2.md) — MossFormer2
- [`muscriptor`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/muscriptor.md) — MuScriptor
- [`nemotron_3_diar`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/nemotron_3_diar.md) — Nemotron 3 Diarization
- [`neutts`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/neutts.md) — NeuTTS
- [`omnivoice`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/omnivoice.md) — OmniVoice
- [`outetts`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/outetts.md) — Llama-OuteTTS 1.0
- [`owsm`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/owsm.md) — OWSM
- [`owsm_ctc`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/owsm_ctc.md) — OWSM-CTC
- [`parakeet_tdt`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/parakeet_tdt.md) — Parakeet-TDT 0.6B v3
- [`personaplex`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/personaplex.md) — PersonaPlex
- [`piper_tts`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/piper_tts.md) — Piper TTS
- [`pulsevad`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/pulsevad.md) — PulseVAD
- [`qwen3_asr`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/qwen3_asr.md) — Qwen3-ASR
- [`qwen3_forced_aligner`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/qwen3_forced_aligner.md) — Qwen3 Forced Aligner
- [`qwen3_tts`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/qwen3_tts.md) — Qwen3-TTS
- [`reuse`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/reuse.md) — RE-USE
- [`sam_audio`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/sam_audio.md) — SAM Audio
- [`samsone`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/samsone.md) — SAMSONE
- [`sanotts`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/sanotts.md) — sanoTTS Nano
- [`seed_vc`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/seed_vc.md) — Seed-VC
- [`sense_asr`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/sense_asr.md) — SenseVoice-Small
- [`sherpa_kws`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/sherpa_kws.md) — sherpa-onnx Zipformer KWS
- [`sidon`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/sidon.md) — Sidon v0.1
- [`smart_turn`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/smart_turn.md) — Smart Turn v3.2
- [`soprano_tts`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/soprano_tts.md) — Soprano
- [`sopro_tts`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/sopro_tts.md) — Sopro V2 Turbo
- [`sortformer_diar_v2`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/sortformer_diar_v2.md) — Sortformer Diarization v2.1
- [`stable_audio`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/stable_audio.md) — Stable Audio 3
- [`tf_gridnet`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/tf_gridnet.md) — TF-GridNet
- [`tone_color_vc`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/tone_color_vc.md) — Tone Color VC
- [`universr`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/universr.md) — UniverSR
- [`vevo2`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/vevo2.md) — Vevo2
- [`vibeasr`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/vibeasr.md) — VibeVoice-ASR-BitNet
- [`vibevoice_asr`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/vibevoice_asr.md) — VibeVoice ASR
- [`vieneu_v3_turbo`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/vieneu_v3_turbo.md) — VieNeu-TTS v3 Turbo
- [`voxcpm1`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/voxcpm1.md) — VoxCPM1
- [`voxtral_realtime`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/voxtral_realtime.md) — Voxtral Mini 4B Realtime
- [`voxtral_tts`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/voxtral_tts.md) — Voxtral 4B TTS
- [`whistle_asr`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/whistle_asr.md) — Whistle ASR
- [`yue2`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/yue2.md) — YuE2
- [`zipvoice`](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/zipvoice.md) — ZipVoice

---

> 本页由 [`scripts/gen-models-doc.ps1`](../scripts/gen-models-doc.ps1) 依据上游 `model_specs/*.json`、`CMakeLists.txt` 与本仓库 `Cargo.toml` 生成
> （中文简介表见 [`scripts/model-desc-zh.tsv`](../scripts/model-desc-zh.tsv)）；随 submodule 升级重跑。族名 / feature 的当前状态以
> `audio-cpp/src/types.rs` 与各 crate 的 `Cargo.toml` 为准。
