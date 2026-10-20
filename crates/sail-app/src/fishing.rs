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

/// The rod worked: the drag, the line, the fish on the end of it.
///
/// What this module does in a frame, in one place — see `stages.rs` for why the
/// schedule is named sets rather than one long chain.
pub fn plugin(app: &mut App) {
    use crate::stages::Deck;
    app.add_systems(Update, work.in_set(Deck::Intent));
}

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
/// A multiplier with a full spool gains the best part of a metre a turn and two turns a
/// second is a brisk wind. It stood at 0.80, which is the figure off an *empty* spool and
/// felt, from the deck, like winding treacle.
const RETRIEVE: f64 = 1.15;

/// Tension at which you cannot turn the handle at all, newtons.
///
/// Not the reel's limit — yours. Work it out at the handle: a five-to-one multiplier
/// turns the spool five times per turn of the handle, so the force your hand needs is the
/// line's pull times the spool radius times the ratio, over the handle's own radius —
/// `25 mm × 5 / 50 mm`, which is **two and a half times the line tension**. A person can
/// put a hundred newtons on a reel handle for a few seconds, so winding stops at about
/// forty on the line.
///
/// This has now been wrong in both directions and the second was worse than the first.
/// Seventy was twenty-five kilogrammes at the handle and not a thing a person does, and
/// with a reel that strong there was no load a rod was better at than a handle — which is
/// why pumping gained nothing, and is how the error was found at all. Twenty-four was the
/// same arithmetic with a six-to-one ratio and an empty spool, and it overshot: through
/// the middle of an ordinary fight the handle was simply dead, and the arrow key was
/// reported from the deck as having stopped working. It had not. It was telling the truth
/// about a reel nobody would buy.
///
/// Forty leaves it working and slow, which is the honest answer and the one that leaves
/// the gearbox and the rod a reason to exist rather than making them compulsory.
const STALL: f64 = 40.0;

/// And the same two numbers in low gear.
///
/// A two-speed multiplier exists purely because of the problem above. Drop it into low
/// and the handle turns against a load that stopped it dead a moment ago: the gearing
/// trades the line you gain per turn for the force you can turn against, which is the
/// whole of what a gearbox is for and is why every serious boat reel has two of them.
///
/// Half the speed for twice the pull, which is exactly what the real ratios give: three
/// to one against six to one, and the hand force falls in proportion. The trade is not
/// free and is not meant to be — winding against a heavy fish *puts that load on*, so
/// low gear gains you line and wears the hook at the same time.
const LOW_RETRIEVE: f64 = 0.48;
const LOW_STALL: f64 = 75.0;

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

/// How much line the spool gives up in the first instant of a take, metres a second,
/// and how quickly that dies away.
///
/// **Every fish big enough to take this lure rips line off the spool when it does.** The
/// first attempt at this put a shock *load* on the line, and it was wrong by measurement:
/// thirty newtons of drag against half a kilogramme of mackerel hauls the fish bodily in,
/// so the spring dumped in a twentieth of a second and the whole event came to five
/// centimetres of line. The model was telling the truth and the mechanism was wrong.
///
/// The right one is that the drum is **standing still** and has to be got turning. Three
/// small real things arrive in the same tenth of a second — the spool's own inertia, the
/// fish turning away with the lure in its mouth, and the boat sailing on from where the
/// fish now is — and not one of them alone would be worth modelling. Together they are
/// about a third of a metre of line and ten teeth of ratchet, which is what a take
/// sounds like.
///
/// It is line *released*, not load applied, so it cannot part anything. The one thing
/// that must not happen at a take is losing the fish before you have touched a control.
const TAKE_SURGE: f64 = 0.8;
const TAKE_SURGE_SPEED: f64 = 0.35;
const TAKE_SURGE_DIES: f64 = 0.35;

/// What a take puts on the line over and above the drag setting, newtons.
///
/// **Every fish big enough to take this lure rips line off the spool when it does**, and
/// that is not a courtesy to the player — it is what the event physically is. A take
/// while trolling is a collision: the fish turns on a lure going the other way at three
/// knots, and the whole of that closing speed has to be absorbed by a spool that is
/// standing still. The drag cannot spin up inside the few milliseconds it takes, so the
/// line goes over the setting whatever the setting is, and the ratchet squawks.
///
/// Without it a mackerel at eleven newtons against a thirty newton drag simply arrived,
/// and the most exciting moment in the activity was a line in the log.
///
/// It is bounded well under the breaking strain, because a take that broke you off
/// before you had touched anything would be an event with no decision in it. The one
/// setting where nothing is ripped off at all is a drag screwed down past what the fish
/// can pull — which is correct, and is the same lesson the dial teaches everywhere else.
const TAKE_SHOCK: f64 = 7.0;

/// How far the rod tip travels through one pump, metres, and how fast it goes each way.
///
/// **Pumping is the genre's signature mechanic and this file argued against having it**,
/// on the grounds that the gearbox already answers the question it answers. That was
/// wrong in an interesting way: the gearbox answers *how do I gain line against a load*
/// and pumping answers *how do I gain line without the reel at all*, which is a different
/// question with a different failure mode, and it is the one every angler is taught
/// first.
///
/// What it costs to add is one term, because the model was already a spring. Lifting the
/// rod moves the tip away from the fish, which is exactly the same thing as the fish
/// swimming away: the spring takes it up, the tension rises, and the fish comes. Dropping
/// the rod gives that distance back — and if you are not winding as it drops, what you
/// have made is **slack**, which on a head-shake throws the hook. That is the whole of
/// why "wind down" is the second half of the instruction, and none of it had to be
/// written: it was already in `d(stretch)/dt`.
///
/// Down slower than up, and slower than a reel in high gear can take line, so a pump
/// wound down properly loses nothing and a pump dropped on a slack reel loses the lot.
const SWEEP: f64 = 1.1;
const LIFT_UP: f64 = 0.42;
const LIFT_DOWN: f64 = 0.60;

/// Line left when the fish is at the rod tip and the fight is over, metres.
const LEADER: f64 = 1.6;

/// What the rod holder will take before it lets go, and for how long.
///
/// **The one loss in this file you do not get back**, and the only one that is a mistake
/// rather than a misfortune.
///
/// A rod in a holder is held by a tube and its own weight, and what fails is not a force
/// but a *moment*: two metres of lever with the load at the far end of it, which a tube
/// and a bolt will take for a while and not for ever.
///
/// So it happens in exactly one situation, and it is the situation the drag dial exists
/// to teach: **a screwed-down drag, a fish big enough to use it, and nobody holding the
/// rod.** A drag set anywhere near right never gets there, because a slipping drag is a
/// load that cannot build. And holding the rod — `U`, which is a pair of hands on it and
/// not merely a lift — stops it outright.
/// Measured against the only window it can happen in, which is narrower than it looks
/// and is set by the stiction rather than by the holder. A drag only slips once the load
/// passes it by a third, so a setting above about three quarters of the breaking strain
/// never gives line at all — it holds until the *line* goes. The band where a drag is
/// both slipping and loading the rod hard is therefore everything between the rule of
/// thumb and there, and the holder has to give somewhere inside it or it never gives at
/// all. Sixty-two hundredths puts it comfortably above a drag set to the rule — which
/// settles around fifty newtons on the worst fish here — and comfortably below the
/// setting that parts the line instead.
const HOLDER: f64 = 0.62 * BREAKING;
const HOLDER_HOLDS: f64 = 1.6;

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
    /// The shark. See [`TOPE`].
    Tope,
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
    /// Whether it can bite through what it is hooked on, per second, with the load up.
    ///
    /// The one thing that makes a shark a different animal rather than a bigger fish.
    /// Everything else in this table is hooked in a mouth made of bone and skin and
    /// stays hooked until the hook pulls; a tope has a mouth full of teeth, and a leader
    /// of monofilament in it is a matter of time. Serious shark anglers use a wire trace
    /// for exactly this reason and nobody trolling a lure off a cruising boat is carrying
    /// one.
    ///
    /// Nought for everything that is not a shark, which is everything else here.
    bite_through: f64,
    /// How often a fish that hits the lure ends up on it.
    ///
    /// The miss is half of trolling and it was missing. A fish slashes at a moving lure
    /// and takes it properly about three times in four — a cod's mouth is a bucket and
    /// closes over the whole thing, a mackerel is small and fast and often just knocks
    /// it. What the knock gives the player is the thing a bite alarm gives an angler:
    /// *you are in the right place*. Nothing happened, and that is information, and it
    /// is the reason anybody keeps trolling the same patch of water.
    sticks: f64,
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
    bite_through: 0.0,
    sticks: 0.70,
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
    bite_through: 0.0,
    sticks: 0.80,
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
    bite_through: 0.0,
    sticks: 0.72,
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
    bite_through: 0.0,
    sticks: 0.86,
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
    // Slower than it looked, and the page in `logbook.rs` is what found it: at one and
    // eight tenths of a knot the lure cannot be got to twenty-eight metres on the whole
    // spool — a hundred and forty metres of line gets to twenty-four. So the band was
    // asking for a depth that could not be reached at the speed it asked for, which is a
    // fish nobody could have caught on purpose. One and four tenths is where the
    // arithmetic says it has to be, and it is the right answer anyway: a ling is a
    // bottom fish and you are barely moving.
    speed: (1.4, 0.6),
    ground: (60.0, 30.0),
    plenty: 0.15,
    bite_through: 0.0,
    sticks: 0.80,
    opening: Doing::Sound,
    repertoire: &[Doing::Hang, Doing::Sound, Doing::Run, Doing::Hang],
};

/// A tope: the shark that is actually here.
///
/// Asked for as "sharks", and this is the one these waters have. A tope is a two-metre
/// requiem shark that comes into the Irish and Scottish coast in spring and summer, takes
/// a bait or a lure on the move, and makes the first run anybody who has caught one talks
/// about afterwards — a hundred metres of line in the time it takes to get your hand to
/// the reel. It is caught on purpose here every season and released again.
///
/// Three things make it a different animal in this table rather than a heavier fish.
///
/// **It cannot be stopped.** A hundred and fifteen newtons against braid that parts at
/// eighty-nine means the drag has to be giving line or the line is going. You do not
/// fight a tope, you let it run and you keep in touch with it.
///
/// **It does not tire in a hurry.** The stamina is the highest in the list by half
/// again, which with a hundred and sixty metres on the spool is the real question: it is
/// a race between the fish tiring and the spool emptying, and on a big one the spool
/// wins.
///
/// **It has teeth.** See [`Nature::bite_through`]: without a wire trace, a leader in that
/// mouth is a matter of time, and that is a way of losing a fish that nothing else here
/// can do to you.
const TOPE: Nature = Nature {
    name: "tope",
    mass: (9.0, 3.5),
    power: 1.3,
    dash: 3.4,
    stamina: 150.0,
    hold: 0.78,
    // Mid-water over a clean bottom, and it will come up for something moving.
    depth: (12.0, 7.0),
    speed: (2.6, 1.2),
    ground: (40.0, 25.0),
    // Rare, and meant to be. One in a long afternoon, and you will remember which one.
    plenty: 0.09,
    bite_through: 0.016,
    sticks: 0.74,
    opening: Doing::Run,
    repertoire: &[Doing::Run, Doing::Run, Doing::Sound, Doing::Hang],
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
    bite_through: 0.0,
    sticks: 1.0,
    opening: Doing::Hang,
    repertoire: &[Doing::Hang],
};

impl Quarry {
    /// By name, for the dev switch. Anything unrecognised is nothing rather than a
    /// default fish, because silently hooking a mackerel when you asked for a ling is
    /// a worse afternoon than being told you spelled it wrong.
    pub fn named(name: &str) -> Option<Self> {
        SPECIES.iter().copied().find(|q| q.name() == name)
    }

    fn nature(self) -> &'static Nature {
        match self {
            Quarry::Mackerel => &MACKEREL,
            Quarry::Saithe => &SAITHE,
            Quarry::Pollack => &POLLACK,
            Quarry::Cod => &COD,
            Quarry::Ling => &LING,
            Quarry::Tope => &TOPE,
            Quarry::Bottom => &GROUND,
        }
    }

    pub fn name(self) -> &'static str {
        self.nature().name
    }

    /// Where it feeds, how fast it will chase a lure, and the water it is over —
    /// depth in metres, speed in knots, ground in metres.
    ///
    /// The three numbers the page in `logbook.rs` is built from. Read off the same table
    /// the fish themselves are drawn from, so a species whose numbers are tuned cannot
    /// end up with a page that lies about it.
    pub fn hunting(self) -> (f64, f64, f64) {
        let n = self.nature();
        (n.depth.0, n.speed.0, n.ground.0)
    }

    /// What it does when it feels the hook, in the words somebody would use.
    ///
    /// Written out rather than derived from the repertoire, because "Sound, Shake, Hang,
    /// Sound" is a list of states and what a person wants is the character. The states
    /// are the truth and this is the description of it; if they ever disagree, the states
    /// are right and this line is out of date.
    pub fn fights(self) -> &'static str {
        match self {
            Quarry::Mackerel => "tight circles, over in twenty seconds, paper mouth",
            Quarry::Saithe => "runs, and keeps running — the best fight per pound",
            Quarry::Pollack => "one savage dive at the hook; survive it and it is yours",
            Quarry::Cod => "dead weight and a head-shake, no runs at all",
            Quarry::Ling => "give it line or it takes the lot",
            Quarry::Tope => "a hundred metres before your hand reaches the reel — and teeth",
            Quarry::Bottom => "it does not fight, it waits",
        }
    }
}

/// Every species that swims, in the order they are rolled for. The ground is not in the
/// list because it is not caught, it is run into.
pub const SPECIES: [Quarry; 6] = [
    Quarry::Mackerel,
    Quarry::Saithe,
    Quarry::Pollack,
    Quarry::Cod,
    Quarry::Ling,
    Quarry::Tope,
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

/// What the weather is doing to the bite, as a multiplier.
///
/// Rising weather shuts them down and a front coming on wakes them up. It is the oldest
/// belief in angling and one of the few that holds up, and here it costs nothing because
/// the boat already carries the reading — the OUTLOOK line on the console, which until
/// now only told you when to reef. Half again as many takes with a gale on the way, a
/// third fewer once it is settling.
///
/// The useful consequence is that the fishing and the sailing now read the same
/// instrument, and read it in opposite directions: the hour you ought to be shortening
/// down is the hour they are feeding.
pub fn bite_weather(glass: f64) -> f64 {
    // Falling is the good one, so the sign turns over here and nowhere else. Six
    // hectopascals in three hours is a proper fall — a front, and the hour to be out in.
    (1.0 + 0.5 * (-glass / 6.0).clamp(-0.7, 1.0)).max(0.25)
}

/// Where the handle stops, in the gear asked for. For the console, which has to be able
/// to say *why* the arrow key has gone quiet.
pub fn stall(low_gear: bool) -> f64 {
    if low_gear {
        LOW_STALL
    } else {
        STALL
    }
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
    /// What the barometer has done over the last three hours, hectopascals. Negative is
    /// falling.
    ///
    /// The real instrument rather than the wind trend it is derived from. This read
    /// `outlook().0` until somebody pointed at the console and showed there was a BARO
    /// line on it — and the tendency is what an angler actually reads, because the number
    /// on the dial says nothing and the three-hour change says everything.
    pub glass: f64,
}

/// What the hands are doing this frame.
#[derive(Clone, Copy, Default, Debug)]
pub struct Hands {
    /// Positive winds in, negative gives line.
    pub reel: f64,
    /// Where the drag star is set, nought to one of the line's breaking strain.
    pub drag: f64,
    /// Whether the reel is in low gear.
    pub low_gear: bool,
    /// Whether the rod is being lifted.
    pub pump: bool,
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
    /// Line still coming off the spool from the shock of the take, metres a second.
    surge: f64,
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
    /// Over the side. See [`HOLDER`].
    ///
    /// A boat carries one rod. There is another in the chandlery at the next harbour and
    /// nowhere nearer, which is the whole weight of this state: everything else in this
    /// file costs you a fish, and this costs you the afternoon.
    Gone,
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
    /// How far through a pump the rod is, nought low and one high. Public because the
    /// drawing has to show it: a pump you cannot see is a key with a number behind it.
    pub lift: f64,
    /// How fast line is leaving the spool against the drag, m/s. Drives the ratchet.
    pub paying: f64,
    /// What is in the box.
    pub caught: Vec<Landed>,
    /// And how many got away, which is the more interesting number.
    pub lost: u32,
    rng: Pcg32,
    /// Seconds since the last take, so one fish does not arrive on top of another.
    quiet: f64,
    /// A fish to hook on demand, from `SAIL_HOOK`.
    ///
    /// Waiting for a take is the right way to test the *trolling* and a useless way to
    /// test the *fight*: the interesting fish are the rare ones, and the interesting
    /// moments — a drag that will not hold a ling, low gear against a load that stops the
    /// handle, a lift mistimed on the drop of a wave — all need one on the line now and
    /// another one after it. So `SAIL_HOOK=ling` puts one on as soon as the lure is
    /// fishing and another whenever the line comes free.
    ///
    /// Same family as `SAIL_SQUALL` and `SAIL_FISH`, and the same reasoning: you cannot
    /// get a thing right by looking at it once every few minutes.
    forced: Option<Quarry>,
    /// Seconds the holder has been taking more than it can take.
    straining: f64,
    /// Seconds since the rod went over the side, for the screen that says so.
    pub gone_for: f64,
    /// A knock on the rod that has not died away yet, newtons.
    ///
    /// Something hit the lure and did not stay on. It is a real force for a real reason —
    /// a fish's whole weight arriving on a tight line and leaving again — so it belongs
    /// in the tension rather than in a message, and what the player gets is the rod
    /// nodding and the ratchet giving half a turn.
    knock: f64,
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
        let plenty = crate::knob::var("SAIL_FISH")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(1.0)
            .clamp(0.0, 500.0);
        let streamed = crate::knob::var("SAIL_ROD")
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
            lift: 0.0,
            depth: 0.0,
            stretch: 0.0,
            paying: 0.0,
            caught: Vec::new(),
            lost: 0,
            rng: Pcg32::new(seed ^ 0xF15E),
            quiet: 0.0,
            straining: 0.0,
            gone_for: 0.0,
            forced: crate::knob::var("SAIL_HOOK").ok().and_then(|name| {
                let found = Quarry::named(name.trim());
                if found.is_none() {
                    println!(
                        "sail: SAIL_HOOK={name} is not a fish — try {}",
                        SPECIES
                            .iter()
                            .map(|q| q.name())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                found
            }),
            knock: 0.0,
            shoal: 0.0,
            plenty,
            accumulator: 0.0,
        }
    }

    pub fn out(&self) -> bool {
        matches!(self.rod, Rod::Trolling | Rod::Playing(_))
    }

    /// Whether there is a rod aboard at all.
    pub fn have_a_rod(&self) -> bool {
        !matches!(self.rod, Rod::Gone)
    }

    /// A new one out of the chandlery, which is a thing that happens in a harbour and
    /// nowhere else. Called when a consignment is delivered — see `main.rs`.
    pub fn ashore(&mut self) {
        if matches!(self.rod, Rod::Gone) {
            self.rod = Rod::Stowed;
            self.gone_for = 0.0;
            self.straining = 0.0;
        }
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
            Rod::Gone => "there is no rod — there will be one at the next harbour",
        }
    }

    /// How hard the fight is pressing the gear, nought to one.
    ///
    /// For the band. `music.rs` already has a measure of how far past her own numbers
    /// the *boat* is — cloth over its rated wind, knocked down, a sea too big for her —
    /// and when that passes a third it changes language entirely, into something with a
    /// flattened second in it. This is the same question asked of the tackle, and it goes
    /// into the same place.
    ///
    /// Which is the whole design: there is no fish theme and there is no sting. A
    /// mackerel does not reach it, because a mackerel is eleven newtons against braid
    /// that stands eighty-nine and nothing is in danger. A tope running reaches it
    /// outright, because it is, and so does a ling you are trying to stop. **What the
    /// music responds to is the line, not the fish** — so a big one played properly, on a
    /// drag set right and given its head, does not get the full treatment until you make
    /// a mistake, and that is exactly the right way round.
    ///
    /// Two measures, worst wins. The load against what the line will stand, reading as
    /// past it at fifty-five per cent; and line leaving the spool, which is the other way
    /// a fight gets away from you and the one you can hear from the cockpit.
    pub fn pressing(&self) -> f32 {
        if self.fish().is_none() {
            return 0.0;
        }
        let load = (self.tension / BREAKING - 0.55) / 0.35;
        let running = (self.paying - 0.8) / 2.0;
        load.max(running).clamp(0.0, 1.0) as f32
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
        if matches!(self.rod, Rod::Gone) {
            self.gone_for += dt;
            self.tension = 0.0;
            self.paying = 0.0;
            return None;
        }
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
        // The rod goes up while the key is held and comes down when it is let go. What
        // the fight sees is not the angle but the *distance the tip moved*, which is the
        // same quantity the fish's own swimming contributes and goes into the same
        // spring.
        let was = self.lift;
        self.lift = if hands.pump {
            (self.lift + LIFT_UP * dt).min(1.0)
        } else {
            (self.lift - LIFT_DOWN * dt).max(0.0)
        };
        let swept = (self.lift - was) * SWEEP;
        // What the hand is asking of the reel. Winding in is limited by the tension —
        // past the stall you cannot turn the handle at all — and giving line is not,
        // because a free spool does not care.
        let wanted = hands.reel;
        let (gain, stall) = if hands.low_gear {
            (LOW_RETRIEVE, LOW_STALL)
        } else {
            (RETRIEVE, STALL)
        };
        let reel = if wanted > 0.0 {
            wanted * gain * (1.0 - self.tension / stall).clamp(0.0, 1.0)
        } else {
            // Free spool does not care what gear the handle is in: it is not connected
            // to anything, which is the point of it.
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
            let tow = LINE_DRAG * self.line_out * sea.stw * sea.stw
                + TOW * 0.25 * sea.stw * sea.stw
                + self.knock;
            // A knock is a thing that happened, not a thing that is happening: it decays
            // in about a quarter of a second, which is how long a fish is on a lure it
            // did not take properly.
            self.knock *= (-dt / 0.25).exp();
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
        self.fight(dt, sea, cap, reel, swept, hands.pump)
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
        // The dev switch, before anything that could stop it: asked for a ling, you get
        // a ling, over any ground and at any speed.
        if let Some(what) = self.forced {
            return Some(self.hook(what, sea));
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

        let glass = bite_weather(sea.glass);

        for species in SPECIES {
            let n = species.nature();
            // One take in seventy seconds with everything right, which over a two-hour
            // passage is a fish box worth having and nowhere near a fish a minute.
            let mut rate = self.plenty * n.plenty / 70.0
                * liking(swimming, n.depth)
                * liking(knots, n.speed)
                * liking(sea.sounding, n.ground)
                * light
                * glass;
            if species == Quarry::Mackerel {
                // A shoal is not one fish. Having had one, the next few minutes are the
                // best fishing of the day, and that is the whole reason anybody keeps
                // trolling the same patch of water.
                rate *= 1.0 + 5.0 * (self.shoal / 180.0).min(1.0);
            }
            if self.rng.next_f64() < rate * dt {
                // It hit the lure. Whether it is *on* the lure is another question, and
                // three times in four is the answer.
                if self.rng.next_f64() < n.sticks {
                    return Some(self.hook(species, sea));
                }
                let n = species.nature();
                // What you feel is the fish's own weight arriving and leaving.
                self.knock = (n.mass.0 * G * n.power * 0.45).min(0.4 * BREAKING);
                // A shorter wait than a take earns, because a fish that missed is a fish
                // still there — which is the whole value of knowing it happened.
                self.quiet = 3.0;
                return Some("something knocked it and missed".into());
            }
        }
        None
    }

    /// Something is on.
    fn hook(&mut self, what: Quarry, sea: Sea) -> String {
        let n = what.nature();
        let mass = (n.mass.0 + n.mass.1 * self.rng.next_normal()).max(n.mass.0 * 0.35);
        let depth = lure_depth(self.line_out, sea.stw).min((sea.sounding - 0.5).max(1.0));
        // What the take puts on the line, worked out before the fish exists because the
        // spool's answer to it decides what the fish is born holding.
        let v2 = sea.stw * sea.stw;
        let tow = LINE_DRAG * self.line_out * v2 + TOW * mass.powf(2.0 / 3.0) * v2;
        let want = (self.drag * BREAKING * STICTION + TAKE_SHOCK).min(0.72 * BREAKING);
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
            // Nothing comes off a drum the take cannot start turning. A drag screwed
            // down past the load a take generates holds it dead, which is the same
            // lesson the dial teaches everywhere else rather than an exception to it.
            surge: if want > self.drag * BREAKING {
                TAKE_SURGE + TAKE_SURGE_SPEED * sea.stw
            } else {
                0.0
            },
        });
        self.quiet = 0.0;
        self.knock = 0.0;
        self.stretch = ((want - tow) / ROD).max(0.0);
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
    #[allow(clippy::too_many_arguments)]
    fn fight(
        &mut self,
        dt: f64,
        sea: Sea,
        cap: f64,
        reel: f64,
        swept: f64,
        hands_on: bool,
    ) -> Option<String> {
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
            // How hard you are leaning on it, as a fraction of what it has left.
            let pressed = self.tension > 0.6 * fish.strength();
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
                } else if pressed && beaten < 0.55 && matches!(want, Doing::Hang | Doing::Circle) {
                    // Leaned on hard, a fish that still has something left does not hang
                    // there and take it — it goes. This is the one place the fight
                    // answers the *player* rather than a table, and it is why a heavy
                    // hand is not simply a faster way to win: pull harder and you get a
                    // longer fight, which is the first thing anybody is told on a boat.
                    if matches!(n.opening, Doing::Sound) {
                        Doing::Sound
                    } else {
                        Doing::Run
                    }
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
        // And whatever is still coming off the drum from the take itself.
        self.paying += fish.surge;
        fish.surge *= (-dt / TAKE_SURGE_DIES).exp();
        let supplied = self.paying - reel;
        self.line_out = (self.line_out + supplied * dt).max(0.0);
        // The spring takes up whatever the fish's movement, the line supply and the rod
        // tip's own travel disagree about, which is the one equation the whole fight runs
        // on. `swept` is a distance and not a rate, so it goes in as it is.
        self.stretch = (self.stretch + (away - supplied) * dt + swept).clamp(-2.0, 4.0);

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

        // And the teeth, which are not wear on a hook but a leader being sawn through.
        // Only while there is load on it: a slack trace is a trace lying in a mouth
        // rather than being drawn across it, which is also why a shark is lost on the
        // *run* and not while it is hanging there.
        let sawing = n.bite_through > 0.0
            && self.tension > 0.25 * BREAKING
            && self.rng.next_f64() < n.bite_through * dt;

        // --- Everything that ends it -----------------------------------------------
        let beaten = fish.beaten();
        let hold = fish.hold;

        // The holder, before anything else, because if it goes nothing after it matters.
        // Only while nobody has hold of the rod: a hand on it is the whole difference,
        // and it is the difference in life too.
        // And only while it is *running*. A rod is worked out of a tube by a fish that
        // is still taking line, not by a dead weight: the load has to be high, sustained
        // and moving. That is also what keeps the two punishments apart, which they have
        // to be — a drag locked outright does not slip, so a pollack hangs there at
        // eighty newtons and the *hook* comes out, while a drag wound to nine tenths
        // still gives and a tope goes on taking line at that load until the holder
        // tires. Both are the same mistake and they fail differently, and it was worth
        // an afternoon to keep both.
        self.straining = if self.tension > HOLDER && self.paying > 0.2 && !hands_on {
            self.straining + dt
        } else {
            0.0
        };
        if self.straining > HOLDER_HOLDS {
            self.rod = Rod::Gone;
            self.line_out = 0.0;
            self.tension = 0.0;
            self.paying = 0.0;
            self.depth = 0.0;
            self.stretch = 0.0;
            self.straining = 0.0;
            self.gone_for = 0.0;
            self.lost += 1;
            return Some("the rod has gone over the side".into());
        }
        if self.tension > BREAKING {
            self.lost_the_lot();
            return Some(format!(
                "the line parted — {BREAKING:.0} N of braid against {:.0}",
                self.tension.max(BREAKING)
            ));
        }
        if sawing {
            self.lost_the_lot();
            return Some("it bit through the leader — that was a tope".into());
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
            self.lost_the_lot();
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

    /// It has gone, and the lure is still on. Back to trolling with the line where the
    /// fight left it — a thrown hook costs you the fish and nothing else.
    fn give_up(&mut self) {
        self.rod = Rod::Trolling;
        self.stretch = 0.0;
        self.tension = 0.0;
        self.paying = 0.0;
        self.lost += 1;
        self.quiet = 0.0;
    }

    /// It has gone and taken the tackle with it.
    ///
    /// A parted line does not leave you fishing. Everything beyond the break — the lure,
    /// the leader and whatever length of braid was out — is in the sea, and what is on
    /// the reel is whatever was inboard of it, which for these purposes is nothing you
    /// can fish with until you have tied another lure on. So the rod comes in.
    ///
    /// Left as it was, the gear went back to trolling with a hundred and sixty metres
    /// notionally still out, drew a line and a lure astern of a spool whose knot had
    /// just gone, and then parted the same line a second time.
    fn lost_the_lot(&mut self) {
        self.give_up();
        self.rod = Rod::Stowed;
        self.line_out = 0.0;
        self.depth = 0.0;
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
#[allow(clippy::too_many_arguments)]
pub fn work(
    mut gear: ResMut<Fishing>,
    sim: Res<Sim>,
    course: Res<crate::course::Course>,
    sky: Res<crate::sky::Sky>,
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    time: Res<Time>,
    mut since_report: Local<f32>,
    mut landfalls: Local<u32>,
) {
    let dt = time.delta_secs_f64();
    let fine = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    // A new rod comes out of the chandlery, and a chandlery is in a harbour. Hung on the
    // delivery count rather than on a timer or a key, because that is what it costs: not
    // a wait, but *getting there* — which is the game this is attached to, and it means
    // losing the rod is paid for in the thing the rest of the boat is already doing.
    if *landfalls != course.delivered {
        *landfalls = course.delivered;
        gear.ashore();
    }

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
        // The same reading the BARO line on the console shows, and the same half of it:
        // the tendency, not the number.
        glass: sim.sim.wind.barometer().1,
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
    if let Some(said) = gear.step(
        dt,
        sea,
        Hands {
            reel,
            drag,
            // Shift is low gear on the reel and a fine turn on the drag star, which is
            // one meaning rather than two: slower and more deliberate, whichever of them
            // your hand is on.
            low_gear: fine,
            // Held, because a rod held up is a position your arms are holding. Let go
            // and it comes down, which is the half of a pump that loses fish.
            pump: keys.pressed(KeyCode::KeyU),
        },
    ) {
        info!("{said}");
    }

    // A line on the terminal every second while the dev switch is set, so the gear can
    // be told apart from the drawing of it. `world::report_vitals` does the same thing
    // for the boat and for the same reason: when something looks wrong, the first
    // question is always whether it *is* wrong or merely invisible, and that question
    // should cost nothing to answer.
    *since_report += time.delta_secs();
    if gear.out() && *since_report >= 1.0 && crate::knob::var("SAIL_ROD").is_ok() {
        *since_report = 0.0;
        info!(
            "rod: line {:.1} m, lure {:.1} m down over {:.0} m, load {:.0} N, \
             drag {:.0} N, reel {reel:+.1}{}, rod {:.0}%, sea {:.1} m {}, {}",
            gear.line_out,
            gear.depth,
            sea.sounding,
            gear.tension,
            gear.drag_force(),
            if fine { " low" } else { "" },
            gear.lift * 100.0,
            sea.swell,
            if on_the_rise(sea.lift, sea.swell) {
                "rising"
            } else {
                "dropping"
            },
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
            glass: 0.0,
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
            glass: 0.0,
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
            // Wind when there is something to wind against, and not otherwise — reeling
            // your own lure in while nothing is on it is how a long run of this ended
            // with the rod stowed after the first fish and the next two hours spent not
            // fishing.
            let working = Hands {
                reel: if f.fish().is_some() { hands.reel } else { 0.0 },
                ..hands
            };
            if let Some(word) = f.step(1.0 / 60.0, sea, working) {
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
            surge: 0.0,
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
        Hands {
            reel,
            drag,
            low_gear: false,
            pump: false,
        }
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
        // The first one, not the only one: the lure goes back in the water the moment a
        // fish is boated, so a long enough run catches another — which is the gear
        // working rather than the test failing.
        assert_eq!(
            f.caught.first().map(|c| c.what),
            Some(Quarry::Saithe),
            "{said:?}"
        );
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
            sail(&mut f, 35.0, sea(1.0, 30.0), hands(0.0, drag));
            // **The same fish**, and that is the whole correction. Run long enough and
            // the pollack is finished, the lure goes back in the water, and at a knot it
            // sinks onto the ground inside a minute — so what was being measured was the
            // seabed, which has a stamina of a thousand million and is never beaten at
            // all. It read as both drags tiring nothing, which is half the claim being
            // made here and was true for the wrong reason.
            match f.fish() {
                Some(fish) if fish.what == Quarry::Pollack => fish.beaten(),
                _ => 1.0,
            }
        };
        let slack = tired(0.02);
        let proper = tired(1.0 / 3.0);
        // Twice over, not three times, and the gap between those two numbers is worth
        // knowing: a backed-off drag is not *no* load. A spool turning fast has its own
        // resistance — `SLIP` is viscous, nine newtons for every metre a second — so a
        // fish running at two and a half metres a second puts twenty-five newtons on the
        // line through a drag set at under two. Which is true of a real reel and is why
        // a screaming spool still tires a fish, slowly.
        assert!(
            proper > slack * 1.8,
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

    /// Low gear turns a handle that high gear cannot. This is the whole reason a boat
    /// reel has two speeds, and the trade — half the line per turn — has to be there too
    /// or it would simply be a better gear.
    #[test]
    fn low_gear_winds_when_high_gear_will_not() {
        let against = |low_gear: bool| {
            // A pollack sounding, which pulls about sixty newtons — above where a handle
            // in high gear stops and below where one in low gear does, on a drag tight
            // enough that nothing is slipping. That band is the gearbox's whole purpose
            // and it is a narrow one: the first version of this test used a ling and
            // found the drag paying out in both gears, and the second used a saithe,
            // which stopped being in the band the moment the stall was corrected.
            let mut f = with(Quarry::Pollack, 3.0);
            let water = sea(0.8, 40.0);
            // Let the opening run have its way first, with the hands off. Measuring
            // across it instead measured the *fish*, which takes line far faster than
            // either gear gains it — both numbers came out near zero and the comparison
            // was between two kinds of noise.
            sail(
                &mut f,
                2.0,
                water,
                Hands {
                    reel: 0.0,
                    drag: 0.62,
                    low_gear: false,
                    pump: false,
                },
            );
            let before = f.line_out;
            // And then two and a half seconds of winding, still inside its opening run,
            // because that is where the load is: a fish that has moved on to hanging or
            // shaking pulls half as hard and is out of the band the gearbox is for.
            sail(
                &mut f,
                2.5,
                water,
                Hands {
                    reel: 1.0,
                    drag: 0.62,
                    low_gear,
                    pump: false,
                },
            );
            before - f.line_out
        };
        let high = against(false);
        let low = against(true);
        // Not nothing in high gear — the load takes a moment to come on and the handle
        // turns freely until it does — but once it is on, the handle stops and low gear
        // keeps coming.
        assert!(
            low > high * 2.0 + 0.1,
            "low gained {low} m against high gear's {high} m"
        );
    }

    /// And with nothing on the line, high gear is the quicker way to get the lure back,
    /// or the gearbox would be free.
    #[test]
    fn high_gear_is_the_faster_wind() {
        let wound = |low_gear: bool| {
            let mut f = rig(2);
            f.stow_or_set();
            f.line_out = 60.0;
            sail(
                &mut f,
                10.0,
                sea(2.0, 40.0),
                Hands {
                    reel: 1.0,
                    drag: 1.0 / 3.0,
                    low_gear,
                    pump: false,
                },
            );
            60.0 - f.line_out
        };
        assert!(wound(false) > wound(true) * 1.8);
    }

    /// A parted line does not leave you fishing: the lure and everything beyond the
    /// break are in the sea, so the rod comes in rather than going on trolling with a
    /// spool's worth of line notionally still out.
    #[test]
    fn a_parted_line_takes_the_tackle_with_it() {
        let mut f = with(Quarry::Ling, 9.0);
        let said = sail(&mut f, 90.0, sea(6.0, 70.0), hands(0.0, 1.0 / 3.0));
        assert!(said.iter().any(|s| s.contains("parted")), "{said:?}");
        assert!(!f.out(), "the rod should have come in");
        assert_eq!(f.line_out, 0.0);
    }

    /// Where a thrown hook does not: the lure is still on the end and you carry on.
    #[test]
    fn a_thrown_hook_leaves_you_fishing() {
        let mut f = with(Quarry::Pollack, 4.0);
        let said = sail(&mut f, 200.0, sea(1.5, 30.0), hands(0.0, 1.0));
        assert!(
            said.iter().any(|s| s.contains("hook")),
            "expected the hook to go: {said:?}"
        );
        assert!(f.out(), "the gear should still be fishing");
        assert!(f.line_out > 1.0);
    }

    /// **The headline claim.** Every fish big enough to take this lure rips line off the
    /// spool when it does — including the smallest one in the table, which pulls eleven
    /// newtons against a thirty newton drag and, before the shock of the take was
    /// modelled, simply arrived.
    #[test]
    fn every_take_rips_line_off_the_spool() {
        for what in SPECIES {
            let mut f = rig(3);
            f.rod = Rod::Trolling;
            f.line_out = 20.0;
            f.drag = 1.0 / 3.0;
            let water = sea(3.5, 30.0);
            f.hook(what, water);
            let before = f.line_out;
            // A second is enough: the drag breaks away in the first few milliseconds.
            sail(&mut f, 1.0, water, hands(0.0, 1.0 / 3.0));
            assert!(
                f.line_out > before + 0.08,
                "{what:?} took only {:.3} m",
                f.line_out - before
            );
        }
    }

    /// And a drag screwed down past what the fish can pull gives nothing, which is the
    /// same lesson the dial teaches everywhere else rather than an exception to it.
    #[test]
    fn a_locked_drag_gives_no_line_even_at_the_take() {
        let mut f = rig(3);
        f.rod = Rod::Trolling;
        f.line_out = 20.0;
        f.drag = 1.0;
        let water = sea(2.0, 30.0);
        f.hook(Quarry::Mackerel, water);
        let before = f.line_out;
        sail(&mut f, 1.0, water, hands(0.0, 1.0));
        assert!(f.line_out <= before + 0.01);
    }

    /// Not everything that hits a lure stays on it, and the miss is information: it says
    /// you are in the right water. Without it the only two states were nothing and a
    /// fish, and an hour of nothing looked exactly like an hour of fishing badly.
    #[test]
    fn some_of_them_knock_it_and_miss() {
        // Fished properly, so each fish is landed and the lure goes back to work —
        // otherwise the first take stays on for the rest of the hour and the roll that
        // this test is about is never made again.
        let mut f = rig(21);
        f.stow_or_set();
        f.line_out = 14.0;
        let said = fish_it(&mut f, 7200.0, sea(3.6, 20.0), hands(1.0, 1.0 / 3.0));
        assert!(said.iter().any(|s| s.contains("knocked")), "{said:?}");
        // And a knock is felt, not merely announced.
        let mut g = rig(21);
        g.rod = Rod::Trolling;
        g.line_out = 14.0;
        g.knock = 20.0;
        let quiet = sea(2.0, 30.0);
        g.step(0.1, quiet, hands(0.0, 1.0 / 3.0));
        let knocked = g.tension;
        let mut h = rig(21);
        h.rod = Rod::Trolling;
        h.line_out = 14.0;
        h.step(0.1, quiet, hands(0.0, 1.0 / 3.0));
        assert!(knocked > h.tension + 2.0, "{knocked} against {}", h.tension);
    }

    /// A front on the way turns them on, which is the oldest belief in angling and one
    /// of the few that holds up — and it puts the fishing and the sailing on the same
    /// instrument, since the hour you ought to be reefing is the hour they are feeding.
    #[test]
    fn a_falling_glass_fishes_better_than_a_rising_one() {
        // Asserted on the factor rather than on a count of takes in a simulation. The
        // count is a sample of a Poisson process with a handful of events in it, and the
        // difference this makes — half again against a third fewer — is smaller than the
        // noise in a sample that size. A test that passes or fails on the seed is worse
        // than no test, and the first version of this one did exactly that.
        // Falling glass is the good one, and the sign has been the wrong way round once
        // already in this file's life.
        assert!(bite_weather(-6.0) > bite_weather(0.0) * 1.3);
        assert!(bite_weather(6.0) < bite_weather(0.0) * 0.8);
        assert!(bite_weather(0.0) == 1.0);
        // And it never shuts them off altogether, because they do still feed.
        assert!(bite_weather(100.0) > 0.2);
    }

    /// Pumping gains line where the reel alone cannot, which is the whole reason every
    /// angler is taught it before they are taught anything else.
    ///
    /// Measured against a hand that only winds, on the same fish, at a load that has the
    /// handle stopped. The rod is doing the lifting and your back is doing the work, and
    /// neither of them cares what the gearing is.
    #[test]
    fn pumping_gains_line_the_reel_cannot() {
        let worked = |pump: bool| {
            // Three kilogrammes rather than four, and the difference is the whole
            // condition the technique has. A pump only gains line while the drag is
            // *holding*: lift against a fish strong enough to open it and all you have
            // done is pay line out through a slipping clutch, which is what the first
            // version of this test measured and reported as pumping being useless.
            let mut f = with(Quarry::Pollack, 4.0);
            let water = sea(0.8, 40.0);
            let before = f.line_out;
            // Four full cycles: up for a second and a half, down for a second with the
            // handle turning, which is the rhythm.
            let mut t = 0.0;
            while t < 9.0 {
                let up = pump && (t % 3.5) < 2.2;
                f.step(
                    1.0 / 60.0,
                    water,
                    Hands {
                        // Winding on the drop, which is the half of it that keeps what
                        // the lift gained.
                        reel: 1.0,
                        drag: 0.8,
                        low_gear: false,
                        pump: up,
                    },
                );
                t += 1.0 / 60.0;
            }
            before - f.line_out
        };
        let pumped = worked(true);
        let wound = worked(false);
        // A ratio rather than a margin: what matters is that the rod is doing the work
        // the handle cannot, and over twelve seconds of a fight that is a few tenths of
        // a metre against nearly nothing.
        assert!(
            pumped > wound * 2.5,
            "pumping gained {pumped} m, winding alone {wound} m"
        );
    }

    /// And dropping the rod without winding makes slack, which is what loses fish — the
    /// second half of "lift and wind down" is not advice, it is the whole technique.
    #[test]
    fn dropping_the_rod_on_a_slack_reel_makes_slack() {
        let slack_after = |reel: f64| {
            let mut f = with(Quarry::Cod, 4.0);
            let water = sea(0.5, 40.0);
            // Lift it right up, then let it fall.
            let mut t = 0.0;
            while t < 2.0 {
                f.step(
                    1.0 / 60.0,
                    water,
                    Hands {
                        reel: 0.0,
                        drag: 0.5,
                        low_gear: false,
                        pump: true,
                    },
                );
                t += 1.0 / 60.0;
            }
            let mut least = f.tension;
            t = 0.0;
            while t < 1.5 {
                f.step(
                    1.0 / 60.0,
                    water,
                    Hands {
                        reel,
                        drag: 0.5,
                        low_gear: false,
                        pump: false,
                    },
                );
                least = least.min(f.tension);
                t += 1.0 / 60.0;
            }
            least
        };
        let wound_down = slack_after(1.0);
        let let_go = slack_after(0.0);
        assert!(
            let_go < wound_down,
            "dropping on a slack reel left {let_go} N, winding down left {wound_down} N"
        );
    }

    /// The dev switch takes a fish by name and refuses anything else, because silently
    /// hooking a mackerel when you asked for a ling is a worse afternoon than being told
    /// you spelled it wrong.
    #[test]
    fn a_fish_can_be_asked_for_by_name() {
        assert_eq!(Quarry::named("ling"), Some(Quarry::Ling));
        assert_eq!(Quarry::named("saithe"), Some(Quarry::Saithe));
        assert_eq!(Quarry::named("trout"), None);
        // The ground is not in the list, because it is not caught.
        assert_eq!(Quarry::named("the bottom"), None);
        // And every name in the table round-trips, so adding a species cannot leave it
        // unreachable from the switch.
        for what in SPECIES {
            assert_eq!(Quarry::named(what.name()), Some(what));
        }
    }

    /// **The one loss you do not get back**, and it happens for exactly one reason: a
    /// drag wound up near the top, a fish big enough to use it, and nobody holding the
    /// rod.
    ///
    /// Seven tenths and not the whole way, and the difference between those is the whole
    /// shape of it. A drag only slips once the load passes it by a third, so wound right
    /// to the top it never gives at all: the load runs straight past the line's breaking
    /// strain and the *line* goes, leaving a parted line and a rod still in its holder.
    /// It is the drag that still gives — but gives at seventy newtons — that holds the
    /// load up there second after second while the fish goes on taking line, and that is
    /// what works a rod out of a tube.
    ///
    /// So everything above the rule of thumb is a mistake and there are two different
    /// punishments for it, which is worth the arithmetic it took to keep both.
    #[test]
    fn a_tight_drag_on_a_big_fish_takes_the_rod_over_the_side() {
        let mut f = with(Quarry::Tope, 11.0);
        let said = sail(&mut f, 20.0, sea(1.5, 40.0), hands(0.0, 0.7));
        assert!(said.iter().any(|s| s.contains("over the side")), "{said:?}");
        assert!(!f.have_a_rod());
        // And it does not come back by waiting.
        sail(&mut f, 600.0, sea(1.5, 40.0), hands(0.0, 1.0 / 3.0));
        assert!(!f.have_a_rod(), "a rod does not swim back");
    }

    /// A hand on it is the whole difference, and it is the difference in life too.
    #[test]
    fn a_hand_on_the_rod_keeps_it_aboard() {
        let mut f = with(Quarry::Tope, 11.0);
        let mut t = 0.0;
        while t < 20.0 {
            f.step(
                1.0 / 60.0,
                sea(1.5, 40.0),
                Hands {
                    reel: 0.0,
                    drag: 0.7,
                    low_gear: false,
                    pump: true,
                },
            );
            t += 1.0 / 60.0;
        }
        assert!(f.have_a_rod(), "it should not have gone with hands on it");
    }

    /// And a drag set anywhere near the rule never gets near it, because a slipping drag
    /// is a load that cannot build. The holder is not a hazard, it is a consequence.
    #[test]
    fn a_proper_drag_never_threatens_the_holder() {
        let mut f = with(Quarry::Tope, 11.0);
        sail(&mut f, 60.0, sea(1.5, 40.0), hands(0.0, 1.0 / 3.0));
        assert!(f.have_a_rod());
    }

    /// A harbour is where a new one comes from, and nowhere else.
    #[test]
    fn a_new_rod_comes_from_the_chandlery() {
        let mut f = with(Quarry::Tope, 11.0);
        sail(&mut f, 20.0, sea(1.5, 40.0), hands(0.0, 0.7));
        assert!(!f.have_a_rod());
        f.ashore();
        assert!(f.have_a_rod());
        // And it arrives stowed, not streamed: you have to put it out again.
        assert!(!f.out());
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
