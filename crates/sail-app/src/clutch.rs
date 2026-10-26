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

/// The bank of clutches, and what is on each of them.
///
/// What this module does in a frame, in one place — see `stages.rs` for why the
/// schedule is named sets rather than one long chain.
pub fn plugin(app: &mut App) {
    use crate::stages::Deck;
    app.init_resource::<Bank>();
    app.add_systems(Update, handle.in_set(Deck::Intent));
}

/// A line on a clutch.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Line {
    Outhaul,
    Vang,
    GenoaCar,
    Reef,
    Furl,
    Staysail,
    StaysailSheet,
    /// Storm canvas aft, and the last line on the bank because it is the last thing
    /// anybody touches: it only does something with the main off her.
    Trysail,
}

pub const BANK: [Line; 8] = [
    Line::Outhaul,
    Line::Vang,
    Line::GenoaCar,
    Line::Reef,
    Line::Furl,
    Line::Staysail,
    Line::StaysailSheet,
    Line::Trysail,
];

impl Line {
    /// Where this line sits on the bank.
    ///
    /// Three places were each writing `BANK.iter().position(|l| *l == Line::Reef)` to
    /// find out, which is the kind of duplication that survives until somebody reorders
    /// the bank and two of the three are fixed.
    pub fn on_the_bank(self) -> usize {
        BANK.iter()
            .position(|l| *l == self)
            .expect("every line is on the bank")
    }

    /// Whether this line does anything with the sail that is up.
    ///
    /// The genoa car leads the genoa's sheet, and a Code 0 or a gennaker is not sheeted
    /// to it — it goes to a block on the quarter, right aft, which is a shackle and not
    /// a control. Reported from the gear rather than hidden, because a line that is
    /// there and does nothing is a thing a boat genuinely has.
    pub fn rigged_for(self, c: &Controls) -> bool {
        match self {
            Line::GenoaCar => c.headsail == sail_core::state::Headsail::Genoa,
            // The trysail goes up the mast on the track the mainsail is on, so with the
            // main bent on there is nowhere for it to go. Said by the bank rather than
            // hidden, because the halyard is there and a hand can pull it: it is a line
            // that does nothing *yet*, which is a different thing from a line that is
            // not rigged.
            Line::Trysail => c.reef >= sail_core::boat::DROPPED,
            _ => true,
        }
    }

    /// What the line is called with the sail in its present state.
    ///
    /// The outhaul becomes the reef pennant the moment a slab goes in, because that is
    /// what is on the other end of the hand: the outhaul is shackled to the old clew,
    /// which is bunched along the boom pulling on nothing, and the foot is held out by
    /// the pennant through the cheek block at the boom end. One control, one decision,
    /// a different rope — and the bank ought to say which rope.
    /// The positions this line rests at, if it has any.
    ///
    /// **Three slabs and then she comes off her**, which is five positions: full, one,
    /// two, three, handed. It was four — two slabs and handed — and the jump from the
    /// second reef to no mainsail at all is the wrong size: it is the step you need in
    /// the forties, which is exactly where a cruising main has a third row of points
    /// sewn into it. Reported from the deck as a missing reef.
    ///
    /// **And the roller genoa has them too, which is a change of mind worth writing
    /// down.** This said a drum stops wherever you stop it and that sailing with a third
    /// of it rolled away is an ordinary thing to do. Both of those are true about a
    /// boat, and both were wrong about a *game*: expressing "a third away" meant holding
    /// a key and watching a number, which is the winch's view of the job rather than the
    /// person's. A roller headsail is reefed in rolls — you put a reef in it, you put
    /// another one in — so the stops are those reefs, and getting to one still takes the
    /// drum as long as it takes.
    ///
    /// A sail set flying has two stops and they are the only two: up, or away. So does
    /// the staysail, which is hanked on and hoisted and has no middle state either.
    pub fn detents(self, c: &Controls) -> Option<&'static [f64]> {
        const SLABS: [f64; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];
        const EITHER: [f64; 2] = [0.0, 1.0];
        match self {
            Line::Reef => Some(&SLABS),
            Line::Staysail | Line::Trysail => Some(&EITHER),
            Line::Furl if c.headsail != sail_core::state::Headsail::Genoa => Some(&EITHER),
            Line::Furl => Some(&SLABS),
            _ => None,
        }
    }

    /// What a detented line says it is *at*, rather than how far along it is.
    ///
    /// **A position is not a number.** `0.75` on a reef line means nothing to anybody;
    /// *third reef* means the thing you did and the thing you can undo. A line with
    /// stops has names for them and a line without has none — which is also how the
    /// instrument says which kind of control it is, without a word of explanation.
    pub fn position(self, c: &Controls) -> Option<&'static str> {
        let stops = self.detents(c)?;
        let v = self.get(c);
        let (mut which, mut best) = (0, f64::MAX);
        for (i, stop) in stops.iter().enumerate() {
            let off = (stop - v).abs();
            if off < best {
                (which, best) = (i, off);
            }
        }
        self.stop_called(c, which as i32)
    }

    /// Which stop she is at, as an index.
    pub fn stop_index(self, c: &Controls) -> Option<usize> {
        let stops = self.detents(c)?;
        let v = self.get(c);
        let (mut which, mut best) = (0, f64::MAX);
        for (i, stop) in stops.iter().enumerate() {
            let off = (stop - v).abs();
            if off < best {
                (which, best) = (i, off);
            }
        }
        Some(which)
    }

    /// What *any* stop is called, not only the one she is at.
    ///
    /// **Because a button should say what it does.** Reported from the deck: the
    /// staysail's buttons said HAUL and EASE, which is what a hand does to a rope and
    /// tells nobody whether the sail is going up or coming down. A line with positions
    /// has names for them, so the button can carry the name of the position it is about
    /// to put her at — *set*, *handed*, *2nd reef* — and then there is nothing to work
    /// out at all.
    pub fn stop_called(self, c: &Controls, which: i32) -> Option<&'static str> {
        let stops = self.detents(c)?;
        if which < 0 || which as usize >= stops.len() {
            return None;
        }
        let which = which as usize;
        Some(match (self, which, stops.len()) {
            (Line::Reef, 0, _) => "full",
            (Line::Reef, 1, _) => "1st reef",
            (Line::Reef, 2, _) => "2nd reef",
            (Line::Reef, 3, _) => "3rd reef",
            (Line::Reef, _, _) => "handed",
            // A roller headsail is reefed in rolls, and the names are the ones anybody
            // uses for them: full, a reef in it, another reef, and rolled away.
            (Line::Furl, 0, 5) => "full",
            (Line::Furl, 1, 5) => "1st roll",
            (Line::Furl, 2, 5) => "2nd roll",
            (Line::Furl, 3, 5) => "3rd roll",
            (Line::Furl, _, 5) => "away",
            (_, 0, _) => "down",
            (_, _, _) => "set",
        })
    }

    /// The name as it is engraved on the plate under the lever, which has room for
    /// eight letters and not fourteen.
    ///
    /// The long name is still said under the rack, where there is a line for it: *a
    /// nameplate is a label, and the sentence explaining it is a different object.*
    pub fn tag(self, c: &Controls) -> &'static str {
        match self {
            Line::Outhaul if c.reef > 0.02 => "pennant",
            Line::Outhaul => "outhaul",
            Line::Vang => "vang",
            Line::GenoaCar => "car",
            Line::Reef => "reef",
            Line::Furl => "furler",
            Line::Staysail => "stays'l",
            Line::StaysailSheet => "st sheet",
            Line::Trysail => "trysail",
        }
    }

    pub fn name_now(self, c: &Controls) -> &'static str {
        match self {
            Line::Outhaul if c.reef > 0.02 => "reef pennant",
            _ => self.name(),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Line::Outhaul => "outhaul",
            Line::Vang => "vang",
            Line::GenoaCar => "genoa car",
            Line::Reef => "reef",
            Line::Furl => "furling line",
            Line::Staysail => "staysail",
            Line::StaysailSheet => "stays'l sheet",
            Line::Trysail => "trysail",
        }
    }

    /// What hauling it in does, in words, since none of these are obvious from a
    /// number between nought and one.
    pub fn hauled_now(self, c: &Controls) -> &'static str {
        match self {
            Line::Outhaul if c.reef > 0.02 => "flattens the reefed foot",
            Line::Furl if c.headsail != sail_core::state::Headsail::Genoa => {
                "hands the sail — there is no half of it"
            }
            _ => self.hauled(),
        }
    }

    pub fn hauled(self) -> &'static str {
        match self {
            Line::Outhaul => "flattens the foot",
            Line::Vang => "takes the twist out",
            Line::GenoaCar => "car aft: flat, open leech",
            Line::Reef => "shortens the main",
            Line::Furl => "rolls the headsail away",
            Line::Staysail => "sets the inner sail",
            Line::StaysailSheet => "brings the club boom in",
            Line::Trysail => "sets it — only with the main off her",
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
            // Backwards, like the other sheets on the bank: hauling in is a smaller
            // number, because what the line does is shorten.
            Line::StaysailSheet => 1.0 - c.staysail_sheet,
            Line::Trysail => c.trysail,
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
            Line::StaysailSheet => c.staysail_sheet = 1.0 - v,
            Line::Trysail => c.trysail = v,
        }
    }

    /// How fast it comes in, in fraction per second.
    ///
    /// Not all the same, and the differences are the point. A reef is a job: you round
    /// up, drop the halyard, hook the tack, winch the clew down. It takes time, and if
    /// it did not then reefing early would cost nothing and the decision would carry
    /// no weight at all.
    pub fn rate(self, c: &Controls) -> f64 {
        match self {
            // A reef pennant takes the whole load of the foot and comes in on a winch,
            // where an outhaul is a light tackle you pull by hand.
            Line::Outhaul if c.reef > 0.02 => 0.17,
            Line::Outhaul => 0.30,
            Line::Vang => 0.35,
            Line::GenoaCar => 0.28,
            Line::Reef => 0.085,
            // A headsail comes in round its own stay against the full load of the sail,
            // which is why there is a winch on the furling line and why putting a reef
            // in one is a minute of work rather than a flick. The same rate as a reef
            // pennant, which is the other line on this boat that shortens sail.
            Line::Furl => 0.085,
            // A halyard, by hand. Slower than anything else on the bank, because that
            // is what setting a sail is — and because the decision to put it up ought
            // to be made before you need it rather than while you do.
            Line::Staysail => 0.11,
            Line::StaysailSheet => 0.26,
            // Slower again, and it is the slowest line on the boat. Bending a trysail
            // on is not hoisting a sail that is already hanked and ready: it comes out
            // of its bag on a heaving foredeck, the slides go into the gate one at a
            // time, and the sheets are led aft to the quarters. Twenty-odd seconds of
            // it here, which is as much of that as a game should ask for and still
            // enough that nobody does it as an afterthought.
            Line::Trysail => 0.045,
        }
    }
}

impl Bank {
    /// Bend the trysail on, taking the mainsail off her first if it is still up.
    ///
    /// **One call, because on deck it is one decision.** The winch's view of this is
    /// four steps — select the reef line, haul to handed, select the halyard, haul —
    /// and the person's view is *get the main off her and the trysail up*, which is the
    /// same argument the headsail list won.
    ///
    /// Both lines are given a destination and left to run there at their own rates, and
    /// that puts them in the right order for nothing: the reef line is nearly twice as
    /// fast as the halyard, so the main is down well before the trysail is up whether or
    /// not anybody thought about it. Which is also the order it has to happen in, since
    /// they are on the same track.
    pub fn bend_on_the_trysail(&mut self, controls: &Controls) {
        if controls.reef < sail_core::boat::DROPPED {
            self.target[Line::Reef.on_the_bank()] = Some(1.0);
        }
        self.target[Line::Trysail.on_the_bank()] = Some(1.0);
    }

    /// And take it in again. The main stays where it is: shaking three slabs out in a
    /// gale is a separate decision and a much later one.
    pub fn hand_the_trysail(&mut self) {
        self.target[Line::Trysail.on_the_bank()] = Some(0.0);
    }

    /// Whether either of those jobs is still running.
    pub fn working_the_trysail(&self, controls: &Controls) -> bool {
        self.target[Line::Trysail.on_the_bank()].is_some()
            || (controls.reef < sail_core::boat::DROPPED
                && self.target[Line::Reef.on_the_bank()].is_some())
    }
}

/// Which line is on the winch.
#[derive(Resource, Default)]
pub struct Bank {
    pub selected: usize,
    /// Seconds since the selection last changed, for a brief highlight.
    pub since_change: f32,
    /// Where a detented line is being hauled to, if it is on its way somewhere.
    ///
    /// A slab reef has positions, not a range: there is a first reef, a second and a
    /// third, and nothing in between but a sail half pulled down. But getting there is
    /// not instant — you ease the halyard, haul the pennant, winch it home — so the
    /// line runs at its own pace and *stops at the cringle*. One press starts the job;
    /// the job takes as long as it takes.
    pub target: [Option<f64>; BANK.len()],
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
    sound: Res<crate::sound::SoundHandle>,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    let dt = time.delta_secs_f64();
    bank.since_change += time.delta_secs();

    // A clutch is a cam with a spring behind it and it lands with a bang you can hear
    // from the foredeck. Rated low: it is a sharp sound rather than a loud one, and a
    // boom arriving must still be the loudest thing on the boat.
    if keys.just_pressed(KeyCode::Digit1) || keys.just_pressed(KeyCode::Digit2) {
        sound.0.clack(0.35);
    }
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
    let selected = bank.selected;
    let rate = line.rate(&sim.controls) * if fine { 0.3 } else { 1.0 };

    // A detented line is given a destination and left to get there. Everything else
    // moves while the key is down, which is what a tackle or a drum does.
    if let Some(stops) = line.detents(&sim.controls) {
        let now = line.get(&sim.controls);
        let at = bank.target[selected].unwrap_or(now);
        if keys.just_pressed(KeyCode::KeyD) {
            bank.target[selected] = stops.iter().copied().find(|s| *s > at + 1e-6).or(Some(1.0));
        }
        if keys.just_pressed(KeyCode::KeyA) {
            bank.target[selected] = stops
                .iter()
                .rev()
                .copied()
                .find(|s| *s < at - 1e-6)
                .or(Some(0.0));
        }
    } else {
        bank.target[selected] = None;
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

    // Every line that has somewhere to be runs there, selected or not: a reef you
    // started does not stop because you turned to the vang.
    for (i, l) in BANK.iter().enumerate() {
        let Some(want) = bank.target[i] else {
            continue;
        };
        if l.detents(&sim.controls).is_none() {
            bank.target[i] = None;
            continue;
        }
        let now = l.get(&sim.controls);
        let step = l.rate(&sim.controls) * dt;
        if (want - now).abs() <= step {
            l.set(&mut sim.controls, want);
            bank.target[i] = None;
        } else {
            l.set(&mut sim.controls, now + step * (want - now).signum());
        }
    }
}

#[cfg(test)]
mod the_bank {
    use super::*;

    /// **One click takes the main off her and puts the trysail up**, in that order,
    /// which is the order the mast track forces and the order anybody would do it in.
    ///
    /// Asked for from the deck in exactly those words. The failure this guards against
    /// is the quiet one: a button that hoists the trysail and leaves the mainsail bent
    /// on sets nothing at all — they share a track — so the halyard runs, the control
    /// says SET, and she is still under a reefed main in a gale.
    #[test]
    fn bending_the_trysail_on_takes_the_main_off_her_first() {
        let mut bank = Bank::default();
        let third = Controls {
            reef: 0.75,
            ..Default::default()
        };
        bank.bend_on_the_trysail(&third);
        assert_eq!(
            bank.target[Line::Reef.on_the_bank()],
            Some(1.0),
            "the main was left bent on"
        );
        assert_eq!(bank.target[Line::Trysail.on_the_bank()], Some(1.0));

        // And with the main already down it is one job, not two: nobody re-hauls a reef
        // line that is already home.
        let mut bank = Bank::default();
        let handed = Controls {
            reef: 1.0,
            ..Default::default()
        };
        bank.bend_on_the_trysail(&handed);
        assert_eq!(bank.target[Line::Reef.on_the_bank()], None);
        assert_eq!(bank.target[Line::Trysail.on_the_bank()], Some(1.0));
    }

    /// **And the order comes out of the rates rather than out of a sequencer.** The
    /// reef line is the faster of the two, so the main is down before the halyard is
    /// home — which is what makes "both at once" the right instruction instead of a
    /// bug waiting for a slow machine.
    #[test]
    fn the_main_is_down_before_the_trysail_is_up() {
        let handed = Controls {
            reef: 1.0,
            ..Default::default()
        };
        let reefing = Line::Reef.rate(&handed);
        let hoisting = Line::Trysail.rate(&handed);
        assert!(
            reefing > hoisting * 1.5,
            "the reef line runs at {reefing} and the halyard at {hoisting}"
        );
    }

    /// Handing it leaves the main alone: shaking three slabs out in a gale is a
    /// separate decision and a much later one.
    #[test]
    fn handing_the_trysail_does_not_shake_the_reefs_out() {
        let mut bank = Bank::default();
        bank.hand_the_trysail();
        assert_eq!(bank.target[Line::Trysail.on_the_bank()], Some(0.0));
        assert_eq!(bank.target[Line::Reef.on_the_bank()], None);
    }

    /// **Every stop on the reef line has a name, and they are the right names.**
    ///
    /// Three slabs and then she comes off her. It was two and a jump to nothing, which
    /// is the wrong step in the forties — reported from the deck as a missing reef.
    #[test]
    fn the_reef_line_rests_at_five_named_places() {
        let stops = Line::Reef
            .detents(&Controls::default())
            .expect("a slab reef has positions");
        assert_eq!(stops.len(), 5, "full, three slabs, handed");
        let said: Vec<&str> = stops
            .iter()
            .map(|stop| {
                let c = Controls {
                    reef: *stop,
                    ..Default::default()
                };
                Line::Reef.position(&c).expect("a named position")
            })
            .collect();
        assert_eq!(said, ["full", "1st reef", "2nd reef", "3rd reef", "handed"]);
    }

    /// A line that stops wherever you stop it has no positions to name.
    ///
    /// Which is what tells the instrument which kind of control it is drawing: notches
    /// and a name for *which*, a column and a figure for *how much*.
    #[test]
    fn a_line_without_stops_has_nothing_to_name() {
        let c = Controls::default();
        assert!(Line::Vang.detents(&c).is_none());
        assert!(Line::Vang.position(&c).is_none());
        // And the furling line *does* have them now: a roller headsail is reefed in
        // rolls, and the stops are those reefs. The flying sails keep their two.
        assert_eq!(Line::Furl.position(&c), Some("full"));
        let flying = Controls {
            headsail: sail_core::state::Headsail::Code0,
            ..Default::default()
        };
        assert_eq!(Line::Furl.detents(&flying).map(|d| d.len()), Some(2));
    }

    /// Half-way between two slabs she is at the nearer one, not at neither.
    #[test]
    fn a_line_between_stops_reports_the_nearest() {
        let c = Controls {
            reef: 0.47,
            ..Default::default()
        };
        assert_eq!(Line::Reef.position(&c), Some("2nd reef"));
    }
}
