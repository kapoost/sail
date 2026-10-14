//! Not knowing where you are, which is the condition navigation exists to answer.
//!
//! A chart that always shows your position is a plotter, and a plotter is a solved
//! problem: you look at it and you are there. Everything that makes terrestrial
//! navigation worth doing comes from the other state — the one where the boat is sailing
//! well, the coast is in sight, and nobody aboard can say which bay that is.
//!
//! So the game has both. On a delivery she knows where she is, because she left a
//! harbour an hour ago and has been keeping a plot. **Shift and L puts her somewhere at
//! sea with her sails set and no idea where that is**, which is the exercise: take
//! bearings on what you can see, cross them, and find out.
//!
//! The position comes back when it is earned and goes again when it goes stale, because
//! a fix is a statement about a moment. An hour later it is a story about where you
//! were.

use bevy::prelude::*;
use glam::DVec2;
use sail_core::autopilot::optimal_sheets_with;
use sail_core::units::{kn_to_ms, ms_to_kn, rad};

use crate::sim::Sim;

/// How long a fix is worth looking at, seconds of simulated time.
///
/// Six minutes. A fix is a statement about a moment and it ages at the speed the boat
/// moves: at five knots six minutes is half a mile, which is about the error a careful
/// plot has in it anyway. Longer than that and the chart is showing you where you were,
/// which is the one lie a chart must not tell.
pub const FIX_HOLDS: f64 = 360.0;

#[derive(Resource, Default)]
pub struct Navigation {
    /// Whether she is on her own wits. False on a delivery, true after the exercise is
    /// started and until a fix is good enough to clear it.
    pub lost: bool,
    /// When the last good fix was taken, in simulation seconds.
    pub last_fix: f64,
    /// And where it put her, which is what the chart draws rather than the truth: a fix
    /// is the navigator's answer and the chart carries the answer, right or wrong.
    pub fixed_at: Option<DVec2>,
    /// The bearings taken and not yet thrown away. Three is a fix; four is a navigator
    /// who has stopped sailing the boat.
    pub sights: Vec<sail_core::fix::Sight>,
}

impl Navigation {
    /// Whether the chart may show where she is.
    pub fn shows_position(&self, now: f64) -> bool {
        !self.lost || now - self.last_fix < FIX_HOLDS
    }

    /// How much of the fix's life is left, for drawing something that empties.
    pub fn fix_left(&self, now: f64) -> f32 {
        if !self.lost {
            return 1.0;
        }
        (1.0 - ((now - self.last_fix) / FIX_HOLDS)).clamp(0.0, 1.0) as f32
    }
}

/// Somewhere at sea, sails set, position unknown.
///
/// What this is *not* is a teleport with a scrambled chart. She arrives sailing: the
/// sheets are where a trimmer would have them for the wind she is in, the right headsail
/// is up, the main is reefed if the breeze wants it, and she is doing the speed that trim
/// gives. The exercise is navigation and nothing else, so nothing else is made difficult.
pub fn lost_at_sea(
    keys: Res<ButtonInput<KeyCode>>,
    mut sim: ResMut<Sim>,
    mut nav: ResMut<Navigation>,
    mut track: ResMut<crate::chart::Track>,
    mut pilot: ResMut<crate::input::Pilot>,
) {
    let asked = keys.just_pressed(KeyCode::KeyL)
        && (keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight));
    if !asked {
        return;
    }
    let Some(land) = sim.sim.land.clone() else {
        return;
    };

    // Somewhere in the archipelago with water under her and sea room round her — and in
    // sight of something, because an exercise with nothing to take a bearing on is not
    // an exercise, it is a blank page.
    let mut rng =
        sail_core::rng::Pcg32::new((sim.sim.state.time * 1_000.0) as u64 ^ 0x9E37_79B9_7F4A_7C15);
    let marks = land.landmarks();
    let mut here = sim.sim.state.pos;
    for _ in 0..400 {
        let candidate = DVec2::new(
            (rng.next_f64() * 2.0 - 1.0) * 7_000.0,
            (rng.next_f64() * 2.0 - 1.0) * 7_000.0,
        );
        // Deep water and a quarter of a mile of room, which is somewhere a boat would
        // actually be left sailing.
        if land.signed_distance(candidate) < 450.0 {
            continue;
        }
        // And at least three marks in sight, or there is no fix to be had.
        let in_sight = marks
            .iter()
            .filter(|m| (m.at - candidate).length() < m.visible_from(2.0).min(9_000.0))
            .count();
        if in_sight >= 3 {
            here = candidate;
            break;
        }
    }

    // A heading she could be on, and the sails set for it.
    let heading = rng.next_f64() * std::f64::consts::TAU;
    let wind = sim.sim.wind.sample(here, sim.sim.state.time);
    let tws = ms_to_kn(wind.length());
    let mut state = sail_core::state::BoatState::at_rest(heading);
    state.pos = here;
    state.time = sim.sim.state.time;
    // Moving, because a boat found drifting is a different puzzle. Two thirds of hull
    // speed is what a well-trimmed boat does in most of what blows.
    state.surge = kn_to_ms(4.6);
    sim.sim.state = state;

    // Everything a trimmer would have done, in the order they would have done it.
    let controls = &mut sim.controls;
    controls.headsail = if tws > 22.0 {
        // Too much wind for the big genoa: she carries it rolled.
        sail_core::state::Headsail::Genoa
    } else {
        sail_core::state::Headsail::Genoa
    };
    controls.furl = if tws > 22.0 { 0.35 } else { 0.0 };
    // Reefed by the breeze rather than by a solve: above sixteen knots a boat this size
    // has a slab in, above twenty-two she has two, and that is a rule of thumb that has
    // put more reefs in than any calculation.
    controls.reef = if tws > 22.0 {
        0.55
    } else if tws > 16.0 {
        0.3
    } else {
        0.0
    };
    // Flat in a breeze, full in a drifter: the outhaul and the vang are how a mainsail's
    // depth is set and the breeze decides which.
    controls.outhaul = ((tws - 8.0) / 12.0).clamp(0.15, 0.95);
    controls.vang = ((tws - 10.0) / 14.0).clamp(0.1, 0.8);
    controls.genoa_car = 0.45;
    controls.staysail = 0.0;
    controls.headsail_aback = false;
    controls.preventer = false;
    controls.rudder = 0.0;
    // The sheets want an apparent wind angle and the apparent wind wants her moving, so
    // she is run for a moment before they are set: a substep of the real engine rather
    // than an apparent wind worked out here, which would be the same arithmetic written
    // twice and would be wrong the day the engine's changes.
    let settle = sim.controls;
    sim.sim.run(sail_core::sim::SUBSTEP, &settle);
    let (main, jib) = optimal_sheets_with(
        &sim.config(),
        sim.sim.telemetry.awa,
        sim.controls.outhaul,
        sim.controls.headsail,
    );
    sim.controls.main_sheet = main;
    sim.controls.jib_sheet = jib;

    // And she does not know where she is. The log starts here: a track drawn from before
    // the exercise is a line pointing straight at the answer.
    nav.lost = true;
    nav.last_fix = f64::NEG_INFINITY;
    nav.fixed_at = None;
    nav.sights.clear();
    track.clear();
    pilot.disengage();

    info!(
        "somewhere at sea: {:.0} kn of wind, {} marks within nine miles",
        tws,
        marks
            .iter()
            .filter(|m| (m.at - here).length() < 9_000.0)
            .count()
    );
    let _ = rad(0.0);
}

/// Writing the fix in the log, and finding out whether it was worth writing.
///
/// This is where the exercise is scored and the scoring is the honest one: a fix is good
/// if it is *near where she actually is*. Not whether the triangle was small — a tight
/// cocked hat on two misidentified marks is a confident lie, and it is the classic way to
/// put a boat on a rock. The sea does not grade the method, it grades the answer.
///
/// A quarter of a mile. That is the error a careful fix off a small boat's hand bearing
/// compass actually has, and it is close enough to pilot by in open water and nothing
/// like close enough to go between two rocks on.
pub fn enter_in_the_log(
    sim: Res<Sim>,
    mut nav: ResMut<Navigation>,
    mut chart: ResMut<crate::chart::Chart>,
) {
    let Some(said) = chart.accepted.take() else {
        return;
    };
    const GOOD_ENOUGH: f64 = 460.0;
    let truth = sim.sim.state.pos;
    let out_by = (said - truth).length();
    nav.sights.clear();
    if out_by < GOOD_ENOUGH {
        nav.last_fix = sim.sim.state.time;
        nav.fixed_at = Some(said);
        info!(
            "fix entered: {:.0} m from where she is — the chart knows her again for six \
             minutes",
            out_by
        );
    } else {
        // Not good enough, and the chart says nothing more than it did: being wrong
        // about where you are is not an error message, it is the condition.
        nav.fixed_at = None;
        info!("fix entered: {:.0} m out — she is still lost", out_by);
    }
}
