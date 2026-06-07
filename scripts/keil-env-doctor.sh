#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=/dev/null
. "$script_dir/path-helpers.sh"

sample="${1:-arith_bench}"

print_item() {
    local level="$1"
    local label="$2"
    local value="$3"
    printf '[%s] %s: %s\n' "$level" "$label" "$value"
}

resolve_drive_c() {
    if [ -n "${KEIL_DRIVE_C:-}" ]; then
        expand_tilde_path "$KEIL_DRIVE_C"
        return 0
    fi

    if [ -n "${KEIL_CROSSOVER_BOTTLE:-}" ]; then
        local bottle_dir
        bottle_dir="$HOME/Library/Application Support/CrossOver/Bottles/$KEIL_CROSSOVER_BOTTLE/drive_c"
        if [ -d "$bottle_dir" ]; then
            printf '%s\n' "$bottle_dir"
            return 0
        fi
    fi

    if [ -n "${KEIL_WINEPREFIX:-}" ]; then
        local wineprefix_path
        wineprefix_path="$(expand_tilde_path "$KEIL_WINEPREFIX")"
        if [ -d "${wineprefix_path}/drive_c" ]; then
            printf '%s\n' "${wineprefix_path}/drive_c"
            return 0
        fi
    fi

    if [ -d "$HOME/.wine/drive_c" ]; then
        printf '%s\n' "$HOME/.wine/drive_c"
        return 0
    fi

    return 1
}

resolve_keil_root_host() {
    if [ -n "${KEIL_ROOT_HOST:-}" ]; then
        expand_tilde_path "$KEIL_ROOT_HOST"
        return 0
    fi

    local drive_c="$1"
    if [ -d "$drive_c/Keil_v5" ]; then
        printf '%s\n' "$drive_c/Keil_v5"
        return 0
    fi

    return 1
}

launcher_status() {
    if [ -n "${KEIL_UV4_LAUNCHER:-}" ]; then
        local launcher
        launcher="$(expand_tilde_path "$KEIL_UV4_LAUNCHER")"
        if [ -x "$launcher" ]; then
            print_item ok 'UV4 启动器' "$launcher"
        else
            print_item missing 'UV4 启动器' "$launcher"
        fi
        return 0
    fi

    local wine_bin="${KEIL_WINE:-wine}"
    wine_bin="$(expand_tilde_path "$wine_bin")"
    if command -v "$wine_bin" >/dev/null 2>&1; then
        print_item ok 'wine 可执行文件' "$(command -v "$wine_bin")"
    else
        print_item missing 'wine 可执行文件' "$wine_bin"
    fi
}

check_path() {
    local label="$1"
    local path="$2"
    if [ -e "$path" ]; then
        print_item ok "$label" "$path"
    else
        print_item missing "$label" "$path"
    fi
}

printf '==> Keil macOS 环境自检\n'
printf 'sample: %s\n' "$sample"

launcher_status

drive_c=''
if drive_c="$(resolve_drive_c)"; then
    print_item ok 'drive_c' "$drive_c"
else
    print_item missing 'drive_c' '未能自动定位. 可设置 KEIL_DRIVE_C 或 KEIL_WINEPREFIX 或 KEIL_CROSSOVER_BOTTLE'
fi

keil_root=''
if [ -n "$drive_c" ] && keil_root="$(resolve_keil_root_host "$drive_c")"; then
    print_item ok 'Keil 根目录' "$keil_root"
    check_path 'UV4.exe' "$keil_root/UV4/UV4.exe"
    check_path 'TOOLS.INI' "$keil_root/TOOLS.INI"
    check_path 'STC.CDB' "$keil_root/UV4/STC.CDB"
    check_path 'STC15 头文件' "$keil_root/C51/INC/STC/STC15F2K60S2.H"
else
    print_item missing 'Keil 根目录' '未能自动定位. 可设置 KEIL_ROOT_HOST'
fi

sample_dir="sample/$sample"
prj_dir="$sample_dir/prj"
uvproj="$(find "$prj_dir" -maxdepth 1 -type f -name '*.uvproj' | sort | head -n 1)"
if [ -n "$uvproj" ]; then
    target_name="$(sed -n 's:.*<TargetName>\(.*\)</TargetName>.*:\1:p' "$uvproj" | head -n 1 | tr -d '\r')"
    device_name="$(sed -n 's:.*<Device>\(.*\)</Device>.*:\1:p' "$uvproj" | head -n 1 | tr -d '\r')"
    register_file="$(sed -n 's:.*<RegisterFile>\(.*\)</RegisterFile>.*:\1:p' "$uvproj" | head -n 1 | tr -d '\r')"
    print_item ok 'uvproj' "$uvproj"
    print_item ok 'target' "${target_name:-unknown}"
    print_item ok '器件' "${device_name:-unknown}"
    print_item ok '寄存器头文件' "${register_file:-unknown}"
else
    print_item missing 'uvproj' "$sample_dir/prj/*.uvproj"
fi

printf '\n'
printf '如果 STC 资源缺失, 请按 docs/c51-cli-build.md 里的 macOS 教程手动复制 STC.CDB, STC 头文件目录, 并修改 TOOLS.INI.\n'
