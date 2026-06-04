default:
    @just --list

clippy:
    cargo clippy

test:
    cargo test --release

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
    hex="sample/{{ sample }}/prj/Objects/{{ sample }}.hex"
    cargo build --release --bin stcjudge
    if [ -f "$hex" ]; then
        "$bin" run --hex "$hex" --script "$judge"
    else
        "$bin" run --script "$judge"
    fi

judge-samples:
    #!/usr/bin/env bash
    set -euo pipefail
    bin="target/release/stcjudge"
    cargo build --release --bin stcjudge
    while IFS= read -r judge; do
        sample_dir=$(dirname "$(dirname "$judge")")
        sample_name=$(basename "$sample_dir")
        hex="$sample_dir/prj/Objects/$sample_name.hex"
        echo "==> $judge"
        if [ -f "$hex" ]; then
            "$bin" run --hex "$hex" --script "$judge"
        else
            "$bin" run --script "$judge"
        fi
    done < <(find sample -type f -path '*/judge/*.rhai' | sort)

wave-sample sample script start="0" end="" output="":
    #!/usr/bin/env bash
    set -euo pipefail
    bin="target/release/stcjudge"
    judge="sample/{{ sample }}/judge/{{ script }}"
    hex="sample/{{ sample }}/prj/Objects/{{ sample }}.hex"
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
    if [ -f "$hex" ]; then
        cmd=("$bin" run --hex "$hex" --script "$judge" --wave-start "{{ start }}" --wave-html "$output_path")
    fi
    if [ -n "{{ end }}" ]; then
        cmd+=(--wave-end "{{ end }}")
    fi
    "${cmd[@]}"
    echo "wave html: $output_path"
