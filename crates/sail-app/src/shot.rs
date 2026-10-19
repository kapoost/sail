//! A picture of the screen, on a key and on a timer.
//!
//! **Two quite different jobs that are one line of code apart.** `F12` is for a player
//! who has just seen something wrong and wants to show somebody — the fault that cannot
//! be described is the one that never gets fixed, and *here is a photograph of it* is
//! worth a paragraph of prose.
//!
//! The other is development, and it is the reason this exists at all: a window that can
//! be told *start, sail for eight seconds, photograph yourself and quit* can be run from
//! a terminal, which makes the look of the instruments something that can be worked on
//! by looking rather than by asking somebody to take a screenshot and send it. Nothing
//! about a head-up display can be asserted in a test — it is read by eye or it is not
//! read at all — so the next best thing is to make looking cheap.
//!
//! ```text
//! SAIL_SHOT=/tmp/deck.png SAIL_SHOT_AFTER=8 SAIL_SHOT_EXIT=1 sail
//! ```
//!
//! The sea takes a few seconds to settle and the sky is baked on the first frames, so a
//! picture taken at once is a picture of a half-built world. `SAIL_SHOT_AFTER` is in
//! seconds of wall clock and eight is about right.

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

/// What the timed shot is waiting for.
#[derive(Resource)]
pub struct Shot {
    pub path: String,
    pub after: f32,
    pub quit: bool,
    taken: bool,
}

/// Set up from the environment, if it was asked for.
pub fn wanted() -> Option<Shot> {
    // A run of frames reads the same variable and has its own timing; two systems both
    // photographing and both calling for the window to close is a race with a
    // half-written picture at the end of it.
    if std::env::var("SAIL_SHOT_EVERY").is_ok() {
        return None;
    }
    let path = std::env::var("SAIL_SHOT").ok()?;
    Some(Shot {
        path,
        after: std::env::var("SAIL_SHOT_AFTER")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(8.0),
        quit: std::env::var("SAIL_SHOT_EXIT").is_ok(),
        taken: false,
    })
}

/// `F12`: a picture, into the pictures folder, named for the moment it was taken.
pub fn on_a_key(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    sim: Res<crate::sim::Sim>,
) {
    if !keys.just_pressed(KeyCode::F12) {
        return;
    }
    // Named with the ship's clock rather than the wall clock, because what a picture is
    // *of* is a moment in the passage — and because two pictures of the same bug ten
    // seconds apart want to sort in the order they were taken.
    let at = sim.sim.state.time;
    let path = format!(
        "{}/sail-{:02.0}{:02.0}{:02.0}.png",
        pictures(),
        (at / 3_600.0) % 24.0,
        (at / 60.0) % 60.0,
        at % 60.0
    );
    info!("picture: {path}");
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
}

/// Where a Mac keeps pictures, and failing that where the game was started from.
fn pictures() -> String {
    match std::env::var("HOME") {
        Ok(home) if std::path::Path::new(&format!("{home}/Pictures")).is_dir() => {
            format!("{home}/Pictures")
        }
        _ => ".".to_string(),
    }
}

/// The timed one, for working on the instruments from a terminal.
pub fn on_a_timer(
    mut commands: Commands,
    mut shot: ResMut<Shot>,
    time: Res<Time>,
    mut quit: MessageWriter<AppExit>,
    mut since: Local<f32>,
) {
    *since += time.delta_secs();
    if shot.taken {
        // A frame or two for the capture to make it to disk, then out.
        if shot.quit && *since > shot.after + 1.5 {
            quit.write(AppExit::Success);
        }
        return;
    }
    if *since < shot.after {
        return;
    }
    shot.taken = true;
    info!("picture: {} after {:.1} s", shot.path, *since);
    let path = shot.path.clone();
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
}

// ---------------------------------------------------------------------------
// A score for the camera, because the keyboard cannot be driven from outside.
// ---------------------------------------------------------------------------
//
// The timed shot above photographs whatever the boat happens to be doing, which is
// enough for the helm and useless for everything that lives behind a key: the chart
// spread on the table, the glasses up, the help screen. The obvious way to reach those
// from a script is to have the window-server type for us — and it does not work. macOS
// will post synthetic key events through the accessibility API, the game window takes
// focus and accepts the click that goes with them, and `winit` never reports the key.
// Measured, not assumed: `h` and `F1` both delivered to a focused window, both ignored,
// console still up in the photograph.
//
// So the schedule is read here instead, which is better than the thing it replaces.
// It needs no permission from the operating system, it is the same on anybody's machine,
// and it would work on a build server with no desk in front of it.
//
//     SAIL_SHOT_KEYS="70:shift+m" SAIL_SHOT=/tmp/chart.png SAIL_SHOT_AFTER=75 sail
//
// Seconds against keys, separated by semicolons, with `+` for a chord. The names are
// the ones a person would write — `m`, `shift+b`, `f1`, `esc`, `enter`, `[`.
//
// **A key is held for exactly one frame and then let go.** `ButtonInput` clears
// `just_pressed` every frame by itself but goes on reporting `pressed` until something
// releases it, so a key pressed and forgotten is a key held down for the rest of the
// run — which, on the tiller, is a boat going round in a circle. Each entry therefore
// releases on the frame after it presses.

/// What to press, and when.
#[derive(Resource)]
pub struct Score {
    /// Seconds since start, the keys to press at that moment, and whether it has fired.
    cues: Vec<(f32, Vec<KeyCode>, bool)>,
    since: f32,
    /// Pressed on the last frame and due to be released on this one.
    held: Vec<KeyCode>,
}

/// The keys this program answers to, by the name somebody would type.
///
/// Deliberately the same set as `help::DOCUMENTED` — a schedule cannot ask for a key
/// the game does not have, and the test in `help.rs` reads this file like any other,
/// so a `KeyCode` named here that is not in that list fails the build.
fn key_named(name: &str) -> Option<KeyCode> {
    Some(match name.trim().to_ascii_lowercase().as_str() {
        "left" => KeyCode::ArrowLeft,
        "right" => KeyCode::ArrowRight,
        "up" => KeyCode::ArrowUp,
        "down" => KeyCode::ArrowDown,
        "space" => KeyCode::Space,
        "shift" => KeyCode::ShiftLeft,
        "esc" => KeyCode::Escape,
        "enter" => KeyCode::Enter,
        "f1" => KeyCode::F1,
        "f12" => KeyCode::F12,
        "/" | "slash" => KeyCode::Slash,
        "," => KeyCode::Comma,
        "." => KeyCode::Period,
        "-" => KeyCode::Minus,
        "=" => KeyCode::Equal,
        "[" => KeyCode::BracketLeft,
        "]" => KeyCode::BracketRight,
        "1" => KeyCode::Digit1,
        "2" => KeyCode::Digit2,
        "a" => KeyCode::KeyA,
        "b" => KeyCode::KeyB,
        "c" => KeyCode::KeyC,
        "d" => KeyCode::KeyD,
        "e" => KeyCode::KeyE,
        "f" => KeyCode::KeyF,
        "g" => KeyCode::KeyG,
        "h" => KeyCode::KeyH,
        "i" => KeyCode::KeyI,
        "j" => KeyCode::KeyJ,
        "k" => KeyCode::KeyK,
        "l" => KeyCode::KeyL,
        "m" => KeyCode::KeyM,
        "n" => KeyCode::KeyN,
        "o" => KeyCode::KeyO,
        "p" => KeyCode::KeyP,
        "q" => KeyCode::KeyQ,
        "r" => KeyCode::KeyR,
        "s" => KeyCode::KeyS,
        "t" => KeyCode::KeyT,
        "u" => KeyCode::KeyU,
        "w" => KeyCode::KeyW,
        "x" => KeyCode::KeyX,
        "z" => KeyCode::KeyZ,
        _ => return None,
    })
}

/// Read the schedule, if one was asked for. A name it cannot place is fatal rather
/// than skipped: a scene that silently did not happen is a photograph of the wrong
/// thing, and it looks exactly like a photograph of the right thing.
pub fn scored() -> Option<Score> {
    let text = std::env::var("SAIL_SHOT_KEYS").ok()?;
    let mut cues = Vec::new();
    for entry in text.split(';').filter(|s| !s.trim().is_empty()) {
        let (at, keys) = entry
            .split_once(':')
            .unwrap_or_else(|| panic!("SAIL_SHOT_KEYS: {entry:?} is not `seconds:keys`"));
        let at: f32 = at
            .trim()
            .parse()
            .unwrap_or_else(|_| panic!("SAIL_SHOT_KEYS: {at:?} is not a number of seconds"));
        let chord: Vec<KeyCode> = keys
            .split('+')
            .map(|k| {
                key_named(k).unwrap_or_else(|| panic!("SAIL_SHOT_KEYS: no key called {k:?}"))
            })
            .collect();
        cues.push((at, chord, false));
    }
    cues.sort_by(|a, b| a.0.total_cmp(&b.0));
    info!("score: {} cues", cues.len());
    Some(Score {
        cues,
        since: 0.0,
        held: Vec::new(),
    })
}

/// Press what is due, and let go of what was pressed last frame.
///
/// Runs in `PreUpdate` after the input plugin has done its own clearing, so that every
/// system in `Update` sees the same `just_pressed` a real key would have produced.
pub fn on_a_schedule(
    mut score: ResMut<Score>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    time: Res<Time>,
) {
    for key in score.held.drain(..) {
        keys.release(key);
    }
    score.since += time.delta_secs();
    let now = score.since;
    let mut pressed = Vec::new();
    for (at, chord, fired) in score.cues.iter_mut() {
        if !*fired && now >= *at {
            *fired = true;
            info!("score: {:.1} s — {:?}", now, chord);
            pressed.extend(chord.iter().copied());
        }
    }
    for key in pressed {
        keys.press(key);
        score.held.push(key);
    }
}

// ---------------------------------------------------------------------------
// A run of frames, for showing motion.
// ---------------------------------------------------------------------------
//
// A still photograph cannot show the two things this boat is most about: that she heels
// and that the sea moves under her. Screen recording would, and it wants a permission
// the operating system only grants by hand — the same wall the key schedule above ran
// into. So the window photographs itself repeatedly instead, and the frames are made
// into a loop outside.
//
//     SAIL_SHOT=/tmp/f.png SAIL_SHOT_EVERY=0.25 SAIL_SHOT_FRAMES=24 SAIL_SHOT_AFTER=70
//
// The frames land beside the named file, numbered: `/tmp/f-000.png`, `f-001.png`, and
// so on. It is not free — each frame is a few megabytes through the encoder — so this
// is a handful of seconds for a web page, not a recording tool.

/// A run of frames rather than a single picture.
#[derive(Resource)]
pub struct Reel {
    stem: String,
    every: f32,
    frames: usize,
    taken: usize,
    since: f32,
    next: f32,
    pub quit: bool,
}

/// Set up from the environment, if a run was asked for.
pub fn reel() -> Option<Reel> {
    let every: f32 = std::env::var("SAIL_SHOT_EVERY").ok()?.parse().ok()?;
    let path = std::env::var("SAIL_SHOT").expect("SAIL_SHOT_EVERY needs SAIL_SHOT");
    let stem = path.strip_suffix(".png").unwrap_or(&path).to_string();
    Some(Reel {
        stem,
        every,
        frames: std::env::var("SAIL_SHOT_FRAMES")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(24),
        taken: 0,
        since: 0.0,
        next: std::env::var("SAIL_SHOT_AFTER")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(8.0),
        quit: std::env::var("SAIL_SHOT_EXIT").is_ok(),
    })
}

/// One frame every `every` seconds, until there are enough of them.
pub fn on_a_reel(
    mut commands: Commands,
    mut reel: ResMut<Reel>,
    time: Res<Time>,
    mut quit: MessageWriter<AppExit>,
) {
    reel.since += time.delta_secs();
    if reel.taken >= reel.frames {
        // A moment for the last frame to reach the disk, then out.
        if reel.quit && reel.since > reel.next + 2.0 {
            quit.write(AppExit::Success);
        }
        return;
    }
    if reel.since < reel.next {
        return;
    }
    let path = format!("{}-{:03}.png", reel.stem, reel.taken);
    reel.taken += 1;
    reel.next += reel.every;
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
}
