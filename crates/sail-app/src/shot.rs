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
