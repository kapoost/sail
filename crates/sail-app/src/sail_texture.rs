//! Sailcloth: panels, seams, weave and the creases that radiate from the corners.
//!
//! The sails are the largest surface on the boat by a wide margin and they were a flat
//! colour, which is why they read as plastic sheet rather than cloth. Everything that
//! makes a sail look like a sail is structure at a scale you can see from the cockpit:
//!
//! * **Panels.** A sail is not one piece of cloth. It is strips a metre wide, and
//!   where two strips are joined the cloth is doubled and stitched, so the seam stands
//!   proud and catches the light differently from the panel. A crosscut main has them
//!   running square to the leech; this generates them square to the foot, which is the
//!   same thing to within the taper and is what the texture coordinates give for free.
//! * **Weave.** Close to, sailcloth is a visible warp and weft. Far off it is what
//!   stops the surface being mirror-smooth, which matters because a smooth white
//!   surface in sunlight is a blown-out blank.
//! * **Creases.** Cloth under load pulls into lines from the corners that carry it.
//!   Those lines are where a trimmer looks: the ones from the clew tell you about the
//!   foot and the ones from the head about the leech.
//!
//! The texture coordinates are in metres, so a panel is a metre wherever it is — on
//! the main, on the headsail, and on a reefed main that has had a third of itself
//! rolled away.

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// Texels across the tile. Two millimetres each, which resolves a stitch line.
const SIZE: u32 = 512;
/// The tile is one panel wide and one panel tall.
pub const PANEL_METRES: f32 = 1.0;
/// Width of the stitched seam as a fraction of a panel.
const SEAM: f32 = 0.035;
/// Threads per panel, each way.
const WEAVE: f32 = 110.0;

/// Colour of the cloth.
pub fn cloth() -> Image {
    let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let u = x as f32 / SIZE as f32;
            let v = y as f32 / SIZE as f32;

            // The weave: two crossed combs, which is what a plain weave is.
            let warp = (u * WEAVE * std::f32::consts::TAU).sin();
            let weft = (v * WEAVE * std::f32::consts::TAU).sin();
            let weave = 1.0 + 0.030 * warp * weft;

            // Age. Not dirt — a used sail is faintly unevenly bleached, and a sail
            // that is perfectly even reads as new in a way nothing else on this boat
            // is.
            let age = 1.0 - 0.035 * ((u * 5.3).sin() * (v * 3.7).cos()).abs();

            // The seam runs along the bottom edge of the tile, so panels stack in v.
            let from_seam = v.min(1.0 - v);
            let seam = if from_seam < SEAM {
                // Doubled cloth: slightly darker because it is twice as thick, with
                // the stitching a shade darker again.
                let stitch = ((u * 90.0).sin() * 0.5 + 0.5).powf(6.0);
                0.93 - 0.06 * stitch
            } else {
                1.0
            };

            let shade = weave * age * seam;
            let base = Vec3::new(0.93, 0.925, 0.895) * shade;
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

/// The relief: the seam standing proud, and the weave.
pub fn cloth_normal() -> Image {
    let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let u = x as f32 / SIZE as f32;
            let v = y as f32 / SIZE as f32;

            let k = WEAVE * std::f32::consts::TAU;
            let du = 0.012 * (u * k).cos();
            let mut dv = 0.012 * (v * k).cos();

            // A seam is a ridge, not a groove: two layers of cloth lapped and sewn.
            // Getting that backwards is easy and reads immediately as a crease where
            // there should be a welt.
            let from_seam = v.min(1.0 - v);
            if from_seam < SEAM {
                let across = from_seam / SEAM;
                let slope = (1.0 - across) * 0.55;
                dv += if v < 0.5 { -slope } else { slope };
            }

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
