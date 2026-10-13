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
        let openings: [[i32; 4]; 6] = [
            [0, 2, 1, 0],  // a turn round the tonic: the commonest opening there is
            [0, 1, 2, 4],  // a climb
            [4, 2, 1, 0],  // and the fall that answers it
            [0, 4, 2, 1],  // a leap, then filling in behind it
            [0, 2, 4, 2],  // an arch
            [2, 1, 0, -3], // dropping to the seventh below, which is very Nordic
        ];
        let shape = openings[(rng() * openings.len() as f32) as usize % openings.len()];
        // Rhythms that leave the right silences. A cell that speaks on every quaver has
        // no rhythm; what gives a tune its gait is where it does *not* play.
        let rhythms: [[bool; 8]; 4] = [
            [true, false, true, false, true, false, true, false],
            [true, false, true, true, false, true, false, false],
            [true, true, false, true, false, false, true, false],
            [true, false, false, true, true, false, true, false],
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

/// What a bar does with the motif.
///
/// The plan of a folk phrase, which is the same plan every time because it works:
/// state it, state it again, go somewhere, come back and land.
fn how_to_say_it(bar: u32) -> (i32, bool) {
    match bar % PHRASE {
        0 | 1 => (0, false),   // say it
        2 | 3 => (2, false),   // say it a third higher
        4 => (0, true),        // turn it over
        5 => (-2, true),       // and again, lower
        6 => (2, false),       // back towards home
        _ => (0, false),       // and land
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
    /// The cell the whole passage is built from, invented once when she casts off.
    motif: Option<Motif>,
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
}

/// Read the boat and the sky, and decide what kind of music this is.
///
/// Every line of this is a relation somebody could argue with on the water, which is the
/// only way a soundtrack earns the right to change.
fn asking(sim: &Sim, hour: f32) -> Asking {
    let t = &sim.sim.telemetry;
    let wind = sail_core::units::ms_to_kn(t.tws) as f32;
    let force = crate::weather::force(wind as f64) as f32;
    let twa = sail_core::units::deg(t.twa).abs() as f32;

    Asking {
        // The wind sets the pulse. Five knots is a drift and a slow air; thirty is not
        // the moment for a jig, so it climbs and then stops climbing — past a gale the
        // music is not getting faster, it is getting *thinner*.
        tempo: (52.0 + 9.0 * force.min(6.0)).clamp(52.0, 112.0),
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
    }
}

/// Place the notes.
pub fn compose(
    time: Res<Time>,
    sim: Res<Sim>,
    sky: Res<crate::sky::Sky>,
    ink: Res<crate::world::Ink>,
    sound: Res<SoundHandle>,
    mut music: ResMut<Music>,
) {
    let p = &sound.0;
    p.set_shape(if ink.0 { 1.0 } else { 0.0 });

    let want = asking(&sim, sky.hour);
    let t = &sim.sim.telemetry;

    // Three things stop the music, and each of them is the sea taking the floor.
    //
    // On the rocks there is nothing to play. A squall coming is the one moment the game
    // wants your whole attention, so the band drops to a single low note and a pulse —
    // which is louder, in the way that mattering is louder than volume. And a boat lying
    // over past the point of being sailed has a sound of her own already.
    let dying = t.mishap.is_some();
    let squall = sim
        .sim
        .wind
        .squalls()
        .iter()
        .any(|&(at, radius, _)| (at - sim.sim.state.pos).length() < radius * 2.2);
    let pressed = t.heel.abs() > sail_core::autopilot::OVERPOWERED_HEEL * 1.6;
    let hush = if dying {
        0.0
    } else if squall {
        0.3
    } else if pressed {
        0.6
    } else {
        1.0
    };

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

    let scale = if want.dorian { DORIAN } else { AEOLIAN };
    let per_bar = (want.beats * 2) as i64;
    let bar = step.div_euclid(per_bar) as u32 % PHRASE;
    let in_bar = step.rem_euclid(per_bar) as usize;
    let strong = in_bar == 0;
    let chord = if want.dorian { TURNING } else { WALKING }[(bar % 4) as usize] as i32;

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
        music.motif = Some(Motif::invent(&mut rng));
        music.rng = seed;
    }
    let motif = music.motif.expect("just set");

    let (lift, upside_down) = how_to_say_it(bar);
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
        chord + lift + turned
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
        p.strike(hz(degree, 1.0), want.level * weight * hush);
        // Going well, the melody is doubled an octave up — the oldest way there is of
        // saying the same thing brighter, and it arrives exactly when she does.
        if flying && strong {
            p.strike(hz(degree, 2.0), want.level * 0.4 * hush);
        }
    } else if going > 0.45 && !ink.0 {
        // The passing notes: only when she is moving, and only in the vanilla picture,
        // because a chip has no voices to spare for them. Between the motif's notes,
        // stepping towards wherever it goes next.
        let step_to = music.degree + if music.next() < 0.5 { 1 } else { -1 };
        p.strike(hz(step_to, 1.0), want.level * 0.34 * going * hush);
    }

    // The left hand. A chord on the downbeat, which is what makes this a piano and not a
    // whistle — and spread a little rather than struck dead together, because nobody's
    // hand lands flat and the spread is most of what makes it sound played.
    if strong && !ink.0 {
        // Both of these modes are minor, so the third of the chord is the same in each —
        // which is worth saying because it is the one interval somebody would expect to
        // change with the mode and it does not. What dorian moves is the sixth, four
        // degrees further up, and that note belongs to the tune rather than to this
        // chord.
        for (i, tone) in [0, 2, 4].iter().enumerate() {
            let fatten = if want.fullness > 0.6 || i < 2 { 1.0 } else { 0.0 };
            p.strike(
                hz(chord + tone, -1.0),
                want.level * 0.5 * fatten * hush * (1.0 - 0.1 * i as f32),
            );
        }
    }

    // And the bass, which holds under a long sea and walks under a short one.
    let swell = sim.sim.water.significant_height() > 1.2;
    if strong || (!swell && in_bar == (per_bar / 2) as usize && going > 0.5) {
        p.strike(hz(chord, -2.0), want.level * 0.7 * hush);
    }

    if strong || (want.beats == 4 && in_bar == (per_bar / 2) as usize) {
        p.tick(0.4 * hush);
    }
}

/// This hull's own speed, which is what "going well" is measured against.
fn cfg_hull(sim: &Sim) -> f64 {
    sim.config().hull_speed()
}
