//! Speed polar for the reference boat, reefed the way a sailor would reef it.
//!
//! The single most useful development instrument in the project: one screen that says
//! whether the physics is sane. Run it after touching `sail`, `hull` or `boat`.
//!
//!   cargo run -p sail-core --release --example polar

use sail_core::prelude::*;
use sail_core::wind::SteadyWind;

/// Reef to keep the boat on its feet. 22 degrees is about where a cruising crew stops
/// enjoying themselves and starts rolling away genoa.
const TARGET_HEEL_DEG: f64 = 22.0;
const SETTLE: f64 = 420.0;

fn main() {
    let cfg = BoatConfig::charter_sloop_34();
    println!(
        "{} — LWL {:.2} m, disp {:.0} kg, sail area {:.0} m2, hull speed {:.2} kn",
        cfg.name,
        cfg.lwl,
        cfg.displacement,
        cfg.sail_area(),
        ms_to_kn(cfg.hull_speed())
    );
    println!("reefed to hold heel at or below {TARGET_HEEL_DEG:.0} deg; * marks a point that did not converge\n");

    let wind_speeds = [6.0, 8.0, 12.0, 16.0, 20.0, 25.0];
    let angles = [
        30.0, 35.0, 40.0, 45.0, 50.0, 60.0, 70.0, 80.0, 90.0, 110.0, 120.0, 135.0, 150.0, 165.0,
        180.0,
    ];
    let max_heel = rad(TARGET_HEEL_DEG);

    print!("{:>5}", "TWA");
    for w in wind_speeds {
        print!("{:>10}", format!("{w:.0}kn"));
    }
    println!("     <- boat speed, kn");

    for twa in angles {
        print!("{twa:>5.0}");
        for w in wind_speeds {
            let make = || -> Box<dyn WindField> { Box::new(SteadyWind::new(0.0, kn_to_ms(w))) };
            let (_, s) = reef_for_heel(cfg, &make, rad(twa), SETTLE, max_heel);
            print!(
                "{:>9.2}{}",
                ms_to_kn(s.telemetry.stw),
                if s.converged { " " } else { "*" }
            );
        }
        println!();
    }

    for tws in [12.0, 20.0, 25.0] {
        println!("\n-- {tws:.0} kn --");
        println!(
            "{:>5}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>7}{:>7}",
            "TWA", "STW", "AWA", "AWS", "heel", "leeway", "VMG", "reef", "helm"
        );
        for twa in angles {
            let make = || -> Box<dyn WindField> { Box::new(SteadyWind::new(0.0, kn_to_ms(tws))) };
            let (reef, s) = reef_for_heel(cfg, &make, rad(twa), SETTLE, max_heel);
            let t = s.telemetry;
            println!(
                "{:>5.0}{:>8.2}{:>8.1}{:>8.2}{:>8.1}{:>8.2}{:>8.2}{:>7.2}{:>7.1}{}",
                twa,
                ms_to_kn(t.stw),
                deg(t.awa),
                ms_to_kn(t.aws),
                deg(t.heel),
                deg(t.leeway),
                ms_to_kn(t.vmg),
                reef,
                deg(s.controls.rudder),
                if s.converged { "" } else { "  *unconverged" }
            );
        }
    }
}
