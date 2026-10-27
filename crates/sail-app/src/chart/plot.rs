//! The plot: everything worked out and drawn on the paper.
//!
//! Position lines and the fix they make, the reckoning and the circle round it, the
//! pencil work, and what she knows. The whole of this file is the answer to *where is
//! she*, which is why every piece of it has to ask whether she is allowed to be told.

use bevy_egui::egui;

use super::*;
use crate::course::Course;
use crate::sim::Sim;

/// The reckoning, the track between fixes, and the circle she is somewhere in.
///
/// Drawn whether or not the chart may say where she is, because none of it *is* where
/// she is: it is what she worked out, which is hers to have.
/// The track made good: fix to fix, the one line on this paper that is not an estimate.
///
/// Drawn whether or not the chart may say where she is, because it is not where she is
/// — it is where she *was*, twice, and both of those she established herself. The speed
/// on it is distance over time between two positions she worked, which is speed over
/// the ground and the number no instrument aboard can give her.
pub(crate) fn the_track_made_good(t: &Table, nav: &crate::navigation::Navigation) {
    for pair in nav.fixes.windows(2) {
        let (one, then) = (pair[0], pair[1]);
        let (a, b) = (t.to_screen(one.0), t.to_screen(then.0));
        t.p.line_segment([a, b], egui::Stroke::new(1.4, ink::TRACK));
        let hours = ((then.1 - one.1) / 3_600.0).max(1e-6);
        let miles = (then.0 - one.0).length() / 1_852.0;
        t.p.text(
            egui::pos2((a.x + b.x) * 0.5, (a.y + b.y) * 0.5) + egui::vec2(7.0, -5.0),
            egui::Align2::LEFT_BOTTOM,
            format!("{:.1} kn made good", miles / hours),
            egui::FontId::monospace(10.0),
            ink::TRACK,
        );
        t.note(
            egui::pos2((a.x + b.x) * 0.5, (a.y + b.y) * 0.5) + egui::vec2(7.0, 6.0),
            "track made good: fix to fix, the one line here that is not an estimate",
            ink::TRACK,
        );
    }
}

/// The reckoning and the circle she is somewhere in.
///
/// Drawn whether or not the chart may say where she is, because none of it *is* where
/// she is: it is what she worked out, which is hers to have.
pub(crate) fn reckoning(t: &Table, nav: &crate::navigation::Navigation, when: f64) {
    // **The reckoning is the half of navigation the fix cannot do — so it is drawn
    // when the fix cannot do it.**
    //
    // While the chart can say where she is it says so, and a dashed argument about where
    // she must have got to, laid alongside a leg she actually measured, is two lines
    // within a pencil-width of each other carrying two sets of numbers about the same
    // water. Reported from the deck as *two lines drawn for no reason*, and it is the
    // right reading: the second one is not telling her anything she has not been told
    // better. It is still run on underneath, every frame, so that the moment the last
    // mark goes behind the headland the plot is already there — which is the moment it
    // was always for.
    if t.knows {
        return;
    }
    if let Some(run) = nav.reckoning.as_ref() {
        // **The legs, corner by corner.** The fix at one end, where she has got to at
        // the other, and every alteration in between — because that is the shape of
        // the thing she actually worked out, and because a leg she has finished has
        // stopped moving and can therefore carry numbers. Run as one line it would say
        // she had sailed straight since the fix, with its course and distance written
        // at the running end, sliding along under the mark as she went: numbers about
        // the present, on the one instrument that is not about the present.
        let mut corners = Vec::with_capacity(run.altered.len() + 2);
        corners.push((run.from, run.since));
        corners.extend(run.altered.iter().copied());
        corners.push((run.plotted_at(), when));
        let live = corners.len() - 2;
        let mut span = egui::vec2(0.0, -1.0);
        for (leg, pair) in corners.windows(2).enumerate() {
            let ((one, began), (then, ended)) = (pair[0], pair[1]);
            let (a, b) = (t.to_screen(one), t.to_screen(then));
            // Dashed, because a reckoning is not a track: it is an argument about
            // where she must have got to, and paper says that with a broken line.
            span = b - a;
            let steps = ((span.length() / 9.0) as usize).clamp(1, 240);
            for i in (0..steps).step_by(2) {
                let along_it = |k: usize| k as f32 / steps as f32;
                t.p.line_segment(
                    [
                        a + span * along_it(i),
                        a + span * along_it((i + 1).min(steps)),
                    ],
                    egui::Stroke::new(1.1, ink::PEN),
                );
            }
            if leg == live {
                continue;
            }
            // A leg she has finished, with the two figures a navigator writes along
            // one: the course she made good on it and how far she ran. Written where
            // she sailed it and left there, which is the whole point of breaking the
            // line — the water she has already crossed does not change its mind.
            let off = then - one;
            let course = off.x.atan2(off.y).rem_euclid(std::f64::consts::TAU);
            let hours = ((ended - began) / 3_600.0).max(1e-6);
            let miles = off.length() / 1_852.0;
            t.p.text(
                egui::pos2((a.x + b.x) * 0.5, (a.y + b.y) * 0.5) + egui::vec2(6.0, -5.0),
                egui::Align2::LEFT_BOTTOM,
                format!(
                    "{:03.0}   {miles:.2} M   {:.1} kn",
                    course.to_degrees(),
                    miles / hours
                ),
                egui::FontId::monospace(9.0),
                ink::PEN.gamma_multiply(0.85),
            );
            t.note(
                egui::pos2((a.x + b.x) * 0.5, (a.y + b.y) * 0.5) + egui::vec2(6.0, 6.0),
                "reckoning: a course she steered and the miles the log counted on it",
                ink::PEN,
            );
            // And a bar across the corner, so that the place she altered is a mark on
            // the paper rather than a bend in a dashed line.
            if span.length() > 1.0 {
                let across = egui::vec2(-span.normalized().y, span.normalized().x) * 4.0;
                t.p.line_segment([b + across, b - across], egui::Stroke::new(1.0, ink::PEN));
            }
        }
        let now = t.to_screen(run.plotted_at());
        // Where it says she is: a dot with a bar across it, which is how a dead
        // reckoning position is marked and how it is told apart at a glance from
        // the ringed dot of a fix.
        t.p.circle_filled(now, 1.5, ink::PEN);
        let along = if span.length() > 1.0 {
            span.normalized()
        } else {
            egui::vec2(0.0, -1.0)
        };
        let across = egui::vec2(-along.y, along.x) * 5.0;
        t.p.line_segment(
            [now + across, now - across],
            egui::Stroke::new(1.2, ink::PEN),
        );
        // **And what the plot does not know yet.**
        //
        // The paper has been laying off a course she is no longer steering, so the mark
        // is somewhere she certainly is not — and without a word on it that reads as a
        // broken plot rather than as an unwritten log. It is the one number here that is
        // about the navigator and not about the boat, and it goes away the moment the
        // chart is spread, because spreading it is the work.
        if run.unwritten.len() > 1 {
            t.p.text(
                now + egui::vec2(8.0, -8.0),
                egui::Align2::LEFT_BOTTOM,
                format!(
                    "log not written up — {:.0} min",
                    ((when - run.written_up) / 60.0).max(1.0)
                ),
                egui::FontId::monospace(9.0),
                ink::BOAT,
            );
        }
        // **And the circle she is somewhere in**, drawn round the reckoning
        // rather than round the boat — which is the whole point of it. Round the
        // boat it would be a statement about the truth, and the truth is the one
        // thing nobody in this exercise has. Round the reckoning it says what it
        // means: this is my best position, and this is how far out I may be.
        if run.spread > 1.0 {
            let radius = (run.spread as f32 * t.scale).clamp(3.0, 400.0);
            t.p.circle(
                now,
                radius,
                egui::Color32::TRANSPARENT,
                egui::Stroke::new(1.0, ink::PEN.gamma_multiply(0.55)),
            );
            t.p.text(
                now + egui::vec2(8.0, 8.0),
                egui::Align2::LEFT_TOP,
                format!("DR — within {:.1} M", run.spread / 1_852.0),
                egui::FontId::monospace(10.0),
                ink::PEN,
            );
            // **What the log said, drawn where it cannot be read as a place.**
            //
            // A leg timed with the log is a real measurement and the chart should show
            // it — the player has just spent a minute of glass on it and wants to see
            // that it landed. But laid off as a line on the water it is a claim about
            // two positions, and while she is running on a reckoning she has neither.
            // Asked for from the deck as *draw it somewhere inside the circle*, which is
            // the right place and for the right reason: inside the circle nothing is a
            // position. So it is a vector at the reckoning — which way the water was
            // going past her and how fast — kept short enough to stay in the doubt it
            // belongs to.
            let fresh = nav
                .measured
                .zip(nav.measured_on)
                .filter(|((_, read_at), _)| when - read_at < 1_800.0);
            if let Some(((knots, _), course)) = fresh {
                let along = egui::vec2(course.sin() as f32, -(course.cos() as f32));
                let head = now + along * (radius * 0.7).clamp(10.0, 44.0);
                let across = egui::vec2(-along.y, along.x);
                t.p.line_segment([now, head], egui::Stroke::new(1.3, ink::TRACK));
                for side in [-1.0, 1.0] {
                    t.p.line_segment(
                        [head, head - along * 6.0 + across * 3.0 * side],
                        egui::Stroke::new(1.3, ink::TRACK),
                    );
                }
                t.p.text(
                    head + egui::vec2(5.0, -4.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("{knots:.1} kn by the log"),
                    egui::FontId::monospace(9.0),
                    ink::TRACK,
                );
            }
        }
    }
}

/// The card in the corner: the figures she has, each fading with its own age.
///
/// Navigation is a state of knowledge rather than a position, and this is that state
/// written down the way a navigator keeps it — added to as things become known, and
/// believed less as they go stale.
pub(crate) fn what_she_knows(t: &Table, nav: &crate::navigation::Navigation, now: f64) {
    // --- What she knows ------------------------------------------------
    //
    // **A sheet of paper with the figures on it, fading as they go stale.**
    //
    // Navigation is not a position, it is a *state of knowledge*, and until now
    // that state was scattered: the fix was a cross, the set was a line in the
    // log file, the measured speed was gone the moment it was written, and how
    // old any of it was you had to work out from the clock. A navigator keeps
    // this on the corner of the chart in pencil, adds to it as things become
    // known, and stops believing a line when it is old enough to be wrong.
    //
    // So each line carries its own age and fades with it, which is the honest
    // way to draw knowledge: a figure at full strength is one you may act on, a
    // faint one is one worth taking again. Nothing is deleted — a faded line is
    // still the last thing you knew, which is more than nothing.
    {
        let card = egui::Rect::from_min_size(
            egui::pos2(t.rect.left() + 16.0, t.rect.bottom() - 128.0),
            egui::vec2(214.0, 74.0),
        );
        let mut lines: Vec<(String, f32)> = Vec::new();
        // How much of a figure is left, given how long ago it was taken and how
        // long that kind of figure stays worth having.
        let left =
            |when: f64, good_for: f64| (1.0 - ((now - when) / good_for).clamp(0.0, 1.0)) as f32;
        if nav.fixed_at.is_some() {
            lines.push((
                format!(
                    "fix     {:>5.0} min ago",
                    (now - nav.last_fix).max(0.0) / 60.0
                ),
                // A fix is worth having for as long as it is worth sailing on,
                // which is the same twenty minutes the chart gives it.
                left(nav.last_fix, crate::navigation::FIX_HOLDS),
            ));
        }
        if let Some((knots, when)) = nav.measured {
            lines.push((
                format!("log     {knots:>5.1} kn  ±{:.0}%", nav.log_slip * 100.0),
                // Half an hour. A boat's speed is a fact about her trim and the
                // sea she is in, and both of those have changed by then.
                left(when, 1_800.0),
            ));
        }
        if nav.set_measured && nav.unexplained > 0.01 {
            lines.push((
                format!(
                    "set     {:>5.2} kn",
                    sail_core::units::ms_to_kn(nav.unexplained)
                ),
                // The stream is a fact about this water and this hour of the
                // tide, and it holds up longer than anything else here.
                left(nav.last_fix, 5_400.0).max(0.35),
            ));
        }
        if let Some(run) = nav.reckoning.as_ref() {
            lines.push((
                format!("within  {:>5.2} M", run.spread / 1_852.0),
                // Always current: it is the one line here that is *about* going
                // stale, so it cannot go stale itself.
                1.0,
            ));
        }
        if !lines.is_empty() {
            t.p.rect_filled(card, 2.0, ink::PAPER.gamma_multiply(0.75));
            t.p.rect_stroke(
                card,
                2.0,
                egui::Stroke::new(1.0, ink::COAST.gamma_multiply(0.6)),
                egui::StrokeKind::Inside,
            );
            t.p.text(
                card.left_top() + egui::vec2(8.0, 6.0),
                egui::Align2::LEFT_TOP,
                "WHAT SHE KNOWS",
                egui::FontId::monospace(9.0),
                ink::COAST,
            );
            for (i, (line, alpha)) in lines.iter().take(4).enumerate() {
                t.p.text(
                    card.left_top() + egui::vec2(8.0, 20.0 + i as f32 * 13.0),
                    egui::Align2::LEFT_TOP,
                    line,
                    egui::FontId::monospace(10.0),
                    ink::PEN.gamma_multiply(0.25 + 0.75 * alpha),
                );
            }
        }
    }
}

/// Everything the pencil has left on the paper: position lines and their bands, ruled
/// legs, timed runs, times, fixes and crosses.
pub(crate) fn plotted(t: &Table, chart: &Chart, crossing_at: Option<glam::DVec2>, heading: f64) {
    // --- What is already on the chart --------------------------------
    //
    // On the table only. The folded chart is a glance — *where am I, what is ahead* —
    // and a plot on it is a thicket of pencil over a picture two inches square. Working
    // belongs where there is room to work.
    //
    // One line per kind of mark, and each kind draws itself: what a plot contains is
    // then the list below rather than three hundred lines of drawing with the list
    // buried in it.
    for (i, item) in chart.plot.iter().enumerate() {
        match *item {
            Plotted::Line {
                through,
                bearing,
                speed,
            } => hand_line(
                t,
                through,
                bearing,
                speed,
                crossing_at,
                chart.range as f64,
                heading,
            ),
            Plotted::Cross { at, fix } => a_mark(t, at, fix),
            Plotted::Run { from, to, minutes } => the_log_leg(t, from, to, minutes),
            Plotted::Stamp { at, hour } => the_time(t, at, hour),
            Plotted::Entry(entry) => an_entry(t, &entry),
            // Darker while it is in your hand, which is the only feedback a straight
            // edge needs: you can see that you have hold of this one rather than of the
            // chart.
            Plotted::Rule { from, to } => {
                ruled(t, from, to, chart.moving.is_some_and(|(held, _)| held == i))
            }
        }
    }
    if let Some((from, to)) = chart.dividers {
        the_dividers(t, from, to);
    }
}

/// A position line somebody ruled, rather than one the compass took.
///
/// The protractor and the parallel rule both leave one: a bearing laid off from a point
/// on the paper, which is how you work with a bearing you were *told* — a light bearing
/// such-and-such from the harbour entrance, the lead of a transit — rather than one you
/// shot yourself. It carries the speed that was known when it was drawn, because a line
/// and a speed are the pair a running fix is made of.
pub(crate) fn hand_line(
    t: &Table,
    through: glam::DVec2,
    bearing: f64,
    speed: Option<f64>,
    crossing_at: Option<glam::DVec2>,
    range: f64,
    heading: f64,
) {
    // Drawn the length a navigator draws one, which is not across the
    // whole chart: a position line is a *pencil line near where you
    // think you are*, long enough to cross the others and no longer.
    // Lines run off both edges make a cobweb and hide the coast under
    // it — reported from the deck in those words.
    //
    // So it runs from the mark towards the observer, as far as the
    // crossing it makes plus a little, and when there is no crossing
    // yet, far enough to be worth looking at.
    let away = (bearing + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU);
    let along = glam::DVec2::new(away.sin(), away.cos());
    let reach = crossing_at
        .map(|c| ((c - through).length() * 1.25).max(range * 0.25))
        .unwrap_or(range * 0.9);
    let a = t.to_screen(through);
    let b = t.to_screen(through + along * reach);
    // **The corridor, not the line.**
    //
    // A hand bearing compass is good to two or three degrees, so a
    // position line is honestly a band — and a navigator who draws
    // the band knows at a glance how much of the chart his fix is
    // actually standing on. Three degrees either side, which is the
    // error the instrument in this game really has.
    for side in [-1.0, 1.0] {
        let edge = (away + side * sail_core::units::rad(3.0)).rem_euclid(std::f64::consts::TAU);
        let off = glam::DVec2::new(edge.sin(), edge.cos());
        t.p.line_segment(
            [a, t.to_screen(through + off * reach)],
            egui::Stroke::new(0.7, ink::PENCIL.gamma_multiply(0.4)),
        );
    }
    t.p.line_segment([a, b], egui::Stroke::new(1.1, ink::PENCIL));
    t.p.text(
        t.to_screen(through) + egui::vec2(6.0, 10.0),
        egui::Align2::LEFT_TOP,
        format!("{:03.0}", bearing.to_degrees()),
        egui::FontId::monospace(10.0),
        ink::PENCIL,
    );
    // **What the line is worth, written on the line.**
    //
    // A position line is not a position and the two things it *is*
    // depend entirely on where the mark lay: one taken ahead or
    // astern tells you how far along your track you have got and
    // nothing about how far off you are; one on the beam tells you
    // how far off and nothing about how far along. Every pilot book
    // says it and no chart shows it, and a navigator who does not
    // know which of the two he has just drawn will believe a fix
    // made of two lines that are both the same one.
    //
    // And the speed above it when there is a speed, because a line
    // plus a speed is a running fix waiting to happen: that is the
    // pair that lets the next bearing on the same mark be crossed
    // with this one.
    let off_the_bow = sail_core::units::wrap_pi(bearing - heading)
        .abs()
        .to_degrees();
    let worth = if !(30.0..150.0).contains(&off_the_bow) {
        "how far along"
    } else {
        "how far off"
    };
    let middle = egui::pos2((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
    t.p.text(
        middle + egui::vec2(6.0, 4.0),
        egui::Align2::LEFT_TOP,
        worth,
        egui::FontId::monospace(9.0),
        ink::PENCIL.gamma_multiply(0.7),
    );
    t.note(
        middle + egui::vec2(6.0, 14.0),
        "laid off by hand: a bearing you ruled, not one the compass took",
        ink::PENCIL,
    );
    if let Some(knots) = speed {
        t.p.text(
            middle + egui::vec2(6.0, -4.0),
            egui::Align2::LEFT_BOTTOM,
            format!("{knots:.1} kn"),
            egui::FontId::monospace(10.0),
            ink::PENCIL,
        );
    }
}

/// A dot on the paper: a fix if it is ringed, anything else if it is crossed.
///
/// **The dot is the position** and the ring or the arms exist only to make it findable
/// among the pencil — which is why the arms have a gap in the middle. A cross drawn
/// through its own centre buries the point it is claiming under the crossing of two
/// strokes; four short arms leave the dot showing, which is how a draughtsman marks a
/// point and why a cocked hat has a dot in the middle of it.
pub(crate) fn a_mark(t: &Table, at: glam::DVec2, fix: bool) {
    let c = t.to_screen(at);
    // The dot first, because the dot is the position and everything
    // else on this mark exists to point at it.
    t.p.circle_filled(c, 1.5, ink::PENCIL);
    t.note(
        c + egui::vec2(8.0, 6.0),
        if fix {
            "fix: where the bearings crossed, and the position she worked"
        } else {
            "pencil: a point you marked yourself"
        },
        ink::PENCIL,
    );
    if fix {
        // A ring round it, which is what a fix is on paper — and
        // left open at the dot so the ink never covers the point it
        // is claiming.
        t.p.circle(
            c,
            6.0,
            egui::Color32::TRANSPARENT,
            egui::Stroke::new(1.2, ink::PENCIL),
        );
    } else {
        // Arms with a gap in the middle. A cross drawn through its
        // own centre has its position buried under the crossing of
        // two strokes; one drawn as four short arms leaves the dot
        // showing, which is how a draughtsman marks a point.
        for (dx, dy) in [(1.0, 1.0), (1.0, -1.0)] {
            let along = egui::vec2(dx, dy);
            t.p.line_segment(
                [c + along * 3.0, c + along * 7.0],
                egui::Stroke::new(1.3, ink::PENCIL),
            );
            t.p.line_segment(
                [c - along * 3.0, c - along * 7.0],
                egui::Stroke::new(1.3, ink::PENCIL),
            );
        }
    }
}

/// A leg that was actually sailed, drawn the way a course made good is drawn.
///
/// One arrow along it, the course over the line and what she did under it. Two figures
/// rather than three, because the distance is the one you can measure off the paper with
/// the dividers and the speed is the one you cannot.
pub(crate) fn the_log_leg(t: &Table, from: glam::DVec2, to: glam::DVec2, minutes: f64) {
    let (a, b) = (t.to_screen(from), t.to_screen(to));
    t.p.line_segment([a, b], egui::Stroke::new(1.3, ink::TRACK));
    let off = to - from;
    let miles = off.length() / 1_852.0;
    let course = off.x.atan2(off.y).rem_euclid(std::f64::consts::TAU);
    // The arrowhead, at the far end and pointing the way she went.
    let along = (b - a).normalized();
    let across = egui::vec2(-along.y, along.x);
    for side in [-1.0, 1.0] {
        t.p.line_segment(
            [b, b - along * 9.0 + across * 4.0 * side],
            egui::Stroke::new(1.3, ink::TRACK),
        );
    }
    let middle = egui::pos2((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
    t.p.text(
        middle + egui::vec2(6.0, -4.0),
        egui::Align2::LEFT_BOTTOM,
        format!("{:03.0}", course.to_degrees()),
        egui::FontId::monospace(10.0),
        ink::TRACK,
    );
    t.note(
        middle + egui::vec2(6.0, 16.0),
        "log leg: timed with the glass between two positions she had",
        ink::TRACK,
    );
    t.p.text(
        middle + egui::vec2(6.0, 4.0),
        egui::Align2::LEFT_TOP,
        format!(
            "{:.2} M in {:.0} min — {:.1} kn",
            miles,
            minutes,
            miles / (minutes / 60.0).max(1e-6)
        ),
        egui::FontId::monospace(10.0),
        ink::TRACK,
    );
}

/// A time written beside a mark, in the four figures a deck log is kept in.
pub(crate) fn the_time(t: &Table, at: glam::DVec2, hour: f32) {
    // Four figures and a tick under them, which is how a time is
    // written beside a mark: no colon, because a deck log has never
    // used one, and the tick so that a page with six times on it
    // still says which is which.
    let c = t.to_screen(at);
    t.p.line_segment(
        [c + egui::vec2(0.0, -3.0), c + egui::vec2(0.0, 3.0)],
        egui::Stroke::new(1.2, ink::PENCIL),
    );
    t.note(
        c + egui::vec2(5.0, 4.0),
        "the time the mark beside it was made",
        ink::PENCIL,
    );
    t.p.text(
        c + egui::vec2(5.0, -4.0),
        egui::Align2::LEFT_BOTTOM,
        format!(
            "{:02.0}{:02.0}",
            hour.floor(),
            (hour.fract() * 60.0).floor()
        ),
        egui::FontId::monospace(11.0),
        ink::PENCIL,
    );
}

/// A ruled leg: from here to there, with what you steer and how far it is.
///
/// The ends say which point they mean. A tick across the line says *the line stops here*
/// and leaves you unable to tell whether the leg ends at the tick, at the middle of it,
/// or somewhere under the pencil; a small upright cross with a dot at its centre says a
/// point and nothing else.
pub(crate) fn ruled(t: &Table, from: glam::DVec2, to: glam::DVec2, in_hand: bool) {
    // A ruled leg, with the two things written along it that a
    // navigator writes along one: the course to steer and how far it
    // is. Written once, above the line, which is where it goes on
    // paper — the distance goes underneath on a passage chart, but
    // one label on a screen this size is already two lines of type.
    let (a, b) = (t.to_screen(from), t.to_screen(to));
    // Darker while it is in your hand, which is the only feedback a
    // straight edge needs: you can see that you have hold of this
    // one rather than of the chart.
    t.p.line_segment(
        [a, b],
        egui::Stroke::new(if in_hand { 2.0 } else { 1.2 }, ink::PENCIL),
    );
    let off = to - from;
    let bearing = off.x.atan2(off.y).rem_euclid(std::f64::consts::TAU);
    // **The ends say which point they mean.**
    //
    // A tick across the line says *the line stops here*, which is
    // not the same thing and was what was drawn: at a glance you
    // could not tell whether the leg ended at the tick, at the
    // middle of it, or somewhere under the pencil. A small upright
    // cross with a dot at its centre says a point and nothing else,
    // which is how a position is marked on paper and why a cocked
    // hat has a dot in the middle of it. Asked for from the deck as
    // ends that point at something exactly and unambiguously.
    for end in [a, b] {
        for (dx, dy) in [(5.0, 0.0), (0.0, 5.0)] {
            t.p.line_segment(
                [end + egui::vec2(dx, dy), end - egui::vec2(dx, dy)],
                egui::Stroke::new(1.0, ink::PENCIL),
            );
        }
        t.p.circle_filled(end, 1.4, ink::PENCIL);
    }
    t.note(
        egui::pos2((a.x + b.x) * 0.5, (a.y + b.y) * 0.5) + egui::vec2(6.0, 6.0),
        "ruled by hand: a leg to steer, or two marks held in line",
        ink::PENCIL,
    );
    t.p.text(
        egui::pos2((a.x + b.x) * 0.5, (a.y + b.y) * 0.5) + egui::vec2(6.0, -6.0),
        egui::Align2::LEFT_BOTTOM,
        format!(
            "{:03.0}   {:.2} M",
            bearing.to_degrees(),
            off.length() / 1_852.0
        ),
        egui::FontId::monospace(10.0),
        ink::PENCIL,
    );
}

/// The dividers, which measure and leave nothing behind.
pub(crate) fn the_dividers(t: &Table, from: glam::DVec2, to: glam::DVec2) {
    let (a, b) = (t.to_screen(from), t.to_screen(to));
    let off = to - from;
    let miles = off.length() / 1_852.0;
    let bearing = off.x.atan2(off.y).rem_euclid(std::f64::consts::TAU);
    t.p.line_segment([a, b], egui::Stroke::new(1.4, ink::TRACK));
    for end in [a, b] {
        // A ring with the point showing through it, like every other mark on
        // this paper: the dividers measure *between two points* and a filled
        // circle three pixels across hides the one it is standing on.
        t.p.circle(
            end,
            3.5,
            egui::Color32::TRANSPARENT,
            egui::Stroke::new(1.2, ink::TRACK),
        );
        t.p.circle_filled(end, 1.2, ink::TRACK);
    }
    let label = format!(
        "{miles:.2} M   {:03.0} / {:03.0}",
        bearing.to_degrees(),
        (bearing.to_degrees() + 180.0) % 360.0
    );
    let middle = egui::pos2((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
    t.note(
        middle + egui::vec2(8.0, 6.0),
        "dividers: measuring, not drawing — they leave nothing behind",
        ink::TRACK,
    );
    t.p.text(
        middle + egui::vec2(8.0, -8.0),
        egui::Align2::LEFT_BOTTOM,
        label,
        egui::FontId::monospace(12.0),
        ink::TRACK,
    );
}

/// Where she is, where she is bound, and who else is on the water — or the words that
/// say the chart is not allowed to tell you.
///
/// All of it behind one question, which is the whole of the exercise: a chart that
/// always knows is a plotter, and the question *which bay is that* cannot be asked of
/// one.
pub(crate) fn her_own_place(
    t: &Table,
    sim: &Sim,
    fleet: &crate::fleet::Fleet,
    course: &Course,
    nav: &crate::navigation::Navigation,
    spread: bool,
) {
    // What a chart with no fix on it says, which is nothing — and it says it where
    // the boat would have been, because that is where the eye goes first.
    // How long the fix has left, when she is living on one. A position is a
    // statement about a moment and this is the moment running out — drawn rather
    // than counted, because a bar is read without being looked at.
    if nav.lost && t.knows && spread {
        // How many of the marks this fix was taken on can still be made out, which
        // is what is actually keeping the position: while two of them are there,
        // another fix is a minute's work and she is not lost in any sense that
        // matters. The clock behind it is only a backstop.
        t.p.text(
            egui::pos2(t.rect.left() + 24.0, t.rect.top() + 114.0),
            egui::Align2::LEFT_CENTER,
            format!(
                "{} of {} marks still in sight",
                nav.still_in_sight.min(nav.fixed_on.len()),
                nav.fixed_on.len()
            ),
            egui::FontId::monospace(11.0),
            if nav.still_in_sight > 2 {
                ink::COAST
            } else {
                ink::BOAT
            },
        );
        let left = nav.fix_left(sim.sim.state.time);
        let bar = egui::Rect::from_min_size(
            egui::pos2(t.rect.left() + 24.0, t.rect.top() + 96.0),
            egui::vec2(188.0, 7.0),
        );
        t.p.rect_filled(bar, 1.0, ink::PAPER);
        let mut run = bar;
        run.set_width(bar.width() * left);
        t.p.rect_filled(run, 1.0, ink::MARK);
        t.p.text(
            egui::pos2(bar.right() + 8.0, bar.center().y),
            egui::Align2::LEFT_CENTER,
            "this fix",
            egui::FontId::monospace(10.0),
            ink::COAST,
        );
    }
    if !t.knows {
        t.p.text(
            t.rect.center(),
            egui::Align2::CENTER_CENTER,
            "POSITION UNKNOWN",
            egui::FontId::monospace(if spread { 16.0 } else { 11.0 }),
            ink::BOAT,
        );
        if spread {
            t.p.text(
                t.rect.center() + egui::vec2(0.0, 20.0),
                egui::Align2::CENTER_CENTER,
                "take bearings on two marks and cross them",
                egui::FontId::monospace(11.0),
                ink::COAST,
            );
        }
    }
    if t.knows {
        // The boat: a wedge pointing where she is pointing, drawn where she *is* on
        // the paper rather than in the middle of it. Those are the same point only
        // while the chart is following her, which it stops doing the moment she does
        // not know where she is.
        let her = t.to_screen(sim.sim.state.pos);
        let heading = sim.sim.state.heading as f32;
        let nose = egui::vec2(heading.sin(), -heading.cos());
        let beam = egui::vec2(-nose.y, nose.x);
        t.p.add(egui::Shape::convex_polygon(
            vec![
                her + nose * 9.0,
                her - nose * 5.0 + beam * 4.0,
                her - nose * 5.0 - beam * 4.0,
            ],
            ink::BOAT,
            egui::Stroke::NONE,
        ));
        // And the bearing to the consignment, which is the line you are trying to
        // make good rather than the one you are sailing.
        let target = t.to_screen(course.target());
        t.p.line_segment([her, target], egui::Stroke::new(1.0, ink::MARK));
        t.note(
            egui::pos2((her.x + target.x) * 0.5, (her.y + target.y) * 0.5) + egui::vec2(6.0, 6.0),
            "to where she is bound: where you are going, not where you are",
            ink::MARK,
        );

        // **And everybody else.**
        //
        // Until now the only place another boat appeared was a list of names in the
        // settings panel with a distance beside each — which answers "how far" and
        // not the question anybody actually has, which is *where*. A fleet on a
        // chart is the oldest answer to it and it costs one wedge each.
        //
        // Hollow rather than filled, so your own boat is still the one the eye finds
        // first on a page with five of them on it, and named on the table where
        // there is room for the lettering. Not on the folded chart: at two inches
        // square a name is a smudge across an island.
        //
        // Only when she knows where she is, like everything else here — a chart that
        // will not say where *you* are cannot say where your friends are either, or
        // the exercise is over the moment somebody else joins.
        for boat in &fleet.boats {
            let at = glam::DVec2::new(boat.drawn.pos[0] as f64, boat.drawn.pos[1] as f64);
            let them = t.to_screen(at);
            if !t.rect.contains(them) {
                continue;
            }
            let heading = boat.drawn.heading;
            let nose = egui::vec2(heading.sin(), -heading.cos());
            let beam = egui::vec2(-nose.y, nose.x);
            t.p.add(egui::Shape::convex_polygon(
                vec![
                    them + nose * 8.0,
                    them - nose * 4.5 + beam * 3.5,
                    them - nose * 4.5 - beam * 3.5,
                ],
                egui::Color32::TRANSPARENT,
                egui::Stroke::new(1.3, ink::BOAT),
            ));
            if spread && !boat.name.is_empty() {
                t.p.text(
                    them + egui::vec2(10.0, -2.0),
                    egui::Align2::LEFT_CENTER,
                    &boat.name,
                    egui::FontId::monospace(10.0),
                    ink::BOAT,
                );
            }
        }
    }
}

/// Where the lines cross, the triangle they make, and the button that says you believe
/// it.
///
/// **Both are drawn**: the crossing because it is the answer and the triangle because it
/// is the confidence — a cocked hat is the navigator's own report on how carefully he
/// just did it, and a fix with none round it is a fix nobody has checked.
///
/// A fix is not a position until somebody writes it in the log, which is the moment it
/// becomes what the boat is working from and the moment it can be wrong. So this returns
/// the point only when the button was pressed; believing it is a decision and decisions
/// have a button.
/// What the buttons under the fix are asking for.
pub(crate) enum Pressed {
    /// Put the position on the paper.
    OnTheChart(glam::DVec2),
    /// Write the line in the book.
    InTheLog,
}

/// Where the lines cross, the triangle they make, and the two buttons under them.
///
/// **Both parts of the fix are drawn**: the crossing because it is the answer and the
/// triangle because it is the confidence — a cocked hat is the navigator's own report on
/// how carefully he just did it, and a fix with none round it is a fix nobody has
/// checked.
///
/// **And the two buttons are two acts.** Marking the paper and writing the book are not
/// the same thing and never were: most lines in a deck log have no fix in them — a time,
/// a course, the glass, what was set — and a position pencilled on a chart is a
/// statement about *where*, which is the one thing a page of a book cannot show you.
/// Asked for from the deck as two buttons, and splitting them makes each one say exactly
/// what it did.
pub(crate) fn the_cocked_hat(
    t: &Table,
    nav: &crate::navigation::Navigation,
    buttons: (egui::Rect, egui::Rect),
    pressed: Option<egui::Pos2>,
) -> Option<Pressed> {
    let got = sail_core::fix::plot(&nav.sights);
    if let Some(got) = got {
        let at = t.to_screen(got.at);
        t.p.circle(
            at,
            6.0,
            egui::Color32::TRANSPARENT,
            egui::Stroke::new(1.4, ink::BOAT),
        );
        t.p.circle_filled(at, 1.6, ink::BOAT);
        t.p.text(
            at + egui::vec2(10.0, -4.0),
            egui::Align2::LEFT_BOTTOM,
            format!(
                "FIX  hat {:.0} m, cut {:.0}  —  {}",
                got.hat,
                got.cut.to_degrees(),
                sail_core::fix::verdict(&got, nav.sights.len())
            ),
            egui::FontId::monospace(11.0),
            ink::BOAT,
        );
    }
    // **And why there is no fix, when there is no fix.**
    //
    // A greyed button that does not say what it is waiting for is a broken key. The
    // three reasons are three different mistakes and each has a different answer: no
    // lines on the paper means the book is full of bearings nobody has laid on, one line
    // is not a position, and two lines that do not cross are almost always two bearings
    // on the same mark with nothing run between them — which is the one case that *looks*
    // like it ought to work and is the thing a running fix exists to fix.
    if got.is_none() {
        let waiting = match nav.sights.len() {
            0 => "no lines on the chart — lay a bearing on from the book",
            1 => "one line is not a position — lay a second bearing on",
            _ => {
                "these lines do not cross: two bearings on one mark need a run \
                  between them"
            }
        };
        t.p.text(
            buttons.0.left_bottom() + egui::vec2(0.0, 5.0),
            egui::Align2::LEFT_TOP,
            waiting,
            egui::FontId::monospace(9.0),
            ink::BOAT,
        );
    }
    let (chart_button, log_button) = buttons;
    let mut asked = None;
    for (slot, said, live) in [
        ("ON THE CHART", chart_button, got.is_some()),
        ("IN THE LOG", log_button, true),
    ]
    .map(|(name, rect, live)| (rect, name, live))
    {
        t.p.rect_filled(
            slot,
            2.0,
            ink::PAPER.gamma_multiply(if live { 1.0 } else { 0.5 }),
        );
        t.p.rect_stroke(
            slot,
            2.0,
            egui::Stroke::new(1.0, ink::COAST.gamma_multiply(if live { 1.0 } else { 0.5 })),
            egui::StrokeKind::Inside,
        );
        t.p.text(
            slot.center(),
            egui::Align2::CENTER_CENTER,
            said,
            egui::FontId::monospace(10.0),
            ink::COAST.gamma_multiply(if live { 1.0 } else { 0.5 }),
        );
        if live && pressed.is_some_and(|q| slot.contains(q)) {
            asked = Some(match (said, got) {
                ("ON THE CHART", Some(got)) => Pressed::OnTheChart(got.at),
                _ => Pressed::InTheLog,
            });
        }
    }
    asked
}

/// The position lines: the sights themselves, drawn where they now apply.
///
/// **A position line is a statement about where she was when the bearing was taken**,
/// and she does not stay there — so each one carries the run since it was shot and is
/// drawn through the mark *plus that run*. Fresh, it goes through the mark, which is
/// where a position line starts its life. Twenty minutes later it is a hand's breadth
/// off it, dashed, with the run it was carried along drawn in: that is a transferred
/// position line, and crossing one with a fresh bearing on the same mark is a running
/// fix — the oldest trick on a coast with one lighthouse and nothing else to shoot.
pub(crate) fn the_sights(
    t: &Table,
    nav: &crate::navigation::Navigation,
    coast: &crate::marks::Coast,
    crossing_at: Option<glam::DVec2>,
    range: f64,
    heading: f64,
) {
    for sight in &nav.sights {
        let through = sight.through();
        let carried = sight.ran.length();
        let moved = carried > 60.0;
        let away = (sight.bearing + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU);
        let along = glam::DVec2::new(away.sin(), away.cos());
        // The length a navigator draws one: from the mark towards the observer, as far
        // as the crossing it makes plus a little. Lines run off both edges make a cobweb
        // and hide the coast under it.
        let reach = crossing_at
            .map(|c| ((c - through).length() * 1.25).max(range * 0.25))
            .unwrap_or(range * 0.9);
        let a = t.to_screen(through);
        let b = t.to_screen(through + along * reach);
        // **The corridor, not the line.** A hand bearing compass is good to two or three
        // degrees, so a position line is honestly a band — and a navigator who draws the
        // band can see at a glance how much of the chart his fix is standing on.
        for side in [-1.0, 1.0] {
            let edge = (away + side * sail_core::units::rad(3.0)).rem_euclid(std::f64::consts::TAU);
            let off = glam::DVec2::new(edge.sin(), edge.cos());
            t.p.line_segment(
                [a, t.to_screen(through + off * reach)],
                egui::Stroke::new(0.7, ink::PENCIL.gamma_multiply(0.4)),
            );
        }
        if moved {
            // Dashed, because it is no longer a reading — it is a reading carried, and
            // everything carried on a chart is drawn broken.
            let span = b - a;
            let steps = ((span.length() / 10.0) as usize).clamp(1, 240);
            for i in (0..steps).step_by(2) {
                let at = |k: usize| k as f32 / steps as f32;
                t.p.line_segment(
                    [a + span * at(i), a + span * at((i + 1).min(steps))],
                    egui::Stroke::new(1.1, ink::PENCIL),
                );
            }
            // The run it was carried along, drawn from the mark to where the line stands
            // now. This is the construction, and drawing it is the difference between a
            // chart that shows a transferred line and one that shows *why* it is there.
            let from = t.to_screen(sight.mark);
            t.p.line_segment(
                [from, a],
                egui::Stroke::new(0.8, ink::TRACK.gamma_multiply(0.7)),
            );
            t.p.circle_filled(a, 1.6, ink::PENCIL);
            t.p.text(
                egui::pos2((from.x + a.x) * 0.5, (from.y + a.y) * 0.5) + egui::vec2(4.0, 2.0),
                egui::Align2::LEFT_TOP,
                format!("ran {:.2} M", carried / 1_852.0),
                egui::FontId::monospace(9.0),
                ink::TRACK,
            );
        } else {
            t.p.line_segment([a, b], egui::Stroke::new(1.1, ink::PENCIL));
        }
        t.p.text(
            a + egui::vec2(6.0, 10.0),
            egui::Align2::LEFT_TOP,
            format!("{:03.0}", sight.bearing.to_degrees()),
            egui::FontId::monospace(10.0),
            ink::PENCIL,
        );
        // **What the line is worth, written on the line.** One taken ahead or astern
        // tells you how far along your track you have got and nothing about how far off;
        // one on the beam tells you how far off and nothing about how far along. Every
        // pilot book says it and no chart shows it.
        let off_the_bow = sail_core::units::wrap_pi(sight.bearing - heading)
            .abs()
            .to_degrees();
        let worth = if !(30.0..150.0).contains(&off_the_bow) {
            "how far along"
        } else {
            "how far off"
        };
        let middle = egui::pos2((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
        t.p.text(
            middle + egui::vec2(6.0, 4.0),
            egui::Align2::LEFT_TOP,
            worth,
            egui::FontId::monospace(9.0),
            ink::PENCIL.gamma_multiply(0.7),
        );
        let name = coast
            .marks
            .iter()
            .find(|m| (m.at - sight.mark).length() < 1.0)
            .map(|m| m.name.clone())
            .unwrap_or_default();
        t.note(
            middle + egui::vec2(6.0, 14.0),
            if moved {
                format!(
                    "transferred line on {name}: the old bearing carried along her run, to \
                     cross with the new one"
                )
            } else {
                format!("position line on {name}: she is somewhere on it")
            },
            ink::PENCIL,
        );
    }
}

/// A line of the deck log, written on the paper where it happened.
///
/// Four figures and a tick, like any other time on a chart, and then what a skipper
/// actually writes: the course if he has just altered, what the log said, the glass and
/// which way it is going, the wind and the sea — and what was set, drawn rather than
/// spelled, because a sail plan is the one thing in a log book that has always been a
/// picture.
pub(crate) fn an_entry(t: &Table, e: &Entry) {
    // **On the paper, the navigational minimum and nothing else.**
    //
    // A log line has a dozen figures in it and most of them are about the day rather
    // than about the place: the glass, the sea, what was set. Written beside every mark
    // they turn a plot into a wall of type you have to read past to see the coast —
    // reported from the deck as clutter, which is exactly what it was. The time, the
    // course and what she was doing are the three a navigator wants *at the position*;
    // the rest of the line is kept in the book, down the side of the table, where it
    // can be read as a column instead of hunted for among islands.
    let c = t.to_screen(e.at);
    t.p.line_segment(
        [c + egui::vec2(0.0, -3.0), c + egui::vec2(0.0, 3.0)],
        egui::Stroke::new(1.2, ink::PENCIL),
    );
    let mut said = format!(
        "{:02.0}{:02.0}",
        e.hour.floor(),
        (e.hour.fract() * 60.0).floor()
    );
    if let Some(course) = e.course {
        said += &format!("  co {:03.0}", course.to_degrees().rem_euclid(360.0));
    }
    if let Some(knots) = e.speed {
        said += &format!("  {knots:.1} kn");
    }
    t.p.text(
        c + egui::vec2(7.0, -6.0),
        egui::Align2::LEFT_BOTTOM,
        said,
        egui::FontId::monospace(11.0),
        ink::PENCIL,
    );
    t.note(
        c + egui::vec2(7.0, 4.0),
        "a line of the log: the rest of it is in the book",
        ink::PENCIL,
    );
}

/// What was set, drawn the way a log book draws it.
///
/// The main with a bar across the foot for every reef in her, and the headsail as the
/// triangle that is actually up — rolled away, it is drawn narrower, because that is
/// what a furling genoa *is* and a book that wrote "genoa" for a hand's width of cloth
/// would be a book that lied about the morning.
///
/// In the book and not on the chart: a sail plan beside every mark is a page of pictures
/// over the coast you are trying to look at.
pub(crate) fn sail_plan(t: &Table, at: egui::Pos2, e: &Entry, tall: f32) {
    let ink = ink::PENCIL;
    let foot = tall * 0.56;
    let mast = at + egui::vec2(0.0, tall);
    t.p.line_segment([at, mast], egui::Stroke::new(1.0, ink));
    t.p.line_segment(
        [mast, mast + egui::vec2(foot, 0.0)],
        egui::Stroke::new(1.0, ink),
    );
    t.p.line_segment(
        [at, mast + egui::vec2(foot, 0.0)],
        egui::Stroke::new(1.0, ink),
    );
    // A bar across the foot for each slab she has in.
    let reefs = (e.reef * 3.0).round() as i32;
    for slab in 1..=reefs {
        let up = tall * (1.0 - 0.17 * slab as f32);
        let across = foot * (1.0 - 0.17 * slab as f32);
        let y = at.y + up;
        t.p.line_segment(
            [egui::pos2(at.x, y), egui::pos2(at.x + across, y)],
            egui::Stroke::new(0.9, ink),
        );
    }
    // The headsail, forward of the mast and as wide as what is left of it.
    let set = (1.0 - e.furl as f32).clamp(0.0, 1.0);
    if set > 0.02 {
        let head = at + egui::vec2(-2.0, 0.0);
        let tack = head + egui::vec2(-foot * set, tall);
        let clew = head + egui::vec2(0.0, tall);
        t.p.line_segment([head, tack], egui::Stroke::new(1.0, ink));
        t.p.line_segment([tack, clew], egui::Stroke::new(1.0, ink));
        t.p.line_segment([head, clew], egui::Stroke::new(1.0, ink));
    }
}

/// What the sail plan says in words, for the column beside the picture.
pub(crate) fn what_was_set(e: &Entry) -> String {
    let reefs = (e.reef * 3.0).round() as i32;
    let set = 1.0 - e.furl;
    let what = match e.headsail {
        sail_core::state::Headsail::Genoa => "gen",
        sail_core::state::Headsail::Code0 => "C0",
        sail_core::state::Headsail::Gennaker => "gnk",
        sail_core::state::Headsail::Spinnaker => "kite",
    };
    match (reefs, set > 0.02) {
        (0, true) if set < 0.75 => format!("full + {what} rolled"),
        (0, true) => format!("full main + {what}"),
        (0, false) => "full main alone".to_string(),
        (n, true) if set < 0.75 => format!("{n} reef + {what} rolled"),
        (n, true) => format!("{n} reef + {what}"),
        (n, false) => format!("{n} reef, no headsail"),
    }
}

/// The pilot's water: the way in, as legs somebody steers.
///
/// **Drawn like advice and not like a track.** It is dashed, because it is where she
/// *should* go and not where anybody has been; the legs are ruled between marks a
/// helmsman can steer for; and the last one is the approach, which gets a circle on it
/// because arriving is a different act from passing. The wind is in it — the search
/// refuses to lay a leg a square-rigger cannot sail — so the line moves when the wind
/// does, which is the whole reason a pilot is a person and not a chart.
pub(crate) fn the_way_in(t: &Table, route: &[glam::DVec2], anchorage: glam::DVec2) {
    if route.len() < 2 {
        return;
    }
    const INK: egui::Color32 = egui::Color32::from_rgb(40, 132, 96);
    for pair in route.windows(2) {
        let (a, b) = (t.to_screen(pair[0]), t.to_screen(pair[1]));
        let along = b - a;
        let len = along.length();
        if len < 1.0 {
            continue;
        }
        let step = along / len;
        // Dashes of four pixels with three of air, which is how a recommended track is
        // drawn on paper and why nobody confuses one with a bearing.
        let mut at = 0.0;
        while at < len {
            let end = (at + 4.0).min(len);
            t.p.line_segment([a + step * at, a + step * end], egui::Stroke::new(1.6, INK));
            at = end + 3.0;
        }
    }
    for turn in route.iter().take(route.len() - 1).skip(1) {
        t.p.circle_stroke(t.to_screen(*turn), 3.0, egui::Stroke::new(1.3, INK));
    }
    // The anchorage, which is the end of the job.
    let home = t.to_screen(anchorage);
    t.p.circle_stroke(home, 7.0, egui::Stroke::new(1.6, INK));
    t.p.circle_filled(home, 2.0, INK);
    t.note(
        home + egui::vec2(10.0, -10.0),
        "the way in — deep water she can sail",
        INK,
    );
}

/// The race course: the line, the marks, and the order they are taken in.
///
/// **A course is a drawing before it is a sail.** Every race in the world starts with
/// somebody looking at a board or a chart and working out which way round it goes, and
/// the one question a sailor has on the water — *where is the line from here* — is a
/// question a chart answers and a horizon does not.
pub(crate) fn the_course(t: &Table, race: &crate::race::Race) {
    if !race.running() {
        return;
    }
    const INK: egui::Color32 = egui::Color32::from_rgb(214, 92, 20);
    let (boat, pin) = race.line;
    let (mut a, mut b) = (t.to_screen(boat), t.to_screen(pin));
    // **A chart symbol has a size of its own.**
    //
    // The course is eight hundred metres on a chart five miles across, so drawn to
    // scale the start line is six pixels and the whole thing is a smudge — which is
    // what *I cannot see the line on the map* meant. Every paper chart in the world
    // solves this the same way: a buoy is drawn at the size of the symbol and not at
    // the size of the buoy. So the line is never shorter on the paper than a symbol
    // somebody can aim at, and it grows into a true-scale drawing as the chart is
    // zoomed in on it.
    const LEAST_LINE: f32 = 26.0;
    let middle_on_paper = ((a.to_vec2() + b.to_vec2()) * 0.5).to_pos2();
    let span = b - a;
    if span.length() < LEAST_LINE {
        let along = if span.length() > 0.1 {
            span / span.length()
        } else {
            egui::vec2(1.0, 0.0)
        };
        a = middle_on_paper - along * LEAST_LINE * 0.5;
        b = middle_on_paper + along * LEAST_LINE * 0.5;
    }
    // The line itself, heavier than anything else on the paper: for five minutes it is
    // the only thing on the chart that matters.
    t.p.line_segment([a, b], egui::Stroke::new(2.4, INK));
    // Ends: the committee boat is a square and the pin is a circle, which is how a
    // sailing instruction draws them and how everybody tells the two apart.
    t.p.rect_filled(
        egui::Rect::from_center_size(a, egui::vec2(6.0, 6.0)),
        1.0,
        INK,
    );
    t.p.circle_filled(b, 3.2, INK);
    t.p.text(
        middle_on_paper + egui::vec2(0.0, 9.0),
        egui::Align2::CENTER_TOP,
        "START",
        egui::FontId::monospace(8.0),
        INK,
    );

    // The legs, in the order they are sailed, with the marks numbered on them. Drawn
    // from the middle of the line out and back, which is the shape of the course and
    // the thing a list of positions cannot say.
    let mut was = boat.lerp(pin, 0.5);
    for (n, mark) in race.marks.iter().enumerate() {
        let (from, to) = (t.to_screen(was), t.to_screen(*mark));
        let along = to - from;
        let len = along.length();
        if len > 2.0 {
            let step = along / len;
            let mut at = 0.0;
            while at < len {
                let end = (at + 4.0).min(len);
                t.p.line_segment(
                    [from + step * at, from + step * end],
                    egui::Stroke::new(1.2, INK),
                );
                at = end + 4.0;
            }
            // An arrow at the middle of the leg: which way round a course goes is the
            // thing that gets argued about in the dinghy park and never on the water.
            let mid = from + step * (len * 0.5);
            let wing = egui::vec2(-step.y, step.x) * 3.0;
            t.p.line_segment([mid - step * 4.0 + wing, mid], egui::Stroke::new(1.2, INK));
            t.p.line_segment([mid - step * 4.0 - wing, mid], egui::Stroke::new(1.2, INK));
        }
        // The mark, at symbol size whatever the scale is, with its number beside it.
        let at = t.to_screen(*mark);
        t.p.circle_stroke(at, 4.5, egui::Stroke::new(1.6, INK));
        t.p.circle_filled(at, 1.4, INK);
        t.p.text(
            at + egui::vec2(6.0, -7.0),
            egui::Align2::LEFT_TOP,
            format!("{}", n + 1),
            egui::FontId::monospace(9.0),
            INK,
        );
        was = *mark;
    }
    // Home again: the last leg ends at the line it started from.
    let (from, to) = (t.to_screen(was), middle_on_paper);
    t.p.line_segment([from, to], egui::Stroke::new(1.0, INK.gamma_multiply(0.6)));

    // The other boats, as the marks a committee puts on a sketch — enough to say who is
    // where, which is the whole of what a chart can say about a fleet.
    for rival in &race.rivals {
        let at = t.to_screen(rival.at);
        t.p.circle_filled(at, 2.2, INK);
        t.note(at + egui::vec2(4.0, -10.0), rival.name, INK);
    }
}

/// The water the brig can be taken into, drawn where she is.
///
/// **Asked for from the deck in the one sentence that matters: *what are her limits?***
/// She was circling and would not sail, and everything needed to understand why was in
/// the model and nowhere on the paper — her wind is not your wind, her limit is not your
/// limit, and the one hail she had for it, *we cannot lie that close*, says what is
/// wrong without saying where she *can* go.
///
/// So: a wedge she cannot be led into, and the two courses that bound it. Sixty-five
/// degrees either side of the wind at **her** position — sampled at her rig's height,
/// which in a shear is a knot or two more than you have on deck — with the six degrees
/// of margin her master complains inside, because a limit you are exactly on is a limit
/// you are about to be inside.
///
/// Drawn from her bow out to half a mile, which is the water a pilot is choosing in. The
/// rule to sail by is the one every square-rig pilot book states: **lay her a course
/// outside the wedge, and if the mark is inside it, give her two legs instead of one.**
pub(crate) fn where_she_can_go(
    t: &Table,
    at: glam::DVec2,
    heading: f64,
    from: f64,
    astern: &[glam::DVec2],
) {
    use std::f64::consts::TAU;
    // The limit, with the margin her master starts complaining inside — taken from the
    // model rather than restated here, so the shading and the hail cannot disagree.
    let shut = crate::mission::WILL_NOT_LIE;
    /// How far out it is worth drawing, metres. Half a mile: further than she can be led
    /// in one leg and about what fits on a glance chart.
    const REACH: f64 = 900.0;
    const CLOSED: egui::Color32 = egui::Color32::from_rgba_premultiplied(104, 26, 26, 68);
    const EDGE: egui::Color32 = egui::Color32::from_rgb(150, 58, 48);

    let her = t.to_screen(at);
    // **Where she has been**, which is the question a pilot is really asking when he
    // looks astern: not *where is she* but *is she following me*. A line of her own,
    // thinner and darker than the recommended track, so that her water and the water you
    // chose for her can be told apart at a glance — and when the two disagree, that is
    // the picture of what went wrong.
    for pair in astern.windows(2) {
        t.p.line_segment(
            [t.to_screen(pair[0]), t.to_screen(pair[1])],
            egui::Stroke::new(1.2, EDGE.gamma_multiply(0.65)),
        );
    }
    // The wedge, as a fan of triangles — the painter has no pie slice, and a fan is what
    // one is anyway.
    let steps = 18;
    for i in 0..steps {
        let a = from - shut + 2.0 * shut * (i as f64 / steps as f64);
        let b = from - shut + 2.0 * shut * ((i + 1) as f64 / steps as f64);
        let p = |angle: f64| t.to_screen(at + glam::DVec2::new(angle.sin(), angle.cos()) * REACH);
        t.p.add(egui::Shape::convex_polygon(
            vec![her, p(a), p(b)],
            CLOSED,
            egui::Stroke::NONE,
        ));
    }
    // And the two edges, which are the courses she *can* lie — the thing a pilot reads
    // off this rather than the shading.
    for side in [-1.0, 1.0] {
        let course = from + side * shut;
        let end = t.to_screen(at + glam::DVec2::new(course.sin(), course.cos()) * REACH);
        t.p.line_segment([her, end], egui::Stroke::new(1.4, EDGE));
    }
    // Her own hull: a wedge pointing where she is pointing, so the shading is attached
    // to a ship and not floating on the water.
    let nose = |ahead: f64, across: f64| {
        let along = glam::DVec2::new(heading.sin(), heading.cos());
        let side = glam::DVec2::new(along.y, -along.x);
        t.to_screen(at + along * ahead + side * across)
    };
    t.p.add(egui::Shape::convex_polygon(
        vec![nose(70.0, 0.0), nose(-40.0, 26.0), nose(-40.0, -26.0)],
        EDGE,
        egui::Stroke::NONE,
    ));
    // Where the wind is on this bit of water, written once at the point of the wedge:
    // her wind is not yours, and that is the half of it nobody guesses.
    //
    // Painted rather than noted: `Table::note` is the explain layer and is off unless
    // somebody turns it on, and this is not a remark about how the chart works — it is
    // the limit of the ship you are leading.
    let out = from + TAU / 2.0;
    let label = t.to_screen(at + glam::DVec2::new(out.sin(), out.cos()) * (REACH * 0.5));
    t.p.text(
        label,
        egui::Align2::CENTER_CENTER,
        format!(
            "she will not lie inside {:.0}° of the wind",
            shut.to_degrees()
        ),
        egui::FontId::monospace(9.0),
        EDGE,
    );
}

/// The anchor, and the water she can swing over.
///
/// **An anchor watch is a bearing and a circle.** The one question at two in the morning
/// is whether she is still where she was put, and the shape that answers it is the
/// circle she can swing in: the anchor at the middle, the chain as a radius, and her own
/// position somewhere on it. Inside the circle is lying to it; outside the circle is
/// dragging, and you do not need a number to see which.
pub(crate) fn where_she_lies(t: &Table, down: &sail_core::anchor::Down, knows: bool) {
    const INK: egui::Color32 = egui::Color32::from_rgb(58, 92, 132);
    let at = t.to_screen(down.at);
    // The swinging circle: what is left of the chain once the depth is taken out of it,
    // which is the furthest from the anchor she can get.
    let depth = (down.depth + 1.4).max(0.5);
    let reach = (down.out * down.out - depth * depth).max(0.0).sqrt();
    if reach > 1.0 {
        t.p.circle_stroke(
            at,
            (reach * t.scale as f64) as f32,
            egui::Stroke::new(1.0, INK.gamma_multiply(0.7)),
        );
    }
    // The anchor itself: a shank, a stock and two flukes, which is the symbol every
    // chart in the world uses for one.
    let arm = 5.0;
    t.p.line_segment(
        [at + egui::vec2(0.0, -arm), at + egui::vec2(0.0, arm)],
        egui::Stroke::new(1.6, INK),
    );
    t.p.line_segment(
        [
            at + egui::vec2(-arm * 0.7, -arm * 0.5),
            at + egui::vec2(arm * 0.7, -arm * 0.5),
        ],
        egui::Stroke::new(1.4, INK),
    );
    for side in [-1.0, 1.0] {
        t.p.line_segment(
            [
                at + egui::vec2(0.0, arm),
                at + egui::vec2(side * arm * 0.8, arm * 0.4),
            ],
            egui::Stroke::new(1.4, INK),
        );
    }
    // What is under her and what she is lying in, written where the anchor is —
    // because the ground is a fact about the place and this is the place.
    //
    // Only when the chart may say where she is: an anchor drawn on the paper is a fix,
    // and a boat that does not know where she is has not earned one.
    if knows {
        t.p.text(
            at + egui::vec2(8.0, 2.0),
            egui::Align2::LEFT_TOP,
            format!("{} {:.0} m", down.ground.charted(), down.depth),
            egui::FontId::monospace(9.0),
            INK,
        );
    }
}

#[cfg(test)]
mod what_the_paper_says_about_her {
    /// **The shading and the ship agree, because they are the same number.**
    ///
    /// The whole use of the wedge is that water inside it is water her master will
    /// refuse; a chart that shaded sixty-five degrees while he complained at seventy-one
    /// would be a chart that gets you a refusal from water it called clear, which is
    /// worse than no chart at all.
    #[test]
    fn the_wedge_is_the_angle_her_master_refuses() {
        use crate::mission::{remark, WILL_NOT_LIE};
        use sail_core::units::rad;
        let deep = 40.0;
        // A hair inside it, and he says so.
        let inside = remark(deep, deep, WILL_NOT_LIE - rad(1.0), 0.0, 400.0, 0.0);
        assert!(
            inside.is_some_and(|said| said.contains("cannot lie")),
            "he accepted a course inside the wedge: {inside:?}"
        );
        // And a hair outside it, and he does not.
        assert_eq!(
            remark(deep, deep, WILL_NOT_LIE + rad(1.0), 0.0, 400.0, 0.0),
            None
        );
        // Both tacks, because a wedge has two sides and a sign error here would shade
        // the water she can sail and clear the water she cannot.
        let other = remark(deep, deep, -(WILL_NOT_LIE - rad(1.0)), 0.0, 400.0, 0.0);
        assert!(other.is_some_and(|said| said.contains("cannot lie")));
    }

    /// And it is wider than the rig's own limit, which is the margin a seaman keeps.
    #[test]
    fn it_is_the_seamans_limit_and_not_the_rigs() {
        let margin = crate::mission::WILL_NOT_LIE - crate::ship::CLOSEST;
        assert!(
            margin > 0.0 && margin < sail_core::units::rad(15.0),
            "the seaman's margin came out at {:.0} degrees",
            margin.to_degrees()
        );
    }
}
