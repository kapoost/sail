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
//! hundredths of a beat either way — about forty milliseconds at this music's tempo,
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

/// The least time between two calls, seconds.
///
/// **A watch that announces everything is a watch nobody listens to.** Trimming a sheet
/// is not an event — it is what the hands are doing continuously, all passage — and a
/// voice for each one would be chatter that trains the player to stop hearing the voice
/// at all, which costs the warnings their meaning. So only decisions speak: a reef in or
/// out, a sail changed. Even those cluster, because a gust does several things at once,
/// and twelve seconds is long enough that two calls are two events rather than one event
/// described twice.
///
/// **A warning is never held back by this.** The whole reason to keep the deck quiet is
/// so that the one call that matters arrives into quiet; a cooldown that swallowed it
/// would have spent the quiet and then thrown away the thing it was saved for.
const QUIET_FOR: f32 = 12.0;

#[derive(Resource, Default)]
pub struct Hail {
    waiting: Option<(Call, f64)>,
    rng: u32,
    /// Seconds since anybody last spoke.
    since: f32,
}

impl Hail {
    /// Somebody says something. It goes out on the next quaver, or at once if there is
    /// no music for it to go out with.
    pub fn call(&mut self, kind: Call) {
        // Quiet enough to be worth saying — or urgent enough that it does not matter.
        // See `QUIET_FOR`.
        if self.since < QUIET_FOR && kind != Call::Warn {
            return;
        }
        // The newest call wins. Two things happening at once on a boat is one person
        // shouting about the more recent of them, not two people shouting together.
        self.waiting = Some((kind, f64::NAN));
    }

    /// Say it whatever the deck has been doing, for the rehearsal dial.
    ///
    /// The quiet between calls is the whole point of `call`, and auditioning recordings
    /// is the one job where it is in the way: a harness that fires every three seconds
    /// would be answered by nothing but warnings, which are the only kind allowed
    /// through. Its own method rather than a flag on `call`, so nothing in the game can
    /// reach for it by accident.
    pub fn anyway(&mut self, kind: Call) {
        self.since = QUIET_FOR;
        self.call(kind);
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
    time: Res<Time>,
    mut was: Local<f64>,
) {
    hail.since += time.delta_secs();
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
        hail.since = 0.0;
        sound.0.shout(kind, pick);
    }
}

pub fn plugin(app: &mut App) {
    use crate::stages::Deck;
    app.init_resource::<Hail>();
    // **In the music's own set, not the sound's.** This has to run after the beat has
    // moved on this frame or every call is placed against last frame's, and the composer
    // lives in `Deck::Weather` while the sound lives in `Deck::Dress` — which runs
    // *first*. Asking for `.after(compose)` from `Dress` is therefore a cycle, and the
    // schedule refuses to build at all rather than quietly picking an order.
    //
    // Worth knowing how that was found: not by a test. Nothing here builds the real
    // schedule — doing so wants a window — so this whole class of fault is caught by
    // running the program and in no other way.
    app.add_systems(
        Update,
        place.in_set(Deck::Weather).after(crate::music::compose),
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

    /// **A watch that announces everything is a watch nobody listens to**, and the
    /// price of that is paid by the warnings: a player who has learned to tune the deck
    /// out has tuned out the one call that mattered. So the quiet between calls is kept
    /// — and a warning is never the thing that is kept out by it.
    #[test]
    fn chatter_is_held_back_and_a_warning_never_is() {
        let mut hail = Hail {
            since: QUIET_FOR + 1.0,
            ..Default::default()
        };

        hail.call(Call::Report);
        assert!(
            hail.waiting.is_some(),
            "the deck was quiet and nobody spoke"
        );

        // Said, so the clock starts; the system does this when it actually goes out.
        hail.waiting = None;
        hail.since = 0.0;

        hail.call(Call::Report);
        assert!(
            hail.waiting.is_none(),
            "a second report straight after the first"
        );
        hail.call(Call::Aye);
        assert!(hail.waiting.is_none(), "and an aye is not more important");

        // The one that gets through regardless.
        hail.call(Call::Warn);
        assert_eq!(
            hail.waiting.map(|(k, _)| k),
            Some(Call::Warn),
            "a warning was held back by the quiet that exists to make room for it"
        );
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
