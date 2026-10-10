//! sail — presentation layer.
//!
//! The only crate in the workspace that knows a game engine exists. Everything
//! numerically interesting lives in `sail-core`, which has no engine types in it at
//! all; this binary reads its state and draws it. That split is what keeps the engine
//! a replaceable component, and it is why choosing Bevy here is a decision that can be
//! revisited for the cost of rewriting this one crate.

mod boat;
mod camera;
mod course;
mod frame;
mod hud;
mod input;
mod sea_texture;
mod sim;
mod sound;
mod world;

use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};

/// Conditions for the proof of concept: a steady moderate breeze, close-hauled on
/// starboard tack with way on. No time compression — the clock runs at one to one.
const WIND_KNOTS: f64 = 12.0;
const WIND_FROM_DEG: f64 = 0.0;
const START_TWA_DEG: f64 = 45.0;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "sail".into(),
                resolution: (1440u32, 880u32).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_plugins(sound::plugin)
        .insert_resource(sim::Sim::new(WIND_KNOTS, WIND_FROM_DEG, START_TWA_DEG))
        .insert_resource(hud::HudVisible(true))
        .init_resource::<hud::Polar>()
        .init_resource::<input::HelmSource>()
        .init_resource::<input::Pilot>()
        .add_systems(
            Startup,
            (
                world::setup,
                boat::setup,
                camera::setup,
                hud::build_polar,
                sound::setup,
                course::setup,
            ),
        )
        .add_systems(
            Update,
            (
                // Intent, then physics, then everything that reads the result.
                input::gather,
                sim::advance,
                (
                    boat::update_hull,
                    boat::update_rudder,
                    boat::update_boom,
                    boat::update_windex,
                    boat::update_telltales,
                    boat::update_sails,
                    course::update,
                    world::update_sea,
                    course::float_marks,
                    camera::follow,
                    sound::update,
                ),
                (camera::toggle, hud::toggle, course::restart),
            )
                .chain(),
        )
        .add_systems(EguiPrimaryContextPass, hud::draw)
        .run();
}
