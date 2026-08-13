#!/usr/bin/env bash
# 收集 audio-cpp 预编译静态库（Windows CI）。
#
# Windows 上 build.rs 用 CMAKE_ARCHIVE_OUTPUT_DIRECTORY 把归档集中到
# cmake_dir/lib；cmake_dir 默认 = OUT_DIR（cargo build/<crate>-<hash>/out），
# 路径过长时重定向到 %TEMP%\acb<hash>。两种位置都扫描。
#
# 用法：collect-windows-prebuilt.sh <输出根目录>

set -euo pipefail

ROOT="${1:?output root}"

mkdir -p "$ROOT/lib" "$ROOT/lib64" "$ROOT/bin"

is_audio_lib_name() {
  local base="$1"
  case "$base" in
    engine_runtime.lib|ggml*.lib|sentencepiece.lib|cjson_vendor.lib|yaml_vendor.lib) return 0 ;;
    *) return 1 ;;
  esac
}

collect_from() {
  local search_root="$1"
  [[ -d "$search_root" ]] || return 0
  while IFS= read -r -d '' f; do
    local base
    base="$(basename "$f")"
    if is_audio_lib_name "$base"; then
      cp -f "$f" "$ROOT/lib/$base"
    fi
  done < <(find "$search_root" -type f -name '*.lib' -print0 2>/dev/null)
}

# OUT_DIR 下的 cmake install 树。
collect_from target

# MAX_PATH 规避重定向目录（%TEMP%\acb<hash>）。
TEMP_DIR="${TEMP:-$TMPDIR}"
if [[ -n "${TEMP_DIR:-}" ]]; then
  while IFS= read -r -d '' d; do
    collect_from "$d"
  done < <(find "$TEMP_DIR" -maxdepth 1 -type d -name 'acb*' -print0 2>/dev/null)
fi

COUNT="$(find "$ROOT/lib" -type f 2>/dev/null | wc -l | tr -d ' ')"
echo "Collected $COUNT file(s) for windows"

if [[ "$COUNT" -lt 1 ]]; then
  echo "::error::No libraries collected"
  echo "::group::Debug: .lib files under target"
  find target -type f -name '*.lib' 2>/dev/null | head -50 || true
  echo "::endgroup::"
  exit 1
fi