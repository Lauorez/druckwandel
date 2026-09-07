#!/bin/sh
set -eu
workspace="$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)"
cd "$workspace"

npm run demo:invoice
npx tauri build --config apps/desktop/src-tauri/tauri.conf.json --bundles dmg

mkdir -p artifacts/macos
bundle_root="${CARGO_TARGET_DIR:-apps/desktop/src-tauri/target}/release/bundle/dmg"
dmg="$(ls -t "$bundle_root"/*.dmg 2>/dev/null | head -n 1 || true)"
if [ -z "$dmg" ] || [ ! -f "$dmg" ]; then
  echo "macOS-Installer (DMG) wurde nicht erzeugt unter $bundle_root." >&2
  exit 1
fi
cp "$dmg" artifacts/macos/
cp artifacts/demo/muster-rechnung.pdf artifacts/macos/
shasum -a 256 "$dmg" | awk '{print $1}' > "artifacts/macos/$(basename "$dmg").sha256"
echo "Installer: artifacts/macos/$(basename "$dmg")"
echo "Musterrechnung: artifacts/macos/muster-rechnung.pdf"
