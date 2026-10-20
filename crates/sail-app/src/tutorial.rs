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
//! **The pattern it coaches is the figure of eight** — the reach, tack, reach that US
//! Sailing and the RYA both teach for a yacht with the casualty in sight, and the one
//! that works under sail because it never asks her to gybe and ends on a close reach,
//! which is the only point of sail you can stop a boat on. Bear away onto a beam reach
//! and sail four boat lengths; tack; work down to leeward of him; then come up at him on
//! a close reach with everything eased, and arrive stopped. The Williamson turn in the
//! IAMSAR manual is the right answer for a ship under power and the wrong one here: it
//! is a two-stage turn through two hundred and forty degrees of rudder that puts you back
//! in your own wake, which a sailing boat cannot do without going through the wind twice
//! and stopping dead in the middle of it.
//!
//! Each leg is given a **target heading and an arrow**, and a mark is put in the water at
//! the point she should be steering for. That is deliberate hand-holding: a beginner
//! being told "work down to leeward of him" has no idea what that means, and a beginner
//! steering at a lit mark learns what it means by doing it. The marks and the arrows are
//! obviously artificial and are meant to be — they are a training aid, not scenery.
//!
//! And the shouting is deliberate too. When something has to be done in the next two
//! seconds it is **four words across the middle of the screen**, because a sentence in a
//! panel is not read by somebody who is looking at the water and trying to steer. The
//! rest of the time the card is quiet and small. A game that shouts all the time is a
//! game nobody reads; the whole value of big letters is that they are rare.
//!
//! The coaching inside each leg is **advice and not a gate**: the drill ends when Oscar
//! is aboard, however that was managed. A tutorial that refuses to advance until you
//! perform the approved manoeuvre in the approved order is not teaching, it is a form to
//! fill in.

use bevy::prelude::*;
use bevy_egui::egui;
use glam::DVec2;

use crate::course::Floating;
use crate::mob::{Drill, Overboard};
use crate::sim::Sim;
use crate::sound::SoundHandle;
use sail_core::units::{deg, ms_to_kn, rad, wrap_pi};

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
/// Boat lengths to sail on the first reach before tacking.
///
/// Four. Far enough that the tack and the run back down give her time to settle and to
/// arrive at him slowly; near enough that he is never more than a hundred metres off.
const CLEAR_LENGTHS: f64 = 4.0;
/// How far off the wind the final approach is, degrees.
///
/// Fifty: a close reach. Any closer and she cannot be slowed without stalling and
/// drifting sideways onto him; any broader and the sails cannot be spilled enough to
/// stop her at all, which is the mistake that runs people down.
const APPROACH_ANGLE: f64 = 50.0;
/// And how far downwind of him the approach begins, in boat lengths.
const GATE_LENGTHS: f64 = 3.5;

/// Where in the figure of eight she is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Leg {
    /// Away on a beam reach, getting clear and getting control.
    Clear,
    /// Round through the wind. Not round the stern.
    Tack,
    /// Down to leeward of him, so that the last of it can be done slowly.
    Downwind,
    /// And up at him on a close reach, with everything eased.
    Approach,
}

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
    /// Sailing the pattern back to him.
    Search(Leg),
    /// He is gone.
    Lost,
    /// And the numbers.
    Done,
}

/// How loudly a thing is said.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tone {
    /// Something has happened. Red.
    Alarm,
    /// Do this, now. Amber.
    Order,
    /// It worked. Green.
    Good,
    /// It did not. The colour of dried blood, and it stays up.
    Dead,
}

/// Four words across the middle of the screen, and how long they have been there.
#[derive(Clone, Copy)]
struct Shout {
    words: &'static str,
    under: &'static str,
    since: f64,
    hold: f64,
    tone: Tone,
}

#[derive(Resource, Default)]
pub struct Tutorial {
    pub stage: Stage,
    /// When this stage began, in simulation seconds.
    since: f64,
    /// Set by the settings panel, cleared here the same frame.
    pub wants_start: bool,
    pub wants_stop: bool,
    /// The big letters, if anything is being shouted.
    shout: Option<Shout>,
    /// Which side the wind was on when the tack began, so the tack can be detected.
    tack_from: f64,
    /// Which side of him the approach is being made from, chosen once when the run down
    /// to leeward begins and then kept. Never nought — a side of nought would put the
    /// gate dead upwind of him, which is the one place she cannot sail from.
    ///
    /// Kept, because it was read live off the wind and that was a fault: sailing down to
    /// leeward she passes through dead downwind, the apparent wind changes sides, and the
    /// mark she is steering at jumps seventy metres across to the other side of him.
    approach_side: f64,
    /// Where she should be steering, if there is a mark to put in the water for it.
    gate: Option<DVec2>,
}

/// The mark the lesson puts in the water to steer at.
#[derive(Component)]
pub struct Guide;

impl Tutorial {
    /// A lesson that starts itself on the first frame, which is what `SAIL_TUTORIAL=1`
    /// asks for: the drill gets looked at twenty times in an afternoon of building it,
    /// and clicking through a panel each time is twenty clicks.
    pub fn asked_for(start: bool) -> Self {
        Self {
            wants_start: start,
            approach_side: 1.0,
            ..Self::default()
        }
    }

    pub fn running(&self) -> bool {
        self.stage != Stage::Off
    }

    fn shout(&mut self, words: &'static str, under: &'static str, tone: Tone, now: f64) {
        self.shout = Some(Shout {
            words,
            under,
            since: now,
            hold: match tone {
                // Long enough to read while steering, short enough not to be in the way
                // of the water. The dead one does not go away: it is the end of the
                // drill and there is nothing to look at the sea for any more.
                Tone::Alarm => 3.2,
                Tone::Order => 2.2,
                Tone::Good => 4.0,
                Tone::Dead => f64::INFINITY,
            },
            tone,
        });
    }
}

/// The mark in the water, which is only there while a leg has somewhere to steer.
pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    sim: Res<Sim>,
) {
    // A column of light rather than a buoy. It is a training aid and it should look like
    // one: nothing in the archipelago glows, so nobody will mistake this for a mark that
    // belongs to the sea.
    commands.spawn((
        Guide,
        Floating {
            at: sim.sim.state.pos,
            lift: 2.6,
        },
        Mesh3d(meshes.add(Cylinder::new(0.30, 5.4))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.72, 0.18, 0.30),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        })),
        Transform::default(),
        Visibility::Hidden,
    ));
}

/// Run the lesson.
#[allow(clippy::too_many_arguments)]
pub fn run(
    sim: Res<Sim>,
    sound: Res<SoundHandle>,
    mut lesson: ResMut<Tutorial>,
    mut over: ResMut<Overboard>,
    mut guide: Query<(&mut Floating, &mut Visibility), With<Guide>>,
) {
    let now = sim.sim.state.time;
    if lesson.wants_start {
        lesson.wants_start = false;
        lesson.stage = Stage::Trim;
        lesson.since = now;
        lesson.shout = None;
        lesson.gate = None;
        over.drill = Drill::Secure;
    }
    if lesson.wants_stop {
        lesson.wants_stop = false;
        lesson.stage = Stage::Off;
        lesson.shout = None;
        lesson.gate = None;
        over.drill = Drill::Secure;
    }
    if !lesson.running() {
        if let Ok((_, mut vis)) = guide.single_mut() {
            *vis = Visibility::Hidden;
        }
        return;
    }

    let tm = &sim.sim.telemetry;
    let controls = &sim.controls;
    let knots = ms_to_kn(tm.stw);
    let held = now - lesson.since;
    let loa = sim.config().loa;

    match lesson.stage {
        Stage::Off => {}
        Stage::Trim => {
            if controls.main_sheet < 0.72 && controls.jib_sheet < 0.72 {
                step(&mut lesson, Stage::Steer, now);
            }
        }
        Stage::Steer => {
            if (45.0..135.0).contains(&deg(tm.awa.abs())) {
                step(&mut lesson, Stage::Build, now);
            }
        }
        Stage::Build => {
            if knots > SAILING {
                step(&mut lesson, Stage::Steady, now);
            }
        }
        Stage::Steady => {
            if knots < SAILING - 0.8 {
                step(&mut lesson, Stage::Build, now);
            } else if held > HOLD {
                let st = &sim.render_state();
                // Over the side at the quarter, where it is racked: astern of her and a
                // little to leeward, which is where a person goes in from the rail.
                // Off the rail and clear of her, not inside her. A point a metre and a
                // half off the centreline is *inside* a boat three and a half metres in
                // the beam, so `off_the_rail` read nought at the instant he went in — and
                // nought at six knots is the run-down check firing on the accident
                // itself, which opened every debrief by accusing the player of sailing
                // over him. Found in review. How far clear is `CLEAR_OF_HER`, which lives
                // beside the check it has to stay outside of.
                let clear = sim.config().beam * 0.5 + crate::mob::CLEAR_OF_HER;
                let at = st.pos - st.forward() * (loa * 0.45)
                    + st.starboard() * tm.awa.signum() * -clear;
                over.man_overboard(at, now);
                // Three strikes. The bell is the one thing aboard whose whole purpose is
                // telling people something, and this is the thing it is for.
                sound.0.ring(3);
                step(&mut lesson, Stage::Alarm, now);
                lesson.shout(
                    "OSCAR OVERBOARD",
                    "throw him something — now",
                    Tone::Alarm,
                    now,
                );
            }
        }
        Stage::Alarm => {
            let thrown = matches!(over.drill, Drill::Overboard { ring: Some(_), .. });
            if thrown || held > 14.0 {
                lesson.tack_from = tm.awa.signum();
                step(&mut lesson, Stage::Search(Leg::Clear), now);
                lesson.shout(
                    "BEAR AWAY",
                    "onto a beam reach, and sail on four lengths",
                    Tone::Order,
                    now,
                );
            }
        }
        Stage::Search(leg) => match over.drill {
            Drill::Recovered { .. } => {
                step(&mut lesson, Stage::Done, now);
                lesson.shout(
                    "OSCAR ABOARD",
                    "and that is the one that counts",
                    Tone::Good,
                    now,
                );
            }
            Drill::Lost { .. } => {
                step(&mut lesson, Stage::Lost, now);
                sound.0.ring(1);
                lesson.shout("OSCAR IS GONE", "", Tone::Dead, now);
            }
            Drill::Overboard { mark, oscar, .. } => {
                // Can the last leg be sailed from where she has got to? Asked before the
                // leg is advanced, because the answer can send her back one — and being
                // sent back with a reason is the coaching. The first version pointed the
                // arrow straight at him whatever the wind was doing and said *steady as
                // she goes*, which put her ten metres dead upwind of him being told she
                // was doing well. Reported from the deck.
                if leg == Leg::Approach {
                    let aim = aim_point(oscar, tm.twd, sim.config().beam);
                    let course = bearing(sim.sim.state.pos, aim);
                    let (words, under) = match can_she_lay_him(tm.twd, course) {
                        Lay::Good => ("", ""),
                        Lay::DeadUpwind => (
                            "HE IS DEAD UPWIND",
                            "you cannot sail at him from there — fall off and come round \
                             below him",
                        ),
                        Lay::FromWindward => (
                            "YOU ARE ABOVE HIM",
                            "never come down on a man with the wind behind you — go round \
                             and come up from below",
                        ),
                    };
                    if !words.is_empty() {
                        lesson.approach_side = better_side(sim.sim.state.pos, oscar, tm.twd, loa);
                        lesson.stage = Stage::Search(Leg::Downwind);
                        lesson.since = now;
                        lesson.shout(words, under, Tone::Order, now);
                    }
                }
                if let Stage::Search(leg) = lesson.stage {
                    let next = advance(
                        leg,
                        &sim,
                        &over,
                        lesson.tack_from,
                        mark,
                        lesson.approach_side,
                    );
                    if next != leg {
                        if next == Leg::Downwind {
                            // Chosen here, and kept from here: read live off the wind it
                            // jumped to the other side of him every time she went through
                            // dead downwind.
                            lesson.approach_side =
                                better_side(sim.sim.state.pos, oscar, tm.twd, loa);
                        }
                        lesson.stage = Stage::Search(next);
                        lesson.since = now;
                        if let Some((words, under)) = order_for(next) {
                            lesson.shout(words, under, Tone::Order, now);
                        }
                        if next == Leg::Tack {
                            lesson.tack_from = tm.awa.signum();
                        }
                    }
                }
            }
            Drill::Secure => {}
        },
        Stage::Lost => {
            // It stays up a good while — it is the end of the drill and there is nothing
            // to look at the water for any more — and then it goes, because a screen a
            // player cannot dismiss is a screen that has stopped being drama.
            if held > VERDICT {
                lesson.stage = Stage::Off;
            }
        }
        Stage::Done => {
            if held > VERDICT {
                lesson.stage = Stage::Off;
            }
        }
    }

    // And the mark in the water, which is wherever this leg wants her to go.
    lesson.gate = match lesson.stage {
        Stage::Search(leg) => steer_for(leg, &sim, &over, lesson.approach_side),
        _ => None,
    };
    if let Ok((mut float, mut vis)) = guide.single_mut() {
        match lesson.gate {
            Some(at) => {
                float.at = at;
                *vis = Visibility::Inherited;
            }
            None => *vis = Visibility::Hidden,
        }
    }
}

fn step(lesson: &mut Tutorial, to: Stage, now: f64) {
    lesson.stage = to;
    lesson.since = now;
}

/// The true bearing from one place to another, clockwise from north.
fn bearing(from: DVec2, to: DVec2) -> f64 {
    let off = to - from;
    off.x.atan2(off.y).rem_euclid(std::f64::consts::TAU)
}

/// The course the final approach is sailed on: a close reach, on the tack she is on.
///
/// Fifty degrees off the true wind, on the side the wind is already on, so that the last
/// hundred metres needs no tack and no gybe — which is the whole reason the pattern is
/// shaped the way it is.
fn approach_course(twd: f64, wind_side: f64) -> f64 {
    (twd - rad(APPROACH_ANGLE) * wind_side).rem_euclid(std::f64::consts::TAU)
}

/// Where the approach begins: that many boat lengths back down the approach course from
/// him, which is to leeward and off to one side.
fn gate(oscar: DVec2, twd: f64, wind_side: f64, loa: f64) -> DVec2 {
    let course = approach_course(twd, wind_side);
    oscar - DVec2::new(course.sin(), course.cos()) * (GATE_LENGTHS * loa)
}

/// Where she should be steering on this leg, if there is a point for it.
fn steer_for(leg: Leg, sim: &Sim, over: &Overboard, side: f64) -> Option<DVec2> {
    let oscar = over.oscar()?;
    let tm = &sim.sim.telemetry;
    let loa = sim.config().loa;
    match leg {
        // Nothing to steer at: the instruction is an angle to the wind and a distance,
        // and an arrow says it better than a mark would.
        Leg::Clear | Leg::Tack => None,
        Leg::Downwind => Some(gate(oscar, tm.twd, side, loa)),
        // Nothing on the last leg: a light standing on top of him is a light in the way
        // of the one thing he is supposed to be looking at, which is the flag.
        Leg::Approach => None,
    }
}

/// Where to aim on the last leg, which is **not** at him.
///
/// A boat's length to leeward of him, so that when she stops he is on her *windward*
/// side: the mainsail and the boom are over the other rail where they cannot come across
/// him, you are looking at him over the high side, and she is not being set down on top
/// of his head by her own leeway. Aiming at him exactly is how a casualty ends up under
/// the bow, which is the one outcome worse than missing him.
fn aim_point(oscar: DVec2, twd: f64, beam: f64) -> DVec2 {
    let downwind = DVec2::new(-twd.sin(), -twd.cos());
    oscar + downwind * (beam * 0.5 + 1.2)
}

/// Which side of him to come up from, chosen once: whichever gate is nearer to where she
/// is now, so the run down to leeward is the short way round rather than a lap of him.
fn better_side(pos: DVec2, oscar: DVec2, twd: f64, loa: f64) -> f64 {
    let (a, b) = (gate(oscar, twd, 1.0, loa), gate(oscar, twd, -1.0, loa));
    if (pos - a).length() <= (pos - b).length() {
        1.0
    } else {
        -1.0
    }
}

/// How far off the true wind a course is, in radians, 0 dead into it and pi dead before
/// it.
fn off_the_wind(twd: f64, course: f64) -> f64 {
    wrap_pi(twd - course).abs()
}

/// Whether a boat can actually sail the course to him from where she is.
///
/// Three answers, and the two bad ones are the whole of why this exists. The first
/// version of the approach pointed the arrow straight at him and said *steady as she
/// goes* whatever that course happened to be — which put her ten metres dead upwind of
/// him, head to wind, being told she was doing well. Reported from the deck.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lay {
    /// He is in the no-go zone: she cannot sail at him at all from here.
    DeadUpwind,
    /// She is above him and sailing at him means bearing down on him with the wind
    /// behind her, which no method of recovery allows: she cannot be stopped, and if
    /// anything goes wrong she goes over the top of him.
    FromWindward,
    /// A close reach, or near enough to one to be slowed and stopped.
    Good,
}

/// Forty degrees is about what a cruising boat tacks through either side of the wind, so
/// inside that there is no course to him at all. Past a hundred she has the wind behind
/// her and is running down on him.
fn can_she_lay_him(twd: f64, course: f64) -> Lay {
    let off = deg(off_the_wind(twd, course));
    if off < 40.0 {
        Lay::DeadUpwind
    } else if off > 100.0 {
        Lay::FromWindward
    } else {
        Lay::Good
    }
}

/// Whether this leg is done, and what comes after it.
///
/// Every one of these is a fact about the boat rather than a key the player has to press,
/// so somebody who sails the pattern without reading a word of it gets carried along by
/// the same prompts as somebody following them exactly.
fn advance(
    leg: Leg,
    sim: &Sim,
    over: &Overboard,
    tack_from: f64,
    mark: DVec2,
    approach_side: f64,
) -> Leg {
    let tm = &sim.sim.telemetry;
    let loa = sim.config().loa;
    let pos = sim.sim.state.pos;
    let Some(oscar) = over.oscar() else {
        return leg;
    };
    match leg {
        // Clear of him, with the boat going and under control.
        Leg::Clear => {
            if (pos - mark).length() > CLEAR_LENGTHS * loa {
                Leg::Tack
            } else {
                leg
            }
        }
        // Through the wind. Detected rather than asked for: the side the wind is on has
        // changed, which is what a tack *is*.
        Leg::Tack => {
            if tm.awa.signum() != tack_from && tm.awa.abs() > rad(30.0) {
                Leg::Downwind
            } else {
                leg
            }
        }
        // Down to leeward of him, far enough out that the last of it is a close reach.
        Leg::Downwind => {
            let here = gate(oscar, tm.twd, approach_side, loa);
            if (pos - here).length() < loa * 1.3 {
                Leg::Approach
            } else {
                leg
            }
        }
        Leg::Approach => leg,
    }
}

/// The four words for the start of a leg.
fn order_for(leg: Leg) -> Option<(&'static str, &'static str)> {
    match leg {
        Leg::Clear => Some(("BEAR AWAY", "onto a beam reach, and sail on four lengths")),
        Leg::Tack => Some(("TACK NOW", "through the wind — never round the stern")),
        Leg::Downwind => Some(("DOWN TO LEEWARD", "steer for the light, below him")),
        Leg::Approach => Some(("EASE EVERYTHING", "come up at him, and arrive stopped")),
    }
}

/// What to say, which is one instruction at a time and nothing else.
fn instruction(stage: Stage, sim: &Sim, over: &Overboard, held: f64) -> (String, String) {
    let tm = &sim.sim.telemetry;
    let knots = ms_to_kn(tm.stw);
    let loa = sim.config().loa;
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
        Stage::Alarm => {
            // The number that teaches, live: how far from him the lifebuoy would land if
            // it left the boat now. It grows while you read this sentence.
            let gap = over.off_the_rail(sim).unwrap_or(0.0);
            (
                format!("THROW IT — IT WOULD LAND {gap:.0} M FROM OSCAR"),
                format!(
                    "Oscar is what every crew calls the man in the water, after flag O, \
                     and in a drill he is the dummy you practise on. {} throws the \
                     lifebuoy from behind the helm. It lands where *you* are, and you are \
                     leaving him at {knots:.1} knots.",
                    key_name(THROW)
                ),
            )
        }
        Stage::Search(leg) => {
            // Off her rail, not from her middle: the same number the recovery is decided
            // on, so that "two metres" means you can reach him.
            let (_, bearing) = over.to_oscar(sim.sim.state.pos).unwrap_or((0.0, 0.0));
            let range = over.off_the_rail(sim).unwrap_or(0.0);
            let left = over.endurance_left(sim.sim.state.time);
            let minutes = left * crate::mob::ENDURANCE / 60.0;
            // The distance from the *mark* — the spot where he went in — because that is
            // the one the leg ends on. It was showing the range to him instead, which
            // drifts away from it as he does, so the counter on screen and the thing that
            // advanced the leg were two different measurements of two different places.
            let run = over
                .mark()
                .map(|m| (sim.sim.state.pos - m).length() / loa)
                .unwrap_or(0.0);
            let body = match leg {
                Leg::Clear => format!(
                    "Sail clear on a beam reach — wind square on the side of her, {:.0} \
                     now. {CLEAR_LENGTHS:.0} boat lengths is enough; you are {run:.1} \
                     along.",
                    deg(tm.awa.abs())
                ),
                Leg::Tack => "Helm over and through the wind. Not round the stern: a gybe \
                              with a man in the water puts the boom across the cockpit and \
                              takes your eyes off him."
                    .into(),
                Leg::Downwind => "Work down to leeward of Oscar — below him, downwind — \
                                  so that the last of it can be sailed slowly uphill. \
                                  Steer for the light."
                    .into(),
                Leg::Approach => format!(
                    "Up at him on a close reach, sheets eased, as slowly as she will \
                     still steer. {range:.0} m off at {knots:.1} knots. Aim a boat's \
                     length below him so he finishes on your *windward* side, forward of \
                     amidships: the boom is over the other rail, you can see him, and she \
                     is not setting down on top of him."
                ),
            };
            (
                format!(
                    "{range:.0} M, BEARING {:03.0} — {minutes:.0} MIN",
                    deg(bearing)
                ),
                body,
            )
        }
        Stage::Lost => {
            let Drill::Lost { range, ring_gap } = over.drill else {
                return ("OSCAR IS GONE".into(), String::new());
            };
            (
                format!("OSCAR LOST — {range:.0} M AWAY"),
                match ring_gap {
                    Some(gap) => format!(
                        "Ten minutes in the water is as long as a person's hands keep \
                         working. The lifebuoy is still out there, {gap:.0} m from where \
                         he was. Esc, then DRILL, and again."
                    ),
                    None => "Ten minutes in the water is as long as a person's hands keep \
                             working, and nothing went in with him to hold on to. Esc, \
                             then DRILL, and again."
                        .into(),
                },
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
                return ("OSCAR ABOARD".into(), String::new());
            };
            (
                crate::mob::verdict(took, ring_gap, thrown_after, run_downs),
                match ring_gap {
                    Some(gap) if gap < 20.0 => "That is a lifebuoy he could swim to. The \
                                                throw is the part of this that cannot be \
                                                done late."
                        .into(),
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

/// Where she should be pointing on this leg, so the arrow has something to point at.
///
/// Every one of these is a heading rather than a feeling, which is the hand-holding: the
/// difference between "work down to leeward of him" and a number with an arrow beside it
/// is the difference between a beginner guessing and a beginner steering.
fn target_heading(
    stage: Stage,
    sim: &Sim,
    over: &Overboard,
    tack_from: f64,
    approach_side: f64,
) -> Option<f64> {
    let tm = &sim.sim.telemetry;
    let side = if tm.awa.abs() < rad(1.0) {
        1.0
    } else {
        tm.awa.signum()
    };
    let beam = |on: f64| (tm.twd - rad(90.0) * on).rem_euclid(std::f64::consts::TAU);
    let bearing_to = |at: DVec2| bearing(sim.sim.state.pos, at);
    match stage {
        // Getting her going in the first place: the wind on the beam is the easiest
        // thing to hold and the fastest way to four knots.
        Stage::Steer | Stage::Build | Stage::Steady => Some(beam(side)),
        Stage::Search(Leg::Clear) => Some(beam(side)),
        // Through the wind: the arrow points at the course on the new tack, which is
        // where the helm has to take her.
        Stage::Search(Leg::Tack) => Some(beam(-tack_from)),
        Stage::Search(Leg::Downwind) => over
            .oscar()
            .map(|o| gate(o, tm.twd, approach_side, sim.config().loa))
            .map(bearing_to),
        // At a point a boat's length to leeward of him, not at him — see `aim_point`.
        Stage::Search(Leg::Approach) => over
            .oscar()
            .map(|o| aim_point(o, tm.twd, sim.config().beam))
            .map(bearing_to),
        _ => None,
    }
}

/// Draw the lesson: a quiet card at the top, an arrow for the helm, and — when something
/// has to happen in the next two seconds — four words across the middle.
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
    let now = sim.sim.state.time;
    let held = now - lesson.since;
    let (heading, body) = instruction(lesson.stage, &sim, &over, held);
    let ink = match lesson.stage {
        Stage::Alarm => egui::Color32::from_rgb(236, 110, 86),
        Stage::Lost => egui::Color32::from_rgb(168, 58, 46),
        Stage::Search(_) => egui::Color32::from_rgb(232, 176, 58),
        Stage::Done => egui::Color32::from_rgb(120, 190, 130),
        _ => egui::Color32::from_rgb(200, 190, 150),
    };
    card(ctx, ink, &heading, &body, |ui| {
        // The helm cue, under the words, because it is the thing you act on.
        if let Some(want) = target_heading(
            lesson.stage,
            &sim,
            &over,
            lesson.tack_from,
            lesson.approach_side,
        ) {
            ui.add_space(6.0);
            // Never "steady as she goes" onto a course she cannot sail. The cue knows
            // what the wind is doing now, which is the difference between an arrow and
            // an instrument.
            let lay = can_she_lay_him(sim.sim.telemetry.twd, want);
            helm_cue(ui, wrap_pi(want - sim.sim.state.heading), lay);
        }
        // And how long he has, drawn as something emptying rather than as a number,
        // because a bar going down is read without being looked at.
        if over.live() {
            ui.add_space(5.0);
            endurance_bar(ui, over.endurance_left(now) as f32);
        }
    });

    if let Some(shout) = lesson.shout {
        let age = now - shout.since;
        // In quickly, out slowly, and the dead one never leaves.
        let alpha = if shout.hold.is_infinite() {
            crate::curve::smoothstep(0.0, 0.5, age as f32)
        } else {
            crate::curve::smoothstep(0.0, 0.22, age as f32)
                * (1.0
                    - crate::curve::smoothstep(
                        shout.hold as f32,
                        shout.hold as f32 + 0.7,
                        age as f32,
                    ))
        };
        if alpha > 0.01 {
            big_letters(ctx, shout, alpha);
        }
    }
    Ok(())
}

/// **The card the lessons talk through**, at the top of the screen.
///
/// One frame, one instruction, and whatever the lesson wants under it. Shared by the
/// man-overboard drill and by SAILING 101 rather than written twice, because two cards
/// drift apart in size and colour and then the game has two voices.
///
/// **And it is half again the size it was.** At 430 pixels across with eleven-point body
/// text it was a footnote on a window 1440 wide: the one thing on the screen telling
/// somebody what to do, set smaller than the labels on the instruments. The words a
/// beginner is reading while trying to steer have to be readable from where a beginner
/// sits, which is further back than the person who wrote them.
pub fn card(
    ctx: &egui::Context,
    ink: egui::Color32,
    heading: &str,
    body: &str,
    extra: impl FnOnce(&mut egui::Ui),
) {
    let frame = egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(14, 18, 24, 236))
        .stroke(egui::Stroke::new(1.0, ink))
        .inner_margin(egui::Margin::symmetric(20, 15));
    egui::Area::new("lesson".into())
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 20.0))
        .show(ctx, |ui| {
            frame.show(ui, |ui| {
                ui.set_width(600.0);
                ui.label(
                    egui::RichText::new(heading)
                        .color(ink)
                        .monospace()
                        .size(18.0),
                );
                if !body.is_empty() {
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(body)
                            .color(egui::Color32::from_rgb(206, 212, 218))
                            .monospace()
                            .size(14.0),
                    );
                }
                extra(ui);
            });
        });
}

/// Which way to push the helm, and how hard.
///
/// Chevrons rather than a number alone: the number tells you how far off you are and the
/// chevrons tell you which way to move your hand, and a beginner needs the second one
/// first. Green inside eight degrees, which is as close as anybody steers by hand.
fn helm_cue(ui: &mut egui::Ui, error: f64, lay: Lay) {
    let off = deg(error);
    // A course inside the no-go zone is not a course, however well she is steering it.
    if lay == Lay::DeadUpwind {
        ui.label(
            egui::RichText::new("◀ THAT IS INTO THE WIND — FALL OFF ▶")
                .color(egui::Color32::from_rgb(236, 110, 86))
                .monospace()
                .size(13.0),
        );
        return;
    }
    let on_course = off.abs() < 8.0;
    let colour = if on_course {
        egui::Color32::from_rgb(120, 190, 130)
    } else {
        egui::Color32::from_rgb(230, 198, 120)
    };
    let chevrons = ((off.abs() / 15.0).ceil() as usize).clamp(1, 4);
    let text = if on_course {
        "— STEADY AS SHE GOES —".to_string()
    } else if off > 0.0 {
        format!(
            "{} HELM TO STARBOARD {:.0}",
            "▶".repeat(chevrons),
            off.abs()
        )
    } else {
        format!("HELM TO PORT {:.0} {}", off.abs(), "◀".repeat(chevrons))
    };
    ui.label(
        egui::RichText::new(text)
            .color(colour)
            .monospace()
            .size(13.0),
    );
}

/// How long he has left, as a bar that empties.
fn endurance_bar(ui: &mut egui::Ui, left: f32) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 7.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 1.0, egui::Color32::from_rgb(26, 27, 29));
    let mut filled = rect;
    filled.set_width(rect.width() * left.clamp(0.0, 1.0));
    // Amber while there is time, and red when there is not, because the colour is the
    // part of this that gets read out of the corner of an eye.
    let colour = if left > 0.35 {
        egui::Color32::from_rgb(214, 160, 56)
    } else {
        egui::Color32::from_rgb(222, 74, 58)
    };
    p.rect_filled(filled, 1.0, colour);
}

/// Four words across the middle of the screen.
///
/// Spaced out by hand, which is the whole look: the letters of a thing shouted are far
/// apart, and egui has no letter spacing. A dark band behind them so they are readable
/// over a bright sea, and the sea still showing through it so the player can go on
/// steering — which is the difference between this and a modal dialogue, and the reason
/// it is allowed to be this big.
fn big_letters(ctx: &egui::Context, shout: Shout, alpha: f32) {
    let (ink, band) = match shout.tone {
        Tone::Alarm => (egui::Color32::from_rgb(242, 96, 72), 170),
        Tone::Order => (egui::Color32::from_rgb(240, 200, 110), 120),
        Tone::Good => (egui::Color32::from_rgb(150, 214, 158), 140),
        Tone::Dead => (egui::Color32::from_rgb(176, 42, 34), 210),
    };
    let fade = |c: egui::Color32, a: f32| {
        egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a * 255.0) as u8)
    };
    let spaced: String = shout
        .words
        .chars()
        .map(|c| {
            if c == ' ' {
                "   ".to_string()
            } else {
                format!("{c} ")
            }
        })
        .collect();
    egui::Area::new("shout".into())
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, -40.0))
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(egui::Color32::from_rgba_unmultiplied(
                    6,
                    7,
                    9,
                    (band as f32 * alpha) as u8,
                ))
                .inner_margin(egui::Margin::symmetric(44, 18))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new(spaced.trim_end())
                                .color(fade(ink, alpha))
                                .monospace()
                                .size(46.0),
                        );
                        if !shout.under.is_empty() {
                            ui.add_space(2.0);
                            ui.label(
                                egui::RichText::new(shout.under)
                                    .color(fade(egui::Color32::from_rgb(214, 218, 222), alpha))
                                    .monospace()
                                    .size(15.0),
                            );
                        }
                    });
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every key the lesson asks for is a key the program reads.
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
    // above them claims.
    #[allow(clippy::assertions_on_constants)]
    fn the_drill_waits_for_a_boat_that_is_actually_sailing() {
        assert_eq!(Stage::default(), Stage::Off);
        assert!(SAILING > 3.0, "a boat doing {SAILING:.1} knots is drifting");
        assert!(
            (3.0..12.0).contains(&HOLD),
            "{HOLD:.0} seconds is not a countdown"
        );
        // The pattern's own numbers: a close reach is a close reach, and the gate is far
        // enough out that the approach along it is a leg and not a lurch.
        assert!(
            (40.0..60.0).contains(&APPROACH_ANGLE),
            "{APPROACH_ANGLE:.0} degrees off the wind is not a close reach"
        );
        assert!(GATE_LENGTHS > 2.0 && CLEAR_LENGTHS > 2.0);
    }

    /// The approach is a close reach, on the tack she is already on, from below him.
    ///
    /// This is the geometry of the whole pattern and it is the part that would be wrong
    /// silently: a sign slip puts the gate upwind of him, which coaches the player into
    /// running down on a casualty from windward at six knots — the exact thing the
    /// pattern exists to prevent.
    #[test]
    fn the_gate_is_downwind_of_him_and_the_approach_is_a_close_reach() {
        let loa = 10.4;
        for twd_deg in [0.0, 37.0, 180.0, 300.0] {
            for side in [1.0_f64, -1.0] {
                let twd = rad(twd_deg);
                let oscar = DVec2::new(120.0, -40.0);
                let here = gate(oscar, twd, side, loa);
                let course = approach_course(twd, side);
                // The approach runs from the gate to him.
                let along = (oscar - here).normalize();
                let want = DVec2::new(course.sin(), course.cos());
                assert!(
                    (along - want).length() < 1e-9,
                    "the gate is not on the approach course"
                );
                // That course is a close reach: fifty degrees off the true wind.
                let off = wrap_pi(twd - course).abs();
                assert!(
                    (off - rad(APPROACH_ANGLE)).abs() < 1e-9,
                    "the approach is {:.1} degrees off the wind",
                    deg(off)
                );
                // With the wind on the side she is already on, so the last leg needs
                // neither a tack nor a gybe.
                assert_eq!(
                    wrap_pi(twd - course).signum(),
                    side,
                    "the approach puts the wind on the other side"
                );
                // And the gate is downwind of him: further along the way the wind is
                // blowing than he is.
                let downwind = DVec2::new(-twd.sin(), -twd.cos());
                assert!(
                    (here - oscar).dot(downwind) > 0.0,
                    "the gate is upwind of him, which coaches running him down"
                );
                // A sensible distance off, not a mile.
                assert!(((2.0 * loa)..(5.0 * loa)).contains(&(here - oscar).length()));
            }
        }
    }

    /// The approach is checked against the wind, which is the fault this fixed.
    ///
    /// Reported from the deck: *following the orders it put me ten metres from him,
    /// exactly upwind, and said steady as she goes.* It did — the last leg pointed the
    /// arrow at him and never asked whether that course could be sailed, or whether
    /// sailing it meant bearing down on a man in the water with the wind behind her.
    #[test]
    fn she_is_never_told_to_steer_a_course_she_cannot_sail() {
        for twd_deg in [0.0, 73.0, 180.0, 294.0] {
            let twd = rad(twd_deg);
            // Dead at the wind: no boat sails there and the drill must say so.
            assert_eq!(can_she_lay_him(twd, twd), Lay::DeadUpwind);
            assert_eq!(can_she_lay_him(twd, twd + rad(30.0)), Lay::DeadUpwind);
            // Behind the wind: she is above him, running down on him, which no method of
            // recovery allows.
            assert_eq!(
                can_she_lay_him(twd, twd + rad(180.0)),
                Lay::FromWindward,
                "running down on him with the wind behind is not an approach"
            );
            assert_eq!(can_she_lay_him(twd, twd - rad(140.0)), Lay::FromWindward);
            // And a close reach either side is what the pattern is for.
            for side in [1.0_f64, -1.0] {
                assert_eq!(
                    can_she_lay_him(twd, approach_course(twd, side)),
                    Lay::Good,
                    "the pattern's own approach course is not sailable"
                );
            }
        }
    }

    /// From the gate, the course to the aim point is always one she can sail.
    ///
    /// Which is the claim the whole pattern rests on: if this were ever false the drill
    /// would send her to a place from which it then told her off.
    #[test]
    fn the_gate_is_a_place_she_can_approach_from() {
        let (loa, beam) = (10.4, 3.4);
        for twd_deg in [0.0, 37.0, 151.0, 300.0] {
            for side in [1.0_f64, -1.0] {
                let twd = rad(twd_deg);
                let oscar = DVec2::new(-60.0, 210.0);
                let from = gate(oscar, twd, side, loa);
                let course = bearing(from, aim_point(oscar, twd, beam));
                assert_eq!(
                    can_she_lay_him(twd, course),
                    Lay::Good,
                    "from the gate at {twd_deg:.0}/{side} she cannot lay him"
                );
            }
        }
    }

    /// The aim is below him, so he finishes to windward of her.
    ///
    /// The boom and the mainsail are on the leeward rail, and a boat makes leeway: aim at
    /// him exactly and she is set down on top of him, which is the one outcome worse than
    /// missing him.
    #[test]
    fn she_is_aimed_below_him_rather_than_at_him() {
        for twd_deg in [0.0, 88.0, 203.0] {
            let twd = rad(twd_deg);
            let oscar = DVec2::new(14.0, -3.0);
            let aim = aim_point(oscar, twd, 3.4);
            let downwind = DVec2::new(-twd.sin(), -twd.cos());
            assert!(
                (aim - oscar).dot(downwind) > 0.0,
                "the aim is upwind of him, which puts him under the bow"
            );
            // A boat's beam and a bit, not a hundred metres.
            assert!((aim - oscar).length() < 4.0);
        }
    }

    /// The side is the near one, so the run to leeward is the short way round.
    #[test]
    fn the_approach_comes_from_whichever_side_she_is_already_on() {
        let (twd, loa) = (rad(0.0), 10.4);
        let oscar = DVec2::ZERO;
        for side in [1.0_f64, -1.0] {
            let near = gate(oscar, twd, side, loa);
            // Standing right beside one gate, that is the one chosen.
            assert_eq!(
                better_side(near + DVec2::new(1.0, 1.0), oscar, twd, loa),
                side
            );
        }
    }

    /// A leg only ends when the boat has actually done the thing.
    #[test]
    fn the_legs_are_in_the_order_of_the_pattern() {
        assert_eq!(order_for(Leg::Clear).unwrap().0, "BEAR AWAY");
        assert_eq!(order_for(Leg::Tack).unwrap().0, "TACK NOW");
        // And the tack is never described as a gybe, because that is the one thing this
        // pattern exists to avoid.
        for leg in [Leg::Clear, Leg::Tack, Leg::Downwind, Leg::Approach] {
            let (words, under) = order_for(leg).unwrap();
            assert!(!words.to_lowercase().contains("gybe"));
            assert!(
                words.len() <= 16,
                "{words} is not four words across a screen"
            );
            assert!(!under.is_empty());
        }
    }
}
