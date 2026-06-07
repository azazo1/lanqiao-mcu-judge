#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=/dev/null
. "$script_dir/path-helpers.sh"

usage() {
    cat <<'EOF' >&2
用法:
  bash scripts/build-sample-macos.sh <sample>

环境变量:
  推荐通过 just 调用, 由 just 自动加载项目根目录 .env.
  KEIL_UV4_LAUNCHER      已配置好的 UV4 启动器. 例如 CrossOver 生成的命令行快捷方式.
  KEIL_WINE              Wine 可执行文件路径. 默认值为 wine.
  KEIL_WINEPREFIX        普通 Wine 的 WINEPREFIX. 仅在未使用 CrossOver bottle 时生效.
  KEIL_CROSSOVER_BOTTLE  CrossOver bottle 名称.
  KEIL_CROSSOVER_APP     CrossOver 中的应用名. 例如 UV4.exe.
  KEIL_UV4               UV4.exe 的 Windows 路径. 默认值为 C:\Keil_v5\UV4\UV4.exe.
EOF
    exit 1
}

if [ "$#" -ne 1 ]; then
    usage
fi

sample="$1"
sample_dir="sample/$sample"
prj_dir="$sample_dir/prj"
objects_dir="$prj_dir/Objects"

if [ ! -d "$sample_dir" ]; then
    echo "sample 不存在: $sample_dir" >&2
    exit 1
fi

if [ ! -d "$prj_dir" ]; then
    echo "工程目录不存在: $prj_dir" >&2
    exit 1
fi

uvproj="$(find "$prj_dir" -maxdepth 1 -type f -name '*.uvproj' | sort | head -n 1)"
if [ -z "$uvproj" ]; then
    echo "未找到 uvproj: $prj_dir" >&2
    exit 1
fi

xml_first_text() {
    local tag="$1"
    sed -n "s:.*<$tag>\\(.*\\)</$tag>.*:\\1:p" "$uvproj" | head -n 1 | tr -d '\r'
}

target_name="$(xml_first_text TargetName)"
output_name="$(xml_first_text OutputName)"

if [ -z "$target_name" ]; then
    target_name="$(basename "$uvproj" .uvproj)"
fi

if [ -z "$output_name" ]; then
    output_name="$target_name"
fi

mkdir -p "$objects_dir"
log_path="$objects_dir/uv4.log"
hex_path="$objects_dir/$output_name.hex"

abs_path() {
    local path="$1"
    local dir
    local base
    dir="$(cd "$(dirname "$path")" && pwd)"
    base="$(basename "$path")"
    printf '%s/%s\n' "$dir" "$base"
}

to_windows_path() {
    local unix_path
    unix_path="$(abs_path "$1")"
    printf 'Z:%s\n' "${unix_path//\//\\}"
}

require_executable() {
    local candidate="$1"
    if [ -x "$candidate" ]; then
        return 0
    fi
    if command -v "$candidate" >/dev/null 2>&1; then
        return 0
    fi
    echo "未找到可执行文件: $candidate" >&2
    exit 1
}

project_win="$(to_windows_path "$uvproj")"
log_win="$(to_windows_path "$log_path")"

run_with_launcher() {
    local launcher="$1"
    require_executable "$launcher"
    "$launcher" -b "$project_win" -j0 -t "$target_name" -o "$log_win"
}

run_with_wine() {
    local wine_bin="${KEIL_WINE:-wine}"
    local uv4_path="${KEIL_UV4:-C:\\Keil_v5\\UV4\\UV4.exe}"
    wine_bin="$(expand_tilde_path "$wine_bin")"
    require_executable "$wine_bin"

    if [ -n "${KEIL_CROSSOVER_BOTTLE:-}" ] && [ -n "${KEIL_CROSSOVER_APP:-}" ]; then
        "$wine_bin" --bottle "$KEIL_CROSSOVER_BOTTLE" --cx-app "$KEIL_CROSSOVER_APP" \
            -b "$project_win" -j0 -t "$target_name" -o "$log_win"
        return
    fi

    if [ -n "${KEIL_CROSSOVER_BOTTLE:-}" ]; then
        "$wine_bin" --bottle "$KEIL_CROSSOVER_BOTTLE" "$uv4_path" \
            -b "$project_win" -j0 -t "$target_name" -o "$log_win"
        return
    fi

    if [ -n "${KEIL_WINEPREFIX:-}" ]; then
        local wineprefix_path
        wineprefix_path="$(expand_tilde_path "$KEIL_WINEPREFIX")"
        WINEPREFIX="$wineprefix_path" "$wine_bin" "$uv4_path" \
            -b "$project_win" -j0 -t "$target_name" -o "$log_win"
        return
    fi

    "$wine_bin" "$uv4_path" -b "$project_win" -j0 -t "$target_name" -o "$log_win"
}

echo "==> 构建 sample/$sample"
echo "uvproj: $uvproj"
echo "target: $target_name"
echo "log: $log_path"

if [ -n "${KEIL_UV4_LAUNCHER:-}" ]; then
    run_with_launcher "$(expand_tilde_path "$KEIL_UV4_LAUNCHER")"
else
    run_with_wine
fi

if [ -f "$hex_path" ]; then
    echo "hex: $hex_path"
else
    echo "构建已结束, 但未找到 hex. 请检查日志: $log_path"
fi
