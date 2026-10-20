//! Standing orders: what you have asked for, carried out at the pace of the deck.
//!
//! **A boat is worked in states, not in keystrokes.** You do not hold a button down to
//! roll a genoa; you decide that the number two is what should be up, and then somebody
//! spends four minutes making that true. Every control on this boat that has an obvious
//! *destination* is being moved to that shape — the reef has stops, the staysail is up or
//! down, and a sail change is now a thing you ask for rather than a sequence you perform.
//!
//! The sequence itself has not gone anywhere, and that is the whole point: ordering the
//! Code 0 still means the genoa is rolled away first, the new sail is bent on with the bow
//! clear, and then it is set — a minute of foredeck work you can watch happening, at the
//! drum's own rate. What has gone is the clicking. Asking for the end state and watching
//! the deck get there is what it is like to be the person who decided; holding a key until
//! a number reaches 1.00 is what it is like to be the winch.
//!
//! One executor, used by the player's order and by the watch's own decision, because two
//! copies of a sail change is two sail changes that disagree — which is this repository's
//! oldest fault wearing a new hat.

use bevy::prelude::*;
use sail_core::state::Headsail;

use crate::clutch::Line;
use crate::sim::Sim;

/// What the watch has been told to do.
///
/// What this module does in a frame, in one place — see `stages.rs` for why the
/// schedule is named sets rather than one long chain.
pub fn plugin(app: &mut App) {
    use crate::stages::Deck;
    app.init_resource::<Orders>();
    app.add_systems(Update, carry_out.in_set(Deck::Weather));
}

/// What has been asked for and not yet done.
#[derive(Resource, Default)]
pub struct Orders {
    /// The sail that should be on the bow. `None` once it is.
    pub headsail: Option<Headsail>,
    /// What the foredeck is at, for the console to say.
    pub doing: Option<&'static str>,
}

impl Orders {
    /// Ask for a sail. Ordering the one already set cancels an order in progress, which
    /// is what *belay that* means and is the only way to change your mind half way
    /// through a change.
    pub fn set(&mut self, sim: &Sim, which: Headsail) {
        if self.headsail == Some(which) || sim.controls.headsail == which && self.headsail.is_none()
        {
            self.headsail = None;
            return;
        }
        self.headsail = Some(which);
    }

    /// Whether a change is under way.
    pub fn changing(&self) -> bool {
        self.headsail.is_some()
    }
}

/// Carry out a sail change: roll the old one away, bend the new one on, set it.
///
/// Returns whether the foredeck is busy, so that whoever called knows not to put hands on
/// anything else — there is one winch forward and one crew to turn it.
///
/// This is the one copy. The watch calls it with its own decision and the console calls it
/// with yours; the only difference between them is who did the deciding.
pub fn work_the_change(sim: &mut Sim, orders: &mut Orders, dt: f64, say: bool) -> bool {
    let Some(to) = orders.headsail else {
        orders.doing = None;
        return false;
    };
    // **Worked right to its end rather than to within a tenth of it.** A drum nine
    // tenths home is a sail still on the bow, and a change that stops there stands half
    // done for as long as anybody watches it.
    let bent_on = sim.controls.headsail == to;
    let want = if bent_on { 0.0 } else { 1.0 };
    let step = Line::Furl.rate(&sim.controls) * dt;
    let now = sim.controls.furl;
    let next = if (want - now).abs() <= step {
        want
    } else {
        now + step * (want - now).signum()
    };
    sim.controls.furl = next;
    orders.doing = Some(if bent_on {
        "setting her"
    } else {
        "rolling the old one away"
    });
    if !bent_on && next >= 0.999 {
        // The bow is clear: this is the moment the sails are swapped, and the only
        // moment at which it can happen on a boat.
        sim.controls.headsail = to;
        // A sail held aback is held by a sheet that is now on a different sail.
        sim.controls.headsail_aback = false;
        if say {
            info!("{} bent on", to.name());
        }
    }
    if bent_on && next <= 0.001 {
        orders.headsail = None;
        orders.doing = None;
        if say {
            info!("{} set and drawing", to.name());
        }
        return false;
    }
    true
}

/// The player's own standing order, worked by the hands that are on deck anyway.
///
/// It runs whether or not the watch is standing: with nobody called, this *is* you doing
/// it, which is why it takes exactly as long either way.
pub fn carry_out(
    mut sim: ResMut<Sim>,
    mut orders: ResMut<Orders>,
    mut bank: ResMut<crate::clutch::Bank>,
    time: Res<Time>,
) {
    if !orders.changing() {
        return;
    }
    // Whatever the furling line was told to do before this, it is not what it is doing
    // now. A destination left on the bank would be hauling the drum back the other way
    // while the foredeck turns it — two hands on one rope, which is how a line parts.
    if let Some(which) = crate::clutch::BANK.iter().position(|l| *l == Line::Furl) {
        bank.target[which] = None;
    }
    let dt = time.delta_secs_f64();
    let (sim, orders) = (&mut *sim, &mut *orders);
    work_the_change(sim, orders, dt, true);
}

#[cfg(test)]
mod a_change_is_a_sequence {
    use super::*;

    fn boat() -> Sim {
        Sim::new(crate::weather::Weather::draw(7), 7)
    }

    /// **The order is the end state; the sequence is still the sequence.**
    ///
    /// Ask for the Code 0 with the genoa drawing and three things have to happen in
    /// order, at the drum's pace: the genoa rolls away, the sails swap with the bow
    /// clear, and the new one is set. The fault this guards against is a sail that
    /// teleports into another sail while drawing, which is the one piece of this rig
    /// nobody could believe.
    #[test]
    fn the_old_sail_is_rolled_away_before_the_new_one_is_bent_on() {
        let mut sim = boat();
        sim.controls.furl = 0.0;
        sim.controls.headsail = Headsail::Genoa;
        let mut orders = Orders::default();
        orders.set(&sim, Headsail::Code0);

        let mut swapped_at: Option<f64> = None;
        let mut seconds = 0.0;
        for _ in 0..4_000 {
            seconds += 0.05;
            let busy = work_the_change(&mut sim, &mut orders, 0.05, false);
            if sim.controls.headsail == Headsail::Code0 && swapped_at.is_none() {
                swapped_at = Some(sim.controls.furl);
            }
            if !busy {
                break;
            }
        }
        let swapped = swapped_at.expect("the sails never swapped");
        assert!(
            swapped > 0.99,
            "the Code 0 was bent on with the genoa still {:.0}% out",
            (1.0 - swapped) * 100.0
        );
        assert_eq!(sim.controls.headsail, Headsail::Code0);
        assert!(sim.controls.furl < 0.001, "and it was left in its bag");
        assert!(
            (20.0..160.0).contains(&seconds),
            "a sail change took {seconds:.0} seconds, which is not a foredeck"
        );
    }

    /// Asking for the sail that is already up is *belay that*: it cancels a change in
    /// progress, which is the only way to change your mind half way through one.
    #[test]
    fn ordering_the_sail_you_have_cancels_the_order() {
        let sim = boat();
        let mut orders = Orders::default();
        orders.set(&sim, Headsail::Gennaker);
        assert!(orders.changing());
        orders.set(&sim, Headsail::Gennaker);
        assert!(!orders.changing(), "asking twice should belay it");
    }
}
