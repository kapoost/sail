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

Keeping those two apart is the load-bearing structural decision. Two reasons:

1. **The engine stays a reversible decision.** Bevy, Fyrox and Godot-via-gdext are all
   still on the table; the presentation layer is a few days of work on top of this
   crate, not a rewrite.
2. **The physics is testable without opening a window.** 35 tests assert against
   figures a sailor knows from the water. When one fails, the physics is lying, and you
   find out in a second instead of after twenty minutes of sailing.

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

On Windows, `Sail.exe` from the releases page.

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

`M` mutes it.

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

## Controls

Gamepad is the intended input; keyboard works. The design rule is that you map
*intentions*, not lines, and you match each control's nature to the gear it stands
for — the helm and mainsheet are positions your hand holds, the genoa sheet is a winch
you grind, and a reef is a job you do.

| | gamepad | keyboard |
|---|---|---|
| helm (held where you leave it) | left stick X | A / D |
| centre the helm | | Space |
| mainsheet: haul in / ease | right stick Y | W / S |
| genoa winch: grind / ease | triggers | E / Q |
| outhaul: ease / haul out | | 1 / 2 |
| roll the genoa away or out | X | G |
| part furl | | [ ] |
| reef | D-pad up/down | R / F |
| engine | D-pad left/right | Z / X |
| autopilot: standby / course / wind angle | Start | P |
| nudge the pilot's target | | , / . |
| fine, on helm and sheets | | Shift |
| snap to good trim | Y | T |
| camera | Select | C |
| new lap | | L |
| developer panel | | Tab |

The cockpit camera heels with the boat. That is deliberate: heel is the primary
feedback channel, and a view that stays level throws it away.

## Current polar

```
charter sloop 34 — LWL 9.20 m, disp 5500 kg, sail area 64 m2, hull speed 7.36 kn
reefed to hold heel at or below 22 deg; * marks a point that did not converge

  TWA       6kn       8kn      12kn      16kn      20kn      25kn     <- boat speed, kn
   30     2.72      3.44      4.70      5.36      5.67      6.04 
   35     3.26      4.06      5.39      5.86      6.09      6.50 
   40     3.64      4.48      5.87      6.26      6.56      6.77 
   45     3.93      4.81      6.19      6.61      6.77      6.96 
   50     4.16      5.06      6.40      6.77      6.92      7.10 
   60     4.48      5.41      6.64      7.07      7.23      7.43 
   70     4.66      5.60      6.77      7.35      7.53      7.66 
   80     4.72      5.67      6.81      7.40      7.75      7.92 
   90     4.68      5.62      6.79      7.39      7.83      8.19 
  110     4.32      5.24      6.56      7.22      7.69      8.14 
  120     4.03      4.93      6.34      7.07      7.56      8.02 
  135     3.50      4.36      5.75      6.39      6.87      7.33 
  150     2.65      3.41      4.75      5.89      6.69      7.31 
  165     2.52      3.26      4.54      5.65      6.50      7.15 
  180     2.40      3.11      4.36      5.45      6.35      7.04 
```

Best upwind VMG in 12 knots lands at 40 degrees true: 5.9 knots through the water,
20 degrees of heel, 3 degrees of leeway, apparent wind 26 degrees, carrying about
three degrees of weather helm. Those are the numbers off a real boat.
