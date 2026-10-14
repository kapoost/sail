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

#[derive(Resource, Default)]
pub struct Net {
    pub link: Option<Link>,
    /// What went wrong, for the panel. Empty while things are well.
    pub trouble: String,
    pub joined: bool,
    /// Where the panel will knock, and what to call yourself when it does.
    pub address: String,
    pub name: String,
    /// Looking for games on this network, while the panel's page is open. Dropping it
    /// stops the thread.
    pub scan: Option<sail_net::discovery::Scan>,
}

pub fn setup(mut commands: Commands) {
    let mut net = Net {
        name: std::env::var("SAIL_NAME").unwrap_or_else(|_| whoami()),
        ..Default::default()
    };
    if let Ok(server) = std::env::var("SAIL_JOIN") {
        let name = net.name.clone();
        net.address = server.clone();
        match Link::join(&server, &name) {
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

/// A name over the boat, taken from whatever the machine already knows about its owner
/// rather than asked for in a dialogue nobody wants to fill in.
fn whoami() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "somebody".into())
}

pub fn exchange(mut net: ResMut<Net>, mut fleet: ResMut<Fleet>, mut sim: ResMut<Sim>) {
    let Some(link) = net.link.as_mut() else {
        return;
    };
    // Ours, every frame. The thread sends fifteen a second out of whatever it has been
    // given most recently, so handing it a snapshot per frame costs a channel push.
    link.send(Drawn::of(&sim.sim.state, &sim.controls));

    let events = link.drain();
    for event in events {
        match event {
            Event::Joined { id, world, sky } => {
                net.joined = true;
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
                // Safe to snap: the simulation clock drives the wave field, the gusts and
                // the squalls, and nothing else. The passage deadline counts frame time
                // and the time of day has its own hour.
                sim.sim.state.time = sky.t as f64;
                if world.seed != sim.weather.seed {
                    // Not fixed here on purpose. Rebuilding the archipelago under a boat
                    // that is sailing in it is a different piece of work — the islands,
                    // the sea texture and the consignment all come off the seed — and
                    // doing it badly would be worse than saying so. Start both with
                    // `SAIL_SEED` matching the server's and this line never appears.
                    warn!(
                        "this game is seed {} and that one is seed {} — different \
                         archipelagos. Start with SAIL_SEED={}",
                        sim.weather.seed, world.seed, world.seed
                    );
                    net.trouble = "different archipelago — see the console".into();
                }
            }
            Event::Boat { id, name, drawn } => fleet.heard(id, &name, drawn),
            Event::Gone(id) => fleet.gone(id),
            Event::Weather { sky, squalls } => {
                if sky.t <= 0.0 {
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
