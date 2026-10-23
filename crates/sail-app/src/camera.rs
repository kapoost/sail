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

/// Where you are sitting, and where you are looking.
///
/// What this module does in a frame, in one place — see `stages.rs` for why the
/// schedule is named sets rather than one long chain.
pub fn plugin(app: &mut App) {
    use crate::stages::{Build, Deck};
    app.init_resource::<ChaseState>();
    app.insert_resource(Sweep::asked());
    app.init_resource::<ViewAim>();
    app.add_systems(Startup, setup.in_set(Build::Scene));
    app.add_systems(Update, follow.in_set(Deck::Dress));
    app.add_systems(Update, (toggle, dress).in_set(Deck::Weather));
    app.add_systems(Update, aim.in_set(Deck::Weather));
}

#[derive(Component)]
pub struct MainCamera;

#[derive(Resource, Default, PartialEq, Eq, Clone, Copy, Debug)]
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
    /// Under the surface, beside the lure.
    ///
    /// This file argued against it on the grounds that the sea is opaque and there would
    /// be nothing to see — which was an argument about the *surface*. Below it there is
    /// the one thing the whole activity is about and that no other view can show: the
    /// lure working, and the fish on it.
    ///
    /// What makes it read as water rather than as a camera that has fallen in is the
    /// visibility. Ten metres, green, and the sky taken away altogether — you cannot fog
    /// a skybox, it is drawn at infinity and no distance falloff reaches it, so the
    /// honest fix is to take it off the camera while the camera is under. Everything
    /// beyond the fog is then the clear colour, which is the colour of water, which is
    /// what is actually out there.
    Under,
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
    /// What the hand cannot hold still, kept separately so that it can be taken out
    /// again. The wobble is applied as the *change* in it rather than added to the aim,
    /// or a boat rolling for a minute would walk the view round the horizon.
    hand_yaw: f32,
    hand_pitch: f32,
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
/// And how far round with an instrument at the eye, which is further.
///
/// A hundred and seventy-five degrees: all but straight astern. The neck's limit is the
/// right one for a glance over your shoulder, but a man taking a bearing turns his whole
/// body and steps round the mainsheet to do it — nobody has ever failed to shoot a mark
/// because it was behind them. Asked for from the deck along with the view itself.
const AIM_AROUND: f32 = 3.05;
const AIM_PITCH_UP: f32 = 1.15;
const AIM_PITCH_DOWN: f32 = 0.65;
/// Radians per pixel of mouse.
const AIM_RATE: f32 = 0.0042;
/// How quickly an untouched view comes back to centre, per second.
///
/// Slow, and half what it was. Reported from the deck as coming back too fast: a view
/// that hurries home reads as a camera on a spring, and what it is meant to be is
/// somebody who was looking at something and has gone back to watching where they are
/// going. That takes a couple of seconds and nobody does it deliberately.
const AIM_RECENTRE: f32 = 0.42;

/// And how much her rolling holds it where it is.
///
/// A head does not drift back to centre while the boat is throwing it about — you are
/// hanging on, and your eyes are busy with the horizon. So the homing lets go while she
/// is rolling and gathers again as she steadies, which makes the return something that
/// happens *between* the seas rather than across them. Nought and the view ignores her
/// entirely, which is what it did.
const ROLL_HOLDS_THE_EYE: f32 = 2.6;

/// How fast the view sweeps round her on its own, radians a second, if at all.
///
/// A resource rather than a read of the environment, because two things ask for it now:
/// `SAIL_ORBIT=9` from outside, which is what it was built for, and the reel in
/// `reel.rs`, which turns it on for one scene and off again — and a dial that can only be
/// set before the program starts cannot do that.
#[derive(Resource, Clone, Copy, Default)]
pub struct Sweep(pub Option<f32>);

impl Sweep {
    pub fn asked() -> Self {
        Self(
            crate::knob::var("SAIL_ORBIT")
                .ok()
                .and_then(|v| v.parse::<f32>().ok())
                .filter(|v| *v > 0.0)
                .map(|deg| deg.to_radians()),
        )
    }
}

/// Swing the view with the right mouse button held.
#[allow(clippy::too_many_arguments)]
pub fn aim(
    sim: Res<crate::sim::Sim>,
    chart: Res<crate::chart::Chart>,
    sighting: Res<crate::sight::Sighting>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<bevy::input::mouse::AccumulatedMouseMotion>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut aim: ResMut<ViewAim>,
    mut attention: ResMut<Attention>,
    sweep: Res<Sweep>,
) {
    aim.since_hand += time.delta_secs();
    // `SAIL_ORBIT=9` swings the view round her at nine degrees a second and leaves it
    // swinging, which is for the demo reel and for looking at a thing from every side
    // without a hand on the mouse. Within the same reach a hand has — two and a half
    // radians either way — so it sweeps rather than spins, and nothing drags it home
    // behind it.
    if let Some(rate) = sweep.0 {
        let reach = AIM_YAW * 0.92;
        aim.yaw = reach * (time.elapsed_secs() * rate).sin();
        aim.pitch = 0.08;
        aim.since_hand = 0.0;
        return;
    }
    // **The paper has the right button while it is on the table.**
    //
    // Right-drag moves the chart about under the instruments, and it was moving the
    // view at the same time: you shift the paper half an inch and the boat swings round
    // behind it. Reported from the deck. With the chart spread there is nothing of the
    // water to look at anyway — it covers the screen — so the rule is simply that the
    // two never share the button.
    if chart.spread {
        return;
    }
    if buttons.pressed(MouseButton::Right) {
        let reach = if sighting.up || sighting.glasses {
            AIM_AROUND
        } else {
            AIM_YAW
        };
        aim.yaw = (aim.yaw - motion.delta.x * AIM_RATE).clamp(-reach, reach);
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
        // Asked for explicitly, so it goes home even with an instrument up — and the
        // check below that holds the view for the instruments is written after this for
        // exactly that reason.
        aim.yaw -= aim.yaw * (1.0 - (-6.0 * time.delta_secs()).exp());
        aim.pitch -= aim.pitch * (1.0 - (-6.0 * time.delta_secs()).exp());
        return;
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
    // Nor while there is an instrument at the eye. A man with binoculars up does not
    // swing his head back to look forward, and neither does one holding a hand bearing
    // compass on a lighthouse — the whole gesture is *keeping it there* while you read
    // the card. The view drifting home underneath that is the instrument fighting the
    // hand that is using it, which is what was reported from the deck.
    //
    // **But it is not steady.** Nobody holds a glass still on a small boat, and the
    // instrument makes it worse rather than better: three times nearer is three times
    // the wobble, which is the real reason a pair of seven-by-fifties is almost unusable
    // at sea and why people brace them against a shroud. So what is held is the
    // *aim*, and on top of it goes what she is doing to the hand holding it — her roll,
    // her pitch and her yaw, in proportion to the sea she is in.
    //
    // Added as the change in the wobble rather than to the aim itself, or a boat rolling
    // for a minute would walk the view round the horizon.
    let st = &sim.sim.state;
    let sea = (sim.sim.telemetry.significant_height as f32 / 1.6).clamp(0.25, 1.8);
    let holding = sighting.up || sighting.glasses;
    // Her heel counts as much as her rolling. Standing on a deck canted twenty degrees
    // you are holding on with one hand and holding the glass with the other, braced
    // against a boat that is leaning away from you — and what that costs is steadiness.
    // So the heel multiplies the wobble, and it leans the line of sight a little as well:
    // a hand-held glass on a heeled boat drifts low to leeward, which is why you end up
    // looking at the water rather than at the thing.
    let heeled = (st.heel.abs() as f32 / 0.44).min(1.6);
    let gain = if holding {
        sea * (1.0 + heeled) * if sighting.glasses { 2.3 } else { 1.0 }
    } else {
        0.0
    };
    let want_yaw =
        (st.roll_rate as f32 * 0.085 + st.yaw_rate as f32 * 0.22 + st.heel as f32 * 0.06) * gain;
    let want_pitch = (st.pitch_rate as f32 * 0.20 + st.heave_rate as f32 * 0.010
        - st.heel.abs() as f32 * 0.05)
        * gain;
    let reach = if holding { AIM_AROUND } else { AIM_YAW };
    aim.yaw = (aim.yaw + want_yaw - aim.hand_yaw).clamp(-reach, reach);
    aim.pitch = (aim.pitch + want_pitch - aim.hand_pitch).clamp(-AIM_PITCH_DOWN, AIM_PITCH_UP);
    aim.hand_yaw = want_yaw;
    aim.hand_pitch = want_pitch;
    if holding {
        return;
    }
    let k = if home {
        // Asked for explicitly, which is a decision rather than a drift: it goes.
        1.0 - (-6.0 * time.delta_secs()).exp()
    } else {
        // How much of her roll is in it at the moment. A tenth of a radian a second is
        // a boat working in a seaway; two tenths is one being thrown about.
        let working = (sim.sim.state.roll_rate.abs() as f32 * ROLL_HOLDS_THE_EYE).min(0.92);
        1.0 - (-AIM_RECENTRE * (1.0 - working) * time.delta_secs()).exp()
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
    let started = bevy::platform::time::Instant::now();
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
        // **Four in a tab, because WebGPU has only one and four.** Eight samples is a
        // native luxury; asking for it in a browser is not a slower picture but no
        // picture at all — the first thing the renderer says is "the sample count (8) of
        // the texture is not supported" and every pass after it is invalid. At this
        // drawn width the difference between four and eight is a boundary pixel that is
        // six screen pixels across either way.
        #[cfg(not(target_arch = "wasm32"))]
        bevy::render::view::Msaa::Sample8,
        #[cfg(target_arch = "wasm32")]
        bevy::render::view::Msaa::Sample4,
        // Daylight exposure. Bevy defaults to roughly EV100 9.7 — an indoor setting,
        // at which white falls around a thousand candela per square metre. A sky of
        // several thousand then saturates to a flat white sheet with no gradient and
        // no sun in it, which is exactly what it was doing. Exposing for sunlight and
        // giving the sky its real luminance is the fix; dimming the sky to suit an
        // indoor exposure would only have hidden it.
        Exposure::SUNLIGHT,
        // **What the sun does to grass when it is too bright to draw.**
        //
        // A tone curve has to get a sunlit hillside — several thousand candela — into a
        // screen that stops at one, and the whole question is what it throws away on the
        // way. The film curves, Bevy's default among them, throw away *colour*: they
        // walk a bright hue towards white, because that is what film emulsion does and
        // what a photograph of a bright thing looks like. The island went pale grey-green
        // in the light, which is a photograph of grass and not grass.
        //
        // Scaling by luminance instead keeps the proportions between the channels, so a
        // thing that is green and over-lit comes out *greener* rather than whiter.
        //
        // **And it was tried and put back**, because of what else it did: the foam is
        // drawn additively, so a whitecap is a bright thing on a dark sea, and a curve
        // that never reaches white crushes every one of them into the water's own tone.
        // Measured on the same gale: the film curve puts 0.78 % of the picture above
        // three quarters brightness and the luminance curve 0.07 %. The sea went bare.
        //
        // So the film curve stays and the hillside wants fixing where the hillside is.
        // `SAIL_TONEMAP=tony|aces|agx|neutral|reinhard|luminance` to look at the others
        // — `neutral` keeps hue in the highlights without flattening them, and is the
        // one to try next.
        match crate::knob::var("SAIL_TONEMAP").as_deref() {
            Ok("tony") => bevy::core_pipeline::tonemapping::Tonemapping::TonyMcMapface,
            Ok("aces") => bevy::core_pipeline::tonemapping::Tonemapping::AcesFitted,
            Ok("agx") => bevy::core_pipeline::tonemapping::Tonemapping::AgX,
            Ok("neutral") => bevy::core_pipeline::tonemapping::Tonemapping::KhronosPbrNeutral,
            Ok("reinhard") => bevy::core_pipeline::tonemapping::Tonemapping::Reinhard,
            Ok("luminance") => bevy::core_pipeline::tonemapping::Tonemapping::ReinhardLuminance,
            _ => bevy::core_pipeline::tonemapping::Tonemapping::TonyMcMapface,
        },
        crate::sky::SkyImage(sky.clone()),
        Skybox {
            image: Some(sky.clone()),
            brightness: crate::sky::SKY_BRIGHTNESS,
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
            // Both of these are set again by `dress` whenever the view changes or the
            // weather does, and this is the pair that was left behind when visibility
            // stopped being a constant: `dress` used to return early until the view
            // switched, so the opening scene kept the old 2600 m and every judgement
            // about the change was made against an unchanged picture.
            directional_light_color: crate::sky::glow_colour(&sky_state),
            directional_light_exponent: 3.0,
            falloff: FogFalloff::from_visibility_colors(
                crate::sky::visibility(&sky_state),
                crate::sky::haze_near(&sky_state),
                crate::sky::haze_colour(&sky_state),
            ),
        },
    ));
    // Worth a line in the log, now that it is weather rather than a constant: "why can
    // I not see the island" and "why can I see for miles" are both answered by it, and
    // neither is answerable from the picture alone.
    info!(
        "haze: visibility {:.0} m, cover {:.0}%, sun {:.0} deg",
        crate::sky::visibility(&sky_state),
        sky_state.cover * 100.0,
        sky_state.elevation().to_degrees()
    );
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
            View::Astern => View::Under,
            View::Under => View::Chase,
        };
    }
}

/// How far you can see under water, metres, and what colour it is.
///
/// Ten metres is a good day in these waters and a flattering one. The colour is green
/// rather than blue because this is a shelf sea with a plankton bloom in it in spring —
/// blue water is deep ocean water, and the archipelago is neither.
const UNDER_VISIBILITY: f32 = 10.0;
const UNDER_WATER: Color = Color::srgb(0.045, 0.145, 0.135);

/// Dress the camera for where it is: under the water, or above it.
///
/// Three things change together and they have to, because any one of them alone looks
/// worse than none: the fog, the sky, and what is left where nothing is drawn.
///
/// The skybox comes **off** rather than being dimmed. A skybox is drawn at infinity and
/// no distance falloff reaches it, so a fogged scene under a bright sky is a green room
/// with a hole in the ceiling. With it gone, everything past the fog is the clear colour,
/// and the clear colour is the water.
///
/// Runs on a change of view, **and on a change of weather** — not every frame, because
/// adding and removing a component is a structural change to the entity and doing it
/// sixty times a second to say the same thing would be sixty archetype moves a second.
///
/// The weather half is the half that was missing, and it cost two reports from the deck.
/// The haze is weather now — its colour and the distance you can see through it are both
/// read off the sky — but this ran on a change of view alone, so both were fixed at
/// whatever the sky was in the first frame of the game. The visibility went to sixty
/// metres in fog and the water stayed clear to the horizon; the dome went white and the
/// sea it stood over did not. Everything below is cheap except `haze_colour`, which
/// walks the horizon, so it is gated on the same test the dome's repaint uses rather
/// than run every frame.
pub fn dress(
    mut commands: Commands,
    view: Res<View>,
    sky: Res<crate::sky::Sky>,
    image: Query<(Entity, &crate::sky::SkyImage), With<MainCamera>>,
    mut fog: Query<&mut DistanceFog, With<MainCamera>>,
    mut camera: Query<&mut Camera, With<MainCamera>>,
    mut was: Local<Option<(View, crate::sky::Sky)>>,
) {
    let turned = was.is_none_or(|(seen, _)| seen != *view);
    let weathered = was.is_none_or(|(_, seen)| crate::sky::worth_repainting(&seen, &sky));
    if !turned && !weathered {
        return;
    }
    *was = Some((*view, *sky));
    let under = *view == View::Under;
    for mut fog in &mut fog {
        fog.color = if under {
            UNDER_WATER
        } else {
            crate::sky::haze_colour(&sky)
        };
        // **Haze is not the same brightness in every direction, and treating it as if
        // it were is what flattened the islands.** Light scatters forward: looking
        // towards the sun through four kilometres of air you are looking at lit
        // moisture, and looking away from it you are looking at moisture in its own
        // shade. That difference is most of what tells an eye which side of a hill is
        // which at a distance — so with the fog isotropic, the far shore came out as a
        // single pale band with no sunlit face and no dark one.
        //
        // Off underwater, where there is no sun to scatter and the whole point is that
        // the light comes from one flat direction above.
        fog.directional_light_color = if under {
            Color::NONE
        } else {
            crate::sky::glow_colour(&sky)
        };
        // Broad rather than tight: this is the whole lit half of the sky, not a disc.
        // Eight — Bevy's default — is a glow around the sun; three is the difference
        // between the sunward and shadowed quarters of a seascape.
        fog.directional_light_exponent = 3.0;
        fog.falloff = if under {
            FogFalloff::from_visibility_colors(UNDER_VISIBILITY, UNDER_WATER, UNDER_WATER)
        } else {
            FogFalloff::from_visibility_colors(
                crate::sky::visibility(&sky),
                crate::sky::haze_near(&sky),
                crate::sky::haze_colour(&sky),
            )
        };
    }
    // Worth a line in the log every time it moves, and not only at startup: "why can I
    // not see the island" and "why can I see for miles" are both answered by it, and
    // neither is answerable from the picture alone. Debug rather than info because this
    // one fires whenever the weather does.
    debug!(
        "haze: visibility {:.0} m, fog {:.0}%, cover {:.0}%, sun {:.0} deg",
        crate::sky::visibility(&sky),
        sky.fog * 100.0,
        sky.cover * 100.0,
        sky.elevation().to_degrees()
    );

    // The structural half, which is the part that must not happen every time the cloud
    // thickens: adding or removing a component moves the entity between archetypes.
    if !turned {
        return;
    }
    for mut camera in &mut camera {
        camera.clear_color = if under {
            bevy::camera::ClearColorConfig::Custom(UNDER_WATER)
        } else {
            bevy::camera::ClearColorConfig::Default
        };
    }
    for (entity, image) in &image {
        if under {
            commands.entity(entity).remove::<Skybox>();
        } else {
            commands.entity(entity).insert(Skybox {
                image: Some(image.0.clone()),
                brightness: crate::sky::SKY_BRIGHTNESS,
                ..default()
            });
        }
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

/// How far the outside camera's look point may move from the boat, metres.
///
/// Nine, which at the chase camera's distance swings the frame about twenty degrees — far
/// enough to say plainly which way to look and not so far that the boat leaves the
/// picture. She is the subject; the thing being looked at is something happening on her.
const CHASE_SWING: f32 = 9.0;

/// How far the underwater camera stands off the lure, metres, and how far below it.
///
/// Beyond it rather than inside the boat's side of it, so the composition is lure first
/// and the hull behind it going away — which is the picture, and is also the only
/// arrangement in which the line has anywhere to go in the frame.
const UNDER_STANDOFF: f32 = 3.2;
const UNDER_BELOW: f32 = 0.45;

#[allow(clippy::too_many_arguments)]
pub fn follow(
    eye: Eye,
    mut aim: ResMut<ViewAim>,
    mut attention: ResMut<Attention>,
    mut chase: ResMut<ChaseState>,
    mut head: ResMut<HeadMotion>,
    gear: Option<Res<crate::rod::Ratchet>>,
    fishing: Res<crate::fishing::Fishing>,
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
                // And the point it was looking at, which is a *world* position: left
                // set, coming back to the chase view after a few minutes aboard aims it
                // at where the fish was a mile astern, and it lerps home over a second
                // while you wonder what you are looking at.
                chase.looking_set = false;
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
                // And the point it was looking at, which is a *world* position: left
                // set, coming back to the chase view after a few minutes aboard aims it
                // at where the fish was a mile astern, and it lerps home over a second
                // while you wonder what you are looking at.
                chase.looking_set = false;
            }
            View::Under => {
                // Beside the lure if there is one in the water, and under her counter if
                // there is not — which is worth having anyway, since it is the only way
                // to see the keel and the rudder doing what the physics says they do.
                let surface = head.heave;
                let (stand, at) = match gear.as_deref().filter(|_| fishing.out()) {
                    Some(gear) => {
                        let out = (gear.far - Vec3::new(hull.x, surface, hull.z))
                            .with_y(0.0)
                            .normalize_or(Vec3::Z);
                        (
                            gear.far + out * UNDER_STANDOFF - Vec3::Y * UNDER_BELOW,
                            gear.far,
                        )
                    }
                    None => {
                        let astern = frame::heading_only(st) * Vec3::Z;
                        let keel = Vec3::new(hull.x, surface - 2.2, hull.z);
                        (keel + astern * 7.0 - Vec3::Y * 0.6, keel)
                    }
                };
                // Kept under the surface whatever the lure is doing. A camera that
                // breaks out of the water for a moment reads as a bug, not as a wave.
                let ceiling = surface - 0.55;
                cam.translation = Vec3::new(stand.x, stand.y.min(ceiling), stand.z);
                cam.look_at(at, Vec3::Y);
                chase.initialised = false;
                chase.looking_set = false;
            }
            View::Chase => {
                let heading = frame::heading_only(st);
                // Orbit. The chase camera swings round the boat rather than turning
                // on the spot, because what you want from outside is to look *at* her
                // from somewhere else, not to look away from her.
                let swing =
                    Quat::from_rotation_y(aim.yaw) * Quat::from_rotation_x(-aim.pitch * 0.6);
                // **And she is further away in a big sea**, because from twenty-one
                // metres a boat in a ten-metre swell spends half her time behind a wall
                // of water, and the wall is the thing worth looking at.
                //
                // Deliberately slow to start. An ordinary gale here is three and a half
                // metres and moves this by a fifth, which is not a change anybody will
                // notice; the band it is really for is the one the fetch dial opened up,
                // where the waves are taller than the mast. Capped, because past four
                // times out she is a speck and the camera is no longer following a boat.
                let sea = sim.sim.water.significant_height().max(0.0) as f32;
                let out = 1.0 + ((sea - 2.0).max(0.0) / 8.0).min(3.0);
                let target = Vec3::new(hull.x, head.heave, hull.z)
                    + heading * swing * Vec3::new(0.0, 9.0 * out, 21.0 * out);
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
                // Blended rather than aimed. Pointing it at the thing alone puts the
                // thing in the middle of the screen and the boat off the edge of it,
                // which is not what anybody asked for: the boat is the subject and what
                // is being looked at is something happening on her. Looking between them
                // keeps both in frame and swings the view the right way, which is what a
                // person standing off in a dinghy with a camera would do.
                //
                // By default the filtered height, or a sharp heave swings the whole
                // frame about a boat that has barely moved.
                let boat = Vec3::new(hull.x, head.heave + 4.5, hull.z);
                // How far the look point may travel from her, rather than a fraction of
                // the way to the thing. A fraction was tuned when the thing being looked
                // at was a fish tens of metres astern, and it quietly stopped working
                // when the claims moved to the rod: at five metres from her, four tenths
                // of the way is two metres, which at a camera twenty-three metres off is
                // six degrees — so a take, a run and a fish being played all resolved to
                // the same barely visible nudge and the ranks made no difference out
                // here at all.
                //
                // Capping the *distance* instead means anything near her is looked at
                // squarely, which is most of what the gear is, and anything far off is
                // leaned towards far enough to say where it is. She stays in frame
                // either way, which is the only thing the cap is for.
                let wanted = match focus {
                    Some(target) => {
                        let away = target - boat;
                        boat + away.normalize_or_zero() * away.length().min(CHASE_SWING)
                    }
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
