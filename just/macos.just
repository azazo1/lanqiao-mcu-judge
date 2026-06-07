set dotenv-load := true

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

build-sample sample:
    bash scripts/build-sample-macos.sh {{ quote(sample) }}

keil-doctor sample="arith_bench":
    bash scripts/keil-env-doctor.sh {{ quote(sample) }}
