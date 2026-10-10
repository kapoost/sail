//! Two cameras, for two different jobs.
//!
//! The cockpit view heels with the boat. That is deliberate and it is the single most
//! important thing this file does: heel is the primary feedback channel, and a view
//! that stays level throws it away. The chase view deliberately does *not* heel, since
//! a horizon that rolls with the hull from outside the boat just makes people ill.

use bevy::camera::Exposure;
use bevy::light::{GeneratedEnvironmentMapLight, Skybox};
use bevy::prelude::*;

use crate::frame;
use crate::sim::Sim;

#[derive(Component)]
pub struct MainCamera;

#[derive(Resource, Default, PartialEq, Eq, Clone, Copy)]
pub enum View {
    #[default]
    Cockpit,
    Chase,
}

/// A head and neck, in effect.
///
/// The cockpit view used to ride the full rigid-body attitude, which was a decision I
/// made deliberately and got wrong. Nobody aboard experiences that: the
/// vestibulo-ocular reflex holds your gaze on the horizon, your neck absorbs the
/// jolts, and your legs take the vertical. A camera welded to the hull is far more
/// violent than standing on the boat, and the result is a screen that throws you
/// about while the boat itself is behaving perfectly well.
///
/// So each axis is filtered on its own terms, and the point is to keep the
/// *information* while dropping the violence:
///
/// * **Heading** passes through untouched. You look where the bow points.
/// * **Heel** is mostly kept, because it is the primary feedback channel — but a
///   person leans into it, so some of it is absorbed rather than all of it shown.
/// * **Pitch** is nearly all removed. You look at the horizon, not along the deck.
/// * **Heave** keeps its low frequencies, so you still rise over a swell, and loses
///   the high ones, which are what actually make people ill.
#[derive(Resource, Default)]
pub struct HeadMotion {
    heel: f32,
    pitch: f32,
    heave: f32,
    primed: bool,
}

/// How much of each axis reaches the eye at all.
const HEEL_FOLLOW: f32 = 0.70;
const PITCH_FOLLOW: f32 = 0.22;
/// Time constants, seconds. Longer means more absorbed.
const HEEL_LAG: f32 = 0.22;
const PITCH_LAG: f32 = 0.55;
const HEAVE_LAG: f32 = 0.70;

impl HeadMotion {
    fn follow(&mut self, heel: f32, pitch: f32, heave: f32, dt: f32) {
        if !self.primed {
            self.heel = heel * HEEL_FOLLOW;
            self.pitch = pitch * PITCH_FOLLOW;
            self.heave = heave;
            self.primed = true;
            return;
        }
        let k = |tau: f32| 1.0 - (-dt / tau).exp();
        self.heel += (heel * HEEL_FOLLOW - self.heel) * k(HEEL_LAG);
        self.pitch += (pitch * PITCH_FOLLOW - self.pitch) * k(PITCH_LAG);
        self.heave += (heave - self.heave) * k(HEAVE_LAG);
    }

    /// Attitude for the eye: the boat's heading, with the rest softened.
    fn attitude(&self, heading: f32) -> Quat {
        Quat::from_rotation_y(-heading)
            * Quat::from_rotation_z(-self.heel)
            * Quat::from_rotation_x(self.pitch)
    }
}

#[derive(Resource, Default)]
pub struct ChaseState {
    position: Vec3,
    initialised: bool,
}

pub fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    // One cubemap, two jobs: drawn as the sky, and filtered on the GPU into the
    // environment light the water reflects. Sharing the image is what guarantees the
    // sun you can see and the sun glittering off the wave faces are the same sun.
    let sky = images.add(crate::sky::cubemap());
    commands.spawn((
        MainCamera,
        Camera3d::default(),
        // Daylight exposure. Bevy defaults to roughly EV100 9.7 — an indoor setting,
        // at which white falls around a thousand candela per square metre. A sky of
        // several thousand then saturates to a flat white sheet with no gradient and
        // no sun in it, which is exactly what it was doing. Exposing for sunlight and
        // giving the sky its real luminance is the fix; dimming the sky to suit an
        // indoor exposure would only have hidden it.
        Exposure::SUNLIGHT,
        Skybox {
            image: Some(sky.clone()),
            // A clear sky runs five to ten thousand candela per square metre.
            brightness: 8000.0,
            ..default()
        },
        GeneratedEnvironmentMapLight {
            environment_map: sky,
            intensity: 8000.0,
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            // Vertical, and 1.15 rad was 66 degrees — about 100 across at 16:9, which
            // is a GoPro. Everything sweeps through the frame faster at that angle and
            // reads smaller with it, and the deck and bow shrink to marks instead of
            // being the thing you are standing on. A longer lens is also how you make
            // something feel heavy: 0.72 rad is 41 degrees, slightly tighter than the
            // 45 Bevy defaults to.
            fov: 0.72,
            near: 0.08,
            far: 8000.0,
            ..default()
        }),
        Transform::from_xyz(0.0, 3.0, 12.0).looking_at(Vec3::new(0.0, 2.0, 0.0), Vec3::Y),
        // Haze. Without it the sea patch ends in a hard line against the sky at seven
        // hundred metres, which is the single most obvious tell that you are looking
        // at a finite piece of geometry. Real visibility at sea is rarely better than
        // this anyway.
        DistanceFog {
            color: crate::sky::haze_colour(),
            falloff: FogFalloff::from_visibility_colors(
                2600.0,
                crate::sky::haze_near(),
                crate::sky::haze_colour(),
            ),
            ..default()
        },
    ));
    commands.init_resource::<View>();
    commands.init_resource::<ChaseState>();
    commands.init_resource::<HeadMotion>();
}

pub fn toggle(keys: Res<ButtonInput<KeyCode>>, pads: Query<&Gamepad>, mut view: ResMut<View>) {
    let pressed = keys.just_pressed(KeyCode::KeyC)
        || pads.iter().any(|p| p.just_pressed(GamepadButton::Select));
    if pressed {
        *view = match *view {
            View::Cockpit => View::Chase,
            View::Chase => View::Cockpit,
        };
    }
}

pub fn follow(
    sim: Res<Sim>,
    view: Res<View>,
    time: Res<Time>,
    mut chase: ResMut<ChaseState>,
    mut head: ResMut<HeadMotion>,
    mut q: Query<&mut Transform, With<MainCamera>>,
) {
    let st = &sim.render_state();
    let hull = frame::hull_translation(st);
    head.follow(
        st.heel as f32,
        st.pitch as f32,
        st.heave as f32,
        time.delta_secs().max(1e-5),
    );

    for mut cam in &mut q {
        match *view {
            View::Cockpit => {
                // Standing at the helm. The eye sits where the hull puts it, but it
                // does not take the hull's full attitude — see HeadMotion.
                let eye = head.attitude(st.heading as f32);
                let eye_local = Vec3::new(0.0, 2.35, 3.6);
                // The eye's own height is filtered too, so a sharp heave moves the
                // boat under you rather than throwing the view.
                let base = Vec3::new(hull.x, head.heave, hull.z);
                cam.translation = base + frame::attitude(st) * eye_local;
                cam.rotation = eye * Quat::from_rotation_x(-0.10);
                chase.initialised = false;
            }
            View::Chase => {
                let heading = frame::heading_only(st);
                let target =
                    Vec3::new(hull.x, head.heave, hull.z) + heading * Vec3::new(0.0, 9.0, 21.0);
                if !chase.initialised {
                    chase.position = target;
                    chase.initialised = true;
                }
                // Lag the camera so the boat moves within the frame instead of being
                // welded to it. Without the lag a tack reads as the world spinning.
                let k = 1.0 - (-3.0 * time.delta_secs()).exp();
                chase.position = chase.position.lerp(target, k);
                cam.translation = chase.position;
                // Aim at the filtered height, or a sharp heave swings the whole
                // frame about a boat that has barely moved.
                cam.look_at(Vec3::new(hull.x, head.heave + 4.5, hull.z), Vec3::Y);
            }
        }
    }
}
