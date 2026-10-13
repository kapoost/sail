//! Foam off the hull.
//!
//! A boat moving through water makes a mess of it, and that mess is the best speedometer
//! on board. You feel six knots in your feet and you *see* it at the bow: the stem
//! splitting the surface, the lee bow throwing a sheet of white, the quarter wave
//! rolling away aft. Without it a hull at eight knots and a hull at two look the same
//! from the helm, which was the case here.
//!
//! Where it comes from, and why there are three sources rather than one:
//!
//! * **The stem.** The bow wave breaks at the waterline and throws foam forward and
//!   outward. It is the first thing to appear as the boat accelerates and the last to
//!   go, and it grows roughly as the cube of the speed, because that is how the energy
//!   going into wave-making grows.
//! * **The lee bow.** Heeled over, the lee bow is buried and shoulders the water aside,
//!   and in a seaway it slices into the back of every wave. That is where the sheets of
//!   spray come from and why they come in bursts rather than steadily.
//! * **The quarter.** Foam shed along the run, which falls into the wake the wake module
//!   is already drawing — this just hands it over fed rather than starting it cold.
//! * **The rudder.** A blade at an angle of attack sheds its boundary layer off the
//!   trailing edge, and at any speed worth having that is a visible trail of disturbed
//!   water a few metres astern. Put the helm hard over and the trail doubles; let it
//!   stall and it *ventilates* — the low pressure on the back of the blade pulls air
//!   down from the surface and the whole thing goes white, which is the sound and the
//!   sight of a boat being over-steered and the moment the helm goes light in your hand.
//! * **The keel.** The same thing on a bigger foil working at a smaller angle: the fin
//!   is making its side force at two or three degrees of leeway, and what surfaces is a
//!   thin pair of trails off the tip vortices, well aft and much fainter than the
//!   rudder's. Drawn weaker on purpose — a keel that foamed like a rudder would be a
//!   keel that had stalled, which is a different and much worse afternoon.
//!
//! Drawn deliberately heavy. A true photograph of a 36-footer at seven knots has less
//! white water in it than this, and at the resolution this is played at a true amount
//! would be a few grey pixels. Overdrawn foam is how every marine painter has ever
//! conveyed speed, for the same reason.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::frame;
use crate::sim::Sim;

/// How many specks of foam are alive at once.
///
/// Nine hundred, which is a great deal, and deliberately so. They cost one quad each
/// and nothing at all in the physics, and the alternative — a true photographic amount
/// of white water — is a few grey pixels at the resolution this is played at.
const COUNT: usize = 2800;
/// Seconds one lives.
const LIFE: f32 = 3.4;
/// How fast foam spreads outward from where it was thrown, m/s.
const SPREAD: f64 = 0.55;
/// How near the top of a wave a dart has to land, as a fraction of the significant
/// height. Elevation only — see the dart loop for why steepness is the wrong question.
const CREST_TOP: f64 = 0.42;

/// How many looks at the water a fully whitecapped sea gets a second.
const CREST_DARTS: f32 = 440.0;
/// Marks in one breaking crest, and how far they are scattered, metres.
const CREST_SPECKS: usize = 6;
const CREST_PATCH: f64 = 1.5;

/// How far out breaking crests are drawn, metres.
///
/// Ninety: far enough that the sea has whitecaps in it rather than a ring of them round
/// the boat, near enough that each speck is still more than one pixel. Past this the
/// shading on the water carries it, which is the right division of labour — a speck at
/// three hundred metres is a sub-pixel dot and the tone it would contribute is already
/// there.
const CREST_REACH: f64 = 90.0;

struct Speck {
    at: glam::DVec2,
    drift: glam::DVec2,
    age: f32,
    /// Varies, or a hundred identical blobs read as a texture rather than as water.
    scale: f32,
    /// Its own phase, so the shimmer is a thousand separate sparkles rather than one
    /// field of foam pulsing in time with itself.
    phase: f32,
    /// How blue this one is to begin with, 0 white and 1 the colour of the sea it came
    /// out of.
    tint: f32,
    /// Whether this one is foam torn off a breaking crest rather than spray thrown off
    /// the bow, which decides which mark it is drawn with — see `spray_marks`.
    claw: bool,
}

#[derive(Resource)]
pub struct Spray {
    mesh: Handle<Mesh>,
    live: Vec<Speck>,
    seed: sail_core::rng::Pcg32,
    /// Fractional specks carried between frames, so a birth rate of three and a half a
    /// second does not round down to three and lose its half for ever.
    owed: f32,
    /// The same, for darts thrown at the sea looking for a breaking crest.
    ///
    /// A separate budget, and that *is* the fix rather than a tidying. Sharing one with
    /// the hull's foam meant a dart that found flat water still spent a speck's worth of
    /// it — so with one crest in a hundred darts breaking, ninety-nine hundredths of the
    /// sea's entire allowance went on looking and three whitecaps a second got drawn.
    /// Measured: three a second, over two and a half hectares of water.
    darts: f32,
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    let vertices = COUNT * 4;
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![Vec3::ZERO; vertices]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![Vec3::Y; vertices]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![Vec2::ZERO; vertices]);
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_COLOR,
        vec![[1.0_f32, 1.0, 1.0, 0.0]; vertices],
    );
    let mut indices = Vec::with_capacity(COUNT * 6);
    for i in 0..COUNT {
        let a = (i * 4) as u32;
        indices.extend_from_slice(&[a, a + 1, a + 2, a, a + 2, a + 3]);
    }
    mesh.insert_indices(Indices::U32(indices));
    let handle = meshes.add(mesh);

    commands.spawn((
        Mesh3d(handle.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            // The shape of a mark, instead of the square it is drawn on. Additive, so
            // the sheet is read as light added and black is nothing — which is why it
            // needs no alpha channel and why the marks can overlap without the usual
            // business of sorting them.
            base_color_texture: Some(images.add(crate::sea_texture::spray_marks())),
            // Added rather than blended: foam is bright water, and two layers of it are
            // brighter than one. It also means the specks never darken the sea they are
            // drawn over, which a blended quad with any alpha at all will do.
            alpha_mode: AlphaMode::Add,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        })),
        Transform::default(),
    ));

    commands.insert_resource(Spray {
        mesh: handle,
        live: Vec::with_capacity(COUNT),
        seed: sail_core::rng::Pcg32::new(0xF0_A3_11),
        owed: 0.0,
        darts: 0.0,
    });
}

#[allow(clippy::too_many_arguments)]
pub fn update(
    origin: Res<frame::RenderOrigin>,
    ink: Res<crate::world::Ink>,
    sim: Res<Sim>,
    time: Res<Time>,
    mut spray: ResMut<Spray>,
    mut meshes: ResMut<Assets<Mesh>>,
    camera: Query<&Transform, With<crate::camera::MainCamera>>,
) {
    let Ok(view) = camera.single() else {
        return;
    };
    let st = &sim.render_state();
    let cfg = sim.config();
    let dt = time.delta_secs();
    let t = st.time;

    // Age and carry what is already in the water. Foam does not chase the boat: it is
    // left where it was made, spreading and dying, and the boat sails out from under it.
    let mut keep = Vec::with_capacity(COUNT);
    for mut s in spray.live.drain(..) {
        s.age += dt;
        s.at += s.drift * dt as f64;
        if s.age < LIFE {
            keep.push(s);
        }
    }
    spray.live = keep;

    // How much she is making. Wave-making energy goes as a high power of the speed, so
    // a boat loafing at three knots makes almost none and the same boat at seven is
    // throwing it everywhere — which is exactly the behaviour that makes this worth
    // drawing at all.
    let speed = sim.sim.telemetry.stw.max(0.0);
    let fraction = (speed / cfg.hull_speed()).clamp(0.0, 1.4);
    let heel = st.heel.abs();
    // Heeled, the lee bow is buried and makes far more of it.
    let pressed = 1.0 + 1.6 * (heel / sail_core::units::rad(30.0)).clamp(0.0, 1.5);
    let rate = 420.0 * fraction.powi(3) as f32 * pressed as f32;
    spray.owed += rate * dt;

    // And the weather, which until now made no spray at all unless the boat was going
    // fast — so a gale with her hove-to under a scrap of main was a dry afternoon. It is
    // the other way round: in bad weather most of the water in the air was never touched
    // by the hull. The wind tears the tops off the seas and carries them, and at the top
    // of force six it starts doing it in long streaks that are the classic sign of a
    // gale coming on. The rate goes as a high power of the wind because the threshold is
    // sharp: below about twenty-five knots the tops stay on.
    let tws_kn = sail_core::units::ms_to_kn(sim.sim.telemetry.tws).max(0.0) as f32;
    // Named apart from the `hs` further down, which is the same quantity floored
    // differently for a different job. Two bindings of one name in one function is how
    // a float quietly becomes the wrong float.
    let sea_height = sim.sim.telemetry.significant_height.max(0.0) as f32;
    let blown = ((tws_kn - 22.0) / 14.0).clamp(0.0, 2.2);
    spray.owed += 900.0 * blown * blown * blown * (0.35 + sea_height) * dt;

    let forward = st.forward();
    let starboard = st.starboard();
    let lee = -sim.sim.telemetry.awa.signum();

    // Riding a crest. The top of a wave is where the water is already breaking — it is
    // the steepest part of it and the part with the least support under it — so a hull
    // driving through one tears up far more than the same hull in the trough. It is also
    // the moment a sailor sees: she lifts, the whole length of her goes white, and then
    // she settles and it goes quiet again.
    //
    // Measured against the sea's own significant height rather than an absolute, so a
    // crest in a one-metre sea counts as a crest and a one-metre swell on a flat day
    // does not drown the boat in foam.
    let hs = sim.sim.telemetry.significant_height.max(0.05);
    let at_crest = ((sim.sim.water.height(st.pos, t) / hs - 0.2) / 0.6).clamp(0.0, 1.0);
    spray.owed += 320.0 * (at_crest * at_crest) as f32 * fraction as f32 * dt;

    // The rudder, and then the keel. Both are foils and the rate goes with the work they
    // are doing: the flow over them squared, times how hard they are turning it.
    //
    // The rudder's own factor is the helm angle, so she leaves almost nothing steering
    // straight and a proper trail through a tack. Stall multiplies it, because a stalled
    // blade is not shedding a boundary layer any more, it is pulling air down the back
    // of itself — and that is the one of these two a sailor is actually watching for.
    let bite = sim.sim.telemetry.rudder_bite as f32;
    let helm = (sim.controls.rudder.abs() / cfg.max_rudder_angle).clamp(0.0, 1.0) as f32;
    let stalled = sim.sim.telemetry.rudder_stalled as f32;
    let rudder_work = fraction as f32 * fraction as f32 * (helm * 0.8 + stalled * 1.5) * bite;
    spray.owed += 150.0 * rudder_work * dt;

    // The keel is a bigger foil at a smaller angle, so it is keyed to leeway rather than
    // to the helm — and it is deliberately a tenth of the rudder's, because a keel that
    // foams like a rudder is a keel that has let go.
    let leeway = (sim.sim.telemetry.leeway.abs() / sail_core::units::rad(6.0)).clamp(0.0, 1.5);
    let keel_work = fraction as f32 * fraction as f32 * leeway as f32;
    spray.owed += 26.0 * keel_work * dt;

    // And the stem cutting a wave, which is a different thing from a bow wave and looks
    // it: not a steady moustache but a burst, thrown when she drives her forefoot into
    // the back of the next one. It is the sound a boat makes as much as the sight, and
    // in a seaway it is most of the white water there is.
    //
    // Measured where the stem is: the water's own surface there against the hull's, with
    // her heave and pitch in it. Buried, she throws it; riding over, she does not.
    let stem = st.pos + forward * (cfg.loa * 0.48);
    let surface = sim.sim.water.height(stem, t);
    let stem_rides = st.heave + (cfg.loa * 0.48) * st.pitch.sin();
    let buried = (surface - stem_rides - 0.15).max(0.0);
    if buried > 0.0 && speed > 1.0 {
        spray.owed += (buried * 260.0 * fraction) as f32 * dt;
    }

    // How much of the budget goes to the open sea rather than to the hull, and how far
    // out to look. Both rise with the sea: in a flat calm there is nothing to break.
    let crest_share = ((hs - 0.35) * 0.55 + blown as f64 * 0.25).clamp(0.0, 0.72);
    // Where the tops are carried: downwind, which is the direction the flow goes rather
    // than the direction the wind is quoted from.
    let downwind = sail_core::wind::flow_from(sim.sim.telemetry.twd, 1.0);
    // Darts, not specks: this is how many *looks* at the water the sea gets this frame,
    // and only the ones that find a crest steep enough to break turn into anything.
    spray.darts += CREST_DARTS * crest_share as f32 * (1.0 + blown) * dt;

    // --- Breaking crests, anywhere in sight ------------------------------------
    //
    // What this draws is whitecaps, and they are the thing that tells you what the sea
    // is doing when there is nothing else out there to look at: a sea with no white in
    // it is one you can carry full sail in, and one going white all over is one you
    // should already have reefed for.
    //
    // Found by throwing darts rather than by walking a grid. A grid tied to the boat
    // makes a pattern that travels with her, which is the one thing a sea must never do;
    // scattered darts come out even and cost a fraction of the sweep. Each one asks how
    // steep the surface is where it landed, and steep *and* lifted is a crest about to
    // go — both conditions, or the sea foams in its hollows.
    let mut born_breaking = 0u32;
    let mut darts_thrown = 0u32;
    {
        let Spray {
            live, seed, darts, ..
        } = &mut *spray;
        while *darts >= 1.0 && live.len() < COUNT {
            *darts -= 1.0;
            darts_thrown += 1;
            let r = &mut *seed;
            let a = r.next_f64() * std::f64::consts::TAU;
            let range = CREST_REACH * r.next_f64().sqrt();
            let at = st.pos + glam::DVec2::new(a.cos(), a.sin()) * range;
            // Near the top of a wave, and that is the whole test.
            //
            // The first version asked for steep *and* lifted, which is very nearly a
            // contradiction and measured like one: twenty-three darts a frame finding
            // nought or one. On any wave the surface is steepest on the *face*, halfway
            // down, and flattest at the crest — the slope goes through zero at the apex
            // by definition. Asking for the steepest water at the highest point is
            // asking for the two places a wave does not have at once.
            //
            // What actually decides whether a crest breaks is not local at all: it is
            // the steepness of the *sea*, which is a property of the whole spectrum and
            // lives in the rate below rather than in the dart. So the dart's only job is
            // to find the top of a wave, and how often a top turns white is the weather's
            // business.
            let lifted = sim.sim.water.height(at, t) / hs.max(0.05);
            if lifted < CREST_TOP {
                continue;
            }
            // A handful of small ones, not one big one.
            //
            // These specks are drawn as plain quads — there is no texture on them, which
            // is right for foam at the hull where each is a pixel or two and the *field*
            // of them is the picture. Scale one of those up to two metres and you have
            // not drawn a whitecap, you have drawn a white square, which is what a
            // square looks like when it is big enough to see its corners.
            //
            // So a breaking crest is a cluster: half a dozen specks a third of a metre
            // across, scattered over a metre and a half of water. Same grammar as the
            // foam off the bow — many small marks, each twinkling on its own — at the
            // size the thing actually is.
            let blow = downwind * (0.5 + 1.5 * r.next_f64()) * blown.min(1.2) as f64;
            for _ in 0..CREST_SPECKS {
                if live.len() >= COUNT {
                    break;
                }
                let spread = r.next_f64() * std::f64::consts::TAU;
                let out = CREST_PATCH * r.next_f64().sqrt();
                live.push(Speck {
                    at: at + glam::DVec2::new(spread.cos(), spread.sin()) * out,
                    drift: blow + glam::DVec2::new(a.cos(), a.sin()) * 0.25,
                    age: 0.0,
                    scale: 2.0 + 2.6 * r.next_f64() as f32,
                    phase: r.next_f64() as f32 * std::f32::consts::TAU,
                    tint: 0.2 * r.next_f64() as f32,
                    claw: true,
                });
            }
            born_breaking += 1;
        }
    }

    let Spray {
        live, seed, owed, ..
    } = &mut *spray;
    while *owed >= 1.0 && live.len() < COUNT {
        *owed -= 1.0;
        let r = &mut *seed;
        // Two thirds at the stem, the rest along the lee bow and the quarter.
        // How wide she is at a station, as a fraction of her greatest half-beam. The
        // same fine ellipse the culling test uses, so foam is born exactly where the
        // planking is rather than at a fixed distance from the centreline that is
        // outside her at the bow and inside her amidships.
        let hull_at = |along: f64| (1.0 - (along * 2.0).powi(2)).max(0.0).sqrt();
        // Which source this one came from. The two foils are placed by *where they
        // are* rather than as a fraction of the hull, because that is the whole content
        // of the effect: a trail that starts at the transom is the rudder's and one
        // that starts under the keel is not.
        let foils = (rudder_work + keel_work) as f64;
        let where_ = r.next_f64() * (1.0 + foils.min(1.4));
        let (along, across) = if where_ > 1.0 {
            // On the centreline, at one foil or the other, a little aft of it: the
            // disturbed water surfaces behind the blade rather than over it.
            let rudder = r.next_f64() * (rudder_work + keel_work) as f64 <= rudder_work as f64;
            // Both measured abaft the *transom*, not from where the foil is, and that
            // is the whole of getting this right. Water disturbed by a blade a metre
            // under the counter does not appear beside the cockpit; it surfaces astern,
            // and the deeper the foil the further astern it gets there. The rudder's
            // trail breaks almost at once because the blade reaches nearly to the
            // surface; the keel's comes up two metres further back and in a pair,
            // because what rises is the tip vortices off either side of the fin.
            //
            // Placing them at the foil instead put every keel speck inside the hull,
            // where the waterplane cull ate it — a source that cost its arithmetic and
            // drew nothing.
            let astern = if rudder {
                0.2 + 1.1 * r.next_f64()
            } else {
                1.4 + 3.2 * r.next_f64()
            };
            let across = if rudder {
                (r.next_f64() - 0.5) * 0.3
            } else {
                (if r.next_f64() < 0.5 { -1.0 } else { 1.0 }) * (0.5 + 0.35 * r.next_f64())
            };
            ((-cfg.loa * 0.5 - astern) / cfg.loa, across)
        } else if where_ < 0.62 {
            // At the stem: a touch ahead of the bow, scattered either side of it.
            (0.46 + 0.04 * r.next_f64(), (r.next_f64() - 0.5) * 0.5)
        } else if where_ < 0.88 {
            // The lee bow, from the stem back to amidships, hard against the planking.
            //
            // These were pushed out to a beam and a half to keep them from appearing
            // inside the boat, which was the wrong cure: foam is made *at* the hull, by
            // the hull, and a band of white standing off the topsides with clear water
            // between is not something a boat does. The waterplane test does that job
            // properly now — the hull is narrow at the ends, so a speck three quarters
            // of the way out is already outside her up forward and gets culled where she
            // is wide amidships.
            let along = 0.12 + 0.3 * r.next_f64();
            (along, lee * hull_at(along) * (1.02 + 0.22 * r.next_f64()))
        } else {
            // The quarter, feeding the wake.
            let along = -0.3 - 0.15 * r.next_f64();
            (along, lee * hull_at(along) * (1.02 + 0.3 * r.next_f64()))
        };
        let at = st.pos + forward * (cfg.loa * along) + starboard * (cfg.beam * 0.5 * across);
        // Thrown outward and a little forward of where it was made, then left behind.
        let outward = if across.abs() < 1e-6 {
            starboard * (r.next_f64() - 0.5)
        } else {
            starboard * across.signum()
        };
        let drift =
            outward * SPREAD * (0.6 + 0.8 * r.next_f64()) + forward * speed * 0.12 * r.next_f64();
        live.push(Speck {
            at,
            drift,
            age: 0.0,
            scale: 0.55 + 0.9 * r.next_f64() as f32,
            phase: r.next_f64() as f32 * std::f32::consts::TAU,
            tint: r.next_f64() as f32,
            claw: false,
        });
    }
    *owed = owed.min(8.0);

    let Some(mut mesh) = meshes.get_mut(&spray.mesh) else {
        return;
    };
    let mut positions = Vec::with_capacity(COUNT * 4);
    let mut colours = Vec::with_capacity(COUNT * 4);
    let mut uvs = Vec::with_capacity(COUNT * 4);

    for i in 0..COUNT {
        let Some(s) = spray.live.get(i) else {
            // Unused specks collapse onto the boat, transparent — never onto the world
            // origin, which draws a sheet from the hull to wherever that happens to be.
            let here = origin.to_bevy(st.pos, 0.0);
            for _ in 0..4 {
                positions.push(here);
                colours.push([1.0, 1.0, 1.0, 0.0]);
                uvs.push(Vec2::ZERO);
            }
            continue;
        };
        // Nothing inside the boat. Foam blowing over the rail from outside is right and
        // is what a lee bow does in a seaway; foam rising through the deck from
        // underneath is a hull with no bottom in it. The specks drift, and a drift of
        // half a metre a second across a three-metre beam puts them inside her in a
        // couple of seconds — so the test is at drawing time, every frame, rather than
        // once at birth.
        let local = st.to_body(s.at - st.pos);
        let half = cfg.beam * 0.5;
        let ends = cfg.loa * 0.5;
        let inside = local.x.abs() < ends
            && local.y.abs() < half * (1.0 - (local.x / ends).powi(2)).max(0.0).sqrt();
        if inside {
            let here = origin.to_bevy(st.pos, 0.0);
            for _ in 0..4 {
                positions.push(here);
                colours.push([1.0, 1.0, 1.0, 0.0]);
                uvs.push(Vec2::ZERO);
            }
            continue;
        }
        let life = (s.age / LIFE).clamp(0.0, 1.0);
        // Thrown up bright, spreading and thinning as it settles. The brightness falls
        // faster than the size grows, which is what foam does: it goes from white to a
        // patch of bubbles to nothing, getting wider the whole time.
        // Pixels, not blobs. Each speck is a few centimetres across, so what reads is
        // the *texture* of broken water — hundreds of separate bright points — rather
        // than a handful of white splodges. At this resolution a speck is one or two
        // pixels, which is what foam is on a screen this size.
        // Smaller than they were, and smaller again for the fine ones. What reads as
        // broken water is the *number* of marks, not their size: at a couple of
        // centimetres a speck is a pixel or two, which is foam, and at five it is a
        // blob of paint. The claws keep their size, because a mark torn off a crest is
        // a handful of foam and genuinely is that big.
        let fine = if s.claw { 1.0 } else { 0.62 };
        let size = (0.055 + 0.085 * life) * s.scale * fine;
        // And every one of them twinkles. Real broken water does not glow steadily: a
        // bubble catches the sky, turns, and goes out, and what the eye picks up from a
        // bow wave is that seething rather than any single bright patch. Two beats, not
        // one, and both prime-ish so a thousand specks never fall into step.
        // Slower than it was. At seventeen beats a second a speck crosses every shade it
        // has between one frame and the next, which is the same thing as not having them
        // — the levels only read if the climb through them takes long enough to see.
        let twinkle = 0.62
            + 0.38
                * (0.6 * (s.age * 7.5 + s.phase).sin() + 0.4 * (s.age * 4.1 + s.phase * 2.1).sin());
        let alpha = (1.0 - life).powf(1.7) * 0.95 * twinkle.clamp(0.0, 1.4);
        // The surface this mark has to sit on is the one that is *drawn*, which is not
        // the one the water has. The mesh carries nothing finer than four samples
        // across it, so `update_sea` asks the field filtered to that — and this asked
        // the field itself, short chop and all. A few centimetres of disagreement, and
        // a few centimetres is the whole height a speck floats at: half the foam on the
        // sea spent its life just under the drawn skin, blinking in and out as the
        // triangles slid over it.
        //
        // One definition now. `world::drawn_resolution` is what the grid was built
        // from, and both the sea and the foam on it read it.
        let offset = s.at - st.pos;
        let resolution =
            crate::world::drawn_resolution(Vec2::new(offset.x as f32, -offset.y as f32));
        let surface = sim.sim.water.height_resolved(s.at, t, resolution);
        // And it comes *up through* the surface rather than being pinned to it. Foam
        // thrown by a breaking wave is under the water for a moment and then it is not;
        // what the eye reads is the arrival. A mark rises its own height over the first
        // quarter of its life and settles back, and because it is faded in across the
        // skin rather than clipped by it, the arrival is a fade and not a switch.
        let rise = (life * 4.0).min(1.0) * (1.0 - life * 0.55);
        let height = surface + 0.02 + (0.3 + 0.9 * rise as f64) * size as f64;
        // Foam is not white. It starts as the sea with air beaten into it and whitens as
        // the bubbles rise, so a fresh speck carries the water's own colour and an old
        // one is nearly white — and between them is the whole range you see along a bow
        // wave at once.
        let sea = Vec3::new(0.62, 0.78, 0.92);
        let mut colour = sea.lerp(
            Vec3::ONE,
            (life * 1.4 + (1.0 - s.tint) * 0.5).clamp(0.0, 1.0),
        );
        // Faded in across the skin. A mark whose middle is still below the drawn
        // surface is mostly hidden anyway, and the depth buffer hides it abruptly: this
        // takes it out smoothly first so the last of it goes while there is nothing
        // left to see. Over its own size, so a big claw emerges slowly and a speck is
        // through in an instant.
        let clear = ((height - surface) / (size as f64).max(1e-3)) as f32;
        let mut alpha = alpha * crate::curve::smoothstep(0.0, 0.9, clear);
        if ink.0 {
            // Drawn foam is ink rather than airbrush, but ink is not a light switch.
            //
            // The first version made the twinkle binary — a white dot or nothing — which
            // is how a single mark behaves on a page and not how a field of them reads
            // in motion: a thousand specks blinking on and off is a static noise, and
            // what the eye wants from broken water is specks *coming up* and *going
            // out*. The seething is in the transitions.
            //
            // Four levels, so each speck climbs and falls through three intermediate
            // shades on its way. Still quantised — still the drawn sea's own grammar,
            // where a tone is one of a few and never anything between — but with
            // somewhere to go.
            const LEVELS: f32 = 4.0;
            let stepped = (alpha / 0.95 * LEVELS).round() / LEVELS;
            alpha = stepped.clamp(0.0, 1.0) * 0.95;
            colour = Vec3::new(0.97, 0.99, 1.0);
        }
        // Sitting *on* the water rather than half in it. A speck off the bow is a few
        // centimetres across and six centimetres of lift clears it; a whitecap is a
        // metre or two across, and at the same lift the sea — which is drawn opaque —
        // hid its bottom half and what was left read as something sunk rather than
        // something floating.
        let centre = origin.to_bevy(s.at, height as f32);
        // Facing the eye, so a speck is a speck from everywhere rather than a flake
        // lying on the water that vanishes edge-on.
        let to_eye = (view.translation - centre).normalize_or_zero();
        let right = to_eye.cross(Vec3::Y).normalize_or_zero();
        let up = right.cross(to_eye).normalize_or_zero();
        // Which half of the sheet this one is drawn from. A speck off the bow is a
        // drop of water and a mark torn off a crest is a handful of foam, and until now
        // both were the same white square — which is what "I still cannot see the
        // whitecaps, and they are squares" was about from two directions at once.
        let cell = if s.claw { 0.5 } else { 0.0 };
        for (u, v) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            positions.push(centre + right * (size * u) + up * (size * v));
            colours.push([colour.x, colour.y, colour.z, alpha]);
            uvs.push(Vec2::new(cell + (u * 0.5 + 0.5) * 0.5, v * 0.5 + 0.5));
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);

    // A pulse, because "there are no whitecaps" has two completely different causes —
    // none are being born, or they are being born too small to see — and no amount of
    // looking at the water tells you which.
    if darts_thrown > 0 {
        debug!(
            "spray: {born_breaking} of {darts_thrown} darts found a crest, {} specks alive",
            spray.live.len()
        );
    }
}
