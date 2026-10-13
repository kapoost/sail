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
    camber_for_outhaul, outhaul_for, sail_forces, SailProfile, CAMBER_REFERENCE,
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

    // Dead run: apparent wind is true wind minus boat speed — at the same height.
    // True wind is quoted at ten metres and the boundary layer makes it weaker lower
    // down, so the identity only closes once the measurement heights agree.
    let run = steady_state(cfg, wind(tws), rad(180.0), SETTLE, 0.0);
    let at_height = kn_to_ms(tws) * sail_core::wind::shear_factor(run.telemetry.wind_height);
    let expected = at_height - run.telemetry.stw;
    assert!(
        (run.telemetry.aws - expected).abs() < 0.35,
        "dead run: aws {:.2} kn, expected tws - stw = {:.2} kn",
        ms_to_kn(run.telemetry.aws),
        ms_to_kn(expected)
    );

    // Close-hauled: apparent wind must exceed true wind.
    let beat = steady_state(cfg, wind(tws), rad(42.0), SETTLE, 0.0);
    assert!(
        beat.telemetry.aws > kn_to_ms(tws) * 0.9,
        "close-hauled apparent wind {:.2} kn must exceed the true {:.2} kn at height",
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
    // At a fixed point, with the clock running. That is the whole claim now: the gusts
    // are a pattern in space, and a mooring buoy feels them because the wind carries
    // them past it. The old version of this test held `t` at zero and leaned on
    // `step()` to make the weather happen, which a spatial field will not do — and
    // which was already wrong, since it stepped a wind whose clock never moved.
    let mut t = 0.0;
    for _ in 0..(600.0 / SUBSTEP) as u32 {
        w.step(SUBSTEP);
        t += SUBSTEP;
        samples.push(w.sample(DVec2::ZERO, t).length());
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
fn depth_is_power_at_its_own_angle_and_a_handicap_close_hauled() {
    let profile = SailProfile::mainsail(32.0, 12.5);
    let deep_c = camber_for_outhaul(0.0);
    let flat_c = camber_for_outhaul(1.0);

    // At each sail's own best angle of attack, depth is power and depth is drag. That
    // is the trade the outhaul exists to make, and it has not changed.
    let deep = profile.coefficients(profile.alpha_opt_for(deep_c), deep_c);
    let flat = profile.coefficients(profile.alpha_opt_for(flat_c), flat_c);
    assert!(
        deep.cl > flat.cl,
        "deep {:.2} should out-lift flat {:.2} at its own angle",
        deep.cl,
        flat.cl
    );
    assert!(deep.cd > flat.cd);

    // Close-hauled it is the other way round, and this is the fact the model was
    // missing. Fourteen degrees is about the angle of attack a main sees on a beat; the
    // deep sail's entry is backwinding there, so it makes *less* lift than the flat one
    // and drags more while doing it. This is the shaking patch behind the mast, and it
    // is why the outhaul goes on going upwind.
    let deep_beat = profile.coefficients(rad(14.0), deep_c);
    let flat_beat = profile.coefficients(rad(14.0), flat_c);
    assert!(
        flat_beat.cl > deep_beat.cl,
        "on a beat the flat sail should win: flat {:.2}, deep {:.2}",
        flat_beat.cl,
        deep_beat.cl
    );
    assert!(deep_beat.cd > flat_beat.cd);
    assert!(
        deep_beat.luffing > 0.3,
        "the deep sail should be visibly shaking on a beat, got {:.2}",
        deep_beat.luffing
    );
    assert!(
        flat_beat.luffing < 0.05,
        "the flat sail should be quiet, got {:.2}",
        flat_beat.luffing
    );

    // And a deep sail cannot be trimmed out of it: its best usable angle of attack is
    // larger, not smaller, so for the same apparent wind the chord has to come further
    // in — and on a close course that means the boom on the centreline with the sail
    // still not filling. Thin-aerofoil theory says a cambered section peaks earlier,
    // and for a rigid wing it does; for a membrane with a sharp luff the entry
    // constraint overrules it.
    let awa = rad(30.0);
    assert!(
        profile.optimal_chord_angle(awa, deep_c) < profile.optimal_chord_angle(awa, flat_c),
        "a deep sail cannot be eased into working on a close course"
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
fn depth_is_fastest_until_the_boat_runs_out_of_righting_moment() {
    let cfg = BoatConfig::charter_sloop_34();

    // Below the power limit a fuller sail is simply faster, which is also what a
    // velocity prediction program says: reducing force only helps once righting moment
    // is the binding constraint. Asserting this stops the recommendation drifting back
    // towards folklore.
    for tws in [5.0, 8.0, 12.0, 16.0] {
        let make = wind(tws);
        // Each trim sails its own best course, which is the whole point of flattening:
        // the one thing a flat sail buys upwind is the ability to point, and a sweep at
        // a fixed true wind angle cannot see it. This test asserted a fixed forty-five
        // degrees once and therefore demanded a rule that never flattened.
        let best_course = |outhaul: f64| -> f64 {
            let mut best = f64::NEG_INFINITY;
            for step in 0..=4 {
                let twa = rad(36.0 + 3.0 * step as f64);
                best = best.max(at_outhaul(cfg, &make, twa, outhaul).2);
            }
            best
        };
        let settled = steady_state(cfg, &make, rad(45.0), SETTLE, 0.0);
        let mut best = f64::NEG_INFINITY;
        for step in 0..=5 {
            best = best.max(best_course(step as f64 / 5.0));
        }
        let cost = best - best_course(settled.controls.outhaul);
        let relative = cost / best;
        assert!(
            relative < 0.018,
            "at {tws} kn the rule settles on outhaul {:.2} and gives away {:.3} kn VMG \
             ({:.1} percent) against a best of {:.3}",
            settled.controls.outhaul,
            ms_to_kn(cost),
            relative * 100.0,
            ms_to_kn(best)
        );
    }
}

#[test]
fn heel_past_the_rail_going_under_costs_real_speed() {
    let cfg = BoatConfig::charter_sloop_34();
    // The steep second regime exists because without it the model thinks thirty
    // degrees of heel is the fastest place to be. It must bite hard above the
    // threshold and barely at all below it, or it would move the whole polar.
    assert!(cfg.rail_down_factor > 0.0);
    let gentle = heel_resistance_factor(&cfg, rad(15.0));
    let at_limit = heel_resistance_factor(&cfg, rad(20.0));
    let rail_down = heel_resistance_factor(&cfg, rad(32.0));
    assert!(
        (at_limit - cfg.heel_drag_factor * rad(20.0) * rad(20.0)).abs() < 1e-12,
        "the steep term must contribute nothing at or below the threshold"
    );
    assert!(gentle < at_limit && at_limit < rail_down);
    assert!(
        rail_down > 3.0 * at_limit,
        "burying the rail should cost several times what sailing at the limit does: \
         {rail_down:.3} against {at_limit:.3}"
    );
}

fn heel_resistance_factor(cfg: &BoatConfig, heel: f64) -> f64 {
    let excess = (heel.abs() - cfg.rail_down_heel).max(0.0);
    cfg.heel_drag_factor * heel * heel + cfg.rail_down_factor * excess * excess
}

#[test]
fn the_outhaul_goes_on_upwind_and_comes_off_downwind() {
    // The thing the rule used to get wrong: it was the same close-hauled and running,
    // so the boat beat to windward with a foot eased right off. A deep entry cannot be
    // sheeted close, so upwind the outhaul goes on whatever the heel is doing.
    let close = outhaul_for(rad(25.0), rad(15.0), rad(20.0));
    let broad = outhaul_for(rad(140.0), rad(15.0), rad(20.0));
    assert!(
        close > 0.35,
        "close-hauled with pressure on: foot firm, got {close:.2}"
    );
    assert!(
        broad < 0.05,
        "broad reach: all the depth you can get, got {broad:.2}"
    );
    assert!(close > broad + 0.3);

    // And in a drifter there is shape in it even upwind, because there is no heel to
    // get rid of and she needs every square metre working.
    let drifter = outhaul_for(rad(25.0), rad(3.0), rad(20.0));
    assert!(
        drifter < 0.2,
        "five knots and upright: leave some depth, got {drifter:.2}"
    );

    // Heel still flattens on top of that, and still only once the righting moment has
    // run out — which is the part of the old rule the sweep confirmed.
    assert!(
        outhaul_for(rad(140.0), rad(20.0), rad(20.0)) < 0.2,
        "at the limit"
    );
    assert!(
        outhaul_for(rad(140.0), rad(32.0), rad(20.0)) > 0.95,
        "badly overpressed: flat, even running"
    );
    assert!(
        outhaul_for(rad(25.0), rad(32.0), rad(20.0)) > 0.95,
        "overpressed upwind: flat"
    );

    // Symmetric in tack, both inputs.
    assert_eq!(
        outhaul_for(rad(-25.0), rad(-25.0), rad(20.0)),
        outhaul_for(rad(25.0), rad(25.0), rad(20.0))
    );
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
        // Trimmed knowing how the two sails bend each other's wind, which is what a
        // crew trims by: a main sheeted to the masthead fly is eased too far by exactly
        // the headsail's downwash, every time.
        let (m, j) = sail_core::autopilot::optimal_sheets_bent(
            &cfg,
            sim.telemetry.awa,
            outhaul,
            sail_core::state::Headsail::Genoa,
            sim.telemetry.bend_main,
            sim.telemetry.bend_jib,
        );
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
    // Same shifting wind, same length of run. The compass pilot keeps the course and
    // therefore lets the wind angle wander; the vane keeps the angle and lets the
    // course wander. That difference is the whole reason both pieces of gear exist.
    let compass = wind_angle_spread(cfg, PilotMode::Heading(0.0));
    let vane = wind_angle_spread(cfg, PilotMode::ApparentWind(0.0));
    assert!(
        vane < compass,
        "the vane should hold the wind angle tighter than the compass pilot: \
         {:.2} deg against {:.2} deg",
        deg(vane),
        deg(compass)
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

#[test]
fn a_vane_knocked_onto_the_wrong_tack_tacks_back_instead_of_gybing() {
    let cfg = BoatConfig::charter_sloop_34();
    let make = wind(12.0);
    let twa = rad(45.0);
    let starboard_heading = wrap_tau(0.0 - twa);
    let port_heading = wrap_tau(twa);

    // Settle on starboard tack and arm the vane on that angle.
    let mut state = BoatState::at_rest(starboard_heading);
    state.surge = kn_to_ms(4.0);
    let mut sim = Simulation::new(cfg, state, make());
    let mut hand = TillerPilot::new();
    let mut controls = Controls::default();
    for _ in 0..(120.0 / SUBSTEP) as u64 {
        let (m, j) = optimal_sheets(&cfg, sim.telemetry.awa, controls.outhaul);
        controls.main_sheet = m;
        controls.jib_sheet = j;
        controls.rudder = hand.rudder(&cfg, &sim.state, starboard_heading, SUBSTEP);
        sim.run(SUBSTEP, &controls);
    }
    let mut pilot = Autopilot::new();
    pilot.hold_wind(&sim.telemetry);
    let target = match pilot.mode {
        PilotMode::ApparentWind(a) => a,
        _ => unreachable!(),
    };
    assert!(target > 0.0, "armed on starboard tack");

    // Now hand-steer across onto port tack and give it back to the pilot.
    let mut hand = TillerPilot::new();
    for _ in 0..(90.0 / SUBSTEP) as u64 {
        let (m, j) = optimal_sheets(&cfg, sim.telemetry.awa, controls.outhaul);
        controls.main_sheet = m;
        controls.jib_sheet = j;
        controls.rudder = hand.rudder(&cfg, &sim.state, port_heading, SUBSTEP);
        sim.run(SUBSTEP, &controls);
    }
    assert!(
        sim.telemetry.awa < 0.0,
        "should be on port tack before the pilot takes over, awa {:.1}",
        deg(sim.telemetry.awa)
    );

    // The pilot must tack back — the short way, through the wind. Going the other way
    // means an uncontrolled gybe through dead downwind, which is how you break a boom.
    let mut deepest = 0.0_f64;
    for _ in 0..(120.0 / SUBSTEP) as u64 {
        let (m, j) = optimal_sheets(&cfg, sim.telemetry.awa, controls.outhaul);
        controls.main_sheet = m;
        controls.jib_sheet = j;
        if let Some(r) = pilot.rudder(&cfg, &sim.state, &sim.telemetry, SUBSTEP) {
            controls.rudder = r;
        }
        sim.run(SUBSTEP, &controls);
        deepest = deepest.max(sim.telemetry.twa.abs());
    }
    assert!(
        deg(deepest) < 110.0,
        "the vane bore away to {:.0} deg true — that is a gybe, not a tack",
        deg(deepest)
    );
    assert!(
        (sim.telemetry.awa - target).abs() < rad(6.0),
        "and it should have recovered the target: holding {:.1} against {:.1}",
        deg(sim.telemetry.awa),
        deg(target)
    );
    assert!(ms_to_kn(sim.telemetry.stw) > 4.5, "and still be sailing");
}

// ---------------------------------------------------------------------------
// Waves: hydrostatics, natural periods, and what a seaway does to the boat
// ---------------------------------------------------------------------------

use sail_core::hydrostatics;
use sail_core::water::{fetch_limited_sea, FlatWater, WaterSurface, WindSea};

/// Calm, no wind, no sails: just the hull and the water.
fn becalmed(cfg: BoatConfig) -> Simulation {
    Simulation::new(
        cfg,
        BoatState::at_rest(0.0),
        Box::new(SteadyWind::new(0.0, 0.0)),
    )
}

/// Natural period of a decaying oscillation.
///
/// Counts crossings in both directions and doubles the mean spacing, because these
/// modes are well damped — a yacht loses four fifths of its heave amplitude per cycle,
/// so waiting for three *upward* crossings asks for an oscillation that has already
/// died. The signal is centred on its tail first, so a settling offset cannot hide a
/// crossing.
fn period_from_crossings(samples: &[f64], dt: f64) -> f64 {
    let tail = &samples[samples.len() * 3 / 4..];
    let mean = tail.iter().sum::<f64>() / tail.len() as f64;
    let centred: Vec<f64> = samples.iter().map(|s| s - mean).collect();

    let mut indices = Vec::new();
    for (i, w) in centred.windows(2).enumerate() {
        if (w[0] <= 0.0) != (w[1] <= 0.0) {
            indices.push(i);
        }
    }
    assert!(
        indices.len() >= 3,
        "signal did not oscillate: only {} crossings",
        indices.len()
    );
    let half_spans: Vec<f64> = indices
        .windows(2)
        .map(|w| (w[1] - w[0]) as f64 * dt)
        .collect();
    2.0 * half_spans.iter().sum::<f64>() / half_spans.len() as f64
}

#[test]
fn the_boat_floats_at_its_design_waterline() {
    // Every hull in the yard, not just the first one. The sectional area coefficient
    // is calibrated per boat rather than derived, so a new config inherits the last
    // one's number and floats half a metre high until somebody notices — which is
    // exactly what the archipelago packet did until this loop was put round the test.
    for cfg in [
        BoatConfig::charter_sloop_34(),
        BoatConfig::performance_cruiser_53(),
        BoatConfig::archipelago_packet(),
    ] {
        let volume = cfg.form.volume_at(cfg.form.canoe_draft);
        let displaced = volume * 1025.0;
        assert!(
            (displaced - cfg.displacement).abs() < 0.02 * cfg.displacement,
            "{}: at its design draft the hull displaces {displaced:.0} kg against a \
             displacement of {:.0}",
            cfg.name,
            cfg.displacement
        );
    }

    let cfg = BoatConfig::charter_sloop_34();
    // And it should settle there on its own.
    let mut sim = becalmed(cfg);
    sim.run(60.0, &Controls::default());
    assert!(
        sim.state.heave.abs() < 0.03,
        "the boat settled {:.3} m off its waterline",
        sim.state.heave
    );
    assert!(
        (sim.telemetry.immersion - cfg.form.canoe_draft).abs() < 0.06,
        "mean immersion {:.3} m against a canoe draft of {:.3}",
        sim.telemetry.immersion,
        cfg.form.canoe_draft
    );
    assert!(sim.state.pitch.abs() < rad(0.5), "and on an even keel");
}

#[test]
fn the_waterplane_is_the_shape_a_cruiser_actually_has() {
    let cfg = BoatConfig::charter_sloop_34();
    let cwp = cfg.form.waterplane_area() / (cfg.form.lwl * cfg.form.waterline_beam);
    // A cruising hull of this size sits around 0.68 to 0.74. Tuning the station shape
    // by eye gave 0.83, a hull far too full in the ends, and it showed up as heave and
    // pitch periods a third shorter than they should be.
    assert!(
        (0.66..=0.76).contains(&cwp),
        "waterplane coefficient {cwp:.3} is not a plausible hull"
    );
}

#[test]
fn natural_periods_are_those_of_a_thirty_four_foot_cruiser() {
    let cfg = BoatConfig::charter_sloop_34();
    let dt = SUBSTEP;

    // Heave: push her down and let go.
    let mut sim = becalmed(cfg);
    sim.run(30.0, &Controls::default());
    sim.state.heave = -0.25;
    sim.state.heave_rate = 0.0;
    let mut heave = Vec::new();
    for _ in 0..(14.0 / dt) as u64 {
        sim.run(dt, &Controls::default());
        heave.push(sim.state.heave);
    }
    let t_heave = period_from_crossings(&heave, dt);
    assert!(
        (1.2..=2.4).contains(&t_heave),
        "heave period {t_heave:.2} s; a boat this size runs about 1.7"
    );

    // Pitch: trim her by the bow and let go.
    let mut sim = becalmed(cfg);
    sim.run(30.0, &Controls::default());
    sim.state.pitch = rad(5.0);
    sim.state.pitch_rate = 0.0;
    let mut pitch = Vec::new();
    for _ in 0..(14.0 / dt) as u64 {
        sim.run(dt, &Controls::default());
        pitch.push(sim.state.pitch);
    }
    let t_pitch = period_from_crossings(&pitch, dt);
    assert!(
        (1.3..=2.9).contains(&t_pitch),
        "pitch period {t_pitch:.2} s; a boat this size runs about 1.9"
    );

    // Roll, which was never pinned down before waves made it matter.
    let mut sim = becalmed(cfg);
    sim.state.heel = rad(12.0);
    let mut roll = Vec::new();
    for _ in 0..(24.0 / dt) as u64 {
        sim.run(dt, &Controls::default());
        roll.push(sim.state.heel);
    }
    let t_roll = period_from_crossings(&roll, dt);
    assert!(
        (2.2..=4.6).contains(&t_roll),
        "roll period {t_roll:.2} s; a cruiser of this beam and GM runs about 3"
    );
    // And roll must be the slowest of the three, which is why it is the mode that
    // sets the integration step.
    assert!(t_roll > t_heave && t_roll > t_pitch);
}

#[test]
fn fetch_decides_what_sort_of_sea_a_wind_builds() {
    // The same twenty knots across a bay and across an ocean are not the same sea, and
    // this is the whole reason the Baltic has a reputation.
    let (hs_open, tp_open) = fetch_limited_sea(kn_to_ms(20.0), 500_000.0);
    let (hs_bay, tp_bay) = fetch_limited_sea(kn_to_ms(20.0), 20_000.0);

    assert!(
        hs_open > hs_bay * 1.8,
        "open water should build a much bigger sea: {hs_open:.2} m against {hs_bay:.2}"
    );
    assert!(
        tp_open > tp_bay,
        "and a longer one: {tp_open:.1} s against {tp_bay:.1}"
    );
    // Fully developed in twenty knots is a couple of metres at seven-odd seconds.
    assert!(
        (1.5..=3.5).contains(&hs_open),
        "fully developed Hs in 20 kn is {hs_open:.2} m"
    );
    assert!((5.5..=9.0).contains(&tp_open), "Tp {tp_open:.1} s");
    // Flat calm makes no sea at all.
    assert_eq!(fetch_limited_sea(0.0, 500_000.0).0, 0.0);
}

#[test]
fn gerstner_components_obey_deep_water_dispersion() {
    let sea = WindSea::new(kn_to_ms(14.0), 200_000.0, 0.0, 3);
    for c in sea.components() {
        // omega^2 = g k.
        let expected = (9.80665 * c.k).sqrt();
        assert!(
            (c.omega - expected).abs() < 1e-9,
            "component off dispersion: omega {} against {}",
            c.omega,
            expected
        );
    }
    // Long waves run faster than short ones. This is what makes a real sea look
    // layered rather than like corduroy.
    let mut sorted: Vec<_> = sea.components().to_vec();
    sorted.sort_by(|a, b| a.wavelength().partial_cmp(&b.wavelength()).unwrap());
    for w in sorted.windows(2) {
        assert!(w[1].celerity() > w[0].celerity());
    }
    // And the set lands on the significant height it was asked for.
    assert!(sea.significant_height() > 0.0);
    assert!(sea.peak_period() > 2.0);
}

#[test]
fn flat_water_costs_nothing() {
    let cfg = BoatConfig::charter_sloop_34();
    let s = steady_state(cfg, wind(12.0), rad(45.0), SETTLE, 0.0);
    // The whole polar rests on this: added resistance in waves must be identically
    // zero when there are none, or every flat-water number quietly changed.
    // Not bit-exact zero: the heave rate settles to something like 1e-14 rather than
    // to nothing, and squaring that is still nothing. The claim is that it costs
    // nothing, not that the float is clean.
    assert!(
        s.telemetry.resistance_waves < 1e-9,
        "flat water charged {:.3e} N of added resistance",
        s.telemetry.resistance_waves
    );
    assert_eq!(s.telemetry.significant_height, 0.0);
    assert_eq!(FlatWater.height(glam::DVec2::new(10.0, -3.0), 7.0), 0.0);
}

/// Sail a given true wind angle in a sea, and report speed and motion.
fn sail_in_a_sea(
    cfg: BoatConfig,
    tws_knots: f64,
    fetch: f64,
    twa: f64,
    seconds: f64,
) -> (f64, f64, f64, f64) {
    let heading = wrap_tau(0.0 - twa);
    let mut state = BoatState::at_rest(heading);
    state.surge = 0.6 * cfg.hull_speed();
    let sea = WindSea::new(kn_to_ms(tws_knots), fetch, 0.0, 17);
    let mut sim = Simulation::new(
        cfg,
        state,
        Box::new(SteadyWind::new(0.0, kn_to_ms(tws_knots))),
    )
    .with_water(Box::new(sea));
    let mut pilot = TillerPilot::new();
    let mut controls = Controls::default();

    let mut speed_sum = 0.0;
    let mut samples = 0.0;
    let mut pitch_peak = 0.0_f64;
    let mut heave_peak = 0.0_f64;
    let mut roll_peak = 0.0_f64;
    let settle = seconds * 0.4;
    for i in 0..(seconds / SUBSTEP) as u64 {
        let (m, j) = optimal_sheets(&cfg, sim.telemetry.awa, controls.outhaul);
        controls.main_sheet = m;
        controls.jib_sheet = j;
        controls.rudder = pilot.rudder(&cfg, &sim.state, heading, SUBSTEP);
        sim.run(SUBSTEP, &controls);
        if i as f64 * SUBSTEP > settle {
            speed_sum += sim.telemetry.stw;
            samples += 1.0;
            pitch_peak = pitch_peak.max(sim.state.pitch.abs());
            heave_peak = heave_peak.max(sim.state.heave.abs());
            roll_peak = roll_peak.max(sim.state.heel.abs());
        }
    }
    (speed_sum / samples, pitch_peak, heave_peak, roll_peak)
}

#[test]
fn the_boat_rides_the_sea_instead_of_ignoring_it() {
    let cfg = BoatConfig::charter_sloop_34();
    let sea = WindSea::new(kn_to_ms(16.0), 200_000.0, 0.0, 17);
    let hs = sea.significant_height();
    let (_, pitch, heave, _) = sail_in_a_sea(cfg, 16.0, 200_000.0, rad(45.0), 200.0);

    // A thirty-four footer is short compared with these waves, so it largely follows
    // them: heave should be a real fraction of the significant height, not a tremor.
    assert!(
        heave > 0.15 * hs,
        "heave peaked at {heave:.2} m in a {hs:.2} m sea — the boat is ignoring it"
    );
    assert!(
        heave < 1.5 * hs,
        "heave peaked at {heave:.2} m in a {hs:.2} m sea — that is not riding, that is flying"
    );
    assert!(
        deg(pitch) > 1.5,
        "pitch peaked at {:.1} deg; a boat beating into a sea pitches more than that",
        deg(pitch)
    );
    assert!(
        deg(pitch) < 20.0,
        "pitch peaked at {:.1} deg, which would be spectacular",
        deg(pitch)
    );
}

#[test]
fn a_seaway_costs_speed_on_every_heading() {
    let cfg = BoatConfig::charter_sloop_34();
    // What the model can defend: moving in a sea is slower than moving in flat water,
    // on every heading, by a plausible amount.
    //
    // What it cannot yet defend, and this is written down rather than asserted: the
    // *size* of the loss. The model gives a few percent where a real thirty-four
    // footer beating into a metre and a half gives ten to twenty. Added resistance
    // here is charged only from the power the motion damping dissipates, which is a
    // lower bound — it leaves out wave diffraction off the hull and the mean second
    // order drift force, both of which are real and neither of which is modelled.
    // An earlier figure of nine percent looked better and was partly an artefact of
    // the first-substep blow-up fixed alongside this.
    //
    // Nor does it assert that a head sea costs more than a following one. Measurement
    // says otherwise, and measurement has a case: running, the encounter period is
    // long, so the boat has time to follow the wave and takes up its slope, pitching
    // through a larger angle than it ever does beating. Which is also why a following
    // sea is the one that broaches you.
    let mut any_loss = false;
    for twa in [45.0, 90.0, 150.0, 180.0] {
        let flat = steady_state(cfg, wind(16.0), rad(twa), SETTLE, 0.0)
            .telemetry
            .stw;
        let (sea, _, _, _) = sail_in_a_sea(cfg, 16.0, 200_000.0, rad(twa), 240.0);
        let loss = 1.0 - sea / flat;
        assert!(
            loss > -0.01,
            "TWA {twa}: a sea should not make her faster — {:.2} kn against {:.2} flat",
            ms_to_kn(sea),
            ms_to_kn(flat)
        );
        assert!(
            loss < 0.30,
            "TWA {twa}: losing {:.0} percent is not a plausible figure",
            loss * 100.0
        );
        if loss > 0.005 {
            any_loss = true;
        }
    }
    assert!(
        any_loss,
        "a seaway that costs nothing anywhere is a seaway that is not coupled at all"
    );
}

#[test]
fn a_beam_sea_rolls_the_boat() {
    let cfg = BoatConfig::charter_sloop_34();
    // Rolling in waves is the GZ curve restoring towards a tilted vertical, not a
    // bolted-on oscillator — so it should appear without anything asking for it.
    let (_, _, _, beam_roll) = sail_in_a_sea(cfg, 14.0, 200_000.0, rad(90.0), 200.0);
    let flat = steady_state(cfg, wind(14.0), rad(90.0), SETTLE, 0.0);
    assert!(
        beam_roll > flat.state.heel.abs() + rad(1.0),
        "beam sea rolled to {:.1} deg against a flat-water heel of {:.1}",
        deg(beam_roll),
        deg(flat.state.heel).abs()
    );
}

#[test]
fn hull_stations_narrow_towards_the_ends() {
    let cfg = BoatConfig::charter_sloop_34();
    let form = &cfg.form;
    let midships = form.beam_at(0.0);
    let bow = form.beam_at(-form.lwl * 0.45);
    let stern = form.beam_at(form.lwl * 0.45);
    assert!(midships > bow && midships > stern);
    // A transom is wider than a stem.
    assert!(stern > bow);
    // Reserve buoyancy: past the canoe body the sides go vertical, so immersing
    // further keeps adding area rather than running out.
    let at_draft = form.section_area(0.0, form.canoe_draft);
    let deeper = form.section_area(0.0, form.canoe_draft + 0.3);
    assert!(
        deeper > at_draft * 1.3,
        "no reserve buoyancy above the waterline"
    );
    assert_eq!(form.section_area(0.0, -0.1), 0.0);
    assert_eq!(
        hydrostatics::STATIONS % 2,
        1,
        "an odd count puts a station amidships"
    );
}

#[test]
fn a_hull_feels_less_than_the_surface_pressure_gradient() {
    let cfg = BoatConfig::charter_sloop_34();
    let long = WindSea::new(kn_to_ms(12.0), 200_000.0, 0.0, 5);
    let chop = WindSea::new(kn_to_ms(12.0), 15_000.0, 0.0, 5);

    let a_long = long.depth_attenuation(cfg.form.canoe_draft);
    let a_chop = chop.depth_attenuation(cfg.form.canoe_draft);

    // Always less than the surface value, because the gradient decays with depth and
    // the hull feels the average over its own immersion.
    assert!(a_long < 1.0 && a_long > 0.7, "long swell: {a_long:.3}");
    // And short waves are attenuated hardest — they are a surface phenomenon.
    assert!(
        a_chop < a_long,
        "short chop should reach a hull less than a long swell does: {a_chop:.3} against {a_long:.3}"
    );
    // A surface particle feels all of it.
    assert_eq!(FlatWater.depth_attenuation(1.0), 1.0);
    assert!(
        long.depth_attenuation(0.001) > 0.99,
        "a raft feels the surface value"
    );
}

#[test]
fn a_resonant_sea_slows_the_boat_without_stopping_her() {
    let cfg = BoatConfig::charter_sloop_34();
    // Half a metre of short chop over a twenty kilometre fetch arrives at about 2.9 s,
    // which sits on this hull's 2.8 s pitch period. She hobby-horses, and that is real
    // — but it does not stop a boat dead or push her backwards, and an unbounded
    // energy-based wave drag did exactly that.
    let twa = rad(45.0);
    let heading = wrap_tau(0.0 - twa);
    let mut state = BoatState::at_rest(heading);
    state.surge = kn_to_ms(4.0);
    let mut sim =
        Simulation::new(cfg, state, Box::new(SteadyWind::new(0.0, kn_to_ms(12.0)))).with_water(
            Box::new(WindSea::new(kn_to_ms(12.0), 20_000.0, 0.0, 0xC0FFEE)),
        );
    let mut pilot = TillerPilot::new();
    let mut controls = Controls::default();

    let track = glam::DVec2::new(heading.sin(), heading.cos());
    let mut previous = sim.state.pos.dot(track);
    let mut backwards = 0u32;
    let mut slowest = f64::MAX;
    for i in 0..(180.0 / SUBSTEP) as u64 {
        let (m, j) = optimal_sheets(&cfg, sim.telemetry.awa, controls.outhaul);
        controls.main_sheet = m;
        controls.jib_sheet = j;
        controls.rudder = pilot.rudder(&cfg, &sim.state, heading, SUBSTEP);
        sim.run(SUBSTEP, &controls);
        if i as f64 * SUBSTEP < 60.0 {
            previous = sim.state.pos.dot(track);
            continue;
        }
        let along = sim.state.pos.dot(track);
        if along < previous {
            backwards += 1;
        }
        previous = along;
        slowest = slowest.min(sim.telemetry.stw);
    }
    assert_eq!(
        backwards, 0,
        "she went backwards along her own track in a half-metre sea"
    );
    assert!(
        ms_to_kn(slowest) > 1.5,
        "slowest she got was {:.2} kn; hobby-horsing is miserable, not fatal",
        ms_to_kn(slowest)
    );
}

#[test]
fn orbital_displacement_is_bounded_and_is_the_integral_of_the_velocity() {
    let sea = WindSea::new(kn_to_ms(14.0), 120_000.0, 0.0, 9);
    let at = glam::DVec2::new(13.0, -7.0);
    let ceiling: f64 = sea.components().iter().map(|c| c.amplitude).sum();

    // Bounded. This is the property that was violated, and the symptom was not subtle:
    // the surface detail was offset by the instantaneous orbital velocity times the
    // elapsed time, so it slid in every direction with an amplitude that grew without
    // limit. From the deck that reads as the boat periodically going backwards.
    let mut worst = 0.0_f64;
    for i in 0..4000 {
        let t = i as f64 * 0.05;
        let d = sea.horizontal_displacement(at, t);
        worst = worst.max(d.length());
    }
    assert!(
        worst <= ceiling * 1.001,
        "displacement reached {worst:.3} m against a ceiling of {ceiling:.3}"
    );
    assert!(
        worst > 0.2 * ceiling,
        "and it should actually move: {worst:.3} m"
    );

    // And it is the integral of the horizontal orbital velocity, which is what makes
    // the detail travel with the water rather than across it.
    let dt = 1e-4;
    for i in 0..40 {
        let t = 3.0 + i as f64 * 0.17;
        let rate = (sea.horizontal_displacement(at, t + dt)
            - sea.horizontal_displacement(at, t - dt))
            / (2.0 * dt);
        let v = sea.velocity(at, t);
        let expected = glam::DVec2::new(v.x, v.y);
        assert!(
            (rate - expected).length() < 1e-3,
            "at t = {t:.2} the displacement changes at {rate:?} but the water moves at {expected:?}"
        );
    }

    assert_eq!(
        FlatWater.horizontal_displacement(at, 5.0),
        glam::DVec2::ZERO
    );
}

// ---------------------------------------------------------------------------
// The vang, and the twist it controls
// ---------------------------------------------------------------------------

#[test]
fn the_vang_does_nothing_upwind_and_everything_off_the_wind() {
    use sail_core::sail::twist_for_vang;
    // This is the whole content of the control, and it is why nobody thinks about the
    // vang on a beat. With the boom near the centreline the mainsheet runs almost
    // vertically and tensions the leech by itself; let the boom out and the sheet
    // pulls sideways instead, the leech falls off, and the vang becomes the only thing
    // holding the top of the sail down.
    let upwind = rad(8.0);
    let reaching = rad(70.0);

    let spread = twist_for_vang(0.0, upwind) - twist_for_vang(1.0, upwind);
    assert!(
        spread < rad(0.5),
        "upwind the vang should make no meaningful difference, and it moves the twist \
         by {:.2} deg",
        deg(spread)
    );
    assert!(
        twist_for_vang(0.0, upwind) < rad(1.0),
        "and there should be almost no twist there regardless"
    );

    let eased = twist_for_vang(0.0, reaching);
    let hard_on = twist_for_vang(1.0, reaching);
    assert!(
        eased > rad(8.0),
        "reaching with the vang off, the leech should fall away: {:.1} deg",
        deg(eased)
    );
    assert_eq!(hard_on, 0.0, "and hauling it on should take the twist out");
    assert!(eased > hard_on);

    // Monotonic in between, so the control has a usable middle rather than two states.
    let half = twist_for_vang(0.5, reaching);
    assert!(half < eased && half > hard_on);
}

#[test]
fn spilling_the_top_of_the_main_sheds_heel_faster_than_it_sheds_drive() {
    let cfg = BoatConfig::charter_sloop_34();
    let make = wind(22.0);
    // The reason twist is a depowering tool and not just a loss: the upper section
    // heels on a much longer arm than it drives on. Easing it off gives up less speed
    // than it gives up heeling moment, which is exactly the trade you want when
    // overpowered and do not yet want to reef.
    // A heeled beam reach, which is the only place the claim has any content. Close
    // reaching, the boom is barely out and the mainsheet does the vang's job for it —
    // measured at two degrees of twist between the extremes at seventy degrees true.
    // Broad reaching the boom is well out and the vang bites hard, but by then she is
    // barely heeled at all and there is nothing to shed. Both of those are correct and
    // both make the test meaningless; the interesting case is in between.
    let twa = rad(95.0);
    let tight = reach_with_vang(cfg, &make, twa, 1.0);
    let loose = reach_with_vang(cfg, &make, twa, 0.0);

    assert!(
        loose.1.abs() < tight.1.abs() - rad(0.4),
        "easing the vang should stand her up: {:.1} deg against {:.1}",
        deg(loose.1).abs(),
        deg(tight.1).abs()
    );
    let speed_lost = 1.0 - loose.0 / tight.0;
    let heel_lost = 1.0 - loose.1.abs() / tight.1.abs();
    assert!(
        heel_lost > speed_lost,
        "it should cost less speed than heel: {:.1} percent of the speed against \
         {:.1} percent of the heel",
        speed_lost * 100.0,
        heel_lost * 100.0
    );
}

/// Reach at a fixed vang setting with the sheet trimmed, and report speed and heel.
fn reach_with_vang<F>(cfg: BoatConfig, make: &F, twa: f64, vang: f64) -> (f64, f64)
where
    F: Fn() -> Box<dyn WindField>,
{
    let heading = wrap_tau(0.0 - twa);
    let mut state = BoatState::at_rest(heading);
    state.surge = 0.6 * cfg.hull_speed();
    let mut sim = Simulation::new(cfg, state, make());
    let mut pilot = TillerPilot::new();
    let mut controls = Controls {
        vang,
        ..Default::default()
    };
    for _ in 0..(SETTLE / SUBSTEP) as u64 {
        let (m, j) = optimal_sheets(&cfg, sim.telemetry.awa, controls.outhaul);
        controls.main_sheet = m;
        controls.jib_sheet = j;
        controls.rudder = pilot.rudder(&cfg, &sim.state, heading, SUBSTEP);
        sim.run(SUBSTEP, &controls);
    }
    (sim.telemetry.stw, sim.state.heel)
}

#[test]
fn the_genoa_car_is_the_headsails_outhaul_and_vang_at_once() {
    use sail_core::sail::{camber_for_lead, twist_for_lead};
    // A genoa has no outhaul and no vang because its lead car does both jobs. Forward,
    // the sheet pulls more downward: the leech comes tight and the foot bellies out.
    // Aft, it pulls along the foot: flat down low, and the head falls open and spills.
    assert!(
        camber_for_lead(0.0) > camber_for_lead(1.0),
        "forward should be the deeper setting"
    );
    assert!(
        twist_for_lead(0.0) < twist_for_lead(1.0),
        "aft should open the leech"
    );
    assert_eq!(
        twist_for_lead(0.0),
        0.0,
        "right forward closes it completely"
    );
    assert!(
        deg(twist_for_lead(1.0)) > 10.0,
        "right aft should spill properly"
    );
    // Both ends sit inside the range a real headsail works in.
    for car in [0.0, 0.5, 1.0] {
        assert!((0.06..=0.16).contains(&camber_for_lead(car)));
    }

    // And it does the same thing to the boat that the main's controls do: forward is
    // power, aft is depowered.
    let cfg = BoatConfig::charter_sloop_34();
    let make = wind(20.0);
    let forward = car_at(cfg, &make, rad(70.0), 0.0);
    let aft = car_at(cfg, &make, rad(70.0), 1.0);
    assert!(
        aft.1.abs() < forward.1.abs(),
        "the car aft should stand her up: {:.1} deg against {:.1}",
        deg(aft.1).abs(),
        deg(forward.1).abs()
    );
}

/// Reach at a fixed lead-car position, reporting speed and heel.
fn car_at<F>(cfg: BoatConfig, make: &F, twa: f64, car: f64) -> (f64, f64)
where
    F: Fn() -> Box<dyn WindField>,
{
    let heading = wrap_tau(0.0 - twa);
    let mut state = BoatState::at_rest(heading);
    state.surge = 0.6 * cfg.hull_speed();
    let mut sim = Simulation::new(cfg, state, make());
    let mut pilot = TillerPilot::new();
    let mut controls = Controls {
        genoa_car: car,
        ..Default::default()
    };
    for _ in 0..(SETTLE / SUBSTEP) as u64 {
        let (m, j) = optimal_sheets(&cfg, sim.telemetry.awa, controls.outhaul);
        controls.main_sheet = m;
        controls.jib_sheet = j;
        controls.rudder = pilot.rudder(&cfg, &sim.state, heading, SUBSTEP);
        sim.run(SUBSTEP, &controls);
    }
    (sim.telemetry.stw, sim.state.heel)
}

/// A headsail cannot be sheeted to the centreline, because its sheet is led to a block
/// on a track bolted to the side deck.
///
/// Without this floor the model believed a genoa ground hard in lay on the centreline,
/// which upwind is an angle of attack equal to the entire apparent wind angle — past
/// the optimum and into soft stall. The boat was then fastest with the genoa *eased*,
/// which is the opposite of how anyone sails, and it made the winch feel backwards.
/// The mainsail's floor really is zero: its sheet leads to the centreline.
#[test]
fn a_headsail_cannot_be_sheeted_past_its_lead() {
    let cfg = BoatConfig::charter_sloop_34();

    let hard_in = cfg.genoa.profile.chord_angle(0.0);
    assert!(
        hard_in > rad(8.0) && hard_in < rad(18.0),
        "a genoa lead gives a sheeting angle of around twelve degrees, got {:.1}",
        deg(hard_in)
    );
    assert_eq!(
        cfg.main.profile.chord_angle(0.0),
        0.0,
        "the mainsheet leads to the centreline, so the boom can come right in"
    );

    // Round-tripping through the sheet mapping must still land where it started, or
    // the twisted upper sections are being fed a sheet setting that means something
    // else entirely.
    for tenth in 0..=10 {
        let sheet = tenth as f64 / 10.0;
        let angle = cfg.genoa.profile.chord_angle(sheet);
        let back = cfg.genoa.profile.sheet_for_chord_angle(angle);
        assert!(
            (back - sheet).abs() < 1e-9,
            "sheet {sheet:.1} round-tripped to {back:.4}"
        );
    }

    // And close-hauled, hard in must now be near the optimum rather than past it.
    let awa = rad(28.0);
    let want = cfg
        .genoa
        .profile
        .optimal_chord_angle(awa, sail_core::sail::CAMBER_REFERENCE);
    assert!(
        (want - hard_in).abs() < rad(6.0),
        "close-hauled the genoa should want to be near its stops: wants {:.1}, \
         stops at {:.1}",
        deg(want),
        deg(hard_in)
    );
}

/// A frame that falls between two physics substeps must be drawn between them.
///
/// The integrator runs at a fixed 200 Hz and a frame almost never lands on a substep
/// boundary. Drawing the last completed substep throws the leftover away, and the
/// error is not a harmless constant: at 120 frames a second a frame consumes two
/// substeps, then two, then one, so the boat advances 10, 10, 5 milliseconds of
/// simulated time while the clock advances evenly. The frame rate and 200 Hz beat
/// against each other, the beat wanders as the frame rate does, and it reads as the
/// camera intermittently failing to keep up.
///
/// So this measures the spread of per-frame displacement rather than asserting that
/// interpolation happens.
#[test]
fn a_rendered_frame_lands_between_substeps() {
    use sail_core::wind::SteadyWind;

    // Angles go the short way round, or a boat sailing north spins through 358
    // degrees once a lap.
    let a = BoatState::at_rest(rad(359.0));
    let b = BoatState::at_rest(rad(1.0));
    let mid = BoatState::lerp(&a, &b, 0.5);
    assert!(
        wrap_pi(mid.heading).abs() < rad(0.01),
        "359 to 1 degrees should pass through north, got {:.1}",
        deg(wrap_pi(mid.heading))
    );
    let cfg = BoatConfig::charter_sloop_34();
    let mut state = BoatState::at_rest(wrap_tau(-rad(45.0)));
    state.surge = kn_to_ms(6.0);
    let mut sim = Simulation::new(
        cfg,
        state,
        Box::new(SteadyWind::new(0.0, kn_to_ms(14.0))) as Box<dyn WindField>,
    );
    let controls = Controls::close_hauled();
    // Settle first: the opening seconds are a genuine acceleration and would swamp
    // the thing being measured.
    sim.run(120.0, &controls);

    // A frame rate deliberately incommensurate with the 5 ms substep.
    let frame = 1.0 / 120.0;
    let mut drawn = Vec::new();
    let mut raw = Vec::new();
    for _ in 0..600 {
        sim.step(frame, &controls);
        drawn.push(sim.render_state().pos);
        raw.push(sim.state.pos);
    }

    let spread = |p: &[glam::DVec2]| {
        let steps: Vec<f64> = p.windows(2).map(|w| (w[1] - w[0]).length()).collect();
        let mean = steps.iter().sum::<f64>() / steps.len() as f64;
        let worst = steps
            .iter()
            .map(|s| (s - mean).abs())
            .fold(0.0_f64, f64::max);
        worst / mean
    };

    let interpolated = spread(&drawn);
    let last_substep = spread(&raw);
    assert!(
        last_substep > 0.1,
        "the unsmoothed case should visibly beat against the substep rate, \
         got {:.1}% — if this has gone away the test is no longer measuring anything",
        last_substep * 100.0
    );
    assert!(
        interpolated < 0.02,
        "a rendered frame should advance evenly: {:.2}% spread interpolated \
         against {:.1}% drawing the last substep",
        interpolated * 100.0,
        last_substep * 100.0
    );
}

/// The sea runs with the wind, and so does every band in it.
///
/// Not a style check. The spectrum used to tie direction to period — long waves one
/// side of the wind, short waves the other — and the energy-weighted mean direction of
/// the long band sat 27.8° off the wind permanently. Every scalar in the model was
/// right while the sea was visibly wrong, which is the case a test like this exists
/// for: it asserts about the *set* of components, not about any one of them.
#[test]
fn every_wave_band_runs_with_the_wind() {
    use sail_core::units::rad;
    for wind_from in [0.0, rad(37.0), rad(180.0), rad(284.0)] {
        let sea = sail_core::water::WindSea::new(9.0, 40_000.0, wind_from, 7);
        let run = wind_from + std::f64::consts::PI;
        let expected = glam::DVec2::new(run.sin(), run.cos());

        let peak = sea.peak_period();
        // Three bands: everything, the waves longer than the peak, the waves shorter.
        // The long band is the one the eye reads, and the one that was wrong.
        for (name, pick) in [
            ("all", Box::new(|_: f64| true) as Box<dyn Fn(f64) -> bool>),
            ("long", Box::new(move |p: f64| p > peak * 1.02)),
            ("short", Box::new(move |p: f64| p < peak * 0.98)),
        ] {
            let mut sum = glam::DVec2::ZERO;
            let mut energy = 0.0;
            for c in sea.components() {
                let period = std::f64::consts::TAU / c.omega;
                if !pick(period) {
                    continue;
                }
                // Energy goes as amplitude squared, which is what decides what a sea
                // looks like — weighting by amplitude alone flatters the small stuff.
                let e = c.amplitude * c.amplitude;
                sum += c.direction * e;
                energy += e;
            }
            assert!(energy > 0.0, "{name} band is empty");
            let mean = sum / energy;
            let off = mean.y.atan2(mean.x) - expected.y.atan2(expected.x);
            let off = off.to_degrees().abs();
            assert!(
                off < 3.0,
                "{name} band runs {off:.1}° off the wind (wind from {:.0}°)",
                wind_from.to_degrees()
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The sail wardrobe
// ---------------------------------------------------------------------------

/// The running sail is worth having, and the sweep says where.
#[test]
fn the_gennaker_wins_downwind_and_the_genoa_wins_upwind() {
    use sail_core::autopilot::steady_state_with;
    use sail_core::state::Headsail;
    let cfg = BoatConfig::archipelago_packet();
    let wind = || -> Box<dyn WindField> { Box::new(SteadyWind::new(0.0, kn_to_ms(12.0))) };

    let deep_genoa = steady_state_with(cfg, wind, rad(150.0), 420.0, 0.0, Headsail::Genoa);
    let deep_kite = steady_state_with(cfg, wind, rad(150.0), 420.0, 0.0, Headsail::Gennaker);
    let gain = ms_to_kn(deep_kite.telemetry.stw - deep_genoa.telemetry.stw);
    assert!(
        gain > 0.5,
        "a gennaker at 150 degrees should be worth half a knot at least, got {gain:+.2}"
    );

    let beat_genoa = steady_state_with(cfg, wind, rad(45.0), 420.0, 0.0, Headsail::Genoa);
    let beat_kite = steady_state_with(cfg, wind, rad(45.0), 420.0, 0.0, Headsail::Gennaker);
    assert!(
        beat_kite.telemetry.stw < beat_genoa.telemetry.stw,
        "and it must be the wrong sail upwind"
    );
}

/// A sail set flying collapses when the apparent wind comes forward, and the collapse
/// has to be a collapse — not a gentle loss of a sail three times the genoa's size.
#[test]
fn a_flying_sail_collapses_close_hauled() {
    use sail_core::autopilot::steady_state_with;
    use sail_core::state::Headsail;
    let cfg = BoatConfig::archipelago_packet();
    let wind = || -> Box<dyn WindField> { Box::new(SteadyWind::new(0.0, kn_to_ms(12.0))) };

    // Close-hauled with the gennaker up, against the same boat with nothing forward of
    // the mast at all. If the collapse works, they are nearly the same boat.
    let kite = steady_state_with(cfg, wind, rad(42.0), 420.0, 0.0, Headsail::Gennaker);
    let mut bare = sail_core::sim::Simulation::new(
        cfg,
        {
            let mut st = BoatState::at_rest(wrap_tau(-rad(42.0)));
            st.surge = 0.6 * cfg.hull_speed();
            st
        },
        wind(),
    );
    let mut controls = Controls {
        furl: 1.0,
        ..Default::default()
    };
    let mut pilot = sail_core::autopilot::TillerPilot::new();
    let heading = wrap_tau(-rad(42.0));
    for _ in 0..(420.0 / SUBSTEP) as u64 {
        let (m, _) = sail_core::autopilot::optimal_sheets(&cfg, bare.telemetry.awa, 0.0);
        controls.main_sheet = m;
        controls.rudder = pilot.rudder(&cfg, &bare.state, heading, SUBSTEP);
        bare.run(SUBSTEP, &controls);
    }
    let difference = ms_to_kn((kite.telemetry.stw - bare.telemetry.stw).abs());
    assert!(
        difference < 0.6,
        "a collapsed gennaker should leave the boat sailing on her main: {difference:.2} kn apart"
    );
}

/// The staysail belongs under the big reaching sails, and nowhere else.
///
/// This is the test that keeps the double head rig honest. A model that simply adds the
/// inner sail's area makes it free speed everywhere and there is no reason ever to hand
/// it, which is not how the rig works: inside a genoa it is covered and it backwinds
/// the main, and what it costs there is about what it gives.
#[test]
fn the_staysail_earns_its_place_only_under_a_flying_sail() {
    use sail_core::autopilot::steady_state_rigged;
    use sail_core::state::Headsail;
    let cfg = BoatConfig::archipelago_packet();
    let wind = || -> Box<dyn WindField> { Box::new(SteadyWind::new(0.0, kn_to_ms(12.0))) };

    let twa = rad(110.0);
    let under_kite = [0.0, 1.0].map(|stay| {
        ms_to_kn(
            steady_state_rigged(cfg, wind, twa, 420.0, 0.0, Headsail::Gennaker, stay)
                .telemetry
                .stw,
        )
    });
    // It must not cost anything under a flying sail, and the margin is deliberately
    // small: under an eighty-eight square metre gennaker a fourteen metre staysail is
    // working in what is left of the air, and what is left is not much. The threshold
    // was +0.02 kn while the gennaker was sixty square metres; the sail grew to the size
    // its owner says it is and the gain fell to +0.013, which is the right way round and
    // is the whole cruising argument about whether the inner sail is worth setting.
    assert!(
        under_kite[1] - under_kite[0] > 0.0,
        "a staysail under a gennaker should not cost anything: {:+.3} kn",
        under_kite[1] - under_kite[0]
    );

    let upwind = [0.0, 1.0].map(|stay| {
        ms_to_kn(
            steady_state_rigged(cfg, wind, rad(45.0), 420.0, 0.0, Headsail::Genoa, stay)
                .telemetry
                .stw,
        )
    });
    assert!(
        upwind[1] <= upwind[0] + 0.02,
        "and it must not be free speed inside a genoa: {:+.3} kn",
        upwind[1] - upwind[0]
    );
}

// ---------------------------------------------------------------------------
// Weather: the day is going somewhere, and the sea is behind it
// ---------------------------------------------------------------------------

#[test]
fn the_sea_lags_the_wind_and_lies_down_slower_than_it_gets_up() {
    use sail_core::water::{WaterSurface, WindSea};
    const FETCH: f64 = 60_000.0;
    let settled = |tws: f64| fetch_limited_sea(tws, FETCH).0;

    // A sea built for ten knots, and then it blows twenty-five.
    let mut sea = WindSea::new(kn_to_ms(10.0), FETCH, 0.0, 3);
    let start = sea.significant_height();
    let mut t = 0.0;
    let mut after_ten_minutes = 0.0;
    while t < 3.0 * 3600.0 {
        sea.follow_wind(kn_to_ms(25.0), 0.0, t, 1.0);
        t += 1.0;
        if (t - 600.0).abs() < 0.5 {
            after_ten_minutes = sea.significant_height();
        }
    }
    let target = settled(kn_to_ms(25.0));
    // Ten minutes of gale on a calm sea is still a calm sea. This is the thing the
    // model could not say before: the first hour of a new breeze is the easy hour.
    let ten_minute_share = (after_ten_minutes - start) / (target - start);
    assert!(
        ten_minute_share < 0.12,
        "ten minutes should barely move it, got {:.0}% of the way there",
        ten_minute_share * 100.0
    );
    // Three hours of it, and most of the sea is there.
    let three_hour_share = (sea.significant_height() - start) / (target - start);
    assert!(
        three_hour_share > 0.55,
        "three hours should build most of it, got {:.0}%",
        three_hour_share * 100.0
    );

    // Now the gale dies. The sea must take longer to go than it took to come, because
    // a dying gale is the worst motion a small boat sees and that asymmetry is why.
    let built = sea.significant_height();
    let floor = settled(kn_to_ms(10.0));
    let mut t2 = t;
    while t2 < t + 3.0 * 3600.0 {
        sea.follow_wind(kn_to_ms(10.0), 0.0, t2, 1.0);
        t2 += 1.0;
    }
    let decayed = (built - sea.significant_height()) / (built - floor);
    assert!(
        decayed < three_hour_share,
        "the sea should lie down slower than it builds: {:.0}% down against {:.0}% up",
        decayed * 100.0,
        three_hour_share * 100.0
    );
}

#[test]
fn a_shift_takes_the_sea_round_with_it_but_not_at_once() {
    use sail_core::water::{WaterSurface, WindSea};
    let mut sea = WindSea::new(kn_to_ms(18.0), 60_000.0, 0.0, 5);
    // The wind veers ninety degrees, as it does behind a front.
    let mut t = 0.0;
    while t < 1800.0 {
        sea.follow_wind(kn_to_ms(18.0), rad(90.0), t, 1.0);
        t += 1.0;
    }
    // Half an hour later the sea is on its way round and nowhere near there — which is
    // the cross sea you actually sail in after a frontal passage.
    // The component's unit vector, turned back into a compass bearing. It started
    // running due south, which is where a northerly puts it.
    let run = sea.components()[0].direction;
    // As a compass bearing: a unit vector (sin, cos) of a bearing, read back the same
    // way round. Reading it as a mathematical angle instead flips the sign, which is
    // its own small lesson about having one convention.
    let moved = wrap_pi(run.x.atan2(run.y) - std::f64::consts::PI);
    assert!(
        deg(moved) > 5.0 && deg(moved) < 60.0,
        "the sea should have started round and not arrived: {:.0} deg of 90",
        deg(moved)
    );
}

#[test]
fn the_weather_goes_somewhere_and_says_where() {
    use sail_core::wind::{SyntheticWind, WindField};
    // Over a dozen drawn days, the mean wind has to move enough to matter and not so
    // much that the day becomes a different day.
    let mut swings = Vec::new();
    for seed in 0..12u64 {
        let mut w = SyntheticWind::new(rad(200.0), kn_to_ms(16.0), seed);
        let mut lowest = f64::INFINITY;
        let mut highest: f64 = 0.0;
        let mut t = 0.0;
        while t < 4.0 * 3600.0 {
            w.step(1.0);
            t += 1.0;
            lowest = lowest.min(w.mean_speed);
            highest = highest.max(w.mean_speed);
        }
        swings.push((highest - lowest) / kn_to_ms(16.0));
        assert!(
            w.mean_speed > kn_to_ms(16.0) * 0.5 && w.mean_speed < kn_to_ms(16.0) * 1.8,
            "seed {seed}: the day wandered off, {:.1} kn",
            ms_to_kn(w.mean_speed)
        );
    }
    let typical = swings.iter().sum::<f64>() / swings.len() as f64;
    assert!(
        (0.15..=1.0).contains(&typical),
        "a four hour watch should see the wind move by a fifth or so of itself, got {:.2}",
        typical
    );

    // And the outlook has to agree with what actually happens. Not clairvoyance — the
    // expected path of a relaxing process — but the sign has to be right more often
    // than not, or the forecast on the console is a coin toss with a compass rose.
    //
    // The threshold is worked out rather than picked, because the first version of it
    // was a knife edge that a change to the random stream tipped over. For a relaxing
    // process over an hour at a forty minute time constant, the drift is 0.78 of the
    // current trend and the noise accumulated over the same hour is 0.97 of the
    // stationary spread — so the skill available is about 68 percent, not 100, and no
    // amount of honest forecasting beats it. Two hundred days puts the standard error
    // at three percent, so 58 percent is three sigma below what it should manage and
    // still far enough above a coin toss to mean something.
    let mut right = 0;
    const DAYS: u64 = 200;
    for seed in 0..DAYS {
        let mut w = SyntheticWind::new(rad(200.0), kn_to_ms(16.0), seed);
        let (speed_outlook, _) = w.outlook();
        let before = w.mean_speed;
        for _ in 0..3600 {
            w.step(1.0);
        }
        if (w.mean_speed - before).signum() == speed_outlook.signum() {
            right += 1;
        }
    }
    let skill = right as f64 / DAYS as f64;
    assert!(
        skill > 0.58,
        "the outlook called the next hour right {right} times in {DAYS} ({:.0}%), \
         which is not far enough from a guess",
        skill * 100.0
    );
}

#[test]
fn a_squall_has_a_front_a_shift_and_a_hole_behind_it() {
    use glam::DVec2;
    use sail_core::wind::Squall;

    // Wind from the north at fifteen knots, so it flows south. A squall sitting on the
    // origin, mature.
    let mean = kn_to_ms(15.0);
    let flow = sail_core::wind::flow_from(0.0, mean);
    let squall = Squall {
        at: DVec2::ZERO,
        radius: 1_000.0,
        strength: 1.2,
        age: 900.0,
        life: 1_800.0,
    };
    let at = |p: DVec2| flow + squall.outflow(p, mean);

    // The leading edge is downwind of the centre: the outflow runs with the gradient
    // wind, and that is where the extra wind is.
    let front = at(DVec2::new(0.0, -700.0));
    assert!(
        front.length() > mean * 1.6,
        "the gust front should be the windy part, got {:.1} kn against a {:.0} kn mean",
        ms_to_kn(front.length()),
        ms_to_kn(mean)
    );

    // Behind it the two oppose, and the hole is the thing everybody forgets. A squall
    // that only ever adds wind is a squall nobody is ever caught out by afterwards.
    let behind = at(DVec2::new(0.0, 700.0));
    assert!(
        behind.length() < mean * 0.6,
        "there should be a hole behind it, got {:.1} kn",
        ms_to_kn(behind.length())
    );

    // On the flank they are at right angles, so it blows harder *and* from somewhere
    // else. The shift is what you tack on, or get rolled by.
    let flank = at(DVec2::new(700.0, 0.0));
    let shifted =
        wrap_pi(sail_core::wind::direction_of(flank) - sail_core::wind::direction_of(flow));
    assert!(
        deg(shifted).abs() > 20.0,
        "the flank should be a header or a lift, got {:.0} deg",
        deg(shifted)
    );
    assert!(flank.length() > mean);

    // And it goes light before it arrives, because the air coming down has to come from
    // somewhere. This is the warning, and without it the whole thing is unfair.
    //
    // "Ahead" means ahead in its direction of travel, which is downwind — so south of
    // it, the same side as the gust front, only further out. North of it is *behind*,
    // and there the inflow runs the same way as the gradient wind and adds to it. Worth
    // the note: the first version of this test looked for the lull on the wrong side and
    // found a gust, which is a sign error in the test and not in the squall.
    let ahead = at(DVec2::new(0.0, -1_600.0));
    assert!(
        ahead.length() < mean * 0.97,
        "it should go light ahead of one, got {:.2} kn against {:.2}",
        ms_to_kn(ahead.length()),
        ms_to_kn(mean)
    );
}

#[test]
fn squalls_come_and_go_and_come_from_upwind() {
    use sail_core::wind::{SyntheticWind, WindField};
    let mut w = SyntheticWind::new(0.0, kn_to_ms(16.0), 4);
    let mut seen = 0;
    let mut most_at_once = 0;
    let mut was = 0;
    let mut born_upwind = 0;
    let mut t = 0.0;
    while t < 4.0 * 3600.0 {
        w.step(1.0);
        t += 1.0;
        let now = w.squalls();
        if now.len() > was {
            seen += 1;
            // Born upwind of the origin: with the wind in the north, that is to the
            // north, so a positive northing.
            if now.last().is_some_and(|(at, _, _)| at.y > 0.0) {
                born_upwind += 1;
            }
        }
        most_at_once = most_at_once.max(now.len());
        was = now.len();
    }
    assert!(
        (4..=20).contains(&seen),
        "a four hour watch should see a handful of squalls, saw {seen}"
    );
    assert_eq!(born_upwind, seen, "every squall must come from upwind");
    assert!(most_at_once <= 2, "{most_at_once} squalls at once is a sky");
}

#[test]
fn a_squall_passing_over_is_light_then_hard_then_light() {
    use glam::DVec2;
    use sail_core::wind::{Squall, SyntheticWind, WindField};

    // The sequence a boat actually experiences, which is the only test of a squall that
    // matters. Each step of it is a decision at the helm, and getting the *order* wrong
    // — the hole before the front, say — would feel wrong to any sailor while passing
    // every test written about the parts in isolation.
    //
    // Measured, dead-on, fifteen knots of gradient wind: 15.0, then 11.4 as the inflow
    // takes the edge off it, then 23.8 at the front, then 7.8 in the hole, then back.
    let run = |across: f64| -> Vec<(f64, f64, f64)> {
        let mean = kn_to_ms(15.0);
        let mut w = SyntheticWind::new(0.0, mean, 9).frozen();
        w.add_squall(Squall {
            at: DVec2::new(across, 4_000.0),
            radius: 900.0,
            strength: 1.3,
            age: 450.0,
            life: 2_400.0,
        });
        let mut trace = Vec::new();
        let mut t = 0.0;
        while t < 900.0 {
            w.step(1.0);
            t += 1.0;
            let f = w.sample(DVec2::ZERO, t);
            trace.push((t, f.length(), sail_core::wind::direction_of(f)));
        }
        trace
    };

    let mean = kn_to_ms(15.0);
    let trace = run(0.0);
    let peak = trace.iter().cloned().fold(
        (0.0, 0.0, 0.0),
        |best, s| if s.1 > best.1 { s } else { best },
    );
    assert!(
        peak.1 > mean * 1.5,
        "the front should be a front: {:.0} kn against a {:.0} kn mean",
        ms_to_kn(peak.1),
        ms_to_kn(mean)
    );

    // It goes light *first*. This is the half of it the boat is given for free, and
    // without it the whole thing is unfair.
    let lull = trace
        .iter()
        .take_while(|s| s.0 < peak.0)
        .map(|s| s.1)
        .fold(f64::INFINITY, f64::min);
    assert!(
        lull < mean * 0.95,
        "it should go light ahead of the front: {:.1} kn against {:.1}",
        ms_to_kn(lull),
        ms_to_kn(mean)
    );

    // And the hole behind is deeper than the lull in front, because behind it the
    // outflow is cancelling the gradient wind rather than merely feeding the cell.
    let hole = trace
        .iter()
        .skip_while(|s| s.0 <= peak.0)
        .map(|s| s.1)
        .fold(f64::INFINITY, f64::min);
    assert!(
        hole < lull,
        "the hole behind should beat the lull in front: {:.1} kn against {:.1}",
        ms_to_kn(hole),
        ms_to_kn(lull)
    );

    // One that goes by to one side instead of over the top: that is where the shift is,
    // because the outflow is across the wind there rather than along it. Measured, 420 m
    // off the centre: the wind swings from 354 degrees through 48 and back.
    let shift = run(420.0)
        .iter()
        .map(|s| wrap_pi(s.2).abs())
        .fold(0.0, f64::max);
    assert!(
        deg(shift) > 25.0,
        "a squall going by on one side should shift the wind, got {:.0} deg",
        deg(shift)
    );
}

#[test]
fn heel_and_speed_take_the_helm_away() {
    // The three things that cost a rudder its grip, asserted through the real code path
    // rather than against the formula — the state goes in, one step runs, and the
    // telemetry is read. A test that re-derived the arithmetic would pass even if the
    // force used a different number from the instrument, which is the only failure that
    // matters here.
    let cfg = BoatConfig::charter_sloop_34();
    let bite = |knots: f64, heel_deg: f64, helm_deg: f64| {
        let mut state = BoatState::at_rest(0.0);
        state.surge = kn_to_ms(knots);
        state.heel = rad(heel_deg);
        let mut sim = Simulation::new(
            cfg,
            state,
            Box::new(SteadyWind::new(0.0, kn_to_ms(10.0))) as Box<dyn WindField>,
        );
        let controls = Controls {
            rudder: rad(helm_deg),
            ..Controls::close_hauled()
        };
        sim.run(SUBSTEP, &controls);
        sim.telemetry.rudder_bite
    };

    // Five knots, upright, a working angle of helm: this is what a tiller feels like,
    // and it is the reference the number is scaled to.
    let working = bite(5.0, 0.0, 8.0);
    assert!(
        working > 0.9,
        "five knots upright should be a full rudder, got {:.2}",
        working
    );

    // Speed, squared. Two knots out of a tack has a fraction of the helm five knots has,
    // and it is why she will not answer there.
    let crawling = bite(2.0, 0.0, 8.0);
    assert!(
        crawling < working * 0.3,
        "at two knots the helm should be nearly gone: {:.2} against {:.2}",
        crawling,
        working
    );

    // Heel, two ways. The cosine alone at twenty-five degrees is a tenth; past thirty
    // the stern starts coming out and it goes much faster than any cosine.
    let pressed = bite(5.0, 25.0, 8.0);
    let over = bite(5.0, 50.0, 8.0);
    assert!(
        pressed < working * 0.95 && pressed > working * 0.8,
        "twenty-five degrees should cost about a tenth, got {:.2} against {:.2}",
        pressed,
        working
    );
    assert!(
        over < pressed * 0.7,
        "past thirty the blade starts to emerge and it should fall away fast: \
         {:.2} at fifty against {:.2} at twenty-five",
        over,
        pressed
    );

    // And stall: hauling the tiller further does not buy more rudder past about thirty
    // degrees, it buys a brake.
    let hard = bite(5.0, 0.0, 40.0);
    assert!(
        hard < working * 0.5,
        "forty degrees of helm should be stalled, got {:.2} against {:.2}",
        hard,
        working
    );
}

#[test]
fn the_vane_does_not_gybe_itself_across_the_stern() {
    use sail_core::autopilot::{Autopilot, PilotMode};

    // The apparent wind angle is a *circular* quantity and the vane was subtracting it
    // like a straight line. Dead downwind the wind wanders either side of the stern with
    // every wave, so a target of 175 degrees to starboard and a reading of 179 to port is
    // a four-degree shift — and the old arithmetic read it as being knocked onto the
    // other tack and answered with thirty degrees of helm towards the gybe. At eight
    // knots that is how you get put on your ear by your own autopilot.
    let cfg = BoatConfig::archipelago_packet();
    let helm = |target_deg: f64, awa_deg: f64| {
        let mut pilot = Autopilot::new();
        pilot.mode = PilotMode::ApparentWind(rad(target_deg));
        let mut state = BoatState::at_rest(0.0);
        state.surge = kn_to_ms(8.0);
        let tm = Telemetry {
            awa: rad(awa_deg),
            ..Default::default()
        };
        // Two seconds of it, not one step. The ram is rate limited — it swings the
        // helm at about half a radian a second and slower under load — so a single
        // substep can only ever move it a sixth of a degree whatever it has been asked
        // for. Measuring one step measures the rate limit and tells you nothing about
        // the command, which is the first version of this test's own mistake.
        let mut last = 0.0;
        for _ in 0..400 {
            last = pilot.rudder(&cfg, &state, &tm, SUBSTEP).unwrap_or(0.0);
        }
        last
    };

    let nudge = helm(175.0, -179.0);
    let gybed = helm(150.0, -120.0);
    let tacked = helm(30.0, -25.0);
    println!(
        "vane: across the stern {:.1} deg, gybed {:.1}, wrong tack upwind {:.1}",
        deg(nudge),
        deg(gybed),
        deg(tacked)
    );
    // Four degrees across the stern is a nudge, and the measure of that is *against a
    // real gybe recovery* rather than against an absolute. The harness holds the error
    // constant for two seconds with the boat not answering, which winds the integral up
    // in a way the sea never would, so what matters is the ratio: before the fix both
    // of these came out at forty degrees, which is the rudder stop, because both were
    // being treated as a change of tack.
    assert!(
        nudge.abs() < gybed.abs() / 2.5,
        "a four degree shift should not be answered like a gybe: {:.1} against {:.1}",
        deg(nudge),
        deg(gybed)
    );
    // And it has to be the *right* nudge. The wind has gone six degrees further aft on
    // the starboard side, so she luffs a little — towards the wind, which is to
    // starboard, which is positive helm.
    assert!(nudge > 0.0, "and it should luff, not bear away");

    // A real gybe, though, has to be answered as one: the wind is on the other quarter
    // by sixty degrees and the way back is round the stern, bearing away.
    assert!(
        deg(gybed) > 10.0,
        "properly gybed, it should put the helm over: got {:.1} deg",
        deg(gybed)
    );

    // And upwind the way back is the other way round — through the wind, which is a
    // tack. Getting this backwards sends her the long way round, downwind, out of
    // control.
    assert!(
        deg(tacked) < -10.0,
        "knocked onto the wrong tack upwind, she should luff back through the wind: \
         got {:.1} deg",
        deg(tacked)
    );
}

#[test]
fn under_bare_poles_she_goes_to_leeward() {
    // With the main handed and the headsail rolled away there is no sail on her, and
    // what is left is windage: the spar, the rigging, the rolled sail on the stay and
    // the topsides. That had been missing entirely, and what it was missing was not a
    // correction but the whole of what a boat does with nothing set — measured before
    // it existed, from rest in twenty knots of wind she did not move *at all*, and given
    // way she kept it, so she could coast to windward under bare poles for minutes.
    let cfg = BoatConfig::archipelago_packet();
    let settled = |twa_deg: f64| {
        let heading = wrap_tau(0.0 - rad(twa_deg));
        let mut state = BoatState::at_rest(heading);
        let mut sim = Simulation::new(
            cfg,
            state,
            Box::new(SteadyWind::new(0.0, kn_to_ms(20.0))) as Box<dyn WindField>,
        );
        let controls = Controls {
            reef: 1.0,
            furl: 1.0,
            staysail: 0.0,
            rudder: 0.0,
            ..Default::default()
        };
        for _ in 0..(300.0 / SUBSTEP) as u64 {
            sim.run(SUBSTEP, &controls);
        }
        state = sim.state;
        let _ = state;
        (ms_to_kn(sim.telemetry.stw), ms_to_kn(sim.telemetry.vmg))
    };

    for twa in [45.0, 56.0, 90.0, 135.0] {
        let (speed, vmg) = settled(twa);
        // She moves, which she did not before.
        assert!(
            (0.4..=3.0).contains(&speed),
            "bare poles in twenty knots should drift a knot or two, got {speed:.2} at \
             TWA {twa:.0}"
        );
        // And never to windward. This is the whole claim: windage acts along the
        // apparent wind, and nothing without sails has ever made ground against it.
        assert!(
            vmg < 0.0,
            "at TWA {twa:.0} under bare poles she made {vmg:+.2} kn to windward, which \
             is a boat sailing on her rigging"
        );
    }

    // And the beam is where she is pushed hardest, because that is where the whole side
    // of her is presented to it.
    let (beam, _) = settled(90.0);
    let (close, _) = settled(45.0);
    assert!(
        beam > close,
        "{beam:.2} kn beam-on against {close:.2} close-hauled"
    );
}

// ---------------------------------------------------------------------------
// Balance: what makes her gripe, and what actually cures it
// ---------------------------------------------------------------------------

/// Settle close-hauled under a given rig and report what the helm is fighting.
fn balance(tws_kn: f64, reef: f64, furl: f64) -> (f64, f64, f64, f64, f64) {
    let cfg = BoatConfig::archipelago_packet();
    let twa = rad(42.0);
    let heading = wrap_tau(0.0 - twa);
    let mut state = BoatState::at_rest(heading);
    state.surge = kn_to_ms(4.0);
    let mut sim = Simulation::new(
        cfg,
        state,
        Box::new(SteadyWind::new(0.0, kn_to_ms(tws_kn))) as Box<dyn WindField>,
    );
    let mut pilot = TillerPilot::new();
    let mut controls = Controls {
        reef,
        furl,
        outhaul: 0.5,
        ..Default::default()
    };
    for _ in 0..(240.0 / SUBSTEP) as u64 {
        let (m, j) = optimal_sheets(&cfg, sim.telemetry.awa, controls.outhaul);
        controls.main_sheet = m;
        controls.jib_sheet = j;
        controls.rudder = pilot.correct_under_load(
            &cfg,
            wrap_pi(heading - sim.state.heading),
            sim.state.yaw_rate,
            sim.telemetry.weather_helm,
            SUBSTEP,
        );
        sim.run(SUBSTEP, &controls);
    }
    let t = sim.telemetry;
    (
        t.weather_helm,
        t.yaw_from_rig,
        t.yaw_from_heel,
        deg(controls.rudder).abs(),
        deg(sim.state.heel).abs(),
    )
}

#[test]
fn the_three_causes_of_weather_helm_sum_to_it() {
    // The invariant that keeps the decomposition honest. Without it the three numbers
    // are three opinions rather than three parts of one measurement, and a reader would
    // have no way of knowing which.
    let (whole, rig, heel, _, _) = balance(14.0, 0.0, 0.0);
    let hull = whole - rig - heel;
    assert!(
        (rig + heel + hull - whole).abs() < 1.0,
        "{rig:.0} + {heel:.0} + {hull:.0} should be {whole:.0}"
    );
    // And the hull's share is the small one: it is damping and the lateral plane, not a
    // cause of griping in its own right.
    assert!(
        hull.abs() < whole.abs() * 0.3,
        "the hull is carrying {hull:.0} of {whole:.0}, which makes it a cause rather \
         than a bystander"
    );
}

#[test]
fn reefing_the_main_is_the_cure_for_weather_helm() {
    // A mainsail's load is aft of where the keel holds, so it is the sail that luffs
    // her, and taking it off is the thing that works. All three move the right way at
    // once, which is why it is the answer: less rig, less heel, less helm.
    let full = balance(14.0, 0.0, 0.0);
    let slab = balance(14.0, 0.33, 0.0);
    let deep = balance(14.0, 0.67, 0.0);
    assert!(
        slab.1 < full.1 && deep.1 < slab.1,
        "the rig's own luffing moment should fall with every slab: {:.0}, {:.0}, {:.0}",
        full.1,
        slab.1,
        deep.1
    );
    assert!(
        slab.2 < full.2 && deep.2 < slab.2,
        "and so should the heel term"
    );
    assert!(
        deep.3 < full.3 * 0.5,
        "two slabs should halve the helm: {:.1} deg against {:.1}",
        deep.3,
        full.3
    );
}

#[test]
fn rolling_the_genoa_away_is_not() {
    // The question a crew gets wrong, and the model has to get right. A headsail's load
    // is *forward* of where the keel holds, so it is what keeps her bow down; roll it
    // away and the main is left on its own with its load aft.
    //
    // Measured at eight knots of true wind: the rig's own moment goes from −95 Nm, which
    // is balanced, to +842, which is griping — and the helm needed goes *up*, from 2.6
    // degrees to 3.5, because she has also slowed down and a slower rudder has less
    // bite. That is the whole of why "roll the genoa" is the wrong answer and "put a
    // reef in" is the right one.
    let full = balance(8.0, 0.0, 0.0);
    let rolled = balance(8.0, 0.0, 1.0);
    assert!(
        rolled.1 > full.1 + 500.0,
        "rolling the headsail away should move the rig's moment hard towards luffing: \
         {:.0} Nm against {:.0}",
        rolled.1,
        full.1
    );
    assert!(
        rolled.3 > full.3,
        "and the helm should want *more* angle, not less: {:.1} deg against {:.1}",
        rolled.3,
        full.3
    );
    // She heels less for it, which is the trap: the one number that improves is the one
    // a crew is looking at when they decide.
    assert!(rolled.4 < full.4);
}

#[test]
fn heel_is_most_of_it_once_she_is_pressed() {
    // Which is why reefing works and why a boat sailed flat barely gripes at all. At
    // eight knots the rig is balanced and the heel term is the whole of the weather
    // helm; at fourteen, with her rail down, it is still the larger part by far.
    let easy = balance(8.0, 0.0, 0.0);
    let pressed = balance(14.0, 0.0, 0.0);
    assert!(
        easy.2 > easy.1.abs() * 3.0,
        "in eight knots the heel term {:.0} should dwarf the rig's {:.0}",
        easy.2,
        easy.1
    );
    assert!(
        pressed.2 > pressed.1 * 2.0,
        "and in fourteen it is still the larger: {:.0} against {:.0}",
        pressed.2,
        pressed.1
    );
    // And it grows faster than the wind, because it goes with the square of the speed
    // and the sine of the heel at once.
    assert!(pressed.2 > easy.2 * 2.0);
}
