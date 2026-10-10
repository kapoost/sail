//! The simulation as a Bevy resource, and the system that advances it.
//!
//! Everything numerically interesting lives in `sail-core`. This file exists only to
//! own the simulation and feed it the player's intent once per frame — which is what
//! keeps the engine a replaceable component rather than a dependency of the physics.

use bevy::prelude::*;
use sail_core::prelude::*;
use sail_core::water::WindSea;
use sail_core::wind::SyntheticWind;

/// Fetch for the scenario's sea, metres.
///
/// Eighty kilometres, and the obvious-looking change to twenty was tried and reverted.
/// A short fetch does not mean an easy sea: twelve knots over twenty kilometres builds
/// only half a metre, but at a 2.9 s period — which sits on this hull's pitch
/// resonance of 2.8 s. She hobby-horses, and the model's added resistance stops her
/// dead. The nearly-one-metre sea over a long fetch has a 4.5 s period, well clear of
/// resonance, and she shoulders through it. Steepness and period matter more than
/// height, which is the whole reason the Baltic has its reputation.
pub const FETCH_METRES: f64 = 80_000.0;

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
        // A coastal fetch: enough to build a proper sea without the long ocean swell
        // that would dwarf a thirty-four footer. At twelve knots this is about 0.9 m
        // at four and a half seconds.
        let sea = WindSea::new(
            kn_to_ms(tws_knots),
            FETCH_METRES,
            rad(wind_direction_deg),
            0xC0FFEE,
        );
        let mut sim = Simulation::new(cfg, state, Box::new(wind)).with_water(Box::new(sea));

        // Sheets hard in is the wrong place to start. At three knots close-hauled the
        // apparent wind is well forward of where it settles, so block-to-block sheets
        // put both sails past their best angle and the boat cannot accelerate out of
        // it — it just bears away. A crew eases for speed first and trims on as the
        // boat builds, so begin where they would.
        sim.run(SUBSTEP, &Controls::default());
        // Full depth to start: the boat is upright and underpowered, so there is no
        // reason to take anything out of the sail yet.
        let outhaul = 0.0;
        let (main_sheet, jib_sheet) = optimal_sheets(&cfg, sim.telemetry.awa, outhaul);
        Self {
            sim,
            controls: Controls {
                main_sheet,
                jib_sheet,
                outhaul,
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
