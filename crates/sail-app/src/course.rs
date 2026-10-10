//! Something to sail to.
//!
//! The smallest honest goal for a sailor: a mark set to windward. You cannot reach it
//! by pointing at it, so getting there means beating, and beating means tacking and
//! trimming — the core skill this proof of concept exists to test. Without a mark the
//! build is a boat on an empty sea, and "is this fun" has nothing to bite on.
//!
//! The lap time is also the development instrument that matters most. If a
//! well-trimmed beat is meaningfully faster than a sloppy one, the game has depth. If
//! it is not, no amount of water shader will save it.

use bevy::prelude::*;
use glam::DVec2;
use sail_core::prelude::*;

use crate::frame;
use crate::sim::Sim;

/// How far to windward the mark is set, metres. At a VMG around four knots that is
/// about a six-minute beat, which is long enough to need real tacking decisions and
/// short enough to sail twice in a sitting.
const BEAT_LENGTH: f64 = 800.0;
/// How close counts as rounded. A real rounding is tighter, but the boat has no
/// bowsprit to judge by yet.
const ROUNDING_RADIUS: f64 = 22.0;

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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Leg {
    /// Beating to the windward mark.
    Upwind,
    /// Running back to the start.
    Downwind,
    Finished,
}

#[derive(Resource)]
pub struct Course {
    pub start: DVec2,
    pub windward: DVec2,
    pub leg: Leg,
    pub elapsed: f64,
    /// Elapsed time at the windward rounding, for splitting the lap.
    pub upwind_split: Option<f64>,
    pub best_lap: Option<f64>,
    pub tacks: u32,
    /// Sign of the true wind angle last frame, to count tacks.
    last_tack_side: f64,
}

impl Course {
    fn new(start: DVec2, wind_from: f64) -> Self {
        // Straight upwind from the start.
        let to_wind = DVec2::new(wind_from.sin(), wind_from.cos());
        Self {
            start,
            windward: start + to_wind * BEAT_LENGTH,
            leg: Leg::Upwind,
            elapsed: 0.0,
            upwind_split: None,
            best_lap: None,
            tacks: 0,
            last_tack_side: 0.0,
        }
    }

    pub fn target(&self) -> DVec2 {
        match self.leg {
            Leg::Upwind => self.windward,
            _ => self.start,
        }
    }

    /// Distance and compass bearing to the mark in play.
    pub fn to_target(&self, pos: DVec2) -> (f64, f64) {
        let d = self.target() - pos;
        (d.length(), wrap_tau(d.x.atan2(d.y)))
    }
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    origin: Res<frame::RenderOrigin>,
    sim: Res<Sim>,
) {
    // Startup runs before the first substep, so telemetry is still zeroed here. Ask
    // the wind field directly.
    let wind_from = sail_core::wind::direction_of(sim.sim.wind.sample(sim.sim.state.pos, 0.0));
    let course = Course::new(sim.sim.state.pos, wind_from);

    let buoy_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.35, 0.08),
        perceptual_roughness: 0.6,
        ..default()
    });
    let pole_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.12, 0.14),
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
    let pole = meshes.add(Cylinder::new(0.05, 3.4));
    let flag = meshes.add(Cuboid::new(0.02, 0.5, 0.75));

    for (pos, flagged) in [(course.windward, true), (course.start, false)] {
        let at = origin.to_bevy(pos, 0.0);
        commands.spawn((
            Floating {
                at: pos,
                lift: 0.35,
            },
            Mesh3d(body.clone()),
            MeshMaterial3d(buoy_mat.clone()),
            Transform::from_translation(at + Vec3::Y * 0.35),
        ));
        // A bare buoy is invisible at eight hundred metres — a real course uses a pin
        // with a flag for exactly this reason.
        commands.spawn((
            Floating { at: pos, lift: 1.9 },
            Mesh3d(pole.clone()),
            MeshMaterial3d(pole_mat.clone()),
            Transform::from_translation(at + Vec3::Y * 1.9),
        ));
        if flagged {
            commands.spawn((
                Floating { at: pos, lift: 3.1 },
                Mesh3d(flag.clone()),
                MeshMaterial3d(flag_mat.clone()),
                Transform::from_translation(at + Vec3::new(0.0, 3.1, 0.4)),
            ));
        }
    }

    commands.insert_resource(course);
}

pub fn update(sim: Res<Sim>, time: Res<Time>, mut course: ResMut<Course>) {
    if course.leg == Leg::Finished {
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

    let (distance, _) = course.to_target(sim.sim.state.pos);
    if distance < ROUNDING_RADIUS {
        match course.leg {
            Leg::Upwind => {
                course.upwind_split = Some(course.elapsed);
                course.leg = Leg::Downwind;
            }
            Leg::Downwind => {
                let lap = course.elapsed;
                course.best_lap = Some(course.best_lap.map_or(lap, |b| b.min(lap)));
                course.leg = Leg::Finished;
            }
            Leg::Finished => {}
        }
    }
}

/// Start another lap from wherever the boat happens to be.
pub fn restart(keys: Res<ButtonInput<KeyCode>>, sim: Res<Sim>, mut course: ResMut<Course>) {
    if !keys.just_pressed(KeyCode::KeyL) {
        return;
    }
    let best = course.best_lap;
    let start = course.start;
    let windward = course.windward;
    *course = Course {
        start,
        windward,
        leg: Leg::Upwind,
        elapsed: 0.0,
        upwind_split: None,
        best_lap: best,
        tacks: 0,
        last_tack_side: sim.sim.telemetry.twa.signum(),
    };
}

/// Minutes and seconds, because a lap time read as 412.7 seconds means nothing.
pub fn clock(seconds: f64) -> String {
    let m = (seconds / 60.0).floor() as i64;
    let s = seconds - m as f64 * 60.0;
    format!("{m}:{s:04.1}")
}
