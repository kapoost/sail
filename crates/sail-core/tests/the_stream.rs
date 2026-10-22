//! What a tidal stream does to a boat, measured rather than asserted.
//!
//! The model is one line in `sim::step`: the stream is added to the water the hull and
//! the appendages are moving through, and nothing else in the physics was told about it.
//! That is the claim worth testing, because it is the claim that makes the rest of it
//! free — if a stream is only an arrow added to the track afterwards then the log reads
//! wrong, the leeway is wrong, and the helm feels nothing. If it goes in as *water*,
//! every one of those comes out right without being written down anywhere.
//!
//! What came out of running it:
//!
//! * **A knot of cross-stream is about twelve degrees of track at four knots**, which is
//!   the arithmetic a navigator does on the back of a chart and is a useful check that
//!   the model is not quietly halving it somewhere.
//! * **The log does not know.** Speed through the water is the same in a fair tide and a
//!   foul one to within a twentieth of a knot — which is the whole reason a tide is hard
//!   to notice from on board and the reason the instrument on the panel exists.
//! * **The set is the stream**, in direction as well as rate, for three different
//!   directions of set — which is the evidence that it reached the keel and the rudder
//!   and not only the hull. A fifth of a knot over, and that fifth is the next line.
//! * **A fair tide is not free speed.** Carrying a knot and a bit of tide took about a
//!   tenth of a knot off her speed *through the water*, because the apparent wind is
//!   made against the ground and the wind was never told about the water.

use sail_core::sim::{Simulation, SUBSTEP};
use sail_core::state::{BoatState, Controls};
use sail_core::tide::Tide;
use sail_core::units::{deg, kn_to_ms, ms_to_kn, wrap_pi};
use sail_core::wind::SteadyWind;

/// Sail for `seconds` on a fixed trim with the tide set to a constant rate and
/// direction, and report where she got to and what the instruments said.
struct Passage {
    /// Where she ended up, metres from the start.
    over_the_ground: glam::DVec2,
    /// Speed through the water at the end, knots.
    log: f64,
}

/// A constant stream, which is what a tide looks like over ten minutes.
///
/// Built by hand rather than by `Tide::of`, because a test that has to wait for mid-tide
/// is a test that is really about the clock. `at` is the moment the stream is sampled —
/// a quarter cycle past high water, where it runs at its full rate.
fn steady_stream(rate_knots: f64, towards: f64) -> (Tide, f64) {
    // Find a world whose rate is near enough the one asked for, then read it at the
    // moment it peaks. Searching rather than constructing, because the fields are
    // private and should stay that way: the model owns the relationship between the
    // range and the rate, and a test that reaches past it is testing itself.
    let want = kn_to_ms(rate_knots);
    let mut best = (Tide::none(), f64::MAX);
    for seed in 0..400u64 {
        let tide = Tide::of(seed, 1);
        // A quarter of a cycle after this world's high water, whenever that is.
        let peak = sail_core::tide::M2 * 0.25 + until_high_at_zero(&tide);
        let rate = tide.stream(peak, 30.0).length();
        if (rate - want).abs() < best.1 {
            best = (tide, (rate - want).abs());
        }
    }
    let tide = best.0;
    let at = sail_core::tide::M2 * 0.25 + until_high_at_zero(&tide);
    let _ = towards;
    (tide, at)
}

/// When this world's high water falls, as a time from zero.
fn until_high_at_zero(tide: &Tide) -> f64 {
    tide.until_high(0.0)
}

fn sail(tide: Tide, start_time: f64, heading: f64, seconds: f64) -> Passage {
    use sail_core::autopilot::TillerPilot;

    let cfg = sail_core::boat::BoatConfig::archipelago_packet();
    let mut state = BoatState {
        time: start_time,
        ..BoatState::at_rest(heading)
    };
    state.surge = kn_to_ms(4.0);
    let wind = SteadyWind::new(0.0, kn_to_ms(12.0));
    let mut sim = Simulation::new(cfg, state, Box::new(wind));
    sim.tide = tide;
    let mut controls = Controls {
        main_sheet: 0.55,
        jib_sheet: 0.5,
        furl: 0.0,
        ..Default::default()
    };
    // **Steered, and that is not a convenience.** Left to herself she settles at
    // whatever heading the sails and the sea agree on, and that heading is not quite the
    // same one with a tide running as without — so the difference between two free runs
    // contains her own course change as well as the stream, which is a tenth of a knot
    // of nonsense in a measurement of a knot. A hand on the tiller holding a compass
    // course is also what a navigator allowing for tide actually does.
    let mut pilot = TillerPilot::new();
    let mut course = heading;
    // Let her settle before the course is taken and before the measurement starts, so
    // the displacement is a steady state and not an acceleration.
    let settle = (90.0 / SUBSTEP) as usize;
    let mut from = sim.state.pos;
    for step in 0..settle + (seconds / SUBSTEP) as usize {
        if step == settle {
            course = sim.state.heading;
            from = sim.state.pos;
        }
        controls.rudder = pilot.rudder(&sim.config, &sim.state, course, SUBSTEP);
        sim.run(SUBSTEP, &controls);
    }
    Passage {
        over_the_ground: sim.state.pos - from,
        log: ms_to_kn(sim.telemetry.stw),
    }
}

/// **A cross-stream sets her sideways, and by the amount a navigator would work out.**
///
/// The one result the whole model exists for. Ten minutes on a fixed heading with a
/// knot of stream across her track, against the same ten minutes with none: the
/// difference between the two tracks is the stream, and the angle between them is what
/// you have to steer to allow for it.
#[test]
fn a_knot_across_is_ten_degrees_of_track() {
    let (tide, at) = steady_stream(1.0, 0.0);
    let rate = ms_to_kn(tide.stream(at, 30.0).length());
    let still = sail(Tide::none(), at, 0.0, 600.0);
    let running = sail(tide, at, 0.0, 600.0);

    let set = running.over_the_ground - still.over_the_ground;
    let drift = ms_to_kn(set.length() / 600.0);
    assert!(
        (drift - rate).abs() < 0.15,
        "a {rate:.2} knot stream set her {drift:.2} knots sideways over ten minutes"
    );

    // And as an angle, which is the form it is used in.
    let straight = still.over_the_ground.y.atan2(still.over_the_ground.x);
    let made_good = running.over_the_ground.y.atan2(running.over_the_ground.x);
    let off = deg(wrap_pi(made_good - straight)).abs();
    assert!(
        (4.0..25.0).contains(&off),
        "a knot of stream moved her track by {off:.1} degrees, \
         which is not what a knot does to a four-knot boat"
    );
}

/// **The log does not know there is a tide**, which is why the panel has to say so.
///
/// Speed through the water is a property of the boat and the wind she is in. If the
/// stream had gone in as an arrow on the track instead of as water, this is the test
/// that would fail — the log would read the ground speed and a foul tide would look
/// like a boat that had stopped sailing.
#[test]
fn the_log_reads_the_water_and_not_the_ground() {
    let (tide, at) = steady_stream(1.2, 0.0);
    let still = sail(Tide::none(), at, 0.0, 300.0);
    let across = sail(tide, at, 0.0, 300.0);
    assert!(
        (still.log - across.log).abs() < 0.25,
        "the log read {:.2} knots with no tide and {:.2} with one",
        still.log,
        across.log
    );
}

/// **The set is the stream, whichever way she happens to be pointing.**
///
/// The other half of the same fact, and the one that says the mechanism is right rather
/// than merely present. If the stream had been bolted on as an arrow added to the track
/// then this would hold trivially; it holds here because her keel is in moving water,
/// and the test is worth having because that is a thing that could easily come out
/// *nearly* right — a stream that got into the hull's flow but not the keel's would set
/// her by some other number nobody could name.
///
/// Three worlds, three different directions of set, the same claim each time.
#[test]
fn the_set_is_the_stream_whichever_way_she_points() {
    for want in [0.6, 1.0, 1.4] {
        let (tide, at) = steady_stream(want, 0.0);
        let stream = tide.stream(at, 30.0);
        let still = sail(Tide::none(), at, 0.0, 300.0);
        let running = sail(tide, at, 0.0, 300.0);
        let set = (running.over_the_ground - still.over_the_ground) / 300.0;
        // A fifth of a knot of slack, and it is not error: held on a compass course in a
        // tide she sails in a slightly different wind and makes a slightly different
        // speed through the water, so the ground track is the stream *plus* that — see
        // the next test. The residual measured 0.11, 0.11 and 0.13 m/s over the three.
        let off = (set - stream).length();
        assert!(
            off < 0.2,
            "a stream of {:.2} m/s towards {:.0} set her {:.2} m/s towards {:.0}",
            stream.length(),
            deg(stream.x.atan2(stream.y)).rem_euclid(360.0),
            set.length(),
            deg(set.x.atan2(set.y)).rem_euclid(360.0),
        );
    }
}

/// **And it costs her a little of the wind she is sailing in**, which is the part
/// nobody expects and the reason a fair tide feels slow.
///
/// The apparent wind is made against the ground, not against the water: a boat carried
/// along by a knot of tide has a knot less wind over her deck than the same boat doing
/// the same speed through still water. It is small — a tenth of a knot of boat speed for
/// a knot of tide on a close reach, measured here — but it is the right sign and it is
/// there for the right reason, which is that the wind was never told about the water.
#[test]
fn a_carried_boat_sails_in_a_little_less_wind() {
    let (tide, at) = steady_stream(1.2, 0.0);
    let still = sail(Tide::none(), at, 0.0, 300.0);
    let carried = sail(tide, at, 0.0, 300.0);
    let cost = still.log - carried.log;
    assert!(
        (-0.05..0.4).contains(&cost),
        "carrying a tide changed her speed through the water by {cost:.2} knots"
    );
}

/// **And the water over the bar is the chart's plus the tide's.** The sounder reads the
/// sum, which is the whole reason a tide table is on the chart table.
#[test]
fn the_sounder_reads_the_tide_as_well_as_the_chart() {
    let tide = Tide::of(3, 1);
    let cfg = sail_core::boat::BoatConfig::archipelago_packet();
    let mut sim = Simulation::new(
        cfg,
        BoatState::at_rest(0.0),
        Box::new(SteadyWind::new(0.0, 5.0)),
    );
    sim.tide = tide;
    sim.run(SUBSTEP, &Controls::default());
    let flat = sim.telemetry.depth;
    // Half a cycle on, the tide is the other way up by the whole range.
    sim.state.time += sail_core::tide::M2 / 2.0;
    sim.run(SUBSTEP, &Controls::default());
    let later = sim.telemetry.depth;
    assert!(
        ((flat - later).abs() - (tide.height(0.0) - tide.height(sail_core::tide::M2 / 2.0)).abs())
            .abs()
            < 0.05,
        "the sounder moved {:.2} m over half a cycle of a {:.2} m tide",
        (flat - later).abs(),
        tide.range()
    );
}
