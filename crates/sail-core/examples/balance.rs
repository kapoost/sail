//! Weather helm, against the two things a crew changes when she gripes.
use sail_core::prelude::*;
use sail_core::wind::{SteadyWind, WindField};

fn settle(cfg: BoatConfig, tws_kn: f64, twa: f64, reef: f64, furl: f64) -> (f64, f64, f64, f64) {
    let heading = wrap_tau(0.0 - rad(twa));
    let mut state = BoatState::at_rest(heading);
    state.surge = kn_to_ms(4.0);
    let mut sim = Simulation::new(
        cfg,
        state,
        Box::new(SteadyWind::new(0.0, kn_to_ms(tws_kn))) as Box<dyn WindField>,
    );
    let mut pilot = TillerPilot::new();
    let mut controls = Controls {
        reef,
        furl,
        outhaul: 0.5,
        ..Default::default()
    };
    for _ in 0..(240.0 / SUBSTEP) as u64 {
        let (m, j) = optimal_sheets(&cfg, sim.telemetry.awa, controls.outhaul);
        controls.main_sheet = m;
        controls.jib_sheet = j;
        controls.rudder = pilot.correct_under_load(
            &cfg,
            wrap_pi(heading - sim.state.heading),
            sim.state.yaw_rate,
            sim.telemetry.weather_helm,
            SUBSTEP,
        );
        sim.run(SUBSTEP, &controls);
    }
    (
        sim.telemetry.weather_helm,
        deg(controls.rudder),
        deg(sim.state.heel),
        ms_to_kn(sim.telemetry.stw),
    )
}

fn main() {
    let cfg = BoatConfig::archipelago_packet();
    for tws in [14.0, 22.0, 30.0] {
        println!("\n-- {tws:.0} kn true, close hauled --");
        println!("  reef  furl   weather helm      helm     heel      stw");
        for (reef, furl) in [
            (0.0, 0.0),
            (0.33, 0.0),
            (0.67, 0.0),
            (0.0, 0.5),
            (0.0, 1.0),
            (0.67, 1.0),
        ] {
            let (wh, helm, heel, stw) = settle(cfg, tws, 42.0, reef, furl);
            println!(
                "  {reef:>4.2}  {furl:>4.2}  {wh:>10.0} Nm  {helm:>7.1}  {heel:>7.1}  {stw:>7.2}"
            );
        }
    }
}
