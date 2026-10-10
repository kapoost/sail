//! What sea does a given wind build, how does the boat move in it, and what does it
//! cost?
//!
//! The polar's counterpart for waves. Run it after touching `water.rs`,
//! `hydrostatics.rs`, or anything in `sim.rs` to do with heave and pitch.
//!
//!   cargo run -p sail-core --release --example seaway

use sail_core::prelude::*;
use sail_core::water::{fetch_limited_sea, WindSea};
use sail_core::wind::{SteadyWind, WindField};

const SETTLE: f64 = 420.0;

fn main() {
    let cfg = BoatConfig::charter_sloop_34();

    println!("=== what the wind builds ===\n");
    println!(
        "{:>7}{:>14}{:>14}{:>14}",
        "TWS", "bay 20 km", "coastal 80 km", "open 500 km"
    );
    println!(
        "{:>7}{:>14}{:>14}{:>14}",
        "kn", "Hs / Tp", "Hs / Tp", "Hs / Tp"
    );
    for tws in [8.0, 12.0, 16.0, 20.0, 25.0, 30.0] {
        print!("{tws:>7.0}");
        for fetch in [20_000.0, 80_000.0, 500_000.0] {
            let (hs, tp) = fetch_limited_sea(kn_to_ms(tws), fetch);
            print!("{:>14}", format!("{hs:.2} m / {tp:.1} s"));
        }
        println!();
    }

    println!("\n=== wavelengths in a 16 kn coastal sea ===\n");
    let sea = WindSea::new(kn_to_ms(16.0), 80_000.0, 0.0, 7);
    println!(
        "Hs {:.2} m, peak period {:.1} s, boat LWL {:.1} m\n",
        sea.significant_height(),
        sea.peak_period(),
        cfg.lwl
    );
    println!(
        "{:>12}{:>12}{:>12}{:>12}",
        "period s", "length m", "speed kn", "amp m"
    );
    let mut sorted: Vec<_> = sea.components().to_vec();
    sorted.sort_by(|a, b| b.wavelength().partial_cmp(&a.wavelength()).unwrap());
    for c in &sorted {
        println!(
            "{:>12.1}{:>12.1}{:>12.2}{:>12.2}",
            std::f64::consts::TAU / c.omega,
            c.wavelength(),
            ms_to_kn(c.celerity()),
            c.amplitude
        );
    }

    println!("\n=== natural periods, flat calm ===\n");
    for (name, nudge) in [("heave", 0usize), ("pitch", 1), ("roll", 2)] {
        let (period, decay) = natural_period(cfg, nudge);
        println!(
            "{name:>8}  {period:.2} s   amplitude after one cycle {:.0}%",
            decay * 100.0
        );
    }

    println!("\n=== what a 16 kn coastal sea costs, by heading ===\n");
    println!(
        "{:>6}{:>9}{:>9}{:>8}{:>7}{:>8}{:>9}{:>9}{:>9}",
        "TWA", "flat kn", "sea kn", "loss", "pitch", "mean p", "R waves", "mean surf", "R total"
    );
    for twa in [45.0, 60.0, 90.0, 120.0, 150.0, 180.0] {
        let flat = steady_state(
            cfg,
            || -> Box<dyn WindField> { Box::new(SteadyWind::new(0.0, kn_to_ms(16.0))) },
            rad(twa),
            SETTLE,
            0.0,
        );
        let m = in_a_sea(cfg, 16.0, 80_000.0, rad(twa));
        let loss = 1.0 - m.speed / flat.telemetry.stw;
        println!(
            "{:>6.0}{:>9.2}{:>9.2}{:>7.1}%{:>7.1}{:>8.2}{:>9.0}{:>9.0}{:>9.0}",
            twa,
            ms_to_kn(flat.telemetry.stw),
            ms_to_kn(m.speed),
            loss * 100.0,
            deg(m.pitch_peak),
            deg(m.pitch_mean),
            m.r_waves,
            m.surf_mean,
            m.r_total
        );
    }
    println!("\npitch in degrees: peak then mean. Forces in newtons, mean over the run.");
    println!("a positive mean pitch is bow-up, and a negative mean surf force retards.");
}

/// Release the boat from a displaced position and measure the period and the decay.
fn natural_period(cfg: BoatConfig, which: usize) -> (f64, f64) {
    let mut sim = Simulation::new(
        cfg,
        BoatState::at_rest(0.0),
        Box::new(SteadyWind::new(0.0, 0.0)),
    );
    sim.run(30.0, &Controls::default());
    match which {
        0 => sim.state.heave = -0.25,
        1 => sim.state.pitch = rad(5.0),
        _ => sim.state.heel = rad(12.0),
    }
    let mut samples = Vec::new();
    for _ in 0..(26.0 / SUBSTEP) as u64 {
        sim.run(SUBSTEP, &Controls::default());
        samples.push(match which {
            0 => sim.state.heave,
            1 => sim.state.pitch,
            _ => sim.state.heel,
        });
    }
    let tail = &samples[samples.len() * 3 / 4..];
    let mean = tail.iter().sum::<f64>() / tail.len() as f64;
    let centred: Vec<f64> = samples.iter().map(|s| s - mean).collect();
    let mut indices = Vec::new();
    for (i, w) in centred.windows(2).enumerate() {
        if (w[0] <= 0.0) != (w[1] <= 0.0) {
            indices.push(i);
        }
    }
    if indices.len() < 3 {
        return (f64::NAN, f64::NAN);
    }
    let half: f64 = indices
        .windows(2)
        .map(|w| (w[1] - w[0]) as f64 * SUBSTEP)
        .sum::<f64>()
        / (indices.len() - 1) as f64;
    let period = 2.0 * half;
    let first = centred[0].abs();
    let after = centred[((period / SUBSTEP) as usize).min(centred.len() - 1)].abs();
    (period, if first > 0.0 { after / first } else { 0.0 })
}

struct Motion {
    speed: f64,
    pitch_peak: f64,
    pitch_mean: f64,
    r_waves: f64,
    surf_mean: f64,
    r_total: f64,
}

/// Sail a heading in a sea and report what the forces are actually doing.
fn in_a_sea(cfg: BoatConfig, tws_knots: f64, fetch: f64, twa: f64) -> Motion {
    let heading = wrap_tau(0.0 - twa);
    let mut state = BoatState::at_rest(heading);
    state.surge = 0.6 * cfg.hull_speed();
    let mut sim = Simulation::new(
        cfg,
        state,
        Box::new(SteadyWind::new(0.0, kn_to_ms(tws_knots))),
    )
    .with_water(Box::new(WindSea::new(kn_to_ms(tws_knots), fetch, 0.0, 17)));
    let mut pilot = TillerPilot::new();
    let mut controls = Controls::default();
    let total = 300.0;
    let settle = 120.0;
    let mut sum = 0.0;
    let mut n = 0.0;
    let mut pitch = 0.0_f64;
    let mut pitch_sum = 0.0;
    let mut waves_sum = 0.0;
    let mut surf_sum = 0.0;
    let mut total_sum = 0.0;
    for i in 0..(total / SUBSTEP) as u64 {
        let (m, j) = optimal_sheets(&cfg, sim.telemetry.awa, controls.outhaul);
        controls.main_sheet = m;
        controls.jib_sheet = j;
        controls.rudder = pilot.rudder(&cfg, &sim.state, heading, SUBSTEP);
        sim.run(SUBSTEP, &controls);
        if i as f64 * SUBSTEP > settle {
            sum += sim.telemetry.stw;
            n += 1.0;
            pitch = pitch.max(sim.state.pitch.abs());
            pitch_sum += sim.state.pitch;
            waves_sum += sim.telemetry.resistance_waves;
            surf_sum += sim.telemetry.surf_force;
            total_sum += sim.telemetry.resistance;
        }
    }
    Motion {
        speed: sum / n,
        pitch_peak: pitch,
        pitch_mean: pitch_sum / n,
        r_waves: waves_sum / n,
        surf_mean: surf_sum / n,
        r_total: total_sum / n,
    }
}
