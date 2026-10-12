//! Drawing the archipelago.
//!
//! The mesh comes from the same signed-distance field the physics runs aground on, so
//! the coast you see and the coast you touch are the same coast. That is worth more
//! than it sounds: the usual way to build this is to model the land and then write a
//! separate collision shape, and the two drift apart until the player is grounding in
//! open water or sailing through a cliff.
//!
//! Each island is a radial fan — a ring of points round the shore, lifted inland to
//! the island's own summit. Crude next to a heightfield, and right for the job: the
//! land is a silhouette on the horizon and a thing to keep off, not somewhere you go.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use sail_core::land::{Archipelago, Island};

use crate::frame;
use crate::sim::Sim;

/// Points round the shoreline of each island.
const RING: usize = 72;
/// Rings from the shore to the summit. More than before: the relief needs somewhere
/// to happen, and seven rings over a kilometre and a half of island is a tent.
const STEPS: usize = 14;

/// Marks an island so it can be re-placed when the render origin moves.
#[derive(Component)]
pub struct Landmass {
    /// Where it sits in simulation coordinates.
    pub at: glam::DVec2,
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    origin: Res<frame::RenderOrigin>,
    sim: Res<Sim>,
) {
    let Some(land) = sim.sim.land.clone() else {
        return;
    };

    // Two materials: what is above water and what is below it. The underwater skirt
    // is what makes a shoal visible before you are on it — a pale shelf showing
    // through the surface is the oldest piloting cue there is.
    let rock = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.94,
        ..default()
    });
    let shoal = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.96,
        ..default()
    });

    for island in land.islands() {
        commands.spawn((
            Landmass { at: island.centre },
            Mesh3d(meshes.add(island_mesh(island, false))),
            MeshMaterial3d(rock.clone()),
            Transform::from_translation(origin.to_bevy(island.centre, 0.0)),
        ));
        commands.spawn((
            Landmass { at: island.centre },
            Mesh3d(meshes.add(island_mesh(island, true))),
            MeshMaterial3d(shoal.clone()),
            Transform::from_translation(origin.to_bevy(island.centre, 0.0)),
        ));
    }
}

/// Keep the land where the simulation says it is after a change of render origin.
pub fn follow_origin(origin: Res<frame::RenderOrigin>, mut q: Query<(&Landmass, &mut Transform)>) {
    if !origin.is_changed() {
        return;
    }
    for (land, mut transform) in &mut q {
        transform.translation = origin.to_bevy(land.at, 0.0);
    }
}

/// One island, in its own local coordinates.
///
/// `below` builds the drowned part instead: the same outline carried down the shelf.
///
/// Coloured per vertex rather than textured. These are seen from one to five
/// kilometres away and almost never closer, which is a distance at which a texture is
/// a mip-mapped smear and the only things the eye actually gets are the shape of the
/// skyline and the way the colour changes with height. So: wet rock at the water, a
/// strand above it, heath over that, and bare stone on the tops — which is what a
/// Shetland island looks like and, more to the point, is legible at four miles.
fn island_mesh(island: &Island, below: bool) -> Mesh {
    let mut positions: Vec<Vec3> = Vec::new();
    let mut colours: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for step in 0..=STEPS {
        let t = step as f32 / STEPS as f32;
        for i in 0..RING {
            let bearing = i as f32 / RING as f32 * std::f32::consts::TAU;
            let shore = island.shore_radius(bearing as f64) as f32;
            let (r, y) = if below {
                // Out and down the bank.
                let out = shore + t * 300.0;
                (out, -34.0 * (t * t).min(1.0))
            } else {
                // In and up. Relief on top of the mean rise, because a smooth dome
                // reads as a dish-cover and a skyline is the only thing at this
                // distance with any information in it: two harmonics are enough to
                // give an island a summit, a shoulder and a saddle.
                let inward = shore * (1.0 - t);
                let relief = 1.0
                    + 0.30 * (bearing * 2.0 + island.centre.x as f32).sin()
                    + 0.16 * (bearing * 3.0 - island.centre.y as f32).cos();
                (inward, island.height as f32 * t.sqrt() * relief.max(0.35))
            };
            positions.push(Vec3::new(bearing.sin() * r, y, -bearing.cos() * r));
            colours.push(if below {
                // Pale sand going to the colour of deep water, so a bank announces
                // itself through the surface the way a real one does.
                let fade = 1.0 - t;
                [
                    0.30 + 0.34 * fade,
                    0.38 + 0.30 * fade,
                    0.36 + 0.18 * fade,
                    1.0,
                ]
            } else {
                let up = (y / island.height as f32).clamp(0.0, 1.0);
                // Wet rock, strand, heath, stone.
                let band = |a: [f32; 3], b: [f32; 3], f: f32| {
                    [
                        a[0] + (b[0] - a[0]) * f,
                        a[1] + (b[1] - a[1]) * f,
                        a[2] + (b[2] - a[2]) * f,
                    ]
                };
                let c = if up < 0.06 {
                    band([0.24, 0.23, 0.20], [0.52, 0.49, 0.40], up / 0.06)
                } else if up < 0.22 {
                    band([0.52, 0.49, 0.40], [0.26, 0.31, 0.20], (up - 0.06) / 0.16)
                } else {
                    band([0.26, 0.31, 0.20], [0.40, 0.39, 0.36], (up - 0.22) / 0.78)
                };
                [c[0], c[1], c[2], 1.0]
            });
        }
    }

    for step in 0..STEPS {
        for i in 0..RING {
            let next = (i + 1) % RING;
            let a = (step * RING + i) as u32;
            let b = (step * RING + next) as u32;
            let c = ((step + 1) * RING + next) as u32;
            let d = ((step + 1) * RING + i) as u32;
            if below {
                indices.extend_from_slice(&[a, b, c, a, c, d]);
            } else {
                indices.extend_from_slice(&[a, d, c, a, c, b]);
            }
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    mesh.insert_indices(Indices::U32(indices));
    mesh.compute_normals();
    mesh
}

/// The archipelago the game is set in.
///
/// Eighteen kilometres square with twenty-six islands: far enough that the far side is
/// a passage rather than a hop, close enough that there is always something on the
/// horizon to steer for.
pub fn generate(seed: u64) -> Archipelago {
    Archipelago::generate_around(seed ^ 0x15_1A_4D, 9_000.0, 26, 900.0)
}
