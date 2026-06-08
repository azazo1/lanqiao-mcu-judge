#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=/dev/null
. "$script_dir/path-helpers.sh"

usage() {
    cat <<'EOF' >&2
用法:
  bash scripts/build-sample-macos.sh <sample>
EOF
    exit 1
}

if [ "$#" -ne 1 ]; then
    usage
fi

sample="$1"
sample_dir="samples/$sample"
prj_dir="$sample_dir/prj"

if [ ! -d "$sample_dir" ]; then
    echo "sample 不存在: $sample_dir" >&2
    exit 1
fi

if [ ! -d "$prj_dir" ]; then
    echo "工程目录不存在: $prj_dir" >&2
    exit 1
fi

uvproj="$(find "$prj_dir" -maxdepth 1 -type f -name '*.uvproj' | sort | head -n 1)"
if [ -z "$uvproj" ]; then
    echo "未找到 uvproj: $prj_dir" >&2
    exit 1
fi

echo "==> 构建 samples/$sample"
bash "$script_dir/build-uvproj-macos.sh" "$uvproj"
