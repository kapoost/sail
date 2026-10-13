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
    /// The melody's position in the scale, so it walks rather than jumps.
    degree: i32,
    /// What was played in the first half of the phrase, to be answered in the second.
    /// AABB is a form and not an ornament: the repeat is what makes it memorable.
    said: [i32; 8],
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
            said: [0; 8],
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
    settings: Res<crate::settings::Settings>,
    sound: Res<SoundHandle>,
    mut music: ResMut<Music>,
) {
    let p = &sound.0;
    p.set_shape(if ink.0 { 1.0 } else { 0.0 });
    p.set_music(settings.music);

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
        for v in 0..3 {
            p.set_voice(v, 0.0, 0.0);
        }
        return;
    }

    let scale = if want.dorian { DORIAN } else { AEOLIAN };
    let per_bar = (want.beats * 2) as i64;
    let bar = step.div_euclid(per_bar) as u32 % PHRASE;
    let in_bar = step.rem_euclid(per_bar);
    let strong = in_bar == 0;
    let chord = if want.dorian { TURNING } else { WALKING }[(bar % 4) as usize];

    // The tune. First half of the phrase it is made up; second half it is answered, which
    // is what AABB means and what the ear is waiting for.
    let index = (in_bar as usize).min(7);
    let degree = if bar < PHRASE / 2 {
        // A walk, mostly by step, landing on a chord tone where the bar does.
        let roll = music.next();
        let jump = if roll < 0.44 {
            -1
        } else if roll < 0.88 {
            1
        } else if roll < 0.95 {
            2
        } else {
            -2
        };
        let mut d = (music.degree + jump).clamp(-3, 9);
        if strong {
            // Land on something that belongs to the chord under it, or the tune wanders.
            d = chord as i32 + if music.next() < 0.5 { 0 } else { 2 };
        }
        music.degree = d;
        if bar * per_bar as u32 / per_bar as u32 == bar {
            music.said[index] = d;
        }
        d
    } else {
        music.said[index]
    };

    let hz = |deg: i32, octave: f32| -> f32 {
        let d = deg.rem_euclid(7) as usize;
        let oct = (deg as f32 / 7.0).floor();
        music.tonic
            * 2.0_f32.powf(
                scale[d] as f32 / 12.0 + oct + octave + want.register,
            )
    };

    // Lead. In the ink drawing it is quantised on to the pentatonic degrees — no fourth,
    // no seventh — which is what a chip tune does and why those tunes never sound sour.
    let lead_deg = if ink.0 {
        const PENTATONIC: [i32; 5] = [0, 1, 2, 4, 5];
        let oct = degree.div_euclid(7) * 7;
        oct + PENTATONIC[(degree.rem_euclid(7) as usize * 5 / 7).min(4)]
    } else {
        degree
    };
    let swing = if want.beats == 6 && in_bar % 3 == 1 {
        // The lilt of a jig: the middle of each triplet is lighter.
        0.6
    } else {
        1.0
    };
    p.set_voice(
        0,
        hz(lead_deg, 1.0),
        want.level * hush * swing * if strong { 1.0 } else { 0.78 },
    );

    // The counter-line: a third below, and only when there is room for it.
    let counter = want.fullness > 0.55 && in_bar % 2 == 0;
    p.set_voice(
        1,
        hz(lead_deg - 2, 0.0),
        if counter { want.level * 0.5 * hush } else { 0.0 },
    );

    // The bass walks the chord on the beat, and holds through a swell — a long note
    // under a long sea is the oldest trick there is and it is right for the reason it is
    // a trick: the music takes its length from the water.
    let swell = sim.sim.water.significant_height() > 1.2;
    let bass_now = strong || (!swell && in_bar == per_bar / 2);
    if bass_now {
        p.set_voice(2, hz(chord as i32, -1.0), want.level * 0.8 * hush);
    }

    if strong || (want.beats == 4 && in_bar == per_bar / 2) {
        p.tick(0.5 * hush);
    }
}
