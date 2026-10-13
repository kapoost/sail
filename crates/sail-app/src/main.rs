//! sail — presentation layer.
//!
//! The only crate in the workspace that knows a game engine exists. Everything
//! numerically interesting lives in `sail-core`, which has no engine types in it at
//! all; this binary reads its state and draws it. That split is what keeps the engine
//! a replaceable component, and it is why choosing Bevy here is a decision that can be
//! revisited for the cost of rewriting this one crate.

mod boat;
mod camera;
mod chart;
mod clouds;
mod clutch;
mod course;
mod deck;
mod deck_texture;
mod fittings;
mod frame;
mod horizon;
mod hud;
mod ink;
mod input;
mod islands;
mod panel;
mod rain;
mod retro;
mod rig;
mod sail_texture;
mod sea_texture;
mod settings;
mod sim;
mod sky;
mod sound;
mod spray;
mod wake;
mod weather;
mod windstreaks;
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
/// The scanned hull, carried inside the executable.
///
/// Everything else in this game is generated at startup — the sea, the sky, the deck
/// planking, the compass card, the sail cloth, the sound — and the one thing that is not
/// is this: ten megabytes of measured hull. Handing a friend *one file* means it has to
/// travel inside that file.
///
/// `bevy_embedded_assets` would have done it with one line and there is no release of it
/// for Bevy 0.20 — its 0.16 targets Bevy 0.16 and does not implement this version's
/// `Plugin` trait at all. Writing a custom `AssetReader` to avoid touching the disk is
/// the pure answer and it is a lot of API archaeology for a single file; unpacking it
/// beside the user's temporary directory on first run is four lines, cannot be wrong in
/// an interesting way, and leaves the distributed artefact a single file either way.
const HULL: &[u8] = include_bytes!("../../../assets/venus.glb");

/// Where the asset server should look.
///
/// Three places, in this order, and the order is the development loop: a loose `assets`
/// directory beside the executable wins, so a new hull can be dropped in without a
/// rebuild; then the source tree, for `cargo run`; and only then the copy carried inside
/// the binary, unpacked. `SAIL_ASSETS=embedded` skips straight to the last one, which is
/// how the path a friend will take gets tested on a machine that has the other two.
fn asset_root() -> String {
    let forced = std::env::var("SAIL_ASSETS").is_ok_and(|v| v == "embedded");
    if !forced {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(beside) = exe.parent().map(|d| d.join("assets")) {
                if beside.is_dir() {
                    return beside.to_string_lossy().into_owned();
                }
            }
        }
        let source = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");
        if std::path::Path::new(source).is_dir() {
            return source.to_string();
        }
    }
    unpacked().unwrap_or_else(|| {
        // Nothing to be done about it here — the window has not opened and there is no
        // logger yet — so say so on the terminal and let the asset server fail loudly.
        println!("sail: could not unpack the hull; the boat will have no hull in it");
        String::from("assets")
    })
}

/// Put the embedded hull somewhere the asset server can read it, and return that
/// directory. Skipped when the file is already there with the right length, so the
/// second run costs a stat.
fn unpacked() -> Option<String> {
    let dir = std::env::temp_dir().join(concat!("sail-assets-", env!("CARGO_PKG_VERSION")));
    std::fs::create_dir_all(&dir).ok()?;
    let hull = dir.join("venus.glb");
    let fresh = std::fs::metadata(&hull).is_ok_and(|m| m.len() == HULL.len() as u64);
    if !fresh {
        std::fs::write(&hull, HULL).ok()?;
        println!("sail: unpacked the hull to {}", dir.display());
    }
    Some(dir.to_string_lossy().into_owned())
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
    // `--licences` before anything else, because it has to work in a build that cannot
    // open a window — and because the single-file build carries the fonts and the hull
    // *inside* it, which is only allowed if the licences travel with them. The OFL says
    // the text must accompany the font; embedding it and printing it on request is how
    // one file does that honestly.
    if std::env::args().any(|a| a == "--licences" || a == "--licenses") {
        println!("{}", include_str!("../../../assets/ATTRIBUTION.md"));
        println!("\n--- VT323, SIL Open Font Licence ---\n");
        println!("{}", include_str!("../../../assets/VT323-OFL.txt"));
        return;
    }

    let weather = weather::Weather::draw(seed);
    // Printed rather than logged: the draw happens before Bevy's log plugin exists,
    // so `info!` here goes nowhere at all.
    println!("forecast: {}", weather.report());

    // Handing out one file to a friend means the hull has to be *inside* it. Everything
    // else in this game is generated at startup — the sea, the sky, the deck planking,
    // the compass card, the sound — and the one thing that is not is the scanned hull,
    // ten megabytes of GLB that the asset server reads off the disk. Embedded, it is
    // read out of the binary instead, and `ReplaceDefault` means nothing downstream has
    // to know: `assets.load("venus.glb")` goes on saying exactly that.
    //
    // The loose `assets` directory still wins when it exists, which is what keeps the
    // development loop: drop a new hull in and run, no rebuild.
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
        // The plugin must not claim the first camera for its context: that is the one
        // looking at the sea, and in the pixelated picture it draws into a 480-pixel
        // image. The instruments get a camera of their own — see `retro::UiCamera`.
        .insert_resource(bevy_egui::EguiGlobalSettings {
            auto_create_primary_context: false,
            ..default()
        })
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
        .insert_resource(sky::Sky::at(
            weather.start_hour as f32,
            weather.cover as f32,
        ))
        .init_resource::<sky::Baking>()
        .init_resource::<settings::Settings>()
        .insert_resource(retro::Retro::from_env())
        .insert_resource(world::Ink::from_env())
        .init_resource::<world::SeaFilm>()
        .init_resource::<chart::Chart>()
        .init_resource::<chart::Track>()
        .init_resource::<world::SurfaceDrift>()
        .init_resource::<camera::ChaseState>()
        .init_resource::<camera::ViewAim>()
        .add_systems(
            Startup,
            (
                retro::setup_ui_camera,
                world::setup,
                boat::setup,
                boat::report,
                islands::setup,
                horizon::setup,
                clouds::setup,
                camera::setup,
                hud::build_polar,
                sound::setup,
                course::setup,
                wake::setup,
                spray::setup,
                rain::setup,
                rig::setup,
                panel::setup,
                windstreaks::setup,
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
                world::report_vitals,
                world::report_frame_times,
                chart::record,
                islands::follow_origin,
                (
                    boat::update_hull,
                    boat::update_rudder,
                    fittings::update_tiller,
                    fittings::update_compass,
                    boat::update_boom,
                    boat::update_staysail_boom,
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
                    windstreaks::update,
                    spray::update,
                    clouds::update,
                ),
                (
                    // The weather you can see, after everything that places it.
                    fittings::update_vane,
                    horizon::follow,
                    sky::drift,
                    sky::light,
                    rain::update,
                    rain::shroud,
                    settings::toggle,
                    camera::toggle,
                    camera::aim,
                    hud::toggle,
                    course::restart,
                    chart::toggle,
                    retro::toggle,
                    world::toggle_ink,
                    world::roll_sea_film,
                    ink::apply,
                    retro::apply,
                ),
            )
                .chain(),
        )
        .add_systems(
            EguiPrimaryContextPass,
            (
                // Once, at the start: `set_fonts` rebuilds the atlas, so it must not
                // be called every frame.
                retro::install_font.run_if(bevy::ecs::schedule::common_conditions::run_once),
                retro::scale_instruments,
                panel::draw,
                settings::draw,
                chart::draw,
                hud::draw,
            )
                .chain(),
        )
        .run();
}
