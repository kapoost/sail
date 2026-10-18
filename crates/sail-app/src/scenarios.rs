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
    app.init_resource::<crate::orders::Orders>();
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
    /// two sails and the Code 0 could not be reached at all. What the press does now is
    /// *order* the next sail — the foredeck still has to roll the old one away first —
    /// so the assertion is about what was asked for rather than about what is up.
    #[test]
    fn x_steps_one_sail() {
        let mut app = sailing();
        press(&mut app, KeyCode::KeyX);
        assert_eq!(
            app.world().resource::<crate::orders::Orders>().headsail,
            Some(sail_core::state::Headsail::Code0),
            "one press, one sail"
        );
        press(&mut app, KeyCode::KeyX);
        assert_eq!(
            app.world().resource::<crate::orders::Orders>().headsail,
            Some(sail_core::state::Headsail::Gennaker),
            "and the next press is the next sail, not the one after it"
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

    /// A boat sailing, with the hands on deck and the helm held by the test.
    fn with_a_crew(heading: f64) -> App {
        let mut app = crewed(heading);
        app.add_systems(Update, held.before(crate::sim::advance));
        app
    }

    /// The same boat with nobody on the helm, for the tests that are about steering.
    fn crewed(heading: f64) -> App {
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
            (
                crate::sim::advance,
                crate::orders::carry_out,
                crate::crew::work,
            )
                .chain(),
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
        // **And it is worked home, not to within a tenth of home.**
        //
        // The threshold that decides whether a line is worth a trip to the winch was
        // doing both jobs, so every control on the bank sat up to a tenth of its travel
        // from where it belonged — permanently a little bit wrong, which is the one
        // thing a crew is there to prevent.
        //
        // Not nought, because what the line is being worked *to* moves: the wind shifts
        // while the hands are on another rope, and a crew goes back to it when it is
        // worth the trip again. Half of the old floor is the claim.
        let want = crate::crew::wants(
            crate::clutch::Line::Vang,
            &sim.controls,
            sim.sim.telemetry.awa,
            sim.sim.state.heel,
            sim.sim.telemetry.aws,
            false,
        );
        assert!(
            (sim.controls.vang - want).abs() < 0.05,
            "the vang was left at {:.3} with {want:.3} wanted",
            sim.controls.vang
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

    /// **A sail change does not take everybody forward.**
    ///
    /// The sheets are on their own winches and somebody's hands never leave them. The
    /// first version of the foredeck job returned out of the whole system while it ran,
    /// so for the quarter of a minute a change takes nobody trimmed anything and nobody
    /// would have backed the headsail if she had gone into irons. What a change actually
    /// monopolises is the one winch on the coachroof.
    #[test]
    fn the_sheets_are_still_worked_while_the_foredeck_changes_sail() {
        let mut app = with_a_crew(rad(185.0));
        let mut caught = false;
        for _ in 0..900 {
            app.update();
            let mid_change = {
                let sim = app.world().resource::<Sim>();
                sim.controls.headsail == sail_core::state::Headsail::Genoa
                    && (0.2..0.9).contains(&sim.controls.furl)
            };
            if !mid_change {
                continue;
            }
            // Throw the mainsheet right off in the middle of the job and see whether
            // anybody takes it up again.
            app.world_mut().resource_mut::<Sim>().controls.main_sheet = 0.9;
            app.update();
            let after = app.world().resource::<Sim>().controls.main_sheet;
            assert!(
                after < 0.89,
                "the mainsheet was dumped half way through a sail change and left at \
                 {after:.3} — the whole crew went forward"
            );
            caught = true;
            break;
        }
        assert!(caught, "no sail change happened, so nothing was tested");
    }

    /// **A crew that reefs its way down to bare poles in a Force 5 is not a crew.**
    ///
    /// The one behaviour of this whole system that cannot be judged from reading it:
    /// what it does over a quarter of an hour of real weather, with gusts, a wind that
    /// wanders and a sea running. Measured before it was fixed — in nineteen knots the
    /// watch had the main *handed* and had touched the reefing pennant seven times in
    /// half an hour, because every slab was followed by a trim back to maximum lift that
    /// stood her straight up on her ear again.
    ///
    /// **Run it by hand: `cargo test -- --ignored the_watch_does_not_reef`.**
    ///
    /// Half an hour of weather is a minute of test, and the fault it catches needs the
    /// half hour — at six minutes it does not show at all, which was checked by putting
    /// the old constants back and watching this pass. So it is the measurement harness
    /// it actually is, and the two mechanisms it found are pinned by unit tests in
    /// `crew.rs` that cost nothing and run every time.
    #[test]
    #[ignore]
    fn the_watch_does_not_reef_its_way_to_bare_poles_in_a_working_breeze() {
        let seed = 30; // nineteen and a half knots, which is a reef and not a storm.
        let w = crate::weather::Weather::draw(seed);
        let kn = sail_core::units::ms_to_kn(w.tws);
        assert!(
            (18.0..21.0).contains(&kn),
            "the weather this test is about changed: {kn:.1} kn"
        );
        let mut app = at_sea();
        app.insert_resource(Sim::new(crate::weather::Weather::draw(seed)));
        app.init_resource::<crate::clutch::Bank>();
        // Close-hauled, where the heel is.
        app.insert_resource(HeldOn(w.wind_from + rad(45.0)));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(250),
        ));
        app.add_systems(
            Update,
            (
                held,
                crate::sim::advance,
                crate::orders::carry_out,
                crate::crew::work,
                crate::clutch::handle,
            )
                .chain(),
        );
        app.world_mut().resource_mut::<crate::crew::Crew>().standing = true;
        let (mut changes, mut last, mut deepest) = (0, 0.0f64, 0.0f64);
        for _ in 0..7_200 {
            app.update();
            let reef = app.world().resource::<Sim>().controls.reef;
            deepest = deepest.max(reef);
            if (reef - last).abs() > 0.2 {
                changes += 1;
                last = reef;
            }
        }
        assert!(
            deepest < 0.95,
            "the main was handed in {kn:.0} knots of true wind"
        );
        assert!(
            changes <= 3,
            "{changes} trips to the reefing pennant in half an hour of steady weather"
        );
    }

    /// **The pilot steers and the watch sails: the two run together.**
    ///
    /// The combination nobody had tried, and the one a tired skipper actually wants —
    /// the boat sailing herself while you go below. The hazard is not physics, it is
    /// wiring: `gather` stands the watch down when a hand goes on a sheet, and the pilot
    /// lives in the same system. If one of its keys ever counted as a hand on a rope,
    /// engaging the pilot would quietly fire the crew.
    #[test]
    fn the_pilot_and_the_watch_do_not_put_each_other_out_of_work() {
        // No `held` here: the whole question is whether the pilot steers her, so the
        // test keeps its hands off the helm.
        let mut app = crewed(rad(60.0));
        app.add_systems(Update, crate::input::gather);
        // Way on first. A tiller pilot engaged on a stopped boat has nothing going over
        // the blade and lets go within seconds — correctly, and it is what happened the
        // first time this was written: *it could not hold the course, rudder was hard
        // over*. You do not switch one on until she is sailing.
        for _ in 0..600 {
            app.update();
        }
        let heading = {
            let sim = app.world().resource::<Sim>();
            let state = sim.sim.state;
            assert!(
                sail_core::units::ms_to_kn(sim.sim.telemetry.stw) > 2.0,
                "she never got going, so the pilot was never the thing under test"
            );
            app.world_mut()
                .resource_mut::<crate::input::Pilot>()
                .hold_heading(&state);
            state.heading
        };
        for _ in 0..600 {
            app.update();
        }
        let crew = app.world().resource::<crate::crew::Crew>();
        assert!(crew.standing, "engaging the pilot stood the watch down");
        let sim = app.world().resource::<Sim>();
        assert!(
            app.world().resource::<crate::input::Pilot>().engaged(),
            "the watch stood the pilot down: {:?}",
            app.world().resource::<crate::input::Pilot>().last_reason
        );
        let off = sail_core::units::deg(sail_core::units::wrap_pi(sim.sim.state.heading - heading));
        assert!(
            off.abs() < 20.0,
            "she wandered {off:.0} deg off the course the pilot was holding"
        );
        // And the sails were worked while it steered.
        assert!(
            sim.controls.main_sheet < 0.6,
            "the mainsheet was left flogging at {:.2} with a watch on deck",
            sim.controls.main_sheet
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
mod the_pilotage {
    use super::*;
    use crate::mission::Pilotage;
    use crate::ship::Brig;
    use sail_core::units::{kn_to_ms, rad};

    fn with_a_brig() -> App {
        let mut app = at_sea();
        app.init_resource::<Brig>();
        app.init_resource::<Pilotage>();
        app.init_resource::<crate::barrels::Flotsam>();
        app.init_resource::<crate::boathook::Boathook>();
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(250),
        ));
        app.add_systems(Update, (crate::mission::run, crate::ship::sail_her).chain());
        app
    }

    /// Put her a cable and a half astern of the boat, heading the same way.
    fn standing_in(app: &mut App, astern: f64) -> glam::DVec2 {
        let here = app.world().resource::<Sim>().sim.state.pos;
        let mut brig = app.world_mut().resource_mut::<Brig>();
        brig.afloat = true;
        brig.heading = rad(0.0);
        brig.speed = kn_to_ms(5.0);
        brig.at = here - glam::DVec2::new(0.0, astern);
        brig.bound_for = here + glam::DVec2::new(0.0, 5_000.0);
        let mut job = app.world_mut().resource_mut::<Pilotage>();
        job.running = true;
        // Where she is bound on her own, which is what she steers for until somebody is
        // ahead of her — the real job sets it in `begin`.
        job.bound_for = here + glam::DVec2::new(0.0, 5_000.0);
        job.let_go_at = job.bound_for;
        here
    }

    /// **Being ahead of her is the whole act.** Nobody is transferred and nothing is
    /// pressed: a ship follows a boat she can see in front of her, and the moment the
    /// boat is there she starts steering at the water it has crossed.
    #[test]
    fn she_follows_the_boat_that_is_ahead_of_her() {
        let mut app = with_a_brig();
        let here = standing_in(&mut app, 300.0);
        let was = app.world().resource::<Brig>().bound_for;
        for _ in 0..40 {
            app.update();
        }
        assert!(
            app.world().resource::<Pilotage>().leading,
            "she had the pilot boat three cables ahead and did not take it"
        );
        let now = app.world().resource::<Brig>().bound_for;
        assert!(
            (now - was).length() > 100.0,
            "she is still steering for wherever she was going on her own"
        );
        assert!(
            (now - here).length() < 400.0,
            "she is steering at something that is not where the pilot boat has been"
        );
    }

    /// **The chase, and then the waiting.** Both halves are the manoeuvre: she comes in
    /// from seaward under her own navigation and will not wait for a boat that has not
    /// caught her, and a ship that *has* taken a pilot and then lost sight of him rounds
    /// up and lies there rather than carrying on into a bay she does not know.
    #[test]
    fn she_stands_on_until_she_is_led_and_waits_once_she_has_been() {
        let mut app = with_a_brig();
        // Astern of her: nobody is leading, and nobody has.
        standing_in(&mut app, -200.0);
        let harbour = app.world().resource::<Brig>().bound_for;
        for _ in 0..8 {
            app.update();
        }
        {
            let brig = app.world().resource::<Brig>();
            assert!(
                (brig.bound_for - harbour).length() < 1.0,
                "she stopped before anybody had caught her"
            );
            assert_eq!(brig.set, [1.0, 1.0, 1.0], "and she had all sail set");
        }
        // Now get ahead of her, and then drop back again.
        let here = app.world().resource::<Sim>().sim.state.pos;
        app.world_mut().resource_mut::<Brig>().at = here - glam::DVec2::new(0.0, 300.0);
        for _ in 0..4 {
            app.update();
        }
        assert!(app.world().resource::<Pilotage>().led_once);
        app.world_mut().resource_mut::<Brig>().at = here + glam::DVec2::new(0.0, 300.0);
        for _ in 0..4 {
            app.update();
        }
        let brig = app.world().resource::<Brig>();
        assert!(!app.world().resource::<Pilotage>().leading);
        assert!(
            brig.set[0] < 0.1 && brig.set[2] < 0.1,
            "she carried on under all plain sail with no pilot in sight: {:?}",
            brig.set
        );
    }

    /// And she does not follow a boat under her own stern, however close it is.
    #[test]
    fn a_boat_astern_of_her_is_not_leading_her() {
        let mut app = with_a_brig();
        standing_in(&mut app, -60.0); // behind her transom
        for _ in 0..10 {
            app.update();
        }
        assert!(!app.world().resource::<Pilotage>().leading);
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
