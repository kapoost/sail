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
//! sail-server --word-file /etc/sail-server.word    # and a door
//! ```
//!
//! **On a public address, give it a seed and a word.** The seed because without one the
//! archipelago comes from the clock, so a restart — which is what a service unit does
//! for a living — puts everybody in a different sea. The word because otherwise the
//! first port scan that reaches you is a boat in your game.
//!
//! The word is read from a file or from `SAIL_WORD` in the environment, and never from
//! an argument: `/proc/<pid>/cmdline` is readable by every user on the machine, so a
//! secret on a command line is a secret printed on the wall. It is not printed here
//! either, at any log level.

use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant, SystemTime};

use sail_core::wind::{SyntheticWind, WindField};
use sail_net::{
    decode, encode, on_our_own_network, BoatId, Drawn, Sky, Squall, ToClient, ToServer, World,
    FORGOTTEN, PORT, PROTOCOL,
};

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
    // The word, if there is one: from the environment, or from a file whose *name* is
    // the thing on the command line. Trimmed, because a file written with an editor has
    // a newline on the end of it and nobody would ever find that.
    let mut word = std::env::var("SAIL_WORD")
        .unwrap_or_default()
        .trim()
        .to_string();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--port" => port = args.next().and_then(|v| v.parse().ok()).unwrap_or(PORT),
            "--seed" => seed = args.next().and_then(|v| v.parse().ok()).unwrap_or(seed),
            "--word-file" => {
                let path = args.next().unwrap_or_default();
                match std::fs::read_to_string(&path) {
                    Ok(text) => word = text.trim().to_string(),
                    // Loud and fatal. A door that silently fails open is worse than no
                    // door, because you believe you have one.
                    Err(e) => {
                        eprintln!("cannot read the word from {path}: {e}");
                        std::process::exit(2);
                    }
                }
            }
            other => eprintln!("ignoring {other}"),
        }
    }

    let socket = UdpSocket::bind(("0.0.0.0", port))?;
    socket.set_read_timeout(Some(Duration::from_millis(20)))?;
    println!(
        "sail-server on {port}, seed {seed}, {}",
        if word.is_empty() {
            "no word — anybody who can reach this port can join"
        } else {
            "a word is set"
        }
    );

    // The sea everybody will be in. The archipelago is a pure function of this, so it is
    // the whole of what a new arrival needs to be sailing the same water.
    //
    // The day is drawn here rather than fixed, and drawn from the same seed, so two
    // people who agree on a number get the same morning and a different number is a
    // different one. Force three to six: below three this model is known to be poor
    // company and above six a shared passage turns into everybody separately surviving.
    let mut dice = sail_core::rng::Pcg32::new(seed ^ 0xDA1_5EED);
    let force = 3.0 + 3.0 * dice.next_f64();
    let world = World {
        seed,
        // Beaufort to metres a second, by the usual 0.836·B^1.5.
        tws: (0.836 * force.powf(1.5)) as f32,
        wind_from: (dice.next_f64() * std::f64::consts::TAU) as f32,
        fetch: 14_000.0,
        start_hour: 8.0,
        cover: (0.25 + 0.5 * dice.next_f64()) as f32,
    };
    println!(
        "force {:.1}, {:.0} knots from {:03.0}",
        force,
        world.tws * 1.9438,
        world.wind_from.to_degrees()
    );
    // And the weather, which is the one thing a client is not allowed to decide for
    // itself. Everything else about the world is derivable from the seed and therefore
    // free; the weather is a process with state, and two people in different winds is the
    // one divergence that would actually spoil this.
    let mut wind = SyntheticWind::new(world.wind_from as f64, world.tws as f64, seed ^ 0x5A11);

    // **How many refusals a second this will say out loud.**
    //
    // A "no" is bigger than the "hello" that provokes it, so answering every one of them
    // is a small amplifier of exactly the kind the beacon used to be. A whole-server
    // budget rather than one per address, because per-address means a table keyed by
    // whatever a flood claims to be from, which is a memory leak with a stranger holding
    // the tap. Five a second is more than enough for the real case — somebody typing
    // the word wrong, once a second, and reading what came back — and nothing at all for
    // anybody sweeping the port.
    const REFUSALS_A_SECOND: u32 = 5;
    let mut refused = 0u32;
    let mut refusals_since = Instant::now();

    // **How many boats this relay will have on it.**
    //
    // "Two to eight" was a bandwidth sum and nothing enforced it: every `Hello` from an
    // address not already aboard inserted a map entry holding a name the sender chose,
    // and nothing ever said no. A source address on UDP is whatever the sender writes in
    // it, so that is an attacker-sized table with an attacker holding the tap, against a
    // unit with `MemoryMax=32M` and `Restart=always` on it — which makes the failure a
    // loop rather than an outage.
    //
    // Twelve rather than eight: the number is a *limit*, and a limit set exactly at the
    // comfortable figure turns a ninth friend into a bug report. Relaying is
    // (N-1)-squared in the worst case and twelve boats at fifteen snapshots is still
    // under a megabit.
    const ABOARD: usize = 12;

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

        // **With a budget**, because the read timeout only ends this loop when the socket
        // goes quiet, and under a stream of packets it never does. Everything after it is
        // the housekeeping this relay exists to do: forgetting the boats that went
        // silent, and sending the weather that keeps everybody in the same wind. A server
        // that cannot stop reading stops relaying the weather, and the sea diverges on
        // every screen while the log says it is busy.
        //
        // Two hundred and fifty-six is a pass and a half of a full game — twelve boats at
        // fifteen snapshots a second — so the real case never reaches it.
        const LETTERS_A_PASS: usize = 256;
        let mut read = 0;
        while let Ok((n, from)) = socket.recv_from(&mut buf) {
            read += 1;
            if read > LETTERS_A_PASS {
                break;
            }
            let Some(msg) = decode::<ToServer>(&buf[..n]) else {
                continue;
            };
            match msg {
                ToServer::Hello {
                    protocol,
                    name,
                    word: said,
                } => {
                    // The door. Checked before the protocol version, because a stranger
                    // has no business learning which build this is.
                    //
                    // Already aboard and the word stops mattering: a boat that is in the
                    // fleet got in through the door, and re-knocking is what a client
                    // does when it has heard nothing for six seconds.
                    //
                    // Every refusal below goes through the one budget. It used to be the
                    // wrong word alone, which left the protocol refusal — a long sentence
                    // sent in answer to a six-byte hello, so a twenty-times amplifier —
                    // with nothing on it at all.
                    let mut refuse = |because: String| {
                        if refusals_since.elapsed() >= Duration::from_secs(1) {
                            refusals_since = Instant::now();
                            refused = 0;
                        }
                        if refused < REFUSALS_A_SECOND {
                            refused += 1;
                            let _ = socket.send_to(&encode(&ToClient::No { because }), from);
                        }
                    };
                    if !word.is_empty() && said != word && !fleet.contains_key(&from) {
                        refuse("wrong word".into());
                        continue;
                    }
                    if protocol != PROTOCOL {
                        refuse(format!(
                            "this game is on protocol {PROTOCOL} and you are on \
                             {protocol} — somebody is on an older build"
                        ));
                        continue;
                    }
                    // And the door has a width. Said out loud rather than dropped,
                    // because "the game is full" is a thing a person can act on and
                    // silence is a thing they can only stare at.
                    if fleet.len() >= ABOARD && !fleet.contains_key(&from) {
                        refuse(format!("this game is full — {ABOARD} boats are on it"));
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
                ToServer::WhoIsThere { word: said } => {
                    // **Who gets an answer**, which is the whole of the reflector
                    // question. The reply is thirty times the size of the question and
                    // goes to whatever address the question claims to come from, so
                    // answering strangers is a thirty-times amplifier — and the first
                    // thing that costs is the provider null-routing the machine, with
                    // everything else on it.
                    //
                    // Two ways to deserve an answer and no third. Say the word, which is
                    // how somebody knocks on a game across the internet before joining
                    // it. Or be on this network and be asking a game that has no word on
                    // it, which is the wifi case discovery was built for — a broadcast
                    // has never crossed a router in its life.
                    let knows = !word.is_empty() && said == word;
                    let at_home = word.is_empty() && on_our_own_network(from.ip());
                    if !knows && !at_home {
                        continue;
                    }
                    // Answered unicast, straight back to whoever shouted. The name is
                    // the machine's, because in a list of games on a home network that is
                    // the only thing anybody will recognise.
                    let beacon = encode(&ToClient::Beacon {
                        protocol: PROTOCOL,
                        game: host_name(),
                        players: fleet.len().min(255) as u8,
                        seed,
                    });
                    let _ = socket.send_to(&beacon, from);
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
            let squalls: Vec<Squall> = wind.squall_state().iter().map(Squall::of).collect();
            let weather = encode(&ToClient::Weather {
                sky: sky_now(&wind, t),
                squalls,
            });
            broadcast(&socket, &fleet, &weather);
        }
    }
}

/// Whose machine this is, for the list. Taken from whatever the host already knows
/// rather than asked for: a game that will not start until somebody has named it is a
/// game nobody starts.
fn host_name() -> String {
    std::env::var("SAIL_GAME")
        .or_else(|_| std::env::var("USER"))
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "a game".into())
}

fn sky_now(wind: &SyntheticWind, t: f32) -> Sky {
    // The *synoptic* state, not a sample of it. Those are different numbers, and sending
    // the wrong one was the first version of this: `speed_at` includes the gust at that
    // spot, so the far end would have taken a local wobble for the state of the day and
    // then added its own gusts on top of it. The gusts are a stateless function of
    // position and time and every client already computes the same ones; what has to
    // travel is only the slow weather underneath them.
    Sky {
        tws: wind.synoptic.speed_now() as f32,
        wind_from: wind.synoptic.direction_now() as f32,
        cover: wind.synoptic.cover() as f32,
        t,
    }
}

fn broadcast(socket: &UdpSocket, fleet: &HashMap<SocketAddr, Aboard>, bytes: &[u8]) {
    for addr in fleet.keys() {
        let _ = socket.send_to(bytes, addr);
    }
}
