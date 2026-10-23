//! What the player has set, kept between evenings.
//!
//! **Every dial in this game went back to its default at the next launch.** The volume,
//! the view, the drawn width, the frame counter, whether the window is full screen —
//! fifty environment variables for the person developing it and nothing at all for the
//! person playing it, who had to set the sound again every single time. The one thing
//! that survived was the address of the last game joined, which tells you how long ago
//! somebody noticed this problem and how narrowly they fixed it.
//!
//! So: a second file beside the platform's, in the same lines-of-text form, holding the
//! handful of things that are a *preference* rather than a world. It is deliberately not
//! in the save: your sea and what is on your platform belong to a world you can leave;
//! how loud you like the rigging does not.
//!
//! **A dial still wins.** The order is: what the environment asked for, then what the
//! file remembers, then the default — because `SAIL_PIXELS=720` is somebody answering a
//! question about this run, and a saved preference quietly overriding it would make
//! every measurement in this repository a lie.

use bevy::prelude::*;

use crate::camera::View;
use crate::retro::Retro;
use crate::screen::Screen;
use crate::settings::{Picture, Settings};

/// The version at the top of the file.
const FORMAT: u32 = 1;
/// How often it is written when something has changed. Somebody dragging the volume
/// slider changes it sixty times a second; the disk does not need to hear about that.
const WRITE_EVERY: f32 = 2.0;

/// What is remembered, as it sits in the file.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Kept {
    pub volume: f32,
    pub music: f32,
    pub sea: f32,
    pub boat: f32,
    pub crew: f32,
    /// Where you are sitting: at the helm, or astern of her.
    pub cockpit: bool,
    /// The drawn sea rather than the lit one.
    pub ink: bool,
    /// Which drawn width, or none for the sharp picture.
    pub pixels: Option<usize>,
    /// The frame counter in the corner.
    pub counter: bool,
    /// Full screen.
    pub full: bool,
    /// The diagnostic panel.
    pub hud: bool,
}

impl Default for Kept {
    fn default() -> Self {
        let settings = Settings::default();
        Self {
            volume: settings.volume,
            music: settings.music,
            sea: settings.sea,
            boat: settings.boat,
            crew: settings.crew,
            cockpit: false,
            ink: false,
            pixels: Retro::default_step(),
            counter: false,
            full: false,
            hud: false,
        }
    }
}

impl Kept {
    /// What is on disk, if anything legible is.
    pub fn read() -> Option<Self> {
        let text = crate::kept::read("settings")?;
        if !crate::kept::stamped(&text, "settings", FORMAT) {
            return None;
        }
        let mut kept = Self::default();
        let yes = |v: &str| matches!(v.trim(), "1" | "yes" | "on" | "true");
        for (key, value) in crate::kept::lines(&text) {
            match key {
                "volume" => kept.volume = value.trim().parse().unwrap_or(kept.volume),
                "music" => kept.music = value.trim().parse().unwrap_or(kept.music),
                "sea" => kept.sea = value.trim().parse().unwrap_or(kept.sea),
                "boat" => kept.boat = value.trim().parse().unwrap_or(kept.boat),
                // A file written before the watch had a fader simply has no line for
                // it, and the default stands — which is why this did not want a new
                // format number.
                "crew" => kept.crew = value.trim().parse().unwrap_or(kept.crew),
                "cockpit" => kept.cockpit = yes(value),
                "ink" => kept.ink = yes(value),
                "counter" => kept.counter = yes(value),
                "full" => kept.full = yes(value),
                "hud" => kept.hud = yes(value),
                // "sharp" is a real answer and not a missing one, so it is written and
                // read as a word rather than as an absent line.
                "pixels" => {
                    kept.pixels = match value.trim() {
                        "sharp" => None,
                        n => n.parse().ok().or(kept.pixels),
                    }
                }
                _ => {}
            }
        }
        Some(kept)
    }

    pub fn text(&self) -> String {
        let flag = |on: bool| if on { "1" } else { "0" };
        format!(
            "sail settings {FORMAT}\nvolume {:.3}\nmusic {:.3}\nsea {:.3}\nboat {:.3}\n\
             crew {:.3}\ncockpit {}\nink {}\npixels {}\ncounter {}\nfull {}\nhud {}\n",
            self.volume,
            self.music,
            self.sea,
            self.boat,
            self.crew,
            flag(self.cockpit),
            flag(self.ink),
            match self.pixels {
                Some(step) => step.to_string(),
                None => "sharp".into(),
            },
            flag(self.counter),
            flag(self.full),
            flag(self.hud),
        )
    }

    pub fn write(&self) -> Option<()> {
        crate::kept::write("settings", &self.text())
    }
}

/// Read the file onto the resources, once, on the first frame — and only where the
/// environment has not already answered.
///
/// On the first frame rather than while the resources are being built, because several
/// of them are built from the weather and the seed in `main` before any of this exists.
/// One frame of the default volume is inaudible; one frame of the wrong window is not
/// even a frame, because the window is not drawn until after this has run.
pub fn apply(
    kept: Res<Kept>,
    mut settings: ResMut<Settings>,
    mut picture: Picture,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    *done = true;
    let asked = |name: &str| crate::knob::var(name).is_ok();
    settings.volume = kept.volume;
    settings.music = kept.music;
    settings.sea = kept.sea;
    settings.boat = kept.boat;
    *picture.view = if kept.cockpit {
        View::Cockpit
    } else {
        View::Chase
    };
    if !asked("SAIL_INK") {
        picture.ink.0 = kept.ink;
    }
    if !asked("SAIL_PIXELS") {
        picture.retro.step = kept.pixels;
    }
    if !asked("SAIL_FPS") {
        picture.counter.0 = kept.counter;
    }
    if !asked("SAIL_FULL") {
        *picture.screen = if kept.full {
            Screen::Full
        } else {
            Screen::Windowed
        };
    }
    picture.hud.0 = kept.hud;
}

/// Which fields the environment answered for this run.
///
/// **A dial must not leak into the file.** `SAIL_FPS=1` is somebody turning the counter
/// on for one run to look at a number; writing that back would turn it on for every
/// evening afterwards, and the player never asked for that. So a field the environment
/// answered keeps whatever the file already said about it — the dial wins over the file
/// *while the program is running* and the file is left exactly as it was found.
fn dialled() -> [bool; 4] {
    [
        crate::knob::var("SAIL_INK").is_ok(),
        crate::knob::var("SAIL_PIXELS").is_ok(),
        crate::knob::var("SAIL_FPS").is_ok(),
        crate::knob::var("SAIL_FULL").is_ok(),
    ]
}

/// And write it back when it changes, which is a few times an evening.
///
/// **Not while a reel is running**, and that is the same rule as `dialled` one paragraph
/// up: a dial must not leak into the file, and a *camera* is a dial with a timetable.
/// The demo reel hides the console, turns the HUD off and sets its own mix — the sea at
/// a twentieth, the gear at full, the music off — because that is the mix a take wants.
/// All four were being written back, so running the reel once left whoever sailed next
/// with no music and a silent sea, in a file they never touched. Found by somebody
/// saying they could not hear the crew.
pub fn remember(
    mut kept: ResMut<Kept>,
    settings: Res<Settings>,
    picture: Picture,
    time: Res<Time>,
    reel: Res<crate::reel::Reel>,
    mut since: Local<f32>,
) {
    if reel.on {
        return;
    }
    let now = as_set(&settings, &picture, &kept);
    if now == *kept {
        return;
    }
    *since += time.delta_secs();
    if *since < WRITE_EVERY {
        return;
    }
    *since = 0.0;
    if now.write().is_some() {
        *kept = now;
    }
}

/// What the resources say, with the dialled fields left as the file has them.
fn as_set(settings: &Settings, picture: &Picture, kept: &Kept) -> Kept {
    let [ink_asked, pixels_asked, counter_asked, full_asked] = dialled();
    Kept {
        volume: settings.volume,
        music: settings.music,
        sea: settings.sea,
        boat: settings.boat,
        crew: settings.crew,
        cockpit: *picture.view == View::Cockpit,
        ink: if ink_asked { kept.ink } else { picture.ink.0 },
        pixels: if pixels_asked {
            kept.pixels
        } else {
            picture.retro.step
        },
        counter: if counter_asked {
            kept.counter
        } else {
            picture.counter.0
        },
        full: if full_asked {
            kept.full
        } else {
            *picture.screen == Screen::Full
        },
        hud: picture.hud.0,
    }
}

/// On the way out as well, for the change made in the last two seconds.
pub fn remember_on_the_way_out(
    mut quitting: MessageReader<AppExit>,
    kept: Res<Kept>,
    settings: Res<Settings>,
    picture: Picture,
) {
    if quitting.is_empty() {
        return;
    }
    quitting.clear();
    as_set(&settings, &picture, &kept).write();
}

pub fn plugin(app: &mut App) {
    use crate::stages::Deck;
    app.insert_resource(Kept::read().unwrap_or_default());
    app.add_systems(Update, (apply, remember).chain().in_set(Deck::Weather));
    app.add_systems(Last, remember_on_the_way_out);
}

#[cfg(test)]
mod what_is_remembered {
    use super::*;

    fn a_file() -> Kept {
        Kept {
            volume: 0.42,
            music: 0.0,
            sea: 0.9,
            boat: 0.75,
            crew: 0.33,
            cockpit: true,
            ink: true,
            pixels: Some(0),
            counter: true,
            full: true,
            hud: false,
        }
    }

    /// What goes out comes back, which is the whole job.
    #[test]
    fn it_reads_back_what_it_wrote() {
        let text = a_file().text();
        assert!(crate::kept::stamped(&text, "settings", FORMAT));
        let back = {
            // `read` goes to disk; the parsing is the part worth testing, so it is
            // exercised through the same lines a file would hand it.
            let mut kept = Kept::default();
            let yes = |v: &str| matches!(v.trim(), "1" | "yes" | "on" | "true");
            for (key, value) in crate::kept::lines(&text) {
                match key {
                    "volume" => kept.volume = value.trim().parse().unwrap(),
                    "music" => kept.music = value.trim().parse().unwrap(),
                    "sea" => kept.sea = value.trim().parse().unwrap(),
                    "boat" => kept.boat = value.trim().parse().unwrap(),
                    "crew" => kept.crew = value.trim().parse().unwrap(),
                    "cockpit" => kept.cockpit = yes(value),
                    "ink" => kept.ink = yes(value),
                    "counter" => kept.counter = yes(value),
                    "full" => kept.full = yes(value),
                    "hud" => kept.hud = yes(value),
                    "pixels" => {
                        kept.pixels = match value.trim() {
                            "sharp" => None,
                            n => n.parse().ok(),
                        }
                    }
                    _ => {}
                }
            }
            kept
        };
        assert_eq!(back, a_file());
    }

    /// **Sharp is an answer, not a missing line.** Written as a word so that a file with
    /// the sharp picture chosen does not read as a file that forgot to say.
    #[test]
    fn the_sharp_picture_survives_the_file() {
        let kept = Kept {
            pixels: None,
            ..a_file()
        };
        assert!(
            kept.text().contains("pixels sharp"),
            "the sharp picture was written as {:?}",
            kept.text()
        );
    }

    /// And the stamp is the name of this file, so that the platform's save cannot be
    /// read as a set of preferences by a build that confuses the two.
    #[test]
    fn it_is_stamped_as_settings() {
        assert!(!crate::kept::stamped(&a_file().text(), "home", FORMAT));
    }
}
