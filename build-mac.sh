#!/bin/bash
# Bygger TRK ForceShot 2 för macOS och paketerar den som en .app.
# Körs PÅ EN MAC (kräver Xcode Command Line Tools och Rust):
#
#   ./build-mac.sh
#
# OBS: skriptet är skrivet men ännu inte provkört på en riktig Mac.
#
# Resultatet hamnar i dist/TRK ForceShot.app. Första gången appen tar ett skärmklipp
# frågar macOS efter behörigheten Skärminspelning (Systeminställningar > Integritet
# och säkerhet). Utan ett Apple Developer-certifikat signeras appen bara lokalt
# ("ad hoc"), och då måste behörigheten ges om efter varje nytt bygge.
set -euo pipefail
cd "$(dirname "$0")"

# Utanför Windows gäller inte sökvägen i .cargo/config.toml.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/target}"
cargo build --release

VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
APP="dist/TRK ForceShot.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$CARGO_TARGET_DIR/release/TRK-ForceShot" "$APP/Contents/MacOS/TRK-ForceShot"

# Ikon i alla storlekar macOS vill ha. (Plattan fyller hela bilden; Apples mall har lite luft runt.)
ICONSET=$(mktemp -d)/icon.iconset
mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
  sips -z $s $s assets/icon-1024.png --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
  d=$((s * 2))
  sips -z $d $d assets/icon-1024.png --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/icon.icns"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>TRK ForceShot</string>
  <key>CFBundleDisplayName</key><string>TRK ForceShot</string>
  <key>CFBundleIdentifier</key><string>se.trk.forceshot</string>
  <key>CFBundleExecutable</key><string>TRK-ForceShot</string>
  <key>CFBundleIconFile</key><string>icon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>LSMinimumSystemVersion</key><string>12.3</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>CFBundleDocumentTypes</key>
  <array>
    <dict>
      <key>CFBundleTypeName</key><string>Bild</string>
      <key>CFBundleTypeRole</key><string>Editor</string>
      <key>LSItemContentTypes</key>
      <array><string>public.png</string><string>public.jpeg</string><string>com.microsoft.bmp</string></array>
    </dict>
  </array>
</dict>
</plist>
PLIST

codesign --force --deep --sign - "$APP"
echo "Klart: $APP (version $VERSION)"
