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

/// Knowing where you are, and the ways of finding out again.
///
/// What this module does in a frame, in one place — see `stages.rs` for why the
/// schedule is named sets rather than one long chain.
pub fn plugin(app: &mut App) {
    use crate::stages::{Deck, Paint};
    app.init_resource::<Navigation>();
    app.add_systems(
        Update,
        (
            lost_at_sea,
            enter_in_the_log,
            write_up_the_log,
            the_fix_holds,
            reckon,
        )
            .in_set(Deck::Weather),
    );
    app.add_systems(
        bevy_egui::EguiPrimaryContextPass,
        adrift_card.in_set(Paint::Over),
    );
}

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
    /// How far she has run through the water since the game began, as a vector.
    ///
    /// **Not a position** — it is the sum of what the compass and the log have said,
    /// which is a thing she has whether or not she knows where she is. Two of them
    /// subtracted give the run between two moments, and that is what transfers a
    /// position line: it is the only reason a bearing can be written in the book now
    /// and plotted twenty minutes later.
    pub water_track: glam::DVec2,
    /// The marks she has drawn on the chart, which is not the same list.
    ///
    /// **Knowing which tower that is and having written its name on the paper are two
    /// acts**, and the instruments ask different questions: the hand bearing compass
    /// asks whether she can *name* what is in the notch — knowledge, the moment the
    /// glasses have told her — and the chart asks whether somebody has put it on the
    /// paper, which happens when the line in the book is laid on. Running both off one
    /// list made a bearing impossible to take on a mark she had just identified, because
    /// the identification was waiting at the chart table for a hand that was on deck
    /// with the glasses.
    pub on_the_paper: Vec<String>,
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
    /// How far out the hand was on the last cast of the log, as a fraction of the
    /// interval: nought is a hand that stopped on the mark, a tenth is six seconds late
    /// on a minute.
    ///
    /// Kept because it is what the reading is *worth*, and a measurement whose accuracy
    /// is not recorded beside it is a measurement that will be believed too far.
    pub log_slip: f64,
    /// The last speed measured by hand off the log, and when — knots, and the clock.
    ///
    /// Kept apart from the instrument's own reading because it is a different kind of
    /// number: the instrument says what she is doing now, and this is what she *did*,
    /// averaged over a leg somebody chose to time. It is the figure a reckoning should
    /// be run at.
    pub measured: Option<(f64, f64)>,
    /// The course she was steering when the log was read.
    ///
    /// Not a position and not pretending to be one: it is which way the water was going
    /// past her, which is the other half of what a cast of the log tells you and the
    /// part that cannot be written in a card.
    pub measured_on: Option<f64>,
    /// Whether the set has ever been *measured* rather than assumed.
    ///
    /// The circle has to grow at something from the first minute, so the figure starts
    /// at an assumption — but an assumption is not knowledge, and a card that prints it
    /// beside things she worked out is a card that lies about where it got them. Until
    /// a fix or a timed leg has taught it, the line is simply not there.
    pub set_measured: bool,
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
#[derive(Clone, Debug)]
pub struct Reckoning {
    /// The fix it was started from, and when.
    pub from: DVec2,
    pub since: f64,
    /// Where and when she altered course, as the log has it written up.
    ///
    /// **A reckoning is a sequence of courses, not a line**, and these are the corners
    /// between them. A navigator keeps it as *this course for that long, then this
    /// course* — each leg laid off from the end of the last — and a leg he has written
    /// up has stopped moving, which is what makes it worth putting numbers beside.
    pub altered: Vec<(DVec2, f64)>,
    /// What she has sailed since the log was last written up: the course, the distance
    /// run on it through the water, and the time she settled on it. Never empty — the
    /// last of them is the leg she is on now.
    ///
    /// **This is the half of a reckoning the chart does not know yet.** The compass is
    /// on the bulkhead and she knows perfectly well that she tacked; the *plot* does
    /// not, because nobody has been to the chart table since. So the paper goes on
    /// laying off the course it was last told, with the distance off the log, and the
    /// pencil position walks away from the boat — which is not a bug in the plot, it is
    /// what an unwritten log costs, and every navigator has stood looking at one.
    pub unwritten: Vec<(f64, f64, f64)>,
    /// When the log was last written up, for saying how old the plot is.
    pub written_up: f64,
    /// The head the leg she is on now was settled on, and how long her head has been
    /// still. Both only so that an alteration can be told from a helmsman wandering.
    pub steering: f64,
    pub steady: f64,
    /// How far the last reckoning turned out to be wrong, for the circle. Metres.
    pub spread: f64,
}

impl Reckoning {
    /// A reckoning started from a fix: the fix, the course she is on, and nothing run
    /// on yet.
    pub fn from_a_fix(at: DVec2, heading: f64, now: f64) -> Self {
        Self {
            from: at,
            since: now,
            altered: Vec::new(),
            unwritten: vec![(heading, 0.0, now)],
            written_up: now,
            steering: heading,
            steady: 0.0,
            spread: 0.0,
        }
    }

    /// Run her on one step: the course steered, the speed through the water, and what
    /// the step is worth.
    ///
    /// Engine-free and takes six numbers, so that what a reckoning does to itself can
    /// be asserted in a test instead of being sailed for twenty minutes to find out.
    pub fn run_on(
        &mut self,
        heading: f64,
        stw: f64,
        yaw_rate: f64,
        now: f64,
        dt: f64,
        unexplained: f64,
    ) {
        // The log counts on whatever course she happens to be on, which is the whole of
        // what a log does.
        if let Some(leg) = self.unwritten.last_mut() {
            leg.1 += stw * dt;
        }
        // Accumulated rather than worked out from the clock, so that a stretch spent
        // steering on a mark stays bought: the circle you did not grow while the tower
        // was on the stem is a circle you do not grow later either.
        self.spread += unexplained * dt;
        // **A new course begins where her head stopped moving**, not where the helm went
        // over: a tack takes fifteen seconds to come round and would otherwise be four
        // legs a few metres long. Steady is what a navigator waits for before he writes
        // the new course down.
        self.steady = if yaw_rate.abs() < sail_core::units::rad(2.0) {
            self.steady + dt
        } else {
            0.0
        };
        let turned = sail_core::units::wrap_pi(heading - self.steering);
        if turned.abs() > ALTERATION && self.steady > SETTLED {
            self.spread += THE_TURN * turned.abs();
            self.steering = heading;
            // **The alteration is always recorded; it is the far end of the plot that
            // goes.**
            //
            // Refusing the new leg at the cap was the wrong half to drop. The course she
            // is steering had already been accepted, so every mile after it went on
            // being laid off along a heading she had stopped steering: the reckoning did
            // not merely stop drawing her turns, it walked away from her, confidently,
            // in the wrong direction — one and three quarter miles over a thirty-tack
            // beat, which the test measures. Losing the oldest corner loses the drawing
            // of water she crossed an hour ago and moves nothing.
            if self.altered.len() + self.unwritten.len() >= CORNERS {
                if self.altered.is_empty() {
                    // Nothing written up, so the start of the plot is what gives: the
                    // oldest leg is folded into where the reckoning begins. It stops
                    // being *from the fix* — which is honest, because after this many
                    // alterations with nobody at the chart table it is not from the fix
                    // in any useful sense either.
                    let (course, ran, _) = self.unwritten.remove(0);
                    self.from += DVec2::new(course.sin(), course.cos()) * ran;
                    self.since = self.unwritten.first().map_or(now, |(_, _, began)| *began);
                } else {
                    self.altered.remove(0);
                }
            }
            self.unwritten.push((heading, 0.0, now));
        }
    }

    /// The last position the log has written down — the fix, or the last corner.
    pub fn written(&self) -> DVec2 {
        self.altered.last().map_or(self.from, |(at, _)| *at)
    }

    /// **Where the plot says she is**, which is not where the reckoning would put her if
    /// anybody had written it up.
    ///
    /// Laid off from the last thing in the log, on the course the log was last told,
    /// with every mile the log has counted since — including the miles she ran after
    /// altering. That is what makes the pencil position drift away from her: the
    /// distance is right and the direction is out of date.
    pub fn plotted_at(&self) -> DVec2 {
        let course = self.unwritten.first().map_or(self.steering, |(c, _, _)| *c);
        let ran: f64 = self.unwritten.iter().map(|(_, ran, _)| ran).sum();
        self.written() + DVec2::new(course.sin(), course.cos()) * ran
    }

    /// **Writing the log up**, which is what going to the chart table is.
    ///
    /// Every course she has sailed since the last visit, laid off in order from where
    /// the log left her: the corners appear on the paper where they belong and the
    /// pencil position stops lying. The compass knew all of this the whole time — the
    /// paper did not, and this is the moment the two are reconciled.
    ///
    /// Costs nothing and changes nothing when there is nothing to write, so it can be
    /// called on every frame the chart is open: *being at the chart table* is the
    /// condition, not pressing anything.
    pub fn write_up(&mut self, now: f64) -> Option<(DVec2, f64, f64)> {
        self.written_up = now;
        if self.unwritten.len() < 2 {
            return None;
        }
        let legs = std::mem::take(&mut self.unwritten);
        let mut here = self.written();
        for (i, (course, ran, _)) in legs.iter().enumerate() {
            here += DVec2::new(course.sin(), course.cos()) * *ran;
            // The end of a finished leg is a corner, and it is stamped with the moment
            // she settled on the *next* course, because that is the time the alteration
            // goes in the log against.
            if let Some((_, _, settled)) = legs.get(i + 1) {
                self.altered.push((here, *settled));
            }
        }
        // The leg she is on survives the write-up with the distance it has already run,
        // so that the plot carries on from the corner rather than from the fix.
        if let Some(last) = legs.last() {
            self.unwritten = vec![*last];
        }
        // **And the course she is on now, with the time she settled on it.** One line,
        // not one per tack: a skipper beating up a sound does not write twenty
        // alterations in the book, he writes the course he is on when he next looks at
        // the chart. The corner it happened at is where the line belongs on the paper.
        self.altered
            .last()
            .zip(self.unwritten.first())
            .map(|((at, when), (course, _, _))| (*at, *when, *course))
    }
}

/// A line of the log, made of what she can see at this moment.
///
/// **The weather in it is the weather when it is written, not when it happened.** A
/// skipper who comes below an hour after the alteration writes the time he altered and
/// the glass he is looking at — which is one of the reasons a log written up late is a
/// worse log, and is a thing this one is allowed to be.
fn note(
    sim: &Sim,
    nav: &Navigation,
    hour: f32,
    at: DVec2,
    course: Option<f64>,
) -> crate::chart::Entry {
    let (glass, tendency) = sim.sim.wind.barometer();
    crate::chart::Entry {
        at,
        hour,
        course,
        // Only a reading still worth something. A speed measured an hour ago is not
        // what she is doing, and every figure on a log line is a statement about the
        // moment beside it.
        speed: nav
            .measured
            .filter(|(_, when)| sim.sim.state.time - when < 900.0)
            .map(|(knots, _)| knots),
        glass,
        tendency,
        sounding: sim.sim.telemetry.depth,
        // Only once it has been measured: the figure the passage *starts* with is an
        // assumption, and an assumption written into a log book becomes a measurement
        // the moment somebody reads it back.
        set: nav.set_measured.then_some(nav.unexplained),
        within: nav
            .reckoning
            .as_ref()
            .filter(|run| run.spread > 1.0)
            .map(|run| run.spread),
        wind_from: sim.sim.telemetry.twd,
        wind: sim.sim.telemetry.tws,
        sea: sim.sim.telemetry.significant_height,
        reef: sim.controls.reef,
        headsail: sim.controls.headsail,
        furl: sim.controls.furl,
    }
}

/// **Write up the log with whatever is known**, which is most of what a log book is.
///
/// A deck log is kept every watch and the sky is only sometimes clear: most lines in one
/// have no fix in them at all, they have a time, a course, the glass, the sounding and
/// what was set. So the button writes a line whether or not two bearings have crossed —
/// asked for from the deck in exactly those words, *even with no bearings, fill it in
/// from what she knows*.
///
/// Where the line goes on the paper is the one judgement here: against her position if
/// the chart may say it, against the reckoning if it may not, and against whatever the
/// paper is laid over if she has neither — because a line in a book still has to be
/// written somewhere, and a reckoning you have not started is not a place.
pub fn write_up_the_log(
    sim: Res<Sim>,
    sky: Res<crate::sky::Sky>,
    mut log: ResMut<crate::chart::DeckLog>,
    mut nav: ResMut<Navigation>,
    mut chart: ResMut<crate::chart::Chart>,
) {
    if !std::mem::take(&mut chart.writing_up) {
        return;
    }
    let now = sim.sim.state.time;
    let at = if nav.shows_position(now) {
        Some(sim.sim.state.pos)
    } else {
        nav.reckoning
            .as_ref()
            .map(|run| run.plotted_at())
            .or(chart.laid_at)
    };
    let Some(at) = at else {
        info!("nothing to write the log against yet — take a fix, or stream the log");
        return;
    };
    // Writing the log up is being at the chart table, so the alterations come in with
    // it and the course on the line is the one she is actually on.
    let course = nav
        .reckoning
        .as_mut()
        .and_then(|run| run.write_up(now))
        .map(|(_, _, course)| course)
        .or_else(|| nav.reckoning.as_ref().map(|run| run.steering));
    // In the book and nowhere else: pencilling it as well would put a second copy of
    // every line on the paper, which is the clutter that took the weather off the chart
    // in the first place.
    let line = note(&sim, &nav, sky.hour, at, course);
    log.write(sky.hour, crate::chart::Wrote::Entered(line));
    info!(
        "log written up at {:02.0}{:02.0}",
        sky.hour.floor(),
        (sky.hour.fract() * 60.0).floor()
    );
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

/// How far she has to come round before it is a new course rather than bad steering.
///
/// Twenty degrees. A deck log gets a new line at every alteration a navigator would
/// call one, and below about fifteen degrees what he has is a helmsman, not a course —
/// writing that down as a leg would break the line into a staircase of nothing.
const ALTERATION: f64 = 0.349;

/// How long her head has to be still before the corner goes on the paper.
///
/// The corner belongs where she **settled** on the new course, not where the helm went
/// over: a tack takes fifteen seconds to come round and would otherwise leave four
/// corners a few metres apart, which is a drawing of the turn rather than a plot of the
/// passage. Four seconds of a steady head is what a navigator waits for before he
/// writes the new course in the log.
const SETTLED: f64 = 4.0;

/// Metres of doubt a radian of alteration adds.
///
/// **This is leeway, not a tax on tacking.** Leeway is the biggest term in a reckoning
/// that nothing aboard measures, and an alteration is exactly where it changes sign and
/// size: she comes out of a tack slow, sails not yet trimmed and hull not yet footing,
/// and makes ten or twelve degrees of it for the first minute instead of her steady
/// four. At four knots that first minute is a little over twenty metres across the
/// track — so a tack through ninety degrees is worth about twenty-five metres, and the
/// figure is per radian turned because that is what the leeway actually scales with.
const THE_TURN: f64 = 16.0;

/// How many corners one page of plotting holds.
///
/// A reckoning is reset by every fix, so this many alterations without one is already a
/// long time to have gone unfixed. Past it the line stops breaking rather than silently
/// dropping the oldest corner — a leg she never sailed is worse than a leg not drawn.
const CORNERS: usize = 24;

/// Run the reckoning on, and grow the doubt.
///
/// Integrated from **the course steered and the speed through the water**, which is what
/// a navigator has: not the boat's true track over the ground, which is the answer and
/// would make the exercise a formality. The difference between the two is exactly the
/// thing the circle is about.
pub fn reckon(
    sim: Res<Sim>,
    coast: Res<crate::marks::Coast>,
    sky: Res<crate::sky::Sky>,
    chart: Res<crate::chart::Chart>,
    mut log: ResMut<crate::chart::DeckLog>,
    mut nav: ResMut<Navigation>,
    time: Res<Time>,
) {
    let dt = time.delta_secs_f64();
    // **Steering at something she can see costs her nothing across the track.**
    //
    // Most of what the circle is made of is not the stream, it is the helmsman: a
    // course held to five degrees over an hour is a mile out sideways before the water
    // has done anything at all. Steering *on a mark* — the tower kept on the stem, or a
    // pair of marks held in line — takes that away entirely, because any wander shows
    // up immediately as the mark moving off the bow and is corrected before it becomes
    // distance. It is the oldest trick in pilotage and the reason a transit is worth
    // more than a bearing.
    //
    // So: a recognised mark within a few degrees of her head, and close enough to be
    // told apart, and the doubt grows at a third of the rate. It is not a reward for
    // pressing anything — it is what the geometry actually does.
    let here = sim.sim.state.pos;
    let heading = sim.sim.state.heading;
    let steering_on_something = coast.marks.iter().any(|m| {
        if !nav.recognised.contains(&m.name) && nav.lost {
            return false;
        }
        let off = m.at - here;
        let range = off.length();
        if range > crate::sight::nameable_from(m, &nav.recognised, &sky) {
            return false;
        }
        sail_core::units::wrap_pi(off.x.atan2(off.y) - heading)
            .abs()
            .to_degrees()
            < 4.0
    });
    let unexplained = nav.unexplained * if steering_on_something { 0.33 } else { 1.0 };
    // The log counts whatever happens. Here rather than in the reckoning below because
    // it goes on counting when there is no reckoning to run.
    nav.through_water += sim.sim.telemetry.stw.abs() * dt;
    // **And every position line walks with her.**
    //
    // A bearing is a statement about where she was when it was taken, and she does not
    // stay there. So each line carries the run since it was shot, and crossing them is
    // crossing them *now* — which is the running fix, and which is what makes a single
    // lighthouse on a long coast worth two bearings and a wait. Through the water,
    // because the compass and the log are what she has.
    let step = DVec2::new(heading.sin(), heading.cos()) * sim.sim.telemetry.stw * dt;
    nav.water_track += step;
    for sight in nav.sights.iter_mut() {
        sight.ran += step;
    }
    let Some(run) = nav.reckoning.as_mut() else {
        return;
    };
    let st = &sim.sim.state;
    run.run_on(
        st.heading,
        sim.sim.telemetry.stw,
        st.yaw_rate,
        st.time,
        dt,
        unexplained,
    );
    // **The plot is as old as your last visit to the chart table.** Spread the chart and
    // the log gets written up — every alteration since you were last here laid off in
    // order — because that is the work, and it is work you do sitting at the table with
    // the compass you have been reading all along. Glance at the folded corner of it
    // instead and you are looking at a plot that has not been told you tacked.
    let altered = if chart.spread {
        run.write_up(st.time)
    } else {
        None
    };
    // **And one line in the book for it**, with the time she actually settled on the
    // course rather than the time anybody noticed. Not one line per tack — a beat up a
    // sound is twenty alterations and one entry in the book.
    //
    // In the book and not on the paper: writing it on the chart is a thing the navigator
    // does, with the button that writes the log up, and the difference matters because
    // ink that appears on a chart by itself is ink nobody can account for.
    if let Some((_, when, course)) = altered {
        let hour = (sky.hour - ((st.time - when) / 3_600.0) as f32).rem_euclid(24.0);
        log.write(hour, crate::chart::Wrote::Altered { course });
    }
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
            water_track: glam::DVec2::ZERO,
            on_the_paper: Vec::new(),
            recognised: Vec::new(),
            reading: None,
            reckoning: None,
            fixes: Vec::new(),
            through_water: 0.0,
            measured: None,
            measured_on: None,
            log_slip: 0.0,
            set_measured: false,
            adrift: None,
            // Not nought. A circle of uncertainty that starts at nothing tells a
            // beginner that dead reckoning is exact, which is the one thing it is not.
            unexplained: ASSUMED_SET,
        }
    }
}

impl Navigation {
    /// Take one more measurement of how fast the water is moving her about.
    ///
    /// Two things measure it and they must fold it in the same way, or the circle grows
    /// at a different rate depending on which instrument happened to notice: a fix
    /// landing on a reckoning, and a leg timed between two positions with the log
    /// running. One method, one constant.
    pub fn learn_the_set(&mut self, measured: f64) {
        self.unexplained = self.unexplained * KEEPS + measured * (1.0 - KEEPS);
        self.set_measured = true;
    }

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
/// `Shift Backspace`, and arriving in somebody else's archipelago when the game is reset under
/// you, which is the same condition arrived at from the other end. A boat whose world
/// has just been replaced is a boat that has never seen this coast before, and a chart
/// that went on drawing her position through that would be the one piece of the
/// exercise that cannot be given away by accident.
pub fn cast_adrift(
    nav: &mut Navigation,
    chart: &mut crate::chart::Chart,
    log: &mut crate::chart::DeckLog,
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
    nav.on_the_paper.clear();
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
    // And the book starts a new page. Every line in it was about water she is no longer
    // in — bearings on marks that may not be on this coast at all, when the world has
    // been changed under her — and a log book that carries them over is a log book that
    // lies about the morning.
    log.lines.clear();
    track.clear();
    // **And whatever was being measured is over.**
    //
    // A cast of the log that began before she was moved would be read against water she
    // is no longer in, and the leg it drew would be a line between two places that have
    // nothing to do with each other — which would then teach the circle a set that was
    // never there. Everything that was in hand is put down: the log, the watch, and the
    // speed from the last cast.
    chart.streaming = None;
    chart.watch = crate::chart::Stopwatch::default();
    nav.measured = None;
    nav.log_slip = 0.0;
    nav.set_measured = false;
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
    mut log: ResMut<crate::chart::DeckLog>,
    mut sim: ResMut<Sim>,
    mut nav: ResMut<Navigation>,
    mut track: ResMut<crate::chart::Track>,
    mut pilot: ResMut<crate::input::Pilot>,
) {
    // **Shift and backspace, not shift and L.**
    //
    // This throws the passage away: she is picked up and put down somewhere else in the
    // archipelago, the fix is gone and the deck log is cleared. That is a fine thing to
    // ask for and a terrible thing to *land on*, and it used to sit one shifted finger
    // away from `L`, which streams the log — a key you press while sailing, with the
    // other hand already holding shift for a fine bite on the helm. Reported from the
    // deck after it happened by accident.
    //
    // Backspace is the right neighbourhood for it: the key that undoes, nowhere near
    // anything that trims or steers, and nothing else in the game reads it.
    let asked = keys.just_pressed(KeyCode::Backspace)
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
    cast_adrift(&mut nav, &mut chart, &mut log, &mut track, now);
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
    mut log: ResMut<crate::chart::DeckLog>,
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
    if out_by < good_enough {
        // The lines come off the chart the moment the fix is in the log, which is what a
        // navigator does with them: they were working, the fix is the answer, and
        // leaving them on is how a plot turns into a thicket nobody can read an hour
        // later. What is left behind is the fix itself, which is the one thing worth
        // keeping.
        //
        // **Only when there is a fix to leave behind.** Rubbing them out before knowing
        // whether the cross was good enough took away the navigator's work and gave him
        // nothing for it — and the lines are exactly what he needs at that moment,
        // because the answer to a fix that will not close is a third bearing on top of
        // the two he has. Reported from the deck as a line vanishing for no reason.
        // Clearing the sights takes the position lines off the paper with them: they
        // are drawn from the sights themselves rather than copied into the plot, so
        // there is one place where a line exists and no way for the copy to outlive the
        // reading. What is left behind is the fix, which is the one thing worth keeping.
        nav.sights.clear();
        // **What the last run turned out to be worth.**
        //
        // The reckoning was started at the previous fix and has been run on ever since;
        // this fix says where she actually got to. The distance between the two,
        // divided by the time, is everything the reckoning could not know — the stream,
        // the leeway, the helmsman — as a speed. It is the only honest figure for how
        // fast the doubt grows, because it is hers, measured on this water on this day.
        let now = sim.sim.state.time;
        // Entering a fix in the log is being at the chart table, so the log is written
        // up before anything is measured against it: what the run was worth is a
        // question about where the reckoning actually got her, not about where a plot
        // nobody had brought up to date was pointing.
        if let Some(run) = nav.reckoning.as_mut() {
            run.write_up(now);
        }
        if let Some((since, ran_to)) = nav
            .reckoning
            .as_ref()
            .map(|run| (run.since, run.plotted_at()))
        {
            let ran_for = (now - since).max(1.0);
            let out = (ran_to - said).length() / ran_for;
            nav.learn_the_set(out);
            info!(
                "the reckoning was {:.0} m out over {:.0} min — call it {:.2} kn of set",
                (ran_to - said).length(),
                ran_for / 60.0,
                sail_core::units::ms_to_kn(nav.unexplained)
            );
        }
        // And it starts again from here, which is what a fix is for.
        nav.reckoning = Some(Reckoning::from_a_fix(said, sim.sim.state.heading, now));
        nav.fixes.push((said, now));
        // Six of them is an afternoon of plotting and as much line as a chart will carry
        // without becoming a drawing of itself.
        if nav.fixes.len() > 6 {
            nav.fixes.remove(0);
        }
        nav.last_fix = sim.sim.state.time;
        nav.fixed_at = Some(said);
        nav.fixed_on = on.clone();
        // **And they are in sight, because she has just shot them.**
        //
        // Counting how many of the marks a fix stands on is another system's job and it
        // runs on the next frame, so for one frame the count belonged to the *previous*
        // fix — usually nought, which is why she was taking this one. Everything that
        // asks whether the chart may say where she is was told no at the exact moment it
        // learned where she was, and the cross and the time went on the paper through
        // that answer: the fix appeared with no hour against it. Reported from the deck
        // in those words. The marks were shot a second ago; this is not an assumption.
        nav.still_in_sight = on.len();
        // The paper is laid over the fix, which is where a navigator puts it: you work
        // where you are, and she sails across the page from there.
        chart.laid_at = Some(said);
        // And it goes on the chart where it was worked out, which is where a fix goes:
        // a cross, and the time beside it, so an hour later you can see what you were
        // working from.
        chart.write_down(
            crate::chart::Plotted::Cross {
                at: said,
                fix: true,
            },
            nav.shows_position(now),
        );
        // **And the time beside it, without being asked.**
        //
        // A fix with no time against it is a fix you cannot run on — run on from when? —
        // and writing the time is the half of the job that gets forgotten, on paper and
        // here. The watch tool exists for marking anything else; a fix stamps itself,
        // because there is exactly one moment it can mean.
        let below = DVec2::new(0.0, -(chart.range as f64) * 0.012);
        let line = note(&sim, &nav, hour, said + below, None);
        chart.write_down(crate::chart::Plotted::Entry(line), nav.shows_position(now));
        log.write(hour, crate::chart::Wrote::Entered(line));
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
pub fn the_fix_holds(
    sim: Res<Sim>,
    coast: Res<crate::marks::Coast>,
    sky: Res<crate::sky::Sky>,
    mut nav: ResMut<Navigation>,
) {
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
                Some(mark) => crate::sight::nameable_from(mark, &nav.recognised, &sky),
                // A mark that is no longer on the coast at all — a world rebuilt under
                // the fix — cannot be re-shot, and the old rule is the safe answer.
                None => kind.identified_from(),
            };
            range < reach * through_it
        })
        .count();
}

#[cfg(test)]
mod the_reckoning {
    use super::*;
    use sail_core::units::rad;

    /// Run her on for a stretch, steering what she is told, in steps a frame long.
    ///
    /// `yaw_rate` is worked out from the heading she is asked for rather than passed in,
    /// because that is what the boat does: a head that is moving has a rate and a head
    /// that is not has none, and a test that could set the two independently could
    /// assert a corner the sim can never produce.
    fn sail(run: &mut Reckoning, legs: &[(f64, f64)]) -> f64 {
        let dt = 1.0 / 60.0;
        let mut now = run.since + run.unwritten.iter().map(|(_, ran, _)| ran).sum::<f64>() / 2.5;
        let mut was = run.steering;
        for &(heading, seconds) in legs {
            for _ in 0..(seconds / dt) as usize {
                now += dt;
                run.run_on(heading, 2.5, (heading - was) / dt, now, dt, ASSUMED_SET);
                was = heading;
            }
        }
        now
    }

    /// Coming round through ninety degrees the way she really does it: a few seconds of
    /// turning, then steady on the new course.
    fn tack_to(run: &mut Reckoning, to: f64, then_for: f64) -> f64 {
        let from = run.steering;
        let mut legs: Vec<(f64, f64)> = (1..=15)
            .map(|k| (from + (to - from) * k as f64 / 15.0, 1.0))
            .collect();
        legs.push((to, then_for));
        sail(run, &legs)
    }

    /// **The plot does not know she tacked until somebody tells it.**
    ///
    /// This is the whole of it. The compass has been on the bulkhead the entire time and
    /// she knows exactly what she did; the paper goes on laying off the old course with
    /// the new miles, so the pencil position walks away from her. Writing the log up
    /// puts it where it belongs — and the distance it jumps is what the unwritten log
    /// was worth.
    #[test]
    fn an_unwritten_log_leaves_the_plot_on_the_old_course() {
        let mut run = Reckoning::from_a_fix(DVec2::ZERO, 0.0, 0.0);
        sail(&mut run, &[(0.0, 300.0)]);
        let now = tack_to(&mut run, rad(90.0), 600.0);
        // Nobody has been to the chart table: one leg in the log, still running north.
        assert!(run.altered.is_empty(), "no corner has been written yet");
        let stale = run.plotted_at();
        assert!(
            stale.x.abs() < 1.0 && stale.y > 2_200.0,
            "the plot is still running north: {stale:?}"
        );
        run.write_up(now);
        let honest = run.plotted_at();
        assert_eq!(run.altered.len(), 1, "one alteration goes in the log");
        assert!(
            honest.y > 700.0 && honest.y < 800.0 && honest.x > 1_400.0,
            "written up, she is round the corner and running east: {honest:?}"
        );
        let cost = (honest - stale).length();
        assert!(
            cost > 1_500.0,
            "ten minutes of unwritten log is a mile and a half of pencil error, got \
             {cost:.0} m"
        );
    }

    /// Writing up a log with nothing in it changes nothing.
    ///
    /// Which is what lets the chart write itself up on every frame it is open: *being at
    /// the table* is the condition, and a condition that is cheap to satisfy does not
    /// need a key of its own.
    #[test]
    fn writing_up_nothing_costs_nothing() {
        let mut run = Reckoning::from_a_fix(DVec2::ZERO, rad(30.0), 0.0);
        sail(&mut run, &[(rad(30.0), 400.0)]);
        let before = run.plotted_at();
        for k in 0..50 {
            run.write_up(400.0 + k as f64);
        }
        assert!(run.altered.is_empty(), "there was no alteration to write");
        assert!(
            (run.plotted_at() - before).length() < 1e-9,
            "the plot did not move"
        );
    }

    /// A tack is a new leg, and it costs her something.
    ///
    /// The two halves of what a reckoning has to say about an alteration: the line
    /// breaks where she settled on the new course, and the circle grows because her
    /// leeway has just changed sign and nothing aboard measures it.
    #[test]
    fn a_tack_breaks_the_line_and_grows_the_circle() {
        let mut run = Reckoning::from_a_fix(DVec2::ZERO, rad(40.0), 0.0);
        sail(&mut run, &[(rad(40.0), 300.0)]);
        let before = run.spread;
        let now = tack_to(&mut run, rad(-50.0), 300.0);
        run.write_up(now);
        assert_eq!(run.altered.len(), 1, "one corner for one tack");
        let grew = run.spread - before - ASSUMED_SET * 315.0;
        assert!(
            (20.0..35.0).contains(&grew),
            "a tack through ninety is worth twenty-odd metres of doubt, not {grew:.0} m"
        );
    }

    /// A helmsman is not an alteration.
    ///
    /// **This is the test the threshold exists for.** Nobody steers to the degree, and a
    /// reckoning that put a corner on the paper every time she wandered would be a
    /// staircase of legs a few metres long with a course written on each — which is
    /// worse than one straight line, because it looks like information.
    #[test]
    fn wandering_is_not_a_new_course() {
        let mut run = Reckoning::from_a_fix(DVec2::ZERO, 0.0, 0.0);
        let legs: Vec<(f64, f64)> = (0..40)
            .map(|i| (rad(if i % 2 == 0 { 12.0 } else { -12.0 }), 15.0))
            .collect();
        let now = sail(&mut run, &legs);
        run.write_up(now);
        assert!(
            run.altered.is_empty(),
            "twelve degrees either side of the course is a helmsman, not a leg — got {} \
             corners",
            run.altered.len()
        );
    }

    /// The corner goes where she settled, not where the helm went over.
    ///
    /// A tack takes a quarter of a minute to come round, and in that time she runs a
    /// hundred feet. Mark the corner at the first degree of the turn and the chart says
    /// she altered before she did; mark it where her head stopped moving and it says
    /// what the deck log says.
    #[test]
    fn the_corner_is_where_her_head_stopped() {
        let mut run = Reckoning::from_a_fix(DVec2::ZERO, 0.0, 0.0);
        let now = tack_to(&mut run, rad(88.0), 60.0);
        run.write_up(now);
        assert_eq!(run.altered.len(), 1, "one manoeuvre, one corner");
        let (corner, _) = run.altered[0];
        let ran = (corner - run.from).length();
        assert!(
            (25.0..80.0).contains(&ran),
            "the corner belongs where she settled, about fifty metres on — not {ran:.0} m"
        );
    }

    /// Ten minutes north is fifteen hundred metres north.
    ///
    /// The boring case, asserted because everything else here is about the plot being
    /// wrong on purpose and something has to pin down that the arithmetic underneath it
    /// is right.
    #[test]
    fn a_straight_run_is_one_leg() {
        let mut run = Reckoning::from_a_fix(DVec2::ZERO, 0.0, 0.0);
        sail(&mut run, &[(0.0, 600.0)]);
        assert!(run.altered.is_empty());
        assert!(
            (run.plotted_at().y - 1_500.0).abs() < 10.0,
            "ten minutes at two and a half metres a second is fifteen hundred metres, \
             got {:.0}",
            run.plotted_at().y
        );
    }

    /// **Past the corner limit she must still know where she is.**
    ///
    /// The cap exists so that one page of plotting does not become a thousand corners.
    /// But refusing to record the alteration while still accepting the new course was
    /// the wrong half to drop: every mile after it went on being laid off along a
    /// heading she had stopped steering, so the reckoning did not merely stop drawing
    /// the turns — it walked away from her, confidently, in the wrong direction.
    ///
    /// A beat to windward in a narrow sound is twenty tacks, and twenty-five is not an
    /// unusual hour. So: thirty alterations, and the plot still has to agree with the
    /// water she went through.
    #[test]
    fn a_long_beat_does_not_walk_the_plot_off_the_chart() {
        let mut run = Reckoning::from_a_fix(DVec2::ZERO, rad(45.0), 0.0);
        let mut truth = DVec2::ZERO;
        for tack in 0..30 {
            let course = if tack % 2 == 0 { rad(-45.0) } else { rad(45.0) };
            let now = tack_to(&mut run, course, 120.0);
            run.write_up(now);
            // What she actually went through the water, laid off the same way a
            // navigator would: the course she settled on, for the minutes she held it.
            truth += DVec2::new(course.sin(), course.cos()) * 2.5 * 135.0;
        }
        let out = (run.plotted_at() - truth).length();
        assert!(
            out < 400.0,
            "after thirty tacks the plot is {out:.0} m from the water she ran through"
        );
    }
}
