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
use crate::units::{rad, smoothstep, wrap_pi, wrap_tau};
use crate::wind::{direction_of, WindField};
use glam::DVec2;

/// Sheet positions that put both sails at their best angle of attack for the given
/// apparent wind angle.
pub fn optimal_sheets(cfg: &BoatConfig, awa: f64, outhaul: f64) -> (f64, f64) {
    optimal_sheets_with(cfg, awa, outhaul, crate::state::Headsail::Genoa)
}

/// The same, for whichever sail is set forward of the mast.
///
/// A Code 0 and a gennaker have their own chord-angle limits, so the sheet position
/// that puts them at their best angle of attack is not the genoa's. Trimming a gennaker
/// to a genoa's numbers sheets it far too hard and chokes it.
pub fn optimal_sheets_with(
    cfg: &BoatConfig,
    awa: f64,
    outhaul: f64,
    head: crate::state::Headsail,
) -> (f64, f64) {
    optimal_sheets_bent(cfg, awa, outhaul, head, 0.0, 0.0)
}

/// The same, told how the two sails are bending each other's wind.
///
/// This is not a refinement, it is the difference between trimming and guessing. The
/// headsail's downwash takes several degrees off the angle the mainsail meets, so a main
/// trimmed to the masthead fly is trimmed for a wind that is not there — eased too far,
/// every time, by exactly the amount of the bend. Hand the trimmer the bent angle and it
/// sheets the main in near the centreline the way a crew does, for the reason a crew
/// does it.
pub fn optimal_sheets_bent(
    cfg: &BoatConfig,
    awa: f64,
    outhaul: f64,
    head: crate::state::Headsail,
    bend_main: f64,
    bend_jib: f64,
) -> (f64, f64) {
    let bent = |by: f64| awa.signum() * (awa.abs() + by).max(0.0);
    (
        cfg.main
            .profile
            .optimal_sheet(bent(bend_main), crate::sail::camber_for_outhaul(outhaul)),
        cfg.headsail(head)
            .profile
            .optimal_sheet(bent(bend_jib), crate::sail::CAMBER_REFERENCE),
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
    let (kp, _, kd) = cfg.pilot_gains();
    (kp * error - kd * st.yaw_rate).clamp(-cfg.max_rudder_angle, cfg.max_rudder_angle)
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
    /// Where the ram actually has the helm, which is not always where the loop asked
    /// for it: the rudder has to be swung there, and swinging takes time and force.
    last: f64,
    /// How long the output has been hard against the stops.
    ///
    /// A rudder on the stops is not steering, it is braking — a stalled foil with
    /// enormous drag, holding the boat too slow to answer it, which is a state that
    /// sustains itself. Every real pilot has an off-course alarm for exactly this,
    /// and the right response is to hand the helm back rather than keep pulling.
    saturated_for: f64,
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
        self.correct_under_load(cfg, error, yaw_rate, 0.0, dt)
    }

    /// The same loop, against a boat that is fighting it.
    ///
    /// A tiller pilot is a small electric ram with a motor in it. On this size of boat
    /// it pushes sixty kilogrammes or so, and that is the whole of what it has — there
    /// is no crew on the end of it leaning back. So it has two limits a hand does not:
    /// it can only push so hard, and it can only move so fast, and both get worse
    /// together because a loaded motor turns slower.
    ///
    /// Which is the thing an overcanvassed boat does to an autopilot, and it does it
    /// long before anything breaks. Measured on this boat: an easy beat loads the helm
    /// with about three and a half kilonewton-metres, being properly pressed with
    /// twelve to sixteen, and a beam reach in twenty knots under full sail with
    /// twenty-five — which is eighteen degrees of standing helm, and no tiller pilot
    /// alive holds that. It does not fail cleanly either. First it holds the course
    /// with the rudder further and further over, then it stops being able to put the
    /// rudder where it wants, then it cannot swing it fast enough to catch her, and
    /// then she is away and the ram is still pushing.
    ///
    /// The right answer to all of that is to reef. That is the point.
    pub fn correct_under_load(
        &mut self,
        cfg: &BoatConfig,
        error: f64,
        yaw_rate: f64,
        load: f64,
        dt: f64,
    ) -> f64 {
        /// Helm load the ram handles without noticing, newton-metres.
        const EASY: f64 = 9_000.0;
        /// And the load at which it has nothing left.
        const OVERWHELMED: f64 = 24_000.0;
        /// How fast the ram swings the helm with nothing against it, rad/s.
        const RAM_RATE: f64 = 0.55;

        let capacity = 1.0 - smoothstep(EASY, OVERWHELMED, load.abs());
        let (kp, ki, kd) = cfg.pilot_gains();
        let raw = kp * error + ki * self.integral - kd * yaw_rate;
        // What it can hold, and what it can hold it at.
        let reach = cfg.max_rudder_angle * (0.28 + 0.72 * capacity);
        let clamped = raw.clamp(-reach, reach);
        // And how fast it gets there: a loaded motor is a slow motor.
        let step = RAM_RATE * (0.35 + 0.65 * capacity) * dt;
        let clamped = if (clamped - self.last).abs() <= step {
            clamped
        } else {
            self.last + step * (clamped - self.last).signum()
        };
        self.last = clamped;
        // Anti-windup: stop integrating once the rudder is on the stops, otherwise the
        // integral runs away while the boat is overpowered and cannot answer anyway.
        if (raw - clamped).abs() < 1e-6 {
            self.integral += error * dt;
            self.integral = self.integral.clamp(-6.0, 6.0);
            self.saturated_for = 0.0;
        } else {
            self.saturated_for += dt;
        }
        clamped
    }

    /// Seconds the rudder has been hard over without the boat answering.
    pub fn saturated_for(&self) -> f64 {
        self.saturated_for
    }

    /// Forget the accumulated correction. Call on engaging, or the pilot starts by
    /// applying whatever helm the last course needed.
    pub fn reset(&mut self) {
        self.integral = 0.0;
        self.saturated_for = 0.0;
        self.last = 0.0;
    }

    /// Current standing rudder angle the pilot is holding, radians. On a well balanced
    /// boat this is the weather helm, expressed as something you can put on a gauge.
    pub fn standing_helm(&self, cfg: &BoatConfig) -> f64 {
        cfg.pilot_gains().1 * self.integral
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
    steady_state_with(
        cfg,
        make_wind,
        twa,
        seconds,
        reef,
        crate::state::Headsail::Genoa,
    )
}

/// The same, with a chosen headsail.
pub fn steady_state_with<F>(
    cfg: BoatConfig,
    make_wind: F,
    twa: f64,
    seconds: f64,
    reef: f64,
    head: crate::state::Headsail,
) -> Steady
where
    F: Fn() -> Box<dyn WindField>,
{
    steady_state_rigged(cfg, make_wind, twa, seconds, reef, head, 0.0)
}

/// The same again, with the staysail set or handed.
///
/// Headsail furl follows the reef here, which is how a boat is shortened down: you put
/// a slab in and roll a bit of genoa away together, and the rig stays balanced. For a
/// *deliberately* unbalanced rig — the main handed with full genoa, or the other way
/// about, which is a real question and not a mistake — use
/// [`steady_state_sailplan`].
pub fn steady_state_rigged<F>(
    cfg: BoatConfig,
    make_wind: F,
    twa: f64,
    seconds: f64,
    reef: f64,
    head: crate::state::Headsail,
    staysail: f64,
) -> Steady
where
    F: Fn() -> Box<dyn WindField>,
{
    // Half, not five sixths. A crew shortening down takes the mainsail in first and
    // rolls the headsail as little as it can get away with, because rolling a genoa is
    // not reefing it: the flat cloth goes first and what is left is a bag that heels her
    // without driving her. See `sail::camber_rolled`. The old coupling put forty-two per
    // cent of the roller in for one slab in the main, which is a shape no one carries on
    // purpose, and that only stopped being free the moment the model knew what a rolled
    // sail looks like.
    let furl = (reef * 0.5).clamp(0.0, 1.0);
    steady_state_sailplan(cfg, make_wind, twa, seconds, reef, furl, head, staysail)
}

/// The same again, with the headsail's lead car set as well.
///
/// Because the car is how a headsail's depth is trimmed, and depth is what decides
/// whether a sail alone can be sheeted close — so a question about sailing under
/// headsail alone that cannot set the car is not asking the whole question.
#[allow(clippy::too_many_arguments)]
pub fn steady_state_trimmed<F>(
    cfg: BoatConfig,
    make_wind: F,
    twa: f64,
    seconds: f64,
    reef: f64,
    furl: f64,
    head: crate::state::Headsail,
    staysail: f64,
    car: f64,
) -> Steady
where
    F: Fn() -> Box<dyn WindField>,
{
    steady_state_full(
        cfg, make_wind, twa, seconds, reef, furl, head, staysail, car,
    )
}

/// The same, with the reef and the furl set independently.
///
/// Every other entry point ties them together because that is how shortening down
/// works. This one exists because "can she go to windward under headsail alone" is a
/// question with a real answer, and it cannot even be *asked* through an interface where
/// handing the main rolls the genoa away with it.
#[allow(clippy::too_many_arguments)]
pub fn steady_state_sailplan<F>(
    cfg: BoatConfig,
    make_wind: F,
    twa: f64,
    seconds: f64,
    reef: f64,
    furl: f64,
    head: crate::state::Headsail,
    staysail: f64,
) -> Steady
where
    F: Fn() -> Box<dyn WindField>,
{
    steady_state_full(
        cfg, make_wind, twa, seconds, reef, furl, head, staysail, 0.0,
    )
}

#[allow(clippy::too_many_arguments)]
fn steady_state_full<F>(
    cfg: BoatConfig,
    make_wind: F,
    twa: f64,
    seconds: f64,
    reef: f64,
    furl: f64,
    head: crate::state::Headsail,
    staysail: f64,
    car: f64,
) -> Steady
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
        furl,
        genoa_car: car,
        headsail: head,
        staysail,
        ..Default::default()
    };
    let mut pilot = TillerPilot::new();
    let steps = (seconds / SUBSTEP).round() as u64;
    // Sample the speed a few seconds before the end to detect "still accelerating".
    let settle_check = steps.saturating_sub((4.0 / SUBSTEP) as u64);
    let mut speed_before = 0.0;

    for i in 0..steps {
        // On going upwind, off coming down, and further on once the boat is
        // overpowered — a slow outer loop, because the outhaul is a cleated line and
        // not something anyone plays with. Rate limited so it settles rather than
        // chasing the heel it is itself changing.
        let want = crate::sail::outhaul_for(sim.telemetry.awa, sim.state.heel, OVERPOWERED_HEEL);
        let step = (want - controls.outhaul).clamp(-0.25 * SUBSTEP, 0.25 * SUBSTEP);
        controls.outhaul = (controls.outhaul + step).clamp(0.0, 1.0);

        // The inner sail has a sheet of its own now, so a crew that trims everything
        // else and leaves it where it was is not a crew. Its own best angle, for the
        // wind at its own height.
        controls.staysail_sheet = cfg
            .staysail
            .profile
            .optimal_sheet(sim.telemetry.awa, crate::sail::CAMBER_REFERENCE);
        let (m, j) = optimal_sheets_bent(
            &cfg,
            sim.telemetry.awa,
            controls.outhaul,
            head,
            sim.telemetry.bend_main,
            sim.telemetry.bend_jib,
        );
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
    reef_for_heel_with(
        cfg,
        make_wind,
        twa,
        seconds,
        max_heel,
        crate::state::Headsail::Genoa,
    )
}

/// The same, with a chosen headsail.
pub fn reef_for_heel_with<F>(
    cfg: BoatConfig,
    make_wind: &F,
    twa: f64,
    seconds: f64,
    max_heel: f64,
    head: crate::state::Headsail,
) -> (f64, Steady)
where
    F: Fn() -> Box<dyn WindField>,
{
    let mut best = steady_state_with(cfg, make_wind, twa, seconds, 0.0, head);
    if best.state.heel.abs() <= max_heel {
        return (0.0, best);
    }
    let mut chosen = 0.0;
    // Seven rungs, not eight, and the eighth is the whole point of this comment.
    //
    // `reef` at 1.0 does not mean the deepest reef: past `boat::DROPPED` it means the
    // main is *off her*, which is a different decision entirely. Reefing is how you hold
    // a boat on her feet while she keeps sailing; handing the main is what you do to
    // heave to, or to run home under headsail alone. Letting a heel target choose between
    // them means that in any wind where the deepest reef is not enough — twenty-five
    // knots on a beat, which is a normal bad afternoon — the rule takes the mainsail off
    // and the boat stops being able to point at all. Measured in the polar: TWA 35 to 50
    // in 25 knots came out at 2.2 knots with thirty degrees of leeway and the helm hard
    // over, which is not a slow boat, it is a boat lying there.
    //
    // So the ladder stops at the last real reef. Past that she is simply overpowered, the
    // heel target is not met, and the honest answer is the one a crew gets: you are
    // carrying too much sail for this and there is nothing left to take in.
    for step in 1..=7 {
        let reef = step as f64 / 8.0;
        let trial = steady_state_with(cfg, make_wind, twa, seconds, reef, head);
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

    /// True once the pilot has held the rudder on the stops long enough that it is
    /// plainly not going to make the course.
    ///
    /// Six seconds, because a tack or a wave knock legitimately needs full helm for a
    /// second or two and alarming on that would be useless. Past that it is no longer
    /// steering: the rudder is stalled, it is dragging hard enough to keep the boat
    /// too slow to answer, and the state feeds itself. Handing the helm back is both
    /// what a real pilot does and the only way out.
    pub fn struggling(&self) -> bool {
        self.saturated_seconds() > 6.0
    }

    /// Whether this is a thing that *can* let go.
    ///
    /// The electric pilot is a ram with a controller on it, and when it has been on the
    /// stops for six seconds the controller knows it is braking rather than steering and
    /// drops out — with an alarm, because you are probably below.
    ///
    /// The vane gear has nothing to let go *with*. It is a blade, a linkage and the
    /// water: no controller, no relay, nothing to decide anything. Overpowered, it does
    /// not disengage — it simply stops coping. The vane lies over on its stop, the boat
    /// wanders further and further off the wind she was set to, and in the end she tacks
    /// or gybes herself and the gear goes on patiently steering the new course. That is
    /// the failure mode, and it is a different one to be caught out by.
    pub fn can_let_go(&self) -> bool {
        !matches!(self.mode, PilotMode::ApparentWind(_))
    }

    /// How long the rudder has been hard over, for diagnostics.
    pub fn saturated_seconds(&self) -> f64 {
        self.loop_.saturated_for()
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
                // The apparent wind angle is a *circular* quantity, and this used to
                // subtract it like a straight line. Which is a tree of special cases
                // waiting to happen, and one of them was a trap: dead downwind the wind
                // wanders either side of the stern with every wave, so a target of 175
                // degrees to starboard and a reading of 179 to port is a four-degree
                // shift — and the old arithmetic read it as being knocked onto the other
                // tack and answered with the helm hard over towards the gybe. Measured
                // before the fix: forty degrees of rudder, which is the stop, for four
                // degrees of wind. At eight knots that is how an autopilot puts you on
                // your ear.
                //
                // Wrapping the difference is the whole of the fix, and the special cases
                // fall out of it rather than having to be written:
                //
                // * Four degrees across the stern is four degrees of error, not a tack.
                // * Properly gybed — the wind on the other quarter — the short way round
                //   the circle *is* round the stern, so she bears away. Which is what
                //   you want, and what the old code had to say explicitly.
                // * Knocked onto the wrong tack upwind, the short way round is through
                //   the wind, so she luffs and tacks back. Also what you want, and the
                //   one the old code warned that getting backwards sends her the long
                //   way round out of control.
                // * And in irons, where the sign of the measurement means nothing, the
                //   short way round is onto the side the target is on, which is the
                //   same answer the old head-to-wind branch forced by hand.
                //
                // One subtraction and a wrap, and it is right everywhere the tree was
                // right and right where the tree was dangerous.
                wrap_pi(tm.awa - target)
            }
        };
        // The load the ram is working against, which is everything the rig and the
        // hull are doing to turn her, with the rudder's own contribution taken out.
        Some(
            self.loop_
                .correct_under_load(cfg, error, st.yaw_rate, tm.weather_helm, dt),
        )
    }
}
