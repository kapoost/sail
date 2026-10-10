//! Where is the outhaul actually fastest, and does the recommendation agree?
//!
//! `outhaul_for_heel` drives the HUD advice, the trim-assist key, and the outhaul the
//! polar solver settles on — so if it disagrees with the model's own optimum then the
//! game tells the player to do something slow, and the reference polar is beatable by
//! ignoring it. This sweep is the check.
//!
//! It is also the measurement that corrected a wrong assumption: depth wins at every
//! wind speed the boat can still hold a course at, exactly as a velocity prediction
//! program would say, and flattening only pays once righting moment is the binding
//! constraint. The first version of the rule crossed over at fourteen knots on the
//! strength of folklore and gave away real VMG for it.
//!
//!   cargo run -p sail-core --release --example outhaul

use sail_core::prelude::*;
use sail_core::wind::{SteadyWind, WindField};

const SETTLE: f64 = 420.0;

fn main() {
    let cfg = BoatConfig::charter_sloop_34();
    println!("best upwind VMG by outhaul, TWA 45, reef 0\n");
    print!("{:>7}", "TWS");
    for o in 0..=5 {
        print!("{:>9}", format!("{:.1}", o as f64 / 5.0));
    }
    println!("{:>8}{:>8}{:>10}{:>9}", "best", "rule", "cost", "heel@best");

    for tws in [5.0, 8.0, 12.0, 16.0, 20.0, 24.0, 28.0] {
        print!("{tws:>7.0}");
        let mut best = (0.0, f64::NEG_INFINITY);
        for o in 0..=5 {
            let outhaul = o as f64 / 5.0;
            let vmg = vmg_at(cfg, tws, outhaul);
            print!("{:>9.3}", ms_to_kn(vmg));
            if vmg > best.1 {
                best = (outhaul, vmg);
            }
        }
        // What the heel-based rule settles on, found by letting the solver run it.
        let settled = settled_outhaul(cfg, tws);
        let at_recommended = vmg_at(cfg, tws, settled);
        println!(
            "{:>8.1}{:>8.2}{:>9.3} kn{:>8.1}",
            best.0,
            settled,
            ms_to_kn(best.1 - at_recommended),
            deg(heel_at(cfg, tws, best.0))
        );
    }
    println!("\n`cost` is the VMG the recommendation gives away against the sweep's best.");
}

/// Heel at a given outhaul, for working out where the model actually turns over.
fn heel_at(cfg: BoatConfig, tws_knots: f64, outhaul: f64) -> f64 {
    run_at(cfg, tws_knots, outhaul).1
}

/// The outhaul the heel-based rule converges on at this wind speed.
fn settled_outhaul(cfg: BoatConfig, tws_knots: f64) -> f64 {
    let make = || -> Box<dyn WindField> { Box::new(SteadyWind::new(0.0, kn_to_ms(tws_knots))) };
    let s = steady_state(cfg, make, rad(45.0), SETTLE, 0.0);
    s.controls.outhaul
}

fn vmg_at(cfg: BoatConfig, tws_knots: f64, outhaul: f64) -> f64 {
    run_at(cfg, tws_knots, outhaul).0
}

/// Returns (VMG, heel); VMG is -inf when the boat could not be held on course.
fn run_at(cfg: BoatConfig, tws_knots: f64, outhaul: f64) -> (f64, f64) {
    let twa = rad(45.0);
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
