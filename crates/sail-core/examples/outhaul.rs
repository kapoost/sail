//! Where is the outhaul actually fastest, and does the recommendation agree?
//!
//! `outhaul_for` drives the HUD advice, the trim-assist key, and the outhaul the
//! polar solver settles on — so if it disagrees with the model's own optimum then the
//! game tells the player to do something slow, and the reference polar is beatable by
//! ignoring it. This sweep is the check.
//!
//! **The sweep itself was wrong once, and this is the correction.** It used to hold the
//! true wind angle at forty-five degrees, and concluded from that that depth wins at
//! every wind speed — which is what you must conclude, because the one thing a flat sail
//! buys upwind is the ability to sail *closer*, and a fixed course cannot see it. VMG is
//! a projection: at forty-five degrees, one degree of extra pointing is worth about one
//! and a half percent of VMG for nothing. So each outhaul now gets its own best course,
//! which is what a crew sailing the boat would find, and the comparison is between two
//! properly sailed boats rather than between two trims of one badly sailed one.
//!
//!   cargo run -p sail-core --release --example outhaul

use sail_core::prelude::*;
use sail_core::wind::{SteadyWind, WindField};

const SETTLE: f64 = 420.0;

fn main() {
    // Both boats. The charter sloop is the calibration reference the whole model is
    // anchored to; the packet is the one actually being sailed, and a rule that is
    // right for one and wrong for the other would be worth knowing about.
    for cfg in [
        BoatConfig::archipelago_packet(),
        BoatConfig::charter_sloop_34(),
    ] {
        sweep(cfg);
    }
}

fn sweep(cfg: BoatConfig) {
    println!(
        "\n{} — best upwind VMG by outhaul, each at its own best course, reef 0\n",
        cfg.name
    );
    print!("{:>7}", "TWS");
    for o in 0..=5 {
        print!("{:>11}", format!("{:.1}", o as f64 / 5.0));
    }
    println!(
        "{:>8}{:>8}{:>10}{:>9}{:>8}",
        "best", "rule", "cost", "heel@best", "twa"
    );

    for tws in [5.0, 8.0, 12.0, 16.0, 20.0, 24.0, 28.0] {
        print!("{tws:>7.0}");
        let mut best = (0.0, f64::NEG_INFINITY, 0.0);
        for o in 0..=5 {
            let outhaul = o as f64 / 5.0;
            let (vmg, twa) = best_course(cfg, tws, outhaul);
            // The course each trim can hold is half the story, so it is printed: a flat
            // sail that points two degrees higher is faster to windward at the same
            // speed through the water, and a table of speeds alone hides that.
            print!("{:>11}", format!("{:.2}/{:.0}", ms_to_kn(vmg), deg(twa)));
            if vmg > best.1 {
                best = (outhaul, vmg, twa);
            }
        }
        // What the heel-based rule settles on, found by letting the solver run it.
        let settled = settled_outhaul(cfg, tws);
        let at_recommended = best_course(cfg, tws, settled).0;
        println!(
            "{:>8.1}{:>8.2}{:>9.3} kn{:>8.1}{:>8.0}",
            best.0,
            settled,
            ms_to_kn(best.1 - at_recommended),
            deg(heel_at(cfg, tws, best.0, best.2)),
            deg(best.2)
        );
    }
    println!("`cost` is the VMG the recommendation gives away against the sweep's best.");
}

/// Heel at a given outhaul and course, for working out where the model turns over.
fn heel_at(cfg: BoatConfig, tws_knots: f64, outhaul: f64, twa: f64) -> f64 {
    run_at(cfg, tws_knots, outhaul, twa).1
}

/// The best VMG this outhaul can reach, and the course it reaches it on.
///
/// Letting each trim pick its own course is the whole point: a flattened main holds a
/// closer course without stalling, and that is most of what flattening is for.
fn best_course(cfg: BoatConfig, tws_knots: f64, outhaul: f64) -> (f64, f64) {
    let mut best = (f64::NEG_INFINITY, rad(45.0));
    for step in 0..=7 {
        let twa = rad(36.0 + 2.0 * step as f64);
        let vmg = run_at(cfg, tws_knots, outhaul, twa).0;
        if vmg > best.0 {
            best = (vmg, twa);
        }
    }
    best
}

/// The outhaul the heel-based rule converges on at this wind speed.
fn settled_outhaul(cfg: BoatConfig, tws_knots: f64) -> f64 {
    let make = || -> Box<dyn WindField> { Box::new(SteadyWind::new(0.0, kn_to_ms(tws_knots))) };
    let s = steady_state(cfg, make, rad(45.0), SETTLE, 0.0);
    s.controls.outhaul
}

/// Returns (VMG, heel); VMG is -inf when the boat could not be held on course.
fn run_at(cfg: BoatConfig, tws_knots: f64, outhaul: f64, twa: f64) -> (f64, f64) {
    let heading = wrap_tau(0.0 - twa);
    let mut state = BoatState::at_rest(heading);
    state.surge = 0.6 * cfg.hull_speed();
    let mut sim = Simulation::new(
        cfg,
        state,
        Box::new(SteadyWind::new(0.0, kn_to_ms(tws_knots))) as Box<dyn WindField>,
    );
    let mut pilot = TillerPilot::new();
    let mut controls = Controls {
        outhaul,
        ..Default::default()
    };
    for _ in 0..(SETTLE / SUBSTEP) as u64 {
        let (m, j) = optimal_sheets(&cfg, sim.telemetry.awa, outhaul);
        controls.main_sheet = m;
        controls.jib_sheet = j;
        controls.rudder = pilot.rudder(&cfg, &sim.state, heading, SUBSTEP);
        sim.run(SUBSTEP, &controls);
    }
    if wrap_pi(sim.state.heading - heading).abs() > rad(3.0) {
        return (f64::NEG_INFINITY, sim.state.heel);
    }
    (sim.telemetry.vmg, sim.state.heel)
}
