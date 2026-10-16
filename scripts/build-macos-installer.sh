#!/bin/sh
set -eu
workspace="$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)"
cd "$workspace"
cargo build --release --locked --bin druckwandel-native
version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' apps/native/Cargo.toml | head -n 1)"
short_version="${version%%-*}"
bundle="artifacts/macos/bundle"
app="$bundle/Druckwandel.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/druckwandel-native "$app/Contents/MacOS/Druckwandel"
cp apps/native/assets/icon.icns "$app/Contents/Resources/icon.icns"
cp LICENSE THIRD-PARTY-NOTICES.md "$app/Contents/Resources/"
if [ ! -x resources/validators/jre/bin/java ]; then
  echo 'macOS-Prüfpaket fehlt. Zuerst python3 scripts/fetch-validators.py ausführen.' >&2
  exit 1
fi
rm -rf "$app/Contents/Resources/validators"
cp -R resources/validators "$app/Contents/Resources/validators"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Druckwandel</string>
<key>CFBundleDisplayName</key><string>Druckwandel</string>
<key>CFBundleIdentifier</key><string>de.druckwandel.desktop</string>
<key>CFBundleExecutable</key><string>Druckwandel</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$short_version</string>
<key>CFBundleVersion</key><string>$short_version</string>
<key>CFBundleIconFile</key><string>icon.icns</string>
<key>LSMinimumSystemVersion</key><string>12.0</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
codesign --force --deep --sign - "$app"
ln -sfn /Applications "$bundle/Applications"
dmg="artifacts/macos/Druckwandel-$version-$(uname -m).dmg"
hdiutil create -volname Druckwandel -srcfolder "$bundle" -ov -format UDZO "$dmg"
shasum -a 256 "$dmg" > "$dmg.sha256"
echo "$dmg"
