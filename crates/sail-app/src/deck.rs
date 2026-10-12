//! The deck, the coachroof and the cockpit, built over the scanned hull.
//!
//! The hull is a photogrammetric scan of an open boat, so she arrives with no deck at
//! all: a bare shell you look down into, with the winches apparently floating over the
//! bilge. Everything above the sheer has to be built, and this is where it is built.
//!
//! Generated rather than modelled, for the same reason the sky and the water and the
//! sails are: the project owns no modelling tool, and a shape described in code can be
//! driven by measurements. The measurements here are the scan's own. Her sheer line
//! and the beam at it were read off the mesh station by station, so the deck lands on
//! the real gunwale rather than near it, and the coachroof follows a hull that curves
//! the way the hull actually curves.
//!
//! There is also a precedent, which is not a joke: this is what happened to the real
//! boat. "Venus" was an open fishing fourareen until 1964, when Paul Johnstone decked
//! her, gave her a tiny cabin, and sailed her across the Atlantic. Putting a coachroof
//! on her is not a liberty taken with the scan — it is the second half of her life.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

/// Her sheer, station by station: `(z, half-beam, height above the waterline)`.
///
/// Measured off the scan's vertices at half-metre stations, in the boat's own frame —
/// bow at -Z, which is where the waterline shape says it is. The hull is fine forward
/// and full aft, which is the only test that actually distinguishes the ends of a
/// double-ender; the stem being *lower* than the sternpost here is a quirk of this
/// boat and led the first attempt to put her bow at the wrong end.
///
/// The heights are smoothed. Raw, they jump a hand's breadth between neighbouring
/// stations, because the scan is of a real boat with thwarts, a rubbing strake and a
/// century of wear, and a deck laid on that reads as a mistake rather than as detail.
const SHEER: [(f32, f32, f32); 21] = [
    (-5.50, 0.16, 1.10),
    (-5.00, 0.26, 1.06),
    (-4.50, 0.42, 1.02),
    (-4.00, 0.75, 0.97),
    (-3.50, 1.00, 0.91),
    (-3.00, 1.18, 0.85),
    (-2.50, 1.35, 0.80),
    (-2.00, 1.47, 0.76),
    (-1.50, 1.56, 0.74),
    (-1.00, 1.63, 0.73),
    (-0.50, 1.68, 0.73),
    (0.00, 1.70, 0.74),
    (0.50, 1.70, 0.76),
    (1.00, 1.69, 0.79),
    (1.50, 1.65, 0.82),
    (2.00, 1.59, 0.86),
    (2.50, 1.53, 0.90),
    (3.00, 1.44, 0.95),
    (3.50, 1.32, 1.01),
    (4.50, 0.93, 1.16),
    (5.50, 0.29, 1.35),
];

/// Where the coachroof begins and ends, and where the cockpit does.
const CABIN_FWD: f32 = -3.1;
const CABIN_AFT: f32 = 0.9;
const COCKPIT_AFT: f32 = 4.0;
/// Coachroof height above the deck at its aft end, tapering forward.
/// Low. The first attempt stood 0.78 m above the deck, which on an eleven-metre boat
/// is a caravan: it blocked the view forward from the cockpit and made her look like
/// a motorsailer. A coachroof on a hull this fine is something you step over, not
/// something you walk round.
const CABIN_RISE_AFT: f32 = 0.46;
const CABIN_RISE_FWD: f32 = 0.24;
/// How much of the local half-beam the coachroof takes, leaving a side deck to walk on.
const CABIN_WIDTH: f32 = 0.56;
/// The cockpit well: narrower than the cabin, and sunk below the deck.
const COCKPIT_WIDTH: f32 = 0.46;
const COCKPIT_DEPTH: f32 = 0.62;

/// Linear interpolation along the sheer table.
fn sheer_at(z: f32) -> (f32, f32) {
    if z <= SHEER[0].0 {
        return (SHEER[0].1, SHEER[0].2);
    }
    for w in SHEER.windows(2) {
        if z <= w[1].0 {
            let t = (z - w[0].0) / (w[1].0 - w[0].0);
            return (
                w[0].1 + (w[1].1 - w[0].1) * t,
                w[0].2 + (w[1].2 - w[0].2) * t,
            );
        }
    }
    let last = SHEER[SHEER.len() - 1];
    (last.1, last.2)
}

/// Smoothstep, for shapes that have to start and stop without a crease.
fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How far inboard the deck runs at this station, and how high the structure inside
/// it stands. Returns `(inner half-beam, rise above the deck)`.
///
/// Forward of the coachroof and aft of the cockpit the deck closes to the centreline,
/// which is what makes a foredeck you could stand on to handle an anchor.
fn opening(z: f32) -> (f32, f32) {
    let (half, _) = sheer_at(z);
    if z < CABIN_FWD || z > COCKPIT_AFT {
        return (0.0, 0.0);
    }
    if z <= CABIN_AFT {
        // The coachroof: narrowing and dropping towards the bow, so the front of it
        // is a slope rather than a wall.
        let t = ease((z - CABIN_FWD) / 1.4);
        let along = (z - CABIN_FWD) / (CABIN_AFT - CABIN_FWD);
        let rise = CABIN_RISE_FWD + (CABIN_RISE_AFT - CABIN_RISE_FWD) * ease(along);
        (half * CABIN_WIDTH * t, rise * t)
    } else {
        // The cockpit, which goes down rather than up.
        let t = ease((COCKPIT_AFT - z) / 0.8);
        (half * COCKPIT_WIDTH * t, -COCKPIT_DEPTH * t)
    }
}

/// Radius of the coachroof's top corner, metres.
///
/// A cabin side meeting a cabin top at a right angle is a packing crate. Every boat
/// rounds it, partly because that is how a laminate or a bent plank wants to go and
/// partly because it is what you grab when the deck tips — a sharp edge there is
/// something you cut your hand on.
const CABIN_CORNER: f32 = 0.11;
/// Where the cockpit sole sits below the sheer, metres.
const SOLE_DROP: f32 = 0.62;

/// The section from the centreline out to the sheer, as `(x, y)` going outwards.
///
/// Returned rather than inlined because it is three different shapes depending on
/// where along the boat you cut — foredeck, coachroof, cockpit — and keeping them in
/// one list of points is what lets the stitching treat them all the same.
fn half_section(z: f32) -> Vec<(f32, f32)> {
    let (half, sheer) = sheer_at(z);
    let (inner, rise) = opening(z);
    let mut out: Vec<(f32, f32)> = Vec::with_capacity(10);

    if inner <= 1e-4 {
        // Plain deck, closed to the centreline.
        out.push((0.0, sheer + camber(0.0, half)));
    } else if rise > 0.0 {
        // Coachroof: crowned top, then a rounded corner down onto the side.
        let top = sheer + rise;
        out.push((0.0, top + camber(0.0, inner)));
        let flat = (inner - CABIN_CORNER).max(0.0);
        out.push((flat, top + camber(flat, inner)));
        // A quarter round, so the top rolls into the side instead of breaking at it.
        for k in 1..=3 {
            let a = (k as f32 / 4.0) * std::f32::consts::FRAC_PI_2;
            out.push((
                flat + CABIN_CORNER * a.sin(),
                top - CABIN_CORNER * (1.0 - a.cos()),
            ));
        }
        out.push((inner, sheer));
    } else {
        // Cockpit: a flat sole, vertical sides, and a lip at deck level.
        //
        // It used to be a shallow V meeting at the centreline, which is a bilge, not
        // a sole. You stand in a cockpit.
        let sole = sheer - SOLE_DROP;
        out.push((0.0, sole));
        out.push((inner * 0.92, sole));
        out.push((inner, sole + 0.06));
        out.push((inner, sheer));
    }
    out.push((half, sheer));
    out
}

/// Camber: a deck and a coachroof are both crowned, or they hold water.
fn camber(across: f32, half: f32) -> f32 {
    if half <= 1e-4 {
        return 0.0;
    }
    let u = (across / half).clamp(-1.0, 1.0);
    0.055 * half * (1.0 - u * u)
}

/// Build the structure above the sheer as two meshes: what you walk on, and what you
/// walk past.
///
/// Split by the direction each quad faces, because they want different things. A deck
/// is laid planking with the lay running fore and aft and the seams catching the
/// light across it; a cabin side is painted, glossier, and has no lay at all. One
/// material across both is what made the whole structure read as a single plastic
/// moulding.
///
/// The split also fixes the texture coordinates. They used to be a projection from
/// directly above, which on a vertical face barely changes with height — so any
/// texture put on the cabin sides would have smeared into vertical streaks. Each
/// mesh now gets coordinates in the plane it actually lies in, in metres, so a plank
/// is 62 mm wide wherever it is and nothing is stretched.
pub fn structure_mesh() -> (Mesh, Mesh) {
    // Walk the hull in fine steps so the openings start and stop smoothly.
    let steps = 220;
    let z0 = SHEER[0].0;
    let z1 = SHEER[SHEER.len() - 1].0;

    // Each station contributes a ring of points: sheer edge, deck inner edge, then up
    // the cabin side (or down into the cockpit) and in across its top.
    let mut rings: Vec<Vec<Vec3>> = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let z = z0 + (z1 - z0) * (i as f32 / steps as f32);
        // One continuous section across the boat: port sheer, in across the side
        // deck, up the cabin side or down into the cockpit, over the centreline and
        // back out to starboard.
        //
        // The first version built two half-sections — each running from a sheer to the
        // centreline — and laid them end to end. Stitching then joined the last point
        // of one half to the first point of the other, which are the centreline and
        // the *opposite* sheer: a single quad spanning the whole width of the boat,
        // across the cockpit, through the cabin and out the other side. From the helm
        // it was a grey wall filling the view. A cross-section has to be a path, not
        // two paths in a list.
        let half_section = half_section(z);
        let mut ring = Vec::with_capacity(half_section.len() * 2);
        for (i, (x, y)) in half_section.iter().enumerate() {
            // Skip the duplicated centreline point on the way back out.
            if i > 0 || *x > 1e-4 {
                ring.push(Vec3::new(-x, *y, z));
            }
        }
        for (x, y) in half_section.iter().rev() {
            if *x > 1e-4 {
                ring.push(Vec3::new(*x, *y, z));
            }
        }
        rings.push(ring);
    }

    // Stitch neighbouring rings, sorting each quad by which way it faces. Rings only
    // have the same length where the opening is present or absent in both, so the
    // transitions are skipped — a seam of one step at the ends of the cabin and the
    // cockpit, which no one will find.
    let mut deck = Surface::default();
    let mut sides = Surface::default();
    for i in 0..steps {
        let (a, b) = (&rings[i], &rings[i + 1]);
        if a.len() != b.len() {
            continue;
        }
        for k in 0..a.len() - 1 {
            let quad = [a[k], b[k], b[k + 1], a[k + 1]];
            let normal = (quad[1] - quad[0])
                .cross(quad[3] - quad[0])
                .normalize_or_zero();
            let target = if normal.y.abs() < 0.5 {
                &mut sides
            } else {
                &mut deck
            };
            target.quad(quad, normal.y.abs() >= 0.5);
        }
    }
    (deck.build(), sides.build())
}

/// A mesh under construction, with its own texture coordinates.
#[derive(Default)]
struct Surface {
    positions: Vec<Vec3>,
    uvs: Vec<Vec2>,
    indices: Vec<u32>,
}

impl Surface {
    fn quad(&mut self, corners: [Vec3; 4], horizontal: bool) {
        let base = self.positions.len() as u32;
        for p in corners {
            self.positions.push(p);
            // Metres, in the plane the face lies in. A deck is read across and along
            // the boat; a cabin side along the boat and up it.
            self.uvs.push(if horizontal {
                Vec2::new(p.x, p.z)
            } else {
                Vec2::new(p.z, p.y)
            });
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
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh.compute_normals();
        mesh
    }
}

/// Height of the deck at a point, for putting fittings on it rather than near it.
pub fn deck_height(z: f32) -> f32 {
    sheer_at(z).1
}

/// Half-beam at the sheer, for putting the headsail tracks on the side deck.
pub fn deck_half_beam(z: f32) -> f32 {
    sheer_at(z).0
}

/// Where the cockpit sole is, which is where the helmsman is standing.
pub fn cockpit_sole() -> f32 {
    deck_height(2.6) - SOLE_DROP
}

/// Where the aft face of the coachroof is.
pub fn cabin_aft() -> f32 {
    CABIN_AFT
}

/// Height of the coachroof at a station, for putting things on top of it.
pub fn cabin_top(z: f32) -> f32 {
    let (_, rise) = opening(z);
    sheer_at(z).1 + rise.max(0.0)
}

/// Half-beam of the coachroof at a station.
pub fn cabin_half_beam(z: f32) -> f32 {
    opening(z).0
}

/// Height of the cockpit coaming above the deck, metres.
///
/// Low. It is something to brace a foot against and to sit on with your back to, not
/// a bulwark — and on a boat this fine a tall one would look like an afterthought
/// bolted to a hull that never asked for it.
const COAMING: f32 = 0.24;
/// Height of the toerail above the deck.
const TOERAIL: f32 = 0.09;

/// The toerail along the sheer, and the coaming round the cockpit.
///
/// Both are a strip standing on a curve the file already knows, so both come out of
/// one routine. The toerail is what covers the joint between the scanned hull and the
/// built deck — a hard edge there is the single most obvious tell that the two were
/// made separately — and the coaming is what turns a hole in the deck into a cockpit.
pub fn trim_mesh() -> Mesh {
    let mut surface = Surface::default();
    let steps = 240;
    let z0 = SHEER[0].0;
    let z1 = SHEER[SHEER.len() - 1].0;

    let mut previous: Option<(Vec<Vec3>, Vec<Vec3>)> = None;
    for i in 0..=steps {
        let z = z0 + (z1 - z0) * (i as f32 / steps as f32);
        let (half, sheer) = sheer_at(z);
        let (inner, rise) = opening(z);

        // Toerail: a capping on the sheer, both sides.
        let mut low = Vec::new();
        let mut high = Vec::new();
        for side in [1.0_f32, -1.0] {
            low.push(Vec3::new(side * half, sheer, z));
            high.push(Vec3::new(side * half, sheer + TOERAIL, z));
        }
        // Coaming: only where the cockpit is, and only on its outer edge.
        if z > CABIN_AFT && z < COCKPIT_AFT && inner > 0.05 && rise < 0.0 {
            for side in [1.0_f32, -1.0] {
                low.push(Vec3::new(side * inner, sheer, z));
                high.push(Vec3::new(side * inner, sheer + COAMING, z));
            }
        }
        if let Some((plow, phigh)) = previous.take() {
            if plow.len() == low.len() {
                for k in 0..low.len() {
                    surface.quad([plow[k], low[k], high[k], phigh[k]], false);
                }
            }
        }
        previous = Some((low, high));
    }
    surface.build()
}
