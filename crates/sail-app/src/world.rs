//! Sea, sky and light.
//!
//! The surface is displaced from **the same wave field the physics uses**. That is not
//! an implementation detail: a boat riding one sea while the eye watches a different
//! one is worse than no waves at all, because every cue disagrees with every other.
//!
//! The mesh is a graded grid that follows the boat — about 0.7 m between vertices
//! underneath her, widening to 25 m at the horizon seven hundred metres out, so the
//! detail is where the eye is. Vertices are sampled at true world coordinates rather
//! than at grid offsets, so recentring the grid does not make the sea swim.
//!
//! Normals come from the field's analytic slope rather than from the triangles. That
//! is both cheaper than recomputing them and exactly right, and it means the lighting
//! shows the wave shape even where the mesh is too coarse to.

use bevy::asset::RenderAssetUsages;
use bevy::light::CascadeShadowConfigBuilder;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::camera;
use crate::curve::smoothstep;
use crate::frame;
use crate::sea_texture;
use crate::sim::Sim;
use crate::sky;

/// How far the ring `i` stands from the boat, metres.
///
/// Free functions rather than closures inside the setup, because the spray has to ask
/// the same questions: a mark of foam sitting on a surface worked out one way, drawn
/// over a surface worked out another, is two descriptions of one thing and it blinks.
fn half(i: usize) -> f32 {
    if i <= NEAR_RINGS {
        // Arithmetic spacing: the sum of a ramp, in closed form.
        let n = i as f32;
        let last = (NEAR_RINGS - 1) as f32;
        SPACING_NEAR * n + (SPACING_FAR - SPACING_NEAR) * n * (n - 1.0) / (2.0 * last)
    } else {
        let rest = (GRID / 2 - NEAR_RINGS) as f32;
        let step = (i - NEAR_RINGS) as f32;
        NEAR_FIELD * (EXTENT / NEAR_FIELD).powf(step / rest)
    }
}

fn grade(i: usize) -> f32 {
    let centre = (GRID / 2) as i32;
    let k = i as i32 - centre;
    half(k.unsigned_abs() as usize) * (k.signum() as f32)
}

/// Spacing to the wider neighbour, per axis; the coarser axis decides.
fn spacing(i: usize) -> f32 {
    let lo = grade(i.saturating_sub(1));
    let hi = grade((i + 1).min(GRID - 1));
    let span = hi - lo;
    let steps = ((i + 1).min(GRID - 1) - i.saturating_sub(1)).max(1) as f32;
    span / steps
}

/// The shortest wave the *drawn* sea carries at this offset from the boat, metres.
///
/// What the surface is, as opposed to what the water is. The mesh cannot animate
/// anything finer than four samples across it, so `update_sea` asks the water for the
/// field filtered to this — and anything that has to sit on that surface has to ask for
/// the same filtering or it will sit somewhere else. Near the boat the difference is
/// the short chop, a few centimetres, which is exactly enough to put a mark of foam
/// under the skin and leave it blinking there as the mesh slides over it.
pub fn drawn_resolution(offset: Vec2) -> f64 {
    let ring = |d: f32| -> usize {
        // Monotone, so a short walk out from the boat finds it. Bounded by the grid.
        let mut lo = 0usize;
        let mut hi = GRID / 2;
        while lo < hi {
            let mid = (lo + hi) / 2;
            if half(mid) < d {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo
    };
    let centre = GRID / 2;
    let ix = centre + ring(offset.x.abs());
    let iz = centre + ring(offset.y.abs());
    4.0 * spacing(ix.min(GRID - 1)).max(spacing(iz.min(GRID - 1))) as f64
}

/// Vertices across the grid. Odd, so one sits under the boat.
const GRID: usize = 201;
/// How far out the grid reaches. Three kilometres is past the horizon from a cockpit,
/// and it costs almost nothing because the outer rings carry no waves at all — see
/// `resolutions`.
const EXTENT: f32 = 3000.0;
/// Where the wave-carrying part of the grid ends and the flat run to the horizon
/// begins, metres.
const NEAR_FIELD: f32 = 209.0;
/// Vertex spacing at the boat, and at the edge of the near field.
const SPACING_NEAR: f32 = 1.30;
const SPACING_FAR: f32 = 4.20;
/// How many rings make up the near field; the rest sprint to the horizon.
const NEAR_RINGS: usize = 76;

#[derive(Component)]
pub struct SeaSurface {
    mesh: Handle<Mesh>,
    /// Grid offsets in the Bevy horizontal plane, metres, precomputed once.
    offsets: Vec<Vec2>,
    /// Shortest wavelength this vertex can carry: twice its spacing from its
    /// neighbours, by Nyquist. Precomputed, because the grid never changes shape.
    ///
    /// The grid is graded — a vertex every 0.7 m under the boat, every 25 m at the
    /// rim — which is the right way to spend a hundred and sixty squared vertices.
    /// But a 4.5 second sea has a 32 metre wavelength, and 32 metres sampled every 25
    /// is not a wave, it is noise that crawls as the grid slides along underneath the
    /// boat. That crawl was the shimmer in the distance. Asking the water only for
    /// what each vertex can actually hold removes it at the source.
    resolutions: Vec<f64>,
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    // Sun, low enough to put some glitter on the water.
    commands.spawn((
        DirectionalLight {
            // Real sunlight, now that the camera is exposed for it. Thirty-two
            // thousand lux is a cloudy afternoon, and the scene was lit for one.
            illuminance: 85_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        // **Shadows had a reach of a hundred and fifty metres, which is the boat and
        // nothing else.** That is Bevy's default and it is the right one for a scene
        // the size of a room; here it meant the islands neither cast a shadow nor
        // received one, so a headland with the sun behind it was lit exactly like a
        // headland with the sun on it. A hill with no dark side does not read as a
        // hill — it reads as a cut-out, which is what the far shore has always looked
        // like in this game.
        //
        // Two and a half kilometres instead, which covers the islands you are actually
        // sailing among. The cost is resolution: four cascades spread over that much
        // ground are coarser than four over a hundred and fifty metres, so the near
        // bound is pushed out to thirty — the boat's own shadows get the whole first
        // cascade to themselves and stay sharp, and the distant ones can afford to be
        // soft because a cliff's shadow a mile off has no edge to speak of anyway.
        CascadeShadowConfigBuilder {
            num_cascades: 4,
            minimum_distance: 0.1,
            maximum_distance: 2_500.0,
            first_cascade_far_bound: 30.0,
            overlap_proportion: 0.2,
        }
        .build(),
        // Pointed from the one definition of where the sun is, so the drawn disc and
        // the light cannot drift apart.
        // Pointed from the one definition of where the sun is, and followed every frame
        // by `sky::light` so the drawn disc and the light cannot drift apart.
        Transform::default().looking_to(-sky::Sky::default().sun, Vec3::Y),
    ));
    // Much dimmer than before: the environment map now supplies the sky's own
    // contribution properly, and leaving the old flat ambient term in place on top of
    // it just washes the whole scene out.
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.62, 0.74, 0.88),
        brightness: 300.0,
        ..default()
    });

    // Water is a dielectric, not a metal. `metallic: 1.0` was simply wrong: it tints
    // the specular with the base colour and removes the diffuse term entirely, which
    // is how you render chrome. Water has an F0 of about two percent — Bevy maps
    // reflectance 0.5 to four percent, so 0.25 — and Fresnel takes that to a near
    // mirror at grazing angles on its own. That rise towards the horizon is most of
    // what the eye reads as water, and it only appears once there is an environment
    // map to reflect.
    //
    // White base colour because the water's own colour is carried per vertex, which
    // is what lets a crest reach foam without overdriving a dark base past one.
    // The ripple animation, baked here so the first frame is ready before anything is
    // drawn. `SAIL_WATER_FRAMES=0` turns it off and leaves the still map, which is both
    // an escape hatch and the way to see what the motion is actually buying.
    let still = std::env::var("SAIL_WATER_FRAMES").is_ok_and(|v| v == "0");
    let mut keys: Vec<Vec<u8>> = Vec::new();
    let mut live = sea_texture::normal_map();
    if !still {
        let started = std::time::Instant::now();
        let frames = sea_texture::normal_map_frames();
        let bytes: usize = frames
            .first()
            .and_then(|f| f.data.as_ref())
            .map_or(0, Vec::len);
        keys = frames
            .into_iter()
            .filter_map(|f| f.data)
            .filter(|d| d.len() == bytes && bytes > 0)
            .collect();
        info!(
            "water: {} ripple frames over {:.1} s at {:.0} fps, {:.1} MB of key frames, \
             baked in {:.2} s",
            keys.len(),
            sea_texture::RIPPLE_LOOP,
            keys.len() as f32 / sea_texture::RIPPLE_LOOP,
            (keys.len() * bytes) as f32 / 1.0e6,
            started.elapsed().as_secs_f32()
        );
        // The live image starts as the first key frame, mip chain and all, so the very
        // first frame drawn is already the right shape.
        live = sea_texture::normal_map_at_size(sea_texture::ANIM_SIZE, 0.0);
        // And it has to keep its bytes on this side of the fence. Every other texture
        // here is `RENDER_WORLD` — built once, uploaded, and the processor's copy
        // dropped, which is right for something that never changes. This one is written
        // every frame, and with that usage Bevy had thrown the data away by the second
        // frame, so the blend wrote nothing and the ripples stood still on frame zero
        // while the log cheerfully reported thirty-six frames at twelve a second.
        live.asset_usage = bevy::asset::RenderAssetUsages::MAIN_WORLD
            | bevy::asset::RenderAssetUsages::RENDER_WORLD;
    }
    let first = images.add(live);
    let water = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        normal_map_texture: Some(first.clone()),
        metallic_roughness_texture: Some(images.add(sea_texture::roughness_map())),
        perceptual_roughness: 1.0,
        metallic: 0.0,
        reflectance: 0.25,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    commands.insert_resource(Hatching(images.add(sea_texture::hatch_map())));
    commands.insert_resource(SeaFilm {
        keys,
        live: Some(first),
        blended: None,
        writes: 0,
        reported: 0.0,
    });

    // Spacing chosen from what the sea is, not from a convenient curve.
    //
    // A twelve-knot sea peaks around a thirty-two metre wavelength, and a *moving*
    // surface needs eight to ten samples across a wave before the crests stop beating
    // between the vertices as the grid slides along under the boat. The old cubic
    // grading held eight samples out to fifty-one metres and nothing like it beyond,
    // which is why the sea kept jumping however much else was fixed: two samples per
    // wave is Nyquist, and Nyquist is the condition for *reconstructing* a signal, not
    // for animating one.
    //
    // So: spacing opens gently from 1.3 m at the boat to 4.2 m at two hundred metres,
    // which holds eight samples out to a hundred and eighty, and then the last rings
    // sprint to three kilometres. Those outer rings cost almost nothing, because they
    // are too coarse to carry a wave and `update_sea` does not even ask the water
    // about them.
    let mut offsets = Vec::with_capacity(GRID * GRID);
    let mut resolutions = Vec::with_capacity(GRID * GRID);
    for ix in 0..GRID {
        for iz in 0..GRID {
            offsets.push(Vec2::new(grade(ix), grade(iz)));
            let coarsest = spacing(ix).max(spacing(iz));
            // Four times the spacing, not twice. Twice is the Nyquist limit, at which
            // a component is kept at full strength with four samples across it — and
            // four samples is where a moving sea visibly beats. This rolls a component
            // off between four and eight samples, so nothing is drawn that the mesh
            // cannot animate smoothly.
            resolutions.push(4.0 * coarsest as f64);
        }
    }

    let mut indices = Vec::with_capacity((GRID - 1) * (GRID - 1) * 6);
    for ix in 0..GRID - 1 {
        for iz in 0..GRID - 1 {
            let a = (ix * GRID + iz) as u32;
            let b = ((ix + 1) * GRID + iz) as u32;
            let c = ((ix + 1) * GRID + iz + 1) as u32;
            let d = (ix * GRID + iz + 1) as u32;
            indices.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    let count = offsets.len();
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![Vec3::ZERO; count]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![Vec3::Y; count]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![Vec2::ZERO; count]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, vec![Vec4::X; count]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0_f32, 1.0, 1.0, 1.0]; count]);
    mesh.insert_indices(Indices::U32(indices));
    let handle = meshes.add(mesh);

    commands.spawn((
        SeaSurface {
            mesh: handle.clone(),
            offsets,
            resolutions,
        },
        Mesh3d(handle),
        MeshMaterial3d(water),
        Transform::default(),
    ));
}

/// Displace the surface from the physics' own wave field, and keep it under the boat.
/// Where the surface detail has drifted to, integrated frame by frame.
#[derive(Resource, Default)]
pub struct SurfaceDrift {
    offset: glam::DVec2,
}

/// How much of the wind ends up as a current in the top of the water.
///
/// Two and a half percent, which is the old rule of thumb and is in `update_sea` as the
/// thing that carries the texture along. Public so the chart can draw the set and drift
/// from the same number the water is moved by, rather than from a second estimate of it:
/// a chart that disagrees with the sea about which way it is running is worse than a
/// chart with nothing on it.
pub const DRIFT_OF_WIND: f64 = 0.025;

/// The colour of water too far off to carry any detail.
///
/// Public because the horizon is drawn as a line now, and a line along the top of the
/// sea has to be the sea's own colour darkened rather than a second dark blue chosen
/// next to it. If the water's colour is ever changed, the line has to come with it.
pub const DEEP: Vec3 = Vec3::new(0.035, 0.095, 0.150);

/// The drawn sea's own two ends, before the weather is allowed to move them.
///
/// Prussian blue, which is not a decorative choice: it is *the* pigment of the picture
/// everybody means by a drawn wave. Hokusai had Berlin blue for about a decade before
/// the Great Wave and used it for exactly this — a blue that holds its hue at full
/// strength instead of going grey, so a flat field of it reads as water rather than as
/// shadow. The sea here was mixed from the sky instead, which gave it a green-cyan cast
/// at two-thirds the blue of the real pigment: a lagoon, not a print.
///
/// The light end is the paper, which in a woodblock is not white but a warm off-cream,
/// and the contrast between the two ends is what makes flat tone read as form.
const PRUSSIAN: Vec3 = Vec3::new(0.012, 0.062, 0.260);
const PAPER: Vec3 = Vec3::new(0.780, 0.800, 0.775);

/// Where the water is thin: over a bank, and in the back of a wave.
///
/// A print has more than two ends to its range and this one had two, which is why the
/// drawn sea read as flat however many bands were cut into it: four tones spread between
/// a mid blue and a pale blue are four mid blues. The dark end goes down towards the
/// pigment at full strength and the light end up towards the paper, and *between* them
/// the water is allowed to be a different colour rather than a mixture of the two —
/// because that is what water does. Thin water is not pale deep water. It is green,
/// because what you are seeing is light that went down, hit something and came back
/// rather than light that went down and did not return.
///
/// Two things make the water thin and they are the same thing at two scales. There is
/// the bank underneath — a metre over sand is a different colour from forty metres over
/// nothing, and the edge where one becomes the other is the single most useful mark on
/// any chart drawn by eye. And there is the wave itself: the back of a crest is a lens
/// of water with daylight coming through it from the other side, which is why a sea
/// looks lit from within along its crests and why that is where a painter puts the green.
const SHOAL: Vec3 = Vec3::new(0.090, 0.330, 0.300);

/// The key line, struck on the dark side of each tone step.
///
/// A woodblock print is a *key block* — the drawing in line — with colour blocks
/// registered behind it, so every edge in the picture carries a dark line on one side.
/// The sea had only the white one, and the argument for white still holds on the side
/// it was written for: it marks the crest, where water is bright, and dark there reads
/// as a crack in the surface. The side it does not hold for is the other one, where the
/// form turns away and a print has its heaviest line.
const KEY_LINE: Vec3 = Vec3::new(0.012, 0.030, 0.075);

/// How far apart the streaks run on the water, as a fraction of the wave height.
///
/// Because the strokes are contours of the surface now. A fixed bearing laid them in
/// rows across the sea, which is hatching — shading laid *over* a form. What the print
/// does is draw lines that follow the form itself, round the shoulder of a wave and up
/// its face, and that is the whole difference between a shaded photograph and a drawing.
///
/// Iso-height lines are exactly those lines, and they come free: the height is already
/// known at every vertex, so making it the texture coordinate turns the stroke pattern
/// into a contour map of the sea. They crowd where the water is steep and open out where
/// it is flat, which is tonal shading by line density arrived at from the geometry
/// rather than imposed on it — and the card's mipmapping handles the crowding, so the
/// steepest water merges to a solid tone instead of aliasing.
///
/// A seventh of the significant height, so a wave carries about seven streaks from
/// trough to crest whether it is a ripple or a gale.
const STREAK_OF_HEIGHT: f32 = 0.14;

/// How fast the streaks run up the back of a wave and down its face, bands a second.
///
/// Iso-height lines standing still are a contour map, and a contour map of a moving sea
/// is still a map: the water goes through the lines and the lines do not move, so
/// nothing on the surface is *going* anywhere. What a wave actually does to the water in
/// it is the orbit — a parcel rises up the back, goes forward over the top, falls down
/// the face and comes back through the trough — so the water climbs through the
/// contours on the windward side and falls through them on the lee side.
///
/// Which is a thing you can see from a deck and the reason a wave reads as moving rather
/// than as a shape being carried along. So the streaks are given that motion: the
/// pattern's phase is shifted up on the back and down on the front, signed by which way
/// the surface is tilted with respect to the wind and strongest where it is steepest.
/// Both sides of a crest shear against each other, which piles the lines up along it —
/// correctly, because that is where the water is thrown together.
const STREAK_RUN: f32 = 0.55;

/// How far from the boat cloud shadows are drawn on the water, metres.
///
/// **Raised from 1900 when visibility stopped being 2600.** The old number was honest
/// against the old haze: with the far water fading out at two and a half kilometres
/// there was nothing past two for a shadow to fall on. On a clear day you can now see
/// ten kilometres of sea, and cloud shadow that stops dead at nineteen hundred metres
/// draws a circle on the water with the boat at its centre — the one artefact that
/// says "this is a patch of geometry around you" louder than the hard edge the haze
/// was added to hide.
///
/// Five kilometres is the compromise. The eye is two metres above the water, so past
/// about three the surface is foreshortened into the horizon and a shadow out there is
/// a line rather than a patch — which means the far half of this range buys very
/// little, and buys it cheaply: the field is rasterised into a fixed grid, so a longer
/// reach costs resolution rather than time. At five kilometres the grid is 64 nodes
/// across 10 km, which is 157 m a node against a cloud shadow a few hundred metres
/// wide — coarse, and still finer than the thing it is drawing.
const SHADOW_REACH: f32 = 5000.0;

/// How many tones the drawn sea is allowed.
///
/// Four. Three is a poster and six is a photograph with a fault in it; four is what an
/// inker uses for water, and it is enough to carry a wave's shape and not enough to
/// suggest a gradient.
const INK_BANDS: usize = 4;

/// How wide the contour line on the water is, as a fraction of a tone band.
///
/// The sea gets no silhouette, because it has nothing to be a silhouette against — it
/// is what everything else is drawn against. What a drawn sea has instead is a contour:
/// the line along the top of a wave, which in this style is not a separate thing at all.
/// It is the edge between two tones. The bands are already here, so the line is drawn
/// by darkening the water that sits just on the shallow side of each step, and it lands
/// exactly where an inker would put it — along the crest and around the shoulder —
/// because that is where the tone changes.
const CONTOUR: f32 = 0.13;

/// How strongly the contour is struck, towards white.
///
/// White, not dark. The boat is drawn with dark lines because a dark line separates a
/// solid object from what is behind it, but water has nothing behind it and no edge to
/// separate — what the line is marking is the crest, and a crest is where the water is
/// bright. Drawn dark it reads as a crack in the surface; drawn white it reads as the
/// top of a wave, which is what every painter of water has done with it.
const CONTOUR_INK: f32 = 0.6;

/// How nearly a facet must mirror the sun to be struck as a highlight.
///
/// The cosine of the angle between the surface normal and the half-vector between eye
/// and sun — which is the mirror condition, so the glitter path is where it actually
/// falls rather than where it would be convenient. Tuned by counting: at 0.985 a
/// fifteen-knot sea lights about a tenth of a per cent of the resolved vertices, which
/// is a path and not a sheet.
const GLINT_COS: f32 = 0.985;

/// Fraction of eligible facets that are actually struck.
///
/// Not every facet on the path gets a mark. A continuous band of white is a lens flare;
/// what a sea does — and what an inker draws — is a scatter of separate dashes. The
/// choice is a hash of the water parcel, so each dash belongs to a piece of water and
/// rides the wave through instead of flickering as the grid slides underneath.
const GLINT_STIPPLE: f32 = 0.42;

/// How much of a tone band is the step between it and the one below, 0 to 1.
///
/// The drawn sea shimmered, and this is why. A tone band was a `floor`: a vertex whose
/// tone crept across a boundary changed colour by a *whole band* between one frame and
/// the next, and since there are forty thousand of them and the water under them never
/// stops moving, some were always crossing. Thirty per cent of the brightness range,
/// flipped, sixty times a second, scattered over the surface.
///
/// The fault was already known in this file and already fixed once, a dozen lines
/// further down, where the hatching softens its own stroke edge "or the lines crawl with
/// the vertex grid instead of lying still on the water". The reasoning was never carried
/// back to the bands, the contour or the foam, which are the same hard threshold on the
/// same moving grid, and between them they cover the whole sea rather than a stroke.
///
/// So the staircase keeps its treads and grows a riser. Eighty-four per cent of each
/// band is dead flat, which is what makes the drawing a drawing; the remaining sixth is
/// a smooth climb to the next tone, so a vertex crossing it moves instead of jumping.
/// The worst a hundredth of tone can now do is two hundredths of a band, against a third
/// of the whole range before.
const INK_RISE: f32 = 0.16;

/// The drawn sea's tone, and how near this vertex is to a step in it.
///
/// Returns the quantised tone in 0..1 and the strength of the contour line, which is
/// the same quantity read differently: the line marks where the tone changes, so it is
/// brightest on the riser and gone by the middle of a tread.
///
/// The line straddles the step rather than sitting above it, which is the one visible
/// change. One-sided it could not be continuous — it would have to appear at full
/// strength the instant a vertex crossed, which is the flicker again, one band narrower.
fn ink_tone(tone: f32) -> (f32, f32, f32) {
    let bands = INK_BANDS as f32;
    let x = (tone * bands).clamp(0.0, bands);
    let f = x - x.floor();
    // Flat, then a climb, and the climb lands exactly where the next band's floor
    // begins — so the two agree at the join and the staircase is continuous.
    let level = x.floor() + smoothstep(0.0, INK_RISE, f) - 1.0;
    let tone = (level / (INK_BANDS - 1) as f32).clamp(0.0, 1.0);
    // Distance to the nearest step, the short way round, and which side of it this
    // vertex fell. The step is at the bottom of a band, so the first half of a band is
    // the light side of the step just climbed and the second half is the dark side of
    // the one coming.
    let to_step = f.min(1.0 - f);
    let side = if f < 0.5 { 1.0 } else { -1.0 };
    (tone, 1.0 - smoothstep(0.0, CONTOUR, to_step), side)
}

/// Stable per-parcel hash, used to decide which facets catch the light.
fn stipple(parcel: glam::DVec2) -> f32 {
    let x = (parcel.x * 0.37).floor() as i64;
    let y = (parcel.y * 0.37).floor() as i64;
    let mut h = (x.wrapping_mul(0x27d4_eb2d) ^ y.wrapping_mul(0x1656_67b1)) as u64;
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^= h >> 29;
    (h >> 40) as f32 / 16_777_216.0
}

/// The ripple animation: one normal map per frame of the loop, and the material they
/// are shown on.
///
/// Pre-baked rather than regenerated. Making the map afresh every frame would be the
/// obvious way and it is the wrong one twice over: half a megabyte of texture rebuilt
/// and re-uploaded sixty times a second is real work on both the processor and the bus,
/// and it is exactly the kind of per-frame graphics-resource churn that a long run
/// punishes. There is an unexplained stall in this application's history that has that
/// shape, and this is not the moment to add another candidate. Baked frames are a fixed
/// cost paid once at startup, and after that showing one is a pointer swap.
#[derive(Resource, Default)]
pub struct SeaFilm {
    /// The key frames, as bytes on the processor rather than textures on the card.
    ///
    /// They are never bound to anything now: what the material carries is one `live`
    /// image that is written every frame as the blend of two of these. So the card
    /// holds *one* ripple map instead of thirty-six, which is 37 MB of video memory
    /// handed back, and the processor holds the key frames instead.
    keys: Vec<Vec<u8>>,
    /// The image the material actually samples. The material itself no longer needs
    /// holding: the handle never changes now, only the bytes behind it.
    live: Option<Handle<Image>>,
    /// Which pair was last written, so an unchanged blend is not written again.
    blended: Option<(usize, u16, u16)>,
    /// Blends written since the last report, and when that was.
    writes: u32,
    reported: f32,
}

/// Show the frame the clock is on.
///
/// The material is only touched when the frame actually changes — twelve times a second
/// rather than sixty — because writing a material marks it changed and its bind group is
/// rebuilt, and there is no reason to pay that for a frame that is already up.
/// How strong the drawn ripples are, against the wind that makes them, in knots.
///
/// **A ripple is a wind product and the map had one strength for every day.** Under nine
/// knots with half a metre of sea the water came out as a field of hard bright specks —
/// reported from the deck as wanting smoothing, and quite right: that is the texture of
/// a fresh breeze painted on a morning that has not got one. Capillary ripples are the
/// first thing the wind raises and the first thing it stops raising, so the map is faded
/// towards flat as the breeze drops and the water goes glassy.
const RIPPLES_FROM: f32 = 4.0;
const RIPPLES_FULL: f32 = 20.0;
/// And what is left of them in a calm: not nothing, because a glassy sea still has the
/// oiliest of swell texture on it, and a mirror-flat sea reads as a bug.
const RIPPLES_LEAST: f32 = 0.28;

pub fn roll_sea_film(
    time: Res<Time>,
    sim: Res<Sim>,
    mut film: ResMut<SeaFilm>,
    mut images: ResMut<Assets<Image>>,
) {
    if film.keys.len() < 2 {
        return;
    }
    let count = film.keys.len();
    let phase = (time.elapsed_secs() / sea_texture::RIPPLE_LOOP).fract();
    let phase = (phase + 1.0).fract() * count as f32;
    let i = phase.floor() as usize % count;
    let f = phase - phase.floor();

    // Quantised to a few hundred steps, which is the only reason this is cheap: a blend
    // that has not visibly changed is not written, so standing still costs nothing and
    // the texture is rewritten at most a few times a frame's worth of motion.
    let tws = sail_core::units::ms_to_kn(sim.sim.telemetry.tws) as f32;
    let strength =
        RIPPLES_LEAST + (1.0 - RIPPLES_LEAST) * smoothstep(RIPPLES_FROM, RIPPLES_FULL, tws);
    // Quantised like the blend, and for the same reason: the wind wanders a tenth of a
    // knot a second and nobody can see a hundredth of a ripple.
    let strength = (strength * 16.0).round() / 16.0;
    let step = (f * BLEND_STEPS as f32) as u16;
    if film.blended == Some((i, step, (strength * 16.0) as u16)) {
        return;
    }
    film.blended = Some((i, step, (strength * 16.0) as u16));
    let f = step as f32 / BLEND_STEPS as f32;

    let Some(handle) = film.live.clone() else {
        return;
    };
    let Some(mut target) = images.get_mut(&handle) else {
        return;
    };
    let Some(out) = target.data.as_mut() else {
        // Loudly, once. A missing buffer here means the image is not keeping its
        // processor-side copy, and the symptom is water that has simply stopped — which
        // is indistinguishable from a tuning choice until somebody says so.
        warn!("water: the live ripple map has no data on this side; animation is dead");
        film.keys.clear();
        return;
    };
    let a = &film.keys[i];
    let b = &film.keys[(i + 1) % count];
    if out.len() != a.len() {
        return;
    }
    // Thirty-six key frames over three seconds is twelve a second, and twelve a second
    // steps visibly: the whole ripple pattern jumps five times out of every six display
    // frames. Reported from the deck as the water clattering.
    //
    // So the frames become key frames and what the card samples is their blend. The
    // whole buffer is interpolated, *mip levels included*, and that is the part that
    // makes it affordable: box filtering is linear, so the mip chain of a blend is the
    // blend of the mip chains, and there is nothing to rebuild. One linear pass over a
    // megabyte and a half, no shader, and the motion is continuous rather than
    // thirty-six positions.
    let mix = (f * 256.0) as u32;
    // Towards flat by the same pass. A tangent-space normal map is flat at (128, 128,
    // 255) — straight up out of the surface — so fading the ripples is a lerp towards
    // that, and because it is linear it goes through the mip chain with everything else
    // and costs one more multiply a texel.
    let keep = (strength * 256.0) as u32;
    for (n, ((o, &x), &y)) in out.iter_mut().zip(a.iter()).zip(b.iter()).enumerate() {
        let blend = ((x as u32) * (256 - mix) + (y as u32) * mix) >> 8;
        let flat = match n % 4 {
            0 | 1 => 128,
            2 => 255,
            _ => 255,
        };
        *o = ((blend * keep + flat * (256 - keep)) >> 8) as u8;
    }

    // A pulse, so a stopped animation says so instead of looking like a decision. The
    // first version of this wrote into an image Bevy had already taken the bytes away
    // from, so the water froze on frame zero while the startup line went on reporting
    // thirty-six frames at twelve a second — a log that described the intention and not
    // the result, which is the same fault as a diagnostic that reads a constant.
    let sample: u32 = out.iter().take(32).map(|&b| b as u32).sum();
    film.writes += 1;
    let now = time.elapsed_secs();
    if now - film.reported > 2.0 {
        film.reported = now;
        debug!(
            "water: {} blends in {:.0} s, first texels sum {sample}",
            film.writes, 2.0
        );
        film.writes = 0;
    }
}

/// How finely the blend between two key frames is stepped.
///
/// Not continuous, because the texture is only rewritten when the number changes and a
/// truly continuous blend would rewrite it every single frame whether the water had
/// moved or not. Sixty-four steps between key frames is 768 a second against the twelve
/// it had — far past anything the eye resolves, and it means a boat sitting still in a
/// calm does no work at all.
const BLEND_STEPS: u16 = 64;

/// Whether the sea is drawn rather than lit.
#[derive(Resource, Default)]
pub struct Ink(pub bool);

/// The hatching, held so the sea's material can be given it and taken off it.
#[derive(Resource)]
pub struct Hatching(pub Handle<Image>);

impl Ink {
    /// `SAIL_INK=1` starts in the drawn sea, the same way `SAIL_PIXELS` starts in a
    /// chosen resolution: a look you have settled on should not need a keypress every
    /// time the boat is launched.
    pub fn from_env() -> Self {
        Self(std::env::var("SAIL_INK").is_ok_and(|v| v != "0"))
    }
}

// The ink drawing had a key — `B` — and the key is gone. The keys on this boat should be
// things you do to the boat: trim, steer, navigate, look. Which of two pictures the sea
// is drawn in is a preference, and a preference on a key is a line in the help that
// everybody reads past while looking for something they need; the list had three of them.
//
// Nothing else had to change when it went, which is the part worth recording: `apply_ink`
// has always watched the *flag* rather than the keypress, so the settings panel was
// already the other way in.

/// How polished the water is, which is the wind's business and not a constant.
///
/// Roughness in a microfacet model *is* the spread of the small slopes — the capillary
/// ripple the wind raises and nothing else. Pinned at one, the water answered a nine
/// knot morning with the specular of a gale: a hard bright speck on every facet, which
/// is the thing that wanted smoothing. Light air is nearer a mirror, which is why a calm
/// shows the sky and a long soft path to the sun instead of a field of sparks.
pub fn polish(
    ink: Res<Ink>,
    sim: Res<Sim>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    q: Query<&MeshMaterial3d<StandardMaterial>, With<SeaSurface>>,
) {
    if ink.0 {
        return;
    }
    let tws = sail_core::units::ms_to_kn(sim.sim.telemetry.tws) as f32;
    let want = GLASSY + (CHOPPY - GLASSY) * smoothstep(RIPPLES_FROM, RIPPLES_FULL, tws);
    for handle in &q {
        if let Some(m) = materials.get_mut(&handle.0).as_deref_mut() {
            if (m.perceptual_roughness - want).abs() > 0.01 {
                m.perceptual_roughness = want;
            }
        }
    }
}

/// The two ends of it: glassy in a calm, fully rough in a blow.
const GLASSY: f32 = 0.46;
const CHOPPY: f32 = 1.0;

/// Take the lighting off the water, or put it back, whenever the flag moves.
pub fn apply_ink(
    ink: Res<Ink>,
    hatching: Res<Hatching>,
    film: Res<SeaFilm>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    q: Query<&MeshMaterial3d<StandardMaterial>, With<SeaSurface>>,
) {
    if !ink.is_changed() {
        return;
    }
    // The lighting has to be taken off the water as well, or the bands are drowned in
    // a specular sheen that has no edges in it — flat tones and a glossy highlight are
    // two different pictures and the sheen wins.
    for handle in &q {
        if let Some(m) = materials.get_mut(&handle.0).as_deref_mut() {
            if ink.0 {
                m.perceptual_roughness = 1.0;
                m.reflectance = 0.02;
                // The strokes go on the base colour, which is free: what the ripple
                // film has always been is a *normal* map. And it comes off, because the
                // two want the same UVs and cannot both have them — in the drawn sea
                // the coordinates address the hatching, and a normal map read through
                // them would be ripples sampled along a stroke. No loss: the drawn sea
                // has had its lighting taken off it two lines above, and ripple detail
                // is the one thing flat tones are a deliberate refusal of.
                m.base_color_texture = Some(hatching.0.clone());
                m.normal_map_texture = None;
            } else {
                m.perceptual_roughness = 1.0;
                m.reflectance = 0.25;
                m.base_color_texture = None;
                m.normal_map_texture = film.live.clone();
            }
        }
    }
    info!("sea: {}", if ink.0 { "drawn" } else { "lit" });
}

// A Bevy system's parameters are its dependency list, not an argument list; the eighth
// one is the camera, and the alternative is a struct that exists only to satisfy a lint.
#[allow(clippy::too_many_arguments)]
pub fn update_sea(
    ink: Res<Ink>,
    sky_state: Res<sky::Sky>,
    dome: Res<sky::Dome>,
    clouds: Option<Res<crate::clouds::Clouds>>,
    origin: Res<frame::RenderOrigin>,
    time: Res<Time>,
    mut drift: ResMut<SurfaceDrift>,
    sim: Res<Sim>,
    mut meshes: ResMut<Assets<Mesh>>,
    chase: Res<camera::ChaseState>,
    mut q: Query<(&SeaSurface, &mut Transform)>,
    mut spent: ResMut<crate::world::Spent>,
) {
    let clock = crate::world::Spent::wanted().then(std::time::Instant::now);
    let ink = ink.0;
    let sun = sky_state.sun.normalize();
    let st = &sim.render_state();
    let centre = origin.to_bevy(st.pos, 0.0);
    let t = st.time;

    for (sea, mut transform) in &mut q {
        transform.translation = Vec3::new(centre.x, 0.0, centre.z);
        // The eye in the sea's own frame: the grid's vertices are offsets from the
        // boat, and a mirror condition has to be evaluated in one frame or the other,
        // not half in each.
        let eye = chase.eye() - transform.translation;
        let Some(mut mesh) = meshes.get_mut(&sea.mesh) else {
            continue;
        };

        let count = sea.offsets.len();
        let mut positions = Vec::with_capacity(count);
        let mut normals = Vec::with_capacity(count);
        let mut uvs = Vec::with_capacity(count);
        let mut tangents = Vec::with_capacity(count);
        let mut colours = Vec::with_capacity(count);

        let hs = (sim.sim.telemetry.significant_height as f32).max(0.05);
        // The sea's two ends, worked out once a frame rather than forty thousand times,
        // and — the point of hoisting them — available to the water *beyond* the near
        // field as well. That water took the lit palette whatever the sea was drawn in:
        // a band more than twice as dark as the drawn water inshore of it, running from
        // a quarter of a kilometre out to the horizon, which is most of the sea there
        // is. One palette now, and both halves of the grid read it.
        let body = Vec3::new(0.02, 0.07, 0.06);
        // **The sea's two ends come from the sky that is actually up there.**
        //
        // They were `ZENITH` and `HORIZON`, two constants: a fine blue afternoon, drawn
        // the same at seven in the morning, at noon and under a squall, with the sun
        // wherever it liked. Water is a mirror and this is what it mirrors — a trough
        // sees the narrow strip of sky overhead and a crest sees the whole dome, so the
        // dark end is the zenith and the light end the horizon, as before, but both of
        // them now move with the hour and the cover.
        //
        // The horizon end is also a *direction*: looking towards a low sun the water is
        // a sheet of light and the same water over your shoulder is nearly black, which
        // is most of what the sea looks like at the ends of the day. `Dome` carries that
        // as a constant plus a bearing term; it is applied per vertex below.
        // Only the dark end is worked out once a frame now: the light end is a bearing
        // and belongs at the vertex.
        let deep = if ink {
            // Mixed towards the pigment rather than from the sky, but not all the way:
            // the weather still has to be able to put a grey afternoon on the water,
            // and a print that never changes with the light is wallpaper.
            PRUSSIAN.lerp(dome.zenith() * 0.34 + body, 0.30)
        } else {
            dome.zenith() * 0.16 + body * 0.6
        };
        let far = if ink { deep } else { DEEP };
        // The bank, where there is one to see. Only asked for in the drawn sea, where
        // it is a tone the picture is built out of; the lit sea gets its shallows from
        // the light instead.
        let land = ink.then(|| sim.sim.land.clone()).flatten();
        let typical = (sim.sim.water.characteristic_slope() as f32).max(1e-4);
        // **Whether this sea breaks at all**, which is a property of the whole spectrum
        // and not of the facet under a vertex. The same significant steepness the spray
        // is judged by, from the same two numbers and the same thresholds — one
        // definition, so the white water on the mesh and the white water in the air
        // agree about what the day is doing.
        //
        // Without it the foam term was relative all the way down: it asked only whether
        // *this* facet was steeper than usual, so a glassy nine-knot morning came out
        // stippled with pale green crests exactly as a gale did. Reported from the deck
        // as wanting smoothing in those conditions, and this is the half of it that is
        // in the mesh.
        let whitecapping = {
            let period = sim.sim.water.peak_period_or(6.0) as f32;
            let wavelength = 9.81 * period * period / std::f32::consts::TAU;
            smoothstep(
                crate::spray::PLUNGES_ABOVE as f32,
                crate::spray::PLUNGES_HARD as f32,
                hs / wavelength.max(1.0),
            )
        };
        // Steady surface drift: about two and a half percent of the wind speed,
        // downwind, which is the usual rule of thumb. This is a real current and it
        // accumulates honestly with time.
        //
        // What it replaces was a bug worth remembering: the instantaneous orbital
        // velocity multiplied by elapsed time. The velocity oscillates with the wave
        // period and the elapsed time only grows, so the detail slid back and forth in
        // every direction with an amplitude that increased without bound — which from
        // the deck reads exactly like the boat periodically going backwards.
        let tm = &sim.sim.telemetry;
        let downwind = glam::DVec2::new(
            (tm.twd + std::f64::consts::PI).sin(),
            (tm.twd + std::f64::consts::PI).cos(),
        );
        // Two motions, both integrated rather than evaluated.
        //
        // The slow one is a real surface current, about two and a half percent of the
        // wind speed. The fast one is the ripples' own travel: the normal map stands
        // for short waves a few metres long, and those move through the water at
        // their own phase speed of nearly 3 m/s. Advecting at the drift rate alone is
        // what made the surface read as a photograph of a sea rather than a sea.
        //
        // `offset += direction * rate * dt`, and the `dt` is the whole point. Written
        // the obvious way — `direction * rate * elapsed` — the offset is a vector
        // whose length grows without bound multiplied by a direction that wanders
        // with every gust, so a one-degree windshift five minutes in swings an
        // eight-hundred-metre vector and the texture jumps fifteen metres sideways.
        // That is the same mistake as the very first texture bug in this file, and
        // raising the ripple speed tenfold made it ten times worse. Integrating the
        // rate means a windshift changes where the water is going from now on,
        // instead of rewriting where it has been.
        // The current, and only the current.
        //
        // This used to carry a ripple speed as well — the group velocity of a one-metre
        // wave, about 0.6 m/s — because the texture was a still photograph and sliding
        // it was the only motion available. It was always a lie of a particular kind:
        // sliding moves every scale in the pattern at one speed and in one direction,
        // and water does the opposite. Short ripples crawl, long ones run, and the
        // pattern reforms instead of marching. The ripples travel inside the frames
        // now, each at its own dispersion speed, so the texture coordinates are left to
        // carry what genuinely is a bulk translation: the wind-driven surface drift, two
        // and a half percent of the wind speed.
        let speed = tm.tws * DRIFT_OF_WIND;
        drift.offset += downwind * speed * time.delta_secs_f64();
        let current = drift.offset;
        // Rebase the texture coordinates by a whole number of tile repeats.
        //
        // The UVs address the sea in world metres so that the detail stays with the
        // water rather than with the boat. That is right, and it is also the thing
        // that breaks first on a long passage: twenty minutes out, the numbers are in
        // the hundreds, the GPU interpolates them in single precision across each
        // triangle, and the quantisation reads as the surface detail stuttering and
        // racing. Taking off whole repeats costs nothing and cannot shift the pattern,
        // because a whole repeat is where the pattern comes back to itself. The
        // subtraction is identical for every vertex, so no triangle is torn.
        let tile = sea_texture::TILE_METRES as f64;
        let anchor = st.pos - current;
        let uv_base = glam::DVec2::new(
            (anchor.x / tile).round() * tile,
            (anchor.y / tile).round() * tile,
        );
        // Where every cloud shadow falls, worked out once for the frame rather than per
        // vertex. Forty thousand vertices times twenty clouds is a number that has to be
        // cheap in the inner loop and does not have to be cheap out here.
        let shadows = clouds
            .as_ref()
            .map(|c| c.shadows(st.pos, SHADOW_REACH as f64, sky_state.sun))
            .unwrap_or_else(crate::clouds::ShadowField::clear);

        // Past this spacing the mesh carries nothing, so the water is never asked.
        // On a grid that runs to the horizon that is most of the vertices, and it is
        // what pays for the fine spacing close in.
        let longest = sim.sim.water.longest_wavelength();
        for (offset, &min_wavelength) in sea.offsets.iter().zip(&sea.resolutions) {
            if min_wavelength >= longest {
                positions.push(Vec3::new(offset.x, 0.0, offset.y));
                normals.push(Vec3::Y);
                tangents.push(Vec4::new(1.0, 0.0, 0.0, -1.0));
                // The far water is nearly all horizon reflection — it is seen at a
                // grazing angle — so it takes the sky on its own bearing rather than one
                // flat colour for the whole ring. This is where a low sun shows most: the
                // bright road to the sun and the dark water either side of it are both
                // out there, beyond anything the near grid draws.
                let look = Vec3::new(offset.x - eye.x, 0.0, offset.y - eye.z);
                let from_sky = dome.towards(look);
                let lit = if ink {
                    far
                } else {
                    // A third of the way towards the sky it reflects and no further.
                    // At a grazing angle the sea really is close to the colour of the
                    // horizon, but drawn that way the horizon stops being a line: in an
                    // evening overcast the far water went the same pale grey as the sky
                    // above it and the two became one field with a boat in the middle.
                    far.lerp(from_sky * 0.17 + body * 1.4, 0.3)
                };
                let flat = st.pos + glam::DVec2::new(offset.x as f64, -offset.y as f64) - uv_base;
                uvs.push(if ink {
                    // Past the near field there is no wave to shade and nothing to
                    // hatch: the top row of the map is blank paper.
                    Vec2::ZERO
                } else {
                    sea_texture::ripple_uv(Vec2::new(flat.x as f32, flat.y as f32))
                });
                colours.push([lit.x, lit.y, lit.z, 1.0]);
                continue;
            }
            // Sample at the true world position. Bevy's -Z is sim north, so the
            // horizontal offset flips sign on the way across.
            let world = st.pos + glam::DVec2::new(offset.x as f64, -offset.y as f64);
            // Band-limited to what this vertex can carry — see `resolutions`.
            let water = sim.sim.water.surface_resolved(world, t, min_wavelength);
            let (lift, slope) = (water.height, water.slope);
            let height = lift as f32;
            let (sx, sz) = (slope.x as f32, -slope.y as f32);

            // **And it moves sideways as well as up**, which is the difference between
            // a sea and a corrugated sheet.
            //
            // Gerstner is a *parametrisation*: the parcel whose mean position is here is
            // drawn at here-plus-its-own-displacement, and that displacement is what
            // gathers water into the crests and spreads it out under the troughs. It is
            // the whole reason the module says Gerstner rather than a sum of sines —
            // crests sharper than troughs — and the drawn mesh was not doing it. Every
            // vertex sat on its grid point and moved only up and down, so the picture
            // was Airy: round humps of exactly the height the sea reports and none of
            // the shape. Reported from the deck as the sea going flat, with no breakers
            // and no rollers in it, which is precisely what a correct height and a wrong
            // shape look like.
            //
            // It costs nothing. The displacement was already being computed for the
            // texture coordinates, and this is what it was for.
            let swell = water.drift;
            let point = Vec3::new(offset.x + swell.x as f32, height, offset.y - swell.y as f32);
            let normal = Vec3::new(-sx, 1.0, -sz).normalize();
            positions.push(point);
            normals.push(normal);

            // World-locked UVs, so the detail stays with the sea rather than with the
            // boat: the grid recentres every frame and the texture must not.
            //
            // And the parcel is now simply *here*. While the vertex stood on its grid
            // point, the water it was showing was the parcel that had drifted to it from
            // upwave, so the coordinates had to be walked back by that displacement to
            // keep the ripples riding the wave instead of being dragged across it. Now
            // that the vertex goes where the parcel goes, its mean position is its own
            // grid point and the correction would be applying the drift twice.
            let parcel = world - current - uv_base;
            uvs.push(sea_texture::ripple_uv(Vec2::new(
                parcel.x as f32,
                parcel.y as f32,
            )));

            // Tangent along +U, which is world +X, lying in the surface. Handedness is
            // negative because cross(normal, tangent) points at -Z while V runs to +Z.
            let tangent = Vec3::new(1.0, sx, 0.0).normalize();
            tangents.push(Vec4::new(tangent.x, tangent.y, tangent.z, -1.0));

            // Colour per vertex: troughs darker, crests lighter, and foam where the
            // surface is steep enough to be breaking. That is the depth shading and
            // the whitecaps, without a shader having to be written for either.
            let lift = (height / hs).clamp(-1.0, 1.0);
            // The lit sea's own colours, and a lighter pair for the drawn one.
            //
            // A photograph of deep water really is nearly black, and the drawn sea
            // inherited that and then had hatching laid on top of it — two darkenings on
            // the same water, which put the bottom of the range somewhere no ink drawing
            // goes. A manga sea is a mid tone with white in it: the darkest thing on the
            // page is the *line*, not the field it is drawn on, and the field has to stay
            // light enough for a black stroke to show against it.
            // Both derived from the sky, because that is where a sea's colour comes
            // from: most of what you see looking down at water is the sky looking back
            // at you, plus the small amount of blue-green the water itself returns from
            // below. They were written out as constants beside the sky's own, which
            // meant a storm could roll in overhead and the sea underneath it stayed the
            // colour of a fine afternoon.
            //
            // A trough is lit by the narrow strip of sky it can see, which is mostly the
            // zenith, so it takes the zenith darkened; a crest sees the whole dome and
            // takes the horizon. The water's own body colour leans green, which is why
            // the two are not simply the sky scaled.
            let mut tone = (lift * 0.5 + 0.5).clamp(0.0, 1.0);
            // Cloud shadow. Taken off the tone *before* it is quantised, which is the
            // whole trick: in ink mode the shadow then lands on a tone boundary and
            // comes out as a flat patch with a hard contour round it, which is what a
            // painter does with a cloud shadow. Added afterwards it would be a grey
            // wash over the drawing.
            //
            // The patch is nowhere near under the cloud that makes it — this sun is
            // seventeen degrees up, so the shadow of a cell with its base at four
            // hundred metres falls thirteen hundred metres to leeward. That offset is
            // what reads as height, and getting it from the real sun angle is cheaper
            // than faking it.
            // Only near the boat, and this is as much an honesty as an economy. The
            // eye is two metres above the water: at three kilometres the surface is
            // foreshortened into the horizon and a shadow patch out there is a line, not
            // a patch. Asking twenty clouds about forty thousand vertices cost five per
            // cent of the frame to draw something you cannot see; faded out between one
            // and two kilometres it costs almost nothing and looks the same.
            let range = (offset.x * offset.x + offset.y * offset.y).sqrt();
            let near = 1.0 - crate::clouds::step_between(SHADOW_REACH * 0.58, SHADOW_REACH, range);
            let shade = if near > 0.0 {
                near * shadows.shade_at(world)
            } else {
                0.0
            };
            tone = (tone - 0.42 * shade).clamp(0.0, 1.0);
            let mut edge = 0.0;
            let mut side = 1.0;
            if ink {
                // Drawn water, not photographed water.
                //
                // The whole difference is that a painter does not shade a gradient; a
                // painter picks three or four tones and puts a hard edge between them,
                // and the eye reads the shape of that edge as the shape of the wave.
                // So the gradient is quantised, and the step is the drawing.
                //
                // It is the right place for it, too: this colour is already computed
                // per vertex on the processor every frame, so a style that would
                // normally want a shader costs one rounding here.
                let (banded, on_step, which) = ink_tone(tone);
                tone = banded;
                side = which;
                // How near a step this vertex sits, kept before the tone is flattened,
                // because afterwards the information is gone.
                edge = on_step;
            }
            // The light end on *this* bearing. A crest shows the sky it faces, and at
            // the ends of the day the sky it faces is a different colour every way you
            // look: a low sun lays a sheet of light on the water one way and leaves it
            // nearly black the other.
            let look = Vec3::new(offset.x - eye.x, 0.0, offset.y - eye.z);
            let from_sky = dome.towards(look);
            let shallow = if ink {
                PAPER.lerp(from_sky * 0.62 + body, 0.34)
            } else {
                from_sky * 0.17 + body * 1.4
            };
            let mut base = deep.lerp(shallow, tone);
            if ink {
                // How thin the water is here, from the two things that thin it.
                //
                // The wave's own lens first: a crest is water standing above the mean
                // with daylight coming through it from behind, and the deeper the sea
                // the less that is true of any given crest — so it is the elevation
                // *relative to the wave height*, which is the same number whether this
                // is a ripple or a gale.
                let lens = (height / (0.9 * hs)).clamp(0.0, 1.0);
                // And the bank underneath, which is the other scale of the same thing.
                // Ten metres is where a sand bottom starts to show through on a clear
                // day; below two it is all bottom.
                let over_sand = land.as_ref().map_or(0.0, |l| {
                    let d = l.depth(world) as f32;
                    1.0 - smoothstep(2.0, 11.0, d)
                });
                let thin = (lens * 0.55).max(over_sand);
                base = base.lerp(SHOAL, thin * thin);
            }
            // Foam judged against this sea's own typical slope rather than a fixed
            // number. A long swell and a short chop of the same height are nothing
            // alike in steepness, so a threshold tuned on one gives either no foam at
            // all — which is what was happening, since the fixed 0.16 sat almost at
            // this sea's maximum — or a sea of milk on the other.
            let steepness = (sx * sx + sz * sz).sqrt() / typical;
            let mut foam =
                ((steepness - 1.25) / 0.75).clamp(0.0, 1.0) * crest_weight(lift) * whitecapping;
            if ink {
                // Foam becomes ink: on or off, no half measures, because a drawn
                // whitecap is a shape with an outline rather than a soft bloom. And
                // thrown earlier, since a hard-edged mark has to be large enough to
                // read as a mark.
                // Over a narrow range rather than at a point: still a mark with an
                // edge to it at any one moment, and no longer a crest that switches
                // itself on and off as the sea breathes under the grid.
                //
                // And with a ragged edge to it. Foam thrown as a clean shape is the one
                // thing in the drawn sea that looked machined: what the print draws, and
                // what a breaking crest does, is fingers — the water tears along the top
                // and comes down in claws. A slow world-locked noise on the threshold
                // does it, and it travels with the water because it is read at the
                // parcel rather than at the vertex.
                let torn = sail_core::noise::two_octaves(parcel * 0.9, 0x5EA_F0A4) as f32;
                foam = smoothstep(0.15 + 0.07 * torn, 0.21 + 0.07 * torn, foam);
            }
            let mut colour = base.lerp(
                if ink {
                    Vec3::new(0.95, 0.97, 1.0)
                } else {
                    Vec3::new(0.82, 0.88, 0.92)
                },
                foam,
            );
            // --- Water you can see *into*, where it is thin --------------------
            //
            // **The one thing a painted sea never has.** A wave is a lens: at the crest
            // there is half a metre of water between you and the sky on the other side,
            // and with the sun behind it the whole top of the wave goes a light jade and
            // glows — which is why the sea looks like a liquid and a grey surface with
            // highlights on it looks like a floor. It is the first thing you notice from
            // a boat and the last thing anybody puts in a renderer.
            //
            // Asked for from the deck as transparency in the top layer, and this is that
            // — written as colour rather than as alpha, which is a deliberate choice and
            // not a shortcut. Making the surface itself translucent means sorting forty
            // thousand vertices against the spray, the foam, the hull and the rain every
            // frame, and buys nothing the eye can tell apart: what reads as *seeing into
            // the water* is the colour of the light that came through it, and that is a
            // number this loop already has everything to compute.
            //
            // Three things have to agree for it: thin water, a face turned towards you,
            // and the sun on the other side of it.
            // **And how much water there is to shine through.**
            //
            // The rest of this is relative — how near a crest, how steep against this
            // sea's own typical — so it fired exactly as hard on a thirty centimetre
            // morning as in a gale, and what that looks like is a jade stipple laid over
            // the whole surface. A wave is held up to the light when there is a wave:
            // half a metre and up, which is where a crest has a face thick enough to
            // carry colour and thin enough to pass any.
            let enough_sea = smoothstep(0.35, 1.0, hs);
            let thin = crest_weight(lift) * smoothstep(0.7, 1.8, steepness) * enough_sea;
            if thin > 0.01 && shade < 0.6 {
                let view = (eye - point).normalize_or_zero();
                // Light arriving through the wave rather than off it: the sun opposite
                // the eye, and the facet leaning towards the eye so there is water in
                // the way rather than beside it.
                let through = seen_through(sun, view, normal);
                // Jade, not cyan: it is the colour of a wave held up to the light, which
                // is the sea's own green with the blue taken out of it by the depth it
                // did *not* have to travel through.
                let lit = if ink {
                    // The drawn sea works in flat tones, so this is another tone rather
                    // than a gradient — which is what a painter does with the same
                    // observation.
                    Vec3::new(0.42, 0.74, 0.62)
                } else {
                    Vec3::new(0.34, 0.80, 0.68)
                };
                let how_much = (thin * through * (1.0 - foam)).clamp(0.0, 0.75);
                colour = colour.lerp(lit, how_much);
            }
            if ink {
                // The hatching's coordinates, now that this vertex knows how dark it
                // is. Across the stroke in world metres, so the strokes lie on the
                // water and ride its orbital motion with the ripples they replace;
                // down the tone, which is what picks the width of the stroke out of
                // the tonal art map. Written over the ripple coordinates pushed above,
                // because the tone is not known until here and a vertex gets one UV.
                // Which face of the wave this is, −1 on the lee side and +1 on the
                // windward, from the surface's own tilt along the wind. Through a
                // tanh so a steep face is fully one or the other and the crest and the
                // trough — where the tilt passes through nothing — are neither, which
                // is what keeps the pattern continuous across them.
                // `sz` is the *negated* north slope, because Bevy's -Z is north, while
                // `downwind` is in the simulation's frame where y is north. Dotting them
                // as they stand would have the north-going half of every wind backwards
                // — the streaks running down the back of a wave and up its face, which
                // is the one thing this is for.
                let tilt = (sx * downwind.x as f32 - sz * downwind.y as f32) / typical;
                let face = tilt.tanh();
                if let Some(uv) = uvs.last_mut() {
                    *uv = Vec2::new(
                        height / (STREAK_OF_HEIGHT * hs) + face * STREAK_RUN * t as f32,
                        1.0 - tone,
                    );
                }
            }
            if ink && edge > 0.0 {
                // The line along the step between two tones, struck as hard as the
                // vertex is near one — and struck in two colours, which is what a
                // woodblock does. White where the form turns towards the light, because
                // that edge is the crest and a crest is bright; the key block's own dark
                // line where it turns away, because that is where a print puts its
                // weight. One line in two halves, and the pair of them is the drawing.
                let (mark, strength) = if side > 0.0 {
                    (Vec3::new(0.97, 0.99, 1.0), CONTOUR_INK)
                } else {
                    (KEY_LINE, CONTOUR_INK * 0.85)
                };
                colour = colour.lerp(mark, strength * edge);
            }
            if ink {
                // Glints off the wave faces.
                //
                // The sheen was taken off the water to let the flat tones read, and
                // that took the sun off it as well — a drawn sea without highlights
                // looks like a painted floor. So they go back on deliberately, as
                // marks rather than as a sheen: struck where the facet actually
                // mirrors the sun towards the eye, and only on the facets the stipple
                // picks, which is how you get a scatter of dashes along the glitter
                // path instead of a continuous smear of white.
                //
                // Done here rather than by the renderer because this is the one place
                // that knows the true surface normal. The lit sea gets its highlights
                // from the material and does not want a second set fighting them.
                let view = (eye - point).normalize_or_zero();
                let half = (view + sun).normalize_or_zero();
                // Not in a cloud shadow: there is no sun in there to glint off, and
                // leaving the glitter path running through a shadow is the one thing
                // that would give the shadow away as paint.
                if shade < 0.45 && normal.dot(half) > GLINT_COS && stipple(parcel) < GLINT_STIPPLE {
                    colour = Vec3::new(1.0, 1.0, 0.97);
                }
            }
            colours.push([colour.x, colour.y, colour.z, 1.0]);
        }
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, tangents);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    }
    if let Some(clock) = clock {
        spent.sea += clock.elapsed().as_secs_f32() * 1000.0;
    }
}

/// How much of what you are looking at came *through* the water rather than off it.
///
/// **Three things have to agree**: the sun on the far side of the surface from the eye,
/// and the facet leaning towards the eye so that there is water in the way rather than
/// beside it. Looking down-sun at a wave you see reflection; looking up-sun through the
/// same wave you see the light that crossed half a metre of sea, and that is the jade a
/// crest goes when it is held up to the light.
///
/// Its own function because it is the one piece of this that can be checked without a
/// sea: the loop it lives in builds forty thousand vertices and cannot be asked a
/// question.
#[inline]
pub fn seen_through(sun: Vec3, view: Vec3, normal: Vec3) -> f32 {
    (-sun.dot(view)).max(0.0).powf(1.6) * normal.dot(view).max(0.0)
}

/// Foam only belongs near a crest. A steep trough is still green water.
#[inline]
fn crest_weight(lift: f32) -> f32 {
    // In deviations rather than in significant heights, which is the same correction the
    // spray's `CREST_TOP` needed and for the same reason: Hs is four times the rms, so a
    // threshold written as a fraction of Hs quietly moved when the sea stopped being a
    // root two too tall. Full weight at 1.2 deviations — the top tenth of the water,
    // which is where whitecaps are.
    ((lift + 0.10) / 0.40).clamp(0.0, 1.0)
}

/// Report how long a frame takes, and how evenly.
///
/// Guessing at a stutter is a way to spend an afternoon. A sea that "jumps" has two
/// completely different explanations — the surface being badly sampled, or the whole
/// application not making its frames — and they are told apart by a number, not by
/// looking. The worst frame in each second matters more than the mean: a steady
/// forty is smooth, and a forty that is sometimes ten is not.
/// What is growing, once a minute.
///
/// Added after a run froze at the two-hour mark. The stack said what it was not: the
/// main thread was inside `renderer_extract` waiting on the render thread while Metal
/// sat in `submitCommandBuffers`, so the simulation was still stepping and nothing in
/// this crate was deadlocked — the submission queue was the thing that had stopped
/// moving. That is the signature of a graphics resource that keeps being created and
/// never dropped, and it is also the signature of a driver having a bad afternoon. One
/// frozen process cannot tell those apart, and guessing at it would have meant
/// rewriting something that may well be innocent.
///
/// So this writes down the four counts that would separate them. If entities or assets
/// climb steadily across a long passage, the leak is ours and the log says which kind.
/// If they sit flat and the picture stops anyway, it is not ours, and that is worth
/// knowing before a day is spent looking.
#[allow(clippy::too_many_arguments)]
pub fn report_vitals(
    sim: Res<Sim>,
    rain: Res<crate::rain::Rain>,
    time: Res<Time>,
    meshes: Res<Assets<Mesh>>,
    images: Res<Assets<Image>>,
    materials: Res<Assets<StandardMaterial>>,
    entities: Query<()>,
    mut since: Local<f32>,
) {
    *since += time.delta_secs();
    if *since < 60.0 {
        return;
    }
    *since = 0.0;
    // The weather is in here because the weather now moves, and a frame-time report
    // without it cannot be read: the cost of the sea goes up with the sea, so "slow at
    // minute forty" means nothing unless you know what it was blowing at minute forty.
    let (building, turning) = sim.sim.wind.outlook();
    info!(
        "vitals at {:.0} min: {} entities, {} meshes, {} images, {} materials; \
         {:.0} kn, sea {:.2} m at {:.1} s, outlook {:+.1} kn/h {:+.0} deg/h, \
         {} squalls, rain {:.0}%",
        time.elapsed_secs() / 60.0,
        entities.iter().count(),
        meshes.len(),
        images.len(),
        materials.len(),
        sail_core::units::ms_to_kn(sim.sim.telemetry.tws),
        sim.sim.telemetry.significant_height,
        sim.sim.water.peak_period_or(0.0),
        sail_core::units::ms_to_kn(building),
        sail_core::units::deg(turning),
        sim.sim.wind.squalls().len(),
        rain.fall * 100.0,
    );
}

/// Where the frame goes, on the processor's side of it.
///
/// **A frame rate pinned to the display says nothing about headroom.** Every
/// measurement in this session came back at sixty on this machine, in a gale with the
/// sky overcast and the sea full of white water, which answers "is it fast enough here"
/// and not "what would give out first on a slower one". So the three things that build
/// geometry every frame time themselves: the sea mesh, the foam, and the cloud.
///
/// Each is wall clock inside the system, summed over a couple of seconds and reported as
/// milliseconds a frame against the budget — sixteen and a half at sixty, thirty-three at
/// thirty. It is on with `SAIL_FRAMES`, beside the frame-time report, because the two
/// questions are asked together.
#[derive(Resource, Default)]
pub struct Spent {
    pub sea: f32,
    pub foam: f32,
    pub cloud: f32,
    frames: u32,
    since: f32,
}

impl Spent {
    /// Whether anybody is asking.
    ///
    /// Read once at the top of each timed system, which is why the clock is an
    /// `Option<Instant>` there: nobody asking costs one environment lookup and no
    /// clock reads at all.
    pub fn wanted() -> bool {
        std::env::var("SAIL_FRAMES").is_ok()
    }
}

/// Say where it went, once every couple of seconds.
pub fn report_time_spent(time: Res<Time>, mut spent: ResMut<Spent>) {
    if !Spent::wanted() {
        return;
    }
    spent.frames += 1;
    spent.since += time.delta_secs();
    if spent.since < 2.0 {
        return;
    }
    let n = spent.frames.max(1) as f32;
    info!(
        "frame: sea {:.1} ms, foam {:.1} ms, cloud {:.1} ms a frame over {} frames",
        spent.sea / n,
        spent.foam / n,
        spent.cloud / n,
        spent.frames
    );
    *spent = Spent::default();
}

pub fn report_frame_times(time: Res<Time>, mut window: Local<(f32, u32, f32)>) {
    let dt = time.delta_secs();
    window.0 += dt;
    window.1 += 1;
    window.2 = window.2.max(dt);
    if window.0 >= 2.0 {
        // Only when something is actually wrong. Left chattering it buries the one
        // line that matters, and the whole value of this is that it answered "is the
        // sea badly sampled or is the application not making its frames" with a
        // number instead of an afternoon.
        if window.2 > 0.025 || std::env::var("SAIL_FRAMES").is_ok() {
            warn!(
                "slow frames: {:.0} per second mean, worst {:.1} ms",
                window.1 as f32 / window.0,
                window.2 * 1000.0
            );
        }
        *window = (0.0, 0, 0.0);
    }
}

/// What the drawn sea is allowed to do between one frame and the next.
#[cfg(test)]
mod drawn_sea {
    use super::{ink_tone, smoothstep, INK_BANDS};

    /// The shimmer, stated as a number: a vertex whose tone creeps may not change
    /// colour by a whole band because of it.
    ///
    /// This is the test the old code fails, and it fails it by a factor of a hundred.
    /// `(tone * 4.0).floor()` moves a third of the brightness range the instant its
    /// argument crosses an integer, and across forty thousand vertices of moving water
    /// some are always crossing — which is exactly what was reported from the deck as
    /// the drawn sea glittering where the lit one does not.
    #[test]
    fn a_creeping_tone_may_not_jump_a_band() {
        const STEP: f32 = 0.001;
        // The arithmetic, because I got it wrong by a factor of four first time and
        // the measurement said so. A smoothstep of width `INK_RISE` has a steepest
        // slope of 1.5/INK_RISE; the argument is the tone times the number of bands and
        // the result is divided by the bands less one, so the gain from input to output
        // is 1.5 * 4 / (0.16 * 3), which is twelve and a half. A thousandth of tone can
        // therefore move the drawn tone by 0.0125, and does.
        //
        // Which is the point rather than a disappointment: it is *bounded and
        // proportional*. Halve the creep and the flicker halves. A `floor` moves a
        // third of the whole range for an input change of any size at all, however
        // small, and no amount of slowing the water down would have quietened it.
        const ALLOWED: f32 = 0.02;
        let mut worst: f32 = 0.0;
        let mut x = 0.0_f32;
        while x < 1.0 {
            let (a, line_a, _) = ink_tone(x);
            let (b, line_b, _) = ink_tone(x + STEP);
            worst = worst.max((b - a).abs());
            // And the contour line with it, which was the second hard threshold: it
            // used to switch on at full strength the moment a vertex crossed.
            assert!(
                (line_b - line_a).abs() < 0.12,
                "the contour jumped {:.3} at tone {x:.3}",
                (line_b - line_a).abs()
            );
            x += STEP;
        }
        assert!(
            worst < ALLOWED,
            "a tone creeping by {STEP} moved the drawn tone by {worst:.4}, which is \
             a vertex flickering between two bands"
        );
        // The old way, for the record, so the number above is a comparison and not an
        // assertion about nothing.
        let floored =
            |t: f32| ((t * INK_BANDS as f32).floor() / (INK_BANDS - 1) as f32).clamp(0.0, 1.0);
        let jump = (floored(0.25 + STEP) - floored(0.25 - STEP)).abs();
        assert!(
            jump > 0.3,
            "the fault this replaces moved {jump:.2} of the range in one step"
        );
    }

    /// And it is still a drawing: most of the sea sits on one of four flat tones,
    /// because that is the whole of the style. A staircase with no treads is a ramp.
    #[test]
    fn the_treads_are_still_flat() {
        let mut flat = 0;
        let mut total = 0;
        let mut x = 0.0_f32;
        while x < 1.0 {
            let (a, _, _) = ink_tone(x);
            let (b, _, _) = ink_tone(x + 0.001);
            if (b - a).abs() < 1e-6 {
                flat += 1;
            }
            total += 1;
            x += 0.001;
        }
        let share = flat as f32 / total as f32;
        assert!(
            share > 0.75,
            "only {:.0}% of the tone range is a flat band, which is a gradient with \
             steps in it rather than a drawing",
            share * 100.0
        );
        // The treads are where they always were: four tones evenly spread, so the
        // picture is the same picture.
        for (i, at) in [0.20_f32, 0.45, 0.70, 0.95].iter().enumerate() {
            let (tone, _, _) = ink_tone(*at);
            let want = i as f32 / (INK_BANDS - 1) as f32;
            assert!(
                (tone - want).abs() < 1e-4,
                "band {i} sits at {tone:.3} and should be at {want:.3}"
            );
        }
    }

    #[test]
    fn smoothstep_is_the_usual_curve() {
        assert_eq!(smoothstep(0.0, 1.0, -1.0), 0.0);
        assert_eq!(smoothstep(0.0, 1.0, 2.0), 1.0);
        assert!((smoothstep(0.0, 1.0, 0.5) - 0.5).abs() < 1e-6);
    }
}

#[cfg(test)]
mod light_through_a_wave {
    use super::*;

    /// **Up-sun you see through a wave; down-sun you see off it.**
    ///
    /// The whole of the translucency is in that sentence, and it is the one part of it
    /// that can be asked a question without a sea in front of you.
    #[test]
    fn the_crest_only_glows_with_the_sun_behind_it() {
        // Looking north along the water at a crest leaning towards us.
        let view = Vec3::new(0.0, 0.35, -0.94).normalize();
        let facing = Vec3::new(0.0, 0.7, -0.71).normalize();
        // Sun low and beyond the wave, so the light has to cross the water to reach us:
        // `view` runs from the water *to the eye*, so a sun on the far side points the
        // other way along it. Getting that round the wrong way is the easiest mistake
        // here and this test made it first.
        let behind = Vec3::new(0.0, 0.22, 0.98).normalize();
        // And the same sun over our own shoulder.
        let over_us = Vec3::new(0.0, 0.22, -0.98).normalize();
        let lit = seen_through(behind, view, facing);
        let dark = seen_through(over_us, view, facing);
        assert!(
            lit > 0.5,
            "a crest with the sun behind it came out at {lit:.2}"
        );
        assert!(
            dark < 0.01,
            "a crest with the sun behind the *eye* glowed at {dark:.2}"
        );
        // A facet turned away gives nothing however the sun lies: there is no water in
        // the way, only water beside.
        let away = Vec3::new(0.0, 0.7, 0.71).normalize();
        assert!(seen_through(behind, view, away) < 0.01);
    }
}
