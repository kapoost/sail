#!/usr/bin/env bash
# Build the Steam depot payloads, and upload them if there is somewhere to upload to.
#
#     dist/steam.sh            # build this machine's depot into dist/steam/content
#     dist/steam.sh --upload   # and run steamcmd, which needs the credentials below
#
# **The Steam build is a different binary from the one on the releases page**, and that is
# not a preference. Linking Steamworks rewrites the executable's loader path: a binary
# built `--features steam` will not *start* — not fail, not warn, killed by the dynamic
# loader before `main` — unless `libsteam_api` is beside it. Measured, not assumed:
#
#     dyld: Library not loaded: @loader_path/libsteam_api.dylib
#
# So the plain build goes on GitHub where somebody unzips it anywhere, and this one goes
# in the depot with the library next to it. Same tree, same commit, one flag.
#
# **Cargo does not put the library where the binary is.** It leaves it in the build
# directory it came out of, under a hash that changes, so this finds it rather than
# naming it. That is a line of shell and an hour of somebody's evening if it is missing.
#
# Three depots because three platforms, and each is built on its own kind of machine —
# there is no cross-compiling a Bevy game to macOS from Linux worth having. This script
# builds *this* machine's depot; CI runs it on all three and the upload collects them.

set -uo pipefail
cd "$(dirname "$0")/.."

OUT=dist/steam/content
APP=${SAIL_STEAM_APPID:-480}

case "$(uname -s)" in
  Darwin) PLATFORM=macos; LIB=libsteam_api.dylib ;;
  Linux)  PLATFORM=linux; LIB=libsteam_api.so ;;
  *)      PLATFORM=windows; LIB=steam_api64.dll ;;
esac

echo "building the $PLATFORM depot for app $APP"
cargo build --release -p sail-app --features steam || exit 1

BIN=target/release/sail-app
[ -x "$BIN" ] || { echo "no $BIN"; exit 1; }

# Found rather than named: the path has a build-script hash in it and the hash moves.
FOUND=$(find target/release/build -name "$LIB" -print -quit 2>/dev/null)
[ -n "$FOUND" ] || { echo "no $LIB anywhere under target/release/build — did the steam feature compile?"; exit 1; }

rm -rf "$OUT/$PLATFORM"
mkdir -p "$OUT/$PLATFORM"
cp "$BIN" "$OUT/$PLATFORM/sail"
cp "$FOUND" "$OUT/$PLATFORM/"
echo "  $OUT/$PLATFORM/sail  +  $(basename "$FOUND")"

# And the check that matters, which is not "did the files copy": a depot whose binary
# cannot find its library is a depot that installs perfectly and never opens a window.
if [ "$PLATFORM" = macos ] && command -v otool >/dev/null; then
  NEEDS=$(otool -L "$OUT/$PLATFORM/sail" | grep -c 'libsteam_api')
  [ "$NEEDS" -gt 0 ] || echo "  — warning: this binary does not link steam at all"
fi
if [ ! -f "$OUT/$PLATFORM/$LIB" ]; then
  echo "  — the library did not land next to the binary; the depot would not run"
  exit 1
fi

if [ "${1:-}" != "--upload" ]; then
  echo "built. Add --upload once there is an App ID and a build account."
  exit 0
fi

# --- The upload, which needs four things this repository does not have ----------------
#
# An App ID, three depot IDs, a Steam account with *build* permission on the app, and
# that account's password out of the environment. Until the partner account clears, this
# is the part that cannot be tested at all — so it is written to fail loudly and early rather
# than to half-run.
: "${SAIL_STEAM_APPID:?set it to the real App ID; 480 is Spacewar and has no depots of ours}"
: "${SAIL_STEAM_USER:?a Steam account with build permission on the app}"
: "${SAIL_STEAM_PASSWORD:?from the environment, never from a file in this repository}"
command -v steamcmd >/dev/null || { echo "no steamcmd on PATH"; exit 1; }

sed "s/APPID/$SAIL_STEAM_APPID/g" dist/steam/app_build.vdf > dist/steam/app_build.generated.vdf
steamcmd +login "$SAIL_STEAM_USER" "$SAIL_STEAM_PASSWORD" \
         +run_app_build "$(pwd)/dist/steam/app_build.generated.vdf" \
         +quit
