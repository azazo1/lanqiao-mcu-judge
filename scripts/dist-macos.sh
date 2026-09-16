#!/usr/bin/env bash

# 生成 macOS 发布产物: CLI 的 tar.gz 与桌面应用的 dmg.
#
# 输入环境变量同 scripts/dist-linux.sh.

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

cli_name="stcjudge-${version}-macos-${platform_arch}${fake_suffix}.tar.gz"
app_name="stcjudge-gui-${version}-macos-${platform_arch}${fake_suffix}.dmg"
app_bundle="stcjudge GUI.app"

"$release_dir/stcjudge" --version | grep -F "$version_raw" > /dev/null
for bin in stcjudge stcjudge-gui analyze-objects; do
  test -x "$release_dir/$bin" || {
    echo "缺少可执行文件: $release_dir/$bin" >&2
    exit 1
  }
  file "$release_dir/$bin" | grep -q 'Mach-O'
done

mkdir -p "$out_dir"
staging="$(mktemp -d)"
dmg_staging="$(mktemp -d)"
mount_point="$(mktemp -d)"
cleanup() {
  hdiutil detach "$mount_point" > /dev/null 2>&1 || true
  rm -rf "$staging" "$dmg_staging" "$mount_point"
}
trap cleanup EXIT

# CLI 归档
cp "$release_dir/stcjudge" "$release_dir/analyze-objects" "$staging/"
cp LICENSE README.md "$staging/"
tar -czf "$out_dir/$cli_name" -C "$staging" .

# 应用包
bundle="$staging/$app_bundle"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
cp "$release_dir/stcjudge-gui" "$bundle/Contents/MacOS/stcjudge-gui"
cp LICENSE "$bundle/Contents/Resources/LICENSE"

iconset="$staging/AppIcon.iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" assets/app-icon.png --out "$iconset/icon_${size}x${size}.png" > /dev/null
  sips -z "$((size * 2))" "$((size * 2))" assets/app-icon.png \
    --out "$iconset/icon_${size}x${size}@2x.png" > /dev/null
done
iconutil -c icns "$iconset" -o "$bundle/Contents/Resources/AppIcon.icns"

cat > "$bundle/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>stcjudge GUI</string>
    <key>CFBundleDisplayName</key>
    <string>stcjudge GUI</string>
    <key>CFBundleIdentifier</key>
    <string>com.azazo1.stcjudge-gui</string>
    <key>CFBundleExecutable</key>
    <string>stcjudge-gui</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>${version}</string>
    <key>CFBundleVersion</key>
    <string>${version}</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.developer-tools</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
PLIST

codesign --force --sign - "$bundle/Contents/MacOS/stcjudge-gui"
codesign --force --sign - "$bundle"

cp -R "$bundle" "$dmg_staging/"
ln -s /Applications "$dmg_staging/Applications"
hdiutil create -volname "stcjudge GUI" -srcfolder "$dmg_staging" \
  -ov -format UDZO "$out_dir/$app_name" > /dev/null

hdiutil attach -nobrowse -readonly -mountpoint "$mount_point" "$out_dir/$app_name" > /dev/null
test -L "$mount_point/Applications" || {
  echo "dmg 中缺少指向 /Applications 的替身" >&2
  exit 1
}
test -d "$mount_point/$app_bundle" || {
  echo "dmg 中缺少应用包" >&2
  exit 1
}
hdiutil detach "$mount_point" > /dev/null

echo "已生成 $out_dir/$cli_name"
echo "已生成 $out_dir/$app_name"
