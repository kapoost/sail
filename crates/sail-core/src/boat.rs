//! Boat configuration.
//!
//! One real boat, modelled properly, beats five boats modelled vaguely. The reference
//! hull is a production charter sloop of about 34 feet — the Bavaria / Jeanneau /
//! Dehler class — because that is the boat most sailors have actually steered and can
//! therefore judge.

use crate::hull::{wetted_surface_estimate, Foil};
use crate::hydrostatics::HullForm;
use crate::sail::SailProfile;
use crate::units::rad;

/// Geometry and aerodynamics of one sail as rigged on this boat.
#[derive(Clone, Copy, Debug)]
pub struct RiggedSail {
    pub profile: SailProfile,
    /// Full area, m^2.
    pub area: f64,
    /// Centre of effort above the waterline, m.
    pub z_ce: f64,
    /// Centre of effort fore-and-aft relative to the centre of gravity, m, bow positive.
    pub x_ce: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct BoatConfig {
    pub name: &'static str,

    // --- Hull geometry and mass ---
    pub loa: f64,
    pub lwl: f64,
    pub beam: f64,
    pub draft: f64,
    /// Displacement, kg.
    pub displacement: f64,
    pub wetted_surface: f64,

    /// Longitudinal form used for distributed buoyancy. Heave and pitch come out of
    /// sampling this against the local water surface, which is also what makes waves
    /// excite the motion without a separate excitation term.
    pub form: HullForm,

    // --- Resistance ---
    /// Scales the residuary-resistance curve. The one number that sets how fast the
    /// boat is; calibrated against known performance rather than derived.
    pub residuary_coefficient: f64,
    /// Effective span for induced drag, m. Less than draft because of the free surface.
    pub induced_span: f64,
    /// Form resistance from sailing heeled, quadratic in heel.
    pub heel_drag_factor: f64,
    /// Heel past which the leeward rail starts going under and the price steepens.
    pub rail_down_heel: f64,
    /// How much steeper. Calibrated so the model's upwind VMG peaks near twenty
    /// degrees of heel, which is where a real cruiser's does — see
    /// `examples/outhaul.rs`, which is the measurement that caught it being wrong.
    pub rail_down_factor: f64,

    // --- Lateral plane and steering ---
    pub lateral_plane: Foil,
    /// Fore-and-aft position of the centre of lateral resistance, m, bow positive.
    pub x_clr: f64,
    pub rudder: Foil,
    /// Rudder position, m, bow positive — so a negative number.
    pub x_rudder: f64,
    pub max_rudder_angle: f64,

    // --- Stability ---
    /// Transverse metacentric height, m.
    pub gm: f64,
    /// Heel angle at which the righting arm peaks, radians.
    pub gz_peak_angle: f64,
    /// Depth of the centre of lateral resistance below the waterline, m. Together with
    /// a sail's `z_ce` this is the heeling arm.
    pub z_clr: f64,

    // --- Inertia, including added mass ---
    pub surge_added_mass: f64,
    /// Large for a keelboat: roughly the displacement again. This is why a boat does
    /// not simply slide sideways when the sails load up.
    pub sway_added_mass: f64,
    pub roll_inertia: f64,
    pub yaw_inertia: f64,
    /// Heave mass including added mass, which for a hull at these frequencies is
    /// comparable to the displacement again.
    pub heave_mass: f64,
    /// Pitch inertia including added inertia.
    pub pitch_inertia: f64,
    /// Fraction of critical damping in heave. Yachts are well damped in heave and
    /// pitch — the energy goes straight into radiated waves.
    pub heave_damping: f64,
    pub pitch_damping: f64,
    /// What fraction of the power dissipated by heave and pitch damping is charged
    /// back against forward motion as added resistance in waves.
    ///
    /// The argument is energy, not a fitted curve: a pitching hull radiates waves, and
    /// that power has to come from somewhere. Some of it comes from the incident sea
    /// and some from the boat's own propulsion, so charging all of it would overstate
    /// the loss — hence a fraction. The attraction of doing it this way is that it
    /// needs no sea-state formula and gets the heading dependence free: a head sea
    /// makes the boat pitch and therefore slows it, a following sea barely does either.
    pub wave_drag_fraction: f64,
    /// How much of the wave's surface slope the boat actually takes up. Less than one,
    /// because the keel is deep and the wave's pressure field decays with depth.
    pub effective_wave_slope: f64,
    /// Fraction of critical damping in roll, at rest.
    pub roll_damping: f64,
    /// Mean depth of the lateral plane below the roll axis, metres. Sets the lever for
    /// the keel's roll damping, which is the dominant term once the boat is moving.
    pub roll_damping_lever: f64,
    /// Residual bare-hull yaw damping, N*m per (rad/s * m/s).
    pub yaw_damping: f64,
    /// Strength of the bow-to-windward moment from the asymmetric heeled waterplane.
    /// Together with the heel-shifted centre of effort this produces weather helm.
    pub heel_yaw_coefficient: f64,

    // --- Rig ---
    pub main: RiggedSail,
    pub genoa: RiggedSail,
    /// Largest mainsail reduction available, as a fraction of its area. The headsail
    /// has no equivalent limit: a roller genoa rolls all the way away.
    pub max_reef: f64,

    // --- Auxiliary engine ---
    pub max_thrust: f64,
    /// Thrust acts below and aft of the centre of gravity; the offset gives prop walk.
    pub prop_walk: f64,
}

impl BoatConfig {
    /// Production charter sloop, about 34 feet.
    pub fn charter_sloop_34() -> Self {
        let displacement = 5500.0;
        let lwl = 9.20;
        let beam = 3.45;
        let loa = 10.40;

        let main_area = 32.0;
        let main_luff = 12.5;
        let genoa_area = 32.0;
        let genoa_luff = 13.2;

        // Radii of gyration from the usual rules of thumb, then added inertia.
        let roll_radius = 0.35 * beam;
        let yaw_radius = 0.25 * loa;

        Self {
            name: "charter sloop 34",

            loa,
            lwl,
            beam,
            draft: 1.90,
            displacement,
            wetted_surface: wetted_surface_estimate(displacement, lwl),

            form: HullForm {
                lwl,
                waterline_beam: 3.30,
                canoe_draft: 0.55,
                freeboard: 0.95,
                // Calibrated so the boat floats at its design waterline. Asserted by
                // `the_boat_floats_at_its_design_waterline` rather than assumed — it
                // is low for a sectional area coefficient because the stations here do
                // not narrow with depth, so this number is carrying the V-shape too.
                section_coefficient: 0.452,
            },

            residuary_coefficient: 1.15,
            induced_span: 0.80 * 1.90,
            heel_drag_factor: 0.57,
            rail_down_heel: rad(20.0),
            rail_down_factor: 3.0,

            lateral_plane: Foil::lateral_plane(4.5, 1.41),
            x_clr: 0.10,
            rudder: Foil::rudder(0.55, 2.20),
            x_rudder: -4.30,
            max_rudder_angle: rad(40.0),

            gm: 1.15,
            gz_peak_angle: rad(60.0),
            z_clr: 0.85,

            surge_added_mass: 0.05 * displacement,
            sway_added_mass: 0.85 * displacement,
            roll_inertia: 1.5 * displacement * roll_radius * roll_radius,
            yaw_inertia: 1.3 * displacement * yaw_radius * yaw_radius,
            heave_mass: 2.8 * displacement,
            pitch_inertia: 2.5 * displacement * yaw_radius * yaw_radius,
            heave_damping: 0.26,
            pitch_damping: 0.32,
            effective_wave_slope: 0.65,
            wave_drag_fraction: 0.5,
            roll_damping: 0.14,
            roll_damping_lever: 1.20,
            yaw_damping: 4200.0,
            heel_yaw_coefficient: 0.060,

            main: RiggedSail {
                profile: SailProfile::mainsail(main_area, main_luff),
                area: main_area,
                z_ce: 6.20,
                x_ce: -0.30,
            },
            genoa: RiggedSail {
                profile: SailProfile::genoa(genoa_area, genoa_luff),
                area: genoa_area,
                z_ce: 4.60,
                x_ce: 1.90,
            },
            max_reef: 0.55,

            max_thrust: 2200.0,
            prop_walk: 0.08,
        }
    }

    /// Mainsail area and centre of effort after reefing. Rolling sail away lowers the
    /// centre of effort as well as shrinking it, which is why a reefed boat stands up
    /// rather than merely slowing down.
    pub fn reefed_main(&self, reef: f64) -> (f64, f64) {
        let r = reef.clamp(0.0, 1.0) * self.max_reef;
        (
            self.main.area * (1.0 - r),
            self.main.z_ce * (1.0 - 0.55 * r),
        )
    }

    /// Headsail area and centre of effort after furling. A rolled genoa loses area
    /// from its foot and leech, so its centre of effort rises slightly even as it
    /// shrinks. At `furl` of 1 it is gone entirely, which is a real and often correct
    /// way to sail: main alone is how you shorten down in a blow, and how you isolate
    /// the mainsail when you want to learn what it is doing.
    pub fn furled_genoa(&self, furl: f64) -> (f64, f64) {
        let f = furl.clamp(0.0, 1.0);
        (
            self.genoa.area * (1.0 - f),
            self.genoa.z_ce * (1.0 - 0.25 * f),
        )
    }

    /// Total upwind sail area, unreefed.
    pub fn sail_area(&self) -> f64 {
        self.main.area + self.genoa.area
    }

    /// Classic hull-speed ceiling for this waterline, m/s.
    pub fn hull_speed(&self) -> f64 {
        crate::units::hull_speed(self.lwl)
    }
}

impl Default for BoatConfig {
    fn default() -> Self {
        Self::charter_sloop_34()
    }
}
