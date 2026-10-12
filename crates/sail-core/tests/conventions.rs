//! Sign and frame conventions.
//!
//! Separate from the performance tests on purpose. A sign error in a moment arm is a
//! day of debugging, and it presents as "the physics feels wrong" rather than as a
//! failure. These tests make it present as a failure.

use glam::DVec2;
use sail_core::prelude::*;
use sail_core::wind::{direction_of, flow_from, shear_factor, WindField};

const EPS: f64 = 1e-9;

#[test]
fn heading_zero_points_north() {
    let st = BoatState::at_rest(0.0);
    assert!((st.forward() - DVec2::new(0.0, 1.0)).length() < EPS);
    assert!((st.starboard() - DVec2::new(1.0, 0.0)).length() < EPS);
}

#[test]
fn heading_ninety_points_east() {
    let st = BoatState::at_rest(rad(90.0));
    assert!((st.forward() - DVec2::new(1.0, 0.0)).length() < 1e-12);
    assert!((st.starboard() - DVec2::new(0.0, -1.0)).length() < 1e-12);
}

#[test]
fn body_transform_is_its_own_inverse() {
    let st = BoatState::at_rest(rad(37.0));
    let world = DVec2::new(3.0, -7.0);
    let round_trip = st.to_body(st.to_body(world));
    assert!((round_trip - world).length() < 1e-12);
}

#[test]
fn wind_direction_is_where_it_comes_from() {
    // A westerly blows towards the east.
    let flow = flow_from(rad(270.0), 10.0);
    assert!((flow.x - 10.0).abs() < 1e-9, "flow {flow:?}");
    assert!(flow.y.abs() < 1e-9);
    // And round-trips.
    assert!((direction_of(flow) - rad(270.0)).abs() < 1e-9);

    // A northerly blows towards the south.
    let flow = flow_from(0.0, 10.0);
    assert!(flow.x.abs() < 1e-9);
    assert!((flow.y + 10.0).abs() < 1e-9, "flow {flow:?}");
}

#[test]
fn apparent_wind_angle_is_positive_from_starboard() {
    let cfg = BoatConfig::charter_sloop_34();

    // Bow north, wind from the east: it must read as coming from starboard.
    let wind = SteadyWind::new(rad(90.0), kn_to_ms(10.0));
    let mut sim = Simulation::new(cfg, BoatState::at_rest(0.0), Box::new(wind));
    sim.run(SUBSTEP * 2.0, &Controls::default());
    assert!(
        sim.telemetry.awa > 0.0,
        "wind from the east on a northerly heading must be on the starboard side, got {} deg",
        deg(sim.telemetry.awa)
    );

    // Wind from the west: port side.
    let wind = SteadyWind::new(rad(270.0), kn_to_ms(10.0));
    let mut sim = Simulation::new(cfg, BoatState::at_rest(0.0), Box::new(wind));
    sim.run(SUBSTEP * 2.0, &Controls::default());
    assert!(
        sim.telemetry.awa < 0.0,
        "wind from the west must be on the port side, got {} deg",
        deg(sim.telemetry.awa)
    );
}

#[test]
fn head_to_wind_reads_zero_and_dead_run_reads_one_eighty() {
    let cfg = BoatConfig::charter_sloop_34();

    let wind = SteadyWind::new(0.0, kn_to_ms(10.0));
    let mut sim = Simulation::new(cfg, BoatState::at_rest(0.0), Box::new(wind));
    sim.run(SUBSTEP * 2.0, &Controls::default());
    assert!(
        sim.telemetry.awa.abs() < rad(0.5),
        "{} deg",
        deg(sim.telemetry.awa)
    );

    let mut sim = Simulation::new(
        cfg,
        BoatState::at_rest(rad(180.0)),
        Box::new(SteadyWind::new(0.0, kn_to_ms(10.0))),
    );
    sim.run(SUBSTEP * 2.0, &Controls::default());
    assert!(
        sim.telemetry.awa.abs() > rad(179.0),
        "{} deg",
        deg(sim.telemetry.awa)
    );
}

#[test]
fn wind_on_starboard_heels_the_boat_to_port() {
    let cfg = BoatConfig::charter_sloop_34();
    let make = || -> Box<dyn WindField> { Box::new(SteadyWind::new(0.0, kn_to_ms(14.0))) };

    // Measured sailing, not from a standstill: sheeted hard in with no way on the boat
    // simply goes into irons and bears away, which is correct but says nothing about
    // which way it heels.
    let starboard = steady_state(cfg, make, rad(45.0), 300.0, 0.0);
    assert!(
        starboard.telemetry.awa > 0.0,
        "positive true wind angle is starboard tack, awa {} deg",
        deg(starboard.telemetry.awa)
    );
    assert!(
        starboard.state.heel < rad(-5.0),
        "starboard tack must heel to port (negative), got {} deg",
        deg(starboard.state.heel)
    );

    let port = steady_state(cfg, make, rad(-45.0), 300.0, 0.0);
    assert!(
        port.telemetry.awa < 0.0,
        "awa {} deg",
        deg(port.telemetry.awa)
    );
    assert!(
        port.state.heel > rad(5.0),
        "port tack must heel to starboard (positive), got {} deg",
        deg(port.state.heel)
    );
    // Mirror symmetry: the two tacks must be the same boat.
    assert!(
        (starboard.state.heel + port.state.heel).abs() < rad(1.0),
        "tacks are not mirror images: {} vs {} deg",
        deg(starboard.state.heel),
        deg(port.state.heel)
    );
}

#[test]
fn positive_rudder_turns_the_bow_to_starboard() {
    let cfg = BoatConfig::charter_sloop_34();
    // Carrying way with no wind, so the rudder has flow over it but the rig is not
    // steering. This used to motor; the boat has no engine now, and starting her with
    // speed on is the better test anyway — it isolates the rudder from everything,
    // including a propeller.
    let wind = SteadyWind::new(0.0, 0.0);
    let mut state = BoatState::at_rest(0.0);
    state.surge = sail_core::units::kn_to_ms(4.5);
    let mut sim = Simulation::new(cfg, state, Box::new(wind));
    let ahead = Controls::default();
    sim.run(4.0, &ahead);
    let start = sim.state.heading;

    let turning = Controls {
        rudder: rad(10.0),
        ..ahead
    };
    sim.run(10.0, &turning);
    assert!(
        wrap_pi(sim.state.heading - start) > rad(1.0),
        "positive rudder must swing the bow to starboard, moved {} deg",
        deg(wrap_pi(sim.state.heading - start))
    );
    assert!(sim.state.yaw_rate > 0.0);
}

#[test]
fn leeway_is_positive_when_slipping_to_starboard() {
    let mut st = BoatState::at_rest(0.0);
    st.surge = 3.0;
    st.sway = 0.3;
    assert!(st.leeway() > 0.0);
    st.sway = -0.3;
    assert!(st.leeway() < 0.0);
}

#[test]
fn wind_shear_reduces_speed_below_reference_height() {
    assert!((shear_factor(10.0) - 1.0).abs() < 1e-12);
    assert!(shear_factor(6.0) < 1.0);
    assert!(shear_factor(15.0) > 1.0);
    // The effect is a few percent, not a factor.
    assert!(shear_factor(6.0) > 0.9);
}

#[test]
fn righting_arm_opposes_heel() {
    use sail_core::hull::righting_arm;
    let gm = 1.15;
    let peak = rad(60.0);
    assert!(righting_arm(rad(20.0), gm, peak) > 0.0);
    assert!(righting_arm(rad(-20.0), gm, peak) < 0.0);
    assert!((righting_arm(0.0, gm, peak)).abs() < 1e-12);
    // Monotonic up to the peak.
    assert!(righting_arm(rad(40.0), gm, peak) > righting_arm(rad(20.0), gm, peak));
    // And it gives way beyond it.
    assert!(righting_arm(rad(85.0), gm, peak) < righting_arm(rad(60.0), gm, peak));
}
