//! A start line, a countdown, and four legs round two buoys.
//!
//! **The one thing sailing has that nothing else does is the start.** You cannot stop a
//! sailing boat and you cannot park her: she is always going somewhere, so the start of a
//! race is not a button but a *manoeuvre* — you spend the last two minutes making the
//! boat arrive at a line she must not touch early, at full speed, on the end of the line
//! you have decided is favoured. Everybody who has raced knows that the race is usually
//! decided there, and everybody who has not is astonished by it.
//!
//! So that is the whole mission: a line, a gun five minutes away, and a course that is
//! twenty minutes of sailing for somebody who is not very good yet.
//!
//! **Why windward-leeward and not a triangle.** The course is laid off the wind at the
//! moment it is set — a buoy dead to windward, another dead to leeward — which makes
//! every leg either a beat or a run and nothing in between. That is deliberate: those two
//! are the points of sail with something to decide on them. A reach is a lovely thing to
//! sail and there is nothing to do on it but steer, and a course made of reaches is a
//! course where the boat that started first wins.
//!
//! **What is not here.** No other boats yet — the fleet exists in `fleet.rs` and the
//! times that would make it a race are two fields on the wire — and no rules of the road
//! between them, because boats in this game do not collide and a protest nobody can
//! lodge is a rule nobody has. What is here is the clock, the line, the marks, and the
//! one rule that makes a start a start: over early and you come back.

use bevy::prelude::*;
use glam::DVec2;
use sail_core::units::rad;

use crate::sim::Sim;

/// How long from the gun that starts the countdown to the one that starts the race.
///
/// Five minutes, which is the classic sequence and is long enough to be a manoeuvre
/// rather than a reaction: you have time to sail away from the line, look at the wind,
/// decide which end is favoured and come back at it.
pub const COUNTDOWN: f64 = 300.0;

/// How long the line is, metres.
///
/// A hundred and forty: about fifteen boat lengths, which is what a club line is for a
/// handful of boats. Long enough that there is a choice of where to cross and short
/// enough that the choice matters.
const LINE_LONG: f64 = 140.0;

/// How far to windward the first mark is laid, metres.
///
/// **This is the whole length of the race.** Four legs of eight hundred metres is 3.2 km;
/// a thirty-six footer beating does about four knots made good to windward and runs at
/// six, so the two beats are eight minutes each and the two runs four and a half —
/// twenty-five minutes for somebody sailing it badly, twenty for somebody sailing it
/// properly, which is the number that was asked for.
const WINDWARD: f64 = 800.0;

/// How near a mark counts as rounding it.
///
/// Twenty-five metres. A rounding is a thing you do *round* the buoy and the game cannot
/// tell which side you passed without far more machinery than this is worth, so it asks
/// the honest easy question — were you there — and leaves the rest to the sailor.
const ROUNDED: f64 = 25.0;

/// Which part of the business she is in.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Stage {
    #[default]
    Idle,
    /// Counting down to the gun, with the seconds left.
    Before(f64),
    /// Racing, with the leg she is on.
    Racing,
    /// Over the line, and has to come back and cross again.
    Recalled,
    /// Done, with her time in seconds.
    Finished(f64),
}

/// The course, the clock, and where she has got to.
#[derive(Resource, Default)]
pub struct Race {
    pub stage: Stage,
    /// The line: the committee boat end first, then the pin.
    pub line: (DVec2, DVec2),
    /// The marks in the order they are rounded, finishing at the line.
    pub marks: Vec<DVec2>,
    /// Which one she is going to now.
    pub at: usize,
    /// Simulation time the gun went, and when she crossed.
    gun_at: f64,
    crossed_at: Option<f64>,
    /// What the deck is told, and until when.
    pub said: Option<(String, f64)>,
    /// Seconds to put on the clock instead of five minutes, for the knob below.
    shorten_to: Option<f64>,
    /// Asked for from the menu, and read by `run` on the next frame.
    ///
    /// The wish lives on the thing it is a wish about rather than in a resource of its
    /// own — partly because that is where it belongs, and partly because the settings
    /// panel is one parameter off the sixteen a system may take, and a resource that
    /// exists to carry two booleans is the one to lose.
    pub wants_start: bool,
    pub wants_stop: bool,
}

/// Which end of the line is worth starting at, and by how much.
///
/// **The favoured end is the one further to windward**, and it is not a matter of taste:
/// a line square to the wind gives both ends the same distance to the first mark, and a
/// line skewed by ten degrees gives one end twenty-five metres of free distance — which
/// at four knots is ten seconds, and ten seconds is the whole first beat.
///
/// Returns how far the committee end is to windward of the pin, in metres of free
/// distance: positive for the committee boat, negative for the pin.
pub fn favoured_by(line: (DVec2, DVec2), wind_from: f64) -> f64 {
    let (boat, pin) = line;
    let along = pin - boat;
    // How far up the wind one end is over the other: the component of the line along the
    // wind's own direction.
    let to_windward = DVec2::new(wind_from.sin(), wind_from.cos());
    -along.dot(to_windward)
}

/// Which side of the line she is on: positive to windward of it — the wrong side before
/// the gun.
///
/// One function, because the countdown, the recall and the finish all ask it and a race
/// where the line means two different things in two places is not a race.
pub fn across(line: (DVec2, DVec2), at: DVec2, wind_from: f64) -> f64 {
    let (boat, pin) = line;
    let along = (pin - boat).normalize_or_zero();
    // The line's own normal, pointed to windward.
    let up = DVec2::new(wind_from.sin(), wind_from.cos());
    let normal = DVec2::new(-along.y, along.x);
    let normal = if normal.dot(up) < 0.0 {
        -normal
    } else {
        normal
    };
    (at - boat).dot(normal)
}

/// And whether she is between the ends rather than past one of them.
///
/// Crossing the *extension* of the line is not crossing the line, which is the one piece
/// of pedantry every sailor has had an argument about.
pub fn between_the_ends(line: (DVec2, DVec2), at: DVec2) -> bool {
    let (boat, pin) = line;
    let along = pin - boat;
    let t = (at - boat).dot(along) / along.length_squared().max(1e-9);
    (0.0..=1.0).contains(&t)
}

impl Race {
    /// Lay a course off the wind, with the line to leeward of the first mark.
    ///
    /// The line is laid across the wind a little to leeward of where she is, so that the
    /// five minutes are spent somewhere she has to sail *back* from — which is what the
    /// five minutes are for.
    pub fn lay_out(&mut self, at: DVec2, wind_from: f64, now: f64) {
        let to_windward = DVec2::new(wind_from.sin(), wind_from.cos());
        let across = DVec2::new(to_windward.y, -to_windward.x);
        // The middle of the line, two cables to leeward of her: far enough that she has
        // to think about getting back to it.
        let middle = at - to_windward * 320.0;
        // **The line is never quite square**, and that is the point of having two ends.
        // Ten degrees is what a committee boat's anchor and a bit of wind shift give you
        // on any club line, and it is enough to make one end worth a boat length.
        let skew = rad(10.0);
        let lie = DVec2::new(
            across.x * skew.cos() - across.y * skew.sin(),
            across.x * skew.sin() + across.y * skew.cos(),
        );
        self.line = (
            middle - lie * LINE_LONG * 0.5,
            middle + lie * LINE_LONG * 0.5,
        );
        // Windward mark dead upwind of the line's middle, leeward mark on the line.
        let windward = middle + to_windward * WINDWARD;
        self.marks = vec![windward, middle, windward, middle];
        self.at = 0;
        self.stage = Stage::Before(COUNTDOWN);
        self.gun_at = now + COUNTDOWN;
        self.crossed_at = None;
        self.say("five minutes — the line is to leeward of you", now);
    }

    fn say(&mut self, words: &str, now: f64) {
        self.said = Some((words.to_string(), now + 8.0));
    }

    /// Whether anything is going on at all.
    pub fn running(&self) -> bool {
        !matches!(self.stage, Stage::Idle)
    }

    /// Her time, if she has one.
    pub fn time(&self) -> Option<f64> {
        match self.stage {
            Stage::Finished(t) => Some(t),
            _ => None,
        }
    }
}

/// The clock, the line and the marks, a frame at a time.
pub fn run(sim: Res<Sim>, mut race: ResMut<Race>, mut first: Local<bool>) {
    let now = sim.sim.state.time;
    // `SAIL_RACE=90` lays the course at the start and puts ninety seconds on the clock.
    // The same kind of knob as `SAIL_BRIG`, and for the same reason: the five minutes
    // before a gun are the part worth looking at, and a scripted camera cannot click a
    // menu. A bare `SAIL_RACE=1` gives the full sequence.
    if !*first {
        *first = true;
        if let Ok(value) = std::env::var("SAIL_RACE") {
            race.wants_start = true;
            race.shorten_to = value.parse::<f64>().ok().filter(|s| *s > 1.0);
        }
    }
    if std::mem::take(&mut race.wants_stop) {
        *race = Race::default();
        return;
    }
    if std::mem::take(&mut race.wants_start) {
        let wind = sim.sim.wind.sample(sim.sim.state.pos, now);
        let from = sail_core::wind::direction_of(wind);
        race.lay_out(sim.sim.state.pos, from, now);
        if let Some(left) = race.shorten_to.take() {
            race.gun_at = now + left;
            race.stage = Stage::Before(left);
        }
        return;
    }
    if !race.running() {
        return;
    }
    if race.said.as_ref().is_some_and(|(_, until)| now > *until) {
        race.said = None;
    }
    let here = sim.sim.state.pos;
    let wind = sim.sim.wind.sample(here, now);
    let from = sail_core::wind::direction_of(wind);
    let side = across(race.line, here, from);
    let between = between_the_ends(race.line, here);

    match race.stage {
        Stage::Before(_) => {
            let left = race.gun_at - now;
            race.stage = Stage::Before(left.max(0.0));
            // The warnings a committee gives, and the only ones worth giving: a minute
            // is when you commit to an end, and ten seconds is when you stop steering
            // for the line and start steering for speed.
            for (mark, words) in [(60.0, "one minute"), (10.0, "ten seconds")] {
                if (left - mark).abs() < 0.08 {
                    race.say(words, now);
                }
            }
            if left <= 0.0 {
                if side > 0.0 && between {
                    race.stage = Stage::Recalled;
                    race.say("over early — come back and cross again", now);
                } else {
                    race.stage = Stage::Racing;
                    race.say("the gun — go", now);
                }
            }
        }
        Stage::Recalled => {
            // Back across to the right side, and then she may start.
            if side < 0.0 {
                race.stage = Stage::Racing;
                race.crossed_at = None;
                race.say("cleared — you are racing", now);
            }
        }
        Stage::Racing => {
            // Crossing the line the first time is the start; after that the line is the
            // finish and only the last leg counts.
            if race.crossed_at.is_none() {
                if side > 0.0 && between {
                    race.crossed_at = Some(now);
                    race.say("across — first beat", now);
                }
                return;
            }
            let Some(&mark) = race.marks.get(race.at) else {
                return;
            };
            if (here - mark).length() < ROUNDED {
                race.at += 1;
                let left = race.marks.len() - race.at;
                let words = match left {
                    0 => "round and home — the line is the finish".to_string(),
                    1 => "last leg".to_string(),
                    n => format!("{n} legs to go"),
                };
                race.say(&words, now);
            }
            if race.at >= race.marks.len() && side < 0.0 && between {
                let took = now - race.gun_at;
                race.stage = Stage::Finished(took);
                race.say(
                    &format!("finished — {:.0}:{:02.0}", took / 60.0, took % 60.0),
                    now,
                );
                info!("race finished in {:.0} s", took);
            }
        }
        Stage::Idle | Stage::Finished(_) => {}
    }
}

#[cfg(test)]
mod the_line {
    use super::*;
    use sail_core::units::kn_to_ms;

    /// A line across the wind, committee boat to the east.
    fn line_across(wind_from: f64) -> (DVec2, DVec2) {
        let up = DVec2::new(wind_from.sin(), wind_from.cos());
        let across = DVec2::new(up.y, -up.x);
        (across * -70.0, across * 70.0)
    }

    /// **Which end is favoured is a measurement, not an opinion.**
    ///
    /// A line square to the wind gives both ends the same distance to the first mark; a
    /// line skewed by ten degrees gives one end twenty-odd metres of free distance, and
    /// at four knots that is ten seconds. Everybody who has raced has lost a start by
    /// getting this backwards.
    #[test]
    fn the_favoured_end_is_the_one_to_windward() {
        let wind = rad(0.0); // from the north
        let square = line_across(wind);
        assert!(
            favoured_by(square, wind).abs() < 1.0,
            "a square line has no favoured end"
        );
        // Swing the committee end to windward: it becomes the favoured one.
        let (boat, pin) = square;
        let skewed = (boat + DVec2::new(0.0, 24.0), pin);
        assert!(
            favoured_by(skewed, wind) > 20.0,
            "the end to windward should be favoured by about its own advantage"
        );
        // And the mirror image, the other way.
        let other = (boat, pin + DVec2::new(0.0, 24.0));
        assert!(favoured_by(other, wind) < -20.0);
    }

    /// Which side of the line she is on, and that crossing its *extension* is not
    /// crossing the line — the one piece of pedantry every sailor has argued about.
    #[test]
    fn the_line_has_a_windward_side_and_two_ends() {
        let wind = rad(0.0);
        let line = line_across(wind);
        // North of the line is to windward: the wrong side before the gun.
        assert!(across(line, DVec2::new(0.0, 30.0), wind) > 0.0);
        assert!(across(line, DVec2::new(0.0, -30.0), wind) < 0.0);
        // Between the ends, and well past one of them.
        assert!(between_the_ends(line, DVec2::new(0.0, 5.0)));
        assert!(!between_the_ends(line, DVec2::new(200.0, 5.0)));
    }

    /// **The course is twenty minutes of sailing**, which is the number that was asked
    /// for and the reason the legs are as long as they are.
    #[test]
    fn the_course_is_about_twenty_minutes() {
        let mut race = Race::default();
        race.lay_out(DVec2::ZERO, rad(0.0), 0.0);
        let legs: f64 = race
            .marks
            .windows(2)
            .map(|p| (p[1] - p[0]).length())
            .sum::<f64>()
            + (race.marks[0] - race.line.0.lerp(race.line.1, 0.5)).length();
        // Beating costs about forty per cent in distance sailed over the ground, which
        // is what tacking through ninety degrees does to a straight line.
        let upwind = WINDWARD * 2.0 * 1.4;
        let downwind = legs - WINDWARD * 2.0;
        let minutes = (upwind / kn_to_ms(5.2) + downwind / kn_to_ms(6.0)) / 60.0;
        assert!(
            (15.0..30.0).contains(&minutes),
            "the course works out at {minutes:.0} minutes"
        );
    }

    /// And she starts to leeward of the line, with something to sail back from.
    #[test]
    fn the_line_is_laid_to_leeward_of_her() {
        let mut race = Race::default();
        let wind = rad(0.0);
        race.lay_out(DVec2::ZERO, wind, 0.0);
        let middle = race.line.0.lerp(race.line.1, 0.5);
        assert!(
            across(race.line, DVec2::ZERO, wind) > 100.0,
            "she should start well to windward of her own start line"
        );
        assert!((middle - DVec2::ZERO).length() > 200.0);
        assert_eq!(race.stage, Stage::Before(COUNTDOWN));
    }
}

// ---------------------------------------------------------------------------------------
// The buoys, and what the deck is told.
// ---------------------------------------------------------------------------------------

/// A mark of the course, or an end of the line.
#[derive(Component)]
pub struct Buoy;

/// Lay the buoys when the course is laid, and take them up when it is over.
///
/// **A race is a thing in the water**, not a line on a chart: the whole of a start is
/// looking at a buoy and a committee boat and deciding which one you want to be near.
/// So the marks exist out there, they ride the sea like everything else that floats, and
/// the orange of them is the one colour in this game that is not in the weather.
pub fn dress(
    mut commands: Commands,
    race: Res<Race>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    origin: Res<crate::frame::RenderOrigin>,
    laid: Query<Entity, With<Buoy>>,
) {
    let want: Vec<DVec2> = if race.running() {
        // The two ends of the line, then the marks — and the leeward mark is the same
        // water as the middle of the line, so it is not laid twice.
        let mut all = vec![race.line.0, race.line.1];
        all.extend(race.marks.iter().take(1).copied());
        all
    } else {
        Vec::new()
    };
    if laid.iter().count() == want.len() {
        return;
    }
    for entity in &laid {
        commands.entity(entity).despawn();
    }
    if want.is_empty() {
        return;
    }
    let orange = materials.add(StandardMaterial {
        base_color: Color::srgb(0.90, 0.36, 0.06),
        perceptual_roughness: 0.75,
        ..default()
    });
    let black = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.10, 0.12),
        perceptual_roughness: 0.9,
        ..default()
    });
    // **A mark you cannot see is a mark you cannot sail to.**
    //
    // The first version was an inflatable: a cone a metre and a half high, which is what
    // a club lays and what everybody pictures. At two hundred metres in a drawn picture
    // seven hundred and twenty pixels wide it is *one orange pixel in a grey sea* — the
    // start was being sailed off the panel, with the thing the panel was about invisible.
    // Reported from the session photographing it, and it is a sailing fault rather than
    // a drawing one.
    //
    // So the marks are spar buoys, which is the other thing a club lays and the one a
    // committee uses when the water is open: a staff three metres out of the sea with a
    // black topmark on it and a float at the waterline. The topmark is the point — a
    // dark shape against the sky is visible at a distance where orange against grey
    // water is not, and that is exactly why every real one has one.
    let float = meshes.add(Sphere::new(0.55));
    let staff = meshes.add(Cylinder::new(0.07, 3.0));
    let topmark = meshes.add(Cone::new(0.42, 0.8));
    for at in want {
        commands
            .spawn((
                Buoy,
                crate::course::Floating { at, lift: 0.3 },
                Transform::from_translation(origin.to_bevy(at, 0.3)),
                Visibility::default(),
            ))
            .with_children(|mark| {
                mark.spawn((
                    Mesh3d(float.clone()),
                    MeshMaterial3d(orange.clone()),
                    Transform::default(),
                ));
                mark.spawn((
                    Mesh3d(staff.clone()),
                    MeshMaterial3d(orange.clone()),
                    Transform::from_xyz(0.0, 1.5, 0.0),
                ));
                mark.spawn((
                    Mesh3d(topmark.clone()),
                    MeshMaterial3d(black.clone()),
                    Transform::from_xyz(0.0, 3.2, 0.0),
                ));
            });
    }
}

/// What the deck is told: the clock, the line, and which end of it is worth having.
pub fn draw(
    mut contexts: bevy_egui::EguiContexts,
    race: Res<Race>,
    sim: Res<Sim>,
    chart: Res<crate::chart::Chart>,
) -> Result {
    use bevy_egui::egui;
    if !race.running() || chart.spread {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let screen = ctx.viewport_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("race"),
    ));
    let middle = screen.center().x;
    let top = screen.top() + 92.0;
    let here = sim.sim.state.pos;
    let now = sim.sim.state.time;
    let wind = sim.sim.wind.sample(here, now);
    let from = sail_core::wind::direction_of(wind);

    let line = match race.stage {
        Stage::Before(left) => {
            // How far she is from the line, and how long at this speed — which is the
            // only arithmetic a start is made of, and the reason a good one looks like
            // somebody who knew what time it was.
            let to_line = distance_to_the_line(race.line, here);
            let speed = sim.sim.telemetry.stw.max(0.1);
            let run = to_line / speed;
            format!(
                "{:.0}:{:02.0} to the gun — {:.0} m to the line, {:.0} s at this speed",
                left / 60.0,
                left % 60.0,
                to_line,
                run
            )
        }
        Stage::Recalled => "OVER EARLY — go back across the line".to_string(),
        Stage::Racing => {
            let legs = race.marks.len().saturating_sub(race.at);
            match race.marks.get(race.at) {
                Some(mark) => {
                    let off = *mark - here;
                    format!(
                        "mark bears {:03.0}, {:.0} m — {} to go",
                        off.x
                            .atan2(off.y)
                            .rem_euclid(std::f64::consts::TAU)
                            .to_degrees(),
                        off.length(),
                        if legs == 1 {
                            "one leg".into()
                        } else {
                            format!("{legs} legs")
                        }
                    )
                }
                None => "home — cross the line".to_string(),
            }
        }
        Stage::Finished(_) => match race.time() {
            Some(t) => format!("finished in {:.0}:{:02.0}", t / 60.0, t % 60.0),
            None => String::new(),
        },
        Stage::Idle => String::new(),
    };
    painter.text(
        egui::pos2(middle, top),
        egui::Align2::CENTER_CENTER,
        line,
        egui::FontId::monospace(15.0),
        egui::Color32::from_rgb(214, 210, 198),
    );
    // Which end to start at, before the gun and never after it: it is the one decision
    // of the five minutes and it stops being a question the moment they go.
    if matches!(race.stage, Stage::Before(_)) {
        let favour = favoured_by(race.line, from);
        let words = if favour.abs() < 8.0 {
            "the line is square — take either end".to_string()
        } else {
            format!(
                "{} end favoured by {:.0} m",
                if favour > 0.0 {
                    "committee boat"
                } else {
                    "pin"
                },
                favour.abs()
            )
        };
        painter.text(
            egui::pos2(middle, top + 20.0),
            egui::Align2::CENTER_CENTER,
            words,
            egui::FontId::monospace(13.0),
            egui::Color32::from_rgb(176, 170, 156),
        );
    }
    if let Some((said, _)) = &race.said {
        painter.text(
            egui::pos2(middle, top + 42.0),
            egui::Align2::CENTER_CENTER,
            said,
            egui::FontId::monospace(17.0),
            egui::Color32::from_rgb(232, 176, 58),
        );
    }
    Ok(())
}

/// How far she is from the line itself — the segment, not its extension.
pub fn distance_to_the_line(line: (DVec2, DVec2), at: DVec2) -> f64 {
    let (boat, pin) = line;
    let along = pin - boat;
    let t = ((at - boat).dot(along) / along.length_squared().max(1e-9)).clamp(0.0, 1.0);
    (at - (boat + along * t)).length()
}
