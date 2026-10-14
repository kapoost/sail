# Sea trial: one passage, only invariants

A plan for the one kind of test this project does not have, written after a day in which
nineteen faults were found and fifteen of them were found by a person looking at the
screen.

## Why

The faults that cost the most that day were not wrong numbers. They were things that
*jumped*: a boom teleporting across the boat in one substep, a gybe flickering several
times a second, a headsail crossing the foredeck every time the wind wandered over the
stern, a sheet drawn to a corner the sail no longer had. Every one of them is invisible
to a steady-state test — the boat settles to the right answer either side of the event —
and every one of them is loud in a time series.

What they have in common is that none of them could have been predicted. Writing a test
per fault would have meant knowing the fault first. So this test asserts nothing about
what the boat *should* do and everything about what it must never do.

## What it is

One scripted passage, about ten minutes of simulated time, through every manoeuvre the
boat has:

1. leave on a beat, settle
2. tack, settle, tack back
3. bear away to a reach, ease
4. set the spinnaker, square the pole
5. gybe, settle, gybe back
6. rig the preventer, try to gybe again — and fail to
7. round up, douse the kite, roll out the genoa
8. take a reef, heave to

At roughly 120,000 substeps that is a fraction of a second without a window, which is
cheap enough to run on every `cargo test`.

## What it asserts

Nothing about speeds, angles or times. Only invariants, checked every substep:

| | |
|---|---|
| **finite** | no NaN or infinity in any state or telemetry field, ever |
| **nothing teleports** | the drawn boom moves < 2° per substep; the pole < 2°; `jib_side` < 0.05; every sail corner < 0.5 m |
| **nothing flickers** | the sign of `boom` changes exactly as many times as the script gybes |
| **the preventer holds** | zero sign changes while it is rigged, whatever the helm does |
| **physical bounds** | speed 0–12 kn, heel under 60°, boom within its own stops, sheets and furls in 0–1 |
| **no accidental mishap** | nothing aground, blown out or capsized in a passage that does not ask for it |

## What it will not catch

Anything about how it looks. That same day the spinnaker was variously shaped like a
Code 0, inflated backwards into the mast, hung from a head that wrapped it round the spar
and flattened out without flogging — four faults in a row, all found by eye, none of
which any number here would have noticed. This test is insurance against regression in
*motion*, not against a wrong shape.

It also says that something broke, not what. That is the price of not having to predict
the fault, and it is worth paying: a red line pointing at the substep where the boom
moved forty degrees is a morning's work saved even if it names nothing.

## How to keep it honest

* Invariants only. The first expected speed written into this file is the day it starts
  failing for reasons that are nobody's fault.
* The script is a *sailor's* sequence, not a coverage checklist. If a manoeuvre is in it,
  somebody would do it in that order.
* When it fails and the model was right, the invariant was wrong — widen it with a note
  saying what was measured, the way every band in the validation suite says.
