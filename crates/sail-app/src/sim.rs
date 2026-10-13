//! The simulation as a Bevy resource, and the system that advances it.
//!
//! Everything numerically interesting lives in `sail-core`. This file exists only to
//! own the simulation and feed it the player's intent once per frame — which is what
//! keeps the engine a replaceable component rather than a dependency of the physics.

use bevy::prelude::*;
use sail_core::prelude::*;
use sail_core::state::Headsail;
use sail_core::water::WindSea;
use sail_core::wind::SyntheticWind;

use crate::weather::Weather;

#[derive(Resource)]
pub struct Sim {
    pub sim: Simulation,
    pub controls: Controls,
    /// True wind speed the world was set up with, for the polar overlay.
    pub nominal_tws: f64,
    /// The day that was drawn, kept so the forecast can be shown.
    pub weather: Weather,
}

impl Sim {
    pub fn new(weather: Weather) -> Self {
        let cfg = BoatConfig::archipelago_packet();
        let mut wind = SyntheticWind::new(weather.wind_from, weather.tws, weather.seed ^ 0x5A11);
        wind.squall_interval = weather.squall_interval;
        // `SAIL_SQUALL=120` for a squall every two minutes. The same kind of switch as
        // `SAIL_HEADSAIL`: you cannot look at a thing that happens every twenty-five
        // minutes often enough to get it right, and a knob beats sitting and waiting.
        if let Some(interval) = std::env::var("SAIL_SQUALL")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
        {
            wind.squall_interval = interval.max(10.0);
            wind.next_squall = wind.next_squall.min(interval);
        }
        let heading = wrap_tau(weather.wind_from - weather.start_twa);
        let mut state = BoatState::at_rest(heading);
        // Start with way on, close-hauled. Dead in the water head to wind is realistic
        // and also a miserable first ten seconds of a play session.
        state.surge = kn_to_ms(3.5);
        // A coastal fetch: enough to build a proper sea without the long ocean swell
        // that would dwarf a thirty-four footer. At twelve knots this is about 0.9 m
        // at four and a half seconds.
        let sea = WindSea::new(
            weather.tws,
            weather.fetch,
            weather.wind_from,
            weather.seed ^ 0xC0FFEE,
        );
        // The archipelago, and a wind that knows it is there. The land goes in before
        // the first substep so the boat is never started on a rock.
        let land = std::sync::Arc::new(crate::islands::generate(weather.seed));
        let wind = sail_core::land::ShelteredWind::new(Box::new(wind), land.clone());
        let mut sim = Simulation::new(cfg, state, Box::new(wind))
            .with_water(Box::new(sea))
            .with_land(land);

        // Sheets hard in is the wrong place to start. At three knots close-hauled the
        // apparent wind is well forward of where it settles, so block-to-block sheets
        // put both sails past their best angle and the boat cannot accelerate out of
        // it — it just bears away. A crew eases for speed first and trims on as the
        // boat builds, so begin where they would.
        sim.run(SUBSTEP, &Controls::default());
        // Full depth to start: the boat is upright and underpowered, so there is no
        // reason to take anything out of the sail yet.
        // Which sail to start with, so one can be looked at without furling and
        // changing first. `SAIL_HEADSAIL=code0` or `=gennaker`; anything else is the
        // genoa.
        let headsail = match std::env::var("SAIL_HEADSAIL").as_deref() {
            Ok("code0") => Headsail::Code0,
            Ok("gennaker") => Headsail::Gennaker,
            _ => Headsail::Genoa,
        };
        let outhaul = 0.0;
        let (main_sheet, jib_sheet) = optimal_sheets(&cfg, sim.telemetry.awa, outhaul);
        Self {
            sim,
            controls: Controls {
                main_sheet,
                jib_sheet,
                headsail,
                outhaul,
                // Genoa rolled away: main alone, to isolate what the mainsail does —
                // unless a sail was asked for by name, in which case it is wanted set.
                furl: if headsail == Headsail::Genoa {
                    1.0
                } else {
                    0.0
                },
                // Whatever the forecast said to tie in before casting off.
                reef: weather.start_reef,
                ..Default::default()
            },
            nominal_tws: weather.tws,
            weather,
        }
    }

    /// Where the boat is for *drawing*, interpolated across the leftover of the fixed
    /// 200 Hz substep. Instruments and the autopilot read `sim.state` instead: they
    /// want the integrator's own answer, not a blend, and five milliseconds of it
    /// makes no difference to a dial.
    pub fn render_state(&self) -> BoatState {
        self.sim.render_state()
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
