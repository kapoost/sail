//! A boat at anchor, measured — the whole of it through the hull rather than in a
//! formula of its own.
//!
//! **Anchoring was a circle on the chart you sat inside until a number went up.** What
//! is in the model now is a pull at the stemhead, and what came out of running it is
//! everything a boat at anchor does, none of which is written down anywhere:
//!
//! * **She lies head to the wind** — because the pull is at the bow, and a force a
//!   hull-length forward of the centre of gravity is a yaw moment as well as a force.
//!   Nothing says *point her at the wind*; the lever says it.
//! * **She rides to the stream when the stream is stronger than the wind**, and lies
//!   across them both when they disagree, which is the thing that catches everybody out
//!   in a tidal anchorage.
//! * **She stops.** Under sail in a fresh breeze with the anchor down she comes up to
//!   her chain and stays there, which is what letting go actually does to a boat.
//! * **And she drags when the ground will not hold her**, slowly, downwind, which is a
//!   plough rather than a release.

use sail_core::anchor::{Anchor, Down, Ground};
use sail_core::boat::BoatConfig;
use sail_core::sim::{Simulation, SUBSTEP};
use sail_core::state::{BoatState, Controls};
use sail_core::units::{deg, kn_to_ms, ms_to_kn, rad, wrap_pi};
use sail_core::wind::SteadyWind;

/// Let go where she is, in this ground, with this much chain.
fn let_go(sim: &mut Simulation, ground: Ground, out: f64, depth: f64) {
    let bow = sim.state.pos
        + glam::DVec2::new(sim.state.heading.sin(), sim.state.heading.cos())
            * sim.config.form.lwl
            * 0.5;
    sim.anchor = Anchor::Down(Down {
        at: bow,
        depth,
        out,
        dug: ground.sets_in(),
        ground,
        dragged: 0.0,
    });
}

/// Sail her for a while with everything handed, which is what a boat at anchor is.
fn ride(sim: &mut Simulation, minutes: f64) {
    let controls = Controls {
        reef: 1.0,
        furl: 1.0,
        ..Default::default()
    };
    for _ in 0..(minutes * 60.0 / SUBSTEP) as usize {
        sim.run(SUBSTEP, &controls);
    }
}

fn riding_in(wind_from: f64, knots: f64) -> Simulation {
    let cfg = BoatConfig::archipelago_packet();
    // Lying beam on to start with, so that swinging head to wind is something that has
    // to happen rather than something she was set up in.
    let state = BoatState::at_rest(wrap_pi(wind_from + rad(90.0)));
    let wind = SteadyWind::new(wind_from, kn_to_ms(knots));
    Simulation::new(cfg, state, Box::new(wind))
}

/// **She lies head to the wind, and nothing in the model says so.**
///
/// The pull is at the stemhead and a force forward of the centre of gravity is a moment:
/// put her beam on to a fresh breeze with the anchor down and she rounds up to it by
/// herself. That is the test that says the chain is in the physics rather than beside
/// it.
#[test]
fn she_swings_head_to_wind() {
    let from = rad(30.0);
    let mut sim = riding_in(from, 18.0);
    let_go(&mut sim, Ground::Sand, 30.0, 6.0);
    ride(&mut sim, 6.0);
    let off = deg(wrap_pi(sim.state.heading - from)).abs();
    assert!(
        off < 25.0,
        "she lay {off:.0} degrees off the wind at anchor"
    );
    // And she is not sailing anywhere.
    assert!(
        ms_to_kn(sim.telemetry.sog) < 0.6,
        "she was making {:.1} knots at anchor",
        ms_to_kn(sim.telemetry.sog)
    );
}

/// **And she stays where she was put**, within the length of her own chain — which is
/// what *holding* means and is the only number anybody at anchor cares about.
#[test]
fn a_well_set_anchor_holds_her() {
    let mut sim = riding_in(rad(200.0), 22.0);
    let started = sim.state.pos;
    let_go(&mut sim, Ground::Sand, 30.0, 6.0);
    ride(&mut sim, 20.0);
    let moved = (sim.state.pos - started).length();
    assert!(
        moved < 40.0,
        "she ended {moved:.0} m from where she let go in twenty minutes"
    );
    let Anchor::Down(down) = sim.anchor else {
        panic!("the anchor came up by itself")
    };
    assert!(
        down.dragged < 6.0,
        "the anchor walked {:.1} m",
        down.dragged
    );
}

/// **Short scope in weed does not hold**, and the boat goes downwind of where she went
/// to sleep. Same wind, same boat, a different decision when she let go.
#[test]
fn short_scope_in_weed_drags() {
    let mut sim = riding_in(rad(200.0), 26.0);
    let started = sim.state.pos;
    let_go(&mut sim, Ground::Weed, 11.0, 5.0);
    ride(&mut sim, 20.0);
    let moved = (sim.state.pos - started).length();
    let Anchor::Down(down) = sim.anchor else {
        panic!("the anchor came up by itself")
    };
    assert!(
        down.dragged > 10.0,
        "a scope of two in weed held twenty-six knots for twenty minutes"
    );
    assert!(
        moved > 20.0,
        "the anchor walked {:.0} m and the boat did not follow it",
        down.dragged
    );
    // Downwind, which is the direction a dragging boat goes and the reason you wake up
    // in a different bay.
    let downwind = glam::DVec2::new(rad(200.0).sin(), rad(200.0).cos());
    assert!(
        (sim.state.pos - started).normalize().dot(-downwind) > 0.7,
        "she dragged somewhere other than downwind"
    );
}

/// **The stream is the other thing she lies to.** In a tidal anchorage with a light
/// breeze across it, a boat lies to the tide and not to the wind — which is the single
/// most surprising thing about anchoring for anybody who has only done it in a lake, and
/// it needs nothing in the model but the stream the keel is already in.
#[test]
fn she_rides_to_the_tide_when_the_tide_is_the_stronger() {
    let from = rad(0.0);
    let mut sim = riding_in(from, 6.0);
    // A stream running hard to the east, in water shallow enough to make it a stream.
    sim.tide = sail_core::tide::Tide::of(11, 4);
    sim.tide.put_at(0.0, sim.state.time);
    let stream = sim.tide.stream(sim.state.time, 12.0);
    assert!(
        ms_to_kn(stream.length()) > 1.0,
        "the test's own tide is slack at {:.1} kn",
        ms_to_kn(stream.length())
    );
    let_go(&mut sim, Ground::Sand, 36.0, 8.0);
    ride(&mut sim, 12.0);
    // Her head is up-stream — into the water going past her, which is where a boat
    // riding to a tide points.
    let heading = glam::DVec2::new(sim.state.heading.sin(), sim.state.heading.cos());
    let into_it = -stream.normalize();
    assert!(
        heading.dot(into_it) > 0.5,
        "she lay {:.0} degrees off the stream in a six-knot breeze",
        deg(heading.angle_to(into_it)).abs()
    );
}
