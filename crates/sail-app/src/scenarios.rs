//! Scenarios: the game run headless, a frame at a time, with the answers asserted.
//!
//! **The gap this fills is the one that has cost every real bug this week.** The physics
//! has tests and they are good ones; the small pure functions have tests and they catch
//! arithmetic. What had nothing was the *wiring* — which system runs, in what order,
//! with which resource, and what one frame of it does to the next. Every fault reported
//! from the deck in the last few days lived exactly there: a key read twice by a block
//! pasted twice, a fix that asked a counter another system had not yet written, an
//! identification that reached the chart table but not the hand holding the compass.
//!
//! None of those is findable by reading and all of them are one assertion wide.
//!
//! The technology is Bevy's own: an `App` with `MinimalPlugins`, the resources the
//! systems under test need, and `update()` for a frame. No renderer, no window, no new
//! dependency — the suite still runs in under a second, which is the property that
//! decides whether tests get run at all. What cannot come in here is anything that draws
//! through egui, which needs a context; that is the half this file deliberately does not
//! reach, and the reason the drawing was pulled apart from the deciding first.

#![cfg(test)]

use std::sync::Arc;

use bevy::prelude::*;

use crate::sim::Sim;

/// A boat at sea with nothing else running: the smallest world a system can be asked a
/// question in.
fn at_sea() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(Sim::new(crate::weather::Weather::draw(42)));
    app.insert_resource(crate::sound::SoundHandle(Arc::new(
        crate::sound::SeaParams::default(),
    )));
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<crate::input::HelmSource>();
    app.init_resource::<crate::input::Pilot>();
    app.init_resource::<crate::crew::Crew>();
    app
}

/// Press a key for exactly one frame, the way a hand does.
///
/// Bevy's input plugin is what normally clears `just_pressed`, and it is not here — so
/// the clearing is done by hand, which also makes *held for one frame* and *held for
/// ten* two different things a test can say.
fn press(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.update();
    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.release(key);
    keys.clear();
}

#[cfg(test)]
mod the_deck {
    use super::*;

    fn sailing() -> App {
        let mut app = at_sea();
        app.add_systems(Update, crate::input::gather);
        // Sails set and drawing, which is the state most of the deck assumes: a boat
        // with her headsail rolled away refuses half these keys on purpose, and a
        // scenario that did not say so would be testing the refusal.
        let mut sim = app.world_mut().resource_mut::<Sim>();
        sim.controls.main_sheet = 0.3;
        sim.controls.jib_sheet = 0.3;
        sim.controls.furl = 0.0;
        app
    }

    /// **G backs the genoa, and backs it once.**
    ///
    /// The scenario that would have found the fault that cost five controls: a block of
    /// `gather` pasted twice read every key in it twice on the same frame, so a toggle
    /// never moved and a rate ran at double speed. Nothing about that is a type error
    /// and `clippy` is happy with it; one press and one assertion is the whole test.
    #[test]
    fn g_backs_the_genoa_once() {
        let mut app = sailing();
        assert!(!app.world().resource::<Sim>().controls.headsail_aback);
        press(&mut app, KeyCode::KeyG);
        assert!(
            app.world().resource::<Sim>().controls.headsail_aback,
            "one press backs it"
        );
        press(&mut app, KeyCode::KeyG);
        assert!(
            !app.world().resource::<Sim>().controls.headsail_aback,
            "and the next press sheets it across"
        );
    }

    /// **X and Z walk the wardrobe one sail at a time.**
    ///
    /// The other half of the same fault: with the block read twice, one press stepped
    /// two sails and the Code 0 could not be reached at all.
    #[test]
    fn x_steps_one_sail() {
        let mut app = sailing();
        {
            // Only with the headsail rolled away, which is the rule on a boat.
            let mut sim = app.world_mut().resource_mut::<Sim>();
            sim.controls.furl = 1.0;
        }
        press(&mut app, KeyCode::KeyX);
        assert_eq!(
            app.world().resource::<Sim>().controls.headsail,
            sail_core::state::Headsail::Code0,
            "one press, one sail"
        );
    }
}

#[cfg(test)]
mod the_watch_on_deck {
    use super::*;
    use sail_core::units::{deg, rad};

    /// The course the test holds her on while the crew works.
    #[derive(Resource)]
    struct HeldOn(f64);

    /// What is under test is the crew, not the steering, so the helm is the test's.
    /// Left to herself with the sheets flogging she rounds up and the question becomes
    /// a different one.
    fn held(mut sim: ResMut<Sim>, on: Res<HeldOn>) {
        sim.sim.state.heading = on.0;
        sim.sim.state.yaw_rate = 0.0;
    }

    /// A boat sailing, with the hands called on deck and nothing else running.
    fn with_a_crew(heading: f64) -> App {
        let mut app = at_sea();
        app.init_resource::<crate::clutch::Bank>();
        app.insert_resource(HeldOn(heading));
        // **A tenth of a second a frame, instead of whatever the wall clock says.**
        //
        // Headless frames take microseconds, so a system whose rate is per *second*
        // does nothing at all in a thousand of them: the crew hauling at half a sheet a
        // second moved the mainsheet by eight hundredths in what should have been ten
        // minutes. Bevy has the knob for exactly this, and a test about how long a job
        // takes had better be the thing holding the clock.
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(100),
        ));
        app.add_systems(
            Update,
            (held, crate::sim::advance, crate::crew::work).chain(),
        );
        {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            // Sheets flogging right out and the vang off: everything wrong, which is
            // what a crew is for.
            sim.sim.state.heading = heading;
            sim.controls.main_sheet = 0.9;
            sim.controls.jib_sheet = 0.9;
            sim.controls.vang = 0.0;
            sim.controls.furl = 0.0;
        }
        app.world_mut().resource_mut::<crate::crew::Crew>().standing = true;
        app
    }

    /// **The watch trims the sheets, and does it at a winch's pace.**
    ///
    /// Both halves matter. A crew that does not trim is a switch that does nothing; a
    /// crew that arrives at the answer in one frame is a cheat, and through a tack the
    /// difference is visible from the cockpit.
    #[test]
    fn the_hands_haul_the_sheets_in_and_take_time_over_it() {
        let mut app = with_a_crew(rad(45.0));
        app.update();
        let after_one = app.world().resource::<Sim>().controls.main_sheet;
        assert!(
            after_one > 0.8,
            "one frame took the mainsheet from 0.9 to {after_one:.2}, which is a winch \
             nobody is turning"
        );
        for _ in 0..300 {
            app.update();
        }
        let sim = app.world().resource::<Sim>();
        // Against the trim a crew is actually chasing, which is the *bent* one: the
        // headsail's downwash takes several degrees off the wind the mainsail meets,
        // and a main trimmed to the masthead fly is eased too far by exactly that.
        let t = sim.sim.telemetry;
        let (want, _) = sail_core::autopilot::optimal_sheets_bent(
            &sim.config(),
            t.awa,
            sim.controls.outhaul,
            sim.controls.headsail,
            t.bend_main,
            t.bend_jib,
        );
        assert!(
            (sim.controls.main_sheet - want).abs() < 0.08,
            "the watch left the mainsheet at {:.2} with {:.2} wanted at {:.0} deg apparent",
            sim.controls.main_sheet,
            want,
            deg(sim.sim.telemetry.awa)
        );
    }

    /// **And they go to the clutch bank for the shape controls**, one line at a time,
    /// because there is one winch on the coachroof and a crew is subject to it too.
    #[test]
    fn the_vang_comes_on_without_anybody_asking() {
        // Off the wind, which is where the vang is the whole leech.
        let mut app = with_a_crew(rad(200.0));
        for _ in 0..400 {
            app.update();
        }
        let sim = app.world().resource::<Sim>();
        assert!(
            sim.controls.vang > 0.5,
            "the vang is still at {:.2} with the boom out at {:.0} deg apparent",
            sim.controls.vang,
            deg(sim.sim.telemetry.awa)
        );
    }

    /// **They change the headsail, and they do it in the right order.**
    ///
    /// The whole job, not the setting: the genoa rolled away first because nothing is
    /// bent on with the bow full of sail, then the new one on the sprit, then set. This
    /// is the wiring test for it — the resource, the clutch bank and the rule the
    /// keyboard obeys all have to agree, and a state machine that decides afresh every
    /// frame would get stuck half way through with the drum turning both ways.
    #[test]
    fn the_watch_rolls_the_genoa_away_before_it_sets_a_kite() {
        // Running in a working breeze, which is where a kite belongs and a genoa is in
        // the mainsail's shadow doing nothing.
        let mut app = with_a_crew(rad(185.0));
        let mut rolled_away = false;
        for _ in 0..900 {
            app.update();
            let sim = app.world().resource::<Sim>();
            if sim.controls.headsail == sail_core::state::Headsail::Genoa {
                // While the genoa is still the sail, the only thing that may have
                // happened to it is being rolled up.
                rolled_away |= sim.controls.furl > 0.98;
            } else {
                assert!(
                    rolled_away,
                    "a sail was bent on with the genoa still out: furl {:.2}",
                    sim.controls.furl
                );
            }
        }
        let sim = app.world().resource::<Sim>();
        assert!(
            sim.controls.headsail.flying(),
            "running in a breeze and still under the genoa"
        );
        assert!(
            sim.controls.furl < 0.05,
            "the new sail was bent on and left in its bag: furl {:.2}",
            sim.controls.furl
        );
    }

    /// And with the watch stood down they touch nothing at all, which is the other half
    /// of a switch meaning anything.
    #[test]
    fn stood_down_they_do_not_touch_a_rope() {
        let mut app = with_a_crew(rad(45.0));
        app.world_mut().resource_mut::<crate::crew::Crew>().standing = false;
        for _ in 0..200 {
            app.update();
        }
        let sim = app.world().resource::<Sim>();
        assert_eq!(sim.controls.main_sheet, 0.9);
        assert_eq!(sim.controls.vang, 0.0);
    }
}

#[cfg(test)]
mod the_chart_table {
    use super::*;
    use crate::chart::{Chart, DeckLog, Plotted, Wrote};
    use crate::navigation::Navigation;

    /// A coast with two marks on it, far enough apart to cross squarely.
    fn with_two_marks(app: &mut App) -> (glam::DVec2, glam::DVec2) {
        let one = glam::DVec2::new(-2_000.0, 3_000.0);
        let two = glam::DVec2::new(2_400.0, 2_600.0);
        app.insert_resource(crate::marks::Coast {
            marks: vec![
                sail_core::land::Landmark {
                    at: one,
                    name: "Bergskerry Spire".into(),
                    kind: sail_core::land::Mark::Spire,
                    ground: 20.0,
                    height: 40.0,
                },
                sail_core::land::Landmark {
                    at: two,
                    name: "Nelstad".into(),
                    kind: sail_core::land::Mark::Lighthouse,
                    ground: 10.0,
                    height: 30.0,
                },
            ],
            ..default()
        });
        (one, two)
    }

    fn bearing_to(from: glam::DVec2, mark: glam::DVec2) -> f64 {
        let off = mark - from;
        off.x.atan2(off.y).rem_euclid(std::f64::consts::TAU)
    }

    /// **Entering a fix leaves a line in the book and a mark on the paper.**
    ///
    /// Which is a question about four resources agreeing on one frame, and it has been
    /// wrong twice: once when the mark went on the chart through a condition another
    /// system had not written yet — so the fix appeared with no time against it — and
    /// once when the lines were rubbed out before anybody had decided whether the cross
    /// was good enough to keep. Neither is visible in any one function.
    #[test]
    fn a_fix_leaves_a_line_in_the_book_and_a_mark_on_the_paper() {
        let mut app = at_sea();
        let (one, two) = with_two_marks(&mut app);
        app.init_resource::<Chart>();
        app.init_resource::<DeckLog>();
        app.init_resource::<Navigation>();
        app.init_resource::<crate::sky::Sky>();
        app.add_systems(Update, crate::navigation::enter_in_the_log);
        let here = app.world().resource::<Sim>().sim.state.pos;
        {
            let mut nav = app.world_mut().resource_mut::<Navigation>();
            nav.lost = true;
            for mark in [one, two] {
                nav.sights.push(sail_core::fix::Sight {
                    mark,
                    bearing: bearing_to(here, mark),
                    ran: glam::DVec2::ZERO,
                });
            }
        }
        // The navigator says he believes the crossing.
        app.world_mut().resource_mut::<Chart>().accepted = Some(here);
        app.update();

        let chart = app.world().resource::<Chart>();
        let log = app.world().resource::<DeckLog>();
        let nav = app.world().resource::<Navigation>();
        assert!(
            nav.fixed_at.is_some(),
            "two good bearings are a position she may keep"
        );
        assert!(
            chart
                .plot
                .iter()
                .any(|item| matches!(item, Plotted::Entry(_))),
            "and the position goes on the paper with its time"
        );
        assert!(
            log.lines
                .iter()
                .any(|line| matches!(line.wrote, Wrote::Entered(_))),
            "and in the book, which is where the rest of the line lives"
        );
        assert!(
            nav.sights.is_empty(),
            "the working lines come off once there is a fix to leave in their place"
        );
    }

    /// **A fix that will not close leaves the navigator his lines.**
    ///
    /// The other half of the same frame, and it was wrong the other way: the bearings
    /// were rubbed out before the cross had been judged, so an attempt that was not
    /// good enough took away the work and gave nothing back — and the answer to a fix
    /// that will not close is a third bearing laid on top of the two you have.
    #[test]
    fn a_fix_that_will_not_close_keeps_the_working_lines() {
        let mut app = at_sea();
        let (one, two) = with_two_marks(&mut app);
        app.init_resource::<Chart>();
        app.init_resource::<DeckLog>();
        app.init_resource::<Navigation>();
        app.init_resource::<crate::sky::Sky>();
        app.add_systems(Update, crate::navigation::enter_in_the_log);
        let here = app.world().resource::<Sim>().sim.state.pos;
        {
            let mut nav = app.world_mut().resource_mut::<Navigation>();
            nav.lost = true;
            for mark in [one, two] {
                nav.sights.push(sail_core::fix::Sight {
                    mark,
                    bearing: bearing_to(here, mark),
                    ran: glam::DVec2::ZERO,
                });
            }
        }
        // A cross a mile from where she is, which is not a position.
        app.world_mut().resource_mut::<Chart>().accepted =
            Some(here + glam::DVec2::new(1_852.0, 0.0));
        app.update();

        let nav = app.world().resource::<Navigation>();
        assert!(nav.fixed_at.is_none(), "she is still lost");
        assert_eq!(nav.sights.len(), 2, "and still has her two bearings");
    }

    /// **Two bearings written in the book and laid on the chart are a fix.**
    ///
    /// The whole path the deck uses, in one test: the glasses name a mark, the compass
    /// writes a line, a hand at the table lays it on the paper, and the two crossings
    /// make a position the button can enter. Reported from the deck as the compass
    /// having stopped making fixes, so the question is which half it is — this is the
    /// half that does not involve a mouse.
    #[test]
    fn two_bearings_laid_on_from_the_book_make_a_fix() {
        let mut app = at_sea();
        let (one, two) = with_two_marks(&mut app);
        app.init_resource::<DeckLog>();
        app.init_resource::<Navigation>();
        let here = app.world().resource::<Sim>().sim.state.pos;
        let mut log = DeckLog::default();
        for (name, mark) in [("Bergskerry Spire", one), ("Nelstad", two)] {
            log.write(
                6.5,
                Wrote::Bearing {
                    name: name.into(),
                    mark,
                    bearing: bearing_to(here, mark),
                    taken: glam::DVec2::ZERO,
                },
            );
        }
        let mut nav = Navigation {
            lost: true,
            ..default()
        };
        let mut laid_at = None;
        crate::chart::lay_it_on_the_chart(&mut log, &mut nav, &mut laid_at, 0);
        crate::chart::lay_it_on_the_chart(&mut log, &mut nav, &mut laid_at, 1);
        assert_eq!(nav.sights.len(), 2, "both lines are on the paper");
        let got = sail_core::fix::plot(&nav.sights).expect("two lines that cross are a fix");
        assert!(
            (got.at - here).length() < 1.0,
            "and they cross where she is, {:.0} m out",
            (got.at - here).length()
        );
    }
}
