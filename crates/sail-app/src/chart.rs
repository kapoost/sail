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
