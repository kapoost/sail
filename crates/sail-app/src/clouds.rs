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
//!
//! # Shadows
//!
//! Cloud that does not shade anything is a sticker. Two consequences, from one
//! definition — [`Clouds::shade_at`], which asks where the sun ray through a point
//! crosses each cloud's own base:
//!
//! * **On the water.** The sea reads it per vertex and takes the tone down a step, so in
//!   ink mode a cloud shadow is a flat tone patch with a hard contour, which is what a
//!   painter does with one. Glints go out inside it, because there is no sun in there to
//!   glint off. And the patch is nowhere near under the cloud: this sun sits seventeen
//!   degrees up, so a cell with its base at four hundred metres throws its shadow
//!   thirteen hundred metres to leeward of itself. That offset is the thing that reads
//!   as height.
//! * **In the air.** The shaft between the cloud and its shadow is drawn: a few narrow
//!   ribbons per cell, parallel to the sun and ending where they meet the water. What
//!   you see as sunbeams is the lit air *between* shadow cones, so drawing the cones is
//!   the honest way round, and with a sun this low they run nearly horizontally and
//!   reach for a kilometre and more — long diagonal shafts under a scattered cloud
//!   deck, which is the thing you actually see at sea.

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::frame;
use crate::sim::Sim;

/// How many separate clouds.
const CELLS: usize = 48;
/// Puffs in each. Enough that the silhouette is lumpy rather than a row of circles.
const PER_CELL: usize = 26;
const COUNT: usize = CELLS * PER_CELL;

/// Beyond this from the boat a cell is recycled upwind, metres.
///
/// Seven and a half kilometres, which is the archipelago's own size: the islands are
/// scattered over nine kilometres each way, and a cloud layer that stopped at three and
/// a half had clear sky over every one of them. Cloud over land is not decoration — it
/// is the oldest way of finding land there is, and it is still in the pilot books: a
/// cumulus stands over an island all day because the land heats and the sea does not,
/// so a cloud that stays put while the others go by has something underneath it.
///
/// This was three and a half kilometres on purpose and the reason still holds — the
/// baked deck in the cubemap covers the sky for nothing, and what geometry buys is
/// parallax, which only exists close to. So the cell count goes up with the range
/// rather than the density going down: forty-eight over seven and a half kilometres
/// keeps roughly what twenty had over three and a half, and some of them now have
/// islands under them.
const RANGE: f64 = 7500.0;
/// Nothing is born closer than this, metres. Cells drift overhead and are welcome to —
/// but one *arriving* on top of the mast reads as a bug rather than as weather.
const KEEP_CLEAR: f64 = 700.0;
/// Cloud base, metres. Low: stratocumulus and fair-weather cumulus sit here, and this is
/// the band where parallax is worth having at all.
const BASE_LOW: f32 = 260.0;
const BASE_HIGH: f32 = 520.0;
/// Texels across the puff.
const PUFF: u32 = 128;
/// How much a patch of dry land is worth against a gust when siting a cloud.
///
/// Comparable with a strong gust, not overwhelming: an island gets cloud over it more
/// often than the water does, and not always.
const LAND_THERMAL: f64 = 0.55;
/// How much higher a cloud base sits over land, metres.
const LAND_LIFT: f32 = 90.0;

/// Shafts drawn per cloud. Three narrow ones rather than one the width of the cell: a
/// ray reads as a ray because it is narrow, and a single wedge as wide as the cloud is a
/// grey smear.
const BEAMS: usize = 3;

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
    /// Where across the cell each shaft starts, as a fraction of the spread, and how
    /// wide it is in metres. Fixed with the cell so the shafts do not crawl about.
    beams: [(f32, f32); BEAMS],
    /// How much of a squall this cell is: 0 for fair-weather cloud, 1 for the thing
    /// with the rain under it.
    storm: f32,
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

    /// Its own tone, so the cell is not one flat grey.
    tone: f32,
}

#[derive(Resource)]
pub struct Clouds {
    mesh: Handle<Mesh>,
    /// The shafts, a mesh of their own because they are a different shape and want a
    /// different fade, and sharing a mesh would mean sharing a sort order with the
    /// puffs — which is wrong, since a shaft is drawn behind the cloud that casts it.
    shafts: Handle<Mesh>,
    cells: Vec<Cell>,
    puffs: Vec<Puff>,
    /// Scratch for sorting back to front, kept so the frame allocates nothing.
    order: Vec<(f32, usize)>,
}

/// Where the cloud shadows fall on the water, as a coarse map in metres.
///
/// The one definition of the shadow, read by the sea in `world.rs` and by the shafts
/// drawn here. Two definitions would be two shadows, and they would not line up.
///
/// A map rather than a list of discs, and that is a measurement rather than a
/// preference. Asked disc by disc, per vertex, this cost five per cent of the frame —
/// because the sea grid is a few metres across near the boat and a cloud shadow varies
/// over five hundred, so every shadow edge was being resolved a hundred times finer than
/// it has any detail. Sixty metres a cell is finer than the edge is sharp; four
/// thousand nodes built once a frame replaces forty thousand vertices each asking
/// twenty clouds, and the inner loop becomes four array reads.
pub struct ShadowField {
    /// Shade at each node, row-major, `MAP` square. Empty when there is no cloud.
    map: Vec<f32>,
    /// Simulation coordinates of node (0, 0).
    corner: glam::DVec2,
    /// Metres between nodes.
    pitch: f64,
}

/// Nodes across the shadow map, each way.
const MAP: usize = 64;

impl ShadowField {
    /// No cloud, full sun. For the frames before the layer exists.
    pub fn clear() -> Self {
        Self {
            map: Vec::new(),
            corner: glam::DVec2::ZERO,
            pitch: 1.0,
        }
    }

    /// How shaded a point on the water is: 0 in full sun, 1 under the middle of a cloud.
    pub fn shade_at(&self, at: glam::DVec2) -> f32 {
        if self.map.is_empty() {
            return 0.0;
        }
        let p = (at - self.corner) / self.pitch;
        if p.x < 0.0 || p.y < 0.0 {
            return 0.0;
        }
        let (ix, iy) = (p.x as usize, p.y as usize);
        if ix + 1 >= MAP || iy + 1 >= MAP {
            return 0.0;
        }
        let (fx, fy) = ((p.x - ix as f64) as f32, (p.y - iy as f64) as f32);
        let at = |x: usize, y: usize| self.map[y * MAP + x];
        let top = at(ix, iy) + (at(ix + 1, iy) - at(ix, iy)) * fx;
        let bottom = at(ix, iy + 1) + (at(ix + 1, iy + 1) - at(ix, iy + 1)) * fx;
        top + (bottom - top) * fy
    }
}

impl Clouds {
    /// Project the clouds onto the water along the sun, over a square of side `2 * reach`
    /// centred on `boat`.
    ///
    /// Only the shadows that can reach that square are considered: a disc whose centre
    /// is further off than its own radius plus the reach cannot touch it, which is exact
    /// and leaves two or three clouds out of twenty to actually evaluate.
    pub fn shadows(&self, boat: glam::DVec2, reach: f64, sun: Vec3) -> ShadowField {
        // The sun's horizontal run per metre of height, in simulation coordinates.
        // Bevy's -Z is simulation north, so the second component flips. A point is in
        // shadow when the ray from it to the sun crosses a cloud, so the shadow sits
        // *down*-sun of the cloud by the height over the tangent of the elevation —
        // which at seventeen degrees is more than three times the altitude. That offset
        // is what reads as height, and it comes from the real sun angle rather than
        // from a guess at how far a shadow ought to be thrown.
        let sun = sun.normalize_or_zero();
        let run = glam::DVec2::new(sun.x as f64, -sun.z as f64) / sun.y.max(0.05) as f64;
        let discs: Vec<(glam::DVec2, f32, f32)> = self
            .cells
            .iter()
            .map(|cell| (cell.at - run * cell.base as f64, cell.spread * 0.95))
            .filter(|(centre, outer)| (*centre - boat).length() < reach + *outer as f64)
            .map(|(centre, outer)| (centre, outer * 0.45, outer))
            .collect();
        if discs.is_empty() {
            return ShadowField::clear();
        }

        let pitch = 2.0 * reach / (MAP - 1) as f64;
        let corner = boat - glam::DVec2::splat(reach);
        let mut map = vec![0.0_f32; MAP * MAP];
        for iy in 0..MAP {
            for ix in 0..MAP {
                let at = corner + glam::DVec2::new(ix as f64, iy as f64) * pitch;
                let mut most = 0.0_f32;
                for &(centre, inner, outer) in &discs {
                    let r = (at - centre).length() as f32;
                    // Soft at the rim, because the cloud is soft at the rim. The core is
                    // wholly opaque, which is why the middle of a cloud shadow is as
                    // dark as it is.
                    most = most.max(1.0 - step_between(inner, outer, r));
                }
                map[iy * MAP + ix] = most.clamp(0.0, 1.0);
            }
        }
        ShadowField { map, corner, pitch }
    }
}

/// Smoothstep in `f32`, local because the core's is `f64` and this is texture work.
pub fn step_between(a: f32, b: f32, x: f32) -> f32 {
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

    // The shafts, same vertex layout, their own mesh.
    let mut beam_mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    let beam_quads = CELLS * BEAMS;
    let beam_vertices = beam_quads * 4;
    beam_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![Vec3::ZERO; beam_vertices]);
    beam_mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![Vec3::Y; beam_vertices]);
    beam_mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![Vec2::ZERO; beam_vertices]);
    beam_mesh.insert_attribute(
        Mesh::ATTRIBUTE_COLOR,
        vec![[1.0_f32, 1.0, 1.0, 0.0]; beam_vertices],
    );
    let mut beam_indices = Vec::with_capacity(beam_quads * 6);
    for i in 0..beam_quads {
        let a = (i * 4) as u32;
        beam_indices.extend_from_slice(&[a, a + 1, a + 2, a, a + 2, a + 3]);
    }
    beam_mesh.insert_indices(Indices::U32(beam_indices));
    let beam_handle = meshes.add(beam_mesh);

    // One material for both. The shaft wants a soft edge across its width and nothing
    // along its length, and the puff's own radial falloff sampled along one row is
    // exactly that — so the texture is shared rather than a second one baked to do the
    // same job.
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(images.add(puff())),
        // Blended, not added: cloud is opaque stuff seen against the sky, and the
        // shadowed side of it is *darker* than the sky behind it. Added foam can never
        // darken what it is drawn over, which is right for foam and would make this a
        // layer of glowing cotton — and would make the shadow shafts impossible, since
        // a shaft of shadow is the one thing in the frame that only subtracts.
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        cull_mode: None,
        double_sided: true,
        ..default()
    });

    for mesh in [handle.clone(), beam_handle.clone()] {
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material.clone()),
            Transform::default(),
            // Never culled by its own bounds: the mesh is rebuilt in render coordinates
            // every frame and its vertices move kilometres, so the automatic bounding
            // box computed at startup is meaningless and will pop the deck out of view.
            bevy::camera::visibility::NoFrustumCulling,
        ));
    }

    let mut seed = sail_core::rng::Pcg32::new(0x5C_1D_0B);
    let boat = sim.sim.state.pos;
    let land = sim.sim.land.clone();
    let mut cells = Vec::with_capacity(CELLS);
    for _ in 0..CELLS {
        cells.push(seeded_cell(&mut seed, boat, None, &|_| 0.0, &|at| {
            dry_land(&land, at)
        }));
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
                tone: 0.82 + seed.next_f64() as f32 * 0.18,
            });
        }
    }

    let clouds = Clouds {
        mesh: handle,
        shafts: beam_handle,
        cells,
        puffs,
        order: Vec::with_capacity(COUNT),
    };

    // One line, because a layer that silently fails to appear is indistinguishable from
    // a layer that is there and too faint, and the difference matters when the only way
    // to look at this is to run it. The shaded fraction is the other half of the same
    // check: the shadow is projected along the sun, so a sign error in that projection
    // puts every patch on the wrong side of its cloud, and nothing else in the frame
    // would say so.
    let nearest = clouds
        .cells
        .iter()
        .map(|c| (c.at - boat).length())
        .fold(f64::INFINITY, f64::min);
    let field = clouds.shadows(boat, RANGE, crate::sky::Sky::default().sun);
    let mut shaded = 0;
    const PROBE: i32 = 40;
    for ix in -PROBE..=PROBE {
        for iy in -PROBE..=PROBE {
            let at = boat + glam::DVec2::new(ix as f64, iy as f64) * (RANGE / PROBE as f64);
            if field.shade_at(at) > 0.5 {
                shaded += 1;
            }
        }
    }
    let probes = (2 * PROBE + 1) * (2 * PROBE + 1);
    // How many have an island under them, because that is the claim: cloud stands over
    // land. If this comes out zero the thermal term is not reaching the placement and
    // nothing else in the frame would say so.
    let over_land = clouds
        .cells
        .iter()
        .filter(|c| dry_land(&land, c.at))
        .count();
    info!(
        "cloud: {CELLS} cells of {PER_CELL} puffs, {over_land} over land, \
         bases {:.0}-{:.0} m, nearest {:.0} m, {:.0}% of the water in shadow",
        clouds
            .cells
            .iter()
            .map(|c| c.base)
            .fold(f32::INFINITY, f32::min),
        clouds.cells.iter().map(|c| c.base).fold(0.0, f32::max),
        nearest,
        100.0 * shaded as f32 / probes as f32
    );

    commands.insert_resource(clouds);
}

/// Is there land here? False everywhere when there is no archipelago.
fn dry_land(land: &Option<std::sync::Arc<sail_core::land::Archipelago>>, at: glam::DVec2) -> bool {
    land.as_ref().is_some_and(|l| l.depth(at) <= 0.0)
}

/// Place a cloud. With a drift given it comes in from upwind; without one it is
/// scattered anywhere in range, which is what the first frame wants.
fn seeded_cell(
    seed: &mut sail_core::rng::Pcg32,
    boat: glam::DVec2,
    drift: Option<glam::DVec2>,
    gust: &dyn Fn(glam::DVec2) -> f64,
    dry: &dyn Fn(glam::DVec2) -> bool,
) -> Cell {
    // What makes a place worth a cloud. Two terms, both real convection:
    //
    // * **A gust.** A cumulus is the visible top of a column of rising air and the gust
    //   is what comes back down beside it, so the cloud and the dark patch on the water
    //   and the pressure in the sails are one event.
    // * **Land.** Rock and heath heat in the morning sun and the sea does not, so the
    //   air over an island rises and the air over the water around it does not. That is
    //   why a cumulus stands over an island while the rest of the sky streams past, and
    //   why a cloud sitting still is land.
    let worth = |at: glam::DVec2| gust(at) + if dry(at) { LAND_THERMAL } else { 0.0 };
    let at = match drift {
        Some(d) if d.length_squared() > 1e-6 => {
            let upwind = -d.normalize();
            let across = glam::DVec2::new(-upwind.y, upwind.x);
            // Over a gust. A cumulus is the visible top of a column of rising air, and
            // what comes back down beside it is the gust you feel on the water — so the
            // cloud and the dark patch and the extra pressure in the sails are one
            // event. The wind field knows where its gusts are, so the cloud is placed by
            // asking it rather than by either of them guessing.
            //
            // They separate afterwards, and that is right too: the patch runs over the
            // water at about four fifths of the surface wind while the cloud is carried
            // by the gradient wind above it, a quarter faster. The gust arrives first and
            // the cloud follows, which is the order you see it in.
            let mut best = (boat, f64::NEG_INFINITY);
            for _ in 0..6 {
                let candidate =
                    boat + upwind * RANGE * 0.92 + across * (seed.next_f64() * 2.0 - 1.0) * RANGE;
                let strength = gust(candidate);
                if strength > best.1 {
                    best = (candidate, strength);
                }
            }
            best.0
        }
        _ => {
            // The first frame: scattered anywhere in range, but still asked which of a
            // few places is worth a cloud, so the islands start with their own.
            let mut best = (boat, f64::NEG_INFINITY);
            for _ in 0..8 {
                let a = seed.next_f64() * std::f64::consts::TAU;
                let r = KEEP_CLEAR + seed.next_f64().sqrt() * (RANGE - KEEP_CLEAR);
                let candidate = boat + glam::DVec2::new(a.cos(), a.sin()) * r;
                let score = worth(candidate);
                if score > best.1 {
                    best = (candidate, score);
                }
            }
            best.0
        }
    };
    let spread = 230.0 + seed.next_f64() as f32 * 280.0;
    let mut beams = [(0.0, 0.0); BEAMS];
    for b in &mut beams {
        *b = (
            (seed.next_f64() as f32 * 2.0 - 1.0) * 0.7,
            spread * (0.16 + seed.next_f64() as f32 * 0.22),
        );
    }
    // Higher over land. A cumulus base is set by where the rising air reaches its
    // condensation level, and air off a sun-warmed island starts warmer and drier than
    // air off the sea, so it has to go further up before anything happens to it. The
    // flat bases of a line of cumulus standing at one height over the land and lower
    // over the water is a real thing to look at.
    let over_land = if dry(at) { LAND_LIFT } else { 0.0 };
    Cell {
        at,
        base: BASE_LOW + over_land + seed.next_f64() as f32 * (BASE_HIGH - BASE_LOW),
        spread,
        rise: 90.0 + seed.next_f64() as f32 * 170.0,
        beams,
        storm: 0.0,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update(
    origin: Res<frame::RenderOrigin>,
    sky: Res<crate::sky::Sky>,
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
            *cell = seeded_cell(
                &mut seed,
                boat,
                Some(drift),
                &|at| sim.sim.wind.gust_at(at, t),
                &|at| dry_land(&sim.sim.land, at),
            );
        }
    }

    // The squall cloud. The first cells are given over to whatever squalls are about,
    // which is the piece that makes a squall one object rather than three: the wind that
    // hits you, the rain you are standing in and the black thing to windward are the
    // same event, and you get to see it coming because it is drawn where it is.
    //
    // Low, wide and dark. A squall base is lower than fair-weather cloud — that is most
    // of why it looks like one — and it is dark because it is thick enough to stop the
    // light getting through rather than because of any shadow on it.
    let squalls = sim.sim.wind.squalls();
    for (i, cell) in clouds.cells.iter_mut().enumerate() {
        match squalls.get(i) {
            Some(&(at, radius, maturity)) => {
                cell.at = at;
                cell.base = 150.0 + 60.0 * (1.0 - maturity as f32);
                cell.spread = radius as f32 * 0.85;
                cell.rise = 260.0;
                cell.storm = maturity as f32;
            }
            None if cell.storm > 0.0 => {
                // Its squall has gone, so the cell goes back to being weather. Put it
                // out of the way upwind rather than letting it fade on the spot, which
                // would read as a cloud evaporating overhead.
                *cell = seeded_cell(
                    &mut seed,
                    boat,
                    Some(drift),
                    &|at| sim.sim.wind.gust_at(at, t),
                    &|at| dry_land(&sim.sim.land, at),
                );
            }
            None => {}
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

    // The same two tones the painted decks are shaded with, for the same weather: one
    // palette, so the particles are the sky they are standing in front of.
    let (shadow, sunlit) = sky.cloud_tones();
    let haze = crate::sky::haze_colour(&sky).to_linear();
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
        // Which flank of its cell this puff is on, measured against the sun. Done per
        // frame now rather than once at startup, because the sun moves: a cloud lit
        // from the side it was lit from this morning is a cloud with the light painted
        // on it.
        let across = Vec3::new(p.offset.x, 0.0, p.offset.z);
        let sunward = across.dot(Vec3::new(sky.sun.x, 0.0, sky.sun.z).normalize_or_zero());
        let lit = (0.5 + 0.5 * sunward).clamp(0.0, 1.0);
        let height = p.offset.y.clamp(0.0, 1.0);
        let mut light = (0.30 + 0.52 * lit + 0.34 * height).clamp(0.0, 1.0);
        // A squall is dark underneath and bright on top, and it is one cloud: the base
        // is in its own shadow and the top is in the sun. Keeping the top lit is what
        // stops it reading as a grey blob and makes it read as something tall.
        if cell.storm > 0.0 {
            let towering = (height - 0.45).max(0.0) / 0.55;
            light = light * (1.0 - cell.storm) + (0.04 + 0.85 * towering) * cell.storm;
        }
        let mut colour = shadow.lerp(sunlit, light) * p.tone;

        // Into the haze with distance, so the near cells sit in front of the deck
        // painted on the cubemap instead of ending at a visible edge against it.
        let far = (distance / (RANGE as f32 * 0.85)).clamp(0.0, 1.0);
        colour = colour.lerp(haze, far * far * 0.6);
        // Opaque where it is a squall: you cannot see through one, which is the whole
        // reason it takes the island away.
        let mut alpha = (0.80 + 0.19 * cell.storm) * (1.0 - far * far * 0.4);

        if ink.0 {
            // Drawn cloud is flat tone and hard edges: three levels, no gradient. The
            // same grammar as the ink sea, and the reason the puff texture's soft rim
            // survives it is that the *stack* of them still gives a ragged silhouette.
            const LEVELS: f32 = 3.0;
            let step = (light * LEVELS).round() / LEVELS;
            colour = shadow.lerp(sunlit, step).lerp(haze, far * far * 0.5);
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

    // Scoped, because the shafts are a second mesh out of the same store and two live
    // mutable borrows of it is one more than there can be.
    {
        let handle = clouds.mesh.clone();
        let Some(mut mesh) = meshes.get_mut(&handle) else {
            return;
        };
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    }

    // --- The shafts ---------------------------------------------------------------
    //
    // A shadow cone, drawn from the cloud base down the sun to where it meets the
    // water. Parallel beams in three dimensions, which is why they appear to converge
    // on the sun when you look towards it and on the antisolar point when you look
    // away: that is not an effect added here, it is what parallel lines do, and it is
    // the reason this is worth drawing as geometry rather than painting into the sky.
    let shafts = clouds.shafts.clone();
    let Some(mut beam_mesh) = meshes.get_mut(&shafts) else {
        return;
    };
    let sun = sky.sun.normalize();
    let axis = -sun;
    let mut positions = Vec::with_capacity(CELLS * BEAMS * 4);
    let mut colours = Vec::with_capacity(CELLS * BEAMS * 4);
    let mut uvs = Vec::with_capacity(CELLS * BEAMS * 4);
    for cell in &clouds.cells {
        // Across the sun, horizontally: the direction the beams are spread along.
        let across = Vec3::new(-sun.z, 0.0, sun.x).normalize_or_zero();
        let head = origin.to_bevy(cell.at, cell.base);
        // Where it reaches the water. The whole visible length of a shaft is the bit
        // between the cloud and the sea, and with the sun this low that is long.
        let length = cell.base / sun.y.max(0.05);
        for &(offset, width) in &cell.beams {
            let start = head + across * (offset * cell.spread) + axis * (cell.spread * 0.25);
            let end = start + axis * length;
            let middle = (start + end) * 0.5;
            let to_eye = (view.translation - middle).normalize_or_zero();
            // A beam seen end-on is not a beam. This is most of what keeps the effect
            // honest: look along the shafts and they go, which is what happens when you
            // turn to face the sun with the shadows running away behind you.
            let side_on = (1.0 - axis.dot(to_eye).abs()).clamp(0.0, 1.0);
            let right = axis.cross(to_eye).normalize_or_zero() * (width * 0.5);
            // Shadowed air: the haze colour with the light taken out of it. Not black,
            // and not a colour of its own — a shaft is the same sky, unlit.
            let mut tone = haze * 0.58;
            let mut alpha = 0.16 * side_on * side_on;
            if ink.0 {
                // Two levels and a touch stronger, because a drawn beam is a wedge with
                // an edge and not an airbrushed gradient.
                tone = haze * 0.52;
                alpha = ((alpha / 0.16 * 2.0).round() / 2.0) * 0.20;
            }
            // Thinning along its length: the far end of a shaft is filled in by the
            // light scattered into it from everywhere else, so it fades out rather than
            // stopping at the water.
            for (point, fade, v) in [
                (start - right, 1.0, 0.0),
                (start + right, 1.0, 1.0),
                (end + right, 0.0, 1.0),
                (end - right, 0.0, 0.0),
            ] {
                positions.push(point);
                colours.push([tone.x, tone.y, tone.z, alpha * fade]);
                // Across the width only: the puff texture's radial falloff sampled
                // along its middle row is a soft-edged band, which is the profile a
                // shaft wants.
                uvs.push(Vec2::new(v, 0.5));
            }
        }
    }
    beam_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    beam_mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    beam_mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
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
