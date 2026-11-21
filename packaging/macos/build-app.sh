#!/bin/sh
# Wrap a (universal) fluxshot binary into FluxShot.app, then package it as a
# DMG (for people) and a zip (for the Homebrew cask).
#
#   packaging/macos/build-app.sh <binary> <version> <out-dir>
#
# Signing is controlled by the environment:
#   MACOS_SIGN_IDENTITY   Developer ID Application identity (must already be in
#                         the keychain). Unset: ad-hoc signature.
#   MACOS_NOTARY_KEY      path to an App Store Connect API key (.p8)  } all three
#   MACOS_NOTARY_KEY_ID   its key ID                                  } enable
#   MACOS_NOTARY_ISSUER   its issuer ID                               } notarization
set -eu

bin=$1
version=$2
out=$3
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
name="fluxshot_${version}_darwin_universal"

rm -rf "$out"
mkdir -p "$out"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

app="$work/FluxShot.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin" "$app/Contents/MacOS/fluxshot"
chmod 755 "$app/Contents/MacOS/fluxshot"
cp "$root/assets/fluxshot.icns" "$app/Contents/Resources/fluxshot.icns"
sed "s/@VERSION@/$version/g" "$here/Info.plist" >"$app/Contents/Info.plist"
plutil -lint "$app/Contents/Info.plist" >/dev/null

notarize() {
  if [ -n "${MACOS_NOTARY_KEY:-}" ] && [ -n "${MACOS_NOTARY_KEY_ID:-}" ] && [ -n "${MACOS_NOTARY_ISSUER:-}" ]; then
    xcrun notarytool submit "$1" --wait \
      --key "$MACOS_NOTARY_KEY" --key-id "$MACOS_NOTARY_KEY_ID" --issuer "$MACOS_NOTARY_ISSUER"
    return 0
  fi
  return 1
}

if [ -n "${MACOS_SIGN_IDENTITY:-}" ]; then
  codesign --force --timestamp --options runtime --sign "$MACOS_SIGN_IDENTITY" "$app"
else
  # Ad-hoc: satisfies Apple Silicon's "must be signed" rule, but Gatekeeper
  # still blocks it on first launch and TCC grants don't survive updates.
  codesign --force --sign - "$app"
fi
codesign --verify --strict --verbose=2 "$app"

# Notarize the app itself so the ticket can be stapled into the bundle,
# which keeps the zip (cask) and the DMG contents valid offline.
if [ -n "${MACOS_SIGN_IDENTITY:-}" ]; then
  ditto -c -k --keepParent "$app" "$work/notarize.zip"
  if notarize "$work/notarize.zip"; then
    xcrun stapler staple "$app"
  fi
fi

# Zip for the Homebrew cask. ditto is what Apple's tools expect for bundles.
ditto -c -k --keepParent "$app" "$out/${name}_app.zip"

# DMG with the usual drag-to-Applications layout.
stage="$work/dmg"
mkdir "$stage"
cp -R "$app" "$stage/"
ln -s /Applications "$stage/Applications"
hdiutil create -quiet -volname "FluxShot" -srcfolder "$stage" -fs HFS+ \
  -format UDZO -ov "$out/$name.dmg"
if [ -n "${MACOS_SIGN_IDENTITY:-}" ]; then
  codesign --force --timestamp --sign "$MACOS_SIGN_IDENTITY" "$out/$name.dmg"
  if notarize "$out/$name.dmg"; then
    xcrun stapler staple "$out/$name.dmg"
  fi
fi

ls -l "$out"
