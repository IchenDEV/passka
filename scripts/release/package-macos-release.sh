#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VERSION="${1:?usage: package-macos-release.sh <version> <arch-label>}"
ARCH_LABEL="${2:?usage: package-macos-release.sh <version> <arch-label>}"

CLI_BINARY="$ROOT_DIR/target/release/passka"
APP_BINARY="$ROOT_DIR/app/.build/release/PasskaApp"
DIST_DIR="$ROOT_DIR/dist"
WORK_DIR="$(mktemp -d)"
CLI_STAGE="$WORK_DIR/passka-cli"
APP_STAGE="$WORK_DIR/Passka.app"

cleanup() {
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

if [[ ! -x "$CLI_BINARY" ]]; then
  echo "missing CLI binary at $CLI_BINARY" >&2
  exit 1
fi

if [[ ! -x "$APP_BINARY" ]]; then
  echo "missing app binary at $APP_BINARY" >&2
  exit 1
fi

mkdir -p "$DIST_DIR"
mkdir -p "$CLI_STAGE"
mkdir -p "$APP_STAGE/Contents/MacOS"
mkdir -p "$APP_STAGE/Contents/Resources"

cp "$CLI_BINARY" "$CLI_STAGE/passka"
chmod +x "$CLI_STAGE/passka"

cat > "$CLI_STAGE/README.txt" <<EOF
Passka CLI (${VERSION}, ${ARCH_LABEL})

Install:
  chmod +x passka
  mkdir -p "\$HOME/.local/bin"
  mv passka "\$HOME/.local/bin/passka"

Then add ~/.local/bin to PATH if needed and run:
  passka --help
EOF

cp "$APP_BINARY" "$APP_STAGE/Contents/MacOS/PasskaApp"
chmod +x "$APP_STAGE/Contents/MacOS/PasskaApp"

cat > "$APP_STAGE/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleDisplayName</key>
  <string>Passka</string>
  <key>CFBundleExecutable</key>
  <string>PasskaApp</string>
  <key>CFBundleIdentifier</key>
  <string>dev.passka.PasskaApp</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>Passka</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>${VERSION}</string>
  <key>CFBundleVersion</key>
  <string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key>
  <string>13.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
EOF

cat > "$APP_STAGE/Contents/Resources/README.txt" <<EOF
Passka macOS app (${VERSION}, ${ARCH_LABEL})

Install:
  1. Drag Passka.app into /Applications
  2. Right-click the app and choose Open the first time if macOS warns that the app is unsigned
EOF

xattr -cr "$APP_STAGE" 2>/dev/null || true

CLI_ARCHIVE="$DIST_DIR/passka-cli-${VERSION}-macos-${ARCH_LABEL}.tar.gz"
APP_ARCHIVE="$DIST_DIR/Passka-${VERSION}-macos-${ARCH_LABEL}.zip"

tar -C "$CLI_STAGE" -czf "$CLI_ARCHIVE" .
(
  cd "$WORK_DIR"
  COPYFILE_DISABLE=1 /usr/bin/zip -r -X "$APP_ARCHIVE" "Passka.app" >/dev/null
)

echo "created:"
echo "  $CLI_ARCHIVE"
echo "  $APP_ARCHIVE"
