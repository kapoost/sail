//! The reel: the game playing itself through its own scenes, for filming.
//!
//! **A trailer wants footage this program cannot take of itself.** Its own screenshot
//! dials can photograph a scene and can even record one frame at a time — but not faster
//! than about twelve a second, because past that the capture is served a picture that has
//! not been drawn yet and every frame comes back black. Twelve interpolated up to
//! twenty-four is a cut; it is not sixty frames of real motion with the rigging in it.
//!
//! So this is the other way round: instead of the game filming itself, something else
//! films the game — a screen recorder, which takes sixty real frames a second and the
//! sound with them — and the game's job is to **put on the show without a hand on the
//! keyboard**. `SAIL_REEL` starts a timetable: the weather, the hour, the camera, the
//! course and the chart are each changed at a stated second, in an order that builds.
//!
//! There are two of them, because they are for two different things.
//!
//! * `SAIL_REEL=1` — **the passage.** A minute and three quarters of what the game *is*:
//!   the boat, the deck, the paper, the gale, the drawn sea, home, a flag, the night. It
//!   has the instruments in it, because they are half the game.
//! * `SAIL_REEL=short` — **under way.** Eighty-odd seconds of nothing but a boat sailing:
//!   dawn, the wardrobe changing as she bears away, the breeze making, a reef, sunset,
//!   moonrise, night. No panels, no mission, no text. It is the one to put in front of
//!   somebody who has never seen the game, and it is the one to cut down to sixty.
//!
//! What neither of them is, is a cutscene. Every beat below is the same resource the
//! settings panel, the compass buttons or the view key writes to, changed the same way a
//! hand would change it. The single exception is the sky's clock, which runs a sunrise at
//! five hundred times so that ninety minutes of light fits in ten seconds — see
//! [`crate::sky::Clock`], which says so in its own words.
//!
//! Press record, run it, and cut the result up afterwards: each scene is long enough to
//! take six or seven seconds out of the middle, away from the moment the weather jumps.

use bevy::prelude::*;

use crate::camera::View;
use crate::sim::Sim;

/// One thing the reel does, at one moment.
#[derive(Clone, Copy, Debug)]
pub enum Act {
    /// Force, oktas, the hour, and the fetch in kilometres — the four dials the weather
    /// page has, set together because a scene is all four at once.
    ///
    /// The hour is optional, and the `None` is the whole reason: a reel that is running
    /// its own clock through a sunrise must be able to change the wind without dragging
    /// the sun back to where the last beat left it.
    Weather {
        force: u8,
        oktas: u8,
        hour: Option<f32>,
        fetch: f64,
    },
    /// Where the camera sits.
    Seat(View),
    /// And whether it is sweeping round her.
    Orbit(f32),
    /// The console and the chart: off for the sailing shots, on where the instruments
    /// are the subject.
    Furniture { console: bool, chart: Chart },
    /// The drawn sea rather than the lit one.
    Ink(bool),
    /// A flag at the masthead, for the scene with the brig in it.
    Hoist(crate::signals::Flag),
    /// Moor the platform fine on the bow, so that home comes into the picture.
    Home,
    /// How fast the sun goes. One is a sailing day; five hundred is a sunrise in ten
    /// seconds.
    Clock(f32),
    /// Send the foredeck for a sail.
    ///
    /// **The console has a button for each of these and this presses it.** The watch
    /// chooses its own wardrobe off a minute's average of the wind, which is right at sea
    /// and far too slow to film: measured here, an order given by bearing away took
    /// thirty-odd seconds to come out as cloth, by which time the shot it belonged to was
    /// over. A skipper who wants the kite up now says so, and so does the reel.
    Headsail(sail_core::state::Headsail),
    /// Reef the main: nought for full, one, two, three, four for handed.
    ///
    /// The same as selecting the reef line and hauling it to the next cringle. It runs at
    /// its own pace and stops at the stop, because that is what a slab reef does.
    Reef(usize),
    /// Put the pilot on a course that gives this true wind angle, signed degrees —
    /// positive with the wind to starboard, so the sign is the tack.
    ///
    /// **This is how a sail change gets filmed.** There is no dial for "set the Code 0":
    /// the watch chooses the wardrobe off the angle and the breeze, the way a watch does,
    /// so the way to film the kite going up is to bear away in light air and let them.
    /// The pilot is handed a compass course — the same thing its own four buttons nudge —
    /// worked out from the angle wanted and the angle she is on.
    Steer(f64),
}

/// What the chart is doing in a beat.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Chart {
    Away,
    Corner,
    Table,
}

/// Which reel.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Which {
    #[default]
    Passage,
    UnderWay,
}

/// The passage: a minute and three quarters of what the game is.
///
/// Ordered, and the order is the cut: it opens on her sailing, goes round her once, sits
/// the viewer at the helm, puts the paper on the table, builds a gale, turns the sea into
/// a drawing, brings the evening down over the platform and finishes at night with a moon
/// on the water.
pub const PASSAGE: &[(f64, Act)] = &[
    (
        0.0,
        Act::Weather {
            force: 4,
            oktas: 3,
            hour: Some(8.0),
            fetch: 60.0,
        },
    ),
    (
        0.5,
        Act::Furniture {
            console: false,
            chart: Chart::Away,
        },
    ),
    (1.0, Act::Seat(View::Chase)),
    // Round her once: rig, heel, wake, hull.
    (9.0, Act::Orbit(12.0)),
    (20.0, Act::Orbit(0.0)),
    // The deck as a place.
    (21.0, Act::Seat(View::Cockpit)),
    // The paper, which is the half of this game that is not the water.
    (
        31.0,
        Act::Furniture {
            console: true,
            chart: Chart::Table,
        },
    ),
    // A gale, from the same seat she was just sailing quietly in.
    (
        43.0,
        Act::Weather {
            force: 8,
            oktas: 6,
            hour: Some(11.0),
            fetch: 200.0,
        },
    ),
    (
        43.5,
        Act::Furniture {
            console: false,
            chart: Chart::Away,
        },
    ),
    (44.0, Act::Seat(View::Chase)),
    // The drawn sea: the same water, told differently.
    (57.0, Act::Ink(true)),
    // Evening, and home.
    (
        68.0,
        Act::Weather {
            force: 4,
            oktas: 2,
            hour: Some(19.3),
            fetch: 40.0,
        },
    ),
    (68.5, Act::Home),
    (69.0, Act::Ink(false)),
    // A flag at the masthead. Worth filming on its own — the cloth is on the shroud
    // either way — and it is the scene with the brig in it when the reel is run with
    // one: `SAIL_BRIG=600 SAIL_BRIG_SHOW=170` puts her where she can be seen, and she
    // answers this hoist with her pendant.
    (78.0, Act::Hoist(crate::signals::Flag::Kilo)),
    // And the night, with a moon on the water and the chart in the corner under its own
    // dim lamp — which is the one place the red torch can be filmed.
    (
        88.0,
        Act::Weather {
            force: 4,
            oktas: 1,
            hour: Some(1.6),
            fetch: 40.0,
        },
    ),
    (
        88.5,
        Act::Furniture {
            console: true,
            chart: Chart::Corner,
        },
    ),
];

/// Under way: a boat sailing, and nothing else.
///
/// **A sail change takes twenty-four seconds and that is what shapes this.** The furling
/// line runs at 0.085 a second and the change is two full turns of the drum — roll the old
/// one away, swap at the bow, set the new one — so an order given at the shot it belongs
/// to is a bare forestay for the whole shot. Measured, not guessed: the first cut of this
/// reel ordered the gennaker ten seconds before it wanted it and filmed a boat with no
/// headsail. Every order below is therefore given a shot early, and the rolling away is
/// part of the shot before.
///
/// The second thing it shapes is the order of the weather. The kite comes in **half a
/// minute before the breeze arrives**, because a gennaker in force seven is over its
/// cloth and a sail that is over its cloth has about five seconds of dignity left — the
/// twelve seconds it takes to roll away would have blown it out. Which is also how it
/// works at sea: nobody hands a kite because it is already blowing.
///
/// **No panels and no story**, which is the point of having a second one. The passage
/// reel is for somebody deciding whether to buy it; this is for somebody deciding whether
/// to look at it, and what makes that decision is water, light and cloth.
///
/// The sun does the work. Sunrise here is **05:29** and sunset **18:31** — fifty-seven
/// north, five degrees of declination, both read off the same two sines the sky itself
/// uses — so the dawn beat opens at a quarter to five with the clock at five hundred and
/// twenty, which puts the sun on the horizon about five seconds in and gives four more of
/// it climbing. The evening beat opens at ten to six and runs on past sunset without
/// stopping, which is how the moonrise is free: a full moon is at the antisolar point, so
/// it comes up on the opposite bearing as the sun goes down, and the camera only has to
/// turn round.
///
/// The two long scenes are long because somebody else is doing the work in them and they
/// take their own time: the watch chooses the wardrobe off a minute's average of the wind,
/// so bearing away from close-hauled to a broad reach in light air walks up the ladder —
/// genoa, Code 0, gennaker — at the watch's pace and not the reel's. Same at the other
/// end, where it comes on to blow and the kite has to come in before the reef goes down.
/// Both are marked in `docs/store.md` as the scenes to cut from.
pub const UNDER_WAY: &[(f64, Act)] = &[
    (
        0.0,
        Act::Furniture {
            console: false,
            chart: Chart::Away,
        },
    ),
    (
        0.1,
        Act::Weather {
            force: 3,
            oktas: 2,
            hour: Some(4.75),
            fetch: 25.0,
        },
    ),
    (0.2, Act::Seat(View::Chase)),
    (0.4, Act::Steer(50.0)),
    (0.5, Act::Clock(520.0)),
    // SUNRISE, 0.5 to 10 — 04:45 to 06:09. The sun breaks the horizon about five seconds
    // in and climbs for the rest of it; before that the sky is doing the thing it only
    // does for twenty minutes a day.
    (10.0, Act::Clock(1.0)),
    (10.2, Act::Seat(View::Cockpit)),
    // FIRST LIGHT, CLOSE-HAULED, 10 to 18 — the deck as a place, heeled, low sun through
    // the genoa. A texture shot.
    (18.0, Act::Seat(View::Chase)),
    (18.2, Act::Orbit(6.0)),
    (18.4, Act::Steer(135.0)),
    (18.6, Act::Headsail(sail_core::state::Headsail::Gennaker)),
    // BEARING AWAY, 18 to 30 — fifty degrees to a hundred and thirty-five, the camera
    // walking round her the other way, and the genoa rolling away on the stay as she
    // goes. The order is given a shot early on purpose: see the note above.
    (30.0, Act::Orbit(0.0)),
    // THE KITE, 30 to 42 — the bow clear at about thirty, the gennaker going up off the
    // sprit and full and drawing by forty-two, which is the whole twenty-four seconds the
    // drum takes and the reason this shot is twelve and not ten.
    (42.0, Act::Headsail(sail_core::state::Headsail::Genoa)),
    (42.2, Act::Orbit(5.0)),
    // AND IN AGAIN, 42 to 54 — twelve seconds before the breeze arrives, which is when a
    // kite actually comes down. Nobody hands one because it is already blowing.
    (54.0, Act::Orbit(0.0)),
    (
        54.2,
        Act::Weather {
            force: 7,
            oktas: 6,
            hour: None,
            fetch: 150.0,
        },
    ),
    (54.4, Act::Seat(View::Astern)),
    (55.4, Act::Reef(2)),
    // IT COMES ON TO BLOW, 54 to 66 — two slabs into the main and the genoa unrolling
    // into it, seen from aft where the wake tilts as she lies over. The hour is left
    // alone on purpose: the morning carries on.
    (66.0, Act::Seat(View::Chase)),
    (66.2, Act::Steer(115.0)),
    (
        66.4,
        Act::Weather {
            force: 5,
            oktas: 3,
            hour: Some(17.83),
            fetch: 100.0,
        },
    ),
    (66.6, Act::Clock(520.0)),
    // SUNSET, 66 to 76 — 17:50 onwards, the sun on the horizon at about seventy-one.
    (76.0, Act::Orbit(8.0)),
    // MOONRISE, 76 to 85 — the clock still running and the camera coming round to the
    // opposite bearing, which is where a full moon always is. Nothing stages this: it is
    // the same two sines as the sunrise, read backwards.
    (85.0, Act::Orbit(0.0)),
    (85.2, Act::Clock(1.0)),
    // NIGHT UNDER THE MOON, 85 to 93 — half past eight, the moon fourteen degrees up and
    // its path on the water.
];

/// How long each runs before it says so.
pub const PASSAGE_RUNS_FOR: f64 = 102.0;
pub const UNDER_WAY_RUNS_FOR: f64 = 93.0;

impl Which {
    pub fn beats(self) -> &'static [(f64, Act)] {
        match self {
            Which::Passage => PASSAGE,
            Which::UnderWay => UNDER_WAY,
        }
    }

    pub fn runs_for(self) -> f64 {
        match self {
            Which::Passage => PASSAGE_RUNS_FOR,
            Which::UnderWay => UNDER_WAY_RUNS_FOR,
        }
    }
}

/// Whether a reel was asked for, and which.
#[derive(Resource, Clone, Copy, Default)]
pub struct Reel {
    pub on: bool,
    pub which: Which,
    /// Seconds of quiet before the first beat — `SAIL_REEL_AFTER`.
    ///
    /// **For the hand that is holding the stopwatch.** Somebody recording this has a
    /// window to find, a screen recorder to point at it and a mouse to get out of the
    /// frame, and all of that happens while the reel is already playing its opening shot
    /// at them. Thirty seconds of her simply sailing, before anything is staged, is the
    /// difference between a take and a retake.
    ///
    /// It also buys the only way to photograph the early scenes from inside the program:
    /// the capture cannot produce a picture for the first forty seconds or so of a run —
    /// the same limitation `dist/store.sh` works around by waiting seventy — so a reel
    /// that starts at zero has its first four scenes in a part of the run the game cannot
    /// photograph. Delayed, they can be checked like any other.
    pub from: f64,
    done: usize,
    said: bool,
}

impl Reel {
    pub fn asked() -> Self {
        let (on, which) = match crate::knob::var("SAIL_REEL").as_deref() {
            Ok("0") | Err(_) => (false, Which::Passage),
            Ok("short") | Ok("2") | Ok("under-way") => (true, Which::UnderWay),
            Ok(_) => (true, Which::Passage),
        };
        Self {
            on,
            which,
            from: crate::knob::var("SAIL_REEL_AFTER")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0),
            done: 0,
            said: false,
        }
    }
}

/// Everything a beat can reach, behind one name: a system may have sixteen parameters
/// and this one wanted more.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Stage<'w> {
    pub sky: ResMut<'w, crate::sky::Sky>,
    pub clock: ResMut<'w, crate::sky::Clock>,
    pub view: ResMut<'w, View>,
    pub sweep: ResMut<'w, crate::camera::Sweep>,
    pub ink: ResMut<'w, crate::world::Ink>,
    pub hud: ResMut<'w, crate::hud::HudVisible>,
    pub console: ResMut<'w, crate::panel::Console>,
    pub chart: ResMut<'w, crate::chart::Chart>,
    pub hoist: ResMut<'w, crate::signals::Hoist>,
    pub berth: ResMut<'w, crate::anchorage::Berth>,
    pub pilot: ResMut<'w, crate::input::Pilot>,
    pub orders: ResMut<'w, crate::orders::Orders>,
    pub bank: ResMut<'w, crate::clutch::Bank>,
}

/// Run the timetable.
pub fn run(mut reel: ResMut<Reel>, mut sim: ResMut<Sim>, mut stage: Stage) {
    if !reel.on {
        return;
    }
    let beats = reel.which.beats();
    // Everything below is said in reel time, which is the clock the cut is made against;
    // `from` is the only place it and the boat's clock are allowed to differ.
    let now = sim.sim.state.time - reel.from;
    while reel.done < beats.len() && beats[reel.done].0 <= now {
        let (at, act) = beats[reel.done];
        reel.done += 1;
        play(act, &mut sim, &mut stage);
        info!("reel: {at:>5.1} s — {act:?}");
    }
    let runs_for = reel.which.runs_for();
    if !reel.said && now > runs_for {
        reel.said = true;
        info!("reel: that is the lot — {runs_for:.0} s");
    }
}

fn play(act: Act, sim: &mut Sim, stage: &mut Stage) {
    match act {
        Act::Weather {
            force,
            oktas,
            hour,
            fetch,
        } => {
            // The same two resources the weather page writes, and in the same way: this
            // is a hand on the sliders, not a back door into the model.
            if let Some(wind) = sim.sim.wind.adjustable() {
                // `blows` rather than the base alone — see its own note. A reel that sets
                // force 7 and gets force 4 for the next half hour films the wrong day.
                wind.synoptic
                    .blows(sail_core::units::kn_to_ms(crate::weather::force_wind(
                        force,
                    )));
                wind.synoptic.start_cover(oktas as f64 / 8.0);
            }
            if let Some(sea) = sim.sim.water.adjustable() {
                sea.set_fetch(fetch * 1000.0);
                // Settled rather than built from nothing: a trailer cannot wait half an
                // hour for a gale to raise its own sea.
                sea.settle(sim.sim.telemetry.tws, 0.0);
            }
            *stage.sky = crate::sky::Sky::at(hour.unwrap_or(stage.sky.hour), oktas as f32 / 8.0);
        }
        Act::Seat(seat) => {
            *stage.view = seat;
        }
        Act::Orbit(rate) => {
            stage.sweep.0 = (rate > 0.0).then_some(rate.to_radians());
        }
        Act::Furniture { console, chart } => {
            stage.console.hidden = !console;
            stage.hud.0 = false;
            stage.chart.open = chart != Chart::Away;
            stage.chart.spread = chart == Chart::Table;
        }
        Act::Ink(on) => stage.ink.0 = on,
        Act::Hoist(flag) => {
            stage.hoist.flying = Some(flag);
            stage.hoist.since = sim.sim.state.time;
        }
        Act::Home => {
            // Fine on the port bow, where the drawing knob puts it — see `anchorage`.
            let st = &sim.sim.state;
            let heading = st.heading - 0.6;
            stage.berth.at = Some(st.pos + glam::DVec2::new(heading.sin(), heading.cos()) * 70.0);
            stage.berth.harbour = "alongside".into();
        }
        Act::Clock(rate) => stage.clock.0 = rate,
        Act::Headsail(which) => stage.orders.set(sim, which),
        Act::Reef(slab) => {
            use crate::clutch::{Line, BANK};
            let which = BANK.iter().position(|l| *l == Line::Reef).unwrap_or(3);
            if let Some(stop) = Line::Reef
                .detents(&sim.controls)
                .and_then(|stops| stops.get(slab))
            {
                stage.bank.target[which] = Some(*stop);
            }
        }
        Act::Steer(twa) => {
            // Turning to starboard by δ swings the wind δ further aft on the port side,
            // which is to say the true wind angle *falls* by δ. So the course that gives
            // the angle wanted is the course she is on plus the difference the other way
            // round. Said once here rather than guessed at each beat.
            let st = &sim.sim.state;
            let want = twa.to_radians();
            let turn = sim.sim.telemetry.twa - want;
            stage.pilot.inner.mode = sail_core::autopilot::PilotMode::Heading(
                sail_core::units::wrap_tau(st.heading + turn),
            );
        }
    }
}

pub fn plugin(app: &mut App) {
    use crate::stages::Deck;
    app.insert_resource(Reel::asked());
    app.add_systems(Update, run.in_set(Deck::Weather));
}

#[cfg(test)]
mod the_running_order {
    use super::*;

    /// **A timetable runs forwards.** Out of order it would play the gale before the
    /// breeze and the beats would fire in one lump, because the loop that runs them takes
    /// everything whose moment has passed.
    #[test]
    fn the_beats_are_in_order_and_inside_the_run() {
        for which in [Which::Passage, Which::UnderWay] {
            let mut last = -1.0;
            for (at, _) in which.beats() {
                assert!(
                    *at >= last,
                    "{which:?}: a beat at {at} s comes after {last}"
                );
                assert!(
                    *at < which.runs_for(),
                    "{which:?}: a beat at {at} s is after the reel has ended"
                );
                last = *at;
            }
        }
    }

    /// And every scene has room to be cut out of: six seconds in the middle, away from
    /// the moment the weather jumps.
    #[test]
    fn every_scene_is_long_enough_to_cut_from() {
        for which in [Which::Passage, Which::UnderWay] {
            let beats = which.beats();
            // Beats within a second and a half of an earlier one are that one's scene
            // being staged — the weather, the furniture, the seat and the course all
            // change together — so only the gaps that are a *scene* are measured. By
            // index rather than by value, so that two beats at the same moment are one
            // scene and not a scene of no length.
            let moments: Vec<f64> = beats
                .iter()
                .enumerate()
                .filter(|(i, (at, _))| {
                    beats[..*i]
                        .iter()
                        .all(|(other, _)| (at - other).abs() > 1.5)
                })
                .map(|(_, (at, _))| *at)
                .collect();
            for pair in moments.windows(2) {
                assert!(
                    pair[1] - pair[0] >= 8.0,
                    "{which:?}: a scene from {:.1} to {:.1} is {:.1} s, which is not a shot",
                    pair[0],
                    pair[1],
                    pair[1] - pair[0]
                );
            }
            // And the last scene gets the same room before the reel says it is done.
            let last = *moments.last().expect("a reel has beats in it");
            assert!(
                which.runs_for() - last >= 8.0,
                "{which:?}: the last scene gets {:.1} s",
                which.runs_for() - last
            );
        }
    }

    /// **The short one is the one with no furniture in it**, which is the whole reason it
    /// exists: it is a boat sailing, for somebody who has never seen the game. A panel
    /// that crept back in would not fail anything else here.
    #[test]
    fn nothing_is_on_the_screen_in_the_short_one() {
        for (at, act) in UNDER_WAY {
            match act {
                Act::Furniture { console, chart } => assert!(
                    !console && *chart == Chart::Away,
                    "{at} s puts the furniture back on screen"
                ),
                Act::Hoist(_) | Act::Home | Act::Ink(_) => {
                    panic!("{at} s is story, and this reel has none")
                }
                _ => {}
            }
        }
    }
}
