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

/// How long a fix is worth looking at, seconds — **and this is the backstop rather than
/// the rule.**
///
/// What really keeps a position is being able to see the marks it was taken on: while
/// two of them are still there to be recognised you can re-fix whenever you like, so you
/// are not lost in any sense that matters. Asked for from the deck in exactly those
/// words, and it is better seamanship than a clock.
///
/// The clock is still here for the case the marks never go: a fix an hour old on marks
/// you have been watching the whole time is still an hour-old fix, and she has sailed
/// four miles since. Twenty minutes, after which you take another one.
pub const FIX_HOLDS: f64 = 1_200.0;

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
    /// The marks the last fix was taken on, and whether they can still be made out.
    /// While two of them can, the position holds.
    pub fixed_on: Vec<(glam::DVec2, sail_core::land::Mark)>,
    /// How many of them are in sight now, for the chart to say so.
    pub still_in_sight: usize,
    /// The marks she has *recognised* — worked out through the glasses by the shape of
    /// the coast rather than told to her. Until a mark is on this list it is a symbol on
    /// the chart with no name, which is what an unidentified mark is.
    pub recognised: Vec<String>,
    /// What the glasses last made of what they are looking at, for the player to read.
    pub reading: Option<String>,
}

impl Navigation {
    /// Whether the chart may show where she is.
    ///
    /// Two things, and the first is the real one: the marks the fix was taken on are
    /// still there to be recognised, so another fix is a minute's work whenever she
    /// wants one. The clock is only the backstop for a fix that would otherwise last all
    /// afternoon.
    pub fn shows_position(&self, now: f64) -> bool {
        if !self.lost {
            return true;
        }
        self.still_in_sight >= 2 && now - self.last_fix < FIX_HOLDS
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
    mut chart: ResMut<crate::chart::Chart>,
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
    nav.fixed_on.clear();
    nav.still_in_sight = 0;
    nav.recognised.clear();
    nav.reading = None;
    // And the paper stops following her. Laid down over the middle of the archipelago,
    // which tells her nothing she did not already have: it is where the chart is *of*
    // rather than where she is.
    chart.laid_at = Some(DVec2::ZERO);
    chart.plot.clear();
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
    coast: Res<crate::marks::Coast>,
    mut nav: ResMut<Navigation>,
    mut chart: ResMut<crate::chart::Chart>,
) {
    let Some(said) = chart.accepted.take() else {
        return;
    };
    let truth = sim.sim.state.pos;
    let out_by = (said - truth).length();
    // Which marks it was taken on, kept so that the position can hold for as long as
    // they can be made out.
    let on: Vec<(DVec2, sail_core::land::Mark)> = nav
        .sights
        .iter()
        .filter_map(|s| {
            coast
                .marks
                .iter()
                .find(|m| (m.at - s.mark).length() < 1.0)
                .map(|m| (m.at, m.kind))
        })
        .collect();
    // A quarter of a mile off two bearings, and a third more off three.
    //
    // Leaning the three-bearing case the player's way was asked for from the deck and it
    // is defensible rather than charity: three lines are more work, they carry their own
    // check in the cocked hat, and a navigator who has taken a third bearing *has* done
    // the thing the pilot book asks for. The reward for method is a little more rope.
    let good_enough = if on.len() >= 3 { 620.0 } else { 460.0 };
    nav.sights.clear();
    // The lines come off the chart the moment the fix is in the log, which is what a
    // navigator does with them: they were working, the fix is the answer, and leaving
    // them on is how a plot turns into a thicket nobody can read an hour later. What is
    // left behind is the fix itself, which is the one thing worth keeping.
    chart
        .plot
        .retain(|item| !matches!(item, crate::chart::Plotted::Line { .. }));
    if out_by < good_enough {
        nav.last_fix = sim.sim.state.time;
        nav.fixed_at = Some(said);
        nav.fixed_on = on.clone();
        // The paper is laid over the fix, which is where a navigator puts it: you work
        // where you are, and she sails across the page from there.
        chart.laid_at = Some(said);
        // And it goes on the chart where it was worked out, which is where a fix goes:
        // a cross, and the time beside it, so an hour later you can see what you were
        // working from.
        chart.plot.push(crate::chart::Plotted::Cross { at: said });
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

/// Whether the marks the fix was taken on can still be made out.
///
/// A position is not a thing that decays on a clock; it is a thing you can renew as long
/// as the coast is still telling you where you are. So this counts, every frame, how many
/// of the marks the last fix was worked from are still inside the range at which they
/// could be identified — and while two of them are, the chart goes on knowing where she
/// is, because another fix is a minute's work whenever she wants one.
///
/// When the second one goes — round a headland, into the rain, over the horizon — the
/// chart stops saying. That is the moment a navigator starts keeping a dead-reckoning
/// plot, and it arrives by itself rather than on a timer.
pub fn the_fix_holds(sim: Res<Sim>, coast: Res<crate::marks::Coast>, mut nav: ResMut<Navigation>) {
    if !nav.lost || nav.fixed_on.is_empty() {
        if !nav.lost {
            nav.still_in_sight = usize::MAX;
        }
        return;
    }
    let here = sim.sim.state.pos;
    // What the weather is doing to the view. Rain and the murk under a squall take the
    // coast away long before the curve of the earth does, and the one that takes it away
    // first is the one that decides.
    let murk = sim.sim.wind.squall_at(here).clamp(0.0, 1.0);
    let through_it = 1.0 - 0.75 * murk;
    // The same rule the compass uses, asked of the same marks: a fix lasts as long as
    // the marks it was taken on could be shot again, because that is what makes it worth
    // believing — another one is a minute's work whenever she wants it. Asked through
    // `sight::nameable_from` rather than written out a second time, which is how the two
    // came to disagree in the first place.
    nav.still_in_sight = nav
        .fixed_on
        .iter()
        .filter(|(at, kind)| {
            let range = (*at - here).length();
            let reach = match coast.marks.iter().find(|m| (m.at - *at).length() < 1.0) {
                Some(mark) => crate::sight::nameable_from(mark, &nav.recognised),
                // A mark that is no longer on the coast at all — a world rebuilt under
                // the fix — cannot be re-shot, and the old rule is the safe answer.
                None => kind.identified_from(),
            };
            range < reach * through_it
        })
        .count();
}
