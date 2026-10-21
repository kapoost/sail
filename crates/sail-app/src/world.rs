//! Sea, sky and light.
//!
//! The surface is displaced from **the same wave field the physics uses**. That is not
//! an implementation detail: a boat riding one sea while the eye watches a different
//! one is worse than no waves at all, because every cue disagrees with every other.
//!
//! The mesh is a graded grid that follows the boat — about 0.7 m between vertices
//! underneath her, widening to 25 m at the horizon seven hundred metres out, so the
//! detail is where the eye is. It is built here, once, and never written again: the
//! displacement, the normals and the colour are all worked out per vertex on the
//! graphics card, from the spectrum this file hands over each frame. See `sea_card` for
//! what crosses, and `assets/sea_wave.wesl` for what is done with it.
//!
//! What stays on this side is everything that is the same for the whole sea: where the
//! boat is, what the sky looks like on each bearing, where the cloud shadows fall, how
//! deep the water is round here, and the spectrum itself with each phase worked out in
//! double precision — which is the one number a float cannot hold.
//!
//! Normals come from the field's analytic slope rather than from the triangles. That
//! is both cheaper than recomputing them and exactly right, and it means the lighting
//! shows the wave shape even where the mesh is too coarse to.

use bevy::asset::RenderAssetUsages;
use bevy::light::CascadeShadowConfigBuilder;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::storage::ShaderBuffer;
use bevy_egui::egui;

use crate::camera;
use crate::curve::smoothstep;
use crate::frame;
use crate::sea_card;
use crate::sea_texture;
use crate::sim::Sim;
use crate::sky;

/// The sea itself: the surface, the film of ripples on it, and what a frame cost.
///
/// What this module does in a frame, in one place — see `stages.rs` for why the
/// schedule is named sets rather than one long chain.
pub fn plugin(app: &mut App) {
    use crate::stages::{Build, Deck, Paint};
    app.init_resource::<Spent>();
    app.init_resource::<Frames>();
    app.init_resource::<Counter>();
    app.insert_resource(Ink::from_env());
    app.init_resource::<SeaFilm>();
    app.init_resource::<SurfaceDrift>();
    app.add_systems(Startup, setup.in_set(Build::Scene));
    app.add_systems(Update, report_vitals.in_set(Deck::Report));
    app.add_systems(Update, report_frame_times.in_set(Deck::Report));
    app.add_systems(Update, report_time_spent.in_set(Deck::Report));
    app.add_systems(Update, update_sea.in_set(Deck::Dress));
    app.add_systems(Update, apply_ink.in_set(Deck::Weather));
    app.add_systems(
        bevy_egui::EguiPrimaryContextPass,
        draw_counter.in_set(Paint::Instruments),
    );
    app.add_systems(
        Update,
        (bake_sea_film, roll_sea_film, polish)
            .chain()
            .in_set(Deck::Weather),
    );
}

/// How far the ring `i` stands from the boat, metres.
///
/// Free functions rather than closures inside the setup, because the spray has to ask
/// the same questions: a mark of foam sitting on a surface worked out one way, drawn
/// over a surface worked out another, is two descriptions of one thing and it blinks.
pub(crate) fn half(i: usize) -> f32 {
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
pub(crate) fn spacing(i: usize) -> f32 {
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
pub(crate) const GRID: usize = 201;
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

/// The sea's one entity.
///
/// It used to carry the grid with it — forty thousand offsets and the shortest wave each
/// of them could hold — because the processor walked that grid every frame. The grid is
/// in the mesh now, and the mesh is written once: the vertex shader reads a grid offset
/// out of the position slot and works out the rest. So what is left is a name to find
/// the entity by, which is all a marker ever is.
#[derive(Component)]
pub struct SeaSurface;

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut sea_materials: ResMut<Assets<sea_card::SeaMaterial>>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
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
    // **The ripple animation is baked while the boat sails, not before she launches.**
    //
    // Thirty-six frames of normal map is a fifth of a second across every core here —
    // and four to six seconds on one, which is what a browser has. Baked up front that
    // is a window that does not open; baked a few rows at a time, with a budget, it is
    // a sea that starts still and begins to move a second later, on any machine.
    //
    // `SAIL_WATER_FRAMES=0` turns it off and leaves the still map, which is both an
    // escape hatch and the way to see what the motion is actually buying.
    let still = crate::knob::var("SAIL_WATER_FRAMES").is_ok_and(|v| v == "0");
    // One frame of the loop to begin with, so the very first picture already has the
    // right shape of ripple on it rather than a flat sheet.
    let mut live = if still {
        sea_texture::normal_map()
    } else {
        sea_texture::normal_map_at_size(sea_texture::ANIM_SIZE, 0.0)
    };
    if !still {
        // The live image has to keep its bytes on this side of the fence. Every other
        // texture here is `RENDER_WORLD` — built once, uploaded, and the processor's
        // copy dropped, which is right for something that never changes. This one is
        // written every frame, and with that usage Bevy had thrown the data away by the
        // second frame, so the blend wrote nothing and the ripples stood still on frame
        // zero while the log cheerfully reported thirty-six frames at twelve a second.
        live.asset_usage = bevy::asset::RenderAssetUsages::MAIN_WORLD
            | bevy::asset::RenderAssetUsages::RENDER_WORLD;
    }
    let first = images.add(live);
    let water = StandardMaterial {
        base_color: Color::WHITE,
        normal_map_texture: Some(first.clone()),
        metallic_roughness_texture: Some(images.add(sea_texture::roughness_map())),
        perceptual_roughness: 1.0,
        metallic: 0.0,
        reflectance: 0.25,
        double_sided: true,
        cull_mode: None,
        ..default()
    };
    commands.insert_resource(Hatching(images.add(sea_texture::hatch_map())));
    commands.insert_resource(SeaFilm {
        keys: Vec::new(),
        live: Some(first),
        blended: None,
        writes: 0,
        reported: 0.0,
        baking: (!still).then(Baking::default),
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
    for ix in 0..GRID {
        for iz in 0..GRID {
            offsets.push(Vec2::new(grade(ix), grade(iz)));
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
    // **The mesh is written here and never again.**
    //
    // The position slot carries the grid offset: x and z are where this vertex stands
    // relative to the boat, and y is zero, because the height is what the vertex shader
    // is for. The other four attributes are not read at all — they are present because
    // their presence is what tells the renderer to give the fragment stage a normal, a
    // tangent, a texture coordinate and a colour, all four of which the shader writes
    // over. Two megabytes of them, uploaded once.
    let count = offsets.len();
    let flat: Vec<Vec3> = offsets.iter().map(|o| Vec3::new(o.x, 0.0, o.y)).collect();
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, flat);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![Vec3::Y; count]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![Vec2::ZERO; count]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, vec![Vec4::X; count]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0_f32, 1.0, 1.0, 1.0]; count]);
    mesh.insert_indices(Indices::U32(indices));
    let handle = meshes.add(mesh);

    // The grid as the card needs it: how far each ring stands from the boat, and the
    // spacing there. The shader finds a vertex's ring by walking this table, which is
    // the same walk `drawn_resolution` does for everything that has to sit on the
    // surface — so a mark of foam and the water under it are band-limited by one answer
    // rather than by two that nearly agree.
    let rings: Vec<f32> = (0..=GRID / 2).map(half).collect();
    let spacings: Vec<f32> = (0..=GRID / 2).map(|k| spacing(GRID / 2 + k)).collect();
    let card = sea_card::SeaCard::new(&rings, &spacings, water, &mut buffers, &mut sea_materials);

    commands.spawn((
        SeaSurface,
        Mesh3d(handle),
        MeshMaterial3d(card.material.clone()),
        Transform::default(),
        // **Said rather than computed**, because the mesh is a flat plane and the water
        // is not. Bevy works a bounding box out from the attributes it was given; those
        // put every vertex at y = 0, so in a gale the crests would stand outside the box
        // the renderer culls by. Thirty metres covers any sea this model builds.
        bevy::camera::primitives::Aabb::from_min_max(
            Vec3::new(-EXTENT, -30.0, -EXTENT),
            Vec3::new(EXTENT, 30.0, EXTENT),
        ),
    ));
    commands.insert_resource(card);
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
/// The light end — the paper, which in a woodblock is not white but a warm off-cream —
/// is a bearing rather than a number now, and lives with the rest of the drawing's dials
/// in `assets/sea_wave.wesl`. This end is here because it is mixed once a frame, from
/// the sky that is actually up there.
const PRUSSIAN: Vec3 = Vec3::new(0.012, 0.062, 0.260);

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
    /// How far through baking the film we are, until it is done.
    baking: Option<Baking>,
}

/// The frame being baked, and how much of it stands.
///
/// A frame at a time would be the obvious shape and it is the wrong one: on one thread
/// a single 512-square frame is sixty milliseconds, which is four dropped frames every
/// time, thirty-six times over. Rows with a budget cost the same in total and are paid
/// in instalments nobody can see.
#[derive(Default)]
struct Baking {
    frame: usize,
    row: u32,
    base: Vec<u8>,
    bytes: usize,
}

/// How long a frame is allowed to spend baking the film, milliseconds.
///
/// Four. On this machine the task pool gets through a frame and a half of the loop in
/// that time, so the film is complete in about a second; on one thread it is thirty
/// rows, and the film arrives in ten seconds instead of freezing the first ten. Either
/// way what is drawn meanwhile is the still map, which is a sea with ripples on it that
/// do not move — the thing this feature was added to improve, not a hole in the picture.
const BAKE_BUDGET_MS: f32 = 4.0;

/// Bake what this frame can afford of the ripple film.
pub fn bake_sea_film(time: Res<Time>, mut film: ResMut<SeaFilm>) {
    let Some(mut work) = film.baking.take() else {
        return;
    };
    let size = sea_texture::ANIM_SIZE;
    if work.base.is_empty() {
        work.base = vec![0u8; (size * size * 4) as usize];
    }
    let clock = bevy::platform::time::Instant::now();
    // At least one batch, however slow the machine: a budget that can be missed on the
    // first comparison is a budget that never bakes anything at all.
    let mut rows = 0;
    while work.row < size {
        let batch = 32.min(size - work.row);
        sea_texture::bake_rows(
            &mut work.base,
            size,
            sea_texture::frame_time(work.frame),
            work.row,
            batch,
        );
        work.row += batch;
        rows += batch;
        if clock.elapsed().as_secs_f32() * 1000.0 > BAKE_BUDGET_MS {
            break;
        }
    }
    let _ = rows;
    if work.row >= size {
        // A finished frame, with its mip chain, into the film.
        let done = sea_texture::mipped(std::mem::take(&mut work.base), size);
        if work.bytes == 0 {
            work.bytes = done.len();
        }
        if done.len() == work.bytes {
            film.keys.push(done);
        }
        work.frame += 1;
        work.row = 0;
        if work.frame >= sea_texture::RIPPLE_FRAMES {
            info!(
                "water: {} ripple frames over {:.1} s at {:.0} fps, {:.1} MB of key \
                 frames, complete {:.2} s in",
                film.keys.len(),
                sea_texture::RIPPLE_LOOP,
                film.keys.len() as f32 / sea_texture::RIPPLE_LOOP,
                (film.keys.len() * work.bytes) as f32 / 1.0e6,
                time.elapsed_secs()
            );
            return;
        }
    }
    film.baking = Some(work);
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
    // Not until the whole film is in. A part of it would still animate — the frames
    // arrive in order — but over a shorter loop, so the ripples would run fast and then
    // slow down as the rest arrived, which is a worse thing to watch than water that is
    // still for a second and then moves.
    if film.keys.len() < sea_texture::RIPPLE_FRAMES {
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
        Self(crate::knob::var("SAIL_INK").is_ok_and(|v| v != "0"))
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
    mut materials: ResMut<Assets<sea_card::SeaMaterial>>,
    q: Query<&MeshMaterial3d<sea_card::SeaMaterial>, With<SeaSurface>>,
) {
    if ink.0 {
        return;
    }
    let tws = sail_core::units::ms_to_kn(sim.sim.telemetry.tws) as f32;
    let want = GLASSY + (CHOPPY - GLASSY) * smoothstep(RIPPLES_FROM, RIPPLES_FULL, tws);
    for handle in &q {
        if let Some(m) = materials.get_mut(&handle.0).as_deref_mut() {
            if (m.base.perceptual_roughness - want).abs() > 0.01 {
                m.base.perceptual_roughness = want;
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
    mut materials: ResMut<Assets<sea_card::SeaMaterial>>,
    q: Query<&MeshMaterial3d<sea_card::SeaMaterial>, With<SeaSurface>>,
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
                m.base.perceptual_roughness = 1.0;
                m.base.reflectance = 0.02;
                // The strokes go on the base colour, which is free: what the ripple
                // film has always been is a *normal* map. And it comes off, because the
                // two want the same UVs and cannot both have them — in the drawn sea
                // the coordinates address the hatching, and a normal map read through
                // them would be ripples sampled along a stroke. No loss: the drawn sea
                // has had its lighting taken off it two lines above, and ripple detail
                // is the one thing flat tones are a deliberate refusal of.
                m.base.base_color_texture = Some(hatching.0.clone());
                m.base.normal_map_texture = None;
            } else {
                m.base.perceptual_roughness = 1.0;
                m.base.reflectance = 0.25;
                m.base.base_color_texture = None;
                m.base.normal_map_texture = film.live.clone();
            }
        }
    }
    info!("sea: {}", if ink.0 { "drawn" } else { "lit" });
}

// A Bevy system's parameters are its dependency list, not an argument list; the ninth
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
    chase: Res<camera::ChaseState>,
    mut card: ResMut<sea_card::SeaCard>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    mut q: Query<&mut Transform, With<SeaSurface>>,
    mut spent: ResMut<crate::world::Spent>,
) {
    let clock = crate::world::Spent::wanted().then(bevy::platform::time::Instant::now);
    let ink = ink.0;
    let sun = sky_state.sun.normalize();
    let st = &sim.render_state();
    let centre = origin.to_bevy(st.pos, 0.0);
    let t = st.time;

    for mut transform in &mut q {
        transform.translation = Vec3::new(centre.x, 0.0, centre.z);
        // The eye in the sea's own frame: the grid's vertices are offsets from the
        // boat, and a mirror condition has to be evaluated in one frame or the other,
        // not half in each.
        let eye = chase.eye() - transform.translation;

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
        // Only the dark end is worked out here: the light end is a *bearing*, which is
        // most of what the sea looks like at the ends of the day, and a bearing belongs
        // at the vertex. The card is handed the dome's three terms instead.
        let deep = if ink {
            // Mixed towards the pigment rather than from the sky, but not all the way:
            // the weather still has to be able to put a grey afternoon on the water,
            // and a print that never changes with the light is wallpaper.
            PRUSSIAN.lerp(dome.zenith() * 0.34 + body, 0.30)
        } else {
            dome.zenith() * 0.16 + body * 0.6
        };
        let far = if ink { deep } else { DEEP };
        let typical = (sim.sim.water.characteristic_slope() as f32).max(1e-4);
        // **Whether this sea breaks at all**, which is a property of the whole spectrum
        // and not of the facet under a vertex. The same significant steepness the spray
        // is judged by, from the same two numbers and the same thresholds — one
        // definition, so the white water on the mesh and the white water in the air
        // agree about what the day is doing.
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
        //
        // `offset += direction * rate * dt`, and the `dt` is the whole point. Written
        // the obvious way — `direction * rate * elapsed` — the offset is a vector whose
        // length grows without bound multiplied by a direction that wanders with every
        // gust, so a one-degree windshift five minutes in swings an eight-hundred-metre
        // vector and the texture jumps fifteen metres sideways.
        //
        // What it carries is a bulk translation and nothing else: the ripples travel
        // inside the baked frames now, each at its own dispersion speed, so the texture
        // coordinates are left with the wind-driven surface drift.
        let tm = &sim.sim.telemetry;
        let downwind = glam::DVec2::new(
            (tm.twd + std::f64::consts::PI).sin(),
            (tm.twd + std::f64::consts::PI).cos(),
        );
        let speed = tm.tws * DRIFT_OF_WIND;
        drift.offset += downwind * speed * time.delta_secs_f64();
        let current = drift.offset;
        // Rebase the texture coordinates by a whole number of tile repeats.
        //
        // The UVs address the sea in world metres so that the detail stays with the
        // water rather than with the boat. That is right, and it is also the thing that
        // breaks first on a long passage: twenty minutes out, the numbers are in the
        // hundreds, the card interpolates them in single precision across each triangle,
        // and the quantisation reads as the surface detail stuttering and racing. Taking
        // off whole repeats costs nothing and cannot shift the pattern, because a whole
        // repeat is where the pattern comes back to itself. The subtraction is identical
        // for every vertex, so no triangle is torn.
        let tile = sea_texture::TILE_METRES as f64;
        let anchor = st.pos - current;
        let uv_base = glam::DVec2::new(
            (anchor.x / tile).round() * tile,
            (anchor.y / tile).round() * tile,
        );
        // Where every cloud shadow falls, rasterised once for the frame. Forty thousand
        // vertices times twenty clouds is a number that has to be cheap in the inner
        // loop and does not have to be cheap out here — and now that the inner loop is
        // on the card, this map is also the only shape the card could read it from.
        let shadows = clouds
            .as_ref()
            .map(|c| c.shadows(st.pos, SHADOW_REACH as f64, sky_state.sun))
            .unwrap_or_else(crate::clouds::ShadowField::clear);

        // And over it goes. Eighteen kilobytes, against the one and a half megabytes of
        // vertex attributes this function used to write every frame.
        card.write(
            &sea_card::Scene {
                ink,
                eye,
                sun,
                deep,
                far,
                body,
                dome: &dome,
                boat: st.pos,
                time: t,
                uv_base,
                current,
                downwind,
                hs,
                typical,
                whitecapping,
                longest: sim.sim.water.longest_wavelength(),
                tile: sea_texture::TILE_METRES,
                shadow_reach: SHADOW_REACH,
                shadows: &shadows,
                land: sim.sim.land.as_ref(),
                water: sim.sim.water.as_ref(),
            },
            &mut buffers,
        );
    }
    if let Some(clock) = clock {
        spent.sea += clock.elapsed().as_secs_f32() * 1000.0;
    }
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
        crate::knob::var("SAIL_FRAMES").is_ok()
    }
}

/// Say where it went, once every couple of seconds.
///
/// Two decimals rather than one, since the sea left. A tenth of a millisecond was the
/// right resolution for a figure that ran to five; it is the whole figure now, and a
/// report that can only say "0.0" cannot tell a cost from a disappearance.
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
        "frame: sea {:.2} ms, foam {:.2} ms, cloud {:.2} ms a frame over {} frames",
        spent.sea / n,
        spent.foam / n,
        spent.cloud / n,
        spent.frames
    );
    *spent = Spent::default();
}

/// How the frames are going, as two numbers.
///
/// **A mean on its own is a reassuring lie.** Sixty a second with one frame of a hundred
/// and twenty milliseconds in it reads as sixty and feels like a stutter, and the stutter
/// is the thing worth finding — so the worst frame of the window is carried beside the
/// mean, which is also what the log has always printed.
///
/// One measurement, read by two things: the line in the log and the counter on the screen
/// come out of this, and the lesson behind that is written in half the comments in this
/// repository. Two copies of a measurement is two chances to disagree in front of
/// somebody trying to work out why their picture is slow.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct Frames {
    /// Frames a second, meaned over the window that has just closed.
    pub rate: f32,
    /// And the longest single frame in it, seconds.
    pub worst: f32,
    /// What is accumulating towards the next window.
    window: (f32, u32, f32),
}

impl Frames {
    /// How long a window is. Two seconds: long enough that the number is steady enough
    /// to read, short enough that trimming a sail and watching the counter answer is
    /// still one action.
    const WINDOW: f32 = 2.0;

    /// A frame happened. Returns the window if this closed one.
    pub fn frame(&mut self, dt: f32) -> Option<(f32, f32)> {
        self.window.0 += dt;
        self.window.1 += 1;
        self.window.2 = self.window.2.max(dt);
        if self.window.0 < Self::WINDOW {
            return None;
        }
        self.rate = self.window.1 as f32 / self.window.0;
        self.worst = self.window.2;
        self.window = (0.0, 0, 0.0);
        Some((self.rate, self.worst))
    }

    /// Whether the window that just closed had a frame in it that somebody would feel.
    ///
    /// Twenty-five milliseconds is forty a second: below that the helm starts to lag the
    /// hand, which on a boat you steer by feel is the first thing to go.
    pub fn stuttered(&self) -> bool {
        self.worst > 0.025
    }
}

/// Whether the counter is on the screen.
///
/// **Off unless asked for.** A number burning in the corner of a sailing picture is a
/// number you stop seeing in a minute and a thing in the way of the horizon for the rest
/// of the evening — and the one question it answers, *is this machine making its frames*,
/// is asked twice in the life of an installation. `SAIL_FPS=1` opens with it, the
/// PICTURE page turns it on and off, and the log says the same thing either way.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Counter(pub bool);

impl Default for Counter {
    fn default() -> Self {
        Self(crate::knob::var("SAIL_FPS").is_ok_and(|v| v != "0"))
    }
}

/// The counter, bottom left, where there is nothing else.
///
/// Two numbers because one of them lies: the mean, and the worst frame of the last two
/// seconds, which is the one that is felt. Amber when that worst frame is past what a
/// hand at the helm can ignore, grey when it is not — a colour rather than a second
/// number, because the question at a glance is *is it all right*.
pub fn draw_counter(
    mut contexts: bevy_egui::EguiContexts,
    counter: Res<Counter>,
    frames: Res<Frames>,
) -> Result {
    if !counter.0 {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let ink = if frames.stuttered() {
        egui::Color32::from_rgb(230, 198, 120)
    } else {
        egui::Color32::from_rgb(150, 150, 130)
    };
    egui::Area::new("frames".into())
        .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(12.0, -10.0))
        .interactable(false)
        .show(ctx, |ui| {
            // One line, however narrow egui decides the area is: a counter that wraps
            // onto two lines in the corner of the picture is worse than no counter.
            ui.add(
                egui::Label::new(
                    egui::RichText::new(format!(
                        "{:.0} fps · worst {:.0} ms",
                        frames.rate,
                        frames.worst * 1000.0
                    ))
                    .monospace()
                    .size(11.0)
                    .color(ink),
                )
                .wrap_mode(egui::TextWrapMode::Extend),
            );
        });
    Ok(())
}

pub fn report_frame_times(time: Res<Time>, mut frames: ResMut<Frames>) {
    let Some((rate, worst)) = frames.frame(time.delta_secs()) else {
        return;
    };
    // Only when something is actually wrong. Left chattering it buries the one
    // line that matters, and the whole value of this is that it answered "is the
    // sea badly sampled or is the application not making its frames" with a
    // number instead of an afternoon.
    if frames.stuttered() || crate::knob::var("SAIL_FRAMES").is_ok() {
        warn!(
            "slow frames: {rate:.0} per second mean, worst {:.1} ms",
            worst * 1000.0
        );
    }
}

// **The drawn sea's own tests have moved to the card with the code they test.**
//
// `ink_tone` and `seen_through` were here, with a test apiece: that a tone creeping
// across a band boundary cannot jump a whole band, and that a crest glows up-sun and not
// down-sun. Both are properties of the drawing rather than of the water, both still
// hold, and both are now asserted against the implementation that actually draws —
// `sea_card`'s tests compile the shader, run those two functions on the card and check
// the same two things. A property tested against a copy of the code is a property that
// can pass while the picture is wrong.

#[cfg(test)]
mod how_the_frames_are_going {
    use super::*;

    /// **The mean is over the window, not over a frame.** Sixty frames in a second and
    /// sixty in the next is sixty a second, however the deltas were distributed.
    #[test]
    fn a_steady_sixty_reads_as_sixty() {
        let mut frames = Frames::default();
        let mut closed = None;
        for _ in 0..180 {
            if let Some(window) = frames.frame(1.0 / 60.0) {
                closed = Some(window);
            }
        }
        let (rate, worst) = closed.expect("three seconds closed a two-second window");
        assert!((rate - 60.0).abs() < 1.0, "it read {rate:.1} a second");
        assert!((worst - 1.0 / 60.0).abs() < 1e-4, "worst was {worst:.4} s");
        assert!(!frames.stuttered());
    }

    /// **And the worst frame is the point.** A window with one long frame in it still
    /// means something near sixty, which is exactly the reassuring lie this second number
    /// exists to contradict.
    #[test]
    fn one_long_frame_shows_up_even_though_the_mean_does_not() {
        let mut frames = Frames::default();
        let mut closed = None;
        for n in 0..180 {
            let dt = if n == 90 { 0.120 } else { 1.0 / 60.0 };
            if let Some(window) = frames.frame(dt) {
                closed = Some(window);
            }
        }
        let (rate, worst) = closed.expect("a window");
        assert!(
            rate > 50.0,
            "the mean collapsed to {rate:.1}, so it is not a mean"
        );
        assert!(
            (worst - 0.120).abs() < 1e-4,
            "the hundred-and-twenty millisecond frame was lost: {worst:.4} s"
        );
    }

    /// The window closes at two seconds and starts again, rather than meaning the whole
    /// session — a counter that remembers a stutter from twenty minutes ago is a counter
    /// nobody can use to try something.
    #[test]
    fn the_window_starts_again() {
        let mut frames = Frames::default();
        for _ in 0..30 {
            frames.frame(0.1);
        }
        assert!(
            frames.stuttered(),
            "a tenth of a second a frame is a stutter"
        );
        // Two windows of fast frames. One is not enough and should not be: the slow
        // frames left over when the first window closed belong to the second one, and a
        // counter that threw them away would be reporting a window it had not finished.
        for _ in 0..480 {
            frames.frame(1.0 / 120.0);
        }
        assert!(
            !frames.stuttered(),
            "the old worst frame outlived the window it happened in"
        );
    }
}
