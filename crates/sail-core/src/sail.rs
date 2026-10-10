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

/// Camber the tuned coefficients were calibrated at: maximum draft as a fraction of
/// chord. Roughly a working upwind shape for a cruising main.
pub const CAMBER_REFERENCE: f64 = 0.11;
/// Draft with the outhaul eased right off. Deep, powerful, draggy.
pub const CAMBER_DEEP: f64 = 0.15;
/// Draft with the outhaul hard out. Flat, less power, less heeling force.
pub const CAMBER_FLAT: f64 = 0.07;

/// How far the best angle of attack moves per unit of relative camber. A deeper sail
/// peaks earlier because the camber is already doing part of the work.
pub const ALPHA_SHIFT_PER_DEPTH: f64 = 3.0 * std::f64::consts::PI / 180.0;

/// Mainsail draft for an outhaul setting: 0 eased and deep, 1 hard out and flat.
///
/// What the outhaul actually does is pull the clew aft along the boom, tensioning the
/// foot and flattening the lower third of the sail. The depth here is still a single
/// number for the whole sail, so it overstates the outhaul's reach — but the mainsail
/// now carries two sections for twist, so the *vang* is modelled where it belongs
/// rather than being lumped in here. The cunningham and backstay remain unmodelled.
#[inline]
pub fn camber_for_outhaul(outhaul: f64) -> f64 {
    let o = outhaul.clamp(0.0, 1.0);
    CAMBER_DEEP + (CAMBER_FLAT - CAMBER_DEEP) * o
}

/// Largest twist the leech will fall off to with the vang eased right off, radians.
pub const MAX_TWIST: f64 = 20.0 * std::f64::consts::PI / 180.0;

/// Twist in the mainsail's leech, in radians, from the vang and the boom's angle.
///
/// The second argument is what makes this a real control rather than a dial. With the
/// boom near the centreline the mainsheet runs almost vertically, so sheeting in
/// tensions the leech all by itself and the vang has nothing left to do — which is why
/// nobody thinks about the vang upwind. Let the boom out and the sheet pulls sideways
/// instead of down; the leech falls off, the top of the sail spills, and the vang is
/// suddenly the only thing holding the shape.
///
/// Twist is not a defect to be eliminated. The apparent wind aloft is **freer** than
/// on deck, because the true wind is stronger up there while the boat's speed is the
/// same — so some twist is exactly right, and the amount depends on the gradient.
/// That optimum is what gives the vang something to be set correctly or incorrectly.
///
/// This is the traveller-amidships case. Use [`twist_for_leech`] when the traveller
/// has been moved, because moving it is precisely a change to this geometry.
pub fn twist_for_vang(vang: f64, chord_angle: f64) -> f64 {
    twist_for_leech(vang, chord_angle.cos().clamp(0.0, 1.0))
}

/// Twist from the vang competing with whatever downward pull the sheet already has.
pub fn twist_for_leech(vang: f64, sheet_pull: f64) -> f64 {
    let leech_tension = vang.clamp(0.0, 1.0).max(sheet_pull.clamp(0.0, 1.0));
    MAX_TWIST * (1.0 - leech_tension)
}

/// Half the mainsheet traveller's track, as a fraction of beam.
///
/// About 0.6 m each way on a 34-footer, which is a real cockpit-bridge track.
pub const TRAVELLER_HALF: f64 = 0.17;

/// Height of the boom above the traveller track, as a fraction of beam.
const SHEET_RISE: f64 = 0.62;

/// Boom length as a fraction of beam.
///
/// Both hulls in the yard scale the same way — 4.2 m on the 3.5 m-beam thirty-four,
/// 5.9 m on the 4.8 m-beam fifty-three — so one ratio serves rather than another field
/// in every config. If a boat ever breaks the pattern this becomes a `SailPlan` field.
const BOOM_PER_BEAM: f64 = 1.2;

/// Where the mainsheet car sits, in metres to leeward of the centreline.
///
/// `traveller` is signed in boat terms: −1 is the port end of the track, +1 the
/// starboard end — because that is what the hardware does, and the player can see
/// which way the car has gone. `lee` is +1 when the leeward side is starboard.
fn car_to_leeward(traveller: f64, lee: f64, beam: f64) -> f64 {
    traveller.clamp(-1.0, 1.0) * lee * TRAVELLER_HALF * beam
}

/// Extra boom angle from easing the traveller down to leeward, radians.
///
/// The mainsheet is not made fast to the centreline. It is made fast to a car that
/// runs *across* the boat, and that car decides which way the sheet pulls the boom
/// end. Slide the car to leeward with the sheet untouched and the boom end follows it,
/// because the sheet is nearly vertical and the boom end hangs under wherever the car
/// is. The angle opens; the leech stays exactly as tight as it was.
///
/// That is the whole reason the fitting exists, and it is the gust response a sailor
/// reaches for first: ease the sheet and you open the sail but spill the top third
/// with it; drop the traveller and you open the same angle with the sail still working
/// all the way up. Hauling the car to windward of amidships cannot push the boom past
/// the centreline, so it adds no angle — it only changes where the sheet pulls from,
/// which is the next function.
pub fn traveller_opening(traveller: f64, lee: f64, beam: f64) -> f64 {
    let lee_travel = car_to_leeward(traveller, lee, beam).max(0.0);
    (lee_travel / (BOOM_PER_BEAM * beam).max(0.1))
        .clamp(0.0, 1.0)
        .asin()
}

/// How much of the mainsheet's pull goes *down* the leech rather than sideways.
///
/// The sheet runs from the boom end down to the car, so its downward fraction is set
/// by how far the car is offset from under the boom end. Directly underneath and the
/// sheet is vertical: all of its pull is leech tension. Car amidships with the boom
/// well out and the sheet lies over at an angle, pulling the boom inboard rather than
/// down, and the leech goes slack no matter how hard you grind.
///
/// With the traveller centred this reduces to the `cos(boom angle)` that the old model
/// asserted — so the whole sail-shape calibration is unchanged — except that now it is
/// a consequence of the geometry instead of a guess, and the traveller moves it.
pub fn sheet_leech_pull(chord_angle: f64, traveller: f64, lee: f64, beam: f64) -> f64 {
    let car = car_to_leeward(traveller, lee, beam);
    let boom_end = chord_angle.abs().sin() * BOOM_PER_BEAM * beam;
    let across = boom_end - car;
    let rise = SHEET_RISE * beam;
    rise / (rise * rise + across * across).sqrt()
}

/// Largest twist a headsail's leech will fall off to with the car right aft, radians.
pub const MAX_LEAD_TWIST: f64 = 15.0 * std::f64::consts::PI / 180.0;

/// Headsail depth from the sheet lead car: 0 is the car right forward, 1 right aft.
///
/// The car is the headsail's outhaul and vang in one fitting, which is why a genoa has
/// no separate controls for either. Forward, the sheet pulls more downward: it
/// tensions the leech and lets the foot belly out, giving a deep sail with a closed
/// top. Aft, it pulls more along the foot: flat down low, and the leech falls open so
/// the head spills. Light air and pointing want the car forward; being overpowered
/// wants it aft.
pub fn camber_for_lead(car: f64) -> f64 {
    let c = car.clamp(0.0, 1.0);
    CAMBER_DEEP + (CAMBER_FLAT - CAMBER_DEEP) * c
}

/// Twist in a headsail's leech from the car position, radians.
///
/// Unlike the mainsail's vang, there is no competition with the sheet to model: the
/// car *is* the geometry that decides how much of the sheet's pull goes downward, so
/// its position sets the twist directly.
pub fn twist_for_lead(car: f64) -> f64 {
    MAX_LEAD_TWIST * car.clamp(0.0, 1.0)
}

/// Outhaul a trimmer would carry, given how hard the boat is already heeling.
///
/// Keyed on heel rather than wind speed, which is both what a trimmer actually reacts
/// to and what a velocity prediction program does: full sail is fastest until the boat
/// becomes limited by its righting moment, and only then does reducing force start to
/// *increase* VMG. An earlier version of this crossed over around fourteen knots on the
/// strength of received wisdom; sweeping the model (see `examples/outhaul.rs`) showed
/// depth winning at every wind speed the boat could still hold a course at, which is
/// the same answer an ORC VPP gives. The function was wrong, not the physics.
///
/// `target_heel` is the angle past which the boat is judged overpowered — around
/// twenty degrees for a cruising crew.
pub fn outhaul_for_heel(heel: f64, target_heel: f64) -> f64 {
    smoothstep(target_heel, target_heel + rad(9.0), heel.abs())
}

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
    /// Smallest chord angle the sheet can reach, radians.
    ///
    /// For a headsail this is not zero and never was: the sheet is led to a block on a
    /// track bolted to the side deck, so the clew cannot come inboard past the line
    /// from the tack to that block however hard the winch is ground. A cruiser's genoa
    /// lead gives something in the region of twelve degrees.
    ///
    /// Leaving it at zero had the model believe a genoa sheeted hard in lay on the
    /// centreline, which upwind means an angle of attack equal to the whole apparent
    /// wind angle — past the optimum and into soft stall. The boat was fastest with
    /// the genoa *eased*, which is the opposite of how one sails. The mainsail's floor
    /// genuinely is zero, because the traveller can bring the boom to the centreline.
    pub min_chord_angle: f64,
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
            min_chord_angle: 0.0,
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
            min_chord_angle: rad(12.0),
        }
    }

    /// Lift and drag coefficients, plus how luffing and how stalled the sail is.
    ///
    /// `alpha` may be negative (sheet eased past the apparent wind, sail flogging) or
    /// well past 90 degrees (boom squared off on a run). `camber` is the maximum draft
    /// as a fraction of chord — see [`camber_for_outhaul`].
    ///
    /// Camber moves three things, all in the directions thin-aerofoil theory predicts:
    /// a deeper sail reaches a higher peak lift, reaches it at a *lower* angle of
    /// attack (the camber is already doing part of the work), and drags more.
    pub fn coefficients(&self, alpha: f64, camber: f64) -> SailCoeffs {
        // Camber scales three things at once. Values are relative to the shape the
        // profile was tuned at, so a reference-camber sail reproduces it exactly.
        let depth = self.depth_ratio(camber);
        let cl_max = self.cl_max * (0.60 + 0.40 * depth);
        let alpha_opt = self.alpha_opt_for(camber);
        let alpha_stall = self.alpha_stall + ALPHA_SHIFT_PER_DEPTH * (1.0 - depth);
        // Profile drag rises steeply with camber — roughly with its square, since the
        // pressure recovery over a deeper section is that much harder. The earlier
        // linear law moved parasitic drag by ten percent across the whole outhaul
        // range, which is not defensible and left depth free of any real cost.
        let cd0 = self.cd0 * (0.60 + 0.40 * depth * depth);

        // Eased past the wind: the sail flogs. No useful lift, and real flogging drag.
        if alpha <= 0.0 {
            return SailCoeffs {
                cl: 0.0,
                cd: cd0 + 0.30,
                luffing: 1.0,
                stalled: 0.0,
            };
        }

        let luffing = 1.0 - smoothstep(0.0, self.luff_margin, alpha);

        // Attached flow: rise to the peak, then a soft post-peak decay. The decay is
        // gentle because a sail is a cambered membrane with a flexible leech, not a
        // hard aerofoil — it gives way gradually and still carries most of its lift
        // well past the best angle. Make this steep and an over-sheeted sail becomes
        // a brake producing negative drive, which is wrong, and which in a game reads
        // as being punished rather than slowed.
        let past_peak = ((alpha - alpha_opt) / (alpha_stall - alpha_opt)).clamp(0.0, 1.0);
        let cl_attached = if alpha <= alpha_opt {
            cl_max * (std::f64::consts::FRAC_PI_2 * alpha / alpha_opt).sin()
        } else {
            cl_max * (1.0 - 0.25 * past_peak * past_peak)
        };
        let cd_attached = cd0
            + cl_attached * cl_attached / (std::f64::consts::PI * self.aspect_ratio.max(0.5))
            + 0.35 * past_peak * past_peak;

        // Fully separated: a cambered plate presenting its area to the flow. This is
        // what a mainsail squared off on a run is actually doing.
        let cl_plate = self.cn_separated * alpha.sin() * alpha.cos();
        let cd_plate = cd0 + self.cn_separated * alpha.sin() * alpha.sin();

        let s = smoothstep(alpha_stall, alpha_stall + rad(25.0), alpha);
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
    pub fn optimal_chord_angle(&self, awa: f64, camber: f64) -> f64 {
        (awa.abs() - self.alpha_opt_for(camber)).clamp(self.min_chord_angle, self.max_chord_angle)
    }

    /// Camber relative to the shape the coefficients were tuned at.
    #[inline]
    pub fn depth_ratio(&self, camber: f64) -> f64 {
        let _ = self;
        (camber / CAMBER_REFERENCE).clamp(0.4, 1.6)
    }

    /// Best angle of attack at a given camber.
    ///
    /// Deliberately the single definition. The trim reference and the force curve both
    /// need it, and if they ever disagree then the HUD, the T assist, `optimal_sheets`
    /// and the polar all point at a sheet position where the sail does not actually
    /// peak — a silent, hard-to-see wrongness. One function, two callers.
    #[inline]
    pub fn alpha_opt_for(&self, camber: f64) -> f64 {
        self.alpha_opt + ALPHA_SHIFT_PER_DEPTH * (1.0 - self.depth_ratio(camber))
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
        self.min_chord_angle + (self.max_chord_angle - self.min_chord_angle) * s * s
    }

    /// Inverse of [`chord_angle`](Self::chord_angle).
    #[inline]
    pub fn sheet_for_chord_angle(&self, angle: f64) -> f64 {
        let span = (self.max_chord_angle - self.min_chord_angle).max(1e-9);
        ((angle.clamp(self.min_chord_angle, self.max_chord_angle) - self.min_chord_angle) / span)
            .sqrt()
    }

    /// Sheet position a good trimmer would be holding for this apparent wind angle.
    pub fn optimal_sheet(&self, awa: f64, camber: f64) -> f64 {
        self.sheet_for_chord_angle(self.optimal_chord_angle(awa, camber))
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
    /// Maximum draft as a fraction of chord, as trimmed.
    pub camber: f64,
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
/// * `camber` — maximum draft as a fraction of chord; see [`camber_for_outhaul`].
#[allow(clippy::too_many_arguments)]
pub fn sail_forces(
    profile: &SailProfile,
    awa: f64,
    aws: f64,
    sheet: f64,
    area: f64,
    heel: f64,
    blanketing: f64,
    camber: f64,
) -> SailForces {
    let beta = awa.abs();
    let chord_angle = profile.chord_angle(sheet);
    let alpha = beta - chord_angle;
    let coeffs = profile.coefficients(alpha, camber);

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
        camber,
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
