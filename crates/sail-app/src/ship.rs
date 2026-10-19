//! The square-rigger.
//!
//! **Why there is a second vessel in this game at all.** Everything in it so far is
//! something you do to your own boat: trim her, navigate her, get somebody back over her
//! rail. The one thing missing is the sea having anybody else on it — and the moment it
//! does, the oldest trade on the coast is available, which is **pilotage**: a deep-draft
//! stranger stands in towards a coast she does not know, and somebody who does know it
//! goes out, gets aboard, and takes her through. That is a real job, it is done in a
//! small boat, and it is the one piece of seamanship where your local knowledge is the
//! whole of your value.
//!
//! **Why a brig.** Two masts, square on both, and nothing else: it is the simplest rig
//! that is honestly square, and it is what the coasting trade actually used — colliers,
//! timber brigs, the ordinary heavy lifting of the nineteenth century. A full ship wants
//! three masts and a page of extra spars to say the same thing. She carries a spanker on
//! the main and headsails off the bowsprit because a brig that cannot steer without
//! them, and the one thing everybody knows about square rig is that she will not go to
//! windward: about sixty-five degrees off the true wind on a good day, which is why she
//! needs sea room and why a lee shore is the thing that kills her.
//!
//! **What is modelled and what is not.** She is drawn to the same standard as our own
//! boat — a lofted hull, masts in their proper segments, yards that brace round with the
//! wind, cloth that bellies to leeward and backs when the wind comes on the front of it
//! — and she is *sailed* by something much simpler than `sail-core`: a speed off her own
//! polar and a course she steers. That split is deliberate. What the player reads off
//! her is the sail work and her heading; nobody can see her leeway and nobody is trimming
//! her.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use glam::DVec2;
use sail_core::units::{kn_to_ms, ms_to_kn, rad, smoothstep, wrap_pi, wrap_tau};

#[allow(unused_imports)]
use crate::frame;
use crate::sim::Sim;

// ---------------------------------------------------------------------------------------
// Her dimensions. A coasting brig of about three hundred tons.
// ---------------------------------------------------------------------------------------

/// Hull length on deck, metres, not counting the bowsprit.
pub const LOA: f32 = 31.0;
/// Greatest beam.
pub const BEAM: f32 = 8.2;
/// Draft aft, loaded.
pub const DRAFT: f32 = 3.9;
/// Freeboard at the waist, deck to water.
const FREEBOARD: f32 = 2.3;
/// Bulwark above the deck: a working brig has a solid one, chest high, because the deck
/// is where cargo is handled and the sea comes aboard.
const BULWARK: f32 = 1.25;

/// Where each mast stands, as a fraction of the length from the bow.
const FOREMAST: f32 = 0.26;
const MAINMAST: f32 = 0.63;

/// Height of the lower masts above the deck, and of the masts above them.
const LOWER_MAST: f32 = 15.5;
const TOPMAST: f32 = 11.5;
const TOPGALLANT: f32 = 7.5;

/// The bowsprit, outboard of the stem, and how far it steeves up.
const BOWSPRIT: f32 = 9.0;
const STEEVE: f32 = 0.22;

/// Half-length of each yard, as a fraction of beam. A course yard on a brig this size is
/// a touch over her beam; everything above is shorter, which is what gives a square
/// rigger her pyramid.
const YARDS: [(f32, f32); 3] = [
    // (height above the deck, half-length)
    (3.2, 0.62),
    (14.2, 0.52),
    (23.0, 0.38),
];

// ---------------------------------------------------------------------------------------
// What she is doing.
// ---------------------------------------------------------------------------------------

/// The brig, where she is and what she is at.
#[derive(Resource, Clone, Debug)]
pub struct Brig {
    /// Where she is, in simulation coordinates.
    pub at: DVec2,
    /// Her head, radians from north.
    pub heading: f64,
    /// Through the water, m/s.
    pub speed: f64,
    /// Positive to starboard, radians.
    pub heel: f64,
    /// Where she is actually going, as against where she is pointed.
    ///
    /// **A vessel has way, and the way is a vector.** Hers lags her heading through a
    /// turn and sits a few degrees to leeward of it under sail; the difference between
    /// the two is the crab you can see from another boat, and its absence is what made
    /// her look like she was sliding.
    pub way: DVec2,
    /// How far the yards are braced round from square, radians, signed: positive swings
    /// the starboard yardarms aft.
    pub braced: f64,
    /// How much of each sail is set, 0 furled to 1 sheeted home. Course, topsail,
    /// topgallant — the order they are taken in, which is from the top down.
    pub set: [f32; 3],
    /// Whether she is on the water at all yet.
    pub afloat: bool,
    /// Where she is going. In her own right until a pilot is aboard; after that it is
    /// the pilot's problem.
    pub bound_for: DVec2,
}

impl Default for Brig {
    fn default() -> Self {
        Self {
            at: DVec2::ZERO,
            heading: 0.0,
            speed: 0.0,
            heel: 0.0,
            way: DVec2::ZERO,
            braced: 0.0,
            set: [1.0, 1.0, 1.0],
            afloat: false,
            bound_for: DVec2::ZERO,
        }
    }
}

/// The height her sail plan works at, metres.
///
/// **The same wind field as ours, read where her sails are.** Both vessels sample one
/// field at their own positions, so a gust that heads you heads her when it reaches her
/// and a squall is in the same place for both — but our rig is read sail by sail at its
/// own centre of effort, around six metres, and hers was being read at the reference
/// height of ten. Her courses are at eight metres and her topgallants at twenty-six: the
/// middle of that, weighted by where her area actually is, is sixteen, and the shear
/// over open water makes that five per cent more wind than the ten-metre figure. Small,
/// and it is the reason a square-rigger keeps her upper sails drawing in air the deck
/// cannot feel.
const SAIL_PLAN_HEIGHT: f64 = 16.0;

/// The wind she is actually in: the field, at her position, at her rig's height.
pub fn her_wind(sim: &Sim, at: DVec2) -> DVec2 {
    sim.sim.wind.sample(at, sim.sim.state.time) * sail_core::wind::shear_factor(SAIL_PLAN_HEIGHT)
}

/// Her wind angle: nought head to wind, plus or minus a hundred and eighty running.
///
/// **A function, because getting it upside down makes a square-rigger sail into the wind
/// at six knots.** That is not a hypothetical: she did, and she was reported from the
/// deck for pointing higher than a bermudan sloop, which is the one thing this rig
/// cannot do. The simulation's convention is that `direction_of` gives the direction the
/// wind blows *from*, and an angle off the bow is measured from it — no half turn in it
/// anywhere.
pub fn angle_off(heading: f64, from: f64) -> f64 {
    wrap_pi(heading - from)
}

/// And the course that puts her at a given angle to the wind, which is the same statement
/// turned round. One place, so the two cannot drift apart.
pub fn course_for(from: f64, angle: f64) -> f64 {
    wrap_tau(from + angle)
}

/// The closest she will lie to the true wind, radians.
///
/// **Sixty-five degrees, and it is the whole character of the rig.** A square sail cannot
/// be braced past the shrouds, so the best angle of attack it can be given runs out long
/// before a fore-and-aft sail's does. Everything else about handling a square-rigger —
/// why she needs sea room, why a lee shore kills her, why she wears rather than tacks —
/// comes out of this one number.
pub const CLOSEST: f64 = rad(65.0);

/// How fast she goes at a given true wind angle and strength, m/s.
///
/// A square-rigger's polar is the opposite shape to a yacht's: nothing upwind, and her
/// best point of sailing is a broad reach where every sail on both masts is drawing and
/// none of them is in another's lee. Running dead before it the fore blankets the main
/// and she loses a knot of it, which is why she is sailed a little off the dead run.
pub fn through_the_water(twa: f64, tws: f64) -> f64 {
    let off = twa.abs();
    if off < CLOSEST {
        // In irons, or near enough that she is making leeway and not much else.
        return kn_to_ms(0.6);
    }
    // Shape: rises from the closest she can lie, best about a hundred and twenty, and
    // falls away again dead before the wind.
    let reach = ((off - CLOSEST) / (rad(120.0) - CLOSEST)).min(1.0);
    let past = ((off - rad(120.0)).max(0.0) / rad(60.0)).min(1.0);
    let shape = reach.powf(0.65) * (1.0 - 0.22 * past * past);
    // Eight knots in a stiff breeze is a fast day for a loaded brig; she is a carrier,
    // not a clipper, and her hull speed is the wall at the top of it.
    let wind = (ms_to_kn(tws) / 18.0).min(1.6);
    kn_to_ms(8.4 * shape * wind.powf(0.72))
}

/// How far round the yards are braced for a given apparent wind angle, radians.
///
/// Square to the mast running, hauled round as the wind comes forward, and up against
/// the shrouds at about fifty degrees — which is as far as the braces will take them on a
/// brig, and the reason she stops going anywhere above it.
pub fn brace_for(awa: f64) -> f64 {
    let off = awa.abs().min(std::f64::consts::PI);
    // Dead aft the yards are square; by the beam they are halfway round; close-hauled
    // they are against the stops.
    let want = (std::f64::consts::PI - off) * 0.62;
    want.min(rad(50.0)) * -awa.signum()
}

/// A sail that has the wind on the *front* of its cloth is aback.
///
/// Not a detail: it is the one state of a square sail everybody recognises, it is how a
/// square-rigger is stopped and how she is turned in a narrow place, and a drawn sail
/// that bellies the same way whatever the wind is doing is a flag, not a sail.
pub fn aback(awa: f64, braced: f64) -> bool {
    // The yard's own normal, against the wind. Square to her head the sail faces
    // forward; braced round it faces further and further out on the bow.
    wrap_pi(awa - braced).abs() < std::f64::consts::FRAC_PI_2
}

#[cfg(test)]
mod how_she_sails {
    use super::*;

    /// **Head to wind is nought and running is a hundred and eighty**, which is the
    /// convention the rest of the simulation uses and the one this file had upside down.
    ///
    /// With the half turn in it she read a beat as a run: ordered to a mark dead to
    /// windward she squared her yards and went at it at six knots, pointing higher than
    /// a bermudan sloop, which is the one thing a square-rigger cannot do. Reported from
    /// the deck in four words — *poszedł dużo ostrzej ode mnie*.
    #[test]
    fn her_wind_angle_is_measured_the_way_everybody_elses_is() {
        let north = 0.0; // the wind comes from the north
        assert!(
            angle_off(rad(0.0), north).abs() < 1e-9,
            "head to wind is nought"
        );
        assert!(
            (angle_off(rad(180.0), north).abs() - std::f64::consts::PI).abs() < 1e-9,
            "running is a hundred and eighty"
        );
        assert!((angle_off(rad(90.0), north) - rad(90.0)).abs() < 1e-9);
        // And the course that puts her at an angle is the same statement turned round.
        assert!(
            (angle_off(course_for(north, CLOSEST), north) - CLOSEST).abs() < 1e-9,
            "the two halves of the convention do not agree"
        );
        // The thing that was actually wrong: a ship headed at the wind must be stopped,
        // not flying.
        assert!(
            ms_to_kn(through_the_water(
                angle_off(rad(5.0), north),
                kn_to_ms(14.0)
            )) < 1.0
        );
        assert!(
            ms_to_kn(through_the_water(
                angle_off(rad(170.0), north),
                kn_to_ms(14.0)
            )) > 4.0
        );
    }

    /// **She does not go to windward**, which is the whole character of the rig.
    #[test]
    fn nothing_upwind_and_her_best_is_a_broad_reach() {
        let tws = kn_to_ms(14.0);
        let beating = through_the_water(rad(40.0), tws);
        assert!(
            ms_to_kn(beating) < 1.0,
            "a brig was making {:.1} kn at forty degrees off the wind",
            ms_to_kn(beating)
        );
        let mut best = (0.0, 0.0);
        for twa in (70..=180).step_by(5) {
            let v = through_the_water(rad(twa as f64), tws);
            if v > best.1 {
                best = (twa as f64, v);
            }
        }
        assert!(
            (100.0..=150.0).contains(&best.0),
            "her best point of sailing came out at {:.0} degrees",
            best.0
        );
        assert!(
            ms_to_kn(best.1) > 4.0 && ms_to_kn(best.1) < 9.0,
            "{:.1} kn in fourteen knots of wind",
            ms_to_kn(best.1)
        );
        // And dead before it is slower than the broad reach, because the fore takes the
        // main's wind.
        assert!(through_the_water(rad(180.0), tws) < best.1);
    }

    /// The yards go square running and up against the shrouds close-hauled, and they go
    /// round the right way: the wind on the starboard side puts the starboard arms aft.
    #[test]
    fn the_yards_brace_round_with_the_wind_and_stop_at_the_shrouds() {
        assert!(brace_for(rad(180.0)).abs() < rad(2.0), "square before it");
        let beam = brace_for(rad(90.0));
        assert!(
            beam < 0.0 && beam.abs() > rad(25.0),
            "braced {:.0} deg with the wind on the beam",
            beam.to_degrees()
        );
        assert!(
            brace_for(rad(70.0)).abs() <= rad(50.0) + 1e-9,
            "the braces took the yards past the shrouds"
        );
        // Mirror image on the other tack.
        assert!((brace_for(rad(-90.0)) + beam).abs() < 1e-9);
    }

    /// And a sail with the wind on the front of it is aback — the state that stops her,
    /// and the one that turns her in a narrow place.
    #[test]
    fn a_yard_braced_against_the_wind_is_aback() {
        // Running, yards square: the wind is on the back of the cloth.
        assert!(!aback(rad(180.0), 0.0));
        // Braced for a reach, with the wind where it should be: drawing.
        assert!(!aback(rad(100.0), brace_for(rad(100.0))));
        // Head to wind with the yards still braced for the old tack: aback, which is
        // exactly what happens when a square-rigger misses stays.
        assert!(aback(rad(10.0), brace_for(rad(100.0))));
    }
}

// ---------------------------------------------------------------------------------------
// Her shape.
// ---------------------------------------------------------------------------------------

/// Half-breadth at a station, metres. `t` runs 0 at the stem to 1 at the transom.
///
/// Full in the middle and carried well aft — a cargo brig is a box with ends on her, not
/// a yacht. The transom does not come to a point, because a transom is a flat board.
fn half_beam(t: f32) -> f32 {
    let fore = (t / 0.45).clamp(0.0, 1.0);
    let aft = ((1.0 - t) / 0.55).clamp(0.0, 1.0);
    let body = fore.powf(0.55) * (0.44 + 0.56 * aft.powf(0.42));
    BEAM * 0.5 * body.clamp(0.02, 1.0)
}

/// Height of the deck line above the water at a station: the sheer.
///
/// Highest at the bow, lowest a little abaft the middle, and up again at the stern. It is
/// the line that makes a hull look like a ship rather than like a bathtub.
fn sheer(t: f32) -> f32 {
    let low = 0.62; // where the sheer is lowest, from forward
    let d = (t - low) / if t < low { low } else { 1.0 - low };
    FREEBOARD + 1.05 * d * d
}

/// How deep she is at a station. The keel is straight for most of her length and rises
/// at both ends — the forefoot sharply, the run aft more gently.
fn depth(t: f32) -> f32 {
    let fore = (t / 0.16).clamp(0.0, 1.0);
    let aft = ((1.0 - t) / 0.22).clamp(0.0, 1.0);
    DRAFT * fore.powf(0.75) * aft.powf(0.55).max(0.18)
}

/// The half-section at a station: how far out the hull is at a given height.
///
/// `u` runs 0 at the keel to 1 at the deck line. A hull like this is slack in the garboard,
/// turns hard at the bilge, and comes back in again above the waterline — the tumblehome
/// that every wooden ship has and that makes her sides read as curved rather than as
/// walls.
fn section(t: f32, u: f32) -> Vec3 {
    let b = half_beam(t);
    let d = depth(t);
    let s = sheer(t);
    // Out from the centreline: fast at the turn of the bilge, then nearly straight.
    let out = u.powf(0.42);
    let tumblehome = 1.0 - 0.10 * u.powf(5.0);
    Vec3::new(
        b * out * tumblehome,
        -d + (d + s) * u,
        LOA * (t - 0.5), // bow is -Z
    )
}

/// What colour the hull is at a given height: a working brig's paintwork.
///
/// Black topsides, a broad ochre band at the wale with the painted ports on it, and
/// copper below the waterline gone the colour copper goes. It is paint rather than
/// decoration — the band is where the wale is, which is the heaviest strake in the hull.
fn paintwork(y: f32) -> [f32; 4] {
    const COPPER: [f32; 4] = [0.32, 0.20, 0.13, 1.0];
    const BLACK: [f32; 4] = [0.07, 0.07, 0.08, 1.0];
    const OCHRE: [f32; 4] = [0.52, 0.38, 0.16, 1.0];
    if y < 0.05 {
        COPPER
    } else if (0.9..1.9).contains(&y) {
        OCHRE
    } else {
        BLACK
    }
}

/// A mesh under construction.
#[derive(Default)]
struct Shell {
    positions: Vec<Vec3>,
    colours: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl Shell {
    /// A quad painted by where it is on the hull.
    fn quad(&mut self, corners: [Vec3; 4]) {
        let colours = corners.map(|p| paintwork(p.y));
        self.faces(corners, colours);
    }

    /// And one painted a colour of its own: rope, cloth, a spar.
    fn painted(&mut self, corners: [Vec3; 4], colour: [f32; 4]) {
        self.faces(corners, [colour; 4]);
    }

    fn faces(&mut self, corners: [Vec3; 4], colours: [[f32; 4]; 4]) {
        let base = self.positions.len() as u32;
        for (p, c) in corners.into_iter().zip(colours) {
            self.positions.push(p);
            self.colours.push(c);
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn build(self) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colours);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh.compute_normals();
        mesh
    }
}

/// The hull, both sides, with its deck and its bulwarks.
fn hull_mesh() -> Mesh {
    const STATIONS: usize = 40;
    const HEIGHTS: usize = 12;
    let mut shell = Shell::default();
    let at = |i: usize, j: usize, side: f32| {
        let t = i as f32 / STATIONS as f32;
        let u = j as f32 / HEIGHTS as f32;
        let p = section(t, u);
        Vec3::new(p.x * side, p.y, p.z)
    };
    for side in [1.0_f32, -1.0] {
        for i in 0..STATIONS {
            for j in 0..HEIGHTS {
                // Wound so that both sides face outwards.
                let q = if side > 0.0 {
                    [
                        at(i, j, side),
                        at(i, j + 1, side),
                        at(i + 1, j + 1, side),
                        at(i + 1, j, side),
                    ]
                } else {
                    [
                        at(i, j, side),
                        at(i + 1, j, side),
                        at(i + 1, j + 1, side),
                        at(i, j + 1, side),
                    ]
                };
                shell.quad(q);
            }
        }
        // The bulwark: the same line carried up above the deck, and a cap rail on it.
        for i in 0..STATIONS {
            let top = |i: usize| {
                let p = at(i, HEIGHTS, side);
                Vec3::new(p.x * 0.985, p.y + BULWARK, p.z)
            };
            let q = if side > 0.0 {
                [
                    at(i, HEIGHTS, side),
                    top(i),
                    top(i + 1),
                    at(i + 1, HEIGHTS, side),
                ]
            } else {
                [
                    at(i, HEIGHTS, side),
                    at(i + 1, HEIGHTS, side),
                    top(i + 1),
                    top(i),
                ]
            };
            shell.quad(q);
        }
    }
    // The deck, laid between the two sheers at deck height.
    for i in 0..STATIONS {
        let deck = |i: usize| {
            let p = at(i, HEIGHTS, 1.0);
            (Vec3::new(p.x, p.y, p.z), Vec3::new(-p.x, p.y, p.z))
        };
        let (sb0, pt0) = deck(i);
        let (sb1, pt1) = deck(i + 1);
        shell.quad([pt0, sb0, sb1, pt1]);
    }
    // And the transom, which is a flat board across the stern and the one place a hull
    // like this is not a curve.
    let stern: Vec<Vec3> = (0..=HEIGHTS).map(|j| at(STATIONS, j, 1.0)).collect();
    for j in 0..HEIGHTS {
        let (a, b) = (stern[j], stern[j + 1]);
        shell.quad([Vec3::new(-a.x, a.y, a.z), Vec3::new(-b.x, b.y, b.z), b, a]);
    }
    shell.build()
}

/// Where the deck is at a station, for putting masts and fittings on it.
fn deck_at(t: f32) -> f32 {
    section(t, 1.0).y
}

/// Along the hull, in her own frame, for a fraction of her length from the bow.
fn along(t: f32) -> f32 {
    LOA * (t - 0.5)
}

#[cfg(test)]
mod her_shape {
    use super::*;

    /// **A hull is a shape, and the way to know it is one is to measure it.**
    ///
    /// Not a box: she narrows at both ends, she is deepest in the middle, and her sheer
    /// is higher at the bow than anywhere else — which is the line that makes a hull
    /// read as a ship.
    #[test]
    fn she_is_hull_shaped() {
        assert!(
            half_beam(0.0) < BEAM * 0.1,
            "the stem is a point, not a wall"
        );
        assert!(
            half_beam(0.45) > BEAM * 0.45,
            "she should be full amidships"
        );
        assert!(
            half_beam(1.0) > BEAM * 0.2 && half_beam(1.0) < BEAM * 0.42,
            "a transom is a board: {:.2} m across the half",
            half_beam(1.0)
        );
        assert!(depth(0.5) > DRAFT * 0.9, "deepest in the middle");
        assert!(depth(0.0) < DRAFT * 0.2, "the forefoot rises to the stem");
        assert!(
            sheer(0.0) > sheer(0.6),
            "the bow is the high point of the sheer"
        );
        assert!(sheer(1.0) > sheer(0.6), "and she lifts again at the stern");
    }

    /// Every station is a closed, outward-growing section — no part of her comes *in*
    /// as it goes up, except the little the tumblehome asks for.
    #[test]
    fn no_section_folds_back_on_itself() {
        for i in 0..=20 {
            let t = i as f32 / 20.0;
            let mut last = -1.0;
            for j in 0..=12 {
                let p = section(t, j as f32 / 12.0);
                assert!(
                    p.x >= last - 0.12,
                    "station {t:.2} came back in by {:.3} m at height {j}",
                    last - p.x
                );
                last = p.x;
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Her rig, and the cloth on it.
// ---------------------------------------------------------------------------------------

/// The hull, which everything else hangs off.
#[derive(Component)]
pub struct Hull;

/// A yard: which mast it is on, and which tier — 0 the course, 2 the topgallant.
#[derive(Component)]
pub struct Yard {
    pub mast: usize,
    pub tier: usize,
}

/// The cloth bent to that yard.
#[derive(Component)]
pub struct Canvas {
    pub mast: usize,
    pub tier: usize,
}

/// The fore-and-aft sails: the spanker aft of the main, and the headsails on the
/// bowsprit. A brig without them will not steer, which is the only reason a square
/// rigger carries any.
#[derive(Component)]
pub struct Driver(usize);

/// Where a mast stands and how far up each of its pieces reaches, in her own frame.
fn mast_foot(mast: usize) -> Vec3 {
    let t = if mast == 0 { FOREMAST } else { MAINMAST };
    Vec3::new(0.0, deck_at(t), along(t))
}

/// Where the end of the bowsprit is.
fn bowsprit_end() -> Vec3 {
    let stem = Vec3::new(0.0, deck_at(0.02) + 0.4, along(0.0));
    stem + Vec3::new(0.0, BOWSPRIT * STEEVE, -BOWSPRIT)
}

/// A rope or a wire, as a pair of crossed ribbons so that it reads from any angle.
///
/// The same trick the standing rigging on our own boat uses and for the same reason: a
/// true seven-millimetre shroud at thirty metres is less than a pixel, so it is drawn
/// over-scale, and a single flat ribbon disappears the moment you look at it edge on.
fn rope(shell: &mut Shell, from: Vec3, to: Vec3, width: f32, colour: [f32; 4]) {
    let along = to - from;
    if along.length_squared() < 1e-6 {
        return;
    }
    let dir = along.normalize();
    let a = dir.cross(Vec3::Y).normalize_or(Vec3::X) * width;
    let b = dir.cross(a).normalize_or(Vec3::Z) * width;
    for across in [a, b] {
        shell.painted(
            [from - across, from + across, to + across, to - across],
            colour,
        );
    }
}

const TARRED: [f32; 4] = [0.09, 0.08, 0.07, 1.0];
const HEMP: [f32; 4] = [0.42, 0.36, 0.26, 1.0];

/// Shrouds, ratlines and stays: the web that holds the masts up and that a crew climbs.
///
/// **The ratlines are not decoration.** They are the single thing that says *square
/// rigger* at half a mile — the eye reads the ladder up the shrouds long before it can
/// count the yards — and they are there because on this ship the sails are handed by
/// people who have to get to them.
fn rigging_mesh() -> Mesh {
    let mut shell = Shell::default();
    for mast in 0..2 {
        let foot = mast_foot(mast);
        let top = foot + Vec3::Y * LOWER_MAST;
        let cap = top + Vec3::Y * TOPMAST;
        let truck = cap + Vec3::Y * TOPGALLANT;
        let t = if mast == 0 { FOREMAST } else { MAINMAST };
        // The channels: a shelf outboard of the rail that spreads the shrouds, and the
        // reason a shroud on a wooden ship stands clear of the bulwark.
        let chain = half_beam(t) * 1.06;
        for side in [1.0_f32, -1.0] {
            let feet: Vec<Vec3> = (0..5)
                .map(|i| Vec3::new(side * chain, foot.y + 1.0, foot.z + 0.55 + i as f32 * 0.62))
                .collect();
            for f in &feet {
                rope(&mut shell, *f, top, 0.055, TARRED);
            }
            // Ratlines, every fifteen inches of it, which is a stair tread.
            let rungs = ((LOWER_MAST - 1.5) / 0.4) as usize;
            for r in 1..rungs {
                let u = r as f32 / rungs as f32;
                let a = feet[0].lerp(top, u);
                let b = feet[feet.len() - 1].lerp(top, u);
                rope(&mut shell, a, b, 0.028, HEMP);
            }
            // Topmast shrouds, from the top to the topmast head, and a backstay.
            let spread = Vec3::new(side * 1.9, top.y + 0.1, top.z);
            rope(&mut shell, spread, cap, 0.045, TARRED);
            rope(
                &mut shell,
                cap,
                Vec3::new(side * chain, foot.y + 1.0, along(0.96)),
                0.045,
                TARRED,
            );
        }
        // Stays, forward: what actually stops a mast falling aft, and what the headsails
        // are hanked to.
        let ahead = if mast == 0 {
            bowsprit_end()
        } else {
            mast_foot(0) + Vec3::Y * (LOWER_MAST * 0.72)
        };
        rope(&mut shell, top, ahead, 0.06, TARRED);
        let high = if mast == 0 {
            bowsprit_end() + Vec3::new(0.0, 1.6, -0.6)
        } else {
            mast_foot(0) + Vec3::Y * (LOWER_MAST + TOPMAST * 0.6)
        };
        rope(&mut shell, cap, high, 0.05, TARRED);
        rope(&mut shell, truck, high + Vec3::Y * 2.0, 0.04, TARRED);
        // And the braces: the ropes that swing the yards, led aft to the next mast or to
        // the rail. They are what the whole watch hauls on.
        for (tier, (height, half)) in YARDS.iter().enumerate() {
            let arm = |side: f32| Vec3::new(side * BEAM * half, foot.y + height, foot.z);
            let aft = if mast == 0 {
                mast_foot(1) + Vec3::Y * (height * 0.86)
            } else {
                Vec3::new(0.0, foot.y + 1.2, along(0.97))
            };
            for side in [1.0_f32, -1.0] {
                let _ = tier;
                rope(
                    &mut shell,
                    arm(side),
                    aft + Vec3::X * side * 0.8,
                    0.035,
                    HEMP,
                );
            }
        }
    }
    // The bobstay, which holds the bowsprit *down* against everything the headsails do
    // to it, and the martingale under it.
    let end = bowsprit_end();
    rope(
        &mut shell,
        end,
        Vec3::new(0.0, -1.2, along(0.015)),
        0.06,
        TARRED,
    );
    shell.build()
}

/// One square sail, as cloth rather than as a board.
///
/// `set` is how much of it is there: one sheeted home, nought clewed up to the yard in a
/// bunt. In between is what you actually see from another boat — the clews coming up to
/// the yardarms and the belly going out of it — because a square sail is taken in from
/// its corners and not lowered.
fn square_sail(half_head: f32, half_foot: f32, drop: f32, set: f32, aback: bool) -> Mesh {
    const ACROSS: usize = 14;
    const DOWN: usize = 8;
    let mut shell = Shell::default();
    // Clewed up: the corners rise to the yardarms and the foot gathers under it.
    let hang = drop * (0.16 + 0.84 * set);
    let foot = half_foot * set + half_head * 0.42 * (1.0 - set);
    // Which way the cloth is pushed. Drawing, the wind is on the after side and the
    // belly goes forward; aback it is on the front of it and the whole sail is pressed
    // back against the mast, which is a thing anybody can see from a mile away.
    let push = if aback { 1.0 } else { -1.0 };
    let belly = (0.09 + 0.55 * set) * drop * 0.5;
    let at = |i: usize, j: usize| {
        let u = i as f32 / ACROSS as f32; // 0 at the port yardarm, 1 at the starboard
        let v = j as f32 / DOWN as f32; // 0 at the head, 1 at the foot
        let half = half_head + (foot - half_head) * v;
        let x = (u * 2.0 - 1.0) * half;
        // The head of a square sail is bent to the jackstay along the top of the yard,
        // so it is a straight line; everything below it is free.
        let bag = (std::f32::consts::PI * v.powf(0.85)).sin()
            * (std::f32::consts::PI * u).sin().powf(0.6);
        // And the foot scallops between the clews, because the only things holding it
        // are at the corners.
        let scallop = (1.0 - (u * 2.0 - 1.0).abs().powf(2.0)) * 0.1 * hang * set;
        Vec3::new(x, -0.18 - hang * v - scallop, push * belly * bag)
    };
    for i in 0..ACROSS {
        for j in 0..DOWN {
            shell.painted(
                [at(i, j), at(i, j + 1), at(i + 1, j + 1), at(i + 1, j)],
                [0.78, 0.74, 0.66, 1.0],
            );
        }
    }
    shell.build()
}

/// A fore-and-aft sail — the spanker, or a headsail — as a triangle or a quad of cloth.
fn gaff_or_jib(corners: [Vec3; 4], belly: f32, lee: f32) -> Mesh {
    const N: usize = 10;
    let mut shell = Shell::default();
    let at = |u: f32, v: f32| {
        let fore = corners[0].lerp(corners[1], v);
        let aft = corners[3].lerp(corners[2], v);
        let p = fore.lerp(aft, u);
        let bag = (std::f32::consts::PI * u).sin() * (std::f32::consts::PI * v).sin();
        p + Vec3::X * lee * belly * bag
    };
    for i in 0..N {
        for j in 0..N {
            let (u0, u1) = (i as f32 / N as f32, (i + 1) as f32 / N as f32);
            let (v0, v1) = (j as f32 / N as f32, (j + 1) as f32 / N as f32);
            shell.painted(
                [at(u0, v0), at(u0, v1), at(u1, v1), at(u1, v0)],
                [0.76, 0.72, 0.64, 1.0],
            );
        }
    }
    shell.build()
}

// ---------------------------------------------------------------------------------------
// Building her, and sailing her.
// ---------------------------------------------------------------------------------------

/// How long the braces take to swing the yards from square to hard up, seconds.
///
/// **Thirty seconds, and the number is the point.** Bracing a brig round is the whole
/// watch on a rope: it is why a square-rigger is sailed in long legs, why she wears
/// instead of tacking, and why a pilot who asks her to turn twice in a narrow place is
/// asking for something she cannot do. A yard that snapped round like a boom would make
/// her a yacht with the wrong sails on.
const BRACING: f64 = 30.0;

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    assets: Res<AssetServer>,
) {
    // One material for everything with its colour in the mesh — the hull's paintwork,
    // the tarred rigging, the canvas — and a second for the spars, which are varnished
    // wood and the one thing aboard that catches the light.
    let painted = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.82,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let spar = materials.add(StandardMaterial {
        base_color: Color::srgb(0.46, 0.33, 0.19),
        perceptual_roughness: 0.55,
        ..default()
    });
    let hull = meshes.add(hull_mesh());
    let rigging = meshes.add(rigging_mesh());

    // **Which hull she wears.**
    //
    // Her rig is not a model and never will be: the yards brace, the cloth bellies and
    // backs, and the whole of what makes her read as a square-rigger is that those
    // things move. What *can* come out of a file is the hull — a lofted shape with paint
    // on it is exactly the kind of object a generator is good at and the one place our
    // own is weakest, which is her transom. So the hull is a child rather than the root,
    // and a generated one takes its place without the rigging knowing:
    //
    //   SAIL_BRIG_HULL=brig.glb SAIL_BRIG_HULL_LOA=1.0 SAIL_BRIG_HULL_TURN=2 \
    //   SAIL_BRIG_HULL_SINK=-0.4
    //
    // Same four numbers as the boat's, for the same reason: all four are wrong on the
    // first look at any model, and a rebuild between each guess is the thing that makes
    // that a bad evening.
    let knob = |name: &str, fallback: f32| -> f32 {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(fallback)
    };
    let bought = std::env::var("SAIL_BRIG_HULL")
        .ok()
        .filter(|f| !f.is_empty());

    commands
        .spawn((Hull, Transform::default(), Visibility::Hidden))
        .with_children(|ship| {
            match &bought {
                Some(file) => {
                    ship.spawn((
                        bevy::world_serialization::WorldAssetRoot(
                            assets.load(GltfAssetLabel::Scene(0).from_asset(file.clone())),
                        ),
                        Transform::from_xyz(0.0, knob("SAIL_BRIG_HULL_SINK", 0.0), 0.0)
                            .with_rotation(Quat::from_rotation_y(
                                knob("SAIL_BRIG_HULL_TURN", 0.0) * std::f32::consts::FRAC_PI_2,
                            ))
                            .with_scale(Vec3::splat(LOA / knob("SAIL_BRIG_HULL_LOA", 1.0))),
                    ));
                }
                None => {
                    ship.spawn((
                        Mesh3d(hull),
                        MeshMaterial3d(painted.clone()),
                        Transform::default(),
                    ));
                }
            }
            ship.spawn((
                Mesh3d(rigging),
                MeshMaterial3d(painted.clone()),
                Transform::default(),
            ));
            // The bowsprit, which is a spar like any other and the reason she has any
            // head sails at all.
            let end = bowsprit_end();
            let stem = Vec3::new(0.0, deck_at(0.02) + 0.4, along(0.0));
            ship.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.26, (end - stem).length()))),
                MeshMaterial3d(spar.clone()),
                Transform::from_translation((stem + end) * 0.5)
                    .with_rotation(Quat::from_rotation_arc(Vec3::Y, (end - stem).normalize())),
            ));
            for mast in 0..2 {
                let foot = mast_foot(mast);
                // Lower mast, topmast, topgallant: three sticks of decreasing size, which
                // is what a mast *is* — nobody ever grew a tree thirty-five metres long.
                for (i, (len, r)) in [(LOWER_MAST, 0.34_f32), (TOPMAST, 0.23), (TOPGALLANT, 0.15)]
                    .into_iter()
                    .enumerate()
                {
                    let below: f32 = [0.0, LOWER_MAST, LOWER_MAST + TOPMAST][i];
                    ship.spawn((
                        Mesh3d(meshes.add(Cylinder::new(r, len))),
                        MeshMaterial3d(spar.clone()),
                        Transform::from_translation(foot + Vec3::Y * (below + len * 0.5)),
                    ));
                }
                // The top: a platform at the masthead that spreads the topmast shrouds
                // and that the watch stands on.
                ship.spawn((
                    Mesh3d(meshes.add(Cuboid::new(3.6, 0.16, 2.2))),
                    MeshMaterial3d(spar.clone()),
                    Transform::from_translation(foot + Vec3::Y * (LOWER_MAST - 0.4)),
                ));
                for (tier, (height, half)) in YARDS.iter().enumerate() {
                    let length = BEAM * half * 2.0;
                    ship.spawn((
                        Yard { mast, tier },
                        Transform::from_translation(foot + Vec3::Y * *height),
                        Visibility::default(),
                    ))
                    .with_children(|yard| {
                        yard.spawn((
                            Mesh3d(meshes.add(Cylinder::new(0.17 - tier as f32 * 0.03, length))),
                            MeshMaterial3d(spar.clone()),
                            Transform::from_rotation(Quat::from_rotation_z(
                                std::f32::consts::FRAC_PI_2,
                            )),
                        ));
                        yard.spawn((
                            Canvas { mast, tier },
                            Mesh3d(meshes.add(square_sail(
                                BEAM * half * 0.92,
                                BEAM * half,
                                8.0,
                                1.0,
                                false,
                            ))),
                            MeshMaterial3d(painted.clone()),
                            Transform::default(),
                        ));
                    });
                }
            }
            // The spanker: a gaff sail abaft the mainmast, and the sail she steers with.
            ship.spawn((
                Driver(0),
                Mesh3d(meshes.add(empty_cloth())),
                MeshMaterial3d(painted.clone()),
                Transform::default(),
            ));
            // Two headsails on the stays forward, which do the other half of the
            // steering: a brig balanced on her square sails alone will not hold a course.
            ship.spawn((
                Driver(1),
                Mesh3d(meshes.add(empty_cloth())),
                MeshMaterial3d(painted.clone()),
                Transform::default(),
            ));
        });
}

/// A mesh with nothing in it yet, for the sails that are rebuilt every frame.
fn empty_cloth() -> Mesh {
    Shell::default().build()
}

/// Where she is going and how fast, which is the whole of her sailing.
///
/// She is not simulated: she is *sailed*, the way a vessel you are looking at from
/// another boat is sailed — a course she is trying to make, a speed her rig will give
/// her on that course, and the one hard rule that she cannot lie closer than
/// [`CLOSEST`] to the wind and will bear away rather than stop.
pub fn sail_her(sim: Res<Sim>, time: Res<Time>, mut brig: ResMut<Brig>) {
    if !brig.afloat {
        return;
    }
    // `SAIL_BRIG_SHOW=130` pins her that many metres off the port bow and holds her
    // there, sailing our course. A drawing knob and nothing else: a ship that is being
    // looked at has to stay where it can be seen, and the alternative is twelve seconds
    // of sailing between every screenshot and the thing you wanted to look at.
    if let Some(range) = std::env::var("SAIL_BRIG_SHOW")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
    {
        let st = &sim.sim.state;
        // Ten degrees off the bow, not thirty-eight: the chase camera's field is
        // narrower than it looks and a ship pinned out on the beam sits outside every
        // view in the game, which is how a knob for *looking* at her managed to be a
        // knob that never showed her.
        let off = st.heading - rad(10.0);
        brig.at = st.pos + DVec2::new(off.sin(), off.cos()) * range;
        brig.heading = st.heading;
        brig.speed = kn_to_ms(5.0);
        let wind = her_wind(&sim, brig.at);
        let from = sail_core::wind::direction_of(wind);
        brig.braced = brace_for(angle_off(brig.heading, from));
        return;
    }
    let dt = time.delta_secs_f64();
    let wind = her_wind(&sim, brig.at);
    let from = sail_core::wind::direction_of(wind);
    let tws = wind.length();

    // The course she wants, and the course she can actually steer: a square-rigger
    // headed inside her own limit does not pinch, she falls off on to it.
    let want = {
        let off = brig.bound_for - brig.at;
        off.x.atan2(off.y)
    };
    let twa_wanted = angle_off(want, from);
    let steer = if twa_wanted.abs() < CLOSEST {
        // Pick whichever tack is nearer to where she is going, and lie as close as the
        // rig allows on it.
        let side = if twa_wanted.abs() < 1e-6 {
            if angle_off(brig.heading, from).is_sign_negative() {
                -1.0
            } else {
                1.0
            }
        } else {
            twa_wanted.signum()
        };
        course_for(from, side * CLOSEST)
    } else {
        want
    };
    // She comes round at a ship's rate, not a dinghy's.
    let error = wrap_pi(steer - brig.heading);
    let swing = rad(2.2) * dt;
    brig.heading = wrap_pi(brig.heading + error.clamp(-swing, swing));

    let twa = angle_off(brig.heading, from);
    let want_speed = through_the_water(twa, tws) * brig.set.iter().sum::<f32>() as f64 / 3.0;
    // Three hundred tons does not change speed quickly; a minute to pick up most of a
    // change is about right.
    brig.speed += (want_speed - brig.speed) * (1.0 - (-dt / 25.0).exp());

    // Apparent wind, for the yards. She is slow enough that it is not far off the true
    // one, but bracing to the true wind would be visibly wrong on a reach.
    let apparent = {
        let head = DVec2::new(brig.heading.sin(), brig.heading.cos());
        let flow = wind - head * brig.speed;
        angle_off(brig.heading, sail_core::wind::direction_of(flow))
    };
    let want_brace = brace_for(apparent);
    let haul = rad(50.0) / BRACING * dt;
    brig.braced += (want_brace - brig.braced).clamp(-haul, haul);

    // Heel: she is a deep, stiff, loaded hull and she does not lie over like a yacht,
    // but she leans, and the lean is how you read what the wind is doing to her.
    let press = (ms_to_kn(tws) / 25.0).min(1.4) * apparent.sin().abs();
    let want_heel = -apparent.signum() * rad(9.0) * press;
    brig.heel += (want_heel - brig.heel) * (1.0 - (-dt / 6.0).exp());

    // --- How she actually moves, which is not along her own head -------------------
    //
    // **Reported from the deck as sliding, and it was: she had no way of her own.** Her
    // position was stepped straight along her heading, so a turn moved her sideways the
    // instant the helm went over and she crabbed not at all — a ship on rails, which
    // from another boat looks exactly like skating.
    //
    // Two things were missing and they are the two things that make a vessel look like
    // one. The first is **way**: three hundred tons carry their velocity through a turn,
    // so her track lags her head and swings round to it over ten seconds or so, which is
    // what you are watching when a ship alters course and her wake bends behind her
    // rather than kinking. The second is **leeway**: nothing under sail goes where it is
    // pointed, and a square-rigger is the worst of all of them — six or seven degrees
    // close-hauled, which is most of why she cannot work to windward and is visible from
    // half a mile as the angle between her hull and her wake.
    let head = DVec2::new(brig.heading.sin(), brig.heading.cos());
    let lee = DVec2::new(head.y, -head.x) * twa.signum();
    // Most of it close-hauled, none of it running: the sideways force is what makes it
    // and there is none of that before the wind.
    let crab = rad(7.0) * (1.0 - smoothstep(rad(60.0), rad(140.0), twa.abs()));
    let want = (head * crab.cos() + lee * crab.sin()) * brig.speed;
    // A ship's way: ten seconds to come round to a new direction, which is the number
    // that turns a kink into a curve.
    let was = brig.way;
    brig.way = was + (want - was) * (1.0 - (-dt / 10.0).exp());
    let run = brig.way * dt;
    brig.at += run;
}

/// Put her on the water, at the attitude the sea gives her.
pub fn place(
    sim: Res<Sim>,
    origin: Res<frame::RenderOrigin>,
    brig: Res<Brig>,
    mut hull: Query<(&mut Transform, &mut Visibility), With<Hull>>,
) {
    let Ok((mut tr, mut show)) = hull.single_mut() else {
        return;
    };
    *show = if brig.afloat {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if !brig.afloat {
        return;
    }
    let t = sim.sim.state.time;
    let height = sim.sim.water.height(brig.at, t) as f32;
    let slope = sim.sim.water.slope(brig.at, t);
    tr.translation = origin.to_bevy(brig.at, height);
    // Her own attitude, and then the sea's: a long hull does not follow a short wave, so
    // the slope under her is taken at a fraction of its value.
    let sea = Quat::from_rotation_z(-slope.x as f32 * 0.35)
        * Quat::from_rotation_x(-slope.y as f32 * 0.35);
    tr.rotation = sea
        * Quat::from_rotation_y(-brig.heading as f32)
        * Quat::from_rotation_z(-brig.heel as f32);
}

/// Brace the yards and rebuild the cloth on them.
#[allow(clippy::too_many_arguments)]
pub fn work_her(
    sim: Res<Sim>,
    brig: Res<Brig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut yards: Query<(&Yard, &mut Transform)>,
    canvas: Query<(&Canvas, &Mesh3d)>,
    drivers: Query<(&Driver, &Mesh3d)>,
) {
    if !brig.afloat {
        return;
    }
    for (yard, mut tr) in &mut yards {
        // The after mast's yards are braced a touch less sharply than the fore's, which
        // is what a crew does: the main is in the fore's wind and opening it up gives it
        // air. It is also the single detail that makes a square-rigger read as being
        // sailed rather than as being posed.
        // The after mast's yards are braced a touch less sharply than the fore's, and
        // the upper yards a touch *more* than the lower ones — the fan a square-rigger
        // shows from astern on a reach. Both are what a crew does: the main is in the
        // fore's wind and opening it up gives it air, and the upper sails are in cleaner,
        // stronger wind that has been bent less by the hull.
        let ease = if yard.mast == 1 { 0.86 } else { 1.0 };
        let fan = 1.0 + yard.tier as f64 * 0.05;
        tr.rotation = Quat::from_rotation_y(-(brig.braced * ease * fan) as f32);
    }
    let wind = her_wind(&sim, brig.at);
    let from = sail_core::wind::direction_of(wind);
    let awa = angle_off(brig.heading, from);
    for (sail, mesh) in &canvas {
        let (height, half) = YARDS[sail.tier];
        let drop = match sail.tier {
            0 => 9.5,
            1 => 8.5,
            _ => 6.0,
        };
        let _ = height;
        // The main's course is the one sail a brig hauls up first when she wants to see
        // where she is going: it is the biggest, the lowest, and the one in the way.
        let set = brig.set[sail.tier]
            * if sail.mast == 1 && sail.tier == 0 {
                0.92
            } else {
                1.0
            };
        let backed = aback(awa, brig.braced);
        if let Some(mut mesh) = meshes.get_mut(&mesh.0) {
            *mesh = square_sail(BEAM * half * 0.92, BEAM * half, drop, set, backed);
        }
    }
    // The fore-and-aft sails, which are what she steers with. The spanker is a gaff sail
    // abaft the main, sheeted to leeward like any other; the headsails stand on the
    // stays forward. A brig under square sails alone is a boat with no rudder worth
    // having, which is why no brig has ever been rigged that way.
    let lee = if awa.is_sign_negative() { 1.0 } else { -1.0 };
    for (driver, mesh) in &drivers {
        let main = mast_foot(1);
        let cloth = if driver.0 == 0 {
            // Luff on the mast, head on the gaff, foot on the boom: the four corners a
            // gaff sail actually has.
            let throat = main + Vec3::new(0.0, 4.0, 0.3);
            let peak = main + Vec3::new(0.0, 11.5, 5.2);
            let tack = main + Vec3::new(0.0, 3.2, 0.5);
            let clew = main + Vec3::new(lee * 2.2, 3.9, 9.4);
            gaff_or_jib([throat, peak, clew, tack], 0.9, lee)
        } else {
            // Two headsails read as one piece of cloth at this distance: tack on the
            // bowsprit, head up the fore topmast stay, clew sheeted in on the rail.
            let end = bowsprit_end();
            let head = mast_foot(0) + Vec3::Y * (LOWER_MAST + TOPMAST * 0.55);
            let tack = end;
            let clew = Vec3::new(lee * BEAM * 0.42, deck_at(0.22) + 1.6, along(0.24));
            gaff_or_jib([tack, head, clew, clew], 0.7, lee)
        };
        if let Some(mut mesh) = meshes.get_mut(&mesh.0) {
            *mesh = cloth;
        }
    }
}
