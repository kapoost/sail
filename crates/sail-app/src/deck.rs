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

/// Where the cockpit begins and ends.
///
/// There is no coachroof. She is flush-decked, which is right for this hull and is
/// what the first version should have been: a cabin trunk on a boat this fine is a
/// shed on a racing shell, it blocked the whole view forward from the tiller, and
/// everything that was on top of it belongs on the deck anyway. The accommodation is
/// under the deck, reached through a hatch, the way it is on a working boat.
const COCKPIT_FWD: f32 = 0.9;
const COCKPIT_AFT: f32 = 4.0;
/// The cockpit well: how much of the local half-beam it takes, and how deep it is.
const COCKPIT_WIDTH: f32 = 0.52;
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
    if z < COCKPIT_FWD || z > COCKPIT_AFT {
        return (0.0, 0.0);
    }
    // Eased in at both ends, so the well has rounded corners in plan rather than
    // stopping dead against a bulkhead.
    let t = ease((z - COCKPIT_FWD) / 0.8).min(ease((COCKPIT_AFT - z) / 0.8));
    (half * COCKPIT_WIDTH * t, -COCKPIT_DEPTH * t)
}

/// Where the bottom boards sit below the deck, metres.
///
/// Low enough that you are standing *in* the hull, high enough to stay dry.
///
/// This is the number that decides whether you see the sea inside the boat, and the
/// first two attempts both got it wrong by treating the problem as one of hiding.
/// Lining the inside did not help and could not: at a drop of 0.68 m the floor stood
/// only 0.18 m above mean water, while the sea the game draws has a significant height
/// of around 0.8 m, so its crests reach four tenths of a metre and genuinely come up
/// through the floor. No amount of surface in the way fixes a floor that is under the
/// water.
///
/// At 0.42 m of drop the boards sit 0.44 m up, two and a half times the clearance, and
/// the well is a footwell you sit beside rather than a hole you stand in — which on a
/// boat laid out like this is right anyway.
const SOLE_DROP: f32 = 0.42;

/// The section from the centreline out to the sheer, as `(x, y)` going outwards.
///
/// Returned rather than inlined because it is three different shapes depending on
/// where along the boat you cut — foredeck, coachroof, cockpit — and keeping them in
/// one list of points is what lets the stitching treat them all the same.
fn half_section(z: f32) -> Vec<(f32, f32)> {
    let (half, sheer) = sheer_at(z);
    let (inner, _) = opening(z);
    let mut out: Vec<(f32, f32)> = Vec::with_capacity(8);

    if inner <= 1e-4 {
        // Plain deck, closed to the centreline.
        out.push((0.0, sheer + camber(0.0, half)));
    } else {
        // Cockpit: a hole straight through the deck.
        //
        // There is no sole and no well. The deck simply stops, and what you are
        // standing in is the inside of the hull — the scanned planking, which is the
        // best surface on the boat and was being hidden under a fabricated floor. An
        // open boat with her ends decked over is what she is; this is what that looks
        // like from the tiller.
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
    let mut rings: Vec<(Vec<Vec3>, usize)> = Vec::with_capacity(steps + 1);
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
        // True where the section reaches the centreline, false where it is cut by the
        // cockpit opening — which has to be known, because a path that does not close
        // across the middle must not be stitched across it either. That bridging quad
        // is exactly the grey wall this file has already been caught by once.
        let closed = half_section[0].0 <= 1e-4;
        let mut ring = Vec::with_capacity(half_section.len() * 2);
        // `half_section` runs from the centreline outwards, so the port half is it
        // reversed and mirrored — sheer first, working in — and the starboard half is
        // it forwards. Written the other way round the path came out as centreline,
        // port sheer, starboard sheer: a scrambled order in which the recorded gap
        // index pointed at the wrong segment, so the deck was still laid straight
        // across the cockpit opening.
        for (x, y) in half_section.iter().rev() {
            ring.push(Vec3::new(-x, *y, z));
        }
        for (i, (x, y)) in half_section.iter().enumerate() {
            // On a closed section the centreline point has just been pushed.
            if i == 0 && closed {
                continue;
            }
            ring.push(Vec3::new(*x, *y, z));
        }
        let gap = if closed {
            usize::MAX
        } else {
            half_section.len() - 1
        };
        rings.push((ring, gap));
    }

    // Stitch neighbouring rings, sorting each quad by which way it faces. Rings only
    // have the same length where the opening is present or absent in both, so the
    // transitions are skipped — a seam of one step at the ends of the cabin and the
    // cockpit, which no one will find.
    let mut deck = Surface::default();
    let mut sides = Surface::default();
    for i in 0..steps {
        let ((a, gap_a), (b, gap_b)) = (&rings[i], &rings[i + 1]);
        if a.len() != b.len() {
            continue;
        }
        for k in 0..a.len() - 1 {
            // Do not lay deck across the cockpit opening.
            if k == *gap_a || k == *gap_b {
                continue;
            }
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

    /// A quad on a strip of trim, with the grain running *along* it.
    ///
    /// The field planking is addressed across the boat, which is right for planks laid
    /// fore and aft. A king plank and a covering board are the same timber turned the
    /// other way, so their texture has to turn with them or the grain runs across the
    /// board instead of down it.
    fn strip(&mut self, corners: [Vec3; 4], along: f32) {
        let base = self.positions.len() as u32;
        for (i, p) in corners.iter().enumerate() {
            self.positions.push(*p);
            let across = if i == 0 || i == 3 { 0.0 } else { 0.18 };
            self.uvs.push(Vec2::new(across, along));
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

/// Half-beam of the cockpit opening at a station, outside the coaming.
pub fn coaming_half_beam(z: f32) -> f32 {
    let (inner, _) = opening(z);
    if inner > 0.02 {
        inner + COAMING_THICK
    } else {
        inner
    }
}

/// The forward end of the cockpit, which is where its coaming runs across.
pub fn cockpit_fwd() -> f32 {
    COCKPIT_FWD
}

/// Height of the cockpit coaming above the deck, metres.
///
/// Low. The first one stood thirty centimetres proud, which on an eleven-metre flush
/// deck is a fence: it reads as something bolted on rather than as part of the boat,
/// and it was the only thing in the way of the view forward once the coachroof went.
const COAMING: f32 = 0.16;
/// And how thick it is.
///
/// It was a single ribbon of surface with no thickness at all, which is why it looked
/// like paper standing on edge. A coaming is a piece of timber: it has an inside
/// face, a top you can sit on, and an outside face, and the top is what the eye reads
/// to judge that it is wood rather than a line drawn on the deck.
const COAMING_THICK: f32 = 0.055;
/// Height of the toerail above the deck.
const TOERAIL: f32 = 0.11;
/// And how thick, inboard from the sheer. It sits on the deck's edge rather than
/// hanging outboard of the hull, because the hull is a scan and ends exactly where it
/// ends.
const TOERAIL_THICK: f32 = 0.06;

/// The toerail along the sheer, and the wooden coaming round the cockpit.
///
/// Both are a strip standing on a curve the file already knows, so both come out of
/// one routine. The toerail covers the joint between the scanned hull and the built
/// deck — a hard edge there is the single most obvious tell that the two were made
/// separately — and the coaming is what turns a hole in the deck into a cockpit.
///
/// The coaming runs across the forward end as well as down the sides, because that is
/// the piece you actually brace against and sit with your back to, and a well open at
/// the front is a footwell, not a cockpit.
pub fn trim_mesh() -> Mesh {
    let mut surface = Surface::default();
    let steps = 260;
    let z0 = SHEER[0].0;
    let z1 = SHEER[SHEER.len() - 1].0;

    let mut previous: Option<(Vec<Vec3>, Vec<Vec3>)> = None;
    for i in 0..=steps {
        let z = z0 + (z1 - z0) * (i as f32 / steps as f32);
        let (half, sheer) = sheer_at(z);
        let (inner, _) = opening(z);

        let mut low = Vec::new();
        let mut high = Vec::new();
        // Toerail, capping the sheer both sides. Built as a section with thickness —
        // inboard face, top, outboard face — rather than one ribbon standing on edge.
        // A rail you can see the top of is a piece of timber; a rail you cannot is a
        // line drawn along the deck, which is what it looked like.
        for side in [1.0_f32, -1.0] {
            let inner_face = side * (half - TOERAIL_THICK);
            let outer_face = side * half;
            low.push(Vec3::new(inner_face, sheer, z));
            high.push(Vec3::new(inner_face, sheer + TOERAIL, z));
            low.push(Vec3::new(inner_face, sheer + TOERAIL, z));
            high.push(Vec3::new(outer_face, sheer + TOERAIL, z));
            low.push(Vec3::new(outer_face, sheer + TOERAIL, z));
            high.push(Vec3::new(outer_face, sheer, z));
        }
        // Coaming, following the edge of the well all the way round — including
        // across the ends, where the eased opening carries it. Built as a real rim:
        // up the inside face, across the top, and down the outside.
        if inner > 0.02 {
            for side in [1.0_f32, -1.0] {
                let edge = side * inner;
                let out = side * (inner + COAMING_THICK);
                low.push(Vec3::new(edge, sheer, z));
                high.push(Vec3::new(edge, sheer + COAMING, z));
                low.push(Vec3::new(edge, sheer + COAMING, z));
                high.push(Vec3::new(out, sheer + COAMING, z));
                low.push(Vec3::new(out, sheer + COAMING, z));
                high.push(Vec3::new(out, sheer, z));
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

/// The inside of the cockpit: a floor over the bilge and the lining up to the deck.
///
/// A floor alone was not enough, and the reason is a sight line rather than a texture.
/// Boards wider than the opening hide the sea when you look *straight down*, but from
/// astern and above you see past their edge, down into the hull, and the sea surface
/// runs through her at the waterline — so there it was, inside the boat.
///
/// The answer is to close the surface. The lining runs from the edge of the boards
/// out and up to the deck's own edge, sloping the way a hull's topsides do, so there
/// is no line of sight into the hull at all: floor, lining, deck, in one unbroken
/// skin. That is also what a decked boat really has below her side decks.
///
/// Both ends look after themselves, because the opening eases to nothing there and
/// takes the floor and the lining with it.
pub fn sole_mesh() -> Mesh {
    let mut surface = Surface::default();
    let (z0, z1) = (COCKPIT_FWD - 0.5, COCKPIT_AFT + 0.5);
    let steps = 90;
    let mut previous: Option<Vec<Vec3>> = None;
    for i in 0..=steps {
        let z = z0 + (z1 - z0) * (i as f32 / steps as f32);
        let (_, sheer) = sheer_at(z);
        let inner = opening(z).0;
        let floor = sheer - SOLE_DROP;
        // Narrower than the opening, so the lining leans outboard going up.
        let boards = inner * 0.76;
        let section = vec![
            Vec3::new(-inner, sheer, z),
            Vec3::new(-boards, floor, z),
            Vec3::new(boards, floor, z),
            Vec3::new(inner, sheer, z),
        ];
        if let Some(p) = previous.take() {
            for k in 0..section.len() - 1 {
                surface.quad([p[k], section[k], section[k + 1], p[k + 1]], k == 1);
            }
        }
        previous = Some(section);
    }
    surface.build()
}

/// The king plank and the covering board.
///
/// A laid deck is not a field of parallel planks running off the edge of the boat. It
/// has a **king plank** down the centreline that the others are sprung against, and a
/// **covering board** following the sheer that they are cut into. Without them the
/// planking just meets itself at the centreline and runs off into the toerail, which
/// is what "the texture is broken down the middle" actually was: not a fault in the
/// texture, but the one timber on a deck whose job is to be there.
///
/// Laid as their own strips with their own texture coordinates — grain along the plank
/// rather than across it — because that is how they are cut, and because the change of
/// direction at the join is the thing the eye reads as carpentry.
pub fn deck_detail_mesh() -> Mesh {
    let mut surface = Surface::default();
    let steps = 220;
    let z0 = SHEER[0].0;
    let z1 = SHEER[SHEER.len() - 1].0;
    /// Half-width of the king plank, metres.
    const KING: f32 = 0.085;
    /// Width of the covering board, inboard from the toerail.
    const BOARD: f32 = 0.20;
    /// Proud of the deck by a shaving, so it never fights the planking for depth.
    const PROUD: f32 = 0.004;

    let mut previous: Option<(Option<[Vec3; 2]>, [Vec3; 4])> = None;
    for i in 0..=steps {
        let z = z0 + (z1 - z0) * (i as f32 / steps as f32);
        let (half, sheer) = sheer_at(z);
        let open = opening(z).0 > 1e-4;
        let y = sheer + camber(0.0, half) * 0.5 + PROUD;

        // The king plank only exists where the deck is closed across the middle.
        let king = (!open).then_some([Vec3::new(-KING, y, z), Vec3::new(KING, y, z)]);
        let board = [
            Vec3::new(-half, sheer + PROUD, z),
            Vec3::new(-(half - BOARD), sheer + PROUD, z),
            Vec3::new(half - BOARD, sheer + PROUD, z),
            Vec3::new(half, sheer + PROUD, z),
        ];
        if let Some((pk, pb)) = previous.take() {
            if let (Some(a), Some(b)) = (pk, king) {
                surface.strip([a[0], b[0], b[1], a[1]], z);
            }
            surface.strip([pb[0], board[0], board[1], pb[1]], z);
            surface.strip([pb[2], board[2], board[3], pb[3]], z);
        }
        previous = Some((king, board));
    }
    surface.build()
}
