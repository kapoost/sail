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

/// The least time between two calls, seconds — **under way**, and **while working**.
///
/// Reported from the deck as the shape it should have: talk every few seconds to a dozen
/// or so, sometimes longer; a lot of it while manoeuvring and very little on a leg. That
/// is not a cooldown, it is two cooldowns and a way of telling which one you are in.
///
/// **A watch that announces everything is a watch nobody listens to**, and the price of
/// that is paid by the warnings: a player who has learned to tune the deck out has tuned
/// out the one call that mattered. So only decisions speak, and a warning is never the
/// thing any of this holds back.
///
/// A deck under way is quiet for minutes at a time and the quiet is the point — it is
/// what makes the next call mean something. A deck *working* is nearly continuous talk,
/// because everybody has a job and the jobs have to be said out loud: that is what a
/// manoeuvre sounds like from the cockpit and it is the only time a boat is noisy.
const QUIET_UNDER_WAY: f32 = 40.0;
const QUIET_WORKING: f32 = 3.5;

/// And how long the deck stays "working" after the last thing anybody was doing.
///
/// Six seconds, so a manoeuvre is one noisy stretch rather than a burst for each step of
/// it. A tack is a dozen separate jobs and it should sound like one event.
const STILL_AT_IT: f32 = 6.0;

#[derive(Resource, Default)]
pub struct Hail {
    waiting: Option<(Call, f64)>,
    rng: u32,
    /// Seconds since anybody last spoke.
    since: f32,
    /// Seconds of "there is a job on" left. See [`STILL_AT_IT`].
    working: f32,
}

impl Hail {
    /// Somebody says something. It goes out on the next quaver, or at once if there is
    /// no music for it to go out with.
    /// Returns whether it was taken, which the rehearsal dial needs and nothing else
    /// does — a refusal is not a thing the game has to handle, it is the rule working.
    pub fn call(&mut self, kind: Call) -> bool {
        // Quiet enough to be worth saying — or urgent enough that it does not matter.
        if self.since < self.quiet() && kind != Call::Warn {
            return false;
        }
        // **The newest call wins, unless it matters less than the one already waiting.**
        //
        // Two things at once on a boat is one person shouting about the more recent of
        // them — right up to the moment the earlier one was the one that mattered. And
        // the effort call fires *every frame* while a job is on, so without a rank it
        // walks over everything: a reef is decided while the hands are at the pennant,
        // which means the report of it is queued during a manoeuvre and stomped by the
        // next frame's grunt. Found by reading; it would have sounded like the watch
        // grunting at every reef and never saying what they had done.
        if self
            .waiting
            .is_some_and(|(held, _)| matters(held) > matters(kind))
        {
            return false;
        }
        self.waiting = Some((kind, f64::NAN));
        true
    }

    /// Say it whatever the deck has been doing, for the rehearsal dial.
    ///
    /// The quiet between calls is the whole point of `call`, and auditioning recordings
    /// is the one job where it is in the way: a harness that fires every three seconds
    /// would be answered by nothing but warnings, which are the only kind allowed
    /// through. Its own method rather than a flag on `call`, so nothing in the game can
    /// reach for it by accident.
    pub fn anyway(&mut self, kind: Call) {
        let was = self.since;
        self.since = f32::MAX;
        // Put the clock back if the call was refused anyway — it still can be, by the
        // rank — because a refusal that left it at infinity would switch the quiet off
        // for the rest of the session. A dial for listening to recordings has no
        // business doing that to the game.
        if !self.call(kind) {
            self.since = was;
        }
    }

    /// There is a job on deck. Called every frame while there is one; it decays, so a
    /// manoeuvre is one noisy stretch rather than a burst per step of it.
    pub fn working(&mut self) {
        self.working = STILL_AT_IT;
    }

    /// How long the deck stays quiet before anybody speaks again.
    ///
    /// Not a constant: a boat working and a boat on a leg are two different places to be
    /// standing. And not a fixed number either — the jitter is what stops it becoming a
    /// metronome, which is what "sometimes longer" means when a sailor says it.
    fn quiet(&self) -> f32 {
        let base = if self.working > 0.0 {
            QUIET_WORKING
        } else {
            QUIET_UNDER_WAY
        };
        // The same xorshift state, read without advancing it: this has to be stable
        // within a frame or the answer changes between two calls in the same breath.
        let wobble = 0.7 + 0.6 * ((self.rng >> 16) & 0xff) as f32 / 255.0;
        base * wobble
    }

    fn pick(&mut self) -> u32 {
        // Picks which of a dozen recordings to use and nothing else, so it only has to
        // avoid repeating itself.
        self.rng = self.rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.rng >> 8
    }
}

/// How much a call matters, for deciding which of two survives.
///
/// A warning over everything; a report of a thing done over an acknowledgement of a
/// thing started; a wordless grunt last, because it is the sound of work rather than
/// anything anybody needed to know.
fn matters(kind: Call) -> u8 {
    match kind {
        Call::Warn => 3,
        Call::Report => 2,
        Call::Aye => 1,
        Call::Effort => 0,
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
    let dt = time.delta_secs();
    hail.since += dt;
    hail.working = (hail.working - dt).max(0.0);
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
    //
    // **And after the watch**, which is in this same set and queues calls. Within a set
    // Bevy is free to order two systems however it likes, so without this whether a call
    // goes out on the frame it was made or waits for the next one is unspecified — and a
    // call that waits a frame is a call the next frame's grunt can stand on.
    app.add_systems(
        Update,
        place
            .in_set(Deck::Weather)
            .after(crate::music::compose)
            .after(crate::crew::work),
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
            since: f32::MAX,
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
        // And once it is queued, nothing walks over it while it waits for its quaver —
        // which matters because the effort call fires every frame a job is on.
        hail.call(Call::Effort);
        hail.call(Call::Report);
        assert_eq!(
            hail.waiting.map(|(k, _)| k),
            Some(Call::Warn),
            "a warning was held back by the quiet that exists to make room for it"
        );
    }

    /// **A grunt may not stand on a report.** The effort call fires every frame while a
    /// job is on, and a reef is decided *during* a job — so the report of it is queued
    /// in the middle of a manoeuvre and, without a rank, replaced by the next frame's
    /// wordless noise. It would have sounded like a watch that grunts at every reef and
    /// never says what it has done.
    #[test]
    fn a_queued_call_is_not_replaced_by_a_lesser_one() {
        let quiet = || Hail {
            since: f32::MAX,
            ..Default::default()
        };

        let mut hail = quiet();
        hail.call(Call::Report);
        hail.call(Call::Effort);
        assert_eq!(
            hail.waiting.map(|(k, _)| k),
            Some(Call::Report),
            "a grunt replaced the report of a reef going in"
        );

        // The other way round it is the newest that wins, because the newer thing is
        // the more important one as well.
        let mut hail = quiet();
        hail.call(Call::Effort);
        hail.call(Call::Report);
        assert_eq!(hail.waiting.map(|(k, _)| k), Some(Call::Report));

        // And a warning takes anything.
        for over in [Call::Effort, Call::Aye, Call::Report] {
            let mut hail = quiet();
            hail.call(over);
            hail.call(Call::Warn);
            assert_eq!(
                hail.waiting.map(|(k, _)| k),
                Some(Call::Warn),
                "over {over:?}"
            );
        }
    }

    /// The dial for listening to recordings must not leave the game's quiet switched
    /// off behind it.
    #[test]
    fn rehearsing_does_not_silence_the_rule_it_steps_over() {
        let mut hail = Hail::default();
        hail.call(Call::Warn);
        let before = hail.since;
        // Refused, because a warning is already waiting and a report matters less.
        hail.anyway(Call::Report);
        assert_eq!(hail.waiting.map(|(k, _)| k), Some(Call::Warn));
        assert_eq!(
            hail.since, before,
            "the quiet was left switched off for the rest of the session"
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
