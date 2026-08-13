#!/usr/bin/env bash
# 收集 audio-cpp 预编译静态库（Linux / macOS CI）。
#
# audio-cpp-sys 的 build.rs 用 CMAKE_ARCHIVE_OUTPUT_DIRECTORY 把归档集中放到
# cmake_dir/lib（cmake_dir 默认即 OUT_DIR，位于 cargo 的 build/<crate>-<hash>/out）。
# 静态 .a 文件因此散布在 target/**/out/lib 下，这里按库名过滤后复制到归档目录。
#
# 用法：collect-unix-prebuilt.sh <输出根目录> <os>   # os 为 linux / macos

set -euo pipefail

ROOT="${1:?output root}"
OS="${2:?linux or macos}"

mkdir -p "$ROOT/lib" "$ROOT/lib64" "$ROOT/bin"

is_audio_lib_name() {
  local base="$1"
  base="${base#lib}"        # Unix 归档为 lib<name>.a，剥掉 lib 前缀
  base="${base%.a*}"        # 去掉扩展名
  case "$base" in
    engine_runtime*|ggml*|sentencepiece*|cjson_vendor*|yaml_vendor*) return 0 ;;
    *) return 1 ;;
  esac
}

collect_static() {
  local search_root="$1"
  [[ -d "$search_root" ]] || return 0
  while IFS= read -r -d '' f; do
    local base
    base="$(basename "$f")"
    if is_audio_lib_name "$base" && [[ "$base" == *.a ]]; then
      # BSD cp 无 -n，手动判重
      if [[ ! -e "$ROOT/lib/$base" ]]; then
        cp -f "$f" "$ROOT/lib/$base" 2>/dev/null || true
      fi
    fi
  done < <(find "$search_root" -type f -name '*.a' -print0 2>/dev/null)
}

# 主要来源：cargo 的 OUT_DIR=（target/**/out）下的 cmake install 树。
collect_static target

# 兜底：Windows MAX_PATH 规避会把 CMake 构建树重定向到系统临时目录
# （acb<hash>），Unix 上不会发生；此处仅为未来兼容，扫描可用的几个位置。
for extra in "$TMPDIR" "/tmp"; do
  [[ -d "$extra" ]] || continue
  while IFS= read -r -d '' d; do
    collect_static "$d"
  done < <(find "$extra" -maxdepth 1 -type d -name 'acb*' -print0 2>/dev/null)
done

COUNT="$(find "$ROOT/lib" -type f 2>/dev/null | wc -l | tr -d ' ')"
echo "Collected $COUNT file(s) for os=$OS"

if [[ "$COUNT" -lt 1 ]]; then
  echo "::error::No libraries collected" 
  echo "::group::Debug: .a files under target"
  find target -type f -name '*.a' 2>/dev/null | head -50 || true
  echo "::endgroup::"
  exit 1
fi