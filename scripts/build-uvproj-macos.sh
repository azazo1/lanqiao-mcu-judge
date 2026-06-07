#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=/dev/null
. "$script_dir/path-helpers.sh"

usage() {
    cat <<'EOF' >&2
用法:
  bash scripts/build-uvproj-macos.sh <uvproj>

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

abs_existing_path() {
    local path="$1"
    local dir
    local base
    dir="$(cd "$(dirname "$path")" && pwd)"
    base="$(basename "$path")"
    printf '%s/%s\n' "$dir" "$base"
}

rel_to_abs_path() {
    local base_dir="$1"
    local path="$2"
    perl -MFile::Spec -e 'print File::Spec->rel2abs($ARGV[1], $ARGV[0])' "$base_dir" "$path"
}

trim_wrapping_quotes() {
    local value="$1"
    value="${value#\"}"
    value="${value%\"}"
    printf '%s\n' "$value"
}

trim_trailing_slashes() {
    local value="$1"
    while [ "$value" != "/" ] && [ -n "$value" ] && [ "${value%/}" != "$value" ]; do
        value="${value%/}"
    done
    printf '%s\n' "$value"
}

resolve_project_path() {
    local base_dir="$1"
    local raw_path="$2"
    local normalized

    normalized="$(trim_wrapping_quotes "$raw_path")"
    normalized="${normalized//\\//}"

    if [ -z "$normalized" ]; then
        printf '%s\n' "$base_dir"
        return 0
    fi

    case "$normalized" in
        [Zz]:/*)
            trim_trailing_slashes "${normalized:2}"
            ;;
        [A-Ya-y]:/*)
            echo "暂不支持解析非 Z 盘绝对路径: $raw_path" >&2
            exit 1
            ;;
        /*)
            trim_trailing_slashes "$normalized"
            ;;
        *)
            trim_trailing_slashes "$(rel_to_abs_path "$base_dir" "$normalized")"
            ;;
    esac
}

resolve_uvproj_path() {
    local input_path="$1"
    local expanded_path
    local candidate_path

    expanded_path="$(expand_tilde_path "$input_path")"
    if [ -f "$expanded_path" ]; then
        abs_existing_path "$expanded_path"
        return 0
    fi

    candidate_path="$(rel_to_abs_path "$(pwd)" "$expanded_path")"
    if [ -f "$candidate_path" ]; then
        abs_existing_path "$candidate_path"
        return 0
    fi

    echo "uvproj 不存在: $input_path" >&2
    exit 1
}

xml_first_text() {
    local file_path="$1"
    local tag="$2"
    sed -n "s:.*<$tag>\\(.*\\)</$tag>.*:\\1:p" "$file_path" | head -n 1 | tr -d '\r'
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

to_windows_path() {
    local unix_path="$1"
    printf 'Z:%s\n' "${unix_path//\//\\}"
}

pick_first_existing_file() {
    local candidate
    for candidate in "$@"; do
        if [ -n "$candidate" ] && [ -f "$candidate" ]; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done
    return 1
}

get_first_matching_file() {
    local dir_path="$1"
    local pattern="$2"
    find "$dir_path" -maxdepth 1 -type f -name "$pattern" | sort | head -n 1
}

get_build_report() {
    local build_log_path="$1"
    local fallback_log_path="$2"

    if [ -n "$build_log_path" ] && [ -f "$build_log_path" ]; then
        LC_ALL=C perl -0pe 's/<[^>]+>//g' "$build_log_path" | awk 'NF { print }'
        return 0
    fi

    if [ -f "$fallback_log_path" ]; then
        cat "$fallback_log_path"
        return 0
    fi

    return 1
}

run_with_launcher() {
    local launcher="$1"
    local project_win="$2"
    local target_name="$3"
    local log_win="$4"

    require_executable "$launcher"
    "$launcher" -b "$project_win" -j0 -t "$target_name" -o "$log_win"
}

run_with_wine() {
    local project_win="$1"
    local target_name="$2"
    local log_win="$3"
    local wine_bin="${KEIL_WINE:-wine}"
    local uv4_path="${KEIL_UV4:-C:\\Keil_v5\\UV4\\UV4.exe}"

    wine_bin="$(expand_tilde_path "$wine_bin")"
    require_executable "$wine_bin"

    if [ -n "${KEIL_CROSSOVER_BOTTLE:-}" ] && [ -n "${KEIL_CROSSOVER_APP:-}" ]; then
        "$wine_bin" --bottle "$KEIL_CROSSOVER_BOTTLE" --cx-app "$KEIL_CROSSOVER_APP" \
            -b "$project_win" -j0 -t "$target_name" -o "$log_win"
        return 0
    fi

    if [ -n "${KEIL_CROSSOVER_BOTTLE:-}" ]; then
        "$wine_bin" --bottle "$KEIL_CROSSOVER_BOTTLE" "$uv4_path" \
            -b "$project_win" -j0 -t "$target_name" -o "$log_win"
        return 0
    fi

    if [ -n "${KEIL_WINEPREFIX:-}" ]; then
        local wineprefix_path
        wineprefix_path="$(expand_tilde_path "$KEIL_WINEPREFIX")"
        WINEPREFIX="$wineprefix_path" "$wine_bin" "$uv4_path" \
            -b "$project_win" -j0 -t "$target_name" -o "$log_win"
        return 0
    fi

    "$wine_bin" "$uv4_path" -b "$project_win" -j0 -t "$target_name" -o "$log_win"
}

if [ "$#" -ne 1 ]; then
    usage
fi

uvproj="$(resolve_uvproj_path "$1")"
case "$uvproj" in
    *.uvproj)
        ;;
    *)
        echo "不是 uvproj 文件: $uvproj" >&2
        exit 1
        ;;
esac

uvproj_dir="$(cd "$(dirname "$uvproj")" && pwd)"
target_name="$(xml_first_text "$uvproj" TargetName)"
output_name="$(xml_first_text "$uvproj" OutputName)"
output_dir_raw="$(xml_first_text "$uvproj" OutputDirectory)"

if [ -z "$target_name" ]; then
    target_name="$(basename "$uvproj" .uvproj)"
fi

if [ -z "$output_name" ]; then
    output_name="$target_name"
fi

if [ -z "$output_dir_raw" ]; then
    output_dir_raw='Objects'
fi

output_name_path="${output_name//\\//}"
output_name_base="$(basename "$output_name_path")"
output_dir="$(resolve_project_path "$uvproj_dir" "$output_dir_raw")"

mkdir -p "$output_dir"
log_path="$output_dir/uv4.log"
hex_candidates=(
    "$output_dir/$output_name_base.hex"
    "$output_dir/$target_name.hex"
)
build_log_candidates=(
    "$output_dir/$output_name_base.build_log.htm"
    "$output_dir/$target_name.build_log.htm"
)

project_win="$(to_windows_path "$uvproj")"
log_win="$(to_windows_path "$log_path")"

echo "==> 构建 uvproj"
echo "uvproj: $uvproj"
echo "target: $target_name"
echo "output dir: $output_dir"
echo "log: $log_path"

uv4_exit_code=0
set +e
if [ -n "${KEIL_UV4_LAUNCHER:-}" ]; then
    run_with_launcher "$(expand_tilde_path "$KEIL_UV4_LAUNCHER")" "$project_win" "$target_name" "$log_win"
else
    run_with_wine "$project_win" "$target_name" "$log_win"
fi
uv4_exit_code=$?
set -e

build_log_path="$(pick_first_existing_file "${build_log_candidates[@]}" || true)"
if [ -z "$build_log_path" ] && [ -d "$output_dir" ]; then
    build_log_path="$(get_first_matching_file "$output_dir" '*.build_log.htm')"
fi

hex_path="$(pick_first_existing_file "${hex_candidates[@]}" || true)"
if [ -z "$hex_path" ] && [ -d "$output_dir" ]; then
    hex_path="$(get_first_matching_file "$output_dir" '*.hex')"
fi

build_has_zero_errors=0
error_probe_path="$build_log_path"
if [ -z "$error_probe_path" ]; then
    error_probe_path="$log_path"
fi
if [ -f "$error_probe_path" ] && grep -q '0 Error(s)' "$error_probe_path"; then
    build_has_zero_errors=1
fi

build_report=""
if build_report="$(get_build_report "$build_log_path" "$log_path")"; then
    :
fi

if [ "$uv4_exit_code" -ne 0 ] && ! { [ -n "$hex_path" ] && [ -f "$hex_path" ] && [ "$build_has_zero_errors" -eq 1 ]; }; then
    if [ -n "$build_log_path" ]; then
        echo "UV4 exited with code $uv4_exit_code. check build log: $build_log_path" >&2
    else
        echo "UV4 exited with code $uv4_exit_code. check log: $log_path" >&2
    fi
    if [ -n "$build_report" ]; then
        echo "build log:"
        printf '%s\n' "$build_report"
    fi
    exit "$uv4_exit_code"
fi

if [ -n "$build_report" ]; then
    echo "build log:"
    printf '%s\n' "$build_report"
fi

if [ -n "$hex_path" ] && [ -f "$hex_path" ]; then
    echo "hex: $hex_path"
    if [ "$uv4_exit_code" -ne 0 ]; then
        echo "UV4 exited with code $uv4_exit_code, but build log reports 0 errors."
    fi
else
    echo "构建已结束, 但未找到 hex. 请检查日志: $log_path"
    exit 1
fi
