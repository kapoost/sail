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

/// Is the chart out, and how far it is zoomed.
#[derive(Resource)]
pub struct Chart {
    pub open: bool,
    /// Half-width of what is shown, metres.
    pub range: f32,
}

impl Default for Chart {
    fn default() -> Self {
        Self {
            open: true,
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
}

pub fn toggle(keys: Res<ButtonInput<KeyCode>>, mut chart: ResMut<Chart>) {
    if keys.just_pressed(KeyCode::KeyM) {
        chart.open = !chart.open;
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

pub fn draw(
    mut contexts: EguiContexts,
    chart: Res<Chart>,
    sim: Res<Sim>,
    course: Res<Course>,
    track: Res<Track>,
) -> Result {
    if !chart.open {
        return Ok(());
    }
    let Some(land) = sim.sim.land.as_ref() else {
        return Ok(());
    };
    let ctx = contexts.ctx_mut()?;
    let st = &sim.sim.state;

    egui::Area::new("chart".into())
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 12.0))
        .show(ctx, |ui| {
            let size = egui::vec2(330.0, 330.0);
            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            let p = ui.painter_at(rect);
            p.rect_filled(rect, 4.0, ink::DEEP);

            let centre = rect.center();
            let scale = rect.width() * 0.5 / chart.range;
            // Sim north is +y; on paper north is up, so the screen's y runs the other
            // way. Getting this backwards puts every island on the wrong side of you,
            // which is the one mistake a chart must never make.
            let to_screen = |w: glam::DVec2| -> egui::Pos2 {
                let d = w - st.pos;
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

            // Anchorages, with the one in hand picked out.
            for (i, h) in course.harbours.iter().enumerate() {
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

            // The boat: a wedge pointing where she is pointing.
            let heading = st.heading as f32;
            let nose = egui::vec2(heading.sin(), -heading.cos());
            let beam = egui::vec2(-nose.y, nose.x);
            p.add(egui::Shape::convex_polygon(
                vec![
                    centre + nose * 9.0,
                    centre - nose * 5.0 + beam * 4.0,
                    centre - nose * 5.0 - beam * 4.0,
                ],
                ink::BOAT,
                egui::Stroke::NONE,
            ));
            // And the bearing to the consignment, which is the line you are trying to
            // make good rather than the one you are sailing.
            let target = to_screen(course.target());
            p.line_segment([centre, target], egui::Stroke::new(1.0, ink::MARK));

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
            const BARBS: i32 = 3;
            for ix in -BARBS..=BARBS {
                for iy in -BARBS..=BARBS {
                    if (ix + iy) % 2 != 0 {
                        continue; // A quincunx, so they do not crowd into a lattice.
                    }
                    let step = chart.range as f64 / (BARBS as f64 + 0.35);
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
