//! How big the picture is, and who decides.
//!
//! **The window used to be nailed shut.** Fourteen hundred and forty by eight hundred and
//! eighty, not resizable, with the zoom button turned off — and the reasoning was sound as
//! far as it went: the instruments are not a responsive layout. The console, the chart and
//! the gauges are drawn at known sizes into a picture that is then scaled, so a window at
//! some other size was a promise the drawing could not keep.
//!
//! What has changed is that the drawing *can* keep it. The world is rendered at a fixed
//! drawn width and scaled to whatever the window is, which has always been true; and the
//! console now shrinks until the strip fits, which is what [`crate::retro::console_scale`]
//! does. Between them there is no longer a size the picture cannot handle — only sizes it
//! handles by making the instruments smaller, which is the honest answer and the one a
//! player can see for themselves.
//!
//! So the window opens, resizes, and goes full screen. The floor is the one thing left
//! fixed: below [`NARROWEST`] by [`SHORTEST`] the console is squeezed past legibility, and
//! a window that can be dragged into uselessness is a bug report nobody can diagnose.
//!
//! `SAIL_FULL=1` opens full screen — and on this one the desk and the tab really do
//! differ: a browser will not go full screen because a program asked, only because a
//! person pressed something, so in a tab the dial is ignored and the key is the browser's
//! own.

use bevy::prelude::*;
use bevy::window::{MonitorSelection, PrimaryWindow, WindowMode};

/// The smallest window the console can be read in, in logical pixels.
///
/// The strip wants fourteen hundred and fifty points and will shrink to half of that
/// before it gives up, so twelve hundred is comfortable and nine hundred is the floor.
/// Picked against the one screen that matters for this: a Steam Deck is 1280 by 800.
pub const NARROWEST: f32 = 900.0;
/// And the shortest. The console sits across the bottom and the chart in a corner; under
/// this they meet in the middle.
pub const SHORTEST: f32 = 560.0;

/// Where the picture is drawn: in a window, or over the whole screen.
///
/// Borderless rather than exclusive full screen. Exclusive takes the display mode over,
/// which on a modern desktop buys a few milliseconds of latency and costs every other
/// thing on the machine — and this is a game somebody plays with a chart open beside it.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Screen {
    #[default]
    Windowed,
    Full,
}

impl Screen {
    /// What the world outside asked for, if it asked.
    pub fn asked() -> Self {
        match crate::knob::var("SAIL_FULL").as_deref() {
            Ok("0") | Ok("off") | Ok("no") => Self::Windowed,
            Ok(_) => Self::Full,
            Err(_) => Self::Windowed,
        }
    }

    /// The window mode this is, for the engine.
    pub fn mode(self) -> WindowMode {
        match self {
            Self::Windowed => WindowMode::Windowed,
            // The monitor the window is already on, not the primary one: a laptop with a
            // screen beside it is the normal case, and full screen on the wrong display
            // is worse than no full screen at all.
            Self::Full => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
        }
    }

    /// Reading the engine's answer back, so the two cannot drift apart.
    pub fn of(mode: &WindowMode) -> Self {
        match mode {
            WindowMode::Windowed => Self::Windowed,
            _ => Self::Full,
        }
    }

    pub fn toggled(self) -> Self {
        match self {
            Self::Windowed => Self::Full,
            Self::Full => Self::Windowed,
        }
    }
}

/// `F11`, which is what every other program on the machine uses.
///
/// Not a letter: the key list on this boat is things you do *to the boat* — trim, steer,
/// navigate, look — and how big the picture is, is not one of them. It is a window
/// manager's key and it keeps a window manager's spelling.
pub fn the_key(
    keys: Res<ButtonInput<KeyCode>>,
    mut screen: ResMut<Screen>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let alt_enter = keys.just_pressed(KeyCode::Enter)
        && (keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight));
    if !keys.just_pressed(KeyCode::F11) && !alt_enter {
        return;
    }
    *screen = screen.toggled();
    if let Ok(mut window) = windows.single_mut() {
        window.mode = screen.mode();
        info!("picture: {}", screen.name());
    }
}

/// Follow the resource, so the settings panel and the key are the same switch.
///
/// Reads the window back rather than writing it blind: the mode can also change from
/// outside — a window manager's own full-screen gesture, the green button on a Mac — and
/// a panel that then shows the wrong state is a panel nobody trusts.
pub fn follow(mut screen: ResMut<Screen>, mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    let is = Screen::of(&window.mode);
    if is != *screen {
        // The resource is the intention; if it changed, the window follows it. If the
        // *window* changed under us, the resource follows instead — whichever moved last
        // wins, and both end up saying the same thing.
        if screen.is_changed() {
            window.mode = screen.mode();
        } else {
            *screen = is;
        }
    }
}

impl Screen {
    /// For the panel and the log.
    pub fn name(self) -> &'static str {
        match self {
            Self::Windowed => "in a window",
            Self::Full => "full screen",
        }
    }
}

#[cfg(test)]
mod what_the_window_is_told {
    use super::*;

    /// The resource and the engine's own enum say the same thing in both directions.
    #[test]
    fn the_mode_and_the_resource_agree() {
        for screen in [Screen::Windowed, Screen::Full] {
            assert_eq!(Screen::of(&screen.mode()), screen);
        }
    }

    #[test]
    fn the_key_toggles_both_ways() {
        assert_eq!(Screen::Windowed.toggled(), Screen::Full);
        assert_eq!(Screen::Full.toggled(), Screen::Windowed);
    }

    /// **The floor has to leave a readable console**, which is the whole reason it is
    /// there: at the narrowest window the game will allow, the instruments must still be
    /// drawn above the size at which the figures stop being figures.
    #[test]
    fn the_smallest_window_can_still_be_read() {
        let scale = crate::retro::console_scale(NARROWEST, 1.0);
        assert!(
            scale >= 0.5,
            "the console comes out at {scale:.2} in the smallest window we allow"
        );
        // And the one screen this number was picked against.
        let deck = crate::retro::console_scale(1280.0, 1.0);
        assert!(
            deck >= 0.5,
            "a Steam Deck gets the console at {deck:.2}, which is below legible"
        );
    }
}
