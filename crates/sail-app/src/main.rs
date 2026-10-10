//! sail — presentation layer.
//!
//! The only crate in the workspace that knows a game engine exists. Everything
//! numerically interesting lives in `sail-core`, which has no engine types in it at
//! all; this binary reads its state and draws it. That split is what keeps the engine
//! a replaceable component, and it is why choosing Bevy here is a decision that can be
//! revisited for the cost of rewriting this one crate.

mod boat;
mod camera;
mod clutch;
mod course;
mod deck;
mod frame;
mod hud;
mod input;
mod panel;
mod rig;
mod sea_texture;
mod sim;
mod sky;
mod sound;
mod wake;
mod weather;
mod world;

use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};

/// No time compression — the clock runs at one to one.
///
/// The conditions are drawn, not chosen: see `weather.rs`. `SAIL_SEED` in the
/// environment pins the draw, so a day worth sailing again, or one that broke
/// something, can be had back exactly.
/// Where to look for `assets/`.
///
/// Bevy defaults to a directory beside the executable, which is right for something
/// shipped and wrong for `./target/release/sail-app`, where it finds nothing and the
/// hull silently fails to appear. So: beside the binary if it is there, otherwise the
/// workspace, whose path is known at compile time.
fn asset_root() -> String {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(beside) = exe.parent().map(|d| d.join("assets")) {
            if beside.is_dir() {
                return beside.to_string_lossy().into_owned();
            }
        }
    }
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets").to_string()
}

fn main() {
    let seed = std::env::var("SAIL_SEED")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0x5A1Fu64)
        });
    let weather = weather::Weather::draw(seed);
    // Printed rather than logged: the draw happens before Bevy's log plugin exists,
    // so `info!` here goes nowhere at all.
    println!("forecast: {}", weather.report());

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: asset_root(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "sail".into(),
                        resolution: (1440u32, 880u32).into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(EguiPlugin::default())
        .add_plugins(sound::plugin)
        .insert_resource(sim::Sim::new(weather))
        .insert_resource(weather)
        // The console is the interface now; the diagnostic panel is the microscope and
        // starts put away. Tab brings it back, or the switch on the console does.
        .insert_resource(hud::HudVisible(false))
        .init_resource::<hud::Polar>()
        .init_resource::<input::HelmSource>()
        .init_resource::<clutch::Bank>()
        .init_resource::<input::Pilot>()
        .init_resource::<frame::RenderOrigin>()
        .init_resource::<camera::ChaseState>()
        .add_systems(
            Startup,
            (
                world::setup,
                boat::setup,
                camera::setup,
                hud::build_polar,
                sound::setup,
                course::setup,
                wake::setup,
                rig::setup,
                panel::setup,
            )
                // Chained because several of these depend on each other's entities,
                // and at startup the cost of ordering them is nothing at all.
                .chain(),
        )
        .add_systems(
            Update,
            (
                // Intent, then physics, then everything that reads the result.
                input::gather,
                clutch::handle,
                sim::advance,
                // Pick where the world is drawn from before anything is placed in it,
                // so the whole frame agrees on one mark.
                frame::rebase_origin,
                (
                    boat::update_hull,
                    boat::update_rudder,
                    boat::update_boom,
                    boat::update_windex,
                    boat::update_genoa_cars,
                    boat::update_telltales,
                    boat::update_sails,
                    course::update,
                    world::update_sea,
                    course::float_marks,
                    wake::update,
                    rig::update,
                    camera::follow,
                    sound::update,
                ),
                (camera::toggle, hud::toggle, course::restart),
            )
                .chain(),
        )
        .add_systems(EguiPrimaryContextPass, (panel::draw, hud::draw).chain())
        .run();
}
