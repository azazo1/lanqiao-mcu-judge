judge-sample sample script="":
    #!/usr/bin/env bash
    set -euo pipefail
    echo "linux 暂不支持" >&2
    exit 1

judge-samples:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "linux 暂不支持" >&2
    exit 1

wave-sample sample script="smoke" start="0" end="" output="":
    #!/usr/bin/env bash
    set -euo pipefail
    echo "linux 暂不支持" >&2
    exit 1

build-sample sample:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "linux 暂不支持" >&2
    exit 1

build-uvproj uvproj:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "linux 暂不支持" >&2
    exit 1

keil-doctor sample="arith_bench":
    #!/usr/bin/env bash
    set -euo pipefail
    echo "linux 暂不支持" >&2
    exit 1

analyze-objects sample pattern="":
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --quiet --release --bin analyze-objects
    bin="target/release/analyze-objects"
    if [ -n "{{ pattern }}" ]; then
        "$bin" "{{ sample }}" "{{ pattern }}"
    else
        "$bin" "{{ sample }}"
    fi
