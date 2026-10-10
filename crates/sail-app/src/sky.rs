//! A procedural sky, and the thing it is really for: giving the water something to
//! reflect.
//!
//! The water had `metallic: 1.0` and a reflectance, which sounds like it should look
//! like water and cannot. A mirror in an empty room is black. With no environment map
//! in the scene there was nothing in the reflection at all, so the surface was lit
//! only by the sun and a flat ambient term — which is why it read as painted plastic
//! however good the geometry underneath it was.
//!
//! One cubemap does both jobs here: it is drawn as the sky, and it is filtered on the
//! GPU into the environment light that the water reflects. Because it is the same
//! image, the sun you can see and the sun glittering off the wave faces are the same
//! sun. Letting those two drift apart is a tell that no amount of shader work hides.

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
};

/// Face resolution. Must be a power of two for the GPU filtering that turns this into
/// an environment light.
const FACE: u32 = 256;

/// Direction *towards* the sun, which is the opposite of the way its light travels.
///
/// Defined once, here, so the drawn sun and the lighting cannot disagree. Low in the
/// west: a low sun puts a long glitter path across the water, which is both the most
/// flattering light for a sea and the most useful, since the path shows you the wave
/// shape all the way to the horizon.
pub const SUN: Vec3 = Vec3::new(-0.62, 0.30, -0.72);

/// Zenith and horizon sky colours, in relative units.
const ZENITH: Vec3 = Vec3::new(0.14, 0.30, 0.62);
pub const HORIZON: Vec3 = Vec3::new(0.62, 0.72, 0.84);
/// What the lower hemisphere contributes. Never seen directly — the sea covers it —
/// but the water's downward-facing reflections sample it, so it has to be a plausible
/// sea rather than black.
const BELOW: Vec3 = Vec3::new(0.055, 0.105, 0.135);

/// Relative brightness of the sun's disc. Large, because this is what makes the
/// glitter path: every wave facet that happens to line up flashes.
const SUN_DISC: f32 = 2600.0;

/// Build the sky cubemap.
pub fn cubemap() -> Image {
    let sun = SUN.normalize();
    let n = FACE as f32;
    // Six faces stacked vertically, then reinterpreted as array layers.
    let mut data: Vec<u8> = Vec::with_capacity((FACE * FACE * 6 * 16) as usize);

    for face in 0..6u32 {
        for y in 0..FACE {
            for x in 0..FACE {
                // Texel centres, mapped to [-1, 1].
                let u = (x as f32 + 0.5) / n * 2.0 - 1.0;
                let v = (y as f32 + 0.5) / n * 2.0 - 1.0;
                let dir = face_direction(face, u, v).normalize();
                let colour = sky_colour(dir, sun);
                for c in [colour.x, colour.y, colour.z, 1.0] {
                    data.extend_from_slice(&c.to_le_bytes());
                }
            }
        }
    }

    let mut image = Image::new(
        Extent3d {
            width: FACE,
            height: FACE * 6,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba32Float,
        RenderAssetUsages::RENDER_WORLD,
    );
    image
        .reinterpret_stacked_2d_as_array(6)
        .expect("six equal faces stacked vertically");
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    image
}

/// Standard cubemap face orientations, in wgpu's order: +X, -X, +Y, -Y, +Z, -Z.
fn face_direction(face: u32, u: f32, v: f32) -> Vec3 {
    match face {
        0 => Vec3::new(1.0, -v, -u),
        1 => Vec3::new(-1.0, -v, u),
        2 => Vec3::new(u, 1.0, v),
        3 => Vec3::new(u, -1.0, -v),
        4 => Vec3::new(u, -v, 1.0),
        _ => Vec3::new(-u, -v, -1.0),
    }
}

/// Sky radiance in a direction, relative units.
fn sky_colour(dir: Vec3, sun: Vec3) -> Vec3 {
    let up = dir.y;
    let sky = if up >= 0.0 {
        // Pale at the horizon, deep overhead. The exponent is doing the work of
        // atmospheric depth: most of the gradient happens in the first few degrees.
        ZENITH.lerp(HORIZON, (1.0 - up).powf(3.2))
    } else {
        // Fade into the sea colour just below the horizon rather than cutting, or the
        // reflection picks up a hard edge where the two meet.
        let t = (-up / 0.08).clamp(0.0, 1.0);
        HORIZON.lerp(BELOW, t)
    };

    let alignment = dir.dot(sun).max(0.0);
    // Disc, then a tight corona, then the broad forward-scattered haze that makes the
    // whole sky brighter near the sun.
    let disc = if alignment > 0.99975 { SUN_DISC } else { 0.0 };
    let corona = alignment.powf(1800.0) * 180.0;
    let haze = alignment.powf(5.0) * 0.45;

    let warm = Vec3::new(1.0, 0.94, 0.82);
    sky + warm * (disc + corona) + warm * haze
}

/// The colour the haze has to be.
///
/// Derived from the sky's own horizon rather than picked separately. If the two
/// differ, the sea fades towards one colour while the sky above it is another, and the
/// join shows as a band exactly where the eye is looking hardest.
pub fn haze_colour() -> Color {
    Color::linear_rgb(HORIZON.x, HORIZON.y, HORIZON.z)
}

/// Slightly deeper, for the near end of the fog gradient.
pub fn haze_near() -> Color {
    let c = HORIZON * 0.88;
    Color::linear_rgb(c.x, c.y, c.z)
}
