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
/// Rings from the shore to the summit.
const STEPS: usize = 7;

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
        base_color: Color::srgb(0.30, 0.31, 0.26),
        perceptual_roughness: 0.92,
        ..default()
    });
    let shoal = materials.add(StandardMaterial {
        base_color: Color::srgb(0.52, 0.55, 0.45),
        perceptual_roughness: 0.95,
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
/// `below` builds the drowned part instead: the same outline carried down the shelf
/// to where the water is deep enough to hide it.
fn island_mesh(island: &Island, below: bool) -> Mesh {
    let mut positions: Vec<Vec3> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for step in 0..=STEPS {
        let t = step as f32 / STEPS as f32;
        for i in 0..RING {
            let bearing = i as f32 / RING as f32 * std::f32::consts::TAU;
            let shore = island.shore_radius(bearing as f64) as f32;
            let (r, y) = if below {
                // Out and down the bank. The shelf is wide, so this is a broad pale
                // apron rather than a cliff — which is what a shoal looks like.
                let out = shore + t * 400.0;
                (out, -40.0 * (t * t).min(1.0))
            } else {
                // In and up. A square root rather than a straight slope, so the land
                // rises steeply from the water and flattens towards the top the way a
                // weathered island does.
                let inward = shore * (1.0 - t);
                (inward, island.height as f32 * t.sqrt())
            };
            // Bevy's -Z is north, so the bearing turns the other way across.
            positions.push(Vec3::new(bearing.sin() * r, y, -bearing.cos() * r));
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
