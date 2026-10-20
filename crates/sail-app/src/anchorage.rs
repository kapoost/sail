//! Her own anchorage: a platform moored in it, and what is piled on it.
//!
//! **This game does not save at sea and it is not going to.** A passage is an evening,
//! the clock runs at one to one, and a boat that can be put down mid-gale and picked up
//! next week is a boat nothing can go wrong to. What persists is not where she is — it is
//! what you have brought home.
//!
//! Home is a platform: a raft of a thing moored in one anchorage of this archipelago,
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
use glam::DVec2;

use crate::sim::Sim;

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
    /// Whether anything has changed since the last write.
    pub unsaved: bool,
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
        let text = std::fs::read_to_string(Self::path()?).ok()?;
        Self::parse(&text)
    }

    /// Write it where it will be found again. Through a temporary file and a rename, so
    /// that a crash halfway through a write leaves the old one rather than half a new one.
    pub fn write(&self) -> Option<()> {
        let path = Self::path()?;
        std::fs::create_dir_all(path.parent()?).ok()?;
        let beside = path.with_extension("writing");
        std::fs::write(&beside, self.text()).ok()?;
        std::fs::rename(&beside, &path).ok()?;
        Some(())
    }

    /// Where the file lives, by the conventions of the machine it is on.
    ///
    /// No crate for this. Three platforms, three environment variables, and the whole of
    /// what a crate would do for us is the part we would still have to check.
    pub fn path() -> Option<std::path::PathBuf> {
        let home = std::env::var("HOME").ok();
        #[cfg(target_os = "macos")]
        let dir = std::path::PathBuf::from(home?).join("Library/Application Support/sail");
        #[cfg(target_os = "windows")]
        let dir = std::path::PathBuf::from(std::env::var("APPDATA").ok()?).join("sail");
        #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
        let dir = match std::env::var("XDG_DATA_HOME") {
            Ok(data) if !data.is_empty() => std::path::PathBuf::from(data).join("sail"),
            _ => std::path::PathBuf::from(home?).join(".local/share/sail"),
        };
        Some(dir.join("home"))
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
        out
    }

    /// And back again. A line it does not know is skipped rather than fatal.
    pub fn parse(text: &str) -> Option<Self> {
        let mut lines = text.lines();
        let stamp = lines.next()?;
        let version: u32 = stamp.strip_prefix("sail home ")?.trim().parse().ok()?;
        if version > FORMAT {
            return None;
        }
        let mut home = Self::default();
        for line in lines {
            let (key, value) = line.trim().split_once(' ').unwrap_or((line.trim(), ""));
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
                _ => {}
            }
        }
        Some(home)
    }
}

/// Pick the berth, once, and never again.
///
/// The nearest anchorage to where she first came to — which on a new sea is the clear
/// water the day was chosen for, so the platform ends up a mile or two from the start and
/// in sight of it. Any anchorage would do; *the same* anchorage every time is the whole
/// requirement.
pub fn moor(mut home: ResMut<Home>, sim: Res<Sim>, course: Res<crate::course::Course>) {
    if home.moored.is_some() || course.harbours.is_empty() {
        return;
    }
    let from = sim.sim.state.pos;
    // `SAIL_PLATFORM=here` moors it where she is, which is the only way to look at a
    // thing that is otherwise two miles away through a day's sailing. A drawing knob and
    // nothing else — the same argument as `SAIL_BRIG_SHOW`, and it is not written to the
    // file, so a berth looked at is not a berth moved.
    if crate::knob::var("SAIL_PLATFORM").as_deref() == Ok("here") {
        info!("the platform is moored alongside, for looking at");
        home.harbour = "alongside".into();
        // Fine on the port bow, sixty metres off: dead ahead is behind her own mainsail
        // from the chase camera, and abeam is outside its field. This is the one place
        // she can be looked at from without steering first.
        let heading = sim.sim.state.heading - 0.6;
        home.moored = Some(from + DVec2::new(heading.sin(), heading.cos()) * 60.0);
        return;
    }
    let Some(berth) = course
        .harbours
        .iter()
        .min_by(|a, b| (a.at - from).length().total_cmp(&(b.at - from).length()))
        .cloned()
    else {
        return;
    };
    info!(
        "the platform lies at {} — {:.1} nm off",
        berth.name,
        (berth.at - from).length() / 1_852.0
    );
    home.harbour = berth.name.clone();
    home.moored = Some(berth.at);
    home.unsaved = true;
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
    mut built: Local<bool>,
) {
    let Some(at) = home.moored else {
        return;
    };
    if !*built {
        *built = true;
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
        let deck = meshes.add(Cuboid::new(9.0, 0.5, 6.0));
        let float = meshes.add(Cylinder::new(0.8, 8.4));
        let hut = meshes.add(Cuboid::new(2.8, 2.1, 2.4));
        let roof = meshes.add(Cuboid::new(3.2, 0.18, 2.8));
        commands
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
            });
        info!("platform moored at {}", home.harbour);
    }

    // The pile. Rebuilt only when the count it is drawing has changed, which is a few
    // times an evening.
    let want = home.stores.casks.min(SHOWS);
    let have = stack.iter().count() as u32;
    if want == have {
        return;
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
    sim: Res<Sim>,
    mut flotsam: ResMut<crate::barrels::Flotsam>,
    mut fishing: ResMut<crate::fishing::Fishing>,
    time: Res<Time>,
    sound: Res<crate::sound::SoundHandle>,
    mut passing: Local<f64>,
) {
    let Some(berth) = home.moored else {
        return;
    };
    let here = sim.sim.state.pos;
    let knots = sail_core::units::ms_to_kn(sim.sim.telemetry.stw);
    if !alongside(here, knots, berth) {
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
    if *passing < A_CASK_TAKES {
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

/// Write it down, when there is something to write and not more often than that.
pub fn keep(mut home: ResMut<Home>, time: Res<Time>, mut since: Local<f32>) {
    if !home.unsaved {
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
