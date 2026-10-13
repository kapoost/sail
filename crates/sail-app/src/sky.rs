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
/// Where the sun is and how much cloud there is, right now.
///
/// This was two constants, and they had to stop being constants for the same reason:
/// a passage is hours long and the sky is the one thing in the frame that was not
/// allowed to change over them. The sun was welded to a single bearing and elevation,
/// and the cloud cover was welded into the cubemap at startup, so a front could arrive
/// in the wind and in the sea and the sky would not notice.
///
/// One definition, read by the cubemap, the directional light, the cloud layer, the
/// cloud shadows, the shafts and the sea's own glints. The whole point is that they
/// cannot disagree about where the sun is.
#[derive(bevy::prelude::Resource, Clone, Copy, Debug)]
pub struct Sky {
    /// Unit vector towards the sun.
    pub sun: Vec3,
    /// Cover of the low deck, 0 clear to 1 solid.
    pub cover: f32,
    /// Local time, hours since midnight.
    pub hour: f32,
}

/// Latitude the sun is worked out for, degrees. Nordic, which is the whole reason the
/// light looks like this: at fifty-seven degrees the sun never gets high enough in the
/// morning or the evening to flatten anything, and that long low light is most of what
/// the sky is doing.
const LATITUDE: f32 = 57.0;
/// Solar declination, degrees. Fixed for the season rather than worked out from a date,
/// because there is no date in this game. Five degrees is early spring — the Irish
/// spring the islands were drawn for.
const DECLINATION: f32 = 5.0;

impl Sky {
    /// The sky at a given local hour, under a given cover.
    ///
    /// Real solar geometry, which costs two sines and is the difference between a sun
    /// that moves and a sun that is dragged about. The elevation falls out of the hour
    /// angle and the azimuth out of the elevation, so morning light comes from the east
    /// and climbs, which is the behaviour and not a number anyone chose.
    pub fn at(hour: f32, cover: f32) -> Self {
        let lat = LATITUDE.to_radians();
        let dec = DECLINATION.to_radians();
        // Hour angle: zero at local noon, fifteen degrees an hour either side.
        let h = ((hour - 12.0) * 15.0).to_radians();
        let sin_el = lat.sin() * dec.sin() + lat.cos() * dec.cos() * h.cos();
        let elevation = sin_el.clamp(-1.0, 1.0).asin();
        // Azimuth from north, measured the way a compass does. The arc cosine only
        // gives the half of the circle between north and south, so the hour angle picks
        // the side: a negative hour angle is morning and the sun is to the *east*, which
        // is a plus sign and was a minus for one run — a ten o'clock sun in the
        // south-west, which is the sort of thing that looks merely odd until you notice
        // every shadow on the boat is pointing the wrong way.
        let cos_az = (dec.sin() - sin_el * lat.sin()) / (lat.cos() * elevation.cos()).max(1e-4);
        let azimuth = cos_az.clamp(-1.0, 1.0).acos() * if h < 0.0 { 1.0 } else { -1.0 };
        // Into the world: Bevy's −Z is north, so a compass bearing becomes
        // (sin, _, −cos) with the elevation lifting it out of the plane.
        let (ce, se) = (elevation.cos(), elevation.sin());
        Self {
            sun: Vec3::new(ce * azimuth.sin(), se, -ce * azimuth.cos()).normalize(),
            cover: cover.clamp(0.0, 1.0),
            hour,
        }
    }

    /// How high the sun is, radians. Negative is below the horizon.
    pub fn elevation(&self) -> f32 {
        self.sun.y.clamp(-1.0, 1.0).asin()
    }

    /// Colour of the direct sunlight. Low sun is warm because its light has come a long
    /// way through the atmosphere and the blue end has been scattered out of it — the
    /// same physics that makes the sky blue, seen from the other end.
    pub fn sunlight(&self) -> Vec3 {
        let low = (1.0 - (self.elevation() / 0.52).clamp(0.0, 1.0)).powf(1.4);
        Vec3::new(1.0, 1.0, 1.0).lerp(Vec3::new(1.0, 0.76, 0.55), low)
    }

    /// The shadowed side and the sunlit face of cloud, for the weather there is.
    ///
    /// These were two constants, and constants are wrong for the same reason a constant
    /// sun was: the thing that decides a cloud's colour is *how much direct sunlight
    /// reaches it*, and that is exactly what the cover and the sun's height say.
    ///
    /// * **Cover closes the contrast.** The bright top and the dark base of a cumulus
    ///   are the same cloud lit from one side; take the direct sun away and there is no
    ///   side to be lit, so a solid overcast is one flat tone from edge to edge. Under
    ///   eight oktas both ends of the range converge on it, which is why an overcast sky
    ///   has no shape in it and a broken one is all shape.
    /// * **A low sun gilds the tops**, through the same reddening that warms everything
    ///   else: the light has come a long way through the atmosphere and the blue end has
    ///   been scattered out of it. The sunlit face reads orange at six in the morning
    ///   and white at noon, and it was reading warm-white at both.
    /// * **And the flat tone follows the sun's height**, because a diffuse sky is only as
    ///   bright as the light getting into the top of it.
    pub fn cloud_tones(&self) -> (Vec3, Vec3) {
        let direct = (1.0 - self.cover).clamp(0.0, 1.0);
        let height = (self.elevation() / 0.52).clamp(0.0, 1.0);
        // What a sky with no direct sun in it is: one diffuse tone, and that is the whole
        // of an overcast.
        let flat = Vec3::new(0.26, 0.29, 0.34) * (0.55 + 0.75 * height);
        let diffuse = 1.0 - direct;
        (
            CLOUD_SHADOW.lerp(flat, diffuse),
            (CLOUD_SUNLIT * self.sunlight()).lerp(flat, diffuse),
        )
    }

    /// And how strong it is: thinner through cloud, and weaker with a low sun for the
    /// same reason it is redder.
    ///
    /// **Bounded, and the bound is the camera's.** Real direct sunlight runs from a
    /// hundred thousand lux in the clear to ten under a heavy overcast — ten to one, more
    /// than three stops — and an eye walks that range without noticing because it adapts.
    /// This camera cannot: `Exposure::SUNLIGHT` is a fixed setting, chosen once for a
    /// sunny day. The first version of this function was physically respectable and
    /// unusable for exactly that reason; on a six-okta morning it put 33 000 lux into a
    /// scene exposed for 85 000, and the whole picture came out flat and grey with the
    /// sails reading as sheets of cardboard.
    ///
    /// So the direct light moves over about one stop either side of what the scene was
    /// built for, and the *diffuse* light carries the rest — see [`Sky::ambient`]. Which
    /// is not a fudge but the half of the physics that was missing: cloud does not
    /// simply remove light, it converts it from a point source into a dome. Losing the
    /// shadows and keeping the brightness is what an overcast actually looks like.
    pub fn illuminance(&self) -> f32 {
        let height = (self.elevation() / 0.52).clamp(0.0, 1.0);
        let clear = 91_800.0 + 40_000.0 * height;
        clear * (1.0 - 0.33 * self.cover)
    }

    /// The sky's own light, which is the other half of it.
    ///
    /// Rises with cover, because that is where the direct light has gone: an overcast is
    /// the whole dome lit evenly, and it is why a cloudy day has no shadows in it rather
    /// than no light. Holding this constant while the sun was dimmed is what made cloudy
    /// weather read as dusk.
    pub fn ambient(&self) -> f32 {
        300.0 + 700.0 * self.cover
    }
}

impl Default for Sky {
    fn default() -> Self {
        // The morning this was all drawn against: a low sun and a broken sky.
        Self::at(8.0, 0.55)
    }
}

/// Zenith and horizon sky colours, in relative units.
///
/// Northern. The zenith goes deeper and colder and the horizon keeps a trace of the low
/// sun in it, which is the light a high latitude has for most of the sailing season: the
/// sun never gets far up, so the sky above you stays a hard blue while everything near
/// the water is washed pale and faintly warm.
pub const ZENITH: Vec3 = Vec3::new(0.10, 0.24, 0.58);
/// The shadowed side of cloud, and the sunlit face.
///
/// Public because the particle cloud in `clouds.rs` is the same weather as the decks
/// baked into this cubemap, and two palettes for one sky is how you get a layer of
/// puffs that visibly does not belong to the sky behind it.
pub const CLOUD_SHADOW: Vec3 = Vec3::new(0.10, 0.13, 0.19);
/// The sunlit face. Over one, because a cloud top in a low sun is brighter than white
/// paper and the tone map is what brings it back.
pub const CLOUD_SUNLIT: Vec3 = Vec3::new(1.25, 1.18, 1.08);
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
fn shafts(dir: Vec3, sun: Vec3, cover: f32) -> f32 {
    let alignment = dir.dot(sun);
    if alignment <= 0.55 || dir.y <= 0.0 {
        return 0.0;
    }
    const STEPS: usize = 8;
    let mut clear = 0.0;
    for i in 1..=STEPS {
        let f = i as f32 / STEPS as f32;
        let at = dir.lerp(sun, f * 0.85).normalize();
        let (low, _) = deck(at, sun, CLOUD_LOW, SCALE_LOW, cover, 3.0);
        let (storm, _) = deck(
            at,
            sun,
            CLOUD_STORM,
            SCALE_STORM,
            COVER_STORM * cover / COVER_LOW,
            23.0,
        );
        clear += (1.0 - (low + storm).min(1.0)) / STEPS as f32;
    }
    // Sharpened: a shaft is a shaft because it has an edge. And faded out towards the
    // zenith, since a ray you are looking along is a ray you cannot see.
    let reach = ((alignment - 0.55) / 0.45).clamp(0.0, 1.0).powf(1.6);
    clear.powf(2.2) * reach * (1.0 - dir.y).clamp(0.0, 1.0)
}

/// Build the sky cubemap.
pub fn cubemap(sky: &Sky) -> Image {
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
                let colour = sky_colour(dir, sky);
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
fn sky_colour(dir: Vec3, state: &Sky) -> Vec3 {
    let sun = state.sun;
    let cover = state.cover;
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
    // Cover scales all three decks about their drawn proportions, so a clearing sky
    // opens from the bottom up — the low deck goes first and the high cirrus is the
    // last to leave, which is what clearing weather actually looks like.
    let shade = cover / COVER_LOW;
    let (storm_d, storm_l) = deck(
        dir,
        sun,
        CLOUD_STORM,
        SCALE_STORM,
        COVER_STORM * shade,
        23.0,
    );
    let (high_d, high_l) = deck(
        dir,
        sun,
        CLOUD_HIGH,
        SCALE_HIGH,
        COVER_HIGH * (0.45 + 0.55 * shade),
        11.0,
    );
    let (low_d, low_l) = deck(dir, sun, CLOUD_LOW, SCALE_LOW, cover, 3.0);
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
    let (shadow, sunlit) = state.cloud_tones();
    let sky = {
        let storm = shadow.lerp(sunlit, (storm_l * 0.4 + towering * 0.75).clamp(0.0, 1.0));
        let high = shadow.lerp(sunlit, (high_l * 0.7 + towering * 0.4).clamp(0.0, 1.0));
        // The low deck is the one that stays dark: it is under everything else and only
        // its edges catch the sun. This is what makes a squall line read as a squall
        // line rather than as grey weather.
        let low = shadow.lerp(
            sunlit,
            (low_l * 0.9 + towering * 0.15).clamp(0.0, 1.0) * 0.8,
        );
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
    let rays = shafts(dir, sun, cover) * 0.5;
    sky + warm * (disc + corona) + warm * haze + warm * rays
}

/// The colour the haze has to be: the sky's own average along the horizon.
///
/// Derived rather than picked, and now derived from the *rendered* sky rather than from
/// the constant it starts out as. The difference matters since there is weather in it.
/// Distance fades everything towards this colour, so an island ten miles off is this
/// colour — and if the sky behind it is a squall while this is a clear-day horizon, the
/// island comes out brighter than its own background and cuts a hard white shape out of
/// it. Which is exactly what it did: a pale silhouette against dark cloud, with an edge
/// no amount of distance would ever produce.
///
/// Averaged over a ring of directions just above the horizon, which is where everything
/// far away actually sits. A hundred and twenty-eight samples of the same function the
/// cubemap is painted with, so the two cannot disagree about what colour the distance is.
fn horizon_average(sky: &Sky) -> Vec3 {
    let mut sum = Vec3::ZERO;
    const N: usize = 128;
    for i in 0..N {
        let a = std::f32::consts::TAU * i as f32 / N as f32;
        // A couple of degrees up: low enough to be the distance, high enough not to be
        // sampling the sea's own reflection band.
        let dir = Vec3::new(a.cos(), 0.04, a.sin()).normalize();
        sum += sky_colour(dir, sky);
    }
    // The sun's disc and corona are in there and would drag the mean towards white if a
    // sample landed on them; clamped, because haze is haze and not a light source.
    (sum / N as f32).min(Vec3::splat(1.0))
}

pub fn haze_colour(sky: &Sky) -> Color {
    let c = horizon_average(sky);
    Color::linear_rgb(c.x, c.y, c.z)
}

/// Slightly deeper, for the near end of the fog gradient.
pub fn haze_near(sky: &Sky) -> Color {
    let c = horizon_average(sky) * 0.88;
    Color::linear_rgb(c.x, c.y, c.z)
}

/// Marks the camera's copy of the cubemap handle, so the rebake can find the image to
/// write into without going through the skybox or the environment light.
#[derive(bevy::prelude::Component)]
pub struct SkyImage(pub Handle<Image>);

/// A rebake in flight.
///
/// The bake is fifty-nine milliseconds. That is nothing once an hour and far too much
/// in a frame, so it goes to the task pool and the result is swapped in when it lands.
/// Spreading it over frames instead would mean ten milliseconds a frame for six frames,
/// which is a stutter rather than a saving.
#[derive(bevy::prelude::Resource, Default)]
pub struct Baking {
    task: Option<bevy::tasks::Task<Image>>,
    /// The sky the last bake was made for, so the next one is only started when the
    /// picture has actually moved.
    baked: Option<Sky>,
}

/// How long a passage hour takes. One to one, like everything else here.
///
/// The sun moves fifteen degrees of hour angle an hour, which at this latitude is about
/// five degrees of elevation in the morning. Over a two hour leg that is a visible
/// change in the light and nowhere near a different time of day — which is right: a
/// delivery is hours, and the sun should move like it.
const HOURS_PER_SECOND: f32 = 1.0 / 3_600.0;

/// How far the sun has to move, or the cover change, before it is worth sixty
/// milliseconds of someone's core to repaint the sky.
const SUN_STEP: f32 = 0.6_f32.to_radians();
const COVER_STEP: f32 = 0.04;

/// Advance the clock and the weather, and repaint the sky when either has moved.
pub fn drift(
    sim: bevy::prelude::Res<crate::sim::Sim>,
    time: bevy::prelude::Res<Time>,
    mut sky: bevy::prelude::ResMut<Sky>,
    mut baking: bevy::prelude::ResMut<Baking>,
    mut images: bevy::prelude::ResMut<Assets<Image>>,
    held: bevy::prelude::Query<&SkyImage>,
) {
    let hour = sky.hour + time.delta_secs() * HOURS_PER_SECOND;
    // Cover comes from the same slow weather that moves the mean wind, plus whatever
    // squall is overhead — which is how a squall manages to darken the whole sky rather
    // than only the patch its own cloud is drawn on.
    let overcast = sim.sim.wind.cover() as f32;
    let under = sim.sim.wind.squall_at(sim.sim.state.pos) as f32;
    *sky = Sky::at(hour, (overcast + 0.45 * under).clamp(0.0, 1.0));

    // Collect a finished bake first, so a slow frame cannot leave one waiting.
    if let Some(task) = &mut baking.task {
        if let Some(image) = bevy::tasks::block_on(bevy::tasks::poll_once(task)) {
            baking.task = None;
            if let Some(handle) = held.iter().next() {
                if let Some(mut slot) = images.get_mut(&handle.0) {
                    // Written into the same asset, so the skybox and the environment
                    // light both follow without being told: one image, two jobs, and
                    // the sun you can see stays the sun the water reflects.
                    *slot = image;
                    debug!(
                        "sky: repainted at {:02.0}:{:02.0}, cover {:.0}%, sun {:.0} deg up",
                        sky.hour.floor(),
                        sky.hour.fract() * 60.0,
                        sky.cover * 100.0,
                        sky.elevation().to_degrees()
                    );
                }
            }
        }
        return;
    }

    let moved = baking.baked.is_none_or(|was| {
        was.sun.angle_between(sky.sun) > SUN_STEP || (was.cover - sky.cover).abs() > COVER_STEP
    });
    if moved {
        let want = *sky;
        baking.baked = Some(want);
        baking.task =
            Some(bevy::tasks::AsyncComputeTaskPool::get().spawn(async move { cubemap(&want) }));
    }
}

/// Point the sun at where the sun is, and colour it for how high it is.
///
/// The light and the drawn sky come from the same `Sky`, which is the only arrangement
/// in which a shadow can be trusted to fall the way the sky says it should.
pub fn light(
    sky: bevy::prelude::Res<Sky>,
    mut ambient: bevy::prelude::ResMut<GlobalAmbientLight>,
    mut lamp: bevy::prelude::Query<(&mut Transform, &mut DirectionalLight)>,
) {
    let Ok((mut t, mut light)) = lamp.single_mut() else {
        return;
    };
    t.look_to(-sky.sun, Vec3::Y);
    let tint = sky.sunlight();
    light.color = Color::linear_rgb(tint.x, tint.y, tint.z);
    light.illuminance = sky.illuminance();
    ambient.brightness = sky.ambient();
    // And the colour of it: a clear sky's diffuse light is blue because it is the sky,
    // and an overcast's is grey because the cloud has taken the colour out of it.
    let c = crate::sky::ZENITH.lerp(Vec3::new(0.78, 0.80, 0.84), sky.cover);
    ambient.color = Color::linear_rgb(c.x, c.y, c.z);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sun has to be in the east in the morning and the west in the afternoon.
    ///
    /// Trivial, and it is here because it was wrong. The arc cosine in `Sky::at` only
    /// gives half the circle and the hour angle picks the side; the sign was inverted,
    /// which put a ten o'clock sun in the south-west. Nothing in the picture says
    /// "wrong" at a glance — it just quietly lights the boat from the wrong quarter and
    /// points every shadow the wrong way.
    #[test]
    fn the_sun_rises_in_the_east() {
        let bearing = |hour: f32| {
            let s = Sky::at(hour, 0.5);
            s.sun.x.atan2(-s.sun.z).to_degrees().rem_euclid(360.0)
        };
        assert!(
            (70.0..140.0).contains(&bearing(7.0)),
            "a seven o'clock sun should bear east-south-east, got {:03.0}",
            bearing(7.0)
        );
        assert!(
            (220.0..290.0).contains(&bearing(17.0)),
            "a five o'clock sun should bear west-south-west, got {:03.0}",
            bearing(17.0)
        );
        // And it is highest at noon, which is what "noon" means.
        assert!(Sky::at(12.0, 0.5).elevation() > Sky::at(9.0, 0.5).elevation());
        assert!(Sky::at(12.0, 0.5).elevation() > Sky::at(15.0, 0.5).elevation());
    }

    /// The cover has to reach the paint. It is threaded through four functions to get
    /// there, and a dropped argument anywhere along the way would leave a sky that
    /// never changes while every number in the game says it has.
    #[test]
    fn cover_and_sun_both_change_the_painted_sky() {
        let brightness = |sky: &Sky| {
            cubemap(sky)
                .data
                .as_ref()
                .map(|d| d.iter().map(|&b| b as u64).sum::<u64>())
                .unwrap_or(0)
        };
        let clear = brightness(&Sky::at(8.0, 0.1));
        let solid = brightness(&Sky::at(8.0, 0.95));
        assert_ne!(clear, solid, "cover must change the sky");
        let morning = brightness(&Sky::at(7.0, 0.5));
        let noon = brightness(&Sky::at(12.0, 0.5));
        assert_ne!(morning, noon, "the sun moving must change the sky");
    }
}

#[cfg(test)]
mod lighting {
    use super::*;

    /// The one test that would have caught the grey afternoon.
    ///
    /// The camera has a *fixed* exposure — `Exposure::SUNLIGHT`, chosen once — so the
    /// scene's light is not free to be physically honest across its whole range. Real
    /// sun runs ten to one from clear to overcast; a fixed exposure holds about two
    /// stops before the picture is either a silhouette or a sheet of white.
    ///
    /// So this asserts the *band*, not the formula: whatever weather the generator draws
    /// and whatever hour the passage starts, the direct light has to land inside what
    /// the camera can hold. A version of `illuminance` that was perfectly defensible as
    /// physics put 33 000 lux into a scene exposed for 85 000 and the whole picture came
    /// out flat and grey, which is a fault no test about *directions* would have seen.
    #[test]
    fn the_light_stays_inside_what_the_camera_can_hold() {
        // Sixty to a hundred and forty thousand is about one stop either side of the
        // 85 000 the scene was lit and exposed for.
        const FLOOR: f32 = 60_000.0;
        const CEILING: f32 = 140_000.0;
        let mut worst = (f32::MAX, 0.0_f32, (0.0, 0.0));
        for hour in [5.0, 6.5, 8.0, 10.0, 12.0, 15.0, 18.0] {
            for cover in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let sky = Sky::at(hour, cover);
                let lux = sky.illuminance();
                if lux < worst.0 {
                    worst = (lux, worst.1, (hour, cover));
                }
                worst.1 = worst.1.max(lux);
                assert!(
                    (FLOOR..=CEILING).contains(&lux),
                    "at {hour:02}:00 under {:.0}% cloud the sun is {lux:.0} lux, which the \
                     camera cannot hold",
                    cover * 100.0
                );
            }
        }
        println!(
            "light: {:.0} to {:.0} lux across the weather (darkest at {:02.0}:00, {:.0}% cloud)",
            worst.0,
            worst.1,
            worst.2 .0,
            worst.2 .1 * 100.0
        );
    }

    /// And the directions, which are the physics the band is allowed to move inside.
    #[test]
    fn cloud_moves_light_from_the_sun_to_the_sky() {
        let clear = Sky::at(10.0, 0.05);
        let dull = Sky::at(10.0, 0.95);
        assert!(
            dull.illuminance() < clear.illuminance(),
            "cloud has to take something off the sun"
        );
        assert!(
            dull.ambient() > clear.ambient() * 1.5,
            "and put it in the sky: that is where an overcast's light comes from, and \
             holding the ambient still while dimming the sun is what makes cloudy \
             weather read as dusk"
        );
        // Total light is roughly conserved, which is the claim: an overcast day is not
        // a dark day, it is a day with no shadows in it.
        let total = |s: &Sky| s.illuminance() + s.ambient() * 40.0;
        let ratio = total(&dull) / total(&clear);
        assert!(
            (0.7..=1.05).contains(&ratio),
            "an overcast should be flat, not dark: {ratio:.2} of the clear-sky total"
        );
        // Higher sun, more light, whatever the cloud is doing.
        assert!(Sky::at(12.0, 0.5).illuminance() > Sky::at(6.5, 0.5).illuminance());
    }
}
