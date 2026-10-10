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
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// Texture resolution. 512 across a six-metre tile is about a centimetre a texel,
/// which is finer than anything the eye resolves at sailing distances.
const SIZE: u32 = 512;
/// How much world the tile covers, metres. Sets the size of the ripples.
/// Eleven metres rather than six. At six the repeat was plainly visible across the
/// middle distance — one tile is about two boat beams, so the eye had dozens of copies
/// of the same patch in view at once. Breaking a tile properly needs detail at two
/// scales and therefore two texture lookups, which means a shader; enlarging it is the
/// honest half-measure until then.
pub const TILE_METRES: f32 = 11.0;

/// Re-export so the lighting and the sky cannot disagree about where the sun is.
pub use crate::sky::SUN as SUN_FROM_SKY;

/// One ripple: integer cycles across the tile, so the result tiles seamlessly.
struct Ripple {
    cycles_u: f32,
    cycles_v: f32,
    amplitude: f32,
    phase: f32,
}

/// Short wind waves and capillary ripples, fanned across directions and scales.
/// Wavelengths run from the full six-metre tile down to about thirty centimetres.
const RIPPLES: &[Ripple] = &[
    Ripple {
        cycles_u: 1.0,
        cycles_v: 2.0,
        amplitude: 0.085,
        phase: 0.0,
    },
    Ripple {
        cycles_u: 3.0,
        cycles_v: 1.0,
        amplitude: 0.060,
        phase: 1.7,
    },
    Ripple {
        cycles_u: 2.0,
        cycles_v: -4.0,
        amplitude: 0.045,
        phase: 0.6,
    },
    Ripple {
        cycles_u: 5.0,
        cycles_v: 3.0,
        amplitude: 0.030,
        phase: 2.9,
    },
    Ripple {
        cycles_u: -4.0,
        cycles_v: 7.0,
        amplitude: 0.022,
        phase: 4.1,
    },
    Ripple {
        cycles_u: 9.0,
        cycles_v: -2.0,
        amplitude: 0.016,
        phase: 5.3,
    },
    Ripple {
        cycles_u: 7.0,
        cycles_v: 11.0,
        amplitude: 0.011,
        phase: 3.3,
    },
    Ripple {
        cycles_u: 14.0,
        cycles_v: 5.0,
        amplitude: 0.007,
        phase: 0.9,
    },
];

/// Tangent-space normal map for the small-scale ripples.
pub fn normal_map() -> Image {
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
            for r in RIPPLES {
                let k = std::f32::consts::TAU;
                let theta = k * (r.cycles_u * u + r.cycles_v * v) + r.phase;
                let c = theta.cos() * r.amplitude * k;
                du += c * r.cycles_u;
                dv += c * r.cycles_v;
            }
            // Tangent space has z out of the surface.
            let normal = Vec3::new(-du, -dv, 1.0).normalize();
            data.push(encode(normal.x));
            data.push(encode(normal.y));
            data.push(encode(normal.z));
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
    image.sampler = repeating();
    image
}

/// The tile has to repeat, and the mesh's UVs run to hundreds of tiles across.
fn repeating() -> ImageSampler {
    ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    })
}

#[inline]
fn encode(component: f32) -> u8 {
    (((component * 0.5 + 0.5) * 255.0).clamp(0.0, 255.0)) as u8
}
