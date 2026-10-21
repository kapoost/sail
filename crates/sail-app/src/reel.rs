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
//! keyboard**. `SAIL_REEL=1` starts a timetable: the weather, the hour, the camera, the
//! chart and the drawn sea are each changed at a stated second, in an order that builds,
//! and the whole thing runs for about a minute and a half and then says so.
//!
//! What it is *not* is a cutscene. Every beat below is the same resource the settings
//! panel writes to, changed the same way a hand would change it; nothing here is a
//! special case in the renderer, and anything the reel can show, a player can do.
//!
//! Press record, run it, and cut the result up afterwards: each beat is long enough to
//! take six or seven seconds out of the middle, away from the moment the weather jumps.

use bevy::prelude::*;

use crate::camera::View;
use crate::sim::Sim;

/// One thing the reel does, at one moment.
#[derive(Clone, Copy, Debug)]
pub enum Act {
    /// Force, oktas, the hour, and the fetch in kilometres — the four dials the weather
    /// page has, set together because a scene is all four at once.
    Weather {
        force: u8,
        oktas: u8,
        hour: f32,
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
}

/// What the chart is doing in a beat.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Chart {
    Away,
    Corner,
    Table,
}

/// The timetable.
///
/// Ordered, and the order is the cut: it opens on her sailing, goes round her once, sits
/// the viewer at the helm, puts the paper on the table, builds a gale, turns the sea into
/// a drawing, brings the evening down over the platform and finishes at night with a moon
/// on the water. Each beat holds for long enough that six seconds can be taken out of the
/// middle of it.
pub const REEL: [(f64, Act); 17] = [
    (
        0.0,
        Act::Weather {
            force: 4,
            oktas: 3,
            hour: 8.0,
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
            hour: 11.0,
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
            hour: 19.3,
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
            hour: 1.6,
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

/// How long the whole thing runs before it says so.
pub const RUNS_FOR: f64 = 102.0;

/// Whether the reel was asked for.
#[derive(Resource, Clone, Copy, Default)]
pub struct Reel {
    pub on: bool,
    done: usize,
    said: bool,
}

impl Reel {
    pub fn asked() -> Self {
        Self {
            on: crate::knob::var("SAIL_REEL").is_ok_and(|v| v != "0"),
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
    pub view: ResMut<'w, View>,
    pub sweep: ResMut<'w, crate::camera::Sweep>,
    pub ink: ResMut<'w, crate::world::Ink>,
    pub hud: ResMut<'w, crate::hud::HudVisible>,
    pub console: ResMut<'w, crate::panel::Console>,
    pub chart: ResMut<'w, crate::chart::Chart>,
    pub hoist: ResMut<'w, crate::signals::Hoist>,
    pub berth: ResMut<'w, crate::anchorage::Berth>,
}

/// Run the timetable.
pub fn run(mut reel: ResMut<Reel>, mut sim: ResMut<Sim>, mut stage: Stage) {
    if !reel.on {
        return;
    }
    let now = sim.sim.state.time;
    while reel.done < REEL.len() && REEL[reel.done].0 <= now {
        let (at, act) = REEL[reel.done];
        reel.done += 1;
        play(act, &mut sim, &mut stage);
        info!("reel: {at:>5.1} s — {act:?}");
    }
    if !reel.said && now > RUNS_FOR {
        reel.said = true;
        info!("reel: that is the lot — {RUNS_FOR:.0} s");
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
            *stage.sky = crate::sky::Sky::at(hour, oktas as f32 / 8.0);
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
        let mut last = -1.0;
        for (at, _) in REEL {
            assert!(at >= last, "a beat at {at} s comes after one at {last} s");
            assert!(
                at < RUNS_FOR,
                "a beat at {at} s is after the reel has ended"
            );
            last = at;
        }
    }

    /// And every scene has room to be cut out of: six seconds in the middle, away from
    /// the moment the weather jumps.
    #[test]
    fn every_scene_is_long_enough_to_cut_from() {
        // Beats within a second and a half of each other are one scene being staged —
        // the weather, the furniture and the seat all change together — so only the gaps
        // that are a *scene* are measured.
        let moments: Vec<f64> = REEL
            .iter()
            .map(|(at, _)| *at)
            .filter(|at| {
                REEL.iter()
                    .all(|(other, _)| *other >= *at || (at - other).abs() > 1.5)
            })
            .collect();
        for pair in moments.windows(2) {
            assert!(
                pair[1] - pair[0] >= 8.0,
                "a scene from {:.1} to {:.1} is {:.1} s, which is not a shot",
                pair[0],
                pair[1],
                pair[1] - pair[0]
            );
        }
    }
}
