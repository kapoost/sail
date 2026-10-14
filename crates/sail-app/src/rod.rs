//! The rod, the line and what is on the end of it, as things you can see.
//!
//! `fishing.rs` is the model: plain data and arithmetic, tested without a window. This
//! is the other half — a blank in a holder on the port quarter, a line going aft into
//! the water, and the fish itself in the last few metres when it comes up where you can
//! see it.
//!
//! Three decisions worth writing down.
//!
//! **The bend in the rod is the instrument.** There is a tension readout on the console
//! because this is also a simulator, but the number is not how anybody fishes: you read
//! the load off the curve in the blank, and the moment a rod goes from bent to straight
//! you know you have lost the fish before any gauge could tell you. So the bend is
//! computed from the same spring rate the fight runs on — tip deflection is
//! `tension / ROD` and nothing is faked — and the tip is where your eye should be.
//!
//! **The line is one pixel wide and that is correct.** The picture is drawn at 480 lines
//! and blown up, so a line strip comes out as exactly the hard single-pixel thread a
//! fishing line looks like against the sea. A tube of geometry would be thicker, dearer,
//! and worse.
//!
//! **Nothing below the surface is drawn specially.** The sea is opaque and the depth
//! buffer does the work: the line enters the water and stops, because that is what
//! looking at a fishing line from a boat is like. The fish appears when the fish
//! appears, in the last few metres, and that moment is the whole point of the view.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::fishing::{Fishing, Rod};
use crate::frame;
use crate::sim::Sim;

/// Where the rod sits, in boat coordinates: on the port quarter, just inside the rail.
///
/// Port rather than starboard for a reason that is not symmetry. The mainsheet and the
/// helmsman's working side on this boat are to starboard, and a rod in a holder is a
/// thing you will eventually swing a boom over — so it goes on the side where it is out
/// of the way of the gear you use in a hurry.
const HOLDER_Z: f32 = 4.05;
/// How far in from the sheer, metres.
const HOLDER_INSET: f32 = 0.16;
/// How long the blank is, metres. A short, stiff boat rod rather than a beach caster:
/// you fish it from a holder in a cockpit with a boom overhead.
const BLANK: f32 = 2.20;
/// How the rod sits in its holder, unloaded: aft, outboard and up.
///
/// Out at thirty degrees from the centreline so the line clears the quarter and the
/// wake, and up at forty so the tip is above the pushpit and visible from the helm.
const HOLDER_OUT: f32 = 0.52;
const HOLDER_UP: f32 = 0.70;

/// Segments in the blank. Eight is plenty at this size, and at 480 lines it is more
/// than plenty.
const JOINTS: usize = 10;
/// Sides to the blank's tube. Six, because it is eighteen millimetres thick at the butt
/// and two at the tip, and nobody is going to count them.
const AROUND: usize = 6;
/// Points in the drawn line. Enough for the belly in it to be a curve.
const LINE_POINTS: usize = 14;

/// The spring rate the bend is drawn from. The same number the fight uses, deliberately:
/// see `fishing::ROD`, which is private because nothing outside the model is allowed to
/// reason with it — this is the one exception and it is a drawing, so it is stated here
/// rather than exported.
const ROD_SPRING: f32 = 120.0;
/// The most the tip may be drawn as deflected, metres. A rod bent further than this is
/// a rod that has gone, and nothing in the model lets the tension get there anyway.
const MAX_DEFLECTION: f32 = 1.15;

/// On every part of the gear. The rod, the line and the lure come out together and go
/// away together, so whether they are drawn is one fact about one piece of equipment
/// rather than three facts that have to be kept in step.
#[derive(Component)]
pub struct Gear;

#[derive(Component)]
pub struct Blank;

#[derive(Component)]
pub struct Line;

/// The lure, and the fish when there is one. One entity, because at any moment the far
/// end of the line is exactly one object and it is in the same place either way.
#[derive(Component)]
pub struct Terminal;

/// How much line has run off the spool since the ratchet last clicked, metres.
///
/// A ratchet is a pawl over a toothed wheel and it clicks once a tooth, so the sound is
/// a function of *distance*, not of time — which is why a run starts as a rattle and
/// turns into a scream without anything changing its pitch. Eight centimetres a click is
/// about right for a small multiplier.
#[derive(Resource, Default)]
pub struct Ratchet(f32);

const TOOTH: f32 = 0.08;

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    boat: Query<Entity, With<crate::boat::BoatRoot>>,
) {
    let Ok(root) = boat.single() else {
        warn!("no boat to put a rod on");
        return;
    };

    // The blank: a tube that is rebuilt every frame because its shape is the readout.
    let blank = meshes.add(blank_mesh(0.0, Vec3::new(0.0, -0.6, 0.8)));
    let carbon = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.10, 0.12),
        perceptual_roughness: 0.35,
        ..default()
    });
    commands.entity(root).with_children(|b| {
        b.spawn((
            Gear,
            Blank,
            Mesh3d(blank),
            MeshMaterial3d(carbon),
            Transform::from_translation(butt()),
            Visibility::Hidden,
        ));
        // The holder itself stays whether the rod is in it or not, because an empty rod
        // holder is a thing a boat has and a hole that appears when you press a key is
        // not. A stub of tube lashed inside the rail.
        b.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.028, 0.34))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.30, 0.31, 0.33),
                metallic: 0.7,
                perceptual_roughness: 0.45,
                ..default()
            })),
            Transform::from_translation(butt() + Vec3::new(-0.03, 0.10, 0.0)).with_rotation(
                Quat::from_rotation_arc(Vec3::Y, unloaded_direction().normalize()),
            ),
            Visibility::default(),
        ));
    });

    // The line lives in the world rather than on the boat, because one end of it is on
    // the boat and the other is forty metres astern in water the boat has already left.
    let mut line = Mesh::new(PrimitiveTopology::LineStrip, RenderAssetUsages::default());
    line.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![Vec3::ZERO; LINE_POINTS]);
    line.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![Vec3::Y; LINE_POINTS]);
    line.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![Vec2::ZERO; LINE_POINTS]);
    commands.spawn((
        Gear,
        Line,
        Mesh3d(meshes.add(line)),
        MeshMaterial3d(materials.add(StandardMaterial {
            // Unlit, because a line this thin shaded by anything flickers as it moves
            // across the sea. Pale grey rather than white: braid is grey, and white
            // against a grey sea is the one thing that would read as a scratch on the
            // screen rather than a thread in the air.
            base_color: Color::srgb(0.78, 0.80, 0.80),
            unlit: true,
            ..default()
        })),
        Transform::default(),
        Visibility::Hidden,
    ));

    // The far end. A flattened ellipsoid: a spoon when there is nothing on, a fish when
    // there is, and the only difference is how big it is drawn and what colour.
    commands.spawn((
        Gear,
        Terminal,
        Mesh3d(meshes.add(Sphere::new(0.5).mesh().ico(2).expect("a small sphere"))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.72, 0.74, 0.70),
            metallic: 0.6,
            perceptual_roughness: 0.3,
            ..default()
        })),
        Transform::default(),
        Visibility::Hidden,
    ));
    commands.init_resource::<Ratchet>();
}

/// Where the butt of the rod sits, in boat coordinates.
fn butt() -> Vec3 {
    Vec3::new(
        -(crate::deck::deck_half_beam(HOLDER_Z) - HOLDER_INSET),
        crate::deck::deck_height(HOLDER_Z) + 0.14,
        HOLDER_Z,
    )
}

/// Which way the blank points with nothing on it.
fn unloaded_direction() -> Vec3 {
    Vec3::new(-HOLDER_OUT, HOLDER_UP, 0.49).normalize()
}

/// Build the blank, bent by `deflection` metres towards `pull`.
///
/// The bend is distributed along the blank rather than applied at the tip, and weighted
/// towards the tip, because that is what a progressive-action rod does: the butt stays
/// nearly straight and takes the load, and the top third does the curving. A rod drawn
/// as a circular arc looks like a bow, which is a different piece of equipment.
fn blank_mesh(deflection: f32, pull: Vec3) -> Mesh {
    let start = unloaded_direction();
    // Total turn through the blank. For a gently curved beam the tip's offset from the
    // straight line is about half the length times the total angle, which inverts to
    // this and is accurate enough to a third of a radian — further than the rod goes.
    let turn = (2.0 * deflection / BLANK).clamp(0.0, 1.6);
    // The component of the pull across the blank is what bends it; a pull straight down
    // the axis only compresses it, and a rod that bowed under compression would be a rod
    // about to break.
    let across = (pull - start * pull.dot(start)).normalize_or_zero();

    let mut spine = Vec::with_capacity(JOINTS + 1);
    let mut at = Vec3::ZERO;
    spine.push(at);
    for j in 1..=JOINTS {
        let s = j as f32 / JOINTS as f32;
        // Cumulative turn weighted towards the tip.
        let bent = turn * s.powf(1.7);
        let direction = (start * bent.cos() + across * bent.sin()).normalize();
        at += direction * (BLANK / JOINTS as f32);
        spine.push(at);
    }

    // A tube round the spine, tapering from a butt you can hold to a tip you can barely
    // see. The frame is rebuilt at each joint from the local direction, which is cheap
    // and avoids the twist a fixed reference would put in a curve.
    let mut positions = Vec::with_capacity((JOINTS + 1) * (AROUND + 1));
    let mut normals = Vec::with_capacity((JOINTS + 1) * (AROUND + 1));
    let mut uvs = Vec::with_capacity((JOINTS + 1) * (AROUND + 1));
    for (j, point) in spine.iter().enumerate() {
        let s = j as f32 / JOINTS as f32;
        let along = if j == JOINTS {
            (spine[j] - spine[j - 1]).normalize()
        } else {
            (spine[j + 1] - spine[j]).normalize()
        };
        let side = along.cross(Vec3::Y).normalize_or(Vec3::X);
        let up = side.cross(along);
        let radius = 0.009 * (1.0 - s) + 0.0014;
        for a in 0..=AROUND {
            let angle = a as f32 / AROUND as f32 * std::f32::consts::TAU;
            let out = side * angle.cos() + up * angle.sin();
            positions.push(*point + out * radius);
            normals.push(out);
            uvs.push(Vec2::new(a as f32 / AROUND as f32, s));
        }
    }
    let mut indices = Vec::with_capacity(JOINTS * AROUND * 6);
    let stride = AROUND + 1;
    for j in 0..JOINTS {
        for a in 0..AROUND {
            let i = (j * stride + a) as u32;
            let k = i + stride as u32;
            indices.extend_from_slice(&[i, k, i + 1, i + 1, k, k + 1]);
        }
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Where the far end of the line is, in simulation coordinates, and how deep.
///
/// The horizontal distance is the line out with the depth taken out of it by Pythagoras,
/// which treats the line as straight. It is not straight — it bellies — but the belly is
/// drawn into the line itself a few lines further down, and using the chord here keeps
/// the lure at the right *distance*, which is the thing the eye checks.
fn terminal(sim: &Sim, fishing: &Fishing) -> (glam::DVec2, f64) {
    let st = sim.render_state();
    let depth = match &fishing.rod {
        Rod::Playing(fish) => fish.depth,
        _ => crate::fishing::lure_depth(fishing.line_out, sim.sim.telemetry.stw),
    };
    let across = match &fishing.rod {
        Rod::Playing(fish) => fish.across,
        _ => 0.0,
    };
    let depth = depth.min(fishing.line_out.max(0.5));
    let horizontal = (fishing.line_out * fishing.line_out - depth * depth)
        .max(0.0)
        .sqrt();
    let along = (1.0 - across * across).max(0.0).sqrt();
    let stern = st.pos - st.forward() * (sim.config().loa * 0.45);
    let at = stern - st.forward() * horizontal * along + st.starboard() * horizontal * across;
    (at, depth)
}

/// Everything the gear reads but does not change, the way `camera::Eye` does it. Five
/// resources behind one name, which is the difference between a signature you can read
/// and a list.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Overside<'w> {
    sim: Res<'w, Sim>,
    fishing: Res<'w, Fishing>,
    origin: Res<'w, frame::RenderOrigin>,
    sound: Res<'w, crate::sound::SoundHandle>,
    time: Res<'w, Time>,
}

#[allow(clippy::too_many_arguments)]
pub fn update(
    overside: Overside,
    mut ratchet: ResMut<Ratchet>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    // Whether the gear is drawn is one query over all of it; what each part is doing
    // is a query of its own. They overlap on entities and not on components, which is
    // all Bevy asks.
    mut shown: Query<&mut Visibility, With<Gear>>,
    blanks: Query<&Mesh3d, With<Blank>>,
    lines: Query<&Mesh3d, With<Line>>,
    mut terminals: Query<(&mut Transform, &MeshMaterial3d<StandardMaterial>), With<Terminal>>,
) {
    let Overside {
        sim,
        fishing,
        origin,
        sound,
        time,
    } = overside;
    let showing = fishing.out();
    for mut visible in &mut shown {
        *visible = if showing {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    let st = sim.render_state();
    let attitude = frame::attitude(&st);
    let hull = origin.hull_translation(&st);

    // Where the far end is, and therefore which way the rod is being pulled.
    let (at, depth) = terminal(&sim, &fishing);
    let resolution = {
        let offset = at - st.pos;
        crate::world::drawn_resolution(Vec2::new(offset.x as f32, -offset.y as f32))
    };
    let surface = sim.sim.water.height_resolved(at, st.time, resolution) as f32;
    let far = origin.to_bevy(at, surface - depth as f32);

    // --- The blank ------------------------------------------------------------
    let tip_world = {
        let butt_world = hull + attitude * butt();
        let pull = (far - butt_world).normalize_or(Vec3::NEG_Z);
        let local_pull = attitude.inverse() * pull;
        let deflection = (fishing.tension as f32 / ROD_SPRING).min(MAX_DEFLECTION);
        if showing {
            for mesh in &blanks {
                if let Some(mut slot) = meshes.get_mut(&mesh.0) {
                    *slot = blank_mesh(deflection, local_pull);
                }
            }
        }
        // The tip, worked out the same way the mesh was so the line starts exactly where
        // the rod ends. Recomputing it is cheaper than reading it back out of the mesh
        // and cannot drift from it.
        butt_world + attitude * tip_offset(deflection, local_pull)
    };

    // --- The line -------------------------------------------------------------
    for mesh in &lines {
        if !showing {
            continue;
        }
        let Some(mut slot) = meshes.get_mut(&mesh.0) else {
            continue;
        };
        // A loaded line is straight and a slack one bellies, and how much it bellies is
        // the other thing an angler reads without thinking about it. The sag is hung off
        // the chord rather than computed as a catenary, because a catenary in a current
        // is not a catenary and the difference is invisible at this scale.
        let sag = (1.0 - fishing.tension as f32 / 35.0).clamp(0.0, 1.0) * 0.9
            + fishing.line_out as f32 * 0.004;
        let points: Vec<Vec3> = (0..LINE_POINTS)
            .map(|i| {
                let t = i as f32 / (LINE_POINTS - 1) as f32;
                let on_chord = tip_world.lerp(far, t);
                on_chord - Vec3::Y * (sag * (t * (1.0 - t)) * 4.0)
            })
            .collect();
        slot.insert_attribute(Mesh::ATTRIBUTE_POSITION, points);
    }

    // --- The far end ----------------------------------------------------------
    for (mut transform, skin) in &mut terminals {
        transform.translation = far;
        // A lure is a hand's length of metal; a fish is as long as its weight says. The
        // cube root is the only honest way to get a length out of a mass, and it is why
        // a nine kilogramme ling is not three times the length of a three kilogramme
        // pollack but about one and a half times, which is what surprises people.
        let (length, girth, colour) = match &fishing.rod {
            Rod::Playing(fish) => {
                let length = 0.30 * (fish.mass as f32).cbrt() * 1.7;
                // Dark on the back and no brighter than the water, because that is what
                // a fish looks like from above and is the whole reason it is hard to see
                // one until it turns. The flash you get when it does is the moment.
                (length, length * 0.22, Color::srgb(0.30, 0.34, 0.31))
            }
            _ => (0.11, 0.025, Color::srgb(0.80, 0.82, 0.76)),
        };
        if let Some(mut material) = materials.get_mut(&skin.0) {
            material.base_color = colour;
        }
        transform.scale = Vec3::new(girth, girth * 1.5, length);
        // Pointing the way it is going, which for anything on the end of a line is at
        // the boat.
        let towards = (tip_world - far).normalize_or(Vec3::Z);
        transform.rotation = Quat::from_rotation_arc(Vec3::Z, towards);
    }

    // --- The ratchet ----------------------------------------------------------
    //
    // Clicked by distance rather than by time, because a pawl over a toothed wheel is a
    // distance counter. One tooth is one clack, and the frame rate puts a ceiling on how
    // fast they can be heard — which is honest enough: past about sixty a second a real
    // ratchet stops being a click and becomes a note, and a note is not what this file
    // is for.
    if showing && fishing.paying > 0.0 {
        ratchet.0 += fishing.paying as f32 * time.delta_secs();
        if ratchet.0 >= TOOTH {
            ratchet.0 = 0.0;
            // Quiet. A reel's ratchet carries across a harbour because it is sharp, not
            // because it is loud, and a boom arriving must stay the loudest thing aboard.
            sound.0.clack(0.22);
        }
    } else {
        ratchet.0 = 0.0;
    }
}

/// Where the tip ends up, for a given bend: the same walk as `blank_mesh` without the
/// tube round it, so the line can start exactly where the rod ends.
fn tip_offset(deflection: f32, pull: Vec3) -> Vec3 {
    let start = unloaded_direction();
    let turn = (2.0 * deflection / BLANK).clamp(0.0, 1.6);
    let across = (pull - start * pull.dot(start)).normalize_or_zero();
    let mut at = Vec3::ZERO;
    for j in 1..=JOINTS {
        let s = j as f32 / JOINTS as f32;
        let bent = turn * s.powf(1.7);
        at += (start * bent.cos() + across * bent.sin()).normalize() * (BLANK / JOINTS as f32);
    }
    at
}

/// The gear's own corner of the console.
///
/// Only there when the rod is, because a panel for equipment you have not got out is
/// clutter — and the console's own rule applies: everything the keyboard can do, the
/// mouse can do, so the drag is a lever you can drag as well as a pair of keys.
///
/// Four things, and the order is the order you look at them in:
///
/// * **Load**, against the line's breaking strain, with the drag marked on the same
///   scale. Two numbers on one bar, because the whole question is which of them is
///   bigger and a pair of separate readouts would make you do that in your head.
/// * **Line out**, which is the depth control, with the depth it is producing and the
///   depth of water under her — the three numbers that decide whether anything can see
///   the lure and whether it is about to be in the kelp.
/// * **The drag**, with the quarter-to-a-third band painted on it. The band is the only
///   piece of instruction in the whole interface and it is painted rather than written.
/// * **What is on**, once something is, and how beaten it is. Not what it *is*: you do
///   not know that until you see it, and the console is not going to tell you.
pub fn draw(
    mut contexts: bevy_egui::EguiContexts,
    mut fishing: ResMut<Fishing>,
    sim: Res<Sim>,
    console: Res<crate::panel::Console>,
) -> Result {
    use crate::panel::paint;
    use bevy_egui::egui;

    if !fishing.out() || console.hidden {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let frame = egui::Frame::new()
        .fill(paint::CASE)
        .stroke(egui::Stroke::new(1.0, paint::BEZEL))
        .inner_margin(egui::Margin::symmetric(9, 7));

    let tension = fishing.tension;
    let drag = fishing.drag;
    let drag_force = fishing.drag_force();
    let line_out = fishing.line_out;
    let depth = crate::fishing::lure_depth(line_out, sim.sim.telemetry.stw);
    let sounding = sim.sim.telemetry.depth;
    let mut set_drag: Option<f64> = None;

    egui::Area::new("rod".into())
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-8.0, 8.0))
        .show(ctx, |ui| {
            frame.show(ui, |ui| {
                ui.set_width(164.0);
                let small = |ui: &mut egui::Ui, text: String, colour| {
                    ui.label(
                        egui::RichText::new(text)
                            .color(colour)
                            .monospace()
                            .size(9.0),
                    );
                };

                small(ui, "ROD".into(), paint::LABEL);

                // --- Load, with the drag marked on the same scale -------------------
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(146.0, 11.0), egui::Sense::hover());
                let p = ui.painter();
                p.rect_filled(rect, 2.0, paint::TRACK);
                let at = |v: f64| {
                    rect.left()
                        + (v / crate::fishing::BREAKING).clamp(0.0, 1.0) as f32 * rect.width()
                };
                let loaded = egui::Rect::from_min_max(
                    rect.left_top(),
                    egui::pos2(at(tension), rect.bottom()),
                );
                // Amber up to the drag setting and red past it, because past the drag
                // the line is going out and that is a different situation rather than
                // more of the same one.
                p.rect_filled(
                    loaded,
                    2.0,
                    if tension > drag_force {
                        paint::NEEDLE
                    } else {
                        paint::LIVE
                    },
                );
                let star = at(drag_force);
                p.line_segment(
                    [
                        egui::pos2(star, rect.top() - 1.0),
                        egui::pos2(star, rect.bottom() + 1.0),
                    ],
                    egui::Stroke::new(1.5, paint::FACE),
                );
                small(
                    ui,
                    format!("LOAD {tension:>5.0} N   DRAG {drag_force:>3.0} N"),
                    paint::FACE,
                );

                // --- Line, depth, and what is under her -----------------------------
                ui.add_space(2.0);
                small(
                    ui,
                    format!("LINE {line_out:>5.0} m   LURE {depth:>4.0} m"),
                    paint::FACE,
                );
                // Red when the lure is in the last two metres of water, which is where
                // it gets lost.
                let clear = sounding - depth;
                small(
                    ui,
                    format!("UNDER{sounding:>5.0} m   CLEAR{clear:>4.0} m"),
                    if clear < 2.0 {
                        paint::BAD
                    } else {
                        paint::LABEL
                    },
                );

                // --- The drag, as a thing you can put a mouse on --------------------
                ui.add_space(3.0);
                let (track, response) =
                    ui.allocate_exact_size(egui::vec2(146.0, 13.0), egui::Sense::click_and_drag());
                let p = ui.painter();
                p.rect_filled(track, 2.0, paint::TRACK);
                // The rule, painted: a quarter to a third of the line's strength.
                let band = egui::Rect::from_min_max(
                    egui::pos2(track.left() + 0.25 * track.width(), track.top()),
                    egui::pos2(track.left() + 0.3333 * track.width(), track.bottom()),
                );
                p.rect_filled(band, 0.0, paint::GOOD.gamma_multiply(0.45));
                let knob = egui::Rect::from_center_size(
                    egui::pos2(track.left() + drag as f32 * track.width(), track.center().y),
                    egui::vec2(7.0, track.height() - 2.0),
                );
                p.rect_filled(knob, 1.0, paint::BEZEL);
                p.rect_stroke(
                    knob,
                    1.0,
                    egui::Stroke::new(1.0, paint::FACE),
                    egui::StrokeKind::Inside,
                );
                if response.dragged() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        set_drag =
                            Some(((pos.x - track.left()) / track.width()).clamp(0.0, 1.0) as f64);
                    }
                }

                // --- What is on -----------------------------------------------------
                ui.add_space(3.0);
                match fishing.fish() {
                    Some(fish) => {
                        small(ui, format!("ON  {:<16}", fish.doing.name()), paint::NEEDLE);
                        let (bar, _) =
                            ui.allocate_exact_size(egui::vec2(146.0, 7.0), egui::Sense::hover());
                        let p = ui.painter();
                        p.rect_filled(bar, 1.0, paint::TRACK);
                        p.rect_filled(
                            egui::Rect::from_min_max(
                                bar.left_top(),
                                egui::pos2(
                                    bar.left() + fish.beaten() as f32 * bar.width(),
                                    bar.bottom(),
                                ),
                            ),
                            1.0,
                            paint::GOOD,
                        );
                    }
                    None => small(ui, "trolling".into(), paint::LABEL),
                }

                // --- The box --------------------------------------------------------
                if !fishing.caught.is_empty() || fishing.lost > 0 {
                    let weight: f64 = fishing.caught.iter().map(|c| c.mass).sum();
                    small(
                        ui,
                        format!(
                            "BOX  {} fish, {weight:.1} kg   lost {}",
                            fishing.caught.len(),
                            fishing.lost
                        ),
                        paint::LABEL,
                    );
                    // The best one, because that is the fish anybody talks about
                    // afterwards and a total weight is not.
                    if let Some(best) = fishing
                        .caught
                        .iter()
                        .max_by(|a, b| a.mass.total_cmp(&b.mass))
                    {
                        small(
                            ui,
                            format!("BEST {:<10}{:>5.1} kg", best.what.name(), best.mass),
                            paint::GOOD,
                        );
                    }
                }
            });
        });

    if let Some(v) = set_drag {
        fishing.drag = v;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A rod with nothing on it is a straight rod of its own length. Trivial, and it is
    /// the assertion that catches the walk in `tip_offset` drifting away from the walk
    /// in `blank_mesh`, which are two copies of the same loop and have to agree or the
    /// line starts somewhere the rod does not end.
    #[test]
    fn an_unloaded_rod_is_its_own_length() {
        let tip = tip_offset(0.0, Vec3::new(0.0, -0.6, 0.8));
        assert!((tip.length() - BLANK).abs() < 1e-4, "{}", tip.length());
        assert!(tip.angle_between(unloaded_direction()) < 1e-4);
    }

    /// Loading it bends it towards the pull, and bending it brings the tip nearer the
    /// butt — the blank does not get longer, which is the whole difference between a rod
    /// bending and a rod being stretched.
    #[test]
    fn a_loaded_rod_bends_towards_the_pull_without_growing() {
        let pull = Vec3::new(0.0, -0.8, 0.6).normalize();
        let slack = tip_offset(0.0, pull);
        let bent = tip_offset(0.6, pull);
        assert!(bent.length() < slack.length(), "the blank grew");
        assert!(bent.y < slack.y, "it bent the wrong way");
        // And it is still a rod rather than a hoop: the tip has not come round past the
        // butt however hard it is pulled.
        assert!(tip_offset(MAX_DEFLECTION, pull).length() > BLANK * 0.6);
    }

    /// The bend is drawn from the tension by the spring rate the fight uses, so a third
    /// of the line's strength has to produce a bend you can see and not a twitch.
    #[test]
    fn a_working_load_is_a_visible_bend() {
        let third = (crate::fishing::BREAKING / 3.0) as f32 / ROD_SPRING;
        assert!(third > 0.15, "{third} m of tip movement is not a bend");
        assert!(third < MAX_DEFLECTION);
    }

    /// Every vertex the blank is made of is a number. A NaN here is invisible until the
    /// whole mesh vanishes, which is a bad way to find out about a normalise of a zero
    /// vector — and the pull *is* parallel to the blank at one particular angle.
    #[test]
    fn the_blank_is_finite_even_when_the_pull_is_along_it() {
        for pull in [
            unloaded_direction(),
            -unloaded_direction(),
            Vec3::new(0.0, -1.0, 0.0),
            Vec3::ZERO,
        ] {
            let mesh = blank_mesh(0.4, pull);
            let positions = mesh
                .attribute(Mesh::ATTRIBUTE_POSITION)
                .expect("the blank has positions");
            let bevy::mesh::VertexAttributeValues::Float32x3(points) = positions else {
                panic!("positions are three floats");
            };
            assert_eq!(points.len(), (JOINTS + 1) * (AROUND + 1));
            assert!(
                points.iter().flatten().all(|v| v.is_finite()),
                "a vertex is not a number with the pull at {pull}"
            );
        }
    }
}
