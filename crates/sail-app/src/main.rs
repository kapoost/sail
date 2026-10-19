//! sail — presentation layer.
//!
//! The only crate in the workspace that knows a game engine exists. Everything
//! numerically interesting lives in `sail-core`, which has no engine types in it at
//! all; this binary reads its state and draws it. That split is what keeps the engine
//! a replaceable component, and it is why choosing Bevy here is a decision that can be
//! revisited for the cost of rewriting this one crate.

mod barrels;
mod boat;
mod boathook;
mod camera;
mod chart;
mod clouds;
mod clutch;
mod course;
mod creel;
mod crew;
mod curve;
mod deck;
mod deck_texture;
mod fishing;
mod fittings;
mod fleet;
mod frame;
mod glance;
mod help;
mod horizon;
mod hud;
mod ink;
mod input;
mod islands;
mod logbook;
mod marks;
mod mission;
mod mob;
mod music;
mod navigation;
mod net;
mod orders;
mod panel;
mod race;
mod rain;
mod retro;
mod rig;
mod rod;
mod sail_texture;
mod scenarios;
mod sea_texture;
mod settings;
mod ship;
mod shot;
mod sight;
mod sim;
mod sky;
mod sound;
mod spray;
mod stowage;
mod swing;
mod tutorial;
mod wake;
mod weather;
mod windstreaks;
mod world;

use bevy::prelude::*;
use bevy::window::EnabledButtons;
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

/// The window, and the knob for looking at a smaller one.
fn window_size() -> (u32, u32) {
    let asked = std::env::var("SAIL_WINDOW").ok().and_then(|v| {
        let (w, h) = v.split_once(['x', 'X'])?;
        Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
    });
    asked.unwrap_or((1440, 880))
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
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: asset_root(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "sail".into(),
                    // `SAIL_WINDOW=1024x640` for looking at the console on a screen
                    // smaller than this one. The window is not resizable — see below —
                    // but a laptop whose display is smaller than the window gets one
                    // anyway, and that case has to be looked at rather than reasoned
                    // about.
                    resolution: window_size().into(),
                    // The window is fixed at this size on purpose. The instruments are
                    // not a responsive layout: the console, the chart and the gauges are
                    // drawn at known pixel sizes into a 480-line image that is then
                    // scaled, and at any other aspect the needles sit off their dials and
                    // the chart corner crawls out of the frame. A resizable window offers
                    // a promise the picture cannot keep, so it is withdrawn.
                    resizable: false,
                    // Resizing is also reachable past `resizable` on macOS: the green
                    // button zooms the window to the display. Turning the button off is
                    // part of the same decision, not a separate nicety.
                    enabled_buttons: EnabledButtons {
                        maximize: false,
                        ..default()
                    },
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
    .init_resource::<help::Help>()
    .init_resource::<logbook::Logbook>()
    .insert_resource(retro::Retro::from_env())
    .insert_resource(world::Ink::from_env())
    .init_resource::<world::SeaFilm>()
    .init_resource::<music::Music>()
    .init_resource::<fleet::Fleet>()
    .init_resource::<chart::Chart>()
    .init_resource::<chart::Track>()
    .init_resource::<chart::DeckLog>()
    .init_resource::<barrels::Flotsam>()
    .init_resource::<world::SurfaceDrift>()
    .init_resource::<camera::ChaseState>()
    // The gear is drawn against the same seed as the weather, so a day worth
    // sailing again is also a day worth fishing again: the same fish take in the
    // same places, and a passage can be repeated rather than merely re-rolled.
    .insert_resource(fishing::Fishing::new(weather.seed))
    .init_resource::<camera::ViewAim>()
    .init_resource::<boathook::Boathook>()
    .init_resource::<crew::Crew>()
    .init_resource::<ship::Brig>()
    .init_resource::<mission::Pilotage>()
    .init_resource::<mission::WantsPilotage>()
    .init_resource::<orders::Orders>()
    .init_resource::<race::Race>()
    .init_resource::<navigation::Navigation>()
    .init_resource::<sight::Sighting>()
    .init_resource::<marks::Coast>()
    // `SAIL_TUTORIAL=1` starts the drill on the first frame, which is how it gets
    // looked at twenty times in an afternoon without clicking through a panel.
    .insert_resource(tutorial::Tutorial::asked_for(
        std::env::var("SAIL_TUTORIAL").is_ok(),
    ))
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
            // The deck gear, the rod, and the basket the catch goes in: things put
            // on the boat once she exists. Grouped because a startup tuple is
            // twenty long at the most and this one is full.
            (stowage::setup, rod::setup, creel::setup),
            (
                mob::setup,
                boathook::setup,
                ship::setup,
                boathook::mark_setup,
                tutorial::setup,
                marks::setup,
                net::setup,
            ),
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
            // The rod is worked before the physics for the same reason the helm is:
            // it is intent, and intent belongs at the top of the frame. The fight
            // reads last frame's speed through the water, which is five milliseconds
            // stale and matters to nothing.
            fishing::work,
            sim::advance,
            // Pick where the world is drawn from before anything is placed in it,
            // so the whole frame agrees on one mark.
            frame::rebase_origin,
            world::report_vitals,
            world::report_frame_times,
            chart::record,
            (
                islands::follow_origin,
                marks::follow_origin,
                marks::keep_the_coast,
            ),
            // Everybody else's boats: told where they are, given their hulls, then
            // placed and dressed like any other thing in the scene. One group of its
            // own because the four have to happen in that order and because a tuple
            // of systems is twenty long at the most.
            (
                net::exchange,
                net::rebuild,
                fleet::rehearsal,
                fleet::sync,
                fleet::place,
                fleet::dress,
            )
                .chain(),
            (
                boat::update_hull,
                boat::update_rudder,
                fittings::update_tiller,
                fittings::update_compass,
                boat::update_boom,
                (
                    boat::update_staysail_boom,
                    boat::update_pole,
                    (stowage::update, stowage::flutter),
                ),
                boat::update_windex,
                boat::update_genoa_cars,
                boat::update_telltales,
                boat::update_sails,
                course::update,
                world::update_sea,
                course::float_marks,
                // The things that live in the water she has just left.
                (wake::update, rod::update, creel::update, mob::update),
                rig::update,
                camera::follow,
                sound::update,
                windstreaks::update,
                spray::update,
                clouds::update,
            ),
            (
                // The weather you can see, after everything that places it.
                (
                    fittings::update_vane,
                    fittings::light_the_lamps,
                    marks::flash,
                ),
                horizon::follow,
                sky::drift,
                sky::light,
                rain::update,
                rain::shroud,
                (settings::toggle, settings::apply),
                (help::toggle, logbook::toggle, mob::throw, tutorial::run),
                (camera::toggle, camera::dress, glance::posts),
                camera::aim,
                (
                    course::restart,
                    shot::on_a_key,
                    (
                        barrels::scatter,
                        barrels::drift,
                        barrels::dress,
                        barrels::float,
                        barrels::land_them,
                    )
                        .chain(),
                    navigation::lost_at_sea,
                    navigation::enter_in_the_log,
                    navigation::write_up_the_log,
                    navigation::the_fix_holds,
                    navigation::reckon,
                    orders::carry_out,
                    crew::work,
                    // The brig and the job she is here for: she is sailed, then told
                    // where to go, then drawn — in that order, because her yards are
                    // braced for the wind she is in this frame and not last one's.
                    (
                        mission::take_the_job,
                        mission::discharge,
                        mission::fly_the_flag,
                        mission::lay_off_the_route,
                        race::run,
                        race::rivals,
                        race::dress,
                        mission::run,
                        ship::sail_her,
                        ship::work_her,
                        ship::place,
                    )
                        .chain(),
                    boathook::reach,
                    boathook::mark_the_water,
                    boathook::dress,
                    sight::aim,
                    sight::glasses,
                    sight::recognise,
                ),
                chart::toggle,
                chart::stopwatch,
                chart::by_hand,
                retro::stress.run_if(|| std::env::var("SAIL_RETRO_STRESS").is_ok()),
                music::compose,
                world::apply_ink,
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
            help::draw,
            chart::draw,
            logbook::draw,
            logbook::lost,
            hud::draw,
            tutorial::draw,
            sight::draw,
            navigation::adrift_card,
            chart::glass,
            boathook::draw,
        )
            .chain(),
    )
    // The two missions' deck lines, in their own call because a tuple of systems has a
    // length and that one is at it. Ordered after the boathook's prompt for the same
    // reason everything in the list above is ordered: the last thing drawn is the thing
    // on top, and a mission's one line belongs over the instruments rather than under
    // them.
    .add_systems(
        EguiPrimaryContextPass,
        (mission::draw, race::draw).chain().after(boathook::draw),
    );
    // A picture of herself on a timer, when one was asked for: see `shot`.
    if let Some(shot) = shot::wanted() {
        app.insert_resource(shot)
            .add_systems(Update, shot::on_a_timer);
    }
    // Or a run of them, when the thing worth showing is motion.
    if let Some(reel) = shot::reel() {
        app.insert_resource(reel)
            .add_systems(Update, shot::on_a_reel);
    }
    // And the keys to press before it, for the views that live behind one. In
    // `PreUpdate` after the input plugin, so a scripted key is indistinguishable from
    // a real one to everything in `Update`.
    if let Some(score) = shot::scored() {
        app.insert_resource(score).add_systems(
            PreUpdate,
            shot::on_a_schedule.after(bevy::input::InputSystems),
        );
    }
    app.run();
}
