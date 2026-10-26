//! sail — presentation layer.
//!
//! The only crate in the workspace that knows a game engine exists. Everything
//! numerically interesting lives in `sail-core`, which has no engine types in it at
//! all; this binary reads its state and draws it. That split is what keeps the engine
//! a replaceable component, and it is why choosing Bevy here is a decision that can be
//! revisited for the cost of rewriting this one crate.

mod aboard;
mod anchorage;
mod band;
mod barrels;
mod basics;
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
mod earned;
mod fishing;
mod fittings;
mod fleet;
mod frame;
mod glance;
mod hail;
mod help;
mod horizon;
mod hud;
mod ink;
mod input;
mod islands;
mod kept;
mod knob;
mod logbook;
mod marks;
mod mission;
mod mob;
mod music;
mod navigation;
mod net;
mod orders;
mod panel;
mod preferences;
mod race;
mod radio;
mod rain;
mod reel;
mod retro;
mod rig;
mod rod;
mod sail_texture;
mod scenarios;
mod screen;
mod sea_card;
mod sea_texture;
mod settings;
mod ship;
mod shot;
mod sight;
mod signals;
mod sim;
mod sky;
mod sound;
mod spray;
mod stages;
mod steam;
mod stowage;
mod swing;
mod tutorial;
mod wake;
mod weather;
mod windstreaks;
mod world;

use bevy::prelude::*;
use bevy::window::WindowResizeConstraints;
use bevy_egui::EguiPlugin;

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
#[cfg(not(target_arch = "wasm32"))]
const HULL: &[u8] = include_bytes!("../../../assets/venus.glb");

/// And the sea's shaders, for the same reason.
///
/// They are read off the disk like any other asset — the renderer loads them by path and
/// recompiles them if they change, which is what makes a shader something you can work
/// on by looking — so a binary handed to a friend has to carry them too. Three small
/// text files against ten megabytes of hull; the mechanism is already here.
#[cfg(not(target_arch = "wasm32"))]
const SHADERS: [(&str, &str); 3] = [
    ("sea.wesl", include_str!("../../../assets/sea.wesl")),
    (
        "sea_prepass.wesl",
        include_str!("../../../assets/sea_prepass.wesl"),
    ),
    (
        "sea_wave.wesl",
        include_str!("../../../assets/sea_wave.wesl"),
    ),
];

/// Where the asset server should look.
///
/// Three places, in this order, and the order is the development loop: a loose `assets`
/// directory beside the executable wins, so a new hull can be dropped in without a
/// rebuild; then the source tree, for `cargo run`; and only then the copy carried inside
/// the binary, unpacked. `SAIL_ASSETS=embedded` skips straight to the last one, which is
/// how the path a friend will take gets tested on a machine that has the other two.
fn asset_root() -> String {
    // A tab has no executable, no source tree and nowhere to unpack to — and it does not
    // need any of them: the asset server fetches over HTTP, so the directory served
    // beside the page *is* the asset root. This is the one platform where the fallback
    // path was always the right answer.
    #[cfg(target_arch = "wasm32")]
    {
        return String::from("assets");
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let forced = crate::knob::var("SAIL_ASSETS").is_ok_and(|v| v == "embedded");
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
}

/// Put the embedded hull somewhere the asset server can read it, and return that
/// directory.
///
/// Skipped when the file is already there with the right length, so the
/// second run costs a stat.
#[cfg(not(target_arch = "wasm32"))]
fn unpacked() -> Option<String> {
    let dir = std::env::temp_dir().join(concat!("sail-assets-", env!("CARGO_PKG_VERSION")));
    std::fs::create_dir_all(&dir).ok()?;
    let hull = dir.join("venus.glb");
    let fresh = std::fs::metadata(&hull).is_ok_and(|m| m.len() == HULL.len() as u64);
    if !fresh {
        std::fs::write(&hull, HULL).ok()?;
        println!("sail: unpacked the hull to {}", dir.display());
    }
    for (name, source) in SHADERS {
        let at = dir.join(name);
        // Length is enough to tell a stale copy from a fresh one here, as it is for the
        // hull: the directory is named for the version, so the only way to find a file
        // of the right length that is the wrong file is to have edited it by hand in a
        // temporary directory, which is a thing somebody doing that would want to work.
        if !std::fs::metadata(&at).is_ok_and(|m| m.len() == source.len() as u64) {
            std::fs::write(&at, source).ok()?;
        }
    }
    Some(dir.to_string_lossy().into_owned())
}

/// The window, and the knob for looking at a smaller one.
fn window_size() -> (u32, u32) {
    let asked = crate::knob::var("SAIL_WINDOW").ok().and_then(|v| {
        let (w, h) = v.split_once(['x', 'X'])?;
        Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
    });
    asked.unwrap_or((1440, 880))
}

/// The file the log is copied into, opened fresh each launch.
///
/// `None` when there is nowhere to keep it — a browser, or a machine whose home
/// directory cannot be found — and then the program behaves exactly as it did before,
/// which is to say it talks to standard output and nowhere else.
#[cfg(not(target_arch = "wasm32"))]
fn log_to_a_file() -> Option<bevy::log::BoxedLayer> {
    use bevy::log::tracing_subscriber::Layer;
    let path = kept::at("log")?;
    std::fs::create_dir_all(path.parent()?).ok()?;
    let file = std::fs::File::create(&path).ok()?;
    Some(Box::new(
        bevy::log::tracing_subscriber::fmt::layer()
            .with_writer(file)
            // No colour: this is a file somebody will open in a text editor or paste
            // into a bug report, and escape codes in either are noise.
            .with_ansi(false)
            .boxed(),
    ))
}

#[cfg(target_arch = "wasm32")]
fn log_to_a_file() -> Option<bevy::log::BoxedLayer> {
    None
}

fn main() {
    // **The sea she is moored in, and a day that has not been sailed before.**
    //
    // Two seeds, because they answer two questions. The archipelago is the place you come
    // back to — the platform at `anchorage.rs` is moored in it, so a fresh draw every
    // launch would move the water out from under it. The *day* is the opposite: weather
    // you have already had is a replay. So the islands take the saved sea and the weather
    // takes the clock, and `SAIL_SEED=` pins both together, which is what makes a reported
    // bug reproducible and how a new world is asked for.
    let seed = knob::seed();
    #[cfg(not(target_arch = "wasm32"))]
    let home = anchorage::Home::read();
    #[cfg(not(target_arch = "wasm32"))]
    let sea = match (knob::var("SAIL_SEED").is_ok(), &home) {
        (false, Some(home)) if home.moored.is_some() => home.sea,
        _ => seed,
    };
    #[cfg(target_arch = "wasm32")]
    let sea = seed;
    // `--licences` before anything else, because it has to work in a build that cannot
    // open a window — and because the single-file build carries the fonts and the hull
    // *inside* it, which is only allowed if the licences travel with them. The OFL says
    // the text must accompany the font; embedding it and printing it on request is how
    // one file does that honestly.
    // `SAIL_BAND=music.wav` records the band and nothing else, with no window and no
    // audio device — see `band.rs`. Before the window for the same reason `--licences`
    // is: it has to work in a build that cannot open one, and on a machine where
    // nothing is listening.
    #[cfg(not(target_arch = "wasm32"))]
    if let Some((path, seconds)) = band::asked() {
        match band::record(&path, seconds, seed) {
            Ok(()) => println!("band: {seconds:.1} s written to {path}"),
            Err(e) => eprintln!("band: {path}: {e}"),
        }
        return;
    }

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
            // **A copy of the log, in a file beside the save.**
            //
            // Everything this program says goes to standard output, which is perfect for
            // the person who started it from a terminal and useless to everybody else: a
            // game opened by double-clicking has no console, so the one thing a player
            // could send after a crash was a description. The file is written fresh each
            // launch and holds the last run — the forecast, the seed, what was built,
            // what went wrong — which is exactly what a bug report needs attached to it.
            .set(bevy::log::LogPlugin {
                custom_layer: |_| log_to_a_file(),
                ..default()
            })
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
                    // **It resizes now, and it goes full screen.** It did not, and the
                    // reasoning was sound as far as it went: the instruments are not a
                    // responsive layout, so a window at some other size was a promise the
                    // drawing could not keep. What changed is that the drawing can keep
                    // it — the world is rendered at a fixed drawn width and scaled to
                    // whatever the window is, and the console shrinks until the strip
                    // fits. See `screen.rs` for the rest of that argument, and `F11`.
                    resizable: true,
                    mode: screen::Screen::asked().mode(),
                    // The floor, because a window that can be dragged into uselessness is
                    // a bug report nobody can diagnose.
                    resize_constraints: WindowResizeConstraints {
                        min_width: screen::NARROWEST,
                        min_height: screen::SHORTEST,
                        ..default()
                    },
                    // The page owns the canvas, and for the same reason the window is
                    // a fixed size: the instruments are drawn at known pixel sizes, so
                    // the picture is scaled by the page rather than re-laid-out by it.
                    // `fit_canvas_to_parent` would do the opposite — change the
                    // resolution with the viewport — and put the needles off their
                    // dials on every phone it met.
                    #[cfg(target_arch = "wasm32")]
                    canvas: Some(String::from("#sail")),
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
    // The sea's own material: the standard one with a vertex shader of its own. The
    // plugin is what builds its pipeline and loads the two shaders beside it.
    .add_plugins(MaterialPlugin::<sea_card::SeaMaterial>::default())
    .insert_resource(sim::Sim::new(weather, sea))
    .insert_resource(islands::Sea(sea))
    .insert_resource(weather)
    .insert_resource(sky::Sky::at(
        weather.start_hour as f32,
        weather.cover as f32,
    ))
    // What is on the platform, and which sea it is moored in. On a desk it is read from
    // a file; a tab has nowhere to keep it and starts with nothing, which is what a
    // first minute should do anyway.
    .insert_resource({
        #[cfg(not(target_arch = "wasm32"))]
        {
            // A home from another sea is not this home: `SAIL_SEED=` means a different
            // archipelago, and a platform moored at a position in that one would be in
            // open water or inside a hill. It is left on disk and ignored.
            match home {
                Some(home) if home.sea == sea => home,
                _ => anchorage::Home {
                    sea,
                    ..Default::default()
                },
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            anchorage::Home {
                sea,
                ..Default::default()
            }
        }
    })
    // The gear is drawn against the same seed as the weather, so a day worth
    // sailing again is also a day worth fishing again: the same fish take in the
    // same places, and a passage can be repeated rather than merely re-rolled.
    .insert_resource(fishing::Fishing::new(weather.seed))
    // `SAIL_TUTORIAL=1` starts the drill on the first frame, which is how it gets
    // looked at twenty times in an afternoon without clicking through a panel.
    .insert_resource(basics::Basics::asked_for(
        crate::knob::var("SAIL_LESSON").is_ok_and(|v| v != "0"),
    ))
    .insert_resource(tutorial::Tutorial::asked_for(
        crate::knob::var("SAIL_TUTORIAL").is_ok(),
    ))
    // The order of a frame, named once. Before the plugins, because they all put their
    // systems into the sets it configures.
    .add_plugins(stages::order)
    // **Everything else is a plugin, and the order of a frame is in `stages.rs`.**
    //
    // This used to be the whole schedule written out here: twenty-seven systems in a
    // chained Startup tuple, a hundred and five in a chained Update one nested three
    // deep, and two more bolted on in their own calls because the tuple was full. It
    // was a frame whose shape was decided by a type limit.
    //
    // Now each module says what it does and which stage it does it in, and this is the
    // list of modules. In four groups because a tuple of plugins has a length too — but
    // they are flat, they are grouped by what the modules are rather than by when they
    // run, and adding a mechanic is a line in its own file and a name here.
    .add_plugins((
        // The frame itself: intent, the step, where it is all drawn from.
        input::plugin,
        clutch::plugin,
        fishing::plugin,
        sim::plugin,
        frame::plugin,
        aboard::plugin,
        screen::plugin,
        reel::plugin,
        steam::plugin,
        earned::plugin,
        hail::plugin,
        retro::plugin,
        ink::plugin,
    ))
    .add_plugins((
        // The world and the boat in it.
        world::plugin,
        islands::plugin,
        horizon::plugin,
        clouds::plugin,
        sky::plugin,
        rain::plugin,
        windstreaks::plugin,
        camera::plugin,
        boat::plugin,
        rig::plugin,
        fittings::plugin,
        wake::plugin,
        spray::plugin,
        sound::plugin,
        music::plugin,
    ))
    .add_plugins((
        // What is on her deck, where she is going, and who else is out there.
        stowage::plugin,
        rod::plugin,
        creel::plugin,
        mob::plugin,
        boathook::plugin,
        barrels::plugin,
        crew::plugin,
        orders::plugin,
        course::plugin,
        marks::plugin,
        chart::plugin,
        navigation::plugin,
        sight::plugin,
        signals::plugin,
        anchorage::plugin,
    ))
    .add_plugins((
        // The missions, the people you meet, and the panels you read.
        mission::plugin,
        race::plugin,
        ship::plugin,
        fleet::plugin,
        panel::plugin,
        preferences::plugin,
        settings::plugin,
        help::plugin,
        logbook::plugin,
        hud::plugin,
        tutorial::plugin,
        basics::plugin,
        glance::plugin,
        net::plugin,
        shot::plugin,
    ))
    // Its own call: a plugin tuple goes to fifteen and the one above is full. Said here
    // rather than by shuffling the list, because that grouping is a reading order and
    // reordering it to make room would cost more than a line does.
    .add_plugins(radio::plugin);
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
