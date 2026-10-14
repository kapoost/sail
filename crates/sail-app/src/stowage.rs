//! What is on deck when it is not being used.
//!
//! A boat's foredeck is not empty. The sails that are not set are in bags lashed to the
//! rail, the pole that is not in use is lying along the side deck in its chocks, and
//! between them they are most of what tells you, from forty metres away, what kind of
//! boat she is and what she is rigged for. A deck with nothing on it reads as a model.
//!
//! All of it is derived from one fact — which sail is up — so there is nothing here to
//! keep in step with anything: the bag for the sail that is set is simply not there,
//! because its cloth is up the rig.

use bevy::prelude::*;

use crate::boat::MAST_FOOT_Z;
use crate::sim::Sim;
use sail_core::state::Headsail;

/// A bag on deck, and which sail is in it.
#[derive(Component)]
pub struct SailBag(pub Headsail);

/// The pole in its chocks, which is a different thing from the pole on the mast.
#[derive(Component)]
pub struct StowedPole;

/// The one thing about a bag that is not its shape: what has been done to it.
///
/// Four identical sausages in four colours read as inventory. What makes a real foredeck
/// legible is that every bag on it has a history — the one that gets stood on is worn
/// through at the turn, the one that was torn has a square of new cloth sewn over it,
/// the furled sail's bag is a zip from end to end because that is the only way to get a
/// furled sail out of it.
enum Feature {
    /// Worn through where the crew stands on it, going aboard, every time.
    Chafe,
    /// A long zip and a row of ties: a snake bag, for a sail that stays furled.
    Zip,
    /// A square of newer cloth sewn over a tear, stitching and all.
    Patch,
    /// A drawcord mouth with eyelets: a spinnaker turtle, not a duffle.
    Collar,
}

/// What is in the bag, which is the whole reason they are not all the same.
///
/// A sail bag is the sail's shape once it is off the boat, and the four sails this boat
/// carries are not remotely alike. The genoa is heavy dacron: a big firm roll that holds
/// its own shape and barely sags. The code 0 lives furled on its own luff rope, so it is
/// a two-metre snake of a thing, thin as a leg. The gennaker and the spinnaker are nylon
/// — half a kilo of cloth that packs down to nothing and flows into whatever it is put
/// in, which is why the kite travels in a squat turtle rather than a duffle at all.
struct Bagged {
    /// Half-girth of the roll.
    radius: f32,
    /// And how long the sausage is, not counting its ends.
    length: f32,
    /// How much it spreads under its own weight: dacron holds round, nylon does not.
    spread: f32,
    /// And flattens, which is the same fact seen from the side.
    flat: f32,
    /// How crumpled: a hard-rolled dacron genoa is nearly smooth, a nylon kite stuffed
    /// in by hand is nothing but creases.
    crease: f32,
    /// Which dead sail this bag was cut from, as a shade: they do not match.
    tint: f32,
    /// Where the hash starts, so no two bags crumple the same way.
    seed: f32,
    feature: Feature,
}

fn bagged(which: Headsail) -> Bagged {
    match which {
        // Heavy dacron, rolled on the dock by two people, and the one in daily use.
        Headsail::Genoa => Bagged {
            radius: 0.32,
            length: 1.05,
            spread: 1.08,
            flat: 0.84,
            crease: 0.55,
            tint: 0.94,
            seed: 0.0,
            feature: Feature::Chafe,
        },
        // Furled on its luff rope and never unrolled on deck, so: a snake.
        Headsail::Code0 => Bagged {
            radius: 0.19,
            length: 1.35,
            spread: 1.02,
            flat: 0.92,
            crease: 0.35,
            tint: 1.04,
            seed: 3.1,
            feature: Feature::Zip,
        },
        // Nylon, soft, and this one has been round the headstay the hard way once.
        Headsail::Gennaker => Bagged {
            radius: 0.27,
            length: 0.80,
            spread: 1.26,
            flat: 0.62,
            crease: 1.05,
            tint: 0.99,
            seed: 7.7,
            feature: Feature::Patch,
        },
        // A turtle: squat, round, drawcord mouth, and the lightest thing on the boat.
        Headsail::Spinnaker => Bagged {
            radius: 0.30,
            length: 0.50,
            spread: 1.34,
            flat: 0.58,
            crease: 1.25,
            tint: 1.01,
            seed: 11.3,
            feature: Feature::Collar,
        },
    }
}

/// Where each bag lies, and which side.
///
/// Three facts decide this and none of them is taste. The port side deck forward of the
/// mast is taken: the pole is in its chocks there from a foot forward of the mast nearly
/// to the stem, so nothing else goes on it. Forward of about two and three quarter metres
/// ahead of amidships the sheer closes in until the side deck is a hand's breadth wide,
/// and a bag put there is inside the stem. And aft of the companionway is the cockpit.
///
/// What is left is two side decks about three metres long, and four bags that between
/// them are five. So the two heavy sails go to starboard — the genoa by the hatch, where
/// you want the sail you hoist most, and the long thin code 0 forward of it — and the two
/// nylon ones go to port abaft the pole chocks, with the kite nearest the hatch for the
/// same reason.
fn berth(which: Headsail) -> (f32, f32) {
    match which {
        // (z along the boat — more negative is further forward — and which side)
        Headsail::Genoa => (-0.03, 1.0),
        Headsail::Code0 => (-1.88, 1.0),
        Headsail::Gennaker => (-1.32, -1.0),
        Headsail::Spinnaker => (0.0, -1.0),
    }
}

/// The band on the bag, which is how anybody tells them apart on a foredeck.
///
/// The bag itself is not a colour anybody chose: a sail bag is made out of a sail. When a
/// working jib is finally past repair it goes to the loft and comes back as bags, which is
/// why every bag on every cruising boat is the same tired cream as every other, and why
/// the only thing that names them is a band of tape and a stencil.
fn band(which: Headsail) -> [u8; 3] {
    match which {
        Headsail::Genoa => [38, 44, 62],
        Headsail::Code0 => [26, 96, 128],
        Headsail::Gennaker => [168, 72, 28],
        Headsail::Spinnaker => [190, 150, 24],
    }
}

/// Worn canvas: cream gone grey, which is what a bag made out of a dead sail looks like
/// after a few seasons in a cockpit locker.
const CANVAS: [f32; 3] = [0.72, 0.69, 0.60];

/// A duffle with a sail in it, which is not a cylinder.
///
/// Reported from the deck: too regular, should be flattened, crumpled, not that stiff,
/// and lying slightly bent. All of which is one observation — canvas has no opinions.
/// A bag with thirty kilos of wet sailcloth in it spreads under its own weight, takes
/// the camber of the deck it is lying on, and is covered in the creases of however it
/// was stuffed; the only straight thing about it is the tape round its mouth.
///
/// Built by taking the engine's capsule and ruining it: squashed, bent along its length
/// to lie into the deck, and every vertex pushed about by a cheap hash of its own
/// position so the surface is lumpy in a way that does not repeat. The hash takes the
/// bag's own seed, so the four on this deck crumple four different ways while each one
/// stays the same shape from frame to frame.
///
/// `lean` is which way the deck falls away, so the bag sags inboard rather than always
/// towards the same beam.
///
/// And it is pressed down onto the deck between its two ends. A bag is not a plank: the
/// deck under it dips a little amidships and lifts towards the bow, and canvas with
/// thirty kilos in it takes that shape rather than bridging it. Before this, the bag lay
/// along the straight line between its ends and stood a clear four centimetres off the
/// deck in the middle of the dip — found by the arithmetic below, not by eye, which is
/// the first time that has happened with these bags.
fn duffle(spec: &Bagged, lean: f32, aft: Vec3, fwd: Vec3) -> Mesh {
    // Ringed along its length, which the engine's capsule is not: by default the barrel
    // of it has vertices only where it meets the two ends, so there was nothing in the
    // middle of the bag to crumple or to press onto the deck. Everything below was acting
    // on four rings at the extremities and the bag between them stayed a smooth tube —
    // which is exactly how it read, and why the conforming appeared not to work at all.
    let mut mesh = Capsule3d::new(spec.radius, spec.length)
        .mesh()
        .rings(9)
        .longitudes(26)
        .latitudes(10)
        .build();
    let Some(bevy::render::mesh::VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    else {
        return mesh;
    };
    let half = spec.length * 0.5 + spec.radius;
    for p in positions.iter_mut() {
        let (x, y, z) = (p[0], p[1], p[2]);
        // Spread and flattened: a bag of cloth is wider than it is tall, and how much
        // wider is a fact about the cloth.
        let mut nx = x * spec.spread;
        let mut nz = z * spec.flat;
        // And bent, so it lies along a deck rather than across it, sagging the way the
        // deck falls.
        let along = (y / half).clamp(-1.0, 1.0);
        nx += along * along * 0.07 * spec.radius / 0.3 * lean;
        // Creases. A hash of the position, which gives the same lumps every time without
        // anything to store and without a pattern the eye can pick up.
        let hash = |a: f32, b: f32, c: f32| {
            let v = (a * 12.9898 + b * 78.233 + c * 37.719 + spec.seed * 5.431).sin() * 43758.547;
            v - v.floor() - 0.5
        };
        let r = (nx * nx + nz * nz).sqrt().max(1e-4);
        let crease = (hash(x * 9.0, y * 11.0, z * 7.0) * 0.055
            + hash(x * 23.0, y * 19.0, z * 29.0) * 0.022)
            * spec.crease;
        nx += nx / r * crease;
        nz += nz / r * crease;
        let ny = y + hash(x * 5.0, y * 13.0, z * 3.0) * 0.02 * spec.crease;
        // Pressed onto the deck. The axis runs from one end to the other, so a vertex at
        // `along` is over a known point of the sheer; the difference between the deck
        // there and the straight line between the ends is how far the cloth has to go
        // down. Laying the bag over maps local `+z` onto world *up* — which is the
        // opposite of what it looks like it should be, and is why the first attempt
        // doubled the gap it was supposed to close instead of closing it. The test below
        // measures the drawn vertices, so it said so.
        let t = (along + 1.0) * 0.5;
        let over = aft.z + (fwd.z - aft.z) * t;
        let chord = aft.y + (fwd.y - aft.y) * t;
        let conform = crate::deck::deck_height(over) - chord;
        // And nothing goes through the deck. A crease deep enough to put the cloth below
        // the planking is flattened against it instead, which is what the deck does to a
        // bag resting on it: the underside is the only smooth part of a sail bag.
        let floor = conform - (spec.radius * spec.flat + 0.006);
        *p = [nx, ny, (nz + conform).max(floor)];
    }
    mesh.compute_normals();
    mesh
}

/// The markings on a bag: tape up both sides for the handles, a binding round each end,
/// a band of the sail's own colour at the mouth, and then whatever has happened to this
/// particular bag.
///
/// Drawn into a small texture rather than modelled, because that is what they are — sewn
/// tape and a stencil — and because a boat carries four of these and nobody is going to
/// look at them closely enough to want geometry.
fn markings(spec: &Bagged, band_colour: [u8; 3]) -> Image {
    use bevy::asset::RenderAssetUsages;
    use bevy::image::Image;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    const W: usize = 64;
    const H: usize = 64;
    let mut data = vec![255u8; W * H * 4];
    let mut put = |x: usize, y: usize, v: [u8; 3]| {
        let i = (y * W + x) * 4;
        data[i] = v[0];
        data[i + 1] = v[1];
        data[i + 2] = v[2];
    };
    // A capsule's texture runs `u` round the girth and `v` along the length, so a stripe
    // at constant `u` is a tape down the length of the bag and one at constant `v` is a
    // binding round it. Which is exactly how a sail bag is made: webbing up each side for
    // the handles, a band round each end where the cloth is turned and sewn, and a strip
    // of tape at the mouth with the sail's name written on it in marker.
    //
    // Reported as missing, and they were: the first stencil had a hairline seam and a
    // block of colour, which at the size a bag is drawn reads as a dirty patch.
    let tape = [96, 92, 84];
    let stitch = [64, 61, 56];
    let worn = [132, 124, 108];
    let newer = [196, 190, 176];
    for y in 0..H {
        for x in 0..W {
            let on_tape = |at: usize| x.abs_diff(at) <= 2;
            let stitched = |at: usize| x.abs_diff(at) == 3;
            let handle = on_tape(16) || on_tape(48);
            let handle_stitch = stitched(16) || stitched(48);
            let binding = (2..5).contains(&y) || (H - 5..H - 2).contains(&y);
            let label = (8..15).contains(&y);
            if handle {
                put(x, y, tape);
            } else if handle_stitch || binding {
                put(x, y, stitch);
            } else if label {
                put(x, y, band_colour);
            }
        }
    }
    // And then the one thing that is true of this bag and no other.
    match spec.feature {
        // Worn through on the upper turn, where the crew stands on it going forward. A
        // smudge rather than an edge, because that is how canvas goes: the weave opens
        // before it tears.
        Feature::Chafe => {
            for y in 0..H {
                for x in 0..W {
                    let du = (x as f32 - 32.0) / 11.0;
                    let dv = (y as f32 - 40.0) / 7.0;
                    let d = du * du + dv * dv;
                    if d < 1.0 {
                        let t = (1.0 - d).sqrt() * 0.85;
                        put(
                            x,
                            y,
                            [0, 1, 2]
                                .map(|c| (255.0 * (1.0 - t) + worn[c] as f32 * t).round() as u8),
                        );
                    }
                }
            }
        }
        // A zip the whole length with its teeth, and ties across it: a snake bag only
        // opens along its length, because a furled sail cannot be lifted out of an end.
        Feature::Zip => {
            for y in 0..H {
                for x in 0..W {
                    if x.abs_diff(32) <= 1 {
                        put(x, y, stitch);
                    } else if x.abs_diff(32) == 2 && y % 2 == 0 {
                        put(x, y, tape);
                    }
                }
            }
            for v in [14usize, 32, 50] {
                for x in 0..W {
                    put(x, v, tape);
                    put(x, v + 1, stitch);
                }
            }
        }
        // A square of newer cloth sewn over a tear, which is what happens to a nylon
        // sail that has been round the headstay once.
        Feature::Patch => {
            for y in 26..42 {
                for x in 38..54 {
                    let edge = y == 26 || y == 41 || x == 38 || x == 53;
                    put(x, y, if edge { stitch } else { newer });
                }
            }
        }
        // A drawcord mouth with its eyelets and the tails hanging: a turtle is closed by
        // pulling a string, not by a zip, which is the whole point of it.
        Feature::Collar => {
            for y in 0..H {
                for x in 0..W {
                    if (17..24).contains(&y) {
                        put(x, y, tape);
                    }
                    if y == 20 && x % 8 == 4 {
                        put(x, y, stitch);
                    }
                    if (24..30).contains(&y) && x.abs_diff(28) <= 1 {
                        put(x, y, stitch);
                    }
                }
            }
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: W as u32,
            height: H as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = bevy::image::ImageSampler::linear();
    image
}

/// Where a bag actually sits: the two ends of it on deck, and how wide it is.
///
/// Beside the drawing rather than inside it, because the test at the bottom of this file
/// asks the same question — twice now the bags have been put somewhere impossible and
/// twice it took somebody looking at the boat to notice. The first time they were in the
/// stem, where the half-beam is a quarter of a metre and a bag is simply inside the hull;
/// the second time one was lying through the stowed pole. Neither is visible from the
/// code and both are arithmetic.
///
/// Laid along the sheer rather than parallel to the centreline, which matters most for
/// the long one: a two-metre bag put down at a constant offset from the middle is inboard
/// at one end and over the water at the other, because the side deck narrows towards the
/// bow. Both ends are set in from the gunwale by the same hand's breadth and the bag
/// takes the angle between them — which is what a crew does without thinking about it.
fn footprint(which: Headsail) -> (Vec3, Vec3, f32) {
    let spec = bagged(which);
    let (z, side) = berth(which);
    let half = spec.length * 0.5 + spec.radius;
    let wide = spec.radius * spec.spread;
    let end = |at: f32| {
        Vec3::new(
            side * (crate::deck::deck_half_beam(at) - wide - 0.09),
            crate::deck::deck_height(at),
            at,
        )
    };
    let (aft, fwd) = (end(z + half), end(z - half));
    // And a few degrees off square, because nobody puts a bag down straight. Turned about
    // its middle, so both ends move and neither goes over the side.
    let middle = (aft + fwd) * 0.5;
    let skew = Quat::from_rotation_y(side * 0.05);
    (
        middle + skew * (aft - middle),
        middle + skew * (fwd - middle),
        wide,
    )
}

/// The bag as it is actually drawn: conformed to the deck it is lying on, and laid along
/// the line between its two ends with the rise of the deck included.
fn bag_mesh(which: Headsail) -> Mesh {
    let (aft, fwd, _) = footprint(which);
    duffle(&bagged(which), -berth(which).1, aft, fwd)
}

/// And where it is put: a capsule stands up the Y axis, so it is laid over to run between
/// its two ends, and lifted off the line by its own flattened radius so that it sits on
/// the deck instead of half inside it.
fn placement(which: Headsail) -> Transform {
    let spec = bagged(which);
    let (aft, fwd, _) = footprint(which);
    let along = (fwd - aft).normalize_or_zero();
    Transform::from_translation((aft + fwd) * 0.5 + Vec3::Y * (spec.radius * spec.flat + 0.01))
        .with_rotation(Quat::from_rotation_arc(Vec3::Y, along))
}

/// The pole's chocks, which are the other thing on the side deck.
fn chocks(loa: f32) -> (f32, f32) {
    let aft = MAST_FOOT_Z - 0.3;
    (aft, aft - crate::boat::pole_length(loa))
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    sim: Res<Sim>,
    boat: Query<Entity, With<crate::boat::BoatRoot>>,
) {
    let Ok(root) = boat.single() else {
        warn!("no boat to stow anything on");
        return;
    };
    for which in [
        Headsail::Genoa,
        Headsail::Code0,
        Headsail::Gennaker,
        Headsail::Spinnaker,
    ] {
        let spec = bagged(which);
        commands.entity(root).with_children(|b| {
            b.spawn((
                SailBag(which),
                Mesh3d(meshes.add(bag_mesh(which))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(
                        CANVAS[0] * spec.tint,
                        CANVAS[1] * spec.tint,
                        CANVAS[2] * spec.tint,
                    ),
                    base_color_texture: Some(images.add(markings(&spec, band(which)))),
                    perceptual_roughness: 0.95,
                    ..default()
                })),
                placement(which),
                Visibility::default(),
            ));
        });
    }

    for which in [Headsail::Genoa, Headsail::Spinnaker] {
        let spec = bagged(which);
        let (z, side) = berth(which);
        info!(
            "stowed {:?}: z {:.2}, side {:+.0}, {:.2} x {:.2} m",
            which,
            z,
            side,
            spec.radius * 2.0 * spec.spread,
            spec.length + spec.radius * 2.0
        );
    }
    // And the pole in its chocks along the port side deck.
    //
    // Laid *along the sheer* for the same reason the bags are, and more so: it is three
    // metres of spar and a constant offset puts its forward end over the water.
    let loa = sim.config().loa as f32;
    let length = crate::boat::pole_length(loa);
    let (aft, forward) = chocks(loa);
    const INSET: f32 = 0.34;
    let end = |z: f32| {
        Vec3::new(
            -(crate::deck::deck_half_beam(z) - INSET),
            crate::deck::deck_height(z) + 0.10,
            z,
        )
    };
    let (chock_aft, chock_fwd) = (end(aft), end(forward));
    let along = (chock_fwd - chock_aft).normalize_or_zero();
    commands.entity(root).with_children(|b| {
        b.spawn((
            StowedPole,
            Mesh3d(meshes.add(Cylinder::new(0.05, length))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.52, 0.38, 0.21),
                perceptual_roughness: 0.22,
                ..default()
            })),
            Transform::from_translation((chock_aft + chock_fwd) * 0.5)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, along)),
            Visibility::default(),
        ));
    });
}

/// A bag is on deck when its sail is not up, and the pole is in its chocks when the kite
/// is not the sail of the day. Which is one fact each, read from the same place the rig
/// reads it.
pub fn update(
    sim: Res<Sim>,
    mut bags: Query<(&SailBag, &mut Visibility), Without<StowedPole>>,
    mut pole: Query<&mut Visibility, With<StowedPole>>,
) {
    let up = sim.controls.headsail;
    for (bag, mut vis) in &mut bags {
        *vis = if bag.0 == up {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    let on_the_mast = up == Headsail::Spinnaker;
    for mut vis in &mut pole {
        *vis = if on_the_mast {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAILS: [Headsail; 4] = [
        Headsail::Genoa,
        Headsail::Code0,
        Headsail::Gennaker,
        Headsail::Spinnaker,
    ];

    /// Where the drawn bag actually is, vertex by vertex, in the boat's own frame.
    ///
    /// The tests below ask about the bag that is on the boat rather than about the
    /// numbers it was built from, because every fault these bags have had was in the step
    /// between the two: berths three metres forward of the mast where the hull is a
    /// hand's breadth wide, a bag lying through the stowed pole, a straight bag bridging
    /// a dip in the deck. None of those is visible in the table and all of them are
    /// arithmetic.
    fn drawn(which: Headsail) -> Vec<Vec3> {
        let mesh = bag_mesh(which);
        let at = placement(which);
        let Some(bevy::render::mesh::VertexAttributeValues::Float32x3(p)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("a bag with no vertices");
        };
        p.iter()
            .map(|v| at.transform_point(Vec3::from_array(*v)))
            .collect()
    }

    /// What a bag occupies along the boat, which is what two bags must not share.
    fn span(which: Headsail) -> (f32, f32) {
        let v = drawn(which);
        (
            v.iter().map(|p| p.z).fold(f32::MAX, f32::min),
            v.iter().map(|p| p.z).fold(f32::MIN, f32::max),
        )
    }

    /// Every bag is on the deck: inboard of the gunwale over its whole length, outboard
    /// of the centreline, and forward of the cockpit.
    ///
    /// The first berths put two of them three metres forward of the mast, where this hull
    /// is a quarter of a metre wide at the sheer — so the bags were inside the stem and
    /// invisible, and nothing in the repository could say so. This can: the sheer is a
    /// function and it can simply be asked.
    #[test]
    fn no_bag_hangs_over_the_water() {
        for which in SAILS {
            for p in drawn(which) {
                let rail = crate::deck::deck_half_beam(p.z);
                assert!(
                    p.x.abs() <= rail + 1e-3,
                    "{which:?} has cloth {:.2} m out at z {:.2} and the deck is {rail:.2} m",
                    p.x.abs(),
                    p.z
                );
                assert!(
                    p.x.abs() > 0.05,
                    "{which:?} is over the centreline at z {:.2}",
                    p.z
                );
                assert!(
                    p.z < crate::deck::cockpit_fwd(),
                    "{which:?} reaches aft into the cockpit, to z {:.2}",
                    p.z
                );
            }
        }
    }

    /// Two bags on the same side deck do not lie through each other, and neither lies
    /// through the pole.
    ///
    /// The pole is in its chocks along the port side from the mast nearly to the stem, so
    /// the whole of the port foredeck is taken — which is why the dacron sails are to
    /// starboard. One of the bags was inside the spar before this test existed.
    #[test]
    fn nothing_stowed_lies_through_anything_else() {
        for a in SAILS {
            for b in SAILS {
                if a == b || berth(a).1 != berth(b).1 {
                    continue;
                }
                let (a_fwd, a_aft) = span(a);
                let (b_fwd, b_aft) = span(b);
                assert!(
                    a_aft < b_fwd || b_aft < a_fwd,
                    "{a:?} ({a_fwd:.2}..{a_aft:.2}) and {b:?} ({b_fwd:.2}..{b_aft:.2}) \
                     are on the same side deck and overlap"
                );
            }
        }
        let loa = sail_core::boat::BoatConfig::charter_sloop_34().loa as f32;
        let (pole_aft, pole_fwd) = chocks(loa);
        for which in SAILS {
            if berth(which).1 > 0.0 {
                continue;
            }
            let (fwd, aft) = span(which);
            assert!(
                aft < pole_fwd || pole_aft < fwd,
                "{which:?} ({fwd:.2}..{aft:.2}) is lying through the pole \
                 ({pole_fwd:.2}..{pole_aft:.2})"
            );
        }
    }

    /// A bag sits on the deck over its whole length rather than bridging it.
    ///
    /// Measured the way you would with a hand: take the lowest cloth in each slice along
    /// the bag and see how far off the deck it is. Canvas touches; what it must not do is
    /// stand a finger's width clear, which is how the straight version read over the dip
    /// amidships, nor sink into the deck, which is how it reads if the seating radius is
    /// wrong.
    ///
    /// The rounded ends are left out of it: a bag's end turns up off the deck because the
    /// cloth is gathered there, and measuring it would be measuring the shape rather than
    /// the seating.
    #[test]
    fn every_bag_touches_the_deck() {
        for which in SAILS {
            let v = drawn(which);
            let (ends_fwd, ends_aft) = span(which);
            let turn = bagged(which).radius;
            let (fwd, aft) = (ends_fwd + turn, ends_aft - turn);
            let slices = 7;
            for i in 0..slices {
                let lo = fwd + (aft - fwd) * i as f32 / slices as f32;
                let hi = fwd + (aft - fwd) * (i + 1) as f32 / slices as f32;
                let under: Vec<&Vec3> = v.iter().filter(|p| p.z >= lo && p.z <= hi).collect();
                if under.is_empty() {
                    continue;
                }
                let lowest = under.iter().fold(f32::MAX, |m, p| m.min(p.y));
                let middle = (lo + hi) * 0.5;
                let deck = crate::deck::deck_height(middle);
                assert!(
                    lowest - deck < 0.035,
                    "{which:?} stands {:.3} m off the deck at z {middle:.2}",
                    lowest - deck
                );
                assert!(
                    deck - lowest < 0.035,
                    "{which:?} is {:.3} m into the deck at z {middle:.2}",
                    deck - lowest
                );
            }
        }
    }

    /// The four bags are four different bags.
    ///
    /// Asked for from the deck — vary the shape and size a little, give them
    /// distinguishing features — and it is the kind of thing that silently reverts the
    /// first time somebody tidies the table into a loop.
    #[test]
    fn no_two_bags_are_the_same_bag() {
        for a in SAILS {
            for b in SAILS {
                if a == b {
                    continue;
                }
                let (x, y) = (bagged(a), bagged(b));
                assert!(
                    (x.radius - y.radius).abs() > 0.01 || (x.length - y.length).abs() > 0.05,
                    "{a:?} and {b:?} are the same size"
                );
                assert!(x.seed != y.seed, "{a:?} and {b:?} crumple identically");
                assert!(
                    std::mem::discriminant(&x.feature) != std::mem::discriminant(&y.feature),
                    "{a:?} and {b:?} carry the same marking"
                );
            }
        }
    }
}
