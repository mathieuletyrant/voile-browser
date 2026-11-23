#!/usr/bin/env bash
# Build Voile.app and pack it into dist/Voile-<version>-<arch>.dmg (macOS only).
#
# Signing, from strongest to weakest:
#   MACOS_SIGN_IDENTITY="Developer ID Application: …"  → hardened runtime signature
#   + APPLE_ID, APPLE_TEAM_ID, APPLE_APP_PASSWORD        → notarized and stapled DMG
#   nothing                                             → ad-hoc signature (Gatekeeper asks
#                                                          for a right-click → Open the first time)
set -euo pipefail
cd "$(dirname "$0")/.."

[[ "$(uname)" == Darwin ]] || { echo "build-dmg.sh runs on macOS only" >&2; exit 1; }

version=$(sed -n 's/^version = "\([^"]*\)".*/\1/p' Cargo.toml | head -1)
arch=$(uname -m)
bundle_dir=target/bundle
app="$bundle_dir/Voile.app"
dmg="dist/Voile-$version-$arch.dmg"

echo "==> Building Voile $version ($arch)"
cargo run --release -p voile-bundle -- --release -o "$bundle_dir"

plist="$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Add :NSHighResolutionCapable bool true" "$plist" 2>/dev/null \
  || /usr/libexec/PlistBuddy -c "Set :NSHighResolutionCapable true" "$plist"
for key in NSCameraUsageDescription NSMicrophoneUsageDescription NSBluetoothAlwaysUsageDescription \
           NSWebBrowserPublicKeyCredentialUsageDescription; do
  /usr/libexec/PlistBuddy -c "Set :$key A website in Voile wants to use this." "$plist" 2>/dev/null || true
done

entitlements=scripts/entitlements.plist
if [[ -n "${MACOS_SIGN_IDENTITY:-}" ]]; then
  echo "==> Signing with $MACOS_SIGN_IDENTITY"
  sign() { codesign --force --timestamp --options runtime --entitlements "$entitlements" --sign "$MACOS_SIGN_IDENTITY" "$@"; }
  # Inside out: framework, helpers, then the app itself.
  framework="$app/Contents/Frameworks/Chromium Embedded Framework.framework"
  find "$framework/Libraries" -name '*.dylib' -print0 | while IFS= read -r -d '' lib; do sign "$lib"; done
  sign "$framework"
  for helper in "$app"/Contents/Frameworks/*Helper*.app; do sign "$helper"; done
  sign "$app"
else
  echo "==> Ad-hoc signature (no MACOS_SIGN_IDENTITY)"
  codesign --force --deep --sign - "$app"
fi
codesign --verify --deep --strict "$app"

echo "==> Packing $dmg"
mkdir -p dist
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
cp -R "$app" "$stage/"
ln -s /Applications "$stage/Applications"
rm -f "$dmg"
hdiutil create -volname "Voile" -srcfolder "$stage" -fs HFS+ -format UDZO -ov "$dmg" >/dev/null

if [[ -n "${MACOS_SIGN_IDENTITY:-}" ]]; then
  codesign --force --timestamp --sign "$MACOS_SIGN_IDENTITY" "$dmg"
  if [[ -n "${APPLE_ID:-}" && -n "${APPLE_TEAM_ID:-}" && -n "${APPLE_APP_PASSWORD:-}" ]]; then
    echo "==> Notarizing"
    xcrun notarytool submit "$dmg" --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" \
      --password "$APPLE_APP_PASSWORD" --wait
    xcrun stapler staple "$dmg"
  fi
fi

echo "$dmg"
