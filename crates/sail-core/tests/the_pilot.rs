//! What the compass pilot actually does, measured rather than argued about.
//!
//! Reported from the deck as "it goes off course more often than it should lately, and I
//! cannot tell why it sometimes catches it and sometimes does not" — which is the exact
//! shape of a fault that needs a bench rather than a theory. An autopilot that holds in
//! one condition and not in another is not random; it is a loop meeting a limit, and
//! there are only three limits it can meet:
//!
//! * the loop is **asking for the wrong thing** — tuning, and it would be wrong always;
//! * the loop is asking for **more helm than the ram can hold** against the load, which
//!   is a boat carrying too much sail and is the pilot telling you to reef;
//! * the loop is asking for helm **faster than the ram can swing it**, which is a
//!   seaway, and is a different thing entirely.
//!
//! From the deck all three look the same: the boat wanders off. In the rudder trace all
//! three look the same too. `Effort` separates them, and this file runs the pilot across
//! a grid of real conditions and prints which one it is meeting, so the question is
//! answered with a table instead of a guess.
//!
//! Run it with the numbers showing:
//!
//! ```text
//! cargo test -p sail-core --test the_pilot -- --nocapture
//! ```

use sail_core::autopilot::{optimal_sheets_with, Autopilot};
use sail_core::boat::BoatConfig;
use sail_core::sim::{Simulation, SUBSTEP};
use sail_core::state::{BoatState, Controls, Headsail};
use sail_core::units::{deg, kn_to_ms, ms_to_kn, rad, wrap_pi, wrap_tau};
use sail_core::water::WindSea;
use sail_core::wind::{SteadyWind, SyntheticWind, WindField};

/// One run on one course in one set of conditions.
struct Held {
    /// Root mean square of the heading error, degrees. What "she wanders" means as a
    /// number: a tenth of a degree is a plotter, two degrees is a good pilot in a sea,
    /// ten degrees is a boat sailing herself.
    rms: f64,
    /// And the worst single moment of it.
    worst: f64,
    /// The standing helm she needed, degrees — the weather helm, as the pilot sees it.
    helm: f64,
    /// How much of the time the ram had not the strength: the loop wanted more helm
    /// than it can hold against the load.
    on_the_stops: f64,
    /// How much of the time it had not the speed: swinging as fast as it can and still
    /// behind the ask.
    too_slow: f64,
    /// The longest the pilot thought itself beaten — six seconds of this and the real
    /// one drops out with an alarm.
    worst_struggle: f64,
    /// Whether it dropped out at all, and how far in.
    gave_up: Option<f64>,
    /// And how fast she was going, because a pilot that holds the course by stopping
    /// the boat has not held anything.
    knots: f64,
    /// The longest unbroken run of the ram being merely *behind* the ask — swinging
    /// flat out, nowhere near the stops. Kept apart from the other two because it is
    /// the one that is not a fault in the boat.
    worst_behind: f64,
}

/// What the sea is doing under her.
#[derive(Clone, Copy, PartialEq)]
enum Sea {
    /// Flat water: a river mouth, or a lee shore at dawn.
    Flat,
    /// Fetch-limited wind sea, kilometres of fetch.
    Fetch(f64),
}

/// Sail one course with the pilot engaged and watch it work.
///
/// She is settled first and measured afterwards, because the first half minute is the
/// loop winding its integral up to the weather helm and has nothing to do with whether
/// it holds a course.
fn hold(twa_deg: f64, tws_kn: f64, sea: Sea, gusty: bool, reef: f64) -> Held {
    hold_at(twa_deg, tws_kn, sea, gusty, reef, 1.0 / SUBSTEP)
}

/// The same, with the pilot's loop running at a chosen rate.
///
/// Which is not a detail. In the game the loop is stepped **once a frame** while the
/// boat is integrated two hundred times a second, so the pilot on the water is running
/// at whatever the picture is managing — sixty on a good machine, and a good deal less
/// with weather on the screen. A controller's damping is a function of its rate, and a
/// rate that changes with the scenery is a pilot that behaves differently in different
/// places for no reason anybody on deck can see. That is exactly the complaint this
/// file exists to chase: *sometimes it holds and sometimes it does not*.
fn hold_at(twa_deg: f64, tws_kn: f64, sea: Sea, gusty: bool, reef: f64, pilot_hz: f64) -> Held {
    let cfg = BoatConfig::charter_sloop_34();
    // Wind from the north, so a compass course is also an angle to the wind.
    let course = wrap_tau(rad(twa_deg));
    let mut state = BoatState::at_rest(course);
    state.surge = kn_to_ms(4.0);
    let wind: Box<dyn WindField> = if gusty {
        Box::new(SyntheticWind::new(0.0, kn_to_ms(tws_kn), 7))
    } else {
        Box::new(SteadyWind::new(0.0, kn_to_ms(tws_kn)))
    };
    let mut sim = Simulation::new(cfg, state, wind);
    if let Sea::Fetch(km) = sea {
        sim = sim.with_water(Box::new(WindSea::new(kn_to_ms(tws_kn), km * 1_000.0, 0.0, 11)));
    }
    let mut controls = Controls {
        outhaul: 0.5,
        reef,
        headsail: Headsail::Genoa,
        furl: reef * 0.6,
        ..Default::default()
    };
    let mut pilot = Autopilot::new();

    const SETTLE: f64 = 90.0;
    const MEASURE: f64 = 240.0;
    let steps = ((SETTLE + MEASURE) / SUBSTEP) as u64;
    let mut sum_sq = 0.0;
    let mut worst = 0.0_f64;
    let mut helm = 0.0;
    let mut knots = 0.0;
    let mut on_the_stops = 0.0;
    let mut too_slow = 0.0;
    let mut worst_struggle = 0.0_f64;
    let mut gave_up = None;
    let mut counted = 0.0;
    let mut t = 0.0;
    let mut behind_for = 0.0_f64;
    let mut worst_behind = 0.0_f64;
    // The pilot's own clock: it is asked for helm every so often, and between times the
    // helm stays where it was put.
    let every = ((1.0 / pilot_hz) / SUBSTEP).round().max(1.0) as u64;
    let pilot_dt = every as f64 * SUBSTEP;

    for step in 0..steps {
        let (m, j) =
            optimal_sheets_with(&cfg, sim.telemetry.awa, controls.outhaul, controls.headsail);
        controls.main_sheet = m;
        controls.jib_sheet = j;
        // Engaged once she is moving, the way it is engaged on the water: you get her
        // sailing, you settle on the course, and then you press the button.
        if step == (20.0 / SUBSTEP) as u64 {
            pilot.hold_heading(&sim.state);
        }
        if step % every == 0 {
            if let Some(r) = pilot.rudder(&cfg, &sim.state, &sim.telemetry, pilot_dt) {
                controls.rudder = r;
            }
            let effort = pilot.effort();
            if effort.too_slow() && !effort.on_the_stops() {
                behind_for += pilot_dt;
                worst_behind = worst_behind.max(behind_for);
            } else {
                behind_for = 0.0;
            }
        }
        sim.run(SUBSTEP, &controls);
        t += SUBSTEP;

        // The real one lets go after six seconds on the stops, and the game hands the
        // helm back when it does. Measured, not asserted, because it is the behaviour
        // under test.
        worst_struggle = worst_struggle.max(pilot.saturated_seconds());
        if gave_up.is_none() && pilot.struggling() && pilot.can_let_go() {
            gave_up = Some(t);
        }
        if t < SETTLE {
            continue;
        }
        let error = deg(wrap_pi(
            match pilot.mode {
                sail_core::autopilot::PilotMode::Heading(h) => h,
                _ => course,
            } - sim.state.heading,
        ));
        sum_sq += error * error;
        worst = worst.max(error.abs());
        helm += deg(controls.rudder);
        knots += ms_to_kn(sim.telemetry.stw);
        let effort = pilot.effort();
        if effort.on_the_stops() {
            on_the_stops += 1.0;
        }
        if effort.too_slow() {
            too_slow += 1.0;
        }
        counted += 1.0;
    }
    Held {
        rms: (sum_sq / counted).sqrt(),
        worst,
        helm: helm / counted,
        on_the_stops: on_the_stops / counted,
        too_slow: too_slow / counted,
        worst_struggle,
        gave_up,
        knots: knots / counted,
        worst_behind,
    }
}

/// The grid, printed. Not an assertion — a bench.
#[test]
fn what_the_pilot_meets_and_where() {
    println!(
        "\n{:<34} {:>6} {:>6} {:>6} {:>7} {:>7} {:>7} {:>6}",
        "", "rms", "worst", "helm", "stops", "slow", "struggle", "kn"
    );
    let mut rows = 0;
    for &(tws, sea) in &[
        (8.0, Sea::Flat),
        (8.0, Sea::Fetch(20.0)),
        (14.0, Sea::Flat),
        (14.0, Sea::Fetch(20.0)),
        (20.0, Sea::Fetch(20.0)),
        (20.0, Sea::Fetch(80.0)),
    ] {
        for &twa in &[45.0, 70.0, 110.0, 150.0] {
            let reef = if tws >= 20.0 { 0.5 } else { 0.0 };
            let held = hold(twa, tws, sea, true, reef);
            let water = match sea {
                Sea::Flat => "flat".to_string(),
                Sea::Fetch(km) => format!("{km:.0} km fetch"),
            };
            println!(
                "{:<34} {:>6.1} {:>6.1} {:>6.1} {:>6.0}% {:>6.0}% {:>7.1} {:>6.1}{}",
                format!("{tws:.0} kn, {water}, TWA {twa:.0}"),
                held.rms,
                held.worst,
                held.helm,
                held.on_the_stops * 100.0,
                held.too_slow * 100.0,
                held.worst_struggle,
                held.knots,
                match held.gave_up {
                    Some(at) => format!("  GAVE UP at {at:.0} s"),
                    None => String::new(),
                }
            );
            rows += 1;
        }
    }
    assert!(rows > 0);
}

/// The same boat and the same sea, with only the pilot's loop rate changed.
///
/// Two hundred is the simulation's own step and is the pilot behaving perfectly. Sixty
/// is a good frame rate. Thirty is what the machine does with a squall on the screen,
/// and twenty is what it does with a squall on the screen and the chart spread.
#[test]
fn what_the_frame_rate_does_to_it() {
    println!(
        "\n{:<34} {:>6} {:>6} {:>6} {:>7} {:>7} {:>7} {:>8} {:>6}",
        "", "rms", "worst", "helm", "stops", "slow", "struggle", "behind", "kn"
    );
    for &(tws, sea, twa, reef) in &[
        (14.0, Sea::Fetch(20.0), 45.0, 0.0),
        (14.0, Sea::Fetch(20.0), 110.0, 0.0),
        (20.0, Sea::Fetch(20.0), 110.0, 0.5),
    ] {
        for &hz in &[200.0, 60.0, 30.0, 20.0] {
            let held = hold_at(twa, tws, sea, true, reef, hz);
            println!(
                "{:<34} {:>6.1} {:>6.1} {:>6.1} {:>6.0}% {:>6.0}% {:>7.1} {:>8.1} {:>6.1}{}",
                format!("TWA {twa:.0}, {tws:.0} kn, pilot at {hz:.0} Hz"),
                held.rms,
                held.worst,
                held.helm,
                held.on_the_stops * 100.0,
                held.too_slow * 100.0,
                held.worst_struggle,
                held.worst_behind,
                held.knots,
                match held.gave_up {
                    Some(at) => format!("  GAVE UP at {at:.0} s"),
                    None => String::new(),
                }
            );
        }
    }
}

/// **A pilot that is holding the course does not report itself beaten.**
///
/// This is the fault the bench was built to find, and it is the one that reads from the
/// deck as "it goes off course for no reason": at twenty knots on a beam reach the ram
/// sat on its limit four per cent of the time — which is honest, it is a pressed boat —
/// held the course to one degree RMS, and the game handed the helm back with an alarm
/// anyway. The old test for "beaten" was the rudder being at its limit, and a rudder at
/// its limit is a rudder doing its job.
///
/// So: in every condition where she is in fact on course, nothing lets go.
#[test]
fn it_does_not_give_up_while_it_is_holding_the_course() {
    for &(tws, sea, reef) in &[
        (8.0, Sea::Fetch(20.0), 0.0),
        (14.0, Sea::Fetch(20.0), 0.0),
        (20.0, Sea::Fetch(20.0), 0.5),
    ] {
        for &twa in &[45.0, 70.0, 110.0, 150.0] {
            let held = hold(twa, tws, sea, true, reef);
            if held.rms > 3.0 {
                // Not on course, so being beaten is allowed and is a separate question.
                continue;
            }
            assert!(
                held.gave_up.is_none(),
                "the pilot let go at {:.0} s on TWA {twa:.0} in {tws:.0} knots while \
                 holding the course to {:.1} degrees RMS",
                held.gave_up.unwrap_or_default(),
                held.rms
            );
            assert!(
                held.worst_struggle < 6.0,
                "the pilot called itself beaten for {:.1} s on TWA {twa:.0} in {tws:.0} \
                 knots while holding the course to {:.1} degrees RMS",
                held.worst_struggle,
                held.rms
            );
        }
    }
}

/// And a pilot that is genuinely beaten still says so.
///
/// The other half of the same change, because the cheap way to stop false alarms is to
/// stop alarming. Twenty-five knots on a beam reach under full sail is the case the
/// load model was built around — eighteen degrees of standing helm, which no tiller
/// pilot on a thirty-four footer holds — and the answer to it is to reef, which is
/// exactly what the pilot letting go is telling you to do.
#[test]
fn a_pilot_that_is_beaten_still_lets_go() {
    let held = hold(110.0, 25.0, Sea::Fetch(80.0), true, 0.0);
    assert!(
        held.worst_struggle > 6.0 || held.gave_up.is_some(),
        "overpowered on a beam reach and the pilot never complained: {:.1} degrees RMS, \
         worst {:.1}, on the stops {:.0}% of the time",
        held.rms,
        held.worst,
        held.on_the_stops * 100.0
    );
}
