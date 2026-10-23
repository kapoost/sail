#!/usr/bin/env bash
# The demo, filmed off the screen in one take.
#
#     dist/demo.sh            # the whole run, with a countdown to start recording on
#     dist/demo.sh 45         # and give me forty-five seconds to get set up
#
# **This script does not record anything.** The game plays its own five scenes — see
# `SAIL_REEL=demo` in `reel.rs`, which is the timetable and the reasoning — and a screen
# recorder films the screen. That is the right way round for a demo with sound in it: the
# game's own capture tops out at about twelve frames a second and has no audio at all,
# and a demo whose whole point is a winch, a sheet and water along the topsides cannot be
# filmed by something that cannot hear.
#
# ## What you do
#
# 1. Run this. It prints the running order and counts down.
# 2. Before the countdown ends: QuickTime -> File -> New Screen Recording, pick the game
#    window, and **turn system audio on**. In the ⌘⇧5 toolbar that is Options; if this
#    machine has no system-audio capture, install BlackHole and route the output through
#    it — the microphone setting records the room, not the game.
# 3. Leave the keyboard and the mouse alone until it says it is finished. Every beat is
#    on a clock; a hand on anything is a retake.
# 4. Cut it up with the timecodes printed at the end.
#
# ## One take, and why it can be
#
# Nothing in the run waits for anything: the sea is settled rather than built, the hour
# is moved rather than waited for, and every order that takes time — the furling drum is
# twenty-four seconds, the reef line nine — is given a shot early on purpose. The two
# ways a take can be spoiled are a blown-out sail and a knockdown, and both are the same
# mistake: canvas still up when the gale arrives. The timetable shortens her down eight
# seconds before it, and there is a test that says so.
#
# ## No voices
#
# `SAIL_VOICE` is unset below, deliberately. The voices on this machine are public-domain
# recordings standing in for the real thing, and a demo is a public document: they are
# fine on the deck and they are not fine on a shop page. If you have recorded your own,
# set it after this line and not before.

set -uo pipefail
cd "$(dirname "$0")/.."

BIN=target/release/sail-app
COUNTDOWN=${1:-40}
SEED=${SAIL_SEED:-7}
[ -x "$BIN" ] || { echo "no $BIN — run: cargo build --release -p sail-app"; exit 1; }

# See the note above. Explicit rather than assumed: this is the one line in this file
# that stops a placeholder reaching a shop page.
unset SAIL_VOICE

# A home with a season's work in it, so the platform in the last scene has something on
# it. Its own directory, because a demo run must not touch the player's own save — see
# `kept.rs`, where HOME is the whole of what decides that.
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
SAVE

cat <<ORDER

  THE DEMO — five scenes, one take, 138 seconds of material.

   0:00  night going to twilight   force 2, flat water, the reef shaking out
   0:12  the sun up                and her genoa set, within half a second of each other
   0:24  the beat to the mark      force 4, the buoy forty metres ahead, three boats racing
   0:32  the kite ordered          the genoa starts rolling away
   0:40  the bear-away             round the buoy, the camera walking round her
   0:56  the kite drawing          from the cockpit, where you would be standing
   1:06  the brig in fog           visibility about 350 m, she is 260 m off
   1:14  snugging down             the drum and the reef line, sixteen seconds before it blows
   1:30  the gale                  force 8, a 6.5 m ocean swell, running off
   1:38  from astern               the wake tilting as she lies over
   1:54  sunset at the platform    the sun down at about 1:59, she is dressed by 2:07

  The mix is set by the reel itself: the sea at a twentieth, the gear at full,
  no music. Music goes under the cut afterwards.

ORDER

printf '  starting in %s s — start the recorder now' "$COUNTDOWN"
for ((left = COUNTDOWN; left > 0; left--)); do
  printf '\r  starting in %3s s — start the recorder now   ' "$left"
  sleep 1
done
printf '\r  running — hands off until it says it is done.          \n\n'

# 1456x819 is 16:9 and lands at 2912x1638 on a 2x display, which is over Valve's
# minimum with room to spare. The same size the shop screenshots are taken at.
env HOME="$STAGE" \
    SAIL_REEL=demo SAIL_REEL_AFTER=0 \
    SAIL_SEED="$SEED" SAIL_WINDOW=1456x819 \
    SAIL_CREW=1 SAIL_PILOT=1 \
    "$BIN" 2>&1 | grep --line-buffered -E "reel:|forecast:|tide:|fog:" || true

cat <<CUT

  That is the take. Eight shots to cut out of it, each from the middle of its
  scene and clear of the moment the weather jumps:

    0:13 - 0:22   the sun out of the water, flat sea
    0:26 - 0:34   the approach, mark coming up, the fleet with her
    0:38 - 0:46   the bear-away round the buoy
    0:58 - 1:06   the gennaker drawing, from the cockpit
    1:06 - 1:14   the brig out of the fog
    1:16 - 1:26   snugging down in the fog — the sound more than the picture
    1:32 - 1:40   the gale from the chase seat
    1:42 - 1:54   the same gale from astern
    2:07 - 2:17   the platform, the afterglow on the water

  Forty-odd seconds of demo out of a hundred and thirty-eight of take. The scenes are
  in `crates/sail-app/src/reel.rs` under DEMO, with the arithmetic for why each
  one is as long as it is.

CUT
rm -rf "$STAGE"
