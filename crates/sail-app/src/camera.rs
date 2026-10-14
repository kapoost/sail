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
    Cockpit,
    /// The default, which is not where the helmsman is.
    ///
    /// From the helm you cannot see the thing you are steering: the sails are above and
    /// behind the mast from there, the heel is hard to read, and the hull's attitude —
    /// which is most of what tells you she is overpressed — is under your feet. From
    /// astern all three are in front of you at once. It is the view every sailing
    /// photograph is taken from and for the same reason.
    #[default]
    Chase,
    /// Facing aft from the after end of the cockpit, down the wake.
    ///
    /// Added for the rod and worth having without one. A boat gives you two directions
    /// that matter and a simulator usually gives you one: ahead is where you are going
    /// and astern is where your wake, your gear and anything overtaking you are. The
    /// chase camera looks *at* the boat from behind her, which is a different thing
    /// entirely — it shows you the boat, and this shows you what the boat is leaving.
    ///
    /// It is on board, so it heels. The wake tilting as she lies over is most of what
    /// tells you how hard she is pressed when you cannot see the sails.
    Astern,
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

/// Where the eye is pointed, relative to straight ahead.
///
/// A fixed view forward is the one thing a boat never gives you. Half of sailing is
/// looking somewhere else: up at the burgee, out to leeward at the next gust on the
/// water, over your shoulder at the boat coming up behind, down at the telltales. A
/// simulator that bolts your head to the centreline has removed the sense that makes
/// the rest of it work.
///
/// Held on the right mouse button rather than always-on, so the left button stays the
/// console's and a hand that lets go gets its course back. Drifts back to centre on
/// its own, the way a head does.
#[derive(Resource, Default)]
pub struct ViewAim {
    yaw: f32,
    pitch: f32,
}

/// How far round you can look, radians. Most of the way, but not through your own neck.
const AIM_YAW: f32 = 2.6;
const AIM_PITCH_UP: f32 = 1.15;
const AIM_PITCH_DOWN: f32 = 0.65;
/// Radians per pixel of mouse.
const AIM_RATE: f32 = 0.0042;
/// How quickly an untouched view comes back to centre, per second.
const AIM_RECENTRE: f32 = 0.9;

/// Swing the view with the right mouse button held.
pub fn aim(
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<bevy::input::mouse::AccumulatedMouseMotion>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut aim: ResMut<ViewAim>,
) {
    if buttons.pressed(MouseButton::Right) {
        aim.yaw = (aim.yaw - motion.delta.x * AIM_RATE).clamp(-AIM_YAW, AIM_YAW);
        aim.pitch = (aim.pitch - motion.delta.y * AIM_RATE).clamp(-AIM_PITCH_DOWN, AIM_PITCH_UP);
        return;
    }
    // Space already means "put the helm amidships"; it means "look ahead" too, which
    // is the same instinct.
    let home = keys.pressed(KeyCode::Space);
    let k = if home {
        1.0 - (-6.0 * time.delta_secs()).exp()
    } else {
        1.0 - (-AIM_RECENTRE * time.delta_secs()).exp()
    };
    aim.yaw -= aim.yaw * k;
    aim.pitch -= aim.pitch * k;
}

#[derive(Resource, Default)]
pub struct ChaseState {
    position: Vec3,
    initialised: bool,
}

impl ChaseState {
    /// Where the eye is, in render-origin space.
    ///
    /// Exposed because the sea's glitter path depends on it: a highlight is a mirror,
    /// so where it lies is a fact about where you are standing. Reading the camera's
    /// own `Transform` instead would be a frame behind, and a highlight that lags the
    /// view by a frame shears across the water when you look around.
    pub fn eye(&self) -> Vec3 {
        self.position
    }

    /// Carry the stored camera position across a change of render origin.
    pub fn shift(&mut self, by: Vec3) {
        self.position += by;
    }
}

pub fn setup(
    mut commands: Commands,
    sky_state: Res<crate::sky::Sky>,
    mut images: ResMut<Assets<Image>>,
) {
    // One cubemap, two jobs: drawn as the sky, and filtered on the GPU into the
    // environment light the water reflects. Sharing the image is what guarantees the
    // sun you can see and the sun glittering off the wave faces are the same sun.
    let started = std::time::Instant::now();
    let sky = images.add(crate::sky::cubemap(&sky_state));
    info!(
        "sky: cubemap baked in {:.3} s at {:02.0}:{:02.0}, cover {:.0}%, \
         sun {:.0} deg up bearing {:03.0}",
        started.elapsed().as_secs_f32(),
        sky_state.hour.floor(),
        sky_state.hour.fract() * 60.0,
        sky_state.cover * 100.0,
        sky_state.elevation().to_degrees(),
        sky_state
            .sun
            .x
            .atan2(-sky_state.sun.z)
            .to_degrees()
            .rem_euclid(360.0)
    );
    commands.spawn((
        MainCamera,
        Camera3d::default(),
        // Eight samples rather than the default four, and stated rather than inherited
        // so there is no doubt it reaches a render-to-texture target.
        //
        // It smooths the boat's edge against the sea *inside* the small picture, which
        // is the only place it can: each of those pixels is six across on the screen,
        // so what multisampling buys is the colour of the boundary pixel rather than
        // the straightness of the boundary. It takes the hardness off a hull against
        // the water without touching the grain. Going further would mean rendering at
        // twice the width and filtering down before the nearest-neighbour blow-up —
        // genuinely antialiased pixel art — which is a bigger change than a trial.
        bevy::render::view::Msaa::Sample8,
        // Daylight exposure. Bevy defaults to roughly EV100 9.7 — an indoor setting,
        // at which white falls around a thousand candela per square metre. A sky of
        // several thousand then saturates to a flat white sheet with no gradient and
        // no sun in it, which is exactly what it was doing. Exposing for sunlight and
        // giving the sky its real luminance is the fix; dimming the sky to suit an
        // indoor exposure would only have hidden it.
        Exposure::SUNLIGHT,
        crate::sky::SkyImage(sky.clone()),
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
            color: crate::sky::haze_colour(&sky_state),
            falloff: FogFalloff::from_visibility_colors(
                2600.0,
                crate::sky::haze_near(&sky_state),
                crate::sky::haze_colour(&sky_state),
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
            View::Chase => View::Astern,
            View::Astern => View::Cockpit,
        };
    }
}

/// Everything the camera reads but does not change.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Eye<'w> {
    sim: Res<'w, Sim>,
    origin: Res<'w, frame::RenderOrigin>,
    aim: Res<'w, ViewAim>,
    view: Res<'w, View>,
    time: Res<'w, Time>,
}

pub fn follow(
    eye: Eye,
    mut chase: ResMut<ChaseState>,
    mut head: ResMut<HeadMotion>,
    mut q: Query<&mut Transform, With<MainCamera>>,
) {
    let Eye {
        sim,
        origin,
        aim,
        view,
        time,
    } = eye;
    let st = &sim.render_state();
    let hull = origin.hull_translation(st);
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
                // The head turns on the body: the aim is applied inside the hull's
                // own attitude, so looking to leeward on a heeled boat looks *down*
                // the heel the way it does in life, rather than along a level horizon.
                let eye = head.attitude(st.heading as f32)
                    * Quat::from_rotation_y(aim.yaw)
                    * Quat::from_rotation_x(aim.pitch);
                // Standing in the cockpit well, which is a real place now: the sole
                // is below the sheer, so the eye is lower than it was when the deck
                // was a number. Far enough aft to be at the helm and to see the whole
                // sail plan rather than the back of the coachroof.
                let eye_local = Vec3::new(0.0, crate::deck::cockpit_sole() + 1.62, 3.3);
                // The eye's own height is filtered too, so a sharp heave moves the
                // boat under you rather than throwing the view.
                let base = Vec3::new(hull.x, head.heave, hull.z);
                cam.translation = base + frame::attitude(st) * eye_local;
                cam.rotation = eye * Quat::from_rotation_x(-0.10);
                chase.initialised = false;
            }
            View::Astern => {
                // Standing in the after end of the cockpit, turned round. The same head
                // and neck as the cockpit view — it is the same person — with the whole
                // frame swung through half a turn.
                let eye = head.attitude(st.heading as f32)
                    * Quat::from_rotation_y(std::f32::consts::PI + aim.yaw)
                    * Quat::from_rotation_x(aim.pitch);
                // A little to port, where the rod is, and low: the thing worth looking
                // at from here is forty metres of water, not the sky over it.
                let eye_local = Vec3::new(-0.34, crate::deck::cockpit_sole() + 1.58, 3.6);
                let base = Vec3::new(hull.x, head.heave, hull.z);
                cam.translation = base + frame::attitude(st) * eye_local;
                // Tipped down more than the cockpit view, because the line goes into the
                // water astern and that is where the eye wants to be.
                cam.rotation = eye * Quat::from_rotation_x(-0.17);
                chase.initialised = false;
            }
            View::Chase => {
                let heading = frame::heading_only(st);
                // Orbit. The chase camera swings round the boat rather than turning
                // on the spot, because what you want from outside is to look *at* her
                // from somewhere else, not to look away from her.
                let swing =
                    Quat::from_rotation_y(aim.yaw) * Quat::from_rotation_x(-aim.pitch * 0.6);
                let target = Vec3::new(hull.x, head.heave, hull.z)
                    + heading * swing * Vec3::new(0.0, 9.0, 21.0);
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
