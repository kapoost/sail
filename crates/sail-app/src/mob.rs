//! Man overboard: the dan buoy, the lifebuoy, and getting Oscar back aboard.
//!
//! The most serious thing that can happen on a small boat, and the one manoeuvre every
//! crew is supposed to have practised and almost none have. It belongs in a sailing
//! simulator for exactly that reason — it is the one piece of boat handling where the
//! cost of being bad at it is somebody's life, and it is practised with a dan buoy and a
//! fender precisely because you cannot practise it with a person.
//!
//! **Oscar is the dan buoy.** In a real drill the pole goes over the side and the crew
//! recovers *it*, calling it Oscar after the flag — flag O, red over yellow on the
//! diagonal, which is the signal for a man overboard and is what is sewn to the top of
//! every dan buoy ever made. So the pole here is both the casualty and the thing that
//! makes him findable: a head in the water disappears in the first wave trough, and two
//! metres of pole with a flag on it does not.
//!
//! **The lifebuoy is the thing you throw**, and when you throw it is the whole lesson.
//! A horseshoe thrown in the first two seconds lands within a boat's length of him; one
//! thrown after a quarter of a minute of getting organised lands wherever the boat had
//! got to by then, which at six knots is forty metres away and downwind of him. That
//! distance is reported at the end of the drill, because it is the number that teaches.
//!
//! **And the drift is why the whole thing is urgent.** A person in the water, and a dan
//! buoy with a drogue under it, go with the water and about three per cent of the wind —
//! a tenth of a knot in a summer breeze. The boat under bare poles does one to four
//! knots. So the gap opens at nearly the boat's own drift rate whatever you do, and
//! every second spent not sailing back is a metre of search area.

use bevy::prelude::*;
use glam::DVec2;
use sail_core::units::ms_to_kn;

use crate::course::Floating;
use crate::frame;
use crate::sim::Sim;

/// The dan buoy in its tube on the quarter, which is where one lives.
#[derive(Component)]
pub struct Stowed;

/// And the same pole in the water, which is a different object because it is no longer
/// attached to the boat.
#[derive(Component)]
pub struct Oscar;

/// The horseshoe lifebuoy, once it has been thrown.
#[derive(Component)]
pub struct Horseshoe;

/// How far **off the rail** he can be and still be got hold of, metres.
///
/// Two: an arm and a boathook over the side. Off the rail rather than from the middle of
/// the boat, which is the correction — reported from the deck as "I was touching him and
/// it said four metres", and it did, because the range was measured to her own position,
/// which is a point on the centreline amidships. A man at the bow of a ten-metre boat is
/// five metres from that point while his head is against the hull.
///
/// So the measure is the distance from *her side*: the nearest point on her centreline
/// between stem and transom, less her half-beam. The readout on screen is the same
/// function, because a number that says four when he is touching the planking is worse
/// than no number — and because the recovery test and the thing the player reads being
/// two different measurements is this repository's oldest fault.
const REACH: f64 = 2.0;
/// And how slowly she has to be going, knots.
///
/// Under one and a half. Faster than that and you cannot hold him — the water drags him
/// aft along the hull and out of your hands, which is what happens to crews who approach
/// at speed and is why the whole manoeuvre is about arriving stopped.
const ALONGSIDE_SPEED: f64 = 1.5;
/// Seconds he has to stay in reach before he is aboard.
const HANDS_ON: f64 = 2.0;
/// Running him down: this close to her side, this fast.
const RUN_DOWN: f64 = 1.0;
const RUN_DOWN_SPEED: f64 = 3.0;

/// How long he lasts, seconds.
///
/// Ten minutes. See `Drill::Lost` — this is a cold-water incapacitation time and not a
/// difficulty setting, which is why it is a constant with a reason rather than a knob.
pub const ENDURANCE: f64 = 600.0;

/// Where the tube is, along the boat. Abaft the cockpit, which ends at four metres.
const TUBE_Z: f32 = 4.35;

/// What fraction of the wind a thing in the water goes downwind at.
///
/// These are search-and-rescue numbers rather than invented ones, because the whole
/// drill turns on them. A person in the water — and a dan buoy with its drogue set,
/// which is what Oscar is — makes something like two per cent of the wind speed: mostly
/// submerged, held back by the water it is in, a third of a knot in a fresh breeze. A
/// horseshoe is a ring of closed-cell foam lying on top with nothing under it and makes
/// four to eight per cent, so it goes two or three times as fast.
///
/// That difference is why the throw cannot be late. The ring walks downwind away from
/// him at about half a knot in sixteen knots of breeze, which is fifteen metres a
/// minute: thrown early it is something he can swim to, thrown after a minute of getting
/// organised it is upwind of nothing and he is still where he was.
///
/// And neither of them is anywhere near the boat's own drift, which this engine's
/// validation puts at one to four knots under bare poles. The gap between you and him
/// opens at *your* rate, not his.
const SWIM_LEEWAY: f64 = 0.02;
const RING_LEEWAY: f64 = 0.05;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Drill {
    /// Everybody aboard and the pole in its tube.
    Secure,
    /// Somebody in the water.
    Overboard {
        /// Where he is now.
        oscar: DVec2,
        /// Where the alarm was raised, which is what a plotter's MOB button records and
        /// what you steer back to while you are looking for him.
        mark: DVec2,
        /// When he went in.
        since: f64,
        /// Where the lifebuoy is, once it has been thrown, and how far from him it
        /// landed — which is the number the drill is really scored on.
        ring: Option<DVec2>,
        thrown_at: Option<f64>,
        /// Seconds he has been within reach, at a speed that lets you hold him.
        hands_on: f64,
        /// Times she has sailed over him at speed.
        run_downs: u32,
    },
    /// And he is gone, which is the other way this ends.
    ///
    /// There is a clock on this and it is not a game mechanic. In water at the
    /// temperature of a northern summer a person has ten to twenty minutes before their
    /// hands stop working and they cannot hold a line or a ladder, and not much longer
    /// before they cannot hold their head up. Ten minutes is the number used here, which
    /// is the hopeful end of that band: generous for a crew who sail straight back, and
    /// not nearly enough for one who mill about looking for him.
    Lost {
        /// How far away he was when it stopped mattering.
        range: f64,
        ring_gap: Option<f64>,
    },
    /// And back aboard, with the numbers.
    Recovered {
        took: f64,
        /// How far the lifebuoy landed from him, if it was thrown at all.
        ring_gap: Option<f64>,
        /// And how long after he went in it left the boat, which is the number that
        /// explains the one above it.
        thrown_after: Option<f64>,
        run_downs: u32,
    },
}

#[derive(Resource)]
pub struct Overboard {
    pub drill: Drill,
}

impl Default for Overboard {
    fn default() -> Self {
        Self {
            drill: Drill::Secure,
        }
    }
}

impl Overboard {
    /// Put the pole over the side. Called by the tutorial, and by anything else that
    /// wants to start the drill.
    pub fn man_overboard(&mut self, at: DVec2, now: f64) {
        self.drill = Drill::Overboard {
            oscar: at,
            mark: at,
            since: now,
            ring: None,
            thrown_at: None,
            hands_on: 0.0,
            run_downs: 0,
        };
    }

    /// Whether anybody is in the water, which is most of what other modules want to ask.
    pub fn live(&self) -> bool {
        matches!(self.drill, Drill::Overboard { .. })
    }

    /// Where he is, for anything that has to work out a course to him.
    pub fn oscar(&self) -> Option<DVec2> {
        match self.drill {
            Drill::Overboard { oscar, .. } => Some(oscar),
            _ => None,
        }
    }

    /// How far he is off her rail, which is the number everything is decided on and the
    /// number the player is shown. See `off_the_rail`.
    pub fn off_the_rail(&self, sim: &Sim) -> Option<f64> {
        self.oscar().map(|him| off_the_rail(sim, him))
    }

    /// Seconds he has left, 1 at the moment he goes in and 0 when he is past holding on.
    pub fn endurance_left(&self, now: f64) -> f64 {
        match self.drill {
            Drill::Overboard { since, .. } => {
                ((ENDURANCE - (now - since)) / ENDURANCE).clamp(0.0, 1.0)
            }
            _ => 1.0,
        }
    }

    /// Range and true bearing to him, for the readout and for the tutorial's prompts.
    pub fn to_oscar(&self, from: DVec2) -> Option<(f64, f64)> {
        let Drill::Overboard { oscar, .. } = self.drill else {
            return None;
        };
        let off = oscar - from;
        // North is +y and bearings run clockwise from it, which is the convention the
        // whole simulation uses.
        Some((
            off.length(),
            off.x.atan2(off.y).rem_euclid(std::f64::consts::TAU),
        ))
    }
}

/// How far a point in the water is from her side.
///
/// Her centreline is a segment from stem to transom; the nearest point on it is where
/// she is closest to him, and her half-beam is how much hull there is between that point
/// and the water. Nothing clever, and it is the one definition — the recovery, the
/// run-down check and the range on screen all read it.
pub fn off_the_rail(sim: &Sim, him: DVec2) -> f64 {
    let st = &sim.render_state();
    let cfg = sim.config();
    let off = him - st.pos;
    let along = off.dot(st.forward()).clamp(-cfg.loa * 0.45, cfg.loa * 0.45);
    let nearest = st.pos + st.forward() * along;
    ((him - nearest).length() - cfg.beam * 0.5).max(0.0)
}

/// The pole: two and a bit metres of it, ballasted at the bottom, with flag O at the top.
///
/// Built about its own waterline rather than about its foot, so the same mesh is right
/// in the tube and right in the sea: the ballast hangs below zero and the flag is up at
/// the top, and a dan buoy floating properly shows about a metre and a half.
fn dan_buoy(w: &mut Workshop) -> (Handle<Mesh>, Handle<Mesh>, Handle<Mesh>) {
    let pole = w.meshes.add(Cylinder::new(0.022, 2.3));
    let collar = w.meshes.add(Cylinder::new(0.17, 0.22));
    let flag = w.meshes.add(Rectangle::new(0.42, 0.30));
    (pole, collar, flag)
}

/// The bits a spawn needs, so the two call sites below do not each take six arguments.
struct Workshop<'a> {
    meshes: &'a mut Assets<Mesh>,
    materials: &'a mut Assets<StandardMaterial>,
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    origin: Res<frame::RenderOrigin>,
    sim: Res<Sim>,
    boat: Query<Entity, With<crate::boat::BoatRoot>>,
) {
    let mut w = Workshop {
        meshes: &mut meshes,
        materials: &mut materials,
    };
    let (pole, collar, flag) = dan_buoy(&mut w);
    let white = w.materials.add(StandardMaterial {
        base_color: Color::srgb(0.88, 0.88, 0.86),
        perceptual_roughness: 0.7,
        ..default()
    });
    let orange = w.materials.add(StandardMaterial {
        base_color: Color::srgb(0.93, 0.42, 0.10),
        perceptual_roughness: 0.75,
        ..default()
    });
    // Flag O: divided on the diagonal, red above and yellow below. Two halves of one
    // rectangle, drawn as two rectangles because a diagonal split needs a texture or a
    // second quad and a second quad is cheaper to read.
    let red = w.materials.add(StandardMaterial {
        base_color: Color::srgb(0.78, 0.12, 0.10),
        perceptual_roughness: 0.9,
        unlit: false,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let yellow = w.materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.80, 0.12),
        perceptual_roughness: 0.9,
        double_sided: true,
        cull_mode: None,
        ..default()
    });

    // The pole in its tube, lashed to the quarter: a child of the boat, so it rides with
    // her. On the port quarter — clear of the wind vane, which is on the centreline right
    // aft, and outboard of the tiller, which sweeps forty degrees each way about a stock
    // a quarter of a metre abaft this. Both of those clearances are arithmetic and both
    // are in the test at the bottom of this file, because a dan buoy the tiller knocks
    // over every time the helm goes down is the kind of thing that is obvious on the
    // water and invisible here.
    let z = TUBE_Z;
    let x = -(crate::deck::deck_half_beam(z) - 0.16);
    let deck = crate::deck::deck_height(z);
    let Ok(root) = boat.single() else {
        warn!("no boat to lash a dan buoy to");
        return;
    };
    commands.entity(root).with_children(|b| {
        b.spawn((
            Stowed,
            Transform::from_xyz(x, deck + 0.70, z),
            Visibility::default(),
        ))
        .with_children(|p| {
            dress_buoy(p, &pole, &collar, &flag, &white, &orange, &red, &yellow);
            // And the tube it stands in, which is what holds it upright on the rail.
            p.spawn((
                Mesh3d(collar.clone()),
                MeshMaterial3d(white.clone()),
                Transform::from_xyz(0.0, -0.62, 0.0).with_scale(Vec3::new(0.52, 1.8, 0.52)),
            ));
        });
    });

    // The same pole in the water, hidden until it is needed.
    commands
        .spawn((
            Oscar,
            Floating {
                at: sim.sim.state.pos,
                lift: 0.0,
            },
            Transform::from_translation(origin.to_bevy(sim.sim.state.pos, 0.0)),
            Visibility::Hidden,
        ))
        .with_children(|p| {
            dress_buoy(p, &pole, &collar, &flag, &white, &orange, &red, &yellow);
        });

    // And the lifebuoy, likewise.
    let ring = w.meshes.add(Torus::new(0.26, 0.38));
    commands.spawn((
        Horseshoe,
        Floating {
            at: sim.sim.state.pos,
            lift: 0.04,
        },
        Mesh3d(ring),
        MeshMaterial3d(orange.clone()),
        Transform::from_translation(origin.to_bevy(sim.sim.state.pos, 0.0)),
        Visibility::Hidden,
    ));

    commands.init_resource::<Overboard>();
}

/// The pole itself, which is the same object in the tube and in the sea.
#[allow(clippy::too_many_arguments)]
fn dress_buoy(
    p: &mut ChildSpawnerCommands,
    pole: &Handle<Mesh>,
    collar: &Handle<Mesh>,
    flag: &Handle<Mesh>,
    white: &Handle<StandardMaterial>,
    orange: &Handle<StandardMaterial>,
    red: &Handle<StandardMaterial>,
    yellow: &Handle<StandardMaterial>,
) {
    // Built about its own waterline: the ballast below, the float at it, the flag above.
    p.spawn((
        Mesh3d(pole.clone()),
        MeshMaterial3d(white.clone()),
        Transform::from_xyz(0.0, 0.45, 0.0),
    ));
    p.spawn((
        Mesh3d(collar.clone()),
        MeshMaterial3d(orange.clone()),
        Transform::from_xyz(0.0, 0.02, 0.0),
    ));
    // Flag O at the head of it, two triangles' worth, hung in the fore-and-aft plane
    // and again across it so there is something to see from any angle — which is what a
    // real one does by flapping.
    for turn in [0.0_f32, std::f32::consts::FRAC_PI_2] {
        p.spawn((
            Mesh3d(flag.clone()),
            MeshMaterial3d(red.clone()),
            Transform::from_xyz(0.21 * turn.cos(), 1.42, 0.21 * turn.sin())
                .with_rotation(Quat::from_rotation_y(turn)),
        ));
        p.spawn((
            Mesh3d(flag.clone()),
            MeshMaterial3d(yellow.clone()),
            Transform::from_xyz(0.21 * turn.cos(), 1.10, 0.21 * turn.sin())
                .with_rotation(Quat::from_rotation_y(turn)),
        ));
    }
}

/// Throw the lifebuoy: the key, and the whole of what the crew can do in the first two
/// seconds.
pub fn throw(
    keys: Res<ButtonInput<KeyCode>>,
    sim: Res<Sim>,
    mut over: ResMut<Overboard>,
    mut ring: RingAfloat,
) {
    if !keys.just_pressed(KeyCode::Enter) {
        return;
    }
    let now = sim.sim.state.time;
    let st = &sim.render_state();
    // Over the stern, which is where it is racked and where it would go in.
    let at = st.pos - st.forward() * (sim.config().loa * 0.5);
    if let Drill::Overboard {
        ring: ref mut where_it_is,
        ref mut thrown_at,
        ..
    } = over.drill
    {
        if where_it_is.is_none() {
            *where_it_is = Some(at);
            *thrown_at = Some(now);
            if let Ok((mut float, mut vis)) = ring.single_mut() {
                float.at = at;
                *vis = Visibility::Inherited;
            }
        }
    }
}

/// What a thing floating in the water needs to be told each frame: where it is, and
/// whether it can be seen. Two of them, and each says it is not the other — because the
/// engine proves two queries disjoint from their filters rather than taking it on trust
/// that nothing is both the pole and the lifebuoy, and refuses to run the system
/// otherwise. Found by running it.
type OscarAfloat<'w, 's> = Query<
    'w,
    's,
    (&'static mut Floating, &'static mut Visibility),
    (With<Oscar>, Without<Horseshoe>),
>;
type RingAfloat<'w, 's> = Query<
    'w,
    's,
    (&'static mut Floating, &'static mut Visibility),
    (With<Horseshoe>, Without<Oscar>),
>;

/// And the pole in its tube, which only needs to know whether it is still aboard.
type InItsTube<'w, 's> =
    Query<'w, 's, &'static mut Visibility, (With<Stowed>, Without<Oscar>, Without<Horseshoe>)>;

/// The drift, the search, and getting him back.
///
/// Everything in the water goes downwind at its own small fraction of the breeze, the
/// boat is told nothing about any of it, and the recovery is decided by the two things
/// that decide it on the water: how close she is, and how slowly she is going.
pub fn update(
    sim: Res<Sim>,
    time: Res<Time>,
    mut over: ResMut<Overboard>,
    mut oscar: OscarAfloat,
    mut ring: RingAfloat,
    mut stowed: InItsTube,
    mut over_him: Local<bool>,
) {
    let now = sim.sim.state.time;
    let dt = time.delta_secs_f64();
    let live = over.live();

    if let Ok(mut vis) = stowed.single_mut() {
        *vis = if live {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    if let Ok((_, mut vis)) = oscar.single_mut() {
        *vis = if live {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    let mut done: Option<Drill> = None;
    if let Drill::Overboard {
        oscar: ref mut him,
        since,
        ring: ref mut ring_at,
        thrown_at,
        hands_on: ref mut hands,
        run_downs: ref mut downs,
        ..
    } = over.drill
    {
        // The drift. Sampled where each thing is rather than where the boat is, which
        // matters as soon as she has sailed far enough to be in a different gust.
        let breeze = sim.sim.wind.sample(*him, now);
        *him += breeze * SWIM_LEEWAY * dt;
        if let Some(at) = ring_at.as_mut() {
            let there = sim.sim.wind.sample(*at, now);
            *at += there * RING_LEEWAY * dt;
        }
        if let Ok((mut float, _)) = oscar.single_mut() {
            float.at = *him;
        }
        if let (Some(at), Ok((mut float, _))) = (*ring_at, ring.single_mut()) {
            float.at = at;
        }

        // And the recovery, which is two numbers and no cleverness: near enough to get
        // a hand on him over the rail, slow enough to hold him when you do.
        let range = off_the_rail(&sim, *him);
        let speed = ms_to_kn(sim.sim.telemetry.stw.abs());
        if range < RUN_DOWN && speed > RUN_DOWN_SPEED {
            if !*over_him {
                *downs += 1;
                *over_him = true;
            }
        } else if range > RUN_DOWN * 2.0 {
            *over_him = false;
        }
        if range < REACH && speed < ALONGSIDE_SPEED {
            *hands += dt;
        } else {
            *hands = 0.0;
        }
        // The clock. A drill that cannot be failed teaches nothing about hurrying, and
        // hurrying is the whole of this manoeuvre.
        if now - since > ENDURANCE {
            done = Some(Drill::Lost {
                range,
                ring_gap: ring_at.map(|r| (r - *him).length()),
            });
        } else if *hands >= HANDS_ON {
            done = Some(Drill::Recovered {
                took: now - since,
                ring_gap: ring_at.map(|r| (r - *him).length()),
                thrown_after: thrown_at.map(|t| t - since),
                run_downs: *downs,
            });
        }
    }
    if let Some(end) = done {
        let lost = matches!(end, Drill::Lost { .. });
        over.drill = end;
        if let Ok((_, mut vis)) = oscar.single_mut() {
            *vis = Visibility::Hidden;
        }
        // The lifebuoy stays in the water when he does not. It is the one thing out there
        // that was any use, and it is still floating.
        if let Ok((_, mut vis)) = ring.single_mut() {
            if !lost {
                *vis = Visibility::Hidden;
            }
        }
    }
}

/// What the recovery is worth, in the units a sailor would use.
///
/// Kept beside the drill rather than in the tutorial, because the tutorial is one user
/// of it: a boat that has a man-overboard button has this readout whether anybody is
/// being taught or not.
pub fn verdict(
    took: f64,
    ring_gap: Option<f64>,
    thrown_after: Option<f64>,
    run_downs: u32,
) -> String {
    let mut said = format!(
        "ABOARD IN {}:{:02}",
        (took / 60.0).floor() as u32,
        (took % 60.0).floor() as u32
    );
    match (ring_gap, thrown_after) {
        (Some(gap), Some(after)) => said.push_str(&format!(
            " — lifebuoy thrown after {after:.0} s, {gap:.0} m from him"
        )),
        (Some(gap), None) => said.push_str(&format!(" — lifebuoy {gap:.0} m from him")),
        _ => said.push_str(" — and nobody threw him anything"),
    }
    if run_downs > 0 {
        said.push_str(&format!(", sailed over him {run_downs} time(s)"));
    }
    said
}

#[cfg(test)]
mod tests {
    use super::*;
    use sail_core::units::kn_to_ms;

    /// The dan buoy is where one goes, and nothing on the boat can knock it over.
    ///
    /// Two clearances, both arithmetic: the tiller sweeps forty degrees each way about
    /// its stock and reaches a metre and a quarter forward of it, so at the tube's
    /// station it has a known lateral reach; and the wind vane stands on the centreline
    /// right aft. A pole that fouls either is obvious the first time the helm goes hard
    /// over on the water and invisible in the code.
    #[test]
    // Constant assertions on purpose: these are the numbers somebody tuning the drill
    // will reach for, and what they must not do is drift out of the band the comment
    // above them claims. A test that can only fail when an edit happens is the test for
    // that.
    #[allow(clippy::assertions_on_constants)]
    fn nothing_aboard_knocks_the_dan_buoy_over() {
        let x = (crate::deck::deck_half_beam(TUBE_Z) - 0.16) as f64;
        assert!(
            x > 0.25,
            "the tube is {x:.2} m off the centreline, which is on top of the vane"
        );
        // The tiller, at this station: it pivots abaft the tube and reaches forward, so
        // the arm crosses this station a short way from its pivot.
        let stock = 4.60_f32;
        let arm = (stock - TUBE_Z).max(0.0);
        let sweep = arm * (40.0_f32).to_radians().tan();
        assert!(
            x as f32 > sweep + 0.10,
            "the tiller reaches {sweep:.2} m out at this station and the tube is {x:.2}"
        );
        // And abaft the cockpit, not in it.
        assert!(
            TUBE_Z > 4.0,
            "the tube is in the cockpit well at z {TUBE_Z:.2}"
        );
    }

    /// The bearing to him is a bearing: clockwise from north, and north is where north is.
    ///
    /// Written because this is the arithmetic that read right and was ninety degrees out
    /// in the wind vane, in the same codebase, for the same reason: a bearing and a
    /// mathematical angle run opposite ways round and agree at nought.
    #[test]
    fn the_bearing_to_him_is_a_bearing() {
        let mut over = Overboard::default();
        for (off, want) in [
            (DVec2::new(0.0, 100.0), 0.0),
            (DVec2::new(100.0, 0.0), 90.0),
            (DVec2::new(0.0, -100.0), 180.0),
            (DVec2::new(-100.0, 0.0), 270.0),
            (DVec2::new(100.0, 100.0), 45.0),
        ] {
            over.man_overboard(off, 0.0);
            let (range, bearing) = over.to_oscar(DVec2::ZERO).expect("a live drill");
            assert!((range - off.length()).abs() < 1e-6);
            assert!(
                (bearing.to_degrees() - want).abs() < 1e-6,
                "{off:?} bears {:.1} and should bear {want:.1}",
                bearing.to_degrees()
            );
        }
    }

    /// A drill that has not started has nobody in the water, and one that has, has.
    #[test]
    fn the_drill_knows_whether_anybody_is_in_the_water() {
        let mut over = Overboard::default();
        assert!(!over.live());
        assert!(over.to_oscar(DVec2::ZERO).is_none());
        over.man_overboard(DVec2::new(10.0, 0.0), 12.0);
        assert!(over.live());
    }

    /// The verdict says the three things that matter and says them in sailor's units.
    #[test]
    fn the_verdict_reads_like_a_debrief() {
        let said = verdict(108.0, Some(14.0), Some(3.0), 0);
        assert!(said.contains("1:48"), "{said}");
        assert!(said.contains("14 m"), "{said}");
        assert!(said.contains("after 3 s"), "{said}");
        let nothing = verdict(61.0, None, None, 2);
        assert!(nothing.contains("nobody threw him anything"), "{nothing}");
        assert!(nothing.contains("2 time"), "{nothing}");
        assert!(nothing.contains("1:01"), "{nothing}");
    }

    /// Everything in the water drifts, and the lifebuoy drifts faster than he does.
    ///
    /// Which is the whole reason the throw has to be early.
    ///
    /// Held against the search-and-rescue figures rather than against taste: a person in
    /// the water makes one to three per cent of the wind, a lifering four to eight. The
    /// first version of this test asserted the ring did less than six tenths of a knot in
    /// sixteen of wind, which is not what a lifering does — the measurement was right and
    /// the test was wrong, which is the way round that costs an hour if nobody checks.
    #[test]
    fn the_lifebuoy_walks_away_from_him() {
        let fresh = kn_to_ms(16.0);
        let him = ms_to_kn(fresh * SWIM_LEEWAY);
        let ring = ms_to_kn(fresh * RING_LEEWAY);
        assert!(
            (0.15..0.5).contains(&him),
            "a man in the water doing {him:.2} knots in sixteen of wind"
        );
        assert!(
            (0.5..1.3).contains(&ring),
            "a lifebuoy doing {ring:.2} knots in sixteen of wind"
        );
        // They part company, which is the lesson, and slowly enough that an early throw
        // stays useful for minutes.
        assert!(
            ring - him > 0.25,
            "they drift together, so nothing is taught"
        );
        // And both are inside the boat's own bare-pole drift, so the distance that opens
        // up is hers and not theirs.
        assert!(ring < 0.8 * 2.0);
    }
}
