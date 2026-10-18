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
use crate::sim::Sim;

/// How fast the hands haul a sheet, fraction of its travel per second.
///
/// The same pace the five-second trim assist uses, because it is the same pair of hands
/// on the same winch. A crew is not a servo and a sheet that snaps to its answer is the
/// difference between a crew and a cheat.
const HAUL: f64 = 0.45 * 2.2;

/// How long she has to be lying over before anybody reefs, seconds.
///
/// **Reefing is a decision about the next hour, not about this gust.** Twelve seconds of
/// being over on her ear is weather; two seconds of it is a puff, and a crew that shortens
/// sail for a puff spends the afternoon shaking reefs out again.
const PRESSED_FOR: f64 = 12.0;

/// And how long she has to be standing up before they shake one out, seconds.
///
/// Longer, deliberately. Putting a reef in is cheap and taking it out is a job, so the
/// bias is towards leaving it in — which is what every cruising crew that has ever been
/// rained on does.
const EASY_FOR: f64 = 40.0;

/// How far off a shape control has to be before it is worth going to the winch.
///
/// A tenth of its travel. Less than that and the crew would live at the clutch bank
/// chasing a number, which is both wrong about sailing and visibly silly.
const WORTH_THE_TRIP: f64 = 0.10;

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
}

impl Crew {
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
fn shorten(crew: &mut Crew, heel: f64, dt: f64, at: f64) -> Option<f64> {
    const SLABS: [f64; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];
    let over = heel.abs() > OVERPOWERED_HEEL + rad(4.0);
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

/// Where each line on the bank ought to be with the wind where it is.
///
/// One function because a crew does not have a different opinion in two places, and
/// because the test below reads the same numbers the deck does.
pub fn wants(line: Line, c: &sail_core::state::Controls, awa: f64, heel: f64, aws: f64) -> f64 {
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
        // The genoa comes in with the main, which is how a rig stays balanced as it is
        // shortened down: roll a bit away with each slab rather than sailing her under
        // a full headsail and a handkerchief of main.
        Line::Furl => (c.reef * 0.8).clamp(0.0, 0.95),
        // Not theirs to decide. Setting a staysail is a sail change, which is the
        // skipper's call and takes somebody to the foredeck.
        Line::Reef | Line::Staysail | Line::StaysailSheet => line.get(c),
    }
}

/// The watch, working.
pub fn work(mut sim: ResMut<Sim>, mut bank: ResMut<Bank>, mut crew: ResMut<Crew>, time: Res<Time>) {
    if !crew.standing {
        crew.on = None;
        crew.pressed = 0.0;
        crew.easy = 0.0;
        return;
    }
    let dt = time.delta_secs_f64();
    let cfg = sim.config();
    let t = sim.sim.telemetry;
    let (awa, heel, aws) = (t.awa, sim.sim.state.heel, t.aws);

    // **In irons, back the headsail.** Nobody waits to be told: she is stopped, head to
    // wind, with the rudder doing nothing because there is no water going over it, and
    // the only thing aboard that will push her bow off is a sail held on the wrong side.
    // A crew does it before the skipper has finished swearing.
    let in_irons = t.twa.abs() < rad(28.0) && ms_to_kn(t.stw) < 1.6;
    let genoa = sim.controls.headsail == sail_core::state::Headsail::Genoa;
    if in_irons && genoa && sim.controls.furl < 0.9 {
        sim.controls.headsail_aback = true;
        crew.on = Some("backing the headsail");
    } else if sim.controls.headsail_aback && !in_irons {
        // And let it across the moment she has paid off and is sailing again, which is
        // the other half of the manoeuvre and the half that gets forgotten.
        sim.controls.headsail_aback = false;
    }

    // The sheets, continuously, on their own winches — and trimmed to the wind each sail
    // actually meets rather than to the masthead fly, which is what the bend terms are.
    if !sim.controls.headsail_aback {
        let (m, j) = optimal_sheets_bent(
            &cfg,
            awa,
            sim.controls.outhaul,
            sim.controls.headsail,
            t.bend_main,
            t.bend_jib,
        );
        let haul = (HAUL * dt).min(1.0);
        sim.controls.main_sheet += (m - sim.controls.main_sheet) * haul;
        sim.controls.jib_sheet += (j - sim.controls.jib_sheet) * haul;
    }

    // The reef, which is a decision rather than a trim, and goes to the bank as a
    // destination the way the player's own press does: one winch, one job, and it takes
    // as long as it takes.
    let at = sim.controls.reef;
    if let Some(stop) = shorten(&mut crew, heel, dt, at) {
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

    // And the shape controls, one line at a time, because there is one winch and this
    // crew is subject to the same rule the player is. Whichever is furthest from where
    // it should be gets the hands.
    if bank.target.iter().any(|t| t.is_some()) {
        return;
    }
    let mut worst: Option<(Line, f64, f64)> = None;
    for line in BANK {
        if !line.rigged_for(&sim.controls) || line.detents(&sim.controls).is_some() {
            continue;
        }
        if line == Line::StaysailSheet || line == Line::Staysail {
            continue;
        }
        let want = wants(line, &sim.controls, awa, heel, aws);
        let off = (want - line.get(&sim.controls)).abs();
        if off > WORTH_THE_TRIP && worst.as_ref().is_none_or(|(_, _, had)| off > *had) {
            worst = Some((line, want, off));
        }
    }
    if let Some((line, want, _)) = worst {
        let now = line.get(&sim.controls);
        let step = line.rate(&sim.controls) * dt;
        let next = if (want - now).abs() <= step {
            want
        } else {
            now + step * (want - now).signum()
        };
        line.set_from_ui(&mut sim.controls, next);
        if crew.on.is_none() {
            crew.on = Some(match line {
                Line::Outhaul => "on the outhaul",
                Line::Vang => "on the vang",
                Line::GenoaCar => "on the car",
                Line::Furl => "on the furling line",
                _ => "on the sheets",
            });
        }
    } else if crew.on.is_none() {
        crew.on = Some("on the sheets");
    }
}

#[cfg(test)]
mod the_watch {
    use super::*;
    use sail_core::state::Controls;

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
            assert!(shorten(&mut crew, hard, 0.05, 0.0).is_none());
        }
        // And then she stays there.
        let mut took = None;
        for _ in 0..400 {
            if let Some(stop) = shorten(&mut crew, hard, 0.05, 0.0) {
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
        for _ in 0..2000 {
            seconds += 0.05;
            if let Some(stop) = shorten(&mut crew, easy, 0.05, 0.5) {
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

    /// The vang is the one that is easiest to get backwards: it does nothing upwind,
    /// where the mainsheet is already pulling the boom down, and it is everything on a
    /// reach, where the sheet pulls sideways only.
    #[test]
    fn the_vang_comes_on_as_the_boom_goes_out() {
        let c = Controls::default();
        let close = wants(Line::Vang, &c, rad(30.0), 0.0, 8.0);
        let reach = wants(Line::Vang, &c, rad(100.0), 0.0, 8.0);
        let run = wants(Line::Vang, &c, rad(160.0), 0.0, 8.0);
        assert!(reach > close + 0.2, "{close:.2} close, {reach:.2} reaching");
        assert!(run >= reach - 1e-6);
        assert!((0.0..=1.0).contains(&run));
    }

    /// And the genoa comes in with the main, which is what keeps a shortened rig
    /// balanced instead of sailing her under a full headsail and a handkerchief.
    #[test]
    fn the_headsail_is_rolled_away_with_the_reefs() {
        let mut c = Controls::default();
        let full = wants(Line::Furl, &c, rad(40.0), 0.0, 10.0);
        c.reef = 0.75;
        let deep = wants(Line::Furl, &c, rad(40.0), 0.0, 10.0);
        assert!(full < 0.05);
        assert!(
            deep > 0.5,
            "three slabs in and the genoa is still {deep:.2}"
        );
    }
}
