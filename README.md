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
2. **The physics is testable without opening a window.** 128 tests in the core assert
   against figures a sailor knows from the water — 248 across the workspace. When one
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

## Navigation

The archipelago has names on it, and finding out which bit of it you are looking at is a
game in its own right — the oldest one there is, and the one a chart plotter deleted.

**The chart.** `M` puts it in the corner, `Shift M` spreads it on the table. The corner
one is a glance: where am I, what is ahead, and it stays under the boat. The one on the
table is an instrument, and it has the instruments a chart table has — dividers for
distance and bearing, a plain ruler for a leg with two ends, a parallel rule that plots a
position line through a mark, a pencil, a watch that writes the time where you put it,
and a rubber. The coast on it is read off the same signed-distance field the boat runs
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

**`Shift L` puts you somewhere at sea with no idea where.** The sails are set for the
wind she is in and nothing else is made difficult; the chart stops following her, the
names come off the marks, and getting them back is the exercise. Three bearings, crossed,
entered in the log — and the scoring is the honest one: a fix is good if it is near where
she actually is, not if the cocked hat was small. A tight triangle on two misidentified
marks is a confident lie and the classic way to put a boat on a rock.

## Man overboard

There is a horseshoe lashed to the starboard quarter on a short strop, twenty metres of
floating line coiled on the same cleat, and a canvas grab bag lying on the deck over
them. `Enter` throws the ring, and the throw is the part that cannot be late: a person in
the water makes two per cent of the wind speed and a ring of foam makes four to eight, so
the ring walks away downwind at fifteen metres a minute — thrown early it is something he
can swim to, thrown after a minute of getting organised it is upwind of nothing. Those are
search-and-rescue numbers rather than invented ones, and the whole drill turns on them.

`Esc`, then the **DRILL** page, practises the lot on a dan buoy: get her sailing, lose
Oscar over the side, and get him back. It talks you through a recognised recovery rather
than leaving you to invent one — and it is the one piece of boat handling where being bad
at it costs a life and nobody practises.

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
