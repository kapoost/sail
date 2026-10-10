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

/// The genoa sheet car, which slides fore and aft along a track on the side deck.
/// `starboard` picks which of the two it is.
/// The mainsheet car. Unlike the genoa cars it runs *across* the boat.
#[derive(Component)]
pub struct TravellerCar;

#[derive(Component)]
pub struct GenoaCar {
    pub starboard: bool,
}

/// Where the genoa track runs, in the hull's frame: inboard of the rail on the side
/// deck, from abreast the mast back to the forward end of the cockpit.
pub const TRACK_X: f32 = 0.38;
pub const TRACK_FORWARD: f32 = -0.80;
pub const TRACK_AFT: f32 = 2.20;

/// Fore-and-aft position of the car for a given control setting.
/// Where the mainsheet traveller's track lies, fore and aft.
///
/// On the cockpit bridge, under the aft third of the boom, which is where a thirty-four
/// footer puts it.
pub const TRAVELLER_Z: f32 = 1.7;

/// Where the mainsheet car sits on its track, in metres from the centreline.
///
/// Signed exactly like the control: negative is the port end. The car is the point the
/// sheet pulls the boom end towards, so this is not decoration — it is the geometry the
/// force model is using, and seeing it move is how the control teaches itself.
pub fn traveller_position(traveller: f32, beam: f32) -> f32 {
    traveller.clamp(-1.0, 1.0) * sail_core::sail::TRAVELLER_HALF as f32 * beam
}

pub fn car_position(car: f32) -> f32 {
    TRACK_FORWARD + car.clamp(0.0, 1.0) * (TRACK_AFT - TRACK_FORWARD)
}

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
/// How high a headsail's clew floats when nothing is pulling it down, metres above
/// the tack. The sail hangs from the stay and the wind holds the foot up; only the
/// sheet brings it down.
const CLEW_FREE_RISE: f32 = 2.6;
/// And how low the sheet can drag it before the foot is on the deck.
const CLEW_MIN_RISE: f32 = 0.35;
/// Sheet length from the clew to the car, hauled right in and eased right out.
const SHEET_SHORT: f32 = 2.9;
const SHEET_LONG: f32 = 7.4;
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
            // The mainsheet traveller: a track athwartships, and the car on it that
            // decides which way the sheet pulls the boom end. It is the one trim
            // control whose whole point is the direction of a pull rather than the
            // length of a line, so it has to be visible from the helm.
            let beam = cfg.beam as f32;
            let traveller_span = 2.0 * sail_core::sail::TRAVELLER_HALF as f32 * beam;
            b.spawn((
                Mesh3d(meshes.add(Cuboid::new(traveller_span, 0.035, 0.055))),
                MeshMaterial3d(spar_mat.clone()),
                Transform::from_xyz(0.0, DECK + 0.018, TRAVELLER_Z),
            ));
            b.spawn((
                TravellerCar,
                Mesh3d(meshes.add(Cuboid::new(0.18, 0.08, 0.1))),
                MeshMaterial3d(underwater_mat.clone()),
                Transform::from_xyz(0.0, DECK + 0.062, TRAVELLER_Z),
            ));
            // Genoa tracks and their cars, one each side. The car is the headsail's
            // only shape control, so it has to be something you can watch move —
            // a number on a panel is not a trim you can learn by eye.
            let track_length = TRACK_AFT - TRACK_FORWARD;
            let track_mesh = meshes.add(Cuboid::new(0.05, 0.03, track_length));
            let car_mesh = meshes.add(Cuboid::new(0.085, 0.07, 0.17));
            for starboard in [false, true] {
                let x = if starboard { 1.0 } else { -1.0 } * cfg.beam as f32 * TRACK_X;
                b.spawn((
                    Mesh3d(track_mesh.clone()),
                    MeshMaterial3d(spar_mat.clone()),
                    Transform::from_xyz(x, DECK + 0.015, (TRACK_FORWARD + TRACK_AFT) * 0.5),
                ));
                b.spawn((
                    GenoaCar { starboard },
                    Mesh3d(car_mesh.clone()),
                    MeshMaterial3d(underwater_mat.clone()),
                    Transform::from_xyz(x, DECK + 0.055, car_position(0.4)),
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
        t.translation = frame::hull_translation(&sim.sim.state);
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

/// Slide the cars to wherever the control says they are.
pub fn update_genoa_cars(sim: Res<Sim>, mut q: Query<(&GenoaCar, &mut Transform)>) {
    let cfg = sim.config();
    let z = car_position(sim.controls.genoa_car as f32);
    for (car, mut transform) in &mut q {
        let side = if car.starboard { 1.0 } else { -1.0 };
        transform.translation = Vec3::new(side * cfg.beam as f32 * TRACK_X, DECK + 0.055, z);
    }
}

/// Slide the mainsheet car across to wherever the control says it is.
pub fn update_traveller_car(sim: Res<Sim>, mut q: Query<&mut Transform, With<TravellerCar>>) {
    let x = traveller_position(sim.controls.traveller as f32, sim.config().beam as f32);
    for mut transform in &mut q {
        transform.translation = Vec3::new(x, DECK + 0.062, TRAVELLER_Z);
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
    mut q: Query<(&SailSurface, &mut Visibility)>,
) {
    let t_ = &sim.sim.telemetry;
    let cfg = sim.config();
    let side = -t_.awa.signum() as f32;
    let now = time.elapsed_secs();

    for (surface, mut visibility) in &mut q {
        *visibility = if surface.which == Sail::Genoa && sim.controls.furl > 0.985 {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        let (forces, geometry) = match surface.which {
            Sail::Main => {
                let reef = sim.controls.reef as f32;
                let foot = 4.2 * (1.0 - 0.35 * reef);
                let boom = side * t_.main.chord_angle as f32;
                let tack = Vec3::new(0.0, DECK + 1.25, MAST_FOOT_Z);
                (
                    t_.main,
                    SailGeometry {
                        tack,
                        // Reefing pulls the head down the mast, not the foot up.
                        head: Vec3::new(
                            0.0,
                            DECK + 1.25 + (14.6 - 1.25) * (1.0 - 0.45 * reef),
                            MAST_FOOT_Z,
                        ),
                        // The clew is the boom end: level with the tack, because the
                        // boom is level. That is what made the horizontal-chord
                        // shortcut look right until a headsail turned up.
                        clew: tack + Vec3::new(boom.sin() * foot, 0.0, boom.cos() * foot),
                    },
                )
            }
            Sail::Genoa => {
                // A rolling genoa loses area from its foot and leech as the drum
                // turns, so it visibly shrinks on the stay rather than vanishing.
                let out = 1.0 - sim.controls.furl as f32;
                let foot = 4.9 * out;
                let angle = side * t_.jib.chord_angle as f32;
                let tack = Vec3::new(0.0, DECK + 0.25, -(cfg.loa as f32) * 0.46);
                let head = Vec3::new(0.0, DECK + 13.4, MAST_FOOT_Z);
                let car = Vec3::new(
                    side * cfg.beam as f32 * TRACK_X,
                    DECK + 0.09,
                    car_position(sim.controls.genoa_car as f32),
                );
                (
                    t_.jib,
                    SailGeometry {
                        tack,
                        head: tack.lerp(head, 0.35 + 0.65 * out),
                        clew: genoa_clew(
                            tack,
                            angle,
                            foot,
                            car,
                            sheet_length(sim.controls.jib_sheet as f32),
                        ),
                    },
                )
            }
        };
        if let Some(mut mesh) = meshes.get_mut(&surface.mesh) {
            // Only the mainsail twists here; a headsail's shape is set by its lead.
            let twist = match surface.which {
                Sail::Main => side * t_.twist as f32,
                Sail::Genoa => 0.0,
            };
            build_sail(
                &mut mesh,
                &geometry,
                twist,
                forces.coeffs.luffing as f32,
                forces.camber as f32,
                now,
            );
        }
    }
}

/// Sheet length from the clew to the car for a given sheet setting.
pub fn sheet_length(jib_sheet: f32) -> f32 {
    SHEET_SHORT + (SHEET_LONG - SHEET_SHORT) * jib_sheet.clamp(0.0, 1.0)
}

/// Where a headsail's clew actually ends up.
///
/// Two constraints and a push, which between them are the whole of headsail shape:
///
/// * the foot cannot stretch, so the clew lies on a sphere about the tack;
/// * the sheet cannot stretch either, so once taut the clew lies on a sphere about
///   the car;
/// * and the wind pushes the sail to leeward, which picks which point on the circle
///   where those two spheres meet.
///
/// The first attempt solved only the *height* and kept the bearing from the force
/// model. That left the clew hanging near the centreline while the car sat out on the
/// side deck, so the sheet ran diagonally across instead of pulling the corner to
/// where it was led — and from the cockpit the sheets looked as though they were made
/// fast to the boom rather than to the headsail. The sheet pulls in every direction
/// it is led, not just downwards.
///
/// If the sheet is longer than it needs to be, nothing is taut and the clew floats:
/// out to leeward and well clear of the deck, which is the state the sail is in while
/// it is flogging and is exactly what it should look like.
pub fn genoa_clew(tack: Vec3, angle: f32, foot: f32, car: Vec3, sheet: f32) -> Vec3 {
    let leeward = Vec3::new(angle.signum(), 0.28, 0.0).normalize();
    // Freely hanging: on the sphere about the tack, blown to leeward and aft.
    let free_dir = Vec3::new(angle.sin(), 0.0, angle.cos()).normalize_or_zero();
    let free = tack + free_dir * foot + Vec3::Y * CLEW_FREE_RISE;
    let free = tack + (free - tack).normalize_or_zero() * foot;

    let to_car = car - tack;
    let span = to_car.length();
    if span < 1e-4 {
        return free;
    }
    if sheet >= span + foot {
        // Slack: nothing is pulling.
        return free;
    }
    if sheet + foot <= span {
        // Bar taut and still short: the clew is dragged straight at the car.
        return tack + to_car / span * foot;
    }

    // The two spheres meet in a circle. Find it, then take the point on it that the
    // wind would push the sail towards.
    let axis = to_car / span;
    let along = (foot * foot - sheet * sheet + span * span) / (2.0 * span);
    let radius = (foot * foot - along * along).max(0.0).sqrt();
    let centre = tack + axis * along;

    // Project the leeward push into the circle's plane to choose the point.
    let mut out = leeward - axis * leeward.dot(axis);
    if out.length_squared() < 1e-8 {
        out = Vec3::Y - axis * axis.y;
    }
    let clew = centre + out.normalize_or_zero() * radius;
    // Never below the deck, whatever the geometry says.
    Vec3::new(clew.x, clew.y.max(DECK + CLEW_MIN_RISE), clew.z)
}

/// A sail is three corners and the surface between them.
///
/// The first version described it as a luff plus a *horizontal* chord, which is fine
/// for a mainsail — its foot lies along a level boom — and simply wrong for a
/// headsail, whose foot is a free curve rising from the tack to a clew well above the
/// deck. The result was a genoa with its clew glued to the deck, dragging along it.
/// Three corners is both more correct and less code.
struct SailGeometry {
    tack: Vec3,
    head: Vec3,
    clew: Vec3,
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

fn build_sail(mesh: &mut Mesh, g: &SailGeometry, twist: f32, luffing: f32, draft: f32, now: f32) {
    let mut positions = Vec::with_capacity(SAIL_U * SAIL_V);
    let mut uvs = Vec::with_capacity(SAIL_U * SAIL_V);

    // Which side the sail bellies towards: away from the chord, on the leeward hand.
    let chord = g.clew - g.tack;
    let leeward = if chord.x >= 0.0 { 1.0 } else { -1.0 };

    let reference = sail_core::sail::CAMBER_REFERENCE as f32;
    let deviation = draft * (1.0 - luffing) - reference;

    for ui_ in 0..SAIL_U {
        let u = ui_ as f32 / (SAIL_U - 1) as f32;
        // Luff and leech both run to the head, so the chord closes to nothing there
        // on its own — no taper fudge required.
        let luff = g.tack.lerp(g.head, u);
        let mut leech = g.clew.lerp(g.head, u);

        // Twist: the leech falls off aloft, swinging about the luff at that height.
        // Zero at the foot, full at the head, which is what makes the vang visible.
        if twist.abs() > 1e-5 {
            let a = twist * u.powf(1.15);
            let arm = leech - luff;
            let (sin, cos) = (a.sin(), a.cos());
            leech = luff + Vec3::new(arm.x * cos + arm.z * sin, arm.y, -arm.x * sin + arm.z * cos);
        }

        let span = leech - luff;
        let chord_length = span.length().max(1e-4);
        // Camber acts normal to the chord and across the sail, which for a surface
        // this upright means the horizontal perpendicular.
        let normal = span.normalize().cross(Vec3::Y).normalize_or_zero() * leeward;
        // The outhaul tensions the foot, so its share of the depth sits low down.
        let foot_weight = (1.0 - u).powf(1.6);
        let local_camber = reference + deviation * (0.35 + 1.65 * foot_weight);

        for vi in 0..SAIL_V {
            let v = vi as f32 / (SAIL_V - 1) as f32;
            let belly = (std::f32::consts::PI * v).sin() * local_camber * chord_length;
            // A luffing sail shakes along its free edge only: it is bolted to the mast
            // up the luff and to the boom along the foot, and the clew is pinned by
            // the outhaul, so the shaking vanishes at v = 0, at u = 0, and therefore
            // at the corner where the two meet.
            let free_edge = v * u.powf(0.65);
            let flap = (now * 9.0 + u * 5.0 + v * 3.0).sin()
                * luffing
                * 0.42
                * free_edge
                * chord_length.max(0.4);
            positions.push(luff.lerp(leech, v) + normal * (belly + flap));
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
