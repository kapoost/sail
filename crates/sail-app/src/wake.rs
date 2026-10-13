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

/// Samples kept. At half a metre apart that is a hundred and twenty metres of wake.
///
/// Asked for from the deck: the track she has been over should stay bright behind her.
/// It should, and it does on the water — a disturbed strip holds its smoothed, paler
/// look for a minute or more in a seaway, long after the foam in it has gone. Sixty
/// metres was a wake; this is a track.
const SAMPLES: usize = 240;
/// Points across the wake at each sample.
///
/// Two was not enough and was the whole of the problem. With only the edges, the quad
/// between them is a flat chord nine metres wide laid across a sea whose waves are
/// thirty-two metres long — a plank lying over the water rather than foam on it, which
/// from astern reads exactly as a slab passing through the hull. Five points, each
/// asking the water its own height, and the ribbon follows the surface.
const ACROSS: usize = 5;
/// Distance between samples, metres.
const SPACING: f64 = 0.5;
/// How long a sample stays visible, seconds.
///
/// Thirty-five, up from fourteen. At seven knots the oldest sample in the buffer is a
/// hundred and twenty metres astern and half a minute old, so the two limits are now
/// about equal and whichever runs out first does so honestly: a fast boat is cut off by
/// the length of the buffer, a slow one by the water settling.
const LIFETIME: f32 = 35.0;

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
    let vertices = SAMPLES * ACROSS;
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![Vec3::ZERO; vertices]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![Vec3::Y; vertices]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![Vec2::ZERO; vertices]);
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_COLOR,
        vec![[1.0_f32, 1.0, 1.0, 0.0]; vertices],
    );

    // A strip of quads between each pair of samples, and across the width.
    let mut indices = Vec::with_capacity((SAMPLES - 1) * (ACROSS - 1) * 6);
    for i in 0..SAMPLES - 1 {
        for k in 0..ACROSS - 1 {
            let a = (i * ACROSS + k) as u32;
            let b = a + 1;
            let c = ((i + 1) * ACROSS + k) as u32;
            let d = c + 1;
            indices.extend_from_slice(&[a, b, d, a, d, c]);
        }
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
    ink: Res<crate::world::Ink>,
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
        // What she is actually tearing up. Squared rather than linear, because that is
        // how the energy she leaves in the water grows, and because it keeps a boat
        // drifting at two knots from laying the same white scar as one driving at eight.
        let knots = sail_core::units::ms_to_kn(sim.sim.telemetry.stw).max(0.0);
        let strength = ((knots / 6.5).powi(2)).clamp(0.0, 1.0);
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
    let mut positions = Vec::with_capacity(SAMPLES * ACROSS);
    let mut colours = Vec::with_capacity(SAMPLES * ACROSS);
    let mut uvs = Vec::with_capacity(SAMPLES * ACROSS);

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
            for _ in 0..ACROSS {
                positions.push(here);
                colours.push([1.0, 1.0, 1.0, 0.0]);
                uvs.push(Vec2::ZERO);
            }
            continue;
        };
        let age = (now - sample.born).max(0.0);
        let life = (1.0 - age / LIFETIME).clamp(0.0, 1.0);
        // A wake spreads as it ages and thins as it spreads.
        let half_width = beam * (0.75 + 1.6 * (age as f64 / LIFETIME as f64).min(1.0));
        // Two fades in one: the foam goes quickly, and the smoothed water it leaves
        // behind stays. A single curve could only do one of them, and doing the first
        // alone is what made the wake vanish while the track it marked was still there
        // on the sea astern.
        let foam = life.powf(3.0);
        let track = life.powf(0.55);
        let alpha = (foam * 0.55 + track * 0.30) * sample.strength;

        for k in 0..ACROSS {
            // -1 at the port edge, 0 down the middle, +1 at the starboard edge.
            let u = k as f64 / (ACROSS - 1) as f64 * 2.0 - 1.0;
            let at = sample.at + sample.across * half_width * u;
            // AGAINST THE SURFACE THAT IS DRAWN, not the one the water has. The sea mesh
            // carries nothing finer than four samples across a cell, so `update_sea` asks
            // the field filtered to that; this asked the field itself, short chop and all.
            // The disagreement is a few centimetres and the ribbon floats four — so on a
            // short sea the wake spent half its length *under* the drawn skin, surfacing
            // and vanishing as the triangles slid over it.
            //
            // `spray.rs` already learned this and says so at length; the two now read the
            // same definition out of `world::drawn_resolution`.
            // Measured from the BOAT, not from `RenderOrigin`: the sea grid is built
            // around the boat, so it is distance from her that decides how coarse the
            // cell is here. `spray.rs` reads it the same way.
            let offset = at - st.pos;
            let resolution =
                crate::world::drawn_resolution(Vec2::new(offset.x as f32, -offset.y as f32));
            let height = sim.sim.water.height_resolved(at, t, resolution);
            positions.push(origin.to_bevy(at, height as f32 + 0.04));
            // Bright down the middle where the hull churned it, and genuinely zero at
            // the edges. The old version set the full value on *both* edge vertices —
            // the comment said the edges were softer and the code said otherwise — so
            // the wake had a hard rim, which is the other half of why it read as a
            // sheet of something rather than as foam.
            let across_fade = 1.0 - (u * u) as f32;
            let mut a = alpha * across_fade * across_fade;
            if ink.0 {
                // The drawn sea has four tones in it and a hard edge between them, and a
                // wake smeared smoothly across that is the one soft thing left in the
                // picture. Three steps and nothing between: the white of the break, the
                // paler water behind it, and the sea.
                a = match a {
                    x if x > 0.34 => 0.62,
                    x if x > 0.12 => 0.26,
                    _ => 0.0,
                };
            }
            colours.push([1.0, 1.0, 1.0, a]);
            uvs.push(Vec2::new(
                (u as f32 * 0.5 + 0.5).clamp(0.0, 1.0),
                index as f32,
            ));
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
}
