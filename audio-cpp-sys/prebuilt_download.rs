//! 下载并缓存 audio.cpp 预编译静态库（`prebuilt` feature 专用）。
//!
//! 资产由 CI（.github/workflows/prebuilt-audio-cpp.yml）在打 tag 时生成并上传到
//! GitHub Releases，命名约定：
//! `audio-cpp-prebuilt-{linux|macos|windows}-{target}-{backend}-{modelset}-{static|dynamic}.tar.gz`
//! 其中：
//!   - `backend`：cpu / vulkan / metal（cuda / hip 暂不发布预编译）；
//!   - `modelset`：core / full / custom-<族1>-<族2>...（与 feature 组合对应）。
//!
//! 身份校验：归档内的 `metadata.json` 记录 `audio_commit`（打包时 audio.cpp
//! submodule 的 HEAD）。消费端比对本地 submodule HEAD，不符则视为不可用并回落
//! 源码构建（与 llama-cpp-rs 对 patches 的 fail-closed 同理）。
//!
//! 环境变量（均可在 build.rs 侧通过 rerun-if-env-changed 触发重新构建）：
//!   - `AUDIOCPP_PREBUILT_OFF`：非空真值（1/true/on）禁用自动下载，直接源码构建；
//!   - `AUDIOCPP_PREBUILT_DIR`：显式本地目录，优先级高于自动下载（见 build.rs）；
//!   - `AUDIOCPP_PREBUILT_TAG`：GitHub Release tag，默认 `v{version}`；
//!   - `AUDIOCPP_PREBUILT_REPO`：GitHub 仓库，默认 `launcher-rs/audio-cpp-rs`；
//!   - `AUDIOCPP_PREBUILT_URL`：下载地址模板。可含 `{tag}` / `{asset}` 占位符
//!     方便内网镜像/自定义存储；不含占位符视为完整地址。`file://` 前缀表示本地
//!     归档，直接复制不走网络。

use std::env;
use std::fs::{self, File};
use std::io::{copy, BufReader};
use std::path::{Path, PathBuf};

use serde_json::Value;

const DEFAULT_REPO: &str = "launcher-rs/audio-cpp-rs";

/// 解析当前构建配置对应的预编译归档文件名。
pub fn asset_name(target: &str, use_shared_libs: bool) -> Option<String> {
    let os = platform_os(target)?;
    let backend = backend_suffix()?;
    let modelset = modelset_suffix()?;
    let library_type = if use_shared_libs { "dynamic" } else { "static" };
    Some(format!(
        "audio-cpp-prebuilt-{os}-{target}-{backend}-{modelset}-{library_type}.tar.gz"
    ))
}

/// 确保预编译库可用；缺失时下载并解压。
///
/// 返回 `None` 表示应走源码构建：自动下载被禁用、平台/后端/模型组合无对应
/// 资产、下载失败，或归档身份（audio.cpp commit）与本地 submodule 不符。
pub fn ensure_prebuilt(target: &str, use_shared_libs: bool) -> Option<PathBuf> {
    if is_disabled() {
        return None;
    }

    if env::var("AUDIOCPP_PREBUILT_DIR").is_ok() {
        // 显式目录由 build.rs 的 resolve_prebuilt_directory() 处理。
        return None;
    }

    let asset = asset_name(target, use_shared_libs)?;
    let tag = release_tag();
    let cache_root = cache_root()?;
    let extract_dir = cache_root
        .join(tag.trim_start_matches('v'))
        .join(asset.strip_suffix(".tar.gz").unwrap_or(&asset));

    if is_valid_prebuilt_root(&extract_dir) {
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
            if !identity_matches(&extract_dir) {
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
            println!(
                "cargo:warning=预编译归档已解压但未找到库文件，回落到源码构建"
            );
            let _ = fs::remove_dir_all(&extract_dir);
            None
        }
        Err(err) => {
            println!(
                "cargo:warning=预编译库下载失败（{err}），回落到源码构建"
            );
            let _ = fs::remove_dir_all(&extract_dir);
            None
        }
    }
}

fn is_disabled() -> bool {
    matches!(
        env::var("AUDIOCPP_PREBUILT_OFF").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("on") | Ok("ON")
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
        return template
            .replace("{tag}", tag)
            .replace("{asset}", asset);
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
fn backend_suffix() -> Option<String> {
    if cfg!(feature = "cuda") || cfg!(feature = "hip") {
        // 尚无这些后端的预编译资产（仍需本地 SDK 链接，收益有限）。
        return None;
    }
    if cfg!(feature = "metal") {
        return Some("metal".to_string());
    }
    if cfg!(feature = "vulkan") {
        return Some("vulkan".to_string());
    }
    Some("cpu".to_string())
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
            for m in env_models.split(',').map(str::trim).filter(|s| !s.is_empty()) {
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
fn identity_matches(root: &Path) -> bool {
    let local_commit = local_audio_commit();
    let metadata_path = root.join("metadata.json");
    if local_commit.is_none() || !metadata_path.is_file() {
        // 无法取得任一侧 commit 时不做强校验（保守放行）。
        return true;
    }
    let Ok(text) = fs::read_to_string(&metadata_path) else {
        return true;
    };
    let Ok(json) = serde_json::from_str::<Value>(&text) else {
        return true;
    };
    match json.get("audio_commit").and_then(|v| v.as_str()) {
        Some(recorded) => {
            if recorded != local_commit.as_deref().unwrap_or("") {
                return false;
            }
        }
        None => {}
    }
    // MSVC 静态库与工具集版本强绑定：新版编译器编译的库无法被旧版链接器
    // 使用（STL/CRT 内部符号如 __std_unique_8 随版本新增）。归档记录了生产者
    // 的 _MSC_VER（如 1944），本地 MSVC 版本不足则不可用，回落到源码构建。
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
    // 版本行形如 "Optimizing Compiler Version 19.44.35222 ..."（Bv 输出的首个版本号）。
    let mut first = 0i64;
    let mut second = 0i64;
    let mut seen = 0usize;
    for tok in text.split(|c: char| !c.is_ascii_digit()) {
        if tok.is_empty() {
            continue;
        }
        if seen == 0 {
            first = tok.parse().unwrap_or(0);
        } else if seen == 1 {
            second = tok.parse().unwrap_or(0);
            break;
        }
        seen += 1;
    }
    // _MSC_VER = 19xx（版本 19.44 → 1944）。first 通常即 19。
    if first == 19 {
        Some(1900 + second)
    } else {
        None
    }
}

/// 读取本地 audio.cpp submodule 的 HEAD commit（短 8 位）。
fn local_audio_commit() -> Option<String> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").ok()?;
    let sub = Path::new(&manifest_dir).join("audio.cpp");
    if !sub.join("CMakeLists.txt").exists() {
        return None;
    }
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--short=8", "HEAD"])
        .current_dir(&sub)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8(output.stdout).ok()?;
    let s = s.trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
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
        // 本地归档：直接复制，绕开网络。
        let src = PathBuf::from(local);
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
                println!("cargo:warning=预编译下载第 {attempt}/{MAX_ATTEMPTS} 次失败：{err}");
                last_err = err;
                std::thread::sleep(std::time::Duration::from_secs(attempt as u64));
            }
        }
    }
    Err(last_err)
}

fn try_download(url: &str, partial: &Path) -> Result<(), String> {
    let response = ureq::get(url)
        .call()
        .map_err(|e| format!("HTTP GET {url}: {e}"))?;
    if !(200..300).contains(&response.status()) {
        return Err(format!("HTTP {} for {url}", response.status()));
    }
    let mut reader = response.into_reader();
    let mut file = File::create(partial).map_err(|e| e.to_string())?;
    copy(&mut reader, &mut file).map_err(|e| e.to_string())?;
    Ok(())
}
