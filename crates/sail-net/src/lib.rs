//! What two boats say to each other.
//!
//! The whole protocol, and deliberately a small one. The argument for its size is in
//! `docs/online.md` and it comes down to two facts about this particular game:
//!
//! * **The world is a pure function of a seed.** `islands::generate(seed)` is the
//!   archipelago and the weather is nine numbers, so joining a game costs about eighty
//!   bytes and then both machines are sailing the same water. There is nothing else to
//!   replicate — no terrain, no objects, no spawns.
//! * **Every boat is computed by the person sailing her.** A sailor feels fifty
//!   milliseconds on the tiller, so the helm, the rig and the hull are all local and what
//!   crosses the network is only what the *other* boats look like.
//!
//! So this is not a replication layer. It is a format for saying "here is my boat" at
//! fifteen hertz, plus enough handshake to agree on which sea everybody is in.
//!
//! # What is deliberately not here
//!
//! No delta encoding, no compression, no interest management, no prioritisation, no
//! reliability layer over the snapshots. Eight boats at fifteen hertz is eight and a half
//! kilobytes a second, and every one of those mechanisms is work spent on a problem this
//! game does not have. A newer snapshot replaces an older one, a lost one is replaced
//! sixty-six milliseconds later, and the receiver dead-reckons across the gap.

use serde::{Deserialize, Serialize};

/// Bumped whenever a message changes shape. Two versions that disagree refuse each other
/// at the handshake rather than at the first malformed snapshot, because "my friend's
/// boat is in the sky" is a much worse afternoon than "you are both on different builds".
pub const PROTOCOL: u16 = 1;

/// The port the server listens on and the one discovery broadcasts to. Fixed rather than
/// configurable: everybody on this network is a friend, and a port number nobody has to
/// agree on in a chat window is worth more than the flexibility.
pub const PORT: u16 = 7714;

/// Snapshots a second. Fifteen is two per hundred milliseconds, which is well under what
/// a boat's motion needs — a yacht changes attitude over seconds, not frames — and the
/// receiver interpolates anyway.
pub const SNAPSHOT_HZ: f32 = 15.0;

pub type BoatId = u16;

/// Everything it takes to put somebody else's boat on your water, and nothing else.
///
/// Sixteen floats and two bytes. Written as `f32` and as plain arrays rather than as the
/// `DVec2`s and `f64`s the simulation uses, because this is a *drawing*: the archipelago
/// is five and a half kilometres across and `f32` holds that to the centimetre, which is
/// a hundred times finer than anybody can see a boat a hundred metres away.
///
/// The velocities are here so the receiver can dead-reckon between packets rather than
/// teleporting the hull sixty-six milliseconds at a time. The trim is here because a boat
/// is unrecognisable without her sails: a ketch-rigged blob at two hundred metres tells
/// you nothing, and "she has got her genoa rolled away and two reefs in" tells you
/// everything about what the last ten minutes were like over there.
///
/// What is *not* here is any force, coefficient or apparent wind. The receiver already
/// has the wind field — it is a pure function of seed and time — and it has this boat's
/// position and velocity, so it can work out her apparent wind itself and set her sails
/// from it. Sending what can be derived is how a protocol starts growing.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Drawn {
    /// Metres east and north of the world origin.
    pub pos: [f32; 2],
    pub heading: f32,
    pub heel: f32,
    pub pitch: f32,
    pub heave: f32,
    /// Body-frame velocity, m/s, for dead reckoning between snapshots.
    pub surge: f32,
    pub sway: f32,
    pub yaw_rate: f32,
    /// Where the boom is, which is the single most legible thing about another boat.
    pub boom_angle: f32,
    pub main_sheet: f32,
    pub jib_sheet: f32,
    pub reef: f32,
    pub furl: f32,
    pub staysail: f32,
    pub staysail_sheet: f32,
    pub genoa_car: f32,
    /// Which headsail is up: 0 genoa, 1 code zero, 2 gennaker.
    pub headsail: u8,
    /// Backed, which is visible from a long way off and means something has gone wrong
    /// or she is heaving to.
    pub aback: bool,
}

impl Drawn {
    /// Taken from a boat that is actually being sailed.
    pub fn of(st: &sail_core::state::BoatState, c: &sail_core::state::Controls) -> Self {
        Self {
            pos: [st.pos.x as f32, st.pos.y as f32],
            heading: st.heading as f32,
            heel: st.heel as f32,
            pitch: st.pitch as f32,
            heave: st.heave as f32,
            surge: st.surge as f32,
            sway: st.sway as f32,
            yaw_rate: st.yaw_rate as f32,
            boom_angle: st.boom_angle as f32,
            main_sheet: c.main_sheet as f32,
            jib_sheet: c.jib_sheet as f32,
            reef: c.reef as f32,
            furl: c.furl as f32,
            staysail: c.staysail as f32,
            staysail_sheet: c.staysail_sheet as f32,
            genoa_car: c.genoa_car as f32,
            headsail: match c.headsail {
                sail_core::state::Headsail::Genoa => 0,
                sail_core::state::Headsail::Code0 => 1,
                sail_core::state::Headsail::Gennaker => 2,
            },
            aback: c.headsail_aback,
        }
    }

    /// And put back into the shapes the drawing code already takes, so a ghost goes
    /// through the same hull transform and the same sail builder as the boat you are
    /// standing on. The alternative — a second set of drawing functions for other
    /// people's boats — is how two boats end up looking like different games.
    pub fn state(&self) -> sail_core::state::BoatState {
        let mut st = sail_core::state::BoatState::at_rest(self.heading as f64);
        st.pos = glam::DVec2::new(self.pos[0] as f64, self.pos[1] as f64);
        st.heel = self.heel as f64;
        st.pitch = self.pitch as f64;
        st.heave = self.heave as f64;
        st.surge = self.surge as f64;
        st.sway = self.sway as f64;
        st.yaw_rate = self.yaw_rate as f64;
        st.boom_angle = self.boom_angle as f64;
        st
    }

    pub fn controls(&self) -> sail_core::state::Controls {
        sail_core::state::Controls {
            main_sheet: self.main_sheet as f64,
            jib_sheet: self.jib_sheet as f64,
            reef: self.reef as f64,
            furl: self.furl as f64,
            staysail: self.staysail as f64,
            staysail_sheet: self.staysail_sheet as f64,
            genoa_car: self.genoa_car as f64,
            headsail: match self.headsail {
                1 => sail_core::state::Headsail::Code0,
                2 => sail_core::state::Headsail::Gennaker,
                _ => sail_core::state::Headsail::Genoa,
            },
            headsail_aback: self.aback,
            ..Default::default()
        }
    }

    /// Where she will be `dt` seconds after this snapshot was taken, if she carries on
    /// doing what she was doing.
    ///
    /// Straight-line reckoning in the body frame, turned by her rate of turn. Over the
    /// sixty-six milliseconds between snapshots a yacht is going almost exactly straight —
    /// she takes several seconds to answer her helm — so this is accurate to centimetres
    /// and it is what keeps another boat's hull from stepping across the water.
    pub fn reckoned(&self, dt: f32) -> Self {
        let heading = self.heading + self.yaw_rate * dt;
        let (s, c) = (heading.sin(), heading.cos());
        // Body to world: bow is (sin h, cos h) and starboard is (cos h, -sin h).
        let east = self.surge * s + self.sway * c;
        let north = self.surge * c - self.sway * s;
        Self {
            pos: [self.pos[0] + east * dt, self.pos[1] + north * dt],
            heading,
            ..*self
        }
    }
}

/// The weather, which is the one thing the server owns.
///
/// Clients run their own simulation as fast as they like and compute their own gusts —
/// the gust field is a stateless function of position and time, so it needs no packet at
/// all — but the synoptic state and the squalls come from one place. Two people in
/// different winds is the one divergence that would actually spoil this.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Sky {
    pub tws: f32,
    pub wind_from: f32,
    pub cover: f32,
    /// Server time, seconds since the passage began. The clients' clocks follow this.
    pub t: f32,
}

/// A squall: a thing with a position, so the tension can be keyed to the real distance.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Squall {
    pub pos: [f32; 2],
    pub radius: f32,
    pub strength: f32,
}

/// The sea everybody is in, sent once at the handshake.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct World {
    pub seed: u64,
    pub tws: f32,
    pub wind_from: f32,
    pub fetch: f32,
    pub start_hour: f32,
    pub cover: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ToServer {
    /// Knocking. The name is what goes over the other boat.
    Hello {
        protocol: u16,
        name: String,
    },
    /// Fifteen times a second, and nothing else from here on.
    Here {
        id: BoatId,
        drawn: Drawn,
    },
    Leaving {
        id: BoatId,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ToClient {
    /// You are in, this is the sea, and these are the people already on it.
    Welcome {
        protocol: u16,
        your_id: BoatId,
        world: World,
        sky: Sky,
    },
    /// Somebody else's boat. Sent as its own message rather than batched, so a packet
    /// lost is one boat stale for a frame rather than the whole fleet.
    Boat {
        id: BoatId,
        name: String,
        drawn: Drawn,
    },
    Gone {
        id: BoatId,
    },
    /// Once a second.
    Weather {
        sky: Sky,
        squalls: Vec<Squall>,
    },
    /// The handshake failed, and why — so the person sees "different version" rather
    /// than a game that does not start.
    No {
        because: String,
    },
}

/// One datagram is one message. No framing, no length prefix, no stream: UDP already
/// delivers whole datagrams or nothing, which is exactly the shape of a snapshot.
pub fn encode<T: Serialize>(msg: &T) -> Vec<u8> {
    postcard::to_allocvec(msg).expect("a message with no unbounded field in it")
}

pub fn decode<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Option<T> {
    postcard::from_bytes(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_boat_survives_the_wire() {
        let st = sail_core::state::BoatState::at_rest(1.2);
        let c = sail_core::state::Controls {
            reef: 0.4,
            furl: 0.25,
            ..Default::default()
        };
        let there = Drawn::of(&st, &c);
        let bytes = encode(&ToServer::Here {
            id: 3,
            drawn: there,
        });
        // Small enough that the question of batching never has to come up: eight boats
        // at fifteen hertz is under ten kilobytes a second.
        assert!(
            bytes.len() < 120,
            "a snapshot should be about seventy bytes, got {}",
            bytes.len()
        );
        let back: ToServer = decode(&bytes).expect("round trip");
        match back {
            ToServer::Here { id, drawn } => {
                assert_eq!(id, 3);
                assert!((drawn.controls().reef - 0.4).abs() < 1e-6);
                assert!((drawn.state().heading - 1.2).abs() < 1e-6);
            }
            _ => panic!("the wrong message came back"),
        }
    }

    #[test]
    fn dead_reckoning_goes_the_way_she_is_pointing() {
        // Heading east, five metres a second, no turn: after two seconds she is ten
        // metres east and has not moved north.
        let mut d = Drawn {
            heading: std::f32::consts::FRAC_PI_2,
            surge: 5.0,
            ..Default::default()
        };
        d.pos = [0.0, 0.0];
        let on = d.reckoned(2.0);
        assert!((on.pos[0] - 10.0).abs() < 1e-3, "east {}", on.pos[0]);
        assert!(on.pos[1].abs() < 1e-3, "north {}", on.pos[1]);
    }
}
