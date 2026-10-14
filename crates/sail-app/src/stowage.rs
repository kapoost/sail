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

/// The colour of the bag, which is how anybody tells them apart on a foredeck.
///
/// Sailmakers do this for the same reason, and the convention is loose but real: the
/// light sails get the loud bags because they are the ones wanted in a hurry.
fn cloth(which: Headsail) -> Color {
    match which {
        Headsail::Genoa => Color::srgb(0.33, 0.35, 0.38),
        Headsail::Code0 => Color::srgb(0.28, 0.40, 0.52),
        Headsail::Gennaker => Color::srgb(0.55, 0.33, 0.22),
        Headsail::Spinnaker => Color::srgb(0.62, 0.48, 0.16),
    }
}

/// A sausage of a sail bag: round in the middle, drawn in at both ends by its own
/// drawstring, with a seam along the top and a band round one end where the label is.
///
/// Built rather than modelled because it has to sit on a deck whose height is a function
/// and not a drawing, and because a bag is three numbers: how long, how fat, and how hard
/// it is pulled in at the ends.
fn bag_mesh(length: f32, girth: f32) -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::mesh::{Indices, PrimitiveTopology};

    const RINGS: usize = 14;
    const AROUND: usize = 12;
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    for ring in 0..=RINGS {
        let t = ring as f32 / RINGS as f32;
        let z = (t - 0.5) * length;
        // Drawn in at the ends: a cosine shoulder rather than a cone, because a bag with
        // rope through its mouth gathers rather than tapers.
        let pinch = (std::f32::consts::PI * t).sin().powf(0.45);
        let r = girth * (0.18 + 0.82 * pinch);
        for a in 0..=AROUND {
            let ang = a as f32 / AROUND as f32 * std::f32::consts::TAU;
            let (s, c) = (ang.sin(), ang.cos());
            positions.push([r * s, r * c, z]);
            normals.push([s, c, 0.0]);
            uvs.push([a as f32 / AROUND as f32, t]);
        }
    }
    let mut indices = Vec::new();
    let stride = AROUND + 1;
    for ring in 0..RINGS {
        for a in 0..AROUND {
            let i = (ring * stride + a) as u32;
            let j = i + stride as u32;
            indices.extend_from_slice(&[i, j, i + 1, i + 1, j, j + 1]);
        }
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh.compute_normals();
    mesh
}

/// The markings on a bag: a seam up one side, a band round the label end, and a chevron
/// pointing at the mouth so a foredeck hand knows which way to drag it.
///
/// Drawn into a small texture rather than modelled, because that is what they are — sewn
/// tape and a stencil — and because a boat carries four of these and nobody is going to
/// look at them closely enough to want geometry.
fn markings() -> Image {
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
    for y in 0..H {
        for x in 0..W {
            // The seam: two stitched lines up the length of the bag.
            let seam = x == 3 || x == 5;
            // The label band, round the end where the drawstring is.
            let band = (6..11).contains(&y);
            // And the chevrons, pointing at that end.
            let into = |y: usize, x: usize| {
                let dx = (x as i32 - 32).abs();
                let on = (y as i32 - (20 + dx / 2)).abs() <= 1;
                on && dx < 22
            };
            let arrow = into(y, x) || into(y.wrapping_sub(14).min(H - 1), x);
            if seam {
                put(x, y, [120, 118, 112]);
            } else if band {
                put(x, y, [40, 42, 46]);
            } else if arrow {
                put(x, y, [90, 92, 96]);
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
    let stencil = images.add(markings());
    let bag = meshes.add(bag_mesh(1.55, 0.26));
    for which in [
        Headsail::Genoa,
        Headsail::Code0,
        Headsail::Gennaker,
        Headsail::Spinnaker,
    ] {
        let (z, side) = berth(which);
        // Sitting on the deck against the toe rail, not floating over the middle of it.
        let y = crate::deck::deck_height(z) + 0.17;
        let x = side * (crate::deck::deck_half_beam(z) - 0.36);
        commands.entity(root).with_children(|b| {
            b.spawn((
                SailBag(which),
                Mesh3d(bag.clone()),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: cloth(which),
                    base_color_texture: Some(stencil.clone()),
                    perceptual_roughness: 0.92,
                    ..default()
                })),
                // Lying fore and aft along the side deck, which is where a bag goes: it
                // is the only orientation that leaves the side deck walkable.
                Transform::from_xyz(x, y, z),
                Visibility::default(),
            ));
        });
    }

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
