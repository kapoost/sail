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
const MAST_HEAD: f32 = DECK + 15.2;
const BOOM_HEIGHT: f32 = DECK + 1.25;
const BOOM_LENGTH: f32 = 4.2;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Forestay,
    Backstay,
    Shroud(bool),
    MainSheet,
    Vang,
    JibSheet(bool),
}

/// All the lines, in the order they are packed into the mesh.
const LINES: [Kind; 8] = [
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

#[derive(Resource)]
pub struct Rigging {
    mesh: Handle<Mesh>,
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
    commands.insert_resource(Rigging { mesh: handle });
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
    colour: [f32; 4],
}

pub fn update(
    sim: Res<Sim>,
    mut meshes: ResMut<Assets<Mesh>>,
    rigging: Res<Rigging>,
    camera: Query<&Transform, With<MainCamera>>,
) {
    let Ok(view) = camera.single() else {
        return;
    };
    let Some(mut mesh) = meshes.get_mut(&rigging.mesh) else {
        return;
    };

    let st = &sim.sim.state;
    let cfg = sim.config();
    let t = &sim.sim.telemetry;
    let hull = crate::frame::hull_translation(st);
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
    let wire = [0.86_f32, 0.88, 0.90, 1.0];
    let rope_colour = [0.93_f32, 0.90, 0.80, 1.0];

    let spans: Vec<Span> = LINES
        .iter()
        .map(|kind| match *kind {
            Kind::Forestay => Span {
                from: masthead,
                to: Vec3::new(0.0, DECK + 0.25, -(cfg.loa as f32) * 0.46),
                tension: 1.0,
                width: 0.022,
                colour: wire,
            },
            Kind::Backstay => Span {
                from: masthead,
                to: Vec3::new(0.0, DECK + 0.20, (cfg.loa as f32) * 0.46),
                tension: 1.0,
                width: 0.022,
                colour: wire,
            },
            Kind::Shroud(starboard) => {
                let side = if starboard { 1.0 } else { -1.0 };
                // The leeward shroud slackens as she heels: the mast leans away from
                // it and the windward one takes the load. Watching this happen is one
                // of the ways you know you are overpowered.
                let leeward = side * lee > 0.0;
                let slack = if leeward {
                    (heel.abs() / rad(28.0) as f32).clamp(0.0, 0.85)
                } else {
                    0.0
                };
                Span {
                    from: masthead,
                    to: Vec3::new(side * cfg.beam as f32 * 0.45, DECK, MAST_FOOT_Z + 0.4),
                    tension: 1.0 - slack,
                    width: 0.020,
                    colour: wire,
                }
            }
            Kind::MainSheet => Span {
                from: boom_end,
                to: Vec3::new(0.0, DECK, 1.7),
                // A luffing main pulls on nothing, and the sheet says so before the
                // telltales have finished.
                tension: main_load * (1.0 - t.main.coeffs.luffing as f32),
                width: 0.034,
                colour: rope_colour,
            },
            Kind::Vang => Span {
                from: Vec3::new(
                    boom.sin() * BOOM_LENGTH * 0.42,
                    BOOM_HEIGHT,
                    MAST_FOOT_Z + boom.cos() * BOOM_LENGTH * 0.42,
                ),
                to: Vec3::new(0.0, DECK + 0.1, MAST_FOOT_Z + 0.2),
                tension: 0.55 + 0.45 * main_load,
                width: 0.028,
                colour: rope_colour,
            },
            Kind::JibSheet(starboard) => {
                let side = if starboard { 1.0 } else { -1.0 };
                let working = side * lee > 0.0;
                // Only the working sheet is drawn taut from the clew; a furled genoa
                // has no clew to pull on at all.
                let clew = Vec3::new(
                    jib_angle.sin() * 4.9 * (1.0 - furled),
                    DECK + 0.25,
                    -(cfg.loa as f32) * 0.46 + jib_angle.cos() * 4.9 * (1.0 - furled),
                );
                Span {
                    // The lazy sheet runs from the same clew but is not pulling, so it
                    // falls away in a bight across the foredeck.
                    from: clew,
                    to: Vec3::new(side * cfg.beam as f32 * 0.38, DECK, 1.0),
                    tension: if working {
                        jib_load * (1.0 - t.jib.coeffs.luffing as f32) * (1.0 - furled)
                    } else {
                        0.0
                    },
                    width: 0.030,
                    colour: [
                        rope_colour[0],
                        rope_colour[1],
                        rope_colour[2],
                        // Fades out with the sail as it rolls away.
                        (1.0 - furled).clamp(0.0, 1.0),
                    ],
                }
            }
        })
        .collect();

    let mut positions = Vec::with_capacity(LINES.len() * SEGMENTS * 2);
    let mut normals = Vec::with_capacity(LINES.len() * SEGMENTS * 2);
    let mut colours = Vec::with_capacity(LINES.len() * SEGMENTS * 2);

    for span in &spans {
        let from = hull + attitude * span.from;
        let to = hull + attitude * span.to;
        let length = (to - from).length().max(0.01);
        // Slack rope sags; taut rope does not. The scale is the span's own length, so
        // a long sheet droops further than a short vang at the same slackness.
        let sag = (1.0 - span.tension).powf(1.4) * length * 0.16;

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
            let side = along.cross(to_eye).normalize_or_zero() * (span.width * 0.5);
            positions.push(point - side);
            positions.push(point + side);
            normals.push(to_eye);
            normals.push(to_eye);
            colours.push(span.colour);
            colours.push(span.colour);
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
}

/// A point along a sagging line. Parabolic, which over a four-metre span is
/// indistinguishable from a catenary and far cheaper to evaluate.
#[inline]
fn curve(from: Vec3, to: Vec3, sag: f32, f: f32) -> Vec3 {
    from.lerp(to, f) - Vec3::Y * (sag * 4.0 * f * (1.0 - f))
}
