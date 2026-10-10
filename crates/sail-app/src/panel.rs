//! The ship's console: flat instruments and controls, drawn over the view.
//!
//! The older simulators had this right. A rank of painted dials along the bottom of
//! the screen and a column of them down one side, every one of them something you
//! could put a mouse on — no modes, no menus, nothing hidden. Silent Hunter III did
//! its stations this way and they work better than the modelled interiors that
//! replaced them, because a painted panel can put everything within one glance and a
//! three-dimensional one cannot.
//!
//! Two rules hold this together:
//!
//! * **Everything the keyboard can do, the mouse can do.** A control that exists only
//!   as a key is a control nobody discovers. Every line, lever and switch here is
//!   draggable or clickable, and the keys are the shortcut rather than the interface.
//! * **Nothing is drawn as a number that is better drawn as a position.** A needle
//!   against a scale and a lever against its travel are read at a glance; "0.42" has
//!   to be interpreted.
//!
//! The developer panel in `hud.rs` is a separate thing and stays behind Tab. This is
//! the boat; that is the microscope.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use sail_core::prelude::*;

use crate::camera::View;
use crate::clutch::{Bank, BANK};
use crate::course::{clock, Course, Leg};
use crate::input::Pilot;
use crate::sim::Sim;
use crate::sound::SoundHandle;

/// Painted metal, cream dial faces, red needles. A deliberately small palette: the
/// old panels were cheap to print and that is exactly why they read so well.
mod paint {
    use bevy_egui::egui::Color32;
    pub const CASE: Color32 = Color32::from_rgb(38, 40, 43);
    pub const BEZEL: Color32 = Color32::from_rgb(62, 65, 69);
    pub const FACE: Color32 = Color32::from_rgb(222, 220, 208);
    pub const INK: Color32 = Color32::from_rgb(38, 38, 40);
    pub const NEEDLE: Color32 = Color32::from_rgb(186, 44, 34);
    pub const LIVE: Color32 = Color32::from_rgb(232, 176, 58);
    pub const GOOD: Color32 = Color32::from_rgb(120, 190, 130);
    pub const TRACK: Color32 = Color32::from_rgb(26, 27, 29);
    pub const LABEL: Color32 = Color32::from_rgb(176, 178, 182);
}

#[derive(Resource, Default)]
pub struct Console {
    /// Hidden with H, for a clean look at the water.
    pub hidden: bool,
}

pub fn setup(mut commands: Commands) {
    commands.init_resource::<Console>();
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut contexts: EguiContexts,
    mut sim: ResMut<Sim>,
    mut bank: ResMut<Bank>,
    mut pilot: ResMut<Pilot>,
    mut view: ResMut<View>,
    mut console: ResMut<Console>,
    mut hud: ResMut<crate::hud::HudVisible>,
    course: Res<Course>,
    sound: Res<SoundHandle>,
    keys: Res<ButtonInput<KeyCode>>,
) -> Result {
    if keys.just_pressed(KeyCode::KeyH) {
        console.hidden = !console.hidden;
    }
    if console.hidden {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let cfg = sim.config();
    let telemetry = sim.sim.telemetry;
    let heading = sim.sim.state.heading;

    let frame = egui::Frame::new()
        .fill(paint::CASE)
        .stroke(egui::Stroke::new(1.0, paint::BEZEL))
        .inner_margin(egui::Margin::symmetric(10, 8));

    // Floating blocks sized to their contents, not panels that reserve a full edge of
    // the screen. A panel took the whole height of the window for a column of
    // readouts a third as tall, and the water is the thing worth looking at.
    egui::Area::new("instruments".into())
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(8.0, 8.0))
        .interactable(true)
        .show(ctx, |ui| {
            frame.show(ui, |ui| {
                ui.set_width(186.0);
                ui.spacing_mut().item_spacing.y = 6.0;

                ui.horizontal(|ui| {
                    wind_dial(ui, telemetry.awa, telemetry.twa);
                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        gauge(ui, "LOG", ms_to_kn(telemetry.stw), 0.0, 10.0, "kn");
                        gauge(ui, "WIND", ms_to_kn(telemetry.aws), 0.0, 30.0, "kn");
                    });
                });

                heel_scale(ui, telemetry.heel);

                ui.add_space(2.0);
                readout(ui, "HDG", &format!("{:03.0}", deg(heading)));
                readout(ui, "TWA", &format!("{:+.0}", deg(telemetry.twa)));
                readout(ui, "VMG", &format!("{:.2} kn", ms_to_kn(telemetry.vmg)));

                ui.add_space(4.0);
                ui.separator();
                let (distance, bearing) = course.to_target(sim.sim.state.pos);
                ui.label(
                    egui::RichText::new(match course.leg {
                        Leg::Upwind => "TO WINDWARD MARK",
                        Leg::Downwind => "RUNNING HOME",
                        Leg::Finished => "LAP COMPLETE",
                    })
                    .color(paint::LIVE)
                    .monospace()
                    .size(11.0),
                );
                readout(ui, "RANGE", &format!("{distance:.0} m"));
                readout(ui, "BRG", &format!("{:03.0}", deg(bearing)));
                readout(ui, "TIME", &clock(course.elapsed));
                readout(ui, "TACKS", &format!("{}", course.tacks));
            });
        });

    // --- Controls along the bottom -------------------------------------------
    egui::Area::new("console".into())
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -8.0))
        .interactable(true)
        .show(ctx, |ui| {
            frame.show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    // Helm. A tiller is a position your hand holds, so it is a bar you
                    // drag and it stays where you leave it.
                    ui.vertical(|ui| {
                        let max = cfg.max_rudder_angle;
                        let mut rudder = sim.controls.rudder;
                        if tiller(ui, &mut rudder, max) {
                            sim.controls.rudder = rudder;
                            pilot.disengage();
                        }
                        ui.horizontal(|ui| {
                            if small_button(ui, "AMIDSHIPS").clicked() {
                                sim.controls.rudder = 0.0;
                                pilot.disengage();
                            }
                        });
                    });

                    ui.add_space(10.0);
                    separator(ui);
                    ui.add_space(10.0);

                    // Sheets, on their own winches, continuously trimmed.
                    let want_main = cfg
                        .main
                        .profile
                        .optimal_sheet(telemetry.awa, telemetry.main.camber);
                    let want_jib = cfg
                        .genoa
                        .profile
                        .optimal_sheet(telemetry.awa, telemetry.jib.camber);
                    let mut main_sheet = sim.controls.main_sheet;
                    let mut jib_sheet = sim.controls.jib_sheet;
                    if lever(
                        ui,
                        "MAIN",
                        &mut main_sheet,
                        Some(want_main),
                        sail_state(&telemetry.main),
                    ) {
                        sim.controls.main_sheet = main_sheet;
                    }
                    if lever(
                        ui,
                        "GENOA",
                        &mut jib_sheet,
                        Some(want_jib),
                        sail_state(&telemetry.jib),
                    ) {
                        sim.controls.jib_sheet = jib_sheet;
                    }

                    ui.add_space(10.0);
                    separator(ui);
                    ui.add_space(10.0);

                    // The traveller, laid out across because that is how it moves. A
                    // vertical lever would be a lie about a fitting whose entire meaning
                    // is which side of the boat it is standing on.
                    let mut traveller = sim.controls.traveller;
                    if traveller_track(ui, &mut traveller, -telemetry.awa.signum()) {
                        sim.controls.traveller = traveller;
                    }

                    ui.add_space(10.0);
                    separator(ui);
                    ui.add_space(10.0);

                    // The clutch bank: one winch, one line at a time.
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("CLUTCHES")
                                .color(paint::LABEL)
                                .monospace()
                                .size(10.0),
                        );
                        ui.horizontal(|ui| {
                            for (i, line) in BANK.iter().enumerate() {
                                let mut value = line.get(&sim.controls);
                                let selected = i == bank.selected;
                                let (clicked, dragged) =
                                    clutch(ui, line.name(), &mut value, selected);
                                if clicked {
                                    bank.selected = i;
                                    bank.since_change = 0.0;
                                }
                                if dragged {
                                    bank.selected = i;
                                    line.set_from_ui(&mut sim.controls, value);
                                }
                            }
                        });
                        ui.label(
                            egui::RichText::new(format!("hauling in {}", bank.line().hauled()))
                                .color(paint::LABEL)
                                .size(10.0),
                        );
                    });

                    ui.add_space(10.0);
                    separator(ui);
                    ui.add_space(10.0);

                    // Engine and the pilot.
                    ui.vertical(|ui| {
                        let mut throttle = sim.controls.throttle;
                        if throttle_lever(ui, &mut throttle) {
                            sim.controls.throttle = throttle;
                        }
                    });

                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("PILOT")
                                .color(paint::LABEL)
                                .monospace()
                                .size(10.0),
                        );
                        let engaged = pilot.engaged();
                        if lamp_button(ui, if engaged { "ENGAGED" } else { "STANDBY" }, engaged)
                            .clicked()
                        {
                            if engaged {
                                pilot.disengage();
                            } else {
                                let state = sim.sim.state;
                                pilot.hold_heading(&state);
                            }
                        }
                        let wind_mode = matches!(pilot.mode, PilotMode::ApparentWind(_));
                        if lamp_button(
                            ui,
                            if wind_mode { "WIND VANE" } else { "COMPASS" },
                            wind_mode,
                        )
                        .clicked()
                            && pilot.engaged()
                        {
                            if wind_mode {
                                let state = sim.sim.state;
                                pilot.hold_heading(&state);
                            } else {
                                pilot.hold_wind(&telemetry);
                            }
                        }
                        ui.horizontal(|ui| {
                            if small_button(ui, "<").clicked() {
                                pilot.adjust(-rad(1.0));
                            }
                            if small_button(ui, ">").clicked() {
                                pilot.adjust(rad(1.0));
                            }
                        });
                    });

                    ui.add_space(10.0);
                    separator(ui);
                    ui.add_space(10.0);

                    // Everything else that had only a key.
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("SHIP")
                                .color(paint::LABEL)
                                .monospace()
                                .size(10.0),
                        );
                        if small_button(ui, "TRIM TO BEST").clicked() {
                            let (m, j) = optimal_sheets(&cfg, telemetry.awa, sim.controls.outhaul);
                            sim.controls.main_sheet = m;
                            sim.controls.jib_sheet = j;
                        }
                        if small_button(
                            ui,
                            match *view {
                                View::Cockpit => "VIEW: HELM",
                                View::Chase => "VIEW: ASTERN",
                            },
                        )
                        .clicked()
                        {
                            *view = match *view {
                                View::Cockpit => View::Chase,
                                View::Chase => View::Cockpit,
                            };
                        }
                        let muted = sound.0.muted();
                        if lamp_button(ui, if muted { "SOUND OFF" } else { "SOUND ON" }, !muted)
                            .clicked()
                        {
                            sound.0.set_muted(!muted);
                        }
                        if lamp_button(ui, "DIAGNOSTICS", hud.0).clicked() {
                            hud.0 = !hud.0;
                        }
                    });
                });
            });
        });
    Ok(())
}

fn sail_state(f: &sail_core::sail::SailForces) -> (&'static str, egui::Color32) {
    if f.coeffs.luffing > 0.3 {
        ("LUFFING", paint::LIVE)
    } else if f.coeffs.stalled > 0.4 {
        ("STALLED", paint::NEEDLE)
    } else {
        ("DRAWING", paint::GOOD)
    }
}

fn separator(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(1.0, 132.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0.0, paint::BEZEL);
}

/// The wind dial: the one instrument a sailor looks at more than the water.
fn wind_dial(ui: &mut egui::Ui, awa: f64, twa: f64) {
    let size = egui::vec2(104.0, 104.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter();
    let c = rect.center();
    let r = size.x * 0.46;

    p.circle_filled(c, r + 3.0, paint::BEZEL);
    p.circle_filled(c, r, paint::FACE);

    // Graduations every ten degrees, long ones every thirty.
    for i in 0..36 {
        let a = i as f32 * std::f32::consts::TAU / 36.0;
        let long = i % 3 == 0;
        let inner = r - if long { 10.0 } else { 5.0 };
        let dir = egui::vec2(a.sin(), -a.cos());
        p.line_segment(
            [c + dir * inner, c + dir * r],
            egui::Stroke::new(if long { 1.6 } else { 0.8 }, paint::INK),
        );
    }
    // The close-hauled sector, which is the part of the dial anyone actually uses.
    for side in [-1.0_f32, 1.0] {
        for a in [25.0_f32, 45.0] {
            let rad = side * a.to_radians();
            let dir = egui::vec2(rad.sin(), -rad.cos());
            p.line_segment(
                [c + dir * (r - 16.0), c + dir * r],
                egui::Stroke::new(2.0, paint::GOOD),
            );
        }
    }

    // True wind as a thin ghost behind the apparent, because seeing them apart is
    // how you understand what the boat's own speed is doing to the wind you feel.
    needle(p, c, r * 0.82, twa as f32, 2.0, paint::LABEL);
    needle(p, c, r * 0.90, awa as f32, 3.2, paint::NEEDLE);
    p.circle_filled(c, 4.0, paint::INK);
    p.text(
        c + egui::vec2(0.0, r * 0.52),
        egui::Align2::CENTER_CENTER,
        format!("{:.0}", deg(awa).abs()),
        egui::FontId::monospace(12.0),
        paint::INK,
    );
}

fn needle(p: &egui::Painter, c: egui::Pos2, len: f32, angle: f32, width: f32, col: egui::Color32) {
    let dir = egui::vec2(angle.sin(), -angle.cos());
    p.line_segment(
        [c - dir * 8.0, c + dir * len],
        egui::Stroke::new(width, col),
    );
}

/// A linear gauge: a bar against its scale, which reads faster than a number.
fn gauge(ui: &mut egui::Ui, label: &str, value: f64, lo: f64, hi: f64, unit: &str) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(86.0, 42.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 2.0, paint::TRACK);
    let f = ((value - lo) / (hi - lo)).clamp(0.0, 1.0) as f32;
    let bar = egui::Rect::from_min_size(
        rect.left_bottom() + egui::vec2(3.0, -13.0),
        egui::vec2((rect.width() - 6.0) * f, 10.0),
    );
    p.rect_filled(bar, 1.0, paint::LIVE);
    p.text(
        rect.left_top() + egui::vec2(4.0, 3.0),
        egui::Align2::LEFT_TOP,
        label,
        egui::FontId::monospace(9.0),
        paint::LABEL,
    );
    p.text(
        rect.right_top() + egui::vec2(-4.0, 2.0),
        egui::Align2::RIGHT_TOP,
        format!("{value:.1}{unit}"),
        egui::FontId::monospace(12.0),
        paint::FACE,
    );
}

/// Heel, drawn as a horizon that tilts. One to one, because it has to agree with what
/// your inner ear is already telling you.
fn heel_scale(ui: &mut egui::Ui, heel: f64) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(180.0, 42.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 2.0, paint::TRACK);
    let c = rect.center();
    let a = (heel as f32).clamp(-1.0, 1.0);
    let dir = egui::vec2(a.cos(), a.sin());
    p.line_segment(
        [c - dir * 70.0, c + dir * 70.0],
        egui::Stroke::new(2.0, paint::GOOD),
    );
    for mark in [-30.0_f32, -20.0, -10.0, 0.0, 10.0, 20.0, 30.0] {
        let x = c.x + mark / 30.0 * 70.0;
        let h = if mark == 0.0 { 9.0 } else { 5.0 };
        p.line_segment(
            [
                egui::pos2(x, rect.top() + 3.0),
                egui::pos2(x, rect.top() + 3.0 + h),
            ],
            egui::Stroke::new(1.0, paint::LABEL),
        );
    }
    p.text(
        rect.left_bottom() + egui::vec2(4.0, -3.0),
        egui::Align2::LEFT_BOTTOM,
        "HEEL",
        egui::FontId::monospace(9.0),
        paint::LABEL,
    );
    p.text(
        rect.right_bottom() + egui::vec2(-4.0, -3.0),
        egui::Align2::RIGHT_BOTTOM,
        format!("{:.0}", deg(heel).abs()),
        egui::FontId::monospace(14.0),
        paint::FACE,
    );
}

fn readout(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("{label:<6}"))
                .color(paint::LABEL)
                .monospace()
                .size(11.0),
        );
        ui.label(
            egui::RichText::new(value)
                .color(paint::FACE)
                .monospace()
                .size(13.0),
        );
    });
}

/// The tiller: drag it, and it stays.
fn tiller(ui: &mut egui::Ui, rudder: &mut f64, max: f64) -> bool {
    ui.label(
        egui::RichText::new("HELM")
            .color(paint::LABEL)
            .monospace()
            .size(10.0),
    );
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(188.0, 92.0), egui::Sense::click_and_drag());
    let p = ui.painter();
    p.rect_filled(rect, 3.0, paint::TRACK);

    let changed = if response.dragged() {
        let f = ((response
            .interact_pointer_pos()
            .map(|q| q.x)
            .unwrap_or(rect.center().x)
            - rect.center().x)
            / (rect.width() * 0.45))
            .clamp(-1.0, 1.0);
        *rudder = f as f64 * max;
        true
    } else {
        false
    };

    let c = egui::pos2(rect.center().x, rect.bottom() - 10.0);
    for mark in [-1.0_f32, -0.5, 0.0, 0.5, 1.0] {
        let x = c.x + mark * rect.width() * 0.45;
        p.line_segment(
            [
                egui::pos2(x, rect.top() + 6.0),
                egui::pos2(x, rect.top() + 14.0),
            ],
            egui::Stroke::new(if mark == 0.0 { 2.0 } else { 1.0 }, paint::LABEL),
        );
    }
    // Drawn as a tiller swinging about its pivot, so which way it moves the boat is
    // obvious rather than learned.
    let angle = (*rudder / max).clamp(-1.0, 1.0) as f32;
    let tip = c + egui::vec2(angle * rect.width() * 0.45, -(rect.height() - 26.0));
    p.line_segment([c, tip], egui::Stroke::new(5.0, paint::NEEDLE));
    p.circle_filled(c, 5.0, paint::BEZEL);
    p.text(
        rect.left_top() + egui::vec2(5.0, 3.0),
        egui::Align2::LEFT_TOP,
        format!("{:+.0}", deg(*rudder)),
        egui::FontId::monospace(13.0),
        paint::FACE,
    );
    changed
}

/// A sheet lever: drag up to haul in, down to ease. The wanted position is marked, so
/// the gap between where it is and where it should be is a distance rather than a sum.
fn lever(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    want: Option<f64>,
    state: (&'static str, egui::Color32),
) -> bool {
    ui.vertical(|ui| {
        ui.label(
            egui::RichText::new(label)
                .color(paint::LABEL)
                .monospace()
                .size(10.0),
        );
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(46.0, 108.0), egui::Sense::click_and_drag());
        let p = ui.painter();
        p.rect_filled(rect, 3.0, paint::TRACK);

        let changed = if response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                let f = ((rect.bottom() - 8.0 - pos.y) / (rect.height() - 16.0)).clamp(0.0, 1.0);
                // Up is hauled in, so the lever reads the way the rope moves.
                *value = 1.0 - f as f64;
            }
            true
        } else {
            false
        };

        let y_of = |v: f64| rect.bottom() - 8.0 - (1.0 - v as f32) * (rect.height() - 16.0);
        if let Some(want) = want {
            let y = y_of(want);
            p.line_segment(
                [
                    egui::pos2(rect.left() + 3.0, y),
                    egui::pos2(rect.right() - 3.0, y),
                ],
                egui::Stroke::new(1.5, paint::GOOD),
            );
        }
        let y = y_of(*value);
        let knob = egui::Rect::from_center_size(
            egui::pos2(rect.center().x, y),
            egui::vec2(rect.width() - 8.0, 13.0),
        );
        p.rect_filled(knob, 2.0, paint::BEZEL);
        p.rect_stroke(
            knob,
            2.0,
            egui::Stroke::new(1.0, paint::FACE),
            egui::StrokeKind::Inside,
        );
        ui.label(
            egui::RichText::new(state.0)
                .color(state.1)
                .monospace()
                .size(9.0),
        );
        changed
    })
    .inner
}

/// The mainsheet traveller: a car on an athwartships track, dragged with the mouse.
///
/// Drawn across rather than up and down, because that is the direction the fitting
/// moves and the direction that matters: a car at the port end means something
/// completely different on one tack than the other. The leeward half of the track is
/// marked, so you can see at a glance which way is "down" right now — that is the
/// orientation a sailor has from the heel of the boat and a player does not.
fn traveller_track(ui: &mut egui::Ui, value: &mut f64, lee: f64) -> bool {
    ui.vertical(|ui| {
        ui.label(
            egui::RichText::new("TRAVELLER")
                .color(paint::LABEL)
                .monospace()
                .size(10.0),
        );
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(124.0, 26.0), egui::Sense::click_and_drag());
        let p = ui.painter();
        p.rect_filled(rect, 3.0, paint::TRACK);

        let span = rect.width() - 16.0;
        let x_of = |v: f64| rect.center().x + v as f32 * span * 0.5;

        // Shade the leeward half, so "drop the traveller" has a direction on screen.
        if lee != 0.0 {
            let (a, b) = if lee > 0.0 {
                (rect.center().x, rect.right() - 2.0)
            } else {
                (rect.left() + 2.0, rect.center().x)
            };
            p.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(a, rect.top() + 2.0),
                    egui::pos2(b, rect.bottom() - 2.0),
                ),
                2.0,
                paint::BEZEL,
            );
        }
        // Centreline mark: amidships is a setting you come back to.
        p.line_segment(
            [
                egui::pos2(rect.center().x, rect.top() + 3.0),
                egui::pos2(rect.center().x, rect.bottom() - 3.0),
            ],
            egui::Stroke::new(1.0, paint::LABEL),
        );

        let changed = if response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                *value = (((pos.x - rect.center().x) / (span * 0.5)) as f64).clamp(-1.0, 1.0);
            }
            true
        } else {
            false
        };

        let car = egui::Rect::from_center_size(
            egui::pos2(x_of(*value), rect.center().y),
            egui::vec2(16.0, rect.height() - 8.0),
        );
        p.rect_filled(car, 2.0, paint::FACE);
        p.rect_stroke(
            car,
            2.0,
            egui::Stroke::new(1.0, paint::LIVE),
            egui::StrokeKind::Inside,
        );
        changed
    })
    .inner
}

/// One clutch: click to put it on the winch, drag to work it.
fn clutch(ui: &mut egui::Ui, label: &str, value: &mut f64, selected: bool) -> (bool, bool) {
    ui.vertical(|ui| {
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(58.0, 96.0), egui::Sense::click_and_drag());
        let p = ui.painter();
        p.rect_filled(
            rect,
            3.0,
            if selected { paint::BEZEL } else { paint::TRACK },
        );
        if selected {
            p.rect_stroke(
                rect,
                3.0,
                egui::Stroke::new(1.5, paint::LIVE),
                egui::StrokeKind::Inside,
            );
        }
        let dragged = response.dragged();
        if dragged {
            if let Some(pos) = response.interact_pointer_pos() {
                let f = ((rect.bottom() - 10.0 - pos.y) / (rect.height() - 30.0)).clamp(0.0, 1.0);
                *value = f as f64;
            }
        }
        // Travel drawn as a filled column, because a clutch either has rope through
        // it or it does not and the amount is what matters.
        let h = (rect.height() - 30.0) * *value as f32;
        let bar = egui::Rect::from_min_size(
            egui::pos2(rect.left() + 6.0, rect.bottom() - 10.0 - h),
            egui::vec2(rect.width() - 12.0, h),
        );
        p.rect_filled(bar, 1.0, if selected { paint::LIVE } else { paint::LABEL });
        p.text(
            rect.center_top() + egui::vec2(0.0, 4.0),
            egui::Align2::CENTER_TOP,
            label,
            egui::FontId::monospace(9.0),
            if selected { paint::LIVE } else { paint::LABEL },
        );
        p.text(
            rect.center_bottom() + egui::vec2(0.0, -1.0),
            egui::Align2::CENTER_BOTTOM,
            format!("{:.2}", *value),
            egui::FontId::monospace(10.0),
            paint::FACE,
        );
        (response.clicked(), dragged)
    })
    .inner
}

/// Engine telegraph: ahead, stop, astern, with a detent at stop.
fn throttle_lever(ui: &mut egui::Ui, throttle: &mut f64) -> bool {
    ui.label(
        egui::RichText::new("ENGINE")
            .color(paint::LABEL)
            .monospace()
            .size(10.0),
    );
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(56.0, 108.0), egui::Sense::click_and_drag());
    let p = ui.painter();
    p.rect_filled(rect, 3.0, paint::TRACK);
    let changed = if response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            let f = ((rect.center().y - pos.y) / (rect.height() * 0.42)).clamp(-1.0, 1.0);
            // A detent at stop, because finding neutral by eye on a drag is hopeless
            // and every real telegraph has one.
            *throttle = if f.abs() < 0.12 { 0.0 } else { f as f64 };
        }
        true
    } else {
        false
    };
    let y = rect.center().y - (*throttle as f32) * rect.height() * 0.42;
    p.line_segment(
        [
            egui::pos2(rect.left() + 6.0, rect.center().y),
            egui::pos2(rect.right() - 6.0, rect.center().y),
        ],
        egui::Stroke::new(1.0, paint::LABEL),
    );
    let knob = egui::Rect::from_center_size(
        egui::pos2(rect.center().x, y),
        egui::vec2(rect.width() - 10.0, 14.0),
    );
    p.rect_filled(knob, 2.0, paint::BEZEL);
    p.rect_stroke(
        knob,
        2.0,
        egui::Stroke::new(1.0, paint::FACE),
        egui::StrokeKind::Inside,
    );
    p.text(
        rect.center_bottom() + egui::vec2(0.0, -2.0),
        egui::Align2::CENTER_BOTTOM,
        if *throttle > 0.01 {
            "AHEAD"
        } else if *throttle < -0.01 {
            "ASTERN"
        } else {
            "STOP"
        },
        egui::FontId::monospace(9.0),
        paint::FACE,
    );
    changed
}

fn small_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(text)
                .monospace()
                .size(10.0)
                .color(paint::FACE),
        )
        .fill(paint::TRACK)
        .stroke(egui::Stroke::new(1.0, paint::BEZEL))
        .min_size(egui::vec2(96.0, 20.0)),
    )
}

/// A switch with a lamp in it, which is how you read state on a panel at a glance.
fn lamp_button(ui: &mut egui::Ui, text: &str, lit: bool) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(text)
                .monospace()
                .size(10.0)
                .color(if lit { paint::INK } else { paint::FACE }),
        )
        .fill(if lit { paint::LIVE } else { paint::TRACK })
        .stroke(egui::Stroke::new(1.0, paint::BEZEL))
        .min_size(egui::vec2(96.0, 20.0)),
    )
}
