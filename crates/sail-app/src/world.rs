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
use crate::sea_texture;
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
    mut images: ResMut<Assets<Image>>,
) {
    // Sun, low enough to put some glitter on the water.
    commands.spawn((
        DirectionalLight {
            illuminance: 32_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        // Pointed from the one definition of where the sun is, so the drawn disc and
        // the light cannot drift apart.
        Transform::default().looking_to(-sea_texture::SUN_FROM_SKY, Vec3::Y),
    ));
    // Much dimmer than before: the environment map now supplies the sky's own
    // contribution properly, and leaving the old flat ambient term in place on top of
    // it just washes the whole scene out.
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.62, 0.74, 0.88),
        brightness: 60.0,
        ..default()
    });

    // Water is a dielectric, not a metal. `metallic: 1.0` was simply wrong: it tints
    // the specular with the base colour and removes the diffuse term entirely, which
    // is how you render chrome. Water has an F0 of about two percent — Bevy maps
    // reflectance 0.5 to four percent, so 0.25 — and Fresnel takes that to a near
    // mirror at grazing angles on its own. That rise towards the horizon is most of
    // what the eye reads as water, and it only appears once there is an environment
    // map to reflect.
    //
    // White base colour because the water's own colour is carried per vertex, which
    // is what lets a crest reach foam without overdriving a dark base past one.
    let water = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        normal_map_texture: Some(images.add(sea_texture::normal_map())),
        metallic_roughness_texture: Some(images.add(sea_texture::roughness_map())),
        perceptual_roughness: 1.0,
        metallic: 0.0,
        reflectance: 0.25,
        double_sided: true,
        cull_mode: None,
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
    let count = offsets.len();
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![Vec3::ZERO; count]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![Vec3::Y; count]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![Vec2::ZERO; count]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, vec![Vec4::X; count]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0_f32, 1.0, 1.0, 1.0]; count]);
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

        let count = sea.offsets.len();
        let mut positions = Vec::with_capacity(count);
        let mut normals = Vec::with_capacity(count);
        let mut uvs = Vec::with_capacity(count);
        let mut tangents = Vec::with_capacity(count);
        let mut colours = Vec::with_capacity(count);

        let hs = (sim.sim.telemetry.significant_height as f32).max(0.05);
        // Steady surface drift: about two and a half percent of the wind speed,
        // downwind, which is the usual rule of thumb. This is a real current and it
        // accumulates honestly with time.
        //
        // What it replaces was a bug worth remembering: the instantaneous orbital
        // velocity multiplied by elapsed time. The velocity oscillates with the wave
        // period and the elapsed time only grows, so the detail slid back and forth in
        // every direction with an amplitude that increased without bound — which from
        // the deck reads exactly like the boat periodically going backwards.
        let tm = &sim.sim.telemetry;
        let downwind = glam::DVec2::new(
            (tm.twd + std::f64::consts::PI).sin(),
            (tm.twd + std::f64::consts::PI).cos(),
        );
        let current = downwind * tm.tws * 0.025 * t;

        for offset in &sea.offsets {
            // Sample at the true world position. Bevy's -Z is sim north, so the
            // horizontal offset flips sign on the way across.
            let world = st.pos + glam::DVec2::new(offset.x as f64, -offset.y as f64);
            let height = sim.sim.water.height(world, t) as f32;
            let slope = sim.sim.water.slope(world, t);
            let (sx, sz) = (slope.x as f32, -slope.y as f32);

            positions.push(Vec3::new(offset.x, height, offset.y));
            normals.push(Vec3::new(-sx, 1.0, -sz).normalize());

            // World-locked UVs, so the detail stays with the sea rather than with the
            // boat: the grid recentres every frame and the texture must not. On top of
            // that the detail rides the water's own orbital displacement, which is
            // bounded — a surface parcel travels a circle of the wave's amplitude — so
            // the ripples go round with the wave instead of being dragged across it.
            let parcel = world - sim.sim.water.horizontal_displacement(world, t) - current;
            uvs.push(Vec2::new(
                parcel.x as f32 / sea_texture::TILE_METRES,
                -parcel.y as f32 / sea_texture::TILE_METRES,
            ));

            // Tangent along +U, which is world +X, lying in the surface. Handedness is
            // negative because cross(normal, tangent) points at -Z while V runs to +Z.
            let tangent = Vec3::new(1.0, sx, 0.0).normalize();
            tangents.push(Vec4::new(tangent.x, tangent.y, tangent.z, -1.0));

            // Colour per vertex: troughs darker, crests lighter, and foam where the
            // surface is steep enough to be breaking. That is the depth shading and
            // the whitecaps, without a shader having to be written for either.
            let lift = (height / hs).clamp(-1.0, 1.0);
            let deep = Vec3::new(0.035, 0.095, 0.150);
            let shallow = Vec3::new(0.090, 0.210, 0.260);
            let base = deep.lerp(shallow, (lift * 0.5 + 0.5).clamp(0.0, 1.0));
            let steepness = (sx * sx + sz * sz).sqrt();
            let foam = ((steepness - 0.16) / 0.14).clamp(0.0, 1.0) * crest_weight(lift);
            let colour = base.lerp(Vec3::new(0.82, 0.88, 0.92), foam);
            colours.push([colour.x, colour.y, colour.z, 1.0]);
        }
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, tangents);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    }
}

/// Foam only belongs near a crest. A steep trough is still green water.
#[inline]
fn crest_weight(lift: f32) -> f32 {
    ((lift + 0.15) / 0.6).clamp(0.0, 1.0)
}
