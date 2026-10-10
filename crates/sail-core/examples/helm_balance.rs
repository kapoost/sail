//! Does the boat hold a close-hauled course, and at what rudder angle?
//!
//! A boat that bears away whenever you stop steering has a helm balance problem, and
//! the balance depends on trim: too much headsail gives lee helm, too much mainsail
//! gives weather helm. This prints both the free-running behaviour and the standing
//! rudder needed to hold course, across the trim space.
//!
//!   cargo run -p sail-core --release --example helm_balance

use sail_core::prelude::*;
use sail_core::wind::{SteadyWind, WindField};

fn main() {
    let cfg = BoatConfig::charter_sloop_34();
    let tws = kn_to_ms(12.0);
    let twa = rad(45.0);
    let heading = wrap_tau(0.0 - twa);

    println!("=== free running: rudder centred, nobody steering ===");
    println!("starboard tack, TWA 45 at the start, 12 kn true\n");
    for (name, main, jib) in [
        ("sheets hard in (the game's start state)", 0.0, 0.0),
        ("optimal trim", -1.0, -1.0),
        ("main only, genoa furled away", 0.0, 1.0),
    ] {
        let mut sim = Simulation::new(
            cfg,
            start_state(heading),
            Box::new(SteadyWind::new(0.0, tws)),
        );
        let mut controls = Controls::default();
        println!("-- {name} --");
        println!(
            "{:>6}{:>9}{:>8}{:>8}{:>8}{:>11}",
            "t", "heading", "TWA", "heel", "speed", "yaw mom"
        );
        for step in 0..=12 {
            let secs = 10.0;
            for _ in 0..(secs / SUBSTEP) as u64 {
                if main < 0.0 {
                    let (m, j) = optimal_sheets(&cfg, sim.telemetry.awa);
                    controls.main_sheet = m;
                    controls.jib_sheet = j;
                } else {
                    controls.main_sheet = main;
                    controls.jib_sheet = jib;
                }
                sim.run(SUBSTEP, &controls);
            }
            let t = sim.telemetry;
            println!(
                "{:>6.0}{:>9.0}{:>8.0}{:>8.1}{:>8.2}{:>11.0}",
                step as f64 * 10.0,
                deg(sim.state.heading),
                deg(t.twa),
                deg(t.heel),
                ms_to_kn(t.stw),
                t.weather_helm
            );
            if step == 0 {
                // skip to keep the table short
            }
        }
        println!();
    }

    println!("=== standing rudder needed to hold TWA 45, by trim ===");
    println!("negative = port rudder = weather helm (boat wants to luff)");
    println!("positive = starboard rudder = LEE helm (boat wants to bear away)\n");
    println!(
        "{:>12}{:>12}{:>9}{:>8}{:>8}{:>8}",
        "main sheet", "genoa sheet", "helm", "heel", "speed", "VMG"
    );
    let make = || -> Box<dyn WindField> { Box::new(SteadyWind::new(0.0, tws)) };
    for (m, j) in [
        (0.00, 0.00),
        (0.15, 0.15),
        (0.30, 0.30),
        (0.30, 0.00),
        (0.00, 0.30),
        (0.30, 1.00),
    ] {
        let s = fixed_trim(cfg, &make, twa, m, j);
        println!(
            "{:>12.2}{:>12.2}{:>9.1}{:>8.1}{:>8.2}{:>8.2}{}",
            m,
            j,
            deg(s.0),
            deg(s.1),
            ms_to_kn(s.2),
            ms_to_kn(s.3),
            if s.4 { "" } else { "   *unconverged" }
        );
    }
    let auto = {
        let s = steady_state(cfg, make, twa, 420.0, 0.0);
        (
            s.controls.rudder,
            s.state.heel,
            s.telemetry.stw,
            s.telemetry.vmg,
            s.converged,
        )
    };
    println!(
        "{:>12}{:>12}{:>9.1}{:>8.1}{:>8.2}{:>8.2}",
        "auto",
        "auto",
        deg(auto.0),
        deg(auto.1),
        ms_to_kn(auto.2),
        ms_to_kn(auto.3)
    );
}

fn start_state(heading: f64) -> BoatState {
    let mut s = BoatState::at_rest(heading);
    s.surge = kn_to_ms(3.0);
    s
}

/// Hold a heading at a fixed trim and report the rudder the pilot settles on.
fn fixed_trim<F>(
    cfg: BoatConfig,
    make: &F,
    twa: f64,
    main: f64,
    jib: f64,
) -> (f64, f64, f64, f64, bool)
where
    F: Fn() -> Box<dyn WindField>,
{
    let heading = wrap_tau(0.0 - twa);
    let mut sim = Simulation::new(cfg, start_state(heading), make());
    let mut pilot = TillerPilot::new();
    let mut controls = Controls {
        main_sheet: main,
        jib_sheet: jib,
        ..Default::default()
    };
    for _ in 0..(420.0 / SUBSTEP) as u64 {
        controls.rudder = pilot.rudder(&cfg, &sim.state, heading, SUBSTEP);
        sim.run(SUBSTEP, &controls);
    }
    let err = wrap_pi(sim.state.heading - heading);
    (
        controls.rudder,
        sim.state.heel,
        sim.telemetry.stw,
        sim.telemetry.vmg,
        err.abs() < rad(3.0),
    )
}
