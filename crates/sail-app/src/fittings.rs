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
///
/// A metre and a quarter. It was 1.70, which is a tiller for a much heavier boat and
/// looked like one — it reached most of the way up the cockpit. The length is a real
/// trade and not a drawing choice: a long tiller gives leverage and feel, a short one
/// gives the crew somewhere to sit, and on a boat this light the helm loads are small
/// enough that the crew wins.
const TILLER_LENGTH: f32 = 1.25;

/// Everything needed to put a fitting on the boat: somewhere to make meshes and
/// textures, and the handful of finishes the whole boat is built out of.
pub struct Workshop<'a> {
    pub meshes: &'a mut Assets<Mesh>,
    pub images: &'a mut Assets<Image>,
    pub materials: &'a mut Assets<StandardMaterial>,
    pub varnish: Handle<StandardMaterial>,
    pub metal: Handle<StandardMaterial>,
    pub loa: f32,
}

pub fn spawn(b: &mut ChildSpawnerCommands, w: &mut Workshop) {
    // Cloned out of the workshop so the asset collections inside it stay borrowable.
    let varnish = &w.varnish.clone();
    let metal = &w.metal.clone();
    let loa = w.loa;
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
    // On the forward coaming, facing aft, so it is read from the tiller without
    // leaning. With no coachroof left there is nowhere else it could sensibly go, and
    // this is where a flush-decked boat puts it anyway.
    let fwd = deck::cockpit_fwd();
    let at = Vec3::new(0.0, deck::deck_height(fwd) + 0.20, fwd + 0.30);
    // The bowl stands upright and the card hangs level inside it, which is the whole of
    // what makes a compass an instrument rather than a dial. A card bolted to the boat
    // reads her heel as a change of heading and is useless in the conditions you most
    // want it; a card on gimbals stays with the horizon and tells the truth at thirty
    // degrees of heel with the boat pitching.
    //
    // The bowl used to be tilted sixty degrees towards the helm with the card bolted
    // across it, which is a dial painted on a slope.
    b.spawn((
        Mesh3d(w.meshes.add(Cylinder::new(0.115, 0.13))),
        MeshMaterial3d(metal.clone()),
        Transform::from_translation(at),
    ))
    .with_children(|bowl| {
        // The lubber line: a mark on the *bowl*, which is bolted to the boat, and
        // therefore the one part of a compass that points where she is going. Reading
        // one is reading the card against this, and without it there is nothing to read
        // against.
        bowl.spawn((
            Mesh3d(w.meshes.add(Cuboid::new(0.008, 0.02, 0.055))),
            MeshMaterial3d(w.materials.add(StandardMaterial {
                base_color: Color::srgb(0.68, 0.14, 0.10),
                perceptual_roughness: 0.7,
                ..default()
            })),
            Transform::from_xyz(0.0, 0.066, -0.088),
        ));
    });
    // Hung from the boat rather than from the bowl, so undoing her attitude is one
    // quaternion rather than two, and so that a change to the mount cannot tilt the card.
    b.spawn((
        CompassCard,
        Mesh3d(w.meshes.add(disc(0.098))),
        MeshMaterial3d(card_mat.clone()),
        Transform::from_translation(at + Vec3::new(0.0, 0.062, 0.0)),
    ));

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
            Mesh3d(w.meshes.add(Cuboid::new(0.045, 0.062, TILLER_LENGTH))),
            MeshMaterial3d(varnish.clone()),
            // Forward of the pivot, and rising slightly, the way a tiller does so it
            // clears the coaming when it is hard over.
            Transform::from_xyz(0.0, 0.10, -TILLER_LENGTH * 0.5)
                .with_rotation(Quat::from_rotation_x(0.10)),
        ));
        // The head of the stock it is clamped to.
        t.spawn((
            Mesh3d(w.meshes.add(Cylinder::new(0.055, 0.21))),
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
        // Abaft the headsail track, so the sheet runs forward to its lead and back to
        // the drum without the two fittings fouling each other — and far enough aft
        // that the person grinding is sitting in the cockpit rather than kneeling on
        // the side deck.
        let z = 2.9;
        let x = side * (crate::boat::track_x(z) + 0.02);
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
    // The clutch-bank winch, on the deck just forward of the coaming where the slow
    // lines come aft to. It used to live on the coachroof; there is no coachroof.
    b.spawn((
        Mesh3d(drum.clone()),
        MeshMaterial3d(metal.clone()),
        Transform::from_xyz(0.52, deck::deck_height(0.55) + 0.10, 0.55),
    ));

    // --- Companionway --------------------------------------------------------
    //
    // A flush sliding hatch, because there is no trunk for it to stand on any more.
    // On a working boat this is all there is: a hatch in the deck, and everything you
    // live in underneath it.
    // Between the mast and the forward coaming, which is the only clear run of deck
    // on a boat laid out like this.
    let hatch = deck::cockpit_fwd() - 1.05;
    b.spawn((
        Mesh3d(w.meshes.add(Cuboid::new(0.74, 0.10, 0.86))),
        MeshMaterial3d(varnish.clone()),
        Transform::from_xyz(0.0, deck::deck_height(hatch) + 0.05, hatch),
    ));
    for side in [-1.0_f32, 1.0] {
        b.spawn((
            Mesh3d(w.meshes.add(Cuboid::new(0.05, 0.12, 0.94))),
            MeshMaterial3d(varnish.clone()),
            Transform::from_xyz(side * 0.40, deck::deck_height(hatch) + 0.06, hatch),
        ));
    }

    // --- Grabrails -----------------------------------------------------------
    //
    // Either side of the hatch, which is what you hold on the way forward. On a flush
    // deck they matter more than they did on a trunk: there is nothing else up there
    // to get a hand to.
    // Either side of the hatch, where a hand actually goes, and not out in the middle
    // of the foredeck where the first pair ended up.
    let rail = w.meshes.add(Cuboid::new(0.045, 0.085, 1.5));
    for side in [-1.0_f32, 1.0] {
        b.spawn((
            Mesh3d(rail.clone()),
            MeshMaterial3d(varnish.clone()),
            Transform::from_xyz(side * 0.58, deck::deck_height(hatch) + 0.05, hatch),
        ));
    }

    // --- Samson post ---------------------------------------------------------
    //
    // On the foredeck, for the anchor and for warps. A bow with nothing on it is the
    // mark of a boat nobody has ever had to tie up.
    let post = -loa * 0.38;
    b.spawn((
        Mesh3d(w.meshes.add(Cuboid::new(0.11, 0.34, 0.11))),
        MeshMaterial3d(varnish.clone()),
        Transform::from_xyz(0.0, deck::deck_height(post) + 0.17, post),
    ));
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
            // North: a red arrowhead rather than a letter. Widened, because at the
            // resolution this is played at the whole card is a dozen pixels across and
            // the only mark that can possibly read is the one that says which way is
            // north — which is also the only mark a helmsman needs at a glance.
            let from_north = (bearing.min(360.0 - bearing)) * r;
            if r > 0.18 && r < 0.86 && from_north < 11.0 {
                shade = Vec3::new(0.74, 0.15, 0.11);
            }
            // East, south and west as dark wedges. Letters would be mush; a wedge at a
            // quarter turn is legible as long as it is a wedge, and three of them tell
            // you which way the card is round even when the red one is hidden by the rim.
            for cardinal in [90.0_f32, 180.0, 270.0] {
                let off = (bearing - cardinal).abs().min(360.0 - (bearing - cardinal).abs());
                if r > 0.46 && r < 0.86 && off * r < 6.0 {
                    shade = Vec3::new(0.14, 0.14, 0.16);
                }
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
    let st = sim.render_state();
    // The exact inverse of the boat's own attitude, which is how a gimbal works: whatever
    // she does, the card does the opposite and stays where the world is. `attitude` is
    // yaw then roll then pitch, so undoing it is pitch then roll then yaw, each negated
    // and in the other order — and getting that order wrong gives a card that is right
    // upright and swims as soon as she heels, which is the failure that looks like it
    // works.
    for mut t in &mut q {
        t.rotation = Quat::from_rotation_x(-st.pitch as f32)
            * Quat::from_rotation_z(st.heel as f32)
            * Quat::from_rotation_y(st.heading as f32);
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
