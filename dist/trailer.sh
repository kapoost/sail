#!/usr/bin/env bash
# A trailer, cut from the game filming itself.
#
#     dist/trailer.sh                 # the whole cut
#     dist/trailer.sh gale night      # just those scenes, for looking at
#
# **The game already films itself**, which is the whole reason this is a shell script and
# not an afternoon with a capture card: `SAIL_SHOT_EVERY` takes a frame every so many
# seconds of *game* time and `SAIL_SHOT_KEYS` presses keys on a schedule, so a scene is a
# line of environment variables and the camera work is a key at a moment. What ffmpeg does
# here is join them.
#
# **Frames are spaced in game time, not wall clock.** A capture costs a good fraction of a
# second and the recording therefore runs at a fraction of real speed — but the world
# advances by the same amount between every frame, so the cut plays evenly. That is the
# trick that makes this work at all.
#
# **Twelve frames a second recorded, twenty-four played**, and the first number is a
# measurement rather than a taste: at twenty-four the capture cannot keep up and every
# frame comes back *black* — the screenshot is served before the picture is drawn into it.
# Twelve is the fastest rate that reliably produces a picture, so the cut records at
# twelve and ffmpeg interpolates the frames between, which is a thing it is good at on
# footage this slow.
#
# **And a breath between scenes.** Starting the next game before the last window has gone
# produces the same black frames, which cost an hour to find: two runs back to back fail
# and the same run alone is perfect, so every rate looked broken until they were tested
# one at a time.
#
# 1920 x 1080 comes out of the window: 1024 x 576 on a 2x display is 2048 x 1152, scaled
# down in the encode. The floor in `screen.rs` is 900 by 560, so anything shorter than
# that comes back taller than asked for — which is why this is not 960 by 540.
#
# No sound. The game synthesises its own — the sea, the rigging, a band — and capturing
# that needs a loopback device and somebody's permission; a silent cut with the sound laid
# under it later is the honest order to do this in.

set -uo pipefail
cd "$(dirname "$0")/.."

BIN=target/release/sail-app
OUT=dist/store
CAPTURE=12
FPS=24
[ -x "$BIN" ] || { echo "no $BIN — run: cargo build --release -p sail-app"; exit 1; }
command -v ffmpeg >/dev/null || { echo "no ffmpeg"; exit 1; }
mkdir -p "$OUT/clips"

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

# Scene, then: how long it runs, when it starts, and the environment that stages it.
# The camera is part of the staging — `c` walks the view round her (astern, at the helm,
# down the wake, under the water) and `SAIL_ORBIT` sweeps it without a hand on the mouse.
#
# **Most scenes press `h` and `m` first**, which takes the console and the chart off the
# picture. A trailer is not a screenshot: the instruments are what the game *is*, and they
# are in the pictures on the shop page where somebody is reading rather than watching —
# but sixty seconds of moving water with a panel bolted over a third of it reads as a
# screen recording. The two scenes where the instruments are the subject keep them.
scene_env () {
  case $1 in
    # Astern of her in a fresh breeze: the shot that says what the game is.
    reach)    echo "SECONDS=5 AFTER=64 SAIL_SEED=31 SAIL_WIND=18 SAIL_SHOT_KEYS=60:h;61:m" ;;
    # The same boat, from the helmsman's seat, where the deck is a place.
    helm)     echo "SECONDS=5 AFTER=66 SAIL_SEED=31 SAIL_SHOT_KEYS=60:h;61:m;64:c" ;;
    # All the way round her, which is what an orbit is for: rig, heel, wake, hull.
    orbit)    echo "SECONDS=6 AFTER=66 SAIL_SEED=31 SAIL_ORBIT=14 SAIL_SHOT_KEYS=60:h;61:m" ;;
    # Forty-odd knots and a sea already built.
    gale)     echo "SECONDS=5 AFTER=70 SAIL_SEED=31 SAIL_WIND=34 SAIL_SEA=built SAIL_SHOT_KEYS=64:h;65:m" ;;
    # The paper on the table, worked with dividers.
    chart)    echo "SECONDS=4 AFTER=68 SAIL_SEED=31 SAIL_SHOT_KEYS=66:shift+m" ;;
    # A square-rigger to lead in, a flag up, and her answer at the masthead.
    brig)     echo "SECONDS=6 AFTER=70 SAIL_SEED=31 SAIL_BRIG=600 SAIL_BRIG_SHOW=170 SAIL_HOIST=K" ;;
    # Under the water beside the lure, three presses round.
    below)    echo "SECONDS=4 AFTER=72 SAIL_SEED=3 SAIL_ROD=30 SAIL_FISH=20 SAIL_SHOT_KEYS=62:h;63:m;68:c;69:c;70:c" ;;
    # The drawn sea, which is a different game to look at.
    ink)      echo "SECONDS=4 AFTER=66 SAIL_SEED=31 SAIL_INK=1 SAIL_SHOT_KEYS=60:h;61:m" ;;
    # A moon, a path under it, and a coast with lights on it.
    night)    echo "SECONDS=6 AFTER=64 SAIL_SEED=31 SAIL_HOUR=1.6 SAIL_COVER=0.15 SAIL_SHOT_KEYS=58:h;59:m" ;;
    # Home, at the end: the lamp lit and the casks stacked.
    platform) echo "SECONDS=4 AFTER=5 SAIL_SEED=31 SAIL_PLATFORM=here SAIL_HOUR=19.4 SAIL_SHOT_KEYS=2:h;3:m" ;;
    *) return 1 ;;
  esac
}

# Did the scene actually record a picture? Count *and* brightness: a run that produces
# every frame and every one of them black looks exactly like a run that worked, which is
# the failure `site/shoot.sh` learned the expensive way and this one inherited — three
# scenes in ten came back black the first time this cut was made, and the clips were
# eleven kilobytes of nothing with perfectly good file names.
lit () {
  python3 - "$1" "$2" <<'CHECK'
import sys, glob
from PIL import Image, ImageStat
frames = sorted(glob.glob(f"{sys.argv[1]}/*.png"))
if len(frames) < int(sys.argv[2]):
    print(f"only {len(frames)} frames", end=" ")
    sys.exit(1)
dark = sum(1 for f in frames if ImageStat.Stat(Image.open(f).convert("L")).mean[0] < 4)
if dark > len(frames) // 8:
    print(f"{dark} of {len(frames)} black", end=" ")
    sys.exit(1)
sys.exit(0)
CHECK
}

ORDER="reach helm orbit gale chart brig below ink night platform"
scenes=${*:-$ORDER}
failed=0

for scene in $scenes; do
  spec=$(scene_env "$scene") || { echo "no scene called $scene"; failed=1; continue; }
  # shellcheck disable=SC2086
  eval "$spec" >/dev/null 2>&1 || true
  secs=$(echo "$spec" | sed -n 's/.*SECONDS=\([0-9.]*\).*/\1/p')
  after=$(echo "$spec" | sed -n 's/.*AFTER=\([0-9.]*\).*/\1/p')
  env_only=$(echo "$spec" | sed 's/SECONDS=[0-9.]* //; s/AFTER=[0-9.]* //')
  frames=$(python3 -c "print(int($secs * $CAPTURE))")
  raw="$STAGE/$scene"
  mkdir -p "$raw"
  printf '%-9s %s frames ' "$scene" "$frames"
  rm -f "$STAGE/Library/Application Support/sail/settings"
  # shellcheck disable=SC2086
  env HOME="$STAGE" SAIL_WINDOW=1024x576 SAIL_CREW=1 SAIL_PILOT=1 \
      SAIL_SHOT="$raw/f.png" SAIL_SHOT_EVERY=$(python3 -c "print(1/$CAPTURE)") \
      SAIL_SHOT_FRAMES="$frames" SAIL_SHOT_AFTER="$after" SAIL_SHOT_EXIT=1 \
      $env_only "$BIN" >/dev/null 2>&1
  if ! lit "$raw" "$frames"; then
    # Black frames come in runs, and a scene that fails this way fails for the whole
    # clip — the capture was serving pictures that had not been drawn yet. Worth one
    # more attempt rather than a note in a log nobody reads.
    echo "— nothing on it, going round again"
    sleep 6
    rm -rf "$raw"; mkdir -p "$raw"
    printf '%-9s again  ' "$scene"
    # shellcheck disable=SC2086
    env HOME="$STAGE" SAIL_WINDOW=1024x576 SAIL_CREW=1 SAIL_PILOT=1 \
        SAIL_SHOT="$raw/f.png" SAIL_SHOT_EVERY=$(python3 -c "print(1/$CAPTURE)") \
        SAIL_SHOT_FRAMES="$frames" SAIL_SHOT_AFTER="$after" SAIL_SHOT_EXIT=1 \
        $env_only "$BIN" >/dev/null 2>&1
    if ! lit "$raw" "$frames"; then
      echo "— still nothing"; failed=1; rm -rf "$raw"; sleep 5; continue
    fi
  fi
  # Each scene encoded on its own and the frames thrown away: a minute of this at full
  # size is two gigabytes of PNG, and keeping it all to the end is how a laptop runs out
  # of disk in the middle of a cut.
  out_frames=$((frames * FPS / CAPTURE))
  ffmpeg -y -framerate "$CAPTURE" -i "$raw/f-%03d.png" \
      -vf "scale=1920:1080:flags=lanczos,minterpolate=fps=$FPS:mi_mode=mci:mc_mode=aobmc:vsbmc=1,fade=in:0:6,fade=out:$((out_frames-7)):6" \
      -c:v libx264 -preset slow -crf 18 -pix_fmt yuv420p \
      "$OUT/clips/$scene.mp4" >/dev/null 2>&1
  rm -rf "$raw"
  echo "— $OUT/clips/$scene.mp4"
  # Let the window go before the next one opens; see the note at the top.
  sleep 5
done

# And the cut, in the order the scenes are listed above rather than the order asked for:
# the first shot has to be the one that says what the game is, and the last has to be
# home.
if [ "$failed" -eq 0 ] && [ "$#" -eq 0 ]; then
  list="$STAGE/list.txt"
  : > "$list"
  for scene in $ORDER; do echo "file '$(pwd)/$OUT/clips/$scene.mp4'" >> "$list"; done
  ffmpeg -y -f concat -safe 0 -i "$list" -c copy "$OUT/trailer.mp4" >/dev/null 2>&1
  python3 - "$OUT/trailer.mp4" <<'PY'
import subprocess, sys
out = subprocess.run(["ffprobe","-v","error","-show_entries","format=duration",
                      "-show_entries","stream=width,height","-of","csv=p=0",sys.argv[1]],
                     capture_output=True, text=True).stdout.split()
print("trailer:", " ".join(out))
PY
fi

rm -rf "$STAGE"
exit "$failed"
