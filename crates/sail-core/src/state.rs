//! Simulation state, player controls, and telemetry.
//!
//! Sign conventions, stated once because a sign error in a moment is a day of
//! debugging. Verified in `tests/conventions.rs`.
//!
//! * World: `x` east, `y` north, metres. Heading measured from north, clockwise.
//! * Body: `x` bow, `y` starboard, `z` down. Right-handed, so a positive yaw moment
//!   swings the bow to starboard and a positive roll moment heels to starboard.
//! * Heel is positive to starboard. Wind on the starboard side therefore gives
//!   negative heel.
//! * Apparent wind angle is positive with the wind on the starboard side.
//! * Rudder angle is positive when it turns the bow to starboard.

use crate::sail::SailForces;
use glam::DVec2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoatState {
    /// World position, metres.
    pub pos: DVec2,
    /// Heading from north, clockwise, radians.
    pub heading: f64,
    /// Heel, radians, positive to starboard.
    pub heel: f64,
    /// Surge: speed along the hull axis, m/s.
    pub surge: f64,
    /// Sway: speed athwartships, m/s, positive to starboard.
    pub sway: f64,
    /// Yaw rate, rad/s, positive turning to starboard.
    pub yaw_rate: f64,
    /// Roll rate, rad/s.
    pub roll_rate: f64,
    /// Heave: elevation of the hull reference above mean water level, metres,
    /// positive up. Zero means floating at the design waterline in flat water.
    pub heave: f64,
    pub heave_rate: f64,
    /// Pitch, radians, positive bow-up.
    pub pitch: f64,
    pub pitch_rate: f64,
    /// Where the boom actually is, radians from the centreline, always positive —
    /// which side it is on is decided by the wind.
    ///
    /// A degree of freedom of its own, because a boom is sixty kilogrammes of spar and
    /// cloth hanging off a lever and a sheet that can only pull. See
    /// [`crate::sim::Simulation`] for what that costs in a seaway.
    pub boom_angle: f64,
    /// How far through changing sides the headsail is, nought to one.
    ///
    /// Nought is the instant the wind crossed: the sail is still sheeted on what has just
    /// become the windward side, backed and pushing her bow round, which is the one
    /// moment in a tack when a jib is doing something useful by being wrong. One is set
    /// and drawing on the new side. In between it is across the foretriangle, flogging,
    /// making nothing — and that crossing is most of what a tack *looks* like from the
    /// cockpit.
    pub jib_across: f64,
    /// Where the spinnaker pole is squared to, radians from the centreline, as a
    /// magnitude: which side it is on is the opposite of the boom's.
    ///
    /// It follows the trim while it is free, and stops dead where it is when the
    /// preventer goes on — because that is what the button does. One line forward from
    /// each spar, and after that neither of them moves.
    pub pole_angle: f64,
    /// How fast it is swinging, rad/s.
    pub boom_rate: f64,
    /// Seconds since start.
    pub time: f64,
}

impl BoatState {
    /// Blend two states, for drawing a frame that falls between two physics substeps.
    ///
    /// The integrator advances in fixed 5 ms steps and a frame almost never lands on
    /// one, so the leftover has to be interpolated or the picture stutters — see
    /// [`Simulation::render_state`](crate::sim::Simulation::render_state).
    ///
    /// Heading goes the short way round. Lerping 359 degrees to 1 degree through the
    /// middle spins the boat the wrong way through 358 of them, once per lap, which
    /// is a bug that hides until somebody sails north.
    pub fn lerp(a: &Self, b: &Self, t: f64) -> Self {
        let f = |x: f64, y: f64| x + (y - x) * t;
        Self {
            pos: a.pos + (b.pos - a.pos) * t,
            heading: crate::units::wrap_tau(
                a.heading + crate::units::wrap_pi(b.heading - a.heading) * t,
            ),
            heel: f(a.heel, b.heel),
            surge: f(a.surge, b.surge),
            sway: f(a.sway, b.sway),
            yaw_rate: f(a.yaw_rate, b.yaw_rate),
            roll_rate: f(a.roll_rate, b.roll_rate),
            heave: f(a.heave, b.heave),
            heave_rate: f(a.heave_rate, b.heave_rate),
            pitch: f(a.pitch, b.pitch),
            pitch_rate: f(a.pitch_rate, b.pitch_rate),
            boom_angle: f(a.boom_angle, b.boom_angle),
            jib_across: f(a.jib_across, b.jib_across),
            pole_angle: f(a.pole_angle, b.pole_angle),
            boom_rate: f(a.boom_rate, b.boom_rate),
            time: f(a.time, b.time),
        }
    }

    /// Stopped, on the given heading.
    pub fn at_rest(heading: f64) -> Self {
        Self {
            pos: DVec2::ZERO,
            heading,
            heel: 0.0,
            surge: 0.0,
            sway: 0.0,
            yaw_rate: 0.0,
            roll_rate: 0.0,
            heave: 0.0,
            heave_rate: 0.0,
            pitch: 0.0,
            pitch_rate: 0.0,
            boom_angle: 0.0,
            boom_rate: 0.0,
            // Set and drawing, because a boat at rest is not in the middle of a tack.
            jib_across: 1.0,
            pole_angle: 0.0,
            time: 0.0,
        }
    }

    /// Unit vector along the hull axis, in world coordinates.
    #[inline]
    pub fn forward(&self) -> DVec2 {
        DVec2::new(self.heading.sin(), self.heading.cos())
    }

    /// Unit vector to starboard, in world coordinates.
    #[inline]
    pub fn starboard(&self) -> DVec2 {
        DVec2::new(self.heading.cos(), -self.heading.sin())
    }

    /// Velocity over ground, world coordinates.
    #[inline]
    pub fn velocity_world(&self) -> DVec2 {
        self.forward() * self.surge + self.starboard() * self.sway
    }

    /// Rotate a world vector into the body frame. The transform is its own inverse in
    /// this parameterisation.
    #[inline]
    pub fn to_body(&self, world: DVec2) -> DVec2 {
        let (s, c) = (self.heading.sin(), self.heading.cos());
        DVec2::new(world.x * s + world.y * c, world.x * c - world.y * s)
    }

    /// Speed through the water, m/s. Over flat water with no current this equals
    /// speed over ground.
    #[inline]
    pub fn speed(&self) -> f64 {
        (self.surge * self.surge + self.sway * self.sway).sqrt()
    }

    /// Leeway: the angle between where the bow points and where the boat actually
    /// goes, radians. Positive means slipping to starboard.
    #[inline]
    pub fn leeway(&self) -> f64 {
        if self.surge.abs() < 1e-3 {
            0.0
        } else {
            self.sway.atan2(self.surge)
        }
    }
}

/// What the player is doing with their hands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Controls {
    /// Rudder angle, radians, positive turns the bow to starboard.
    pub rudder: f64,
    /// Mainsheet: 0 hard in, 1 fully eased.
    pub main_sheet: f64,
    /// Genoa sheet: 0 hard in, 1 fully eased.
    pub jib_sheet: f64,
    /// Whether the headsail's clew is held on the windward side — **aback**.
    ///
    /// The sheet is a rope made up on one side of the boat, and which side that is was
    /// not a fact this model had: the sail was put to leeward by construction, every
    /// substep, from the sign of the apparent wind. Which is where it is nine hundred
    /// and ninety-nine times out of a thousand and is also the reason the thousandth
    /// could not happen at all.
    ///
    /// The thousandth is the one that matters. A sail held to windward has the wind on
    /// the front of it, and what that does is push the bow off — which is the only way
    /// out of irons, the thing a crew does without being asked every time a tack fails,
    /// and half of what it means to heave to. None of those existed here. She went head
    /// to wind, stopped, and stayed there with a sail up doing nothing, which the engine
    /// described to itself as "no steerage and goes nowhere — correct physics". It was
    /// correct about the first half.
    pub headsail_aback: bool,
    /// Outhaul: 0 eased right off for a deep, powerful main, 1 hard out for a flat
    /// one. Depth is power; flat is less heeling force and less drag. The call changes
    /// with the breeze, which is why it is a control and not a constant.
    pub outhaul: f64,
    /// Boom vang, 0 eased right off to 1 hard on. Holds the leech down, and therefore
    /// controls twist — but only once the boom is out far enough that the mainsheet
    /// has stopped doing the job itself.
    pub vang: f64,
    /// Mainsail reef, 0 to 1 of the available reduction. Slab or in-mast reefing.
    pub reef: f64,
    /// Headsail sheet lead car, 0 right forward to 1 right aft. The genoa's outhaul
    /// and vang in one fitting: forward for depth and a closed leech, aft for a flat
    /// foot and a head that spills.
    pub genoa_car: f64,
    /// Headsail furl, 0 fully unrolled to 1 rolled completely away. A roller-furling
    /// genoa is a separate piece of gear from the main's reef, and rolling it is
    /// usually the first thing anyone does — so it gets its own control.
    pub furl: f64,
    /// Which sail is set forward of the mast.
    pub headsail: Headsail,
    /// How much of the staysail is set: 0 handed, 1 drawing.
    pub staysail: f64,
    /// And the trysail: 0 handed, 1 set.
    ///
    /// **It only draws with the main off her**, which is not a rule invented for the
    /// game: the trysail goes on its own track up the mast and the mainsail is on that
    /// track. You cannot have both, and the order — main down, trysail up — is the
    /// manoeuvre. Set it with the main still bent on and the control moves and nothing
    /// happens, which is also what would happen on deck.
    pub trysail: f64,
    /// Its sheet, which is a tackle on the end of the club boom: 0 hauled flat, 1 eased.
    ///
    /// It does not change sides — that is what the boom is for — but it is still a line
    /// somebody pulls, and it is the only shape control the sail has. Hauled, the boom
    /// comes in and the leech tightens; eased, the boom swings out and the sail opens.
    pub staysail_sheet: f64,
    /// A preventer on the boom: rigged or not.
    ///
    /// The one line aboard that exists to stop something rather than to make it happen.
    /// Led from the boom end forward to the rail and made fast, it is what a cruising
    /// crew rigs before a long run, so that an accidental gybe is a backed mainsail and a
    /// round-up rather than a spar across the cockpit.
    pub preventer: bool,
}

/// Hand-written rather than derived, because a derived default would put the outhaul
/// fully eased — the deepest possible main, not the shape the sail coefficients were
/// calibrated at. Every test and warm-up step that reaches for `Controls::default()`
/// would quietly be measuring a different sail.
impl Default for Controls {
    fn default() -> Self {
        Self {
            rudder: 0.0,
            main_sheet: 0.0,
            jib_sheet: 0.0,
            headsail_aback: false,
            outhaul: 0.5,
            vang: 0.6,
            genoa_car: 0.4,
            reef: 0.0,
            furl: 0.0,
            headsail: Headsail::Genoa,
            staysail: 0.0,
            trysail: 0.0,
            staysail_sheet: 0.35,
            preventer: false,
        }
    }
}

/// What is set on the stay, or flying from the sprit.
///
/// One slot, not three, because that is how a boat with one bow works: the genoa rolls
/// away and the reaching sail goes up in its place. They are alternatives in the gear
/// as well as in the model.
///
/// The spread between them is the whole reason to have them. A genoa is the sail that
/// goes upwind. A Code 0 is a flat, enormous reaching sail — set flying on its own
/// luff rope ahead of the forestay, it carries from about fifty degrees of apparent
/// wind out to a beam reach and it is the sail that turns a light-air passage from four
/// knots into seven. A gennaker is the running sail: far bigger, far deeper, and
/// useless anywhere near the wind, because a sail with that much shape in it cannot be
/// flattened and will not stay full with the apparent wind forward.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Headsail {
    #[default]
    Genoa,
    Code0,
    Gennaker,
    /// The symmetric kite, on a pole. Deeper, bigger and squarer than the gennaker, and
    /// the only sail aboard whose tack goes out to *windward* — which is what takes it
    /// out of the mainsail's shadow and lets her run deep.
    Spinnaker,
}

impl Headsail {
    /// Set flying from the sprit rather than hanked to the forestay.
    ///
    /// Which decides two things at once: a flying sail is tacked well forward of the
    /// stem, and it is clear of the mast and the shrouds, so none of the rig
    /// interference a genoa suffers applies to it.
    pub fn flying(self) -> bool {
        !matches!(self, Headsail::Genoa)
    }

    /// The apparent wind angle below which it stops staying full, radians.
    ///
    /// A headsail on a stay has its luff held by the stay and simply luffs along it
    /// when you point too high. A sail set flying has nothing holding its luff but its
    /// own tension and the wind in it, so with the apparent wind too far forward the
    /// luff falls in and the sail collapses — not a soft loss of drive but a sail
    /// flogging in the lee of the main. That limit, and not area, is what decides which
    /// sail you have up.
    /// Measured against the boat, not guessed. The first numbers here were 48 and 70
    /// degrees, taken from how these sails are talked about rather than from anything,
    /// and the sweep showed what that costs: this boat is fast enough that a beam reach
    /// in eight knots of breeze puts the apparent wind at 46 degrees, so a Code 0 — the
    /// one sail that exists for exactly that condition — was collapsing in it. A limit
    /// expressed in apparent wind has to be set where the apparent wind actually is.
    pub fn fills_above(self) -> f64 {
        match self {
            Headsail::Genoa => 0.0,
            Headsail::Code0 => crate::units::rad(32.0),
            Headsail::Gennaker => crate::units::rad(52.0),
            // Higher than the gennaker's, because a symmetric cannot be carried as close
            // as an asymmetric: it has no luff tension to hold a leading edge with and
            // the pole will not let it sheet in. Reaching with a spinnaker up is how
            // people end up swimming.
            Headsail::Spinnaker => crate::units::rad(70.0),
        }
    }

    /// The name on the sail bag.
    pub fn name(self) -> &'static str {
        match self {
            Headsail::Genoa => "genoa",
            Headsail::Code0 => "code 0",
            Headsail::Gennaker => "gennaker",
            Headsail::Spinnaker => "spinnaker",
        }
    }
}

impl Controls {
    /// Both sheets hard in, rudder centred, outhaul at a working upwind setting.
    pub fn close_hauled() -> Self {
        Self::default()
    }
}

/// Everything worth putting on a gauge, a telltale, or a chart. The polar overlay
/// built from `vmg` and `speed` is the single most useful development instrument.
/// The three ways a passage ends badly, which are the three a cruising sailor fears.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mishap {
    /// The keel found the bottom.
    Aground,
    /// A sail was carried in more wind than it is built for, and the cloth let go.
    BlownOut(Blown),
    /// She went past the point where the righting arm gives up.
    Capsized,
}

/// Which sail it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blown {
    Main,
    Headsail(Headsail),
    Staysail,
}

impl Mishap {
    /// What to tell somebody who was looking at the sea rather than the instruments.
    pub fn what_happened(&self) -> &'static str {
        match self {
            Mishap::Aground => "aground — the keel found the bottom",
            Mishap::BlownOut(Blown::Main) => "the mainsail blew out",
            Mishap::BlownOut(Blown::Headsail(Headsail::Genoa)) => "the genoa blew out",
            Mishap::BlownOut(Blown::Headsail(Headsail::Code0)) => "the Code 0 blew out",
            Mishap::BlownOut(Blown::Headsail(Headsail::Gennaker)) => "the gennaker blew out",
            Mishap::BlownOut(Blown::Headsail(Headsail::Spinnaker)) => "the spinnaker blew out",
            Mishap::BlownOut(Blown::Staysail) => "the staysail blew out",
            Mishap::Capsized => "capsized",
        }
    }

    /// And what they should have done, because a loss you cannot learn from is a bug.
    pub fn the_lesson(&self) -> &'static str {
        match self {
            Mishap::Aground => "the depth gauge was on the panel the whole time",
            Mishap::BlownOut(_) => "every sail is cut for a wind; past it the cloth goes",
            Mishap::Capsized => "she asked to be reefed long before this",
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Telemetry {
    /// Water under the keel's deepest point, metres. Negative means she is on it.
    pub under_keel: f64,
    /// Depth of water, metres, as a sounder reads it — the charted sounding plus the
    /// tide, which is what makes a bar passable in the afternoon and not at dawn.
    pub depth: f64,
    /// Height of the tide above mean level, metres, and the stream it arrived on, m/s
    /// over the ground.
    ///
    /// Published because a tide you cannot read is indistinguishable from a bug: the
    /// boat crabs sideways, the log and the ground disagree, and without a number
    /// saying why, the only available explanation is that the steering is broken.
    pub tide: f64,
    pub stream: glam::DVec2,
    /// True wind speed and direction at reference height.
    pub tws: f64,
    pub twd: f64,
    /// True wind angle relative to the bow, positive from starboard.
    pub twa: f64,
    /// Apparent wind at the rig, speed and angle.
    pub aws: f64,
    pub awa: f64,
    /// How much each sail's wind has been bent by the other, radians, to be added to the
    /// apparent wind angle before trimming it. See [`crate::sail::slot_interaction`].
    ///
    /// Published rather than kept inside the force model because a trimmer has to work
    /// with the same numbers the sail does: the whole reason a mainsail is sheeted near
    /// the centreline upwind is the headsail's downwash, and a trim routine that does not
    /// know about it will sheet the main for a wind that is not there.
    pub bend_main: f64,
    pub bend_jib: f64,
    /// Height above the water the apparent wind was read at, metres.
    ///
    /// Not a detail. The true wind is quoted at ten metres and the boundary layer
    /// means it is weaker lower down, so `aws` and `tws` are not comparable without
    /// knowing where each was measured. Leaving this implicit is how
    /// `aws = tws - stw` stops holding for reasons that look like a bug.
    pub wind_height: f64,
    /// Speed through the water and over ground, m/s.
    pub stw: f64,
    pub sog: f64,
    /// Course over ground, radians from north.
    pub cog: f64,
    /// Velocity made good towards the wind, m/s. Positive is upwind progress.
    pub vmg: f64,
    pub heel: f64,
    pub leeway: f64,
    pub yaw_rate: f64,

    /// Lower section of the mainsail — the one whose chord angle is the boom's.
    pub main: SailForces,
    /// Storm canvas aft, when it is up. Zero otherwise — and zero is a real answer
    /// here, because the drawn cloth reads the luffing out of it.
    pub trysail: SailForces,
    /// Upper section, eased by the twist in the leech.
    pub main_upper: SailForces,
    /// Twist between the two, radians.
    pub twist: f64,
    /// Lower section of the headsail.
    pub jib: SailForces,
    /// Upper section of the headsail, eased by the car's twist.
    pub jib_upper: SailForces,
    pub jib_twist: f64,

    /// Sea state and the boat's motion in it.
    pub significant_height: f64,
    pub heave: f64,
    pub pitch: f64,
    /// Mean immersion across the hull stations, metres. The design figure is the canoe
    /// body draft; much more than that and the boat is burying itself.
    pub immersion: f64,
    /// Vertical acceleration at the hull, m/s^2. What the crew actually feels, and
    /// what makes a short steep sea so much worse than a long one of the same height.
    pub heave_accel: f64,

    /// How near the rig is to breaking something, nought to one.
    ///
    /// Not a force but a *countdown*: a sail carried over the wind its cloth is cut for
    /// accumulates this at the rate it is overloaded by, and at one it goes. So the
    /// number answers a question no instantaneous reading can — whether she is merely
    /// pressed or has already run out of time — and it is the one thing on this boat
    /// that is genuinely past what she can do rather than near it.
    ///
    /// The worst of the three sails, because the first one to go is the one that matters.
    pub strain: f64,

    /// Where the headsail's clew is, as a fraction from port to starboard.
    ///
    /// Plus one is hard on the starboard side, minus one on the port, and nought is the
    /// sail streaming straight back down the centreline with the wind in neither side of
    /// it — which is what it is doing in the middle of a tack. For the picture, which
    /// otherwise has to pick a side from the wind and therefore flicks the sail across
    /// the foretriangle in a single frame.
    pub jib_side: f64,

    /// Where the spinnaker pole is squared to, radians from the centreline. Positive to
    /// starboard, like the boom, and always the opposite side from it.
    pub pole: f64,

    /// Where the boom actually is, radians, **positive to starboard**.
    ///
    /// The one place the boom's position is stated in the boat's own frame rather than
    /// relative to leeward, and it exists because every consumer outside the force model
    /// wants that: the thing drawn over the deck, the thing sent to another player, the
    /// thing a sailor ducks under. `BoatState::boom_angle` is signed towards leeward
    /// because that is the frame the sail's angle of attack lives in, and a renderer that
    /// re-derives the side from the wind gets a spar that jumps across the boat the
    /// instant the wind does — which is exactly what it did, after the simulation had
    /// already been taught not to.
    pub boom: f64,

    /// Net driving force and side force from the rig, N.
    pub drive: f64,
    pub side: f64,
    /// Total hull resistance, N.
    pub resistance: f64,
    /// Breakdown, for working out where the speed went.
    pub resistance_friction: f64,
    pub resistance_residuary: f64,
    pub resistance_induced: f64,
    /// Added resistance from moving in a seaway. Zero in flat water.
    pub resistance_waves: f64,
    /// Component of the boat's weight acting along its axis, newtons. Positive is
    /// forward: bow-down on a wave face, which is surfing.
    pub surf_force: f64,

    /// Angle of attack on the lateral plane, radians — the keel's working angle
    /// relative to the water it is actually in, which in a seaway is not the same as
    /// the boat's track over the ground.
    pub keel_alpha: f64,
    pub keel_stalled: f64,
    /// Flow at the keel in the body frame, m/s. Diagnostic: when this reverses, the
    /// boat is being told the water is overtaking it, and everything downstream lies.
    pub keel_surge: f64,
    pub keel_sway: f64,
    /// Rudder angle of attack and how stalled it is.
    pub rudder_alpha: f64,
    pub rudder_stalled: f64,
    /// How much of its nominal authority the blade actually has, 0 to 1.
    ///
    /// A rudder on the centreline cannot be seen from the boat — it is a metre under
    /// the transom and the water is not transparent — so the three things that take its
    /// grip away are invisible unless they are reported. All three are real and all
    /// three are in the model already; this is the one number that says how much is
    /// left, and it is computed from exactly the factors the force itself uses rather
    /// than from a second estimate of them:
    ///
    /// * **Speed.** Force goes as the square of the flow over the blade, so a boat at
    ///   two knots has a sixth of the helm she has at five. This is why she will not
    ///   answer coming out of a tack, and it is the first thing a new helm gets wrong.
    /// * **Heel**, two ways. The blade's lift is normal to its own plane, and heeled
    ///   over that plane is tilted, so only the cosine of the heel acts in the
    ///   horizontal — at forty degrees that is three quarters. Past thirty degrees the
    ///   stern also starts to come out and the blade loses up to another half of its
    ///   area, which is the mechanism that makes a knocked-down boat stop steering.
    /// * **Stall.** Past about thirty degrees of helm the flow separates and the blade
    ///   is a brake rather than a foil, which is the other half of why hauling the
    ///   tiller harder is not the answer.
    pub rudder_bite: f64,
    /// Net yaw moment with the rudder centred — positive means the boat wants to luff.
    /// This is weather helm, expressed as a number.
    pub weather_helm: f64,
    /// And what it is made of, because "she gripes" is a symptom with three causes and
    /// they want opposite cures.
    ///
    /// A crew that reefs when the trouble is heel does the right thing; a crew that
    /// rolls the genoa when the trouble is the centre of effort makes it worse, and the
    /// one thing that cannot tell them apart is a single number. These three are the
    /// decomposition, all about the same origin as `weather_helm` and summing to it:
    ///
    /// * **The rig**, which is where the sails push and how far that is from where the
    ///   keel holds. Positive is luffing — a main alone, with its load aft, does this.
    ///   A headsail's is negative, because its load is forward, and that is why rolling
    ///   one away is not a cure for weather helm.
    /// * **Heel**, through the asymmetric waterplane and through the rig's own load
    ///   swinging out to leeward onto a lever arm. This one grows with the square of the
    ///   speed and the sine of the heel, and it is the one that reefing cures.
    /// * **The hull**, meaning the lateral plane and what is left of the bare-hull
    ///   damping.
    pub yaw_from_rig: f64,
    pub yaw_from_heel: f64,
    pub yaw_from_hull: f64,

    /// What ended the passage, once something has.
    ///
    /// Sticky: a sail that has blown out does not come back, and a boat on the rocks
    /// stays there. The simulation keeps running — she still rolls, still drifts, and
    /// the sea does not care — but this is set and the game above has something to say.
    pub mishap: Option<Mishap>,

    /// The surge ledger, in newtons: what pushes her along her own axis and what holds
    /// her back, signed so forward is positive and the three of them sum to the net.
    ///
    /// Put here because the question "why is she doing more than the wind speed" has no
    /// answer in a model that reports only a total, and because the obvious ledger — the
    /// drive against `resistance` — is not a ledger at all: `resistance` is a sum of drag
    /// *magnitudes* measured along several different directions, and the surge equation
    /// is a sum of *components* along one. Comparing them leaves a fifth of the force
    /// unaccounted for and it looks exactly like a leak.
    ///
    /// What is actually in that fifth is the foils. A keel at a leeway angle makes lift
    /// perpendicular to the flow, and the flow is not along the hull, so some of that
    /// lift points forward — the same mechanism as a sail, working in the other fluid,
    /// and the reason a boat with a good keel goes to windward at all. It belongs in the
    /// books as thrust, not hidden inside a word that means drag.
    pub surge_from_sails: f64,
    pub surge_from_hull: f64,
    pub surge_from_foils: f64,
    /// Righting arm at the current heel, m.
    pub righting_arm: f64,
}

impl Telemetry {
    /// True if either sail is shaking enough to see or hear.
    pub fn luffing(&self) -> bool {
        self.main.coeffs.luffing > 0.25 || self.jib.coeffs.luffing > 0.25
    }
}
