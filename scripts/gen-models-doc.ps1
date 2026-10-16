#Requires -Version 7
<#
.SYNOPSIS
    由上游 model_specs + 本仓库代码生成 docs/models.md（模型目录）。

.DESCRIPTION
    读取 audio-cpp-sys/audio.cpp/model_specs/*.json 的模型元数据，结合
    audio-cpp-sys/audio.cpp/CMakeLists.txt 的 target / loader / alias 与
    audio-cpp/Cargo.toml 的 model-* feature，生成按任务分类的中文模型目录。
    中文简介取自同目录的 model-desc-zh.tsv，缺失的族回落到上游英文描述。

    运行前置：
      * 已补齐 submodule：git submodule update --init --recursive
      * PowerShell 7（pwsh）

.EXAMPLE
    pwsh -File scripts/gen-models-doc.ps1
    pwsh -File scripts/gen-models-doc.ps1 -RepoRoot C:\code\audio-cpp-rs

.NOTES
    升级 audio.cpp submodule 后应重跑本脚本，并同步 scripts/model-desc-zh.tsv
    （新增/改名族）。生成结果直接覆盖 docs/models.md。
#>
[CmdletBinding()]
param(
    # 仓库根目录，默认取本脚本所在目录的上一级
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
)

$ErrorActionPreference = 'Stop'

$specDir   = Join-Path $RepoRoot 'audio-cpp-sys/audio.cpp/model_specs'
$cmakePath = Join-Path $RepoRoot 'audio-cpp-sys/audio.cpp/CMakeLists.txt'
$upDocsDir = Join-Path $RepoRoot 'audio-cpp-sys/audio.cpp/docs/models'
$cargoToml = Join-Path $RepoRoot 'audio-cpp/Cargo.toml'
$zhPath    = Join-Path $PSScriptRoot 'model-desc-zh.tsv'
$outPath   = Join-Path $RepoRoot 'docs/models.md'

if (-not (Test-Path $specDir)) {
    throw "未找到 $specDir —— 请先执行: git submodule update --init --recursive"
}

# ---- 上游 model_specs ----
$specs = @()
foreach ($f in Get-ChildItem $specDir -Filter *.json | Sort-Object Name) {
    $j = Get-Content $f.FullName -Raw -Encoding UTF8 | ConvertFrom-Json
    $specs += [pscustomobject]@{
        family  = $j.family
        display = $j.display_name
        desc    = $j.description
        cat     = $j.category
        status  = $j.status
        tasks   = @($j.tasks)
        modes   = @($j.modes)
        langs   = @($j.languages)
    }
}

# ---- 中文简介表（family<TAB>中文），缺失时回落上游英文描述 ----
$zhMap = @{}
if (Test-Path $zhPath) {
    foreach ($line in Get-Content $zhPath -Encoding UTF8) {
        if (-not $line -or $line -match '^\s*#') { continue }
        $parts = $line -split "`t", 2
        if ($parts.Count -eq 2) { $zhMap[$parts[0].Trim()] = $parts[1].Trim() }
    }
}

# ---- CMake：解析每个 audiocpp_add_model(...) 块（按括号配平，因为有块以
#      "DEPENDS x)" 结尾，朴素的 "\n)" 正则会把下一个块吞进来）----
$lines = Get-Content $cmakePath
$blocks = New-Object System.Collections.Generic.List[object]
$cur = $null
$bal = 0
foreach ($ln in $lines) {
    if ($null -eq $cur -and $ln -cmatch '^audiocpp_add_model\(\s*(\w+)') {
        $cur = [pscustomobject]@{ target = $Matches[1]; text = '' }
        $bal = 0
    }
    if ($null -ne $cur) {
        $cur.text += $ln + "`n"
        $bal += ([regex]::Matches($ln, '\(')).Count - ([regex]::Matches($ln, '\)')).Count
        if ($bal -le 0) {
            $blocks.Add($cur)
            $cur = $null
        }
    }
}
$blocks = $blocks | ForEach-Object {
    $loaders = [regex]::Matches($_.text, 'make_(\w+?)_loader') | ForEach-Object { $_.Groups[1].Value }
    $aliasBlock = [regex]::Match($_.text, 'ALIASES([\s\S]*?)(\n\s{4}[A-Z]+|\z)')
    $aliases = @()
    if ($aliasBlock.Success) {
        $aliases = ($aliasBlock.Groups[1].Value -split '\s+') | Where-Object { $_ -match '^\w+$' }
    }
    [pscustomobject]@{ target = $_.target; loaders = $loaders; aliases = $aliases }
}

# target/alias -> 主 target
$aliasMap = @{}
foreach ($b in $blocks) {
    $aliasMap[$b.target] = $b.target
    foreach ($a in $b.aliases) { $aliasMap[$a] = $b.target }
}

# family -> target（按 loader 名归族）
$familyTarget = @{}
foreach ($b in $blocks) {
    foreach ($ld in $b.loaders) { $familyTarget[$ld] = $b.target }
}

# ---- audio-cpp/Cargo.toml 的 model-* feature ----
$cargo = Get-Content $cargoToml -Raw -Encoding UTF8
$features = [regex]::Matches($cargo, '(?m)^(model-[a-z0-9-]+)\s*=') | ForEach-Object { $_.Groups[1].Value }
$targetFeature = @{}
foreach ($feat in $features) {
    $suffix = $feat.Substring('model-'.Length).Replace('-', '_')
    $tgt = if ($aliasMap.ContainsKey($suffix)) { $aliasMap[$suffix] } else { $suffix }
    if (-not $targetFeature.ContainsKey($tgt)) { $targetFeature[$tgt] = @() }
    $targetFeature[$tgt] += $feat
}

# ---- 上游 docs/models/*.md 集合 ----
$docFiles = @{}
if (Test-Path $upDocsDir) {
    foreach ($d in Get-ChildItem $upDocsDir -Filter *.md) { $docFiles[$d.BaseName] = $true }
}

function Get-FeatureCell($family) {
    if ($familyTarget.ContainsKey($family)) {
        $t = $familyTarget[$family]
        if ($targetFeature.ContainsKey($t)) { return ('`' + ($targetFeature[$t] -join '` / `') + '`') }
    }
    return '—'
}

# 上游 spec 的 tasks 拼写 → 运行时规范 token（见上游 task_vocabulary.cpp）
$taskMap = @{ music = 'gen'; sfx = 'gen'; edit = 'gen'; audio_generation = 'gen'; clone = 'clon'; design = 'vdes'; speaker = 'spk'; wake_word = 'wake'; classify = 'cls' }
function Get-TaskCell($tasks) {
    $tokens = $tasks | ForEach-Object { if ($taskMap.ContainsKey($_)) { $taskMap[$_] } else { $_ } } | Select-Object -Unique
    if (-not $tokens) { return '—' }
    return ('`' + ($tokens -join '` `') + '`')
}

$statusMap = @{ supported = '稳定'; experimental = '实验'; community = '社区'; testing = '测试'; wip = '开发中' }
# 语言列里非 ISO 码的取值本地化
$langMap = @{
    'auto' = '自动检测'; 'language_agnostic' = '语言无关'; 'model_dependent' = '随模型而定';
    'multilingual' = '多语'; 'music' = '音乐'; 'nospeech' = '无语音'; 'zh dialects' = '中文方言';
    'Chinese' = '中文'; 'English' = '英语'; 'Cantonese' = '粤语'; 'Japanese' = '日语';
    'Korean' = '韩语'; 'Spanish' = '西班牙语'; 'French' = '法语'; 'German' = '德语';
    'Russian' = '俄语'; 'Arabic' = '阿拉伯语'; 'Hindi' = '印地语'; 'Italian' = '意大利语';
    'Portuguese' = '葡萄牙语'; 'Dutch' = '荷兰语'; 'Polish' = '波兰语'; 'Turkish' = '土耳其语';
    'Ukrainian' = '乌克兰语'; 'Vietnamese' = '越南语'; 'Thai' = '泰语'; 'Indonesian' = '印尼语';
    'Czech' = '捷克语'; 'Greek' = '希腊语'; 'Finnish' = '芬兰语'; 'Romanian' = '罗马尼亚语'
}
function Get-StatusCell($s) { $cn = if ($statusMap.ContainsKey($s)) { $statusMap[$s] } else { $s }; return "$cn（$s）" }

function Get-ModeCell($modes) {
    $o = $modes -contains 'offline'; $st = $modes -contains 'streaming'
    if ($o -and $st) { return '离线 + 流式' } elseif ($o) { return '离线' } elseif ($st) { return '流式' } else { return '—' }
}

function Get-LangCell($langs) {
    $langs = @($langs | Where-Object { $_ })
    if ($langs.Count -eq 0) { return 'auto / 多语' }
    $mapped = foreach ($l in $langs) {
        if ($langMap.ContainsKey($l)) { $langMap[$l] }
        elseif ($l -match '^(\d+)\+ languages$') { "$($Matches[1])+ 种语言" }
        elseif ($l -match '^(\d+) languages$') { "$($Matches[1]) 种语言" }
        else { $l }
    }
    $mapped = @($mapped)
    if ($mapped.Count -le 10) { return ($mapped -join ', ') }
    return (($mapped[0..7] -join ', ') + " …（共 $($mapped.Count) 种）")
}

function Get-TextCell($family, $s) {
    if ($zhMap.ContainsKey($family)) { return ($zhMap[$family] -replace '\|', '\|') }
    if (-not $s) { return '—' }
    $t = ($s -replace '\s+', ' ').Trim()
    if ($t.Length -gt 180) {
        $cut = $t.IndexOf('. ')
        if ($cut -ge 60 -and $cut -le 180) { $t = $t.Substring(0, $cut + 1) }
        else { $t = $t.Substring(0, 177) + '…' }
    }
    return ($t -replace '\|', '\|')
}

$catOrder = @('asr', 'tts', 'voice_conversion', 'speech_analysis', 'audio_tools', 'audio_generation', 'community')
$catTitle = @{
    asr              = '语音识别（ASR）'
    tts              = '语音合成（TTS）'
    voice_conversion = '语音转换 / 声音克隆（VC）'
    speech_analysis  = '语音分析（VAD · 说话人分离 · 对齐 · 分类 · 唤醒 · 轮次检测）'
    audio_tools      = '音频工具（降噪 / 修复 / 超分 / 源分离 / 转谱）'
    audio_generation = '音频与音乐生成'
    community        = '其他社区模型'
}
$catIntro = @{
    asr              = '把语音转成文本（部分模型同时给时间戳 / 说话人标签）。'
    tts              = '把文本合成为语音（含声音克隆 / 语音设计等变体）。'
    voice_conversion = '在保留内容的前提下改变音色，或参考说话人的音色转换。'
    speech_analysis  = '不做转录，而是分析音频结构（人声活动、说话人、对齐、事件分类等）。'
    audio_tools      = '通用音频处理：降噪、语音修复、超分、音源分离、录音转谱等。'
    audio_generation = '从文本或音频条件生成音乐 / 音效 / 更长的音频。'
    community        = '未归入上述类别的社区贡献模型。'
}

$sb = [System.Text.StringBuilder]::new()
$null = $sb.AppendLine('# 模型目录（Model Catalog）')
$null = $sb.AppendLine()
$null = $sb.AppendLine('本页按任务分类列出 **audio-cpp-rs**（经上游 [audio.cpp](https://github.com/0xShug0/audio.cpp)）支持的全部模型族。每条给出：')
$null = $sb.AppendLine()
$null = $sb.AppendLine('- **族**：上游 family 名，即加载 GGUF 时传给 `family_hint` / `Registry::load` 的字符串，也是 `ModelFamily` 枚举的规范名；')
$null = $sb.AppendLine('- **专用 feature**：本仓库 `Cargo.toml` 里可直接启用的 `model-<族>` Cargo feature（`—` 表示无专用 feature，改用 `custom-models` + `AUDIOCPP_MODELS=<族>`）；')
$null = $sb.AppendLine('- **任务**：运行时的**规范 token**，即传给 `Request` / `TaskKind` 的值。上游 `model_specs` 对个别任务用了不同拼写，本页已换算：`music`/`sfx`/`edit`/`audio_generation`→`gen`、`clone`→`clon`、`design`→`vdes`、`speaker`→`spk`、`wake_word`→`wake`、`classify`→`cls`；')
$null = $sb.AppendLine('- **模式 / 语言 / 状态**：取自上游 `model_specs/<族>.json`，模式中的“流式”指支持流式会话（`streaming`）；**简介**为该模型的中文一句话说明。')
$null = $sb.AppendLine()
$null = $sb.AppendLine('> ⚠️ 模型权重**不随本仓库提供**（内置 VAD 除外），下载方式见 [README 的「模型下载」](../README.md#模型下载)。')
$null = $sb.AppendLine('> GGUF 加载时通常须**显式传 `family_hint`**（引擎自动探测可能误判），用法见 [audio-cpp/README.md](../audio-cpp/README.md)。')
$null = $sb.AppendLine('> 上游“状态”为 audio.cpp 的成熟度标注：稳定（`supported`）/ 实验（`experimental`）/ 社区（`community`）/ 测试（`testing`）/ 开发中（`wip`），仅供参考。')
$null = $sb.AppendLine()
$null = $sb.AppendLine("共收录 **$($specs.Count)** 个上游模型族，另有 **2** 个内置 VAD 模型（随引擎编译、权重已 vendored，无需下载）。")
$null = $sb.AppendLine()

# 内置模型
$null = $sb.AppendLine('## 内置模型（开箱即用，无需下载权重）')
$null = $sb.AppendLine()
$null = $sb.AppendLine('| 族 | 模型 | 任务 | 模式 | 语言 | 状态 | 专用 feature | 简介 |')
$null = $sb.AppendLine('|---|---|---|---|---|---|---|---|')
$null = $sb.AppendLine('| `silero_vad` | Silero VAD | `vad` | 离线 + 流式 | auto / 多语 | 稳定（内置） | `core-models`（默认） | Silero 语音活动检测，16kHz 单声道，随引擎编译，权重 vendored。 |')
$null = $sb.AppendLine('| `marblenet_vad` | MarbleNet VAD | `vad` | 离线 | auto / 多语 | 稳定（内置） | `core-models`（默认） | NVIDIA NeMo MarbleNet VAD；NeMo checkpoint 自动探测易误判，须显式传 `family_hint="marblenet_vad"`。 |')
$null = $sb.AppendLine()

# TOC
$null = $sb.AppendLine('## 分类速览')
$null = $sb.AppendLine()
$null = $sb.AppendLine('| 分类 | 数量 | 跳转 |')
$null = $sb.AppendLine('|---|---|---|')
foreach ($c in $catOrder) {
    $n = ($specs | Where-Object { $_.cat -eq $c }).Count
    if ($n -eq 0) { continue }
    $null = $sb.AppendLine("| $($catTitle[$c]) | $n | [跳转](#cat-$c) |")
}
$null = $sb.AppendLine()

foreach ($c in $catOrder) {
    $rows = $specs | Where-Object { $_.cat -eq $c } | Sort-Object family
    if ($rows.Count -eq 0) { continue }
    $null = $sb.AppendLine("<a id=`"cat-$c`"></a>")
    $null = $sb.AppendLine()
    $null = $sb.AppendLine("## $($catTitle[$c])")
    $null = $sb.AppendLine()
    $null = $sb.AppendLine($catIntro[$c])
    $null = $sb.AppendLine()
    $null = $sb.AppendLine('| 族 | 模型 | 任务 | 模式 | 语言 | 状态 | 专用 feature | 简介 |')
    $null = $sb.AppendLine('|---|---|---|---|---|---|---|---|')
    foreach ($r in $rows) {
        $feature = Get-FeatureCell $r.family
        $display = if ($r.display) { $r.display } else { $r.family }
        $null = $sb.AppendLine("| ``$($r.family)`` | $display | $(Get-TaskCell $r.tasks) | $(Get-ModeCell $r.modes) | $(Get-LangCell $r.langs) | $(Get-StatusCell $r.status) | $feature | $(Get-TextCell $r.family $r.desc) |")
    }
    $null = $sb.AppendLine()
}

# 上游文档索引
$null = $sb.AppendLine('## 上游文档索引')
$null = $sb.AppendLine()
$null = $sb.AppendLine('下列模型族在上游 `docs/models/` 有专门的说明页（模型结构、转换脚本、权重来源与效果评测）：')
$null = $sb.AppendLine()
$withDoc = $specs | Where-Object { $docFiles.ContainsKey($_.family) } | Sort-Object family
foreach ($r in $withDoc) {
    $null = $sb.AppendLine("- [``$($r.family)``](https://github.com/0xShug0/audio.cpp/blob/main/docs/models/$($r.family).md) — $($r.display)")
}
$null = $sb.AppendLine()
$null = $sb.AppendLine('---')
$null = $sb.AppendLine()
$null = $sb.AppendLine('> 本页由 [`scripts/gen-models-doc.ps1`](../scripts/gen-models-doc.ps1) 依据上游 `model_specs/*.json`、`CMakeLists.txt` 与本仓库 `Cargo.toml` 生成')
$null = $sb.AppendLine('> （中文简介表见 [`scripts/model-desc-zh.tsv`](../scripts/model-desc-zh.tsv)）；随 submodule 升级重跑。族名 / feature 的当前状态以')
$null = $sb.AppendLine('> `audio-cpp/src/types.rs` 与各 crate 的 `Cargo.toml` 为准。')

[System.IO.File]::WriteAllText($outPath, $sb.ToString(), (New-Object System.Text.UTF8Encoding($false)))

$missing = $specs | Where-Object { -not $zhMap.ContainsKey($_.family) } | ForEach-Object { $_.family }
Write-Host "written: $outPath"
Write-Host "families: $($specs.Count), with upstream doc: $($withDoc.Count), missing zh desc: $($missing.Count)"
if ($missing) {
    Write-Warning ("以下族缺少中文简介（当前回落为英文，请补 scripts/model-desc-zh.tsv）:`n  " + ($missing -join ', '))
}
