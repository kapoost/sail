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
use sail_core::land::{Mark, OPEN_WATER};

use crate::course::Course;
use crate::sim::Sim;

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
    /// A leg being timed: where she was when the log was streamed, and when.
    pub streaming: Option<(glam::DVec2, f64)>,
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
    /// are looking at. Here it plots a position line through a mark.
    ParallelRule,
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
pub const TOOLS: [Tool; 7] = [
    Tool::Dividers,
    Tool::Ruler,
    Tool::ParallelRule,
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
    Line { through: glam::DVec2, bearing: f64 },
    /// A cross: an estimated position, a danger, a place to come back to.
    Cross { at: glam::DVec2 },
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
}

impl Default for Chart {
    fn default() -> Self {
        Self {
            open: true,
            spread: false,
            dividers: None,
            tool: Tool::Dividers,
            plot: Vec::new(),
            laid_at: None,
            moving: None,
            streaming: None,
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
        chart.open = !chart.open;
        if std::env::var("SAIL_KEYS").is_ok() {
            info!("chart: M -> {}", chart.open);
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
pub fn record(sim: Res<Sim>, time: Res<Time>, mut track: ResMut<Track>) {
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

#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut contexts: EguiContexts,
    mut chart: ResMut<Chart>,
    sim: Res<Sim>,
    course: Res<Course>,
    track: Res<Track>,
    nav: Res<crate::navigation::Navigation>,
    coast: Res<crate::marks::Coast>,
    sky: Res<crate::sky::Sky>,
    fleet: Res<crate::fleet::Fleet>,
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
    let mut set_dividers: Option<(glam::DVec2, glam::DVec2)> = None;
    let mut clear_dividers = false;
    let mut draw_on: Option<Plotted> = None;
    let mut rub_out: Option<glam::DVec2> = None;
    let mut pick: Option<Tool> = None;
    let mut accept: Option<glam::DVec2> = None;
    let mut lay_at: Option<glam::DVec2> = None;
    let mut take_hold: Option<(usize, Grab)> = None;
    let mut shift: Option<(usize, Grab, glam::DVec2)> = None;
    let mut let_go = false;
    let mut stream: Option<bool> = None;
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
            egui::vec2(330.0, 330.0)
        };
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 4.0, ink::DEEP);

        let centre = rect.center();
        let scale = rect.width() * 0.5 / chart.range;
        // Sim north is +y; on paper north is up, so the screen's y runs the other
        // way. Getting this backwards puts every island on the wrong side of you,
        // which is the one mistake a chart must never make.
        let to_water = |s: egui::Pos2| -> glam::DVec2 {
            middle_of_paper
                + glam::DVec2::new(
                    ((s.x - centre.x) / scale) as f64,
                    ((centre.y - s.y) / scale) as f64,
                )
        };
        let to_screen = |w: glam::DVec2| -> egui::Pos2 {
            let d = w - middle_of_paper;
            egui::pos2(centre.x + d.x as f32 * scale, centre.y - d.y as f32 * scale)
        };

        // A kilometre grid, so distance on the chart means something.
        let step = if chart.range > 6_000.0 {
            5_000.0
        } else {
            1_000.0
        };
        let first = ((st.pos.x - chart.range as f64) / step).ceil() * step;
        let mut g = first;
        while g < st.pos.x + chart.range as f64 {
            let x = to_screen(glam::DVec2::new(g, st.pos.y)).x;
            p.line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                egui::Stroke::new(0.5, ink::GRID),
            );
            g += step;
        }
        let first = ((st.pos.y - chart.range as f64) / step).ceil() * step;
        let mut g = first;
        while g < st.pos.y + chart.range as f64 {
            let y = to_screen(glam::DVec2::new(st.pos.x, g)).y;
            p.line_segment(
                [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                egui::Stroke::new(0.5, ink::GRID),
            );
            g += step;
        }

        // The land, and the bank round it. The shoal is drawn first and wider, so
        // it reads as what it is: the thing that gets you before the rocks do.
        for island in land.islands() {
            let outline = |grow: f64| -> Vec<egui::Pos2> {
                (0..64)
                    .map(|i| {
                        let a = i as f64 / 64.0 * std::f64::consts::TAU;
                        let r = island.shore_radius(a) + grow;
                        to_screen(island.centre + glam::DVec2::new(a.sin(), a.cos()) * r)
                    })
                    .collect()
            };
            // The two-metre contour, which is where this boat stops.
            p.add(egui::Shape::convex_polygon(
                outline(110.0),
                ink::SHOAL,
                egui::Stroke::NONE,
            ));
            p.add(egui::Shape::convex_polygon(
                outline(0.0),
                ink::LAND,
                egui::Stroke::new(1.2, ink::COAST),
            ));
        }

        // Where she has been.
        if track.points.len() > 1 {
            let pts: Vec<egui::Pos2> = track.points.iter().map(|q| to_screen(*q)).collect();
            p.add(egui::Shape::line(pts, egui::Stroke::new(1.2, ink::TRACK)));
        }

        // --- The marks ---------------------------------------------------
        //
        // The things you navigate by, drawn the way a chart draws them: a symbol you
        // can tell apart at a glance, a name, and for a light its character — which
        // is the only thing that identifies it in the dark and is therefore the one
        // piece of text on a chart that is worth more than the name beside it.
        for mark in &coast.marks {
            let at = to_screen(mark.at);
            if !rect.contains(at) {
                continue;
            }
            // **Nothing is on the chart until she has recognised it.**
            //
            // Which is a decision about what this chart *is*. A printed chart carries
            // every mark on the coast whether you know which is which or not — but the
            // exercise is not about reading a printed chart, it is about building one:
            // you look at a group through the glasses, you work out what it is, and
            // *then* it goes on the paper with its name. Asked for from the deck in
            // exactly those terms.
            //
            // The coast itself stays. Land does not need identifying — it is there, you
            // can see it, and the shape of it is what you recognise the marks by.
            if nav.lost && !nav.recognised.contains(&mark.name) {
                continue;
            }
            match mark.kind {
                Mark::Lighthouse => {
                    // The magenta flare: a dot with a teardrop out of it, which is
                    // how every chart in the world says "light" and has since they
                    // were engraved.
                    p.circle_filled(at, 2.6, ink::LIGHT);
                    for i in 0..7 {
                        let a = -0.9 + i as f32 * 0.3;
                        p.line_segment(
                            [at, at + egui::vec2(a.sin(), -a.cos()) * 11.0],
                            egui::Stroke::new(1.0, ink::LIGHT),
                        );
                    }
                }
                Mark::Spire => {
                    // A circle with a dot in it and a cross on top: the chart symbol
                    // for a church, and the dot is the bit that is the position.
                    p.circle(
                        at,
                        3.4,
                        egui::Color32::TRANSPARENT,
                        egui::Stroke::new(1.1, ink::COAST),
                    );
                    p.circle_filled(at, 1.0, ink::COAST);
                    p.line_segment(
                        [at + egui::vec2(0.0, -4.0), at + egui::vec2(0.0, -8.5)],
                        egui::Stroke::new(1.1, ink::COAST),
                    );
                    p.line_segment(
                        [at + egui::vec2(-2.0, -6.5), at + egui::vec2(2.0, -6.5)],
                        egui::Stroke::new(1.1, ink::COAST),
                    );
                }
                Mark::Beacon => {
                    p.line_segment(
                        [at, at + egui::vec2(0.0, -9.0)],
                        egui::Stroke::new(1.2, ink::COAST),
                    );
                    p.circle_filled(at + egui::vec2(0.0, -10.0), 2.0, ink::COAST);
                    p.circle_filled(at, 1.4, ink::COAST);
                }
                Mark::Cairn => {
                    p.circle_filled(at, 1.8, ink::COAST);
                    p.circle(
                        at,
                        4.2,
                        egui::Color32::TRANSPARENT,
                        egui::Stroke::new(0.7, ink::COAST),
                    );
                }
            }
            // Named when the chart is big enough to have room for the name, which on
            // the folded one means the lights only.
            // Labelled where there is room for the label. On the folded chart only the
            // lights are named, and none of them within a label's width of the edge —
            // names running off the corner of a chart are worse than no names, because
            // the eye reads half of one and believes it.
            let room = rect.contains(at + egui::vec2(120.0, 0.0))
                && at.y > rect.top() + 14.0
                && at.y < rect.bottom() - 14.0;
            // And named only if she knows which one it is. While she is lost the chart
            // carries the symbols — the coast does not stop being surveyed because you
            // are — but the names belong to marks she has recognised through the
            // glasses. A chart that names everything is a chart that has told you where
            // you are.
            // Anything that is drawn at all has been recognised, so it carries its
            // name: there is no such thing here as a mark on the chart that she cannot
            // name.
            if room && (spread || mark.kind == Mark::Lighthouse || nav.lost) {
                let label = if mark.kind.lit() {
                    format!(
                        "{}  {}",
                        mark.name,
                        crate::marks::light_character(&mark.name)
                    )
                } else {
                    format!("{}  {}", mark.name, mark.kind.abbreviation())
                };
                p.text(
                    at + egui::vec2(9.0, 1.0),
                    egui::Align2::LEFT_CENTER,
                    label,
                    egui::FontId::monospace(if spread { 11.0 } else { 9.0 }),
                    if mark.kind.lit() {
                        ink::LIGHT
                    } else {
                        ink::COAST
                    },
                );
            }
        }

        // Anchorages, with the one in hand picked out — and not at all while she is
        // lost. A harbour is a navigation object like any other: four of them named on
        // the chart is a position given away to anybody who can count islands.
        for (i, h) in course.harbours.iter().enumerate().filter(|_| !nav.lost) {
            let at = to_screen(h.at);
            let bound = i == course.to;
            p.circle(
                at,
                if bound { 5.0 } else { 3.0 },
                if bound { ink::MARK } else { ink::PAPER },
                egui::Stroke::new(1.0, ink::COAST),
            );
            if bound || chart.range < 6_000.0 {
                p.text(
                    at + egui::vec2(7.0, -6.0),
                    egui::Align2::LEFT_TOP,
                    &h.name,
                    egui::FontId::monospace(10.0),
                    ink::COAST,
                );
            }
        }

        // What a chart with no fix on it says, which is nothing — and it says it where
        // the boat would have been, because that is where the eye goes first.
        // How long the fix has left, when she is living on one. A position is a
        // statement about a moment and this is the moment running out — drawn rather
        // than counted, because a bar is read without being looked at.
        if nav.lost && knows && spread {
            // How many of the marks this fix was taken on can still be made out, which
            // is what is actually keeping the position: while two of them are there,
            // another fix is a minute's work and she is not lost in any sense that
            // matters. The clock behind it is only a backstop.
            p.text(
                egui::pos2(rect.left() + 24.0, rect.top() + 84.0),
                egui::Align2::LEFT_CENTER,
                format!(
                    "{} of {} marks still in sight",
                    nav.still_in_sight.min(nav.fixed_on.len()),
                    nav.fixed_on.len()
                ),
                egui::FontId::monospace(11.0),
                if nav.still_in_sight > 2 {
                    ink::COAST
                } else {
                    ink::BOAT
                },
            );
            let left = nav.fix_left(sim.sim.state.time);
            let bar = egui::Rect::from_min_size(
                egui::pos2(rect.left() + 24.0, rect.top() + 66.0),
                egui::vec2(188.0, 7.0),
            );
            p.rect_filled(bar, 1.0, ink::PAPER);
            let mut run = bar;
            run.set_width(bar.width() * left);
            p.rect_filled(run, 1.0, ink::MARK);
            p.text(
                egui::pos2(bar.right() + 8.0, bar.center().y),
                egui::Align2::LEFT_CENTER,
                "this fix",
                egui::FontId::monospace(10.0),
                ink::COAST,
            );
        }
        if !knows {
            p.text(
                centre,
                egui::Align2::CENTER_CENTER,
                "POSITION UNKNOWN",
                egui::FontId::monospace(if spread { 16.0 } else { 11.0 }),
                ink::BOAT,
            );
            if spread {
                p.text(
                    centre + egui::vec2(0.0, 20.0),
                    egui::Align2::CENTER_CENTER,
                    "take bearings on two marks and cross them",
                    egui::FontId::monospace(11.0),
                    ink::COAST,
                );
            }
        }
        if knows {
            // The boat: a wedge pointing where she is pointing, drawn where she *is* on
            // the paper rather than in the middle of it. Those are the same point only
            // while the chart is following her, which it stops doing the moment she does
            // not know where she is.
            let her = to_screen(st.pos);
            let heading = st.heading as f32;
            let nose = egui::vec2(heading.sin(), -heading.cos());
            let beam = egui::vec2(-nose.y, nose.x);
            p.add(egui::Shape::convex_polygon(
                vec![
                    her + nose * 9.0,
                    her - nose * 5.0 + beam * 4.0,
                    her - nose * 5.0 - beam * 4.0,
                ],
                ink::BOAT,
                egui::Stroke::NONE,
            ));
            // And the bearing to the consignment, which is the line you are trying to
            // make good rather than the one you are sailing.
            let target = to_screen(course.target());
            p.line_segment([her, target], egui::Stroke::new(1.0, ink::MARK));

            // **And everybody else.**
            //
            // Until now the only place another boat appeared was a list of names in the
            // settings panel with a distance beside each — which answers "how far" and
            // not the question anybody actually has, which is *where*. A fleet on a
            // chart is the oldest answer to it and it costs one wedge each.
            //
            // Hollow rather than filled, so your own boat is still the one the eye finds
            // first on a page with five of them on it, and named on the table where
            // there is room for the lettering. Not on the folded chart: at two inches
            // square a name is a smudge across an island.
            //
            // Only when she knows where she is, like everything else here — a chart that
            // will not say where *you* are cannot say where your friends are either, or
            // the exercise is over the moment somebody else joins.
            for boat in &fleet.boats {
                let at = glam::DVec2::new(boat.drawn.pos[0] as f64, boat.drawn.pos[1] as f64);
                let them = to_screen(at);
                if !rect.contains(them) {
                    continue;
                }
                let heading = boat.drawn.heading;
                let nose = egui::vec2(heading.sin(), -heading.cos());
                let beam = egui::vec2(-nose.y, nose.x);
                p.add(egui::Shape::convex_polygon(
                    vec![
                        them + nose * 8.0,
                        them - nose * 4.5 + beam * 3.5,
                        them - nose * 4.5 - beam * 3.5,
                    ],
                    egui::Color32::TRANSPARENT,
                    egui::Stroke::new(1.3, ink::BOAT),
                ));
                if spread && !boat.name.is_empty() {
                    p.text(
                        them + egui::vec2(10.0, -2.0),
                        egui::Align2::LEFT_CENTER,
                        &boat.name,
                        egui::FontId::monospace(10.0),
                        ink::BOAT,
                    );
                }
            }
        }

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
            let log_button = egui::Rect::from_min_size(
                egui::pos2(rect.left() + 24.0, rect.top() + 66.0),
                egui::vec2(188.0, 22.0),
            );
            let on_furniture = response.interact_pointer_pos().is_some_and(|at| {
                furniture.iter().any(|slot| slot.contains(at)) || log_button.contains(at)
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

            // What a drag means depends on what is in your hand, which is how a chart
            // table works: the dividers measure, the rule plots, the pencil marks and
            // the rubber takes it off again.
            match chart.tool {
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
                            let water = to_water(at);
                            // A finger's width on the paper, so a line can be caught
                            // without the pointer having to be on the pencil mark.
                            let near = (12.0 / scale) as f64;
                            take_hold = chart.plot.iter().enumerate().find_map(|(i, item)| {
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
                    if let (Some((i, grab)), Some(at)) =
                        (chart.moving, response.interact_pointer_pos())
                    {
                        if response.dragged() {
                            shift = Some((i, grab, to_water(at)));
                        }
                        if response.drag_stopped() {
                            let_go = true;
                        }
                    } else if response.drag_started() && !on_furniture {
                        if let Some(at) = response.interact_pointer_pos() {
                            set_dividers = Some((to_water(at), to_water(at)));
                        }
                    } else if response.dragged() {
                        if let (Some(at), Some((from, _))) =
                            (response.interact_pointer_pos(), chart.dividers)
                        {
                            set_dividers = Some((from, to_water(at)));
                        }
                    } else if response.drag_stopped() {
                        // The rule leaves its line on the chart; the dividers leave
                        // nothing, because dividers are for reading and not for drawing.
                        // The leg being timed, while it is being timed: a line from where the log
                        // was streamed to where she is now, with the figures running. Without it the
                        // tool is a button that appears to do nothing for ten minutes.
                        if let Some((was, when)) = chart.streaming {
                            let (a, b) = (to_screen(was), to_screen(st.pos));
                            p.line_segment([a, b], egui::Stroke::new(1.0, ink::MARK));
                            let off = st.pos - was;
                            let minutes = (st.time - when) / 60.0;
                            let miles = off.length() / 1_852.0;
                            p.text(
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
                        if let Some((from, to)) = chart.dividers {
                            let off = to - from;
                            // A twitch of the hand is not a line. Below a tenth of an
                            // inch of paper it was meant as a click.
                            if off.length() > chart.range as f64 * 0.01 {
                                if chart.tool == Tool::ParallelRule {
                                    draw_on = Some(Plotted::Line {
                                        through: from,
                                        bearing: off
                                            .x
                                            .atan2(off.y)
                                            .rem_euclid(std::f64::consts::TAU),
                                    });
                                } else if chart.tool == Tool::Ruler {
                                    draw_on = Some(Plotted::Rule { from, to });
                                }
                            }
                        }
                        clear_dividers = true;
                    } else if response.clicked() {
                        clear_dividers = true;
                    }
                }
                Tool::Pencil => {
                    if response.clicked() && !on_furniture {
                        if let Some(at) = response.interact_pointer_pos() {
                            draw_on = Some(Plotted::Cross { at: to_water(at) });
                        }
                    }
                }
                Tool::Rubber => {
                    if response.clicked() && !on_furniture {
                        if let Some(at) = response.interact_pointer_pos() {
                            rub_out = Some(to_water(at));
                        }
                    }
                }
                Tool::Watch => {
                    if response.clicked() && !on_furniture {
                        if let Some(at) = response.interact_pointer_pos() {
                            draw_on = Some(Plotted::Stamp {
                                at: to_water(at),
                                hour: sky.hour,
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
                            None => stream = Some(true),
                            Some((was, when)) => {
                                stream = Some(false);
                                let minutes = (st.time - when) / 60.0;
                                // A leg of a few seconds is a slipped finger rather than
                                // a measurement: at half a minute the distance run is
                                // inside the width of the pencil.
                                if minutes > 0.5 {
                                    draw_on = Some(Plotted::Run {
                                        from: was,
                                        to: st.pos,
                                        minutes,
                                    });
                                }
                            }
                        }
                    }
                }
            }

            // Where the lines cross, if they do — worked out before they are drawn, so
            // that each of them can be drawn the length that reaches the crossing.
            let crossing_at = sail_core::fix::plot(&nav.sights).map(|f| f.at);

            // --- What is already on the chart --------------------------------
            //
            // On the table only. The folded chart is a glance — *where am I, what is
            // ahead* — and a plot on it is a thicket of pencil over a picture two inches
            // square. Working belongs where there is room to work.
            for (i, item) in chart.plot.iter().enumerate() {
                match *item {
                    Plotted::Line { through, bearing } => {
                        // Drawn the length a navigator draws one, which is not across the
                        // whole chart: a position line is a *pencil line near where you
                        // think you are*, long enough to cross the others and no longer.
                        // Lines run off both edges make a cobweb and hide the coast under
                        // it — reported from the deck in those words.
                        //
                        // So it runs from the mark towards the observer, as far as the
                        // crossing it makes plus a little, and when there is no crossing
                        // yet, far enough to be worth looking at.
                        let away =
                            (bearing + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU);
                        let along = glam::DVec2::new(away.sin(), away.cos());
                        let reach = crossing_at
                            .map(|c| ((c - through).length() * 1.25).max(chart.range as f64 * 0.25))
                            .unwrap_or(chart.range as f64 * 0.9);
                        let a = to_screen(through);
                        let b = to_screen(through + along * reach);
                        p.line_segment([a, b], egui::Stroke::new(1.1, ink::PENCIL));
                        p.text(
                            to_screen(through) + egui::vec2(6.0, 10.0),
                            egui::Align2::LEFT_TOP,
                            format!("{:03.0}", bearing.to_degrees()),
                            egui::FontId::monospace(10.0),
                            ink::PENCIL,
                        );
                    }
                    Plotted::Cross { at } => {
                        let c = to_screen(at);
                        for (dx, dy) in [(-5.0, -5.0), (-5.0, 5.0)] {
                            p.line_segment(
                                [c + egui::vec2(dx, dy), c - egui::vec2(dx, dy)],
                                egui::Stroke::new(1.3, ink::PENCIL),
                            );
                        }
                    }
                    // A leg that was sailed, drawn the way a course made good is drawn:
                    // one arrow along it, the course over the line and what she did
                    // under it. Two figures rather than three, because the distance is
                    // the one you can measure off the paper with the dividers and the
                    // speed is the one you cannot.
                    Plotted::Run { from, to, minutes } => {
                        let (a, b) = (to_screen(from), to_screen(to));
                        p.line_segment([a, b], egui::Stroke::new(1.3, ink::TRACK));
                        let off = to - from;
                        let miles = off.length() / 1_852.0;
                        let course = off.x.atan2(off.y).rem_euclid(std::f64::consts::TAU);
                        // The arrowhead, at the far end and pointing the way she went.
                        let along = (b - a).normalized();
                        let across = egui::vec2(-along.y, along.x);
                        for side in [-1.0, 1.0] {
                            p.line_segment(
                                [b, b - along * 9.0 + across * 4.0 * side],
                                egui::Stroke::new(1.3, ink::TRACK),
                            );
                        }
                        let middle = egui::pos2((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
                        p.text(
                            middle + egui::vec2(6.0, -4.0),
                            egui::Align2::LEFT_BOTTOM,
                            format!("{:03.0}", course.to_degrees()),
                            egui::FontId::monospace(10.0),
                            ink::TRACK,
                        );
                        p.text(
                            middle + egui::vec2(6.0, 4.0),
                            egui::Align2::LEFT_TOP,
                            format!(
                                "{:.2} M in {:.0} min — {:.1} kn",
                                miles,
                                minutes,
                                miles / (minutes / 60.0).max(1e-6)
                            ),
                            egui::FontId::monospace(10.0),
                            ink::TRACK,
                        );
                    }
                    Plotted::Stamp { at, hour } => {
                        // Four figures and a tick under them, which is how a time is
                        // written beside a mark: no colon, because a deck log has never
                        // used one, and the tick so that a page with six times on it
                        // still says which is which.
                        let c = to_screen(at);
                        p.line_segment(
                            [c + egui::vec2(0.0, -3.0), c + egui::vec2(0.0, 3.0)],
                            egui::Stroke::new(1.2, ink::PENCIL),
                        );
                        p.text(
                            c + egui::vec2(5.0, -4.0),
                            egui::Align2::LEFT_BOTTOM,
                            format!(
                                "{:02.0}{:02.0}",
                                hour.floor(),
                                (hour.fract() * 60.0).floor()
                            ),
                            egui::FontId::monospace(11.0),
                            ink::PENCIL,
                        );
                    }
                    Plotted::Rule { from, to } => {
                        // A ruled leg, with the two things written along it that a
                        // navigator writes along one: the course to steer and how far it
                        // is. Written once, above the line, which is where it goes on
                        // paper — the distance goes underneath on a passage chart, but
                        // one label on a screen this size is already two lines of type.
                        let (a, b) = (to_screen(from), to_screen(to));
                        // Darker while it is in your hand, which is the only feedback a
                        // straight edge needs: you can see that you have hold of this
                        // one rather than of the chart.
                        let in_hand = chart.moving.is_some_and(|(held, _)| held == i);
                        p.line_segment(
                            [a, b],
                            egui::Stroke::new(if in_hand { 2.0 } else { 1.2 }, ink::PENCIL),
                        );
                        let off = to - from;
                        let bearing = off.x.atan2(off.y).rem_euclid(std::f64::consts::TAU);
                        // **The ends say which point they mean.**
                        //
                        // A tick across the line says *the line stops here*, which is
                        // not the same thing and was what was drawn: at a glance you
                        // could not tell whether the leg ended at the tick, at the
                        // middle of it, or somewhere under the pencil. A small upright
                        // cross with a dot at its centre says a point and nothing else,
                        // which is how a position is marked on paper and why a cocked
                        // hat has a dot in the middle of it. Asked for from the deck as
                        // ends that point at something exactly and unambiguously.
                        for end in [a, b] {
                            for (dx, dy) in [(5.0, 0.0), (0.0, 5.0)] {
                                p.line_segment(
                                    [end + egui::vec2(dx, dy), end - egui::vec2(dx, dy)],
                                    egui::Stroke::new(1.0, ink::PENCIL),
                                );
                            }
                            p.circle_filled(end, 1.4, ink::PENCIL);
                        }
                        p.text(
                            egui::pos2((a.x + b.x) * 0.5, (a.y + b.y) * 0.5)
                                + egui::vec2(6.0, -6.0),
                            egui::Align2::LEFT_BOTTOM,
                            format!(
                                "{:03.0}   {:.2} M",
                                bearing.to_degrees(),
                                off.length() / 1_852.0
                            ),
                            egui::FontId::monospace(10.0),
                            ink::PENCIL,
                        );
                    }
                }
            }
            if let Some((from, to)) = chart.dividers {
                let (a, b) = (to_screen(from), to_screen(to));
                let off = to - from;
                let miles = off.length() / 1_852.0;
                let bearing = off.x.atan2(off.y).rem_euclid(std::f64::consts::TAU);
                p.line_segment([a, b], egui::Stroke::new(1.4, ink::TRACK));
                for end in [a, b] {
                    p.circle(end, 3.0, ink::PAPER, egui::Stroke::new(1.2, ink::TRACK));
                }
                let label = format!(
                    "{miles:.2} M   {:03.0} / {:03.0}",
                    bearing.to_degrees(),
                    (bearing.to_degrees() + 180.0) % 360.0
                );
                let middle = egui::pos2((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
                p.text(
                    middle + egui::vec2(8.0, -8.0),
                    egui::Align2::LEFT_BOTTOM,
                    label,
                    egui::FontId::monospace(12.0),
                    ink::TRACK,
                );
            }

            // The scale, which a chart has to carry or its distances are a rumour.
            // A mile, drawn as a mile is drawn: ten cables, alternating.
            let mile = 1_852.0 * scale;
            let bar = egui::pos2(rect.left() + 24.0, rect.bottom() - 26.0);
            for i in 0..10 {
                let from = bar + egui::vec2(mile * i as f32 / 10.0, 0.0);
                let to = bar + egui::vec2(mile * (i + 1) as f32 / 10.0, 0.0);
                p.line_segment(
                    [from, to],
                    egui::Stroke::new(5.0, if i % 2 == 0 { ink::COAST } else { ink::PAPER }),
                );
            }
            p.text(
                bar + egui::vec2(mile + 8.0, 0.0),
                egui::Align2::LEFT_CENTER,
                "1 nautical mile",
                egui::FontId::monospace(11.0),
                ink::COAST,
            );
            p.text(
                egui::pos2(rect.left() + 24.0, rect.top() + 46.0),
                egui::Align2::LEFT_CENTER,
                chart.tool.hint(),
                egui::FontId::monospace(11.0),
                ink::COAST,
            );
            // The instruments, along the top of the table where a hand reaches for them.
            for (tool, slot) in TOOLS.iter().zip(&furniture) {
                let (tool, slot) = (*tool, *slot);
                let text = tool.name();
                let on = chart.tool == tool;
                p.rect_filled(slot, 2.0, if on { ink::MARK } else { ink::PAPER });
                p.rect_stroke(
                    slot,
                    2.0,
                    egui::Stroke::new(1.0, ink::COAST),
                    egui::StrokeKind::Inside,
                );
                p.text(
                    slot.center(),
                    egui::Align2::CENTER_CENTER,
                    text,
                    egui::FontId::monospace(10.0),
                    ink::COAST,
                );
                if response.clicked()
                    && response
                        .interact_pointer_pos()
                        .is_some_and(|at| slot.contains(at))
                {
                    pick = Some(tool);
                }
            }

            // --- The fix -----------------------------------------------------
            //
            // Two position lines cross at a point and three cross in a triangle, and the
            // triangle is the navigator's own report on how carefully he just did it.
            // Both are drawn: the crossing because it is the answer, the triangle because
            // it is the confidence, and a fix with no cocked hat round it is a fix
            // nobody has checked.
            if let Some(got) = sail_core::fix::plot(&nav.sights) {
                let at = to_screen(got.at);
                p.circle(
                    at,
                    6.0,
                    egui::Color32::TRANSPARENT,
                    egui::Stroke::new(1.4, ink::BOAT),
                );
                p.circle_filled(at, 1.6, ink::BOAT);
                p.text(
                    at + egui::vec2(10.0, -4.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!(
                        "FIX  hat {:.0} m, cut {:.0}  —  {}",
                        got.hat,
                        got.cut.to_degrees(),
                        sail_core::fix::verdict(&got, nav.sights.len())
                    ),
                    egui::FontId::monospace(11.0),
                    ink::BOAT,
                );
                // And the button that says you believe it. A fix is not a measurement
                // until somebody writes it in the log, which is the moment it becomes the
                // position the boat is working from — and the moment it can be wrong.
                let slot = log_button;
                p.rect_filled(slot, 2.0, ink::PAPER);
                p.rect_stroke(
                    slot,
                    2.0,
                    egui::Stroke::new(1.0, ink::COAST),
                    egui::StrokeKind::Inside,
                );
                p.text(
                    slot.center(),
                    egui::Align2::CENTER_CENTER,
                    "ENTER IT IN THE LOG",
                    egui::FontId::monospace(11.0),
                    ink::COAST,
                );
                if response.clicked()
                    && response
                        .interact_pointer_pos()
                        .is_some_and(|q| slot.contains(q))
                {
                    accept = Some(got.at);
                }
            }

            // --- The compass rose --------------------------------------------
            //
            // Printed on every chart ever made, and not as decoration: it is what a
            // parallel rule is laid against to read a bearing off, and it is how you can
            // tell at a glance which way is north on a chart somebody has turned round on
            // the table. Graduated every ten degrees and numbered every thirty.
            let rose = egui::pos2(rect.right() - 86.0, rect.bottom() - 86.0);
            p.circle(
                rose,
                56.0,
                egui::Color32::TRANSPARENT,
                egui::Stroke::new(1.0, ink::COAST),
            );
            p.circle(
                rose,
                44.0,
                egui::Color32::TRANSPARENT,
                egui::Stroke::new(0.7, ink::COAST),
            );
            for tick in 0..36 {
                let a = (tick as f32 * 10.0).to_radians();
                let dir = egui::vec2(a.sin(), -a.cos());
                let long = tick % 3 == 0;
                p.line_segment(
                    [
                        rose + dir * if long { 44.0 } else { 50.0 },
                        rose + dir * 56.0,
                    ],
                    egui::Stroke::new(if long { 1.1 } else { 0.6 }, ink::COAST),
                );
                if long {
                    p.text(
                        rose + dir * 34.0,
                        egui::Align2::CENTER_CENTER,
                        format!("{:03}", tick * 10),
                        egui::FontId::monospace(8.0),
                        ink::COAST,
                    );
                }
            }
            p.add(egui::Shape::convex_polygon(
                vec![
                    rose + egui::vec2(0.0, -56.0),
                    rose + egui::vec2(-4.5, -40.0),
                    rose + egui::vec2(4.5, -40.0),
                ],
                ink::BOAT,
                egui::Stroke::NONE,
            ));
        }

        p.rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.0, ink::COAST),
            egui::StrokeKind::Inside,
        );
        // --- The weather, pencilled on ---------------------------------------
        //
        // A navigator corrects the chart in ink and writes the forecast on it in
        // pencil, because the coastline is true next year and the wind is true for
        // the afternoon. These are drawn from the *live* field rather than from the
        // forecast's own numbers, which is a better reading of the same thing: the
        // barbs show the lee of an island and the outflow of a squall, neither of
        // which a forecast can tell you and both of which decide which side you pass.
        // Four of them, one to a quadrant, and the middle left clear for the boat
        // and her track. Twenty-five was a weather map; this is a chart with the
        // weather written on it, and the difference is that the chart is still
        // legible underneath. Four is also honest about what the symbol is for: the
        // wind over this much water is one wind, and the barbs are there to say
        // which way it sets and roughly how hard, not to map a field.
        const BARBS: i32 = 1;
        for ix in -BARBS..=BARBS {
            for iy in -BARBS..=BARBS {
                if ix == 0 || iy == 0 {
                    continue;
                }
                let step = chart.range as f64 * 0.62;
                let at = st.pos + glam::DVec2::new(ix as f64 * step, iy as f64 * step);
                // Nothing over the land: a wind arrow on a hillside is a symbol
                // placed where its quantity does not exist.
                if land.depth(at) <= 0.0 {
                    continue;
                }
                let screen = to_screen(at);
                if !rect.contains(screen) {
                    continue;
                }
                let flow = sim.sim.wind.sample(at, st.time);
                barb(
                    &p,
                    screen,
                    sail_core::wind::direction_of(flow),
                    sail_core::units::ms_to_kn(flow.length()),
                    ink::PEN,
                );
            }
        }

        // Squalls, as a ring round what they cover. The console says how far off the
        // nearest one is; this says which way it is going and what it will miss,
        // which is the question you actually have.
        for (centre, radius, maturity) in sim.sim.wind.squalls() {
            let at = to_screen(centre);
            let r = radius as f32 * scale;
            if r < 2.0 {
                continue;
            }
            let ring = ink::SQUALL.gamma_multiply(0.35 + 0.65 * maturity as f32);
            p.circle_stroke(at, r, egui::Stroke::new(1.0, ring));
            p.circle_stroke(at, r * 0.55, egui::Stroke::new(1.0, ring));
        }

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
    });
    if let Some(set) = set_dividers {
        chart.dividers = Some(set);
    }
    if clear_dividers {
        chart.dividers = None;
    }
    if let Some(item) = draw_on {
        chart.plot.push(item);
        // What a navigator would have on the chart and not a day's worth of pencil: past
        // a dozen lines the plot is a scribble, and the oldest of them is about water she
        // left an hour ago.
        if chart.plot.len() > 12 {
            chart.plot.remove(0);
        }
    }
    if let Some(at) = rub_out {
        // Whatever is nearest, within a reasonable reach — which is what a rubber does:
        // you put it on the thing you meant.
        let reach = chart.range as f64 * 0.04;
        let mut nearest: Option<(usize, f64)> = None;
        for (i, item) in chart.plot.iter().enumerate() {
            let d = match *item {
                Plotted::Cross { at: c } => (c - at).length(),
                Plotted::Stamp { at: c, .. } => (c - at).length(),
                Plotted::Line { through, bearing } => {
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
    if let Some(held) = take_hold {
        chart.moving = Some(held);
        // Nothing is being measured while something is being moved: the dividers and the
        // hand are the same hand.
        chart.dividers = None;
    }
    if let Some((i, grab, to)) = shift {
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
    if let_go {
        chart.moving = None;
    }
    match stream {
        Some(true) => chart.streaming = Some((sim.sim.state.pos, sim.sim.state.time)),
        Some(false) => chart.streaming = None,
        None => {}
    }
    if let Some(tool) = pick {
        chart.tool = tool;
        chart.dividers = None;
    }
    if let Some(at) = lay_at {
        chart.laid_at = Some(at);
    }
    if let Some(_at) = accept {
        // Handled by `navigation::enter_in_the_log`, which owns the question of whether
        // the fix was good enough to believe — the chart draws, it does not judge.
        chart.accepted = accept;
    }
    Ok(())
}

/// A wind barb: the staff along the wind, the feathers at its tail.
///
/// The symbol every weather chart has used for a century, and it is on this one because
/// it says two things at once in a shape the eye reads without counting. The staff lies
/// along the wind and points at the place it is blowing *from* — so a glance gives the
/// direction without a compass rose — and the feathers at its tail are the strength: a
/// full one is ten knots, a half is five, a solid pennant is fifty. Force six comes out
/// as two feathers and a half, and you learn to see "two and a half" as a weight rather
/// than as a number, which is the whole point of a symbol.
///
/// Rounded to the nearest five knots by its own nature, which is also what keeps it
/// still: the wind field under it has gusts in it, and a barb that redrew itself every
/// time a patch went by would be a flickering mess instead of a reading.
fn barb(p: &egui::Painter, at: egui::Pos2, from: f64, knots: f64, colour: egui::Color32) {
    use sail_core::units::deg;
    // North up: a bearing turns clockwise from the top of the chart, and the screen's y
    // runs down, so north is −y and east is +x.
    let bearing = deg(from).to_radians() as f32;
    let along = egui::vec2(bearing.sin(), -bearing.cos());
    let stroke = egui::Stroke::new(1.0, colour);
    // Under three knots there is no wind to draw and the convention says so: a bare
    // circle, calm.
    if knots < 3.0 {
        p.circle_stroke(at, 2.5, stroke);
        return;
    }
    const STAFF: f32 = 21.0;
    const FEATHER: f32 = 7.0;
    let tail = at + along * STAFF;
    p.line_segment([at, tail], stroke);
    // Feathers hang off the upwind end, raked back towards the staff's point.
    let across = egui::vec2(along.y, -along.x);
    let rake = (across * 0.86 - along * 0.5).normalized();
    let mut left = (knots / 5.0).round() * 5.0;
    let mut step = 0.0;
    while left >= 50.0 {
        // A pennant: a filled triangle, and a day to stay in harbour.
        let root = tail - along * step;
        let next = root - along * (FEATHER * 0.55);
        p.add(egui::Shape::convex_polygon(
            vec![root, root + rake * FEATHER, next],
            colour,
            egui::Stroke::NONE,
        ));
        step += FEATHER * 0.8;
        left -= 50.0;
    }
    while left >= 10.0 {
        let root = tail - along * step;
        p.line_segment([root, root + rake * FEATHER], stroke);
        step += FEATHER * 0.5;
        left -= 10.0;
    }
    if left >= 5.0 {
        // A half feather, and it is drawn half a step in from the end when it is the
        // only one, because a half barb at the very tail reads as a full one.
        let root = tail - along * step.max(FEATHER * 0.5);
        p.line_segment([root, root + rake * (FEATHER * 0.5)], stroke);
    }
}

/// An arrow with an open head, which is how a stream is marked.
fn stream_arrow(p: &egui::Painter, from: egui::Pos2, to: egui::Pos2, colour: egui::Color32) {
    let stroke = egui::Stroke::new(1.0, colour);
    p.line_segment([from, to], stroke);
    let along = (to - from).normalized();
    let across = egui::vec2(along.y, -along.x);
    for side in [1.0_f32, -1.0] {
        p.line_segment([to, to - along * 6.0 + across * (4.0 * side)], stroke);
    }
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

#[cfg(test)]
mod symbols {
    /// A barb's feathers have to add up to the wind it is drawn for.
    ///
    /// The drawing is pixels and cannot be asserted, but the *count* can, and the count
    /// is the whole content of the symbol: ten knots a feather, five a half, fifty a
    /// pennant. Getting it wrong by one half-feather is a five-knot lie on a chart, and
    /// five knots is the difference between a reef and no reef.
    fn feathers(knots: f64) -> (i32, i32, i32) {
        let mut left = (knots / 5.0).round() * 5.0;
        let (mut pennants, mut full, mut half) = (0, 0, 0);
        while left >= 50.0 {
            pennants += 1;
            left -= 50.0;
        }
        while left >= 10.0 {
            full += 1;
            left -= 10.0;
        }
        if left >= 5.0 {
            half += 1;
        }
        (pennants, full, half)
    }

    #[test]
    fn the_feathers_add_up_to_the_wind() {
        assert_eq!(feathers(0.0), (0, 0, 0));
        // Four knots is a half feather, because four rounds to five and five is what a
        // half feather means. Calm is a different symbol altogether — a bare circle,
        // drawn below three knots — and conflating the two was this test's own first
        // mistake rather than the code's.
        assert_eq!(feathers(4.0), (0, 0, 1));
        assert_eq!(feathers(5.0), (0, 0, 1));
        assert_eq!(
            feathers(12.0),
            (0, 1, 0),
            "twelve rounds to ten: one feather"
        );
        assert_eq!(feathers(13.0), (0, 1, 1), "fifteen: a feather and a half");
        assert_eq!(feathers(25.0), (0, 2, 1), "force six");
        assert_eq!(
            feathers(34.0),
            (0, 3, 1),
            "a gale, and you can see the weight of it"
        );
        assert_eq!(feathers(50.0), (1, 0, 0), "a pennant, and a day to stay in");
        assert_eq!(feathers(65.0), (1, 1, 1));
        // And every one of them reads back as the wind it was drawn for, to the nearest
        // five knots, which is the only promise the symbol makes.
        for step in 0..30 {
            let knots = step as f64 * 2.7;
            let (p, f, h) = feathers(knots);
            let read = (p * 50 + f * 10 + h * 5) as f64;
            assert!(
                (read - knots).abs() <= 2.5,
                "{knots:.1} kn drawn as {p} pennants, {f} feathers and {h} halves reads \
                 back as {read:.0}"
            );
        }
    }
}
