//! Her own anchorage: a platform moored in it, and what is piled on it.
//!
//! **This game does not save at sea and it is not going to.** A passage is an evening,
//! the clock runs at one to one, and a boat that can be put down mid-gale and picked up
//! next week is a boat nothing can go wrong to. What persists is not where she is — it is
//! what you have brought home.
//!
//! **The platform is wherever you are sailing.** Not because it sails — it is a raft
//! with a hut on it and it goes nowhere — but because it is the one thing in this game
//! that is *yours* rather than the sea's. Whatever water she opens in, the platform is
//! moored at the nearest town to where she starts, and the store on it is the store you
//! left there. In somebody else's islands it is also the way out: cast off from it and
//! you are back in your own, with everything that was aboard.
//!
//! Home is a platform: a raft of a thing moored in an anchorage of the archipelago,
//! with a hut on it and the casks you have fished out of the sea stacked where you can
//! see them from the water. It is the second half of every mechanic this game already
//! has. Casks adrift were collected and handed over at any anchorage, which made them a
//! number; the catch filled a creel and the creel emptied into nothing. Both of them have
//! somewhere to go now, and the pile is *drawn*, because a store you cannot see from the
//! helm is a figure on a panel and this project has an opinion about those.
//!
//! **Which also means the sea has to stay the same sea.** The archipelago is a pure
//! function of its seed, and a desk build has always drawn a new one from the clock every
//! launch — which is right for a simulator you sit down at and wrong the moment anything
//! belongs to you. So the first thing the file below records is the seed, and from the
//! second launch onwards the water, the islands and the names are the ones your platform
//! is moored in. `SAIL_SEED=` still overrides it, and that is how a new world is asked
//! for.
//!
//! The file is lines of text on purpose: a save a person can read, diff and delete is one
//! they can also recover from, and nothing in here is worth a binary format or a crate to
//! parse it.

use bevy::prelude::*;
use bevy_egui::egui;
use glam::DVec2;

use crate::sim::Sim;

/// Her own anchorage: the platform, the store on it, and the chandler's list.
///
/// What this module does in a frame, in one place — see `stages.rs` for why the
/// schedule is named sets rather than one long chain.
pub fn plugin(app: &mut App) {
    use crate::stages::{Deck, Paint};
    app.init_resource::<Berth>();
    app.add_systems(
        Update,
        (moor, build, land, the_lamp_at_dusk, keep)
            .chain()
            .in_set(Deck::Weather),
    );
    // **In `Last`, not in `Update`.** The engine checks for a request to quit after the
    // main schedule has run, so a system that wants to catch one has to come after
    // everything that can send it — in `Update` it is a coin toss whether the reader runs
    // before the writer, and losing that toss is losing the last few casks.
    app.add_systems(Last, keep_on_the_way_out);
    app.add_systems(
        bevy_egui::EguiPrimaryContextPass,
        chandler.in_set(Paint::Instruments),
    );
}

/// How close to the platform she has to be, metres, and how slowly moving.
///
/// The same numbers as handing casks over a rail anywhere else, because it is the same
/// act: see `barrels.rs`, which this took them from.
const ALONGSIDE: f64 = 70.0;
const WORKING_SPEED_KN: f64 = 2.0;
/// Seconds a cask takes to go from the painter onto the platform. One at a time, because
/// that is what hands do.
const A_CASK_TAKES: f64 = 3.0;
/// And how often the file is written when something has changed. Not every cask: a disk
/// write per cask is three a minute for a mechanic that loses nothing by being a few
/// seconds behind.
const WRITE_EVERY: f32 = 4.0;

/// How big the raft is, across, metres.
///
/// Nine metres of deck with a hut on it. Named because the hull now has to know: a thing
/// you can moor to is a thing you can hit, and `contact.rs` wants the number the mesh is
/// built from rather than one typed twice.
pub const RAFT: f32 = 9.0;

/// What is on the platform.
///
/// Counts rather than a list of objects, because that is what a store *is* — the fish you
/// landed last Tuesday is not a fish any more, it is three kilos of the catch.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stores {
    /// Casks fished out of the water and landed here.
    pub casks: u32,
    /// Fish out of the creel, and what they weighed.
    pub fish: u32,
    pub catch_kg: f64,
}

/// What has been built on the platform, and what it costs.
///
/// **A store with nothing to spend it on is a counter with a longer name.** This is the
/// other half: three things that can be built out there, each paid for in what the sea
/// gives up, each one *drawn* on the raft, and each one doing a single thing you can feel
/// from the water rather than read off a panel.
///
/// Priced in casks and in fish because those are what there is. A chandler in an
/// archipelago takes salvage and he takes the catch; coin is a thing this game does not
/// have yet, and inventing one to stand between the two halves of a trade would be a
/// worse answer than the trade itself.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fitting {
    /// A boom and a tackle: casks come off the painter three times as fast.
    Derrick,
    /// Where the catch hangs. It is the catch, drawn — a season's fishing you can see
    /// from the cockpit instead of a number in a store.
    Rack,
    /// A lamp on a pole, lit when the light goes. Finding your own platform in poor
    /// visibility is the thing it is for, and it is the first piece of the night.
    Lamp,
}

impl Fitting {
    /// All of them, in the order a chandler would read them out.
    pub const ALL: [Fitting; 3] = [Fitting::Derrick, Fitting::Rack, Fitting::Lamp];

    /// What it is called, and what it does, in the fewest words that are still true.
    pub fn name(self) -> &'static str {
        match self {
            Fitting::Derrick => "derrick",
            Fitting::Rack => "drying rack",
            Fitting::Lamp => "lamp",
        }
    }

    pub fn does(self) -> &'static str {
        match self {
            Fitting::Derrick => "casks come aboard three times as fast",
            Fitting::Rack => "the catch hangs where you can see it",
            Fitting::Lamp => "lit at dusk — you can find her in poor visibility",
        }
    }

    /// What it costs: casks, and fish.
    pub fn costs(self) -> (u32, u32) {
        match self {
            Fitting::Derrick => (6, 0),
            Fitting::Rack => (2, 6),
            Fitting::Lamp => (9, 0),
        }
    }

    /// The word in the file. Short, lower case, and never reused.
    pub fn key(self) -> &'static str {
        match self {
            Fitting::Derrick => "derrick",
            Fitting::Rack => "rack",
            Fitting::Lamp => "lamp",
        }
    }

    pub fn of(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.key() == key)
    }
}

/// The anchorage, the platform in it, and the seed of the water it all sits in.
#[derive(Resource, Clone, Debug, Default)]
pub struct Home {
    /// The sea this belongs to. Everything else here is meaningless in another one.
    pub sea: u64,
    /// Where the platform lies, in the archipelago's own coordinates.
    pub moored: Option<DVec2>,
    /// And the name of the anchorage it is in, for saying where you are going.
    pub harbour: String,
    pub stores: Stores,
    /// What has been built out there.
    pub built: Vec<Fitting>,
    /// Whether anything has changed since the last write.
    pub unsaved: bool,
}

impl Home {
    pub fn has(&self, fitting: Fitting) -> bool {
        self.built.contains(&fitting)
    }

    /// Whether the store can pay for it. Its own function because it is the one piece of
    /// arithmetic in a trade, and both the panel and the test have to use the same one.
    pub fn affords(&self, fitting: Fitting) -> bool {
        let (casks, fish) = fitting.costs();
        !self.has(fitting) && self.stores.casks >= casks && self.stores.fish >= fish
    }

    /// Pay for it and put it on the list. Nothing happens if it cannot be paid for, which
    /// is the only failure mode a chandler has.
    pub fn buy(&mut self, fitting: Fitting) -> bool {
        if !self.affords(fitting) {
            return false;
        }
        let (casks, fish) = fitting.costs();
        self.stores.casks -= casks;
        self.stores.fish -= fish;
        self.built.push(fitting);
        self.unsaved = true;
        true
    }
}

/// The version stamp at the top of the file.
///
/// An old save has to fail *politely* — read what it understands, ignore what it does
/// not, and never crash the game of somebody who upgraded. A file from the future is left
/// alone entirely: it belongs to a build that knows more than this one.
const FORMAT: u32 = 1;

impl Home {
    /// The file, if there is one and it makes sense.
    pub fn read() -> Option<Self> {
        Self::parse(&crate::kept::read("home")?)
    }

    /// Write it where it will be found again — see `kept.rs` for how and where.
    pub fn write(&self) -> Option<()> {
        crate::kept::write("home", &self.text())
    }

    /// The file's contents. One fact a line, in the order somebody would want to read
    /// them.
    pub fn text(&self) -> String {
        let mut out = format!("sail home {FORMAT}\nsea {}\n", self.sea);
        if let Some(at) = self.moored {
            out.push_str(&format!("moored {:.1} {:.1}\n", at.x, at.y));
        }
        if !self.harbour.is_empty() {
            out.push_str(&format!("harbour {}\n", self.harbour));
        }
        out.push_str(&format!("casks {}\n", self.stores.casks));
        out.push_str(&format!("fish {}\n", self.stores.fish));
        out.push_str(&format!("catch {:.2}\n", self.stores.catch_kg));
        for fitting in &self.built {
            out.push_str(&format!("built {}\n", fitting.key()));
        }
        out
    }

    /// And back again. A line it does not know is skipped rather than fatal.
    pub fn parse(text: &str) -> Option<Self> {
        if !crate::kept::stamped(text, "home", FORMAT) {
            return None;
        }
        let mut home = Self::default();
        for (key, value) in crate::kept::lines(text) {
            match key {
                "sea" => home.sea = value.trim().parse().unwrap_or(0),
                "moored" => {
                    if let Some((x, y)) = value.trim().split_once(' ') {
                        if let (Ok(x), Ok(y)) = (x.parse::<f64>(), y.parse::<f64>()) {
                            home.moored = Some(DVec2::new(x, y));
                        }
                    }
                }
                "harbour" => home.harbour = value.trim().to_string(),
                "casks" => home.stores.casks = value.trim().parse().unwrap_or(0),
                "fish" => home.stores.fish = value.trim().parse().unwrap_or(0),
                "catch" => home.stores.catch_kg = value.trim().parse().unwrap_or(0.0),
                "built" => {
                    // A fitting this build has never heard of is skipped rather than
                    // guessed at, the same as any other unknown line.
                    if let Some(fitting) = Fitting::of(value.trim()) {
                        if !home.built.contains(&fitting) {
                            home.built.push(fitting);
                        }
                    }
                }
                _ => {}
            }
        }
        Some(home)
    }
}

/// Where the platform lies **in the water we are in now**, and whether that water is
/// ours.
///
/// **There is one platform and it is in whatever water she is.** The first version
/// moored it in one archipelago and left it there, which lasted exactly until somebody
/// joined a friend's game: the raft floats, a joined world despawns everything that
/// floats, and a season's casks went with the old islands and did not come back.
///
/// The answer is not to pin it down harder. A platform that only exists in one sea is a
/// reason not to visit anybody. So it is moored again wherever she opens — at the
/// anchorage she is lying in, or the nearest town if she is at sea — the store on it is
/// untouched, and when the water is not yours the raft is also the way out: cast off from
/// it and you are back in your own islands with everything that was aboard.
///
/// Kept apart from [`Home`] because they answer different questions: `Home` is the berth
/// in *your* sea and goes in the file, this is where the raft is at this moment and is
/// forgotten when the program closes.
#[derive(Resource, Clone, Debug, Default)]
pub struct Berth {
    /// Which archipelago this berth is a position in.
    pub sea: u64,
    pub at: Option<DVec2>,
    pub harbour: String,
    /// Whether this is your own water. False while you are somebody's guest, and what
    /// the way home hangs off.
    pub own: bool,
}

/// Pick the berth, once per sea.
///
/// The nearest anchorage to where she first came to — which on a new sea is the clear
/// water the day was chosen for, so the platform ends up a mile or two from the start and
/// in sight of it. Any anchorage would do; *the same* anchorage every time is the whole
/// requirement.
pub fn moor(
    mut home: ResMut<Home>,
    mut berth: ResMut<Berth>,
    sea: Res<crate::islands::Sea>,
    sim: Res<Sim>,
    course: Res<crate::course::Course>,
) {
    if berth.at.is_some() && berth.sea == sea.0 {
        return;
    }
    let from = sim.sim.state.pos;
    let own = sea.0 == home.sea;
    // `SAIL_PLATFORM=here` moors it where she is, which is the only way to look at a
    // thing that is otherwise two miles away through a day's sailing. A drawing knob and
    // nothing else: it never reaches the file — see `keep`.
    if crate::knob::var("SAIL_PLATFORM").as_deref() == Ok("here") {
        // Fine on the port bow, sixty metres off: dead ahead is behind her own mainsail
        // from the chase camera, and abeam is outside its field.
        let heading = sim.sim.state.heading - 0.6;
        *berth = Berth {
            sea: sea.0,
            at: Some(from + DVec2::new(heading.sin(), heading.cos()) * 60.0),
            harbour: "alongside".into(),
            own,
        };
        info!("the platform is moored alongside, for looking at");
        return;
    }
    let Some(picked) = berth_for(
        own,
        home.moored.map(|at| (at, home.harbour.clone())),
        from,
        &course.harbours,
    ) else {
        return;
    };
    if picked.fresh {
        info!(
            "the platform lies at {} — {:.1} nm off{}",
            picked.harbour,
            (picked.at - from).length() / 1_852.0,
            if own { "" } else { ", in borrowed water" }
        );
    }
    *berth = Berth {
        sea: sea.0,
        at: Some(picked.at),
        harbour: picked.harbour.clone(),
        own,
    };
    // Only your own berth is written down. A berth in somebody else's islands is a
    // position in a world you are visiting, and writing it to the file would move your
    // platform to a point in your own sea that has nothing at it.
    if own && picked.fresh {
        home.harbour = picked.harbour;
        home.moored = Some(picked.at);
        home.unsaved = true;
    }
}

/// The berth the platform takes in this water, and whether it is a new one.
pub struct Picked {
    pub at: DVec2,
    pub harbour: String,
    /// True when it was chosen now rather than read out of the file — which is also the
    /// only case worth writing down or announcing.
    pub fresh: bool,
}

/// Where the platform goes, given whose water this is.
///
/// **In your own sea it goes back where it was**; anywhere else it is moored at the
/// nearest town to where she opened, which is the anchorage she is lying in if she opened
/// in one. Its own function because the rule is the whole of the mechanic and a test
/// should be able to read it without a world — and because the one thing that must never
/// happen is a guest berth being written into the file as home.
pub fn berth_for(
    own: bool,
    remembered: Option<(DVec2, String)>,
    from: DVec2,
    harbours: &[sail_core::land::Harbour],
) -> Option<Picked> {
    if own {
        if let Some((at, harbour)) = remembered {
            return Some(Picked {
                at,
                harbour,
                fresh: false,
            });
        }
    }
    let found = harbours
        .iter()
        .min_by(|a, b| (a.at - from).length().total_cmp(&(b.at - from).length()))?;
    Some(Picked {
        at: found.at,
        harbour: found.name.clone(),
        fresh: true,
    })
}

/// Whether this is a place and a moment to be handing things over the rail.
///
/// One statement, because the thing on screen and the thing the test uses have to be the
/// same measurement — which is the lesson this repository keeps relearning. It came from
/// `barrels.rs`, where the same rule applied at any anchorage before there was a platform
/// to take them to.
pub fn alongside(here: DVec2, stw_kn: f64, berth: DVec2) -> bool {
    stw_kn.abs() < WORKING_SPEED_KN && (berth - here).length() < ALONGSIDE
}

/// The platform itself, and the pile on it.
#[derive(Component)]
pub struct Platform;

/// The lamp, which is the one fitting that is not always on.
#[derive(Component)]
pub struct Lamp;

/// Something built on the platform, drawn. Despawned and rebuilt whole when the list
/// changes, which is a few times in the life of a world.
#[derive(Component)]
pub struct Fitted;

/// One cask of the store, drawn.
///
/// The number is which of the pile it is. Nothing reads it — the pile is rebuilt whole
/// when the count changes — and it is here because a stack of anonymous entities is a
/// stack nobody can debug: `Stacked(13)` in an inspector is the thirteenth cask.
#[derive(Component)]
pub struct Stacked(#[allow(dead_code)] pub u32);

/// How many casks are drawn before the rest are "below".
///
/// Twenty-four is two courses of twelve, which is a pile you can read at a glance. Past
/// that a store is a number again whatever we draw, and a raft under two hundred barrels
/// is a raft on the bottom.
const SHOWS: u32 = 24;

/// Build her the first time there is a berth, and keep the pile in step after that.
#[allow(clippy::too_many_arguments)]
pub fn build(
    mut commands: Commands,
    home: Res<Home>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    origin: Res<crate::frame::RenderOrigin>,
    platform: Query<Entity, With<Platform>>,
    stack: Query<(Entity, &Stacked)>,
    fitted: Query<Entity, With<Fitted>>,
    berth: Res<Berth>,
    mut standing: Local<Option<(Entity, DVec2)>>,
    mut fittings: Local<usize>,
) {
    let Some(at) = berth.at else {
        return;
    };
    // **Built when there is no raft, rather than once.**
    //
    // It used to be a `bool`, and the bool was wrong the moment anything else despawned
    // the platform — which the joined world does, because it takes out everything that
    // floats. The raft went and the latch said it was still there, so a player who
    // visited a friend had no platform for the rest of the session. Keeping the entity
    // answers the real question, which is *is she still in the water*, and the berth
    // beside it answers the other one: has she moved to another sea.
    let afloat = standing
        .map(|(entity, was)| platform.get(entity).is_ok() && (was - at).length() < 1.0)
        .unwrap_or(false);
    if !afloat {
        for entity in &platform {
            commands.entity(entity).despawn();
        }
        *fittings = usize::MAX;
        // The raft: a deck of planks on two logs, a hut at one end, and a ladder over the
        // side where a boat comes alongside. Generated like everything else here — it is
        // six boxes, and six boxes that are the right size read better than a model that
        // is the wrong one.
        let timber = materials.add(StandardMaterial {
            base_color: Color::srgb(0.56, 0.45, 0.31),
            perceptual_roughness: 0.95,
            ..default()
        });
        let tarred = materials.add(StandardMaterial {
            base_color: Color::srgb(0.17, 0.16, 0.15),
            perceptual_roughness: 0.9,
            ..default()
        });
        let deck = meshes.add(Cuboid::new(RAFT, 0.5, RAFT * 0.67));
        let float = meshes.add(Cylinder::new(0.8, 8.4));
        let hut = meshes.add(Cuboid::new(2.8, 2.1, 2.4));
        let roof = meshes.add(Cuboid::new(3.2, 0.18, 2.8));
        let raft = commands
            .spawn((
                Platform,
                crate::course::Floating { at, lift: 0.0 },
                Mesh3d(deck.clone()),
                MeshMaterial3d(timber.clone()),
                Transform::from_translation(origin.to_bevy(at, 0.35)),
            ))
            .with_children(|raft| {
                for side in [-1.0_f32, 1.0] {
                    raft.spawn((
                        Mesh3d(float.clone()),
                        MeshMaterial3d(tarred.clone()),
                        Transform::from_xyz(0.0, -0.55, side * 2.2)
                            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                    ));
                }
                raft.spawn((
                    Mesh3d(hut),
                    MeshMaterial3d(timber.clone()),
                    Transform::from_xyz(-2.6, 1.3, 0.0),
                ));
                raft.spawn((
                    Mesh3d(roof),
                    MeshMaterial3d(tarred.clone()),
                    Transform::from_xyz(-2.6, 2.45, 0.0),
                ));
            })
            .id();
        *standing = Some((raft, at));
        info!(
            "platform moored at {}{}",
            berth.harbour,
            if berth.own { "" } else { " — borrowed water" }
        );
    }

    // What has been built out there. Same rule as the pile: rebuilt when the list it is
    // drawing changes, and not otherwise.
    if *fittings != home.built.len() {
        *fittings = home.built.len();
        for entity in &fitted {
            commands.entity(entity).despawn();
        }
        if let Ok(raft) = platform.single() {
            let timber = materials.add(StandardMaterial {
                base_color: Color::srgb(0.5, 0.4, 0.28),
                perceptual_roughness: 0.95,
                ..default()
            });
            let iron = materials.add(StandardMaterial {
                base_color: Color::srgb(0.22, 0.21, 0.2),
                perceptual_roughness: 0.75,
                ..default()
            });
            let spar = meshes.add(Cylinder::new(0.11, 4.4));
            let post = meshes.add(Cylinder::new(0.13, 3.0));
            let beam = meshes.add(Cuboid::new(3.4, 0.1, 0.1));
            let fish = meshes.add(Sphere::new(0.16));
            let silver = materials.add(StandardMaterial {
                base_color: Color::srgb(0.62, 0.66, 0.68),
                perceptual_roughness: 0.4,
                ..default()
            });
            let glass = materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.86, 0.55),
                emissive: LinearRgba::new(6.0, 4.4, 1.8, 1.0),
                ..default()
            });
            commands.entity(raft).with_children(|raft| {
                for fitting in &home.built {
                    match fitting {
                        // A post at the outboard corner with a boom slung off it, angled
                        // out over the water where a boat comes alongside.
                        Fitting::Derrick => {
                            raft.spawn((
                                Fitted,
                                Mesh3d(post.clone()),
                                MeshMaterial3d(timber.clone()),
                                Transform::from_xyz(2.8, 1.7, -2.0),
                            ));
                            raft.spawn((
                                Fitted,
                                Mesh3d(spar.clone()),
                                MeshMaterial3d(timber.clone()),
                                Transform::from_xyz(3.9, 2.8, -3.2).with_rotation(
                                    Quat::from_rotation_x(0.9) * Quat::from_rotation_z(0.35),
                                ),
                            ));
                        }
                        // Two uprights and a rail, with the catch on it.
                        Fitting::Rack => {
                            for end in [-1.0_f32, 1.0] {
                                raft.spawn((
                                    Fitted,
                                    Mesh3d(post.clone()),
                                    MeshMaterial3d(timber.clone()),
                                    Transform::from_xyz(-0.4, 1.7, end * 1.6)
                                        .with_scale(Vec3::new(0.7, 0.75, 0.7)),
                                ));
                            }
                            raft.spawn((
                                Fitted,
                                Mesh3d(beam.clone()),
                                MeshMaterial3d(timber.clone()),
                                Transform::from_xyz(-0.4, 2.7, 0.0).with_rotation(
                                    Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
                                ),
                            ));
                            // **The catch, hung on it.** A dozen at most: past that a rack
                            // is a wall of fish, and the point of the thing is that a
                            // season's fishing is a sight rather than a figure.
                            for n in 0..home.stores.fish.min(12) {
                                let along = -1.4 + n as f32 * 0.26;
                                raft.spawn((
                                    Fitted,
                                    Mesh3d(fish.clone()),
                                    MeshMaterial3d(silver.clone()),
                                    Transform::from_xyz(-0.4, 2.35, along)
                                        .with_scale(Vec3::new(0.7, 1.9, 0.5)),
                                ));
                            }
                        }
                        // A pole with a lamp on it, and the light it actually throws.
                        Fitting::Lamp => {
                            raft.spawn((
                                Fitted,
                                Mesh3d(post.clone()),
                                MeshMaterial3d(iron.clone()),
                                Transform::from_xyz(-3.9, 2.2, 0.0)
                                    .with_scale(Vec3::new(0.6, 1.3, 0.6)),
                            ));
                            raft.spawn((
                                Fitted,
                                Lamp,
                                Mesh3d(meshes.add(Sphere::new(0.26))),
                                MeshMaterial3d(glass.clone()),
                                Transform::from_xyz(-3.9, 4.3, 0.0),
                            ));
                            raft.spawn((
                                Fitted,
                                Lamp,
                                PointLight {
                                    // Warm, and not very far: a platform lamp is a thing
                                    // you steer for, not a floodlight.
                                    color: Color::srgb(1.0, 0.82, 0.52),
                                    intensity: 400_000.0,
                                    range: 120.0,
                                    shadow_maps_enabled: false,
                                    ..default()
                                },
                                Transform::from_xyz(-3.9, 4.3, 0.0),
                            ));
                        }
                    }
                }
            });
        }
    }

    // The pile. Rebuilt only when the count it is drawing has changed, which is a few
    // times an evening.
    let want = home.stores.casks.min(SHOWS);
    let have = stack.iter().count() as u32;
    if want == have {
        return;
    }
    // A fish landed while the rack is up changes the rack too, and the cheapest way to
    // say that is to make the next landing rebuild both.
    if home.has(Fitting::Rack) {
        *fittings = usize::MAX;
    }
    let Ok(raft) = platform.single() else {
        return;
    };
    for (entity, _) in &stack {
        commands.entity(entity).despawn();
    }
    let cask = meshes.add(Cylinder::new(0.29, 0.86));
    let oak = materials.add(StandardMaterial {
        base_color: Color::srgb(0.38, 0.27, 0.16),
        perceptual_roughness: 0.92,
        ..default()
    });
    commands.entity(raft).with_children(|raft| {
        for n in 0..want {
            // Two courses of six a side, stacked on their sides the way a cooper's yard
            // stacks them: the second course sits in the hollows of the first.
            let course = n / 12;
            let along = (n % 12) / 2;
            let side = if n % 2 == 0 { 1.0 } else { -1.0 };
            let lean = if course == 1 { 0.3 } else { 0.0 };
            raft.spawn((
                Stacked(n),
                Mesh3d(cask.clone()),
                MeshMaterial3d(oak.clone()),
                Transform::from_xyz(
                    1.1 + along as f32 * 0.95,
                    0.55 + course as f32 * 0.52,
                    side * (1.0 - lean),
                )
                .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            ));
        }
    });
}

/// Landing what she has brought: casks off the painter, and the creel into the store.
///
/// Alongside and slow, which is the rule everywhere else in this game that something is
/// handed over a rail, and the casks come off one at a time so that a string emptying
/// reads as cargo rather than as a counter resetting.
#[allow(clippy::too_many_arguments)]
pub fn land(
    mut home: ResMut<Home>,
    berth: Res<Berth>,
    sim: Res<Sim>,
    mut flotsam: ResMut<crate::barrels::Flotsam>,
    mut fishing: ResMut<crate::fishing::Fishing>,
    time: Res<Time>,
    sound: Res<crate::sound::SoundHandle>,
    mut passing: Local<f64>,
) {
    let Some(at) = berth.at else {
        return;
    };
    let here = sim.sim.state.pos;
    let knots = sail_core::units::ms_to_kn(sim.sim.telemetry.stw);
    if !alongside(here, knots, at) {
        *passing = 0.0;
        return;
    }
    // The catch first: a creel is emptied in one go, because it is one basket and the
    // hands that carry it make one trip.
    if !fishing.caught.is_empty() {
        let kg: f64 = fishing.caught.iter().map(|f| f.mass).sum();
        home.stores.fish += fishing.caught.len() as u32;
        home.stores.catch_kg += kg;
        info!(
            "the creel goes up: {} fish, {kg:.1} kg — {} in the store",
            fishing.caught.len(),
            home.stores.fish
        );
        fishing.caught.clear();
        home.unsaved = true;
        sound.0.strike_damped(320.0, 0.25, 0.5);
    }
    if flotsam.astern.is_empty() {
        return;
    }
    *passing += time.delta_secs_f64();
    // A derrick is a boom and a tackle, and what it buys is the thing it bought on every
    // quay that ever had one: you are not lifting the cask, you are swinging it.
    let takes = if home.has(Fitting::Derrick) {
        A_CASK_TAKES / 3.0
    } else {
        A_CASK_TAKES
    };
    if *passing < takes {
        return;
    }
    *passing = 0.0;
    flotsam.astern.remove(0);
    flotsam.landed += 1;
    home.stores.casks += 1;
    home.unsaved = true;
    sound.0.strike_damped(190.0, 0.3, 0.75);
    info!(
        "cask aboard the platform — {} in the store, {} still on the painter",
        home.stores.casks,
        flotsam.astern.len()
    );
}

/// Light it when the light goes, and not before.
///
/// **A lamp burning at nine in the morning is a lamp nobody will ever steer by.** The
/// thing it is for is the half hour either side of dusk and the days when the visibility
/// shuts down, so it follows the sun: full from the moment the sun is on the horizon,
/// out well before anybody would call it daylight, and fading between the two rather than
/// snapping, because that is what the eye does with a light at dusk anyway.
pub fn the_lamp_at_dusk(
    sky: Res<crate::sky::Sky>,
    mut lights: Query<&mut PointLight, With<Lamp>>,
    mut glass: Query<&mut Visibility, (With<Lamp>, With<Mesh3d>)>,
) {
    let lit = lit_at(sky.elevation().to_degrees());
    for mut light in &mut lights {
        light.intensity = 400_000.0 * lit;
    }
    for mut visible in &mut glass {
        *visible = if lit > 0.02 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// How bright the lamp is with the sun that many degrees above the horizon.
///
/// Its own function so the curve can be read without a sky: out in daylight, full once
/// the sun is down, and the fade between them over the last eight degrees — which is
/// about the half hour before sunset at this latitude.
fn lit_at(elevation_deg: f32) -> f32 {
    ((8.0 - elevation_deg) / 8.0).clamp(0.0, 1.0)
}

/// The chandler's list, while she is lying alongside.
///
/// **It is only there when she is**, which is the whole of why it is a panel and not a
/// menu: trading is a place you go to rather than a screen you open, and a list you can
/// read from the middle of the fairway is a shop that has come to you.
///
/// What is on it is what the store can pay for. A thing already built is struck through
/// rather than hidden — you looked at it last week and wondered what it cost, and a shop
/// whose shelves change when you are not looking is a shop you cannot plan against.
pub fn chandler(
    mut contexts: bevy_egui::EguiContexts,
    mut home: ResMut<Home>,
    berth: Res<Berth>,
    mut net: ResMut<crate::net::Net>,
    weather: Res<crate::weather::Weather>,
    sim: Res<Sim>,
) -> Result {
    let Some(at) = berth.at else {
        return Ok(());
    };
    let knots = sail_core::units::ms_to_kn(sim.sim.telemetry.stw);
    if !alongside(sim.sim.state.pos, knots, at) {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let ink = egui::Color32::from_rgb(230, 198, 120);
    let quiet = egui::Color32::from_rgb(150, 150, 130);
    let frame = egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(14, 18, 24, 236))
        .stroke(egui::Stroke::new(1.0, ink))
        .inner_margin(egui::Margin::symmetric(16, 12));
    let mut buying: Option<Fitting> = None;
    let mut casting_off = false;
    egui::Area::new("platform".into())
        .anchor(egui::Align2::LEFT_CENTER, egui::vec2(12.0, 0.0))
        .show(ctx, |ui| {
            frame.show(ui, |ui| {
                ui.set_width(300.0);
                ui.label(
                    egui::RichText::new(format!("THE PLATFORM — {}", berth.harbour.to_uppercase()))
                        .monospace()
                        .size(12.0)
                        .color(ink),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "{} casks · {} fish, {:.0} kg",
                        home.stores.casks, home.stores.fish, home.stores.catch_kg
                    ))
                    .monospace()
                    .size(11.0)
                    .color(quiet),
                );
                ui.add_space(6.0);
                for fitting in Fitting::ALL {
                    let (casks, fish) = fitting.costs();
                    let price = match (casks, fish) {
                        (c, 0) => format!("{c} casks"),
                        (0, f) => format!("{f} fish"),
                        (c, f) => format!("{c} casks, {f} fish"),
                    };
                    if home.has(fitting) {
                        ui.label(
                            egui::RichText::new(format!("{} — built", fitting.name()))
                                .monospace()
                                .size(11.0)
                                .color(quiet)
                                .strikethrough(),
                        );
                        continue;
                    }
                    let can = home.affords(fitting);
                    let line = format!("{} — {price}", fitting.name());
                    let button = ui.add_enabled(
                        can,
                        egui::Button::new(
                            egui::RichText::new(line)
                                .monospace()
                                .size(11.0)
                                .color(if can { ink } else { quiet }),
                        ),
                    );
                    if button.on_hover_text(fitting.does()).clicked() {
                        buying = Some(fitting);
                    }
                }
                // **And the way home.**
                //
                // The platform is in this water because she is; in somebody else's
                // islands it is also the only way back to your own, and the store comes
                // with you because the store is yours and not the sea's. A line rather
                // than a key: it is a thing you do at one place, standing still, and the
                // key list on this boat is for things you do *to the boat*.
                if !berth.own {
                    ui.add_space(6.0);
                    if ui
                        .button(
                            egui::RichText::new("cast off for your own water")
                                .monospace()
                                .size(11.0)
                                .color(ink),
                        )
                        .on_hover_text("back to your own islands, with everything aboard")
                        .clicked()
                    {
                        casting_off = true;
                    }
                }
            });
        });
    if let Some(fitting) = buying {
        if home.buy(fitting) {
            info!("built on the platform: {}", fitting.name());
        }
    }
    if casting_off {
        // The same handover a join uses, pointed the other way: your seed, and the day
        // you are actually sailing rather than the one it opened on. `net::rebuild` does
        // the rest — the islands, the harbours, the chart that no longer knows where
        // anybody is.
        net.rebuild = Some(sail_net::World {
            seed: home.sea,
            tws: weather.tws as f32,
            wind_from: weather.wind_from as f32,
            fetch: weather.fetch as f32,
            start_hour: weather.start_hour as f32,
            cover: weather.cover as f32,
        });
        info!("casting off for her own water: seed {}", home.sea);
    }
    Ok(())
}

/// Write it down, when there is something to write and not more often than that.
pub fn keep(mut home: ResMut<Home>, time: Res<Time>, mut since: Local<f32>) {
    if !home.unsaved || looking_only() {
        return;
    }
    *since += time.delta_secs();
    if *since < WRITE_EVERY {
        return;
    }
    *since = 0.0;
    if home.write().is_some() {
        home.unsaved = false;
    }
}

/// Whether the platform is where it is only to be looked at.
///
/// `SAIL_PLATFORM=here` moves the raft to the port bow so it can be photographed without
/// a two-mile sail, and a berth looked at must not become a berth moved: with the knob on
/// nothing is written, because the first fitting bought or cask landed would otherwise
/// put a position sixty metres off her bow into the file as your home — in a sea where
/// there is nothing there.
///
/// The reel does the same thing at its evening beat, for the same reason — the platform
/// has to be in the picture — so it counts as looking too. A trailer being filmed must not
/// move somebody's home.
fn looking_only() -> bool {
    crate::kept::staging()
}

/// And write it on the way out.
///
/// The timer above writes every few seconds, which loses up to that much when somebody
/// quits — three casks swung aboard and then the window closed is three casks gone, and
/// "it did not save" is the one bug a player never forgives. This catches both ways out
/// that the program knows about, the key and the window's own button; a kill still loses
/// the last few seconds, which is what the timer is for.
pub fn keep_on_the_way_out(mut quitting: MessageReader<AppExit>, home: Res<Home>) {
    if quitting.is_empty() {
        return;
    }
    quitting.clear();
    if home.unsaved && !looking_only() {
        home.write();
    }
}

#[cfg(test)]
mod what_is_kept {
    use super::*;

    fn a_home() -> Home {
        Home {
            sea: 0x15_1A_4D,
            moored: Some(DVec2::new(-1234.5, 678.25)),
            harbour: "Fuglhavn".into(),
            stores: Stores {
                casks: 17,
                fish: 9,
                catch_kg: 23.456,
            },
            built: vec![Fitting::Derrick],
            unsaved: true,
        }
    }

    /// **What goes out comes back.** The only thing this file has to do.
    #[test]
    fn it_reads_back_what_it_wrote() {
        let home = a_home();
        let back = Home::parse(&home.text()).expect("a file it just wrote");
        assert_eq!(back.sea, home.sea);
        assert_eq!(back.harbour, home.harbour);
        assert_eq!(back.stores.casks, home.stores.casks);
        assert_eq!(back.stores.fish, home.stores.fish);
        assert!((back.stores.catch_kg - home.stores.catch_kg).abs() < 0.01);
        let (was, is) = (home.moored.unwrap(), back.moored.unwrap());
        assert!((was - is).length() < 0.1, "the berth moved: {was} to {is}");
    }

    /// A file written by a build that knows more than this one is left alone rather than
    /// half-read: a platform that quietly loses its casks on a downgrade is worse than one
    /// that starts again.
    #[test]
    fn a_file_from_the_future_is_not_guessed_at() {
        let later = format!("sail home {}\nsea 7\ncasks 400\n", FORMAT + 1);
        assert!(Home::parse(&later).is_none());
    }

    /// And a line it has never heard of is skipped, so that adding a field later does not
    /// invalidate every save that came before it.
    #[test]
    fn it_skips_what_it_does_not_know() {
        let text = format!("sail home {FORMAT}\nsea 12\nrope 40\ncasks 3\n");
        let home = Home::parse(&text).expect("a file with a stranger in it");
        assert_eq!(home.sea, 12);
        assert_eq!(home.stores.casks, 3);
    }

    /// Nonsense is nonsense, and the game starts fresh rather than on a wrong sea.
    #[test]
    fn rubbish_is_refused() {
        assert!(Home::parse("").is_none());
        assert!(Home::parse("a text file that is not this one\n").is_none());
        assert!(Home::parse("sail home\n").is_none());
    }

    /// **Alongside and stopped, or it does not happen.**
    ///
    /// A barrel is a two-man lift over a rail; at four knots it is how you lose the
    /// barrel and a finger. And seventy metres is inside the anchorage rather than off
    /// the headland, so arriving is a thing you did rather than a line you crossed.
    ///
    /// This rule and this test came from `barrels.rs`, where they applied at any
    /// anchorage in the archipelago. The only thing that changed is that there is now one
    /// place they apply at, and it is yours.
    #[test]
    fn things_go_over_the_rail_alongside_and_slowly_or_not_at_all() {
        let berth = DVec2::new(1_000.0, 500.0);
        assert!(alongside(berth + DVec2::new(40.0, 0.0), 1.2, berth));
        assert!(
            !alongside(berth + DVec2::new(40.0, 0.0), 4.0, berth),
            "a cask went over the rail at four knots"
        );
        assert!(
            !alongside(berth + DVec2::new(400.0, 0.0), 1.0, berth),
            "a cask was handed across four cables of open water"
        );
        // Astern counts as moving too: she can be making two knots backwards off a berth
        // she has overshot, and a hand reaching over the rail then is the same hand.
        assert!(!alongside(berth, -3.0, berth), "she was going astern");
    }

    /// The seed is the first thing in the file because everything else is meaningless
    /// without it — a platform's berth is a position in *one* archipelago.
    #[test]
    fn the_sea_comes_first() {
        let text = a_home().text();
        let second = text.lines().nth(1).unwrap_or("");
        assert!(second.starts_with("sea "), "second line was {second:?}");
    }
}

#[cfg(test)]
mod what_can_be_built {
    use super::*;

    fn with(casks: u32, fish: u32) -> Home {
        Home {
            sea: 1,
            moored: Some(DVec2::ZERO),
            harbour: "Testvik".into(),
            stores: Stores {
                casks,
                fish,
                catch_kg: fish as f64 * 1.4,
            },
            built: Vec::new(),
            unsaved: false,
        }
    }

    /// **A store with nothing to spend it on is a counter**, and the first thing a trade
    /// has to get right is that it cannot be made on credit.
    #[test]
    fn nothing_is_built_on_an_empty_store() {
        let mut home = with(0, 0);
        for fitting in Fitting::ALL {
            assert!(!home.affords(fitting), "{} was affordable", fitting.name());
            assert!(!home.buy(fitting));
        }
        assert!(home.built.is_empty());
        assert!(!home.unsaved, "an empty store wrote a file");
    }

    /// And when it is paid for, it is paid *out*: the casks that bought the derrick are
    /// not still on the platform afterwards.
    #[test]
    fn building_costs_what_it_says_it_costs() {
        let mut home = with(8, 0);
        let (casks, fish) = Fitting::Derrick.costs();
        assert!(home.buy(Fitting::Derrick));
        assert_eq!(home.stores.casks, 8 - casks);
        assert_eq!(home.stores.fish, 0 - fish);
        assert!(home.has(Fitting::Derrick));
        assert!(
            home.unsaved,
            "a trade did not ask for the file to be written"
        );
        // And not twice.
        assert!(!home.affords(Fitting::Derrick));
        assert!(!home.buy(Fitting::Derrick));
        assert_eq!(home.built.len(), 1);
    }

    /// A rack is paid for in both, which is the point of having two things to pay with.
    #[test]
    fn the_rack_wants_fish_as_well_as_salvage() {
        let mut home = with(9, 2);
        assert!(
            !home.affords(Fitting::Rack),
            "two fish bought a drying rack"
        );
        home.stores.fish = 6;
        assert!(home.buy(Fitting::Rack));
        assert_eq!(home.stores.fish, 0);
        assert_eq!(home.stores.casks, 7);
    }

    /// What is built survives the file, and an older save without the line reads as a
    /// platform with nothing on it rather than as a broken one.
    #[test]
    fn the_fittings_come_back_out_of_the_file() {
        let mut home = with(20, 20);
        assert!(home.buy(Fitting::Derrick));
        assert!(home.buy(Fitting::Lamp));
        let back = Home::parse(&home.text()).expect("a file it just wrote");
        assert!(back.has(Fitting::Derrick) && back.has(Fitting::Lamp));
        assert!(!back.has(Fitting::Rack));
        let older = format!("sail home {FORMAT}\nsea 1\ncasks 4\n");
        let older = Home::parse(&older).expect("a save from before any of this");
        assert!(older.built.is_empty());
        assert_eq!(older.stores.casks, 4);
    }

    /// A fitting from a later build is skipped rather than guessed at — the same promise
    /// the rest of the file makes.
    #[test]
    fn a_fitting_this_build_has_never_heard_of_is_skipped() {
        let text = format!("sail home {FORMAT}\nsea 1\nbuilt smokehouse\nbuilt derrick\n");
        let home = Home::parse(&text).expect("a file with a stranger in it");
        assert_eq!(home.built, vec![Fitting::Derrick]);
    }
}

#[cfg(test)]
mod when_the_lamp_burns {
    use super::*;

    /// **Not at nine in the morning.** A lamp that is always on is not a lamp, it is a
    /// decoration, and the thing this one is for is the half hour either side of dusk.
    #[test]
    fn it_is_out_in_daylight_and_full_once_the_sun_is_down() {
        assert_eq!(lit_at(40.0), 0.0, "it was burning at noon");
        assert_eq!(lit_at(24.0), 0.0, "it was burning in the forenoon");
        assert_eq!(lit_at(0.0), 1.0, "it was not lit at sunset");
        assert_eq!(lit_at(-6.0), 1.0, "it was not lit after dark");
    }

    /// And it fades rather than snapping, because a light that appears between one frame
    /// and the next reads as a bug in the sky.
    #[test]
    fn it_comes_up_through_the_dusk() {
        let half = lit_at(4.0);
        assert!(
            (half - 0.5).abs() < 0.01,
            "halfway through the dusk it was at {half:.2}"
        );
        assert!(lit_at(6.0) < lit_at(2.0), "it got dimmer as the sun set");
    }
}

#[cfg(test)]
mod where_she_lies {
    use super::*;

    fn town(at: DVec2, name: &str) -> sail_core::land::Harbour {
        sail_core::land::Harbour {
            at,
            name: name.into(),
            exposed_to: 0.0,
        }
    }

    fn coast() -> Vec<sail_core::land::Harbour> {
        vec![
            town(DVec2::new(0.0, 2_000.0), "Nordvik"),
            town(DVec2::new(600.0, 200.0), "Testvik"),
            town(DVec2::new(-4_000.0, 0.0), "Melnes"),
        ]
    }

    /// **In your own water she goes back where she was.** The whole of what a home is.
    #[test]
    fn her_own_berth_is_the_one_in_the_file() {
        let was = DVec2::new(-1_234.0, 99.0);
        let picked = berth_for(true, Some((was, "Fuglhavn".into())), DVec2::ZERO, &coast())
            .expect("a berth");
        assert_eq!(picked.at, was);
        assert_eq!(picked.harbour, "Fuglhavn");
        assert!(!picked.fresh, "a remembered berth was treated as a new one");
    }

    /// **And in anybody else's, the nearest town to where she opened** — which is the
    /// anchorage she is lying in when she opens in one.
    #[test]
    fn a_guest_moors_at_the_nearest_town() {
        let picked = berth_for(false, None, DVec2::new(500.0, 150.0), &coast()).expect("a berth");
        assert_eq!(picked.harbour, "Testvik");
        assert!(picked.fresh);
    }

    /// **A berth in borrowed water is never written down**, which is the one way this
    /// could lose somebody their platform: a position in a visited archipelago, saved as
    /// home, is open water or the inside of a hill in their own.
    ///
    /// The rule is in `moor`, and this is the half of it a function can hold: a guest
    /// berth is always `fresh`, and the file is only touched when the sea is ours.
    #[test]
    fn a_guest_berth_is_not_mistaken_for_home() {
        let remembered = Some((DVec2::new(-1_234.0, 99.0), "Fuglhavn".into()));
        let guest = berth_for(false, remembered, DVec2::ZERO, &coast()).expect("a berth");
        assert_eq!(
            guest.harbour, "Testvik",
            "it did not moor at the nearest town in the water she is actually in"
        );
        assert!(
            guest.fresh,
            "a berth in borrowed water was reported as the remembered one, which is how \
             it would end up in the file"
        );
    }

    /// A world with no towns in it leaves the platform where it was rather than dropping
    /// it in the middle of the sea.
    #[test]
    fn nowhere_to_moor_is_not_an_answer() {
        assert!(berth_for(false, None, DVec2::ZERO, &[]).is_none());
    }
}
