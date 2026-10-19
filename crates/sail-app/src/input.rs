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

/// How far the stick has to go to take the helm off the pilot.
///
/// A quarter, which is a long way past the dead zone and nowhere near a real steer. The
/// two numbers are different on purpose: the dead zone is what the helm *ignores*, and
/// this is what counts as somebody deciding to steer. A pad whose stick rests a little
/// off centre — which is most pads over a year old — otherwise disengages the autopilot
/// the instant it is switched on, with a reason that reads like a lie because nobody
/// touched anything.
const TAKES_THE_HELM: f32 = 0.25;

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

/// What the dump is doing, between frames.
///
/// Two stages and not a held key, because they are two manoeuvres: one press gets her on
/// her feet and keeps her sailing, a second — or a hand that stays on it — takes
/// everything off and stops her. A stage latches until its job is done, so a tap does a
/// whole job rather than a frame of one.
#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub enum Dump {
    #[default]
    Off,
    /// Easing until the deck is steady and the pilot is not being overrun.
    Steadying,
    /// Everything off. She will stop.
    Letting,
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
/// How fast a sheet runs, in sheet fraction per second. Still the winch's rate — the
/// mainsheet asks for degrees instead, see `BOOM_RATE`.
const SHEET_RATE: f64 = 0.45;

/// How fast the mainsheet swings the boom, degrees a second.
///
/// Eighteen, which is the whole travel in about five seconds and is what a hand on a
/// four-part tackle actually does. Dumping it is a different action with its own key.
const BOOM_RATE: f64 = 18.0;
/// And the fine rate, for placing the boom to a degree, which is most of the game.
const BOOM_FINE: f64 = 5.0;
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
    mut dumping: Local<Dump>,
    mut held_for: Local<f32>,
    mut stage: Local<u8>,
    mut crew: ResMut<crate::crew::Crew>,
    mut orders: ResMut<crate::orders::Orders>,
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
            // Switched off, not dropped out. `drop_out` is for the gear giving up — it
            // writes a reason that the console shows in the alarm colour for half a
            // minute — and a person pressing the button is not a failure. It read "you
            // switched it off" in red, which is a console crying wolf at the one moment
            // nothing at all has gone wrong, and it is also what the panel's own
            // ENGAGED lamp has always done: the key and the button behaved differently
            // for the same action.
            pilot.disengage();
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
    //
    // **Space is not touching the helm while the pilot has it**, and that was reported
    // from the deck twice as "it keeps switching itself off and nothing says why". It
    // was saying why — in the pilot's own box, in red — but the press that did it was
    // not a press at the helm: `Space` is also the key that brings the camera back to
    // where it was looking, which is a thing you do every time you put the glasses down.
    // So a camera key was quietly undoing a course somebody had set up, and the message
    // explaining it was about a helm they had not touched.
    //
    // Centring the helm with a machine holding it is not a manoeuvre anyway. The ram is
    // clipped to the tiller; what you do when you want the helm is take it — the arrows,
    // the stick, or `P` — and all three of those still throw the pilot out at once.
    //
    // And the stick has to *move*, not merely read something. A worn one sits a little
    // off centre for ever, which past the dead zone is a pilot that cannot be engaged at
    // all and a dropout a second with a reason that reads like a lie.
    if pilot.engaged() {
        if key_rate != 0.0 {
            pilot.drop_out("you took the helm");
        } else if stick.abs() > TAKES_THE_HELM {
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

    // **A hand on a sheet stands the watch down.**
    //
    // The same bargain the pilot makes with the helm, and for the same reason: two
    // people hauling on one rope in opposite directions is not a crew, it is a fight,
    // and the one who should win is the one whose hand is actually on it. Taking the
    // sheets back is as plain as putting a hand on them, and calling the watch again is
    // one press of the switch.
    // **`shift T` calls the watch on deck, or stands them down.**
    //
    // The same letter as the five-second trim assist and deliberately: `T` is a hand
    // going to a winch for as long as it takes to get that sail right, and `shift T` is
    // that hand staying there. One is a thing you do; the other is a thing you arrange.
    // The console switch says the same in a picture, and a control with no key is a
    // control nobody finds.
    let called_the_watch = fine && keys.just_pressed(KeyCode::KeyT);
    if called_the_watch {
        let standing = !crew.standing;
        crew.standing = standing;
        crew.on = None;
        info!(
            "{}",
            if standing {
                "watch on deck — they have the sails"
            } else {
                "watch stood down — you have the sails"
            }
        );
    }
    if !called_the_watch
        && [
            KeyCode::KeyW,
            KeyCode::KeyS,
            KeyCode::KeyQ,
            KeyCode::KeyE,
            KeyCode::KeyA,
            KeyCode::KeyD,
            // `T` on its own is a hand on a sheet; `shift T` is the switch, and the switch
            // is handled below.
            KeyCode::KeyT,
            // The wardrobe as well: ordering a sail is a decision about the rig, and a
            // watch that goes on having its own opinion about which sail is up is a watch
            // that puts the staysail back up as fast as you can get it down.
            KeyCode::KeyX,
            KeyCode::KeyZ,
        ]
        .iter()
        .any(|k| keys.just_pressed(*k))
    {
        crew.stand_down("the ropes");
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
        let mut way = 0.0;
        if keys.pressed(KeyCode::KeyW) {
            way -= 1.0; // haul in
        }
        if keys.pressed(KeyCode::KeyS) {
            way += 1.0; // ease
        }
        if way != 0.0 {
            // At a rate in **boom degrees**, not in sheet travel.
            //
            // Reported from the deck as the boom reacting too strongly when trimming by
            // hand, and the reason is in the sheet's own curve: the chord angle goes with
            // the square of the sheet, so a fixed fraction of travel a second swings the
            // boom at about twenty degrees a second when it is hauled in and seventy when
            // it is eased — the same key doing three different things depending on where
            // you already are, and the fast end is exactly where a reach is trimmed.
            //
            // A hand on a tackle does not work like that. The tail comes in at the speed
            // an arm pulls, which at a fixed purchase is a roughly constant angular rate
            // at the boom. So the key asks for degrees and the sheet step is worked back
            // through the profile's own slope — numerically, so that this stays right if
            // anybody changes the curve.
            let profile = &sim.config().main.profile;
            let here = sim.controls.main_sheet;
            let h = 1.0e-3;
            let slope = ((profile.chord_angle((here + h).min(1.0))
                - profile.chord_angle((here - h).max(0.0)))
                / (2.0 * h))
                .abs()
                .max(rad(8.0));
            let wanted = rad(if fine { BOOM_FINE } else { BOOM_RATE });
            let step = way * (wanted / slope) * dt;
            sim.controls.main_sheet = (sim.controls.main_sheet + step).clamp(0.0, 1.0);
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
        // An order rather than a switch: the key says which sail should be up, and the
        // foredeck does the furling, the swap and the setting in that order and at its
        // own pace. The refusal that used to live here — *roll the headsail away first*
        // — was the game asking the player to be the winch.
        let wardrobe = [
            Headsail::Genoa,
            Headsail::Code0,
            Headsail::Gennaker,
            Headsail::Spinnaker,
        ];
        let from = orders.headsail.unwrap_or(sim.controls.headsail);
        let at = wardrobe.iter().position(|h| *h == from).unwrap_or(0) as i32;
        let next = (at + step).clamp(0, wardrobe.len() as i32 - 1) as usize;
        if wardrobe[next] != from {
            orders.headsail = Some(wardrobe[next]);
            info!("hands forward — {} to go up", wardrobe[next].name());
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
        // **Only the genoa**, because it is the only sail aboard with a clew, a sheet
        // and a side deck to hold it against. The sails set flying are tacked out on
        // the sprit ahead of everything with nothing to back them onto, and the
        // staysail's club boom goes across whatever anybody wants. The engine already
        // ignores the flag for those — this is so that pressing the key says why
        // instead of appearing to do something.
        if sim.controls.headsail != sail_core::state::Headsail::Genoa {
            info!("only the genoa can be backed — the rest are set flying");
        } else if sim.controls.furl > 0.9 {
            info!("the genoa is rolled away — there is nothing to back");
        } else {
            sim.controls.headsail_aback = !sim.controls.headsail_aback;
            info!(
                "{}",
                if sim.controls.headsail_aback {
                    "genoa backed — the clew is against the rail and she will pay off"
                } else {
                    "genoa sheeted across"
                }
            );
        }
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
    // The preventer. A toggle rather than a held key, because it is a rope somebody ties
    // on and leaves: you rig it at the top of a long run and you take it off before you
    // mean to gybe, and forgetting which of those you have done is part of the thing.
    if keys.just_pressed(KeyCode::KeyF) {
        sim.controls.preventer = !sim.controls.preventer;
        info!(
            "preventer {}",
            if sim.controls.preventer {
                "rigged — she will not gybe until it is off"
            } else {
                "off"
            }
        );
    }

    // **One press steadies her; a second, or holding it, lets everything go.**
    //
    // Asked for from the deck, and the two are different manoeuvres that happen to use
    // the same hand. The first is what a crew does in a gust: dump enough to get her on
    // her feet and keep sailing — and *enough* has a definition, because the thing you
    // are easing against is the helm. Past a certain load the pilot is holding the
    // rudder over instead of steering with it, and a boat whose autopilot is being
    // overrun is a boat sailing itself off the course whatever the sails are doing. So
    // the first press eases until she is upright and the helm load is back inside what
    // the ram can hold, and then it stops — it does not keep running the sheets out
    // because the key is still down.
    //
    // The second is what you do when you want to *stop*: everything off, both sheets to
    // the knot, cloth flogging. Nobody sails like that and that is the point of it.
    let dump_key =
        keys.pressed(KeyCode::KeyR) || pad.is_some_and(|p| p.pressed(GamepadButton::West));
    let dump_now = keys.just_pressed(KeyCode::KeyR)
        || pad.is_some_and(|p| p.just_pressed(GamepadButton::West));
    if dump_now {
        *dumping = match *dumping {
            // Already easing, and asked again: she is to stop.
            Dump::Steadying => Dump::Letting,
            _ => Dump::Steadying,
        };
        if *dumping == Dump::Letting {
            info!("everything off — she will stop");
        }
    }
    if dump_key {
        *held_for += dt as f32;
        // Held rather than tapped is the same instruction as pressing it twice, because
        // a hand that stays on the clutch is a hand that means all of it.
        if *held_for > 0.6 && *dumping == Dump::Steadying {
            *dumping = Dump::Letting;
            info!("everything off — she will stop");
        }
    } else {
        *held_for = 0.0;
    }
    if *dumping != Dump::Off {
        let heel = sim.sim.telemetry.heel.abs();
        let over = ((heel - sail_core::autopilot::OVERPOWERED_HEEL)
            / sail_core::autopilot::OVERPOWERED_HEEL)
            .clamp(0.0, 1.0) as f32;
        // **Is she steady enough to leave alone?**
        //
        // Two readings and both are hers rather than the sails': on her feet, and a
        // helm the ram is not fighting. Eight tenths of the easy load leaves the pilot
        // somewhere to go when the next gust arrives — stopping exactly on the line
        // would be a boat that is overrun by the first puff after you let go of the key.
        let helm_load = sim.sim.telemetry.weather_helm.abs();
        let steady = heel <= sail_core::autopilot::OVERPOWERED_HEEL
            && helm_load <= sail_core::autopilot::EASY_HELM * 0.8;
        if *dumping == Dump::Steadying && steady {
            *dumping = Dump::Off;
            info!("that is the deck steady — she is sailing and the pilot can hold her");
        } else {
            // A second and a half of travel when she is right over, and still four
            // seconds when she is barely past the mark — because a hand on a clutch
            // does not meter the rope out in proportion to anything, it opens it. The
            // proportional part is there so the last of it comes off gently instead of
            // the sheet running out and the boom slamming against the shrouds.
            //
            // Letting her go runs at the full rate whatever she is doing: the point of
            // it is not to depower, it is to stop, and a sheet that comes off slowly
            // because the boat happens to be upright is a sheet that is no use for it.
            let urgency = if *dumping == Dump::Letting {
                1.0
            } else {
                0.35 + 0.65 * over
            };
            let run = (0.7 * urgency * dt as f32).min(1.0);
            // The mainsail first and then the headsail, which is the order that works
            // and the order a crew uses. The main is up high with a long lever and it
            // is the sail that spills cleanly: eased, it lets the top go first and the
            // heeling moment comes off before the drive does. A headsail eased just
            // stops working. So the main goes to the shrouds before the genoa is
            // touched at all.
            let main_room = 1.0 - sim.controls.main_sheet as f32;
            if main_room > 0.01 {
                sim.controls.main_sheet = (sim.controls.main_sheet + run as f64).clamp(0.0, 1.0);
            } else {
                sim.controls.jib_sheet = (sim.controls.jib_sheet + run as f64).clamp(0.0, 1.0);
            }
            // And it takes the hands off whatever `T` was doing, or the two of them
            // fight over the same winch.
            *assisting = 0.0;
            // Everything is out and she is flogging: the job is done and the key is
            // free to mean "steady her" again next time.
            if *dumping == Dump::Letting
                && sim.controls.main_sheet > 0.999
                && sim.controls.jib_sheet > 0.999
            {
                *dumping = Dump::Off;
            }
        }
    }
    if keys.pressed(KeyCode::KeyR) || pad.is_some_and(|p| p.pressed(GamepadButton::West)) {
        let heel = sim.sim.telemetry.heel.abs();
        let over = ((heel - sail_core::autopilot::OVERPOWERED_HEEL)
            / sail_core::autopilot::OVERPOWERED_HEEL)
            .clamp(0.0, 1.0) as f32;
        {
            // A second and a half of travel when she is right over, and still four
            // seconds when she is barely past the mark — because a hand on a clutch
            // does not meter the rope out in proportion to anything, it opens it. The
            // proportional part is there so the last of it comes off gently instead of
            // the sheet running out and the boom slamming against the shrouds.
            //
            // And it runs at *any* heel, which it did not. This was gated on being past
            // the angle a cruising crew reefs at, so holding it down at fifteen degrees
            // did nothing whatever and the key read as broken — reported from the deck
            // exactly that way. A release that refuses to release until things are
            // already bad is the one control you cannot afford to have a condition on:
            // the moment you want it is the moment before it is needed, when you have
            // seen the gust coming across the water and she is still on her feet.
            let run = (0.7 * (0.35 + 0.65 * over) * dt as f32).min(1.0);
            let main_room = 1.0 - sim.controls.main_sheet as f32;
            if main_room > 0.01 {
                sim.controls.main_sheet = (sim.controls.main_sheet + run as f64).clamp(0.0, 1.0);
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
    let pressed = (keys.just_pressed(KeyCode::KeyT) && !fine)
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
                if on_the_main {
                    "mainsheet"
                } else {
                    "headsail sheet"
                }
            );
        }
    }
}

#[cfg(test)]
mod trimming {
    use sail_core::boat::BoatConfig;
    use sail_core::units::{deg, rad};

    /// The mainsheet moves the boom at the same rate wherever it already is.
    ///
    /// Reported from the deck as the boom reacting too strongly when trimmed by hand, and
    /// the fault is in the sheet's own curve rather than in the key: the chord angle goes
    /// with the *square* of the sheet, so a fixed fraction of travel a second swung the
    /// boom at twenty degrees a second hauled in and seventy eased — three times as fast
    /// at exactly the end where a reach is trimmed.
    #[test]
    // The last two assertions are on constants on purpose: they are the pair somebody
    // adjusting the feel will reach for, and what they must not do is cross.
    #[allow(clippy::assertions_on_constants)]
    fn the_mainsheet_swings_the_boom_at_one_rate_wherever_it_is() {
        let cfg = BoatConfig::charter_sloop_34();
        let profile = &cfg.main.profile;
        let wanted = rad(super::BOOM_RATE);
        let h = 1.0e-3;
        let mut slowest = f64::MAX;
        let mut fastest = 0.0_f64;
        // A second of the key from each starting point, integrated a frame at a time the
        // way the game does it: the slope is re-read every frame, so measuring the second
        // as one step measures the integration error rather than the trim — which is what
        // the first version of this test did, and it reported nine degrees against ninety.
        let dt = 1.0 / 60.0;
        for i in 2..18 {
            let start = i as f64 / 20.0;
            let mut here = start;
            for _ in 0..60 {
                let slope = ((profile.chord_angle((here + h).min(1.0))
                    - profile.chord_angle((here - h).max(0.0)))
                    / (2.0 * h))
                    .abs()
                    .max(rad(8.0));
                here = (here + (wanted / slope) * dt).clamp(0.0, 1.0);
            }
            let moved = deg((profile.chord_angle(here) - profile.chord_angle(start)).abs());
            slowest = slowest.min(moved);
            fastest = fastest.max(moved);
        }
        assert!(
            fastest / slowest < 1.6,
            "a second on the key moves the boom between {slowest:.0} and {fastest:.0} \
             degrees depending on where it already is"
        );
        // And the rate is a hand on a tackle rather than a dump: the whole travel in
        // about five seconds, not in two.
        assert!(
            (10.0..26.0).contains(&super::BOOM_RATE),
            "{:.0} degrees a second is not somebody trimming",
            super::BOOM_RATE
        );
        assert!(super::BOOM_FINE < super::BOOM_RATE * 0.4);
    }
}
