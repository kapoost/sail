# sail

A sailing simulator in the spirit of a long-haul truck sim: you skipper a small
cruising sloop, and the point is the passage rather than the race.

The non-negotiable requirement is that sailing and handling the boat are modelled for
real — apparent wind, sail trim, heel and righting moment, leeway, the hull-speed
wall, rudder stall, weather helm. Not a wind arrow and a speed multiplier.

**[kapoost.github.io/sail](https://kapoost.github.io/sail)** — what it looks like, how to
get it, and [the manual](https://kapoost.github.io/sail/manual.html)
([PDF](https://kapoost.github.io/sail/manual.pdf)). The page is generated from this
repository, key list included, so it cannot drift from the program; see `site/`.

**[And she sails in a tab](https://kapoost.github.io/sail/play/)**: the same game built
for the browser, published beside the page. It needs WebGPU — Chrome, Edge, or Safari 18
and up — and about fifteen megabytes down the wire. Single-handed only; a page cannot
find the game your friend is running in the next room. `dist/web.sh` builds it and
`docs/browser.md` has the measurements.

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
<!-- The two numbers in the next paragraph are read by `site/build.py`, which builds the
     website out of this file so that a page cannot disagree with the program. They have
     to be digits and they have to be in that order: "N tests in the core ... M across the
     workspace". Written in words once, and the site build went red on every push for six
     hours without anybody noticing, because nothing else depends on it. -->

2. **The physics is testable without opening a window.** 180 tests in the core assert
   against figures a sailor knows from the water — 563 across the workspace, run by CI
   on every push. When one
   fails, the physics is lying, and you find out in a second instead of after twenty
   minutes of sailing.

The third kind of test is newer and came out of a week in which every fault reported
from the deck lived in the *wiring* rather than in the arithmetic: a key read twice by a
block that had been pasted twice, a fix that asked a counter another system had not
written yet, an identification that reached the chart table but not the hand holding the
compass. None of those is findable by reading, and all of them are one assertion wide.
`scenarios.rs` runs the game headless — a Bevy `App` with `MinimalPlugins`, the resources
a system needs, and `update()` for a frame — and asks the questions those faults should
have been asked. No renderer, no window, no new dependency, four hundredths of a second.

## What it keeps

Three files, in `~/Library/Application Support/sail` and the equivalents, all of them
lines of text: a save you can read, diff and delete is one you can also recover from.

**`home`** — the sea you are sailing, where your platform is moored in it, and what is on
it. Not a passage: this game does not save at sea and is not going to, because an evening
is the length of a passage and a boat that can be put down mid-gale is a boat nothing can
go wrong to. What persists is what you brought home. The seed is in there too, so the
second launch opens on *your* islands — with a new day's weather over them, because a day
you have already sailed is a replay.

**`settings`** — volume, the view, the drawn width, the frame counter, full screen. An
environment dial still wins while it is set, and is deliberately not written back: `SAIL_FPS=1`
is somebody answering a question about one run.

**`log`** — everything the program said, written fresh each launch. A game opened by
double-clicking has no console, so this is the thing to attach to a bug report.

`Esc` stops the sea while the panel is open — unless you are sailing with other people,
who cannot be stopped by your menu. **PASSAGE → DRAW ANOTHER ARCHIPELAGO** deals a new
sea and brings the platform and the store with you.

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

In a browser, installing nothing:
[**kapoost.github.io/sail/play**](https://kapoost.github.io/sail/play/). She starts with
the watch aboard and the pilot steering, in open water, so the first minute can be spent
looking rather than rounding up — and `Esc → MISSIONS → SAILING 101` walks you round her
one control at a time. The dials this README gives as environment variables are the
query string there: `?wind=32&sea=built&hour=14`.

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
cargo run -p sail-bios --release     # the setup screen, for working on it
```

The last one is a development tool and looks like the screen that used to come up when
you held Delete: every environment knob the game reads as a row you can change with the
cursor keys, the relay's state in the corner, and start, restart and stop for it —
locally, and over ssh on the server if `SAIL_REMOTE` names one. Working on this game
means doing the same four things forty times an afternoon, and all four of them were a
shell line somebody had to remember.

`cargo test -p sail-net` is worth knowing about separately: it runs the real server
binary and two real clients over real sockets, including the door and what happens when
the server restarts underneath somebody.

The polar is the most useful instrument here. One screen says whether the physics is
sane; run it after touching `sail.rs`, `hull.rs` or `boat.rs`.

### What it needs, and what gives out first

Measured rather than guessed, on an Apple M5 at the default drawn width of 960, with
`SAIL_FRAMES=1` — which reports the frame rate and, beside it, where the processor's
share of the frame went:

| | light airs | gale, overcast, squall |
|---|---|---|
| sea | 0.04 ms | 0.12 ms |
| foam | 0.35 ms | 1.7 ms |
| cloud | 0.12 ms | 0.14 ms |
| **of a 16.7 ms frame** | **3 %** | **12 %** |

Around 370 MB resident, 50 MB of which is the ripple film — thirty-six key frames of
normal map at 512, blended on the processor so the water moves continuously rather than
in twelve steps a second. About 160 MB on disk.

**She draws two tenths of a second after the window opens.** That used to be a second and
a half, and the difference was one system: the speed polar, sixteen steady-state solves
with a reef search inside each, solved in `Startup` — before anything was on the screen —
where it cost 1.30 s. Every texture this game bakes comes to two tenths between them, so
the polar was larger than all of them put together. It is solved while she sails now, as
many angles at a time as there are cores, and nothing had to wait for it: the compass
bezel falls back to the figure every pilot book gives for a cruising sloop until the real
curve lands two frames later.

**The sea was the largest single cost and it is the card's now.** It was 2.3 ms in a gale
and 7.0 ms in the drawn sea — nearly half the frame — and it is a tenth of a millisecond
in both. What is left on the processor is not the water: it is the cloud shadow map,
rasterised once a frame for the whole square around the boat, and that is most of the
figure above.

The mesh is 201 × 201 vertices and it is written **once, at startup**. Each vertex's
position slot carries its offset from the boat and nothing else; `assets/sea_wave.wesl`
turns that offset into a piece of sea — height, slope and parcel drift out of the
twenty-one component spectrum, and on top of them the colour, the foam, the light coming
through a crest, and the drawn sea's whole treatment of tone, contour and glint. What
crosses the bus each frame is eighteen kilobytes: the sky's three bearing terms, the
cloud shadow map, the grid's own tables, and the spectrum.

**The one number a float cannot hold is worked out on this side.** A Gerstner phase is
`k·x − ωt + φ`, and after an hour at sea that is tens of thousands of radians; in single
precision it would be worth centimetres of error and the picture would crawl. So each
component's phase *at the boat* is evaluated in `f64` and handed over wrapped into a
turn, and the shader only ever adds `k·(direction·offset)` to it — an offset being a few
hundred metres. Measured against the Rust spectrum at sample points out to a kilometre
and a half, the two seas agree to about a micron.

That agreement is the cost of this change, stated plainly: **there are two
implementations of the sea now**, one in Rust that the hull floats on and one in WGSL
that the eye looks at. Nothing in a type system can hold them together, so a test does —
`sea_card`'s tests compile the shipped shader, run it on the card, and compare its height,
slope, drift and band limit against `sail-core`. They also check the drawn sea's own two
properties on the card: that a creeping tone cannot jump a band, and that a crest glows
up-sun and not down.

A tab is now a measurement rather than a theory: `dist/web.sh` builds it, and the same
scene runs in Chromium over WebGPU at 48 frames a second with the sea at 0.17 ms — the
shader is WGSL and goes over unchanged. What is slow there is the engine's own frame on
one thread, not the water. `docs/browser.md` has the table and the four things that were
in the way.

The graphics card was not the limit before and still is not: the frame rate is the same
at a drawn width of 480 and at 1920, because the world is rendered into a small image and
blown up. Taking the sea off the processor was therefore spending something the machine
had spare. **Single-core speed is still what to look at**, not the GPU — though the floor
has moved: any card with Metal 3, Vulkan 1.2 or DX12 will do, and now it has to run a
vertex shader with a twenty-one term sum in it, which every one of them can.

### Knobs, and the demo reel

Every question about what the water looks like in a given condition otherwise starts
with hunting for a seed that happens to have that condition in it, so the conditions are
knobs:

```sh
SAIL_WIND=32 SAIL_SEA=built SAIL_HOUR=14 sail      # a gale, already built, mid-afternoon
SAIL_COVER=0.95 SAIL_SQUALL=25 sail                # overcast, with a squall every 25 s
SAIL_TONEMAP=neutral sail                          # a different tone curve
SAIL_LESSON=1 sail                                 # SAILING 101, from the first frame
SAIL_CREW=1 SAIL_PILOT=1 sail                      # the watch aboard and the pilot steering
SAIL_BRIG=600 SAIL_SAY=brig sail                   # a ship wanting a pilot, and the call answered
```

In a browser the same dials are the query string: `?wind=32&sea=built&hour=14`.

`SAIL_WIND` is the day's breeze in knots, `SAIL_SEA=built` opens with the sea that wind
has already made rather than the two and a half hours it takes to build, `SAIL_COVER` is
the cloud, `SAIL_HOUR` the time of day, `SAIL_TONEMAP` the tone curve and `SAIL_ORBIT=9`
sweeps the view round her at nine degrees a second with no hand on the mouse. `sail-bios`
lists all of them as rows you can change without remembering any of this.

`tools/demo.sh` cuts a reel out of them:

```sh
tools/demo.sh                 # every scene, into target/demo/sail-demo.mp4
tools/demo.sh gale light      # only the ones named
FPS=12 SECONDS=5 tools/demo.sh
```

Each scene is the real simulation in a stated condition, photographed by the game's own
`SAIL_SHOT_EVERY` and cut together with ffmpeg; the captions are drawn in VT323, which is
what the instruments are lettered in. Nothing is staged — if a scene shows whitecaps in a
gale, that gale is `SAIL_WIND=32` and the whitecaps are the ones the model made. The
frames are checked for being black before anything is cut, because a window that is
covered photographs as a sheet of nothing and the files are the right size anyway.

## Model, and what it approximates

| Part | Treatment |
|---|---|
| Rigid body | 4 DOF — surge, sway, roll, yaw — with added mass. Heave and pitch arrive with waves. |
| Sail forces | Hazen decomposition, with coefficients blending into flat-plate behaviour past separation |
| The slot | Each sail bends the other's wind — the headsail downwashes the main, the main upwashes the headsail — scaled by how much of the sail doing the bending is actually *set*. Without that last part a main rolled up on the boom went on turning the genoa's wind by four degrees, and the boom had to be trimmed as though the main were there. |
| Reefing | Three slabs and then she is handed: full, one, two, three, down. Each slab takes a sixth of her off, and the step from the second reef to no mainsail at all is the one you need in the forties. |
| Frictional resistance | ITTC-57 line plus roughness allowance. Not an approximation. |
| Wave-making resistance | **Calibrated** curve shaped like the Delft series, with the hull-speed wall. The full DSYHS regression needs a dozen hull coefficients nobody publishes for a production cruiser; `residuary_resistance` is the seam where it drops in. |
| Induced resistance | Classic, with effective span below draft because of the free surface |
| Keel and rudder | Lifting-line slope with a real stall. Rudder authority goes as the square of flow speed. |
| Stability | GM slope blending into a GZ curve that peaks and gives way |
| Weather helm | Heel-shifted centre of effort plus the asymmetric heeled waterplane |
| Wind | Ornstein-Uhlenbeck gusts that veer as they build, with a boundary-layer gradient. `WindField` is where a real GRIB2 forecast replaces it. |
| Sea | Twenty-one Gerstner components on a Pierson-Moskowitz spectrum and deep-water dispersion, heights and periods from fetch-limited relations |
| Heave and pitch | Distributed buoyancy over nine hull stations against the local surface |
| Waves on roll | The GZ curve evaluated at heel *relative to the wave slope* |
| Surfing | Froude-Krylov: displacement times g times the surface slope |
| Added resistance in waves | Charged from the power the motion damping dissipates |
| Integration | Fixed 200 Hz substeps, decoupled from frame rate |

Not modelled yet, and known to be absent: **tide and current**. The chart draws a
wind-driven set and drift in its corner, two and a half per cent of the wind, and the
sea's own texture is carried along by it — but the hull is not, so that arrow is the one
thing on the paper the boat does not obey. Worth knowing before trusting a dead-reckoning
position.

Sail twist and sail-to-sail interaction *are* modelled now and were on this list until
recently: every sail is carried as two sections with the upper one twisted off, the lead
car and the vang are what set that twist, and a headsail works in the main's slot rather
than beside it.

### A calibration gap, and what closed most of it

The model's upwind VMG used to peak around **29 degrees of heel**, about eight degrees
past where a real cruiser's does, which made sail depth look nearly free: flattening the
main never paid on pure speed until the boat physically could not be held on course.
`examples/outhaul.rs` is the measurement, and it was written to check whether the outhaul
recommendation agreed with the model and found this instead.

Two things closed most of it. The `rail_down_*` terms in `BoatConfig` put a steeper drag
regime above 20 degrees — physically justified, since a cruiser past that angle is burying
its leeward rail and the waterplane goes badly asymmetric. And then the missing mechanism
arrived: **twist**. Flattening a real mainsail takes power out of the *top* of the sail,
where the heeling arm is longest, and a sail carried as one section with one centre of
effort can only remove force evenly. Every sail is two sections now, the upper one twisted
off by the vang and the lead car, which are controls on the bank rather than a constant.

Measured again on the thirty-four footer, best upwind VMG by outhaul at each wind's own
best course: 19 degrees of heel in 12 knots, 28 in 16, 35 in 20 with no reef in — the
first of those is where a real cruiser sits, and the others are a boat that should have
been reefed two sweeps ago and is being measured without one on purpose.

The cost of the rule is unchanged and measured in the same sweep: because the outhaul
recommendation flattens at 20 degrees of heel — the cruising call, not the model's
fastest — it gives away at most 0.045 knots of VMG, about one per cent, and less than
that everywhere except twelve knots. It used to be bounded by a test; that test is gone,
and the number is now only as fresh as the last time somebody ran the example.

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

### The spectrum, and the sea it reports

The wind sea is twenty-one Gerstner components on a Pierson-Moskowitz spectrum, spread
about the wind by the Mitsuyasu rule — widest away from the peak, widest of all on the
short side. It used to be five, and five is not a sea: it is an interference pattern,
and an interference pattern repeats. Measured, the old surface correlated 0.92 with the
surface 1164 m downwind. The present one scores 0.50 beyond 250 m across three seeds, and
`tests/the_sea_repeats.rs` holds it there — the frequencies are spaced geometrically with
each rung jittered inside its own band, and the directions are placed by the golden angle,
so no two components share a beat.

**A sea that says a metre is a metre.** Significant height is four times the rms
elevation and the rms of a sum of sinusoids is the quadrature sum of the amplitudes over
root two. Both sentences were in the comment and only the first was in the arithmetic, so
every sea was 41 percent higher than the instruments reported. Fixing it moved two more
things that were written as fractions of Hs and therefore moved with it: the elevation a
whitecap needs, and the weight given to a crest. Both are in standard deviations now,
which do not move.

**The hull gets the slope a hull feels**, not the slope at a point: each component
attenuated over her draft and averaged over her length and beam, `sinc(k·L/2)`. A
one-metre ripple under an eleven-metre boat has a gradient and no force. Before this the
self-steering vane wandered 43 degrees in sheltered water and had the helm at its stop 7
percent of the time; it is 1.9 degrees and never.

**The drawn surface is Gerstner too.** The mesh used to move its vertices only up and
down, which is Airy — round humps of exactly the right height and none of the shape.
Parcels move sideways now, which gathers the water into the crests and flattens the
troughs, and the foam goes with them.

### Whitecaps

Breaking is judged by significant steepness — `Hs / λp` — because it has to be: a
four-metre Atlantic swell does not break and a metre and a half of short wind sea breaks
everywhere, and the same number tells you which. A whitecap is drawn as nine marks laid
along the crest line, over a length that follows the sea, which is what a breaking crest
looks like; a circle of marks is a white dot however many are in it. The foam arrives
bright and hard and then lies there fading, because that is what foam does.

Everything on the water follows the sea state rather than a constant: the ripple normal
map fades towards flat as the breeze drops, the surface roughness follows it, and the
jade of light through a wave needs half a metre of wave before it appears. A nine-knot
morning used to be drawn with the texture of a gale.

### The sky, the cloud and the colour of the water

The water's colour is the sky it reflects. A trough shows the strip overhead and a crest
shows the dome, so the dark end is the zenith and the light end the horizon — both taken
from the `Sky` the scene is lit by rather than from two constants, and the light end is a
bearing: towards a low sun the sea is a sheet of light and over your shoulder it is
nearly black. `Dome` fits twenty-four samples round the horizon to a constant and a
direction, rebuilt when the sky is repainted.

Low cloud is drawn once. It is particles out to seven and a half kilometres — parallax,
shadows thrown down-sun onto the water, shafts between the cloud and its shadow — and the
cubemap's painted deck fades in from six kilometres, so the near sky belongs to the
geometry and the far sky to the painting. The particles follow the cover: each cell has a
rank and is there once the day has reached it, and what is there grows and flattens as
the cover rises, because fair-weather cumulus are separate lumps and an overcast is the
same cloud joined up.

## Filming it

`SAIL_REEL=1` and the game plays itself: a timetable of scenes — a breeze astern of her,
a sweep all the way round, the helmsman's seat, the chart on the table, a gale, the drawn
sea, evening over the platform, a flag at the masthead, and a night with a moon on the
water. About a minute and three quarters, each scene long enough to take six seconds out
of the middle of.

It exists because the game's own screenshot dials cannot film at speed: past about twelve
frames a second the capture is served a picture that has not been drawn yet and every
frame comes back **black**. So the reel is the other way round — something else films the
screen at sixty frames with the sound, and the game's job is to put on the show without a
hand on the keyboard. Every beat writes the same resource the settings panel writes;
anything the reel can show, a player can do.

`SAIL_REEL=short` is the other one: ninety-three seconds of nothing but a boat sailing —
sunrise, the wardrobe changing as she bears away, the breeze making, a reef, sunset,
moonrise, night. No panels and no story. The shape of it is set by three measurements and
not by taste: a sail change takes twenty-four seconds at the drum's pace, a kite has to
come in before the wind arrives rather than after, and the sun here rises at 05:29.

`dist/store.sh` takes the shop's stills and `dist/capsules.py` composes its capsules from
one of them. `dist/trailer.sh` cuts a trailer out of the game filming itself, at the
twelve frames a second that works, for when there is nobody to press record.

## Night

**One passage in five starts in the dark**, somewhere between ten at night and three in
the morning, and the rest of them can sail into it. It used to be morning always, on the
reasoning that a delivery leg starts at a civilised hour — and that night was a different
game needing instrument lights, navigation lights and a sea you cannot read. All three of
those exist now.

**A moon at the antisolar point**, which is where a full moon always is: that is not a
simplification dressed up as physics, it is the same fact twice, and what falls out of it
is a moon that rises as the sun sets, stands highest at midnight and sets at dawn. It
throws a path of broken light across the water that moves as she moves, and it is the one
directional light in the scene once the sun is down — cold, because an eye adapted to a
quarter of a lux reports blue. The phase is the thing given up, and deliberately: a phase
needs a calendar, and this game has no calendar.

**Stars**, as a function of direction rather than a catalogue, so the baked dome gives the
same answer every time and nothing crawls. Bright ones only: the sky is painted into a
cubemap at 256 pixels a face, a third of a degree to the texel, and a star is a hundredth
of that — so the smallest point this sky can draw is already twenty times too big, and a
thousand of them is not a starry night, it is snow. A few hundred, each worth looking at.

**Her own lamps and the coast's.** She carries side lights in boxes with a lens in one
face, because the box is what makes a sector and nobody should see green from your port
bow; the marks flash their real characteristics, which is what identifies a light in the
dark and the only thing that does.

**And the chart goes under a dim lamp**, because a white chart at night is a flashbang:
twenty minutes of dark adaptation gone in a second, and no coast out there any more. The
paper dims and warms; the console keeps its own backlight, which is what a lit binnacle
does.

## Navigation

The archipelago has names on it, and finding out which bit of it you are looking at is a
game in its own right — the oldest one there is, and the one a chart plotter deleted.

**The chart.** `M` puts it in the corner, `Shift M` spreads it on the table. The corner
one is a glance: where am I, what is ahead, and it stays under the boat. The one on the
table is an instrument, and it has the instruments a chart table has — dividers for
distance and bearing, a plain ruler for a leg with two ends, a parallel rule that plots a
position line through a mark, a protractor, a pencil, a stopwatch with a bell at the end
of the minute, a hand log read against the glass, and a rubber. The coast on it is read off the same signed-distance field the boat runs
aground on, so the chart cannot lie about the depths: there is only one set of them.

**The instruments are in your hands, and you get one hand.** `Shift B` is a pair of
glasses and `Shift N` a hand bearing compass, and picking one up puts the other down —
seven-by-fifties are two-handed on a moving boat and a compass is held out at arm's
length. Either of them puts you in the cockpit, because there are no instruments fifteen
metres astern of her, and gives the view back when it goes down. Spreading the chart puts
both away: you work on the paper or you work on the horizon.

**Seeing a thing and knowing which thing it is are different questions**, and the second
one is the whole difficulty. A lighthouse is painted to be told apart and lit; a beacon
has to be close enough for its topmark to be read, or it is a stick. The glasses roughly
double the range at which a mark can be *named*, which is what they are for, and `B` asks
the question: a mark near enough to read its symbol names itself, and three well spread
apart are a shape that belongs in one place on the coast and nowhere else — worked out
the way a station pointer does it, from the two horizontal angles between them.

**A bearing is a line, not a position.** The compass has a notch about a degree and a
quarter wide, which is what a notch at arm's length is, and holding it on a mark from a
moving deck is the small act of steadiness the instrument exists to need. It answers
either way: the name and the number if it caught, and how far off you were if it did not.
The number carries the error a card in a bowl actually has — two or three degrees in a
seaway — which is why a fix is a triangle rather than a point, why a fine cut is a bad
fix, and why the third line is worth taking.

**The book, and the paper.** A chart is not where a navigator writes things down — the
book is. Every bearing, every mark made out through the glasses, every reading off the
log goes on the page beside the paper with the time against it, and the line goes on the
chart as it is taken. What the book is *for* is the record and the rubber: one press
takes a doubtful line off the paper and another puts it back where it now belongs,
because the usual reason a fix will not close is a line you should not have believed.

**One mark, two bearings and the run between them are a position.** A position line is a
statement about where she was when the bearing was taken, and she does not stay there —
so every line carries the distance she has run through the water since it was shot and is
drawn through the mark *plus that run*. Fresh, it passes through the mark; twenty minutes
later it is a hand's breadth off it, dashed, with the run drawn in from the mark. That is
a transferred position line, and crossing it with a fresh bearing is the running fix: the
oldest trick on a coast with a single light. The transfer is through the water, because
the compass and the log are what she has, which is also its honest limit — a knot of tide
nobody has measured goes into the transfer and comes out as position.

**Two buttons, because marking the paper and writing the book are two acts.** ON THE
CHART puts the position on the paper and is dead until two lines cross, which is the only
thing it can mean. IN THE LOG writes the line and is live always: most entries in a deck
log have no fix in them at all — a time, a course, the glass, the sounding, and what was
set, drawn as a sail plan with a bar across the foot for every reef in her.

**The reckoning is a sequence of courses, not a line**, and the plot is as old as your
last visit to the chart table. She knows perfectly well that she tacked — the compass is
on the bulkhead — but the *paper* does not, so it goes on laying off the course it was
last told with the miles off the log, and the pencil position walks away from her.
Spreading the chart writes the log up: every alteration since you were last there laid
off in order, the corners appearing where they belong. Ten minutes of unwritten log after
a ninety-degree tack is a mile and a half of pencil error, measured.

**WHY**, on the end of the row of instruments, makes every line on the paper say what it
is and why it is there — position line, transferred, ruled by hand, log leg, reckoning,
track made good. A plot is six kinds of line in two colours with no legend, and telling
them apart is most of the subject.

**`Shift Backspace` puts you somewhere at sea with no idea where.** The sails are set for the
wind she is in and nothing else is made difficult; the chart stops following her, the
names come off the marks, and getting them back is the exercise. Three bearings, crossed,
entered in the log — and the scoring is the honest one: a fix is good if it is near where
she actually is, not if the cocked hat was small. A tight triangle on two misidentified
marks is a confident lie and the classic way to put a boat on a rock.

## The instruments

Rebuilt in a day against four words from the deck — analogue, engineering typography,
aviation's way with information, and 1900s — and the result is a case rather than a
screen.

**A palette of materials.** Japanned black for the case, brass for bezels and for
whatever is live, ivory for dial faces and for the nameplates with the names engraved on
them, vermilion for a pointer. Every surface on a boat of this age is a material and
materials have a temperature; four greys do not. Four type sizes, each with a job, so
that the figures you act on are not *found* among the ones you look up.

**One glass, in the corner, in every view.** The top half is the compass card as you see
it looking down into the bowl, with a window at the lubber line; the bottom half is the
sea, cut as a segment of the same glass so that however far she lies over the water stays
inside the bowl; the boss in the middle is the wind she is actually in, apparent on the
needle with the true behind it.

**And the bezel is the part that is navigation.** On it: where the wind is from, the
shaded water she cannot sail into, and the two headings that bound it — which is what a
beat consists of. *Can I lay it on this tack* is read straight off the ring instead of
being worked out from three numbers, and the angle comes from her own polar rather than
from a book, so it narrows when she is reefed and opens when she is footing.

**Each view gets the tools its job needs.** At the helm you are working the sheets, the
preventer and the pilot, and the rack of seven control lines is pinned in the middle,
because seven lines on one winch is cockpit work. From astern you are watching her sail
and thinking about what is set. Under the water you are fishing, and there is nothing on
this boat you could adjust from down there. A bar that showed everything everywhere
showed the wrong thing three times out of four.

**And three buttons for the things you pick up.** Everything else on the console is a
reading or a rope; the glasses, the hand bearing compass and the rod are neither — they
are in your hands, and a hand is the one thing a keyboard hides. `GLASSES`, `BEARING`
and `ROD` sit under the weather and light when the thing is in use. They do the half
that was undiscoverable — picking it up and putting it down — and leave naming a mark,
shooting a bearing and taking a fish on the keys, because those three are timing.

**Nothing is printed beside the drawing of itself.** Her head is the card, the apparent
wind is the needle, her heel is the line across the bottom half of the bowl — so those
three figures are not also set in type next to them, which is the oldest way to make an
instrument panel unreadable. What is printed is everything the glass cannot show: the
log, the strength of both winds, what she is making good, the water under her, and what
the barometer is doing.

**She can photograph herself**, on `F12` or on a timer:

```sh
SAIL_SHOT=/tmp/deck.png SAIL_SHOT_AFTER=3 SAIL_SHOT_EXIT=1 sail
```

Nothing about a head-up display can be asserted in a test — it is read by eye or it is
not read at all — so the next best thing is to make looking cheap. That loop is how the
panel above got rebuilt, and it paid for itself in the first hour by finding a clinometer
drawing a green line across the whole console.

## Anchoring

**The end of a passage was a circle on the chart you sat inside until a number went up.**
What is in the model now is a pull at the stemhead, and everything a boat at anchor does
comes out of it rather than being written down.

**Three numbers.** *Scope* — the chain out over the water under her — is most of whether
she holds: what makes an anchor bite is the pull arriving along the bottom, and what
keeps it along the bottom is the weight of chain lying there. Under two and a half to one
she is parked rather than anchored; five to one is the almanac's figure and is where the
curve flattens, and `V` veers exactly that on the water she is in. *The ground* is a fact
about the place, drawn from the position — so the same bay has the same bottom next week
and learning it is what local knowledge is. Sand holds twenty times the anchor's weight,
mud nine, weed four. *The pull* is the windage and the stream the simulation is already
working out.

**What falls out of that, measured through the hull rather than asserted:** she lies head
to the wind, because a force a hull-length forward of the centre of gravity is a yaw
moment and the moment swings her head round — put her beam on to eighteen knots with the
anchor down and she rounds up to it by herself. She rides to the tide when the tide is
the stronger, which is the thing that catches out everybody who has only anchored in a
lake. An anchor *sets by being dragged*: let go in sand and load her with forty knots and
she walks seven metres while the fluke digs itself down, and then stops. In weed she
skates over the top and is still walking five minutes later, a cable downwind of where
she went to sleep. And dragging is a plough rather than a release — the chain never goes
slack, which is why it is noticed by a bearing changing rather than by anything happening
on deck.

**The chart keeps the watch.** The anchor is drawn where it lies, with the ground and the
depth beside it, and the circle she can swing in round it: inside the circle is lying to
it and outside it is dragging, which is a thing you can see without reading a number. The
console carries the scope, the ground and how far the anchor has walked; past fifteen
metres the alarm goes off.

**The windlass lifts chain, not a boat.** Press `V` again and it comes in at a windlass's
pace — and while the cable is bar taut nothing comes in at all and the foredeck says so,
because getting your anchor back means sailing up to it and taking in the slack as it
comes. `SAIL_ANCHORED=1` opens lying to it in the nearest anchorage, which is how this
gets photographed.

## The radio

A VHF, on the console under the weather, because what comes out of it *is* weather.

**Sixteen is watched and not worked.** The set has four channels — sixteen to call, six
and twelve for the coast station and the harbour, seventy-two between boats — and a dual
watch, which is the one feature of a real VHF nobody explains and everybody uses: it
listens to your working channel and flicks to sixteen between words, so you can be
talking to the harbour and still hear a distress call. The squelch is where a set is
left, shut just far enough that an empty channel is quiet, and a station too weak to
break it is a station you do not hear.

**Range is worked out rather than assumed.** `2.5 × (√h₁ + √h₂)` nautical miles, the
radio horizon, which is further than the optical one because the atmosphere bends the
signal down — the whole reason a radio reaches past the sea's curve at all. Our aerial is
at the masthead at sixteen metres; a coast station sits on a hill at a hundred and eighty.
That asymmetry is why the harbour hears you from forty miles and the yacht two miles
behind the island does not, and it is the same model the horizon and the fog are drawn
from rather than a table of ranges.

**The forecast is generated out of this world's weather, not played from a recording.**
A canned one would be somebody else's weather read over yours, and you would learn within
two passages that it never matches — after that it is furniture. The wind and its outlook
come off the synoptic, the second half of it (*veering northwesterly and increasing
later*) off the front, which has a time and an air mass behind it, and the visibility off
the fog, through the sky's own function with the sun left out of it. The form is the real
one and the order is fixed — wind, then weather, then visibility — because that order is
what makes a shipping forecast possible to copy down.

**At 0520, 1120, 1720 and 2320, and that is the content.** A forecast you can ask for at
any moment is a weather page in a different font; a forecast at a fixed hour is a thing
you plan a departure around. It is called on sixteen and read on channel six, which is
the first thing a radio teaches: sixteen is where you are told *where to listen*, not
where you listen. Leave the set on sixteen and you hear that there is a bulletin and miss
it.

**The ship you are leading is on it, and that is where her voice went.** The pilotage
mission is built on the fact that two vessels cannot shout at each other — it is the
whole reason the flag locker exists — and her master was being heard calling across four
hundred metres of water about the tide. One of those two was wrong, and it was not the
flags. So she calls up on sixteen the way a ship wanting a pilot does, says what channel
to go to, and everything she has to say about the water arrives there. The flags still
work out of earshot: she reads the hoist, answers with the pendant and obeys it. What she
*says* needs a radio, a channel and a range, and a pilot boat left on sixteen hears her
call up and nothing after it.

**And the harbour talks to the water it can see.** Come inside two miles of a harbour and
its port control calls on sixteen and reads traffic and tidal information on twelve: when
its high water is, what the stream is doing and which way it sets, and what else is under
way in the approach. All four of those are in the simulation — the tide has been under
the keel since the spring and the only place it ever showed was the depth sounder. There
is no ferry in this model, so there is no ferry on the air: what it counts as traffic is
whoever else is actually sailing.

**And you can say something back.** There is no voice in this game and the answer to
that is not a text box: it is the four things a sailor says on a VHF, each in the form
the book gives it, offered when there is somebody to say them to. A radio check, a call
to the harbour you are standing into, a request for the bulletin you slept through, and
an answer to the ship that has been calling for a pilot. You pick the call; the words
that go out are the right words, which is how anybody learns them, and the whole phrase
is on the hover because the words *are* the content.

They go out on the channel in the window and not on the one they belong on — call the
harbour while the set is on seventy-two and nobody answers, which is the entire reason
channels are taught. Who hears you is the same two square roots as everything else: a
station behind the horizon is silence, and a radio check is answered with a readability
report, *loud and clear* or *readable but with difficulty*, which is the one place in the
game where the range model is spoken out loud. Ask for the forecast and you are sent to
channel six, because sixteen is where you are told where to listen — a set left on
sixteen hears the instruction and misses the bulletin. And the set is deaf while its own
carrier is up, for as long as the words take to say, which is why *over* exists.

**And you can hear it, which was the whole of what was missing.** The first version of
all this was text: the words appeared on the console and in the book, and the one thing a
VHF does in a cockpit — break the quiet so that you look up — did not happen. Three
sounds, none of them a voice. The open channel hisses when you turn the squelch down,
which is what the knob is for and what its own documentation had been promising to
nobody. Somebody on the channel is noise through a speech band shaped into syllables at
the four a second of ordinary speech: you hear *that* the channel is occupied and you
read what was said, which is the right amount when there is no voice in the game and a
synthesised one would be worse than none. And the carrier clicks when it opens and breaks
into noise when it drops, which is the one radio sound everybody can imitate and the one
that says *that was the end of it*. A weak station has noise under it, so the range model
arrives at the ear and not only at the page — and the band ducks under a transmission,
because music over a radio call is why every cockpit speaker in the world is wired to
mute the stereo. Measured at six and a half decibels over a fresh breeze at the default
faders: enough to look up, not enough to shout.

**The log is a page of the book.** A VHF is the width of a VHF and a forecast is five
lines of writing, so the console carries one line — the last thing said, marked *(weak)*
if it came in broken — and the third tab of the log book carries the set's own log: time,
channel, station, and the words underneath, kept whether or not anybody was listening.
You come below at four in the morning, turn to that page, and find out what you slept
through. What the *deck* log gets is one line against the hour — `copied Nordholm
Coastguard, ch 6` — because a navigator writes down that he had the weather, not the
whole of it, and the bearings on that page are what it is for.

## Sailing 101

`Esc`, then **MISSIONS**, then **SAILING 101**: fifteen steps round the deck, one
instruction at a time, each about *one control and the instrument that answers it*.
Tiller, view, mainsheet, headsail, rope bank, the dump, the preventer and the backed
genoa, pilot, watch, chart, glasses, hand bearing compass, log, rod, lifebuoy — and
nothing moves on until the thing has been done.

**The key list is not a lesson.** `H` puts forty-four keys on the screen in six groups,
which is the right reference and the wrong introduction: somebody who has never sailed
does not know which of the forty-four they need in the first minute, and somebody who has
sailed does not know this boat carries binoculars until they read the whole list looking
for something else.

It sets the stage before the first instruction — the watch stood down, the pilot taken
off — because a lesson about the mainsheet is nonsense while somebody else is trimming
it, and both come back as steps of their own. That is also the honest order: you learn to
do a thing before you learn to hand it over. The two sighting steps watch the
*instrument* rather than the keystroke, so finding the console buttons first counts as
having done it.

`SAIL_LESSON=1` starts it from the first frame, `?lesson=1` in a tab.

## Man overboard

There is a horseshoe lashed to the starboard quarter on a short strop, twenty metres of
floating line coiled on the same cleat, and a canvas grab bag lying on the deck over
them. `Enter` throws the ring, and the throw is the part that cannot be late: a person in
the water makes two per cent of the wind speed and a ring of foam makes four to eight, so
the ring walks away downwind at fifteen metres a minute — thrown early it is something he
can swim to, thrown after a minute of getting organised it is upwind of nothing. Those are
search-and-rescue numbers rather than invented ones, and the whole drill turns on them.

`Esc`, then **MISSIONS**, practises the lot on a dan buoy: get her sailing, lose
Oscar over the side, and get him back. It talks you through a recognised recovery rather
than leaving you to invent one — and it is the one piece of boat handling where being bad
at it costs a life and nobody practises.

## The boathook, and what is floating out there

Four feet of varnished ash with a hook on the end, lying in its chocks on the starboard
side deck. It is on the boat because it is on every boat, and it is in the game because
of what it does to two manoeuvres that used to end by themselves: coming alongside a
person in the water, and picking the lifebuoy back out of it. The hook goes to whatever
is within its length — you do not aim it — and `Enter` takes the catch. **When** is the
whole of it: a hook put on while the deck is dropping away is a hook torn out of your
hands, and the sea decides when that is, not the player.

To practise that for the price of a barrel, there are forty casks adrift in the
archipelago. Through the glasses the nearest one inside four hundred metres is reported
the way a lookout reports it — *a cask fine on the starboard bow, 280 m* — because a
bearing in words puts your head in the right place where a figure makes you do arithmetic
first. The water you have to get her into is drawn on the water: a ring round each one,
ivory outside it, green when she is in it and slow enough to hold what she hooks, and
vermilion when she is in it going too fast, which is the commonest way this is got wrong.
Hooked casks go on the painter astern, nose to tail, seven metres apart, and follow her
round a headland instead of cutting the corner onto it. They come off at any anchorage,
one at a time, alongside and stopped.

## The watch on deck

The pilot steers because a tiller pilot is a box you can buy and bolt down. There is no
box that trims a boat — what trims a boat is people, which is why the switch beside the
instruments has two of them engraved on it. With the watch on deck you take the helm and
they sail her: the sheets continuously, trimmed to the wind each sail actually meets
rather than to the masthead fly; the shape controls one line at a time, because there is
one winch on the coachroof and a crew is subject to that rule too; the right sail for the
angle and the breeze, which is a job with stages rather than a setting — roll the old one
away, bend the new one on, set it. They ease before they reef, which is the order that
matters: a slab is twenty minutes of work and a sheet is a second. A hand on any sheet
stands them down, the way a hand on the tiller takes the helm from the pilot.

## Pilotage, and the brig

The oldest job on any coast, and the only one where local knowledge is the whole of the
value. A loaded brig stands in from seaward wanting a pilot for the inner passage: she
makes six knots and will not wait for a boat that has not caught her, she will not lie
closer than sixty-five degrees to the wind, and she draws four metres where you draw two.

Nothing is transferred and nothing is handed over. **Get ahead of her** — inside seventy
degrees of her bow and within three quarters of a mile — and she follows the water you
cross, two cables astern and thirty seconds behind everything you do. Drop astern or out
of sight after she has taken you and she rounds up and waits, because a ship that has a
pilot does not carry on into a bay she does not know. Her master says what his vessel
will and will not do, in the order a seaman says it: the bottom first, then a course she
cannot sail, then a turn too fine for three hundred tons.

**And you can talk to her.** A pilot boat and the ship she is leading cannot shout across
half a cable of water with two sets of canvas between them, so they do what every ship
has done since long before anybody had a radio: they hoist flags. Five single-letter
signals from the International Code are bent on in this boat, and each one means on this
water what it means on any other — **K** *I wish to communicate with you*, **L** *you
should stop your vessel instantly*, **X** *stop what you are doing and watch for my
signals*, **P** the Blue Peter, **U** *you are running into danger*. She reads them with
a lookout and a signal book, which takes a few seconds, and answers the way a ship
answers: the **answering pendant** to the dip when the hoist has been seen, close up when
it has been read and will be acted on. Out of signalling distance, or in thick weather, a
flag is cloth nobody can read and nothing happens — which is why the pendant is the thing
to watch. K is the one that was missing: ask her what she is doing and she says whether
she has you ahead of her, is waiting for you, or is standing in under her own navigation.

The chart carries the way in — a search over the soundings for deep water away from the
edges that a square-rigged ship can actually sail, so the line moves when the wind does.
The pilot flag is in our own rigging, hoisted when the job is taken; `Enter` in clear
water off the anchorage strikes it, and that is what ends the job. Her deck cargo has
been working loose since the gale, so there are casks in the water on the way out to her.

She is drawn to the boat's own standard: a lofted hull with her paint on it, masts in the
three pieces a mast is made of, ratlines up the shrouds, and yards that brace round with
the wind and stop where the shrouds stop them. The cloth bellies to leeward, gathers to
the yard as it is clewed up, and lies flat against the mast when the wind comes on the
front of it — aback, which is how a square-rigger is stopped.

`Esc`, then **MISSIONS**, which also has the man-overboard drill.

## The pilot

Two pieces of gear in one box, because they are two pieces of gear on a real boat. `P`
engages and `O` chooses which: a tiller pilot holding a compass course, good for a
passage leg and useless upwind because a shift leaves the sails wrong, or a vane holding
an angle to the wind, which follows every shift and is how you sleep on a long passage.

The electric one can let go and says so — it is a ram with a controller, and a controller
that has been on the stops and off the course together for six seconds knows it is
braking rather than steering. The vane has nothing to let go *with*: no controller, no
relay, nothing to decide anything. Overpowered it does not disengage, it stops coping —
the blade lies over on its stop, she wanders further and further off, and in the end she
tacks or gybes herself and the gear goes on patiently steering the new course. That is a
different failure to be caught out by, and `tests/the_pilot.rs` is the bench that keeps
both of them honest.

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
| the reel | a rate — a handle you turn, in two gears | line in, line out |
| the drag | a position — a star you screw down | whether the fish tires, or the hook pulls |
| line out | a length | how deep the lure swims, and therefore what can see it |

Shift is low gear, and it is the one piece of tackle here that exists purely to solve a
problem the rest of the model creates: past about half the line's strength the handle
simply stops turning, and low gear trades the line you gain per turn for the force you
can turn against. It is not free — winding against a heavy fish *puts that load on* — so
it gains you line and wears the hook at the same time.

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

**Depth is the boat's speed.** A plain trolled lure's downforce is its own weight, which
is constant, and the line's drag goes as the square of the speed — so the whole thing
collapses as she goes faster. At five knots it skips along in the first metre or two and
you can watch it work; take the way off her and it sinks hard, nine metres at two knots
and thirty at one. That is why mackerel are caught fast and shallow and cod are caught
slow and deep: **the boat chooses the quarry before the tackle does.**

This was wrong twice. First as a triangle, with the line's drag lumped at the lure, which
saturates around fifteen metres however much line you let out and deletes the deep fish
from the game — the drag is distributed, so everything near the lure hangs steep and
everything near the rod lies flat, and done properly it is an integral with a closed
form. Then as a *diving* lure, whose vane lift goes as the square of the speed just as
the line's drag does, so it held fifteen metres at every speed. That is a deep-diving
plug on a downrigger, not anything a small boat tows, and it made the one thing you can
actually see — the lure working astern — invisible at all times.

Which is the other half of it: **the splash is the depth gauge**. The lure is a hand's
length of metal under an opaque sea, so what you see is what it does to the water, and
the mark fades out as it sinks and is gone by four metres. No disturbance astern means
the lure is down. The number is on the console for when you want it.

A line long enough and a boat fast enough will also part the line with nothing whatever
on the hook — a hundred and sixty metres towed at six knots pulls harder than twenty
pound braid will stand. That is the commonest way to lose tackle off a sailing boat,
because the load builds while you are busy with the sails and the first you know of it is
a straight rod.

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

### What is not in the water, and what would have to change first

No mahi, no marlin, no swordfish, and that is a fact about the place rather than a gap in
the work. The sun in `sky.rs` is worked out for 57 degrees north at a declination of five,
which is an Irish spring: eight degrees of water. Mahi want twenty-one. Marlin have never
been here at all, and a swordfish off Scotland is a stray that makes the papers.

**Bluefin tuna are the interesting case, because they genuinely came back.** They have
been taken off the Hebrides and Donegal since about 2018 and Ireland has run a tag-and-
release fishery for them since 2019. Two things stand in the way and neither is modelling
effort. They arrive in August and run to October, chasing the mackerel — so putting one in
means moving the season, which moves the sun, the sea temperature and the whole look of
the light. And a hundred and fifty kilogrammes of it against twenty pound braid is not a
fish you land; it is the spool going empty and the arbor knot letting go in half a minute.
Which is an honest thing for a game to contain, and it is a different feature from the one
this is — that is a fish you lose on purpose, and it wants the season first.

What would come before any of that, because it is here now and in this season:

* **Tope.** The real big run in these waters — fifteen to twenty-five kilogrammes of
  shark that will take a trolled bait and make the ling look patient. It drops into the
  table beside the others without changing anything.
* **Garfish.** The one fish on this coast that *does* leap and tail-walk, which would
  overturn the paragraph above — honestly, and that is the only way a rule like that
  should ever be overturned. A beak full of bone and a hook hold to match.
* **Sea trout and bass**, which are ordinary here and are simply missing.

**Every take rips line off the spool**, including the smallest fish in the table. That
took two goes. The first put a shock *load* on the line and was wrong by measurement —
thirty newtons of drag against half a kilogramme of mackerel hauls the fish bodily in, so
the spring dumped in a twentieth of a second and the whole event came to five centimetres
of line. The model was telling the truth and the mechanism was wrong. What actually
happens is that the drum is standing still and has to be got turning: the spool's inertia,
the fish turning away with the lure in its mouth, and the boat sailing on from where the
fish now is, none of them worth modelling alone and together about a third of a metre of
line and ten teeth of ratchet. A drag screwed down past the load a take generates holds it
dead, which is the same lesson the dial teaches everywhere else rather than an exception
to it.

**Not everything that hits a lure stays on it.** A knock you feel and nothing more is half
of trolling, and it is information rather than a near miss: it says you are in the right
water, which is the whole reason anybody keeps towing the same patch. A cod's mouth closes
over the lot and a mackerel often just slashes at it, so the rate is per species.

**The glass drives the bite.** Rising weather shuts them down and a front coming on wakes
them up — the oldest belief in angling and one of the few that holds up. It costs nothing
here because the boat already carries the reading: OUTLOOK on the console, which until now
only told you when to reef. The two now read the same instrument in opposite directions,
and the hour you ought to be shortening down is the hour they are feeding.

**Lean on it and it goes.** A fish with something left does not hang there and take heavy
pressure; it runs. That is the one place the fight answers the *player* rather than a
table, and it is why a heavy hand is not simply a faster way to win — pull harder and you
get a longer fight, which is the first thing anybody is told on a boat.

**Pumping, and the error it found.** Lifting the rod moves the tip away from the fish,
which is arithmetically the same thing as the fish swimming away: the spring takes it up,
the tension rises and the fish comes. Dropping the rod gives that distance back — and if
you are not winding as it drops, what you have made is *slack*, which on a head-shake
throws the hook. None of that had to be written; it was already in one line of the model,
and `U` is the only thing that was added.

What it exposed is worth more than the mechanic. Pumping gained nothing at first, which
is the opposite of every angler's experience — because the reel was modelled far too
strong. Work it out at the handle: a six-to-one multiplier needs the line's pull times the
spool radius times the ratio over the handle radius, about **three and a half times the
line tension**, so twenty-four newtons on the line is nine kilogrammes in your fingers and
that is where winding stops. It had stood at seventy, which is twenty-five kilogrammes and
not a thing a person does. With a reel that strong there is no load a rod is better at
than a handle, and *that* is why pumping looked useless. The parameter was wrong and
asking for the technique is what found it.

**Nothing comes quietly, and the last move is timed.** Every fish, however beaten, finds
one more run at the sight of the hull — that is where most fish are actually lost, and a
model in which a tired one simply arrived was missing the most famous ten seconds in
angling. What is left after that is not a question of strength but of timing: a fish
alongside is lifted *on the rise*, letting the sea bring it up to the rail, and one swung
aboard as she drops away takes its whole weight on a hook that has been working loose for
five minutes. `J` is the lift, the console says when the moment is there, and a fish will
not lie beside the boat for ever while you decide.

The catch goes in a creel on a lanyard off the port quarter, where the sea keeps it cool,
and it swings — as a real pendulum with a real period rather than a filtered copy of the
roll, because what hanging gear mostly does is swing *against* the boat, and a filter
cannot do that. A score is not a thing; three fish in a basket off the quarter is an
afternoon, and you can see it from the helm without asking.

`C` gives a fourth view, under the surface beside the lure — which this project argued
against on the grounds that the sea is opaque and there would be nothing to see. That was
an argument about the *surface*. Below it is the one thing the whole activity is about and
that no other view can show. What makes it read as water rather than as a camera that has
fallen in is the visibility: ten metres, green rather than blue because this is a shelf
sea with a spring bloom in it, and the sky taken off the camera altogether — you cannot
fog a skybox, it is drawn at infinity and no falloff reaches it, so a fogged scene under a
bright sky is a green room with a hole in the ceiling. With no rod out it sits under her
counter instead, which is the only way to watch the keel and the rudder do what the
physics says they do.

The third view faces aft from the cockpit down the wake — which is worth
having without a rod in your hand, because ahead is where you are going and astern is
where your wake, your gear and anything overtaking you are. `SAIL_ROD=30` streams thirty
metres before the first frame, `SAIL_HOOK=ling` puts one of that species on as soon as the
lure is fishing and another whenever the line comes free, and `SAIL_FISH=20` makes them
take twenty times as often,
for the same reason `SAIL_SQUALL` exists: the fight is the interesting part, and testing
it by waiting for takes is an afternoon per change.

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
archipelago, their weather and their time of day. Everybody's boat is on the chart with
a name beside it, which is the only place "where is everybody" has a useful answer.

A game on a server rather than a laptop has an address and a **word**, and both go in
that page. **KNOCK** asks what is there without joining it — the game, how many are
aboard, which sea — and when nothing answers it says which of the three things it is:
the word, the port, or a server that is not running. All three used to look identical
and all three are different evenings. The address and the word are remembered between
sessions.

Over the internet the same thing works with the host's address typed in, and the
least-work answer is one Tailscale network between friends: no port forwarding and no
code. `docs/online.md` is the whole design, including what it deliberately does not do.

## Controls

The camera looks at what just happened: the reel when your hand is on the drag, and
otherwise a short way *down the line* — tip, bend and the place it enters the water in
one frame, landing squarely on the far end once most of the line is in — including when something takes. Not the fish, which was the
obvious choice and the wrong one, because a fish is two metres under an opaque sea and
aiming at it frames a patch of blank water. Where the fish appears is in the bend of the
blank. It is posted as a *claim* on
the eye rather than as a camera command, highest claim wins, and your own hand on the
right mouse button outranks all of it and is remembered for three seconds afterwards —
because a view that corrects itself while you are still looking at the thing you turned to
see is a view that argues with you.

**They live in the program.** Press `?` or `F1`, or click the mark in the bottom-right
corner — and `Esc → MISSIONS → SAILING 101` walks through the ones that matter first,
one control at a time, which is the half a list cannot do. `F12` takes a picture of the screen into your Pictures folder, which is the
answer to *it did something odd and I cannot describe it*. This list used to be here as well, and three of the keys in it were wrong — a
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
