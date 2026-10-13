//! The settings a person wants to reach without reading the source.
//!
//! Two kinds of thing live here and they are not the same kind, which is why the panel
//! separates them with a line rather than a heading.
//!
//! **Sound** is a preference. There is no right master volume; there is the one that
//! suits the room, and until now the only control over it was a mute key. A number that
//! belongs to the listener rather than to the boat is the one number in the audio path
//! that is nobody's measurement, and it is reasonable to put it where it can be reached.
//!
//! **Weather** is not a preference, and the panel says so out loud. The whole argument
//! of this simulator is that you are handed conditions and have to deal with them — the
//! forecast is drawn before you cast off, the reef goes in because of what it says, and
//! the skill is in the dealing. A dial that turns the wind down is therefore a *practice
//! aid*, not a setting, in the same way a wave pool is not the sea. It is here because
//! learning to gybe in thirty knots by being repeatedly knocked down in thirty knots is
//! not learning, and because wanting to look at a squall from close to is a legitimate
//! thing to want. It is labelled for what it is and it leaves the drawn day alone until
//! it is touched.

use bevy::prelude::*;
use bevy_egui::egui;

use crate::sim::Sim;
use crate::sound::SoundHandle;

#[derive(Resource)]
pub struct Settings {
    pub open: bool,
    /// What the listener asked for, which is remembered across a mute.
    pub volume: f32,
    /// Whether the weather has been touched. Until it is, the panel shows the day as
    /// drawn and says so; after it, it stops pretending the forecast still applies.
    pub weather_touched: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            open: false,
            volume: 0.8,
            weather_touched: false,
        }
    }
}

/// Escape opens it, which is the one key everybody tries first.
pub fn toggle(keys: Res<ButtonInput<KeyCode>>, mut settings: ResMut<Settings>) {
    if keys.just_pressed(KeyCode::Escape) {
        settings.open = !settings.open;
    }
}

pub fn draw(
    mut contexts: bevy_egui::EguiContexts,
    mut settings: ResMut<Settings>,
    mut sim: ResMut<Sim>,
    mut sky: ResMut<crate::sky::Sky>,
    sound: Res<SoundHandle>,
) -> Result {
    if !settings.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let frame = egui::Frame::new()
        .fill(egui::Color32::from_rgb(16, 20, 26))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 82, 96)))
        .inner_margin(egui::Margin::symmetric(14, 12));

    let mut close = false;
    egui::Area::new("settings".into())
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, -20.0))
        .show(ctx, |ui| {
            frame.show(ui, |ui| {
                ui.set_width(300.0);
                heading(ui, "SOUND");
                let mut volume = settings.volume;
                if ui
                    .add(
                        egui::Slider::new(&mut volume, 0.0..=1.0)
                            .show_value(false)
                            .text("volume"),
                    )
                    .changed()
                {
                    settings.volume = volume;
                }
                sound.0.set_master(settings.volume);
                let muted = sound.0.muted();
                if ui
                    .button(if muted { "UNMUTE  (K)" } else { "MUTE  (K)" })
                    .clicked()
                {
                    sound.0.set_muted(!muted);
                }

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);

                heading(ui, "WEATHER");
                ui.label(
                    egui::RichText::new(if settings.weather_touched {
                        "set by hand — this is practice, not the day you were given"
                    } else {
                        "as drawn for this passage"
                    })
                    .color(egui::Color32::from_rgb(150, 150, 130))
                    .monospace()
                    .size(10.0),
                );
                ui.add_space(4.0);

                // The hour is the app's own, so it is always adjustable: the sun moves
                // whatever is driving the wind.
                let mut hour = sky.hour;
                if ui
                    .add(egui::Slider::new(&mut hour, 4.0..=20.0).text("hour"))
                    .changed()
                {
                    *sky = crate::sky::Sky::at(hour, sky.cover);
                    settings.weather_touched = true;
                }

                // And the wind, if whatever is behind the field has dials on it. A real
                // forecast would not, and would say so here rather than pretending.
                let Some(wind) = sim.sim.wind.adjustable() else {
                    ui.label(
                        egui::RichText::new("the wind is coming from a forecast and cannot be set")
                            .color(egui::Color32::from_rgb(150, 150, 130))
                            .monospace()
                            .size(10.0),
                    );
                    footer(ui, &mut close);
                    return;
                };
                let mut knots = sail_core::units::ms_to_kn(wind.synoptic.base_speed) as f32;
                if ui
                    .add(egui::Slider::new(&mut knots, 4.0..=40.0).text("wind, kn"))
                    .changed()
                {
                    wind.synoptic.base_speed = sail_core::units::kn_to_ms(knots as f64);
                    settings.weather_touched = true;
                }
                let mut oktas = (wind.synoptic.cover() * 8.0).round() as u8;
                if ui
                    .add(egui::Slider::new(&mut oktas, 0..=8).text("cloud, oktas"))
                    .changed()
                {
                    wind.synoptic.start_cover(oktas as f64 / 8.0);
                    settings.weather_touched = true;
                }
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("squalls")
                            .color(egui::Color32::from_rgb(150, 150, 130))
                            .monospace()
                            .size(11.0),
                    );
                    for (name, interval) in [
                        ("none", f64::INFINITY),
                        ("odd one", 2_400.0),
                        ("squally", 900.0),
                        ("constant", 180.0),
                    ] {
                        let on = (wind.squall_interval - interval).abs() < 1.0
                            || (wind.squall_interval.is_infinite() && interval.is_infinite());
                        if ui.selectable_label(on, name).clicked() {
                            wind.squall_interval = interval;
                            // So the change is felt rather than waited for.
                            wind.next_squall = wind.next_squall.min(interval * 0.25);
                            settings.weather_touched = true;
                        }
                    }
                });
                footer(ui, &mut close);
            });
        });
    if close {
        settings.open = false;
    }
    Ok(())
}

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .color(egui::Color32::from_rgb(200, 190, 150))
            .monospace()
            .size(12.0),
    );
    ui.add_space(4.0);
}

fn footer(ui: &mut egui::Ui, close: &mut bool) {
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if ui.button("CLOSE  (Esc)").clicked() {
            *close = true;
        }
    });
}
