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
    /// What one look through the glasses made out.
    ///
    /// **A sweep of a coast is one act and one line.** Naming them one to a line put ten
    /// entries on the page at the same minute and pushed the bearings off it — and a
    /// page that has to be read past is a page nobody reads. It is also what a navigator
    /// writes: *made out Nordvik, Sandvoe and the Fuglhavn light*, once, in the book.
    Sighted { names: Vec<String> },
    /// A speed read off the log, with how good the reading was.
    Speed { knots: f64, slip: f64 },
    /// A course she settled on.
    Altered { course: f64 },
    /// A full entry written up at the table.
    ///
    /// It carries the whole line — the glass, the wind, the sea, the sounding, what was
    /// set — because the *book* is where those belong. On the paper they were a wall of
    /// type standing between the navigator and the coast; here they are a column he can
    /// read down. Clicking one lays the chart over the water it was written about.
    Entered(crate::chart::Entry),
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
        if let Wrote::Sighted { names } = &wrote {
            // Nothing already in the book goes in it twice: looking at the same headland
            // for the third time is not a third entry.
            let fresh: Vec<String> = names
                .iter()
                .filter(|name| !self.already(name))
                .cloned()
                .collect();
            if fresh.is_empty() {
                return;
            }
            self.lines.push(Line {
                hour,
                wrote: Wrote::Sighted { names: fresh },
            });
            self.trim();
            return;
        }
        self.lines.push(Line { hour, wrote });
        self.trim();
    }

    /// Whether a mark has already been written down as made out.
    fn already(&self, name: &str) -> bool {
        self.lines.iter().any(|line| match &line.wrote {
            Wrote::Sighted { names } => names.iter().any(|seen| seen == name),
            _ => false,
        })
    }

    fn trim(&mut self) {
        while self.lines.len() > PAGE {
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
        Wrote::Sighted { names } => match names.split_first() {
            Some((first, [])) => format!("made out {first}"),
            Some((first, rest)) => format!("made out {first} and {} more", rest.len()),
            None => "made out nothing".to_string(),
        },
        Wrote::Speed { knots, slip } => format!("log {knots:.1} kn  ±{:.0}%", slip * 100.0),
        Wrote::Altered { course } => {
            format!("altered to {:03.0}", course.to_degrees().rem_euclid(360.0))
        }
        Wrote::Entered(_) => "written up".to_string(),
    }
}

/// Whether clicking the line puts something on the paper that is not there yet.
fn can_be_plotted(wrote: &Wrote) -> bool {
    matches!(wrote, Wrote::Bearing { .. } | Wrote::Sighted { .. })
}

/// Whether this line is on the chart **now** — asked of the chart, not remembered here.
///
/// It used to be a flag on the line, and the flag went stale the first time anything
/// else cleared the sights: entering a fix takes the position lines off the paper, and
/// the book went on claiming they were drawn, so pressing one again took away a line
/// that was not there and the second press was needed to put it back. Reported from the
/// deck as *adding it a second time is impossible*, which is what it was.
///
/// A fact with one owner cannot do that. The sights are the position lines, the names
/// on the paper are the names on the paper, and this reads them.
pub(crate) fn on_the_chart(wrote: &Wrote, nav: &crate::navigation::Navigation) -> bool {
    match wrote {
        Wrote::Bearing { mark, bearing, .. } => nav
            .sights
            .iter()
            .any(|s| (s.mark - *mark).length() < 1.0 && (s.bearing - *bearing).abs() < 1e-9),
        Wrote::Sighted { names } => names.iter().all(|name| nav.on_the_paper.contains(name)),
        _ => false,
    }
}

/// Which page of the book is open.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Page {
    /// The running page: everything written, newest at the bottom, clickable.
    #[default]
    Log,
    /// One entry, in its rubrics.
    Entries,
}

/// What was pressed on the book.
pub(crate) enum Turned {
    /// A line, which is either laid on the chart or taken off it.
    Line(usize),
    /// A tab.
    To(Page),
}

/// The book, lying open down the right-hand side of the table.
///
/// **Two pages, because a log book has two jobs.** The running page is the one you write
/// on — a line per thing that happened, in the order it happened, each one a press away
/// from being on the chart. The entry page is the one you *read*: a single line of the
/// log opened out into its rubrics, which is how a log book has been laid out since
/// before anybody wrote any of this down, because a column of headings you fill in is
/// the only format that makes a missing figure obvious.
///
/// **Beside the chart and not on it.** It began as a rectangle painted on the paper,
/// which meant every press on a line was also a press on the water underneath and had to
/// be excluded by hand — the oldest bug on this chart, and one that keeps coming back
/// wearing different clothes. A book is a separate object on the table. So it is a
/// separate widget, with its own rows that are clicked because egui was asked whether
/// they were clicked, rather than because a point was tested against a rectangle.
pub(crate) fn the_book(
    ui: &mut egui::Ui,
    log: &DeckLog,
    nav: &crate::navigation::Navigation,
    showing: (Page, Option<usize>),
) -> Option<Turned> {
    let (open, chosen) = showing;
    let (page, _) = ui.allocate_exact_size(egui::vec2(312.0, 420.0), egui::Sense::hover());
    let t = &Table {
        p: ui.painter_at(page),
        rect: page,
        scale: 1.0,
        middle: glam::DVec2::ZERO,
        knows: true,
        why: false,
    };
    // Opaque. It is a book on a table, not a pane of glass: the sea showing through the
    // lines made them hard to read and made the page look like something half-drawn.
    t.p.rect_filled(page, 2.0, ink::PAPER);
    t.p.rect_stroke(
        page,
        2.0,
        egui::Stroke::new(1.0, ink::COAST.gamma_multiply(0.6)),
        egui::StrokeKind::Inside,
    );
    let mut turned = None;
    // The tabs, which are the only two things on this page that are not writing.
    let mut x = page.left() + 6.0;
    for (which, name) in [(Page::Log, "DECK LOG"), (Page::Entries, "ENTRY")] {
        let tab =
            egui::Rect::from_min_size(egui::pos2(x, page.top() + 5.0), egui::vec2(78.0, 17.0));
        let on = which == open;
        t.p.rect_filled(tab, 2.0, if on { ink::MARK } else { ink::PAPER });
        t.p.rect_stroke(
            tab,
            2.0,
            egui::Stroke::new(1.0, ink::COAST.gamma_multiply(0.7)),
            egui::StrokeKind::Inside,
        );
        t.p.text(
            tab.center(),
            egui::Align2::CENTER_CENTER,
            name,
            egui::FontId::monospace(9.0),
            ink::COAST,
        );
        if ui
            .interact(tab, ui.id().with(("tab", name)), egui::Sense::click())
            .clicked()
        {
            turned = Some(Turned::To(which));
        }
        x += 84.0;
    }
    match open {
        Page::Log => {
            t.p.text(
                page.left_top() + egui::vec2(8.0, 26.0),
                egui::Align2::LEFT_TOP,
                "+  click a line to lay it on the chart",
                egui::FontId::monospace(9.0),
                ink::BOAT,
            );
            for (i, line) in log.lines.iter().enumerate() {
                let y = page.top() + 42.0 + i as f32 * 14.0;
                if y > page.bottom() - 12.0 {
                    break;
                }
                let row = egui::Rect::from_min_size(
                    egui::pos2(page.left() + 4.0, y - 1.0),
                    egui::vec2(page.width() - 8.0, 13.0),
                );
                if ui
                    .interact(row, ui.id().with(("line", i)), egui::Sense::click())
                    .clicked()
                {
                    turned = Some(Turned::Line(i));
                }
                let drawn = on_the_chart(&line.wrote, nav);
                let waiting = can_be_plotted(&line.wrote) && !drawn;
                if waiting {
                    // A mark in the margin against the lines that are not on the chart
                    // yet, which is what a navigator's own page looks like: the ones he
                    // has plotted are ticked and the rest are waiting for him.
                    t.p.text(
                        egui::pos2(page.left() + 8.0, y),
                        egui::Align2::LEFT_TOP,
                        "+",
                        egui::FontId::monospace(10.0),
                        ink::BOAT,
                    );
                } else if drawn {
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
        }
        Page::Entries => the_rubrics(t, log, page, chosen),
    }
    turned
}

/// One entry, opened out into the rubrics a log book keeps.
///
/// **A column of headings, and the blanks are the point.** A line with no speed against
/// it says she had not streamed the log; one with no *within* says she is not running on
/// a reckoning at all. Written as a sentence those absences disappear; written as a form
/// they are the first thing the eye finds, which is why every log book ever printed is
/// a grid.
fn the_rubrics(t: &Table, log: &DeckLog, page: egui::Rect, chosen: Option<usize>) {
    // The one he asked for, or the last one written.
    let entry = chosen
        .and_then(|i| log.lines.get(i))
        .into_iter()
        .chain(log.lines.iter().rev())
        .find_map(|line| match &line.wrote {
            Wrote::Entered(entry) => Some(entry),
            _ => None,
        });
    let Some(e) = entry else {
        t.p.text(
            page.left_top() + egui::vec2(8.0, 30.0),
            egui::Align2::LEFT_TOP,
            "nothing written up yet.\n\npress WRITE UP THE LOG\nand this page fills.",
            egui::FontId::monospace(9.0),
            ink::COAST,
        );
        return;
    };
    let mut y = page.top() + 32.0;
    let mut rubric = |heading: &str, said: Option<String>| {
        t.p.text(
            egui::pos2(page.left() + 8.0, y),
            egui::Align2::LEFT_TOP,
            heading,
            egui::FontId::monospace(9.0),
            ink::COAST.gamma_multiply(0.8),
        );
        t.p.text(
            egui::pos2(page.left() + 78.0, y),
            egui::Align2::LEFT_TOP,
            // An empty rubric says something: *this was not known*. A dash is how a
            // book says it and is the reason the form beats a sentence.
            said.unwrap_or_else(|| "—".to_string()),
            egui::FontId::monospace(10.0),
            ink::PENCIL,
        );
        y += 15.0;
    };
    rubric(
        "TIME",
        Some(format!(
            "{:02.0}{:02.0}",
            e.hour.floor(),
            (e.hour.fract() * 60.0).floor()
        )),
    );
    rubric(
        "COURSE",
        e.course
            .map(|c| format!("{:03.0}", c.to_degrees().rem_euclid(360.0))),
    );
    rubric("LOG", e.speed.map(|kn| format!("{kn:.1} kn")));
    let way = if e.tendency.abs() < 0.5 {
        "steady"
    } else if e.tendency < 0.0 {
        "falling"
    } else {
        "rising"
    };
    rubric("GLASS", Some(format!("{:.0} {way}", e.glass)));
    rubric(
        "WIND",
        Some(format!(
            "{} {:.0} kn",
            sail_core::units::point(e.wind_from),
            sail_core::units::ms_to_kn(e.wind)
        )),
    );
    rubric("SEA", Some(format!("{:.1} m", e.sea)));
    rubric("SOUNDING", Some(format!("{:.0} m", e.sounding)));
    rubric(
        "SET",
        e.set
            .map(|set| format!("{:.2} kn", sail_core::units::ms_to_kn(set))),
    );
    rubric(
        "WITHIN",
        e.within.map(|within| format!("{:.2} M", within / 1_852.0)),
    );
    rubric("SAILS", Some(crate::chart::what_was_set(e)));
    // And the sail plan drawn, which is the one rubric in a log book that has always
    // been a picture.
    crate::chart::sail_plan(t, egui::pos2(page.left() + 86.0, y + 4.0), e, 22.0);
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
    let Some(line) = log.lines.get(which) else {
        return;
    };
    let drawn = on_the_chart(&line.wrote, nav);
    match &line.wrote {
        Wrote::Bearing {
            mark,
            bearing,
            taken,
            ..
        } => {
            if drawn {
                nav.sights
                    .retain(|s| (s.mark - *mark).length() > 1.0 || s.bearing != *bearing);
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
            }
        }
        Wrote::Sighted { names } => {
            // The names go on the paper. She knew them already — the glasses told her on
            // deck — and this is the pencil, which is a different act and a later one.
            // All of them at once, because one look was one act.
            if drawn {
                nav.on_the_paper.retain(|had| !names.contains(had));
            } else {
                for name in names {
                    if !nav.on_the_paper.contains(name) {
                        nav.on_the_paper.push(name.clone());
                    }
                }
            }
        }
        // An entry is already on the paper; pressing it puts the paper over it, which is
        // what a hand does to a chart when it wants to look at something written an hour
        // ago and three miles back.
        Wrote::Entered(entry) => {
            *laid_at = Some(entry.at);
        }
        _ => {}
    }
}

#[cfg(test)]
mod the_page {
    use super::*;
    use crate::navigation::Navigation;

    /// **Recognising and plotting are two acts, in that order.**
    ///
    /// The glasses put the name in her head and a line in the book; the chart gets it
    /// when a hand at the table says so. The test that matters is the middle state: she
    /// knows the mark and the paper does not, and everything on deck has to work.
    #[test]
    fn a_name_reaches_the_paper_when_it_is_laid_on() {
        let mut log = DeckLog::default();
        let mut nav = Navigation::default();
        // What the glasses do on deck.
        nav.recognised.push("Bergskerry Spire".to_string());
        log.write(
            6.5,
            Wrote::Sighted {
                names: vec!["Bergskerry Spire".to_string()],
            },
        );
        assert!(
            nav.on_the_paper.is_empty(),
            "nothing is drawn until somebody draws it"
        );
        assert!(
            crate::sight::can_be_named("Bergskerry Spire", true, &nav.recognised),
            "but she can shoot it from the deck straight away"
        );
        let mut laid_at = None;
        lay_it_on_the_chart(&mut log, &mut nav, &mut laid_at, 0);
        assert_eq!(nav.on_the_paper, ["Bergskerry Spire"]);
        assert!(
            on_the_chart(&log.lines[0].wrote, &nav),
            "and the book says so"
        );
    }

    /// **One look at a coast is one line**, however many rocks it names.
    ///
    /// Written one to a line, a sweep of an archipelago put ten entries on the page at
    /// the same minute and pushed the bearings off it — and a page that has to be read
    /// past is a page nobody reads.
    #[test]
    fn a_sweep_of_the_coast_is_one_line() {
        let mut log = DeckLog::default();
        log.write(
            6.5,
            Wrote::Sighted {
                names: vec!["Nordvik".into(), "Sandvoe".into(), "Fuglhavn".into()],
            },
        );
        assert_eq!(log.lines.len(), 1);
        // And the next look only writes what is new.
        log.write(
            6.6,
            Wrote::Sighted {
                names: vec!["Sandvoe".into(), "Kirkvoe".into()],
            },
        );
        assert_eq!(log.lines.len(), 2);
        let Wrote::Sighted { names } = &log.lines[1].wrote else {
            panic!("a line of made-out marks")
        };
        assert_eq!(names, &["Kirkvoe".to_string()], "Sandvoe was already in it");
    }

    /// Looking at the same headland three times is one line in the book.
    #[test]
    fn an_identification_is_written_once() {
        let mut log = DeckLog::default();
        for _ in 0..3 {
            log.write(
                6.5,
                Wrote::Sighted {
                    names: vec!["Fuglvoe".to_string()],
                },
            );
        }
        assert_eq!(log.lines.len(), 1);
    }

    /// **A bearing plotted late is transferred by what she ran in between.**
    ///
    /// This is the field the whole book is built around: shoot a light, sail on, plot it
    /// twenty minutes later, and the line lands where that bearing applies *now*. Half a
    /// mile of running through the water between the shot and the pencil is half a mile
    /// of transfer, and nothing else in the game can supply that number afterwards.
    #[test]
    fn a_bearing_carries_the_run_it_was_plotted_after() {
        let mut log = DeckLog::default();
        let mut nav = Navigation::default();
        let mark = glam::DVec2::new(0.0, 4_000.0);
        log.write(
            6.5,
            Wrote::Bearing {
                name: "Nelstad".to_string(),
                mark,
                bearing: 0.0,
                taken: glam::DVec2::new(100.0, 200.0),
            },
        );
        // She runs on before anybody goes below.
        nav.water_track = glam::DVec2::new(100.0, 1_126.0);
        let mut laid_at = None;
        lay_it_on_the_chart(&mut log, &mut nav, &mut laid_at, 0);
        assert_eq!(nav.sights.len(), 1);
        let ran = nav.sights[0].ran;
        assert!(
            (ran - glam::DVec2::new(0.0, 926.0)).length() < 1.0,
            "the transfer is the run between the shot and the pencil, got {ran:?}"
        );
        assert!(
            (nav.sights[0].through() - glam::DVec2::new(0.0, 4_926.0)).length() < 1.0,
            "so the line passes through the mark carried along by it"
        );
        // And the rubber takes it off again, which is how a fix that will not close is
        // argued with.
        lay_it_on_the_chart(&mut log, &mut nav, &mut laid_at, 0);
        assert!(nav.sights.is_empty());
        assert!(
            !on_the_chart(&log.lines[0].wrote, &nav),
            "and the book stops claiming it is drawn"
        );
    }

    /// Pressing a written-up entry lays the chart over the water it was written about.
    #[test]
    fn an_entry_moves_the_paper_rather_than_marking_it() {
        let mut log = DeckLog::default();
        let mut nav = Navigation::default();
        let at = glam::DVec2::new(1_500.0, -800.0);
        log.write(
            7.0,
            Wrote::Entered(crate::chart::Entry {
                at,
                hour: 7.0,
                course: None,
                speed: None,
                glass: 1_013.0,
                tendency: 0.0,
                sounding: 40.0,
                set: None,
                within: None,
                wind_from: 0.0,
                wind: 10.0,
                sea: 1.0,
                reef: 0.0,
                headsail: sail_core::state::Headsail::Genoa,
                furl: 0.0,
            }),
        );
        let mut laid_at = None;
        lay_it_on_the_chart(&mut log, &mut nav, &mut laid_at, 0);
        assert_eq!(laid_at, Some(at));
    }

    /// **A fix takes the position lines off the paper, and the book has to notice.**
    ///
    /// The book used to remember that it had plotted them, so after a fix every bearing
    /// line still claimed to be drawn: pressing one took away a line that was not there
    /// and it took a second press to put it back. Reported from the deck as adding the
    /// data a second time being impossible. Nothing remembers now — the page reads the
    /// chart.
    #[test]
    fn after_a_fix_the_bearings_are_no_longer_claimed_to_be_drawn() {
        let mut log = DeckLog::default();
        let mut nav = Navigation::default();
        log.write(
            6.5,
            Wrote::Bearing {
                name: "Nelstad".to_string(),
                mark: glam::DVec2::new(0.0, 4_000.0),
                bearing: 0.0,
                taken: glam::DVec2::ZERO,
            },
        );
        let mut laid_at = None;
        lay_it_on_the_chart(&mut log, &mut nav, &mut laid_at, 0);
        assert!(on_the_chart(&log.lines[0].wrote, &nav));
        // What entering a fix does to the plot.
        nav.sights.clear();
        assert!(
            !on_the_chart(&log.lines[0].wrote, &nav),
            "the line is off the paper, so the book must not say it is on it"
        );
        // And one press puts it back, rather than two.
        lay_it_on_the_chart(&mut log, &mut nav, &mut laid_at, 0);
        assert_eq!(nav.sights.len(), 1, "one press, one line");
    }
}
