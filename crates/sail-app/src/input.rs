//! Player intent to boat controls.
//!
//! The design decision that makes a sailing sim playable at all: you do not map lines
//! to keys. A boat has a dozen of them and sailing needs several at once, which no
//! keyboard affords. You map *intentions*, and you match the control's nature to the
//! gear it stands for.
//!
//! * Helm is **absolute**, because a tiller has a position your hand is holding.
//! * The mainsheet is **absolute**, because it is a tackle you pull and hold.
//! * The genoa sheet is a **rate**, because it is a winch you grind — and grinding
//!   takes time you do not always have, which is the whole drama of a tack.
//! * Reefing is **discrete**, because it is a job you do, not a dial you turn.

use bevy::prelude::*;
use sail_core::prelude::*;

use crate::sim::Sim;

/// Dead zone on the sticks. Analog hardware drifts, and a drifting helm is maddening.
const STICK_DEADZONE: f32 = 0.12;
/// How fast the genoa winch takes up sheet, in sheet fraction per second.
const WINCH_RATE: f64 = 0.55;
/// How fast the keyboard helm swings, radians per second.
const KEY_HELM_RATE: f64 = 0.9;
/// How fast an unattended keyboard helm returns amidships.
const KEY_HELM_CENTRE: f64 = 1.6;

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
) {
    let dt = time.delta_secs_f64();
    let cfg = sim.config();
    let max_rudder = cfg.max_rudder_angle;

    let pad = pads.iter().next();

    // --- Helm -----------------------------------------------------------------
    let mut helm_set = false;
    if let Some(pad) = pad {
        let x = deadzone(pad.get(GamepadAxis::LeftStickX).unwrap_or(0.0));
        if x != 0.0 {
            sim.controls.rudder = x as f64 * max_rudder;
            helm_set = true;
        }
    }
    if !helm_set {
        let mut rate = 0.0;
        if keys.pressed(KeyCode::KeyA) {
            rate -= KEY_HELM_RATE;
        }
        if keys.pressed(KeyCode::KeyD) {
            rate += KEY_HELM_RATE;
        }
        if rate != 0.0 {
            sim.controls.rudder = (sim.controls.rudder + rate * dt).clamp(-max_rudder, max_rudder);
        } else if pad.is_none() {
            // Let go of the tiller and it drifts back amidships.
            let r = sim.controls.rudder;
            let step = KEY_HELM_CENTRE * dt;
            sim.controls.rudder = if r.abs() <= step {
                0.0
            } else {
                r - step * r.signum()
            };
        } else {
            sim.controls.rudder = 0.0;
        }
    }

    // --- Mainsheet: a tackle, so absolute -------------------------------------
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
            rate -= WINCH_RATE;
        }
        if keys.pressed(KeyCode::KeyS) {
            rate += WINCH_RATE;
        }
        if rate != 0.0 {
            sim.controls.main_sheet = (sim.controls.main_sheet + rate * dt).clamp(0.0, 1.0);
        }
    }

    // --- Genoa sheet: a winch, so a rate --------------------------------------
    let mut winch = 0.0;
    if let Some(pad) = pad {
        let grind = pad.get(GamepadButton::RightTrigger2).unwrap_or(0.0);
        let ease = pad.get(GamepadButton::LeftTrigger2).unwrap_or(0.0);
        winch += (ease - grind) as f64;
    }
    if keys.pressed(KeyCode::KeyQ) {
        winch += 1.0;
    }
    if keys.pressed(KeyCode::KeyE) {
        winch -= 1.0;
    }
    if winch != 0.0 {
        sim.controls.jib_sheet =
            (sim.controls.jib_sheet + winch.clamp(-1.0, 1.0) * WINCH_RATE * dt).clamp(0.0, 1.0);
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
        let (m, j) = optimal_sheets(&cfg, awa);
        sim.controls.main_sheet = m;
        sim.controls.jib_sheet = j;
    }
}
