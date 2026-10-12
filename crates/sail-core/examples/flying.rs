//! What the reaching sails are worth, and where they stop being worth it.
//!
//! The question a sail wardrobe exists to answer is not "which is fastest" but "which
//! is fastest *here*", so the instrument is a sweep: every sail at every angle, and the
//! crossovers read straight off the table. If a Code 0 never wins anywhere, it is a
//! decoration and the numbers behind it are wrong.
//!
//!   cargo run -p sail-core --release --example flying

use sail_core::autopilot::reef_for_heel_with;
use sail_core::prelude::*;
use sail_core::state::Headsail;
use sail_core::wind::SteadyWind;

const SETTLE: f64 = 420.0;

fn main() {
    let cfg = BoatConfig::archipelago_packet();
    let max_heel = rad(22.0);
    let sails = [Headsail::Genoa, Headsail::Code0, Headsail::Gennaker];

    println!(
        "{} — genoa {:.0} m2, code 0 {:.0} m2, gennaker {:.0} m2\n",
        cfg.name, cfg.genoa.area, cfg.code_zero.area, cfg.gennaker.area
    );

    for tws in [8.0, 14.0, 20.0] {
        println!("-- {tws:.0} kn true --");
        print!("{:>5}", "TWA");
        for s in sails {
            print!("{:>12}", s.name());
        }
        println!("{:>10}", "best");

        for twa in [
            40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 110.0, 120.0, 135.0, 150.0, 165.0, 180.0,
        ] {
            print!("{twa:>5.0}");
            let mut best = (0.0, "");
            for sail in sails {
                let make = || -> Box<dyn WindField> { Box::new(SteadyWind::new(0.0, kn_to_ms(tws))) };
                let (_, st) = reef_for_heel_with(cfg, &make, rad(twa), SETTLE, max_heel, sail);
                let kn = ms_to_kn(st.telemetry.stw);
                print!("{kn:>12.2}");
                if kn > best.0 {
                    best = (kn, sail.name());
                }
            }
            println!("{:>10}", best.1);
        }
        println!();
    }

    // What the staysail is worth, which is the question a double head rig asks.
    println!("-- staysail: set against handed, 12 kn true, boat speed in knots --");
    println!(
        "{:>5}{:>12}{:>10}{:>10}{:>12}{:>10}{:>10}",
        "TWA", "genoa", "+stays'l", "delta", "gennaker", "+stays'l", "delta"
    );
    for twa in [50.0, 70.0, 90.0, 110.0, 135.0, 150.0] {
        print!("{twa:>5.0}");
        for sail in [Headsail::Genoa, Headsail::Gennaker] {
            let mut pair = [0.0; 2];
            for (i, stay) in [0.0, 1.0].iter().enumerate() {
                let make =
                    || -> Box<dyn WindField> { Box::new(SteadyWind::new(0.0, kn_to_ms(12.0))) };
                let st = sail_core::autopilot::steady_state_rigged(
                    cfg,
                    make,
                    rad(twa),
                    SETTLE,
                    0.0,
                    sail,
                    *stay,
                );
                pair[i] = ms_to_kn(st.telemetry.stw);
            }
            print!(
                "{:>12.2}{:>10.2}{:>+10.2}",
                pair[0],
                pair[1],
                pair[1] - pair[0]
            );
        }
        println!();
    }
    println!();

    // Where the speed is coming from. Two sails that give the same boat speed are not
    // doing the same thing if one of them is heeling the boat twice as far, and a sail
    // that adds no heel at all is not drawing.
    println!("-- what the sails are actually doing, 8 kn true --");
    println!(
        "{:>5}{:>10}{:>8}{:>8}{:>8}{:>8}",
        "TWA", "sail", "STW", "heel", "leeway", "AWA"
    );
    for twa in [60.0, 90.0, 110.0, 150.0] {
        for sail in sails {
            let make = || -> Box<dyn WindField> { Box::new(SteadyWind::new(0.0, kn_to_ms(8.0))) };
            let (_, st) = reef_for_heel_with(cfg, &make, rad(twa), SETTLE, max_heel, sail);
            println!(
                "{twa:>5.0}{:>10}{:>8.2}{:>8.1}{:>8.2}{:>8.0}",
                sail.name(),
                ms_to_kn(st.telemetry.stw),
                deg(st.state.heel),
                deg(st.telemetry.leeway),
                deg(st.telemetry.awa).abs(),
            );
        }
    }
}
