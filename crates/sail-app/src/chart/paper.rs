//! The paper itself: the grid, the land, the scale, the rose, the weather and the
//! marks.
//!
//! None of this is navigation. It is the sheet the navigation is done on, and it is
//! drawn first and asks nothing about where she is — except the marks, which name
//! themselves only when she can tell which one she is looking at.

use bevy_egui::egui;
use sail_core::land::Mark;

use super::*;
use crate::course::Course;
use crate::sim::Sim;

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
pub(crate) fn barb(
    p: &egui::Painter,
    at: egui::Pos2,
    from: f64,
    knots: f64,
    colour: egui::Color32,
) {
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
pub(crate) fn stream_arrow(
    p: &egui::Painter,
    from: egui::Pos2,
    to: egui::Pos2,
    colour: egui::Color32,
) {
    let stroke = egui::Stroke::new(1.0, colour);
    p.line_segment([from, to], stroke);
    let along = (to - from).normalized();
    let across = egui::vec2(along.y, -along.x);
    for side in [1.0_f32, -1.0] {
        p.line_segment([to, to - along * 6.0 + across * (4.0 * side)], stroke);
    }
}

/// The rose, printed on every chart ever made and not as decoration: it is what a
/// parallel rule is laid against, and how you tell which way is north on a chart
/// somebody has turned round on the table.
pub(crate) fn compass_rose(t: &Table) {
    // --- The compass rose --------------------------------------------
    //
    // Printed on every chart ever made, and not as decoration: it is what a
    // parallel rule is laid against to read a bearing off, and it is how you can
    // tell at a glance which way is north on a chart somebody has turned round on
    // the table. Graduated every ten degrees and numbered every thirty.
    let rose = egui::pos2(t.rect.right() - 86.0, t.rect.bottom() - 86.0);
    t.p.circle(
        rose,
        56.0,
        egui::Color32::TRANSPARENT,
        egui::Stroke::new(1.0, ink::COAST),
    );
    t.p.circle(
        rose,
        44.0,
        egui::Color32::TRANSPARENT,
        egui::Stroke::new(0.7, ink::COAST),
    );
    for tick in 0..36 {
        let a = (tick as f32 * 10.0).to_radians();
        let dir = egui::vec2(a.sin(), -a.cos());
        let long = tick % 3 == 0;
        t.p.line_segment(
            [
                rose + dir * if long { 44.0 } else { 50.0 },
                rose + dir * 56.0,
            ],
            egui::Stroke::new(if long { 1.1 } else { 0.6 }, ink::COAST),
        );
        if long {
            t.p.text(
                rose + dir * 34.0,
                egui::Align2::CENTER_CENTER,
                format!("{:03}", tick * 10),
                egui::FontId::monospace(8.0),
                ink::COAST,
            );
        }
    }
    t.p.add(egui::Shape::convex_polygon(
        vec![
            rose + egui::vec2(0.0, -56.0),
            rose + egui::vec2(-4.5, -40.0),
            rose + egui::vec2(4.5, -40.0),
        ],
        ink::BOAT,
        egui::Stroke::NONE,
    ));
}

/// The forecast, pencilled on: wind barbs over the water the way a navigator writes
/// the afternoon's wind on a chart he has corrected in ink.
pub(crate) fn weather_on_the_chart(
    t: &Table,
    sim: &Sim,
    land: &sail_core::land::Archipelago,
    range: f64,
    here: glam::DVec2,
    now: f64,
) {
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
            let step = range * 0.62;
            let at = here + glam::DVec2::new(ix as f64 * step, iy as f64 * step);
            // Nothing over the land: a wind arrow on a hillside is a symbol
            // placed where its quantity does not exist.
            if land.depth(at) <= 0.0 {
                continue;
            }
            let screen = t.to_screen(at);
            if !t.rect.contains(screen) {
                continue;
            }
            let flow = sim.sim.wind.sample(at, now);
            barb(
                &t.p,
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
        let at = t.to_screen(centre);
        let r = radius as f32 * t.scale;
        if r < 2.0 {
            continue;
        }
        let ring = ink::SQUALL.gamma_multiply(0.35 + 0.65 * maturity as f32);
        t.p.circle_stroke(at, r, egui::Stroke::new(1.0, ring));
        t.p.circle_stroke(at, r * 0.55, egui::Stroke::new(1.0, ring));
    }
}

/// The things you navigate by, drawn the way a chart draws them: a symbol you can
/// tell apart at a glance, a name, and for a light its character.
///
/// Nothing is named while she is lost and has not recognised it — a symbol on a chart
/// with no name is what an unidentified mark is, and the naming is the exercise.
#[allow(clippy::too_many_arguments)]
pub(crate) fn marks_on_the_paper(
    t: &Table,
    coast: &crate::marks::Coast,
    nav: &crate::navigation::Navigation,
    range: f64,
    spread: bool,
    course: &Course,
    home: &crate::anchorage::Home,
) {
    // --- The marks ---------------------------------------------------
    //
    // The things you navigate by, drawn the way a chart draws them: a symbol you
    // can tell apart at a glance, a name, and for a light its character — which
    // is the only thing that identifies it in the dark and is therefore the one
    // piece of text on a chart that is worth more than the name beside it.
    for mark in &coast.marks {
        let at = t.to_screen(mark.at);
        if !t.rect.contains(at) {
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
        if nav.lost && !nav.on_the_paper.contains(&mark.name) {
            continue;
        }
        match mark.kind {
            Mark::Lighthouse => {
                // The magenta flare: a dot with a teardrop out of it, which is
                // how every chart in the world says "light" and has since they
                // were engraved.
                t.p.circle_filled(at, 2.6, ink::LIGHT);
                for i in 0..7 {
                    let a = -0.9 + i as f32 * 0.3;
                    t.p.line_segment(
                        [at, at + egui::vec2(a.sin(), -a.cos()) * 11.0],
                        egui::Stroke::new(1.0, ink::LIGHT),
                    );
                }
            }
            Mark::Spire => {
                // A circle with a dot in it and a cross on top: the chart symbol
                // for a church, and the dot is the bit that is the position.
                t.p.circle(
                    at,
                    3.4,
                    egui::Color32::TRANSPARENT,
                    egui::Stroke::new(1.1, ink::COAST),
                );
                t.p.circle_filled(at, 1.0, ink::COAST);
                t.p.line_segment(
                    [at + egui::vec2(0.0, -4.0), at + egui::vec2(0.0, -8.5)],
                    egui::Stroke::new(1.1, ink::COAST),
                );
                t.p.line_segment(
                    [at + egui::vec2(-2.0, -6.5), at + egui::vec2(2.0, -6.5)],
                    egui::Stroke::new(1.1, ink::COAST),
                );
            }
            Mark::Beacon => {
                t.p.line_segment(
                    [at, at + egui::vec2(0.0, -9.0)],
                    egui::Stroke::new(1.2, ink::COAST),
                );
                t.p.circle_filled(at + egui::vec2(0.0, -10.0), 2.0, ink::COAST);
                t.p.circle_filled(at, 1.4, ink::COAST);
            }
            Mark::Cairn => {
                t.p.circle_filled(at, 1.8, ink::COAST);
                t.p.circle(
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
        let room = t.rect.contains(at + egui::vec2(120.0, 0.0))
            && at.y > t.rect.top() + 14.0
            && at.y < t.rect.bottom() - 14.0;
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
            t.p.text(
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
        let at = t.to_screen(h.at);
        let bound = i == course.to;
        t.p.circle(
            at,
            if bound { 5.0 } else { 3.0 },
            if bound { ink::MARK } else { ink::PAPER },
            egui::Stroke::new(1.0, ink::COAST),
        );
        if bound || range < 6_000.0 {
            t.p.text(
                at + egui::vec2(7.0, -6.0),
                egui::Align2::LEFT_TOP,
                &h.name,
                egui::FontId::monospace(10.0),
                ink::COAST,
            );
        }
    }

    // **And the platform**, which is the one thing on this paper that is yours.
    //
    // Drawn whether or not she is lost, because it is not a fix: a mooring is printed on
    // the chart the day it is laid, and knowing where your own berth is tells you nothing
    // about where *you* are. A diamond rather than a circle, so that at a glance it is
    // not another anchorage — and the word, because a symbol nobody has seen before needs
    // one the first time.
    if let Some(berth) = home.moored {
        let at = t.to_screen(berth);
        if t.rect.contains(at) {
            let r = 5.0;
            let diamond = [
                at + egui::vec2(0.0, -r),
                at + egui::vec2(r, 0.0),
                at + egui::vec2(0.0, r),
                at + egui::vec2(-r, 0.0),
            ];
            t.p.add(egui::Shape::convex_polygon(
                diamond.to_vec(),
                ink::PAPER,
                egui::Stroke::new(1.2, ink::MARK),
            ));
            t.p.circle_filled(at, 1.6, ink::MARK);
            t.p.text(
                at + egui::vec2(8.0, -6.0),
                egui::Align2::LEFT_TOP,
                "home",
                egui::FontId::monospace(10.0),
                ink::MARK,
            );
        }
    }
}

/// The water itself: the grid, the land and where she has been.
///
/// Nothing here is navigation — it is the paper the navigation is done on, which is why
/// it is one function and the first thing drawn. The one judgement in it is the shoal,
/// drawn first and wider than the land, because the two-metre contour is what stops this
/// boat and the rocks are only what happens afterwards.
pub(crate) fn the_water(
    t: &Table,
    land: &sail_core::land::Archipelago,
    track: &Track,
    range: f64,
    pos: glam::DVec2,
) {
    // A kilometre grid, so distance on the chart means something.
    let step = if range > 6_000.0 { 5_000.0 } else { 1_000.0 };
    let first = ((pos.x - range) / step).ceil() * step;
    let mut g = first;
    while g < pos.x + range {
        let x = t.to_screen(glam::DVec2::new(g, pos.y)).x;
        t.p.line_segment(
            [egui::pos2(x, t.rect.top()), egui::pos2(x, t.rect.bottom())],
            egui::Stroke::new(0.5, ink::GRID),
        );
        g += step;
    }
    let first = ((pos.y - range) / step).ceil() * step;
    let mut g = first;
    while g < pos.y + range {
        let y = t.to_screen(glam::DVec2::new(pos.x, g)).y;
        t.p.line_segment(
            [egui::pos2(t.rect.left(), y), egui::pos2(t.rect.right(), y)],
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
                    t.to_screen(island.centre + glam::DVec2::new(a.sin(), a.cos()) * r)
                })
                .collect()
        };
        // The two-metre contour, which is where this boat stops.
        t.p.add(egui::Shape::convex_polygon(
            outline(110.0),
            ink::SHOAL,
            egui::Stroke::NONE,
        ));
        t.p.add(egui::Shape::convex_polygon(
            outline(0.0),
            ink::LAND,
            egui::Stroke::new(1.2, ink::COAST),
        ));
    }

    // Where she has been — **and only while the chart may say where she is.**
    //
    // The track is her true path, recorded off the simulation, and it was drawn
    // unconditionally: six seconds after being cast adrift there was a short line on
    // the paper with one end at the boat, which is the position handed over in the
    // one colour nobody looks at twice. Reported from the deck as the log's line
    // still showing, which it was not — it was this, and it had been there all
    // along.
    //
    // It is not recorded while she is lost either (see `record`), so the plot starts
    // again at the fix: which is what a navigator's chart actually looks like, a
    // line beginning where he last knew.
    if t.knows && track.points.len() > 1 {
        let pts: Vec<egui::Pos2> = track.points.iter().map(|q| t.to_screen(*q)).collect();
        t.p.add(egui::Shape::line(pts, egui::Stroke::new(1.2, ink::TRACK)));
    }
}

/// The scale: a mile, drawn as a mile is drawn — ten cables, alternating.
///
/// A chart has to carry one or its distances are a rumour, and it is the first thing a
/// navigator's eye goes to on a strange sheet.
pub(crate) fn the_scale(t: &Table) {
    // The t.scale, which a chart has to carry or its distances are a rumour.
    // A mile, drawn as a mile is drawn: ten cables, alternating.
    let mile = 1_852.0 * t.scale;
    let bar = egui::pos2(t.rect.left() + 24.0, t.rect.bottom() - 26.0);
    for i in 0..10 {
        let from = bar + egui::vec2(mile * i as f32 / 10.0, 0.0);
        let to = bar + egui::vec2(mile * (i + 1) as f32 / 10.0, 0.0);
        t.p.line_segment(
            [from, to],
            egui::Stroke::new(5.0, if i % 2 == 0 { ink::COAST } else { ink::PAPER }),
        );
    }
    t.p.text(
        bar + egui::vec2(mile + 8.0, 0.0),
        egui::Align2::LEFT_CENTER,
        "1 nautical mile",
        egui::FontId::monospace(11.0),
        ink::COAST,
    );
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
