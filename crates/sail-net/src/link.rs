//! The socket, on its own thread, so the frame never waits for the network.
//!
//! Two channels and a thread. The game pushes a `Drawn` in whenever it feels like it and
//! drains whatever has arrived; the thread does the blocking. Nothing in the render loop
//! ever touches a socket, which is the only hard requirement here — a frame that stalls
//! on a packet is a boat that stutters, and the person at the helm will feel it long
//! before they see it.
//!
//! UDP rather than TCP, and with no reliability layer over it. A snapshot is worth less
//! the moment a newer one exists, so the one thing a retransmission can buy — eventually
//! delivering an old position — is the one thing we do not want. Lost packets are
//! replaced sixty-six milliseconds later and the receiver reckons across the gap.

use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Mutex;
use std::time::Duration;

use crate::{decode, encode, BoatId, Drawn, ToClient, ToServer, PROTOCOL};

/// What the game needs to know about the state of its connection, in the order it
/// happens.
#[derive(Clone, Debug)]
pub enum Event {
    /// In. Here is your id and the sea everybody is on.
    Joined {
        id: BoatId,
        world: crate::World,
        sky: crate::Sky,
    },
    /// Somebody's boat, as of a moment ago.
    Boat {
        id: BoatId,
        name: String,
        drawn: Drawn,
    },
    Gone(BoatId),
    Weather {
        sky: crate::Sky,
        squalls: Vec<crate::Squall>,
    },
    /// The far end said no, or there is nobody there. Carries something a person can
    /// read, because "it does not work" is the least useful sentence in networking.
    Lost(String),
}

/// A connection to a game, held by the client.
pub struct Link {
    outgoing: Sender<Drawn>,
    /// Behind a mutex only because a channel receiver is `Send` and not `Sync`, and a
    /// game engine wants to hold this in a resource that several threads could in
    /// principle reach. One system touches it, so the lock is never contended; it is
    /// paperwork for the type system rather than synchronisation.
    incoming: Mutex<Receiver<Event>>,
    pub id: Option<BoatId>,
}

impl Link {
    /// Knock on a server and keep knocking until it answers.
    ///
    /// Does not block: the handshake happens on the thread, and the game finds out it is
    /// in when a `Joined` arrives. A game that cannot start until the network is up is a
    /// game that cannot be played alone, and sailing alone is the whole of this one.
    pub fn join(server: &str, name: &str) -> std::io::Result<Self> {
        let server: SocketAddr = server
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| std::io::Error::other("no such address"))?;
        // Any port: the client does not need a known one, the server replies to whatever
        // it hears from.
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_read_timeout(Some(Duration::from_millis(20)))?;
        let (out_tx, out_rx) = channel::<Drawn>();
        let (in_tx, in_rx) = channel::<Event>();
        let name = name.to_string();
        std::thread::Builder::new()
            .name("sail-net".into())
            .spawn(move || run(socket, server, name, out_rx, in_tx))?;
        Ok(Self {
            outgoing: out_tx,
            incoming: Mutex::new(in_rx),
            id: None,
        })
    }

    /// Here is my boat. Dropped on the floor if the thread has gone, which is right:
    /// losing the network is not a reason to stop sailing.
    pub fn send(&self, drawn: Drawn) {
        let _ = self.outgoing.send(drawn);
    }

    pub fn drain(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        let Ok(incoming) = self.incoming.lock() else {
            return events;
        };
        loop {
            match incoming.try_recv() {
                Ok(event) => events.push(event),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    events.push(Event::Lost("the network thread stopped".into()));
                    break;
                }
            }
        }
        drop(incoming);
        for event in &events {
            if let Event::Joined { id, .. } = event {
                self.id = Some(*id);
            }
        }
        events
    }
}

fn run(
    socket: UdpSocket,
    server: SocketAddr,
    name: String,
    outgoing: Receiver<Drawn>,
    incoming: Sender<Event>,
) {
    use std::time::Instant;
    let mut id: Option<BoatId> = None;
    let mut buf = [0u8; 2048];
    let mut knocked = Instant::now() - Duration::from_secs(9);
    let mut sent = Instant::now();
    let every = Duration::from_secs_f32(1.0 / crate::SNAPSHOT_HZ);
    let mut latest: Option<Drawn> = None;
    loop {
        // Whatever the game has put in since the last pass, and only the newest of it:
        // if the game is running at two hundred frames a second and the socket at
        // fifteen, the nineteen snapshots in between are already wrong by the time they
        // would go out.
        loop {
            match outgoing.try_recv() {
                Ok(drawn) => latest = Some(drawn),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    // The game has gone. Say goodbye, so the others lose the boat at
                    // once rather than in six seconds of silence.
                    if let Some(id) = id {
                        let _ = socket.send_to(&encode(&ToServer::Leaving { id }), server);
                    }
                    return;
                }
            }
        }

        // Say hello until somebody says welcome, once a second. A server that is not up
        // yet is the commonest reason this fails and the commonest fix is to wait four
        // seconds while somebody finds the window.
        if id.is_none() && knocked.elapsed() >= Duration::from_secs(1) {
            knocked = Instant::now();
            let hello = encode(&ToServer::Hello {
                protocol: PROTOCOL,
                name: name.clone(),
            });
            if socket.send_to(&hello, server).is_err() {
                let _ = incoming.send(Event::Lost("cannot reach that address".into()));
                return;
            }
        }

        if let (Some(id), Some(drawn)) = (id, latest) {
            if sent.elapsed() >= every {
                sent = Instant::now();
                let _ = socket.send_to(&encode(&ToServer::Here { id, drawn }), server);
            }
        }

        // And whatever has arrived. The read timeout paces the whole loop; twenty
        // milliseconds is three times round between snapshots, which is enough to keep
        // the sending on time without spinning a core.
        while let Ok((n, from)) = socket.recv_from(&mut buf) {
            if from.ip() != server.ip() {
                continue;
            }
            let Some(msg) = decode::<ToClient>(&buf[..n]) else {
                continue;
            };
            match msg {
                ToClient::Welcome {
                    protocol,
                    your_id,
                    world,
                    sky,
                } => {
                    if protocol != PROTOCOL {
                        let _ = incoming.send(Event::Lost(format!(
                            "that game is on protocol {protocol} and this one is on \
                             {PROTOCOL} — somebody is on an older build"
                        )));
                        return;
                    }
                    if id.is_none() {
                        id = Some(your_id);
                        let _ = incoming.send(Event::Joined {
                            id: your_id,
                            world,
                            sky,
                        });
                    }
                }
                ToClient::Boat {
                    id: who,
                    name,
                    drawn,
                } => {
                    // Your own boat comes back round the relay; it is the one boat on the
                    // water you already know everything about.
                    if Some(who) != id {
                        let _ = incoming.send(Event::Boat {
                            id: who,
                            name,
                            drawn,
                        });
                    }
                }
                ToClient::Gone { id: who } => {
                    let _ = incoming.send(Event::Gone(who));
                }
                ToClient::Weather { sky, squalls } => {
                    let _ = incoming.send(Event::Weather { sky, squalls });
                }
                ToClient::No { because } => {
                    let _ = incoming.send(Event::Lost(because));
                    return;
                }
            }
        }
    }
}
