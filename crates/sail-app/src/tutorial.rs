//! The man-overboard drill, taught rather than announced.
//!
//! The first thing a sailing simulator has to do is teach somebody to make the boat go,
//! and the usual way — a wall of text with every key on it — teaches nobody anything.
//! This does it the way an instructor does: one instruction at a time, nothing until the
//! last one has been done, and then something happens that makes the rest of it matter.
//!
//! What happens is that the dan buoy goes over the side. That is not a stunt. Getting a
//! boat going is three keys and two minutes; getting her back to one spot and *stopped*
//! there is the whole of boat handling, and nobody practises it until they need it. So
//! the lesson is: sheet in, bear away, get her moving — and now your pole is a hundred
//! metres astern and going downwind at a tenth of a knot while you are going at six.
//!
//! The coaching in the search is deliberately **advice and not a script**. The drill is
//! over when Oscar is aboard, however that was managed, and the lines change with the
//! situation rather than waiting for the player to perform the manoeuvre in the order the
//! book gives it. A tutorial that refuses to advance until you do the approved thing is
//! not teaching, it is a form to fill in.

use bevy::prelude::*;
use bevy_egui::egui;

use crate::mob::{Drill, Overboard};
use crate::sim::Sim;
use crate::sound::SoundHandle;
use sail_core::units::{deg, ms_to_kn};

/// The keys the lesson tells the player to press.
///
/// Named here as the input layer's own codes and printed from these, so a prompt cannot
/// name a key the lesson does not actually use — and checked against `help::DOCUMENTED`
/// by the test at the bottom, which is the one direction that matters: a lesson asking
/// for a key the program does not read is a lesson that cannot be followed.
const MAINSHEET: KeyCode = KeyCode::KeyW;
const WINCH: KeyCode = KeyCode::KeyE;
const THROW: KeyCode = KeyCode::Enter;

/// What to call a key on screen.
fn key_name(key: KeyCode) -> &'static str {
    match key {
        KeyCode::KeyW => "W",
        KeyCode::KeyE => "E",
        KeyCode::Enter => "ENTER",
        _ => "?",
    }
}

/// Knots she has to be making before anything is allowed to go over the side.
///
/// Four. Enough that she is sailing rather than drifting, and enough that the pole is
/// properly astern of her by the time the alarm is over.
const SAILING: f64 = 4.0;
/// And how long she has to hold it, seconds, which is the countdown.
const HOLD: f64 = 5.0;
/// How long the verdict stays up at the end.
const VERDICT: f64 = 30.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Stage {
    /// Not running. The game is the game.
    #[default]
    Off,
    /// Sheets.
    Trim,
    /// Onto a reach.
    Steer,
    /// Up to speed.
    Build,
    /// Holding it, while the clock runs down.
    Steady,
    /// The pole is in the water and nobody has thrown anything yet.
    Alarm,
    /// Looking for him, with whatever advice fits what she is doing.
    Search,
    /// And the numbers.
    Done,
}

#[derive(Resource, Default)]
pub struct Tutorial {
    pub stage: Stage,
    /// When this stage began, in simulation seconds.
    since: f64,
    /// Set by the settings panel, cleared here the same frame.
    pub wants_start: bool,
    pub wants_stop: bool,
}

impl Tutorial {
    /// A lesson that starts itself on the first frame, which is what `SAIL_TUTORIAL=1`
    /// asks for: the drill gets looked at twenty times in an afternoon of building it,
    /// and clicking through a panel each time is twenty clicks.
    pub fn asked_for(start: bool) -> Self {
        Self {
            wants_start: start,
            ..Self::default()
        }
    }

    pub fn running(&self) -> bool {
        self.stage != Stage::Off
    }
}

/// Run the lesson.
pub fn run(
    sim: Res<Sim>,
    sound: Res<SoundHandle>,
    mut lesson: ResMut<Tutorial>,
    mut over: ResMut<Overboard>,
) {
    let now = sim.sim.state.time;
    if lesson.wants_start {
        lesson.wants_start = false;
        lesson.stage = Stage::Trim;
        lesson.since = now;
        over.drill = Drill::Secure;
    }
    if lesson.wants_stop {
        lesson.wants_stop = false;
        lesson.stage = Stage::Off;
        over.drill = Drill::Secure;
    }
    if !lesson.running() {
        return;
    }

    let tm = &sim.sim.telemetry;
    let controls = &sim.controls;
    let knots = ms_to_kn(tm.stw);
    let held = now - lesson.since;
    let step = |to: Stage, lesson: &mut Tutorial| {
        lesson.stage = to;
        lesson.since = now;
    };

    match lesson.stage {
        Stage::Off => {}
        // Sheets in. Both of them, a little: a boat with her sheets eased sits there
        // whatever you do with the helm, which is the first thing to find out.
        Stage::Trim => {
            if controls.main_sheet < 0.72 && controls.jib_sheet < 0.72 {
                step(Stage::Steer, &mut lesson);
            }
        }
        // Onto a reach. The easiest point of sail to get her going on, and the one a
        // beginner can hold without pinching or running by the lee.
        Stage::Steer => {
            let awa = deg(tm.awa.abs());
            if (45.0..135.0).contains(&awa) {
                step(Stage::Build, &mut lesson);
            }
        }
        Stage::Build => {
            if knots > SAILING {
                step(Stage::Steady, &mut lesson);
            }
        }
        // And now hold it, while the clock runs down to the thing this is all for.
        Stage::Steady => {
            if knots < SAILING - 0.8 {
                step(Stage::Build, &mut lesson);
            } else if held > HOLD {
                let st = &sim.render_state();
                // Over the side at the quarter, where it is racked: astern of her and a
                // little to leeward, which is where a person goes in from the rail.
                let at = st.pos - st.forward() * (sim.config().loa * 0.45)
                    + st.starboard() * tm.awa.signum() * -1.6;
                over.man_overboard(at, now);
                // Three strikes: the bell is the one thing aboard for telling people
                // something, and this is the thing it is for.
                sound.0.ring(3);
                step(Stage::Alarm, &mut lesson);
            }
        }
        Stage::Alarm => {
            let thrown = matches!(over.drill, Drill::Overboard { ring: Some(_), .. });
            // Twenty seconds and then the lesson moves on whether anybody threw anything
            // or not — because that is what happens on the water, and the verdict at the
            // end says so.
            if thrown || held > 20.0 {
                step(Stage::Search, &mut lesson);
            }
        }
        Stage::Search => {
            if matches!(over.drill, Drill::Recovered { .. }) {
                step(Stage::Done, &mut lesson);
            }
        }
        Stage::Done => {
            if held > VERDICT {
                lesson.stage = Stage::Off;
            }
        }
    }
}

/// What to say, which is one instruction at a time and nothing else.
fn instruction(stage: Stage, sim: &Sim, over: &Overboard, held: f64) -> (String, String) {
    let tm = &sim.sim.telemetry;
    let knots = ms_to_kn(tm.stw);
    match stage {
        Stage::Off => (String::new(), String::new()),
        Stage::Trim => (
            "SHEET HER IN".into(),
            format!(
                "Her sheets are eased and she is going nowhere. Haul the mainsheet with \
                 {}, and wind the genoa on with {}.",
                key_name(MAINSHEET),
                key_name(WINCH)
            ),
        ),
        Stage::Steer => (
            "ONTO A REACH".into(),
            format!(
                "Helm with the arrow keys; she holds the angle you leave it at. Bring the \
                 wind onto the side of her — somewhere between forty-five and a hundred \
                 and thirty-five degrees. It reads {:.0} now.",
                deg(tm.awa.abs())
            ),
        ),
        Stage::Build => (
            "GET HER GOING".into(),
            format!(
                "Four knots. She is making {knots:.1}. Ease or trim until the sails stop \
                 flapping and she settles — the telltales on the sail are the instrument, \
                 not the number."
            ),
        ),
        Stage::Steady => (
            "HOLD THAT".into(),
            format!("{:.0}…", (HOLD - held).max(0.0).ceil()),
        ),
        Stage::Alarm => (
            "MAN OVERBOARD".into(),
            format!(
                "The dan buoy is in the water astern of you. Press {} to throw the \
                 lifebuoy — now, not in a minute: it lands where *you* are, and you are \
                 leaving him behind at six knots.",
                key_name(THROW)
            ),
        ),
        Stage::Search => {
            let (range, bearing) = over.to_oscar(sim.sim.state.pos).unwrap_or((0.0, 0.0));
            let lengths = range / sim.config().loa;
            let advice = if range < 15.0 && knots > 2.5 {
                "Too fast. Ease both sheets right off and let her stop — you cannot hold \
                 a man against a hull doing three knots."
            } else if range < 25.0 {
                "Come at him on a close reach, as slowly as she will still steer, and aim \
                 to arrive stopped with him at your shoulder — not under your bow."
            } else if lengths > 6.0 && knots > 2.0 {
                "Far enough. Tack — do not gybe with a man in the water — and work back \
                 towards him."
            } else if knots < 1.0 {
                "You have stopped out of reach of him. Sheet in and sail back; a drifting \
                 boat goes downwind faster than he does."
            } else {
                "Keep the flag in sight and sail back to it. He is going downwind at a \
                 tenth of a knot; everything else out there is you."
            };
            (
                format!("{range:.0} M, BEARING {:03.0}", deg(bearing)),
                advice.into(),
            )
        }
        Stage::Done => {
            let Drill::Recovered {
                took,
                ring_gap,
                thrown_after,
                run_downs,
            } = over.drill
            else {
                return ("ABOARD".into(), String::new());
            };
            (
                crate::mob::verdict(took, ring_gap, thrown_after, run_downs),
                match ring_gap {
                    Some(gap) if gap < 20.0 => {
                        "That is a lifebuoy he could swim to. The throw is the part of \
                         this that cannot be done late."
                            .into()
                    }
                    Some(_) => "Thrown late: by the time it left the boat you had sailed \
                                on, and it landed where you were rather than where he \
                                was."
                        .into(),
                    None => "Nothing went in the water with him. A lifebuoy is what he \
                             holds on to while you are making your mistakes."
                        .into(),
                },
            )
        }
    }
}

/// Draw the lesson: a card at the top of the screen and nothing else.
///
/// At the top because the water is at the bottom, and the whole point of the drill is
/// that you are looking at the water.
pub fn draw(
    mut contexts: bevy_egui::EguiContexts,
    lesson: Res<Tutorial>,
    sim: Res<Sim>,
    over: Res<Overboard>,
) -> Result {
    if !lesson.running() {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let held = sim.sim.state.time - lesson.since;
    let (heading, body) = instruction(lesson.stage, &sim, &over, held);
    // The alarm is red and everything else is the panel's own cream, because a man in
    // the water is the one thing in this game that is allowed to shout.
    let (ink, loud) = match lesson.stage {
        Stage::Alarm => (egui::Color32::from_rgb(236, 110, 86), true),
        Stage::Search => (egui::Color32::from_rgb(232, 176, 58), false),
        _ => (egui::Color32::from_rgb(200, 190, 150), false),
    };
    let frame = egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(14, 18, 24, 232))
        .stroke(egui::Stroke::new(1.0, ink))
        .inner_margin(egui::Margin::symmetric(16, 11));
    egui::Area::new("tutorial".into())
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 18.0))
        .show(ctx, |ui| {
            frame.show(ui, |ui| {
                ui.set_width(420.0);
                ui.label(
                    egui::RichText::new(&heading)
                        .color(ink)
                        .monospace()
                        .size(if loud { 20.0 } else { 14.0 }),
                );
                if !body.is_empty() {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(&body)
                            .color(egui::Color32::from_rgb(196, 202, 208))
                            .monospace()
                            .size(11.5),
                    );
                }
            });
        });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every key the lesson asks for is a key the program reads.
    ///
    /// The lesson names three and `help.rs` holds the list of everything the game
    /// answers to, checked against the source by its own test. This is the join between
    /// them: a tutorial that says "press ENTER" while nothing reads Enter is the one
    /// failure a player cannot work around, and it is invisible in both files on their
    /// own.
    #[test]
    fn the_lesson_only_asks_for_keys_the_game_reads() {
        for key in [MAINSHEET, WINCH, THROW] {
            assert!(
                crate::help::DOCUMENTED.contains(&key),
                "the lesson asks for {key:?} and the help has never heard of it"
            );
            assert_ne!(key_name(key), "?", "{key:?} has no name to print");
        }
    }

    /// The numbers the lesson waits on are the numbers of a boat sailing.
    #[test]
    // Constant assertions on purpose: these are the numbers somebody tuning the drill
    // will reach for, and what they must not do is drift out of the band the comment
    // above them claims. A test that can only fail when an edit happens is the test for
    // that.
    #[allow(clippy::assertions_on_constants)]
    fn the_drill_waits_for_a_boat_that_is_actually_sailing() {
        assert_eq!(Stage::default(), Stage::Off);
        // Below three knots she has no steerage worth the name and the pole would not be
        // properly astern of her when it went in.
        assert!(SAILING > 3.0, "a boat doing {SAILING:.1} knots is drifting");
        // And a countdown has to be long enough to read and short enough to be a
        // countdown.
        assert!(
            (3.0..12.0).contains(&HOLD),
            "{HOLD:.0} seconds is not a countdown"
        );
    }
}
