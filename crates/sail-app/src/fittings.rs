//! What a cockpit has in it.
//!
//! The deck had a surface and no furniture: no toerail along the sheer, no coaming
//! round the cockpit, no way into the cabin, no winches for the sheets that are
//! visibly led to them, and — the one that matters — nothing in the helmsman's hands.
//!
//! The tiller is not decoration. It is the only instrument on this boat that reads
//! continuously and needs no numbers: how far over it is says how hard she is
//! griping, and watching it creep to windward as the breeze builds is how a sailor
//! knows to reef before the heel gauge does. A boat steered by an invisible rudder is
//! being driven, not sailed.

use bevy::prelude::*;

use crate::deck;
use crate::sim::Sim;

/// The tiller, so it can be swung with the rudder.
#[derive(Component)]
pub struct Tiller;

/// The compass card, which stays still while the boat turns under it.
#[derive(Component)]
pub struct CompassCard;

/// Where the rudder stock comes through the deck, metres aft of the centre of gravity.
const STOCK_Z: f32 = 4.60;
/// Length of the tiller forward of the stock.
const TILLER_LENGTH: f32 = 1.70;

/// Everything needed to put a fitting on the boat: somewhere to make meshes and
/// textures, and the handful of finishes the whole boat is built out of.
pub struct Workshop<'a> {
    pub meshes: &'a mut Assets<Mesh>,
    pub images: &'a mut Assets<Image>,
    pub materials: &'a mut Assets<StandardMaterial>,
    pub varnish: Handle<StandardMaterial>,
    pub metal: Handle<StandardMaterial>,
    pub painted: Handle<StandardMaterial>,
    pub loa: f32,
}

pub fn spawn(b: &mut ChildSpawnerCommands, w: &mut Workshop) {
    let Workshop {
        varnish,
        metal,
        painted,
        loa,
        ..
    } = &*w;
    let (varnish, metal, painted, loa) = (varnish.clone(), metal.clone(), painted.clone(), *loa);
    let varnish = &varnish;
    let metal = &metal;
    let painted = &painted;
    // --- Bowsprit ------------------------------------------------------------
    //
    // A spar out over the stem, for a gennaker to be tacked to later. Stayed by a
    // bobstay down to the waterline and a pair of shrouds to the rail — see `rig.rs` —
    // because an unstayed sprit is not a spar, it is a lever against the stem.
    let tip = crate::boat::sprit_end(loa);
    let heel_at = Vec3::new(0.0, deck::deck_height(-loa * 0.46) + 0.18, -loa * 0.46);
    let run = tip - heel_at;
    b.spawn((
        Mesh3d(w.meshes.add(Cylinder::new(0.072, run.length()))),
        MeshMaterial3d(varnish.clone()),
        Transform::from_translation(heel_at + run * 0.5)
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, run.normalize())),
    ));

    // --- Compass -------------------------------------------------------------
    //
    // On the aft face of the coachroof, where the helmsman reads it without leaving
    // the tiller. The card is the instrument: it stays pointing north while the boat
    // swings under it, which is the opposite of how a heading readout behaves and is
    // why a compass takes learning. The lubber line is the boat; the card is the world.
    let card_mat = w.materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(w.images.add(compass_card())),
        perceptual_roughness: 0.55,
        ..default()
    });
    let aft = deck::cabin_aft();
    let at = Vec3::new(0.0, deck::cabin_top(aft) - 0.18, aft + 0.07);
    b.spawn((
        Mesh3d(w.meshes.add(Cylinder::new(0.115, 0.10))),
        MeshMaterial3d(metal.clone()),
        Transform::from_translation(at).with_rotation(Quat::from_rotation_x(1.05)),
    ))
    .with_children(|bowl| {
        bowl.spawn((
            CompassCard,
            Mesh3d(w.meshes.add(disc(0.098))),
            MeshMaterial3d(card_mat.clone()),
            Transform::from_xyz(0.0, 0.052, 0.0),
        ));
    });

    // --- The tiller ----------------------------------------------------------
    //
    // Pivoted at the rudder head and reaching forward into the cockpit, where a hand
    // can be on it. The shaft is offset forward by half its length so the transform's
    // rotation happens about the stock rather than about the middle of the stick.
    b.spawn((
        Tiller,
        Transform::from_xyz(0.0, deck::deck_height(STOCK_Z) + 0.18, STOCK_Z),
        Visibility::default(),
    ))
    .with_children(|t| {
        t.spawn((
            Mesh3d(w.meshes.add(Cuboid::new(0.055, 0.075, TILLER_LENGTH))),
            MeshMaterial3d(varnish.clone()),
            // Forward of the pivot, and rising slightly, the way a tiller does so it
            // clears the coaming when it is hard over.
            Transform::from_xyz(0.0, 0.10, -TILLER_LENGTH * 0.5)
                .with_rotation(Quat::from_rotation_x(0.10)),
        ));
        // The head of the stock it is clamped to.
        t.spawn((
            Mesh3d(w.meshes.add(Cylinder::new(0.065, 0.24))),
            MeshMaterial3d(metal.clone()),
            Transform::from_xyz(0.0, 0.0, 0.0),
        ));
    });

    // --- Winches -------------------------------------------------------------
    //
    // Two for the sheets, one each side on the coaming, and one on the coachroof for
    // the bank of slow lines. They are drums rather than models of winches, which is
    // the right level of detail for something seen from two metres away, and they
    // matter because the sheets are already drawn running to them.
    let drum = w.meshes.add(Cylinder::new(0.105, 0.20));
    let top = w.meshes.add(Cylinder::new(0.125, 0.035));
    for side in [-1.0_f32, 1.0] {
        let z = 1.9;
        let x = side * deck::deck_half_beam(z) * 0.62;
        let y = deck::deck_height(z) + 0.10;
        b.spawn((
            Mesh3d(drum.clone()),
            MeshMaterial3d(metal.clone()),
            Transform::from_xyz(x, y, z),
        ));
        b.spawn((
            Mesh3d(top.clone()),
            MeshMaterial3d(metal.clone()),
            Transform::from_xyz(x, y + 0.11, z),
        ));
    }
    // The clutch-bank winch, on the coachroof where the lines come aft to.
    b.spawn((
        Mesh3d(drum.clone()),
        MeshMaterial3d(metal.clone()),
        Transform::from_xyz(0.42, deck::cabin_top(0.2) + 0.10, 0.2),
    ));

    // --- Companionway --------------------------------------------------------
    //
    // A cabin with no way into it is a box. The sliding hatch and the washboards under
    // it are the most recognisable thing on a small yacht's coachroof, and they also
    // tell you which end of the cabin is the back.
    let aft = deck::cabin_aft();
    b.spawn((
        Mesh3d(w.meshes.add(Cuboid::new(0.62, 0.52, 0.04))),
        MeshMaterial3d(varnish.clone()),
        Transform::from_xyz(0.0, deck::cabin_top(aft) - 0.26, aft + 0.02),
    ));
    // The hatch garage, slid shut.
    b.spawn((
        Mesh3d(w.meshes.add(Cuboid::new(0.70, 0.06, 0.78))),
        MeshMaterial3d(painted.clone()),
        Transform::from_xyz(0.0, deck::cabin_top(aft - 0.5) + 0.04, aft - 0.45),
    ));

    // --- Grabrails -----------------------------------------------------------
    //
    // Along the coachroof, which is what you hold on the way forward. Cheap, and their
    // absence is felt the moment anyone looks at the cabin top and wonders how you are
    // supposed to cross it in a seaway.
    let rail = w.meshes.add(Cuboid::new(0.05, 0.09, 2.6));
    for side in [-1.0_f32, 1.0] {
        let z = -1.1;
        b.spawn((
            Mesh3d(rail.clone()),
            MeshMaterial3d(varnish.clone()),
            Transform::from_xyz(
                side * deck::cabin_half_beam(z) * 0.72,
                deck::cabin_top(z) + 0.05,
                z,
            ),
        ));
    }
}

/// A flat disc with radial texture coordinates, for the compass card.
fn disc(radius: f32) -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};
    let n = 48;
    let mut positions = vec![Vec3::ZERO];
    let mut uvs = vec![Vec2::new(0.5, 0.5)];
    let mut indices: Vec<u32> = Vec::new();
    for i in 0..=n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        positions.push(Vec3::new(a.sin() * radius, 0.0, -a.cos() * radius));
        uvs.push(Vec2::new(0.5 + 0.5 * a.sin(), 0.5 - 0.5 * a.cos()));
        if i > 0 {
            indices.extend_from_slice(&[0, i as u32, i as u32 + 1]);
        }
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh.compute_normals();
    mesh
}

/// The card: ticks every ten degrees, longer every thirty, and a red north.
///
/// No lettering. Drawing legible glyphs procedurally is a lot of work for something
/// read at a glance from a metre away, and a card with a red north and a clear tick
/// pattern is read the way a real one is — by where the marks are, not by spelling
/// out the word.
fn compass_card() -> Image {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    const SIZE: u32 = 256;
    let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let u = (x as f32 + 0.5) / SIZE as f32 * 2.0 - 1.0;
            let v = (y as f32 + 0.5) / SIZE as f32 * 2.0 - 1.0;
            let r = (u * u + v * v).sqrt();
            // Bearing, clockwise from the top of the card.
            let bearing = u.atan2(-v).rem_euclid(std::f32::consts::TAU).to_degrees();

            let mut shade = Vec3::new(0.93, 0.92, 0.88);
            if r > 1.0 {
                data.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            // Ticks.
            let near = |step: f32| {
                let d = (bearing % step).min(step - bearing % step);
                d * r
            };
            if r > 0.72 && near(10.0) < 0.9 {
                shade = Vec3::new(0.18, 0.18, 0.20);
            }
            if r > 0.58 && near(30.0) < 1.3 {
                shade = Vec3::new(0.12, 0.12, 0.14);
            }
            // North: a red arrowhead rather than a letter.
            let from_north = (bearing.min(360.0 - bearing)) * r;
            if r > 0.22 && r < 0.80 && from_north < 7.0 {
                shade = Vec3::new(0.72, 0.16, 0.12);
            }
            if r > 0.88 {
                shade *= 0.80;
            }
            data.push((shade.x * 255.0) as u8);
            data.push((shade.y * 255.0) as u8);
            data.push((shade.z * 255.0) as u8);
            data.push(255);
        }
    }
    Image::new(
        Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// Hold the card still while the boat turns under it.
///
/// The boat's own transform carries `-heading`, so the card has to carry `+heading` to
/// come out pointing at the world. That is exactly what the gimballed card of a real
/// compass does, and it is why the numbers appear to run backwards to anyone who has
/// only ever read a digital heading.
pub fn update_compass(sim: Res<Sim>, mut q: Query<&mut Transform, With<CompassCard>>) {
    let heading = sim.render_state().heading as f32;
    for mut t in &mut q {
        t.rotation = Quat::from_rotation_y(heading);
    }
}

/// Swing the tiller with the rudder.
pub fn update_tiller(sim: Res<Sim>, mut q: Query<&mut Transform, With<Tiller>>) {
    let angle = sim.controls.rudder as f32;
    for mut t in &mut q {
        // Opposite to the blade. Positive rudder turns the bow to starboard, which
        // means the blade's trailing edge goes to starboard and the tiller — being
        // forward of the stock rather than aft of it — goes to port. Pushing the
        // tiller away from the wind to bear away is the oldest muscle memory in
        // sailing, and getting it backwards would be felt immediately.
        t.rotation = Quat::from_rotation_y(angle);
    }
}
