//! What swims here, and what you have caught of it.
//!
//! Two things that want to be one page, because on a boat they are one page: the back of
//! a skipper's notebook has the fish in it *and* the record of them, and nobody keeps
//! those separately.
//!
//! **The left half is knowledge and is there from the first minute.** Where each one
//! feeds, how fast you have to tow a lure for it to be interested, and what it does when
//! it feels the hook — which is not a reward for playing, it is what anybody who has
//! sailed these waters already knows and would be insulted to have to unlock. The game
//! does not hide the fact that cod are deep and slow.
//!
//! **The right half is yours and starts empty.** How many, the best one, and what it
//! weighed. That is the part that fills.
//!
//! The reason it exists at all is that the trolling has four inputs — depth, speed,
//! ground and the glass — and three of them are invisible unless somebody says so. A
//! player who never finds this page will catch mackerel for an hour and conclude there
//! are only mackerel.
//!
//! Behind `I`, which was the last letter on the keyboard worth having and is the first
//! one anybody presses looking for information.

use bevy::prelude::*;
use bevy_egui::egui;

use crate::fishing::{Fishing, Quarry, SPECIES};

#[derive(Resource, Default)]
pub struct Logbook {
    pub open: bool,
}

pub fn toggle(keys: Res<ButtonInput<KeyCode>>, mut book: ResMut<Logbook>) {
    if keys.just_pressed(KeyCode::KeyI) {
        book.open = !book.open;
    }
}

/// What to tow past it, in words a person can act on.
///
/// Worked out from the same table the fish are drawn from rather than written beside it,
/// so a species whose numbers are tuned cannot end up with a page that lies about them.
/// The depth is given as the line to let out at that speed, because that is the control
/// the player actually has — nobody can set a depth, they can only count line.
fn advice(what: Quarry, stw: f64) -> (String, String) {
    let (depth, speed, ground) = what.hunting();
    // The line that puts the lure at its depth, at its own speed. Found by walking out
    // rather than inverted, because `lure_depth` is a logarithm and this is six
    // comparisons once a frame on a page nobody holds open.
    let want = sail_core::units::kn_to_ms(speed);
    let mut line = 4.0;
    while line < 140.0 && crate::fishing::lure_depth(line, want) < depth {
        line += 2.0;
    }
    let now = crate::fishing::lure_depth(line, stw.max(0.1));
    (
        format!("{speed:.1} kn, {line:.0} m of line"),
        format!("{depth:.0} m over {ground:.0} m   (now {now:.0} m)"),
    )
}

/// How long the screen stands there before it gets out of the way, seconds.
///
/// Eight. Long enough to read twice and not long enough to be in the way of a boat that
/// is still sailing — which she is, and that is the point: nothing has stopped, the rod
/// is simply not there any more. A modal box with a button on it would be a *game* saying
/// you failed. This is a boat telling you what just happened over your shoulder.
const SHOWN_FOR: f64 = 8.0;

/// The one that is not a fish getting away.
///
/// Everything else in the fishing costs you a fish and leaves you fishing. This costs you
/// the rod, and there is another one at the next harbour and nowhere nearer, so it is the
/// only thing here that ends the afternoon.
///
/// It says what happened, why, and what would have prevented it — in that order, because
/// that is the order somebody wants it in and because the *why* is the whole lesson: the
/// holder gives when the drag does not, so a rod goes over the side for exactly one
/// reason, and it is a reason you chose a minute earlier.
pub fn lost(mut contexts: bevy_egui::EguiContexts, fishing: Res<Fishing>) -> Result {
    if fishing.have_a_rod() || fishing.gone_for > SHOWN_FOR {
        return Ok(());
    }
    use crate::panel::paint;
    let ctx = contexts.ctx_mut()?;
    // Fading out at the end rather than vanishing, so it reads as the moment passing
    // instead of a dialog being dismissed by something you did not do.
    let fade = ((SHOWN_FOR - fishing.gone_for) / 1.5).clamp(0.0, 1.0) as f32;

    egui::Area::new("rod gone".into())
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, -40.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(egui::Color32::from_rgb(22, 12, 12).gamma_multiply(fade))
                .stroke(egui::Stroke::new(
                    1.0,
                    paint::BAD.gamma_multiply(fade * 0.8),
                ))
                .inner_margin(egui::Margin::symmetric(22, 16))
                .show(ui, |ui| {
                    ui.set_width(392.0);
                    let say = |ui: &mut egui::Ui, text: &str, colour: egui::Color32, size| {
                        ui.label(
                            egui::RichText::new(text)
                                .color(colour.gamma_multiply(fade))
                                .monospace()
                                .size(size),
                        );
                    };
                    say(ui, "THE ROD HAS GONE OVER THE SIDE", paint::BAD, 16.0);
                    ui.add_space(6.0);
                    say(
                        ui,
                        "The holder gives before the line does. It takes the bending",
                        paint::FACE,
                        11.0,
                    );
                    say(
                        ui,
                        "moment of two metres of lever, and a drag that will not slip",
                        paint::FACE,
                        11.0,
                    );
                    say(ui, "puts it there and holds it there.", paint::FACE, 11.0);
                    ui.add_space(6.0);
                    say(
                        ui,
                        "Back the drag off, or keep a hand on it — U is a hand on it.",
                        paint::LIVE,
                        11.0,
                    );
                    ui.add_space(6.0);
                    say(
                        ui,
                        "There is another in the chandlery at the next harbour.",
                        paint::LABEL,
                        11.0,
                    );
                });
        });
    Ok(())
}

pub fn draw(
    mut contexts: bevy_egui::EguiContexts,
    book: Res<Logbook>,
    fishing: Res<Fishing>,
    sim: Res<crate::sim::Sim>,
) -> Result {
    if !book.open {
        return Ok(());
    }
    use crate::panel::paint;
    let ctx = contexts.ctx_mut()?;
    let stw = sim.sim.telemetry.stw;

    let frame = egui::Frame::new()
        .fill(egui::Color32::from_rgb(16, 20, 26))
        .stroke(egui::Stroke::new(1.0, paint::BEZEL))
        .inner_margin(egui::Margin::symmetric(16, 14));

    egui::Area::new("logbook".into())
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, -10.0))
        .show(ctx, |ui| {
            frame.show(ui, |ui| {
                ui.set_width(468.0);
                let line = |ui: &mut egui::Ui, text: String, colour, size| {
                    ui.label(
                        egui::RichText::new(text)
                            .color(colour)
                            .monospace()
                            .size(size),
                    );
                };
                line(ui, "WHAT SWIMS HERE".into(), paint::LABEL, 12.0);
                if !fishing.have_a_rod() {
                    line(
                        ui,
                        "— and nothing to catch it with until the next harbour".into(),
                        paint::BAD,
                        11.0,
                    );
                }
                ui.add_space(4.0);

                for what in SPECIES {
                    let (tow, where_) = advice(what, stw);
                    let caught: Vec<f64> = fishing
                        .caught
                        .iter()
                        .filter(|c| c.what == what)
                        .map(|c| c.mass)
                        .collect();
                    // The name, and what it is worth knowing about it in one line: a
                    // table with a column for every number would be a spreadsheet, and
                    // the thing a person wants is a sentence.
                    ui.horizontal(|ui| {
                        line(
                            ui,
                            format!("{:<9}", what.name()),
                            if caught.is_empty() {
                                paint::FACE
                            } else {
                                paint::GOOD
                            },
                            13.0,
                        );
                        line(ui, tow, paint::LIVE, 11.0);
                    });
                    ui.horizontal(|ui| {
                        line(ui, "         ".into(), paint::LABEL, 11.0);
                        line(ui, where_, paint::LABEL, 10.0);
                    });
                    ui.horizontal(|ui| {
                        line(ui, "         ".into(), paint::LABEL, 11.0);
                        line(ui, what.fights().into(), paint::LABEL, 10.0);
                        // And the half that is yours.
                        if let Some(best) = caught
                            .iter()
                            .cloned()
                            .fold(None, |b: Option<f64>, m| Some(b.map_or(m, |b| b.max(m))))
                        {
                            line(
                                ui,
                                format!("— {} caught, best {best:.1} kg", caught.len()),
                                paint::GOOD,
                                10.0,
                            );
                        }
                    });
                    ui.add_space(5.0);
                }

                ui.separator();
                // The two things that are true of all of them, and both are already on
                // the console somewhere — but nobody connects OUTLOOK to fishing until
                // it is written down next to the fish.
                line(
                    ui,
                    "a falling glass turns them on; a rising one puts them off".into(),
                    paint::LABEL,
                    10.0,
                );
                line(
                    ui,
                    "low sun is the hour they feed — dawn and dusk, not noon".into(),
                    paint::LABEL,
                    10.0,
                );
                line(
                    ui,
                    format!(
                        "glass {}   bite {:.0}%",
                        tendency(sim.sim.wind.barometer().1),
                        crate::fishing::bite_weather(sim.sim.wind.barometer().1) * 100.0
                    ),
                    paint::FACE,
                    11.0,
                );
                ui.add_space(4.0);
                line(ui, "CLOSE  (I)".into(), paint::LABEL, 10.0);
            });
        });
    Ok(())
}

/// The glass in the words a log is kept in.
fn tendency(three_hours: f64) -> &'static str {
    match three_hours {
        t if t < -3.0 => "falling fast",
        t if t < -0.8 => "falling",
        t if t > 3.0 => "rising fast",
        t if t > 0.8 => "rising",
        _ => "steady",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The page's advice has to *work*: towing at the speed and line it gives has to put
    /// the lure where it says the fish is. A guide that reads well and sends you to the
    /// wrong depth is worse than no guide, because it is believed.
    #[test]
    fn the_advice_puts_the_lure_where_the_fish_is() {
        for what in SPECIES {
            let (depth, speed, _) = what.hunting();
            let (tow, _) = advice(what, 2.0);
            // Pull the line length back out of the sentence the player is given, so what
            // is checked is the thing on the screen and not a number beside it.
            let metres: f64 = tow
                .split(", ")
                .nth(1)
                .and_then(|s| s.split(' ').next())
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(|| panic!("no line length in {tow:?}"));
            let got = crate::fishing::lure_depth(metres, sail_core::units::kn_to_ms(speed));
            assert!(
                got >= depth && got < depth + 4.0,
                "{}: {metres} m at {speed} kn is {got:.1} m, wanted {depth}",
                what.name()
            );
        }
    }

    /// And every one of them is reachable at all — the walk out stops at a hundred and
    /// forty metres, and a fish that needs more than the spool holds would be a fish the
    /// page quietly lies about.
    #[test]
    fn every_fish_is_within_the_spool() {
        for what in SPECIES {
            let (tow, _) = advice(what, 2.0);
            let metres: f64 = tow
                .split(", ")
                .nth(1)
                .and_then(|s| s.split(' ').next())
                .and_then(|s| s.parse().ok())
                .unwrap();
            assert!(
                metres < crate::fishing::SPOOL,
                "{} wants {metres} m and the reel holds {}",
                what.name(),
                crate::fishing::SPOOL
            );
        }
    }
}
