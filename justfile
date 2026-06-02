default:
    @just --list

clippy:
    RUSTC_WRAPPER= cargo clippy

test:
    RUSTC_WRAPPER= cargo test

run-sample hex script:
    cargo run -- run --hex {{hex}} --script {{script}}

run-stdin hex:
    cargo run -- run --hex {{hex}} --stdin

repl hex:
    cargo run -- repl --hex {{hex}}
