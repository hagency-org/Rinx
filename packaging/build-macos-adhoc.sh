#!/usr/bin/env bash
# Package a local-signature macOS build without Developer ID or notarization.
set -euo pipefail
cd "$(dirname "$0")/.."

version="$(cargo metadata --no-deps --format-version 1 --locked | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "rinx"))')"
case "$(uname -m)" in
  arm64) arch=aarch64 ;;
  x86_64) arch=x86_64 ;;
  *) echo "Unsupported macOS architecture" >&2; exit 1 ;;
esac

stage="$(mktemp -d "${TMPDIR:-/tmp}/rinx-dmg.XXXXXX")"
cp packaging/macos/Info.plist "$stage/Info.plist"
cleanup() {
  cp "$stage/Info.plist" packaging/macos/Info.plist
  rm -rf "$stage"
}
trap cleanup EXIT
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $version" packaging/macos/Info.plist
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $(date -u +%Y%m%d.%H%M)" packaging/macos/Info.plist

env -u APPLE_ID -u APPLE_PASSWORD -u APPLE_TEAM_ID cargo packager --release --formats app
app=dist/Rinx.app
test -x "$app/Contents/MacOS/rinx"
test -x "$app/Contents/MacOS/octos"
codesign --force --sign - "$app/Contents/MacOS/octos"
codesign --force --sign - "$app/Contents/MacOS/rinx"
codesign --force --sign - "$app"
codesign --verify --deep --strict "$app"

mkdir "$stage/image"
ditto "$app" "$stage/image/Rinx.app"
ln -s /Applications "$stage/image/Applications"
cat > "$stage/image/UNNOTARIZED.txt" <<'EOF'
This Rinx build has a local ad-hoc signature. It has no Apple Developer ID
signature or notarization ticket. macOS may require approval in System Settings
> Privacy & Security before it will open. Only approve downloads you trust.
EOF
hdiutil create -ov -format UDZO -volname "Rinx $version" \
  -srcfolder "$stage/image" "dist/Rinx_${version}_${arch}_unnotarized.dmg"
