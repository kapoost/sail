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
/// Two degrees and a half. A hand bearing compass has a notch and a prism and that is
/// about the width of the notch at arm's length — which is also, not by coincidence,
/// about the accuracy of the reading.
const NOTCH: f64 = 0.044;

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
    let holding = sighting.up || sighting.glasses;
    if holding && sighting.was_viewing.is_none() {
        sighting.was_viewing = Some(*view);
        *view = crate::camera::View::Cockpit;
    }
    if !holding {
        if let Some(back) = sighting.was_viewing.take() {
            *view = back;
        }
    }
    if keys.just_pressed(KeyCode::KeyN) && shifted {
        return;
    }
    if !sighting.up {
        return;
    }
    let shoot_key = keys.just_pressed(KeyCode::KeyN);
    let Ok(eye) = camera.single() else {
        return;
    };

    // Where the notch is pointing, as a true bearing. The camera looks down its own
    // negative z, and the world's north is the simulation's +y — which in Bevy's frame is
    // −z. One conversion, in one place, because a bearing taken through a sight that is
    // ninety degrees out is a fix on the wrong side of the archipelago and looks
    // perfectly reasonable.
    let look = eye.forward();
    let bearing = wrap_tau((look.x as f64).atan2(-look.z as f64));
    sighting.bearing = bearing;

    // What is in the notch: the nearest thing you could actually name, within the width
    // of the notch of the line you are looking down.
    let here = sim.sim.state.pos;
    let mut best: Option<(String, Mark, f64, f64)> = None;
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
        if away > NOTCH {
            continue;
        }
        // And when she does not know where she is, a mark is only nameable once it has
        // been *recognised* through the glasses: the chart has a symbol there and you
        // have a tower in front of you, and nothing joins them until you have read the
        // shape of the coast.
        if nav.lost && !nav.recognised.contains(&mark.name) {
            continue;
        }
        if best.as_ref().is_none_or(|(_, _, _, b)| away < *b) {
            best = Some((mark.name.clone(), mark.kind, range, away));
        }
    }
    sighting.on = best
        .as_ref()
        .map(|(name, kind, range, _)| (name.clone(), *kind, *range));

    // And taking it. A click is the moment you read the card — the bearing is to what is
    // in the notch rather than to where the camera happens to point, because that is
    // what you did: you put the notch *on the lighthouse*.
    if buttons.just_pressed(MouseButton::Left) || shoot_key {
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
    Ok(())
}

/// Where a plotted line's mark is, for keeping the chart tidy.
fn line_mark(item: &crate::chart::Plotted) -> glam::DVec2 {
    match item {
        crate::chart::Plotted::Line { through, .. } => *through,
        crate::chart::Plotted::Cross { at } => *at,
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
    sim: Res<Sim>,
    coast: Res<crate::marks::Coast>,
    sighting: Res<Sighting>,
    mut nav: ResMut<Navigation>,
) {
    if !sighting.glasses || !nav.lost {
        if nav.reading.is_some() && !sighting.glasses {
            nav.reading = None;
        }
        return;
    }
    let here = sim.sim.state.pos;
    // What is in the field of the glasses: anything that can be *seen* from here, which
    // is a different and longer range than anything that can be named.
    let mut in_view: Vec<(f64, &sail_core::land::Landmark)> = coast
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
            Some((bearing, m))
        })
        .collect();
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
