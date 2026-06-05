default:
    @just --list

clippy:
    cargo clippy

test:
    cargo test --release
    just judge-samples

bench-run-to-callback:
    cargo test --release bench_run_to_callback_predicate -- --ignored --nocapture

# 示例: just run-sample sample/key_seg/prj/Objects/key_seg.hex sample/key_seg/judge/smoke.rhai
# 运行任意脚本文件, 需要显式给出 hex 和 script 路径.
run-sample hex script:
    cargo run --release -- run --hex {{ hex }} --script {{ script }}

run-stdin hex:
    cargo run --release -- run --hex {{ hex }} --stdin

repl hex:
    cargo run --release -- repl --hex {{ hex }}

# 示例: just judge-sample ds1302
# 示例: just judge-sample ds1302 smoke
# 示例: just judge-sample ds1302 toggle_12h.rhai
# 评测单个 sample; 单参数跑全部 judge, 指定 script 时支持省略 .rhai.
judge-sample sample script="":
    #!/usr/bin/env bash
    set -euo pipefail
    bin="target/release/stcjudge"
    sample_dir="sample/{{ sample }}"
    hex_dir="$sample_dir/prj/Objects"
    hex_candidates=()
    judges=()
    script_name="{{ script }}"
    if [ -d "$hex_dir" ]; then
        while IFS= read -r hex; do
            hex_candidates+=("$hex")
        done < <(find "$hex_dir" -maxdepth 1 -type f -name '*.hex' | sort)
    fi
    if [ -n "$script_name" ]; then
        if [[ "$script_name" != *.rhai ]]; then
            script_name="$script_name.rhai"
        fi
        judges+=("$sample_dir/judge/$script_name")
    else
        while IFS= read -r judge; do
            judges+=("$judge")
        done < <(find "$sample_dir/judge" -maxdepth 1 -type f -name '*.rhai' | sort)
    fi
    if [ "${#judges[@]}" -eq 0 ]; then
        echo "no judge scripts found in $sample_dir/judge" >&2
        exit 1
    fi
    cargo build --release --bin stcjudge
    for judge in "${judges[@]}"; do
        echo "==> $judge"
        if [ "${#hex_candidates[@]}" -eq 1 ]; then
            "$bin" run --hex "${hex_candidates[0]}" --script "$judge"
        elif [ "${#hex_candidates[@]}" -eq 0 ]; then
            echo "warning: no hex found in $hex_dir, run script without --hex" >&2
            "$bin" run --script "$judge"
        else
            echo "expected exactly one hex in $hex_dir, found ${#hex_candidates[@]}" >&2
            exit 1
        fi
    done

# 评测仓库内全部 sample 的全部 judge 脚本.
judge-samples:
    #!/usr/bin/env bash
    set -euo pipefail
    bin="target/release/stcjudge"
    cargo build --release --bin stcjudge
    while IFS= read -r judge; do
        sample_dir=$(dirname "$(dirname "$judge")")
        hex_dir="$sample_dir/prj/Objects"
        hex_candidates=()
        if [ -d "$hex_dir" ]; then
            while IFS= read -r hex; do
                hex_candidates+=("$hex")
            done < <(find "$hex_dir" -maxdepth 1 -type f -name '*.hex' | sort)
        fi
        echo "==> $judge"
        if [ "${#hex_candidates[@]}" -eq 1 ]; then
            "$bin" run --hex "${hex_candidates[0]}" --script "$judge"
        elif [ "${#hex_candidates[@]}" -eq 0 ]; then
            echo "warning: no hex found in $hex_dir, run script without --hex" >&2
            "$bin" run --script "$judge"
        else
            echo "expected exactly one hex in $hex_dir, found ${#hex_candidates[@]}" >&2
            exit 1
        fi
    done < <(find sample -type f -path '*/judge/*.rhai' | sort)

# 示例: just wave-sample ds1302
# 示例: just wave-sample ds1302 toggle_12h 0 200000000
# 为指定 sample 的 judge 脚本导出波形; script 默认 smoke, 支持省略 .rhai.
wave-sample sample script="smoke" start="0" end="" output="":
    #!/usr/bin/env bash
    set -euo pipefail
    bin="target/release/stcjudge"
    sample_dir="sample/{{ sample }}"
    script_name="{{ script }}"
    if [[ "$script_name" != *.rhai ]]; then
        script_name="$script_name.rhai"
    fi
    judge="$sample_dir/judge/$script_name"
    hex_dir="$sample_dir/prj/Objects"
    hex_candidates=()
    if [ -d "$hex_dir" ]; then
        while IFS= read -r hex; do
            hex_candidates+=("$hex")
        done < <(find "$hex_dir" -maxdepth 1 -type f -name '*.hex' | sort)
    fi
    script_name=$(basename "$judge" .rhai)
    output_path="{{ output }}"
    if [ -z "$output_path" ]; then
        output_dir="outputs/waves/{{ sample }}"
        mkdir -p "$output_dir"
        output_path="$output_dir/$script_name.html"
    else
        mkdir -p "$(dirname "$output_path")"
    fi
    cargo build --release --bin stcjudge
    cmd=("$bin" run --script "$judge" --wave-start "{{ start }}" --wave-html "$output_path")
    if [ "${#hex_candidates[@]}" -eq 1 ]; then
        cmd=("$bin" run --hex "${hex_candidates[0]}" --script "$judge" --wave-start "{{ start }}" --wave-html "$output_path")
    elif [ "${#hex_candidates[@]}" -eq 0 ]; then
        echo "warning: no hex found in $hex_dir, run script without --hex" >&2
    elif [ "${#hex_candidates[@]}" -gt 1 ]; then
        echo "expected exactly one hex in $hex_dir, found ${#hex_candidates[@]}" >&2
        exit 1
    fi
    if [ -n "{{ end }}" ]; then
        cmd+=(--wave-end "{{ end }}")
    fi
    "${cmd[@]}"
    echo "wave html: $output_path"
