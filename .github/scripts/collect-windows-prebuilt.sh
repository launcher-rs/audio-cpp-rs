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
    # 注意 sentencepiece 在 MSVC 下产物可能是 sentencepiece-static.lib，
    # 必须收进包，否则消费端报 LNK1181（build.rs 按剥 -static 后缀链接）。
    engine_runtime.lib|ggml*.lib|sentencepiece*.lib|cjson_vendor.lib|yaml_vendor.lib) return 0 ;;
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

# 关键归档断言：缺 engine_runtime / sentencepiece / ggml 任一，消费端必链接失败。
# 这里失败比发一个坏包出去更容易定位（对应 build.rs 的 sentencepiece 警告）。
missing=0
for need in 'engine_runtime.lib' 'sentencepiece*.lib' 'ggml*.lib'; do
  if ! compgen -G "$ROOT/lib/$need" > /dev/null; then
    echo "::error::Missing key archive: $need"
    missing=1
  fi
done
if [[ "$missing" -ne 0 ]]; then
  echo "::group::Debug: collected files"
  find "$ROOT/lib" -type f 2>/dev/null || true
  echo "::endgroup::"
  exit 1
fi