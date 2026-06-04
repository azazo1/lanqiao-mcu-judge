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
