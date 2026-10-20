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

/// The dome, the sun in it, and the light they throw.
///
/// What this module does in a frame, in one place — see `stages.rs` for why the
/// schedule is named sets rather than one long chain.
pub fn plugin(app: &mut App) {
    use crate::stages::Deck;
    app.init_resource::<Baking>();
    app.init_resource::<Dome>();
    app.add_systems(Update, (drift, light).in_set(Deck::Weather));
}

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

    /// How far into the ends of the day this is: 0 by mid-morning, 1 at the horizon.
    ///
    /// One number, read by the sky gradient, the shafts and the undersides of cloud, for
    /// the same reason the sun's direction is one number: a sunset in which the sky has
    /// turned and the rays through it have not is worse than no sunset, because the eye
    /// reads the mismatch before it reads either half.
    ///
    /// Twelve degrees is where it starts, which is roughly where a person first says
    /// "it's going red" rather than "the sun is low". Squared, because the colour does
    /// not arrive linearly — nothing much happens for the first hour and then the last
    /// twenty minutes do all of it.
    pub fn low_sun(&self) -> f32 {
        let deg = self.elevation().to_degrees();
        // Rising into it from twelve degrees up, and then — the half that a clamp at
        // zero gets wrong — *falling out of it again* once the sun is properly down.
        // Colour at dusk is sunlight hitting air you can still see; at six degrees
        // below the horizon that air is only the high stuff and the fire is going, and
        // by ten it is night. Without the second edge the sky stayed on fire until
        // dawn, which is a thing no one has ever seen.
        let climbing = (1.0 - (deg / 12.0).clamp(0.0, 1.0)).powf(2.0);
        let dying = ((deg + 10.0) / 6.0).clamp(0.0, 1.0);
        climbing * dying
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
        clear * (1.0 - 0.33 * self.cover) * self.below_horizon()
    }

    /// How much of the sun's light survives it being under the horizon.
    ///
    /// **A bug that could not be seen until the hour could be set.** The height above
    /// was clamped at zero, so a sun twenty-seven degrees below the horizon lit the
    /// scene exactly as hard as a sun on it: one in the morning came out as bright as
    /// noon. Nobody had met it because a passage starts between half past six and half
    /// past eight and runs for well under a day — the game had never been anywhere near
    /// midnight. `SAIL_HOUR` went there on its first run.
    ///
    /// Civil twilight is the shape of it: the light goes quickly for the first few
    /// degrees and is gone by about six, which is when you stop being able to read.
    /// Not quite to nothing, because a sea at night is not pitch black and a scene that
    /// is would look broken rather than dark.
    pub fn daylight(&self) -> f32 {
        self.below_horizon()
    }

    fn below_horizon(&self) -> f32 {
        let deg = self.elevation().to_degrees();
        if deg >= 0.0 {
            return 1.0;
        }
        (1.0 + deg / 6.0).clamp(0.02, 1.0)
    }

    /// The sky's own light, which is the other half of it.
    ///
    /// Rises with cover, because that is where the direct light has gone: an overcast is
    /// the whole dome lit evenly, and it is why a cloudy day has no shadows in it rather
    /// than no light. Holding this constant while the sun was dimmed is what made cloudy
    /// weather read as dusk.
    pub fn ambient(&self) -> f32 {
        // The sky's own light goes out with the sun, but more slowly: a dome that is
        // still lit from below the horizon is exactly what twilight is, and it is why
        // you can see for half an hour after sunset without being able to read.
        let dusk = self.below_horizon().powf(0.55);
        (300.0 + 700.0 * self.cover) * dusk.max(0.03)
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
/// The zenith at the ends of the day, and the horizon under the sun then.
///
/// **Why the sky needed to stop being two constants.** Dawn and dusk were the same
/// grey as noon: only the *sunlight* warmed, so the water carried a gold glitter path
/// under a sky with no colour in it at all. That is the one combination the eye never
/// sees, because the redness of low light and the redness of the sky are the same
/// event seen from two directions — the blue end of the spectrum has been scattered
/// out of the beam, and what it scattered *into* is the sky you are looking at.
///
/// At a low sun the light crosses something like forty times the air it crosses
/// overhead. Blue is gone from the beam entirely, so the beam reads orange-red; the
/// zenith loses its source of blue too and goes deep, towards the violet a clear
/// twilight actually is.
const ZENITH_LOW: Vec3 = Vec3::new(0.055, 0.075, 0.21);
/// Horizon under a low sun: this is the fire, and it is deliberately beyond what a
/// screen can show. The tone map brings it back, and a sunset that does not clip
/// somewhere is a sunset nobody believes.
const HORIZON_FIRE: Vec3 = Vec3::new(1.35, 0.46, 0.17);
/// And the horizon *away* from a low sun, which is not grey either: the anti-twilight
/// arch, a dusty pink band over a blue-grey shadow of the earth itself. Weak, wide,
/// and the thing that stops a sunset looking like a lamp in a black room.
const HORIZON_COUNTER: Vec3 = Vec3::new(0.52, 0.40, 0.46);
/// The horizon at dusk in the directions that are neither: abeam of the sun.
///
/// Needed because the first version left this as the daytime horizon, and the sum came
/// out saying that ninety degrees off the sun a sunset has not happened. It has: the
/// whole dome dims and warms, and only the *amount* varies with bearing. Leaving the
/// beam pale blue made the fire look like a lamp someone had switched on beside an
/// otherwise ordinary afternoon.
const HORIZON_DUSK: Vec3 = Vec3::new(0.46, 0.36, 0.38);
/// Night, overhead and at the horizon.
///
/// **`low_sun` and darkness are two different questions, and conflating them made the
/// sky bright at one in the morning.** The first asks how red the light is, and it
/// peaks at the horizon and falls away on *both* sides — there is no colour at noon and
/// none at midnight. The second asks how much light there is at all, and only falls.
/// Driving the drawn sky from the first alone meant that once the sun was far enough
/// down for the fire to go out, the sky went back to its daytime blue rather than on to
/// black.
///
/// Not pure black: a sea under a clear night sky is dark but the horizon is still
/// findable, and a scene rendered to nothing reads as broken rather than as night.
const NIGHT_ZENITH: Vec3 = Vec3::new(0.008, 0.012, 0.030);
const NIGHT_HORIZON: Vec3 = Vec3::new(0.030, 0.038, 0.062);
/// What the sea throws back up at the underside of cloud.
///
/// Cloud bases over water are not lit only from above. A sea under a low sun is a
/// sheet of broken mirrors throwing a good deal of that light back up, and the
/// undersides catch it — which is why cloud over water reads warmer underneath than
/// the same cloud over land, and why the base of a squall at sunset glows instead of
/// going flat black. Small, because the sea is dark and most of what hits it is
/// absorbed, but the difference between a little and none is the difference between
/// cloud that sits in the scene and cloud pasted on it.
const SEA_BOUNCE: Vec3 = Vec3::new(0.30, 0.26, 0.21);
/// The sun's own disc and corona at the horizon, and the shafts through low air.
///
/// Separate from the light it *casts* — [`Sky::sunlight`] — because they are not the
/// same quantity. What the sun casts on the boat has crossed the air once; what you
/// see when you look at the disc has crossed it once as well but is being compared
/// against a dark sky, so it reads far redder. A sun that sets white over an orange
/// sea is the specific wrongness this pair of constants exists to remove.
const SUN_LOW: Vec3 = Vec3::new(1.0, 0.44, 0.17);
/// Shafts go further still: a ray is a long slant through the thickest air there is.
const RAY_LOW: Vec3 = Vec3::new(1.0, 0.34, 0.14);
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

/// The middle storey: altocumulus, the mackerel sky.
///
/// Between the ragged low stuff and the high sheet there was nothing, and a sky with a
/// gap in the middle of it reads as two wallpapers rather than as weather. This is the
/// deck that fills it — two and a half thousand metres, a finer grain than the layers
/// either side of it, and broken rather than solid.
///
/// It is also the storey that *makes* a red sky. A low sun is below the low cloud and
/// shining along underneath the middle one, so this is the layer that lights from
/// beneath while everything under it goes to silhouette — which is the whole mechanism
/// of the thing people photograph.
const CLOUD_MID: f32 = 2500.0;
const SCALE_MID: f32 = 2600.0;
const COVER_MID: f32 = 0.38;

/// And cirrus, which is ice and sits above all the weather.
///
/// Thin, fast, and the last thing lit at dusk: eight thousand metres of altitude buys
/// perhaps twenty extra minutes of sunlight, so cirrus burns on after the sea has gone
/// dark. That is why it is here rather than as another cumulus deck — it is the layer
/// that gives a sky colour when everything below it has stopped having any.
const CLOUD_CIRRUS: f32 = 8200.0;
const SCALE_CIRRUS: f32 = 7000.0;
const COVER_CIRRUS: f32 = 0.34;

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
/// Where the painted low deck starts, metres along the ray.
///
/// Six kilometres: the particle field in `clouds.rs` reaches seven and a half, so the
/// painting fades in while the particles are still going and neither leaves a gap.
const PAINTED_FROM: f32 = 6_000.0;

fn deck(
    dir: Vec3,
    sun: Vec3,
    height: f32,
    scale: f32,
    cover: f32,
    drift: f32,
    painted_from: f32,
) -> (f32, f32) {
    if dir.y <= 0.02 {
        return (0.0, 0.0);
    }
    let reach = height / dir.y;
    // **Only what is genuinely far away belongs on a cubemap.**
    //
    // A cubemap is painted at infinity, so a cloud on it is the same cloud from every
    // position and has no parallax at all. That is right for the high deck and wrong for
    // the low one, which `clouds.rs` also draws, as particles, out to seven and a half
    // kilometres — so the low cloud was being drawn twice, in two different places, from
    // two different fields: the painted one blocked the shafts and the real one cast the
    // shadows, and neither knew about the other.
    //
    // Faded in with distance along the ray instead. Looking up steeply the deck is a few
    // hundred metres off and the particles own that sky; looking towards the horizon it
    // is kilometres away and the painting owns it. The handover overlaps the particles'
    // own range so there is no band of empty sky between them.
    let near = if painted_from > 0.0 {
        ((reach / painted_from) - 1.0).clamp(0.0, 1.0)
    } else {
        1.0
    };
    if near <= 0.0 {
        return (0.0, 0.0);
    }
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
    let density = ((n - (1.0 - cover)) / 0.22).clamp(0.0, 1.0) * fade * near;
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
        let (low, _) = deck(at, sun, CLOUD_LOW, SCALE_LOW, cover, 3.0, PAINTED_FROM);
        let (storm, _) = deck(
            at,
            sun,
            CLOUD_STORM,
            SCALE_STORM,
            COVER_STORM * cover / COVER_LOW,
            23.0,
            0.0,
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
    let low = state.low_sun();
    // Night is a separate axis from the colour of dusk: the sun going further down
    // takes the light away whether or not it took the red with it.
    let day = state.daylight();
    // **The fire is where the sun is, and that is the whole difference between a sunset
    // and a tinted sky.** Dusk is not a uniform band: it is a hot arc perhaps sixty
    // degrees wide over the sun, a cool blue through the beam of the sky, and a dusty
    // pink arch on the far side. Warming the whole horizon equally is what a colour
    // grade does, and it reads as one — the sky goes orange and the direction of the
    // light disappears with it.
    //
    // Compass bearing only: the sun's height has already been taken into account by
    // `low_sun`, and what is left is which way you are facing.
    let flat_sun = Vec2::new(sun.x, sun.z).normalize_or_zero();
    let flat_dir = Vec2::new(dir.x, dir.z).normalize_or_zero();
    let facing = flat_dir.dot(flat_sun);
    // Towards the sun, and away from it. The exponents set how wide each arc is: the
    // fire covers roughly the sun's quarter of the compass and the counter-glow is
    // broader and weaker, which is the shape of the real thing.
    let towards = ((facing + 0.25) / 1.25).clamp(0.0, 1.0).powf(1.2);
    let against = ((-facing + 0.2) / 1.2).clamp(0.0, 1.0).powf(1.6);
    // The three arcs, summed by how much of each this direction is in. They do not
    // overlap much — towards and against are opposite ends of the same dot product —
    // and what is left over in the beam of the sky keeps the daytime horizon.
    let horizon = {
        let rest = (1.0 - towards - against).max(0.0);
        let dusk = HORIZON_FIRE * towards + HORIZON_COUNTER * against + HORIZON_DUSK * rest;
        let lit = HORIZON.lerp(dusk, low);
        NIGHT_HORIZON.lerp(lit, day)
    };
    let zenith = NIGHT_ZENITH.lerp(ZENITH.lerp(ZENITH_LOW, low), day);
    let sky = if up >= 0.0 {
        // Pale at the horizon, deep overhead. The exponent is doing the work of
        // atmospheric depth: most of the gradient happens in the first few degrees.
        //
        // And the exponent itself moves: at dusk the warm band is *thinner* and sits
        // closer to the water, so the gradient has to tighten or the whole dome goes
        // orange and the thing stops being a sunset and becomes a filter.
        let tightness = 3.2 + 2.6 * low;
        zenith.lerp(horizon, (1.0 - up).powf(tightness))
    } else {
        // Fade into the sea colour just below the horizon rather than cutting, or the
        // reflection picks up a hard edge where the two meet.
        let t = (-up / 0.08).clamp(0.0, 1.0);
        horizon.lerp(BELOW, t)
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
        0.0,
    );
    let (high_d, high_l) = deck(
        dir,
        sun,
        CLOUD_HIGH,
        SCALE_HIGH,
        COVER_HIGH * (0.45 + 0.55 * shade),
        11.0,
        0.0,
    );
    let (mid_d, mid_l) = deck(
        dir,
        sun,
        CLOUD_MID,
        SCALE_MID,
        COVER_MID * (0.5 + 0.5 * shade),
        7.0,
        0.0,
    );
    let (cir_d, cir_l) = deck(
        dir,
        sun,
        CLOUD_CIRRUS,
        SCALE_CIRRUS,
        COVER_CIRRUS * (0.7 + 0.3 * shade),
        17.0,
        0.0,
    );
    let (low_d, low_l) = deck(dir, sun, CLOUD_LOW, SCALE_LOW, cover, 3.0, PAINTED_FROM);
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
    // Cloud at night is darker than the sky behind it, not brighter: it is blocking
    // what little there is rather than catching any.
    let night_cloud = NIGHT_ZENITH * 0.6;
    let shadow = night_cloud.lerp(shadow, day);
    let sunlit = night_cloud.lerp(sunlit, day);
    // **What the sea throws back up.** The underside of cloud over water is lit from
    // below as well as above, and at the ends of the day that is most of the light it
    // gets: the sun is under the low deck, so the bases are in shadow from above and
    // facing a sheet of broken mirrors. Strongest where you are looking along the
    // water towards the sun and nearly gone overhead, because that is where the sea
    // in view is actually bright.
    let bounce = {
        let flat = (1.0 - dir.y.max(0.0)).powf(2.5);
        SEA_BOUNCE * state.sunlight() * (low * 0.9 + 0.1) * flat * (1.0 - cover * 0.5)
    };
    let shadow = shadow + bounce * towards.max(0.25);
    // Cloud lit from beneath by a sun that is already under it. This is the red sky:
    // not cloud turned orange, but cloud whose *lit side is the bottom*, which only
    // happens for the twenty minutes either end of the day.
    let underlit = sunlit * SUN_LOW * 1.6;
    let sunlit = sunlit.lerp(underlit, low * towards.max(0.2));
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
        // Cirrus is ice and barely shades anything, so it is lit almost regardless of
        // which way it faces — and it is the last layer with sun on it.
        let cirrus = shadow.lerp(sunlit * 1.15, (cir_l * 0.5 + 0.5).clamp(0.0, 1.0));
        let mid = shadow.lerp(sunlit, (mid_l * 0.8 + towering * 0.3).clamp(0.0, 1.0));
        // Laid in from the top down, so each deck runs in front of the ones above it.
        let with_cirrus = sky.lerp(cirrus, cir_d * 0.55);
        let with_storm = with_cirrus.lerp(storm, storm_d * 0.95);
        let with_high = with_storm.lerp(high, high_d * 0.8);
        let with_mid = with_high.lerp(mid, mid_d * 0.7);
        with_mid.lerp(low, low_d * 0.92)
    };

    let alignment = dir.dot(sun).max(0.0);
    // Disc, then a tight corona, then the broad forward-scattered haze that makes the
    // whole sky brighter near the sun.
    // The sun is behind whatever cloud is in front of it, which is the other half of
    // why a northern sky has any drama in it: the disc comes and goes.
    let veiled = (1.0 - low_d * 0.95)
        * (1.0 - mid_d * 0.7)
        * (1.0 - high_d * 0.6)
        * (1.0 - cir_d * 0.25)
        * (1.0 - storm_d * 0.98);
    let disc = if alignment > 0.99975 {
        SUN_DISC * veiled
    } else {
        0.0
    };
    let corona = alignment.powf(1800.0) * 180.0 * veiled;
    let haze = alignment.powf(5.0) * 0.45;

    // **The sun's own colour, which was a constant and is the reason a sunset read as
    // a bright afternoon with a tinted sky.** The disc, the corona round it and the
    // forward-scattered haze are all the same light arriving by different paths, so
    // they take the same tint — and at the horizon that tint is not warm white, it is
    // orange going on red. The beam has crossed forty times the air it crosses
    // overhead, and almost nothing blue is left in it.
    let warm = Vec3::new(1.0, 0.94, 0.82).lerp(SUN_LOW, low);
    // The shafts, which are haze lit along a clear line to the sun. Added rather than
    // mixed, because that is what light does to air.
    //
    // **And they carry more colour than the haze around them**, which is what makes a
    // sky with rays in it worth looking at: a shaft is a long path through low air, so
    // it is reddened further than the general glow — and the gaps between shafts are
    // the sky's own blue showing through, so the contrast is in hue as much as in
    // brightness. Pushing the tint rather than the brightness is what keeps them from
    // looking like a lens effect.
    let ray_tint = warm.lerp(RAY_LOW, low * 0.8);
    let rays = shafts(dir, sun, cover) * 0.5;
    sky + warm * (disc + corona) + warm * haze + ray_tint * rays
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

/// The sky as the *water* sees it.
///
/// **The sea is a mirror and it was painted from two constants.** Its two ends — the
/// colour a trough shows and the colour a crest shows — were `ZENITH` and `HORIZON`,
/// which are a fine blue afternoon and nothing else: the same water at seven in the
/// morning, at noon and under a squall, with the sun anywhere it liked. Reported from
/// the deck as the sun's position not reaching the water, and it did not, because
/// nothing connected them.
///
/// What a patch of sea shows is the sky it reflects, and that depends on the bearing:
/// looking towards a low sun the water is a sheet of light and the same water over your
/// shoulder is nearly black. So the dome is sampled round the horizon and fitted to a
/// constant and a direction — `mean + x·east + z·south`, which is the first spherical
/// harmonic and is exactly the "bright one way, dark the other" that a low sun does.
/// Two multiplies and two adds at a vertex, against an arc tangent and a table lookup.
///
/// Built when the sky is repainted and not before: these are the expensive samples, the
/// ones that walk the cloud decks, and the sky only moves half a degree at a time.
#[derive(bevy::prelude::Resource, Clone, Copy, Debug)]
pub struct Dome {
    mean: Vec3,
    east: Vec3,
    south: Vec3,
    zenith: Vec3,
}

impl Dome {
    /// How many bearings are sampled round the horizon.
    const POINTS: usize = 24;
    /// How far up to sample: the band a wave face actually mirrors.
    const BAND: f32 = 0.12;

    pub fn of(sky: &Sky) -> Self {
        let mut mean = Vec3::ZERO;
        let (mut east, mut south) = (Vec3::ZERO, Vec3::ZERO);
        for i in 0..Self::POINTS {
            let a = std::f32::consts::TAU * i as f32 / Self::POINTS as f32;
            let dir = Vec3::new(a.cos(), Self::BAND, a.sin()).normalize();
            // The sun's own disc would drag a sample to white and is not what the sea
            // is showing; the glitter is drawn separately and from the real mirror
            // condition.
            let c = sky_colour(dir, sky).min(Vec3::splat(1.2));
            mean += c;
            east += c * a.cos();
            south += c * a.sin();
        }
        let n = Self::POINTS as f32;
        Self {
            mean: mean / n,
            east: east * 2.0 / n,
            south: south * 2.0 / n,
            zenith: sky_colour(Vec3::Y, sky),
        }
    }

    /// Straight up, which is what a trough shows.
    #[inline]
    pub fn zenith(&self) -> Vec3 {
        self.zenith
    }

    /// What the horizon looks like on a bearing, as the three terms it is made of.
    ///
    /// A mean plus an east term plus a south term: dotted with a horizontal direction
    /// they give the colour arriving from near the horizon that way, which is what a
    /// wave crest shows. The sea's vertex shader evaluates that at forty thousand
    /// vertices, so it is handed the coefficients rather than the answer — and this is
    /// the one place that knows what they are.
    #[inline]
    pub fn terms(&self) -> (Vec3, Vec3, Vec3) {
        (self.mean, self.east, self.south)
    }
}

impl Default for Dome {
    fn default() -> Self {
        Self::of(&Sky::default())
    }
}

pub fn haze_colour(sky: &Sky) -> Color {
    let c = horizon_average(sky);
    Color::linear_rgb(c.x, c.y, c.z)
}

/// Slightly deeper, for the near end of the fog gradient.
/// How far you can see, metres.
///
/// **This was 2600 and a constant, and it is the reason islands had no shape in them.**
/// Aerial perspective is what tells an eye how far away a hill is, and at two and a half
/// kilometres of visibility an island four kilometres off is one flat pale band: the
/// haze has eaten the difference between its sunlit side and its shadowed one before
/// either reaches you. The far shore read as a cut-out rather than as land.
///
/// Real visibility at sea is weather. A clear day on this coast is ten miles and more —
/// you see the next island's cliffs and the cloud standing over it — and the same water
/// in drizzle is two thousand metres, in a squall a few hundred, and in fog less than
/// you would like. So it follows the sky rather than sitting still:
///
/// * **Cover closes it**, because the weather that puts cloud over you is the weather
///   that puts moisture between you and everything else.
/// * **A low sun closes it further**, which is the sideways path through the haze: the
///   same reason the sun reddens is the reason you cannot see far along the water at
///   dusk.
/// * **And it never opens past about twelve kilometres**, because the sea patch ends
///   somewhere and haze is what hides the edge. That bound is the renderer's, not the
///   weather's, and it is worth saying so.
///
/// What colour the haze glows where you are looking towards the sun through it.
///
/// The sun's own tint, dimmed by whatever is in front of it: this is scattered light,
/// so it is never brighter than the source and it goes when the source does.
pub fn glow_colour(sky: &Sky) -> Color {
    let tint = Vec3::new(1.0, 0.94, 0.82).lerp(SUN_LOW, sky.low_sun());
    let strength = 0.55 * (1.0 - 0.6 * sky.cover) * sky.daylight();
    let c = tint * strength;
    Color::linear_rgb(c.x, c.y, c.z)
}

pub fn visibility(sky: &Sky) -> f32 {
    let clear = 12_000.0;
    let thick = 1_400.0;
    let shut = sky.cover.powf(0.75) * 0.72 + sky.low_sun() * 0.25;
    clear + (thick - clear) * shut.clamp(0.0, 1.0)
}

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
    mut dome: bevy::prelude::ResMut<Dome>,
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
        // The water's own copy of the sky, rebuilt on the same trigger and from the same
        // state — which is the only arrangement in which the sea can be trusted to be
        // the colour of what is above it.
        *dome = Dome::of(&want);
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
        // **Daylight hours only, and that is a narrowing of this test rather than a
        // weakening of it.** It used to include 05:00, which at this latitude and
        // declination is half an hour *before sunrise* — and it passed, because the
        // sun's height was clamped at zero and a sun four degrees under the horizon lit
        // the scene as hard as one on it. That was the bug, not the bound.
        //
        // The fixed exposure is a claim about the hours the game is sailed in: a
        // passage starts between half past six and half past eight and runs for well
        // under a day. Outside them the light is supposed to go, and the test below
        // says so.
        for hour in [6.0, 6.5, 8.0, 10.0, 12.0, 15.0, 18.0] {
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

    /// Sunrise and sunset for the latitude and declination this archipelago is drawn at.
    const SUNSET: f32 = 18.52;
    const NOON: f32 = 12.0;
    const NIGHT: f32 = 1.0;

    /// The redness peaks at the horizon and falls away on *both* sides of it.
    ///
    /// The second half is the one that was wrong: clamping the sun's height at zero
    /// meant `low_sun` stayed at one all night, so the sky was on fire until dawn.
    #[test]
    fn colour_belongs_to_the_ends_of_the_day() {
        let noon = Sky::at(NOON, 0.3).low_sun();
        let setting = Sky::at(SUNSET, 0.3).low_sun();
        let night = Sky::at(NIGHT, 0.3).low_sun();
        assert!(
            noon < 0.05,
            "noon should have no sunset colour in it: {noon}"
        );
        assert!(
            setting > 0.8,
            "the horizon is where the colour is: {setting}"
        );
        assert!(night < 0.05, "one in the morning is not a sunset: {night}");
    }

    /// And outside those hours it is *meant* to leave the camera's range, downwards.
    ///
    /// The pair of this and the test above is the whole contract: inside the sailing
    /// day the light holds within a stop of what the scene was exposed for, and outside
    /// it there is simply less of it.
    #[test]
    fn outside_the_sailing_day_the_light_goes() {
        let dawn = Sky::at(4.5, 0.0).illuminance();
        let midnight = Sky::at(0.0, 0.0).illuminance();
        assert!(
            dawn < 60_000.0,
            "an hour before sunrise should be below the daylight floor, got {dawn:.0}"
        );
        assert!(
            midnight < 5_000.0,
            "midnight should be dark, got {midnight:.0}"
        );
    }

    /// And the light goes out when the sun does, which is a different question.
    #[test]
    fn the_sun_stops_lighting_the_scene_once_it_is_down() {
        let noon = Sky::at(NOON, 0.3).illuminance();
        let dusk = Sky::at(SUNSET + 0.5, 0.3).illuminance();
        let night = Sky::at(NIGHT, 0.3).illuminance();
        assert!(
            dusk < noon * 0.75,
            "half an hour after sunset is dimmer: {dusk}"
        );
        assert!(
            night < noon * 0.05,
            "one in the morning lit at {night} against {noon} at noon"
        );
    }

    /// The fire is on the sun's side of the sky, not all round the horizon.
    ///
    /// A sunset that warms every bearing equally is a colour grade, and it reads as
    /// one: the direction of the light disappears with it.
    #[test]
    fn the_fire_is_where_the_sun_is() {
        let sky = Sky::at(SUNSET, 0.0);
        let flat = Vec3::new(sky.sun.x, 0.0, sky.sun.z).normalize();
        let just_above = |d: Vec3| {
            let dir = (d + Vec3::new(0.0, 0.06, 0.0)).normalize();
            sky_colour(dir, &sky)
        };
        let towards = just_above(flat);
        let away = just_above(-flat);
        let warmth = |c: Vec3| c.x - c.z;
        assert!(
            warmth(towards) > warmth(away),
            "towards the sun {:?} should be warmer than away {:?}",
            towards,
            away
        );
        assert!(
            warmth(towards) > 0.1,
            "the sunset side should be properly warm, not a tint: {towards:?}"
        );
    }

    /// Noon is cold and blue, which is the half that must not regress.
    #[test]
    fn noon_stays_blue() {
        let sky = Sky::at(NOON, 0.0);
        let up = sky_colour(Vec3::new(0.0, 1.0, 0.0), &sky);
        assert!(
            up.z > up.x * 1.5,
            "the zenith at noon should be much bluer than it is red: {up:?}"
        );
    }

    /// Night is darker than day everywhere, including straight up.
    #[test]
    fn night_is_dark() {
        let day = sky_colour(Vec3::new(0.0, 1.0, 0.0), &Sky::at(NOON, 0.3));
        let night = sky_colour(Vec3::new(0.0, 1.0, 0.0), &Sky::at(NIGHT, 0.3));
        assert!(
            night.length() < day.length() * 0.2,
            "night zenith {night:?} against day {day:?}"
        );
    }

    /// Every cloud storey is above the one below it, and they do not share a height.
    ///
    /// Cheap, and it catches the edit that puts the middle deck above the high one —
    /// which draws as the layers passing through each other and is hard to read as a
    /// height mistake when you are looking at it.
    #[test]
    fn the_storeys_are_in_order() {
        let decks = [CLOUD_LOW, CLOUD_MID, CLOUD_HIGH, CLOUD_STORM, CLOUD_CIRRUS];
        for pair in decks.windows(2) {
            assert!(
                pair[1] > pair[0],
                "cloud decks out of order: {} then {}",
                pair[0],
                pair[1]
            );
        }
    }
}
