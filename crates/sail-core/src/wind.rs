//! Wind field.
//!
//! Meteorological convention throughout: a wind *direction* is where the wind comes
//! FROM. The flow vector for direction `theta` (compass radians, from north, clockwise)
//! and speed `s` is `(-s*sin(theta), -s*cos(theta))` in world ENU — verified in
//! `tests/conventions.rs`.
//!
//! `WindField` is the seam where a real GRIB2 forecast replaces the synthetic model
//! without touching anything downstream.

use crate::rng::Pcg32;
use crate::units::{wrap_tau, RHO_AIR};
use glam::DVec2;

/// Reference height for a quoted wind speed, by convention 10 m above the surface.
pub const WIND_REF_HEIGHT: f64 = 10.0;

/// Power-law exponent for the marine boundary layer. 0.11 is the standard open-water
/// value; it means a 12 kn wind at 10 m is ~11.3 kn at the 6 m centre of effort.
pub const SHEAR_EXPONENT: f64 = 0.11;

/// Scale a reference wind speed to an arbitrary height. Sampling the rig at its centre
/// of effort rather than at 10 m matters: it is a few percent of drive force, and more
/// importantly it is the mechanism behind sail twist.
#[inline]
pub fn shear_factor(height: f64) -> f64 {
    (height.max(0.5) / WIND_REF_HEIGHT).powf(SHEAR_EXPONENT)
}

/// Dynamic pressure of air at a given speed.
#[inline]
pub fn q_air(speed: f64) -> f64 {
    0.5 * RHO_AIR * speed * speed
}

pub trait WindField: Send + Sync {
    /// True wind flow vector in world ENU at the reference height, in m/s.
    /// `pos` is in metres, `t` in seconds since simulation start.
    fn sample(&self, pos: DVec2, t: f64) -> DVec2;

    /// Advance any internal stochastic state. A pure/analytic field may ignore this.
    fn step(&mut self, _dt: f64) {}
}

/// Flow vector from a meteorological direction and speed.
#[inline]
pub fn flow_from(direction: f64, speed: f64) -> DVec2 {
    DVec2::new(-speed * direction.sin(), -speed * direction.cos())
}

/// Meteorological direction (where it comes from) of a flow vector.
#[inline]
pub fn direction_of(flow: DVec2) -> f64 {
    wrap_tau((-flow.x).atan2(-flow.y))
}

/// Steady wind. Useful as a baseline and for isolating a bug to the gust model.
#[derive(Clone, Debug)]
pub struct SteadyWind {
    pub direction: f64,
    pub speed: f64,
}

impl SteadyWind {
    pub fn new(direction: f64, speed: f64) -> Self {
        Self { direction, speed }
    }
}

impl WindField for SteadyWind {
    fn sample(&self, _pos: DVec2, _t: f64) -> DVec2 {
        flow_from(self.direction, self.speed)
    }
}

/// Synthetic but physical wind: an Ornstein-Uhlenbeck process on both speed and
/// direction.
///
/// Three properties make this feel like real wind rather than noise:
///   * turbulence intensity ~10-15% of the mean over open water,
///   * gust timescales of tens of seconds, not frames,
///   * gusts veer. In the northern hemisphere a gust brings down air with more
///     momentum from aloft, which arrives backed to the right of the surface wind.
///     Sailors steer on this, so an unveering gust reads as fake immediately.
#[derive(Clone, Debug)]
pub struct SyntheticWind {
    pub mean_direction: f64,
    pub mean_speed: f64,
    /// Standard deviation of speed as a fraction of the mean.
    pub turbulence_intensity: f64,
    /// Relaxation time of the speed process, seconds.
    pub gust_timescale: f64,
    /// Standard deviation of direction, radians.
    pub direction_spread: f64,
    /// Relaxation time of the direction process, seconds.
    pub shift_timescale: f64,
    /// How strongly a gust veers right, radians per unit normalised speed anomaly.
    pub veer_coupling: f64,

    speed_anomaly: f64,
    direction_anomaly: f64,
    rng: Pcg32,
}

impl SyntheticWind {
    /// A plausible open-water day: moderate breeze, moderate gustiness.
    pub fn new(mean_direction: f64, mean_speed: f64, seed: u64) -> Self {
        Self {
            mean_direction,
            mean_speed,
            turbulence_intensity: 0.12,
            gust_timescale: 25.0,
            direction_spread: 6.0_f64.to_radians(),
            shift_timescale: 90.0,
            veer_coupling: 7.0_f64.to_radians(),
            speed_anomaly: 0.0,
            direction_anomaly: 0.0,
            rng: Pcg32::new(seed),
        }
    }

    /// Current instantaneous speed at the reference height.
    pub fn speed(&self) -> f64 {
        (self.mean_speed * (1.0 + self.speed_anomaly)).max(0.0)
    }

    /// Current instantaneous direction, including the veer coupled to gust strength.
    pub fn direction(&self) -> f64 {
        wrap_tau(
            self.mean_direction + self.direction_anomaly + self.veer_coupling * self.speed_anomaly,
        )
    }

    /// Freeze the stochastic state. Handy for reproducing a moment while tuning.
    pub fn frozen(mut self) -> Self {
        self.turbulence_intensity = 0.0;
        self.direction_spread = 0.0;
        self.speed_anomaly = 0.0;
        self.direction_anomaly = 0.0;
        self
    }
}

impl WindField for SyntheticWind {
    fn sample(&self, _pos: DVec2, _t: f64) -> DVec2 {
        flow_from(self.direction(), self.speed())
    }

    fn step(&mut self, dt: f64) {
        // Exact discretisation of dX = -X/tau dt + sigma sqrt(2/tau) dW, so the step
        // size cannot change the statistics of the process.
        ou_step(
            &mut self.speed_anomaly,
            self.turbulence_intensity,
            self.gust_timescale,
            dt,
            &mut self.rng,
        );
        ou_step(
            &mut self.direction_anomaly,
            self.direction_spread,
            self.shift_timescale,
            dt,
            &mut self.rng,
        );
    }
}

fn ou_step(x: &mut f64, sigma: f64, tau: f64, dt: f64, rng: &mut Pcg32) {
    if sigma <= 0.0 || tau <= 0.0 {
        return;
    }
    let decay = (-dt / tau).exp();
    let noise = sigma * (1.0 - decay * decay).sqrt();
    *x = *x * decay + noise * rng.next_normal();
}
