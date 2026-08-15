// 构建脚本中 expect/unwrap 是惯用法：环境变量缺失、git/编译命令失败等
// 都应立即 panic 中止构建并给出明确信息，展开错误链无实际收益。
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::collections::hash_map::DefaultHasher;
use std::env;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use cmake::Config;
use glob::glob;

/// `prebuilt` feature 启用时提供自动下载预编译库的模块。
#[cfg(feature = "prebuilt")]
mod prebuilt_download;

/// 通过 BUILD_DEBUG 环境变量控制构建脚本调试日志输出。
macro_rules! debug_log {
    ($($arg:tt)*) => {
        if std::env::var("BUILD_DEBUG").is_ok() {
            println!("cargo:warning=[DEBUG] {}", format!($($arg)*));
        }
    };
}

/// audio.cpp 上游源码树位于 `$CARGO_MANIFEST_DIR/audio.cpp`。
///
/// 该目录通常是 git submodule（`.gitmodules` 指向
/// `https://github.com/0xShug0/audio.cpp.git`），内容不提交进本仓库。
/// 克隆本项目后需先 `git submodule update --init --recursive`。
fn audio_src_dir() -> PathBuf {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR 未设置");
    Path::new(&manifest_dir).join("audio.cpp")
}

/// 上游源码 URL（crates.io 打包场景没有 .git 上下文，无法走 submodule，
/// 需要用本 URL 直接 clone）。
const AUDIO_CPP_URL: &str = "https://github.com/0xShug0/audio.cpp.git";

/// 确保 `audio.cpp` 源码树存在；缺失时自动获取。
///
/// 返回包含源码树的路径。获取策略：
/// 1. 源码已存在（`$CARGO_MANIFEST_DIR/audio.cpp`）→ 直接返回；
/// 2. 处于 git 仓库内（含 `.gitmodules`）→ `git submodule update --init`;
/// 3. 否则（如 crates.io 打包验证场景，manifest 目录只读）→ `git clone --depth 1`
///    到 `OUT_DIR/audio.cpp`（build.rs 只允许写 OUT_DIR）。
fn ensure_audio_src() -> PathBuf {
    let manifest_src = audio_src_dir();
    if manifest_src.join("CMakeLists.txt").exists() {
        return manifest_src;
    }

    // 从 manifest 目录向上找 .git，判断是否处于 git 仓库。
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| manifest_src.clone());
    let mut dir = manifest_dir.clone();
    let mut in_git_repo = false;
    loop {
        if dir.join(".git").exists() {
            in_git_repo = true;
            break;
        }
        if !dir.pop() {
            break;
        }
    }

    if in_git_repo {
        debug_log!("audio.cpp 缺失，执行 git submodule update --init ...");
        let status = std::process::Command::new("git")
            .args(["submodule", "update", "--init", "--recursive"])
            .current_dir(&manifest_dir)
            .status()
            .expect("failed to run git submodule update");
        if status.success() && manifest_src.join("CMakeLists.txt").exists() {
            return manifest_src;
        }
        debug_log!("submodule 更新失败，回退到 git clone");
    }

    // 不在 git 仓库（或 submodule 不可用）：clone 到 OUT_DIR。
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR 未设置"));
    let src_dir = out_dir.join("audio.cpp");
    if !src_dir.join("CMakeLists.txt").exists() {
        debug_log!("audio.cpp 缺失，执行 git clone --depth 1 {AUDIO_CPP_URL} 到 OUT_DIR ...");
        let status = std::process::Command::new("git")
            .args(["clone", "--depth", "1", AUDIO_CPP_URL])
            .arg(&src_dir)
            .status()
            .expect("failed to run git clone");
        assert!(
            status.success() && src_dir.join("CMakeLists.txt").exists(),
            "无法自动获取 audio.cpp 源码。请确认网络可用，或手动把源码放到 {}",
            manifest_src.display()
        );
    }
    src_dir
}

/// 从 Rust 目标三元组推断出粗粒度的操作系统类别，用于决定静态库的
/// 文件后缀（Windows 为 .lib，其余为 .a）与链接提示。
fn target_os() -> String {
    let target = env::var("TARGET").unwrap_or_default();
    if target.contains("windows") {
        "windows".to_string()
    } else if target.contains("apple") {
        "apple".to_string()
    } else if target.contains("android") {
        "android".to_string()
    } else if target.contains("linux") {
        "linux".to_string()
    } else {
        target
    }
}

/// 是否启用了 Rust 目标的 `crt-static`（静态 CRT 链接）。
///
/// 通过 cargo 注入的 `CARGO_CFG_TARGET_FEATURE` 环境变量检测（如
/// `-C target-feature=+crt-static` 会令其含 `crt-static`）。MSVC 下开启后：
/// - cc crate 编译的 C shim（capi.o）会自动加 `/MT`（静态 CRT）；
/// - Rust 自身 std 也按静态 CRT 链接；
/// - 但 CMake 构建的 engine_runtime 默认 `/MD`（动态 CRT），预编译资产同样
///   是 `/MD`，混链接会报 LNK2038（RuntimeLibrary 不匹配）与 LNK2019
///   （`__imp_*` 符号无法解析）。
///   因此 build.rs 需据此强制 CMake 全目标 `/MT`，且跳过 `/MD` 的预编译资产。
fn crt_static_enabled() -> bool {
    env::var("CARGO_CFG_TARGET_FEATURE")
        .map(|f| f.split(',').any(|s| s.trim() == "crt-static"))
        .unwrap_or(false)
}

/// 收集 audio.cpp CMake 构建产出的静态库文件名（去掉前缀/后缀）。
///
/// audio.cpp 产出：`engine_runtime`、`ggml`、`ggml-cpu`、`ggml-base`、
/// `sentencepiece-static`（别名 `sentencepiece`）、`cjson_vendor`、
/// `yaml_vendor`，以及可选的各后端库。cmake crate 把归档库放在
/// `OUT_DIR/build`（及其子目录），因此对给定的搜索目录做递归 glob，
/// 并按 stem 去重。链接顺序由各库的相互依赖决定（engine_runtime 依赖
/// ggml 系列与 sentencepiece 等），这里按发现顺序输出即可，Rust 链接器
/// 会按需求遍历。
fn extract_static_lib_names(search_dirs: &[PathBuf], os: &str) -> Vec<String> {
    let ext = match os {
        "windows" => "*.lib",
        _ => "*.a",
    };
    let mut names: Vec<String> = Vec::new();
    for dir in search_dirs {
        let pattern = dir.join("**").join(ext).to_string_lossy().into_owned();
        for entry in glob(&pattern).expect("构建 lib glob 失败") {
            let Ok(path) = entry else { continue };
            // 跳过 CMake 内部产物：CUDA 构建会在 build_dir/CMakeFiles/.../
            // CompilerIdCUDA/ 生成编译器探测归档 `a.lib`，其 stem 为 `a`，
            // 若收集会产出 `cargo:rustc-link-lib=static=a`，链接时报
            // "could not find native static library `a`"。
            if path.components().any(|c| c.as_os_str() == "CMakeFiles") {
                continue;
            }
            let Some(stem) = path.file_stem() else {
                continue;
            };
            let mut name = stem.to_string_lossy().into_owned();
            if !name.starts_with("lib") && path.extension().map(|e| e == "a").unwrap_or(false) {
                // MinGW 下无 lib 前缀的归档由构建过程处理，这里保留原始 stem。
            }
            if name.starts_with("lib") {
                name = name.strip_prefix("lib").unwrap_or(&name).to_string();
            }
            if name.ends_with("-static") {
                name = name.strip_suffix("-static").unwrap_or(&name).to_string();
            }
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names
}

/// 为发现的每个静态库输出 `cargo:rustc-link-lib=static=` 指令。
fn link_static_libs(names: &[String]) {
    for name in names {
        println!("cargo:rustc-link-lib=static={}", name);
    }
}

/// 解析 `AUDIOCPP_PREBUILT_DIR` 环境变量指向的预编译库目录。
///
/// 目录应包含 audio.cpp 的静态库归档（engine_runtime 及依赖库），可放在
/// `<dir>`、`<dir>/lib`、`<dir>/lib64` 或 `<dir>/bin`（与 llama-cpp-rs 的
/// `LLAMA_PREBUILT_DIR` 约定一致）。设置后跳过整个 CMake 构建，仅编译 C
/// shim 与生成绑定，显著加快下游应用的迭代/CI 速度。
///
/// 若未设置该变量，且启用了 `prebuilt` feature，则尝试从 GitHub Releases
/// 自动下载匹配当前平台/后端/模型组合的归档（见 `prebuilt_download` 模块）。
fn resolve_prebuilt_directory(use_shared_libs: bool) -> Option<PathBuf> {
    if let Ok(raw) = env::var("AUDIOCPP_PREBUILT_DIR")
        && !raw.is_empty()
    {
        let dir = PathBuf::from(&raw);
        if !dir.is_dir() {
            panic!("AUDIOCPP_PREBUILT_DIR 指向的目录不存在：{}", dir.display());
        }
        return Some(dir);
    }

    #[cfg(feature = "prebuilt")]
    {
        let target = env::var("TARGET").unwrap_or_default();
        prebuilt_download::ensure_prebuilt(&target, use_shared_libs)
    }

    #[cfg(not(feature = "prebuilt"))]
    {
        let _ = use_shared_libs;
        None
    }
}

/// 定位 CUDA Toolkit 的库目录（`lib/x64` 或 `lib64`）。
///
/// 搜索顺序与上游 `find_package(CUDAToolkit)` 一致：
///   1. `CUDA_PATH` 环境变量（NVIDIA 安装器写入的规范位置），
///      其次 `CUDA_PATH_V12_4` 这类版本化变量；
///   2. PATH 里的 `nvcc`，其父目录的父目录即 Toolkit 根；
///   3. 常见安装目录（Windows 的 "NVIDIA GPU Computing Toolkit" 与
///      Linux 的 /usr/local/cuda），取版本号最高的。
fn cuda_toolkit_lib_dir(os: &str) -> Option<PathBuf> {
    let env_root: Option<PathBuf> = env::var("CUDA_PATH")
        .ok()
        .or_else(|| {
            let mut versions: Vec<String> = env::vars()
                .filter_map(|(k, v)| k.strip_prefix("CUDA_PATH_V").map(|_| v))
                .collect();
            versions.sort();
            versions.pop()
        })
        .map(PathBuf::from);
    let from_nvcc = env::var("PATH").ok().and_then(|path| {
        let nvcc_name = if os == "windows" { "nvcc.exe" } else { "nvcc" };
        env::split_paths(&path)
            .map(|d| d.join(nvcc_name))
            .find(|p| p.is_file())
            .and_then(|p| {
                p.parent()
                    .and_then(|d| d.parent())
                    .map(std::path::Path::to_path_buf)
            })
    });
    let common = || -> Option<PathBuf> {
        let candidates: Vec<PathBuf> = match os {
            "windows" => glob("C:/Program Files/NVIDIA GPU Computing Toolkit/CUDA/v*")
                .ok()?
                .filter_map(std::result::Result::ok)
                .collect(),
            "linux" => vec![PathBuf::from("/usr/local/cuda")],
            _ => vec![],
        };
        candidates.into_iter().max()
    };
    let root = env_root.or(from_nvcc).or_else(common)?;
    let lib = if os == "windows" {
        root.join("lib").join("x64")
    } else {
        root.join("lib64")
    };
    lib.is_dir().then_some(lib)
}

/// CUDA feature 启用时为最终链接补上 CUDA 运行时库。
///
/// engine_runtime / ggml-cuda 以静态库参与链接，它们在 CMake 里 PRIVATE
/// 声明的 CUDA 依赖（cudart/cublas/cublasLt/cufft/驱动）不会传导到最终
/// 可执行文件，因此这里按上游链接清单显式输出。Toolkit 缺失时给出明确报错。
fn emit_cuda_links(os: &str) {
    if !cfg!(feature = "cuda") {
        return;
    }
    let lib_dir = cuda_toolkit_lib_dir(os).unwrap_or_else(|| {
        panic!(
            "cuda feature 已启用但未找到 CUDA Toolkit。请安装 CUDA Toolkit >= 12.0，\
             并确保 CUDA_PATH 环境变量可用（如 C:\\Program Files\\NVIDIA GPU \
             Computing Toolkit\\CUDA\\v12.4）"
        )
    });
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    for lib in ["cudart", "cublas", "cublasLt", "cufft", "cuda"] {
        println!("cargo:rustc-link-lib={}", lib);
    }
    debug_log!("CUDA 链接库目录: {}", lib_dir.display());
}

/// 定位 Vulkan SDK 的库目录（Windows 为 `Lib`）。
///
/// 搜索顺序与上游 `find_package(Vulkan)` 一致：
///   1. `VULKAN_SDK` 环境变量（LunarG 安装器写入的规范位置）；
///   2. Windows 常见安装目录 `C:/VulkanSDK/v*`，取版本号最高的。
fn vulkan_sdk_lib_dir(os: &str) -> Option<PathBuf> {
    let from_env = env::var("VULKAN_SDK").ok().map(PathBuf::from);
    let common = || -> Option<PathBuf> {
        let candidates: Vec<PathBuf> = match os {
            "windows" => glob("C:/VulkanSDK/v*")
                .ok()?
                .filter_map(std::result::Result::ok)
                .collect(),
            _ => vec![],
        };
        candidates.into_iter().max()
    };
    let root = from_env.or_else(common)?;
    let lib = if os == "windows" {
        root.join("Lib")
    } else {
        root.join("lib")
    };
    lib.is_dir().then_some(lib)
}

/// vulkan feature 启用时为最终链接补上 Vulkan loader 库。
///
/// ggml-vulkan 以静态库参与链接，它在 CMake 里 PRIVATE 声明的
/// `Vulkan::Vulkan` 依赖不会传导到最终可执行文件（与 CUDA 同理），
/// 因此这里显式输出 loader 链接；SDK 缺失时链接会报 LNK2019 无法解析的
/// `vk*` 符号，这里提前给出可读提示。
fn emit_vulkan_links(os: &str) {
    if !cfg!(feature = "vulkan") {
        return;
    }
    println!("cargo:rerun-if-env-changed=VULKAN_SDK");
    let lib_name = if os == "windows" {
        "vulkan-1"
    } else {
        "vulkan"
    };
    if let Some(lib_dir) = vulkan_sdk_lib_dir(os) {
        println!("cargo:rustc-link-search=native={}", lib_dir.display());
        debug_log!("Vulkan loader 库目录: {}", lib_dir.display());
    } else {
        println!(
            "cargo:warning=vulkan feature 已启用但未找到 Vulkan SDK。请安装 LunarG Vulkan SDK，\
             或设置 VULKAN_SDK 环境变量（如 C:\\VulkanSDK\\1.4.328.1），否则最终链接会因缺 \
             vk* 符号失败（LNK2019）"
        );
    }
    println!("cargo:rustc-link-lib={}", lib_name);
    debug_log!("Vulkan loader 链接: {}", lib_name);
}

/// 收集"以 feature 方式启用的模型族"。
///
/// Cargo 会把每个启用的 feature 以环境变量 `CARGO_FEATURE_<名>`（名字中的
/// `-` 映射为 `_`，全大写）注入 build.rs。本项目约定模型族 feature 一律以
/// `model-` 为前缀（如 `model-qwen3-asr`），其对应环境变量即为
/// `CARGO_FEATURE_MODEL_QWEN3_ASR`。拿到后缀后大写→小写即是上游 CMake 的
/// alias/target 名（如 `qwen3_asr`），因此新增模型族 feature 时无需改动
/// build.rs，只要在 Cargo.toml 里声明的名字与上游 target/alias 一致。
fn enabled_model_features() -> Vec<String> {
    let mut names = Vec::new();
    for (key, _) in env::vars() {
        if let Some(suffix) = key.strip_prefix("CARGO_FEATURE_MODEL_") {
            names.push(suffix.to_lowercase());
        }
    }
    names.sort();
    names
}

/// 把 feature 名（`citrinet_asr` 等）与 `AUDIOCPP_MODELS` 环境变量的内容
/// 合并去重，作为传给 CMake 的 `AUDIOCPP_MODELS` 取值。
fn merge_custom_models(feature_names: Vec<String>) -> String {
    let mut all: Vec<String> = feature_names;
    if let Ok(env_models) = env::var("AUDIOCPP_MODELS") {
        for m in env_models
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            if !all.iter().any(|s| s == m) {
                all.push(m.to_string());
            }
        }
    }
    all.join(",")
}

/// 输出平台相关的系统库链接指令（CMake 路径与预编译路径共用）。
///
/// - Windows：`advapi32`（ggml-cpu 经注册表查询 CPU 特性）；
/// - CUDA / Vulkan：静态库不传导它们 PRIVATE 的依赖，启用对应 feature 时
///   必须显式补齐（见 `emit_cuda_links` / `emit_vulkan_links`）。
fn emit_platform_links(os: &str) {
    if os == "windows" {
        println!("cargo:rustc-link-lib=advapi32");
    }

    // CUDA 运行时库：静态的 ggml-cuda/engine_runtime 不会传导它们 PRIVATE
    // 的 CUDA 依赖，启用 cuda feature 时必须显式补齐（缺失时报明确错误）。
    emit_cuda_links(os);

    // Vulkan loader 库：同理，ggml-vulkan 是静态库，PRIVATE 的 Vulkan 依赖
    // 不传导到最终可执行文件，启用 vulkan feature 时必须显式补齐。
    emit_vulkan_links(os);
}

/// 编译 C shim（capi.cpp）为独立静态库 `audio_cpp_capi`。
///
/// 需要 audio.cpp 源码头文件（include 目录），因此 prebuilt 旁路也依赖
/// `ensure_audio_src()` 的源码树；只是不进行 CMake 构建。
fn compile_capi_shim(manifest_dir: &Path, src_dir: &Path, build_dir: &Path, os: &str) {
    let mut cpp = cc::Build::new();
    cpp.cpp(true)
        .file(manifest_dir.join("capi.cpp"))
        .include(src_dir.join("include"))
        .include(src_dir.join("external/ggml/include"))
        .include(src_dir.join("external/sentencepiece/src"))
        .include(src_dir.join("external/llama_tokenizer"))
        .include(src_dir.join("external/cJSON"))
        .include(src_dir.join("external/libyaml/include"))
        .include(build_dir.join("generated"))
        .pic(true);
    if os == "windows" {
        // MSVC 使用 /std:c++17 语法；同时开启 /utf-8（capi.cpp 含中文注释）。
        cpp.flag("/std:c++17").flag("/utf-8").flag("/EHsc");
    } else {
        cpp.flag_if_supported("-std=c++17");
    }
    if !cfg!(feature = "openmp") {
        cpp.flag_if_supported("-fno-openmp");
    }
    cpp.compile("audio_cpp_capi");
}

/// 用 bindgen 为 capi.h 生成 Rust FFI 绑定，写入 OUT_DIR/bindings.rs。
fn generate_bindings(manifest_dir: &Path, out_dir: &Path, os: &str) {
    let mut bindings_builder = bindgen::Builder::default()
        .header(manifest_dir.join("capi.h").to_str().unwrap())
        .allowlist_function("audiocpp_.*")
        .allowlist_type("audiocpp_.*")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .derive_partialeq(true);

    // MSVC 目标下，把编译器（cc crate 探测到的）INCLUDE 环境变量透传给
    // bindgen 的 clang，否则标准头文件无法解析。
    if os == "windows" {
        let cc = cc::Build::new();
        let compiler = cc.try_get_compiler().expect("探测 C 编译器失败");
        if let Some((_, include_env)) = compiler
            .env()
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("INCLUDE"))
        {
            for inc in include_env
                .to_string_lossy()
                .split(';')
                .filter(|s| !s.is_empty())
            {
                bindings_builder = bindings_builder.clang_arg("-isystem").clang_arg(inc);
            }
        }
        let target = env::var("TARGET").unwrap_or_default();
        bindings_builder = bindings_builder
            .clang_arg(format!("--target={}", target))
            .clang_arg("-fms-compatibility")
            .clang_arg("-fms-extensions");
    }

    let bindings = bindings_builder.generate().expect("生成 capi 绑定失败");
    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("写入 bindings.rs 失败");
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml"); // feature 组合变化（model-* 增删）会重跑
    println!("cargo:rerun-if-changed=capi.h");
    println!("cargo:rerun-if-changed=capi.cpp");
    println!("cargo:rerun-if-env-changed=AUDIOCPP_PREBUILT_DIR");
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_FEATURE"); // crt-static 切换会重跑
    println!("cargo:rerun-if-env-changed=AUDIOCPP_PREBUILT_TAG");
    println!("cargo:rerun-if-env-changed=AUDIOCPP_PREBUILT_REPO");
    println!("cargo:rerun-if-env-changed=AUDIOCPP_PREBUILT_URL");
    println!("cargo:rerun-if-env-changed=AUDIOCPP_PREBUILT_OFF");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR 未设置");
    let manifest_dir = PathBuf::from(&manifest_dir);

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let os = target_os();
    let crt_static = crt_static_enabled();

    // ------------------------------------------------------------------
    // 可选预编译旁路：设置 AUDIOCPP_PREBUILT_DIR 后跳过整个 CMake 构建，
    // 直接链接外部提供的 engine_runtime 等静态库（跳过 CMake 编译）。
    // 与 llama-cpp-rs 的 LLAMA_PREBUILT_DIR 约定一致。
    //
    // 目录布局灵活：库文件可在 <dir>、<dir>/lib、<dir>/lib64、<dir>/bin。
    // 仍依赖 audio.cpp 源码头文件编译 C shim（capi.cpp），故 prebuilt 旁路
    // 也会调用 ensure_audio_src()；只是不做 CMake 构建。
    //
    // CRT 变体：Windows 预编译资产区分 `/MD`（动态 CRT，资产名 `-md`）与
    // `/MT`（静态 CRT，`crt-static` 开启，资产名 `-mt`）。crt-static 时下载端
    // 自动选 `-mt` 资产，无匹配资产（如旧 release 只发过 md）才回退源码构建，
    // 源码构建仍会走下方 CMake 的 /MT 强制逻辑。Linux/macOS 无此维度。
    // ------------------------------------------------------------------
    // 预编译是否会被尝试：显式 AUDIOCPP_PREBUILT_DIR 或启用了 `prebuilt`
    // feature（自动下载）。两者都没有时走纯源码构建。
    if let Some(prebuilt_dir) = resolve_prebuilt_directory(false) {
        println!(
            "cargo:warning=使用预编译 audio.cpp 静态库：{}（跳过 CMake 构建）",
            prebuilt_dir.display()
        );

        let src_dir = ensure_audio_src();

        let mut search_dirs = vec![
            prebuilt_dir.clone(),
            prebuilt_dir.join("lib"),
            prebuilt_dir.join("lib64"),
            prebuilt_dir.join("bin"),
        ];
        search_dirs.retain(|d| d.is_dir());
        for d in &search_dirs {
            println!("cargo:rustc-link-search=native={}", d.display());
        }
        debug_log!("prebuilt 链接搜索目录: {:?}", search_dirs);

        let lib_names = extract_static_lib_names(&search_dirs, &os);
        assert!(
            lib_names.iter().any(|n| n == "engine_runtime"),
            "AUDIOCPP_PREBUILT_DIR 下未找到 engine_runtime 静态库（找到: {:?}）",
            lib_names
        );
        link_static_libs(&lib_names);
        debug_log!("prebuilt 静态库: {:?}", lib_names);

        emit_platform_links(&os);
        compile_capi_shim(&manifest_dir, &src_dir, &out_dir, &os);
        generate_bindings(&manifest_dir, &out_dir, &os);
        return;
    }

    let src_dir = ensure_audio_src();

    // ------------------------------------------------------------------
    // Windows 路径长度规避：MSVC 工具链（cl.exe/rc.exe）在路径超过 ~250
    // 字符时会直接失败（实测 cl.exe 报 C1083、rc.exe 在 manifest 嵌入环节报
    // RC2136）。`vulkan` feature 会触发 ggml 的 vulkan-shaders-gen
    // ExternalProject，其构建目录嵌套极深
    // （out/build/ggml/src/ggml-vulkan/vulkan-shaders-gen-prefix/src/...），
    // 叠加 OUT_DIR 前缀很容易越过 CMAKE_OBJECT_PATH_MAX(250)。此时把 CMake
    // 构建目录重定向到系统临时目录下的短路径（按 OUT_DIR 哈希取唯一子目录，
    // 保住跨构建的增量缓存），下游用户无需手动设置 CARGO_TARGET_DIR。
    // ------------------------------------------------------------------
    const MAX_SAFE_PATH: usize = 240; // 略低于 CMAKE_OBJECT_PATH_MAX(250)
    const VULKAN_EXTRA_PATH: usize = 161; // 嵌套 ExternalProject 相对 OUT_DIR/build 的最大额外深度
    let build_root = out_dir.join("build");
    let projected_len = build_root.to_string_lossy().len()
        + if cfg!(feature = "vulkan") {
            VULKAN_EXTRA_PATH
        } else {
            0
        };
    let cmake_dir = if projected_len > MAX_SAFE_PATH {
        let mut h = DefaultHasher::new();
        out_dir.to_string_lossy().hash(&mut h);
        let short = std::env::temp_dir().join(format!("acb{:012x}", h.finish()));
        println!(
            "cargo:warning=MSVC 路径过长（预计 {} 字符，上限约 250），CMake 构建目录重定向到 {}",
            projected_len,
            short.display()
        );
        short
    } else {
        out_dir.clone()
    };

    // ------------------------------------------------------------------
    // 1. 用 CMake 构建 audio.cpp 的 engine_runtime 静态库。
    //    强制使用 Ninja 生成器，保证单一配置（single-config）的输出布局
    //    （归档直接落在 OUT_DIR/lib 下），不受宿主机默认生成器影响。
    // ------------------------------------------------------------------
    let mut config = Config::new(&src_dir);
    config.generator("Ninja");
    if cmake_dir != out_dir {
        // 重定向场景下让 cmake crate 在短目录里 configure/build。
        config.out_dir(&cmake_dir);
    }

    // MSVC 目标下全局注入编译选项：
    //   - /utf-8  —— audio.cpp 源码是 UTF-8 无 BOM，MSVC 默认按 ANSI 代码页
    //                解析，含中文文本的源（如 chinese_normalization.cpp）会报
    //                C2001；上游只给个别文件加了此选项，这里对所有目标生效。
    //   - /EHsc   —— 启用 C++ 异常展开语义，避免 C4530 警告。
    //   - crt-static：强制 CMake 全目标 `/MT`（静态 CRT）。cargo 开启
    //     `-C target-feature=+crt-static` 后，Rust 侧 std 与 cc 编译的 C shim
    //     都是 `/MT`，若 engine_runtime 仍是默认 `/MD` 会链接报 LNK2038 /
    //     LNK2019（`__imp_*` 无法解析）。这里用 CMP0091 的
    //     CMAKE_MSVC_RUNTIME_LIBRARY 统一切换；`sentencepiece` 等子目录
    //     cmake_minimum_required 较低（3.5），需显式
    //     CMAKE_POLICY_DEFAULT_CMP0091=NEW 强制策略，否则不会继承 `/MT`。
    if os == "windows" {
        config.cxxflag("/utf-8").cxxflag("/EHsc");
        config.cflag("/utf-8");
        if crt_static {
            let profile =
                env::var("AUDIOCPP_LIB_PROFILE").unwrap_or_else(|_| "Release".to_string());
            let runtime = if profile.contains("Debug") {
                "MultiThreadedDebug"
            } else {
                "MultiThreaded"
            };
            config.define("CMAKE_POLICY_DEFAULT_CMP0091", "NEW");
            config.define("CMAKE_MSVC_RUNTIME_LIBRARY", runtime);
            println!(
                "cargo:warning=crt-static 已启用：CMake 全目标强制 /MT（{}）",
                runtime
            );
        }
    }

    // 只构建库本身，关闭示例/测试/benchmark，避免无关目标进入构建图。
    config.define("ENGINE_BUILD_EXAMPLES", "OFF");
    config.define("ENGINE_BUILD_TESTS", "OFF");
    config.define("ENGINE_BUILD_WARMBENCH", "OFF");
    config.define("AUDIOCPP_DEPLOYMENT_BUILD", "OFF");
    config.define("SPM_BUILD_TEST", "OFF");
    config.define("SPM_ENABLE_SHARED", "OFF");

    // 后端选项由 Cargo feature 映射（与工作区 feature 划分保持一致）。
    config.define(
        "ENGINE_ENABLE_CUDA",
        if cfg!(feature = "cuda") { "ON" } else { "OFF" },
    );
    config.define(
        "ENGINE_ENABLE_HIP",
        if cfg!(feature = "hip") { "ON" } else { "OFF" },
    );
    config.define(
        "ENGINE_ENABLE_VULKAN",
        if cfg!(feature = "vulkan") {
            "ON"
        } else {
            "OFF"
        },
    );
    let metal_on = cfg!(feature = "metal") || (os == "apple");
    config.define("ENGINE_ENABLE_METAL", if metal_on { "ON" } else { "OFF" });
    // OpenMP：既控制 engine_runtime 自身的链接（ENGINE_ENABLE_OPENMP），
    // 也要同步 ggml 的 GGML_OPENMP（上游 audio.cpp 未把它接到前者，默认 ON，
    // 若不显式关闭，ggml 内部仍会 /openmp 编译并动态链接 vcomp140.dll）。
    // MSVC 的 OpenMP 运行时只有 DLL 版（vcomp140，无静态库），因此
    // crt-static（静态 CRT）下必须彻底关闭 OpenMP，否则产物仍依赖该 DLL。
    let openmp_on = cfg!(feature = "openmp") && !(crt_static && os == "windows");
    config.define("ENGINE_ENABLE_OPENMP", if openmp_on { "ON" } else { "OFF" });
    config.define("GGML_OPENMP", if openmp_on { "ON" } else { "OFF" });
    if crt_static && os == "windows" && cfg!(feature = "openmp") {
        println!(
            "cargo:warning=crt-static 已启用：MSVC OpenMP 运行时无静态版（vcomp140.dll），强制关闭 OpenMP"
        );
    }
    config.define(
        "ENGINE_ENABLE_NATIVE_CPU",
        if cfg!(feature = "native") {
            "ON"
        } else {
            "OFF"
        },
    );

    // 把所有静态归档统一输出到 OUT_DIR/lib，便于后续 glob 收集与链接。
    // audio.cpp 的 CMake 未设置 archive 输出目录，归档默认散落在各
    // target 的构建子目录（ggml/src、external/sentencepiece/src 等）。
    config.define(
        "CMAKE_ARCHIVE_OUTPUT_DIRECTORY",
        cmake_dir.join("lib").to_string_lossy().into_owned(),
    );

    // 模型组合选择（映射 AUDIOCPP_MODEL_SET：full / core / custom）。
    let model_set = if cfg!(feature = "full-models") {
        "full"
    } else if cfg!(feature = "custom-models") {
        // custom：只编译指定的模型族。来源有二，且会取并集：
        //   1. `model-<族>` feature（如 model-qwen3-asr）——build.rs 扫描
        //      CARGO_FEATURE_MODEL_* 自动收集，无需手动设置环境变量；
        //   2. `AUDIOCPP_MODELS` 环境变量（逗号分隔的 alias/target 名）。
        // 引擎核心 + 内置 VAD 始终编入，见上游 CMakeLists 的 AUDIOCPP_RUNTIME_OBJECTS。
        let requested = merge_custom_models(enabled_model_features());
        if requested.is_empty() {
            panic!(
                "feature `custom-models` 未指定任何模型族。请至少先启用一个 \
                 `model-<族>` feature（如 --features model-qwen3-asr），或设置 \
                 环境变量 AUDIOCPP_MODELS（逗号分隔的模型族目标，如 \
                 AUDIOCPP_MODELS=qwen3_asr,citrinet_asr）"
            );
        }
        println!("cargo:rerun-if-env-changed=AUDIOCPP_MODELS");
        config.define("AUDIOCPP_MODELS", &requested);
        debug_log!("AUDIOCPP_MODELS(合并后)={}", requested);
        "custom"
    } else {
        "core"
    };
    config.define("AUDIOCPP_MODEL_SET", model_set);

    // 透传 GGML_*/CMAKE_* 环境变量，方便下游用户按需微调 ggml 选项，
    // 而无需修改本脚本。优先级低于脚本中显式设置的选项。
    for (key, value) in env::vars() {
        if key.starts_with("GGML_") || key.starts_with("CMAKE_") {
            println!("cargo:rerun-if-env-changed={key}");
            config.define(&key, &value);
        }
    }

    // 用 cc crate 探测 MSVC 编译器，把正确解析出的 INCLUDE/LIB 环境注入
    // CMake 子进程。未运行 vcvarsall 的普通 shell 下，CMake 直接调 cl.exe
    // 会因缺少 INCLUDE 而找不到标准头（stdbool.h 等）。
    if os == "windows" {
        let cc = cc::Build::new();
        let compiler = cc.try_get_compiler().expect("探测 C 编译器失败");
        for (key, value) in compiler.env().iter().filter(|(k, _)| {
            k.eq_ignore_ascii_case("INCLUDE")
                || k.eq_ignore_ascii_case("LIB")
                || k.eq_ignore_ascii_case("PATH")
        }) {
            debug_log!(
                "注入 MSVC 环境变量 {}={}",
                key.to_string_lossy(),
                value.to_string_lossy()
            );
            config.env(key, value);
        }
    }

    let profile = env::var("AUDIOCPP_LIB_PROFILE").unwrap_or_else(|_| "Release".to_string());
    let build_dir = config
        .profile(&profile)
        .build_target("engine_runtime")
        .very_verbose(env::var("CMAKE_VERBOSE").is_ok())
        // 每次构建都重新 configure：CMake 会在 configure 时根据
        // AUDIOCPP_MODEL_SET / AUDIOCPP_MODELS 重新生成 registry.inc，
        // 因此切换模型组合后能正确更新注册的 loader 集合（Ninja 只会
        // 重编受影响的 registry.cpp 及链接）。
        .always_configure(true)
        .build();

    println!("cargo:rerun-if-env-changed=AUDIOCPP_LIB_PROFILE");

    // cmake crate 会把归档放在 cmake_dir/lib（及 lib64）；多配置生成器下
    // 还会出现 cmake_dir/lib/<Config> 子目录。全部加入链接搜索路径。
    let mut search_dirs = vec![
        cmake_dir.join("lib"),
        cmake_dir.join("lib64"),
        build_dir.clone(),
    ];
    debug_log!(
        "out_dir={} cmake_dir={} build_dir={}",
        out_dir.display(),
        cmake_dir.display(),
        build_dir.display()
    );
    search_dirs.retain(|d| d.is_dir());
    for cfg in ["Release", "RelWithDebInfo", "Debug"] {
        for base in [&cmake_dir, &build_dir] {
            let d = base.join("lib").join(cfg);
            if d.is_dir() {
                search_dirs.push(d);
            }
        }
    }
    let mut seen: Vec<PathBuf> = Vec::new();
    for d in &search_dirs {
        if !seen.contains(d) {
            println!("cargo:rustc-link-search=native={}", d.display());
            seen.push(d.clone());
        }
    }

    let lib_names = extract_static_lib_names(&search_dirs, &os);
    assert!(
        lib_names.iter().any(|n| n == "engine_runtime"),
        "在 OUT_DIR 下未找到 engine_runtime 静态库（找到: {:?}）",
        lib_names
    );
    link_static_libs(&lib_names);
    debug_log!("发现的静态库: {:?}", lib_names);

    emit_platform_links(&os);

    // 2. 用 cc crate 编译 C shim（capi.cpp），以独立静态库形式提供 C ABI
    //    符号。最终由 Rust 侧链接 engine_runtime 及其依赖库。
    compile_capi_shim(&manifest_dir, &src_dir, &build_dir, &os);

    // 3. 用 bindgen 为 C shim 生成 Rust 绑定。
    generate_bindings(&manifest_dir, &out_dir, &os);
}
