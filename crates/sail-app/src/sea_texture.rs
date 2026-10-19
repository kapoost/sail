//! Procedural water textures.
//!
//! No image files, for the same reason there are no audio files: a water surface is
//! summed ripples and summed noise, so generating it is the method rather than a
//! shortcut. It also means the tile is seamless by construction instead of by a
//! retoucher's patience — every component is given an integer number of cycles across
//! the tile, so the edges match exactly.
//!
//! Two maps come out of here. The normal map carries the capillary and short wind
//! waves that live far below the mesh resolution: at 0.7 m between vertices the
//! geometry cannot express a 20 cm ripple, and it is precisely those ripples that
//! make water glitter. The roughness map varies on a much longer scale, which is what
//! produces the dark patches and bright patches a real surface has — cat's paws,
//! where a gust has ruffled the water and the rest is still slick.

use crate::curve::smoothstep;
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// Texture resolution. A thousand texels across a twenty-four metre tile is a little
/// over two centimetres each, which is finer than the eye resolves at any distance
/// you see water from a boat.
const SIZE: u32 = 1024;
/// How much world the tile covers, metres.
///
/// Twenty-four, up from eleven and six before that. The repeat is what gives a
/// procedural sea away, and it gives it away worst from the high chase camera, where
/// a dozen copies of the same patch are in view at once. Each enlargement has been
/// paired with more resolution and a wider band of ripples, so the detail per metre
/// stays put and only the period of the repeat grows — stretching the tile alone
/// would just turn the ripples into waves, which the mesh is already drawing.
///
/// Properly breaking a repeat needs detail at two scales and therefore two texture
/// lookups in a shader. This is still the honest half-measure, just a bigger one.
pub const TILE_METRES: f32 = 24.0;

/// How far the texture coordinates are pushed about, in tiles, and over what distance.
///
/// **A repeating texture is not seen as repetition — it is seen as a grid.** Twenty-four
/// metres of ripples across three hundred metres of water is a dozen copies of one
/// patch, and what gives it away is not that the patches are the same but that their
/// corners line up: the eye finds the lattice long before it recognises the content.
/// Reported from the deck as the water being repetitive at the finer drawn width, which
/// is exactly when a lattice becomes legible.
///
/// So the coordinates are warped by a slow swirl before they are used. The same patches
/// are still there — there is one texture and that does not change — but they no longer
/// land on a grid: each copy is offset and sheared a little differently from its
/// neighbours, and a lattice with no straight lines in it stops being a lattice. It
/// costs two noise samples a vertex and no memory at all.
///
/// Two swirls rather than one, because one was not enough to hide the grid and a single
/// strong one only trades a lattice for a smear. The long one moves whole tiles about
/// over two hundred metres; the short one bends their edges over sixty. Between them a
/// copy of the tile is displaced, rotated a little by the shear, and distorted
/// differently from its neighbours, which is as close to *not repeating* as one texture
/// lookup gets.
///
/// The stretch between two vertices is what bounds them: 1.1 tiles over two hundred
/// metres and 0.35 over sixty both come out near a quarter of a tile per ten metres of
/// water, against a vertex every seventy centimetres. Smooth where it is looked at
/// closely and scrambled at the distance where the repeat used to be legible.
const WARP_LONG: f32 = 1.1;
const WARP_LONG_OVER: f32 = 200.0;
const WARP_SHORT: f32 = 0.35;
const WARP_SHORT_OVER: f32 = 60.0;

/// Where a patch of water reads its ripples from.
///
/// One function because the near field and the far field both ask, and a seam between
/// two different answers would be a line across the sea — which is the one thing worse
/// than a repeat.
pub fn ripple_uv(parcel: Vec2) -> Vec2 {
    // One octave, not two. This is asked four times at every one of forty thousand
    // vertices, and a second octave of a field whose whole job is to be slow buys
    // nothing anybody can see at eight milliseconds a frame — measured: sixty frames a
    // second with it gone against forty with it in.
    let swirl = |seed: u64, offset: Vec2, over: f32| -> f32 {
        let at = (parcel + offset) / over;
        sail_core::noise::value(glam::DVec2::new(at.x as f64, at.y as f64), seed) as f32
    };
    let warp = Vec2::new(
        swirl(0x5EA_1A11, Vec2::ZERO, WARP_LONG_OVER),
        swirl(0x5EA_1A12, Vec2::splat(37.0), WARP_LONG_OVER),
    ) * WARP_LONG
        + Vec2::new(
            swirl(0x5EA_1A13, Vec2::splat(113.0), WARP_SHORT_OVER),
            swirl(0x5EA_1A14, Vec2::splat(211.0), WARP_SHORT_OVER),
        ) * WARP_SHORT;
    Vec2::new(parcel.x / TILE_METRES, -parcel.y / TILE_METRES) + warp
}

/// Resolution of the animated frames.
///
/// Half the still map's. Forty-eight frames is the whole cost of this feature and it is
/// paid in memory: at 1024 it would be a quarter of a gigabyte of texture for moving
/// water, which is not a trade anyone would take. At 512 a tile texel is 4.7 cm and the
/// shortest ripple in the spectrum still spans eleven of them, which is plenty to carry
/// a smooth normal — and the window this is played in is 480 pixels across.
pub const ANIM_SIZE: u32 = 512;

/// How long the ripple animation takes to come back to itself, seconds.
///
/// Every component's frequency is rounded to a whole number of cycles in this window,
/// which is what makes the loop seamless — the alternative is a jump every time it
/// wraps, and a jump in water is the most visible thing there is. The rounding is also
/// the cost: at three seconds the granularity is a third of a hertz, so the shortest
/// ripples land within two percent of their true speed and the longest, which are slow
/// and small, run up to a third fast. That is the right way round. The eye follows the
/// band around a metre, and that band is nearly exact.
pub const RIPPLE_LOOP: f32 = 3.0;

/// How many frames the loop is cut into.
///
/// Twelve a second. Below that the shorter ripples strobe; above it the memory climbs
/// for motion nobody is looking at, since what the frames carry is the ripples'
/// *travel* and evolution while the slow surface current is still carried smoothly by
/// the texture coordinates.
pub const RIPPLE_FRAMES: usize = 36;

/// One ripple: integer cycles across the tile, so the result tiles seamlessly.
struct Ripple {
    cycles_u: f32,
    cycles_v: f32,
    amplitude: f32,
    phase: f32,
    /// Angular frequency, radians per second, rounded to close the loop.
    omega: f32,
}

/// The ripple spectrum, generated rather than written out by hand.
///
/// A hand-written list of eight components was fine for a six-metre tile and is the
/// wrong tool for a twenty-four metre one: the point of the larger tile is to carry
/// the same fine detail *and* a longer repeat, which needs components spread across a
/// much wider band than anyone wants to type. Describing the spectrum by a rule also
/// means it follows the tile if the tile changes again.
///
/// Amplitudes fall as `1/k`. The map encodes *slope*, and for a wind sea the slope
/// spectrum is close to flat with wavenumber while the elevation spectrum falls as
/// `k^-2`; one power of `k` comes back out in taking the gradient, so `1/k` on the
/// elevation amplitude is what leaves the glitter looking right at every scale rather
/// than piling all of it into the short waves.
///
/// Directions are fanned rather than aligned, because a wind sea is short-crested:
/// aligned components give corduroy, which is exactly what this looked like before
/// anyone fanned them.
fn ripples() -> Vec<Ripple> {
    // **Nothing longer than a quarter of the tile.** Reported from the deck as the
    // bright reflections lining up in rows, and that is what a long component in a
    // tiled texture *is*: with amplitudes falling as `1/k`, the one-cycle and two-cycle
    // components carried most of the picture's large-scale light and dark, they are
    // identical in every copy of the tile, and so the whole sea was banded at
    // twenty-four metres however well the short ripples behaved.
    //
    // The band also has no business being there. The mesh draws the sea down to about
    // eight metres of wavelength and the texture is what fills in below the vertices;
    // a six-metre feature painted on is a wave drawn twice, once by geometry that moves
    // and once by a picture that repeats. Starting at four cycles across the tile leaves
    // the texture carrying six metres and under — a scale at which a twenty-four metre
    // repeat has nothing in it for the eye to line up.
    //
    // It costs no brightness: amplitude is `0.075/k`, so every component contributes the
    // same slope, and dropping three of twenty-six leaves the glitter where it was.
    let longest = 4.0_f32;
    let shortest = (TILE_METRES / 0.55).round();
    let count = 26;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let t = i as f32 / (count - 1) as f32;
        // Geometric in wavenumber, so every octave gets the same number of components.
        let k = longest * (shortest / longest).powf(t);
        // A golden-angle fan: never repeats, never clusters.
        let angle = i as f32 * 2.399_963_2;
        let cycles_u = (k * angle.cos()).round();
        let cycles_v = (k * angle.sin()).round();
        if cycles_u == 0.0 && cycles_v == 0.0 {
            continue;
        }
        let k_actual = (cycles_u * cycles_u + cycles_v * cycles_v).sqrt().max(1.0);
        // Dispersion, so each ripple travels at its own speed rather than the whole
        // pattern sliding at one. This is the thing the texture could not do before:
        // a still image advected downwind moves every scale at the same rate, which is
        // the one behaviour water never has. Short waves are slow and long waves are
        // fast, and that difference *is* the look of a water surface — a pattern that
        // keeps reforming rather than one that marches.
        //
        // Gravity and surface tension both, because at these sizes neither dominates
        // alone: at half a metre the gravity term still carries it, at two centimetres
        // the capillary term would, and the crossover sits inside the band.
        const G: f32 = 9.80665;
        const SIGMA_OVER_RHO: f32 = 7.4e-5;
        let k_rad = std::f32::consts::TAU * k_actual / TILE_METRES;
        let omega_true = (G * k_rad + SIGMA_OVER_RHO * k_rad * k_rad * k_rad).sqrt();
        // Rounded to a whole number of cycles in the loop. At least one, or the slowest
        // components freeze — and frozen large-scale structure in a tiled texture is
        // precisely what gives a procedural sea away.
        let cycles_in_loop = (omega_true * RIPPLE_LOOP / std::f32::consts::TAU)
            .round()
            .max(1.0);
        out.push(Ripple {
            cycles_u,
            cycles_v,
            amplitude: 0.075 / k_actual,
            omega: std::f32::consts::TAU * cycles_in_loop / RIPPLE_LOOP,
            // Irrational stride, so no two components ever line up to make a seam.
            phase: (i as f32 * 2.399_963_2) % std::f32::consts::TAU,
        });
    }
    out
}

/// Tangent-space normal map for the small-scale ripples.
pub fn normal_map() -> Image {
    normal_map_at_size(SIZE, 0.0)
}

/// The whole loop, as one image per frame.
///
/// Built on every core there is, because the serial version is the difference between
/// a game that starts and a game you wait for: thirty-six frames is thirty-six times
/// the work of the one map this used to make.
pub fn normal_map_frames() -> Vec<Image> {
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(RIPPLE_FRAMES);
    let mut out: Vec<Option<Image>> = (0..RIPPLE_FRAMES).map(|_| None).collect();
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for (chunk, slot) in out.chunks_mut(RIPPLE_FRAMES.div_ceil(threads)).enumerate() {
            let base = chunk * RIPPLE_FRAMES.div_ceil(threads);
            handles.push(scope.spawn(move || {
                for (i, cell) in slot.iter_mut().enumerate() {
                    let t = (base + i) as f32 / RIPPLE_FRAMES as f32 * RIPPLE_LOOP;
                    *cell = Some(normal_map_at_size(ANIM_SIZE, t));
                }
            }));
        }
        for h in handles {
            let _ = h.join();
        }
    });
    out.into_iter().flatten().collect()
}

/// One frame of it, at a chosen resolution and a chosen moment in the loop.
pub fn normal_map_at_size(size: u32, t: f32) -> Image {
    let spectrum = ripples();
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    let n = size as f32;
    for y in 0..size {
        for x in 0..size {
            let u = x as f32 / n;
            let v = y as f32 / n;
            // Analytic gradient of the summed ripples. Exact, and far cheaper than
            // differencing a rendered height field.
            let mut du = 0.0;
            let mut dv = 0.0;
            for r in &spectrum {
                let k = std::f32::consts::TAU;
                let theta = k * (r.cycles_u * u + r.cycles_v * v) - r.omega * t + r.phase;
                let c = theta.cos() * r.amplitude * k;
                du += c * r.cycles_u;
                dv += c * r.cycles_v;
            }
            // Tangent space has z out of the surface.
            let normal = Vec3::new(-du, -dv, 1.0).normalize();
            data.push(encode_normal(normal.x));
            data.push(encode_normal(normal.y));
            data.push(encode_normal(normal.z));
            data.push(255);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        // Normal maps are vectors, not colour, so they must not be gamma-decoded.
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    build_mipmaps(&mut image, true);
    image.sampler = repeating();
    image
}

/// Roughness, in the glTF channel layout the renderer expects: green is roughness,
/// blue is metallic.
///
/// Varies on a long scale on purpose. Uniform roughness gives an evenly shiny sheet;
/// real water has slick patches and ruffled patches, and the contrast between them is
/// most of what reads as "water" at a distance.
pub fn roughness_map() -> Image {
    let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    let n = SIZE as f32;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let u = x as f32 / n;
            let v = y as f32 / n;
            let k = std::f32::consts::TAU;
            // Two long components and one short, all integer-cycled so it still tiles.
            let patch = 0.5
                + 0.26 * (k * (u + 0.5 * v)).sin()
                + 0.16 * (k * (2.0 * v - u) + 1.3).sin()
                + 0.08 * (k * (3.0 * u + 4.0 * v) + 2.2).sin();
            let roughness = (0.05 + 0.30 * patch.clamp(0.0, 1.0)).clamp(0.02, 0.6);
            data.push(0);
            data.push((roughness * 255.0) as u8);
            // Not metallic at all. Water's sky comes from Fresnel on a dielectric,
            // not from a metal's tinted specular.
            data.push(0);
            data.push(255);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    build_mipmaps(&mut image, false);
    image.sampler = repeating();
    image
}

/// How dark a stroke is, as a multiplier on the water under it.
pub const HATCH_DEPTH: f32 = 0.5;

/// The hatching, as a tonal art map: across is the stroke, down is how dark the water.
///
/// The drawn sea's strokes were drawn per *vertex* — a test of whether each vertex sat
/// inside a stroke, laid out in world metres. That cannot work and never could: the
/// strokes are three quarters of a metre apart and the vertices are one and three tenths
/// apart under the boat, so the pattern was being sampled at half a sample per period,
/// a seventh of what it needs. It was not hatching that shimmered, it was noise at the
/// grid's own frequency that happened to look like hatching while it stood still. The
/// same file states the rule it was breaking — `resolutions` holds the wave field to
/// four samples per wavelength, "not twice; twice is the Nyquist limit, at which a
/// moving sea visibly beats".
///
/// A mesh of that spacing cannot carry a stroke finer than two vertices, which is two
/// and a half metres of sea, so vertex colours are simply the wrong instrument. A
/// texture is the right one, and the whole apparatus is already here and already proven
/// on exactly this complaint: `build_mipmaps` exists because a 512-texel tile at a
/// grazing angle "aliases, and the aliasing crawls as the boat moves". Strokes in a
/// mipmapped, anisotropically-filtered texture are filtered by the card, at the pixel,
/// correctly, and they stop dead.
///
/// Two axes, which is what makes it a tonal art map rather than a pattern. **U** is the
/// stroke phase, addressed in world metres so the strokes belong to the water and ride
/// its orbital motion. **V** is the tone: each row carries a stroke of a different
/// width, none at all at the top and better than half the period at the bottom, so
/// "darker water, wider stroke" survives — that relation is the whole of why this reads
/// as shading rather than as stripes. A vertex picks its row by how dark it is.
///
/// Linear, not sRGB, because this multiplies the water rather than being a colour: half
/// here must mean half of what is under it, and an sRGB texture would decode 0.5 to
/// 0.21 and put the sea in a coal cellar. Mipping a multiplier in linear space is also
/// the correct average rather than merely the convenient one.
pub fn hatch_map() -> Image {
    /// Across one stroke period.
    const W: u32 = 128;
    /// Tone rows, lightest first.
    const H: u32 = 64;
    let mut data = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        let dark = y as f32 / (H - 1) as f32;
        // No stroke at all on the lightest water: on a page that tone is blank paper,
        // and a stroke that never goes away is a texture rather than shading. The
        // exponent puts most of the growth early, so the second tone already reads as
        // hatched and the darkest is nearly solid.
        let width = 0.62 * dark.powf(0.75);
        for x in 0..W {
            let u = (x as f32 + 0.5) / W as f32;
            // Centred in the tile, so the softened edges never straddle the wrap.
            let from_centre = (u - 0.5).abs();
            let half = width * 0.5;
            // One texel of soft edge, which is what keeps the texture itself clean at
            // mip zero. Everything coarser is the card's business.
            let edge = 1.0 / W as f32;
            let inside = 1.0 - smoothstep(half - edge, half + edge, from_centre);
            let value = 1.0 - (1.0 - HATCH_DEPTH) * inside;
            let byte = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
            data.extend_from_slice(&[byte, byte, byte, 255]);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: W,
            height: H,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    build_mipmaps(&mut image, false);
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        // Round the stroke, clamped down the tone: U is a repeating phase and V is a
        // position in a range. Repeating V would put the blackest hatching next to the
        // blank paper and draw a seam along it.
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::ClampToEdge,
        address_mode_w: ImageAddressMode::Repeat,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 4,
        ..ImageSamplerDescriptor::linear()
    });
    image
}

/// The two marks the spray is drawn with, side by side on one sheet.
///
/// The particles were untextured quads, which is a square of light, and a square of
/// light is what "the whitecaps are squares" meant — twice, from two different
/// directions, because the same quad was being used for two things that are not alike.
/// A speck off the bow is a *drop*: round, bright in the middle, gone at the edge. A
/// mark torn off a breaking crest is a *handful of foam*, and what foam does when it
/// comes off the top of a wave is tear into fingers. The print everybody means by a
/// drawn wave is built out of exactly those claws.
///
/// Left half is the drop, right half the claw. Additive, so this is light rather than
/// colour: black adds nothing, which is why it wants no alpha channel and why two marks
/// may overlap without anybody having to sort them back to front.
pub fn spray_marks() -> Image {
    const W: u32 = 128;
    // Six cells in a row: a drop, a bubble, and four frames of a handful of foam coming
    // apart.
    //
    // **The four are the animation.** A mark that only fades out is a mark that keeps
    // its shape while it disappears, which is not what foam does: foam *tears* — the
    // holes in it open, the edges go ragged, and what is left is a scatter of smaller
    // patches. A flipbook of four says that in the one channel this sheet has, and it
    // costs nothing at all because the frame is chosen by the vertex's own age when the
    // quad is laid down.
    pub const CELLS: u32 = 6;
    let mut data = vec![0u8; (W * CELLS * W * 4) as usize];
    for y in 0..W {
        for x in 0..W * CELLS {
            let cell = x / W;
            let u = ((x % W) as f32 + 0.5) / W as f32 * 2.0 - 1.0;
            let v = (y as f32 + 0.5) / W as f32 * 2.0 - 1.0;
            let value = match cell {
                0 => drop_mark(u, v),
                1 => bubble_mark(u, v),
                frame => torn_claw(u, v, frame - 2),
            };
            // The stride is the whole row of cells, and it was `W * 2` — the width of
            // this sheet back when it held a drop and a claw. Adding the bubble and the
            // four frames of tearing made the row three times longer and left the stride
            // where it was, so every row was written into the first third of the buffer,
            // on top of the rows before it: two thirds of the sheet stayed transparent
            // black and what was left was a solid white block with no shape in it.
            //
            // Every mark on the sea has been that block ever since, which is the whole
            // of "I cannot see the particles" and "there is no foam along the side"  —
            // the foam was drawn, in quantity, as a texture with no marks in it.
            let i = ((y * W * CELLS + x) * 4) as usize;
            let byte = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
            data[i] = byte;
            data[i + 1] = byte;
            data[i + 2] = byte;
            data[i + 3] = 255;
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: W * CELLS,
            height: W,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    build_mipmaps(&mut image, false);
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        // Clamped both ways: each half is a mark on its own and must not bleed into its
        // neighbour or wrap round to itself.
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mipmap_filter: ImageFilterMode::Linear,
        ..ImageSamplerDescriptor::linear()
    });
    image
}

/// A drop: bright in the middle, out by the rim, with nothing in the corners.
fn drop_mark(u: f32, v: f32) -> f32 {
    let r = (u * u + v * v).sqrt();
    1.0 - smoothstep(0.12, 0.92, r)
}

/// A handful of foam: round, with a lobed rim and a little reach to it.
///
/// Five deep lobes made a *star*, which is what it was reported as, and a star is the
/// one shape loose foam never takes — a point needs something to hold it out and foam
/// has nothing. Six shallow ones instead: enough to keep the rim from reading as a
/// circle stamped out with a punch, nowhere near enough to make corners. What is left
/// is a soft blob that is slightly not round, which is what a handful of water with air
/// in it looks like from ten metres.
///
/// It keeps a little more reach upwards than down, because the water is thrown over the
/// top of the wave and comes down, so the mark is not quite symmetrical about its own
/// middle. That asymmetry does more for it than the lobes do.
fn claw_mark(u: f32, v: f32) -> f32 {
    let r = (u * u + v * v).sqrt().max(1e-4);
    let angle = v.atan2(u);
    // Six, shallow, and offset so no lobe sits square on an axis.
    let lobes = (angle * 6.0 + 0.4).cos();
    let up = (v * 0.5 + 0.5).clamp(0.0, 1.0);
    let rim = 0.74 + 0.07 * lobes + 0.07 * up;
    // Soft all the way out. The hard outer edge was half of why it read as a drawn
    // shape rather than as foam; what an edge of foam does is get thinner.
    1.0 - smoothstep(rim * 0.35, rim, r)
}

/// A bubble: a rim with the light bent round it and a highlight where the sky is.
///
/// **This is the thing foam is made of**, and at the scale where you can see one at all
/// — alongside, in the wake, in a patch that has just stopped breaking — it is not a
/// white blob but a little sphere: dark in the middle where you are looking through it
/// into the water, bright round the edge where the surface turns away from you, and with
/// one hard glint high on it where the sky is reflected. The mark is drawn additively,
/// so the dark middle is simply nothing added and the sea shows through it, which is
/// exactly right and costs no sorting.
fn bubble_mark(u: f32, v: f32) -> f32 {
    let r = (u * u + v * v).sqrt();
    if r > 0.98 {
        return 0.0;
    }
    // The rim: brightest just inside the edge, where a sphere's surface is most nearly
    // side-on to the eye.
    let rim = smoothstep(0.52, 0.86, r) * (1.0 - smoothstep(0.86, 0.99, r));
    // The glint, up and to the left of centre, small and hard. A bubble has one and
    // only one, which is how the eye knows it is a sphere and not a ring.
    let (gx, gy) = (u + 0.33, v - 0.36);
    let glint = 1.0 - smoothstep(0.0, 0.20, (gx * gx + gy * gy).sqrt());
    // And the faintest wash across the whole of it, so it is a thing rather than an
    // outline.
    let body = 0.10 * (1.0 - smoothstep(0.2, 0.95, r));
    (rim * 0.55 + glint * 0.95 + body).clamp(0.0, 1.0)
}

/// A handful of foam, torn apart in four stages.
///
/// The holes are the animation: the same mark with more of it eaten away each frame, and
/// eaten in a fixed pattern rather than at random, so one speck's four frames are four
/// stages of the same patch rather than four different patches.
fn torn_claw(u: f32, v: f32, frame: u32) -> f32 {
    let whole = claw_mark(u, v);
    if frame == 0 {
        return whole;
    }
    // Lace: two fields of lumps at different scales, which is what the gaps in foam
    // look like — big holes with small ones through the webs between them.
    let big = ((u * 3.1 + 0.7).sin() * (v * 2.7 - 0.4).sin()).abs();
    let small = ((u * 7.3 - 1.1).sin() * (v * 6.1 + 0.9).cos()).abs();
    let lace = big * 0.65 + small * 0.35;
    // Each frame takes more: a third gone, then two thirds, then all but the thickest
    // of it.
    let eats = [0.0, 0.30, 0.52, 0.72][frame.min(3) as usize];
    let kept = smoothstep(eats, eats + 0.22, lace);
    whole * kept
}

/// Rafts of foam, for the wake: four of them, so a bandful is not one mark repeated.
///
/// A wake close astern is not a white stripe, which is what it was drawn as and what was
/// reported from the deck. It is *rafts*: patches of bubbles with holes in them, lying
/// flat on the water, drifting apart and dissolving over half a minute. The holes are the
/// whole point — a patch of foam is mostly air and water with gaps all through it, and a
/// mark with no gaps reads as paint however well its rim is drawn.
///
/// So each of the four is a soft disc with a lacy bubble pattern eaten out of it, lacier
/// towards the rim than in the middle, because that is where a raft comes apart first.
/// Four variants and a random turn on each patch is enough that a hundred of them in one
/// band never show their repeat.
pub fn wake_rafts() -> Image {
    const CELL: u32 = 128;
    const GRID: u32 = 2;
    const W: u32 = CELL * GRID;
    let mut data = vec![0u8; (W * W * 4) as usize];
    for y in 0..W {
        for x in 0..W {
            let which = (y / CELL) * GRID + (x / CELL);
            let u = ((x % CELL) as f32 + 0.5) / CELL as f32 * 2.0 - 1.0;
            let v = ((y % CELL) as f32 + 0.5) / CELL as f32 * 2.0 - 1.0;
            let value = raft_mark(u, v, which as f32);
            let i = ((y * W + x) * 4) as usize;
            let byte = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
            data[i] = byte;
            data[i + 1] = byte;
            data[i + 2] = byte;
            data[i + 3] = 255;
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: W,
            height: W,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    build_mipmaps(&mut image, false);
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mipmap_filter: ImageFilterMode::Linear,
        ..ImageSamplerDescriptor::linear()
    });
    image
}

/// One raft: a soft, slightly irregular disc with bubbles eaten out of it.
pub fn raft_mark(u: f32, v: f32, seed: f32) -> f32 {
    let r = (u * u + v * v).sqrt().max(1e-4);
    let angle = v.atan2(u);
    // Not round. Three slow lobes and five quick ones, offset per variant, which is a
    // patch of foam rather than a disc stamped out with a punch.
    let rim = 0.84 + 0.07 * (angle * 3.0 + seed).sin() + 0.04 * (angle * 5.0 - seed * 2.3).sin();
    let body = 1.0 - smoothstep(rim * 0.30, rim, r);
    // The bubbles. Value noise at two scales: the big one makes the holes a raft breaks
    // into, the small one is the texture of the foam itself.
    let n = |scale: f32, offset: f32| -> f32 {
        let (px, py) = (u * scale + offset, v * scale + offset * 1.7);
        let (ix, iy) = (px.floor(), py.floor());
        let (fx, fy) = (px - ix, py - iy);
        let hash = |a: f32, b: f32| {
            let h = (a * 127.1 + b * 311.7 + seed * 41.3).sin() * 43758.547;
            h - h.floor()
        };
        let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
        let a = hash(ix, iy);
        let b = hash(ix + 1.0, iy);
        let c = hash(ix, iy + 1.0);
        let d = hash(ix + 1.0, iy + 1.0);
        (a + (b - a) * sx) + ((c + (d - c) * sx) - (a + (b - a) * sx)) * sy
    };
    let lace = (0.62 * n(5.0, 0.0) + 0.38 * n(13.0, 3.1)).clamp(0.0, 1.0);
    // Open towards the rim, solid in the middle: a raft comes apart from the outside.
    let open = smoothstep(0.10, 0.95, r / rim) * 0.85;
    body * (1.0 - open * smoothstep(0.30, 0.78, 1.0 - lace))
}

/// The tile has to repeat, and the mesh's UVs run to hundreds of tiles across.
fn repeating() -> ImageSampler {
    ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        // Trilinear, so the mip chain is actually used rather than snapping between
        // levels at a visible ring on the water.
        mipmap_filter: ImageFilterMode::Linear,
        // The sea is seen at a grazing angle everywhere, which is precisely the case
        // mipmaps alone handle badly: the right mip for the compressed axis blurs the
        // other one to nothing. Four taps, not the usual sixteen — measured, sixteen
        // cost a dropped frame every couple of seconds and four cost none, and four
        // already covers the compression that occurs in the near and middle distance.
        anisotropy_clamp: 4,
        ..ImageSamplerDescriptor::linear()
    })
}

/// Build the mip chain, down to a single texel.
///
/// Bevy does not generate mipmaps for an image made at runtime, and without them a
/// 512-texel tile seen across a hundred and eighty metres of sea at a grazing angle
/// gives each screen pixel one texel out of a violently varying map. That aliases, and
/// the aliasing crawls as the boat moves — a shimmer with nothing to do with the mesh,
/// which is why fixing the mesh did not cure it.
///
/// The downsample is a plain box filter, and for the normal map that is not only
/// adequate but *correct*: averaging neighbouring normals and renormalising is what a
/// surface does when its ripples drop below one pixel. The mean normal flattens, the
/// distance loses its glitter, and the water goes smooth and dark towards the horizon
/// the way it really does. The fade is a consequence rather than a fudge.
pub fn build_mipmaps(image: &mut Image, renormalise: bool) {
    let mut width = image.texture_descriptor.size.width;
    let mut height = image.texture_descriptor.size.height;
    let mut level: Vec<u8> = image.data.clone().unwrap_or_default();
    let mut chain = level.clone();
    let mut levels = 1;

    while width > 1 || height > 1 {
        let (w, h) = ((width / 2).max(1), (height / 2).max(1));
        let mut next = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let mut sum = [0.0f32; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let sx = (x * 2 + dx).min(width - 1);
                    let sy = (y * 2 + dy).min(height - 1);
                    let i = ((sy * width + sx) * 4) as usize;
                    for c in 0..4 {
                        sum[c] += level[i + c] as f32;
                    }
                }
                if renormalise {
                    // Back to a vector, average, renormalise, and encode again.
                    let v = Vec3::new(
                        sum[0] / 4.0 / 255.0 * 2.0 - 1.0,
                        sum[1] / 4.0 / 255.0 * 2.0 - 1.0,
                        sum[2] / 4.0 / 255.0 * 2.0 - 1.0,
                    )
                    .normalize_or(Vec3::Z);
                    next.push(encode_normal(v.x));
                    next.push(encode_normal(v.y));
                    next.push(encode_normal(v.z));
                    next.push(255);
                } else {
                    for s in sum {
                        next.push((s / 4.0).round().clamp(0.0, 255.0) as u8);
                    }
                }
            }
        }
        chain.extend_from_slice(&next);
        level = next;
        width = w;
        height = h;
        levels += 1;
    }

    image.texture_descriptor.mip_level_count = levels;
    image.data = Some(chain);
}

/// Pack a normal component into a byte. Shared, so the deck and the sea cannot
/// disagree about what a normal map means.
#[inline]
pub fn encode_normal(component: f32) -> u8 {
    (((component * 0.5 + 0.5) * 255.0).clamp(0.0, 255.0)) as u8
}

#[cfg(test)]
mod the_marks {
    use super::*;

    /// **A bubble is a sphere, and the sheet has to say so in one channel.**
    ///
    /// Dark in the middle, because that is where you are looking through it into the
    /// water; bright at the rim, where its surface turns away from you; and one hard
    /// glint high on it, which is the thing that tells the eye it is a sphere and not a
    /// ring. Drawn additively, so the dark middle adds nothing and the sea shows
    /// through — which is the whole reason it can be drawn this cheaply.
    #[test]
    fn a_bubble_is_dark_in_the_middle_and_bright_round_the_edge() {
        let middle = bubble_mark(0.05, 0.05);
        let rim = bubble_mark(0.0, 0.72);
        let glint = bubble_mark(-0.33, 0.36);
        let outside = bubble_mark(1.2, 0.0);
        assert!(
            middle < 0.2,
            "the middle of a bubble came out at {middle:.2}"
        );
        assert!(
            rim > middle * 1.5,
            "the rim is not brighter than the middle"
        );
        assert!(glint > rim, "the glint is not the brightest thing on it");
        assert_eq!(outside, 0.0, "there is nothing outside a bubble");
    }

    /// And foam tears: each frame keeps less of the same patch than the last.
    ///
    /// Not four different patches — four stages of one, which is what makes a flipbook
    /// read as a thing coming apart rather than as a thing being replaced.
    #[test]
    fn foam_comes_apart_frame_by_frame() {
        let mut was = f32::MAX;
        for frame in 0..4u32 {
            // How much of the mark is left, over a grid of it.
            let mut kept = 0.0;
            for i in 0..21 {
                for j in 0..21 {
                    let u = i as f32 / 10.0 - 1.0;
                    let v = j as f32 / 10.0 - 1.0;
                    kept += torn_claw(u, v, frame);
                }
            }
            assert!(
                kept < was,
                "frame {frame} kept {kept:.1} against {was:.1} before it"
            );
            was = kept;
        }
        // And the last frame still has something in it: foam thins out, it does not
        // blink off.
        assert!(was > 0.0);
    }
}

#[cfg(test)]
mod the_sheet {
    use super::*;

    fn sheet() -> (Vec<u8>, usize, usize) {
        let image = spray_marks();
        let size = image.texture_descriptor.size;
        let data = image.data.expect("the sheet has no pixels");
        (data, size.width as usize, size.height as usize)
    }

    /// **Every cell has a mark in it, and the sheet is written all the way down.**
    ///
    /// The fault this is for was a row stride: the sheet grew from two cells to six and
    /// the stride stayed at two cells' width, so every row was written into the first
    /// third of the buffer on top of the row before it. Two thirds of the image was
    /// untouched black and the rest was a solid block with no shape in it — and since
    /// the marks are drawn additively, what that produces is foam you cannot see.
    /// Nothing else caught it: the image was the right size, the right format, had
    /// mipmaps and uploaded cleanly. Only its content was wrong.
    #[test]
    fn each_mark_is_a_mark_and_the_whole_sheet_is_written() {
        let (data, w, h) = sheet();
        let cells = w / h;
        assert_eq!(cells, 6, "six cells across a sheet one cell high");
        let red = |x: usize, y: usize| data[(y * w + x) * 4] as f32 / 255.0;
        // The sheet is written all the way down. These marks are round, so the top and
        // the bottom thirds of the image carry about the same amount of ink; a stride
        // that stops short leaves the bottom exactly empty, which no mark does.
        let band = |from: usize, to: usize| -> f32 {
            (from..to)
                .flat_map(|y| (0..w).map(move |x| (x, y)))
                .map(|(x, y)| red(x, y))
                .sum::<f32>()
                / ((to - from) * w) as f32
        };
        let (top, bottom) = (band(0, h / 3), band(2 * h / 3, h));
        assert!(
            bottom > top * 0.4,
            "the bottom third of the sheet carries {bottom:.3} ink against the top's \
             {top:.3}, which is a row stride that does not reach the bottom"
        );
        for cell in 0..cells {
            let left = cell * h;
            let ink: f32 = (0..h)
                .flat_map(|y| (0..h).map(move |x| (x, y)))
                .map(|(x, y)| red(left + x, y))
                .sum::<f32>()
                / (h * h) as f32;
            assert!(
                (0.002..0.6).contains(&ink),
                "cell {cell} is {:.3} covered, which is either blank or a solid block",
                ink
            );
            // A mark is a mark: its corners are water, not paint.
            for (x, y) in [(2, 2), (h - 3, 2), (2, h - 3), (h - 3, h - 3)] {
                assert!(
                    red(left + x, y) < 0.25,
                    "cell {cell} has paint in its corner, which is a square rather than a mark"
                );
            }
        }
    }

    /// The four frames of a handful of foam coming apart get emptier, which is what
    /// *tearing* means and is the whole reason there are four of them.
    #[test]
    fn the_foam_tears_as_it_goes() {
        let (data, w, h) = sheet();
        let red = |x: usize, y: usize| data[(y * w + x) * 4] as f32 / 255.0;
        let ink = |cell: usize| -> f32 {
            let left = cell * h;
            (0..h)
                .flat_map(|y| (0..h).map(move |x| (x, y)))
                .map(|(x, y)| red(left + x, y))
                .sum::<f32>()
                / (h * h) as f32
        };
        let frames: Vec<f32> = (2..6).map(ink).collect();
        for pair in frames.windows(2) {
            assert!(
                pair[1] < pair[0],
                "a later frame of the tear is fuller than an earlier one: {frames:?}"
            );
        }
    }
}
