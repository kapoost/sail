//! Music made out of the weather.
//!
//! # Why generated rather than played
//!
//! The first question was whether to find a cappella shanties under a free licence. The
//! songs are free — they are two and three hundred years old — but a *recording* carries
//! rights of its own whoever sang it, and the ones that are genuinely clear are a short
//! list of variable takes that would have to be checked one by one. They would also be
//! tens of megabytes in a game whose whole distribution argument is that it is one file.
//!
//! But the real reason is none of that. A recording cannot know what the wind is doing.
//! This is a game where the weather is the antagonist and the whole skill is reading it,
//! and a soundtrack that does not change when the glass falls is decoration. What is
//! here instead changes mode when you tack, changes metre with the point of sail, thins
//! to a drone when a squall is coming and stops altogether on the rocks.
//!
//! # The idiom
//!
//! Irish and Nordic folk, which is one idiom with two accents and the right one for a
//! northern archipelago. What makes it that rather than generic minor-key game music:
//!
//! * **Modes, not keys.** Dorian and Aeolian. The major scale is wrong here and
//!   obviously so; the leading note of a classical cadence is the one interval that
//!   never appears in a reel. Dorian — minor with a raised sixth — is the sound of
//!   nearly every session tune ever played, and the raised sixth against a minor third
//!   is the whole flavour.
//! * **No dominant sevenths.** Modal harmony moves i–VII–VI–VII and i–VII–IV–i. A V7
//!   resolving to i is a different century and a different continent.
//! * **The metre is the point of sail.** A halyard shanty is in four because it is sung
//!   to a pull; a jig rolls in six-eight the way a boat rolls off the wind; a Nordic
//!   polska leans in three. The boat decides which, and that is not an effect, it is
//!   the same relation that put the shanty in four in the first place.
//! * **Phrases of eight bars, AABB.** The form of every reel and every shanty, and the
//!   only structure that makes a generated tune sound like a tune rather than a walk.
//!
//! # The cartridge
//!
//! In the ink drawing the voices change from triangles to pulse waves and the tune
//! quantises on to a pentatonic subset: the sound of a four-channel chip, which is what
//! "MIDI in a JRPG" means to anybody who remembers it. It is right that the picture and
//! the music change together — both are the same decision about what kind of thing this
//! is being drawn as.

use bevy::prelude::*;

use crate::sim::Sim;
use crate::sound::SoundHandle;

/// Semitones above the tonic, for the two modes that matter.
///
/// Aeolian is the minor everyone knows. Dorian raises the sixth, and that one note is
/// the difference between a lament and a reel — it is why a tune in D dorian lifts where
/// the same notes in D minor sag.
const AEOLIAN: [i32; 7] = [0, 2, 3, 5, 7, 8, 10];
const DORIAN: [i32; 7] = [0, 2, 3, 5, 7, 9, 10];

/// Chord roots as scale degrees, in the two progressions folk music actually uses.
///
/// i–VII–VI–VII is the one every ballad walks down; i–VII–IV–i is the one a reel turns
/// on. Neither contains a dominant, which is the point.
const WALKING: [usize; 4] = [0, 6, 5, 6];
const TURNING: [usize; 4] = [0, 6, 3, 0];

/// And the one every film score reaches for when something is about to happen.
///
/// i–VI–III–VII, which is the same four chords under half the music written for
/// pictures since the seventies, and it is not a cliché by accident: every root moves by
/// a *third* rather than by a step or a fifth, so each chord shares two notes with the
/// last and the progression slides instead of walking. Nothing resolves and nothing is
/// unresolved either — it can turn for as long as the scene needs and never arrive —
/// which is exactly the feeling of being on a boat that is going too fast to be
/// comfortable and cannot be slowed down.
const CINEMATIC: [usize; 4] = [0, 5, 2, 6];

/// How long a phrase is, in bars. Eight, because every tune in this tradition is eight.
const PHRASE: u32 = 8;

/// The four notes the whole passage is made of.
///
/// This is what was missing, and it is the difference between generated notes and a
/// tune. A random walk through a scale, however well weighted, is not music: there is
/// nothing in it to recognise, so the ear has nothing to be satisfied by when it comes
/// back. What folk music does instead — all of it, everywhere — is state a short cell
/// and then turn it over: say it again, say it a step higher, say it upside down, say it
/// slower, and finally say it once more and land. A listener who has heard bar one knows
/// bar five is *about* bar one, and that recognition is the whole pleasure.
///
/// Four notes because that is the length of nearly every one that has lasted: the
/// opening of a reel, of a hornpipe, of "Drunken Sailor". Long enough to have a shape,
/// short enough to be turned over without the join showing.
#[derive(Clone, Copy, Debug)]
struct Motif {
    /// Scale degrees relative to whatever it is being said on.
    shape: [i32; 4],
    /// Which of the eight quavers in a bar it speaks on. A rhythm is as much of an
    /// identity as the pitches — more, if anything, which is why you can recognise a
    /// tune somebody is drumming on a table.
    voiced: [bool; 8],
}

impl Motif {
    /// Make one, from the passage's own seed.
    ///
    /// Shaped rather than random: an opening leap or step, a turn, and a return. The
    /// constraint is what makes it singable — four notes drawn uniformly from a scale is
    /// four notes, not a motif.
    fn invent(rng: &mut impl FnMut() -> f32) -> Self {
        let openings: [[i32; 4]; 12] = [
            [0, 2, 1, 0],   // a turn round the tonic: the commonest opening there is
            [0, 1, 2, 4],   // a climb
            [4, 2, 1, 0],   // and the fall that answers it
            [0, 4, 2, 1],   // a leap, then filling in behind it
            [0, 2, 4, 2],   // an arch
            [2, 1, 0, -3],  // dropping to the seventh below, which is very Nordic
            [0, 0, 2, 4],   // a repeated note that then goes: a shanty's shape
            [4, 4, 2, 0],   // the same thing falling, which is how a chorus answers
            [0, -1, 0, 2],  // dipping under the tonic first — a lift that works by delay
            [2, 4, 3, 2],   // a turn higher up, round the third
            [0, 3, 2, 1],   // a fourth, the interval every hymn and half the sea songs
            [1, 0, 2, 0],   // leaning on the second and resolving down, twice
        ];
        let shape = openings[(rng() * openings.len() as f32) as usize % openings.len()];
        // Rhythms that leave the right silences. A cell that speaks on every quaver has
        // no rhythm; what gives a tune its gait is where it does *not* play.
        let rhythms: [[bool; 8]; 7] = [
            [true, false, true, false, true, false, true, false], // even, the reel's walk
            [true, false, true, true, false, true, false, false],
            [true, true, false, true, false, false, true, false],
            [true, false, false, true, true, false, true, false],
            [true, true, true, false, true, false, false, false], // three and a rest
            [true, false, true, false, false, true, true, false],
            [true, false, false, false, true, true, false, true], // a long one, then two
        ];
        let voiced = rhythms[(rng() * rhythms.len() as f32) as usize % rhythms.len()];
        Self { shape, voiced }
    }

    /// The degree this cell says at a given quaver, if it speaks at all.
    fn at(&self, quaver: usize) -> Option<i32> {
        if !self.voiced[quaver % 8] {
            return None;
        }
        // The four notes are spread across the bar in the order they are voiced, so a
        // cell with three notes in it says three and a cell with four says four — the
        // rhythm chooses how much of the shape is heard.
        let spoken = self.voiced[..=(quaver % 8)].iter().filter(|v| **v).count();
        Some(self.shape[(spoken - 1).min(3)])
    }
}

/// What a bar does, and which of the two strains it belongs to.
///
/// AABB is two *different* tunes, not one tune played twice, and that is the thing the
/// first version of this missed. Every reel, every jig and every shanty has an A part
/// and a B part: the B sits higher, usually starts where the A ended, and is what the
/// whole form is for — you get to come *back* to the A, and coming back is only worth
/// anything if you went somewhere.
///
/// Returns which strain, how far to lift it, whether to turn it over, and whether this
/// bar is a cadence — the bar that has to land rather than carry on.
fn how_to_say_it(bar: u32) -> (usize, i32, bool, bool) {
    match bar % PHRASE {
        0 => (0, 0, false, false),  // the A part
        1 => (0, 2, false, false),  // said a third higher
        2 => (0, 0, true, false),   // turned over
        3 => (0, 0, false, true),   // and landed: the half cadence
        4 => (1, 4, false, false),  // the B part, up where a B part lives
        5 => (1, 4, false, false),  // said again
        6 => (1, 2, true, false),   // turned over and coming down
        _ => (0, 0, false, true),   // home, with the A's own shape, and landed
    }
}

#[derive(Resource)]
pub struct Music {
    /// Musical time, in beats since the passage began.
    beat: f64,
    /// The step last played, so a note is placed once rather than every frame.
    last_step: i64,
    /// The tonic, in hertz, settled at the start and left alone — a tune that changes
    /// key whenever the wind shifts is not a tune.
    tonic: f32,
    rng: u32,
    /// The melody's position in the scale, for the passing notes between motif notes.
    degree: i32,
    /// The two cells the whole passage is built from, invented once when she casts off.
    motif: Option<[Motif; 2]>,
    /// What is being played now, as opposed to what the weather is asking for.
    ///
    /// The two are different on purpose. Music that changed the instant the wind did
    /// would never be in any metre long enough to be in it — a tune has to *be*
    /// somewhere before leaving is worth anything.
    playing: Option<Asking>,
    /// And where it is going, once the weather has asked for somewhere else.
    pending: Option<Asking>,
    /// Semiquavers of bridge left to play, counted down at the new tempo.
    bridge: i32,
    /// The step the bridge last struck, so each semiquaver sounds once.
    bridge_step: i64,
    /// The bridge's own clock, in semiquavers of the tempo being *arrived at*.
    bridge_clock: f64,
    /// Phrases since the last key change, so a tune sits somewhere before it leaves.
    phrases_here: u32,
}

impl Default for Music {
    fn default() -> Self {
        Self {
            beat: 0.0,
            last_step: -1,
            // D, which is the key half this music is written in and the one every whistle
            // and fiddle is happiest in.
            tonic: 146.83,
            rng: 0x5EA_5A11,
            degree: 0,
            motif: None,
            playing: None,
            pending: None,
            bridge: 0,
            bridge_step: -1,
            bridge_clock: 0.0,
            phrases_here: 0,
        }
    }
}

impl Music {
    fn next(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        self.rng as f32 / u32::MAX as f32
    }
}

/// What the weather is asking for, worked out once and then turned into notes.
#[derive(Clone, Copy, PartialEq)]
struct Asking {
    /// Beats a minute.
    tempo: f32,
    /// Beats in a bar: four for a shanty, three for a polska, six for a jig.
    beats: u32,
    /// Whether the sixth is raised.
    dorian: bool,
    /// How much of the band is playing, 0 to 1.
    fullness: f32,
    /// Overall level.
    level: f32,
    /// How far up or down the whole thing sits, in octaves.
    register: f32,
    /// The tonic, in hertz. Changed only at a phrase end and only by an interval the
    /// tradition uses — see `modulate`.
    tonic: f32,
}

impl Asking {
    /// Whether this is a different *kind* of music, as opposed to a slightly different
    /// one. A tempo that has drifted two beats a minute is not worth a bridge; a change
    /// of metre always is.
    fn worth_moving_for(&self, other: &Self) -> bool {
        self.beats != other.beats
            || self.dorian != other.dorian
            || (self.tempo - other.tempo).abs() > 7.0
            || (self.tonic - other.tonic).abs() > 1.0
    }
}

/// Where a tune may go when it goes somewhere.
///
/// Not an arbitrary transposition. Every tradition that has lasted moves by the same
/// handful of intervals, and the reason is that they share notes with where you were:
/// the ear hears the new key as a *place in the old one* before it hears it as new. Up a
/// fifth is the oldest move in music and takes six of the seven notes with it; down a
/// fifth is the same journey the other way. The relative major — up a minor third from a
/// minor tonic — shares *all* of them, which is why it is the one that sounds like the
/// sun coming out rather than like a modulation at all. And down a whole tone is the
/// Nordic one, the step every slått and every polska takes when it wants to sound older.
///
/// Which of them is chosen is the weather's: more wind lifts, less drops, and a long
/// settled passage goes to the relative major because nothing is happening and that is a
/// pleasant place for nothing to happen in.
fn modulate(from: f32, rising: bool, settled: bool) -> f32 {
    let semitones = if settled {
        3.0 // the relative major
    } else if rising {
        7.0 // up a fifth
    } else {
        -2.0 // down a tone, which is the northern move
    };
    let moved = from * 2.0_f32.powf(semitones / 12.0);
    // Kept inside an octave of where this boat's music lives, by dropping or lifting a
    // whole octave rather than refusing the move — the interval is the music, the
    // register is only comfort.
    if moved > 220.0 {
        moved * 0.5
    } else if moved < 110.0 {
        moved * 2.0
    } else {
        moved
    }
}

/// How hard a sail is working, 0 to 1 — and therefore how much of the band it is.
///
/// Every sail gets a voice and the voice is only there while the sail is. That is the
/// thing a soundtrack can do that a picture cannot: you *hear* the sail plan. Hand the
/// main and the bass goes with it; roll the genoa away and the tune thins to a line;
/// set the staysail and a second voice comes in under the melody. A sail that is luffing
/// stops playing, which is exactly what a luffing sail has stopped doing.
///
/// Measured as the force the cloth is actually making against the force it *could* make
/// in this wind — the resultant coefficient, in other words — so it is about how hard
/// she is pressed rather than how much wind there is. A sail at its best trim in six
/// knots is working as hard as the same sail at its best trim in twenty, and the music
/// says so; what the wind changes is the tempo, which is a different thing and already
/// handled.
fn worked(sail: &sail_core::sail::SailForces) -> f32 {
    let c = sail.coeffs;
    let resultant = (c.cl * c.cl + c.cd * c.cd).sqrt();
    let alive = (1.0 - c.luffing) * (1.0 - 0.5 * c.stalled);
    ((resultant / 1.5) * alive).clamp(0.0, 1.0) as f32
}

/// How often the hull meets a wave, in seconds.
///
/// The wave period as the *boat* experiences it, which is not the period of the sea:
/// going with them she meets them slowly and punching into them she meets them fast,
/// and the difference is large — a four-second sea met at seven knots head-on arrives
/// every two and a half, and the same sea from astern every eight or more.
///
/// `Te = T / |1 - (V/c) cos mu|`, where c is the wave's own speed `gT/2pi` and mu is the
/// angle between where she is going and where they are going. The denominator goes to
/// nothing when she is running at exactly wave speed — which is surfing, and which the
/// clamp on the result handles by leaving the tempo to the wind.
fn encounter_period(sim: &Sim) -> f64 {
    let t = &sim.sim.telemetry;
    let period = sim.sim.water.peak_period_or(3.0);
    if period < 0.5 {
        return 0.0;
    }
    let wave_speed = 9.81 * period / std::f64::consts::TAU;
    // The sea runs with the wind, so the angle between her course and the waves is her
    // true wind angle — which is the one number a sailor already has in front of them.
    let mu = std::f64::consts::PI - t.twa.abs();
    let relative = 1.0 - (t.stw / wave_speed) * mu.cos();
    if relative.abs() < 0.08 {
        return 0.0;
    }
    period / relative.abs()
}

/// Read the boat and the sky, and decide what kind of music this is.
///
/// Every line of this is a relation somebody could argue with on the water, which is the
/// only way a soundtrack earns the right to change.
fn asking(sim: &Sim, hour: f32, tonic: f32) -> Asking {
    let t = &sim.sim.telemetry;
    let wind = sail_core::units::ms_to_kn(t.tws) as f32;
    let force = crate::weather::force(wind as f64) as f32;
    let twa = sail_core::units::deg(t.twa).abs() as f32;

    // The rhythm section is the hull meeting the sea, so the tempo is chosen to lock to
    // it rather than to run across it.
    //
    // This is the best idea anybody has had about this soundtrack and it belongs to the
    // sailor. A boat in a seaway is already making a rhythm — the slap under the bow,
    // the shoulder going in, the quarter wave — and it is a *steady* rhythm with a period
    // you can calculate: the encounter period, which is the wave period stretched or
    // squeezed by how fast she is going through them and at what angle. Music laid over
    // that at an unrelated tempo is two rhythms in the same room. Music whose beat
    // divides it is one.
    //
    // So the wind proposes a tempo and the sea rounds it: pick the whole number of beats
    // per wave that lands nearest what the wind asked for. Eight beats to a four-second
    // wave is a hundred and twenty; punching into a short sea at speed gives a quicker
    // encounter and a quicker tune, which is also what it feels like.
    let proposed = (52.0 + 9.0 * force.min(6.0)).clamp(52.0, 112.0);
    let encounter = encounter_period(sim) as f32;
    let tempo = if encounter > 0.8 && encounter < 14.0 {
        let beats = (proposed * encounter / 60.0).round().clamp(2.0, 24.0);
        (beats * 60.0 / encounter).clamp(48.0, 124.0)
    } else {
        proposed
    };

    Asking {
        tempo,
        // The point of sail chooses the metre, which is where the shanty came from: four
        // to haul to, close-hauled; six-eight rolling off the wind; three in light airs
        // when nothing is being hauled at all and the boat is only drifting.
        beats: if wind < 6.0 {
            3
        } else if twa < 70.0 {
            4
        } else {
            6
        },
        // Upwind is work and Aeolian; off the wind the sixth lifts and it is a reel.
        dorian: twa >= 70.0,
        // The band thins as it blows. One voice in a gale is not a limitation, it is
        // what the sea leaves room for.
        fullness: (1.0 - (force - 4.0).max(0.0) / 4.0).clamp(0.25, 1.0),
        level: 0.55,
        // Dawn is thin and high, the middle of the day open, night low. The sun has been
        // moving the light all along and there is no reason it should not move this.
        register: if hour < 8.0 {
            0.5
        } else if hour > 18.0 {
            -1.0
        } else {
            0.0
        },
        tonic,
    }
}

/// Place the notes.
pub fn compose(
    time: Res<Time>,
    sim: Res<Sim>,
    sky: Res<crate::sky::Sky>,
    ink: Res<crate::world::Ink>,
    pilot: Res<crate::input::Pilot>,
    sound: Res<SoundHandle>,
    mut music: ResMut<Music>,
) {
    let p = &sound.0;
    p.set_shape(if ink.0 { 1.0 } else { 0.0 });

    // What the weather is asking for, against what is being played.
    //
    // A key change is not asked for by the weather directly — nothing out there is in a
    // key. It is asked for by *time*: a tune that has been in one place for a few phrases
    // wants to go somewhere, and where it goes is chosen by whether the day is making up
    // or lying down. That is the one part of this that is a musician's decision rather
    // than a sailor's, and it belongs to the form rather than to the sea.
    let here = music.playing.map_or(music.tonic, |p| p.tonic);
    let asked = asking(&sim, sky.hour, here);
    let want = music.playing.unwrap_or(asked);
    let t = &sim.sim.telemetry;

    // Three things stop the music, and each of them is the sea taking the floor.
    //
    // On the rocks there is nothing to play. A squall coming is the one moment the game
    // wants your whole attention, so the band drops to a single low note and a pulse —
    // which is louder, in the way that mattering is louder than volume. And a boat lying
    // over past the point of being sailed has a sound of her own already.
    let dying = t.mishap.is_some();
    // How near the nearest squall is, as a thing that can be built on rather than a
    // switch that is thrown.
    //
    // This is what a game can do that a film cannot. A composer scoring a storm has to
    // guess when it will arrive and write towards that guess; here the squall is an
    // object with a position and a radius, so the tension can be keyed to the actual
    // distance and be *right* — it grows because the thing is nearer, not because the
    // music has decided it is time.
    //
    // Nought at four radii out, which is far enough that you can only see it as a dark
    // patch, and one when the edge of it reaches her. Past that it is not a warning any
    // more, it is weather.
    let approach = sim
        .sim
        .wind
        .squalls()
        .iter()
        .map(|&(at, radius, _)| {
            let away = (at - sim.sim.state.pos).length() / radius.max(50.0);
            ((4.0 - away) / 3.0).clamp(0.0, 1.0) as f32
        })
        .fold(0.0_f32, f32::max);
    let inside = approach >= 1.0;
    let pressed = t.heel.abs() > sail_core::autopilot::OVERPOWERED_HEEL * 1.6;
    let hush = if dying {
        0.0
    } else if inside {
        // Arrived. The release is the squall itself: the band stops and the sea has the
        // floor, which is louder than any climax that could have been written for it.
        0.25
    } else if pressed {
        0.6
    } else {
        1.0
    };

    // --- Getting from one kind of music to another -------------------------------
    //
    // The boat's own noises do it, which is the idea this came from and a better one
    // than anything a drum could manage here: a run of semiquavers on her gear, in the
    // tempo being arrived at, over the last bar of the phrase. That is what a fill is
    // for — it is not decoration, it is the ear being taught the new pulse before the
    // tune has to land in it, and a bar of it is enough to re-lock on a different metre
    // entirely. Doing it with the winch and the clutches rather than a drum means the
    // transition is made of things that are actually on the boat, so it reads as the
    // crew getting ready rather than as music changing by itself.
    if music.bridge > 0 {
        let to = music.pending.unwrap_or(want);
        music.bridge_clock += time.delta_secs_f64() * (to.tempo as f64 / 60.0) * 4.0;
        let tick = music.bridge_clock.floor() as i64;
        if tick > music.bridge_step {
            music.bridge_step = tick;
            music.bridge -= 1;
            // Accented on the beat, so the new metre is in the fill and not only the new
            // speed — four semiquavers to a beat, and the first of each is the one the
            // ear counts.
            let on_the_beat = tick % 4 == 0;
            // The music's own block, not her gear — see the note in `sound.rs`. A fill
            // played on a winch would make the winch mean two things, and one of them is
            // something you need to be able to hear.
            p.tick(if on_the_beat { 0.75 } else { 0.3 } * hush);
            if music.bridge <= 0 {
                // Arrived. The new plan starts on a phrase boundary, which is where the
                // bridge was put so that it would.
                if let Some(to) = music.pending.take() {
                    music.playing = Some(to);
                    music.tonic = to.tonic;
                    music.beat = 0.0;
                    music.last_step = -1;
                    music.phrases_here = 0;
                }
            }
        }
        return;
    }

    music.beat += time.delta_secs_f64() * (want.tempo as f64 / 60.0);
    // Two notes to the beat, which is a quaver — the unit nearly every tune in this
    // idiom is actually made of.
    let step = (music.beat * 2.0).floor() as i64;
    if step == music.last_step {
        return;
    }
    music.last_step = step;

    if hush <= 0.0 {
        return;
    }

    // At the end of a phrase, see whether the weather has asked for somewhere else — and
    // whether the tune has sat here long enough to be worth moving at all.
    let per_bar_now = (want.beats * 2) as i64;
    if step.rem_euclid(per_bar_now) == 0 {
        let bar_now = step.div_euclid(per_bar_now) as u32 % PHRASE;
        if bar_now == PHRASE - 1 && music.pending.is_none() {
            if step > 0 {
                music.phrases_here += 1;
            }
            let mut going = asked;
            // A key change is the form's own business and happens when the tune has been
            // in one place for four phrases, whether or not the weather has moved.
            if music.phrases_here >= 4 {
                let rising = sim.sim.wind.outlook().0 > 0.0;
                let settled = !asked.worth_moving_for(&want);
                going.tonic = modulate(want.tonic, rising, settled);
            }
            if going.worth_moving_for(&want) {
                music.pending = Some(going);
                // One bar of the new metre, in semiquavers.
                music.bridge = (going.beats * 4) as i32;
                music.bridge_clock = 0.0;
                music.bridge_step = -1;
                return;
            }
        }
    }
    if music.playing.is_none() {
        music.playing = Some(asked);
    }

    let scale = if want.dorian { DORIAN } else { AEOLIAN };
    let per_bar = (want.beats * 2) as i64;
    let bar = step.div_euclid(per_bar) as u32 % PHRASE;
    let in_bar = step.rem_euclid(per_bar) as usize;
    let strong = in_bar == 0;
    // How much trouble she is in, nought to one.
    //
    // Drama does not wait for the end of a phrase. Everything else in this composer
    // changes at a bar line, with a bridge, because music that moves whenever the wind
    // moves is never anywhere — but the whole point of a cinematic shift is that it
    // arrives *with* the thing it is about. A boat lying over at twenty-three degrees
    // with a kite up and the autopilot handing back the helm does not get an eight-bar
    // preparation.
    //
    // Four things, and any of them is enough: pressed past the angle a cruising crew
    // reefs at, a sea that is throwing her about, a sail carrying more wind than its
    // cloth is cut for, and the pilot having given up — which is the machine's own
    // opinion that this is beyond it.
    let pressed = (t.heel.abs() / sail_core::autopilot::OVERPOWERED_HEEL - 1.0).clamp(0.0, 1.0);
    let rough = ((sim.sim.water.significant_height() - 1.2) / 2.0).clamp(0.0, 1.0);
    let overloaded = {
        let limit = sail_core::units::ms_to_kn(cfg_headsail_limit(&sim));
        ((sail_core::units::ms_to_kn(t.aws) / limit - 0.85) / 0.3).clamp(0.0, 1.0)
    };
    let abandoned = if pilot.last_reason.is_some() && pilot.since_reason < 12.0 {
        1.0
    } else {
        0.0
    };
    let drama = (pressed.max(rough).max(overloaded).max(abandoned) as f32).clamp(0.0, 1.0);

    // The harmony changes language when she is in trouble. Same tune, different world
    // under it — which is exactly what a film score does with a theme and why the theme
    // is worth having.
    let progression = if drama > 0.45 {
        CINEMATIC
    } else if want.dorian {
        TURNING
    } else {
        WALKING
    };
    let chord = progression[(bar % 4) as usize] as i32;

    // How hard she is going, as a fraction of what this hull can do.
    //
    // The most expressive single number a sailor has, and the one a soundtrack has no
    // business ignoring: it is the difference between punching about and *sailing*. It
    // does not change the notes — the tune is the tune — it changes how many of them
    // there are. A boat ghosting along in four knots gets the bones of the motif; the
    // same boat at seven gets it filled in with the passing notes between, which is
    // exactly what a player does when the tune lifts.
    let going = (sail_core::units::ms_to_kn(t.stw) / sail_core::units::ms_to_kn(cfg_hull(&sim)))
        .clamp(0.0, 1.15) as f32;
    let flying = going > 0.82;

    // Invented once, when she casts off, and then said over and over for the whole
    // passage. That is the point of it: by the third phrase you know this tune.
    if music.motif.is_none() {
        let mut seed = music.rng;
        let mut rng = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as f32 / u32::MAX as f32
        };
        music.motif = Some([Motif::invent(&mut rng), Motif::invent(&mut rng)]);
        music.rng = seed;
    }
    let motifs = music.motif.expect("just set");

    // What each sail is doing, which is what each voice is.
    let head_work = worked(&t.jib).max(worked(&t.jib_upper) * 0.8);
    let main_work = worked(&t.main).max(worked(&t.main_upper) * 0.8);
    let stay_work = if sim.controls.staysail > 0.02 {
        // The staysail shares the headsail's telemetry slot, so what says it is there is
        // the halyard rather than a force of its own.
        head_work * 0.8
    } else {
        0.0
    };

    let (strain, lift, upside_down, cadence) = how_to_say_it(bar);
    let motif = motifs[strain];
    // Which time through this is. A session player never plays a repeat the way he
    // played it the first time — same bones, different flesh — and that is most of what
    // makes a tune played for five minutes bearable rather than a loop.
    let repeat = (step.div_euclid(per_bar * PHRASE as i64) % 2) as u32;
    // Copied out, so the closure does not hold a borrow of the resource the notes are
    // also writing to.
    let tonic = music.tonic;
    let hz = |deg: i32, octave: f32| -> f32 {
        let d = deg.rem_euclid(7) as usize;
        let oct = (deg as f32 / 7.0).floor();
        tonic * 2.0_f32.powf(scale[d] as f32 / 12.0 + oct + octave + want.register)
    };

    // The tune. The motif, said the way this bar says it, on the chord this bar sits on.
    let spoken = motif.at(in_bar).map(|d| {
        let turned = if upside_down { -d } else { d };
        let mut deg = chord + lift + turned;
        // The cadence bar lands rather than carries on: the half cadence at the end of
        // the A part comes to rest on the fifth, and the end of the whole phrase comes
        // home to the tonic. Without this a tune has no punctuation at all — it is eight
        // bars of saying things and never finishing a sentence.
        if cadence && in_bar >= per_bar as usize - 2 {
            deg = if bar == PHRASE - 1 { 0 } else { 4 };
        }
        // And the second time through, the bones get different flesh. One note displaced
        // up a third is the commonest variation there is and it never sounds wrong,
        // because a third up is still in the chord.
        if repeat == 1 && !cadence && in_bar % 4 == 2 {
            deg += 2;
        }
        deg
    });

    if let Some(mut degree) = spoken {
        // In the ink drawing it quantises on to the pentatonic degrees — no fourth and
        // no seventh — which is why chip tunes never sound sour.
        if ink.0 {
            const PENTATONIC: [i32; 5] = [0, 1, 2, 4, 5];
            let oct = degree.div_euclid(7) * 7;
            degree = oct + PENTATONIC[(degree.rem_euclid(7) as usize * 5 / 7).min(4)];
        }
        music.degree = degree;
        let weight = if strong { 1.0 } else { 0.72 };
        // The melody is the headsail's. It is the sail you look at, the one that decides
        // whether she is going, and the one a trimmer's hands are on — so it carries the
        // tune, and it carries it only while there is a headsail doing something.
        // The tune thins out as a squall comes on and is gone before it arrives.
        let left = (1.0 - approach * 1.3).clamp(0.0, 1.0);
        p.strike(
            hz(degree, 1.0),
            want.level * weight * hush * head_work * left,
        );
        // Going well, the melody is doubled an octave up — the oldest way there is of
        // saying the same thing brighter, and it arrives exactly when she does.
        if flying && strong {
            p.strike(hz(degree, 2.0), want.level * 0.4 * hush * head_work);
        }
    } else if going > 0.35 {
        // The quaver the motif does not speak on, and what a player puts there.
        //
        // Not a random passing note, which is what was here and which sounds like
        // filling. What a piper or a fiddler does is *cut*: a flick of the note above,
        // short and unaccented, immediately before the note that matters. It is the
        // single most idiomatic thing in this music — the ornament that makes a whistle
        // sound like a whistle rather than a recorder — and it costs one strike in a slot
        // that was silent anyway.
        //
        // Only when she is going, because ornament is what a player adds when the tune
        // is running and the hands are free.
        let coming = motif.at((in_bar + 1).min(7)).map(|d| {
            let turned = if upside_down { -d } else { d };
            chord + lift + turned
        });
        if let Some(next) = coming {
            // A step above whatever is about to be played, which is what a cut is.
            p.strike(hz(next + 1, 1.0), want.level * 0.22 * going * hush);
        } else if going > 0.55 && !ink.0 {
            // And in the gaps with nothing coming, a passing note towards where the tune
            // was — only in the vanilla picture, because a chip has no voice to spare.
            let step_to = music.degree + if music.next() < 0.5 { 1 } else { -1 };
            p.strike(hz(step_to, 1.0), want.level * 0.3 * going * hush);
        }
    }

    // A squall coming, built rather than announced.
    //
    // Four things, all of them keyed to the distance, and all of them older than film:
    //
    // * **The tune goes.** The melody thins and then stops, so the ear has nothing to
    //   hold on to — which is the oldest trick there is for making somebody uneasy and
    //   needs nothing added to do it.
    // * **A pedal appears**, low and unmoving, and it does not belong to the harmony
    //   above it. A note that will not go away is a thing being waited for.
    // * **A semitone above it**, once it is close. The minor second is simply the most
    //   uncomfortable interval there is, and against a pedal it is unbearable in exactly
    //   the way that is wanted: not loud, not fast, just wrong and not stopping.
    // * **The pulse quickens.** Quavers, then semiquavers, then a tremolo — a heartbeat
    //   rather than a drum roll, which is why the rate rises instead of the volume.
    if approach > 0.15 && !inside {
        let near = ((approach - 0.15) / 0.85).clamp(0.0, 1.0);
        if strong {
            p.strike(hz(0, -2.0), (0.25 + 0.6 * near) * hush);
        }
        if near > 0.45 && strong {
            // The semitone. Not in the scale at all, which is the point — it is not part
            // of the music, it is the thing arriving.
            p.strike(music.tonic * 2.0_f32.powf(1.0 / 12.0) * 0.25, 0.3 * near * hush);
        }
        // The pulse quickens by *thinning the gaps* rather than by playing faster than
        // the composer can place: every fourth quaver while it is still a dark patch on
        // the horizon, every other one when it has a shape, every one when it is close.
        //
        // The first version of this struck twice in the same step for the close case,
        // which does nothing at all — the slot is a store and the second strike only
        // overwrites the first. A comment claiming a thing the code did not do is worse
        // than the missing beat.
        let every = if near > 0.7 {
            1
        } else if near > 0.4 {
            2
        } else {
            4
        };
        if in_bar.is_multiple_of(every) {
            p.tick((0.15 + 0.55 * near) * hush);
        }
    }

    // The cinematic treatment of the same tune: wide, slow and open.
    //
    // Three things do nearly all of it, and none is a new melody. The register opens
    // out — a pedal two octaves down and the tune up where it can be heard over it, with
    // the middle left empty, which is what makes an orchestra sound large. The chords
    // lose their thirds and become open fifths, because a bare fifth is neither major nor
    // minor and will not say how this ends. And an ostinato runs underneath on the block,
    // three and three and two, which is the oldest way there is of making something feel
    // like it is not going to stop.
    if drama > 0.45 {
        let urgency = ((drama - 0.45) / 0.55).clamp(0.0, 1.0);
        if strong {
            // The pedal: the tonic, low, held under everything whatever the chord above
            // is doing. It is what a film score puts under a scene that has not resolved.
            p.strike(hz(0, -2.0), want.level * 0.85 * hush);
            // And the chord as an open fifth, wide apart.
            p.strike(hz(chord, -1.0), want.level * 0.6 * hush * main_work.max(0.35));
            p.strike(hz(chord + 4, 0.0), want.level * 0.5 * hush * main_work.max(0.35));
        }
        // Three and three and two, which is a bar of eight read the way a heartbeat
        // reads it rather than the way a dance does.
        if matches!(in_bar % 8, 0 | 3 | 6) {
            p.tick(0.3 + 0.45 * urgency);
        }
    }

    // The left hand. A chord on the downbeat, which is what makes this a piano and not a
    // whistle — and spread a little rather than struck dead together, because nobody's
    // hand lands flat and the spread is most of what makes it sound played.
    // The mainsail is the ground: the chord under everything, which is what a mainsail
    // is to a sail plan. Reef it down and the harmony goes quiet; hand it and there is no
    // chord at all and the melody is left standing on its own, which is precisely how a
    // boat under headsail alone sounds to sail.
    if strong && !ink.0 && main_work > 0.05 && drama <= 0.45 {
        // Both of these modes are minor, so the third of the chord is the same in each —
        // which is worth saying because it is the one interval somebody would expect to
        // change with the mode and it does not. What dorian moves is the sixth, four
        // degrees further up, and that note belongs to the tune rather than to this
        // chord.
        for (i, tone) in [0, 2, 4].iter().enumerate() {
            let fatten = if want.fullness > 0.6 || i < 2 { 1.0 } else { 0.0 };
            p.strike(
                hz(chord + tone, -1.0),
                want.level * 0.5 * fatten * hush * main_work * (1.0 - 0.1 * i as f32),
            );
        }
    }

    // And the bass, which holds under a long sea and walks under a short one.
    let swell = sim.sim.water.significant_height() > 1.2;
    if (strong || (!swell && in_bar == (per_bar / 2) as usize && going > 0.5))
        && main_work > 0.05
    {
        p.strike(hz(chord, -2.0), want.level * 0.7 * hush * main_work);
    }

    // And the staysail, when it is set, is the counter-line under the melody — a second
    // voice that is only there when there is a second sail making it.
    if stay_work > 0.05 && in_bar % 4 == 2 {
        p.strike(
            hz(music.degree - 2, 0.0),
            want.level * 0.45 * hush * stay_work,
        );
    }

    if strong || (want.beats == 4 && in_bar == (per_bar / 2) as usize) {
        p.tick(0.4 * hush);
    }
}

/// What the headsail that is up is built to carry, in metres a second.
fn cfg_headsail_limit(sim: &Sim) -> f64 {
    sim.config().headsail(sim.controls.headsail).limit_aws
}

/// This hull's own speed, which is what "going well" is measured against.
fn cfg_hull(sim: &Sim) -> f64 {
    sim.config().hull_speed()
}
