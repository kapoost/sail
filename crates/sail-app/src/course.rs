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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Leg {
    /// Carrying a consignment.
    Carrying,
    /// Nowhere to go: there is only one anchorage, or none.
    Idle,
}

#[derive(Resource)]
pub struct Course {
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
}

impl Course {
    fn new(harbours: Vec<Harbour>, start: DVec2) -> Self {
        // Start from whichever anchorage is nearest, so the first consignment reads as
        // leaving from somewhere rather than materialising at sea.
        let from = nearest(&harbours, start);
        let mut course = Self {
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
        };
        course.take_consignment(start);
        course
    }

    /// Where the parcels are going now.
    pub fn target(&self) -> DVec2 {
        self.harbours
            .get(self.to)
            .map(|h| h.at)
            .unwrap_or(DVec2::ZERO)
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
        let here = course.target();
        course.take_consignment(here);
        for mut mark in &mut marks {
            mark.at = course.target();
        }
    }
}

/// Give up on this consignment and take another from where you are.
pub fn restart(
    keys: Res<ButtonInput<KeyCode>>,
    sim: Res<Sim>,
    mut course: ResMut<Course>,
    mut marks: Query<&mut Floating, With<Destination>>,
) {
    if !keys.just_pressed(KeyCode::KeyL) {
        return;
    }
    let pos = sim.sim.state.pos;
    course.from = nearest(&course.harbours, pos);
    course.tacks = 0;
    course.take_consignment(pos);
    for mut mark in &mut marks {
        mark.at = course.target();
    }
}

/// Minutes and seconds, because a passage time read as 412.7 seconds means nothing.
pub fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0);
    format!("{}:{:04.1}", (s / 60.0) as u32, s % 60.0)
}
