//! One passage, and only the things that must never happen.
//!
//! The plan is `docs/sea-trial.md` and the reason for it is in there: of nineteen faults
//! found in a day, fifteen were found by a person looking at the screen, and the
//! expensive ones were not wrong numbers — they were things that **jumped**. A boom
//! teleporting across the boat in one substep. A gybe flickering several times a second.
//! A headsail crossing the foredeck every time the wind wandered over the stern. Every
//! one of those is invisible to a steady-state test, because the boat settles to the
//! right answer either side of the event, and every one of them is loud in a time series.
//!
//! So this asserts nothing about what the boat *should* do and everything about what it
//! must never do: nothing infinite, nothing teleporting, nothing flickering, nothing
//! outside the bounds of a boat. It cannot say what broke — only that something did, at
//! this substep, in this manoeuvre. That is the price of not having to predict the fault
//! first, and a red line pointing at the substep where the boom moved forty degrees is a
//! morning saved even if it names nothing.
//!
//! The script is a sailor's sequence rather than a coverage checklist: leave on a beat,
//! tack and tack back, bear away, set the kite, gybe and gybe back, rig the preventer and
//! fail to gybe, round up, douse, roll the genoa out, reef, heave to. Ten minutes of
//! simulated time, which is a fraction of a second here.

use sail_core::autopilot::{optimal_sheets_with, TillerPilot};
use sail_core::boat::BoatConfig;
use sail_core::sim::{Simulation, SUBSTEP};
use sail_core::state::{BoatState, Controls, Headsail};
use sail_core::units::{deg, kn_to_ms, ms_to_kn, rad, wrap_pi, wrap_tau};
use sail_core::water::WindSea;
use sail_core::wind::SteadyWind;

/// A leg of the passage: what she is doing, for how long, and what the hands do.
struct Leg {
    what: &'static str,
    seconds: f64,
    /// Where to steer, as a compass course.
    steer: f64,
    /// And what to do with everything that is not the helm.
    hands: fn(&mut Controls),
}

/// How much the drawn spars may move between two substeps.
///
/// Two degrees in five milliseconds is four hundred degrees a second, which is far more
/// than anything on a boat does and far less than a teleport. The boom crossing on a gybe
/// is governed at forty-five degrees a second — a fifth of a degree a substep — so this
/// has two orders of magnitude of room before it complains about something real.
const NO_TELEPORT: f64 = 2.0;

#[test]
fn a_passage_in_which_nothing_jumps() {
    let cfg = BoatConfig::charter_sloop_34();
    let wind = 12.0;
    // Wind from the north, so a compass course is also an angle to the wind and the
    // script reads like a plan rather than like arithmetic.
    let close_hauled = 45.0;
    let legs = [
        Leg {
            what: "leave on a beat",
            seconds: 70.0,
            steer: close_hauled,
            hands: |c| {
                c.reef = 0.0;
                c.headsail = Headsail::Genoa;
                c.furl = 0.0;
                c.headsail_aback = false;
                c.preventer = false;
            },
        },
        Leg {
            what: "tack onto port",
            seconds: 45.0,
            steer: -close_hauled,
            hands: |_| {},
        },
        Leg {
            what: "settle on port",
            seconds: 45.0,
            steer: -close_hauled,
            hands: |_| {},
        },
        Leg {
            what: "tack back",
            seconds: 45.0,
            steer: close_hauled,
            hands: |_| {},
        },
        Leg {
            what: "bear away to a reach",
            seconds: 55.0,
            steer: 95.0,
            hands: |_| {},
        },
        Leg {
            what: "set the kite",
            seconds: 55.0,
            steer: 140.0,
            hands: |c| {
                c.headsail = Headsail::Spinnaker;
                c.furl = 0.0;
            },
        },
        Leg {
            what: "run",
            seconds: 45.0,
            steer: 155.0,
            hands: |_| {},
        },
        Leg {
            what: "gybe",
            seconds: 55.0,
            steer: 205.0,
            hands: |_| {},
        },
        Leg {
            what: "settle on the new gybe",
            seconds: 45.0,
            steer: 205.0,
            hands: |_| {},
        },
        Leg {
            what: "gybe back",
            seconds: 55.0,
            steer: 155.0,
            hands: |_| {},
        },
        Leg {
            what: "preventer on, and try to gybe",
            seconds: 70.0,
            steer: 205.0,
            hands: |c| c.preventer = true,
        },
        Leg {
            what: "round up and douse",
            seconds: 45.0,
            steer: 120.0,
            hands: |c| {
                c.preventer = false;
                c.furl = 1.0;
            },
        },
        Leg {
            what: "roll the genoa out",
            seconds: 45.0,
            steer: 70.0,
            hands: |c| {
                c.headsail = Headsail::Genoa;
                c.furl = 0.0;
            },
        },
        Leg {
            what: "take a reef",
            seconds: 45.0,
            steer: close_hauled,
            hands: |c| c.reef = 0.5,
        },
        Leg {
            what: "heave to",
            seconds: 60.0,
            steer: close_hauled - 25.0,
            hands: |c| c.headsail_aback = true,
        },
    ];

    let heading = wrap_tau(rad(close_hauled));
    let mut state = BoatState::at_rest(heading);
    state.surge = kn_to_ms(4.0);
    let mut sim = Simulation::new(cfg, state, Box::new(SteadyWind::new(0.0, kn_to_ms(wind))))
        // A real sea under her, because half of what jumps only jumps when something is
        // throwing her about. Twenty kilometres of fetch is a day in soundings rather than
        // an ocean.
        .with_water(Box::new(WindSea::new(kn_to_ms(wind), 20_000.0, 0.0, 11)));
    let mut helm = TillerPilot::new();
    let mut controls = Controls {
        outhaul: 0.5,
        ..Default::default()
    };

    // What the last substep looked like, for the things that are about *change* rather
    // than about value.
    let mut was = (0.0_f64, 0.0_f64, 0.0_f64);
    let mut started = false;
    // Three counts of the same event, because "the boom went over" turned out to mean
    // three different things. A *crossing* is the boom past ten degrees the other way. A
    // *gybe* is a crossing that was not part of the previous one — the boom slammed
    // across by a released preventer swings past the centreline and settles back a
    // second or two later, and that is one event seen from the deck, not two. *Chatter*
    // is crossings inside half a second of each other, which is the fault this test was
    // written for: a boom in a confused sea changing sides several times a second.
    let mut crossings = 0u32;
    let mut gybes = 0u32;
    let mut chatter = 0u32;
    let mut last_crossing = f64::NEG_INFINITY;
    let mut jib_flips = 0u32;
    let mut preventer_flips = 0u32;
    // Which side things are *settled* on, with a dead band round the centreline.
    //
    // Counting sign changes of a quantity that legitimately sits at nought is the classic
    // way to measure nothing, and the first version of this test did it: going through a
    // tack the boom slats about the centreline by a fraction of a degree, and that came
    // out as five crossings in one second and read like the very flicker this test is
    // for. It was the sail doing what a sail does. A boom has gone over when it has
    // *gone over* — ten degrees the other way — and that is what a person sees.
    let mut boom_side = 0.0_f64;
    let mut jib_settled = 0.0_f64;
    let mut worst = (0.0_f64, "", 0.0_f64);
    let mut elapsed = 0.0;

    for leg in &legs {
        (leg.hands)(&mut controls);
        let course = wrap_tau(rad(leg.steer));
        let steps = (leg.seconds / SUBSTEP) as u64;
        for _ in 0..steps {
            let (m, j) =
                optimal_sheets_with(&cfg, sim.telemetry.awa, controls.outhaul, controls.headsail);
            controls.main_sheet = m;
            controls.jib_sheet = j;
            controls.rudder = helm.rudder(&cfg, &sim.state, course, SUBSTEP);
            sim.run(SUBSTEP, &controls);
            elapsed += SUBSTEP;

            let t = &sim.telemetry;
            let s = &sim.state;

            // --- Nothing infinite ------------------------------------------------
            for (name, v) in [
                ("surge", s.surge),
                ("sway", s.sway),
                ("heading", s.heading),
                ("heel", s.heel),
                ("yaw_rate", s.yaw_rate),
                ("boom_angle", s.boom_angle),
                ("boom_rate", s.boom_rate),
                ("pole_angle", s.pole_angle),
                ("jib_across", s.jib_across),
                ("stw", t.stw),
                ("awa", t.awa),
                ("twa", t.twa),
                ("aws", t.aws),
                ("boom", t.boom),
                ("pole", t.pole),
                ("jib_side", t.jib_side),
                ("drive", t.drive),
                ("side", t.side),
                ("strain", t.strain),
            ] {
                assert!(
                    v.is_finite(),
                    "{name} is {v} at {elapsed:.1} s, during \"{}\"",
                    leg.what
                );
            }

            // --- Nothing outside the bounds of a boat -----------------------------
            let knots = ms_to_kn(t.stw);
            assert!(
                (-2.0..12.0).contains(&knots),
                "she is doing {knots:.1} knots at {elapsed:.1} s, during \"{}\"",
                leg.what
            );
            assert!(
                deg(s.heel.abs()) < 60.0,
                "she is at {:.0} degrees of heel at {elapsed:.1} s, during \"{}\"",
                deg(s.heel.abs()),
                leg.what
            );
            assert!(
                deg(t.boom.abs()) < 110.0,
                "the boom is {:.0} degrees off the centreline at {elapsed:.1} s",
                deg(t.boom.abs())
            );
            assert!((0.0..=1.0).contains(&controls.main_sheet));
            assert!((0.0..=1.0).contains(&controls.jib_sheet));
            assert!(
                t.mishap.is_none(),
                "{:?} at {elapsed:.1} s, during \"{}\" — a passage that does not ask for \
                 one",
                t.mishap,
                leg.what
            );

            // --- And nothing teleports --------------------------------------------
            if started {
                for (name, now, before) in [("boom", t.boom, was.0), ("pole", t.pole, was.1)] {
                    let moved = deg(wrap_pi(now - before).abs());
                    if moved > worst.0 {
                        worst = (moved, name, elapsed);
                    }
                    assert!(
                        moved < NO_TELEPORT,
                        "the {name} moved {moved:.1} degrees in one substep at \
                         {elapsed:.1} s, during \"{}\"",
                        leg.what
                    );
                }
                // The headsail's side is allowed to change in a step — a symmetric
                // kite's clew *is* on the boom's side, and the boom finishes its crossing
                // in one. What neither of them may do is chatter, which is counted
                // outside the dead band rather than bounded inside it.
                let settled = |now: f64, was: f64, band: f64| {
                    if now > band {
                        1.0
                    } else if now < -band {
                        -1.0
                    } else {
                        was
                    }
                };
                let jib_now = settled(t.jib_side, jib_settled, 0.5);
                if jib_now != jib_settled && jib_settled != 0.0 {
                    jib_flips += 1;
                }
                jib_settled = jib_now;

                let boom_now = settled(t.boom, boom_side, rad(10.0));
                if boom_now != boom_side && boom_side != 0.0 {
                    crossings += 1;
                    // One slam, however many times it passes the centreline. The boom
                    // is governed at forty-five degrees a second, so a traverse of up to
                    // a hundred and eighty degrees takes four seconds and a slam that
                    // goes over and comes back takes eight — which is exactly what a
                    // preventer let go at dead run with the helm hard over does, and it
                    // is one event seen from the deck. The fault this test is for looks
                    // nothing like it: several crossings a second, counted below as
                    // chatter.
                    const SAME_SWING: f64 = 8.0;
                    let since = elapsed - last_crossing;
                    if since > SAME_SWING {
                        gybes += 1;
                    }
                    if since < 0.5 {
                        chatter += 1;
                    }
                    last_crossing = elapsed;
                    if std::env::var("TRACE").is_ok() {
                        println!(
                            "   boom went over at {elapsed:6.1} s during \"{}\" (twa {:+.0}, boom {:+.0}, roll {:+.0}, preventer {})",
                            leg.what,
                            deg(t.twa),
                            deg(t.boom),
                            deg(sim.state.heel),
                            controls.preventer
                        );
                    }
                    if controls.preventer {
                        preventer_flips += 1;
                    }
                }
                boom_side = boom_now;
            }
            if std::env::var("WINDOW").is_ok() && (555.0..600.0).contains(&elapsed) {
                let step = (elapsed * 4.0).round() / 4.0;
                if (elapsed - step).abs() < 0.003 {
                    println!(
                        "{elapsed:7.2} {} twa {:+6.1} boom {:+6.1} sheet {:.2} heel {:+5.1} \
                         yaw rate {:+5.1} preventer {}",
                        leg.what,
                        deg(t.twa),
                        deg(t.boom),
                        controls.main_sheet,
                        deg(sim.state.heel),
                        deg(sim.state.yaw_rate),
                        controls.preventer
                    );
                }
            }
            was = (t.boom, t.pole, t.jib_side);
            started = true;
        }
    }

    println!(
        "ten minutes: {gybes} gybes in {crossings} crossings, {chatter} of them chatter; \
         the headsail changed sides {jib_flips} times; worst spar movement {:.2} deg in a \
         substep ({} at {:.0} s)",
        worst.0, worst.1, worst.2
    );

    // --- Nothing flickers -----------------------------------------------------
    //
    // The script puts the boom across the centreline five times: the two tacks, the two
    // gybes, and the held gybe finishing when the preventer is let go on the leg after
    // it. That fifth one was missing from the count here and from the boat — she failed
    // to gybe when asked, which left the budget looking generous and hid the omission.
    // Anything above five is the flicker this test exists to catch: before it was fixed,
    // a boom in a confused sea changed sides several times a second.
    assert!(
        gybes <= 5,
        "the boom went over {gybes} times in a passage that asks for five"
    );
    assert_eq!(
        chatter, 0,
        "the boom crossed {chatter} times inside half a second of another crossing, which \
         is the flicker rather than a swing"
    );
    assert!(
        crossings <= 2 * gybes,
        "{crossings} crossings for {gybes} gybes: a slammed boom may swing past the \
         centreline and back once, and not more"
    );
    assert!(
        jib_flips <= 8,
        "the headsail changed sides {jib_flips} times in a passage with four turns in it"
    );

    // --- The preventer holds ---------------------------------------------------
    assert_eq!(
        preventer_flips, 0,
        "the boom crossed {preventer_flips} times with the preventer rigged, which is the \
         one thing that button is for"
    );
}
