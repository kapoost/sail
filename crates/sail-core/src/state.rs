//! Simulation state, player controls, and telemetry.
//!
//! Sign conventions, stated once because a sign error in a moment is a day of
//! debugging. Verified in `tests/conventions.rs`.
//!
//! * World: `x` east, `y` north, metres. Heading measured from north, clockwise.
//! * Body: `x` bow, `y` starboard, `z` down. Right-handed, so a positive yaw moment
//!   swings the bow to starboard and a positive roll moment heels to starboard.
//! * Heel is positive to starboard. Wind on the starboard side therefore gives
//!   negative heel.
//! * Apparent wind angle is positive with the wind on the starboard side.
//! * Rudder angle is positive when it turns the bow to starboard.

use crate::sail::SailForces;
use glam::DVec2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoatState {
    /// World position, metres.
    pub pos: DVec2,
    /// Heading from north, clockwise, radians.
    pub heading: f64,
    /// Heel, radians, positive to starboard.
    pub heel: f64,
    /// Surge: speed along the hull axis, m/s.
    pub surge: f64,
    /// Sway: speed athwartships, m/s, positive to starboard.
    pub sway: f64,
    /// Yaw rate, rad/s, positive turning to starboard.
    pub yaw_rate: f64,
    /// Roll rate, rad/s.
    pub roll_rate: f64,
    /// Heave: elevation of the hull reference above mean water level, metres,
    /// positive up. Zero means floating at the design waterline in flat water.
    pub heave: f64,
    pub heave_rate: f64,
    /// Pitch, radians, positive bow-up.
    pub pitch: f64,
    pub pitch_rate: f64,
    /// Seconds since start.
    pub time: f64,
}

impl BoatState {
    /// Blend two states, for drawing a frame that falls between two physics substeps.
    ///
    /// The integrator advances in fixed 5 ms steps and a frame almost never lands on
    /// one, so the leftover has to be interpolated or the picture stutters — see
    /// [`Simulation::render_state`](crate::sim::Simulation::render_state).
    ///
    /// Heading goes the short way round. Lerping 359 degrees to 1 degree through the
    /// middle spins the boat the wrong way through 358 of them, once per lap, which
    /// is a bug that hides until somebody sails north.
    pub fn lerp(a: &Self, b: &Self, t: f64) -> Self {
        let f = |x: f64, y: f64| x + (y - x) * t;
        Self {
            pos: a.pos + (b.pos - a.pos) * t,
            heading: crate::units::wrap_tau(
                a.heading + crate::units::wrap_pi(b.heading - a.heading) * t,
            ),
            heel: f(a.heel, b.heel),
            surge: f(a.surge, b.surge),
            sway: f(a.sway, b.sway),
            yaw_rate: f(a.yaw_rate, b.yaw_rate),
            roll_rate: f(a.roll_rate, b.roll_rate),
            heave: f(a.heave, b.heave),
            heave_rate: f(a.heave_rate, b.heave_rate),
            pitch: f(a.pitch, b.pitch),
            pitch_rate: f(a.pitch_rate, b.pitch_rate),
            time: f(a.time, b.time),
        }
    }

    /// Stopped, on the given heading.
    pub fn at_rest(heading: f64) -> Self {
        Self {
            pos: DVec2::ZERO,
            heading,
            heel: 0.0,
            surge: 0.0,
            sway: 0.0,
            yaw_rate: 0.0,
            roll_rate: 0.0,
            heave: 0.0,
            heave_rate: 0.0,
            pitch: 0.0,
            pitch_rate: 0.0,
            time: 0.0,
        }
    }

    /// Unit vector along the hull axis, in world coordinates.
    #[inline]
    pub fn forward(&self) -> DVec2 {
        DVec2::new(self.heading.sin(), self.heading.cos())
    }

    /// Unit vector to starboard, in world coordinates.
    #[inline]
    pub fn starboard(&self) -> DVec2 {
        DVec2::new(self.heading.cos(), -self.heading.sin())
    }

    /// Velocity over ground, world coordinates.
    #[inline]
    pub fn velocity_world(&self) -> DVec2 {
        self.forward() * self.surge + self.starboard() * self.sway
    }

    /// Rotate a world vector into the body frame. The transform is its own inverse in
    /// this parameterisation.
    #[inline]
    pub fn to_body(&self, world: DVec2) -> DVec2 {
        let (s, c) = (self.heading.sin(), self.heading.cos());
        DVec2::new(world.x * s + world.y * c, world.x * c - world.y * s)
    }

    /// Speed through the water, m/s. Over flat water with no current this equals
    /// speed over ground.
    #[inline]
    pub fn speed(&self) -> f64 {
        (self.surge * self.surge + self.sway * self.sway).sqrt()
    }

    /// Leeway: the angle between where the bow points and where the boat actually
    /// goes, radians. Positive means slipping to starboard.
    #[inline]
    pub fn leeway(&self) -> f64 {
        if self.surge.abs() < 1e-3 {
            0.0
        } else {
            self.sway.atan2(self.surge)
        }
    }
}

/// What the player is doing with their hands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Controls {
    /// Rudder angle, radians, positive turns the bow to starboard.
    pub rudder: f64,
    /// Mainsheet: 0 hard in, 1 fully eased.
    pub main_sheet: f64,
    /// Genoa sheet: 0 hard in, 1 fully eased.
    pub jib_sheet: f64,
    /// Outhaul: 0 eased right off for a deep, powerful main, 1 hard out for a flat
    /// one. Depth is power; flat is less heeling force and less drag. The call changes
    /// with the breeze, which is why it is a control and not a constant.
    pub outhaul: f64,
    /// Boom vang, 0 eased right off to 1 hard on. Holds the leech down, and therefore
    /// controls twist — but only once the boom is out far enough that the mainsheet
    /// has stopped doing the job itself.
    pub vang: f64,
    /// Mainsail reef, 0 to 1 of the available reduction. Slab or in-mast reefing.
    pub reef: f64,
    /// Headsail sheet lead car, 0 right forward to 1 right aft. The genoa's outhaul
    /// and vang in one fitting: forward for depth and a closed leech, aft for a flat
    /// foot and a head that spills.
    pub genoa_car: f64,
    /// Headsail furl, 0 fully unrolled to 1 rolled completely away. A roller-furling
    /// genoa is a separate piece of gear from the main's reef, and rolling it is
    /// usually the first thing anyone does — so it gets its own control.
    pub furl: f64,
    /// Engine, -1 astern to 1 ahead.
    pub throttle: f64,
}

/// Hand-written rather than derived, because a derived default would put the outhaul
/// fully eased — the deepest possible main, not the shape the sail coefficients were
/// calibrated at. Every test and warm-up step that reaches for `Controls::default()`
/// would quietly be measuring a different sail.
impl Default for Controls {
    fn default() -> Self {
        Self {
            rudder: 0.0,
            main_sheet: 0.0,
            jib_sheet: 0.0,
            outhaul: 0.5,
            vang: 0.6,
            genoa_car: 0.4,
            reef: 0.0,
            furl: 0.0,
            throttle: 0.0,
        }
    }
}

impl Controls {
    /// Both sheets hard in, rudder centred, outhaul at a working upwind setting.
    pub fn close_hauled() -> Self {
        Self::default()
    }
}

/// Everything worth putting on a gauge, a telltale, or a chart. The polar overlay
/// built from `vmg` and `speed` is the single most useful development instrument.
#[derive(Clone, Copy, Debug, Default)]
pub struct Telemetry {
    /// True wind speed and direction at reference height.
    pub tws: f64,
    pub twd: f64,
    /// True wind angle relative to the bow, positive from starboard.
    pub twa: f64,
    /// Apparent wind at the rig, speed and angle.
    pub aws: f64,
    pub awa: f64,
    /// Height above the water the apparent wind was read at, metres.
    ///
    /// Not a detail. The true wind is quoted at ten metres and the boundary layer
    /// means it is weaker lower down, so `aws` and `tws` are not comparable without
    /// knowing where each was measured. Leaving this implicit is how
    /// `aws = tws - stw` stops holding for reasons that look like a bug.
    pub wind_height: f64,
    /// Speed through the water and over ground, m/s.
    pub stw: f64,
    pub sog: f64,
    /// Course over ground, radians from north.
    pub cog: f64,
    /// Velocity made good towards the wind, m/s. Positive is upwind progress.
    pub vmg: f64,
    pub heel: f64,
    pub leeway: f64,
    pub yaw_rate: f64,

    /// Lower section of the mainsail — the one whose chord angle is the boom's.
    pub main: SailForces,
    /// Upper section, eased by the twist in the leech.
    pub main_upper: SailForces,
    /// Twist between the two, radians.
    pub twist: f64,
    /// Lower section of the headsail.
    pub jib: SailForces,
    /// Upper section of the headsail, eased by the car's twist.
    pub jib_upper: SailForces,
    pub jib_twist: f64,

    /// Sea state and the boat's motion in it.
    pub significant_height: f64,
    pub heave: f64,
    pub pitch: f64,
    /// Mean immersion across the hull stations, metres. The design figure is the canoe
    /// body draft; much more than that and the boat is burying itself.
    pub immersion: f64,
    /// Vertical acceleration at the hull, m/s^2. What the crew actually feels, and
    /// what makes a short steep sea so much worse than a long one of the same height.
    pub heave_accel: f64,

    /// Net driving force and side force from the rig, N.
    pub drive: f64,
    pub side: f64,
    /// Total hull resistance, N.
    pub resistance: f64,
    /// Breakdown, for working out where the speed went.
    pub resistance_friction: f64,
    pub resistance_residuary: f64,
    pub resistance_induced: f64,
    /// Added resistance from moving in a seaway. Zero in flat water.
    pub resistance_waves: f64,
    /// Component of the boat's weight acting along its axis, newtons. Positive is
    /// forward: bow-down on a wave face, which is surfing.
    pub surf_force: f64,

    /// Angle of attack on the lateral plane, radians — the keel's working angle
    /// relative to the water it is actually in, which in a seaway is not the same as
    /// the boat's track over the ground.
    pub keel_alpha: f64,
    pub keel_stalled: f64,
    /// Flow at the keel in the body frame, m/s. Diagnostic: when this reverses, the
    /// boat is being told the water is overtaking it, and everything downstream lies.
    pub keel_surge: f64,
    pub keel_sway: f64,
    /// Rudder angle of attack and how stalled it is.
    pub rudder_alpha: f64,
    pub rudder_stalled: f64,
    /// Net yaw moment with the rudder centred — positive means the boat wants to luff.
    /// This is weather helm, expressed as a number.
    pub weather_helm: f64,
    /// Righting arm at the current heel, m.
    pub righting_arm: f64,
}

impl Telemetry {
    /// True if either sail is shaking enough to see or hear.
    pub fn luffing(&self) -> bool {
        self.main.coeffs.luffing > 0.25 || self.jib.coeffs.luffing > 0.25
    }
}
