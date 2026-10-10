//! Unit conversions. The simulation is strictly SI internally (m, m/s, rad, N, kg, s);
//! knots and degrees exist only at the edges — tests, HUD, telemetry.

pub const KNOT: f64 = 0.514_444_444; // m/s
pub const NAUTICAL_MILE: f64 = 1852.0; // m
pub const GRAVITY: f64 = 9.806_65; // m/s^2
pub const RHO_AIR: f64 = 1.225; // kg/m^3 at 15 C
pub const RHO_SEA: f64 = 1025.0; // kg/m^3
pub const NU_SEA: f64 = 1.188e-6; // kinematic viscosity, m^2/s at 15 C

#[inline]
pub fn kn_to_ms(kn: f64) -> f64 {
    kn * KNOT
}

#[inline]
pub fn ms_to_kn(ms: f64) -> f64 {
    ms / KNOT
}

#[inline]
pub fn deg(rad: f64) -> f64 {
    rad.to_degrees()
}

#[inline]
pub fn rad(deg: f64) -> f64 {
    deg.to_radians()
}

/// Wrap to (-pi, pi]. Used for every heading and wind-angle difference; forgetting it
/// is how a boat ends up "turning the long way round" through 350 degrees.
#[inline]
pub fn wrap_pi(a: f64) -> f64 {
    let mut a = (a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU);
    if a <= 0.0 {
        a += std::f64::consts::TAU;
    }
    a - std::f64::consts::PI
}

/// Wrap to [0, 2pi). For compass bearings.
#[inline]
pub fn wrap_tau(a: f64) -> f64 {
    a.rem_euclid(std::f64::consts::TAU)
}

#[inline]
pub fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    if (edge1 - edge0).abs() < f64::EPSILON {
        return if x < edge0 { 0.0 } else { 1.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Classic displacement-hull speed limit, 1.34 * sqrt(LWL in feet) knots, in m/s.
/// Sailors feel this as a wall; the residuary-resistance curve has to reproduce it.
pub fn hull_speed(lwl_m: f64) -> f64 {
    kn_to_ms(1.34 * (lwl_m / 0.3048).sqrt())
}

/// Froude number on waterline length.
#[inline]
pub fn froude(speed_ms: f64, lwl_m: f64) -> f64 {
    speed_ms.abs() / (GRAVITY * lwl_m).sqrt()
}
