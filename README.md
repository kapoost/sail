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

```sh
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
| Integration | Fixed 200 Hz substeps, decoupled from frame rate |

Not modelled yet, and known to be absent: waves and the boat's motion in them, tide
and current, sail-to-sail interaction beyond headsail blanketing, and any sea state at
all.

## Deliberate decisions

* **Real time, 1:1.** No time compression yet. It will have to come back — forty miles
  at five knots is eight hours — and it will collide with hands-on trimming. The answer
  is a sailor's answer (wind vane, watch-keeping, an event that drops you back to 1x),
  but that is a decision for after the proof of concept.
* **Synthetic but physical wind.** Gust statistics rather than a forecast download, so
  week one has no network dependency. The interface is already the right shape.
* **Bevy, used thin.** Chosen in week two, with working physics already in hand.

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
| helm | left stick X | A / D |
| mainsheet | right stick Y | W / S |
| genoa winch | triggers | Q / E |
| reef | D-pad up/down | R / F |
| engine | D-pad left/right | Z / X |
| snap to good trim | Y | T |
| camera | Select | C |
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
