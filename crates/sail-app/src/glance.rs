//! Where the eye goes when you work the boat.
//!
//! The camera already knows how to look at things — `camera::Attention` takes a claim on
//! it and the strongest live claim wins, with your own hand on the view outranking all of
//! them. The rod uses it. This file is the rest of the boat using it.
//!
//! **The rule is: look at what just moved, not at what is important.** They are not the
//! same and the difference is the whole of whether a camera helps or fights you. A sheet
//! running out is important and you are already looking at it; a clutch going over is a
//! thing that happened three metres away behind your shoulder, and it is the one you
//! cannot see. So a key press buys a glance at the gear that key works, and nothing buys
//! a glance at the thing a designer thinks you ought to care about.
//!
//! Three rules hold it together, and the second is the one that is easy to get wrong.
//!
//! * **The glance lasts as long as the job.** A reef is twenty seconds of work and the
//!   frame should stay on it; a clutch is half a second and should flick. That is the
//!   hold time in the call and not a constant, which is why there is no constant.
//! * **Priority is urgency, not loudness.** Five is something gone wrong that you did not
//!   do — the pilot letting go, which is the one dropout you need telling about. Three is
//!   a deliberate act of yours. Two is following something that is still happening. One
//!   is background. The rod holds one to four, so five is free and is the only thing that
//!   interrupts a fish.
//! * **Never look at what cannot be seen.** This was learned expensively: the camera
//!   pointed at a hooked fish for a day, and a fish is two metres under an opaque sea, so
//!   it framed blank water at the one moment the whole feature existed for.
//!
//! What it deliberately does not do is *cut*. Every one of these is a head turning at
//! four and a half radians a second, because a cut is the one thing a camera standing in
//! for a person must never do.

use bevy::prelude::*;

use crate::camera::Attention;
use crate::input::Pilot;

/// What each claim is worth. See the module header: urgency, not loudness.
const ALARM: u8 = 5;
const DELIBERATE: u8 = 3;

/// A glance at a thing you are *doing* lasts about as long as the doing of it. These are
/// the short ones; anything held refreshes its own claim every frame instead.
const FLICK: f32 = 1.3;
const JOB: f32 = 2.6;
const HELD: f32 = 0.5;

/// Where a marked fitting is, in rendered space.
///
/// Generic over the filter rather than a closure, because a closure takes the type of its
/// first call site and these are four different queries. Asked of the *drawn* transform
/// and not recomputed from the geometry, so a glance points at the thing itself and
/// cannot drift from it — which is the fault that put a creel a quarter of a metre inside
/// the planking earlier today, in a file that worked its position out for itself.
fn where_is<F: bevy::ecs::query::QueryFilter>(q: &Query<&GlobalTransform, F>) -> Option<Vec3> {
    q.iter().next().map(|t| t.translation())
}

/// Everything the glances read, which is only ever "has this been pressed".
#[derive(bevy::ecs::system::SystemParam)]
pub struct Hands<'w> {
    keys: Res<'w, ButtonInput<KeyCode>>,
    pilot: Res<'w, Pilot>,
}

/// Look where the hands are.
///
/// Reads the keyboard directly, like the other modules that do, and posts claims rather
/// than moving anything — so a glance can be outranked, and the player's own hand on the
/// view silences the lot of it. Nothing here can take the camera away from you.
pub fn posts(
    hands: Hands,
    mut attention: ResMut<Attention>,
    winch: Query<&GlobalTransform, With<crate::fittings::ClutchWinch>>,
    boom: Query<&GlobalTransform, With<crate::boat::Boom>>,
    tiller: Query<&GlobalTransform, With<crate::fittings::Tiller>>,
    bags: Query<&GlobalTransform, With<crate::stowage::SailBag>>,
) {
    let Hands { keys, pilot } = hands;

    // --- The pilot has given up ------------------------------------------------
    //
    // The one claim that outranks a fish, and the only dropout that is not your own
    // doing: it could not hold the course and the boat is now steering herself. It
    // already sounds an alarm — see `input.rs` — and an alarm you have to be looking at
    // the screen to act on is only half of one. The helm is what you look at, because the
    // helm is where the answer is.
    if pilot.since_reason < 0.2
        && pilot
            .last_reason
            .is_some_and(|why| why.contains("could not"))
    {
        if let Some(helm) = where_is(&tiller) {
            attention.look(helm, 4.0, ALARM);
        }
    }

    // --- The clutch bank -------------------------------------------------------
    //
    // The control nobody finds. It is a bank of levers and one winch on the deck forward
    // of the coaming, it works seven different lines, and from the helm it is behind your
    // shoulder — so the single most useful thing the camera can do on this boat is show
    // you it the first time you press `1`.
    //
    // Choosing a line flicks; working one holds, because working one is a job with a
    // duration and the frame should stay on it until you stop.
    if let Some(bank) = where_is(&winch) {
        if keys.just_pressed(KeyCode::Digit1) || keys.just_pressed(KeyCode::Digit2) {
            attention.look(bank, FLICK, DELIBERATE);
        }
        if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::KeyD) {
            attention.look(bank, HELD, DELIBERATE);
        }
    }

    // --- The boom --------------------------------------------------------------
    //
    // Two keys, one object, and in both cases the boom is the thing the key is *about*.
    // The dump is held and the boom is going somewhere while you hold it, so the claim
    // refreshes; the preventer is a rope somebody ties on, and what you want is a look at
    // the spar it will not let move.
    if let Some(spar) = where_is(&boom) {
        if keys.pressed(KeyCode::KeyR) {
            attention.look(spar, HELD, DELIBERATE);
        }
        if keys.just_pressed(KeyCode::KeyF) {
            attention.look(spar, JOB, DELIBERATE);
        }
    }

    // --- The foredeck ----------------------------------------------------------
    //
    // A sail change is the one thing on this boat that happens entirely out of sight of
    // the helm: a bag disappears off the side deck and a different sail goes up. Aimed at
    // the bags rather than at the headsail, because the bags are where the *change* is —
    // the sail is up the rig and in frame already from anywhere.
    if keys.just_pressed(KeyCode::KeyX) || keys.just_pressed(KeyCode::KeyZ) {
        // The middle of them, so the glance does not depend on which bag happens to be
        // first out of the query — and so it still points somewhere sensible when the
        // bag for the sail you just set has gone.
        let mut sum = Vec3::ZERO;
        let mut count = 0.0;
        for bag in &bags {
            sum += bag.translation();
            count += 1.0;
        }
        if count > 0.0 {
            attention.look(sum / count, JOB, DELIBERATE);
        }
    }
}
