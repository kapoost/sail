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
//!    have to sail your own boat better than she is sailing hers, and get *ahead* — a
//!    pilot boards from in front and from to leeward, because coming up under the stern
//!    of a moving ship is how a small boat is run down.
//! 2. **Board her**, which here is `Enter` within hailing distance. The flag that goes up
//!    at her foremast head is H — *I have a pilot on board* — and from that moment her
//!    going aground is your doing.
//! 3. **Take her through.** She follows your wake at a ship's pace, and she draws nearly
//!    four metres where you draw two: the water that is nothing to you is the bottom to
//!    her. What the mission is *about* is that difference.
//!
//! And before any of it, there are her barrels. A deck cargo that has broken its lashings
//! in the gale she has just come through goes over the side a cask at a time — which is
//! why they are in the water at all, and why collecting them is on the way to the job
//! rather than instead of it.

use bevy::prelude::*;
use glam::DVec2;
use sail_core::units::{kn_to_ms, ms_to_kn, rad, wrap_pi};

use crate::ship::{Brig, CLOSEST};
use crate::sim::Sim;

/// How far off she will take a pilot aboard, metres.
///
/// **A cable, and from ahead.** A pilot cutter comes alongside a ship that has slowed and
/// hauled up her courses, and the transfer happens in the smooth water under her lee. A
/// hundred and eighty metres is generous for the boarding itself and mean enough that it
/// cannot be done by sailing vaguely in her direction.
pub const HAILING: f64 = 180.0;

/// And she will only take one from a boat that is *ahead* of her: this many degrees of
/// her bow, either side.
///
/// Coming up under the quarter of a moving ship to put somebody aboard is how small boats
/// are run down, and it is the one piece of the manoeuvre that is about where you are
/// rather than how near.
pub const ABAFT_THE_BEAM: f64 = rad(80.0);

/// How far astern of the pilot boat the brig tries to stay, metres.
///
/// She is three hundred tons at six knots: she will not stop, and she cannot turn inside
/// her own length. Two cables is the distance at which what you did thirty seconds ago is
/// what she is doing now — which is the whole feeling of having one behind you.
const ASTERN: f64 = 370.0;

/// What she is carrying under her, below which she is on the bottom.
pub const SHE_DRAWS: f64 = crate::ship::DRAFT as f64;

/// Water she is through and clear in, metres. Past this depth there is nothing to pilot
/// her round and the job is done.
const SAFE_WATER: f64 = 18.0;

/// How often she loses another cask while her lashings are going, seconds.
const A_CASK_EVERY: f64 = 70.0;

/// The state of the job.
#[derive(Resource, Default)]
pub struct Pilotage {
    /// Whether the mission has been taken on at all.
    pub running: bool,
    /// Whether the pilot is aboard her.
    pub aboard: bool,
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
}

/// How long a hail stands on screen, and the least time between two of them, seconds.
const HAIL_STANDS: f64 = 9.0;
const BETWEEN_HAILS: f64 = 11.0;

/// How a piece of pilotage ended.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// Through, in deep water, with the time it took and the casks that were saved.
    Delivered { minutes: f64, casks: u32 },
    /// On the putty. The one failure this job has.
    Aground { least: f64 },
    /// She went by. A pilot who cannot catch a ship has not lost anything except the fee.
    MissedHer,
}

impl Outcome {
    pub fn words(&self) -> String {
        match self {
            Outcome::Delivered { minutes, casks } => {
                format!("she is through — {minutes:.0} minutes, {casks} casks out of the water")
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
    pub fn begin(&mut self, brig: &mut Brig, at: DVec2, bound_for: DVec2) {
        *self = Pilotage {
            running: true,
            least_water: f64::INFINITY,
            ..Default::default()
        };
        brig.afloat = true;
        brig.at = at;
        brig.bound_for = bound_for;
        let off = bound_for - at;
        brig.heading = off.x.atan2(off.y);
        brig.speed = kn_to_ms(5.0);
        brig.set = [1.0, 1.0, 1.0];
        self.said = Some("a brig standing in from seaward, wanting a pilot".into());
    }

    /// Whether a boat here, now, could put a pilot aboard her.
    ///
    /// One function because the prompt on screen, the key that takes the job and the test
    /// all have to agree about it — the oldest fault in this repository is the drawn
    /// answer and the acted answer being two different measurements.
    pub fn can_board(&self, brig: &Brig, boat: DVec2, boat_speed: f64) -> Result<(), &'static str> {
        if !self.running || self.aboard {
            return Err("");
        }
        let off = boat - brig.at;
        let range = off.length();
        if range > HAILING {
            return Err("close her first — she will not come to you");
        }
        // Where the boat lies from her bow, which is what decides whether this is a
        // transfer or an accident.
        let relative = wrap_pi(off.x.atan2(off.y) - brig.heading);
        if relative.abs() > ABAFT_THE_BEAM {
            return Err("get ahead of her — nobody boards over a moving stern");
        }
        if ms_to_kn(boat_speed) > 7.0 {
            return Err("take way off — you cannot step across at seven knots");
        }
        Ok(())
    }
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
    // Water, first and always. He is reading his own lead line and he has four metres
    // under him when you have ten.
    if ahead < SHE_DRAWS + 2.5 {
        return Some("shoaling ahead — we draw four metres, pilot");
    }
    if under < SHE_DRAWS + 1.5 {
        return Some("we are feeling the bottom — take us off this");
    }
    // The course he is being led onto. A square-rigger that is asked to lie closer than
    // she can lie does not pinch: she stops, and then she is a ship with no steerage
    // way in a narrow place, which is how this ends badly.
    if twa_wanted.abs() < CLOSEST + rad(6.0) {
        return Some("we cannot lie that close — she will not go there");
    }
    // The turn. Three hundred tons does not go round a corner: she needs a hundred and
    // fifty metres of water to swing in, and asking for less is asking for nothing.
    if turn > rad(55.0) {
        return Some("that is too fine a turn for her — give her room to swing");
    }
    // And the station. Either of these is recoverable; both of them are the pilot
    // forgetting that there is a ship behind him.
    if gap > TOO_FAR {
        return Some("you are running away from us — we cannot hold that");
    }
    if gap < TOO_NEAR && closing > kn_to_ms(1.0) {
        return Some("we are coming up on you — she has no brakes");
    }
    None
}

/// Station: further than this and she has lost you, nearer and she is about to run you
/// down.
const TOO_FAR: f64 = 900.0;
const TOO_NEAR: f64 = 110.0;

impl Pilotage {
    /// Put a hail on the deck, if she has not just said something else.
    fn call_across(&mut self, words: &str, now: f64) {
        if now - self.hailed_at < BETWEEN_HAILS {
            return;
        }
        self.hailed_at = now;
        self.hail = Some((words.to_string(), now + HAIL_STANDS));
        info!("brig: {words}");
    }
}

/// The job, frame by frame: her casks, her following, and the two ways it ends.
pub fn run(
    time: Res<Time>,
    sim: Res<Sim>,
    mut brig: ResMut<Brig>,
    mut job: ResMut<Pilotage>,
    mut flotsam: ResMut<crate::barrels::Flotsam>,
) {
    if !job.running || job.over.is_some() {
        return;
    }
    let dt = time.delta_secs_f64();
    let here = sim.sim.state.pos;

    // --- Her deck cargo, going over the side a cask at a time ----------------------
    if !job.aboard {
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
    if job.aboard {
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
            let wind = sim.sim.wind.sample(brig.at, now);
            let from = sail_core::wind::direction_of(wind);
            let to_mark = brig.bound_for - brig.at;
            let twa_wanted = wrap_pi(to_mark.x.atan2(to_mark.y) - from + std::f64::consts::PI);
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
            let closing = brig.speed - sim.sim.telemetry.sog;
            if let Some(words) = remark(under, ahead, twa_wanted, turn, gap, closing) {
                job.call_across(words, now);
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
            // Clear water on the far side and nothing left to pilot her round.
            if under > SAFE_WATER && (brig.at - here).length() < ASTERN * 2.0 {
                let clear = (0..8).all(|i| {
                    let ahead = brig.at
                        + DVec2::new(brig.heading.sin(), brig.heading.cos()) * (i as f64 * 120.0);
                    land.depth(ahead) > SAFE_WATER
                });
                if clear {
                    job.over = Some(Outcome::Delivered {
                        minutes: sim.sim.state.time / 60.0,
                        casks: flotsam.landed + flotsam.astern.len() as u32,
                    });
                    job.said = Some("she has sea room — you are discharged".into());
                }
            }
        }
    } else if (brig.at - here).length() > 4_000.0 {
        // She has stood on past you. Nothing is lost but the fee.
        job.over = Some(Outcome::MissedHer);
        job.said = Some("she is hull down — you were too slow".into());
    }
}

/// `Enter` puts the pilot aboard, if this is a place and a moment to do it from.
pub fn board(
    keys: Res<ButtonInput<KeyCode>>,
    sim: Res<Sim>,
    brig: Res<Brig>,
    mut job: ResMut<Pilotage>,
    hook: Res<crate::boathook::Boathook>,
    sound: Res<crate::sound::SoundHandle>,
) {
    if !job.running || job.aboard || job.over.is_some() {
        return;
    }
    if !keys.just_pressed(KeyCode::Enter) {
        return;
    }
    // The boathook has first call on the key: if there is something alongside to be
    // caught, catching it is what `Enter` means. One key, and whichever job is actually
    // in front of you.
    if hook.on.is_some() {
        return;
    }
    match job.can_board(&brig, sim.sim.state.pos, sim.sim.telemetry.stw) {
        Ok(()) => {
            job.aboard = true;
            job.wake.clear();
            job.said = Some("aboard — H at her fore truck, and she is yours".into());
            sound.0.strike_damped(520.0, 0.3, 0.7);
            info!("pilot aboard the brig");
        }
        Err("") => {}
        Err(why) => {
            job.said = Some(why.into());
            sound.0.strike_damped(140.0, 0.2, 0.9);
        }
    }
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
            .contains("running away"));
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
mod boarding {
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

    /// **A pilot boards from ahead and from close to, with the way off.**
    ///
    /// All three, and none of them is arbitrary: the range is the length of a heaving
    /// line, the bearing is because nobody steps aboard over a moving stern, and the
    /// speed is because this is a step across a gap and not a jump.
    #[test]
    fn she_takes_a_pilot_from_ahead_and_not_from_anywhere_else() {
        let brig = brig_heading_north();
        let mut job = Pilotage {
            running: true,
            ..Default::default()
        };
        job.running = true;
        // A cable ahead of her, stopped: aboard.
        let ahead = DVec2::new(0.0, 120.0);
        assert!(job.can_board(&brig, ahead, kn_to_ms(3.0)).is_ok());
        // The same place at nine knots: not a transfer, a collision.
        assert!(job.can_board(&brig, ahead, kn_to_ms(9.0)).is_err());
        // Right astern of her, however close: no.
        assert!(job
            .can_board(&brig, DVec2::new(0.0, -100.0), kn_to_ms(2.0))
            .is_err());
        // Half a mile off, dead ahead: no.
        assert!(job
            .can_board(&brig, DVec2::new(0.0, 900.0), kn_to_ms(2.0))
            .is_err());
        // And once somebody is aboard, the question does not arise again.
        job.aboard = true;
        assert!(job.can_board(&brig, ahead, kn_to_ms(3.0)).is_err());
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

    // Out to sea from there: whichever bearing has the most water under it for five
    // kilometres is the one she would have come in on.
    let mut best: Option<(DVec2, f64)> = None;
    for step in 0..24 {
        let bearing = step as f64 / 24.0 * std::f64::consts::TAU;
        let dir = DVec2::new(bearing.sin(), bearing.cos());
        let offing = bound_for + dir * 5_200.0;
        // The worst water anywhere along the way, which is what decides whether a ship
        // could have come in this way at all.
        let least = (1..=26)
            .map(|i| land.depth(bound_for + dir * (i as f64 * 200.0)))
            .fold(f64::INFINITY, f64::min);
        // She has to *start* in deep water; the pinch in the middle is the job.
        if land.depth(offing) > SAFE_WATER && best.as_ref().is_none_or(|(_, had)| least > *had) {
            best = Some((offing, least));
        }
    }
    best.map(|(offing, _)| (offing, bound_for))
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
        if std::env::var("SAIL_BRIG").is_ok() {
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
    let offing = match std::env::var("SAIL_BRIG")
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
    job.begin(&mut brig, offing, bound_for);
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
    } else if job.aboard {
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

    // And the one instruction that matters right now.
    if job.over.is_none() && !job.aboard {
        let hint = match job.can_board(&brig, sim.sim.state.pos, sim.sim.telemetry.stw) {
            Ok(()) => Some(("ENTER — go aboard and take her", true)),
            Err("") => None,
            Err(why) => Some((why, false)),
        };
        if let Some((words, ready)) = hint {
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
