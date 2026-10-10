//! Bulkhead instruments.
//!
//! The developer panel says in its own header that it is not the game's interface,
//! and this is the start of what is. These are the repeaters that live on the
//! coachroof bulkhead where the helmsman can see them: a wind dial, a log, an
//! anemometer and an inclinometer, read by looking up from the tiller rather than by
//! opening anything.
//!
//! Needles on dials rather than numbers on a screen, and that is a deliberate choice
//! rather than an aesthetic one. A needle needs no font, no render target and no text
//! layout — it is a box and a rotation — and it is also what the instrument actually
//! is. More to the point, a needle is read as a *position* at a glance, which is how
//! you use a wind dial: you do not read "forty-three degrees", you see that the
//! pointer is a little off the mark and bear away.
//!
//! The developer panel keeps its numbers. It is a diagnostic tool and stripping it to
//! make a point would only make the physics harder to check.

use bevy::prelude::*;
use sail_core::prelude::*;

use crate::sim::Sim;

/// Where the panel sits in the hull's frame: on the aft face of the coachroof,
/// forward of the helm and a little below eye level, tilted up towards the eye.
const PANEL_Z: f32 = 1.25;
const PANEL_Y: f32 = 1.72;
const PANEL_TILT: f32 = 0.22;

const DIAL_RADIUS: f32 = 0.068;
const DIAL_SPACING: f32 = 0.158;

/// Which quantity a dial shows.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum Dial {
    /// Apparent wind angle: zero at the top, starboard to the right, like every wind
    /// instrument ever fitted.
    WindAngle,
    /// Apparent wind speed, 0 to 30 knots over a 270 degree sweep.
    WindSpeed,
    /// Speed through the water, 0 to 10 knots.
    Log,
    /// Heel, read one to one against the dial so the needle *is* the mast.
    Inclinometer,
}

const DIALS: [Dial; 4] = [
    Dial::WindAngle,
    Dial::WindSpeed,
    Dial::Log,
    Dial::Inclinometer,
];

/// A needle, and which dial it belongs to.
#[derive(Component)]
pub struct Needle(pub Dial);

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    root: Query<Entity, With<crate::boat::BoatRoot>>,
) {
    let Ok(root) = root.single() else {
        return;
    };

    let case = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.10, 0.11),
        perceptual_roughness: 0.55,
        ..default()
    });
    // Unlit, because an instrument has to be readable in the shade of the sprayhood
    // and in full sun alike — which is exactly why real ones are backlit.
    let face = materials.add(StandardMaterial {
        base_color: Color::srgb(0.90, 0.92, 0.94),
        unlit: true,
        ..default()
    });
    let mark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.13, 0.15),
        unlit: true,
        ..default()
    });
    let pointer = materials.add(StandardMaterial {
        base_color: Color::srgb(0.80, 0.11, 0.09),
        unlit: true,
        ..default()
    });

    let width = DIAL_SPACING * DIALS.len() as f32 + 0.05;
    let disc = meshes.add(Circle::new(DIAL_RADIUS).mesh());
    let tick = meshes.add(Cuboid::new(0.006, 0.014, 0.002));
    let major = meshes.add(Cuboid::new(0.009, 0.024, 0.002));
    let needle = meshes.add(Cuboid::new(0.005, DIAL_RADIUS * 0.84, 0.002));
    let boss = meshes.add(Circle::new(0.009).mesh());

    commands.entity(root).with_children(|b| {
        // The panel itself, facing aft towards the helm.
        b.spawn((
            Mesh3d(meshes.add(Cuboid::new(width, 0.19, 0.03))),
            MeshMaterial3d(case.clone()),
            Transform::from_xyz(0.0, PANEL_Y, PANEL_Z)
                .with_rotation(Quat::from_rotation_x(-PANEL_TILT)),
        ))
        .with_children(|panel| {
            for (i, dial) in DIALS.iter().enumerate() {
                let x = (i as f32 - (DIALS.len() as f32 - 1.0) * 0.5) * DIAL_SPACING;
                // Dials sit proud of the case, facing aft.
                panel
                    .spawn((Transform::from_xyz(x, 0.0, 0.018), Visibility::default()))
                    .with_children(|d| {
                        d.spawn((Mesh3d(disc.clone()), MeshMaterial3d(face.clone())));

                        // Graduations. A wind dial is marked all the way round; the
                        // others sweep 270 degrees with a gap at the bottom, which is
                        // where the needle rests at zero.
                        let full_circle = *dial == Dial::WindAngle;
                        let count = if full_circle { 36 } else { 28 };
                        for t in 0..count {
                            let f = t as f32 / count as f32;
                            let angle = if full_circle {
                                f * std::f32::consts::TAU
                            } else {
                                (-0.75 + f * 1.5) * std::f32::consts::PI
                            };
                            let big = t % if full_circle { 9 } else { 7 } == 0;
                            let r = DIAL_RADIUS - if big { 0.014 } else { 0.009 };
                            d.spawn((
                                Mesh3d(if big { major.clone() } else { tick.clone() }),
                                MeshMaterial3d(mark.clone()),
                                Transform::from_xyz(-angle.sin() * r, angle.cos() * r, 0.001)
                                    .with_rotation(Quat::from_rotation_z(-angle)),
                            ));
                        }

                        // The needle pivots at the centre and reaches towards the rim.
                        d.spawn((
                            Needle(*dial),
                            Transform::from_xyz(0.0, 0.0, 0.003),
                            Visibility::default(),
                        ))
                        .with_children(|n| {
                            n.spawn((
                                Mesh3d(needle.clone()),
                                MeshMaterial3d(pointer.clone()),
                                Transform::from_xyz(0.0, DIAL_RADIUS * 0.42, 0.0),
                            ));
                        });
                        d.spawn((
                            Mesh3d(boss.clone()),
                            MeshMaterial3d(mark.clone()),
                            Transform::from_xyz(0.0, 0.0, 0.005),
                        ));
                    });
            }
        });
    });
}

/// Sweep for the dials that are not the wind angle: 270 degrees, zero at the bottom
/// left, full scale at the bottom right.
fn sweep(fraction: f32) -> f32 {
    (-0.75 + fraction.clamp(0.0, 1.0) * 1.5) * std::f32::consts::PI
}

pub fn update(sim: Res<Sim>, mut needles: Query<(&Needle, &mut Transform)>) {
    let t = &sim.sim.telemetry;
    for (needle, mut transform) in &mut needles {
        let angle = match needle.0 {
            // Straight through: the pointer sits where the wind is coming from.
            Dial::WindAngle => t.awa as f32,
            Dial::WindSpeed => sweep(ms_to_kn(t.aws) as f32 / 30.0),
            Dial::Log => sweep(ms_to_kn(t.stw) as f32 / 10.0),
            // One to one, so the needle hangs where the mast is. An inclinometer that
            // exaggerated would be worse than none: heel is the one reading you are
            // already feeling through your feet, and the two must agree.
            Dial::Inclinometer => (t.heel as f32).clamp(-rad(60.0) as f32, rad(60.0) as f32),
        };
        transform.rotation = Quat::from_rotation_z(-angle);
    }
}
