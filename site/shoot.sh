#!/usr/bin/env bash
# Refresh the pictures on the page by having the game photograph itself.
#
# The game can be told to start, sail for a while, take a picture and quit, which is
# what the screenshot knobs in `shot.rs` exist for. This drives that once per scene and
# then converts the results to the WebP the page actually serves — a raw frame is three
# megabytes and nobody on a phone waits for five of those.
#
# **Eighty seconds in, not eight.** The sea takes a few seconds to settle, but the thing
# worth photographing takes longer: the watch on deck has to call for the sails, trim
# them, and get her moving. A picture taken at eight seconds is a boat sitting still
# with her genoa rolled up, which is a true photograph of nothing.
#
# **It checks that each picture arrived.** Two of the first eight runs ended early with
# nothing written and nothing in the log to say so — the window had lost its place on a
# busy machine. A series that quietly produces six files out of eight looks exactly like
# a series that produced eight, and the page then ships whatever was left over from last
# time. So every scene is verified and retried once.
#
#     bash site/shoot.sh            # every scene
#     bash site/shoot.sh brig rod   # just these
#
# **The scenes behind a key are driven from inside the game**, by the schedule in
# `shot.rs`, not by making the window server type at it. That was tried first and does
# not work: macOS will post synthetic key events to the focused window and `winit` never
# reports them — measured with `h` and `F1`, both delivered, both ignored, console still
# up in the resulting photograph. Doing it inside needs no permission from the operating
# system and gives the same picture on anybody's machine.
#
# Needs the release binary (`cargo build --release -p sail-app`) and Python with Pillow
# for the conversion. It opens a window per scene; it is not a headless tool.

set -uo pipefail
cd "$(dirname "$0")/.."

BIN=target/release/sail-app
RAW=$(mktemp -d)
OUT=site/shots

[ -x "$BIN" ] || { echo "no $BIN — run: cargo build --release -p sail-app"; exit 1; }

# Scene name, then the environment that stages it. The seeds are not arbitrary: they are
# the weather each scene needs, found by looking. Nothing here is a feature of the game —
# these are development knobs, and they stay out of the page.
scene_env () {
  case $1 in
    brig)     echo "SAIL_SEED=42 SAIL_BRIG_SHOW=140 SAIL_SHOT_AFTER=80" ;;
    squall)   echo "SAIL_SEED=19 SAIL_SQUALL=70     SAIL_SHOT_AFTER=120" ;;
    gennaker) echo "SAIL_SEED=11 SAIL_HEADSAIL=gennaker SAIL_SHOT_AFTER=80" ;;
    rod)      echo "SAIL_SEED=3  SAIL_ROD=30 SAIL_HOOK=ling SAIL_FISH=20 SAIL_SHOT_AFTER=85" ;;
    drill)    echo "SAIL_SEED=5  SAIL_TUTORIAL=1   SAIL_SHOT_AFTER=70" ;;
    # Everything below here needs a key pressed, which is what the score is for.
    chart)    echo "SAIL_SEED=42 SAIL_SHOT_KEYS=70:shift+m SAIL_SHOT_AFTER=76" ;;
    # Put her somewhere at sea with no idea where, then raise the glasses and ask what
    # is in them. The plain version of this is weaker: a boat that already knows where
    # she is answers "you know where you are", which is a photograph of the wrong half
    # of the feature.
    lost)     echo "SAIL_SEED=42 SAIL_SHOT_KEYS=66:shift+l;72:shift+b;76:b SAIL_SHOT_AFTER=79" ;;
    # Three presses walks the view round to under the water, beside the lure.
    below)    echo "SAIL_SEED=3 SAIL_ROD=30 SAIL_SHOT_KEYS=70:c;71:c;72:c SAIL_SHOT_AFTER=76" ;;
    keys)     echo "SAIL_SEED=42 SAIL_SHOT_KEYS=70:f1 SAIL_SHOT_AFTER=73" ;;
    *) return 1 ;;
  esac
}

# A run of frames rather than one, for the single picture on the page that has to move.
reel_env () {
  echo "SAIL_SEED=19 SAIL_SQUALL=70 SAIL_SHOT_EVERY=0.22 SAIL_SHOT_FRAMES=30 SAIL_SHOT_AFTER=116"
}

SCENES=${*:-"brig squall gennaker rod drill chart lost below keys"}
failed=()

# `reel` is its own thing: thirty frames instead of one picture, made into a loop.
if [ "$SCENES" = "reel" ]; then
  frames=$(mktemp -d)
  echo "reel      ... this one runs for two minutes"
  # shellcheck disable=SC2086
  env $(reel_env) SAIL_SHOT="$frames/f.png" SAIL_SHOT_EXIT=1 "$BIN" >"$frames/log" 2>&1
  count=$(find "$frames" -name 'f-*.png' | wc -l | tr -d ' ')
  echo "          $count frames"
  [ "$count" -ge 20 ] || { echo "only $count frames — not enough for a loop"; exit 1; }
  python3 - "$frames" "$OUT" <<'PY'
import pathlib, sys
from PIL import Image

frames, out = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
ims = []
for p in sorted(frames.glob("f-*.png")):
    im = Image.open(p).convert("RGB")
    ims.append(im.resize((1200, round(im.height * 1200 / im.width)), Image.LANCZOS))
# Animated WebP rather than GIF: a GIF of moving water is 256 colours and about five
# times the weight, and the sea is the one thing a 256-colour palette cannot do.
dest = out / "sailing.webp"
ims[0].save(dest, "WEBP", save_all=True, append_images=ims[1:], duration=220, loop=0,
            quality=72, method=4)
print(f"  {dest}  {dest.stat().st_size/1e3:.0f} kB  {len(ims)} frames")
PY
  rm -rf "$frames"
  echo
  echo "done — rebuild with: python3 site/build.py --pdf"
  exit 0
fi

for name in $SCENES; do
  env_line=$(scene_env "$name") || { echo "unknown scene: $name"; exit 1; }
  for attempt in 1 2; do
    printf '%-9s attempt %d ... ' "$name" "$attempt"
    # shellcheck disable=SC2086
    env $env_line SAIL_SHOT="$RAW/$name.png" SAIL_SHOT_EXIT=1 "$BIN" >"$RAW/$name.log" 2>&1
    if [ -s "$RAW/$name.png" ]; then
      echo "ok"
      break
    fi
    echo "no picture written"
    [ "$attempt" = 2 ] && failed+=("$name")
  done
done

python3 - "$RAW" "$OUT" <<'PY'
import pathlib, sys
from PIL import Image

raw, out = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
out.mkdir(parents=True, exist_ok=True)
for p in sorted(raw.glob("*.png")):
    im = Image.open(p).convert("RGB")
    w, h = im.size
    # Twice the column the page gives them, which is what a retina screen asks for.
    if w > 2000:
        im = im.resize((2000, round(h * 2000 / w)), Image.LANCZOS)
    dest = out / (p.stem + ".webp")
    im.save(dest, "WEBP", quality=82, method=6)
    print(f"  {dest}  {dest.stat().st_size/1e3:.0f} kB  {im.size[0]}x{im.size[1]}")
PY

rm -rf "$RAW"

if [ ${#failed[@]} -gt 0 ]; then
  echo
  echo "these scenes produced nothing, twice: ${failed[*]}"
  echo "the page still has whatever was there before — do not call it refreshed"
  exit 1
fi
echo
echo "done — rebuild with: python3 site/build.py --pdf"
