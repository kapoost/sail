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
const COUNT: usize = 9000;

/// Multiply the whole spray budget, for finding out what it costs.
///
/// `SAIL_SPRAY=4`. "Can there be a great many more marks on the crests, or is that
/// expensive?" is a question with a number for an answer, and the only way to get the
/// number is to run the same gale twice with two budgets.
fn budget() -> usize {
    let scale: f32 = std::env::var("SAIL_SPRAY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0);
    ((COUNT as f32) * scale.clamp(0.1, 20.0)) as usize
}
/// Seconds one lives.
const LIFE: f32 = 3.4;
/// And how long a mark torn off a crest does.
///
/// *Short*, and many of them, which is the opposite of the obvious answer. A long life
/// gives a few marks that sit there being looked at; what a breaking crest actually
/// does is throw a great deal of foam that is gone almost at once, and the eye reads the
/// seething rather than any one patch of it. So: a second and a half, and four times as
/// many thrown.
///
/// They grow as they go. A puff of foam does not fade where it was — it spreads, which
/// is air coming out of it and the water running off the top of the wave, and a mark
/// that gets bigger while it gets fainter reads as something dispersing rather than
/// something being turned off. The two together are most of what makes it look like
/// foam and not like a sprite.
const CLAW_LIFE: f32 = 4.0;
/// How fast foam spreads outward from where it was thrown, m/s.
const SPREAD: f64 = 0.55;
/// How near the top of a wave a dart has to land, as a fraction of the significant
/// height. Elevation only — see the dart loop for why steepness is the wrong question.
///
/// **Re-set when the sea stopped being a root two too tall.** Significant height is four
/// times the root-mean-square elevation, so this threshold is really a number of standard
/// deviations: 0.42 of Hs is 1.7 of them, which a Gaussian sea is above 4.6 % of the
/// time. On the old surface — the one that was 41 % higher than it said — the same
/// fraction of Hs was 1.2 deviations and 12 % of the water, and the whole look of a
/// breaking sea was calibrated by eye against that. Correcting the height without
/// correcting this took two thirds of the whitecaps off the sea.
///
/// Three tenths is 1.2 deviations and 11 % of the surface, which is where it was.
const CREST_TOP: f64 = 0.30;

/// How many looks at the water a fully whitecapped sea gets a second.
///
/// **Set against measured whitecap coverage**, which is a real number and not a taste:
/// Monahan's relation gives the fraction of sea surface covered in white as
/// `3.8e-6 U^3.4` — about one per cent of the water at twenty knots and five at a gale's
/// thirty-two. Each crest found here is now nine marks along a streak a boat's length
/// long, living four seconds; at a third of the old rate that comes out near five per
/// cent in a gale, against the quarter of a per cent the old scattering of dots made.
///
/// Fewer and bigger is also what the frame can afford: these are additive quads and what
/// they cost is area, so the same white water in a tenth as many marks is most of a
/// millisecond back.
/// Raised with the reach: the darts are scattered over the whole disc, so taking that
/// disc from ninety metres to two hundred and sixty is eight times the water and eight
/// times as few crests found in any part of it.
const CREST_DARTS: f32 = 1600.0;
/// How many bubbles are drawn in a patch of foam.
///
/// Four, which is not how many there are — a square metre of fresh foam has thousands —
/// but how many it takes for the eye to read *bubbles* rather than *white*. Past that
/// they stop being individuals and go back to being a texture, which the foam marks are
/// already doing better and more cheaply.
const BUBBLES_IN_A_PATCH: usize = 4;
/// And how far out it is worth making any, metres.
const BUBBLES_WITHIN: f64 = 35.0;

/// Marks in one breaking crest, and the shape they are laid in.
///
/// **A whitecap is a streak, not a blob.** It was a handful of marks scattered in a
/// circle a metre and a half across, which from the deck is a white dot — and a sea
/// covered in white dots reads as sparkle rather than as breaking water. What a crest
/// actually does is break *along itself*: the white runs down the crest line for ten or
/// fifteen metres and a couple of metres across, perpendicular to the way the sea is
/// going, and that long shape is the whole of what the eye recognises.
///
/// Nine marks along a streak whose length follows the sea — a metre and a half of chop
/// breaks in patches you could stand on, a three-metre gale sea in streaks the length of
/// the boat.
const CREST_SPECKS: usize = 9;
/// Half-width of the streak across the crest, metres.
const CREST_WIDE: f64 = 1.1;
/// Length along the crest, metres: a base and a part that follows the significant
/// height, bounded either side so a millpond and a storm both stay believable.
const CREST_LONG: (f64, f64) = (4.0, 3.4);
const CREST_LONGEST: f64 = 16.0;

// ---------------------------------------------------------------------------------------
// The third source: a crest that is actually breaking, throwing water forward of itself.
// ---------------------------------------------------------------------------------------

/// Significant steepness at which a wind sea starts to throw its crests forward, and
/// where it is doing it everywhere.
///
/// **Steepness is the whole criterion and it is not a local measurement.** A wave breaks
/// when the water at its crest is moving as fast as the wave itself, which for a random
/// sea comes down to the *significant steepness* — the significant height over the
/// wavelength of the dominant component, `H / L` with `L = gT²/2π`. A long ocean swell
/// four metres high is not breaking; a metre and a half of short wind sea on a six-second
/// period is breaking all over, and it is the same number that tells you which.
///
/// **The numbers are the sea's, and the first pair were twice what they should have
/// been.** A fully developed wind sea runs at a significant steepness of about one in
/// twenty; whitecapping starts around one in fifty, which is a Force 4 chop, and by one
/// in twenty-two the sea is white from end to end. Set at four and seven and a half
/// hundredths, an ordinary working sea of a metre and a quarter on a six-second period
/// came out at 0.021 and never broke at all — which is not a sea anybody has been out
/// in. Measured against the three cases in the test below before they were changed.
const PLUNGES_ABOVE: f64 = 0.018;
const PLUNGES_HARD: f64 = 0.045;

/// How far ahead of the crest the water lands, as a fraction of the significant height.
///
/// **This is the effect.** A breaker is not a white hat sitting on top of a wave: the
/// crest curls over and throws its water down the *front face*, so the foam appears
/// ahead of the crest and the wave then runs over it. That is what the eye reads as
/// breaking rather than as foam — the white leading the wave by a few metres.
const LEADS_BY: f64 = 1.1;

/// How wide one breaking line is, metres, and how many marks are in it.
///
/// A line rather than a blob, because a breaker goes along the crest: it starts at one
/// place and unzips sideways, and from a boat what you see is a white *edge* several
/// metres long with the sea behind it still green.
const TONGUE_WIDE: f64 = 9.0;
const TONGUE_SPECKS: usize = 10;

/// How hard the plunging water is thrown forward, m/s.
///
/// **Not the wave's speed.** The wave travels at `gT/2π` — ten metres a second for a
/// six-second sea — and the water does not: it is moving at the orbital velocity, which
/// is a tenth of that. Foam given the wave's speed shoots across the sea like something
/// with an engine in it, which is the classic way to get this wrong. What *does* travel
/// at the wave's speed is the place where the next breaking line is born, and that is
/// handled by birth position rather than by giving anything a velocity.
///
/// So a speck gets the water's own velocity where it lands, plus this much of a throw
/// down the face, and the throw is the only part that is about breaking.
const THROWN_FORWARD: f64 = 1.8;

/// What fraction of the wave's own speed the white water runs at while it breaks.
///
/// **Most of it.** A breaking crest's foam is not floating debris, it is a roller being
/// pushed along by the wave that made it: it rides the front face at close to the wave's
/// speed until the break is over, and then it stops and becomes a patch of foam the sea
/// carries at its own slow pace. Two thirds, which in a six-second sea is six metres a
/// second — fast enough to be a thing coming at you, and not so fast that it outruns the
/// crest it belongs to.
const ROLLER_RIDES: f64 = 0.66;

/// How long the roller keeps running, seconds.
///
/// A break in a wind sea lasts about a second: the crest curls, the white water surges
/// down the face with the wave, and then it is a patch of foam the sea is carrying. The
/// surge decays with this as its time constant, so by a second and a half there is
/// nothing left of it but the water's own drift.
const ROLLER_DIES: f64 = 0.55;

/// How far out breaking crests are drawn, metres.
///
/// Ninety: far enough that the sea has whitecaps in it rather than a ring of them round
/// the boat, near enough that each speck is still more than one pixel. Past this the
/// shading on the water carries it, which is the right division of labour — a speck at
/// three hundred metres is a sub-pixel dot and the tone it would contribute is already
/// there.
const CREST_REACH: f64 = 260.0;

/// How many marks a crest is drawn with, by how far off it is, and how much bigger each
/// one gets to make up for there being fewer.
///
/// **Coverage is what reads, and coverage is marks times area.** Ninety metres of white
/// water around the boat and bare sea beyond it is a sea with a lid on; the eye finds
/// the edge of the foam long before it counts the marks. But keeping the near field's
/// nine marks a crest out to a quarter of a mile is eight times the water and eight
/// times the quads, which the frame will not have.
///
/// So a crest at distance is drawn with fewer and larger marks — the same white water in
/// a third of the geometry. At two hundred metres a whitecap is a few pixels across and
/// nobody can tell how many pieces it is in.
fn detail_at(range: f64) -> (usize, f32) {
    if range < 70.0 {
        (CREST_SPECKS, 1.0)
    } else if range < 160.0 {
        (5, 1.35)
    } else {
        (3, 1.75)
    }
}

/// The three things in the water that catch the light.
#[derive(Clone, Copy, PartialEq)]
enum Mark {
    /// Thrown off the bow: a drop of water in the air.
    Drop,
    /// Torn off a breaking crest: a handful of foam, which tears as it dies.
    Claw,
    /// **A bubble**, which is what foam is actually made of.
    ///
    /// Not a third size of blob: a little sphere, dark in the middle where you are
    /// looking through it into the water, bright round the rim, with one hard glint
    /// where the sky is. Alongside and in the wake the eye is close enough to resolve
    /// them, and a patch of foam drawn without any is a patch of white paint.
    Bubble,
}

impl Mark {
    /// Which cell of the sheet, and how many frames that cell has.
    fn cell(self, life: f32) -> f32 {
        const CELLS: f32 = 6.0;
        let n = match self {
            Mark::Drop => 0.0,
            Mark::Bubble => 1.0,
            // Four frames of coming apart, walked through over the mark's own life.
            Mark::Claw => 2.0 + (life * 4.0).floor().clamp(0.0, 3.0),
        };
        n / CELLS
    }

    /// How long it lasts, seconds.
    fn life(self) -> f32 {
        match self {
            Mark::Drop => LIFE,
            Mark::Claw => CLAW_LIFE,
            // Longer than the foam it came out of: the patch tears apart and the
            // bubbles in it go on sitting there until they pop.
            Mark::Bubble => CLAW_LIFE * 1.6,
        }
    }
}

struct Speck {
    at: glam::DVec2,
    drift: glam::DVec2,
    /// **The roller**, and it only belongs to foam that is breaking.
    ///
    /// Corrected from the deck, and the correction is about which thing is being drawn.
    /// Foam that has *finished* breaking is dead water: it sits where it was made,
    /// moving at the orbital velocity, and the wave sails out from under it. But the
    /// white water of a breaker **while it is breaking** is a roller riding the front of
    /// the wave, and it goes forward with it at a good fraction of the wave's own speed
    /// — which is what makes a breaker a thing that comes *at* you rather than a patch
    /// you pass.
    ///
    /// So a breaking mark is born with both: the water's velocity, which it keeps, and
    /// this surge, which dies away over about the time the break lasts and leaves the
    /// foam lying where the roller gave up.
    surge: glam::DVec2,
    age: f32,
    /// Varies, or a hundred identical blobs read as a texture rather than as water.
    scale: f32,
    /// Its own phase, so the shimmer is a thousand separate sparkles rather than one
    /// field of foam pulsing in time with itself.
    phase: f32,
    /// How blue this one is to begin with, 0 white and 1 the colour of the sea it came
    /// out of.
    tint: f32,
    /// What kind of thing it is, which decides the mark it is drawn with, how long it
    /// lives and how big it gets — see `spray_marks`.
    kind: Mark,
    /// How fast this one breathes, radians a second.
    ///
    /// Its own, which is the correction: every speck used the same two beats and
    /// differed only in phase, so a thousand of them were a thousand copies of one
    /// rhythm offset against each other. That reads as a pattern however well the
    /// phases are spread — reported from the deck as the sparkles coming too regularly.
    /// Drawn across a wide range rather than jittered around one, because what the eye
    /// is looking for is *no* tempo at all.
    beat: f32,
    /// And how deeply it twinkles, 0 for one that just glows.
    ///
    /// Most foam does not sparkle. It sits there being white, and the glitter is a few
    /// bubbles in it catching the sky and turning. Giving every speck the same strong
    /// twinkle made the whole field pulse; most of them barely move now, and the few
    /// that do are what the sparkle is.
    sparkle: f32,
}

/// A speck's own tempo, radians a second, from a uniform draw.
///
/// Geometric across four to one, so the draw spreads the *rhythms* evenly rather than
/// the frequencies — which is how the ear and the eye both measure tempo, and the
/// difference between a field with no beat in it and one with a beat and some jitter.
/// Slower than the single rate it replaces, because every speck having the same one
/// made it the rate of the whole sea.
fn beat_of(r: f32) -> f32 {
    0.9 * 4.0_f32.powf(r)
}

/// How deeply it twinkles. Squared, so most hardly do and a few do it hard.
fn sparkle_of(r: f32) -> f32 {
    0.62 * r * r
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
    /// And the same again for the breakers, which have their own rate: whitecaps are a
    /// property of how much white is on the sea, and a breaking crest is an event.
    plunges: f64,
}

impl Spray {
    /// Throw foam at a patch of water that is not the hull.
    ///
    /// Exposed because the hull is not the only thing out there disturbing the surface.
    /// A lure skipping along two metres down leaves a swirl you can follow, and that
    /// swirl is the only way to see where your gear is — the lure itself is a hand's
    /// length of metal under the water and invisible at any resolution, let alone this
    /// one. See `rod.rs`, which uses the presence and absence of it as a depth gauge:
    /// no mark on the water means the lure is down.
    ///
    /// Takes a fractional count and rounds it with the generator rather than by
    /// truncating, so a tenth of a speck a frame comes out as one speck every ten
    /// frames instead of as nothing at all for ever.
    pub fn disturb(&mut self, at: glam::DVec2, drift: glam::DVec2, specks: f32, scale: f32) {
        let whole = specks.floor();
        let n = whole as usize
            + usize::from((self.seed.next_f64() as f32) < (specks - whole).clamp(0.0, 1.0));
        for _ in 0..n {
            if self.live.len() >= budget() {
                return;
            }
            let r = &mut self.seed;
            let spread = r.next_f64() * std::f64::consts::TAU;
            let out = 0.55 * r.next_f64().sqrt();
            self.live.push(Speck {
                at: at + glam::DVec2::new(spread.cos(), spread.sin()) * out,
                drift,
                surge: glam::DVec2::ZERO,
                // Born a little old, which is how they get the short life this wants
                // without a second lifetime constant and a second branch in the ageing
                // loop. A lure's swirl closes up in a second or so; the hull's foam has
                // a boat's worth of water in it and lasts.
                age: LIFE * 0.55,
                scale: scale * (0.7 + 0.8 * r.next_f64() as f32),
                phase: r.next_f64() as f32 * std::f32::consts::TAU,
                tint: 0.45 + 0.55 * r.next_f64() as f32,
                kind: Mark::Drop,
                beat: beat_of(r.next_f64() as f32),
                sparkle: sparkle_of(r.next_f64() as f32),
            });
        }
    }
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
    let vertices = budget() * 4;
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![Vec3::ZERO; vertices]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![Vec3::Y; vertices]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![Vec2::ZERO; vertices]);
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_COLOR,
        vec![[1.0_f32, 1.0, 1.0, 0.0]; vertices],
    );
    let mut indices = Vec::with_capacity(budget() * 6);
    for i in 0..budget() {
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
        live: Vec::with_capacity(budget()),
        seed: sail_core::rng::Pcg32::new(0xF0_A3_11),
        owed: 0.0,
        darts: 0.0,
        plunges: 0.0,
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
    let mut keep = Vec::with_capacity(budget());
    for mut s in spray.live.drain(..) {
        s.age += dt;
        // The water's own motion, plus whatever is left of the roller. A breaker's
        // white water runs forward with the wave while the break lasts and then stops;
        // half a second of time constant is about the length of one.
        s.at += (s.drift + s.surge) * dt as f64;
        s.surge *= (-(dt as f64) / ROLLER_DIES).exp();
        if s.age < s.kind.life() {
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
    spray.darts +=
        CREST_DARTS * (budget() as f32 / COUNT as f32) * crest_share as f32 * (1.0 + blown) * dt;

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
        while *darts >= 1.0 && live.len() < budget() {
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
            // Laid along the crest, which is across the way the sea is running.
            //
            // Each mark is still a small one and the *field* of them is the picture —
            // that part was right. What was wrong was the arrangement: a circle of them
            // is a dot however many there are, and the thing being drawn is a line of
            // broken water a boat's length long.
            let blow = downwind * (0.5 + 1.5 * r.next_f64()) * blown.min(1.2) as f64;
            let run = downwind.normalize_or_zero();
            let crest = glam::DVec2::new(-run.y, run.x);
            let long = (CREST_LONG.0 + CREST_LONG.1 * hs).min(CREST_LONGEST);
            let (marks, fatter) = detail_at(range);
            for i in 0..marks {
                if live.len() >= budget() {
                    break;
                }
                // Spaced along the streak rather than scattered in it, with enough
                // jitter that the spacing is not a row of beads.
                let step = i as f64 / (marks - 1).max(1) as f64 - 0.5;
                let along = (step + (r.next_f64() - 0.5) * 0.12) * long;
                let across = (r.next_f64() - 0.5) * 2.0 * CREST_WIDE;
                live.push(Speck {
                    at: at + crest * along + run * across,
                    drift: blow + glam::DVec2::new(a.cos(), a.sin()) * 0.25,
                    surge: glam::DVec2::ZERO,
                    age: 0.0,
                    scale: (2.0 + 2.6 * r.next_f64() as f32) * fatter,
                    phase: r.next_f64() as f32 * std::f32::consts::TAU,
                    tint: 0.2 * r.next_f64() as f32,
                    kind: Mark::Claw,
                    beat: beat_of(r.next_f64() as f32),
                    sparkle: sparkle_of(r.next_f64() as f32),
                });
            }
            // **And the bubbles in it**, which are what the foam is made of.
            //
            // Near enough to be resolved, a patch of foam is not a white shape: it is a
            // few hundred little spheres, each with the sky in it, sitting in water
            // that is merely pale. They are spawned with the patch and outlive it — the
            // handful tears apart and the bubbles go on sitting there — which is why a
            // whitecap leaves a trail behind it instead of simply vanishing.
            // Only near enough to be resolved. A bubble is a few centimetres: at eighty
            // metres it is a hundredth of a pixel, and a thousand of them out there were
            // a thousand marks of budget and fill spent on nothing at all.
            let close = range < BUBBLES_WITHIN;
            for _ in 0..(if close { BUBBLES_IN_A_PATCH } else { 0 }) {
                if live.len() >= budget() {
                    break;
                }
                let along = (r.next_f64() - 0.5) * long;
                let across = (r.next_f64() - 0.5) * 2.4 * CREST_WIDE;
                live.push(Speck {
                    at: at + crest * along + run * across,
                    // They go with the water and not with the wind: a bubble is in the
                    // sea rather than on it, and that difference is visible the moment
                    // there are both kinds in one patch.
                    drift: blow * 0.25,
                    surge: glam::DVec2::ZERO,
                    age: 0.0,
                    scale: 0.8 + 1.8 * r.next_f64() as f32,
                    phase: r.next_f64() as f32 * std::f32::consts::TAU,
                    tint: 0.0,
                    kind: Mark::Bubble,
                    beat: beat_of(0.55 + 0.45 * r.next_f64() as f32),
                    // Every one of them twinkles: a bubble is a sphere with the sky on
                    // it, and the glint moves as the water turns under it.
                    sparkle: 0.5 + 0.5 * r.next_f64() as f32,
                });
            }
            born_breaking += 1;
        }
    }

    // --- And the crests that are actually breaking -----------------------------
    //
    // The source above draws whitecaps: foam *on* the tops, which is what a sea with
    // some white in it looks like. This one draws the moment itself — the crest curling
    // over and throwing its water down the front face, so the white appears a couple of
    // metres **ahead** of the wave and the wave then runs over it.
    //
    // It only happens when the sea is steep enough to do it, which is a property of the
    // whole spectrum rather than of the patch under the dart: significant height over
    // the dominant wavelength. In a long swell nothing here fires at all, however big
    // the waves are, and that is correct — a four-metre swell does not break in deep
    // water and a metre and a half of short wind sea breaks everywhere.
    let period = sim.sim.water.peak_period_or(6.0);
    // How fast the waves themselves are going: `gT/2π` in deep water, which is the
    // speed the breaking front rides at and nothing else in here moves at.
    let phase_speed = 9.81 * period / std::f64::consts::TAU;
    let steepness = {
        let wavelength = 9.81 * period * period / std::f64::consts::TAU;
        hs / wavelength.max(1.0)
    };
    let plunging =
        crate::curve::smoothstep(PLUNGES_ABOVE as f32, PLUNGES_HARD as f32, steepness as f32)
            as f64;
    if plunging > 0.01 {
        // The direction the sea is *going*, which is where a breaker throws its water.
        // Not the wind at the boat and not the wave's own speed — see `THROWN_FORWARD`.
        let forward = downwind.normalize_or_zero();
        let across = glam::DVec2::new(forward.y, -forward.x);
        let Spray {
            live,
            seed,
            plunges,
            ..
        } = &mut *spray;
        *plunges += CREST_DARTS as f64 * 0.22 * plunging * dt as f64;
        while *plunges >= 1.0 && live.len() < budget() {
            *plunges -= 1.0;
            let r = &mut *seed;
            let a = r.next_f64() * std::f64::consts::TAU;
            let range = CREST_REACH * r.next_f64().sqrt();
            let at = st.pos + glam::DVec2::new(a.cos(), a.sin()) * range;
            // On a crest, and with the water *ahead* of it lower — which is how the
            // front face is told from the back one. A breaker that threw its water up
            // the back of the wave would be the effect drawn backwards, and from the
            // deck that reads as the sea running the wrong way.
            let here = sim.sim.water.height(at, t);
            if here / hs.max(0.05) < CREST_TOP * 0.8 {
                continue;
            }
            let ahead = at + forward * (hs * LEADS_BY).max(1.0);
            if sim.sim.water.height(ahead, t) >= here {
                continue;
            }
            // The line, laid along the crest rather than scattered round a point.
            let width = TONGUE_WIDE * (0.5 + 0.5 * r.next_f64());
            for n in 0..TONGUE_SPECKS {
                if live.len() >= budget() {
                    break;
                }
                // Along the crest at even spacing would be a string of beads, which
                // is what it looked like: real foam is ragged, thickest where the wave
                // went first and thinning away at both ends. So the marks are jittered
                // along the line as well as across it, and a few are thrown further
                // down the face than the rest.
                let step = width / (TONGUE_SPECKS - 1) as f64;
                let along = (n as f64 / (TONGUE_SPECKS - 1) as f64 - 0.5) * width
                    + (r.next_f64() - 0.5) * step * 0.9;
                let down = (r.next_f64() - 0.5) * 1.2 + r.next_f64().powi(3) * hs * 0.9;
                let spot = ahead + across * along + forward * down;
                // Thickest in the middle of the break and thinner at its ends.
                let taper = {
                    let u = n as f32 / (TONGUE_SPECKS - 1) as f32;
                    0.45 + 0.55 * (std::f32::consts::PI * u).sin()
                };
                // The water's own velocity where it lands, plus the throw down the
                // face. The first is why the foam stays with the sea instead of
                // sliding over it; the second is the breaking.
                let orbital = sim.sim.water.velocity(spot, t);
                // What it keeps: the water's own motion and the wind's push on it.
                let drift =
                    glam::DVec2::new(orbital.x, orbital.y) + downwind * blown.min(1.2) as f64 * 0.6;
                // And what dies away: the roller. White water on a breaking crest runs
                // forward **with the wave**, at a good part of the wave's own speed,
                // for as long as the break lasts — that is what makes a breaker a thing
                // that comes at you instead of a patch you sail past. The throw down
                // the face is in here too, because it is the same second of the wave's
                // life.
                let roller = phase_speed * ROLLER_RIDES * (0.7 + 0.5 * r.next_f64());
                let surge = forward * (roller + THROWN_FORWARD * (0.3 + 0.5 * r.next_f64()));
                live.push(Speck {
                    at: spot,
                    drift,
                    surge,
                    age: 0.0,
                    // Bigger than a whitecap speck: this is the front of a breaker, and
                    // at two metres across it is one of the larger things on the water.
                    scale: (3.2 + 3.0 * r.next_f64() as f32) * taper,
                    phase: r.next_f64() as f32 * std::f32::consts::TAU,
                    tint: 0.12 * r.next_f64() as f32,
                    kind: Mark::Claw,
                    beat: beat_of(r.next_f64() as f32),
                    sparkle: sparkle_of(r.next_f64() as f32),
                });
            }
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
        // **Some of it is bubbles, and this is where they can be seen.**
        //
        // Bubbles were born only in breaking crests, which are scattered over ninety
        // metres of sea — and at that distance a bubble is a fraction of a pixel. The
        // one patch of broken water that is always within a few metres of the eye is the
        // one the boat is making herself, so a fifth of the foam along the hull is now
        // bubbles rather than foam, at a size that has pixels: a bow wave seen from the
        // cockpit is a mass of them, each with the sky in it.
        const BUBBLES_AT_THE_HULL: f64 = 0.2;
        let bubbly = r.next_f64() < BUBBLES_AT_THE_HULL;
        let scale = 0.55 + 0.9 * r.next_f64() as f32;
        live.push(Speck {
            at,
            drift,
            surge: glam::DVec2::ZERO,
            age: 0.0,
            // A bubble's size does not come from how hard the water was beaten, so the
            // spread of hull scales would make most of them too small to resolve. Held
            // up to the size the eye reads as a bubble at arm's length over the rail.
            scale: if bubbly { scale * 2.4 } else { scale },
            phase: r.next_f64() as f32 * std::f32::consts::TAU,
            tint: r.next_f64() as f32,
            kind: if bubbly { Mark::Bubble } else { Mark::Drop },
            beat: beat_of(r.next_f64() as f32),
            sparkle: sparkle_of(r.next_f64() as f32),
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
        let life = (s.age / s.kind.life()).clamp(0.0, 1.0);
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
        // Two different things with two different profiles. A speck off the bow is a
        // drop that spreads a little as it falls. A mark off a crest *blooms*: it comes
        // out small and hard and ends up four times the size and gone, which is what a
        // handful of foam does when the air comes out of it.
        // Smaller, both of them. The mark reads by *how many* there are, and a big one
        // is a splodge of paint however good its shape: the claw now starts at three
        // centimetres and ends under twenty, which at the distance a crest breaks from
        // the boat is a handful of foam rather than a bedsheet.
        let size = if s.kind == Mark::Bubble {
            // A bubble does not grow. What it does is sit there until it is gone, which
            // is the other half of why a patch of foam does not read as paint — the big
            // marks spread and tear while the small bright ones stay exactly the size
            // they are.
            //
            // **And it has to be a size the screen has pixels for.** At a centimetre
            // and a half, a bubble twenty metres off — which is where the camera sits —
            // is six hundredths of a pixel wide on a nine-hundred-and-sixty-pixel
            // picture. It was drawn, it was in the mesh, and it could not be seen, which
            // is "nie widzę particles" exactly. Three to fifteen centimetres is still a
            // bubble and is two or three pixels at the stern rail.
            (0.03 + 0.012 * life) * s.scale
        } else if s.kind == Mark::Claw {
            // The same arithmetic for the foam itself. A whitecap is a patch of white
            // water a couple of metres across, not a scatter of coins: at a third of a
            // metre a mark forty metres out was a pixel and a half and the sea's own
            // texture — which draws white streaks of its own — simply swallowed it.
            (0.10 + 0.30 * life) * s.scale
        } else {
            (0.04 + 0.06 * life) * s.scale * 0.62
        };
        // And every one of them twinkles. Real broken water does not glow steadily: a
        // bubble catches the sky, turns, and goes out, and what the eye picks up from a
        // bow wave is that seething rather than any single bright patch. Two beats, not
        // one, and both prime-ish so a thousand specks never fall into step.
        // Slower than it was. At seventeen beats a second a speck crosses every shade it
        // has between one frame and the next, which is the same thing as not having them
        // — the levels only read if the climb through them takes long enough to see.
        let twinkle = 1.0
            - s.sparkle
                * (0.6 * (s.age * s.beat + s.phase).sin()
                    + 0.4 * (s.age * s.beat * 1.73 + s.phase * 2.1).sin())
                .mul_add(-0.5, 0.5);
        // And they go out faster than they grow, or a bloom is a cloud that swells to
        // fill the sea. The claws are given a steeper curve than the specks for the same
        // reason they are given a bigger one: the mark is the *arrival* of the foam.
        let fade = match s.kind {
            Mark::Claw => 2.6,
            Mark::Bubble => 3.1,
            Mark::Drop => 1.7,
        };
        // **Foam arrives and then it lies there.** A curve that only falls says the
        // first, and the first is a tenth of what a whitecap is: the break is white and
        // hard for a second and the patch it leaves goes on sitting on the water for
        // ten or twenty, pale and plain to see, drifting down the face of the wave. With
        // the fall alone, seventeen hundred marks were alive on the sea and all but a
        // couple of hundred were too faint to find — measured as bright pixels, the foam
        // put 0.6 % of white on the water where the measured whitecap coverage for this
        // wind is nearer three.
        //
        // So the arrival keeps its steep curve and a patch is laid under it: not quite
        // half the brightness, holding nearly to the end of the mark's life. The maximum
        // of the two, because they are the same water seen at two ages and the brighter
        // is whichever one is happening.
        let bloom = (1.0 - life).powf(fade);
        let alpha = match s.kind {
            Mark::Claw => bloom.max(0.45 * (1.0 - life).powf(0.7)),
            _ => bloom,
        } * 0.95
            * twinkle.clamp(0.0, 1.4);
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
        // Asked for the whole parcel, not just its height.
        //
        // **A mark of foam sits on a parcel of water, and the drawn sea moves its
        // parcels sideways.** The mesh is Gerstner: the vertex whose mean position is
        // here is drawn here-plus-its-own-displacement, which gathers the water into
        // the crests. A speck placed at its mean position with the height of that
        // position is therefore drawn beside the water it belongs to — and since a mark
        // floats a few centimetres proud, a few centimetres of disagreement puts it
        // under the skin. Measured: six times the foam on the water made the picture
        // barely a per cent brighter, because most of it was inside the sea.
        //
        // So the mark goes where its parcel goes. Same question, same answer, one
        // definition — which is what the drift being in `Parcel` is for.
        let water = sim.sim.water.surface_resolved(s.at, t, resolution);
        let surface = water.height;
        let afloat = s.at + water.drift;
        // And it comes *up through* the surface rather than being pinned to it. Foam
        // thrown by a breaking wave is under the water for a moment and then it is not;
        // what the eye reads is the arrival. A mark rises its own height over the first
        // quarter of its life and settles back, and because it is faded in across the
        // skin rather than clipped by it, the arrival is a fade and not a switch.
        let rise = (life * 4.0).min(1.0) * (1.0 - life * 0.55);
        // **And it sits in the water, not on it.** The lift was a fraction of the
        // mark's own size with nothing holding it down, which is harmless while every
        // mark is a few centimetres across and wrong as soon as one is not: a breaking
        // crest's marks are the better part of a metre, and at three tenths of that
        // they were floating a foot clear of the sea with daylight under them.
        //
        // Foam is air in water rather than something resting on top of it, so the lift
        // is a small part of the mark and stops growing once a mark is big: a hand's
        // breadth proud as it arrives, settling to a few centimetres. The cap is what
        // makes a two-metre breaker and a two-centimetre speck both look like they are
        // *in* the sea.
        let swim = (size as f64).min(0.5);
        let height = surface + 0.02 + (0.14 + 0.30 * rise as f64) * swim;
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
        let centre = origin.to_bevy(afloat, height as f32);
        // **Lying on the water, not standing up in front of the camera.**
        //
        // A billboard is right for a drop of water in the air off the bow — it has no
        // orientation of its own and turning it to face the eye is the only honest
        // thing to do with it. It is wrong for foam: a whitecap is a patch floating on
        // a surface, and drawn upright it reads as a sheet of paper stood on edge,
        // which is why the white sat *on top of* the sea instead of *in* it.
        //
        // So the quad's normal is leaned from the eye towards straight up, by kind: the
        // foam nearly all the way, the bubbles half — a bubble is a sphere and looks the
        // same from anywhere — and the drops not at all. `right` stays horizontal and
        // across the view whatever the lean is, which keeps the quad stable and never
        // degenerates except looking straight down.
        let to_eye = (view.translation - centre).normalize_or_zero();
        let lies_flat = match s.kind {
            Mark::Claw => 0.85,
            Mark::Bubble => 0.5,
            Mark::Drop => 0.0,
        };
        let right = Vec3::new(-to_eye.z, 0.0, to_eye.x)
            .normalize_or(to_eye.cross(Vec3::Y).normalize_or_zero());
        let face = to_eye.lerp(Vec3::Y, lies_flat).normalize_or(to_eye);
        let up = face.cross(right).normalize_or_zero();
        // Which half of the sheet this one is drawn from. A speck off the bow is a
        // drop of water and a mark torn off a crest is a handful of foam, and until now
        // both were the same white square — which is what "I still cannot see the
        // whitecaps, and they are squares" was about from two directions at once.
        // Which cell of the sheet, and for foam which frame of its coming apart.
        let cell = s.kind.cell(life);
        const ACROSS: f32 = 1.0 / 6.0;
        for (u, v) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            positions.push(centre + right * (size * u) + up * (size * v));
            colours.push([colour.x, colour.y, colour.z, alpha]);
            uvs.push(Vec2::new(cell + (u * 0.5 + 0.5) * ACROSS, v * 0.5 + 0.5));
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);

    // A pulse, because "there are no whitecaps" has two completely different causes —
    // none are being born, or they are being born too small to see — and no amount of
    // looking at the water tells you which.
    if darts_thrown > 0 {
        // Broken down by kind, because "there are no whitecaps" has three causes and
        // one number cannot tell them apart: none are born, they are born and die at
        // once, or they are there at a size nobody can see. A count of each kind alive
        // separates the first two from the third in one line.
        let (mut drops, mut claws, mut bubbles) = (0u32, 0u32, 0u32);
        for s in &spray.live {
            match s.kind {
                Mark::Drop => drops += 1,
                Mark::Claw => claws += 1,
                Mark::Bubble => bubbles += 1,
            }
        }
        debug!(
            "spray: {born_breaking} of {darts_thrown} darts found a crest; alive {}              = {drops} hull, {claws} crest, {bubbles} bubbles",
            spray.live.len()
        );
    }
}

#[cfg(test)]
mod when_a_sea_breaks {
    use super::*;

    /// How steep the sea is, from the two numbers that decide it.
    fn steepness(hs: f64, period: f64) -> f64 {
        hs / (9.81 * period * period / std::f64::consts::TAU)
    }

    /// **A long swell does not break and a short wind sea does**, whatever their
    /// heights are — which is the whole reason the criterion is steepness rather than
    /// size, and the fault it guards against is a four-metre ocean swell covered in
    /// white water two hundred miles from the nearest land.
    #[test]
    fn breaking_is_steepness_and_not_height() {
        // Four metres of long Atlantic swell: nothing.
        let swell = steepness(4.0, 14.0);
        assert!(
            swell < PLUNGES_ABOVE,
            "a fourteen-second swell came out at {swell:.3}, which would break"
        );
        // A metre and a half of short wind sea: breaking everywhere.
        let wind_sea = steepness(1.5, 4.5);
        assert!(
            wind_sea > PLUNGES_HARD,
            "a four-and-a-half-second sea came out at {wind_sea:.3}, which would not"
        );
        // And the ordinary working sea in between is in between.
        let working = steepness(1.2, 6.0);
        assert!(
            (PLUNGES_ABOVE..PLUNGES_HARD).contains(&working),
            "{working:.3} is not between the two thresholds"
        );
    }

    /// **The roller rides with the wave and then stops**, which is the difference
    /// between a breaker and a patch of foam.
    ///
    /// Corrected from the deck: white water on a breaking crest is pushed along by the
    /// wave that made it and goes forward at most of the wave's own speed for as long
    /// as the break lasts — that is what makes a breaker come *at* you. What it must
    /// not do is keep going: after the break it is dead water the sea carries at its
    /// own slow pace, and foam that kept the wave's speed would cross the sea like
    /// something with an engine in it.
    #[test]
    fn the_roller_runs_with_the_wave_and_then_gives_up() {
        let phase = 9.81 * 6.0 / std::f64::consts::TAU;
        let roller = phase * ROLLER_RIDES;
        assert!(
            roller > phase * 0.4 && roller < phase,
            "a roller doing {roller:.1} against a wave doing {phase:.1}"
        );
        // Half of it is gone in less than a second, and by two there is nothing left:
        // what the foam does after the break is sit in the water.
        let left = |after: f64| (-after / ROLLER_DIES).exp();
        assert!(
            left(1.0) < 0.25,
            "still {:.0}% of it after a second",
            left(1.0) * 100.0
        );
        assert!(left(2.0) < 0.05);
        // And how far it got: a few metres down the face, not a hundred.
        let run = roller * ROLLER_DIES;
        assert!(
            (2.0..12.0).contains(&run),
            "the roller ran {run:.1} m before it gave up"
        );
    }
}
