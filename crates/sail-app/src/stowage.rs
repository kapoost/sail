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

/// Where each bag lies: along the side deck, forward of the mast, in the order a crew
/// would strike them — the one you are most likely to want next is nearest the hatch.
fn berth(which: Headsail) -> (f32, f32) {
    // Where the deck is wide enough to put a bag down and still walk past it, which on
    // this hull is between the mast and the forward end of the coachroof: half a metre
    // further forward the half-beam is down to a quarter of a metre and a bag put there
    // is inside the stem. Measured rather than guessed — the sheer is a function in
    // `deck.rs` and it can simply be asked.
    match which {
        // (z along the boat, which side)
        Headsail::Genoa => (MAST_FOOT_Z + 0.5, 1.0),
        Headsail::Code0 => (MAST_FOOT_Z - 0.6, -1.0),
        Headsail::Gennaker => (MAST_FOOT_Z + 0.5, -1.0),
        Headsail::Spinnaker => (MAST_FOOT_Z - 0.6, 1.0),
    }
}

/// The band on the bag, which is how anybody tells them apart on a foredeck.
///
/// The bag itself is the same for all four and it is not a colour anybody chose: a sail
/// bag is made out of a sail. When a working jib is finally past repair it goes to the
/// loft and comes back as bags, which is why every bag on every cruising boat is the
/// same tired cream as every other, and why the only thing that distinguishes them is a
/// band of tape and a stencil. That is what is drawn.
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
const CANVAS: Color = Color::srgb(0.72, 0.69, 0.60);

/// A duffle with a sail in it, which is not a cylinder.
///
/// Reported from the deck: too regular, should be flattened, crumpled, not that stiff,
/// and lying slightly bent. All of which is one observation — canvas has no opinions.
/// A bag with thirty kilos of wet sailcloth in it spreads under its own weight, takes
/// the camber of the deck it is lying on, and is covered in the creases of however it
/// was stuffed; the only straight thing about it is the tape round its mouth.
///
/// Built by taking the engine's capsule and ruining it: squashed to two thirds, bent
/// along its length to lie into the deck, and every vertex pushed about by a cheap hash
/// of its own position so the surface is lumpy in a way that does not repeat. The hash is
/// deterministic, so the four bags on this deck are the same shape — which is right,
/// because they came out of the same loft.
fn duffle() -> Mesh {
    let mut mesh = Mesh::from(Capsule3d::new(0.30, 1.05));
    let Some(bevy::render::mesh::VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    else {
        return mesh;
    };
    for p in positions.iter_mut() {
        let (x, y, z) = (p[0], p[1], p[2]);
        // Squashed: a bag of cloth is wider than it is tall.
        let mut nx = x * 1.18;
        let mut nz = z * 0.66;
        // And bent, so it lies along a deck rather than across it.
        let along = (y / 0.82).clamp(-1.0, 1.0);
        nx += along * along * 0.07;
        // Creases. A hash of the position, which gives the same lumps every time without
        // anything to store and without a pattern the eye can pick up.
        let hash = |a: f32, b: f32, c: f32| {
            let v = (a * 12.9898 + b * 78.233 + c * 37.719).sin() * 43758.547;
            v - v.floor() - 0.5
        };
        let r = (nx * nx + nz * nz).sqrt().max(1e-4);
        let crease =
            hash(x * 9.0, y * 11.0, z * 7.0) * 0.055 + hash(x * 23.0, y * 19.0, z * 29.0) * 0.022;
        nx += nx / r * crease;
        nz += nz / r * crease;
        let ny = y + hash(x * 5.0, y * 13.0, z * 3.0) * 0.02;
        *p = [nx, ny, nz];
    }
    mesh.compute_normals();
    mesh
}

/// The markings on a bag: a seam up one side, a band round the label end, and a chevron
/// pointing at the mouth so a foredeck hand knows which way to drag it.
///
/// Drawn into a small texture rather than modelled, because that is what they are — sewn
/// tape and a stencil — and because a boat carries four of these and nobody is going to
/// look at them closely enough to want geometry.
fn markings(band_colour: [u8; 3]) -> Image {
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
    for y in 0..H {
        for x in 0..W {
            // Two handle tapes, a quarter of the way round each side, each with its
            // stitching either side of it.
            let on_tape = |at: usize| x.abs_diff(at) <= 2;
            let stitched = |at: usize| x.abs_diff(at) == 3;
            let handle = on_tape(16) || on_tape(48);
            let handle_stitch = stitched(16) || stitched(48);
            // Bindings round both ends, and the label band inboard of the one at the
            // mouth.
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
    let bag = meshes.add(duffle());
    for which in [
        Headsail::Genoa,
        Headsail::Code0,
        Headsail::Gennaker,
        Headsail::Spinnaker,
    ] {
        let (z, side) = berth(which);
        // Sitting on the deck against the toe rail, not floating over the middle of it.
        let y = crate::deck::deck_height(z) + 0.24;
        let x = side * (crate::deck::deck_half_beam(z) - 0.36);
        commands.entity(root).with_children(|b| {
            b.spawn((
                SailBag(which),
                Mesh3d(bag.clone()),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: CANVAS,
                    base_color_texture: Some(images.add(markings(band(which)))),
                    perceptual_roughness: 0.95,
                    ..default()
                })),
                // Lying fore and aft along the side deck, which is where a bag goes: it
                // is the only orientation that leaves the side deck walkable. A capsule
                // stands up the Y axis, so it is laid down.
                // Lying fore and aft, and turned a little off the centreline because
                // nobody puts a bag down square.
                Transform::from_xyz(x, y, z).with_rotation(
                    Quat::from_rotation_y(side * 0.12)
                        * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                Visibility::default(),
            ));
        });
    }

    info!(
        "stowed: bags at z {:.1} and {:.1}, x +/-{:.2}, y {:.2}",
        berth(Headsail::Genoa).0,
        berth(Headsail::Code0).0,
        crate::deck::deck_half_beam(berth(Headsail::Genoa).0) - 0.36,
        crate::deck::deck_height(berth(Headsail::Genoa).0) + 0.26
    );
    // And the pole in its chocks along the port side deck.
    //
    // Laid *along the sheer* rather than parallel to the centreline, because the side
    // deck narrows towards the bow: a spar put down at a constant offset from the middle
    // is inboard at its after end and hanging over the water at its forward one, which is
    // what was reported. Both ends are set in from the gunwale by the same hand's breadth
    // and the spar takes the angle between them, which is what a pair of chocks does.
    let length = crate::boat::pole_length(sim.config().loa as f32);
    let aft = MAST_FOOT_Z - 0.3;
    let forward = aft - length;
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
