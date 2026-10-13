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
#[derive(Component)]
pub struct GenoaCar {
    pub starboard: bool,
}

/// Where the genoa track runs, in the hull's frame: inboard of the rail on the side
/// deck, from abreast the mast back to the forward end of the cockpit.
/// How far outboard of the cockpit coaming the headsail track is laid, metres.
///
/// Keyed to the coaming rather than to a fraction of beam, which is what it was and
/// which put the track *inside the cockpit opening*: at the after end the car sat at
/// 0.69 m from the centreline and the coaming at 0.83, so the whole track and both
/// cars were hanging in the hole. A sheet lead has to be on the side deck, between
/// the coaming and the toerail, because that is the only part of the boat that is
/// there to walk on and to bolt things to.
const TRACK_OUTBOARD: f32 = 0.22;
pub const TRACK_FORWARD: f32 = -0.80;
pub const TRACK_AFT: f32 = 2.20;

/// Fore-and-aft position of the car for a given control setting.
/// Where a headsail lead car actually stands, in the boat's own frame.
///
/// One definition, for the same reason the shroud has one: the fitting is drawn here,
/// the sheet is drawn to here, and the clew is solved against here. The rigging used
/// to compute its own — a flat deck at a fixed beam fraction — so the sheet ran to a
/// point a hand's breadth from the car it was supposed to be turning through.
pub fn genoa_car_at(car: f32, starboard: bool) -> Vec3 {
    let side = if starboard { 1.0 } else { -1.0 };
    Vec3::new(
        side * track_x(car_position(car)),
        lead_height(car_position(car)),
        car_position(car),
    )
}

/// Where the track lies at a station: out on the side deck, clear of the coaming.
///
/// Driven by the side deck, with the coaming only as a floor under it. The first
/// version had it the other way about — the coaming plus an offset, clamped to the
/// rail — which looks reasonable until you notice that the coaming does not exist
/// forward of the cockpit, where `coaming_half_beam` is zero. Most of the track is
/// forward of the cockpit, so most of it collapsed onto the minimum of a quarter of a
/// metre from the centreline: the track and both cars ended up tucked against the
/// mast foot, which is why they could not be found on deck. Measured, before and
/// after, at seven stations along the track: 0.25 m everywhere, against 1.13 to 1.22.
pub fn track_x(z: f32) -> f32 {
    let deck = crate::deck::deck_half_beam(z);
    let inboard = crate::deck::coaming_half_beam(z) + TRACK_OUTBOARD;
    (deck * 0.72).clamp(inboard, deck - 0.18)
}

fn lead_height(z: f32) -> f32 {
    crate::deck::deck_height(z) + 0.06
}

/// The outboard end of the bowsprit.
///
/// Where a gennaker will be tacked. It is there now because the spar has to be drawn,
/// stayed and lived with before anything is set from it — and because a sprit is the
/// single most characteristic thing about the bow of a boat of this kind.
pub fn sprit_end(loa: f32) -> Vec3 {
    let stem = -loa * 0.49;
    Vec3::new(
        0.0,
        crate::deck::deck_height(stem) + 0.32,
        stem - SPRIT_LENGTH,
    )
}

/// How far the sprit reaches beyond the stem, metres.
pub const SPRIT_LENGTH: f32 = 1.9;

/// Where the headsail is made fast at the stem.
pub fn genoa_tack(loa: f32) -> Vec3 {
    let z = -loa * 0.46;
    Vec3::new(0.0, crate::deck::deck_height(z) + 0.12, z)
}

pub fn car_position(car: f32) -> f32 {
    TRACK_FORWARD + car.clamp(0.0, 1.0) * (TRACK_AFT - TRACK_FORWARD)
}

/// Where the mast is stepped, metres aft of the centre of gravity. One definition, in
/// the core, shared by the physics and the drawing.
const MAST_FOOT_Z: f32 = sail_core::boat::packet::MAST_FOOT_Z as f32;
/// Height of the boom above the waterline — the mainsail's tack. Defined in `rig`, so
/// the spar and the sail cannot disagree about it.
use crate::rig::BOOM_HEIGHT;

/// Which sail an entity draws, and the handle to its regenerated mesh.
#[derive(Component)]
pub struct SailSurface {
    pub which: Sail,
    pub mesh: Handle<Mesh>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Sail {
    Main,
    /// Whichever sail is up forward: genoa, Code 0 or gennaker. One entity, because
    /// they are one slot on the boat — you roll one away and set the next in its place.
    Headsail,
    /// The inner sail of the double head rig.
    Staysail,
}

/// A ribbon on the sail. `along_luff` is its height up the sail, 0 at the foot.
#[derive(Component)]
pub struct Telltale {
    pub along_luff: f32,
}

/// Overall length of the scanned hull, metres, after its own node transforms.
///
/// Measured off the file rather than taken from the museum's "eighteen feet", because
/// what matters is what the vertices say: 5.396 m stem to stern, 1.573 m across.
const SCAN_LOA: f32 = 5.396;
/// How far down the scaled scan is dropped so that y = 0 is her waterline.
///
/// The scan is of a boat out of the water, so the file knows nothing about where she
/// floats. This is not guessed: the mesh was integrated, cell by cell, for the volume
/// below a trial waterline, and the plane solved for where that volume equals the
/// displacement. She floats 1.294 m above the bottom of the scaled hull, which also
/// gave the waterline length and beam the physics config now uses.
const HULL_SINKAGE: f32 = -1.294;

/// Height of the gunwale above the waterline, metres.
///
/// Measured off the scan station by station, not taken from the bounding box: the top
/// of that box is the stem head, 1.45 m up, while the rail you would actually put a
/// winch on runs at 0.79 m amidships. Getting this from the box puts every fitting
/// half a metre above the boat.
const DECK: f32 = 0.82;

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    assets: Res<AssetServer>,
    sim: Res<Sim>,
) {
    let cfg = sim.config();

    let underwater_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.11, 0.26, 0.35),
        perceptual_roughness: 0.3,
        ..default()
    });
    // Varnished spruce, not anodised aluminium. She is a boat built of driftwood on
    // Foula in 1898; an alloy rig on her would be the one thing in the scene that
    // nobody would believe.
    let spar_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.52, 0.38, 0.21),
        metallic: 0.0,
        perceptual_roughness: 0.22,
        reflectance: 0.45,
        ..default()
    });
    let sail_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(images.add(crate::sail_texture::cloth())),
        normal_map_texture: Some(images.add(crate::sail_texture::cloth_normal())),
        perceptual_roughness: 0.78,
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
    let staysail_mesh = meshes.add(empty_sail_mesh());

    commands
        .spawn((
            BoatRoot,
            crate::ink::Inked,
            Transform::default(),
            Visibility::default(),
        ))
        .with_children(|b| {
            // Hull: a photogrammetric scan of a real boat, which is the one thing
            // here worth measuring rather than drawing. See `assets/ATTRIBUTION.md`.
            //
            // Only the shape is borrowed. The scan is of an eighteen-foot Shetland
            // fourareen and the boat being sailed is thirty-six feet, so it is scaled
            // to length; what that buys is the proportions, and the length-to-beam of
            // 3.4 the scan carries is exactly the fine, easily-driven hull the
            // delivery polar wanted. The shape the game needed and the shape that
            // turned up were the same shape.
            //
            // The scan arrives with its length along X and Y up, so it is turned a
            // quarter circle to put the bow at -Z where the rest of the boat expects
            // it, and dropped so her waterline is the waterline.
            b.spawn((
                // Bevy 0.20 renamed the old `SceneRoot` to this, scenes having been
                // rebuilt around the world-serialization machinery.
                bevy::world_serialization::WorldAssetRoot(
                    assets.load(GltfAssetLabel::Scene(0).from_asset("venus.glb")),
                ),
                Transform::from_xyz(0.0, HULL_SINKAGE, 0.0)
                    // Minus a quarter turn, not plus. Which end of a double-ender is
                    // the bow is not obvious from a bounding box — both are pointed —
                    // so it was settled from the hull's own shape: her greatest beam
                    // is a little abaft amidships and the longer run from there is
                    // towards -X, which is where the bow of a hull like this is.
                    .with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::splat(cfg.loa as f32 / SCAN_LOA)),
            ));
            // Deck, coachroof and cockpit, built over the open hull — see `deck.rs`.
            // Painted rather than varnished: a working boat's deck is painted, and the
            // scan's own weathered planking is the thing worth looking at.
            let (walked_on, painted) = crate::deck::structure_mesh();
            // Laid planking on everything you stand on, in metres, so a plank is
            // 62 mm wide on the foredeck and on the cabin top alike.
            b.spawn((
                Mesh3d(meshes.add(walked_on)),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::WHITE,
                    base_color_texture: Some(images.add(crate::deck_texture::planking())),
                    normal_map_texture: Some(images.add(crate::deck_texture::planking_normal())),
                    perceptual_roughness: 0.68,
                    ..default()
                })),
                Transform::default(),
            ));
            // Cabin sides and the cockpit well: painted, and glossier than the deck,
            // which is the difference your eye uses to tell a wall from a floor.
            let painted_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(0.88, 0.87, 0.83),
                perceptual_roughness: 0.32,
                double_sided: true,
                cull_mode: None,
                ..default()
            });
            b.spawn((
                Mesh3d(meshes.add(painted)),
                MeshMaterial3d(painted_mat.clone()),
                Transform::default(),
            ));
            // Bottom boards over the bilge, planked like the deck because that is
            // what they are. Without them the open cockpit looks straight through the
            // hull at the sea, which is a thing no boat does for long.
            b.spawn((
                Mesh3d(meshes.add(crate::deck::sole_mesh())),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.62, 0.58, 0.50),
                    base_color_texture: Some(images.add(crate::deck_texture::planking())),
                    perceptual_roughness: 0.80,
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                })),
                Transform::default(),
            ));
            // King plank and covering board, in the same timber as the trim.
            b.spawn((
                Mesh3d(meshes.add(crate::deck::deck_detail_mesh())),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.70, 0.64, 0.52),
                    base_color_texture: Some(images.add(crate::deck_texture::planking())),
                    perceptual_roughness: 0.60,
                    ..default()
                })),
                Transform::default(),
            ));
            // Toerail along the sheer and coaming round the cockpit: varnished, so
            // they read as trim rather than as more deck.
            let varnish = materials.add(StandardMaterial {
                base_color: Color::srgb(0.46, 0.31, 0.16),
                perceptual_roughness: 0.25,
                reflectance: 0.45,
                double_sided: true,
                cull_mode: None,
                ..default()
            });
            let metal = materials.add(StandardMaterial {
                base_color: Color::srgb(0.74, 0.74, 0.76),
                metallic: 0.85,
                perceptual_roughness: 0.30,
                ..default()
            });
            b.spawn((
                Mesh3d(meshes.add(crate::deck::trim_mesh())),
                MeshMaterial3d(varnish.clone()),
                Transform::default(),
            ));
            crate::fittings::spawn(
                b,
                &mut crate::fittings::Workshop {
                    meshes: &mut meshes,
                    images: &mut images,
                    materials: &mut materials,
                    varnish: varnish.clone(),
                    metal: metal.clone(),
                    loa: cfg.loa as f32,
                },
            );

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
            // Mast: from where it is stepped on the deck to the masthead, which are both
            // known, so neither its length nor its position is written down again here.
            //
            // It had a typed-in length of 15.2 m and a typed-in centre, and when the
            // masthead moved — because the mainsail's luff said it had to — the spar
            // stayed where it was and the top of the rig came off it. The second half of
            // that was mine twice over: the constant I replaced in the centre position
            // was not the spreaders, it was the middle of the mast.
            let step = crate::deck::deck_height(MAST_FOOT_Z);
            let mast = crate::rig::MAST_HEAD - step;
            b.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.09, mast))),
                MeshMaterial3d(spar_mat.clone()),
                Transform::from_xyz(0.0, step + mast * 0.5, MAST_FOOT_Z),
            ));
            // Boom, pivoting at the mast. Its own transform carries the sheet angle.
            b.spawn((
                Boom,
                Transform::from_xyz(0.0, BOOM_HEIGHT, MAST_FOOT_Z),
                Visibility::default(),
            ))
            .with_children(|boom| {
                boom.spawn((
                    // The spar is the foot of the sail, so it is drawn at the length
                    // the sail is cut to — the sixth thing in this file that was
                    // described in two places and had drifted apart.
                    Mesh3d(meshes.add(Cylinder::new(
                        0.065,
                        crate::rig::BOOM_LENGTH,
                    ))),
                    MeshMaterial3d(spar_mat.clone()),
                    Transform::from_xyz(0.0, 0.0, crate::rig::BOOM_LENGTH * 0.5)
                        .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                ));
                // The vang bail, under the boom where the vang is made fast.
                //
                // The line was ending in mid-air against the side of the spar. A vang
                // pulls down hard enough to bend a boom, so it is never tied to one —
                // it takes hold of a strap banded right round it, and that strap is
                // the fitting you look for when you want to know where the vang is
                // led. A child of the boom, so it swings with the spar it is on.
                let at = crate::rig::BOOM_LENGTH * crate::rig::VANG_ALONG_BOOM;
                let cheek = meshes.add(Cuboid::new(0.022, 0.16, 0.09));
                for side in [-1.0_f32, 1.0] {
                    boom.spawn((
                        Mesh3d(cheek.clone()),
                        MeshMaterial3d(metal.clone()),
                        Transform::from_xyz(side * 0.058, -0.045, at),
                    ));
                }
                // The pin between the cheeks, which is what the line actually turns on.
                boom.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.016, 0.14))),
                    MeshMaterial3d(metal.clone()),
                    Transform::from_xyz(0.0, -0.115, at)
                        .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                ));
            });
            // And the padeye it pulls against, at the mast foot.
            b.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.14, 0.11, 0.04))),
                MeshMaterial3d(metal.clone()),
                Transform::from_xyz(
                    0.0,
                    crate::deck::deck_height(MAST_FOOT_Z + 0.2) + 0.03,
                    MAST_FOOT_Z + 0.2,
                ),
            ));
            // The staysail's club boom, pivoting at its own tack. Self-tacking: the
            // sheet does not change sides, the boom does.
            b.spawn((
                StaysailBoom,
                Transform::from_translation(staysail_tack()),
                Visibility::default(),
            ))
            .with_children(|club| {
                let length = staysail_boom_length(cfg.staysail.area as f32);
                club.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.045, length))),
                    MeshMaterial3d(spar_mat.clone()),
                    Transform::from_xyz(0.0, 0.11, length * 0.5)
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
                    which: Sail::Headsail,
                    mesh: genoa_mesh.clone(),
                },
                Mesh3d(genoa_mesh.clone()),
                MeshMaterial3d(sail_mat.clone()),
                Transform::default(),
            ));
            b.spawn((
                SailSurface {
                    which: Sail::Staysail,
                    mesh: staysail_mesh.clone(),
                },
                Mesh3d(staysail_mesh.clone()),
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
            // Genoa tracks and their cars, one each side. The car is the headsail's
            // only shape control, so it has to be something you can watch move —
            // a number on a panel is not a trim you can learn by eye.
            let track_length = TRACK_AFT - TRACK_FORWARD;
            let track_mesh = meshes.add(Cuboid::new(0.05, 0.03, track_length));
            let car_mesh = meshes.add(Cuboid::new(0.085, 0.07, 0.17));
            let sheave_mesh = meshes.add(Cylinder::new(0.055, 0.03));
            for starboard in [false, true] {
                let side = if starboard { 1.0 } else { -1.0 };
                let mid = (TRACK_FORWARD + TRACK_AFT) * 0.5;
                b.spawn((
                    Mesh3d(track_mesh.clone()),
                    MeshMaterial3d(spar_mat.clone()),
                    Transform::from_xyz(
                        side * track_x(mid),
                        crate::deck::deck_height(mid) + 0.02,
                        mid,
                    ),
                ));
                b.spawn((
                    GenoaCar { starboard },
                    Mesh3d(car_mesh.clone()),
                    MeshMaterial3d(underwater_mat.clone()),
                    Transform::from_translation(genoa_car_at(0.4, starboard)),
                ))
                .with_children(|car| {
                    // The sheave. The sheet does not end at the car, it turns through
                    // this and runs on to the winch, which is why hauling brings the
                    // clew right down to the fitting instead of stopping short of it.
                    car.spawn((
                        Mesh3d(sheave_mesh.clone()),
                        MeshMaterial3d(spar_mat.clone()),
                        Transform::from_xyz(0.0, 0.07, 0.0)
                            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                    ));
                });
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
pub fn update_hull(
    sim: Res<Sim>,
    origin: Res<frame::RenderOrigin>,
    mut q: Query<&mut Transform, With<BoatRoot>>,
) {
    for mut t in &mut q {
        let st = sim.render_state();
        t.translation = origin.hull_translation(&st);
        t.rotation = frame::attitude(&st);
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

/// The club boom swings to whatever angle the staysail has found, and goes away with
/// the sail — a boom lying on the foredeck with no sail on it is not a thing a boat has
/// in the middle of a passage.
pub fn update_staysail_boom(
    sim: Res<Sim>,
    mut q: Query<(&mut Transform, &mut Visibility), With<StaysailBoom>>,
) {
    let t_ = &sim.sim.telemetry;
    let cfg = sim.config();
    let side = -t_.awa.signum() as f32;
    let angle = side
        * cfg
            .staysail
            .profile
            .chord_angle(sim.controls.staysail_sheet) as f32;
    for (mut tr, mut vis) in &mut q {
        tr.rotation = Quat::from_rotation_y(angle);
        *vis = if sim.controls.staysail < 0.02 {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
}

/// How far the boom end lifts per radian of leech twist.
///
/// The boom swings sideways on its sheet and it also rides up and down, and the second
/// one is not decoration: a mainsheet tackle and a vang exist to stop it, and what they
/// are stopping is the leech of the sail pulling the end of the spar skyward. Which is
/// exactly the twist the force model already computes — the leech falls off aloft
/// *because* the boom has lifted — so the two are the same number seen from two ends,
/// and tying them together is the only way they can stay honest.
///
/// A third of the twist angle, so the full twenty degrees of twist with the vang right
/// off lifts the end of a six-metre boom by two thirds of a metre, and hardening the
/// vang puts it back down.
const BOOM_LIFT_PER_TWIST: f32 = 0.34;

/// Where the boom is: swung out on its sheet, and lifted by its leech.
pub fn boom_attitude(t: &sail_core::state::Telemetry) -> (f32, f32) {
    let swing = -t.awa.signum() as f32 * t.main.chord_angle as f32;
    (swing, t.twist as f32 * BOOM_LIFT_PER_TWIST)
}

pub fn update_boom(sim: Res<Sim>, mut q: Query<&mut Transform, With<Boom>>) {
    let (swing, lift) = boom_attitude(&sim.sim.telemetry);
    for mut tr in &mut q {
        // Swung about the mast first, then lifted about the gooseneck, which is the
        // order the two joints come in on the boat.
        tr.rotation = Quat::from_rotation_y(swing) * Quat::from_rotation_x(-lift);
    }
}

/// Slide the cars to wherever the control says they are.
pub fn update_genoa_cars(sim: Res<Sim>, mut q: Query<(&GenoaCar, &mut Transform)>) {
    // The side deck curves, so the car rides up and inboard as it goes forward. That
    // is a real track on a real sheer rather than a straight rail in mid-air.
    for (car, mut transform) in &mut q {
        transform.translation = genoa_car_at(sim.controls.genoa_car as f32, car.starboard);
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
/// Put the telltales on the sail and let them live.
///
/// They had come adrift from it, and for the usual reason: their position was worked
/// out in the boat's own frame — a height above a deck constant, a distance aft of the
/// mast — while the sail itself had since acquired a belly, twist, a reef that moves
/// its head, and a shroud to lean on. Two descriptions of one surface, and only one of
/// them was being kept up to date. They are sampled off the sail's own geometry now,
/// a little abaft the luff, which is where a telltale is sewn.
///
/// And their behaviour was backwards. They stood rigid in attached flow and only moved
/// when the sail was luffing, which is the opposite of what a strip of wool does: in
/// good flow it streams aft and *shivers*, never quite still, and that restless
/// steadiness is the whole signal. It is when the flow goes that it stops streaming —
/// first lifting and wandering, then falling against the cloth when the sail stalls.
///
/// Which matters, because this is an instrument. A trimmer reads the windward
/// telltales to find the groove, and a telltale that is perfectly still is not telling
/// anyone anything.
pub fn update_telltales(sim: Res<Sim>, time: Res<Time>, mut q: Query<(&Telltale, &mut Transform)>) {
    let t_ = &sim.sim.telemetry;
    let side = -t_.awa.signum() as f32;
    let luff = t_.main.coeffs.luffing as f32;
    let stall = t_.main.coeffs.stalled as f32;
    let now = time.elapsed_secs();

    let reef = sim.controls.reef as f32;
    let foot = crate::rig::BOOM_LENGTH * (1.0 - 0.35 * reef);
    let boom = side * t_.main.chord_angle as f32;
    let tack = Vec3::new(0.0, BOOM_HEIGHT, MAST_FOOT_Z);
    let head = Vec3::new(
        0.0,
        BOOM_HEIGHT + (15.4 - BOOM_HEIGHT) * (1.0 - 0.45 * reef),
        MAST_FOOT_Z,
    );
    let clew = tack + Vec3::new(boom.sin() * foot, 0.0, boom.cos() * foot);

    for (tt, mut tr) in &mut q {
        // The same three corners the sail mesh is built from, so the two cannot drift
        // apart again — and a short way out along the chord, where the wool is sewn.
        let u = tt.along_luff;
        let on_luff = tack.lerp(head, u);
        let to_leech = clew.lerp(head, u) - on_luff;
        let chord = to_leech.length().max(0.01);
        let along = to_leech / chord;
        // Clear of the cloth on the windward side, which is the set you read.
        let normal = Vec3::Y.cross(along).normalize_or_zero();
        let out = if normal.x * side < 0.0 {
            normal
        } else {
            -normal
        };
        tr.translation = on_luff + along * (chord * 0.12).min(0.75) + out * 0.04;

        // Three scales of motion, and the slowest is the one that reads.
        //
        // A shiver that never stops, a slow wander that makes the shiver look alive
        // rather than mechanical, and — once the flow lets go — a lift and a thrash.
        // The frequencies are deliberately not multiples of each other, or the three
        // lock together within a second and the whole thing beats.
        let phase = u * 7.3;
        let shiver = (now * 13.7 + phase).sin() * 0.055;
        let wander = (now * 1.9 + phase * 2.1).sin() * 0.085;
        let thrash = ((now * 9.1 + phase).sin() + (now * 5.3 + phase * 1.7).sin()) * 0.55 * luff;
        let lift = 0.35 * luff;
        // Stalled: it falls back against the sail and goes dead, which is the other
        // half of the reading and the half most simulators leave out.
        let dead = 1.0 - stall * 0.85;
        let swing = (shiver + wander + thrash) * dead + lift;
        let droop = -stall * 0.9;

        tr.rotation = Quat::from_rotation_arc(Vec3::NEG_Z, along)
            * Quat::from_rotation_y(swing)
            * Quat::from_rotation_x(droop);
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
    // Only the leeward shroud. A sail bears on the wire the wind is pressing it
    // against and is *supposed* to be to leeward of the other one — that is where it
    // lives. Handing both to the constraint made the windward shroud an obstacle too,
    // and since the test is "the cloth may not be to leeward of the wire", every point
    // on the mainsail's luff counted as having gone through it. Measured on the
    // half-height luff: 0.84 m from the windward shroud, which the falloff weighted at
    // 0.87, which shoved the cloth three quarters of a metre to windward and collapsed
    // the sail.
    let shrouds = shroud_polyline(cfg.beam as f32, side > 0.0, t_.heel as f32, side);

    for (surface, mut visibility) in &mut q {
        *visibility = match surface.which {
            Sail::Main if sim.controls.reef >= sail_core::boat::DROPPED => Visibility::Hidden,
            Sail::Headsail if sim.controls.furl > 0.985 => Visibility::Hidden,
            Sail::Staysail if sim.controls.staysail < 0.02 => Visibility::Hidden,
            _ => Visibility::Inherited,
        };
        let (forces, geometry) = match surface.which {
            Sail::Main => {
                // A slab reef takes the luff down and leaves the foot where it was: the
                // pennant runs through a cheek block at the *end* of the boom, so the
                // new clew lands within a hand's breadth of the old one. What you lose
                // is hoist.
                let foot = main_foot(&cfg, &sim.controls);
                let (_, lift) = boom_attitude(t_);
                // And the hoist comes down by the same scale, because the reef band is
                // cut parallel to the foot and takes the sail in both ways at once.
                let shrink = main_reef_scale(&cfg, &sim.controls);
                let boom = side * t_.main.chord_angle as f32;
                let tack = Vec3::new(0.0, BOOM_HEIGHT, MAST_FOOT_Z);
                (
                    t_.main,
                    SailGeometry {
                        tack,
                        // Reefing pulls the head down the mast, not the foot up.
                        head: Vec3::new(
                            0.0,
                            BOOM_HEIGHT + (DECK + 15.4 - BOOM_HEIGHT) * shrink,
                            MAST_FOOT_Z,
                        ),
                        // The clew is the boom end: level with the tack, because the
                        // boom is level. That is what made the horizontal-chord
                        // shortcut look right until a headsail turned up.
                        // On the boom end, lifted with it: the clew of a mainsail is
                        // shackled to the spar, so if the spar's end rises the corner
                        // rises with it. Drawing the sail flat while the boom lifted
                        // would put the cloth through the spar.
                        clew: tack
                            + Vec3::new(
                                boom.sin() * foot * lift.cos(),
                                foot * lift.sin(),
                                boom.cos() * foot * lift.cos(),
                            ),
                    },
                )
            }
            Sail::Staysail => {
                // Small, low, and set on its own stay a third of the way aft from the
                // stem. Self-tacking, so its angle is not a control: it finds the best
                // one its track allows, which is the same number the force model uses.
                let set = sim.controls.staysail as f32;
                let angle = side
                    * cfg
                        .staysail
                        .profile
                        .chord_angle(sim.controls.staysail_sheet) as f32;
                let tack = staysail_tack();
                let head = staysail_head();
                let foot = staysail_boom_length(cfg.staysail.area as f32);
                // Hoisted, not furled: it climbs its stay as the halyard goes up.
                (
                    t_.jib,
                    SailGeometry {
                        tack,
                        head: tack.lerp(head, 0.18 + 0.82 * set),
                        // On the boom end, because that is where a club-footed sail's
                        // clew is: shackled to the spar, not flying free.
                        clew: staysail_boom_end(angle, foot),
                    },
                )
            }
            Sail::Headsail => (
                t_.jib,
                headsail_shape(&cfg, &sim.controls, side, t_.jib.chord_angle as f32),
            ),
        };
        if let Some(mut mesh) = meshes.get_mut(&surface.mesh) {
            // Both sails twist. The main's comes from the vang arguing with the
            // mainsheet's angle; the headsail's comes from its lead car, which is the
            // only shape control it has and so had better be visible in the cloth.
            let twist = side
                * match surface.which {
                    Sail::Main => t_.twist as f32,
                    Sail::Headsail => t_.jib_twist as f32,
                    Sail::Staysail => t_.jib_twist as f32 * 0.4,
                };
            build_sail(
                &mut mesh,
                &geometry,
                &SailTrim {
                    lee: side,
                    // The main's foot is hauled by the outhaul; a headsail's is pulled
                    // straight by its sheet, which is the same line that trims it, so
                    // there is nothing left over to let it hang.
                    foot_sag: if surface.which == Sail::Main {
                        0.42 * (1.0 - sim.controls.outhaul as f32)
                    } else {
                        0.0
                    },
                    twist,
                    luffing: forces.coeffs.luffing as f32,
                    // What the sail is cut to, plus what the wind puts into it.
                    //
                    // The camber in the trim is the designed shape — what the outhaul
                    // and the lead car are arguing about upwind. Off the wind the sail
                    // stops being a wing and becomes a bag: with the sheet eased there is
                    // nothing tensioning the leech, the cloth takes the shape it was cut
                    // to and then some, and the foot comes straight between tack and clew
                    // because that is the one edge still under load. A sailmaker builds
                    // that in deliberately — the broadseam is there to put a belly in
                    // exactly this condition — so a reaching sail drawn with its upwind
                    // section looks starved, which is what it looked like.
                    // How much a sail bags out when it is eased is not the same for all
                    // of them, and treating it as one number made the mainsail wrong.
                    //
                    // A main has a boom holding its foot straight and a leech the vang
                    // and the sheet are both pulling on: eased on a reach it gets
                    // fuller, but it is still a sail stretched between two spars and it
                    // never goes anywhere near a bag. A sail set flying has nothing on
                    // any edge except its own tension, and that is the one that balloons.
                    draft: forces.camber as f32
                        * (1.0
                            + if surface.which == Sail::Main { 0.55 } else { 1.9 }
                                * forces.chord_angle.abs().sin() as f32),
                    // Where the deepest part of the sail sits, measured from the luff.
                    //
                    // Forty-two percent for a working sail, and it moves aft as the sail
                    // is loaded and deepened — which is what a trimmer is watching when
                    // he looks at the draft stripes, and the reason the outhaul and the
                    // vang exist: both of them pull it forward again. The drawing had it
                    // at the exact middle of the chord, which is a shape no sail takes.
                    draft_at: 0.42 + 0.14 * (forces.camber as f32 / 0.15).clamp(0.0, 1.0),
                    now,
                    // Once the sail is eased far enough it is sheeted outboard of the
                    // cap shroud and flying clear of the rig, so the wire stops being
                    // an obstacle and the cloth must stop being held off it. The same
                    // angle the force model uses, because the picture and the physics
                    // have to agree about where the sail is — the fault this file keeps
                    // being caught by.
                    shrouds: if surface.which == Sail::Headsail
                        && t_.jib.chord_angle.abs() > cfg.headsail_bears_at + sail_core::units::rad(30.0)
                    {
                        &[]
                    } else {
                        &shrouds
                    },
                    // The sails on the bow have to be kept out of the mast. A sail set
                    // flying is tacked a metre and a half ahead of the rig, which clears
                    // its *tack* and not its leech: eased on a reach the cloth sweeps
                    // aft past the spar, and a gennaker was being drawn straight through
                    // it.
                    //
                    // Including the staysail. It was excepted while its stay landed
                    // fifteen centimetres forward of the spar's axis — inside the
                    // clearance radius, so the constraint shoved the whole sail sideways
                    // from the head down. The stay has been moved forward to where an
                    // inner stay's tang actually is, which leaves the head clear and the
                    // constraint free to do its one job: stop the cloth being drawn
                    // through the mast, which is what it was doing.
                    clear_mast: surface.which != Sail::Main,
                },
            );
        }
    }
}


/// How far the clew sits from the gooseneck along the boom.
///
/// This is what the outhaul does, and it is a small thing: it hauls the clew the last
/// hand's breadth out towards the boom end and holds it there, which tightens the foot
/// and takes the belly out of the bottom of the sail. Twenty-two centimetres of travel.
/// It is not a way of shortening the sail — that is the reef's job — and drawing it as
/// one was wrong in a way that showed, because a sail whose whole foot changes length
/// with a trim control is a sail nobody has ever sheeted.
///
/// Reefing moves the point the outhaul is working on. Each slab's clew cringle is cut a
/// little inboard of the one below it, so when the first reef goes in the whole
/// argument moves forward along the spar and starts again from there: the pennant hauls
/// the new clew out towards the same boom end, with the same small travel in hand.
/// How much smaller the reefed sail is, as a scale on both of its edges.
///
/// A slab reef is cut *parallel to the foot*, so it takes the sail in in both
/// directions at once: the hoist comes down and the foot comes in with it, and the
/// reefed sail is the same triangle smaller. Which is why the reefed clew sits well
/// forward of the boom end, however far out the pennant hauls it — reported from the
/// deck, and the model had the whole of the area coming out of the hoist with the foot
/// moving sixteen centimetres. Measured against what the configuration says the reefs
/// are worth: 5.80 m of foot becomes 5.10 at the first reef and 4.29 at the second,
/// not 5.64 and 5.48.
///
/// Area goes as luff times foot, so each edge scales as the square root of what is
/// left.
pub fn main_reef_scale(cfg: &sail_core::boat::BoatConfig, controls: &sail_core::state::Controls) -> f32 {
    let slab = (controls.reef as f32 * 3.0).round();
    let taken = cfg.max_reef as f32 * slab / 3.0;
    (1.0 - taken).max(0.0).sqrt()
}

pub fn main_foot(cfg: &sail_core::boat::BoatConfig, controls: &sail_core::state::Controls) -> f32 {
    /// What the outhaul can move the clew by, metres.
    ///
    /// Twenty-two centimetres on a full sail — which is what an outhaul's travel is on a
    /// six-metre boom, and which is also barely visible from the cockpit, which is fair
    /// because it is barely visible on a boat. Reefed it is a different line doing a
    /// different job: the reef cringle starts bunched in the gathered cloth and has to
    /// be hauled the whole way out to the boom end, so the travel is twice as long and
    /// you can see the corner move along the spar as you winch it.
    const TRAVEL: f32 = 0.22;
    const TRAVEL_REEFED: f32 = 0.48;
    let slab = (controls.reef as f32 * 3.0).round();
    let hauled = controls.outhaul as f32;
    let travel = if slab > 0.5 { TRAVEL_REEFED } else { TRAVEL };
    crate::rig::BOOM_LENGTH * main_reef_scale(cfg, controls) - travel * (1.0 - hauled)
}

/// What the rig actually measures, printed once on request.
///
/// `SAIL_RIG=1`. The verification loop on this project has been: somebody looks at the
/// screen, says what is wrong, and I guess at which of two descriptions of the same
/// thing has drifted. Four faults in one evening were found that way and two of them
/// took three attempts. Every one of them would have fallen straight out of this list —
/// a mast shorter than its own mainsail's luff, a boom longer than its area allows, a
/// Code 0 whose foot does not reach the mast, a sail plan belonging to a different boat.
pub fn report(sim: Res<Sim>) {
    if !std::env::var("SAIL_RIG").is_ok_and(|v| v != "0") {
        return;
    }
    let cfg = sim.config();
    let p = sail_core::boat::packet::MAST_HEAD;
    info!("rig: mast head {p:.2} m, forestay {:.2}, boom {:.2} x {:.2} high, spreaders {:.2}",
        sail_core::boat::packet::FORESTAY_HEAD,
        crate::rig::BOOM_LENGTH,
        crate::rig::BOOM_HEIGHT,
        crate::rig::SPREADER_HEIGHT);
    let luff_main = crate::rig::MAST_HEAD - crate::rig::BOOM_HEIGHT;
    info!(
        "     mainsail: luff drawn {:.2} m against {:.2} configured, foot {:.2}, area {:.1} m2",
        luff_main, sail_core::boat::packet::MAIN_LUFF, crate::rig::BOOM_LENGTH, cfg.main.area
    );
    for kind in [
        sail_core::state::Headsail::Genoa,
        sail_core::state::Headsail::Code0,
        sail_core::state::Headsail::Gennaker,
    ] {
        let controls = sail_core::state::Controls {
            headsail: kind,
            furl: 0.0,
            jib_sheet: 0.0,
            ..Default::default()
        };
        let g = headsail_shape(&cfg, &controls, -1.0, sail_core::units::rad(14.0) as f32);
        // LP is the *perpendicular* from the clew to the luff — the measurement a
        // sailmaker quotes a headsail by — and the base it is quoted against is J, the
        // deck distance from that sail's tack to the mast. The first attempt here
        // multiplied the foot by the clew's athwartships direction, which is neither,
        // and reported every sail at a quarter of its real size. An instrument's first
        // job is to be right about the thing nobody else is measuring.
        let luff_dir = (g.head - g.tack).normalize_or_zero();
        let to_clew = g.clew - g.tack;
        let lp = (to_clew - luff_dir * to_clew.dot(luff_dir)).length();
        let base = (MAST_FOOT_Z - g.tack.z).abs();
        info!(
            "     {:8}: area {:5.1} m2  luff {:5.2}  foot {:5.2}  clew z {:+5.2}  LP/base {:4.0}%",
            kind.name(),
            cfg.headsail(kind).area,
            (g.head - g.tack).length(),
            (g.clew - g.tack).length(),
            g.clew.z,
            lp / base * 100.0,
        );
    }
}

/// The three corners of whichever sail is up forward.
///
/// One definition, used by the cloth and by the sheet that pulls on it. They were two,
/// and the sheet's copy still had the old typed-in foot length in it after the sail's
/// had been replaced by the measured one — which is the same fault as the shroud, the
/// lead car, the vang bail, the telltales and the sail areas, caught this time before
/// anybody had to look at it on the water.
pub fn headsail_shape(
    cfg: &sail_core::boat::BoatConfig,
    controls: &sail_core::state::Controls,
    side: f32,
    chord: f32,
) -> SailGeometry {
    let angle = side * chord;
    let kind = controls.headsail;
    if kind == sail_core::state::Headsail::Genoa {
        // A rolling genoa loses area from its foot and leech as the drum turns, so it
        // visibly shrinks on the stay rather than vanishing.
        let out = 1.0 - controls.furl as f32;
        let tack = genoa_tack(cfg.loa as f32);
        // The genoa's head is the forestay's head: it is hanked to it.
        let head = Vec3::new(0.0, crate::rig::FORESTAY_HEAD - 0.1, MAST_FOOT_Z);
        let foot = foot_for_area(
            cfg.genoa.area as f32,
            head - tack,
            Vec3::new(angle.sin(), 0.0, angle.cos()),
            1.15,
        ) * out;
        // The head does not move. A roller genoa is wound round its own stay: it gets
        // narrower from the foot and the leech while the halyard stays where it is, and
        // the head stays at the top until the whole sail is gone. Walking it down the
        // wire as the drum turned was drawing a sail being *lowered*, which is a
        // different operation with a different line.
        //
        // This was written once before and did not take — the edit was aimed at text
        // that had since moved, and the commit claimed it anyway. Measured in the
        // running app this time: with the drum nearly home the head was sitting five
        // metres above the tack instead of fifteen.
        let hoisted = head;
        // A genoa is cut low: its foot sweeps the deck and its clew rides just above
        // the rail. That is what the "rise" is — how far above the tack the sail is cut
        // to carry its clew — and it is most of what tells a genoa from a reaching sail
        // at a glance. The flying sails are cut away underneath, high-clewed so they
        // clear the pulpit and the water, and they were all being cut the same.
        // Cut high enough to keep its own foot off the deck: at a rise of 0.16 the clew
        // sat barely a metre above the planking and the cloth below it went through the
        // foredeck. A genoa is cut low. It is not cut into the boat.
        let cloth = Cloth::cut(tack, hoisted, foot, 0.30);
        // The car is the headsail's shape control: forward for a deep sail, aft for a
        // flat one. That depth is what moves the ring the clew rides.
        let belly = 1.0 - controls.genoa_car as f32;
        let car = genoa_car_at(controls.genoa_car as f32, side > 0.0);
        // What the perpendicular has to be for the triangle to hold the sail's area.
        let want = 2.0 * (cfg.genoa.area as f32 / 1.15) / (hoisted - tack).length() * out;
        let fitted = Cloth::cut(tack, hoisted, fitted_foot(tack, hoisted, &cloth, belly, angle, car, want), 0.30);
        return SailGeometry {
            tack,
            head: hoisted,
            clew: clew_on_cloth(tack, hoisted, &fitted, belly, angle, car),
        };
    }
    // A sail set flying: tacked out on the sprit end, hoisted to the masthead, clew
    // sheeted away aft. Nothing holds its luff but its own tension.
    let gennaker = kind == sail_core::state::Headsail::Gennaker;
    // The same hoist curve the force model uses: up, or away, and the sliver between is
    // the sail folding rather than a size to sail in.
    let out =
        1.0 - sail_core::units::smoothstep(0.10, 0.45, controls.furl) as f32;
    let tack = sprit_end(cfg.loa as f32);
    // Both go to the masthead. A sail set flying is hoisted on its own halyard to the
    // very top of the spar — above the forestay, which is where its extra hoist comes
    // from and half the reason it is worth setting.
    let head = Vec3::new(0.0, crate::rig::MAST_HEAD - 0.12, MAST_FOOT_Z);
    // How much more cloth there is in the sail than in the flat triangle it spans, which
    // is a question about how deeply it is cut and nothing else.
    //
    // A gennaker is a bag: half again. A Code 0 is *flat* — that is the whole of what
    // makes it a Code 0 rather than a spinnaker, and what lets it be carried at
    // forty-five degrees of apparent wind — so its cloth is only a few percent over its
    // triangle. It was being given a quarter, which is a reaching spinnaker's number,
    // and the sail came out small: measured against its own fore-triangle base it was
    // setting at an LP of 116 percent when a Code 0 is 130 and up, and with the sheet
    // hard in the clew did not even reach the mast.
    //
    //     cloth/triangle   1.25 -> foot 5.66 m, LP 116% of J
    //                      1.15 ->      6.15     126%
    //                      1.08 ->      6.55     136%
    let fullness = if gennaker { 1.45 } else { 1.08 };
    let foot = foot_for_area(
        cfg.headsail(kind).area as f32,
        head - tack,
        Vec3::new(angle.sin(), 0.0, angle.cos()),
        fullness,
    ) * out;
    let hoisted = tack.lerp(head, 0.3 + 0.7 * out);
    let cloth = Cloth::flying(tack, hoisted, foot, if gennaker { 0.90 } else { 0.93 });
    // A gennaker is cut deep and a Code 0 nearly flat, and neither has a car to change
    // it: the depth is the sailmaker's, not the trimmer's.
    let belly = if gennaker { 1.0 } else { 0.25 };
    // A sail set flying sheets to a block on the quarter, right aft — the same point the
    // sheet is drawn to, so the cloth and the rope agree about where they meet.
    let block = Vec3::new(
        side * 1.15,
        crate::deck::deck_height(3.9) + 0.07,
        3.9,
    );
    let want = 2.0 * (cfg.headsail(kind).area as f32 / fullness) / (hoisted - tack).length() * out;
    let fitted = Cloth::flying(
        tack,
        hoisted,
        fitted_foot(tack, hoisted, &cloth, belly, angle, block, want),
        if gennaker { 0.90 } else { 0.93 },
    );
    SailGeometry {
        tack,
        head: hoisted,
        clew: clew_on_cloth(tack, hoisted, &fitted, belly, angle, block),
    }
}

/// Marks the staysail's club boom, which swings on its own.
#[derive(Component)]
pub struct StaysailBoom;

/// Where the staysail is tacked down, on its own stay a third of the way aft.
pub fn staysail_tack() -> Vec3 {
    Vec3::new(0.0, DECK + 0.12, MAST_FOOT_Z - 3.3)
}

/// The head of it, where the inner stay meets the mast.
///
/// Forty centimetres forward of the spar's axis, not fifteen. At fifteen the head sat
/// *inside* the mast's own clearance radius, so the sail could not be given the mast
/// constraint at all without the constraint shoving the whole sail sideways from the
/// head down — which is why it was excepted from it, and why it was then drawn straight
/// through the mast. An inner stay lands on a tang bolted to the front of the spar
/// anyway; it has never been on the centreline of it.
pub fn staysail_head() -> Vec3 {
    Vec3::new(0.0, crate::rig::INNER_STAY_HEAD, MAST_FOOT_Z - 0.40)
}

/// The club boom's length, which is the sail's foot and therefore not a free choice.
///
/// A spar does not change length with the trim, so it is cut for a working angle and
/// the sail is cut to the spar. Taken at thirty degrees of chord, which is where a
/// self-tacker spends most of its life.
pub fn staysail_boom_length(area: f32) -> f32 {
    foot_for_area(
        area,
        staysail_head() - staysail_tack(),
        Vec3::new(0.5, 0.0, 0.866),
        1.15,
    )
}

/// Where the end of that boom is, which is also the sail's clew. One definition, so
/// the spar and the cloth cannot end up in different places.
pub fn staysail_boom_end(angle: f32, length: f32) -> Vec3 {
    staysail_tack() + Vec3::new(angle.sin() * length, 0.22, angle.cos() * length)
}

/// The foot length that makes a drawn sail the size the physics thinks it is.
///
/// Every sail in this file used to have its foot typed in as a number that looked right,
/// and every one of them was wrong — the main by 36 percent small, the genoa by 32
/// percent large, the gennaker by 38 percent small. Which is the same fault as the
/// shroud, the lead car, the vang bail and the telltales: one thing described twice, in
/// two places, with nothing keeping the descriptions together. A sail is a measured
/// quantity in `BoatConfig`; the triangle that draws it has no business inventing its
/// own.
///
/// A sail is not flat, so the cloth is larger than the triangle between its three
/// corners — roach on a main, camber and a round leech on a headsail. Fifteen percent
/// covers a working sail. A spinnaker-cut sail is a different matter and the caller says
/// so: a gennaker's quoted area is the cloth in it, which is most of twice the flat
/// triangle it spans, and taking it for a triangle gave the sail a ten-metre foot and a
/// clew out beyond the spreaders.
fn foot_for_area(area: f32, luff: Vec3, foot_dir: Vec3, cloth_over_triangle: f32) -> f32 {
    let cloth_over_triangle = cloth_over_triangle.max(1.0);
    let dir = foot_dir.normalize_or_zero();
    // Area of a triangle is half the cross product, so the foot that gives the wanted
    // area falls straight out of it — and it grows when the sail is sheeted in hard,
    // which is correct: at a small angle between luff and foot the triangle is narrow.
    let across = luff.cross(dir).length().max(0.15);
    (2.0 * area / cloth_over_triangle / across).clamp(1.0, 16.0)
}

/// Where a cap shroud runs, in the boat's own frame.
///
/// Defined once, because two things need it and they must not disagree: the rigging
/// draws it, and the sails lean on it. A sail leaning on a wire that is drawn
/// somewhere else is worse than no constraint at all.
pub fn shroud(beam: f32, starboard: bool) -> (Vec3, Vec3) {
    let side = if starboard { 1.0 } else { -1.0 };
    (
        Vec3::new(0.0, crate::rig::MAST_HEAD, MAST_FOOT_Z),
        Vec3::new(side * beam * 0.45, DECK, MAST_FOOT_Z + 0.4),
    )
}

/// The leeward shroud as the rigging actually draws it, sagged and all.
///
/// Sampled into a short polyline rather than used as a straight chord. The chord was
/// the third time in this file that two places computed the same piece of geometry and
/// disagreed: the cloth was held off the straight line between the shroud's ends while
/// the wire itself was drawn hanging below it, so it cut through a sail that was, by
/// its own lights, perfectly clear. Both now come from the same two functions.
pub fn shroud_polyline(beam: f32, starboard: bool, heel: f32, lee: f32) -> Vec<Vec3> {
    let (top, foot) = shroud(beam, starboard);
    let slack = crate::rig::shroud_slack(starboard, heel, lee);
    let sag = crate::rig::line_sag((foot - top).length(), 1.0 - slack, true);
    (0..9)
        .map(|i| crate::rig::curve(top, foot, sag, i as f32 / 8.0))
        .collect()
}

/// How close the cloth may come to the wire, metres.
const SHROUD_RADIUS: f32 = 0.05;
/// How far either side of the wire the sail is dragged into the crease, metres.
const SHROUD_INFLUENCE: f32 = 0.9;

/// Hold a point of sailcloth off the standing rigging.
///
/// An eased headsail does not pass through the shrouds. It lies *against* them, and
/// what you see is a crease running up the sail from each wire — which is also the
/// thing that limits how far a genoa can be eased on a reach, and the reason the sail
/// chafes where it does.
///
/// Formulated as "the cloth may not be further to leeward than the wire", because that
/// is the direction the wind is pressing it: find the nearest point on the shroud,
/// split the offset into a component along the sail's own leeward normal and one
/// across it, and clamp the first. The second drives a smooth falloff, so the crease
/// spreads either side of the wire rather than being a dimple where they touch.
fn clear_of_shrouds(point: Vec3, normal: Vec3, shrouds: &[Vec3]) -> Vec3 {
    let mut p = point;
    for pair in shrouds.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        let run = *b - *a;
        let len2 = run.length_squared().max(1e-6);
        let t = ((p - *a).dot(run) / len2).clamp(0.0, 1.0);
        let nearest = *a + run * t;
        let v = p - nearest;
        let along = v.dot(normal);
        let lateral = (v - normal * along).length();
        if lateral >= SHROUD_INFLUENCE {
            continue;
        }
        // Windward of the wire is `along <= -radius`; anything else has gone through.
        let u = 1.0 - lateral / SHROUD_INFLUENCE;
        let weight = u * u * (3.0 - 2.0 * u);
        // Rounded off in the depth as well as across. The push already faded sideways,
        // but along the wire's own normal it switched on at a point: zero one side of
        // the surface and growing linearly the other, which is continuous and has a
        // corner in it — and a corner in a displacement is a crease in the cloth. Same
        // fault as the hard floor over the deck, in the second of the three places that
        // shove a sail about.
        p -= normal * soft_push(along + SHROUD_RADIUS, 0.10) * weight;
    }
    p
}

/// Radius the cloth is kept outside the mast, metres. The spar plus a hand's width.
const MAST_CLEARANCE: f32 = 0.16;

/// Push a point of sailcloth out of the mast.
///
/// Radially, which is the one unambiguous answer: a cylinder has no windward or
/// leeward side, and whichever way the sail has already gone round it is the way it
/// should keep going. The shroud constraint cannot be reused here — it asks "is the
/// cloth to leeward of the wire", and for a spar standing in the middle of the sail
/// plan that question has no correct answer.
///
/// Only the headsail is held off. The mainsail's luff is bent *on* the mast; pushing
/// it clear would tear the sail off the spar it is attached to.
/// Everything that pushes a piece of sailcloth about, in one place.
///
/// The mast, the shrouds and the deck, applied in that order, to a single point. It
/// exists because the sheet has to end where the *cloth* ends and the cloth is not where
/// the geometry says: the corner is computed from three points and then shoved by
/// whatever it is leaning on, so a rope drawn to the unshoved corner misses the sail by
/// however much the shove was. Reported from the deck as the sheet not reaching the
/// clew, and it did not.
pub fn constrained(point: Vec3, normal: Vec3, shrouds: &[Vec3], clear_mast: bool) -> Vec3 {
    let point = clear_of_shrouds(point, normal, shrouds);
    let point = if clear_mast { clear_of_mast(point) } else { point };
    let floor = crate::deck::deck_height(point.z) + 0.05;
    const BAND: f32 = 0.25;
    let above = (point.y - floor) / BAND;
    let eased = if above > 8.0 {
        point.y
    } else {
        floor + BAND * (1.0 + above.exp()).ln()
    };
    Vec3::new(point.x, eased, point.z)
}

/// A push that has no corner in it: zero well clear, the full amount well inside, and a
/// smooth bend in between.
///
/// `softplus`, which is the positive part of a number with its corner rounded over a
/// band. Cloth meeting anything — the deck, a wire, a spar — flattens into it over a
/// hand's breadth rather than hinging against it, and a displacement with a kink in it
/// draws that kink into the sail.
fn soft_push(depth: f32, band: f32) -> f32 {
    let x = depth / band;
    if x > 8.0 {
        depth
    } else if x < -8.0 {
        0.0
    } else {
        band * (1.0 + x.exp()).ln()
    }
}

fn clear_of_mast(point: Vec3) -> Vec3 {
    let radial = Vec2::new(point.x, point.z - MAST_FOOT_Z);
    let d = radial.length();
    if point.y < DECK || point.y > crate::rig::MAST_HEAD || d >= MAST_CLEARANCE + 0.5 {
        return point;
    }
    // Dead on the axis: nothing to push along, so nudge it forward out of the spar.
    let out = if d < 1e-4 {
        Vec2::new(0.0, -1.0)
    } else {
        radial / d
    };
    // The same rounding again: pushed to the clearance radius when it is inside, left
    // alone when it is well outside, and eased between the two instead of snapping to
    // the circle at a point.
    let eased = MAST_CLEARANCE + soft_push(d - MAST_CLEARANCE, 0.12);
    Vec3::new(out.x * eased, point.y, MAST_FOOT_Z + out.y * eased)
}



/// A sail is a piece of cloth and cloth does not stretch.
///
/// This replaced the thing that was wrong with every headsail in the file. The clew was
/// being placed on a *sphere* about the tack — foot length only — and then shoved to
/// leeward by whatever the sheet allowed, which let it fly three and a half metres off
/// the centreline on a boat with a metre and three quarters of half-beam. Reported from
/// the deck exactly that way: with the car right forward the clew always ends up outside
/// the boat, and the foot has a fixed length.
///
/// It does, and so does the leech, and that is the constraint that was missing. Two
/// fixed lengths from two fixed corners put the clew on a *circle* — the intersection of
/// the foot's sphere about the tack and the leech's sphere about the head — and the only
/// freedom left is where on that ring it sits. It cannot leave the boat because the
/// cloth will not let it.
///
/// What makes the ring move is the belly. The lengths that are fixed are measured along
/// the cloth, and a sail with camber in it takes up some of that length in the curve, so
/// the straight-line distances shrink. Measured on this rig: flattened, the ring has a
/// radius of 2.79 m and the clew sits 2.39 m above the water with the sheet in; with the
/// sail deep the ring shrinks to 1.80 m and lifts, and the clew rises to 3.08. Which is
/// the other half of what was reported — the car forward makes the sail deep, and a deep
/// sail carries its clew higher and further inboard.
pub struct Cloth {
    /// Along the foot, tack to clew.
    pub foot: f32,
    /// Along the leech, head to clew.
    pub leech: f32,
}

impl Cloth {
    /// The dimensions a sailmaker would cut, from the shape the sail is designed to.
    ///
    /// The leech is not a free number: given the luff and the foot it falls out of where
    /// the clew is meant to sit, and sitting it a little above the tack is what makes a
    /// headsail's clew ride where a headsail's clew rides rather than on the deck.
    /// A sail cut from the rig's own proportions rather than from a designed clew.
    ///
    /// For the sails set flying this is the only way to get it right, and the design-
    /// clew method was putting them badly wrong: a leech worked out from a nominal clew
    /// position came out nearly as long as the luff, which shrinks the ring the clew
    /// rides until the corner cannot get aft of its own tack. Measured, the gennaker's
    /// clew was landing two metres abeam the sprit with the sail a narrow deep sliver —
    /// every bit of belly the numbers promised and nowhere for it to be.
    ///
    /// A sailmaker does not work that way round. An asymmetric is cut to proportions of
    /// the hoist: leech about nine tenths of the luff, foot a little over half. Give it
    /// those and the clew flies where a gennaker's flies — four to five metres off the
    /// boat, three to five up, and well aft of the tack.
    pub fn flying(tack: Vec3, head: Vec3, foot: f32, leech_of_luff: f32) -> Self {
        Self {
            foot,
            leech: (head - tack).length() * leech_of_luff,
        }
    }

    pub fn cut(tack: Vec3, head: Vec3, foot: f32, rise: f32) -> Self {
        let aft = Vec3::new(0.0, rise, 1.0).normalize();
        let design = tack + aft * foot;
        Self {
            foot,
            leech: (head - design).length(),
        }
    }
}

/// The same shape, with the foot corrected so the sail comes out the size it is.
///
/// The foot is worked out from the area as though the clew were free to go wherever the
/// arithmetic puts it, and then the cloth's ring pulls it somewhere else — closer in,
/// because the ring is the intersection of two spheres and the bearing the trim asks
/// for is rarely where the design clew sat. The drawn sail is then smaller than the sail
/// the physics is pulling with: measured by the rig report, the genoa was coming out at
/// three quarters of its own area, an LP of 82 percent of J where the configuration says
/// 129.
///
/// One Newton step closes it. Solve, measure the perpendicular from the clew to the
/// luff, scale the foot by how far short it fell, solve again. Twice the work of a
/// single solve, which is nothing, and the two descriptions agree to within a percent.
fn fitted_foot(tack: Vec3, head: Vec3, cloth: &Cloth, belly: f32, chord: f32, block: Vec3, want: f32) -> f32 {
    let clew = clew_on_cloth(tack, head, cloth, belly, chord, block);
    let luff_dir = (head - tack).normalize_or_zero();
    let to_clew = clew - tack;
    let lp = (to_clew - luff_dir * to_clew.dot(luff_dir)).length();
    if lp < 0.05 {
        return cloth.foot;
    }
    (cloth.foot * (want / lp)).clamp(cloth.foot * 0.5, cloth.foot * 2.5)
}

/// Where on that ring the clew has been pulled to.
///
/// Two things decide it and the first attempt used only one. The force model has already
/// said what the sheet means — a chord angle — so the clew wants the point on the ring
/// whose bearing from the tack matches it. But a bearing is a direction on the water and
/// the ring stands up out of it: on a gennaker the ring is eight metres across, and
/// every height from the deck to the spreaders has a point on it at the right bearing.
/// Matching the bearing alone left the clew to pick one at random, and it picked high —
/// the sail came out as a tall sliver with its clew up by the masthead.
///
/// What settles the height is the thing that is actually pulling: the sheet, which runs
/// to a block on the deck. Of the points at the right bearing the clew takes the one
/// nearest that block, because that is the direction the sheet hauls it. Which is also
/// why a sail set flying stands up tall when it is eased and flattens down when it is
/// sheeted hard, without anybody having to say so.
pub fn clew_on_cloth(
    tack: Vec3,
    head: Vec3,
    cloth: &Cloth,
    belly: f32,
    chord: f32,
    block: Vec3,
) -> Vec3 {
    // Camber takes up cloth, so the straight-line lengths shrink.
    let b = belly.clamp(0.0, 1.0);
    let foot = cloth.foot * (1.0 - 0.11 * b);
    let leech = cloth.leech * (1.0 - 0.05 * b);

    let luff_v = head - tack;
    let luff = luff_v.length().max(0.1);
    let axis = luff_v / luff;
    let along = (foot * foot - leech * leech + luff * luff) / (2.0 * luff);
    let radius = (foot * foot - along * along).max(0.0).sqrt();
    let centre = tack + axis * along;
    if radius < 1e-3 {
        return centre;
    }
    // A frame on the ring: aft, and athwartships.
    let aft = Vec3::Z;
    let e1 = (aft - axis * aft.dot(axis)).normalize_or_zero();
    let e2 = axis.cross(e1).normalize_or_zero();
    let e2 = if e2.x * chord < 0.0 { -e2 } else { e2 };

    // Walk the ring and take the point whose bearing from the tack is closest to the
    // chord angle the sheet has set. Sampling rather than solving: the ring is a curve in
    // three dimensions and the bearing along it is not monotonic once the luff is raked.
    let want = chord.abs();
    let mut best = centre + e1 * radius;
    let mut best_cost = f32::MAX;
    // The whole ring, not half of it: the clew of an eased sail can be forward of
    // abeam, and on a gennaker tacked out on the sprit it usually is.
    for i in 0..144 {
        let phi = std::f32::consts::TAU * i as f32 / 144.0;
        let p = centre + e1 * (radius * phi.cos()) + e2 * (radius * phi.sin());
        let from_tack = p - tack;
        // The full half-turn, not a quadrant: `max(z, 0.01)` folded everything forward
        // of the tack onto ninety degrees, and the clew of an eased sail is often
        // forward of its tack — on a gennaker set from the sprit, usually.
        let bearing = from_tack.x.abs().atan2(from_tack.z);
        // The bearing is what the trim says, so it dominates. What settles the rest is
        // which way the wind is pushing, and it is pushing *out*: a filling sail carries
        // its clew away from the boat, and the sheet only says how far. Choosing the
        // point nearest the sheet block instead — which is what this did — walked the
        // corner in towards the centreline as the sail filled, which is the one
        // direction it never goes.
        let cost = 20.0 * (bearing - want).powi(2) - 0.3 * p.x.abs() / cloth.foot.max(0.5)
            + 0.1 * (p - block).length() / cloth.foot.max(0.5);
        if cost < best_cost {
            best_cost = cost;
            best = p;
        }
    }
    best
}


/// A sail is three corners and the surface between them.
///
/// The first version described it as a luff plus a *horizontal* chord, which is fine
/// for a mainsail — its foot lies along a level boom — and simply wrong for a
/// headsail, whose foot is a free curve rising from the tack to a clew well above the
/// deck. The result was a genoa with its clew glued to the deck, dragging along it.
/// Three corners is both more correct and less code.
pub struct SailGeometry {
    pub tack: Vec3,
    pub head: Vec3,
    pub clew: Vec3,
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

/// How the cloth is behaving this frame, as opposed to where its corners are.
struct SailTrim<'a> {
    /// Which way is leeward: the only side a sail can belly towards.
    lee: f32,
    twist: f32,
    luffing: f32,
    draft: f32,
    /// Seconds, for the flogging.
    now: f32,
    /// Standing rigging the cloth is not allowed through, as a polyline.
    /// How far the foot hangs below the straight line from tack to clew, metres.
    foot_sag: f32,
    /// Where the deepest point of the section is, as a fraction of the chord from the
    /// luff.
    draft_at: f32,
    shrouds: &'a [Vec3],
    /// Whether this sail has to keep clear of the mast. The main does not: its luff
    /// is bent on to it.
    clear_mast: bool,
}

fn build_sail(mesh: &mut Mesh, g: &SailGeometry, trim: &SailTrim) {
    let SailTrim {
        lee,
        twist,
        luffing,
        draft,
        now,
        foot_sag,
        draft_at,
        shrouds,
        clear_mast,
    } = *trim;
    let mut positions = Vec::with_capacity(SAIL_U * SAIL_V);
    let mut uvs = Vec::with_capacity(SAIL_U * SAIL_V);

    let reference = sail_core::sail::CAMBER_REFERENCE as f32;
    // A sail cannot be flat and a luffing sail is not a flat sail.
    //
    // Sails are cut with their shape built in: broadseam in the panels, round in the
    // luff, more cloth round the edges than a plane between the corners would need.
    // That is why you cannot spread one out on a lawn — it stands up in a curve, and it
    // keeps that curve whatever is being done to it. The drawing had the depth collapse
    // to nothing as the sail luffed, which drew a flat sheet of cloth: the one shape a
    // sail physically cannot take.
    //
    // A luffing sail loses its *working* shape, not its built-in one. It goes soft, the
    // draft wanders aft and the whole thing shakes — and the shaking is already drawn,
    // on top of this.
    const CUT_IN: f32 = 0.06;
    let draft = draft.max(CUT_IN) * (1.0 - 0.45 * luffing) + CUT_IN * 0.45 * luffing;
    let deviation = draft - reference;

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
        // this upright means the horizontal perpendicular — pointed to leeward,
        // because that is the only side a sail can belly towards.
        //
        // Which side that is comes in as an argument now. It used to be inferred from
        // the sign of the chord's athwartships component, which was inverted on both
        // tacks and would have been undefined anyway with the boom amidships: the
        // chord lies along the centreline there and has no side to read.
        let mut normal = Vec3::Y.cross(span.normalize()).normalize_or_zero();
        if normal.x.abs() < 1e-4 {
            // Boom squared off dead across the boat: the chord has no athwartships
            // component left to read a side from, and the sail's surface is lying
            // across the hull. Its leeward is then forward — which is exactly right
            // for a main squared away on a run, and is the only answer that joins up
            // continuously with the angles either side of it.
            normal = Vec3::NEG_Z;
        } else if normal.x * lee < 0.0 {
            normal = -normal;
        }
        // The outhaul tensions the foot, so its share of the depth sits low down.
        let foot_weight = (1.0 - u).powf(1.6);
        let local_camber = reference + deviation * (0.35 + 1.65 * foot_weight);
        // And an eased foot does not merely get deeper, it *hangs*. The foot is the one
        // edge an outhaul has its hands on: hauled, it is a straight tight line from
        // tack to clew; eased, the cloth between them bags down under its own weight.
        // Which is the half of the control a sailor looks at, and the sail had only the
        // other half — it changed shape sideways and kept a ruler-straight bottom edge
        // whatever the line was doing.
        let hang = foot_sag * (1.0 - u).powf(2.2);

        for vi in 0..SAIL_V {
            let v = vi as f32 / (SAIL_V - 1) as f32;
            // The section, with its deepest point where the sailmaker put it rather
            // than in the middle of the chord. The chord is warped so that `draft_at`
            // lands at the top of the curve: steeper entry ahead of it, long flat run
            // behind — which is the shape of every working sail ever cut, and the
            // difference between a sail and a sack.
            let d = draft_at.clamp(0.12, 0.88);
            let warped = if v < d {
                v / (2.0 * d)
            } else {
                0.5 + (v - d) / (2.0 * (1.0 - d))
            };
            let belly = (std::f32::consts::PI * warped).sin() * local_camber * chord_length;
            let droop = (std::f32::consts::PI * warped).sin() * hang;
            // Creases, radiating from the clew.
            //
            // Not decoration and not something a tiling texture can do, because they
            // belong to the sail rather than to the cloth: a corner takes the load of
            // everything attached to it, and the cloth pulls into lines running away
            // from it. They are also what a trimmer reads — the ones from the clew
            // tell you about the foot — so a sail without them is missing an
            // instrument, not just a detail.
            //
            // Strongest at the clew, fading out over about a third of the sail, and
            // vanishing when the sail is luffing, because an unloaded sail has no
            // load lines in it.
            let from_clew = ((1.0 - v) * (1.0 - v) + u * u).sqrt();
            let pull = (1.0 - from_clew * 1.8).max(0.0) * (1.0 - luffing);
            let fan = (u.atan2(1.0 - v + 1e-3) * 7.0).sin();
            let crease = pull * pull * fan * 0.016 * chord_length;
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
            let point = luff.lerp(leech, v) + normal * (belly + flap + crease) - Vec3::Y * droop;
            let point = constrained(point, normal, shrouds, clear_mast);
            positions.push(point);
            // Metres, not a normalised zero to one. A panel of sailcloth is a metre
            // wide whatever sail it is in, so the seams have to be laid out in the
            // sail's own size — otherwise a reefed main grows its panels as it
            // shrinks, which is the one thing cloth cannot do.
            uvs.push(Vec2::new(
                (point - luff).length() / crate::sail_texture::PANEL_METRES,
                (point.y - g.tack.y) / crate::sail_texture::PANEL_METRES,
            ));
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.compute_normals();
}
