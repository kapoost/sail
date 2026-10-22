//! Calls from the deck, placed in the music rather than on top of it.
//!
//! **A voice that arrives wherever the frame fell is a voice fighting the band.** The
//! game already has a tune with a tempo and a metre in it, and the boat has a rhythm of
//! her own in the swell; a shout dropped between the two sounds like a notification,
//! which is exactly what it must not sound like. Held for a moment and let go on a
//! quaver, the same recording sounds like somebody speaking *over* the music — which is
//! what a person on a deck with a radio on actually is.
//!
//! **And the sea pushes it about.** A call is early when she is lifting and late when she
//! is falling away, which is not an effect: you speak on the rise because that is when
//! you have the breath and the footing for it, and you wait out the drop. Eight
//! hundredths of a beat either way — about forty milliseconds at this music\'s tempo,
//! which is under the threshold where anybody hears a *delay* and over the one where
//! they hear a *feel*.
//!
//! One rule overrides all of it: **a call is never swallowed.** No band, no clock, or the
//! grid point does not arrive — it goes out as it is. A warning that waits for a bar line
//! is a warning that is late, and late is the one thing a warning may not be.

use bevy::prelude::*;

use crate::sound::{Call, SoundHandle};

/// How the calls are spaced against the tune.
///
/// Quavers, not beats. On the beat a voice sits exactly where the woodblock is and the
/// two fight for the same instant; on the half-beat it lands in the gap, which is where
/// a singer would be.
const GRID: f64 = 0.5;

/// How far the sea moves a call off the grid, in beats.
///
/// Small on purpose. Push it to a quarter of a beat and it stops being a feel and starts
/// being a different rhythm, and then the voice is not with the music, it is arguing
/// with it.
const SWING: f64 = 0.08;

/// How long a call may be held waiting for its moment, in beats.
const HELD_FOR: f64 = 1.5;

#[derive(Resource, Default)]
pub struct Hail {
    waiting: Option<(Call, f64)>,
    rng: u32,
}

impl Hail {
    /// Somebody says something. It goes out on the next quaver, or at once if there is
    /// no music for it to go out with.
    pub fn call(&mut self, kind: Call) {
        // The newest call wins. Two things happening at once on a boat is one person
        // shouting about the more recent of them, not two people shouting together.
        self.waiting = Some((kind, f64::NAN));
    }

    fn pick(&mut self) -> u32 {
        // Picks which of a dozen recordings to use and nothing else, so it only has to
        // avoid repeating itself.
        self.rng = self.rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.rng >> 8
    }
}

/// Where the next quaver falls, and how far the sea has moved it.
///
/// Its own function so the test exercises the arithmetic the system uses rather than a
/// second copy of it — which is how a test comes to agree with a bug.
fn lands_at(asked: f64, rise: f64) -> f64 {
    ((asked / GRID).floor() + 1.0) * GRID - rise * SWING
}

/// Hold a call until the music is ready for it.
pub fn place(
    mut hail: ResMut<Hail>,
    music: Res<crate::music::Music>,
    sim: Res<crate::sim::Sim>,
    sound: Res<SoundHandle>,
    mut was: Local<f64>,
) {
    let beat = music.beat();
    let moved = beat - *was;
    *was = beat;

    let Some((kind, asked)) = hail.waiting else {
        return;
    };
    // First sight of it: note when it was asked for, in musical time.
    let asked = if asked.is_nan() {
        hail.waiting = Some((kind, beat));
        beat
    } else {
        asked
    };

    // **No clock, no waiting.** The band is off, or paused, or the tune has not started,
    // so there is no rhythm to be in and the call is simply made.
    let tempo = music.report.tempo;
    let adrift = moved <= 0.0 || !tempo.is_finite() || tempo <= 0.0;
    // Held too long is a late warning, which is worse than an unmusical one.
    let overdue = beat - asked > HELD_FOR;

    // `heave_rate` is her vertical speed and positive is lifting. Taken against the sea
    // she is actually in rather than a constant, so the swing is the same gesture in a
    // metre of chop and in seven of swell: in both it is "early on the rise".
    let sea = sim.sim.water.significant_height().max(0.2);
    let rise = (sim.sim.state.heave_rate / sea).clamp(-1.0, 1.0);

    if adrift || overdue || beat >= lands_at(asked, rise) {
        let pick = hail.pick();
        hail.waiting = None;
        sound.0.shout(kind, pick);
    }
}

pub fn plugin(app: &mut App) {
    use crate::stages::Deck;
    app.init_resource::<Hail>();
    // After the music has moved its clock on this frame, or every call is placed against
    // last frame\'s beat.
    app.add_systems(
        Update,
        place.in_set(Deck::Dress).after(crate::music::compose),
    );
}

#[cfg(test)]
mod where_a_call_lands {
    use super::*;

    /// **It lands on a quaver, not where the frame fell.** Asked for anywhere inside a
    /// half-beat, it goes out at the end of that half-beat.
    #[test]
    fn a_call_waits_for_the_next_quaver() {
        for asked in [0.01, 0.2, 0.49] {
            assert!(
                (lands_at(asked, 0.0) - 0.5).abs() < 1e-9,
                "asked at {asked}"
            );
        }
        for asked in [0.51, 0.9] {
            assert!(
                (lands_at(asked, 0.0) - 1.0).abs() < 1e-9,
                "asked at {asked}"
            );
        }
    }

    /// Early on the rise, late on the drop, and never by enough to become a different
    /// rhythm.
    #[test]
    fn the_sea_pushes_it_about_but_only_a_little() {
        let flat = lands_at(0.2, 0.0);
        let lifting = lands_at(0.2, 1.0);
        let falling = lands_at(0.2, -1.0);
        assert!(lifting < flat, "she is rising and the call went late");
        assert!(falling > flat, "she is dropping and the call went early");
        let swung = (lifting - falling).abs();
        assert!(
            swung < 0.25,
            "{swung} of a beat is a different rhythm rather than a feel"
        );
    }
}
