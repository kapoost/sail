#!/usr/bin/env bash
# Cut the game's window out of a screen recording.
#
#     dist/crop.sh ~/Downloads/"Screen Recording 2026-10-21 at 15.37.38.mov"
#     dist/crop.sh recording.mov 4.5 94      # and trim to those seconds
#
# **The reel is filmed from outside**, because the game cannot photograph itself faster
# than about twelve frames a second — see `dist/trailer.sh` for that measurement. So the
# footage arrives as a recording of a whole screen with a window somewhere in it, a menu
# bar above it and a terminal behind it, and the first job is always the same: find the
# window and throw the rest away.
#
# **The window is found rather than typed in.** A 1280 x 720 window on a 2x display is
# 2560 x 1440 of recording and lands wherever it landed, so a hand-measured crop is right
# once and wrong the next time. This reads a frame from the middle of the recording and
# looks for the longest run of game across it — the picture is a sky and a sea and the
# desktop behind it is not — which gets the edges to the pixel. It then checks the result
# is a 16:9 shape of a plausible size, because a crop that has quietly locked onto a
# terminal window is a thing you want to be told about rather than to discover in an edit.
#
# Writes <name>-crop.mp4 beside the recording, 1920 x 1080, with the sound kept.

set -uo pipefail
cd "$(dirname "$0")/.."

IN=${1:-}
FROM=${2:-}
TO=${3:-}
[ -n "$IN" ] && [ -f "$IN" ] || { echo "usage: dist/crop.sh <recording.mov> [from] [to]"; exit 1; }
command -v ffmpeg >/dev/null || { echo "no ffmpeg"; exit 1; }
command -v ffprobe >/dev/null || { echo "no ffprobe"; exit 1; }

DUR=$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$IN")
SHOTS=$(mktemp -d)
# Four frames spread across the recording, which is what the search below needs: see the
# note in it for why one frame is not enough.
for frac in 0.25 0.40 0.55 0.70; do
  AT=$(python3 -c "print(max(0.5, float('$DUR') * $frac))")
  ffmpeg -y -ss "$AT" -i "$IN" -frames:v 1 "$SHOTS/$frac.png" -v error
done

RECT=$(python3 - "$SHOTS" <<'FIND'
import glob
import sys

import numpy as np
from PIL import Image

frames = [np.asarray(Image.open(f).convert("RGB")).astype(int) for f in sorted(glob.glob(f"{sys.argv[1]}/*.png"))]
if len(frames) < 2:
    print("could not read frames", file=sys.stderr)
    sys.exit(1)
h, w, _ = frames[0].shape

# **The window is what moves.** The first attempt at this looked for sky and sea — blue
# ahead of red — and it worked until the frame it happened to sample was a night scene,
# where it locked onto a nine-hundred-pixel patch of something else and produced a crop of
# the wrong thing that was nonetheless a valid video file. Motion is the honest test: the
# game redraws sixty times a second and a desktop does not. Compared across frames seconds
# apart, so a calm sea still counts as moving.
moving = np.zeros((h, w), dtype=bool)
for a, b in zip(frames, frames[1:]):
    moving |= np.abs(a - b).max(axis=2) > 10

# A few still pixels inside the picture — a flat horizon, a sail against an even sky —
# must not cut a run in half. Six is the number: large enough to bridge those and small
# enough not to step over a window border into whatever is behind it. At twenty-four it
# bridged a two-pixel border and took a hundred and thirty columns of scrolling terminal
# with it, which is a crop that is wrong by exactly as much as it is hard to notice.
BRIDGE = 6


def longest(mask):
    """Start and length of the longest true run, bridging gaps up to BRIDGE."""
    best = (0, 0)
    start = None
    gap = 0
    for i, v in enumerate(mask):
        if v:
            if start is None:
                start = i
            gap = 0
            if i - start + 1 > best[1]:
                best = (start, i - start + 1)
        elif start is not None:
            gap += 1
            if gap > BRIDGE:
                start = None
                gap = 0
    return best


# **Density, not runs.** A run of moving pixels walks straight out of the window and into
# whatever is behind it — in the recording this was written against, a terminal with
# scrolling log lines sat hard against the right-hand edge, and a search for the longest
# run took seventy-one columns of it every time, which is a crop wrong by exactly as much
# as it is hard to notice. A column *inside* the picture is moving nearly all the way
# down; a column of scrolling text is moving on the few rows that have glyphs on them. So
# the test is what fraction of a column moves, and the edge is where that falls off a
# cliff.
DENSE = 0.45

columns = moving.mean(axis=0)
x, cw = longest(columns > DENSE)
if cw < 320:
    print("no window found", file=sys.stderr)
    sys.exit(1)

rows = moving[:, x : x + cw].mean(axis=1)
y, ch = longest(rows > DENSE)

# The top of the picture is a hard edge against the title bar; the bottom is whatever was
# behind the window, which may itself have been moving. So trust the top and take the
# height from the width — the window is the shape the game asked for, and it asks in
# sixteen by nine.
ch = round(cw * 9 / 16)
if y + ch > h:
    ch = h - y
print(f"{cw}:{ch}:{x}:{y}")
FIND
)
rm -rf "$SHOTS"
[ -n "$RECT" ] || { echo "could not find the window in $IN"; exit 1; }

CW=${RECT%%:*}
echo "window: $RECT  (w:h:x:y)"
if [ "$CW" -lt 960 ]; then
  echo "  — that is only $CW px wide, which is under what the shop wants; check it before using it"
fi

OUT="${IN%.*}-crop.mp4"
TRIM=""
[ -n "$FROM" ] && TRIM="-ss $FROM"
[ -n "$TO" ] && TRIM="$TRIM -to $TO"
# shellcheck disable=SC2086
ffmpeg -y $TRIM -i "$IN" \
    -vf "crop=$RECT,scale=1920:1080:flags=lanczos" \
    -c:v libx264 -preset slow -crf 18 -pix_fmt yuv420p \
    -c:a aac -b:a 192k "$OUT" -v error || { echo "ffmpeg failed"; exit 1; }
ffprobe -v error -show_entries format=duration -show_entries stream=width,height -of csv=p=0 "$OUT" | tr '\n' ' '
echo "— $OUT"
