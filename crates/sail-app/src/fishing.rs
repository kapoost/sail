//! A rod over the stern, and what is on the end of it.
//!
//! The boat is a Shetland fourareen. She was an open fishing boat until 1964 and the
//! deck she is wearing is the second half of her life — so a rod in a holder on her
//! quarter is not a minigame bolted to a sailing simulator, it is the thing she was
//! built to do. `deck.rs` tells that story; this file is the rest of it.
//!
//! The whole design rests on one decision, and everything good about it follows from
//! that decision: **fishing is not a separate set of controls**. A fish on the line at
//! six knots is a fish you are about to lose, because the water dragging a nine-kilo
//! ling broadside at six knots pulls harder than the line will stand — so the first
//! thing you do when something takes is get the way off her, and the gear for that is
//! the mainsheet, the dump, the backed headsail and the helm you already have. The rod
//! comes with no speed control. The boat is the speed control.
//!
//! What the player actually works:
//!
//! * **The reel**, which is a rate, because it is a handle you turn. Winding against a
//!   fish that is going the other way gains nothing at all, which is not a penalty — it
//!   is what a slipping drag *is*.
//! * **The drag**, which is a position, because it is a star you screw down and leave.
//!   This is the whole game. Too light and the fish never tires, because a fish only
//!   tires against what you make it pull; too tight and the hook comes out of its mouth.
//!   The rule of thumb printed on every spool is a quarter to a third of the line's
//!   breaking strain, and the dial in `rod.rs` has that band marked on it.
//! * **How much line is out**, which sets how deep the lure swims and therefore what can
//!   see it. There is no depth control. There is a length of line and a speed through
//!   the water, and the lure goes where those two put it.
//!
//! ## What is modelled, and what that buys
//!
//! The line is a spring with a slipping clutch at one end and an animal at the other.
//! Written that way because the ways to lose a fish fall out of it rather than being
//! special-cased:
//!
//! * The fish accelerates faster than the drag can give line, the spring takes up the
//!   difference, and the tension overshoots the setting on the dial. That is why the
//!   number on the dial is not the number the line sees.
//! * The hook is a steel point in soft tissue, and what opens the hole it sits in is
//!   load. Hold a fish too hard for too long and the hook pulls out — which, with braid
//!   this strong, is how a screwed-down drag actually loses a fish. The line does not
//!   part on a four-kilo pollack and it was dishonest to pretend otherwise; it parts on
//!   the bottom, and on something big while you are still sailing fast.
//! * The fish swims *at* the boat, the spring goes slack, and a head-shake with no bend
//!   in the rod puts the hook back out the way it went in.
//!
//! The fish is a force-velocity relation rather than a force: muscle cannot produce its
//! greatest force and its greatest speed at once, so a fish held hard swims slowly and a
//! fish given slack swims fast. One line of arithmetic, and it is the difference between
//! something that pulls and something that is alive. It is also why the same fish is
//! landed in forty seconds on a properly set drag and never landed at all on a loose
//! one: fatigue is charged for *work done against the line*, so a drag you have backed
//! off is a fish you are not tiring.
//!
//! ## No fish in this file jumps
//!
//! Fishing games put a leaping fish on the box because a salmon leaps. A mackerel does
//! not, a pollack does not, a saithe does not, and a cod has never left the water under
//! its own power in its life. At fifty-seven degrees north on a spring morning what you
//! are towing a lure past is mid-water and bottom fish, and what they do instead —
//! sound, bore, hang, shake — is more interesting anyway, because each one asks a
//! different thing of the hand on the reel.

use bevy::prelude::*;
use sail_core::rng::Pcg32;

use crate::sim::Sim;

/// What the line will stand, newtons.
///
/// Twenty pound braid: 0.20 mm, 89 N. Braid rather than monofilament, and the choice
/// matters more here than it looks. Mono stretches something like a fifth of its length
/// before it parts, and that stretch is a shock absorber a hundred metres long — braid
/// has none, so every snatch goes straight into the knot. The whole of the cushioning in
/// this system is the rod, which is why [`ROD`] is the most load-bearing number in the
/// file and why a stiff rod loses fish.
///
/// It is also why the line is thin enough to troll with: drag on the line itself is most
/// of what the reel feels at six knots, and mono of the same strength is half again as
/// thick and pulls like a sea anchor.
pub const BREAKING: f64 = 89.0;

/// How much line is on the spool, metres. A small multiplier reel.
///
/// It can be emptied, and a ling that empties it is a ling you were never going to land.
pub const SPOOL: f64 = 160.0;

/// The spring rate of rod and line together, newtons per metre.
///
/// Measured the way a rod is sold: a boat rod with a kilogramme hung on the tip bends
/// about eight centimetres, which is 120 N/m, and the braid behind it contributes almost
/// nothing. Everything dynamic in the fight is this number against [`SLIP`].
const ROD: f64 = 120.0;

/// How freely the spool turns once the drag has broken away, newton-seconds per metre.
///
/// A drag is not a wall: past its setting the spool accelerates and line goes out at a
/// rate set by how far past it you are. Nine newtons of overload buys a metre a second,
/// which is a run you can hear across the cockpit.
const SLIP: f64 = 9.0;

/// How much harder the drag holds before it has started to move than after.
///
/// Static friction, and it is the reason a drag set carefully at the dockside still
/// snatches on the first fish. Carbon washers with a night's pressure on them break away
/// at something like a third more than they then hold at, and the whole of that extra
/// arrives as a step the instant a fish bolts.
const STICTION: f64 = 1.3;

/// The fastest the reel will take line in with no load on it, metres per second.
///
/// A multiplier gains about three quarters of a metre a turn and two turns a second is a
/// brisk wind, so this is honest rather than generous.
const RETRIEVE: f64 = 0.80;

/// Tension at which you cannot turn the handle at all, newtons.
///
/// Not the reel's limit — yours. The gearing is what decides this, and the point of
/// having it is that winding is not the answer to everything: past about half the line's
/// strength the handle simply stops, and the way to gain line is to let the fish tire.
const STALL: f64 = 70.0;

/// How fast line runs off a spool in free spool, metres per second.
const PAYOUT: f64 = 2.6;

/// Drag of the line itself, newtons per metre of line per (metre per second) squared.
///
/// `½ρ·Cd·d`, with a factor for the fact that most of a trolled line lies along the flow
/// rather than across it: 1025 kg/m³, Cd 1.2 on a cylinder, 0.20 mm diameter, and 0.45
/// for the angle. Sixty metres out at six knots is thirty newtons, which is a third of
/// the line's strength with nothing on the hook — and that is not a modelling artefact,
/// it is why trollers run braid and why a drag has to be set with the lure in the water
/// rather than on the quayside.
const LINE_DRAG: f64 = 0.0553;

/// And the lure's own drag, newtons per (metre per second) squared.
///
/// Small, and it matters only at the very end of the line, where it is the only thing
/// the first metre of line has to pull against. Leaving it out made the line vertical at
/// the lure, which is not wrong by much and is wrong in the one place the shape of the
/// curve is decided.
const LURE_DRAG: f64 = 0.18;

/// What the lure weighs in water, newtons. Forty grammes of leaded spinner.
///
/// It is the *only* thing pulling the lure down, and that is a correction rather than a
/// simplification. The first version gave it a diving vane — a hydrofoil pointed down,
/// whose lift goes as the square of the speed exactly as the line's drag does, so the
/// angle came out the same at two knots and at six and the lure sat fifteen metres down
/// whatever you did. That is a deep-diving plug on a downrigger. It is not what anybody
/// tows behind a small boat, and it made the one thing you can actually see — the lure
/// working in the water astern — invisible at every speed.
///
/// A plain weighted lure behaves the other way round, and far more interestingly: the
/// weight is constant and the drag goes as the speed squared, so **speed is the depth
/// control**. At five knots it skips along in the first metre or two and you can watch
/// it. Take the way off her and it sinks fast — nine metres at two knots, thirty at
/// one — which is how you reach the fish that live deep, and is also why slowing down
/// for a fish puts the next cast of the lure on the bottom.
///
/// Two consequences worth stating because they are the whole of the game loop. Mackerel
/// are caught fast and shallow and cod are caught slow and deep, so the boat's speed
/// chooses the quarry before the tackle does. And the splash the lure makes is a depth
/// gauge you can see: no disturbance on the water means the lure is down.
const LURE_WEIGHT: f64 = 0.35;

/// How hard the water holds a fish being hauled sideways through it, newtons per
/// (metre per second) squared per kilogramme to the two thirds.
///
/// Area goes as mass to the two thirds for anything shaped like itself, which is the
/// only scaling law in this file and the only one it needs.
const HAUL: f64 = 20.0;

/// The same thing, for a fish being towed by a boat that has not slowed down.
///
/// Lower than [`HAUL`] because a towed fish streams head-on behind the boat and a hauled
/// one is coming in broadside, and a fish is many times as slippery one way round as the
/// other. It is still the largest force in the file at any speed worth sailing at.
const TOW: f64 = 3.3;

/// Line left when the fish is at the rod tip and the fight is over, metres.
const LEADER: f64 = 1.6;

/// How long a beaten fish will lie alongside before it finds one more run, seconds.
///
/// Twelve, which is two or three waves — long enough to pick a good one and far too
/// short to think about it.
const PATIENCE: f64 = 12.0;

/// Below this sea there is nothing to time: you lean over and lift it, metres.
const CALM: f64 = 0.25;

/// How much of the sea's own rise you have to catch it on.
///
/// A quarter, with a floor, which works out at about a third of each wave — generous
/// enough to be learnable and tight enough that a fish is not landed by holding the key
/// down. It scales with the sea rather than being an absolute, so a big day is not
/// easier merely because everything is moving faster.
const ON_THE_RISE: f64 = 0.25;

/// Gravity, for turning a weight into a force.
const G: f64 = 9.81;

/// What took the lure.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Quarry {
    Mackerel,
    Saithe,
    Pollack,
    Cod,
    Ling,
    /// The ground. It fights like a fish with infinite patience, which is to say it is
    /// the one thing here you cannot beat — you get the lure back by taking the way off
    /// her so the angle of pull can change, or you do not get it back.
    Bottom,
}

/// What a fish is doing at this moment.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Doing {
    /// Straight down, as hard as it has. A pollack's whole fight is the first ten
    /// seconds of this.
    Sound,
    /// Away, long and level. The saithe's answer to everything.
    Run,
    /// Round the boat, which is what a small fast fish does because it cannot get away.
    Circle,
    /// Nothing at all. A cod hanging in the tide, or a ling with its tail round a rock.
    Hang,
    /// Head-shaking, which is how a hook comes out.
    Shake,
    /// Beaten, and coming up.
    Done,
}

impl Doing {
    /// How hard it is trying, as a fraction of what it has.
    fn effort(self) -> f64 {
        match self {
            Doing::Sound => 1.0,
            Doing::Run => 0.92,
            Doing::Circle => 0.70,
            Doing::Hang => 0.30,
            Doing::Shake => 0.55,
            Doing::Done => 0.18,
        }
    }

    /// Which way it is going: down, and across.
    fn heading(self) -> (f64, f64) {
        match self {
            Doing::Sound => (1.0, 0.0),
            Doing::Run => (0.15, 0.55),
            Doing::Circle => (0.0, 1.0),
            Doing::Hang => (0.6, 0.0),
            Doing::Shake => (0.3, 0.3),
            Doing::Done => (-0.5, 0.1),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Doing::Sound => "going down",
            Doing::Run => "running",
            Doing::Circle => "circling",
            Doing::Hang => "hanging",
            Doing::Shake => "shaking its head",
            Doing::Done => "beaten",
        }
    }
}

/// Everything that distinguishes one fish from another.
///
/// A table rather than five branches of code, because the whole claim this file makes is
/// that the species differ in a few measurable ways and not in kind. If a pollack needs
/// special handling to fight like a pollack then the model is not describing fish, it is
/// describing five animations.
struct Nature {
    name: &'static str,
    /// Mean weight and spread, kilogrammes.
    mass: (f64, f64),
    /// Peak pull as a multiple of its own weight.
    ///
    /// Pound for pound the mackerel is the hardest thing in this list and the cod is the
    /// softest, which is the first thing anybody who has caught both will tell you and
    /// the single most important number for making them feel different.
    power: f64,
    /// Burst swimming speed, metres per second, with nothing holding it back.
    dash: f64,
    /// How long it can pull at its own weight before it is beaten, seconds.
    stamina: f64,
    /// How well a hook holds. A cod's mouth is a bucket; a mackerel's is paper.
    hold: f64,
    /// The depth it feeds at, mean and spread, metres.
    depth: (f64, f64),
    /// The speed it will chase a lure at, mean and spread, knots.
    speed: (f64, f64),
    /// The depth of water it is found over, mean and spread, metres.
    ground: (f64, f64),
    /// How many there are, relative to each other.
    plenty: f64,
    /// What it does when it feels the hook, and what it does after that.
    opening: Doing,
    repertoire: &'static [Doing],
}

const MACKEREL: Nature = Nature {
    name: "mackerel",
    mass: (0.45, 0.14),
    power: 2.6,
    dash: 3.2,
    stamina: 20.0,
    hold: 0.62,
    depth: (2.5, 3.0),
    speed: (5.0, 1.6),
    ground: (25.0, 40.0),
    plenty: 1.0,
    opening: Doing::Circle,
    repertoire: &[Doing::Circle, Doing::Circle, Doing::Shake],
};

const SAITHE: Nature = Nature {
    name: "saithe",
    mass: (2.2, 1.1),
    power: 1.9,
    dash: 2.8,
    stamina: 80.0,
    hold: 0.88,
    depth: (5.0, 4.0),
    speed: (3.6, 1.4),
    ground: (18.0, 25.0),
    plenty: 0.9,
    opening: Doing::Run,
    repertoire: &[Doing::Run, Doing::Run, Doing::Circle, Doing::Shake],
};

const POLLACK: Nature = Nature {
    name: "pollack",
    mass: (3.0, 1.4),
    power: 2.1,
    dash: 2.6,
    stamina: 45.0,
    hold: 0.80,
    depth: (9.0, 5.0),
    speed: (2.8, 1.2),
    ground: (22.0, 25.0),
    plenty: 0.7,
    opening: Doing::Sound,
    repertoire: &[Doing::Sound, Doing::Shake, Doing::Hang, Doing::Sound],
};

const COD: Nature = Nature {
    name: "cod",
    mass: (4.5, 2.2),
    power: 0.85,
    dash: 1.6,
    stamina: 35.0,
    hold: 0.95,
    depth: (16.0, 8.0),
    speed: (2.2, 1.0),
    ground: (35.0, 25.0),
    plenty: 0.55,
    opening: Doing::Hang,
    repertoire: &[Doing::Shake, Doing::Hang, Doing::Hang, Doing::Run],
};

const LING: Nature = Nature {
    name: "ling",
    mass: (8.0, 3.5),
    power: 1.3,
    dash: 2.2,
    stamina: 130.0,
    hold: 0.90,
    depth: (28.0, 12.0),
    speed: (1.8, 0.8),
    ground: (60.0, 30.0),
    plenty: 0.15,
    opening: Doing::Sound,
    repertoire: &[Doing::Hang, Doing::Sound, Doing::Run, Doing::Hang],
};

/// The ground, given a nature so the fight code never has to ask whether it is fishing.
///
/// Strength it will never need, no speed, no stamina, and a hold nothing will shake out.
/// What frees it is handled where the fight ends, because it is not a property of the
/// rock — it is a property of the boat having stopped.
const GROUND: Nature = Nature {
    name: "the bottom",
    mass: (6.0, 0.0),
    power: 1.0e4,
    dash: 0.0,
    stamina: 1.0e9,
    hold: 1.0e9,
    depth: (0.0, 1.0),
    speed: (0.0, 1.0),
    ground: (0.0, 1.0),
    plenty: 0.0,
    opening: Doing::Hang,
    repertoire: &[Doing::Hang],
};

impl Quarry {
    fn nature(self) -> &'static Nature {
        match self {
            Quarry::Mackerel => &MACKEREL,
            Quarry::Saithe => &SAITHE,
            Quarry::Pollack => &POLLACK,
            Quarry::Cod => &COD,
            Quarry::Ling => &LING,
            Quarry::Bottom => &GROUND,
        }
    }

    pub fn name(self) -> &'static str {
        self.nature().name
    }
}

/// Every species that swims, in the order they are rolled for. The ground is not in the
/// list because it is not caught, it is run into.
pub const SPECIES: [Quarry; 5] = [
    Quarry::Mackerel,
    Quarry::Saithe,
    Quarry::Pollack,
    Quarry::Cod,
    Quarry::Ling,
];

/// How deep a trolled lure swims, metres.
///
/// The line's drag is **distributed along it**, and that is the whole of why this is an
/// integral rather than a triangle. Write it as a triangle — all the drag lumped at the
/// lure, `depth = L·sin(atan(W/D))` — and the answer saturates: the drag grows with the
/// length of line exactly as fast as the length does, so past about fifteen metres the
/// lure never goes any deeper however much line you let out. That is not merely
/// inaccurate, it deletes the deep fish from the game, and it is what the first version
/// of this function did.
///
/// Done properly, each element of line hangs at the angle set by what is *below* it:
/// everything near the lure is steep, because only the lure's own small drag is holding
/// it back, and everything near the rod is nearly flat, because it is carrying the drag
/// of all the line beyond. Integrating `sin θ` along it has a closed form,
///
/// ```text
///   depth = (W/k)·[ asinh((D₀ + k·L)/W) − asinh(D₀/W) ]
/// ```
///
/// with `W` the downforce, `D₀` the lure's own drag and `k` the line's drag per metre.
/// It grows like a logarithm: the first twenty metres of line buy most of the depth and
/// the next eighty buy the rest, which is exactly the complaint every troller has about
/// getting a lure down without a downrigger.
///
/// `W` is a constant — the lure's weight — and `k` goes as the square of the speed, so
/// the whole thing collapses as she goes faster. That is the behaviour of every plain
/// trolled lure there has ever been, and it is why the troller's depth control is the
/// throttle and not the line counter. Here the throttle is the mainsheet.
pub fn lure_depth(line_out: f64, stw: f64) -> f64 {
    let v2 = (stw * stw).max(0.0);
    let line_out = line_out.max(0.0);
    let down = LURE_WEIGHT;
    let k = LINE_DRAG * v2;
    if k < 1e-9 {
        // Stopped: the line has no drag on it at all and the lure is straight under the
        // rod tip. The expression below is `0/0` here and the limit is this.
        return line_out;
    }
    let lure = LURE_DRAG * v2;
    let asinh = |x: f64| (x + (x * x + 1.0).sqrt()).ln();
    (down / k) * (asinh((lure + k * line_out) / down) - asinh(lure / down))
}

/// Whether the sea is lifting her hard enough to swing a fish aboard on.
///
/// A free function with the two numbers that decide it, because it is asked twice — once
/// by the hand on the key and once by the prompt that tells you the moment is there — and
/// a prompt that lit at a different instant from the one the key was judged against would
/// be worse than no prompt at all.
pub fn on_the_rise(lift: f64, swell: f64) -> bool {
    swell < CALM || lift > (ON_THE_RISE * swell).max(0.12)
}

/// A Gaussian preference: one where the thing is right, falling away either side.
fn liking(x: f64, (mean, spread): (f64, f64)) -> f64 {
    let z = (x - mean) / spread.max(1e-6);
    (-0.5 * z * z).exp()
}

/// The state of the water as the fishing needs it. A deliberately small window on the
/// simulation: speed, how much water there is, and where the sun is.
#[derive(Clone, Copy, Debug)]
pub struct Sea {
    /// Speed through the water, m/s. Not over the ground — a lure swims in the water.
    pub stw: f64,
    /// Depth under the boat, metres.
    pub sounding: f64,
    /// Solar elevation, radians. Negative is below the horizon.
    pub sun: f64,
    /// How fast she is rising, metres a second, positive up.
    ///
    /// The thing you time the last movement of the whole business to. Lifting a fish
    /// over a rail is not a feat of strength, it is a feat of timing: you let the sea
    /// bring the fish up to you and you swing it aboard on the rise. Do it as she drops
    /// away and the fish's whole weight comes onto a hook that has been working loose
    /// for five minutes.
    pub lift: f64,
    /// The sea running, metres. In flat water there is nothing to time and you simply
    /// lift the thing aboard, which is also true.
    pub swell: f64,
}

/// What the hands are doing this frame.
#[derive(Clone, Copy, Default, Debug)]
pub struct Hands {
    /// Positive winds in, negative gives line.
    pub reel: f64,
    /// Where the drag star is set, nought to one of the line's breaking strain.
    pub drag: f64,
}

/// A fish on the line.
#[derive(Clone, Debug)]
pub struct Hooked {
    pub what: Quarry,
    /// What it weighs, kilogrammes. Drawn when it takes and fixed thereafter.
    pub mass: f64,
    /// How deep it is, metres.
    pub depth: f64,
    /// How far it has gone off the line of the wake, as a fraction of the line that is
    /// out. Only for drawing; the fight does not care which way it went.
    pub across: f64,
    /// Which way it is working at the moment, as a sign.
    sway: f64,
    /// Work done against the line, in seconds of pulling at its own weight. The fish is
    /// beaten when this passes its stamina.
    pub spent: f64,
    /// What is left of the hook hold.
    pub hold: f64,
    pub doing: Doing,
    /// Seconds left in what it is doing.
    until: f64,
    /// Whether you have had a sight of it, which is a different moment from landing it
    /// and is the one everybody remembers.
    pub seen: bool,
    /// Seconds it has been on.
    pub fought: f64,
    /// Seconds it has lain alongside waiting to be lifted. Nought until it gets there.
    ///
    /// The one state in the fight that is *yours* to end rather than the fish's, and it
    /// does not last: a fish beside a boat is a fish with one more run in it, and every
    /// second you spend deciding is a second the hook works looser.
    pub alongside: f64,
    /// Whether it has made its run at the sight of the hull yet.
    bolted: bool,
}

impl Hooked {
    /// The most it can pull right now, newtons: its weight, times its power, times what
    /// it has left, times how hard it is currently trying.
    pub fn strength(&self) -> f64 {
        let n = self.what.nature();
        let left = (1.0 - self.spent / n.stamina).clamp(0.08, 1.0);
        self.mass * G * n.power * left * self.doing.effort()
    }

    /// How fast it would swim with nothing holding it, m/s.
    fn speed(&self) -> f64 {
        let n = self.what.nature();
        let left = (1.0 - self.spent / n.stamina).clamp(0.05, 1.0);
        n.dash * left * self.doing.effort()
    }

    /// How beaten it is, nought to one, for the gauge.
    pub fn beaten(&self) -> f64 {
        (self.spent / self.what.nature().stamina).clamp(0.0, 1.0)
    }
}

/// Where the rod is and what is happening to it.
#[derive(Clone, Debug, Default)]
pub enum Rod {
    /// Below. The holder on the quarter is empty.
    #[default]
    Stowed,
    /// Out, with the lure astern and nothing on it.
    Trolling,
    /// Something is on.
    Playing(Hooked),
}

/// A fish in the box, which is the only score this game keeps.
#[derive(Clone, Copy, Debug)]
pub struct Landed {
    pub what: Quarry,
    pub mass: f64,
}

/// The fight integrates at a fixed step for the same reason the hull does: the drag's
/// time constant is `SLIP/ROD`, seventy-five milliseconds, and a frame three times that
/// long integrated explicitly does not merely lose accuracy — it oscillates, and parts a
/// line on a fish that was never pulling.
pub const SUBSTEP: f64 = 1.0 / 120.0;

/// The gear, and everything that happens to it.
///
/// A `Resource` and otherwise plain data. That one derive is the only line in this file
/// that knows an engine exists, which is what lets the whole model be tested without
/// opening a window — the tests at the bottom build one of these, hand it a sea and a
/// pair of hands, and never mention Bevy.
#[derive(Resource, Debug)]
pub struct Fishing {
    pub rod: Rod,
    /// Where the drag star is set, nought to one of the breaking strain.
    ///
    /// Opens at a third, which is the number printed on the side of the spool and is
    /// right often enough that somebody who never touches it will still land mackerel
    /// and saithe. The fish that teach you to move it are the pollack and the ling.
    pub drag: f64,
    /// Line off the spool, metres.
    pub line_out: f64,
    /// What the line is feeling, newtons. The number the whole game is about.
    pub tension: f64,
    /// How deep the far end of the line is, metres — and never deeper than there is
    /// water, which is the correction this field exists for.
    ///
    /// It used to be worked out again wherever it was wanted: once in the model, once in
    /// the readout, once in the drawing. The readout's copy was the raw geometry with
    /// nothing clamping it, so it cheerfully reported the lure forty-nine metres down
    /// over forty metres of water — a lure eight metres inside the seabed. Three copies
    /// of an expression is three chances to forget the one thing that keeps it honest.
    pub depth: f64,
    /// Extension of the rod-and-line spring, metres. Negative is slack.
    stretch: f64,
    /// How fast line is leaving the spool against the drag, m/s. Drives the ratchet.
    pub paying: f64,
    /// What is in the box.
    pub caught: Vec<Landed>,
    /// And how many got away, which is the more interesting number.
    pub lost: u32,
    rng: Pcg32,
    /// Seconds since the last take, so one fish does not arrive on top of another.
    quiet: f64,
    /// A shoal does not contain one fish. For a while after a mackerel there are more.
    shoal: f64,
    /// How much more fish there are in the sea than there are, for looking at the
    /// fight. One in a game being played.
    plenty: f64,
    /// Leftover time, so the fight integrates at a fixed step whatever the frame rate.
    accumulator: f64,
}

impl Fishing {
    pub fn new(seed: u64) -> Self {
        // `SAIL_ROD=30` streams thirty metres before the first frame. The same kind of
        // switch as `SAIL_HEADSAIL` and `SAIL_SQUALL`, and for the same reason: a take
        // happens every minute or two at best, and you cannot look at a thing that
        // seldom often enough to get it right. A knob beats sitting and waiting.
        // `SAIL_FISH=20` makes them take twenty times as often. The fight is the
        // interesting part and a take is a once-in-two-minutes event, so testing one by
        // playing the other is an afternoon's work per change. It multiplies the rate
        // and nothing else: what takes, where, and how it fights are all unaltered, so
        // what you are looking at is the real thing arriving oftener.
        let plenty = std::env::var("SAIL_FISH")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(1.0)
            .clamp(0.0, 500.0);
        let streamed = std::env::var("SAIL_ROD")
            .ok()
            .map(|v| v.parse::<f64>().unwrap_or(16.0).clamp(1.0, SPOOL));
        Self {
            rod: if streamed.is_some() {
                Rod::Trolling
            } else {
                Rod::Stowed
            },
            line_out: streamed.unwrap_or(0.0),
            drag: 1.0 / 3.0,
            tension: 0.0,
            depth: 0.0,
            stretch: 0.0,
            paying: 0.0,
            caught: Vec::new(),
            lost: 0,
            rng: Pcg32::new(seed ^ 0xF15E),
            quiet: 0.0,
            shoal: 0.0,
            plenty,
            accumulator: 0.0,
        }
    }

    pub fn out(&self) -> bool {
        !matches!(self.rod, Rod::Stowed)
    }

    pub fn fish(&self) -> Option<&Hooked> {
        match &self.rod {
            Rod::Playing(f) => Some(f),
            _ => None,
        }
    }

    /// What the drag is set to in newtons, which is the only form of it worth reading.
    pub fn drag_force(&self) -> f64 {
        self.drag * BREAKING
    }

    /// Take the rod out, or put it away.
    ///
    /// Putting it away with a fish on is not offered. You cannot stow a bent rod, and a
    /// key that threw the fish back would be a key pressed by accident once and resented
    /// for the rest of the passage.
    pub fn stow_or_set(&mut self) -> &'static str {
        match self.rod {
            Rod::Stowed => {
                self.rod = Rod::Trolling;
                // Streamed to a working depth rather than dropped over the side at
                // nothing, because that is what a hand does: the lure goes in, the spool
                // runs, and you stop it with your thumb at about the right mark.
                self.line_out = 16.0;
                self.stretch = 0.0;
                self.quiet = 0.0;
                "rod out — lure astern, and the reel is on the arrows"
            }
            Rod::Trolling => {
                self.rod = Rod::Stowed;
                self.line_out = 0.0;
                self.tension = 0.0;
                self.stretch = 0.0;
                self.paying = 0.0;
                self.depth = 0.0;
                "rod stowed"
            }
            Rod::Playing(_) => "not with a fish on",
        }
    }

    /// Whether there is a fish alongside waiting to be lifted, and whether this is the
    /// moment to do it. For the prompt on the console, which is the whole of how anybody
    /// learns that the moment exists.
    pub fn ready_to_lift(&self, sea: Sea) -> Option<bool> {
        let fish = self.fish()?;
        if fish.alongside <= 0.0 {
            return None;
        }
        Some(on_the_rise(sea.lift, sea.swell))
    }

    /// Swing it aboard.
    ///
    /// The one moment in the whole business that is a question of timing rather than of
    /// judgement, and it is the moment that is actually like that on the water: you wait
    /// for the sea to bring the fish up to the rail and you lift on the rise. Pull as she
    /// drops away from under it and you are lifting the fish *and* the boat's departure,
    /// which is how a hook that has held for five minutes comes out in the last metre.
    pub fn lift_aboard(&mut self, sea: Sea) -> Option<String> {
        let ready = self.ready_to_lift(sea)?;
        let Rod::Playing(fish) = &mut self.rod else {
            return None;
        };
        if ready {
            let (what, mass, fought) = (fish.what, fish.mass, fish.fought);
            self.rod = Rod::Trolling;
            self.stretch = 0.0;
            self.line_out = 4.0;
            self.quiet = 0.0;
            self.caught.push(Landed { what, mass });
            return Some(format!(
                "{} — {mass:.1} kg, {fought:.0} seconds",
                what.name()
            ));
        }
        // Mistimed. Not fatal on its own — it costs you a chunk of the hook hold and
        // another run — because a game that ended on one badly judged keypress after a
        // minute of work would be a game nobody pressed the key in.
        fish.alongside = 0.0;
        fish.bolted = false;
        fish.hold -= 0.12;
        fish.doing = Doing::Run;
        fish.until = 2.0;
        self.stretch = self.stretch.max(0.4);
        Some("you lifted as she dropped away — it is off again".into())
    }

    /// Advance the gear. Returns anything worth saying out loud.
    pub fn step(&mut self, dt: f64, sea: Sea, hands: Hands) -> Option<String> {
        self.drag = hands.drag.clamp(0.0, 1.0);
        if matches!(self.rod, Rod::Stowed) {
            self.tension = 0.0;
            self.paying = 0.0;
            return None;
        }
        self.accumulator += dt.min(0.25);
        let mut said = None;
        while self.accumulator >= SUBSTEP {
            self.accumulator -= SUBSTEP;
            if let Some(word) = self.substep(SUBSTEP, sea, hands) {
                // The first thing that happened wins. Two events in one frame is a fish
                // taking and being lost inside eight milliseconds, which does not happen,
                // and if it ever did the first of them is the one to report.
                said.get_or_insert(word);
            }
        }
        said
    }

    fn substep(&mut self, dt: f64, sea: Sea, hands: Hands) -> Option<String> {
        let cap = self.drag * BREAKING;
        // What the hand is asking of the reel. Winding in is limited by the tension —
        // past the stall you cannot turn the handle at all — and giving line is not,
        // because a free spool does not care.
        let wanted = hands.reel;
        let reel = if wanted > 0.0 {
            wanted * RETRIEVE * (1.0 - self.tension / STALL).clamp(0.0, 1.0)
        } else {
            wanted * PAYOUT
        };

        if matches!(self.rod, Rod::Stowed) {
            return None;
        }
        if matches!(self.rod, Rod::Trolling) {
            let depth = lure_depth(self.line_out, sea.stw);
            // Never below the ground. The geometry does not know the sea has a bottom,
            // and a lure reported under it is the one reading nobody can believe.
            self.depth = depth.min((sea.sounding - 0.2).max(0.0));
            // Towing the lure and the line: the bend in the rod while you are simply
            // sailing along, which is most of what tells you the gear is fishing.
            let tow =
                LINE_DRAG * self.line_out * sea.stw * sea.stw + TOW * 0.25 * sea.stw * sea.stw;
            self.tension += (tow - self.tension) * (1.0 - (-dt / 0.35).exp());
            self.paying = ((self.tension - cap) / SLIP).max(0.0);
            self.line_out = (self.line_out + (self.paying - reel) * dt).clamp(0.0, SPOOL);
            self.stretch = 0.0;
            // A hundred and sixty metres of line towed at six knots pulls harder than
            // twenty pound braid will stand, with nothing whatever on the hook. That is
            // not a curiosity — it is the commonest way to lose tackle off a sailing
            // boat, because the load builds while you are busy with the sails and the
            // first you know of it is a straight rod. The line used to be checked for
            // breaking only during a fight, so this situation simply persisted: a
            // hundred newtons on an eighty-nine newton line, for ever.
            if self.tension > BREAKING {
                self.rod = Rod::Stowed;
                self.line_out = 0.0;
                self.tension = 0.0;
                self.paying = 0.0;
                self.depth = 0.0;
                return Some(format!(
                    "the line parted at {:.0} N with nothing on it — too much line at \
                     this speed",
                    BREAKING
                ));
            }
            // And the other end of the same story: the drag is lighter than the tow, so
            // line has been going out all along and there is none left.
            if self.line_out >= SPOOL - 0.01 && self.paying > 0.0 {
                self.rod = Rod::Stowed;
                self.line_out = 0.0;
                self.tension = 0.0;
                self.paying = 0.0;
                self.depth = 0.0;
                return Some(
                    "the spool emptied and the knot went — tighten the drag or slow down".into(),
                );
            }
            if self.line_out <= 0.05 && wanted > 0.0 {
                // Wound right in: the lure is at the rod tip and there is nothing left to
                // fish with, so the gear comes in rather than sitting there pretending.
                self.rod = Rod::Stowed;
                return Some("lure's at the tip — rod stowed".into());
            }
            return self.take(dt, sea, depth);
        }
        self.fight(dt, sea, cap, reel)
    }

    /// Roll for something taking the lure.
    ///
    /// A Poisson process whose rate is the product of how well the lure is presented and
    /// how many of that fish there are: at the right depth, over the right ground, at a
    /// speed the lure swims properly at, with the sun low. Everything multiplies, so
    /// being wrong about any one of them is enough to catch nothing — which is the honest
    /// account of trolling and the reason it is an activity rather than a reward.
    fn take(&mut self, dt: f64, sea: Sea, depth: f64) -> Option<String> {
        self.quiet += dt;
        self.shoal = (self.shoal - dt).max(0.0);
        if self.quiet < 6.0 / self.plenty.max(1.0) {
            return None;
        }
        let knots = sail_core::units::ms_to_kn(sea.stw);

        // The ground first, because a lure in the kelp cannot be taken by anything. The
        // last two metres of water are where the trouble is, and below trolling speed the
        // vane has stopped flying, so the lure is sinking into it.
        let clearance = sea.sounding - depth;
        if clearance < 2.0 && sea.sounding > 0.5 {
            let risk = (1.0 - clearance / 2.0).clamp(0.0, 1.0);
            if self.rng.next_f64() < 0.35 * risk * risk * dt {
                return Some(self.hook(Quarry::Bottom, sea));
            }
        }

        // Low sun is most of it. Dawn and dusk are when these fish feed and the middle of
        // the day is when they do not, and asking the sun's elevation rather than the
        // clock gets that right at fifty-seven degrees north without anybody having to
        // write down what time dusk is in April.
        let light = {
            let elevation = sea.sun.to_degrees();
            if elevation < -8.0 {
                0.22
            } else {
                0.38 + 0.62 * (-0.5 * (elevation / 11.0).powi(2)).exp()
            }
        };
        // The lure cannot be seen below the bottom, so match the species against where it
        // actually is rather than where the geometry put it.
        let swimming = depth.min(sea.sounding.max(0.5));

        for species in SPECIES {
            let n = species.nature();
            // One take in seventy seconds with everything right, which over a two-hour
            // passage is a fish box worth having and nowhere near a fish a minute.
            let mut rate = self.plenty * n.plenty / 70.0
                * liking(swimming, n.depth)
                * liking(knots, n.speed)
                * liking(sea.sounding, n.ground)
                * light;
            if species == Quarry::Mackerel {
                // A shoal is not one fish. Having had one, the next few minutes are the
                // best fishing of the day, and that is the whole reason anybody keeps
                // trolling the same patch of water.
                rate *= 1.0 + 5.0 * (self.shoal / 180.0).min(1.0);
            }
            if self.rng.next_f64() < rate * dt {
                return Some(self.hook(species, sea));
            }
        }
        None
    }

    /// Something is on.
    fn hook(&mut self, what: Quarry, sea: Sea) -> String {
        let n = what.nature();
        let mass = (n.mass.0 + n.mass.1 * self.rng.next_normal()).max(n.mass.0 * 0.35);
        let depth = lure_depth(self.line_out, sea.stw).min((sea.sounding - 0.5).max(1.0));
        self.rod = Rod::Playing(Hooked {
            what,
            mass,
            depth,
            across: 0.0,
            sway: if self.rng.next_f64() < 0.5 { -1.0 } else { 1.0 },
            spent: 0.0,
            // Some hooks are well set and some are in the skin of a lip, and which one
            // you have is not knowable until it comes out. The spread is what makes two
            // identical fights end differently.
            hold: n.hold * (0.55 + 0.45 * self.rng.next_f64()),
            doing: n.opening,
            until: 3.0 + 4.0 * self.rng.next_f64(),
            seen: false,
            fought: 0.0,
            alongside: 0.0,
            bolted: false,
        });
        self.quiet = 0.0;
        if what == Quarry::Mackerel {
            self.shoal = 180.0;
        }
        if what == Quarry::Bottom {
            return "the lure is in the ground — get the way off her".into();
        }
        // Not named. You do not know what it is until you see it, and being told would
        // throw away the best thirty seconds the activity has.
        if sail_core::units::ms_to_kn(sea.stw) > 4.0 {
            "something has it — and you are doing five knots".into()
        } else {
            "something has it".into()
        }
    }

    /// One substep of the fight.
    ///
    /// Four lines of physics with everything else arranged around them: the fish's speed
    /// from its force-velocity relation, the spring between the fish and the spool, the
    /// drag letting line go once the spring passes its setting, and fatigue charged for
    /// the work the line does on the fish.
    fn fight(&mut self, dt: f64, sea: Sea, cap: f64, reel: f64) -> Option<String> {
        let Rod::Playing(fish) = &mut self.rod else {
            return None;
        };
        fish.fought += dt;
        let n = fish.what.nature();
        let fixed = fish.what == Quarry::Bottom;

        // --- What it is doing, and for how much longer ---------------------------
        fish.until -= dt;
        if fish.until <= 0.0 {
            let beaten = fish.beaten();
            fish.doing = if beaten > 0.85 {
                Doing::Done
            } else {
                // A tired fish stops choosing the hard options. Weighting the repertoire
                // by what it has left is the difference between a fight with a shape and
                // a fight that is the same for a minute and then stops.
                let pick = (self.rng.next_f64() * n.repertoire.len() as f64) as usize;
                let want = n.repertoire[pick.min(n.repertoire.len() - 1)];
                if beaten > 0.6 && matches!(want, Doing::Sound | Doing::Run) {
                    Doing::Hang
                } else {
                    want
                }
            };
            fish.until = match fish.doing {
                Doing::Sound => 4.0 + 5.0 * self.rng.next_f64(),
                Doing::Run => 5.0 + 8.0 * self.rng.next_f64(),
                Doing::Circle => 4.0 + 6.0 * self.rng.next_f64(),
                Doing::Hang => 3.0 + 6.0 * self.rng.next_f64(),
                Doing::Shake => 1.2 + 1.8 * self.rng.next_f64(),
                Doing::Done => 1.0e9,
            };
            fish.sway = if self.rng.next_f64() < 0.35 {
                -fish.sway
            } else {
                fish.sway
            };
        }

        // --- The forces -----------------------------------------------------------
        // Being towed. This is the term that makes the boat part of the tackle: a fish
        // broadside to six knots of water pulls harder than anything in this file can
        // pull itself, so the answer to a big one is the mainsheet. The ground is not
        // towed — it is the boat that is moving away from it, which is the line below.
        let v2 = sea.stw * sea.stw;
        let tow = if fixed {
            LINE_DRAG * self.line_out * v2
        } else {
            LINE_DRAG * self.line_out * v2 + TOW * fish.mass.powf(2.0 / 3.0) * v2
        };
        self.tension = (ROD * self.stretch.max(0.0) + tow).max(0.0);

        let strength = fish.strength();
        let haul = HAUL * fish.mass.powf(2.0 / 3.0);
        // Muscle cannot give its greatest force and its greatest speed at once. Held
        // below its strength the fish swims away at what is left; held above it, it comes
        // towards the boat at the speed the water will let it be dragged. The ground does
        // neither: it stays where it is, and the boat sails away from it.
        let away = if fixed {
            sea.stw
        } else if self.tension < strength {
            fish.speed() * (1.0 - self.tension / strength.max(1e-6))
        } else {
            -(((self.tension - strength) / haul).sqrt())
        };

        // --- The spool ------------------------------------------------------------
        // Static friction before it has broken away, and the set value after. That step
        // is the shock that snatches at a hook, and it is why a drag that measured right
        // on the bench still loses the first fish of the day.
        let breakaway = cap * if self.paying > 0.01 { 1.0 } else { STICTION };
        self.paying = if self.tension > breakaway {
            ((self.tension - cap) / SLIP).max(0.0)
        } else {
            0.0
        };
        let supplied = self.paying - reel;
        self.line_out = (self.line_out + supplied * dt).max(0.0);
        // The spring takes up whatever the fish's movement and the line supply disagree
        // about, which is the one equation the whole fight runs on.
        self.stretch = (self.stretch + (away - supplied) * dt).clamp(-2.0, 4.0);

        // --- Where it is, for the drawing ----------------------------------------
        let (down, across) = fish.doing.heading();
        let travel = away.abs().max(0.2);
        let floor = (sea.sounding - 0.5).max(1.0);
        fish.depth = (fish.depth + down * travel * dt * 0.8)
            .clamp(0.3, floor)
            .min(self.line_out.max(1.0));
        self.depth = fish.depth;
        fish.across = (fish.across + across * fish.sway * travel * dt * 0.08).clamp(-0.75, 0.75);
        if fish.depth < 1.5 && self.line_out < 14.0 {
            fish.seen = true;
        }

        // --- Fatigue ---------------------------------------------------------------
        // Charged for the work the line does on it, in seconds of pulling its own weight.
        // This is the reason a drag backed off to nothing never lands anything: a fish
        // that is not being made to pull is a fish that is not getting tired.
        fish.spent += self.tension / (fish.mass * G).max(1e-6) * dt;

        // --- The hook ---------------------------------------------------------------
        // Two different ways to lose it, and they pull in opposite directions, which is
        // why the drag has a right answer rather than a safe one.
        //
        // Load opens the hole the hook sits in, and steeply. Measured against the drag
        // rule rather than against the breaking strain, because the rule is what the
        // tackle is matched to: at a third of the line — the number on the spool — the
        // hook loses about a quarter of its hold over a two minute fight, and at three
        // times that it is out in twenty seconds.
        //
        // And a slack line during a head-shake lets the hook fall out the way it went in,
        // which is what "keep the rod up" means.
        let slack = self.tension < 6.0;
        let wear = 0.0018 * (self.tension / (BREAKING / 3.0)).powf(2.9)
            + if fish.doing == Doing::Shake {
                if slack {
                    0.30
                } else {
                    0.05
                }
            } else {
                0.0
            };
        fish.hold -= wear * dt;

        // --- Everything that ends it -----------------------------------------------
        let beaten = fish.beaten();
        let hold = fish.hold;

        if self.tension > BREAKING {
            self.give_up();
            return Some(format!(
                "the line parted — {BREAKING:.0} N of braid against {:.0}",
                self.tension.max(BREAKING)
            ));
        }
        if hold <= 0.0 {
            let pulled = self.tension > 0.45 * BREAKING;
            self.give_up();
            return Some(if pulled {
                "the hook pulled out — too much drag".into()
            } else {
                "it threw the hook".into()
            });
        }
        if self.line_out >= SPOOL - 0.01 {
            self.line_out = SPOOL;
            self.give_up();
            return Some("that was all the line there was".into());
        }
        if fixed {
            // Nothing beats the ground. It lets go when the angle of pull changes, which
            // means when the boat has stopped dragging the line one way — and a steady
            // pull from a stopped boat is what frees a snagged lure in life too.
            if sea.stw < 1.2 && self.tension > 15.0 && self.rng.next_f64() < 0.8 * dt {
                self.rod = Rod::Trolling;
                self.stretch = 0.0;
                self.quiet = 0.0;
                return Some("the lure came away".into());
            }
            return None;
        }
        if self.line_out <= LEADER {
            let roll = self.rng.next_f64();
            let Rod::Playing(fish) = &mut self.rod else {
                return None;
            };
            fish.seen = true;
            // **Every fish does this, and it is where most fish are lost.** It comes up,
            // sees the hull, finds something it did not have thirty seconds ago and goes
            // under the boat with it — and the leader is the shortest and least forgiving
            // piece of line in the whole system. There is no fish that comes quietly, and
            // a model in which a beaten one simply arrived was a model missing the most
            // famous ten seconds in angling.
            if !fish.bolted {
                fish.bolted = true;
                fish.doing = Doing::Run;
                fish.until = 1.6 + 2.2 * roll;
                fish.hold -= 0.04;
                self.stretch = self.stretch.max(0.3);
                return Some("it has seen the boat — and gone again".into());
            }
            if beaten > 0.78 {
                // Alongside, beaten, and waiting for you to pick your moment.
                fish.alongside += dt;
                if fish.alongside > PATIENCE {
                    fish.alongside = 0.0;
                    fish.bolted = false;
                    fish.hold -= 0.05;
                    fish.doing = Doing::Run;
                    fish.until = 2.0;
                    return Some("it would not lie there for ever".into());
                }
            } else {
                // Still has something left, so it will do it again when it comes back.
                fish.bolted = false;
            }
            self.stretch = self.stretch.max(0.15);
        }
        None
    }

    /// It has gone. Back to a lure astern, with the line where the fight left it.
    fn give_up(&mut self) {
        self.rod = Rod::Trolling;
        self.stretch = 0.0;
        self.tension = 0.0;
        self.paying = 0.0;
        self.lost += 1;
        self.quiet = 0.0;
    }
}

/// How fast the drag star turns, in fractions of the breaking strain per second.
///
/// Held rather than pressed, because a star drag is a wheel you turn and not a switch —
/// and because the useful band is narrow enough that stepping it would mean choosing
/// the step size for everybody. Shift is the fine turn, as it is everywhere else here.
const DRAG_RATE: f64 = 0.22;
const DRAG_FINE: f64 = 0.06;

/// Work the gear: the rod, the reel and the drag, and then advance the model.
///
/// Reads the keyboard directly, like the other eleven modules that do. The key list in
/// `help.rs` is checked against this file by a test, so a key added here and not
/// documented there fails the build rather than going unfound.
pub fn work(
    mut gear: ResMut<Fishing>,
    sim: Res<Sim>,
    sky: Res<crate::sky::Sky>,
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    time: Res<Time>,
    mut since_report: Local<f32>,
) {
    let dt = time.delta_secs_f64();
    let fine = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    let sea = Sea {
        stw: sim.sim.telemetry.stw,
        // What the sounder reads. Trolling is the one activity in this game that cares
        // where the deep water is, which is what makes the chart worth opening for a
        // reason other than not hitting anything.
        sounding: sim.sim.telemetry.depth,
        sun: sky.elevation() as f64,
        // The integrator's own heave rate rather than a difference of positions: it is
        // the quantity being timed, and differencing a drawn height would put the window
        // a frame late on exactly the frames that matter.
        lift: sim.sim.state.heave_rate,
        swell: sim.sim.telemetry.significant_height,
    };

    // One key, and what it does depends on what is on the end — which is not a trick,
    // it is the same hand doing the same thing. It reaches for the rod when there is
    // nothing on, and it reaches for the fish when there is.
    if keys.just_pressed(KeyCode::KeyJ) {
        if gear.ready_to_lift(sea).is_some() {
            if let Some(said) = gear.lift_aboard(sea) {
                info!("{said}");
            }
        } else {
            info!("{}", gear.stow_or_set());
        }
    }

    // The drag. A position your hand leaves behind, so it is set and kept rather than
    // held — which is also the only way it could work, since the hand you would hold it
    // with is the one on the reel.
    let turn = if fine { DRAG_FINE } else { DRAG_RATE };
    let mut drag = gear.drag;
    if keys.pressed(KeyCode::Equal) {
        drag += turn * dt;
    }
    if keys.pressed(KeyCode::Minus) {
        drag -= turn * dt;
    }

    // The reel. A rate, because it is a handle you turn — and the D-pad as well as the
    // arrows, because a gamepad is the intended input and the left stick is already the
    // tiller.
    let pad = pads.iter().next();
    let mut reel = 0.0;
    if keys.pressed(KeyCode::ArrowUp) || pad.is_some_and(|p| p.pressed(GamepadButton::DPadUp)) {
        reel += 1.0;
    }
    if keys.pressed(KeyCode::ArrowDown) || pad.is_some_and(|p| p.pressed(GamepadButton::DPadDown)) {
        reel -= 1.0;
    }
    if fine {
        reel *= 0.35;
    }

    if let Some(said) = gear.step(dt, sea, Hands { reel, drag }) {
        info!("{said}");
    }

    // A line on the terminal every second while the dev switch is set, so the gear can
    // be told apart from the drawing of it. `world::report_vitals` does the same thing
    // for the boat and for the same reason: when something looks wrong, the first
    // question is always whether it *is* wrong or merely invisible, and that question
    // should cost nothing to answer.
    *since_report += time.delta_secs();
    if gear.out() && *since_report >= 1.0 && std::env::var("SAIL_ROD").is_ok() {
        *since_report = 0.0;
        info!(
            "rod: line {:.1} m, lure {:.1} m down over {:.0} m, load {:.0} N, \
             drag {:.0} N, reel {reel:+.1}, {}",
            gear.line_out,
            lure_depth(gear.line_out, sea.stw),
            sea.sounding,
            gear.tension,
            gear.drag_force(),
            match gear.fish() {
                Some(fish) => fish.doing.name(),
                None => "nothing on",
            }
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Flat water, where the lift is free: there is no sea to time and you simply
    /// reach over and pick the fish up.
    fn sea(knots: f64, sounding: f64) -> Sea {
        Sea {
            stw: sail_core::units::kn_to_ms(knots),
            sounding,
            sun: 0.1,
            lift: 0.0,
            swell: 0.0,
        }
    }

    /// A metre of sea with the boat at a given point in it, so the timing can be tested.
    fn seaway(knots: f64, lift: f64) -> Sea {
        Sea {
            stw: sail_core::units::kn_to_ms(knots),
            sounding: 30.0,
            sun: 0.1,
            lift,
            swell: 1.0,
        }
    }

    /// Run the gear for a while with a fixed hand on it.
    fn sail(f: &mut Fishing, seconds: f64, sea: Sea, hands: Hands) -> Vec<String> {
        let mut said = Vec::new();
        let mut t = 0.0;
        while t < seconds {
            if let Some(word) = f.step(1.0 / 60.0, sea, hands) {
                said.push(word);
            }
            t += 1.0 / 60.0;
        }
        said
    }

    /// Fish it properly: work the gear, and lift when the sea offers the moment. What a
    /// player does, as against `sail`, which only holds the reel.
    fn fish_it(f: &mut Fishing, seconds: f64, sea: Sea, hands: Hands) -> Vec<String> {
        let mut said = Vec::new();
        let mut t = 0.0;
        while t < seconds {
            if let Some(word) = f.step(1.0 / 60.0, sea, hands) {
                said.push(word);
            }
            if f.ready_to_lift(sea) == Some(true) {
                if let Some(word) = f.lift_aboard(sea) {
                    said.push(word);
                }
            }
            t += 1.0 / 60.0;
        }
        said
    }

    /// Put a known fish on the line, so a fight can be tested without waiting for one.
    fn with(what: Quarry, mass: f64) -> Fishing {
        let mut f = rig(7);
        f.line_out = 30.0;
        let n = what.nature();
        f.rod = Rod::Playing(Hooked {
            what,
            mass,
            depth: 10.0,
            across: 0.0,
            sway: 1.0,
            spent: 0.0,
            hold: n.hold,
            doing: n.opening,
            until: 5.0,
            seen: false,
            fought: 0.0,
            alongside: 0.0,
            bolted: false,
        });
        f
    }

    /// The environment must not reach the tests. `Fishing::new` reads `SAIL_FISH` and
    /// `SAIL_ROD`, and a suite whose answers depend on a shell variable is a suite that
    /// passes on one machine.
    fn rig(seed: u64) -> Fishing {
        let mut f = Fishing::new(seed);
        f.plenty = 1.0;
        f.rod = Rod::Stowed;
        f.line_out = 0.0;
        f
    }

    fn hands(reel: f64, drag: f64) -> Hands {
        Hands { reel, drag }
    }

    /// More line is more depth, and it keeps being more depth. The triangle version of
    /// this function saturated at fifteen metres and no amount of line got past it,
    /// which is the failure this test exists to catch — so it asks for the depth to keep
    /// climbing a long way out, not merely to climb at all.
    /// The down arrow lets line off the spool and the up arrow gets it back. The
    /// plainest thing the gear does, and therefore the one nothing was watching.
    #[test]
    fn the_reel_gives_line_and_takes_it_back() {
        let mut f = rig(5);
        f.stow_or_set();
        let streamed = f.line_out;
        sail(&mut f, 4.0, sea(3.0, 40.0), hands(-1.0, 1.0 / 3.0));
        let given = f.line_out;
        assert!(
            given > streamed + 5.0,
            "four seconds of free spool gave {given} from {streamed}"
        );
        sail(&mut f, 4.0, sea(3.0, 40.0), hands(1.0, 1.0 / 3.0));
        assert!(f.line_out < given - 1.0, "wound back to {}", f.line_out);
    }

    /// More line is more depth. The triangle version of this function saturated and no
    /// amount of line got past fifteen metres, which is the failure this catches.
    #[test]
    fn the_lure_goes_deeper_the_more_line_is_out() {
        let shallow = lure_depth(20.0, 2.0);
        let deep = lure_depth(60.0, 2.0);
        assert!(deep > shallow * 1.4, "{deep} against {shallow}");
    }

    /// The claim the whole depth control rests on, and it belongs to the boat: a plain
    /// lure's downforce is its own weight and the line's drag goes as the square of the
    /// speed, so slowing down sinks it hard and driving her plane it up to the surface.
    ///
    /// This is the opposite of what the file used to assert. The old answer held its
    /// depth against speed, which is true of a deep-diving plug on a downrigger and not
    /// of anything a small boat tows — and it put the lure fifteen metres down at every
    /// speed, where nobody could see it do anything.
    #[test]
    fn speed_is_the_depth_control() {
        let fast = lure_depth(30.0, 2.6);
        let slow = lure_depth(30.0, 0.8);
        assert!(fast < 4.0, "five knots should skip it along: {fast} m");
        assert!(slow > fast * 4.0, "{slow} against {fast}");
    }

    /// Which is what puts the deep fish within reach: a long line behind a boat barely
    /// moving gets down to the ground a ling is on, and nothing else does.
    #[test]
    fn a_long_line_on_a_slow_boat_reaches_the_bottom_fish() {
        let deep = lure_depth(100.0, 0.77);
        assert!(deep > 25.0, "{deep} m");
    }

    /// Stopped, the lure hangs nearly straight down under the rod tip, which is how one
    /// finds the bottom at the end of a leg.
    #[test]
    fn a_lure_hangs_when_the_boat_stops() {
        assert!(lure_depth(30.0, 0.1) > 29.0);
    }

    /// A drag set to a third of the breaking strain lands a saithe.
    #[test]
    fn a_properly_set_drag_beats_a_saithe() {
        let mut f = with(Quarry::Saithe, 2.5);
        let said = fish_it(&mut f, 400.0, sea(1.5, 25.0), hands(1.0, 1.0 / 3.0));
        assert_eq!(f.caught.len(), 1, "{said:?}");
        assert_eq!(f.caught[0].what, Quarry::Saithe);
        // And it did not come quietly.
        assert!(said.iter().any(|s| s.contains("seen the boat")), "{said:?}");
    }

    /// And a drag screwed down hard loses the big one.
    ///
    /// On a ling, because the size of the fish is the whole point and getting this wrong
    /// once already cost an afternoon. A locked drag on a four kilogramme pollack is
    /// *fine* — the fish cannot pull eighty-nine newtons, so nothing parts and the hook
    /// mostly holds — and the first version of this test asserted otherwise and failed,
    /// correctly. A tight drag is not a mistake you can make on a small fish. It is a
    /// mistake you make once on a big one, and then never again.
    #[test]
    fn a_locked_drag_loses_the_big_one() {
        let mut f = with(Quarry::Ling, 9.0);
        let said = sail(&mut f, 120.0, sea(1.5, 60.0), hands(1.0, 1.0));
        assert!(f.caught.is_empty(), "{said:?}");
        assert_eq!(f.lost, 1, "{said:?}");
    }

    /// The same locked drag against something small is survivable, which is the other
    /// half of the claim and the reason mackerel are what everybody catches first.
    #[test]
    fn a_locked_drag_is_no_trouble_on_a_mackerel() {
        let mut f = with(Quarry::Mackerel, 0.45);
        let said = fish_it(&mut f, 180.0, sea(1.5, 25.0), hands(1.0, 1.0));
        assert_eq!(f.caught.len(), 1, "{said:?}");
    }

    /// The other half of the same lesson, and the one nobody expects: a drag backed off
    /// to nothing never tires the fish, so it is never landed either. Measured against
    /// the same fish on a proper drag, because the absolute number means nothing.
    #[test]
    fn no_drag_never_tires_the_fish() {
        let tired = |drag: f64| {
            let mut f = with(Quarry::Pollack, 4.0);
            sail(&mut f, 120.0, sea(1.0, 30.0), hands(0.0, drag));
            f.fish().map(|f| f.beaten()).unwrap_or(1.0)
        };
        let slack = tired(0.02);
        let proper = tired(1.0 / 3.0);
        assert!(
            proper > slack * 3.0,
            "a third of the breaking strain tires it {proper}, nothing tires it {slack}"
        );
    }

    /// Hooking something substantial at six knots and doing nothing about it ends the way
    /// it ends on the water. This is the mechanic that makes the boat part of the tackle.
    #[test]
    fn a_big_fish_at_six_knots_is_lost() {
        let mut f = with(Quarry::Ling, 9.0);
        let said = sail(&mut f, 90.0, sea(6.0, 70.0), hands(0.0, 1.0 / 3.0));
        assert!(f.caught.is_empty(), "{said:?}");
        assert_eq!(f.lost, 1, "{said:?}");
    }

    /// And the same fish, with the way taken off her, is a fish you can fight.
    #[test]
    fn the_same_fish_slowed_down_is_still_on() {
        let mut f = with(Quarry::Ling, 9.0);
        let said = sail(&mut f, 60.0, sea(1.0, 70.0), hands(0.6, 1.0 / 3.0));
        assert!(f.fish().is_some() || f.caught.len() == 1, "{said:?}");
    }

    /// Pound for pound the mackerel out-pulls everything else here, and the table has to
    /// say so or the species are five names on one fish.
    #[test]
    fn a_mackerel_pulls_harder_for_its_size_than_a_cod() {
        let mackerel = with(Quarry::Mackerel, 0.5);
        let cod = with(Quarry::Cod, 0.5);
        let m = mackerel.fish().unwrap().strength();
        let c = cod.fish().unwrap().strength();
        assert!(m > c * 2.0, "{m} against {c}");
    }

    /// Trolling in the right water, for long enough, catches something. The rate is five
    /// small numbers multiplied together and it is entirely possible to get them wrong by
    /// a factor of a hundred without noticing.
    #[test]
    fn an_hour_over_good_ground_catches_something() {
        let mut f = rig(11);
        f.stow_or_set();
        f.line_out = 14.0;
        let said = sail(&mut f, 3600.0, sea(3.6, 20.0), hands(0.0, 1.0 / 3.0));
        assert!(
            said.iter().any(|s| s.contains("something has it")),
            "an hour of trolling and not a touch: {said:?}"
        );
    }

    /// And trolling at a speed at which no lure swims catches nothing, which is the same
    /// claim from the other side.
    #[test]
    fn nothing_takes_a_lure_towed_at_twelve_knots() {
        let mut f = rig(11);
        f.stow_or_set();
        let said = sail(&mut f, 3600.0, sea(12.0, 30.0), hands(0.0, 1.0));
        assert!(
            !said.iter().any(|s| s.contains("something has it")),
            "{said:?}"
        );
    }

    /// A hundred and sixty metres of line towed at six knots parts it with nothing on
    /// the hook at all, and the gear has to notice. It did not: breaking was only ever
    /// checked during a fight, so a hundred newtons sat on an eighty-nine newton line
    /// indefinitely while the spool emptied. Reported from the deck with a photograph of
    /// the gauge saying exactly that.
    #[test]
    fn too_much_line_at_speed_parts_it_with_nothing_on() {
        let mut f = rig(9);
        f.stow_or_set();
        f.line_out = 150.0;
        let said = sail(&mut f, 12.0, sea(6.3, 60.0), hands(0.0, 1.0));
        assert!(said.iter().any(|s| s.contains("parted")), "{said:?}");
        assert!(!f.out(), "the gear should have come in with the lure");
    }

    /// And with the drag left light it goes the other way: the line streams out until
    /// there is none, and then the knot gives.
    #[test]
    fn a_light_drag_at_speed_empties_the_spool() {
        let mut f = rig(9);
        f.stow_or_set();
        f.line_out = 120.0;
        let said = sail(&mut f, 60.0, sea(5.0, 60.0), hands(0.0, 0.02));
        assert!(said.iter().any(|s| s.contains("spool")), "{said:?}");
    }

    /// The lure is never below the ground. The readout worked the depth out for itself
    /// from the raw geometry and reported it eight metres inside the seabed.
    #[test]
    fn the_lure_is_never_deeper_than_the_water() {
        let mut f = rig(4);
        f.stow_or_set();
        f.line_out = 120.0;
        // Barely moving over shoal ground, which is where the geometry wants to put the
        // lure thirty metres down in twelve metres of water.
        sail(&mut f, 3.0, sea(0.8, 12.0), hands(0.0, 1.0 / 3.0));
        assert!(f.depth <= 12.0, "lure at {} m over 12 m", f.depth);
        assert!(lure_depth(120.0, sail_core::units::kn_to_ms(0.8)) > 12.0);
    }

    /// Dragging the lure along the bottom finds the bottom.
    #[test]
    fn trolling_too_deep_finds_the_ground() {
        let mut f = rig(3);
        f.stow_or_set();
        f.line_out = 60.0;
        let said = sail(&mut f, 600.0, sea(2.0, 12.0), hands(0.0, 1.0 / 3.0));
        assert!(said.iter().any(|s| s.contains("ground")), "{said:?}");
    }

    /// A snagged lure comes back when the way is off her, and not before.
    #[test]
    fn a_snag_frees_when_the_boat_stops() {
        let mut f = with(Quarry::Bottom, 6.0);
        let stuck = sail(&mut f, 20.0, sea(4.0, 12.0), hands(0.0, 1.0 / 3.0));
        assert!(
            !stuck.iter().any(|s| s.contains("came away")),
            "it should not free itself at four knots: {stuck:?}"
        );
        let freed = sail(&mut f, 60.0, sea(0.4, 12.0), hands(1.0, 1.0 / 3.0));
        assert!(freed.iter().any(|s| s.contains("came away")), "{freed:?}");
    }

    /// Every fish makes one more run at the sight of the hull, however beaten it is.
    /// There is no fish that comes quietly, and a model in which a tired one simply
    /// arrived was missing the most famous ten seconds in angling.
    #[test]
    fn nothing_comes_quietly() {
        for what in [Quarry::Mackerel, Quarry::Saithe, Quarry::Cod] {
            let mut f = with(what, 2.0);
            let said = fish_it(&mut f, 400.0, sea(1.0, 25.0), hands(1.0, 1.0 / 3.0));
            assert!(
                said.iter().any(|s| s.contains("seen the boat")),
                "{what:?}: {said:?}"
            );
        }
    }

    /// In a seaway the lift has to be timed to the rise, and lifting as she drops away
    /// costs you the fish's place alongside and a piece of the hook hold.
    #[test]
    fn lifting_as_she_drops_loses_the_moment() {
        let mut f = with(Quarry::Saithe, 2.0);
        // Beaten and alongside. Run until it gets there rather than for a fixed time:
        // a fish will not lie beside the boat for ever — see `PATIENCE` — so "alongside"
        // is a window that opens and shuts, and a test that sampled it at four hundred
        // seconds would be sampling the phase of a cycle.
        let water = sea(1.0, 25.0);
        let mut t = 0.0;
        while t < 400.0 && f.fish().is_some_and(|fish| fish.alongside <= 0.0) {
            f.step(1.0 / 60.0, water, hands(1.0, 1.0 / 3.0));
            t += 1.0 / 60.0;
        }
        assert!(
            f.fish().is_some_and(|fish| fish.alongside > 0.0),
            "it never got alongside"
        );
        let hold = f.fish().unwrap().hold;

        // Now a sea, with her dropping away under it.
        let dropping = seaway(1.0, -0.4);
        assert_eq!(f.ready_to_lift(dropping), Some(false));
        let said = f.lift_aboard(dropping).expect("it should say so");
        assert!(said.contains("dropped away"), "{said}");
        assert!(f.caught.is_empty());
        assert!(f.fish().unwrap().hold < hold - 0.1);

        // And on the rise it comes aboard.
        let rising = seaway(1.0, 0.5);
        sail(&mut f, 60.0, rising, hands(1.0, 1.0 / 3.0));
        assert_eq!(f.ready_to_lift(rising), Some(true));
        f.lift_aboard(rising);
        assert_eq!(f.caught.len(), 1, "it should have come aboard on the rise");
    }

    /// And in flat water there is nothing to time: you lean over and pick it up.
    #[test]
    fn flat_water_asks_for_no_timing() {
        assert!(on_the_rise(0.0, 0.0));
        assert!(!on_the_rise(0.0, 1.2));
        assert!(on_the_rise(0.6, 1.2));
    }

    /// The gear cannot be stowed with a fish on it, and saying so is better than doing
    /// nothing silently.
    #[test]
    fn you_cannot_stow_a_bent_rod() {
        let mut f = with(Quarry::Cod, 5.0);
        let said = f.stow_or_set();
        assert!(f.fish().is_some());
        assert!(said.contains("fish on"));
    }

    /// The fight integrates at a fixed step, so the frame rate must not change the
    /// outcome. Thirty frames a second and a hundred and twenty land the same fish.
    #[test]
    fn the_fight_does_not_depend_on_the_frame_rate() {
        let run = |frame: f64| {
            let mut f = with(Quarry::Saithe, 2.5);
            let mut t = 0.0;
            while t < 400.0 {
                let water = sea(1.5, 25.0);
                f.step(frame, water, hands(1.0, 1.0 / 3.0));
                if f.ready_to_lift(water) == Some(true) {
                    f.lift_aboard(water);
                }
                t += frame;
            }
            f.caught.len()
        };
        assert_eq!(run(1.0 / 30.0), 1);
        assert_eq!(run(1.0 / 120.0), 1);
    }
}
