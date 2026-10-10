//! Distributed buoyancy.
//!
//! Heave and pitch are not modelled with a spring and a lever; they fall out of
//! sampling the hull at stations along its length and asking each one how deep it is
//! in the *local* water surface. In a seaway those stations see different wave heights
//! at the same instant, and that difference is the pitching moment — no separate
//! wave-excitation term required.
//!
//! Deliberate hybrid: roll still uses the calibrated GZ curve in `hull.rs` rather than
//! coming from these stations. Replacing it would throw away a matched righting-moment
//! curve and the whole validated polar with it, to re-derive behaviour that is already
//! correct. What the waves add to roll is the one thing the GZ curve cannot know — the
//! surface it is standing on is tilted — and that enters as an offset to the heel the
//! curve is evaluated at.
//!
//! The section shape is crude and worth naming: the canoe body is treated as having a
//! level bottom with the stations simply narrowing towards the ends, so the beam
//! distribution carries all of the hull form and the rocker carries none.

use crate::units::{GRAVITY, RHO_SEA};

/// How many stations to integrate over. Nine is plenty for heave and pitch at a
/// hundredth of a second; the cost is nine wave-field samples per substep.
pub const STATIONS: usize = 9;

/// Longitudinal hull description for hydrostatics.
#[derive(Clone, Copy, Debug)]
pub struct HullForm {
    pub lwl: f64,
    /// Beam at the waterline amidships, metres.
    pub waterline_beam: f64,
    /// Depth of the canoe body below the design waterline, excluding the keel.
    pub canoe_draft: f64,
    /// Freeboard above the design waterline, metres. Sets how much reserve buoyancy
    /// there is before the deck goes under.
    pub freeboard: f64,
    /// Sectional area coefficient: submerged area over beam times draft. Calibrated so
    /// the boat floats at its design waterline, which is asserted by a test rather
    /// than assumed.
    pub section_coefficient: f64,
}

impl HullForm {
    /// Station positions, metres from the centre of gravity, bow positive.
    pub fn station_x(&self, i: usize) -> f64 {
        let f = (i as f64 + 0.5) / STATIONS as f64;
        -self.lwl * 0.5 + f * self.lwl
    }

    /// Length each station represents.
    pub fn station_length(&self) -> f64 {
        self.lwl / STATIONS as f64
    }

    /// Waterline beam at a station. Fills out quickly from the bow, holds through
    /// midships, narrows a little into the transom.
    pub fn beam_at(&self, x: f64) -> f64 {
        let f = (x / (self.lwl * 0.5)).clamp(-1.0, 1.0);
        // Bow is -x. The exponent is set by the waterplane coefficient it produces:
        // 1.30 gives Cwp = 0.711, which is where a cruiser of this size actually sits.
        // Tuning it by eye produced 0.83 — a hull far too full in the ends, which
        // showed up as heave and pitch periods a third shorter than they should be.
        let shape = (1.0 - f.abs().powi(4)).max(0.0);
        // Asymmetric: a transom is wider than a stem.
        let aft_bias = if f > 0.0 { 1.0 } else { 0.86 };
        self.waterline_beam * shape.powf(1.30) * aft_bias
    }

    /// Submerged sectional area at a local immersion depth.
    ///
    /// Below the canoe body's depth the section is a wedge scaled by the sectional
    /// coefficient; above it the sides are treated as vertical, which is what gives a
    /// hull its reserve buoyancy and what stops it diving through a wave.
    pub fn section_area(&self, x: f64, immersion: f64) -> f64 {
        let beam = self.beam_at(x);
        if immersion <= 0.0 {
            return 0.0;
        }
        let d = immersion.min(self.canoe_draft);
        let wedge = beam * d * self.section_coefficient;
        let above = (immersion - self.canoe_draft).clamp(0.0, self.freeboard);
        wedge + beam * above
    }

    /// Displaced volume at a uniform immersion, cubic metres. Used to calibrate the
    /// sectional coefficient against the boat's actual displacement.
    pub fn volume_at(&self, immersion: f64) -> f64 {
        let dx = self.station_length();
        (0..STATIONS)
            .map(|i| self.section_area(self.station_x(i), immersion) * dx)
            .sum()
    }

    /// Waterplane area at the design waterline. Sets the heave stiffness.
    pub fn waterplane_area(&self) -> f64 {
        let dx = self.station_length();
        (0..STATIONS)
            .map(|i| self.beam_at(self.station_x(i)) * dx)
            .sum()
    }

    /// Longitudinal centre of buoyancy at the design draft, metres from the
    /// waterline's midpoint.
    ///
    /// Not zero, because the stations are not symmetric fore and aft — a transom is
    /// wider than a stem. A real hull is designed so the centre of gravity sits over
    /// the centre of buoyancy and the boat floats level, so every moment here is taken
    /// about this point rather than about the geometric middle. Without it the boat
    /// floats permanently bow-up.
    pub fn centre_of_buoyancy(&self) -> f64 {
        let mut moment = 0.0;
        let mut area = 0.0;
        for i in 0..STATIONS {
            let x = self.station_x(i);
            let a = self.section_area(x, self.canoe_draft);
            moment += a * x;
            area += a;
        }
        if area <= 0.0 {
            0.0
        } else {
            moment / area
        }
    }

    /// Second moment of the waterplane about the transverse axis through the centre of
    /// buoyancy. Sets pitch stiffness, and it is why pitch is so much stiffer than
    /// heave: the same buoyancy acting on a long lever arm.
    pub fn longitudinal_moment(&self) -> f64 {
        let dx = self.station_length();
        let lcb = self.centre_of_buoyancy();
        (0..STATIONS)
            .map(|i| {
                let x = self.station_x(i);
                let arm = x - lcb;
                self.beam_at(x) * arm * arm * dx
            })
            .sum()
    }
}

/// What the stations add up to.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hydrostatics {
    /// Upward force, newtons.
    pub lift: f64,
    /// Bow-up moment, newton metres.
    pub pitch_moment: f64,
    /// Mean immersion across the stations, for telemetry and for working out whether
    /// the boat is burying its bow.
    pub mean_immersion: f64,
    /// Deepest station immersion. Past the freeboard the deck is under.
    pub deepest: f64,
}

/// Integrate buoyancy over the stations.
///
/// `surface_at` returns the water elevation at a station's longitudinal offset, so the
/// caller owns the geometry of where the station actually is in the world — which
/// keeps this function free of headings and wave fields.
pub fn integrate<F>(form: &HullForm, heave: f64, pitch: f64, mut surface_at: F) -> Hydrostatics
where
    F: FnMut(f64) -> f64,
{
    let dx = form.station_length();
    let mut lift = 0.0;
    let mut pitch_moment = 0.0;
    let mut immersion_sum = 0.0;
    let mut deepest = 0.0_f64;

    let lcb = form.centre_of_buoyancy();
    for i in 0..STATIONS {
        let x = form.station_x(i);
        let arm = x - lcb;
        let surface = surface_at(x);
        // Pitch lifts the bow, so a forward station rides higher. Rotation is about
        // the centre of buoyancy, which is where the mass is assumed to balance.
        let bottom = heave + arm * pitch.sin() - form.canoe_draft;
        let immersion = (surface - bottom).max(0.0);
        let force = RHO_SEA * GRAVITY * form.section_area(x, immersion) * dx;

        lift += force;
        // A force up at a forward station pitches the bow up.
        pitch_moment += arm * force;
        immersion_sum += immersion;
        deepest = deepest.max(immersion);
    }

    Hydrostatics {
        lift,
        pitch_moment,
        mean_immersion: immersion_sum / STATIONS as f64,
        deepest,
    }
}
