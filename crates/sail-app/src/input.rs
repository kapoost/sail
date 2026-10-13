//! Player intent to boat controls.
//!
//! The design decision that makes a sailing sim playable at all: you do not map lines
//! to keys. A boat has a dozen of them and sailing needs several at once, which no
//! keyboard affords. You map *intentions*, and you match the control's nature to the
//! gear it stands for.
//!
//! * Helm is **absolute and held**, because a tiller has a position your hand is
//!   holding. On a keyboard that means A and D *move* the helm and letting go leaves
//!   it where it was; Space centres it. A self-centring keyboard helm is unusable —
//!   if it springs back faster than you can push it over, you cannot hold any angle
//!   at all, and steering becomes a drum solo.
//! * The mainsheet is **absolute**, because it is a tackle you pull and hold.
//! * The genoa sheet is a **rate**, because it is a winch you grind — and grinding
//!   takes time you do not always have, which is the whole drama of a tack.
//! * The slow control lines are not here at all. They live on a clutch bank — see
//!   `clutch.rs` — because one winch serves all of them and a crew works one at a
//!   time.
//!
//! The autopilot disengages the moment you touch the helm. Real pilots have a clutch
//! you have to throw, but a boat that fights your hand is worse than one that lets go.

use bevy::prelude::*;
use sail_core::autopilot::optimal_sheets_with;
use sail_core::prelude::*;
use sail_core::state::Headsail;

use crate::sim::Sim;

/// Dead zone on the sticks. Analog hardware drifts, and a drifting helm is maddening.
const STICK_DEADZONE: f32 = 0.12;

/// The autopilot, as a Bevy resource.
///
/// A newtype only because `Resource` cannot be derived on a type from another crate —
/// which is the small, honest tax for keeping `sail-core` free of engine types.
#[derive(Resource, Default)]
pub struct Pilot {
    pub inner: Autopilot,
    /// Why it last dropped out, and how long ago.
    ///
    /// A pilot that disengages without saying why is indistinguishable from one that
    /// is broken. Recording the reason turns "it keeps switching off" from a report
    /// into a fact in a single run.
    pub last_reason: Option<&'static str>,
    pub since_reason: f32,
}

impl std::ops::Deref for Pilot {
    type Target = Autopilot;
    fn deref(&self) -> &Autopilot {
        &self.inner
    }
}

impl std::ops::DerefMut for Pilot {
    fn deref_mut(&mut self) -> &mut Autopilot {
        &mut self.inner
    }
}

impl Pilot {
    fn drop_out(&mut self, reason: &'static str) {
        if self.inner.engaged() {
            self.inner.disengage();
            self.last_reason = Some(reason);
            self.since_reason = 0.0;
        }
    }
}

/// Which device last touched the helm.
///
/// Without this, a connected but idle gamepad fights the keyboard: the stick reads
/// centred, so it zeroes the rudder every frame and the keys can never hold an angle.
/// Whoever moved the helm last owns it until the other device moves.
#[derive(Resource, Default, PartialEq, Eq, Clone, Copy)]
pub enum HelmSource {
    #[default]
    Keyboard,
    Stick,
}
/// How fast a sheet runs, in sheet fraction per second.
const SHEET_RATE: f64 = 0.45;
/// Fine sheet rate while Shift is held. Upwind the whole useful trim band is about a
/// third of the travel, and placing it to a degree is the entire point of the game.
const SHEET_FINE: f64 = 0.11;
/// How fast the keyboard helm swings, radians per second. Full lock in about half a
/// second, which is roughly how fast you can actually throw a tiller over.
const KEY_HELM_RATE: f64 = 1.3;
/// Fine rate while a modifier is held, for trimming out weather helm a degree at a
/// time instead of overshooting it.
const KEY_HELM_FINE: f64 = 0.28;
/// How fast Space brings the helm back amidships.
const KEY_HELM_CENTRE: f64 = 2.2;

/// What one press moves the autopilot's target, and what it moves with Shift held.
///
/// The same two numbers serve the compass and the wind vane, because the pilot takes a
/// *target* either way and the hand asking for it is the same hand. They live here, in
/// the input, rather than in the autopilot: how far a key moves something is a property
/// of the key, and `Autopilot::adjust` is given an angle and does not care where it came
/// from. The panel reaches for them so its buttons and the keys cannot drift apart.
pub const PILOT_STEP: f64 = rad(10.0);
pub const PILOT_FINE: f64 = rad(1.0);

fn deadzone(v: f32) -> f32 {
    if v.abs() < STICK_DEADZONE {
        0.0
    } else {
        (v.abs() - STICK_DEADZONE) / (1.0 - STICK_DEADZONE) * v.signum()
    }
}

#[allow(clippy::too_many_arguments)]
pub fn gather(
    mut sim: ResMut<Sim>,
    sound: Res<crate::sound::SoundHandle>,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    mut helm_source: ResMut<HelmSource>,
    mut pilot: ResMut<Pilot>,
    mut assisting: Local<f32>,
    mut stage: Local<u8>,
) {
    let dt = time.delta_secs_f64();
    let cfg = sim.config();
    let max_rudder = cfg.max_rudder_angle;

    let pad = pads.iter().next();

    // --- Autopilot -------------------------------------------------------------
    pilot.since_reason += time.delta_secs();

    // P engages and disengages; O switches between holding a course and holding a wind
    // angle. One key cycling three states meant the third press dropped the pilot
    // whether or not that was what you wanted, which is a surprising way for a piece
    // of equipment to behave.
    let toggle = keys.just_pressed(KeyCode::KeyP)
        || pad.is_some_and(|p| p.just_pressed(GamepadButton::Start));
    if toggle {
        if pilot.engaged() {
            pilot.drop_out("you switched it off");
        } else {
            pilot.hold_heading(&sim.sim.state);
            pilot.last_reason = None;
        }
    }
    if keys.just_pressed(KeyCode::KeyO) && pilot.engaged() {
        match pilot.mode {
            PilotMode::Heading(_) => {
                let tm = sim.sim.telemetry;
                pilot.hold_wind(&tm);
            }
            _ => pilot.hold_heading(&sim.sim.state),
        }
    }
    // Ten degrees a press, one with Shift held — the same way round as the helm, where
    // Shift is the small bite. A degree at a time was the wrong default: putting the
    // pilot thirty degrees onto a new course meant thirty presses, and nobody steers a
    // boat in single degrees except in the last moments of lining something up. Ten is
    // a point of the compass and change, which is how a course alteration is actually
    // thought about, and the fine step is still there under Shift for the lining-up.
    let nudge = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
        PILOT_FINE
    } else {
        PILOT_STEP
    };
    if keys.just_pressed(KeyCode::Period) {
        pilot.adjust(nudge);
    }
    if keys.just_pressed(KeyCode::Comma) {
        pilot.adjust(-nudge);
    }

    // --- Helm -----------------------------------------------------------------
    // A gamepad stick is an absolute position, so when it owns the helm its centre
    // means amidships. The keyboard holds its angle instead, because a key has no
    // position of its own to read.
    let stick = pad
        .map(|p| deadzone(p.get(GamepadAxis::LeftStickX).unwrap_or(0.0)))
        .unwrap_or(0.0);
    if stick != 0.0 {
        *helm_source = HelmSource::Stick;
    }

    let mut key_rate = 0.0;
    let fine = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let rate = if fine { KEY_HELM_FINE } else { KEY_HELM_RATE };
    // Arrows only. A and D work the clutch bank's chosen line.
    if keys.pressed(KeyCode::ArrowLeft) {
        key_rate -= rate;
    }
    if keys.pressed(KeyCode::ArrowRight) {
        key_rate += rate;
    }
    let centring = keys.pressed(KeyCode::Space);
    if key_rate != 0.0 || centring {
        *helm_source = HelmSource::Keyboard;
    }

    // A pilot that has been on the stops for six seconds is braking, not steering. This
    // is the one dropout that is *not* your doing, so it is the one that alarms: you
    // switched it off, you took the helm, you centred it — those you already know about,
    // and a machine that beeps at you for doing what you meant to do gets ignored within
    // the hour. Losing the course is the one you need telling about, usually because you
    // are below and the boat is now steering herself.
    if pilot.engaged() && pilot.struggling() && pilot.can_let_go() {
        pilot.drop_out("it could not hold the course — rudder was hard over");
        sound.0.alarm();
    }
    // Touching the helm throws the pilot out, and says so.
    if pilot.engaged() {
        if key_rate != 0.0 {
            pilot.drop_out("you took the helm");
        } else if centring {
            pilot.drop_out("you centred the helm");
        } else if stick != 0.0 {
            pilot.drop_out("the stick moved");
        }
    }
    if pilot.engaged() {
        let state = sim.sim.state;
        let tm = sim.sim.telemetry;
        if let Some(r) = pilot.rudder(&cfg, &state, &tm, dt.max(1e-6)) {
            sim.controls.rudder = r;
        }
    } else {
        match *helm_source {
            HelmSource::Stick => sim.controls.rudder = stick as f64 * max_rudder,
            HelmSource::Keyboard => {
                if key_rate != 0.0 {
                    sim.controls.rudder =
                        (sim.controls.rudder + key_rate * dt).clamp(-max_rudder, max_rudder);
                } else if centring {
                    // Explicit, because an automatic one would undo every correction you
                    // make the moment you let go of the key.
                    let r = sim.controls.rudder;
                    let step = KEY_HELM_CENTRE * dt;
                    sim.controls.rudder = if r.abs() <= step {
                        0.0
                    } else {
                        r - step * r.signum()
                    };
                }
                // Otherwise: left exactly where you put it.
            }
        }
    }

    // --- Mainsheet: a tackle you pull and hold, so absolute ---------------------
    let sheet_rate = if fine { SHEET_FINE } else { SHEET_RATE };

    let mut main_set = false;
    if let Some(pad) = pad {
        let y = deadzone(pad.get(GamepadAxis::RightStickY).unwrap_or(0.0));
        if y != 0.0 {
            // Stick forward eases, stick back hauls in — the direction your arm moves.
            sim.controls.main_sheet = ((-y as f64) * 0.5 + 0.5).clamp(0.0, 1.0);
            main_set = true;
        }
    }
    if !main_set {
        let mut rate = 0.0;
        if keys.pressed(KeyCode::KeyW) {
            rate -= sheet_rate; // haul in
        }
        if keys.pressed(KeyCode::KeyS) {
            rate += sheet_rate; // ease
        }
        if rate != 0.0 {
            sim.controls.main_sheet = (sim.controls.main_sheet + rate * dt).clamp(0.0, 1.0);
        }
    }

    // --- Genoa sheet: a winch, so a rate ---------------------------------------
    let mut winch = 0.0;
    if let Some(pad) = pad {
        let grind = pad.get(GamepadButton::RightTrigger2).unwrap_or(0.0);
        let ease = pad.get(GamepadButton::LeftTrigger2).unwrap_or(0.0);
        winch += (ease - grind) as f64;
    }
    if keys.pressed(KeyCode::KeyQ) {
        winch += 1.0; // ease
    }
    if keys.pressed(KeyCode::KeyE) {
        winch -= 1.0; // grind in
    }
    if winch != 0.0 {
        sim.controls.jib_sheet =
            (sim.controls.jib_sheet + winch.clamp(-1.0, 1.0) * sheet_rate * dt).clamp(0.0, 1.0);
    }

    // --- The sail wardrobe ---------------------------------------------------
    // Z and X walk through it: genoa, Code 0, gennaker. They used to be the engine's
    // throttle, which is a fair trade — the boat has no engine and the thing you
    // actually change on a passage is which sail is up.
    //
    // Only with the sail rolled away, because that is the only way it happens on a
    // boat: you furl what is up, change it, and set the new one. A sail that teleports
    // into a different sail while drawing would be the one piece of this rig that
    // nobody could believe.
    let mut step = 0i32;
    if keys.just_pressed(KeyCode::KeyX) {
        step += 1;
    }
    if keys.just_pressed(KeyCode::KeyZ) {
        step -= 1;
    }
    if step != 0 {
        if sim.controls.furl > 0.9 {
            let order = [Headsail::Genoa, Headsail::Code0, Headsail::Gennaker];
            let at = order
                .iter()
                .position(|h| *h == sim.controls.headsail)
                .unwrap_or(0) as i32;
            let next = (at + step).clamp(0, order.len() as i32 - 1) as usize;
            if order[next] != sim.controls.headsail {
                sim.controls.headsail = order[next];
                info!("bent on the {}", order[next].name());
            }
        } else {
            info!("roll the headsail away first — furling line is on the bank");
        }
    }

    // --- Trim assist: a crew on the sheets for five seconds -------------------
    //
    // It used to trim once, on the keypress, which is the one moment it is least use.
    // The reason is the gybe: through a gybe the apparent wind swings across a hundred
    // and something degrees in a few seconds, and a trim taken at the start of it is
    // wrong before the boom has moved. What a crew does is *stay on the sheets* while
    // the boat comes round, and that is what this does now — five seconds of somebody
    // trimming, which is about as long as a gybe takes and not nearly long enough to
    // sail the boat for you.
    // --- Aback: the sheet on the wrong side, which is sometimes the right side -------
    //
    // A switch, not a key you hold down. It went in as a hold on the reasoning that it
    // is a rope you have in your hand, and that is wrong about the rope: a backed sheet
    // is made up on a cleat like every other sheet. Nobody stands there holding it — and
    // the case it exists for is *heaving to*, where the whole point is that she looks
    // after herself while you go below. A control you have to keep a finger on cannot
    // express a boat left to lie for an hour.
    //
    // It is the only way out of irons and half of what it means to heave to. Head to
    // wind with a genoa sheeted normally there is nothing to be done — the sail is at a
    // negative angle of attack and luffs whatever you do with the helm — and the boat
    // sits there going slowly backwards, which is a real and miserable state that this
    // simulator could get into and not out of.
    if keys.just_pressed(KeyCode::KeyG) || pad.is_some_and(|p| p.just_pressed(GamepadButton::East))
    {
        sim.controls.headsail_aback = !sim.controls.headsail_aback;
        info!(
            "{}",
            if sim.controls.headsail_aback {
                "headsail backed — she will pay off"
            } else {
                "headsail sheeted across"
            }
        );
    }

    // --- Dump: the other half of the sheet, and the one you need in a hurry ---------
    //
    // `T` hauls towards the right trim. There was nothing that let it *go*, and letting
    // it go is the thing you do first and fastest: a gust arrives, she lies over, and
    // what a crew does is dump the mainsheet — not trim it correctly, dump it — and sort
    // the shape out once she is on her feet again.
    //
    // Held rather than pressed, because that is what a hand on a clutch is. The rate is
    // the sheet running out through the jammer and it is quick: a main can be dumped in
    // a second and a half, which is the difference between a broach and a wobble.
    //
    // The mainsail first and then the headsail, which is the order that works and the
    // order a crew uses. The main is up high with a long lever and it is the sail that
    // spills cleanly: eased, it lets the top go first and the heeling moment comes off
    // before the drive does. A headsail eased just stops working. So the main goes to
    // the shrouds before the genoa is touched at all.
    //
    // It stops at the heel it is aiming for rather than running the sheets right out,
    // and it never hauls anything back in — this is a release, and a control that both
    // eased and trimmed would be `T` with a different name.
    if keys.pressed(KeyCode::KeyR) || pad.is_some_and(|p| p.pressed(GamepadButton::West)) {
        let heel = sim.sim.telemetry.heel.abs();
        let over = ((heel - sail_core::autopilot::OVERPOWERED_HEEL)
            / sail_core::autopilot::OVERPOWERED_HEEL)
            .clamp(0.0, 1.0) as f32;
        if over > 0.0 {
            // A second and a half of travel when she is right over, and still four
            // seconds when she is barely past the mark — because a hand on a clutch
            // does not meter the rope out in proportion to anything, it opens it. The
            // proportional part is there so the last of it comes off gently instead of
            // the sheet running out and the boom slamming against the shrouds.
            let run = (0.7 * (0.35 + 0.65 * over) * dt as f32).min(1.0);
            let main_room = 1.0 - sim.controls.main_sheet as f32;
            if main_room > 0.01 {
                sim.controls.main_sheet =
                    (sim.controls.main_sheet + run as f64).clamp(0.0, 1.0);
            } else {
                sim.controls.jib_sheet = (sim.controls.jib_sheet + run as f64).clamp(0.0, 1.0);
            }
            // And it takes the hands off whatever `T` was doing, or the two of them
            // fight over the same winch.
            *assisting = 0.0;
        }
    }

    // One sail at a time, and the order is the order a crew works in.
    //
    // It took both sheets at once, which is two people and this boat has one of you. A
    // press is a pair of hands going to *a* winch: the first press puts them on one
    // sail and the next press moves them to the other, which is also how you sail her
    // — you set one, look at it, and then set the other against what the first is
    // doing.
    //
    // Which one comes first is not a preference. Upwind the mainsail is the sail you
    // steer to and the headsail is set to the slot it leaves, so the main goes first.
    // From the broad reach on, the headsail is the engine and the main is a thing you
    // ease until it stops backwinding, so the headsail goes first. The changeover is at
    // the broad reach because that is where the headsail takes over the driving, and it
    // is read off the *true* wind angle — the apparent one moves with the boat's own
    // speed, and the point of sail does not.
    let pressed = keys.just_pressed(KeyCode::KeyT)
        || pad.is_some_and(|p| p.just_pressed(GamepadButton::North));
    if pressed {
        // A second press soon after the first moves to the other sail; after a pause the
        // hands have left the winch and a press starts the sequence again.
        *stage = if *assisting > 2.0 { 1 - *stage } else { 0 };
        *assisting = 5.0;
    }
    if *assisting > 0.0 {
        *assisting -= time.delta_secs();
        let awa = sim.sim.telemetry.awa;
        let headsail_first = sim.sim.telemetry.twa.abs() >= rad(120.0);
        let on_the_main = (*stage == 0) == !headsail_first;
        if on_the_main {
            // The outhaul goes with the mainsail, because it is the mainsail's.
            sim.controls.outhaul = outhaul_for(
                awa,
                sim.sim.telemetry.heel,
                sail_core::autopilot::OVERPOWERED_HEEL,
            );
        }
        let (m, j) = optimal_sheets_with(&cfg, awa, sim.controls.outhaul, sim.controls.headsail);
        // Hands, not servos: the sheet is hauled towards the right place at a winch's
        // pace rather than teleported to it. Through a gybe that is the difference
        // between a crew and a cheat.
        let haul = (SHEET_RATE * 2.2 * dt).min(1.0);
        if on_the_main {
            sim.controls.main_sheet += (m - sim.controls.main_sheet) * haul;
        } else {
            // Trimming the headsail means putting it where it belongs, which includes
            // the side it belongs on. Hands going to that winch is the moment a backed
            // sheet gets tacked across.
            sim.controls.headsail_aback = false;
            sim.controls.jib_sheet += (j - sim.controls.jib_sheet) * haul;
        }
        if pressed {
            info!(
                "on the {}",
                if on_the_main { "mainsheet" } else { "headsail sheet" }
            );
        }
    }
}
