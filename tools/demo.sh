#!/bin/zsh
# A demo reel, built by the game itself.
#
# Every scene here is the real simulation in a stated condition — the same environment
# knobs anybody can set — photographed by `SAIL_SHOT_EVERY` and cut together with
# ffmpeg. Nothing is staged and nothing is a mock-up: if a scene shows whitecaps in a
# gale, that gale is `SAIL_WIND=32` and the whitecaps are the ones the model made.
#
#   tools/demo.sh                 # the whole reel, into target/demo/sail-demo.mp4
#   tools/demo.sh gale light      # only the scenes named
#   FPS=12 SECONDS=5 tools/demo.sh
#
# Needs: ffmpeg, python3 with Pillow (both already used by this repository's tooling).
#
# Two keys are pressed in every scene, both of them keys anybody can press: shift-T calls
# the watch, so the sails are trimmed and drawing rather than luffing where the boat was
# started, and H hides the instruments, because what is being shown is the water. The view
# sweeps on its own where a scene asks for it — `SAIL_ORBIT` is degrees a second.
#
# **Frames are checked for being black.** A window that is occluded or behind another
# application photographs as a sheet of nothing, the file is the right size, and the
# reel comes out as a black rectangle with captions on it. So the window is raised
# before each scene and the brightness of the frames is measured after it.
set -e
# No "no matches found" for a glob that matches nothing — this script removes and lists
# files that may not exist yet, and zsh treats that as an error by default.
setopt null_glob
cd "$(dirname "$0")/.."

FPS=${FPS:-10}
SECONDS_PER_SCENE=${SECONDS:-4}
OUT=${OUT:-target/demo}
BIN=target/release/sail-app
FONT=assets/VT323-Regular.ttf

# name | caption | environment
SCENES=(
  "sails|Cloth that fills, and a watch that trims it.|SAIL_WIND=14 SAIL_SEA=built SAIL_HOUR=11 SAIL_SEED=6 SAIL_ORBIT=9|75"
  "gale|Twenty-one Gerstner components. A sea that does not repeat.|SAIL_WIND=32 SAIL_SEA=built SAIL_HOUR=12 SAIL_SEED=7 SAIL_ORBIT=5"
  "breaking|Whitecaps break along the crest, and the foam lies where it fell.|SAIL_WIND=28 SAIL_SEA=built SAIL_HOUR=14 SAIL_SEED=11"
  "light|Six knots: the ripple fades with the breeze and the water goes glassy.|SAIL_WIND=6 SAIL_SEA=built SAIL_HOUR=9 SAIL_SEED=7 SAIL_ORBIT=7"
  "dusk|The sea takes its colour from the sky that is actually up there.|SAIL_WIND=14 SAIL_SEA=built SAIL_HOUR=20.1 SAIL_SEED=3 SAIL_ORBIT=6"
  "overcast|Cloud is the weather: it fills and clears in its own order.|SAIL_WIND=18 SAIL_SEA=built SAIL_COVER=0.95 SAIL_HOUR=11 SAIL_SEED=5 SAIL_ORBIT=5"
  "squall|A squall is one object — the wind, the rain and the black thing to windward.|SAIL_WIND=20 SAIL_SEA=built SAIL_SQUALL=25 SAIL_HOUR=13 SAIL_SEED=2"
  "race|A start line, a gun and three boats that know the laylines.|SAIL_RACE=1 SAIL_WIND=12 SAIL_SEA=built SAIL_HOUR=11 SAIL_SEED=9"
  "brig|Pilotage: overhaul the brig, hoist the flag, and show her the way in.|SAIL_BRIG=1 SAIL_WIND=14 SAIL_SEA=built SAIL_HOUR=10 SAIL_SEED=4 SAIL_ORBIT=4"
)

[ -x "$BIN" ] || { echo "building the game first"; cargo build --release -p sail-app; }
[ -f "$FONT" ] || { echo "no font at $FONT"; exit 1; }
mkdir -p "$OUT"
rm -f "$OUT"/*.png "$OUT"/*.mp4 "$OUT"/list.txt

frames=$(( FPS * SECONDS_PER_SCENE ))
clips=()

for entry in "${SCENES[@]}"; do
  name="${entry%%|*}"
  rest="${entry#*|}"
  caption="${rest%%|*}"
  rest="${rest#*|}"
  env="${rest%%|*}"
  # How long to let the scene run before the camera opens. The default is enough for the
  # sky to bake and the boat to find her trim; a scene that waits on the watch to set a
  # headsail — a minute of foredeck work, at the drum's own rate — says so.
  settle="${rest#*|}"
  [ "$settle" = "$env" ] && settle=9
  if [ $# -gt 0 ]; then
    found=0
    for a in "$@"; do [ "$a" = "$name" ] && found=1; done
    [ $found -eq 1 ] || continue
  fi

  echo "== $name: $caption"
  rm -f "$OUT/$name"-*.png
  # The settle: the sky is baked over the first frames, the boat starts with way on and
  # no trim, and a scene that wants a headsail set has to wait for the foredeck.
  env $(echo $env) \
    SAIL_SHOT="$OUT/$name.png" \
    SAIL_SHOT_EVERY=$(python3 -c "print(1.0/$FPS)") \
    SAIL_SHOT_FRAMES=$frames \
    SAIL_SHOT_AFTER=$settle \
    SAIL_SHOT_EXIT=1 \
    SAIL_SHOT_KEYS="2:shift+t;6:h" \
    "$BIN" > "$OUT/$name.log" 2>&1 &
  pid=$!
  sleep 4
  osascript -e 'tell application "System Events" to tell (first process whose name contains "sail") to set frontmost to true' 2>/dev/null || true
  wait $pid || true

  shot=$(ls "$OUT/$name"-*.png 2>/dev/null | head -1)
  [ -n "$shot" ] || { echo "   no frames — skipped"; continue; }
  bright=$(python3 - "$shot" <<'PY'
import sys, statistics
from PIL import Image
print(round(statistics.mean(list(Image.open(sys.argv[1]).convert("L").getdata())), 1))
PY
)
  echo "   $(ls "$OUT/$name"-*.png | wc -l | tr -d ' ') frames, mean brightness $bright"
  if python3 -c "import sys; sys.exit(0 if float('$bright') < 8 else 1)"; then
    echo "   black frames: the window was covered. Scene dropped."
    continue
  fi

  # The caption is drawn into a strip of its own rather than by `drawtext`, because a
  # caption is a sentence and a sentence has colons, apostrophes and commas in it — every
  # one of which is punctuation inside an ffmpeg filter description. One overlay and no
  # escaping, and the font is the game's own.
  python3 tools/caption.py "$OUT/$name-caption.png" "$caption" "$FONT"
  fadeout=$(python3 -c "print(max(0.1,$SECONDS_PER_SCENE-0.5))")
  ffmpeg -v error -framerate $FPS -pattern_type glob -i "$OUT/$name-[0-9]*.png" \
    -i "$OUT/$name-caption.png" \
    -filter_complex "[0:v]scale=1280:-2[v];[v][1:v]overlay=x=(W-w)/2:y=H-h-54[o];[o]fade=t=in:st=0:d=0.4,fade=t=out:st=$fadeout:d=0.5,format=yuv420p[f]" \
    -map "[f]" -r 30 -c:v libx264 -preset slow -crf 26 "$OUT/$name.mp4" -y
  clips+=("$OUT/$name.mp4")
done

[ ${#clips[@]} -gt 0 ] || { echo "nothing to cut"; exit 1; }
for c in "${clips[@]}"; do echo "file '$(basename $c)'" >> "$OUT/list.txt"; done
( cd "$OUT" && ffmpeg -v error -f concat -safe 0 -i list.txt -c copy -movflags +faststart sail-demo.mp4 -y )
ls -lh "$OUT/sail-demo.mp4"
echo "done: $OUT/sail-demo.mp4"
