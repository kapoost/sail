//! SAILING 101: the boat, the console and the keys, one at a time.
//!
//! **The key list is not a lesson.** `H` puts forty-four keys on the screen in six
//! groups, which is the right reference and the wrong introduction: somebody who has
//! never sailed does not know which of the forty-four they need in the first minute, and
//! somebody who has sailed does not know that this boat has binoculars aboard until they
//! read the whole list looking for something else.
//!
//! So this walks the deck instead. One instruction at a time, each about **one control
//! and the instrument that answers it** — pull this, and watch that change — and nothing
//! moves on until the thing has been done. It is the shape the man-overboard drill
//! already uses, pointed at the interface rather than at a manoeuvre: the drill teaches
//! you to get her back to a spot, and this teaches you where everything is.
//!
//! Three things it does deliberately.
//!
//! **It sets the stage.** The watch is stood down and the pilot is taken off at the
//! start, because a lesson about the mainsheet is nonsense while somebody else is
//! trimming it and a lesson about the tiller is nonsense while the pilot is holding the
//! course. Both come back as their own steps, which is also the honest order: you learn
//! to do a thing before you learn to hand it over.
//!
//! **It names the thing on the screen, not only the key.** Every step says what to look
//! at while the key is pressed — the lever that moves, the lamp that lights, the word
//! that changes — because the point is not that `W` hauls the mainsheet, it is that the
//! boat tells you what hauling it did.
//!
//! **It is forgiving.** A step is finished by doing the thing, in any order, with the
//! key or with the button on the console — the two sighting steps watch the *instrument*
//! rather than the keystroke, so a reader who finds the buttons first is not told to
//! press a key they have just made unnecessary. Nothing here is a gate; the lesson can
//! be stopped from the same page it was started on.

use bevy::prelude::*;
use bevy_egui::egui;

use crate::sim::Sim;

/// Where the lesson has got to.
///
/// In the order a hand learns them: steer her, make her go, shorten sail, hand her over,
/// then everything you pick up to find out where you are. The last two are the ones
/// nobody would ever find on their own.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Step {
    /// Not running.
    #[default]
    Off,
    Helm,
    View,
    Main,
    Headsail,
    Clutch,
    Dump,
    Preventer,
    Pilot,
    Watch,
    Chart,
    Glasses,
    Bearing,
    Log,
    Rod,
    Buoy,
    Done,
}

/// Every step in order, which is also how the card counts them.
const WALK: [Step; 15] = [
    Step::Helm,
    Step::View,
    Step::Main,
    Step::Headsail,
    Step::Clutch,
    Step::Dump,
    Step::Preventer,
    Step::Pilot,
    Step::Watch,
    Step::Chart,
    Step::Glasses,
    Step::Bearing,
    Step::Log,
    Step::Rod,
    Step::Buoy,
];

/// How long the green tick stays up before the next instruction, seconds.
///
/// Long enough to be seen as an answer to what you just did, short enough that the
/// lesson does not feel like it is waiting for you to admire it.
const TICK: f64 = 1.1;

/// And how long the last step waits for a key before letting itself be finished.
///
/// The lifebuoy is the one step whose action puts something in the water, so it is the
/// one step that may be *read* rather than done.
const READ_IT: f64 = 12.0;

#[derive(Resource, Default)]
pub struct Basics {
    pub step: Step,
    /// When this step came up, on the ship's clock.
    since: f64,
    /// What has been seen of this step so far: most want two things, one bit each.
    seen: u8,
    /// When the step was satisfied, so the tick can stand for a moment.
    done_at: Option<f64>,
    /// Asked for from the missions page.
    pub wants_start: bool,
    pub wants_stop: bool,
}

impl Basics {
    /// `SAIL_LESSON=1` walks her round from the moment she opens.
    ///
    /// The same family as `SAIL_TUTORIAL`, and the same reason: a lesson you have to
    /// find in a menu before you can look at it is a lesson that gets looked at once.
    /// In a tab it is `?lesson=1`.
    pub fn asked_for(start: bool) -> Self {
        Self {
            wants_start: start,
            ..Self::default()
        }
    }

    pub fn running(&self) -> bool {
        self.step != Step::Off
    }

    /// Which of the walk this is, for the counter on the card.
    fn number(&self) -> Option<usize> {
        WALK.iter().position(|s| *s == self.step).map(|i| i + 1)
    }

    fn begin(&mut self, step: Step, now: f64) {
        self.step = step;
        self.since = now;
        self.seen = 0;
        self.done_at = None;
    }
}

/// What the card says at each step: the instruction, and what to watch while you do it.
fn instruction(step: Step) -> (&'static str, &'static str) {
    match step {
        Step::Off => ("", ""),
        Step::Helm => (
            "The tiller: press ← and →",
            "She is steered with the arrow keys, and the helm *stays where you leave \
             it* — this is a tiller you have lashed, not a wheel you are holding. Watch \
             HELM in the corner: it says which way the blade is over and by how much. \
             Hold shift with an arrow for a smaller bite.",
        ),
        Step::View => (
            "Press Space",
            "Space does two things that are one thing: it centres the helm and puts the \
             view back where it was looking. Hold the right mouse button to look around \
             the boat — your head turns, she does not.",
        ),
        Step::Main => (
            "The mainsheet: W hauls it in, S eases it",
            "Watch MAIN at the bottom right. The lever moves, and under it the sail \
             says what it is doing: LUFFING is too far out, STALLED is too far in, \
             DRAWING is right. The telltales on the sail say the same thing in the \
             language a sailor reads first.",
        ),
        Step::Headsail => (
            "The headsail: Q eases, E grinds in — X and Z pick which one",
            "Q and E are the winch. X and Z *order* a sail — genoa, code 0, gennaker, \
             spinnaker — and the foredeck rolls the old one away, bends the new one on \
             and sets it, which takes a minute you can watch under HEADSAIL.",
        ),
        Step::Clutch => (
            "The rope bank: 1 and 2 pick a line, A and D work it",
            "The row along the bottom is the clutch bank — the vang, the car, the reef, \
             the furling line, the staysail halyard. One line is in your hands at a \
             time: 1 and 2 walk along the bank, A and D haul and ease what you have. \
             That is how a reef goes in.",
        ),
        Step::Dump => (
            "When it blows: press R",
            "The one key worth knowing before you need it. Once, and the watch eases \
             until the deck is steady and the pilot can hold her again; pressed again, \
             or held, everything goes — cloth flogging, and she stops. A boat that is \
             over too far is not a boat that needs steering, it is a boat that needs \
             less sail, and this is the fastest way to have less of it.",
        ),
        Step::Preventer => (
            "The two ropes that do the opposite: F and G",
            "F rigs the preventer — a line from the end of the boom to the bow that \
             stops it coming across, for a long run downwind. Take it off before you \
             gybe on purpose. G backs the genoa: the only sail aboard that can be held \
             to windward, and the way out of irons when she has stopped head to wind \
             and will not pay off.",
        ),
        Step::Pilot => (
            "Hand her over: press P",
            "PILOT lights ENGAGED and she steers herself. O chooses what it holds — a \
             compass course or a wind angle — and , and . move the target ten degrees \
             at a time, one degree with shift. Touch the tiller and it stands down.",
        ),
        Step::Watch => (
            "Call the watch: shift T",
            "The two figures at the bottom right light up and the hands take the \
             sheets: they trim while you steer, and they say what they are on. REEFS \
             and SAILS under them are what they are *allowed to decide* — shortening \
             sail and changing a headsail are decisions, and they are yours if you want \
             them.",
        ),
        Step::Chart => (
            "The chart: press M",
            "It comes up in the corner, and [ and ] zoom it. Shift M spreads the real \
             one on the table, with dividers, parallel rule and a pencil that writes \
             the log up while it is out. She is only drawn on it while she knows where \
             she is — which is what the next two steps are for.",
        ),
        Step::Glasses => (
            "The glasses: shift B, or GLASSES on the console",
            "Binoculars do not magnify so much as *identify*: a beacon is a stick at \
             two miles and a beacon with a cone on it through the glasses. With them up, \
             B asks what is in them — a mark near enough names itself, and the name goes \
             in the deck log.",
        ),
        Step::Bearing => (
            "The hand bearing compass: shift N, or BEARING",
            "Aim it with the right mouse button held, put the notch on something you \
             have named, and N shoots the bearing. Three of them, well spread, cross on \
             the chart and that is a fix: the boat appears on the paper and the \
             instruments start answering again.",
        ),
        Step::Log => (
            "Stream the log: press L",
            "A minute of glass over the side, and it reads her speed through the water \
             — which is not her speed over the ground, and the difference is the tide. \
             The bar says when to stop her.",
        ),
        Step::Rod => (
            "The rod: press J, or ROD on the console",
            "It goes over the stern and trolls a lure at whatever depth the line is out \
             to. ↑ and ↓ are the reel, - and = the drag; shift is low gear. When \
             something takes, J again lifts her aboard — on the rise, as the sea comes \
             up to you.",
        ),
        Step::Buoy => (
            "And the one that cannot wait: Enter throws the lifebuoy",
            "If somebody goes in, the throw is the part that must not be late: Enter \
             puts the buoy over the side straight away, and the same key alongside \
             works the boathook. MISSIONS has the whole drill on Oscar, which is worth \
             an afternoon before you need it.",
        ),
        Step::Done => (
            "That is the boat",
            "Everything else is sailing. H hides the console, H again brings it back, \
             and the full key list is behind the mark in the corner. Esc opens MISSIONS, \
             where the drill, the regatta and the pilot job are.",
        ),
    }
}

/// Watch for the thing the step asked for, and move the lesson on when it happens.
#[allow(clippy::too_many_arguments)]
pub fn run(
    mut lesson: ResMut<Basics>,
    keys: Res<ButtonInput<KeyCode>>,
    sim: Res<Sim>,
    mut pilot: ResMut<crate::input::Pilot>,
    mut crew: ResMut<crate::crew::Crew>,
    sighting: Res<crate::sight::Sighting>,
    fishing: Res<crate::fishing::Fishing>,
    drill: Res<crate::tutorial::Tutorial>,
) {
    let now = sim.sim.state.time;
    if lesson.wants_stop {
        lesson.wants_stop = false;
        lesson.begin(Step::Off, now);
    }
    if lesson.wants_start {
        lesson.wants_start = false;
        // **The stage is set before the first instruction.** A lesson about the
        // mainsheet is nonsense while the watch is trimming it, and a lesson about the
        // tiller is nonsense while the pilot is holding the course. Both come back as
        // steps of their own, in the order a hand learns them: do it yourself first,
        // hand it over second.
        crew.standing = false;
        pilot.disengage();
        lesson.begin(Step::Helm, now);
    }
    // One card at a time. The drill owns the same corner of the screen, and two
    // instructions in one place is no instruction at all.
    if drill.running() && lesson.running() {
        lesson.begin(Step::Off, now);
    }
    if !lesson.running() {
        return;
    }

    // The tick is up: on to the next one, or out.
    if let Some(at) = lesson.done_at {
        if now - at > TICK {
            let next = WALK
                .iter()
                .position(|s| *s == lesson.step)
                .and_then(|i| WALK.get(i + 1).copied())
                .unwrap_or(Step::Done);
            if lesson.step == Step::Done {
                lesson.begin(Step::Off, now);
            } else {
                lesson.begin(next, now);
            }
        }
        return;
    }

    let shifted = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let mark = |bit: u8, pressed: bool, lesson: &mut Basics| {
        if pressed {
            lesson.seen |= bit;
        }
    };
    let satisfied = match lesson.step {
        Step::Off => false,
        Step::Helm => {
            // Both ways, because the thing worth learning is that it stays where it is
            // put: one press teaches a rudder, two teach a tiller.
            mark(1, keys.just_pressed(KeyCode::ArrowLeft), &mut lesson);
            mark(2, keys.just_pressed(KeyCode::ArrowRight), &mut lesson);
            lesson.seen == 3
        }
        Step::View => keys.just_pressed(KeyCode::Space),
        Step::Main => {
            mark(1, keys.just_pressed(KeyCode::KeyW), &mut lesson);
            mark(2, keys.just_pressed(KeyCode::KeyS), &mut lesson);
            lesson.seen == 3
        }
        Step::Headsail => {
            keys.just_pressed(KeyCode::KeyQ)
                || keys.just_pressed(KeyCode::KeyE)
                || keys.just_pressed(KeyCode::KeyX)
                || keys.just_pressed(KeyCode::KeyZ)
        }
        Step::Clutch => {
            // A line picked *and* worked, which is the whole of how the bank is used.
            mark(
                1,
                keys.just_pressed(KeyCode::Digit1) || keys.just_pressed(KeyCode::Digit2),
                &mut lesson,
            );
            mark(
                2,
                keys.just_pressed(KeyCode::KeyA) || keys.just_pressed(KeyCode::KeyD),
                &mut lesson,
            );
            lesson.seen == 3
        }
        // The state rather than the key, so the console button finishes the step as
        // honestly as the keyboard does.
        Step::Dump => keys.just_pressed(KeyCode::KeyR),
        // Either of them: the preventer wants a boom off the centreline and backing
        // the genoa wants a genoa that is out, and a lesson is not the place to insist
        // on the conditions for a rope.
        Step::Preventer => keys.just_pressed(KeyCode::KeyF) || keys.just_pressed(KeyCode::KeyG),
        Step::Pilot => pilot.engaged(),
        Step::Watch => crew.standing,
        Step::Chart => keys.just_pressed(KeyCode::KeyM),
        Step::Glasses => sighting.glasses,
        Step::Bearing => sighting.up,
        Step::Log => keys.just_pressed(KeyCode::KeyL) && !shifted,
        Step::Rod => fishing.out(),
        Step::Buoy => keys.just_pressed(KeyCode::Enter) || now - lesson.since > READ_IT,
        Step::Done => now - lesson.since > READ_IT,
    };
    if satisfied {
        lesson.done_at = Some(now);
    }
}

/// The card, and the tick when a step is done.
pub fn draw(mut contexts: bevy_egui::EguiContexts, lesson: Res<Basics>, sim: Res<Sim>) -> Result {
    if !lesson.running() {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let now = sim.sim.state.time;
    let done = lesson.done_at.is_some();
    let (heading, body) = instruction(lesson.step);
    // Green when it has just been done, the lesson's own amber while it is being asked
    // for: the colour is the answer, and it arrives before the words are read.
    let ink = if done {
        egui::Color32::from_rgb(120, 190, 130)
    } else {
        egui::Color32::from_rgb(200, 190, 150)
    };
    crate::tutorial::card(ctx, ink, heading, body, |ui| {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            // Where in the walk this is. A lesson with no end in sight is a lesson
            // people leave half way through.
            if let Some(n) = lesson.number() {
                ui.label(
                    egui::RichText::new(format!("{n} of {}", WALK.len()))
                        .color(egui::Color32::from_rgb(130, 140, 150))
                        .monospace()
                        .size(12.0),
                );
            }
            if done {
                ui.label(
                    egui::RichText::new("✔  that is the one")
                        .color(ink)
                        .monospace()
                        .size(13.0),
                );
            } else if lesson.step == Step::Done {
                let left = (READ_IT - (now - lesson.since)).max(0.0);
                ui.label(
                    egui::RichText::new(format!("closing in {left:.0}"))
                        .color(egui::Color32::from_rgb(130, 140, 150))
                        .monospace()
                        .size(12.0),
                );
            } else {
                ui.label(
                    egui::RichText::new("MISSIONS → SAILING 101 stops it")
                        .color(egui::Color32::from_rgb(110, 118, 128))
                        .monospace()
                        .size(11.0),
                );
            }
        });
    });
    Ok(())
}

#[cfg(test)]
mod wiring {
    /// A system with a parameter the engine cannot supply fails to compile at the place
    /// it is *registered*, in a type error a page long that names the other twenty
    /// systems in the tuple and not this one. Asserting it here fails in one line with
    /// the right file on it.
    #[test]
    fn the_lesson_is_a_system() {
        bevy::ecs::system::assert_is_system(super::run);
    }
}
