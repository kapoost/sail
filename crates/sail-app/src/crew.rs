//! The watch on deck: a crew that works the sails while you steer.
//!
//! **What this is not is an autopilot for the sails.** The pilot steers because a tiller
//! pilot is a box you can buy and bolt to the deck. There is no box that trims a boat —
//! what trims a boat is people, and that is exactly the point of having them: you take
//! the helm, you say where you want to go, and somebody else is on the winches. It is
//! how a boat with more than one person aboard is sailed, and until now this one was
//! sailed single-handed whether you wanted to be or not.
//!
//! So the crew works like a crew. They haul at the pace a winch gives, they take one
//! line at a time off the clutch bank because there is one winch to take it on, and they
//! do the jobs in the order a crew does: the sheets continuously, because the sheets have
//! their own winches and somebody's hands are always on them, and the shape controls when
//! one of them has drifted far enough from where it should be to be worth the trip.
//!
//! They also do the two things a crew does without being asked. They back the headsail
//! when she is in irons, because that is the only way out and nobody waits to be told.
//! And they reef when she is lying over too far for too long — not on a gust, on a
//! *while*, which is the difference between reefing and flinching.

use bevy::prelude::*;
use sail_core::autopilot::{optimal_sheets_bent, OVERPOWERED_HEEL};
use sail_core::sail::outhaul_for;
use sail_core::units::{ms_to_kn, rad, smoothstep};

use crate::clutch::{Bank, Line, BANK};
use crate::orders::Orders;
use crate::sim::Sim;

/// How fast the hands haul a sheet, fraction of its travel per second.
///
/// The same pace the five-second trim assist uses, because it is the same pair of hands
/// on the same winch. A crew is not a servo and a sheet that snaps to its answer is the
/// difference between a crew and a cheat.
const HAUL: f64 = 0.45 * 2.2;

/// How much of the mainsheet's travel the watch will spill before it reefs.
///
/// **A crew eases before it reefs**, and the order is not a preference: a slab is twenty
/// minutes of work and a sheet is a second, so the first answer to being over on her ear
/// is to let the mainsheet go and the top of the sail twist open. Measured why it has to
/// be here at all: trimming both sails to their best angle of attack whatever the heel
/// is, the watch reefed, re-trimmed to maximum lift, lay over again and reefed again —
/// in nineteen and a half knots it had the main *handed* inside half an hour, which is
/// storm canvas for a Force 5. The reef rule was not wrong; it was being fed by a trim
/// rule that would not give an inch.
///
/// A third of the travel, which is enough to take ten degrees off her and not so much
/// that she stops going anywhere.
const SPILLS: f64 = 0.35;

/// How long she has to be lying over before anybody reefs, seconds.
///
/// **Reefing is a decision about the next hour, not about this gust.** Twelve seconds of
/// being over on her ear is weather; two seconds of it is a puff, and a crew that shortens
/// sail for a puff spends the afternoon shaking reefs out again.
const PRESSED_FOR: f64 = 12.0;

/// And how long she has to be standing up before they shake one out, seconds.
///
/// Three minutes, which is fifteen times the other number, and deliberately. Putting a
/// reef in is cheap and taking it out is a job, so the bias is towards leaving it in —
/// what every cruising crew that has ever been rained on does. Measured why it has to be
/// this long: at forty seconds the watch put reefs in and shook them out five times in
/// half an hour in a steady nineteen knots, which is a crew nobody would sail with.
const EASY_FOR: f64 = 180.0;

/// How far off a shape control has to be before it is worth going to the winch.
///
/// A tenth of its travel. Less than that and the crew would live at the clutch bank
/// chasing a number, which is both wrong about sailing and visibly silly.
const WORTH_THE_TRIP: f64 = 0.10;

/// And how close is close enough to close the clutch again.
///
/// **The threshold decides whether you go; it does not decide where you stop.** With one
/// number doing both jobs every line on the bank sat up to a tenth of its travel away
/// from where it should be — the vang and the outhaul permanently a little bit wrong,
/// which is the one thing a crew is there to prevent. You put the rope on the drum, you
/// work it home, and then you close the clutch.
const HOME: f64 = 0.01;

/// How long the conditions have to ask for a different headsail before anybody goes
/// forward, seconds.
///
/// **A sail change is twenty minutes of work and a wet foredeck.** Nobody does one
/// because the wind went aft for half a minute, and a crew that did would spend the
/// passage changing sails and none of it sailing. Forty seconds of the same answer.
const BEFORE_A_CHANGE: f64 = 40.0;

/// How much of what a sail is built for the crew will actually carry, as a fraction of
/// its own cloth limit.
///
/// Three quarters, and the margin is the point: a kite that blows out at twenty-two
/// knots apparent is a kite that comes down at sixteen, because the gust that takes it
/// is the one that has not arrived yet. Cloth is the one thing aboard with no second
/// chance.
const CARRIES_TO: f64 = 0.72;

/// The hands, and whether they are on deck.
#[derive(Resource, Default)]
pub struct Crew {
    /// Whether the watch is working the sails.
    pub standing: bool,
    /// What they are on at this moment, for the plate to say.
    pub on: Option<&'static str>,
    /// Seconds she has been over too far, and seconds she has been easy.
    pressed: f64,
    easy: f64,
    /// And how long the conditions have been asking for that sail.
    asking: f64,
    asked_for: Option<sail_core::state::Headsail>,
    /// The line they have on the winch, kept until it is home.
    ///
    /// Which rope is on the drum is a fact about the deck and not a thing to re-derive
    /// every frame: a crew that re-chose forty times a second would put a line on,
    /// bring it within a tenth of where it belongs, find another line further out and
    /// walk away from the first one — leaving every control a little bit wrong for ever.
    on_the_winch: Option<Line>,
    /// The wind as somebody deciding about sails sees it: a running average rather than
    /// this instant.
    ///
    /// **This is not a smoothing hack, it is the difference between a decision and a
    /// twitch.** Running at a hundred and twenty degrees in a seaway the apparent angle
    /// swings thirty degrees every time she rolls, and a wardrobe chosen off the
    /// instantaneous number changed its mind between the gennaker, the Code 0 and the
    /// genoa four times a minute — so the dwell timer never got anywhere and nothing was
    /// ever set. Measured in the headless boat before it was written: want Gennaker,
    /// want Code0, want Genoa, want Gennaker, ten seconds apart.
    ///
    /// A crew looks at where the wind has been for the last minute. Twenty seconds of
    /// time constant is that, and it is deliberately far longer than the sheets', which
    /// answer the wind that is here now.
    settled: Option<(f64, f64, f64)>,
}

impl Crew {
    /// **A hand on anything stands them down**, and it is one function because every
    /// place that touches a rope has to do the same thing.
    ///
    /// Reported from the deck as the staysail that would not come down: it was coming
    /// down, and the watch was hoisting it again inside a second, because a crew that
    /// has an opinion about the staysail and no way of being overruled is not a crew —
    /// it is an argument. Overruling it is the same act as taking a sheet: you did
    /// something with your own hands, so the hands on deck are stood down until you
    /// call them again. A new watch may set whatever it likes; this one has been told.
    pub fn stand_down(&mut self, what: &str) {
        if !self.standing {
            return;
        }
        self.standing = false;
        self.on = None;
        info!("you took {what} — watch stood down");
    }

    /// What the instrument says they are doing.
    pub fn doing(&self) -> &'static str {
        match (self.standing, self.on) {
            (false, _) => "you have her",
            (true, Some(job)) => job,
            (true, None) => "on the sheets",
        }
    }
}

/// Where the reef wants to be, as one of the stops on the pennant.
///
/// Heel, because heel is the pressure gauge every sailor on every boat actually reads,
/// and because it is the one that already decides the outhaul. Hysteresis with real
/// dwell either side of it: she has to mean it.
fn shorten(crew: &mut Crew, heel: f64, dt: f64, at: f64, spilled: bool) -> Option<f64> {
    const SLABS: [f64; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];
    // Over *and* out of sheet: the slab is for the wind that the mainsheet could not
    // answer. Without the second half of that condition the watch reefs its way down to
    // bare poles in a Force 5, because every slab is followed by a trim that stands her
    // straight back up on her ear.
    let over = spilled && heel.abs() > OVERPOWERED_HEEL + rad(4.0);
    let easy = heel.abs() < OVERPOWERED_HEEL - rad(7.0);
    crew.pressed = if over { crew.pressed + dt } else { 0.0 };
    crew.easy = if easy { crew.easy + dt } else { 0.0 };
    if crew.pressed > PRESSED_FOR {
        crew.pressed = 0.0;
        return SLABS.iter().copied().find(|s| *s > at + 1e-6);
    }
    if crew.easy > EASY_FOR {
        crew.easy = 0.0;
        return SLABS.iter().rev().copied().find(|s| *s < at - 1e-6);
    }
    None
}

/// Which sail belongs on the bow in this wind, at this angle.
///
/// **The point of sail is read off the *true* wind and the sail's own limits off the
/// apparent one**, and that is not pedantry: the point of sail is where you are going
/// relative to the weather and does not move with boat speed, while whether a sail fills
/// and whether its cloth survives are both questions about the air actually going over
/// it. Mixing those up is how a Code 0 ends up set on a run and a kite on a beat.
///
/// The ladder runs from the deep running sail inwards, and the first one that will both
/// fill and survive gets the job. The genoa is the bottom of it and always available:
/// it is the sail that is still there when everything else has been put back in its bag.
pub fn wardrobe(
    cfg: &sail_core::boat::BoatConfig,
    twa: f64,
    awa: f64,
    aws: f64,
) -> sail_core::state::Headsail {
    use sail_core::state::Headsail::*;
    let off = twa.abs();
    // **A Code 0 is not a running sail and a spinnaker is not a reaching one**, which
    // is why this is three short lists and not one long one sorted by size. Each list
    // holds only the sails that belong at that angle at all; the cloth limit then
    // decides which of them the day will allow, and the genoa catches everything that
    // falls through.
    let ladder: &[sail_core::state::Headsail] = if off > rad(150.0) {
        &[Spinnaker, Gennaker]
    } else if off > rad(115.0) {
        &[Gennaker, Spinnaker]
    } else if off > rad(72.0) {
        &[Code0, Gennaker]
    } else {
        // Upwind there is one sail and it is on the stay. Everything else aboard is cut
        // for a wind that is not forward of the beam.
        return Genoa;
    };
    for kind in ladder.iter().copied() {
        // Will it stay full at this apparent angle, with a margin so that one luff of
        // the helm does not collapse it — and is the cloth within what it is built for.
        let fills = awa.abs() > kind.fills_above() + rad(6.0);
        let holds = aws < cfg.headsail(kind).limit_aws * CARRIES_TO;
        if fills && holds {
            return kind;
        }
    }
    Genoa
}

/// Where each line on the bank ought to be with the wind where it is.
///
/// One function because a crew does not have a different opinion in two places, and
/// because the test below reads the same numbers the deck does.
pub fn wants(
    line: Line,
    c: &sail_core::state::Controls,
    awa: f64,
    heel: f64,
    aws: f64,
    changing: bool,
) -> f64 {
    let off = awa.abs();
    match line {
        Line::Outhaul => outhaul_for(awa, heel, OVERPOWERED_HEEL),
        // **The vang does nothing upwind and everything off the wind.** Close-hauled the
        // mainsheet is pulling the boom down as much as in, so the leech looks after
        // itself; eased out on a reach the sheet is pulling sideways only, and without
        // the vang the top of the sail twists open and spills everything it has.
        Line::Vang => 0.35 + 0.50 * smoothstep(rad(55.0), rad(120.0), off),
        // The car is the genoa's outhaul and vang in one fitting: aft to flatten the
        // sail and open its leech when she is pressed, forward for depth when she is
        // not. Upwind in a breeze it goes aft; reaching it comes forward to put shape
        // back in the sail and close the foot.
        Line::GenoaCar => {
            let upwind = 1.0 - smoothstep(rad(50.0), rad(95.0), off);
            let breeze = smoothstep(10.0, 22.0, ms_to_kn(aws));
            (0.20 + 0.45 * upwind + 0.30 * breeze).clamp(0.0, 1.0)
        }
        // **Rolled right away while a change is going on**, because that is the first
        // act of one: you cannot bend a new sail on with the old one drawing. Otherwise
        // the genoa comes in with the main, which is how a rig stays balanced as it is
        // shortened down — a bit rolled away with each slab, rather than sailing her
        // under a full headsail and a handkerchief of main.
        Line::Furl if changing => 1.0,
        Line::Furl if c.headsail.flying() => 0.0,
        Line::Furl => (c.reef * 0.8).clamp(0.0, 0.95),
        // **The staysail is the heavy-weather sail and the slot-filler**, and the two
        // conditions it goes up in are the same condition: a rig that has been shortened
        // down. Two slabs in the main, or it blowing hard enough that it is about to be,
        // and it is hoisted — a small sail low down and inboard, which is the one that
        // keeps her driving when the genoa has been rolled to a scrap.
        Line::Staysail => {
            let shortened = c.reef >= 0.45;
            let blowing = ms_to_kn(aws) > 26.0;
            // Not under a kite: the staysail stands in the middle of the foretriangle
            // and a spinnaker sheet comes across it.
            let kite = matches!(
                c.headsail,
                sail_core::state::Headsail::Gennaker | sail_core::state::Headsail::Spinnaker
            );
            if (shortened || blowing) && !kite {
                1.0
            } else {
                0.0
            }
        }
        // Its club boom finds its own angle; the tackle on the end is the only shape
        // control it has. Hauled upwind, eased as the boom goes out — the same argument
        // as the vang, on a smaller sail.
        Line::StaysailSheet => 0.75 - 0.45 * smoothstep(rad(60.0), rad(130.0), off),
        // A decision rather than a trim: [`shorten`] owns the reef.
        Line::Reef => line.get(c),
    }
}

/// The watch, working.
///
/// The order is a crew's order: look at the sky first — is the right sail even on the
/// bow — then the thing that cannot wait, then the sheets, then the gear.
pub fn work(
    mut sim: ResMut<Sim>,
    mut bank: ResMut<Bank>,
    mut crew: ResMut<Crew>,
    mut orders: ResMut<Orders>,
    time: Res<Time>,
) {
    if !crew.standing {
        crew.on = None;
        crew.pressed = 0.0;
        crew.easy = 0.0;
        crew.asking = 0.0;
        crew.settled = None;
        crew.on_the_winch = None;
        return;
    }
    let dt = time.delta_secs_f64();
    let cfg = sim.config();
    let t = sim.sim.telemetry;
    let (awa, heel, aws) = (t.awa, sim.sim.state.heel, t.aws);
    crew.on = None;

    // --- The wardrobe: is the right sail even on the bow ---------------------------
    //
    // A change is a job with stages, so it is decided once and then carried out: roll
    // the old sail away, bend the new one on, set it. The decision has to hold for most
    // of a minute first, because the alternative is a foredeck crew that never gets to
    // sit down.
    // What somebody deciding about sails is looking at: the wind of the last minute
    // rather than of this wave. See [`Crew::settled`].
    let settled = {
        let k = 1.0 - (-dt / 20.0).exp();
        let (mut twa, mut a, mut w) = crew.settled.unwrap_or((t.twa.abs(), awa.abs(), aws));
        twa += (t.twa.abs() - twa) * k;
        a += (awa.abs() - a) * k;
        w += (aws - w) * k;
        crew.settled = Some((twa, a, w));
        (twa, a, w)
    };
    let want_sail = wardrobe(&cfg, settled.0, settled.1, settled.2);
    if !orders.changing() {
        if want_sail == sim.controls.headsail {
            crew.asking = 0.0;
            crew.asked_for = None;
        } else {
            if crew.asked_for != Some(want_sail) {
                crew.asked_for = Some(want_sail);
                crew.asking = 0.0;
            }
            crew.asking += dt;
            // Cloth that is already past what it is built for does not get to wait out
            // the dwell: coming down is the one sail change nobody argues about.
            let urgent = aws > cfg.headsail(sim.controls.headsail).limit_aws * CARRIES_TO;
            if crew.asking > BEFORE_A_CHANGE || urgent {
                orders.headsail = Some(want_sail);
                crew.asking = 0.0;
                info!(
                    "crew: {} coming down, {} going up",
                    sim.controls.headsail.name(),
                    want_sail.name()
                );
            }
        }
    }
    let changing = orders.changing();

    // --- In irons, back the headsail ------------------------------------------------
    //
    // Nobody waits to be told: she is stopped, head to wind, with the rudder doing
    // nothing because there is no water going over it, and the only thing aboard that
    // will push her bow off is a sail held on the wrong side. A crew does it before the
    // skipper has finished swearing.
    let in_irons = t.twa.abs() < rad(28.0) && ms_to_kn(t.stw) < 1.6;
    let genoa = sim.controls.headsail == sail_core::state::Headsail::Genoa;
    if in_irons && genoa && sim.controls.furl < 0.9 && !changing {
        sim.controls.headsail_aback = true;
        crew.on = Some("backing the headsail");
    } else if sim.controls.headsail_aback && !in_irons {
        // And let it across the moment she has paid off and is sailing again, which is
        // the other half of the manoeuvre and the half that gets forgotten.
        sim.controls.headsail_aback = false;
    }

    // --- The preventer, before the run and not during the gybe ---------------------
    //
    // The one line aboard that exists to stop something happening. A crew rigs it when
    // she settles down to run and takes it off before anybody means to gybe, and the
    // angles are deliberately far apart so that it is not being rigged and unrigged
    // every time she yaws on a wave.
    let running = t.twa.abs() > rad(150.0);
    if running && !sim.controls.preventer {
        sim.controls.preventer = true;
        info!("crew: preventer on — she is running");
    } else if t.twa.abs() < rad(128.0) && sim.controls.preventer {
        sim.controls.preventer = false;
        info!("crew: preventer off");
    }

    // --- The sheets, continuously, on their own winches ----------------------------
    //
    // Trimmed to the wind each sail actually meets rather than to the masthead fly,
    // which is what the bend terms are.
    // **Eased past the best trim when she is overpowered**, which is the first answer to
    // heel and the one that costs nothing: the mainsheet goes, the top of the sail twists
    // open, and the heeling moment comes off before the drive does. The main first and
    // the headsail hardly at all — the main is up high with a long lever and it is the
    // sail that spills cleanly.
    let spill = SPILLS * smoothstep(OVERPOWERED_HEEL, OVERPOWERED_HEEL + rad(9.0), heel.abs());
    if !sim.controls.headsail_aback {
        let (m, j) = optimal_sheets_bent(
            &cfg,
            awa,
            sim.controls.outhaul,
            sim.controls.headsail,
            t.bend_main,
            t.bend_jib,
        );
        let (m, j) = ((m + spill).min(1.0), (j + spill * 0.25).min(1.0));
        let haul = (HAUL * dt).min(1.0);
        sim.controls.main_sheet += (m - sim.controls.main_sheet) * haul;
        sim.controls.jib_sheet += (j - sim.controls.jib_sheet) * haul;
    }

    // --- The reef, which is a decision rather than a trim ---------------------------
    //
    // It goes to the bank as a destination the way the player's own press does: one
    // winch, one job, and it takes as long as it takes.
    let at = sim.controls.reef;
    // Out of sheet: what she is still carrying at this heel is not something the trim
    // can take off her any more.
    let spilled = spill > SPILLS * 0.9;
    if let Some(stop) = shorten(&mut crew, heel, dt, at, spilled) {
        let which = BANK.iter().position(|l| *l == Line::Reef).unwrap_or(3);
        bank.target[which] = Some(stop);
        crew.on = Some(if stop > at {
            "putting a reef in"
        } else {
            "shaking a reef out"
        });
        info!(
            "{}",
            if stop > at {
                "crew: she is over too far — taking a reef in"
            } else {
                "crew: she has come up — shaking a reef out"
            }
        );
    }

    // --- And the gear, one line at a time -------------------------------------------
    //
    // Because there is one winch on the coachroof and this crew is subject to the same
    // rule the player is. Whichever line is furthest from where it should be gets the
    // hands; a line with stops is given a destination and left to run there.
    // --- The foredeck, if a change was decided on above -----------------------------
    //
    // It sits *here*, in the gear section, and not up with the decision — which is where
    // it was written first, with a `return` under it. That return meant that for the
    // quarter of a minute a change takes, nobody was on the sheets and nobody would have
    // backed the headsail if she had gone into irons: the whole crew had apparently
    // walked forward. The sheets are on their own winches and somebody's hands never
    // leave them. What a sail change actually monopolises is the one winch on the
    // coachroof, which is exactly what this section is.
    // The change itself is not written here. It is one sequence — roll the old sail
    // away, bend the new one on with the bow clear, set it — and it belongs to whoever
    // asked for it rather than to whoever is doing it, so the watch asks the same way
    // you do. Two copies of a sail change is two sail changes that disagree.
    //
    // And it is not *called* here either, because then it would be called twice in a
    // frame — once by the standing order's own system and once by the watch — and the
    // drum would turn at double speed with the hands on deck. The watch asks; one
    // system does. What the watch owes the change is the rest of the deck leaving it
    // alone: one winch, one job.
    if orders.changing() {
        crew.on = Some("changing the headsail");
        return;
    }
    if bank.target.iter().any(|t| t.is_some()) {
        if crew.on.is_none() {
            crew.on = Some("at the clutch bank");
        }
        return;
    }
    // The rope already on the drum keeps the hands until it is home; only when the
    // clutch has been closed again is there a choice to make.
    let mut worst: Option<(Line, f64, f64)> = crew.on_the_winch.and_then(|line| {
        let want = wants(line, &sim.controls, awa, heel, aws, changing);
        let off = (want - line.get(&sim.controls)).abs();
        (off > HOME && line.rigged_for(&sim.controls)).then_some((line, want, off))
    });
    if worst.is_none() {
        crew.on_the_winch = None;
        for line in BANK {
            if !line.rigged_for(&sim.controls) || line == Line::Reef {
                continue;
            }
            let want = wants(line, &sim.controls, awa, heel, aws, changing);
            let off = (want - line.get(&sim.controls)).abs();
            if off > WORTH_THE_TRIP && worst.as_ref().is_none_or(|(_, _, had)| off > *had) {
                worst = Some((line, want, off));
            }
        }
        crew.on_the_winch = worst.map(|(line, _, _)| line);
    }
    let Some((line, want, _)) = worst else {
        if crew.on.is_none() {
            crew.on = Some("on the sheets");
        }
        return;
    };
    if let Some(stops) = line.detents(&sim.controls) {
        // A line with positions is not dragged to a number: it is sent to the nearest
        // stop, which for a halyard or a sail in its bag is up or down and nothing else.
        let stop = stops
            .iter()
            .copied()
            .min_by(|a, b| (a - want).abs().total_cmp(&(b - want).abs()))
            .unwrap_or(want);
        let which = BANK.iter().position(|l| *l == line).unwrap_or(0);
        bank.target[which] = Some(stop);
    } else {
        let now = line.get(&sim.controls);
        let step = line.rate(&sim.controls) * dt;
        let next = if (want - now).abs() <= step {
            want
        } else {
            now + step * (want - now).signum()
        };
        line.set_from_ui(&mut sim.controls, next);
    }
    if crew.on.is_none() {
        crew.on = Some(match line {
            Line::Outhaul => "on the outhaul",
            Line::Vang => "on the vang",
            Line::GenoaCar => "on the car",
            Line::Furl => "on the furling line",
            Line::Staysail => "setting the staysail",
            Line::StaysailSheet => "on the staysail sheet",
            Line::Reef => "on the reef pennant",
        });
    }
}

#[cfg(test)]
mod the_watch {
    use super::*;
    use sail_core::state::Controls;
    use sail_core::units::kn_to_ms;

    /// **A crew that reefs for a gust is a crew nobody wants.**
    ///
    /// Twelve seconds of being over on her ear is weather and gets a slab; two seconds
    /// of it is a puff and gets nothing.
    #[test]
    fn a_puff_is_not_a_reason_to_shorten_sail() {
        let mut crew = Crew::default();
        let hard = OVERPOWERED_HEEL + rad(9.0);
        // Two seconds of it.
        for _ in 0..40 {
            assert!(shorten(&mut crew, hard, 0.05, 0.0, true).is_none());
        }
        // And then she stays there.
        let mut took = None;
        for _ in 0..400 {
            if let Some(stop) = shorten(&mut crew, hard, 0.05, 0.0, true) {
                took = Some(stop);
                break;
            }
        }
        assert_eq!(took, Some(0.25), "twelve seconds over should take a slab");
    }

    /// And it comes out again when she stands up — but not in a hurry, because putting
    /// a reef in is cheap and taking one out is a job.
    #[test]
    fn a_reef_comes_out_slower_than_it_went_in() {
        let mut crew = Crew::default();
        let easy = OVERPOWERED_HEEL - rad(12.0);
        let mut seconds = 0.0;
        let mut shook = None;
        for _ in 0..8_000 {
            seconds += 0.05;
            if let Some(stop) = shorten(&mut crew, easy, 0.05, 0.5, true) {
                shook = Some(stop);
                break;
            }
        }
        assert_eq!(shook, Some(0.25));
        assert!(
            seconds > PRESSED_FOR,
            "a reef came out after {seconds:.0} s, faster than one goes in"
        );
    }

    /// **Eased before reefed, and no slab until the sheet has been given away.**
    ///
    /// The two halves of the fault that had the watch reefing its way to bare poles in a
    /// Force 5: it trimmed to maximum lift whatever the heel was, so every slab was
    /// followed by a trim that stood her straight back up on her ear.
    #[test]
    fn she_is_eased_before_she_is_reefed() {
        let mut crew = Crew::default();
        let hard = OVERPOWERED_HEEL + rad(9.0);
        // On her ear with the sheet still in hand: the answer is the sheet, not a reef,
        // however long it goes on for.
        for _ in 0..2_000 {
            assert!(
                shorten(&mut crew, hard, 0.05, 0.0, false).is_none(),
                "a slab was taken with the mainsheet still hard in"
            );
        }
        // And once there is no sheet left to give, the slab comes.
        let mut took = None;
        for _ in 0..600 {
            if let Some(stop) = shorten(&mut crew, hard, 0.05, 0.0, true) {
                took = Some(stop);
                break;
            }
        }
        assert_eq!(took, Some(0.25));
    }

    /// The vang is the one that is easiest to get backwards: it does nothing upwind,
    /// where the mainsheet is already pulling the boom down, and it is everything on a
    /// reach, where the sheet pulls sideways only.
    #[test]
    fn the_vang_comes_on_as_the_boom_goes_out() {
        let c = Controls::default();
        let close = wants(Line::Vang, &c, rad(30.0), 0.0, 8.0, false);
        let reach = wants(Line::Vang, &c, rad(100.0), 0.0, 8.0, false);
        let run = wants(Line::Vang, &c, rad(160.0), 0.0, 8.0, false);
        assert!(reach > close + 0.2, "{close:.2} close, {reach:.2} reaching");
        assert!(run >= reach - 1e-6);
        assert!((0.0..=1.0).contains(&run));
    }

    /// **The ladder of sails, and what keeps a crew off the foredeck.**
    ///
    /// Upwind there is one sail and it is on the stay; off the wind the deepest sail
    /// that will both stay full and survive the breeze gets the job; and when it is
    /// blowing, everything cut for light air stays in its bag whatever the angle is.
    #[test]
    fn the_right_sail_for_the_angle_and_the_breeze() {
        use sail_core::state::Headsail::*;
        let cfg = sail_core::boat::BoatConfig::charter_sloop_34();
        let light = kn_to_ms(9.0);
        assert_eq!(
            wardrobe(&cfg, rad(40.0), rad(28.0), light),
            Genoa,
            "beating"
        );
        assert_eq!(
            wardrobe(&cfg, rad(90.0), rad(62.0), light),
            Code0,
            "a beam reach in nine knots is the one condition a Code 0 exists for"
        );
        assert_eq!(wardrobe(&cfg, rad(130.0), rad(95.0), light), Gennaker);
        assert_eq!(wardrobe(&cfg, rad(170.0), rad(150.0), light), Spinnaker);
        // And it blowing: a kite that blows out at twenty-two knots apparent comes down
        // well before twenty-two, because the gust that takes it is the one that has not
        // arrived yet.
        assert_eq!(
            wardrobe(&cfg, rad(170.0), rad(150.0), kn_to_ms(19.0)),
            Genoa,
            "nineteen knots over the deck is no place for half-ounce cloth"
        );
        // Nor is a kite any use with the apparent wind too far forward, whatever the
        // true angle says: the luff falls in and it flogs in the lee of the main. With
        // nothing on the running list able to stand, the sail on the stay gets it —
        // which is what the genoa is for and why it is the bottom of every ladder.
        assert_eq!(wardrobe(&cfg, rad(155.0), rad(40.0), light), Genoa);
    }

    /// The staysail is the heavy-weather sail: up when the rig has been shortened down,
    /// in its bag when it has not, and never under a kite that would sheet across it.
    #[test]
    fn the_staysail_goes_up_when_she_is_shortened_down() {
        let mut c = Controls::default();
        assert_eq!(wants(Line::Staysail, &c, rad(40.0), 0.0, 8.0, false), 0.0);
        c.reef = 0.5;
        assert_eq!(wants(Line::Staysail, &c, rad(40.0), 0.0, 8.0, false), 1.0);
        c.headsail = sail_core::state::Headsail::Spinnaker;
        assert_eq!(
            wants(Line::Staysail, &c, rad(160.0), 0.0, 8.0, false),
            0.0,
            "a spinnaker sheet comes across the foretriangle the staysail stands in"
        );
    }

    /// And the genoa comes in with the main, which is what keeps a shortened rig
    /// balanced instead of sailing her under a full headsail and a handkerchief.
    #[test]
    fn the_headsail_is_rolled_away_with_the_reefs() {
        let mut c = Controls::default();
        let full = wants(Line::Furl, &c, rad(40.0), 0.0, 10.0, false);
        c.reef = 0.75;
        let deep = wants(Line::Furl, &c, rad(40.0), 0.0, 10.0, false);
        assert!(full < 0.05);
        assert!(
            deep > 0.5,
            "three slabs in and the genoa is still {deep:.2}"
        );
    }
}
