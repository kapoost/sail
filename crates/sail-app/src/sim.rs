//! The simulation as a Bevy resource, and the system that advances it.
//!
//! Everything numerically interesting lives in `sail-core`. This file exists only to
//! own the simulation and feed it the player's intent once per frame — which is what
//! keeps the engine a replaceable component rather than a dependency of the physics.

use bevy::prelude::*;
use sail_core::prelude::*;
use sail_core::wind::SyntheticWind;

#[derive(Resource)]
pub struct Sim {
    pub sim: Simulation,
    pub controls: Controls,
    /// True wind speed the world was set up with, for the polar overlay.
    pub nominal_tws: f64,
}

impl Sim {
    pub fn new(tws_knots: f64, wind_direction_deg: f64, start_twa_deg: f64) -> Self {
        let cfg = BoatConfig::charter_sloop_34();
        let wind = SyntheticWind::new(rad(wind_direction_deg), kn_to_ms(tws_knots), 0x5A11_u64);
        let heading = wrap_tau(rad(wind_direction_deg) - rad(start_twa_deg));
        let mut state = BoatState::at_rest(heading);
        // Start with way on, close-hauled. Dead in the water head to wind is realistic
        // and also a miserable first ten seconds of a play session.
        state.surge = kn_to_ms(3.5);
        let mut sim = Simulation::new(cfg, state, Box::new(wind));

        // Sheets hard in is the wrong place to start. At three knots close-hauled the
        // apparent wind is well forward of where it settles, so block-to-block sheets
        // put both sails past their best angle and the boat cannot accelerate out of
        // it — it just bears away. A crew eases for speed first and trims on as the
        // boat builds, so begin where they would.
        sim.run(SUBSTEP, &Controls::default());
        let (main_sheet, jib_sheet) = optimal_sheets(&cfg, sim.telemetry.awa);
        Self {
            sim,
            controls: Controls {
                main_sheet,
                jib_sheet,
                // Genoa rolled away: main alone, to isolate what the mainsail does.
                furl: 1.0,
                ..Default::default()
            },
            nominal_tws: kn_to_ms(tws_knots),
        }
    }

    pub fn config(&self) -> BoatConfig {
        self.sim.config
    }
}

/// Advance the physics. `Simulation::step` does its own fixed substepping, so a
/// variable frame time never reaches the integrator.
pub fn advance(mut sim: ResMut<Sim>, time: Res<Time>) {
    let dt = time.delta_secs_f64().min(0.25);
    let controls = sim.controls;
    sim.sim.step(dt, &controls);
}
