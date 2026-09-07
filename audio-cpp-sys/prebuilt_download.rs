//! 下载并缓存 audio.cpp 预编译静态库（`prebuilt` feature 专用）。
//!
//! 资产由 CI（.github/workflows/prebuilt-audio-cpp.yml）在打 tag 时生成并上传到
//! GitHub Releases，命名约定：
//! `audio-cpp-prebuilt-{os}-{target}-{backend}[-{crt}]-{modelset}-static-{commit}.tar.gz`
//! 其中：
//!   - `backend`：cpu / vulkan / metal（cuda / hip 暂不发布预编译）；
//!     macOS 默认构建即 Metal（与 build.rs 的 `metal_on` 同口径），故 mac 只有 metal 资产；
//!   - `commit`：本地 audio.cpp submodule 完整 SHA 的前 12 位（精确寻址，见 `local_audio_commit`）；
//!   - `crt`：**仅 Windows**。`md`（动态 CRT，默认）/ `mt`（静态 CRT，
//!     crt-static）。其他平台无此段；
//!   - `modelset`：core / full / custom-<族1>-<族2>...（与 feature 组合对应）。
//!   - 实际只发布 static 归档（`use_shared_libs` 的 dynamic 分支保留接口，未使用）。
//!
//! 身份校验：归档内的 `metadata.json` 记录 `audio_commit`（打包时 audio.cpp
//! submodule 的 HEAD）。消费端比对本地 submodule HEAD，不符则视为不可用并回落
//! 源码构建（与 llama-cpp-rs 对 patches 的 fail-closed 同理）。
//!
//! 环境变量（均可在 build.rs 侧通过 rerun-if-env-changed 触发重新构建）：
//!   - `AUDIOCPP_PREBUILT_OFF`：真值（1/true/yes/on，大小写不敏感）禁用自动下载，直接源码构建；
//!   - `AUDIOCPP_PREBUILT_DIR`：显式本地目录，优先级高于自动下载（见 build.rs）；
//!   - `AUDIOCPP_PREBUILT_TAG`：GitHub Release tag，默认 `v{version}`；
//!   - `AUDIOCPP_PREBUILT_REPO`：GitHub 仓库，默认 `launcher-rs/audio-cpp-rs`；
//!   - `AUDIOCPP_PREBUILT_URL`：下载地址模板。可含 `{tag}` / `{asset}` 占位符
//!     方便内网镜像/自定义存储；不含占位符视为完整地址。`file://` 前缀表示本地
//!     归档，直接复制不走网络。

use std::env;
use std::fs::{self, File};
use std::io::{BufReader, copy};
use std::path::{Path, PathBuf};

use serde_json::Value;

const DEFAULT_REPO: &str = "launcher-rs/audio-cpp-rs";

/// 解析当前构建配置对应的预编译归档文件名。
///
/// `commit` 为本地 audio.cpp submodule 的完整 SHA 前 12 位（见 `local_audio_commit`）。
/// 把 commit 编入文件名后，消费端只需按自身 commit 精确请求资产：命中才下载、
/// 404 即立即回落源码构建，避免在“commit 不符”时白下整包（旧逻辑是下载解压后才
/// 由 `metadata.json` 校验）。
pub fn asset_name(target: &str, use_shared_libs: bool, commit: &str) -> Option<String> {
    let os = platform_os(target)?;
    let backend = backend_suffix(target)?;
    let modelset = modelset_suffix()?;
    let library_type = if use_shared_libs { "dynamic" } else { "static" };
    let crt = crt_suffix(target);
    Some(asset_name_of(
        &os,
        target,
        &backend,
        &modelset,
        &library_type,
        crt.as_deref(),
        commit,
    ))
}

/// 按各段拼资产名（crt 仅 Windows 存在，其余平台为 None；commit 为完整 SHA 的前 12 位）。
fn asset_name_of(
    os: &str,
    target: &str,
    backend: &str,
    modelset: &str,
    library_type: &str,
    crt: Option<&str>,
    commit: &str,
) -> String {
    match crt {
        Some(crt) => format!(
            "audio-cpp-prebuilt-{os}-{target}-{backend}-{crt}-{modelset}-{library_type}-{commit}.tar.gz"
        ),
        None => {
            format!(
                "audio-cpp-prebuilt-{os}-{target}-{backend}-{modelset}-{library_type}-{commit}.tar.gz"
            )
        }
    }
}

/// 尝试按指定 modelset 后缀获取预编译库。
///
/// 返回 `None` 表示该资产不可用（下载失败 / 身份校验未过）或应走源码构建。
/// 若 modelset 为 `None`，则使用当前 feature 推导的默认 modelset 后缀。
///
/// `commit` 为本地 audio.cpp submodule 的完整 SHA 前 12 位，已编入资产名；仅当文件名
/// 带该 commit 的资产存在时才下载，因此“commit 不符”必然 404 而立即回落，不会白下整包。
fn fetch_prebuilt(
    target: &str,
    use_shared_libs: bool,
    modelset_override: Option<&str>,
    commit: &str,
) -> Option<PathBuf> {
    if is_disabled() {
        return None;
    }

    if env::var("AUDIOCPP_PREBUILT_DIR").is_ok() {
        // 显式目录由 build.rs 的 resolve_prebuilt_directory() 处理。
        return None;
    }

    let modelset = modelset_override
        .map(|s| s.to_string())
        .or_else(modelset_suffix)?;
    let os = platform_os(target)?;
    let backend = backend_suffix(target)?;
    let library_type = if use_shared_libs { "dynamic" } else { "static" };
    let asset = asset_name_of(
        &os,
        target,
        &backend,
        &modelset,
        &library_type,
        crt_suffix(target).as_deref(),
        commit,
    );
    let tag = release_tag();
    let cache_root = cache_root().or_else(|| {
        // 非标准 target 目录布局时无法定位缓存：明示原因而非静默回落。
        println!(
            "cargo:warning=无法定位预编译缓存目录（OUT_DIR/PROFILE 布局非标准），跳过预编译下载"
        );
        None
    })?;
    let extract_dir = cache_root
        .join(tag.trim_start_matches('v'))
        .join(asset.strip_suffix(".tar.gz").unwrap_or(&asset));

    if is_valid_prebuilt_root(&extract_dir) && identity_matches(&extract_dir, target) {
        println!(
            "cargo:warning=使用缓存中的 audio.cpp 预编译库：{}",
            extract_dir.display()
        );
        return Some(extract_dir);
    }

    let url = download_url(&tag, &asset);
    println!("cargo:warning=下载 audio.cpp 预编译库：{url}");

    match download_and_extract(&url, &extract_dir) {
        Ok(()) if is_valid_prebuilt_root(&extract_dir) => {
            // 身份校验：归档内的 audio.cpp commit 须与本地 submodule 一致，
            // 否则 ABI 可能与当前 bindgen 生成的绑定不符，回落到源码构建。
            if !identity_matches(&extract_dir, target) {
                println!(
                    "cargo:warning=预编译归档的 audio.cpp commit 与本地 submodule 不符，回落到源码构建"
                );
                let _ = fs::remove_dir_all(&extract_dir);
                return None;
            }
            println!(
                "cargo:warning=audio.cpp 预编译库就绪：{}",
                extract_dir.display()
            );
            Some(extract_dir)
        }
        Ok(()) => {
            println!("cargo:warning=预编译归档已解压但未找到库文件，回落到源码构建");
            let _ = fs::remove_dir_all(&extract_dir);
            None
        }
        Err(err) => {
            println!("cargo:warning=预编译库下载失败（{err}），该资产不可用");
            let _ = fs::remove_dir_all(&extract_dir);
            None
        }
    }
}

/// 确保预编译库可用；缺失时下载并解压。
///
/// 返回 `None` 表示应走源码构建：自动下载被禁用、平台/后端/模型组合无对应
/// 资产、下载失败，或归档身份（audio.cpp commit）与本地 submodule 不符。
///
/// 只发布 full 全模型资产（full 是任何 model 组合的超集）。因此先尝试精确
/// 匹配当前 feature 的资产（core / custom-<族>），404 时回退到 full 资产；
/// 两者都失败才回落到源码构建。full 资产下载体积较大，但保证任何 feature
/// 组合都能用上预编译加速。
pub fn ensure_prebuilt(target: &str, use_shared_libs: bool) -> Option<PathBuf> {
    // 本地 audio.cpp 的完整 commit：资产名按它精确寻址。无法取得（submodule 未初始化 /
    // 工作树漂移 / 有未提交改动）时跳过下载，直接源码构建，避免误用不匹配的预编译。
    let commit = match local_audio_commit() {
        Some(c) => c,
        None => {
            println!(
                "cargo:warning=无法确定本地 audio.cpp commit（未初始化/漂移/含未提交改动），\
                 跳过预编译下载，回落到源码构建"
            );
            return None;
        }
    };
    let exact = modelset_suffix();
    // 精确资产（core / custom-*）与 full 回退资产；两者都按同一 commit 寻址。
    match exact.as_deref() {
        Some("full") => fetch_prebuilt(target, use_shared_libs, Some("full"), &commit),
        Some(mset) => fetch_prebuilt(target, use_shared_libs, Some(mset), &commit).or_else(|| {
            println!("cargo:warning=custom/core 资产不可用，回退到 full 全模型资产");
            fetch_prebuilt(target, use_shared_libs, Some("full"), &commit)
        }),
        None => fetch_prebuilt(target, use_shared_libs, None, &commit),
    }
}

fn is_disabled() -> bool {
    matches!(
        env::var("AUDIOCPP_PREBUILT_OFF")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn release_tag() -> String {
    env::var("AUDIOCPP_PREBUILT_TAG").unwrap_or_else(|_| {
        format!(
            "v{}",
            env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.1.0".into())
        )
    })
}

fn github_repo() -> String {
    env::var("AUDIOCPP_PREBUILT_REPO").unwrap_or_else(|_| DEFAULT_REPO.to_string())
}

/// 计算资产下载地址。
///
/// `AUDIOCPP_PREBUILT_URL` 可用作模板，含 `{tag}` / `{asset}` 占位符（内网镜像、
/// 自定义存储等场景）；不含占位符时视为完整地址直接使用。未设置时走默认的
/// GitHub Releases。`file://` 前缀表示本地归档，交给下载层按文件复制处理。
fn download_url(tag: &str, asset: &str) -> String {
    if let Ok(template) = env::var("AUDIOCPP_PREBUILT_URL") {
        if !template.contains("{tag}") && !template.contains("{asset}") {
            // 无占位符的完整地址：tag/asset 全部被忽略，多配置会共用同一
            // URL 与同一缓存键，易互相误导，明确提示。
            println!(
                "cargo:warning=AUDIOCPP_PREBUILT_URL 不含 {{tag}}/{{asset}} 占位符，\
                 tag={tag} asset={asset} 均被忽略，共用固定地址"
            );
        }
        return template.replace("{tag}", tag).replace("{asset}", asset);
    }
    format!(
        "https://github.com/{}/releases/download/{}/{}",
        github_repo(),
        tag,
        asset
    )
}

fn cache_root() -> Option<PathBuf> {
    let out_dir = env::var("OUT_DIR").ok()?;
    let profile = env::var("PROFILE").ok()?;
    let mut target_dir = None;
    let mut sub_path = Path::new(&out_dir);
    while let Some(parent) = sub_path.parent() {
        if parent.ends_with(&profile) {
            target_dir = Some(parent);
            break;
        }
        sub_path = parent;
    }
    Some(target_dir?.join("audio-cpp-prebuilt-cache"))
}

fn platform_os(target: &str) -> Option<&'static str> {
    if target.contains("linux") {
        Some("linux")
    } else if target.contains("windows") {
        Some("windows")
    } else if target.contains("apple") {
        Some("macos")
    } else {
        None
    }
}

/// 把启用的计算后端 feature 映射为 CI 资产名里的 backend 后缀。
///
/// 口径与 build.rs 的 `metal_on` 一致：显式 `metal` feature，或 Apple
/// target（macOS 默认即 Metal 构建，CI 也只发布 macos-metal 资产）。
fn backend_suffix(target: &str) -> Option<String> {
    if cfg!(feature = "cuda") || cfg!(feature = "hip") {
        // 尚无这些后端的预编译资产（仍需本地 SDK 链接，收益有限）。
        return None;
    }
    if cfg!(feature = "metal") || target.contains("apple") {
        return Some("metal".to_string());
    }
    if cfg!(feature = "vulkan") {
        return Some("vulkan".to_string());
    }
    Some("cpu".to_string())
}

/// MSVC 目标的 CRT 变体后缀：`mt`（静态 CRT）/ `md`（动态 CRT，默认）。
///
/// 预编译资产区分 CRT 运行时库：`crt-static`（`-C target-feature=+crt-static`）
/// 开启时 Rust/cc 侧用 `/MT`，须下载 `-mt` 资产；否则 `/MD` 用 `-md` 资产。
/// 仅 Windows 有此维度，其余平台返回 `None`（资产名不含 crt 段）。
/// 注意按 target（而非宿主 OS）判断，否则交叉编译时资产名对不上。
fn crt_suffix(target: &str) -> Option<String> {
    if !target.contains("windows") {
        return None;
    }
    let static_crt = env::var("CARGO_CFG_TARGET_FEATURE")
        .map(|f| f.split(',').any(|s| s.trim() == "crt-static"))
        .unwrap_or(false);
    Some(if static_crt {
        "mt".to_string()
    } else {
        "md".to_string()
    })
}

/// 把启用的模型组合 feature 映射为 CI 资产名里的 modelset 后缀。
fn modelset_suffix() -> Option<String> {
    if cfg!(feature = "full-models") {
        return Some("full".to_string());
    }
    if cfg!(feature = "custom-models") {
        // 收集 model-<族> feature 与 AUDIOCPP_MODELS 环境变量指定的族名
        // （与 build.rs 的 merge_custom_models 一致），拼成 custom-<族1>-<族2>...
        let mut families: Vec<String> = Vec::new();
        for (key, _) in env::vars() {
            if let Some(suffix) = key.strip_prefix("CARGO_FEATURE_MODEL_") {
                families.push(suffix.to_lowercase());
            }
        }
        if let Ok(env_models) = env::var("AUDIOCPP_MODELS") {
            for m in env_models
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                if !families.iter().any(|s| s == m) {
                    families.push(m.to_string());
                }
            }
        }
        families.sort();
        if families.is_empty() {
            // custom-models 未指定任何族：无匹配资产，回落源码构建。
            return None;
        }
        return Some(format!("custom-{}", families.join("-")));
    }
    Some("core".to_string())
}

/// 校验归档内的 `metadata.json.audio_commit` 与本地 submodule HEAD 一致。
///
/// fail-closed：任一侧 commit 取不到、无 metadata、读失败或解析失败都返回
/// false（回落源码构建），损坏的缓存不会被信任链接。另校验 `msvc_ver`。
fn identity_matches(root: &Path, target: &str) -> bool {
    let local_commit = local_audio_commit();
    let metadata_path = root.join("metadata.json");
    if local_commit.is_none() || !metadata_path.is_file() {
        // 任一侧 commit 缺失：无法证明等价，不放行。
        return false;
    }
    let Ok(text) = fs::read_to_string(&metadata_path) else {
        return false;
    };
    let Ok(json) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    match json.get("audio_commit").and_then(|v| v.as_str()) {
        Some(recorded) => {
            if recorded != local_commit.as_deref().unwrap_or("") {
                return false;
            }
        }
        // 无 audio_commit 记录：无法证明等价，不放行。
        None => return false,
    }
    // MSVC 静态库与工具集版本强绑定：新版编译器编译的库无法被旧版链接器
    // 使用（STL/CRT 内部符号如 __std_unique_8 随版本新增）。归档记录了生产者
    // 的 _MSC_VER（如 1944），本地 MSVC 版本不足则不可用，回落到源码构建。
    // 非 Windows target 直接跳过本项。
    if !target.contains("windows") {
        return true;
    }
    if let Some(recorded_msvc) = json.get("msvc_ver").and_then(|v| v.as_i64()) {
        if recorded_msvc > 0 {
            if let Some(local_msvc) = local_msvc_ver() {
                if local_msvc < recorded_msvc {
                    println!(
                        "cargo:warning=预编译归档由 MSVC {} 构建，本地为 MSVC {}（偏低），\
                         可能导致链接失败，回落到源码构建",
                        recorded_msvc, local_msvc
                    );
                    return false;
                }
            }
        }
    }
    true
}

/// 探测本地 MSVC 编译器版本（`_MSC_VER`，如 14.44 → 1944）。
fn local_msvc_ver() -> Option<i64> {
    if std::env::consts::OS != "windows" {
        return None;
    }
    let cc = cc::Build::new();
    let compiler = cc.try_get_compiler().ok()?;
    if !compiler.is_like_msvc() {
        return None;
    }
    let cl = compiler.path();
    // cl /Bv 只打印版本 banner，无需 INCLUDE 环境即可运行。
    let banner = std::process::Command::new(cl).arg("/Bv").output().ok()?;
    let text = String::from_utf8_lossy(&banner.stdout);
    let text = format!("{}{}", text, String::from_utf8_lossy(&banner.stderr));
    // banner 形如 "... Compiler Version 19.44.35222 ..."（可能含 "x64" 等干扰数字，
    // 因此锚定 19.xx 模式：第一个形如 19.<40..99> 的二元组即版本）。
    let nums: Vec<i64> = text
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|tok| tok.parse().ok())
        .collect();
    for w in nums.windows(2) {
        if w[0] == 19 && (40..=99).contains(&w[1]) {
            // _MSC_VER = 19xx（版本 19.44 → 1944）。
            return Some(1900 + w[1]);
        }
    }
    None
}

/// 运行 git 并取 stdout（trim）。失败返回 None。
fn git_capture(args: &[&str], dir: &Path) -> Option<String> {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    Some(s.trim().to_string())
}

/// 读取本地 audio.cpp submodule 的“权威 commit”，用于预编译资产精确寻址。
///
/// 取父仓库记录的 submodule gitlink（40 位完整 SHA，取前 12 位作寻址键，正是 CI 构建时的基准），并要求：
/// 1. 子模块工作树实际 HEAD 与 gitlink 一致；
/// 2. 子模块工作树干净（无未提交改动）。
/// 二者任一不满足则返回 None，迫使消费端回落源码构建——这样资产名里的 commit 才真正
/// 等价于本地即将编译的 audio.cpp 内容，不会误用 ABI 不匹配的预编译。
fn local_audio_commit() -> Option<String> {
    let manifest_dir_str = env::var("CARGO_MANIFEST_DIR").ok()?;
    let manifest_dir = Path::new(&manifest_dir_str);
    let sub = manifest_dir.join("audio.cpp");
    if !sub.join("CMakeLists.txt").exists() {
        return None;
    }
    // 父仓库记录的 submodule gitlink（第 3 字段为 40 位完整 SHA，取前 12 位作寻址键）。
    // CARGO_MANIFEST_DIR 即
    // audio-cpp-sys，submodule 相对此目录的路径为 audio.cpp。
    let gitlink_line = git_capture(&["ls-tree", "HEAD", "audio.cpp"], manifest_dir)?;
    let gitlink = gitlink_line.split_whitespace().nth(2)?.to_string();
    if gitlink.len() != 40 {
        return None;
    }
    // 工作树实际 HEAD 须等于 gitlink，否则开发者手动 checkout / 未 submodule update。
    let wt = git_capture(&["-C", sub.to_str()?, "rev-parse", "HEAD"], manifest_dir)?;
    if wt != gitlink {
        return None;
    }
    // 工作树必须干净：有未提交改动则内容未被任何预编译覆盖，强制源码构建。
    let dirty = git_capture(
        &["-C", sub.to_str()?, "status", "--porcelain"],
        manifest_dir,
    );
    if dirty.map(|s| !s.is_empty()).unwrap_or(true) {
        return None;
    }
    // 取完整 SHA 的前 12 位作为资产寻址键：碰撞概率可忽略（远超本仓库 commit 量级），
    // 同时显著缩短文件名，规避 Windows 路径长度问题。CI 侧用同样方式截取，保证一致。
    Some(gitlink.get(..12).unwrap_or(&gitlink).to_string())
}

fn is_valid_prebuilt_root(root: &Path) -> bool {
    if !root.is_dir() {
        return false;
    }
    for dir in [
        root.to_path_buf(),
        root.join("lib"),
        root.join("lib64"),
        root.join("bin"),
    ] {
        if !dir.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_file() && !file_type.is_symlink() {
                continue;
            }
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if is_audio_lib_name(name) {
                return true;
            }
        }
    }
    false
}

fn is_audio_lib_name(name: &str) -> bool {
    let base = name
        .strip_prefix("lib")
        .unwrap_or(name)
        .split('.')
        .next()
        .unwrap_or(name);
    // 归档里可能是 sentencepiece-static.lib 形态，去掉 -static 后缀再比对。
    let base = base.strip_suffix("-static").unwrap_or(base);
    matches!(
        base,
        "engine_runtime"
            | "ggml"
            | "ggml-base"
            | "ggml-cpu"
            | "sentencepiece"
            | "cjson_vendor"
            | "yaml_vendor"
    )
}

fn download_and_extract(url: &str, extract_dir: &Path) -> Result<(), String> {
    if extract_dir.exists() {
        fs::remove_dir_all(extract_dir).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(extract_dir).map_err(|e| e.to_string())?;

    let archive_path = extract_dir.with_extension("tar.gz");
    download_file(url, &archive_path)?;

    let file = File::open(&archive_path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    let decoder = flate2::read::GzDecoder::new(reader);
    let mut archive = tar::Archive::new(decoder);
    archive.unpack(extract_dir).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(&archive_path);
    Ok(())
}

fn download_file(url: &str, dest: &Path) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if let Some(local) = url.strip_prefix("file://") {
        // 本地归档：直接复制，绕开网络。Windows 下 file:///C:/... 会剩前导
        // `/`，遇到盘符路径（X:/...）时剥掉它。
        let src = match local.strip_prefix('/') {
            Some(stripped) => {
                let mut c = stripped.chars();
                if matches!(c.next(), Some(d) if d.is_ascii_alphabetic()) && c.next() == Some(':') {
                    PathBuf::from(stripped)
                } else {
                    PathBuf::from(local)
                }
            }
            None => PathBuf::from(local),
        };
        if !src.is_file() {
            return Err(format!("本地归档不存在：{}", src.display()));
        }
        fs::copy(&src, dest).map_err(|e| e.to_string())?;
        return Ok(());
    }
    let partial = dest.with_extension("partial");
    download_with_retry(url, &partial)?;
    fs::rename(&partial, dest).map_err(|e| e.to_string())?;
    Ok(())
}

/// 下载到临时文件；网络抖动时重试若干次，全部失败返回最后一次错误。
fn download_with_retry(url: &str, partial: &Path) -> Result<(), String> {
    const MAX_ATTEMPTS: usize = 3;
    let mut last_err = format!("下载 {url} 失败");
    for attempt in 1..=MAX_ATTEMPTS {
        match try_download(url, partial) {
            Ok(()) => return Ok(()),
            Err(err) => {
                if is_non_retryable(&err) {
                    // 4xx（资源不存在 / 权限问题等）为确定性失败，重试无意义，立即返回。
                    println!("cargo:warning=预编译下载失败（{err}）");
                    return Err(err);
                }
                println!("cargo:warning=预编译下载第 {attempt}/{MAX_ATTEMPTS} 次失败：{err}");
                last_err = err;
                std::thread::sleep(std::time::Duration::from_secs(attempt as u64));
            }
        }
    }
    Err(last_err)
}

/// 判断下载错误是否不可重试：HTTP 4xx（除 429 限流外）为确定性失败。
///
/// 判定依据是 `try_download` 打上的 `[non-retryable]` 标记（直接取自
/// ureq 的状态码，不依赖错误文本格式）；旧格式的 "status code NNN" 串
/// 仍兼容解析。
fn is_non_retryable(err: &str) -> bool {
    if err.contains("[non-retryable]") {
        return true;
    }
    // 错误串形如 "HTTP GET {url}: https://...: status code 404"。
    if let Some(pos) = err.rfind("status code ") {
        if let Some(code) = err[pos + "status code ".len()..].split_whitespace().next() {
            if let Ok(n) = code.parse::<u16>() {
                return (400..500).contains(&n) && n != 429;
            }
        }
    }
    false
}

fn try_download(url: &str, partial: &Path) -> Result<(), String> {
    // 超时：网络挂起时不能无限卡住构建（连接/读取整体 60 秒）。
    let result = ureq::get(url)
        .timeout(std::time::Duration::from_secs(60))
        .call();
    let response = match result {
        Ok(resp) => resp,
        // ureq 2.x 对 HTTP 错误状态直接返回 Err(Status)：4xx（除限流）
        // 是确定性失败，打标记免重试；其他错误走重试。
        Err(ureq::Error::Status(code, _)) if (400..500).contains(&code) && code != 429 => {
            return Err(format!("HTTP {code} for {url} [non-retryable]"));
        }
        Err(e) => return Err(format!("HTTP GET {url}: {e}")),
    };
    if !(200..300).contains(&response.status()) {
        return Err(format!("HTTP {} for {url}", response.status()));
    }
    let mut reader = response.into_reader();
    let mut file = File::create(partial).map_err(|e| e.to_string())?;
    copy(&mut reader, &mut file).map_err(|e| e.to_string())?;
    Ok(())
}
