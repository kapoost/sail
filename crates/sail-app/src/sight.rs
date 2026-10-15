//! The hand bearing compass.
//!
//! A hand bearing compass is a card in a bowl with a prism over it and a notch to look
//! through: you put the notch on a lighthouse, you read the number off the card through
//! the prism, and you have a position line. It is the single most useful instrument on a
//! small boat that has no electronics, it costs about as much as a dinner, and it has not
//! changed in a hundred years because there is nothing in it to improve.
//!
//! What it gives you is **one line, not a position**. Everything downstream — crossing
//! two of them, the triangle three of them make, whether to believe it — is in
//! `sail_core::fix`, and what is here is the instrument: where she is looking, what is
//! out there, and the number.
//!
//! The error in the number is the player's. A bearing read off a card in a seaway is good
//! to two or three degrees and no better, and at four miles three degrees is four hundred
//! metres of position line. That is not a difficulty setting either — it is why a fix is
//! a triangle rather than a point, and why the pilot book says to take the beam bearing
//! first and the one ahead last.

use bevy::prelude::*;
use sail_core::fix::Sight;
use sail_core::land::Mark;
use sail_core::units::wrap_tau;

use crate::navigation::Navigation;
use crate::sim::Sim;

/// Whether the compass is up at the eye.
#[derive(Resource, Default)]
pub struct Sighting {
    pub up: bool,
    /// Whether the glasses are at the eye, and how far the lens has got there: a pair of
    /// binoculars comes up in about a third of a second and the picture does not jump.
    pub glasses: bool,
    pub lens: f32,
    /// Where she was looking from before an instrument came out, so it can be given
    /// back. There are no instruments outside the cockpit — the glasses and the compass
    /// are in your hands and your hands are on the boat — so picking one up puts you at
    /// the helm, and putting it down leaves you where you were.
    was_viewing: Option<crate::camera::View>,
    /// What the notch is on at the moment, if anything: its name, what it is, and how
    /// far off. Worked out once a frame so the drawing and the taking agree.
    pub on: Option<(String, Mark, f64)>,
    /// The bearing the card is showing, radians true.
    pub bearing: f64,
    /// What the instrument last said back, whether it was a sight taken or a sight
    /// missed, and until when it stays on the screen.
    ///
    /// An instrument that answers is the whole of the difference between a key that
    /// works and a key you are not sure about. A hand bearing compass answers by having
    /// the number under the lubber line and the tower in the notch at the same moment —
    /// you *see* that you got it. On a screen that moment has to be said out loud.
    pub said: Option<(String, bool)>,
    said_until: f64,
}

/// What the glasses are worth.
///
/// Seven by fifty is what goes to sea — seven times, fifty millimetres of light — and
/// seven times is a field of about seven degrees. Seven degrees on a screen you cannot
/// brace against a shroud is unusable, so the picture here is nearer three times, which
/// is honest about what it is: the *reach* of the instrument rather than its optics.
///
/// Reach is the part that matters. What binoculars do on a boat is not magnify, it is
/// **identify**: a beacon is a stick at two miles and a beacon with a cone on it at two
/// miles through the glasses. So they roughly double the range at which a mark can be
/// named, which is the number the navigation actually uses.
const MAGNIFIES: f32 = 3.1;
pub const GLASSES_REACH: f64 = 2.2;

/// How near the notch has to be before you can say you are on it, radians.
///
/// **One degree and a quarter**, and it was two and a half. Narrowed because a notch
/// you cannot miss is not a notch: the old one caught a tower from a yard either side
/// of it, so a bearing was a key press rather than a thing you lined up, and lining it
/// up is the entire skill the instrument exists to need. A real notch-and-prism is
/// about this wide at arm's length, and holding it on a mark from a moving deck is
/// genuinely a small act of steadiness.
const NOTCH: f64 = 0.022;

/// And how near counts as having nearly got it, radians.
///
/// Four degrees. Inside this she was *aiming at it* and missed, which is worth saying
/// and worth saying differently from "there is nothing there" — a miss you are told
/// about is a miss you correct, and a shot that quietly does nothing teaches that the
/// key is broken.
const NEARLY: f64 = 0.07;

/// Raise and lower it, and read what it is on.
#[allow(clippy::too_many_arguments)]
pub fn aim(
    keys: Res<ButtonInput<KeyCode>>,
    mut view: ResMut<crate::camera::View>,
    buttons: Res<ButtonInput<MouseButton>>,
    sim: Res<Sim>,
    camera: Query<&GlobalTransform, With<crate::camera::MainCamera>>,
    mut sighting: ResMut<Sighting>,
    mut nav: ResMut<Navigation>,
    mut chart: ResMut<crate::chart::Chart>,
    coast: Res<crate::marks::Coast>,
    sound: Res<crate::sound::SoundHandle>,
) {
    // **Shift and N has the compass in your hand; N shoots what is in the notch.**
    //
    // Asked for from the deck after the first arrangement, which put it down again after
    // every sight — and that is wrong for the thing it is: you do not pick a hand bearing
    // compass up three times to take three bearings, you stand there with it and swing it
    // round the horizon, shooting what you recognise. Having it up is a *mode*, and modes
    // get a modifier.
    //
    // Taking the bearing is still a key rather than a click, because aiming is a
    // right-button drag and the deck is a trackpad; the click works too for anybody with
    // a mouse.
    let shifted = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    // The glasses, which are a different instrument from the compass and are used with
    // the other hand: Shift and B puts them up. You can have both — you find the thing
    // with the glasses and then shoot it with the compass, which is exactly the order a
    // navigator does it in.
    if keys.just_pressed(KeyCode::KeyB) && shifted {
        sighting.glasses = !sighting.glasses;
    }
    if keys.just_pressed(KeyCode::KeyN) && shifted {
        sighting.up = !sighting.up;
        if !sighting.up {
            sighting.on = None;
        }
    }
    // **An instrument puts you in the cockpit**, because there are no instruments
    // anywhere else: the glasses and the compass are in your hands, and your hands are
    // on the boat rather than fifteen metres astern of her watching her sail. Asked for
    // from the deck, and it is also the only view the gesture makes sense from — you
    // cannot put a notch on a lighthouse from a camera that is not aboard.
    //
    // Where she was looking is given back when the instrument goes down, so a glance
    // through the glasses does not cost you the view you had set up.
    //
    // Held *every frame* rather than once on the way in, because the view key is still
    // live and one press of it put the glasses fifteen metres astern of the boat — which
    // is what came back from the deck. An instrument in your hand is a place you are
    // standing, not a camera angle you chose.
    let holding = sighting.up || sighting.glasses;
    if holding {
        if sighting.was_viewing.is_none() {
            sighting.was_viewing = Some(*view);
        }
        if *view != crate::camera::View::Cockpit {
            *view = crate::camera::View::Cockpit;
        }
    } else if let Some(back) = sighting.was_viewing.take() {
        *view = back;
    }
    if !holding {
        return;
    }
    let Ok(eye) = camera.single() else {
        return;
    };

    // Where she is looking, as a true bearing. The camera looks down its own negative
    // z, and the world's north is the simulation's +y — which in Bevy's frame is −z. One
    // conversion, in one place, because a bearing taken through a sight that is ninety
    // degrees out is a fix on the wrong side of the archipelago and looks perfectly
    // reasonable.
    //
    // Worked out whenever *either* instrument is up rather than only for the compass. The glasses need it as much: what is in
    // the circle is decided by this number, and with it left behind at whatever the
    // compass last read, a coast full of beacons came back as nothing in the glasses at
    // all. Reported from the deck as "0 in the glasses" with two towers plainly in it.
    let look = eye.forward();
    let bearing = wrap_tau((look.x as f64).atan2(-look.z as f64));
    sighting.bearing = bearing;

    if keys.just_pressed(KeyCode::KeyN) && shifted {
        return;
    }
    if !sighting.up {
        // Nothing is in the notch when there is no notch; the glasses on their own name
        // places rather than taking bearings.
        sighting.on = None;
        return;
    }
    let shoot_key = keys.just_pressed(KeyCode::KeyN);

    // What is in the notch: the nearest thing you could actually name, within the width
    // of the notch of the line you are looking down. And, separately, whatever she was
    // *nearly* on — which is not the same question and is the one that makes a miss
    // worth something.
    let here = sim.sim.state.pos;
    let mut best: Option<(String, Mark, f64, f64)> = None;
    let mut nearly: Option<(String, f64, bool)> = None;
    for mark in &coast.marks {
        let off = mark.at - here;
        let range = off.length();
        // Through the glasses a mark can be named from about twice as far, which is what
        // they are for.
        let reach = if sighting.glasses { GLASSES_REACH } else { 1.0 };
        if range > (mark.kind.identified_from() * reach).min(mark.visible_from(2.0)) {
            continue;
        }
        let to_it = wrap_tau(off.x.atan2(off.y));
        let away = sail_core::units::wrap_pi(to_it - bearing).abs();
        if away > NEARLY {
            continue;
        }
        // And when she does not know where she is, a mark is only nameable once it has
        // been *recognised* through the glasses: the chart has a symbol there and you
        // have a tower in front of you, and nothing joins them until you have read the
        // shape of the coast.
        let nameable = !nav.lost || nav.recognised.contains(&mark.name);
        if nearly.as_ref().is_none_or(|(_, b, _)| away < *b) {
            nearly = Some((mark.name.clone(), away, nameable));
        }
        if !nameable || away > NOTCH {
            continue;
        }
        if best.as_ref().is_none_or(|(_, _, _, b)| away < *b) {
            best = Some((mark.name.clone(), mark.kind, range, away));
        }
    }
    sighting.on = best
        .as_ref()
        .map(|(name, kind, range, _)| (name.clone(), *kind, *range));
    if sighting.said_until < sim.sim.state.time {
        sighting.said = None;
    }

    // And taking it. A click is the moment you read the card — the bearing is to what is
    // in the notch rather than to where the camera happens to point, because that is
    // what you did: you put the notch *on the lighthouse*.
    if buttons.just_pressed(MouseButton::Left) || shoot_key {
        // **And it either catches or it does not, out loud.**
        //
        // Three ways to miss and they are three different mistakes, so they get three
        // different answers: aiming a few degrees off a mark you could have had, aiming
        // at something you have not identified yet — which is the whole of the lost
        // exercise and must not look like a broken key — and aiming at open water.
        if best.is_none() {
            let (word, good) = match &nearly {
                Some((_, away, false)) if *away <= NOTCH => (
                    "that is on the chart but you cannot name it — the glasses first".to_string(),
                    false,
                ),
                Some((name, away, _)) => (
                    format!(
                        "missed {name} by {:.1}° — hold the notch on it and take it \
                         between seas",
                        sail_core::units::deg(*away)
                    ),
                    false,
                ),
                None => ("nothing in the notch".to_string(), false),
            };
            sighting.said = Some((word, good));
            sighting.said_until = sim.sim.state.time + 4.0;
            sound.0.strike_damped(196.0, 0.16, 0.9);
            return;
        }
        if let Some((name, _, _, _)) = best {
            if let Some(mark) = coast.marks.iter().find(|m| m.name == name) {
                let off = mark.at - here;
                // **With the error a hand bearing compass actually has.**
                //
                // Without this the fix is perfect and the cocked hat is a point, which
                // teaches the opposite of the truth: that any two marks will do. A card
                // swinging in a bowl in a seaway is good to two or three degrees, and it
                // is *because* of those degrees that a fine cut is a bad fix and that
                // the third line is worth taking. The error is drawn from the mark and
                // the moment, so it is the same mistake if the same bearing is taken
                // twice in a second and a different one a minute later.
                let swing =
                    0.6 + 1.8 * (sim.sim.telemetry.significant_height as f32 / 2.5).min(1.4) as f64;
                let seed = name
                    .bytes()
                    .fold(0u64, |a, b| a.wrapping_mul(31) ^ b as u64)
                    ^ ((sim.sim.state.time * 0.25) as u64).wrapping_mul(0x9E37_79B9);
                let mut rng = sail_core::rng::Pcg32::new(seed);
                let slip = (rng.next_f64() * 2.0 - 1.0) * sail_core::units::rad(swing);
                let took = wrap_tau(off.x.atan2(off.y) + slip);
                nav.sights.push(Sight {
                    mark: mark.at,
                    bearing: took,
                });
                // Three is a fix and four is a navigator who has stopped sailing. The
                // oldest goes, because it is the one taken from the furthest back — and
                // a bearing taken five minutes ago is a bearing on water she has left.
                if nav.sights.len() > 3 {
                    nav.sights.remove(0);
                }
                // And it goes on the chart, because a bearing you have not plotted is a
                // number you will have forgotten by the time you get below.
                // On the chart, and only the ones that are still part of a fix: a plot
                // with ten old position lines on it is not a plot, it is a scribble, and
                // the first thing a navigator does with a stale line is rub it out.
                chart.plot.retain(|item| {
                    !matches!(item, crate::chart::Plotted::Line { .. })
                        || nav
                            .sights
                            .iter()
                            .any(|s| (s.mark - line_mark(item)).length() < 1.0)
                });
                chart.plot.push(crate::chart::Plotted::Line {
                    through: mark.at,
                    bearing: took,
                });
                // And she is told she got it, with the number she got — which is what a
                // navigator writes in the book and the only part of this that survives
                // to the chart table.
                sighting.said = Some((
                    format!(
                        "{name} {:03.0}° — {} of three",
                        sail_core::units::deg(took).rem_euclid(360.0),
                        nav.sights.len()
                    ),
                    true,
                ));
                sighting.said_until = sim.sim.state.time + 4.0;
                sound.0.strike_damped(784.0, 0.2, 0.85);
            }
        }
    }
}

/// The glasses at the eye: the lens coming up, and the view narrowing with it.
///
/// Over a third of a second rather than instantly, because a cut from one field of view
/// to another reads as a different camera rather than as a pair of binoculars — the eye
/// needs the movement to understand what happened.
pub fn glasses(
    time: Res<Time>,
    mut sighting: ResMut<Sighting>,
    mut lens: Query<&mut Projection, With<crate::camera::MainCamera>>,
) {
    let want = if sighting.glasses { 1.0 } else { 0.0 };
    let k = 1.0 - (-7.0 * time.delta_secs()).exp();
    sighting.lens += (want - sighting.lens) * k;
    if sighting.lens < 0.001 && want == 0.0 {
        sighting.lens = 0.0;
    }
    let Ok(mut projection) = lens.single_mut() else {
        return;
    };
    if let Projection::Perspective(perspective) = projection.as_mut() {
        // The camera's own field, which is the one the rest of the game is framed for.
        const NAKED: f32 = 0.72;
        perspective.fov = NAKED / (1.0 + (MAGNIFIES - 1.0) * sighting.lens);
    }
}

/// Draw it: a card across the bottom of the view with a notch down the middle.
pub fn draw(
    mut contexts: bevy_egui::EguiContexts,
    sighting: Res<Sighting>,
    nav: Res<Navigation>,
    chart: Res<crate::chart::Chart>,
) -> Result {
    use bevy_egui::egui;
    if !sighting.up && sighting.lens < 0.01 {
        return Ok(());
    }
    // Not over the chart. With it spread on the table the compass card and the notch
    // were being drawn across the paper — and you cannot sight a mark through a chart
    // table anyway: one of them is what you are looking at and the other is where you
    // write it down.
    if chart.spread {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let screen = ctx.viewport_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("bearing compass"),
    ));
    let ink = egui::Color32::from_rgb(232, 228, 214);
    let card = egui::Color32::from_rgba_unmultiplied(12, 14, 18, 190);

    // The glasses, drawn as what you see through them: one circle, because that is what
    // a pair of binoculars actually looks like from behind. The two-circle picture is a
    // convention from films and is not what your eyes do — they fuse the fields into one.
    //
    // Cut out of the dark with a stroke rather than a hole: a circle outlined with a
    // line as wide as the screen fills everything outside it, which egui can do and
    // cutting a hole in a rectangle cannot.
    if sighting.lens > 0.01 {
        let hole = screen.height() * 0.42;
        let wide = screen.width() + screen.height();
        painter.circle_stroke(
            screen.center(),
            hole + wide * 0.5,
            egui::Stroke::new(
                wide,
                egui::Color32::from_rgba_unmultiplied(4, 5, 7, (235.0 * sighting.lens) as u8),
            ),
        );
        painter.circle_stroke(
            screen.center(),
            hole,
            egui::Stroke::new(
                2.0,
                egui::Color32::from_rgba_unmultiplied(18, 20, 24, (200.0 * sighting.lens) as u8),
            ),
        );
    }
    // What the glasses make of the shape in front of them, which is the other half of
    // pilotage: before you take a bearing on a lighthouse you have to know it is *that*
    // lighthouse.
    if sighting.lens > 0.5 {
        if let Some(reading) = &nav.reading {
            painter.text(
                egui::pos2(
                    screen.center().x,
                    screen.center().y + screen.height() * 0.30,
                ),
                egui::Align2::CENTER_CENTER,
                reading,
                egui::FontId::monospace(13.0),
                egui::Color32::from_rgb(232, 176, 58),
            );
        }
    }
    if !sighting.up {
        return Ok(());
    }

    // The notch, down the middle of the view, which is what you line up on the thing.
    let middle = screen.center().x;
    painter.line_segment(
        [
            egui::pos2(middle, screen.center().y - 70.0),
            egui::pos2(middle, screen.center().y + 70.0),
        ],
        egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(232, 228, 214, 120),
        ),
    );

    // The card: a strip of degrees sliding past, which is what you see through the
    // prism. Only the arc either side of the notch, because that is all the bowl shows.
    // Above the console rather than across it. The console is a hundred and thirty
    // points of instruments along the bottom and the card was being drawn on top of
    // them — reported from the deck with a screenshot, which is the only way a thing
    // like that gets found.
    let strip = egui::Rect::from_center_size(
        egui::pos2(middle, screen.bottom() - 232.0),
        egui::vec2(420.0, 46.0),
    );
    painter.rect_filled(strip, 4.0, card);
    let degrees = sighting.bearing.to_degrees();
    let per_degree: f64 = 420.0 / 60.0;
    for tick in -32..=32 {
        let at = (degrees + tick as f64).round();
        let x = middle as f64 + (at - degrees) * per_degree;
        if (x as f32) < strip.left() + 4.0 || (x as f32) > strip.right() - 4.0 {
            continue;
        }
        let whole = (at.rem_euclid(360.0)) as i32;
        let long = whole % 10 == 0;
        painter.line_segment(
            [
                egui::pos2(x as f32, strip.bottom() - if long { 18.0 } else { 10.0 }),
                egui::pos2(x as f32, strip.bottom() - 4.0),
            ],
            egui::Stroke::new(if long { 1.2 } else { 0.6 }, ink),
        );
        if long {
            painter.text(
                egui::pos2(x as f32, strip.top() + 12.0),
                egui::Align2::CENTER_CENTER,
                format!("{whole:03}"),
                egui::FontId::monospace(11.0),
                ink,
            );
        }
    }
    // The lubber line and the number, which is the thing you write down.
    painter.line_segment(
        [
            egui::pos2(middle, strip.top()),
            egui::pos2(middle, strip.bottom()),
        ],
        egui::Stroke::new(1.6, egui::Color32::from_rgb(224, 104, 84)),
    );
    painter.text(
        egui::pos2(middle, strip.top() - 14.0),
        egui::Align2::CENTER_BOTTOM,
        format!("{:03.0}", degrees.rem_euclid(360.0)),
        egui::FontId::monospace(20.0),
        ink,
    );

    // What is in the notch, if anything — which is the question that decides whether the
    // number is worth anything at all.
    let said = match &sighting.on {
        Some((name, kind, range)) => {
            let what = match kind {
                Mark::Lighthouse => format!("{name} Lt  {}", crate::marks::light_character(name)),
                _ => format!("{name}  {}", kind.abbreviation()),
            };
            format!("{what}   {:.1} M — N takes it", range / 1_852.0)
        }
        None => "nothing in the notch — aim with the right button held, Shift N to put \
                 it down"
            .to_string(),
    };
    painter.text(
        egui::pos2(middle, strip.bottom() + 16.0),
        egui::Align2::CENTER_TOP,
        said,
        egui::FontId::monospace(13.0),
        if sighting.on.is_some() {
            egui::Color32::from_rgb(232, 176, 58)
        } else {
            ink
        },
    );
    painter.text(
        egui::pos2(middle, strip.bottom() + 36.0),
        egui::Align2::CENTER_TOP,
        format!(
            "{} taken — two cross, three check each other",
            nav.sights.len()
        ),
        egui::FontId::monospace(11.0),
        ink,
    );
    // And what the last press of the key did, which is the half of an instrument that a
    // screen has to say and a real one does not: on the water the tower is in the notch
    // and the number is under the lubber line and you can see for yourself that you have
    // it. Above the card rather than below, where the eye already is for the number.
    if let Some((said, got_it)) = &sighting.said {
        painter.text(
            egui::pos2(middle, strip.top() - 40.0),
            egui::Align2::CENTER_BOTTOM,
            said,
            egui::FontId::monospace(14.0),
            if *got_it {
                egui::Color32::from_rgb(142, 214, 128)
            } else {
                egui::Color32::from_rgb(226, 118, 96)
            },
        );
    }
    Ok(())
}

/// Where a plotted line's mark is, for keeping the chart tidy.
fn line_mark(item: &crate::chart::Plotted) -> glam::DVec2 {
    match item {
        crate::chart::Plotted::Line { through, .. } => *through,
        crate::chart::Plotted::Cross { at } => *at,
        crate::chart::Plotted::Rule { from, .. } => *from,
    }
}

/// How wide a group has to be before it is a *shape* rather than a row of dots, radians.
///
/// Eight degrees. Three marks within a few degrees of each other are all the same
/// bearing as far as recognition goes: the angles between them are smaller than the
/// error in reading them, and the pattern is not distinctive enough to be anywhere in
/// particular. A coast you can recognise is one that is spread out.
const WORTH_RECOGNISING: f64 = 0.14;

/// How near the three-point solution has to land to the water she is actually in before
/// she may believe she has recognised the place, metres.
///
/// Half a mile. This is not a test of the player's eye — the angles the glasses report
/// are honest — it is a test of whether the *pattern is unambiguous*: if a group of
/// marks could be another group somewhere else on this coast, the solution lands
/// somewhere else and the recognition fails, which is exactly what happens to a sailor
/// who decides a headland is the one he wants.
const RECOGNISED_WITHIN: f64 = 900.0;

/// Recognising where you are from the shape of what you can see.
///
/// This is the other half of pilotage and the half nobody writes down: before you take a
/// bearing on a lighthouse you have to know it is *that* lighthouse. What a sailor
/// actually does is look at the group — this tower, that island behind it, the gap —
/// and match the arrangement against the chart. Two marks is never enough; three in a
/// row is a pattern that belongs somewhere.
///
/// Done here the way a station pointer does it: the two horizontal angles between three
/// marks put you at one point on the chart and nowhere else, so if the solution lands
/// where she is, the group has been read correctly. If it lands somewhere else, the
/// arrangement she is looking at matches a *different* place on this coast — which is
/// precisely the mistake the exercise is about, and the glasses say so rather than
/// correcting her.
pub fn recognise(
    keys: Res<ButtonInput<KeyCode>>,
    sim: Res<Sim>,
    coast: Res<crate::marks::Coast>,
    sighting: Res<Sighting>,
    mut nav: ResMut<Navigation>,
) {
    // Nothing happens until you ask. **B, with the glasses up, is the question *is this
    // enough to name where I am*** — and asking it is the act, rather than the game
    // quietly scanning the horizon for you.
    //
    // Which is the difference between an instrument and an oracle. A pair of glasses
    // held on a stretch of coast tells you nothing by itself; what tells you something
    // is a person looking at a group, deciding it is distinctive enough to be worth
    // working on, and doing the work. So the player composes the view — three marks,
    // well spread — and then asks.
    if !sighting.glasses {
        if nav.reading.is_some() {
            nav.reading = None;
        }
        return;
    }
    let shifted = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if !(keys.just_pressed(KeyCode::KeyB) && !shifted) {
        if nav.reading.is_none() {
            nav.reading =
                Some("B — see whether what is in the glasses is enough to name".to_string());
        }
        return;
    }
    if !nav.lost {
        nav.reading = Some("you know where you are — every mark on the chart is named".to_string());
        return;
    }
    let here = sim.sim.state.pos;
    // What is in the field of the glasses: anything that can be *seen* from here, which
    // is a different and longer range than anything that can be named.
    let mut in_view: Vec<(f64, &sail_core::land::Landmark, f64)> = coast
        .marks
        .iter()
        .filter_map(|m| {
            let off = m.at - here;
            let range = off.length();
            if range > m.visible_from(2.0) {
                return None;
            }
            let bearing = wrap_tau(off.x.atan2(off.y));
            let away = sail_core::units::wrap_pi(bearing - sighting.bearing).abs();
            // The field of a pair of glasses, give or take: everything the circle holds.
            if away > 0.42 {
                return None;
            }
            Some((bearing, m, range))
        })
        .collect();

    // **A mark whose symbol you can read names itself**, and needs no group at all.
    //
    // Asked for from the deck, and it is the first half of pilotage rather than an
    // easing of it: a beacon near enough for its topmark to be made out is not "a
    // stick", it is *that* beacon, because the topmark is the whole reason somebody put
    // a shape on top of a pole. The same goes for a lighthouse close enough to read its
    // bands and a spire with its village under it. What that does and does not give you
    // is the point — it tells you *which mark this is*, so you may take a bearing on it
    // and write its name on the chart; it does not tell you where you are. That still
    // takes three lines, or the three-point work below.
    //
    // The ranges are `Mark::identified_from`, the same numbers the notch uses, through
    // the glasses' reach. One fact, one place: an instrument that names a thing you
    // could not then take a bearing on would be a lie told twice.
    let readable: Vec<&sail_core::land::Landmark> = in_view
        .iter()
        .filter(|(_, m, range)| {
            *range <= (m.kind.identified_from() * GLASSES_REACH).min(m.visible_from(2.0))
        })
        .map(|(_, m, _)| *m)
        .collect();
    if !readable.is_empty() {
        let mut fresh: Vec<String> = Vec::new();
        for m in &readable {
            if !nav.recognised.contains(&m.name) {
                nav.recognised.push(m.name.clone());
                fresh.push(format!("{} {}", m.name, m.kind.abbreviation()));
            }
        }
        nav.reading = Some(if fresh.is_empty() {
            format!(
                "{} — already on the chart. A bearing on it is one line.",
                readable
                    .iter()
                    .map(|m| m.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        } else if fresh.len() == 1 {
            format!(
                "the mark is clear enough to read: {}. Named on the chart — now take a \
                 bearing on it.",
                fresh[0]
            )
        } else {
            format!(
                "clear enough to read: {}. Named on the chart.",
                fresh.join(", ")
            )
        });
        return;
    }

    if in_view.len() < 3 {
        nav.reading = Some(format!(
            "{} in the glasses — a group of three is what tells you where you are",
            in_view.len()
        ));
        return;
    }
    // Left to right, which is how a shape is read and how the angles below are taken.
    in_view.sort_by(|a, b| {
        sail_core::units::wrap_pi(a.0 - sighting.bearing)
            .total_cmp(&sail_core::units::wrap_pi(b.0 - sighting.bearing))
    });

    // Every three of them, nearest the middle of the field first.
    let mut best: Option<(f64, [&sail_core::land::Landmark; 3])> = None;
    for i in 0..in_view.len() - 2 {
        for j in i + 1..in_view.len() - 1 {
            for k in j + 1..in_view.len() {
                let (a, b, c) = (in_view[i], in_view[j], in_view[k]);
                let left = sail_core::units::wrap_pi(b.0 - a.0);
                let right = sail_core::units::wrap_pi(c.0 - b.0);
                if left.abs() < WORTH_RECOGNISING || right.abs() < WORTH_RECOGNISING {
                    continue;
                }
                let Some(stands) = sail_core::fix::three_point(a.1.at, b.1.at, c.1.at, left, right)
                else {
                    continue;
                };
                let out_by = (stands - here).length();
                if best.as_ref().is_none_or(|(had, _)| out_by < *had) {
                    best = Some((out_by, [a.1, b.1, c.1]));
                }
            }
        }
    }

    match best {
        Some((out_by, group)) if out_by < RECOGNISED_WITHIN => {
            let named: Vec<String> = group.iter().map(|m| m.name.clone()).collect();
            for name in &named {
                if !nav.recognised.contains(name) {
                    nav.recognised.push(name.clone());
                }
            }
            nav.reading = Some(format!(
                "that is {} — {} and {} beyond it. Named on the chart.",
                named[0], named[1], named[2]
            ));
        }
        Some(_) => {
            nav.reading = Some(
                "a tower, an island and a gap — and that arrangement is somewhere else \
                 on this coast. Try a group further apart."
                    .to_string(),
            );
        }
        None => {
            nav.reading = Some(
                "all much the same bearing — a shape you can recognise is one that is \
                 spread out"
                    .to_string(),
            );
        }
    }
}
