#!/usr/bin/env bash
#
# Wrap the built binary in a macOS application bundle.
#
# A bare Unix binary is one file and a bad way to hand a game to somebody: double-clicking
# it in Finder opens a Terminal window, and a downloaded unsigned binary is quarantined
# with "cannot be opened because the developer cannot be verified" and no obvious way
# past it. A `.app` is a directory that Finder treats as a single item, it launches
# without a terminal, it can carry a name and a high-resolution flag, and the
# right-click-Open route past Gatekeeper is something Mac users have done before.
#
# Zipped at the end because that is the only way a bundle survives being sent: a .app is
# a directory, and a directory does not travel through a chat window.
#
# Both architectures when both are built, whichever one exists when only one is — so the
# same script makes the release artefact in CI and a testable bundle on a laptop that has
# only its own target installed.
set -euo pipefail

NAME="Sail"
OUT="dist/out"
APP="$OUT/$NAME.app"
ARM="target/aarch64-apple-darwin/dist/sail-app"
INTEL="target/x86_64-apple-darwin/dist/sail-app"
PLAIN="target/dist/sail-app"

# Scoped to this build's own output directory and nothing above it.
rm -rf "$OUT"
mkdir -p "$APP/Contents/MacOS"

if [ -f "$ARM" ] && [ -f "$INTEL" ]; then
  lipo -create -output "$APP/Contents/MacOS/sail" "$ARM" "$INTEL"
  echo "bundled: universal"
elif [ -f "$ARM" ]; then
  cp "$ARM" "$APP/Contents/MacOS/sail"
  echo "bundled: apple silicon only"
elif [ -f "$INTEL" ]; then
  cp "$INTEL" "$APP/Contents/MacOS/sail"
  echo "bundled: intel only"
elif [ -f "$PLAIN" ]; then
  cp "$PLAIN" "$APP/Contents/MacOS/sail"
  echo "bundled: host architecture only"
else
  echo "nothing built: run cargo build --profile dist -p sail-app first" >&2
  exit 1
fi

chmod +x "$APP/Contents/MacOS/sail"

cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Sail</string>
  <key>CFBundleDisplayName</key><string>Sail</string>
  <key>CFBundleExecutable</key><string>sail</string>
  <key>CFBundleIdentifier</key><string>net.kapoost.sail</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>0.1.0</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
</dict>
</plist>
PLIST

# The licences travel with the bundle as well as inside the binary. `--licences` prints
# them from the executable, which is what satisfies the font's terms for a single file;
# these are here so nobody has to run a command to read them.
cp assets/ATTRIBUTION.md assets/VT323-OFL.txt "$OUT/"

( cd "$OUT" && zip -qry "$NAME-macos.zip" "$NAME.app" ATTRIBUTION.md VT323-OFL.txt )
echo "wrote $OUT/$NAME-macos.zip"
ls -lh "$OUT/$NAME-macos.zip" | awk '{print $5}'
