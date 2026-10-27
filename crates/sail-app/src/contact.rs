//! Hitting things that are not the sea bed.
//!
//! **The only solid thing in this game was the bottom.** Running aground has been in the
//! model since the islands were, and it is the right shape for it — a depth, a draft, and
//! an instant end. Everything else floating out there was a drawing: the platform you
//! built, the brig you are leading, the boats other people are sailing. You went through
//! them, and a world you can sail through is a world with nothing in it.
//!
//! **The simple kind, deliberately.** A hull is a circle of its own length, every other
//! floating thing is a circle of its own, and a contact is two circles overlapping. That
//! is all. It is wrong in the way a circle is wrong about a boat — she is three times as
//! long as she is wide — and it is right about the only two questions anybody asks of it:
//! *did I touch it* and *how hard*. The alternative is a separating-axis test on two
//! rotating rectangles, which is a day's work and a different set of bugs for an answer
//! nobody at this speed can tell apart.
//!
//! What it does on a touch is the half worth thinking about:
//!
//! * **It takes the way off her.** The component of her velocity going into the thing is
//!   removed; the component along it is kept, which is what fending off *is* and is why
//!   coming alongside at an angle works and coming in square does not.
//! * **It pushes her clear**, by exactly the overlap, so two hulls cannot end up inside
//!   each other and jitter there for ever.
//! * **And it reports, and the two reports are different.** Under a knot and a half it
//!   is a touch, which is how a boat comes alongside and is not news; over three it is
//!   *struck*, with the speed in it and the alarm behind it, because three knots into a
//!   pontoon is the difference between a scuffed fender and a hole.

use bevy::prelude::*;
use glam::DVec2;

use crate::sim::Sim;

/// Something solid, afloat.
#[derive(Clone, Copy, Debug)]
pub struct Afloat {
    pub at: DVec2,
    /// How far from its middle it is solid, metres.
    pub radius: f64,
    /// What it is called when it is hit.
    pub what: &'static str,
}

/// How fast a touch has to be before it is damage rather than seamanship, m/s.
///
/// **A knot and a half.** Coming alongside is a touch at a walking pace and a fender
/// taking it; anything that arrives faster than this is arriving faster than anybody
/// means to, and three times it is through the planking. The two numbers bracket the one
/// decision a skipper makes on approach, which is *how fast is too fast*.
const NUDGE: f64 = 0.8;
const DAMAGE: f64 = 1.6;

/// Everything solid and floating, this frame.
///
/// Gathered rather than registered: the things that can be hit are already resources
/// with positions in them, and a list built here cannot go stale the way a registry of
/// handles does when something is despawned and nobody says so.
pub fn afloat(
    brig: &crate::ship::Brig,
    fleet: &crate::fleet::Fleet,
    berth: &crate::anchorage::Berth,
) -> Vec<Afloat> {
    let mut out = Vec::new();
    if brig.afloat {
        out.push(Afloat {
            at: brig.at,
            // A brig is thirty-one metres of hull; the circle is her half-length, which
            // is generous across her and right along her — and being generous across a
            // three-hundred-ton ship is the correct error.
            radius: crate::ship::LOA as f64 * 0.5,
            what: "the brig",
        });
    }
    if let Some(at) = berth.at {
        out.push(Afloat {
            at,
            radius: crate::anchorage::RAFT as f64 * 0.6,
            what: "the platform",
        });
    }
    for ghost in &fleet.boats {
        out.push(Afloat {
            at: DVec2::new(ghost.drawn.pos[0] as f64, ghost.drawn.pos[1] as f64),
            radius: 5.5,
            what: "another boat",
        });
    }
    out
}

/// What a touch did.
#[derive(Resource, Default)]
pub struct Struck {
    /// What was hit and how hard, for the deck to be told.
    pub said: Option<(String, f64)>,
}

pub fn plugin(app: &mut App) {
    use crate::stages::Deck;
    app.init_resource::<Struck>();
    app.add_systems(Update, touching.in_set(Deck::Weather));
}

/// Push her out of anything she is inside, and say what she hit.
pub fn touching(
    mut sim: ResMut<Sim>,
    brig: Res<crate::ship::Brig>,
    fleet: Res<crate::fleet::Fleet>,
    berth: Res<crate::anchorage::Berth>,
    mut struck: ResMut<Struck>,
    sound: Res<crate::sound::SoundHandle>,
) {
    let now = sim.sim.state.time;
    if struck.said.as_ref().is_some_and(|(_, until)| now > *until) {
        struck.said = None;
    }
    let ours = sim.sim.config.form.lwl * 0.42;
    let here = sim.sim.state.pos;
    for thing in afloat(&brig, &fleet, &berth) {
        let off = here - thing.at;
        let range = off.length();
        let touching = ours + thing.radius;
        if range >= touching || range < 1e-6 {
            continue;
        }
        let out = off / range;
        // How fast she was going into it, which is the only speed that matters: a boat
        // passing close aboard at six knots is passing, and one drifting onto a pontoon
        // at a knot is arriving.
        let way = sim.sim.state.velocity_world();
        let into_it = -way.dot(out);
        // Out of it, by the overlap — so two hulls cannot be inside each other for two
        // frames running and jitter there.
        sim.sim.state.pos = thing.at + out * touching;
        if into_it > 0.0 {
            // The way that was going into it is gone; the way along it is kept, which is
            // what fending off does and is why an angled approach works.
            let kept = way + out * into_it;
            let body = sim.sim.state.to_body(kept);
            sim.sim.state.surge = body.x;
            sim.sim.state.sway = body.y;
        }
        if into_it > DAMAGE {
            sound.0.alarm();
            struck.said = Some((
                format!(
                    "struck {} at {:.1} knots",
                    thing.what,
                    sail_core::units::ms_to_kn(into_it)
                ),
                now + 8.0,
            ));
            warn!(
                "contact: {} at {:.1} kn",
                thing.what,
                sail_core::units::ms_to_kn(into_it)
            );
        } else if into_it > NUDGE && struck.said.is_none() {
            struck.said = Some((format!("touched {}", thing.what), now + 5.0));
        }
    }
}

#[cfg(test)]
mod what_she_can_hit {
    use super::*;
    use sail_core::units::kn_to_ms;

    fn aboard() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(Sim::new(crate::weather::Weather::draw(42), 42));
        app.insert_resource(crate::sound::SoundHandle(std::sync::Arc::new(
            crate::sound::SeaParams::default(),
        )));
        app.init_resource::<crate::ship::Brig>();
        app.init_resource::<crate::fleet::Fleet>();
        app.init_resource::<crate::anchorage::Berth>();
        app.init_resource::<Struck>();
        app.add_systems(Update, touching);
        app
    }

    /// Put the platform right where she is, with her sailing into it.
    fn alongside(app: &mut App, knots: f64) {
        let here = app.world().resource::<Sim>().sim.state.pos;
        let heading = app.world().resource::<Sim>().sim.state.heading;
        let ahead = DVec2::new(heading.sin(), heading.cos());
        {
            let mut berth = app.world_mut().resource_mut::<crate::anchorage::Berth>();
            berth.at = Some(here + ahead * 6.0);
        }
        let mut sim = app.world_mut().resource_mut::<Sim>();
        sim.sim.state.surge = kn_to_ms(knots);
        sim.sim.state.sway = 0.0;
    }

    /// **She stops being inside it.** Two solid things cannot occupy the same water, and
    /// a model that lets them is a model where the platform is a painting.
    #[test]
    fn she_is_pushed_out_of_what_she_is_inside() {
        let mut app = aboard();
        alongside(&mut app, 1.0);
        app.update();
        let sim = app.world().resource::<Sim>();
        let berth = app
            .world()
            .resource::<crate::anchorage::Berth>()
            .at
            .expect("a platform");
        let apart = (sim.sim.state.pos - berth).length();
        let touching = sim.sim.config.form.lwl * 0.42 + crate::anchorage::RAFT as f64 * 0.6;
        assert!(
            apart >= touching - 0.01,
            "she ended {apart:.1} m from the middle of a thing {touching:.1} m across"
        );
    }

    /// **And the way that was going into it is gone.** That is what a fender does, and it
    /// is the difference between coming alongside and bouncing off.
    #[test]
    fn a_touch_takes_the_way_off_her() {
        let mut app = aboard();
        alongside(&mut app, 2.0);
        app.update();
        let sim = app.world().resource::<Sim>();
        assert!(
            sail_core::units::ms_to_kn(sim.sim.state.surge) < 0.2,
            "she bounced off at {:.1} knots",
            sail_core::units::ms_to_kn(sim.sim.state.surge)
        );
    }

    /// **A walking pace is seamanship and three knots is a hole.** The two speeds bracket
    /// the one decision anybody makes on an approach, so the report has to tell them
    /// apart — and only the hard one is worth an alarm.
    #[test]
    fn a_nudge_and_a_collision_are_different_things() {
        let mut app = aboard();
        // Half a knot is how a boat comes alongside, and it is not news.
        alongside(&mut app, 0.5);
        app.update();
        assert!(
            app.world().resource::<Struck>().said.is_none(),
            "coming alongside at half a knot was reported as a collision"
        );

        let mut app = aboard();
        alongside(&mut app, 2.4);
        app.update();
        let said = app.world().resource::<Struck>().said.clone();
        assert!(
            said.as_ref().is_some_and(|(s, _)| s.starts_with("touched")),
            "a touch at two and a half knots was reported as {said:?}"
        );

        let mut app = aboard();
        alongside(&mut app, 5.0);
        app.update();
        let said = app.world().resource::<Struck>().said.clone();
        assert!(
            said.as_ref().is_some_and(|(s, _)| s.starts_with("struck")),
            "five knots into a platform was reported as {said:?}"
        );
    }

    /// **The brig is solid too**, which is the one that matters while you are leading
    /// her: *she has no brakes* was a thing her master said and nothing enforced.
    #[test]
    fn the_ship_you_are_leading_is_a_solid_object() {
        let mut app = aboard();
        let here = app.world().resource::<Sim>().sim.state.pos;
        {
            let mut brig = app.world_mut().resource_mut::<crate::ship::Brig>();
            brig.afloat = true;
            brig.at = here + DVec2::new(4.0, 0.0);
        }
        app.update();
        let sim = app.world().resource::<Sim>();
        let apart = (sim.sim.state.pos - (here + DVec2::new(4.0, 0.0))).length();
        assert!(
            apart > crate::ship::LOA as f64 * 0.5,
            "she was {apart:.0} m from the middle of a thirty-one metre ship"
        );
    }
}
