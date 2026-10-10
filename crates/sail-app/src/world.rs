//! Sea, sky and light.
//!
//! The surface is displaced from **the same wave field the physics uses**. That is not
//! an implementation detail: a boat riding one sea while the eye watches a different
//! one is worse than no waves at all, because every cue disagrees with every other.
//!
//! The mesh is a graded grid that follows the boat — about 0.7 m between vertices
//! underneath her, widening to 25 m at the horizon seven hundred metres out, so the
//! detail is where the eye is. Vertices are sampled at true world coordinates rather
//! than at grid offsets, so recentring the grid does not make the sea swim.
//!
//! Normals come from the field's analytic slope rather than from the triangles. That
//! is both cheaper than recomputing them and exactly right, and it means the lighting
//! shows the wave shape even where the mesh is too coarse to.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::frame;
use crate::sim::Sim;

/// Vertices across the grid. Odd, so one sits under the boat.
const GRID: usize = 161;
/// Half-width of the sea patch, metres.
const EXTENT: f32 = 700.0;
/// Share of the grid spacing that stays uniform. The rest is cubic, which is what
/// concentrates resolution near the boat.
const GRADE_LINEAR: f32 = 0.08;

#[derive(Component)]
pub struct SeaSurface {
    mesh: Handle<Mesh>,
    /// Grid offsets in the Bevy horizontal plane, metres, precomputed once.
    offsets: Vec<Vec2>,
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Sun, low enough to put some glitter on the water.
    commands.spawn((
        DirectionalLight {
            illuminance: 32_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, -0.9, -0.55, 0.0)),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.62, 0.74, 0.88),
        brightness: 420.0,
        ..default()
    });
    commands.insert_resource(ClearColor(Color::srgb(0.58, 0.70, 0.82)));

    let water = materials.add(StandardMaterial {
        base_color: Color::srgb(0.06, 0.14, 0.21),
        perceptual_roughness: 0.22,
        metallic: 0.10,
        ..default()
    });

    // Graded spacing: mostly cubic, with a linear floor so the centre does not
    // collapse to millimetres and waste vertices on detail nobody can see.
    let grade = |i: usize| -> f32 {
        let t = (i as f32 / (GRID - 1) as f32) * 2.0 - 1.0;
        let a = t.abs();
        t.signum() * EXTENT * (GRADE_LINEAR * a + (1.0 - GRADE_LINEAR) * a * a * a)
    };

    let mut offsets = Vec::with_capacity(GRID * GRID);
    for ix in 0..GRID {
        for iz in 0..GRID {
            offsets.push(Vec2::new(grade(ix), grade(iz)));
        }
    }

    let mut indices = Vec::with_capacity((GRID - 1) * (GRID - 1) * 6);
    for ix in 0..GRID - 1 {
        for iz in 0..GRID - 1 {
            let a = (ix * GRID + iz) as u32;
            let b = ((ix + 1) * GRID + iz) as u32;
            let c = ((ix + 1) * GRID + iz + 1) as u32;
            let d = (ix * GRID + iz + 1) as u32;
            indices.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![Vec3::ZERO; offsets.len()]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![Vec3::Y; offsets.len()]);
    mesh.insert_indices(Indices::U32(indices));
    let handle = meshes.add(mesh);

    commands.spawn((
        SeaSurface {
            mesh: handle.clone(),
            offsets,
        },
        Mesh3d(handle),
        MeshMaterial3d(water),
        Transform::default(),
    ));
}

/// Displace the surface from the physics' own wave field, and keep it under the boat.
pub fn update_sea(
    sim: Res<Sim>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut q: Query<(&SeaSurface, &mut Transform)>,
) {
    let st = &sim.sim.state;
    let centre = frame::to_bevy(st.pos, 0.0);
    let t = st.time;

    for (sea, mut transform) in &mut q {
        transform.translation = Vec3::new(centre.x, 0.0, centre.z);
        let Some(mut mesh) = meshes.get_mut(&sea.mesh) else {
            continue;
        };

        let mut positions = Vec::with_capacity(sea.offsets.len());
        let mut normals = Vec::with_capacity(sea.offsets.len());
        for offset in &sea.offsets {
            // Sample at the true world position. Bevy's -Z is sim north, so the
            // horizontal offset flips sign on the way across.
            let world = st.pos + glam::DVec2::new(offset.x as f64, -offset.y as f64);
            let height = sim.sim.water.height(world, t);
            let slope = sim.sim.water.slope(world, t);
            positions.push(Vec3::new(offset.x, height as f32, offset.y));
            // Surface normal from the gradient. The sign on the second component
            // follows the same axis flip as the sample position.
            normals.push(Vec3::new(-slope.x as f32, 1.0, slope.y as f32).normalize());
        }
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    }
}
