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
use sail_core::state::Headsail;

use crate::clutch::{Bank, BANK};
use crate::course::{clock, Course, Leg};
use crate::input::Pilot;
use crate::sim::Sim;

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
    /// For the things you want seen across a cockpit in a hurry: overdue, aground.
    pub const BAD: Color32 = Color32::from_rgb(224, 104, 84);
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
    mut console: ResMut<Console>,
    mut settings: ResMut<crate::settings::Settings>,
    mut course: ResMut<Course>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    // How long the screen has been up, so it can surface rather than appear.
    mut dying: Local<f32>,
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
        .inner_margin(egui::Margin::symmetric(8, 5));

    // Floating blocks sized to their contents, not panels that reserve a full edge of
    // the screen. A panel took the whole height of the window for a column of
    // readouts a third as tall, and the water is the thing worth looking at.
    egui::Area::new("instruments".into())
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(8.0, 8.0))
        .interactable(true)
        .show(ctx, |ui| {
            frame.show(ui, |ui| {
                ui.set_width(172.0);
                ui.spacing_mut().item_spacing.y = 6.0;

                ui.horizontal(|ui| {
                    wind_dial(ui, telemetry.awa, telemetry.twa);
                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        gauge(ui, "LOG", ms_to_kn(telemetry.stw), 0.0, 10.0, "kn");
                        gauge(ui, "WIND", ms_to_kn(telemetry.aws), 0.0, 30.0, "kn");
                        // The sea as it *is*, not as it was drawn. Not an instrument a
                        // boat carries, but the forecast is, and knowing whether the
                        // slamming is a metre or two is the difference between carrying
                        // on and bearing away. This read `weather.hs` until the sea
                        // started following the wind, at which point it became a number
                        // from the start of the passage with a label claiming otherwise.
                        gauge(ui, "SEA", telemetry.significant_height, 0.0, 4.0, "m");
                        // The sounder. The one instrument that turns a shoal from an
                        // ambush into a decision, which is the whole difference
                        // between land being a hazard and land being a wall.
                        // Scaled to ten metres, not forty: a sounder reading thirty
                        // is a sounder with nothing to say, and compressing the whole
                        // useful range into the bottom quarter of the bar is how a
                        // gauge gets ignored.
                        gauge_alarmed(
                            ui,
                            "DEPTH",
                            telemetry.depth,
                            0.0,
                            10.0,
                            "m",
                            telemetry.under_keel < 1.5,
                        );
                    });
                });

                heel_scale(ui, telemetry.heel);

                ui.add_space(2.0);
                readout(ui, "HDG", &format!("{:03.0}", deg(heading)));
                readout(ui, "TWA", &format!("{:+.0}", deg(telemetry.twa)));
                // The apparent, which is the wind the sails are actually in and the one
                // a trimmer reads. It was in the diagnostic panel and on the windex and
                // nowhere a person glances at — so the one number you set a sheet by was
                // the one number the instruments did not have, and the true wind beside
                // it was quietly doing duty for it.
                readout(
                    ui,
                    "AWA",
                    &format!(
                        "{:+.0}  {:.1} kn",
                        deg(telemetry.awa),
                        ms_to_kn(telemetry.aws)
                    ),
                );
                readout(ui, "VMG", &format!("{:.2} kn", ms_to_kn(telemetry.vmg)));
                // The barometer. A skipper does not sail the wind that is blowing, he
                // sails the one he thinks will be blowing in an hour: whether to reef
                // now while it is easy, whether this shift is worth tacking on. The
                // weather is going somewhere now, so the instrument that says where is
                // worth more than any of the ones above it.
                let (building, turning) = sim.sim.wind.outlook();
                readout(ui, "OUTLOOK", &outlook_words(building, turning));
                // And the one piece of weather that is not a trend but an object. A
                // squall is a thing with a position, a size and a time of arrival, and
                // every one of those is a decision: reef, bear away, or carry on and
                // find out. Shown only when there is one, because an instrument that
                // says NO SQUALL all day is an instrument you stop reading.
                if let Some(warning) = squall_warning(&sim) {
                    ui.label(
                        egui::RichText::new(warning)
                            .color(paint::BAD)
                            .monospace()
                            .size(11.0),
                    );
                }

                ui.add_space(4.0);
                ui.separator();
                let (distance, bearing) = course.to_target(sim.sim.state.pos);
                let overdue = course.remaining() < 0.0;
                ui.label(
                    egui::RichText::new(match course.leg {
                        Leg::Carrying => "BOUND FOR",
                        Leg::Idle => "NO CONSIGNMENT",
                    })
                    .color(paint::LABEL)
                    .monospace()
                    .size(9.0),
                );
                ui.label(
                    egui::RichText::new(course.destination_name().to_uppercase())
                        .color(if overdue { paint::BAD } else { paint::LIVE })
                        .monospace()
                        .size(13.0),
                );
                readout(ui, "RANGE", &format!("{:.2} nm", distance / 1852.0));
                readout(ui, "BRG", &format!("{:03.0}", deg(bearing)));
                readout(
                    ui,
                    if overdue { "OVERDUE" } else { "LEFT" },
                    &clock(course.remaining().abs()),
                );
                readout(ui, "DONE", &format!("{}", course.delivered));
            });
        });

    // --- Controls along the bottom -------------------------------------------
    egui::Area::new("console".into())
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -8.0))
        .interactable(true)
        .show(ctx, |ui| {
            frame.show(ui, |ui| {
                // Fixed, so the bar cannot move.
                //
                // It is anchored on its middle, so anything that changes its width slides
                // the whole console sideways — measured, it breathed between 1144 and
                // 1176 points as the clutch hint and the pilot's reason changed. On its
                // own that is a twitch you would blame on your eyes.
                //
                // What made it a fault is egui's multipass: about once in a thousand
                // frames a widget asks for a second layout pass, the console is drawn
                // twice, and with different content the two passes had different widths —
                // so the centre anchor put them in different places and both ended up on
                // the screen. Reported from the deck as stacked consoles, and that
                // rarity is exactly why it could be seen and not found.
                //
                // A fixed width makes the second pass land on top of the first, which is
                // what a discarded pass should look like.
                ui.set_min_width(CONSOLE_WIDTH);
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
                        // What the blade has left, and why. A rudder on the centreline
                        // is a metre under the transom and the water is not glass, so
                        // none of the three things that take its grip away can be seen
                        // from on deck. On a real boat you read them through the tiller;
                        // here there is no tiller in your hand, so they are printed.
                        readout(ui, "BITE", &bite_words(&telemetry));
                        ui.horizontal(|ui| {
                            if small_button(ui, "AMIDSHIPS").clicked() {
                                sim.controls.rudder = 0.0;
                                pilot.disengage();
                            }
                        });
                    });

                    ui.add_space(8.0);
                    // The compass next to the helm, because the two are one instrument:
                    // the card tells you where she is going and the tiller is how you
                    // argue with it.
                    compass_repeater(
                        ui,
                        heading,
                        match course.leg {
                            Leg::Carrying => Some(course.to_target(sim.sim.state.pos).1),
                            Leg::Idle => None,
                        },
                    );

                    ui.add_space(7.0);
                    separator(ui);
                    ui.add_space(7.0);

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
                    // The two sheets. Trimming for you has no switch here and is the
                    // one thing on the boat that does not: it stands for a crew doing
                    // it, which is a different kind of act from putting your own hand
                    // on a winch, and a button for it sat on the console inviting
                    // itself to be pressed instead of the levers beside it. `T`.
                    ui.horizontal_top(|ui| {
                        if lever(
                            ui,
                            "MAIN",
                            &mut main_sheet,
                            Some(want_main),
                            sail_state(&telemetry.main, 1.0 - sim.controls.reef * cfg.max_reef),
                        ) {
                            sim.controls.main_sheet = main_sheet;
                        }
                        if lever(
                            ui,
                            "GENOA",
                            &mut jib_sheet,
                            Some(want_jib),
                            sail_state(&telemetry.jib, 1.0 - sim.controls.furl),
                        ) {
                            sim.controls.jib_sheet = jib_sheet;
                        }
                    });

                    ui.add_space(7.0);
                    separator(ui);
                    ui.add_space(7.0);

                    // The clutch bank: one winch, one line at a time.
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("CLUTCHES")
                                .color(paint::LABEL)
                                .monospace()
                                .size(9.0),
                        );
                        ui.horizontal(|ui| {
                            for (i, line) in BANK.iter().enumerate() {
                                let mut value = line.get(&sim.controls);
                                let selected = i == bank.selected;
                                // A line that leads nowhere on the sail that is up gets
                                // its name bracketed rather than hidden: the clutch is
                                // still on the coachroof, it just has nothing on the
                                // other end of it at the moment.
                                let rigged = line.rigged_for(&sim.controls);
                                let name = if rigged {
                                    line.name_now(&sim.controls).to_string()
                                } else {
                                    format!("({})", line.name_now(&sim.controls))
                                };
                                let (clicked, dragged) = clutch(ui, &name, &mut value, selected);
                                if clicked {
                                    bank.selected = i;
                                    bank.since_change = 0.0;
                                }
                                if dragged {
                                    bank.selected = i;
                                    // A detented line cannot be dragged to anywhere
                                    // between its stops; the hand on the lever picks
                                    // the nearest one and the line runs there.
                                    if let Some(stops) = line.detents(&sim.controls) {
                                        let nearest = stops
                                            .iter()
                                            .copied()
                                            .min_by(|a, b| {
                                                (a - value)
                                                    .abs()
                                                    .partial_cmp(&(b - value).abs())
                                                    .unwrap()
                                            })
                                            .unwrap_or(0.0);
                                        bank.target[i] = Some(nearest);
                                    } else {
                                        line.set_from_ui(&mut sim.controls, value);
                                    }
                                }
                            }
                        });
                        ui.label(
                            egui::RichText::new(fit(
                                &if bank.line().rigged_for(&sim.controls) {
                                    format!("hauling in {}", bank.line().hauled_now(&sim.controls))
                                } else {
                                    format!("{} is not led to this sail", bank.line().name())
                                },
                                44,
                            ))
                            .color(paint::LABEL)
                            .monospace()
                            .size(9.0),
                        );
                    });

                    ui.add_space(7.0);
                    separator(ui);
                    ui.add_space(7.0);

                    // The sail wardrobe, and the pilot.
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("HEADSAIL")
                                .color(paint::LABEL)
                                .monospace()
                                .size(9.0),
                        );
                        ui.add_space(4.0);
                        // The preventer, which is a thing that is either on or off and
                        // therefore a light rather than a slider. Beside the sail
                        // buttons because it belongs to the same decision: what you are
                        // carrying and how you have made it safe.
                        let rigged = sim.controls.preventer;
                        if ui
                            .selectable_label(
                                rigged,
                                egui::RichText::new("PREVENTER (F)")
                                    .monospace()
                                    .size(10.0)
                                    .color(if rigged {
                                        egui::Color32::from_rgb(20, 20, 20)
                                    } else {
                                        egui::Color32::from_rgb(150, 150, 130)
                                    }),
                            )
                            .on_hover_text("stops the boom crossing — rig it before a run")
                            .clicked()
                        {
                            sim.controls.preventer = !rigged;
                        }
                        ui.add_space(3.0);
                        let rolled = sim.controls.furl > 0.9;
                        for sail in [
                            Headsail::Genoa,
                            Headsail::Code0,
                            Headsail::Gennaker,
                            Headsail::Spinnaker,
                        ] {
                            let up = sim.controls.headsail == sail;
                            let label = egui::RichText::new(sail.name().to_uppercase())
                                .monospace()
                                .size(10.0)
                                .color(if up {
                                    egui::Color32::BLACK
                                } else if rolled {
                                    paint::LABEL
                                } else {
                                    egui::Color32::from_gray(90)
                                });
                            let button = egui::Button::new(label).fill(if up {
                                paint::LIVE
                            } else {
                                egui::Color32::from_gray(26)
                            });
                            // Only with the sail rolled away: you furl, change, and set.
                            if ui.add_enabled(rolled, button).clicked() {
                                sim.controls.headsail = sail;
                            }
                        }
                        if !rolled {
                            ui.label(
                                egui::RichText::new("roll it away first")
                                    .monospace()
                                    .size(10.0)
                                    .color(egui::Color32::from_gray(90)),
                            );
                        }
                    });

                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("PILOT")
                                .color(paint::LABEL)
                                .monospace()
                                .size(9.0),
                        );
                        // Why it last let go, for half a minute after it did. The
                        // diagnostic panel has carried this since it was written, which
                        // is the wrong place: the question "why did it switch off?" is
                        // asked by somebody who was not watching, and they are looking
                        // at the console because that is what is on the screen.
                        if let Some(reason) = pilot.last_reason {
                            if pilot.since_reason < 30.0 && !pilot.engaged() {
                                ui.label(
                                    egui::RichText::new(fit(reason, 30))
                                        .color(paint::BAD)
                                        .monospace()
                                        .size(10.0),
                                );
                            }
                        }
                        // The vane cannot drop out, so when it is beaten it has to say so
                        // some other way: there is no alarm to sound and no relay to
                        // click. It says it here, and it says it on the transom, where
                        // the blade is lying on its stop.
                        if pilot.engaged() && pilot.struggling() && !pilot.can_let_go() {
                            ui.label(
                                egui::RichText::new("the vane is not coping")
                                    .color(paint::BAD)
                                    .monospace()
                                    .size(10.0),
                            );
                        }
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
                        // The same two steps the keys use, from the same constants, and
                        // the doubled arrow is the coarse one in every piece of marine
                        // electronics ever built.
                        ui.horizontal(|ui| {
                            for (label, delta) in [
                                ("<<", -crate::input::PILOT_STEP),
                                ("<", -crate::input::PILOT_FINE),
                                (">", crate::input::PILOT_FINE),
                                (">>", crate::input::PILOT_STEP),
                            ] {
                                if nudge_button(ui, label).clicked() {
                                    pilot.adjust(delta);
                                }
                            }
                        });
                    });

                    ui.add_space(7.0);
                    separator(ui);
                    ui.add_space(7.0);

                    // What is left when everything that is a *setting* has gone to the
                    // settings panel.
                    //
                    // The console's rule is that everything the keyboard can do the
                    // mouse can do, and it was being read as though it also said
                    // everything the mouse can do must be on the bar. It does not. The
                    // sound switch was the mute key and the mute button in the menu and
                    // a third copy here; the view and the diagnostic panel are a
                    // preference and a microscope. Three switches came off the bar, and
                    // in exchange the ink drawing and the resolution — which were keys
                    // and nothing else, so the rule was being broken in the direction
                    // that matters — can now be reached with a mouse at all.
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("SHIP")
                                .color(paint::LABEL)
                                .monospace()
                                .size(9.0),
                        );
                        if small_button(ui, "SETTINGS  ESC").clicked() {
                            settings.open = !settings.open;
                        }
                        // The third of the keyboard-only three, and the one with
                        // consequences, so it says what it will cost rather than
                        // sitting there as a word.
                        if small_button(
                            ui,
                            match course.leg {
                                Leg::Idle => "TAKE A JOB  L",
                                Leg::Carrying => "GIVE UP, NEW JOB",
                            },
                        )
                        .clicked()
                        {
                            course.wants_new = true;
                        }
                    });
                });
            });
        });
    // --- And the one thing that stops the passage ---------------------------------
    if telemetry.mishap.is_none() {
        *dying = 0.0;
    }
    if let Some(mishap) = telemetry.mishap {
        // Drawn the way the game that made this screen famous draws it, because there is
        // no point pretending otherwise and because the way it is done is *right*: the
        // letters do not arrive, they surface. A black veil comes up over the whole
        // picture, the words bleed in out of it in deep red with the letters held far
        // apart, and nothing is in a hurry. What that buys is a beat of silence in which
        // you work out what you did — which is also what the sea gives you.
        //
        // Two seconds to come up and nothing to click for the first of them, so the
        // instinctive mash of a key does not skip the only moment of reflection the
        // game has.
        *dying += time.delta_secs();
        let age = *dying;
        let veil = crate::curve::smoothstep(0.0, 1.6, age);
        let words = crate::curve::smoothstep(0.35, 2.2, age);
        // `viewport_rect` in egui 0.36 — what used to be `screen_rect`, and the whole of
        // the window rather than what is left after the panels.
        let screen = ctx.viewport_rect();
        let paint_on = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("mishap"),
        ));
        paint_on.rect_filled(
            screen,
            0.0,
            egui::Color32::from_black_alpha((215.0 * veil) as u8),
        );
        // Letterspaced by hand, because egui has no tracking: a space between every
        // letter is what the original does and it is most of the look.
        let spaced: String = "YOU DIED"
            .chars()
            .flat_map(|c| [c, ' '])
            .collect::<String>()
            .trim_end()
            .to_string();
        let red = egui::Color32::from_rgba_unmultiplied(170, 18, 18, (255.0 * words) as u8);
        paint_on.text(
            screen.center() - egui::vec2(0.0, 30.0),
            egui::Align2::CENTER_CENTER,
            &spaced,
            egui::FontId::proportional(76.0),
            red,
        );
        let faint = egui::Color32::from_rgba_unmultiplied(150, 140, 130, (190.0 * words) as u8);
        paint_on.text(
            screen.center() + egui::vec2(0.0, 36.0),
            egui::Align2::CENTER_CENTER,
            mishap.what_happened(),
            egui::FontId::monospace(15.0),
            faint,
        );
        paint_on.text(
            screen.center() + egui::vec2(0.0, 60.0),
            egui::Align2::CENTER_CENTER,
            mishap.the_lesson(),
            egui::FontId::monospace(11.0),
            egui::Color32::from_rgba_unmultiplied(120, 112, 104, (170.0 * words) as u8),
        );
        if age > 2.2 {
            egui::Area::new("another boat".into())
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 120.0))
                .show(ctx, |ui| {
                    if small_button(ui, "ANOTHER BOAT  (L)").clicked() {
                        course.wants_new = true;
                    }
                });
        }
    }
    Ok(())
}

/// How wide the console is, in points, whatever is written in it.
const CONSOLE_WIDTH: f32 = 1180.0;

/// A line of monospace text of a fixed width, whatever it says.
///
/// Because the console is anchored on its *middle*, so anything that changes its width
/// moves the whole bar sideways — and two of the lines in it are sentences that change:
/// the clutch bank's hint and the pilot's reason for letting go. Measured, the bar
/// breathed between 1144 and 1176 points as those two changed.
///
/// On its own that is a twitch. What made it a fault was egui's multipass: a frame in
/// which a widget asks for a second layout pass draws the console twice, and because the
/// two passes had different *content* they had different widths, so the centre anchor
/// put them in different places and both ended up on the screen. Reported from the deck
/// as stacked consoles, and it happened about once in a thousand frames, which is why it
/// could be seen and not found.
///
/// A monospace line of n characters is n characters wide. Padding and truncating is the
/// whole fix: the content cannot change the layout, so the two passes land on top of
/// each other and the second is invisible — which is what a discarded pass should be.
fn fit(text: &str, columns: usize) -> String {
    let mut out: String = text.chars().take(columns).collect();
    while out.chars().count() < columns {
        out.push(' ');
    }
    out
}

/// What a sail is doing, in one word.
///
/// `set` is how much of it is still up. A rolled-away headsail reported DRAWING
/// because the coefficients are computed from the angle of attack whatever the area
/// is — true of the cloth, and a lie about the sail, since there was none of it left.
/// An instrument that is right about a quantity nobody asked for and wrong about the
/// one on the label is worse than no instrument.
fn sail_state(f: &sail_core::sail::SailForces, set: f64) -> (&'static str, egui::Color32) {
    if set < 0.02 {
        return ("FURLED", paint::LABEL);
    }
    if f.coeffs.luffing > 0.3 {
        ("LUFFING", paint::LIVE)
    } else if f.coeffs.stalled > 0.4 {
        ("STALLED", paint::NEEDLE)
    } else {
        ("DRAWING", paint::GOOD)
    }
}

/// The rule between two columns of the console.
///
/// Its height is a fixed number and that made it the floor under the whole bar: a
/// hundred and thirty-two pixels of hairline held the console open even after every
/// widget in it had been brought down. Eighty-four is the tallest column plus its label,
/// which is what a rule between columns is for.
fn separator(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(1.0, 84.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0.0, paint::BEZEL);
}

/// The wind dial: the one instrument a sailor looks at more than the water.
fn wind_dial(ui: &mut egui::Ui, awa: f64, twa: f64) {
    let size = egui::vec2(90.0, 90.0);
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
/// A gauge that can sound an alarm.
///
/// `alarm` turns the bar and the reading red. One gauge on this panel needs it and
/// only one: a sounder that merely *displays* the depth is a gauge you have to
/// remember to look at, and an island coming over the horizon while you are trimming
/// is precisely the moment nobody is looking. Every boat's sounder has an alarm, and
/// this is why.
fn gauge_alarmed(
    ui: &mut egui::Ui,
    label: &str,
    value: f64,
    lo: f64,
    hi: f64,
    unit: &str,
    alarm: bool,
) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(80.0, 34.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 2.0, paint::TRACK);
    let f = ((value - lo) / (hi - lo)).clamp(0.0, 1.0) as f32;
    let bar = egui::Rect::from_min_size(
        rect.left_bottom() + egui::vec2(3.0, -13.0),
        egui::vec2((rect.width() - 6.0) * f.max(0.02), 10.0),
    );
    let colour = if alarm { paint::BAD } else { paint::LIVE };
    p.rect_filled(bar, 1.0, colour);
    if alarm {
        p.rect_stroke(
            rect,
            2.0,
            egui::Stroke::new(1.5, paint::BAD),
            egui::StrokeKind::Inside,
        );
    }
    p.text(
        rect.left_top() + egui::vec2(4.0, 3.0),
        egui::Align2::LEFT_TOP,
        label,
        egui::FontId::monospace(9.0),
        if alarm { paint::BAD } else { paint::LABEL },
    );
    p.text(
        rect.right_top() + egui::vec2(-4.0, 2.0),
        egui::Align2::RIGHT_TOP,
        format!("{value:.1}{unit}"),
        egui::FontId::monospace(12.0),
        if alarm { paint::BAD } else { paint::FACE },
    );
}

fn gauge(ui: &mut egui::Ui, label: &str, value: f64, lo: f64, hi: f64, unit: &str) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(80.0, 34.0), egui::Sense::hover());
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
    let (rect, _) = ui.allocate_exact_size(egui::vec2(164.0, 34.0), egui::Sense::hover());
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
                .size(9.0),
        );
        ui.label(
            egui::RichText::new(value)
                .color(paint::FACE)
                .monospace()
                .size(12.0),
        );
    });
}

/// The tiller: drag it, and it stays.
/// How much helm there is, and what is taking it away.
///
/// The number first, because it is the one you act on, and then the reason — a helm
/// that has gone soft because there is no way on wants the opposite response to one
/// that has gone soft because the blade is stalled. Only the dominant cause is named:
/// three reasons at once is a sentence, and an instrument is not read, it is glanced at.
fn bite_words(t: &sail_core::state::Telemetry) -> String {
    let bite = t.rudder_bite;
    if bite > 0.75 {
        return format!("{:.0}%", bite * 100.0);
    }
    // Which of the three has taken the most, measured as what each one costs on its
    // own. Speed is the square law, heel is the cosine plus the stern coming out, and
    // stall is the flow letting go of the blade.
    let slow = 1.0 - (ms_to_kn(t.stw) / 5.0).clamp(0.0, 1.0).powi(2);
    let heeled = 1.0 - t.heel.abs().cos();
    let stalled = t.rudder_stalled;
    let worst = if stalled > slow.max(heeled) {
        "stalled"
    } else if heeled > slow {
        "heeled"
    } else {
        "no way on"
    };
    format!("{:.0}% {worst}", bite * 100.0)
}

/// The nearest squall, if one is near enough to matter.
///
/// Range and bearing, and whether it is going to hit — which is the question, and it is
/// not the same as whether it is close. A squall passing two miles up to windward is a
/// spectacle; the same squall dead upwind is twenty minutes away and means reefing now
/// while the deck is still flat. The test is the same one a radar does: project it along
/// its own drift and see how near it comes.
fn squall_warning(sim: &Sim) -> Option<String> {
    let boat = sim.sim.state.pos;
    let drift = sim.sim.wind.sample(boat, sim.sim.state.time);
    let (at, radius, _) = sim.sim.wind.squalls().into_iter().min_by(|a, b| {
        (a.0 - boat)
            .length_squared()
            .total_cmp(&(b.0 - boat).length_squared())
    })?;
    let offset = at - boat;
    let range = offset.length();
    if range > 9_000.0 {
        return None;
    }
    // Closest approach, if it keeps coming the way it is coming. The squall travels a
    // quarter faster than the surface wind — see `wind.rs` — so that is the track used.
    let track = drift * 1.25;
    let speed = track.length().max(0.1);
    let along = offset.dot(track) / speed;
    // Behind us and going away: nothing to say.
    if along > 0.0 {
        return None;
    }
    let miss = (offset - track / speed * along).length();
    let minutes = (-along / speed) / 60.0;
    let bearing = sail_core::units::deg(sail_core::wind::direction_of(-offset)).rem_euclid(360.0);
    Some(if miss < radius {
        format!(
            "SQUALL {:.1} nm {:03.0} — {:.0} min",
            range / 1852.0,
            bearing,
            minutes
        )
    } else {
        format!(
            "squall {:.1} nm {:03.0} — passing {:.1} nm off",
            range / 1852.0,
            bearing,
            miss / 1852.0
        )
    })
}

/// The hour ahead, in the words a forecast uses.
///
/// Knots per hour and degrees per hour are not how anyone thinks about weather, and a
/// console full of signed numbers is a console nobody reads. "Easing, backing" is what
/// a watch leader says, and it is what this says, with the figures only when they are
/// big enough to act on.
fn outlook_words(building: f64, turning: f64) -> String {
    let knots = ms_to_kn(building);
    let degrees = deg(turning);
    let wind = match knots {
        k if k > 3.0 => format!("building {:.0}", k),
        k if k > 1.0 => "building".to_string(),
        k if k < -3.0 => format!("easing {:.0}", -k),
        k if k < -1.0 => "easing".to_string(),
        _ => "steady".to_string(),
    };
    // Veering is clockwise and backing is anticlockwise. Every sailor knows the words
    // and nobody has ever been sure which is which, which is a good reason to print the
    // figure next to them.
    let shift = match degrees {
        d if d > 5.0 => format!(", veer {:.0}", d),
        d if d < -5.0 => format!(", back {:.0}", -d),
        _ => String::new(),
    };
    format!("{wind}{shift}")
}

/// A compass repeater: the same card as the one in the bowl, on the console.
///
/// The card turns and the lubber line does not, which is the one thing that makes a
/// compass a compass and the thing a digital heading readout cannot teach. There is a
/// heading in figures three centimetres away on this same panel; this is here because
/// steering to a number and steering to a card are different skills, and the second one
/// is the one that works when she is being thrown about.
fn compass_repeater(ui: &mut egui::Ui, heading: f64, mark: Option<f64>) {
    ui.label(
        egui::RichText::new("COMPASS")
            .color(paint::LABEL)
            .monospace()
            .size(9.0),
    );
    let (rect, _) = ui.allocate_exact_size(egui::vec2(74.0, 74.0), egui::Sense::hover());
    let p = ui.painter();
    let c = rect.center();
    let r = rect.width() * 0.42;
    p.circle_filled(c, r + 4.0, paint::CASE);
    p.circle_filled(c, r, paint::FACE);

    let at = |bearing: f32, radius: f32| -> egui::Pos2 {
        // Card bearings run clockwise from north, and the card is turned by the heading
        // so that north stays north while the boat swings under it.
        let a = bearing - heading as f32;
        egui::pos2(c.x + radius * a.sin(), c.y - radius * a.cos())
    };
    for step in 0..36 {
        let b = std::f32::consts::TAU * step as f32 / 36.0;
        let long = step % 3 == 0;
        p.line_segment(
            [at(b, r * if long { 0.74 } else { 0.84 }), at(b, r * 0.96)],
            egui::Stroke::new(if long { 1.6 } else { 1.0 }, paint::INK),
        );
    }
    // North, as the arrowhead it is on the card.
    let north = at(0.0, r * 0.80);
    p.add(egui::Shape::convex_polygon(
        vec![north, at(-0.10, r * 0.34), at(0.10, r * 0.34)],
        paint::NEEDLE,
        egui::Stroke::NONE,
    ));
    for (cardinal, _name) in [
        (std::f32::consts::FRAC_PI_2, "E"),
        (std::f32::consts::PI, "S"),
        (3.0 * std::f32::consts::FRAC_PI_2, "W"),
    ] {
        p.add(egui::Shape::convex_polygon(
            vec![
                at(cardinal, r * 0.78),
                at(cardinal - 0.08, r * 0.42),
                at(cardinal + 0.08, r * 0.42),
            ],
            paint::INK,
            egui::Stroke::NONE,
        ));
    }
    // The lubber line, which belongs to the boat and therefore does not move.
    p.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(c.x, c.y - r - 1.0),
            egui::pos2(c.x - 4.0, c.y - r - 9.0),
            egui::pos2(c.x + 4.0, c.y - r - 9.0),
        ],
        paint::NEEDLE,
        egui::Stroke::NONE,
    ));
    // Where we are bound, on the rim. A repeater with a course pointer answers the
    // question you actually have at the helm — am I pointing at it — without arithmetic
    // between two three-digit numbers.
    if let Some(bearing) = mark {
        let b = bearing as f32;
        p.add(egui::Shape::convex_polygon(
            vec![
                at(b, r * 0.98),
                at(b - 0.09, r * 1.14),
                at(b + 0.09, r * 1.14),
            ],
            paint::LIVE,
            egui::Stroke::NONE,
        ));
    }
    p.text(
        egui::pos2(c.x, c.y + r * 0.52),
        egui::Align2::CENTER_CENTER,
        format!("{:03.0}", sail_core::units::deg(heading).rem_euclid(360.0)),
        egui::FontId::monospace(12.0),
        paint::INK,
    );
}

fn tiller(ui: &mut egui::Ui, rudder: &mut f64, max: f64) -> bool {
    ui.label(
        egui::RichText::new("HELM")
            .color(paint::LABEL)
            .monospace()
            .size(9.0),
    );
    // A strip rather than a box. A tiller has one number in it — where it is — and the
    // old panel gave that one number a hundred and eighty by ninety pixels of console,
    // which is more room than the compass gets and more than the thing deserves.
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(138.0, 30.0), egui::Sense::click_and_drag());
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

    let c = egui::pos2(rect.center().x, rect.bottom() - 6.0);
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
                .size(9.0),
        );
        // Forty-two by seventy-four, down from forty-six by a hundred and eight. It was
        // the tallest thing in the console by a wide margin and therefore the thing
        // setting the height of the whole bar, while the clutches, the headsail, the
        // pilot and the ship columns all sat in sixty-odd with air above them.
        //
        // Still *vertical*, which is the part worth keeping: up is hauled in, so the
        // lever reads the way the rope moves. Turning these on their side would have
        // saved more and thrown that away.
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(42.0, 74.0), egui::Sense::click_and_drag());
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

/// One clutch: click to put it on the winch, drag to work it.
fn clutch(ui: &mut egui::Ui, label: &str, value: &mut f64, selected: bool) -> (bool, bool) {
    ui.vertical(|ui| {
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(50.0, 70.0), egui::Sense::click_and_drag());
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

/// A button with nothing in it but an arrow, sized to the arrow.
///
/// `small_button` carries a 96 pixel floor so that a column of labelled switches lines
/// up, which is right for a switch and badly wrong for four chevrons in a row: it put
/// three hundred and eighty pixels of pilot across a console and opened a gap you could
/// park the chart in.
fn nudge_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(text)
                .monospace()
                .size(10.0)
                .color(paint::FACE),
        )
        .fill(paint::TRACK)
        .stroke(egui::Stroke::new(1.0, paint::BEZEL))
        .min_size(egui::vec2(22.0, 20.0)),
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
