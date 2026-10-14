//! What the wind vane does when the sea is trying to gybe her.
//!
//! Written to answer a question from the deck rather than to defend a number: *perhaps
//! the sea will stop gybing us under spinnaker alone now that the vane steers the true
//! wind downwind.* It does, and two of the things I thought I had found on the way did
//! not survive being measured more than once, which is most of the value of the file.
//!
//! * **No gybe in any case measured** — fourteen and twenty knots, two hundred
//!   kilometres of fetch, seven minutes of it, four seas each, under kite alone with no
//!   mainsail up. Steering the true wind deep is what buys that: apparent wind angle on a
//!   run is mostly the boat's own speed, so a vane chasing it yaws her with the waves
//!   instead of across them, and that is how the wind gets behind the main.
//! * **In a seaway she wanders a long way off her order and that is the sea, not the
//!   gear.** Twelve to twenty-five degrees under kite in an ocean fetch, against three to
//!   five in sheltered water, and single runs that end up forty degrees from where they
//!   started. The first run I looked at walked her from a hundred and fifty degrees of
//!   true wind onto a beam reach and I wrote down that a kite with no mainsail cannot
//!   hold a run; over ten seas the mainsail is worth a degree and a half of wander and
//!   nothing at all in holding the order. One run of a chaotic thing is an anecdote.
//! * **And the pilot looked like it was throwing away half its rudder**, going to the
//!   mechanical stop at forty degrees where the blade is past its stall and making half
//!   the side force it had at twenty-eight. Holding it at the stall instead was better in
//!   one case out of four and worse in two, over five seas: a rudder hard over past its
//!   stall is also a brake right aft, and braking the stern is half of what stops a boat
//!   being slewed by a following sea. The reasoning was good and the measurement said no.

use sail_core::autopilot::{optimal_sheets_with, Autopilot, PilotMode, TillerPilot};
use sail_core::boat::BoatConfig;
use sail_core::sim::{Simulation, SUBSTEP};
use sail_core::state::{BoatState, Controls, Headsail};
use sail_core::units::{deg, kn_to_ms, rad, wrap_pi, wrap_tau};
use sail_core::water::WindSea;
use sail_core::wind::SteadyWind;

struct Run {
    gybes: u32,
    worst: f64,
    held: f64,
    settled: f64,
    stops: f64,
}

/// Running under spinnaker alone with the vane steering, in whatever sea the fetch makes.
fn run_deep(tws_knots: f64, fetch: f64, twa: f64, seconds: f64, seed: u64, reef: f64) -> Run {
    let chase = std::env::var("CHASE").is_ok();
    let cfg = BoatConfig::charter_sloop_34();
    let heading = wrap_tau(0.0 - twa);
    let mut state = BoatState::at_rest(heading);
    state.surge = 0.6 * cfg.hull_speed();
    let sea = WindSea::new(kn_to_ms(tws_knots), fetch, 0.0, seed);
    let mut sim = Simulation::new(
        cfg,
        state,
        Box::new(SteadyWind::new(0.0, kn_to_ms(tws_knots))),
    )
    .with_water(Box::new(sea));
    // The mainsail is off her: the last position on the reef control is the sail coming
    // down, not a third slab.
    let mut controls = Controls {
        reef,
        headsail: Headsail::Spinnaker,
        furl: 0.0,
        ..Default::default()
    };
    let mut hand = TillerPilot::new();
    let mut pilot = Autopilot::new();

    let mut gybes = 0;
    let mut worst = 0.0_f64;
    let mut was = sim.telemetry.twa.signum();
    let mut held = 0.0;
    let mut twa_sum = 0.0_f64;
    let mut twa_n = 0.0_f64;
    let mut at_the_stop = 0.0_f64;
    for i in 0..(seconds / SUBSTEP) as u64 {
        // Trimmed for the course she is on, and then left alone, which is what a crew
        // does: a kite is not re-trimmed ten times a second to whatever the last wave did
        // to the apparent wind. Chasing it is available for comparison, and it is what
        // turned out to broach her.
        if chase || (i as f64 * SUBSTEP) < 20.0 {
            let (m, j) =
                optimal_sheets_with(&cfg, sim.telemetry.awa, controls.outhaul, controls.headsail);
            controls.main_sheet = m;
            controls.jib_sheet = j;
        }
        let t = i as f64 * SUBSTEP;
        if t < 90.0 {
            controls.rudder = hand.rudder(&cfg, &sim.state, heading, SUBSTEP);
        } else {
            if t >= 90.0 && !pilot.engaged() {
                pilot.hold_wind(&sim.telemetry);
                held = match pilot.mode {
                    PilotMode::ApparentWind { twa, .. } => twa,
                    _ => 0.0,
                };
            }
            if let Some(r) = pilot.rudder(&cfg, &sim.state, &sim.telemetry, SUBSTEP) {
                controls.rudder = r;
            }
        }
        sim.run(SUBSTEP, &controls);
        if true {
            let twa = sim.telemetry.twa;
            // The wind crossing the stern is a gybe; the sign of the angle flipping
            // anywhere deep is the only way that can happen.
            if twa.signum() != was && twa.abs() > rad(120.0) {
                gybes += 1;
            }
            was = twa.signum();
            if t > 90.0 {
                worst = worst.max(wrap_pi(twa.abs() - held.abs()).abs());
                twa_sum += twa.abs();
                twa_n += 1.0;
                // Hard over by a helmsman's standard rather than by the fitting's: a
                // rudder past its own stall angle is making less force than it was at
                // twenty-eight degrees, so that is where "no more helm available" is.
                if controls.rudder.abs() > cfg.rudder.stall_angle - rad(0.5) {
                    at_the_stop += SUBSTEP;
                }
            }
        }
    }
    Run {
        gybes,
        worst,
        held,
        settled: twa_sum / twa_n.max(1.0),
        stops: at_the_stop / (seconds - 90.0),
    }
}

/// Under spinnaker alone, with the vane steering, the sea does not gybe her.
///
/// Which is the question that was asked, and the answer is the same in a summer breeze
/// and in twenty knots with the whole North Sea behind it. A gybe here is the wind
/// crossing the stern: the sign of the true wind angle changing while she is deep, which
/// is the only way it can happen.
#[test]
fn the_sea_does_not_gybe_her_under_spinnaker_alone() {
    for (tws, twa) in [(14.0, 150.0), (14.0, 165.0), (20.0, 150.0), (20.0, 165.0)] {
        for seed in [3u64, 17, 31, 61] {
            let r = run_deep(tws, 200_000.0, rad(twa), 420.0, seed, 1.0);
            println!(
                "tws {tws:>4.0} twa {twa:>5.0} sea {seed:>3}: told {:>5.1}, sailed {:>5.1}, \
             gybes {}, \
             wander {:>5.1}, helm at its limit {:>4.0}%",
                deg(r.held),
                deg(r.settled),
                r.gybes,
                deg(r.worst),
                r.stops * 100.0
            );
            assert_eq!(
                r.gybes, 0,
                "the sea gybed her {} times at {tws:.0} knots on {twa:.0} degrees of true \
             wind in sea {seed}, under a kite with no mainsail up",
                r.gybes
            );
        }
    }
}

/// And in water that is not trying to throw her about, it holds what it was told.
///
/// The control for the test above: whatever the vane does in a seaway, in sheltered
/// water and in a light air it has to hold the angle it was set to, or the gear is not
/// steering at all and the absence of gybes means nothing.
#[test]
fn the_vane_holds_what_it_was_told_when_the_water_lets_it() {
    for (tws, fetch, twa) in [
        (14.0, 200.0, 150.0),
        (14.0, 200.0, 165.0),
        (8.0, 200_000.0, 150.0),
    ] {
        let r = run_deep(tws, fetch, rad(twa), 300.0, 17, 1.0);
        println!(
            "tws {tws:>4.0} fetch {fetch:>8.0} twa {twa:>5.0}: told {:>5.1}, sailed {:>5.1}, \
             wander {:>5.1}",
            deg(r.held),
            deg(r.settled),
            deg(r.worst)
        );
        assert!(
            wrap_pi(r.settled - r.held).abs() < rad(6.0),
            "told {:.1} and sailed {:.1} in water that is not an excuse",
            deg(r.held),
            deg(r.settled)
        );
        assert!(
            r.stops < 0.02,
            "the helm was at its limit {:.0}% of the time in sheltered water, which is a \
             pilot that is tuned wrong rather than a boat that is hard pressed",
            r.stops * 100.0
        );
    }
}
