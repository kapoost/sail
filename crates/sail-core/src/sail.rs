//! Sail aerodynamics.
//!
//! Force model after Hazen (1980): lift and drag coefficients as functions of the
//! angle of attack, decomposed into a driving force along the hull axis and a side
//! force athwartships. Beyond separation the coefficients blend into flat-plate
//! behaviour, which is what actually happens to a mainsail squared off on a run.
//!
//! The load-bearing design decision: coefficients are a function of **angle of
//! attack**, not of apparent wind angle. A VPP assumes optimal trim and can therefore
//! tabulate against apparent wind angle alone; a game cannot, because trimming is the
//! player's job. The sheet sets the chord angle, the chord angle sets the angle of
//! attack, and the angle of attack decides whether the sail drives, luffs or stalls.

use crate::units::{rad, smoothstep};
use crate::wind::q_air;

/// Aerodynamic character of one sail.
#[derive(Clone, Copy, Debug)]
pub struct SailProfile {
    /// Peak lift coefficient.
    pub cl_max: f64,
    /// Angle of attack at peak lift, radians.
    pub alpha_opt: f64,
    /// Angle of attack at which separation is substantially complete, radians. Well
    /// past the best angle, because a sail stalls softly.
    pub alpha_stall: f64,
    /// Parasitic drag: cloth, rig, mast interference.
    pub cd0: f64,
    /// Geometric aspect ratio, luff height squared over area.
    pub aspect_ratio: f64,
    /// Below this angle of attack the sail starts to shake, radians.
    pub luff_margin: f64,
    /// Normal-force coefficient once fully separated. A flat plate is 2.0; a sail with
    /// camber and a mast in the way runs nearer 1.5.
    pub cn_separated: f64,
    /// Largest chord angle the sheet can reach, radians. For a main this is the boom
    /// hitting the shrouds; past it, deep downwind angles are necessarily stalled.
    pub max_chord_angle: f64,
}

impl SailProfile {
    /// Mainsail on a cruising rig: moderate camber, full-length luff, mast ahead of it.
    pub fn mainsail(area: f64, luff_height: f64) -> Self {
        Self {
            cl_max: 1.45,
            alpha_opt: rad(18.0),
            alpha_stall: rad(34.0),
            cd0: 0.055,
            aspect_ratio: luff_height * luff_height / area,
            luff_margin: rad(4.0),
            cn_separated: 1.5,
            max_chord_angle: rad(90.0),
        }
    }

    /// Overlapping genoa: cleaner leading edge than a main, so slightly more lift and
    /// a slightly earlier peak, but it cannot be eased as far before the shrouds.
    pub fn genoa(area: f64, luff_height: f64) -> Self {
        Self {
            cl_max: 1.55,
            alpha_opt: rad(16.0),
            alpha_stall: rad(30.0),
            cd0: 0.045,
            aspect_ratio: luff_height * luff_height / area,
            luff_margin: rad(3.5),
            cn_separated: 1.45,
            max_chord_angle: rad(85.0),
        }
    }

    /// Lift and drag coefficients, plus how luffing and how stalled the sail is.
    ///
    /// `alpha` may be negative (sheet eased past the apparent wind, sail flogging) or
    /// well past 90 degrees (boom squared off on a run).
    pub fn coefficients(&self, alpha: f64) -> SailCoeffs {
        // Eased past the wind: the sail flogs. No useful lift, and real flogging drag.
        if alpha <= 0.0 {
            return SailCoeffs {
                cl: 0.0,
                cd: self.cd0 + 0.30,
                luffing: 1.0,
                stalled: 0.0,
            };
        }

        let luffing = 1.0 - smoothstep(0.0, self.luff_margin, alpha);

        // Attached-flow branch: rise to the peak, then a gentle post-peak decay.
        let cl_attached = if alpha <= self.alpha_opt {
            self.cl_max * (std::f64::consts::FRAC_PI_2 * alpha / self.alpha_opt).sin()
        } else {
            let t =
                ((alpha - self.alpha_opt) / (self.alpha_stall - self.alpha_opt)).clamp(0.0, 1.0);
            self.cl_max * (1.0 - 0.55 * t * t)
        };
        let separation = if alpha > self.alpha_opt {
            let t =
                ((alpha - self.alpha_opt) / (self.alpha_stall - self.alpha_opt)).clamp(0.0, 1.0);
            0.35 * t * t
        } else {
            0.0
        };
        let cd_attached = self.cd0
            + cl_attached * cl_attached / (std::f64::consts::PI * self.aspect_ratio.max(0.5))
            + separation;

        // Fully separated branch: a cambered plate presenting its area to the flow.
        let cl_plate = self.cn_separated * alpha.sin() * alpha.cos();
        let cd_plate = self.cd0 + self.cn_separated * alpha.sin() * alpha.sin();

        let s = smoothstep(self.alpha_stall, self.alpha_stall + rad(25.0), alpha);
        SailCoeffs {
            cl: (1.0 - s) * cl_attached * (1.0 - luffing) + s * cl_plate,
            cd: (1.0 - s) * cd_attached + s * cd_plate,
            luffing,
            stalled: s,
        }
    }

    /// Chord angle that puts this sail at its best angle of attack for a given
    /// apparent wind angle. Used by the validation suite, and by the in-game trim
    /// reference: this is the line a good trimmer is chasing.
    pub fn optimal_chord_angle(&self, awa: f64) -> f64 {
        (awa.abs() - self.alpha_opt).clamp(0.0, self.max_chord_angle)
    }

    /// Boom angle for a sheet position, where 0 is hard in and 1 fully eased.
    ///
    /// **Deliberately non-linear, and this is an ergonomic decision rather than a
    /// physical one.** Close-hauled wants a chord angle around eight degrees out of a
    /// ninety-degree range, so a linear sheet puts the entire upwind trim band inside
    /// its first tenth — unusable, and it reads to the player as a sail that jumps
    /// straight from stalled to luffing with nothing in between. Squaring the input
    /// spreads that band across the first third of the travel, where the hand can
    /// actually place it, and leaves the coarse end for reaching and running where a
    /// few degrees either way costs nothing.
    #[inline]
    pub fn chord_angle(&self, sheet: f64) -> f64 {
        let s = sheet.clamp(0.0, 1.0);
        self.max_chord_angle * s * s
    }

    /// Inverse of [`chord_angle`](Self::chord_angle).
    #[inline]
    pub fn sheet_for_chord_angle(&self, angle: f64) -> f64 {
        (angle.clamp(0.0, self.max_chord_angle) / self.max_chord_angle).sqrt()
    }

    /// Sheet position a good trimmer would be holding for this apparent wind angle.
    pub fn optimal_sheet(&self, awa: f64) -> f64 {
        self.sheet_for_chord_angle(self.optimal_chord_angle(awa))
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SailCoeffs {
    pub cl: f64,
    pub cd: f64,
    /// 0 = drawing, 1 = flogging. Drives telltales and sound.
    pub luffing: f64,
    /// 0 = attached, 1 = fully separated.
    pub stalled: f64,
}

/// Everything one sail contributes, in the body frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct SailForces {
    pub alpha: f64,
    pub chord_angle: f64,
    pub coeffs: SailCoeffs,
    pub lift: f64,
    pub drag: f64,
    /// Along the hull axis, bow positive.
    pub drive: f64,
    /// Athwartships, starboard positive. Negative with the wind on the starboard side.
    pub side: f64,
}

/// Forces from one sail.
///
/// * `awa` — signed apparent wind angle at the centre of effort, positive with the
///   wind on the starboard side.
/// * `aws` — apparent wind speed at the centre of effort.
/// * `sheet` — 0 is hard in, 1 is fully eased.
/// * `area` — effective area, already reduced for any reef.
/// * `heel` — signed heel angle; a heeled rig presents less area to the wind.
/// * `blanketing` — 0 is clear air, 1 is fully shadowed by the sail ahead.
pub fn sail_forces(
    profile: &SailProfile,
    awa: f64,
    aws: f64,
    sheet: f64,
    area: f64,
    heel: f64,
    blanketing: f64,
) -> SailForces {
    let beta = awa.abs();
    let chord_angle = profile.chord_angle(sheet);
    let alpha = beta - chord_angle;
    let coeffs = profile.coefficients(alpha);

    // A heeled rig presents a smaller projected area to a horizontal wind. Applying
    // cos(heel) to both force components is the standard first-order treatment, and it
    // produces the self-limiting behaviour sailors rely on: heel costs you drive.
    let heel_factor = heel.cos().abs().max(0.0);
    let clear_air = (1.0 - blanketing.clamp(0.0, 1.0)).max(0.0);
    let q = q_air(aws) * area * heel_factor * clear_air;

    let lift = q * coeffs.cl;
    let drag = q * coeffs.cd;

    // Hazen decomposition: lift acts normal to the apparent wind, drag along it.
    let drive = lift * beta.sin() - drag * beta.cos();
    let side_magnitude = lift * beta.cos() + drag * beta.sin();
    let side = -awa.signum() * side_magnitude;

    SailForces {
        alpha,
        chord_angle,
        coeffs,
        lift,
        drag,
        drive,
        side,
    }
}

/// How much of the headsail's air the mainsail is stealing.
///
/// Real and worth modelling even in a proof of concept: it is the whole reason a boat
/// poles out a genoa or gybes downwind instead of running dead. Below 120 degrees of
/// apparent wind the headsail is in clear air; by 180 it is almost entirely dead.
pub fn headsail_blanketing(awa: f64) -> f64 {
    0.85 * smoothstep(rad(120.0), rad(175.0), awa.abs())
}
