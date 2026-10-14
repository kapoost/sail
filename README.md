# sail

A sailing simulator in the spirit of a long-haul truck sim: you skipper a small
cruising sloop, and the point is the passage rather than the race.

The non-negotiable requirement is that sailing and handling the boat are modelled for
real — apparent wind, sail trim, heel and righting moment, leeway, the hull-speed
wall, rudder stall, weather helm. Not a wind arrow and a speed multiplier.

## Where things stand

`crates/sail-core` — the dynamics, and nothing else. No renderer, no input handling,
no dependency on any game engine.

`crates/sail-app` — the presentation layer, and the only crate that knows a game
engine exists. Bevy 0.20, pinned.

`crates/sail-net` — what two boats say to each other, and the relay that passes it on.
Engine-free for a stronger reason than the core is: the server has to build and run on
the smallest virtual machine anybody rents, so Bevy must never appear in its tree. The
whole protocol is about seventy bytes per boat per snapshot; see `docs/online.md`.

Keeping the first two apart is the load-bearing structural decision. Two reasons:

1. **The engine stays a reversible decision.** Bevy, Fyrox and Godot-via-gdext are all
   still on the table; the presentation layer is a few days of work on top of this
   crate, not a rewrite.
2. **The physics is testable without opening a window.** 98 tests in the core assert
   against figures a sailor knows from the water — 127 across the workspace. When one
   fails, the physics is lying, and you find out in a second instead of after twenty
   minutes of sailing.

## The core loop

```
true wind  ->  trim  ->  drive + leeway  ->  apparent wind shifts  ->  trim
```

Everything else — anchoring, the auxiliary, winches, reefing — extends that loop
rather than standing beside it. The load-bearing design decision is that sail
coefficients are a function of **angle of attack**, not of apparent wind angle: a
velocity prediction program can assume optimal trim and tabulate against wind angle
alone, but a game cannot, because trimming is the player's job. The sheet sets the
chord angle, the chord angle sets the angle of attack, and the angle of attack decides
whether the sail drives, luffs, or stalls.

## Reference boat

A production charter sloop of about 34 feet — the class most sailors have actually
steered, and can therefore judge. One boat modelled properly beats five modelled
vaguely.

## Try it

On a Mac, without building anything:

```sh
curl -fsSL https://raw.githubusercontent.com/kapoost/sail/main/dist/install.sh | sh
```

A line in a terminal rather than a download on purpose — macOS quarantines what a
*browser* saves and not what `curl` fetches, and since macOS 15 removed right-click-Open
there is otherwise no straightforward way past "Sail is damaged and can't be opened" on
an application nobody has paid Apple to vouch for. See `dist/install.sh`, which explains
itself at length.

On Windows, the same idea in PowerShell:

```powershell
irm https://raw.githubusercontent.com/kapoost/sail/main/dist/install.ps1 | iex
```

and for the same reason: *"Windows protected your PC"* comes from the mark of the web,
which the program doing the downloading attaches — a browser does, `Invoke-WebRequest`
does not. Piped into `iex` the script is a string in memory, so the execution policy has
nothing to refuse either. `Sail.exe` from the releases page still works if you would
rather click **More info** and **Run anyway**.

From source:

```sh
cargo run -p sail-app --release      # the game
cargo test -p sail-core              # the validation suite
cargo run -p sail-core --release --example polar   # the speed polar
```

The polar is the most useful instrument here. One screen says whether the physics is
sane; run it after touching `sail.rs`, `hull.rs` or `boat.rs`.

## Model, and what it approximates

| Part | Treatment |
|---|---|
| Rigid body | 4 DOF — surge, sway, roll, yaw — with added mass. Heave and pitch arrive with waves. |
| Sail forces | Hazen decomposition, with coefficients blending into flat-plate behaviour past separation |
| Frictional resistance | ITTC-57 line plus roughness allowance. Not an approximation. |
| Wave-making resistance | **Calibrated** curve shaped like the Delft series, with the hull-speed wall. The full DSYHS regression needs a dozen hull coefficients nobody publishes for a production cruiser; `residuary_resistance` is the seam where it drops in. |
| Induced resistance | Classic, with effective span below draft because of the free surface |
| Keel and rudder | Lifting-line slope with a real stall. Rudder authority goes as the square of flow speed. |
| Stability | GM slope blending into a GZ curve that peaks and gives way |
| Weather helm | Heel-shifted centre of effort plus the asymmetric heeled waterplane |
| Wind | Ornstein-Uhlenbeck gusts that veer as they build, with a boundary-layer gradient. `WindField` is where a real GRIB2 forecast replaces it. |
| Sea | Gerstner components on deep-water dispersion, heights and periods from fetch-limited relations |
| Heave and pitch | Distributed buoyancy over nine hull stations against the local surface |
| Waves on roll | The GZ curve evaluated at heel *relative to the wave slope* |
| Surfing | Froude-Krylov: displacement times g times the surface slope |
| Added resistance in waves | Charged from the power the motion damping dissipates |
| Integration | Fixed 200 Hz substeps, decoupled from frame rate |

Not modelled yet, and known to be absent: tide and current, sail-to-sail interaction
beyond headsail blanketing, and sail twist.

### One measured calibration gap, with its cause

The model's upwind VMG peaks around **29 degrees of heel**, about eight degrees past
where a real cruiser's does. `examples/outhaul.rs` is the measurement; it was written
to check whether the outhaul recommendation agreed with the model and found this
instead.

The consequence is that sail depth looks nearly free: flattening the main never pays
on pure speed until the boat physically cannot be held on course. A real boat gains
from flattening well before that. The `rail_down_*` terms in `BoatConfig` push back on
this with a steeper drag regime above 20 degrees — physically justified, since a
cruiser past that angle is burying its leeward rail and the waterplane goes badly
asymmetric — and that closed roughly half the gap. It does not close all of it, and a
harsher penalty than the rail-down term would not be defensible.

The missing mechanism is **twist**. Flattening a real mainsail takes power out of the
*top* of the sail, where the heeling arm is longest; a sail with one section and one
centre of effort can only remove force evenly. Sections, and therefore twist, are the
real fix — and the same limitation is why the outhaul here stands in for the whole
shape rather than just the lower third, and why cunningham, vang and backstay are not
separate controls.

Because the outhaul rule flattens at 20 degrees of heel — the cruising call, not the
model's fastest — it gives away up to about 0.8 percent of VMG in a breeze. That is
asserted and bounded by `the_outhaul_rule_costs_little_against_the_measured_best`
rather than left to drift.

## Sound

Synthesised, not sampled. There are no audio files in the repository and there do not
need to be: wind and water *are* filtered noise, so generating them is the honest
method rather than a shortcut. It is also the only way to get what matters here — the
sound has to answer the boat continuously, because it is a feedback channel. You hear
the rig load up before you see the heel, and you hear a sail start to shake before you
can read a telltale.

It runs as a `Decodable`, so it is a real-time generator feeding the audio thread, not
a loop being pitch-shifted. The simulation writes a handful of parameters each frame
and the audio thread reads them lock-free. No loop seams, no transposition artefacts.

| voice | driven by | note |
|---|---|---|
| wind | apparent wind speed | two-pole lowpass whose cutoff opens as it builds: louder *and* brighter |
| rigging whine | apparent wind speed | pitch from the Strouhal relation `f = St·V/d`, St ≈ 0.2 on a 6 mm shroud — 170 Hz at 5 m/s, 330 at 10 |
| water at the hull | speed through the water | bandpassed noise with a slow surge on it for the bow wave |
| flogging canvas | how badly the sails are luffing | gated bursts with a hard attack, faster in more wind |

The rigging note is gated on how *steady* the wind is, which is the difference between
wire and a theremin: vortex shedding has to lock on and cannot lock on to a flow that is
changing underneath it, so a shroud in a gust lets go, goes quiet, and finds a new note
when the gust steadies. A four-knot puff drops it nine decibels.

There is a band as well, and it is written by the boat rather than played at her. The
tempo comes from the encounter period — the rate the hull actually meets the waves at —
so the music is in the same rhythm as the sailing instead of across it. The point of
sail chooses the metre and the idiom: beating is syncopated and square, reaching gets a
walking bass, running is sparse. Each sail is a voice, and whether it is *set* decides
whether that voice exists while how hard it is *working* decides how loud it is. Past
what the boat can take — cloth over its rated wind, knocked down, a sea too big for her —
it changes language entirely, into something with a flattened second in it.

Three timbres out of the same six partials: struck for the tune, bowed for anything that
has to arrive and hold, and a dark ground for the bass. One voice doing all three was why
a change of language used to be inaudible.

`K` mutes everything. `Esc` opens the mixer, which has the sea, the boat and the band on
separate faders because they are three different things to want.

## Waves

`examples/seaway.rs` is the polar's counterpart: what sea a wind builds, how the boat
moves in it, and what it costs.

Four things are worth knowing about how this is put together, because each one was a
mistake first.

**Slopes are measured as differences over the boat's own dimensions**, not as analytic
point derivatives. A wave narrower than the beam does not heel her and a wave shorter
than the waterline does not trim her; the point slope cannot know that, and using it
summed up slopes the hull should have averaged out.

**Heave, pitch and roll are damped relative to the water surface, not to the world.**
A boat pitching exactly with a wave is following it, not radiating energy. Charging
her for that motion made a following sea cost as much as a head one.

**Surfing is the Froude-Krylov force, not gravity along the hull axis.** The first
version took the weight component from the pitch attitude, which is wrong: weight and
buoyancy are both vertical, so neither produces a horizontal force on a body in
equilibrium. Mixing a gravity-along-axis term with vertical buoyancy left them
unbalanced and turned a mean trim of three tenths of a degree into 284 N of drag from
nowhere. The real horizontal force is the wave's pressure gradient — displacement
times g times the surface slope — which depends on where the boat is in the wave
rather than on how it is trimmed, and so carries no spurious mean.

**Roll damping underway is dominated by the keel**, not by the still-water decay. A
keel rolling while moving forward sees an angle of attack and generates lift against
the roll; at five knots that is three times the figure measured at rest. It is why a
boat under way rolls so much less than the same boat drifting.

Added resistance is charged from the power the heave and pitch damping dissipate, on
the grounds that a pitching hull radiates waves and that energy has to be paid for.
The attraction of doing it this way is that it needs no sea-state formula and gets the
heading dependence for free. In a 1.3 m coastal sea the model gives up 9 percent
beating, 2 to 3 percent reaching and 7 percent dead downwind, which is about right.

Roll still uses the calibrated GZ curve rather than coming from the stations.
Replacing it would discard a matched righting-moment curve, and the whole validated
polar with it, to re-derive behaviour that is already correct. What waves add is the
one thing the curve cannot know — that the surface it stands on is tilted — and that
enters as an offset to the heel it is evaluated at.

Flat water remains the default, so the polar is unchanged to the last digit and every
flat-water test still measures what it did before.

## Fishing

She was an open fishing boat until 1964, so there is a rod in a holder on her port
quarter. It is not a minigame beside the sailing; it is bolted to it, and the join is
one fact: **a fish on the line at six knots is a fish you are going to lose.** The water
dragging a nine-kilo ling broadside at that speed pulls harder than the line will stand,
so the first thing you do when something takes is get the way off her — and the gear for
that is the mainsheet, the dump, the backed headsail and the helm you already have. The
rod has no speed control. The boat is the speed control.

Three things to work, and they match the gear they stand for the way the sheets do:

| control | nature | what it decides |
|---|---|---|
| the reel | a rate — a handle you turn | line in, line out |
| the drag | a position — a star you screw down | whether the fish tires, or the hook pulls |
| line out | a length | how deep the lure swims, and therefore what can see it |

The drag is the whole game, and it is wrong in both directions. Too tight and the hook
comes out of the fish's mouth, because load is what opens the hole it sits in. Too light
and the fish never tires at all — fatigue is charged for *work done against the line*,
so a drag you have backed off is a fish you are not beating. The rule printed on every
spool is a quarter to a third of the line's breaking strain, and that band is painted on
the dial rather than written anywhere.

The line is a spring with a slipping clutch at one end and an animal at the other, and
the ways to lose a fish fall out of that rather than being special-cased. The fish is a
force-velocity relation and not a force, which is one line of arithmetic and the
difference between something that pulls and something that is alive: held hard it swims
slowly, given slack it swims fast.

**Depth is a length of line.** A diving lure's downforce and the line's drag both go as
the square of the speed, so the angle the line takes up is nearly the same at two knots
and at six — which is what a line counter is sold on. The depth is an integral along the
line rather than a triangle, because the drag is distributed: everything near the lure
hangs steep and everything near the rod lies flat. Done as a triangle it saturates
around fifteen metres however much line you let out, which deletes the deep fish from
the game, and that is exactly what the first version did.

**Five fish, and they differ in a table rather than in code** — weight, pull per
kilogramme, burst speed, stamina, how well a hook holds, and the depth, speed and ground
they are found at. A mackerel out-pulls everything here pound for pound and tires in
twenty seconds; a pollack's whole fight is one savage dive in the first ten; a cod is
dead weight and a head-shake; a saithe runs and keeps running; a ling has to be given
line or it takes the lot. The sixth thing you can hook is the ground, which fights like a
fish with infinite patience and lets go only when the way is off her.

**Nothing jumps.** A fishing game puts a leaping fish on the box because a salmon leaps.
A mackerel does not, a pollack does not, and a cod has never left the water under its own
power in its life.

`C` now gives a third view, facing aft from the cockpit down the wake — which is worth
having without a rod in your hand, because ahead is where you are going and astern is
where your wake, your gear and anything overtaking you are. `SAIL_ROD=30` streams thirty
metres before the first frame, for the same reason `SAIL_SQUALL` exists.

## Deliberate decisions

* **Real time, 1:1.** No time compression yet. It will have to come back — forty miles
  at five knots is eight hours — and it will collide with hands-on trimming. The answer
  is a sailor's answer (wind vane, watch-keeping, an event that drops you back to 1x),
  but that is a decision for after the proof of concept.
* **Synthetic but physical wind.** Gust statistics rather than a forecast download, so
  week one has no network dependency. The interface is already the right shape.
* **Bevy, used thin.** Chosen in week two, with working physics already in hand.
* **Procedural audio.** No asset pipeline to build, nothing to license, and the sound
  tracks the simulation exactly rather than approximately.
* **Native, not the browser.** Checked twice, most recently on 14 October: the whole
  thing *compiles* for `wasm32-unknown-unknown` with no errors, so the old note about
  three hard blockers was wrong — what breaks is at run time, and the one that matters is
  that the ripple bake uses real threads. What settles it now is sailing together: a
  browser has no UDP and cannot broadcast, so the relay, the list of games on the network
  and joining by address all go, and the replacement is a certificate and a public host.
  A single-player demo in a tab is still a reasonable shop window; the game is not.
* **Nobody is authoritative over anybody's boat.** The server relays, keeps the weather
  and runs the lobby. A sailor feels fifty milliseconds on the tiller, so the helm, the
  rig and the hull stay on the machine the hands are on. See `docs/online.md`.

## Why Bevy

The question is not which engine has more features, it is what the presentation layer
has to do in the next three months. That list is: flat water, one boat, two cameras, a
gamepad, telltales, a developer panel, sound. No level design, no terrain, no harbours,
no save system, no menus.

Against that list Godot offers an editor we do not need yet and charges for it now —
an FFI boundary to debug across, two toolchains, `gdext` pinned to Godot's minor
releases, and a custom double-precision build once the water gets big. Bevy gives one
language, one build, one debugger, and shaders in WGSL, which is the same mental model
as the rest of the stack.

The decisive point: **the hardest visual, the ocean, is built into neither.** The water
shader is ours to write either way. That removes the strongest argument for a
batteries-included engine, because the batteries do not cover the expensive part.

What Bevy genuinely costs, recorded honestly rather than argued away:

* Breaking releases roughly quarterly, and it chases the newest rustc hard — 0.20
  requires 1.97.1, which forced a toolchain upgrade the day it was adopted.
* No editor. Fine now; a real nuisance once harbours need authoring.
* Coastline, terrain LOD and scene authoring are all weaker than Godot or Unreal. That
  bill arrives at production, which is past the gate this proof of concept exists to
  decide.

Both versions are pinned with `=` on purpose. An engine upgrade is scheduled work with
its own budget, not a side effect of `cargo update`.

**The condition for revisiting**: if coastline and harbour authoring becomes the
bottleneck rather than the sailing, Godot wins and we move. The move costs `sail-app`
and nothing else — which is exactly why the split is worth maintaining even when it
feels like extra structure.

## Sailing together

Two to eight friends in one archipelago. Every boat is computed by the person sailing
her and the server is a relay, a clock and a lobby — anything else would put a round trip
between somebody's hand and their rudder, and a sailor feels fifty milliseconds on the
tiller.

```sh
sail-server --seed 42          # on one machine
```

Everybody else presses `Esc` and opens the **TOGETHER** page: a game on the same network
is in the list, because an address read out over a chat window is wrong about a third of
the time. The world comes from whoever is hosting — join and you are in their
archipelago, their weather and their time of day.

Over the internet the same thing works with the host's address typed in, and the
least-work answer is one Tailscale network between friends: no port forwarding and no
code. `docs/online.md` is the whole design, including what it deliberately does not do.

## Controls

**They live in the program.** Press `?` or `F1`, or click the mark in the bottom-right
corner. This list used to be here as well, and three of the keys in it were wrong — a
key list in a README is a list that rots the first time somebody rearranges the console,
and nobody notices because the person who changed the key does not read the README.

Gamepad is the intended input; keyboard works. The design rule is that you map
*intentions*, not lines, and you match each control's nature to the gear it stands
for — the helm and mainsheet are positions your hand holds, the genoa sheet is a winch
you grind, and a reef is a job you do.

The cockpit camera heels with the boat. That is deliberate: heel is the primary
feedback channel, and a view that stays level throws it away.

## Current polar

Regenerate it with `cargo run -p sail-core --release --example polar`; this is that
output, pasted.

```
charter sloop 34 — LWL 9.20 m, disp 5500 kg, sail area 64 m2, hull speed 7.36 kn
reefed to hold heel at or below 22 deg; * marks a point that did not converge

  TWA       6kn       8kn      12kn      16kn      20kn      25kn     <- boat speed, kn
   30     1.20*     1.56*     2.74      3.10      2.87*     2.91*
   35     2.35      3.42      4.90      5.03      4.89      5.18 
   40     3.22      4.28      5.65      5.84      5.62      6.21 
   45     3.78      4.63      5.89      6.29      6.10      6.57 
   50     3.98      4.78      6.20      6.53      6.65      6.82 
   60     4.29      5.22      6.54      6.94      6.95      7.15 
   70     4.53      5.48      6.71      7.27      7.41      7.36 
   80     4.64      5.60      6.78      7.40      7.71      7.80 
   90     4.63      5.60      6.79      7.43      7.89      8.10 
  110     4.35      5.30      6.64      7.32      7.80      8.27 
  120     4.10      5.03      6.46      7.19      7.68      8.15 
  135     3.64      4.53      6.00      6.82      7.36      7.84 
  150     2.98      3.78      5.15      6.26      6.92      7.49 
  165     2.63      3.39      4.72      5.86      6.67      7.31 
  180     2.34      3.04      4.28      5.35      6.26      6.96
```

Best upwind VMG in 12 knots lands at 40 degrees true: 5.6 knots through the water, 4.1
of velocity made good, 21 degrees of heel, 3.6 degrees of leeway, apparent wind at 20
degrees. Those are the numbers off a real boat.

The thirty-degree column is a boat being pinched, and it is slow or unconverged on
purpose — no cruising sloop sails at thirty degrees true, and a polar that claimed she
did would be the first thing a sailor disbelieved.
