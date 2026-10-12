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

use crate::camera;
use crate::frame;
use crate::sea_texture;
use crate::sky;
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
    // The ripple animation, baked here so the first frame is ready before anything is
    // drawn. `SAIL_WATER_FRAMES=0` turns it off and leaves the still map, which is both
    // an escape hatch and the way to see what the motion is actually buying.
    let animated = std::env::var("SAIL_WATER_FRAMES").is_ok_and(|v| v == "0");
    let film: Vec<Handle<Image>> = if animated {
        Vec::new()
    } else {
        let started = std::time::Instant::now();
        let frames: Vec<Handle<Image>> = sea_texture::normal_map_frames()
            .into_iter()
            .map(|f| images.add(f))
            .collect();
        info!(
            "water: {} ripple frames over {:.1} s, baked in {:.2} s",
            frames.len(),
            sea_texture::RIPPLE_LOOP,
            started.elapsed().as_secs_f32()
        );
        frames
    };
    let first = film
        .first()
        .cloned()
        .unwrap_or_else(|| images.add(sea_texture::normal_map()));
    let water = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        normal_map_texture: Some(first),
        metallic_roughness_texture: Some(images.add(sea_texture::roughness_map())),
        perceptual_roughness: 1.0,
        metallic: 0.0,
        reflectance: 0.25,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    commands.insert_resource(SeaFilm {
        frames: film,
        material: Some(water.clone()),
        shown: 0,
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

/// How many tones the drawn sea is allowed.
///
/// Four. Three is a poster and six is a photograph with a fault in it; four is what an
/// inker uses for water, and it is enough to carry a wave's shape and not enough to
/// suggest a gradient.
const INK_BANDS: usize = 4;

/// Spacing of the hatching, metres on the water.
///
/// Manga shades with line, not with grey: a tone is a field of parallel strokes, and
/// how dark it reads is how close together they are. Three quarters of a metre at the
/// boat, which at this resolution puts a stroke every few pixels close in and lets them
/// merge into an even tone further out — which is what happens on the page too, and why
/// a screentone looks like shading rather than like stripes.
const HATCH_SPACING: f32 = 0.75;

/// How dark a stroke is.
const HATCH_DEPTH: f32 = 0.5;

/// How far the hatching leans off the wind, radians.
///
/// Not aligned with anything in the scene. Hatching that runs square to the waves reads
/// as corduroy, and hatching that runs square to the screen reads as a screen-door; a
/// draughtsman lays strokes across the form at an angle to it, and that is the one
/// direction that does not fight what it is shading.
const HATCH_LEAN: f32 = 0.6;

/// How wide the contour line on the water is, as a fraction of a tone band.
///
/// The sea gets no silhouette, because it has nothing to be a silhouette against — it
/// is what everything else is drawn against. What a drawn sea has instead is a contour:
/// the line along the top of a wave, which in this style is not a separate thing at all.
/// It is the edge between two tones. The bands are already here, so the line is drawn
/// by darkening the water that sits just on the shallow side of each step, and it lands
/// exactly where an inker would put it — along the crest and around the shoulder —
/// because that is where the tone changes.
const CONTOUR: f32 = 0.13;

/// How strongly the contour is struck, towards white.
///
/// White, not dark. The boat is drawn with dark lines because a dark line separates a
/// solid object from what is behind it, but water has nothing behind it and no edge to
/// separate — what the line is marking is the crest, and a crest is where the water is
/// bright. Drawn dark it reads as a crack in the surface; drawn white it reads as the
/// top of a wave, which is what every painter of water has done with it.
const CONTOUR_INK: f32 = 0.6;

/// How nearly a facet must mirror the sun to be struck as a highlight.
///
/// The cosine of the angle between the surface normal and the half-vector between eye
/// and sun — which is the mirror condition, so the glitter path is where it actually
/// falls rather than where it would be convenient. Tuned by counting: at 0.985 a
/// fifteen-knot sea lights about a tenth of a per cent of the resolved vertices, which
/// is a path and not a sheet.
const GLINT_COS: f32 = 0.985;

/// Fraction of eligible facets that are actually struck.
///
/// Not every facet on the path gets a mark. A continuous band of white is a lens flare;
/// what a sea does — and what an inker draws — is a scatter of separate dashes. The
/// choice is a hash of the water parcel, so each dash belongs to a piece of water and
/// rides the wave through instead of flickering as the grid slides underneath.
const GLINT_STIPPLE: f32 = 0.42;

/// Stable per-parcel hash, used to decide which facets catch the light.
fn stipple(parcel: glam::DVec2) -> f32 {
    let x = (parcel.x * 0.37).floor() as i64;
    let y = (parcel.y * 0.37).floor() as i64;
    let mut h = (x.wrapping_mul(0x27d4_eb2d) ^ y.wrapping_mul(0x1656_67b1)) as u64;
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^= h >> 29;
    (h >> 40) as f32 / 16_777_216.0
}

/// The ripple animation: one normal map per frame of the loop, and the material they
/// are shown on.
///
/// Pre-baked rather than regenerated. Making the map afresh every frame would be the
/// obvious way and it is the wrong one twice over: half a megabyte of texture rebuilt
/// and re-uploaded sixty times a second is real work on both the processor and the bus,
/// and it is exactly the kind of per-frame graphics-resource churn that a long run
/// punishes. There is an unexplained stall in this application's history that has that
/// shape, and this is not the moment to add another candidate. Baked frames are a fixed
/// cost paid once at startup, and after that showing one is a pointer swap.
#[derive(Resource, Default)]
pub struct SeaFilm {
    frames: Vec<Handle<Image>>,
    material: Option<Handle<StandardMaterial>>,
    shown: usize,
}

/// Show the frame the clock is on.
///
/// The material is only touched when the frame actually changes — twelve times a second
/// rather than sixty — because writing a material marks it changed and its bind group is
/// rebuilt, and there is no reason to pay that for a frame that is already up.
pub fn roll_sea_film(
    time: Res<Time>,
    mut film: ResMut<SeaFilm>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if film.frames.is_empty() {
        return;
    }
    let count = film.frames.len();
    let phase = time.elapsed_secs() / sea_texture::RIPPLE_LOOP;
    let want = ((phase.fract() + 1.0).fract() * count as f32) as usize % count;
    if want == film.shown {
        return;
    }
    film.shown = want;
    let Some(handle) = film.material.clone() else {
        return;
    };
    let frame = film.frames[want].clone();
    if let Some(m) = materials.get_mut(&handle).as_deref_mut() {
        m.normal_map_texture = Some(frame);
    }
}

/// Whether the sea is drawn rather than lit.
#[derive(Resource, Default)]
pub struct Ink(pub bool);

impl Ink {
    /// `SAIL_INK=1` starts in the drawn sea, the same way `SAIL_PIXELS` starts in a
    /// chosen resolution: a look you have settled on should not need a keypress every
    /// time the boat is launched.
    pub fn from_env() -> Self {
        Self(std::env::var("SAIL_INK").is_ok_and(|v| v != "0"))
    }
}

/// `B` for the drawn sea.
pub fn toggle_ink(
    keys: Res<ButtonInput<KeyCode>>,
    mut ink: ResMut<Ink>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    q: Query<&MeshMaterial3d<StandardMaterial>, With<SeaSurface>>,
) {
    if !keys.just_pressed(KeyCode::KeyB) {
        return;
    }
    ink.0 = !ink.0;
    // The lighting has to be taken off the water as well, or the bands are drowned in
    // a specular sheen that has no edges in it — flat tones and a glossy highlight are
    // two different pictures and the sheen wins.
    for handle in &q {
        if let Some(m) = materials.get_mut(&handle.0).as_deref_mut() {
            if ink.0 {
                m.perceptual_roughness = 1.0;
                m.reflectance = 0.02;
            } else {
                m.perceptual_roughness = 1.0;
                m.reflectance = 0.25;
            }
        }
    }
    info!("sea: {}", if ink.0 { "drawn" } else { "lit" });
}

// A Bevy system's parameters are its dependency list, not an argument list; the eighth
// one is the camera, and the alternative is a struct that exists only to satisfy a lint.
#[allow(clippy::too_many_arguments)]
pub fn update_sea(
    ink: Res<Ink>,
    origin: Res<frame::RenderOrigin>,
    time: Res<Time>,
    mut drift: ResMut<SurfaceDrift>,
    sim: Res<Sim>,
    mut meshes: ResMut<Assets<Mesh>>,
    chase: Res<camera::ChaseState>,
    mut q: Query<(&SeaSurface, &mut Transform)>,
) {
    let ink = ink.0;
    let sun = sky::SUN.normalize();
    let st = &sim.render_state();
    let centre = origin.to_bevy(st.pos, 0.0);
    let t = st.time;

    for (sea, mut transform) in &mut q {
        transform.translation = Vec3::new(centre.x, 0.0, centre.z);
        // The eye in the sea's own frame: the grid's vertices are offsets from the
        // boat, and a mirror condition has to be evaluated in one frame or the other,
        // not half in each.
        let eye = chase.eye() - transform.translation;
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
        // The current, and only the current.
        //
        // This used to carry a ripple speed as well — the group velocity of a one-metre
        // wave, about 0.6 m/s — because the texture was a still photograph and sliding
        // it was the only motion available. It was always a lie of a particular kind:
        // sliding moves every scale in the pattern at one speed and in one direction,
        // and water does the opposite. Short ripples crawl, long ones run, and the
        // pattern reforms instead of marching. The ripples travel inside the frames
        // now, each at its own dispersion speed, so the texture coordinates are left to
        // carry what genuinely is a bulk translation: the wind-driven surface drift, two
        // and a half percent of the wind speed.
        let speed = tm.tws * 0.025;
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

            let point = Vec3::new(offset.x, height, offset.y);
            let normal = Vec3::new(-sx, 1.0, -sz).normalize();
            positions.push(point);
            normals.push(normal);

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
            // The lit sea's own colours, and a lighter pair for the drawn one.
            //
            // A photograph of deep water really is nearly black, and the drawn sea
            // inherited that and then had hatching laid on top of it — two darkenings on
            // the same water, which put the bottom of the range somewhere no ink drawing
            // goes. A manga sea is a mid tone with white in it: the darkest thing on the
            // page is the *line*, not the field it is drawn on, and the field has to stay
            // light enough for a black stroke to show against it.
            // Both derived from the sky, because that is where a sea's colour comes
            // from: most of what you see looking down at water is the sky looking back
            // at you, plus the small amount of blue-green the water itself returns from
            // below. They were written out as constants beside the sky's own, which
            // meant a storm could roll in overhead and the sea underneath it stayed the
            // colour of a fine afternoon.
            //
            // A trough is lit by the narrow strip of sky it can see, which is mostly the
            // zenith, so it takes the zenith darkened; a crest sees the whole dome and
            // takes the horizon. The water's own body colour leans green, which is why
            // the two are not simply the sky scaled.
            let body = Vec3::new(0.02, 0.07, 0.06);
            let (deep, shallow) = if ink {
                (
                    crate::sky::ZENITH * 0.55 + body * 1.6,
                    crate::sky::HORIZON * 0.44 + body,
                )
            } else {
                (
                    crate::sky::ZENITH * 0.16 + body * 0.6,
                    crate::sky::HORIZON * 0.17 + body * 1.4,
                )
            };
            let mut tone = (lift * 0.5 + 0.5).clamp(0.0, 1.0);
            let mut edge = 1.0;
            if ink {
                // Drawn water, not photographed water.
                //
                // The whole difference is that a painter does not shade a gradient; a
                // painter picks three or four tones and puts a hard edge between them,
                // and the eye reads the shape of that edge as the shape of the wave.
                // So the gradient is quantised, and the step is the drawing.
                //
                // It is the right place for it, too: this colour is already computed
                // per vertex on the processor every frame, so a style that would
                // normally want a shader costs one rounding here.
                let banded = (tone * INK_BANDS as f32).floor();
                // How far into its band this vertex sits, kept before the tone is
                // flattened, because afterwards the information is gone.
                edge = tone * INK_BANDS as f32 - banded;
                tone = (banded / (INK_BANDS - 1) as f32).clamp(0.0, 1.0);
            }
            let base = deep.lerp(shallow, tone);
            // Foam judged against this sea's own typical slope rather than a fixed
            // number. A long swell and a short chop of the same height are nothing
            // alike in steepness, so a threshold tuned on one gives either no foam at
            // all — which is what was happening, since the fixed 0.16 sat almost at
            // this sea's maximum — or a sea of milk on the other.
            let steepness = (sx * sx + sz * sz).sqrt() / typical;
            let mut foam = ((steepness - 1.25) / 0.75).clamp(0.0, 1.0) * crest_weight(lift);
            if ink {
                // Foam becomes ink: on or off, no half measures, because a drawn
                // whitecap is a shape with an outline rather than a soft bloom. And
                // thrown earlier, since a hard-edged mark has to be large enough to
                // read as a mark.
                foam = if foam > 0.18 { 1.0 } else { 0.0 };
            }
            let mut colour = base.lerp(
                if ink {
                    Vec3::new(0.95, 0.97, 1.0)
                } else {
                    Vec3::new(0.82, 0.88, 0.92)
                },
                foam,
            );
            // Hatching: the shading is made of strokes, and the darker the water the
            // more of it they cover. A stroke is wide where the tone is dark and thins
            // to nothing where it is light, so the three bands of the drawn sea come out
            // as three densities of line rather than three greys — which is what a
            // manga page does and the reason it reads as drawn rather than rendered.
            //
            // Laid in world metres on the surface, so the strokes belong to the water
            // and travel with it rather than crawling across it as the camera moves.
            if ink {
                let lean = tm.twd as f32 + HATCH_LEAN;
                let along = world.x as f32 * lean.sin() + world.y as f32 * lean.cos();
                let phase = (along / HATCH_SPACING).fract().abs();
                // Dark water gets a wide stroke, light water none at all.
                let width = (1.0 - tone) * 0.55;
                if phase < width {
                    // Softened at the stroke's own edge by a hair, or the lines crawl
                    // with the vertex grid instead of lying still on the water.
                    let bite = ((width - phase) / 0.08).clamp(0.0, 1.0);
                    colour *= 1.0 - HATCH_DEPTH * bite;
                }
            }
            if ink && edge < CONTOUR {
                // The contour line along the step between two tones.
                colour = colour.lerp(Vec3::new(0.97, 0.99, 1.0), CONTOUR_INK);
            }
            if ink {
                // Glints off the wave faces.
                //
                // The sheen was taken off the water to let the flat tones read, and
                // that took the sun off it as well — a drawn sea without highlights
                // looks like a painted floor. So they go back on deliberately, as
                // marks rather than as a sheen: struck where the facet actually
                // mirrors the sun towards the eye, and only on the facets the stipple
                // picks, which is how you get a scatter of dashes along the glitter
                // path instead of a continuous smear of white.
                //
                // Done here rather than by the renderer because this is the one place
                // that knows the true surface normal. The lit sea gets its highlights
                // from the material and does not want a second set fighting them.
                let view = (eye - point).normalize_or_zero();
                let half = (view + sun).normalize_or_zero();
                if normal.dot(half) > GLINT_COS && stipple(parcel) < GLINT_STIPPLE {
                    colour = Vec3::new(1.0, 1.0, 0.97);
                }
            }
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
/// What is growing, once a minute.
///
/// Added after a run froze at the two-hour mark. The stack said what it was not: the
/// main thread was inside `renderer_extract` waiting on the render thread while Metal
/// sat in `submitCommandBuffers`, so the simulation was still stepping and nothing in
/// this crate was deadlocked — the submission queue was the thing that had stopped
/// moving. That is the signature of a graphics resource that keeps being created and
/// never dropped, and it is also the signature of a driver having a bad afternoon. One
/// frozen process cannot tell those apart, and guessing at it would have meant
/// rewriting something that may well be innocent.
///
/// So this writes down the four counts that would separate them. If entities or assets
/// climb steadily across a long passage, the leak is ours and the log says which kind.
/// If they sit flat and the picture stops anyway, it is not ours, and that is worth
/// knowing before a day is spent looking.
pub fn report_vitals(
    time: Res<Time>,
    meshes: Res<Assets<Mesh>>,
    images: Res<Assets<Image>>,
    materials: Res<Assets<StandardMaterial>>,
    entities: Query<()>,
    mut since: Local<f32>,
) {
    *since += time.delta_secs();
    if *since < 60.0 {
        return;
    }
    *since = 0.0;
    info!(
        "vitals at {:.0} min: {} entities, {} meshes, {} images, {} materials",
        time.elapsed_secs() / 60.0,
        entities.iter().count(),
        meshes.len(),
        images.len(),
        materials.len(),
    );
}

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
