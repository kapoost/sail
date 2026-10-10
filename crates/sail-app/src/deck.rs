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

/// Camber: a deck and a coachroof are both crowned, or they hold water.
fn camber(across: f32, half: f32) -> f32 {
    if half <= 1e-4 {
        return 0.0;
    }
    let u = (across / half).clamp(-1.0, 1.0);
    0.055 * half * (1.0 - u * u)
}

/// Build the whole structure above the sheer as one mesh.
pub fn structure_mesh() -> Mesh {
    let mut positions: Vec<Vec3> = Vec::new();
    let mut uvs: Vec<Vec2> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // Walk the hull in fine steps so the openings start and stop smoothly.
    let steps = 220;
    let z0 = SHEER[0].0;
    let z1 = SHEER[SHEER.len() - 1].0;

    // Each station contributes a ring of points: sheer edge, deck inner edge, then up
    // the cabin side (or down into the cockpit) and in across its top.
    let mut rings: Vec<Vec<Vec3>> = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let z = z0 + (z1 - z0) * (i as f32 / steps as f32);
        let (half, sheer) = sheer_at(z);
        let (inner, rise) = opening(z);
        let mut ring = Vec::with_capacity(9);
        // Starboard sheer, in across the side deck, up or down the opening, over the
        // top to the centreline. Mirrored on the way back out to port.
        for side in [1.0_f32, -1.0] {
            let x_out = side * half;
            let x_in = side * inner;
            ring.push(Vec3::new(x_out, sheer, z));
            ring.push(Vec3::new(x_in, sheer + camber(x_in, half) * 0.6, z));
            if inner > 1e-4 {
                ring.push(Vec3::new(x_in, sheer + rise, z));
                ring.push(Vec3::new(
                    x_in * 0.5,
                    sheer + rise + camber(x_in * 0.5, inner) * side.abs(),
                    z,
                ));
            }
            ring.push(Vec3::new(0.0, sheer + rise, z));
        }
        rings.push(ring);
    }

    // Stitch neighbouring rings. Rings only have the same length where the opening is
    // present or absent in both, so transitions are skipped — a seam of one step at
    // the ends of the cabin and the cockpit, which no one will find.
    for i in 0..steps {
        let (a, b) = (&rings[i], &rings[i + 1]);
        if a.len() != b.len() {
            continue;
        }
        let base_a = positions.len() as u32;
        for p in a {
            positions.push(*p);
            uvs.push(Vec2::new(p.x * 0.5 + 0.5, p.z * 0.25));
        }
        let base_b = positions.len() as u32;
        for p in b {
            positions.push(*p);
            uvs.push(Vec2::new(p.x * 0.5 + 0.5, p.z * 0.25));
        }
        for k in 0..(a.len() - 1) as u32 {
            indices.extend_from_slice(&[
                base_a + k,
                base_b + k,
                base_b + k + 1,
                base_a + k,
                base_b + k + 1,
                base_a + k + 1,
            ]);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh.compute_normals();
    mesh
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
    deck_height(2.6) - COCKPIT_DEPTH
}
