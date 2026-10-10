//! Hull hydrodynamics: resistance, lateral lift, rudder, righting moment.
//!
//! Honest about its approximations. Frictional resistance is the real ITTC-57 line.
//! Residuary resistance is a *calibrated* curve shaped like the Delft Systematic Yacht
//! Hull Series rather than the full DSYHS regression — that needs a dozen hull
//! coefficients nobody publishes for a production cruiser. `residuary_resistance` is
//! the single seam where the regression drops in later.

use crate::units::{froude, smoothstep, GRAVITY, NU_SEA, RHO_SEA};

/// ITTC-57 frictional line plus a roughness allowance. This part is not an
/// approximation — it is the standard towing-tank correlation.
pub fn frictional_resistance(speed: f64, lwl: f64, wetted_surface: f64) -> f64 {
    let v = speed.abs();
    if v < 1e-4 {
        return 0.0;
    }
    let re = (v * lwl / NU_SEA).max(1.0e5);
    let cf = 0.075 / (re.log10() - 2.0).powi(2) + 0.0004;
    0.5 * RHO_SEA * wetted_surface * cf * v * v * speed.signum()
}

/// Wave-making resistance, including the hull-speed wall.
///
/// Shape: proportional to Fn^4 at low speed, with a barrier that steepens past
/// Fn ~= 0.33. A displacement hull cannot climb its own bow wave, and a sailor feels
/// that as a hard ceiling — so the barrier is not cosmetic, it is the thing being
/// modelled.
pub fn residuary_resistance(speed: f64, lwl: f64, displacement: f64, coefficient: f64) -> f64 {
    let fn_ = froude(speed, lwl);
    if fn_ < 1e-6 {
        return 0.0;
    }
    // Two terms: the wave-making hump from Fn ~= 0.32, and a near-wall above Fn 0.40
    // that a displacement hull cannot climb through without surfing.
    let hump = 120.0 * (fn_ - 0.32).max(0.0).powi(2);
    let wall = 900.0 * (fn_ - 0.40).max(0.0).powi(3);
    let barrier = 1.0 + hump + wall;
    displacement * GRAVITY * coefficient * fn_.powi(4) * barrier * speed.signum()
}

/// Induced resistance: the price of generating side force with a keel.
///
/// Classic induced drag with the effective span taken as a fraction of draft. The
/// fraction is below one because the free surface lets the pressure difference
/// short-circuit over the top of the foil.
pub fn induced_resistance(side_force: f64, speed: f64, effective_span: f64) -> f64 {
    let v = speed.abs();
    if v < 0.05 || effective_span <= 0.0 {
        return 0.0;
    }
    let denom = 0.5 * RHO_SEA * std::f64::consts::PI * effective_span * effective_span * v * v;
    side_force * side_force / denom
}

/// A lifting surface — keel or rudder — with a linear lift slope and a stall.
///
/// The stall is the point of this function. Rudder stall above ~30 degrees is the
/// behaviour that separates a simulator from a toy: the helm loads up, then goes
/// light, and the boat stops answering.
#[derive(Clone, Copy, Debug)]
pub struct Foil {
    /// Planform area, m^2.
    pub area: f64,
    /// Lift slope, per radian. Lifting-line value for the effective aspect ratio.
    pub lift_slope: f64,
    /// Angle of attack at which lift peaks, radians.
    pub stall_angle: f64,
    /// Fraction of peak lift retained once fully stalled.
    pub post_stall_lift: f64,
    /// Parasitic drag coefficient.
    pub cd0: f64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FoilForces {
    pub alpha: f64,
    pub cl: f64,
    /// Lift magnitude, N; sign follows `alpha`.
    pub lift: f64,
    pub drag: f64,
    /// 0 attached, 1 fully stalled.
    pub stalled: f64,
}

impl Foil {
    /// Combined hull-and-keel lateral plane for a fin-keel cruiser.
    pub fn lateral_plane(area: f64, effective_aspect_ratio: f64) -> Self {
        Self {
            area,
            lift_slope: 2.0 * std::f64::consts::PI * effective_aspect_ratio
                / (effective_aspect_ratio + 2.0),
            stall_angle: 14.0_f64.to_radians(),
            post_stall_lift: 0.65,
            cd0: 0.008,
        }
    }

    pub fn rudder(area: f64, effective_aspect_ratio: f64) -> Self {
        Self {
            area,
            lift_slope: 2.0 * std::f64::consts::PI * effective_aspect_ratio
                / (effective_aspect_ratio + 2.0),
            stall_angle: 28.0_f64.to_radians(),
            post_stall_lift: 0.45,
            cd0: 0.010,
        }
    }

    pub fn forces(&self, alpha: f64, flow_speed: f64) -> FoilForces {
        let a = alpha.abs();
        let cl_linear = self.lift_slope * a;
        let cl_peak = self.lift_slope * self.stall_angle;

        // Past the stall angle, lift decays towards a separated plateau over the next
        // 15 degrees rather than falling off a cliff — which is how a real foil feels.
        let s = smoothstep(
            self.stall_angle,
            self.stall_angle + 15.0_f64.to_radians(),
            a,
        );
        let cl_mag = (1.0 - s) * cl_linear.min(cl_peak) + s * cl_peak * self.post_stall_lift;

        let q = 0.5 * RHO_SEA * self.area * flow_speed * flow_speed;
        let cd = self.cd0 + cl_mag * cl_mag * 0.06 + s * 0.9 * a.sin().abs();

        FoilForces {
            alpha,
            cl: cl_mag * alpha.signum(),
            lift: q * cl_mag * alpha.signum(),
            drag: q * cd,
            stalled: s,
        }
    }
}

/// Righting arm GZ.
///
/// `GM * sin(heel)` is accurate to roughly 25 degrees. Beyond that a real cruiser's
/// GZ curve peaks near 55-65 degrees and then collapses; reproducing that shape
/// matters because it is the difference between "stiff then softening" and a linear
/// spring, and sailors feel the knockdown.
pub fn righting_arm(heel: f64, gm: f64, gz_peak_angle: f64) -> f64 {
    let phi = heel.abs();
    let linear = gm * phi.sin();
    // Blend from the initial GM slope into a curve that peaks and falls away.
    let peak_gz = gm * gz_peak_angle.sin() * 0.78;
    let shaped = peak_gz * (std::f64::consts::PI * phi / (2.0 * gz_peak_angle)).sin();
    let w = smoothstep(gz_peak_angle * 0.45, gz_peak_angle, phi);
    ((1.0 - w) * linear + w * shaped) * heel.signum()
}

/// Wetted surface estimate when the real figure is unknown.
/// Standard displacement-hull approximation.
pub fn wetted_surface_estimate(displacement: f64, lwl: f64) -> f64 {
    let volume = displacement / RHO_SEA;
    2.6 * (volume * lwl).sqrt()
}
