//! The wind, made visible.
//!
//! Streaks running over the water, the way a gust shows itself as a cat's paw before
//! it reaches you. Drawn at the full resolution of the screen while the world behind
//! them may be four hundred and eighty pixels across, which is the whole idea: one
//! sharp moving thing over a deliberately coarse picture reads as *weather* rather
//! than as a mistake, and no era of hardware could have done it, which is why it looks
//! like nothing else.
//!
//! It is not decoration. Each streak is carried by the same wind field the sails are
//! working in — gusts, shifts and all — so what you are watching is the actual
//! quantity the physics is using. Behind an island they thin out and bend, because
//! that is what `ShelteredWind` does to the air there. The wind shadow has been in the
//! model since the archipelago went in and this is the first time anyone can see it
//! without reading a number off a panel.
//!
//! Low over the surface on purpose. Streaks at mast height would need to be hidden by
//! the sails, and they cannot be: the world's depth buffer belongs to the small image
//! and this is drawn afterwards, straight to the window. Keeping them on the water
//! keeps them out of the rig, which is both the honest workaround and where cat's paws
//! actually are.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::frame;
use crate::sim::Sim;

/// How many streaks are alive at once.
const COUNT: usize = 1000;
/// Points across the little grid of wind the streaks are carried by.
///
/// They cannot each ask the wind field directly. `ShelteredWind` answers by tracing
/// five rays of up to sixty-four steps through twenty-six islands, which is fine once a
/// substep at the boat and ruinous nine hundred times a frame — measured, it halved the
/// frame rate on its own: around fifteen million island-distance evaluations every
/// frame to move some specks of foam.
///
/// So the field is sampled on a seven by seven lattice over the ring, once, and the
/// streaks interpolate between the corners. Forty-nine evaluations instead of eighteen
/// hundred, and nothing is lost: the whole point is the lee of an island, which is a
/// kilometre across, and the lattice is fifteen metres.
const GRID: usize = 7;
/// How far from the boat they are kept, metres.
const INNER: f64 = 6.0;
const OUTER: f64 = 110.0;
/// Seconds a streak lives.
const LIFE: f32 = 3.4;
/// How fast a streak travels compared with the wind that carries it.
///
/// A cat's paw is a patch of ruffled water, and what moves is the *pattern* rather than
/// the air — which travels at a good fraction of the wind but not at it.
const CARRY: f64 = 0.55;

struct Streak {
    at: glam::DVec2,
    age: f32,
    /// Length and brightness vary, or they read as a repeating pattern.
    scale: f32,
}

#[derive(Resource)]
pub struct Streaks {
    mesh: Handle<Mesh>,
    live: Vec<Streak>,
    seed: sail_core::rng::Pcg32,
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
    let vertices = COUNT * 4;
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![Vec3::ZERO; vertices]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![Vec3::Y; vertices]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![Vec2::ZERO; vertices]);
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_COLOR,
        vec![[1.0_f32, 1.0, 1.0, 0.0]; vertices],
    );
    let mut indices = Vec::with_capacity(COUNT * 6);
    for i in 0..COUNT {
        let a = (i * 4) as u32;
        indices.extend_from_slice(&[a, a + 1, a + 2, a, a + 2, a + 3]);
    }
    mesh.insert_indices(Indices::U32(indices));
    let handle = meshes.add(mesh);

    commands.spawn((
        Mesh3d(handle.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            alpha_mode: AlphaMode::Add,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        })),
        Transform::default(),
    ));

    commands.insert_resource(Streaks {
        mesh: handle,
        live: Vec::with_capacity(COUNT),
        seed: sail_core::rng::Pcg32::new(0x57_12_EA),
    });
}

pub fn update(
    origin: Res<frame::RenderOrigin>,
    sim: Res<Sim>,
    time: Res<Time>,
    mut streaks: ResMut<Streaks>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let st = &sim.render_state();
    let dt = time.delta_secs();
    let t = st.time;

    // The wind over the ring, sampled once.
    let span = OUTER * 1.3;
    let mut field = [[glam::DVec2::ZERO; GRID]; GRID];
    for (i, row) in field.iter_mut().enumerate() {
        for (k, cell) in row.iter_mut().enumerate() {
            let at = st.pos
                + glam::DVec2::new(
                    (i as f64 / (GRID - 1) as f64 * 2.0 - 1.0) * span,
                    (k as f64 / (GRID - 1) as f64 * 2.0 - 1.0) * span,
                );
            *cell = sim.sim.wind.sample(at, t);
        }
    }
    let flow_at = |p: glam::DVec2| -> glam::DVec2 {
        let u = ((p.x - st.pos.x) / span * 0.5 + 0.5).clamp(0.0, 1.0) * (GRID - 1) as f64;
        let v = ((p.y - st.pos.y) / span * 0.5 + 0.5).clamp(0.0, 1.0) * (GRID - 1) as f64;
        let (i, k) = (u.floor() as usize, v.floor() as usize);
        let (i1, k1) = ((i + 1).min(GRID - 1), (k + 1).min(GRID - 1));
        let (fu, fv) = (u - i as f64, v - k as f64);
        let a = field[i][k].lerp(field[i1][k], fu);
        let b = field[i][k1].lerp(field[i1][k1], fu);
        a.lerp(b, fv)
    };

    // Age, carry, and retire. A streak that has wandered outside the ring is retired
    // rather than wrapped, because wrapping makes them visibly reappear in a line.
    let mut keep = Vec::with_capacity(COUNT);
    for mut s in streaks.live.drain(..) {
        s.age += dt;
        let flow = flow_at(s.at);
        s.at += flow * CARRY * dt as f64;
        let range = (s.at - st.pos).length();
        if s.age < LIFE && range < OUTER * 1.25 {
            keep.push(s);
        }
    }
    streaks.live = keep;

    // Top up, scattered round the boat. More of them in more wind, because that is
    // what the water does — a calm has no cat's paws in it at all.
    // Twenty-two knots used to fill the field and that is far too late: the water starts
    // showing the wind long before it is blowing hard, and a fresh breeze over open water
    // is *covered* in cat's paws. Full at fourteen now, with a floor so a drifting day
    // still has something moving on it — the one condition where you are reading the
    // water hardest is the one where there is least to read.
    let knots = sail_core::units::ms_to_kn(sim.sim.telemetry.tws);
    let density = (0.12 + knots / 14.0).clamp(0.0, 1.0);
    let want = (COUNT as f64 * density as f64) as usize;
    while streaks.live.len() < want {
        let a = streaks.seed.next_f64() * std::f64::consts::TAU;
        let r = INNER + (OUTER - INNER) * streaks.seed.next_f64().sqrt();
        let scale = 0.45 + 1.1 * streaks.seed.next_f64() as f32;
        let age = streaks.seed.next_f64() as f32 * LIFE;
        streaks.live.push(Streak {
            at: st.pos + glam::DVec2::new(a.sin(), a.cos()) * r,
            age,
            scale,
        });
    }

    let Some(mut mesh) = meshes.get_mut(&streaks.mesh) else {
        return;
    };
    let mut positions = Vec::with_capacity(COUNT * 4);
    let mut colours = Vec::with_capacity(COUNT * 4);
    let mut uvs = Vec::with_capacity(COUNT * 4);

    for i in 0..COUNT {
        let Some(s) = streaks.live.get(i) else {
            // Unused streaks collapse onto the boat, transparent. Not onto the world
            // origin — that mistake drew a sheet across the sea once already.
            let here = origin.to_bevy(st.pos, 0.0);
            for _ in 0..4 {
                positions.push(here);
                colours.push([1.0, 1.0, 1.0, 0.0]);
                uvs.push(Vec2::ZERO);
            }
            continue;
        };
        let flow = flow_at(s.at);
        let speed = flow.length().max(0.01);
        let along = flow / speed;
        let across = glam::DVec2::new(-along.y, along.x);
        // Longer in more wind and over its own life, and a hair wider than it needs to
        // be or a one-pixel line crawls when it moves.
        let length = (1.4 + speed * 0.42) * s.scale as f64;
        let width = 0.075 * s.scale as f64;

        // In and out again: nothing appears or vanishes, it arrives and passes.
        let life = (s.age / LIFE).clamp(0.0, 1.0);
        let fade = (life * std::f32::consts::PI).sin();
        // And out with distance, so the ring has no edge to it.
        let range = (s.at - st.pos).length() as f32;
        let far = 1.0 - ((range - OUTER as f32 * 0.6) / (OUTER as f32 * 0.5)).clamp(0.0, 1.0);
        let alpha = fade * far * 0.30;

        for (u, v) in [(-0.5, -1.0), (0.5, -1.0), (0.5, 1.0), (-0.5, 1.0)] {
            let at = s.at + along * length * v * 0.5 + across * width * u;
            let height = sim.sim.water.height(at, t) as f32 + 0.05;
            let p = origin.to_bevy(at, height);
            positions.push(p);
            // Brighter in the middle of the streak than at its ends.
            let taper = 1.0 - (v as f32).abs() * 0.65;
            colours.push([1.0, 1.0, 1.0, alpha * taper]);
            uvs.push(Vec2::new(u as f32 + 0.5, v as f32 * 0.5 + 0.5));
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
}
