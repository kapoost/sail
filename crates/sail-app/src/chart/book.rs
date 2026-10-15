//! The deck log: what was seen, when, and what has been laid on the chart because of it.
//!
//! **A chart is not where a navigator writes things down.** The book is: every bearing,
//! every identification, every reading off the log, each with the time against it — and
//! the chart gets a line drawn on it afterwards, deliberately, because somebody decided
//! that bearing was worth plotting. That order is not ceremony. A bearing written in the
//! book and not plotted is still a bearing you have; a line drawn on the chart you
//! cannot account for is a line nobody can check.
//!
//! So this page fills by itself and the paper does not. Click a line and it goes on the
//! chart: a bearing becomes a position line, an identification puts a name beside the
//! symbol. Click it again and it comes off, which is the pencil and the rubber the way
//! round that a navigator actually uses them.

use bevy::prelude::Resource;
use bevy_egui::egui;

use super::*;

/// One line of the book.
#[derive(Clone, Debug)]
pub struct Line {
    /// Ship's time, which is the only thing every line here has.
    pub hour: f32,
    pub wrote: Wrote,
    /// Whether it has been laid on the chart.
    pub plotted: bool,
}

/// What the line says.
#[derive(Clone, Debug)]
pub enum Wrote {
    /// A bearing on something she could name.
    ///
    /// `taken` is how far she had run through the water when it was shot, so that a
    /// bearing plotted twenty minutes later is transferred by the right distance rather
    /// than drawn where it would have gone at the time. The whole reason the book can
    /// be plotted late is in that one field.
    Bearing {
        name: String,
        mark: glam::DVec2,
        bearing: f64,
        taken: glam::DVec2,
    },
    /// Something identified through the glasses.
    Sighted { name: String },
    /// A speed read off the log, with how good the reading was.
    Speed { knots: f64, slip: f64 },
    /// A course she settled on.
    Altered { course: f64 },
    /// A full entry written up at the table. Already on the paper; clicking it lays the
    /// chart over the place it was written.
    Entered { at: glam::DVec2 },
}

/// The book itself.
#[derive(Resource, Default)]
pub struct DeckLog {
    pub lines: Vec<Line>,
}

/// How many lines a page holds before the top of it goes.
///
/// A watch of four hours at a line every few minutes, which is what a page of a real one
/// holds and about as much as can be read without scrolling something.
const PAGE: usize = 14;

impl DeckLog {
    /// Write a line, and say whether it was new.
    ///
    /// Identifications are written once: looking at the same headland for the third time
    /// is not three entries, and a page that filled with them would bury the bearings.
    pub fn write(&mut self, hour: f32, wrote: Wrote) {
        if let Wrote::Sighted { name } = &wrote {
            let had = self.lines.iter().any(|l| match &l.wrote {
                Wrote::Sighted { name: seen } => seen == name,
                _ => false,
            });
            if had {
                return;
            }
        }
        self.lines.push(Line {
            hour,
            wrote,
            plotted: false,
        });
        if self.lines.len() > PAGE {
            self.lines.remove(0);
        }
    }
}

/// What a line says on the page.
fn reads(wrote: &Wrote) -> String {
    match wrote {
        Wrote::Bearing { name, bearing, .. } => {
            format!("{name} {:03.0}", bearing.to_degrees().rem_euclid(360.0))
        }
        Wrote::Sighted { name } => format!("made out {name}"),
        Wrote::Speed { knots, slip } => format!("log {knots:.1} kn  ±{:.0}%", slip * 100.0),
        Wrote::Altered { course } => {
            format!("altered to {:03.0}", course.to_degrees().rem_euclid(360.0))
        }
        Wrote::Entered { .. } => "written up".to_string(),
    }
}

/// Whether clicking the line puts something on the paper that is not there yet.
fn can_be_plotted(wrote: &Wrote) -> bool {
    matches!(wrote, Wrote::Bearing { .. } | Wrote::Sighted { .. })
}

/// The page, down the right-hand side of the table where a book lies open beside a
/// chart.
///
/// Returns the line that was pressed, and the rectangle it occupies so that the press
/// does not also land on the water underneath — which is the oldest bug on this chart
/// and the one that keeps coming back wearing different clothes.
pub(crate) fn the_book(
    t: &Table,
    log: &DeckLog,
    page: egui::Rect,
    pressed: Option<egui::Pos2>,
) -> Option<usize> {
    t.p.rect_filled(page, 2.0, ink::PAPER.gamma_multiply(0.75));
    t.p.rect_stroke(
        page,
        2.0,
        egui::Stroke::new(1.0, ink::COAST.gamma_multiply(0.6)),
        egui::StrokeKind::Inside,
    );
    t.p.text(
        page.left_top() + egui::vec2(8.0, 6.0),
        egui::Align2::LEFT_TOP,
        "DECK LOG",
        egui::FontId::monospace(10.0),
        ink::COAST,
    );
    t.p.text(
        page.left_top() + egui::vec2(8.0, 20.0),
        egui::Align2::LEFT_TOP,
        "click a line to lay it on the chart",
        egui::FontId::monospace(8.0),
        ink::COAST.gamma_multiply(0.8),
    );
    let mut hit = None;
    for (i, line) in log.lines.iter().enumerate() {
        let y = page.top() + 36.0 + i as f32 * 14.0;
        if y > page.bottom() - 12.0 {
            break;
        }
        let row = egui::Rect::from_min_size(
            egui::pos2(page.left() + 4.0, y - 1.0),
            egui::vec2(page.width() - 8.0, 13.0),
        );
        let mine = pressed.is_some_and(|at| row.contains(at));
        if mine {
            hit = Some(i);
        }
        let waiting = can_be_plotted(&line.wrote) && !line.plotted;
        if waiting {
            // A mark in the margin against the lines that are not on the chart yet,
            // which is what a navigator's own page looks like: the ones he has plotted
            // are ticked and the rest are waiting for him.
            t.p.text(
                egui::pos2(page.left() + 8.0, y),
                egui::Align2::LEFT_TOP,
                "+",
                egui::FontId::monospace(10.0),
                ink::BOAT,
            );
        } else if line.plotted {
            t.p.circle_filled(egui::pos2(page.left() + 11.0, y + 6.0), 1.6, ink::PENCIL);
        }
        t.p.text(
            egui::pos2(page.left() + 20.0, y),
            egui::Align2::LEFT_TOP,
            format!(
                "{:02.0}{:02.0}  {}",
                line.hour.floor(),
                (line.hour.fract() * 60.0).floor(),
                reads(&line.wrote)
            ),
            egui::FontId::monospace(10.0),
            if waiting { ink::PENCIL } else { ink::COAST },
        );
    }
    hit
}

/// Lay a line on the chart, or take it off again.
///
/// **A bearing goes on with the run it has made since it was taken**, which is what lets
/// the book be plotted late at all: shoot a light, sail on for twenty minutes, plot it
/// then, and the line lands where that bearing applies *now*. Taking it off again is the
/// rubber — and the rubber is a navigator's most-used instrument, because the usual
/// reason a fix will not close is a line you should not have believed.
pub(crate) fn lay_it_on_the_chart(
    log: &mut DeckLog,
    nav: &mut crate::navigation::Navigation,
    laid_at: &mut Option<glam::DVec2>,
    which: usize,
) {
    let Some(line) = log.lines.get_mut(which) else {
        return;
    };
    match &line.wrote {
        Wrote::Bearing {
            mark,
            bearing,
            taken,
            ..
        } => {
            if line.plotted {
                nav.sights
                    .retain(|s| (s.mark - *mark).length() > 1.0 || s.bearing != *bearing);
                line.plotted = false;
            } else {
                nav.sights.push(sail_core::fix::Sight {
                    mark: *mark,
                    bearing: *bearing,
                    ran: nav.water_track - *taken,
                });
                // Three is a fix and four is a navigator who has stopped sailing.
                if nav.sights.len() > 3 {
                    nav.sights.remove(0);
                }
                line.plotted = true;
            }
        }
        Wrote::Sighted { name } => {
            if !nav.recognised.contains(name) {
                nav.recognised.push(name.clone());
            }
            line.plotted = true;
        }
        // An entry is already on the paper; pressing it puts the paper over it, which is
        // what a hand does to a chart when it wants to look at something written an hour
        // ago and three miles back.
        Wrote::Entered { at } => {
            *laid_at = Some(*at);
        }
        _ => {}
    }
}
