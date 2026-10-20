//! The order of a frame, stated once.
//!
//! **This file exists because a schedule written as one long chain stops being readable
//! at about forty systems and stops *compiling* at twenty.** Bevy's tuples end there, and
//! the game had eighty-eight systems in a single chained tuple held together by nesting:
//! three levels of parentheses, two of them with comments apologising for being there,
//! and every new mechanic arriving as a game of finding a group with room in it. Two of
//! them this week ended up bolted on in their own `add_systems` call purely because the
//! tuple was full, which is a schedule decided by a type limit rather than by the frame.
//!
//! So the order is named instead. A handful of sets, chained once, and every module says
//! which set its systems belong in. Adding a mechanic is then a line in that module and
//! nothing at all here — and, more to the point, the order of the frame can be read in
//! one screen by somebody who has never opened this program.
//!
//! **What was kept exactly.** The sets below are the stages the old chain already had;
//! each system is in the stage it was in. Inside a stage the order is unspecified, which
//! was also true before — a nested tuple without its own `.chain()` is a bag, and most of
//! that chain was bags. Where two systems really do depend on each other the module
//! chains them itself, which is where that fact belongs.

use bevy::prelude::*;

/// The stages of one frame, in the order they happen.
///
/// Reading this list is reading the game: intent, then the physics it drives, then where
/// the world is drawn from, then everything that places a thing in it, then the sky and
/// the weather over the lot.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Deck {
    /// What the hands asked for: the helm, the clutches, the rod. Before the physics,
    /// because intent belongs at the top of a frame.
    Intent,
    /// The boat herself, integrated. One system, and the only one in its stage.
    Step,
    /// Where the world is drawn from, picked before anything is placed in it so that the
    /// whole frame agrees on one mark.
    Origin,
    /// What the frame says about itself: the vitals, the frame times, the track.
    Report,
    /// Things anchored in the sea rather than to her, re-placed against the new origin.
    Follow,
    /// Everybody else's boats, which arrive over a wire and have to be told where they
    /// are before they can be drawn.
    Fleet,
    /// The boat and the water dressed: hull, rudder, sails, wake, spray, the sea itself.
    /// The bulk of the frame, and a bag — these do not read each other.
    Dress,
    /// The sky, the weather, the instruments' own state, and everything that is neither
    /// the boat nor the water: missions, navigation, the chart, the music, the picture.
    Weather,
}

/// What is built before the first frame, in the order it has to be built in.
///
/// Coarser than the chain it replaces, on purpose. The old one was strictly ordered end
/// to end, and almost none of that order was load-bearing: what actually matters is that
/// the scene exists before the boat, the boat before the gear on her deck, and the net
/// last because it wants everything else to have a shape already.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Build {
    /// The sea, the sky, the land, the light — the world she is put into.
    Scene,
    /// The boat, her rig and her instruments.
    Boat,
    /// What is lying about on her: the gear, the rod, the basket, the hook.
    Deck,
    /// And the things that want all of the above to exist: the fleet, the marks, the
    /// lesson.
    Last,
}

/// What is painted over what.
///
/// **Layering is one decision about the picture, so it is one list.** egui stacks its
/// areas in the order they are shown, which means draw order *is* the z-order; scattering
/// it across fifteen modules would make "why is the glass under the chart" a question
/// nobody can answer by reading. Four layers are enough, and each is a statement rather
/// than a position in a tuple.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Paint {
    /// The type and the size of it. Before anything is drawn, because the first frame is
    /// drawn in whatever font is installed when it happens — and within a set the order
    /// is unspecified, so "the font goes first" has to be said rather than assumed.
    Setting,
    /// The console, the chart, the gauges: the furniture of the screen, anchored to its
    /// edges and never overlapping each other.
    Instruments,
    /// Things that sit over the picture while they are open — the settings, the key list,
    /// the lesson, the log.
    Cards,
    /// And what must be on top of both: the glass, a fix being taken, the one line a
    /// mission has to say.
    Over,
}

/// Chain all three schedules, once.
pub fn order(app: &mut App) {
    app.configure_sets(
        Startup,
        (Build::Scene, Build::Boat, Build::Deck, Build::Last).chain(),
    )
    .configure_sets(
        Update,
        (
            Deck::Intent,
            Deck::Step,
            Deck::Origin,
            Deck::Report,
            Deck::Follow,
            Deck::Fleet,
            Deck::Dress,
            Deck::Weather,
        )
            .chain(),
    )
    .configure_sets(
        bevy_egui::EguiPrimaryContextPass,
        (
            Paint::Setting,
            Paint::Instruments,
            Paint::Cards,
            Paint::Over,
        )
            .chain(),
    );
}
