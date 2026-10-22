//! Being on somebody else's water.
//!
//! The thin half of the network: the socket and its thread live in `sail-net`, which
//! knows nothing about Bevy, and this is the dozen lines that hand a snapshot out and
//! drop what comes back into the fleet.
//!
//! `SAIL_JOIN=192.168.1.20:7714` joins a game; without it nothing in this file does
//! anything at all, which is the way round it has to be. A sailing game that cannot be
//! played alone because a server is down is a worse game than one with no server in it.
//!
//! # What the far end is allowed to decide
//!
//! The weather, and nothing else. The wind is a process with state, so two machines
//! stepping their own cannot be relied on to stay in the same breeze, and being in
//! different winds is the one divergence that would actually spoil this — you would be
//! watching somebody sail beautifully in air that is not there. Everything else about the
//! world is a pure function of the seed, so it needs no packets, and everything about a
//! boat is decided by the person sailing her, because fifty milliseconds on the tiller is
//! felt immediately.

use bevy::prelude::*;
use sail_net::link::{Event, Link};
use sail_net::Drawn;

use crate::fleet::Fleet;
use crate::sim::Sim;

/// The wire to everybody else.
///
/// What this module does in a frame, in one place — see `stages.rs` for why the
/// schedule is named sets rather than one long chain.
pub fn plugin(app: &mut App) {
    use crate::stages::Build;
    app.add_systems(Startup, setup.in_set(Build::Last));
}

#[derive(Resource, Default)]
pub struct Net {
    /// The leg the relay has called, if it has called one.
    ///
    /// Held here rather than written straight into the `Course` because the packet
    /// reader has no business owning the consignment system: it takes what arrived and
    /// a system beside the course takes it from here. The same move as `rebuild`.
    pub leg: Option<sail_net::Passage>,
    /// Who is in on this leg and how long it took them, newest first.
    pub finished: Vec<(sail_net::BoatId, String, f32)>,
    pub link: Option<Link>,
    /// What went wrong, for the panel. Empty while things are well.
    pub trouble: String,
    pub joined: bool,
    /// Where the panel will knock, and what to call yourself when it does.
    pub address: String,
    pub name: String,
    /// Whether the address in the box came from the environment rather than from the
    /// person. A borrowed address is not written back to the file.
    pub borrowed: bool,
    /// The word the far end wants, if it wants one. A doorbell rather than a password —
    /// it travels in the clear — so it is kept with the address rather than treated as
    /// a credential, and it is never written to the log.
    pub word: String,
    /// Knocking on one typed address, and what came back. Not the same thing as the
    /// scan: the scan shouts at the wifi, this asks a server somewhere by name.
    pub probe: Option<sail_net::discovery::Probe>,
    /// What the knock found, kept after the probe is dropped so the answer stays on the
    /// screen rather than flashing past.
    pub knocked: Option<Result<sail_net::discovery::Found, String>>,
    /// Looking for games on this network, while the panel's page is open. Dropping it
    /// stops the thread.
    pub scan: Option<sail_net::discovery::Scan>,
    /// A world to go to, set by the handshake and taken by `rebuild`.
    pub rebuild: Option<sail_net::World>,
}

pub fn setup(mut commands: Commands) {
    // What the last session was sailing on. An address and a word retyped every time is
    // the kind of friction that quietly ends a thing like this.
    let (remembered_name, address, word) = sail_net::remembered::read();
    let mut net = Net {
        name: crate::knob::var("SAIL_NAME").unwrap_or_else(|_| {
            if remembered_name.is_empty() {
                whoami()
            } else {
                remembered_name.clone()
            }
        }),
        address,
        word,
        ..Default::default()
    };
    if let Ok(server) = crate::knob::var("SAIL_JOIN") {
        let name = net.name.clone();
        net.address = server.clone();
        // Told where to go by the environment — by the setup screen, or by a line in a
        // terminal — which is somebody trying something rather than somebody changing
        // where they sail. It goes in the box so it can be seen, and it does not go in
        // the file: one experiment from the launcher should not quietly replace the
        // address a person has been using all week.
        net.borrowed = true;
        if let Ok(word) = crate::knob::var("SAIL_WORD") {
            net.word = word;
        }
        let word = net.word.clone();
        match Link::join(&server, &name, &word) {
            Ok(link) => {
                info!("knocking on {server} as {name}");
                net.link = Some(link);
            }
            Err(e) => {
                warn!("cannot join {server}: {e}");
                net.trouble = e.to_string();
            }
        }
    }
    commands.insert_resource(net);
}

/// Everything in the scene that came off the old seed and therefore has to go.
type OfTheOldWorld = Or<(
    With<crate::islands::Landmass>,
    With<crate::course::Floating>,
)>;

/// Move to the world the game is being played in.
///
/// Everything that comes off the seed goes: the archipelago, the harbours, the
/// consignment, the track she has drawn so far. What stays is the boat herself — her
/// class, her rig, where her sheets are — because that is the player's and not the
/// world's.
///
/// Done in one frame and visibly: she is somewhere else afterwards, in weather that is
/// not the weather she cast off in. That is the honest presentation of what joining a
/// game actually is, and the alternative — quietly sailing on in a different sea from
/// everybody else — was what this replaced.
#[allow(clippy::too_many_arguments)]
pub fn rebuild(
    mut commands: Commands,
    mut net: ResMut<Net>,
    mut sim: ResMut<Sim>,
    mut clock: ResMut<crate::sky::Clock>,
    mut track: ResMut<crate::chart::Track>,
    mut nav_state: ResMut<crate::navigation::Navigation>,
    mut chart: ResMut<crate::chart::Chart>,
    mut log: ResMut<crate::chart::DeckLog>,
    old: Query<Entity, OfTheOldWorld>,
    mut sea: ResMut<crate::islands::Sea>,
) {
    let Some(world) = net.rebuild.take() else {
        return;
    };
    // The day as that machine drew it, over the day this one drew. The parts that are a
    // property of the *world* come across; the parts that are a property of this boat's
    // first minute — which tack she starts on, how much she starts reefed — stay, because
    // nobody else has an opinion about those.
    let mut weather = crate::weather::Weather::draw(world.seed);
    weather.tws = world.tws as f64;
    weather.wind_from = world.wind_from as f64;
    weather.fetch = world.fetch as f64;
    weather.start_hour = world.start_hour as f64;
    weather.cover = world.cover as f64;
    let (hs, tp) = sail_core::water::fetch_limited_sea(weather.tws, weather.fetch);
    weather.hs = hs;
    weather.tp = tp;

    // Their sea, not ours: joining somebody means sailing in the archipelago their copy
    // generated, and the islands have to be the same islands or nothing either of us says
    // about the water is true.
    //
    // **The platform comes with you**, which is the one thing that does. It is re-moored
    // in whatever water this is and the store on it is untouched — see `anchorage.rs`,
    // where that is also how you get home again.
    *sim = Sim::new(weather, world.seed);
    *sea = crate::islands::Sea(world.seed);
    clock.follow_the_passage();
    track.clear();
    // Out with the old archipelago. The marks go with it: a consignment to a harbour that
    // is no longer there is a buoy floating in open water.
    for entity in &old {
        commands.entity(entity).despawn();
    }
    // And in with the new, through the same two systems that built the first one. Running
    // them again rather than writing a second version is the only way the joined world
    // and the solo one cannot drift apart.
    commands.run_system_cached(crate::islands::setup);
    commands.run_system_cached(crate::course::setup);
    // **And nobody knows where they are.**
    //
    // A world replaced under you is a coast you have never seen, so the chart stops
    // saying where she is until somebody works it out — which is what makes resetting
    // the game from the launcher a thing worth doing to a fleet rather than an
    // inconvenience. The boat has already been put somewhere by the new simulation
    // above; this is the half that happens to the paper. See `navigation::cast_adrift`.
    let now = sim.sim.state.time;
    crate::navigation::cast_adrift(&mut nav_state, &mut chart, &mut log, &mut track, now);
    info!(
        "sailing in seed {} now — and nobody knows where they are",
        world.seed
    );
}

/// A name over the boat, taken from whatever the machine already knows about its owner
/// rather than asked for in a dialogue nobody wants to fill in.
fn whoami() -> String {
    crate::knob::var("USER")
        .or_else(|_| crate::knob::var("USERNAME"))
        .unwrap_or_else(|_| "somebody".into())
}

pub fn exchange(
    mut net: ResMut<Net>,
    mut fleet: ResMut<Fleet>,
    mut sim: ResMut<Sim>,
    mut clock: ResMut<crate::sky::Clock>,
    sound: Res<crate::sound::SoundHandle>,
) {
    let Some(link) = net.link.as_mut() else {
        return;
    };
    // Ours, every frame. The thread sends fifteen a second out of whatever it has been
    // given most recently, so handing it a snapshot per frame costs a channel push.
    link.send(Drawn::of(&sim.sim.state, &sim.controls, &sim.sim.telemetry));

    let events = link.drain();
    for event in events {
        match event {
            Event::Joined {
                id,
                world,
                sky,
                passage,
            } => {
                // The leg everybody is on, taken whether or not the sea is changing —
                // see the rebuild below, which only happens when the seed differs.
                net.leg = passage;
                net.joined = true;
                net.trouble.clear();
                info!("joined as boat {id}");
                // The day as that machine drew it. The trends the weather wanders on are
                // measured against this base, so without taking it over, following the
                // far end's wind would mean asking our own process for a speed it is
                // clamped out of reaching — a boat in a drawn force 3 cannot follow a
                // force 7 by trend alone, and the two would sit in different weather
                // while both believed they were synchronised.
                if let Some(w) = sim.sim.wind.adjustable() {
                    w.synoptic.base_speed = world.tws as f64;
                    w.synoptic.base_direction = world.wind_from as f64;
                }
                // And the clock, which matters more than it looks.
                //
                // "The gust field needs no packets because it is a stateless function of
                // position and time" is true of the position and was *false of the time*:
                // two machines that started four minutes apart were evaluating the same
                // function at different `t` and therefore making different gusts. Both
                // would have been computing them perfectly and neither would have been
                // sailing in the other's air — which is the exact thing this was all
                // supposed to prevent, hiding inside the sentence that claimed to prevent
                // it.
                //
                // Safe to snap: the simulation clock drives the wave field, the gusts,
                // the squalls — and now the time of day as well, which is new and is the
                // point. The sun used to have a number of its own that advanced itself,
                // so joining an hour into somebody's passage put you in your own morning
                // while they watched the afternoon. It is read off this clock and the
                // hour the passage began, both of which are shared, so it comes right
                // here with no packet of its own.
                //
                // The start hour is taken whether or not the sea is changing: joining a
                // game on the seed you are already sailing skips the rebuild below, and
                // the hour would otherwise stay this machine's while the clock became
                // theirs — which is a worse answer than either end alone.
                sim.weather.start_hour = world.start_hour as f64;
                clock.follow_the_passage();
                // Guarded here too: this arm is the handshake and the one above is the
                // hourly weather, and they are two different packets.
                if sky.t.is_finite() && sky.t > 0.0 {
                    sim.sim.state.time = sky.t as f64;
                }
                if world.seed != sim.weather.seed {
                    // Sail where they are sailing. Handled by its own system rather than
                    // here, because moving to another archipelago means replacing the
                    // simulation, taking the islands and the marks out of the scene and
                    // building the new ones — which is five resources and two queries,
                    // and threading all of that through the packet reader would make the
                    // packet reader about something else.
                    info!("their sea is seed {} — going there", world.seed);
                    net.rebuild = Some(world);
                }
            }
            Event::Leg(passage) => {
                info!(
                    "leg: harbour {} to harbour {}, called at {:.0} s",
                    passage.from, passage.to, passage.began
                );
                net.leg = Some(passage);
            }
            Event::Home { id, name, seconds } => {
                // Kept newest first and short: this is a scoreboard for one leg, not a
                // table of records, and a card on a console has room for three names.
                net.finished.retain(|(who, _, _)| *who != id);
                net.finished.insert(0, (id, name.clone(), seconds));
                net.finished.truncate(8);
                info!(
                    "{name} is in — {:.0}m{:02.0}s",
                    seconds / 60.0,
                    seconds % 60.0
                );
                sound.0.ring(2);
            }
            Event::Boat { id, name, drawn } => {
                // Two strokes for a boat arriving: it is the one thing that happens out
                // here which nobody on deck can see coming, and a bell is what a boat
                // already has for telling somebody something.
                if fleet.heard(id, &name, drawn) {
                    info!("{name} is on the water");
                    sound.0.ring(2);
                }
            }
            Event::Gone(id) => {
                if fleet.gone(id) {
                    sound.0.ring(1);
                }
            }
            Event::Weather { sky, squalls } => {
                // **Written so that NaN fails it**, and spelled out rather than
                // negated: `sky.t <= 0.0` is `false` for NaN,
                // so the old spelling let one through — and a NaN here is not a glitch
                // that passes, it is permanent: the next lines nudge the simulation clock
                // by five percent of the error, the error is NaN, and the clock that
                // drives the wave field, the gusts and the squalls is NaN from then on
                // with no path back to a number.
                //
                // The wire refuses it now (`Sane` in `sail-net`) and this is the second
                // lock on the one value in the program that cannot recover from a bad
                // one. Two locks on this and none on anything else is the right shape:
                // every other number here is a drawing.
                if !sky.t.is_finite() || sky.t <= 0.0 {
                    continue;
                }
                // Through the same seam the weather sliders use. The first version of
                // this wrote the means directly and did *nothing at all*: the synoptic
                // process sets `mean_speed` from its own trend on every substep, two
                // hundred times a second, so the server's number survived for five
                // milliseconds and was then overwritten by the weather this machine was
                // having anyway. Reported from the deck as the server holding the
                // weather, which was exactly backwards — it was not holding it at all.
                //
                // `follow` sets the trends the dice would have set, so the correction
                // lands in the one place the weather is actually decided and the process
                // carries on from there between packets.
                // And the clocks stay together. Both machines advance real time at one
                // to one, so the error is only ever what a stutter left behind — nudged
                // rather than snapped, because a jump in `t` is a jump in the wave field
                // and you would see the sea flinch.
                let error = sky.t as f64 - sim.sim.state.time;
                if error.abs() > 3.0 {
                    sim.sim.state.time = sky.t as f64;
                } else {
                    sim.sim.state.time += 0.05 * error;
                }
                if let Some(w) = sim.sim.wind.adjustable() {
                    w.follow(
                        sky.tws as f64,
                        sky.wind_from as f64,
                        sky.cover as f64,
                        squalls.iter().map(|s| s.into_core()).collect(),
                    );
                }
            }
            // Six seconds of silence, and the link is knocking again rather than
            // giving up. Said out loud because the boats are about to vanish off the
            // water and a person watching that happen deserves to know it is being
            // dealt with — this is what a server restarting looks like from here.
            Event::Quiet => {
                info!("nothing from the game for six seconds — knocking again");
                net.joined = false;
                net.trouble = "the game went quiet — knocking again".into();
            }
            Event::Lost(why) => {
                warn!("network: {why}");
                net.trouble = why;
                net.joined = false;
                // The thread is gone, so the link is a handle to nothing. Dropping it is
                // what puts the panel back to the list of games rather than leaving it
                // saying "knocking..." at a socket nobody is holding.
                net.link = None;
            }
        }
    }
}
