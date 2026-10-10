//! Validation against numbers a sailor knows from the water.
//!
//! The point of an engine-free simulation crate: if one of these fails, the physics is
//! lying, and you know within a second rather than after twenty minutes of sailing.
//!
//! Reference boat is a production charter sloop of about 34 feet. The figures the
//! bounds are drawn around: beats at 40-45 degrees true in 12 knots doing a touch
//! under 6 knots at 15-20 degrees of heel, tacks through about 90 degrees losing a
//! knot or more, and cannot be driven much past its hull speed.

use sail_core::prelude::*;
use sail_core::sail::{
    camber_for_outhaul, optimal_outhaul, sail_forces, SailProfile, CAMBER_REFERENCE,
};
use sail_core::wind::{SteadyWind, WindField};

fn wind(kn: f64) -> impl Fn() -> Box<dyn WindField> {
    move || -> Box<dyn WindField> { Box::new(SteadyWind::new(0.0, kn_to_ms(kn))) }
}

const SETTLE: f64 = 420.0;
const TARGET_HEEL: f64 = 22.0;

// ---------------------------------------------------------------------------
// Upwind performance
// ---------------------------------------------------------------------------

#[test]
fn close_hauled_performance_matches_a_real_cruiser() {
    let cfg = BoatConfig::charter_sloop_34();
    let make = wind(12.0);

    let mut best = (0.0, f64::NEG_INFINITY, 0.0, 0.0);
    for twa in [30.0, 35.0, 38.0, 40.0, 42.0, 45.0, 48.0, 50.0, 55.0] {
        let s = steady_state(cfg, &make, rad(twa), SETTLE, 0.0);
        assert!(s.converged, "TWA {twa} did not converge");
        if s.telemetry.vmg > best.1 {
            best = (twa, s.telemetry.vmg, s.telemetry.stw, s.state.heel);
        }
    }
    let (twa, vmg, stw, heel) = best;

    assert!(
        (38.0..=48.0).contains(&twa),
        "best upwind TWA should be 38-48 deg, got {twa}"
    );
    assert!(
        (5.2..=6.5).contains(&ms_to_kn(stw)),
        "upwind boat speed in 12 kn should be 5.2-6.5 kn, got {:.2}",
        ms_to_kn(stw)
    );
    assert!(
        (3.6..=4.6).contains(&ms_to_kn(vmg)),
        "upwind VMG in 12 kn should be 3.6-4.6 kn, got {:.2}",
        ms_to_kn(vmg)
    );
    assert!(
        (14.0..=24.0).contains(&deg(heel).abs()),
        "heel close-hauled in 12 kn should be 14-24 deg, got {:.1}",
        deg(heel).abs()
    );
}

#[test]
fn pinching_costs_velocity_made_good() {
    let cfg = BoatConfig::charter_sloop_34();
    let make = wind(12.0);
    let pinched = steady_state(cfg, &make, rad(30.0), SETTLE, 0.0);
    let proper = steady_state(cfg, &make, rad(42.0), SETTLE, 0.0);
    assert!(
        pinched.telemetry.vmg < proper.telemetry.vmg,
        "pointing at 30 deg must lose VMG against 42 deg: {:.2} vs {:.2} kn",
        ms_to_kn(pinched.telemetry.vmg),
        ms_to_kn(proper.telemetry.vmg)
    );
    assert!(pinched.telemetry.stw < proper.telemetry.stw);
}

#[test]
fn leeway_upwind_is_a_few_degrees_not_a_crab() {
    let cfg = BoatConfig::charter_sloop_34();
    let s = steady_state(cfg, wind(12.0), rad(42.0), SETTLE, 0.0);
    let leeway = deg(s.telemetry.leeway).abs();
    assert!(
        (1.0..=6.0).contains(&leeway),
        "upwind leeway should be 1-6 deg, got {leeway:.2}"
    );
}

// ---------------------------------------------------------------------------
// Downwind and the hull-speed wall
// ---------------------------------------------------------------------------

#[test]
fn broad_reach_beats_a_dead_run() {
    let cfg = BoatConfig::charter_sloop_34();
    let make = wind(12.0);
    let reach = steady_state(cfg, &make, rad(150.0), SETTLE, 0.0);
    let run = steady_state(cfg, &make, rad(180.0), SETTLE, 0.0);
    assert!(
        reach.telemetry.stw > run.telemetry.stw,
        "150 deg must be faster than dead downwind: {:.2} vs {:.2} kn",
        ms_to_kn(reach.telemetry.stw),
        ms_to_kn(run.telemetry.stw)
    );
}

#[test]
fn displacement_hull_cannot_be_driven_past_its_wall() {
    let cfg = BoatConfig::charter_sloop_34();
    let hull = ms_to_kn(cfg.hull_speed());
    // Hardest driving angle, full sail, a lot of wind.
    for twa in [90.0, 110.0, 120.0] {
        let s = steady_state(cfg, wind(30.0), rad(twa), SETTLE, 0.0);
        let speed = ms_to_kn(s.telemetry.stw);
        assert!(
            speed < hull * 1.25,
            "TWA {twa} in 30 kn reached {speed:.2} kn against a {hull:.2} kn hull speed"
        );
    }
}

#[test]
fn more_wind_gives_more_speed_until_it_does_not() {
    let cfg = BoatConfig::charter_sloop_34();
    let mut previous = 0.0;
    for tws in [6.0, 8.0, 12.0, 16.0, 20.0] {
        let make = wind(tws);
        let (_, s) = reef_for_heel(cfg, &make, rad(90.0), SETTLE, rad(TARGET_HEEL));
        let speed = ms_to_kn(s.telemetry.stw);
        assert!(
            speed > previous,
            "{tws} kn gave {speed:.2} kn, no better than the step below at {previous:.2}"
        );
        previous = speed;
    }
}

// ---------------------------------------------------------------------------
// Trim: the core loop
// ---------------------------------------------------------------------------

#[test]
fn oversheeting_stalls_the_sail() {
    let profile = SailProfile::mainsail(32.0, 12.5);
    let awa = rad(28.0);
    let aws = kn_to_ms(16.0);
    let optimal = profile.optimal_sheet(awa, CAMBER_REFERENCE);

    let good = sail_forces(
        &profile,
        awa,
        aws,
        optimal,
        32.0,
        0.0,
        0.0,
        CAMBER_REFERENCE,
    );
    // Sheeted right in, so the angle of attack is the full apparent wind angle.
    let tight = sail_forces(&profile, awa, aws, 0.0, 32.0, 0.0, 0.0, CAMBER_REFERENCE);

    assert!(
        tight.drive < good.drive,
        "oversheeting must cost drive: {:.0} N tight vs {:.0} N optimal",
        tight.drive,
        good.drive
    );
    assert!(
        tight.coeffs.stalled > 0.0 || tight.alpha > profile.alpha_opt,
        "sheeted hard in at 28 deg apparent, the sail should be past its best angle"
    );
}

#[test]
fn easing_past_the_apparent_wind_makes_the_sail_luff() {
    let profile = SailProfile::mainsail(32.0, 12.5);
    let awa = rad(40.0);
    let aws = kn_to_ms(14.0);
    let optimal = profile.optimal_sheet(awa, CAMBER_REFERENCE);

    let drawing = sail_forces(
        &profile,
        awa,
        aws,
        optimal,
        32.0,
        0.0,
        0.0,
        CAMBER_REFERENCE,
    );
    // Eased well past the apparent wind: the sail is now flogging.
    let flogging = sail_forces(&profile, awa, aws, 0.95, 32.0, 0.0, 0.0, CAMBER_REFERENCE);

    assert!(
        flogging.coeffs.luffing > 0.9,
        "sail should be fully luffing"
    );
    assert!(
        flogging.drive < 0.25 * drawing.drive,
        "a luffing sail must lose most of its drive: {:.0} N vs {:.0} N",
        flogging.drive,
        drawing.drive
    );
    assert!(drawing.coeffs.luffing < 0.01);
}

#[test]
fn drive_peaks_near_the_optimal_angle_of_attack() {
    let profile = SailProfile::genoa(32.0, 13.2);
    let awa = rad(35.0);
    let aws = kn_to_ms(15.0);

    let mut best_sheet = 0.0;
    let mut best_drive = f64::NEG_INFINITY;
    for i in 0..=100 {
        let sheet = i as f64 / 100.0;
        let f = sail_forces(&profile, awa, aws, sheet, 32.0, 0.0, 0.0, CAMBER_REFERENCE);
        if f.drive > best_drive {
            best_drive = f.drive;
            best_sheet = sheet;
        }
    }
    let predicted = profile.optimal_sheet(awa, CAMBER_REFERENCE);
    assert!(
        (best_sheet - predicted).abs() < 0.06,
        "the trim reference should find the real optimum: swept {best_sheet:.2}, predicted {predicted:.2}"
    );
}

#[test]
fn heel_costs_drive() {
    let profile = SailProfile::mainsail(32.0, 12.5);
    let awa = rad(30.0);
    let aws = kn_to_ms(18.0);
    let sheet = profile.optimal_sheet(awa, CAMBER_REFERENCE);
    let upright = sail_forces(&profile, awa, aws, sheet, 32.0, 0.0, 0.0, CAMBER_REFERENCE);
    let heeled = sail_forces(
        &profile,
        awa,
        aws,
        sheet,
        32.0,
        rad(-30.0),
        0.0,
        CAMBER_REFERENCE,
    );
    assert!(heeled.drive < upright.drive);
    assert!(heeled.side.abs() < upright.side.abs());
}

// ---------------------------------------------------------------------------
// Steering
// ---------------------------------------------------------------------------

#[test]
fn rudder_stalls_past_its_angle() {
    let cfg = BoatConfig::charter_sloop_34();
    let flow = kn_to_ms(6.0);
    let moderate = cfg.rudder.forces(rad(20.0), flow);
    let hard = cfg.rudder.forces(rad(40.0), flow);
    assert!(
        hard.lift < moderate.lift,
        "40 deg of rudder must give less lift than 20: {:.0} N vs {:.0} N",
        hard.lift,
        moderate.lift
    );
    assert!(hard.stalled > 0.5, "40 deg should be well stalled");
    assert!(moderate.stalled < 0.1, "20 deg should still be attached");
    assert!(hard.drag > moderate.drag);
}

#[test]
fn rudder_is_dead_at_low_speed() {
    let cfg = BoatConfig::charter_sloop_34();
    let slow = cfg.rudder.forces(rad(15.0), kn_to_ms(0.5));
    let quick = cfg.rudder.forces(rad(15.0), kn_to_ms(5.0));
    // Lift goes as the square of the flow, so a tenfold speed drop is a hundredfold
    // loss of authority. This is why a boat with no way on will not answer the helm.
    assert!(
        slow.lift < quick.lift / 50.0,
        "{:.1} N at half a knot against {:.0} N at five",
        slow.lift,
        quick.lift
    );
}

#[test]
fn boat_carries_weather_helm_on_both_tacks() {
    let cfg = BoatConfig::charter_sloop_34();
    let make = wind(12.0);

    // Starboard tack: the wind is to starboard, so holding course needs rudder that
    // bears away, which is to port, which is a negative angle.
    let starboard = steady_state(cfg, &make, rad(45.0), SETTLE, 0.0);
    assert!(
        starboard.controls.rudder < 0.0,
        "starboard tack should need port rudder to hold course, got {:.1} deg",
        deg(starboard.controls.rudder)
    );
    let port = steady_state(cfg, &make, rad(-45.0), SETTLE, 0.0);
    assert!(
        port.controls.rudder > 0.0,
        "port tack should mirror it, got {:.1} deg",
        deg(port.controls.rudder)
    );

    // And the amount should be what a well balanced cruiser carries, not a fight.
    let helm = deg(starboard.controls.rudder).abs();
    assert!(
        (1.0..=8.0).contains(&helm),
        "weather helm close-hauled should be 1-8 deg, got {helm:.1}"
    );
}

#[test]
fn tacking_costs_speed_and_the_boat_recovers() {
    let cfg = BoatConfig::charter_sloop_34();
    let make = wind(12.0);
    let twa = rad(42.0);

    // Settle on starboard tack.
    let settled = steady_state(cfg, &make, twa, SETTLE, 0.0);
    let entry_speed = settled.telemetry.stw;
    assert!(settled.converged);

    let wind_dir = 0.0;
    let old_heading = wrap_tau(wind_dir - twa);
    let new_heading = wrap_tau(wind_dir + twa);
    let tack_angle = deg(wrap_pi(new_heading - old_heading)).abs();
    assert!(
        (80.0..=100.0).contains(&tack_angle),
        "tacking through {tack_angle:.0} deg; a cruiser tacks through about 90"
    );

    let mut sim = Simulation::new(cfg, settled.state, make());
    let mut pilot = TillerPilot::new();
    let mut controls = settled.controls;
    let mut min_speed = f64::INFINITY;

    // Put the helm over and sheet across.
    let steps = (90.0 / SUBSTEP) as u64;
    for _ in 0..steps {
        let (m, j) = optimal_sheets(&cfg, sim.telemetry.awa, controls.outhaul);
        controls.main_sheet = m;
        controls.jib_sheet = j;
        controls.rudder = pilot.rudder(&cfg, &sim.state, new_heading, SUBSTEP);
        sim.run(SUBSTEP, &controls);
        min_speed = min_speed.min(sim.telemetry.stw);
    }

    assert!(
        ms_to_kn(entry_speed - min_speed) > 0.8,
        "a tack must cost at least 0.8 kn, lost {:.2}",
        ms_to_kn(entry_speed - min_speed)
    );
    assert!(
        wrap_pi(sim.state.heading - new_heading).abs() < rad(5.0),
        "boat should have come round onto the new tack, off by {:.1} deg",
        deg(wrap_pi(sim.state.heading - new_heading))
    );
    assert!(
        sim.telemetry.stw > 0.9 * entry_speed,
        "and recovered to within 10 percent of entry speed: {:.2} vs {:.2} kn",
        ms_to_kn(sim.telemetry.stw),
        ms_to_kn(entry_speed)
    );
    assert!(sim.state.heel > 0.0, "should now be heeled to starboard");
}

// ---------------------------------------------------------------------------
// Reefing
// ---------------------------------------------------------------------------

#[test]
fn reefing_appropriately_is_nearly_free_upwind() {
    let cfg = BoatConfig::charter_sloop_34();
    let make = wind(22.0);
    let full = steady_state(cfg, &make, rad(45.0), SETTLE, 0.0);
    // Half reef: rolled headsail and a slab in the main, which is what a crew would
    // actually be carrying in twenty-two knots.
    let sensible = steady_state(cfg, &make, rad(45.0), SETTLE, 0.5);

    assert!(
        sensible.state.heel.abs() < full.state.heel.abs() - rad(4.0),
        "shortening down must take at least 4 deg off the heel: {:.1} vs {:.1}",
        deg(sensible.state.heel).abs(),
        deg(full.state.heel).abs()
    );
    // Standing the boat up costs almost nothing upwind, which is exactly why reefing
    // early is the right call and why nobody who knows this ever regrets it.
    assert!(
        sensible.telemetry.vmg > 0.92 * full.telemetry.vmg,
        "sensibly reefed VMG {:.2} kn against {:.2} kn at full sail",
        ms_to_kn(sensible.telemetry.vmg),
        ms_to_kn(full.telemetry.vmg)
    );
}

#[test]
fn storm_canvas_does_cost_real_speed() {
    let cfg = BoatConfig::charter_sloop_34();
    let make = wind(22.0);
    let full = steady_state(cfg, &make, rad(45.0), SETTLE, 0.0);
    let storm = steady_state(cfg, &make, rad(45.0), SETTLE, 1.0);
    // The far end of the range is a deep reef plus a nearly rolled genoa. It should
    // stand the boat right up and it should be visibly slower — otherwise the reef
    // control carries no trade-off and the player has no decision to make.
    assert!(deg(storm.state.heel).abs() < 16.0);
    assert!(
        storm.telemetry.vmg < 0.92 * full.telemetry.vmg,
        "storm canvas VMG {:.2} kn is not meaningfully slower than {:.2} kn",
        ms_to_kn(storm.telemetry.vmg),
        ms_to_kn(full.telemetry.vmg)
    );
}

#[test]
fn main_alone_sails_but_gives_up_pointing() {
    let cfg = BoatConfig::charter_sloop_34();
    let make = wind(12.0);
    let sloop = steady_state(cfg, &make, rad(45.0), SETTLE, 0.0);

    // Genoa rolled completely away. This is a real way to sail — shortening down in a
    // blow, or isolating the main to learn what it is doing — so it has to work.
    let heading = wrap_tau(0.0 - rad(45.0));
    let mut state = BoatState::at_rest(heading);
    state.surge = kn_to_ms(3.0);
    let mut sim = Simulation::new(cfg, state, make());
    let mut pilot = TillerPilot::new();
    let mut controls = Controls {
        furl: 1.0,
        ..Default::default()
    };
    for _ in 0..(SETTLE / SUBSTEP) as u64 {
        let (m, _) = optimal_sheets(&cfg, sim.telemetry.awa, controls.outhaul);
        controls.main_sheet = m;
        controls.rudder = pilot.rudder(&cfg, &sim.state, heading, SUBSTEP);
        sim.run(SUBSTEP, &controls);
    }
    assert!(
        wrap_pi(sim.state.heading - heading).abs() < rad(3.0),
        "main alone must still hold a close-hauled course, off by {:.1} deg",
        deg(wrap_pi(sim.state.heading - heading))
    );
    assert!(
        ms_to_kn(sim.telemetry.stw) > 3.5,
        "main alone should still sail, got {:.2} kn",
        ms_to_kn(sim.telemetry.stw)
    );
    // Slower than the full rig, because half the sail area is gone.
    assert!(sim.telemetry.stw < sloop.telemetry.stw);
    // And the balance shifts towards weather helm, since the headsail was the thing
    // pulling the centre of effort forward.
    assert!(
        controls.rudder < sloop.controls.rudder + rad(0.5),
        "main alone should not be less weather-helmed: {:.1} vs {:.1} deg",
        deg(controls.rudder),
        deg(sloop.controls.rudder)
    );
}

#[test]
fn heavy_weather_needs_the_reef_and_light_weather_does_not() {
    let cfg = BoatConfig::charter_sloop_34();
    let (light, _) = reef_for_heel(cfg, &wind(8.0), rad(45.0), SETTLE, rad(TARGET_HEEL));
    let (heavy, heavy_state) = reef_for_heel(cfg, &wind(25.0), rad(45.0), SETTLE, rad(TARGET_HEEL));
    assert_eq!(light, 0.0, "8 kn needs no reef");
    assert!(
        heavy > 0.5,
        "25 kn upwind needs serious reduction, got {heavy}"
    );
    assert!(
        deg(heavy_state.state.heel).abs() < 26.0,
        "even reefed, heel came to {:.1} deg",
        deg(heavy_state.state.heel).abs()
    );
}

// ---------------------------------------------------------------------------
// Apparent wind identities and determinism
// ---------------------------------------------------------------------------

#[test]
fn apparent_wind_identities_hold() {
    let cfg = BoatConfig::charter_sloop_34();
    let tws = 12.0;

    // Dead run: apparent wind is true wind minus boat speed.
    let run = steady_state(cfg, wind(tws), rad(180.0), SETTLE, 0.0);
    let expected = kn_to_ms(tws) - run.telemetry.stw;
    assert!(
        (run.telemetry.aws - expected).abs() < 0.35,
        "dead run: aws {:.2} kn, expected tws - stw = {:.2} kn",
        ms_to_kn(run.telemetry.aws),
        ms_to_kn(expected)
    );

    // Close-hauled: apparent wind must exceed true wind.
    let beat = steady_state(cfg, wind(tws), rad(42.0), SETTLE, 0.0);
    assert!(
        beat.telemetry.aws > kn_to_ms(tws),
        "close-hauled apparent wind {:.2} kn must exceed the true {:.2} kn",
        ms_to_kn(beat.telemetry.aws),
        tws
    );
    // And it must come from further forward than the true wind.
    assert!(
        beat.telemetry.awa.abs() < beat.telemetry.twa.abs(),
        "apparent {:.1} deg should be forward of true {:.1} deg",
        deg(beat.telemetry.awa),
        deg(beat.telemetry.twa)
    );
}

#[test]
fn synthetic_wind_is_deterministic_for_a_given_seed() {
    let cfg = BoatConfig::charter_sloop_34();
    let run = |seed: u64| {
        let w = SyntheticWind::new(0.0, kn_to_ms(14.0), seed);
        let mut sim = Simulation::new(cfg, BoatState::at_rest(rad(45.0)), Box::new(w));
        sim.run(180.0, &Controls::close_hauled());
        sim.state.pos
    };
    assert_eq!(run(7), run(7), "same seed must reproduce the track exactly");
    assert_ne!(run(7), run(8));
}

#[test]
fn gusts_are_gusty_but_not_noise() {
    use glam::DVec2;
    let mut w = SyntheticWind::new(0.0, kn_to_ms(14.0), 42);
    let mut samples = Vec::new();
    for _ in 0..(600.0 / SUBSTEP) as u32 {
        w.step(SUBSTEP);
        samples.push(w.sample(DVec2::ZERO, 0.0).length());
    }
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    let var = samples.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / samples.len() as f64;
    let intensity = var.sqrt() / mean;
    assert!(
        (0.05..=0.25).contains(&intensity),
        "turbulence intensity over open water runs 10-15 percent, got {:.3}",
        intensity
    );
    // Correlated in time, not white: consecutive samples must be close.
    let jumps = samples
        .windows(2)
        .filter(|w| (w[1] - w[0]).abs() > 0.05)
        .count();
    assert!(
        jumps < samples.len() / 100,
        "wind jumped {jumps} times in {} samples; gusts should build over seconds",
        samples.len()
    );
}

// ---------------------------------------------------------------------------
// The in-irons case: physically real, and a trap for a steady-state solver
// ---------------------------------------------------------------------------

#[test]
fn a_boat_in_irons_cannot_be_steered_and_barely_moves() {
    let cfg = BoatConfig::charter_sloop_34();
    let mut sim = Simulation::new(
        cfg,
        BoatState::at_rest(0.0),
        Box::new(SteadyWind::new(0.0, kn_to_ms(12.0))),
    );
    // Commanding a bear-away with no way on. The rudder has no flow over it, so the
    // command is ignored — the defining misery of being in irons. The boat will fall
    // off on its own eventually, because head to wind is an unstable attitude, but it
    // does so on the wind's schedule and not the helmsman's.
    let mut pilot = TillerPilot::new();
    let mut controls = Controls::close_hauled();
    let target = rad(60.0);
    for _ in 0..(60.0 / SUBSTEP) as u64 {
        controls.rudder = pilot.rudder(&cfg, &sim.state, target, SUBSTEP);
        sim.run(SUBSTEP, &controls);
    }
    assert!(
        ms_to_kn(sim.telemetry.stw) < 2.5,
        "in irons the boat must not get going properly, got {:.2} kn",
        ms_to_kn(sim.telemetry.stw)
    );
    assert!(
        sim.telemetry.vmg < kn_to_ms(0.6),
        "and must make next to no progress to windward, got {:.2} kn VMG",
        ms_to_kn(sim.telemetry.vmg)
    );
    assert!(
        wrap_pi(sim.state.heading - target).abs() > rad(20.0),
        "with no flow over the rudder the boat cannot be steered round; it answered to \
         within {:.0} deg, which means the rudder is working at a standstill",
        deg(wrap_pi(sim.state.heading - target)).abs()
    );
}

#[test]
fn a_sail_in_the_eye_of_the_wind_is_pure_drag() {
    let profile = SailProfile::mainsail(32.0, 12.5);
    let aws = kn_to_ms(14.0);
    // Head to wind, sheeted in: nothing to fill the sail, so it flogs and the only
    // force left is drag, pointing aft. This is why a boat caught in irons gathers
    // sternway rather than sitting still.
    let flogging = sail_forces(&profile, 0.0, aws, 0.0, 32.0, 0.0, 0.0, CAMBER_REFERENCE);
    assert!(
        flogging.drive < 0.0,
        "a sail head to wind must push the boat backwards, drive {:.0} N",
        flogging.drive
    );
    assert_eq!(flogging.coeffs.cl, 0.0, "no lift from a flogging sail");
    assert!(flogging.coeffs.luffing > 0.99);
    assert!(
        flogging.side.abs() < 1.0,
        "and essentially no side force, got {:.1} N",
        flogging.side
    );
}

#[test]
fn bearing_away_with_way_on_gets_the_boat_sailing() {
    let cfg = BoatConfig::charter_sloop_34();
    let make = wind(12.0);
    // A knot and a half of residual way — the realistic version of losing the wind,
    // and enough flow for the rudder to bite.
    let mut initial = BoatState::at_rest(0.0);
    initial.surge = kn_to_ms(1.5);
    let mut sim = Simulation::new(cfg, initial, make());

    let mut pilot = TillerPilot::new();
    let target = rad(60.0);
    let mut controls = Controls::default();
    for _ in 0..(180.0 / SUBSTEP) as u64 {
        let (m, j) = optimal_sheets(&cfg, sim.telemetry.awa, controls.outhaul);
        controls.main_sheet = m;
        controls.jib_sheet = j;
        controls.rudder = pilot.rudder(&cfg, &sim.state, target, SUBSTEP);
        sim.run(SUBSTEP, &controls);
    }
    assert!(
        ms_to_kn(sim.telemetry.stw) > 5.0,
        "bearing away onto a close reach should get the boat going, got {:.2} kn",
        ms_to_kn(sim.telemetry.stw)
    );
    assert!(!sim.telemetry.luffing());
    assert!(
        wrap_pi(sim.state.heading - target).abs() < rad(5.0),
        "and it should settle on the new heading"
    );
}

#[test]
fn the_engine_gets_you_out_of_the_marina() {
    let cfg = BoatConfig::charter_sloop_34();
    // No wind, no sails, just the auxiliary: the manoeuvre every passage starts with.
    let mut sim = Simulation::new(
        cfg,
        BoatState::at_rest(0.0),
        Box::new(SteadyWind::new(0.0, 0.0)),
    );
    let ahead = Controls {
        throttle: 1.0,
        ..Default::default()
    };
    sim.run(90.0, &ahead);
    let cruise = ms_to_kn(sim.telemetry.stw);
    assert!(
        (5.5..=8.0).contains(&cruise),
        "a 20-odd horsepower auxiliary should push this hull at 5.5-8 kn, got {cruise:.2}"
    );

    // And astern is slower, as a propeller in reverse always is.
    let mut sim = Simulation::new(
        cfg,
        BoatState::at_rest(0.0),
        Box::new(SteadyWind::new(0.0, 0.0)),
    );
    sim.run(
        90.0,
        &Controls {
            throttle: -1.0,
            ..Default::default()
        },
    );
    assert!(
        sim.telemetry.stw < 0.0 || sim.state.surge < 0.0,
        "should make sternway"
    );
}

// ---------------------------------------------------------------------------
// Outhaul: mainsail depth, and the trade it buys
// ---------------------------------------------------------------------------

#[test]
fn easing_the_outhaul_deepens_the_sail_and_hardening_it_flattens() {
    assert!(camber_for_outhaul(0.0) > camber_for_outhaul(1.0));
    assert!((camber_for_outhaul(0.5) - CAMBER_REFERENCE).abs() < 0.005);
    // A cruising main lives between about seven and fifteen percent draft.
    assert!((0.06..=0.16).contains(&camber_for_outhaul(0.0)));
    assert!((0.06..=0.16).contains(&camber_for_outhaul(1.0)));
}

#[test]
fn a_deeper_sail_makes_more_force_and_peaks_earlier() {
    let profile = SailProfile::mainsail(32.0, 12.5);
    let deep = profile.coefficients(rad(14.0), camber_for_outhaul(0.0));
    let flat = profile.coefficients(rad(14.0), camber_for_outhaul(1.0));

    // Depth is power.
    assert!(
        deep.cl > flat.cl,
        "deep {:.2} should out-lift flat {:.2}",
        deep.cl,
        flat.cl
    );
    // And depth costs drag.
    assert!(deep.cd > flat.cd);

    // A deep sail reaches its best angle of attack sooner, because the camber is
    // already doing part of the work.
    let awa = rad(30.0);
    assert!(
        profile.optimal_chord_angle(awa, camber_for_outhaul(0.0))
            > profile.optimal_chord_angle(awa, camber_for_outhaul(1.0)),
        "a deep sail wants the boom further out for the same apparent wind"
    );
}

#[test]
fn flattening_the_main_takes_the_heel_out_in_a_breeze() {
    let cfg = BoatConfig::charter_sloop_34();
    let make = wind(20.0);
    let deep = at_outhaul(cfg, &make, rad(45.0), 0.0);
    let flat = at_outhaul(cfg, &make, rad(45.0), 1.0);

    assert!(
        flat.1.abs() < deep.1.abs() - rad(1.0),
        "flattening must stand the boat up: {:.1} deg flat against {:.1} deg deep",
        deg(flat.1).abs(),
        deg(deep.1).abs()
    );
    // And it should carry less helm, since there is less force up high to luff you.
    assert!(flat.3.abs() <= deep.3.abs() + rad(0.3));
}

#[test]
fn depth_pays_in_light_air_and_flat_pays_in_a_breeze() {
    let cfg = BoatConfig::charter_sloop_34();

    let light = wind(6.0);
    let deep_light = at_outhaul(cfg, &light, rad(45.0), 0.0);
    let flat_light = at_outhaul(cfg, &light, rad(45.0), 1.0);
    assert!(
        deep_light.2 > flat_light.2,
        "in six knots depth should win: deep {:.3} kn VMG against flat {:.3}",
        ms_to_kn(deep_light.2),
        ms_to_kn(flat_light.2)
    );

    // The recommendation should follow the same shape it asserts.
    assert!(
        optimal_outhaul(kn_to_ms(6.0)) < 0.25,
        "ease it off in light air"
    );
    assert!(
        optimal_outhaul(kn_to_ms(22.0)) > 0.75,
        "flatten it in a breeze"
    );
    assert!(optimal_outhaul(kn_to_ms(6.0)) < optimal_outhaul(kn_to_ms(22.0)));
}

/// Hold a close-hauled course at a fixed outhaul with the sheet trimmed to suit.
/// Returns speed, heel, VMG and the standing helm.
fn at_outhaul<F>(cfg: BoatConfig, make: &F, twa: f64, outhaul: f64) -> (f64, f64, f64, f64)
where
    F: Fn() -> Box<dyn WindField>,
{
    let heading = wrap_tau(0.0 - twa);
    let mut state = BoatState::at_rest(heading);
    state.surge = kn_to_ms(3.0);
    let mut sim = Simulation::new(cfg, state, make());
    let mut pilot = TillerPilot::new();
    let mut controls = Controls {
        outhaul,
        ..Default::default()
    };
    for _ in 0..(SETTLE / SUBSTEP) as u64 {
        let (m, j) = optimal_sheets(&cfg, sim.telemetry.awa, outhaul);
        controls.main_sheet = m;
        controls.jib_sheet = j;
        controls.rudder = pilot.rudder(&cfg, &sim.state, heading, SUBSTEP);
        sim.run(SUBSTEP, &controls);
    }
    (
        sim.telemetry.stw,
        sim.state.heel,
        sim.telemetry.vmg,
        controls.rudder,
    )
}

// ---------------------------------------------------------------------------
// Autopilot: compass course, and a wind vane
// ---------------------------------------------------------------------------

/// Run the pilot for a while with the sails trimmed, and report how far it drifted.
fn pilot_run(
    cfg: BoatConfig,
    mode: PilotMode,
    seconds: f64,
    tws_knots: f64,
    seed: Option<u64>,
) -> (Simulation, f64) {
    let twa = rad(45.0);
    let heading = wrap_tau(0.0 - twa);
    let mut state = BoatState::at_rest(heading);
    state.surge = kn_to_ms(4.0);
    let field: Box<dyn WindField> = match seed {
        Some(s) => Box::new(SyntheticWind::new(0.0, kn_to_ms(tws_knots), s)),
        None => Box::new(SteadyWind::new(0.0, kn_to_ms(tws_knots))),
    };
    let mut sim = Simulation::new(cfg, state, field);
    let mut pilot = Autopilot::new();
    let mut controls = Controls {
        outhaul: 0.5,
        ..Default::default()
    };

    // Let it settle on the hand-set course first, then engage.
    let mut hand = TillerPilot::new();
    for _ in 0..(90.0 / SUBSTEP) as u64 {
        let (m, j) = optimal_sheets(&cfg, sim.telemetry.awa, controls.outhaul);
        controls.main_sheet = m;
        controls.jib_sheet = j;
        controls.rudder = hand.rudder(&cfg, &sim.state, heading, SUBSTEP);
        sim.run(SUBSTEP, &controls);
    }
    pilot.mode = match mode {
        PilotMode::Heading(_) => PilotMode::Heading(sim.state.heading),
        PilotMode::ApparentWind(_) => PilotMode::ApparentWind(sim.telemetry.awa),
        PilotMode::Standby => PilotMode::Standby,
    };
    let reference = match pilot.mode {
        PilotMode::Heading(h) => h,
        PilotMode::ApparentWind(a) => a,
        PilotMode::Standby => 0.0,
    };

    let mut worst = 0.0_f64;
    for _ in 0..(seconds / SUBSTEP) as u64 {
        let (m, j) = optimal_sheets(&cfg, sim.telemetry.awa, controls.outhaul);
        controls.main_sheet = m;
        controls.jib_sheet = j;
        if let Some(r) = pilot.rudder(&cfg, &sim.state, &sim.telemetry, SUBSTEP) {
            controls.rudder = r;
        }
        sim.run(SUBSTEP, &controls);
        let drift = match pilot.mode {
            PilotMode::Heading(_) => wrap_pi(sim.state.heading - reference).abs(),
            PilotMode::ApparentWind(_) => (sim.telemetry.awa - reference).abs(),
            PilotMode::Standby => 0.0,
        };
        worst = worst.max(drift);
    }
    (sim, worst)
}

#[test]
fn the_pilot_holds_a_compass_course() {
    let cfg = BoatConfig::charter_sloop_34();
    let (sim, worst) = pilot_run(cfg, PilotMode::Heading(0.0), 300.0, 12.0, None);
    assert!(
        deg(worst) < 2.0,
        "compass pilot wandered {:.1} deg off course",
        deg(worst)
    );
    assert!(
        ms_to_kn(sim.telemetry.stw) > 5.0,
        "and it should still be sailing"
    );
}

#[test]
fn the_vane_holds_a_wind_angle_through_the_shifts() {
    let cfg = BoatConfig::charter_sloop_34();
    // Gusty, shifting wind — the case a compass pilot cannot handle and a vane can.
    let (sim, worst) = pilot_run(cfg, PilotMode::ApparentWind(0.0), 420.0, 12.0, Some(11));
    assert!(
        deg(worst) < 5.0,
        "wind vane let the apparent wind angle move {:.1} deg",
        deg(worst)
    );
    assert!(ms_to_kn(sim.telemetry.stw) > 4.5);
    assert!(
        !sim.telemetry.luffing(),
        "and it should not have pinched into a luff"
    );
}

#[test]
fn a_vane_follows_a_shift_where_a_compass_pilot_would_not() {
    let cfg = BoatConfig::charter_sloop_34();
    // Same shifting wind, same length of run: the compass pilot keeps the course and
    // therefore lets the wind angle wander; the vane keeps the angle and lets the
    // course wander. That difference is the whole reason both exist.
    let (_, compass_wind_drift) = {
        let (sim, _) = pilot_run(cfg, PilotMode::Heading(0.0), 420.0, 12.0, Some(11));
        let _ = &sim;
        // Measure wind-angle wander under the compass pilot.
        let (s2, _) = pilot_run(cfg, PilotMode::Heading(0.0), 420.0, 12.0, Some(11));
        (
            s2.state.heading,
            wind_angle_spread(cfg, PilotMode::Heading(0.0)),
        )
    };
    let vane_wind_drift = wind_angle_spread(cfg, PilotMode::ApparentWind(0.0));
    assert!(
        vane_wind_drift < compass_wind_drift,
        "the vane should hold the wind angle tighter than the compass pilot: \
         {:.2} deg against {:.2} deg",
        deg(vane_wind_drift),
        deg(compass_wind_drift)
    );
}

/// Peak apparent-wind-angle wander over a run in shifting wind.
fn wind_angle_spread(cfg: BoatConfig, mode: PilotMode) -> f64 {
    let twa = rad(45.0);
    let heading = wrap_tau(0.0 - twa);
    let mut state = BoatState::at_rest(heading);
    state.surge = kn_to_ms(4.0);
    let mut sim = Simulation::new(
        cfg,
        state,
        Box::new(SyntheticWind::new(0.0, kn_to_ms(12.0), 11)),
    );
    let mut pilot = Autopilot::new();
    let mut controls = Controls {
        outhaul: 0.5,
        ..Default::default()
    };
    let mut hand = TillerPilot::new();
    for _ in 0..(90.0 / SUBSTEP) as u64 {
        let (m, j) = optimal_sheets(&cfg, sim.telemetry.awa, controls.outhaul);
        controls.main_sheet = m;
        controls.jib_sheet = j;
        controls.rudder = hand.rudder(&cfg, &sim.state, heading, SUBSTEP);
        sim.run(SUBSTEP, &controls);
    }
    pilot.mode = match mode {
        PilotMode::Heading(_) => PilotMode::Heading(sim.state.heading),
        PilotMode::ApparentWind(_) => PilotMode::ApparentWind(sim.telemetry.awa),
        PilotMode::Standby => PilotMode::Standby,
    };
    let reference = sim.telemetry.awa;
    let mut worst = 0.0_f64;
    for _ in 0..(420.0 / SUBSTEP) as u64 {
        let (m, j) = optimal_sheets(&cfg, sim.telemetry.awa, controls.outhaul);
        controls.main_sheet = m;
        controls.jib_sheet = j;
        if let Some(r) = pilot.rudder(&cfg, &sim.state, &sim.telemetry, SUBSTEP) {
            controls.rudder = r;
        }
        sim.run(SUBSTEP, &controls);
        worst = worst.max((sim.telemetry.awa - reference).abs());
    }
    worst
}
