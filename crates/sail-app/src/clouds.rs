//! Low cloud, built out of particles.
//!
//! The sky is a cubemap, which is the right way to draw everything above about a
//! kilometre: it costs nothing, it never moves, and nothing up there needs to. But a
//! cubemap is painted on the inside of a sphere at infinity, and the one thing it cannot
//! do is *parallax*. Low cloud has parallax. It is the layer that tells you the sky is a
//! volume you are sitting underneath rather than a backdrop: the near edge of a cloud
//! crosses the sun before the far edge does, a cell overhead slides past faster than one
//! on the horizon, and the whole deck moves against the islands behind it.
//!
//! So this layer is real geometry at a real altitude, and it is particles rather than a
//! mesh because cloud has no surface to put a mesh on. Each cell is a cluster of puffs
//! and each puff is a billboard; what you read as one cumulus is twenty-odd of them
//! overlapping, which is how every painter and every real-time engine has drawn cloud
//! since before either had any choice about it.
//!
//! Three things move, all of them slowly:
//!
//! * **The deck drifts** with the wind at its own height — not the wind on the deck.
//!   Over water the surface wind is backed fifteen or twenty degrees from the gradient
//!   wind above it and runs at about four fifths of its speed, so the cloud goes a
//!   little faster than the masthead wind and a little to the right of it. Watching that
//!   angle is how you read a front coming through, and it costs one rotation here.
//! * **Each cell breathes.** A cumulus is convection with a skin on it: it rises,
//!   boils over at the top and erodes from the edges, on a timescale of a minute or two.
//!   The puffs are given slow independent wobbles so the cloud changes shape without
//!   anything in it ever visibly moving.
//! * **Nothing twinkles.** Deliberately the opposite of the foam in `spray.rs`, which
//!   seethes. Cloud at two kilometres is the slowest thing in the frame.

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::frame;
use crate::sim::Sim;

/// How many separate clouds.
const CELLS: usize = 20;
/// Puffs in each. Enough that the silhouette is lumpy rather than a row of circles.
const PER_CELL: usize = 26;
const COUNT: usize = CELLS * PER_CELL;

/// Beyond this from the boat a cell is recycled upwind, metres.
///
/// Three and a half kilometres, which is near, and that is the point. This layer is not
/// trying to cover the sky — the deck baked into the cubemap does that, and does it for
/// nothing out to the horizon. What a cubemap cannot do is parallax, and parallax only
/// exists close to. Twenty cells over six kilometres covered three percent of the sky
/// and sat in a band near the horizon, which is a cubemap's job done worse; twenty over
/// three and a half pass overhead, which is the job only geometry can do.
const RANGE: f64 = 3400.0;
/// Nothing is born closer than this, metres. Cells drift overhead and are welcome to —
/// but one *arriving* on top of the mast reads as a bug rather than as weather.
const KEEP_CLEAR: f64 = 700.0;
/// Cloud base, metres. Low: stratocumulus and fair-weather cumulus sit here, and this is
/// the band where parallax is worth having at all.
const BASE_LOW: f32 = 260.0;
const BASE_HIGH: f32 = 520.0;
/// Texels across the puff.
const PUFF: u32 = 128;

/// One cloud.
struct Cell {
    /// Simulation coordinates, metres. Carried by the wind at cloud height.
    at: glam::DVec2,
    /// Altitude of its base.
    base: f32,
    /// How far its puffs are scattered horizontally.
    spread: f32,
    /// How tall it is built.
    rise: f32,
}

/// One billboard.
struct Puff {
    cell: usize,
    /// Where it sits in its cell, as fractions of the cell's spread and rise.
    offset: Vec3,
    /// Direction it breathes along, metres.
    wobble: Vec3,
    /// Seconds for one breath. Slow — a minute and more.
    period: f32,
    phase: f32,
    radius: f32,
    /// How far round the sunward side of its cell it sits, −1 to 1. This is the only
    /// lighting a billboard can have, and it is enough: a cloud is lit from one side and
    /// what you read is the shadowed flank against the bright one.
    sunward: f32,
    /// Its own tone, so the cell is not one flat grey.
    tone: f32,
}

#[derive(Resource)]
pub struct Clouds {
    mesh: Handle<Mesh>,
    cells: Vec<Cell>,
    puffs: Vec<Puff>,
    /// Scratch for sorting back to front, kept so the frame allocates nothing.
    order: Vec<(f32, usize)>,
}

/// Smoothstep in `f32`, local because the core's is `f64` and this is texture work.
fn step_between(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The puff: a soft lump with a lit top and a shaded underside.
///
/// Not a Gaussian blob. A circle fading to nothing at the edge is a cotton ball, and
/// twenty of them is twenty cotton balls — the thing that makes a billboard read as
/// cloud is an edge with lobes in it, so that where two puffs overlap the join is
/// irregular. Three harmonics in the angle does it.
fn puff() -> Image {
    let mut data = Vec::with_capacity((PUFF * PUFF * 4) as usize);
    for y in 0..PUFF {
        for x in 0..PUFF {
            let u = (x as f32 + 0.5) / PUFF as f32 * 2.0 - 1.0;
            let v = (y as f32 + 0.5) / PUFF as f32 * 2.0 - 1.0;
            let r = (u * u + v * v).sqrt();
            let a = v.atan2(u);
            let edge = 0.88
                * (1.0 + 0.16 * (3.0 * a + 1.1).sin() + 0.10 * (5.0 * a - 0.4).sin()
                    - 0.07 * (7.0 * a + 2.0).sin());
            // Dense in the middle and open at the rim, which is what gives the stack of
            // them depth: the centres pile up opaque and the edges stay translucent.
            let alpha = (1.0 - step_between(edge * 0.35, edge, r)).clamp(0.0, 1.0);
            let alpha = alpha * alpha * (3.0 - 2.0 * alpha);

            // Lit from above within the puff itself. The vertex colour decides which
            // flank of the *cloud* this puff is on; this is the smaller shading inside
            // one lump, and without it a puff is a flat disc.
            let lift = ((-v) * 0.5 + 0.5).clamp(0.0, 1.0);
            let shade = 0.62 + 0.38 * lift * lift;

            data.push((shade * 255.0) as u8);
            data.push((shade * 255.0) as u8);
            data.push((shade * 255.0) as u8);
            data.push((alpha * 255.0) as u8);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: PUFF,
            height: PUFF,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    crate::sea_texture::build_mipmaps(&mut image, false);
    image.sampler = ImageSampler::linear();
    image
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    sim: Res<Sim>,
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
            base_color_texture: Some(images.add(puff())),
            // Blended, not added: cloud is opaque stuff seen against the sky, and the
            // shadowed side of it is *darker* than the sky behind it. Added foam can
            // never darken what it is drawn over, which is right for foam and would
            // make this a layer of glowing cotton.
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            double_sided: true,
            ..default()
        })),
        Transform::default(),
        // Never culled by its own bounds: the mesh is rebuilt in render coordinates
        // every frame and its vertices move kilometres, so the automatic bounding box
        // computed at startup is meaningless and will pop the whole deck out of view.
        bevy::camera::visibility::NoFrustumCulling,
    ));

    let mut seed = sail_core::rng::Pcg32::new(0x5C_1D_0B);
    let boat = sim.sim.state.pos;
    let mut cells = Vec::with_capacity(CELLS);
    for _ in 0..CELLS {
        cells.push(seeded_cell(&mut seed, boat, None));
    }

    let mut puffs = Vec::with_capacity(COUNT);
    for cell in 0..CELLS {
        for _ in 0..PER_CELL {
            // Scattered in a flattened ball. Cloud is wider than it is tall at this
            // altitude — a cumulus with a base at three hundred metres is a loaf, not a
            // tower, until it starts to grow into something you should reef for.
            let a = seed.next_f64() as f32 * std::f32::consts::TAU;
            let r = seed.next_f64().sqrt() as f32;
            let h = seed.next_f64() as f32;
            let offset = Vec3::new(r * a.cos(), h * h, r * a.sin());
            puffs.push(Puff {
                cell,
                offset,
                wobble: Vec3::new(
                    seed.next_f64() as f32 - 0.5,
                    (seed.next_f64() as f32 - 0.5) * 0.4,
                    seed.next_f64() as f32 - 0.5,
                ) * 34.0,
                period: 42.0 + seed.next_f64() as f32 * 58.0,
                phase: seed.next_f64() as f32 * std::f32::consts::TAU,
                radius: 70.0 + seed.next_f64() as f32 * 95.0,
                sunward: 0.0,
                tone: 0.82 + seed.next_f64() as f32 * 0.18,
            });
        }
    }
    // Which flank of its cell each puff is on, measured against the sun. Done once:
    // the sun does not move and neither do the offsets.
    let sun = crate::sky::SUN.normalize();
    for p in &mut puffs {
        let across = Vec3::new(p.offset.x, 0.0, p.offset.z);
        p.sunward = across.dot(Vec3::new(sun.x, 0.0, sun.z).normalize_or_zero());
    }

    // One line, because a layer that silently fails to appear is indistinguishable from
    // a layer that is there and too faint, and the difference matters when the only way
    // to look at this is to run it.
    let nearest = cells
        .iter()
        .map(|c| (c.at - boat).length())
        .fold(f64::INFINITY, f64::min);
    info!(
        "cloud: {CELLS} cells of {PER_CELL} puffs, bases {:.0}-{:.0} m, nearest {:.0} m",
        cells.iter().map(|c| c.base).fold(f32::INFINITY, f32::min),
        cells.iter().map(|c| c.base).fold(0.0, f32::max),
        nearest
    );

    commands.insert_resource(Clouds {
        mesh: handle,
        cells,
        puffs,
        order: Vec::with_capacity(COUNT),
    });
}

/// Place a cloud. With a drift given it comes in from upwind; without one it is
/// scattered anywhere in range, which is what the first frame wants.
fn seeded_cell(
    seed: &mut sail_core::rng::Pcg32,
    boat: glam::DVec2,
    drift: Option<glam::DVec2>,
) -> Cell {
    let at = match drift {
        Some(d) if d.length_squared() > 1e-6 => {
            let upwind = -d.normalize();
            let across = glam::DVec2::new(-upwind.y, upwind.x);
            boat + upwind * RANGE * 0.92 + across * (seed.next_f64() * 2.0 - 1.0) * RANGE
        }
        _ => {
            let a = seed.next_f64() * std::f64::consts::TAU;
            let r = KEEP_CLEAR + seed.next_f64().sqrt() * (RANGE - KEEP_CLEAR);
            boat + glam::DVec2::new(a.cos(), a.sin()) * r
        }
    };
    Cell {
        at,
        base: BASE_LOW + seed.next_f64() as f32 * (BASE_HIGH - BASE_LOW),
        spread: 230.0 + seed.next_f64() as f32 * 280.0,
        rise: 90.0 + seed.next_f64() as f32 * 170.0,
    }
}

pub fn update(
    origin: Res<frame::RenderOrigin>,
    ink: Res<crate::world::Ink>,
    sim: Res<Sim>,
    time: Res<Time>,
    mut clouds: ResMut<Clouds>,
    mut meshes: ResMut<Assets<Mesh>>,
    camera: Query<&Transform, With<crate::camera::MainCamera>>,
) {
    let Ok(view) = camera.single() else {
        return;
    };
    let handle = clouds.mesh.clone();
    let Some(mut mesh) = meshes.get_mut(&handle) else {
        return;
    };
    let st = &sim.render_state();
    let t = st.time;
    let dt = time.delta_secs() as f64;
    let boat = st.pos;

    // The wind at cloud height, from the wind on the water. Over open water the surface
    // wind is backed from the gradient wind above it and slower than it — friction turns
    // the flow towards low pressure and slows it — so the cloud runs about a quarter
    // faster and twenty degrees to the right of what the masthead sees. Northern
    // hemisphere; the sign is a hemisphere away from arbitrary.
    let surface = sim.sim.wind.sample(boat, t);
    let veer = sail_core::units::rad(20.0);
    let (s, c) = (veer.sin(), veer.cos());
    let drift =
        glam::DVec2::new(surface.x * c - surface.y * s, surface.x * s + surface.y * c) * 1.25;

    let mut seed = sail_core::rng::Pcg32::new(0x5C_1D_0B ^ (t * 1000.0) as u64);
    for cell in &mut clouds.cells {
        cell.at += drift * dt;
        if (cell.at - boat).length() > RANGE {
            *cell = seeded_cell(&mut seed, boat, Some(drift));
        }
    }

    // Back to front. One mesh means one draw call and therefore one sort order, so the
    // sort has to happen here: blended quads written in index order show their seams
    // wherever a near puff is drawn before a far one.
    let Clouds {
        puffs,
        cells,
        order,
        ..
    } = &mut *clouds;
    order.clear();
    for (i, p) in puffs.iter().enumerate() {
        let cell = &cells[p.cell];
        let here = centre_of(p, cell, &origin, t);
        order.push(((view.translation - here).length_squared(), i));
    }
    order.sort_unstable_by(|a, b| b.0.total_cmp(&a.0));

    let haze = crate::sky::haze_colour().to_linear();
    let haze = Vec3::new(haze.red, haze.green, haze.blue);
    let mut positions = Vec::with_capacity(COUNT * 4);
    let mut colours = Vec::with_capacity(COUNT * 4);
    let mut uvs = Vec::with_capacity(COUNT * 4);

    for &(distance_sq, i) in order.iter() {
        let p = &puffs[i];
        let cell = &cells[p.cell];
        let centre = centre_of(p, cell, &origin, t);
        let distance = distance_sq.sqrt();

        // Lit on the sunward flank, shadowed on the other, and the base of the cell
        // darker than its top. That is the whole of the shading, and it is the whole of
        // what a cloud's shading is at this distance.
        let lit = (0.5 + 0.5 * p.sunward).clamp(0.0, 1.0);
        let height = p.offset.y.clamp(0.0, 1.0);
        let light = (0.30 + 0.52 * lit + 0.34 * height).clamp(0.0, 1.0);
        let mut colour = crate::sky::CLOUD_SHADOW.lerp(crate::sky::CLOUD_SUNLIT, light) * p.tone;

        // Into the haze with distance, so the near cells sit in front of the deck
        // painted on the cubemap instead of ending at a visible edge against it.
        let far = (distance / (RANGE as f32 * 0.85)).clamp(0.0, 1.0);
        colour = colour.lerp(haze, far * far * 0.6);
        let mut alpha = 0.80 * (1.0 - far * far * 0.4);

        if ink.0 {
            // Drawn cloud is flat tone and hard edges: three levels, no gradient. The
            // same grammar as the ink sea, and the reason the puff texture's soft rim
            // survives it is that the *stack* of them still gives a ragged silhouette.
            const LEVELS: f32 = 3.0;
            let step = (light * LEVELS).round() / LEVELS;
            colour = crate::sky::CLOUD_SHADOW
                .lerp(crate::sky::CLOUD_SUNLIT, step)
                .lerp(haze, far * far * 0.5);
            alpha = (alpha * 1.15).min(0.95);
        }

        let breath = ((t as f32 / p.period) * std::f32::consts::TAU + p.phase).sin();
        let size = p.radius * (1.0 + 0.10 * breath);
        let to_eye = (view.translation - centre).normalize_or_zero();
        let right = to_eye.cross(Vec3::Y).normalize_or_zero();
        let up = right.cross(to_eye).normalize_or_zero();
        for (u, v) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            positions.push(centre + right * (size * u) + up * (size * v));
            colours.push([colour.x, colour.y, colour.z, alpha]);
            uvs.push(Vec2::new(u * 0.5 + 0.5, v * 0.5 + 0.5));
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
}

/// Where a puff is this instant, in render coordinates.
///
/// One definition, called by the sort and by the draw. They disagreed once in `spray.rs`
/// — the rope drawn to a corner the cloth had been pushed away from — and that is a bug
/// that costs an hour to see and a minute to fix.
fn centre_of(p: &Puff, cell: &Cell, origin: &frame::RenderOrigin, t: f64) -> Vec3 {
    let breath = ((t as f32 / p.period) * std::f32::consts::TAU + p.phase).sin();
    let local = Vec3::new(
        p.offset.x * cell.spread,
        p.offset.y * cell.rise,
        p.offset.z * cell.spread,
    ) + p.wobble * breath;
    let at = cell.at + glam::DVec2::new(local.x as f64, local.z as f64);
    origin.to_bevy(at, cell.base + local.y)
}
