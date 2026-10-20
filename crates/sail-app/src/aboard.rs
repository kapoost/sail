//! How she starts: who is on deck, and who has the helm.
//!
//! On a desk the answer has always been *you, alone, steering* — which is the right
//! answer for somebody who installed a sailing simulator, and the wrong one for somebody
//! who clicked a link. **A tab is a first minute, not a passage.** What a first minute
//! needs is a boat that is already sailing herself, so the hands are free to look: at
//! the telltales, at the chart, at the sea going past, at what the sheets do when you
//! pull them. A boat that rounds up and stops while the reader is reading the key list
//! has answered a question nobody asked.
//!
//! So in a browser she starts with **the watch on deck** and **the pilot steering**, and
//! the only thing the reader has to do is look. Both are one key away from being taken
//! back — `P` for the helm, the console for the hands — and both are knobs on a desk
//! too: `SAIL_CREW=1 SAIL_PILOT=1` is the same start on this side of the fence, which is
//! how it was tested.
//!
//! The third part of that promise is not here but in `knob::seed`: the day a tab opens
//! on is chosen rather than drawn, for sea room. See the note there.

use bevy::prelude::*;

use crate::crew::Crew;
use crate::input::Pilot;
use crate::sim::Sim;

/// Who is on deck when she starts, and who has the helm.
///
/// What this module does in a frame, in one place — see `stages.rs` for why the
/// schedule is named sets rather than one long chain.
pub fn plugin(app: &mut App) {
    use crate::stages::Build;
    app.add_systems(Startup, how_she_starts.in_set(Build::Scene));
}

/// Whether a dial is on, with a different default in a tab.
///
/// `SAIL_CREW=0` turns the watch off in a browser; `SAIL_CREW=1` turns it on at a desk.
/// Neither is clever — it is the same question with a different answer either side.
fn asked(name: &str) -> bool {
    match crate::knob::var(name).as_deref() {
        Ok("0") | Ok("off") | Ok("no") => false,
        Ok(_) => true,
        Err(_) => cfg!(target_arch = "wasm32"),
    }
}

/// Put the watch on deck and give the pilot the helm, if that is how she is starting.
pub fn how_she_starts(mut crew: ResMut<Crew>, mut pilot: ResMut<Pilot>, sim: Res<Sim>) {
    if asked("SAIL_CREW") {
        crew.standing = true;
        info!("watch on deck: the hands have the sheets");
    }
    if asked("SAIL_PILOT") {
        // The course she is on, not a course chosen for her: the heading comes out of
        // the day's own draw, and the sea room in front of it is what `knob::seed` picks
        // the day for. Heading rather than wind angle, because a wind that veers would
        // otherwise walk her round towards the land this start exists to avoid.
        pilot.hold_heading(&sim.sim.state);
        info!(
            "pilot engaged on {:03.0}",
            sail_core::units::deg(sim.sim.state.heading)
        );
    }
}
