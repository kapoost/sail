//! Two cameras, for two different jobs.
//!
//! The cockpit view heels with the boat. That is deliberate and it is the single most
//! important thing this file does: heel is the primary feedback channel, and a view
//! that stays level throws it away. The chase view deliberately does *not* heel, since
//! a horizon that rolls with the hull from outside the boat just makes people ill.

use bevy::prelude::*;

use crate::frame;
use crate::sim::Sim;

#[derive(Component)]
pub struct MainCamera;

#[derive(Resource, Default, PartialEq, Eq, Clone, Copy)]
pub enum View {
    #[default]
    Cockpit,
    Chase,
}

#[derive(Resource, Default)]
pub struct ChaseState {
    position: Vec3,
    initialised: bool,
}

pub fn setup(mut commands: Commands) {
    commands.spawn((
        MainCamera,
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 1.15,
            near: 0.08,
            far: 8000.0,
            ..default()
        }),
        Transform::from_xyz(0.0, 3.0, 12.0).looking_at(Vec3::new(0.0, 2.0, 0.0), Vec3::Y),
    ));
    commands.init_resource::<View>();
    commands.init_resource::<ChaseState>();
}

pub fn toggle(keys: Res<ButtonInput<KeyCode>>, pads: Query<&Gamepad>, mut view: ResMut<View>) {
    let pressed = keys.just_pressed(KeyCode::KeyC)
        || pads.iter().any(|p| p.just_pressed(GamepadButton::Select));
    if pressed {
        *view = match *view {
            View::Cockpit => View::Chase,
            View::Chase => View::Cockpit,
        };
    }
}

pub fn follow(
    sim: Res<Sim>,
    view: Res<View>,
    time: Res<Time>,
    mut chase: ResMut<ChaseState>,
    mut q: Query<&mut Transform, With<MainCamera>>,
) {
    let st = &sim.sim.state;
    let hull = frame::to_bevy(st.pos, 0.0);

    for mut cam in &mut q {
        match *view {
            View::Cockpit => {
                // At the helm, aft of midships, and heeling with the hull.
                let attitude = frame::attitude(st);
                let eye_local = Vec3::new(0.0, 2.35, 3.6);
                cam.translation = hull + attitude * eye_local;
                // Look forward and slightly down, in the hull's own frame, so the
                // horizon tilts exactly as much as the boat does.
                cam.rotation = attitude * Quat::from_rotation_x(-0.10);
                chase.initialised = false;
            }
            View::Chase => {
                let heading = frame::heading_only(st);
                let target = hull + heading * Vec3::new(0.0, 9.0, 21.0);
                if !chase.initialised {
                    chase.position = target;
                    chase.initialised = true;
                }
                // Lag the camera so the boat moves within the frame instead of being
                // welded to it. Without the lag a tack reads as the world spinning.
                let k = 1.0 - (-3.0 * time.delta_secs()).exp();
                chase.position = chase.position.lerp(target, k);
                cam.translation = chase.position;
                cam.look_at(hull + Vec3::Y * 4.5, Vec3::Y);
            }
        }
    }
}
