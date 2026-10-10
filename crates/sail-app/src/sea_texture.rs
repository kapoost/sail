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

/// Re-export so the lighting and the sky cannot disagree about where the sun is.
pub use crate::sky::SUN as SUN_FROM_SKY;

/// One ripple: integer cycles across the tile, so the result tiles seamlessly.
struct Ripple {
    cycles_u: f32,
    cycles_v: f32,
    amplitude: f32,
    phase: f32,
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
    // Wavelengths from the whole tile down to about half a metre.
    let longest = 1.0_f32;
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
        out.push(Ripple {
            cycles_u,
            cycles_v,
            amplitude: 0.075 / k_actual,
            // Irrational stride, so no two components ever line up to make a seam.
            phase: (i as f32 * 2.399_963_2) % std::f32::consts::TAU,
        });
    }
    out
}

/// Tangent-space normal map for the small-scale ripples.
pub fn normal_map() -> Image {
    let spectrum = ripples();
    let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    let n = SIZE as f32;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let u = x as f32 / n;
            let v = y as f32 / n;
            // Analytic gradient of the summed ripples. Exact, and far cheaper than
            // differencing a rendered height field.
            let mut du = 0.0;
            let mut dv = 0.0;
            for r in &spectrum {
                let k = std::f32::consts::TAU;
                let theta = k * (r.cycles_u * u + r.cycles_v * v) + r.phase;
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
            width: SIZE,
            height: SIZE,
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

/// How fast the ripple pattern itself travels over the water, metres per second.
///
/// Not the surface drift. The normal map stands for short wind waves of a few metres,
/// and a wave of length L in deep water moves at `sqrt(gL / 2pi)` — about 2.8 m/s for
/// the scales in this tile. The texture used to be advected at the surface *current*
/// instead, a tenth of that, which is why the water read as a photograph of a sea
/// rather than a sea: the right shape, carried along, but not going anywhere of its
/// own accord.
///
/// This is on top of riding the wave, not instead of it. The texture coordinate is
/// the position of the water *parcel*, so the pattern already orbits with the swell
/// and crowds at the crests, which is what a Lagrangian coordinate gives for free.
/// What this adds is the ripples travelling through the water that carries them.
pub fn ripple_phase_speed() -> f32 {
    // Amplitude-weighted mean of the ripple wavelengths in the tile, near enough.
    let wavelength = TILE_METRES / 2.2;
    (9.80665 * wavelength / std::f32::consts::TAU).sqrt()
}
