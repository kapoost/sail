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
                let ask = encode(&ToServer::WhoIsThere);
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
