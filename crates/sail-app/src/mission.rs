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

use crate::ship::Brig;
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
}

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
        // A breadcrumb every so often, which is what she is actually steering at: not
        // where you *are* — a ship two cables astern aiming at the pilot boat cuts every
        // corner you just went round — but where you *were*.
        if job.wake.last().is_none_or(|p| (*p - here).length() > 25.0) {
            job.wake.push(here);
        }
        // The oldest crumb still more than her following distance ahead of her.
        let target = job
            .wake
            .iter()
            .find(|p| (**p - brig.at).length() > ASTERN * 0.55)
            .copied()
            .unwrap_or(here);
        brig.bound_for = target;
        job.wake.retain(|p| (*p - brig.at).length() > 20.0);

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
