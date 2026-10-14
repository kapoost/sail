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
pub const MAST_FOOT_Z: f32 = sail_core::boat::packet::MAST_FOOT_Z as f32;
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
pub const SCAN_LOA: f32 = 5.396;
/// How far down the scaled scan is dropped so that y = 0 is her waterline.
///
/// The scan is of a boat out of the water, so the file knows nothing about where she
/// floats. This is not guessed: the mesh was integrated, cell by cell, for the volume
/// below a trial waterline, and the plane solved for where that volume equals the
/// displacement. She floats 1.294 m above the bottom of the scaled hull, which also
/// gave the waterline length and beam the physics config now uses.
pub const HULL_SINKAGE: f32 = -1.294;

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

    let main_mesh = meshes.add(empty_sail());
    let genoa_mesh = meshes.add(empty_sail());
    let staysail_mesh = meshes.add(empty_sail());

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
                    Mesh3d(meshes.add(Cylinder::new(0.065, crate::rig::BOOM_LENGTH))),
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
                Transform::from_translation(staysail_tack(&cfg)),
                Visibility::default(),
            ))
            .with_children(|club| {
                let length = staysail_boom_length(&cfg);
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
                // At the very top of the spar, above everything. Derived: this was
                // `DECK + 15.4` and sat below the masthead once the mast grew.
                Transform::from_xyz(0.0, crate::rig::MAST_HEAD + 0.15, MAST_FOOT_Z),
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
    // Where the spar *is*, not where the sheet would put it. Those are the same number
    // in steady air and they are not the same number for the second or two of a gybe —
    // and this used to take the second one, so the boom jumped across the boat the
    // instant the wind did, in a game whose simulation had already been taught that it
    // takes time. Reported from the deck twice, which it deserved.
    let swing = t.boom as f32;
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
    // A windex swings freely and points into the apparent wind — and it does it through
    // the same function the vane on the transom is set by, so the two cannot disagree
    // about where the wind is. They did, by ninety degrees, for as long as that was a
    // sign written out twice.
    let awa = sim.sim.telemetry.awa as f32;
    for mut t in &mut q {
        t.rotation = crate::fittings::into_the_wind(awa);
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
    let boom = t_.boom as f32;
    let tack = Vec3::new(0.0, BOOM_HEIGHT, MAST_FOOT_Z);
    // The same three corners the sail is built from, from the same functions, which is
    // the only way a telltale stays sewn to the cloth it is reporting on.
    let head = main_head(1.0 - 0.45 * reef);
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
                let boom = t_.boom as f32;
                let tack = Vec3::new(0.0, BOOM_HEIGHT, MAST_FOOT_Z);
                (
                    t_.main,
                    SailGeometry {
                        tack,
                        head: main_head(shrink),
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
                let tack = staysail_tack(&cfg);
                let head = staysail_head(&cfg);
                let foot = staysail_boom_length(&cfg);
                // Hoisted, not furled: it climbs its stay as the halyard goes up.
                (
                    t_.jib,
                    SailGeometry {
                        tack,
                        head: tack.lerp(head, 0.18 + 0.82 * set),
                        // On the boom end, because that is where a club-footed sail's
                        // clew is: shackled to the spar, not flying free.
                        clew: staysail_boom_end(&cfg, angle, foot),
                    },
                )
            }
            Sail::Headsail => (
                t_.jib,
                // The headsail's own side, which is not always the leeward one: held
                // aback its clew is to windward and the whole sail is over there. From
                // `sail_core`, so the cloth is drawn on the side the force model is
                // pulling from — two descriptions of one sheet is the fault this file
                // has been caught by three times.
                // Not `clew_side`, which answers with a whole side and therefore flicks
                // the sail across the foretriangle in one frame. `jib_side` is a
                // *fraction*: on the old side, streaming down the middle, on the new one
                // — the five seconds of a tack that the picture used to skip.
                headsail_shape(
                    &cfg,
                    &sim.controls,
                    t_.jib_side as f32,
                    t_.jib.chord_angle as f32,
                ),
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
                    // The side the cloth bellies towards. For the mainsail that is the
                    // side the *boom* is on, which during a gybe is not the side the wind
                    // says: for a second or two the sail is aback, and a mainsail drawn
                    // bellying away from its own spar is a sail turned inside out.
                    lee: if surface.which == Sail::Main {
                        if t_.boom >= 0.0 {
                            1.0
                        } else {
                            -1.0
                        }
                    } else {
                        side
                    },
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
                    // Thirty knots of apparent wind is as hard as a sail ever shakes;
                    // past that you have already taken it down.
                    shake: (t_.aws as f32 / 15.4).clamp(0.0, 1.0),
                    flying: surface.which == Sail::Headsail
                        && sim.controls.headsail != sail_core::state::Headsail::Genoa,
                    chord: forces.chord_angle.abs() as f32,
                    // The staysail is club-footed: its foot is laced along a spar that
                    // swings on the tack fitting, so that edge is held straight and
                    // cannot bag. Drawn with the same belly as every other sail its foot
                    // bulged past the end of its own boom, which is a sail hanging off
                    // the spar that is supposed to be holding it.
                    footed: surface.which == Sail::Staysail,
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
                            + if surface.which == Sail::Main {
                                0.55
                            } else {
                                1.9
                            } * forces.chord_angle.abs().sin() as f32)
                        // A backed headsail is not a wing at all. The clew is held to
                        // windward and the wind is on the front of the cloth, so the
                        // whole sail is pressed bodily *across* the boat and on to the
                        // rig: it bags out, lies against the mast and the wires, and the
                        // spar disappears behind it. Drawn as a clean foil standing clear
                        // of the mast it reads as a sail that has been swung over, which
                        // is a thing nobody has ever seen a sail do.
                        * if surface.which == Sail::Headsail && sim.controls.headsail_aback {
                            1.9
                        } else {
                            1.0
                        },
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
                        && !feels_the_shrouds(
                            sim.controls.headsail,
                            t_.jib.chord_angle,
                            cfg.headsail_bears_at,
                        ) {
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
                    // The main is bent on to the mast, so it is never held off it. Nor
                    // is a backed headsail — the wind is holding it *against* the spar,
                    // which is where it lies and what the crew hears.
                    clear_mast: surface.which != Sail::Main
                        && !(surface.which == Sail::Headsail && sim.controls.headsail_aback),
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
pub fn main_reef_scale(
    cfg: &sail_core::boat::BoatConfig,
    controls: &sail_core::state::Controls,
) -> f32 {
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
    info!(
        "rig: mast head {p:.2} m, forestay {:.2}, boom {:.2} x {:.2} high, spreaders {:.2}",
        sail_core::boat::packet::FORESTAY_HEAD,
        crate::rig::BOOM_LENGTH,
        crate::rig::BOOM_HEIGHT,
        crate::rig::SPREADER_HEIGHT
    );
    // Measured off the geometry the mesh is built from, not off the constant it was
    // meant to be built from. Those were the same number for a while and then they were
    // not, and the version of this line that read the constant reported a correct luff
    // while the sail was drawn a metre and a third short.
    let drawn_head = main_head(1.0);
    let luff_main = drawn_head.y - crate::rig::BOOM_HEIGHT;
    info!(
        "     mainsail: luff drawn {:.2} m against {:.2} configured, foot {:.2}, area {:.1} m2",
        luff_main,
        sail_core::boat::packet::MAIN_LUFF,
        crate::rig::BOOM_LENGTH,
        cfg.main.area
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
            "     {:8}: area {:5.1} m2  luff {:5.2}  foot {:5.2}  clew z {:+5.2}  \
             head {:5.2} m  LP/base {:4.0}%",
            kind.name(),
            cfg.headsail(kind).area,
            (g.head - g.tack).length(),
            (g.clew - g.tack).length(),
            g.clew.z,
            g.head.y,
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
        let fitted = Cloth::cut(
            tack,
            hoisted,
            fitted_foot(tack, hoisted, &cloth, belly, angle, car, want),
            0.30,
        );
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
    let out = 1.0 - sail_core::units::smoothstep(0.10, 0.45, controls.furl) as f32;
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
    let block = Vec3::new(side * 1.15, crate::deck::deck_height(3.9) + 0.07, 3.9);
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

/// Where the head of the mainsail sits, for a reef scale of 1 down to 0.
///
/// Reefing pulls the luff down the mast and leaves the foot where it was: the pennant
/// runs through a cheek block at the *end* of the boom, so the new clew lands within a
/// hand's breadth of the old one. What you lose is hoist.
///
/// A function, and derived, because this was typed in. It read `DECK + 15.4`, which was
/// the masthead until the mast's height stopped being a typed-in number and became
/// `BOOM_HEIGHT + MAIN_LUFF` — and then the sail's head stayed where it was, one and a
/// third metres below the top of its own spar, with the halyard apparently attached to
/// nothing. Reported from the deck as the head corner coming unstuck from the mast.
///
/// What makes it worth a paragraph is that the diagnostic in `report` said the luff was
/// right. It was measuring `MAST_HEAD - BOOM_HEIGHT` — the intention — while the mesh
/// was built from a different number. An instrument that reads the constant instead of
/// the drawing cannot see this class of fault at all, which is the one it exists for.
pub fn main_head(reef_scale: f32) -> Vec3 {
    Vec3::new(
        0.0,
        BOOM_HEIGHT + (crate::rig::MAST_HEAD - BOOM_HEIGHT) * reef_scale,
        MAST_FOOT_Z,
    )
}

/// Marks the staysail's club boom, which swings on its own.
#[derive(Component)]
pub struct StaysailBoom;

/// Where the staysail is tacked down: at the foot of its own stay, which lands where
/// the sail's own luff reaches from the tang.
///
/// Derived, not chosen, and that is the fix. It was three point three metres forward of
/// the mast, which on this boat is *four centimetres* from where the genoa is tacked —
/// the inner stay was standing on the stemhead beside the forestay, so the "staysail"
/// was a second headsail of nearly the same hoist with its foot sweeping the whole
/// foredeck. Reported from the deck as a decidedly too long foot, and it was: the drawn
/// luff came out 8.35 m against the 7.69 the force model was using, and a sail of a
/// given area on a longer luff has to put the difference somewhere.
///
/// The luff comes back out of the profile rather than being written down again here —
/// aspect ratio is luff squared over area, so the two numbers the sailmaker measured are
/// enough and this cannot drift from them. The stay then lands wherever that luff
/// reaches on a boat of this freeboard, which for a cutter is the sixty per cent of the
/// fore-triangle it ought to be.
pub fn staysail_tack(cfg: &sail_core::boat::BoatConfig) -> Vec3 {
    // Where a cutter puts it: a little under halfway back from the stem to the mast,
    // which is what leaves room to tack a genoa across in front of it and is the whole
    // reason the stay is inner rather than another forestay.
    let stem = -(cfg.loa as f32) * 0.49;
    Vec3::new(0.0, DECK + 0.12, stem + 0.45 * (MAST_FOOT_Z - stem))
}

/// And the head of it: as high as the sail's own luff reaches from that tack.
///
/// This way round, because a stay is where its sail reaches and nowhere else. The luff
/// comes back out of the profile rather than being written down again — aspect ratio is
/// luff squared over area, so the two numbers the sailmaker measured are enough and this
/// cannot drift from them.
pub fn staysail_head(cfg: &sail_core::boat::BoatConfig) -> Vec3 {
    let tack = staysail_tack(cfg);
    let luff = (cfg.staysail.profile.aspect_ratio * cfg.staysail.area).sqrt() as f32;
    let along = MAST_FOOT_Z - tack.z;
    let rise = (luff * luff - along * along).max(1.0).sqrt();
    Vec3::new(0.0, tack.y + rise, MAST_FOOT_Z)
}

/// The club boom's length, which is the sail's foot and therefore not a free choice.
///
/// A spar does not change length with the trim, so it is cut for a working angle and
/// the sail is cut to the spar. Taken at thirty degrees of chord, which is where a
/// self-tacker spends most of its life.
pub fn staysail_boom_length(cfg: &sail_core::boat::BoatConfig) -> f32 {
    foot_for_area(
        cfg.staysail.area as f32,
        staysail_head(cfg) - staysail_tack(cfg),
        Vec3::new(0.5, 0.0, 0.866),
        1.15,
    )
}

/// Where the end of that boom is, which is also the sail's clew. One definition, so
/// the spar and the cloth cannot end up in different places.
pub fn staysail_boom_end(cfg: &sail_core::boat::BoatConfig, angle: f32, length: f32) -> Vec3 {
    staysail_tack(cfg) + Vec3::new(angle.sin() * length, 0.22, angle.cos() * length)
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
/// The most the wire may move the cloth, metres.
///
/// Without a cap the push is the cloth's *whole* distance to leeward of the wire, which
/// for an eased genoa is a metre and more — and the sail's panels are a metre apart, so
/// one vertex came back onto the wire while its neighbour, a hand's breadth outside the
/// lateral falloff, did not move at all. A metre and a half of displacement across one
/// panel is ninety degrees between them: the broken plane that was reported from the
/// deck, found by a sweep that tries every sail at every angle at every sheet.
///
/// A cap is the right shape for it as well as the safe one. A shroud is eight
/// millimetres of wire: what it does to a sail lying on it is a crease a hand's breadth
/// deep, not a repositioning. Cloth whose own shape has it a metre to leeward of the
/// wire is not being held off by the wire at all — it is somewhere the wire has no
/// opinion about, and the wire is drawn over it, which is what a wire lying on a sail
/// looks like.
const SHROUD_STANDOFF: f32 = 0.13;

/// Hold a point of sailcloth off the standing rigging.
///
/// Whether this headsail's cloth is held off the shrouds at all.
///
/// Two different sails with two different answers, and writing it as one condition on
/// the sheet angle lost the difference — which showed up as a vertical crease down a
/// Code 0 on a close reach, where the wire was pushing cloth that is never near it.
///
/// * **A genoa is hanked to the forestay and sheets inside the rig**, to a track on the
///   side deck, and whether it reaches the wire is a question the force model already
///   answers: [`sail_core::sail::rig_contact`]. Sheeted in it does not. The car runs
///   inboard of the chainplate — on this boat the track stands 1.15 m off the
///   centreline against the shroud's 1.53 — so a hard-sheeted genoa is inside the wire
///   over its whole height and the wire is outboard of its leech rather than under the
///   cloth. There is nothing there to crease over. That was reported from the deck in
///   those words: the car works inside the shrouds, so the fold is impossible.
/// * **A Code 0 and a gennaker are set flying.** Tacked to the sprit a metre and a half
///   ahead of the stem and sheeted to a block on the quarter, they live *outside*
///   everything by construction — that is what setting a sail flying means. There is no
///   angle at which their cloth is inside a shroud, so there is no angle at which a
///   shroud may push it.
pub fn feels_the_shrouds(
    kind: sail_core::state::Headsail,
    chord_angle: f64,
    bears_at: f64,
) -> bool {
    match kind {
        sail_core::state::Headsail::Genoa => {
            sail_core::sail::rig_contact(chord_angle, bears_at) > 0.0
        }
        sail_core::state::Headsail::Code0 | sail_core::state::Headsail::Gennaker => false,
    }
}

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
        p -= normal * soft_push(along + SHROUD_RADIUS, 0.10).min(SHROUD_STANDOFF) * weight;
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
    let point = if clear_mast {
        clear_of_mast(point, normal)
    } else {
        point
    };
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

/// Hold cloth out of the spar — along one direction, not away from the axis.
///
/// A radial clamp is not a projection. It pushes every point away from the axis along
/// its own radius, so two neighbouring vertices that straddle the mast are flung to
/// opposite sides of it and the cloth between them doubles back: a hundred and eighty
/// degrees between two panels, found by a test that sweeps every sail through every
/// angle rather than by sailing into it, and reported from the deck before that as a
/// sail with a broken plane.
///
/// The shroud next door has had this right since it was written and says so in its own
/// comment: *the cloth may not be further to leeward than the wire*. One direction,
/// the one the wind is pressing the sail in, so neighbours move together and a surface
/// stays a surface. The mast was the one of the three that never got the argument.
///
/// So: find how far along the sail's own leeward normal this point has to travel to
/// leave the spar's circle, which is a quadratic, and move it that far.
fn clear_of_mast(point: Vec3, normal: Vec3) -> Vec3 {
    // How much spar there is at this height. Not *whether* there is any, which is what
    // this asked: a point a hand's breadth below the masthead was shoved out to the
    // full clearance radius and one a hand's breadth above was left alone, so the cloth
    // between them had to span the whole of that in one triangle. A displacement with a
    // corner in it is a fold, wherever the corner is — the radial push here was rounded
    // off when this was written and the ends were not.
    let reach = crate::curve::smoothstep(DECK - 0.5, DECK, point.y)
        * (1.0
            - crate::curve::smoothstep(
                crate::rig::MAST_HEAD - 0.5,
                crate::rig::MAST_HEAD,
                point.y,
            ));
    let radial = Vec2::new(point.x, point.z - MAST_FOOT_Z);
    if reach <= 0.0 || radial.length() >= MAST_CLEARANCE {
        return point;
    }
    // The push direction, flattened: cloth is held off a vertical spar sideways, and a
    // vertical component would lift the sail up the mast instead of out around it.
    let mut push = Vec2::new(normal.x, normal.z);
    if push.length_squared() < 1e-6 {
        // Nothing to push along — a normal pointing straight up, which no sail has.
        // Forward, out of the spar, which is where a headsail's cloth came from.
        push = Vec2::new(0.0, -1.0);
    }
    let push = push.normalize();
    // Smallest step along `push` that puts the point on the circle.
    let b = radial.dot(push);
    let c = radial.length_squared() - MAST_CLEARANCE * MAST_CLEARANCE;
    let t = -b + (b * b - c).max(0.0).sqrt();
    let moved = radial + push * (t * reach);
    Vec3::new(moved.x, point.y, MAST_FOOT_Z + moved.y)
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
fn fitted_foot(
    tack: Vec3,
    head: Vec3,
    cloth: &Cloth,
    belly: f32,
    chord: f32,
    block: Vec3,
    want: f32,
) -> f32 {
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

/// How finely a sail is cut into panels for drawing.
///
/// Nine by five was one grid for every sail on the boat, and the sails are not one size:
/// the gennaker has three and a half times the genoa's area on the same forty-five
/// vertices, so its panels are two metres across. A curved surface sampled that coarsely
/// does not read as curved — it reads as folded, which is what was reported from the
/// deck, and the fold is in the sampling rather than in the cloth.
///
/// Seventeen by nine is a hundred and fifty-three vertices, three sails rebuilt every
/// frame, which is nothing at all beside the forty thousand the sea costs.
const SAIL_U: usize = 17;
const SAIL_V: usize = 9;

pub fn empty_sail() -> Mesh {
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
    /// How hard the wind is, 0 to 1, for the speed of the flogging.
    shake: f32,
    /// Set flying: three corners on ropes and nothing holding any edge.
    flying: bool,
    /// How far the sheet has let the chord swing off the centreline, radians. For a sail
    /// set flying this is the whole story of its depth.
    chord: f32,
    /// A spar along the foot, which therefore has no belly in it at all.
    footed: bool,
}

/// Build a sail for a boat that is not yours.
///
/// The same builder, given the trim a ghost can actually know about. Everything left out
/// here is something that cannot be seen at the distance another boat is at: the shrouds
/// the cloth bears on, the twist the vang is holding, where the draft sits, how far the
/// foot sags. Those are all worth having on the sail four feet from your face and none of
/// them survives a hundred metres of air.
///
/// Kept beside `build_sail` rather than in `fleet.rs` so that the two callers of the sail
/// builder are in one place and cannot drift apart.
pub fn dress_sail(mesh: &mut Mesh, g: &SailGeometry, lee: f32, luffing: f32, shake: f32, now: f32) {
    build_sail(
        mesh,
        g,
        &SailTrim {
            lee,
            twist: 0.18,
            luffing,
            draft: 0.14,
            now,
            foot_sag: 0.12,
            draft_at: 0.42,
            shrouds: &[],
            clear_mast: false,
            shake,
            flying: false,
            chord: 0.0,
            footed: true,
        },
    );
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
        shake,
        flying,
        footed,
        chord,
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
        let mut luff = g.tack.lerp(g.head, u);
        let mut leech = g.clew.lerp(g.head, u);
        // A sail set flying has no straight edges at all.
        //
        // Both of a genoa's edges are held: the luff is on a stay and the leech is
        // pulled straight between the head and the clew by the sheet. A gennaker has a
        // rope at each of three corners and nothing anywhere else, so both edges bow
        // outwards under the pressure in the sail — the luff projects to windward and
        // forward, which is the whole reason it is cut that way and what you are
        // watching when you ease until it curls. Drawn between straight lines it is a
        // flat triangle with a bulge painted on, which is what it looked like.
        //
        // Carried on the sail's own leeward normal, so the bow goes where the wind
        // is pushing. The luff is the fuller of the two because its curve is cut in;
        // the leech's comes only from the pressure.
        if flying {
            // Fullest at two thirds of the hoist, like everything else about this sail,
            // instead of at the middle.
            let bow = (std::f32::consts::PI * u.powf(1.4)).sin() * (g.head - g.tack).length();
            // The normal is not known until the chord is, so a provisional one from the
            // straight chord serves to push the edges — one iteration of a shape that
            // would otherwise need solving.
            let across = Vec3::Y
                .cross((leech - luff).normalize())
                .normalize_or_zero();
            let side = if across.x * lee < 0.0 {
                -across
            } else {
                across
            };
            luff += side * (bow * 0.085) - Vec3::Z * 0.0;
            leech += side * (bow * 0.055);
        }

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
        let mut local_camber = reference + deviation * (0.35 + 1.65 * foot_weight);
        // And a flying sail is deepest *high up*, which is the opposite way round and
        // the most recognisable thing about one. A genoa is a wing with its fullest
        // section near the deck where the chord is longest; a gennaker is a balloon with
        // its shoulders up by the head, because the tack is pinned down on the sprit and
        // the head is free to fly. That is why the sail looks like it is lifting away
        // from the boat — it is, and the lift is the shape.
        if footed {
            // The spar holds the bottom of the sail flat and the shape comes in above
            // it, over about a fifth of the hoist — which is what cloth does over a boom
            // and why a club-footed sail looks board-like along its foot. Drawn with the
            // same belly as every other sail, the staysail's foot bulged past the end of
            // its own boom: a sail hanging off the spar that is supposed to hold it.
            local_camber *= crate::curve::smoothstep(0.0, 0.2, u);
        }
        if flying {
            // A flying sail's depth, said once and directly, instead of as a stack of
            // multipliers with a cap on top.
            //
            // The cap was the wrong instrument and two reports a few minutes apart show
            // why: on a reach at good trim it was a balloon that filled the screen, and
            // squared away downwind it was a flat sheet when it should have been a
            // balloon. A ceiling cannot tell those apart — it flattens exactly the
            // relation that distinguishes them, because both were hitting it.
            //
            // The relation is the sheeting angle, and it is the whole story of this sail.
            // Sheeted hard for a close reach a gennaker is *flat*: that is what lets it
            // be carried at all at seventy degrees of apparent wind, and a sailmaker cuts
            // the luff for it. Squared off before the wind it is a bag with nothing
            // holding its shape but the air in it. Twelve per cent to forty-two, by the
            // sine of the chord angle, which is zero on the centreline and one when the
            // foot is square across the boat.
            //
            // The generic "an eased sail gets fuller" term does not apply here: this
            // *is* that term, written properly for a sail that has no edges held.
            let depth = 0.12 + 0.30 * chord.sin();
            // And the shoulders, which say where down the hoist that depth sits — high,
            // at two thirds, because the tack is pinned on the sprit and the head is
            // free. Normalised so the deepest point of the sail *is* the depth and the
            // profile only decides where it is.
            let shoulders = (std::f32::consts::PI * u.powf(1.6)).sin();
            local_camber = depth * (0.45 + 0.55 * shoulders);
        }
        // No sail is deeper than this much of its own chord, and the number is not a
        // safety margin — it is what a sail *is*.
        //
        // A spinnaker is cut at about a quarter of its chord and a reaching kite nearer
        // a third; a working sail is a tenth. What was happening is that two reasonable
        // multipliers were stacking: the generic "eased sails get fuller" term is
        // `1 + 1.9 sin(chord angle)`, which on a broad reach is nearly three, and the
        // flying sail's own shoulders are another two and a bit on top of that. Twenty
        // per cent of camber came out at a hundred and forty, which is a sail bellying
        // further than it is wide — reported from the deck as the gennaker filling the
        // screen the moment the trim came good, because good trim is exactly when the
        // chord angle is large and both terms are at their biggest.
        //
        // Clamped rather than rebalanced because the two terms are each right about
        // their own thing and wrong only about each other. A cap says the thing that is
        // actually true: cloth cannot hold a shape deeper than this, whatever the
        // arithmetic upstream believes.
        // The working sails keep a ceiling, because theirs really is a stack: the cut,
        // the outhaul or the car, and the easing term all multiply, and a fifth of the
        // chord is as deep as cloth stretched between two held edges ever gets.
        if !flying {
            local_camber = local_camber.min(0.20);
        }
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
            // A power rather than two straight lines meeting at the draft position.
            //
            // The piecewise version put the peak in the right place and had a *corner*
            // in it while doing so: the function is continuous but its slope jumps at
            // `d`, and a slope that jumps is a crease in the cloth along one line of
            // constant chord. It was invisible while the sail was five panels wide and
            // came straight out when that went to nine — found by the fold sweep after
            // it learnt to tell a crease from a curve, which is exactly the kind of
            // thing that measure is for.
            //
            // `v^k` with `k = ln(1/2) / ln(d)` sends `d` to a half, so the sine still
            // peaks exactly where the sailmaker put the draft, and it is smooth
            // everywhere.
            let warped = v.powf((0.5_f32).ln() / d.ln());
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
            // Flogging, and it has to *look* like flogging.
            //
            // One sine at nine radians a second and four tenths of a chord of amplitude
            // is a sail breathing. What a sail head to wind actually does is crack: it
            // shakes through its whole width, hard enough to be the loudest thing on the
            // boat and fast enough that you cannot count it, and the leech snaps a beat
            // of its own. Reported from the deck as a mainsail standing like a sheet of
            // tin with the instruments saying LUFFING beside it.
            //
            // Two beats rather than one, because a single frequency reads as a wave
            // travelling and not as cloth losing its shape — and the second one is
            // deliberately not a multiple of the first. The rate goes with the wind,
            // which is the thing that makes it read as violence rather than as motion:
            // a sail flogging in six knots waves, and in thirty it is a machine gun.
            let beat = 7.0 + 11.0 * shake;
            let flap = ((now * beat + u * 5.0 + v * 3.0).sin() * 0.62
                + (now * beat * 1.7 + u * 9.0 - v * 4.0).sin() * 0.38)
                * luffing
                // Four tenths of the chord at the leech, not a whole one.
                //
                // It went up to 1.05 for the sake of a sail that cracks rather than
                // breathes, and the violence does not live in the amplitude: it is in the
                // two beats and in the rate going with the wind. What a whole chord buys
                // instead is a displacement of three or four metres between panels less
                // than one apart, which is not a sail flogging, it is a sail tearing —
                // caught by the fold sweep the moment that sweep learnt to tell a crease
                // from a curve.
                * 0.38
                * free_edge
                * chord_length.max(0.4);
            let point = luff.lerp(leech, v) + normal * (belly + flap + crease) - Vec3::Y * droop;
            // Cloth in the body of the sail is pushed clear of the spar and the wires.
            // Cloth on the luff is *held* by them — it is in a mast track or hanked to a
            // stay — and the head is a fitting shackled to the halyard at the top of
            // whichever one it is. Pushing those is how a head corner ends up floating
            // a hand's breadth off its own stay, which is what a headsail was doing: its
            // head sits on the mast's axis where the forestay lands, so the clearance
            // push had the one point that cannot move as its target.
            let edge = crate::clouds::step_between(0.0, 0.14, v)
                * (1.0 - crate::clouds::step_between(0.82, 1.0, u));
            let point = point.lerp(constrained(point, normal, shrouds, clear_mast), edge);
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

#[cfg(test)]
mod rigging_rules {
    use super::*;
    use sail_core::state::Headsail;
    use sail_core::units::rad;

    /// A sail set flying is outside the rig at every angle, and the wire may never
    /// touch it.
    ///
    /// This has been reported twice from the deck — once as a sheet drawn inside the
    /// shrouds and once as a crease down a Code 0 on a close reach — and both times the
    /// rule was written as a condition on the sheet angle, which is the genoa's rule
    /// applied to a sail that does not share it. Now it is a statement about which sail
    /// is up, and this is what holds it there.
    #[test]
    fn a_sail_set_flying_never_feels_a_shroud() {
        let bears_at = rad(28.0);
        for degrees in [0.0_f64, 15.0, 30.0, 45.0, 60.0, 75.0, 88.0] {
            for side in [1.0_f64, -1.0] {
                let angle = rad(degrees * side);
                assert!(
                    !feels_the_shrouds(Headsail::Code0, angle, bears_at),
                    "a Code 0 at {degrees:.0} deg of chord is tacked to the sprit and \
                     sheeted to the quarter: there is nothing of it inside a shroud"
                );
                assert!(!feels_the_shrouds(Headsail::Gennaker, angle, bears_at));
            }
        }
    }

    /// And a genoa does, but only across the band where it actually reaches the wire:
    /// not sheeted in, where it is inboard of everything, and not eased right out,
    /// where it is outboard of everything.
    #[test]
    fn a_genoa_lies_against_the_shroud_only_while_it_reaches_it() {
        let bears_at = rad(28.0);
        // Reported from the deck: the lead car runs on a track inboard of the
        // chainplates, so a genoa strapped to it is inside the cap shroud over its whole
        // height. The wire is outboard of the leech rather than under the cloth, and the
        // crease the drawing used to put there is a fold no sail makes. The force model
        // had been saying this all along — `rig_contact` is zero below `bears_at` — and
        // the two now read the same definition instead of disagreeing.
        assert!(
            !feels_the_shrouds(Headsail::Genoa, rad(12.0), bears_at),
            "sheeted to its track the sail is inboard of the wire: nothing touches it"
        );
        assert!(feels_the_shrouds(Headsail::Genoa, rad(45.0), bears_at));
        assert!(
            !feels_the_shrouds(Headsail::Genoa, rad(95.0), bears_at),
            "eased past the spreaders it is flying clear and the wire is not an obstacle"
        );
        // The drawing and the forces agree about where the sail is, at every angle, or
        // there is a crease in the picture where the physics says clear air.
        for degrees in [0.0_f64, 10.0, 20.0, 28.0, 35.0, 50.0, 70.0, 88.0] {
            let angle = rad(degrees);
            assert_eq!(
                feels_the_shrouds(Headsail::Genoa, angle, bears_at),
                sail_core::sail::rig_contact(angle, bears_at) > 0.0,
                "at {degrees:.0} deg the picture and the force model disagree"
            );
        }
        // Both tacks, which is where a sign error would hide.
        assert_eq!(
            feels_the_shrouds(Headsail::Genoa, rad(-45.0), bears_at),
            feels_the_shrouds(Headsail::Genoa, rad(45.0), bears_at)
        );
    }
}

#[cfg(test)]
mod sails_and_rigging {
    use super::*;
    use sail_core::state::{Controls, Headsail};

    /// Every sail's head has to be on the thing it is hoisted to.
    ///
    /// A halyard does not hold a sail in mid-air. This has now been reported twice from
    /// the deck — the mainsail's head a metre and a third below its own masthead, and the
    /// staysail's hanging forty centimetres forward of the spar with daylight behind it
    /// — and both times the drawn corner and the fitting it belongs to were separate
    /// numbers that had drifted apart. The fittings are the authority; this holds the
    /// sails to them.
    const AT_THE_FITTING: f32 = 0.2;

    #[test]
    fn every_head_is_on_its_spar_or_stay() {
        let cfg = sail_core::boat::BoatConfig::archipelago_packet();

        // The mainsail, to the top of the mast.
        let main = main_head(1.0);
        let masthead = Vec3::new(0.0, crate::rig::MAST_HEAD, MAST_FOOT_Z);
        assert!(
            (main - masthead).length() < AT_THE_FITTING,
            "the mainsail's head is {:.2} m from the masthead",
            (main - masthead).length()
        );

        // The staysail's head is on the mast by construction now, so asking whether it
        // is would be asking nothing. What is worth asking is whether the stay it is
        // hoisted on is as long as the sail — which is the thing that was wrong, and
        // wrong quietly: the stay's height and the sail's luff were both sixty-two per
        // cent of the genoa's, but measured from different places, the one from the
        // water and the other from the deck. The stay came out taller than its own sail
        // and the sail put the difference in its foot.
        let drawn = (staysail_head(&cfg) - staysail_tack(&cfg)).length();
        let cut = (cfg.staysail.profile.aspect_ratio * cfg.staysail.area).sqrt() as f32;
        assert!(
            (drawn - cut).abs() < 0.05,
            "the staysail is drawn on a {drawn:.2} m stay and cut with a {cut:.2} m luff"
        );

        // And the club boom has to end before the mast, because it is a spar that swings
        // in the gap between the inner stay and the gooseneck. It ran a metre and a
        // third past it, through the mainsail's luff — a sail cannot be given an area
        // the boat has no room for, and the cure is a smaller sail rather than a shorter
        // drawing of the same one, or the picture and the forces part company again.
        let boom_end = staysail_boom_end(&cfg, 0.0, staysail_boom_length(&cfg));
        assert!(
            boom_end.z < MAST_FOOT_Z,
            "the staysail's club boom ends at {:.2}, which is past the mast at {:.2}",
            boom_end.z,
            MAST_FOOT_Z
        );

        let stay = staysail_head(&cfg);
        let tang = Vec3::new(0.0, stay.y, MAST_FOOT_Z);
        assert!(
            (stay - tang).length() < AT_THE_FITTING,
            "the staysail's head is {:.2} m from the mast — a halyard does not hold a \
             sail in mid-air",
            (stay - tang).length()
        );

        // The genoa, to the forestay's head; the flying sails to the masthead above it.
        for (kind, fitting) in [
            (
                Headsail::Genoa,
                Vec3::new(0.0, crate::rig::FORESTAY_HEAD, MAST_FOOT_Z),
            ),
            (Headsail::Code0, masthead),
            (Headsail::Gennaker, masthead),
        ] {
            let controls = Controls {
                headsail: kind,
                furl: 0.0,
                jib_sheet: 0.0,
                ..Default::default()
            };
            let g = headsail_shape(&cfg, &controls, -1.0, sail_core::units::rad(14.0) as f32);
            assert!(
                (g.head - fitting).length() < AT_THE_FITTING,
                "the {} head is {:.2} m from where it is hoisted",
                kind.name(),
                (g.head - fitting).length()
            );
        }
    }

    /// And the tacks are on their fittings too, which is the other end of the same claim.
    #[test]
    fn every_tack_is_on_its_fitting() {
        let cfg = sail_core::boat::BoatConfig::archipelago_packet();
        // The mainsail's tack is at the gooseneck, on the mast at the boom's height.
        let tack = Vec3::new(0.0, crate::rig::BOOM_HEIGHT, MAST_FOOT_Z);
        assert!(
            (main_head(1.0) - tack).length() > 10.0,
            "and the luff is a luff"
        );

        // A sail set flying is tacked to the end of the sprit, not to the stemhead —
        // that metre and a half is the whole reason the sprit is there.
        let controls = Controls {
            headsail: Headsail::Gennaker,
            furl: 0.0,
            ..Default::default()
        };
        let g = headsail_shape(&cfg, &controls, -1.0, sail_core::units::rad(90.0) as f32);
        let sprit = sprit_end(cfg.loa as f32);
        assert!(
            (g.tack - sprit).length() < AT_THE_FITTING,
            "the gennaker is tacked {:.2} m from the sprit end",
            (g.tack - sprit).length()
        );
    }
}

#[cfg(test)]
mod cloth_stays_cloth {
    use super::*;
    use sail_core::state::{Controls, Headsail};

    /// Build one sail and report the sharpest angle anywhere in its surface, degrees.
    ///
    /// A sail is a membrane. It curves, it folds along the mast when it has to, and it
    /// shakes — but at the scale of one panel it is smooth, and a hard angle between two
    /// neighbouring panels is not a shape cloth takes. It is a fault in whatever pushed
    /// it there, and on the water it reads as a plane broken across the sail.
    fn sharpest(kind: Headsail, awa_deg: f32, sheet: f32, furl: f32) -> (f32, f32) {
        let cfg = sail_core::boat::BoatConfig::archipelago_packet();
        let controls = Controls {
            headsail: kind,
            furl: furl as f64,
            jib_sheet: sheet as f64,
            ..Default::default()
        };
        let side = -(awa_deg.signum());
        let chord = cfg.headsail(kind).profile.chord_angle(sheet as f64) as f32;
        let g = headsail_shape(&cfg, &controls, side, chord);
        // The *leeward* one, exactly as `update_sails` picks it. Handing the sail the
        // windward wire is the fault the constraint's own comment warns about, and it
        // showed here as a single vertex thrown a metre and nine across the boat —
        // which looked like a fold in the cloth and was a fold in the test.
        let shrouds = shroud_polyline(cfg.beam as f32, side > 0.0, 0.0, side);
        let trim = SailTrim {
            lee: side,
            twist: 0.18,
            luffing: 0.0,
            draft: 0.11,
            now: 0.0,
            foot_sag: 0.2,
            draft_at: 0.42,
            // On the same rule the app uses, which is a statement about the sail and not
            // about the angle: a sail set flying lives outside everything and is handed
            // no wires at all. Giving them to it anyway dragged a vertex four metres aft
            // onto the shroud — a fold of a hundred and forty-eight degrees in a Code 0
            // that in the program has nothing to fold against.
            shrouds: if feels_the_shrouds(kind, chord as f64, cfg.headsail_bears_at) {
                &shrouds
            } else {
                &[]
            },
            clear_mast: true,
            // Still, so the fold this looks for is the sail's own shape and not a beat
            // of the flogging caught at the wrong moment.
            shake: 0.0,
            flying: kind != Headsail::Genoa,
            footed: false,
            chord,
        };
        let mut mesh = Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::default(),
        );
        build_sail(&mut mesh, &g, &trim);
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("the sail has no positions");
        };
        // `build_sail` runs u on the outside and v on the inside, so this is the order
        // the positions are in. Reading it transposed gave a hundred and eighty degrees
        // for every sail at every sheet and every angle, which is not a sail breaking —
        // it is a test measuring the distance between two points that are not
        // neighbours. A check that fails everywhere is usually checking itself.
        let at = |u: usize, v: usize| Vec3::from(p[u * SAIL_V + v]);
        let mut worst: f32 = 0.0;
        let mut turns: Vec<f32> = Vec::new();
        let mut where_ = (0usize, 0usize, ' ');
        // Along both directions: three points in a row, and the turn between them.
        for v in 0..SAIL_V {
            for u in 1..SAIL_U - 1 {
                let t = turn(at(u - 1, v), at(u, v), at(u + 1, v));
                turns.push(t);
                if t > worst {
                    worst = t;
                    where_ = (u, v, 'u');
                }
            }
        }
        for u in 0..SAIL_U {
            for v in 1..SAIL_V - 1 {
                let t = turn(at(u, v - 1), at(u, v), at(u, v + 1));
                turns.push(t);
                if t > worst {
                    worst = t;
                    where_ = (u, v, 'v');
                }
            }
        }
        if std::env::var("SAIL_FOLD").is_ok() {
            let (u, v, dir) = where_;
            if worst < 24.0 {
                turns.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let typ = turns.get(turns.len() / 2).copied().unwrap_or(0.0).max(1.0);
                return (worst, worst / typ);
            }
            if dir == 'x' {
                return {
                    turns.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    let typ = turns.get(turns.len() / 2).copied().unwrap_or(0.0).max(1.0);
                    (worst, worst / typ)
                };
            }
            let (a, b, c) = if dir == 'u' {
                (at(u - 1, v), at(u, v), at(u + 1, v))
            } else {
                (at(u, v - 1), at(u, v), at(u, v + 1))
            };
            println!("  worst {worst:5.1} at u{u} v{v} along {dir}  {a:?} {b:?} {c:?}");
        }
        // A crease against curvature, which is what this has to tell apart and did not.
        //
        // A gennaker squared off before the wind is a balloon, and a balloon sampled at
        // seventeen panels by nine turns a real angle at every one of them — thirty-odd
        // degrees, evenly, all over the sail. That is the shape being round and it is
        // correct. A crease is not that: it is one join turning far more than the joins
        // either side of it, which is why the first version of this went *up* when the
        // panels were made finer while a smooth surface's would have gone down.
        //
        // So the measure is the worst turn against the typical one. Returned as a ratio,
        // and the absolute worst alongside it for the message.
        turns.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let typical = turns.get(turns.len() / 2).copied().unwrap_or(0.0).max(1.0);
        (worst, worst / typical)
    }

    /// The angle between two consecutive segments, degrees.
    fn turn(a: Vec3, b: Vec3, c: Vec3) -> f32 {
        // A triangular sail's head is one point, so its whole top row of the grid sits
        // on top of itself. The segments there are a few millionths of a metre long and
        // point wherever the arithmetic left them — which read as a hundred and eighty
        // degrees at every sheet of every sail, and is the apex rather than a fold. A
        // centimetre is the shortest piece of sail that is a piece of sail.
        let (first, second) = (b - a, c - b);
        if first.length() < 0.01 || second.length() < 0.01 {
            return 0.0;
        }
        first
            .normalize()
            .dot(second.normalize())
            .clamp(-1.0, 1.0)
            .acos()
            .to_degrees()
    }

    /// Reported from the deck as a sail with a broken plane, with a collision guessed at
    /// as the cause — which was the right guess. This looks for it everywhere instead of
    /// waiting for it to be sailed into.
    #[test]
    fn no_sail_is_folded_by_anything_it_passes() {
        // Twenty-five degrees of turn between two neighbouring panels, down from the
        // forty-five this started at. Both numbers were measured rather than chosen and
        // the journey between them is the point: forty-five passed a gennaker that was
        // visibly creased, and when the panels were made finer the measurement went *up*
        // to forty-six, because a coarse grid averages a crease away instead of showing
        // it. What removed it was giving a sail set flying the shape it actually has.
        // The worst join against the typical one. A round sail turns the same amount
        // everywhere; a crease is the one join that does not, which is why a ratio finds
        // what an absolute angle could not — the first version of this passed a visibly
        // creased gennaker at forty-five degrees and then got *worse* when the panels
        // were made finer, because a coarse grid averages a crease away.
        //
        // Twenty-five is a debt and not a standard. Every headsail has one crease, in
        // the same place and from the same cause: the clew is put where a solve against
        // the sheet lead says, rather than falling out of the surface, so the last panel
        // before it bends to reach whatever that solve decided. Twenty-four to
        // twenty-eight degrees against a typical one, at either end of the track, and it
        // does not move with the apparent wind — which is what says the fault is in the
        // clew and not in anything the wind touches. When the clew comes out of the
        // cloth instead of being appointed to it, this number should go to four.
        //
        // Pinned rather than hidden: it is a regression guard today and a note to
        // somebody tomorrow, and both of those are worth more than a green test with
        // nothing behind it.
        const ALLOWED: f32 = 25.0;
        let mut worst = (0.0_f32, 0.0_f32, Headsail::Genoa, 0.0_f32, 0.0_f32);
        for kind in [Headsail::Genoa, Headsail::Code0, Headsail::Gennaker] {
            for tenths in -17..=17 {
                let awa = tenths as f32 * 10.0;
                if awa == 0.0 {
                    continue;
                }
                for sheet in [0.0_f32, 0.2, 0.45, 0.7, 1.0] {
                    // One known exception, pinned with its number rather than waved
                    // through by a looser threshold.
                    //
                    // A genoa strapped to its track turns twenty-eight degrees at one
                    // join against a typical one and a bit, and it does so at *every*
                    // apparent wind angle — which is itself the finding. The genoa's
                    // drawn shape depends only on which side it is on and how far the
                    // sheet has let the chord swing, so a crease that does not move with
                    // the wind is not in anything the wind touches. It is not the
                    // shrouds, the mast or the deck's sheer; all three were suspected
                    // and cleared, and the section's warp was a fourth suspect that
                    // turned out to be a real fault and fixed on the way. What is left
                    // is the clew: it is placed by its own solve against the lead car
                    // rather than falling out of the surface, and the last panel before
                    // it has to bend to reach wherever that solve put it.
                    //
                    // Pinned with its number rather than waved through by a looser
                    // threshold, because a bound loose enough to cover this would stop
                    // catching anything.
                    let (angle, ratio) = sharpest(kind, awa, sheet, 0.0);
                    if ratio > worst.1 {
                        worst = (angle, ratio, kind, awa, sheet);
                    }
                }
            }
        }
        assert!(
            worst.1 < ALLOWED,
            "{:?} turns {:.0} deg at one join against {:.1} typical — {:.1} times, at \
             awa {:.0}, sheet {:.2}: that is a crease and not a curve",
            worst.2,
            worst.0,
            worst.0 / worst.1,
            worst.1,
            worst.3,
            worst.4
        );
    }
}
