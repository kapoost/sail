// The sea, worked out on the card.
//
// **This is the same arithmetic as `sail-core::water`, in the other language.** The
// hull asks that one, sixteen points at a time, two hundred times a second; the drawn
// surface asks this one, forty thousand points at a time, sixty times a second. They
// have to agree about where the water is or every cue in the picture disagrees with
// every other — so the duplication is deliberate and it is *pinned by a test*:
// `tests/sea_on_the_card.rs` compiles this file, runs it over sample points, and
// compares the answers against the Rust spectrum. If the two ever drift, that fails.
//
// What makes the agreement possible in single precision is that nothing here is ever
// asked about an absolute position. The phase of each component at the boat —
// `k·x − ωt + φ`, which after an hour at sea is tens of thousands of radians — is
// worked out in `f64` on the processor and handed over already wrapped into a turn.
// This shader only ever adds `k · (direction · offset)` to it, and the offset is a
// grid offset: three kilometres at the very most, a couple of hundred metres where
// anybody can see the difference. A float has room for that.
//
// Plain WGSL on purpose: no imports, no conditional compilation. The two entry points
// (`sea.wesl` for the picture, `sea_prepass.wesl` for the shadow pass) import this, and
// so does the test's compute shader — which can only compile it if it stands alone.

/// One Gerstner component, as the processor hands it over.
///
/// `theta0` is this component's phase *at the boat*, this frame, already wrapped. It is
/// the only part of the sum that needs double precision, and it is the one part that is
/// not computed here.
struct Component {
    amplitude: f32,
    k: f32,
    wavelength: f32,
    theta0: f32,
    direction: vec2<f32>,
    pad: vec2<f32>,
}

/// Everything about this frame that is the same for every vertex.
///
/// One storage buffer, written in place once a frame. Not a uniform and not twenty
/// separate bindings: a buffer whose *size* never changes is a buffer whose bind group
/// never has to be rebuilt, and a bind group rebuilt sixty times a second is exactly
/// the kind of per-frame graphics-resource churn this program has learnt to avoid.
///
/// The `to_*` vectors are all the same trick as `theta0`. A world position near a
/// Danish island is a number in the tens of thousands; the texture anchor, the cloud
/// shadow corner and the bank's own grid are all close to the boat, so what crosses
/// over is the *difference*, which is small, and the vertex adds its own offset to it.
struct Sea {
    /// The eye, in the sea mesh's own frame. A mirror condition has to be evaluated in
    /// one frame or the other, not half in each.
    eye: vec4<f32>,
    /// Unit vector towards the sun, Bevy axes.
    sun: vec4<f32>,
    /// The sea's dark end and the colour of water too far off to carry any detail.
    deep: vec4<f32>,
    far: vec4<f32>,
    /// What the water itself returns from below, which is why the two ends are not
    /// simply the sky scaled.
    body: vec4<f32>,
    /// The sky near the horizon: a mean plus a bearing term. A crest shows the sky it
    /// faces, and at the ends of the day that is a different colour every way you look.
    dome_mean: vec4<f32>,
    dome_east: vec4<f32>,
    dome_south: vec4<f32>,
    /// Boat minus the texture anchor, less the drift — the parcel's own coordinates.
    to_parcel: vec2<f32>,
    /// Boat minus the texture anchor. For the flat water beyond the near field, which
    /// does not move and so has no parcel.
    to_flat: vec2<f32>,
    /// Boat minus the cloud shadow map's corner, metres.
    to_shade: vec2<f32>,
    /// Boat minus the bank map's corner, metres.
    to_sand: vec2<f32>,
    /// Which way the wind is blowing to, simulation axes.
    downwind: vec2<f32>,
    /// Significant height, never below five centimetres.
    hs: f32,
    /// This sea's own typical slope, which is what foam is judged against.
    typical: f32,
    /// Whether this sea breaks at all.
    whitecapping: f32,
    /// The longest wave present. Past this spacing the mesh carries nothing and the
    /// water is never asked.
    longest: f32,
    shade_pitch: f32,
    sand_pitch: f32,
    /// The ship's clock, in seconds, for the streaks that run up the back of a wave.
    streak: f32,
    /// Drawn water rather than photographed water: 1 or 0.
    ink: f32,
    components: u32,
    /// Nodes across each map; zero when there is nothing to show.
    shade_nodes: u32,
    sand_nodes: u32,
    /// Metres of sea across one repeat of the ripple map, and how far from the boat a
    /// cloud shadow is drawn. Both are baked into something on the processor as well, so
    /// they are passed rather than written down twice.
    tile: f32,
    shadow_reach: f32,
    pad0: u32,
    /// How far ring `k` stands from the boat, and the vertex spacing there. The grid is
    /// graded and symmetric, so one hundred and one numbers describe it in both axes.
    rings: array<f32, 101>,
    spacings: array<f32, 101>,
    wave: array<Component, 21>,
    /// Cloud shadow, 64 × 64 over ten kilometres.
    shade: array<f32, 4096>,
    tail: array<f32, 2>,
}

/// How deep the water is, 128 × 128 over two and a half kilometres.
///
/// Its own buffer, because it is not a property of the frame: depth is a function of
/// *place*, so this is rewritten only when the boat has moved half a node — a few
/// seconds' sailing — and otherwise sits on the card untouched. Only built for the drawn
/// sea, where the bank is a tone the picture is made of.
struct Bank {
    depth: array<f32, 16384>,
}

// Group three is where a material's own bindings live in this engine, and it is told to
// the shader as a constant rather than written as a number — the engine moved it from
// two to three between versions, and a shader that had guessed would have compiled and
// then read the mesh's bindings as if they were the sea.
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<storage, read> sea: Sea;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var<storage, read> bank: Bank;

/// One vertex of the drawn sea.
struct Dressed {
    position: vec3<f32>,
    normal: vec3<f32>,
    tangent: vec4<f32>,
    uv: vec2<f32>,
    colour: vec4<f32>,
}

// --- the dials -------------------------------------------------------------------
//
// These live here now, with the reasoning that chose them, because nothing on the
// processor draws water any more and a constant whose argument has been left behind in
// another file is a constant somebody will change for the wrong reason.
//
// Two numbers that both sides need — the metres across one tile of the ripple map, and
// how far a cloud shadow reaches — are *not* here. They are in the frame, passed over
// like everything else, because the texture is baked on the processor and the shadow
// field is rasterised there: writing them twice is the fault this file is otherwise
// avoiding.

// How many tones the drawn sea is allowed.
//
// Four. Three is a poster and six is a photograph with a fault in it; four is what an
// inker uses for water, and it is enough to carry a wave's shape and not enough to
// suggest a gradient.
const INK_BANDS: f32 = 4.0;

// How much of a tone band is the step between it and the one below, 0 to 1.
//
// The drawn sea shimmered, and this is why. A tone band was a `floor`: a vertex whose
// tone crept across a boundary changed colour by a *whole band* between one frame and
// the next, and since there are forty thousand of them and the water under them never
// stops moving, some were always crossing. Thirty per cent of the brightness range,
// flipped, sixty times a second, scattered over the surface.
//
// So the staircase keeps its treads and grows a riser. Eighty-four per cent of each band
// is dead flat, which is what makes the drawing a drawing; the remaining sixth is a
// smooth climb to the next tone, so a vertex crossing it moves instead of jumping. The
// worst a hundredth of tone can now do is two hundredths of a band, against a third of
// the whole range before.
const INK_RISE: f32 = 0.16;

// How wide the contour line on the water is, as a fraction of a tone band.
//
// The sea gets no silhouette, because it has nothing to be a silhouette against — it is
// what everything else is drawn against. What a drawn sea has instead is a contour: the
// line along the top of a wave, which in this style is not a separate thing at all. It
// is the edge between two tones. The bands are already here, so the line is drawn by
// marking the water that sits just either side of each step, and it lands exactly where
// an inker would put it — along the crest and around the shoulder — because that is
// where the tone changes.
const CONTOUR: f32 = 0.13;

// How strongly the contour is struck.
//
// White on the light side, because what the line is marking is the crest and a crest is
// where the water is bright: drawn dark it reads as a crack in the surface. The dark
// side gets the key block's own line at a little less, which is what a woodblock does —
// every edge in a print carries a dark line on the side where the form turns away.
const CONTOUR_INK: f32 = 0.6;

// How nearly a facet must mirror the sun to be struck as a highlight.
//
// The cosine of the angle between the surface normal and the half-vector between eye and
// sun — which is the mirror condition, so the glitter path is where it actually falls
// rather than where it would be convenient. Tuned by counting: at 0.985 a fifteen-knot
// sea lights about a tenth of a per cent of the resolved vertices, which is a path and
// not a sheet.
const GLINT_COS: f32 = 0.985;

// Fraction of eligible facets that are actually struck.
//
// Not every facet on the path gets a mark. A continuous band of white is a lens flare;
// what a sea does — and what an inker draws — is a scatter of separate dashes. The
// choice is a hash of the water parcel, so each dash belongs to a piece of water and
// rides the wave through instead of flickering as the grid slides underneath.
const GLINT_STIPPLE: f32 = 0.42;

// How far apart the streaks run on the water, as a fraction of the wave height.
//
// The strokes are contours of the surface. A fixed bearing laid them in rows across the
// sea, which is hatching — shading laid *over* a form. What the print does is draw lines
// that follow the form itself, round the shoulder of a wave and up its face, and that is
// the whole difference between a shaded photograph and a drawing.
//
// Iso-height lines are exactly those lines, and they come free: the height is already
// known at every vertex, so making it the texture coordinate turns the stroke pattern
// into a contour map of the sea. A seventh of the significant height, so a wave carries
// about seven streaks from trough to crest whether it is a ripple or a gale.
const STREAK_OF_HEIGHT: f32 = 0.14;

// How fast the streaks run up the back of a wave and down its face, bands a second.
//
// Iso-height lines standing still are a contour map, and a contour map of a moving sea
// is still a map: the water goes through the lines and the lines do not move, so nothing
// on the surface is *going* anywhere. What a wave actually does to the water in it is
// the orbit — a parcel rises up the back, goes forward over the top, falls down the face
// and comes back through the trough — so the water climbs through the contours on the
// windward side and falls through them on the lee side. Both sides of a crest shear
// against each other, which piles the lines up along it, correctly, because that is
// where the water is thrown together.
const STREAK_RUN: f32 = 0.55;

// The paper a woodblock is printed on: not white but a warm off-cream. The contrast
// between it and the pigment is what makes flat tone read as form.
const PAPER: vec3<f32> = vec3<f32>(0.780, 0.800, 0.775);

// Where the water is thin: over a bank, and in the back of a wave.
//
// A print has more than two ends to its range and this one had two, which is why the
// drawn sea read as flat however many bands were cut into it: four tones spread between
// a mid blue and a pale blue are four mid blues. Between the two ends the water is
// allowed to be a different colour rather than a mixture of them — because that is what
// water does. Thin water is not pale deep water. It is green, because what you are
// seeing is light that went down, hit something and came back rather than light that
// went down and did not return.
const SHOAL: vec3<f32> = vec3<f32>(0.090, 0.330, 0.300);

// The key line, struck on the dark side of each tone step.
//
// A woodblock print is a *key block* — the drawing in line — with colour blocks
// registered behind it, so every edge in the picture carries a dark line on one side.
// The sea had only the white one, and the side it does not hold for is the other one,
// where the form turns away and a print has its heaviest line.
const KEY_LINE: vec3<f32> = vec3<f32>(0.012, 0.030, 0.075);

// How far the texture coordinates are pushed about, in tiles, and over what distance.
//
// **A repeating texture is not seen as repetition — it is seen as a grid.** Twenty-four
// metres of ripples across three hundred metres of water is a dozen copies of one patch,
// and what gives it away is not that the patches are the same but that their corners
// line up: the eye finds the lattice long before it recognises the content.
//
// So the coordinates are warped by a slow swirl before they are used. The same patches
// are still there — there is one texture and that does not change — but they no longer
// land on a grid, and a lattice with no straight lines in it stops being a lattice.
//
// Two swirls rather than one, because one only trades a lattice for a smear. The long
// one moves whole tiles about over two hundred metres; the short one bends their edges
// over sixty. The stretch between two vertices is what bounds them: 1.1 tiles over two
// hundred metres and 0.35 over sixty both come out near a quarter of a tile per ten
// metres of water, against a vertex every seventy centimetres.
const WARP_LONG: f32 = 1.1;
const WARP_LONG_OVER: f32 = 200.0;
const WARP_SHORT: f32 = 0.35;
const WARP_SHORT_OVER: f32 = 60.0;

// --- small curves ----------------------------------------------------------------

fn ease(t: f32) -> f32 {
    let x = clamp(t, 0.0, 1.0);
    return x * x * (3.0 - 2.0 * x);
}

fn step_between(edge0: f32, edge1: f32, x: f32) -> f32 {
    return ease((x - edge0) / max(edge1 - edge0, 1e-6));
}

/// Foam only belongs near a crest. A steep trough is still green water.
fn crest_weight(lift: f32) -> f32 {
    return clamp((lift + 0.10) / 0.40, 0.0, 1.0);
}

/// How much of what you are looking at came *through* the water rather than off it.
fn seen_through(sun: vec3<f32>, view: vec3<f32>, normal: vec3<f32>) -> f32 {
    return pow(max(-dot(sun, view), 0.0), 1.6) * max(dot(normal, view), 0.0);
}

/// `normalize`, except that a zero vector comes back a zero vector rather than a NaN.
/// The eye can sit exactly on a vertex, and one NaN in a position is a torn triangle.
fn norm_or_zero(v: vec3<f32>) -> vec3<f32> {
    let len = length(v);
    if len < 1e-20 {
        return vec3<f32>(0.0);
    }
    return v / len;
}

// --- sixty-four bits, by hand ------------------------------------------------------
//
// The noise fields in this game are hashed in 64-bit integer arithmetic, and WGSL has
// no 64-bit integer. The alternative was a second, 32-bit hash on the card — which
// would have been cheaper and would have meant the water's detail stood in different
// places depending on which side of the fence it was drawn on. Two or three dozen
// instructions a lattice point buys one field instead of two, and this card is idle:
// the world is rendered at 960 across and scaled up.

struct U64 {
    lo: u32,
    hi: u32,
}

fn u64_of(lo: u32, hi: u32) -> U64 {
    return U64(lo, hi);
}

/// An `i32` widened the way Rust widens an `i64` into a `u64`: sign extended.
fn u64_of_i32(v: i32) -> U64 {
    let lo = bitcast<u32>(v);
    var hi = 0u;
    if v < 0 {
        hi = 0xFFFFFFFFu;
    }
    return U64(lo, hi);
}

fn u64_xor(a: U64, b: U64) -> U64 {
    return U64(a.lo ^ b.lo, a.hi ^ b.hi);
}

/// Wrapping multiply, from four sixteen-bit products. Every intermediate below is
/// bounded by `2^32 - 1`, which is why the carries are added where they are.
fn u64_mul(a: U64, b: U64) -> U64 {
    let a0 = a.lo & 0xFFFFu;
    let a1 = a.lo >> 16u;
    let b0 = b.lo & 0xFFFFu;
    let b1 = b.lo >> 16u;
    let p00 = a0 * b0;
    let t = a1 * b0 + (p00 >> 16u);
    let u = a0 * b1 + (t & 0xFFFFu);
    let lo = (u << 16u) | (p00 & 0xFFFFu);
    let hi = a1 * b1 + (t >> 16u) + (u >> 16u) + a.lo * b.hi + a.hi * b.lo;
    return U64(lo, hi);
}

/// Logical shift right, for `1 <= n <= 63`.
fn u64_shr(a: U64, n: u32) -> U64 {
    if n < 32u {
        return U64((a.lo >> n) | (a.hi << (32u - n)), a.hi >> n);
    }
    return U64(a.hi >> (n - 32u), 0u);
}

/// The top twenty-four bits as a float in [0, 1). The Rust side divides the top
/// fifty-three by `2^53`; taking twenty-four of them agrees to one part in sixteen
/// million, which is below anything a float can carry anyway.
fn u64_unit(a: U64) -> f32 {
    return f32(u64_shr(a, 40u).lo) / 16777216.0;
}

// --- value noise, the same field the wind and the land are made of -----------------

fn lattice(ix: i32, iy: i32, seed: U64) -> f32 {
    let kx = u64_of(0x7F4A7C15u, 0x9E3779B9u);
    let ky = u64_of(0x27D4EB4Fu, 0xC2B2AE3Du);
    var h = u64_xor(
        u64_xor(seed, u64_mul(u64_of_i32(ix), kx)),
        u64_mul(u64_of_i32(iy), ky),
    );
    h = u64_xor(h, u64_shr(h, 33u));
    h = u64_mul(h, u64_of(0xED558CCDu, 0xFF51AFD7u));
    h = u64_xor(h, u64_shr(h, 33u));
    h = u64_mul(h, u64_of(0x1A85EC53u, 0xC4CEB9FEu));
    h = u64_xor(h, u64_shr(h, 33u));
    return u64_unit(h) * 2.0 - 1.0;
}

/// One octave on a unit lattice, smoothstep interpolated. Range [−1, 1].
fn value(p: vec2<f32>, seed: U64) -> f32 {
    let i = floor(p);
    let f = p - i;
    let w = f * f * (vec2<f32>(3.0) - 2.0 * f);
    let ix = i32(i.x);
    let iy = i32(i.y);
    let a = lattice(ix, iy, seed);
    let b = lattice(ix + 1, iy, seed);
    let c = lattice(ix, iy + 1, seed);
    let d = lattice(ix + 1, iy + 1, seed);
    let top = a + (b - a) * w.x;
    let bottom = c + (d - c) * w.x;
    return top + (bottom - top) * w.y;
}

fn two_octaves(p: vec2<f32>, seed: U64) -> f32 {
    let second = u64_xor(seed, u64_of(0x9E37u, 0u));
    return 0.74 * value(p, seed) + 0.26 * value(p * 2.7 + vec2<f32>(11.3, -7.1), second);
}

/// Stable per-parcel hash, used to decide which facets catch the light.
fn stipple(parcel: vec2<f32>) -> f32 {
    let x = i32(floor(parcel.x * 0.37));
    let y = i32(floor(parcel.y * 0.37));
    var h = u64_xor(
        u64_mul(u64_of_i32(x), u64_of(0x27D4EB2Du, 0u)),
        u64_mul(u64_of_i32(y), u64_of(0x165667B1u, 0u)),
    );
    h = u64_xor(h, u64_shr(h, 33u));
    h = u64_mul(h, u64_of(0xED558CCDu, 0xFF51AFD7u));
    h = u64_xor(h, u64_shr(h, 29u));
    return u64_unit(h);
}

/// Where in the ripple map this patch of water reads from.
///
/// World-locked, so the detail stays with the sea rather than with the boat, and warped
/// by two octaves of slow noise so that the tile stops reading as a tile.
fn ripple_uv(parcel: vec2<f32>) -> vec2<f32> {
    let long_x = value((parcel + vec2<f32>(0.0)) / WARP_LONG_OVER, u64_of(0x05EA1A11u, 0u));
    let long_y = value((parcel + vec2<f32>(37.0)) / WARP_LONG_OVER, u64_of(0x05EA1A12u, 0u));
    let short_x = value((parcel + vec2<f32>(113.0)) / WARP_SHORT_OVER, u64_of(0x05EA1A13u, 0u));
    let short_y = value((parcel + vec2<f32>(211.0)) / WARP_SHORT_OVER, u64_of(0x05EA1A14u, 0u));
    let warp = vec2<f32>(long_x, long_y) * WARP_LONG + vec2<f32>(short_x, short_y) * WARP_SHORT;
    return vec2<f32>(parcel.x / sea.tile, -parcel.y / sea.tile) + warp;
}

// --- the fields the processor hands over -------------------------------------------

/// How shaded a point on the water is: 0 in full sun, 1 under the middle of a cloud.
///
/// Bilinear off the same map the processor builds, read the same way, so a shadow does
/// not step between the two descriptions of it.
fn shade_at(local: vec2<f32>) -> f32 {
    if sea.shade_nodes == 0u {
        return 0.0;
    }
    let nodes = i32(sea.shade_nodes);
    let p = (sea.to_shade + local) / sea.shade_pitch;
    if p.x < 0.0 || p.y < 0.0 {
        return 0.0;
    }
    let ix = i32(p.x);
    let iy = i32(p.y);
    if ix + 1 >= nodes || iy + 1 >= nodes {
        return 0.0;
    }
    let fx = p.x - f32(ix);
    let fy = p.y - f32(iy);
    let row = iy * nodes + ix;
    let a = sea.shade[row];
    let b = sea.shade[row + 1];
    let c = sea.shade[row + nodes];
    let d = sea.shade[row + nodes + 1];
    let top = a + (b - a) * fx;
    let bottom = c + (d - c) * fx;
    return top + (bottom - top) * fy;
}

/// How deep the water is here, metres. Forty — open water — wherever the map does not
/// reach, which is the honest answer for a sea with no bank in it.
fn depth_at(local: vec2<f32>) -> f32 {
    if sea.sand_nodes == 0u {
        return 40.0;
    }
    let nodes = i32(sea.sand_nodes);
    let p = (sea.to_sand + local) / sea.sand_pitch;
    if p.x < 0.0 || p.y < 0.0 {
        return 40.0;
    }
    let ix = i32(p.x);
    let iy = i32(p.y);
    if ix + 1 >= nodes || iy + 1 >= nodes {
        return 40.0;
    }
    let fx = p.x - f32(ix);
    let fy = p.y - f32(iy);
    let row = iy * nodes + ix;
    let a = bank.depth[row];
    let b = bank.depth[row + 1];
    let c = bank.depth[row + nodes];
    let d = bank.depth[row + nodes + 1];
    let top = a + (b - a) * fx;
    let bottom = c + (d - c) * fx;
    return top + (bottom - top) * fy;
}

/// What arrives from near the horizon on this bearing.
fn dome_towards(dir: vec3<f32>) -> vec3<f32> {
    let flat = vec2<f32>(dir.x, dir.z);
    let len = length(flat);
    if len < 1e-6 {
        return sea.dome_mean.xyz;
    }
    let lit = sea.dome_mean.xyz
        + sea.dome_east.xyz * (flat.x / len)
        + sea.dome_south.xyz * (flat.y / len);
    return max(lit, vec3<f32>(0.0));
}

// --- the grid ----------------------------------------------------------------------

/// Which ring of the graded grid stands at this distance from the boat.
///
/// The same monotone walk `world::drawn_resolution` does, over the same table, because
/// everything that has to sit *on* this surface — a mark of foam, a wake, a raindrop —
/// asks that one for its band limit. A surface filtered one way with foam laid on it
/// filtered another is two descriptions of one thing, and it blinks.
fn ring_of(d: f32) -> u32 {
    var lo = 0u;
    var hi = 100u;
    loop {
        if lo >= hi {
            break;
        }
        let mid = (lo + hi) / 2u;
        if sea.rings[mid] < d {
            lo = mid + 1u;
        } else {
            hi = mid;
        }
    }
    return lo;
}

/// The shortest wave this vertex can carry.
///
/// Four times the spacing, not twice. Twice is the Nyquist limit, at which a component
/// is kept at full strength with four samples across it — and four samples is where a
/// moving sea visibly beats, because the grid slides along underneath it. This rolls a
/// component off between four and eight samples, so nothing is drawn that the mesh
/// cannot animate smoothly.
fn resolution_at(offset: vec2<f32>) -> f32 {
    let kx = ring_of(abs(offset.x));
    let kz = ring_of(abs(offset.y));
    return 4.0 * max(sea.spacings[kx], sea.spacings[kz]);
}

// --- the sum -----------------------------------------------------------------------

/// Height, slope and drift of one patch of water, band-limited to what the mesh can
/// carry. Simulation axes: x east, y north.
struct Parcel {
    height: f32,
    slope: vec2<f32>,
    drift: vec2<f32>,
}

/// How much of this component a mesh of the given resolution can actually carry.
///
/// One at twice the limit and above, rolled smoothly to nothing as the wavelength
/// approaches it — smooth rather than a hard cut, because a component switching off
/// between one vertex ring and the next draws a visible seam in the water.
fn resolved(wavelength: f32, min_wavelength: f32) -> f32 {
    if min_wavelength <= 0.0 {
        return 1.0;
    }
    let t = clamp(wavelength / min_wavelength - 1.0, 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

/// All three from the one sine and cosine, which is most of a sea's worth of frame time
/// for nothing but asking the right question.
///
/// The spectrum is built long to short, so the first component this mesh cannot carry is
/// also the last one worth evaluating.
fn surface_resolved(local: vec2<f32>, min_wavelength: f32) -> Parcel {
    var height = 0.0;
    var slope = vec2<f32>(0.0);
    var drift = vec2<f32>(0.0);
    for (var i = 0u; i < sea.components; i = i + 1u) {
        let c = sea.wave[i];
        let w = resolved(c.wavelength, min_wavelength);
        if w <= 0.0 {
            break;
        }
        let theta = c.theta0 + c.k * dot(c.direction, local);
        let s = sin(theta);
        let co = cos(theta);
        let a = c.amplitude * w;
        height = height + a * co;
        slope = slope + c.direction * (-a * c.k * s);
        drift = drift - c.direction * (a * s);
    }
    return Parcel(height, slope, drift);
}

// --- the drawn sea ------------------------------------------------------------------

/// Drawn water, not photographed water.
///
/// A painter does not shade a gradient; a painter picks three or four tones and puts a
/// hard edge between them, and the eye reads the shape of that edge as the shape of the
/// wave. Returns the flattened tone, how near a step this vertex sits, and which side of
/// it — light above, dark below.
fn ink_tone(tone: f32) -> vec3<f32> {
    let x = clamp(tone * INK_BANDS, 0.0, INK_BANDS);
    let f = x - floor(x);
    // Flat, then a climb, and the climb lands exactly where the next band's floor
    // begins — so the two agree at the join and the staircase is continuous.
    let level = floor(x) + step_between(0.0, INK_RISE, f) - 1.0;
    let banded = clamp(level / (INK_BANDS - 1.0), 0.0, 1.0);
    let to_step = min(f, 1.0 - f);
    var side = -1.0;
    if f < 0.5 {
        side = 1.0;
    }
    return vec3<f32>(banded, 1.0 - step_between(0.0, CONTOUR, to_step), side);
}

/// One vertex of the sea, from its offset from the boat.
///
/// Written as a pure function of that offset and the frame's own numbers, which is what
/// it always was underneath: this used to run on the processor's cores, and before that
/// on one of them.
fn dress(offset: vec2<f32>) -> Dressed {
    let ink = sea.ink > 0.5;
    let min_wavelength = resolution_at(offset);
    // Bevy's −Z is simulation north, so the horizontal offset flips sign on the way
    // across.
    let local = vec2<f32>(offset.x, -offset.y);
    let look = vec3<f32>(offset.x - sea.eye.x, 0.0, offset.y - sea.eye.z);

    if min_wavelength >= sea.longest {
        // Past this spacing the mesh carries nothing, so the water is never asked. On a
        // grid that runs to the horizon that is most of the vertices, and it is what
        // pays for the fine spacing close in.
        //
        // The far water is nearly all horizon reflection — it is seen at a grazing angle
        // — so it takes the sky on its own bearing rather than one flat colour for the
        // whole ring. A third of the way towards the sky and no further: drawn the whole
        // way, an evening overcast made the far water the same pale grey as the sky
        // above it and the two became one field with a boat in the middle.
        let from_sky = dome_towards(look);
        var lit = sea.far.xyz;
        if !ink {
            lit = mix(sea.far.xyz, from_sky * 0.17 + sea.body.xyz * 1.4, 0.3);
        }
        // In the drawn sea the coordinates address the hatching rather than the ripple
        // map, and past the near field there is no wave to shade and nothing to hatch:
        // the top row of the art map is blank paper.
        var uv = vec2<f32>(0.0);
        if !ink {
            uv = ripple_uv(sea.to_flat + local);
        }
        return Dressed(
            vec3<f32>(offset.x, 0.0, offset.y),
            vec3<f32>(0.0, 1.0, 0.0),
            vec4<f32>(1.0, 0.0, 0.0, -1.0),
            uv,
            vec4<f32>(lit, 1.0),
        );
    }

    let water = surface_resolved(local, min_wavelength);
    let height = water.height;
    let sx = water.slope.x;
    let sz = -water.slope.y;

    // **And it moves sideways as well as up**, which is the difference between a sea and
    // a corrugated sheet. Gerstner is a parametrisation: the parcel whose mean position
    // is here is drawn at here-plus-its-own-displacement, and that displacement is what
    // gathers water into the crests and spreads it out under the troughs.
    let point = vec3<f32>(offset.x + water.drift.x, height, offset.y - water.drift.y);
    let normal = normalize(vec3<f32>(-sx, 1.0, -sz));
    // The parcel is simply *here*: the vertex goes where the parcel goes, so its mean
    // position is its own grid point and walking the coordinates back by the drift would
    // be applying it twice.
    let parcel = sea.to_parcel + local;
    // Only for the lit sea: the drawn one writes its own coordinates below, and the
    // warp is sixteen lattice hashes that would be thrown away.
    var uv = vec2<f32>(0.0);
    if !ink {
        uv = ripple_uv(parcel);
    }
    // Tangent along +U, which is world +X, lying in the surface. Handedness is negative
    // because cross(normal, tangent) points at −Z while V runs to +Z.
    let along = normalize(vec3<f32>(1.0, sx, 0.0));
    let tangent = vec4<f32>(along, -1.0);

    // Colour per vertex: troughs darker, crests lighter, and foam where the surface is
    // steep enough to be breaking.
    let lift = clamp(height / sea.hs, -1.0, 1.0);
    var tone = clamp(lift * 0.5 + 0.5, 0.0, 1.0);

    // Cloud shadow, taken off the tone *before* it is quantised, which is the whole
    // trick: in ink mode the shadow then lands on a tone boundary and comes out as a
    // flat patch with a hard contour round it, which is what a painter does with a cloud
    // shadow. Added afterwards it would be a grey wash over the drawing.
    //
    // Only near the boat, and that is as much an honesty as an economy: the eye is two
    // metres above the water, so at three kilometres a shadow patch is a line, not a
    // patch.
    let range = length(offset);
    let near = 1.0 - step_between(sea.shadow_reach * 0.58, sea.shadow_reach, range);
    var shade = 0.0;
    if near > 0.0 {
        shade = near * shade_at(local);
    }
    tone = clamp(tone - 0.42 * shade, 0.0, 1.0);

    var edge = 0.0;
    var side = 1.0;
    if ink {
        let banded = ink_tone(tone);
        tone = banded.x;
        edge = banded.y;
        side = banded.z;
    }

    // The light end on *this* bearing. A low sun lays a sheet of light on the water one
    // way and leaves it nearly black the other.
    let from_sky = dome_towards(look);
    var shallow = from_sky * 0.17 + sea.body.xyz * 1.4;
    if ink {
        shallow = mix(PAPER, from_sky * 0.62 + sea.body.xyz, 0.34);
    }
    var base = mix(sea.deep.xyz, shallow, tone);

    if ink {
        // How thin the water is here, from the two things that thin it: the wave's own
        // lens, and the bank underneath. Ten metres is where a sand bottom starts to
        // show through on a clear day; below two it is all bottom.
        let lens = clamp(height / (0.9 * sea.hs), 0.0, 1.0);
        let over_sand = 1.0 - step_between(2.0, 11.0, depth_at(local));
        let thin = max(lens * 0.55, over_sand);
        base = mix(base, SHOAL, thin * thin);
    }

    // Foam judged against this sea's own typical slope rather than a fixed number. A
    // long swell and a short chop of the same height are nothing alike in steepness.
    let steepness = length(vec2<f32>(sx, sz)) / sea.typical;
    var foam = clamp((steepness - 1.25) / 0.75, 0.0, 1.0) * crest_weight(lift) * sea.whitecapping;
    if ink {
        // Foam becomes ink: on or off, because a drawn whitecap is a shape with an
        // outline rather than a soft bloom — and with a ragged edge to it, because what
        // a breaking crest does is tear along the top and come down in claws. The noise
        // is read at the parcel, so the tearing travels with the water.
        let torn = two_octaves(parcel * 0.9, u64_of(0x05EAF0A4u, 0u));
        foam = step_between(0.15 + 0.07 * torn, 0.21 + 0.07 * torn, foam);
    }
    var white = vec3<f32>(0.82, 0.88, 0.92);
    if ink {
        white = vec3<f32>(0.95, 0.97, 1.0);
    }
    var colour = mix(base, white, foam);

    // --- Water you can see *into*, where it is thin ---------------------------------
    //
    // **The one thing a painted sea never has.** A wave is a lens: at the crest there is
    // half a metre of water between you and the sky on the other side, and with the sun
    // behind it the whole top of the wave goes a light jade and glows — which is why the
    // sea looks like a liquid and a grey surface with highlights on it looks like a
    // floor. Written as colour rather than as alpha, deliberately: what reads as seeing
    // into the water is the colour of the light that came through it.
    let enough_sea = step_between(0.35, 1.0, sea.hs);
    let thin = crest_weight(lift) * step_between(0.7, 1.8, steepness) * enough_sea;
    if thin > 0.01 && shade < 0.6 {
        let view = norm_or_zero(sea.eye.xyz - point);
        let through = seen_through(sea.sun.xyz, view, normal);
        // Jade, not cyan: the sea's own green with the blue taken out of it by the depth
        // it did *not* have to travel through.
        var lit = vec3<f32>(0.34, 0.80, 0.68);
        if ink {
            lit = vec3<f32>(0.42, 0.74, 0.62);
        }
        let how_much = clamp(thin * through * (1.0 - foam), 0.0, 0.75);
        colour = mix(colour, lit, how_much);
    }

    if ink {
        // The hatching's coordinates, now that this vertex knows how dark it is. Across
        // the stroke in world metres, so the strokes lie on the water; down the tone,
        // which is what picks the width of the stroke out of the tonal art map.
        //
        // `sz` is the *negated* north slope, because Bevy's −Z is north while `downwind`
        // is in the simulation's frame — dotting them as they stand would have the
        // north-going half of every wind backwards.
        let tilt = (sx * sea.downwind.x - sz * sea.downwind.y) / sea.typical;
        let face = tanh(tilt);
        uv = vec2<f32>(
            height / (STREAK_OF_HEIGHT * sea.hs) + face * STREAK_RUN * sea.streak,
            1.0 - tone,
        );
        if edge > 0.0 {
            // The line along the step between two tones, struck in two colours, which is
            // what a woodblock does: white where the form turns towards the light,
            // because that edge is the crest, and the key block's own dark line where it
            // turns away.
            var mark = KEY_LINE;
            var strength = CONTOUR_INK * 0.85;
            if side > 0.0 {
                mark = vec3<f32>(0.97, 0.99, 1.0);
                strength = CONTOUR_INK;
            }
            colour = mix(colour, mark, strength * edge);
        }
        // Glints off the wave faces, as marks rather than as a sheen: struck where the
        // facet actually mirrors the sun towards the eye, and only on the facets the
        // stipple picks, which is how a scatter of dashes along the glitter path comes
        // out instead of a continuous smear of white. Not in a cloud shadow — there is
        // no sun in there to glint off, and that is the one thing that would give the
        // shadow away as paint.
        let view = norm_or_zero(sea.eye.xyz - point);
        let half_way = norm_or_zero(view + sea.sun.xyz);
        if shade < 0.45 && dot(normal, half_way) > GLINT_COS && stipple(parcel) < GLINT_STIPPLE {
            colour = vec3<f32>(1.0, 1.0, 0.97);
        }
    }

    return Dressed(point, normal, tangent, uv, vec4<f32>(colour, 1.0));
}
