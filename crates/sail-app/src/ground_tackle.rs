//! The foredeck: letting go, veering, weighing, and the anchor watch.
//!
//! The chain is a force in `sail_core::anchor` and this is the hand on it. Three things
//! a crew does and one the sea does to them:
//!
//! * **Let go**, which puts the anchor on the bottom under the bow and starts the chain
//!   running out. Where you let go is the decision — the depth under her now is the
//!   depth the scope is measured against, and the ground is whatever is there.
//! * **Veer**, which is the only thing that happens afterwards that makes any
//!   difference. More chain is more holding, right up to about five to one.
//! * **Weigh**, which a windlass does slowly and cannot do at all while the chain is
//!   loaded — a boat that wants her anchor back has to sail up to it, which is a
//!   manoeuvre and not a button.
//! * And **dragging**, which is noticed by a bearing changing. The alarm is the one
//!   piece of this that is not seamanship: it is a modern anchor watch, it is what
//!   anybody actually relies on, and it is the reason the anchor keeps its own record
//!   of how far it has walked.

use bevy::prelude::*;
use glam::DVec2;
use sail_core::anchor::{Anchor, Down, Ground, GOOD_SCOPE};

use crate::sim::Sim;

/// How fast the chain goes out and comes back, metres a second.
///
/// **Out is a clutch and a brake; in is a windlass.** Letting go is gravity and a drum
/// that is free to run, so the chain goes out as fast as anybody wants it to; getting it
/// back is an electric motor through a worm drive at a steady walking pace, which is why
/// weighing takes minutes and why nobody veers chain they do not need.
const VEERS: f64 = 1.6;
const WEIGHS: f64 = 0.28;

/// What the windlass can pull before it stalls, newtons.
///
/// **A windlass is for lifting chain, not for pulling a boat.** It will lift the weight
/// of the cable and break the anchor out once she is over it; it will not drag five
/// tonnes of boat up to windward. So a boat that wants her anchor back sails up to it
/// and takes in the slack as it comes — which is the manoeuvre, and which the model
/// gives for nothing because the chain's pull falls to nothing the moment she is over
/// her anchor.
const STALLS_AT: f64 = 2_400.0;

/// The least scope the foredeck will let go on at all.
///
/// Two and a half to one is where `sail_core::anchor::for_scope` says she is parked
/// rather than anchored, so water that cannot be given that much of the chain she has is
/// water to go somewhere else from.
const SHALLOW_ENOUGH: f64 = 2.5;

/// How far she may walk before the watch goes off, metres.
///
/// A boat lies back and sheers about on her chain, so a drag alarm set to anything under
/// a boat length is an alarm that cries wolf all night. This is the anchor's *own*
/// walking, which is zero while she is holding — so it can be tight.
const ALARM_AT: f64 = 15.0;

/// How slowly she has to arrive for the lines to go on, m/s.
///
/// **Half a knot, which is a walking pace and is what coming alongside is.** Faster than
/// this and the lines are not made fast, they are a thing somebody is holding while the
/// boat decides where to go — and `contact.rs` has already told you it was a touch.
const ALONGSIDE: f64 = 0.26;

/// The hand on the cable.
#[derive(Resource, Default)]
pub struct Cable {
    /// How much chain the foredeck is working towards, metres. `None` is *leave it*.
    pub want: Option<f64>,
    /// Whether the windlass is stalled on a loaded chain — said on the console, because
    /// a button that does nothing and does not say why is the worst kind.
    pub stalled: bool,
    /// Whether the watch has already gone off for this drag.
    alarmed: bool,
    /// What the deck has just been told, and until when.
    pub said: Option<(String, f64)>,
    /// Where she is made fast, if she is — the offset from the thing she is lying to, so
    /// that a platform which is itself moored can be followed rather than snapped to.
    ///
    /// **Mooring is the other way a passage ends**, and in this world there is one thing
    /// to lie alongside: the platform. Everything needed for it was already here once
    /// the hull could touch things — come up to it under half a knot and the lines go
    /// on; press the foredeck's key and they come off.
    pub fast_to: Option<DVec2>,
    /// Whether she has to get clear before the lines may go on again.
    ///
    /// **Because casting off and making fast are the same geometry.** She is alongside
    /// and slow at the moment the lines come off, which is exactly the state that puts
    /// them on — so without this the key let go of the shore and took hold of it again
    /// in the same frame, and nothing on deck changed.
    shoved_off: bool,
}

impl Cable {
    /// What the foredeck says, for a few seconds.
    fn say(&mut self, words: impl Into<String>, now: f64) {
        self.said = Some((words.into(), now + 6.0));
    }
}

pub fn plugin(app: &mut App) {
    use crate::stages::{Deck, Paint};
    app.init_resource::<Cable>();
    app.add_systems(
        Update,
        (brought_up, the_key, work, make_fast)
            .chain()
            .in_set(Deck::Weather),
    );
    app.add_systems(bevy_egui::EguiPrimaryContextPass, draw.in_set(Paint::Over));
}

/// What the foredeck has just called aft, in the place every other hand's words go.
///
/// **Because the three answers to the key are different and one of them is a refusal.**
/// Let go, weighing, and *forty metres — too deep to lie in* all come from the same
/// press, and a key that silently does nothing in the third case is the worst of them.
pub fn draw(
    mut contexts: bevy_egui::EguiContexts,
    cable: Res<Cable>,
    struck: Res<crate::contact::Struck>,
    chart: Res<crate::chart::Chart>,
) -> Result {
    // The foredeck and the fenders share a line, because they are the same voice: what
    // just happened at the bow. A contact is the louder of the two and wins the line.
    let Some((said, _)) = struck.said.as_ref().or(cable.said.as_ref()) else {
        return Ok(());
    };
    if chart.spread {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let screen = ctx.viewport_rect();
    let painter = ctx.layer_painter(bevy_egui::egui::LayerId::new(
        bevy_egui::egui::Order::Foreground,
        bevy_egui::egui::Id::new("foredeck"),
    ));
    painter.text(
        bevy_egui::egui::pos2(screen.center().x, screen.top() + 64.0),
        bevy_egui::egui::Align2::CENTER_CENTER,
        said,
        bevy_egui::egui::FontId::monospace(14.0),
        bevy_egui::egui::Color32::from_rgb(232, 176, 58),
    );
    Ok(())
}

/// `SAIL_ANCHORED=1` starts her lying to it in the nearest anchorage.
///
/// **Because anchoring is twenty minutes of sailing away from where she starts.** The
/// water she opens in is forty metres deep — which is why the foredeck refuses it — and
/// the anchorages that *are* anchorages are a mile and a half off. A knob that puts her
/// where the thing happens is how this gets photographed, filmed and looked at, which is
/// the same argument as `SAIL_BRIG_SHOW` and `SAIL_SAY`.
pub fn brought_up(mut sim: ResMut<Sim>, course: Res<crate::course::Course>, mut done: Local<bool>) {
    if *done || crate::knob::var("SAIL_ANCHORED").is_err() {
        return;
    }
    let Some(harbour) = course.harbours.first().map(|h| h.at) else {
        return;
    };
    *done = true;
    let depth = sim
        .sim
        .land
        .as_ref()
        .map(|land| land.depth(harbour))
        .unwrap_or(8.0);
    sim.sim.state.pos = harbour;
    sim.sim.state.surge = 0.0;
    sim.sim.state.sway = 0.0;
    sim.sim.anchor = Anchor::Down(Down {
        at: harbour,
        depth,
        out: ((depth + sail_core::anchor::BOW_ROLLER) * GOOD_SCOPE).min(sim.sim.tackle.chain),
        dug: Ground::at(harbour).sets_in(),
        ground: Ground::at(harbour),
        dragged: 0.0,
    });
    info!(
        "anchor: brought up in {depth:.1} m over {}",
        Ground::at(harbour).charted()
    );
}

/// `V`: let go, or weigh.
///
/// One key, because it is one decision — *are we stopping here* — and the two halves of
/// it can never be wanted at the same time.
pub fn the_key(keys: Res<ButtonInput<KeyCode>>, sim: Res<Sim>, mut cable: ResMut<Cable>) {
    if !keys.just_pressed(KeyCode::KeyV) {
        return;
    }
    let now = sim.sim.state.time;
    // Alongside, the foredeck's key is the one that lets go of the shore — because that
    // is what the hand at the bow is holding, and because letting an anchor go while
    // made fast to a pontoon is not a thing anybody does.
    if cable.fast_to.take().is_some() {
        cable.shoved_off = true;
        cable.say("lines off", now);
        return;
    }
    match sim.sim.anchor {
        Anchor::Stowed => {
            let depth = sim.sim.telemetry.depth;
            // **Too deep is an answer, and the foredeck gives it before the anchor goes
            // over.** Fifty metres of chain is an anchorage of ten and a lunch stop of
            // twenty; past that what you get for letting go is an afternoon of hauling
            // and a scope that was never going to hold. Said rather than refused
            // silently, because a key that does nothing is the worst of the three
            // possible answers.
            if depth * SHALLOW_ENOUGH > sim.sim.tackle.chain {
                cable.say(format!("{depth:.0} m — too deep to lie in"), now);
                return;
            }
            // Five to one on the water under her, or all of it if she is in water deep
            // enough that five to one is more chain than there is.
            // Over the water *and the roller*, which is where the chain starts: five
            // times the sounding is four to one by the time it is measured.
            let want =
                ((depth + sail_core::anchor::BOW_ROLLER) * GOOD_SCOPE).min(sim.sim.tackle.chain);
            cable.want = Some(want);
            cable.say(format!("let go — veering {want:.0} m"), now);
        }
        Anchor::Down(_) => {
            cable.want = Some(0.0);
            cable.say("weighing", now);
        }
    }
}

/// Work the chain, and keep the watch.
pub fn work(
    time: Res<Time>,
    mut sim: ResMut<Sim>,
    mut cable: ResMut<Cable>,
    sound: Res<crate::sound::SoundHandle>,
) {
    let dt = time.delta_secs_f64();
    let now = sim.sim.state.time;
    if cable.said.as_ref().is_some_and(|(_, until)| now > *until) {
        cable.said = None;
    }
    let heading = sim.sim.state.heading;
    let bow = sim.sim.state.pos
        + glam::DVec2::new(heading.sin(), heading.cos()) * sim.sim.config.form.lwl * 0.5;
    let depth = sim.sim.telemetry.depth;
    let tackle = sim.sim.tackle;

    // --- Letting go ---------------------------------------------------------------
    //
    // The anchor goes down where the bow is now, in whatever is on the bottom there.
    if matches!(sim.sim.anchor, Anchor::Stowed) {
        let Some(want) = cable.want.filter(|w| *w > 0.0) else {
            cable.want = None;
            return;
        };
        let _ = want;
        sim.sim.anchor = Anchor::Down(Down {
            at: bow,
            depth: depth.max(1.0),
            out: 0.0,
            dug: 0.0,
            ground: Ground::at(bow),
            dragged: 0.0,
        });
        cable.alarmed = false;
        cable.stalled = false;
        info!(
            "anchor: let go in {:.1} m over {}",
            depth,
            Ground::at(bow).charted()
        );
        return;
    }

    let Anchor::Down(mut down) = sim.sim.anchor else {
        return;
    };
    let mut stowed = false;
    if let Some(want) = cable.want {
        let want = want.clamp(0.0, tackle.chain);
        if (down.out - want).abs() < 0.05 {
            cable.want = None;
            cable.stalled = false;
        } else if want > down.out {
            down.out = (down.out + VEERS * dt).min(want);
            cable.stalled = false;
        } else {
            // Hauling. The windlass lifts chain and breaks out an anchor; it does not
            // pull a boat to windward, so while the chain is loaded nothing comes in and
            // the console says so.
            let load = sail_core::anchor::pull(bow, &down, tackle, sim.sim.state.velocity_world())
                .length();
            if load > STALLS_AT {
                if !cable.stalled {
                    cable.stalled = true;
                    cable.say("the chain is bar taut — sail up to it", now);
                }
            } else {
                cable.stalled = false;
                down.out = (down.out - WEIGHS * dt).max(want);
                if down.out <= 0.4 && want <= 0.0 {
                    stowed = true;
                }
            }
        }
    }

    // --- The watch -----------------------------------------------------------------
    if down.dragged > ALARM_AT && !cable.alarmed {
        cable.alarmed = true;
        sound.0.alarm();
        cable.say("she is dragging", now);
        warn!("anchor: dragging — {:.0} m", down.dragged);
    }

    sim.sim.anchor = if stowed {
        cable.want = None;
        cable.say("anchor's aweigh", now);
        info!("anchor: aweigh");
        Anchor::Stowed
    } else {
        Anchor::Down(down)
    };
}

/// Lying alongside: the lines go on when she arrives slowly enough, and hold her there.
///
/// **Held rather than welded.** The platform is itself moored and lies to the weather, so
/// what is kept is the offset between the two of them: she follows it, which is what two
/// hulls on four lines do, instead of being snapped to a point on the chart.
pub fn make_fast(
    mut sim: ResMut<Sim>,
    berth: Res<crate::anchorage::Berth>,
    mut cable: ResMut<Cable>,
) {
    let Some(platform) = berth.at else {
        cable.fast_to = None;
        return;
    };
    let now = sim.sim.state.time;
    let here = sim.sim.state.pos;
    if let Some(offset) = cable.fast_to {
        // Made fast: she lies where the lines put her, with no way on. The sea still
        // moves her about — that is the fenders' problem and `contact.rs` has it — but
        // she goes nowhere.
        sim.sim.state.pos = platform + offset;
        sim.sim.state.surge *= 0.1;
        sim.sim.state.sway *= 0.1;
        sim.sim.state.yaw_rate *= 0.3;
        return;
    }
    // Not fast yet: near enough to pass a line, and slow enough that it is a line and
    // not a snub.
    let off = here - platform;
    let reach = sim.sim.config.form.lwl * 0.42 + crate::anchorage::RAFT as f64 * 0.75;
    if off.length() > reach {
        // Clear of it: whatever happened last time is over, and a line may be passed
        // the next time she comes up to it.
        cable.shoved_off = false;
        return;
    }
    if cable.shoved_off || sim.sim.state.velocity_world().length() > ALONGSIDE {
        return;
    }
    if !matches!(sim.sim.anchor, Anchor::Stowed) {
        return;
    }
    cable.fast_to = Some(off);
    cable.say("made fast alongside", now);
    info!("moored: alongside the platform");
}

/// What to tell the deck about the ground tackle, in one line.
///
/// **Scope first, because scope is the decision.** Then the ground, because it is what
/// the scope has to be enough for, and then whether she is walking — which is the only
/// one of the three that is news.
pub fn reading(sim: &Sim, cable: &Cable) -> Option<String> {
    if cable.fast_to.is_some() {
        return Some("made fast alongside".to_string());
    }
    let Anchor::Down(down) = &sim.sim.anchor else {
        return None;
    };
    let scope = down.scope();
    Some(format!(
        "{:.0} m  {scope:.1}:1  {}{}",
        down.out,
        down.ground.charted(),
        if down.dragged > 2.0 {
            format!("  dragging {:.0} m", down.dragged)
        } else {
            String::new()
        }
    ))
}

#[cfg(test)]
mod the_foredeck {
    use super::*;
    use sail_core::units::ms_to_kn;

    /// A world with a boat in it and nothing else going on.
    fn aboard() -> App {
        let mut app = App::new();
        // `MinimalPlugins` for the clock: without it `Time` never advances, the chain
        // veers nothing a frame and the test measures a windlass that is switched off.
        app.add_plugins(MinimalPlugins);
        app.insert_resource(Sim::new(crate::weather::Weather::draw(42), 42));
        app.insert_resource(crate::sound::SoundHandle(std::sync::Arc::new(
            crate::sound::SeaParams::default(),
        )));
        app.init_resource::<Cable>();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        // And anchored in water a boat anchors in: five to one on fifty metres of chain
        // is ten metres of water, which is an anchorage. The sea she is drawn into is
        // forty metres deep, where nobody anchors anything.
        {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            sim.sim.telemetry.depth = 8.0;
        }
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(100),
        ));
        app.add_systems(Update, (the_key, work).chain());
        app
    }

    /// Press the key for exactly one frame, the way a hand does.
    ///
    /// **And clear it**, because `MinimalPlugins` has no input plugin and nothing else
    /// clears `just_pressed`. Without the clear the key is still down on the next frame
    /// and the next: one press let the anchor go and then immediately weighed it again,
    /// and the test read a chain that never veered a metre.
    fn press(app: &mut App) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyV);
        app.update();
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release(KeyCode::KeyV);
        keys.clear();
    }

    /// **One key, and it veers five to one by itself.** The scope that is right is the
    /// scope the almanac gives for the water she is in, and working it out is arithmetic
    /// rather than seamanship — so the foredeck does it and the deck watches the chain
    /// go out.
    #[test]
    fn letting_go_veers_a_working_scope() {
        let mut app = aboard();
        press(&mut app);
        // The chain takes a while to run out, which is what makes it a thing you watch.
        for _ in 0..400 {
            app.update();
        }
        let sim = app.world().resource::<Sim>();
        let Anchor::Down(down) = sim.sim.anchor else {
            panic!("nothing went over the bow")
        };
        assert!(
            (down.scope() - GOOD_SCOPE).abs() < 1.2,
            "she veered {:.0} m in {:.1} m of water, a scope of {:.1}",
            down.out,
            down.depth,
            down.scope()
        );
        assert!(down.out <= sim.sim.tackle.chain);
    }

    /// **And the same key gets it back**, slowly, because a windlass is slow.
    #[test]
    fn weighing_takes_as_long_as_a_windlass_takes() {
        let mut app = aboard();
        press(&mut app);
        for _ in 0..400 {
            app.update();
        }
        let out = {
            let Anchor::Down(down) = app.world().resource::<Sim>().sim.anchor else {
                panic!("nothing went over the bow")
            };
            down.out
        };
        press(&mut app);
        // A minute of hauling, which at a windlass's pace is some of it and not all.
        for _ in 0..600 {
            app.update();
        }
        let Anchor::Down(down) = app.world().resource::<Sim>().sim.anchor else {
            panic!("it came up in a minute, which is not a windlass")
        };
        assert!(down.out < out, "the chain did not come in at all");
        // And it does come up, given the time it takes.
        for _ in 0..4_000 {
            app.update();
        }
        assert!(
            matches!(app.world().resource::<Sim>().sim.anchor, Anchor::Stowed),
            "it was still down after ten minutes of hauling"
        );
        assert!(app
            .world()
            .resource::<Cable>()
            .said
            .as_ref()
            .is_some_and(|(said, _)| said.contains("aweigh")));
    }

    /// **The windlass will not pull the boat to windward**, which is the whole reason
    /// weighing is a manoeuvre: a loaded chain stops the drum, and the console says why
    /// rather than leaving a button that does nothing.
    #[test]
    fn a_loaded_chain_stalls_the_drum() {
        let mut app = aboard();
        {
            // Lying hard back on short chain: the load is well past what a windlass
            // lifts.
            let mut sim = app.world_mut().resource_mut::<Sim>();
            let heading = sim.sim.state.heading;
            let bow = sim.sim.state.pos
                + glam::DVec2::new(heading.sin(), heading.cos()) * sim.sim.config.form.lwl * 0.5;
            sim.sim.anchor = Anchor::Down(Down {
                at: bow - glam::DVec2::new(0.0, 24.0),
                depth: 6.0,
                out: 25.0,
                dug: 6.0,
                ground: Ground::Sand,
                dragged: 0.0,
            });
        }
        app.world_mut().resource_mut::<Cable>().want = Some(0.0);
        let before = {
            let Anchor::Down(down) = app.world().resource::<Sim>().sim.anchor else {
                unreachable!()
            };
            down.out
        };
        for _ in 0..100 {
            app.update();
        }
        let Anchor::Down(down) = app.world().resource::<Sim>().sim.anchor else {
            unreachable!()
        };
        assert_eq!(down.out, before, "the windlass pulled the boat up to it");
        assert!(app.world().resource::<Cable>().stalled);
    }

    /// **The watch goes off once**, and only for the anchor's own walking — a boat
    /// sheering about on her chain is not dragging, and an alarm that says she is will
    /// be turned off on the first night.
    #[test]
    fn the_drag_alarm_goes_off_for_dragging_and_not_for_sheering() {
        let mut app = aboard();
        {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            let at = sim.sim.state.pos;
            sim.sim.anchor = Anchor::Down(Down {
                at,
                depth: 6.0,
                out: 30.0,
                dug: 6.0,
                ground: Ground::Sand,
                dragged: 0.0,
            });
        }
        app.update();
        assert!(app.world().resource::<Cable>().said.is_none());
        // Now walk the anchor past the alarm.
        {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            if let Anchor::Down(down) = &mut sim.sim.anchor {
                down.dragged = ALARM_AT + 1.0;
            }
        }
        app.update();
        assert!(app
            .world()
            .resource::<Cable>()
            .said
            .as_ref()
            .is_some_and(|(said, _)| said.contains("dragging")));
        let _ = ms_to_kn(1.0);
    }
}

#[cfg(test)]
mod lying_alongside {
    use super::*;
    use sail_core::units::kn_to_ms;

    fn at_the_platform(knots: f64) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(Sim::new(crate::weather::Weather::draw(42), 42));
        app.insert_resource(crate::sound::SoundHandle(std::sync::Arc::new(
            crate::sound::SeaParams::default(),
        )));
        app.init_resource::<Cable>();
        app.init_resource::<crate::anchorage::Berth>();
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.add_systems(Update, (the_key, work, make_fast).chain());
        let here = app.world().resource::<Sim>().sim.state.pos;
        {
            let mut berth = app.world_mut().resource_mut::<crate::anchorage::Berth>();
            berth.at = Some(here + glam::DVec2::new(8.0, 0.0));
        }
        {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            sim.sim.state.surge = kn_to_ms(knots);
        }
        app
    }

    /// **Coming alongside is arriving slowly**, and the lines go on by themselves when
    /// you do — because passing a line is what the crew is for and *how fast you arrive*
    /// is the only part of it the person steering decides.
    #[test]
    fn she_is_made_fast_when_she_arrives_slowly() {
        let mut app = at_the_platform(0.3);
        app.update();
        assert!(
            app.world().resource::<Cable>().fast_to.is_some(),
            "she lay alongside at a third of a knot and nobody took a line"
        );
    }

    /// **And three knots is not coming alongside.** `contact.rs` has already called that
    /// one a collision; the lines stay on the pontoon.
    #[test]
    fn arriving_hard_is_not_mooring() {
        let mut app = at_the_platform(3.0);
        app.update();
        assert!(app.world().resource::<Cable>().fast_to.is_none());
    }

    /// **The foredeck's key lets go of the shore first.** A hand at the bow holding a
    /// mooring line is not going to let an anchor go with the other one.
    #[test]
    fn the_key_casts_off_before_it_anchors() {
        let mut app = at_the_platform(0.2);
        app.update();
        assert!(app.world().resource::<Cable>().fast_to.is_some());
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::KeyV);
        }
        app.update();
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.release(KeyCode::KeyV);
            keys.clear();
        }
        let cable = app.world().resource::<Cable>();
        assert!(cable.fast_to.is_none(), "she stayed fast");
        assert!(matches!(
            app.world().resource::<Sim>().sim.anchor,
            Anchor::Stowed
        ));
    }

    /// **She follows the platform rather than a point on the chart.** The raft is itself
    /// moored and lies to the weather; two hulls on four lines move together.
    #[test]
    fn made_fast_she_moves_with_what_she_is_tied_to() {
        let mut app = at_the_platform(0.2);
        app.update();
        let was = app.world().resource::<Sim>().sim.state.pos;
        {
            let mut berth = app.world_mut().resource_mut::<crate::anchorage::Berth>();
            berth.at = berth.at.map(|at| at + glam::DVec2::new(3.0, 4.0));
        }
        app.update();
        let now = app.world().resource::<Sim>().sim.state.pos;
        assert!(
            ((now - was) - glam::DVec2::new(3.0, 4.0)).length() < 0.01,
            "the platform moved five metres and she moved {:.1}",
            (now - was).length()
        );
    }
}
