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

    /// How much harder than the mean it is blowing at a point, as a fraction: 0.2 is a
    /// twenty percent gust and −0.2 a lull. Zero for a field with no structure in it.
    ///
    /// Exposed on the trait rather than kept inside the gust model because other things
    /// want to know where the gusts are. A cumulus sits over a downdraught, so the
    /// cloud layer places its cells by asking this — which makes the dark patch on the
    /// water and the extra pressure in the sails the same event rather than two
    /// decorations that happen to be on screen together.
    fn gust_at(&self, _pos: DVec2, _t: f64) -> f64 {
        0.0
    }
}

/// Gustiness as a field in space, carried over the water by the wind that makes it.
///
/// This replaced a purely temporal model, and the replacement is the point. The old
/// gust was an Ornstein-Uhlenbeck process on the speed: correct statistics at the boat,
/// and the *same* wind everywhere at once. So a gust did not arrive — it switched on, on
/// the whole ocean, which is the one thing about wind that a sailor can see is false. You
/// watch the water to windward because a gust is a patch travelling towards you, and
/// reading that patch is a real skill with a real pay-off.
///
/// What makes it cheap is that it is analytic: smooth value noise in two octaves,
/// evaluated at `pos` offset by how far the pattern has been carried. No state, no cells
/// to recycle, no synchronisation — the same function answers for the boat, for the wind
/// streaks on the water and for the cloud layer, and it answers for a point a kilometre
/// to windward just as happily as for one under the keel.
///
/// The old calibration is what sets the cell size, which is a pleasing way round. Twelve
/// percent intensity on a twenty-five second timescale was measured over open water; a
/// patch of size `L` carried past at `U` gives a timescale of `L/U`, so twenty-five
/// seconds at eight metres a second *is* a two hundred metre cell. That is the right
/// number for boundary-layer gust cells, and it was not chosen — it fell out.
#[derive(Clone, Copy, Debug)]
pub struct GustField {
    /// Size of a gust cell, metres.
    pub cell: f64,
    /// Standard deviation of the speed anomaly, as a fraction of the mean.
    pub intensity: f64,
    /// How fast the pattern travels compared with the wind carrying it. A gust patch
    /// runs a little slower than the air it is in, because it is a structure in the
    /// boundary layer rather than a parcel of it.
    pub advection: f64,
    seed: u64,
}

/// Standard deviation of [`GustField::shape`], measured rather than derived.
///
/// Pinned by `wind::tests::the_gust_field_has_the_intensity_it_claims`, because the
/// whole point of the number is that `intensity` means what it says: if this drifts,
/// every wind speed in the simulator is quietly wrong by a few percent and nothing
/// else says so.
const SHAPE_SIGMA: f64 = 0.330_6;

impl GustField {
    pub fn new(seed: u64) -> Self {
        Self {
            cell: 200.0,
            intensity: 0.12,
            advection: 0.85,
            seed,
        }
    }

    /// The anomaly at a point, as a fraction of the mean wind speed.
    pub fn at(&self, pos: DVec2, t: f64, carried_by: DVec2) -> f64 {
        if self.intensity <= 0.0 || self.cell <= 0.0 {
            return 0.0;
        }
        let drifted = pos - carried_by * (self.advection * t);
        self.shape(drifted / self.cell) * (self.intensity / SHAPE_SIGMA)
    }

    /// Unit-ish noise: two octaves, the second small and turned off-lattice so the two
    /// never line up and make a grid of it.
    fn shape(&self, p: DVec2) -> f64 {
        0.74 * value_noise(p, self.seed)
            + 0.26 * value_noise(p * 2.7 + DVec2::new(11.3, -7.1), self.seed ^ 0x9E37)
    }
}

/// Hash of a lattice point to [−1, 1]. Deterministic, stateless, and the reason the
/// field needs no memory.
fn lattice(ix: i64, iy: i64, seed: u64) -> f64 {
    let mut h = seed
        ^ (ix as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (iy as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 33;
    h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h ^= h >> 33;
    h = h.wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    h ^= h >> 33;
    (h >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
}

/// Smooth value noise. Smoothstep rather than linear interpolation, because the
/// derivative of the wind matters here: a kink in the field is a step in the force on
/// the rig, and the boat rings like a bell when it crosses one.
fn value_noise(p: DVec2, seed: u64) -> f64 {
    let i = p.floor();
    let f = p - i;
    let w = f * f * (DVec2::splat(3.0) - 2.0 * f);
    let (ix, iy) = (i.x as i64, i.y as i64);
    let a = lattice(ix, iy, seed);
    let b = lattice(ix + 1, iy, seed);
    let c = lattice(ix, iy + 1, seed);
    let d = lattice(ix + 1, iy + 1, seed);
    let top = a + (b - a) * w.x;
    let bottom = c + (d - c) * w.x;
    top + (bottom - top) * w.y
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
    /// The gusts, as a field in space rather than a wobble in time.
    pub gusts: GustField,
    /// Standard deviation of direction, radians.
    pub direction_spread: f64,
    /// Relaxation time of the direction process, seconds.
    pub shift_timescale: f64,
    /// How strongly a gust veers right, radians per unit normalised speed anomaly.
    pub veer_coupling: f64,

    direction_anomaly: f64,
    rng: Pcg32,
}

impl SyntheticWind {
    /// A plausible open-water day: moderate breeze, moderate gustiness.
    pub fn new(mean_direction: f64, mean_speed: f64, seed: u64) -> Self {
        Self {
            mean_direction,
            mean_speed,
            gusts: GustField::new(seed ^ 0x6057),
            direction_spread: 6.0_f64.to_radians(),
            shift_timescale: 90.0,
            veer_coupling: 7.0_f64.to_radians(),
            direction_anomaly: 0.0,
            rng: Pcg32::new(seed),
        }
    }

    /// Where the pattern of gusts is being carried, which is the mean flow.
    fn carried_by(&self) -> DVec2 {
        flow_from(self.mean_direction, self.mean_speed)
    }

    /// Speed at a point and an instant, at the reference height.
    pub fn speed_at(&self, pos: DVec2, t: f64) -> f64 {
        (self.mean_speed * (1.0 + self.gust_at(pos, t))).max(0.0)
    }

    /// Direction at a point and an instant, including the veer that goes with a gust.
    ///
    /// A gust over water arrives from the right of the mean in the northern hemisphere,
    /// because the air in it has come down from a height where it was turning less with
    /// friction. It is the shift you can set a tack up for, so it is modelled: the veer
    /// is tied to the local gust strength, which means it now arrives *with the patch*
    /// rather than happening everywhere at once.
    pub fn direction_at(&self, pos: DVec2, t: f64) -> f64 {
        wrap_tau(
            self.mean_direction
                + self.direction_anomaly
                + self.veer_coupling * self.gust_at(pos, t),
        )
    }

    /// Freeze the stochastic state. Handy for reproducing a moment while tuning.
    pub fn frozen(mut self) -> Self {
        self.gusts.intensity = 0.0;
        self.direction_spread = 0.0;
        self.direction_anomaly = 0.0;
        self
    }
}

impl WindField for SyntheticWind {
    fn sample(&self, pos: DVec2, t: f64) -> DVec2 {
        flow_from(self.direction_at(pos, t), self.speed_at(pos, t))
    }

    fn gust_at(&self, pos: DVec2, t: f64) -> f64 {
        self.gusts.at(pos, t, self.carried_by())
    }

    fn step(&mut self, dt: f64) {
        // Only the slow wander of the mean direction is a process in time now. The
        // gusts are a pattern in space and need no stepping — they arrive because the
        // wind carries them past, which is both cheaper and the actual mechanism.
        //
        // Exact discretisation of dX = -X/tau dt + sigma sqrt(2/tau) dW, so the step
        // size cannot change the statistics of the process.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `intensity` has to mean what it says, or every wind speed in the simulator is
    /// quietly a few percent wrong and nothing else says so. The shape function's own
    /// standard deviation is a measured constant, so this is the measurement.
    #[test]
    fn the_gust_field_has_the_intensity_it_claims() {
        let field = GustField::new(0xA11CE);
        // Sampled over a few square kilometres on an irrational stride, so the points
        // never fall into step with the lattice.
        let mut sum = 0.0;
        let mut sum_sq = 0.0;
        let mut n = 0.0;
        for i in 0..400 {
            for j in 0..400 {
                let p = DVec2::new(i as f64 * 17.37, j as f64 * 13.11);
                let x = field.at(p, 0.0, DVec2::ZERO);
                sum += x;
                sum_sq += x * x;
                n += 1.0;
            }
        }
        let mean = sum / n;
        let sigma = (sum_sq / n - mean * mean).sqrt();
        assert!(
            mean.abs() < 0.005,
            "a gust field with a mean in it is a wind speed error, got {mean:.4}"
        );
        assert!(
            (sigma - field.intensity).abs() < 0.004,
            "intensity {:.3} should be the standard deviation, got {sigma:.4} — \
             SHAPE_SIGMA wants to be {:.4}",
            field.intensity,
            SHAPE_SIGMA * sigma / field.intensity
        );
    }

    /// The pattern travels with the wind, and that is the whole reason it exists: a gust
    /// you can see coming is one that is somewhere else first.
    #[test]
    fn a_gust_arrives_from_upwind() {
        let field = GustField::new(7);
        let flow = flow_from(0.0, 8.0); // from the north, so flowing south
        let here = DVec2::ZERO;
        // Where the air at this point will be in a minute, upstream of us now.
        let upwind = here - flow * (field.advection * 60.0);
        let now = field.at(upwind, 0.0, flow);
        let later = field.at(here, 60.0, flow);
        assert!(
            (now - later).abs() < 1e-9,
            "what is upwind now should be here later: {now:.4} then {later:.4}"
        );
    }
}
