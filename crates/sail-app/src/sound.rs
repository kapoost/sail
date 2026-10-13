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
const MASTER: f32 = 0.26;

/// Parameters the simulation writes and the audio thread reads.
///
/// Atomics rather than a mutex: the audio callback must never block, and a torn read
/// of a single gain is inaudible where a missed deadline is a click.
#[derive(Debug, Default)]
pub struct SeaParams {
    wind_level: AtomicU32,
    wind_cutoff: AtomicU32,
    rig_level: AtomicU32,
    rig_hz: AtomicU32,
    water_level: AtomicU32,
    water_cutoff: AtomicU32,
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
    muted: AtomicU32,
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
        // Zero is what an uninitialised atomic reads, and silence-by-default would look
        // exactly like a broken audio thread. The parameter block is Default-derived, so
        // the stream treats "never set" as "full".
        let v = load(&self.master);
        if v <= 0.0 {
            1.0
        } else {
            v
        }
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
pub struct SeaStream {
    params: Arc<SeaParams>,
    rng: u32,
    wind_a: OnePole,
    wind_b: OnePole,
    water_lp: OnePole,
    water_hp: OnePole,
    rain_lp: OnePole,
    rope_lp: OnePole,
    pawl_phase: f32,
    pawl_env: f32,
    pawl_hz: f32,
    pawl_ring: f32,
    clack_env: f32,
    clack_phase: f32,
    smooth: Smoothed,
    rig_phase: f32,
    rig_phase2: f32,
    surge_phase: f32,
    flog_phase: f32,
    flog_env: f32,
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
            params,
            rng: 0x2545_F491,
            wind_a: OnePole::default(),
            wind_b: OnePole::default(),
            water_lp: OnePole::default(),
            water_hp: OnePole::default(),
            rain_lp: OnePole::default(),
            rope_lp: OnePole::default(),
            pawl_phase: 0.0,
            pawl_env: 0.0,
            pawl_hz: 2_000.0,
            pawl_ring: 0.0,
            clack_env: 0.0,
            clack_phase: 0.0,
            smooth: Smoothed {
                wind_cutoff: 400.0,
                water_cutoff: 500.0,
                rig_hz: 200.0,
                mute: 1.0,
                ..Default::default()
            },
            rig_phase: 0.0,
            rig_phase2: 0.0,
            surge_phase: 0.0,
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
            t_flog,
            flog_hz,
            t_rain,
            t_master,
            t_gear,
            gear_hz,
            clack,
            t_mute,
        ) = {
            let p = &self.params;
            (
                load(&p.wind_level),
                load(&p.wind_cutoff),
                load(&p.rig_level),
                load(&p.rig_hz),
                load(&p.water_level),
                load(&p.water_cutoff),
                load(&p.flog_level),
                load(&p.flog_hz),
                load(&p.rain_level),
                p.master(),
                load(&p.gear_level),
                load(&p.gear_hz),
                // Taken, not read: swapping zero in is what makes a one-shot fire once.
                f32::from_bits(p.clack.swap(0, Ordering::Relaxed)),
                if p.muted() { 0.0 } else { 1.0 },
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
        self.rig_phase = (self.rig_phase + self.smooth.rig_hz * dt).fract();
        self.rig_phase2 = (self.rig_phase2 + self.smooth.rig_hz * 1.013 * dt).fract();
        let rig = ((self.rig_phase * std::f32::consts::TAU).sin() * 0.6
            + (self.rig_phase2 * std::f32::consts::TAU).sin() * 0.4)
            * self.smooth.rig
            * 0.14;

        // --- Water at the hull: bandpassed noise with a bow-wave surge -------------
        self.surge_phase = (self.surge_phase + 0.73 * dt).fract();
        let surge = 0.78 + 0.22 * (self.surge_phase * std::f32::consts::TAU).sin();
        let n2 = self.noise();
        let low = self.water_lp.run(n2, self.smooth.water_cutoff);
        let dc = self.water_hp.run(low, 110.0);
        let water = (low - dc) * self.smooth.water * surge * 3.4;

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
        let tooth = ((self.pawl_ring * std::f32::consts::TAU).sin() * 0.55 + self.noise() * 0.45)
            * self.pawl_env
            * self.pawl_env;

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
        let clack_out = ((self.clack_phase * std::f32::consts::TAU).sin() * 0.7
            + self.noise() * 0.3)
            * self.clack_env
            * self.clack_env
            * 1.45;

        // Loud, and it belongs loud. Everything else in this mix is weather happening
        // around you — wind at 2.2, water at 3.4 — while this is the one voice that is
        // *you*, a hand's width away, and gear a metre from your ear is not background.
        // The pawls were at 0.5 against flogging canvas at 0.9, which had a winch being
        // ground in quieter than a sail shaking two boat-lengths forward.
        let gear = (tooth * 1.15 + rasp * 0.62) * self.smooth.gear + clack_out;

        // --- Flogging canvas: gated bursts with a hard attack ----------------------
        let flog_hz = flog_hz.clamp(0.5, 14.0);
        self.flog_phase += flog_hz * dt;
        if self.flog_phase >= 1.0 {
            self.flog_phase -= 1.0;
            self.flog_env = 1.0;
        }
        self.flog_env *= 0.9994;
        let n3 = self.noise();
        let flog = n3 * self.flog_env * self.flog_env * self.smooth.flog * 0.9;

        let mix = (wind + rig + water + flog + rain + gear) * MASTER * t_master * self.smooth.mute;
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
pub fn setup(mut commands: Commands, mut assets: ResMut<Assets<SeaVoice>>) {
    let params = Arc::new(SeaParams::default());
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

pub fn plugin(app: &mut App) {
    app.add_audio_source::<SeaVoice>();
}

/// Diameter of a shroud, metres. Sets the pitch of the whine through the Strouhal
/// relation — thinner wire sings higher, which is true and free.
const SHROUD_DIAMETER: f32 = 0.006;
const STROUHAL: f32 = 0.2;

/// Feed the synthesiser from the boat.
/// What the lines were doing last frame, so this one can tell how fast they moved.
#[derive(Default)]
pub struct Worked {
    lines: [f64; 6],
    boom: f64,
    started: bool,
}

pub fn update(
    sound: Res<SoundHandle>,
    sim: Res<Sim>,
    rain: Res<crate::rain::Rain>,
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

    // Water at the hull: speed through the water, since that is what the hull feels.
    let stw = ms_to_kn(t.stw) as f32;
    store(&p.water_level, (stw / 7.0).clamp(0.0, 1.3).powf(1.4));
    store(&p.water_cutoff, 260.0 + 150.0 * stw.min(9.0));

    // Flogging: the worse-behaved of the two sails, and faster in more wind.
    let luffing = t
        .main
        .coeffs
        .luffing
        .max(t.jib.coeffs.luffing * (1.0 - sim.controls.furl)) as f32;
    store(&p.flog_level, luffing.clamp(0.0, 1.0));
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
    store(&p.gear_level, (rate * 1.7).min(1.0));
    // Pawls per second. A two-speed winch at a hard grind clicks about twenty times a
    // second; eased by hand it is a slow tick you can count.
    store(&p.gear_hz, 3.0 + 26.0 * rate.min(1.2));
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
