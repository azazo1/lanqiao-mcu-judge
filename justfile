default:
    @just --list

clippy:
    RUSTC_WRAPPER= cargo clippy

run-sample hex script:
    cargo run -- run --hex {{hex}} --script {{script}}
