//! Getting out of irons, which is the one piece of boat handling that has no helm in it.
//!
//! Reported from the deck, in a gale, under a reefed main and a staysail with the genoa
//! rolled away: head to wind, a knot and a half of way on, the helm hard over, and she
//! would not fall off either side. The autopilot could not do it and the vane could not
//! do it. The question was *what is blocking her* — which is exactly the question a
//! bench answers and an argument does not.
//!
//! There are only four things it can be, and each one is a run below:
//!
//! * she is blocked by **the helm**, which at forty degrees is a brake and not a rudder
//!   and at a knot and a half has nothing going over it anyway;
//! * she is blocked by **the main**, which is abaft the mast and holds her head to wind
//!   the way a weathervane holds a church spire;
//! * she is blocked because **nothing is backed** — a sail held to windward is the only
//!   thing aboard that pushes the bow off with no way on at all;
//! * or she is blocked by the model, which is the answer this file exists to rule in or
//!   out.
//!
//! Run it with the numbers showing:
//!
//! ```text
//! cargo test -p sail-core --test in_irons -- --nocapture
//! ```

use sail_core::autopilot::optimal_sheets_with;
use sail_core::boat::BoatConfig;
use sail_core::sim::{Simulation, SUBSTEP};
use sail_core::state::{BoatState, Controls, Headsail};
use sail_core::units::{deg, kn_to_ms, ms_to_kn, rad, wrap_pi, wrap_tau};
use sail_core::water::WindSea;
use sail_core::wind::SyntheticWind;

/// Twenty-nine knots, which is what was on the instruments: a whole gale by the time it
/// is gusting, and the wind in which a missed tack actually happens.
const BLOWING: f64 = 29.0;

/// What she was doing when it was reported: stopped, head to wind, on the starboard bow.
fn in_irons() -> BoatState {
    // Wind from the north, so a heading of fifteen degrees is fifteen degrees off the
    // wind on the starboard bow — the angle on the screen.
    let mut state = BoatState::at_rest(wrap_tau(rad(15.0)));
    state.surge = kn_to_ms(1.5);
    state
}

/// Her deep-reefed storm rig: one reef and a bit, genoa rolled away, staysail set.
fn storm_rig() -> Controls {
    Controls {
        outhaul: 0.8,
        reef: 0.33,
        headsail: Headsail::Genoa,
        furl: 1.0,
        staysail: 1.0,
        staysail_sheet: 0.65,
        ..Default::default()
    }
}

/// Sail her for a while and report where her bow got to.
///
/// `hands` is given the controls and the apparent wind angle every substep, so a run can
/// be "the helm hard over and nothing else" or "ease the main right out" or "back it".
fn try_to_fall_off(what: &str, seconds: f64, hands: impl Fn(&mut Controls, f64)) -> (f64, f64) {
    let cfg = BoatConfig::charter_sloop_34();
    let mut sim = Simulation::new(
        cfg,
        in_irons(),
        Box::new(SyntheticWind::new(0.0, kn_to_ms(BLOWING), 5)),
    )
    .with_water(Box::new(WindSea::new(kn_to_ms(BLOWING), 60_000.0, 0.0, 11)));
    let mut controls = storm_rig();
    let steps = (seconds / SUBSTEP) as u64;
    let mut worst_off = 0.0_f64;
    for _ in 0..steps {
        hands(&mut controls, sim.telemetry.awa);
        sim.run(SUBSTEP, &controls);
        let off = deg(sim.telemetry.twa).abs();
        worst_off = worst_off.max(off);
    }
    let off = deg(sim.telemetry.twa);
    println!(
        "{what:<44} TWA {off:>6.0}  (furthest {worst_off:>5.0})  {:>4.1} kn  heel {:>3.0}",
        ms_to_kn(sim.telemetry.stw),
        deg(sim.state.heel)
    );
    (off.abs(), worst_off)
}

/// Every way out of irons there is, measured side by side.
#[test]
fn what_gets_her_out_and_what_does_not() {
    println!("\nhead to wind in {BLOWING:.0} knots, reefed main and staysail, two minutes:");
    // The helm hard over, which is what was being done on the deck. A rudder at forty
    // degrees is past its stall and at a knot and a half there is nothing going over it:
    // this is the control that cannot work, and it is the one everybody reaches for.
    try_to_fall_off("helm hard over, main sheeted in", 120.0, |c, awa| {
        c.rudder = -rad(40.0);
        let (m, _) =
            optimal_sheets_with(&BoatConfig::charter_sloop_34(), awa, c.outhaul, c.headsail);
        c.main_sheet = m;
    });
    // The same, with the helm where it bites instead of where it stalls.
    try_to_fall_off("helm at twenty, main sheeted in", 120.0, |c, awa| {
        c.rudder = -rad(20.0);
        let (m, _) =
            optimal_sheets_with(&BoatConfig::charter_sloop_34(), awa, c.outhaul, c.headsail);
        c.main_sheet = m;
    });
    // The main right out, which takes the weathervane off her stern.
    try_to_fall_off("main eased right out, helm at twenty", 120.0, |c, _| {
        c.rudder = -rad(20.0);
        c.main_sheet = 1.0;
    });
    // And the thing a crew does without being asked: back the headsail.
    try_to_fall_off("staysail backed, main eased", 120.0, |c, _| {
        c.rudder = -rad(20.0);
        c.main_sheet = 1.0;
        c.headsail_aback = true;
    });
    try_to_fall_off("staysail backed, main sheeted in", 120.0, |c, awa| {
        c.rudder = -rad(20.0);
        c.headsail_aback = true;
        let (m, _) =
            optimal_sheets_with(&BoatConfig::charter_sloop_34(), awa, c.outhaul, c.headsail);
        c.main_sheet = m;
    });
    // With the genoa rolled out and backed, which is the big lever and the one she had
    // rolled away.
    try_to_fall_off("genoa unrolled and backed", 120.0, |c, _| {
        c.rudder = -rad(20.0);
        c.main_sheet = 1.0;
        c.furl = 0.0;
        c.headsail_aback = true;
    });
}

/// **Backing a sail gets her out of irons.** If this fails, the way out does not exist.
///
/// Sixty degrees off the wind inside two minutes is not a brisk recovery — it is the
/// difference between a boat that can be sailed out of a failed tack and a boat that
/// cannot, and a real one does it in twenty seconds.
#[test]
fn backing_a_sail_is_the_way_out() {
    let (off, _) = try_to_fall_off("backed and eased", 120.0, |c, _| {
        c.rudder = -rad(20.0);
        c.main_sheet = 1.0;
        c.furl = 0.0;
        c.headsail_aback = true;
    });
    assert!(
        off > 60.0,
        "backed headsail, main eased, and she is still {off:.0} degrees off the wind \
         after two minutes — there is no way out of irons"
    );
}

/// And the mainsail is what holds her there, which is the other half of the same fact.
#[test]
fn the_main_sheeted_in_is_what_holds_her() {
    let (held, _) = try_to_fall_off("main in", 90.0, |c, awa| {
        c.rudder = -rad(20.0);
        let (m, _) =
            optimal_sheets_with(&BoatConfig::charter_sloop_34(), awa, c.outhaul, c.headsail);
        c.main_sheet = m;
    });
    let (freed, _) = try_to_fall_off("main out", 90.0, |c, _| {
        c.rudder = -rad(20.0);
        c.main_sheet = 1.0;
    });
    let _ = wrap_pi(0.0);
    assert!(
        freed > held,
        "easing the main made no difference in irons: {freed:.0} degrees off against \
         {held:.0} with it sheeted in"
    );
}
