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
const GRID: usize = 201;
/// How far out the grid reaches. Three kilometres is past the horizon from a cockpit,
/// and it costs almost nothing because the outer rings carry no waves at all — see
/// `resolutions`.
const EXTENT: f32 = 3000.0;
/// Where the wave-carrying part of the grid ends and the flat run to the horizon
/// begins, metres.
const NEAR_FIELD: f32 = 209.0;
/// Vertex spacing at the boat, and at the edge of the near field.
const SPACING_NEAR: f32 = 1.30;
const SPACING_FAR: f32 = 4.20;
/// How many rings make up the near field; the rest sprint to the horizon.
const NEAR_RINGS: usize = 76;

#[derive(Component)]
pub struct SeaSurface {
    mesh: Handle<Mesh>,
    /// Grid offsets in the Bevy horizontal plane, metres, precomputed once.
    offsets: Vec<Vec2>,
    /// Shortest wavelength this vertex can carry: twice its spacing from its
    /// neighbours, by Nyquist. Precomputed, because the grid never changes shape.
    ///
    /// The grid is graded — a vertex every 0.7 m under the boat, every 25 m at the
    /// rim — which is the right way to spend a hundred and sixty squared vertices.
    /// But a 4.5 second sea has a 32 metre wavelength, and 32 metres sampled every 25
    /// is not a wave, it is noise that crawls as the grid slides along underneath the
    /// boat. That crawl was the shimmer in the distance. Asking the water only for
    /// what each vertex can actually hold removes it at the source.
    resolutions: Vec<f64>,
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
            // Real sunlight, now that the camera is exposed for it. Thirty-two
            // thousand lux is a cloudy afternoon, and the scene was lit for one.
            illuminance: 85_000.0,
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
        brightness: 300.0,
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

    // Spacing chosen from what the sea is, not from a convenient curve.
    //
    // A twelve-knot sea peaks around a thirty-two metre wavelength, and a *moving*
    // surface needs eight to ten samples across a wave before the crests stop beating
    // between the vertices as the grid slides along under the boat. The old cubic
    // grading held eight samples out to fifty-one metres and nothing like it beyond,
    // which is why the sea kept jumping however much else was fixed: two samples per
    // wave is Nyquist, and Nyquist is the condition for *reconstructing* a signal, not
    // for animating one.
    //
    // So: spacing opens gently from 1.3 m at the boat to 4.2 m at two hundred metres,
    // which holds eight samples out to a hundred and eighty, and then the last rings
    // sprint to three kilometres. Those outer rings cost almost nothing, because they
    // are too coarse to carry a wave and `update_sea` does not even ask the water
    // about them.
    let half = |i: usize| -> f32 {
        if i <= NEAR_RINGS {
            // Arithmetic spacing: the sum of a ramp, in closed form.
            let n = i as f32;
            let last = (NEAR_RINGS - 1) as f32;
            SPACING_NEAR * n + (SPACING_FAR - SPACING_NEAR) * n * (n - 1.0) / (2.0 * last)
        } else {
            let rest = (GRID / 2 - NEAR_RINGS) as f32;
            let step = (i - NEAR_RINGS) as f32;
            NEAR_FIELD * (EXTENT / NEAR_FIELD).powf(step / rest)
        }
    };
    let centre = (GRID / 2) as i32;
    let grade = |i: usize| -> f32 {
        let k = i as i32 - centre;
        half(k.unsigned_abs() as usize) * (k.signum() as f32)
    };

    let mut offsets = Vec::with_capacity(GRID * GRID);
    // Spacing to the wider neighbour, per axis; the coarser axis decides.
    let spacing = |i: usize| -> f32 {
        let lo = grade(i.saturating_sub(1));
        let hi = grade((i + 1).min(GRID - 1));
        let span = hi - lo;
        let steps = ((i + 1).min(GRID - 1) - i.saturating_sub(1)).max(1) as f32;
        span / steps
    };
    let mut resolutions = Vec::with_capacity(GRID * GRID);
    for ix in 0..GRID {
        for iz in 0..GRID {
            offsets.push(Vec2::new(grade(ix), grade(iz)));
            let coarsest = spacing(ix).max(spacing(iz));
            // Four times the spacing, not twice. Twice is the Nyquist limit, at which
            // a component is kept at full strength with four samples across it — and
            // four samples is where a moving sea visibly beats. This rolls a component
            // off between four and eight samples, so nothing is drawn that the mesh
            // cannot animate smoothly.
            resolutions.push(4.0 * coarsest as f64);
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
            resolutions,
        },
        Mesh3d(handle),
        MeshMaterial3d(water),
        Transform::default(),
    ));
}

/// Displace the surface from the physics' own wave field, and keep it under the boat.
/// Where the surface detail has drifted to, integrated frame by frame.
#[derive(Resource, Default)]
pub struct SurfaceDrift {
    offset: glam::DVec2,
}

pub fn update_sea(
    origin: Res<frame::RenderOrigin>,
    time: Res<Time>,
    mut drift: ResMut<SurfaceDrift>,
    sim: Res<Sim>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut q: Query<(&SeaSurface, &mut Transform)>,
) {
    let st = &sim.render_state();
    let centre = origin.to_bevy(st.pos, 0.0);
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
        let typical = (sim.sim.water.characteristic_slope() as f32).max(1e-4);
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
        // Two motions, both integrated rather than evaluated.
        //
        // The slow one is a real surface current, about two and a half percent of the
        // wind speed. The fast one is the ripples' own travel: the normal map stands
        // for short waves a few metres long, and those move through the water at
        // their own phase speed of nearly 3 m/s. Advecting at the drift rate alone is
        // what made the surface read as a photograph of a sea rather than a sea.
        //
        // `offset += direction * rate * dt`, and the `dt` is the whole point. Written
        // the obvious way — `direction * rate * elapsed` — the offset is a vector
        // whose length grows without bound multiplied by a direction that wanders
        // with every gust, so a one-degree windshift five minutes in swings an
        // eight-hundred-metre vector and the texture jumps fifteen metres sideways.
        // That is the same mistake as the very first texture bug in this file, and
        // raising the ripple speed tenfold made it ten times worse. Integrating the
        // rate means a windshift changes where the water is going from now on,
        // instead of rewriting where it has been.
        let speed = tm.tws * 0.025 + sea_texture::ripple_phase_speed() as f64;
        drift.offset += downwind * speed * time.delta_secs_f64();
        let current = drift.offset;
        // Rebase the texture coordinates by a whole number of tile repeats.
        //
        // The UVs address the sea in world metres so that the detail stays with the
        // water rather than with the boat. That is right, and it is also the thing
        // that breaks first on a long passage: twenty minutes out, the numbers are in
        // the hundreds, the GPU interpolates them in single precision across each
        // triangle, and the quantisation reads as the surface detail stuttering and
        // racing. Taking off whole repeats costs nothing and cannot shift the pattern,
        // because a whole repeat is where the pattern comes back to itself. The
        // subtraction is identical for every vertex, so no triangle is torn.
        let tile = sea_texture::TILE_METRES as f64;
        let anchor = st.pos - current;
        let uv_base = glam::DVec2::new(
            (anchor.x / tile).round() * tile,
            (anchor.y / tile).round() * tile,
        );

        // Past this spacing the mesh carries nothing, so the water is never asked.
        // On a grid that runs to the horizon that is most of the vertices, and it is
        // what pays for the fine spacing close in.
        let longest = sim.sim.water.longest_wavelength();
        for (offset, &min_wavelength) in sea.offsets.iter().zip(&sea.resolutions) {
            if min_wavelength >= longest {
                positions.push(Vec3::new(offset.x, 0.0, offset.y));
                normals.push(Vec3::Y);
                tangents.push(Vec4::new(1.0, 0.0, 0.0, -1.0));
                let flat = st.pos + glam::DVec2::new(offset.x as f64, -offset.y as f64) - uv_base;
                uvs.push(Vec2::new(
                    flat.x as f32 / sea_texture::TILE_METRES,
                    -flat.y as f32 / sea_texture::TILE_METRES,
                ));
                let deep = Vec3::new(0.035, 0.095, 0.150);
                colours.push([deep.x, deep.y, deep.z, 1.0]);
                continue;
            }
            // Sample at the true world position. Bevy's -Z is sim north, so the
            // horizontal offset flips sign on the way across.
            let world = st.pos + glam::DVec2::new(offset.x as f64, -offset.y as f64);
            // Band-limited to what this vertex can carry — see `resolutions`.
            let height = sim.sim.water.height_resolved(world, t, min_wavelength) as f32;
            let slope = sim.sim.water.slope_resolved(world, t, min_wavelength);
            let (sx, sz) = (slope.x as f32, -slope.y as f32);

            positions.push(Vec3::new(offset.x, height, offset.y));
            normals.push(Vec3::new(-sx, 1.0, -sz).normalize());

            // World-locked UVs, so the detail stays with the sea rather than with the
            // boat: the grid recentres every frame and the texture must not. On top of
            // that the detail rides the water's own orbital displacement, which is
            // bounded — a surface parcel travels a circle of the wave's amplitude — so
            // the ripples go round with the wave instead of being dragged across it.
            let parcel =
                world - sim.sim.water.horizontal_displacement(world, t) - current - uv_base;
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
            // Foam judged against this sea's own typical slope rather than a fixed
            // number. A long swell and a short chop of the same height are nothing
            // alike in steepness, so a threshold tuned on one gives either no foam at
            // all — which is what was happening, since the fixed 0.16 sat almost at
            // this sea's maximum — or a sea of milk on the other.
            let steepness = (sx * sx + sz * sz).sqrt() / typical;
            let foam = ((steepness - 1.25) / 0.75).clamp(0.0, 1.0) * crest_weight(lift);
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

/// Report how long a frame takes, and how evenly.
///
/// Guessing at a stutter is a way to spend an afternoon. A sea that "jumps" has two
/// completely different explanations — the surface being badly sampled, or the whole
/// application not making its frames — and they are told apart by a number, not by
/// looking. The worst frame in each second matters more than the mean: a steady
/// forty is smooth, and a forty that is sometimes ten is not.
pub fn report_frame_times(time: Res<Time>, mut window: Local<(f32, u32, f32)>) {
    let dt = time.delta_secs();
    window.0 += dt;
    window.1 += 1;
    window.2 = window.2.max(dt);
    if window.0 >= 2.0 {
        // Only when something is actually wrong. Left chattering it buries the one
        // line that matters, and the whole value of this is that it answered "is the
        // sea badly sampled or is the application not making its frames" with a
        // number instead of an afternoon.
        if window.2 > 0.025 {
            warn!(
                "slow frames: {:.0} per second mean, worst {:.1} ms",
                window.1 as f32 / window.0,
                window.2 * 1000.0
            );
        }
        *window = (0.0, 0, 0.0);
    }
}
