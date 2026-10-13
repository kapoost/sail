//! Standing and running rigging.
//!
//! Not decoration. Until now the boom swung with no visible cause, and the single
//! most direct piece of feedback a boat offers was missing: **a loaded line is
//! straight and a slack one sags.** You can see a sheet go soft before the telltales
//! have finished telling you, and you can see which genoa sheet is working and which
//! is lying idle. Drawing the sag honestly turns every line into an instrument.
//!
//! The lines are rebuilt every frame as camera-facing ribbons. Rope is thin enough
//! that real tube geometry is wasted — a constant-width strip turned towards the eye
//! reads identically and costs a fraction. The sag is a parabola rather than a true
//! catenary, which over a four-metre span is a difference nobody can see.
//!
//! One detail worth having: the leeward shroud goes slack when she heels hard. That is
//! real, every sailor has watched it happen, and it costs one line of arithmetic.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use sail_core::prelude::*;

use crate::camera::MainCamera;
use crate::sim::Sim;

/// Segments per line. Enough that a sagging sheet curves smoothly.
const SEGMENTS: usize = 14;

/// Where the spreaders stand, as a fraction of the mast above the deck.
///
/// Was a bare height. On a fractional rig the spreaders are a little above the forestay,
/// which is where the mast most wants holding, so it follows the rig rather than being
/// typed in beside it.
pub const SPREADER_HEIGHT: f32 = FORESTAY_HEAD + 0.6;

/// And the inner stay lands below the forestay, which is what makes it inner.
pub const INNER_STAY_HEAD: f32 = FORESTAY_HEAD * 0.62;
/// Where the mast is stepped — from the core, which derives the whole rig from the two
/// sails that were measured. Both files used to carry their own copy of every one of
/// these numbers.
const MAST_FOOT_Z: f32 = sail_core::boat::packet::MAST_FOOT_Z as f32;
pub const MAST_HEAD: f32 = sail_core::boat::packet::MAST_HEAD as f32;

/// Where the forestay lands, which is not the masthead.
///
/// A fraction below it. The sails set flying go to the very top — that is the point of
/// them, and it is why a Code 0 out-measures a genoa on the same rig — while the
/// forestay stops short, so the genoa's head and the sails hoisted above it are not
/// fighting over the same crane. Half a metre is the usual gap on a rig this size.
pub const FORESTAY_HEAD: f32 = sail_core::boat::packet::FORESTAY_HEAD as f32;
/// The boom, high enough to stand under.
///
/// It was a quarter of a metre *below* the helmsman's eye, which put it across the
/// view like a handrail — and would have taken his head off in a gybe. A cruising boom
/// clears a standing helmsman, barely, and you duck anyway.
/// How high the boom rides above the deck at the gooseneck.
///
/// Defined here and nowhere else. It was in two files with the same number in both,
/// which is the arrangement that works right up until one of them is changed.
///
/// Lowered from 1.95 to 1.62. A boom that high stands over a cockpit nobody could see
/// past; this is still a comfortable duck underneath and it puts the sail's area where
/// it does the most good and the least heeling — the end of a boom is the one part of a
/// mainsail you can lower without losing any of it.
pub const BOOM_HEIGHT: f32 = sail_core::boat::packet::BOOM_HEIGHT as f32;
/// Boom length, and it is not a free choice.
///
/// It is the mainsail's foot, so it is fixed by the sail's area and its luff — and by
/// the luff the *drawn* rig actually has, which is the correction that followed. The
/// first pass used the configured luff of 15.2 m and got 5.3 m; but the drawn mast
/// gives only 13.78 m of hoist above the boom, so at 5.3 the triangle came to 41 square
/// metres rather than 46. With the real hoist the foot is 5.80 m, and the clew lands
/// well forward of the transom, which is where a boom of that length ends on a boat of
/// this one's size.
///
/// The configured luff and the drawn hoist still differ by 1.7 m, and the honest fix is
/// a taller mast rather than a longer boom — the aspect ratio the force model uses comes
/// from that luff. It is written down here rather than quietly absorbed, because moving
/// the masthead moves the shrouds, the forestay, the spreaders and every headsail head
/// with it, and that is a change to make deliberately.
///
/// It was 4.2, which drew a main of thirty square metres on a boat whose physics was
/// pulling with forty-six.
///
/// That mattered more than it sounds. The genoa was drawn a third too *large* at the
/// same time, so on deck the headsail looked nearly the size of the main when it is in
/// fact half of it — the one proportion a sailor reads a rig by, and it was reversed.
pub const BOOM_LENGTH: f32 = sail_core::boat::packet::BOOM_LENGTH as f32;
/// Where along the boom the vang takes hold, as a fraction of its length.
///
/// Shared, because the bail is drawn on the boom and the line is drawn to it, and a
/// fitting in one place with the rope arriving at another is the fault this rig has
/// already been caught by four times.
pub const VANG_ALONG_BOOM: f32 = 0.42;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Outhaul,
    /// Holds the bowsprit down against the pull of everything set from its end. A
    /// sprit without one is a lever waiting to break the stem.
    Bobstay,
    /// And the pair that stop it swinging sideways.
    SpritShroud(bool),
    Forestay,
    Backstay,
    Shroud(bool),
    MainSheet,
    Vang,
    /// The headsail's sheet, in two parts: clew to wherever it is lying on the boom,
    /// and on to the car. With nothing in the way the join sits on the straight line
    /// between the ends and cannot be seen.
    JibSheet(bool, u8),
    /// The staysail's sheet, and the two further parts of the tackle it runs in.
    ///
    /// Drawn as the three parts it has rather than as one rope, because that is what is
    /// there and because one short line between a boom and a deck eye is invisible — it
    /// was, and was reported missing twice. A club boom's sheet is a tackle: it has to
    /// hold the whole force of the sail with nobody's hands on it.
    StaysailSheet(u8),
}

/// All the lines, in the order they are packed into the mesh.
const LINES: [Kind; 17] = [
    Kind::Outhaul,
    Kind::Bobstay,
    Kind::SpritShroud(false),
    Kind::SpritShroud(true),
    Kind::Forestay,
    Kind::Backstay,
    Kind::Shroud(false),
    Kind::Shroud(true),
    Kind::MainSheet,
    Kind::Vang,
    // Both genoa sheets. Only one is ever working; the other lies across the deck with
    // a loop in it, and seeing which is which is half of knowing what tack you are on.
    Kind::JibSheet(false, 0),
    Kind::JibSheet(false, 1),
    Kind::JibSheet(true, 0),
    Kind::JibSheet(true, 1),
    Kind::StaysailSheet(0),
    Kind::StaysailSheet(1),
    Kind::StaysailSheet(2),
];

/// How long a line stays lit after the control that moves it is touched.
const FLASH: f32 = 0.85;

#[derive(Resource)]
pub struct Rigging {
    mesh: Handle<Mesh>,
    /// Last frame's controls, so a change can be noticed.
    previous: Controls,
    /// Seconds of highlight left on each line.
    flash: [f32; LINES.len()],
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let vertices = LINES.len() * SEGMENTS * 2;
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![Vec3::ZERO; vertices]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![Vec3::Y; vertices]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0_f32; 4]; vertices]);

    let mut indices = Vec::new();
    for line in 0..LINES.len() {
        let base = (line * SEGMENTS * 2) as u32;
        for s in 0..SEGMENTS - 1 {
            let a = base + (s * 2) as u32;
            indices.extend_from_slice(&[a, a + 1, a + 3, a, a + 3, a + 2]);
        }
    }
    mesh.insert_indices(Indices::U32(indices));
    let handle = meshes.add(mesh);

    let rope = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.9,
        double_sided: true,
        cull_mode: None,
        ..default()
    });

    commands.spawn((
        Mesh3d(handle.clone()),
        MeshMaterial3d(rope),
        Transform::default(),
    ));
    commands.insert_resource(Rigging {
        mesh: handle,
        previous: Controls::default(),
        flash: [0.0; LINES.len()],
    });
}

/// Where a line is made fast at each end, in the hull's own frame, plus how hard it is
/// being pulled.
struct Span {
    from: Vec3,
    to: Vec3,
    /// 0 is dead slack, 1 is bar taut.
    tension: f32,
    /// Rope diameter as drawn. Over-scaled, as rope always is in a game: a true 10 mm
    /// sheet at five metres is two pixels and simply disappears.
    width: f32,
    /// Wire rather than rope: it goes slack without bellying.
    stiff: bool,
    colour: [f32; 4],
}

pub fn update(
    origin: Res<crate::frame::RenderOrigin>,
    sim: Res<Sim>,
    bank: Res<crate::clutch::Bank>,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut rigging: ResMut<Rigging>,
    camera: Query<&Transform, With<MainCamera>>,
) {
    let Ok(view) = camera.single() else {
        return;
    };
    let Some(mut mesh) = meshes.get_mut(&rigging.mesh) else {
        return;
    };

    // Light up whichever line the player just moved. Eight ropes on a screen is eight
    // things nobody has been taught the names of; showing which one answered to the
    // key is how the rig teaches itself.
    {
        let now = &sim.controls;
        let was = rigging.previous;
        let dt = time.delta_secs();
        let moved = |a: f64, b: f64| (a - b).abs() > 1e-4;
        let mut light = |kind: Kind| {
            if let Some(i) = LINES.iter().position(|k| *k == kind) {
                rigging.flash[i] = FLASH;
            }
        };
        if moved(now.main_sheet, was.main_sheet) {
            light(Kind::MainSheet);
        }
        if moved(now.staysail, was.staysail) || moved(now.staysail_sheet, was.staysail_sheet) {
            for part in 0..3 {
                light(Kind::StaysailSheet(part));
            }
        }
        if moved(now.jib_sheet, was.jib_sheet)
            || moved(now.furl, was.furl)
            || moved(now.genoa_car, was.genoa_car)
        {
            for part in 0..2 {
                light(Kind::JibSheet(false, part));
                light(Kind::JibSheet(true, part));
            }
        }
        if moved(now.outhaul, was.outhaul) {
            light(Kind::Outhaul);
        }
        if moved(now.vang, was.vang) {
            light(Kind::Vang);
        }
        if moved(now.reef, was.reef) {
            light(Kind::MainSheet);
            light(Kind::Outhaul);
        }
        for f in &mut rigging.flash {
            *f = (*f - dt).max(0.0);
        }
        rigging.previous = *now;
    }

    let st = &sim.render_state();
    let cfg = sim.config();
    let t = &sim.sim.telemetry;
    let hull = origin.hull_translation(st);
    let attitude = crate::frame::attitude(st);

    // Which side the wind is on decides which sheet is working and which shroud is
    // going soft.
    let lee = -(t.awa.signum() as f32);
    let boom = lee * t.main.chord_angle as f32;
    let heel = st.heel as f32;

    // Reference force for normalising sheet loads: the rig's own dynamic pressure at a
    // brisk apparent wind, so "taut" means what it means on a windy day.
    let reference = 0.5 * 1.225 * (10.0_f64).powi(2) * cfg.sail_area();
    let main_load = ((t.main.lift + t.main.drag) / reference).clamp(0.0, 1.0) as f32;
    let jib_load = ((t.jib.lift + t.jib.drag) / reference).clamp(0.0, 1.0) as f32;
    let furled = sim.controls.furl as f32;

    let masthead = Vec3::new(0.0, MAST_HEAD, MAST_FOOT_Z);
    // Swung and lifted, from the one function that knows the spar's attitude — the
    // mainsheet is attached to the end of it, so a sheet drawn to a boom that is not
    // where the boom is would be the same fault this rig keeps finding.
    let (_, boom_lift) = crate::boat::boom_attitude(t);
    let boom_end = Vec3::new(
        boom.sin() * BOOM_LENGTH * boom_lift.cos(),
        BOOM_HEIGHT + BOOM_LENGTH * boom_lift.sin(),
        MAST_FOOT_Z + boom.cos() * BOOM_LENGTH * boom_lift.cos(),
    );
    // Standing rigging is wire and running rigging is rope, and they do not look
    // remotely alike. Everything was the same bright near-white, which read as fishing
    // line strung round the boat.
    //
    // The running lines are also colour-coded, which is not decoration: every boat
    // does it, for exactly the reason it helps here — in a hurry you grab the line by
    // its colour, not by tracing where it goes.
    let wire = [0.62_f32, 0.65, 0.68, 1.0];
    let sheet_blue = [0.34_f32, 0.38, 0.50, 1.0];
    let sheet_sand = [0.76_f32, 0.62, 0.44, 1.0];
    let vang_green = [0.40_f32, 0.52, 0.42, 1.0];
    let outhaul_red = [0.66_f32, 0.40, 0.36, 1.0];

    let spans: Vec<Span> = LINES
        .iter()
        .map(|kind| match *kind {
            Kind::Outhaul => Span {
                // Along the underside of the boom, from the gooseneck out to the clew —
                // the clew the sail is actually drawn with, so the rope ends where the
                // corner is. Which is also what makes the control visible: the tail of
                // it shortens as the clew goes out to the nock.
                from: Vec3::new(0.0, BOOM_HEIGHT - 0.07, MAST_FOOT_Z),
                to: {
                    let reach = crate::boat::main_foot(&cfg, &sim.controls);
                    Vec3::new(
                        boom.sin() * reach,
                        BOOM_HEIGHT - 0.07,
                        MAST_FOOT_Z + boom.cos() * reach,
                    )
                },
                tension: 0.35 + 0.65 * sim.controls.outhaul as f32,
                width: 0.022,
                colour: outhaul_red,
                stiff: false,
            },
            Kind::Bobstay => Span {
                from: crate::boat::sprit_end(cfg.loa as f32),
                // Down to the stem at the waterline, which is the only place that can
                // take the load without levering the deck up.
                to: Vec3::new(0.0, -0.15, -(cfg.loa as f32) * 0.495),
                tension: 1.0,
                width: 0.018,
                colour: wire,
                stiff: true,
            },
            Kind::SpritShroud(starboard) => {
                let side = if starboard { 1.0 } else { -1.0 };
                let z = -(cfg.loa as f32) * 0.42;
                Span {
                    from: crate::boat::sprit_end(cfg.loa as f32),
                    to: Vec3::new(
                        side * crate::deck::deck_half_beam(z),
                        crate::deck::deck_height(z),
                        z,
                    ),
                    tension: 1.0,
                    width: 0.015,
                    colour: wire,
                    stiff: true,
                }
            }
            Kind::Forestay => Span {
                from: Vec3::new(0.0, FORESTAY_HEAD, MAST_FOOT_Z),
                // The stemhead, where the headsail is tacked too — the same fitting,
                // and now the same point. It was using the flat deck constant and so
                // stood a quarter of a metre inside the bow.
                to: crate::boat::genoa_tack(cfg.loa as f32),
                tension: 1.0,
                width: 0.022,
                colour: wire,
                stiff: true,
            },
            Kind::Backstay => Span {
                from: masthead,
                // On the counter, which rises: the flat deck constant put this one
                // forty-five centimetres *below* the deck it is bolted to.
                to: {
                    let z = cfg.loa as f32 * 0.46;
                    Vec3::new(0.0, crate::deck::deck_height(z) + 0.10, z)
                },
                tension: 1.0,
                width: 0.022,
                colour: wire,
                stiff: true,
            },
            Kind::Shroud(starboard) => {
                // The leeward shroud slackens as she heels: the mast leans away from
                // it and the windward one takes the load. Watching this happen is one
                // of the ways you know you are overpowered. The rule lives in
                // `shroud_slack`, because the sails have to lean on this wire where it
                // actually ends up.
                let slack = shroud_slack(starboard, heel, lee);
                let (top, foot) = crate::boat::shroud(cfg.beam as f32, starboard);
                Span {
                    from: top,
                    to: foot,
                    tension: 1.0 - slack,
                    width: 0.020,
                    colour: wire,
                    stiff: true,
                }
            }
            Kind::MainSheet => Span {
                from: boom_end,
                // Down to a strop on the cockpit sole. At deck height it ended in
                // mid-air in the middle of the opening, which is where the cockpit is
                // rather than where anything is bolted.
                to: Vec3::new(0.0, crate::deck::cockpit_sole(), 1.9),
                // A luffing main pulls on nothing, and the sheet says so before the
                // telltales have finished.
                tension: main_load * (1.0 - t.main.coeffs.luffing as f32),
                width: 0.030,
                colour: sheet_blue,
                stiff: false,
            },
            Kind::Vang => Span {
                from: Vec3::new(
                    boom.sin() * BOOM_LENGTH * VANG_ALONG_BOOM * boom_lift.cos(),
                    BOOM_HEIGHT - 0.115 + BOOM_LENGTH * VANG_ALONG_BOOM * boom_lift.sin(),
                    MAST_FOOT_Z + boom.cos() * BOOM_LENGTH * VANG_ALONG_BOOM * boom_lift.cos(),
                ),
                to: Vec3::new(
                    0.0,
                    crate::deck::deck_height(MAST_FOOT_Z + 0.2) + 0.08,
                    MAST_FOOT_Z + 0.2,
                ),
                // Its own setting, not the sail's load. A vang hauled on is bar taut
                // whether or not the main is pulling, and an eased one hangs.
                tension: 0.12 + 0.88 * sim.controls.vang as f32,
                width: 0.025,
                colour: vang_green,
                stiff: false,
            },
            Kind::StaysailSheet(part) => {
                // From a point part way out along the club boom down to a deck eye on
                // the centreline. That is the whole of a self-tacker's rigging: nothing
                // leads aft, nothing is handed across, and tacking takes no hands.
                let set = sim.controls.staysail as f32;
                let club = lee
                    * cfg
                        .staysail
                        .profile
                        .chord_angle(sim.controls.staysail_sheet) as f32;
                let length = crate::boat::staysail_boom_length(cfg.staysail.area as f32);
                // From the boom end, not part way along it: a club boom's sheet is made
                // fast at the end where the leverage is, and the earlier attachment at
                // seven tenths left a stub of rope half a metre long that was hidden
                // under the sail and might as well not have been drawn.
                let at = crate::boat::staysail_boom_end(club, length);
                let eye = Vec3::new(
                    0.0,
                    crate::deck::deck_height(MAST_FOOT_Z - 0.55) + 0.06,
                    MAST_FOOT_Z - 0.55,
                );
                // Three parts between the same two blocks, spread a little so they read
                // as a tackle and not as one rope drawn three times.
                let across = (at - eye).cross(Vec3::Y).normalize_or_zero() * 0.035;
                let spread = across * (part as f32 - 1.0);
                // With the sail handed there is no tackle on deck: it comes off with the
                // sail, or at best lies coiled on the boom. Drawn anyway at zero tension
                // it sagged in a long arc right across the foredeck, which reads as a
                // sheet going from nowhere to nothing — and was reported as exactly
                // that.
                let shown = set > 0.02;
                Span {
                    from: if shown { at + spread } else { eye },
                    to: if shown { eye + spread } else { eye },
                    // Bar taut whenever the sail is up, because on a club-footed
                    // staysail this one rope holds the boom against the whole force of
                    // the sail — there is no second sheet and no tackle to share it.
                    // It was drawn at 0.45 of the headsail's load, which hung it in a
                    // bight beside a sail that was plainly working, and a slack rope on
                    // a straining sail reads as a mistake because it is one.
                    tension: (0.55 + 0.45 * jib_load) * set,
                    width: 0.024,
                    colour: sheet_blue,
                    stiff: false,
                }
            }
            Kind::JibSheet(starboard, part) => {
                let side = if starboard { 1.0 } else { -1.0 };
                let working = side * lee > 0.0;
                // Only the working sheet is drawn taut from the clew; a furled genoa
                // has no clew to pull on at all.
                // The same solve the sail uses, so the rope ends where the sail's
                // corner is rather than near it.
                // The sail's own three corners, from the one function that knows them,
                // so the rope ends at the clew rather than near it. It used to solve the
                // clew a second time here with its own typed-in foot length, and when
                // the sail's was replaced by the measured one this copy stayed behind.
                let shape = crate::boat::headsail_shape(
                    &cfg,
                    &sim.controls,
                    if starboard { 1.0 } else { -1.0 },
                    t.jib.chord_angle as f32,
                );
                let clew = shape.clew;
                // A flying sail is sheeted to a block on the quarter, right aft, not to
                // the genoa track — which is why the car does nothing when one is up.
                let car_at = if sim.controls.headsail == sail_core::state::Headsail::Genoa {
                    crate::boat::genoa_car_at(sim.controls.genoa_car as f32, starboard)
                } else {
                    Vec3::new(
                        if starboard { 1.15 } else { -1.15 },
                        crate::deck::deck_height(3.9) + 0.07,
                        3.9,
                    )
                };
                // The boom lies across the sheet's path on any sort of reach, and a sheet
                // that goes through a spar is the one thing on this boat nobody would
                // forgive. So the rope is routed over it: find where the straight run
                // passes the boom, and if it passes close enough to be touching, lift
                // the join onto the top of the spar. Which is where it sits on a real
                // boat — the sheet rides over the boom and chafes there, and on a gybe
                // it is the thing that catches.
                let gooseneck = Vec3::new(0.0, BOOM_HEIGHT, MAST_FOOT_Z);
                let flying = sim.controls.headsail != sail_core::state::Headsail::Genoa;
                // Whether there is actually something for the rope to be led round. If
                // there is not, the sheet is *one* rope and has to be drawn as one:
                // splitting a straight run in half and sagging each half about its own
                // chord puts a kink at the join, because two catenaries of different
                // lengths do not meet at the same angle. The fix is not to bend the
                // rope better; it is not to cut it in two.
                let mut led = true;
                let join = if flying {
                    // A sail set flying sheets *outside* everything — outside the cap
                    // shrouds, outside the lifelines, aft to a block on the quarter.
                    // There is no other way to lead it: inside the shrouds the sheet
                    // would be hard against the wire the moment the sail was eased, and
                    // the sail itself is tacked a metre and a half ahead of the stem
                    // precisely so that it can fly clear of all of that.
                    let side = if starboard { 1.0 } else { -1.0 };
                    let z = MAST_FOOT_Z + 0.4;
                    // Outboard of the shrouds *or* of the straight run, whichever is
                    // further out — and that second half is the correction. A fixed
                    // waypoint abeam the mast sits inside the line from a clew flying
                    // three metres off the boat, so the rope bent in towards the
                    // centreline and then back out again, which no sheet does. A lead
                    // point can hold a rope out. It cannot pull it in.
                    // Only if the straight run would cut through the shrouds. A lead
                    // point is a thing a rope is pulled *around*; it has no business
                    // existing when the rope already passes outside everything, and
                    // putting one there anyway kinked the sheet in towards the
                    // centreline on the way to the sail and out again — reported from
                    // the deck, twice, and right both times.
                    let f = ((z - clew.z) / (car_at.z - clew.z)).clamp(0.0, 1.0);
                    let straight = clew.lerp(car_at, f);
                    let clears = cfg.beam as f32 * 0.5 + 0.18;
                    if straight.x.abs() >= clears {
                        // Already outboard of the wire: nothing to lead it round.
                        led = false;
                        straight
                    } else {
                        Vec3::new(
                            side * clears,
                            crate::deck::deck_height(z) + 0.35,
                            z,
                        )
                    }
                } else {
                    let spar = boom_end - gooseneck;
                    let run = car_at - clew;
                    // Closest approach between the two segments, the cheap way: walk the
                    // sheet and take the point nearest the spar.
                    let mut best = (f32::MAX, 0.5_f32, Vec3::ZERO);
                    for i in 0..=12 {
                        let f = i as f32 / 12.0;
                        let p = clew + run * f;
                        let t = ((p - gooseneck).dot(spar) / spar.length_squared()).clamp(0.0, 1.0);
                        let on_spar = gooseneck + spar * t;
                        let d = (p - on_spar).length();
                        if d < best.0 {
                            best = (d, f, on_spar);
                        }
                    }
                    if best.0 < 0.30 && best.1 > 0.05 && best.1 < 0.95 {
                        best.2 + Vec3::Y * 0.09
                    } else {
                        led = false;
                        clew.lerp(car_at, 0.5)
                    }
                };
                let (a, b) = match (part, led) {
                    // Led round something: two ropes, two sags, a real bend at the block.
                    (0, true) => (clew, join),
                    (_, true) => (join, car_at),
                    // Nothing in the way: the whole run is the second part and the first
                    // collapses onto the clew, so one curve is drawn and it has no corner.
                    (0, false) => (clew, clew),
                    _ => (clew, car_at),
                };
                Span {
                    // Only the working sheet is drawn at all now. The lazy one used to
                    // be shown lying across the foredeck in a bight — which is true of a
                    // real boat and reads as a mistake on a screen, because a slack rope
                    // drawn beside a straining one looks like the straining one has gone
                    // slack too. Asked for from the deck, and the deck is right: show
                    // what is working.
                    from: if working { a } else { car_at },
                    // Led through the car itself, so the rope and the fitting cannot
                    // disagree about where the sheet is being turned.
                    to: if working { b } else { car_at },
                    tension: if working {
                        jib_load * (1.0 - t.jib.coeffs.luffing as f32) * (1.0 - furled)
                    } else {
                        0.0
                    },
                    width: 0.027,
                    colour: [
                        sheet_sand[0],
                        sheet_sand[1],
                        sheet_sand[2],
                        // Fades out with the sail as it rolls away.
                        (1.0 - furled).clamp(0.0, 1.0),
                    ],
                    stiff: false,
                }
            }
        })
        .collect();

    let mut positions = Vec::with_capacity(LINES.len() * SEGMENTS * 2);
    let mut normals = Vec::with_capacity(LINES.len() * SEGMENTS * 2);
    let mut colours = Vec::with_capacity(LINES.len() * SEGMENTS * 2);

    for (index, span) in spans.iter().enumerate() {
        // Two kinds of highlight. A flash says "this answered your key"; a steady
        // glow says "this is the one on the winch", which is the thing you most need
        // to know before pulling anything.
        let flash = (rigging.flash[index] / FLASH).clamp(0.0, 1.0);
        let on_the_winch = matches!(
            (bank.line(), LINES[index]),
            (crate::clutch::Line::Outhaul, Kind::Outhaul)
                | (crate::clutch::Line::Vang, Kind::Vang)
                | (crate::clutch::Line::GenoaCar, Kind::JibSheet(..))
                | (crate::clutch::Line::Furl, Kind::JibSheet(..))
                | (crate::clutch::Line::Reef, Kind::MainSheet)
        );
        let lit = flash.max(if on_the_winch { 0.34 } else { 0.0 });
        let from = hull + attitude * span.from;
        let to = hull + attitude * span.to;
        let length = (to - from).length().max(0.01);
        // Slack rope sags; taut rope does not. The scale is the span's own length, so
        // a long sheet droops further than a short vang at the same slackness.
        let sag = line_sag(length, span.tension, span.stiff);

        for i in 0..SEGMENTS {
            let f = i as f32 / (SEGMENTS - 1) as f32;
            let point = curve(from, to, sag, f);
            // Local tangent, not the chord: a sagging sheet curves, and a ribbon built
            // on the straight line between its ends twists visibly at the bottom of
            // the droop.
            let step = 1.0 / (SEGMENTS - 1) as f32;
            let ahead = (f + step).min(1.0);
            let behind = (f - step).max(0.0);
            let along =
                (curve(from, to, sag, ahead) - curve(from, to, sag, behind)).normalize_or_zero();
            // Face the eye: offset perpendicular to both the rope and the view.
            let to_eye = (view.translation - point).normalize_or_zero();
            // A lit line also thickens a little, which reads at a glance even when
            // the whole rig is backlit.
            let side = along.cross(to_eye).normalize_or_zero() * (span.width * (0.5 + lit * 0.45));
            positions.push(point - side);
            positions.push(point + side);
            normals.push(to_eye);
            normals.push(to_eye);
            let mut colour = span.colour;
            // Brighten towards white and lift the alpha, so even a line that is
            // fading out because its sail is furled still announces itself.
            for c in colour.iter_mut().take(3) {
                *c = (*c + lit * 1.6).min(2.4);
            }
            colour[3] = (colour[3] + lit * 0.7).min(1.0);
            colours.push(colour);
            colours.push(colour);
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
}

/// A point along a sagging line. Parabolic, which over a four-metre span is
/// indistinguishable from a catenary and far cheaper to evaluate.
#[inline]
/// How slack the leeward shroud has gone.
///
/// The mast leans away from it and the windward one takes the load, and watching this
/// happen is one of the ways you know you are overpowered. Shared with the sails,
/// which have to lean on the wire where it actually is.
pub fn shroud_slack(starboard: bool, heel: f32, lee: f32) -> f32 {
    let side = if starboard { 1.0 } else { -1.0 };
    if side * lee > 0.0 {
        (heel.abs() / 28.0_f32.to_radians()).clamp(0.0, 0.85)
    } else {
        0.0
    }
}

/// How far the middle of a span hangs below the straight line between its ends.
///
/// Scaled by the span's own length, so a long sheet droops further than a short vang
/// at the same slackness — and capped hard for standing rigging, because wire is not
/// rope. A slack shroud goes soft and rattles; it does not bow. The uncapped figure
/// put a metre and a half of belly into a leeward cap shroud at nineteen degrees of
/// heel, which is both absurd and the reason the wire appeared to cut through a sail
/// that was being held clear of the straight line between its ends.
pub fn line_sag(length: f32, tension: f32, stiff: bool) -> f32 {
    let soft = (1.0 - tension).clamp(0.0, 1.0).powf(1.4) * length * 0.16;
    if stiff {
        soft.min(0.09)
    } else {
        soft
    }
}

pub fn curve(from: Vec3, to: Vec3, sag: f32, f: f32) -> Vec3 {
    from.lerp(to, f) - Vec3::Y * (sag * 4.0 * f * (1.0 - f))
}
