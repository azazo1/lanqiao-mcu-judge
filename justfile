default:
    @just --list

clippy:
    cargo clippy

test:
    cargo test --release
    just judge-samples

bench-run-to-callback:
    cargo test --release bench_run_to_callback_predicate -- --ignored --nocapture

run-sample hex script:
    cargo run --release -- run --hex {{ hex }} --script {{ script }}

run-stdin hex:
    cargo run --release -- run --hex {{ hex }} --stdin

repl hex:
    cargo run --release -- repl --hex {{ hex }}

judge-sample sample script="smoke.rhai":
    #!/usr/bin/env bash
    set -euo pipefail
    bin="target/release/stcjudge"
    judge="sample/{{ sample }}/judge/{{ script }}"
    sample_dir="sample/{{ sample }}"
    hex_dir="$sample_dir/prj/Objects"
    hex_candidates=()
    if [ -d "$hex_dir" ]; then
        while IFS= read -r hex; do
            hex_candidates+=("$hex")
        done < <(find "$hex_dir" -maxdepth 1 -type f -name '*.hex' | sort)
    fi
    cargo build --release --bin stcjudge
    if [ "${#hex_candidates[@]}" -eq 1 ]; then
        "$bin" run --hex "${hex_candidates[0]}" --script "$judge"
    elif [ "${#hex_candidates[@]}" -eq 0 ]; then
        echo "warning: no hex found in $hex_dir, run script without --hex" >&2
        "$bin" run --script "$judge"
    else
        echo "expected exactly one hex in $hex_dir, found ${#hex_candidates[@]}" >&2
        exit 1
    fi

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

wave-sample sample script start="0" end="" output="":
    #!/usr/bin/env bash
    set -euo pipefail
    bin="target/release/stcjudge"
    judge="sample/{{ sample }}/judge/{{ script }}"
    sample_dir="sample/{{ sample }}"
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
