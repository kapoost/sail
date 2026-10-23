//! The band, recorded without a window.
//!
//!     SAIL_BAND=music.wav SAIL_BAND_FOR=50 sail
//!
//! **A cut needs music the length of the cut, and the band only plays in a game.** The
//! soundtrack here is not a file to be dropped under a video — it is made out of the
//! weather by `music.rs`, note by note, in response to a boat that is being sailed. So
//! the only honest way to get fifty seconds of it is to sail fifty seconds and listen,
//! and until now that meant recording the screen with a loopback device and hoping.
//!
//! This does it the other way round: no window, no audio device, no real time. A
//! headless world is built, the weather is walked through the demo's scenes, the same
//! `music::compose` that plays on the deck is run against it sixty times a second, and
//! the same synthesiser the deck hears is turned by hand — forty-four thousand samples a
//! second pulled straight out of it and written to a file. Thirty seconds of work for a
//! minute of music, and it is the *same* music: the same composer, the same voices, the
//! same decisions about mode and metre.
//!
//! # What is in the mix, and what is not
//!
//! The band, **the gear and the watch** — and the sea only as a whisper. Which is the
//! same decision the demo's own mixer makes and for the same reason: the sea is this
//! program's loudest voice and the worst thing to put under a cut, while a winch, a
//! furling drum and a reef line being hauled are the sounds that say a boat is a
//! machine being worked.
//!
//! None of it is faked. The lines are hauled through `clutch::handle` at the rates the
//! real gear runs at, and `sound::update` hears them the way it always does: by
//! differencing the controls frame to frame. A drum takes twelve seconds here because a
//! drum takes twelve seconds.
//!
//! **The watch calls through `hail::place`**, so the calls land in the music's own
//! rhythm — slightly before or after the beat, which is what was built for them. They
//! are silent unless `SAIL_VOICE` points at a directory of recordings, and that is the
//! correct default: the ones on this machine are public-domain stand-ins, fine on the
//! deck and not fine in a film anybody else will see. Record your own and they appear,
//! in rhythm, with nothing else to change.
//!
//! # The scenes are the cut's, not the reel's
//!
//! `SAIL_REEL=demo` is a hundred and thirty-eight seconds of *take*, out of which about
//! fifty seconds of film is cut. Music written against the take would change key eight
//! seconds before a cut that is not there any more. So the table below is the finished
//! film's own shape — dawn, a breeze and a mark, fog, a gale, an evening — at the
//! lengths they ended up being, and the band is asked for exactly that.
//!
//! Change the cut and change this, which is the same thing a composer does.

use std::io::Write;
use std::sync::Arc;

use bevy::prelude::*;

use crate::sim::Sim;
use crate::sound::{SeaParams, SeaStream, SoundHandle};

/// A stretch of the finished film, and the weather the band is to play it against.
struct Scene {
    /// Seconds into the cut that this takes over.
    at: f64,
    /// Beaufort, and the hour of the day — the two things the composer reads the mood
    /// off. See `music::asking`.
    force: u8,
    oktas: u8,
    hour: f32,
    /// How she is lying to the wind, degrees, signed: it decides the metre.
    twa: f64,
}

/// The demo cut, scene by scene. The seconds are where each shot lands in the finished
/// film and nowhere else — see the module note.
const CUT: &[Scene] = &[
    // Dawn on flat water. Slow, thin, and the only part of the film with no boat work
    // in it.
    Scene {
        at: 0.0,
        force: 2,
        oktas: 1,
        hour: 4.6,
        twa: 60.0,
    },
    // The beat to the mark and the bear-away: fourteen seconds of the only scene with
    // other boats in it, and the one place the film has a *race* in it.
    Scene {
        at: 7.0,
        force: 4,
        oktas: 3,
        hour: 9.5,
        twa: 45.0,
    },
    // Round the mark and away. The metre changes here on its own, because the point of
    // sail did — which is the whole argument of `music.rs` in one cut.
    Scene {
        at: 14.0,
        force: 4,
        oktas: 3,
        hour: 9.8,
        twa: 135.0,
    },
    // Fog: a calm, and the band thins with the visibility.
    Scene {
        at: 21.0,
        force: 2,
        oktas: 6,
        hour: 10.2,
        twa: 110.0,
    },
    // The gale, running off.
    Scene {
        at: 28.0,
        force: 8,
        oktas: 7,
        hour: 11.5,
        twa: 145.0,
    },
    // And the evening at the platform, which is where it lands.
    Scene {
        at: 39.5,
        force: 3,
        oktas: 2,
        hour: 18.3,
        twa: 120.0,
    },
];

/// A job the watch does, at a second of the film.
///
/// Only the ones the picture shows. A soundtrack with a winch in it over a shot of a
/// boat doing nothing is worse than a silent one — it tells the ear a lie the eye can
/// check.
enum Job {
    /// Haul the furling line to here: nought set, one rolled away.
    Furl(f64),
    /// And the reef line, in slabs of a quarter.
    Reef(f64),
    /// A boom across, a clutch thrown: one sharp sound.
    Clack(f32),
    /// Somebody says something. Silent without recordings — see the module note.
    Say(crate::sound::Call),
}

/// What is being done on deck, when. The seconds are the film's.
const WORK: &[(f64, Job)] = &[
    // The approach to the mark: the genoa comes in, which is the twelve seconds of drum
    // the picture spends with the forestay clearing.
    (8.0, Job::Say(crate::sound::Call::Report)),
    (8.4, Job::Furl(1.0)),
    (12.0, Job::Say(crate::sound::Call::Effort)),
    // Round the mark. The boom goes across as she bears away, and the kite goes up out
    // of the turn.
    (14.0, Job::Clack(0.8)),
    (14.3, Job::Say(crate::sound::Call::Aye)),
    (15.0, Job::Furl(0.0)),
    (18.0, Job::Say(crate::sound::Call::Effort)),
    // Fog, and nothing to do in it but listen.
    // Then snugging down, which happens *before* the gale in the film and in life.
    (25.5, Job::Say(crate::sound::Call::Warn)),
    (26.0, Job::Furl(1.0)),
    (26.2, Job::Reef(0.75)),
    (29.0, Job::Say(crate::sound::Call::Effort)),
    (33.0, Job::Say(crate::sound::Call::Report)),
    // And her evening clothes, at the platform.
    (41.0, Job::Reef(0.0)),
    (41.4, Job::Furl(0.0)),
    (44.0, Job::Say(crate::sound::Call::Aye)),
];

/// How long the whole thing runs if nobody says otherwise: the length of the cut.
const RUNS_FOR: f64 = 49.5;

/// One step of the world, and of the band. Sixty a second, which is what the deck gets.
const STEP: f64 = 1.0 / 60.0;

/// Whether a recording was asked for, and where it goes.
pub fn asked() -> Option<(String, f64)> {
    let path = crate::knob::var("SAIL_BAND").ok()?;
    let seconds = crate::knob::var("SAIL_BAND_FOR")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(RUNS_FOR);
    Some((path, seconds))
}

/// Sail the cut with nobody watching, and write down what the band played.
pub fn record(path: &str, seconds: f64, seed: u64) -> std::io::Result<()> {
    // The same block the deck is handed, voices and all — see its own note.
    let params = Arc::new(SeaParams::as_handed_to_the_deck());
    // The mix this is for — the same four numbers the demo's own first beat sets, and
    // for the same reasons. See the module note.
    params.set_master(1.0);
    params.set_music(1.0);
    params.set_boat(1.0);
    params.set_crew(0.9);
    params.set_sea(0.05);

    let mut world = App::new();
    world.add_plugins(MinimalPlugins);
    world.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_secs_f64(STEP),
    ));
    world.insert_resource(Sim::new(crate::weather::Weather::draw(seed), seed));
    world.insert_resource(SoundHandle(params.clone()));
    world.init_resource::<crate::sky::Sky>();
    world.init_resource::<crate::world::Ink>();
    world.init_resource::<crate::input::Pilot>();
    world.insert_resource(crate::fishing::Fishing::new(seed));
    world.init_resource::<crate::course::Course>();
    world.init_resource::<crate::music::Music>();
    world.init_resource::<crate::clutch::Bank>();
    world.init_resource::<crate::hail::Hail>();
    world.init_resource::<ButtonInput<KeyCode>>();
    // The order is the order it happens in: hands on the lines, then the boat answers,
    // then the ear is told what it all sounded like, then the band plays over it, then
    // whatever the watch had to say is fitted into the rhythm the band just set.
    world.add_systems(
        Update,
        (
            crate::clutch::handle,
            crate::sim::advance,
            crate::sound::update,
            crate::music::compose,
            crate::hail::place,
        )
            .chain(),
    );

    // The synthesiser, turned by hand. Same struct the audio thread is handed; it does
    // not know or care that nothing is listening.
    let mut band = SeaStream::new(params.clone());
    let per_step = (crate::sound::SAMPLE_RATE as f64 * STEP).round() as usize;
    let mut pcm: Vec<f32> = Vec::with_capacity((seconds * 44_100.0) as usize + per_step);

    let mut at = 0.0;
    let mut scene = usize::MAX;
    let mut done = 0usize;
    while at < seconds {
        // Which scene the film is in, set once when it changes rather than every step:
        // `blows` and `start_cover` solve for a trend, and solving for it sixty times a
        // second is a weather system that never gets anywhere.
        let now = CUT
            .iter()
            .rposition(|s| s.at <= at)
            .expect("the cut starts at nought");
        if now != scene {
            scene = now;
            set_the_weather(&mut world, &CUT[now]);
        }
        while done < WORK.len() && WORK[done].0 <= at {
            set_to_work(&mut world, &WORK[done].1);
            done += 1;
        }
        world.update();
        for _ in 0..per_step {
            pcm.push(band.next().unwrap_or(0.0));
        }
        at += STEP;
    }

    let (played, lost) = params.notes();
    let peak = pcm.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    // Printed rather than logged: this runs with no log plugin in the world, and a
    // line that goes to a subscriber nobody installed is a line nobody reads.
    println!("band: {at:.1} s, {played} notes ({lost} lost for want of a voice), peak {peak:.2}");
    write_wav(path, &pcm)
}

/// Put the weather where a scene wants it, through the same two doors the weather page
/// and the reel use. Nothing here reaches past the model.
fn set_the_weather(world: &mut App, scene: &Scene) {
    let wanted = sail_core::units::kn_to_ms(crate::weather::force_wind(scene.force));
    let mut sim = world.world_mut().resource_mut::<Sim>();
    if let Some(wind) = sim.sim.wind.adjustable() {
        wind.synoptic.blows(wanted);
        wind.synoptic.start_cover(scene.oktas as f64 / 8.0);
    }
    if let Some(sea) = sim.sim.water.adjustable() {
        sea.settle(wanted, 0.0);
    }
    // The hour is the composer's second input and there is no sky system running here,
    // so it is set rather than drifted into place.
    let mut sky = world.world_mut().resource_mut::<crate::sky::Sky>();
    *sky = crate::sky::Sky::at(scene.hour, scene.oktas as f32 / 8.0);
    // And how she is lying, which is what decides the metre. Straight on to the helm
    // rather than through a pilot, because there is no helmsman in this world and the
    // only thing that matters is the angle the music is told about.
    let mut sim = world.world_mut().resource_mut::<Sim>();
    let twd = sim.sim.telemetry.twd;
    sim.sim.state.heading =
        sail_core::units::wrap_tau(twd + std::f64::consts::PI - sail_core::units::rad(scene.twa));
}

/// Put a hand on a line, or a word in somebody's mouth.
///
/// Through the same doors the deck uses: the clutch bank hauls at the gear's own rate
/// and `sound::update` hears it by differencing the controls, exactly as it does when a
/// player holds the key down.
fn set_to_work(world: &mut App, job: &Job) {
    use crate::clutch::{Line, BANK};
    let at = |line: Line| BANK.iter().position(|l| *l == line);
    // **The deck is working, and something has to say so.** `Hail` keeps two silences —
    // forty seconds on a leg, three and a half with a job on — and the one that decides
    // which is `crew::work`, which is not in this world: there is nobody aboard, the
    // lines are hauled by the table above. Without this, six of seven calls were
    // refused as too soon after the last and a film about a sail change had one word in
    // it. The jobs below *are* the work, so they say so.
    if matches!(job, Job::Furl(_) | Job::Reef(_) | Job::Clack(_)) {
        world
            .world_mut()
            .resource_mut::<crate::hail::Hail>()
            .working();
    }
    match job {
        Job::Furl(to) => {
            if let Some(which) = at(Line::Furl) {
                world
                    .world_mut()
                    .resource_mut::<crate::clutch::Bank>()
                    .target[which] = Some(*to);
            }
        }
        Job::Reef(to) => {
            if let Some(which) = at(Line::Reef) {
                world
                    .world_mut()
                    .resource_mut::<crate::clutch::Bank>()
                    .target[which] = Some(*to);
            }
        }
        Job::Clack(how_hard) => {
            world.world().resource::<SoundHandle>().0.clack(*how_hard);
        }
        Job::Say(kind) => {
            // Queued rather than shouted: `hail::place` decides when it lands, against
            // the beat the band is playing.
            world
                .world_mut()
                .resource_mut::<crate::hail::Hail>()
                .call(*kind);
        }
    }
}

/// Sixteen-bit mono PCM, which is what every editor on earth will open.
///
/// Written by hand because the alternative is a crate for forty-four bytes of header,
/// and this file is the only place in the program that writes one. Reading them, which
/// is a different job with more ways to be wrong, is in `sound.rs`.
fn write_wav(path: &str, pcm: &[f32]) -> std::io::Result<()> {
    let rate = crate::sound::SAMPLE_RATE;
    let bytes = pcm.len() * 2;
    let mut out = std::io::BufWriter::new(std::fs::File::create(path)?);
    out.write_all(b"RIFF")?;
    out.write_all(&((36 + bytes) as u32).to_le_bytes())?;
    out.write_all(b"WAVEfmt ")?;
    out.write_all(&16u32.to_le_bytes())?; // PCM header length
    out.write_all(&1u16.to_le_bytes())?; // PCM, uncompressed
    out.write_all(&1u16.to_le_bytes())?; // one channel
    out.write_all(&rate.to_le_bytes())?;
    out.write_all(&(rate * 2).to_le_bytes())?; // bytes a second
    out.write_all(&2u16.to_le_bytes())?; // bytes a frame
    out.write_all(&16u16.to_le_bytes())?; // bits a sample
    out.write_all(b"data")?;
    out.write_all(&(bytes as u32).to_le_bytes())?;
    for sample in pcm {
        let clipped = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        out.write_all(&clipped.to_le_bytes())?;
    }
    out.flush()
}

#[cfg(test)]
mod the_band_plays_to_the_cut {
    use super::*;

    /// **The table is the film's running order**, so it has to run forwards and start
    /// where the film does. A scene out of order would be looked up by `rposition` and
    /// silently never played.
    #[test]
    fn the_scenes_are_in_order() {
        assert_eq!(CUT[0].at, 0.0, "the cut does not start at nought");
        for pair in CUT.windows(2) {
            assert!(
                pair[1].at > pair[0].at,
                "a scene at {} comes after {}",
                pair[1].at,
                pair[0].at
            );
        }
        let last = CUT.last().expect("a cut has scenes in it");
        assert!(
            last.at < RUNS_FOR,
            "the last scene starts at {} and the film is {RUNS_FOR} long",
            last.at
        );
    }

    /// **The deck's work runs forwards too**, and for a worse reason than the scenes:
    /// the jobs are played off a cursor that only ever moves on, so one out of order is
    /// one that never happens — a reef that is never tied, in silence, with nothing to
    /// show for it.
    #[test]
    fn the_work_is_in_order_and_inside_the_film() {
        let mut last = -1.0;
        for (at, _) in WORK {
            assert!(*at >= last, "a job at {at} comes after {last}");
            assert!(*at < RUNS_FOR, "a job at {at} is after the film has ended");
            last = *at;
        }
    }

    /// And the header says what the body is. Forty-four bytes, and every editor that
    /// opens this will believe whatever they say — including, if they are wrong, that
    /// the file is four times its length and ends in noise.
    #[test]
    fn the_header_describes_the_body() {
        let dir = std::env::temp_dir().join("sail-band-test.wav");
        let path = dir.to_string_lossy().to_string();
        let pcm: Vec<f32> = (0..1_000).map(|i| (i as f32 / 50.0).sin() * 0.5).collect();
        write_wav(&path, &pcm).expect("written");
        let raw = std::fs::read(&path).expect("read back");
        assert_eq!(&raw[0..4], b"RIFF");
        assert_eq!(&raw[8..12], b"WAVE");
        let data = u32::from_le_bytes(raw[40..44].try_into().unwrap()) as usize;
        assert_eq!(data, pcm.len() * 2, "the data chunk is the wrong length");
        assert_eq!(
            raw.len(),
            44 + data,
            "the file is not its own header's size"
        );
        let _ = std::fs::remove_file(&path);
    }
}
