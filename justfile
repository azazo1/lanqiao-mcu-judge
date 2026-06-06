set windows-shell := ["powershell", "-NoProfile", "-Command"]

platform_justfile := if os_family() == "windows" { "just/windows.just" } else { "just/unix.just" }

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
    cargo run --release -- run --hex {{ quote(hex) }} --script {{ quote(script) }}

run-stdin hex:
    cargo run --release -- run --hex {{ quote(hex) }} --stdin

repl hex:
    cargo run --release -- repl --hex {{ quote(hex) }}

# 示例: just judge-sample ds1302
# 示例: just judge-sample ds1302 smoke
# 示例: just judge-sample ds1302 toggle_12h.rhai
# 评测单个 sample; 单参数跑全部 judge, 指定 script 时支持省略 .rhai.
judge-sample sample script="":
    @just --justfile {{ quote(platform_justfile) }} --working-directory {{ quote(justfile_directory()) }} judge-sample {{ quote(sample) }} {{ quote(script) }}

# 评测仓库内全部 sample 的全部 judge 脚本.
judge-samples:
    @just --justfile {{ quote(platform_justfile) }} --working-directory {{ quote(justfile_directory()) }} judge-samples

# 示例: just wave-sample ds1302
# 示例: just wave-sample ds1302 toggle_12h 0 200000000
# 为指定 sample 的 judge 脚本导出波形; script 默认 smoke, 支持省略 .rhai.
wave-sample sample script="smoke" start="0" end="" output="":
    @just --justfile {{ quote(platform_justfile) }} --working-directory {{ quote(justfile_directory()) }} wave-sample {{ quote(sample) }} {{ quote(script) }} {{ quote(start) }} {{ quote(end) }} {{ quote(output) }}

# 示例: just build-sample arith_bench
# Windows only. 使用 UV4 批量编译指定 sample, 自动查找 prj/*.uvproj.
build-sample sample:
    @just --justfile {{ quote(platform_justfile) }} --working-directory {{ quote(justfile_directory()) }} build-sample {{ quote(sample) }}
