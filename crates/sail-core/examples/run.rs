//! Can she really do 8.3 knots in 6 of wind, dead downwind?
use sail_core::autopilot::steady_state_with;
use sail_core::prelude::*;
use sail_core::state::Headsail;
use sail_core::wind::{SteadyWind, WindField};

fn main() {
    let cfg = BoatConfig::archipelago_packet();
    for tws in [6.0, 9.0, 13.0] {
        let make = || -> Box<dyn WindField> { Box::new(SteadyWind::new(0.0, kn_to_ms(tws))) };
        println!("\n{tws:.0} kn true");
        println!("  twa   sail        stw    vmg   ratio");
        for head in [Headsail::Genoa, Headsail::Code0, Headsail::Gennaker] {
            for twa in [140.0, 155.0, 167.0, 180.0] {
                let s = steady_state_with(cfg, make, rad(twa), 420.0, 0.0, head);
                let stw = ms_to_kn(s.telemetry.stw);
                println!(
                    " {twa:4.0}   {:9}  {stw:5.2} {:6.2}  {:5.2}{}",
                    format!("{head:?}"),
                    ms_to_kn(s.telemetry.vmg),
                    stw / tws,
                    if stw > tws {
                        "  <-- faster than the wind"
                    } else {
                        ""
                    }
                );
            }
        }
    }
}
