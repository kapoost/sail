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

#[derive(Resource)]
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
    /// The reckoning: where she *ought* to be, run on from the last fix.
    pub reckoning: Option<Reckoning>,
    /// The fixes she has entered in the log, in order, with their times. Joined on the
    /// chart by a solid line, which is the one line on that paper that is not an
    /// estimate of anything — two positions she worked out and the water between them.
    pub fixes: Vec<(DVec2, f64)>,
    /// How far she has sailed through the water since the game began, metres.
    ///
    /// Which is what a ship's log counts, and the only distance a boat that does not
    /// know where she is can measure at all: a paddlewheel, an impeller or a line with a
    /// rotator on the end of it counts water going past, and it goes on counting whether
    /// or not anybody knows which water.
    pub through_water: f64,
    /// The last speed measured by hand off the log, and when — knots, and the clock.
    ///
    /// Kept apart from the instrument's own reading because it is a different kind of
    /// number: the instrument says what she is doing now, and this is what she *did*,
    /// averaged over a leg somebody chose to time. It is the figure a reckoning should
    /// be run at.
    pub measured: Option<(f64, f64)>,
    /// Metres a second of movement the reckoning does not account for, learned from how
    /// far out the last run was.
    ///
    /// **Which is the honest way to draw a circle of uncertainty.** A radius picked by
    /// the programmer is a decoration; a radius measured from your own last mistake is
    /// navigation. Every time a fix lands on top of a reckoning that was run from the
    /// one before it, the distance between them, divided by the time it took, is the
    /// set and the leeway and the steering error added together — and that number,
    /// multiplied by how long it has been since the last fix, is how big the circle is
    /// now. Start it at a third of a knot, which is a modest tidal stream, so the first
    /// circle of a passage is not nought.
    pub unexplained: f64,
    /// When she was last put somewhere with no idea where, and which of the cards was
    /// drawn for it. `None` once it has been read.
    pub adrift: Option<(f64, usize)>,
}

/// Where she ought to be, worked up from the last fix.
///
/// **This is dead reckoning and it is the half of navigation the fix cannot do.** A fix
/// tells you where you were at one moment; between fixes all you have is what you
/// steered and how fast you went, and laying that off from the last known position is
/// what a navigator does every half hour of every passage. The result is not where she
/// is — it is where she would be if the water had stood still and the helmsman had been
/// perfect — and the whole art is knowing how much to trust it, which is what the
/// circle round it is for.
#[derive(Clone, Copy, Debug)]
pub struct Reckoning {
    /// The fix it was started from, and when.
    pub from: DVec2,
    pub since: f64,
    /// Where the run has got to.
    pub at: DVec2,
    /// How far the last reckoning turned out to be wrong, for the circle. Metres.
    pub spread: f64,
}

/// The set and leeway a passage is assumed to have before it has measured any.
///
/// A third of a knot. Modest, real, and not nought: a circle of uncertainty that starts
/// at nothing tells a beginner that dead reckoning is exact, which is the one thing it
/// is not.
const ASSUMED_SET: f64 = 0.17;

/// How much of the old estimate survives a new measurement.
///
/// Three quarters, so four runs in and the number is the water's rather than the
/// assumption's. **Frequent fixes are worth more than careful ones** — that is the
/// whole of coastal navigation — and this is where the game says so: every fix both
/// resets the reckoning to a known point and sharpens the figure the circle grows at,
/// so a navigator who fixes every twenty minutes carries a small circle and one who
/// fixes at noon carries a huge one.
const KEEPS: f64 = 0.75;

/// Run the reckoning on, and grow the doubt.
///
/// Integrated from **the course steered and the speed through the water**, which is what
/// a navigator has: not the boat's true track over the ground, which is the answer and
/// would make the exercise a formality. The difference between the two is exactly the
/// thing the circle is about.
pub fn reckon(sim: Res<Sim>, mut nav: ResMut<Navigation>, time: Res<Time>) {
    let dt = time.delta_secs_f64();
    let now = sim.sim.state.time;
    let unexplained = nav.unexplained;
    // The log counts whatever happens. Here rather than in the reckoning below because
    // it goes on counting when there is no reckoning to run.
    nav.through_water += sim.sim.telemetry.stw.abs() * dt;
    let Some(run) = nav.reckoning.as_mut() else {
        return;
    };
    let st = &sim.sim.state;
    // Her head, and her speed through the water: the two numbers on the bulkhead.
    let along = DVec2::new(st.heading.sin(), st.heading.cos());
    run.at += along * sim.sim.telemetry.stw * dt;
    run.spread = unexplained * (now - run.since).max(0.0);
}

/// What the screen says when the world has just been taken away from underneath her.
///
/// **A joke, and it is doing a job.** Being dropped somewhere unknown is the one event
/// in this game that arrives with no warning and no explanation — the screen simply
/// changes — and the first thing a player needs is to be told that this was on purpose.
/// A straight sentence would do it and would read like an error message, which is the
/// one thing it must not; the drill is meant to be *enjoyable*, and a navigator who has
/// lost the plot is the oldest joke at sea and the one every sailor has been the subject
/// of.
///
/// So: the voice of the watch below, who is not sorry. The second line is the one that
/// is actually useful, which is the other half of the trick — nobody reads instructions
/// and everybody reads a punchline.
const CARDS: [(&str, &str); 6] = [
    (
        "WHERE ARE WE",
        "the chart is correct. the chart is always correct.",
    ),
    (
        "GOOD MORNING",
        "the watch below has left you a cold kettle and no position",
    ),
    (
        "POSITION: AT SEA",
        "accurate, unhelpful, and the honest answer until you take a bearing",
    ),
    (
        "NOBODY MOVED THE ISLANDS",
        "they are all exactly where the chart says. one of them is to starboard.",
    ),
    (
        "THE LOG IS BLANK",
        "which is either a quiet night or a navigator with his head down",
    ),
    (
        "FORTY METRES OF WATER",
        "and every fathom of it looks the same as the last",
    ),
];

/// How long the card stays up, seconds. Long enough to read twice and short enough that
/// nobody has to dismiss it — a drill that opens with a key press is a drill with a
/// doorstep.
const READ_IT: f64 = 7.0;

impl Default for Navigation {
    fn default() -> Self {
        Self {
            lost: false,
            last_fix: 0.0,
            fixed_at: None,
            sights: Vec::new(),
            fixed_on: Vec::new(),
            still_in_sight: 0,
            recognised: Vec::new(),
            reading: None,
            reckoning: None,
            fixes: Vec::new(),
            through_water: 0.0,
            measured: None,
            adrift: None,
            // Not nought. A circle of uncertainty that starts at nothing tells a
            // beginner that dead reckoning is exact, which is the one thing it is not.
            unexplained: ASSUMED_SET,
        }
    }
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

/// She no longer knows where she is.
///
/// The state half of being lost, with no teleporting in it: what the exercise does to
/// the *chart* rather than to the boat. Its own function because two things do it now —
/// `Shift L`, and arriving in somebody else's archipelago when the game is reset under
/// you, which is the same condition arrived at from the other end. A boat whose world
/// has just been replaced is a boat that has never seen this coast before, and a chart
/// that went on drawing her position through that would be the one piece of the
/// exercise that cannot be given away by accident.
pub fn cast_adrift(
    nav: &mut Navigation,
    chart: &mut crate::chart::Chart,
    track: &mut crate::chart::Track,
    now: f64,
) {
    // Which card, drawn from the clock so the same joke does not open every exercise.
    nav.adrift = Some((now, (now * 7.0) as usize));
    // The log starts here: a track drawn from before the exercise is a line pointing
    // straight at the answer.
    nav.lost = true;
    nav.last_fix = f64::NEG_INFINITY;
    nav.fixed_at = None;
    nav.sights.clear();
    nav.fixed_on.clear();
    nav.still_in_sight = 0;
    nav.recognised.clear();
    nav.reading = None;
    nav.reckoning = None;
    nav.fixes.clear();
    // Back to the assumption, because what was learned was learned about water she is
    // no longer in.
    nav.unexplained = ASSUMED_SET;
    // And the paper stops following her. Laid down over the middle of the archipelago,
    // which tells her nothing she did not already have: it is where the chart is *of*
    // rather than where she is.
    chart.laid_at = Some(DVec2::ZERO);
    chart.plot.clear();
    track.clear();
}

/// The card, which is drawn for a few seconds and then is not.
pub fn adrift_card(
    mut contexts: bevy_egui::EguiContexts,
    mut nav: ResMut<Navigation>,
    sim: Res<Sim>,
) -> Result {
    let Some((since, which)) = nav.adrift else {
        return Ok(());
    };
    let age = sim.sim.state.time - since;
    if age > READ_IT {
        nav.adrift = None;
        return Ok(());
    }
    // Up at once and gone slowly, which is the way round that lets somebody look up from
    // the tiller and still catch it.
    let alpha = (1.0 - ((age - (READ_IT - 2.0)) / 2.0).max(0.0)).clamp(0.0, 1.0) as f32;
    let ctx = contexts.ctx_mut()?;
    let (words, under) = CARDS[which % CARDS.len()];
    let fade = |c: bevy_egui::egui::Color32| {
        bevy_egui::egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (alpha * 255.0) as u8)
    };
    // Letter-spaced, like the shouts in the drill: it is the same voice at a different
    // volume, and the spacing is what makes a line of monospace read as a *sign*.
    let spaced: String = words
        .chars()
        .map(|c| {
            if c == ' ' {
                "   ".into()
            } else {
                format!("{c} ")
            }
        })
        .collect();
    use bevy_egui::egui;
    egui::Area::new("adrift".into())
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, -60.0))
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(egui::Color32::from_rgba_unmultiplied(
                    6,
                    7,
                    9,
                    (150.0 * alpha) as u8,
                ))
                .inner_margin(egui::Margin::symmetric(40, 16))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new(spaced.trim_end())
                                .color(fade(egui::Color32::from_rgb(232, 214, 150)))
                                .monospace()
                                .size(34.0),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(under)
                                .color(fade(egui::Color32::from_rgb(198, 202, 208)))
                                .monospace()
                                .size(13.0),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(
                                "Shift B the glasses — name three marks, then Shift N and \
                                 cross them",
                            )
                            .color(fade(egui::Color32::from_rgb(150, 200, 150)))
                            .monospace()
                            .size(11.0),
                        );
                    });
                });
        });
    Ok(())
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

    let now = sim.sim.state.time;
    cast_adrift(&mut nav, &mut chart, &mut track, now);
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
    sky: Res<crate::sky::Sky>,
    coast: Res<crate::marks::Coast>,
    mut nav: ResMut<Navigation>,
    mut chart: ResMut<crate::chart::Chart>,
) {
    let Some(said) = chart.accepted.take() else {
        return;
    };
    let hour = sky.hour;
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
        // **What the last run turned out to be worth.**
        //
        // The reckoning was started at the previous fix and has been run on ever since;
        // this fix says where she actually got to. The distance between the two,
        // divided by the time, is everything the reckoning could not know — the stream,
        // the leeway, the helmsman — as a speed. It is the only honest figure for how
        // fast the doubt grows, because it is hers, measured on this water on this day.
        let now = sim.sim.state.time;
        if let Some(run) = nav.reckoning {
            let ran_for = (now - run.since).max(1.0);
            let out = (run.at - said).length() / ran_for;
            nav.unexplained = nav.unexplained * KEEPS + out * (1.0 - KEEPS);
            info!(
                "the reckoning was {:.0} m out over {:.0} min — call it {:.2} kn of set",
                (run.at - said).length(),
                ran_for / 60.0,
                sail_core::units::ms_to_kn(nav.unexplained)
            );
        }
        // And it starts again from here, which is what a fix is for.
        nav.reckoning = Some(Reckoning {
            from: said,
            since: now,
            at: said,
            spread: 0.0,
        });
        nav.fixes.push((said, now));
        // Six of them is an afternoon of plotting and as much line as a chart will carry
        // without becoming a drawing of itself.
        if nav.fixes.len() > 6 {
            nav.fixes.remove(0);
        }
        nav.last_fix = sim.sim.state.time;
        nav.fixed_at = Some(said);
        nav.fixed_on = on.clone();
        // The paper is laid over the fix, which is where a navigator puts it: you work
        // where you are, and she sails across the page from there.
        chart.laid_at = Some(said);
        // And it goes on the chart where it was worked out, which is where a fix goes:
        // a cross, and the time beside it, so an hour later you can see what you were
        // working from.
        chart.plot.push(crate::chart::Plotted::Cross {
            at: said,
            fix: true,
        });
        // **And the time beside it, without being asked.**
        //
        // A fix with no time against it is a fix you cannot run on — run on from when? —
        // and writing the time is the half of the job that gets forgotten, on paper and
        // here. The watch tool exists for marking anything else; a fix stamps itself,
        // because there is exactly one moment it can mean.
        let below = DVec2::new(0.0, -(chart.range as f64) * 0.012);
        chart.plot.push(crate::chart::Plotted::Stamp {
            at: said + below,
            hour,
        });
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
