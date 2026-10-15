//! The instruments on the table: the watch, the log, the protractor and the row of
//! them along the top.
//!
//! What they have in common is a *state* — running, streaming, half-laid-down — which
//! is what separates them from the drawing: a line on the paper is finished, and an
//! instrument is in the middle of something.

use bevy_egui::egui;

use super::*;
use crate::sim::Sim;

/// The stopwatch on the chart table.
#[derive(Default, Clone, Copy, Debug)]
pub struct Stopwatch {
    pub running: bool,
    /// When the current run started, in simulation seconds.
    pub since: f64,
    /// And what was on it before that run.
    pub held: f64,
    /// The last whole second that was ticked, so the escapement sounds once a second
    /// rather than once a frame.
    ticked: u32,
}

/// How far the hand goes, seconds.
///
/// **One turn and no more.** A stopwatch whose hand has gone round twice is a stopwatch
/// that has told you nothing — you cannot tell a reading of ten seconds from one of
/// seventy by looking at it, and the whole use of this instrument is reading it at a
/// glance while you watch a light. So it stops itself at the top of the dial rather
/// than carrying on round. Asked for from the deck.
pub const ONE_TURN: f64 = 60.0;

/// When it says so, seconds.
///
/// Five from the end, which is a count of five: long enough to look down, decide and
/// press, and short enough that it is a warning rather than a nuisance. Without it the
/// watch simply stops at the top and the reading you were taking is lost with no idea
/// when it went — which is worse than running on, because at least a hand going round
/// twice is visible.
pub(crate) const WARNS_AT: f64 = 55.0;

impl Stopwatch {
    /// What the face is showing at this moment, capped at one turn.
    pub fn shown(&self, now: f64) -> f64 {
        let run = if self.running {
            (now - self.since).max(0.0)
        } else {
            0.0
        };
        (self.held + run).min(ONE_TURN)
    }
}

/// Stop it at the top of the dial, and say so before it gets there.
///
/// Its own system because the watch runs whether or not the chart is on the table: a
/// navigator starts it, puts the chart away and watches the light, which is the whole
/// point of a thing that counts for you.
pub fn stopwatch(sim: Res<Sim>, mut chart: ResMut<Chart>, sound: Res<crate::sound::SoundHandle>) {
    let now = sim.sim.state.time;
    if !chart.watch.running {
        return;
    }
    let shown = chart.watch.shown(now);
    // **It ticks.**
    //
    // Which is not decoration: a stopwatch you can hear is a stopwatch you do not have
    // to look at, and the whole use of the thing is counting flashes on a dark horizon
    // while the watch runs in your other hand. One tick a second, dry and short, the
    // sound an escapement makes — and it is also how you know it is still going when
    // the chart is away.
    let tick = shown.floor() as u32;
    if tick != chart.watch.ticked {
        chart.watch.ticked = tick;
        // The last five are a countdown and they are a different sound: higher, and a
        // little harder, so a hand knows the end is coming without reading anything.
        if shown >= WARNS_AT {
            sound.0.strike_damped(1_760.0, 0.15, 0.9);
        } else {
            sound.0.strike_damped(2_300.0, 0.045, 0.98);
        }
    }
    if shown >= ONE_TURN {
        chart.watch.running = false;
        chart.watch.held = ONE_TURN;
        chart.watch.since = 0.0;
        // And the end of it, which is a bell rather than a tick: the reading is made
        // and the hand has stopped.
        sound.0.strike_damped(1_180.0, 0.22, 0.75);
    }
}

/// How long a cast of the log runs, seconds.
///
/// **One turn of the hand**, which is also what the watch counts to before it stops
/// itself, and the two numbers being the same one is the point: you throw the log,
/// the hand goes round once, and you stop it at the top.
///
/// This is the hand log as it is actually worked and not a timer with a readout. The
/// chip goes over, the line runs out while a glass empties, and what you read is the
/// line that went out *in that interval* — so the reading is `distance / the interval
/// it was supposed to be`, and if your hand was late the number is wrong by exactly as
/// much as your hand was late. Four hundred years of sailors have measured speed that
/// way and every one of them has been a little out.
pub const CAST: f64 = 60.0;

/// What a timed leg comes to, when the log is hauled in.
///
/// One function because two hands end a leg — the log tool's second click and the
/// stopwatch's STOP — and a leg that measured one thing from one of them and something
/// else from the other would be worse than having only one of them.
///
/// Gives back the line for the paper, the speed for the card, and the set, in that
/// order; `None` for any of them that this leg cannot honestly claim.
#[allow(clippy::type_complexity)]
pub(crate) fn the_leg(
    streaming: Option<(glam::DVec2, f64, f64)>,
    now: f64,
    pos: glam::DVec2,
    through_water: f64,
    knows: bool,
) -> (Option<Plotted>, Option<(f64, f64)>, Option<f64>) {
    let Some((was, when, counter)) = streaming else {
        return (None, None, None);
    };
    let minutes = (now - when) / 60.0;
    // A leg of a few seconds is a slipped finger rather than a measurement: at half a
    // minute the distance run is inside the width of the pencil.
    if minutes <= 0.5 {
        return (None, None, None);
    }
    // **The speed is measured either way; the track only if she knows where she is.**
    //
    // Through the water is what a log counts, and a log counts whether or not anybody
    // knows which water — so a leg timed while lost still gives the one number a
    // reckoning needs. With a position at both ends it gives the other number as well,
    // the speed made good, and the difference between the two is the water moving her.
    let ran = (through_water - counter) / 1_852.0;
    // **Read against the glass, not against the clock.**
    //
    // The speed is the line that ran out divided by the interval the cast was *meant*
    // to be, because that is what a hand log gives you: the number is only as good as
    // the moment you stopped it. Stop a second early and you have read her a knot fast.
    let speed = Some((ran / (CAST / 3_600.0), now));
    if !knows {
        return (None, speed, None);
    }
    let over_ground = (pos - was).length() / 1_852.0;
    (
        Some(Plotted::Run {
            from: was,
            to: pos,
            minutes,
        }),
        speed,
        Some(sail_core::units::kn_to_ms(
            (over_ground - ran).abs() / (minutes / 60.0),
        )),
    )
}

/// The glass, on the screen rather than on the paper.
///
/// **A cast of the log is a thing you do on deck**, and the chart is the place you write
/// the answer down afterwards. So while the line is running the only instrument that
/// matters is a bar emptying in front of you and the tick of the watch — you are looking
/// at the water, not at a chart table — and it is drawn on the same layer as every other
/// thing this game says out loud when it wants a hand at a particular moment.
///
/// It empties rather than fills. A bar that fills says *progress*, which is a thing you
/// wait out; one that empties says *this is running out*, which is a thing you act on,
/// and the act is a hand on the watch at the moment it reaches nothing.
pub fn glass(mut contexts: bevy_egui::EguiContexts, chart: Res<Chart>, sim: Res<Sim>) -> Result {
    use bevy_egui::egui;
    if !chart.watch.running || chart.streaming.is_none() {
        return Ok(());
    }
    let left = (CAST - chart.watch.shown(sim.sim.state.time)).max(0.0);
    let ctx = contexts.ctx_mut()?;
    let screen = ctx.viewport_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("the glass"),
    ));
    let wide = (screen.width() * 0.34).min(420.0);
    let bar = egui::Rect::from_center_size(
        egui::pos2(screen.center().x, screen.bottom() - 232.0),
        egui::vec2(wide, 10.0),
    );
    painter.rect_filled(
        bar,
        2.0,
        egui::Color32::from_rgba_unmultiplied(10, 12, 16, 190),
    );
    let mut run = bar;
    run.set_width(bar.width() * (left / CAST) as f32);
    // Amber while it runs and red for the last five seconds, which are the ones the
    // ticking has already changed its mind about.
    painter.rect_filled(
        run,
        2.0,
        if left <= 5.0 {
            egui::Color32::from_rgb(226, 118, 96)
        } else {
            egui::Color32::from_rgb(232, 176, 58)
        },
    );
    painter.text(
        egui::pos2(bar.center().x, bar.top() - 8.0),
        egui::Align2::CENTER_BOTTOM,
        if left <= 0.05 {
            "STOP HER — now".to_string()
        } else {
            format!("the log is running — stop her in {left:.0}")
        },
        egui::FontId::monospace(if left <= 5.0 { 16.0 } else { 13.0 }),
        if left <= 5.0 {
            egui::Color32::from_rgb(242, 150, 130)
        } else {
            egui::Color32::from_rgb(214, 218, 222)
        },
    );
    Ok(())
}

/// The rose standing on a mark while a bearing is laid off, with the figure under the
/// hand written beside it — which is the thing the parallel rule cannot show you until
/// you have let go of it.
pub(crate) fn protractor(
    t: &Table,
    centre: Option<glam::DVec2>,
    cursor: Option<egui::Pos2>,
    range: f64,
) {
    // The protractor, while a bearing is being laid off: a rose standing on the
    // mark, and the bearing under the cursor written beside it. The reading is
    // the thing that was missing from the old drag — you could not see the
    // angle you were setting until you had let go of it.
    if let Some(centre) = centre {
        let c = t.to_screen(centre);
        let radius = 54.0_f32;
        t.p.circle(
            c,
            radius,
            egui::Color32::TRANSPARENT,
            egui::Stroke::new(1.0, ink::PEN.gamma_multiply(0.7)),
        );
        for tick in 0..72 {
            let a = (tick as f32 * 5.0).to_radians();
            let dir = egui::vec2(a.sin(), -a.cos());
            let long = tick % 6 == 0;
            t.p.line_segment(
                [
                    c + dir * if long { radius - 8.0 } else { radius - 4.0 },
                    c + dir * radius,
                ],
                egui::Stroke::new(if long { 1.0 } else { 0.5 }, ink::PEN),
            );
        }
        for (label, at) in [("N", 0.0_f32), ("E", 90.0), ("S", 180.0), ("W", 270.0)] {
            let a = at.to_radians();
            t.p.text(
                c + egui::vec2(a.sin(), -a.cos()) * (radius - 15.0),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::monospace(9.0),
                ink::PEN,
            );
        }
        t.p.circle_filled(c, 1.6, ink::PEN);
        if let Some(cursor) = cursor {
            let off = t.to_water(cursor) - centre;
            if off.length() > 1.0 {
                let bearing = off.x.atan2(off.y).rem_euclid(std::f64::consts::TAU);
                let far = t.to_screen(centre + off.normalize() * range * 0.9);
                t.p.line_segment([c, far], egui::Stroke::new(1.0, ink::MARK));
                t.p.text(
                    c + egui::vec2(10.0, -radius - 4.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("{:03.0}", bearing.to_degrees()),
                    egui::FontId::monospace(13.0),
                    ink::MARK,
                );
            }
        }
    }
}

/// The stopwatch, where a hand would put one down.
///
/// **It sweeps.** A digital readout alone would be a number on a chart, and the reason a
/// stopwatch has a hand is that counting is easier than reading: timing a light you are
/// watching the sea and the watch at once, and a hand at the top of the dial is a thing
/// you catch out of the corner of your eye while a number is a thing you have to look
/// at. The button under it says what the next press *does* rather than what the watch
/// is, because a control labelled with its own state is a control you read twice.
///
/// Returns whether it was pressed — the watch is an instrument, so what to do about the
/// press belongs to whoever owns the log.
pub(crate) fn the_watch_face(
    t: &Table,
    chart: &Chart,
    now: f64,
    pressed: Option<egui::Pos2>,
) -> bool {
    let on_it = chart.watch.running;
    let shown = chart.watch.shown(now);
    let face = egui::pos2(t.rect.right() - 62.0, t.rect.top() + 62.0);
    t.p.circle(face, 34.0, ink::PAPER, egui::Stroke::new(1.2, ink::COAST));
    // Sixty ticks, long at the fives, which is what the dial of one looks like
    // and is also what makes a half-second readable at a glance.
    for tick in 0..60 {
        let a = (tick as f32 / 60.0) * std::f32::consts::TAU;
        let dir = egui::vec2(a.sin(), -a.cos());
        let long = tick % 5 == 0;
        t.p.line_segment(
            [
                face + dir * if long { 26.0 } else { 29.0 },
                face + dir * 32.0,
            ],
            egui::Stroke::new(if long { 1.1 } else { 0.6 }, ink::COAST),
        );
    }
    let sweep = ((shown % 60.0) / 60.0) as f32 * std::f32::consts::TAU;
    let hand = egui::vec2(sweep.sin(), -sweep.cos());
    t.p.line_segment(
        [face - hand * 6.0, face + hand * 28.0],
        egui::Stroke::new(1.6, if on_it { ink::BOAT } else { ink::PEN }),
    );
    t.p.circle_filled(face, 2.2, ink::PEN);
    // And the minutes, because a light's period is seconds and a leg is minutes
    // and the same watch has to do both.
    t.p.text(
        face + egui::vec2(0.0, 13.0),
        egui::Align2::CENTER_CENTER,
        format!("{}:{:04.1}", (shown / 60.0) as u32, shown % 60.0),
        egui::FontId::monospace(11.0),
        ink::PEN,
    );
    // The button under it, which says what the next press does rather than what
    // the watch is: a control labelled with its own state is a control you read
    // twice.
    let button = egui::Rect::from_center_size(face + egui::vec2(0.0, 48.0), egui::vec2(74.0, 18.0));
    t.p.rect_filled(button, 2.0, if on_it { ink::MARK } else { ink::PAPER });
    t.p.rect_stroke(
        button,
        2.0,
        egui::Stroke::new(1.0, ink::COAST),
        egui::StrokeKind::Inside,
    );
    t.p.text(
        button.center(),
        egui::Align2::CENTER_CENTER,
        if on_it {
            "STOP & LOG"
        } else if shown > 0.05 {
            "ZERO"
        } else {
            "START & LOG"
        },
        egui::FontId::monospace(11.0),
        if on_it { ink::PAPER } else { ink::COAST },
    );
    pressed.is_some_and(|q| button.contains(q) || (q - face).length() < 34.0)
}

/// The row of instruments along the top of the table, and the line that says what the
/// one in your hand is for.
///
/// The hint is half the tool: an instrument with a state and no way of saying which
/// state it is in is an instrument nobody presses twice, which is exactly how the log
/// read from the deck before it said *streamed four minutes ago*.
///
/// Draws them and reports what was picked up; the slots come in rather than being worked
/// out here, because what you can press and what you can see have to be the same
/// rectangles and that is a fact with one owner.
pub(crate) fn the_instruments(
    t: &Table,
    chart: &Chart,
    furniture: &[egui::Rect],
    why_button: egui::Rect,
    streaming: Option<(glam::DVec2, f64, f64)>,
    now: f64,
    pressed: Option<egui::Pos2>,
) -> (Option<Tool>, Option<bool>) {
    let mut pick = None;
    let mut explain = None;
    // What the instrument in hand is for — and, when it is in the middle of
    // doing something, what it is in the middle of. A tool with a state and no
    // way of saying which state it is in is a tool nobody presses twice, which
    // is exactly how the log read from the deck.
    let hint = match (chart.tool, streaming, t.knows) {
        (Tool::Log, Some((_, when, _)), false) => format!(
            "timing the log — {:.0} min, and it gives the speed only",
            (now - when) / 60.0
        ),
        (Tool::Log, None, false) => {
            "time a leg: lost, it gives the speed through the water and no track".to_string()
        }
        (Tool::Log, Some((_, when, _)), true) => format!(
            "log streamed {:.0} min ago — click again at the end of the leg",
            (now - when) / 60.0
        ),
        _ => chart.tool.hint().to_string(),
    };
    t.p.text(
        egui::pos2(t.rect.left() + 24.0, t.rect.top() + 46.0),
        egui::Align2::LEFT_CENTER,
        hint,
        egui::FontId::monospace(11.0),
        ink::COAST,
    );
    // The instruments, along the top of the table where a hand reaches for them.
    for (tool, slot) in TOOLS.iter().zip(furniture) {
        let (tool, slot) = (*tool, *slot);
        let text = tool.name();
        let on = chart.tool == tool;
        t.p.rect_filled(slot, 2.0, if on { ink::MARK } else { ink::PAPER });
        t.p.rect_stroke(
            slot,
            2.0,
            egui::Stroke::new(1.0, ink::COAST),
            egui::StrokeKind::Inside,
        );
        t.p.text(
            slot.center(),
            egui::Align2::CENTER_CENTER,
            text,
            egui::FontId::monospace(10.0),
            ink::COAST,
        );
        if pressed.is_some_and(|at| slot.contains(at)) {
            pick = Some(tool);
        }
    }

    {
        let on = chart.explain;
        t.p.rect_filled(why_button, 2.0, if on { ink::MARK } else { ink::PAPER });
        t.p.rect_stroke(
            why_button,
            2.0,
            egui::Stroke::new(1.0, ink::COAST),
            egui::StrokeKind::Inside,
        );
        t.p.text(
            why_button.center(),
            egui::Align2::CENTER_CENTER,
            "WHY",
            egui::FontId::monospace(10.0),
            ink::COAST,
        );
        if pressed.is_some_and(|at| why_button.contains(at)) {
            explain = Some(!on);
        }
    }
    (pick, explain)
}

/// Start the cast or end it, whichever the watch is doing.
///
/// **Two hands reach for this**: the button on the chart, and `L` on the keyboard —
/// because streaming the log is something you do on deck with the chart put away, and a
/// thing you do on deck in a hurry is a thing that earns a key. One function, so that
/// the key and the button cannot end up doing two different jobs.
pub fn work_the_watch(
    chart: &mut Chart,
    nav: &mut crate::navigation::Navigation,
    log: &mut DeckLog,
    sim: &Sim,
    hour: f32,
    knows: bool,
) {
    let now = sim.sim.state.time;
    let w = chart.watch;
    chart.watch = if w.running {
        // **And the log comes in with it.** Starting a watch and streaming a log
        // are one act on a boat: you are timing a leg, and the two instruments are
        // the two halves of the same measurement. Asked for from the deck after the
        // two were pressed separately once too often — and it is the right way
        // round, because the thing that must not be late is the start, and the
        // stopwatch is what a hand goes to.
        let (line, speed, set) = the_leg(
            chart.streaming,
            now,
            sim.sim.state.pos,
            nav.through_water,
            knows,
        );
        if let Some(line) = line {
            chart.write_down(line, knows);
        }
        if let Some(speed) = speed {
            nav.measured = Some(speed);
            nav.measured_on = Some(sim.sim.state.heading);
            // And in the book, with how good the reading was. A speed with no note of
            // how carefully it was taken is a speed that will be believed exactly as
            // much as a careful one.
            log.write(
                hour,
                Wrote::Speed {
                    knots: speed.0,
                    slip: nav.log_slip,
                },
            );
            // How late the hand was, which is what the reading is worth.
            if let Some((_, when, _)) = chart.streaming {
                nav.log_slip = ((now - when) - CAST).abs() / CAST;
            }
        }
        if let Some(set) = set {
            nav.learn_the_set(set);
        }
        chart.streaming = None;
        Stopwatch {
            running: false,
            since: 0.0,
            held: w.shown(now),
            ticked: w.ticked,
        }
    } else if w.held > 0.05 {
        // Zeroing is putting the instrument away, so the line comes in with it and
        // nothing is written down. A cast whose start is still remembered while the
        // watch reads nothing is a measurement waiting to be made against a time
        // that no longer exists.
        chart.streaming = None;
        Stopwatch::default()
    } else {
        // Streamed at the moment the watch is started, which is what makes the
        // reading afterwards a measurement rather than two things that happened
        // near each other.
        chart.streaming = Some((sim.sim.state.pos, now, nav.through_water));
        Stopwatch {
            running: true,
            since: now,
            held: w.held,
            ticked: 0,
        }
    };
}

/// `L` streams the log and `L` again reads it.
pub fn by_hand(
    keys: Res<ButtonInput<KeyCode>>,
    sim: Res<Sim>,
    sky: Res<crate::sky::Sky>,
    mut chart: ResMut<Chart>,
    mut nav: ResMut<crate::navigation::Navigation>,
    mut log: ResMut<DeckLog>,
) {
    let shifted = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if !keys.just_pressed(KeyCode::KeyL) || shifted {
        return;
    }
    let knows = nav.shows_position(sim.sim.state.time);
    work_the_watch(&mut chart, &mut nav, &mut log, &sim, sky.hour, knows);
}
