//! What the trysail is for, measured.
//!
//! The sail exists to answer a question that had no answer: **what is she under when the
//! main has come off her?** Three slabs and then handed is a real sequence and the model
//! had it, but the step after it was a boat with nothing aft at all — a staysail forward,
//! a hull, and a rudder trying to steer something that wants to lie across the sea.
//!
//! What came out of running it, in forty-two knots with the headsail rolled right away
//! and a pilot holding the course:
//!
//! * **A reefed main at this wind is not a sail, it is a liability.** Under the third
//!   slab she rounds up and stays there: eighty-eight degrees off her course on a close
//!   reach and fifty-eight on a beam reach, at two knots, with the pilot hard over and
//!   unable to do anything about it. The second slab is the same. That is the whole
//!   reason a trysail exists, and the model found it without being told.
//! * **Under the trysail she sails.** Seven knots on the same close reach against six
//!   and a half under bare poles, dead on her course; three and a tenth on the beam
//!   against two and six, and twenty-two degrees off instead of twenty-five.
//! * **At about the same heel.** Twenty degrees with the trysail against twenty-two
//!   under the second reef — so the comparison is not that it heels her less, it is that
//!   at the same heel she is being sailed instead of being lain over.

use sail_core::boat::{BoatConfig, DROPPED};
use sail_core::sim::{Simulation, SUBSTEP};
use sail_core::state::{BoatState, Controls};
use sail_core::units::{deg, kn_to_ms, ms_to_kn, rad};
use sail_core::wind::SteadyWind;

/// Storm canvas at a true wind angle, in a gale, after she has settled.
struct UnderIt {
    /// Speed through the water, knots.
    log: f64,
    /// Heel, degrees.
    heel: f64,
    /// How far off the course she was given she ended up, degrees.
    off_course: f64,
}

fn sail_under(trysail: f64, reef: f64, twa: f64, knots: f64) -> UnderIt {
    use sail_core::autopilot::TillerPilot;

    let cfg = BoatConfig::archipelago_packet();
    // Pointed so that the wind is `twa` off her bow, with the wind from the north.
    let heading = rad(180.0 - twa);
    let state = BoatState {
        surge: kn_to_ms(2.0),
        ..BoatState::at_rest(heading)
    };
    let wind = SteadyWind::new(0.0, kn_to_ms(knots));
    let mut sim = Simulation::new(cfg, state, Box::new(wind));
    let mut controls = Controls {
        // Everything off her: the main handed, the headsail rolled right away. This is
        // the state the sail exists for.
        reef,
        furl: 1.0,
        staysail: 0.0,
        trysail,
        // Trimmed rather than strapped flat: a storm sail is sheeted hard by the
        // standards of a reach and it is still *trimmed*. Amidships it is a plate and
        // makes drag, which was the first thing this measured.
        main_sheet: 0.5,
        ..Default::default()
    };
    let mut pilot = TillerPilot::new();
    for _ in 0..(240.0 / SUBSTEP) as usize {
        controls.rudder = pilot.rudder(&sim.config, &sim.state, heading, SUBSTEP);
        sim.run(SUBSTEP, &controls);
    }
    UnderIt {
        log: ms_to_kn(sim.telemetry.stw),
        heel: deg(sim.state.heel).abs(),
        off_course: deg(sail_core::units::wrap_pi(sim.state.heading - heading)).abs(),
    }
}

/// **She sails under it, and under bare poles she does not.**
///
/// The number that matters is not the speed, it is that there is drive at all and that
/// it is hers. A boat with no sail set is a boat going where the sea puts her.
#[test]
fn a_trysail_gets_her_moving_when_nothing_else_is_up() {
    let bare = sail_under(0.0, 1.0, 60.0, 42.0);
    let storm = sail_under(1.0, 1.0, 60.0, 42.0);
    assert!(
        storm.log > bare.log + 0.4,
        "bare poles {:.2} kn, trysail {:.2} kn — the sail bought nothing",
        bare.log,
        storm.log
    );
}

/// **And this is why the main comes off her first.**
///
/// The striking result, and the one that says the model has the seamanship in it rather
/// than a table of areas: in forty-two knots a reefed mainsail does not slow her down,
/// it takes the boat away from the person steering. Under the third slab she rounds up
/// and stays there with the helm hard over. Under four and a half square metres of
/// trysail she holds the course she was given.
#[test]
fn a_reefed_main_rounds_her_up_and_the_trysail_does_not() {
    let third = sail_under(0.0, 0.97, 60.0, 42.0);
    let storm = sail_under(1.0, 1.0, 60.0, 42.0);
    assert!(
        third.off_course > 45.0,
        "the third reef held her to {:.0} degrees in a gale, which is not what it does",
        third.off_course
    );
    assert!(
        storm.off_course < 10.0,
        "she wandered {:.0} degrees off under the trysail",
        storm.off_course
    );
}

/// **Across the wind it is the difference between steering and being steered.** Less
/// dramatic than the reefed main, and the same fact: something abaft the mast is what
/// gives the rudder anything to work against.
///
/// And it is bought at a heel she can carry — twenty degrees against the twenty-two the
/// second reef lies her over at. The trysail is not the sail that heels her less; it is
/// the sail that, at the same heel, is being sailed.
#[test]
fn it_holds_her_across_the_wind_better_than_bare_poles() {
    let bare = sail_under(0.0, 1.0, 90.0, 42.0);
    let storm = sail_under(1.0, 1.0, 90.0, 42.0);
    assert!(
        storm.off_course < bare.off_course,
        "she held {:.0} degrees off under bare poles and {:.0} under the trysail",
        bare.off_course,
        storm.off_course
    );
    assert!(storm.log > bare.log, "and she was no faster for it either");
    let reefed = sail_under(0.0, 0.75, 90.0, 42.0);
    assert!(
        storm.heel < reefed.heel + 1.0,
        "the trysail heeled her {:.0} degrees against the second reef's {:.0}",
        storm.heel,
        reefed.heel
    );
}

/// **It will not set with the mainsail bent on**, because they share a track. The
/// control moves and nothing happens, which is what would happen on deck.
#[test]
fn it_does_nothing_until_the_main_is_off_her() {
    let cfg = BoatConfig::archipelago_packet();
    // Two slabs in: a long way from handed, and the sail is still in its bag.
    let (area, _) = cfg.trysail_drawing(0.5, 1.0);
    assert_eq!(area, 0.0, "it set itself over a reefed mainsail");
    let (area, _) = cfg.trysail_drawing(DROPPED, 1.0);
    assert!(area > 1.0, "it did not set with the main off her");
    // And it is a storm sail's size rather than a fifth reef: a tenth or so of the main.
    assert!(
        area < cfg.main.area * 0.2,
        "a {:.1} m² trysail against a {:.1} m² main is not a trysail",
        area,
        cfg.main.area
    );
}
