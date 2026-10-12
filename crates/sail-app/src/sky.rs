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
///
/// Northern. The zenith goes deeper and colder and the horizon keeps a trace of the low
/// sun in it, which is the light a high latitude has for most of the sailing season: the
/// sun never gets far up, so the sky above you stays a hard blue while everything near
/// the water is washed pale and faintly warm.
pub const ZENITH: Vec3 = Vec3::new(0.10, 0.24, 0.58);
pub const HORIZON: Vec3 = Vec3::new(0.70, 0.76, 0.84);
/// What the lower hemisphere contributes. Never seen directly — the sea covers it —
/// but the water's downward-facing reflections sample it, so it has to be a plausible
/// sea rather than black.
const BELOW: Vec3 = Vec3::new(0.055, 0.105, 0.135);

/// Relative brightness of the sun's disc. Large, because this is what makes the
/// glitter path: every wave facet that happens to line up flashes.
const SUN_DISC: f32 = 2600.0;

/// Height of the lower cloud deck above the sea, metres, and of the upper one.
///
/// Two decks because one is a wallpaper. What makes a northern sky worth looking at is
/// depth — a low, ragged, fast layer with a towering slower one behind it — and two
/// planes at different heights give that for nothing: the lower deck's perspective
/// crowds towards the horizon much faster than the upper one's, so they slide past each
/// other exactly as real layers do.
const CLOUD_LOW: f32 = 900.0;
const CLOUD_HIGH: f32 = 3400.0;

/// Size of the cloud pattern on each deck, metres.
const SCALE_LOW: f32 = 1700.0;
const SCALE_HIGH: f32 = 5200.0;

/// How much of the sky each deck covers.
///
/// The low one is broken and the high one nearly solid, which is the ordinary northern
/// arrangement: squalls and rags running under a high sheet. Both raised for weather —
/// this is a sky with something in it rather than a fine afternoon.
const COVER_LOW: f32 = 0.55;
const COVER_HIGH: f32 = 0.46;

/// The storm cells: a third deck, sparse and enormous, standing above the other two.
///
/// A squall is not more cloud, it is a different *shape* of cloud — one tower every few
/// miles, with a flat dark base and ten thousand feet of white above it. So it gets its
/// own deck at its own height, covering very little of the sky, and the drama comes from
/// the contrast between the few cells and the gaps between them rather than from
/// filling everything in.
const CLOUD_STORM: f32 = 6200.0;
const SCALE_STORM: f32 = 9000.0;
const COVER_STORM: f32 = 0.26;

/// A hash-based value noise, so the sky is the same sky every time without a texture
/// file to ship or a seed to lose.
fn hash2(x: i32, y: i32) -> f32 {
    let mut h = (x as i64).wrapping_mul(0x27d4_eb2d) ^ (y as i64).wrapping_mul(0x1656_67b1);
    h ^= h >> 29;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd_u64 as i64);
    h ^= h >> 32;
    ((h & 0xffff) as f32) / 65535.0
}

fn value_noise(p: Vec2) -> f32 {
    let i = p.floor();
    let f = p - i;
    // Smoothstep weights: the cheapest interpolation that has no visible grid in it.
    let w = f * f * (Vec2::splat(3.0) - 2.0 * f);
    let (x, y) = (i.x as i32, i.y as i32);
    let a = hash2(x, y);
    let b = hash2(x + 1, y);
    let c = hash2(x, y + 1);
    let d = hash2(x + 1, y + 1);
    (a + (b - a) * w.x) + ((c + (d - c) * w.x) - (a + (b - a) * w.x)) * w.y
}

/// Four octaves, which is enough for cloud and cheap enough for four hundred thousand
/// texels at startup.
fn fbm(p: Vec2) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut at = p;
    for _ in 0..4 {
        sum += value_noise(at) * amp;
        at *= 2.03;
        amp *= 0.5;
    }
    sum
}

/// What a cloud deck contributes in a given direction: how much of it there is, and how
/// lit it is.
///
/// The deck is a plane at a height, so the direction's intersection with it gives the
/// point to sample — which is also where the perspective comes from for free. Looking
/// up you cut the plane close overhead and the pattern is large; looking towards the
/// horizon you cut it miles away and it compresses into a band, which is what a cloudy
/// sky does and the one thing a cube of noise cannot do.
fn deck(dir: Vec3, sun: Vec3, height: f32, scale: f32, cover: f32, drift: f32) -> (f32, f32) {
    if dir.y <= 0.02 {
        return (0.0, 0.0);
    }
    let reach = height / dir.y;
    // Beyond about forty kilometres the deck is below the horizon as far as the eye is
    // concerned, and sampling it there only makes a hard line.
    let fade = 1.0 - ((reach - 26_000.0) / 16_000.0).clamp(0.0, 1.0);
    if fade <= 0.0 {
        return (0.0, 0.0);
    }
    let at = Vec2::new(dir.x, dir.z) * reach / scale + Vec2::splat(drift);
    let n = fbm(at);
    // Coverage as a threshold with a soft shoulder: hard enough to have edges, soft
    // enough that the edges are cloth rather than paper.
    let density = ((n - (1.0 - cover)) / 0.22).clamp(0.0, 1.0) * fade;
    // Lighting, and the trick that makes drawn clouds read: a cloud is lit on the side
    // facing the sun and the *shape* of that lighting comes from the density field's
    // own slope. Sampling the field a little way towards the sun and comparing is a
    // gradient in the direction of the light, which costs one more lookup and gives the
    // bright rims and dark flanks a painter would put there.
    let towards = Vec2::new(sun.x, sun.z).normalize_or_zero() * 0.35;
    let ahead = fbm(at + towards);
    let lit = ((n - ahead) * 3.0 + 0.5).clamp(0.0, 1.0);
    (density, lit)
}

/// Shafts of light between the clouds, which cost nothing because they are baked.
///
/// Crepuscular rays are the sun's light picked out by haze where cloud is not blocking
/// it, so the quantity that draws them is already here: step from a direction towards
/// the sun, ask the cloud decks how much is in the way, and what comes back is how much
/// of a shaft there is. Eight samples, once per texel, at startup — against doing it in
/// a shader every frame for every pixel, which is the usual way and the reason most
/// games have no rays at all.
///
/// They only show near the sun and only where the air between is thick with haze, so
/// the result is multiplied by both: a tight angular falloff and the same forward
/// scattering the sky already uses.
fn shafts(dir: Vec3, sun: Vec3) -> f32 {
    let alignment = dir.dot(sun);
    if alignment <= 0.55 || dir.y <= 0.0 {
        return 0.0;
    }
    const STEPS: usize = 8;
    let mut clear = 0.0;
    for i in 1..=STEPS {
        let f = i as f32 / STEPS as f32;
        let at = dir.lerp(sun, f * 0.85).normalize();
        let (low, _) = deck(at, sun, CLOUD_LOW, SCALE_LOW, COVER_LOW, 3.0);
        let (storm, _) = deck(at, sun, CLOUD_STORM, SCALE_STORM, COVER_STORM, 23.0);
        clear += (1.0 - (low + storm).min(1.0)) / STEPS as f32;
    }
    // Sharpened: a shaft is a shaft because it has an edge. And faded out towards the
    // zenith, since a ray you are looking along is a ray you cannot see.
    let reach = ((alignment - 0.55) / 0.45).clamp(0.0, 1.0).powf(1.6);
    clear.powf(2.2) * reach * (1.0 - dir.y).clamp(0.0, 1.0)
}

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

    // Two cloud decks, the high one laid in first so the low one runs in front of it.
    let (storm_d, storm_l) = deck(dir, sun, CLOUD_STORM, SCALE_STORM, COVER_STORM, 23.0);
    let (high_d, high_l) = deck(dir, sun, CLOUD_HIGH, SCALE_HIGH, COVER_HIGH, 11.0);
    let (low_d, low_l) = deck(dir, sun, CLOUD_LOW, SCALE_LOW, COVER_LOW, 3.0);
    // How high up this direction is looking, which stands in for how far up a cell it
    // is seeing. A squall is dark underneath and brilliant on top, and that is one
    // cloud, not two: the base is in its own shadow and the anvil is in the sun.
    let towering = ((dir.y - 0.10) / 0.55).clamp(0.0, 1.0);
    // Cloud colours: a cold shadow and a warm sunlit face, because the only light on
    // them is a low sun and everything it does not reach is lit by the sky instead.
    // Storm colours. The shadowed side of a squall is nearly black with a blue cast in
    // it — far darker than ordinary cloud, which is what makes the lit parts look as
    // bright as they do — and the sunlit face is close to white, because a cumulonimbus
    // is thick enough to scatter everything back out at you.
    let shadow = Vec3::new(0.10, 0.13, 0.19);
    let sunlit = Vec3::new(1.25, 1.18, 1.08);
    let sky = {
        let storm = shadow.lerp(sunlit, (storm_l * 0.4 + towering * 0.75).clamp(0.0, 1.0));
        let high = shadow.lerp(sunlit, (high_l * 0.7 + towering * 0.4).clamp(0.0, 1.0));
        // The low deck is the one that stays dark: it is under everything else and only
        // its edges catch the sun. This is what makes a squall line read as a squall
        // line rather than as grey weather.
        let low = shadow.lerp(sunlit, (low_l * 0.9 + towering * 0.15).clamp(0.0, 1.0) * 0.8);
        let with_storm = sky.lerp(storm, storm_d * 0.95);
        let with_high = with_storm.lerp(high, high_d * 0.8);
        with_high.lerp(low, low_d * 0.92)
    };

    let alignment = dir.dot(sun).max(0.0);
    // Disc, then a tight corona, then the broad forward-scattered haze that makes the
    // whole sky brighter near the sun.
    // The sun is behind whatever cloud is in front of it, which is the other half of
    // why a northern sky has any drama in it: the disc comes and goes.
    let veiled = (1.0 - low_d * 0.95) * (1.0 - high_d * 0.6) * (1.0 - storm_d * 0.98);
    let disc = if alignment > 0.99975 {
        SUN_DISC * veiled
    } else {
        0.0
    };
    let corona = alignment.powf(1800.0) * 180.0 * veiled;
    let haze = alignment.powf(5.0) * 0.45;

    let warm = Vec3::new(1.0, 0.94, 0.82);
    // And the shafts, which are the haze lit along a clear line to the sun. Added rather
    // than mixed, because that is what light does to air.
    let rays = shafts(dir, sun) * 0.5;
    sky + warm * (disc + corona) + warm * haze + warm * rays
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
