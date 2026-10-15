//! Finding a game on the network without being told where it is.
//!
//! The whole of the difference between "ask your friend for their IP address and type it
//! in" and "it is in the list". Which sounds like a nicety and is not: an address typed
//! into a chat window is wrong about a third of the time — the wrong interface, yesterday's
//! DHCP lease, a digit — and every one of those failures looks exactly like the game being
//! broken.
//!
//! The clients ask and the servers answer. See `ToServer::WhoIsThere` for why that is the
//! right way round rather than the usual one.

use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::{decode, encode, ToClient, ToServer, PORT, PROTOCOL};

#[derive(Clone, Debug)]
pub struct Found {
    pub at: String,
    pub game: String,
    pub players: u8,
    pub seed: u64,
    /// Seconds since this one was last heard from, so a server that has been shut down
    /// leaves the list rather than sitting in it forever.
    pub heard: f32,
}

/// Looks for games for as long as it is kept. Dropping it stops the thread.
pub struct Scan {
    found: Arc<Mutex<Vec<Found>>>,
    stop: Arc<Mutex<bool>>,
}

impl Drop for Scan {
    fn drop(&mut self) {
        if let Ok(mut stop) = self.stop.lock() {
            *stop = true;
        }
    }
}

impl Scan {
    pub fn begin(port: u16) -> std::io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_broadcast(true)?;
        socket.set_read_timeout(Some(Duration::from_millis(200)))?;
        let found: Arc<Mutex<Vec<Found>>> = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(Mutex::new(false));
        let (mine, quit) = (found.clone(), stop.clone());
        std::thread::Builder::new()
            .name("sail-scan".into())
            .spawn(move || {
                let mut buf = [0u8; 2048];
                let mut asked = Instant::now() - Duration::from_secs(9);
                // Shouted at the network, so it carries no word: a game on the wifi is
                // open to the wifi. Asking one address by name is `Probe`, below, and
                // that one does carry it.
                let ask = encode(&ToServer::WhoIsThere {
                    word: String::new(),
                });
                loop {
                    if *quit.lock().unwrap_or_else(|e| e.into_inner()) {
                        return;
                    }
                    if asked.elapsed() >= Duration::from_secs(1) {
                        asked = Instant::now();
                        // The broadcast address of the local network. A game on this very
                        // machine hears it too, which is what makes testing bearable.
                        let _ = socket.send_to(&ask, ("255.255.255.255", port));
                        if let Ok(mut list) = mine.lock() {
                            for game in list.iter_mut() {
                                game.heard += 1.0;
                            }
                            // Three unanswered shouts and it is gone.
                            list.retain(|g| g.heard < 3.5);
                        }
                    }
                    while let Ok((n, from)) = socket.recv_from(&mut buf) {
                        let Some(ToClient::Beacon {
                            protocol,
                            game,
                            players,
                            seed,
                        }) = decode::<ToClient>(&buf[..n])
                        else {
                            continue;
                        };
                        if protocol != PROTOCOL {
                            continue;
                        }
                        let at = format!("{}:{}", from.ip(), from.port());
                        if let Ok(mut list) = mine.lock() {
                            match list.iter_mut().find(|g| g.at == at) {
                                Some(known) => {
                                    known.players = players;
                                    known.heard = 0.0;
                                }
                                None => list.push(Found {
                                    at,
                                    game,
                                    players,
                                    seed,
                                    heard: 0.0,
                                }),
                            }
                        }
                    }
                }
            })?;
        Ok(Self { found, stop })
    }

    pub fn games(&self) -> Vec<Found> {
        self.found.lock().map(|l| l.clone()).unwrap_or_default()
    }
}

/// The default: whatever is on the usual port.
pub fn begin() -> std::io::Result<Scan> {
    Scan::begin(PORT)
}

/// Knocking on one address, which is what joining a game over the internet begins with.
///
/// Discovery shouts and hears whoever is on the wifi. That is no use at all for a game
/// on a server somewhere, and the gap it left was not a missing feature so much as a
/// missing *answer*: you typed an address, pressed join, and if nothing happened there
/// was no telling a closed port from a wrong word from a machine that was switched off.
/// Three quite different evenings, and the game said the same nothing to all of them.
///
/// So: ask first, and be told which of the three it is. The question carries the word,
/// because a server that answered strangers would be a reflector — see
/// `ToServer::WhoIsThere`.
pub struct Probe {
    answer: Arc<Mutex<Option<Result<Found, String>>>>,
}

impl Probe {
    /// Starts asking. Does not block: the answer turns up in [`answer`](Self::answer)
    /// within a few seconds, one way or the other.
    pub fn begin(address: &str, word: &str) -> Self {
        let answer: Arc<Mutex<Option<Result<Found, String>>>> = Arc::new(Mutex::new(None));
        let slot = answer.clone();
        let address = address.to_string();
        let word = word.trim().to_string();
        let started = std::thread::Builder::new()
            .name("sail-probe".into())
            .spawn(move || {
                let put = |r: Result<Found, String>| {
                    if let Ok(mut slot) = slot.lock() {
                        *slot = Some(r);
                    }
                };
                use std::net::ToSocketAddrs;
                let Some(to) = address.to_socket_addrs().ok().and_then(|mut a| a.next()) else {
                    put(Err("no such address".into()));
                    return;
                };
                let Ok(socket) = UdpSocket::bind("0.0.0.0:0") else {
                    put(Err("no socket".into()));
                    return;
                };
                let _ = socket.set_read_timeout(Some(Duration::from_millis(300)));
                let ask = encode(&ToServer::WhoIsThere { word });
                let mut buf = [0u8; 2048];
                // Four seconds of asking once a second. A packet over the internet is
                // lost often enough that one question is not a test of anything, and
                // four seconds is about as long as anybody will look at a button.
                let until = Instant::now() + Duration::from_secs(4);
                while Instant::now() < until {
                    let _ = socket.send_to(&ask, to);
                    while let Ok((n, from)) = socket.recv_from(&mut buf) {
                        if from.ip() != to.ip() {
                            continue;
                        }
                        if let Some(ToClient::Beacon {
                            protocol,
                            game,
                            players,
                            seed,
                        }) = decode::<ToClient>(&buf[..n])
                        {
                            put(if protocol == PROTOCOL {
                                Ok(Found {
                                    at: address.clone(),
                                    game,
                                    players,
                                    seed,
                                    heard: 0.0,
                                })
                            } else {
                                Err(format!(
                                    "that game is on protocol {protocol} and this one is \
                                     on {PROTOCOL} — somebody is on an older build"
                                ))
                            });
                            return;
                        }
                    }
                }
                // Silence is the interesting case, and it has exactly three causes.
                // Said in the order they are worth checking, because the first is by far
                // the commonest and the last is the one people assume.
                put(Err(
                    "nothing answered: the word may be wrong, the port may be shut, or \
                     the game may not be running"
                        .into(),
                ));
            })
            .is_ok();
        if !started {
            if let Ok(mut slot) = answer.lock() {
                *slot = Some(Err("could not start a thread".into()));
            }
        }
        Self { answer }
    }

    /// `None` while it is still asking.
    pub fn answer(&self) -> Option<Result<Found, String>> {
        self.answer.lock().ok().and_then(|a| a.clone())
    }
}
