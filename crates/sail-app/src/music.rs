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

use crate::course::Leg;
use crate::sim::Sim;
use crate::sound::{SoundHandle, Voice};

/// The band aboard.
///
/// What this module does in a frame, in one place — see `stages.rs` for why the
/// schedule is named sets rather than one long chain.
pub fn plugin(app: &mut App) {
    use crate::stages::Deck;
    app.init_resource::<Music>();
    app.add_systems(Update, compose.in_set(Deck::Weather));
}

/// Semitones above the tonic, for the two modes that matter.
///
/// Aeolian is the minor everyone knows. Dorian raises the sixth, and that one note is
/// the difference between a lament and a reel — it is why a tune in D dorian lifts where
/// the same notes in D minor sag.
const AEOLIAN: [i32; 7] = [0, 2, 3, 5, 7, 8, 10];
const DORIAN: [i32; 7] = [0, 2, 3, 5, 7, 9, 10];

/// And the third one, for when the sea is winning.
///
/// Phrygian is the minor with a *flattened second* — one semitone above the tonic
/// instead of a whole tone — and that single note is the most menacing interval in
/// Western music. It is the Dies Irae, it is every flamenco cadence, and it is under
/// essentially every boss theme ever written, which is not a coincidence: a note a
/// semitone above the home note leans on it and will not get off, so the key itself
/// sounds like something pressing down on it. The rest of the scale is the minor
/// everybody already knows, so the mode change is heard as *this one thing has gone
/// wrong* rather than as different music.
const PHRYGIAN: [i32; 7] = [0, 1, 3, 5, 7, 8, 10];

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

/// And the one for a fight that is being lost.
///
/// i–bII–i–bVI: the Phrygian see-saw, two chords a semitone apart rocking against each
/// other with nowhere to go. Everything else in this file walks or slides somewhere;
/// this one is deliberately *stuck*, because the feeling being played is not tension
/// about what happens next — it is being held down by something stronger than you. The
/// flat sixth at the end of the bar is the only movement allowed, and it moves the wrong
/// way, further from home instead of back towards it.
const SIEGE: [usize; 4] = [0, 1, 0, 5];

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
            [0, 2, 1, 0],  // a turn round the tonic: the commonest opening there is
            [0, 1, 2, 4],  // a climb
            [4, 2, 1, 0],  // and the fall that answers it
            [0, 4, 2, 1],  // a leap, then filling in behind it
            [0, 2, 4, 2],  // an arch
            [2, 1, 0, -3], // dropping to the seventh below, which is very Nordic
            [0, 0, 2, 4],  // a repeated note that then goes: a shanty's shape
            [4, 4, 2, 0],  // the same thing falling, which is how a chorus answers
            [0, -1, 0, 2], // dipping under the tonic first — a lift that works by delay
            [2, 4, 3, 2],  // a turn higher up, round the third
            [0, 3, 2, 1],  // a fourth, the interval every hymn and half the sea songs
            [1, 0, 2, 0],  // leaning on the second and resolving down, twice
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
        0 => (0, 0, false, false), // the A part
        1 => (0, 2, false, false), // said a third higher
        2 => (0, 0, true, false),  // turned over
        3 => (0, 0, false, true),  // and landed: the half cadence
        4 => (1, 4, false, false), // the B part, up where a B part lives
        5 => (1, 4, false, false), // said again
        6 => (1, 2, true, false),  // turned over and coming down
        _ => (0, 0, false, true),  // home, with the A's own shape, and landed
    }
}

/// What the composer is doing, for the instrument panel.
///
/// Observability, because the alternative is guessing. Reported from the deck as not
/// being able to hear the music at a moment when it should obviously have been playing
/// something cheerful, and there was no way to tell from outside whether the composer
/// had chosen something thin, or chosen nothing, or chosen well and been drowned. Three
/// very different faults with one symptom, and a panel line settles it in a keypress.
#[derive(Clone, Copy, Default)]
pub struct Report {
    pub tempo: f32,
    pub beats: u32,
    pub mode: &'static str,
    pub going: &'static str,
    pub drama: f32,
    pub beyond: f32,
    /// What each sail is contributing, which is what decides whether there is a tune,
    /// a chord, or neither.
    pub head: f32,
    pub main: f32,
}

#[derive(Resource)]
pub struct Music {
    /// The last thing it decided, so the panel can say what it is playing.
    pub report: Report,
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
            report: Report::default(),
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
    /// Musical time, in beats since the passage began.
    ///
    /// Public so that things which are not music can be placed *in* it — a call from
    /// the deck lands on a quaver rather than wherever the frame happened to fall. See
    /// `hail.rs`.
    pub fn beat(&self) -> f64 {
        self.beat
    }

    fn next(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        self.rng as f32 / u32::MAX as f32
    }
}

/// What a point of sail *is*, musically.
///
/// The three are not three settings of one thing. They are three different kinds of
/// sailing and everybody who has done any of it knows which one they are in without
/// looking at an instrument, so the music should know too.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Going {
    /// Close-hauled: a fight. She is being made to do something she does not want to,
    /// everything is loaded, and it goes on for hours.
    Beating,
    /// Reaching: the one that is worth all the rest. Fastest, most alive, and the only
    /// point of sail that feels like it is *driving* rather than coping or coasting.
    Reaching,
    /// Running: chill. The apparent wind falls away, the boat goes quiet, and there is
    /// nothing to do but keep her there.
    Running,
}

impl Going {
    fn of(twa: f32) -> Self {
        let t = twa.abs();
        if t < 62.0 {
            Going::Beating
        } else if t < 132.0 {
            Going::Reaching
        } else {
            Going::Running
        }
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
        } else {
            match Going::of(twa) {
                // Four to haul to: a halyard shanty is in four because it is sung to a
                // pull, and beating is nothing but pulling.
                Going::Beating => 4,
                // Four again, but it will be filled quite differently — a reach wants a
                // motor and six-eight lilts where this needs to drive.
                Going::Reaching => 4,
                // Six-eight rolls the way she rolls off the wind.
                Going::Running => 6,
            }
        },
        // Upwind is work and Aeolian; off the wind the sixth lifts and it is a reel.
        // Upwind is work and Aeolian. Off the wind the sixth lifts and it is a reel.
        dorian: Going::of(twa) != Going::Beating,
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
#[allow(clippy::too_many_arguments)]
pub fn compose(
    time: Res<Time>,
    sim: Res<Sim>,
    sky: Res<crate::sky::Sky>,
    ink: Res<crate::world::Ink>,
    pilot: Res<crate::input::Pilot>,
    fishing: Res<crate::fishing::Fishing>,
    course: Res<crate::course::Course>,
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

    // --- Past what she can take ---------------------------------------------------
    //
    // `drama` is the boat being *pressed*: heeled past where a cruising crew reefs, a
    // sea throwing her about, the pilot handing back the helm. All of that is hard
    // sailing, and hard sailing is what this game is for — the cinematic treatment is
    // the right answer to it, because a film score under a scene that has not resolved
    // is exactly what being overpowered feels like.
    //
    // This is the other thing, and conflating the two was the gap. There is a line past
    // which the boat is not being sailed hard, she is *beyond her numbers*: the cloth is
    // carrying more wind than it is cut for and has a measurable number of seconds left,
    // or she is over the peak of her righting arm and going. That is not tension about
    // what might happen. It is a fight with something bigger than the boat, and it is
    // the only state in this game that has a *boss* in it.
    //
    // All three measures are already in the model, and all three are past-the-limit
    // rather than near-it — which is what makes this honest. Strain especially: it is a
    // countdown that reaches one and blows the sail out, so it is the sea's own clock
    // and the music is simply reading it.
    // The boat past her numbers, and the tackle past its own, and the worse of the two.
    //
    // A fight goes in here rather than getting a theme of its own, and that is the whole
    // of the design. Everything this composer plays is read off something that has
    // already happened — the cloth's countdown, the heel, the sea against her length —
    // and a line at four fifths of its breaking strain is the same kind of fact about a
    // different object. So the band tightens because the *line* is tight.
    //
    // What it buys is that nothing has to decide which fish is a boss. A mackerel never
    // reaches it; a tope running reaches it outright; and a big one played properly, on a
    // drag set right and given its head, does not get the full treatment until you make a
    // mistake — which is the right way round and is not a thing a trigger could express.
    let beyond = beyond_her(
        t,
        sail_core::units::ms_to_kn(cfg_headsail_limit(&sim)),
        cfg_loa(&sim),
    )
    .max(fishing.pressing());

    // The sea stops setting the pace. Everywhere else in this composer the tempo comes
    // from the encounter period, because the hull meeting the waves is the rhythm
    // section — but a fight has its own pulse and it is faster than the one you are
    // being given. A fifth quicker at the worst of it, which is enough to feel like
    // being hurried and not enough to lose the lock on the sea underneath.
    music.beat += time.delta_secs_f64() * ((want.tempo * (1.0 + 0.2 * beyond)) as f64 / 60.0);
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

    let scale = if beyond > 0.35 {
        PHRYGIAN
    } else if want.dorian {
        DORIAN
    } else {
        AEOLIAN
    };
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
    let progression = if beyond > 0.35 {
        SIEGE
    } else if drama > 0.45 {
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
    // How much of the band is out. A boat at four knots and the same boat at seven are
    // not the same scene, and the difference belongs in the *level* as well as in the
    // number of notes — a crew on a boat that is going sings up.
    let gusto = 0.75 + 0.5 * going.min(1.1);
    let flying = going > 0.72;

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
    //
    // Two different questions, and running them together was a real fault: whether the
    // sail is *there* decides whether the voice exists, and how hard it is working
    // decides how loud it is. A mainsail that luffs for four seconds in a lull is still
    // the mainsail — it does not stop being the ground the harmony stands on — but the
    // first version gated the whole left hand on the instantaneous force, so every time
    // the main shook the chords vanished and the band fell to a melody on its own.
    // Reported from the deck as not being able to hear the music on a beam reach at six
    // knots, which is exactly where a main luffs: eased for the reach and shaking at the
    // luff, while the genoa does the work.
    //
    // So: set, and then weighted, with a floor under the weight. A sail that is up never
    // counts for less than a third, and only taking it off her silences its voice.
    let voice_of = |work: f32, set: bool| if set { 0.35 + 0.65 * work } else { 0.0 };
    let main_set = cfg_main_area(&sim) > 0.5;
    let head_set = sim.controls.furl < 0.985;
    let head_work = voice_of(worked(&t.jib).max(worked(&t.jib_upper) * 0.8), head_set);
    let main_work = voice_of(worked(&t.main).max(worked(&t.main_upper) * 0.8), main_set);
    let stay_work = if sim.controls.staysail > 0.02 {
        // The staysail shares the headsail's telemetry slot, so what says it is there is
        // the halyard rather than a force of its own.
        head_work * 0.8
    } else {
        0.0
    };

    let (strain, lift, upside_down, cadence) = how_to_say_it(bar);
    // Nothing lands while this is going on. A cadence is a sentence finishing, and the
    // one thing a fight with the weather does not do is finish a sentence — which is
    // also why the siege progression has no resolution in it to reach.
    let cadence = cadence && beyond < 0.35;
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

    // How soon each note is let go of — and why a sung idiom needs to be told.
    //
    // A shanty is *sung*. It is a line of separate words with breath between them, and
    // the crew hauling to it stops the note on the pull; an air played on a whistle is
    // tongued the same way. A piano does the opposite: every note rings for two or three
    // seconds, so at this tempo eight or nine of them are sounding at once and the tune
    // dissolves into its own harmony. Which is pleasant, and is not a song — reported
    // from the deck as wanting a little silence between the sounds, which is exactly what
    // a singer leaves without being asked for it.
    //
    // Stopped harder the faster the tune runs, because the thing that has to stay the
    // same is the *gap*, not the note: hold about a quarter of a second of ring whatever
    // the tempo, and a quaver always has air after it.
    //
    // The cinematic treatment below gets none of this. There the long ring is the sound.
    let quaver = 30.0 / want.tempo.max(30.0);
    let detach = (0.25 / quaver).clamp(0.45, 0.85);

    // And how much of the tune is left. The siege does not play *over* the song — it
    // takes the song's place, because the point being made is that the thing you were
    // doing has stopped: you are not on a passage any more, you are trying not to break
    // anything. The danger voices below (the squall, the ground, the harbour) keep their
    // own level, because those are warnings and a warning does not get quieter when
    // things get worse.
    let tune = hush * (1.0 - beyond);

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
        // A silent note is not free: it takes a slot out of the eight a step has and a
        // voice out of the twelve, and the siege below needs both.
        if tune > 0.02 {
            p.strike_damped(
                hz(degree, 1.0),
                want.level * weight * gusto * tune * head_work * left,
                detach,
            );
            // Going well, the melody is doubled an octave up — the oldest way there is of
            // saying the same thing brighter, and it arrives exactly when she does.
            if flying && strong {
                p.strike_damped(
                    hz(degree, 2.0),
                    want.level * 0.4 * gusto * tune * head_work,
                    detach,
                );
            }
        }
    } else if going > 0.35 && tune > 0.02 {
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
            // Stopped almost at once: a cut is a flick of a note, not a note.
            p.strike_damped(hz(next + 1, 1.0), want.level * 0.22 * going * tune, 0.95);
        } else if going > 0.55 && !ink.0 {
            // And in the gaps with nothing coming, a passing note towards where the tune
            // was — only in the vanilla picture, because a chip has no voice to spare.
            let step_to = music.degree + if music.next() < 0.5 { 1 } else { -1 };
            p.strike_damped(hz(step_to, 1.0), want.level * 0.3 * going * tune, detach);
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
            // Bowed, because that is what a thing being waited for sounds like: it
            // arrives, it holds, and it does not stop when the bar does.
            p.strike_voiced(hz(0, -2.0), (0.25 + 0.6 * near) * hush, 0.0, Voice::Bowed);
        }
        if near > 0.45 && strong {
            // The semitone. Not in the scale at all, which is the point — it is not part
            // of the music, it is the thing arriving.
            p.strike_voiced(
                music.tonic * 2.0_f32.powf(1.0 / 12.0) * 0.25,
                0.3 * near * hush,
                0.0,
                Voice::Bowed,
            );
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

    // --- The passage: where she is going, and whether she will get there ------------
    //
    // The music knew everything about the weather and nothing about the *voyage*, which
    // is the whole arc of this game — you set out, you are given a hard time, and you
    // arrive. There was not a note for arriving, which is the one moment in a passage
    // when anything resolves.
    let (range, _) = course.to_target(sim.sim.state.pos);
    let miles = range / 1852.0;
    let overdue = course.remaining() < 0.0;

    // Coming in. A mile out the harbour is a thing you can see, and from there the music
    // starts going home: the major third over the tonic, which is the one bright interval
    // this minor world has and is therefore worth saving for exactly this.
    if miles < 1.2 && !matches!(course.leg, Leg::Idle) {
        let home = ((1.2 - miles) / 1.2).clamp(0.0, 1.0) as f32;
        if strong && bar.is_multiple_of(2) {
            p.strike_voiced(
                hz(0, 0.0),
                want.level * 0.5 * home * hush,
                0.0,
                Voice::Bowed,
            );
            // The third. In a passage that has been in a minor mode for half an hour,
            // one major third is an enormous event, and it costs nothing because it has
            // been withheld all along.
            if home > 0.45 {
                p.strike_voiced(
                    music.tonic * 2.0_f32.powf(4.0 / 12.0),
                    want.level * 0.45 * home * hush,
                    0.0,
                    Voice::Bowed,
                );
            }
        }
    }

    // And overdue, which is the other end of the same feeling: the harmony stops landing.
    // The seventh under the tonic rather than the tonic itself, so every phrase ends
    // somewhere that is nearly home and is not.
    if overdue && strong && bar == PHRASE - 1 {
        p.strike_damped(hz(-1, -1.0), want.level * 0.5 * hush, detach * 0.5);
    }

    // --- Water under the keel -------------------------------------------------------
    //
    // The thing that actually ends passages had an alarm on the panel and not one note in
    // the score. It gets the same device as a squall for the same reason — a pulse whose
    // rate says how near it is — but two octaves down and on the pedal rather than the
    // block, so the two can happen at once and still be told apart. A squall is something
    // arriving; the ground is something you are going *towards*, and the lower register
    // is the difference.
    let under = t.under_keel as f32;
    if under < 6.0 && under > -1.0 {
        let shoal = ((6.0 - under) / 6.0).clamp(0.0, 1.0);
        let every = if shoal > 0.75 {
            1
        } else if shoal > 0.45 {
            2
        } else {
            4
        };
        if in_bar.is_multiple_of(every) {
            // A semitone *below* the tonic, which is the note that does not belong to
            // anything above it. Rising harmony is hope; this is the floor coming up.
            p.strike_voiced(
                music.tonic * 0.25 * 2.0_f32.powf(-1.0 / 12.0),
                (0.2 + 0.6 * shoal) * hush,
                0.0,
                Voice::Ground,
            );
        }
    }

    // --- The motion -----------------------------------------------------------------
    //
    // A boat punching into a head sea and a boat surfing down a swell at the same wind
    // speed feel nothing alike and used to sound identical. Both are in the telemetry
    // and neither was being read.
    //
    // Surfing is the one everybody remembers: she picks up, the apparent goes aft, and
    // for ten seconds she is doing something she cannot normally do. It gets the octave
    // above, struck on the beat — the oldest way of saying *brighter*, and it arrives
    // when she does.
    if t.surf_force > 400.0 && strong {
        p.strike_damped(hz(music.degree, 2.0), want.level * 0.35 * hush, detach);
    }
    // And the slam: when she is pitching hard the block follows the *hull* rather than
    // the bar line, which is the rhythm section going back to what it was taken from.
    if sim.sim.state.pitch_rate.abs() > 0.22 && in_bar.is_multiple_of(2) {
        p.tick(0.3 * hush);
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
            // The whole cinematic shift in one decision: the same notes, bowed. A pedal
            // and two open fifths that swell and hold are an orchestra; struck, they were
            // a piano playing fifths, and nobody could hear the language had changed.
            p.strike_voiced(hz(0, -2.0), want.level * 0.85 * hush, 0.0, Voice::Ground);
            // And the chord as an open fifth, wide apart.
            p.strike_voiced(
                hz(chord, -1.0),
                want.level * 0.6 * hush * main_work.max(0.35),
                0.0,
                Voice::Bowed,
            );
            p.strike_voiced(
                hz(chord + 4, 0.0),
                want.level * 0.5 * hush * main_work.max(0.35),
                0.0,
                Voice::Bowed,
            );
        }
        // Three and three and two, which is a bar of eight read the way a heartbeat
        // reads it rather than the way a dance does.
        if matches!(in_bar % 8, 0 | 3 | 6) {
            p.tick(0.3 + 0.45 * urgency);
        }
    }

    // --- What kind of sailing this is, played rather than described -----------------
    let going_kind = Going::of(sail_core::units::deg(t.twa) as f32);
    music.report = Report {
        tempo: want.tempo,
        beats: want.beats,
        mode: if beyond > 0.35 {
            "phrygian"
        } else if want.dorian {
            "dorian"
        } else {
            "aeolian"
        },
        going: match going_kind {
            Going::Beating => "beating",
            Going::Reaching => "reaching",
            Going::Running => "running",
        },
        drama,
        beyond,
        head: head_work,
        main: main_work,
    };
    match if tune > 0.02 {
        going_kind
    } else {
        Going::Running
    } {
        // **The fight.** Everything lands where it is not expected. Syncopation is
        // effort — a beat pushed off its place is the sound of something resisting — and
        // close-hauled is the only point of sail where the boat is being made to do
        // something she would rather not. The bass goes to one and three and leaves the
        // middle of the bar empty, so each note is a heave rather than a walk.
        Going::Beating => {
            if in_bar % 4 == 3 && drama < 0.45 {
                // The off-beat push, under the melody: a second voice leaning where the
                // beat is not.
                p.strike_voiced(
                    hz(chord + 4, -1.0),
                    want.level * 0.4 * tune * main_work * going.max(0.3),
                    0.3,
                    Voice::Ground,
                );
            }
        }
        // **The drive.** This is the Bach and the rock, which are the same animal: a
        // motor in the quavers and a bass that *walks* — moving by step from one chord to
        // the next instead of jumping to the root and sitting there. That walking is what
        // makes a reach feel like it is going somewhere rather than merely going fast,
        // and it is why a toccata and a riff do the same thing to a listener.
        Going::Reaching => {
            if !ink.0 && main_work > 0.05 {
                // Every quaver, stepping towards the next chord's root. The step is
                // chosen by where the harmony is heading, which is the whole trick.
                let next = progression[((bar + 1) % 4) as usize] as i32;
                let along = (in_bar as i32).min(per_bar as i32 - 1);
                let reach_to = chord + (next - chord) * along / per_bar.max(1) as i32;
                p.strike_voiced(
                    hz(reach_to, -1.0),
                    want.level * 0.3 * gusto * tune * main_work * (0.5 + 0.5 * going),
                    0.35,
                    Voice::Ground,
                );
            }
        }
        // **Chill.** Long notes and a great deal of air. Off the wind the apparent falls
        // away, the boat goes quiet, and the music should do the same — the mistake would
        // be to fill it, when what a run actually sounds like is space with a boat in it.
        Going::Running => {
            if strong && bar.is_multiple_of(2) && tune > 0.02 {
                p.strike_voiced(
                    hz(chord, -1.0),
                    want.level * 0.45 * tune * main_work,
                    0.0,
                    Voice::Bowed,
                );
            }
        }
    }

    // The left hand. A chord on the downbeat, which is what makes this a piano and not a
    // whistle — and spread a little rather than struck dead together, because nobody's
    // hand lands flat and the spread is most of what makes it sound played.
    // The mainsail is the ground: the chord under everything, which is what a mainsail
    // is to a sail plan. Reef it down and the harmony goes quiet; hand it and there is no
    // chord at all and the melody is left standing on its own, which is precisely how a
    // boat under headsail alone sounds to sail.
    if strong && !ink.0 && main_work > 0.05 && drama <= 0.45 && tune > 0.02 {
        // Both of these modes are minor, so the third of the chord is the same in each —
        // which is worth saying because it is the one interval somebody would expect to
        // change with the mode and it does not. What dorian moves is the sixth, four
        // degrees further up, and that note belongs to the tune rather than to this
        // chord.
        for (i, tone) in [0, 2, 4].iter().enumerate() {
            let fatten = if want.fullness > 0.6 || i < 2 {
                1.0
            } else {
                0.0
            };
            p.strike(
                hz(chord + tone, -1.0),
                want.level * 0.5 * gusto * fatten * tune * main_work * (1.0 - 0.1 * i as f32),
            );
        }
    }

    // And the bass, which holds under a long sea and walks under a short one.
    let swell = sim.sim.water.significant_height() > 1.2;
    if (strong || (!swell && in_bar == (per_bar / 2) as usize && going > 0.5)) && main_work > 0.05 {
        p.strike(hz(chord, -2.0), want.level * 0.7 * gusto * tune * main_work);
    }

    // And the staysail, when it is set, is the counter-line under the melody — a second
    // voice that is only there when there is a second sail making it.
    if stay_work > 0.05 && in_bar % 4 == 2 && tune > 0.02 {
        p.strike_voiced(
            hz(music.degree - 2, 0.0),
            want.level * 0.45 * tune * stay_work,
            0.0,
            Voice::Bowed,
        );
    }

    // --- Beyond her: a fight with something bigger than the boat --------------------
    //
    // Everything above this line is a passage: the music of being somewhere, going
    // somewhere, working the boat. This is what replaces it when the boat has run out of
    // numbers, and it is built as a boss theme because that is honestly what the
    // situation is — a thing arrives that is stronger than you are, you cannot outrun it,
    // and the only question left is whether you survive the next minute.
    //
    // Five parts, and not one of them is a new tune, because a boss theme is not a tune:
    //
    // * **The treadmill.** The low note on every quaver without exception, alternating
    //   tonic and flat second. A pedal that *moves* a semitone and comes straight back is
    //   the most relentless figure in the idiom — it is going nowhere at speed, which is
    //   precisely what being overpowered is.
    // * **The cluster.** Root, flat second and fifth together on the downbeat. Not a
    //   chord anybody would write for a song: the second is a semitone off the root and
    //   grinds against it, and that grinding is the whole point.
    // * **The tritone**, once she is genuinely over. The interval the church banned,
    //   and the only one in this file that is not in the scale at all.
    // * **A cell, not a melody.** Four notes said the same way every bar, high and
    //   hammered short — tonic, flat second, tonic, down a fourth. A tune invites you to
    //   follow it somewhere; a cell says the same thing at you until the situation
    //   changes, and the difference between those two is the difference between sailing
    //   and surviving.
    // * **The block on every quaver**, which is the one part that needs no explanation.
    //
    // It comes in from eight per cent rather than at a threshold, so the first thing a
    // sailor hears is the pedal arriving underneath the tune they were already in —
    // which is the right order: you feel the boat go over her numbers, and the music is
    // already there.
    if beyond > 0.08 {
        let fight = ((beyond - 0.08) / 0.92).clamp(0.0, 1.0);
        p.strike_voiced(
            hz(if in_bar.is_multiple_of(2) { 0 } else { 1 }, -2.0),
            want.level * (0.45 + 0.4 * fight) * hush,
            0.45,
            Voice::Ground,
        );
        if strong {
            for (tone, loud) in [(0, 0.8), (1, 0.62), (4, 0.55)] {
                // Bowed and held: a cluster that is struck is a bang, and a bang is over.
                // This one has to still be there when the next bar arrives.
                p.strike_voiced(
                    hz(tone, -1.0),
                    want.level * loud * (0.4 + 0.6 * fight) * hush,
                    0.0,
                    Voice::Bowed,
                );
            }
        }
        if fight > 0.5 && in_bar == (per_bar / 2) as usize {
            p.strike_voiced(
                music.tonic * 2.0_f32.powf(6.0 / 12.0),
                want.level * 0.5 * fight * hush,
                0.0,
                Voice::Bowed,
            );
        }
        // Hammered on the even quavers, which at this tempo is a note every third of a
        // second and is as fast as this synthesiser can speak.
        const CELL: [i32; 4] = [0, 1, 0, -3];
        if in_bar.is_multiple_of(2) {
            let say = CELL[(in_bar / 2) % 4];
            p.strike_damped(hz(say, 1.0), want.level * (0.3 + 0.45 * fight) * hush, 0.8);
        }
        p.tick((0.25 + 0.5 * fight) * hush);
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
/// How far past her own numbers the boat is, nought to one.
///
/// Its own function because it is the one state in this composer worth arguing about on
/// the water, and an argument needs something to point at. Five measures, every one of
/// them a thing that has already happened rather than a thing that might:
fn beyond_her(t: &sail_core::state::Telemetry, headsail_limit_kn: f64, loa: f64) -> f32 {
    use sail_core::units::{ms_to_kn, rad};
    // **The cloth's own clock.** A sail over the wind it is cut for has a measurable
    // number of seconds left, and this is that countdown. The shortest fuse here.
    let cloth = t.strain;
    // **Carrying more than the sail plan can carry.** Reads as beyond at a tenth over
    // the number and fully so at a quarter, which is also about where the strain above
    // starts running fast enough to matter.
    let over = (ms_to_kn(t.aws) / headsail_limit_kn - 1.0) / 0.25;
    // **Knocked down.** Thirty-five degrees is not sailing — the side deck is in, the
    // rudder is coming out, and a cruising boat there is being held down rather than
    // sailed. Not the capsize criterion, which is twice as far and much too late to be
    // worth scoring.
    let knocked = (t.heel.abs() - rad(35.0)) / rad(15.0);
    // **A sea too big for the boat**, and the measure is the height against her own
    // length, because that is what decides whether a wave is a motion or an event: two
    // metres under an eleven-metre hull is sailing, three and a half is being thrown
    // about by something that does not care. This is the one of the five that *lasts* —
    // the others are a few seconds each — and it is why this state is music rather than
    // a sting.
    let sea = (t.significant_height / loa.max(5.0) - 0.18) / 0.14;
    // **And not steering.** The blade has lost its grip while she is still making way,
    // which is the one that frightens people who have actually been out there: the
    // tiller goes light, nothing answers, and the next wave decides where the bow goes.
    let helpless = if ms_to_kn(t.stw) > 1.5 {
        (0.3 - t.rudder_bite) / 0.3
    } else {
        0.0
    };
    // The worst of them, because being past any one of these is being past her.
    let worst = cloth
        .max(over)
        .max(knocked)
        .max(sea)
        .max(helpless)
        .clamp(0.0, 1.0);
    worst as f32
}

/// How much mainsail is actually up, square metres. Nought means it is off her.
fn cfg_main_area(sim: &Sim) -> f64 {
    sim.config().reefed_main(sim.controls.reef).0
}

/// Her length overall, which is the yardstick a sea is measured against.
fn cfg_loa(sim: &Sim) -> f64 {
    sim.config().loa
}

fn cfg_hull(sim: &Sim) -> f64 {
    sim.config().hull_speed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sail_core::state::Telemetry;
    use sail_core::units::{kn_to_ms, rad};

    /// An eleven-metre charter sloop with a genoa on, which is the boat this is for.
    const LIMIT_KN: f64 = 42.0;
    const LOA: f64 = 11.0;

    fn at_sea() -> Telemetry {
        Telemetry {
            aws: kn_to_ms(16.0),
            stw: kn_to_ms(5.5),
            heel: rad(14.0),
            significant_height: 0.6,
            rudder_bite: 0.8,
            strain: 0.0,
            ..Default::default()
        }
    }

    #[test]
    fn a_good_days_sailing_is_not_a_boss_fight() {
        // Fourteen degrees of heel, sixteen knots over the deck and a half-metre sea is
        // a lovely afternoon, and the music has no business changing language for it.
        assert_eq!(beyond_her(&at_sea(), LIMIT_KN, LOA), 0.0);
        // Nor is being pressed. Twenty-eight degrees and twenty-six knots apparent is
        // hard work and wants a reef — that is what `drama` is for, and it must not
        // reach this.
        let pressed = Telemetry {
            aws: kn_to_ms(26.0),
            heel: rad(28.0),
            significant_height: 1.4,
            rudder_bite: 0.55,
            ..at_sea()
        };
        assert_eq!(beyond_her(&pressed, LIMIT_KN, LOA), 0.0);
    }

    #[test]
    fn five_ways_of_being_past_her_numbers() {
        // The cloth's clock, which is the shortest fuse: half spent is half of this.
        let fraying = Telemetry {
            strain: 0.5,
            ..at_sea()
        };
        assert!((beyond_her(&fraying, LIMIT_KN, LOA) - 0.5).abs() < 0.01);

        // Carrying more than the sail is cut for. A genoa in forty-seven knots is a
        // tenth over its number.
        let too_much = Telemetry {
            aws: kn_to_ms(47.0),
            ..at_sea()
        };
        assert!(beyond_her(&too_much, LIMIT_KN, LOA) > 0.35);

        // Knocked down: the side deck is in and the rudder is coming out.
        let over = Telemetry {
            heel: rad(44.0),
            ..at_sea()
        };
        assert!(beyond_her(&over, LIMIT_KN, LOA) > 0.5);

        // And the one that lasts: a three-metre sea under an eleven-metre hull.
        let gale = Telemetry {
            significant_height: 3.0,
            ..at_sea()
        };
        assert!(
            beyond_her(&gale, LIMIT_KN, LOA) > 0.5,
            "a gale is beyond her for hours: {}",
            beyond_her(&gale, LIMIT_KN, LOA)
        );
        // Two metres is not, which is the line that matters: it is the difference
        // between a rough day and a bad one.
        let rough = Telemetry {
            significant_height: 2.0,
            ..at_sea()
        };
        assert!(beyond_her(&rough, LIMIT_KN, LOA) < 0.25);

        // Not steering. Making way, and the blade has nothing left.
        let helpless = Telemetry {
            rudder_bite: 0.1,
            ..at_sea()
        };
        assert!(beyond_her(&helpless, LIMIT_KN, LOA) > 0.6);
        // But a boat stopped has a dead rudder by definition, and that is not a boss
        // fight — it is a boat going nowhere, which the music already has a word for.
        let becalmed = Telemetry {
            rudder_bite: 0.0,
            stw: kn_to_ms(0.4),
            ..at_sea()
        };
        assert_eq!(beyond_her(&becalmed, LIMIT_KN, LOA), 0.0);
    }
}
