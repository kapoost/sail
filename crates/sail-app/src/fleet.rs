//! Other people's boats.
//!
//! The first milestone of sailing together, and deliberately the one with no network in
//! it: everything here works from a `Drawn` — the seventy bytes that describe a boat —
//! whoever put it there. A socket fills the fleet in; so does the rehearsal ghost at the
//! bottom of this file, which sails a slow circle round you so the drawing can be got
//! right before there is anything to debug on the far end of a wire.
//!
//! # Why a ghost is not just the boat drawn again
//!
//! Twenty files in this crate read the global `Sim`, and every one of them assumes there
//! is exactly one boat and that it is *the* boat: the camera, the telltales, the winch
//! that turns, the wake, the spray, the instruments, the sound. Rewriting all of that to
//! take a boat as an argument is days of work and it buys, at a hundred metres, nothing
//! anybody can see. A telltale is four centimetres of wool.
//!
//! So another boat is a deliberately simpler thing: a hull, a deck, a boom and her
//! sails, placed and dressed from what came over the wire. What makes that cheap rather
//! than a second renderer is that the hard parts were already written as functions of
//! plain data rather than of the resource — `build_sail` takes three corners and a trim,
//! `headsail_shape` takes a config and a set of controls, `frame::attitude` takes a
//! state. The single-boat path is not touched by any of this.
//!
//! # What the wire does not carry, and why that is right
//!
//! No forces, no coefficients, no apparent wind. The wind field is a pure function of
//! seed and time, so this machine can sample the wind *at the other boat's position*
//! itself, and with her velocity out of the snapshot it knows her apparent wind as well
//! as she does. Her sails are then set from her sheet positions, which is what they are
//! set from on her own screen. Sending what the receiver can derive is how a protocol
//! starts growing, and this one is meant to stay small.

use bevy::prelude::*;
use glam::DVec2;
use sail_core::units::ms_to_kn;
use sail_net::{BoatId, Drawn};

use crate::boat::{self, Sail};
use crate::frame;
use crate::sim::Sim;

/// Everybody else on the water.
#[derive(Resource, Default)]
pub struct Fleet {
    pub boats: Vec<Ghost>,
}

pub struct Ghost {
    pub id: BoatId,
    /// Who is sailing her. Goes over the boat once there is somebody to name — the
    /// socket is the next milestone and this is the field it fills.
    #[allow(dead_code)]
    pub name: String,
    /// The last snapshot, exactly as it arrived.
    pub drawn: Drawn,
    /// Seconds since it arrived, so the hull can be reckoned forward rather than
    /// stepping across the water sixty-six milliseconds at a time.
    pub age: f32,
}

impl Fleet {
    /// A snapshot has come in. Newest wins and nothing is queued: a late packet is worth
    /// less than the one already here, and a lost one is replaced in sixty-six
    /// milliseconds.
    pub fn heard(&mut self, id: BoatId, name: &str, drawn: Drawn) {
        match self.boats.iter_mut().find(|b| b.id == id) {
            Some(boat) => {
                boat.drawn = drawn;
                boat.age = 0.0;
            }
            None => self.boats.push(Ghost {
                id,
                name: name.to_string(),
                drawn,
                age: 0.0,
            }),
        }
    }

    /// She said goodbye. Without this she would still go, six seconds later, when
    /// nothing had been heard from her — but a boat that vanishes the moment somebody
    /// closes the window is worth the four lines.
    #[allow(dead_code)]
    pub fn gone(&mut self, id: BoatId) {
        self.boats.retain(|b| b.id != id);
    }
}

#[derive(Component)]
pub struct GhostRoot(BoatId);

#[derive(Component)]
pub struct GhostSail {
    id: BoatId,
    which: Sail,
}

#[derive(Component)]
pub struct GhostBoom(BoatId);

/// Nobody has sent anything for this long, so she is gone whether or not she said so.
/// Six seconds is ninety missed snapshots: not a dropped packet, a closed window.
const FORGOTTEN: f32 = 6.0;

/// Spawn a boat for an id that has appeared, and take away the ones that have left.
#[allow(clippy::too_many_arguments)]
pub fn sync(
    mut commands: Commands,
    mut fleet: ResMut<Fleet>,
    time: Res<Time>,
    sim: Res<Sim>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    existing: Query<(Entity, &GhostRoot)>,
) {
    let dt = time.delta_secs();
    for boat in &mut fleet.boats {
        boat.age += dt;
    }
    fleet.boats.retain(|b| b.age < FORGOTTEN);

    for (entity, root) in &existing {
        if !fleet.boats.iter().any(|b| b.id == root.0) {
            commands.entity(entity).despawn();
        }
    }
    let cfg = sim.config();
    for boat in &fleet.boats {
        if existing.iter().any(|(_, root)| root.0 == boat.id) {
            continue;
        }
        // Her cloth is a shade off yours, so that in a fleet you can tell which boat is
        // which from further away than a name tag can be read.
        let tint = 0.86 + 0.14 * ((boat.id % 7) as f32 / 7.0);
        let sail_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(tint, tint * 0.99, tint * 0.95),
            base_color_texture: Some(images.add(crate::sail_texture::cloth())),
            perceptual_roughness: 0.78,
            double_sided: true,
            cull_mode: None,
            ..default()
        });
        let spar_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.52, 0.38, 0.21),
            perceptual_roughness: 0.22,
            ..default()
        });
        commands
            .spawn((
                GhostRoot(boat.id),
                crate::ink::Inked,
                Transform::default(),
                Visibility::default(),
            ))
            .with_children(|b| {
                b.spawn((
                    bevy::world_serialization::WorldAssetRoot(
                        assets.load(GltfAssetLabel::Scene(0).from_asset("venus.glb")),
                    ),
                    Transform::from_xyz(0.0, boat::HULL_SINKAGE, 0.0)
                        .with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2))
                        .with_scale(Vec3::splat(cfg.loa as f32 / boat::SCAN_LOA)),
                ));
                // The deck, because an open hull at two hundred metres reads as a hole
                // in the water rather than as a boat.
                let (walked_on, _painted) = crate::deck::structure_mesh();
                b.spawn((
                    Mesh3d(meshes.add(walked_on)),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgb(0.74, 0.72, 0.66),
                        perceptual_roughness: 0.68,
                        ..default()
                    })),
                    Transform::default(),
                ));
                // The mast and the boom: two spars, which is what makes a hull a yacht
                // at a distance. The rigging wire is left off on purpose — at a hundred
                // metres a six-millimetre shroud is a tenth of a pixel.
                b.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.09, crate::rig::MAST_HEAD))),
                    MeshMaterial3d(spar_mat.clone()),
                    Transform::from_xyz(0.0, crate::rig::MAST_HEAD * 0.5, boat::MAST_FOOT_Z),
                ));
                b.spawn((
                    GhostBoom(boat.id),
                    Mesh3d(meshes.add(Cylinder::new(
                        0.06,
                        boat::main_foot(&cfg, &Default::default()),
                    ))),
                    MeshMaterial3d(spar_mat),
                    Transform::default(),
                ));
                for which in [Sail::Main, Sail::Headsail] {
                    b.spawn((
                        GhostSail { id: boat.id, which },
                        Mesh3d(meshes.add(boat::empty_sail())),
                        MeshMaterial3d(sail_mat.clone()),
                        Transform::default(),
                        Visibility::default(),
                    ));
                }
            });
    }
}

/// Where she is this frame.
pub fn place(
    fleet: Res<Fleet>,
    origin: Res<frame::RenderOrigin>,
    mut q: Query<(&GhostRoot, &mut Transform)>,
) {
    for (root, mut tr) in &mut q {
        let Some(boat) = fleet.boats.iter().find(|b| b.id == root.0) else {
            continue;
        };
        // Reckoned forward from the snapshot rather than held at it. A yacht takes
        // seconds to answer her helm, so a straight line across the gap between packets
        // is right to the centimetre, and the alternative is a hull that moves in
        // sixty-six millisecond steps.
        let st = boat.drawn.reckoned(boat.age).state();
        tr.translation = origin.hull_translation(&st);
        tr.rotation = frame::attitude(&st);
    }
}

/// And what her sails are doing, which is the whole of what you read another boat by.
pub fn dress(
    fleet: Res<Fleet>,
    sim: Res<Sim>,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut sails: Query<(&GhostSail, &Mesh3d, &mut Visibility)>,
    mut booms: Query<(&GhostBoom, &mut Transform)>,
) {
    let cfg = sim.config();
    let now = time.elapsed_secs();
    for (surface, mesh, mut visible) in &mut sails {
        let Some(boat) = fleet.boats.iter().find(|b| b.id == surface.id) else {
            continue;
        };
        let controls = boat.drawn.controls();
        *visible = match surface.which {
            Sail::Main if controls.reef >= sail_core::boat::DROPPED => Visibility::Hidden,
            Sail::Headsail if controls.furl > 0.985 => Visibility::Hidden,
            _ => Visibility::Inherited,
        };
        let (awa, aws) = apparent(&sim, &boat.drawn);
        // Leeward is the only side cloth can belly towards, and it is the side the wind
        // is not on.
        let lee = -(awa.signum() as f32);
        let profile = match surface.which {
            Sail::Main => &cfg.main.profile,
            _ => &cfg.headsail(controls.headsail).profile,
        };
        let sheet = match surface.which {
            Sail::Main => controls.main_sheet,
            _ => controls.jib_sheet,
        };
        let chord = profile.chord_angle(sheet) as f32;
        // The one piece of sailing in this file: a sail eased further out than the wind
        // is forward of her shakes, because the wind is then on the wrong side of it.
        // That is the same relation the simulation uses, and it is what makes a ghost
        // read as badly trimmed rather than as a flag.
        let luffing = ((chord - awa.abs() as f32) / 0.2).clamp(0.0, 1.0);
        let geometry = match surface.which {
            Sail::Main => {
                let scale = boat::main_reef_scale(&cfg, &controls);
                boat::SailGeometry {
                    tack: Vec3::new(0.0, crate::rig::BOOM_HEIGHT, boat::MAST_FOOT_Z),
                    head: boat::main_head(scale),
                    clew: Vec3::new(0.0, crate::rig::BOOM_HEIGHT, boat::MAST_FOOT_Z)
                        + Quat::from_rotation_y(lee * chord)
                            * Vec3::new(0.0, 0.0, boat::main_foot(&cfg, &controls)),
                }
            }
            _ => boat::headsail_shape(&cfg, &controls, lee, chord),
        };
        if let Some(mut mesh) = meshes.get_mut(&mesh.0) {
            boat::dress_sail(
                &mut mesh,
                &geometry,
                lee,
                luffing,
                // How hard it is blowing, for the speed of the shaking.
                (ms_to_kn(aws) as f32 / 25.0).clamp(0.0, 1.0),
                now,
            );
        }
    }
    for (boom, mut tr) in &mut booms {
        let Some(boat) = fleet.boats.iter().find(|b| b.id == boom.0) else {
            continue;
        };
        tr.translation = Vec3::new(0.0, crate::rig::BOOM_HEIGHT, boat::MAST_FOOT_Z);
        tr.rotation = Quat::from_rotation_y(boat.drawn.boom_angle)
            * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    }
}

/// The apparent wind over somebody else's deck, worked out here rather than sent.
fn apparent(sim: &Sim, drawn: &Drawn) -> (f64, f64) {
    let pos = DVec2::new(drawn.pos[0] as f64, drawn.pos[1] as f64);
    let wind = sim.sim.wind.sample(pos, sim.sim.state.time);
    let heading = drawn.heading as f64;
    // Her velocity over the ground, from the body-frame velocity in the snapshot.
    let (s, c) = (heading.sin(), heading.cos());
    let v = DVec2::new(
        drawn.surge as f64 * s + drawn.sway as f64 * c,
        drawn.surge as f64 * c - drawn.sway as f64 * s,
    );
    let flow = wind - v;
    let aws = flow.length();
    // Into the body frame: the angle between the bow and where the wind is coming from.
    let bow = DVec2::new(s, c);
    let starboard = DVec2::new(c, -s);
    let from = -flow;
    let awa = from.dot(starboard).atan2(from.dot(bow));
    (awa, aws)
}

/// A boat to look at before there is a boat to talk to.
///
/// `SAIL_FLEET=3` puts three of them a couple of hundred metres off, reaching back and
/// forth across your stern. They are not simulated — they are a position and a heading
/// moved by hand — and that is the point: this is a rig for getting the *drawing* right,
/// which is the milestone, and it would be a poor test if it needed the physics to be
/// correct as well.
pub fn rehearsal(time: Res<Time>, sim: Res<Sim>, mut fleet: ResMut<Fleet>) {
    let how_many: u16 = match std::env::var("SAIL_FLEET")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        Some(n) => n,
        None => return,
    };
    let t = time.elapsed_secs();
    let here = sim.sim.state.pos;
    // Numbered out of the way of the real ones. A server hands out ids from one, and a
    // rehearsal ghost sharing a number with somebody who has just joined would be a boat
    // teleporting between a circle and the sea every frame — the sort of thing that gets
    // debugged as a network fault for an hour.
    const REHEARSAL: u16 = 60_000;
    for n in 0..how_many {
        let id = REHEARSAL + n;
        // A long slow circle, each one a little further out and a third of a turn apart,
        // so they pass across each other and you can see one boat against another.
        let phase = t * 0.05 + n as f32 * 2.1;
        let radius = 120.0 + 60.0 * n as f32;
        let pos = [
            (here.x as f32) + radius * phase.cos(),
            (here.y as f32) + radius * phase.sin(),
        ];
        // Going round a circle, her bow is a quarter turn ahead of her bearing from the
        // centre, and her heading in this world is measured from north, clockwise.
        let heading = std::f32::consts::FRAC_PI_2 - (phase + std::f32::consts::FRAC_PI_2);
        let drawn = Drawn {
            pos,
            heading,
            heel: 0.22 * (1.0 - 0.3 * (phase * 2.0).sin()),
            surge: 5.0,
            yaw_rate: 0.05,
            boom_angle: 0.5,
            main_sheet: 0.45,
            jib_sheet: 0.4,
            genoa_car: 0.5,
            ..Default::default()
        };
        fleet.heard(id, "rehearsal", drawn);
    }
}
