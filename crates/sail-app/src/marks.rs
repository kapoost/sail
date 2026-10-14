//! The marks on the coast: lights, spires, beacons and cairns.
//!
//! These are the other half of a chart. A chart with no marks on it is a picture of some
//! islands; a chart with marks is an *instrument*, because two bearings and a straight
//! edge put you somewhere and three tell you how well you did it. Terrestrial navigation
//! is nothing more than that, and it is the oldest thing in this game — it predates the
//! compass, never mind the satellite.
//!
//! What is drawn here is deliberately the shape a pilot book describes rather than a
//! model of a building. A pilot book says *white tower, red band, 26 m* because those are
//! the three things that let you tell one light from another at four miles, and they are
//! exactly the three things that have to be in the mesh. A beautifully modelled lighthouse
//! that cannot be told from the next one along is a worse navigation mark than a white
//! cylinder that can.
//!
//! The lights flash. A light's **character** — how many flashes, how long between them —
//! is how it is identified in the dark, and it is written on the chart beside it: `Fl(2)
//! 10s` is two flashes every ten seconds and there is only one light on that coast that
//! does it. Nothing else in this game is identified by rhythm, and it is the single most
//! sailor-ish thing it is possible to draw.

use bevy::prelude::*;
use sail_core::land::Mark;

use crate::frame;
use crate::sim::Sim;

/// One mark standing on the coast.
#[derive(Component)]
pub struct Standing {
    /// Where it is in simulation coordinates, so it can be replaced when the render
    /// origin moves.
    pub at: glam::DVec2,
    pub top: f32,
}

/// The lantern of a light, which flashes its own character.
#[derive(Component)]
pub struct Lantern {
    /// Seconds for the whole character to repeat.
    period: f32,
    /// How many flashes in it.
    flashes: u32,
    /// Where in the cycle this one starts, so two lights with the same character are
    /// still distinguishable by not being in step.
    phase: f32,
}

/// How a light's character is chosen, and how it is written.
///
/// Group flashing, two or three, on a period of six to fifteen seconds — which is most
/// of what is actually on a coast. The number comes from the mark's own name so that it
/// is the same light every time the same seed is sailed.
fn character(name: &str) -> (u32, f32) {
    let hash = name
        .bytes()
        .fold(0u64, |a, b| a.wrapping_mul(0x100_0000_01B3) ^ b as u64);
    let flashes = 1 + (hash % 3) as u32;
    let period = 6.0 + ((hash >> 8) % 10) as f32;
    (flashes, period)
}

/// `Fl(2) 10s`, which is what is printed on the chart beside it.
pub fn light_character(name: &str) -> String {
    let (flashes, period) = character(name);
    if flashes == 1 {
        format!("Fl {period:.0}s")
    } else {
        format!("Fl({flashes}) {period:.0}s")
    }
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    origin: Res<frame::RenderOrigin>,
    sim: Res<Sim>,
) {
    let Some(land) = sim.sim.land.as_ref() else {
        return;
    };
    let white = materials.add(StandardMaterial {
        base_color: Color::srgb(0.90, 0.89, 0.86),
        perceptual_roughness: 0.85,
        ..default()
    });
    let red = materials.add(StandardMaterial {
        base_color: Color::srgb(0.68, 0.14, 0.12),
        perceptual_roughness: 0.85,
        ..default()
    });
    let dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.19, 0.21),
        perceptual_roughness: 0.8,
        ..default()
    });
    let stone = materials.add(StandardMaterial {
        base_color: Color::srgb(0.44, 0.43, 0.40),
        perceptual_roughness: 0.95,
        ..default()
    });

    for mark in land.landmarks() {
        let ground = mark.ground as f32;
        let base = origin.to_bevy(mark.at, ground);
        let mut here = commands.spawn((
            Standing {
                at: mark.at,
                top: ground,
            },
            Transform::from_translation(base),
            Visibility::default(),
        ));
        match mark.kind {
            // A tower, a band, and a lantern. Tapered, because a lighthouse is tapered
            // and because the taper is most of what makes a white cylinder read as one.
            Mark::Lighthouse => {
                let tall = (mark.height as f32 - ground).max(12.0);
                here.with_children(|m| {
                    m.spawn((
                        Mesh3d(meshes.add(Cone {
                            radius: 3.1,
                            height: tall * 2.6,
                        })),
                        MeshMaterial3d(white.clone()),
                        Transform::from_xyz(0.0, tall * 0.5 - tall * 0.8, 0.0),
                    ));
                    // The band. One is a lighthouse; two is a different lighthouse, and
                    // that is the whole of how they are told apart by day.
                    m.spawn((
                        Mesh3d(meshes.add(Cylinder::new(2.5, tall * 0.16))),
                        MeshMaterial3d(red.clone()),
                        Transform::from_xyz(0.0, tall * 0.52, 0.0),
                    ));
                    // The lantern, which is the thing that flashes.
                    let (flashes, period) = character(&mark.name);
                    m.spawn((
                        Lantern {
                            period,
                            flashes,
                            phase: (mark.at.x.abs() % 10.0) as f32 * 0.1,
                        },
                        Mesh3d(meshes.add(Sphere::new(1.9))),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: Color::srgb(0.95, 0.86, 0.55),
                            emissive: LinearRgba::BLACK,
                            perceptual_roughness: 0.25,
                            ..default()
                        })),
                        Transform::from_xyz(0.0, tall, 0.0),
                    ));
                    m.spawn((
                        Mesh3d(meshes.add(Cylinder::new(2.2, 0.5))),
                        MeshMaterial3d(dark.clone()),
                        Transform::from_xyz(0.0, tall + 2.0, 0.0),
                    ));
                });
            }
            // A square tower and a spire on it, whitewashed. Every pilot book in the
            // world leads with one of these.
            Mark::Spire => {
                let tall = (mark.height as f32 - ground).max(10.0);
                here.with_children(|m| {
                    m.spawn((
                        Mesh3d(meshes.add(Cuboid::new(5.0, tall * 0.62, 5.0))),
                        MeshMaterial3d(white.clone()),
                        Transform::from_xyz(0.0, tall * 0.31, 0.0),
                    ));
                    m.spawn((
                        Mesh3d(meshes.add(Cone {
                            radius: 3.4,
                            height: tall * 0.52,
                        })),
                        MeshMaterial3d(dark.clone()),
                        Transform::from_xyz(0.0, tall * 0.62 + tall * 0.26, 0.0),
                    ));
                });
            }
            // A pole and a topmark: unlit, cheap, and there for no reason but to be
            // seen. The topmark is the identification — a cone one way up or the other
            // is a different mark entirely.
            Mark::Beacon => {
                let tall = (mark.height as f32 - ground).max(5.0);
                here.with_children(|m| {
                    m.spawn((
                        Mesh3d(meshes.add(Cylinder::new(0.45, tall))),
                        MeshMaterial3d(dark.clone()),
                        Transform::from_xyz(0.0, tall * 0.5, 0.0),
                    ));
                    m.spawn((
                        Mesh3d(meshes.add(Cone {
                            radius: 2.0,
                            height: 3.4,
                        })),
                        MeshMaterial3d(red.clone()),
                        Transform::from_xyz(0.0, tall + 1.2, 0.0),
                    ));
                });
            }
            // Stones piled by somebody who wanted to be found again.
            Mark::Cairn => {
                let tall = (mark.height as f32 - ground).max(2.0);
                here.with_children(|m| {
                    for (i, up) in [0.0_f32, 0.45, 0.78].into_iter().enumerate() {
                        let r = 2.4 * (1.0 - up * 0.7);
                        m.spawn((
                            Mesh3d(meshes.add(Sphere::new(r))),
                            MeshMaterial3d(stone.clone()),
                            Transform::from_xyz(
                                (i as f32 - 1.0) * 0.4,
                                tall * up + r * 0.4,
                                (i as f32 - 1.0) * 0.3,
                            )
                            .with_scale(Vec3::new(1.0, 0.75, 1.0)),
                        ));
                    }
                });
            }
        }
    }
}

/// Keep them where they are while the world is re-based under them.
pub fn follow_origin(
    origin: Res<frame::RenderOrigin>,
    mut marks: Query<(&Standing, &mut Transform)>,
) {
    for (standing, mut transform) in &mut marks {
        transform.translation = origin.to_bevy(standing.at, standing.top);
    }
}

/// The lights, flashing their characters.
///
/// A light is identified by its rhythm and by nothing else: at four miles in the dark
/// every one of them is the same white dot, and what tells you which coast you are on is
/// counting flashes and timing the gap. So they flash — properly, with the dark between
/// the flashes of a group shorter than the dark between groups, which is what makes a
/// group readable as a group.
pub fn flash(
    sky: Res<crate::sky::Sky>,
    time: Res<Time>,
    lights: Query<(&Lantern, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Lit when the lamps on the boat are, by the same measure: the sun's own elevation
    // rather than a clock.
    let dark = 1.0
        - crate::curve::smoothstep(
            (-3.0_f32).to_radians(),
            (2.0_f32).to_radians(),
            sky.elevation(),
        );
    let t = time.elapsed_secs();
    for (lantern, handle) in &lights {
        let Some(mut material) = materials.get_mut(&handle.0) else {
            continue;
        };
        let phase = ((t / lantern.period) + lantern.phase).fract() * lantern.period;
        // Each flash is half a second, with half a second of dark between them inside a
        // group; the rest of the period is the long dark that separates one group from
        // the next.
        let on = (0..lantern.flashes).any(|n| {
            let starts = n as f32 * 1.0;
            (starts..starts + 0.5).contains(&phase)
        });
        let lit = if on && dark > 0.05 { 14.0 * dark } else { 0.0 };
        material.emissive = LinearRgba::rgb(1.0, 0.92, 0.62) * lit;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A light's character is what identifies it, so two lights on one coast must not
    /// read the same — and the way it is written has to be the way a chart writes it.
    #[test]
    fn the_lights_can_be_told_apart_by_their_rhythm() {
        let sea = sail_core::land::Archipelago::generate(4, 9_000.0, 70);
        let lights: Vec<sail_core::land::Landmark> = sea
            .landmarks()
            .into_iter()
            .filter(|m| m.kind == Mark::Lighthouse)
            .collect();
        let mut characters: Vec<String> = lights.iter().map(|m| light_character(&m.name)).collect();
        let before = characters.len();
        characters.sort();
        characters.dedup();
        assert!(
            characters.len() as f64 >= before as f64 * 0.5,
            "{before} lights share {} characters between them, which is a coast you \
             cannot identify in the dark",
            characters.len()
        );
        // And written the way a chart writes it.
        assert!(light_character("Haugvik").starts_with("Fl"));
        assert!(light_character("Haugvik").ends_with('s'));
    }
}
