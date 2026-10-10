//! The wake.
//!
//! Not decoration. A boat with no wake has displaced no water, and the eye reads that
//! as weightless however heavy the simulation says it is — the complaint that nothing
//! has inertia was partly this. A trail of disturbed water behind the hull is the
//! cheapest and strongest statement a vessel can make about its own mass.
//!
//! Laid down in world coordinates at intervals of distance rather than of time, so it
//! marks where the boat *went* instead of how long it spent there. Each sample rides
//! the water surface it was laid on, which means the wake climbs and falls with the
//! waves like the rest of the sea rather than floating flat across them.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::frame;
use crate::sim::Sim;

/// Samples kept. At half a metre apart that is sixty metres of wake, which is about
/// as far as one stays visible on open water.
const SAMPLES: usize = 120;
/// Distance between samples, metres.
const SPACING: f64 = 0.5;
/// How long a sample stays visible, seconds. Long enough to see, short enough that a
/// slow boat does not trail a scar.
const LIFETIME: f32 = 14.0;

#[derive(Clone, Copy)]
struct Sample {
    at: glam::DVec2,
    /// Across-track unit vector in simulation coordinates, so the ribbon keeps its
    /// width through a turn instead of pinching.
    across: glam::DVec2,
    born: f32,
    /// Speed through the water when laid. A boat barely moving leaves barely a mark.
    strength: f32,
}

#[derive(Resource)]
pub struct Wake {
    mesh: Handle<Mesh>,
    samples: Vec<Sample>,
    last: Option<glam::DVec2>,
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    let vertices = SAMPLES * 2;
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![Vec3::ZERO; vertices]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![Vec3::Y; vertices]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![Vec2::ZERO; vertices]);
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_COLOR,
        vec![[1.0_f32, 1.0, 1.0, 0.0]; vertices],
    );

    // One quad between each pair of samples.
    let mut indices = Vec::with_capacity((SAMPLES - 1) * 6);
    for i in 0..SAMPLES - 1 {
        let a = (i * 2) as u32;
        indices.extend_from_slice(&[a, a + 1, a + 3, a, a + 3, a + 2]);
    }
    mesh.insert_indices(Indices::U32(indices));
    let handle = meshes.add(mesh);

    let foam = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        double_sided: true,
        cull_mode: None,
        // Lifted clear of the surface so it reads as foam on the water rather than
        // fighting the sea mesh for the same depth.
        depth_bias: 8.0,
        ..default()
    });

    commands.spawn((
        Mesh3d(handle.clone()),
        MeshMaterial3d(foam),
        Transform::default(),
    ));
    commands.insert_resource(Wake {
        mesh: handle,
        samples: Vec::with_capacity(SAMPLES),
        last: None,
    });
}

pub fn update(
    origin: Res<frame::RenderOrigin>,
    sim: Res<Sim>,
    time: Res<Time>,
    mut wake: ResMut<Wake>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let st = &sim.render_state();
    let now = time.elapsed_secs();
    let stern = st.pos - st.forward() * (sim.config().loa * 0.45);

    // Lay a new sample once she has moved far enough to have left a new mark.
    let moved = wake.last.is_none_or(|l| (stern - l).length() >= SPACING);
    if moved {
        let strength = (sail_core::units::ms_to_kn(sim.sim.telemetry.stw) / 6.0).clamp(0.0, 1.0);
        wake.samples.push(Sample {
            at: stern,
            across: st.starboard(),
            born: now,
            strength: strength as f32,
        });
        wake.last = Some(stern);
        if wake.samples.len() > SAMPLES {
            wake.samples.remove(0);
        }
    }

    let Some(mut mesh) = meshes.get_mut(&wake.mesh) else {
        return;
    };
    let t = st.time;
    let mut positions = Vec::with_capacity(SAMPLES * 2);
    let mut colours = Vec::with_capacity(SAMPLES * 2);
    let mut uvs = Vec::with_capacity(SAMPLES * 2);

    let beam = sim.config().beam * 0.5;
    for index in 0..SAMPLES {
        // Pad the unused tail with degenerate, invisible vertices.
        let sample = wake
            .samples
            .get(wake.samples.len().saturating_sub(SAMPLES) + index);
        let Some(sample) = sample.filter(|_| index < wake.samples.len()) else {
            // Collapse unused samples onto the boat, not onto Vec3::ZERO. Zero is the
            // *world origin*, and the triangles joining those vertices to the real
            // ones stretched a sheet from the hull all the way back to it — which,
            // since the boat starts near the origin, drew a grey slab across the sea.
            let here = origin.to_bevy(st.pos, 0.0);
            positions.push(here);
            positions.push(here);
            colours.push([1.0, 1.0, 1.0, 0.0]);
            colours.push([1.0, 1.0, 1.0, 0.0]);
            uvs.push(Vec2::ZERO);
            uvs.push(Vec2::ZERO);
            continue;
        };
        let age = (now - sample.born).max(0.0);
        let life = (1.0 - age / LIFETIME).clamp(0.0, 1.0);
        // A wake spreads as it ages and thins as it spreads.
        let half_width = beam * (0.85 + 1.9 * (age as f64 / LIFETIME as f64).min(1.0));
        let alpha = life * life * sample.strength * 0.55;

        for side in [-1.0, 1.0] {
            let at = sample.at + sample.across * half_width * side;
            let height = sim.sim.water.height(at, t);
            positions.push(origin.to_bevy(at, height as f32 + 0.04));
            // Softer at the edges than down the middle, which is where the hull
            // actually churned it.
            colours.push([1.0, 1.0, 1.0, alpha * 0.55]);
            uvs.push(Vec2::new(if side < 0.0 { 0.0 } else { 1.0 }, index as f32));
        }
        // Brighten the centre line by pulling the inner edge alpha up.
        let n = colours.len();
        colours[n - 2][3] = alpha;
        colours[n - 1][3] = alpha;
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
}
