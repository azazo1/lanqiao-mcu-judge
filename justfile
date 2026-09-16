set dotenv-load := true
set windows-shell := ["pwsh", "-NoProfile", "-Command"]

platform_justfile := if os_family() == "windows" { "scripts/just/windows.just" } else if os() == "macos" { "scripts/just/macos.just" } else { "scripts/just/linux.just" }

default:
    @just --list

clippy:
    cargo clippy -p stcjudge --all-targets
    cargo clippy -p stcjudge-gui --all-targets

test:
    cargo test --release -p stcjudge
    cargo test --release -p stcjudge-gui
    just judge-samples

alias sj := stcjudge
stcjudge *args:
    cargo run --release -p stcjudge -- {{ args }}

# 示例: just run-sample samples/key_seg/prj/Objects/key_seg.hex samples/key_seg/judge/smoke.rhai
# 运行任意脚本文件, 需要显式给出 hex 和 script 路径.
run-sample hex script:
    cargo run --release -p stcjudge -- run --hex {{ quote(hex) }} --script {{ quote(script) }}

run-stdin hex:
    cargo run --release -p stcjudge -- run --hex {{ quote(hex) }} --stdin

repl hex:
    cargo run --release -p stcjudge -- repl --hex {{ quote(hex) }}

gui:
    cargo run --release -p stcjudge-gui

# 启动隔离数据目录的调试实例, 完整日志写入隔离目录.
[unix]
debug:
    STCJUDGE_GUI_DATA_DIR=target/gui-debug STCJUDGE_GUI_LOG_FILE=target/gui-debug/app.log RUST_LOG=stcjudge_gui=trace,info cargo run -p stcjudge-gui

# 启动隔离数据目录的调试实例, 完整日志写入隔离目录.
[windows]
[script("pwsh", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File")]
debug:
    $env:STCJUDGE_GUI_DATA_DIR = 'target/gui-debug'
    $env:STCJUDGE_GUI_LOG_FILE = 'target/gui-debug/app.log'
    $env:RUST_LOG = 'stcjudge_gui=trace,info'
    cargo run -p stcjudge-gui
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

# 根据当前平台生成发布产物, 输出到 dist/.
[windows]
[script("pwsh", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File")]
dist:
    $ErrorActionPreference = 'Stop'
    $version = (& 'scripts/build-version.ps1' | Out-String).Trim()
    $env:PROJECT_BUILD_VERSION = "v$version"
    & 'scripts/dist-windows.ps1'
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

# 根据当前平台生成发布产物, 输出到 dist/.
[macos]
dist:
    PROJECT_BUILD_VERSION="v$(bash scripts/build-version.sh)" ./scripts/dist-macos.sh

# 根据当前平台生成发布产物, 输出到 dist/.
[linux]
dist:
    PROJECT_BUILD_VERSION="v$(bash scripts/build-version.sh)" ./scripts/dist-linux.sh

# 生成自动更新测试用的 fake 构建, 版本号固定为 v0.0.0, 产物名带 -fake.
[windows]
[script("pwsh", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File")]
fake-dist:
    $ErrorActionPreference = 'Stop'
    $env:PROJECT_BUILD_VERSION = 'v0.0.0'
    $env:PROJECT_DIST_FAKE = '1'
    & 'scripts/dist-windows.ps1'
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

# 生成自动更新测试用的 fake 构建, 版本号固定为 v0.0.0, 产物名带 -fake.
[macos]
fake-dist:
    PROJECT_DIST_FAKE=1 PROJECT_BUILD_VERSION=v0.0.0 ./scripts/dist-macos.sh

# 生成自动更新测试用的 fake 构建, 版本号固定为 v0.0.0, 产物名带 -fake.
[linux]
fake-dist:
    PROJECT_DIST_FAKE=1 PROJECT_BUILD_VERSION=v0.0.0 ./scripts/dist-linux.sh

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
# macOS 下使用 UV4 批量编译指定 sample, 自动查找 prj/*.uvproj.
build-sample sample:
    @just --justfile {{ quote(platform_justfile) }} --working-directory {{ quote(justfile_directory()) }} build-sample {{ quote(sample) }}

# 示例: just build-uvproj samples/arith_bench/prj/arith_bench.uvproj
# 直接构建任意 uvproj, 不假设项目位于固定目录.
build-uvproj uvproj:
    @just --justfile {{ quote(platform_justfile) }} --working-directory {{ quote(justfile_directory()) }} build-uvproj {{ quote(uvproj) }}

# 示例: just keil-doctor
# 示例: just keil-doctor arith_bench
# 检查 macOS 兼容层中的 Keil 和 STC15 资源是否齐全.
keil-doctor sample="arith_bench":
    @just --justfile {{ quote(platform_justfile) }} --working-directory {{ quote(justfile_directory()) }} keil-doctor {{ quote(sample) }}

# 示例: just analyze-objects arith_bench
# 示例: just analyze-objects arith_bench sink
# 自动分析 Keil 编译产物中适合 peek_* / poke_* 使用的固定地址符号.
analyze-objects sample pattern="":
    @just --justfile {{ quote(platform_justfile) }} --working-directory {{ quote(justfile_directory()) }} analyze-objects {{ quote(sample) }} {{ quote(pattern) }}

# 示例: just bench
# 运行 criterion 仿真基准.
bench:
    cargo bench -p stcjudge --bench sim
