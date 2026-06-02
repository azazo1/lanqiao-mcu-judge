default:
    @just --list

clippy:
    cargo clippy

test:
    cargo test --release

run-sample hex script:
    cargo run --release -- run --hex {{ hex }} --script {{ script }}

run-stdin hex:
    cargo run --release -- run --hex {{ hex }} --stdin

repl hex:
    cargo run --release -- repl --hex {{ hex }}
