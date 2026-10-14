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
    /// Seconds since the hand last moved the view.
    ///
    /// Everything automatic in here is gated on it. A view that corrects itself the
    /// instant you stop dragging is a view that argues with you, and one that corrects
    /// itself while you are *still looking at the thing you turned to look at* is worse
    /// — so the eye holds where it was put, and only afterwards does it drift home or
    /// let anything else have it.
    since_hand: f32,
}

/// How long the eye stays where your hand left it, seconds.
///
/// Long enough to look at the thing you turned to see and think about it, short enough
/// that a view left alone comes back on its own. Nothing automatic happens inside it.
const DWELL: f32 = 3.0;

/// Something worth looking at, and how badly.
///
/// The camera is told *what* to look at and works out the angles itself, because the
/// modules that know where things are — the rod knows where its tip is, the fight knows
/// where the fish is — have no business computing yaw and pitch, and would each do it
/// slightly differently.
///
/// Priority rather than a queue. These are not events to be played in order; they are
/// claims on one pair of eyes, and the right behaviour when two arrive together is that
/// the more interesting one wins outright. A fish taking beats a hand on the drag beats
/// line going out.
#[derive(Resource, Default)]
pub struct Attention {
    at: Option<Vec3>,
    priority: u8,
    left: f32,
}

impl Attention {
    /// Ask the eye to go there. A fresh call at the same priority moves the point, so a
    /// caller can simply post where its fish is every frame and have the view follow it.
    pub fn look(&mut self, at: Vec3, hold: f32, priority: u8) {
        if self.left <= 0.0 || priority >= self.priority {
            self.at = Some(at);
            self.priority = priority;
            self.left = hold;
        }
    }

    fn fade(&mut self, dt: f32) {
        self.left -= dt;
        if self.left <= 0.0 {
            self.at = None;
            self.priority = 0;
        }
    }

    fn held(&self) -> Option<Vec3> {
        self.at
    }
}

/// How fast the eye swings onto something it has been asked to look at, per second.
///
/// Deliberately not instant. A head turns, and a cut is the one thing a camera standing
/// in for a person must never do.
const SETTLE: f32 = 4.5;

impl ViewAim {
    /// Swing towards a point in the world, given the frame the aim is applied inside.
    ///
    /// The derivation, because it is sign-error country. The eye's rotation is
    /// `base * Ry(yaw) * Rx(pitch)` and a camera looks down its own -Z, so for a
    /// direction `d` expressed in `base`'s frame, `Ry(ψ)·Rx(θ)·(0,0,-1)` is
    /// `(-cosθ·sinψ, sinθ, -cosθ·cosψ)` — which inverts to `θ = asin(d.y)` and
    /// `ψ = atan2(-d.x, -d.z)`.
    fn settle_towards(&mut self, base: Quat, eye: Vec3, target: Vec3, dt: f32) {
        let d = base.inverse() * (target - eye);
        if d.length_squared() < 1e-6 {
            return;
        }
        let d = d.normalize();
        // Clamped to the same travel a hand gets, because it is the same neck. Asked to
        // look dead astern from the cockpit, you look over your shoulder as far as you
        // can and no further, which is the honest answer and reads as one.
        let yaw = (-d.x).atan2(-d.z).clamp(-AIM_YAW, AIM_YAW);
        let pitch = d.y.asin().clamp(-AIM_PITCH_DOWN, AIM_PITCH_UP);
        let k = 1.0 - (-SETTLE * dt).exp();
        self.yaw += (yaw - self.yaw) * k;
        self.pitch += (pitch - self.pitch) * k;
    }
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
    mut attention: ResMut<Attention>,
) {
    aim.since_hand += time.delta_secs();
    if buttons.pressed(MouseButton::Right) {
        aim.yaw = (aim.yaw - motion.delta.x * AIM_RATE).clamp(-AIM_YAW, AIM_YAW);
        aim.pitch = (aim.pitch - motion.delta.y * AIM_RATE).clamp(-AIM_PITCH_DOWN, AIM_PITCH_UP);
        aim.since_hand = 0.0;
        // And whatever the gear wanted you to look at, it does not want it any more.
        // A hand on the view is a decision, and nothing in here outranks it.
        *attention = Attention::default();
        return;
    }
    // Space already means "put the helm amidships"; it means "look ahead" too, which
    // is the same instinct — and asking for it explicitly skips the wait.
    let home = keys.pressed(KeyCode::Space);
    if home {
        aim.since_hand = DWELL;
        *attention = Attention::default();
    }
    // The wait. Inside it the view simply stays where it was put.
    if aim.since_hand < DWELL {
        return;
    }
    // And nothing drifts home while the eye is on something, or the drift and the
    // settle would pull against each other and land somewhere between the two.
    if attention.held().is_some() {
        return;
    }
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
    /// Where the outside camera is pointed, eased rather than snapped — see the focus
    /// in `follow`.
    looking: Vec3,
    looking_set: bool,
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
        self.looking += by;
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
    commands.init_resource::<Attention>();
}

pub fn toggle(keys: Res<ButtonInput<KeyCode>>, pads: Query<&Gamepad>, mut view: ResMut<View>) {
    let pressed = keys.just_pressed(KeyCode::KeyC)
        || pads.iter().any(|p| p.just_pressed(GamepadButton::Select));
    if pressed {
        // Top, forward, back. The order is a journey inwards rather than a list: you
        // start outside her where you can see the whole boat, step aboard and look
        // where she is going, then turn round and look at what she is leaving. Cockpit
        // first was the old order and it opened the game standing at the helm, which is
        // the one place a newcomer cannot tell what the boat is doing.
        *view = match *view {
            View::Chase => View::Cockpit,
            View::Cockpit => View::Astern,
            View::Astern => View::Chase,
        };
    }
}

/// Everything the camera reads but does not change.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Eye<'w> {
    sim: Res<'w, Sim>,
    origin: Res<'w, frame::RenderOrigin>,
    view: Res<'w, View>,
    time: Res<'w, Time>,
}

pub fn follow(
    eye: Eye,
    mut aim: ResMut<ViewAim>,
    mut attention: ResMut<Attention>,
    mut chase: ResMut<ChaseState>,
    mut head: ResMut<HeadMotion>,
    mut q: Query<&mut Transform, With<MainCamera>>,
) {
    let Eye {
        sim,
        origin,
        view,
        time,
    } = eye;
    let st = &sim.render_state();
    let hull = origin.hull_translation(st);
    let dt = time.delta_secs().max(1e-5);
    head.follow(st.heel as f32, st.pitch as f32, st.heave as f32, dt);

    // What the eye has been asked to look at, if anything, and if your hand has let go
    // of it for long enough.
    attention.fade(dt);
    let focus = if aim.since_hand >= DWELL {
        attention.held()
    } else {
        None
    };

    for mut cam in &mut q {
        match *view {
            View::Cockpit => {
                // Standing in the cockpit well, which is a real place: the sole is
                // below the sheer, so the eye is lower than it was when the deck was a
                // number. Far enough aft to be at the helm and to see the whole sail
                // plan rather than the back of the coachroof.
                let eye_local = Vec3::new(0.0, crate::deck::cockpit_sole() + 1.62, 3.3);
                // The eye's own height is filtered too, so a sharp heave moves the
                // boat under you rather than throwing the view.
                let base = Vec3::new(hull.x, head.heave, hull.z);
                let stand = base + frame::attitude(st) * eye_local;
                // The head turns on the body: the aim is applied inside the hull's own
                // attitude, so looking to leeward on a heeled boat looks *down* the
                // heel the way it does in life, rather than along a level horizon.
                let body = head.attitude(st.heading as f32);
                if let Some(target) = focus {
                    aim.settle_towards(body, stand, target, dt);
                }
                let eye = body * Quat::from_rotation_y(aim.yaw) * Quat::from_rotation_x(aim.pitch);
                cam.translation = stand;
                cam.rotation = eye * Quat::from_rotation_x(-0.10);
                chase.initialised = false;
            }
            View::Astern => {
                // Standing in the after end of the cockpit, turned round. The same head
                // and neck as the cockpit view — it is the same person — with the whole
                // frame swung through half a turn, so the aim is measured from dead
                // astern and a glance at the lure is a small turn rather than a
                // full one.
                let eye_local = Vec3::new(-0.34, crate::deck::cockpit_sole() + 1.58, 3.6);
                let base = Vec3::new(hull.x, head.heave, hull.z);
                let stand = base + frame::attitude(st) * eye_local;
                let body =
                    head.attitude(st.heading as f32) * Quat::from_rotation_y(std::f32::consts::PI);
                if let Some(target) = focus {
                    aim.settle_towards(body, stand, target, dt);
                }
                let eye = body * Quat::from_rotation_y(aim.yaw) * Quat::from_rotation_x(aim.pitch);
                cam.translation = stand;
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
                // From outside her the camera does not fly anywhere either: it stands
                // where it stands and opens the frame *towards* the thing, which is a
                // different gesture from the head turn the on-board views make and is
                // the same instruction — stay where you are, look that way.
                //
                // Blended rather than aimed. Pointing it at the fish alone puts the fish
                // in the middle of the screen and the boat off the edge of it, which is
                // not what anybody asked for: the boat is the subject, and the fish is a
                // thing happening forty metres astern of the subject. Looking between
                // them keeps both in the frame and swings the view the right way, which
                // is what a person standing off in a dinghy with a camera would do.
                //
                // By default the filtered height, or a sharp heave swings the whole
                // frame about a boat that has barely moved.
                let boat = Vec3::new(hull.x, head.heave + 4.5, hull.z);
                let wanted = match focus {
                    Some(target) => boat.lerp(target, 0.42),
                    None => boat,
                };
                if !chase.looking_set {
                    chase.looking = wanted;
                    chase.looking_set = true;
                }
                chase.looking = chase.looking.lerp(wanted, 1.0 - (-SETTLE * dt).exp());
                cam.look_at(chase.looking, Vec3::Y);
            }
        }
    }
}
