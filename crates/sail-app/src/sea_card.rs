//! The sea, as it crosses to the card.
//!
//! **What the processor still knows about the water, and what it has stopped knowing.**
//!
//! Until this module existed, every one of the drawn sea's forty thousand vertices was
//! worked out here: a twenty-one component spectrum asked for height, slope and drift,
//! a colour mixed from the sky, foam judged against the sea's own steepness, and in the
//! drawn sea a tone quantised, a contour struck and a glint stippled. Spread over every
//! core of this machine that was two and a quarter milliseconds a frame — an eighth of
//! the budget for one mesh, and in the drawn sea seven milliseconds, which is nearly
//! half of it.
//!
//! It is all in `assets/sea_wave.wesl` now, and what is left on this side is one
//! eighteen-kilobyte buffer of numbers that are the same for every vertex: where the
//! boat is, what the sky looks like, where each cloud's shadow falls, and the spectrum
//! itself — twenty-one components with their phases already worked out *at the boat*,
//! in double precision, because that is the one part of the sum a float cannot hold.
//!
//! The hull goes on asking `sail-core` directly, sixteen points at a substep, in `f64`.
//! That is the trade this change makes and it is worth naming: there are two
//! implementations of the same spectrum now, one in Rust for the physics and one in
//! WGSL for the picture. They are pinned together by the tests at the foot of this file,
//! which compile the shader, run it on the card over sample points and compare it
//! against the Rust spectrum. Without that this would be a slow-motion fault: the two
//! would drift,
//! and the first anybody would know of it is a boat sailing through water that is not
//! quite where it is drawn.

use bevy::asset::RenderAssetUsages;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::render::storage::ShaderBuffer;
use bevy::shader::ShaderRef;
use bevy::tasks::ParallelSliceMut;
use glam::DVec2;
use sail_core::land::Archipelago;
use sail_core::water::{Phased, WaterSurface};
use std::sync::Arc;

use crate::clouds::ShadowField;
use crate::sky::Dome;

/// The sea's material: the standard one, with a vertex shader of its own.
///
/// Everything the water gets from the renderer — the environment reflection, the ripple
/// normal map, the roughness that follows the wind, the shadows it receives — is the
/// standard material's and is untouched. What the extension replaces is the one stage
/// that used to be done here: turning a grid offset into a piece of sea.
pub type SeaMaterial = ExtendedMaterial<StandardMaterial, SeaWaves>;

/// Rings in the graded grid, counting out from the boat.
pub const RINGS: usize = 101;
/// Components in the spectrum. The card's buffer is a fixed size, so this is the most
/// it can carry; `sail-core` builds twenty-one.
pub const BANDS: usize = 21;
/// Nodes across the cloud shadow map, which must be what `clouds` builds.
pub const SHADE_NODES: usize = 64;
/// Nodes across the bank map, and how far it reaches from the boat.
///
/// Twenty metres between nodes. The shoal tone runs from about sixty to a hundred and
/// seventy metres off a shore — that is where eleven metres of water becomes two — so
/// six or seven nodes cross the band, which is enough for an edge that reads as an edge.
pub const SAND_NODES: usize = 128;
pub const SAND_REACH: f64 = 1_280.0;

const SEA_SHADER: &str = "sea.wesl";
const SEA_PREPASS_SHADER: &str = "sea_prepass.wesl";

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct SeaWaves {
    /// This frame's numbers. Rewritten in place every frame.
    #[storage(100, read_only)]
    pub frame: Handle<ShaderBuffer>,
    /// How deep the water is round here. Rewritten when the boat has moved half a node.
    #[storage(101, read_only)]
    pub bank: Handle<ShaderBuffer>,
}

impl MaterialExtension for SeaWaves {
    fn vertex_shader() -> ShaderRef {
        SEA_SHADER.into()
    }

    /// **And the same displacement in the prepass**, which is what draws the shadow map.
    ///
    /// Left to the standard one, the sun would cast the shadow of a flat plane at mean
    /// sea level and every trough in the picture would be in it.
    fn prepass_vertex_shader() -> ShaderRef {
        SEA_PREPASS_SHADER.into()
    }
}

/// One component, laid out as the shader reads it.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Wave {
    amplitude: f32,
    k: f32,
    wavelength: f32,
    theta0: f32,
    direction: [f32; 2],
    pad: [f32; 2],
}

/// This frame, laid out as the shader reads it.
///
/// `#[repr(C)]` and every field four-byte aligned, which is what makes the Rust layout
/// and the WGSL layout the same thing. A test asserts the offsets rather than trusting
/// that they still line up: a storage buffer read at the wrong offset does not fail, it
/// draws something wrong.
#[repr(C)]
#[derive(Clone, Copy)]
struct Frame {
    eye: [f32; 4],
    sun: [f32; 4],
    deep: [f32; 4],
    far: [f32; 4],
    body: [f32; 4],
    dome_mean: [f32; 4],
    dome_east: [f32; 4],
    dome_south: [f32; 4],
    to_parcel: [f32; 2],
    to_flat: [f32; 2],
    to_shade: [f32; 2],
    to_sand: [f32; 2],
    downwind: [f32; 2],
    hs: f32,
    typical: f32,
    whitecapping: f32,
    longest: f32,
    shade_pitch: f32,
    sand_pitch: f32,
    streak: f32,
    ink: f32,
    components: u32,
    shade_nodes: u32,
    sand_nodes: u32,
    tile: f32,
    shadow_reach: f32,
    pad: u32,
    rings: [f32; RINGS],
    spacings: [f32; RINGS],
    wave: [Wave; BANDS],
    shade: [f32; SHADE_NODES * SHADE_NODES],
    tail: [f32; 2],
}

// Both are plain old data: `repr(C)`, no padding anywhere in them, no pointers, every
// bit pattern valid. That is exactly bytemuck's contract, and it is what lets the whole
// struct go to the card as bytes.
unsafe impl bytemuck::Zeroable for Wave {}
unsafe impl bytemuck::Pod for Wave {}
unsafe impl bytemuck::Zeroable for Frame {}
unsafe impl bytemuck::Pod for Frame {}

/// Everything about this frame the card has to be told.
///
/// A struct rather than twenty arguments, and it is also the list of what the sea's
/// picture actually depends on — which is worth being able to read in one place.
pub struct Scene<'a> {
    pub ink: bool,
    /// The eye in the sea mesh's own frame.
    pub eye: Vec3,
    pub sun: Vec3,
    pub deep: Vec3,
    pub far: Vec3,
    pub body: Vec3,
    pub dome: &'a Dome,
    /// Where the boat is, simulation coordinates.
    pub boat: DVec2,
    /// The ship's clock.
    pub time: f64,
    /// The texture anchor: the boat's position less the drift, rounded to whole tiles.
    pub uv_base: DVec2,
    /// How far the surface detail has drifted since the start.
    pub current: DVec2,
    pub downwind: DVec2,
    pub hs: f32,
    pub typical: f32,
    pub whitecapping: f32,
    pub longest: f64,
    /// Metres across one tile of the ripple map, and how far a cloud shadow reaches.
    /// Both are properties of things baked on this side, so the card is told them rather
    /// than keeping its own copy.
    pub tile: f32,
    pub shadow_reach: f32,
    pub shadows: &'a ShadowField,
    pub land: Option<&'a Arc<Archipelago>>,
    pub water: &'a dyn WaterSurface,
}

/// The handles, the page of numbers, and the bank as it last stood.
#[derive(Resource)]
pub struct SeaCard {
    pub material: Handle<SeaMaterial>,
    frame: Handle<ShaderBuffer>,
    bank: Handle<ShaderBuffer>,
    /// The page the shader reads, kept on this side so that what crosses each frame is a
    /// write into a buffer that already exists rather than a new one.
    page: Box<Frame>,
    /// Scratch for the spectrum, so a frame costs no allocation.
    phased: Vec<Phased>,
    /// Where the bank map's middle stands, snapped to its own grid, and which islands it
    /// was built from. Both have to match or it is rebuilt.
    anchor: Option<DVec2>,
    islands: Option<Arc<Archipelago>>,
    depths: Vec<f32>,
}

impl SeaCard {
    /// Make the buffers and the material.
    ///
    /// The grid's own tables go in once and never change: the mesh is built at startup
    /// and never rebuilt, so the ring the vertex stands in is a property of the mesh and
    /// not of the frame.
    pub fn new(
        rings: &[f32],
        spacings: &[f32],
        base: StandardMaterial,
        buffers: &mut Assets<ShaderBuffer>,
        materials: &mut Assets<SeaMaterial>,
    ) -> Self {
        assert_eq!(rings.len(), RINGS, "the grid has changed shape");
        assert_eq!(spacings.len(), RINGS, "the grid has changed shape");
        let mut page = Box::new(Frame::zeroed());
        page.rings.copy_from_slice(rings);
        page.spacings.copy_from_slice(spacings);
        // Open water everywhere until the drawn sea asks for a bank. The shader is told
        // which it is by the node count, but a buffer of zeroes would be land.
        let depths = vec![sail_core::land::OPEN_WATER as f32; SAND_NODES * SAND_NODES];

        let frame = buffers.add(ShaderBuffer::new(vec![*page], RenderAssetUsages::default()));
        let bank = buffers.add(ShaderBuffer::new(
            depths.clone(),
            RenderAssetUsages::default(),
        ));
        let material = materials.add(SeaMaterial {
            base,
            extension: SeaWaves {
                frame: frame.clone(),
                bank: bank.clone(),
            },
        });
        Self {
            material,
            frame,
            bank,
            page,
            phased: Vec::with_capacity(BANDS),
            anchor: None,
            islands: None,
            depths,
        }
    }

    /// Write this frame's numbers, and the bank if it has moved.
    pub fn write(&mut self, scene: &Scene, buffers: &mut Assets<ShaderBuffer>) {
        self.bank(scene, buffers);

        let page = &mut self.page;
        page.eye = [scene.eye.x, scene.eye.y, scene.eye.z, 0.0];
        page.sun = [scene.sun.x, scene.sun.y, scene.sun.z, 0.0];
        page.deep = [scene.deep.x, scene.deep.y, scene.deep.z, 0.0];
        page.far = [scene.far.x, scene.far.y, scene.far.z, 0.0];
        page.body = [scene.body.x, scene.body.y, scene.body.z, 0.0];
        let (mean, east, south) = scene.dome.terms();
        page.dome_mean = [mean.x, mean.y, mean.z, 0.0];
        page.dome_east = [east.x, east.y, east.z, 0.0];
        page.dome_south = [south.x, south.y, south.z, 0.0];

        // All four of these are differences taken in double precision and handed over
        // small. The positions they are differences of run to tens of thousands of
        // metres; a float carries a tenth of a millimetre at a thousand and half a
        // centimetre at ten thousand, and the second of those is visible in a texture
        // coordinate.
        let anchor = scene.boat - scene.current;
        page.to_parcel = small(anchor - scene.uv_base);
        page.to_flat = small(scene.boat - scene.uv_base);
        page.downwind = [scene.downwind.x as f32, scene.downwind.y as f32];

        page.hs = scene.hs;
        page.typical = scene.typical;
        page.whitecapping = scene.whitecapping;
        page.longest = scene.longest as f32;
        // The clock, as a float. At a day in, a float still resolves a hundredth of a
        // second, which is a sixth of a frame — the streaks go on moving smoothly, and
        // what the error amounts to is a constant offset in a pattern nobody measures.
        page.streak = scene.time as f32;
        page.ink = if scene.ink { 1.0 } else { 0.0 };
        page.tile = scene.tile;
        page.shadow_reach = scene.shadow_reach;

        // The spectrum, with each phase worked out at the boat.
        scene.water.phased(scene.boat, scene.time, &mut self.phased);
        let count = self.phased.len().min(BANDS);
        for (slot, c) in page.wave.iter_mut().zip(self.phased.iter().take(count)) {
            *slot = Wave {
                amplitude: c.amplitude as f32,
                k: c.k as f32,
                wavelength: c.wavelength as f32,
                theta0: c.phase as f32,
                direction: [c.direction.x as f32, c.direction.y as f32],
                pad: [0.0; 2],
            };
        }
        page.components = count as u32;

        // Cloud shadow. The map is built in `clouds` for the whole square around the
        // boat; it crosses as it stands and the shader reads it the same way the
        // processor used to, bilinearly, so a shadow does not step between the two
        // descriptions of it.
        let (nodes, corner, pitch, map) = scene.shadows.as_map();
        if nodes == SHADE_NODES && map.len() == SHADE_NODES * SHADE_NODES {
            page.shade.copy_from_slice(map);
            page.to_shade = small(scene.boat - corner);
            page.shade_pitch = pitch as f32;
            page.shade_nodes = SHADE_NODES as u32;
        } else {
            // No cloud, or a map this buffer cannot hold. Either way the sun is out:
            // drawing a shadow from half a map would be worse than drawing none.
            page.shade_nodes = 0;
            page.shade_pitch = 1.0;
        }

        let Some(mut buffer) = buffers.get_mut(&self.frame) else {
            return;
        };
        // Cleared and refilled rather than edited: the render world *takes* the data
        // out of this asset when it extracts it, so what is here after the first frame
        // is an empty page with the right size. The buffer on the card is not
        // reallocated — same size, same usage, same label — so this is a write into it
        // and nothing else.
        buffer.clear();
        buffer.extend(std::iter::once(**page));
    }

    /// Rebuild the bank if the boat has sailed off the one we have.
    ///
    /// Depth is a function of *place*, not of the frame, so this is a cache with the
    /// only invalidation it can have: the map's own middle, snapped to its node grid, and
    /// the islands it was built from.
    fn bank(&mut self, scene: &Scene, buffers: &mut Assets<ShaderBuffer>) {
        let Some(land) = scene.land.filter(|_| scene.ink) else {
            // Nothing to draw a bank from. The shader is told by the node count; the
            // buffer is left exactly as it is.
            self.page.sand_nodes = 0;
            self.page.sand_pitch = 1.0;
            self.anchor = None;
            self.islands = None;
            return;
        };
        let pitch = 2.0 * SAND_REACH / (SAND_NODES - 1) as f64;
        let snapped = DVec2::new(
            (scene.boat.x / pitch).round() * pitch,
            (scene.boat.y / pitch).round() * pitch,
        );
        let same_land = self
            .islands
            .as_ref()
            .is_some_and(|had| Arc::ptr_eq(had, land));
        let corner = snapped - DVec2::splat(SAND_REACH);
        if self.anchor != Some(snapped) || !same_land {
            let pool = bevy::tasks::ComputeTaskPool::get();
            // A row to a task. Sixteen thousand soundings of a distance field is real
            // work — a millisecond on one core — and it would land as a hitch every few
            // seconds if it were done on one.
            self.depths
                .par_chunk_map_mut(pool, SAND_NODES, |row, nodes| {
                    let y = corner.y + row as f64 * pitch;
                    for (ix, slot) in nodes.iter_mut().enumerate() {
                        *slot = land.depth(DVec2::new(corner.x + ix as f64 * pitch, y)) as f32;
                    }
                });
            self.anchor = Some(snapped);
            self.islands = Some(land.clone());
            if let Some(mut buffer) = buffers.get_mut(&self.bank) {
                buffer.clear();
                buffer.extend(self.depths.iter().copied());
            }
        }
        self.page.sand_nodes = SAND_NODES as u32;
        self.page.sand_pitch = pitch as f32;
        self.page.to_sand = small(scene.boat - corner);
    }
}

impl Frame {
    fn zeroed() -> Self {
        bytemuck::Zeroable::zeroed()
    }
}

/// A difference taken in double precision, handed over as a pair of floats.
fn small(d: DVec2) -> [f32; 2] {
    [d.x as f32, d.y as f32]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::offset_of;

    /// The one thing that cannot be caught by looking at the picture.
    ///
    /// WGSL lays a storage struct out by its own rules — a `vec4` on sixteen bytes, a
    /// `vec2` on eight — and Rust lays `repr(C)` out by the alignment of its fields,
    /// which here is four throughout. The two agree only because every member of this
    /// struct happens to land where WGSL wants it. Nothing enforces that but this test,
    /// and a buffer read at the wrong offset does not fail: it draws a sea with the
    /// sun's colour in the wave height.
    #[test]
    fn the_page_is_laid_out_the_way_the_shader_reads_it() {
        assert_eq!(offset_of!(Frame, eye), 0);
        assert_eq!(offset_of!(Frame, dome_south), 112);
        assert_eq!(offset_of!(Frame, to_parcel), 128);
        assert_eq!(offset_of!(Frame, downwind), 160);
        assert_eq!(offset_of!(Frame, hs), 168);
        assert_eq!(offset_of!(Frame, components), 200);
        assert_eq!(offset_of!(Frame, rings), 224);
        assert_eq!(offset_of!(Frame, spacings), 628);
        assert_eq!(offset_of!(Frame, wave), 1032);
        assert_eq!(offset_of!(Frame, shade), 1704);
        // Every vec4 on a multiple of sixteen, every vec2 on a multiple of eight, and
        // the struct itself a multiple of sixteen — which is what WGSL will round the
        // binding size up to, and a binding one byte short of its layout is a validation
        // error at pipeline creation.
        assert_eq!(size_of::<Frame>() % 16, 0);
        assert_eq!(size_of::<Frame>(), 18_096);
        assert_eq!(size_of::<Wave>(), 32);
        assert_eq!(offset_of!(Wave, direction), 16);
    }

    /// The shadow map the clouds build has to be the one the shader expects.
    #[test]
    fn the_shadow_map_fits_the_page() {
        let (nodes, _, _, _) = ShadowField::clear().as_map();
        assert_eq!(nodes, 0, "a clear sky has no map");
        // And the one with cloud in it is square and this size — checked where it is
        // written, because `clouds::MAP` is private to that module.
        assert_eq!(SHADE_NODES, 64);
    }
}

/// The shader, compiled and run, against the sea the hull sails in.
///
/// **This is the whole defence of the duplication.** There are two implementations of
/// the Gerstner sum now — `sail-core::water` in Rust for the physics, `sea_wave.wesl` in
/// WGSL for the picture — and nothing in a type system can make them agree. What can is
/// asking both the same questions: this builds a sea, hands the shader exactly the
/// buffer the game hands it, runs the real file on the real card and compares.
///
/// It therefore checks three things at once that are hard to check separately: the
/// arithmetic, the layout of the page (a buffer read at the wrong offset would come back
/// nonsense), and that the file compiles at all — which is otherwise found out by
/// launching the game and seeing no water.
///
/// If there is no card to run it on, these fail rather than pass. A consistency test
/// that quietly skips is worse than no test, because what it leaves behind is the
/// *memory* of having checked.
#[cfg(test)]
mod on_the_card {
    use super::*;
    use sail_core::water::{WaterSurface, WindSea};

    /// The sea both sides are asked about. A fresh breeze over a short fetch, which is
    /// the steep awkward spectrum rather than a flattering one.
    fn sea() -> WindSea {
        let mut sea = WindSea::new(10.0, 40_000.0, 1.2, 0x5EA);
        // Somewhere in a lee, so the shelter is not one: it is folded into the
        // amplitudes on the way over and that is exactly the kind of step that can be
        // dropped without anything saying so.
        sea.set_shelter(0.8);
        sea
    }

    /// Where the boat is. Twenty-three kilometres from the origin and four hours into
    /// the passage, which is the point: the phases the card is handed are tens of
    /// thousands of radians before they are wrapped.
    const AT: DVec2 = DVec2::new(18_400.0, -13_750.0);
    const T: f64 = 14_400.0;

    /// The page, filled the way `write` fills it, for a sea and a grid.
    fn page(water: &WindSea) -> Box<Frame> {
        let mut page = Box::new(Frame::zeroed());
        for (k, slot) in page.rings.iter_mut().enumerate() {
            *slot = crate::world::half(k);
        }
        for (k, slot) in page.spacings.iter_mut().enumerate() {
            *slot = crate::world::spacing(crate::world::GRID / 2 + k);
        }
        let mut phased = Vec::new();
        water.phased(AT, T, &mut phased);
        for (slot, c) in page.wave.iter_mut().zip(phased.iter()) {
            *slot = Wave {
                amplitude: c.amplitude as f32,
                k: c.k as f32,
                wavelength: c.wavelength as f32,
                theta0: c.phase as f32,
                direction: [c.direction.x as f32, c.direction.y as f32],
                pad: [0.0; 2],
            };
        }
        page.components = phased.len() as u32;
        page.tile = 24.0;
        page.shadow_reach = 5_000.0;
        page.hs = water.significant_height() as f32;
        page
    }

    /// The shipped shader, with its bindings moved to a group this harness can bind, and
    /// a handful of entry points that do nothing but ask it questions.
    fn source() -> String {
        let shipped = include_str!("../../../assets/sea_wave.wesl");
        // The one thing the harness has to change: the game's material bindings live in
        // whichever group the engine says, and a compute pass has one group of its own.
        let moved = shipped
            .replace(
                "@group(constants::MATERIAL_BIND_GROUP) @binding(100)",
                "@group(0) @binding(0)",
            )
            .replace(
                "@group(constants::MATERIAL_BIND_GROUP) @binding(101)",
                "@group(0) @binding(1)",
            );
        assert!(
            moved.contains("@group(0) @binding(0)") && moved.contains("@group(0) @binding(1)"),
            "the shader's bindings have moved; this harness is testing nothing"
        );
        format!(
            "{moved}
@group(0) @binding(2) var<storage, read> asked: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> answered: array<vec4<f32>>;

@compute @workgroup_size(1)
fn probe_sum(@builtin(global_invocation_id) id: vec3<u32>) {{
    let q = asked[id.x];
    let water = surface_resolved(q.xy, q.z);
    answered[id.x * 2u] = vec4<f32>(water.height, water.slope.x, water.slope.y, 0.0);
    answered[id.x * 2u + 1u] = vec4<f32>(water.drift.x, water.drift.y, 0.0, 0.0);
}}

@compute @workgroup_size(1)
fn probe_resolution(@builtin(global_invocation_id) id: vec3<u32>) {{
    answered[id.x] = vec4<f32>(resolution_at(asked[id.x].xy), 0.0, 0.0, 0.0);
}}

@compute @workgroup_size(1)
fn probe_tone(@builtin(global_invocation_id) id: vec3<u32>) {{
    answered[id.x] = vec4<f32>(ink_tone(asked[id.x].x), 0.0);
}}

@compute @workgroup_size(1)
fn probe_depth(@builtin(global_invocation_id) id: vec3<u32>) {{
    answered[id.x] = vec4<f32>(depth_at(asked[id.x].xy), 0.0, 0.0, 0.0);
}}

@compute @workgroup_size(1)
fn probe_glass(@builtin(global_invocation_id) id: vec3<u32>) {{
    let sun = asked[id.x * 3u].xyz;
    let view = asked[id.x * 3u + 1u].xyz;
    let normal = asked[id.x * 3u + 2u].xyz;
    answered[id.x] = vec4<f32>(seen_through(sun, view, normal), 0.0, 0.0, 0.0);
}}
"
        )
    }

    /// Run one of those entry points and hand back what it wrote.
    fn ask(page: &Frame, entry: &str, asked: &[[f32; 4]], answers: usize) -> Vec<[f32; 4]> {
        ask_with(
            page,
            &vec![0.0f32; SAND_NODES * SAND_NODES],
            entry,
            asked,
            answers,
        )
    }

    pub(super) fn ask_with(
        page: &Frame,
        soundings: &[f32],
        entry: &str,
        asked: &[[f32; 4]],
        answers: usize,
    ) -> Vec<[f32; 4]> {
        use wgpu::util::DeviceExt;
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .expect("no graphics adapter: this test has nothing to check the shader against");
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("the adapter would not give us a device");

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sea_wave"),
            source: wgpu::ShaderSource::Wgsl(source().into()),
        });
        let frame = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("frame"),
            contents: bytemuck::bytes_of(page),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let bank = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("bank"),
            contents: bytemuck::cast_slice(soundings),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let asked_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("asked"),
            contents: bytemuck::cast_slice(asked),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let bytes = (answers * 16) as u64;
        let answered = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("answered"),
            size: bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let read_back = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("read back"),
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Spelled out rather than taken off the pipeline. A layout derived from the
        // shader carries only the bindings that entry point happens to read — and these
        // entry points each read a different two or three of the four — so the derived
        // layout and the bind group would disagree, differently, for each of them.
        let readable = |binding: u32, writable: bool| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage {
                    read_only: !writable,
                },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("the sea's own bindings"),
            entries: &[
                readable(0, false),
                readable(1, false),
                readable(2, false),
                readable(3, true),
            ],
        });
        let plumbing = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(entry),
            layout: Some(&plumbing),
            module: &module,
            entry_point: Some(entry),
            compilation_options: Default::default(),
            cache: None,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: frame.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: bank.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: asked_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: answered.as_entire_binding(),
                },
            ],
        });

        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(answers as u32, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&answered, 0, &read_back, 0, bytes);
        queue.submit([encoder.finish()]);

        let slice = read_back.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| {
            r.expect("the card would not hand it back")
        });
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("the card never finished");
        let view = slice
            .get_mapped_range()
            .expect("the mapped range went missing");
        let out: Vec<[f32; 4]> = bytemuck::cast_slice(&view).to_vec();
        drop(view);
        read_back.unmap();
        out
    }

    /// **Height, slope and drift, in both languages, at the same patches of water.**
    #[test]
    fn the_card_and_the_hull_are_in_the_same_sea() {
        let water = sea();
        let page = page(&water);
        // Offsets from the boat in the simulation's own frame, out to the far edge of
        // the near field, with the band limit each one would be asked for.
        let mut asked = Vec::new();
        let mut want = Vec::new();
        for i in 0..64 {
            let a = i as f64 * 0.981;
            // Close in where the detail is, and then out across the band-limited part of
            // the grid, where the phase the card adds to the one it was given is itself
            // thousands of radians. That is where single precision has to be shown to be
            // enough, not under the boat.
            let r = if i < 32 {
                0.4 + i as f64 * 3.3
            } else {
                200.0 + (i - 32) as f64 * 42.0
            };
            let local = DVec2::new(a.cos() * r, a.sin() * r);
            let min_wavelength =
                crate::world::drawn_resolution(Vec2::new(local.x as f32, -local.y as f32));
            asked.push([local.x as f32, local.y as f32, min_wavelength as f32, 0.0]);
            want.push(water.surface_resolved(AT + local, T, min_wavelength));
        }
        let got = ask(&page, "probe_sum", &asked, asked.len() * 2);

        let mut worst_height = 0.0_f64;
        let mut worst_slope = 0.0_f64;
        let mut worst_drift = 0.0_f64;
        for (i, parcel) in want.iter().enumerate() {
            let a = got[i * 2];
            let b = got[i * 2 + 1];
            worst_height = worst_height.max((a[0] as f64 - parcel.height).abs());
            worst_slope = worst_slope
                .max((a[1] as f64 - parcel.slope.x).abs())
                .max((a[2] as f64 - parcel.slope.y).abs());
            worst_drift = worst_drift
                .max((b[0] as f64 - parcel.drift.x).abs())
                .max((b[1] as f64 - parcel.drift.y).abs());
        }
        // Bounds measured rather than hoped for, and they are what single precision can
        // do with a phase of a few hundred radians: a float carries about seven digits,
        // so a thousand radians of phase is worth a ten-thousandth of a radian of error,
        // and an amplitude of a metre turns that into a tenth of a millimetre.
        //
        // What matters is the comparison, not the number: the hull floats on the first
        // sea and the eye looks at the second, and a millimetre is four orders below
        // anything the eye can see at the scale of a wave.
        // Printed as well as asserted: the margin is the interesting number, and a
        // bound that has quietly crept up to its limit should be visible as it happens
        // rather than on the day it fails.
        eprintln!(
            "the card is within {worst_height:.6} m, {worst_slope:.7} rad and \
             {worst_drift:.6} m of the sea the hull sails in"
        );
        assert!(
            worst_height < 2.0e-3,
            "the drawn sea stands {worst_height:.5} m from the sailed one"
        );
        assert!(
            worst_slope < 1.0e-5,
            "the drawn surface leans {worst_slope:.6} rad from the sailed one"
        );
        assert!(
            worst_drift < 1.0e-4,
            "the drawn water has drifted {worst_drift:.5} m from the sailed water"
        );
    }

    /// And both sides band-limit the same way.
    ///
    /// This is the agreement that keeps a mark of foam *on* the surface. Spray, wake,
    /// rain and the fishing float all ask `drawn_resolution` for the shortest wave the
    /// mesh carries where they are, and then ask the water for a surface filtered to it.
    /// If the card answered that question differently, every one of those marks would
    /// sit a little above or below the water it belongs to, and blink as the grid slid
    /// under it.
    #[test]
    fn the_card_cuts_the_spectrum_off_where_the_mesh_does() {
        let water = sea();
        let page = page(&water);
        let mut asked = Vec::new();
        let mut want = Vec::new();
        for i in 0..48 {
            // Grid offsets in the mesh's own frame, which is what both sides take.
            let offset = Vec2::new(i as f32 * 13.0 - 300.0, 420.0 - i as f32 * 21.0);
            asked.push([offset.x, offset.y, 0.0, 0.0]);
            want.push(crate::world::drawn_resolution(offset));
        }
        let got = ask(&page, "probe_resolution", &asked, asked.len());
        for (i, &expected) in want.iter().enumerate() {
            assert!(
                (got[i][0] as f64 - expected).abs() < 1e-4,
                "at {:?} the card carries waves down to {:.3} m and the rest of the \
                 game thinks {:.3} m",
                asked[i],
                got[i][0],
                expected
            );
        }
    }

    /// The shimmer, stated as a number: a vertex whose tone creeps may not change colour
    /// by a whole band because of it.
    ///
    /// The fault this replaces was a `floor`, which moves a third of the brightness
    /// range the instant its argument crosses an integer — and across forty thousand
    /// vertices of moving water some are always crossing. The gain from input to output
    /// here is 1.5 × 4 / (0.16 × 3), twelve and a half, so a thousandth of tone moves
    /// the drawn tone by 0.0125: bounded and proportional, which is the whole point.
    #[test]
    fn a_creeping_tone_may_not_jump_a_band() {
        let water = sea();
        let page = page(&water);
        const STEP: f32 = 0.001;
        let asked: Vec<[f32; 4]> = (0..1000)
            .map(|i| [i as f32 * STEP, 0.0, 0.0, 0.0])
            .collect();
        let got = ask(&page, "probe_tone", &asked, asked.len());
        let mut worst: f32 = 0.0;
        let mut flat = 0;
        for pair in got.windows(2) {
            worst = worst.max((pair[1][0] - pair[0][0]).abs());
            if (pair[1][0] - pair[0][0]).abs() < 1e-6 {
                flat += 1;
            }
            // And the contour line with it, which was the second hard threshold: it used
            // to switch on at full strength the moment a vertex crossed.
            assert!(
                (pair[1][1] - pair[0][1]).abs() < 0.12,
                "the contour jumped {:.3}",
                (pair[1][1] - pair[0][1]).abs()
            );
        }
        assert!(
            worst < 0.02,
            "a tone creeping by {STEP} moved the drawn tone by {worst:.4}, which is a \
             vertex flickering between two bands"
        );
        // And it is still a drawing: most of the sea sits on one of four flat tones,
        // because that is the whole of the style. A staircase with no treads is a ramp.
        let share = flat as f32 / (got.len() - 1) as f32;
        assert!(
            share > 0.75,
            "only {:.0}% of the tone range is a flat band, which is a gradient with \
             steps in it rather than a drawing",
            share * 100.0
        );
        // The treads are where they always were: four tones evenly spread.
        for (i, at) in [0.20_f32, 0.45, 0.70, 0.95].iter().enumerate() {
            let tone = got[(at / STEP).round() as usize][0];
            let want = i as f32 / 3.0;
            assert!(
                (tone - want).abs() < 1e-3,
                "band {i} sits at {tone:.3} and should be at {want:.3}"
            );
        }
    }

    /// **Up-sun you see through a wave; down-sun you see off it.**
    ///
    /// The whole of the translucency is in that sentence, and it is the one part of it
    /// that can be asked a question without a sea in front of you.
    #[test]
    fn the_crest_only_glows_with_the_sun_behind_it() {
        let water = sea();
        let page = page(&water);
        // Looking north along the water at a crest leaning towards us.
        let view = Vec3::new(0.0, 0.35, -0.94).normalize();
        let facing = Vec3::new(0.0, 0.7, -0.71).normalize();
        // Sun low and beyond the wave, so the light has to cross the water to reach us:
        // `view` runs from the water *to the eye*, so a sun on the far side points the
        // other way along it. Getting that round the wrong way is the easiest mistake
        // here and this test made it first.
        let behind = Vec3::new(0.0, 0.22, 0.98).normalize();
        // And the same sun over our own shoulder.
        let over_us = Vec3::new(0.0, 0.22, -0.98).normalize();
        // A facet turned away gives nothing however the sun lies: there is no water in
        // the way, only water beside.
        let away = Vec3::new(0.0, 0.7, 0.71).normalize();
        let pack = |v: Vec3| [v.x, v.y, v.z, 0.0];
        let asked = vec![
            pack(behind),
            pack(view),
            pack(facing),
            pack(over_us),
            pack(view),
            pack(facing),
            pack(behind),
            pack(view),
            pack(away),
        ];
        let got = ask(&page, "probe_glass", &asked, 3);
        assert!(
            got[0][0] > 0.5,
            "a crest with the sun behind it came out at {:.2}",
            got[0][0]
        );
        assert!(
            got[1][0] < 0.01,
            "a crest with the sun behind the *eye* glowed at {:.2}",
            got[1][0]
        );
        assert!(got[2][0] < 0.01, "a facet turned away glowed");
    }
}

/// The bank, end to end: a real archipelago, the real filling code, and the shader
/// reading it back.
///
/// **The one part of the drawn sea that a screenshot will not show you.** The shoal
/// tone only appears within a couple of hundred metres of a shore, which is a place the
/// boat takes ten minutes to reach, so a fault in the bank map's corner or its pitch
/// would sit there unseen until somebody sailed close inshore and found the green in the
/// wrong place — or, worse, found nothing and assumed it had always been like that.
///
/// So the whole path is run instead: `write` builds the map from the islands, the buffer
/// goes to the card as it stands, and the shader is asked how deep the water is at
/// points the processor can answer for itself.
#[cfg(test)]
mod the_bank {
    use super::on_the_card::ask_with;
    use super::*;
    use bevy::tasks::{ComputeTaskPool, TaskPool};

    #[test]
    fn the_card_sounds_the_same_bottom_the_sounder_does() {
        // The parallel fill wants a pool; a test has no app to have made one.
        ComputeTaskPool::get_or_init(TaskPool::default);

        let land = Arc::new(crate::islands::generate(58));
        // A kilometre south of the island this seed puts to the north, which is close
        // enough that the shelf is inside the map and far enough that the boat is in
        // open water — the geometry the drawn sea actually meets.
        let boat = DVec2::new(0.0, 0.0);
        let mut buffers = Assets::<ShaderBuffer>::default();
        let mut materials = Assets::<SeaMaterial>::default();
        let rings: Vec<f32> = (0..RINGS).map(crate::world::half).collect();
        let spacings: Vec<f32> = (0..RINGS)
            .map(|k| crate::world::spacing(crate::world::GRID / 2 + k))
            .collect();
        let mut card = SeaCard::new(
            &rings,
            &spacings,
            StandardMaterial::default(),
            &mut buffers,
            &mut materials,
        );

        let water = sail_core::water::WindSea::new(10.0, 40_000.0, 1.2, 0x5EA);
        let dome = Dome::default();
        let shadows = ShadowField::clear();
        card.write(
            &Scene {
                ink: true,
                eye: Vec3::new(0.0, 2.0, 6.0),
                sun: Vec3::new(0.3, 0.6, 0.7),
                deep: Vec3::ZERO,
                far: Vec3::ZERO,
                body: Vec3::ZERO,
                dome: &dome,
                boat,
                time: 120.0,
                uv_base: DVec2::ZERO,
                current: DVec2::ZERO,
                downwind: DVec2::new(0.0, 1.0),
                hs: 1.0,
                typical: 0.05,
                whitecapping: 1.0,
                longest: water.longest_wavelength(),
                tile: 24.0,
                shadow_reach: 5_000.0,
                shadows: &shadows,
                land: Some(&land),
                water: &water,
            },
            &mut buffers,
        );

        let page = *buffers
            .get(&card.frame)
            .expect("the page went missing")
            .cast_slice::<Frame>()
            .expect("the page is not a page")
            .first()
            .unwrap();
        assert_eq!(page.sand_nodes, SAND_NODES as u32, "no bank was built");
        let soundings: Vec<f32> = buffers
            .get(&card.bank)
            .expect("the bank went missing")
            .cast_slice::<f32>()
            .expect("the bank is not soundings")
            .to_vec();

        // Points spread over the shelf north of the boat, where the ground actually
        // changes. Asked in the sea mesh's own frame, which is what the shader takes.
        let mut asked = Vec::new();
        let mut want = Vec::new();
        for i in 0..40 {
            let local = DVec2::new(-300.0 + i as f64 * 15.0, 400.0 + i as f64 * 15.0);
            asked.push([local.x as f32, local.y as f32, 0.0, 0.0]);
            want.push(land.depth(boat + local));
        }
        let got = ask_with(&page, &soundings, "probe_depth", &asked, asked.len());

        // Twenty metres between nodes, bilinear in between, so the card's answer is an
        // interpolation of a field the sounder evaluates exactly. The bound is what the
        // ground's own slope does over half a node — a shelf that runs from forty metres
        // to nothing in four hundred — and it is the map's resolution that is being
        // asserted here, not the arithmetic: a wrong corner or a transposed index would
        // be hundreds of metres out and would miss by the whole depth range.
        let mut worst = 0.0_f64;
        for (i, &expected) in want.iter().enumerate() {
            worst = worst.max((got[i][0] as f64 - expected).abs());
        }
        eprintln!("the card's bank is within {worst:.2} m of the sounder's");
        assert!(
            worst < 3.0,
            "the card thinks the bottom is {worst:.1} m from where the sounder does, \
             which is not interpolation — it is the wrong piece of ground"
        );
    }
}
