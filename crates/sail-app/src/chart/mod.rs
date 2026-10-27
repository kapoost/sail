//! The chart.
//!
//! An island came over the horizon, the sounder went from forty metres to two, and she
//! was aground before anyone had decided anything. That is not a hard passage, it is a
//! missing instrument: no sailor crosses an archipelago without a chart, and the whole
//! point of the place — which side of that island, is the sound worth the wind shadow,
//! can I lay it — cannot be played at all if you can only see as far as the horizon.
//!
//! Drawn rather than rendered, because a chart is a *drawing*. The land is the shape
//! the physics uses, read straight off the same signed-distance field the boat runs
//! aground on, so what you are looking at and what you will hit are the same thing.
//! A chart that lies about the depths is worse than no chart; this one cannot, because
//! there is only one set of depths.
//!
//! North up, which is how a paper chart is read and is also the only orientation in
//! which you can hold a mental picture of a place over a whole passage.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use sail_core::land::OPEN_WATER;

use crate::course::Course;
use crate::sim::Sim;

/// The paper, the instruments on it, and the track she has left.
///
/// What this module does in a frame, in one place — see `stages.rs` for why the
/// schedule is named sets rather than one long chain.
pub fn plugin(app: &mut App) {
    use crate::stages::{Deck, Paint};
    app.init_resource::<Chart>();
    app.init_resource::<Track>();
    app.init_resource::<DeckLog>();
    app.add_systems(Update, record.in_set(Deck::Report));
    app.add_systems(Update, (toggle, stopwatch, by_hand).in_set(Deck::Weather));
    app.add_systems(
        bevy_egui::EguiPrimaryContextPass,
        draw.in_set(Paint::Instruments),
    );
    app.add_systems(bevy_egui::EguiPrimaryContextPass, glass.in_set(Paint::Over));
}

mod book;
mod instruments;
mod paper;
mod plot;

pub(crate) use book::*;
pub(crate) use instruments::*;
pub(crate) use paper::*;
pub(crate) use plot::*;

/// Is the chart out, and how far it is zoomed.
#[derive(Resource)]
pub struct Chart {
    pub open: bool,
    /// Spread out on the table rather than folded in the corner.
    ///
    /// A chart in a corner is a minimap: you glance at it and it tells you where you
    /// are. A chart on a table is an instrument: you put a straight edge on it and work
    /// something out. They are different objects and this is the switch between them.
    pub spread: bool,
    /// Somebody pressed the button that writes the log up.
    ///
    /// A flag rather than the writing itself, because the chart cannot see the glass,
    /// the sails or the sounder — the log line is made where those live and this is the
    /// chart asking for it.
    pub writing_up: bool,
    /// Which page of the book is open, and the entry its rubrics are filled from.
    pub page: Page,
    pub showing: Option<usize>,
    /// Whether each line on the paper says what it is and why.
    pub explain: bool,
    /// Half-width of what is shown, metres.
    pub range: f32,
    /// The dividers: where they were set down, and where they are now. In simulation
    /// coordinates, so they stay on the chart rather than on the screen when she moves.
    pub dividers: Option<(glam::DVec2, glam::DVec2)>,
    /// Which instrument is in hand.
    pub tool: Tool,
    /// What has been drawn on the chart with it.
    pub plot: Vec<Plotted>,
    /// Where the paper is lying, if it is not simply following her.
    ///
    /// **A chart centred on the boat is a chart that says where the boat is** — which is
    /// most of the exercise given away for nothing: recognise one mark and its position
    /// relative to the middle of the paper is your own position, measured with a ruler.
    /// Reported from the deck in one line.
    ///
    /// So while she is lost the paper lies where it is put, the way a chart on a table
    /// does, and moving it is dragging it with the right button. When she knows where
    /// she is it follows her again, because then it is a plotter and that is what a
    /// plotter is for.
    pub laid_at: Option<glam::DVec2>,
    /// The stopwatch: whether it is running, when it was started, and what it had on it
    /// before that.
    ///
    /// **A navigator's stopwatch is not a timer, it is an instrument of
    /// identification.** A light is named by its period — `Fl(2) 13s` on the chart means
    /// two flashes and then dark, the whole pattern repeating every thirteen seconds —
    /// and the way you tell one light from the next along the same coast is to start the
    /// watch on a flash, count, and stop it on the same flash some cycles later. It is
    /// also how you time a leg without doing arithmetic on the ship's clock, and how a
    /// racing navigator counts down to a start.
    pub watch: Stopwatch,
    /// Where the protractor is standing, while a bearing is being laid off.
    pub protractor: Option<glam::DVec2>,
    /// A leg being timed: where she was when the log was streamed, and when.
    pub streaming: Option<(glam::DVec2, f64, f64)>,
    /// Which ruled line is being moved, and by which end.
    ///
    /// A line you cannot move is a line you have to rub out and draw again, which is
    /// what a rubber is for and is not what a straight edge is for: on paper you put the
    /// edge down, look at it, and shift it a quarter of an inch. Asked for from the deck
    /// in exactly those terms.
    pub moving: Option<(usize, Grab)>,
    /// A fix the navigator has just said he believes. Taken by `navigation` the same
    /// frame, which is what decides whether it was worth believing.
    pub accepted: Option<glam::DVec2>,
}

/// The instruments on the chart table.
///
/// A real set, and a small one, because the real set *is* small: four things have been
/// doing this job since the eighteenth century and nobody has needed a fifth.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tool {
    /// Dividers. They measure distance, and they do nothing else at all — you walk them
    /// along the latitude scale and that is a mile a step.
    #[default]
    Dividers,
    /// A plain straight edge. Two points on the paper and a line between them, which
    /// is the oldest thing anybody does on a chart and the one the set was missing:
    /// the course from here to there, the edge of a shoal, the transit of two marks,
    /// the line you are not crossing. Asked for from the deck in two words.
    ///
    /// It is not the parallel rule with the mark left out. The parallel rule carries an
    /// *angle* from one place on the chart to another and that is all it is for; a
    /// ruler joins two places you can point at, and the bearing and distance are what
    /// you read off afterwards rather than what you set.
    Ruler,
    /// The parallel rule: a straight edge that walks across the chart without changing
    /// its angle, which is how a bearing gets from the compass rose onto the water you
    /// are looking at. Drag from a mark and it plots a position line through it.
    ParallelRule,
    /// The protractor: a rose you put down on a mark and read the bearing round.
    ///
    /// **The other way of laying off a bearing, and the one you use when you have a
    /// number rather than a direction.** With the rule you pull the line the way the
    /// mark bore and watch it; with this you stand the rose on the mark and turn until
    /// the figure under your hand is the figure you wrote down. Both are on a real chart
    /// table and neither replaces the other — which is why the rule stayed as it was
    /// when this arrived.
    ///
    /// Two clicks: the centre, then the bearing. And what it leaves is a **band** rather
    /// than a line, because that is what a bearing is — a hand compass is good to two or
    /// three degrees, so the honest answer is *she is somewhere in this corridor*, and
    /// drawing the corridor is the difference between a fix you believe and one you
    /// check.
    Protractor,
    /// The pencil. A cross for an estimated position, and a soft one, because a chart is
    /// corrected in ink and worked in pencil so that the working can be rubbed out.
    Pencil,
    /// And the rubber that rubs it out.
    Rubber,
    /// The log. Click once at the start of a leg and again at the end of it, and what
    /// goes on the paper is what she actually did between those two moments: the course
    /// made good, the distance run and the speed it comes to.
    ///
    /// **Which is not the same number as the one on the instruments**, and that is the
    /// whole reason for it. The log on the console reads speed *through the water*; two
    /// marks on a chart and a time between them give speed *over the ground*, and the
    /// difference between them is the tide, the leeway and whatever else has been
    /// quietly moving the boat. Nobody can measure that from inside the boat; you
    /// measure it by writing down where she was, waiting, and writing down where she is.
    ///
    /// Bounded by the hand rather than by a tack the program detects, because the thing
    /// being measured is *a leg you chose* — from the moment you settled on this board
    /// to the moment you put the helm down.
    Log,
    /// The watch. Not a drawing instrument at all — it writes the *time* where you put
    /// it, which is the one thing on a chart that cannot be worked out afterwards.
    ///
    /// Every mark a navigator makes carries a time beside it and the whole of dead
    /// reckoning is built on them: a fix with no time against it is a fix you cannot run
    /// on, because run on from *when*. Asked for from the deck, and it is the oldest
    /// habit in chartwork — the time goes on the paper as the pencil leaves it.
    Watch,
}

impl Tool {
    pub fn name(&self) -> &'static str {
        match self {
            Tool::Dividers => "DIVIDERS",
            Tool::Ruler => "RULER",
            Tool::ParallelRule => "PARALLEL RULE",
            Tool::Protractor => "PROTRACTOR",
            Tool::Pencil => "PENCIL",
            Tool::Rubber => "RUBBER",
            Tool::Watch => "WATCH",
            Tool::Log => "LOG",
        }
    }

    /// What it does, in the words of somebody showing you.
    pub fn hint(&self) -> &'static str {
        match self {
            Tool::Dividers => "drag across the chart: miles, and the bearing both ways",
            Tool::Ruler => "drag between two points: the leg, in pencil, with its bearing",
            Tool::ParallelRule => {
                "drag from a mark: a position line through it, at the bearing you drag"
            }
            Tool::Protractor => {
                "click a mark to stand the rose on it, then click the bearing you took"
            }
            Tool::Pencil => "click: a cross where you think you are",
            Tool::Rubber => "click a line or a cross to take it off the chart",
            Tool::Watch => "click: the time, written on the chart where you put it",
            Tool::Log => {
                "click to start a leg and again to end it: course, distance and speed \
                 made good"
            }
        }
    }
}

/// The instruments, in the order a hand reaches for them. One list, because the palette
/// is drawn from it and the click targets are worked out from it, and those two being
/// two lists is how a button ends up somewhere other than where it looks.
pub const TOOLS: [Tool; 8] = [
    Tool::Dividers,
    Tool::Ruler,
    Tool::ParallelRule,
    Tool::Protractor,
    Tool::Pencil,
    Tool::Watch,
    Tool::Log,
    Tool::Rubber,
];

/// Which part of a ruled line a hand has hold of.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Grab {
    /// One end, which moves on its own: this is how a leg is adjusted to land on a mark.
    From,
    To,
    /// The whole of it, carrying the offset from where it was taken hold of, so the line
    /// does not jump to the cursor the moment it is picked up.
    Whole(glam::DVec2),
}

/// Something drawn on the chart in pencil.
#[derive(Clone, Copy, Debug)]
pub enum Plotted {
    /// A position line: through this point, at this bearing. Drawn both ways from the
    /// mark, because a position line is a line and not an arrow — you are somewhere on
    /// it and the mark does not say which side.
    ///
    /// **With the speed that was known when it was drawn**, and not the speed she is
    /// doing now. A line plus a speed is a running fix waiting to happen, so the figure
    /// belongs on the paper — but it belongs to the moment the bearing was taken. Read
    /// off the live log instead, the number on an hour-old line changes every time she
    /// trims, which is a chart telling you something about the present through a mark
    /// that is about the past. Written down here it stops moving, like ink.
    Line {
        through: glam::DVec2,
        bearing: f64,
        speed: Option<f64>,
    },
    /// A mark at a point: a fix that has been entered in the log, or whatever the
    /// pencil was put down for — an estimated position, a danger, a place to come back
    /// to.
    ///
    /// `fix` decides which of the two it is drawn as, and that is the convention off a
    /// real chart rather than decoration: **a fix is a dot with a circle round it** and
    /// anything else is a dot with a cross. The dot is the position; the ring or the
    /// arms are only there to make the dot findable among the pencil. Which is the whole
    /// point of the shape — asked for from the deck as *mark it precisely*, because an
    /// X drawn with two strokes has its position somewhere in the middle of the ink and
    /// nowhere exactly.
    Cross { at: glam::DVec2, fix: bool },
    /// A ruled line between two points: a leg, an edge, a transit. Ends where it was
    /// drawn to end, which is the whole difference between it and a position line —
    /// a position line has no ends, and this is a line *from here to there*.
    Rule { from: glam::DVec2, to: glam::DVec2 },
    /// A time written on the paper: ship's time, in the four figures a deck log is kept
    /// in, with a tick under it to say which mark it belongs to.
    Stamp { at: glam::DVec2, hour: f32 },
    /// A leg that was actually sailed: where she was, where she got to, and how long it
    /// took her. The course, the distance and the speed are worked out from those three
    /// rather than stored, because a number written down beside the two points it came
    /// from is a number that can disagree with them.
    Run {
        from: glam::DVec2,
        to: glam::DVec2,
        minutes: f64,
    },
    /// A line of the deck log, written on the chart where it happened.
    Entry(Entry),
}

/// What a skipper writes down when he writes anything down.
///
/// **A fix is not a dot, it is a line in the log**, and the line is the useful part: the
/// position is where she was, and everything beside it is what she was *doing* and what
/// the day was doing to her. A week later the dot tells you nothing and the line tells
/// you the glass was falling, you were down to the second reef, and you were still
/// making four and a half — which is how a passage is read afterwards and why the book
/// is kept at all.
///
/// Written at the two moments a skipper writes: when he fixes, and when he alters.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub at: glam::DVec2,
    /// Ship's time, in hours.
    pub hour: f32,
    /// The course she settled on, when the entry is an alteration.
    pub course: Option<f64>,
    /// What the log said, if it had been read recently enough to be worth writing down.
    pub speed: Option<f64>,
    /// The glass, and how fast it is going which way — the number a skipper writes
    /// before he writes anything else, because it is the only one that is about the
    /// next six hours rather than this minute.
    pub glass: f64,
    pub tendency: f64,
    /// What was under her when she wrote it.
    ///
    /// **A sounding is a position line.** It is the oldest one there is — you are
    /// somewhere on the twenty-metre contour — and it is the only instrument on this
    /// boat that goes on reading when the fog comes down and the marks and the sun and
    /// the horizon have all gone. A log line without it is a log line missing the one
    /// number that was still available.
    pub sounding: f64,
    /// What he could see: where the wind was from and how hard, and the sea with it.
    pub wind_from: f64,
    pub wind: f64,
    pub sea: f64,
    /// The set she has measured, if she has measured any, and how far out the
    /// reckoning may be. The two halves of *how much to believe this line* — and the
    /// only two figures here that are about the navigator rather than the weather.
    pub set: Option<f64>,
    pub within: Option<f64>,
    /// And what was set when he wrote it.
    pub reef: f64,
    pub headsail: sail_core::state::Headsail,
    pub furl: f64,
}

/// How much pencil a chart carries before it stops being a plot.
///
/// A dozen. Past that it is a scribble and the oldest of it is about water she left an
/// hour ago — which is also why the oldest is what goes.
const PAPER_HOLDS: usize = 12;

impl Chart {
    /// **The player's own hand**: a mark he has chosen to put at a place on the paper.
    ///
    /// Allowed whether or not she knows where she is, because none of it is a claim
    /// about where she is — a danger ringed, a time written, a bearing laid off from a
    /// charted mark and a leg ruled between two points are all things a lost navigator
    /// does, and most of them are how he stops being lost.
    pub fn pencil(&mut self, item: Plotted) {
        self.plot.push(item);
        if self.plot.len() > PAPER_HOLDS {
            self.plot.remove(0);
        }
    }

    /// **Anything worked out from her own position**, and the only door it comes
    /// through.
    ///
    /// The one invariant this chart has is that it never draws a position she has not
    /// established, and it has been broken twice by the same mistake: the ink was added
    /// in one place and the condition on it lived in another. So the condition is a
    /// parameter of the door. It can be answered wrongly, but it cannot be forgotten,
    /// and there is one line to read to see what the answer was.
    pub fn write_down(&mut self, item: Plotted, knows: bool) {
        if !knows {
            return;
        }
        self.pencil(item);
    }
}

impl Default for Chart {
    fn default() -> Self {
        Self {
            open: true,
            spread: false,
            writing_up: false,
            page: Page::default(),
            showing: None,
            explain: false,
            dividers: None,
            tool: Tool::Dividers,
            plot: Vec::new(),
            laid_at: None,
            moving: None,
            streaming: None,
            protractor: None,
            watch: Stopwatch::default(),
            accepted: None,
            // Two and a half miles each way: far enough to see the next island and the
            // one after it, close enough that a shoal is a shape rather than a dot.
            range: 4_600.0,
        }
    }
}

/// Colours, chosen to be read at a glance rather than to be pretty. A chart is a
/// working document.
mod ink {
    use bevy_egui::egui::Color32;
    pub const PAPER: Color32 = Color32::from_rgb(222, 214, 192);
    pub const DEEP: Color32 = Color32::from_rgb(168, 190, 204);
    pub const SHOAL: Color32 = Color32::from_rgb(126, 176, 186);
    pub const LAND: Color32 = Color32::from_rgb(206, 196, 160);
    pub const COAST: Color32 = Color32::from_rgb(70, 66, 54);
    pub const GRID: Color32 = Color32::from_rgb(150, 150, 140);
    pub const BOAT: Color32 = Color32::from_rgb(184, 36, 28);
    pub const MARK: Color32 = Color32::from_rgb(196, 112, 24);
    pub const TRACK: Color32 = Color32::from_rgb(120, 60, 140);
    /// What the weather is written on with. A navigator corrects a chart in ink and
    /// pencils the forecast on top of it, and the forecast is the part that is only
    /// true today — so it is a different hand from the coastline, and it reads as
    /// something added rather than something printed.
    pub const PEN: Color32 = Color32::from_rgb(40, 52, 104);
    pub const SQUALL: Color32 = Color32::from_rgb(96, 74, 120);
    /// Magenta, and not by taste: everything on a real chart that is *a light* or a mark
    /// put there by people rather than by geology is printed in magenta, so that it can
    /// be picked out under a red torch at night without reading a word.
    pub const LIGHT: Color32 = Color32::from_rgb(168, 42, 128);
    /// What the working is done in. A chart is corrected in ink and worked in pencil, so
    /// that the working can be rubbed out and the coastline cannot.
    pub const PENCIL: Color32 = Color32::from_rgb(64, 64, 70);
}

/// The chart as it is actually read after dark: dim, and red.
///
/// **A white chart at night is a flashbang.** Twenty minutes of dark adaptation goes in
/// the second it comes up, and then there is no coast out there any more — which is
/// exactly the half of night pilotage a game can get wrong without anybody noticing,
/// because the player's room is lit. Every chart table at sea is lit red for this reason,
/// and every chart is printed so that it can be read under one: that is why lights and
/// everything else put there by people are magenta, which red light turns to black lines
/// on a pale ground while blue water goes dark and even.
///
/// Drawn as a torch rather than as a second palette: one veil over the finished chart,
/// darkening it and taking the colour out, instead of a night-coloured twin of every ink
/// on the paper. It is the same thing physically — the paper has not changed, the light
/// on it has — and it cannot drift out of step with the day version, because there is
/// only one.
///
/// It follows the sun, on the same curve as her own lamps and the lights on the coast.
fn under_a_red_torch(t: &Table, sky: &crate::sky::Sky) {
    let dark = after_dark(sky);
    if dark < 0.02 {
        return;
    }
    // Down first, then red. Two passes because one colour cannot both take light away
    // and put a colour back.
    torch(&t.p, t.rect, dark);
}

/// The torch itself.
///
/// Only the paper goes under it, and that is the right line to draw: a chart is paper and
/// takes the colour of whatever is shining on it, while the console is an instrument with
/// its own backlight and keeps its own dim amber — which is what a lit binnacle does at
/// sea. Putting everything under the torch would be prettier and wrong.
pub(crate) fn torch(p: &egui::Painter, rect: egui::Rect, dark: f32) {
    // **One veil, and it is dark with the red already in it.** Two passes were tried —
    // black to take the light away, then red to put the colour back — and they fight:
    // the red pass lifts what the black pass took down and the chart comes out pink,
    // washed and lower in contrast than it started. A single dark red does both at once.
    //
    // It is a dimmer rather than a filter, and that is an honest limit of painting with
    // alpha: a real red torch *removes* the blue and the green, which is a multiply, and
    // nothing here can multiply. The proper version of this is a second palette — every
    // ink on the paper given a night value — and it is a wider change than one rectangle
    // because a hundred and fifty-odd colours are named across five files.
    p.rect_filled(
        rect,
        4.0,
        egui::Color32::from_rgba_unmultiplied(30, 5, 3, (188.0 * dark) as u8),
    );
}

/// How dark it is, as the lamps and the lights on the coast measure it.
pub(crate) fn after_dark(sky: &crate::sky::Sky) -> f32 {
    1.0 - crate::curve::smoothstep(
        (-3.0_f32).to_radians(),
        (4.0_f32).to_radians(),
        sky.elevation(),
    )
}

pub fn toggle(keys: Res<ButtonInput<KeyCode>>, mut chart: ResMut<Chart>) {
    // Shift and M spreads it on the table; M alone folds the corner one away. Two states
    // of one chart rather than two charts, because a boat has one.
    if keys.just_pressed(KeyCode::KeyM)
        && (keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight))
    {
        chart.spread = !chart.spread;
        if chart.spread {
            chart.open = true;
        }
        return;
    }
    if keys.just_pressed(KeyCode::KeyM) {
        // **Three states and one invariant: spread means open.**
        //
        // `M` used to toggle `open` and leave `spread` alone, so a chart that had been
        // on the table and was then hidden stayed *spread* with nothing on the screen —
        // and `spread` is what tells the glasses and the compass that your hands are on
        // the paper. The instruments went dead for the rest of the session with no chart
        // in sight to explain it, which is how it was reported: the shortcut stopped
        // working after a while.
        //
        // So from the table, `M` folds it to the corner; from the corner it puts it
        // away; from away it brings the corner back. One key, one way round, and the
        // two fields can no longer disagree.
        if chart.spread {
            chart.spread = false;
        } else {
            chart.open = !chart.open;
        }
        if crate::knob::var("SAIL_KEYS").is_ok() {
            info!("chart: M -> open {} spread {}", chart.open, chart.spread);
        }
    }
    if chart.open {
        if keys.just_pressed(KeyCode::BracketRight) {
            chart.range = (chart.range * 0.72).max(700.0);
        }
        if keys.just_pressed(KeyCode::BracketLeft) {
            chart.range = (chart.range / 0.72).min(16_000.0);
        }
    }
}

/// Where the boat has been, so a passage reads as a passage.
#[derive(Resource, Default)]
pub struct Track {
    points: Vec<glam::DVec2>,
    since: f64,
}

impl Track {
    /// Forget where she has been. For when the water under her is not the same water —
    /// joining somebody else's game puts her in a different archipelago, and a track
    /// drawn across it is a line through islands she was never near.
    pub fn clear(&mut self) {
        self.points.clear();
        self.since = 0.0;
    }
}

/// One fix every few seconds, which is what a log book gets anyway.
pub fn record(
    sim: Res<Sim>,
    nav: Res<crate::navigation::Navigation>,
    time: Res<Time>,
    mut track: ResMut<Track>,
) {
    // Nothing is written down while she does not know where she is: a track kept
    // through the exercise is the answer, written in advance, waiting for the moment the
    // chart is allowed to draw it.
    if !nav.shows_position(sim.sim.state.time) {
        track.since = 0.0;
        return;
    }
    track.since += time.delta_secs_f64();
    if track.since < 6.0 {
        return;
    }
    track.since = 0.0;
    track.points.push(sim.sim.state.pos);
    // A few hours of it. Unbounded history is the sort of thing that is invisible
    // until the day somebody leaves the game running overnight.
    if track.points.len() > 2_400 {
        track.points.remove(0);
    }
}

/// The leg being timed, while it is being timed.
///
/// A line from where the log was streamed to where she is now, with the figures
/// running. Without it the tool is a button that appears to do nothing for ten minutes
/// — which is how it came to be reported as unusable from the deck: a leg takes minutes
/// to become visible and a button has to answer in the same second.
pub(crate) fn the_cast(
    t: &Table,
    streaming: Option<(glam::DVec2, f64, f64)>,
    now: f64,
    pos: glam::DVec2,
    hour: f32,
) {
    // The leg being timed, while it is being timed: a line from where the log
    // was streamed to where she is now, with the figures running. Without it the
    // tool is a button that appears to do nothing for ten minutes.
    if let Some((was, when, _)) = streaming.filter(|_| t.knows) {
        let (a, b) = (t.to_screen(was), t.to_screen(pos));
        t.p.line_segment([a, b], egui::Stroke::new(1.0, ink::MARK));
        // Where it was streamed, marked the moment it is streamed. Without this
        // the first click put a line of zero length on the paper and looked like
        // nothing happening at all, which is how the tool came to be reported as
        // unusable: a leg takes minutes to become visible and a button has to
        // answer in the same second.
        t.p.circle(
            a,
            4.0,
            egui::Color32::TRANSPARENT,
            egui::Stroke::new(1.2, ink::MARK),
        );
        t.p.circle_filled(a, 1.3, ink::MARK);
        t.p.text(
            a + egui::vec2(7.0, -6.0),
            egui::Align2::LEFT_BOTTOM,
            format!(
                "log {:02.0}{:02.0}",
                hour.floor(),
                (hour.fract() * 60.0).floor()
            ),
            egui::FontId::monospace(10.0),
            ink::MARK,
        );
        let off = pos - was;
        let minutes = (now - when) / 60.0;
        let miles = off.length() / 1_852.0;
        t.p.text(
            b + egui::vec2(8.0, 10.0),
            egui::Align2::LEFT_TOP,
            format!(
                "log streamed — {:.2} M, {:.0} min, {:.1} kn",
                miles,
                minutes,
                miles / (minutes / 60.0).max(1e-6)
            ),
            egui::FontId::monospace(10.0),
            ink::MARK,
        );
    }
}

/// What the hand on the chart asked for this frame — and nothing done about it yet.
///
/// **The chart is one widget.** Eight instruments, the buttons and the water all come
/// through the same press, so a tool that acted the moment it decided something was a
/// tool that could act twice, or act on a click that belonged to a button drawn over it
/// — which is how picking up the rubber came to rub something out. So the tool in hand
/// says what it *would* do and `draw` does it afterwards, in one place, in an order you
/// can read.
#[derive(Default)]
pub(crate) struct Asked {
    pub dividers: Option<(glam::DVec2, glam::DVec2)>,
    pub clear_dividers: bool,
    pub draw_on: Option<Plotted>,
    pub rub_out: Option<glam::DVec2>,
    pub take_hold: Option<(usize, Grab)>,
    pub shift: Option<(usize, Grab, glam::DVec2)>,
    pub let_go: bool,
    pub stream: Option<bool>,
    pub protractor: Option<Option<glam::DVec2>>,
    pub slip: Option<f64>,
    pub timed: Option<(f64, f64)>,
    pub out_by: Option<f64>,
}

/// What a drag means depends on what is in your hand, which is how a chart table works:
/// the dividers measure, the rule plots, the pencil marks and the rubber takes it off
/// again.
pub(crate) fn in_hand(
    t: &Table,
    chart: &Chart,
    nav: &crate::navigation::Navigation,
    response: &egui::Response,
    on_furniture: bool,
    sim: &Sim,
    hour: f32,
) -> Asked {
    // Where she is and what the clock says: what the log needs to stamp a leg with, and
    // the only two things in here that are about the boat rather than about the hand.
    let (now, pos) = (sim.sim.state.time, sim.sim.state.pos);
    let mut asked = Asked::default();
    // What a drag means depends on what is in your hand, which is how a chart
    // table works: the dividers measure, the rule plots, the pencil marks and
    // the rubber takes it off again.
    match chart.tool {
        // Two clicks: the rose down, then the bearing.
        Tool::Protractor => {
            if response.clicked() && !on_furniture {
                if let Some(at) = response.interact_pointer_pos() {
                    let water = t.to_water(at);
                    match chart.protractor {
                        None => asked.protractor = Some(Some(water)),
                        Some(centre) => {
                            let off = water - centre;
                            if off.length() > chart.range as f64 * 0.01 {
                                asked.draw_on = Some(Plotted::Line {
                                    through: centre,
                                    bearing: off.x.atan2(off.y).rem_euclid(std::f64::consts::TAU),
                                    speed: nav.measured.map(|(knots, _)| knots),
                                });
                            }
                            asked.protractor = Some(None);
                        }
                    }
                }
            }
        }
        Tool::Dividers | Tool::ParallelRule | Tool::Ruler => {
            // **Taking hold of a line that is already there.**
            //
            // With the straight edge in hand, a drag that starts on an existing
            // ruled line moves it rather than drawing a second one across it: an
            // end on its own, so a leg can be walked onto a mark, or the whole of
            // it if the hand lands somewhere along it. Which is what a straight
            // edge is for — you put it down, look at it, and shift it a quarter
            // of an inch — and the alternative is rubbing it out and drawing it
            // again, which is what the rubber is for and is not the same job.
            if chart.tool == Tool::Ruler && response.drag_started() && !on_furniture {
                if let Some(at) = response.interact_pointer_pos() {
                    let water = t.to_water(at);
                    // A finger's width on the paper, so a line can be caught
                    // without the pointer having to be on the pencil mark.
                    let near = (12.0 / t.scale) as f64;
                    asked.take_hold = chart.plot.iter().enumerate().find_map(|(i, item)| {
                        let Plotted::Rule { from, to } = item else {
                            return None;
                        };
                        if (water - *from).length() < near {
                            Some((i, Grab::From))
                        } else if (water - *to).length() < near {
                            Some((i, Grab::To))
                        } else if on_the_line(*from, *to, water) < near {
                            Some((i, Grab::Whole(water - *from)))
                        } else {
                            None
                        }
                    });
                }
            }
            if let (Some((i, grab)), Some(at)) = (chart.moving, response.interact_pointer_pos()) {
                if response.dragged() {
                    asked.shift = Some((i, grab, t.to_water(at)));
                }
                if response.drag_stopped() {
                    asked.let_go = true;
                }
            } else if response.drag_started() && !on_furniture {
                if let Some(at) = response.interact_pointer_pos() {
                    asked.dividers = Some((t.to_water(at), t.to_water(at)));
                }
            } else if response.dragged() {
                if let (Some(at), Some((from, _))) =
                    (response.interact_pointer_pos(), chart.dividers)
                {
                    asked.dividers = Some((from, t.to_water(at)));
                }
            } else if response.drag_stopped() {
                // The rule leaves its line on the chart; the dividers leave
                // nothing, because dividers are for reading and not for drawing.
                if let Some((from, to)) = chart.dividers {
                    let off = to - from;
                    // A twitch of the hand is not a line. Below a tenth of an
                    // inch of paper it was meant as a click.
                    if off.length() > chart.range as f64 * 0.01 {
                        if chart.tool == Tool::ParallelRule {
                            asked.draw_on = Some(Plotted::Line {
                                through: from,
                                bearing: off.x.atan2(off.y).rem_euclid(std::f64::consts::TAU),
                                speed: nav.measured.map(|(knots, _)| knots),
                            });
                        } else if chart.tool == Tool::Ruler {
                            asked.draw_on = Some(Plotted::Rule { from, to });
                        }
                    }
                }
                asked.clear_dividers = true;
            } else if response.clicked() {
                asked.clear_dividers = true;
            }
        }
        Tool::Pencil => {
            if response.clicked() && !on_furniture {
                if let Some(at) = response.interact_pointer_pos() {
                    asked.draw_on = Some(Plotted::Cross {
                        at: t.to_water(at),
                        fix: false,
                    });
                }
            }
        }
        Tool::Rubber => {
            if response.clicked() && !on_furniture {
                if let Some(at) = response.interact_pointer_pos() {
                    asked.rub_out = Some(t.to_water(at));
                }
            }
        }
        Tool::Watch => {
            if response.clicked() && !on_furniture {
                if let Some(at) = response.interact_pointer_pos() {
                    asked.draw_on = Some(Plotted::Stamp {
                        at: t.to_water(at),
                        hour,
                    });
                }
            }
        }
        // Streaming the log and hauling it in again. Where on the paper the
        // click lands does not matter at all — what is being marked is *her*
        // position and the time, which is why this is the one tool that is not
        // about the point under the cursor.
        Tool::Log => {
            if response.clicked() && !on_furniture {
                match chart.streaming {
                    None => asked.stream = Some(true),
                    Some(_) => {
                        asked.stream = Some(false);
                        let (line, speed, set) =
                            the_leg(chart.streaming, now, pos, nav.through_water, t.knows);
                        asked.draw_on = line;
                        asked.timed = speed;
                        asked.out_by = set;
                        if let Some((_, when, _)) = chart.streaming {
                            asked.slip = Some(((now - when) - CAST).abs() / CAST);
                        }
                    }
                }
            }
        }
    }
    asked
}

/// How big the folded chart is: **as wide as the instrument case under it**.
///
/// The chart in the corner and the binnacle below it are one column down the right of
/// the screen, and a column whose two halves are different widths reads as two things
/// that happen to be near each other rather than as one instrument panel. It was a flat
/// three hundred and thirty points against a case that comes out around four hundred,
/// and that step was the first thing the eye found on that side of the picture.
///
/// Measured rather than matched by hand: the case's own area reports its size from the
/// frame before, so a change to what the instruments show moves the chart with it and
/// there is no second number to keep in step. A frame of lag is invisible — the case only
/// changes size when the readings in it do.
///
/// **And it is allowed to be shorter than it is wide**, which the chart on the table is
/// not. The table wants a square because dividers walked across a paper with a different
/// scale up and across measure the wrong distance; the corner is a *glance* — you are
/// checking the water ahead, not stepping off a run — and its scale comes from its width
/// either way. So when the case is tall and the room above it is not square, what gives
/// is the height. The alternative was a chart narrower than the case it sits on, which is
/// the thing this is for.
fn folded(ctx: &egui::Context, screen: egui::Rect) -> egui::Vec2 {
    let case = egui::AreaState::load(ctx, egui::Id::new("corner")).and_then(|area| area.size);
    folded_to(case, screen.height())
}

/// The arithmetic of the above, without a window in it.
///
/// Split out for the usual reason: the shape of the corner is a thing a test can hold —
/// the widths must match, the chart must not reach down into the case, and neither of
/// them may be squeezed into a smear — and none of that needs egui to be running.
fn folded_to(case: Option<egui::Vec2>, screen_height: f32) -> egui::Vec2 {
    /// The size it was before any of this, and the fallback on the first frame.
    const AS_DRAWN: f32 = 330.0;
    /// Below this a chart in a corner is a decoration rather than a glance.
    const LEAST: f32 = 180.0;
    /// What the two of them keep between themselves and the edges.
    const MARGINS: f32 = 36.0;
    let width = case.map(|size| size.x).unwrap_or(AS_DRAWN).max(LEAST);
    let room = screen_height - case.map(|size| size.y).unwrap_or(0.0) - MARGINS;
    egui::vec2(width, width.min(room).max(LEAST))
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut contexts: EguiContexts,
    mut chart: ResMut<Chart>,
    sim: Res<Sim>,
    course: Res<Course>,
    track: Res<Track>,
    mut nav: ResMut<crate::navigation::Navigation>,
    mut log: ResMut<DeckLog>,
    coast: Res<crate::marks::Coast>,
    sky: Res<crate::sky::Sky>,
    fleet: Res<crate::fleet::Fleet>,
    job: Res<crate::mission::Pilotage>,
    race: Res<crate::race::Race>,
    home: Res<crate::anchorage::Home>,
    radio: Res<crate::radio::Radio>,
    brig: Res<crate::ship::Brig>,
) -> Result {
    if !chart.open {
        return Ok(());
    }
    let Some(land) = sim.sim.land.as_ref() else {
        return Ok(());
    };
    let ctx = contexts.ctx_mut()?;
    let st = &sim.sim.state;
    let screen = ctx.viewport_rect();
    let spread = chart.spread;
    // Whether the chart may say where she is.
    //
    // This is the whole of the navigation exercise and it is one boolean: a chart that
    // always knows is a plotter, and the question "which bay is that" cannot be asked of
    // one. Everything else is still drawn — the coast does not stop being there because
    // you are lost — and what goes is the boat, her track, and the line to the place she
    // is bound, which between them give the position away three times over.
    let knows = nav.shows_position(sim.sim.state.time);
    // Where the middle of the paper is.
    //
    // **The one in the corner is always under the boat** when the boat is a thing the
    // chart may draw, which is what a glance at a corner is for: you are not reading it,
    // you are checking the water ahead, and a window that has quietly been left behind
    // five miles astern is worse than no window. Reported from the deck after a fix went
    // in and the corner stayed where the fix was.
    //
    // Lost, it cannot follow her — the middle of the window would say where she is as
    // plainly as drawing her on it — so it stays on the paper as laid. And the chart on
    // the table always stays where it was put, because that is what a chart on a table
    // does: you lay it over the bit of coast you are working on and it waits there.
    let middle_of_paper = if knows && !spread {
        st.pos
    } else {
        chart.laid_at.unwrap_or(st.pos)
    };
    // Set inside the closure and applied after it: the painter borrows the chart.
    // Everything the hand on the paper asked for, which is applied once, below, after
    // the chart has been drawn.
    let mut asked = Asked::default();
    let mut pick: Option<Tool> = None;
    let mut explain: Option<bool> = None;
    let mut accept: Option<glam::DVec2> = None;
    let mut writing_up = false;
    let mut laid: Option<usize> = None;
    let mut turn_to: Option<Page> = None;
    let mut lay_at: Option<glam::DVec2> = None;
    let mut press_the_watch = false;
    let area = if spread {
        egui::Area::new("chart".into()).anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
    } else {
        egui::Area::new("chart".into()).anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 12.0))
    };
    area.show(ctx, |ui| {
        // On the table it takes the screen less a margin, and it is square, because
        // a chart with a different scale across than up is a chart you cannot put
        // dividers on.
        let size = if spread {
            let side = (screen.height() - 70.0).min(screen.width() - 70.0);
            egui::vec2(side, side)
        } else {
            folded(ctx, screen)
        };
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 4.0, ink::DEEP);

        let scale = rect.width() * 0.5 / chart.range;
        // **The paper, as a thing that can be handed to a function.**
        //
        // There were two closures here doing the conversion between water and screen,
        // and every piece of drawing that had not been lifted out yet used them. They
        // are gone: nothing in `draw` draws any more, so there is one conversion, on the
        // table, and no way for a second one to drift away from it.
        // Where a press landed, if this frame had one. Worked out once: every
        // instrument asks the same question and they must all get the same answer.
        let pressed = response
            .interact_pointer_pos()
            .filter(|_| response.clicked());
        let table = Table {
            p: p.clone(),
            rect,
            scale,
            middle: middle_of_paper,
            knows,
            why: chart.explain,
        };

        the_water(&table, land, &track, chart.range as f64, st.pos);

        marks_on_the_paper(
            &table,
            &coast,
            &nav,
            chart.range as f64,
            spread,
            &course,
            &home,
        );

        her_own_place(&table, &sim, &fleet, &course, &nav, spread);

        // --- Dividers and a straight edge --------------------------------
        //
        // Drag across the chart and it measures: the distance in miles and cables,
        // and the bearing both ways. That is the whole of what a pair of dividers and
        // a parallel rule do, and between them they are how every passage was planned
        // for four hundred years — *is that eight miles or twelve, and can I lay it
        // on this tack*.
        //
        // Only on the table. On the folded chart there is no room to put a hand down,
        // and a measurement you cannot read is worse than none.
        if spread {
            // **Where the furniture is, before anything reacts to a click.**
            //
            // The instruments and the log button are drawn *on* the chart, and the chart
            // is one widget — so a click on a button was also a click on the water under
            // it: picking up the rubber rubbed something out, and entering a fix in the
            // log left a pencil cross under the button that did it. Found in review.
            //
            // The slots are worked out here and the drawing below uses the same ones, so
            // what you can click and what you can see are the same rectangles.
            let mut furniture: Vec<egui::Rect> = Vec::new();
            let mut x = rect.left() + 24.0;
            for tool in TOOLS {
                let wide = 11.0 + tool.name().len() as f32 * 7.4;
                furniture.push(egui::Rect::from_min_size(
                    egui::pos2(x, rect.top() + 14.0),
                    egui::vec2(wide, 20.0),
                ));
                x += wide + 8.0;
            }
            // The legend switch, at the end of the row of instruments because that is
            // what it is: a thing you pick up when you want to know what you are
            // looking at.
            let why_button =
                egui::Rect::from_min_size(egui::pos2(x, rect.top() + 14.0), egui::vec2(48.0, 20.0));
            // Two of them, side by side: the paper and the book.
            let chart_button = egui::Rect::from_min_size(
                egui::pos2(rect.left() + 24.0, rect.top() + 66.0),
                egui::vec2(108.0, 22.0),
            );
            let log_button = egui::Rect::from_min_size(
                egui::pos2(rect.left() + 138.0, rect.top() + 66.0),
                egui::vec2(100.0, 22.0),
            );
            // The stopwatch is furniture too, and this is the same fault as the one
            // found in review before it: the instruments are drawn *on* the chart and
            // the chart is one widget, so a press on the watch was also a press on the
            // water under it — start the watch with the pencil in hand and it left a
            // cross under the dial. Worked out here with the rest of the furniture so
            // that what you can press and what you can see are the same rectangles.
            let watch_face = egui::Rect::from_center_size(
                egui::pos2(rect.right() - 62.0, rect.top() + 62.0),
                egui::vec2(74.0, 74.0),
            );
            let watch_button = egui::Rect::from_center_size(
                egui::pos2(rect.right() - 62.0, rect.top() + 110.0),
                egui::vec2(74.0, 18.0),
            );
            let on_furniture = response.interact_pointer_pos().is_some_and(|at| {
                furniture.iter().any(|slot| slot.contains(at))
                    || why_button.contains(at)
                    || chart_button.contains(at)
                    || log_button.contains(at)
                    || watch_face.contains(at)
                    || watch_button.contains(at)
            });

            // Moving the paper. The right button drags the chart about under the
            // instruments, which is what a hand does to a chart on a table and is the
            // only way to look at water she is not in — the left button belongs to
            // whatever is in your other hand.
            if response.dragged_by(egui::PointerButton::Secondary) {
                let moved = response.drag_delta();
                lay_at = Some(
                    middle_of_paper
                        + glam::DVec2::new((-moved.x / scale) as f64, (moved.y / scale) as f64),
                );
            }
            asked = in_hand(
                &table,
                &chart,
                &nav,
                &response,
                on_furniture,
                &sim,
                sky.hour,
            );

            // Where the lines cross, if they do — worked out before they are drawn, so
            // that each of them can be drawn the length that reaches the crossing.
            let crossing_at = sail_core::fix::plot(&nav.sights).map(|f| f.at);

            the_cast(&table, chart.streaming, st.time, st.pos, sky.hour);

            // The pilot's water first, under everything else: it is advice, and advice
            // does not go on top of the plot.
            if job.running && job.over.is_none() {
                the_way_in(&table, &job.route, job.let_go_at);
                // **And the water she can actually be taken into**, which is the one
                // thing a pilot has to know about the ship he is leading and the one
                // thing the chart did not say. Her wind, at her position, at her rig's
                // height — see `where_she_can_go`.
                if brig.afloat {
                    let wind = crate::ship::her_wind(&sim, brig.at);
                    where_she_can_go(
                        &table,
                        brig.at,
                        brig.heading,
                        sail_core::wind::direction_of(wind),
                        &brig.astern,
                    );
                }
            }
            // And the race course, which is the same kind of thing: not where she has
            // been, but where she is supposed to go.
            the_course(&table, &race);

            the_track_made_good(&table, &nav);
            reckoning(&table, &nav, st.time);

            the_sights(
                &table,
                &nav,
                &coast,
                crossing_at,
                chart.range as f64,
                st.heading,
            );
            plotted(&table, &chart, crossing_at, st.heading);

            protractor(
                &table,
                chart.protractor,
                ctx.pointer_latest_pos(),
                chart.range as f64,
            );

            // --- The stopwatch -------------------------------------------------
            //
            // Drawn where a hand would put one down: top right of the paper, clear of
            // the instruments along the top and of the rose in the other corner.
            //
            // It sweeps. A digital readout alone would be a number on a chart, and the
            // reason a stopwatch has a hand is that **counting is easier than reading**
            // — timing a light you are watching the sea and the watch at the same time,
            // and a hand at the top of the dial is a thing you catch out of the corner
            // of your eye while the number is a thing you have to look at.
            press_the_watch = the_watch_face(&table, &chart, st.time, pressed);

            what_she_knows(&table, &nav, st.time);
            the_scale(&table);
            under_a_red_torch(&table, &sky);

            let (picked, explained) = the_instruments(
                &table,
                &chart,
                &furniture,
                why_button,
                chart.streaming,
                st.time,
                pressed,
            );
            if picked.is_some() {
                pick = picked;
            }
            if explained.is_some() {
                explain = explained;
            }

            match the_cocked_hat(&table, &nav, (chart_button, log_button), pressed) {
                Some(Pressed::OnTheChart(at)) => accept = Some(at),
                Some(Pressed::InTheLog) => writing_up = true,
                None => {}
            }

            compass_rose(&table);
        }

        p.rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.0, ink::COAST),
            egui::StrokeKind::Inside,
        );
        weather_on_the_chart(&table, &sim, land, chart.range as f64, st.pos, st.time);

        // The set and drift. Uniform over this much water — it is the wind dragging
        // the top of the sea along, not a tidal stream round a headland — so it is
        // drawn once, in the corner, the way a chart states a current that has no
        // particular place.
        let flow = sim.sim.wind.sample(st.pos, st.time);
        let rate = sail_core::units::ms_to_kn(flow.length() * crate::world::DRIFT_OF_WIND);
        if rate > 0.05 {
            let set = sail_core::wind::direction_of(-flow);
            let from = egui::pos2(rect.left() + 20.0, rect.bottom() - 46.0);
            let bearing = sail_core::units::deg(set).to_radians() as f32;
            let along = egui::vec2(bearing.sin(), -bearing.cos());
            stream_arrow(&p, from, from + along * 26.0, ink::PEN);
            p.text(
                from + along * 26.0 + egui::vec2(0.0, 11.0),
                egui::Align2::CENTER_CENTER,
                format!("{:.1} kn", rate),
                egui::FontId::monospace(9.0),
                ink::PEN,
            );
        }

        // Scale and sounding, in the corner where a chart puts them.
        p.text(
            rect.left_bottom() + egui::vec2(6.0, -6.0),
            egui::Align2::LEFT_BOTTOM,
            format!(
                "{:.1} nm across   depth {:.1} m   open water {OPEN_WATER:.0} m",
                chart.range as f64 * 2.0 / 1852.0,
                sim.sim.telemetry.depth
            ),
            egui::FontId::monospace(10.0),
            ink::COAST,
        );
        // **Last, so that nothing is painted over the torch.** The first attempt put it
        // after the coastline and before the instruments, and everything the plotting
        // layer lays down afterwards went on top: a chart that was red where nothing had
        // been drawn and daylight everywhere that mattered.
        under_a_red_torch(&table, &sky);
    });
    // **The book lies on the table beside the chart**, in its own area with a hand's
    // breadth of gap: it is a separate object, it is read while the chart is being
    // worked on, and a page drawn over the water was a page whose every press had to be
    // kept from reaching the sea underneath.
    if spread {
        egui::Area::new("deck log".into())
            .anchor(egui::Align2::RIGHT_CENTER, egui::vec2(-16.0, 0.0))
            .show(ctx, |ui| {
                match the_book(ui, &log, &radio, &nav, (chart.page, chart.showing)) {
                    Some(Turned::Line(line)) => laid = Some(line),
                    Some(Turned::To(which)) => turn_to = Some(which),
                    None => {}
                }
                // The book is paper too, and it is lying on the same table under the
                // same lamp. A log that stayed cream while the chart went dark would be
                // the brightest thing in the cockpit.
                let dark = after_dark(&sky);
                if dark > 0.02 {
                    torch(ui.painter(), ui.min_rect().expand(6.0), dark);
                }
            });
    }
    if let Some(set) = asked.dividers {
        chart.dividers = Some(set);
    }
    if asked.clear_dividers {
        chart.dividers = None;
    }
    if let Some(item) = asked.draw_on {
        chart.pencil(item);
    }
    if let Some(at) = asked.rub_out {
        // Whatever is nearest, within a reasonable reach — which is what a rubber does:
        // you put it on the thing you meant.
        let reach = chart.range as f64 * 0.04;
        let mut nearest: Option<(usize, f64)> = None;
        for (i, item) in chart.plot.iter().enumerate() {
            let d = match *item {
                Plotted::Cross { at: c, .. } => (c - at).length(),
                Plotted::Stamp { at: c, .. } => (c - at).length(),
                Plotted::Entry(e) => (e.at - at).length(),
                Plotted::Line {
                    through, bearing, ..
                } => {
                    let along = glam::DVec2::new(bearing.sin(), bearing.cos());
                    let off = at - through;
                    (off - along * off.dot(along)).length()
                }
                // A ruled line has ends, so the rubber has to respect them: nearest
                // point *on the segment*, not on the infinite line it lies along.
                Plotted::Rule { from, to } => on_the_line(from, to, at),
                Plotted::Run { from, to, .. } => on_the_line(from, to, at),
            };
            if d < reach && nearest.is_none_or(|(_, best)| d < best) {
                nearest = Some((i, d));
            }
        }
        if let Some((i, _)) = nearest {
            chart.plot.remove(i);
        }
    }
    if let Some(held) = asked.take_hold {
        chart.moving = Some(held);
        // Nothing is being measured while something is being moved: the dividers and the
        // hand are the same hand.
        chart.dividers = None;
    }
    if let Some((i, grab, to)) = asked.shift {
        if let Some(Plotted::Rule { from, to: end }) = chart.plot.get_mut(i) {
            match grab {
                Grab::From => *from = to,
                Grab::To => *end = to,
                Grab::Whole(offset) => {
                    let span = *end - *from;
                    *from = to - offset;
                    *end = *from + span;
                }
            }
        }
    }
    if asked.let_go {
        chart.moving = None;
    }
    // **Start, stop, zero**, in that order and round again, which is the whole of a
    // stopwatch. Three states on one control because the face *is* the control: a hand
    // reaches for the watch, not for a row of buttons beside it.
    if press_the_watch {
        work_the_watch(&mut chart, &mut nav, &mut log, &sim, sky.hour, knows);
    }
    if let Some(measured) = asked.timed {
        nav.measured = Some(measured);
    }
    if let Some(slip) = asked.slip {
        nav.log_slip = slip;
    }
    if let Some(set) = asked.out_by {
        // The same smoothing a fix uses, because it is the same measurement arrived at
        // from a different direction. See `navigation::KEEPS`.
        nav.learn_the_set(set);
    }
    match asked.stream {
        Some(true) => {
            chart.streaming = Some((sim.sim.state.pos, sim.sim.state.time, nav.through_water))
        }
        Some(false) => chart.streaming = None,
        None => {}
    }
    if let Some(where_) = asked.protractor {
        chart.protractor = where_;
    }
    if let Some(up) = explain {
        chart.explain = up;
    }
    if let Some(tool) = pick {
        chart.tool = tool;
        chart.dividers = None;
        // A protractor belongs to the rule that put it down.
        chart.protractor = None;
    }
    if let Some(at) = lay_at {
        chart.laid_at = Some(at);
    }
    if let Some(_at) = accept {
        // Handled by `navigation::enter_in_the_log`, which owns the question of whether
        // the fix was good enough to believe — the chart draws, it does not judge.
        chart.accepted = accept;
    }
    if writing_up {
        // Likewise: the line is made where the glass, the sounder and the sails live.
        chart.writing_up = true;
    }
    if let Some(which) = turn_to {
        chart.page = which;
    }
    // A line of the book laid on the chart, or taken off it again — the pencil and the
    // rubber, which is the whole of what this page does to the paper.
    if let Some(line) = laid {
        let mut over = chart.laid_at;
        lay_it_on_the_chart(&mut log, &mut nav, &mut over, line);
        chart.laid_at = over;
        // An entry pressed is an entry he wants to read, so the rubrics follow the
        // press rather than making him find the tab afterwards.
        if matches!(
            log.lines.get(line).map(|l| &l.wrote),
            Some(Wrote::Entered(_))
        ) {
            chart.showing = Some(line);
            chart.page = Page::Entries;
        }
    }
    Ok(())
}

/// How far a point is from a segment, in whatever units the points are in.
///
/// One function because three places ask it — the rubber, taking hold of a ruled line,
/// and the test below — and because "near a line" computed two ways is a line you can
/// rub out but not pick up.
pub fn on_the_line(from: glam::DVec2, to: glam::DVec2, at: glam::DVec2) -> f64 {
    let span = to - from;
    let t = if span.length_squared() > 1e-9 {
        ((at - from).dot(span) / span.length_squared()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (at - (from + span * t)).length()
}

/// The paper, as everything that draws on it needs to see it.
///
/// **Five things that always travel together**, which is the shape this file was
/// missing: the painter, the rectangle it works in, the scale, where the middle of the
/// paper is over the water, and whether the chart may say where she is. Without it every
/// section of the drawing had to stay inside one closure to reach the locals — which is
/// how `draw` reached sixteen hundred lines, and how a block of drawing came to be
/// pasted inside a drag handler where it ran on one frame in a hundred.
///
/// With it, each thing that puts ink on the chart is a function with a name next to the
/// comment that explains it, and the compiler says so when one of them is handed
/// something it has no business with.
pub struct Table {
    pub p: egui::Painter,
    pub rect: egui::Rect,
    /// Pixels per metre.
    pub scale: f32,
    /// Where the middle of the paper is, in the world.
    pub middle: glam::DVec2,
    /// Whether the chart may say where she is — carried here because half the drawing
    /// has to ask it and the answer must be the same for all of them.
    pub knows: bool,
    /// Whether every line says what it is and why it is there.
    ///
    /// **A teaching switch, and a debugging one.** A plot is six kinds of line in two
    /// colours and no legend, and the difference between a position line, a transferred
    /// position line and a reckoning is the whole of the subject — but a chart carrying
    /// a sentence beside every line is a chart nobody can read. So it is a button.
    pub why: bool,
}

impl Table {
    /// A word beside a line saying what it is, when the legend is up.
    ///
    /// Deliberately plain and deliberately short: the question it answers is *which of
    /// these six lines is which*, and an explanation long enough to be interesting would
    /// cover the chart it is explaining.
    pub fn note(&self, at: egui::Pos2, what: impl ToString, ink: egui::Color32) {
        if !self.why {
            return;
        }
        self.p.text(
            at,
            egui::Align2::LEFT_TOP,
            what.to_string(),
            egui::FontId::monospace(8.0),
            ink.gamma_multiply(0.8),
        );
    }

    pub fn to_screen(&self, at: glam::DVec2) -> egui::Pos2 {
        let off = at - self.middle;
        self.rect.center() + egui::vec2(off.x as f32, -off.y as f32) * self.scale
    }

    pub fn to_water(&self, at: egui::Pos2) -> glam::DVec2 {
        let off = at - self.rect.center();
        self.middle + glam::DVec2::new((off.x / self.scale) as f64, (-off.y / self.scale) as f64)
    }
}

#[cfg(test)]
mod the_corner_is_one_column {
    use super::*;

    /// **The chart and the case under it are the same width**, which is the whole point:
    /// two panels in a corner at different widths read as two things that happen to be
    /// near each other.
    #[test]
    fn the_chart_matches_the_case_it_sits_on() {
        let case = egui::vec2(404.0, 480.0);
        let chart = folded_to(Some(case), 1000.0);
        assert_eq!(chart.x, case.x, "the chart came out {} wide", chart.x);
    }

    /// And it stops before it reaches the case — a square that does not fit the gap
    /// overlaps the thing it was supposed to line up with.
    ///
    /// Over the screens this game is actually played on. The console shrinks itself until
    /// the strip fits, so the screen is about nine hundred and fifty points tall whether
    /// it is a laptop, a large monitor or a Steam Deck; the case is between a third and
    /// two thirds of that depending on what the instruments are showing.
    #[test]
    fn it_never_reaches_down_into_the_instruments() {
        for screen in [900.0_f32, 950.0, 1100.0] {
            for case_height in [360.0_f32, 480.0, 640.0] {
                let case = egui::vec2(404.0, case_height);
                let chart = folded_to(Some(case), screen);
                assert!(
                    chart.y + case.y <= screen,
                    "a {screen:.0}-point screen with a {case_height:.0}-point case: \
                     {:.0} of chart is {:.0} too much",
                    chart.y,
                    chart.y + case.y - screen
                );
            }
        }
    }

    /// **The floor wins over the gap**, which is the one case where the two rules above
    /// disagree: on a screen too short to hold both, a chart squeezed to forty points is
    /// no use to anybody, so it keeps its floor and overlaps instead. The window cannot be
    /// dragged that small — see `screen::SHORTEST` — so this is the behaviour at a size
    /// only a stranger display can produce, and overlapping is the better failure.
    #[test]
    fn it_is_never_squeezed_into_a_smear() {
        let chart = folded_to(Some(egui::vec2(404.0, 700.0)), 740.0);
        assert!(chart.y >= 180.0, "the chart came out {:.0} tall", chart.y);
    }

    /// And before the case has ever been drawn there is still a chart.
    #[test]
    fn the_first_frame_has_a_size_too() {
        let chart = folded_to(None, 880.0);
        assert!(chart.x > 200.0 && chart.y > 200.0, "{chart:?}");
    }
}
