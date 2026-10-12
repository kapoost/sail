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

const MAST_FOOT_Z: f32 = -1.8;
/// Height of the boom above the waterline — the mainsail's tack, and the one number
/// three separate places were each spelling out as `DECK + 1.25`.
const BOOM_HEIGHT: f32 = DECK + 1.95;
/// How high a headsail's clew floats when nothing is pulling it down, metres above
/// the tack. The sail hangs from the stay and the wind holds the foot up; only the
/// sheet brings it down.
const CLEW_FREE_RISE: f32 = 2.6;
/// And how low the sheet can drag it before the foot is on the deck.
const CLEW_MIN_RISE: f32 = 0.35;
/// Sheet length from the clew to the car, hauled right in and eased right out.
/// Shortest the sheet gets: a shackle and the depth of the block, no more.
///
/// The sheet does not end at the car — it runs *through* a block on it and on to the
/// winch, so hauling brings the clew all the way down to the fitting. An earlier value
/// of 2.9 m said the opposite: the clew could never come within three metres of its
/// lead, so the genoa was permanently half-eased and could not be sheeted in at all,
/// whatever the lever said.
const SHEET_SHORT: f32 = 0.25;
/// Longest, which is slack enough for the sail to fly off the stay and flog.
const SHEET_LONG: f32 = 7.4;
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

    commands
        .spawn((BoatRoot, Transform::default(), Visibility::default()))
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
            // Mast.
            b.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.09, 15.2))),
                MeshMaterial3d(spar_mat.clone()),
                Transform::from_xyz(0.0, DECK + 7.6, MAST_FOOT_Z),
            ));
            // Boom, pivoting at the mast. Its own transform carries the sheet angle.
            b.spawn((
                Boom,
                Transform::from_xyz(0.0, BOOM_HEIGHT, MAST_FOOT_Z),
                Visibility::default(),
            ))
            .with_children(|boom| {
                boom.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.065, 4.3))),
                    MeshMaterial3d(spar_mat.clone()),
                    Transform::from_xyz(0.0, 0.0, 2.15)
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
    let foot = 4.2 * (1.0 - 0.35 * reef);
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
                let tack = Vec3::new(0.0, BOOM_HEIGHT, MAST_FOOT_Z);
                (
                    t_.main,
                    SailGeometry {
                        tack,
                        // Reefing pulls the head down the mast, not the foot up.
                        head: Vec3::new(
                            0.0,
                            BOOM_HEIGHT + (15.4 - BOOM_HEIGHT) * (1.0 - 0.45 * reef),
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
                let tack = genoa_tack(cfg.loa as f32);
                let head = Vec3::new(0.0, DECK + 13.4, MAST_FOOT_Z);
                // The car the clew is solved against, which must be the one the
                // fitting is drawn at — the fourth place in this file where the same
                // geometry had been derived twice.
                let car = genoa_car_at(sim.controls.genoa_car as f32, side > 0.0);
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
            // Both sails twist. The main's comes from the vang arguing with the
            // mainsheet's angle; the headsail's comes from its lead car, which is the
            // only shape control it has and so had better be visible in the cloth.
            let twist = side
                * match surface.which {
                    Sail::Main => t_.twist as f32,
                    Sail::Genoa => t_.jib_twist as f32,
                };
            build_sail(
                &mut mesh,
                &geometry,
                &SailTrim {
                    lee: side,
                    twist,
                    luffing: forces.coeffs.luffing as f32,
                    draft: forces.camber as f32,
                    now,
                    shrouds: &shrouds,
                    clear_mast: surface.which == Sail::Genoa,
                },
            );
        }
    }
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
fn shroud_polyline(beam: f32, starboard: bool, heel: f32, lee: f32) -> Vec<Vec3> {
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
        if along > -SHROUD_RADIUS {
            let u = 1.0 - lateral / SHROUD_INFLUENCE;
            let weight = u * u * (3.0 - 2.0 * u);
            p -= normal * (along + SHROUD_RADIUS) * weight;
        }
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
fn clear_of_mast(point: Vec3) -> Vec3 {
    let radial = Vec2::new(point.x, point.z - MAST_FOOT_Z);
    let d = radial.length();
    if d >= MAST_CLEARANCE || point.y < DECK || point.y > crate::rig::MAST_HEAD {
        return point;
    }
    // Dead on the axis: nothing to push along, so nudge it forward out of the spar.
    let out = if d < 1e-4 {
        Vec2::new(0.0, -1.0)
    } else {
        radial / d
    };
    Vec3::new(
        out.x * MAST_CLEARANCE,
        point.y,
        MAST_FOOT_Z + out.y * MAST_CLEARANCE,
    )
}

/// Sheet length from the clew to the car for a given sheet setting.
pub fn sheet_length(jib_sheet: f32) -> f32 {
    // Squared, for the same reason the sheet-to-chord mapping is: the whole useful
    // band lives at the hauled-in end, and a linear travel buries it in the first
    // tenth of the winch.
    let t = jib_sheet.clamp(0.0, 1.0);
    SHEET_SHORT + (SHEET_LONG - SHEET_SHORT) * t * t
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
        shrouds,
        clear_mast,
    } = *trim;
    let mut positions = Vec::with_capacity(SAIL_U * SAIL_V);
    let mut uvs = Vec::with_capacity(SAIL_U * SAIL_V);

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

        for vi in 0..SAIL_V {
            let v = vi as f32 / (SAIL_V - 1) as f32;
            let belly = (std::f32::consts::PI * v).sin() * local_camber * chord_length;
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
            let point = luff.lerp(leech, v) + normal * (belly + flap + crease);
            let point = clear_of_shrouds(point, normal, shrouds);
            let point = if clear_mast {
                clear_of_mast(point)
            } else {
                point
            };
            // And never below the deck. A headsail flogging head to wind swept down
            // through the planking and across the companionway hatch, which no amount
            // of cloth can do: it lies *on* the deck.
            let point = Vec3::new(
                point.x,
                point.y.max(crate::deck::deck_height(point.z) + 0.05),
                point.z,
            );
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
