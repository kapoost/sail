//! Procedural deck surfaces: laid planking, and the grooves between the planks.
//!
//! Generated like everything else, and for a reason beyond consistency: a planked
//! deck is a *periodic* thing — a plank width, a seam width, grain along the lay —
//! which is exactly the kind of surface a rule describes better than a photograph.
//! The tile is one metre square and the texture coordinates are in metres, so the
//! planks come out the same width wherever they are laid and the tile is seamless by
//! construction rather than by retouching.
//!
//! What the deck had before this was a single flat colour across the deck, the cabin
//! sides, the cabin top and the cockpit. That is why she read as plastic: a real deck
//! is the most strongly patterned surface on a boat, and the pattern is the first
//! thing the eye uses to judge the size of what it is looking at.

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// Texels across the one-metre tile. Two millimetres each, which is a quarter of the
/// width of a caulking seam and far finer than anyone standing on it can see.
///
/// It was a thousand and twenty-four, half a millimetre a texel, which is a detail no
/// eye resolves and which cost a dropped frame every four seconds in bandwidth.
const SIZE: u32 = 512;
/// Planks per metre. Sixteen gives a 62 mm plank, which is a yacht deck rather than a
/// ship's. An integer, so the pattern meets itself at the tile edge.
const PLANKS: u32 = 16;
/// Width of the caulking seam as a fraction of a plank.
const SEAM: f32 = 0.09;

/// A cheap deterministic hash, for grain that is different on every plank.
fn noise(x: u32, y: u32) -> f32 {
    let mut h = x.wrapping_mul(0x27d4_eb2d) ^ y.wrapping_mul(0x1656_67b1);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2545_f491);
    h ^= h >> 13;
    (h & 0xffff) as f32 / 65535.0
}

/// Smooth value noise, for grain that runs along the plank rather than fizzing.
fn grain(u: f32, v: f32, scale_u: f32, scale_v: f32) -> f32 {
    let (x, y) = (u * scale_u, v * scale_v);
    let (xi, yi) = (x.floor(), y.floor());
    let (xf, yf) = (x - xi, y - yi);
    let ease = |t: f32| t * t * (3.0 - 2.0 * t);
    let (sx, sy) = (ease(xf), ease(yf));
    let w = |i: f32, j: f32| {
        noise(
            (i as i32).rem_euclid(scale_u as i32).max(0) as u32,
            (j as i32).rem_euclid(scale_v as i32).max(0) as u32,
        )
    };
    let a = w(xi, yi) + (w(xi + 1.0, yi) - w(xi, yi)) * sx;
    let b = w(xi, yi + 1.0) + (w(xi + 1.0, yi + 1.0) - w(xi, yi + 1.0)) * sx;
    a + (b - a) * sy
}

/// Which plank a point is on, and how far across it: `(index, 0..1)`.
fn plank_at(u: f32) -> (u32, f32) {
    let s = u * PLANKS as f32;
    (s.floor() as u32 % PLANKS, s.fract())
}

/// Colour of the laid deck.
pub fn planking() -> Image {
    let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let u = x as f32 / SIZE as f32;
            let v = y as f32 / SIZE as f32;
            let (plank, across) = plank_at(u);

            // Each plank gets its own tone, so the lay reads as boards rather than
            // as stripes drawn on one board.
            let tone = 0.86 + 0.14 * noise(plank, 7);
            // Grain runs along the plank: stretched hard in v, tight in u.
            let g = grain(u, v, PLANKS as f32 * 3.0, 24.0);
            let weathered = grain(u, v, 5.0, 5.0);

            let mut shade = tone * (0.92 + 0.08 * g) * (0.90 + 0.14 * weathered);

            // Caulking: a dark seam each side of the plank, with the edges of the
            // plank itself darkened where it has worn against it.
            let edge = across.min(1.0 - across);
            if edge < SEAM {
                shade *= 0.17;
            } else if edge < SEAM * 2.0 {
                shade *= 0.80 + 0.20 * ((edge - SEAM) / SEAM);
            }

            let base = Vec3::new(0.74, 0.66, 0.52) * shade;
            data.push((base.x.clamp(0.0, 1.0) * 255.0) as u8);
            data.push((base.y.clamp(0.0, 1.0) * 255.0) as u8);
            data.push((base.z.clamp(0.0, 1.0) * 255.0) as u8);
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
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    crate::sea_texture::build_mipmaps(&mut image, false);
    image.sampler = repeating();
    image
}

/// The seams as grooves, so the deck catches light across the lay.
pub fn planking_normal() -> Image {
    let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let u = x as f32 / SIZE as f32;
            let v = y as f32 / SIZE as f32;
            let (_, across) = plank_at(u);

            // The groove: a slope down into the seam from each side, which is what
            // makes a seam visible at a grazing angle instead of being a dark line.
            let edge = across.min(1.0 - across);
            let du = if edge < SEAM {
                let into = (SEAM - edge) / SEAM;
                if across < 0.5 {
                    into * 0.9
                } else {
                    -into * 0.9
                }
            } else {
                0.0
            };
            // A little grain relief, so the boards are not mirror flat.
            let dv = (grain(u, v, PLANKS as f32 * 3.0, 24.0) - 0.5) * 0.10;

            let n = Vec3::new(-du, -dv, 1.0).normalize();
            data.push(crate::sea_texture::encode_normal(n.x));
            data.push(crate::sea_texture::encode_normal(n.y));
            data.push(crate::sea_texture::encode_normal(n.z));
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
    crate::sea_texture::build_mipmaps(&mut image, true);
    image.sampler = repeating();
    image
}

fn repeating() -> ImageSampler {
    ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 4,
        ..ImageSamplerDescriptor::linear()
    })
}
