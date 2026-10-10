//! Developer instrument panel.
//!
//! Explicitly *not* the game's interface. The game's interface is the heel of the
//! hull, the shape of the sail, the telltales and the windex — a sailor reads the
//! water, not a dashboard, and a screen carrying compass, log, depth, apparent and
//! true wind at once looks like an airliner's flight deck. This panel exists so I can
//! see whether the physics is telling the truth, and it is bound to Tab so it can get
//! out of the way.
//!
//! The polar overlay is the useful part: it shows where the boat actually is against
//! the theoretical curve for this wind, which catches a physics regression instantly.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use sail_core::prelude::*;
use sail_core::wind::{SteadyWind, WindField};

use crate::sim::Sim;

#[derive(Resource, Default)]
pub struct HudVisible(pub bool);

/// Theoretical speed against true wind angle for the nominal wind, built once.
#[derive(Resource, Default)]
pub struct Polar {
    pub tws: f64,
    /// (true wind angle, boat speed), both SI.
    pub points: Vec<(f64, f64)>,
    pub max_speed: f64,
}

/// Solve the polar at startup. Fifteen steady-state solves is a fraction of a second
/// in release, and having the curve on screen from the first frame is worth it.
pub fn build_polar(mut polar: ResMut<Polar>, sim: Res<Sim>) {
    let cfg = sim.config();
    let tws = sim.nominal_tws;
    let make = || -> Box<dyn WindField> { Box::new(SteadyWind::new(0.0, tws)) };

    let mut points = Vec::new();
    let mut max_speed: f64 = 0.0;
    for twa in [
        30.0, 35.0, 40.0, 45.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0, 110.0, 120.0, 135.0, 150.0,
        165.0, 180.0,
    ] {
        let (_, s) = reef_for_heel(cfg, &make, rad(twa), 300.0, rad(22.0));
        if s.converged {
            points.push((rad(twa), s.telemetry.stw));
            max_speed = max_speed.max(s.telemetry.stw);
        }
    }
    *polar = Polar {
        tws,
        points,
        max_speed,
    };
}

pub fn toggle(keys: Res<ButtonInput<KeyCode>>, mut visible: ResMut<HudVisible>) {
    if keys.just_pressed(KeyCode::Tab) {
        visible.0 = !visible.0;
    }
}

pub fn draw(
    mut contexts: EguiContexts,
    sim: Res<Sim>,
    polar: Res<Polar>,
    visible: Res<HudVisible>,
) -> Result {
    if !visible.0 {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let t = &sim.sim.telemetry;
    let c = &sim.controls;

    egui::Window::new("instruments")
        .default_pos([12.0, 12.0])
        .resizable(false)
        .show(ctx, |ui| {
            egui::Grid::new("nums").num_columns(4).show(ui, |ui| {
                row(ui, "speed", &format!("{:.2} kn", ms_to_kn(t.stw)));
                row(
                    ui,
                    "heading",
                    &format!("{:03.0}", deg(sim.sim.state.heading)),
                );
                ui.end_row();
                row(
                    ui,
                    "true wind",
                    &format!("{:.1} kn / {:03.0}", ms_to_kn(t.tws), deg(t.twd)),
                );
                row(ui, "TWA", &format!("{:.0}", deg(t.twa)));
                ui.end_row();
                row(ui, "apparent", &format!("{:.1} kn", ms_to_kn(t.aws)));
                row(ui, "AWA", &format!("{:.0}", deg(t.awa)));
                ui.end_row();
                row(ui, "heel", &format!("{:.1}", deg(t.heel)));
                row(ui, "leeway", &format!("{:.1}", deg(t.leeway)));
                ui.end_row();
                row(ui, "VMG", &format!("{:.2} kn", ms_to_kn(t.vmg)));
                row(
                    ui,
                    "helm",
                    &format!(
                        "{:+.1}  {}",
                        deg(c.rudder),
                        if c.rudder.abs() < 1e-3 {
                            "amidships"
                        } else if c.rudder > 0.0 {
                            "to stbd"
                        } else {
                            "to port"
                        }
                    ),
                );
                ui.end_row();
            });

            ui.separator();
            ui.label(egui::RichText::new("trim").strong());
            egui::Grid::new("trim").num_columns(4).show(ui, |ui| {
                sail_row(ui, "main", c.main_sheet, &t.main);
                ui.end_row();
                sail_row(ui, "genoa", c.jib_sheet, &t.jib);
                ui.end_row();
            });
            let (want_main, want_jib) = optimal_sheets(&sim.config(), t.awa);
            ui.label(format!(
                "a good trimmer would be at  main {:.2}   genoa {:.2}",
                want_main, want_jib
            ));
            ui.label(format!(
                "reef {:.2}    engine {:+.2}    rudder {}",
                c.reef,
                c.throttle,
                if t.rudder_stalled > 0.4 {
                    "STALLED"
                } else {
                    "biting"
                }
            ));

            ui.separator();
            ui.label(egui::RichText::new("forces").strong());
            ui.label(format!(
                "drive {:.0} N    side {:.0} N    resistance {:.0} N",
                t.drive, t.side, t.resistance
            ));
            ui.label(format!(
                "friction {:.0}  wave {:.0}  induced {:.0}",
                t.resistance_friction, t.resistance_residuary, t.resistance_induced
            ));

            ui.separator();
            draw_polar(ui, &polar, t);

            ui.separator();
            ui.small(
                "A/D or arrows  helm (held where you leave it; Shift for fine)\n\
                 Space  centre the helm\n\
                 W/S mainsheet   Q/E genoa winch   R/F reef   Z/X engine\n\
                 T snap to good trim   C camera   Tab hide this panel",
            );
        });
    Ok(())
}

fn row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(label);
    ui.label(egui::RichText::new(value).monospace().strong());
}

fn sail_row(ui: &mut egui::Ui, label: &str, sheet: f64, f: &sail_core::sail::SailForces) {
    ui.label(label);
    ui.label(egui::RichText::new(format!("sheet {sheet:.2}")).monospace());
    ui.label(egui::RichText::new(format!("aoa {:+.0}", deg(f.alpha))).monospace());
    let (state, colour) = if f.coeffs.luffing > 0.3 {
        ("luffing", egui::Color32::from_rgb(230, 170, 60))
    } else if f.coeffs.stalled > 0.4 {
        ("stalled", egui::Color32::from_rgb(220, 90, 90))
    } else {
        ("drawing", egui::Color32::from_rgb(120, 200, 130))
    };
    ui.label(egui::RichText::new(state).color(colour).strong());
}

/// Half polar: true wind angle as the angle from straight up, boat speed as radius.
/// The marker is where the boat is right now. Inside the curve means there is speed
/// left in the trim; outside means the physics just changed.
fn draw_polar(ui: &mut egui::Ui, polar: &Polar, t: &Telemetry) {
    ui.label(egui::RichText::new("polar").strong());
    if polar.points.is_empty() {
        ui.label("not solved");
        return;
    }
    let size = egui::vec2(188.0, 188.0);
    let (response, painter) = ui.allocate_painter(size, egui::Sense::hover());
    let rect = response.rect;
    let origin = egui::pos2(rect.left() + 10.0, rect.top() + 10.0);
    let radius = (size.x.min(size.y) - 24.0).max(40.0);
    let scale = radius / polar.max_speed.max(0.1) as f32;

    let faint = egui::Color32::from_gray(90);
    // Speed rings, every two knots.
    let mut ring = 2.0;
    while kn_to_ms(ring) < polar.max_speed * 1.05 {
        let r = kn_to_ms(ring) as f32 * scale;
        painter.circle_stroke(origin, r, egui::Stroke::new(0.6, faint));
        ring += 2.0;
    }
    // Wind axis.
    painter.line_segment(
        [origin, origin + egui::vec2(0.0, radius)],
        egui::Stroke::new(0.8, faint),
    );

    // The curve. Wind comes from straight up, so a true wind angle of zero points up.
    let project = |twa: f64, speed: f64| -> egui::Pos2 {
        let r = speed as f32 * scale;
        origin + egui::vec2(r * twa.sin() as f32, r * twa.cos() as f32)
    };
    let curve: Vec<egui::Pos2> = polar
        .points
        .iter()
        .map(|(twa, s)| project(*twa, *s))
        .collect();
    painter.add(egui::Shape::line(
        curve,
        egui::Stroke::new(1.6, egui::Color32::from_rgb(110, 170, 220)),
    ));

    // Where the boat actually is.
    let here = project(t.twa.abs(), t.stw);
    painter.circle_filled(here, 4.0, egui::Color32::from_rgb(240, 200, 80));
    painter.text(
        origin + egui::vec2(6.0, -2.0),
        egui::Align2::LEFT_TOP,
        format!("{:.0} kn true", ms_to_kn(polar.tws)),
        egui::FontId::proportional(10.0),
        faint,
    );
}
