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
    /// Effective drag area of everything above water with no sail set, square metres.
    ///
    /// Windage, and it was missing entirely — with the main handed and the genoa rolled
    /// away the model had *no* aerodynamic force at all, so a boat under bare poles in
    /// twenty-five knots simply coasted, for a very long time, in whatever direction she
    /// had been going. She could even make ground to windward, which nothing without
    /// sails has ever done.
    ///
    /// A drag coefficient times an area, lumped: mast and boom, the standing and running
    /// rigging, a rolled headsail on the stay, and the topsides. On a boat this size the
    /// usual figure is one to two square metres head-on and nearer three beam-on, where
    /// the whole side of the hull and the furled sail are presented. That is a hundred
    /// and fifty newtons in twenty-five knots, which is what drives her off to leeward at
    /// a knot or two with everything handed — and what lays her over when she is lying
    /// a-hull in a gale, since it acts at the height of the rig.
    pub windage_area: f64,
    /// And where it acts, fore and aft, metres — bow positive.
    ///
    /// A force with no moment is a force applied at the origin, which is an accident
    /// rather than a decision, and that is what this was until somebody asked where the
    /// push from the boom goes. With no sail set it is the *only* yaw moment on her, so
    /// it decides how a bare-poled boat lies, which is a real question with a real
    /// answer: forward of the lateral resistance and she bears away and runs off; aft of
    /// it and she weathercocks head to wind and sits there in irons, which is what a
    /// mizzen is for and what a sloop does not do.
    ///
    /// Added up from the pieces rather than chosen, frontal area times lever:
    ///
    /// | piece                      | area   | at     |
    /// |----------------------------|--------|--------|
    /// | mast, 17.8 m by 0.18       | 3.2 m² | +1.8 m |
    /// | boom and the main on it    | 1.9 m² | −0.8 m |
    /// | rolled genoa on the stay   | 3.2 m² | +3.5 m |
    /// | topsides                   | 10 m²  |  0.0 m |
    ///
    /// which comes to about three quarters of a metre forward — the topsides dominating
    /// the area and sitting amidships is what keeps the answer modest. So she bears away
    /// under bare poles and lies stern-to, which is the commoner of the two things a
    /// sloop does and the safer one.
    ///
    /// The *moment* is small against the sails: sixty-odd newton-metres in twenty knots
    /// of apparent wind, against the two to four thousand the rig makes. This will not
    /// be the reason anything gripes.
    pub windage_x: f64,

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
    /// Mean depth of the lateral plane below the surface, metres. Used to sample the
    /// orbital flow the keel actually sees, which is much less than the hull's.
    pub lateral_plane_depth: f64,
    /// Mean depth of the rudder below the surface, metres.
    pub rudder_depth: f64,
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
    /// The flat reaching sail, set flying from the sprit.
    pub code_zero: RiggedSail,
    /// The running sail.
    pub gennaker: RiggedSail,
    /// The inner headsail of a double head rig.
    pub staysail: RiggedSail,
    /// Mass of the boom and the cloth that hangs on it, kg.
    ///
    /// Not a detail. A boom is the one part of the rig with enough mass, far enough out
    /// on a lever, to have opinions of its own in a seaway — and the sheet that holds it
    /// is a rope, which can pull and cannot push.
    pub boom_mass: f64,
    /// How far out along the boom that mass acts, metres.
    pub boom_cg: f64,

    /// Chord angle at which the headsail starts to lie on the standing rigging.
    ///
    /// Derived from where the cap shroud stands at the sail's own mid-height, not
    /// chosen: see [`crate::sail::rig_interference`].
    pub headsail_bears_at: f64,
    /// Largest mainsail reduction available, as a fraction of its area. The headsail
    /// has no equivalent limit: a roller genoa rolls all the way away.
    pub max_reef: f64,
}

/// The rig of the boat the game is sailed in, as numbers rather than as a drawing.
///
/// Every one of these is either measured or derived from something measured, and they
/// live here — in the crate that knows nothing about any renderer — because the drawing
/// and the physics are descriptions of the *same rig* and this file has been caught
/// three times letting them drift apart. The mast was 1.6 m shorter in the picture than
/// the mainsail's luff needed, the boom half a metre longer than its area allowed, and
/// the forestay two metres higher than the genoa could reach.
///
/// A sail is measured by its luff and its area. Everything else about where the spars
/// are follows, and is written here as the arithmetic that makes it follow.
pub mod packet {
    /// What the sailmaker measured.
    pub const MAIN_AREA: f64 = 46.0;
    pub const MAIN_LUFF: f64 = 15.2;
    pub const GENOA_AREA: f64 = 26.0;
    pub const GENOA_LUFF: f64 = 12.4;

    /// Where the mast is stepped, metres abaft the centre of gravity.
    pub const MAST_FOOT_Z: f64 = -1.8;
    /// Gooseneck height above the water. A choice — head clearance over the cockpit —
    /// and the only free number in this module.
    pub const BOOM_HEIGHT: f64 = 2.62;

    /// The masthead is the gooseneck plus the luff. There is nowhere else it can be.
    pub const MAST_HEAD: f64 = BOOM_HEIGHT + MAIN_LUFF;
    /// And the forestay lands where the genoa's luff reaches from its tack, which on
    /// this boat is four metres below the masthead: a fractional rig, as the sail plan
    /// has been saying all along — a 46 square metre main against a 26 metre jib.
    pub const GENOA_TACK_HEIGHT: f64 = 1.12;
    pub const FORESTAY_HEAD: f64 = GENOA_TACK_HEIGHT + GENOA_LUFF;

    /// The boom is the mainsail's foot, so it is the area over the luff, with the roach
    /// a modern main carries taken off the top.
    pub const CLOTH_OVER_TRIANGLE: f64 = 1.15;
    pub const BOOM_LENGTH: f64 = 2.0 * MAIN_AREA / CLOTH_OVER_TRIANGLE / MAIN_LUFF;
}

/// How the rest of the wardrobe follows from the two sails that are measured.
///
/// Three boats in this file carry the same five sails, and until now each wrote all
/// five out longhand. Which cost exactly what duplication always costs: two scripted
/// edits in a row landed on a boat nobody sails, and the Code 0 the game was played
/// with stayed at 39 square metres while every commit said 55. One function, one set of
/// ratios, and that cannot happen again.
///
/// The ratios are to the genoa, because the genoa is the sail a rig is measured by.
fn wardrobe(genoa: &RiggedSail, genoa_luff: f64) -> (RiggedSail, RiggedSail, RiggedSail) {
    let area = genoa.area;
    let code_zero = RiggedSail {
        // Sixty square metres against the genoa's twenty-six, which is not a typo: a
        // Code 0 is measured as a spinnaker and cut like one. Sized on the only evidence
        // that settles a sail's size, which is somebody who owns one — tacked on the
        // sprit it sheets back nearly to the cockpit, which is a foot of about 7.2 m and
        // an LP of 148 percent of the base the sprit gives it.
        profile: SailProfile::code_zero(area * 2.3, genoa_luff * 1.05),
        area: area * 2.3,
        z_ce: genoa.z_ce * 1.22,
        x_ce: genoa.x_ce * 1.29,
    };
    let gennaker = RiggedSail {
        // Twice the mainsail, which is what a running sail is.
        profile: SailProfile::gennaker(area * 3.4, genoa_luff * 1.02),
        area: area * 3.4,
        z_ce: genoa.z_ce * 1.30,
        x_ce: genoa.x_ce * 1.47,
    };
    let staysail = RiggedSail {
        profile: SailProfile::staysail(area * 0.54, genoa_luff * 0.62),
        area: area * 0.54,
        z_ce: genoa.z_ce * 0.78,
        x_ce: genoa.x_ce * 0.55,
    };
    (code_zero, gennaker, staysail)
}

/// Where on the reef control the mainsail stops being set at all.
pub const DROPPED: f64 = 0.98;

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

        let genoa = RiggedSail {
            profile: SailProfile::genoa(genoa_area, genoa_luff),
            area: genoa_area,
            z_ce: 4.60,
            x_ce: 1.90,
        };
        let (code_zero, gennaker, staysail) = wardrobe(&genoa, genoa_luff);

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
            windage_area: 1.6,
            windage_x: 0.85,

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
            lateral_plane_depth: 1.15,
            rudder_depth: 0.95,
            yaw_damping: 4200.0,
            heel_yaw_coefficient: 0.060,

            main: RiggedSail {
                profile: SailProfile::mainsail(main_area, main_luff),
                area: main_area,
                z_ce: 6.20,
                x_ce: -0.30,
            },
            genoa,
            code_zero,
            gennaker,
            staysail,
            boom_mass: 58.0,
            boom_cg: 2.4,
            headsail_bears_at: rad(19.0),
            max_reef: 0.55,
        }
    }

    /// The archipelago packet, about thirty-six feet.
    ///
    /// Not a model of a particular boat, which is the point. The other two configs
    /// describe real types and inherit whatever those types are bad at; this one is
    /// *designed*, for a game about carrying parcels between islands. The design brief
    /// is a sailing brief rather than a gameplay one, because the two turn out to
    /// agree, and where they agree is the only place a boat can be tuned for fun
    /// without lying about physics:
    ///
    /// * **Short legs, constant course changes.** Every point of sail gets used within
    ///   the hour, so a polar with a hole in it — the usual dead run, the usual wide
    ///   no-go zone — is felt immediately. A hull wants to be easily driven across the
    ///   whole range rather than optimised at one angle.
    /// * **Channels and headlands.** Pointing high is worth more than reaching speed,
    ///   because every degree is a tack not made in a narrow place. That asks for a
    ///   deep efficient keel, which is a real fitting with a real cost in draught.
    /// * **Single handed, with cargo.** The rig is fractional: a big main that does the
    ///   work and a small jib that looks after itself through a tack. Reefing the main
    ///   is then the whole of shortening down, which is one job rather than two.
    /// * **Light air happens.** An archipelago is full of wind shadows under islands.
    ///   Sail area against wetted surface is what matters there, and a narrow hull has
    ///   little wetted surface to carry.
    ///
    /// The proportions come from the hull the boat is drawn with — a Shetland
    /// fourareen scanned by the Scottish Maritime Museum — scaled to length. Its
    /// length-to-beam of 3.4 is the Norse inheritance, and it is also exactly what
    /// makes a hull easily driven: less wave-making for the same displacement. The
    /// shape the game wanted and the shape that arrived were the same shape, so the
    /// residuary coefficient is lower than the charter boat's because the hull really
    /// is finer, not because a slow boat was unwelcome.
    ///
    /// Everything here is a starting point for `examples/delivery.rs`, which measures
    /// what it actually produces.
    pub fn archipelago_packet() -> Self {
        // Light. A displacement-to-length ratio of 116 against the charter boat's 240:
        // this is the single number that decides whether a boat feels alive, because
        // everything else — the sail she can carry, the speed she will make for it,
        // how quickly she answers — is measured against her own weight.
        //
        // It is also the honest way to make her fast. There is no multiplier anywhere
        // in this model that could be turned up; what there is instead is a set of
        // real decisions a designer makes, and every one of them has a cost. She is
        // light, so she is wet and she pounds. She is over-canvassed, so she needs
        // reefing early and often. Her keel is deep, so there are anchorages she
        // cannot get into. That is the trade, and the trade is what makes it a game.
        let displacement = 3600.0;
        // Hydrostatics read off the scan rather than guessed: the hull mesh was
        // integrated station by station and the waterline solved for where the
        // displaced volume equals the displacement. It put her 0.4 m finer in the
        // waterline and 0.4 m deeper in the canoe body than the first estimates, which
        // is the difference between a hull that floats where it is drawn and one that
        // does not.
        let lwl = 9.50;
        // Straight off the scan's vertices: greatest beam 3.40 m once scaled to
        // length. An earlier 3.21 came from the corners of the bounding box, which is
        // not the same thing and is always too small for a hull with any curve in it.
        let beam = 3.40;
        let loa = 11.00;
        // Deep, with a bulb. This is what buys the righting moment a light hull does
        // not get from its own weight, and it is the reason she can stand up to that
        // rig at all. The cost is anchorages: a two-metre draught is a real
        // constraint in a place where the banks shelve for four hundred metres.
        let draft = 2.25;

        // Fractional: the main is the engine, the jib is a handkerchief that tacks
        // itself. Sail area to displacement is now about 31 — a racing number, and
        // deliberately so. Under-canvassed boats are dull in the light airs an
        // archipelago is full of, and the cost of a big rig is paid honestly here,
        // in reefing.
        // From the module that derives the spars from them, so the sail plan and the rig
        // cannot be edited apart.
        let main_area = packet::MAIN_AREA;
        let main_luff = packet::MAIN_LUFF;
        // How far the boom can be eased before it lies on the cap shroud.
        //
        // Not a round number and not a guess: worked out from this boat's own rig.
        // The shroud runs from the masthead at 16.0 m to the rail at 0.45 of the beam,
        // so at boom height it stands 1.33 m outboard and 0.35 m abaft the mast —
        // which from the gooseneck is a bearing of 75 degrees at 1.38 m along the
        // boom. Past that the spar is through the wire.
        //
        // It matters because it is the reason you cannot square a main off on a run,
        // the reason a preventer exists, and the reason a boat of this rig sails deep
        // angles worse than the polar alone suggests. Ninety degrees — what this was —
        // is a boom that has swung through its own standing rigging.
        let boom_against_shrouds = rad(75.0);
        let genoa_area = packet::GENOA_AREA;
        let genoa_luff = packet::GENOA_LUFF;

        let roll_radius = 0.35 * beam;
        let yaw_radius = 0.25 * loa;

        let genoa = RiggedSail {
            profile: SailProfile::genoa(genoa_area, genoa_luff),
            area: genoa_area,
            z_ce: 4.40,
            x_ce: 1.95,
        };
        let (code_zero, gennaker, staysail) = wardrobe(&genoa, genoa_luff);

        Self {
            name: "archipelago packet 36",

            loa,
            lwl,
            beam,
            draft,
            displacement,
            wetted_surface: wetted_surface_estimate(displacement, lwl),

            form: HullForm {
                lwl,
                waterline_beam: 2.60,
                canoe_draft: 1.00,
                // The rail amidships, 0.59 m up.
                //
                // Measured three times and wrong twice. The bounding box said 1.45 m,
                // which is the stem head. Reading the sheer height separately from the
                // beam said 0.79. Taking the gunwale as a single point on the hull —
                // the topmost vertex of each station, its height and its offset
                // together — says 0.59, and that is the one that is a place on the
                // boat rather than a statistic about her.
                freeboard: 0.59,
                // Calibrated so she floats at the waterline the scan says she floats
                // at, and asserted by `the_boat_floats_at_its_design_waterline`. Far
                // finer than the charter boat's 0.452, because the hull is: a deep,
                // narrow canoe body puts much less area in each section than a beamy
                // shallow one of the same beam and draught.
                section_coefficient: 0.2000,
            },

            // Finer again, because a light hull of this length makes far less of a
            // wave than a heavy one. This is also what softens the hull-speed wall:
            // a displacement boat is stopped by its own wave system and a light one
            // climbs over it, which is the difference between a boat that tops out at
            // seven and a half knots and one that keeps going.
            residuary_coefficient: 0.74,
            induced_span: 0.80 * draft,
            heel_drag_factor: 0.52,
            rail_down_heel: rad(22.0),
            rail_down_factor: 3.0,

            // A deep high-aspect fin: less area than the charter boat's, working
            // harder, which is what lets her point.
            lateral_plane: Foil::lateral_plane(4.1, 1.85),
            x_clr: 0.10,
            rudder: Foil::rudder(0.58, 2.40),
            x_rudder: -4.60,
            max_rudder_angle: rad(40.0),

            // Narrow hulls have little form stability and light ones have little of
            // anything else, so the bulb has to do all of it.
            //
            // Pulled back from 1.62, which was measured and rejected: at that stiffness
            // she beat at thirty degrees to the true wind in eighteen knots, which is
            // a number no boat with soft sails makes and which would have given the
            // whole thing away. A fast boat is allowed to be fast; it is not allowed to
            // be impossible.
            gm: 1.42,
            gz_peak_angle: rad(65.0),
            z_clr: 0.90,
            windage_area: 1.6,
            windage_x: 0.85,

            surge_added_mass: 0.05 * displacement,
            sway_added_mass: 0.85 * displacement,
            roll_inertia: 1.5 * displacement * roll_radius * roll_radius,
            yaw_inertia: 1.3 * displacement * yaw_radius * yaw_radius,
            heave_mass: 2.8 * displacement,
            pitch_inertia: 2.5 * displacement * yaw_radius * yaw_radius,
            heave_damping: 0.26,
            pitch_damping: 0.32,
            effective_wave_slope: 0.62,
            wave_drag_fraction: 0.44,
            roll_damping: 0.14,
            roll_damping_lever: 1.20,
            lateral_plane_depth: 1.20,
            rudder_depth: 1.00,
            yaw_damping: 4000.0,
            heel_yaw_coefficient: 0.058,

            main: RiggedSail {
                profile: SailProfile {
                    max_chord_angle: boom_against_shrouds,
                    ..SailProfile::mainsail(main_area, main_luff)
                },
                area: main_area,
                z_ce: 6.60,
                x_ce: -0.35,
            },
            genoa,
            code_zero,
            gennaker,
            staysail,
            boom_mass: 58.0,
            boom_cg: 2.4,
            headsail_bears_at: rad(17.0),
            max_reef: 0.68,
        }
    }

    /// Mainsail area and centre of effort after reefing. Rolling sail away lowers the
    /// centre of effort as well as shrinking it, which is why a reefed boat stands up
    /// rather than merely slowing down.
    pub fn reefed_main(&self, reef: f64) -> (f64, f64) {
        // The last position on the halyard is not a third reef. It is the sail coming
        // down: full, first, second, handed. A boat this size carries two slabs and then
        // you take it off her, and "sailing under headsail alone" is a real way to get
        // home rather than an edge case.
        if reef >= DROPPED {
            return (0.0, self.main.z_ce * 0.3);
        }
        let r = reef.clamp(0.0, 1.0) * self.max_reef;
        (
            self.main.area * (1.0 - r),
            self.main.z_ce * (1.0 - 0.55 * r),
        )
    }

    /// The mainsail split in two for twist: (area, centre of effort height) for the
    /// lower and upper halves.
    ///
    /// Two sections is the smallest split that makes twist mean anything, and twist is
    /// what the vang is for. The lower carries more area because a mainsail is a
    /// triangle; the upper sits much higher, which is why spilling it is such an
    /// effective way to shed heeling moment without reefing.
    pub fn main_sections(&self, reef: f64) -> ((f64, f64), (f64, f64)) {
        let (area, z_ce) = self.reefed_main(reef);
        ((area * 0.62, z_ce * 0.74), (area * 0.38, z_ce * 1.42))
    }

    /// Whichever sail is set forward of the mast.
    pub fn headsail(&self, which: crate::state::Headsail) -> &RiggedSail {
        match which {
            crate::state::Headsail::Genoa => &self.genoa,
            crate::state::Headsail::Code0 => &self.code_zero,
            crate::state::Headsail::Gennaker => &self.gennaker,
        }
    }

    /// The headsail split the same way, so the lead car's twist has somewhere to act.
    pub fn genoa_sections(&self, furl: f64) -> ((f64, f64), (f64, f64)) {
        self.headsail_sections(crate::state::Headsail::Genoa, furl)
    }

    /// The sections of whichever headsail is up.
    ///
    /// A flying sail is split lower than a genoa. Its area is carried in the middle of
    /// a deep curve rather than up a straight luff, so more of it sits in the lower
    /// band — which also puts its centre of effort where it belongs and keeps a
    /// gennaker from heeling the boat like a genoa twice its size would.
    pub fn headsail_sections(
        &self,
        which: crate::state::Headsail,
        furl: f64,
    ) -> ((f64, f64), (f64, f64)) {
        let sail = self.headsail(which);
        let f = furl.clamp(0.0, 1.0);
        let area = sail.area * (1.0 - f);
        let z_ce = sail.z_ce * (1.0 - 0.25 * f);
        if which.flying() {
            ((area * 0.66, z_ce * 0.78), (area * 0.34, z_ce * 1.38))
        } else {
            ((area * 0.60, z_ce * 0.72), (area * 0.40, z_ce * 1.44))
        }
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

    /// Yaw moment the rudder can produce per radian of helm per unit of speed
    /// squared, N*m/(rad*(m/s)^2).
    ///
    /// The boat's steering authority, and the thing any autopilot's gains have to be
    /// expressed against. Hard-coding gains tuned on one hull is fine until a second
    /// hull arrives — the fifty-three footer has 8.8 times the yaw inertia but only
    /// 4.5 times the authority, so the same numbers left it half as responsive as the
    /// controller assumed, and it bore away through a full gybe before the pilot could
    /// catch it.
    pub fn yaw_authority(&self) -> f64 {
        0.5 * crate::units::RHO_SEA
            * self.rudder.area
            * self.rudder.lift_slope
            * self.x_rudder.abs()
    }

    /// PID gains for a tiller pilot on this boat, derived rather than tuned.
    ///
    /// Picks a closed-loop yaw frequency and a damping ratio, then solves for the
    /// gains that produce them given this hull's inertia and steering authority. The
    /// reference speed is five knots: real pilots have a rudder-gain knob for exactly
    /// this reason, since authority goes as the square of speed.
    pub fn pilot_gains(&self) -> (f64, f64, f64) {
        const FREQUENCY: f64 = 1.09; // rad/s, about a six-second correction
        const DAMPING: f64 = 0.8;
        let reference = crate::units::kn_to_ms(5.0);
        let authority = self.yaw_authority() * reference * reference;
        let kp = self.yaw_inertia * FREQUENCY * FREQUENCY / authority;
        let kd = 2.0 * DAMPING * self.yaw_inertia * FREQUENCY / authority;
        // Integral action only has to cancel a standing weather helm, so it is slow.
        let ki = kp * 0.4;
        (kp, ki, kd)
    }

    /// Classic hull-speed ceiling for this waterline, m/s.
    pub fn hull_speed(&self) -> f64 {
        crate::units::hull_speed(self.lwl)
    }

    /// Performance cruiser, about 53 feet — the Oyster, Hallberg-Rassy and
    /// Oceanis 51 class.
    ///
    /// Not just a scaled-up 34. The point of a bigger boat is that it is a different
    /// animal in a sea: every natural period lengthens, so a short chop that throws a
    /// thirty-four footer around barely reaches this one. Roll goes from 2.8 s to
    /// 3.3, and against a four-second sea that is the difference between being tossed
    /// and shouldering through.
    pub fn performance_cruiser_53() -> Self {
        let displacement = 20_000.0;
        let lwl = 14.60;
        let beam = 4.90;
        let loa = 16.20;
        let draft = 2.35;

        let main_area = 75.0;
        let main_luff = 17.5;
        let genoa_area = 85.0;
        let genoa_luff = 19.0;

        let roll_radius = 0.35 * beam;
        let yaw_radius = 0.25 * loa;

        let genoa = RiggedSail {
            profile: SailProfile::genoa(genoa_area, genoa_luff),
            area: genoa_area,
            z_ce: 6.40,
            x_ce: 2.90,
        };
        let (code_zero, gennaker, staysail) = wardrobe(&genoa, genoa_luff);

        Self {
            name: "performance cruiser 53",

            loa,
            lwl,
            beam,
            draft,
            displacement,
            wetted_surface: wetted_surface_estimate(displacement, lwl),

            form: HullForm {
                lwl,
                waterline_beam: 4.60,
                canoe_draft: 0.80,
                freeboard: 1.35,
                // Calibrated for this displacement, same as the 34's is for hers.
                section_coefficient: 0.511,
            },

            residuary_coefficient: 1.15,
            induced_span: 0.80 * draft,
            heel_drag_factor: 0.57,
            rail_down_heel: rad(20.0),
            rail_down_factor: 3.0,

            // Sized against sail area, not plucked. The lateral plane has to balance
            // the rig's side force at a few degrees of leeway, and the first attempt
            // gave this boat proportionally *less* keel than the thirty-four footer
            // (0.059 of sail area against 0.070). The result was thirty-five degrees
            // of leeway in a seaway: once she slowed, the keel could no longer hold
            // her, and that is a spiral, not a slow boat.
            lateral_plane: Foil::lateral_plane(12.0, 1.41),
            x_clr: 0.15,
            rudder: Foil::rudder(1.55, 2.20),
            x_rudder: -6.80,
            max_rudder_angle: rad(40.0),

            gm: 1.60,
            gz_peak_angle: rad(60.0),
            z_clr: 1.05,
            windage_area: 1.6,
            windage_x: 0.85,

            surge_added_mass: 0.05 * displacement,
            sway_added_mass: 0.85 * displacement,
            roll_inertia: 1.5 * displacement * roll_radius * roll_radius,
            yaw_inertia: 1.3 * displacement * yaw_radius * yaw_radius,
            heave_mass: 2.8 * displacement,
            pitch_inertia: 2.5 * displacement * yaw_radius * yaw_radius,
            heave_damping: 0.26,
            pitch_damping: 0.32,
            roll_damping: 0.14,
            roll_damping_lever: 1.55,
            lateral_plane_depth: 1.45,
            rudder_depth: 1.20,
            yaw_damping: 11_000.0,
            heel_yaw_coefficient: 0.060,
            effective_wave_slope: 0.65,
            wave_drag_fraction: 0.5,

            main: RiggedSail {
                profile: SailProfile::mainsail(main_area, main_luff),
                area: main_area,
                z_ce: 8.60,
                x_ce: -0.45,
            },
            genoa,
            code_zero,
            gennaker,
            staysail,
            boom_mass: 58.0,
            boom_cg: 2.4,
            headsail_bears_at: rad(19.0),
            max_reef: 0.55,
        }
    }
}

impl Default for BoatConfig {
    fn default() -> Self {
        Self::charter_sloop_34()
    }
}
