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
    /// Seconds since start.
    pub time: f64,
}

impl BoatState {
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
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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
    /// Mainsail reef, 0 to 1 of the available reduction. Slab or in-mast reefing.
    pub reef: f64,
    /// Headsail furl, 0 fully unrolled to 1 rolled completely away. A roller-furling
    /// genoa is a separate piece of gear from the main's reef, and rolling it is
    /// usually the first thing anyone does — so it gets its own control.
    pub furl: f64,
    /// Engine, -1 astern to 1 ahead.
    pub throttle: f64,
}

impl Controls {
    /// Both sheets hard in, rudder centred, outhaul at a working upwind setting.
    pub fn close_hauled() -> Self {
        Self {
            outhaul: 0.5,
            ..Default::default()
        }
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

    pub main: SailForces,
    pub jib: SailForces,

    /// Net driving force and side force from the rig, N.
    pub drive: f64,
    pub side: f64,
    /// Total hull resistance, N.
    pub resistance: f64,
    /// Breakdown, for working out where the speed went.
    pub resistance_friction: f64,
    pub resistance_residuary: f64,
    pub resistance_induced: f64,

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
