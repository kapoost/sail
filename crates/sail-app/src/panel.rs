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

/// The console.
///
/// What this module does in a frame, in one place — see `stages.rs` for why the
/// schedule is named sets rather than one long chain.
pub fn plugin(app: &mut App) {
    use crate::stages::{Build, Paint};
    app.add_systems(Startup, setup.in_set(Build::Boat));
    app.add_systems(
        bevy_egui::EguiPrimaryContextPass,
        draw.in_set(Paint::Instruments),
    );
}

/// Painted metal, cream dial faces, red needles. A deliberately small palette: the
/// old panels were cheap to print and that is exactly why they read so well.
/// The instrument case, and what is painted on it.
///
/// **One palette, and it is a workshop's rather than a screen's.** A japanned case in
/// warm black, brass for the bezels and for whatever is live, ivory for the dial faces
/// and the engraved nameplates, vermilion for a pointer, and one red that means *now*.
/// Nothing here is a grey: every surface on a boat of this age is a material, and
/// materials have a temperature.
pub mod paint {
    use bevy_egui::egui::Color32;
    /// Japanned steel: the case everything is screwed to.
    pub const CASE: Color32 = Color32::from_rgb(30, 29, 27);
    /// A recess milled into it, for anything sunk below the surface.
    pub const TRACK: Color32 = Color32::from_rgb(20, 19, 18);
    /// Dulled brass, for rims and frames.
    pub const BEZEL: Color32 = Color32::from_rgb(78, 68, 50);
    /// Bright brass: what is selected, engaged, or working.
    pub const LIVE: Color32 = Color32::from_rgb(198, 158, 82);
    /// Ivory, for a dial face and for the figures on it.
    pub const FACE: Color32 = Color32::from_rgb(228, 222, 205);
    /// The nameplate a label is engraved into.
    pub const PLATE: Color32 = Color32::from_rgb(198, 190, 172);
    /// And the engraving itself.
    pub const INK: Color32 = Color32::from_rgb(28, 27, 25);
    /// A pointer, in the red every instrument maker used because it is the one colour
    /// the eye finds against ivory without being told to look.
    pub const NEEDLE: Color32 = Color32::from_rgb(174, 46, 34);
    /// A label stencilled straight onto the case rather than engraved on a plate.
    pub const LABEL: Color32 = Color32::from_rgb(148, 142, 128);
    pub const GOOD: Color32 = Color32::from_rgb(126, 178, 128);
    /// For the things you want seen across a cockpit in a hurry: overdue, aground.
    pub const BAD: Color32 = Color32::from_rgb(214, 96, 74);
}

/// The type scale, with the roles named.
///
/// **Four sizes and each one means something.** An instrument panel drawn in nine point
/// everywhere is a wall of text with the important numbers hidden in it — which is
/// exactly what this one was, and what it was reported as. The sizes here are a
/// hierarchy you can read from across a cockpit: the three figures you steer by, the
/// ones you check, the names of things, and the units, which should never be the size of
/// the number they belong to.
pub mod letter {
    /// The three you steer by.
    pub const PRIMARY: f32 = 30.0;
    /// Anything you look at on purpose.
    pub const VALUE: f32 = 17.0;
    /// What it is called.
    pub const LABEL: f32 = 12.0;
    /// And what it is in. Always smaller than the figure, always dimmer.
    pub const UNIT: f32 = 11.0;
}

/// A name engraved on an ivory plate and screwed to the case.
///
/// The signature of every instrument panel of the period and worth having for more than
/// the look: a dark label on a light plate is read as a *heading* without anybody having
/// to be told, which is how a dozen controls become four groups. Letter-spaced, because
/// engraving is.
pub fn plate(ui: &mut egui::Ui, name: &str) {
    let spaced: String = name
        .chars()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let galley = ui.painter().layout_no_wrap(
        spaced.clone(),
        egui::FontId::monospace(letter::LABEL),
        paint::INK,
    );
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(galley.size().x + 10.0, galley.size().y + 4.0),
        egui::Sense::hover(),
    );
    ui.painter().rect_filled(rect, 1.0, paint::PLATE);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        spaced,
        egui::FontId::monospace(letter::LABEL),
        paint::INK,
    );
}

/// The figures she is steered by, in the size they deserve.
///
/// **Aviation's one good idea about instruments**: the handful of numbers a pilot acts
/// on are large, in a fixed place, and in a fixed order, so that reading them is not
/// *finding* them. On a yacht that is her speed, her head and the wind she is in —
/// everything else on this panel is a thing you look up, and these three are things you
#[derive(Resource)]
pub struct Console {
    /// Hidden with H, for a clean look at the water.
    pub hidden: bool,
    /// Whether the rack of control lines is folded out.
    ///
    /// Open by default and on every camera: the winch is in the cockpit, and the
    /// cockpit is where you are standing whichever way the camera happens to be
    /// pointing. Folded away when the foredeck wants the room.
    pub rack: bool,
}

impl Default for Console {
    fn default() -> Self {
        Self {
            hidden: false,
            rack: true,
        }
    }
}

/// The autopilot, beside the glass.
///
/// **Where the hand already is.** It had a page of its own, which meant the one control
/// you reach for while something is going wrong was behind a tab you were not on — and
/// an autopilot is reached for in exactly those moments. It is four buttons and two
/// lines of type; it fits beside the card, where the course it is holding can be read
/// against the course she is making.
fn the_pilot(ui: &mut egui::Ui, pilot: &mut Pilot, sim: &Sim) {
    let telemetry = sim.sim.telemetry;
    // Why it last let go, for half a minute after it did — and the vane saying it is
    // beaten, which it has to do here because there is no alarm to sound and no relay to
    // click. **One line of space, always**: a line that appears in the middle of a
    // column of buttons moves every button under it, including the one the mouse is on
    // its way to.
    let note = if pilot.engaged() {
        if pilot.struggling() && !pilot.can_let_go() {
            Some("the vane is not coping")
        } else {
            None
        }
    } else {
        pilot.last_reason.filter(|_| pilot.since_reason < 30.0)
    };
    // The same margin for every row: it is a column, and the things in a column line
    // up with each other.
    let row = |ui: &mut egui::Ui| {
        ui.add_space(1.0);
    };
    ui.horizontal(|ui| {
        row(ui);
        plate(ui, "PILOT");
    });
    ui.horizontal(|ui| {
        row(ui);
        ui.label(
            egui::RichText::new(fit(note.unwrap_or(" "), 24))
                .color(if note.is_some() {
                    paint::BAD
                } else {
                    egui::Color32::TRANSPARENT
                })
                .monospace()
                .size(letter::UNIT),
        );
    });
    let engaged = pilot.engaged();
    ui.horizontal(|ui| {
        row(ui);
        if lamp_button(ui, if engaged { "ENGAGED" } else { "STANDBY" }, engaged).clicked() {
            if engaged {
                pilot.disengage();
            } else {
                let state = sim.sim.state;
                pilot.hold_heading(&state);
            }
        }
    });
    let wind_mode = matches!(pilot.mode, PilotMode::ApparentWind { .. });
    ui.horizontal(|ui| {
        row(ui);
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
    });
    // What it is actually holding. Two lamps told you *which* instrument was steering
    // and the arrows moved its target, and nowhere was the target itself — so the one
    // question a person has about an autopilot, *what is it trying to do*, had to be
    // answered by watching the compass and guessing.
    ui.horizontal(|ui| {
        row(ui);
        ui.label(
            egui::RichText::new(match pilot.mode {
                PilotMode::Standby => "   —".to_string(),
                PilotMode::Heading(h) => format!("{:03.0}", deg(h)),
                PilotMode::ApparentWind { awa: a, .. } => format!("{:+.0} awa", deg(a)),
            })
            .monospace()
            .size(letter::VALUE)
            .color(if pilot.engaged() {
                paint::LIVE
            } else {
                egui::Color32::from_rgb(110, 115, 120)
            }),
        );
    });
    // The same two steps the keys use, from the same constants, and the doubled arrow is
    // the coarse one in every piece of marine electronics ever built.
    ui.horizontal(|ui| {
        row(ui);
        ui.spacing_mut().item_spacing.x = 2.0;
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
}

/// **The bowl's own measurements**, so that everything fitted round it asks instead of
/// guessing. The case is square and the glass is round, and the corners of steel either
/// side of it are where the small controls go — which only works if they know where the
/// curve actually is.
/// **The box the glass is allocated**, with room in it for the whole instrument.
///
/// It was made narrower than the bowl on purpose once, so that the columns beside it
/// could tuck into the corners — and that was right while the glass stood in the middle
/// of the bar with things either side of it. Screwed into the corner of the case on its
/// own, the same trick clipped the bowl against the edge of its own frame: the laylines,
/// the wind barb and half the card went over the side. Nothing is beside it now, so it
/// gets the room it needs.
pub const GLASS: egui::Vec2 = egui::vec2(214.0, 282.0);
/// How far below the middle of its box the bowl sits: the lubber window is above it.
pub const GLASS_DROP: f32 = 18.0;
/// And the rim.
pub const GLASS_R: f32 = 96.0;

/// How far the bowl reaches out at a given height — the thing text does round a picture.
///
/// Asked for from the deck as *flowing round it like a word processor*, and it is the
/// right instinct: a straight column beside a circle leaves a crescent of empty case
/// that belongs to neither, while a column that follows the curve puts every button as
/// How far the bowl reaches out at a given height.
///
/// What text does round a picture: a straight column beside a circle leaves a crescent
/// of case belonging to neither, and a column that follows the curve puts every button
/// What the panel needs to know about a rope to draw its clutch.
///
/// A parcel rather than eight arguments: a list that long is a list you get wrong in the
/// order, and these five are one thing anyway — the state of one line on the coachroof.
pub struct Rope<'a> {
    pub selected: bool,
    pub stops: Option<&'a [f64]>,
    pub position: Option<&'a str>,
    /// What the two buttons will do, when the line has positions: the name of the stop
    /// each of them moves her to, or nothing when there is no further to go that way.
    ///
    /// A button that says HAUL on a halyard tells nobody whether the sail is going up or
    /// coming down. A button that says SET does.
    pub to: (Option<&'a str>, Option<&'a str>),
    /// How fast it comes in, and how long this frame was.
    pub rate: f64,
    pub dt: f64,
}

/// A narrow brass tab down the edge of a panel, for folding it away.
///
/// The letters run down it, which is what a tab on the edge of a locker does — and it is
/// the one control on this console that is about the console rather than about the boat,
/// so it is the size of a thumbnail and the colour of the case.
fn tab_edge(ui: &mut egui::Ui, open: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(16.0, 108.0), egui::Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, 2.0, if open { paint::BEZEL } else { paint::LIVE });
    p.rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.0, paint::BEZEL),
        egui::StrokeKind::Inside,
    );
    // Three knurls down it and no legend. A tab on the edge of a locker does not say
    // *locker*, and seven plates with rope names engraved on them do not need a plate
    // over them saying CLUTCHES: a rack is recognised by being a rack.
    for knurl in 0..3 {
        let y = rect.center().y + (knurl as f32 - 1.0) * 9.0;
        p.line_segment(
            [
                egui::pos2(rect.left() + 4.0, y),
                egui::pos2(rect.right() - 4.0, y),
            ],
            egui::Stroke::new(1.2, if open { paint::FACE } else { paint::INK }),
        );
    }
    response.clicked()
}

/// Everything the glass shows, in one parcel.
pub struct Glass {
    pub heading: f64,
    pub heel: f64,
    /// Where the wind is from, and how close she can lie to it.
    pub twd: f64,
    pub beat: f64,
    /// Where she is bound, if she is bound anywhere.
    pub mark: Option<f64>,
    /// The wind she is in: apparent on the needle, true behind it.
    pub awa: f64,
    pub aws: f64,
    pub twa: f64,
}

fn binnacle(ui: &mut egui::Ui, g: &Glass) {
    // Taller than it is wide, because the lubber window sits above the bowl and a
    // square box put it outside the case altogether — over whatever happened to be
    // behind the bar.
    let (rect, _) = ui.allocate_exact_size(GLASS, egui::Sense::hover());
    let p = ui.painter();
    let c = rect.center() + egui::vec2(0.0, GLASS_DROP);
    let r = GLASS_R;
    // Where a bearing sits on the glass: her head is at the top, so everything else is
    // relative to it — which is what a card does when the boat turns under it.
    let screen = |bearing: f64| -> f32 { (bearing - g.heading) as f32 };
    let towards = |a: f32, len: f32| c + egui::vec2(a.sin(), -a.cos()) * len;

    p.circle_filled(c, r + 13.0, paint::BEZEL);
    p.circle_filled(c, r + 11.0, paint::CASE);
    p.circle_filled(c, r, paint::FACE);

    // --- The bezel: the wind, and what she can lay on either tack ------------------
    //
    // The shaded arc is the water she cannot sail into. Its edges are the two tacks,
    // and they carry the headings in figures, because *that* is the number you steer.
    let wind = screen(g.twd);
    let steps = 48;
    for i in 0..steps {
        let a0 = wind - g.beat as f32 + (i as f32 / steps as f32) * 2.0 * g.beat as f32;
        let a1 = wind - g.beat as f32 + ((i + 1) as f32 / steps as f32) * 2.0 * g.beat as f32;
        p.line_segment(
            [towards(a0, r + 6.0), towards(a1, r + 6.0)],
            egui::Stroke::new(9.0, paint::TRACK),
        );
    }
    for side in [-1.0_f32, 1.0] {
        let a = wind + side * g.beat as f32;
        p.line_segment(
            [towards(a, r - 10.0), towards(a, r + 12.0)],
            egui::Stroke::new(2.0, paint::LIVE),
        );
        let lay = (g.twd + side as f64 * g.beat)
            .to_degrees()
            .rem_euclid(360.0);
        p.text(
            towards(a, r + 26.0),
            egui::Align2::CENTER_CENTER,
            format!("{lay:03.0}"),
            egui::FontId::monospace(letter::UNIT),
            paint::LIVE,
        );
    }
    // The wind itself, with a barb on it, pointing the way it blows: into the glass.
    let at = towards(wind, r + 6.0);
    let along = (towards(wind, 0.0) - at).normalized();
    let across = egui::vec2(-along.y, along.x);
    p.line_segment(
        [at, at + along * 16.0],
        egui::Stroke::new(2.4, paint::NEEDLE),
    );
    for side in [-1.0, 1.0] {
        p.line_segment(
            [at + along * 16.0, at + along * 9.0 + across * 5.0 * side],
            egui::Stroke::new(2.0, paint::NEEDLE),
        );
    }

    // --- The upper half: the card, seen from above ---------------------------------
    // Clipped to the *bowl*, not to the box it was allocated: the box is narrower than
    // the glass on purpose, so that the columns beside it can tuck into the corners —
    // and clipping to it cut the card and the sea off at the shoulders.
    let bowl = egui::Rect::from_center_size(c, egui::vec2(r * 2.0 + 40.0, r * 2.0 + 40.0));
    let card = p.with_clip_rect(egui::Rect::from_min_max(
        bowl.left_top(),
        egui::pos2(bowl.right(), c.y),
    ));
    for ten in 0..36 {
        let bearing = sail_core::units::rad(ten as f64 * 10.0);
        let a = screen(bearing);
        let long = ten % 3 == 0;
        card.line_segment(
            [
                towards(a, r - if long { 13.0 } else { 7.0 }),
                towards(a, r - 1.0),
            ],
            egui::Stroke::new(if long { 1.6 } else { 1.0 }, paint::INK),
        );
        if long {
            card.text(
                towards(a, r - 34.0),
                egui::Align2::CENTER_CENTER,
                match ten {
                    0 => "N".to_string(),
                    9 => "E".to_string(),
                    18 => "S".to_string(),
                    27 => "W".to_string(),
                    _ => format!("{}", ten * 10),
                },
                egui::FontId::monospace(letter::UNIT),
                paint::INK,
            );
        }
    }
    // Where she is bound, when she is bound anywhere.
    if let Some(mark) = g.mark {
        let a = screen(mark);
        card.add(egui::Shape::convex_polygon(
            vec![
                towards(a, r - 2.0),
                towards(a + 0.05, r + 9.0),
                towards(a - 0.05, r + 9.0),
            ],
            paint::LIVE,
            egui::Stroke::NONE,
        ));
    }
    // The lubber line: the boat herself, fixed, with the card turning under her.
    p.line_segment(
        [towards(0.0, r - 22.0), towards(0.0, r + 11.0)],
        egui::Stroke::new(2.4, paint::NEEDLE),
    );
    // **In a window at the lubber line**, which is where a repeater puts it and the
    // only place on this glass that cannot collide with anything: the card's own
    // figures turn under it, and twice now the heading has landed on top of one.
    let window =
        egui::Rect::from_center_size(c + egui::vec2(0.0, -(r + 24.0)), egui::vec2(66.0, 26.0));
    p.rect_filled(window, 2.0, paint::FACE);
    p.rect_stroke(
        window,
        2.0,
        egui::Stroke::new(1.0, paint::BEZEL),
        egui::StrokeKind::Inside,
    );
    p.text(
        window.center(),
        egui::Align2::CENTER_CENTER,
        format!("{:03.0}", g.heading.to_degrees().rem_euclid(360.0)),
        egui::FontId::monospace(letter::VALUE + 3.0),
        paint::INK,
    );

    // --- The lower half: the sea, standing still -----------------------------------
    let sea = p.with_clip_rect(egui::Rect::from_min_max(
        egui::pos2(bowl.left(), c.y),
        bowl.right_bottom(),
    ));
    let tilt = -(g.heel as f32);
    let along = egui::vec2(tilt.cos(), tilt.sin());
    // The sea is a *segment of the glass*, not a rectangle laid over it: cut as the
    // chord through the middle and the arc under it, so that however far she lies over,
    // the water stays inside the bowl. A rectangle clipped to the lower half looked
    // right at two degrees and came out of the instrument at twenty.
    // The arc alone, from one end of the horizon round to the other: the chord closes
    // it, and adding the two ends by hand made a figure that crossed itself and drew
    // nothing at all. Sweeping from the horizon's own direction is the half under it,
    // because screen y runs down — the other way round would be the sky.
    let from = along.angle();
    let edge: Vec<egui::Pos2> = (0..=36)
        .map(|i| {
            let a = from + std::f32::consts::PI * i as f32 / 36.0;
            c + egui::vec2(a.cos(), a.sin()) * r
        })
        .collect();
    sea.add(egui::Shape::convex_polygon(
        edge,
        paint::TRACK,
        egui::Stroke::NONE,
    ));
    sea.line_segment(
        [c - along * r, c + along * r],
        egui::Stroke::new(2.0, paint::NEEDLE),
    );
    // Degrees down the rim, so the lean is a number as well as a picture.
    for mark in [10.0_f32, 20.0, 30.0, 40.0] {
        for side in [-1.0_f32, 1.0] {
            let a = std::f32::consts::PI - side * mark.to_radians();
            sea.line_segment(
                [towards(a, r - 10.0), towards(a, r - 1.0)],
                egui::Stroke::new(1.0, paint::FACE),
            );
            sea.text(
                towards(a, r - 20.0),
                egui::Align2::CENTER_CENTER,
                format!("{mark:.0}"),
                egui::FontId::monospace(letter::UNIT),
                paint::FACE,
            );
        }
    }
    p.text(
        c + egui::vec2(0.0, r * 0.74),
        egui::Align2::CENTER_CENTER,
        format!("{:.0}°", g.heel.to_degrees().abs()),
        egui::FontId::monospace(letter::VALUE),
        paint::FACE,
    );

    // --- The boss in the middle: the wind she is actually in ----------------------
    //
    // A binnacle has a pillar through the middle of it, and the instrument that belongs
    // on top of it is the one you trim by. The big card is where she is *going*; this is
    // the wind arriving, apparent, with the true wind behind it — the two angles a
    // trimmer holds in his head, in the middle of the glass he is already looking at.
    let boss = 44.0_f32;
    p.circle_filled(c, boss + 3.0, paint::BEZEL);
    p.circle_filled(c, boss, paint::FACE);
    for ten in 0..12 {
        let a = ten as f32 * std::f32::consts::TAU / 12.0;
        let dir = egui::vec2(a.sin(), -a.cos());
        p.line_segment(
            [c + dir * (boss - 6.0), c + dir * (boss - 1.0)],
            egui::Stroke::new(if ten == 0 { 1.8 } else { 1.0 }, paint::INK),
        );
    }
    // The true wind first and thinner, because it is the one you plan with; the
    // apparent over it, because it is the one the sails are in.
    for (angle, width, colour) in [
        (g.twa as f32, 1.6, paint::INK.gamma_multiply(0.55)),
        (g.awa as f32, 2.6, paint::NEEDLE),
    ] {
        let dir = egui::vec2(angle.sin(), -angle.cos());
        p.line_segment(
            [c - dir * 6.0, c + dir * (boss - 9.0)],
            egui::Stroke::new(width, colour),
        );
    }
    p.circle_filled(c, 3.0, paint::BEZEL);
    p.text(
        c + egui::vec2(0.0, boss * 0.45),
        egui::Align2::CENTER_CENTER,
        format!("{:+.0}°", deg(g.awa)),
        egui::FontId::monospace(letter::VALUE),
        paint::INK,
    );
    p.text(
        c + egui::vec2(0.0, boss * 0.45 + 14.0),
        egui::Align2::CENTER_CENTER,
        format!("{:.0} kn", ms_to_kn(g.aws)),
        egui::FontId::monospace(letter::UNIT),
        paint::INK.gamma_multiply(0.75),
    );

    // --- And a word at each thing on the rim, small enough to ignore ----------------
    //
    // An instrument that needs a legend is badly drawn, and one that *has* no legend
    // anywhere is one you learn by being told. These are at the size of an engraver's
    // afterthought: there when you look for them, invisible when you are sailing.
    let hint = |a: f32, what: &str| {
        p.text(
            towards(a, r + 26.0),
            egui::Align2::CENTER_CENTER,
            what,
            egui::FontId::monospace(letter::UNIT - 2.0),
            paint::LABEL.gamma_multiply(0.7),
        );
    };
    hint(wind, "wind");
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
    mut course: ResMut<Course>,
    nav: Res<crate::navigation::Navigation>,
    polar: Res<crate::hud::Polar>,
    mut in_hand: InHand,
    mut crew: ResMut<crate::crew::Crew>,
    flotsam: Res<crate::barrels::Flotsam>,
    mut orders: ResMut<crate::orders::Orders>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    // How long the screen has been up, so it can surface rather than appear.
    mut dying: Local<f32>,
    // And how long she has been stopped head to wind, which is a state rather than a
    // moment: every tack goes through it and only one tack in twenty stays there.
    mut irons_for: Local<f32>,
) -> Result {
    if keys.just_pressed(KeyCode::KeyH) {
        console.hidden = !console.hidden;
    }
    if console.hidden {
        return Ok(());
    }
    // Disjoint fields of one parameter, taken apart once: a Bevy system may have
    // sixteen parameters and this one had seventeen, which is a limit worth meeting
    // with a *name* rather than with a fight — the rod and the glasses are both
    // things in your hands, and now they say so.
    let (fishing, sighting) = (&mut in_hand.fishing, &mut in_hand.sighting);
    let ctx = contexts.ctx_mut()?;
    let cfg = sim.config();
    let telemetry = sim.sim.telemetry;
    // Whether the instruments may work out anything that needs a position.
    let knows = nav.shows_position(sim.sim.state.time);
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
                // Four points between rows rather than six. Over a dozen rows that is a
                // readout's worth of sea given back, and nothing in here is close enough
                // to anything else to be misread for it.
                ui.spacing_mut().item_spacing.y = 4.0;
                // **The water and the glass, back in the window on the left.**
                //
                // They went to the corner with the rest of the readings and they are the
                // wrong sort of reading for it: the corner is what she is doing this
                // minute — her speed, the wind, what the pilot is holding — and these
                // four are what the day is doing. Which is the question the destination
                // in this window is already asking.
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    gauge(ui, "SEA", telemetry.significant_height, 0.0, 4.0, "m");
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
                let (glass, tendency) = sim.sim.wind.barometer();
                readout(
                    ui,
                    "BARO",
                    &format!("{glass:.0} {}", tendency_words(tendency)),
                );
                let (building, turning) = sim.sim.wind.outlook();
                readout(ui, "OUTLOOK", &fit(&outlook_words(building, turning), 16));

                // **The three she is steered by**, in the size they deserve, in a
                // fixed order, in a fixed place. Everything under them is a thing you
                // look up; these are things you glance at a hundred times an hour.
                // Which belt she is in, when the world has belts. Words rather than a
                // number, because "the trades" is a thing you plan a passage round and
                // a phase angle is not — and only when there are any, like the squall
                // line above it: an instrument that says the same thing all day is an
                // instrument nobody reads.
                if let Some(belt) = sim.sim.wind.band_at(sim.sim.state.pos) {
                    readout(ui, "BELT", belt);
                }
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
                // **And the state nobody recognises from inside it.**
                //
                // Reported from the deck in a gale: head to wind, a knot and a half of
                // way on, the helm hard over, neither the pilot nor the vane able to do
                // anything, and the question *what is blocking me*. Nothing was: she was
                // in irons, which is a place a boat gets to and does not leave by
                // steering, and the one thing on the screen about it — the helm reading
                // one per cent and the word "stalled" — is a symptom rather than a name.
                //
                // Measured on the bench afterwards (`sail-core/tests/in_irons.rs`): with
                // the main sheeted in she sits twenty degrees off the wind whatever the
                // helm does, and with it eased right out or a sail backed she is a
                // hundred degrees off inside two minutes. So the instrument says the
                // name of the state and the two things that end it, and nothing else —
                // it is not a tutorial, it is the word a crew says out loud.
                if in_irons(&telemetry) {
                    *irons_for += time.delta_secs();
                } else {
                    *irons_for = 0.0;
                }
                // Six seconds, because every tack passes through this and a tack is over
                // in four. A boat that is *still* here after six is not tacking.
                if *irons_for > 6.0 {
                    ui.label(
                        egui::RichText::new("IN IRONS")
                            .color(paint::BAD)
                            .monospace()
                            .size(12.0),
                    );
                    ui.label(
                        egui::RichText::new("main right out, back the headsail (G)")
                            .color(paint::LIVE)
                            .monospace()
                            .size(10.0),
                    );
                }

                // And the gear over the stern, when there is any. Here rather than in a
                // corner of its own: line out is a reading you glance at continuously
                // while you are fishing, which makes it an instrument and not a window.
                crate::rod::gauge(ui, fishing, &sim);

                // **The three things you pick up**, which until now were keys and
                // nothing else.
                //
                // Everything on this console is either a reading or a rope, and these
                // are neither: the glasses, the compass and the rod are *in your hands*,
                // and a hand is the one thing a keyboard hides. Somebody who has not
                // read the key list has no way of discovering that the boat has
                // binoculars aboard — and a feature nobody finds is a feature that is
                // not there. They sit under the weather and above the consignment,
                // because that is where the question *what can I do about it* belongs,
                // and they light when the thing is in hand.
                //
                // What they do not do is *work* the instrument: naming a mark, shooting
                // a bearing and taking a fish are timing, and timing lives on the keys
                // the hover text names. These put the thing in your hand and take it
                // away again, which is the half that was undiscoverable.
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    if tool_button(ui, "GLASSES", sighting.glasses)
                        .on_hover_text(
                            "the glasses, for naming what is out there — shift B, then \
                             B to ask what it is",
                        )
                        .clicked()
                    {
                        sighting.glasses = !sighting.glasses;
                        if sighting.glasses {
                            // One at a time: you have two hands and one of them is
                            // holding on. The same rule the keys follow.
                            sighting.up = false;
                        }
                        sighting.on = None;
                    }
                    if tool_button(ui, "BEARING", sighting.up)
                        .on_hover_text(
                            "the hand bearing compass — shift N, aim with the right \
                             button, N to shoot it",
                        )
                        .clicked()
                    {
                        sighting.up = !sighting.up;
                        if sighting.up {
                            sighting.glasses = false;
                        }
                        sighting.on = None;
                    }
                    let aboard = fishing.have_a_rod();
                    if ui
                        .add_enabled_ui(aboard, |ui| tool_button(ui, "ROD", fishing.out()))
                        .inner
                        .on_hover_text(if aboard {
                            "the rod over the stern — J, and J again to take her aboard \
                             when there is one alongside"
                        } else {
                            "no rod aboard"
                        })
                        .clicked()
                    {
                        let said = fishing.stow_or_set();
                        info!("{said}");
                    }
                });

                ui.add_space(4.0);
                ui.separator();
                let (distance, bearing) = course.to_target(sim.sim.state.pos);
                // And whether there is anywhere to be bound for, which in open ocean
                // there is not. `target` falls back to the world origin when the harbour
                // list is empty, so this card was reporting a range and a bearing to the
                // middle of a sea with nothing in it.
                let knows = knows && course.bound_somewhere();
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
                readout_pair(
                    ui,
                    // **Only what she can know.**
                    //
                    // A range and a bearing to a harbour are worked out *from her
                    // position*, so a boat that does not know where she is cannot have
                    // them — and this panel was handing both over in figures while the
                    // chart beside it refused to draw her. That is the whole exercise
                    // given away by an instrument that was never asked whether it
                    // should be speaking. Reported from the deck in one line: do not
                    // show what the player does not know.
                    //
                    // The name of the place stays. Where you are bound is a thing you
                    // decided in harbour and have not forgotten.
                    (
                        "RANGE",
                        &if knows {
                            format!("{:.2} nm", distance / 1852.0)
                        } else {
                            "—".to_string()
                        },
                    ),
                    (
                        "BRG",
                        &if knows {
                            format!("{:03.0}", deg(bearing))
                        } else {
                            "—".to_string()
                        },
                    ),
                );
                readout_pair(
                    ui,
                    (
                        if overdue { "OVERDUE" } else { "LEFT" },
                        &clock(course.remaining().abs()),
                    ),
                    ("DONE", &format!("{}", course.delivered)),
                );
                // **Who is in, and in what.**
                //
                // The whole reason a leg is handed out to everybody is so that there is
                // something to compare at the end of it, and a time nobody can see is
                // not a comparison. Newest first, and only while there is a shared leg:
                // on your own this is a line that would never have anything in it.
                if !in_hand.net.finished.is_empty() {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("IN")
                            .color(paint::LABEL)
                            .monospace()
                            .size(9.0),
                    );
                    for (_, who, seconds) in in_hand.net.finished.iter().take(4) {
                        ui.label(
                            egui::RichText::new(format!(
                                "{:<10} {}",
                                who.chars().take(10).collect::<String>(),
                                clock(*seconds as f64)
                            ))
                            .color(paint::LIVE)
                            .monospace()
                            .size(11.0),
                        );
                    }
                }
                // **The casks, but only once there are any.** A counter that reads zero
                // all passage is a line of instrument saying nothing, and this panel is
                // already the one thing on screen that cannot be folded away. What is on
                // the painter is the number that matters while you are collecting them —
                // she tows slower with every one — and what is ashore is the score.
                if !flotsam.astern.is_empty() || flotsam.landed > 0 {
                    readout_pair(
                        ui,
                        ("IN TOW", &format!("{}", flotsam.astern.len())),
                        ("ASHORE", &format!("{}", flotsam.landed)),
                    );
                }
            });
        });

    // --- One strip along the bottom: the sheets, the lines, the glass ---------------
    //
    // **In that order and in one row.** They were three areas anchored separately and
    // they overlapped the moment any of them changed size — reported from the deck with
    // a picture of the rack behind the sheets. Laid out as one row they cannot: each
    // knows where the last one finished, which is the only thing that ever keeps two
    // panels apart.
    //
    // Anchored on the right, because the glass is the thing that must not move: it is
    // the instrument, the others are tools beside it.
    // --- The corner: the glass, and the sheets under it -----------------------------
    //
    // **The instrument above and the ropes below**, in one column in the corner where
    // the hand rests. Asked for from the deck, and it is the arrangement of a real
    // binnacle pedestal: the card at eye height on top of the pillar and the gear you
    // work at the bottom of it.
    egui::Area::new("corner".into())
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-10.0, -10.0))
        .interactable(true)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                frame.show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        // **What is not already on the glass.**
                        //
                        // Her head is the card, the apparent wind is the needle on the
                        // boss, and her heel is the line across the bottom half of the
                        // bowl: three of the readings that used to be printed in the
                        // corner of the screen are now *drawn*, and printing them again
                        // beside the drawing is the oldest way to make an instrument
                        // panel unreadable. What is left is everything the glass cannot
                        // show — the log, the wind's strength, what she is making good,
                        // the water under her and what the glass is doing.
                        ui.vertical(|ui| {
                            // As wide as its widest reading and no wider: the column was
                            // set to a number and the number was larger than the type,
                            // so there was a hand's breadth of empty case down the
                            // middle of the instrument. The case now comes out about the
                            // width of the folded chart, which is the other thing in a
                            // corner of this screen and the one to match.
                            ui.set_width(118.0);
                            // Tight, because the column has to finish inside the same case
                            // as the bowl beside it: at four points between rows the last
                            // line went over the bottom edge.
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.label(
                                egui::RichText::new(format!("{:.1}", ms_to_kn(telemetry.stw)))
                                    .color(paint::FACE)
                                    .monospace()
                                    .size(letter::PRIMARY),
                            );
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 3.0;
                                ui.label(
                                    egui::RichText::new("LOG")
                                        .color(paint::LABEL)
                                        .monospace()
                                        .size(letter::LABEL),
                                );
                                ui.label(
                                    egui::RichText::new("kn")
                                        .color(paint::LABEL.gamma_multiply(0.75))
                                        .monospace()
                                        .size(letter::UNIT),
                                );
                            });
                            readout(ui, "AWS", &format!("{:.1} kn", ms_to_kn(telemetry.aws)));
                            readout(ui, "TWS", &format!("{:.0} kn", ms_to_kn(telemetry.tws)));
                            readout(ui, "TWA", &format!("{:+.0}°", deg(telemetry.twa)));
                            readout(ui, "VMG", &format!("{:.1} kn", ms_to_kn(telemetry.vmg)));
                            // **The pilot among the figures**, which is where it belongs: what
                            // it is holding reads against the wind above it and the water
                            // below. Two columns and a glass made a corner wider than it had
                            // any right to be; one column makes it as narrow as the bowl.
                            ui.add_space(7.0);
                            the_pilot(ui, &mut pilot, &sim);
                            ui.add_space(7.0);
                        });
                        // **The pilot belongs beside the glass.** The course it is holding
                        // is read against the course she is making, which is one glance and
                        // not two — and an autopilot is reached for from wherever you happen
                        // to be looking, so it follows the glass into the corner rather than
                        // staying on a bar that changes with the view.
                        ui.vertical(|ui| {
                            binnacle(
                                ui,
                                &Glass {
                                    heading,
                                    heel: telemetry.heel,
                                    twd: telemetry.twd,
                                    beat: polar.beat(),
                                    mark: match course.leg {
                                        Leg::Carrying => {
                                            Some(course.to_target(sim.sim.state.pos).1)
                                        }
                                        Leg::Idle => None,
                                    },
                                    awa: telemetry.awa,
                                    aws: telemetry.aws,
                                    twa: telemetry.twa,
                                },
                            );
                            // The blade and what it has left, under the glass: it lost its
                            // own dial when the wind took the middle, and it is two figures
                            // — not an instrument, but not nothing either.
                            ui.horizontal(|ui| {
                                ui.add_space(52.0);
                                readout(ui, "HELM", &format!("{:+.0}°", deg(sim.controls.rudder)));
                                // The words come back when there is something to say: a helm
                                // that has gone soft for want of way wants the opposite
                                // response to one whose blade has stalled.
                                // Held to a fixed width. The column is as wide as its
                                // widest block, so a reason that grew by a word pushed
                                // the whole binnacle out and the sea with it — the
                                // instrument moving because the helm went soft.
                                readout(ui, "BITE", &fit(&bite_words(&telemetry), BITE_COLUMNS));
                            });
                        });
                    });
                });
                // **The same width as the glass over it, so both end at the same edge.**
                //
                // The column is as wide as its widest block and a narrower one in it
                // leaves a strip of sea down the right of the screen between the two —
                // reported from the deck as the sheets not lining up with the
                // instruments above them. Right-aligning the row instead made the area
                // claim the whole window and paint a black wall across it, which is what
                // `Align::Max` does in a container with no width to align inside of.
                let case_width = ui.min_rect().width();
                    frame.show(ui, |ui| {
                    ui.set_min_width(case_width - 16.0);
                    ui.horizontal_top(|ui| {
                        // Sized to its contents now. The fixed width was what kept a
                        // centre-anchored bar from sliding about as the contents
                        // changed; this row is anchored on the right, against the glass,
                        // so the thing that must not move does not and the rest can be
                        // whatever size it needs.
                        // **Each view gets the tools its job needs.**
                        //
                        // At the helm you are working the sheets and the pilot; from astern you
                        // are watching her sail and thinking about what is set; under the water
                        // you are fishing, and there is nothing on this boat to adjust from down
                        // there. A bar that showed everything everywhere showed the wrong thing
                        // three times out of four.
                        ui.scope(|ui| {
                            // **The sheets, on the left, on every camera.** They are the two ropes
                            // your hands are on all day; which way the camera happens to be
                            // pointing has nothing to do with it.
                            {
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
                                        sail_state(
                                            &telemetry.main,
                                            1.0 - sim.controls.reef * cfg.max_reef,
                                        ),
                                        (
                                            telemetry.main.drive + telemetry.main_upper.drive,
                                            telemetry.main.side + telemetry.main_upper.side,
                                        ),
                                    ) {
                                        sim.controls.main_sheet = main_sheet;
                                    }
                                    if lever(
                                        ui,
                                        "GENOA",
                                        &mut jib_sheet,
                                        Some(want_jib),
                                        sail_state(&telemetry.jib, 1.0 - sim.controls.furl),
                                        (
                                            telemetry.jib.drive + telemetry.jib_upper.drive,
                                            telemetry.jib.side + telemetry.jib_upper.side,
                                        ),
                                    ) {
                                        sim.controls.jib_sheet = jib_sheet;
                                    }

                                    ui.horizontal(|ui| {
                                        ui.add_space(8.0);
                                        ui.vertical(|ui| {
                                            ui.add_space(2.0);
                                            plate(ui, "BOOM");
                                            ui.add_space(4.0);
                                            let rigged = sim.controls.preventer;
                                            if ui
                                                .selectable_label(
                                                    rigged,
                                                    egui::RichText::new(if rigged {
                                                        "PREVENTED"
                                                    } else {
                                                        "free (F)"
                                                    })
                                                    .monospace()
                                                    .size(10.0)
                                                    .color(if rigged {
                                                        egui::Color32::from_rgb(20, 20, 20)
                                                    } else {
                                                        egui::Color32::from_rgb(150, 150, 130)
                                                    }),
                                                )
                                                .on_hover_text(
                                                    "a line forward from the boom end: she cannot gybe",
                                                )
                                                .clicked()
                                            {
                                                sim.controls.preventer = !rigged;
                                            }
                                            // **The wardrobe under the boom**, because
                                            // the two are the same conversation: what is
                                            // set forward, and what is stopping the spar
                                            // behind it. Asked for from the deck, and it
                                            // puts every sail she carries in one column.
                                            ui.add_space(10.0);
                                            ui.vertical(|ui| {
                                                                        plate(ui, "HEADSAIL");
                                                                        ui.add_space(4.0);
                                                                        // **The list is an order, not a switch.**
                                                                        //
                                                                        // It used to be enabled only with the bow already
                                                                        // clear, which made changing sail three separate
                                                                        // jobs: pick the furling line out of the bank, hold
                                                                        // a button until the drum was home, then come back
                                                                        // here. That is the winch's view of the work. The
                                                                        // person's view is *I want the Code 0 up*, and the
                                                                        // foredeck takes a minute over it — which is what
                                                                        // clicking a name does now, with the same sequence
                                                                        // underneath and the same minute.
                                                                        for sail in [
                                                                            Headsail::Genoa,
                                                                            Headsail::Code0,
                                                                            Headsail::Gennaker,
                                                                            Headsail::Spinnaker,
                                                                        ] {
                                                                            let up = sim.controls.headsail == sail
                                                                                && !orders.changing();
                                                                            let asked = orders.headsail == Some(sail);
                                                                            let label = egui::RichText::new(sail.name().to_uppercase())
                                                                                .monospace()
                                                                                .size(10.0)
                                                                                .color(if up {
                                                                                    egui::Color32::BLACK
                                                                                } else if asked {
                                                                                    paint::LIVE
                                                                                } else {
                                                                                    paint::LABEL
                                                                                });
                                                                            let button = egui::Button::new(label)
                                                                                .fill(if up {
                                                                                    paint::LIVE
                                                                                } else {
                                                                                    egui::Color32::from_gray(26)
                                                                                })
                                                                                .stroke(if asked {
                                                                                    egui::Stroke::new(1.0, paint::LIVE)
                                                                                } else {
                                                                                    egui::Stroke::NONE
                                                                                });
                                                                            if ui.add(button).clicked() {
                                                                                let was = sim.controls.headsail;
                                                                                crew.stand_down("the foredeck");
                                                                                orders.set(&sim, sail);
                                                                                info!(
                                                                                    "{}",
                                                                                    if orders.changing() {
                                                                                        format!("hands forward — {} off her, {} on", was.name(), sail.name())
                                                                                    } else {
                                                                                        "belay that".to_string()
                                                                                    }
                                                                                );
                                                                            }
                                                                        }
                                                                        // What the foredeck is at, where the "roll it away
                                                                        // first" refusal used to be. The line is the same
                                                                        // height either way so that nothing below it moves.
                                                                        ui.label(
                                                                            egui::RichText::new(orders.doing.unwrap_or(" "))
                                                                                .monospace()
                                                                                .size(10.0)
                                                                                .color(paint::LABEL),
                                                                        );
                                                                        // **The watch, under the wardrobe.**
                                                                        //
                                                                        // It stood on its own plate beside the case, which was
                                                                        // the right idea about what it *is* — a decision about
                                                                        // who is sailing her rather than a reading — and the
                                                                        // wrong place for it: a free-floating panel anchored
                                                                        // off another panel's edge sits on top of whatever
                                                                        // that panel grows into, and it covered the log and
                                                                        // the wind. It belongs with the sails, because what it
                                                                        // does is work them.
                                                                        ui.add_space(8.0);
                                                                        ui.horizontal(|ui| {
                                                                            let job = crew.doing();
                                                                            if hands(ui, crew.standing)
                                                                                .on_hover_text(if crew.standing {
                                                                                    format!("the watch has the sails — {job}")
                                                                                } else {
                                                                                    "call the watch on deck: they sail her, you steer"
                                                                                        .to_string()
                                                                                })
                                                                                .clicked()
                                                                            {
                                                                                crew.standing = !crew.standing;
                                                                                info!(
                                                                                    "{}",
                                                                                    if crew.standing {
                                                                                        "watch on deck — they have the sails"
                                                                                    } else {
                                                                                        "watch stood down — you have the sails"
                                                                                    }
                                                                                );
                                                                            }
                                                                            ui.vertical(|ui| {
                                                                                ui.label(
                                                                                    egui::RichText::new(if crew.standing {
                                                                                        "WATCH"
                                                                                    } else {
                                                                                        "SOLO"
                                                                                    })
                                                                                    .monospace()
                                                                                    .size(letter::UNIT)
                                                                                    .color(if crew.standing {
                                                                                        paint::GOOD
                                                                                    } else {
                                                                                        paint::LABEL
                                                                                    }),
                                                                                );
                                                                                // What they are on, for the same reason the
                                                                                // pilot says what it is holding: a machine
                                                                                // working on its own that cannot say what it is
                                                                                // working on is one you watch instead of trust.
                                                                                ui.set_width(96.0);
                                                                                ui.add(
                                                                                    egui::Label::new(
                                                                                        egui::RichText::new(job)
                                                                                            .monospace()
                                                                                            .size(letter::UNIT)
                                                                                            .color(if crew.standing {
                                                                                                paint::LABEL
                                                                                            } else {
                                                                                                egui::Color32::TRANSPARENT
                                                                                            }),
                                                                                    )
                                                                                    .wrap(),
                                                                                );
                                                                                // **What they may decide**,
                                                                                // under the switch that
                                                                                // calls them. Trim is
                                                                                // theirs and nobody argues;
                                                                                // a reef and a sail change
                                                                                // are decisions, and a
                                                                                // skipper who has made one
                                                                                // does not want it undone
                                                                                // by somebody reading a
                                                                                // heel gauge. Lit is
                                                                                // theirs.
                                                                                let mut allowed = [
                                                                                    (crew.may_reef, "REEFS", "the watch may shorten sail and shake it out again"),
                                                                                    (crew.may_change_sail, "SAILS", "the watch may change the headsail for the wind it has"),
                                                                                ];
                                                                                for (on, name, hint) in allowed.iter_mut().map(|(a, b, c)| (a, *b, *c)) {
                                                                                    if ui
                                                                                        .add(
                                                                                            egui::Button::new(
                                                                                                egui::RichText::new(name)
                                                                                                    .monospace()
                                                                                                    .size(letter::UNIT)
                                                                                                    .color(if *on {
                                                                                                        egui::Color32::BLACK
                                                                                                    } else {
                                                                                                        paint::LABEL
                                                                                                    }),
                                                                                            )
                                                                                            .fill(if *on {
                                                                                                paint::LIVE
                                                                                            } else {
                                                                                                egui::Color32::from_gray(26)
                                                                                            })
                                                                                            .min_size(egui::vec2(54.0, 15.0)),
                                                                                        )
                                                                                        .on_hover_text(hint)
                                                                                        .clicked()
                                                                                    {
                                                                                        *on = !*on;
                                                                                    }
                                                                                }
                                                                                crew.may_reef = allowed[0].0;
                                                                                crew.may_change_sail = allowed[1].0;
                                                                            });
                                                                        });
                                                                    });
                                        });
                                    });
                                });
                            }
                        });
                    });
                });
            });
        });

    // --- The rack of lines, in the middle of the bottom -----------------------------
    //
    // Seven ropes on one winch, on every camera, where there is room for a wall of
    // levers to be read along. The tab on its edge folds it away when the foredeck
    // wants the room.
    egui::Area::new("rack".into())
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -10.0))
        .interactable(true)
        .show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                if tab_edge(ui, console.rack) {
                    console.rack = !console.rack;
                }
                if console.rack {
                    frame.show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            // **Each view gets the tools its job needs.**
                            //
                            // At the helm you are working the sheets and the pilot; from astern
                            // you are watching her sail and thinking about what is set; under
                            // the water you are fishing and there is nothing on this boat to
                            // adjust from down there. A bar that showed everything everywhere
                            // was a bar that showed the wrong thing three times out of four.
                            // **The gear beside the lines**, which is the same kind of
                            // thing: what is set, and what is led where. The wardrobe and the
                            // ship's own switches used to be a page of their own and then a
                            // group on the bar that kept landing on top of the sheets.

                            ui.add_space(8.0);

                            // **And nothing else.** The ship's two switches were here
                            // because the console's rule is that everything the keyboard can
                            // do the mouse can do — which was read as *everything the mouse
                            // can do must be on the bar*. It does not say that. Settings is a
                            // key everybody tries first, and giving up a job is a decision
                            // with consequences that belongs on the page where the passage
                            // is, not beside the winch.

                            // The clutch bank: one winch, one line at a time.
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    for (i, line) in BANK.iter().enumerate() {
                                        let mut value = line.get(&sim.controls);
                                        let selected = i == bank.selected;
                                        // A line that leads nowhere on the sail that is up gets
                                        // its name bracketed rather than hidden: the clutch is
                                        // still on the coachroof, it just has nothing on the
                                        // other end of it at the moment.
                                        let rigged = line.rigged_for(&sim.controls);
                                        // The short name on the plate; the long one is said
                                        // under the rack where there is a line for it.
                                        let name = if rigged {
                                            line.tag(&sim.controls).to_string()
                                        } else {
                                            format!("({})", line.tag(&sim.controls))
                                        };
                                        let stops = line.detents(&sim.controls);
                                        let position = line.position(&sim.controls);
                                        let (clicked, step) = clutch(
                                            ui,
                                            &name,
                                            &Rope {
                                                selected,
                                                stops,
                                                position,
                                                // Where each button takes her, by name.
                                                to: line
                                                    .stop_index(&sim.controls)
                                                    .map(|at| {
                                                        (
                                                            line.stop_called(
                                                                &sim.controls,
                                                                at as i32 + 1,
                                                            ),
                                                            line.stop_called(
                                                                &sim.controls,
                                                                at as i32 - 1,
                                                            ),
                                                        )
                                                    })
                                                    .unwrap_or((None, None)),
                                                rate: line.rate(&sim.controls),
                                                dt: time.delta_secs_f64(),
                                            },
                                            &mut value,
                                        );
                                        if clicked {
                                            bank.selected = i;
                                            bank.since_change = 0.0;
                                        }
                                        // Working one, not picking one up to look at it:
                                        // choosing a line off the bank is a glance, and
                                        // a glance should not fire the watch.
                                        if step != 0 || value != line.get(&sim.controls) {
                                            crew.stand_down("the bank");
                                        }
                                        if step != 0 {
                                            // The next cringle up or down, and the line
                                            // runs there at its own pace: one press
                                            // starts the job, the job takes as long as
                                            // it takes.
                                            if let Some(stops) = line.detents(&sim.controls) {
                                                let here = stops
                                                    .iter()
                                                    .enumerate()
                                                    .min_by(|a, b| {
                                                        (a.1 - value)
                                                            .abs()
                                                            .partial_cmp(&(b.1 - value).abs())
                                                            .unwrap()
                                                    })
                                                    .map(|(n, _)| n as i32)
                                                    .unwrap_or(0);
                                                let want = (here + step)
                                                    .clamp(0, stops.len() as i32 - 1)
                                                    as usize;
                                                bank.target[i] = Some(stops[want]);
                                            }
                                        } else if value != line.get(&sim.controls) {
                                            // A pull on a line that stops wherever you
                                            // stop it: the value is already where the
                                            // hand left it.
                                            bank.target[i] = None;
                                            line.set_from_ui(&mut sim.controls, value);
                                        }
                                    }
                                });
                                ui.label(
                                    egui::RichText::new(fit(
                                        // The long name belongs on this line and not on the
                                        // plate: a nameplate is a label, and the sentence that
                                        // explains it is a different object.
                                        &if bank.line().rigged_for(&sim.controls) {
                                            format!(
                                                "{} — hauling in {}",
                                                bank.line().name_now(&sim.controls),
                                                bank.line().hauled_now(&sim.controls)
                                            )
                                        } else {
                                            format!(
                                                "{} is not led to this sail",
                                                bank.line().name()
                                            )
                                        },
                                        44,
                                    ))
                                    .color(paint::LABEL)
                                    .monospace()
                                    .size(9.0),
                                );
                            });
                        });
                    });
                }
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
/// **As wide as the widest page and no wider.**
///
/// Fixed rather than sized to its contents, which is not tidiness: the bar is anchored
/// on its middle, so a width that changes slides the whole console sideways — and once
/// in a thousand frames egui lays it out twice, which with different widths put two
/// consoles on the screen at once. Fixed makes the second pass land on the first.
///
/// The number is the rack of seven lines plus the glass, the pilot and the tabs: packed
/// down from 1180, which was the old bar's width and had a hand's breadth of empty case
/// in it on every page but one.
/// A gauge: wide enough for a label, a number and a unit, and no taller than it has to
/// be. Two of them side by side are the width of the column.
const GAUGE: egui::Vec2 = egui::vec2(82.0, 29.0);

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
    let (rect, _) = ui.allocate_exact_size(GAUGE, egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 2.0, paint::TRACK);
    let f = ((value - lo) / (hi - lo)).clamp(0.0, 1.0) as f32;
    let bar = egui::Rect::from_min_size(
        rect.left_bottom() + egui::vec2(3.0, -11.0),
        egui::vec2((rect.width() - 6.0) * f.max(0.02), 8.0),
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
        egui::FontId::monospace(11.0),
        if alarm { paint::BAD } else { paint::FACE },
    );
}

fn gauge(ui: &mut egui::Ui, label: &str, value: f64, lo: f64, hi: f64, unit: &str) {
    let (rect, _) = ui.allocate_exact_size(GAUGE, egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 2.0, paint::TRACK);
    let f = ((value - lo) / (hi - lo)).clamp(0.0, 1.0) as f32;
    let bar = egui::Rect::from_min_size(
        rect.left_bottom() + egui::vec2(3.0, -11.0),
        egui::vec2((rect.width() - 6.0) * f, 8.0),
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
        egui::FontId::monospace(11.0),
        paint::FACE,
    );
}

/// Heel, drawn as a horizon that tilts. One to one, because it has to agree with what
/// The clinometer: a pendulum hanging in an engraved arc.
///
/// **It was a horizon line in a box, and the line was not clipped.** At twenty degrees
/// of heel it ran out of its own strip and straight across the rest of the console — a
/// green diagonal through the gauges that looked like a fault in the screen, which is
/// what it was. Found by photographing her and looking, which is the first thing the
/// camera paid for.
///
/// Rebuilt as the instrument a boat of this age actually carries: a brass pendulum
/// swinging in a degree arc, with the figures engraved on ivory. It cannot leave the
/// What the glass has done, in the words a forecast uses.
///
/// The thresholds are the Met Office's own barometric tendency bands, because they are
/// the ones a sailor has already learnt: under half a hectopascal in three hours is
/// steady, and three or more is the one that gets a name — a fall of three in three
/// hours is the classic "gale warning on the glass" and is why it is worth a word of its
/// own rather than a number nobody weighs.
fn tendency_words(three_hourly: f64) -> String {
    let rate = three_hourly.abs();
    let how = if rate < 0.5 {
        return "steady".into();
    } else if rate < 1.6 {
        "slowly"
    } else if rate < 3.5 {
        ""
    } else {
        "fast"
    };
    let way = if three_hourly < 0.0 {
        "falling"
    } else {
        "rising"
    };
    if how.is_empty() {
        format!("{way} {rate:.1}")
    } else {
        format!("{way} {how}")
    }
}

/// Two short readouts on one line.
///
/// Most of what this column says is three characters long — a heading, a wind angle, a
/// count — and a full-width row for three characters is two thirds of that row thrown
/// away. Asked for from the deck as wanting less of the picture spent on the
/// instruments: the column is a third shorter for this and nothing has been taken out
/// of it.
///
/// Only for the short ones. An apparent wind with its speed beside it, a barometer with
/// its tendency, a destination — those are sentences and they keep their own line,
/// because the saving from cramming them is a line and the cost is having to read twice.
fn readout_pair(ui: &mut egui::Ui, left: (&str, &str), right: (&str, &str)) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let half = (ui.available_width() - 4.0) * 0.5;
        ui.scope(|ui| {
            ui.set_width(half);
            readout(ui, left.0, left.1);
        });
        ui.scope(|ui| {
            ui.set_width(half);
            readout(ui, right.0, right.1);
        });
    });
}

fn readout(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(
            egui::RichText::new(format!("{label:<5}"))
                .color(paint::LABEL)
                .monospace()
                .size(9.0),
        );
        ui.label(
            egui::RichText::new(value)
                .color(paint::FACE)
                .monospace()
                .size(letter::VALUE),
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
/// Room for the longest thing it says: `100% stall`.
const BITE_COLUMNS: usize = 10;

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
    // One word each, and short ones: see `BITE_COLUMNS`.
    let worst = if stalled > slow.max(heeled) {
        "stall"
    } else if heeled > slow {
        "heel"
    } else {
        "slow"
    };
    format!("{:.0}% {worst}", bite * 100.0)
}

/// Stopped head to wind, with no way on to steer by.
///
/// Thirty degrees and two knots, together. Either alone is ordinary — thirty degrees
/// with way on is a boat pinching, two knots with the wind on the beam is a boat in a
/// calm — and it is the pair of them that is the state with no way out through the
/// helm. The rudder has nothing going over it and the sails are all luffing, so nothing
/// aboard is doing anything at all.
fn in_irons(tm: &sail_core::state::Telemetry) -> bool {
    deg(tm.twa).abs() < 30.0 && ms_to_kn(tm.stw) < 2.0
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
/// The rudder angle indicator: a sector dial with a needle on it.
///
/// **The same instrument as the compass beside it, and that is the argument.** The helm
/// was a dark strip with a dot sliding along it — a slider, which is a thing from a
/// settings dialog and not a thing from a boat — and it sat next to an engraved ivory
/// card that reads at a glance from a yard away. A panel whose parts come from two
/// different centuries is a panel the eye has to be told how to read twice.
///
/// Drawn the way the indicator over a wheelhouse door is drawn: the blade swinging about
/// its stock, port to the left, starboard to the right, with the degrees engraved on the
/// The working parts of a lever, drawn once for the two things that are levers.
///
/// **A slot milled in the case, a brass lever in it, and the stops engraved beside it.**
/// Everything on this panel that sets a rope is the same object: there is no reason for
/// a sheet and a clutch to look like two different inventions, and a rack of identical
/// levers is read as a rack, which is the point of a rack.
///
/// `want` is where the trimmer would have it; `stops` are the places it rests, if it
/// rests anywhere.
fn lever_face(
    p: &egui::Painter,
    rect: egui::Rect,
    value: f64,
    want: Option<f64>,
    stops: Option<&[f64]>,
    live: bool,
) {
    let slot = egui::Rect::from_center_size(
        egui::pos2(rect.center().x, rect.center().y),
        egui::vec2(7.0, rect.height() - 16.0),
    );
    p.rect_filled(slot, 3.0, paint::TRACK);
    let y_of = |v: f64| slot.bottom() - v as f32 * slot.height();
    // The stops, engraved either side of the slot. A lever between two of them is a job
    // half done, and it shows as being between rather than as a number.
    if let Some(stops) = stops {
        for stop in stops {
            let y = y_of(*stop);
            let near = (stop - value).abs() < 0.02;
            for side in [-1.0, 1.0] {
                p.line_segment(
                    [
                        egui::pos2(slot.center().x + side * 5.0, y),
                        egui::pos2(slot.center().x + side * (if near { 13.0 } else { 9.0 }), y),
                    ],
                    egui::Stroke::new(
                        if near { 2.0 } else { 1.0 },
                        if near { paint::FACE } else { paint::LABEL },
                    ),
                );
            }
        }
    } else {
        // A line that stops wherever you stop it gets a scale instead: quarters, which
        // is as fine as anybody sets a vang by eye.
        for mark in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let y = y_of(mark);
            p.line_segment(
                [
                    egui::pos2(slot.right() + 2.0, y),
                    egui::pos2(slot.right() + if mark % 0.5 == 0.0 { 8.0 } else { 5.0 }, y),
                ],
                egui::Stroke::new(1.0, paint::LABEL.gamma_multiply(0.8)),
            );
        }
    }
    // Where the trimmer would have it, in a green that is not the lever's own colour.
    if let Some(want) = want {
        let y = y_of(want);
        p.line_segment(
            [
                egui::pos2(rect.left() + 4.0, y),
                egui::pos2(rect.right() - 4.0, y),
            ],
            egui::Stroke::new(1.5, paint::GOOD),
        );
    }
    // And the lever itself: a brass knob on the slot, with a milled edge.
    let y = y_of(value);
    let knob = egui::Rect::from_center_size(
        egui::pos2(slot.center().x, y),
        egui::vec2(rect.width() - 14.0, 13.0),
    );
    p.rect_filled(knob, 2.0, if live { paint::LIVE } else { paint::BEZEL });
    p.rect_stroke(
        knob,
        2.0,
        egui::Stroke::new(1.0, paint::INK.gamma_multiply(0.7)),
        egui::StrokeKind::Inside,
    );
    for cut in [-0.3_f32, 0.0, 0.3] {
        let x = knob.center().x + cut * knob.width();
        p.line_segment(
            [
                egui::pos2(x, knob.top() + 3.0),
                egui::pos2(x, knob.bottom() - 3.0),
            ],
            egui::Stroke::new(1.0, paint::INK.gamma_multiply(0.45)),
        );
    }
}

/// A small engraved nameplate, for under a lever.
fn tag(p: &egui::Painter, at: egui::Rect, name: &str, lit: bool) {
    p.rect_filled(at, 1.0, if lit { paint::LIVE } else { paint::PLATE });
    p.text(
        at.center(),
        egui::Align2::CENTER_CENTER,
        name,
        egui::FontId::monospace(letter::UNIT),
        paint::INK,
    );
}

/// A sheet lever: drag up to haul in, down to ease.
///
/// The wanted position is marked, so the gap between where it is and where it should be
/// is a distance rather than a sum.
fn lever(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    want: Option<f64>,
    state: (&'static str, egui::Color32),
    load: (f64, f64),
) -> bool {
    ui.vertical(|ui| {
        // **Long, because a sheet is long.** The travel is the thing your hand is
        // setting, and an inch of it was asking for a trim to be judged in sixths of a
        // millimetre. There is a page to itself now, so it can have the room.
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(84.0, 282.0), egui::Sense::click_and_drag());
        let p = ui.painter();
        p.rect_filled(rect, 3.0, paint::CASE);
        p.rect_stroke(
            rect,
            3.0,
            egui::Stroke::new(1.0, paint::BEZEL),
            egui::StrokeKind::Inside,
        );
        // The rope's own column, with the gauges in the strip beside it rather than
        // over it: a lever and a reading are two things and they were sharing a slot.
        let face = egui::Rect::from_min_max(
            rect.left_top() + egui::vec2(0.0, 17.0),
            rect.right_bottom() - egui::vec2(20.0, 28.0),
        );
        // A click *is* a trim: put the rope where you touched. Dragging is the same
        // act continued, which is how a winch handle behaves and not how a scrollbar
        // does — there is nothing here to grab hold of first.
        let changed = if response.dragged() || response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let f = ((face.bottom() - 8.0 - pos.y) / (face.height() - 16.0)).clamp(0.0, 1.0);
                // Up is hauled in, so the lever reads the way the rope moves.
                *value = 1.0 - f as f64;
            }
            true
        } else {
            false
        };
        // Drawn with the sheet the other way up from its number: hauled in is at the
        // top, because that is where the hand takes the rope to.
        lever_face(p, face, 1.0 - *value, want.map(|w| 1.0 - w), None, false);
        // **What the sail is making of it**, beside the rope that is setting it: two
        // columns, forward and sideways, on one scale so they can be compared by eye.
        // Trimming is the business of turning the second into the first, and until now
        // the only way to see whether it was working was to watch the speed and the
        // angle of the deck.
        for (i, (force, colour)) in [(load.0, paint::GOOD), (load.1.abs(), paint::NEEDLE)]
            .into_iter()
            .enumerate()
        {
            let full = 4_000.0;
            let h = (face.height() - 16.0) * (force / full).clamp(0.0, 1.0) as f32;
            let x = face.right() + 6.0 + i as f32 * 8.0;
            let bar = egui::Rect::from_min_max(
                egui::pos2(x - 2.5, face.bottom() - 8.0 - h),
                egui::pos2(x + 2.5, face.bottom() - 8.0),
            );
            p.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(x - 2.5, face.top() + 8.0),
                    egui::pos2(x + 2.5, face.bottom() - 8.0),
                ),
                1.0,
                paint::TRACK,
            );
            p.rect_filled(bar, 1.0, colour);
        }
        tag(
            p,
            egui::Rect::from_min_size(
                rect.left_top() + egui::vec2(3.0, 3.0),
                egui::vec2(rect.width() - 6.0, 14.0),
            ),
            label,
            false,
        );
        p.text(
            egui::pos2(rect.center().x, rect.bottom() - 13.0),
            egui::Align2::CENTER_CENTER,
            state.0,
            egui::FontId::monospace(letter::UNIT),
            state.1,
        );
        changed
    })
    .inner
}

/// One clutch on the coachroof: click to put it on the winch, drag to work it.
///
/// **A line with stops is drawn as a line with stops.** A reef is not seventy-five per
/// cent of anything — it is the third reef, a place the sail rests at and a job that was
/// done — so a detented line gets notches engraved beside its slot and the name of the
/// position it is at, while a line that stops wherever you stop it gets a scale and a
/// figure. The difference between *how much* and *which*, which an instrument can say
/// One clutch on the coachroof: press to put it on the winch, hold a button to work it.
///
/// **A rope is wound, not dragged.** The lever was a tall slot you had to catch and pull
/// through its travel, which is three hundred points of case for a thing your hand does
/// by turning a handle — and it asked for a drag *inside the control*, which is the one
/// gesture a moving deck makes hard. Hold ▲ and the line comes in at its own rate; hold
/// ▼ and it goes out. Exactly what the keys do, and the same rates, so the mouse and the
/// keyboard are the same winch.
///
/// A line with stops shows them as notches along the travel and says which one it is at;
/// a line without shows a bar and a figure. The difference between *which* and *how
/// much*, which an instrument can say without a word of explanation.
fn clutch(ui: &mut egui::Ui, label: &str, line: &Rope, value: &mut f64) -> (bool, i32) {
    let (selected, stops, position, rate, dt) =
        (line.selected, line.stops, line.position, line.rate, line.dt);
    // A direction with nowhere left to go is not offered: the staysail cannot be set
    // twice and the third reef is the last one there is.
    let dead = (
        line.stops.is_some() && line.to.0.is_none(),
        line.stops.is_some() && line.to.1.is_none(),
    );
    ui.vertical(|ui| {
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(78.0, 104.0), egui::Sense::click());
        let p = ui.painter();
        p.rect_filled(rect, 3.0, paint::CASE);
        p.rect_stroke(
            rect,
            3.0,
            egui::Stroke::new(
                if selected { 1.6 } else { 1.0 },
                if selected { paint::LIVE } else { paint::BEZEL },
            ),
            egui::StrokeKind::Inside,
        );
        tag(
            p,
            egui::Rect::from_min_size(
                rect.left_top() + egui::vec2(3.0, 3.0),
                egui::vec2(rect.width() - 6.0, 14.0),
            ),
            label,
            selected,
        );
        // Where the rope is, along the width of the plate: a recess with the travel in
        // it, and the stops engraved across it when the line has any.
        let slot = egui::Rect::from_min_size(
            rect.left_top() + egui::vec2(6.0, 21.0),
            egui::vec2(rect.width() - 12.0, 9.0),
        );
        p.rect_filled(slot, 2.0, paint::TRACK);
        let x_of = |v: f64| slot.left() + slot.width() * v as f32;
        if let Some(stops) = stops {
            for stop in stops {
                let x = x_of(*stop);
                let near = (stop - *value).abs() < 0.02;
                p.line_segment(
                    [egui::pos2(x, slot.top()), egui::pos2(x, slot.bottom())],
                    egui::Stroke::new(
                        if near { 2.2 } else { 1.0 },
                        if near { paint::FACE } else { paint::LABEL },
                    ),
                );
            }
        } else {
            let mut run = slot;
            run.set_right(x_of(*value));
            p.rect_filled(run, 2.0, if selected { paint::LIVE } else { paint::LABEL });
        }
        p.text(
            egui::pos2(rect.center().x, rect.top() + 40.0),
            egui::Align2::CENTER_CENTER,
            match position {
                Some(said) => said.to_string(),
                None => format!("{:.2}", *value),
            },
            egui::FontId::monospace(letter::UNIT),
            paint::FACE,
        );
        // And the two halves of the winch. Held rather than clicked, because that is
        // what grinding is: the line runs while your hand is on the handle.
        let mut worked = false;
        let mut step = 0i32;
        for (which, legend) in [
            (1.0_f64, line.to.0.unwrap_or("HAUL")),
            (-1.0, line.to.1.unwrap_or("EASE")),
        ] {
            let at = egui::Rect::from_min_size(
                egui::pos2(
                    rect.left() + 6.0,
                    rect.top() + if which > 0.0 { 50.0 } else { 76.0 },
                ),
                egui::vec2(rect.width() - 12.0, 24.0),
            );
            let gone = if which > 0.0 { dead.0 } else { dead.1 };
            let hit = ui.interact(
                at,
                ui.id().with((label, legend)),
                if gone {
                    egui::Sense::hover()
                } else {
                    egui::Sense::click_and_drag()
                },
            );
            let down = hit.is_pointer_button_down_on();
            p.rect_filled(at, 2.0, if down { paint::LIVE } else { paint::TRACK });
            p.rect_stroke(
                at,
                2.0,
                egui::Stroke::new(1.0, paint::BEZEL),
                egui::StrokeKind::Inside,
            );
            p.text(
                at.center(),
                egui::Align2::CENTER_CENTER,
                legend,
                egui::FontId::monospace(letter::UNIT),
                if down { paint::INK } else { paint::FACE },
            );
            // **A line with stops takes a step; a line without takes a pull.** They
            // were both treated as a pull, and the reef went nowhere at all: the button
            // nudged it a thousandth off its cringle and the bank then ran it back to
            // the nearest stop, which was the one it had just left. Reported from the
            // deck as the reef not working, which it was not.
            if stops.is_some() {
                if hit.clicked() {
                    step += which as i32;
                    worked = true;
                }
            } else if down {
                *value = (*value + which * rate * dt).clamp(0.0, 1.0);
                worked = true;
            }
        }
        (response.clicked() || worked, step)
    })
    .inner
}

/// A pressed-steel button with the legend engraved on it.
///
/// **Twenty-six points high, not twenty.** A control you press at sea on a moving deck
/// is pressed with a thumb, and the thumb is the same size whatever the screen is; the
/// old ones were the size they were because they were made out of text. Nothing here
/// loses anything by being bigger — this bar had a hand's breadth of empty case either
/// side of it.
fn small_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(text)
                .monospace()
                .size(letter::LABEL)
                .color(paint::FACE),
        )
        .fill(paint::TRACK)
        .stroke(egui::Stroke::new(1.0, paint::BEZEL))
        .corner_radius(2.0)
        .min_size(egui::vec2(96.0, 26.0)),
    )
}

/// The little ones that step a number: still a thumb, still bigger than they were.
fn nudge_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(text)
                .monospace()
                .size(letter::LABEL)
                .color(paint::FACE),
        )
        .fill(paint::TRACK)
        .stroke(egui::Stroke::new(1.0, paint::BEZEL))
        .corner_radius(2.0)
        .min_size(egui::vec2(24.0, 26.0)),
    )
}

/// A button that lights: brass and engraved when it is on, dark steel when it is not.
///
/// The one state change on this panel that has to be legible from the other end of the
/// cockpit — *is the pilot steering her* — so it is a change of material rather than a
/// change of tint.
/// The watch: a square switch with two hands engraved on it.
///
/// **A picture rather than a word, because it is not an instrument.** Everything else on
/// this console is a reading or a lever; this one says *there are people aboard and they
/// are working*, and the oldest way to say that on a brass plate is to engrave the
/// people. It is square and it is the size of a thumb, because it is the one control you
/// reach for without looking at it.
fn hands(ui: &mut egui::Ui, standing: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(44.0, 44.0), egui::Sense::click());
    let p = ui.painter();
    let lit = standing;
    let hot = response.hovered();
    p.rect_filled(rect, 2.0, if lit { paint::LIVE } else { paint::TRACK });
    p.rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(
            if lit || hot { 1.4 } else { 1.0 },
            if lit { paint::FACE } else { paint::BEZEL },
        ),
        egui::StrokeKind::Inside,
    );
    let ink = if lit { paint::INK } else { paint::FACE };
    let c = rect.center();
    // Two of them, one at a winch handle and one leaning back on a sheet: a head, a
    // body, and the arms, which is as much as survives at this size.
    let figure = |p: &egui::Painter, at: egui::Pos2, h: f32, lean: f32| {
        p.circle_filled(at + egui::vec2(lean * 0.7, -h * 0.62), h * 0.16, ink);
        p.add(egui::Shape::convex_polygon(
            vec![
                at + egui::vec2(lean * 0.6 - h * 0.17, -h * 0.42),
                at + egui::vec2(lean * 0.6 + h * 0.17, -h * 0.42),
                at + egui::vec2(h * 0.20, h * 0.42),
                at + egui::vec2(-h * 0.20, h * 0.42),
            ],
            ink,
            egui::Stroke::NONE,
        ));
        // The arms, out in front and holding something. Nobody stands on a foredeck
        // with their hands by their sides.
        p.line_segment(
            [
                at + egui::vec2(lean * 0.6, -h * 0.34),
                at + egui::vec2(lean * 1.5 + h * 0.10, -h * 0.10),
            ],
            egui::Stroke::new(h * 0.10, ink),
        );
    };
    figure(p, c + egui::vec2(-6.0, 7.0), 24.0, 3.0);
    figure(p, c + egui::vec2(8.0, 9.0), 20.0, -3.0);
    response
}

/// What is in your hands: the two instruments and the rod.
///
/// One parameter rather than three, because a system may have sixteen and the console
/// wanted seventeen. Grouping them is not a workaround — these three are the same kind
/// of thing, and the buttons that work them sit together in the same row.
#[derive(bevy::ecs::system::SystemParam)]
pub struct InHand<'w> {
    pub fishing: ResMut<'w, crate::fishing::Fishing>,
    pub sighting: ResMut<'w, crate::sight::Sighting>,
    /// Not a thing in anybody's hand — it is here because a system may have sixteen
    /// parameters and this panel wanted a seventeenth. The card needs to know who else
    /// is in on this leg; see `net.finished`.
    pub net: Res<'w, crate::net::Net>,
}

/// A narrow lamp for a thing you pick up: three of them fit the window they live in.
///
/// The same material change as `lamp_button` — brass when it is in your hand, dark steel
/// when it is not — at the width three of them can share.
fn tool_button(ui: &mut egui::Ui, text: &str, lit: bool) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(text)
                .monospace()
                .size(letter::UNIT)
                .color(if lit { paint::INK } else { paint::FACE }),
        )
        .fill(if lit { paint::LIVE } else { paint::TRACK })
        .stroke(egui::Stroke::new(
            if lit { 1.4 } else { 1.0 },
            if lit { paint::FACE } else { paint::BEZEL },
        ))
        .corner_radius(2.0)
        .min_size(egui::vec2(53.0, 18.0)),
    )
}

fn lamp_button(ui: &mut egui::Ui, text: &str, lit: bool) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(text)
                .monospace()
                .size(letter::LABEL)
                .color(if lit { paint::INK } else { paint::FACE }),
        )
        .fill(if lit { paint::LIVE } else { paint::TRACK })
        .stroke(egui::Stroke::new(
            if lit { 1.4 } else { 1.0 },
            if lit { paint::FACE } else { paint::BEZEL },
        ))
        .corner_radius(2.0)
        // Narrower than the ship's buttons: a word and a lamp, not a sentence.
        .min_size(egui::vec2(96.0, 26.0)),
    )
}
