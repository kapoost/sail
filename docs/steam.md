# Selling her on Steam

A plan for putting this game in a shop, and for the content that goes with it. It is
written the same way `docs/online.md` was: the shape first, the detail that is missing
from it second, and an honest list of what only the owner can decide.

**The one-line version:** the model is finished enough to sell and the *game around it*
is not — three things stand between here and a shop window, none of them content, and
they should be built before a single new feature is.

---

## Where she stands today

Facts, not impressions:

| | |
|---|---|
| Model | 4 degrees of freedom at 200 Hz, 367 tests across the workspace |
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

## The three things that block a shop window

None of these is content. All three are the difference between "a thing you can run" and
"a thing somebody pays for".

### 1. The window is nailed to 1440 × 880

`main.rs` sets `resizable: false` and a fixed size, on purpose and for a good reason: the
instruments are drawn at known pixel sizes and a resizable window promises a layout the
picture cannot keep. On Steam that reasoning does not survive contact with the customer.
People expect full screen, their own resolution, and a game that fits a 1280 × 800 Steam
Deck.

Half the work is already done — `retro::console_scale` adapts the console to the physical
width, and the 3D picture is rendered at a fixed drawn width and scaled up anyway. What
is missing: a real video settings page (full screen / windowed / borderless, resolution,
drawn width, UI scale), the chart and the panels behaving at both extremes, and a pass
over every hard-coded panel size.

**Lands in:** `main.rs` (window), `retro.rs` (scale), `settings.rs` (the page),
`panel.rs`, `chart/`, `hud.rs` (layout at the extremes).

### 2. Nothing is ever saved

There is no code in the tree that writes a passage to disk. For a game whose clock runs
at one to one and whose legs are measured in nautical miles, that is not an inconvenience
— it is the reason somebody plays for twenty minutes and never comes back.

What it needs: the day's seed, the boat's state, the mission in progress, the logbook, the
settings, and a version stamp so an old save fails politely rather than strangely. One
slot is enough to start; Steam Cloud wants the same file.

**Lands in:** a new `crates/sail-app/src/anchorage.rs` (or `passage.rs`) plus `serde` on
the few state structs that cross the line, `settings.rs` for the UI.

### 3. No pause, no time compression

`main.rs` says it plainly: the clock runs at one to one. `course.rs` lays legs on that
assumption. Forty miles at five knots is eight hours, and the README already admits this
has to come back.

The honest version for a sailing game is not a speed slider but a **watch below**: ×1, ×4,
×16, available only when nothing needs a hand — no squall, no land inside a mile, no
mission step waiting on you — and dropped to ×1 the moment something does. That is both
more defensible and more interesting than a slider, and it uses the hazard tests the model
already has.

**Lands in:** `sim.rs` (the step), `mission.rs` and `navigation.rs` (what counts as
"needs a hand"), `hud.rs` (showing it), plus a test per interrupting condition.

---

## The platform matrix

| Platform | Today | For Steam |
|---|---|---|
| Windows x86-64 | Built in CI | Ship it |
| macOS universal | Built in CI | Ship it — **but it must be signed and notarised**, see risks |
| Linux x86-64 | Server only | Add the game to `release.yml`; needed for a native Deck build |
| Steam Deck | Untested | Controller is already in; needs item 1, then a verification pass |

The cheap path to the Deck is Proton running the Windows build — worth testing on day one,
because if it works the native Linux depot becomes a nice-to-have rather than a blocker.
The frame report (`?frames=1` / `SAIL_FRAMES=1`) and `dist/tab.py`'s measuring loop already
give us the numbers to decide whether the Deck needs a lower drawn width by default.

---

## Steamworks: what to take and what to leave

**Take, in this order:**

1. **Depots and SteamPipe.** Three depots (Windows, macOS, Linux), one app, uploaded from
   CI on a tag — the same trigger that already cuts a GitHub release.
2. **Cloud saves.** Free once item 2 above exists; it is a file path in a config.
3. **Achievements.** Cheap and they shape a first hour: finish Sailing 101, make a first
   delivery, come through a gale with the rig standing, anchor overnight, name your
   position with three bearings, land a fish. Ten to fifteen, no grind.

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

**Recommendation: Early Access, $14.99 (about 59 zł), moving to $19.99 at 1.0.**

The reasoning: the thing that is usually unfinished in Early Access — the simulation — is
the thing that is finished here, and the thing that is usually finished — content and
comfort — is what is missing. That is an honest Early Access and it reads well on the
page: *the boat is done, the world around her is growing, here is the list and the order.*

A regional price for PL below the automatic conversion is worth setting by hand.

---

## Content: what this engine makes cheap

Ordered by what it buys against what it costs. Everything in the first five rows is
mathematics and prose, which is exactly where this project is strong — no new art.

| Content | Why it is cheap here | What it buys | Size |
|---|---|---|---|
| **Tides and tidal streams** | Pure maths over the existing archipelago; drift already exists as a vector | Planning: a tidal window, a stream in a sound, a passage that has to leave at the right hour | 1–2 weeks |
| **Night** | The lighthouses already carry their flash characteristics (`Fl(2) 8s`), the sky model already has a sun | Pilotage by light alone — almost nobody does this, and it is the best screenshot in the game | 2 weeks |
| **Fog and visibility** | Haze and visibility already exist; sound is synthesised | Blind pilotage: the sounder, the clock, a sound signal, dead reckoning that matters | 1 week |
| **Weather that moves** | The weather is drawn per day; make it a front crossing | A veering, backing, building day — the thing that makes a passage a story | 1–2 weeks |
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
* **Tax interview:** required before payouts. A Polish company files W-8BEN-E, a sole
  trader W-8BEN; the Poland–US treaty takes withholding on game sales to zero when it is
  filled in correctly. **Decide the payout entity before opening the account** — moving it
  afterwards is paperwork.
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
| **Scope creep** | The content list above is longer than one Early Access | Freeze the EA scope at the first three rows and ship |
| **One person** | Every row in this plan is the same pair of hands | The order in the milestones is chosen so that stopping after any of them still leaves something shippable |

---

## Milestones

Each ends with a sentence that is either true or false — no "mostly done".

**M0 — Decisions.** The list at the bottom of this document is answered. *Ends when the
Steam partner account exists and the app is created.*

**M1 — A game you can leave and come back to.** Window and video settings, save and
resume, pause and the watch below. *Ends when somebody can sail for twenty minutes, quit
mid-passage, and pick it up the next evening at the right resolution.*

**M2 — The shop skeleton.** Depots, CI upload, cloud saves, ten achievements, store page
live, wishlists open, demo app built from the same binary. *Ends when a stranger can
wishlist it and play the demo.*

**M3 — Content wave one.** Tides, night, fog. *Ends when a night entry into a strange
harbour in poor visibility is a thing you can do and want to tell somebody about.*

**M4 — Early Access launch.** Trailer, capsules, price, date announced four to six weeks
ahead. *Ends on the day it is for sale.*

**M5 — Content wave two → 1.0.** Weather that moves, failures, anchoring, a second boat,
the campaign. *Ends when the Early Access promise on the store page has been kept.*

---

## What only you can decide

Each with the recommendation, so the question is yes or no rather than open:

1. **Payout entity** — company or sole trader. *Recommendation: the one whose accountant
   already handles foreign revenue; decide before the account is opened.*
2. **Early Access or a single 1.0** — *Recommendation: Early Access, for the reason given
   above.*
3. **Price** — *Recommendation: $14.99 in Early Access, $19.99 at 1.0, PL set by hand.*
4. **Platforms at launch** — *Recommendation: Windows and macOS native, Deck through
   Proton, native Linux when the Deck numbers say it is worth it.*
5. **Multiplayer in Early Access?** — *Recommendation: yes, as it is, through the existing
   relay and documented as LAN-first. Steam Networking after launch.*
6. **Demo scope** — *Recommendation: Sailing 101 plus one delivery, no campaign, no save.*
7. **The quarter you are aiming at** — everything above is weeks of work by one person; the
   date decides how much of the content list is inside Early Access and how much is after
   it.
