//! Sea, sky and light.
//!
//! Flat water on purpose. Waves are a week of their own, and the question this proof
//! of concept has to answer — does trimming and tacking feel good — does not depend on
//! them. What it *does* depend on is a sense of motion, so the water carries a field of
//! wavelet marks that wrap around the boat. Without them a flat plane reads as ice and
//! six knots feels like nothing.

use bevy::prelude::*;

use crate::frame;
use crate::sim::Sim;

/// Half-size of the wavelet field, metres.
const FIELD: f32 = 220.0;
/// Spacing of the wavelet grid, metres.
const SPACING: f32 = 11.0;

#[derive(Component)]
pub struct Wavelet;

#[derive(Component)]
pub struct SeaPlane;

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Sun, low enough to put some glitter on the water.
    commands.spawn((
        DirectionalLight {
            illuminance: 32_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, -0.9, -0.55, 0.0)),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.62, 0.74, 0.88),
        brightness: 420.0,
        ..default()
    });
    commands.insert_resource(ClearColor(Color::srgb(0.58, 0.70, 0.82)));

    let water = materials.add(StandardMaterial {
        base_color: Color::srgb(0.07, 0.15, 0.22),
        perceptual_roughness: 0.18,
        metallic: 0.08,
        ..default()
    });
    commands.spawn((
        SeaPlane,
        Mesh3d(meshes.add(Plane3d::default().mesh().size(6000.0, 6000.0))),
        MeshMaterial3d(water),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    // Wavelet marks: short pale streaks lying on the surface.
    let streak = meshes.add(Plane3d::default().mesh().size(1.5, 0.35));
    let foam = materials.add(StandardMaterial {
        base_color: Color::srgba(0.80, 0.88, 0.94, 0.5),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let steps = (FIELD / SPACING) as i32;
    for ix in -steps..=steps {
        for iz in -steps..=steps {
            // Jitter off the grid, or the eye reads a lattice instead of water.
            let jx = ((ix * 73 + iz * 151) % 17) as f32 / 17.0 - 0.5;
            let jz = ((ix * 197 + iz * 31) % 19) as f32 / 19.0 - 0.5;
            let yaw = ((ix * 53 + iz * 97) % 23) as f32 / 23.0 * std::f32::consts::TAU;
            commands.spawn((
                Wavelet,
                Mesh3d(streak.clone()),
                MeshMaterial3d(foam.clone()),
                Transform::from_xyz((ix as f32 + jx) * SPACING, 0.03, (iz as f32 + jz) * SPACING)
                    .with_rotation(Quat::from_rotation_y(yaw)),
            ));
        }
    }
}

/// Keep the water under the boat, and wrap the wavelet field so the marks appear to
/// stream past instead of running out.
pub fn follow_boat(
    sim: Res<Sim>,
    mut sea: Query<&mut Transform, (With<SeaPlane>, Without<Wavelet>)>,
    mut wavelets: Query<&mut Transform, With<Wavelet>>,
) {
    let centre = frame::to_bevy(sim.sim.state.pos, 0.0);
    for mut t in &mut sea {
        t.translation.x = centre.x;
        t.translation.z = centre.z;
    }
    let span = FIELD * 2.0;
    for mut t in &mut wavelets {
        let d = t.translation - centre;
        if d.x > FIELD {
            t.translation.x -= span;
        } else if d.x < -FIELD {
            t.translation.x += span;
        }
        if d.z > FIELD {
            t.translation.z -= span;
        } else if d.z < -FIELD {
            t.translation.z += span;
        }
    }
}
