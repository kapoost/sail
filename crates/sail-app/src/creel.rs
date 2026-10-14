//! The creel, and what is in it.
//!
//! A basket on a lanyard, made fast to the cleat on her port quarter and hanging over
//! the side where the sea can keep what is in it cool. That is what it is for, and it is
//! why it goes outboard rather than in the cockpit: a fish in a basket in the sun is a
//! fish you have caught and then wasted.
//!
//! It exists because a score is not a thing. The console can say BOX 3 FISH, 6.2 KG and
//! that is a number on a panel; a creel swinging off the quarter with three fish in it is
//! an afternoon, and you can see it from the helm without asking for it. The same
//! argument as `stowage.rs` makes about sail bags, and it is the strongest one this
//! project has about what to draw: a boat is known by what is lying about on her.
//!
//! **It swings, and the swing is a pendulum rather than a wobble.** A basket on a metre
//! of line is a real oscillator with a real period — `T = 2π√(l/g)`, which at this length
//! is about a second and a quarter — and it hangs *plumb* while the boat rolls underneath
//! it. Both halves matter. Lagging the hull's roll with a filter, which is what
//! `camera.rs` does for a head and is the obvious thing to reach for, gets neither: a
//! filtered angle has no period of its own, so it cannot keep swinging after the boat
//! has stopped, and it cannot swing *against* her either, which is most of what hanging
//! gear actually does and the whole reason it is worth drawing.

use bevy::prelude::*;

use crate::fishing::{Fishing, Quarry};
use crate::sim::Sim;

/// Where the cleat is, in boat coordinates: on the port quarter, aft of the rod.
const CLEAT_Z: f32 = 4.35;
/// How long the lanyard is, metres. It sets the period of the swing and nothing else.
const LANYARD: f32 = 0.62;
/// How many fish are drawn. A creel this size holds about this many and the rest of the
/// catch is a number on the console.
const VISIBLE: usize = 7;

#[derive(Component)]
pub struct Creel;

/// A fish in the creel, and which one of the catch it is.
#[derive(Component)]
pub struct InTheCreel(usize);

/// Where the basket is in its swing.
///
/// Angle from plumb in the boat's own frame, so it is driven straight by the heel: the
/// basket wants to hang vertically and the boat keeps moving the hanging point.
#[derive(Resource, Default)]
pub struct Swing {
    angle: f32,
    rate: f32,
}

/// Restoring stiffness, per second squared: `g/l` for a pendulum of this length.
const STIFF: f32 = 9.81 / LANYARD;
/// And how quickly it dies away. A wicker basket half in the water is heavily damped —
/// it does not ring like a bell, it takes two or three swings and settles.
const DAMP: f32 = 1.9;

/// What each fish looks like in the basket: how long for its weight, and its colour.
///
/// Lengths from the cube root of the mass, which is the only honest way to get one from
/// the other and is why a nine kilogramme ling is not three times the length of a three
/// kilogramme pollack but about one and a half times. The colours are the fish seen from
/// above in a basket, which is to say their backs: nothing here is a bright fish, because
/// nothing that lives in these waters is one.
fn looks(what: Quarry) -> (f32, Color) {
    match what {
        // Steel blue-green, and the only one in the list with a pattern worth missing.
        Quarry::Mackerel => (0.34, Color::srgb(0.20, 0.34, 0.33)),
        Quarry::Saithe => (0.44, Color::srgb(0.22, 0.25, 0.24)),
        Quarry::Pollack => (0.46, Color::srgb(0.33, 0.27, 0.18)),
        Quarry::Cod => (0.52, Color::srgb(0.38, 0.36, 0.24)),
        Quarry::Ling => (0.68, Color::srgb(0.30, 0.26, 0.19)),
        Quarry::Bottom => (0.2, Color::srgb(0.2, 0.3, 0.2)),
    }
}

/// The basket: a wicker cylinder, open at the top, slightly wider at the mouth.
///
/// Built rather than modelled, like everything else on this boat, and for the same
/// reason — but also because a basket is a shape with a hole in it, and a hole is the one
/// thing a sphere scaled down the middle cannot have. Open at the top is the whole point:
/// it is what lets you see the fish.
fn basket_mesh(bottom: f32, mouth: f32, depth: f32) -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};

    const AROUND: usize = 14;
    const UP: usize = 5;
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    // Outside going up, then back down the inside, so the wall has a thickness and the
    // rim is a real edge you can see rather than a paper line.
    let wall = 0.012;
    for (pass, inward) in [(0, 0.0_f32), (1, wall)].into_iter() {
        for level in 0..=UP {
            let t = level as f32 / UP as f32;
            let r = (bottom + (mouth - bottom) * t) - inward;
            let y = t * depth;
            for a in 0..=AROUND {
                let angle = a as f32 / AROUND as f32 * std::f32::consts::TAU;
                let (sa, ca) = (angle.sin(), angle.cos());
                positions.push([r * sa, y, r * ca]);
                let out = if pass == 0 { 1.0 } else { -1.0 };
                normals.push([sa * out, 0.0, ca * out]);
                uvs.push([a as f32 / AROUND as f32, t]);
            }
        }
    }
    // And a bottom to it, or the fish fall through.
    let floor = positions.len() as u32;
    positions.push([0.0, 0.0, 0.0]);
    normals.push([0.0, -1.0, 0.0]);
    uvs.push([0.5, 0.5]);
    for a in 0..=AROUND {
        let angle = a as f32 / AROUND as f32 * std::f32::consts::TAU;
        positions.push([bottom * angle.sin(), 0.0, bottom * angle.cos()]);
        normals.push([0.0, -1.0, 0.0]);
        uvs.push([0.5, 0.5]);
    }

    let mut indices = Vec::new();
    let stride = AROUND + 1;
    let ring = (UP + 1) * stride;
    for pass in 0..2 {
        for level in 0..UP {
            for a in 0..AROUND {
                let i = (pass * ring + level * stride + a) as u32;
                let k = i + stride as u32;
                if pass == 0 {
                    indices.extend_from_slice(&[i, k, i + 1, i + 1, k, k + 1]);
                } else {
                    indices.extend_from_slice(&[i, i + 1, k, i + 1, k + 1, k]);
                }
            }
        }
    }
    for a in 0..AROUND as u32 {
        indices.extend_from_slice(&[floor, floor + 1 + a + 1, floor + 1 + a]);
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

/// Where the lanyard is made fast, in boat coordinates.
fn cleat() -> Vec3 {
    Vec3::new(
        -(crate::deck::deck_half_beam(CLEAT_Z) - 0.05),
        crate::deck::deck_height(CLEAT_Z) + 0.04,
        CLEAT_Z,
    )
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    boat: Query<Entity, With<crate::boat::BoatRoot>>,
) {
    // Before the early return: `update` takes this as a `ResMut` and would fail every
    // frame if the boat had not appeared, which is the case this branch exists to
    // survive gracefully.
    commands.init_resource::<Swing>();
    let Ok(root) = boat.single() else {
        warn!("no boat to hang a creel on");
        return;
    };
    let wicker = materials.add(StandardMaterial {
        base_color: Color::srgb(0.52, 0.40, 0.22),
        perceptual_roughness: 0.95,
        ..default()
    });
    let rope = materials.add(StandardMaterial {
        base_color: Color::srgb(0.68, 0.62, 0.48),
        perceptual_roughness: 1.0,
        ..default()
    });
    let basket = meshes.add(basket_mesh(0.14, 0.19, 0.26));
    let fish = meshes.add(Sphere::new(0.5).mesh().ico(2).expect("a fish"));

    commands.entity(root).with_children(|b| {
        // The hanging point is the cleat; everything below swings about it, which is why
        // the lanyard and the basket are children of one entity rotating there rather
        // than two things kept in step.
        b.spawn((
            Creel,
            Transform::from_translation(cleat()),
            Visibility::default(),
        ))
        .with_children(|b| {
            b.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.006, LANYARD))),
                MeshMaterial3d(rope),
                Transform::from_xyz(0.0, -LANYARD * 0.5, 0.0),
                Visibility::default(),
            ));
            b.spawn((
                Mesh3d(basket),
                MeshMaterial3d(wicker),
                Transform::from_xyz(0.0, -LANYARD - 0.26, 0.0),
                Visibility::default(),
            ));
            // The catch. Spawned once and shown as it fills, because a fish appearing is
            // one thing to get right and an entity appearing is another.
            for slot in 0..VISIBLE {
                // Lying across each other in a heap, the way fish in a basket do. The
                // angles are from the slot rather than drawn, so a given fish is always
                // in the same place and the heap does not shuffle itself every frame.
                let turn = slot as f32 * 1.21;
                let ring = 0.055 * (slot % 3) as f32;
                b.spawn((
                    InTheCreel(slot),
                    Mesh3d(fish.clone()),
                    MeshMaterial3d(materials.add(StandardMaterial::default())),
                    Transform::from_xyz(
                        turn.cos() * ring,
                        -LANYARD - 0.20 + 0.035 * (slot / 3) as f32,
                        turn.sin() * ring,
                    )
                    .with_rotation(Quat::from_rotation_y(turn * 1.7)),
                    Visibility::Hidden,
                ));
            }
        });
    });
}

/// Swing the creel, and show what is in it.
pub fn update(
    sim: Res<Sim>,
    fishing: Res<Fishing>,
    time: Res<Time>,
    mut swing: ResMut<Swing>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut creel: Query<&mut Transform, With<Creel>>,
    mut fish: Query<
        (
            &InTheCreel,
            &mut Visibility,
            &mut Transform,
            &MeshMaterial3d<StandardMaterial>,
        ),
        Without<Creel>,
    >,
) {
    let st = sim.render_state();
    let dt = time.delta_secs().min(0.05);

    // A pendulum, integrated rather than filtered. It wants to hang plumb, which in the
    // boat's own frame means at minus the heel; gravity pulls it there and the damping
    // of a basket half in the water takes the ring out of it.
    let plumb = -st.heel as f32;
    swing.rate += (STIFF * (plumb - swing.angle) - DAMP * swing.rate) * dt;
    // And the roll itself throws it: a boat rolling under a hanging weight accelerates
    // the hanging point sideways, which is the forcing term and is why gear swings hardest
    // in the moment she comes back up rather than at the end of the roll.
    swing.rate -= st.roll_rate as f32 * dt * 2.4;
    swing.angle += swing.rate * dt;
    // It cannot swing past the quarter it is tied to.
    swing.angle = swing.angle.clamp(-0.8, 0.8);
    for mut transform in &mut creel {
        transform.rotation = Quat::from_rotation_z(swing.angle);
    }

    let caught = &fishing.caught;
    for (slot, mut visible, mut transform, skin) in &mut fish {
        match caught.get(caught.len().saturating_sub(VISIBLE) + slot.0) {
            Some(landed) if slot.0 < caught.len() => {
                *visible = Visibility::Inherited;
                let (length, colour) = looks(landed.what);
                // Its real weight against the species' usual one, so a big one looks
                // like a big one rather than every pollack being the same pollack.
                let size = length * (landed.mass as f32 / 3.0).cbrt().clamp(0.55, 1.8);
                transform.scale = Vec3::new(size * 0.22, size * 0.16, size);
                if let Some(mut material) = materials.get_mut(&skin.0) {
                    material.base_color = colour;
                    material.perceptual_roughness = 0.42;
                }
            }
            _ => *visible = Visibility::Hidden,
        }
    }
}
