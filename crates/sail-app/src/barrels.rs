//! Barrels adrift, and the string of them astern.
//!
//! **Cargo that has already been in the water.** A deck load goes over the side in a
//! gale, the casks float because casks float, and for the next week they turn up along
//! the shores of an archipelago — which is a real thing that happened to every coasting
//! trade that ever existed, and the reason beachcombing is a word.
//!
//! Fishing one out is the boathook's job, so it uses the boathook's rules: close enough
//! to reach, slow enough to hold, and the deck level at the moment you take it. Nothing
//! new to learn and the same hand on the same spar.
//!
//! **And then they tow in a line.** The first follows a point astern of her, each one
//! after that follows the one ahead at the length of its painter, which is both how a
//! string of casks behaves on a towline and — as it happens — exactly the shape a snake
//! makes in the game everybody has played. It is the only piece of this simulator that
//! is a toy, and it earns its place by being the thing that makes you *look* at the
//! water a hundred metres off the bow.

use bevy::prelude::*;
use glam::DVec2;

use crate::sim::Sim;

/// The painter between one cask and the next, metres.
///
/// Three boat lengths of warp is what you would actually bend on — long enough that a
/// cask does not ride up on the one ahead when she slows, short enough that the string
/// follows her round a headland instead of cutting the corner onto it.
const PAINTER: f64 = 7.0;

/// How far astern the first one rides.
const ASTERN: f64 = 9.0;

/// How much of the wind a floating cask takes.
///
/// Most of a barrel is under water and the bit above it is round, so it drifts a little
/// slower than a lifebuoy and a little faster than a man — three per cent, which puts it
/// between the two numbers this game already uses and is the figure flotsam studies
/// give for a half-submerged drum.
const LEEWAY: f64 = 0.03;

/// How many are out there at once.
const HOW_MANY: usize = 14;

/// Where they are scattered, metres from her when the world is built.
const SCATTERED_WITHIN: (f64, f64) = (300.0, 2_600.0);

/// Everything floating that is not a boat.
#[derive(Resource, Default)]
pub struct Flotsam {
    /// Still in the water, waiting to be picked up.
    pub adrift: Vec<DVec2>,
    /// On the painter astern, nose to tail. The first is nearest her.
    pub astern: Vec<DVec2>,
}

impl Flotsam {
    /// The nearest one still in the water, for the hook to reach at.
    pub fn nearest(&self, to: DVec2) -> Option<DVec2> {
        self.adrift
            .iter()
            .copied()
            .min_by(|a, b| (*a - to).length().total_cmp(&(*b - to).length()))
    }

    /// Take that one out of the water and bend it on astern.
    pub fn hooked(&mut self, which: DVec2) {
        if let Some(i) = self
            .adrift
            .iter()
            .position(|at| (*at - which).length() < 0.5)
        {
            let at = self.adrift.remove(i);
            self.astern.push(at);
        }
    }
}

/// One cask, in the water or on the string — and *which* one.
///
/// The number matters: the entities come back from a query in whatever order the world
/// keeps them, so pairing them with the list by position would let two casks swap places
/// between frames. A cask that teleports is a cask nobody believes.
#[derive(Component)]
pub struct Cask(pub usize);

/// Scatter them over the water when the world is built, and whenever it is replaced.
///
/// In water a boat could sail in, which is the only thing that makes them findable: a
/// cask aground on a drying rock is a cask you can see and cannot reach, and a dozen of
/// those is a coastline of things that look like a mistake.
pub fn scatter(sim: Res<Sim>, mut flotsam: ResMut<Flotsam>) {
    if !flotsam.adrift.is_empty() || !flotsam.astern.is_empty() {
        return;
    }
    let Some(land) = sim.sim.land.as_ref() else {
        return;
    };
    let here = sim.sim.state.pos;
    // From the time, which is the one number that differs between a fresh world and the
    // same world rebuilt — and they should not land in the same places twice.
    let mut rng = sail_core::rng::Pcg32::new((sim.sim.state.time * 1_000.0) as u64 ^ 0x9e37_79b9);
    let mut put = 0;
    for _ in 0..HOW_MANY * 40 {
        if put >= HOW_MANY {
            break;
        }
        let a = rng.next_f64() * std::f64::consts::TAU;
        let r = SCATTERED_WITHIN.0 + rng.next_f64() * (SCATTERED_WITHIN.1 - SCATTERED_WITHIN.0);
        let at = here + DVec2::new(a.sin(), a.cos()) * r;
        if land.depth(at) > 6.0 {
            flotsam.adrift.push(at);
            put += 1;
        }
    }
    info!("flotsam: {put} casks adrift within a mile and a half");
}

/// The drift, and the string.
pub fn drift(sim: Res<Sim>, time: Res<Time>, mut flotsam: ResMut<Flotsam>) {
    let dt = time.delta_secs_f64();
    let now = sim.sim.state.time;
    for at in flotsam.adrift.iter_mut() {
        let breeze = sim.sim.wind.sample(*at, now);
        *at += breeze * LEEWAY * dt;
    }
    // **The string.** Each one is pulled up to the length of its painter behind the one
    // ahead, and the first behind a point astern of her: no springs, no masses, just a
    // rope that is either slack or tight. A towline is the one piece of rigging where
    // that is the whole truth.
    let st = &sim.sim.state;
    let along = DVec2::new(st.heading.sin(), st.heading.cos());
    let mut lead = st.pos - along * ASTERN;
    for cask in flotsam.astern.iter_mut() {
        let off = *cask - lead;
        let span = off.length();
        if span > PAINTER {
            *cask = lead + off / span * PAINTER;
        }
        lead = *cask;
    }
}

/// Keep one entity per cask, wherever it happens to be.
pub fn dress(
    mut commands: Commands,
    flotsam: Res<Flotsam>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    origin: Res<crate::frame::RenderOrigin>,
    casks: Query<Entity, With<Cask>>,
) {
    let want = flotsam.adrift.len() + flotsam.astern.len();
    let have = casks.iter().count();
    if want == have {
        return;
    }
    for entity in &casks {
        commands.entity(entity).despawn();
    }
    // A cask is a cask: oak, two hoops' worth of grime, and it floats on its side.
    let mesh = meshes.add(Cylinder::new(0.29, 0.86));
    let paint = materials.add(StandardMaterial {
        base_color: Color::srgb(0.38, 0.27, 0.16),
        perceptual_roughness: 0.92,
        ..default()
    });
    for (n, at) in flotsam
        .adrift
        .iter()
        .chain(flotsam.astern.iter())
        .enumerate()
    {
        commands.spawn((
            Cask(n),
            crate::course::Floating {
                at: *at,
                // Half out of the water, which is how a full cask floats and why you can
                // see one at a hundred metres in a lump of sea.
                lift: 0.12,
            },
            Mesh3d(mesh.clone()),
            MeshMaterial3d(paint.clone()),
            // On its side: the axis is across her, so a string of them reads as a string.
            Transform::from_translation(origin.to_bevy(*at, 0.0))
                .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
        ));
    }
}

/// And move them, every frame, because they are all drifting or being towed.
pub fn float(flotsam: Res<Flotsam>, mut casks: Query<(&Cask, &mut crate::course::Floating)>) {
    let all: Vec<DVec2> = flotsam
        .adrift
        .iter()
        .chain(flotsam.astern.iter())
        .copied()
        .collect();
    for (cask, mut float) in &mut casks {
        if let Some(at) = all.get(cask.0) {
            float.at = *at;
        }
    }
}

#[cfg(test)]
mod the_string {
    use super::*;

    /// **The painter is either tight or slack**, and a tight one is exactly its length.
    ///
    /// Which is the whole of the towing model, and the thing that makes a string of
    /// casks follow her round a headland rather than cut the corner onto it.
    #[test]
    fn each_cask_rides_a_painter_behind_the_one_ahead() {
        let mut astern = [
            DVec2::new(40.0, 0.0),
            DVec2::new(80.0, 0.0),
            DVec2::new(120.0, 0.0),
        ];
        let mut lead = DVec2::ZERO;
        for cask in astern.iter_mut() {
            let off = *cask - lead;
            let span = off.length();
            if span > PAINTER {
                *cask = lead + off / span * PAINTER;
            }
            lead = *cask;
        }
        for (i, pair) in [DVec2::ZERO]
            .iter()
            .chain(astern.iter())
            .collect::<Vec<_>>()
            .windows(2)
            .enumerate()
        {
            let span = (*pair[1] - *pair[0]).length();
            assert!(
                (span - PAINTER).abs() < 1e-6,
                "cask {i} rides {span:.1} m behind the one ahead, not {PAINTER}"
            );
        }
    }

    /// Hooking one takes it out of the water and puts it on the end of the string.
    #[test]
    fn a_hooked_cask_leaves_the_water() {
        let mut flotsam = Flotsam {
            adrift: vec![DVec2::new(10.0, 0.0), DVec2::new(-30.0, 5.0)],
            astern: Vec::new(),
        };
        let near = flotsam
            .nearest(DVec2::new(12.0, 0.0))
            .expect("one alongside");
        assert_eq!(near, DVec2::new(10.0, 0.0), "the nearest is the nearest");
        flotsam.hooked(near);
        assert_eq!(flotsam.adrift.len(), 1);
        assert_eq!(flotsam.astern, [DVec2::new(10.0, 0.0)]);
    }
}
