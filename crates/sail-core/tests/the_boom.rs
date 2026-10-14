//! The one thing the deck noticed twice.

use sail_core::prelude::*;

/// The drawn boom must never jump, however fast the wind line is crossed.
///
/// Reported from the deck as "it still jumps", and then — the observation that found it —
/// "maybe it depends on how fast you cross the wind line; by hand on the helm I can catch
/// the transit I want". Which was exactly right. The simulation flips the boom's side ten
/// degrees *past* dead astern, with hysteresis so that a boat squared off downwind does
/// not flicker; the drawing was re-deriving the side from the raw apparent wind, which
/// flips at a hundred and eighty. For as long as the apparent wind spent between those
/// two angles the two disagreed and the drawn spar was on the wrong side of the boat.
/// Crossed slowly that is a second of something odd; crossed hard it is a teleport.
///
/// So this measures what the renderer sees, substep by substep, through the hardest gybe
/// the boat can be made to do: helm hard over, in a seaway, at two wind speeds.
#[test]
fn the_drawn_boom_never_jumps() {
    for (tws_kn, rudder) in [(10.0, 35.0), (16.0, 35.0)] {
        let cfg = BoatConfig::archipelago_packet();
        let tws = kn_to_ms(tws_kn);
        let sea = sail_core::water::WindSea::new(tws, 14_000.0, 0.0, 0xC0FFEE);
        let mut sim = Simulation::new(
            cfg,
            BoatState::at_rest(wrap_tau(0.0 - rad(165.0))),
            Box::new(SteadyWind::new(0.0, tws)) as Box<dyn WindField>,
        )
        .with_water(Box::new(sea));
        let mut controls = Controls {
            main_sheet: 0.95,
            jib_sheet: 0.85,
            ..Default::default()
        };
        for _ in 0..(60.0 / SUBSTEP) as u64 {
            let off = deg(sim.telemetry.twa) - 165.0;
            controls.rudder = rad((off * 0.5).clamp(-25.0, 25.0));
            sim.run(SUBSTEP, &controls);
        }
        controls.rudder = rad(-rudder);
        let (mut worst, mut at, mut was, mut t) = (0.0f64, 0.0, sim.telemetry.boom, 0.0);
        while t < 12.0 {
            sim.run(SUBSTEP, &controls);
            t += SUBSTEP;
            let now = sim.telemetry.boom;
            let step = deg(now - was).abs();
            if step > worst {
                worst = step;
                at = t;
            }
            was = now;
        }
        // At two hundred substeps a second, half a degree a step is a hundred degrees a
        // second — the fastest part of a real crossing. Two degrees a step would be four
        // hundred, which no spar does; a hundred and thirty-six in one step is the fault
        // this is here for.
        assert!(
            worst < 2.0,
            "{tws_kn:.0} kn: the drawn boom moved {worst:.1} degrees in one substep at \
             {at:.2}s, which is a jump and not a swing"
        );
    }
}

/// A boat that turns sweeps her own mainsail sideways through the air, and the sail
/// notices.
///
/// Asked for from the deck in one line: *the boom could have a little movement from air
/// resistance when the yacht is turning — after all, the sail's area does that*. It is
/// the same damping that was already there, asked a better question. It used to be told
/// how fast the boom was moving relative to the *deck*, and thirty square metres of cloth
/// two metres out from the mast does not care what the deck is doing: when the boat yaws,
/// the whole sail is swept through the air whether the boom moves over the deck or not.
///
/// What comes out is small and should be: about a degree, which at the end of a four
/// metre boom is seven centimetres. That is what it looks like on the water — the boom
/// lags, the leech shakes, and it settles again — and it is larger in more wind because
/// the air is what is doing it.
#[test]
fn the_boom_feels_the_boat_turning_under_it() {
    let swing = |tws_kn: f64| -> f64 {
        let cfg = BoatConfig::archipelago_packet();
        let tws = kn_to_ms(tws_kn);
        let mut sim = Simulation::new(
            cfg,
            BoatState::at_rest(wrap_tau(0.0 - rad(110.0))),
            Box::new(SteadyWind::new(0.0, tws)) as Box<dyn WindField>,
        );
        let mut controls = Controls {
            main_sheet: 0.55,
            jib_sheet: 0.45,
            ..Default::default()
        };
        for _ in 0..(50.0 / SUBSTEP) as u64 {
            let off = deg(sim.telemetry.twa) - 110.0;
            controls.rudder = rad((off * 0.5).clamp(-20.0, 20.0));
            sim.run(SUBSTEP, &controls);
        }
        let settled = sim.state.boom_angle;
        controls.rudder = rad(30.0);
        let (mut most, mut t) = (0.0f64, 0.0);
        while t < 6.0 {
            sim.run(SUBSTEP, &controls);
            t += SUBSTEP;
            most = most.max(deg(sim.state.boom_angle - settled).abs());
        }
        most
    };

    let light = swing(8.0);
    let fresh = swing(16.0);
    assert!(
        light > 0.3,
        "the helm hard over should move the boom at all: {light:.2} deg"
    );
    assert!(
        light < 4.0 && fresh < 4.0,
        "but it is held by its sheet, not loose: {light:.2} and {fresh:.2} deg"
    );
    assert!(
        fresh > light * 1.3,
        "and the air is what does it, so more wind moves it more: {fresh:.2} against \
         {light:.2}"
    );
}

/// With the vang eased on a broad reach, does the boom lift? Asked from the deck, and
/// the answer is a number rather than a yes.
///
/// The end of the spar riding up is the twist seen from the other end: the leech falls
/// off *because* the boom has lifted, so the picture takes a third of the twist angle as
/// the lift and the two cannot disagree. What this asserts is that the vang is a control
/// with a visible range — eased, the end of the boom is up where you can see it; hauled
/// on, it is where it was put.
#[test]
fn an_eased_vang_lets_the_boom_lift_off_the_wind() {
    let twist_at = |twa: f64, sheet: f64, vang: f64| -> f64 {
        let cfg = BoatConfig::archipelago_packet();
        let mut start = BoatState::at_rest(wrap_tau(0.0 - rad(twa)));
        start.surge = kn_to_ms(4.0);
        let mut sim = Simulation::new(
            cfg,
            start,
            Box::new(SteadyWind::new(0.0, kn_to_ms(14.0))) as Box<dyn WindField>,
        );
        let mut c = Controls {
            main_sheet: sheet,
            jib_sheet: 0.6,
            vang,
            ..Default::default()
        };
        for _ in 0..(50.0 / SUBSTEP) as u64 {
            let off = deg(sim.telemetry.twa) - twa;
            c.rudder = rad((off * 0.5).clamp(-15.0, 15.0));
            sim.run(SUBSTEP, &c);
        }
        sim.telemetry.twist
    };
    // A third of the twist, which is what `boat::BOOM_LIFT_PER_TWIST` draws.
    let lift = |twist: f64| 4.6 * (twist * 0.34).sin();

    let eased = twist_at(135.0, 0.8, 0.0);
    let hauled = twist_at(135.0, 0.8, 1.0);
    assert!(
        lift(eased) > 0.3,
        "broad reach, vang off: the end of the boom should be a visible distance up, \
         and it is {:.2} m",
        lift(eased)
    );
    assert!(
        lift(hauled) < 0.05,
        "and hauled on it should be back down: {:.2} m",
        lift(hauled)
    );
    // Deeper still and it is more, because the sheet has stopped pulling down at all.
    let running = twist_at(160.0, 0.95, 0.0);
    assert!(
        lift(running) > lift(eased),
        "running with the vang off should lift it further: {:.2} against {:.2} m",
        lift(running),
        lift(eased)
    );
}

/// A preventer stops the gybe, which is the whole of what it is for.
///
/// Asked for from the deck as a toggle that would be useful running downwind, and it is
/// the one line on a cruising boat that exists to stop something rather than to make it
/// happen: led from the boom end forward to the rail and made fast. With it on, the wind
/// crossing the stern backs the mainsail and leaves it backed — she rounds up, which is
/// unpleasant and survivable — and the boom stays exactly where it was put.
#[test]
fn a_preventer_keeps_the_boom_where_it_was_put() {
    let gybe_with = |preventer: bool| -> f64 {
        let cfg = BoatConfig::archipelago_packet();
        let mut sim = Simulation::new(
            cfg,
            BoatState::at_rest(wrap_tau(0.0 - rad(165.0))),
            Box::new(SteadyWind::new(0.0, kn_to_ms(12.0))) as Box<dyn WindField>,
        );
        let mut controls = Controls {
            main_sheet: 0.95,
            jib_sheet: 0.85,
            preventer,
            ..Default::default()
        };
        for _ in 0..(60.0 / SUBSTEP) as u64 {
            let off = deg(sim.telemetry.twa) - 165.0;
            controls.rudder = rad((off * 0.5).clamp(-25.0, 25.0));
            sim.run(SUBSTEP, &controls);
        }
        let before = sim.telemetry.boom;
        controls.rudder = rad(-18.0);
        for _ in 0..(20.0 / SUBSTEP) as u64 {
            sim.run(SUBSTEP, &controls);
        }
        // Positive if the boom is still on the side it started, negative if it crossed.
        sim.telemetry.boom * before.signum()
    };

    assert!(
        gybe_with(false) < 0.0,
        "without one she gybes, which is what the last commit was about"
    );
    assert!(
        gybe_with(true) > 0.0,
        "with one rigged the boom stays where it was, and she has to be sailed out of it"
    );
}

/// Dead downwind, in a seaway, for three minutes: she stays on the gybe she is on.
///
/// Reported from the deck as jumping between gybes continuously, with the instruction to
/// think about it like a sailor — which turned out to be the whole answer. A sailor does
/// not work out which gybe they are on; the boom is out to port, therefore starboard
/// tack, and it stays that way until the spar physically comes across. The model was
/// deciding it from the *sign of the apparent wind*, which at a hundred and eighty is
/// decided by whichever wave went under her last, so it flickered — and a model that
/// flickers between gybes several times a second is not a boat.
///
/// Two things fixed it and both are sailing rather than arithmetic. The boom's angle is
/// in the boat's own frame, so which gybe she is on is simply where the spar is and there
/// is nothing to flip. And a mainsail is not backed the moment the wind crosses the
/// stern: she can be sailed a good way by the lee with it still drawing, and it is only
/// when the wind gets far enough behind the leech that the sail backs and the boom goes.
#[test]
fn she_holds_her_gybe_dead_downwind() {
    for kite in [false, true] {
        let cfg = BoatConfig::archipelago_packet();
        let tws = kn_to_ms(12.0);
        let sea = sail_core::water::WindSea::new(tws, 14_000.0, 0.0, 0xC0FFEE);
        let mut start = BoatState::at_rest(wrap_tau(0.0 - rad(178.0)));
        start.surge = kn_to_ms(4.0);
        let mut sim = Simulation::new(
            cfg,
            start,
            Box::new(SteadyWind::new(0.0, tws)) as Box<dyn WindField>,
        )
        .with_water(Box::new(sea));
        let mut controls = Controls {
            headsail: if kite {
                sail_core::state::Headsail::Spinnaker
            } else {
                sail_core::state::Headsail::Genoa
            },
            main_sheet: 0.95,
            jib_sheet: 0.8,
            furl: 0.0,
            ..Default::default()
        };
        let want = sim.state.heading;
        let (mut gybes, mut t) = (0, 0.0);
        let mut was = sim.telemetry.boom.signum();
        while t < 180.0 {
            let off = wrap_pi(sim.state.heading - want);
            controls.rudder = rad((deg(off) * -0.8).clamp(-20.0, 20.0));
            sim.run(SUBSTEP, &controls);
            t += SUBSTEP;
            let now = sim.telemetry.boom.signum();
            if now != was {
                gybes += 1;
            }
            was = now;
        }
        // One is the boom settling on to its side at the start. Two in three minutes
        // would be a boat gybing herself every ninety seconds, which is a boat nobody
        // would sail; the fault this is guarding against was several a second.
        assert!(
            gybes <= 1,
            "three minutes dead downwind and she gybed {gybes} times with the kite {kite}"
        );
        // And she is still pointing where she was put.
        assert!(
            deg(sim.telemetry.twa).abs() > 165.0,
            "she should still be running: twa {:.0}",
            deg(sim.telemetry.twa)
        );
    }
}

/// How far by the lee she will sail before the main backs and she gybes.
///
/// Asked for from the deck as a wider angle under the kite, and it is a real asymmetry
/// between the two ways of running: a cruising main eased out with a soft vang invites
/// the wind round the back of its open leech early, and a boat with a pole up and the
/// vang on will sit there with the wind a long way round and nothing happening. That is
/// not a trick — it is how you sail deep with a symmetric, and racers live there.
///
/// Measured by steering her past dead downwind half a degree at a time and asking where
/// the spar went.
#[test]
fn a_kite_lets_her_sail_further_by_the_lee() {
    let gybes_at = |headsail: sail_core::state::Headsail, vang: f64| -> f64 {
        let cfg = BoatConfig::archipelago_packet();
        let mut start = BoatState::at_rest(wrap_tau(0.0 - rad(170.0)));
        start.surge = kn_to_ms(4.0);
        let mut sim = Simulation::new(
            cfg,
            start,
            Box::new(SteadyWind::new(0.0, kn_to_ms(12.0))) as Box<dyn WindField>,
        );
        let mut controls = Controls {
            headsail,
            main_sheet: 0.95,
            jib_sheet: 0.8,
            vang,
            furl: 0.0,
            ..Default::default()
        };
        for _ in 0..(40.0 / SUBSTEP) as u64 {
            let off = deg(sim.telemetry.twa) - 170.0;
            controls.rudder = rad((off * 0.5).clamp(-20.0, 20.0));
            sim.run(SUBSTEP, &controls);
        }
        let side = sim.telemetry.boom.signum();
        let mut lee = 0.0;
        while lee < 45.0 {
            lee += 0.5;
            let aim = -(180.0 - lee);
            for _ in 0..(2.0 / SUBSTEP) as u64 {
                let off = wrap_pi(sim.telemetry.twa - rad(aim));
                controls.rudder = rad((deg(off) * 0.4).clamp(-20.0, 20.0));
                sim.run(SUBSTEP, &controls);
                if sim.telemetry.boom.signum() != side {
                    return lee;
                }
            }
        }
        lee
    };

    let working = gybes_at(sail_core::state::Headsail::Genoa, 0.6);
    let kite = gybes_at(sail_core::state::Headsail::Spinnaker, 0.6);
    let kite_vanged = gybes_at(sail_core::state::Headsail::Spinnaker, 1.0);

    // A working rig gybes when you sail a little past dead downwind, which is what makes
    // an accidental gybe a thing that happens to people.
    assert!(
        (8.0..26.0).contains(&working),
        "under the genoa she should gybe soon after dead downwind: {working:.0} deg"
    );
    // And under the pole she will sit there.
    assert!(
        kite > working * 2.0,
        "the kite should hold her much further round: {kite:.0} against {working:.0} deg"
    );
    // The vang is the other half of it, and the smaller half.
    assert!(
        kite_vanged >= kite,
        "and vanging on should not make it worse: {kite_vanged:.0} against {kite:.0}"
    );
}
