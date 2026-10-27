# Selling her on Steam

A plan for putting this game in a shop, and for the content that goes with it. It is
written the same way `docs/online.md` was: the shape first, the detail that is missing
from it second, and an honest list of what only the owner can decide.

**The one-line version, as it stands now:** the short list of things that stood between
here and a shop window has been built — the window resizes, the anchorage persists, there
are twelve achievements and a depot script. **What blocks the shop window is no longer
code.** It is an App ID, three depot IDs and an account with build permission, and every
line of the upload path is written and untestable until those exist. The content has gone
on growing meanwhile, which is the right thing to do with the waiting and the wrong thing
to mistake for progress towards a launch.

**Decided, 21 October, and it is a reversal of the day before:** **Early Access**, with
multiplayer in it from the first day. Price later. Windows, macOS and Linux, the Mac
unsigned at first and signed before it is sold. No time compression: the game is aimed at
sailing together in small worlds, where a passage is an evening and not a crossing, and a
clock that runs faster for one boat than another is not a thing a shared sea can have.
Demo as recommended — Sailing 101 and one delivery. **Done since:** the window resizes and
goes full screen.

---

## Where she stands today

Facts, not impressions:

| | |
|---|---|
| Model | 4 degrees of freedom at 200 Hz, 531 tests across the workspace |
| Platforms built in CI | macOS universal, Windows; Linux builds the **server** only |
| Artefact size | `Sail.exe` 97 MB, `Sail-macos.zip` 69 MB — one file each, assets inside |
| Controller | Already wired: sticks on the helm, triggers on the sheets, buttons on the rest (`input.rs`) |
| HUD scaling | `retro::console_scale` already matches egui's pixels-per-point to the physical width |
| Licences | Clean: the hull is CC0 from the Scottish Maritime Museum, the font is OFL and ships with its text, everything else is generated in code |
| Multiplayer | Two to eight boats through our own relay (`sail-net`), LAN and a public host |
| Demo in a tab | Published on Pages, Chrome and Edge; Safari paints it black and the page says so |

That is an unusually good starting position for a one-person simulator: the expensive
half — a boat that actually sails — is done and tested, and the assets carry no licence
debt.

---

## What blocks a shop window

None of it is content. It is the difference between "a thing you can run" and "a thing
somebody pays for".

### 1. ~~The window is nailed to 1440 × 880~~ — done

`main.rs` set `resizable: false` and a fixed size, on purpose and for a good reason: the
instruments are drawn at known pixel sizes and a resizable window promises a layout the
picture cannot keep. On Steam that reasoning does not survive contact with the customer —
people expect full screen, their own resolution, and a game that fits a 1280 × 800 Deck.

It does keep the promise now, because two pieces of it were already there: the world is
rendered at a fixed drawn width and scaled to whatever the window is, and
`retro::console_scale` shrinks the console until the strip fits. So the window opens,
resizes and goes full screen on `F11` or from the settings panel, with a floor under it
(`screen::NARROWEST` by `screen::SHORTEST`) because a window that can be dragged into
uselessness is a bug report nobody can diagnose. The chart in the corner now takes its
width from the instrument case beneath it rather than a constant, so the right-hand side
reads as one column at every size.

**Landed in:** new `screen.rs`, plus `main.rs`, `chart/mod.rs`, `settings.rs`, `help.rs`.
What is left of this item: a resolution picker for the windowed case, which is the window
manager's job on every platform we ship to, and only worth adding if somebody asks.

### 2. Nothing is ever saved — and only one thing should be

There is no code in the tree that writes anything to disk, which for a long time read as
the obvious gap. It is half a gap. **This game does not save at sea and it is not going
to**: a passage is an evening, the clock runs at one to one, and a boat that can be put
down mid-gale and picked up next week is a boat nothing can go wrong to.

What persists is what you brought home. The player keeps a **floating platform moored in
one anchorage** — a raft with a hut on it and the casks stacked where they can be seen
from the water — and every mechanic this game already has gains its second half: casks
fished out of the sea were collected and handed over at any anchorage, which made them a
number, and the creel filled and then emptied into nothing. Both of them have somewhere to
go now, and the pile is *drawn*, because a store you cannot see from the helm is a figure
on a panel.

That forces one more thing, and it is the interesting one: **the sea has to stay the same
sea**. The archipelago is a pure function of its seed and a desk build has always drawn a
new one from the clock, which is right for a simulator you sit down at and wrong the
moment something belongs to you. So the saved file records the seed, and from the second
launch the islands, the names and the berth are the ones your platform is moored in —
while the *weather* still comes from the clock, because a day you have already sailed is a
replay. Your islands, a new day.

**Landed, with tests:** the file (`~/Library/Application Support/sail/home` and the
equivalents), the platform and the pile, the two seeds, the chart mark that says where
home is — and the other half of a store, which is something to spend it on. A chandler's
list appears when she is lying alongside and nowhere else, priced in casks and in fish
because those are what there is: a **derrick** that swings casks aboard three times as
fast, a **drying rack** that hangs the catch where it can be seen from the cockpit, and a
**lamp** that lights itself through the last eight degrees of dusk. Each one is built on
the raft and drawn there.

**What is left:** more to build and more to build it with — gear kept ashore, a reason to
go further out, and the fittings the content list implies once there is night and weather
to want them in.

**Lands in:** `anchorage.rs`, with `sim.rs` taking the sea as its own argument.

### 3. ~~No pause, no time compression~~ — not for this game

Struck out rather than deleted, because the reasoning is worth keeping: with the clock at
one to one, forty miles at five knots is eight hours, and that is what the README has
always said would have to come back.

It does not come back here. **The game this is becoming is two to eight boats on the same
water**, and a clock that runs at four for one of them and at one for the others is not a
thing a shared sea can have — the alternative, a compression everybody agrees to, is a
lobby vote in the middle of a passage, which is worse. The answer instead is the size of
the world: five and a half kilometres across, harbours a mile and a half apart, a leg
that is an evening rather than a crossing. A passage you can finish is better than a
passage you can skip.

**A pause for one person on a desk** is still worth having, and is small. It is not on
this list because it blocks nothing.

---

## The platform matrix

| Platform | Today | For Steam |
|---|---|---|
| Windows x86-64 | Built in CI | Ship it |
| macOS universal | Built in CI | **Decided:** ships. Unsigned while it is only being sailed here; signed and notarised before it is sold — see risks |
| Linux x86-64 | Server only | **Decided:** ships. The shared server already runs there, and the same job can build the game beside it |
| Steam Deck | Untested | Controller is already in, the window now fits it; needs a verification pass |

The cheap path to the Deck is Proton running the Windows build — worth testing on day one,
because if it works the native Linux depot becomes a nice-to-have rather than a blocker.
The frame report (`?frames=1` / `SAIL_FRAMES=1`) and `dist/tab.py`'s measuring loop already
give us the numbers to decide whether the Deck needs a lower drawn width by default.

---

## Steamworks: what to take and what to leave

**The one fact that shapes all of it:** linking Steamworks rewrites the binary's own
loader path. A build with it on will not *start* — not fail, not warn, killed by the
dynamic loader before `main` — unless `libsteam_api` is sitting beside the executable:

```text
dyld: Library not loaded: @loader_path/libsteam_api.dylib
```

So Steam is a **cargo feature, off by default**. Two binaries out of one tree: the plain
one on the releases page, which has never heard of Steam and runs from wherever somebody
unzipped it, and `--features steam` in the depot with the library next to it. Nothing in
the game behaves differently; anything that did would be a second game to test.

**Take, in this order:**

1. ~~**Depots and SteamPipe.**~~ Written and dry-run: `dist/steam.sh` builds this
   machine's depot payload — binary plus library, the library *found* rather than named,
   because cargo leaves it under a build hash that moves — and checks that the two landed
   together, since a depot whose binary cannot find its library installs perfectly and
   never opens a window. The manifests are in `dist/steam/`. The upload is written and
   **cannot be tested**: it wants an App ID, three depot IDs and an account with build
   permission, and it fails loudly rather than half-running without them.
2. **Cloud saves.** Still nothing to write: Steam Auto-Cloud watches a directory and
   `kept::dir()` already is one. It is a path typed into the partner site.
3. ~~**Achievements.**~~ Twelve, in `earned.rs`, and they are **watched for rather than
   announced**: this file reads state that already exists and decides for itself, instead
   of twelve modules each carrying a line that says "and tell the achievements" — which is
   exactly the line that gets lost in a refactor and is never missed, because nothing
   fails when an achievement quietly stops being given. They are kept in our own file
   whether Steam is there or not, and shown on the missions page, because a list that only
   exists inside somebody else's client is not ours to show.

**The App ID is 480 until there is a real one.** That is Spacewar, Valve's public test
application: against it the library initialises, the overlay works and an unlock is
accepted and then dropped, so the whole path can be exercised and the only untestable part
is the number itself.

**Leave for later:** Steam Networking in place of our relay. The relay works and is
tested; what Steam would buy is NAT traversal and no port forwarding, which matters for
public play and not at all for a LAN. Revisit after launch.

**Skip:** Workshop, trading cards, inventory, leaderboards. A passage-time leaderboard is
tempting and would turn a cruising game into a racing game by accident.

---

## The shop window

* **Store page up early.** Wishlists are the whole of a small launch: Steam's visibility
  rounds are driven by them, and they accumulate over months. The page should be live
  long before the game is.
* **Trailer.** The game already films itself: `SAIL_SHOT_EVERY` with `SAIL_SHOT_KEYS`
  drives a scripted reel, and `dist/tab.py` can do the same in a browser. Sixty seconds:
  a gale, the ink sea, a night entry once night exists, the chart work, two boats.
* **Capsules.** The one piece of real art to commission or make. Everything else on the
  page is screenshots the game already produces.
* **A demo app.** On Steam a demo is a separate app built from the same binary with a
  limit: Sailing 101 plus one delivery, no campaign. The Pages build stays where it is as
  top-of-funnel — it is not the Steam demo and does not need to be.

---

## Price and shape

**Decided: Early Access, with multiplayer in it from the first day. Price set nearer the
date.**

It was decided the other way on 20 October and reversed the next day, and the reversal is
the honest one. The case against Early Access is real and was not wrong: a game sold as
unfinished is reviewed as unfinished, the launch visibility is spent once either way, and
a one-person project pays for the store-page work twice. What beats it is what this game
most needs, which is **other people sailing it**. The simulation is the finished half and
the content is the growing one; a 1.0 held back until the content list is complete is a
year of nobody on the water, in a game whose own argument is that there should be two
boats in the sound. Early Access is the only shape that puts the relay to work before the
campaign exists.

What that buys downstream, and what it costs:

* **The content list goes back to two waves.** Wave one is the scope of the Early Access
  release; wave two is the promise on the store page, and 1.0 is the day that promise has
  been kept. The milestones below are written that way.
* **The store page has to say what is missing**, in our own words rather than Valve's
  boilerplate: how long it is expected to stay in Early Access, what is in it today, and
  what the full version adds. That text is part of M2 and not an afterthought — it is the
  page a buyer judges the honesty of the project by.
* **Multiplayer is in from day one**, through the existing relay, because the whole reason
  for not waiting is people sailing together. Steam Networking comes after.
* **And the second round of store work is real.** Leaving Early Access is a relaunch:
  new capsules, a new trailer cut, a date announced again. Budget it rather than
  discovering it.

---

## Content: what this engine makes cheap

Ordered by what it buys against what it costs. Everything in the first five rows is
mathematics and prose, which is exactly where this project is strong — no new art.

| Content | Why it is cheap here | What it buys | Size |
|---|---|---|---|
| **Tides and tidal streams** | Pure maths over the existing archipelago; drift already exists as a vector | Planning: a tidal window, a stream in a sound, a passage that has to leave at the right hour | 1–2 weeks |
| **Night** | The lighthouses already carry their flash characteristics (`Fl(2) 8s`), the sky model already has a sun | Pilotage by light alone — almost nobody does this, and it is the best screenshot in the game | 2 weeks |
| **Fog and visibility** | Haze and visibility already exist; sound is synthesised | Blind pilotage: the sounder, the clock, a sound signal, dead reckoning that matters | 1 week |
| ~~**Weather that moves**~~ | *In.* A front is a time rather than a line — at five kilometres across, every boat is through it within seven minutes of every other — so two air masses and a transition give the veer, the building, the clearance and the trough with nothing written down separately | A veering, building day, and a barometer that is ahead of the weather instead of a readout of the wind | 1–2 weeks |
| ~~**The radio**~~ | *In.* The range is two square roots of two aerial heights, the forecast is a reading of the synoptic the barometer already reads, and the stations it talks to — the coastguard, the harbour, the ship you are leading — were all in the model already with nothing to say | A fixed hour you plan a departure around, a channel you have to be on, and a ship whose master can tell you what she will not do | 1 week |
| **Failures** | A parted sheet, a jammed furler, a leak, an engine that will not start | The reason to carry tools and to reef early | 1–2 weeks |
| **Anchoring and mooring** | The anchorages are already generated and named | An end to a passage that is not a teleport, and an anchor watch at night | 1–2 weeks |
| **A second boat** | `BoatConfig` is parametric, so the physics is numbers | A different feel — a heavier cruiser, a lighter dinghy | 1 week + a hull |
| **A campaign** | Deliveries, money, repairs, a reputation per harbour | The reason to keep sailing after the third passage | 3–4 weeks |

Expensive and unavoidable separately: capsule art, a trailer cut, and the QA that three
platforms cost.

---

## The pipeline

Keep what works and bolt Steam onto the side of it:

```
git tag v1.0.0
  → release.yml builds macOS universal, Windows, Linux            (as today, plus Linux)
  → GitHub release                                                 (as today)
  → steam.yml: steamcmd +login +run_app_build app_build_1234.vdf   (new)
```

What the new job needs: `steamcmd` on the runner, a build account (not the partner
account), and the sentry file from a one-time interactive login stored as a secret —
`STEAM_CONFIG_VDF`, base64. Depot layout mirrors the artefacts we already produce. Default
branch stays closed; push to a `beta` branch first and promote by hand.

---

## Legal and admin

* **Steam Direct:** $100 per app, recoupable after $1,000 of sales. 30% of revenue to
  Valve.
* **Payout entity — decided: a Polish JDG.** The right call for a one-person project:
  it exists already or opens in a day, it costs nothing to keep, and Valve is the merchant
  of record — the customer buys from Valve, not from us, which is where most of the
  consumer-facing liability that would argue for a company actually sits.
* **Tax interview:** required before payouts, and a JDG files **W-8BEN** as an individual
  rather than W-8BEN-E: a sole trader has no separate legal personality, so the form
  carries a person's name and claims the Poland–US treaty. Withholding applies only to the
  US-sourced share of sales, and what it comes to with the treaty claimed is a question
  for an accountant rather than for this document — get the number in writing before the
  first payout rather than after it.
* **Three things worth asking that accountant, in this order:**
  1. **IP Box.** Income from a copyright to a computer program can be taxed at 5% for a
     JDG that keeps the required records *from the start of the year in which it arises*.
     A game sold on Steam is exactly that kind of income. Started late, the records cannot
     be reconstructed — which makes this the one question with a deadline on it.
  2. **Which form of taxation**, given the above: ryczałt, liniowy, or the scale.
  3. **VAT on a service sold to a US company**, and whether anything has to be registered
     before the first invoice.
* **The PKD code** for publishing games, 58.21.Z, has to be on the business before it
  sells one.
* **One thing that is not tax:** this is being written by somebody employed elsewhere as a
  CTO. Check what the employment contract says about work created outside it before the
  shop opens, not after. The git history is the record of when and where it was written,
  and it is a good one — but it is worth being able to say the contract allows it.
* **Rating:** Steam does not require one. PEGI/USK matter only for a boxed release.
* **Licences:** already clean. The hull is CC0, VT323 is OFL and its text ships beside the
  binary, the crates are MIT/Apache. `ATTRIBUTION.md` goes into the depot as it goes into
  the release.
* **EULA:** Valve's default is fine.

---

## Risks, and what to do about them

| Risk | Why it is real here | What we do |
|---|---|---|
| **macOS notarisation** | Steam does not sign for you; an unsigned macOS depot gets a Gatekeeper warning on launch | Apple Developer account ($99/year), sign and notarise in CI before the depot upload. Budget a day of pain the first time |
| **Bevy upgrade mid-project** | A quarterly breaking release; this tree pins `=0.20.0` on purpose | Freeze the engine for the whole Early Access run. Upgrade between content waves, never during one |
| **Deck verification rounds** | Valve fails builds for small things — text size, default controller layout, launcher dialogs | Do the checklist before submitting, not after |
| **CI time** | The Windows job compiles Bevy in about an hour cold | Keep the cache warm, tag deliberately |
| **Scope creep** | The content list above is longer than one Early Access, and five of its rows have landed since it was written — a wave two that keeps shrinking is a thin promise on the store page | Freeze the EA scope on the day the store page is written: what is in goes in, the rest is named as wave two in the page's own words, and nothing moves across afterwards |
| **One person** | Every row in this plan is the same pair of hands | The order in the milestones is chosen so that stopping after any of them still leaves something shippable |

---

## Milestones

Each ends with a sentence that is either true or false — no "mostly done".

**M0 — Decisions.** The list at the bottom of this document is answered. *Ends when the
Steam partner account exists and the app is created.*

**M1 — A game you can leave and come back to.** ~~Window and video settings~~ and ~~the
thing that persists~~ — both landed: the window resizes and goes full screen, and the
platform at the anchorage keeps the sea, the berth and the store across launches. *Ends
when there is something worth filling the store for*, which is the next piece of work
rather than this one.

**M2 — The shop skeleton.** Depots, CI upload, cloud saves, ~~twelve achievements~~,
store page live — with the Early Access text written in our own words: how long it is
expected to stay there, what is in it today, what the full version adds — wishlists open,
demo app built from the same binary. *Ends when a stranger can wishlist it and play the
demo.* **Everything left in it is blocked on the partner account**, which is M0.

**M3 — Content wave one.** ~~Tides~~ and ~~fog~~ both in; ~~night~~ landed early, with
the lights flashing their own characters.

*Fog* is weather of its own — a slow process in the synoptic that a breeze blows away,
so a bank sits in a calm and burns off when it fills in. It takes the visibility down to
sixty metres, puts the dome out as well as the water, and reaches the bearing compass:
a headland the renderer has fogged out can no longer be named or shot.

*Tide* is height and stream from one constituent, M2. The height goes on the sounding —
so a bar that is passable at eleven is not at dawn — and the stream goes in as *water*,
which means the keel and the rudder feel it and everything downstream of that comes out
right by itself: she crabs, the log still reads her speed through it, and a fair tide
costs her a little apparent wind. The range belongs to the coast and the hour to the
day, which is what makes tidal planning a thing rather than a constant.

*Ends when a night entry into a strange harbour in poor visibility is a thing you can do
and want to tell somebody about.* What is left for that: the harbour approach itself —
the lights are there, the fog is there, the water is there, port control now reads the
tide and the traffic at you on channel twelve, and **nobody has yet sailed one in anger
to find out what is missing**. That is a sea trial rather than a feature, it costs an
evening, and it is the cheapest item on this whole document: everything a first approach
turns up is something a buyer would have turned up instead.

**M4 — Early Access launch.** Trailer, capsules, price, date announced four to six weeks
ahead. *Ends on the day it is for sale.*

**M5 — Content wave two → 1.0.** ~~Weather that moves~~ and ~~the radio~~ are in ahead of
the launch they were listed after — a front that the glass sees coming, and a VHF that
reads a forecast made out of this world's own weather, talks to the ship you are leading
and to the harbour you are standing into. What is left is failures, anchoring, a second
boat and the campaign. *Ends when the Early Access promise on the store page has been
kept.*

**Which raises the question M4 has to answer: where is the line drawn?** Everything that
lands before the store page goes up is wave one by default, and a wave two that keeps
shrinking is a store page whose promise is thin. The line is drawn when the page is
written, not before — but it is drawn, and the four above are what it is drawn around.

---

## What only you can decide

Each with the recommendation, so the question is yes or no rather than open:

1. ~~Payout entity~~ — **decided: a Polish JDG**, W-8BEN at the tax interview. The one
   question left under it has a deadline rather than a decision: IP Box has to be set up
   from the start of the tax year, so ask the accountant before January rather than before
   launch.
2. ~~Early Access or a single 1.0~~ — **decided: Early Access**, with multiplayer in it
   from the first day. Decided the other way on 20 October and reversed the next day,
   which is the honest way round: the thing this game most needs is other people sailing
   it, and a 1.0 held back until the content list is finished is a year of nobody sailing
   it. See *Price and shape* for what that buys and what it costs.
3. **Price** — open, and deliberately late: it depends on how much of the content list is
   in the release.
4. ~~Platforms at launch~~ — **decided: Windows, macOS and Linux.**
5. **Multiplayer at launch** — yes, through the existing relay. Steam Networking after,
   when there is somebody to be matched with.
6. ~~Demo scope~~ — **decided: Sailing 101 plus one delivery, no campaign, no save.**
7. **The date** — open until the scope is frozen.
