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
    muted: AtomicU32,
}

fn store(slot: &AtomicU32, v: f32) {
    slot.store(v.to_bits(), Ordering::Relaxed);
}

fn load(slot: &AtomicU32) -> f32 {
    f32::from_bits(slot.load(Ordering::Relaxed))
}

impl SeaParams {
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
        let (t_wind, t_wind_cut, t_rig, t_rig_hz, t_water, t_water_cut, t_flog, flog_hz, t_mute) = {
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

        let mix = (wind + rig + water + flog) * MASTER * self.smooth.mute;
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
pub fn update(sound: Res<SoundHandle>, sim: Res<Sim>, keys: Res<ButtonInput<KeyCode>>) {
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
}
