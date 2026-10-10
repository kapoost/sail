//! The boat you can see.
//!
//! Its job is to make the physics legible without a HUD. Three channels do the work,
//! and they are the real instruments a sailor uses:
//!
//! * **Heel.** The primary feedback signal. You feel overpowered before you read it.
//! * **Sail shape.** The boom swings with the sheet, the sail takes up camber when it
//!   is drawing and shakes along the luff when it is not.
//! * **Telltales and the masthead windex.** Cheap to draw, and more informative than
//!   any number — telltales tell you *why* the sail is wrong, not just that it is.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::frame;
use crate::sim::Sim;

#[derive(Component)]
pub struct BoatRoot;

#[derive(Component)]
pub struct Boom;

#[derive(Component)]
pub struct RudderBlade;

#[derive(Component)]
pub struct Windex;

/// Which sail an entity draws, and the handle to its regenerated mesh.
#[derive(Component)]
pub struct SailSurface {
    pub which: Sail,
    pub mesh: Handle<Mesh>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Sail {
    Main,
    Genoa,
}

/// A ribbon on the sail. `along_luff` is its height up the sail, 0 at the foot.
#[derive(Component)]
pub struct Telltale {
    pub along_luff: f32,
}

const MAST_FOOT_Z: f32 = -1.8;
const DECK: f32 = 0.95;

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    sim: Res<Sim>,
) {
    let cfg = sim.config();

    let hull_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.92, 0.90),
        perceptual_roughness: 0.42,
        ..default()
    });
    let underwater_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.11, 0.26, 0.35),
        perceptual_roughness: 0.3,
        ..default()
    });
    let spar_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.70, 0.71, 0.73),
        metallic: 0.65,
        perceptual_roughness: 0.35,
        ..default()
    });
    let sail_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.94, 0.90),
        perceptual_roughness: 0.85,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let telltale_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.15, 0.15),
        unlit: true,
        double_sided: true,
        cull_mode: None,
        ..default()
    });

    let main_mesh = meshes.add(empty_sail_mesh());
    let genoa_mesh = meshes.add(empty_sail_mesh());

    commands
        .spawn((BoatRoot, Transform::default(), Visibility::default()))
        .with_children(|b| {
            // Hull. Bow is at -Z, which is Bevy's forward.
            b.spawn((
                Mesh3d(meshes.add(hull_mesh(cfg.loa as f32, cfg.beam as f32))),
                MeshMaterial3d(hull_mat.clone()),
                Transform::default(),
            ));
            // Keel fin.
            b.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.16, cfg.draft as f32 - 0.5, 1.25))),
                MeshMaterial3d(underwater_mat.clone()),
                Transform::from_xyz(
                    0.0,
                    -(cfg.draft as f32 - 0.5) / 2.0 - 0.45,
                    -cfg.x_clr as f32,
                ),
            ));
            // Rudder blade, which visibly turns. Seeing the blade answer is half of
            // learning why it stops answering when the boat has no way on.
            b.spawn((
                RudderBlade,
                Mesh3d(meshes.add(Cuboid::new(0.08, 1.15, 0.42))),
                MeshMaterial3d(underwater_mat.clone()),
                Transform::from_xyz(0.0, -0.95, -cfg.x_rudder as f32),
            ));
            // Mast.
            b.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.09, 15.2))),
                MeshMaterial3d(spar_mat.clone()),
                Transform::from_xyz(0.0, DECK + 7.6, MAST_FOOT_Z),
            ));
            // Boom, pivoting at the mast. Its own transform carries the sheet angle.
            b.spawn((
                Boom,
                Transform::from_xyz(0.0, DECK + 1.25, MAST_FOOT_Z),
                Visibility::default(),
            ))
            .with_children(|boom| {
                boom.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.065, 4.3))),
                    MeshMaterial3d(spar_mat.clone()),
                    Transform::from_xyz(0.0, 0.0, 2.15)
                        .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                ));
            });
            // Sails. Their meshes are rebuilt every frame from the trim.
            b.spawn((
                SailSurface {
                    which: Sail::Main,
                    mesh: main_mesh.clone(),
                },
                Mesh3d(main_mesh.clone()),
                MeshMaterial3d(sail_mat.clone()),
                Transform::default(),
            ));
            b.spawn((
                SailSurface {
                    which: Sail::Genoa,
                    mesh: genoa_mesh.clone(),
                },
                Mesh3d(genoa_mesh.clone()),
                MeshMaterial3d(sail_mat.clone()),
                Transform::default(),
            ));
            // Telltales up the luff of the main.
            for h in [0.30_f32, 0.52, 0.74] {
                b.spawn((
                    Telltale { along_luff: h },
                    Mesh3d(meshes.add(Cuboid::new(0.02, 0.05, 0.55))),
                    MeshMaterial3d(telltale_mat.clone()),
                    Transform::default(),
                ));
            }
            // Masthead windex.
            b.spawn((
                Windex,
                Transform::from_xyz(0.0, DECK + 15.4, MAST_FOOT_Z),
                Visibility::default(),
            ))
            .with_children(|w| {
                w.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.03, 0.22, 0.85))),
                    MeshMaterial3d(telltale_mat.clone()),
                    Transform::from_xyz(0.0, 0.0, 0.42),
                ));
            });
        });
}

/// Put the hull where the simulation says it is, at the attitude it says it has.
pub fn update_hull(sim: Res<Sim>, mut q: Query<&mut Transform, With<BoatRoot>>) {
    for mut t in &mut q {
        t.translation = frame::to_bevy(sim.sim.state.pos, 0.0);
        t.rotation = frame::attitude(&sim.sim.state);
    }
}

pub fn update_rudder(sim: Res<Sim>, mut q: Query<&mut Transform, With<RudderBlade>>) {
    let angle = sim.controls.rudder as f32;
    for mut t in &mut q {
        // Positive rudder turns the bow to starboard, so the blade's trailing edge
        // goes to starboard, which is a negative rotation about the vertical.
        t.rotation = Quat::from_rotation_y(-angle);
    }
}

pub fn update_boom(sim: Res<Sim>, mut q: Query<&mut Transform, With<Boom>>) {
    let t_ = &sim.sim.telemetry;
    // The boom goes out to leeward, so its sign is opposite the apparent wind side.
    let angle = -t_.awa.signum() as f32 * t_.main.chord_angle as f32;
    for mut tr in &mut q {
        tr.rotation = Quat::from_rotation_y(angle);
    }
}

pub fn update_windex(sim: Res<Sim>, mut q: Query<&mut Transform, With<Windex>>) {
    // A windex swings freely and points into the apparent wind.
    let awa = sim.sim.telemetry.awa as f32;
    for mut t in &mut q {
        t.rotation = Quat::from_rotation_y(-awa);
    }
}

/// Telltales stream aft when the flow is attached, lift and shake when the sail is
/// luffing, and droop when it has stalled. That is the actual diagnostic: the same
/// "sail is wrong" reads differently depending on which way it is wrong.
pub fn update_telltales(sim: Res<Sim>, time: Res<Time>, mut q: Query<(&Telltale, &mut Transform)>) {
    let t_ = &sim.sim.telemetry;
    let side = -t_.awa.signum() as f32;
    let boom = side * t_.main.chord_angle as f32;
    let luff = t_.main.coeffs.luffing as f32;
    let stall = t_.main.coeffs.stalled as f32;
    let now = time.elapsed_secs();

    for (tt, mut tr) in &mut q {
        let height = DECK + 1.4 + tt.along_luff * 12.2;
        // Just aft of the luff, which is where a telltale lives.
        let aft = 0.55 + tt.along_luff * 0.25;
        tr.translation = Vec3::new(
            boom.sin() * aft * 0.4 + side * 0.12,
            height,
            MAST_FOOT_Z + aft,
        );
        let shake = (now * 11.0 + tt.along_luff * 7.0).sin() * luff * 0.9;
        let droop = -stall * 0.7;
        tr.rotation = Quat::from_rotation_y(boom) * Quat::from_rotation_x(shake + droop);
    }
}

/// Rebuild both sail surfaces from the current trim.
///
/// Camber is what sells a drawing sail: a sheeted sail bellies out to leeward, and the
/// belly collapses the moment it luffs. Regenerating the mesh every frame is cheap at
/// this resolution and avoids a skinning rig we would only have to throw away.
pub fn update_sails(
    sim: Res<Sim>,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    q: Query<&SailSurface>,
) {
    let t_ = &sim.sim.telemetry;
    let cfg = sim.config();
    let side = -t_.awa.signum() as f32;
    let now = time.elapsed_secs();

    for surface in &q {
        let (forces, geometry) = match surface.which {
            Sail::Main => (
                t_.main,
                SailGeometry {
                    // Luff up the mast, foot along the boom.
                    tack: Vec3::new(0.0, DECK + 1.25, MAST_FOOT_Z),
                    head: Vec3::new(0.0, DECK + 14.6, MAST_FOOT_Z),
                    foot_length: 4.2 * (1.0 - 0.35 * sim.controls.reef as f32),
                    luff_fraction: 1.0 - 0.45 * sim.controls.reef as f32,
                },
            ),
            Sail::Genoa => (
                t_.jib,
                SailGeometry {
                    tack: Vec3::new(0.0, DECK + 0.25, -(cfg.loa as f32) * 0.46),
                    head: Vec3::new(0.0, DECK + 13.4, MAST_FOOT_Z),
                    foot_length: 4.9,
                    luff_fraction: 1.0,
                },
            ),
        };
        if let Some(mut mesh) = meshes.get_mut(&surface.mesh) {
            build_sail(
                &mut mesh,
                &geometry,
                side * forces.chord_angle as f32,
                forces.coeffs.luffing as f32,
                now,
            );
        }
    }
}

struct SailGeometry {
    tack: Vec3,
    head: Vec3,
    foot_length: f32,
    luff_fraction: f32,
}

const SAIL_U: usize = 9;
const SAIL_V: usize = 5;

fn empty_sail_mesh() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![Vec3::ZERO; SAIL_U * SAIL_V]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![Vec3::Y; SAIL_U * SAIL_V]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![Vec2::ZERO; SAIL_U * SAIL_V]);
    mesh.insert_indices(Indices::U32(sail_indices()));
    mesh
}

fn sail_indices() -> Vec<u32> {
    let mut idx = Vec::with_capacity((SAIL_U - 1) * (SAIL_V - 1) * 6);
    for u in 0..SAIL_U - 1 {
        for v in 0..SAIL_V - 1 {
            let a = (u * SAIL_V + v) as u32;
            let b = ((u + 1) * SAIL_V + v) as u32;
            let c = ((u + 1) * SAIL_V + v + 1) as u32;
            let d = (u * SAIL_V + v + 1) as u32;
            idx.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }
    idx
}

fn build_sail(mesh: &mut Mesh, g: &SailGeometry, chord_angle: f32, luffing: f32, now: f32) {
    let mut positions = Vec::with_capacity(SAIL_U * SAIL_V);
    let mut uvs = Vec::with_capacity(SAIL_U * SAIL_V);

    let leeward = chord_angle.signum();
    // A drawing sail carries roughly a tenth of its chord in camber; a luffing one
    // carries none, because there is nothing holding the shape.
    let camber = 0.11 * (1.0 - luffing);

    for ui in 0..SAIL_U {
        let u = ui as f32 / (SAIL_U - 1) as f32;
        let luff_point = g.tack.lerp(g.head, u * g.luff_fraction);
        // Chord tapers to nothing at the head.
        let chord = g.foot_length * (1.0 - u).powf(0.85);
        for vi in 0..SAIL_V {
            let v = vi as f32 / (SAIL_V - 1) as f32;
            // Along the boom, rotated by the sheet.
            let along = Vec3::new(
                chord_angle.sin() * v * chord,
                0.0,
                chord_angle.cos() * v * chord,
            );
            // Bulge normal to the chord, towards leeward.
            let belly = (std::f32::consts::PI * v).sin() * camber * chord;
            let normal = Vec3::new(chord_angle.cos(), 0.0, -chord_angle.sin()) * leeward;
            // A luffing sail shakes hardest at the leech and barely at the luff.
            let flap = (now * 9.0 + u * 5.0 + v * 3.0).sin() * luffing * 0.34 * v * chord.max(0.4);
            positions.push(luff_point + along + normal * (belly + flap));
            uvs.push(Vec2::new(v, u));
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.compute_normals();
}

/// A canoe body from a handful of stations. Not a hull form, just enough shape that
/// the eye reads "boat" instead of "crate" — the real lines can come from a model.
fn hull_mesh(loa: f32, beam: f32) -> Mesh {
    const STATIONS: usize = 11;
    let half_beam = beam * 0.5;
    let depth = 0.62;

    let mut positions: Vec<Vec3> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // Section shape at a given fraction from bow (0) to stern (1).
    let section = |f: f32| -> (f32, f32) {
        // Beam fills out quickly from the bow, holds through midships, narrows a
        // little into the transom.
        let b = (f * 2.1).min(1.0).powf(0.62) * (1.0 - 0.30 * (f - 0.55).max(0.0) / 0.45);
        let d = (f * 3.0).min(1.0).powf(0.5);
        (half_beam * b, depth * d)
    };

    for s in 0..STATIONS {
        let f = s as f32 / (STATIONS - 1) as f32;
        let z = -loa * 0.5 + f * loa;
        let (b, d) = section(f);
        // Four points per station: port rail, port bilge, starboard bilge, starboard rail.
        positions.push(Vec3::new(-b, DECK, z));
        positions.push(Vec3::new(-b * 0.80, -d * 0.55, z));
        positions.push(Vec3::new(0.0, -d, z));
        positions.push(Vec3::new(b * 0.80, -d * 0.55, z));
        positions.push(Vec3::new(b, DECK, z));
    }
    let ring = 5;
    for s in 0..STATIONS - 1 {
        for p in 0..ring - 1 {
            let a = (s * ring + p) as u32;
            let b = ((s + 1) * ring + p) as u32;
            let c = ((s + 1) * ring + p + 1) as u32;
            let d = (s * ring + p + 1) as u32;
            indices.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }
    // Deck: fan between the rails.
    let deck_start = positions.len() as u32;
    for s in 0..STATIONS {
        let f = s as f32 / (STATIONS - 1) as f32;
        let z = -loa * 0.5 + f * loa;
        let (b, _) = section(f);
        positions.push(Vec3::new(-b, DECK, z));
        positions.push(Vec3::new(b, DECK, z));
    }
    for s in 0..STATIONS - 1 {
        let a = deck_start + (s * 2) as u32;
        let b = a + 1;
        let c = a + 3;
        let d = a + 2;
        indices.extend_from_slice(&[a, d, c, a, c, b]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_indices(Indices::U32(indices));
    mesh.compute_normals();
    mesh
}
