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
/// Draft of a Code 0. Flat for a sail set flying, which is what makes it a Code 0
/// rather than a spinnaker: the rating rule draws the line at a mid-girth of 55 percent
/// of the foot, and the shape that gets you there is nearly a genoa's.
pub const CAMBER_CODE_ZERO: f64 = 0.10;
/// Draft of a gennaker, and it is a different kind of sail because of this number.
///
/// Twenty-two percent. Three times a flattened mainsail. A sail with that much shape
/// cannot be flattened, will not point, and generates more force than anything else on
/// the boat once the wind is abaft the beam — and it has to *look* like that, because a
/// gennaker drawn with a genoa's section is the one sail in the wardrobe a sailor can
/// identify as wrong from a photograph.
pub const CAMBER_GENNAKER: f64 = 0.22;

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

/// The same, once the sail is reefed — and it is a different line doing it.
///
/// With a slab in, the outhaul is slack: it is still shackled to the original clew,
/// which is now bunched up in the sailbag of cloth along the boom and pulling on
/// nothing. What holds the foot out is the reef pennant, hauled down through the cheek
/// block at the boom end, and that is the line a crew flattens a reefed main with.
///
/// Same hand, same decision, different rope — which is why this takes the same control
/// rather than adding one. What it adds is the flattening the reef does by itself. The
/// deepest cloth in a mainsail is the lower third; take a slab out of it and what is
/// left is a flatter sail than the one you started with, at the same setting. That is
/// part of why a reefed boat stands up better than the lost area alone explains, and it
/// is worth having in the model rather than discovering as a missing few degrees of
/// heel.
pub fn camber_reefed(camber: f64, reef: f64, max_reef: f64) -> f64 {
    let taken = (reef.clamp(0.0, 1.0) * max_reef).clamp(0.0, 1.0);
    camber - (camber - CAMBER_FLAT) * 0.45 * taken
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
pub fn twist_for_vang(vang: f64, chord_angle: f64) -> f64 {
    let sheet_pull = chord_angle.cos().clamp(0.0, 1.0);
    let leech_tension = vang.clamp(0.0, 1.0).max(sheet_pull);
    MAX_TWIST * (1.0 - leech_tension)
}

/// How much a headsail loses once it is lying on the standing rigging.
///
/// An eased genoa does not swing freely into clear air. Its leech comes up against the
/// cap shroud and, higher, the spreader, and from there on easing the sheet stops
/// opening the sail and starts pressing it into a wire. That is why a genoa has
/// spreader patches sewn on it, and it is a real loss rather than a cosmetic one: the
/// crease trips the flow over the back of the sail where it was doing the most work.
///
/// Where contact starts is geometry, not opinion. On this rig the cap shroud stands
/// 1.01 m outboard at the genoa's mid-height, and the sail's chord there is about
/// 3.5 m from the stay, so the leech reaches the wire at a chord angle of 17 degrees —
/// which is to say on almost any reach, as anyone who has looked at a genoa on a
/// reach already knows. It is not the contact that costs, it is how hard she is
/// pressed into it, so the penalty starts at nothing there and builds over the next
/// forty degrees of easing.
///
/// The size of it is the least certain thing here. Published figures for a genoa hard
/// on the spreaders run to a few per cent of drive; eight per cent of lift and a
/// fifteen per cent rise in drag at full bearing is the upper end of that, chosen so
/// the effect is felt rather than guessed at precisely — and recorded as a number to
/// argue with rather than a shrug.
pub fn rig_interference(chord_angle: f64, bears_at: f64) -> (f64, f64) {
    let t = rig_contact(chord_angle, bears_at);
    (1.0 - 0.08 * t, 1.0 + 0.15 * t)
}

/// How hard the headsail is lying on the standing rigging, 0 to 1.
///
/// A hump: it builds, and then it goes away again, and it does not exist at all until
/// the sail has been eased far enough to reach the wire in the first place.
///
/// * **Sheeted in, nothing touches.** The lead car runs on a track inboard of the
///   chainplates, so a hard-sheeted headsail is *inside* the cap shroud over its whole
///   height — the wire is outboard of the leech, not underneath the cloth, and there is
///   nothing for a crease to form over. This is the correction: the drawing used to put
///   a crease up a genoa strapped to its track, which is a fold no sail makes, and the
///   force model standing a few hundred lines away had already said so.
/// * **Eased, it bears.** The clew goes out and the belly follows until the leech
///   fetches up on the wire and then on the spreader. From there, easing the sheet stops
///   opening the sail and starts pressing it into steel. That is why a genoa has
///   spreader patches sewn on it.
/// * **Eased right out, it is clear again.** Past about sixty degrees the clew is
///   outboard of the cap shroud and the whole sail is flying free of the rig. The first
///   version of this only built, and charged a beam reach for a contact that had ended.
///
/// Now there is one definition of the band and both the forces and the drawing read it,
/// because a sail creased in the picture where the physics says it is in clear air is
/// the fault this project keeps being caught by.
pub fn rig_contact(chord_angle: f64, bears_at: f64) -> f64 {
    let past = (chord_angle.abs() - bears_at).max(0.0);
    let on = smoothstep(0.0, rad(25.0), past);
    let off = smoothstep(rad(30.0), rad(60.0), past);
    on * (1.0 - off)
}

/// How hard two sails are lying on each other, 0 to 1.
///
/// A double head rig puts two sails a metre apart on the same bow, and nothing in the
/// model stopped the inner one being eased straight into the outer one. On the water
/// that is not a geometric curiosity, it is a noise: the staysail's leech fetches up
/// against the back of the genoa, both of them stop having a clean side and a dirty
/// side, and they flog. The cure is on the sheet — ease the outer or harden the inner
/// until the slot opens.
///
/// Measured off the one quantity that decides it: how much further out the inner sail
/// is sheeted than the outer. While it is inside, there is a gap and clean air through
/// it. Once it swings past, there is cloth where the gap was.
pub fn cloth_on_cloth(inner_chord: f64, outer_chord: f64) -> f64 {
    smoothstep(0.0, rad(14.0), inner_chord.abs() - outer_chord.abs())
}

/// How the two sails bend the wind for each other.
///
/// They are not two sails in the same breeze. Each one's circulation turns the flow the
/// other arrives in, and the pair has been understood this way since Gentry took it
/// apart in the seventies — before which half of trim theory was wrong in print.
///
/// The headsail's circulation *downwashes* the main: the flow reaching the mainsail's
/// luff has already been bent to leeward, so the main meets it at a smaller angle than
/// the masthead fly suggests. That is why a mainsail can be sheeted almost to the
/// centreline upwind without stalling — and why, when the headsail is sheeted too hard,
/// the bend becomes too much and the main's luff backwinds.
///
/// The mainsail's circulation *upwashes* the headsail: the flow arriving at the
/// headsail is turned to windward, so the headsail meets it at a larger angle than it
/// would alone. Which is the thing that was missing, and the thing a trimmer feels
/// first: haul the main in and the headsail starts working at a higher angle of attack
/// without anybody having touched its sheet. It goes soft, then it backwinds, and the
/// cure is to ease the main.
///
/// Returned as angles to add to each sail's apparent wind, in radians. The sizes are
/// the usual quoted ones — a few degrees each, the headsail's effect on the main the
/// larger of the two, both scaled by how hard the sail doing the bending is working.
pub fn slot_interaction(main_cl: f64, jib_cl: f64) -> (f64, f64) {
    /// Downwash on the main per unit of headsail lift coefficient.
    ///
    /// Three degrees, not the six this started at, and the reason is that the bend is
    /// not uniform across the sail. It is strongest at the mainsail's luff — where the
    /// published four to eight degrees is measured, and where the backwinding shows —
    /// and nearly nothing by the leech. A single number applied to the whole sail has to
    /// be the average of that, not the peak.
    ///
    /// Measured consequence of getting it wrong: at six degrees per unit, a close-hauled
    /// boat in five knots of breeze had nine and a half degrees taken off a sixteen
    /// degree apparent wind, which left the mainsail six degrees of angle and no useful
    /// drive at all. The main is a third of the drive upwind, not a rag.
    const ON_MAIN: f64 = 3.0 * std::f64::consts::PI / 180.0;
    /// Upwash on the headsail per unit of mainsail lift coefficient.
    const ON_JIB: f64 = 3.2 * std::f64::consts::PI / 180.0;
    (
        -ON_MAIN * jib_cl.clamp(0.0, 1.8),
        ON_JIB * main_cl.clamp(0.0, 1.8),
    )
}

/// Extra drag coefficient on a headsail sheeted close with no mainsail behind it.
///
/// This is the answer to "you cannot go to windward on the genoa alone", and the
/// mechanism is the leech rather than the luff — which is where I looked first, and
/// wrongly, because the luff of a headsail alone is attached perfectly well: its angle
/// of attack at the track lead is fourteen degrees against an entry angle of twelve,
/// and the test passes. Nothing was stopping her.
///
/// What a cambered sail does at its *leech* is the thing. The flow leaving a sail has
/// been turned by the camber and leaves along the tangent there, which for an eleven
/// percent sail stands about twenty-five degrees off its own chord. Sheeted to its
/// track at twelve degrees, that puts the exit flow pointing a dozen degrees across the
/// centreline — straight into the space the mainsail occupies. With the main set, that
/// space is the slot and the flow is received by it; the pair is one aerofoil and
/// Gentry's whole argument is that neither sail can be understood alone. With the main
/// handed, the same flow has to turn back out around nothing, against a rising pressure,
/// and it separates off the after part of the sail.
///
/// Which is why a headsail worked alone has to be *eased*: you cannot strap it in, and
/// every sailor who has sailed under headsail alone while the main was being reefed
/// knows it. Eased, the exit is fair again and the penalty goes away by itself — which
/// is the test of whether this is a mechanism or a fudge. Flatten the sail and it also
/// goes away, because a flat sail exits nearly along its chord: a storm jib is cut flat
/// and will point, a rolled-up genoa is a bag and will not.
///
/// Expressed as separation drag rather than as a limit on the sheet, because that is how
/// it actually shows up: the winch will still take the sheet, the sail will still look
/// full, and the boat will simply not go to windward. Drag is also exactly the quantity
/// that sets how close a boat can sail — the aerodynamic part of the sailing angle is
/// the arctangent of drag over lift — so this is the right place for it and not a
/// coincidence.
///
/// `SEPARATED` is the one calibrated number: set so this boat will not point closer
/// than fifty degrees true under headsail alone, which is the figure the sailor who
/// asked for it gives. The shape around it is geometry.
pub fn alone_leech_drag(camber: f64, chord_angle: f64, slot_support: f64) -> f64 {
    /// Drag coefficient with the after part of the sail fully separated.
    const SEPARATED: f64 = 2.10;
    let support = slot_support.clamp(0.0, 1.0);
    if support >= 1.0 {
        return 0.0;
    }
    // Tangent at the leech of a circular arc of this camber, from the chord. Twenty-five
    // degrees at the reference eleven percent.
    let exit = 2.0 * (2.0 * camber.max(0.0)).atan();
    if exit <= 0.0 {
        return 0.0;
    }
    let shortfall = ((exit - chord_angle.max(0.0)) / exit).clamp(0.0, 1.0);
    SEPARATED * shortfall * shortfall * (1.0 - support)
}

/// Where along the chord the force actually acts, as a fraction back from the luff.
///
/// Until now every sail's force was applied at a point fixed in the configuration, as
/// though a sail's centre of pressure were a property of the boat rather than of what
/// the sail is doing. It is not. Thin-aerofoil theory puts an attached section's load at
/// the quarter chord; camber carries it a little further forward than that, because the
/// deepest part of a cambered section does the most work and it is forward of the
/// middle; and separation walks it aft towards the half chord, because a stalled sail is
/// a plate with its load spread over the whole of it.
///
/// That walk is worth having. A mainsail eased on a reach and stalling moves its load
/// aft along a three-metre chord by a quarter of a metre or so — and the same movement
/// takes it further out to leeward, since the chord is lying across the boat. Both of
/// those turn the boat, both grow with exactly the conditions that overpower her, and
/// together they are a part of why a boat gripes up as she is pressed rather than
/// merely heeling further.
pub fn centre_of_pressure(camber: f64, stalled: f64) -> f64 {
    let cambered = 0.26 - 0.30 * (camber - CAMBER_REFERENCE);
    (cambered + 0.22 * stalled.clamp(0.0, 1.0)).clamp(0.18, 0.55)
}

/// The centre of pressure the fixed `x_ce` in the configuration already stands for, so
/// only the movement away from it is applied.
pub const COP_NOMINAL: f64 = 0.28;

/// How far to leeward a sail's own centre of effort sits once it is eased, metres.
///
/// The piece that was missing, and it is the piece a broach is made of.
///
/// Weather helm was modelled only through heel: heel swings the centre of effort out to
/// leeward, the drive acts on that lever, and the boat luffs up. True, and on its own
/// it says a boat that is not heeling cannot round up — which is why this model would
/// shrug off a thirty-four knot squall at a hundred and thirty-five degrees with a
/// gennaker set, taking four degrees off course. A real boat broaches *there*, at the
/// angle where she heels least.
///
/// Because heel is not the only thing that puts the sail out to leeward. Easing does.
/// A boom squared off to seventy-five degrees carries its sail a metre and more off the
/// centreline with the boat dead upright, and a gennaker tacked to the sprit and
/// sheeted aft is further out still — its whole belly is out there. The drive acts
/// through that offset and the moment is the same moment heel makes.
///
/// The arm is a fraction of the sail's own mean chord, which comes out of the area and
/// the aspect ratio without anyone having to measure a sail twice. A sail set flying
/// gets the larger fraction: it has no stay holding its luff in and nothing between it
/// and the water, so it bellies out where a genoa is held against the shrouds.
pub fn lateral_offset(
    profile: &SailProfile,
    area: f64,
    chord_angle: f64,
    flying: bool,
    cop: f64,
) -> f64 {
    let mean_chord = (area / profile.aspect_ratio.max(0.2)).sqrt();
    // Where the load sits along the chord, and a sail set flying sits further out again:
    // it is tacked a metre and a half ahead of the stem and bellies out with nothing to
    // hold it in, so its chord does not start on the centreline the way a genoa's does.
    let out = if flying { 0.45 } else { 0.0 };
    (cop + out) * mean_chord * chord_angle.abs().sin()
}

/// How far aft the load has moved from where the configuration assumes it is, metres.
pub fn chordwise_shift(profile: &SailProfile, area: f64, chord_angle: f64, cop: f64) -> f64 {
    let mean_chord = (area / profile.aspect_ratio.max(0.2)).sqrt();
    (cop - COP_NOMINAL) * mean_chord * chord_angle.abs().cos()
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

/// Outhaul a trimmer would carry on this course, at this much heel.
///
/// Two inputs, and the first one used to be missing. The rule was keyed on heel alone,
/// which made it the same rule close-hauled and on a broad reach — so the boat sailed
/// upwind with a foot eased right off, which is not a trim any crew would carry and not
/// one the sail can hold: a deep entry cannot be sheeted close without backwinding
/// behind the mast (see [`SailProfile::entry_angle`]). Upwind the outhaul goes on. Off
/// the wind it comes off, because there the sail wants depth and the entry has all the
/// angle it needs.
///
/// The heel term on top is the old rule and it stays: full sail is fastest until
/// righting moment is the binding constraint, and only then does reducing force start
/// to *increase* VMG — which is the answer an ORC VPP gives and the answer the sweep in
/// `examples/outhaul.rs` gives. The upwind baseline is set where that sweep says it
/// costs almost nothing: two hundredths of a knot of VMG at twelve knots of true wind,
/// against a sail shape a sailor will not accept.
///
/// `target_heel` is the angle past which the boat is judged overpowered — around
/// twenty degrees for a cruising crew.
pub fn outhaul_for(awa: f64, heel: f64, target_heel: f64) -> f64 {
    let upwind = 1.0 - smoothstep(rad(38.0), rad(72.0), awa.abs());
    // In a drifter you leave shape in it even upwind: there is no heel to get rid of
    // and the boat needs every square metre working. Heel is the pressure gauge the
    // function already has, and the sweep agrees — the measured best in five knots of
    // true wind is a touch of depth, not a strapped-down foot.
    let pressure = 0.3 + 0.7 * smoothstep(rad(4.0), rad(12.0), heel.abs());
    let pressed = smoothstep(target_heel, target_heel + rad(9.0), heel.abs());
    (UPWIND_OUTHAUL * upwind * pressure + pressed).clamp(0.0, 1.0)
}

/// Outhaul carried close-hauled before heel asks for any more than that.
///
/// Not 1.0: a foot strapped down hard is a light-air mistake, and the sweep agrees —
/// the measured best upwind in five knots is a touch of depth, not none.
pub const UPWIND_OUTHAUL: f64 = 0.45;

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
    /// Entry angle of the cut at reference draft: the angle the forward of the sail
    /// makes with its own chord, radians. Zero disables the effect.
    ///
    /// This is the number that stops depth being free. A sail is a membrane with a
    /// sharp luff, and the flow has to arrive within a few degrees of the tangent at
    /// that luff or it separates on the windward side — which is backwinding, the
    /// shaking patch behind the mast that every sailor has seen on a main sheeted hard
    /// with the outhaul eased. The entry angle grows with draft, roughly in proportion,
    /// so a deep sail needs *more* angle of attack to keep its entry full, and
    /// therefore cannot be sheeted close. That single fact is why the outhaul is pulled
    /// on going upwind and eased off the wind, and without it the model said depth was
    /// free at every wind speed and every course.
    ///
    /// Left at zero for the gennaker: two and twenty percent of draft is not a shape
    /// any law calibrated on upwind sails describes, and what actually limits a
    /// gennaker is collapse, which `fills_above` already models.
    pub entry_angle: f64,
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
    /// genuinely is zero, because its sheet leads to the centreline.
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
            entry_angle: rad(16.0),
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
            entry_angle: rad(12.0),
            cn_separated: 1.45,
            max_chord_angle: rad(85.0),
            min_chord_angle: rad(12.0),
        }
    }

    /// A staysail: small, flat, set on an inner stay well abaft the stem.
    ///
    /// The sail that makes a double head rig work. On its own it is a heavy-weather
    /// sail — genoa rolled away, main reefed, and this left drawing low down where its
    /// heeling moment is least. Under a Code 0 or a gennaker it is doing something
    /// different: it sits in the slot inside the big sail, adds a few square metres of
    /// its own and steadies the boat, and the gap between the two does more than either
    /// sail does alone.
    ///
    /// Self-tacking on a boom, which is how a cruising staysail of this size is nearly
    /// always rigged, so it has no sheet of its own to play — it finds its own angle and
    /// its track limits how far out it can go. That is why its chord is capped well
    /// short of a genoa's: a self-tacker cannot be squared off.
    pub fn staysail(area: f64, luff_height: f64) -> Self {
        Self {
            cl_max: 1.50,
            alpha_opt: rad(16.0),
            alpha_stall: rad(30.0),
            cd0: 0.050,
            aspect_ratio: luff_height * luff_height / area,
            luff_margin: rad(3.5),
            entry_angle: rad(12.0),
            cn_separated: 1.45,
            // Seventy-five, not sixty-two. The limit on a self-tacker is its own track
            // and nothing else: the sail and its boom live forward of the shrouds, with
            // clear deck either side and nothing to foul, so what stops the boom is the
            // end of the horse. There is no reason to cap it at a genoa's angle when it
            // has none of a genoa's obstructions.
            max_chord_angle: rad(75.0),
            min_chord_angle: rad(8.0),
        }
    }

    /// A Code 0: a reaching sail, which is a genoa's shape taken as far as it will go.
    ///
    /// Flat, which is the point and also the rating rule — a Code 0 is a spinnaker on
    /// paper and a headsail in behaviour, and it gets there by being cut with barely
    /// more depth than a genoa over an enormous area. Flat means it holds an attached
    /// flow to a small angle of attack, so it peaks early and stalls early, and it means
    /// it will sheet in hard: there is no mast in front of it and no shroud behind it,
    /// so the limit on how far it can be eased is the sail itself rather than the rig.
    ///
    /// The clean leading edge of a sail set flying is worth real lift. There is no
    /// forestay in its luff and no mast turbulence over it, which is why a Code 0 out-
    /// points its own area: it is the most efficient sail on the boat on a close reach.
    pub fn code_zero(area: f64, luff_height: f64) -> Self {
        Self {
            cl_max: 1.62,
            alpha_opt: rad(14.0),
            alpha_stall: rad(27.0),
            cd0: 0.040,
            aspect_ratio: luff_height * luff_height / area,
            luff_margin: rad(3.0),
            entry_angle: rad(11.0),
            cn_separated: 1.45,
            max_chord_angle: rad(86.0),
            min_chord_angle: rad(10.0),
        }
    }

    /// A gennaker: the running sail, and a different animal.
    ///
    /// Deep. A sail with twenty percent camber in it generates more lift than anything
    /// else on the boat and pays for it in drag, which is the correct trade downwind,
    /// where drag *is* drive. It peaks late, at an angle of attack that would have a
    /// genoa long stalled, and it stalls gently — and well past the stall it is still
    /// working, because a parachute does not care that its flow has separated. That is
    /// what `cn_separated` is for and why this sail's is the highest in the rig.
    ///
    /// It eases past the beam. On a sprit, tacked out ahead of the bow, the clew can be
    /// squared right out beyond anything a genoa could reach, which is how it gets clear
    /// of the mainsail's wind shadow deep downwind — and why, unlike a genoa, it does
    /// not die behind the main.
    pub fn gennaker(area: f64, luff_height: f64) -> Self {
        Self {
            cl_max: 1.78,
            alpha_opt: rad(22.0),
            alpha_stall: rad(42.0),
            cd0: 0.075,
            aspect_ratio: luff_height * luff_height / area,
            luff_margin: rad(5.0),
            entry_angle: rad(0.0),
            cn_separated: 1.62,
            // Eighty-eight degrees, not the hundred and eight this had.
            //
            // The chord angle is measured from the centreline *aft*, so ninety degrees
            // is the foot square across the boat and anything beyond it has the foot
            // pointing forward of abeam — the sail set back to front, leech ahead of
            // luff. Which is what it looked like: reported from the deck as the gennaker
            // appearing to work backwards, and it was, for every trim past ninety.
            //
            // A sail sheeted to a block on the quarter cannot get its clew forward of
            // abeam however far it is eased; what does go forward of abeam is a
            // symmetric spinnaker on a pole, which is a different sail with a different
            // spar holding its other corner.
            max_chord_angle: rad(88.0),
            min_chord_angle: rad(22.0),
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

        // A round entry cannot be sheeted close. Below the entry angle the flow starts
        // arriving on the windward side of the luff, and the sail answers by shaking
        // there and by dragging: a separation bubble, not attached flow.
        let entry = self.entry_angle * camber.max(0.0) / CAMBER_REFERENCE;
        let backwinding = if entry > 0.0 {
            1.0 - smoothstep(0.5 * entry, entry, alpha)
        } else {
            0.0
        };
        let luffing = (1.0 - smoothstep(0.0, self.luff_margin, alpha)).max(0.55 * backwinding);

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
        // Separation drag past the best angle, and this number was measured against
        // published sail data rather than chosen.
        //
        // At 0.35 the model gave a mainsail a lift-to-drag of 2.3 at thirty-four
        // degrees of attack and 2.0 at forty. Real sails — Marchaj's measurements, and
        // every wind-tunnel set since — are at 1.3 to 1.5 there. The error does not look
        // like much until you ask where the resultant force points: a sail with L/D of
        // 2.2 carries its force twenty-four degrees forward of its own normal, and one
        // with 1.4 carries it eight. On a broad reach with the boom squared off, that is
        // the difference between a side force of a few hundred newtons and none at all —
        // which is why this boat used to sail at a hundred and thirty-five degrees in
        // twenty-six knots of wind heeled two tenths of a degree, and could not be made
        // to broach at any wind speed with any sail set.
        //
        // 0.60 puts L/D at 1.5 where the measurements put it.
        let cd_attached = cd0
            + cl_attached * cl_attached / (std::f64::consts::PI * self.aspect_ratio.max(0.5))
            + 0.60 * past_peak * past_peak
            + 0.25 * backwinding;

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
        // Thin-aerofoil theory says a deeper section peaks earlier, and for a rigid
        // wing it does. For a sail the entry constraint overrules it once the sail is
        // set deep enough: you cannot trim to an angle of attack at which your own luff
        // is backwinding, so the best *usable* angle is the entry angle and a little
        // more. This is why a crew with the outhaul eased trims to a wider chord angle
        // and sails a wider course, rather than pointing the same and going faster.
        let theoretical = self.alpha_opt + ALPHA_SHIFT_PER_DEPTH * (1.0 - self.depth_ratio(camber));
        let entry = self.entry_angle * camber.max(0.0) / CAMBER_REFERENCE;
        theoretical.max(entry + rad(1.5))
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

impl SailForces {
    /// Scale lift and drag, and the drive and side force resolved from them.
    ///
    /// Used for what a headsail loses by lying on the standing rigging. The resolved
    /// components are scaled in place rather than the whole resolution being redone,
    /// because the angle they were resolved at is exactly the thing that is easy to
    /// get wrong and expensive when it is — this model has already paid once for
    /// resolving an appendage's drag in the wrong frame. With both factors this close
    /// to one, scaling drive by the mean and side by the lift factor is within a
    /// fraction of a per cent of redoing it, and it cannot get the frame wrong.
    pub fn scaled(self, lift: f64, drag: f64) -> Self {
        // Recover the angle the forces were resolved at, then resolve again.
        //
        // Drive and side come out of lift and drag as
        //     drive = L sin(a) - D cos(a)
        //     side  = L cos(a) + D sin(a)
        // which is a rotation scaled by hypot(L, D), so it inverts exactly. Doing that
        // costs two divisions and is the only way to be sure.
        //
        // The first attempt scaled drive by the mean of the two factors, which with a
        // lift factor of 0.92 and a drag factor of 1.15 comes to 1.035: it handed the
        // boat *more* drive for *more* drag. The measurement caught it — a beam reach
        // in twenty-five knots got a tenth of a knot faster when a loss was added —
        // and it is exactly the sloppiness the comment above this one was warning
        // about, written by someone who then went and did it anyway.
        let norm = self.lift * self.lift + self.drag * self.drag;
        if norm < 1e-12 {
            return self;
        }
        let sin_a = (self.lift * self.drive + self.drag * self.side) / norm;
        let cos_a = (self.lift * self.side - self.drag * self.drive) / norm;
        let (l, d) = (self.lift * lift, self.drag * drag);
        Self {
            lift: l,
            drag: d,
            drive: l * sin_a - d * cos_a,
            side: l * cos_a + d * sin_a,
            ..self
        }
    }
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
    sail_forces_in_slot(
        profile, awa, aws, sheet, area, heel, blanketing, camber, 1.0,
    )
}

/// The same, for a headsail told how much mainsail is working behind it.
///
/// `slot_support` runs from one — a main set and pulling, the pair working as the single
/// aerofoil it really is — down to zero, the main handed and the headsail on its own.
/// See [`alone_leech_drag`]. A ninth positional argument on a function that already had
/// eight is not an improvement, which is why the plain [`sail_forces`] stays and assumes
/// full support: every other sail on the boat either has a main behind it or is not a
/// headsail.
#[allow(clippy::too_many_arguments)]
pub fn sail_forces_in_slot(
    profile: &SailProfile,
    awa: f64,
    aws: f64,
    sheet: f64,
    area: f64,
    heel: f64,
    blanketing: f64,
    camber: f64,
    slot_support: f64,
) -> SailForces {
    let beta = awa.abs();
    let chord_angle = profile.chord_angle(sheet);
    let alpha = beta - chord_angle;
    let mut coeffs = profile.coefficients(alpha, camber);
    coeffs.cd += alone_leech_drag(camber, chord_angle, slot_support);

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
pub fn headsail_blanketing(awa: f64, boom_angle: f64) -> f64 {
    // The shadow is cast by the mainsail, so where it falls depends on where the
    // mainsail *is*. The fixed hundred and twenty degrees this used to start at happened
    // to be right for a boom squared off against the shrouds and was wrong everywhere
    // else: sheeted in on a reach the main cannot shade a headsail set ahead of it at
    // any angle, and the old curve had it doing so the moment the apparent wind went
    // aft of a hundred and twenty.
    //
    // The headsail is forward of the main, so it is in the lee of it once the wind comes
    // from behind the mainsail — which is the boom's angle plus the width of the sail's
    // own wake, and not before.
    let from = boom_angle.abs() + rad(40.0);
    0.85 * smoothstep(from, from + rad(55.0), awa.abs())
}
