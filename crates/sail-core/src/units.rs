//! Unit conversions. The simulation is strictly SI internally (m, m/s, rad, N, kg, s);
//! knots and degrees exist only at the edges — tests, HUD, telemetry.

pub const KNOT: f64 = 0.514_444_444; // m/s
pub const NAUTICAL_MILE: f64 = 1852.0; // m
pub const GRAVITY: f64 = 9.806_65; // m/s^2
pub const RHO_AIR: f64 = 1.225; // kg/m^3 at 15 C
pub const RHO_SEA: f64 = 1025.0; // kg/m^3
pub const NU_SEA: f64 = 1.188e-6; // kinematic viscosity, m^2/s at 15 C

// `const`, for the same reason `rad` is: a sail's limit or a boat's hull speed written
// in knots wants to be a constant where it is used, and this is one multiplication.
#[inline]
pub const fn kn_to_ms(kn: f64) -> f64 {
    kn * KNOT
}

#[inline]
pub const fn ms_to_kn(ms: f64) -> f64 {
    ms / KNOT
}

// Both are `const`, which is not decoration: a step size or a limit written in degrees
// wants to be a constant where it is used, and `to_radians` is a multiplication, so
// there is no reason for the conversion to wait until run time.
#[inline]
pub const fn deg(rad: f64) -> f64 {
    rad * (180.0 / std::f64::consts::PI)
}

#[inline]
pub const fn rad(deg: f64) -> f64 {
    deg * (std::f64::consts::PI / 180.0)
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

/// A direction in points of the compass, the way it is written in a log.
///
/// **A log says "SW 5", not "237° 19.4 knots".** Figures are what an instrument reads;
/// points are what a person writes and what another person can picture a week later.
/// Sixteen of them, which is as fine as anybody writes a wind — the thirty-two-point
/// card is for steering, not for the book.
pub fn point(bearing: f64) -> &'static str {
    const POINTS: [&str; 16] = [
        "N", "NNE", "NE", "ENE", "E", "ESE", "SE", "SSE", "S", "SSW", "SW", "WSW", "W", "WNW",
        "NW", "NNW",
    ];
    let turns = bearing.rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU;
    POINTS[((turns * 16.0).round() as usize) % 16]
}

#[cfg(test)]
mod the_card {
    use super::*;

    /// Every point names itself, and the rounding falls where a card says it does.
    #[test]
    fn the_sixteen_points_are_where_they_should_be() {
        assert_eq!(point(0.0), "N");
        assert_eq!(point(rad(90.0)), "E");
        assert_eq!(point(rad(180.0)), "S");
        assert_eq!(point(rad(270.0)), "W");
        assert_eq!(point(rad(225.0)), "SW");
        assert_eq!(point(rad(360.0)), "N", "a full turn is north again");
        assert_eq!(point(rad(-45.0)), "NW", "and so is going the other way round");
        // Half a point either side of north is still north; a whole one is not.
        assert_eq!(point(rad(11.0)), "N");
        assert_eq!(point(rad(23.0)), "NNE");
    }
}

