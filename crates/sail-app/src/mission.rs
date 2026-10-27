//! Pilotage: putting a stranger through water she does not know.
//!
//! **The oldest job on any coast, and the only one where local knowledge is the whole of
//! the value.** A deep-draft vessel stands in towards a shore she has never seen, with a
//! chart that stops at the ten-metre line and no idea which side of the skerry is foul.
//! Somebody who sails here every week goes out in a small boat, gets aboard, and takes
//! her in. That is the trade, it is still done exactly like this, and everything the game
//! already has — the chart, the sounder, the glasses, the boat handling — is what it asks
//! for.
//!
//! The job has three parts and they are all seamanship rather than menu:
//!
//! 1. **Catch her.** She is making six knots towards a coast and she will not wait. You
//!    have to sail your own boat better than she is sailing hers, and get *ahead* of her
//!    — which is the whole of it, because a ship follows a pilot boat she can see.
//! 2. **Show her the way.** Nobody is transferred and nothing is handed over: you sail
//!    the water you want her in, and she comes along it behind you at a ship's pace.
//!    She draws nearly four metres where you draw two, so the water that is nothing to
//!    you is the bottom to her. What the mission is *about* is that difference.
//! 3. **Let her go.** `Enter` in safe water off the anchorage, and the pilot flag comes
//!    down off our shrouds — which is where it has been flying since the job was taken,
//!    because the flag is ours and so is the responsibility.
//!
//! And before any of it, there are her barrels. A deck cargo that has broken its lashings
//! in the gale she has just come through goes over the side a cask at a time — which is
//! why they are in the water at all, and why collecting them is on the way to the job
//! rather than instead of it.

use bevy::prelude::*;
use glam::DVec2;
use sail_core::units::{kn_to_ms, rad, wrap_pi};

use crate::ship::{Brig, CLOSEST};
use crate::sim::Sim;

/// The pilot job: the brig is sailed, told where to go, and drawn, in that order.
///
/// What this module does in a frame, in one place — see `stages.rs` for why the
/// schedule is named sets rather than one long chain.
pub fn plugin(app: &mut App) {
    use crate::stages::{Deck, Paint};
    app.init_resource::<Pilotage>();
    app.init_resource::<WantsPilotage>();
    app.add_systems(
        Update,
        (
            take_the_job,
            discharge,
            fly_the_flag,
            lay_off_the_route,
            crate::race::run,
            crate::race::rivals,
            crate::race::dress,
            run,
            crate::ship::sail_her,
            crate::ship::work_her,
            crate::ship::place,
        )
            .chain()
            .in_set(Deck::Weather),
    );
    app.add_systems(bevy_egui::EguiPrimaryContextPass, draw.in_set(Paint::Over));
}

/// What she is carrying under her, below which she is on the bottom.
pub const SHE_DRAWS: f64 = crate::ship::DRAFT as f64;

/// How near the anchorage she has to be brought, metres.
///
/// Three cables, which is where a ship of this size rounds up and lets go — near enough
/// that she is *in*, far enough that nobody is asked to park her.
const ARRIVED: f64 = 550.0;

/// How often she loses another cask while her lashings are going, seconds.
const A_CASK_EVERY: f64 = 70.0;

/// The state of the job.
#[derive(Resource, Default)]
pub struct Pilotage {
    /// Whether the mission has been taken on at all.
    pub running: bool,
    /// Whether she has somebody to follow — set the moment the pilot boat gets ahead of
    /// her, and the only thing that changes about her behaviour.
    pub leading: bool,
    /// How it ended, and why.
    pub over: Option<Outcome>,
    /// Seconds since she last shed a cask.
    shedding: f64,
    /// Where the pilot boat has been, for her to follow. Sim coordinates, newest last.
    wake: Vec<DVec2>,
    /// The least water she has crossed, for the report at the end.
    pub least_water: f64,
    /// What she is told, for the deck to read.
    pub said: Option<String>,
    /// What her master has just called across, and the simulation time it stands until.
    ///
    /// **A ship with a pilot aboard is not a silent ship.** The master does not hand over
    /// and go below: he stands beside you and says what his vessel will and will not do,
    /// because he is the one who knows her and because it is still his ship. Everything
    /// she calls across is information the player cannot get any other way — how fast she
    /// can follow, how fine a turn she can make, how close to the wind she will lie — and
    /// all of it arrives the way it arrives on a real bridge, which is somebody saying it.
    pub hail: Option<(String, f64)>,
    /// When the last one was called, so that she is not a parrot.
    hailed_at: f64,
    /// What she had set last frame, so that a change in it is a thing she can mention —
    /// and `None` before she has set anything, because *nothing* and *not yet known* are
    /// different states and only one of them is worth a hail.
    canvas: Option<[f32; 3]>,
    /// A course she has been ordered to steer, radians from north.
    ///
    /// **The one order this locker could not give.** Every flag in it was *stop*, *wait*
    /// or *answer me*; none of them was *this way*, so the only way to tell a ship where
    /// to go was to go there yourself and hope she followed. E and I are the two signals
    /// a vessel conning another one actually flies — *I am altering my course to
    /// starboard*, *to port* — and on the radio it is a course in three figures. Both
    /// end here, because a course is a course however it arrived.
    ///
    /// Held until she is told to carry on, or until the water says otherwise: an order
    /// does not outrank the bottom, and *you are running into danger* takes the helm
    /// back off it.
    pub ordered: Option<f64>,
    /// When she last called up on sixteen, for the same reason — and `None` until she
    /// has, because a job that begins at time zero would otherwise count the gap from
    /// the start of the world and say nothing for the first three minutes.
    called_at: Option<f64>,
    /// The water she is to be taken through, newest first leg first, and when it was
    /// last worked out. Empty when there is no job or no way in.
    pub route: Vec<DVec2>,
    laid_off: f64,
    /// Where she was when the route was last laid off, so the search starts from her
    /// rather than from the pilot boat.
    pub brig_at: DVec2,
    /// Whether anybody has ever been ahead of her. Before that she stands on by
    /// herself, which is the chase; after it, losing the pilot means waiting for him.
    pub led_once: bool,
    /// And where the job ends: the water off it she can be left in.
    pub let_go_at: DVec2,
    /// Where she is bound, and the simulation time the job started.
    ///
    /// **The anchorage is the end of the job, not deep water.** Written first as *clear
    /// water on the far side*, which read well and meant that a brig started in forty
    /// metres and boarded in forty metres was through before anybody had piloted
    /// anything — it reported her delivered within seconds, with the time counted from
    /// the start of the session rather than from the start of the work. A pilot takes a
    /// ship **in**. That is the whole trade and it is what the player is paid for.
    pub bound_for: DVec2,
    began: f64,
}

/// How long a hail stands on screen, and the least time between two of them, seconds.
const HAIL_STANDS: f64 = 9.0;
const BETWEEN_HAILS: f64 = 11.0;

/// How a piece of pilotage ended.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// In the anchorage, with the time it took, the casks saved, and the least water
    /// she was taken over — which is the number another pilot would ask about.
    Delivered {
        minutes: f64,
        casks: u32,
        least: f64,
    },
    /// On the putty. The one failure this job has.
    Aground { least: f64 },
    /// She went by. A pilot who cannot catch a ship has not lost anything except the fee.
    MissedHer,
}

impl Outcome {
    pub fn words(&self) -> String {
        match self {
            Outcome::Delivered {
                minutes,
                casks,
                least,
            } => {
                format!(
                    "she is in — {minutes:.0} minutes, {least:.1} m at the worst of it, \
                     {casks} casks saved"
                )
            }
            Outcome::Aground { least } => {
                format!("she is aground — you took her over {least:.1} m of water")
            }
            Outcome::MissedHer => "she stood on without you — nobody got aboard".into(),
        }
    }
}

impl Pilotage {
    /// Start the job: put her outside the islands, heading in.
    pub fn begin(
        &mut self,
        brig: &mut Brig,
        at: DVec2,
        bound_for: DVec2,
        let_go_at: DVec2,
        now: f64,
    ) {
        *self = Pilotage {
            running: true,
            least_water: f64::INFINITY,
            bound_for,
            let_go_at,
            began: now,
            ..Default::default()
        };
        brig.afloat = true;
        brig.at = at;
        brig.bound_for = bound_for;
        let off = bound_for - at;
        brig.heading = off.x.atan2(off.y);
        brig.speed = kn_to_ms(5.0);
        brig.set = [1.0, 1.0, 1.0];
        brig.name = crate::ship::ship_name((at.x.abs() as u64) << 20 ^ at.y.abs() as u64);
        self.said = Some(format!(
            "the brig {} standing in from seaward, wanting a pilot",
            brig.name
        ));
    }

    /// Whether the pilot boat is where a ship can follow it: ahead, and near enough to
    /// be seen from her deck.
    ///
    /// **This is the whole mechanic and it is one measurement.** A ship does not follow
    /// a boat that is behind her, and she cannot follow one hull down: you lead her by
    /// being in front of her, which is also the only way anybody has ever piloted
    /// anything from another vessel.
    pub fn leading_her(brig: &Brig, boat: DVec2) -> bool {
        let off = boat - brig.at;
        let range = off.length();
        if range > IN_SIGHT || range < 30.0 {
            return false;
        }
        wrap_pi(off.x.atan2(off.y) - brig.heading).abs() < AHEAD
    }
}

/// How far ahead the pilot boat can be and still be worth following, metres.
///
/// Three quarters of a mile. Further than that and her master is steering at a shape on
/// the horizon, which is not pilotage; nearer than thirty metres and he is steering at
/// something he is about to run down.
pub const IN_SIGHT: f64 = 1_400.0;

/// And how far off her bow, either side, before she stops calling it *ahead*.
pub const AHEAD: f64 = rad(70.0);

/// The course her master will not be led onto — her rig's limit plus the margin a
/// seaman keeps inside it. See [`crate::ship::FULL_AND_BY`], which is the same number
/// because it is the same fact: the course she lies when she cannot lay the mark is the
/// course outside which you must lead her.
pub use crate::ship::FULL_AND_BY as WILL_NOT_LIE;

/// Station: further than this and she has lost you, nearer and she is about to run you
/// down.
const TOO_FAR: f64 = 900.0;
const TOO_NEAR: f64 = 110.0;

/// How fast the gap between her bow and the boat ahead is shutting, m/s.
///
/// **Two speeds subtracted is not a closing rate.** It was `her speed through the water
/// minus our speed over the ground`, which says she is gaining on us whenever she is
/// going faster — true of a ship astern on the same course and false of one crossing,
/// and it was being compared against a tide neither of them is in the same part of. It
/// matters more than it did now that the answer takes sail off her: a brig sailing away
/// from the pilot boat at six knots was spilling her courses to avoid running him down.
///
/// The honest number is the relative velocity projected onto the line between them,
/// which is what a radar calls range rate and what the eye on the stern rail is reading.
fn closing_on(sim: &Sim, brig: &Brig, here: DVec2) -> f64 {
    let between = here - brig.at;
    let range = between.length();
    if range < 1.0 {
        return 0.0;
    }
    let cog = sim.sim.telemetry.cog;
    let ours = DVec2::new(cog.sin(), cog.cos()) * sim.sim.telemetry.sog;
    (brig.way - ours).dot(between / range)
}

/// What her master has to say about how he is being led, if anything.
///
/// One function with the measurements as arguments, because the thing that makes a hail
/// worth having is that it is *true* — it has to be the same numbers her sailing is made
/// of, and a test has to be able to ask it questions without a sea.
///
/// In the order a seaman would say them: the one that is about to put her on the bottom
/// first, then the one that is about to stop her, then the ones about keeping up.
pub fn remark(
    under: f64,
    ahead: f64,
    twa_wanted: f64,
    turn: f64,
    gap: f64,
    closing: f64,
) -> Option<&'static str> {
    if ahead < SHE_DRAWS + 2.5 {
        return Some("shoaling ahead — we draw four metres, pilot");
    }
    if under < SHE_DRAWS + 1.5 {
        return Some("we are feeling the bottom — take us off this");
    }
    // A square-rigger that is led inside her own limit does not pinch: she stops, and a
    // stopped ship in a narrow place is the whole disaster.
    if twa_wanted.abs() < WILL_NOT_LIE {
        return Some("we cannot lie that close — she will not go there");
    }
    if turn > rad(55.0) {
        return Some("that is too fine a turn for her — give her room to swing");
    }
    if gap > TOO_FAR {
        return Some("you are drawing away from us — we cannot hold that");
    }
    if gap < TOO_NEAR && closing > kn_to_ms(1.0) {
        return Some("we are coming up on you — she has no brakes");
    }
    None
}

impl Pilotage {
    /// Put a hail on the deck, if she has not just said something else.
    /// The same, but it is an answer and answers are never suppressed.
    ///
    /// **A ship that has read your signal replies to it.** The limiter above exists so
    /// that she is not a parrot — she has opinions about the water and she offers them
    /// every so often — but a hoist is a question asked on purpose, and "she said nothing
    /// because somebody mentioned the tide nine seconds ago" is the behaviour that makes
    /// a player stop trusting the channel.
    fn answer_across(&mut self, words: &str, now: f64) {
        self.hailed_at = now;
        self.hail = Some((words.to_string(), now + HAIL_STANDS));
        info!("brig: {words}");
    }

    /// Whether she would say an unprompted thing now, or has only just said one.
    ///
    /// **The limiter has to be asked *before* she keys the microphone**, not after. Her
    /// remarks are worked out every frame — the water under her, the corner coming up,
    /// where the pilot boat is — and the gap between hails used to be enforced inside
    /// the line that writes the caption, which was fine while a hail was a caption. The
    /// moment it also put a transmission on the air, sixty copies of *we are coming up
    /// on you* a second went into the queue and the channel never fell quiet again.
    fn would_say(&self, now: f64) -> bool {
        now - self.hailed_at >= BETWEEN_HAILS
    }

    fn call_across(&mut self, words: &str, now: f64) {
        if !self.would_say(now) {
            return;
        }
        self.hailed_at = now;
        self.hail = Some((words.to_string(), now + HAIL_STANDS));
        info!("brig: {words}");
    }
}

/// How far a voice carries between two vessels under sail, metres.
///
/// **Half a cable, and the canvas takes the rest.** `signals.rs` is built on the fact
/// that two ships cannot shout at each other — it is the whole reason a flag locker
/// exists — and then her master was heard calling across four hundred metres of water
/// about the tide. One of those two was wrong, and it was not the flags.
///
/// So she has a set, like everybody else afloat since about 1960. Inside this she is a
/// voice; outside it she is a transmission, on the channel she asked for, heard if you
/// are on it and in range and not if you are not.
pub const HAILING: f64 = 120.0;

/// How she is heard: over the rail, or over the air.
///
/// Built once a frame from what is already known — where she is, how far off we are, and
/// her set — so that every line she says goes through one place and is subject to one
/// rule.
struct Voice<'a> {
    radio: &'a mut crate::radio::Radio,
    /// Where she is, for the range the signal has to cover.
    at: DVec2,
    /// And how far off we are, which decides whether it is a shout or a call.
    range: f64,
    now: f64,
    name: String,
}

impl Voice<'_> {
    /// Put it across. On her set always; onto the deck only if it got there.
    ///
    /// **A hail that was not heard is not on the screen.** That is the whole of the
    /// change: her words used to appear whatever the range, which made the radio
    /// decorative and the flag locker a museum piece. Now the sequence is the one a
    /// pilot actually runs — she calls you up on sixteen, you go to her working channel,
    /// and what she has to say about the water arrives there.
    fn across(&mut self, job: &mut Pilotage, words: &str, answering: bool) {
        // An answer is a thing she was asked for and is never held back; a remark is one
        // of hers, and she is not a parrot — on the air least of all, where a station
        // that never stops talking is a station on top of everybody else.
        if !answering && !job.would_say(self.now) {
            return;
        }
        let t = crate::radio::Transmission::saying(
            &self.name,
            crate::radio::WORKING,
            words,
            Some((self.at, crate::radio::BRIG_AERIAL)),
            self.now,
        );
        let reached = self.range < HAILING || self.radio.hears(&t, self.range) > 0.0;
        self.radio.broadcast(t);
        if reached {
            if answering {
                job.answer_across(words, self.now);
            } else {
                job.call_across(words, self.now);
            }
        } else {
            // **She said it whether or not anybody was on the channel**, and the clock
            // that keeps her from repeating herself has to run on that and not on
            // whether you heard it. Otherwise a pilot boat out of range has her keying
            // the microphone sixty times a second to an empty sea.
            job.hailed_at = self.now;
        }
    }

    /// Her first call, on sixteen, which is where she says what channel to go to.
    ///
    /// Repeated while nobody has come to it, because a ship wanting a pilot keeps
    /// calling — and because the one thing worse than a channel you have to find is a
    /// channel you had one chance to hear.
    fn calling_up(&mut self, job: &mut Pilotage) {
        if job
            .called_at
            .is_some_and(|at| self.now - at < CALLING_EVERY)
        {
            return;
        }
        job.called_at = Some(self.now);
        self.radio.broadcast(crate::radio::Transmission::saying(
            &self.name,
            crate::radio::CALLING,
            &format!(
                "Pilot boat, pilot boat. This is the brig {}, \
                 standing in from seaward and requiring a pilot. \
                 Channel {}. Over.",
                self.name,
                crate::radio::WORKING
            ),
            Some((self.at, crate::radio::BRIG_AERIAL)),
            self.now,
        ));
    }
}

/// How far one hoist of E or I puts her helm over, radians.
///
/// **Thirty degrees, and two hoists is how you get sixty.** A brig takes minutes to come
/// round and a pilot conning one gives her a turn at a time; an order that swung her
/// ninety degrees would be an order she was still carrying out a quarter of an hour
/// later, with the pilot boat somewhere else entirely.
const ORDER_TURNS: f64 = rad(30.0);

/// How often she calls up on sixteen while nobody has answered, seconds.
const CALLING_EVERY: f64 = 180.0;

/// The job, frame by frame: her casks, her following, and the two ways it ends.
/// Carry out what has been hoisted, if she has read it.
///
/// Five flags, and each one does the thing the code says it means rather than a
/// convenience invented for the game: stopped is stopped, held is held, and *you are
/// standing into danger* turns her away from the shoal rather than stopping her, because
/// a ship that stops in the wrong place is still in the wrong place.
fn told_her(
    brig: &mut Brig,
    job: &mut Pilotage,
    answer: &mut crate::signals::Answer,
    sim: &Sim,
    voice: &mut Voice<'_>,
) {
    use crate::signals::Flag;
    let Some(flag) = answer.reading else {
        return;
    };
    match flag {
        // Words, not helm: she says what she is doing, which is the one question this
        // mission could not answer before.
        Flag::Kilo => {
            if answer.said.is_none() {
                let words = if job.leading {
                    "we have you ahead of us and we are following"
                } else if job.led_once {
                    "we have lost sight of you — we are lying here waiting"
                } else {
                    "we are standing in under our own navigation — get ahead of us"
                };
                answer.said = Some(words.to_string());
                voice.across(job, words, true);
            }
        }
        // Everything off her and the way comes off: it takes a ship a while, which is
        // the point of the signal being *instantly*.
        Flag::Lima => {
            brig.set = [0.0, 0.0, 0.0];
            brig.bound_for = brig.at;
            if answer.said.is_none() {
                answer.said = Some("taking the way off her".into());
                voice.across(job, "all aback — we are taking the way off her", true);
            }
        }
        // Hold: topsail to the mast and she stays where she is, and this is *not* losing
        // the pilot — she was told to wait, so she does not complain about it.
        Flag::Xray => {
            brig.set = [0.0, 0.35, 0.0];
            brig.bound_for = brig.at + DVec2::new(brig.heading.sin(), brig.heading.cos()) * 150.0;
            if answer.said.is_none() {
                answer.said = Some("holding on, watching you".into());
                voice.across(job, "holding on — we are watching your signals", true);
            }
        }
        // **Come round, and steady there.** The flag says what the pilot boat is doing
        // and the ship does the same thing a moment later, which is how a vessel is
        // conned from another one. Thirty degrees a hoist: a brig wears round in
        // minutes, so an order that turned her ninety would be an order she spent a
        // quarter of an hour on — and two hoists is how you get ninety if you want it.
        Flag::Echo | Flag::India => {
            if answer.said.is_none() {
                let side = if matches!(flag, Flag::Echo) {
                    1.0
                } else {
                    -1.0
                };
                let course = wrap_pi(brig.heading + side * ORDER_TURNS);
                job.ordered = Some(course);
                let words = if side > 0.0 {
                    "coming round to starboard"
                } else {
                    "coming round to port"
                };
                answer.said = Some(words.to_string());
                voice.across(job, words, true);
            }
        }
        // And on again.
        Flag::Papa => {
            brig.set = [1.0, 1.0, 1.0];
            // Carrying on is the end of an order to steer: she goes back to following
            // the water you are taking her through.
            job.ordered = None;
            if answer.said.is_none() {
                answer.said = Some("making sail".into());
                voice.across(job, "making sail — we are coming on", true);
            }
        }
        // Danger: she sheers away from the nearest shoal rather than from your boat, and
        // keeps her way on while she does it. A ship that stops on a lee shore is a ship
        // that goes onto it.
        Flag::Uniform => {
            // Danger outranks a course she was given, because the bottom outranks
            // everybody.
            job.ordered = None;
            let away = sheer_off(sim, brig.at, brig.heading);
            brig.set = [0.6, 1.0, 0.0];
            brig.bound_for = brig.at + away * 600.0;
            if answer.said.is_none() {
                answer.said = Some("sheering off".into());
                voice.across(job, "sheering off — where is the water?", true);
            }
        }
    }
}

/// Which way is out of it: the heading that gets her off the shoalest bearing within
/// reach, biased to the smaller turn.
///
/// Soundings rather than cleverness. Half a dozen bearings either side of her head, a
/// cast of the lead a cable and a half along each, and she goes where the water is — the
/// same thing a leadsman and a worried master would do, and it is right for the same
/// reason: nobody on that deck knows this coast, which is why they wanted a pilot.
fn sheer_off(sim: &Sim, at: DVec2, heading: f64) -> DVec2 {
    let Some(land) = sim.sim.land.as_ref() else {
        return DVec2::new(heading.sin(), heading.cos());
    };
    let mut best = (f64::MIN, DVec2::new(heading.sin(), heading.cos()));
    for step in -6..=6 {
        let off = rad(15.0) * step as f64;
        let course = heading + off;
        let along = DVec2::new(course.sin(), course.cos());
        // The least water along that bearing, which is what decides it.
        let mut least = f64::MAX;
        for cast in 1..=6 {
            least = least.min(land.depth(at + along * (cast as f64 * 50.0)));
        }
        // A tie goes to the smaller turn: a ship sheering off does not wear round if a
        // point and a half will do.
        let score = least - off.abs() * 2.0;
        if score > best.0 {
            best = (score, along);
        }
    }
    best.1
}

pub fn run(
    time: Res<Time>,
    sim: Res<Sim>,
    mut brig: ResMut<Brig>,
    mut job: ResMut<Pilotage>,
    mut flotsam: ResMut<crate::barrels::Flotsam>,
    mut answer: ResMut<crate::signals::Answer>,
    mut radio: ResMut<crate::radio::Radio>,
) {
    if !job.running || job.over.is_some() {
        return;
    }
    let dt = time.delta_secs_f64();
    let here = sim.sim.state.pos;
    job.brig_at = brig.at;
    let mut voice = Voice {
        at: brig.at,
        range: (brig.at - here).length(),
        now: sim.sim.state.time,
        name: format!("brig {}", brig.name),
        radio: &mut radio,
    };
    // **She calls up until somebody comes to her channel.** A ship standing in for a
    // pilot is on sixteen asking for one; she stops when there is a pilot boat ahead of
    // her, because then she has one.
    if !job.leading && !job.led_once {
        voice.calling_up(&mut job);
    }

    // --- Her deck cargo, going over the side a cask at a time ----------------------
    if !job.leading {
        job.shedding += dt;
        if job.shedding > A_CASK_EVERY {
            job.shedding = 0.0;
            // Off her quarter, where a cask would actually go: it goes over the rail and
            // she sails out from under it.
            let head = DVec2::new(brig.heading.sin(), brig.heading.cos());
            let side = DVec2::new(head.y, -head.x);
            let at = brig.at - head * 12.0 + side * 6.0;
            flotsam.adrift.push(at);
            job.said = Some("another cask off her quarter".into());
        }
    }

    // --- Once aboard, she follows the water you took her into ----------------------
    // **Whether anybody is leading her.** One measurement, taken every frame: a ship
    // follows a boat she can see ahead of her, and stops following one that has dropped
    // astern or out of sight. There is no handshake and nothing to press — being in
    // front of her *is* the act.
    let was_leading = job.leading;
    job.leading = Pilotage::leading_her(&brig, here);
    if job.leading && !was_leading {
        job.said = Some("she has you in sight — she is following".into());
        voice.across(&mut job, "we have you ahead of us — lead on, pilot", false);
    }
    if job.leading {
        job.led_once = true;
    }
    if !job.leading && was_leading {
        voice.across(&mut job, "where are you, pilot — we have lost you", false);
    }
    // **Before anybody has led her she stands on; afterwards she waits.**
    //
    // Both halves are the manoeuvre. She comes in from seaward making for the coast
    // under her own navigation, and that is the chase: she will not wait for a boat
    // that has not caught her yet. But a ship that *has* taken a pilot and then lost
    // sight of him does not carry on into a bay she does not know — she rounds up,
    // hauls up her courses and lies there until he is ahead of her again, which is what
    // every ship waiting off a pilot station is doing.
    //
    // **And what she has set is what the wind and the station say**, rather than
    // everything, always. The wind she is in — hers, at her rig's height — decides how
    // much canvas is on her; how close she is to running the pilot boat down decides how
    // much of it she spills. See `ship::she_carries`: topgallants first, then the
    // courses, and the topsail reefed and reefed again.
    let tws = crate::ship::her_wind(&sim, brig.at).length();
    // How badly she wants to slow: nothing while the gap is comfortable, everything she
    // can spare by the time she is a cable off and still closing.
    let gap = (here - brig.at).length();
    let closing = closing_on(&sim, &brig, here);
    let hurry = if closing > 0.0 {
        ((TOO_NEAR * 2.4 - gap) / (TOO_NEAR * 1.4)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let working = crate::ship::she_carries(tws, hurry);
    if !job.leading {
        if job.led_once {
            // Hove to: her courses up and a topsail to the mast. That is a state, not a
            // trim, so it is written rather than worked out.
            brig.set = [0.0, 0.45, 0.0];
            brig.bound_for = brig.at + DVec2::new(brig.heading.sin(), brig.heading.cos()) * 400.0;
            if was_leading {
                job.said = Some("she is standing off, waiting for you to get ahead".into());
            }
        } else {
            brig.set = working;
            brig.bound_for = job.bound_for;
        }
    } else {
        brig.set = working;
    }

    // **And she says so**, because the deck is where this has to be visible: a ship a
    // cable astern taking in her topgallants is a thing you can see, and a ship doing it
    // because it is coming on to blow is a thing you want to be told. One line when the
    // canvas changes by a step, through the same limiter as everything else she says.
    {
        let step = |x: f32| (x * 3.0).round() as i32;
        // `None` until she has set anything at all: the field began as three noughts,
        // which reads as *she had nothing set a moment ago* and had her announcing that
        // she was setting her topgallants on the first frame of every job.
        let before = job.canvas.map(|had| had.map(step));
        let now = brig.set.map(step);
        if let Some(before) = before.filter(|had| *had != now) {
            let words = match (before[2] > now[2], before[0] > now[0], now != [0, 0, 0]) {
                (true, _, true) => Some("taking in the topgallants"),
                (_, true, true) => Some("the courses are coming up"),
                _ if now[2] > before[2] => Some("setting the topgallants again"),
                _ => None,
            };
            if let Some(words) = words {
                voice.across(&mut job, words, false);
            }
        }
        job.canvas = Some(brig.set);
    }

    // --- And what she has been told, which outranks all of it ----------------------
    //
    // **An order she has read is an order she obeys.** The hoist itself is not enough:
    // `signals::Answer::reading` is what her officer has actually got out of the signal
    // book, and until the answering pendant is close up there is nothing here to act on.
    // That is the whole of why the pendant is worth watching.
    told_her(&mut brig, &mut job, &mut answer, &sim, &mut voice);

    // **A question is not an order.** Only the three signals that say where she is to be
    // take the helm; K asks what she is doing and P tells her to carry on, and gating her
    // steering on *any* flag meant that asking whether she was following stopped her.
    let told = answer
        .reading
        .is_some_and(crate::signals::Flag::takes_the_helm);
    // **An ordered course outranks the wake and nothing else.** She steers it until she
    // is told to carry on, which is what *steady there* means — and `sail_her` still
    // refuses to lie inside her own limit, so an order into the wind is answered by her
    // falling off and her master saying so, rather than by a ship sailing where no
    // square-rigger can.
    if let Some(course) = job.ordered.filter(|_| !matches!(answer.reading, Some(f) if f == crate::signals::Flag::Lima || f == crate::signals::Flag::Xray)) {
        brig.bound_for = brig.at + DVec2::new(course.sin(), course.cos()) * 3_000.0;
    } else if job.leading && !told {
        // **A track, not a tow-rope.** She steers at where the pilot boat *was*, not at
        // where it is: a ship two cables astern aiming at the boat herself cuts every
        // corner you just went round, which on this coast is the corner you went round
        // for a reason.
        if job.wake.last().is_none_or(|p| (*p - here).length() > 25.0) {
            job.wake.push(here);
        }
        // The oldest mark she has not reached yet is the one she steers for, and it is
        // dropped when she gets to it. Written first as *the oldest mark more than two
        // cables off*, which is the same thing right up until she overruns the track —
        // and then it is a mark **astern** of her, and she turns round and sails back
        // down her own wake looking for it.
        while job.wake.len() > 1 && (job.wake[0] - brig.at).length() < 60.0 {
            job.wake.remove(0);
        }
        brig.bound_for = job.wake.first().copied().unwrap_or(here);

        // --- What her master makes of how he is being led --------------------------
        //
        // Everything here is measured off her own sailing rather than invented: the
        // water under her, the course the next mark would put her on against the closest
        // she can lie, how sharp the track turns, and where the pilot boat is.
        let now = sim.sim.state.time;
        if job.hail.as_ref().is_some_and(|(_, until)| now > *until) {
            job.hail = None;
        }
        {
            let head = DVec2::new(brig.heading.sin(), brig.heading.cos());
            let (under, ahead) = sim
                .sim
                .land
                .as_ref()
                .map(|l| (l.depth(brig.at), l.depth(brig.at + head * 320.0)))
                .unwrap_or((99.0, 99.0));
            // Her wind, at her rig's height, because the hail is about what *she*
            // can do with it.
            let wind = crate::ship::her_wind(&sim, brig.at);
            let from = sail_core::wind::direction_of(wind);
            let to_mark = brig.bound_for - brig.at;
            let twa_wanted = crate::ship::angle_off(to_mark.x.atan2(to_mark.y), from);
            // How sharply the track she is on is about to bend: the angle between the
            // leg she is running now and the one after it.
            let turn = if job.wake.len() > 2 {
                let a = job.wake[1] - job.wake[0];
                let b = job.wake[2] - job.wake[1];
                wrap_pi(b.x.atan2(b.y) - a.x.atan2(a.y)).abs()
            } else {
                0.0
            };
            let gap = (here - brig.at).length();
            // Whether that gap is closing, which is what turns "near" into "about to be
            // run down".
            let closing = closing_on(&sim, &brig, here);
            if let Some(words) = remark(under, ahead, twa_wanted, turn, gap, closing) {
                voice.across(&mut job, words, false);
            }
        }

        // --- The bottom, which is the whole point of the job -----------------------
        if let Some(land) = sim.sim.land.as_ref() {
            let under = land.depth(brig.at);
            job.least_water = job.least_water.min(under);
            if under < SHE_DRAWS {
                job.over = Some(Outcome::Aground {
                    least: job.least_water,
                });
                brig.speed = 0.0;
                job.said = Some("she is on the bottom".into());
                return;
            }
            // **In, and brought up.** The job ends where the ship was going: inside the
            // anchorage, with her way off enough to round up and let go. Not "in deep
            // water", which is where she started.
            let in_the_bay = (brig.at - job.bound_for).length() < ARRIVED;
            if in_the_bay {
                job.over = Some(Outcome::Delivered {
                    minutes: (sim.sim.state.time - job.began) / 60.0,
                    casks: flotsam.landed + flotsam.astern.len() as u32,
                    least: job.least_water,
                });
                brig.set = [0.0, 0.0, 0.0];
                job.said = Some("she is in — her anchor is on the bottom".into());
            }
        }
    } else if (brig.at - here).length() > 4_000.0 {
        // She has stood on past you. Nothing is lost but the fee.
        job.over = Some(Outcome::MissedHer);
        job.said = Some("she is hull down — you were too slow".into());
    }
}

/// The two halves of flag H, in our own rigging.
#[derive(Component)]
pub struct PilotFlag;

/// Hoisted while the job is on, struck when she is let go.
pub fn fly_the_flag(job: Res<Pilotage>, mut flag: Query<&mut Visibility, With<PilotFlag>>) {
    let flying = job.running && job.over.is_none();
    for mut show in &mut flag {
        *show = if flying {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Where a ship her size is let go: the nearest water to the anchorage that she can lie
/// in and get out of.
///
/// **The anchorage is not it.** Measured, which is how this was caught: every harbour in
/// this archipelago carries 5.6 m, she draws 3.9, and the route search wants six and a
/// third — so the way in could never reach the harbour dot and the last leg was drawn to
/// it through water nothing had checked. A pilot does not park a ship anyway; he takes
/// her to where she can round up and let go, and that is a place with water under her.
pub fn where_she_is_let_go(sim: &Sim, bound_for: DVec2) -> DVec2 {
    let Some(land) = sim.sim.land.as_ref() else {
        return bound_for;
    };
    let mut best = (bound_for, f64::INFINITY);
    for ring in 1..=8 {
        let r = ring as f64 * 200.0;
        for step in 0..24 {
            let b = step as f64 / 24.0 * std::f64::consts::TAU;
            let at = bound_for + DVec2::new(b.sin(), b.cos()) * r;
            if land.depth(at) > SHE_DRAWS + CLEARANCE * 2.0 && r < best.1 {
                best = (at, r);
            }
        }
        if best.1.is_finite() {
            break;
        }
    }
    best.0
}

/// Whether she is clear: off the anchorage, in water that needs nobody.
///
/// Both halves matter. Deep water two miles from where she is going is the sea she came
/// in from; the anchorage with a reef still between her and it is not clear water. The
/// job ends where the pilot's usefulness does.
pub fn clear_of_it(sim: &Sim, brig: &Brig, let_go_at: DVec2) -> bool {
    let Some(land) = sim.sim.land.as_ref() else {
        return false;
    };
    let head = DVec2::new(brig.heading.sin(), brig.heading.cos());
    let there = (brig.at - let_go_at).length() < ARRIVED;
    let under = land.depth(brig.at) > SHE_DRAWS + CLEARANCE * 2.0;
    let ahead =
        (1..=5).all(|i| land.depth(brig.at + head * (i as f64 * 120.0)) > SHE_DRAWS + CLEARANCE);
    there && under && ahead
}

/// `Enter` lets her go: the pilot flag comes down and the job is done.
///
/// **Nobody is transferred and nothing is handed over.** The flag is on our shrouds
/// because the responsibility is ours, and taking it down is the act that says we are
/// finished with her — which on this coast is done exactly like that, within hail and
/// without anybody stepping anywhere.
pub fn discharge(
    keys: Res<ButtonInput<KeyCode>>,
    sim: Res<Sim>,
    brig: Res<Brig>,
    mut job: ResMut<Pilotage>,
    hook: Res<crate::boathook::Boathook>,
    flotsam: Res<crate::barrels::Flotsam>,
    sound: Res<crate::sound::SoundHandle>,
) {
    if !job.running || job.over.is_some() || !keys.just_pressed(KeyCode::Enter) {
        return;
    }
    // The boathook has first call on the key: with something alongside to be caught,
    // catching it is what `Enter` means. One key, and whichever job is in front of you.
    if hook.on.is_some() {
        return;
    }
    if !clear_of_it(&sim, &brig, job.let_go_at) {
        job.said = Some("not yet — she is not in clear water off the anchorage".into());
        sound.0.strike_damped(140.0, 0.2, 0.9);
        return;
    }
    job.over = Some(Outcome::Delivered {
        minutes: (sim.sim.state.time - job.began) / 60.0,
        casks: flotsam.landed + flotsam.astern.len() as u32,
        least: job.least_water,
    });
    job.said = Some("pilot flag down — she has sea room and you are discharged".into());
    sound.0.strike_damped(520.0, 0.3, 0.7);
    info!("pilotage finished");
}

#[cfg(test)]
mod what_her_master_says {
    use super::*;

    /// **In the order a seaman would say them**, and every one of them measured off her
    /// own sailing rather than invented.
    ///
    /// The water first, because that is the one that puts her on the bottom; then the
    /// course, because a square-rigger led inside her own limit does not pinch, she
    /// *stops*, and a stopped ship in a narrow place is the whole disaster; then the
    /// turn; then the station.
    #[test]
    fn she_says_the_thing_that_matters_most_first() {
        let deep = 30.0;
        let fair = rad(110.0);
        // Nothing wrong: nothing said. A ship that comments on a good passage is a
        // ship nobody listens to by the second headland.
        assert_eq!(remark(deep, deep, fair, 0.0, 400.0, 0.0), None);
        // Shoal water ahead outranks everything else that is wrong at the same time.
        let all_wrong = remark(deep, SHE_DRAWS + 1.0, rad(40.0), rad(80.0), 2_000.0, 0.0);
        assert!(all_wrong.unwrap().contains("shoaling"));
        // A course she cannot lie.
        assert!(remark(deep, deep, rad(40.0), 0.0, 400.0, 0.0)
            .unwrap()
            .contains("cannot lie"));
        // And that she *can* — sixty-five degrees is the rig's limit, so a hair outside
        // it is a course and not a complaint.
        assert_eq!(
            remark(deep, deep, CLOSEST + rad(20.0), 0.0, 400.0, 0.0),
            None
        );
        // A corner too fine for three hundred tons.
        assert!(remark(deep, deep, fair, rad(70.0), 400.0, 0.0)
            .unwrap()
            .contains("too fine a turn"));
        // Station: too far is running away, too near and closing is about to be run
        // down — and too near while *opening* is just a pilot boat getting on with it.
        assert!(remark(deep, deep, fair, 0.0, TOO_FAR + 50.0, 0.0)
            .unwrap()
            .contains("drawing away"));
        assert!(remark(deep, deep, fair, 0.0, 60.0, kn_to_ms(2.0))
            .unwrap()
            .contains("coming up on you"));
        assert_eq!(remark(deep, deep, fair, 0.0, 60.0, -kn_to_ms(2.0)), None);
    }

    /// And she is not a parrot: one hail at a time, and a gap between them.
    #[test]
    fn she_does_not_say_it_twice_in_the_same_breath() {
        let mut job = Pilotage::default();
        job.call_across("first", 100.0);
        job.call_across("second", 102.0);
        assert_eq!(
            job.hail.as_ref().map(|(w, _)| w.as_str()),
            Some("first"),
            "she talked over herself"
        );
        job.call_across("third", 100.0 + BETWEEN_HAILS + 1.0);
        assert_eq!(job.hail.as_ref().map(|(w, _)| w.as_str()), Some("third"));
    }
}

#[cfg(test)]
mod leading_her {
    use super::*;

    fn brig_heading_north() -> Brig {
        Brig {
            at: DVec2::ZERO,
            heading: 0.0,
            speed: kn_to_ms(6.0),
            afloat: true,
            ..Default::default()
        }
    }

    /// **A ship follows a boat she can see ahead of her**, and that is the whole of the
    /// mechanic: no transfer, no handshake, nothing to press. Being in front of her is
    /// the act.
    #[test]
    fn she_follows_what_is_ahead_and_nothing_else() {
        let brig = brig_heading_north();
        // Two cables ahead: leading her.
        assert!(Pilotage::leading_her(&brig, DVec2::new(0.0, 370.0)));
        // Out on her bow, still in the sector.
        assert!(Pilotage::leading_her(&brig, DVec2::new(200.0, 300.0)));
        // Abeam and abaft it: not leading anybody.
        assert!(!Pilotage::leading_her(&brig, DVec2::new(400.0, 0.0)));
        assert!(!Pilotage::leading_her(&brig, DVec2::new(0.0, -200.0)));
        // Hull down ahead: her master is steering at a shape on the horizon, which is
        // not pilotage.
        assert!(!Pilotage::leading_her(
            &brig,
            DVec2::new(0.0, IN_SIGHT + 200.0)
        ));
        // And right under her bowsprit, which is not leading either — it is about to be
        // run down.
        assert!(!Pilotage::leading_her(&brig, DVec2::new(0.0, 12.0)));
    }
}

/// Asked for from the menu: a flag rather than a call, because the page that offers the
/// job has no business knowing where the sea is deep.
#[derive(Resource, Default)]
pub struct WantsPilotage {
    pub start: bool,
    pub stop: bool,
}

/// Where to put her, and where she is trying to get to.
///
/// **The job has to be a real one or it is a fetch quest.** So her destination is an
/// anchorage that is actually inside the islands, she starts from open water outside
/// them, and the line between the two has something in the way — which is the whole
/// reason anybody would pay a pilot. The search is for the *shallowest* water on the
/// direct line, because that is the hazard the job is about; a passage with nothing on
/// it is a passage that does not need you.
pub fn find_her_a_job(sim: &Sim, from: DVec2) -> Option<(DVec2, DVec2)> {
    let land = sim.sim.land.as_ref()?;
    let harbours = land.harbours();
    // An anchorage far enough off to be a passage and near enough to be tonight's work.
    let bound_for = harbours
        .iter()
        .map(|h| h.at)
        .filter(|at| {
            let off = (*at - from).length();
            (1_500.0..9_000.0).contains(&off)
        })
        .min_by(|a, b| (*a - from).length().total_cmp(&(*b - from).length()))?;

    // **Where she is when you take the job**, which is a question about being seen and
    // not about the geometry of the coast.
    //
    // It was written the other way round — five kilometres seaward of the anchorage,
    // along whichever bearing had the most water — and that is correct about where a
    // ship comes from and useless about where she *is*: measured between three and a
    // half and seven and a half kilometres from the boat, past the camera's far plane at
    // the top of that, over the horizon at all of it, and three quarters of an hour of
    // sailing. Reported from the deck as *I take the job and there is no brig*, which is
    // exactly what there was.
    //
    // So the search is round the *pilot boat* instead: a ring at about a mile, which is
    // where a ship waiting for a pilot stands — hull down with her rig showing, which is
    // what a pilot looks for and what the bearing and range on the deck line are for.
    // The coast still has its say, as two conditions rather than as the objective: she
    // must be in water she can lie in, and the way from her to the anchorage must be
    // water she could have come in by. Among the places that satisfy both, the deepest
    // channel wins.
    const IN_SIGHT_AT: f64 = 1_700.0;
    // The rings are tried from the inside out and the first one that works wins: a ship
    // in sight beats a deeper channel two miles away, and an empty sea beats neither.
    // Some days the water round the boat simply does not allow it, and then she is where
    // she can be — hull down with a bearing on the deck line, which is a worse start and
    // still a start.
    let mut best: Option<(DVec2, f64)> = None;
    for reach in [
        IN_SIGHT_AT * 0.75,
        IN_SIGHT_AT,
        IN_SIGHT_AT * 1.25,
        IN_SIGHT_AT * 1.7,
        IN_SIGHT_AT * 2.2,
    ] {
        for step in 0..36 {
            let bearing = step as f64 / 36.0 * std::f64::consts::TAU;
            let dir = DVec2::new(bearing.sin(), bearing.cos());
            let at = from + dir * reach;
            // Outside the place she is going, or there is nothing left to pilot.
            // Water she can lie in, which is not the same as the offing's eighteen
            // metres: she is waiting off a coast, not hove to in the Atlantic.
            if (at - bound_for).length() < 1_200.0 || land.depth(at) < SHE_DRAWS + CLEARANCE * 2.0 {
                continue;
            }
            // And the water between her and it: the worst of it is the pinch the job is
            // about, and it has to be water a ship could be taken through at all.
            // Stopping short of the anchorage itself, which is shoal by definition —
            // that is what makes it an anchorage, and the same exemption the route
            // search needed. What is being asked is whether she could have come in,
            // not whether she could anchor in six inches.
            let run = (at - bound_for).length() - ARRIVED;
            let legs = (run / 200.0).ceil().max(1.0) as i32;
            let least = (0..=legs)
                .map(|i| {
                    let along = i as f64 / legs as f64 * run;
                    land.depth(at.lerp(bound_for, along / (at - bound_for).length()))
                })
                .fold(f64::INFINITY, f64::min);
            if least < SHE_DRAWS + CLEARANCE {
                continue;
            }
            if best.as_ref().is_none_or(|(_, had)| least > *had) {
                best = Some((at, least));
            }
        }
        if best.is_some() {
            break;
        }
    }
    best.map(|(at, _)| (at, bound_for))
}

/// Start and stop the job from the menu.
pub fn take_the_job(
    sim: Res<Sim>,
    mut wants: ResMut<WantsPilotage>,
    mut job: ResMut<Pilotage>,
    mut brig: ResMut<Brig>,
    mut flotsam: ResMut<crate::barrels::Flotsam>,
    mut first: Local<bool>,
) {
    // `SAIL_BRIG=400` puts her four hundred metres away with the job already running.
    // The same kind of knob as `SAIL_SQUALL`: a ship five kilometres off takes twenty
    // minutes to reach, and you cannot look at a thing often enough to get it right if
    // every look costs twenty minutes.
    if !*first {
        *first = true;
        if crate::knob::var("SAIL_BRIG").is_ok() {
            wants.start = true;
        }
    }
    if wants.stop {
        wants.stop = false;
        *job = Pilotage::default();
        brig.afloat = false;
    }
    if !std::mem::take(&mut wants.start) {
        return;
    }
    let Some((offing, bound_for)) = find_her_a_job(&sim, sim.sim.state.pos) else {
        job.said = Some("no anchorage near enough to pilot anybody into".into());
        return;
    };
    let offing = match crate::knob::var("SAIL_BRIG")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
    {
        Some(range) if range > 0.0 => {
            // Dead ahead of wherever she is pointing, because the knob exists to be
            // *looked* at and a ship astern of the camera teaches nobody anything.
            let here = sim.sim.state.pos;
            // Forty degrees off the bow, so that our own mainsail is not standing in
            // front of the thing the knob exists to look at.
            let head = sim.sim.state.heading + rad(40.0);
            here + DVec2::new(head.sin(), head.cos()) * range
        }
        _ => offing,
    };
    let let_go_at = where_she_is_let_go(&sim, bound_for);
    job.begin(&mut brig, offing, bound_for, let_go_at, sim.sim.state.time);
    // Her deck cargo has been working loose since the gale: the first few are already
    // in the water astern of her.
    let head = DVec2::new(brig.heading.sin(), brig.heading.cos());
    for i in 1..4 {
        flotsam.adrift.push(brig.at - head * (260.0 * i as f64));
    }
}

/// What the deck is told about her: one line, where every other prompt in this game is.
pub fn draw(
    mut contexts: bevy_egui::EguiContexts,
    job: Res<Pilotage>,
    brig: Res<Brig>,
    sim: Res<Sim>,
    chart: Res<crate::chart::Chart>,
) -> Result {
    use bevy_egui::egui;
    if !job.running || chart.spread {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let screen = ctx.viewport_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("pilotage"),
    ));
    let middle = screen.center().x;
    let top = screen.top() + 92.0;

    // Where she is and what she is doing, which is what a pilot boat looks at all the
    // way out: range, her bearing, and the water under her once she is yours.
    let off = brig.at - sim.sim.state.pos;
    let range = off.length();
    let bearing = off.x.atan2(off.y).rem_euclid(std::f64::consts::TAU);
    let line = if let Some(over) = &job.over {
        over.words()
    } else if job.leading {
        let under = sim
            .sim
            .land
            .as_ref()
            .map(|l| l.depth(brig.at))
            .unwrap_or(99.0);
        format!(
            "the brig is {:.0} m astern — {:.1} m under her, she draws {:.1}",
            range, under, SHE_DRAWS
        )
    } else {
        format!(
            "the brig bears {:03.0}, {:.1} nm — she wants a pilot",
            bearing.to_degrees(),
            range / 1852.0
        )
    };
    painter.text(
        egui::pos2(middle, top),
        egui::Align2::CENTER_CENTER,
        line,
        egui::FontId::monospace(14.0),
        if job.over.is_some() {
            egui::Color32::from_rgb(232, 176, 58)
        } else {
            egui::Color32::from_rgb(214, 210, 198)
        },
    );
    // And what her master has just called across, in his own words and under the line
    // that says where she is: a hail is a thing somebody said, so it is in quotes and it
    // is not an instrument reading.
    if let Some((words, _)) = &job.hail {
        painter.text(
            egui::pos2(middle, top + 40.0),
            egui::Align2::CENTER_CENTER,
            format!("\u{201c}{words}\u{201d}"),
            egui::FontId::monospace(15.0),
            egui::Color32::from_rgb(232, 176, 58),
        );
    }

    // And the one instruction that matters right now: either she is clear and you may
    // let her go, or she is not and the line says what is still in the way.
    if job.over.is_none() {
        let clear = clear_of_it(&sim, &brig, job.bound_for);
        let words = if clear {
            Some(("ENTER — let her go, pilot flag down", true))
        } else if !job.leading {
            Some(("get ahead of her — she follows what she can see", false))
        } else {
            None
        };
        if let Some((words, ready)) = words {
            painter.text(
                egui::pos2(middle, top + 20.0),
                egui::Align2::CENTER_CENTER,
                words,
                egui::FontId::monospace(if ready { 17.0 } else { 13.0 }),
                if ready {
                    egui::Color32::from_rgb(150, 214, 158)
                } else {
                    egui::Color32::from_rgb(176, 170, 156)
                },
            );
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------
// The water she can be taken through, worked out and drawn on the chart.
// ---------------------------------------------------------------------------------------

/// How much she wants under her keel before a pilot will call it water, metres.
///
/// **Two metres under a four-metre draft, and it is not caution, it is swell.** A metre
/// of sea on a shoal is half a metre less water in the trough of it, and a chart's
/// soundings are a survey rather than a promise. A pilot who takes a loaded ship over
/// her own draft plus nothing is a pilot once.
const CLEARANCE: f64 = 2.4;

/// The grid the route is searched on, metres.
///
/// A hundred and twenty is about her own length: finer than that is a plot of water she
/// cannot steer accurately enough to use, and coarser misses a gap between two skerries
/// that is a real gap.
const CELL: f64 = 120.0;

/// And how far out the search will go round the straight line, in cells.
const MARGIN: i32 = 14;

/// The route, in sim coordinates, from her to the anchorage — or nothing, if there is no
/// water she can be taken through at all.
///
/// **This is the pilot's own knowledge, written down.** A search over the soundings for
/// the cheapest way in, where cheap means deep, away from the edges, and *sailable by a
/// square-rigged ship* — because a route that asks her to lie closer than sixty-five
/// degrees to the wind is not a route, it is a drawing. The wind is an argument to the
/// function for exactly that reason: the water does not change with the weather and the
/// way through it does.
pub fn the_way_in(sim: &Sim, from: DVec2, to: DVec2, wind_from: f64) -> Option<Vec<DVec2>> {
    let land = sim.sim.land.as_ref()?;
    let deep_enough = |at: DVec2| land.depth(at) > SHE_DRAWS + CLEARANCE;
    // The grid is laid along the line between the two, so that it is a corridor rather
    // than a square: the search is for a way *in*, not a survey of the county.
    let along = to - from;
    let len = along.length();
    if len < CELL {
        return Some(vec![to]);
    }
    let ahead = along / len;
    let across = DVec2::new(ahead.y, -ahead.x);
    let cells = (len / CELL).ceil() as i32 + 2;
    let at = |i: i32, j: i32| from + ahead * (i as f64 * CELL) + across * (j as f64 * CELL);
    let index = |i: i32, j: i32| ((i + 1) * (2 * MARGIN + 1) + (j + MARGIN)) as usize;
    let size = index(cells, MARGIN) + 1;

    // Dijkstra, because the costs are not a metric and A* with a bad heuristic is a
    // slower Dijkstra with a bug in it. Six thousand cells is nothing.
    let mut cost = vec![f64::INFINITY; size];
    let mut came: Vec<Option<(i32, i32)>> = vec![None; size];
    let mut queue = std::collections::BinaryHeap::new();
    cost[index(0, 0)] = 0.0;
    // `Reverse` of a bit-pattern-ordered float, which is how a float goes into a binary
    // heap without a wrapper type that pretends to be `Ord` about NaN.
    queue.push(std::cmp::Reverse((0i64, 0i32, 0i32)));
    let mut best: Option<(i32, i32)> = None;
    while let Some(std::cmp::Reverse((_, i, j))) = queue.pop() {
        if i >= cells - 1 {
            best = Some((i, j));
            break;
        }
        let here = cost[index(i, j)];
        for di in 0..=1 {
            for dj in -1..=1 {
                if di == 0 && dj == 0 {
                    continue;
                }
                let (ni, nj) = (i + di, j + dj);
                if ni < 0 || ni > cells || nj.abs() > MARGIN {
                    continue;
                }
                let next = at(ni, nj);
                if !deep_enough(next) {
                    continue;
                }
                let step = (next - at(i, j)).length();
                // Deep water in the middle of the channel is worth going round for: a
                // leg that shaves a skerry is a leg that has no room for a wind shift.
                let under = land.depth(next);
                let edge = 1.0 + 2.5 / (1.0 + (under - SHE_DRAWS - CLEARANCE).max(0.0));
                // And a leg she cannot sail costs what it would cost her: she would
                // have to work up to it in boards, which is three times the distance
                // and an afternoon.
                let leg = (next - at(i, j)).normalize_or_zero();
                let twa = crate::ship::angle_off(leg.x.atan2(leg.y), wind_from);
                let sailable = if twa.abs() < CLOSEST { 3.5 } else { 1.0 };
                let c = here + step * edge * sailable;
                if c < cost[index(ni, nj)] {
                    cost[index(ni, nj)] = c;
                    came[index(ni, nj)] = Some((i, j));
                    queue.push(std::cmp::Reverse(((c * 1000.0) as i64, ni, nj)));
                }
            }
        }
    }
    let mut node = best?;
    let mut back = vec![to];
    while let Some(prev) = came[index(node.0, node.1)] {
        back.push(at(node.0, node.1));
        node = prev;
    }
    back.reverse();
    // Straighten it: a route is the four or five legs somebody steers, not every cell
    // the search walked through. A leg stands as long as the water under the straight
    // line between its ends is water.
    let mut legs: Vec<DVec2> = vec![back[0]];
    let mut i = 0;
    while i < back.len() - 1 {
        let mut j = back.len() - 1;
        while j > i + 1 {
            let clear = (1..20).all(|k| {
                let t = k as f64 / 20.0;
                deep_enough(back[i].lerp(back[j], t))
            });
            if clear {
                break;
            }
            j -= 1;
        }
        legs.push(back[j]);
        i = j;
    }
    Some(legs)
}

/// Work it out again when the wind has moved or the chart has been opened on a new one.
///
/// Once every half minute, because the search is cheap and the wind is not constant —
/// and because a route that was right twenty minutes ago is exactly the kind of advice
/// that puts a ship ashore.
pub fn lay_off_the_route(sim: Res<Sim>, mut job: ResMut<Pilotage>, time: Res<Time>) {
    if !job.running || job.over.is_some() {
        job.route.clear();
        return;
    }
    job.laid_off += time.delta_secs_f64();
    if job.laid_off < 30.0 && !job.route.is_empty() {
        return;
    }
    job.laid_off = 0.0;
    let wind = crate::ship::her_wind(&sim, job.brig_at);
    let from = sail_core::wind::direction_of(wind);
    // From where she is, because what is behind her is nobody's problem now.
    let brig_at = job.brig_at;
    let let_go_at = job.let_go_at;
    job.route = the_way_in(&sim, brig_at, let_go_at, from).unwrap_or_default();
}

#[cfg(test)]
mod the_pilots_water {
    use super::*;

    /// **Every metre of the route is water she can float in.**
    ///
    /// Which is the only promise a pilot makes, and the one thing a drawn line on a
    /// chart can be wrong about in a way that costs a ship. The legs are straightened
    /// after the search, so the test walks the straight line between the marks rather
    /// than the cells the search went through — if the simplification ever cuts a
    /// corner across a shoal, this is what says so.
    #[test]
    fn the_way_in_is_water_she_can_float_in() {
        let mut found = 0;
        for seed in 0..6u64 {
            let sim = Sim::new(crate::weather::Weather::draw(seed), seed);
            let Some(land) = sim.sim.land.as_ref() else {
                continue;
            };
            let here = sim.sim.state.pos;
            let Some((offing, bound_for)) = find_her_a_job(&sim, here) else {
                continue;
            };
            let let_go_at = where_she_is_let_go(&sim, bound_for);
            assert!(
                land.depth(let_go_at) > SHE_DRAWS + CLEARANCE,
                "seed {seed}: she is let go in {:.1} m",
                land.depth(let_go_at)
            );
            let Some(route) = the_way_in(&sim, offing, let_go_at, 0.0) else {
                continue;
            };
            found += 1;
            assert!(route.len() >= 2, "a route with nothing in it");
            assert!(
                (route[route.len() - 1] - let_go_at).length() < 1.0,
                "the way in does not end where she is let go"
            );
            for pair in route.windows(2) {
                let steps = ((pair[1] - pair[0]).length() / 40.0).ceil() as usize;
                for k in 0..=steps {
                    let at = pair[0].lerp(pair[1], k as f64 / steps as f64);
                    // **No exemption for the last leg.** It used to skip everything
                    // within three cables of the anchorage, which is exactly the water
                    // the search could not reach — every harbour here carries 5.6 m and
                    // the search wants 6.3 — so the approach was drawn to the harbour
                    // dot through soundings nothing had looked at, and the test said it
                    // was fine. Now the route ends where she is let go, and the whole of
                    // it is checked.
                    let under = land.depth(at);
                    assert!(
                        under > SHE_DRAWS,
                        "seed {seed}: the way in crosses {under:.1} m and she draws \
                         {SHE_DRAWS:.1}"
                    );
                }
            }
        }
        assert!(found > 0, "no seed produced a pilotage job to check");
    }
}

#[cfg(test)]
mod can_the_job_even_end {
    use super::*;

    /// **The job has to be finishable**, which is a thing to measure rather than hope:
    /// there must be water off every anchorage that she can be left in.
    #[test]
    fn there_is_water_off_every_anchorage_to_let_her_go_in() {
        for seed in 0..6u64 {
            let sim = Sim::new(crate::weather::Weather::draw(seed), seed);
            let Some(land) = sim.sim.land.as_ref() else {
                continue;
            };
            let here = sim.sim.state.pos;
            let Some((_, bound_for)) = find_her_a_job(&sim, here) else {
                continue;
            };
            let let_go_at = where_she_is_let_go(&sim, bound_for);
            let under = land.depth(let_go_at);
            assert!(
                under > SHE_DRAWS + CLEARANCE,
                "seed {seed}: the only water off the anchorage is {under:.1} m"
            );
            assert!(
                (let_go_at - bound_for).length() < ARRIVED * 4.0,
                "seed {seed}: she would be left {:.0} m from where she was going",
                (let_go_at - bound_for).length()
            );
        }
    }

    /// PROBE: how deep is it round an anchorage.
    #[test]
    #[ignore]
    fn probe_clear_water() {
        for seed in 0..6u64 {
            let sim = Sim::new(crate::weather::Weather::draw(seed), seed);
            let Some(land) = sim.sim.land.as_ref() else {
                continue;
            };
            let here = sim.sim.state.pos;
            let Some((_, bound_for)) = find_her_a_job(&sim, here) else {
                continue;
            };
            let at_harbour = land.depth(bound_for);
            let mut deepest: f64 = 0.0;
            let mut best_at = bound_for;
            for step in 0..24 {
                let b = step as f64 / 24.0 * std::f64::consts::TAU;
                for r in [400.0, 800.0, 1_200.0, 1_600.0] {
                    let at = bound_for + DVec2::new(b.sin(), b.cos()) * r;
                    let d = land.depth(at);
                    if d > deepest && r < ARRIVED * 3.0 {
                        deepest = d;
                        best_at = at;
                    }
                }
            }
            println!(
                "seed {seed}: harbour {at_harbour:.1} m, deepest within {:.0} m is {deepest:.1} m \
                 at {:.0} m off",
                ARRIVED * 3.0,
                (best_at - bound_for).length()
            );
        }
    }
}

#[cfg(test)]
mod how_far_off_she_starts {
    use super::*;

    /// **She has to be there when you go out to her.**
    ///
    /// The fault this holds: she was placed five kilometres beyond the anchorage, which
    /// came out between three and a half and seven and a half kilometres from the boat
    /// — past the camera's far plane at the top of that, over the horizon at all of it,
    /// and three quarters of an hour of sailing. Taking the job showed you an empty sea.
    #[test]
    fn she_starts_within_sight_of_the_pilot_boat() {
        let (mut jobs, mut in_sight) = (0, 0);
        for seed in 0..24u64 {
            let sim = Sim::new(crate::weather::Weather::draw(seed), seed);
            let here = sim.sim.state.pos;
            let Some((offing, bound_for)) = find_her_a_job(&sim, here) else {
                continue;
            };
            jobs += 1;
            let range = (offing - here).length();
            // Never over the horizon. This is the fault: five kilometres seaward of the
            // anchorage measured out at three and a half to seven and a half from the
            // boat, and taking the job showed an empty sea.
            assert!(
                range < 3_800.0,
                "seed {seed}: she starts {:.1} km off, which is a sail nobody can see",
                range / 1000.0
            );
            if range < 2_200.0 {
                in_sight += 1;
            }
            // And still outside the place she is going, or there is nothing to pilot.
            assert!(
                (offing - bound_for).length() > 1_100.0,
                "seed {seed}: she starts inside her own destination"
            );
        }
        assert!(jobs > 0, "no seed produced a job to check");
        // Most days she is a mile off; some days the water round the boat does not allow
        // it and she is further, which is a worse start and still a start.
        assert!(
            in_sight * 4 >= jobs * 3,
            "only {in_sight} of {jobs} starts were inside a mile and a quarter"
        );
    }

    /// PROBE: where does she actually appear, relative to the boat.
    #[test]
    #[ignore]
    fn probe_start_range() {
        for seed in 28..44u64 {
            let sim = Sim::new(crate::weather::Weather::draw(seed), seed);
            let here = sim.sim.state.pos;
            let Some((offing, bound_for)) = find_her_a_job(&sim, here) else {
                println!("seed {seed}: no job at all");
                continue;
            };
            println!(
                "seed {seed}: harbour {:.1} km off, she starts {:.1} km from the boat",
                (bound_for - here).length() / 1000.0,
                (offing - here).length() / 1000.0
            );
        }
    }
}
