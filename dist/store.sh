#!/usr/bin/env bash
# The screenshots for the shop, taken by the game itself.
#
#     dist/store.sh              # every scene
#     dist/store.sh gale chart   # just these
#
# The same trick as `site/shoot.sh` and most of the same lessons — the game is told to
# start, sail, photograph itself and quit — with three differences that are Valve's rules
# rather than ours:
#
# * **Sixteen by nine, and at least 1920 wide.** A store screenshot that is not 16:9 is
#   letterboxed into the page and looks like a mistake. `SAIL_WINDOW=1456x819` is 16:9 and
#   lands at 2912 x 1638 on a 2x display, which is over the minimum with room to spare.
#   On a 1x display it is 1456 wide and under it, which the check below catches rather
#   than letting it reach the page.
# * **PNG, kept out of the repository.** The site's pictures are WebP and committed
#   because they are served; these are uploaded once to a shop and are build output.
# * **Long enough in.** The watch has to call for the sails, trim them and get her moving:
#   a picture at eight seconds is a boat sitting still with her genoa rolled, which is a
#   true photograph of nothing. Seventy seconds is the number the site's script arrived at
#   and it is the right one here too — except where the scene is staged by a knob that
#   only holds for a moment, which is noted on the line that does it.
#
# Needs the release binary and Python with Pillow. It opens a window per scene.

set -uo pipefail
cd "$(dirname "$0")/.."

BIN=target/release/sail-app
OUT=dist/store
[ -x "$BIN" ] || { echo "no $BIN — run: cargo build --release -p sail-app"; exit 1; }
mkdir -p "$OUT"

# A home with a season's work in it, so the platform has something on it to photograph.
# Its own directory: a screenshot run must not touch the player's own save, and `HOME` is
# the whole of what decides that — see `kept.rs`.
STAGE=$(mktemp -d)
mkdir -p "$STAGE/Library/Application Support/sail"
cat > "$STAGE/Library/Application Support/sail/home" <<'SAVE'
sail home 1
sea 31
moored 1220.4 -58.3
harbour Grimvik
casks 19
fish 9
catch 21.40
built derrick
built rack
built lamp
SAVE

scene_env () {
  case $1 in
    # Forty-six knots and three and a half metres of sea. `SAIL_SEA=built` opens with the
    # sea the wind has already made rather than making us wait half an hour for it.
    gale)     echo "SAIL_SEED=31 SAIL_WIND=34 SAIL_SEA=built SAIL_SHOT_AFTER=70" ;;
    # The chart spread on the table, which takes one key.
    chart)    echo "SAIL_SEED=31 SAIL_SHOT_KEYS=66:shift+m SAIL_SHOT_AFTER=72" ;;
    # A ship to talk to, pinned where she can be seen, with K flying and her answer up.
    brig)     echo "SAIL_SEED=31 SAIL_BRIG=600 SAIL_BRIG_SHOW=170 SAIL_HOIST=K SAIL_SHOT_AFTER=70" ;;
    # The drawn sea. Nothing else changes, which is the point of the picture.
    ink)      echo "SAIL_SEED=31 SAIL_INK=1 SAIL_SHOT_AFTER=70" ;;
    # At the helm: the deck, the bags, the console — the boat as a place rather than a
    # shape on the water.
    helm)     echo "SAIL_SEED=31 SAIL_SHOT_KEYS=66:c SAIL_SHOT_AFTER=70" ;;
    # **Six seconds, not seventy**, and this one is not laziness. `SAIL_PLATFORM=here`
    # moors the raft fine on the port bow *where she is at that moment*; she then sails on
    # and it is a mile astern inside a minute. The lamp wants the half hour before
    # sunset, which is what the hour is for.
    platform) echo "SAIL_SEED=31 SAIL_PLATFORM=here SAIL_HOUR=19.4 SAIL_SHOT_AFTER=6" ;;
    *) return 1 ;;
  esac
}

# True when the capture is black, or nearly — the failure that passes `test -s`. And
# false when it is the wrong shape, which the shop rejects on upload.
checked () {
  python3 - "$1" <<'PY'
import sys
from PIL import Image, ImageStat
try:
    im = Image.open(sys.argv[1])
except Exception:
    print("unreadable"); sys.exit(1)
w, h = im.size
if ImageStat.Stat(im.convert("L")).mean[0] < 8:
    print("black"); sys.exit(1)
if abs(w / h - 16 / 9) > 0.01:
    print(f"{w}x{h} is not 16:9"); sys.exit(1)
if w < 1920:
    print(f"{w} px wide, under the 1920 the shop wants"); sys.exit(1)
print(f"{w}x{h}")
PY
}

scenes=${*:-"gale chart brig ink helm platform"}
failed=0
for scene in $scenes; do
  env=$(scene_env "$scene") || { echo "no scene called $scene"; failed=1; continue; }
  printf '%-9s ' "$scene"
  # A fresh preferences file every time: the view and the picture are remembered now, so
  # one scene that switches to the cockpit would otherwise stage every scene after it.
  rm -f "$STAGE/Library/Application Support/sail/settings"
  # shellcheck disable=SC2086
  env HOME="$STAGE" SAIL_WINDOW=1456x819 SAIL_CREW=1 SAIL_PILOT=1 \
      SAIL_SHOT="$OUT/$scene.png" SAIL_SHOT_EXIT=1 $env "$BIN" >/dev/null 2>&1
  if ! checked "$OUT/$scene.png"; then failed=1; fi
done

rm -rf "$STAGE"
[ "$failed" -eq 0 ] && echo "$OUT — ready for the shop" || echo "some scenes did not come out"
exit "$failed"
