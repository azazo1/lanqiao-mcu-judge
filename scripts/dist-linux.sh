#!/usr/bin/env bash

# 生成 linux 发布产物.
#
# 输入环境变量:
# - PROJECT_BUILD_VERSION: 必填, 编译期注入的版本号, 例如 v0.1.3.
# - PROJECT_DIST_TARGET: 可选, 交叉编译目标三元组, 例如 aarch64-unknown-linux-gnu.
# - PROJECT_DIST_OUT: 可选, 产物输出目录, 默认 dist.
# - PROJECT_DIST_FAKE: 可选, 非空时产出自动更新测试构建.

set -euo pipefail

version_raw="${PROJECT_BUILD_VERSION:?未设置 PROJECT_BUILD_VERSION, 请通过 just dist 调用}"
version="${version_raw#v}"
target="${PROJECT_DIST_TARGET:-}"
out_dir="${PROJECT_DIST_OUT:-dist}"
fake_suffix=""
if [[ -n "${PROJECT_DIST_FAKE:-}" ]]; then
  fake_suffix="-fake"
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

case "$(uname -m)" in
  x86_64 | amd64) platform_arch="x86_64" ;;
  aarch64 | arm64) platform_arch="aarch64" ;;
  *)
    echo "不支持的构建架构: $(uname -m)" >&2
    exit 1
    ;;
esac

release_dir="target/release"
build_args=(--release --workspace --bins)
if [[ -n "$target" ]]; then
  release_dir="target/$target/release"
  build_args+=(--target "$target")
fi

echo "构建 $version_raw ($platform_arch), 输出到 $release_dir"
PROJECT_BUILD_VERSION="$version_raw" cargo build "${build_args[@]}"

cli_name="stcjudge-${version}-linux-${platform_arch}${fake_suffix}.tar.gz"
gui_name="stcjudge-gui-${version}-linux-${platform_arch}${fake_suffix}.tar.gz"

"$release_dir/stcjudge" --version | grep -F "$version_raw" > /dev/null
for bin in stcjudge stcjudge-gui analyze-objects; do
  test -x "$release_dir/$bin" || {
    echo "缺少可执行文件: $release_dir/$bin" >&2
    exit 1
  }
  file "$release_dir/$bin" | grep -q 'ELF'
done

mkdir -p "$out_dir"
staging="$(mktemp -d)"
trap 'rm -rf "$staging"' EXIT

cp "$release_dir/stcjudge" "$release_dir/analyze-objects" "$staging/"
cp LICENSE README.md "$staging/"
tar -czf "$out_dir/$cli_name" -C "$staging" .

rm -f "$staging"/*
cp "$release_dir/stcjudge-gui" "$staging/"
cp LICENSE "$staging/"
tar -czf "$out_dir/$gui_name" -C "$staging" .

echo "已生成 $out_dir/$cli_name"
echo "已生成 $out_dir/$gui_name"
