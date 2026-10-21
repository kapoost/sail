//! What you have done, kept whether or not anybody is watching.
//!
//! **Achievements are a first hour, not a grind.** Twelve, and every one of them is a
//! thing somebody would have done anyway on the way to learning the game: finish the
//! lesson, make a delivery, come through a gale with the rig standing, cross three
//! bearings, sleep at the anchorage you built. Nothing here asks for a hundred of
//! anything, because a list that asks for a hundred of anything is a list that changes
//! what the game is for.
//!
//! **They are watched for rather than announced.** The alternative was a call at each
//! place where one is earned — twelve modules each with a line in them saying "and tell
//! the achievements" — and that line is exactly the kind that gets lost in a refactor and
//! is never missed, because nothing fails when an achievement silently stops being given.
//! So this file reads state that already exists and decides for itself. The cost is that
//! it is one system with a long parameter list; the benefit is that the whole list is in
//! one place where it can be read and counted.
//!
//! And they are kept **here**, in our own file, whether Steam is running or not. Steam is
//! told as well when it is there — see `steam.rs` — but a game that only remembers what
//! you have done when a shop is open is a game that forgot what the list was for.

use std::collections::BTreeSet;

use bevy::prelude::*;

/// One thing worth having done.
pub struct Deed {
    /// The name Steam knows it by. Set on the partner site; must match exactly.
    pub api: &'static str,
    /// What it is called on the screen.
    pub name: &'static str,
    /// And how it is got, said the way a sailor would say it.
    pub how: &'static str,
}

/// The whole list. Twelve, in roughly the order they will happen.
pub const DEEDS: &[Deed] = &[
    Deed {
        api: "SAILING_101",
        name: "Sailing 101",
        how: "Finish the lesson: the helm, the sheets, the chart and the rod.",
    },
    Deed {
        api: "MAN_OVERBOARD",
        name: "Man overboard",
        how: "Sail the search pattern and get him back aboard.",
    },
    Deed {
        api: "FIRST_DELIVERY",
        name: "Pilot aboard",
        how: "Take a square-rigger in and hand her over in the anchorage.",
    },
    Deed {
        api: "DEEP_WATER",
        name: "Never under two",
        how: "Take her in without ever having less than two metres under her keel.",
    },
    Deed {
        api: "THREE_BEARINGS",
        name: "A cocked hat",
        how: "Cross three bearings and know where you are without being told.",
    },
    Deed {
        api: "A_GALE",
        name: "Rig standing",
        how: "Ten minutes in force eight with everything still up.",
    },
    Deed {
        api: "A_NIGHT_AT_SEA",
        name: "The middle watch",
        how: "Half an hour of darkness, under way, on open water.",
    },
    Deed {
        api: "FIRST_FISH",
        name: "Supper",
        how: "Land a fish.",
    },
    Deed {
        api: "A_FULL_RACK",
        name: "A season's work",
        how: "Twenty casks landed on your own platform.",
    },
    Deed {
        api: "THE_LAMP",
        name: "A light of your own",
        how: "Build all three: the derrick, the drying rack and the lamp.",
    },
    Deed {
        api: "IN_COMPANY",
        name: "In company",
        how: "Sail within sight of somebody else's boat.",
    },
    Deed {
        api: "SPOKEN",
        name: "Spoken",
        how: "Fly a signal and have it answered.",
    },
];

/// The file this is kept in, and the version of its shape.
const KEPT: &str = "earned";
const FORMAT: u32 = 1;

/// How hard it has to blow, and for how long, to count as a gale come through.
///
/// Force 8 is thirty-four knots, which is 17.5 metres a second. Ten minutes rather than a
/// moment, because a gust is not a gale and the thing being recognised is *carrying on*.
const A_GALE: f64 = 17.5;
const LONG_ENOUGH: f64 = 600.0;

/// And a night: half an hour of real darkness with the boat moving.
const DARK_ENOUGH: f64 = 1_800.0;

#[derive(Resource, Default)]
pub struct Earned {
    done: BTreeSet<String>,
    unsaved: bool,
    /// Seconds so far towards the two that are not a moment but a watch.
    in_a_gale: f64,
    in_the_dark: f64,
}

impl Earned {
    /// Whatever was kept last time.
    pub fn remembered() -> Self {
        let Some(text) = crate::kept::read(KEPT) else {
            return Self::default();
        };
        if !crate::kept::stamped(&text, KEPT, FORMAT) {
            return Self::default();
        }
        let mut done = BTreeSet::new();
        for (key, rest) in crate::kept::lines(&text) {
            if key == "did" && DEEDS.iter().any(|d| d.api == rest) {
                done.insert(rest.to_string());
            }
        }
        Self {
            done,
            ..Self::default()
        }
    }

    pub fn has(&self, api: &str) -> bool {
        self.done.contains(api)
    }

    pub fn count(&self) -> usize {
        self.done.len()
    }

    /// Done. Idempotent, and says so once.
    fn did(&mut self, api: &'static str, steam: &crate::steam::Steam) {
        if !self.done.insert(api.to_string()) {
            return;
        }
        self.unsaved = true;
        let name = DEEDS
            .iter()
            .find(|d| d.api == api)
            .map(|d| d.name)
            .unwrap_or(api);
        info!("earned: {name} ({}/{})", self.done.len(), DEEDS.len());
        steam.unlock(api);
    }

    fn write(&self) -> Option<()> {
        let mut text = format!("sail {KEPT} {FORMAT}\n");
        for api in &self.done {
            text.push_str(&format!("did {api}\n"));
        }
        crate::kept::write(KEPT, &text)
    }
}

/// Everything the watcher reads. A bundle because the list is long and every entry on it
/// is a fact that already exists somewhere else — this file invents nothing.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Deeds<'w> {
    pub basics: Res<'w, crate::basics::Basics>,
    pub drill: Res<'w, crate::tutorial::Tutorial>,
    pub job: Res<'w, crate::mission::Pilotage>,
    pub home: Res<'w, crate::anchorage::Home>,
    pub nav: Res<'w, crate::navigation::Navigation>,
    pub sky: Res<'w, crate::sky::Sky>,
    pub fleet: Res<'w, crate::fleet::Fleet>,
    pub linked: Res<'w, crate::net::Net>,
    pub answer: Res<'w, crate::signals::Answer>,
    pub hoist: Res<'w, crate::signals::Hoist>,
}

/// Look at what has happened, and hand out what has been earned.
pub fn watch(
    mut earned: ResMut<Earned>,
    sim: Res<crate::sim::Sim>,
    time: Res<Time>,
    steam: Res<crate::steam::Steam>,
    deeds: Deeds,
) {
    let dt = time.delta_secs_f64();
    let tm = &sim.sim.telemetry;

    if deeds.basics.step == crate::basics::Step::Done {
        earned.did("SAILING_101", &steam);
    }
    if deeds.drill.stage == crate::tutorial::Stage::Done {
        earned.did("MAN_OVERBOARD", &steam);
    }
    if let Some(crate::mission::Outcome::Delivered { least, .. }) = deeds.job.over {
        earned.did("FIRST_DELIVERY", &steam);
        // **The least water she crossed**, which the mission itself calls the number
        // another pilot would ask about. Two metres under a deep-draught ship is a
        // margin rather than a miss; aground is nought.
        //
        // This was `casks == 0` and it was backwards. `Outcome::Delivered.casks` is
        // `flotsam.landed + flotsam.astern` — the casks you *recovered* — so a run where
        // you saved none was being handed an achievement for losing none. Worse, the
        // thing it claimed to measure is not measured anywhere: the brig pushes what she
        // sheds into `flotsam.adrift` with no counter, and the archipelago seeds that
        // same list with salvage nobody shed. There was no condition to fix, only one to
        // replace.
        if least >= 2.0 {
            earned.did("DEEP_WATER", &steam);
        }
    }
    // Three is a fix — a cocked hat with a boat inside it — and the navigation file says
    // so in those words. Read rather than counted here, so there is one definition.
    if deeds.nav.sights.len() >= 3 {
        earned.did("THREE_BEARINGS", &steam);
    }
    if deeds.home.stores.fish >= 1 {
        earned.did("FIRST_FISH", &steam);
    }
    if deeds.home.stores.casks >= 20 {
        earned.did("A_FULL_RACK", &steam);
    }
    if deeds.home.built.len() >= 3 {
        earned.did("THE_LAMP", &steam);
    }
    // **A person, not a ghost.** `Fleet` is also where the rehearsal boats go —
    // `SAIL_FLEET=3` puts three hand-moved drawings on the water for getting the
    // rendering right — and where the regatta puts its AI rivals. Both are boats as far
    // as this list is concerned and neither is somebody else. Linked *and* in sight is
    // the condition the name already claims.
    if deeds.linked.link.is_some() && !deeds.fleet.boats.is_empty() {
        earned.did("IN_COMPANY", &steam);
    }
    // Answered, not merely flown: the pendant close up is her saying she has understood,
    // and a flag nobody read is a flag nobody read.
    if deeds.hoist.flying.is_some() && deeds.answer.pendant == crate::signals::Pendant::CloseUp {
        earned.did("SPOKEN", &steam);
    }

    // The two that are a watch rather than a moment. Both reset the moment the condition
    // goes, because what is being recognised is carrying on rather than having once been
    // there — ten minutes of gale with a lull in the middle is two five-minute gales.
    let sailing = tm.stw > 1.0 && tm.mishap.is_none();
    if sailing && tm.tws >= A_GALE {
        earned.in_a_gale += dt;
        if earned.in_a_gale >= LONG_ENOUGH {
            earned.did("A_GALE", &steam);
        }
    } else {
        earned.in_a_gale = 0.0;
    }
    // Dark: the sun below the horizon by a useful margin, which `Sky` already knows how
    // to say. Not a clock reading, because the hour it gets dark is a function of the
    // latitude and the time of year and both of those live in `sky.rs`.
    if sailing && deeds.sky.elevation() < -0.1 {
        earned.in_the_dark += dt;
        if earned.in_the_dark >= DARK_ENOUGH {
            earned.did("A_NIGHT_AT_SEA", &steam);
        }
    } else {
        earned.in_the_dark = 0.0;
    }
}

/// Write it when there is something to write.
///
/// On a timer like the platform's rather than on every change: a set of twelve strings is
/// nothing to write, but the thing being avoided is a file rewritten sixty times a second
/// for the whole of a passage.
pub fn keep(mut earned: ResMut<Earned>, time: Res<Time>, mut since: Local<f32>) {
    // A screenshot run or a trailer take is not somebody playing: the reel flies a signal
    // and gets it answered, moors the platform on the bow and jumps the weather to a
    // gale, and every one of those is a thing this file would otherwise write down. See
    // `kept::staging`, which the platform has obeyed since an earlier review and this did
    // not because it was written afterwards.
    if !earned.unsaved || crate::kept::staging() {
        return;
    }
    *since += time.delta_secs();
    if *since < 5.0 {
        return;
    }
    *since = 0.0;
    if earned.write().is_some() {
        earned.unsaved = false;
    }
}

/// And on the way out, for the last few seconds the timer has not reached.
pub fn keep_on_the_way_out(quitting: MessageReader<AppExit>, earned: Res<Earned>) {
    if quitting.is_empty() {
        return;
    }
    if earned.unsaved && !crate::kept::staging() {
        earned.write();
    }
}

pub fn plugin(app: &mut App) {
    use crate::stages::Deck;
    app.insert_resource(Earned::remembered());
    app.add_systems(Update, (watch, keep).in_set(Deck::Report));
    app.add_systems(Last, keep_on_the_way_out);
}

#[cfg(test)]
mod what_has_been_done {
    use super::*;

    /// **Steam matches on the API name exactly**, and a name that is in the code and not
    /// on the partner site is an achievement that silently never unlocks for anybody.
    /// There is no way to check the far side from here, so this checks the half that can
    /// be: that the list is a list, with no two the same and nothing empty.
    #[test]
    fn every_deed_has_its_own_name() {
        let mut seen = BTreeSet::new();
        for deed in DEEDS {
            assert!(!deed.api.is_empty() && !deed.name.is_empty() && !deed.how.is_empty());
            assert!(
                deed.api
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit()),
                "{} is not the shape an API name has",
                deed.api
            );
            assert!(seen.insert(deed.api), "{} is in the list twice", deed.api);
        }
        assert!(
            (10..=15).contains(&DEEDS.len()),
            "ten to fifteen, no grind — and there are {}",
            DEEDS.len()
        );
    }

    /// A file written and read back is the same set, and a deed this build has never
    /// heard of is skipped rather than kept — the same promise every other file here
    /// makes.
    #[test]
    fn what_is_kept_comes_back() {
        let mut earned = Earned::default();
        earned.done.insert("FIRST_FISH".into());
        earned.done.insert("THE_LAMP".into());
        let mut text = format!("sail {KEPT} {FORMAT}\n");
        for api in &earned.done {
            text.push_str(&format!("did {api}\n"));
        }
        text.push_str("did FROM_A_LATER_BUILD\n");
        text.push_str("something-else entirely\n");

        assert!(crate::kept::stamped(&text, KEPT, FORMAT));
        let read: BTreeSet<String> = crate::kept::lines(&text)
            .filter(|(k, v)| *k == "did" && DEEDS.iter().any(|d| d.api == *v))
            .map(|(_, v)| v.to_string())
            .collect();
        assert_eq!(read, earned.done);
    }
}
