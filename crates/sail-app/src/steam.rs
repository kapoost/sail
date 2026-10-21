//! The seam where Steam is, when Steam is there.
//!
//! **This is a `cargo` feature and it is off by default**, which is not timidity. The
//! Steamworks library is a dynamic one and linking it rewrites the binary's own loader
//! path: build with it and the program will not start — not fail, not warn, *not start*,
//! killed by the dynamic loader before `main` — unless `libsteam_api` is sitting next to
//! the executable. Measured here before a line of this was written:
//!
//! ```text
//! dyld: Library not loaded: @loader_path/libsteam_api.dylib
//! ```
//!
//! So there are two binaries out of one tree. The one on the releases page is built
//! plain, has never heard of Steam, and runs from wherever somebody unzipped it. The one
//! in the Steam depot is built `--features steam` and ships the library beside it. The
//! difference is a build flag and nothing in the game reads differently because of it.
//!
//! What crosses this seam is deliberately thin: **achievements, and nothing else.** Cloud
//! saves need no code at all — Steam Auto-Cloud watches a directory, and `kept::dir()`
//! already is one. Steam Networking is a later piece of work with its own decision behind
//! it. Anything that made the game *different* under Steam would be a second game to
//! test, and there is one pair of hands here.
//!
//! And Steam not being there is the normal case, not the error case. A player who starts
//! the game from a terminal, a screenshot run, every test in this repository and the
//! whole browser build are all "no Steam", so this says so once at a low volume and is
//! never heard from again.

use bevy::prelude::*;

/// Our App ID, once there is one.
///
/// **480 is Spacewar**, Valve's public test application, and it is what this is set to
/// until the partner account clears its bank check and an App ID exists. Against 480 the
/// library initialises, the overlay works, the callbacks run and an achievement unlock is
/// accepted and then quietly dropped, because Spacewar's achievements are not ours — so
/// the plumbing can be exercised end to end and the only thing that cannot be tested is
/// the thing that needs the real number.
pub const APP_ID: u32 = 480;

/// Steam, or the absence of it.
///
/// One resource either way, so that the call sites have no opinion. A game that said
/// `#[cfg(feature = "steam")]` at every place an achievement is earned would be a game
/// where the achievements are only tested in the build nobody develops in.
#[derive(Resource, Default)]
pub struct Steam {
    #[cfg(feature = "steam")]
    client: Option<steamworks::Client>,
}

impl Steam {
    /// Whether we are actually talking to it.
    pub fn aboard(&self) -> bool {
        #[cfg(feature = "steam")]
        {
            self.client.is_some()
        }
        #[cfg(not(feature = "steam"))]
        {
            false
        }
    }

    /// Tell Steam about something that has been done.
    ///
    /// Nothing here reports failure upwards and that is on purpose: an achievement that
    /// did not reach Steam is a thing to put in the log, and a game that stopped to say
    /// so would be interrupting somebody's passage to talk about itself.
    pub fn unlock(&self, api: &str) {
        #[cfg(feature = "steam")]
        if let Some(client) = &self.client {
            let stats = client.user_stats();
            let deed = stats.achievement(api);
            if deed.set().is_err() {
                warn!("steam: {api} would not set");
                return;
            }
            // **Stored, or it never happened.** Setting an achievement marks it in
            // memory; it reaches Steam when the stats are stored, and a game that is
            // closed before a store is a game that gave somebody an achievement and then
            // took it away.
            if stats.store_stats().is_err() {
                warn!("steam: {api} set but not stored");
            }
        }
        #[cfg(not(feature = "steam"))]
        {
            let _ = api;
        }
    }
}

/// Start it, if it is there.
#[cfg(feature = "steam")]
fn aboard(mut steam: ResMut<Steam>) {
    match steamworks::Client::init_app(APP_ID) {
        Ok(client) => {
            info!(
                "steam: aboard as {} on app {}",
                client.friends().name(),
                APP_ID
            );
            steam.client = Some(client);
        }
        // Not an error. The commonest way to run this program is from a terminal.
        Err(e) => info!("steam: not under it ({e}) — playing on regardless"),
    }
}

/// Let it talk back.
///
/// Steam's callbacks are delivered by asking for them, not by a thread of its own, so a
/// game that never asks never gets the overlay, never gets a stats confirmation and never
/// learns that the user signed out.
#[cfg(feature = "steam")]
fn callbacks(steam: Res<Steam>) {
    if let Some(client) = &steam.client {
        client.run_callbacks();
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Steam>();
    // Said in both builds, because "which app is this" is the first question anybody
    // debugging a missing achievement asks, and the build without Steam in it is the one
    // the achievements are developed in.
    info!(
        "steam: app {APP_ID}{}",
        if cfg!(feature = "steam") {
            ""
        } else {
            " — built without it, achievements kept here only"
        }
    );
    #[cfg(feature = "steam")]
    {
        use crate::stages::Deck;
        app.add_systems(Startup, aboard);
        app.add_systems(Update, callbacks.in_set(Deck::Intent));
    }
}
