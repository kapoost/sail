//! The sound of sailing, synthesised rather than sampled.
//!
//! There are no audio files in this repository and there do not need to be: wind and
//! water *are* filtered noise, so generating them is the honest method, not a
//! shortcut. A sampled loop would also fight the thing that matters most here — the
//! sound has to answer the boat continuously, because it is a feedback channel. You
//! hear the rig load up before you see the heel, and you hear a sail start to shake
//! before you can read a telltale.
//!
//! Implemented as a `Decodable` so it is a real-time generator feeding the audio
//! thread, not a loop being pitch-shifted. The simulation writes a handful of
//! parameters each frame; the audio thread reads them lock-free and keeps running.
//! No loop seams, no transposition artefacts, no latency beyond the device buffer.
//!
//! Four voices:
//!
//! * **Wind** — broadband noise through a two-pole lowpass whose cutoff opens as the
//!   apparent wind builds. Louder *and* brighter, which is what actually happens.
//! * **Rigging** — the tonal whine off the wire. Its pitch comes from the Strouhal
//!   relation for vortex shedding, `f = St·V/d`, with St ≈ 0.2 and a 6 mm shroud: 170
//!   Hz at five metres per second, 330 at ten. Detuned pair, because a single sine
//!   reads as a test tone.
//! * **Water** — bandpassed noise at the hull, with a slow surge on it for the bow
//!   wave. Rises with speed through the water, not over ground.
//! * **Gear** — the sound of the boat being *worked*, which until now it made none of.
//!   Three parts, all of them driven by how fast the lines are actually moving rather
//!   than by which key is down: a click train from the winch pawls, a rasp of rope
//!   through blocks and jammers under it, and a clack for the hard single events — a
//!   clutch thrown, a boom arriving at the end of a gybe. The click rate is the real
//!   instrument here: a winch tells you how fast the sheet is coming in without you
//!   looking at it, and it is the sound every sailor uses to know when to stop.
//! * **Flogging** — gated noise bursts with a hard attack. Canvas cracking, rated by
//!   how badly the sails are luffing.

use std::num::NonZero;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bevy::audio::{AddAudioSource, Decodable, Source};
use bevy::prelude::*;
use sail_core::prelude::*;

use crate::sim::Sim;

const SAMPLE_RATE: u32 = 44_100;
/// Master gain. Deliberately modest — this is an ambience bed, not a soundtrack.
const MASTER: f32 = 1.0;

/// Parameters the simulation writes and the audio thread reads.
///
/// Atomics rather than a mutex: the audio callback must never block, and a torn read
/// of a single gain is inaudible where a missed deadline is a click.
/// The levels start where they should start rather than at the zero an atomic happens
/// to hold, which is the fix for a volume control that did not work.
///
/// It used to read "nought means nobody has set it, so play at full", and that is two
/// different states sharing one value. Dragging the slider to silence stored a nought,
/// which the reader turned back into *full volume* — so the control did the opposite of
/// what it was asked at the bottom of its travel and jumped there from a hundredth. A
/// default belongs in the constructor, where it says what it is, and not in a reader
/// guessing at intent from a sentinel.
impl Default for SeaParams {
    fn default() -> Self {
        let p = Self {
            wind_level: AtomicU32::default(),
            wind_cutoff: AtomicU32::default(),
            rig_level: AtomicU32::default(),
            rig_hz: AtomicU32::default(),
            notes_played: AtomicU32::default(),
            notes_lost: AtomicU32::default(),
            bell: AtomicU32::default(),
            shout: AtomicU32::default(),
            voices: Vec::new(),
            groups: [(0, 0); Call::KINDS],
            water_level: AtomicU32::default(),
            sea_surge: AtomicU32::default(),
            sea_slam: AtomicU32::default(),
            water_cutoff: AtomicU32::default(),
            flog_level: AtomicU32::default(),
            flog_hz: AtomicU32::default(),
            rain_level: AtomicU32::default(),
            master: AtomicU32::default(),
            gear_level: AtomicU32::default(),
            gear_hz: AtomicU32::default(),
            clack: AtomicU32::default(),
            alarm: AtomicU32::default(),
            muted: AtomicU32::default(),
            notes: Default::default(),
            tick: AtomicU32::default(),
            shape: AtomicU32::default(),
            music_level: AtomicU32::default(),
            sea_level: AtomicU32::default(),
            boat_level: AtomicU32::default(),
        };
        store(&p.master, 1.0);
        store(&p.music_level, 0.45);
        store(&p.sea_level, 1.0);
        store(&p.boat_level, 1.0);
        p
    }
}

#[derive(Debug)]
pub struct SeaParams {
    wind_level: AtomicU32,
    wind_cutoff: AtomicU32,
    rig_level: AtomicU32,
    rig_hz: AtomicU32,
    /// Notes that found a slot, and notes that did not. Counted because "I cannot hear
    /// the music" has three quite different causes — nothing composed, something
    /// composed and dropped, something composed and drowned — and a counter tells them
    /// apart without anybody having to guess.
    notes_played: AtomicU32,
    notes_lost: AtomicU32,
    /// Strikes of the ship's bell waiting to be rung.
    bell: AtomicU32,
    /// A call from the deck waiting to be made: `(generation << 8) | index`.
    ///
    /// The generation is what makes two of the same call in a row two calls. A plain
    /// index could not say "again" — the stream would read the same number and have no
    /// way of telling a repeat from a value nobody had cleared.
    shout: AtomicU32,
    /// The recorded calls themselves, loaded once before this is shared and never
    /// written again — so the audio thread reads them without a lock and without an
    /// atomic, which is the only way a sample buffer belongs in a callback.
    voices: Vec<Vec<f32>>,
    /// Where each kind of call starts and ends in that list.
    ///
    /// Grouped by the **name of the file**, which is the one thing a person holding a
    /// microphone can control without being told about data structures: `warn-03.wav` is
    /// a warning. A recording whose name says nothing goes in the general pool and is
    /// used for anything, which is what you want from the first thirty files somebody
    /// records and not what you want from the three hundredth.
    groups: [(usize, usize); Call::KINDS],
    water_level: AtomicU32,
    water_cutoff: AtomicU32,
    /// A wave arriving: how hard she was hit, 0 to 1, taken rather than read.
    ///
    /// **A wave against a hull is an event with a shape**, and the swelling hiss on its
    /// own has no front to it. What you hear from inside a boat is a *blow* and then the
    /// water running off her: the impact is low and over in a moment, the run-off is
    /// bright and takes a second to die away. Modelling only the second of those is the
    /// difference between a sea and a tap running.
    ///
    /// One shot per wave rather than a level, because that is what it is — and because
    /// a level cannot have an attack.
    sea_slam: AtomicU32,
    /// How hard the sea is working on the hull just now, 0 to 1.
    ///
    /// **The sound of a wave is an event and the water voice had none in it.** What was
    /// there was a steady hiss at the speed through the water with a half-hertz wobble
    /// on top, and the wobble was a number somebody picked: it did not know where the
    /// waves were, so in a seaway you heard an even rushing that had nothing to do with
    /// the water going past the window. A boat in a sea does not sound like that. Each
    /// one comes up, works along her, and goes — loud, then nothing, then the next.
    ///
    /// This is that, taken from her own motion rather than from a wave model the mixer
    /// would need a second copy of: the vertical rate of the hull, which is what a sea
    /// does to a boat and is zero in flat water. Which is the other half of it — in
    /// flat water the voice goes back to being a steady hiss, because that is what
    /// flat water sounds like.
    sea_surge: AtomicU32,
    flog_level: AtomicU32,
    flog_hz: AtomicU32,
    rain_level: AtomicU32,
    /// What the person listening asked for, 0 to 1. Separate from every other level in
    /// here, all of which are the boat telling the mix what it is doing — this one is
    /// the only number in the audio path that is nobody's measurement.
    master: AtomicU32,
    /// Rope and pawls: how hard it is being worked, and how fast the pawls are going.
    gear_level: AtomicU32,
    gear_hz: AtomicU32,
    /// A one-shot. The app sets it; the audio thread takes it and leaves zero, so a
    /// single event makes a single sound however the two threads happen to interleave.
    clack: AtomicU32,
    /// The autopilot's alarm, taken the same way.
    alarm: AtomicU32,
    muted: AtomicU32,
    /// A ring of note events, written by the composer in `music.rs`.
    ///
    /// Frequency and gain per voice and nothing else — no sequencer on the audio thread,
    /// no note queue, no timing to get wrong across a thread boundary. The composer runs
    /// in the world at frame rate and *places* notes; sixty hertz is sixteen milliseconds
    /// of placement error against a quaver of two hundred and fifty, which is tighter
    /// than a session fiddler and far simpler than the alternative.
    ///
    /// Why generate instead of play a file: the whole game is one executable, a sound
    /// font is tens of megabytes, and a shanty recording carries rights of its own even
    /// when the song is three hundred years old and free. What a generator also gets —
    /// which no recording can — is music that is *about* the weather it is playing over.
    /// Eight slots, each holding a packed note until the audio thread takes it. Taken
    /// the way the clack is — swapping zero in — so an event fires exactly once however
    /// the two threads interleave, and a slot that is still full is a slot the composer
    /// skips rather than a note it blocks for.
    ///
    /// Eight because a piano chord is four or five notes and the ones before it are
    /// still ringing. This is polyphony, which a piano needs and three fixed voices
    /// could not give: a piano note is not held and released, it is *struck* and then
    /// left to die in its own time, and several are always dying at once.
    notes: [AtomicU32; 8],
    /// A hard tick on the beat, for the ink drawing's sake.
    tick: AtomicU32,
    /// Nought for soft triangles, one for the pulse waves of a cartridge.
    shape: AtomicU32,
    music_level: AtomicU32,
    /// The mix, in the three groups somebody actually wants separately.
    ///
    /// Not one slider per voice — nobody wants to balance a flogging genoa against a
    /// winch. Three: the **sea**, which is the wind and the water and the rain and is
    /// what this game sounds like; the **boat**, which is her rigging and her gear and
    /// her sails making a noise; and the **music** over both. The alarm stays outside
    /// all of them, because a warning that can be mixed away is not a warning.
    sea_level: AtomicU32,
    boat_level: AtomicU32,
}

fn store(slot: &AtomicU32, v: f32) {
    slot.store(v.to_bits(), Ordering::Relaxed);
}

fn load(slot: &AtomicU32) -> f32 {
    f32::from_bits(slot.load(Ordering::Relaxed))
}

impl SeaParams {
    pub fn set_master(&self, level: f32) {
        store(&self.master, level.clamp(0.0, 1.0));
    }

    pub fn master(&self) -> f32 {
        load(&self.master)
    }

    /// Fire a one-shot clack: a clutch, a boom, a winch handle dropped on the sole.
    /// `strength` is how hard, 0 to 1.
    pub fn clack(&self, strength: f32) {
        // The loudest pending one wins rather than the last, so a gybe does not get
        // swallowed by a clutch thrown in the same frame.
        let pending = load(&self.clack);
        if strength > pending {
            store(&self.clack, strength);
        }
    }

    /// The pilot has given up. Three beeps, which is what a tiller pilot does when it
    /// loses the course — and the reason it does it is that the person it is steering
    /// for is usually below, making tea, with no idea that the boat is now steering
    /// herself. A dropout you have to be watching the screen to notice is a dropout
    /// that happens to you rather than one you are told about.
    pub fn alarm(&self) {
        store(&self.alarm, 1.0);
    }

    /// Strike a note: a frequency in hertz and how hard, 0 to 1.
    ///
    /// Returns whether there was room. There usually is; when there is not, the note is
    /// dropped, which is the right failure — a dropped note is a thinner chord and a
    /// blocked audio thread is a click.
    pub fn strike(&self, hz: f32, velocity: f32) -> bool {
        self.strike_as(hz, velocity, 0.0, Voice::Struck)
    }

    /// The same, in one of the three colours the band has.
    ///
    /// This is what was missing when the music was reported as changing too subtly to
    /// hear. Every part of it — melody, bass, pedal, the chord under a storm — was coming
    /// out of one struck string, so a change of *language* arrived as the same sound
    /// playing different notes, and a listener who is steering a boat is not going to
    /// catch that. An orchestra does not tell you the scene has changed by moving the
    /// tune; it tells you by bringing in a different section, and that is audible across
    /// a room with the kettle on.
    pub fn strike_voiced(&self, hz: f32, velocity: f32, damp: f32, voice: Voice) -> bool {
        self.strike_as(hz, velocity, damp, voice)
    }

    /// The same, told how soon to let go of it.
    ///
    /// Nought is a note left to ring, which is what a piano does and what a long line
    /// wants. One is a note stopped almost as soon as it is made — and that is not a
    /// lesser version of the first, it is a different way of singing.
    ///
    /// Which this music needs, because the idiom it is in is *sung*. A shanty is a line
    /// of separate words with air between them; the notes do not run into each other,
    /// and a crew of twenty breathing together is as much of the sound as the pitches
    /// are. Played with a piano's sustain the same notes blur into a wash and the thing
    /// stops being a song and becomes a texture. Reported from the deck as wanting a
    /// little silence between the sounds, which is exactly what a singer does without
    /// being asked.
    pub fn strike_damped(&self, hz: f32, velocity: f32, damp: f32) -> bool {
        self.strike_as(hz, velocity, damp, Voice::Struck)
    }

    fn strike_as(&self, hz: f32, velocity: f32, damp: f32, voice: Voice) -> bool {
        if hz < 20.0 || hz > 8000.0 || velocity <= 0.0 {
            return false;
        }
        // Frequency, velocity, how soon to let go and which voice, packed into one
        // atomic so a note is never half-written: fourteen bits of whole hertz reaches
        // past the top of the keyboard, seven of velocity is finer than a hand, six of
        // damping is finer than the ear, and two name the voice.
        let packed = ((hz.round() as u32).min(0x3FFF) << 15)
            | (((velocity.clamp(0.0, 1.0) * 127.0) as u32).min(127) << 8)
            | (((damp.clamp(0.0, 1.0) * 63.0) as u32).min(63) << 2)
            | (voice as u32 & 0x3);
        for slot in &self.notes {
            if slot
                .compare_exchange(0, packed, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                self.notes_played.fetch_add(1, Ordering::Relaxed);
                return true;
            }
        }
        self.notes_lost.fetch_add(1, Ordering::Relaxed);
        false
    }

    /// Strike the bell.
    ///
    /// Two for somebody arriving and one for somebody leaving, which is not a convention
    /// borrowed from anywhere — a ship's bell is struck in pairs for the watch — but it
    /// is the right shape: two is an event and one is a note being made. What matters is
    /// that it is a *bell* and not a notification sound. A boat has one, it is the thing
    /// aboard that is for telling other people something, and it carries across water
    /// without being loud.
    pub fn ring(&self, strikes: u8) {
        self.bell.store(strikes.min(3) as u32, Ordering::Relaxed);
    }

    /// Somebody on deck says something.
    ///
    /// **The words are on the console and this is not carrying them.** What a call across
    /// a deck carries is who, from where, and how urgently — the content is already in
    /// front of the player in writing, and a voice repeating it would be a voice reading
    /// out a caption. So this takes an index into whatever was recorded and nothing else.
    ///
    /// Mixed inside the stream rather than played as a sound of its own, which is the
    /// whole point: the question a voice has to answer here is whether it carries
    /// *through* thirty knots of synthesised wind, and a one-shot played beside the mix
    /// would answer a different one.
    pub fn shout(&self, kind: Call, pick: u32) {
        let (from, to) = self.groups[kind as usize];
        if to <= from {
            return;
        }
        let i = (from + (pick as usize) % (to - from)) as u32;
        // Ten bits for the index — a thousand recordings is more than anybody will make
        // in an evening — and the rest a generation, so the same call twice is twice.
        let gen = (self.shout.load(Ordering::Relaxed) >> 10).wrapping_add(1) & 0x3f_ffff;
        self.shout.store((gen << 10) | i, Ordering::Relaxed);
    }

    /// How many notes have been asked for and how many were dropped, since the
    /// beginning. The panel takes the difference between two readings.
    pub fn notes(&self) -> (u32, u32) {
        (
            self.notes_played.load(Ordering::Relaxed),
            self.notes_lost.load(Ordering::Relaxed),
        )
    }

    /// A single tick on the beat, taken the same way as the clack.
    pub fn tick(&self, strength: f32) {
        store(&self.tick, strength.clamp(0.0, 1.0));
    }

    /// Nought is a soft triangle, one the pulse of a cartridge.
    pub fn set_shape(&self, shape: f32) {
        store(&self.shape, shape.clamp(0.0, 1.0));
    }

    pub fn set_music(&self, level: f32) {
        store(&self.music_level, level.clamp(0.0, 1.0));
    }

    pub fn set_sea(&self, level: f32) {
        store(&self.sea_level, level.clamp(0.0, 1.0));
    }

    pub fn set_boat(&self, level: f32) {
        store(&self.boat_level, level.clamp(0.0, 1.0));
    }

    pub fn set_muted(&self, muted: bool) {
        self.muted
            .store(if muted { 1 } else { 0 }, Ordering::Relaxed);
    }

    pub fn muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed) != 0
    }
}

/// The audio asset. Holds only the shared parameter block; all the state lives in the
/// stream, which is created once when playback starts.
#[derive(Asset, TypePath, Clone)]
pub struct SeaVoice {
    pub params: Arc<SeaParams>,
}

impl Decodable for SeaVoice {
    type Decoder = SeaStream;

    fn decoder(&self) -> Self::Decoder {
        SeaStream::new(self.params.clone())
    }
}

/// The three colours the band has, and the reason it has three.
///
/// One voice doing everything is why a change of language was arriving inaudibly. These
/// are not three instruments in the sampled sense — they are the same six partials shaped
/// three ways — but they are *categorically* different to the ear, which is the only
/// thing that matters here:
///
/// * **Struck** is the piano: an instant attack and a decay from the first sample.
///   Everything with a tune in it.
/// * **Bowed** swells. It takes a fifth of a second to arrive and then holds, which is
///   the one thing a struck string cannot do, and it is what a film score is made of. A
///   held note under a scene is a thing being waited for; a struck one is an event.
/// * **Ground** is the bass: dark, slow to die, with the upper partials taken out so it
///   sits under everything without competing with the tune.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Voice {
    Struck = 0,
    Bowed = 1,
    Ground = 2,
}

impl From<u32> for Voice {
    fn from(v: u32) -> Self {
        match v {
            1 => Voice::Bowed,
            2 => Voice::Ground,
            _ => Voice::Struck,
        }
    }
}

/// Handle the rest of the app uses to talk to the audio thread.
#[derive(Resource, Clone)]
pub struct SoundHandle(pub Arc<SeaParams>);

/// One-pole lowpass. Cheap, stable, and the right shape for wind: noise does not need
/// a steep filter, it needs the cutoff to move.
#[derive(Default)]
struct OnePole {
    state: f32,
}

impl OnePole {
    #[inline]
    fn run(&mut self, input: f32, cutoff_hz: f32) -> f32 {
        // Standard one-pole coefficient for a given cutoff at this sample rate.
        let k =
            (1.0 - (-std::f32::consts::TAU * cutoff_hz / SAMPLE_RATE as f32).exp()).clamp(0.0, 1.0);
        self.state += k * (input - self.state);
        self.state
    }
}

/// The generator. Runs on the audio thread and never allocates.
/// A sine, from a table, because the audio callback has a deadline.
///
/// Twelve voices of six partials is seventy-two sines a sample, and at forty-four
/// thousand samples a second that is three million two hundred thousand transcendental
/// calls — in a callback that must finish inside the buffer it was handed or the device
/// plays whatever was in memory. What that sounds like is a hole, and a hole in a
/// continuous noise is the thing you notice: the sea stuttering was reported from the
/// deck as the music interrupting it, and the music was interrupting it, by taking its
/// time.
///
/// A thousand entries is a tenth of a degree of phase — far finer than the ear, and with
/// linear interpolation between them the error is below anything a sixteen-bit converter
/// can represent. One table lookup, one multiply, one add.
const SINE_BITS: usize = 10;
const SINE_LEN: usize = 1 << SINE_BITS;

fn sine_table() -> &'static [f32; SINE_LEN] {
    static TABLE: std::sync::OnceLock<[f32; SINE_LEN]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = [0.0; SINE_LEN];
        for (i, v) in t.iter_mut().enumerate() {
            *v = (std::f32::consts::TAU * i as f32 / SINE_LEN as f32).sin();
        }
        t
    })
}

/// Phase in turns, 0 to 1.
#[inline]
fn fast_sin(phase: f32) -> f32 {
    let t = sine_table();
    let x = phase * SINE_LEN as f32;
    let i = x as usize & (SINE_LEN - 1);
    let j = (i + 1) & (SINE_LEN - 1);
    let f = x - x.floor();
    t[i] + (t[j] - t[i]) * f
}

/// One note, struck and then left alone.
///
/// A piano string is not a sine and not a sawtooth. What makes the sound recognisable is
/// three things that are all cheap:
///
/// * **Inharmonic partials.** A real string is stiff, so its nth partial does not sit at
///   n times the fundamental but at `n sqrt(1 + B n^2)` — a little sharp, more so the
///   higher you go. B is about half a thousandth in the middle of the keyboard and larger
///   for the short thick strings at the bottom. That stretch is the single most piano
///   thing about a piano: take it out and the same partials sound like an organ.
/// * **A decay that depends on pitch.** Bass strings ring for twenty seconds, the top
///   octave is gone in one. Give every note the same decay and it sounds like a synth
///   pad whatever else is right.
/// * **Partials that die at different rates.** The top of the spectrum goes first, which
///   is why a piano note turns from bright to round as it rings rather than just getting
///   quieter.
///
/// Six partials is enough. The ear stops being able to pick them out above that, and the
/// cost is six sines a sample per voice.
#[derive(Clone, Copy, Default)]
struct Plucked {
    hz: f32,
    phase: [f32; 6],
    amp: [f32; 6],
    /// Each partial's own frequency, stretched by the string's stiffness and worked out
    /// once — the square root that does the stretching has no business being in a loop
    /// that runs six times a sample for every note that is ringing.
    freq: [f32; 6],
    /// What each partial's amplitude is multiplied by every sample.
    fade: [f32; 6],
    /// A short noise burst for the hammer, which is most of the attack.
    knock: f32,
    /// How far open the note is, and how fast it opens. Nought to one, one multiply a
    /// sample. A struck string opens in two milliseconds, which is also what keeps a
    /// note from starting with a click; a bowed one takes a fifth of a second, and that
    /// single number is most of what makes it a different instrument.
    open: f32,
    open_rate: f32,
    /// Nought for a struck string, one for a chip's square wave.
    shape: f32,
}

impl Plucked {
    fn strike(
        &mut self,
        hz: f32,
        velocity: f32,
        shape: f32,
        sample_rate: f32,
        damp: f32,
        voice: Voice,
    ) {
        self.hz = hz;
        self.shape = shape;
        // A bow has no hammer, and that absence is half of why it does not sound struck.
        self.knock = match voice {
            Voice::Struck => velocity * 0.5,
            Voice::Ground => velocity * 0.35,
            Voice::Bowed => 0.0,
        };
        self.open = 0.0;
        self.open_rate = match voice {
            // A fifth of a second to arrive. Long enough to be heard as a swell rather
            // than a soft attack, short enough that it is in time with the bar it was
            // asked for on.
            Voice::Bowed => 1.0 / (0.20 * sample_rate),
            _ => 1.0 / (0.002 * sample_rate),
        };
        // How quickly this note dies: about eight seconds at the bottom of the keyboard
        // and one at the top, which is roughly what a real one does.
        let natural = match voice {
            // A struck string's length is a property of the string: eight seconds at the
            // bottom of the keyboard and one at the top.
            Voice::Struck => 0.55 + 6.0 * (1.0 - (hz / 900.0).clamp(0.0, 1.0)).powi(2),
            // A bow's is a property of the *player*, so it is the same at every pitch and
            // it is chosen by the music: long enough to hold under the bar it was struck
            // on and the next one, short enough that eight of them do not pile up into a
            // wash. The first version made it four times the string's own, which at the
            // bottom of the register was sixteen seconds — a pad that never stops is not
            // a pad, it is a drone, and the voice pool is twelve.
            Voice::Bowed => 1.6,
            Voice::Ground => 1.2,
        };
        // Damped, the note is stopped rather than left to die.
        //
        // Geometrically rather than by subtracting a fraction, because the range is
        // enormous: a note in the middle of the melody's register rings for seven seconds
        // and the gap it has to fit inside is a third of one. The first version of this
        // took a sixth off the natural decay and I called it a damped note in the
        // comment; measured, it still masked the following note for two and a quarter
        // seconds, which is seven quavers. Proportional thinking was the whole mistake —
        // what matters is the absolute length of the tail against the absolute length of
        // a quaver, and only a measurement says whether that is met.
        //
        // So full damping is forty-three milliseconds of ring whatever the pitch, and
        // everything between is the geometric mean of that and the string's own.
        const STOPPED: f32 = 0.043;
        let base = natural * (STOPPED / natural).powf(damp.clamp(0.0, 1.0));
        // Stiffness. More at the bottom, where the strings are short for their pitch.
        let b = 0.0004 + 0.0022 * (1.0 - (hz / 500.0).clamp(0.0, 1.0));
        for n in 0..6 {
            let partial = n as f32 + 1.0;
            self.phase[n] = 0.0;
            // Falling roughly as one over n, and harder as the note is played softer —
            // a quiet piano note is not a loud one turned down, it is rounder.
            let brightness = 0.35 + 0.65 * velocity;
            // How fast the spectrum falls away, which *is* the colour. A piano at 1.25,
            // a bow brighter and more even because a sawtooth is what a bow makes, and
            // the ground much darker so it never argues with the tune.
            let rolloff = match voice {
                Voice::Struck => 1.25 + 0.9 * (1.0 - brightness),
                Voice::Bowed => 0.95,
                Voice::Ground => 2.1,
            };
            self.amp[n] = 1.0 / partial.powf(rolloff);
            // The stretched partial, and its own decay — higher partials die first.
            // Per sample, so the inner loop is a multiply.
            // Higher partials die first on a struck string. Under a bow they do not —
            // the bow keeps feeding them — so the spectrum stays put while the note lasts,
            // and that steadiness is the other half of sounding sustained.
            let spread = match voice {
                Voice::Bowed => 0.08,
                _ => 0.55,
            };
            let per_second = (1.0 + spread * n as f32) / base;
            self.fade[n] = (-per_second / sample_rate).exp();
            self.freq[n] = hz * partial * (1.0 + b * partial * partial).sqrt();
        }
        // Normalised so that one note at full velocity peaks at one, whatever the
        // brightness did to the partials.
        //
        // Without this a note's loudness was the *sum* of six partials each starting at
        // the velocity — about twice what was intended — and five of those ringing at
        // once put ten through a mix that soft-clips at one. The clip is a limiter, and a
        // limiter fed something that loud ducks everything else under it: reported from
        // the deck as the music interrupting the sea, which is exactly what it was doing.
        // The sea is the thing this game sounds like; the music plays over it and not
        // instead of it.
        let total: f32 = self.amp.iter().sum();
        if total > 0.0 {
            for a in &mut self.amp {
                *a *= velocity / total;
            }
        }
    }

    fn next(&mut self, dt: f32, noise: f32) -> f32 {
        if self.hz < 20.0 {
            return 0.0;
        }
        if self.open < 1.0 {
            self.open = (self.open + self.open_rate).min(1.0);
        }
        let mut out = 0.0;
        let mut alive = false;
        for n in 0..6 {
            if self.amp[n] < 1e-5 {
                continue;
            }
            alive = true;
            self.phase[n] += self.freq[n] * dt;
            self.phase[n] -= self.phase[n].floor();
            let p = self.phase[n];
            let sine = fast_sin(p);
            // The cartridge: a quarter-duty pulse on the fundamental only, because a
            // chip has one oscillator per voice and that is the sound.
            let chip = if n == 0 {
                if p < 0.25 {
                    1.0
                } else {
                    -1.0
                }
            } else {
                0.0
            };
            out += (sine + (chip - sine) * self.shape) * self.amp[n];
            // The decay is a multiply by a constant, worked out when the note was
            // struck rather than exponentiated per partial per sample. Another seventy
            // transcendentals a sample saved, and exactly the same curve.
            self.amp[n] *= self.fade[n];
        }
        if self.knock > 1e-4 {
            alive = true;
            // The hammer. Three milliseconds of filtered noise, which is what the ear
            // hears as a key going down rather than a tone starting.
            out += noise * self.knock * 0.5;
            self.knock *= 1.0 - (dt * 320.0).min(1.0);
        }
        if !alive {
            self.hz = 0.0;
        }
        out * self.open
    }
}

pub struct SeaStream {
    params: Arc<SeaParams>,
    rng: u32,
    wind_a: OnePole,
    wind_b: OnePole,
    water_lp: OnePole,
    water_hp: OnePole,
    /// The blow of a wave, which is low and has no pitch in it.
    slam_lp: OnePole,
    /// The two decays whose difference is the envelope of one: see the water voice.
    slam_slow: f32,
    slam_fast: f32,
    rain_lp: OnePole,
    rope_lp: OnePole,
    /// Canvas has a body to it. See the flogging block.
    flog_lp: OnePole,
    pawl_phase: f32,
    pawl_env: f32,
    pawl_hz: f32,
    pawl_ring: f32,
    clack_env: f32,
    clack_phase: f32,
    alarm_left: u8,
    alarm_clock: f32,
    alarm_phase: f32,
    smooth: Smoothed,
    rig_phase: f32,
    rig_phase2: f32,
    /// The whine's pitch half a second ago, and whether the wire is holding its note.
    rig_steady: f32,
    rig_gate: f32,
    flog_phase: f32,
    /// Twelve voices, each one a note that has been struck and is dying.
    voices: [Plucked; 12],
    poll: u32,
    tick_env: f32,
    tick_phase: f32,
    flog_env: f32,
    /// Strikes still to come, the wait until the next one, and the five partials of the
    /// one that is ringing.
    bell_left: u8,
    /// Which call is being made and how far through it we are.
    shout_at: Option<(usize, usize)>,
    /// The last generation seen, so a repeat of the same call still counts as one.
    shout_seen: u32,
    bell_wait: f32,
    bell_amp: [f32; 5],
    bell_phase: [f32; 5],
}

/// Per-sample slew on every parameter.
///
/// Without this, a parameter that jumps between frames steps the gain mid-waveform
/// and you hear a click on every change — which, at sixty frames a second in a gust,
/// is a buzz rather than wind.
#[derive(Default)]
struct Smoothed {
    wind: f32,
    wind_cutoff: f32,
    rig: f32,
    rig_hz: f32,
    water: f32,
    water_cutoff: f32,
    /// The sea working on the hull, smoothed over a wave rather than over a frame.
    sea_surge: f32,
    flog: f32,
    rain: f32,
    gear: f32,
    mute: f32,
}

impl Smoothed {
    #[inline]
    fn towards(current: &mut f32, target: f32, rate: f32) {
        *current += (target - *current) * rate;
    }
}

impl SeaStream {
    fn new(params: Arc<SeaParams>) -> Self {
        Self {
            voices: [Plucked::default(); 12],
            poll: 0,
            tick_env: 0.0,
            tick_phase: 0.0,
            bell_left: 0,
            shout_at: None,
            shout_seen: 0,
            bell_wait: 0.0,
            bell_amp: [0.0; 5],
            bell_phase: [0.0; 5],
            params,
            rng: 0x2545_F491,
            wind_a: OnePole::default(),
            wind_b: OnePole::default(),
            water_lp: OnePole::default(),
            slam_lp: OnePole::default(),
            slam_slow: 0.0,
            slam_fast: 0.0,
            water_hp: OnePole::default(),
            rain_lp: OnePole::default(),
            rope_lp: OnePole::default(),
            flog_lp: OnePole::default(),
            pawl_phase: 0.0,
            pawl_env: 0.0,
            pawl_hz: 2_000.0,
            pawl_ring: 0.0,
            clack_env: 0.0,
            clack_phase: 0.0,
            alarm_left: 0,
            alarm_clock: 0.0,
            alarm_phase: 0.0,
            smooth: Smoothed {
                wind_cutoff: 400.0,
                water_cutoff: 500.0,
                rig_hz: 200.0,
                mute: 1.0,
                ..Default::default()
            },
            rig_phase: 0.0,
            rig_phase2: 0.0,
            rig_steady: 200.0,
            rig_gate: 1.0,
            flog_phase: 0.0,
            flog_env: 0.0,
        }
    }

    /// xorshift, inline and allocation-free. White noise in [-1, 1].
    #[inline]
    fn noise(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

impl Iterator for SeaStream {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        // Read the whole parameter block up front, into locals. Holding a borrow of
        // `self.params` across the body conflicts with the noise generator, and
        // cloning the Arc per sample would bump an atomic refcount inside the audio
        // callback for nothing.
        let (
            t_wind,
            t_wind_cut,
            t_rig,
            t_rig_hz,
            t_water,
            t_water_cut,
            t_surge,
            slam,
            t_flog,
            flog_hz,
            t_rain,
            t_master,
            t_gear,
            gear_hz,
            clack,
            alarm,
            t_mute,
            t_shape,
            t_music,
            t_sea,
            t_boat,
        ) = {
            let p = &self.params;
            (
                load(&p.wind_level),
                load(&p.wind_cutoff),
                load(&p.rig_level),
                load(&p.rig_hz),
                load(&p.water_level),
                load(&p.water_cutoff),
                load(&p.sea_surge),
                // Taken, not read: swapping zero in is what makes a one-shot fire once.
                f32::from_bits(p.sea_slam.swap(0, Ordering::Relaxed)),
                load(&p.flog_level),
                load(&p.flog_hz),
                load(&p.rain_level),
                p.master(),
                load(&p.gear_level),
                load(&p.gear_hz),
                // Taken, not read: swapping zero in is what makes a one-shot fire once.
                f32::from_bits(p.clack.swap(0, Ordering::Relaxed)),
                f32::from_bits(p.alarm.swap(0, Ordering::Relaxed)),
                if p.muted() { 0.0 } else { 1.0 },
                load(&p.shape),
                load(&p.music_level),
                load(&p.sea_level),
                load(&p.boat_level),
            )
        };

        // Slew everything. 0.0008 per sample is about a 30 ms time constant: fast
        // enough to feel immediate, slow enough that nothing steps. Without this a
        // parameter that jumps between frames clicks on every change, which at sixty
        // frames a second in a gust is a buzz rather than wind.
        const SLEW: f32 = 0.0008;
        Smoothed::towards(&mut self.smooth.wind, t_wind, SLEW);
        Smoothed::towards(&mut self.smooth.wind_cutoff, t_wind_cut, SLEW);
        Smoothed::towards(&mut self.smooth.rig, t_rig, SLEW);
        Smoothed::towards(&mut self.smooth.rig_hz, t_rig_hz, SLEW);
        Smoothed::towards(&mut self.smooth.water, t_water, SLEW);
        Smoothed::towards(&mut self.smooth.water_cutoff, t_water_cut, SLEW);
        // **Slower than everything else in here, on purpose.** Thirty milliseconds is
        // the time constant for a parameter that must not step; a wave is a second and
        // a half of swelling and dying away, and smoothing it at thirty milliseconds
        // would follow the hull's own jitter and give you a rattle rather than a sea.
        // Two hundred milliseconds is the rise and fall of water against a hull.
        Smoothed::towards(&mut self.smooth.sea_surge, t_surge, 0.00011);
        Smoothed::towards(&mut self.smooth.flog, t_flog, SLEW);
        Smoothed::towards(&mut self.smooth.rain, t_rain, SLEW);
        Smoothed::towards(&mut self.smooth.gear, t_gear, SLEW);
        Smoothed::towards(&mut self.smooth.mute, t_mute, 0.0004);

        let dt = 1.0 / SAMPLE_RATE as f32;

        // --- Wind: noise, lowpassed twice, cutoff opening with strength -----------
        let n = self.noise();
        let w = self.wind_a.run(n, self.smooth.wind_cutoff);
        let wind = self.wind_b.run(w, self.smooth.wind_cutoff) * self.smooth.wind * 2.2;

        // --- Rigging whine: a detuned pair, so it reads as wire and not a tone -----
        //
        // And it only sings while the wind is holding still, which is the difference
        // between wire and a theremin.
        //
        // The pitch comes from vortex shedding, and shedding has to *lock on*: the
        // eddies have to fall off the wire in step with each other, and they cannot do
        // that in a flow that is changing underneath them. So a shroud in a rising gust
        // does not slide up the scale — it lets go, goes quiet, and finds a new note
        // when the gust steadies. Anybody who has sat in a cockpit in twenty-five knots
        // has heard the rig pick up and drop out, over and over, and never once heard it
        // glissando.
        //
        // Measuring that costs almost nothing: a second follower of the same pitch, slow
        // enough to be half a second behind, and the gap between the two *is* how much
        // the note has moved lately. No differentiating, so no amplified jitter.
        Smoothed::towards(&mut self.rig_steady, self.smooth.rig_hz, 0.00004);
        let moved = (self.smooth.rig_hz - self.rig_steady).abs() / self.rig_steady.max(60.0);
        let holding = 1.0 / (1.0 + (moved / 0.04).powi(2));
        // Asymmetric, because the two halves are not the same event: the note breaks the
        // moment the flow does, and then the wire has to find it again, which takes a
        // moment and is most of what makes this sound alive.
        let rate = if holding < self.rig_gate {
            0.0015
        } else {
            0.00018
        };
        Smoothed::towards(&mut self.rig_gate, holding, rate);
        self.rig_phase = (self.rig_phase + self.smooth.rig_hz * dt).fract();
        self.rig_phase2 = (self.rig_phase2 + self.smooth.rig_hz * 1.013 * dt).fract();
        let rig = (fast_sin(self.rig_phase) * 0.6 + fast_sin(self.rig_phase2) * 0.4)
            * self.smooth.rig
            * self.rig_gate
            * 0.14;

        // --- Water at the hull: bandpassed noise, and the sea on top of it --------
        //
        // The steady part is the water going past her, which is a function of speed and
        // nothing else. On top of it the sea itself: each wave that works along her
        // swells the hiss and lets it fall away again, which is how one actually sounds
        // from inside a boat — a rush that arrives and leaves rather than a level that
        // is set. In flat water the second term is nought and the voice is the hiss, as
        // it should be.
        //
        // The little half-hertz wobble that used to be here is gone with it. It was a
        // frequency somebody chose, it never agreed with a single wave on the screen,
        // and what it bought was the *impression* of a sea in a game that now has one.
        // The blow, when one arrives. Two decays subtracted from each other, which is
        // the cheapest envelope there is and the only shape that has a front on it: the
        // fast one climbs out of the slow one in twenty milliseconds and then the slow
        // one is left, dying over about a second. Twenty milliseconds is a hull being
        // hit; a second is the water finding its way back off her.
        if slam > 0.0 {
            self.slam_slow = (self.slam_slow + slam).min(1.6);
            self.slam_fast = (self.slam_fast + slam).min(1.6);
        }
        self.slam_slow *= 0.99993;
        self.slam_fast *= 0.9986;
        let blow = (self.slam_slow - self.slam_fast).max(0.0);
        let surge = 0.74 + 0.9 * self.smooth.sea_surge + 1.5 * blow;
        let n2 = self.noise();
        // Brighter while the water is running off her, which is what the hiss of a wave
        // draining is: the same noise with the top let through.
        let cutoff = self.smooth.water_cutoff + 1_100.0 * blow;
        let low = self.water_lp.run(n2, cutoff);
        let dc = self.water_hp.run(low, 110.0);
        let water = (low - dc) * self.smooth.water * surge * 3.4;
        // And the blow itself, which is not a hiss at all: a solid body of water against
        // a hull is low, broad and short, and it is the sound you feel in your feet
        // rather than hear. Squared, so the big ones are the ones that thump.
        let n3 = self.noise();
        let thump = self.slam_lp.run(n3, 150.0) * blow * blow * 9.0;

        // --- Rain on the deck ------------------------------------------------------
        //
        // Nearly unfiltered noise, and that is the whole character: rain on a coachroof
        // is a hiss with no pitch in it, which is exactly what white noise is and why it
        // is the one sound here that needs no shaping. A gentle lift at the bottom for
        // the drumming on the deck, and nothing else — reach for a filter and it turns
        // into wind.
        let n4 = self.noise();
        let drum = self.rain_lp.run(n4, 180.0);
        let rain = (n4 * 0.72 + drum * 1.6) * self.smooth.rain * 0.5;

        // --- Gear: pawls, rope, and the odd clack ----------------------------------
        //
        // The pawls are a click train. A click is a pawl spring snapping a steel tooth,
        // so it is a noise burst with a metallic ring on it and an envelope measured in
        // milliseconds — and the *rate* is the information: it is the speed the drum is
        // turning, which is the speed the sheet is coming in.
        self.pawl_phase += gear_hz.clamp(0.0, 90.0) * dt;
        if self.pawl_phase >= 1.0 {
            self.pawl_phase -= 1.0;
            self.pawl_env = 1.0;
            // Each tooth a slightly different pitch, because a ratchet is not a tuned
            // instrument and twenty identical clicks a second is a buzzer.
            self.pawl_hz = 1_900.0 + (self.noise() * 420.0).abs();
        }
        self.pawl_env *= 0.9972;
        self.pawl_ring = (self.pawl_ring + self.pawl_hz * dt).fract();
        let tooth =
            (fast_sin(self.pawl_ring) * 0.55 + self.noise() * 0.45) * self.pawl_env * self.pawl_env;

        // Rope through a block and a jammer: a rasp, and it is noise with the top taken
        // off rather than a tone — polyester over anodised aluminium has no pitch.
        let n5 = self.noise();
        let rasp = self.rope_lp.run(n5, 1_100.0) * 0.9;

        // And the clack: a clutch thrown, or a boom arriving. Low, short, and loud
        // enough to make you look up, which is what those two sounds are for.
        if clack > 0.0 {
            self.clack_env = clack.clamp(0.0, 1.0);
            self.clack_phase = 0.0;
        }
        self.clack_env *= 0.9988;
        self.clack_phase = (self.clack_phase + 95.0 * dt).fract();
        let clack_out = (fast_sin(self.clack_phase) * 0.7 + self.noise() * 0.3)
            * self.clack_env
            * self.clack_env
            * 1.45;

        // Loud, and it belongs loud. Everything else in this mix is weather happening
        // around you — wind at 2.2, water at 3.4 — while this is the one voice that is
        // *you*, a hand's width away, and gear a metre from your ear is not background.
        // The pawls were at 0.5 against flogging canvas at 0.9, which had a winch being
        // ground in quieter than a sail shaking two boat-lengths forward.
        let gear = (tooth * 1.15 + rasp * 0.62) * self.smooth.gear + clack_out;

        // --- The pilot's alarm ------------------------------------------------------
        //
        // Three beeps, square-ish and deliberately unpleasant: this is the one sound in
        // the mix that is a *machine* rather than the sea, and it has to cut through a
        // gale because that is the weather in which the pilot gives up. Not filtered,
        // not pretty, and it ignores nothing — a soft alarm is a decoration.
        if alarm > 0.0 {
            self.alarm_left = 3;
            self.alarm_clock = 0.0;
        }
        let mut beep = 0.0;
        if self.alarm_left > 0 {
            self.alarm_clock += dt;
            // A fifth of a second on, a fifth off.
            let on = self.alarm_clock % 0.4 < 0.2;
            if self.alarm_clock >= 0.4 {
                self.alarm_clock -= 0.4;
                self.alarm_left -= 1;
            }
            if on && self.alarm_left > 0 {
                self.alarm_phase = (self.alarm_phase + 2_200.0 * dt).fract();
                // Square, because a piezo is a square, and because a sine disappears
                // into wind noise at the one moment it must not.
                beep = if self.alarm_phase < 0.5 { 0.22 } else { -0.22 };
            }
        }

        // --- Flogging canvas: gated bursts with a hard attack ----------------------
        //
        // **Canvas does not hiss.** This was unfiltered white noise, which is both the
        // wrong sound and the worst possible one to put next to a tune: white noise is
        // the most effective masker there is, so a sail flogging four times a second was
        // laying a broadband blanket over the whole mix. Reported from the deck as the
        // music disappearing and the sea covering everything — and measured afterwards at
        // twenty-three decibels over the sea in a light breeze, which is not a sea
        // covering anything, it is a sail.
        //
        // A sail slatting is a *crack and a body*: the snap of the leech going over, and
        // then the sheet of cloth behind it, which has no top end at all. So the body is
        // the noise with the top taken off, and the crack is a few milliseconds of the
        // raw stuff on the attack only — `env` to the eighth, which is three milliseconds
        // at this decay and gone.
        let flog_hz = flog_hz.clamp(0.5, 14.0);
        self.flog_phase += flog_hz * dt;
        if self.flog_phase >= 1.0 {
            self.flog_phase -= 1.0;
            self.flog_env = 1.0;
        }
        self.flog_env *= 0.9994;
        let n3 = self.noise();
        let body = self.flog_lp.run(n3, 820.0) * 2.1;
        let snap = self.flog_env.powi(8);
        let flog =
            (body * self.flog_env * self.flog_env + n3 * 0.45 * snap) * self.smooth.flog * 0.9;

        // The alarm sits outside the master gain and inside only the mute: it is a
        // warning rather than part of the picture, and a volume turned down because the
        // sea was loud must not be the reason nobody heard the pilot let go.
        // --- The music -------------------------------------------------------------
        //
        // Three voices: a lead, a counter-line under it and a bass. Triangle when the
        // sea is drawn the way a photograph is, pulse when it is drawn in ink — which is
        // the whole of the "cartridge" sound, two waveforms and hard envelopes, and it
        // is right that the picture and the music change together.
        let mut song = 0.0;
        // Take whatever the composer has put down. Checked every thirty-two samples
        // rather than every one: that is less than a millisecond of placement error, far
        // under what anybody can hear, and it keeps eight atomic loads out of the inner
        // loop.
        self.poll += 1;
        if self.poll >= 32 {
            self.poll = 0;
            for slot in &self.params.notes {
                let packed = slot.swap(0, Ordering::Relaxed);
                if packed == 0 {
                    continue;
                }
                let hz = (packed >> 15) as f32;
                let velocity = ((packed >> 8) & 0x7F) as f32 / 127.0;
                let damp = ((packed >> 2) & 0x3F) as f32 / 63.0;
                let voice = Voice::from(packed & 0x3);
                // The quietest voice gets taken, not the next one round.
                //
                // Round robin was right while every note was a struck string that had
                // mostly died by the time its turn came round again. It stopped being
                // right the moment there were bowed notes: a pad held for four seconds is
                // four bars at this tempo, three or four of them are sounding at once in
                // the cinematic and siege treatments, and the round robin would cut one
                // off in the middle — which is not a quiet note disappearing, it is a
                // click, because the amplitudes are reset where the waveform was.
                //
                // Taking the quietest means what gets interrupted is whatever had least
                // left to say. Twelve comparisons per note, against a budget of
                // forty-eight thousand samples a second.
                let mut v = 0;
                let mut least = f32::INFINITY;
                for (i, voice) in self.voices.iter().enumerate() {
                    let loudness = if voice.hz < 20.0 {
                        -1.0
                    } else {
                        voice.amp.iter().sum::<f32>() * voice.open
                    };
                    if loudness < least {
                        least = loudness;
                        v = i;
                    }
                }
                self.voices[v].strike(hz, velocity, t_shape, 1.0 / dt, damp, voice);
            }
        }
        for v in 0..self.voices.len() {
            if self.voices[v].hz >= 20.0 {
                let n = self.noise();
                song += self.voices[v].next(dt, n);
            }
        }
        // On the same scale as every other voice in here, and then some: the sea is the
        // ground and the music is over it. A chord of five peaks around a third of what
        // the wind alone is doing.
        song *= MASTER * 0.85;

        // --- The ship's bell ------------------------------------------------------
        //
        // Five partials, and the ratios are what make it a bell rather than a chime: a
        // hum an octave below the note you think you are hearing, the prime, a *minor
        // third* above it, a fifth, and the nominal an octave up. The minor third is the
        // whole character — it is why a bell sounds solemn where a tubular chime sounds
        // cheerful, and it is the one interval nobody expects to find in something struck
        // with a hammer.
        //
        // They die at different rates, slowest at the bottom, which is why a bell turns
        // from a clang into a hum as it rings rather than just getting quieter.
        let struck = self.params.bell.swap(0, Ordering::Relaxed);
        if struck > 0 {
            self.bell_left = struck as u8;
            self.bell_wait = 0.0;
        }
        if self.bell_left > 0 {
            self.bell_wait -= dt;
            if self.bell_wait <= 0.0 {
                self.bell_left -= 1;
                // Not quite half a second between strikes: a bell struck faster than this
                // reads as an alarm, and slower than it as two bells.
                self.bell_wait = 0.42;
                self.bell_amp = [0.55, 1.0, 0.8, 0.45, 0.65];
                // Phases left where they were, so a second strike on a still-ringing bell
                // beats against the first the way it does on the boat.
            }
        }
        let mut bell = 0.0;
        if self.bell_amp.iter().any(|a| *a > 1e-4) {
            const RATIO: [f32; 5] = [0.5, 1.0, 1.2, 1.5, 2.0];
            const FADE: [f32; 5] = [0.45, 0.9, 1.6, 2.2, 3.4];
            // A small bell on a thirty-six foot boat, not a cathedral.
            const NOTE: f32 = 740.0;
            for n in 0..5 {
                self.bell_phase[n] = (self.bell_phase[n] + NOTE * RATIO[n] * dt).fract();
                bell += fast_sin(self.bell_phase[n]) * self.bell_amp[n];
                self.bell_amp[n] *= (-FADE[n] * dt).exp();
            }
            bell *= 0.16;
        }

        // The beat, which exists so the music has a floor to stand on when the voices
        // thin out — and which is a woodblock rather than a drum, because this is a
        // small boat and not a procession.
        let hit = load(&self.params.tick);
        if hit > 0.0 {
            store(&self.params.tick, 0.0);
            self.tick_env = hit;
        }
        if self.tick_env > 1e-4 {
            // A woodblock, and deliberately not anything on the boat.
            //
            // The first version used the clutch's own clack for this, and that is a real
            // fault rather than a shortcut: a winch pawl rattling as a musical fill means
            // nobody can tell whether somebody is actually grinding. Her sounds are
            // *instruments* in this game — they are how you know what is happening — and
            // borrowing one for the band makes an instrument lie.
            //
            // So the music gets a sound of its own that belongs to the same world and is
            // not any of her gear: a pitched wooden knock, short, with a touch of noise
            // in the attack. Near enough to a block on a deck to sit with everything
            // else, nowhere near the broadband crack of a clutch going on.
            self.tick_env *= 1.0 - (dt * 34.0).min(1.0);
            self.tick_phase += 760.0 * dt;
            self.tick_phase -= self.tick_phase.floor();
            let body = fast_sin(self.tick_phase) * 0.7 + self.noise() * 0.3;
            song += body * self.tick_env * self.tick_env * 0.5;
        }

        // Grouped as somebody would actually want them: the elements, the ship, and the
        // music over both.
        // A fish running takes the floor, and the band gets out of its way.
        //
        // The reel is deliberately past unity gain when line is going out — it is the
        // moment the fishing has, and a scream that is politely level with everything
        // else is not a scream. Measured against the band: at a fast run it came out at
        // -14.4 dBFS with the music at -14.4, which is nought decibels of separation, and
        // the whole mix peaking at 0.92 with the soft clip leaning on it.
        //
        // The answer is not to make the reel quieter. This composer already knows what to
        // do when something matters — in a squall the band drops to one low note and a
        // pulse and lets the sea have the floor — so the same thing happens here: the
        // music ducks under the run and comes back when the fish stops. Only past unity,
        // which by the reel's own design is exactly "line is going out"; winching a sheet
        // in does not duck anything.
        // Deeper than it was, because the gear is quieter than it was. The deck asked
        // for the boat and the sea down against the band, which is right — and it took
        // four decibels off the reel, and the reel's scream is the one sound in this
        // game that must not be level with anything. The answer stays what it was: the
        // band gets out of the way rather than the reel getting louder.
        let duck = 1.0 / (1.0 + 7.0 * (self.smooth.gear - 1.0).max(0.0));
        // A call from the deck. Started on a new generation, and a call already running
        // is cut off by the next one — because two people shouting over each other is
        // what actually happens and is also the only sane thing to do with one play head.
        let asked = self.params.shout.load(Ordering::Relaxed);
        if asked >> 10 != self.shout_seen {
            self.shout_seen = asked >> 10;
            self.shout_at = Some(((asked & 0x3ff) as usize, 0));
        }
        let mut shout = 0.0;
        if let Some((which, at)) = self.shout_at {
            match self.params.voices.get(which).and_then(|v| v.get(at)) {
                Some(sample) => {
                    shout = *sample;
                    self.shout_at = Some((which, at + 1));
                }
                None => self.shout_at = None,
            }
        }

        let mix = ((wind + water + thump + rain) * t_sea
            + (rig + flog + gear + bell) * t_boat
            // **On the boat's bus, not the sea's.** A voice on deck is a thing aboard,
            // so it rides the same fader as the rigging and the winches — turn the boat
            // down and the watch goes quiet with it, which is the behaviour anybody
            // would expect from a control labelled "boat".
            + shout * SHOUT * t_boat)
            * MASTER
            * t_master
            + song * t_music * t_master * duck
            + beep * MASTER * 2.0;
        let mix = mix * self.smooth.mute;
        // Soft clip. A gust plus a flogging genoa can stack up, and a hard clip is a
        // much worse sound than a little compression.
        Some(mix.tanh())
    }
}

impl Source for SeaStream {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> bevy::audio::ChannelCount {
        NonZero::new(1).unwrap()
    }

    fn sample_rate(&self) -> bevy::audio::SampleRate {
        NonZero::new(SAMPLE_RATE).unwrap()
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// Register the generator and start it playing. It never ends; the parameters do all
/// the work from here.
/// Read whatever has been recorded, from a directory named by `SAIL_VOICE`.
///
/// **Outside the repository on purpose.** The calls are somebody's voice, recorded once
/// and not part of the source, and a directory that has to be pointed at cannot be
/// shipped by accident — which matters while the ones being listened to are placeholders
/// cut out of a public-domain reading rather than the real thing.
///
/// Sixteen-bit mono PCM, which is what every recorder on earth writes and what the
/// chopping script produces. Written out by hand rather than brought in with a crate: it
/// is forty lines, the format has not changed since 1991, and a dependency that decodes
/// every audio format in the world is a poor trade for a program that has no audio files
/// in it at all.
fn recorded() -> (Vec<Vec<f32>>, [(usize, usize); Call::KINDS]) {
    let empty = (Vec::new(), [(0, 0); Call::KINDS]);
    let Ok(dir) = crate::knob::var("SAIL_VOICE") else {
        return empty;
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        warn!("voice: cannot read {dir}");
        return empty;
    };
    let mut names: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("wav")))
        .collect();
    // Sorted, so an index means the same call between runs.
    names.sort();

    let kind_of = |path: &std::path::Path| -> Option<usize> {
        let name = path.file_name()?.to_str()?.to_ascii_lowercase();
        Call::all().iter().position(|c| name.starts_with(c.tag()))
    };

    // Loaded in kind order, so each group is one run of the list and the audio thread
    // can pick from it with two numbers instead of a search.
    let mut out: Vec<Vec<f32>> = Vec::new();
    let mut groups = [(0usize, 0usize); Call::KINDS];
    let mut loose = Vec::new();
    for (k, group) in groups.iter_mut().enumerate() {
        let start = out.len();
        for path in &names {
            match kind_of(path) {
                Some(j) if j == k => {}
                Some(_) => continue,
                None => {
                    if k == 0 {
                        loose.push(path.clone());
                    }
                    continue;
                }
            }
            if let Some(samples) = read_call(path) {
                out.push(samples);
            }
        }
        *group = (start, out.len());
    }

    // **Anything unnamed is used for everything.** The first thirty files somebody
    // records are not going to be sorted into four buckets, and a game that said nothing
    // until they were would be a poor way to find out whether recording them was worth
    // it at all. Named files take over a kind as soon as there is one.
    if !loose.is_empty() {
        let start = out.len();
        for path in &loose {
            if let Some(samples) = read_call(path) {
                out.push(samples);
            }
        }
        let general = (start, out.len());
        for (k, group) in groups.iter_mut().enumerate() {
            if group.0 == group.1 {
                *group = general;
                info!(
                    "voice: nothing named {} — using the general pool",
                    Call::all()[k].tag()
                );
            }
        }
    }

    for (k, (a, b)) in groups.iter().enumerate() {
        info!("voice: {} {}", b - a, Call::all()[k].tag());
    }
    info!("voice: {} calls from {dir}", out.len());
    (out, groups)
}

/// One recording, at the rate we play at, or nothing with a reason.
fn read_call(path: &std::path::Path) -> Option<Vec<f32>> {
    let bytes = std::fs::read(path).ok()?;
    match wav_mono(&bytes) {
        Some((samples, rate)) if !samples.is_empty() => Some(at_our_rate(samples, rate)),
        _ => {
            warn!("voice: {} is not 16-bit mono PCM", path.display());
            None
        }
    }
}

/// The samples out of a RIFF/WAVE file, if it is the one shape this reads.
///
/// Chunks are walked rather than assumed to be in a fixed order, because they are not:
/// recorders put `LIST` and `fact` chunks wherever they like, and a reader that expects
/// `data` at byte 44 works on its own test file and nothing else.
/// Whatever was recorded, at the rate this stream actually runs at.
///
/// **Not a constraint to put on the person holding the microphone.** Forty-eight is what
/// a recorder writes by default and forty-four one is what this stream runs at, and a
/// nine percent error is a voice a semitone and a half low and slower than it was said —
/// close enough to sound like a choice and wrong enough to be one. Telling somebody to
/// export at a particular rate is a trap they fall into once; converting here is six
/// lines and they never meet it.
///
/// Linear, which is the part worth being honest about: it is audibly fine for speech
/// and it is not what a resampler should be. If voices ever stop being a placeholder
/// this wants to be done once, offline, by something with a proper filter in it.
fn at_our_rate(samples: Vec<f32>, rate: u32) -> Vec<f32> {
    if rate == SAMPLE_RATE || rate == 0 {
        return samples;
    }
    let ratio = SAMPLE_RATE as f64 / rate as f64;
    let n = (samples.len() as f64 * ratio) as usize;
    (0..n)
        .map(|i| {
            let at = i as f64 / ratio;
            let (a, f) = (at.floor() as usize, (at.fract()) as f32);
            let x = samples.get(a).copied().unwrap_or(0.0);
            let y = samples.get(a + 1).copied().unwrap_or(x);
            x + (y - x) * f
        })
        .collect()
}

fn wav_mono(bytes: &[u8]) -> Option<(Vec<f32>, u32)> {
    let word = |at: usize| -> Option<u32> {
        Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
    };
    let half = |at: usize| -> Option<u16> {
        Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
    };
    if bytes.get(..4)? != b"RIFF" || bytes.get(8..12)? != b"WAVE" {
        return None;
    }
    let (mut at, mut channels, mut bits, mut rate) = (12usize, 0u16, 0u16, 0u32);
    while at + 8 <= bytes.len() {
        let id = bytes.get(at..at + 4)?;
        let size = word(at + 4)? as usize;
        let body = at + 8;
        if id == b"fmt " {
            channels = half(body + 2)?;
            rate = word(body + 4)?;
            bits = half(body + 14)?;
        } else if id == b"data" {
            if channels != 1 || bits != 16 {
                return None;
            }
            let end = (body + size).min(bytes.len());
            return Some((
                bytes[body..end]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|b| i16::from_le_bytes(*b) as f32 / 32768.0)
                    .collect(),
                rate,
            ));
        }
        // Chunks are padded to an even length and the padding is not in the size.
        at = body + size + (size & 1);
    }
    None
}

pub fn setup(mut commands: Commands, mut assets: ResMut<Assets<SeaVoice>>) {
    let (voices, groups) = recorded();
    let params = Arc::new(SeaParams {
        voices,
        groups,
        ..SeaParams::default()
    });
    // Seed the filter targets before the first frame writes them. A default of zero
    // would pull every cutoff towards DC for the first few milliseconds, which is an
    // audible thud on startup.
    store(&params.wind_cutoff, 400.0);
    store(&params.water_cutoff, 500.0);
    store(&params.rig_hz, 200.0);
    store(&params.flog_hz, 4.0);
    let voice = SeaVoice {
        params: params.clone(),
    };
    let handle = assets.add(voice);
    commands.spawn((AudioPlayer(handle), PlaybackSettings::LOOP));
    commands.insert_resource(SoundHandle(params));
}

/// The sea, the rig and the hull, synthesised — and the voice that plays them.
///
/// What this module does in a frame, in one place: see `stages.rs` for why the schedule
/// is named sets rather than one long chain.
/// Shout on a timer, for listening to what was recorded.
///
/// **Because waiting for the watch is not a way to judge a voice.** The calls are made
/// when something happens — a reef goes in, a sail comes down — and in a long gale that
/// is once. One four-hundred-millisecond fragment, once, underneath a force nine, is not
/// an audition; it is a thing you miss and then wonder whether it played at all. That is
/// exactly what happened the first time these were listened to.
///
/// `SAIL_VOICE_TEST=3` makes a call every three seconds, walking through every file in
/// turn and saying which one in the log, so a recording that is wrong can be found by
/// name rather than by guessing.
///
/// **And it pulls the sea down**, because the other half of not hearing anything is
/// thirty knots of wind sitting on top of it. Said out loud rather than done quietly —
/// a harness that changes the mix without mentioning it is a harness that teaches you
/// the wrong thing about the mix. The sliders put it back.
fn rehearse(
    sound: Res<SoundHandle>,
    mut hail: ResMut<crate::hail::Hail>,
    time: Res<Time>,
    mut at: Local<usize>,
    mut since: Local<f32>,
    mut said: Local<bool>,
) {
    let Some(every) = crate::knob::var("SAIL_VOICE_TEST")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|v| *v > 0.0)
    else {
        return;
    };
    if !*said {
        *said = true;
        sound.0.set_sea(0.0);
        info!("voice: rehearsing every {every:.1} s, and the sea is down while this runs");
    }
    *since += time.delta_secs();
    if *since < every {
        return;
    }
    *since = 0.0;
    // Through the same path a real call takes, so this auditions where a call *lands*
    // as well as what it sounds like — which is half of what there is to judge.
    let kind = Call::all()[*at % Call::KINDS];
    info!("voice: {kind:?}");
    hail.call(kind);
    *at += 1;
}

pub fn plugin(app: &mut App) {
    use crate::stages::{Build, Deck};
    app.add_audio_source::<SeaVoice>();
    app.add_systems(Startup, setup.in_set(Build::Scene));
    app.add_systems(Update, (update, rehearse).in_set(Deck::Dress));
}

/// How loud a call from the deck is against everything else.
///
/// A number to be argued with by ear and nothing more: the only honest way to set it is
/// to stand the recording up against thirty knots of synthesised wind and find out
/// whether it carries. That is the entire reason the voice is mixed in here rather than
/// played beside the mix.
const SHOUT: f32 = 0.9;

/// What a call is for.
///
/// **Not what it says** — the words are on the console and a voice repeating them would
/// be a voice reading a caption. This is the *shape* of the utterance: how long, how
/// urgent, and whether it is a person on this deck or somebody hailing across water.
/// Four is enough to cover everything the game currently says and few enough that a
/// recording session is one evening.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Call {
    /// Short, sharp, rising. Something is going wrong now.
    Warn,
    /// Level and then falling. A thing has been done and is being reported.
    Report,
    /// One or two syllables, falling. Heard and understood.
    Aye,
    /// Wordless: a winch, a halyard, a reef being hauled down.
    Effort,
}

impl Call {
    pub const KINDS: usize = 4;

    /// The prefix a file carries to belong to this kind.
    pub fn tag(self) -> &'static str {
        match self {
            Call::Warn => "warn",
            Call::Report => "report",
            Call::Aye => "aye",
            Call::Effort => "effort",
        }
    }

    pub fn all() -> [Call; Self::KINDS] {
        [Call::Warn, Call::Report, Call::Aye, Call::Effort]
    }
}

/// Diameter of a shroud, metres. Sets the pitch of the whine through the Strouhal
/// relation — thinner wire sings higher, which is true and free.
const SHROUD_DIAMETER: f32 = 0.006;
const STROUHAL: f32 = 0.2;

/// Feed the synthesiser from the boat.
/// What the lines were doing last frame, so this one can tell how fast they moved.
#[derive(Default)]
pub struct Worked {
    /// Where the spool was last frame, metres. Kept here beside the sheet positions
    /// because it is the same question asked of a different drum — how fast is it
    /// turning — and answered the same way: by differencing the thing itself rather than
    /// by asking which key is down. The reel turns when the autopilot of a fish turns it.
    line_out: f64,
    lines: [f64; 6],
    boom: f64,
    /// Her vertical rate and its rate of change, for finding the moment a wave hits
    /// rather than the fact that one is about. See the slam above.
    heave_rate: f64,
    accel: f32,
    since_slam: f32,
    started: bool,
}

pub fn update(
    sound: Res<SoundHandle>,
    sim: Res<Sim>,
    rain: Res<crate::rain::Rain>,
    fishing: Res<crate::fishing::Fishing>,
    time: Res<Time>,
    mut was: Local<Worked>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    let p = &sound.0;
    let t = &sim.sim.telemetry;

    if keys.just_pressed(KeyCode::KeyK) {
        p.set_muted(!p.muted());
    }

    // Wind noise power goes roughly as the cube of speed, so amplitude as the 1.5
    // power. Referenced to twenty knots so a fresh breeze is near the top.
    let aws = ms_to_kn(t.aws) as f32;
    let wind = (aws / 20.0).clamp(0.0, 1.6).powf(1.5).min(1.15);
    store(&p.wind_level, wind);
    // Brighter as well as louder: more energy in the small eddies.
    store(&p.wind_cutoff, 320.0 + 95.0 * aws.min(30.0));

    // Rigging whine: vortex shedding, audible once there is enough to shed.
    let v = t.aws as f32;
    store(
        &p.rig_hz,
        (STROUHAL * v / SHROUD_DIAMETER).clamp(60.0, 900.0),
    );
    store(
        &p.rig_level,
        ((aws - 11.0) / 16.0).clamp(0.0, 1.0).powf(1.6),
    );

    // **Water at the hull is two sounds, and until now it was one.**
    //
    // The first is the water she is pushing through, which is a function of speed and of
    // nothing else — hove to in a gale it is silent, and that part was right.
    //
    // The second is the sea, and leaving it out meant a boat lying stopped in a breaking
    // swell made no noise at all while the same boat ghosting across a millpond at six
    // knots was loud. Nobody who has been out in one would believe either. A sea makes
    // its own noise: crests tumbling, water slapping into the topsides, the hiss of a
    // broken one going past — and *none* of that asks how fast you are going.
    //
    // It scales with the height of it and with how much of it is breaking, which is a
    // wind question rather than a wave one: below about eight knots a sea does not
    // whitecap however big the swell, and by twenty-five it is white all over. That is
    // the difference between a long silent swell and the same height of sea in a gale,
    // and it is the one thing that makes the sound of weather rather than the sound of
    // water.
    let stw = ms_to_kn(t.stw) as f32;
    let through_it = (stw / 7.0).clamp(0.0, 1.3).powf(1.4);
    let whitecaps = crate::curve::smoothstep(8.0, 25.0, ms_to_kn(t.tws) as f32);
    let sea = (t.significant_height as f32 / 1.6)
        .clamp(0.0, 1.5)
        .powf(0.9)
        * (0.3 + 0.7 * whitecaps);
    store(&p.water_level, (through_it + 0.85 * sea).clamp(0.0, 1.8));
    store(&p.water_cutoff, 260.0 + 150.0 * stw.min(9.0));

    // **Which waves you hear.**
    //
    // Her vertical rate, with the quiet half of it taken off: below a quarter of a metre
    // a second nothing is said at all, so a long low swell passes in silence the way it
    // does on the water, and what speaks is the sea actually working on the hull.
    //
    // That threshold is the whole of "only some of them", and it is measured rather than
    // felt: in a metre of coastal sea it leaves the voice up **sixty per cent of the
    // time and loud for twenty-four**, which is a sea that arrives and leaves. See
    // `the_sea_voice` below, which is also where the three conditions are printed.
    let lift = (sim.sim.state.heave_rate.abs() as f32 - 0.25).max(0.0) / 0.75;
    store(&p.sea_surge, lift.clamp(0.0, 1.0).powf(0.85));

    // **And the blow at the front of it.**
    //
    // A wave does not fade in. She falls, the water stops her, and what you hear is a
    // thump followed by a second of it running off her — so the event is the moment the
    // hull is *decelerated*, which is her vertical acceleration and nothing else.
    //
    // The threshold is measured rather than chosen: in a metre of coastal sea this
    // boat's vertical acceleration is a tenth of a metre a second squared half the time,
    // one and three tenths at the ninetieth, and three at the worst of it. One and two
    // tenths therefore picks out the top tenth of them, which is what "a wave hit her"
    // means — the rest of the sea is the swelling hiss above and does not need a bang.
    //
    // One per wave and not one per frame: it fires on the way *up* through the
    // threshold and then holds off for a third of a second, which is shorter than any
    // wave and longer than the ringing of one.
    let step = time.delta_secs().max(0.004);
    let accel = (sim.sim.state.heave_rate - was.heave_rate) as f32 / step;
    was.heave_rate = sim.sim.state.heave_rate;
    was.since_slam += step;
    if accel > 1.2 && was.accel <= 1.2 && was.since_slam > 0.33 {
        was.since_slam = 0.0;
        store(&p.sea_slam, ((accel - 1.2) / 1.8).clamp(0.08, 1.0));
    }
    was.accel = accel;

    // Flogging: the worse-behaved of the two sails, and faster in more wind.
    let luffing = t
        .main
        .coeffs
        .luffing
        .max(t.jib.coeffs.luffing * (1.0 - sim.controls.furl)) as f32;
    // How loud it is, which until now was not a question the wind got a vote in: a sail
    // luffing in four knots was drawn at exactly the level of one luffing in thirty. The
    // noise a flogging sail makes is the dynamic pressure in it, so it goes with the
    // square of the wind — amplitude with something near the first power and a bit, which
    // is what the exponent here is. In a drifter it is a soft slatting you can talk over;
    // at twenty-five knots it is the loudest thing on the boat and the reason people
    // shout at each other on foredecks.
    let press = (aws / 18.0).clamp(0.0, 1.4).powf(1.6);
    store(&p.flog_level, (luffing * press).clamp(0.0, 1.0));
    store(&p.flog_hz, 2.6 + 0.22 * aws.min(28.0));

    // Rain. The one voice here driven by something other than the boat, and the first
    // sound in the game that tells you about the weather rather than about the sailing.
    store(&p.rain_level, rain.fall.clamp(0.0, 1.0).powf(0.8));

    // --- Gear ----------------------------------------------------------------
    //
    // Driven by how fast the lines are actually moving, not by which key is down. That
    // matters for more than tidiness: the autopilot trims, the assist trims, a sheet
    // eases itself when a clutch is slipped — and all of those should make the noise a
    // moving line makes. Keyed off the controls, every mover gets it for free.
    let c = &sim.controls;
    let now = [c.main_sheet, c.jib_sheet, c.outhaul, c.reef, c.furl, c.vang];
    // Floored at four milliseconds, and *that* was the bug behind the clattering winch.
    // A tenth of a millisecond is not a frame time, it is a frame time that has not
    // been measured yet — and dividing a float's worth of jitter by it turned a
    // hundred-thousandth of a change into half the sheet's range a second. The
    // measurement said so plainly: every reported delta was under 0.00005 while the
    // rate was pinned at its ceiling, which cannot both be true unless the divisor is
    // nonsense.
    let dt = time.delta_secs().max(0.004) as f64;
    let names = ["main", "jib", "outhaul", "reef", "furl", "vang"];
    let mut haul = 0.0;
    let mut biggest = (0usize, 0.0_f64);
    if was.started {
        for (i, (a, b)) in now.iter().zip(&was.lines).enumerate() {
            let moved = (a - b).abs();
            haul += moved;
            if moved > biggest.1 {
                biggest = (i, moved);
            }
        }
    }
    was.lines = now;

    // A dead band first, and it is not a nicety. Measured: the console's own sheet
    // lever writes its value back through a thirty-two bit widget every frame, so the
    // control changes by about a hundred-thousandth of its range with nobody touching
    // anything. Normalised against a quarter of the range a second, that creep read as
    // a hard grind and the winch clattered continuously in an empty cockpit.
    //
    // A real winch answers the same way: a sheet creeping under load turns nothing and
    // makes no sound, and the pawls only speak when the drum actually moves. So
    // anything under two percent of the range a second is silence, and the scale runs
    // from there to half the range a second, which is somebody working hard.
    const DEAD: f64 = 0.02;
    const BRISK: f64 = 0.5;
    let moving = (haul / dt - DEAD).max(0.0);
    let rate = (moving / BRISK).clamp(0.0, 1.6) as f32;
    // Reaching full on a moderate haul rather than only on a sprint: with the dead band
    // taking the first two percent, a brisk but ordinary grind was only getting a third
    // of the voice.
    // And the reel, which goes through the *same* voice rather than being given one of
    // its own. A winch pawl and a reel pawl are the same object: a sprung steel finger
    // riding a toothed wheel, and what distinguishes them is how fast the wheel turns.
    // Writing a second click train would have been writing this one twice.
    //
    // The two differ in one way that matters and it is in the rates. A winch is worked
    // by a person and tops out around twenty clicks a second; a reel is turned by a fish
    // and a tooth goes past every few centimetres of line, so a run is up at the ceiling
    // and that is why it sounds like a scream rather than a rattle. Finer teeth than the
    // ratchet is *drawn* with — the ear hears the pitch of a click train long after the
    // eye has given up counting, so the sound wants the real tooth and the drawing wants
    // one it can show.
    const TOOTH: f32 = 0.035;
    // Metres a second coming in, by differencing the spool rather than reading the key.
    let winding = if was.started {
        ((was.line_out - fishing.line_out) / dt).max(0.0) as f32
    } else {
        0.0
    };
    was.line_out = fishing.line_out;
    let (reel_rate, reel_hz) = if fishing.paying > 0.01 {
        // Line going out against the drag: the ratchet, loud, and the one sound in this
        // game that means put your book down.
        let hz = (fishing.paying as f32 / TOOTH).clamp(0.0, 90.0);
        // Past one, which nothing else in here goes to.
        //
        // A reel screaming is not a background noise and it is not in proportion to
        // anything else on the boat: it is six inches from your hand, it is metal
        // hammering metal eighty times a second, and the whole reason the sound exists
        // is that it is the thing that makes a person put down what they are holding.
        // Levelled with the pawls of a winch — a person's steady grinding — it was
        // polite, which is the one thing it must never be.
        //
        // It stops short of the clack, because a boom coming across stays the loudest
        // thing aboard and that is a rule about safety rather than about mixing.
        (0.8 + 1.1 * (hz / 55.0).min(1.0), hz)
    } else if fishing.out() && winding > 0.02 {
        // Winding in, and it rattles. The pawl is riding the same wheel the other way
        // and a multiplier being wound is one of the noisier objects on a small boat —
        // quieter than a fish taking line, because that is a sprung pawl being hammered
        // rather than pushed, but nothing like silent. It was at a fifth of the voice
        // and half the tooth rate, which is a reel being wound in another boat.
        let hz = (winding / TOOTH).clamp(0.0, 70.0);
        (0.40 + 0.55 * (hz / 22.0).min(1.0), hz)
    } else {
        (0.0, 0.0)
    };
    store(&p.gear_level, (rate * 1.7).min(1.0).max(reel_rate));
    // Pawls per second. A two-speed winch at a hard grind clicks about twenty times a
    // second; eased by hand it is a slow tick you can count.
    store(&p.gear_hz, (3.0 + 26.0 * rate.min(1.2)).max(reel_hz));
    // Kept, because it is what found the three faults behind a winch that clattered in
    // an empty cockpit — an unfloored frame time, a dead band that was not there, and a
    // debug line that compared the current controls against themselves because the
    // previous ones had already been overwritten two statements earlier. A sound driven
    // by a derivative needs an instrument that shows the derivative.
    if moving > 0.08 {
        debug!(
            "gear: {:.2}/s over dt {:.4}, most of it {} {:+.6}",
            moving, dt, names[biggest.0], biggest.1
        );
    }

    // The boom arriving. A gybe is the one event on a small boat loud enough to be
    // heard from the dock, and it is the sheet coming up hard against its own length
    // rather than anything striking anything — so it is rated by how fast the boom was
    // moving when it ran out of rope, not by the angle it stopped at.
    let boom = sim.sim.state.boom_angle;
    if was.started {
        let crossed = boom.signum() != was.boom.signum() && was.boom.abs() > 0.05;
        let swing = ((boom - was.boom).abs() / dt) as f32;
        if crossed && swing > 0.8 {
            p.clack((swing / 3.0).clamp(0.25, 1.0));
            debug!("gear: boom across at {:.1} rad/s", swing);
        }
    }
    was.boom = boom;
    was.started = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How long a note takes to get out of the way of the next one.
    ///
    /// The number that matters is not how long the note is audible — a piano note is
    /// faintly audible for seconds — but how long it is loud enough to blur the note
    /// after it, which is around twenty decibels down for notes of the same loudness.
    /// Measured against the quaver at this music's own tempi (0.25–0.40 s), because a
    /// note still masking when the next one arrives cannot help blurring into it.
    fn masks_for(hz: f32, damp: f32) -> f32 {
        let rate = 48_000.0;
        let mut v = Plucked::default();
        v.strike(hz, 1.0, 0.0, rate, damp, Voice::Struck);
        let mut peak = 0.0f32;
        let mut last_loud = 0.0f32;
        for i in 0..(rate as usize * 10) {
            let out = v.next(1.0 / rate, 0.0).abs();
            peak = peak.max(out);
            if out > peak / 10.0 {
                last_loud = i as f32 / rate;
            }
        }
        last_loud
    }

    /// Only the wire singing: everything else in the mix turned off, so the measurement
    /// is of one voice.
    fn rig_only() -> (Arc<SeaParams>, SeaStream) {
        let p = Arc::new(SeaParams::default());
        store(&p.rig_level, 1.0);
        store(&p.rig_hz, 300.0);
        (p.clone(), SeaStream::new(p))
    }

    fn rms(stream: &mut SeaStream, seconds: f32) -> f32 {
        let n = (SAMPLE_RATE as f32 * seconds) as usize;
        let mut sum = 0.0f64;
        for _ in 0..n {
            let out = stream.next().unwrap_or(0.0);
            sum += (out * out) as f64;
        }
        (sum / n as f64).sqrt() as f32
    }

    /// How loud each part of the mix actually is, in the state the game spends most of
    /// its time in: a fresh breeze, five knots through the water, and the band playing.
    ///
    /// Reported from the deck as barely being able to hear the music change, which is a
    /// balance question before it is a composition question — a difference you cannot
    /// hear is not subtle, it is absent. So the first thing is a number: how far above
    /// or below the sea the music actually sits at the defaults.
    #[test]
    fn where_the_music_sits_against_the_sea() {
        let rate = SAMPLE_RATE as f32;
        let sea_only = |music_on: bool| -> (f32, f32) {
            let p = Arc::new(SeaParams::default());
            // The defaults the settings panel starts at.
            p.set_master(crate::settings::Settings::default().volume);
            p.set_sea(crate::settings::Settings::default().sea);
            p.set_boat(crate::settings::Settings::default().boat);
            p.set_music(crate::settings::Settings::default().music);
            // A fresh breeze: eighteen knots apparent, five and a half through the water.
            store(&p.wind_level, (18.0f32 / 20.0).powf(1.5));
            store(&p.wind_cutoff, 320.0 + 95.0 * 18.0);
            store(&p.water_level, (5.5f32 / 7.0).powf(1.4));
            store(&p.water_cutoff, 260.0 + 150.0 * 5.5);
            store(&p.rig_level, ((18.0 - 11.0) / 16.0f32).powf(1.6));
            store(&p.rig_hz, 300.0);
            let mut s = SeaStream::new(p.clone());
            // Settle the mute ramp and the slews.
            let mut sum = 0.0f64;
            let mut n = 0usize;
            let mut peak = 0.0f32;
            let every = (rate * 0.35) as usize;
            for i in 0..(rate as usize * 12) {
                if music_on && i > (rate as usize * 4) && i % every == 0 {
                    // A chord under the tune, which is what the left hand actually does
                    // and what decides whether the limiter has to work.
                    p.strike(147.0, 0.5);
                    p.strike(220.0, 0.45);
                    p.strike(262.0, 0.4);
                    // A melody note at the level the composer actually asks for: 0.55
                    // from `Asking`, times a sail that is working well.
                    p.strike_damped(330.0, 0.55 * 0.85, 0.76);
                }
                let out = s.next().unwrap_or(0.0);
                if i > (rate as usize * 4) {
                    sum += (out * out) as f64;
                    peak = peak.max(out.abs());
                    n += 1;
                }
            }
            ((sum / n as f64).sqrt() as f32, peak)
        };
        let (sea, _) = sea_only(false);
        let (both, peak) = sea_only(true);
        // The music's own contribution, pulled out of the sum of squares.
        let music = (both * both - sea * sea).max(0.0).sqrt();
        let db = 20.0 * (music / sea).log10();
        let bed = 20.0 * sea.log10();
        // The sea is deliberately a long way down, and that is a decision by the person
        // who actually sails this rather than a mixing rule: the sea here is a *monotone*.
        // It is the same hiss for an hour, it carries no information once you have heard
        // the first second of it, and the things that do carry information — the rig, the
        // gear, the band — are all over it. So it is held at the level of a room tone and
        // the music is the subject.
        //
        // It started at -39.9 dBFS with everything else down there with it, which was the
        // whole mix being too quiet rather than the sea being right. Now the bed is here
        // and the music is well above it.
        assert!(
            (-37.0..=-28.0).contains(&bed),
            "the sea is a bed and a quiet one, around -32 dBFS: measured {bed:.1}"
        );
        assert!(
            (10.0..=22.0).contains(&db),
            "the music is the subject and should stand well over it: measured {db:+.1} dB"
        );
        // And the limiter is not doing the mixing. A four-note chord with the tune over it
        // is the loudest thing the composer asks for, and it has to arrive without `tanh`
        // leaning on it — a soft clip that is working is the thing that was reported, long
        // ago, as the music interrupting the sea.
        assert!(
            peak < 0.8,
            "the loudest chord should leave the soft clip alone: peak {peak:.2}"
        );
    }

    /// Two strokes, and the thing that makes it a bell rather than a chime.
    #[test]
    fn the_bell_is_struck_twice_and_rings_like_a_bell() {
        let rate = SAMPLE_RATE as f32;
        let p = Arc::new(SeaParams::default());
        p.set_master(1.0);
        p.set_boat(1.0);
        p.set_sea(0.0);
        let mut s = SeaStream::new(p.clone());
        // Past the mute ramp first, or the first stroke is measured through a fade.
        for _ in 0..(rate as usize * 4) {
            let _ = s.next();
        }
        let window = (rate * 0.01) as usize;
        let mut env = Vec::new();
        let mut here = 0.0f32;
        // Rung a fifth of a second *into* the measurement rather than before it: with the
        // bell struck at sample zero there is nothing in front of the first stroke to
        // compare it with, and a detector looking for a step up finds one stroke in two.
        // Which it did, and the sound was right all along.
        for i in 0..(rate as usize * 6) {
            if i == (rate * 0.2) as usize {
                p.ring(2);
            }
            let out = s.next().unwrap_or(0.0).abs();
            here = here.max(out);
            if i % window == window - 1 {
                env.push(here);
                here = 0.0;
            }
        }
        let step = window as f32 / rate;
        let loudest = env.iter().copied().fold(0.0f32, f32::max);
        assert!(loudest > 0.02, "a bell has to be audible: {loudest:.4}");
        // Two attacks. A second stroke on a still-ringing bell does not double the
        // level — the first one has decayed to about two thirds by then and the partials
        // are at random phase against each other — so what it looks like in an envelope
        // is a clear step up rather than a spike, and a quarter again is what that is.
        let attacks: Vec<f32> = env
            .windows(2)
            .enumerate()
            // Measured against the quiet rather than against the peak: the second
            // stroke lands on a bell that has already decayed, so it is a step up from a
            // quarter of the peak to four tenths of it — 0.250 to 0.401 in the envelope —
            // and a threshold set against the loudest moment of the first stroke misses
            // it entirely. Which it did.
            .filter(|(_, w)| w[1] > w[0] * 1.25 && w[1] > loudest * 0.1)
            .map(|(i, _)| i as f32 * step)
            .collect();
        assert!(
            attacks.len() >= 2,
            "two strokes for somebody arriving, got {}",
            attacks.len()
        );
        let apart = attacks[attacks.len() - 1] - attacks[0];
        assert!(
            (0.3..0.6).contains(&apart),
            "struck faster than this it is an alarm and slower it is two bells: {apart:.2} s"
        );
        // And it rings on. A notification sound stops; a bell hums.
        let late = env[(4.2 / step) as usize];
        assert!(
            late > loudest / 100.0,
            "four seconds on there should still be a hum: {late:.5} against {loudest:.4}"
        );
    }

    /// The three voices have to be different *instruments* to the ear, not three settings
    /// of one. Two measurements say whether they are: how long each takes to arrive, and
    /// how bright it is.
    #[test]
    fn the_band_has_three_colours() {
        let rate = SAMPLE_RATE as f32;
        let render = |voice: Voice| -> (f32, f32, f32) {
            let mut v = Plucked::default();
            v.strike(220.0, 0.9, 0.0, rate, 0.0, voice);
            // A short-term envelope rather than the sample values: the peak *sample* of a
            // note made of six stretched partials lands wherever they happen to line up,
            // which the first version of this measured and got a hundred and forty
            // milliseconds for a piano. What the ear calls the attack is how fast the
            // envelope gets there.
            let window = (rate * 0.005) as usize;
            let mut env = Vec::new();
            // Brightness as the energy in the *difference* between one sample and the
            // next against the energy in the samples themselves, which rises with the
            // frequencies present. Zero crossings were the first attempt and they count
            // the fundamental and almost nothing else — a dark note and a bright one at
            // the same pitch cross zero the same number of times.
            let (mut here, mut edge, mut power, mut last) = (0.0f32, 0.0f64, 0.0f64, 0.0f32);
            for i in 0..(rate as usize * 6) {
                let out = v.next(1.0 / rate, 0.0);
                here = here.max(out.abs());
                edge += ((out - last) * (out - last)) as f64;
                power += (out * out) as f64;
                last = out;
                if i % window == window - 1 {
                    env.push(here);
                    here = 0.0;
                }
            }
            let loudest = env.iter().copied().fold(0.0f32, f32::max);
            let step = window as f32 / rate;
            let attack = env.iter().position(|&e| e > loudest * 0.5).unwrap_or(0) as f32 * step;
            // How much louder it is a quarter of a second in than it was at fifty
            // milliseconds, which is the whole of what "swells" means.
            let at = |t: f32| env[((t / step) as usize).min(env.len() - 1)].max(1e-6);
            (
                attack,
                (edge / power.max(1e-12)).sqrt() as f32,
                at(0.25) / at(0.05),
            )
        };
        let (struck_attack, struck_bright, struck_swell) = render(Voice::Struck);
        let (bowed_attack, _, bowed_swell) = render(Voice::Bowed);
        let (ground_attack, ground_bright, _) = render(Voice::Ground);

        // A struck string is there at once; a bow arrives. A fifth of a second is the
        // difference between an event and something coming on, and it is the single most
        // audible distinction in this whole synthesiser.
        assert!(
            struck_attack < 0.02 && ground_attack < 0.02,
            "struck {struck_attack:.3} s, ground {ground_attack:.3} s"
        );
        assert!(
            bowed_attack > 0.1,
            "a bow has to be heard arriving: {bowed_attack:.3} s"
        );
        // Said the other way round, which is the way a listener hears it: a quarter of a
        // second in, a bowed note is still getting louder and a struck one is already on
        // its way down.
        assert!(
            bowed_swell > 1.5 && struck_swell < 1.0,
            "swell from 50 ms to 250 ms: bowed x{bowed_swell:.2}, struck x{struck_swell:.2}"
        );
        // The ground is dark. Fewer zero crossings is less upper spectrum, and that is
        // what keeps a bass from arguing with the tune above it.
        assert!(
            ground_bright < struck_bright * 0.85,
            "ground {ground_bright:.4} against struck {struck_bright:.4}"
        );
    }

    /// The rig lets go of its note when the wind changes, and finds it again when the
    /// wind settles.
    #[test]
    fn the_wire_only_sings_while_the_flow_holds_still() {
        let (p, mut s) = rig_only();
        // Settle: the mute ramp alone is a couple of seconds.
        rms(&mut s, 4.0);
        let holding = rms(&mut s, 1.0);
        assert!(holding > 0.01, "a steady rig should sing: {holding:.4}");

        // A gust: the apparent goes from fifteen to nineteen knots over two seconds,
        // which is an ordinary puff and a quarter on the shedding frequency.
        let steps = 120;
        let mut during = 0.0f32;
        for i in 0..steps {
            store(&p.rig_hz, 300.0 + 75.0 * (i as f32 / steps as f32));
            during = during.max(rms(&mut s, 2.0 / steps as f32));
        }
        let gusting = rms(&mut s, 0.2);
        assert!(
            gusting < holding * 0.5,
            "the note should break while the flow moves: {gusting:.4} against {holding:.4}"
        );
        let _ = during;

        // And back, once it is steady at the new speed. Not instantly — the wire has to
        // find the note — but inside a second and a half.
        rms(&mut s, 1.5);
        // Measured: a two-second puff of four knots drops the whine to 0.35 of its
        // steady level — nine decibels, which is audibly letting go and not a dip — and
        // it is back at full inside a second and a half.
        let again = rms(&mut s, 1.0);
        assert!(
            again > holding * 0.8,
            "and come back when it steadies: {again:.4} against {holding:.4}"
        );
    }

    #[test]
    fn a_damped_note_leaves_room_for_the_next_one() {
        // Middle of the melody's range. Undamped it rings straight through two or three
        // quavers, which is the wash that was reported; damped it is clear of the next
        // note.
        let rung = masks_for(440.0, 0.0);
        let sung = masks_for(440.0, 0.76);
        // Measured: 3.93 s left to ring, 0.229 s sung, 0.106 s for a cut — and the
        // bottom of the melody's register, where the strings ring longest, is 0.265 s.
        assert!(rung > 0.8, "a piano note should ring: {rung:.2} s");
        assert!(
            sung < 0.30,
            "a sung note has to be clear of the next quaver: {sung:.2} s"
        );
        // And the cut, which is an ornament rather than a note and has to be gone before
        // the note it decorates.
        let cut = masks_for(660.0, 0.95);
        assert!(cut < 0.12, "a cut is a flick: {cut:.3} s");
    }
}

#[cfg(test)]
mod what_is_loud {
    use super::*;
    use std::sync::Arc;

    /// How loud one group of the mix is on its own, at the settings panel's defaults.
    fn group(seconds: f32, set: impl Fn(&Arc<SeaParams>), music: bool) -> f32 {
        let rate = SAMPLE_RATE as f32;
        let p = Arc::new(SeaParams::default());
        p.set_master(crate::settings::Settings::default().volume);
        p.set_sea(crate::settings::Settings::default().sea);
        p.set_boat(crate::settings::Settings::default().boat);
        p.set_music(crate::settings::Settings::default().music);
        set(&p);
        let mut s = SeaStream::new(p.clone());
        let mut sum = 0.0f64;
        let mut n = 0usize;
        let every = (rate * 0.35) as usize;
        for i in 0..(rate * seconds) as usize {
            if music && i > (rate as usize * 4) && i % every == 0 {
                p.strike(147.0, 0.5);
                p.strike(220.0, 0.45);
                p.strike(262.0, 0.4);
                p.strike_damped(330.0, 0.55 * 0.85, 0.76);
            }
            let out = s.next().unwrap_or(0.0);
            if i > (rate as usize * 4) {
                sum += (out * out) as f64;
                n += 1;
            }
        }
        (sum / n as f64).sqrt() as f32
    }

    fn db(x: f32) -> f32 {
        20.0 * x.max(1e-9).log10()
    }

    /// The level the game asks for when a sail is flogging in a given breeze.
    ///
    /// The same expression as `update`, which is the point of it being a function: the
    /// test and the game have to be asking for the same sound or the test is measuring
    /// something nobody hears.
    fn flogging_at(aws: f32) -> f32 {
        (aws / 18.0).clamp(0.0, 1.4).powf(1.6).min(1.0)
    }

    /// A sail flogging in a drifter is not a sail flogging in a gale, and until this test
    /// existed it was exactly that.
    ///
    /// Reported from the deck as three things — the music disappearing, the main's
    /// flapping being relentless, the sea covering everything — which measurement turned
    /// into one thing. The sea was never the problem: it sits at -49 dBFS in a light air
    /// and -33 in a fresh breeze, twenty to thirty-five decibels under the band. What was
    /// covering everything was a sail luffing in eight knots of wind, drawn at the level
    /// of one luffing in thirty, out of *unfiltered white noise* — which is both the
    /// wrong sound for canvas and the most effective masker there is.
    ///
    /// Measured before: the boat group at -26.1 dBFS with the music only twelve decibels
    /// over it. After: -41.8, and twenty-eight decibels of daylight.
    #[test]
    fn a_sail_flogging_in_a_drifter_is_not_a_sail_flogging_in_a_gale() {
        let state = |aws: f32, stw: f32, luff: f32| {
            move |p: &Arc<SeaParams>| {
                store(
                    &p.wind_level,
                    (aws / 20.0).clamp(0.0, 1.6).powf(1.5).min(1.15),
                );
                store(&p.wind_cutoff, 320.0 + 95.0 * aws.min(30.0));
                store(&p.water_level, (stw / 7.0).clamp(0.0, 1.3).powf(1.4));
                store(&p.water_cutoff, 260.0 + 150.0 * stw.min(9.0));
                store(
                    &p.rig_level,
                    ((aws - 11.0) / 16.0).clamp(0.0, 1.0).powf(1.6),
                );
                store(&p.rig_hz, 300.0);
                store(&p.flog_level, luff * flogging_at(aws));
                store(&p.flog_hz, 2.6 + 0.22 * aws.min(28.0));
            }
        };
        // Sheets eased in a light breeze, which is where the drill begins and where the
        // complaint came from.
        let light = db(group(9.0, state(7.8, 0.7, 1.0), false));
        // And the same sail in a fresh one, which has to still be the loudest thing on
        // the boat, because it is.
        let fresh = db(group(9.0, state(18.0, 5.5, 1.0), false));
        println!("flogging: light {light:.1} dBFS, fresh {fresh:.1} dBFS");
        assert!(
            fresh - light > 10.0,
            "a drifter and a fresh breeze are {:.1} dB apart, which is one sound for both",
            fresh - light
        );
        // Still there, though. A sail flogging is never nothing — it is the sound of
        // something being done wrong and it has to be audible over the sea.
        let sea = db(group(9.0, state(7.8, 0.7, 0.0), false));
        assert!(
            light > sea + 4.0,
            "flogging at {light:.1} against a sea at {sea:.1} is a sail nobody can hear"
        );
        // And the band is over it rather than under it.
        let quiet = group(9.0, state(7.8, 0.7, 1.0), false);
        let with_band = group(9.0, state(7.8, 0.7, 1.0), true);
        let band = (with_band * with_band - quiet * quiet).max(0.0).sqrt();
        let over = db(band) - light;
        println!("the band stands {over:+.1} dB over a flogging main in a light breeze");
        assert!(
            over > 20.0,
            "the music is only {over:+.1} dB over a flogging sail, which is why it \
             disappeared"
        );
    }
}

#[cfg(test)]
mod the_reel_and_the_band {
    use super::*;
    use std::sync::Arc;

    /// A fish running takes the floor; winding one in does not.
    ///
    /// Measured for the session working the fishing, who asked rather than waiting for a
    /// player to tell them. The reel is deliberately past unity gain when line is going
    /// out and it should be — but it came out at exactly the level of the band, nought
    /// decibels of separation, with the mix peaking at 0.92 and the soft clip leaning on
    /// it. Two loud things in the same place is not drama, it is a fight.
    ///
    /// So the band ducks under a run, the way it already drops to one note under a
    /// squall, and comes back when the fish stops. Winching a sheet in ducks nothing: the
    /// threshold is unity gain, which by the reel's own design is exactly "line is going
    /// out".
    #[test]
    fn a_fish_running_takes_the_floor_and_a_winch_does_not() {
        let rate = SAMPLE_RATE as f32;
        let measure = |level: f32, hz: f32, music: bool| {
            let p = Arc::new(SeaParams::default());
            p.set_master(crate::settings::Settings::default().volume);
            p.set_sea(crate::settings::Settings::default().sea);
            p.set_boat(crate::settings::Settings::default().boat);
            p.set_music(crate::settings::Settings::default().music);
            store(&p.wind_level, (18.0f32 / 20.0).powf(1.5));
            store(&p.wind_cutoff, 320.0 + 95.0 * 18.0);
            store(&p.water_level, (5.5f32 / 7.0).powf(1.4));
            store(&p.water_cutoff, 260.0 + 150.0 * 5.5);
            store(&p.rig_level, ((18.0 - 11.0) / 16.0f32).powf(1.6));
            store(&p.rig_hz, 300.0);
            store(&p.gear_level, level);
            store(&p.gear_hz, hz);
            let mut s = SeaStream::new(p.clone());
            let (mut sum, mut n, mut peak) = (0.0f64, 0usize, 0.0f32);
            let every = (rate * 0.35) as usize;
            for i in 0..(rate as usize * 9) {
                if music && i > (rate as usize * 4) && i % every == 0 {
                    p.strike(147.0, 0.5);
                    p.strike(220.0, 0.45);
                    p.strike(262.0, 0.4);
                    p.strike_damped(330.0, 0.55 * 0.85, 0.76);
                }
                let out = s.next().unwrap_or(0.0);
                if i > (rate as usize * 4) {
                    sum += (out * out) as f64;
                    peak = peak.max(out.abs());
                    n += 1;
                }
            }
            ((sum / n as f64).sqrt() as f32, peak)
        };
        let band_under = |level: f32, hz: f32| {
            let (quiet, _) = measure(level, hz, false);
            let (both, peak) = measure(level, hz, true);
            let band = (both * both - quiet * quiet).max(0.0).sqrt();
            let db = |x: f32| 20.0 * x.max(1e-9).log10();
            (db(quiet), db(band), peak)
        };

        // Line going out at a rate that has the reel past unity: the moment of the thing.
        let (reel, band, peak) = band_under(1.9, 70.0);
        println!("a fish running: reel {reel:.1} dBFS, band {band:.1}, peak {peak:.2}");
        assert!(
            reel - band > 8.0,
            "the reel is only {:.1} dB over the band on a run, which is two loud things \
             in the same place",
            reel - band
        );
        assert!(
            peak < 0.90,
            "the soft clip is doing the mixing: peak {peak:.2}"
        );

        // And a winch is not a fish. Nothing ducks here — a sheet coming in is work, not
        // an event, and the band plays through work.
        let (_, winching, _) = band_under(0.95, 22.0);
        let (_, nothing, _) = band_under(0.0, 0.0);
        assert!(
            (winching - nothing).abs() < 1.0,
            "winding a winch ducked the band by {:.1} dB",
            nothing - winching
        );
    }
}

#[cfg(test)]
mod the_sea_voice {
    /// **Which waves speak, as a number.**
    ///
    /// "Only some of them" is an instruction and this is the measurement of it: put her
    /// in a real sea with no way on and count what fraction of the time the sea voice is
    /// above nothing, and how often it is up near the top. A voice that is always on is
    /// the even rushing this replaced; one that is almost never on is a silent sea.
    #[test]
    fn only_some_of_the_waves_speak() {
        use sail_core::sim::{Simulation, SUBSTEP};
        use sail_core::state::{BoatState, Controls};
        let breeze = sail_core::units::kn_to_ms(14.0);
        let mut sim = Simulation::new(
            sail_core::boat::BoatConfig::charter_sloop_34(),
            BoatState::at_rest(0.0),
            Box::new(sail_core::wind::SteadyWind::new(0.0, breeze)),
        )
        .with_water(Box::new(sail_core::water::WindSea::new(
            breeze, 20_000.0, 0.0, 11,
        )));
        let controls = Controls {
            main_sheet: 1.0,
            jib_sheet: 1.0,
            reef: 1.0,
            furl: 1.0,
            ..Default::default()
        };
        let (mut heard, mut loud, mut all) = (0u32, 0u32, 0u32);
        for step in 0..(180.0 / SUBSTEP) as u32 {
            sim.run(SUBSTEP, &controls);
            if (step as f64) * SUBSTEP < 30.0 {
                continue;
            }
            all += 1;
            let lift = ((sim.state.heave_rate.abs() as f32 - 0.25).max(0.0) / 0.75)
                .clamp(0.0, 1.0)
                .powf(0.85);
            if lift > 0.02 {
                heard += 1;
            }
            if lift > 0.5 {
                loud += 1;
            }
        }
        let (heard, loud) = (
            heard as f64 / all as f64 * 100.0,
            loud as f64 / all as f64 * 100.0,
        );
        println!("the sea speaks {heard:.0}% of the time, and is loud {loud:.0}%");
        {
            // Jak mocne są pionowe przyspieszenia — na razie pomiar, nie asercja.
            let mut sim2 = Simulation::new(
                sail_core::boat::BoatConfig::charter_sloop_34(),
                BoatState::at_rest(0.0),
                Box::new(sail_core::wind::SteadyWind::new(0.0, breeze)),
            )
            .with_water(Box::new(sail_core::water::WindSea::new(
                breeze, 20_000.0, 0.0, 11,
            )));
            let mut prev = 0.0f64;
            let mut peaks: Vec<f64> = Vec::new();
            for step in 0..(180.0 / SUBSTEP) as u32 {
                sim2.run(SUBSTEP, &controls);
                let a = (sim2.state.heave_rate - prev) / SUBSTEP;
                prev = sim2.state.heave_rate;
                if (step as f64) * SUBSTEP > 30.0 {
                    peaks.push(a);
                }
            }
            peaks.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let at = |q: f64| peaks[(peaks.len() as f64 * q) as usize];
            println!(
                "pionowe przyspieszenie m/s2: p50 {:.2}  p90 {:.2}  p99 {:.2}  max {:.2}",
                at(0.5),
                at(0.9),
                at(0.99),
                peaks[peaks.len() - 1]
            );
        }
        // And what the level under it comes to, in the three conditions that matter.
        for (name, stw_kn, hs, tws_kn) in [
            ("ghosting on a millpond", 6.0_f32, 0.0_f32, 5.0_f32),
            ("hove to in a gale", 0.6, 2.6, 34.0),
            ("reaching in a fresh breeze", 6.5, 1.2, 18.0),
        ] {
            let through_it = (stw_kn / 7.0).clamp(0.0, 1.3).powf(1.4);
            let whitecaps = crate::curve::smoothstep(8.0, 25.0, tws_kn);
            let sea = (hs / 1.6).clamp(0.0, 1.5).powf(0.9) * (0.3 + 0.7 * whitecaps);
            println!(
                "   {name:<28} hull {through_it:.2} + sea {:.2} = {:.2}",
                0.85 * sea,
                (through_it + 0.85 * sea).clamp(0.0, 1.8)
            );
        }
        assert!(
            (25.0..85.0).contains(&heard),
            "the sea voice is up {heard:.0}% of the time, which is {} rather than a sea",
            if heard > 85.0 {
                "a rushing"
            } else {
                "a silence"
            }
        );
    }
}

#[cfg(test)]
mod what_was_recorded {
    use super::*;

    /// A RIFF file written by hand, with a chunk in front of the data that a reader
    /// expecting a fixed layout would walk straight past.
    fn wav(rate: u32, channels: u16, bits: u16, samples: &[i16], junk_first: bool) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend(b"WAVE");
        if junk_first {
            // `LIST` chunks turn up in front of `data` in files from real recorders, and
            // this one is an odd length so the padding byte matters too.
            body.extend(b"LIST");
            body.extend(5u32.to_le_bytes());
            body.extend(b"INFOx");
            body.push(0);
        }
        body.extend(b"fmt ");
        body.extend(16u32.to_le_bytes());
        body.extend(1u16.to_le_bytes());
        body.extend(channels.to_le_bytes());
        body.extend(rate.to_le_bytes());
        body.extend((rate * channels as u32 * 2).to_le_bytes());
        body.extend((channels * 2).to_le_bytes());
        body.extend(bits.to_le_bytes());
        body.extend(b"data");
        body.extend((samples.len() as u32 * 2).to_le_bytes());
        for s in samples {
            body.extend(s.to_le_bytes());
        }
        let mut out = Vec::new();
        out.extend(b"RIFF");
        out.extend((body.len() as u32).to_le_bytes());
        out.extend(body);
        out
    }

    /// **Chunks are walked, not assumed.** A reader that expects `data` at byte
    /// forty-four works on its own test file and on nothing a recorder produced.
    #[test]
    fn a_chunk_in_front_of_the_data_is_stepped_over() {
        let samples = [0i16, 16384, -16384, 32767];
        let plain = wav_mono(&wav(44_100, 1, 16, &samples, false)).expect("a plain file");
        let padded = wav_mono(&wav(44_100, 1, 16, &samples, true)).expect("one with a LIST");
        assert_eq!(plain.0.len(), 4);
        assert_eq!(plain, padded);
        assert!((plain.0[1] - 0.5).abs() < 1e-3);
        assert!((plain.0[2] + 0.5).abs() < 1e-3);
    }

    /// And what it will not take, it refuses rather than mangles.
    #[test]
    fn stereo_and_eight_bit_are_refused() {
        let samples = [0i16; 8];
        assert!(
            wav_mono(&wav(44_100, 2, 16, &samples, false)).is_none(),
            "stereo"
        );
        assert!(
            wav_mono(&wav(44_100, 1, 8, &samples, false)).is_none(),
            "eight-bit"
        );
        assert!(wav_mono(b"not a riff file at all").is_none());
        assert!(wav_mono(&[]).is_none());
    }

    /// **Does a call actually come out of the mix.** Asked as a test because "I cannot
    /// hear anything" has two causes that feel identical from the deck — nothing is
    /// playing, or something is playing underneath a force nine — and only one of them
    /// is a bug. This settles that half of it without anybody listening.
    #[test]
    fn a_call_comes_out_of_the_mix() {
        let params = Arc::new(SeaParams {
            // A tenth of a second of steady signal: not a sound, but unmistakable.
            voices: vec![vec![0.5; SAMPLE_RATE as usize / 10]],
            groups: [(0, 1); Call::KINDS],
            ..SeaParams::default()
        });
        params.set_master(1.0);
        params.set_boat(1.0);
        params.set_sea(0.0);
        params.set_music(0.0);
        let mut stream = SeaStream::new(params.clone());

        let loudest = |s: &mut SeaStream, n: usize| {
            (0..n).fold(0.0_f32, |m, _| m.max(s.next().unwrap_or(0.0).abs()))
        };
        // Let the filters settle, then measure the floor with nobody speaking.
        loudest(&mut stream, SAMPLE_RATE as usize / 2);
        let quiet = loudest(&mut stream, SAMPLE_RATE as usize / 10);

        params.shout(Call::Warn, 0);
        let called = loudest(&mut stream, SAMPLE_RATE as usize / 10);
        assert!(
            called > quiet + 0.1,
            "a call made no difference: {quiet:.3} before, {called:.3} during"
        );

        // And it stops. A play head that runs off the end of the buffer and keeps going
        // is a click at best and an index out of bounds at worst.
        loudest(&mut stream, SAMPLE_RATE as usize / 5);
        let after = loudest(&mut stream, SAMPLE_RATE as usize / 10);
        assert!(after < quiet + 0.1, "it was still going: {after:.3}");
    }

    /// Two of the same call in a row are two calls. With a bare index they would be one:
    /// the stream reads the same number and cannot tell a repeat from a value nobody
    /// cleared, which is why the trigger carries a generation.
    #[test]
    fn the_same_call_twice_is_twice() {
        let params = Arc::new(SeaParams {
            voices: vec![vec![0.5; SAMPLE_RATE as usize / 20]],
            groups: [(0, 1); Call::KINDS],
            ..SeaParams::default()
        });
        params.shout(Call::Warn, 0);
        let first = params.shout.load(Ordering::Relaxed);
        params.shout(Call::Warn, 0);
        assert_ne!(first, params.shout.load(Ordering::Relaxed));
        assert_eq!(first & 0xff, params.shout.load(Ordering::Relaxed) & 0xff);
    }

    /// **Forty-eight is what a recorder writes and forty-four one is what we play**, and
    /// the difference is a voice a semitone and a half low — close enough to sound like
    /// a choice. Converted on the way in, so nobody has to be told to export at a
    /// particular rate.
    #[test]
    fn a_recording_comes_back_at_the_rate_we_play_at() {
        let second: Vec<f32> = (0..48_000).map(|i| (i as f32 / 48_000.0) - 0.5).collect();
        let played = at_our_rate(second, 48_000);
        assert!(
            (played.len() as i32 - SAMPLE_RATE as i32).abs() < 8,
            "a second came back as {} samples at {SAMPLE_RATE} Hz",
            played.len()
        );
        // A ramp stays a ramp: resampling must not move where the sound is, only how
        // many samples describe it.
        assert!((played[0] + 0.5).abs() < 0.01);
        assert!((played[played.len() / 2]).abs() < 0.01);

        // And the rate we already play at is left alone entirely.
        let same = vec![0.25f32; 100];
        assert_eq!(at_our_rate(same.clone(), SAMPLE_RATE), same);
    }
}
