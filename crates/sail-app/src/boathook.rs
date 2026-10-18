//! The boathook: four feet of spar with a hook on the end, and the only tool aboard for
//! taking hold of something that is not already yours.
//!
//! It is on the boat because it is on every boat, and it is in the *game* because of what
//! it does to two manoeuvres that until now ended by themselves. Getting a person back
//! over the rail finished when you had been near enough and slow enough for two seconds,
//! which is not what the end of that manoeuvre is like: you come alongside, somebody
//! leans out with the hook, and either they get him or the sea takes him past the stern
//! and you go round again. Picking the lifebuoy back out of the water was not a thing you
//! could do at all.
//!
//! **What it does is reach.** It lies on the side deck until there is something within
//! its length, and then it is in somebody's hands, angled out over the water towards
//! whatever that is, with its end up where a hand would hold it. That is the whole of the
//! interface: you do not aim it and you do not choose it, because on a boat you do not —
//! the hook goes to the thing in the water and the only question is *when*.
//!
//! And the when is the game, the same one the rod plays. A fish is swung aboard on the
//! rise; a person is taken as she comes level. A boathook put on somebody while the deck
//! is dropping away under you is a boathook torn out of your hands, and the sea decides
//! when that is rather than the player — which is why this is a moment to be read rather
//! than a button to be pressed.

use bevy::prelude::*;

use crate::frame;
use crate::sim::Sim;

/// How far the hook reaches from her side, metres.
///
/// Two metres of spar and most of an arm. Deliberately longer than `mob::REACH`, which
/// is what a hand over the rail can get to: the hook exists to turn a near miss into a
/// catch, and a tool with the same reach as the hand holding it is not a tool.
const REACH: f64 = 3.1;

/// And how slowly she has to be going for it to be any use, knots.
///
/// The same number the recovery already uses, and for the same reason: faster than this
/// and the water drags whatever you have hooked aft along the hull and out of your
/// hands. A boathook does not stop a boat.
const SLOW_ENOUGH: f64 = 1.6;

/// How still she has to be, metres a second of heave, for the catch to hold.
///
/// **This is the whole of the skill.** A hook put on in the half second when the deck is
/// level is a hook that holds; the same hook taken as she drops away has a man's weight
/// come onto it through two metres of spar and a quarter of a second, and it goes. A
/// third of a metre a second is a boat that is *between* things — at the top of a lift or
/// at the bottom of one — and in a one-metre sea that is open about a third of the time,
/// which is what makes it a moment to wait for rather than a dice roll.
const STEADY: f64 = 0.34;

/// How long before you can try again after one goes wrong.
///
/// Two seconds and a half to get the hook back inboard, find the thing again and get the
/// weight of it over the rail. It is not a penalty, it is the hook being somewhere else —
/// and it is why a catch taken too early costs more than waiting did.
const RECOVER_THE_HOOK: f64 = 2.5;

/// What the hook can be put on.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Quarry {
    /// Somebody in the water, which is the one that matters.
    Person,
    /// The lifebuoy, floating where it was thrown. Worth having back.
    Ring,
    /// A cask adrift. Nobody's life depends on it, which is the whole charm: the
    /// manoeuvre is the same one, practised for the price of a barrel.
    Cask,
}

impl Quarry {
    fn what(&self) -> &'static str {
        match self {
            Quarry::Person => "him",
            Quarry::Ring => "the lifebuoy",
            Quarry::Cask => "the cask",
        }
    }
}

/// Where the hook is and what it is reaching for.
#[derive(Resource, Default)]
pub struct Boathook {
    /// What is within reach, and where it is in the world.
    pub on: Option<(Quarry, glam::DVec2)>,
    /// How far out it is: nought lying in its chocks, one held out over the water. It
    /// moves over about a third of a second, because a thing that appears in somebody's
    /// hands between two frames reads as a glitch rather than as a hand.
    pub out: f32,
    /// Simulation time until which the hook is inboard after a miss.
    busy_until: f64,
    /// What the deck has just been told, and until when.
    pub said: Option<(String, bool)>,
    said_until: f64,
    /// Set the frame a catch lands, for the drill to read. Taken rather than peeked at,
    /// so it cannot be acted on twice.
    pub caught: Option<Quarry>,
}

impl Boathook {
    /// Whether the hook is out and the moment is right, for the prompt to go urgent.
    pub fn steady_now(&self, st: &sail_core::state::BoatState) -> bool {
        self.on.is_some() && st.heave_rate.abs() < STEADY
    }
}

/// What is in the water to be caught, and how far off her side it is.
///
/// One function because the drawing, the prompt and the catch all have to agree about
/// what the hook is on — and because "the thing on screen and the thing the test used
/// were two different measurements" is this repository's oldest fault.
fn within_reach(
    sim: &Sim,
    over: &crate::mob::Overboard,
    flotsam: &crate::barrels::Flotsam,
) -> Option<(Quarry, glam::DVec2)> {
    let mut best: Option<(Quarry, glam::DVec2, f64)> = None;
    let mut look = |what: Quarry, at: glam::DVec2| {
        let off = crate::mob::off_the_rail(sim, at);
        if off < REACH && best.as_ref().is_none_or(|(_, _, had)| off < *had) {
            best = Some((what, at, off));
        }
    };
    if let Some(him) = over.oscar() {
        look(Quarry::Person, him);
    }
    if let Some(ring) = over.ring_afloat() {
        look(Quarry::Ring, ring);
    }
    // And whatever is floating past. A man and a lifebuoy outrank a barrel by being
    // listed first and by the nearest-wins rule underneath: with somebody in the water
    // alongside, the hook does not go for the cargo.
    if let Some(cask) = flotsam.nearest(sim.sim.state.pos) {
        look(Quarry::Cask, cask);
    }
    best.map(|(what, at, _)| (what, at))
}

/// Reaching for whatever is alongside, and taking it when the key says so.
pub fn reach(
    keys: Res<ButtonInput<KeyCode>>,
    sim: Res<Sim>,
    mut over: ResMut<crate::mob::Overboard>,
    mut flotsam: ResMut<crate::barrels::Flotsam>,
    mut hook: ResMut<Boathook>,
    time: Res<Time>,
    sound: Res<crate::sound::SoundHandle>,
) {
    let now = sim.sim.state.time;
    if hook.said_until < now {
        hook.said = None;
    }
    let slow = sail_core::units::ms_to_kn(sim.sim.telemetry.stw.abs()) < SLOW_ENOUGH;
    let found = if now < hook.busy_until || !slow {
        None
    } else {
        within_reach(&sim, &over, &flotsam)
    };
    hook.on = found;

    // Out of the chocks and into somebody's hands, over a third of a second.
    let want = if hook.on.is_some() { 1.0 } else { 0.0 };
    let k = 1.0 - (-9.0 * time.delta_secs()).exp();
    hook.out += (want - hook.out) * k;
    if hook.out < 0.002 && want == 0.0 {
        hook.out = 0.0;
    }

    let Some((what, _)) = hook.on else {
        return;
    };
    if !keys.just_pressed(KeyCode::Enter) {
        return;
    }
    // **The catch, and the only thing that decides it.**
    //
    // Not where the mouse is and not how long the key was held: whether the deck was
    // level at the moment the hook went on. The sea sets that, you read it, and the
    // reading is the skill the manoeuvre is actually made of.
    if sim.sim.state.heave_rate.abs() < STEADY {
        hook.caught = Some(what);
        hook.said = Some((format!("got {} — on the rise, and held", what.what()), true));
        hook.said_until = now + 4.0;
        sound.0.strike_damped(660.0, 0.22, 0.8);
        match what {
            Quarry::Ring => over.ring_recovered(),
            // On the painter, astern of whatever is already there.
            Quarry::Cask => {
                if let Some((_, at)) = hook.on {
                    flotsam.hooked(at);
                }
            }
            Quarry::Person => {}
        }
    } else {
        let dropping = sim.sim.state.heave_rate < 0.0;
        hook.said = Some((
            if dropping {
                "the deck dropped away and it came off — wait until she is level".into()
            } else {
                "she was still lifting and it slipped — take it as she comes level".into()
            },
            false,
        ));
        hook.said_until = now + 4.0;
        hook.busy_until = now + RECOVER_THE_HOOK;
        sound.0.strike_damped(150.0, 0.18, 0.95);
    }
}

/// The one lying in its chocks on the side deck.
#[derive(Component)]
pub struct InTheChocks;

/// And the one in somebody's hands, out over the water.
#[derive(Component)]
pub struct InHand;

/// Where it lives when nobody wants it: along the starboard side deck, outboard of the
/// coachroof and inboard of the toerail, which is the one strip of a boat this size that
/// is not a winch, a track or a place to put a foot.
const CHOCKS_Z: f32 = 2.2;
/// Two metres of spar. A longer one would reach further and not fit anywhere.
const SHAFT: f32 = 2.0;

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    boat: Query<Entity, With<crate::boat::BoatRoot>>,
) {
    let Ok(root) = boat.single() else {
        return;
    };
    // Varnished ash, like every boathook that has ever been on a wooden boat, with the
    // hook itself in galvanised iron: a hook painted the same colour as its handle is a
    // hook nobody can see the shape of.
    let wood = materials.add(StandardMaterial {
        base_color: Color::srgb(0.62, 0.47, 0.26),
        perceptual_roughness: 0.35,
        ..default()
    });
    let iron = materials.add(StandardMaterial {
        base_color: Color::srgb(0.47, 0.49, 0.52),
        perceptual_roughness: 0.55,
        metallic: 0.8,
        ..default()
    });
    let shaft = meshes.add(Cylinder::new(0.022, SHAFT));
    // The cradles: a block each side of the shaft rather than a bar under it, which is
    // what a chock is — the spar drops into the notch between them.
    let w_cord = meshes.add(Torus::new(0.030, 0.038));
    let teak = materials.add(StandardMaterial {
        base_color: Color::srgb(0.46, 0.34, 0.20),
        perceptual_roughness: 0.8,
        ..default()
    });
    let cord = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.13, 0.15),
        perceptual_roughness: 0.9,
        ..default()
    });
    // The head: a spike forward and a hook out to the side, which is what the thing is —
    // one end to push with and one to pull with.
    let spike = meshes.add(Cylinder::new(0.012, 0.13));
    let hook = meshes.add(Torus::new(0.052, 0.082));

    let head = move |b: &mut ChildSpawnerCommands| {
        b.spawn((
            Mesh3d(spike.clone()),
            MeshMaterial3d(iron.clone()),
            Transform::from_xyz(0.0, SHAFT * 0.5 + 0.06, 0.0),
        ));
        b.spawn((
            Mesh3d(hook.clone()),
            MeshMaterial3d(iron.clone()),
            Transform::from_xyz(0.055, SHAFT * 0.5 + 0.02, 0.0)
                .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))
                // Half a ring: the open side is what goes round a warp.
                .with_scale(Vec3::new(1.0, 1.0, 0.55)),
        ));
    };

    // **On chocks, not lying on the paint.**
    //
    // The first attempt put the shaft's centre forty millimetres above `deck_height`,
    // which is the deck *line* — and the drawn deck at this station is cambered and
    // bulwarked up to eighty, so two thirds of a boathook was inside the deck and what
    // showed was a thin varnished line on a painted surface. Reported from the deck as
    // not being there at all, which it very nearly was not. Measured off the drawn mesh
    // rather than reasoned about: the walked-on surface runs 0.77 to 0.80 here.
    //
    // Chocks are also simply what one lies in. A two-metre spar loose on a side deck
    // goes over the side the first time she rolls, so it lives in a pair of cradles with
    // a shock cord over it, and that is what lifts it clear of the camber.
    let deck = crate::deck::deck_height(CHOCKS_Z);
    // **Clear of the rail at both ends**, which is not the same as clear of it in the
    // middle. She narrows going aft — a tenth of a metre of half-beam in the length of
    // this spar — so a boathook laid parallel to the centreline and placed by the
    // station it is centred on has its after end inside the bulwark. Reported from the
    // deck as climbing onto the rail. The place to measure is the narrowest station the
    // thing covers, and it is the same trap the grab bag was in.
    let narrowest = [-0.5, 0.0, 0.5]
        .into_iter()
        .map(|t| crate::deck::inside_the_rail(CHOCKS_Z + t * SHAFT))
        .fold(f32::MAX, f32::min);
    // Its own half-width, the width of a chock, and a hand's breadth for the shock cord
    // to be got round it.
    let out = narrowest - 0.1;
    let rests_at = deck + 0.115;
    commands.entity(root).with_children(|b| {
        // Lying fore and aft in its chocks, hook forward, which is how one is stowed so
        // that the first thing your hand finds is the handle.
        b.spawn((
            InTheChocks,
            Mesh3d(shaft.clone()),
            MeshMaterial3d(wood.clone()),
            Transform::from_xyz(out, rests_at, CHOCKS_Z)
                .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            Visibility::default(),
        ))
        .with_children(&head);
        // The cradles it sits in, and the cord over the top of it.
        for along in [-0.62, 0.42] {
            // As tall as the gap it fills: the deck runs uphill towards the bow and the
            // spar lies level, so one height for both cradles leaves daylight under one
            // of them. The same fault the club's chocks had.
            let stands_on = crate::deck::deck_height(CHOCKS_Z + along);
            let tall = (rests_at - 0.022 - stands_on).max(0.02);
            b.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.10, tall, 0.07))),
                MeshMaterial3d(teak.clone()),
                Transform::from_xyz(out, stands_on + tall * 0.5, CHOCKS_Z + along),
            ));
            b.spawn((
                Mesh3d(w_cord.clone()),
                MeshMaterial3d(cord.clone()),
                Transform::from_xyz(out, rests_at + 0.015, CHOCKS_Z + along)
                    .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::new(1.0, 1.0, 0.5)),
            ));
        }
        // And the same thing in somebody's hands. A second object rather than one that
        // is moved, because the two have nothing in common but the shape: this one is
        // held at an angle, swings with what it is reaching for, and is placed by the
        // quarry rather than by the deck.
        b.spawn((
            InHand,
            Mesh3d(shaft),
            MeshMaterial3d(wood),
            Transform::default(),
            Visibility::Hidden,
        ))
        .with_children(&head);
    });
}

/// Putting it where the hands are, and pointing it at what they are reaching for.
pub fn dress(
    hook: Res<Boathook>,
    sim: Res<Sim>,
    mut chocks: Query<&mut Visibility, (With<InTheChocks>, Without<InHand>)>,
    mut hand: Query<(&mut Visibility, &mut Transform), With<InHand>>,
) {
    let out = hook.out;
    if let Ok(mut vis) = chocks.single_mut() {
        // It is in the chocks or it is in somebody's hands; it is never in both and
        // never half way, because a boathook does not slide out of its own accord.
        *vis = if out > 0.5 {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    let Ok((mut vis, mut at)) = hand.single_mut() else {
        return;
    };
    let Some((_, quarry)) = hook.on else {
        *vis = Visibility::Hidden;
        return;
    };
    *vis = Visibility::Inherited;
    let st = &sim.sim.state;
    // Which way it is pointing, in her own frame: the bearing of the thing in the water
    // off her bow. The hook follows the quarry rather than the camera, because the
    // person holding it is looking at the water and not at you.
    let off = quarry - st.pos;
    let bearing = sail_core::units::wrap_pi(off.x.atan2(off.y) - st.heading) as f32;
    // The butt end over the deck, the head out over the water and down towards it: a
    // boathook in use is held low, because what it is reaching for is at the waterline
    // and because a hook held high is a hook with no weight behind it.
    let hands_at = Vec3::new(
        crate::deck::deck_half_beam(CHOCKS_Z) * 0.55 * bearing.signum(),
        crate::deck::deck_height(CHOCKS_Z) + 0.95,
        CHOCKS_Z,
    );
    let down = (0.46 + 0.22 * out).min(1.0);
    *at = Transform::from_translation(hands_at + Vec3::Y * (SHAFT * 0.5 * (1.0 - down)))
        .with_rotation(
            Quat::from_rotation_y(bearing)
                * Quat::from_rotation_x(down * std::f32::consts::PI * 0.5),
        );
}

/// The prompt, and the sea the catch has to be timed to.
///
/// A strip with a mark in it that rides up and down with her, and a band across the
/// middle that is the moment. It is the rod's gauge in a different hat and for the same
/// reason: the thing being asked of the player is a *reading of the sea*, and a sea you
/// can only feel through a boat that is not under your feet is a sea nobody can read.
/// The number is hers — it is the heave rate the hull is actually doing — so what the
/// strip shows and what the catch tests are the same thing.
pub fn draw(mut contexts: bevy_egui::EguiContexts, hook: Res<Boathook>, sim: Res<Sim>) -> Result {
    use bevy_egui::egui;
    if hook.out < 0.02 && hook.said.is_none() {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let screen = ctx.viewport_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("boathook"),
    ));
    let middle = screen.center().x;
    let bottom = screen.bottom() - 196.0;

    if let Some((what, _)) = hook.on {
        let steady = hook.steady_now(&sim.sim.state);
        painter.text(
            egui::pos2(middle, bottom - 26.0),
            egui::Align2::CENTER_CENTER,
            if steady {
                format!("ENTER — NOW, take {}", what.what())
            } else {
                format!("ENTER — take {} as she comes level", what.what())
            },
            egui::FontId::monospace(if steady { 17.0 } else { 14.0 }),
            if steady {
                egui::Color32::from_rgb(150, 214, 158)
            } else {
                egui::Color32::from_rgb(232, 176, 58)
            },
        );
        // The sea, as the one number that decides it.
        let wide = 240.0;
        let strip =
            egui::Rect::from_center_size(egui::pos2(middle, bottom), egui::vec2(wide, 12.0));
        painter.rect_filled(
            strip,
            2.0,
            egui::Color32::from_rgba_unmultiplied(12, 14, 18, 190),
        );
        // The band in the middle is the window, drawn at the width it actually is.
        let full = 1.2_f32;
        let window = (STEADY as f32 / full * wide * 0.5).min(wide * 0.5);
        painter.rect_filled(
            egui::Rect::from_center_size(strip.center(), egui::vec2(window * 2.0, 12.0)),
            2.0,
            egui::Color32::from_rgba_unmultiplied(60, 110, 70, 220),
        );
        let rate = (sim.sim.state.heave_rate as f32 / full).clamp(-1.0, 1.0);
        let mark = egui::pos2(strip.center().x + rate * wide * 0.5, strip.center().y);
        painter.line_segment(
            [mark + egui::vec2(0.0, -8.0), mark + egui::vec2(0.0, 8.0)],
            egui::Stroke::new(2.0, egui::Color32::from_rgb(236, 238, 242)),
        );
        painter.text(
            egui::pos2(strip.left() - 8.0, strip.center().y),
            egui::Align2::RIGHT_CENTER,
            "dropping",
            egui::FontId::monospace(9.0),
            egui::Color32::from_rgb(150, 150, 130),
        );
        painter.text(
            egui::pos2(strip.right() + 8.0, strip.center().y),
            egui::Align2::LEFT_CENTER,
            "lifting",
            egui::FontId::monospace(9.0),
            egui::Color32::from_rgb(150, 150, 130),
        );
    }
    if let Some((said, good)) = &hook.said {
        painter.text(
            egui::pos2(middle, bottom + 22.0),
            egui::Align2::CENTER_CENTER,
            said,
            egui::FontId::monospace(13.0),
            if *good {
                egui::Color32::from_rgb(142, 214, 128)
            } else {
                egui::Color32::from_rgb(226, 118, 96)
            },
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A tool has to reach further than the hand holding it.**
    ///
    /// The recovery already had a range — what a pair of hands over the rail can get to
    /// — and if that number ever grows past this one the hook stops being worth picking
    /// up, silently, with nothing failing anywhere. The two live in different modules
    /// and this is the only place they are compared.
    #[test]
    // Constant assertions on purpose: the two numbers live in different modules and
    // nothing else in the program compares them, which is exactly how one of them gets
    // tuned past the other and the tool quietly stops being worth picking up.
    #[allow(clippy::assertions_on_constants)]
    fn the_hook_reaches_further_than_an_arm() {
        assert!(
            REACH > crate::mob::REACH + 0.5,
            "the hook reaches {REACH} m and a hand over the rail reaches {} — there is \
             nothing in it",
            crate::mob::REACH
        );
        // And it is no use at a speed a hand would manage either: what pulls somebody
        // out of your hands pulls them off a hook.
        assert!((SLOW_ENOUGH - crate::mob::ALONGSIDE_SPEED).abs() < 0.3);
    }

    /// **How often the moment comes, in a sea.**
    ///
    /// The whole mechanic is a window that the water opens and shuts, so the question
    /// that decides whether it is a skill or a lottery is what fraction of the time it
    /// is open — and the only honest way to answer it is to put a boat in a sea and
    /// count. A window open nine tenths of the time is a keypress; one open a twentieth
    /// is a dice roll with extra steps.
    ///
    /// Measured here rather than asserted from the constant, because the constant is a
    /// heave *rate* and what matters is the shape of the hull's motion in a real
    /// seaway, which no amount of reading the number tells you.
    #[test]
    fn the_window_is_open_often_enough_to_be_a_skill() {
        use sail_core::sim::{Simulation, SUBSTEP};
        use sail_core::state::{BoatState, Controls};
        let cfg = sail_core::boat::BoatConfig::charter_sloop_34();
        let breeze = sail_core::units::kn_to_ms(14.0);
        let mut sim = Simulation::new(
            cfg,
            BoatState::at_rest(0.0),
            Box::new(sail_core::wind::SteadyWind::new(0.0, breeze)),
        )
        // The sea a recovery actually happens in: a coastal fetch in a fresh breeze,
        // about a metre. Flat water would make this test say the window is always open.
        .with_water(Box::new(sail_core::water::WindSea::new(
            breeze, 20_000.0, 0.0, 11,
        )));
        // Stopped alongside, which is where the manoeuvre ends: no sail, no way on.
        let controls = Controls {
            main_sheet: 1.0,
            jib_sheet: 1.0,
            reef: 1.0,
            furl: 1.0,
            ..Default::default()
        };
        let mut open = 0u32;
        let mut all = 0u32;
        for step in 0..(120.0 / SUBSTEP) as u32 {
            sim.run(SUBSTEP, &controls);
            // The first half minute is her settling into the sea from rest.
            if step as f64 * SUBSTEP < 30.0 {
                continue;
            }
            all += 1;
            if sim.state.heave_rate.abs() < STEADY {
                open += 1;
            }
        }
        let share = open as f64 / all as f64;
        println!("the window is open {:.0}% of the time", share * 100.0);
        assert!(
            (0.12..0.75).contains(&share),
            "the window is open {:.0}% of the time, which is a {} rather than a skill",
            share * 100.0,
            if share > 0.75 { "keypress" } else { "lottery" }
        );
    }
}

// ---------------------------------------------------------------------------------------
// The water you have to get her into.
// ---------------------------------------------------------------------------------------

/// How far off a thing can be and still have its water marked, metres.
///
/// **Ninety metres is a sighting, not a plan.** Further off than that and a ring drawn on
/// the sea is a label on the horizon; nearer than that and it is the thing a helmsman is
/// actually steering at — the patch of water she has to be *in*, which until now was a
/// number nobody could see.
const MARKED_WITHIN: f64 = 90.0;

/// How many patches get marked at once. Four: whoever is in the water, the lifebuoy, and
/// the two nearest casks, which is as much as can be read at a glance.
const MARKED: usize = 4;

/// Segments round the ring, and how wide the band of it is in metres.
const SEGMENTS: usize = 72;
const BAND: f64 = 0.34;

/// How far her centreline has to come to the thing, metres.
///
/// **One fact with one owner.** The hook reaches [`REACH`] from her *rail*, and
/// [`crate::mob::off_the_rail`] measures from the rail by taking half her beam off the
/// distance to her centreline — so the ring that says *bring her into this* has to be
/// that much wider than the hook is long, or the drawn circle and the circle the catch
/// actually uses are two different circles. That has been this repository's oldest
/// fault, so the drawing and the test below both call this.
pub fn marked_radius(cfg: &sail_core::boat::BoatConfig) -> f64 {
    REACH + cfg.beam * 0.5
}

/// One of the rings lying on the sea.
#[derive(Component)]
pub struct MarkedWater(usize);

/// A flat band of triangles, its vertices rewritten every frame from the sea under them.
///
/// It is not a torus on a transform, and that is deliberate: a five-metre ring held rigid
/// at the height of its own centre stands a foot clear of the water on one side and goes
/// under on the other the moment a wave runs through it. Seventy-two pairs of vertices
/// put on the surface itself cost nothing and sit *on* the sea, which is the only way a
/// mark drawn on water reads as being drawn on water.
fn band() -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::mesh::{Indices, PrimitiveTopology};
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    let n = SEGMENTS + 1;
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![Vec3::ZERO; n * 2]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![Vec3::Y; n * 2]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![Vec2::ZERO; n * 2]);
    let mut tri: Vec<u32> = Vec::with_capacity(SEGMENTS * 6);
    for i in 0..SEGMENTS as u32 {
        let (a, b, c, d) = (i * 2, i * 2 + 1, i * 2 + 2, i * 2 + 3);
        tri.extend_from_slice(&[a, b, c, b, d, c]);
    }
    mesh.insert_indices(Indices::U32(tri));
    mesh
}

pub fn mark_setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for which in 0..MARKED {
        commands.spawn((
            MarkedWater(which),
            Mesh3d(meshes.add(band())),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgba(0.87, 0.85, 0.80, 0.70),
                // Unlit, because it is not a thing floating in the weather — it is a mark
                // the player has drawn on the sea, and it has to read at dusk as well as
                // at noon.
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                double_sided: true,
                cull_mode: None,
                ..default()
            })),
            Transform::default(),
            Visibility::Hidden,
        ));
    }
}

/// Lay a ring on the water round everything within reaching distance of becoming a catch.
///
/// The colour carries the one condition the geometry cannot: a boathook does not stop a
/// boat, so arriving in the right water at five knots is arriving at nothing. Ivory while
/// she is outside it, green the moment she is in it and slow enough to hold what she
/// hooks, vermilion when she is in it going too fast — which is the commonest way this
/// manoeuvre is got wrong, and now it is visible from the helm instead of from a log line
/// after the fact.
#[allow(clippy::too_many_arguments)]
pub fn mark_the_water(
    sim: Res<Sim>,
    origin: Res<frame::RenderOrigin>,
    over: Res<crate::mob::Overboard>,
    flotsam: Res<crate::barrels::Flotsam>,
    mut rings: Query<(
        &MarkedWater,
        &Mesh3d,
        &MeshMaterial3d<StandardMaterial>,
        &mut Transform,
        &mut Visibility,
    )>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let st = sim.render_state();
    let here = st.pos;
    let t = sim.sim.state.time;
    let radius = marked_radius(&sim.config());

    // Everything catchable, nearest first. A man and a lifebuoy are in the list on the
    // same terms as a barrel, because the manoeuvre is the same manoeuvre.
    let mut about: Vec<glam::DVec2> = Vec::new();
    about.extend(over.oscar());
    about.extend(over.ring_afloat());
    about.extend(flotsam.adrift.iter().copied());
    about.retain(|at| (*at - here).length() < MARKED_WITHIN);
    about.sort_by(|a, b| (*a - here).length().total_cmp(&(*b - here).length()));

    let slow = sail_core::units::ms_to_kn(sim.sim.telemetry.stw.abs()) < SLOW_ENOUGH;
    for (which, mesh, material, mut transform, mut show) in &mut rings {
        let Some(&at) = about.get(which.0) else {
            *show = Visibility::Hidden;
            continue;
        };
        *show = Visibility::Visible;
        // In it when the hook could be put on the thing from where she is now: the same
        // measurement the catch uses, not a second copy of it.
        let inside = crate::mob::off_the_rail(&sim, at) < REACH;
        if let Some(mut m) = materials.get_mut(&material.0) {
            m.base_color = match (inside, slow) {
                (true, true) => Color::srgba(0.42, 0.70, 0.44, 0.80),
                (true, false) => Color::srgba(0.86, 0.30, 0.18, 0.80),
                _ => Color::srgba(0.87, 0.85, 0.80, 0.62),
            };
        }
        let middle = sim.sim.water.height(at, t) as f32;
        transform.translation = origin.to_bevy(at, middle);
        let Some(mut mesh) = meshes.get_mut(&mesh.0) else {
            continue;
        };
        let Some(bevy::render::mesh::VertexAttributeValues::Float32x3(pos)) =
            mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
        else {
            continue;
        };
        for i in 0..=SEGMENTS {
            let a = i as f64 / SEGMENTS as f64 * std::f64::consts::TAU;
            let (s, c) = a.sin_cos();
            let out = glam::DVec2::new(c, s);
            for (k, r) in [radius - BAND * 0.5, radius + BAND * 0.5]
                .into_iter()
                .enumerate()
            {
                let p = at + out * r;
                // Three centimetres over the surface: enough that it is not fighting the
                // sea for the same pixels, little enough that it is still on it.
                let h = sim.sim.water.height(p, t) as f32 + 0.03;
                let v = origin.to_bevy(p, h) - origin.to_bevy(at, middle);
                pos[i * 2 + k] = [v.x, v.y, v.z];
            }
        }
    }
}

#[cfg(test)]
mod the_marked_water {
    use super::*;

    /// **The ring drawn and the circle the catch uses are one circle.**
    ///
    /// The hook reaches from her rail and the mark is drawn round her centreline, so the
    /// two differ by half a beam — and the day they differ by anything else is the day a
    /// player sails into a green ring and is told there is nothing in reach.
    #[test]
    fn the_ring_is_exactly_the_water_the_hook_can_reach_from() {
        let cfg = sail_core::boat::BoatConfig::default();
        let r = marked_radius(&cfg);
        // Her rail is half a beam out from the centreline; the hook adds its own length.
        assert!(
            (r - cfg.beam * 0.5 - REACH).abs() < 1e-9,
            "the ring is {r} m and the hook reaches {REACH} m off a {} m beam",
            cfg.beam
        );
        // And it is a ring somebody can steer at: wider than she is, narrower than a
        // boat length.
        assert!(r > cfg.beam && r < cfg.loa * 0.6, "{r} m");
    }
}
