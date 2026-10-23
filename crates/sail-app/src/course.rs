//! The delivery run: where the parcels are going, and whether they got there in time.
//!
//! This replaced a windward mark and a lap time, which were the right goal for a
//! physics proof of concept and the wrong one for a game. A beat to a buoy asks "can
//! you tack well"; a consignment between two anchorages asks the questions the
//! archipelago exists to ask — which side of that island, is the short way through
//! the sound worth the wind shadow, can you lay it or is this two tacks, and is there
//! water where you are going.
//!
//! The deadline is what makes those questions cost something. Without one every route
//! is equally good and the islands are scenery again.

use bevy::prelude::*;
use glam::DVec2;
use sail_core::land::Harbour;
use sail_core::prelude::*;

use crate::frame;
use crate::sim::Sim;

/// Where she is bound, and the marks that say so.
///
/// What this module does in a frame, in one place — see `stages.rs` for why the
/// schedule is named sets rather than one long chain.
pub fn plugin(app: &mut App) {
    use crate::stages::{Build, Deck};
    app.add_systems(Startup, setup.in_set(Build::Scene));
    app.add_systems(Update, update.in_set(Deck::Dress));
    app.add_systems(Update, float_marks.in_set(Deck::Dress));
    app.add_systems(Update, restart.in_set(Deck::Weather));
    // Before `update`, which is where an arrival is noticed: a leg called this frame
    // should be the leg she is sailing this frame.
    app.add_systems(Update, shared_leg.in_set(Deck::Dress).before(update));
}

/// How close counts as arrived, metres. An anchorage, not a berth: you are there when
/// you could reasonably drop the hook.
const ARRIVAL_RADIUS: f64 = 120.0;
/// And slow enough to be stopping rather than passing through.
const ARRIVAL_SPEED: f64 = 2.0;
/// How long a consignment should be, metres.
///
/// A mile and three quarters. The clock runs at one to one with no compression, so this
/// number *is* the length of a sitting: fifteen to twenty minutes with a fair wind, an
/// hour if it has to be worked for. Anything much longer stops being a passage and
/// becomes a commitment, and anything shorter is a ferry trip rather than a delivery.
const LEG_TARGET: f64 = 3_240.0;

/// Speed made good the deadline assumes, knots.
///
/// Three and a half. The boat does better than that on a reach and much worse beating
/// into a headland's wind shadow, so a consignment is comfortable if you route it well
/// and tight if you do not — which is the whole of the game.
const PLANNING_SPEED_KN: f64 = 3.5;
/// Slack on top, seconds, so a short hop is not impossible.
const SLACK: f64 = 420.0;

/// Anything moored that has to ride the sea rather than hang over it. A mark sitting
/// at mean water level while a wave passes through it is the single most jarring
/// thing you can put on a sea surface.
#[derive(Component)]
pub struct Floating {
    /// World position in simulation coordinates.
    pub at: DVec2,
    /// Height above the local surface, metres.
    pub lift: f32,
}

/// The pole that marks where you are going now.
#[derive(Component)]
pub struct Destination;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Leg {
    /// Carrying a consignment.
    Carrying,
    /// Nowhere to go: there is only one anchorage, or none. The default, because a
    /// course that has not been laid is a course with nowhere to go — which is also
    /// what the open ocean is, and what a headless world rendering music has.
    #[default]
    Idle,
}

#[derive(Resource, Default)]
pub struct Course {
    /// Set by the console button; cleared by [`restart`] the same frame.
    pub wants_new: bool,
    pub harbours: Vec<Harbour>,
    pub from: usize,
    pub to: usize,
    pub leg: Leg,
    /// Seconds since this consignment was taken on.
    pub elapsed: f64,
    /// Seconds allowed for it.
    pub allowed: f64,
    pub delivered: u32,
    pub late: u32,
    /// Best delivery so far, as a fraction of the time allowed.
    pub best_margin: Option<f64>,
    pub tacks: u32,
    last_tack_side: f64,
    /// Counts up while the boat sits in the arrival circle, so a fast pass through it
    /// does not count as a delivery.
    settling: f64,
    /// Whether the relay is handing out the legs.
    ///
    /// When it is, arriving does not pick a new one: the next leg is called for
    /// everybody at once, and a boat that chose her own would be sailing somewhere
    /// nobody else is going while her console said otherwise.
    shared: bool,
}

impl Course {
    fn new(harbours: Vec<Harbour>, start: DVec2) -> Self {
        // Start from whichever anchorage is nearest, so the first consignment reads as
        // leaving from somewhere rather than materialising at sea.
        let from = nearest(&harbours, start);
        let mut course = Self {
            wants_new: false,
            harbours,
            from,
            to: from,
            leg: Leg::Idle,
            elapsed: 0.0,
            allowed: 0.0,
            delivered: 0,
            late: 0,
            best_margin: None,
            tacks: 0,
            last_tack_side: 0.0,
            settling: 0.0,
            shared: false,
        };
        course.take_consignment(start);
        course
    }

    /// Where the parcels are going now.
    /// Where a fresh boat comes in, when the last one came to grief.
    ///
    /// The harbour the consignment starts from — or, if there is no harbour at all,
    /// wherever she went down. **There is a sea with no harbours in it now**: `SAIL_LAND=0`
    /// is open ocean, with no islands, no marks and nowhere to come from, and this was
    /// the one line in this file that indexed the list instead of asking it. Every other
    /// reader here already used `get` and a fallback; this one panicked, and the way to
    /// reach it was to die and press the button that says ANOTHER BOAT — which in a
    /// ten-metre sea with nothing to run into is the likeliest thing that happens.
    pub fn starts_from(&self, adrift: DVec2) -> DVec2 {
        self.harbours.get(self.from).map(|h| h.at).unwrap_or(adrift)
    }

    pub fn target(&self) -> DVec2 {
        self.harbours
            .get(self.to)
            .map(|h| h.at)
            .unwrap_or(DVec2::ZERO)
    }

    /// Whether there is a place to be bound for at all.
    ///
    /// False in open ocean, where there are no harbours — and the console was handing
    /// out a range and a bearing anyway, worked out against the fallback position, which
    /// is the world origin. Three and a half miles to NOWHERE, on a bearing, in figures.
    pub fn bound_somewhere(&self) -> bool {
        self.harbours.get(self.to).is_some()
    }

    pub fn destination_name(&self) -> &str {
        self.harbours
            .get(self.to)
            .map(|h| h.name.as_str())
            .unwrap_or("nowhere")
    }

    pub fn origin_name(&self) -> &str {
        self.harbours
            .get(self.from)
            .map(|h| h.name.as_str())
            .unwrap_or("nowhere")
    }

    /// Distance and compass bearing to the destination.
    pub fn to_target(&self, pos: DVec2) -> (f64, f64) {
        let d = self.target() - pos;
        (d.length(), wrap_tau(d.x.atan2(d.y)))
    }

    /// Seconds left before the consignment is late. Negative once it is.
    pub fn remaining(&self) -> f64 {
        self.allowed - self.elapsed
    }

    /// Pick somewhere new to go, and work out how long it should take.
    ///
    /// The deadline is the straight-line distance at a planning speed, which is
    /// deliberately *not* a routed estimate: a crow-flies allowance is generous for a
    /// reach and punishing for a dead beat, which is exactly the judgement a skipper
    /// is paid for. Taking the job is the decision; the wind decides the rest.
    fn take_consignment(&mut self, from_pos: DVec2) {
        if self.harbours.len() < 2 {
            self.leg = Leg::Idle;
            return;
        }
        // The harbour nearest to a *sailable* leg, which is not the same as the
        // furthest one — and the furthest one is what this used to pick.
        //
        // The clock here runs at one to one and there is no compression, so the length
        // of a consignment is the length of a sitting. Measured across seeds, the
        // archipelago offers gaps from one and a half to twelve miles; taking the
        // longest put the nearest harbour ten or twelve miles off, which at six knots on
        // a reach is the better part of two hours and on a beat is an afternoon. That is
        // not a passage, it is a commitment.
        //
        // A mile and three quarters is fifteen to twenty minutes with the wind where you
        // want it, and an hour if you have to work for it — which is the right shape: the
        // *reward for a fair wind* is the thing a delivery run is about, and it only
        // exists if a fair wind finishes the job inside one sitting.
        //
        // A skipper does not sail to the furthest port on the chart either. He takes the
        // job that is offered, and the jobs that exist on a coast like this are coastal
        // runs between neighbours.
        let mut best = (self.from, f64::INFINITY);
        for (i, h) in self.harbours.iter().enumerate() {
            if i == self.from {
                continue;
            }
            let d = (h.at - from_pos).length();
            // A rotating nudge of a sixth either way, so consecutive runs are not the
            // same two ports back and forth. Deterministic in the delivery count, so a
            // session replays.
            let nudge = 1.0 + 0.34 * ((((i as u32 ^ self.delivered) % 7) as f64 / 6.0) - 0.5);
            let miss = (d - LEG_TARGET * nudge).abs();
            if miss < best.1 {
                best = (i, miss);
            }
        }
        self.to = best.0;
        let distance = (self.target() - from_pos).length();
        self.allowed = distance / kn_to_ms(PLANNING_SPEED_KN) + SLACK;
        self.elapsed = 0.0;
        self.settling = 0.0;
        self.leg = Leg::Carrying;
    }
}

impl Course {
    /// Take the leg the relay has called, instead of choosing one.
    ///
    /// **The same job for everybody**, which is the whole of what makes another boat
    /// worth being near: the sea was already shared and there was nothing to do in it
    /// together. The indices come off the wire and mean the same two harbours on every
    /// screen, because both ends build the archipelago with `Archipelago::of_seed`.
    ///
    /// The allowance is worked out from **the harbour she is leaving**, not from where
    /// this boat happens to be floating — or two people on the same leg would be given
    /// different deadlines for it, which is the one thing a shared job may not do.
    pub fn take_shared(&mut self, from: usize, to: usize) -> bool {
        if from == to || from >= self.harbours.len() || to >= self.harbours.len() {
            return false;
        }
        self.from = from;
        self.to = to;
        let start = self.harbours[from].at;
        let distance = (self.harbours[to].at - start).length();
        self.allowed = distance / kn_to_ms(PLANNING_SPEED_KN) + SLACK;
        self.elapsed = 0.0;
        self.settling = 0.0;
        self.leg = Leg::Carrying;
        true
    }

    /// Whether this leg was handed out rather than chosen here.
    pub fn is_shared(&self) -> bool {
        self.shared
    }

    pub fn share(&mut self, shared: bool) {
        self.shared = shared;
    }
}

fn nearest(harbours: &[Harbour], pos: DVec2) -> usize {
    harbours
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            (a.at - pos)
                .length()
                .partial_cmp(&(b.at - pos).length())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// Lift the marks onto the water, and let them lean with the slope.
pub fn float_marks(
    sim: Res<Sim>,
    origin: Res<frame::RenderOrigin>,
    mut q: Query<(&Floating, &mut Transform)>,
) {
    let t = sim.sim.state.time;
    for (float, mut transform) in &mut q {
        let height = sim.sim.water.height(float.at, t);
        let slope = sim.sim.water.slope(float.at, t);
        // The whole translation, not just the height: a mark is anchored in sim
        // coordinates and has to be re-placed whenever the render origin moves, or it
        // stays a kilometre behind the rest of the world.
        transform.translation = origin.to_bevy(float.at, height as f32 + float.lift);
        // A buoy stands up to the local normal, which on a steep sea is visible.
        transform.rotation =
            Quat::from_rotation_z(-slope.x as f32) * Quat::from_rotation_x(-slope.y as f32);
    }
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    origin: Res<frame::RenderOrigin>,
    sim: Res<Sim>,
) {
    let harbours = sim
        .sim
        .land
        .as_ref()
        .map(|l| l.harbours())
        .unwrap_or_default();
    let course = Course::new(harbours, sim.sim.state.pos);
    // What the archipelago actually offers, in the units a passage is thought in. A
    // consignment's length is a design decision and it cannot be made without this.
    {
        let mut gaps: Vec<f64> = Vec::new();
        for (i, a) in course.harbours.iter().enumerate() {
            for b in course.harbours.iter().skip(i + 1) {
                gaps.push((a.at - b.at).length());
            }
        }
        gaps.sort_by(f64::total_cmp);
        let nm = |m: f64| m / 1852.0;
        info!(
            "course: {} harbours, {} pairs; gaps {:.1} / {:.1} / {:.1} nm (least, median, \
             most); first leg {:.1} nm to {}, allowed {:.0} min",
            course.harbours.len(),
            gaps.len(),
            gaps.first().copied().map_or(0.0, nm),
            gaps.get(gaps.len() / 2).copied().map_or(0.0, nm),
            gaps.last().copied().map_or(0.0, nm),
            nm(course.to_target(sim.sim.state.pos).0),
            course.destination_name(),
            course.allowed / 60.0
        );
    }

    let buoy_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.35, 0.08),
        perceptual_roughness: 0.6,
        ..default()
    });
    let flag_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.85, 0.12),
        unlit: true,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let body = meshes.add(Sphere::new(0.9));
    let pole = meshes.add(Cylinder::new(0.06, 6.0));
    let flag = meshes.add(Cuboid::new(0.02, 0.6, 0.9));

    // One buoy off every anchorage, so the chart is the world rather than a menu.
    for h in &course.harbours {
        let at = origin.to_bevy(h.at, 0.0);
        commands.spawn((
            Floating {
                at: h.at,
                lift: 0.35,
            },
            Mesh3d(body.clone()),
            MeshMaterial3d(buoy_mat.clone()),
            Transform::from_translation(at + Vec3::Y * 0.35),
        ));
    }

    // And a tall flagged pole that moves to wherever the parcels are going.
    let at = origin.to_bevy(course.target(), 0.0);
    commands
        .spawn((
            Destination,
            Floating {
                at: course.target(),
                lift: 3.0,
            },
            Mesh3d(pole.clone()),
            MeshMaterial3d(buoy_mat.clone()),
            Transform::from_translation(at + Vec3::Y * 3.0),
        ))
        .with_children(|p| {
            p.spawn((
                Mesh3d(flag.clone()),
                MeshMaterial3d(flag_mat.clone()),
                Transform::from_xyz(0.0, 2.6, 0.5),
            ));
        });

    commands.insert_resource(course);
}

pub fn update(
    sim: Res<Sim>,
    time: Res<Time>,
    net: Res<crate::net::Net>,
    mut course: ResMut<Course>,
    mut marks: Query<&mut Floating, With<Destination>>,
) {
    if course.leg == Leg::Idle {
        return;
    }
    course.elapsed += time.delta_secs_f64();

    // Count tacks: a change of sign in the true wind angle, with a dead band so
    // sitting head to wind does not rack up a dozen of them.
    let twa = sim.sim.telemetry.twa;
    if twa.abs() > rad(15.0) {
        let side = twa.signum();
        if course.last_tack_side != 0.0 && side != course.last_tack_side {
            course.tacks += 1;
        }
        course.last_tack_side = side;
    }

    let pos = sim.sim.state.pos;
    let (distance, _) = course.to_target(pos);
    let slow = sim.sim.telemetry.stw < kn_to_ms(ARRIVAL_SPEED);
    if distance < ARRIVAL_RADIUS && slow {
        // Dropping the hook takes a moment. Requiring the boat to *stay* means a
        // delivery is an arrival rather than a drive-by, which is the difference
        // between sailing to a place and sailing at it.
        course.settling += time.delta_secs_f64();
    } else {
        course.settling = 0.0;
    }

    if course.settling > 4.0 {
        let margin = course.remaining() / course.allowed.max(1.0);
        if margin < 0.0 {
            course.late += 1;
        }
        course.delivered += 1;
        course.best_margin = Some(course.best_margin.map_or(margin, |b| b.max(margin)));
        course.from = course.to;
        if course.is_shared() {
            // **The relay calls the next leg, for everybody at once.** A boat that
            // picked her own here would sail off to somewhere nobody else is going,
            // with her console insisting she was on the same job as the rest of them.
            // She waits, and the wait is the end of the leg — which is what a finish is.
            course.leg = Leg::Idle;
            if let Some(link) = net.link.as_ref() {
                link.arrived(course.elapsed as f32);
            }
        } else {
            let here = course.target();
            course.take_consignment(here);
        }
        for mut mark in &mut marks {
            mark.at = course.target();
        }
    }
}

/// Take the leg the relay has called.
///
/// **Its own system rather than a line in the packet reader**, for the same reason
/// rebuilding the world is: the reader takes what arrived off the wire, and deciding
/// what the boat is doing about it belongs beside the thing that decides that.
pub fn shared_leg(
    mut net: ResMut<crate::net::Net>,
    mut course: ResMut<Course>,
    mut marks: Query<&mut Floating, With<Destination>>,
) {
    // The link has gone: back to picking her own jobs. A boat left idle for ever
    // because the relay that was handing out legs is no longer there would be a boat
    // with nothing to do and no way of knowing why.
    if net.link.is_none() {
        if course.is_shared() {
            course.share(false);
            if course.leg == Leg::Idle {
                let here = course.harbours.get(course.from).map(|h| h.at);
                if let Some(here) = here {
                    course.take_consignment(here);
                }
            }
            info!("course: on her own again");
        }
        return;
    }
    let Some(passage) = net.leg.take() else {
        return;
    };
    course.share(true);
    if !course.take_shared(passage.from as usize, passage.to as usize) {
        // A leg naming a harbour this archipelago has not got is a leg from a relay
        // that built a different world. Said out loud: it is the one failure that would
        // otherwise look like the console simply not updating.
        warn!(
            "leg: harbours {} and {} are not in this sea of {}",
            passage.from,
            passage.to,
            course.harbours.len()
        );
        return;
    }
    for mut mark in &mut marks {
        mark.at = course.target();
    }
    info!(
        "leg: {} to {}, {:.1} nm, allowed {:.0} min",
        course.harbours[passage.from as usize].name,
        course.destination_name(),
        (course.harbours[passage.to as usize].at - course.harbours[passage.from as usize].at)
            .length()
            / 1852.0,
        course.allowed / 60.0
    );
}

/// Give up on this consignment and take another from where you are.
pub fn restart(
    mut sim: ResMut<Sim>,
    mut course: ResMut<Course>,
    mut marks: Query<&mut Floating, With<Destination>>,
) {
    // **No key.** Asking for a new consignment is not a thing you do with a thumb in a
    // hurry: it throws away the passage you are on. It lives on a button in the console
    // and on the settings page, where pressing it is a decision rather than a neighbour
    // of the key you meant. Taken off the keyboard from the deck.
    let asked = course.wants_new;
    course.wants_new = false;
    if !asked {
        return;
    }
    let pos = sim.sim.state.pos;
    course.from = nearest(&course.harbours, pos);
    course.tacks = 0;
    course.take_consignment(pos);
    for mut mark in &mut marks {
        mark.at = course.target();
    }
    // A fresh boat, if the last one came to grief. She starts where the new consignment
    // starts, at rest, head to the wind she has — which is what stepping aboard is.
    if sim.sim.telemetry.mishap.is_some() {
        let heading = sim.sim.state.heading;
        let mut fresh = sail_core::state::BoatState::at_rest(heading);
        fresh.pos = course.starts_from(sim.sim.state.pos);
        fresh.time = sim.sim.state.time;
        sim.sim.recover(fresh);
        // And a fresh sail plan with her, or the gennaker that just blew out is still
        // set when she reappears and goes again inside a second.
        sim.controls = sail_core::sim::Simulation::safe_rig();
    }
}

/// Minutes and seconds, because a passage time read as 412.7 seconds means nothing.
pub fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0);
    format!("{}:{:04.1}", (s / 60.0) as u32, s % 60.0)
}

#[cfg(test)]
mod a_leg_everybody_is_on {
    use super::*;

    fn two_ports() -> Course {
        Course::new(
            vec![
                Harbour {
                    name: "Grimvik".into(),
                    at: DVec2::new(0.0, 0.0),
                    exposed_to: 0.0,
                },
                Harbour {
                    name: "Fuglnes".into(),
                    at: DVec2::new(4_000.0, 0.0),
                    exposed_to: 0.0,
                },
            ],
            DVec2::ZERO,
        )
    }

    /// **The same job has to be the same job**, and the part of it that is easiest to
    /// get wrong is the clock: an allowance worked out from where each boat is floating
    /// gives two people different deadlines for one leg, and whoever happened to be
    /// nearer the start would be given longer as well.
    #[test]
    fn the_allowance_is_the_same_wherever_she_happens_to_be() {
        let mut near = two_ports();
        let mut far = two_ports();
        assert!(near.take_shared(0, 1));
        assert!(far.take_shared(0, 1));
        assert_eq!(near.allowed, far.allowed);
        // And it is the leg's own length that decides it, not nothing.
        assert!(near.allowed > 4_000.0 / kn_to_ms(PLANNING_SPEED_KN));

        // A leg this sea has not got is refused rather than taken half way.
        let mut course = two_ports();
        let was = (course.from, course.to);
        assert!(!course.take_shared(0, 9), "a harbour that is not there");
        assert!(!course.take_shared(1, 1), "a leg from a place to itself");
        assert_eq!((course.from, course.to), was);
    }

    /// Whose legs these are decides what happens on arrival, so it has to survive the
    /// link going away: a boat left idle for ever because the relay that was handing out
    /// jobs has gone is a boat with nothing to do and no way of knowing why.
    #[test]
    fn she_goes_back_to_her_own_jobs() {
        let mut course = two_ports();
        course.share(true);
        assert!(course.is_shared());
        course.share(false);
        assert!(!course.is_shared());
    }
}

#[cfg(test)]
mod a_sea_with_nowhere_in_it {
    use super::*;

    /// **A passage with no harbours is a thing that exists now.** `SAIL_LAND=0` is open
    /// ocean — no islands, no marks, nowhere to come from — and it was added to look at
    /// water. Everything in this file already asked the harbour list politely and took
    /// a fallback when it was empty, except the one line that put a fresh boat back on
    /// the water after a mishap, which indexed it. The way in was to die and press the
    /// button that says ANOTHER BOAT, which in a ten-metre sea with nothing to run into
    /// is the likeliest thing that happens next.
    #[test]
    fn a_fresh_boat_starts_where_she_went_down_if_there_is_nowhere_else() {
        let adrift = DVec2::new(4_210.0, -889.0);
        let nowhere = Course::new(Vec::new(), adrift);
        // This is the panic, written out: `from` is nought because `nearest` has nothing
        // to be nearest to, and the line that used to be here indexed the list with it.
        assert!(nowhere.harbours.get(nowhere.from).is_none());
        assert_eq!(nowhere.starts_from(adrift), adrift);
        // And the rest of the file's answers for an empty sea, which were already right
        // and are asserted here so they stay that way.
        assert_eq!(nowhere.target(), DVec2::ZERO);
        assert!(matches!(nowhere.leg, Leg::Idle));
    }

    /// With harbours it is the harbour, not the water.
    #[test]
    fn and_from_the_harbour_when_there_is_one() {
        let at = DVec2::new(100.0, 200.0);
        let course = Course::new(
            vec![
                Harbour {
                    name: "Grimvik".into(),
                    at,
                    exposed_to: 0.0,
                },
                Harbour {
                    name: "Fuglnes".into(),
                    at: DVec2::new(3_000.0, 0.0),
                    exposed_to: 0.0,
                },
            ],
            DVec2::new(120.0, 210.0),
        );
        assert_eq!(course.starts_from(DVec2::new(-9.0, -9.0)), at);
    }
}
