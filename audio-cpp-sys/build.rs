use std::env;
use std::path::{Path, PathBuf};

use cmake::Config;
use glob::glob;

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
/// 与 llama-cpp-rs（用 submodule 引入 llama.cpp）不同，本项目按用户选择
/// 保留 vendored 拷贝（`gitignore` 已排除该目录，不提交进仓库）。
fn audio_src_dir() -> PathBuf {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR 未设置");
    Path::new(&manifest_dir).join("audio.cpp")
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
            let Some(stem) = path.file_stem() else { continue };
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

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=capi.h");
    println!("cargo:rerun-if-changed=capi.cpp");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR 未设置");
    let manifest_dir = PathBuf::from(&manifest_dir);
    let src_dir = audio_src_dir();
    assert!(
        src_dir.join("CMakeLists.txt").exists(),
        "vendored audio.cpp 源码树不存在于 {}",
        src_dir.display()
    );

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let os = target_os();

    // ------------------------------------------------------------------
    // 1. 用 CMake 构建 audio.cpp 的 engine_runtime 静态库。
    //    强制使用 Ninja 生成器，保证单一配置（single-config）的输出布局
    //    （归档直接落在 OUT_DIR/lib 下），不受宿主机默认生成器影响。
    // ------------------------------------------------------------------
    let mut config = Config::new(&src_dir);
    config.generator("Ninja");

    // MSVC 目标下全局注入编译选项：
    //   - /utf-8  —— audio.cpp 源码是 UTF-8 无 BOM，MSVC 默认按 ANSI 代码页
    //                解析，含中文文本的源（如 chinese_normalization.cpp）会报
    //                C2001；上游只给个别文件加了此选项，这里对所有目标生效。
    //   - /EHsc   —— 启用 C++ 异常展开语义，避免 C4530 警告。
    if os == "windows" {
        config.cxxflag("/utf-8").cxxflag("/EHsc");
        config.cflag("/utf-8");
    }

    // 只构建库本身，关闭示例/测试/benchmark，避免无关目标进入构建图。
    config.define("ENGINE_BUILD_EXAMPLES", "OFF");
    config.define("ENGINE_BUILD_TESTS", "OFF");
    config.define("ENGINE_BUILD_WARMBENCH", "OFF");
    config.define("AUDIOCPP_DEPLOYMENT_BUILD", "OFF");
    config.define("SPM_BUILD_TEST", "OFF");
    config.define("SPM_ENABLE_SHARED", "OFF");

    // 后端选项由 Cargo feature 映射（与工作区 feature 划分保持一致）。
    config.define("ENGINE_ENABLE_CUDA", if cfg!(feature = "cuda") { "ON" } else { "OFF" });
    config.define("ENGINE_ENABLE_HIP", if cfg!(feature = "hip") { "ON" } else { "OFF" });
    config.define("ENGINE_ENABLE_VULKAN", if cfg!(feature = "vulkan") { "ON" } else { "OFF" });
    let metal_on = cfg!(feature = "metal") || (os == "apple");
    config.define("ENGINE_ENABLE_METAL", if metal_on { "ON" } else { "OFF" });
    config.define("ENGINE_ENABLE_OPENMP", if cfg!(feature = "openmp") { "ON" } else { "OFF" });
    config.define("ENGINE_ENABLE_NATIVE_CPU", if cfg!(feature = "native") { "ON" } else { "OFF" });

    // 把所有静态归档统一输出到 OUT_DIR/lib，便于后续 glob 收集与链接。
    // audio.cpp 的 CMake 未设置 archive 输出目录，归档默认散落在各
    // target 的构建子目录（ggml/src、external/sentencepiece/src 等）。
    config.define(
        "CMAKE_ARCHIVE_OUTPUT_DIRECTORY",
        out_dir.join("lib").to_string_lossy().into_owned(),
    );

    // 模型组合选择（映射 AUDIOCPP_MODEL_SET：full / core）。
    let model_set = if cfg!(feature = "full-models") {
        "full"
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
        .always_configure(false)
        .build();

    println!("cargo:rerun-if-env-changed=AUDIOCPP_LIB_PROFILE");

    // cmake crate 会把归档放在 OUT_DIR/lib（及 lib64）；多配置生成器下
    // 还会出现 OUT_DIR/lib/<Config> 子目录。全部加入链接搜索路径。
    let mut search_dirs = vec![
        out_dir.join("lib"),
        out_dir.join("lib64"),
        build_dir.clone(),
    ];
    debug_log!("out_dir={} build_dir={}", out_dir.display(), build_dir.display());
    search_dirs.retain(|d| d.is_dir());
    for cfg in ["Release", "RelWithDebInfo", "Debug"] {
        for base in [&out_dir, &build_dir] {
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

    // 平台系统库：ggml-cpu 在 Windows 上通过注册表查询 CPU 特性（advapi32），
    // 跨进最终可执行文件链接期才需要解析，因此作为 link 指令透传给下游。
    if os == "windows" {
        println!("cargo:rustc-link-lib=advapi32");
    }

    // ------------------------------------------------------------------
    // 2. 用 cc crate 编译 C shim（capi.cpp），以独立静态库形式提供
    //    C ABI 符号。最终由 Rust 侧链接 engine_runtime 及其依赖库。
    // ------------------------------------------------------------------
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

    // ------------------------------------------------------------------
    // 3. 用 bindgen 为 C shim 生成 Rust 绑定。
    // ------------------------------------------------------------------
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
            for inc in include_env.to_string_lossy().split(';').filter(|s| !s.is_empty()) {
                bindings_builder = bindings_builder.clang_arg("-isystem").clang_arg(inc);
            }
        }
        let target = env::var("TARGET").unwrap_or_default();
        bindings_builder = bindings_builder
            .clang_arg(format!("--target={}", target))
            .clang_arg("-fms-compatibility")
            .clang_arg("-fms-extensions");
    }

    let bindings = bindings_builder
        .generate()
        .expect("生成 capi 绑定失败");
    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("写入 bindings.rs 失败");
}
