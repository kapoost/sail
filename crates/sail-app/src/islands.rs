//! Drawing the archipelago.
//!
//! The mesh comes from the same signed-distance field the physics runs aground on, so
//! the coast you see and the coast you touch are the same coast. That is worth more
//! than it sounds: the usual way to build this is to model the land and then write a
//! separate collision shape, and the two drift apart until the player is grounding in
//! open water or sailing through a cliff.
//!
//! Each island is a radial fan — a ring of points round the shore, lifted inland to
//! the island's own summit. Crude next to a heightfield, and right for the job: the
//! land is a silhouette on the horizon and a thing to keep off, not somewhere you go.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use sail_core::land::{Archipelago, Island};

use crate::frame;
use crate::sim::Sim;

/// The land, and keeping it where it belongs when the origin moves.
///
/// What this module does in a frame, in one place — see `stages.rs` for why the
/// schedule is named sets rather than one long chain.
/// The seed the archipelago in front of her came from.
///
/// **Not always the one in the save.** The islands are a pure function of a seed and the
/// seed can change under you: joining somebody puts you in theirs, and casting off from
/// the platform brings you back to your own. Anything that is moored, marked or buried in
/// *this* water has to know which water it is, and until this existed the only answer
/// available was "the one the game started in", which stopped being true the first time
/// anybody joined a game.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Sea(pub u64);

pub fn plugin(app: &mut App) {
    use crate::stages::{Build, Deck};
    app.add_systems(Startup, setup.in_set(Build::Scene));
    app.add_systems(Update, follow_origin.in_set(Deck::Follow));
}

/// Points round the shoreline of each island.
const RING: usize = 96;
/// Rings from the shore to the summit. More than before: the relief needs somewhere
/// to happen, and seven rings over a kilometre and a half of island is a tent.
const STEPS: usize = 22;

/// Marks an island so it can be re-placed when the render origin moves.
#[derive(Component)]
pub struct Landmass {
    /// Where it sits in simulation coordinates.
    pub at: glam::DVec2,
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    origin: Res<frame::RenderOrigin>,
    sim: Res<Sim>,
) {
    let Some(land) = sim.sim.land.clone() else {
        return;
    };

    // Two materials: what is above water and what is below it. The underwater skirt
    // is what makes a shoal visible before you are on it — a pale shelf showing
    // through the surface is the oldest piloting cue there is.
    let rock = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.94,
        ..default()
    });
    let shoal = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.96,
        ..default()
    });

    for island in land.islands() {
        commands.spawn((
            Landmass { at: island.centre },
            crate::ink::Inked,
            Mesh3d(meshes.add(island_mesh(island, false))),
            MeshMaterial3d(rock.clone()),
            Transform::from_translation(origin.to_bevy(island.centre, 0.0)),
        ));
        commands.spawn((
            Landmass { at: island.centre },
            crate::ink::Inked,
            Mesh3d(meshes.add(island_mesh(island, true))),
            MeshMaterial3d(shoal.clone()),
            Transform::from_translation(origin.to_bevy(island.centre, 0.0)),
        ));
    }
}

/// Keep the land where the simulation says it is after a change of render origin.
pub fn follow_origin(origin: Res<frame::RenderOrigin>, mut q: Query<(&Landmass, &mut Transform)>) {
    if !origin.is_changed() {
        return;
    }
    for (land, mut transform) in &mut q {
        transform.translation = origin.to_bevy(land.at, 0.0);
    }
}

/// One island, in its own local coordinates.
///
/// `below` builds the drowned part instead: the same outline carried down the shelf.
///
/// Coloured per vertex rather than textured. These are seen from one to five
/// kilometres away and almost never closer, which is a distance at which a texture is
/// a mip-mapped smear and the only things the eye actually gets are the shape of the
/// skyline and the way the colour changes with height. So: wet rock at the water, a
/// strand above it, heath over that, and bare stone on the tops — which is what a
/// Shetland island looks like and, more to the point, is legible at four miles.
fn island_mesh(island: &Island, below: bool) -> Mesh {
    let mut positions: Vec<Vec3> = Vec::new();
    let mut colours: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for step in 0..=STEPS {
        let t = step as f32 / STEPS as f32;
        for i in 0..RING {
            let bearing = i as f32 / RING as f32 * std::f32::consts::TAU;
            let shore = island.shore_radius(bearing as f64) as f32;
            let (r, y) = if below {
                // Out and down the bank.
                let out = shore + t * 300.0;
                (out, -34.0 * (t * t).min(1.0))
            } else {
                // In and up, and the height comes from the island itself rather than
                // from a curve written here.
                //
                // This used to be a cliff profile with two harmonics of *bearing* on it,
                // which is why every island read as a dish-cover: a function of bearing
                // scales the whole radial line together, so an island could have lobes
                // and never a ridge running inland, never a valley, never two tops. And
                // it was a second description of the ground — the depth sounder had a
                // third — so a boat could sound six metres over land the eye was looking
                // at the top of.
                //
                // `Island::elevation` is the one description now, in world coordinates,
                // and the mesh is a sampling of it. All the relief in the skyline comes
                // from there, which also means the chart, the sounder and the drawn hill
                // are the same hill.
                let inward = shore * (1.0 - t);
                let at = island.centre
                    + glam::DVec2::new(bearing.sin() as f64, bearing.cos() as f64) * inward as f64;
                (inward, island.elevation(at) as f32)
            };
            positions.push(Vec3::new(bearing.sin() * r, y, -bearing.cos() * r));
            colours.push(if below {
                // The shoal, going to the colour of deep water, so a bank announces
                // itself through the surface the way a real one does — but a northern
                // bank is stone and weed, not coral sand, so it is a cold grey-green
                // rather than the tropical pale it was.
                let fade = 1.0 - t;
                [
                    0.11 + 0.17 * fade,
                    0.16 + 0.21 * fade,
                    0.17 + 0.15 * fade,
                    1.0,
                ]
            } else {
                let up = (y / island.height as f32).clamp(0.0, 1.0);
                // Wet rock, strand, heath, stone.
                let band = |a: [f32; 3], b: [f32; 3], f: f32| {
                    [
                        a[0] + (b[0] - a[0]) * f,
                        a[1] + (b[1] - a[1]) * f,
                        a[2] + (b[2] - a[2]) * f,
                    ]
                };
                // An Atlantic spring: black cliffs with a green roof on them.
                //
                // The arrangement is the thing, and it is the same on every coast facing
                // that ocean. At the water a band of wet black basalt the sea keeps
                // scrubbed, and above the splash a line of pale lichen and guano on dry
                // rock. The cliff face itself is dark and almost bare — nothing grows on
                // vertical stone but a few tufts in the cracks. And then the top, where
                // it lies back far enough to hold soil, is grass: the loudest green in
                // this whole picture, because it is a hundred inches of rain a year on
                // volcanic dirt with nothing to shade it.
                //
                // The contrast between those two is the postcard, and it only works if
                // the rock stays genuinely dark.
                let c = if up < 0.06 {
                    // Wet basalt and weed, scrubbed by every tide.
                    band([0.050, 0.055, 0.058], [0.095, 0.098, 0.095], up / 0.06)
                } else if up < 0.14 {
                    // The splash zone: dry rock, lichen, the white line of the birds.
                    band(
                        [0.095, 0.098, 0.095],
                        [0.300, 0.300, 0.285],
                        (up - 0.06) / 0.08,
                    )
                } else if up < 0.52 {
                    // The face. Dark stone with a little green working into the cracks
                    // as it begins to lie back.
                    band(
                        [0.230, 0.228, 0.215],
                        [0.165, 0.215, 0.120],
                        (up - 0.14) / 0.38,
                    )
                } else if up < 0.80 {
                    // The green roof, and it is meant to be loud.
                    band(
                        [0.165, 0.215, 0.120],
                        [0.240, 0.470, 0.165],
                        (up - 0.52) / 0.28,
                    )
                } else {
                    // Summits: cropped grass going to stone and sky-bleached rock.
                    band(
                        [0.240, 0.470, 0.165],
                        [0.330, 0.420, 0.270],
                        (up - 0.80) / 0.20,
                    )
                };
                [c[0], c[1], c[2], 1.0]
            });
        }
    }

    for step in 0..STEPS {
        for i in 0..RING {
            let next = (i + 1) % RING;
            let a = (step * RING + i) as u32;
            let b = (step * RING + next) as u32;
            let c = ((step + 1) * RING + next) as u32;
            let d = ((step + 1) * RING + i) as u32;
            if below {
                indices.extend_from_slice(&[a, b, c, a, c, d]);
            } else {
                indices.extend_from_slice(&[a, d, c, a, c, b]);
            }
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    mesh.insert_indices(Indices::U32(indices));
    mesh.compute_normals();
    mesh
}

/// The archipelago the game is set in.
///
/// Eighteen kilometres square with twenty-six islands: far enough that the far side is
/// a passage rather than a hop, close enough that there is always something on the
/// horizon to steer for.
pub fn generate(seed: u64) -> Archipelago {
    // Five and a half kilometres each way rather than nine, which brings the harbours
    // within a sailable leg of each other — measured, the median gap drops from six
    // miles to under four and the nearest neighbour from three to under two.
    //
    // It also makes it an archipelago rather than a scatter: at nine kilometres the
    // islands were mostly out of sight of one another, and an island group you can see
    // across is the thing that makes coastal piloting a pleasure rather than a
    // navigation exercise. The count stays at twenty-six and the generator's own
    // rejection rule keeps the channels between them open, so what gives way is empty
    // water and not the sailing room.
    // Thirty-four rather than twenty-six, because the smaller square could not place
    // twenty-six: the generator keeps a navigable channel between every pair and simply
    // gives up on the ones that will not fit, so a tighter world with the same count
    // came out with a third fewer islands and six harbours instead of nine. More
    // candidates in means the same number of islands actually land.
    //
    // The number is still a ceiling now that the generator walks a chain rather than
    // scattering: it fills the band it drew and stops where the water runs out, which
    // over three hundred days lands between twenty-four and thirty-four islands with a
    // median of thirty-two. Asking for more would not make a denser chain, only a
    // longer list of berths it refuses.
    Archipelago::generate_around(seed ^ 0x15_1A_4D, 5_600.0, 34, 900.0)
}
