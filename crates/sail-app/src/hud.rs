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
use sail_core::autopilot::OVERPOWERED_HEEL;
use sail_core::prelude::*;
use sail_core::wind::{SteadyWind, WindField};

use crate::course::{clock, Course, Leg};
use crate::input::Pilot;
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
    course: Res<Course>,
    pilot: Res<Pilot>,
    bank: Res<crate::clutch::Bank>,
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
            ui.small(
                egui::RichText::new(sim.weather.report()).color(egui::Color32::from_gray(150)),
            );
            ui.separator();
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
            draw_course(ui, &course, &sim);

            ui.separator();
            ui.label(egui::RichText::new("trim").strong());
            let cfg = sim.config();
            egui::Grid::new("trim").num_columns(4).show(ui, |ui| {
                sail_row(
                    ui,
                    "main",
                    c.main_sheet,
                    cfg.main.profile.optimal_chord_angle(t.awa, t.main.camber),
                    &t.main,
                );
                ui.end_row();
                sail_row(
                    ui,
                    "genoa",
                    c.jib_sheet,
                    cfg.genoa.profile.optimal_chord_angle(t.awa, t.jib.camber),
                    &t.jib,
                );
                ui.end_row();
            });

            // The outhaul is a shape control, not a sheet: depth is power, flat is
            // less heeling force. Showing the draft as a percentage is how a sailmaker
            // talks about it, and it is the number the player is actually changing.
            let want_outhaul = outhaul_for_heel(t.heel, OVERPOWERED_HEEL);
            ui.label(format!(
                "outhaul {:.2} ({:.0}% draft)   for this heel {:.2} — {}",
                c.outhaul,
                t.main.camber * 100.0,
                want_outhaul,
                if (c.outhaul - want_outhaul).abs() < 0.12 {
                    "about right"
                } else if c.outhaul < want_outhaul {
                    "overpowered, haul it out"
                } else {
                    "too flat for this heel, ease it off"
                }
            ));
            // The vang reads inert upwind on purpose: with the boom near the
            // centreline the mainsheet is already tensioning the leech, so there is
            // nothing for it to do. Saying so beats letting it look broken.
            ui.label(format!(
                "genoa car {:.2} ({:.0}% draft, twist {:.1})   {}",
                c.genoa_car,
                t.jib.camber * 100.0,
                deg(t.jib_twist),
                if c.genoa_car < 0.35 {
                    "forward — full and powerful"
                } else if c.genoa_car > 0.7 {
                    "aft — flat foot, open leech"
                } else {
                    "mid track"
                }
            ));
            ui.label(format!(
                "vang {:.2}   twist {:.1}{}",
                c.vang,
                deg(t.twist),
                if deg(cfg.main.profile.chord_angle(c.main_sheet)) < 25.0 {
                    "   (boom near centreline — the sheet is doing the vang's job)"
                } else {
                    ""
                }
            ));
            ui.label(format!(
                "main {}    {} {}    stays'l {}    rudder {}",
                // Slabs, not a fraction: a sailor says "second reef", never "0.67".
                match (c.reef * 3.0).round() as i32 {
                    0 => "full".to_string(),
                    n => format!("reef {n}"),
                },
                c.headsail.name(),
                if c.headsail == sail_core::state::Headsail::Genoa {
                    match c.furl {
                        f if f > 0.985 => "rolled away".to_string(),
                        f if f < 0.015 => "full".to_string(),
                        f => format!("{:.0}% rolled", f * 100.0),
                    }
                } else if c.furl > 0.5 {
                    "handed".to_string()
                } else {
                    "set".to_string()
                },
                if c.staysail > 0.5 { "set" } else { "handed" },
                if t.rudder_stalled > 0.4 {
                    "STALLED"
                } else {
                    "biting"
                }
            ));

            ui.separator();
            // One winch, one line at a time. The bank is the interface, so it has to be
            // legible at a glance rather than inferred from five loose numbers.
            ui.label(egui::RichText::new("clutches  (1/2 select, A ease, D haul in)").strong());
            for (i, line) in crate::clutch::BANK.iter().enumerate() {
                let on = i == bank.selected;
                let row = format!(
                    "{} {:<13} {:.2}  {}",
                    if on { "\u{25b6}" } else { " " },
                    line.name(),
                    line.get(c),
                    if on { line.hauled() } else { "" }
                );
                let text = egui::RichText::new(row).monospace();
                ui.label(if on {
                    text.color(egui::Color32::from_rgb(240, 210, 90)).strong()
                } else {
                    text.weak()
                });
            }

            ui.separator();
            // Matched on the mode itself, which makes a new mode a compile error here
            // rather than a silent fall-through. An earlier version keyed off a label
            // string from sail-core: renaming it would have dropped the vane into the
            // heading arm and printed a signed wind angle as a compass course, with
            // nothing to complain about it.
            match pilot.mode {
                PilotMode::Standby => {
                    ui.label(egui::RichText::new("pilot  standby — hand steering").weak());
                    // If it dropped out, say why. A pilot that disengages silently is
                    // indistinguishable from one that is broken.
                    if let Some(reason) = pilot.last_reason {
                        if pilot.since_reason < 6.0 {
                            ui.label(
                                egui::RichText::new(format!("       it dropped out: {reason}"))
                                    .color(egui::Color32::from_rgb(230, 170, 60)),
                            );
                        }
                    }
                }
                PilotMode::ApparentWind(target) => {
                    ui.label(
                        egui::RichText::new(format!(
                            "vane   holding {:.0} apparent   (now {:.0})",
                            deg(target).abs(),
                            deg(t.awa).abs()
                        ))
                        .color(egui::Color32::from_rgb(140, 210, 150))
                        .strong(),
                    );
                    ui.small("'.' closer to the wind    ',' further off");
                }
                PilotMode::Heading(target) => {
                    ui.label(
                        egui::RichText::new(format!(
                            "pilot  holding {:03.0}   (now {:03.0})",
                            deg(target),
                            deg(sim.sim.state.heading)
                        ))
                        .color(egui::Color32::from_rgb(120, 190, 240))
                        .strong(),
                    );
                    ui.small("'.' to starboard    ',' to port");
                }
            }

            ui.separator();
            ui.label(egui::RichText::new("sea").strong());
            ui.label(format!(
                "Hs {:.2} m    heave {:+.2} m    pitch {:+.1}    immersion {:.2} m",
                t.significant_height,
                t.heave,
                deg(t.pitch),
                t.immersion
            ));
            ui.label(format!(
                "added resistance in waves {:.0} N    surf {:+.0} N    vertical {:+.1} m/s2",
                t.resistance_waves, t.surf_force, t.heave_accel
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
                "arrows  helm to port / starboard, held where you leave it\n\
                 Space   centre the helm\n\
                 W / S   mainsheet: haul in / ease\n\
                 E / Q   genoa: grind in / ease\n\
                 Shift   fine, on both helm and sheets\n\
                 1 / 2   choose a line on the clutch bank\n\
                 A / D   ease / haul in the chosen line (Shift for fine)\n\
                 P       pilot on / off      O  course or wind angle\n\
                 , / .   nudge the pilot's target (Shift for five)\n\
                 Z / X   headsail: genoa, code 0, gennaker (roll it away first)\n\
                 M       chart on / off      [ ]  zoom it out and in\n\
                 mouse   hold the right button to look around; Space looks ahead\n\
                 V       picture: sharp, 960, 720, 480 across\n\
                 B       sea: lit or drawn\n\
                 T  snap to trim   C  camera   L  new consignment   K  mute   \
Tab  hide panel",
            );
        });
    Ok(())
}

/// The consignment in hand, and whether it is going to make it.
fn draw_course(ui: &mut egui::Ui, course: &Course, sim: &Sim) {
    let (distance, bearing) = course.to_target(sim.sim.state.pos);
    let t = &sim.sim.telemetry;

    // Progress towards the destination specifically, as opposed to progress upwind.
    // Off the layline these differ, and the difference is the whole art of calling a
    // tack — which is now a routing decision rather than a racing one.
    let to_mark = glam::DVec2::new(bearing.sin(), bearing.cos());
    let closing = sim.sim.state.velocity_world().dot(to_mark);

    let (text, colour) = match course.leg {
        Leg::Carrying if course.remaining() > 0.0 => (
            format!("{} to {}", course.origin_name(), course.destination_name()),
            egui::Color32::from_rgb(120, 190, 240),
        ),
        Leg::Carrying => (
            format!("{} — overdue", course.destination_name()),
            egui::Color32::from_rgb(240, 120, 90),
        ),
        Leg::Idle => (
            "nowhere to deliver to".into(),
            egui::Color32::from_rgb(170, 170, 170),
        ),
    };
    ui.label(egui::RichText::new(text).color(colour).strong());
    egui::Grid::new("course").num_columns(4).show(ui, |ui| {
        row(ui, "to run", &format!("{:.2} nm", distance / 1852.0));
        row(ui, "bearing", &format!("{:03.0}", deg(bearing)));
        ui.end_row();
        row(ui, "closing", &format!("{:.2} kn", ms_to_kn(closing)));
        row(ui, "VMG", &format!("{:.2} kn", ms_to_kn(t.vmg)));
        ui.end_row();
        row(ui, "elapsed", &clock(course.elapsed));
        row(ui, "left", &clock(course.remaining()));
        ui.end_row();
        row(ui, "delivered", &format!("{}", course.delivered));
        row(ui, "late", &format!("{}", course.late));
        ui.end_row();
        // The sounder belongs next to the passage, not next to the sails: it is a
        // navigation instrument and this is the navigation panel.
        row(ui, "depth", &format!("{:.1} m", t.depth));
        row(ui, "under keel", &format!("{:+.1} m", t.under_keel));
        ui.end_row();
    });
    if t.under_keel < 1.0 {
        ui.label(
            egui::RichText::new(if t.under_keel < 0.0 {
                "AGROUND"
            } else {
                "shoaling — water under the keel"
            })
            .color(egui::Color32::from_rgb(240, 120, 90))
            .strong(),
        );
    }
    if let Some(best) = course.best_margin {
        ui.label(
            egui::RichText::new(format!(
                "best delivery had {:.0}% of the time to spare",
                best * 100.0
            ))
            .color(egui::Color32::from_rgb(240, 210, 90)),
        );
    }
}

fn row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(label);
    ui.label(egui::RichText::new(value).monospace().strong());
}

/// One sail's trim, read the way you would read it on the boat: where the boom
/// actually is, where it wants to be, and which way to move the sheet. The sheet
/// number alone means nothing to anyone — the boom angle is the physical thing.
fn sail_row(
    ui: &mut egui::Ui,
    label: &str,
    sheet: f64,
    want: f64,
    f: &sail_core::sail::SailForces,
) {
    ui.label(label);
    ui.label(
        egui::RichText::new(format!(
            "boom {:4.1}  want {:4.1}",
            deg(f.chord_angle),
            deg(want)
        ))
        .monospace(),
    );
    let gap = want - f.chord_angle;
    let call = if gap > rad(1.5) {
        "ease"
    } else if gap < -rad(1.5) {
        "haul in"
    } else {
        "on"
    };
    ui.label(egui::RichText::new(format!("{call:>7}  [{sheet:.2}]")).monospace());
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
