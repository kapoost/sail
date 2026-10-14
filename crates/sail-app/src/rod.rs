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

/// Where the rings sit along the blank and how big each one is, metres.
///
/// Six, which is what a two-metre boat rod carries. The butt ring is the size of a
/// wedding ring and the tip ring is barely bigger than the line.
/// How a ring is tessellated: points round the ring, and round the wire it is bent
/// from. Named rather than inlined because the test that counts the blank's vertices
/// has to agree with it, and a magic number in two places is a test that will one day
/// be edited to match the bug.
const RING_ROUND: usize = 8;
const RING_THROUGH: usize = 4;

const RINGS: [(f32, f32); 6] = [
    (0.20, 0.0130),
    (0.38, 0.0100),
    (0.54, 0.0082),
    (0.68, 0.0068),
    (0.82, 0.0056),
    (0.97, 0.0044),
];

/// The spring rate the bend is drawn from. The same number the fight uses, deliberately:
/// see `fishing::ROD`, which is private because nothing outside the model is allowed to
/// reason with it — this is the one exception and it is a drawing, so it is stated here
/// rather than exported.
const ROD_SPRING: f32 = 120.0;
/// The most the tip may be drawn as deflected, metres. A rod bent further than this is
/// a rod that has gone, and nothing in the model lets the tension get there anyway.
const MAX_DEFLECTION: f32 = 1.15;

/// How deep the lure can be and still mark the surface, metres.
///
/// Four, which is generous and is meant to be: the point of the mark is to say *roughly
/// how deep* at a glance, so it has to fade over a useful range rather than switch off.
/// A lure actually breaking the surface at five knots on a short line is at half a
/// metre, and the mark is at its strongest there.
const SURFACE_MARK: f32 = 4.0;

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

/// The drum, which turns. Separate from the frame, which does not.
#[derive(Component)]
pub struct Spool;

/// The far end of the line: where it is, and the skin that tells a lure from a fish.
///
/// Named because the filters have to spell out that it cannot be the spool, and a query
/// with two components and two filters written inline is a signature nobody reads.
type FarEnd<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Transform,
        &'static MeshMaterial3d<StandardMaterial>,
    ),
    (With<Terminal>, Without<Spool>),
>;

/// What the reel was doing last frame.
///
/// Four small memories, and every one of them exists because a *change* is the event and
/// a state is not: a drag that is at a third tells you nothing, a drag that moved to a
/// third is somebody's hand on the star and is worth looking at.
#[derive(Resource, Default)]
pub struct Ratchet {
    /// Whether line was going out, so the drag breaking away is heard once rather than
    /// every frame it stays broken away.
    running: bool,
    /// Where the spool stood, radians, so the drum can be drawn turning.
    turn: f32,
    /// What was on the line, so a take can be told from a fish that is merely still on.
    playing: bool,
    /// Where the drag star was, and how much line was out.
    drag: f64,
    line_out: f64,
}

/// Radius of the spool with line on it, metres. A small multiplier.
///
/// It decides how fast the drum is drawn turning, and it is the real relation rather
/// than a chosen speed: a metre of line a second off a thirty millimetre spool is
/// thirty-three radians a second — five turns — which is a blur, and a blur is what a
/// running reel looks like.
const SPOOL_RADIUS: f32 = 0.030;

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
    // --- The reel -----------------------------------------------------------
    //
    // Two parts, and the split is the whole of what makes it read as a reel rather than
    // as a disc: the **frame** is bolted to the rod and never moves, and the **spool**
    // inside it turns. One entity for both would mean either a reel whose foot rotates
    // with the line — a thing no tackle shop has ever sold — or a drum that does not
    // turn, which is the one thing a reel is for.
    let (seat, lie) = reel_seat();
    let frame_metal = materials.add(StandardMaterial {
        base_color: Color::srgb(0.26, 0.27, 0.30),
        metallic: 0.85,
        perceptual_roughness: 0.35,
        ..default()
    });
    let plate_metal = materials.add(StandardMaterial {
        // Anodised, which is what every reel made since about 1970 is finished in.
        base_color: Color::srgb(0.38, 0.40, 0.44),
        metallic: 0.9,
        perceptual_roughness: 0.28,
        ..default()
    });
    let dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.08, 0.09),
        perceptual_roughness: 0.5,
        ..default()
    });
    let braid = materials.add(StandardMaterial {
        base_color: Color::srgb(0.62, 0.64, 0.63),
        perceptual_roughness: 0.95,
        ..default()
    });
    commands.entity(root).with_children(|b| {
        b.spawn((
            Gear,
            Mesh3d(meshes.add(Cuboid::new(0.018, 0.052, 0.030))),
            MeshMaterial3d(frame_metal.clone()),
            // The foot, standing between the blank and the reel. Halfway down the gap
            // the seat leaves, which is what a reel foot is.
            Transform::from_translation(seat - lie * Vec3::Y * 0.0 - (seat - butt()) * 0.08)
                .with_rotation(lie),
            Visibility::Hidden,
        ))
        .with_children(|b| {
            // The star drag, outboard of the frame on the handle side: the thing the
            // `-` and `=` keys are turning, and worth being able to see them turn.
            b.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.016, 0.006))),
                MeshMaterial3d(dark.clone()),
                Transform::from_xyz(0.0, 0.034, 0.0),
                Visibility::default(),
            ));
        });

        b.spawn((
            Gear,
            Spool,
            // The spool itself: the drum the line is on. Everything that turns hangs
            // off this one entity, which is why the handle goes round with it — on a
            // multiplier the handle and the drum are geared together, so a running fish
            // spins the handle too. That is a real and slightly alarming property of
            // the things.
            Mesh3d(meshes.add(Cylinder::new(SPOOL_RADIUS, 0.040))),
            MeshMaterial3d(braid),
            Transform::from_translation(seat).with_rotation(lie),
            Visibility::Hidden,
        ))
        .with_children(|b| {
            // The two side plates. Flat discs, which is what you see of a reel from
            // anywhere but dead abeam.
            for side in [1.0_f32, -1.0] {
                b.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.046, 0.008))),
                    MeshMaterial3d(plate_metal.clone()),
                    Transform::from_xyz(0.0, side * 0.024, 0.0),
                    Visibility::default(),
                ));
                // Ported: the ring of holes drilled through a side plate to save weight,
                // and the detail that tells you at a glance that the thing is turning.
                // A plain disc spinning looks exactly like a plain disc standing still.
                for hole in 0..6 {
                    let angle = hole as f32 / 6.0 * std::f32::consts::TAU;
                    b.spawn((
                        Mesh3d(meshes.add(Cylinder::new(0.0075, 0.004))),
                        MeshMaterial3d(dark.clone()),
                        Transform::from_xyz(angle.cos() * 0.032, side * 0.029, angle.sin() * 0.032),
                        Visibility::default(),
                    ));
                }
            }
            // The crank: an arm on the handle side with a knob on the end of it.
            b.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.046, 0.005, 0.010))),
                MeshMaterial3d(frame_metal.clone()),
                Transform::from_xyz(0.013, 0.033, 0.0),
                Visibility::default(),
            ));
            b.spawn((
                Mesh3d(meshes.add(Sphere::new(0.009).mesh().ico(2).expect("a knob"))),
                MeshMaterial3d(dark.clone()),
                Transform::from_xyz(0.034, 0.036, 0.0),
                Visibility::default(),
            ));
        });
    });

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

/// Where the reel sits and which way its axle lies.
///
/// A multiplier goes on *top* of the rod, which is not a style choice — the line comes
/// off the spool straight down into the butt ring, and the whole reason the big ring is
/// big is that it is gathering line off a drum eighty millimetres across. Hung
/// underneath, as a fixed-spool reel is, none of that geometry would be right.
///
/// Worked out from the blank's own direction rather than written down, so moving the rod
/// holder moves the reel with it and the two cannot drift apart.
fn reel_seat() -> (Vec3, Quat) {
    let along = unloaded_direction();
    // Across the rod and level: a reel's axle is athwart the blank.
    let axle = along.cross(Vec3::Y).normalize_or(Vec3::X);
    // And the remaining axis, which points away from the blank on the side the reel
    // stands.
    let up = axle.cross(along).normalize_or(Vec3::Y);
    (
        butt() + along * 0.26 + up * 0.052,
        Quat::from_rotation_arc(Vec3::Y, axle),
    )
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

    // The rings, in the same mesh as the blank they are whipped to.
    //
    // In the same mesh because they have to *bend with it*, and the blank's shape is
    // rebuilt from its spine every frame rather than being a transform anything could be
    // parented to. Emitting them here means a guide cannot come adrift from the rod it
    // is on, which is the only interesting way a separate entity could have failed and
    // is exactly how it would have.
    //
    // They crowd towards the tip and shrink as they go, which is how a rod is really
    // ringed: the line has to be held close to the blank where the blank bends most, or
    // it bowstrings across the curve and the top of the rod's action does nothing. The
    // butt ring is the big one because it has to gather line coming off a spool.
    for (station, radius) in RINGS {
        let joint = (station * JOINTS as f32).min(JOINTS as f32 - 1e-3);
        let lower = joint.floor() as usize;
        let upper = (lower + 1).min(JOINTS);
        let at = spine[lower].lerp(spine[upper], joint - lower as f32);
        let direction = (spine[upper] - spine[lower]).normalize_or(start);
        // Standing off on the side the line runs, which is the inside of the bend: the
        // load pulls the line against the guides, which is what they are for. With no
        // bend there is no side to take, so they hang under the rod.
        let foot = if across.length_squared() > 1e-6 {
            across
        } else {
            (Vec3::NEG_Y - direction * Vec3::NEG_Y.dot(direction)).normalize_or(Vec3::NEG_Y)
        };
        let blank_radius = 0.009 * (1.0 - station) + 0.0014;
        let centre = at + foot * (blank_radius + radius * 0.85);
        // A torus: wire of this thickness bent into a ring of that radius, lying across
        // the rod.
        const WIRE: f32 = 0.0016;
        let sideways = direction.cross(foot).normalize_or(Vec3::X);
        let base = positions.len() as u32;
        for a in 0..=RING_ROUND {
            let round = a as f32 / RING_ROUND as f32 * std::f32::consts::TAU;
            let out = foot * round.cos() + sideways * round.sin();
            for b in 0..=RING_THROUGH {
                let through = b as f32 / RING_THROUGH as f32 * std::f32::consts::TAU;
                let normal = out * through.cos() + direction * through.sin();
                positions.push(centre + out * radius + normal * WIRE);
                normals.push(normal);
                uvs.push(Vec2::new(
                    a as f32 / RING_ROUND as f32,
                    b as f32 / RING_THROUGH as f32,
                ));
            }
        }
        let ring_stride = RING_THROUGH + 1;
        for a in 0..RING_ROUND {
            for b in 0..RING_THROUGH {
                let i = base + (a * ring_stride + b) as u32;
                let k = i + ring_stride as u32;
                indices.extend_from_slice(&[i, k, i + 1, i + 1, k, k + 1]);
            }
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
    let across = match &fishing.rod {
        Rod::Playing(fish) => fish.across,
        _ => 0.0,
    };
    let depth = fishing.depth.min(fishing.line_out.max(0.5));
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
    mut spray: ResMut<crate::spray::Spray>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    // Whether the gear is drawn is one query over all of it; what each part is doing
    // is a query of its own. They overlap on entities and not on components, which is
    // all Bevy asks.
    mut shown: Query<&mut Visibility, With<Gear>>,
    blanks: Query<&Mesh3d, With<Blank>>,
    lines: Query<&Mesh3d, With<Line>>,
    mut attention: ResMut<crate::camera::Attention>,
    // `Without` each other, because Bevy will not run a system holding two mutable
    // Transform queries unless the filters *prove* they cannot overlap — a marker
    // component each is not a proof, since nothing stops an entity carrying both.
    mut terminals: FarEnd,
    mut spools: Query<&mut Transform, (With<Spool>, Without<Terminal>)>,
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

    // --- The mark on the water --------------------------------------------------
    //
    // The lure is a hand's length of metal and the sea is opaque, so at any resolution —
    // let alone this one — there is nothing to see of it. What you see on the water is
    // what it is *doing* to the water: a swirl trailing behind a spinner working two
    // metres down, and a proper spatter when it is skipping along in the first metre
    // behind a boat driving at five knots.
    //
    // Which makes it a depth gauge, and the best one in the game. The mark fades out as
    // the lure sinks and is gone by four metres, so **no disturbance means the lure is
    // down** — and that is the whole of what the player needs to know to choose between
    // mackerel and cod without reading a number at all. The number is on the console for
    // when they want it.
    //
    // Thrown into `spray.rs` rather than given a particle system of its own. It is the
    // same stuff: foam on water, lit the same way, dying the same way, counted against
    // the same budget.
    if showing {
        let knots = sail_core::units::ms_to_kn(sim.sim.telemetry.stw).max(0.0) as f32;
        let working = (knots / 5.0).clamp(0.0, 1.2);
        let shallow = (1.0 - depth as f32 / SURFACE_MARK).clamp(0.0, 1.0);
        // The water the lure stirred keeps a little of her way on it and then stops,
        // which is why the swirls fall astern in a line rather than sitting where the
        // boat is.
        let drift = st.forward() * 0.25;
        // Raised to the three halves rather than squared, and measured against the
        // bow's own rate so the two are in proportion: at five knots on a short line
        // this throws about as much water as her stem does, which is right for
        // something skipping on the surface, and it falls away to the odd fleck by
        // three metres rather than switching off.
        let rate = 90.0 * shallow.powf(1.5) * (0.3 + 0.7 * working);
        spray.disturb(at, drift, rate * time.delta_secs(), 0.5);
        // And a boil when the fish itself is near the surface, which is the moment the
        // whole view exists for: something heavy turning just under the water, throwing
        // it about, a little before you can make out what it is.
        if let Rod::Playing(fish) = &fishing.rod {
            let up = (1.0 - fish.depth as f32 / 3.5).clamp(0.0, 1.0);
            let heavy = (fish.mass as f32 / 3.0).clamp(0.4, 2.5);
            spray.disturb(at, drift, 90.0 * up * heavy * time.delta_secs(), 0.8);
        }
    }

    // --- The spool turns ----------------------------------------------------------
    //
    // At the rate the line is actually moving divided by the radius it is coming off,
    // which is the whole of the relation and is why a run is a blur: a metre a second off
    // a thirty millimetre spool is five turns a second. Line going out unwinds it and
    // winding in turns it back, so the drum answers the arrow keys and the fish alike —
    // and the ported side plates are there so that turning is visible at all, a plain
    // disc spinning being indistinguishable from a plain disc at rest.
    let moved = fishing.line_out - ratchet.line_out;
    ratchet.turn -= (moved / SPOOL_RADIUS as f64) as f32;
    let (seat, lie) = reel_seat();
    for mut transform in &mut spools {
        transform.translation = seat;
        transform.rotation = lie * Quat::from_rotation_y(ratchet.turn);
    }

    // --- What the eye ought to be on ------------------------------------------------
    //
    // Posted as claims on the camera rather than as camera commands — `camera.rs` owns
    // the angles and the hold-off that keeps all of this out of your way for a few
    // seconds after your own hand has moved the view.
    //
    // The order is the order a person's attention actually goes, which is not the order
    // of importance: a fish taking wins outright, but a hand on the drag star beats
    // watching the fish, because you looked at the star *on purpose* and the fish is
    // still going to be there.
    if showing {
        let playing = fishing.fish().is_some();
        let reel_world = hull + attitude * seat;
        if playing && !ratchet.playing {
            attention.look(far, 5.0, 4);
        } else if (fishing.drag - ratchet.drag).abs() > 1e-5 {
            attention.look(reel_world, 2.0, 3);
        } else if playing {
            // Refreshed every frame with where the fish now is, so the eye follows it
            // rather than staring at where it was when it took.
            attention.look(far, 0.4, 2);
        } else if moved.abs() > 1e-4 {
            attention.look(tip_world, 1.2, 1);
        }
        ratchet.playing = playing;
        ratchet.drag = fishing.drag;
    }
    ratchet.line_out = fishing.line_out;

    // --- The drag breaking away -------------------------------------------------
    //
    // One sharp clack at the moment a run starts, and nothing after it. The run itself
    // is a click train and a click train belongs on the audio thread, where it can be
    // generated at its own rate instead of at the frame rate — see the pawl voice in
    // `sound.rs`, which a reel and a winch now share because a reel pawl and a winch
    // pawl are the same sprung steel finger on the same toothed wheel.
    //
    // That sharing is also the fix for a thing you could hear: clacking once a frame
    // capped the ratchet at sixty a second *and* tied its tempo to the frame rate, so a
    // run sounded slower on a bad frame. The pawls run off the audio clock now.
    //
    // The breakaway keeps a clack of its own because it is a different event — static
    // friction letting go — and it is the one you want to hear across the cockpit.
    let running = fishing.paying > 0.01;
    if showing && running && !ratchet.running {
        sound.0.clack(0.3);
    }
    ratchet.running = running && showing;
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

/// The gear, as part of the instrument column.
///
/// Drawn *inside* `panel.rs`'s block of readouts rather than in an area of its own, and
/// that is a correction. It had a corner to itself at the top right — which is the
/// corner the chart opens into, and more to the point is not where anybody looks for a
/// number. The complaint from the deck was that you could not see how much line was out,
/// and the line out was on the screen the whole time, in the one place on it nobody had
/// any reason to read. A reading you have to be told the position of is a reading that
/// is not there.
///
/// So it joins the log, the wind, the sea and the sounder, and it appears and disappears
/// with the gear the way the squall warning appears and disappears with the squall.
///
/// Four things, in the order you look at them:
///
/// * **Line out**, first and on its own, because it is the control as well as the
///   reading: it is what sets the depth, and the depth is what decides whether anything
///   can see the lure.
/// * **The lure's depth**, the water under her, and what is between them. Red when the
///   lure is in the last two metres, which is where tackle is lost.
/// * **Load**, against the line's breaking strain, with the drag marked on the same
///   scale. Two numbers on one bar, because the whole question is which is bigger.
/// * **The drag**, with the quarter-to-a-third band painted on. The band is the only
///   instruction in the interface and it is painted rather than written. Draggable,
///   because the console's rule is that everything the keyboard can do the mouse can do.
///
/// What is on the end, once something is — and *not* what it is. You do not know that
/// until you see it, and the console is not going to spoil it.
pub fn gauge(ui: &mut bevy_egui::egui::Ui, fishing: &mut Fishing, sim: &Sim) {
    use crate::panel::paint;
    use bevy_egui::egui;

    if !fishing.out() {
        return;
    }
    let tension = fishing.tension;
    let drag = fishing.drag;
    let drag_force = fishing.drag_force();
    let line_out = fishing.line_out;
    // The model's own figure, which is clamped to the water there is. Working it out
    // again here is how the readout came to claim the lure was eight metres inside the
    // seabed.
    let depth = fishing.depth;
    let sounding = sim.sim.telemetry.depth;
    let clear = sounding - depth;

    let small = |ui: &mut egui::Ui, text: String, colour| {
        ui.label(
            egui::RichText::new(text)
                .color(colour)
                .monospace()
                .size(10.0),
        );
    };

    ui.add_space(4.0);
    ui.separator();
    // The one number the hand on the reel is changing, given a line of its own and the
    // size the headline readouts have.
    ui.label(
        egui::RichText::new(format!("LINE OUT   {line_out:>5.1} m"))
            .color(if fishing.paying > 0.05 {
                paint::LIVE
            } else {
                paint::FACE
            })
            .monospace()
            .size(13.0),
    );
    small(
        ui,
        format!("LURE {depth:>5.1} m   SEABED {sounding:>4.0} m"),
        if clear < 2.0 {
            paint::BAD
        } else {
            paint::LABEL
        },
    );

    // --- Load, with the drag marked on the same scale ---------------------------
    let (rect, _) = ui.allocate_exact_size(egui::vec2(160.0, 10.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 2.0, paint::TRACK);
    let at =
        |v: f64| rect.left() + (v / crate::fishing::BREAKING).clamp(0.0, 1.0) as f32 * rect.width();
    p.rect_filled(
        egui::Rect::from_min_max(rect.left_top(), egui::pos2(at(tension), rect.bottom())),
        2.0,
        // Amber up to the drag setting and red past it, because past the drag the line
        // is going out, and that is a different situation rather than more of the same.
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
        format!("LOAD {tension:>5.0} N   DRAG {drag_force:>4.0} N"),
        paint::FACE,
    );

    // --- The drag, as a thing you can put a mouse on ----------------------------
    let (track, response) =
        ui.allocate_exact_size(egui::vec2(160.0, 12.0), egui::Sense::click_and_drag());
    let p = ui.painter();
    p.rect_filled(track, 2.0, paint::TRACK);
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
            fishing.drag = ((pos.x - track.left()) / track.width()).clamp(0.0, 1.0) as f64;
        }
    }

    // --- What is on --------------------------------------------------------------
    match fishing.fish() {
        Some(fish) if fish.alongside > 0.0 => {
            // Alongside and waiting. The prompt is the whole of how anybody discovers
            // that the last move is a question of timing, so it is the brightest thing
            // on the console and it says which key.
            let now = crate::fishing::on_the_rise(
                sim.sim.state.heave_rate,
                sim.sim.telemetry.significant_height,
            );
            ui.label(
                egui::RichText::new(if now {
                    "LIFT HER  —  J"
                } else {
                    "wait for the rise"
                })
                .color(if now { paint::GOOD } else { paint::LABEL })
                .monospace()
                .size(13.0),
            );
        }
        Some(fish) => {
            small(ui, format!("ON  {}", fish.doing.name()), paint::NEEDLE);
            let (bar, _) = ui.allocate_exact_size(egui::vec2(160.0, 6.0), egui::Sense::hover());
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
        None => small(
            ui,
            if fishing.paying > 0.05 {
                "streaming".into()
            } else {
                "trolling".into()
            },
            paint::LABEL,
        ),
    }

    if !fishing.caught.is_empty() || fishing.lost > 0 {
        let weight: f64 = fishing.caught.iter().map(|c| c.mass).sum();
        // An empty box says so rather than reporting minus nought kilogrammes, which is
        // what the sum of no fish formats as and is a genuinely dispiriting thing to
        // read after losing three.
        small(
            ui,
            if fishing.caught.is_empty() {
                format!("BOX empty   lost {}", fishing.lost)
            } else {
                format!(
                    "BOX {} fish {weight:.1} kg  lost {}",
                    fishing.caught.len(),
                    fishing.lost
                )
            },
            paint::LABEL,
        );
        if let Some(best) = fishing
            .caught
            .iter()
            .max_by(|a, b| a.mass.total_cmp(&b.mass))
        {
            small(
                ui,
                format!("BEST {:<9}{:>5.1} kg", best.what.name(), best.mass),
                paint::GOOD,
            );
        }
    }
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
            // The blank's own tube, plus a ring of wire at each guide.
            let rings = RINGS.len() * (RING_ROUND + 1) * (RING_THROUGH + 1);
            assert_eq!(points.len(), (JOINTS + 1) * (AROUND + 1) + rings);
            assert!(
                points.iter().flatten().all(|v| v.is_finite()),
                "a vertex is not a number with the pull at {pull}"
            );
        }
    }
}
