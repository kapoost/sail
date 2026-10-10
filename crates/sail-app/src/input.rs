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
//! * Reefing is **discrete**, because it is a job you do, not a dial you turn.
//! * The outhaul is a **slow rate**, because it is a line you haul and cleat and then
//!   leave alone for an hour.
//!
//! The autopilot disengages the moment you touch the helm. Real pilots have a clutch
//! you have to throw, but a boat that fights your hand is worse than one that lets go.

use bevy::prelude::*;
use sail_core::prelude::*;

use crate::sim::Sim;

/// Dead zone on the sticks. Analog hardware drifts, and a drifting helm is maddening.
const STICK_DEADZONE: f32 = 0.12;

/// The autopilot, as a Bevy resource.
///
/// A newtype only because `Resource` cannot be derived on a type from another crate —
/// which is the small, honest tax for keeping `sail-core` free of engine types.
#[derive(Resource, Default, Deref, DerefMut)]
pub struct Pilot(pub Autopilot);

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
/// How fast the outhaul hauls, in fraction per second. Slow: it is a cleated line
/// you adjust when the breeze changes, not something you play with.
const OUTHAUL_RATE: f64 = 0.30;
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

fn deadzone(v: f32) -> f32 {
    if v.abs() < STICK_DEADZONE {
        0.0
    } else {
        (v.abs() - STICK_DEADZONE) / (1.0 - STICK_DEADZONE) * v.signum()
    }
}

pub fn gather(
    mut sim: ResMut<Sim>,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    mut helm_source: ResMut<HelmSource>,
    mut pilot: ResMut<Pilot>,
) {
    let dt = time.delta_secs_f64();
    let cfg = sim.config();
    let max_rudder = cfg.max_rudder_angle;

    let pad = pads.iter().next();

    // --- Autopilot -------------------------------------------------------------
    let cycle = keys.just_pressed(KeyCode::KeyP)
        || pad.is_some_and(|p| p.just_pressed(GamepadButton::Start));
    if cycle {
        match pilot.mode {
            PilotMode::Standby => pilot.hold_heading(&sim.sim.state),
            PilotMode::Heading(_) => {
                let tm = sim.sim.telemetry;
                pilot.hold_wind(&tm);
            }
            PilotMode::ApparentWind(_) => pilot.disengage(),
        }
    }
    let nudge = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
        rad(5.0)
    } else {
        rad(1.0)
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
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        key_rate -= rate;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        key_rate += rate;
    }
    let centring = keys.pressed(KeyCode::Space);
    if key_rate != 0.0 || centring {
        *helm_source = HelmSource::Keyboard;
    }

    // Touching the helm throws the pilot out.
    if pilot.engaged() && (key_rate != 0.0 || centring || stick != 0.0) {
        pilot.disengage();
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

    // --- Outhaul: mainsail depth ----------------------------------------------
    // Eased is deep and powerful, hauled out is flat with less heeling force. The
    // right setting changes with the breeze, which is what makes it a control.
    let mut outhaul_rate = 0.0;
    if keys.pressed(KeyCode::Digit1) {
        outhaul_rate -= OUTHAUL_RATE; // ease: deeper
    }
    if keys.pressed(KeyCode::Digit2) {
        outhaul_rate += OUTHAUL_RATE; // haul out: flatter
    }
    if outhaul_rate != 0.0 {
        sim.controls.outhaul = (sim.controls.outhaul + outhaul_rate * dt).clamp(0.0, 1.0);
    }

    // --- Headsail furl: a drum you roll, so a toggle plus a rate ---------------
    // Separate gear from the main's reef, and the first thing anyone touches when it
    // blows up. G rolls it fully away or fully out; the rate keys give part furls.
    let furl_toggle = keys.just_pressed(KeyCode::KeyG)
        || pad.is_some_and(|p| p.just_pressed(GamepadButton::West));
    if furl_toggle {
        sim.controls.furl = if sim.controls.furl > 0.5 { 0.0 } else { 1.0 };
    }
    let mut furl_rate = 0.0;
    if keys.pressed(KeyCode::BracketRight) {
        furl_rate += 0.35;
    }
    if keys.pressed(KeyCode::BracketLeft) {
        furl_rate -= 0.35;
    }
    if furl_rate != 0.0 {
        sim.controls.furl = (sim.controls.furl + furl_rate * dt).clamp(0.0, 1.0);
    }

    // --- Reef: a job, so discrete ---------------------------------------------
    let reef_in = keys.just_pressed(KeyCode::KeyR)
        || pad.is_some_and(|p| p.just_pressed(GamepadButton::DPadDown));
    let reef_out = keys.just_pressed(KeyCode::KeyF)
        || pad.is_some_and(|p| p.just_pressed(GamepadButton::DPadUp));
    if reef_in {
        sim.controls.reef = (sim.controls.reef + 0.25).min(1.0);
    }
    if reef_out {
        sim.controls.reef = (sim.controls.reef - 0.25).max(0.0);
    }

    // --- Auxiliary engine ------------------------------------------------------
    let mut throttle_step = 0.0;
    if keys.just_pressed(KeyCode::KeyX) {
        throttle_step += 0.25;
    }
    if keys.just_pressed(KeyCode::KeyZ) {
        throttle_step -= 0.25;
    }
    if let Some(p) = pad {
        if p.just_pressed(GamepadButton::DPadRight) {
            throttle_step += 0.25;
        }
        if p.just_pressed(GamepadButton::DPadLeft) {
            throttle_step -= 0.25;
        }
    }
    if throttle_step != 0.0 {
        sim.controls.throttle = (sim.controls.throttle + throttle_step).clamp(-1.0, 1.0);
    }

    // --- Trim assist: show me the line a good trimmer would be on -------------
    let assist = keys.just_pressed(KeyCode::KeyT)
        || pad.is_some_and(|p| p.just_pressed(GamepadButton::North));
    if assist {
        let awa = sim.sim.telemetry.awa;
        let tws = sim.sim.telemetry.tws;
        sim.controls.outhaul = optimal_outhaul(tws);
        let (m, j) = optimal_sheets(&cfg, awa, sim.controls.outhaul);
        sim.controls.main_sheet = m;
        sim.controls.jib_sheet = j;
    }
}
