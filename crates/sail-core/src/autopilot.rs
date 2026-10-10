//! Trim reference, heading autopilot, and the steady-state solver.
//!
//! Not cheats. `optimal_sheets` is the line a good trimmer is chasing, so it is both
//! the yardstick the validation suite measures against and the basis of an in-game
//! assist or crew behaviour. `heading_rudder` is a tiller pilot — real equipment on a
//! small cruiser. `reef_for_heel` is the rule of thumb every sailor uses: reduce sail
//! until the boat stands up.

use crate::boat::BoatConfig;
use crate::sim::{Simulation, SUBSTEP};
use crate::state::{BoatState, Controls, Telemetry};
use crate::units::{rad, wrap_pi, wrap_tau};
use crate::wind::{direction_of, WindField};
use glam::DVec2;

/// Sheet positions that put both sails at their best angle of attack for the given
/// apparent wind angle.
pub fn optimal_sheets(cfg: &BoatConfig, awa: f64, outhaul: f64) -> (f64, f64) {
    (
        cfg.main
            .profile
            .optimal_sheet(awa, crate::sail::camber_for_outhaul(outhaul)),
        cfg.genoa
            .profile
            .optimal_sheet(awa, crate::sail::CAMBER_REFERENCE),
    )
}

/// Stateless proportional-derivative rudder. Fine for a nudge, wrong for holding a
/// course: weather helm is a constant disturbance, and a PD controller cannot drive
/// the error to zero against one. Use [`TillerPilot`] to actually steer.
///
/// The derivative term is doing real work regardless — a keelboat has enough yaw
/// inertia that a pure proportional pilot hunts badly.
pub fn heading_rudder(cfg: &BoatConfig, st: &BoatState, target_heading: f64) -> f64 {
    let error = wrap_pi(target_heading - st.heading);
    (2.2 * error - 4.0 * st.yaw_rate).clamp(-cfg.max_rudder_angle, cfg.max_rudder_angle)
}

/// A tiller pilot: PID with anti-windup.
///
/// The integral term is not a refinement, it is the whole point. A boat carrying three
/// degrees of weather helm presents a constant yaw moment; without integral action the
/// pilot settles at a permanent heading error, which upwind reads as a boat that will
/// not hold its course.
#[derive(Clone, Debug, Default)]
pub struct TillerPilot {
    integral: f64,
}

impl TillerPilot {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn rudder(
        &mut self,
        cfg: &BoatConfig,
        st: &BoatState,
        target_heading: f64,
        dt: f64,
    ) -> f64 {
        self.correct(cfg, wrap_pi(target_heading - st.heading), st.yaw_rate, dt)
    }

    /// The PID itself, against whatever error you hand it. Separate from
    /// [`rudder`](Self::rudder) so a wind-angle pilot can drive the same loop — the
    /// control problem is identical, only the thing being measured differs.
    ///
    /// `error` is positive when the bow needs to come to starboard.
    pub fn correct(&mut self, cfg: &BoatConfig, error: f64, yaw_rate: f64, dt: f64) -> f64 {
        let raw = 2.2 * error + 0.9 * self.integral - 4.0 * yaw_rate;
        let clamped = raw.clamp(-cfg.max_rudder_angle, cfg.max_rudder_angle);
        // Anti-windup: stop integrating once the rudder is on the stops, otherwise the
        // integral runs away while the boat is overpowered and cannot answer anyway.
        if (raw - clamped).abs() < 1e-9 {
            self.integral += error * dt;
            self.integral = self.integral.clamp(-6.0, 6.0);
        }
        clamped
    }

    /// Forget the accumulated correction. Call on engaging, or the pilot starts by
    /// applying whatever helm the last course needed.
    pub fn reset(&mut self) {
        self.integral = 0.0;
    }

    /// Current standing rudder angle the pilot is holding, radians. On a well balanced
    /// boat this is the weather helm, expressed as something you can put on a gauge.
    pub fn standing_helm(&self) -> f64 {
        0.9 * self.integral
    }
}

/// Outcome of a steady-state solve.
pub struct Steady {
    pub state: BoatState,
    pub telemetry: Telemetry,
    pub controls: Controls,
    /// How far off the commanded heading the boat ended up, radians.
    pub heading_error: f64,
    /// False when the boat could not be held on course or was still accelerating.
    /// An unconverged point is not a slow point — it is a point with no answer, and
    /// reporting it as a speed would be a quiet lie.
    pub converged: bool,
}

/// Sail to steady state on a given true wind angle with optimal trim.
///
/// `twa` is signed: positive puts the wind on the starboard side, i.e. starboard tack.
/// `make_wind` is a factory so the caller can re-solve with a different reef.
pub fn steady_state<F>(cfg: BoatConfig, make_wind: F, twa: f64, seconds: f64, reef: f64) -> Steady
where
    F: Fn() -> Box<dyn WindField>,
{
    let wind = make_wind();
    let sample = wind.sample(DVec2::ZERO, 0.0);
    let wind_direction = direction_of(sample);
    let heading = wrap_tau(wind_direction - twa);

    // Start with way on. A real boat close-hauled from a standstill has to bear away
    // and build speed first; starting at rest makes the solver converge on the
    // in-irons attractor, which is true but useless as a polar. The standstill
    // transient is covered by its own test.
    let mut initial = BoatState::at_rest(heading);
    initial.surge = 0.6 * cfg.hull_speed();
    let mut sim = Simulation::new(cfg, initial, wind);

    let mut controls = Controls {
        // Start full and let the heel decide, below.
        outhaul: 0.0,
        reef,
        furl: (reef * 0.85).clamp(0.0, 1.0),
        ..Default::default()
    };
    let mut pilot = TillerPilot::new();
    let steps = (seconds / SUBSTEP).round() as u64;
    // Sample the speed a few seconds before the end to detect "still accelerating".
    let settle_check = steps.saturating_sub((4.0 / SUBSTEP) as u64);
    let mut speed_before = 0.0;

    for i in 0..steps {
        // Flatten only once the boat is overpowered — a slow outer loop, because the
        // outhaul is a cleated line and not something anyone plays with. Rate limited
        // so it settles rather than chasing the heel it is itself changing.
        let want = crate::sail::outhaul_for_heel(sim.state.heel, OVERPOWERED_HEEL);
        let step = (want - controls.outhaul).clamp(-0.25 * SUBSTEP, 0.25 * SUBSTEP);
        controls.outhaul = (controls.outhaul + step).clamp(0.0, 1.0);

        let (m, j) = optimal_sheets(&cfg, sim.telemetry.awa, controls.outhaul);
        controls.main_sheet = m;
        controls.jib_sheet = j;
        controls.rudder = pilot.rudder(&cfg, &sim.state, heading, SUBSTEP);
        sim.run(SUBSTEP, &controls);
        if i == settle_check {
            speed_before = sim.telemetry.stw;
        }
    }

    let heading_error = wrap_pi(sim.state.heading - heading);
    let drifting = (sim.telemetry.stw - speed_before).abs() > 0.02;
    Steady {
        state: sim.state,
        telemetry: sim.telemetry,
        controls,
        heading_error,
        converged: heading_error.abs() < rad(3.0) && !drifting,
    }
}

/// Smallest reef that keeps heel within `max_heel`.
///
/// This is how the boat is actually sailed, so it is how the polar should be measured.
/// Returns the reef setting and the resulting steady state.
pub fn reef_for_heel<F>(
    cfg: BoatConfig,
    make_wind: &F,
    twa: f64,
    seconds: f64,
    max_heel: f64,
) -> (f64, Steady)
where
    F: Fn() -> Box<dyn WindField>,
{
    let mut best = steady_state(cfg, make_wind, twa, seconds, 0.0);
    if best.state.heel.abs() <= max_heel {
        return (0.0, best);
    }
    let mut chosen = 0.0;
    for step in 1..=8 {
        let reef = step as f64 / 8.0;
        let trial = steady_state(cfg, make_wind, twa, seconds, reef);
        chosen = reef;
        let within = trial.state.heel.abs() <= max_heel;
        best = trial;
        if within {
            break;
        }
    }
    (chosen, best)
}

/// Heel past which a cruising crew considers itself overpowered and starts taking
/// power out of the sail.
pub const OVERPOWERED_HEEL: f64 = 20.0 * std::f64::consts::PI / 180.0;

/// Closest to the wind a vane will be asked to hold. Below this the boat is pinching
/// and the sails cannot be kept drawing.
pub const VANE_MIN_ANGLE: f64 = 20.0 * std::f64::consts::PI / 180.0;
/// Furthest off the wind a vane will hold, short of dead downwind where the angle's
/// sign — and therefore the tack — stops meaning anything.
pub const VANE_MAX_ANGLE: f64 = 178.0 * std::f64::consts::PI / 180.0;

/// What the autopilot is holding.
///
/// Two modes because there are two pieces of real gear, and they are not
/// interchangeable. An electric tiller pilot holds a compass course: good for motoring
/// and for a passage leg, useless upwind, because a wind shift leaves the sails wrong.
/// A wind vane holds an angle to the apparent wind: it keeps the sails at a constant
/// angle of attack and follows every shift, which is what you want on a beat and what
/// makes it the right way to sleep on a long passage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PilotMode {
    /// Hand steering.
    Standby,
    /// Hold a compass heading, radians from north.
    Heading(f64),
    /// Hold a signed apparent wind angle, radians, positive with the wind to
    /// starboard. The sign carries the tack.
    ApparentWind(f64),
}

/// A tiller pilot with a wind vane in the same box.
#[derive(Clone, Debug)]
pub struct Autopilot {
    pub mode: PilotMode,
    loop_: TillerPilot,
}

impl Default for Autopilot {
    fn default() -> Self {
        Self {
            mode: PilotMode::Standby,
            loop_: TillerPilot::new(),
        }
    }
}

impl Autopilot {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn engaged(&self) -> bool {
        self.mode != PilotMode::Standby
    }

    /// Hold the course the boat is on right now.
    pub fn hold_heading(&mut self, st: &BoatState) {
        self.mode = PilotMode::Heading(st.heading);
        self.loop_.reset();
    }

    /// Hold the wind angle the boat is on right now.
    ///
    /// Clamped to the same band [`adjust`](Self::adjust) works in. Engaging while
    /// pinching at fifteen degrees would otherwise store a target below the floor,
    /// and the next "closer to the wind" press would push it *up* to the floor —
    /// moving the target away from the wind, which is the opposite of what was asked.
    pub fn hold_wind(&mut self, tm: &Telemetry) {
        let sign = if tm.awa >= 0.0 { 1.0 } else { -1.0 };
        let magnitude = tm.awa.abs().clamp(VANE_MIN_ANGLE, VANE_MAX_ANGLE);
        self.mode = PilotMode::ApparentWind(sign * magnitude);
        self.loop_.reset();
    }

    pub fn disengage(&mut self) {
        self.mode = PilotMode::Standby;
        self.loop_.reset();
    }

    /// Nudge the target. Positive luffs in wind mode and turns to starboard in heading
    /// mode — the two are not the same thing, so the pilot does the thinking.
    pub fn adjust(&mut self, delta: f64) {
        match &mut self.mode {
            PilotMode::Standby => {}
            PilotMode::Heading(h) => *h = wrap_tau(*h + delta),
            PilotMode::ApparentWind(a) => {
                // Luffing means a smaller angle, on either tack.
                let sign = if *a >= 0.0 { 1.0 } else { -1.0 };
                let magnitude = (a.abs() - delta).clamp(VANE_MIN_ANGLE, VANE_MAX_ANGLE);
                *a = sign * magnitude;
            }
        }
    }

    /// Rudder angle, or `None` in standby.
    pub fn rudder(
        &mut self,
        cfg: &BoatConfig,
        st: &BoatState,
        tm: &Telemetry,
        dt: f64,
    ) -> Option<f64> {
        let error = match self.mode {
            PilotMode::Standby => return None,
            PilotMode::Heading(target) => wrap_pi(target - st.heading),
            PilotMode::ApparentWind(target) => {
                // Bearing away opens the apparent wind angle, so when the measured
                // angle exceeds the target the boat must luff — a turn towards the
                // wind, and therefore towards whichever side the wind is on. On the
                // intended tack both tacks reduce to the same expression.
                let upwind_target = target.abs() < std::f64::consts::FRAC_PI_2;
                if tm.awa.abs() < rad(8.0) {
                    // Head to wind, where the sign of the measurement is meaningless.
                    // Bear away onto the intended tack.
                    -target.signum() * rad(20.0)
                } else if tm.awa.signum() != target.signum() {
                    // Knocked onto the wrong tack. Which way round matters enormously:
                    // for an upwind target the short way back is a tack, so luff up and
                    // through the wind; for a downwind target it is a gybe, so bear
                    // away. Getting this backwards sends the boat the long way round
                    // and, upwind, straight through an uncontrolled gybe.
                    let through_the_wind = if upwind_target { -1.0 } else { 1.0 };
                    through_the_wind * target.signum() * rad(30.0)
                } else {
                    tm.awa - target
                }
            }
        };
        Some(self.loop_.correct(cfg, error, st.yaw_rate, dt))
    }
}
