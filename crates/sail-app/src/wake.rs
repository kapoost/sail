//! The wake.
//!
//! Not decoration. A boat with no wake has displaced no water, and the eye reads that
//! as weightless however heavy the simulation says it is — the complaint that nothing
//! has inertia was partly this. A trail of disturbed water behind the hull is the
//! cheapest and strongest statement a vessel can make about its own mass.
//!
//! Two things, because a wake is two things and drawing it as one was the fault.
//!
//! **Close astern it is foam**, and foam is not a stripe. It is rafts of bubbles lying
//! flat on the water with holes all through them, each one dissolving over half a minute
//! while the whole band drifts slowly apart — so that is what it is made of here: long
//! lived particles, laid at the transom and left in the water where they were put.
//! Reported from the deck as the white streak not looking real enough, with the remedy in
//! the same breath: make it out of particles, stick them behind the boat, and only leave
//! a white trail further off, beyond their reach, if it is worth it.
//!
//! **Further off it is a slick**, and that is the ribbon. What survives when the bubbles
//! have gone is water that has been smoothed: no foam in it at all, just a paler, flatter
//! strip that holds its look for a minute or more in a seaway and is how you find your
//! own track from the helm. It was worth keeping — but faint, and only where the foam has
//! thinned, which is why it now fades in by the *age* of the water rather than at some
//! distance in metres. Age scales with speed by itself: fast and the foam reaches a long
//! way astern before the slick takes over, slow and it does not.
//!
//! Both are laid down in world coordinates at intervals of distance rather than of time,
//! so they mark where the boat *went* instead of how long it spent there, and both ride
//! the water surface they were laid on rather than floating flat across the waves.

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

/// Rafts of foam alive at once.
///
/// At forty a second and half a minute each this is the cap rather than the rate, which
/// is the way round it should be: the band is as long as the foam lasts, and what fills
/// it is however many patches that takes.
const RAFTS: usize = 1100;
/// Multiply the foam budget, for finding out what it costs: `SAIL_WAKE=3`.
fn rafts() -> usize {
    let scale: f32 = crate::knob::var("SAIL_WAKE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0);
    ((RAFTS as f32) * scale.clamp(0.1, 20.0)) as usize
}
/// Rafts laid a second at full speed.
const BIRTHS: f32 = 40.0;
/// How long one lasts, seconds, drawn per patch.
///
/// Long, which is the whole request: the foam astern of a boat under way is still there
/// half a minute later, thinner and wider and greyer, and a particle system with a second
/// and a half of life in it — which is right for spray off the bow — cannot draw that.
const RAFT_LIFE: (f32, f32) = (15.0, 34.0);
/// And the mean of it, which is what the slick measures its own fade against.
const RAFT_MEAN: f32 = (RAFT_LIFE.0 + RAFT_LIFE.1) * 0.5;

/// A raft of foam in the water, lying where it was left.
#[derive(Clone, Copy)]
struct Raft {
    at: glam::DVec2,
    /// Outward, mostly: a wake widens behind a boat because the turbulence in it spreads,
    /// and a patch that walks slowly away from the centreline is what draws that.
    drift: glam::DVec2,
    age: f32,
    life: f32,
    /// Metres across at birth; it grows from there as the air comes out of it.
    size: f32,
    /// Which of the four marks, and how far round it is turned. Foam has no grain, so a
    /// patch laid down square to the boat every time is the one thing that would give the
    /// repeat away.
    mark: u32,
    turn: f32,
    /// How bright this one is, because a band of identical patches is a texture.
    bright: f32,
    /// Speed through the water when it was laid, so a boat ghosting along leaves little.
    strength: f32,
}

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
    foam: Handle<Mesh>,
    live: Vec<Raft>,
    seed: sail_core::rng::Pcg32,
    /// Fractional rafts carried between frames, so a rate of forty and a half a second
    /// does not round down and lose its half for ever.
    owed: f32,
    /// Where the transom was when the last one was laid, so they are spread evenly along
    /// the water she has passed over rather than stacked up wherever the frames fell.
    trail: Option<glam::DVec2>,
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
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

    // And the foam itself: one quad a raft, in one mesh, rebuilt every frame.
    let mut rafts_mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    let corners = rafts() * 4;
    rafts_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![Vec3::ZERO; corners]);
    rafts_mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![Vec3::Y; corners]);
    rafts_mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![Vec2::ZERO; corners]);
    rafts_mesh.insert_attribute(
        Mesh::ATTRIBUTE_COLOR,
        vec![[1.0_f32, 1.0, 1.0, 0.0]; corners],
    );
    let mut raft_indices = Vec::with_capacity(rafts() * 6);
    for i in 0..rafts() {
        let a = (i * 4) as u32;
        raft_indices.extend_from_slice(&[a, a + 1, a + 2, a, a + 2, a + 3]);
    }
    rafts_mesh.insert_indices(Indices::U32(raft_indices));
    let foam_handle = meshes.add(rafts_mesh);
    commands.spawn((
        Mesh3d(foam_handle.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            // The shape of a raft rather than the square it is drawn on, and added
            // rather than blended for the same reasons the spray is: foam is bright
            // water, two layers of it are brighter than one, and additive marks need no
            // sorting and can never darken the sea they lie on.
            base_color_texture: Some(images.add(crate::sea_texture::wake_rafts())),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        })),
        Transform::default(),
    ));

    commands.insert_resource(Wake {
        mesh: handle,
        samples: Vec::with_capacity(SAMPLES),
        last: None,
        foam: foam_handle,
        live: Vec::with_capacity(rafts()),
        seed: sail_core::rng::Pcg32::new(0x5E_A0_F1),
        owed: 0.0,
        trail: None,
    });
}

/// What she is actually tearing up, 0 to 1.
///
/// Squared rather than linear, because that is how the energy she leaves in the water
/// grows, and because it keeps a boat drifting at two knots from laying the same white
/// scar as one driving at eight. One function, because this was written out twice the
/// moment the foam and the slick stopped being the same thing — which is the fault this
/// repository keeps finding in itself.
fn tearing_up(sim: &Sim) -> f32 {
    let knots = sail_core::units::ms_to_kn(sim.sim.telemetry.stw).max(0.0);
    ((knots / 6.5).powi(2)).clamp(0.0, 1.0) as f32
}

/// Lay rafts of foam at the transom, and let the ones already in the water get on with
/// dissolving.
///
/// Laid along the water she has just passed over rather than all at the transom, so the
/// band is evenly filled whatever the frame rate — the alternative is a clump of foam
/// wherever a frame happened to fall, which at thirty frames and eight knots is a
/// stripe of blobs a quarter of a metre apart.
fn lay_foam(wake: &mut Wake, sim: &Sim, stern: glam::DVec2, strength: f32, dt: f32) {
    let st = &sim.render_state();
    for raft in wake.live.iter_mut() {
        raft.age += dt;
        raft.at += raft.drift * dt as f64;
        // The spread slows. Turbulence runs out of the energy that was widening it, and
        // foam that kept walking outwards at its birth rate would draw a wake a hundred
        // metres wide.
        raft.drift *= 1.0 - (0.06 * dt) as f64;
    }
    wake.live.retain(|r| r.age < r.life);

    let travelled = wake.trail.map_or(0.0, |t| (stern - t).length());
    wake.trail = Some(stern);
    wake.owed += BIRTHS * strength * dt;
    let want = wake.owed.floor();
    wake.owed -= want;
    let beam = sim.config().beam;
    let speed = sim.sim.telemetry.stw.max(0.0);
    let r = &mut wake.seed;
    for _ in 0..(want as usize) {
        if wake.live.len() >= rafts() {
            break;
        }
        // Across the transom, with a thinner patch down the middle: the water right
        // behind a transom is drawn up and smooth, and the churn is either side of it
        // where the quarters shed their own wave.
        let side = if r.next_f64() < 0.5 { -1.0 } else { 1.0 };
        let across = side * (0.2 + 0.8 * r.next_f64()) * beam * 0.5;
        let back = r.next_f64() * travelled.min(4.0);
        let at = stern + st.starboard() * across - st.forward() * back;
        let out = 0.16 * speed * (0.35 + 1.3 * r.next_f64());
        wake.live.push(Raft {
            at,
            drift: st.starboard() * side * out + st.forward() * speed * 0.03,
            age: 0.0,
            life: RAFT_LIFE.0 + (RAFT_LIFE.1 - RAFT_LIFE.0) * r.next_f64() as f32,
            size: 0.34 + 0.46 * r.next_f64() as f32,
            mark: (r.next_f64() * 4.0) as u32 % 4,
            turn: r.next_f64() as f32 * std::f32::consts::TAU,
            bright: 0.62 + 0.38 * r.next_f64() as f32,
            strength,
        });
    }
}

/// And draw them: flat on the water, which is what a raft of foam is.
///
/// Flat rather than facing the eye, unlike the spray — a drop of water thrown off the
/// bow is a drop from any angle, but a patch of foam is a thing lying on a surface, and
/// billboarding it is what makes foam look like a decal standing up out of the sea.
fn draw_foam(
    wake: &Wake,
    sim: &Sim,
    origin: &frame::RenderOrigin,
    ink: bool,
    meshes: &mut Assets<Mesh>,
) {
    let Some(mut mesh) = meshes.get_mut(&wake.foam) else {
        return;
    };
    let st = &sim.render_state();
    let t = st.time;
    let budget = rafts();
    let mut positions = Vec::with_capacity(budget * 4);
    let mut colours = Vec::with_capacity(budget * 4);
    let mut uvs = Vec::with_capacity(budget * 4);
    // The sea's own colour, which is what old foam turns into: the bubbles go and what
    // is left is water with a little white still in it.
    let sea = Vec3::new(0.66, 0.80, 0.92);
    for raft in wake.live.iter().take(budget) {
        let gone = (raft.age / raft.life).clamp(0.0, 1.0);
        // It grows while it fades, which is air coming out of it and the patch coming
        // apart. The two together are most of what makes foam look like foam rather
        // than like something being turned off.
        let size = raft.size * (1.0 + 1.6 * gone);
        // Two terms again, and here they belong: the bubbles go quickly and the raft
        // they were in holds its shape a while longer.
        let bubbles = (1.0 - gone).powf(2.4);
        let body = (1.0 - gone).powf(0.7);
        let mut alpha = ((0.80 * bubbles + 0.34 * body) * raft.bright * raft.strength).min(0.95);
        let mut colour = Vec3::ONE.lerp(sea, gone * 0.85);
        if ink {
            // The drawn sea has a few tones and nothing between them, and the foam has
            // to be one of them. Three steps: the white of the churn, the grey of what
            // is left of it, and nothing.
            alpha = match alpha {
                x if x > 0.45 => 0.85,
                x if x > 0.15 => 0.40,
                _ => 0.0,
            };
            colour = Vec3::new(0.97, 0.99, 1.0);
        }
        // On the surface that is *drawn*, by the same definition the sea and the spray
        // read — see the ribbon below and `spray.rs` at length.
        let offset = raft.at - st.pos;
        let resolution =
            crate::world::drawn_resolution(Vec2::new(offset.x as f32, -offset.y as f32));
        let height = sim.sim.water.height_resolved(raft.at, t, resolution);
        let centre = origin.to_bevy(raft.at, height as f32 + 0.06);
        let (sin, cos) = raft.turn.sin_cos();
        for (u, v) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            let (x, z) = (u * cos - v * sin, u * sin + v * cos);
            positions.push(centre + Vec3::new(x * size, 0.0, z * size));
            colours.push([colour.x, colour.y, colour.z, alpha]);
            // Which of the four marks on the sheet, in a two by two grid.
            let cell = Vec2::new((raft.mark % 2) as f32, (raft.mark / 2) as f32) * 0.5;
            uvs.push(cell + Vec2::new((u * 0.5 + 0.5) * 0.5, (v * 0.5 + 0.5) * 0.5));
        }
    }
    // Everything unspent collapses onto one point, out of sight under the boat: a quad
    // with no area draws nothing, and leaving the tail at the world origin is what once
    // drew a grey slab from the hull to the middle of the chart.
    let nowhere = origin.to_bevy(st.pos, -40.0);
    for _ in wake.live.len()..budget {
        for _ in 0..4 {
            positions.push(nowhere);
            colours.push([1.0, 1.0, 1.0, 0.0]);
            uvs.push(Vec2::ZERO);
        }
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
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
        wake.samples.push(Sample {
            at: stern,
            across: st.starboard(),
            born: now,
            strength: tearing_up(&sim),
        });
        wake.last = Some(stern);
        if wake.samples.len() > SAMPLES {
            wake.samples.remove(0);
        }
    }

    // The foam, which is laid every frame rather than every half metre: it is particles
    // in the water, not a strip of mesh, and a frame that has not moved far enough to
    // deserve a new ribbon sample has still moved.
    lay_foam(&mut wake, &sim, stern, tearing_up(&sim), time.delta_secs());
    draw_foam(&wake, &sim, &origin, ink.0, &mut meshes);

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
        // Where the foam has thinned, and nowhere nearer. The rafts draw the white
        // water; this draws what is left when they have gone, so it has to start where
        // they stop — and measured in the age of the water rather than in metres astern,
        // because then it scales with her speed by itself.
        let beyond = crate::curve::smoothstep(RAFT_MEAN * 0.45, RAFT_MEAN * 0.95, age);
        // A wake spreads as it ages and thins as it spreads.
        let half_width = beam * (0.75 + 1.6 * (age as f64 / LIFETIME as f64).min(1.0));
        // One fade, and it is the slow one. There used to be a second, fast term in
        // here for the foam, and that term *was* the white streak: a smooth sheet of
        // paint laid over the water for twenty metres astern of the transom. The foam is
        // rafts now, so what is left for the ribbon is the slick — paler, flatter water
        // with no bubbles in it — and it is drawn faint because that is how faint it is.
        let track = life.powf(0.55);
        let alpha = track * 0.26 * beyond * sample.strength;

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
                    x if x > 0.13 => 0.34,
                    x if x > 0.05 => 0.16,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The slick starts where the foam has thinned, and both fit inside their own buffers.
    ///
    /// Three numbers decide whether the wake has a hole in it — the life of a raft, the
    /// fade-in of the slick, and how long a ribbon sample lives — and they are in three
    /// places. If the slick's fade-in ran past the ribbon's own lifetime there would be
    /// no slick at all; if it began at nought the ribbon would be painting white over the
    /// foam again, which is the whole thing that was reported.
    #[test]
    // Constant assertions on purpose: these three numbers are the ones somebody tuning
    // the look of the foam will reach for, and what they must not do is drift past each
    // other. A test that can only fail when an edit happens is exactly the test for that.
    #[allow(clippy::assertions_on_constants)]
    fn the_foam_and_the_slick_meet() {
        assert!(
            RAFT_MEAN * 0.45 > 4.0,
            "the slick starts at {:.1} s, which is on top of the foam",
            RAFT_MEAN * 0.45
        );
        assert!(
            RAFT_MEAN * 0.95 < LIFETIME,
            "the slick finishes fading in at {:.1} s and the ribbon is gone at {LIFETIME:.1}",
            RAFT_MEAN * 0.95
        );
        // And the buffer holds a full band: the cap should be the thing that never binds,
        // or the foam is cut off before the water has finished settling.
        assert!(
            BIRTHS * RAFT_MEAN <= rafts() as f32,
            "forty rafts a second for {RAFT_MEAN:.0} seconds is {} patches and the budget \
             is {}",
            (BIRTHS * RAFT_MEAN) as usize,
            rafts()
        );
    }

    /// A raft is a lacy patch, not a disc: bright in the middle, gone in the corners, and
    /// full of holes in between.
    ///
    /// The holes are the point. A mark with none is paint however well its rim is drawn,
    /// and "the white streak does not look real" is what a field of solid discs reads as
    /// just the same as a solid stripe does.
    #[test]
    fn a_raft_of_foam_has_holes_in_it() {
        for which in 0..4 {
            let seed = which as f32;
            assert!(
                crate::sea_texture::raft_mark(0.0, 0.0, seed) > 0.9,
                "raft {which} is not solid in the middle"
            );
            assert_eq!(
                crate::sea_texture::raft_mark(0.98, 0.98, seed),
                0.0,
                "raft {which} reaches into its own corner, which is a square"
            );
            // Somewhere between the middle and the rim it has to be open, and somewhere
            // else closed, or there is no lace in it.
            let mut most = 0.0_f32;
            let mut least = 1.0_f32;
            for i in 0..24 {
                let a = i as f32 / 24.0 * std::f32::consts::TAU;
                let v = crate::sea_texture::raft_mark(a.cos() * 0.62, a.sin() * 0.62, seed);
                most = most.max(v);
                least = least.min(v);
            }
            assert!(
                most - least > 0.25,
                "raft {which} is the same all the way round at {most:.2}, which is a disc"
            );
        }
    }
}
