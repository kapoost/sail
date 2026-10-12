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
const COUNT: usize = 1600;
/// Seconds one lives.
const LIFE: f32 = 3.4;
/// How fast foam spreads outward from where it was thrown, m/s.
const SPREAD: f64 = 0.55;

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
}

#[derive(Resource)]
pub struct Spray {
    mesh: Handle<Mesh>,
    live: Vec<Speck>,
    seed: sail_core::rng::Pcg32,
    /// Fractional specks carried between frames, so a birth rate of three and a half a
    /// second does not round down to three and lose its half for ever.
    owed: f32,
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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

    let forward = st.forward();
    let starboard = st.starboard();
    let lee = -sim.sim.telemetry.awa.signum();

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
        let where_ = r.next_f64();
        let (along, across) = if where_ < 0.62 {
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
        let drift = outward * SPREAD * (0.6 + 0.8 * r.next_f64())
            + forward * speed * 0.12 * r.next_f64();
        live.push(Speck {
            at,
            drift,
            age: 0.0,
            scale: 0.55 + 0.9 * r.next_f64() as f32,
            phase: r.next_f64() as f32 * std::f32::consts::TAU,
            tint: r.next_f64() as f32,
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
        let size = (0.055 + 0.085 * life) * s.scale;
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
        // Foam is not white. It starts as the sea with air beaten into it and whitens as
        // the bubbles rise, so a fresh speck carries the water's own colour and an old
        // one is nearly white — and between them is the whole range you see along a bow
        // wave at once.
        let sea = Vec3::new(0.62, 0.78, 0.92);
        let mut colour =
            sea.lerp(Vec3::ONE, (life * 1.4 + (1.0 - s.tint) * 0.5).clamp(0.0, 1.0));
        let mut alpha = alpha;
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
        let height = sim.sim.water.height(s.at, t) + 0.06;
        let centre = origin.to_bevy(s.at, height as f32);
        // Facing the eye, so a speck is a speck from everywhere rather than a flake
        // lying on the water that vanishes edge-on.
        let to_eye = (view.translation - centre).normalize_or_zero();
        let right = to_eye.cross(Vec3::Y).normalize_or_zero();
        let up = right.cross(to_eye).normalize_or_zero();
        for (u, v) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            positions.push(centre + right * (size * u) + up * (size * v));
            colours.push([colour.x, colour.y, colour.z, alpha]);
            uvs.push(Vec2::new(u * 0.5 + 0.5, v * 0.5 + 0.5));
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
}
