//! Man overboard: the dan buoy, the lifebuoy, and getting Oscar back aboard.
//!
//! The most serious thing that can happen on a small boat, and the one manoeuvre every
//! crew is supposed to have practised and almost none have. It belongs in a sailing
//! simulator for exactly that reason — it is the one piece of boat handling where the
//! cost of being bad at it is somebody's life, and it is practised with a dan buoy and a
//! fender precisely because you cannot practise it with a person.
//!
//! **Oscar is the man**, called that after flag O — red over yellow on the diagonal,
//! which is the signal for a man overboard. He is what is in the water and what has to
//! come back out of it, and he is drawn as what he is: a head, a pair of shoulders and
//! an orange lifejacket, about a third of a metre of which is above the surface. That is
//! deliberately hard to see, because it is deliberately hard to see. Losing sight of
//! somebody between two waves at fifty metres is the thing that kills them and no amount
//! of drawing a convenient flag over his head would teach it.
//!
//! **The lifebuoy is the thing you throw**, and when you throw it is the whole lesson. It
//! lives on edge against the after end of the companionway, which is where a hand falls
//! on it coming up out of the cabin — the one place on a flush-decked boat you can reach
//! without going forward. Thrown in the first two seconds it lands within a boat's length
//! of him; thrown after a quarter of a minute of getting organised it lands wherever the
//! boat had got to by then, which at six knots is forty metres away and downwind of him.
//! Both numbers are reported at the end of the drill, because that pair is what teaches.
//!
//! It is also the only thing out there that can be seen. An orange ring on the water is
//! visible when a head is not, which is the second reason to throw it early and close:
//! it marks him.
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

/// The lifebuoy in its lashings at the companionway, where a hand falls on it.
#[derive(Component)]
pub struct Stowed;

/// The man in the water.
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
pub const RUN_DOWN: f64 = 1.0;

/// How far off her rail a person goes in, metres.
///
/// Further out than `RUN_DOWN`, which is the whole point of naming it: somebody falling
/// off the rail is not somebody being run down, and if he lands inside that distance the
/// check fires on the accident itself. The first version put him a metre and a half off
/// the *centreline* of a boat three and a half metres in the beam — which is to say
/// inside her — so the range read nought at six knots and every debrief opened by
/// accusing the player of sailing over him. Found in review.
pub const CLEAR_OF_HER: f64 = 1.3;
const RUN_DOWN_SPEED: f64 = 3.0;

/// How long he lasts, seconds.
///
/// Ten minutes. See `Drill::Lost` — this is a cold-water incapacitation time and not a
/// difficulty setting, which is why it is a constant with a reason rather than a knob.
pub const ENDURANCE: f64 = 600.0;

/// Where the lifebuoy is lashed: on the starboard quarter, outboard, behind the
/// helmsman's shoulder.
///
/// Three things decide it and none is taste. A tiller sweeps forty degrees each way
/// about its stock and reaches a metre and a quarter forward, which is the whole cockpit
/// — so nothing of this size stands anywhere inboard between the hatch and the transom.
/// Hung *outboard* on the quarter it is clear of that sweep entirely, which is also where
/// a boat with no pushpit carries one: lashed to the topsides, half of it above the rail,
/// where a hand falls on it from the helm. And it goes to **starboard** because the creel
/// hangs off the port quarter on its lanyard, and the two would otherwise be swinging
/// through each other.
const RACK_Z: f32 = 4.30;
/// How big a lifebuoy is: a ring three quarters of a metre across, with a tube as thick
/// as a wrist.
const RING: (f32, f32) = (0.24, 0.37);
/// The strop it hangs on, metres. Short, and it has to be: a boat with a metre of
/// freeboard aft and a ring three quarters of a metre across has no room for a lanyard —
/// hang it on a fathom of line and it is in the sea before she has rolled once. A hand's
/// breadth of strop, and the ring still ends up a foot off the water, which is why it
/// dips when she puts her quarter down. Which is what was asked for.
const STROP: f32 = 0.10;
/// Half the tube's thickness, which is all of the ring there is *across* the boat: it
/// hangs in its own plane, flat along her side.
const TUBE: f32 = (RING.1 - RING.0) * 0.5;
/// And the pendulum's own length, which is the distance to what is swinging: the strop
/// plus the radius of the *tube's middle*, where the weight of a ring is. It read to the
/// outer edge before, which is sixty-five millimetres of pendulum that is not there — a
/// seven per cent error in the period, and the same number feeds the stop below, where it
/// matters more than the period does.
const LANYARD: f32 = STROP + (RING.0 + RING.1) * 0.5;

/// What the lanyard is carrying, for the pendulum in `swing`.
///
/// `buoyant` is the one number not to copy from the creel beside it: a wicker basket lets
/// the sea through and keeps almost none of its weight off the line, while a lifebuoy is
/// *trying* to float. At one and a bit it dips, checks, and comes out lagging instead of
/// swinging on through — which is the whole of what a ring of closed-cell foam looks like
/// when a wave takes it.
fn on_the_lanyard() -> crate::swing::Hung {
    crate::swing::Hung {
        length: LANYARD,
        // From its middle to its bottom edge, which is the part that finds the water.
        reach: RING.1,
        // How far it comes in before it is against her planking — **and the answer is
        // negative**, which is not a sign error to clamp away.
        //
        // A ring on a short strop from a cleat at the gunwale cannot hang plumb, because
        // plumb is inside the boat: the deck back here is drawn a quarter of a metre
        // inside the hull it carries, so the planking at the top of the ring is outboard
        // of the cleat the ring hangs from. What it does instead is rest leaning
        // outboard with its inboard side against her topsides, which is what a ring hung
        // over a rail actually does, and it is why it knocks when she rolls.
        //
        // Guessed at a tenth of a radian the other way, it sat eighty-five millimetres
        // inside her at rest and two hundred and twenty-four at the end of its swing —
        // measured by the session next door, who had the same fault in their creel and
        // wrote the arithmetic out rather than send a number.
        inboard: crate::swing::against_the_planking(
            crate::deck::HULL_ON_THE_QUARTER - 0.02,
            TUBE,
            LANYARD,
            crate::deck::HULL_ON_THE_QUARTER,
        ),
        outboard: 0.55,
        // Closed-cell foam in a canvas cover on planking: it knocks rather than thuds,
        // and it comes back.
        bounce: 0.45,
        buoyant: 1.05,
        damp: 0.9,
        damp_wet: 4.2,
    }
}

/// The coil of line that goes with the ring, metres.
///
/// Twenty, which is what was asked for and is also about right: a throwing line on a
/// thirty-four footer is fifteen to thirty metres of floating polypropylene, long
/// enough to reach somebody who has just gone over the side and short enough that a
/// person on the foredeck can throw the bight of it.
///
/// It decides how big the coil is drawn, and the arithmetic is worth writing down
/// because a coil the size of a dinner plate and a coil the size of a tractor tyre are
/// both "a coil" to whoever is modelling it. Eight-millimetre line is fifty square
/// millimetres in section; twenty metres of it is a thousand square centimetres of rope
/// to put somewhere. Coiled on a bight of three hundred millimetres across, each turn
/// is nine hundred and forty millimetres round, so it is one and twenty turns — and
/// twenty-one turns packed at the seven tenths a hand-made coil manages wants a bundle
/// about forty-five millimetres thick. Which is what is drawn: a hank three hundred
/// across made of rope a wrist thick, and it is the size of a dinner plate after all.
const COIL_LINE: f32 = 20.0;
/// Across the bight of the coil, metres: where the turns lie.
const COIL_BIGHT: f32 = 0.15;
/// The line itself: eight millimetres, which is what a throwing line is.
const COIL_DIAMETER: f32 = 0.008;
/// And how thick the bundle of turns is, worked out rather than chosen — so that
/// changing the length of line changes the size of the coil, which is the only way the
/// two can go on agreeing.
fn coil_tube() -> f32 {
    let turns = COIL_LINE / (std::f32::consts::TAU * COIL_BIGHT);
    let rope = std::f32::consts::PI * COIL_DIAMETER * COIL_DIAMETER * 0.25;
    // Seven tenths, which is about what a coil made up by hand packs to: rope is not
    // stacked, it is laid on, and there is air in a hank.
    (turns * rope / 0.7 / std::f32::consts::PI).sqrt()
}
/// How far forward of the ring the coil hangs.
///
/// The ring swings athwartships on its strop — it hangs in its own plane along her side
/// — so anything a bight's width forward of it is clear of the swing for good, and this
/// is a hand's reach from the ring, which is where the two things that go over together
/// have to be.
const COIL_FORWARD: f32 = 0.46;

/// Where the ring has got to, swinging on its lanyard.
#[derive(Resource, Default)]
pub struct Hanging(crate::swing::Swing);

// How wide the drawn hull is where the ring hangs is `deck::HULL_ON_THE_QUARTER`, and it
// lives there rather than here on purpose: it is a fact about the boat's planking, not
// about this fitting. It has now been rediscovered three times — twice on this lifebuoy
// and once on the creel — and a measurement found three times should not live in
// whichever module last tripped over it.

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
        /// Whether she is alongside him *now*, so that one pass counts once.
        ///
        /// True the moment he goes in, because at that moment he is against her
        /// topsides and she is doing six knots — which is a man falling off a boat and
        /// not a boat running a man down. It was a `Local` on the system before, which
        /// made it both wrong at the start of a drill and stale at the end of one: a
        /// drill that finished while she was over him swallowed the first real run-down
        /// of the next.
        brushing: bool,
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
            brushing: true,
        };
    }

    /// Whether anybody is in the water, which is most of what other modules want to ask.
    pub fn live(&self) -> bool {
        matches!(self.drill, Drill::Overboard { .. })
    }

    /// Where the alarm was raised, which is what a plotter's MOB button records and what
    /// the first leg of the pattern is measured from.
    pub fn mark(&self) -> Option<DVec2> {
        match self.drill {
            Drill::Overboard { mark, .. } => Some(mark),
            _ => None,
        }
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

/// The ring itself, which is the same object on deck and in the water.
fn lifering(w: &mut Workshop) -> Handle<Mesh> {
    w.meshes.add(Torus::new(RING.0, RING.1))
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
    mut images: ResMut<Assets<Image>>,
    origin: Res<frame::RenderOrigin>,
    sim: Res<Sim>,
    boat: Query<Entity, With<crate::boat::BoatRoot>>,
) {
    let mut w = Workshop {
        meshes: &mut meshes,
        materials: &mut materials,
    };
    let ring = lifering(&mut w);
    let orange = w.materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.41, 0.08),
        perceptual_roughness: 0.75,
        ..default()
    });
    let tape = w.materials.add(StandardMaterial {
        base_color: Color::srgb(0.90, 0.90, 0.88),
        perceptual_roughness: 0.5,
        ..default()
    });
    // Floating line, which is what a throwing line is made of and why it is this
    // colour: polypropylene is sold in orange and yellow because it has to be found on
    // the water, and it is the one rope on the boat that is not white or blue.
    let line = w.materials.add(StandardMaterial {
        base_color: Color::srgb(0.93, 0.74, 0.16),
        perceptual_roughness: 0.92,
        ..default()
    });
    // Before anything that can return: a world with no boat in it still has a drill
    // resource, or every system that reads one fails on every frame for ever instead of
    // the deck merely being bare.
    commands.init_resource::<Overboard>();
    commands.init_resource::<Hanging>();
    let Ok(root) = boat.single() else {
        warn!("no boat to lash a lifebuoy to");
        return;
    };

    // **Hung on a lanyard from a cleat at the rail**, not bolted flat to her side.
    //
    // Asked for from the deck, and it is the right answer: a lifebuoy made fast to a boat
    // is on a line with a slip in it, because the one thing it has to do is leave in a
    // hurry. What that buys is the thing you actually see from the helm — a ring that
    // swings as she rolls, knocks against the planking, and dips into the sea when she
    // puts her quarter down.
    //
    // The cleat goes on the gunwale rather than the deck edge: back here the drawn hull
    // stands a quarter of a metre outboard of the deck it carries (see
    // `deck::HULL_ON_THE_QUARTER`), so a fitting put on the deck line would be inside the
    // planking and the ring would swing through her.
    let deck = crate::deck::deck_height(RACK_Z);
    let gunwale = crate::deck::HULL_ON_THE_QUARTER - 0.02;
    commands.entity(root).with_children(|b| {
        // The cleat itself does not move.
        b.spawn((
            Mesh3d(w.meshes.add(Cuboid::new(0.05, 0.035, 0.14))),
            MeshMaterial3d(tape.clone()),
            Transform::from_xyz(gunwale - 0.03, deck + 0.02, RACK_Z),
        ));
        b.spawn((
            Stowed,
            Transform::from_xyz(gunwale, deck + 0.03, RACK_Z),
            Visibility::default(),
        ))
        .with_children(|p| {
            // The lanyard, hanging from the cleat.
            p.spawn((
                Mesh3d(w.meshes.add(Cylinder::new(0.010, STROP))),
                MeshMaterial3d(tape.clone()),
                Transform::from_xyz(0.0, -STROP * 0.5, 0.0),
            ));
            // And the ring under it, hanging in its own plane — along her side, so it
            // knocks on the planking edge-on the way one does.
            p.spawn((
                Mesh3d(ring.clone()),
                MeshMaterial3d(orange.clone()),
                Transform::from_xyz(0.0, -LANYARD, 0.0)
                    .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))
                    // Pressed a little out of round, which is what one hanging on a strop
                    // does: a ring of foam in a canvas cover is not a machined part, and
                    // its own weight pulls it long. Six per cent — enough to read as
                    // cloth and rope, not enough to look broken.
                    .with_scale(Vec3::new(1.06, 1.0, 0.94)),
            ));
        });

        // **Twenty metres of line in a coil, made fast to the cleat.**
        //
        // Asked for from the deck, and it is the half of this gear that does the work. A
        // ring thrown to somebody in the water is a ring you then have to go back for,
        // under sail, in whatever the weather is — which is the whole of why the drill is
        // hard. A ring on a line is a ring you can *pull*, and twenty metres of floating
        // line is the difference between a recovery and another approach.
        //
        // It hangs on the same cleat, a bight's width forward of the ring so the swing
        // never reaches it, and it is a hank rather than a tidy doughnut because a coil
        // made up by hand on a quarter never is: two turns of its own end round the
        // middle hold it, and the turns are what stop it being a rubber ring.
        let coil_z = RACK_Z - COIL_FORWARD;
        let coil_y = deck - 0.26;
        b.spawn((
            Mesh3d(w.meshes.add(Torus::new(
                COIL_BIGHT - coil_tube(),
                COIL_BIGHT + coil_tube(),
            ))),
            MeshMaterial3d(line.clone()),
            Transform::from_xyz(gunwale - 0.015, coil_y, coil_z)
                // In its own plane along her side, like the ring, because that is the
                // only way anything this size hangs on topsides without standing off
                // them like a shelf.
                .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))
                .with_scale(Vec3::new(1.0, 1.0, 0.88)),
        ));
        // The two turns that hold the hank together — rope round rope, which is what a
        // seizing is. They were blocks before, and a block on a coil reads as a cube of
        // something, which is what came back from the deck: *the line is a cube*.
        //
        // At the top and the bottom of the hank, where the bundle runs fore and aft, so
        // the collar round it is a small ring lying athwart the turns.
        for at in [-1.0, 1.0] {
            b.spawn((
                Mesh3d(
                    w.meshes
                        .add(Torus::new(coil_tube() + 0.002, coil_tube() + 0.009)),
                ),
                MeshMaterial3d(line.clone()),
                Transform::from_xyz(gunwale - 0.015, coil_y + COIL_BIGHT * at, coil_z)
                    .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            ));
        }
        // And the standing part: up from the coil to the cleat, in two lengths so it
        // hangs in a bight rather than standing like a wire. A line made fast is the
        // reason the coil is up there at all, and a coil hanging beside a cleat with
        // nothing going to it is a prop.
        let tie =
            |b: &mut ChildSpawnerCommands, from: Vec3, to: Vec3, meshes: &mut Assets<Mesh>| {
                let span = to - from;
                b.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.006, span.length()))),
                    MeshMaterial3d(line.clone()),
                    Transform::from_translation(from + span * 0.5)
                        .with_rotation(Quat::from_rotation_arc(Vec3::Y, span.normalize())),
                ));
            };
        let on_the_cleat = Vec3::new(gunwale - 0.03, deck + 0.02, RACK_Z);
        let top_of_coil = Vec3::new(gunwale - 0.015, coil_y + COIL_BIGHT, coil_z);
        let sag = Vec3::new(
            gunwale - 0.02,
            (on_the_cleat.y + top_of_coil.y) * 0.5 - 0.055,
            (on_the_cleat.z + top_of_coil.z) * 0.5,
        );
        tie(b, on_the_cleat, sag, w.meshes);
        tie(b, sag, top_of_coil, w.meshes);

        // **The grab bag**: a canvas holdall lying on the deck over the ring.
        //
        // The one piece of gear on a small boat that is packed for the day nobody plans
        // for — flares, a hand-held VHF, water, a torch, the ship's papers — and it
        // lives where a hand falls on it from the helm without going below. Over the
        // lifebuoy is exactly that place, and it keeps the two things you leave with
        // together.
        //
        // It is made by `stowage::holdall`, which is the same machinery the sail bags
        // are made by, because the first one here was a box with a lid and came back
        // from the deck as *it looks plastic*. It did. A primitive with a flat colour
        // on it is plastic whatever the colour is; what makes the sail bags read as
        // cloth is that they are creased, spread, flattened and marked with webbing and
        // a stencil. One way of making a bag on this boat, used twice.
        //
        // **And it lies along the deck edge rather than fore and aft.** She narrows
        // sharply back here — a third of a metre of half-beam in the length of this bag
        // — so one laid parallel to the centreline put its after end through her
        // bulwark, which is what came back from the deck. The angle is read off the
        // same table the deck is drawn from rather than guessed: half-beam a little
        // forward, half-beam a little aft, and the line between them is where anything
        // lying on the side deck has to lie.
        let bag_z = RACK_Z;
        let bag_deck = crate::deck::deck_height(bag_z);
        let bag_radius = 0.125;
        let bag_length = 0.24;
        // The run of the deck edge at this station, pointing forward — forward because
        // the bag's own axis maps that way round, which is the thing that decides which
        // way up a laid capsule ends up.
        let reach = bag_length * 0.5 + bag_radius;
        let along = Vec3::new(
            crate::deck::deck_half_beam(bag_z - reach) - crate::deck::deck_half_beam(bag_z + reach),
            0.0,
            -2.0 * reach,
        )
        .normalize();
        // And how far in from the edge: its own spread, and a hand's breadth more for
        // the toerail, which is what it was sitting in.
        let inset = bag_radius * 1.24 + 0.07;
        let (bag, stencil) = crate::stowage::holdall(
            bag_radius,
            bag_length,
            19.7,
            Vec3::new(0.0, bag_deck, bag_z),
        );
        let cloth = w.materials.add(StandardMaterial {
            base_color: Color::srgb(
                crate::stowage::BAG_CANVAS[0],
                crate::stowage::BAG_CANVAS[1],
                crate::stowage::BAG_CANVAS[2],
            ),
            base_color_texture: Some(images.add(stencil)),
            perceptual_roughness: 0.95,
            ..default()
        });
        b.spawn((
            Mesh3d(w.meshes.add(bag)),
            MeshMaterial3d(cloth),
            Transform::from_xyz(
                crate::deck::deck_half_beam(bag_z) - inset,
                bag_deck + bag_radius * 0.68 + 0.01,
                bag_z,
            )
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, along)),
        ));
        // The lashing over it, because anything on deck that is not tied down is over
        // the side by the second gybe. A line round the bag rather than a bar across it.
        b.spawn((
            Mesh3d(
                w.meshes
                    .add(Torus::new(bag_radius * 1.1, bag_radius * 1.1 + 0.008)),
            ),
            MeshMaterial3d(line.clone()),
            Transform::from_xyz(
                crate::deck::deck_half_beam(bag_z) - inset,
                bag_deck + bag_radius * 0.6,
                bag_z,
            )
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, along))
            .with_scale(Vec3::new(1.0, 1.0, 0.72)),
        ));
    });

    // The man in the water, hidden until there is one. A head, a pair of shoulders, an
    // orange lifejacket and one arm up: a third of a metre of him above the surface, and
    // that is the whole of what anybody on deck has to find.
    let skin = w.materials.add(StandardMaterial {
        base_color: Color::srgb(0.72, 0.56, 0.46),
        perceptual_roughness: 0.85,
        ..default()
    });
    let jacket = w.materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.46, 0.06),
        perceptual_roughness: 0.8,
        ..default()
    });
    let head = w.meshes.add(Sphere::new(0.115));
    let shoulders = w.meshes.add(Sphere::new(0.26));
    let arm = w.meshes.add(Capsule3d::new(0.055, 0.42));
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
            // The jacket sits in the water with most of itself under it, which is why a
            // lifejacket keeps a face clear and why it is nearly all you can see.
            p.spawn((
                Mesh3d(shoulders.clone()),
                MeshMaterial3d(jacket.clone()),
                Transform::from_xyz(0.0, 0.02, 0.0).with_scale(Vec3::new(1.0, 0.62, 0.8)),
            ));
            p.spawn((
                Mesh3d(head.clone()),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.0, 0.17, 0.04),
            ));
            // An arm up. It is the only thing about him that moves, and a drill where the
            // casualty is a floating object teaches you to look for a floating object.
            p.spawn((
                Mesh3d(arm.clone()),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.17, 0.26, -0.02).with_rotation(Quat::from_rotation_z(-0.38)),
            ));
        });

    // And the lifebuoy once it is in the water, which is a different object from the one
    // in the lashings because it is no longer attached to the boat.
    commands.spawn((
        Horseshoe,
        Floating {
            at: sim.sim.state.pos,
            lift: 0.03,
        },
        Mesh3d(ring),
        MeshMaterial3d(orange),
        Transform::from_translation(origin.to_bevy(sim.sim.state.pos, 0.0)),
        Visibility::Hidden,
    ));
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
    // Over the stern. It is racked at the companionway, but what the throw models is the
    // ring hitting the water, and a ring thrown by somebody standing in the cockpit goes
    // in astern of her.
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
type InItsTube<'w, 's> = Query<
    'w,
    's,
    (&'static mut Visibility, &'static mut Transform),
    (With<Stowed>, Without<Oscar>, Without<Horseshoe>),
>;

/// The drift, the search, and getting him back.
///
/// Everything in the water goes downwind at its own small fraction of the breeze, the
/// boat is told nothing about any of it, and the recovery is decided by the two things
/// that decide it on the water: how close she is, and how slowly she is going.
pub fn update(
    sim: Res<Sim>,
    time: Res<Time>,
    mut over: ResMut<Overboard>,
    mut hanging: ResMut<Hanging>,
    mut oscar: OscarAfloat,
    mut ring: RingAfloat,
    mut stowed: InItsTube,
) {
    let now = sim.sim.state.time;
    let dt = time.delta_secs_f64();
    let live = over.live();

    // The lashings are empty once the ring has gone over the side, and not before: it is
    // the gear leaving the boat that takes it out of them, not the man leaving the boat.
    let thrown = matches!(over.drill, Drill::Overboard { ring: Some(_), .. });
    if let Ok((mut vis, mut at)) = stowed.single_mut() {
        *vis = if thrown {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        // And it swings. The cleat is a pivot, the ring is a pendulum on a short lanyard,
        // and what it hangs over is her planking on one side and the sea underneath —
        // both of which it knocks against, which is the whole reason this is a pendulum
        // and not a filter on the heel angle.
        let st = sim.render_state();
        let heel = st.heel as f32;
        let wave = sim.sim.water.height(st.pos, st.time) as f32;
        // How far the cleat is above the water. The deck height is from the waterline, so
        // what moves it is her heave against the wave under her — and the heel, which on
        // a fitting this far outboard buries it or lifts it by most of a half-beam. The
        // sign is the other way round from the creel's because this is the other side of
        // the boat: she heels to starboard and the starboard rail goes *down*.
        let clearance = crate::deck::deck_height(RACK_Z) + st.heave as f32
            - wave
            - crate::deck::HULL_ON_THE_QUARTER * heel.sin();
        hanging.0.step(
            &on_the_lanyard(),
            heel,
            st.roll_rate as f32,
            clearance,
            time.delta_secs().min(0.05),
        );
        // Drawn the other way round too, so that "inboard" in the pendulum is inboard on
        // this side: a right-handed turn about the fore-and-aft axis takes a thing hung to
        // starboard *outboard*.
        at.rotation = Quat::from_rotation_z(-hanging.0.angle);
    }
    // And the ring that was thrown is picked up with him, or left floating while the
    // debrief for a man who was not. What it must not do is stay in the water for the
    // rest of the passage and be there again at the start of the next drill, which is
    // what it did: nothing ever took it out once the drill it belonged to had ended.
    if !live && !matches!(over.drill, Drill::Lost { .. }) {
        if let Ok((_, mut vis)) = ring.single_mut() {
            *vis = Visibility::Hidden;
        }
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
        ref mut brushing,
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
            if !*brushing {
                *downs += 1;
                *brushing = true;
            }
        } else if range > RUN_DOWN * 2.0 {
            *brushing = false;
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
        "OSCAR ABOARD IN {}:{:02}",
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
        said.push_str(
            match run_downs {
                1 => ", and sailed over him once".to_string(),
                2 => ", and sailed over him twice".to_string(),
                n => format!(", and sailed over him {n} times"),
            }
            .as_str(),
        );
    }
    said
}

#[cfg(test)]
mod tests {
    use super::*;
    use sail_core::units::kn_to_ms;

    /// The lifebuoy hangs where one hangs: clear of the tiller, clear of her planking,
    /// and low enough that the sea reaches it when she puts her quarter down.
    ///
    /// The tiller is the constraint that put it here. It pivots at the stock, reaches a
    /// metre and a quarter forward and swings forty degrees each way, so it sweeps every
    /// station between the companionway and the transom — three quarters of a metre of
    /// ring does not fit beside it anywhere inboard, and the arithmetic refused every
    /// position before anything was drawn.
    #[test]
    fn the_lifebuoy_hangs_clear_of_her_and_into_the_sea() {
        let stock = 4.60_f32;
        // Outboard of anything the tiller can reach at that station — and it is hung off
        // the gunwale, which is outboard of the deck entirely.
        let sweep = (stock - RACK_Z).max(0.0) * (40.0_f32).to_radians().tan();
        let pivot = crate::deck::HULL_ON_THE_QUARTER - 0.02;
        assert!(
            pivot > sweep + 0.15,
            "the tiller reaches {sweep:.2} m out at z {RACK_Z:.2} and the cleat is at \
             {pivot:.2}"
        );
        // To starboard, because the creel hangs off the port quarter.
        assert!(pivot > 0.0, "the lifebuoy is on the creel's side");

        let hung = on_the_lanyard();
        // Hanging plumb, the bottom of it is below the rail and above the water: it has
        // to dip when she rolls and not drag when she does not.
        let deck = crate::deck::deck_height(RACK_Z);
        let bottom = deck + 0.03 - (hung.length + hung.reach);
        assert!(
            (0.2..0.6).contains(&bottom),
            "the ring hangs with its bottom {bottom:.2} m over the water, which never \
             touches it or never leaves it"
        );
        // The inboard stop puts its inboard face exactly on the planking and never
        // through it — and it is a lean rather than a swing, because plumb is inside the
        // boat. Checked here as geometry rather than as a number anybody typed.
        let gunwale = crate::deck::HULL_ON_THE_QUARTER - 0.02;
        let face_at = |angle: f32| gunwale - LANYARD * angle.sin() - TUBE;
        assert!(
            (face_at(hung.inboard) - crate::deck::HULL_ON_THE_QUARTER).abs() < 1.0e-4,
            "at its stop the ring's inboard face is at {:.3} and her planking is at \
             {:.3}",
            face_at(hung.inboard),
            crate::deck::HULL_ON_THE_QUARTER
        );
        assert!(
            hung.inboard < 0.0,
            "a ring on a strop from the gunwale cannot hang plumb — plumb is inside her"
        );

        // And a sea worse than the model makes never gets it inside her, which is the
        // test worth having: the stop is only right if the thing that uses it respects
        // it under forcing.
        let mut swinging = crate::swing::Swing::default();
        let mut worst = f32::MIN;
        for i in 0..2_000 {
            let t = i as f32 * 0.01;
            // Thirty degrees of roll at a second and a half, which is a knockdown rather
            // than a sea.
            let heel = 0.52 * (t * 4.2).sin();
            let roll_rate = 0.52 * 4.2 * (t * 4.2).cos();
            swinging.step(&hung, heel, roll_rate, 1.0, 0.01);
            worst = worst.max(swinging.angle);
        }
        assert!(
            face_at(worst) >= crate::deck::HULL_ON_THE_QUARTER - 1.0e-3,
            "thrown about, it reached {:.3} and her planking is at {:.3}",
            face_at(worst),
            crate::deck::HULL_ON_THE_QUARTER
        );
        // And it floats. A ring of closed-cell foam is the one thing hung off this boat
        // whose weight the water takes completely.
        assert!(
            hung.buoyant >= 1.0,
            "a lifebuoy that sinks is not a lifebuoy"
        );
        // A lifebuoy rather than a bracelet: three quarters of a metre across.
        assert!(
            (0.60..0.85).contains(&(RING.1 * 2.0)),
            "a ring {:.2} m across is not a lifebuoy",
            RING.1 * 2.0
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

    /// A man goes into the water clear of her side, not against it.
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn he_goes_in_outside_the_run_down_check() {
        assert!(
            CLEAR_OF_HER > RUN_DOWN,
            "he lands {CLEAR_OF_HER:.1} m off her and the run-down check wants \
             {RUN_DOWN:.1}, so going overboard counts as being run over"
        );
        // And within an arm of her, because he went off the rail rather than being
        // thrown.
        assert!(CLEAR_OF_HER < 2.5);
    }

    /// The verdict says the three things that matter and says them in sailor's units.
    #[test]
    fn the_verdict_reads_like_a_debrief() {
        let said = verdict(108.0, Some(14.0), Some(3.0), 0);
        assert!(said.contains("OSCAR"), "{said}");
        assert!(said.contains("1:48"), "{said}");
        assert!(said.contains("14 m"), "{said}");
        assert!(said.contains("after 3 s"), "{said}");
        let nothing = verdict(61.0, None, None, 2);
        assert!(nothing.contains("nobody threw him anything"), "{nothing}");
        assert!(nothing.contains("sailed over him twice"), "{nothing}");
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
