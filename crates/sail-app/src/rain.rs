//! Rain, and the visibility going with it.
//!
//! Drawn as streaks rather than drops, which is both what a camera sees and what a pen
//! draws. A raindrop falls at six or seven metres a second; over the time an eye or a
//! shutter integrates, it is a line and not a point. Manga rain is parallel diagonal
//! strokes for exactly this reason, so the stylised version and the physical one are the
//! same picture — which does not happen often enough to pass up.
//!
//! The angle is the wind's. Rain in twenty-five knots does not fall, it arrives: the
//! horizontal part of a streak is the wind and the vertical part is gravity, and the
//! slope between them is the most immediate reading of wind strength there is, because
//! you do not have to look at an instrument to see it. On a dead calm day it is
//! vertical. That one line does the work of a wind gauge.
//!
//! Where it rains is not a choice made here. [`sail_core::wind::WindField::squall_at`]
//! says how deep in a squall a point is, and the same number drives the extra wind in
//! the sails, the darkness of the cloud overhead and the visibility closing in. A squall
//! you can see but not feel is a screensaver.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::sim::Sim;

/// Streaks alive at once. They are thin and cheap, and rain is the one thing in the
/// frame that should look like too much of it.
const COUNT: usize = 1400;
/// The box they live in, metres: a cylinder about the eye. Rain is only ever drawn near
/// the camera — at a hundred metres it is haze, and the haze does it better and for
/// nothing.
const REACH: f32 = 46.0;
const CEILING: f32 = 30.0;
/// Terminal velocity of a raindrop with some size to it, m/s. Drizzle falls at two and
/// a thunderstorm drop at nine; squall rain is about here.
const FALL: f32 = 7.2;
/// How long a streak is for each metre per second it is travelling. The shutter, in
/// effect — and it is turned up a little past a camera's, because a streak a hand long
/// reads as rain while a true one at this resolution reads as grain.
const SMEAR: f32 = 0.16;

struct Drop {
    /// Metres from the eye, horizontally, and height above the water.
    at: Vec3,
    /// Its own size, so the curtain has depth in it rather than being one flat grid.
    scale: f32,
    seed: f32,
}

#[derive(Resource)]
pub struct Rain {
    mesh: Handle<Mesh>,
    drops: Vec<Drop>,
    rng: sail_core::rng::Pcg32,
    /// How hard it is raining here, 0 to 1, eased so the curtain arrives over a second
    /// or two rather than switching on at the squall's edge.
    pub fall: f32,
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
            // Added, like the foam and for the same reason: a streak is a bright thing
            // against a dark sea and a dark thing against a bright sky, and the one
            // mistake that reads as wrong immediately is rain that *darkens* the water
            // it is falling into.
            alpha_mode: AlphaMode::Add,
            unlit: true,
            cull_mode: None,
            double_sided: true,
            ..default()
        })),
        Transform::default(),
        bevy::camera::visibility::NoFrustumCulling,
    ));

    let mut rng = sail_core::rng::Pcg32::new(0x5A1_1A1);
    let drops = (0..COUNT)
        .map(|_| Drop {
            at: scatter(&mut rng, CEILING),
            scale: 0.6 + rng.next_f64() as f32 * 0.9,
            seed: rng.next_f64() as f32,
        })
        .collect();

    commands.insert_resource(Rain {
        mesh: handle,
        drops,
        rng,
        fall: 0.0,
    });
}

/// Somewhere in the cylinder, up to `top` metres above the water.
fn scatter(rng: &mut sail_core::rng::Pcg32, top: f32) -> Vec3 {
    let a = rng.next_f64() as f32 * std::f32::consts::TAU;
    // Square-rooted, so the drops are spread evenly over the area rather than piling up
    // round the eye — which is what a linear radius does, and it makes the curtain
    // thickest exactly where it blocks the view.
    let r = (rng.next_f64() as f32).sqrt() * REACH;
    Vec3::new(r * a.cos(), rng.next_f64() as f32 * top, r * a.sin())
}

pub fn update(
    ink: Res<crate::world::Ink>,
    sim: Res<Sim>,
    sky: Res<crate::sky::Sky>,
    origin: Res<crate::frame::RenderOrigin>,
    time: Res<Time>,
    mut rain: ResMut<Rain>,
    mut meshes: ResMut<Assets<Mesh>>,
    camera: Query<&Transform, With<crate::camera::MainCamera>>,
) {
    let Ok(view) = camera.single() else {
        return;
    };
    let handle = rain.mesh.clone();
    let st = &sim.render_state();
    let dt = time.delta_secs();

    // How hard it is raining where the boat is. Eased, because a squall's edge is a
    // kilometre of thickening drizzle and not a line you cross.
    //
    // **Squalls were the only thing that made it rain, and that is not this coast.** A
    // squall is the dramatic case — a cell you can see coming, with a hard edge and
    // four hundred metres of visibility inside it — but the ordinary way it rains at
    // fifty-seven north is a sky that has been solid for hours quietly drizzling on
    // you. A sky at eight oktas that never produces a drop is the kind of wrong that
    // nobody points at and everybody feels.
    //
    // So overcast contributes too, well under what a squall does: it starts only when
    // the cover is nearly complete, and at its hardest it is a third of squall rain.
    // Enough to wet the deck, soften the horizon and put streaks in the air; not
    // enough to be mistaken for weather you should be reefing for.
    let squall = sim.sim.wind.squall_at(st.pos) as f32;
    let overcast = ((sky.cover - 0.82) / 0.18).clamp(0.0, 1.0) * 0.34;
    let want = squall.max(overcast);
    let ease = (dt / 2.5).min(1.0);
    rain.fall += (want - rain.fall) * ease;

    let Some(mut mesh) = meshes.get_mut(&handle) else {
        return;
    };
    let fall = rain.fall;
    // The wind at the masthead carries it. Sampled once: a drop does not need its own
    // reading of a field that is uniform over forty metres.
    let wind = sim.sim.wind.sample(st.pos, st.time);
    let drift = Vec3::new(wind.x as f32, 0.0, -wind.y as f32);
    let velocity = drift - Vec3::Y * FALL;
    let along = velocity.normalize_or_zero();
    let length = velocity.length() * SMEAR;

    let mut positions = Vec::with_capacity(COUNT * 4);
    let mut colours = Vec::with_capacity(COUNT * 4);
    let mut uvs = Vec::with_capacity(COUNT * 4);

    // The eye, on the water plane: the curtain follows the camera rather than the boat,
    // because it is drawn for the eye and the eye can be a long way off the transom.
    let eye = Vec3::new(view.translation.x, 0.0, view.translation.z);
    let live = (COUNT as f32 * fall.clamp(0.0, 1.0)) as usize;

    for i in 0..COUNT {
        if i >= live {
            // Parked, transparent, at the eye. Never at the world origin, which draws a
            // sheet from here to wherever that happens to be.
            for _ in 0..4 {
                positions.push(eye);
                colours.push([1.0, 1.0, 1.0, 0.0]);
                uvs.push(Vec2::ZERO);
            }
            continue;
        }
        // The pool and the generator are taken apart because a drop and the dice that
        // reborn it cannot both be borrowed from the same resource at once.
        let Rain { drops, rng, .. } = &mut *rain;
        let drop = &mut drops[i];
        drop.at += velocity * dt;
        // Round the cylinder: a drop that leaves the far side comes back on the near
        // one, which keeps the curtain even however fast the wind is blowing it along.
        let flat = Vec2::new(drop.at.x, drop.at.z);
        // A DROP LANDS ON THE SEA, NOT ON SEA LEVEL. This cut the streak at y = 0, which
        // is the water's *mean* — so on any sea at all the drop went on falling through
        // the face of a crest and was drawn for another few centimetres underneath it.
        // At half a metre of swell that is a curtain of rain visibly inside the water,
        // which is the one place rain never is.
        //
        // Against the surface that is DRAWN, like the foam and the wake: the mesh is
        // coarser than the field far from the boat, and a drop has to disappear into the
        // skin the eye can see, not into the one the physics knows about.
        let world_pos = glam::DVec2::new(
            (eye.x + drop.at.x) as f64 + origin.offset.x,
            -((eye.z + drop.at.z) as f64) + origin.offset.y,
        );
        let from_boat = world_pos - st.pos;
        let resolution =
            crate::world::drawn_resolution(Vec2::new(from_boat.x as f32, -from_boat.y as f32));
        let surface = sim
            .sim
            .water
            .height_resolved(world_pos, st.time, resolution) as f32;
        if drop.at.y < surface || flat.length() > REACH * 1.25 {
            let born = scatter(rng, CEILING);
            // Reborn at the top of the box when it has landed, and anywhere in it when
            // it has blown out the side — otherwise a strong wind empties the upwind
            // half of the cylinder and the rain only falls to leeward of you.
            drop.at = if drop.at.y < surface {
                Vec3::new(born.x, CEILING, born.z)
            } else {
                born
            };
        }

        let centre = Vec3::new(eye.x + drop.at.x, drop.at.y, eye.z + drop.at.z);

        // A streak is a line, so it is drawn as a ribbon along its own velocity, turned
        // to face the eye about that axis.
        let to_eye = (view.translation - centre).normalize_or_zero();
        let across = along.cross(to_eye).normalize_or_zero() * (0.008 * drop.scale);
        let half = along * (length * 0.5 * drop.scale);

        // Brightness: rain is lit by the sky, so it is pale, and it flickers a little
        // because a falling drop turns and catches the light unevenly.
        let flicker = 0.75 + 0.25 * (st.time as f32 * 11.0 + drop.seed * 20.0).sin();
        let mut alpha = 0.42 * fall * flicker;
        let mut tone = Vec3::new(0.86, 0.91, 0.97);
        if ink.0 {
            // Drawn rain is a stroke: one weight, no gradient, and more of them. On the
            // page this is the whole of how rain is done.
            alpha = if fall > 0.08 { 0.60 } else { 0.0 };
            tone = Vec3::new(0.97, 0.99, 1.0);
        }
        for (u, v) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            positions.push(centre + half * u + across * v);
            colours.push([tone.x, tone.y, tone.z, alpha]);
            uvs.push(Vec2::new(u * 0.5 + 0.5, v * 0.5 + 0.5));
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
}

/// The visibility, which is the other half of rain and the half that matters at sea.
///
/// You do not fear a squall because of the water coming out of the sky; you fear it
/// because the island you were steering for goes away. Down to about four hundred
/// metres in the heart of one, which is a real figure for heavy rain and quite enough
/// to lose a coastline in.
///
/// **What it closes *from* is the weather's own visibility, not a constant.** This used
/// to start at a hard 2600 m and it ran every frame, so it quietly overwrote whatever
/// the camera had been given — the clear-weather visibility could be changed anywhere
/// else in the program and nothing would happen, because this put it back. Two numbers
/// for one quantity, and the one that won was the one nobody was editing.
pub fn shroud(
    sky: Res<crate::sky::Sky>,
    rain: Res<Rain>,
    mut fog: Query<&mut DistanceFog, With<crate::camera::MainCamera>>,
) {
    let Ok(mut fog) = fog.single_mut() else {
        return;
    };
    let shut = rain.fall.clamp(0.0, 1.0);
    let clear_range = crate::sky::visibility(&sky);
    let visibility = clear_range * (1.0 - shut) + 420.0 * shut;
    // Darker as well as thicker. Rain does not only hide the distance, it takes the
    // light out of it, and a squall that closes the view without dimming it looks like
    // fog on a sunny day.
    let clear = crate::sky::haze_colour(&sky).to_linear();
    let wet = Color::linear_rgb(clear.red * 0.62, clear.green * 0.66, clear.blue * 0.74);
    let colour = mix(crate::sky::haze_colour(&sky), wet, shut);
    fog.color = colour;
    // The forward scattering goes with it: rain kills the glow as well as the distance,
    // because the light that would have scattered towards you is being absorbed on the
    // way. Keeping it lit through a squall is what makes rain look like a grey filter.
    fog.directional_light_color = {
        let g = crate::sky::glow_colour(&sky).to_linear();
        let keep = 1.0 - shut * 0.85;
        Color::linear_rgb(g.red * keep, g.green * keep, g.blue * keep)
    };
    fog.falloff =
        FogFalloff::from_visibility_colors(visibility, crate::sky::haze_near(&sky), colour);
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    let (a, b) = (a.to_linear(), b.to_linear());
    Color::linear_rgb(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
    )
}
