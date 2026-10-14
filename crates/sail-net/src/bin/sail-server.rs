//! The relay, the clock and the lobby — and nothing else.
//!
//! It does not simulate a boat, it does not know what a sail is, and it cannot tell you
//! whether anybody's trim is any good. It takes a snapshot from one boat and hands it to
//! the others, it owns the weather, and it tells a new arrival which sea everybody is in.
//! That is the whole program, and it is deliberately the whole program: every line that
//! would make it authoritative over a boat is a line that would put a round trip between
//! somebody's hand and their rudder.
//!
//! No Bevy, no graphics, no assets. It builds in a few seconds and runs on the smallest
//! virtual machine anybody rents, which is what makes the internet milestone cheap.
//!
//! ```text
//! sail-server                 # a game on the default port, a seed from the clock
//! sail-server --seed 42       # the same archipelago every time
//! sail-server --port 7800
//! ```

use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant, SystemTime};

use sail_core::wind::{SyntheticWind, WindField};
use sail_net::{
    decode, encode, BoatId, Drawn, Sky, Squall, ToClient, ToServer, World, PORT, PROTOCOL,
};

/// Nothing heard for this long and she is gone. Six seconds is ninety missed snapshots:
/// not a dropped packet, a closed window.
const FORGOTTEN: Duration = Duration::from_secs(6);

struct Aboard {
    id: BoatId,
    name: String,
    drawn: Drawn,
    heard: Instant,
}

fn main() -> std::io::Result<()> {
    let mut port = PORT;
    let mut seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(1);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--port" => port = args.next().and_then(|v| v.parse().ok()).unwrap_or(PORT),
            "--seed" => seed = args.next().and_then(|v| v.parse().ok()).unwrap_or(seed),
            other => eprintln!("ignoring {other}"),
        }
    }

    let socket = UdpSocket::bind(("0.0.0.0", port))?;
    socket.set_read_timeout(Some(Duration::from_millis(20)))?;
    println!("sail-server on {port}, seed {seed}");

    // The sea everybody will be in. The archipelago is a pure function of this, so it is
    // the whole of what a new arrival needs to be sailing the same water.
    let world = World {
        seed,
        tws: 7.5,
        wind_from: 2.6,
        fetch: 14_000.0,
        start_hour: 8.0,
        cover: 0.4,
    };
    // And the weather, which is the one thing a client is not allowed to decide for
    // itself. Everything else about the world is derivable from the seed and therefore
    // free; the weather is a process with state, and two people in different winds is the
    // one divergence that would actually spoil this.
    let mut wind = SyntheticWind::new(world.wind_from as f64, world.tws as f64, seed ^ 0x5A11);

    let mut fleet: HashMap<SocketAddr, Aboard> = HashMap::new();
    let mut next_id: BoatId = 1;
    let mut buf = [0u8; 2048];
    let began = Instant::now();
    let mut stepped = Instant::now();
    let mut told = Instant::now() - Duration::from_secs(9);

    loop {
        // The weather runs in real time. The clients' own wind follows what comes out of
        // here rather than being stepped locally, so nobody is in yesterday's breeze.
        let dt = stepped.elapsed().as_secs_f64();
        stepped = Instant::now();
        wind.step(dt.min(0.25));
        let t = began.elapsed().as_secs_f32();

        while let Ok((n, from)) = socket.recv_from(&mut buf) {
            let Some(msg) = decode::<ToServer>(&buf[..n]) else {
                continue;
            };
            match msg {
                ToServer::Hello { protocol, name } => {
                    if protocol != PROTOCOL {
                        let no = encode(&ToClient::No {
                            because: format!(
                                "this game is on protocol {PROTOCOL} and you are on \
                                 {protocol} — somebody is on an older build"
                            ),
                        });
                        let _ = socket.send_to(&no, from);
                        continue;
                    }
                    // Knocking twice is normal: the hello repeats once a second until a
                    // welcome arrives, and the welcome can be the thing that was lost.
                    let id = match fleet.get(&from) {
                        Some(already) => already.id,
                        None => {
                            let id = next_id;
                            next_id = next_id.wrapping_add(1).max(1);
                            println!("{name} joined as {id} from {from}");
                            fleet.insert(
                                from,
                                Aboard {
                                    id,
                                    name: name.clone(),
                                    drawn: Drawn::default(),
                                    heard: Instant::now(),
                                },
                            );
                            id
                        }
                    };
                    let welcome = encode(&ToClient::Welcome {
                        protocol: PROTOCOL,
                        your_id: id,
                        world,
                        sky: sky_now(&wind, t),
                    });
                    let _ = socket.send_to(&welcome, from);
                }
                ToServer::Here { id, drawn } => {
                    let Some(boat) = fleet.get_mut(&from) else {
                        continue;
                    };
                    if boat.id != id {
                        continue;
                    }
                    boat.drawn = drawn;
                    boat.heard = Instant::now();
                    // Relayed at the sender's rate rather than on a tick of the server's
                    // own: a snapshot is worth most the instant it arrives, and holding
                    // it for a scheduler would add latency to buy tidiness.
                    let out = encode(&ToClient::Boat {
                        id,
                        name: boat.name.clone(),
                        drawn,
                    });
                    for (addr, other) in &fleet {
                        if other.id != id {
                            let _ = socket.send_to(&out, addr);
                        }
                    }
                }
                ToServer::Leaving { id } => {
                    if fleet.get(&from).map(|b| b.id) == Some(id) {
                        fleet.remove(&from);
                        println!("{id} left");
                        broadcast(&socket, &fleet, &encode(&ToClient::Gone { id }));
                    }
                }
            }
        }

        // The ones who stopped saying anything.
        let lost: Vec<(SocketAddr, BoatId)> = fleet
            .iter()
            .filter(|(_, b)| b.heard.elapsed() > FORGOTTEN)
            .map(|(a, b)| (*a, b.id))
            .collect();
        for (addr, id) in lost {
            fleet.remove(&addr);
            println!("{id} went quiet");
            broadcast(&socket, &fleet, &encode(&ToClient::Gone { id }));
        }

        // The weather, once a second. The gust field is not in here and does not need to
        // be: it is a stateless function of position and time, so every client works out
        // the same gusts from the same seed without a packet. Only the synoptic state and
        // the squalls — which have a position and a radius of their own — travel.
        if told.elapsed() >= Duration::from_secs(1) {
            told = Instant::now();
            let squalls: Vec<Squall> = wind
                .squalls()
                .into_iter()
                .map(|(at, radius, strength)| Squall {
                    pos: [at.x as f32, at.y as f32],
                    radius: radius as f32,
                    strength: strength as f32,
                })
                .collect();
            let weather = encode(&ToClient::Weather {
                sky: sky_now(&wind, t),
                squalls,
            });
            broadcast(&socket, &fleet, &weather);
        }
    }
}

fn sky_now(wind: &SyntheticWind, t: f32) -> Sky {
    // Sampled at the world origin rather than at anybody's boat: this is the synoptic
    // state, which is the thing that is the same everywhere, and the local variation is
    // what the gust field is for.
    let at = glam::DVec2::ZERO;
    Sky {
        tws: wind.speed_at(at, t as f64) as f32,
        wind_from: wind.direction_at(at, t as f64) as f32,
        cover: wind.synoptic.cover() as f32,
        t,
    }
}

fn broadcast(socket: &UdpSocket, fleet: &HashMap<SocketAddr, Aboard>, bytes: &[u8]) {
    for addr in fleet.keys() {
        let _ = socket.send_to(bytes, addr);
    }
}
