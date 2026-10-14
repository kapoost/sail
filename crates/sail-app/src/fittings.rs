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
    // Its heel is at the mast, which is where a bowsprit's heel goes.
    //
    // It was stepped at the stem — a spar a metre and a half long sitting in a fitting on
    // the stemhead, which is not a bowsprit, it is a bumkin bolted to the one part of the
    // boat least able to take it. A real sprit runs *inboard*: it lies along the foredeck
    // and fetches up against the mast or a samson post stepped beside it, so the whole
    // length of the deck takes the load and the stemhead fitting is only a fairlead. That
    // is why the spar is as long as it is and why nobody worries about the pull of a kite
    // on its end.
    let tip = crate::boat::sprit_end(loa);
    let heel_at = Vec3::new(
        0.0,
        deck::deck_height(crate::boat::MAST_FOOT_Z) + 0.16,
        crate::boat::MAST_FOOT_Z,
    );
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
        // Lit and visible from either face. A gimballed card turns to whatever the
        // horizon is doing, and a single-sided disc that happens to tip past the
        // vertical vanishes — which from the helm is a compass with nothing in it.
        double_sided: true,
        cull_mode: None,
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
    // A low base with a glass dome over it, which is what a compass is. A straight-sided
    // bowl crops the card the moment she heels — the card stays level and the rim does
    // not, so the rim comes across it — and the dome exists precisely so that it cannot:
    // every line of sight into a hemisphere clears its edge.
    b.spawn((
        Mesh3d(w.meshes.add(Cylinder::new(0.115, 0.06))),
        MeshMaterial3d(metal.clone()),
        Transform::from_translation(at - Vec3::new(0.0, 0.02, 0.0)),
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
        Transform::from_translation(at + Vec3::new(0.0, 0.028, 0.0)),
    ));
    // The dome. Glass: it has to be seen through, so it is nearly clear, and the little
    // it does reflect is what makes it read as glass rather than as a hole in the air.
    b.spawn((
        Mesh3d(w.meshes.add(Sphere::new(0.108))),
        MeshMaterial3d(w.materials.add(StandardMaterial {
            base_color: Color::srgba(0.86, 0.90, 0.94, 0.16),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.05,
            reflectance: 0.9,
            double_sided: true,
            cull_mode: None,
            ..default()
        })),
        Transform::from_translation(at + Vec3::new(0.0, 0.028, 0.0)),
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

    // --- Wind vane -----------------------------------------------------------
    wind_vane(w, b, varnish);

    // --- Winches -------------------------------------------------------------
    //
    // Two for the sheets, one each side on the coaming, and one on the coachroof for
    // the bank of slow lines. They are drums rather than models of winches, which is
    // matter because the sheets are already drawn running to them — and because in the
    // cockpit view you are sitting next to one. See `winch_mesh` for what the shape is
    // doing; it is all function, and the function is legible.
    let drum = w.meshes.add(winch_mesh());
    for side in [-1.0_f32, 1.0] {
        // Abaft the headsail track, so the sheet runs forward to its lead and back to
        // the drum without the two fittings fouling each other — and far enough aft
        // that the person grinding is sitting in the cockpit rather than kneeling on
        // the side deck.
        let z = 2.9;
        let x = side * (crate::boat::track_x(z) + 0.02);
        // On the deck rather than floating above it: the profile starts at its own
        // base, so the fitting is placed where it sits.
        let y = deck::deck_height(z);
        b.spawn((
            Mesh3d(drum.clone()),
            MeshMaterial3d(metal.clone()),
            // Turned so the self-tailing feeder faces the person grinding, which on a
            // real boat is what the installer spends ten minutes getting right.
            Transform::from_xyz(x, y, z).with_rotation(Quat::from_rotation_y(side * 0.5)),
        ));
    }
    // The clutch-bank winch, on the deck just forward of the coaming where the slow
    // lines come aft to. It used to live on the coachroof; there is no coachroof.
    b.spawn((
        Mesh3d(drum.clone()),
        MeshMaterial3d(metal.clone()),
        Transform::from_xyz(0.52, deck::deck_height(0.55), 0.55),
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
                let off = (bearing - cardinal)
                    .abs()
                    .min(360.0 - (bearing - cardinal).abs());
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

/// Marks the wind vane's turret, which turns to the angle it is set to.
#[derive(Component)]
pub struct VaneTurret;

/// Marks the vane blade itself, which falls to leeward when she is off course.
#[derive(Component)]
pub struct VaneBlade;

/// A wind-vane self-steering gear on the transom.
///
/// The thing that makes a vane gear worth drawing rather than hiding in a resource is
/// that it is the only autopilot you can *read*. An electric pilot is a box with a light
/// on it; a vane tells you, from the cockpit and without looking at an instrument, both
/// what it is trying to do and how badly it is failing:
///
/// * **The turret** is the setting. You slacken it, turn the vane until its plane lies
///   along the wind you want to hold, and clamp it. That angle *is* the course order —
///   there is no number anywhere — and here it is the apparent wind angle the pilot is
///   holding.
/// * **The lean** is the error. On course the wind comes edge-on and the blade stands
///   up; let her wander and the wind strikes the face of it and the blade falls to
///   leeward, and the further she is off the further it goes over. Watching it is how a
///   single-hander knows, from the companionway, whether the gear is coping.
///
/// The steering force is still the ram's, modelled in `autopilot::correct_under_load`,
/// and this is honest about that rather than pretending to be a servo-pendulum: a real
/// gear of this kind drives an oar that the water pushes sideways, with its own gearing,
/// its own slop and its own failure modes. What is drawn here is the vane and its
/// setting, driven by the same error the pilot is steering on, so what you see is what
/// it is doing.
fn wind_vane(w: &mut Workshop, b: &mut ChildSpawnerCommands, varnish: &Handle<StandardMaterial>) {
    // Right aft and clear of the tiller's sweep, which is where one goes: it has to be
    // in clean air behind the mainsail and it must not foul the helm.
    // Abaft the rudder stock: +z is aft on this boat, the mast being stepped at −1.8.
    let z = STOCK_Z + 0.34;
    b.spawn((
        VaneTurret,
        Transform::from_xyz(0.0, deck::deck_height(z) + 0.06, z),
        Visibility::default(),
    ))
    .with_children(|t| {
        // The post it all stands on.
        t.spawn((
            Mesh3d(w.meshes.add(Cylinder::new(0.030, PIVOT))),
            MeshMaterial3d(varnish.clone()),
            Transform::from_xyz(0.0, PIVOT * 0.5, 0.0),
        ));
        // The counterweight arm across the top, which is what makes a vane fall over
        // slowly and deliberately rather than slamming: it is balanced just off
        // vertical so gravity brings it back. With the lead on the end of it, because
        // an arm with nothing on it is a stick.
        t.spawn((
            Mesh3d(w.meshes.add(Cuboid::new(ARM, 0.024, 0.024))),
            MeshMaterial3d(varnish.clone()),
            Transform::from_xyz(0.0, PIVOT - 0.02, 0.0),
        ));
        t.spawn((
            Mesh3d(w.meshes.add(Cylinder::new(0.035, 0.07))),
            MeshMaterial3d(w.materials.add(StandardMaterial {
                base_color: Color::srgb(0.38, 0.38, 0.41),
                perceptual_roughness: 0.6,
                metallic: 0.5,
                ..default()
            })),
            Transform::from_xyz(ARM * 0.5, PIVOT - 0.02, 0.0)
                .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
        ));
        t.spawn((
            VaneBlade,
            Transform::from_xyz(0.0, PIVOT, 0.0),
            Visibility::default(),
        ))
        .with_children(|v| {
            // The blade, set *fore and aft* when the turret is at zero: a vane holds a
            // wind angle by presenting its edge to that wind, so its plane lies along
            // the wind it is set for.
            v.spawn((
                Mesh3d(w.meshes.add(plywood_blade())),
                MeshMaterial3d(varnish.clone()),
                Transform::from_xyz(0.0, 0.0, -0.04),
            ));
        });
    });
}

/// How high the blade pivots above the stern deck, and how long the counterweight arm
/// across the top of the post is.
const PIVOT: f32 = 0.58;
const ARM: f32 = 0.36;

/// The blade: height, and its chord at the root and at the tip.
///
/// Reported from the deck as looking pretty feeble, and it was — a foot and a half tall
/// and a foot wide, which is a sixth of a square metre. That is a weathervane on a shed.
/// The air vane on a self-steering gear is a plywood plate about a metre tall and a foot
/// or so across at the head, a third of a square metre of it, standing well above the
/// pushpit where the air is clean — it is one of the two or three things you can pick out
/// on a boat from a mile off, and it has to be that big for a reason: all the gear has to
/// work with is the difference between the wind on one face of it and the other, and
/// below about a quarter of a square metre there is not enough of that to move a helm.
const BLADE: (f32, f32, f32) = (1.02, 0.26, 0.40);

/// A tapered plywood plate, wider at the head than at the root.
///
/// Which is both what they look like and why: the taper puts the area high, where the
/// air is cleanest and the lever on the pivot is longest, without the weight that a
/// plate that wide the whole way down would carry at the top of a post. The leading edge
/// is straight and the trailing edge rakes aft, which is how one is cut out of a sheet.
fn plywood_blade() -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::mesh::PrimitiveTopology;

    let (height, root, tip) = BLADE;
    const THICK: f32 = 0.014;
    // Four corners of the plate, from the root forward corner round to the head.
    let front = -root * 0.5;
    let corners = [
        [front, 0.0],
        [front + root, 0.0],
        [front + tip, height],
        [front, height],
    ];
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    // The two faces, and then the four edges of the sheet, each with its own vertices so
    // that the normals are flat rather than smeared round the corner.
    for side in [-1.0_f32, 1.0] {
        let base = positions.len() as u32;
        for c in corners {
            positions.push([side * THICK * 0.5, c[1], c[0]]);
        }
        if side < 0.0 {
            indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        } else {
            indices.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
        }
    }
    for i in 0..4 {
        let (a, b) = (corners[i], corners[(i + 1) % 4]);
        let base = positions.len() as u32;
        positions.push([-THICK * 0.5, a[1], a[0]]);
        positions.push([THICK * 0.5, a[1], a[0]]);
        positions.push([THICK * 0.5, b[1], b[0]]);
        positions.push([-THICK * 0.5, b[1], b[0]]);
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_indices(bevy::render::mesh::Indices::U32(indices));
    mesh.compute_normals();
    mesh
}

/// Where the apparent wind comes *from*, as a direction in the boat's own frame.
///
/// The one definition of that mapping, and it exists because there were two. The
/// masthead windex has used `from_rotation_y(-awa)` since it was built and it has been
/// right every day since; the vane was written fresh with `+awa` and came out ninety
/// degrees round — standing square across the wind instead of edge-on to it, which is
/// a vane doing the one thing a vane must never do.
///
/// Both signs agree at nought, ninety and a hundred and eighty degrees, which is most of
/// why it survived a reading of the code: the three angles a person checks are the three
/// it cannot catch.
pub fn into_the_wind(awa: f32) -> Quat {
    Quat::from_rotation_y(-awa)
}

/// The same thing as a direction, for anything that needs the vector rather than the
/// turn — which at the moment is only the test that holds the two of them together, and
/// that is reason enough for it to exist: the test is the thing that caught a vane
/// standing square across the wind.
#[cfg(test)]
pub fn wind_from(awa: f32) -> Vec3 {
    into_the_wind(awa) * Vec3::NEG_Z
}

/// How far round the vane's turret stands when it is set to hold `awa`.
///
/// The blade's plane has to lie *along* that wind — a vane holds an angle by presenting
/// its edge to it, so the wind strikes neither face while she is on course and both
/// faces as soon as she is not. Fore and aft for a wind dead astern, athwartships for
/// one on the beam.
fn vane_turret(set: f32) -> Quat {
    // Literally the rotation that points the masthead windex at the wind, which is the
    // fact that makes this right: the vane's plane and the windex's arrow are both
    // laid along the same wind, so if one is correct the other cannot be ninety degrees
    // out. They were, until this was one function instead of two signs.
    into_the_wind(set)
}

/// Set the vane to what the pilot is holding, and lean it by how far she is off.
///
/// Both of those are the same two numbers the pilot steers on, which is the point: the
/// gear on the transom is not an illustration of the autopilot, it is the autopilot's
/// state made visible. If it is leaning hard, the pilot is working hard.
pub fn update_vane(
    sim: Res<Sim>,
    pilot: Res<crate::input::Pilot>,
    mut turret: Query<&mut Transform, (With<VaneTurret>, Without<VaneBlade>)>,
    mut blade: Query<&mut Transform, With<VaneBlade>>,
) {
    use sail_core::autopilot::PilotMode;
    // How far over it falls at its stop, and how much wind error gets it there. A vane
    // is near its limit at about twenty-five degrees of error: past that it is on the
    // stops and the gear has nothing left, which is exactly when a single-hander looks
    // aft and starts shortening sail.
    const LEAN_LIMIT: f32 = 40.0;
    const FULL_ERROR: f32 = 25.0;

    let (set, error) = match pilot.mode {
        PilotMode::ApparentWind { awa: target, .. } => (
            target as f32,
            sail_core::units::wrap_pi(sim.sim.telemetry.awa - target) as f32,
        ),
        // Not steering on the wind: the vane stands upright and keeps whatever it was
        // last clamped at, which is what an unshipped gear looks like from the cockpit.
        _ => (f32::NAN, 0.0),
    };

    if let Ok(mut t) = turret.single_mut() {
        if set.is_finite() {
            t.rotation = vane_turret(set);
        }
    }
    if let Ok(mut t) = blade.single_mut() {
        // Falls to leeward: the wind has moved round to one side of the vane's plane, so
        // it strikes that face and pushes the blade over the other way. A positive error
        // is the wind further round to starboard, so the top goes to port.
        let lean = (error.to_degrees() / FULL_ERROR * LEAN_LIMIT)
            .clamp(-LEAN_LIMIT, LEAN_LIMIT)
            .to_radians();
        t.rotation = Quat::from_rotation_z(lean);
    }
}

/// A sheet winch, as one solid of revolution.
///
/// It was a cylinder with a disc on top, which is the right level of detail for a thing
/// seen at two metres until you are *sitting next to it*, and in the cockpit view you
/// are. Everything that makes a winch recognisable is in its profile, so the profile is
/// the model: one lathe turns the base flange, the waisted drum, the self-tailing jaw
/// with its groove, the cap and the handle socket, and comes out as a single mesh with a
/// single outline round it rather than a pile of primitives each wearing its own.
///
/// What the shapes are for, since a winch is all function:
///
/// * **The waist.** A drum is narrower in the middle than at its ends so the turns ride
///   *down* towards the base as they tighten. A straight drum lets them climb and
///   override, which jams the winch at the one moment you need it — easing a sheet in a
///   gust.
/// * **The ribs.** Vertical knurling. A rope round a polished drum slips; the ribs are
///   what make three turns hold a sail's worth of load. They are also the detail that
///   reads from the helm, because at this distance what you see of a winch is the way
///   light breaks up along its side.
/// * **The jaw.** The self-tailer: a sprung groove in the crown that grips the tail so
///   one person can grind and tail at once. It is the thing that distinguishes a winch
///   made after about 1975 from one made before, and it is the most recognisable part of
///   the shape.
/// * **The socket.** Square, in the top, for the handle. Left empty here, as it should
///   be: a handle stowed in a winch goes over the side on the first lurch.
fn winch_mesh() -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};

    /// Turned stations, bottom to top: radius and height, in metres.
    const PROFILE: [(f32, f32); 13] = [
        (0.000, 0.000),
        (0.113, 0.000),
        (0.113, 0.014),
        (0.105, 0.024),
        (0.097, 0.105),
        (0.105, 0.182),
        (0.119, 0.193),
        (0.103, 0.208),
        (0.123, 0.226),
        (0.106, 0.236),
        (0.040, 0.252),
        (0.031, 0.256),
        (0.012, 0.250),
    ];
    /// Which stations are drum, and therefore ribbed.
    const RIBBED: std::ops::Range<usize> = 3..6;
    /// Segments round. Enough that twelve ribs get six segments each, which is the
    /// fewest that makes a rib a rib rather than a wobble.
    const SEGMENTS: usize = 72;
    const RIBS: f32 = 12.0;
    const RIB_DEPTH: f32 = 0.0045;

    let mut positions: Vec<Vec3> = Vec::new();
    let mut normals: Vec<Vec3> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for (station, &(r, y)) in PROFILE.iter().enumerate() {
        // The ribs stand proud over the drum and fade out at both of its ends, so they
        // do not leave a step where the drum meets its flanges.
        let ribbed = if RIBBED.contains(&station) {
            let span = (RIBBED.end - RIBBED.start - 1).max(1) as f32;
            let along = (station - RIBBED.start) as f32 / span;
            (std::f32::consts::PI * along).sin()
        } else {
            0.0
        };
        for i in 0..=SEGMENTS {
            let a = std::f32::consts::TAU * i as f32 / SEGMENTS as f32;
            let rib = (a * RIBS).cos() * 0.5 + 0.5;
            let radius = r + ribbed * rib * RIB_DEPTH;
            positions.push(Vec3::new(a.sin() * radius, y, a.cos() * radius));
            // Outward, which is right everywhere but the two end caps and near enough
            // there: this is a lit metal fitting, not a surface anyone reads shading off.
            normals.push(Vec3::new(a.sin(), 0.0, a.cos()));
        }
    }
    let ring = SEGMENTS + 1;
    for station in 0..PROFILE.len() - 1 {
        for i in 0..SEGMENTS {
            let a = (station * ring + i) as u32;
            let b = (station * ring + i + 1) as u32;
            let c = ((station + 1) * ring + i + 1) as u32;
            let d = ((station + 1) * ring + i) as u32;
            indices.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

#[cfg(test)]
mod vane {
    use super::*;

    /// The blade must be edge-on to the wind it is set for, at every angle.
    ///
    /// Written after it was found standing square across the wind instead. The test
    /// matters more than the fix: a sign error in an angle is invisible in a code
    /// reading, survives the three angles anybody checks by hand — nought, ninety and a
    /// hundred and eighty, where both signs agree — and is instantly obvious to a
    /// sailor looking at the transom. One dot product catches all of it.
    #[test]
    fn the_vane_stands_edge_on_to_the_wind_it_is_set_for() {
        for degrees in [0.0_f32, 20.0, 45.0, 70.0, 90.0, 120.0, 150.0, 175.0] {
            for side in [1.0_f32, -1.0] {
                let set = (degrees * side).to_radians();
                // The blade is thin across its local x, so that is its face's normal.
                let normal = vane_turret(set) * Vec3::X;
                let wind = wind_from(set);
                assert!(
                    normal.dot(wind).abs() < 1.0e-5,
                    "set to {:.0} deg the vane presents its face to the wind by {:.2} \
                     of a right angle, which is a vane that cannot steer",
                    degrees * side,
                    normal.dot(wind).abs()
                );
                // And the blade's own plane contains the wind, which is the same claim
                // from the other side and catches a normal that is right by accident.
                let along = vane_turret(set) * Vec3::Z;
                assert!(along.dot(wind).abs() > 0.999);
            }
        }
    }

    /// The blade is the size an air vane has to be, and it is where one stands.
    ///
    /// Reported from the deck as feeble-looking, which it was: a sixth of a square metre.
    /// The number is not cosmetic — a vane gear works on the difference in wind pressure
    /// between the two faces of this plate and nothing else, so a small plate is a gear
    /// that cannot steer, and every real one is about a third of a square metre. The
    /// drawing is the only place that number exists, so this is where it is held.
    #[test]
    fn the_vane_is_big_enough_to_steer_with() {
        let (height, root, tip) = BLADE;
        let area = (root + tip) * 0.5 * height;
        assert!(
            (0.28..0.45).contains(&area),
            "a vane blade of {area:.2} square metres is not a self-steering gear"
        );
        // Taller than it is wide, and wider at the head: that is a vane rather than a
        // signboard, and the taper is what puts the area up in the clean air.
        assert!(height > tip * 2.0, "that is a signboard, not a vane");
        assert!(
            tip > root,
            "an air vane is wider at the head than at the root"
        );
        // Standing clear above the pushpit, in air the crew and the coamings have not
        // already spoiled.
        assert!(
            PIVOT + height > 1.4,
            "the head of it is {:.2} m above the deck, which is inside the mess",
            PIVOT + height
        );
        // And abaft everything the mainsail swings: the gear is at the extreme stern and
        // the boom ends over the cockpit, so they cannot meet — but both of those are
        // numbers somebody could change.
        let vane_z = STOCK_Z + 0.34;
        let boom_end = crate::boat::MAST_FOOT_Z + crate::rig::BOOM_LENGTH;
        assert!(
            vane_z > boom_end + 0.5,
            "the vane at z {vane_z:.2} is inside the boom's sweep, which ends at \
             {boom_end:.2}"
        );
    }

    /// The wind direction itself, against the convention the windex has always used.
    #[test]
    fn the_wind_comes_from_where_the_windex_says_it_does() {
        // Dead ahead is forward, which on this boat is negative z.
        assert!((wind_from(0.0) - Vec3::NEG_Z).length() < 1.0e-5);
        // On the starboard beam is to starboard, which is positive x.
        assert!((wind_from(std::f32::consts::FRAC_PI_2) - Vec3::X).length() < 1.0e-5);
        // And on the port beam, the other way. This is the pair that a sign error
        // cannot survive, and the pair that nought and a hundred and eighty let through.
        assert!((wind_from(-std::f32::consts::FRAC_PI_2) - Vec3::NEG_X).length() < 1.0e-5);
        // Astern.
        assert!((wind_from(std::f32::consts::PI) - Vec3::Z).length() < 1.0e-5);
    }
}
