//! What the keys do, in the one place that can be right.
//!
//! This list existed twice before it existed here: once in my head while writing the
//! input handling, and once in the release notes on the download page — where three of
//! them were wrong, because I wrote them from memory of a game I had been building for a
//! week. The ink drawing was B and I said I; the clutch bank is 1/2 and A/D and I said
//! A/D alone; the headsail changes with X/Z and I did not mention it at all.
//!
//! So the list lives in the program it describes, behind a mark in the corner that is
//! the first thing anybody tries. The release notes now say "press ?" instead of saying
//! it again differently.

use bevy::prelude::*;
use bevy_egui::egui;

#[derive(Resource, Default)]
pub struct Help {
    pub open: bool,
}

/// `?` or `F1`, which between them cover everybody's first guess.
pub fn toggle(keys: Res<ButtonInput<KeyCode>>, mut help: ResMut<Help>) {
    let asked = keys.just_pressed(KeyCode::F1)
        || (keys.just_pressed(KeyCode::Slash)
            && (keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight)));
    if asked {
        help.open = !help.open;
    }
    if help.open && keys.just_pressed(KeyCode::Escape) {
        help.open = false;
    }
}

/// Everything the boat answers to.
///
/// Grouped the way a person needs them rather than the way the code is laid out: what
/// your hands do first, then the sails, then what you are looking at. A list in
/// alphabetical order of key would be easier to write and useless to read.
const KEYS: [(&str, &[(&str, &str)]); 3] = [
    (
        "AT THE HELM",
        &[
            ("← →", "the tiller, and it holds the angle you leave it at"),
            ("Space", "centre the helm"),
            ("Shift", "fine — hold it with anything for a smaller bite"),
            ("P", "autopilot on and off"),
            (
                "O",
                "and whether it steers a compass course or a wind angle",
            ),
            (", .", "move its target ten degrees — one with Shift"),
        ],
    ),
    (
        "THE SAILS",
        &[
            ("W S", "mainsheet: haul in, ease"),
            ("Q E", "genoa winch: ease, grind in"),
            ("1 2", "pick a line out of the clutch bank"),
            ("A D", "and work the one you picked"),
            ("X Z", "change headsail — she furls what is up first"),
            ("T", "a hand on the sheet: once for the sail that leads"),
            (" ", "press again for the other — main first upwind, headsail off it"),
            ("R", "dump: hold it and the sheets run until she stands up"),
            ("G", "back the headsail: it stays there until you say otherwise"),
        ],
    ),
    (
        "LOOKING AT IT",
        &[
            ("C", "view: from the helm, or from astern"),
            ("V", "picture: sharp, 960, 720, 480 pixels"),
            ("B", "the ink drawing"),
            ("M", "the chart, and [ ] to zoom it"),
            ("Tab", "the diagnostic panel"),
            ("H", "hide the console"),
            ("K", "mute"),
            ("L", "ask for a new consignment"),
            ("Esc", "settings — sound, weather, sea"),
        ],
    ),
];

pub fn draw(mut contexts: bevy_egui::EguiContexts, mut help: ResMut<Help>) -> Result {
    let ctx = contexts.ctx_mut()?;

    // The mark in the corner. Small, always there, and the first thing anybody tries
    // when they do not know what a program wants from them.
    egui::Area::new("help mark".into())
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-14.0, -14.0))
        .show(ctx, |ui| {
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(26.0, 26.0), egui::Sense::click());
            let hot = response.hovered() || help.open;
            let p = ui.painter();
            p.circle_filled(
                rect.center(),
                13.0,
                if hot {
                    egui::Color32::from_rgb(196, 112, 24)
                } else {
                    egui::Color32::from_rgb(28, 32, 38)
                },
            );
            p.circle_stroke(
                rect.center(),
                13.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(120, 132, 146)),
            );
            p.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "?",
                egui::FontId::monospace(15.0),
                if hot {
                    egui::Color32::from_rgb(20, 20, 20)
                } else {
                    egui::Color32::from_rgb(210, 214, 220)
                },
            );
            if response.clicked() {
                help.open = !help.open;
            }
        });

    if !help.open {
        return Ok(());
    }

    let frame = egui::Frame::new()
        .fill(egui::Color32::from_rgb(16, 20, 26))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 82, 96)))
        .inner_margin(egui::Margin::symmetric(16, 14));
    let mut close = false;
    egui::Area::new("help".into())
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, -20.0))
        .show(ctx, |ui| {
            frame.show(ui, |ui| {
                ui.set_width(470.0);
                for (heading, rows) in KEYS {
                    ui.label(
                        egui::RichText::new(heading)
                            .color(egui::Color32::from_rgb(200, 190, 150))
                            .monospace()
                            .size(12.0),
                    );
                    ui.add_space(3.0);
                    for (key, what) in rows {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("{key:<7}"))
                                    .color(egui::Color32::from_rgb(230, 198, 120))
                                    .monospace()
                                    .size(12.0),
                            );
                            ui.label(
                                egui::RichText::new(*what)
                                    .color(egui::Color32::from_rgb(186, 192, 198))
                                    .monospace()
                                    .size(11.0),
                            );
                        });
                    }
                    ui.add_space(9.0);
                }
                if ui.button("CLOSE  (? or Esc)").clicked() {
                    close = true;
                }
            });
        });
    if close {
        help.open = false;
    }
    Ok(())
}
