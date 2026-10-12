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

const DECK: f32 = 0.95;
const MAST_FOOT_Z: f32 = -1.8;
pub const MAST_HEAD: f32 = DECK + 15.2;
/// The boom, high enough to stand under.
///
/// It was a quarter of a metre *below* the helmsman's eye, which put it across the
/// view like a handrail — and would have taken his head off in a gybe. A cruising boom
/// clears a standing helmsman, barely, and you duck anyway.
const BOOM_HEIGHT: f32 = DECK + 1.95;
/// Boom length, and it is not a free choice.
///
/// It is the mainsail's foot, so it is fixed by the sail's area and its luff: a 46
/// square metre main on a 15.2 metre luff has an average chord of 3.0 m, and with the
/// roach a modern main carries — call it fifteen percent over the flat triangle — the
/// foot has to be 5.3 m. It was 4.2, which drew a main of thirty square metres on a
/// boat whose physics was pulling with forty-six.
///
/// That mattered more than it sounds. The genoa was drawn a third too *large* at the
/// same time, so on deck the headsail looked nearly the size of the main when it is in
/// fact half of it — the one proportion a sailor reads a rig by, and it was reversed.
pub const BOOM_LENGTH: f32 = 5.3;
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
    JibSheet(bool),
}

/// All the lines, in the order they are packed into the mesh.
const LINES: [Kind; 12] = [
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
    Kind::JibSheet(false),
    Kind::JibSheet(true),
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
        if moved(now.jib_sheet, was.jib_sheet)
            || moved(now.furl, was.furl)
            || moved(now.genoa_car, was.genoa_car)
        {
            light(Kind::JibSheet(false));
            light(Kind::JibSheet(true));
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
    let jib_angle = lee * t.jib.chord_angle as f32;
    let heel = st.heel as f32;

    // Reference force for normalising sheet loads: the rig's own dynamic pressure at a
    // brisk apparent wind, so "taut" means what it means on a windy day.
    let reference = 0.5 * 1.225 * (10.0_f64).powi(2) * cfg.sail_area();
    let main_load = ((t.main.lift + t.main.drag) / reference).clamp(0.0, 1.0) as f32;
    let jib_load = ((t.jib.lift + t.jib.drag) / reference).clamp(0.0, 1.0) as f32;
    let furled = sim.controls.furl as f32;

    let masthead = Vec3::new(0.0, MAST_HEAD, MAST_FOOT_Z);
    let boom_end = Vec3::new(
        boom.sin() * BOOM_LENGTH,
        BOOM_HEIGHT,
        MAST_FOOT_Z + boom.cos() * BOOM_LENGTH,
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
                // Along the underside of the boom, from the gooseneck out to the clew.
                // Drawn mostly so the control has something to show for itself: with
                // no line on screen, hauling the foot flat was invisible.
                from: Vec3::new(0.0, BOOM_HEIGHT - 0.07, MAST_FOOT_Z),
                to: Vec3::new(
                    boom.sin() * BOOM_LENGTH,
                    BOOM_HEIGHT - 0.07,
                    MAST_FOOT_Z + boom.cos() * BOOM_LENGTH,
                ),
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
                from: masthead,
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
                    boom.sin() * BOOM_LENGTH * VANG_ALONG_BOOM,
                    BOOM_HEIGHT - 0.115,
                    MAST_FOOT_Z + boom.cos() * BOOM_LENGTH * VANG_ALONG_BOOM,
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
            Kind::JibSheet(starboard) => {
                let side = if starboard { 1.0 } else { -1.0 };
                let working = side * lee > 0.0;
                // Only the working sheet is drawn taut from the clew; a furled genoa
                // has no clew to pull on at all.
                // The same solve the sail uses, so the rope ends where the sail's
                // corner is rather than near it.
                let out = 1.0 - furled;
                let tack = crate::boat::genoa_tack(cfg.loa as f32);
                let car_at = crate::boat::genoa_car_at(sim.controls.genoa_car as f32, starboard);
                let clew = crate::boat::genoa_clew(
                    tack,
                    jib_angle,
                    4.9 * out,
                    car_at,
                    crate::boat::sheet_length(sim.controls.jib_sheet as f32),
                );
                Span {
                    // The lazy sheet runs from the same clew but is not pulling, so it
                    // falls away in a bight across the foredeck.
                    from: clew,
                    // Led through the car itself, so the rope and the fitting cannot
                    // disagree about where the sheet is being turned.
                    to: car_at,
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
                | (crate::clutch::Line::GenoaCar, Kind::JibSheet(_))
                | (crate::clutch::Line::Furl, Kind::JibSheet(_))
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
