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

# From the crate, because a version written down twice is a version that is wrong once.
VERSION="$(sed -n 's/^version *= *"\(.*\)"/\1/p' crates/sail-app/Cargo.toml | head -1)"
VERSION="${VERSION:-0.0.0}"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Sail</string>
  <key>CFBundleDisplayName</key><string>Sail</string>
  <key>CFBundleExecutable</key><string>sail</string>
  <key>CFBundleIdentifier</key><string>net.kapoost.sail</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
</dict>
</plist>
PLIST

# Signed, even though there is nobody to sign as.
#
# This is the difference between a friend opening the game and a friend being told
# **"Sail is damaged and can't be opened. You should move it to the Bin."** — which is
# what happened, and the file was not damaged in any way.
#
# Apple Silicon will not run an arm64 binary that carries *no* signature at all. Not
# "warn about it": refuse, with the only message macOS has for a binary whose signature
# does not check out, which says damaged because from the kernel's point of view that is
# indistinguishable. Intel Macs are laxer, which is why this went out looking fine.
#
# An ad-hoc signature — `--sign -`, no certificate, no account, no money — is enough to
# satisfy that. What it does not do is make Gatekeeper happy about a *downloaded*
# bundle, so the quarantine flag still has to be got past, and right-click-Open is the
# way: that is a real dialogue with a real "Open" button in it, which is what the release
# notes have always promised and what the missing signature was turning into a dead end.
#
# `--deep` because the bundle is one executable today and will not always be. `--force`
# because signing an already-signed bundle is otherwise an error, and this script is run
# twice in a row on a laptop more often than anywhere else.
codesign --force --deep --sign - "$APP"
codesign --verify --strict "$APP" && echo "signed: ad-hoc"

# The licences travel with the bundle as well as inside the binary. `--licences` prints
# them from the executable, which is what satisfies the font's terms for a single file;
# these are here so nobody has to run a command to read them.
cp assets/ATTRIBUTION.md assets/VT323-OFL.txt "$OUT/"

( cd "$OUT" && zip -qry "$NAME-macos.zip" "$NAME.app" ATTRIBUTION.md VT323-OFL.txt )
echo "wrote $OUT/$NAME-macos.zip"
ls -lh "$OUT/$NAME-macos.zip" | awk '{print $5}'
