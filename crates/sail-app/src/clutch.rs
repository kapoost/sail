//! The clutch bank.
//!
//! How a cruising boat is actually worked. The control lines — outhaul, vang, reefing
//! pennants, the furling line, the genoa car — all run aft to a bank of rope clutches
//! at the cabin top, and they share a winch. You cannot adjust two of them at once:
//! you put one on the drum, open its clutch, work it, close it again. Everything else
//! sits locked while you do.
//!
//! That is a constraint, and it is the right one. The alternative — a key pair per
//! line — gives the player ten simultaneous controls that no crew has ever had, and a
//! keyboard cannot express the thing that actually makes sailing hard, which is that
//! jobs take time and you can only do one of them. Sequencing is also what a truck
//! simulator does with its dozens of switches, and for the same reason.
//!
//! The sheets are deliberately *not* in the bank. They live on their own winches and
//! get trimmed continuously, so they keep direct keys. That split is not a compromise
//! — it is how the deck is laid out.

use bevy::prelude::*;
use sail_core::prelude::*;

use crate::sim::Sim;

/// A line on a clutch.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Line {
    Outhaul,
    Vang,
    GenoaCar,
    Reef,
    Furl,
    Staysail,
}

pub const BANK: [Line; 6] = [
    Line::Outhaul,
    Line::Vang,
    Line::GenoaCar,
    Line::Reef,
    Line::Furl,
    Line::Staysail,
];

impl Line {
    pub fn name(self) -> &'static str {
        match self {
            Line::Outhaul => "outhaul",
            Line::Vang => "vang",
            Line::GenoaCar => "genoa car",
            Line::Reef => "reef",
            Line::Furl => "furling line",
            Line::Staysail => "staysail",
        }
    }

    /// What hauling it in does, in words, since none of these are obvious from a
    /// number between nought and one.
    pub fn hauled(self) -> &'static str {
        match self {
            Line::Outhaul => "flattens the foot",
            Line::Vang => "takes the twist out",
            Line::GenoaCar => "car aft: flat, open leech",
            Line::Reef => "shortens the main",
            Line::Furl => "rolls the genoa away",
            Line::Staysail => "sets the inner sail",
        }
    }

    pub fn get(self, c: &Controls) -> f64 {
        match self {
            Line::Outhaul => c.outhaul,
            Line::Vang => c.vang,
            Line::GenoaCar => c.genoa_car,
            Line::Reef => c.reef,
            Line::Furl => c.furl,
            Line::Staysail => c.staysail,
        }
    }

    /// Set directly, for the console's drag handles. The keys go through the winch
    /// rate; a mouse on the lever does not, because the hand is the rate.
    pub fn set_from_ui(self, c: &mut Controls, v: f64) {
        self.set(c, v);
    }

    fn set(self, c: &mut Controls, v: f64) {
        let v = v.clamp(0.0, 1.0);
        match self {
            Line::Outhaul => c.outhaul = v,
            Line::Vang => c.vang = v,
            Line::GenoaCar => c.genoa_car = v,
            Line::Reef => c.reef = v,
            Line::Furl => c.furl = v,
            Line::Staysail => c.staysail = v,
        }
    }

    /// How fast it comes in, in fraction per second.
    ///
    /// Not all the same, and the differences are the point. A reef is a job: you round
    /// up, drop the halyard, hook the tack, winch the clew down. It takes time, and if
    /// it did not then reefing early would cost nothing and the decision would carry
    /// no weight at all.
    fn rate(self) -> f64 {
        match self {
            Line::Outhaul => 0.30,
            Line::Vang => 0.35,
            Line::GenoaCar => 0.28,
            Line::Reef => 0.085,
            Line::Furl => 0.14,
            // A halyard, by hand. Slower than anything else on the bank, because that
            // is what setting a sail is — and because the decision to put it up ought
            // to be made before you need it rather than while you do.
            Line::Staysail => 0.11,
        }
    }
}

/// Which line is on the winch.
#[derive(Resource, Default)]
pub struct Bank {
    pub selected: usize,
    /// Seconds since the selection last changed, for a brief highlight.
    pub since_change: f32,
}

impl Bank {
    pub fn line(&self) -> Line {
        BANK[self.selected % BANK.len()]
    }
}

/// Select a line, then work it. Two keys to choose and two to pull.
pub fn handle(
    mut sim: ResMut<Sim>,
    mut bank: ResMut<Bank>,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    let dt = time.delta_secs_f64();
    bank.since_change += time.delta_secs();

    if keys.just_pressed(KeyCode::Digit2) {
        bank.selected = (bank.selected + 1) % BANK.len();
        bank.since_change = 0.0;
    }
    if keys.just_pressed(KeyCode::Digit1) {
        bank.selected = (bank.selected + BANK.len() - 1) % BANK.len();
        bank.since_change = 0.0;
    }

    let fine = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let line = bank.line();
    let rate = line.rate() * if fine { 0.3 } else { 1.0 };

    let mut change = 0.0;
    if keys.pressed(KeyCode::KeyD) {
        change += rate; // haul in
    }
    if keys.pressed(KeyCode::KeyA) {
        change -= rate; // ease
    }
    if change != 0.0 {
        let now = line.get(&sim.controls);
        line.set(&mut sim.controls, now + change * dt);
    }
}
