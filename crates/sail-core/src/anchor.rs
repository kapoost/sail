//! The anchor, the chain, and whether the two of them are holding.
//!
//! **Anchoring is the end of a passage and it was a teleport.** Arriving was a circle on
//! the chart that you sat in until a number went up. Everything that makes arriving a
//! piece of seamanship — choosing water you can lie in, rounding up head to wind, letting
//! go where the chain will lie fair, veering enough of it, and then finding out whether
//! she is holding — was missing, and all of it is arithmetic over things this model
//! already has: a depth, a wind, a stream, and a hull with windage.
//!
//! The whole of it is three numbers:
//!
//! * **Scope**, the chain out over the water under her. It is the one thing a crew
//!   controls and it is most of whether she holds: the anchor only bites while the pull
//!   on it is *along the bottom*, and what keeps it along the bottom is the weight of
//!   chain lying there. Three to one is a lunch stop in a calm, five to one is a night,
//!   and under about two and a half the pull comes up at an angle that lifts the shank
//!   and she walks.
//! * **Holding power**, what the ground will give. A good bottom holds many times the
//!   anchor's own weight; a bad one holds a fraction of it, and the difference between
//!   sand and weed is the difference between sleeping and not.
//! * **The pull**, which is the windage and the stream on her — the same numbers the
//!   sim is already working out, taken at the bow where the chain is.
//!
//! What falls out of those three without being written down separately: she lies head to
//! the resultant of wind and tide rather than to the wind; she sheers about on her chain
//! the way a boat at anchor does; a gust that exceeds the holding drags her *slowly*,
//! because an anchor dragging is a plough and not a release; and veering more chain in
//! the middle of the night is a thing that works.

use glam::DVec2;

/// What the ground is made of where she let go.
///
/// **Four bottoms, and the numbers are from the anchor tests every cruising association
/// has run.** A modern anchor in firm sand holds something like twenty times its own
/// weight; the same anchor in soft mud holds four or five, and in weed it holds whatever
/// it happens to catch on. The spread is what makes *where* you let go a decision.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Ground {
    /// Firm sand: what every anchor is tested in and the best holding there is.
    Sand,
    /// Mud: holds well once it is in, and it takes a while to get in.
    Mud,
    /// Weed over hard ground: it skates until it finds a bare patch.
    Weed,
    /// Rock and shingle: it either hooks something or it does not.
    Rock,
}

impl Ground {
    /// How many times the anchor's own weight this will hold, at full scope.
    pub fn holds(self) -> f64 {
        match self {
            Ground::Sand => 20.0,
            Ground::Mud => 9.0,
            Ground::Weed => 4.0,
            Ground::Rock => 6.0,
        }
    }

    /// How long it takes to dig in, in metres of dragging, before it is at full hold.
    ///
    /// **An anchor is not set when it reaches the bottom; it is set when it has been
    /// pulled through the ground far enough to bury itself.** That is why you let go,
    /// veer the chain, and then go astern on it — and why the first minute at anchor is
    /// the one worth watching. Mud takes the longest and holds well afterwards.
    pub fn sets_in(self) -> f64 {
        match self {
            Ground::Sand => 6.0,
            Ground::Mud => 14.0,
            Ground::Weed => 10.0,
            Ground::Rock => 2.0,
        }
    }

    /// How much of a metre of ploughing buries it, as a fraction.
    ///
    /// **Dragging is how an anchor sets, in ground it can get into.** Let go in sand and
    /// load her up and she walks a few metres while the fluke digs itself down, and then
    /// she stops — which is the manoeuvre every cruising book describes as *setting* the
    /// anchor and which a model that only ever buried it while it was *holding* cannot
    /// do. Weed is the opposite and is the whole reason it is feared: the anchor skates
    /// over the top of it, never gets below it, and goes on skating all night.
    pub fn buries(self) -> f64 {
        match self {
            Ground::Sand => 1.0,
            Ground::Mud => 0.8,
            Ground::Weed => 0.1,
            Ground::Rock => 0.2,
        }
    }

    /// What it is called on a chart, in the abbreviations a chart uses.
    pub fn charted(self) -> &'static str {
        match self {
            Ground::Sand => "S",
            Ground::Mud => "M",
            Ground::Weed => "Wd",
            Ground::Rock => "R",
        }
    }

    /// The bottom at a place, drawn once per world and the same every visit.
    ///
    /// **A chart tells you what the ground is before you get there**, and a bottom that
    /// was rolled when you let go would make that impossible. It comes out of the
    /// position, like everything else in this archipelago: the same bay has the same
    /// ground next week, you learn it, and that is what local knowledge *is*.
    pub fn at(place: DVec2) -> Self {
        // Coarse patches, a few hundred metres across — one anchorage is one kind of
        // ground, and the bay round the headland may be another.
        let cell = |v: f64| (v / 400.0).floor() as i64;
        let mut n = (cell(place.x) as u64) << 32 ^ cell(place.y) as u64 ^ 0xA1_1C_0D;
        n ^= n >> 33;
        n = n.wrapping_mul(0xff51_afd7_ed55_8ccd);
        n ^= n >> 29;
        match n % 100 {
            0..=52 => Ground::Sand,
            53..=79 => Ground::Mud,
            80..=92 => Ground::Weed,
            _ => Ground::Rock,
        }
    }
}

/// The ground tackle: an anchor of a weight, on a chain of a length.
///
/// A thirty-five foot cruising boat carries about a fifteen-kilo anchor and fifty metres
/// of eight-millimetre chain, which is what this is.
#[derive(Clone, Copy, Debug)]
pub struct Tackle {
    /// The anchor's weight, kilograms.
    pub weight: f64,
    /// All the chain there is, metres. You cannot veer more than you have.
    pub chain: f64,
}

impl Default for Tackle {
    fn default() -> Self {
        Self {
            weight: 15.0,
            chain: 50.0,
        }
    }
}

/// Where the anchor is and what it is doing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Anchor {
    /// On the bow roller.
    #[default]
    Stowed,
    /// On the bottom, with chain out.
    Down(Down),
}

/// An anchor that is down: where it lies, what it is lying in, and how it is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Down {
    /// Where it lies on the bottom. It moves when she drags, which is the whole point of
    /// keeping it as a position rather than as a flag.
    pub at: DVec2,
    /// The water it is in, metres, measured when it went down.
    pub depth: f64,
    /// Chain veered, metres.
    pub out: f64,
    /// How far it has been pulled through the ground, metres. Up to `Ground::sets_in` it
    /// is still digging in and holds proportionally less.
    pub dug: f64,
    /// What it is lying in.
    pub ground: Ground,
    /// How far it has walked since it was let go — the number an anchor watch is kept
    /// for, and the one a drag alarm reads.
    pub dragged: f64,
}

/// How much chain is worth veering, as a multiple of the depth.
///
/// Five to one is the figure in every almanac for chain, and it is where the holding
/// curve below has flattened out. The deck can veer more and it buys very little; it can
/// veer less and that is where the trouble is.
pub const GOOD_SCOPE: f64 = 5.0;

/// What the chain is worth at a given scope, as a fraction of the ground's full hold.
///
/// **The curve, and why it has that shape.** What holds an anchor is the pull arriving
/// along the bottom; what keeps it along the bottom is the catenary — the weight of
/// chain lying on the ground between the anchor and the point where it lifts. At three
/// to one in any sea that catenary straightens and the pull comes on upwards; at five to
/// one there is enough chain down that the last of it stays flat. Below two and a half
/// she is not anchored, she is parked.
pub fn for_scope(scope: f64) -> f64 {
    if scope <= 2.5 {
        // The shank lifts and the anchor walks. Not nothing — the chain's own weight is
        // doing something — but not anchoring.
        return (scope / 2.5).clamp(0.0, 1.0) * 0.18;
    }
    let past = ((scope - 2.5) / (GOOD_SCOPE - 2.5)).clamp(0.0, 1.0);
    // Rises steeply out of the parked case and flattens at the almanac's five to one.
    0.18 + 0.82 * past.powf(0.7)
}

impl Down {
    /// The scope she is lying at: chain out over the water under the anchor.
    ///
    /// The depth includes the height of the bow roller above the water, because the
    /// chain starts there and not at the surface — a metre and a half on a boat this
    /// size, which matters in three metres of water and does not in thirty.
    pub fn scope(&self) -> f64 {
        self.out / (self.depth + BOW_ROLLER).max(0.5)
    }

    /// What this anchor will hold before it starts to walk, newtons.
    ///
    /// Weight times what the ground gives times what the scope is worth times how far it
    /// has dug in — four numbers, each of which is a decision somebody made.
    pub fn holding(&self, tackle: Tackle) -> f64 {
        let dug = (self.dug / self.ground.sets_in()).clamp(0.0, 1.0);
        // Not from nothing: an anchor on the bottom with chain on it holds *something*
        // the moment it lands, which is why a boat that lets go and lies back gently
        // often never drags at all.
        let set = 0.25 + 0.75 * dug;
        tackle.weight * crate::units::GRAVITY * self.ground.holds() * for_scope(self.scope()) * set
    }
}

/// How high the bow roller is above the water, metres.
const BOW_ROLLER: f64 = 1.4;

/// The pull the chain puts on her, and what that pull does to the anchor.
///
/// Given where the bow is and what is pulling her away — the wind and the stream the sim
/// has already worked out — this answers the two questions a boat at anchor asks: *how
/// hard is the chain pulling me back*, and *is the anchor still where I put it*.
///
/// **The chain is a spring with a dead band.** Inside the length that is out, it hangs
/// in a curve and pulls almost nothing; as she comes up to the end of it the curve
/// straightens and the pull comes on hard, which is exactly the snatch that wakes
/// everybody up. Modelled as a stiffness that rises steeply through the last tenth of
/// the chain, because the catenary's geometry is what does that and a linear spring
/// would have her tethered to a rubber band.
pub fn pull(bow: DVec2, down: &Down, tackle: Tackle, boat_velocity: DVec2) -> DVec2 {
    let along = down.at - bow;
    let range = along.length();
    if range < 1e-3 {
        return DVec2::ZERO;
    }
    let to_anchor = along / range;
    // The horizontal distance the chain can reach while still leaving some of it on the
    // bottom. Pythagoras on the straight part: what is left after the depth is taken up.
    let depth = (down.depth + BOW_ROLLER).max(0.5);
    let reach = (down.out * down.out - depth * depth).max(0.0).sqrt();
    if range <= reach * 0.92 {
        // Slack: the chain is a curve on the bottom and she is riding to her own
        // windage, which is the state a boat at anchor spends most of its night in.
        return DVec2::ZERO;
    }
    // How far past the slack she has come, as a fraction of the last tenth.
    let over = (range - reach * 0.92) / (reach * 0.08).max(0.5);
    // Twelve tonnes a metre at the end of it, which is chain rather than rope and is
    // what makes a snatch a snatch.
    const STIFFNESS: f64 = 1.2e4;
    let stretch = over.min(3.0) * (reach * 0.08).max(0.5);
    let tension = STIFFNESS * stretch;
    // And a damper, because the water does that and because a spring with no damping in
    // a numerical integrator is an oscillator that never settles.
    let closing = boat_velocity.dot(-to_anchor).max(0.0);
    let damped = tension + 1.4e3 * closing * tackle.weight / 15.0;
    to_anchor * damped
}

/// Work the anchor through one step: what it holds, whether it is dragging, and how far
/// it has moved.
///
/// Returns the pull on the bow. The anchor itself is updated in place — it digs in while
/// the pull is under what it will hold, and ploughs down-pull when it is over.
pub fn step(down: &mut Down, tackle: Tackle, bow: DVec2, boat_velocity: DVec2, dt: f64) -> DVec2 {
    let force = pull(bow, down, tackle, boat_velocity);
    let size = force.length();
    if size <= 0.0 {
        return DVec2::ZERO;
    }
    let holding = down.holding(tackle);
    if size <= holding {
        // Holding, and burying itself a little further every time it is loaded. This is
        // why going astern on it after letting go is a real thing to do: the load is
        // what sets it.
        down.dug += (size / holding) * dt * 0.4;
        return force;
    }
    // **Dragging, which is a plough and not a release.** The anchor walks down the pull
    // at a speed that goes with how far over the holding the load is — a boat that is a
    // little overloaded creeps, one that is caught by a squall under short scope goes
    // off across the anchorage at a knot. And it keeps holding most of what it held:
    // the chain does not go slack, which is why dragging is something you notice by the
    // bearing of a light rather than by a bang.
    let over = (size - holding) / holding;
    let walk = (over * 0.45).min(0.8) * dt;
    let along = force.normalize();
    // It walks *away* from the boat, which is the direction the chain is pulling it.
    down.at -= along * walk;
    down.dragged += walk;
    // And ploughing is how it sets — in ground it can get into. Each metre of walking
    // buries it a little further in sand and mud, which is why a boat that lets go and
    // drags ten metres has *anchored*; and skates over weed, which is why one that lets
    // go in weed is still walking at four in the morning. The quarter taken off is the
    // ground it breaks on the way through: a fluke that is being pulled is never quite
    // as deep as one that is lying still.
    down.dug = (down.dug + walk * (down.ground.buries() - 0.25)).clamp(0.0, down.ground.sets_in());
    force * 0.92
}

#[cfg(test)]
mod what_holds_her {
    use super::*;
    use crate::units::GRAVITY;

    fn lying_in(ground: Ground, out: f64, depth: f64) -> Down {
        Down {
            at: DVec2::ZERO,
            depth,
            out,
            dug: ground.sets_in(),
            ground,
            dragged: 0.0,
        }
    }

    /// **Scope is most of anchoring**, and the curve has to say so: the same anchor in
    /// the same ground holds a fraction at three to one of what it holds at five, and
    /// nothing worth having at two.
    #[test]
    fn the_chain_is_what_holds_her() {
        let tackle = Tackle::default();
        let good = lying_in(Ground::Sand, 25.0, 5.0).holding(tackle);
        let short = lying_in(Ground::Sand, 15.0, 5.0).holding(tackle);
        let parked = lying_in(Ground::Sand, 10.0, 5.0).holding(tackle);
        assert!(
            short < good * 0.8,
            "three to one held {:.0} N against five to one's {:.0}",
            short,
            good
        );
        assert!(
            parked < good * 0.25,
            "a scope of two held {:.0} N, which is anchoring",
            parked
        );
        // And the good one is a real number: fifteen kilos in sand at five to one should
        // hold a boat of this size in a near gale.
        assert!(
            good > 2_000.0,
            "a well-set anchor held {good:.0} N, which would not hold a dinghy"
        );
    }

    /// **The ground is the other half**, and it is knowable before you let go.
    #[test]
    fn the_bottom_is_a_fact_about_the_place() {
        let tackle = Tackle::default();
        let sand = lying_in(Ground::Sand, 25.0, 5.0).holding(tackle);
        let weed = lying_in(Ground::Weed, 25.0, 5.0).holding(tackle);
        assert!(weed < sand * 0.4, "weed held as well as sand");
        // The same bay has the same bottom next week, which is what makes it worth
        // learning.
        let bay = DVec2::new(4_310.0, -980.0);
        assert_eq!(Ground::at(bay), Ground::at(bay));
        assert_eq!(Ground::at(bay), Ground::at(bay + DVec2::new(30.0, 20.0)));
        // And the archipelago is not all one ground.
        let kinds: std::collections::HashSet<Ground> = (0..40)
            .map(|i| Ground::at(DVec2::new(i as f64 * 613.0, i as f64 * -457.0)))
            .collect();
        assert!(kinds.len() >= 3, "the whole sea was {kinds:?}");
    }

    /// **The chain is slack until it is not.** A boat lying to her anchor in a breeze is
    /// not being pulled backwards by a spring; she is sitting on her own windage with a
    /// curve of chain on the bottom, and the pull only comes on when she reaches the end
    /// of it. Without the dead band she would be tethered and would never sheer about.
    #[test]
    fn the_chain_pulls_nothing_until_she_reaches_the_end_of_it() {
        let tackle = Tackle::default();
        let down = lying_in(Ground::Sand, 30.0, 5.0);
        // Ten metres from the anchor with thirty out: all curve, no pull.
        let near = pull(DVec2::new(10.0, 0.0), &down, tackle, DVec2::ZERO);
        assert_eq!(near, DVec2::ZERO);
        // And out at the end of it, hard.
        let far = pull(DVec2::new(29.0, 0.0), &down, tackle, DVec2::ZERO);
        assert!(
            far.length() > 1_000.0,
            "the snatch came to {:.0} N",
            far.length()
        );
        assert!(far.x < 0.0, "the chain pulled her away from the anchor");
    }

    /// What the wind alone puts on a boat of this size, newtons.
    ///
    /// Eight square metres of topsides, mast and furled sail at a drag coefficient of
    /// one, which is the figure a mooring is designed to — five hundred newtons in
    /// twenty knots, two kilonewtons in forty. The tests are written in knots because
    /// that is the unit the question comes in: *will she lie to it tonight?*
    fn windage(knots: f64) -> f64 {
        let ms = crate::units::kn_to_ms(knots);
        0.5 * 1.225 * 8.0 * ms * ms
    }

    /// A boat lying to her anchor in a wind, stepped — hull, chain and all.
    ///
    /// **The first version pinned the boat and watched the anchor**, and that is not
    /// what dragging is: the anchor walks *towards* the boat, which slackens the chain,
    /// so a boat that cannot move stops her own drag after a few centimetres. The
    /// measurement that means anything is where the *boat* ends up, which needs her to
    /// be free to go there. Six tonnes, with the water's drag on a hull being pulled
    /// backwards — the terminal speed under half a kilonewton comes out about a knot,
    /// which is what a dragging boat does.
    fn lies_to_it(
        ground: Ground,
        out: f64,
        depth: f64,
        knots: f64,
        seconds: f64,
    ) -> (f64, f64, f64) {
        let tackle = Tackle::default();
        let mut down = Down {
            dug: 0.0,
            ..lying_in(ground, out, depth)
        };
        let blow = DVec2::new(windage(knots), 0.0);
        let started = DVec2::new(out * 0.4, 0.0);
        let mut boat = started;
        let mut way = DVec2::ZERO;
        let dt = 0.1;
        let steps = (seconds / dt) as usize;
        // What she walked in the last minute of it, which is the question an anchor
        // watch is really asking: not *did she move* but *is she still moving*.
        let mut lately = 0.0;
        for n in 0..steps {
            if n == steps.saturating_sub((60.0 / dt) as usize) {
                lately = down.dragged;
            }
            let chain = step(&mut down, tackle, boat, way, dt);
            // Her own resistance going backwards: quadratic, and stiff enough that she
            // settles at about a knot under the load above.
            let drag = -way * way.length() * 1_800.0;
            let accel = (blow + chain + drag) / 6_000.0;
            way += accel * dt;
            boat += way * dt;
        }
        (
            (boat - started).length(),
            down.dragged,
            down.dragged - lately,
        )
    }

    /// **A good anchor in good ground holds a near gale**, which is the whole reason
    /// anybody sleeps at anchor — and the number says which gale.
    #[test]
    fn a_good_anchor_in_sand_lies_to_a_gale() {
        let (moved, dragged, lately) = lies_to_it(Ground::Sand, 28.0, 5.0, 40.0, 300.0);
        // **She sets herself.** A few metres of walking while the fluke digs in — which
        // is the manoeuvre, not a failure — and then nothing, which is the half that
        // matters: what an anchor watch asks is not *did she move* but *is she still
        // moving*.
        assert!(
            dragged < 15.0,
            "she walked {dragged:.0} m before she bit, which is not setting, it is dragging"
        );
        assert!(
            lately < 0.05,
            "five minutes in she was still walking at {lately:.2} m a minute"
        );
        assert!(
            moved < 30.0,
            "she ended {moved:.0} m from where she let go, which is not lying to it"
        );
    }

    /// **And short scope in weed does not hold a fresh breeze.** Same anchor, same boat,
    /// a different decision ten minutes earlier — and five minutes later she is a cable
    /// downwind of where she went to sleep.
    #[test]
    fn short_scope_in_weed_walks_in_a_breeze() {
        let (moved, dragged, lately) = lies_to_it(Ground::Weed, 12.0, 4.0, 22.0, 300.0);
        assert!(
            dragged > 20.0,
            "a scope of two in weed held twenty-two knots for five minutes"
        );
        assert!(
            moved > 20.0,
            "the anchor walked {dragged:.0} m and the boat stayed put"
        );
        // A plough and not a bolt: she creeps, at something under a knot.
        assert!(
            moved < 300.0,
            "she ran {moved:.0} m in five minutes, which is sailing"
        );
        // And still going, which is the thing about weed.
        assert!(
            lately > 2.0,
            "she walked and then stopped in weed, which weed does not do"
        );
    }

    /// **An anchor sets by being pulled.** Letting go and lying to it does most of it;
    /// the first load is what buries it. So a boat that has just let go holds less than
    /// the same boat ten minutes later, and that is the difference between a lunch stop
    /// and a night.
    #[test]
    fn it_digs_itself_in_under_load() {
        let tackle = Tackle::default();
        let mut fresh = Down {
            dug: 0.0,
            ..lying_in(Ground::Sand, 28.0, 5.0)
        };
        let cold = fresh.holding(tackle);
        for _ in 0..1_200 {
            step(&mut fresh, tackle, DVec2::new(27.0, 0.0), DVec2::ZERO, 0.1);
        }
        let set = fresh.holding(tackle);
        assert!(
            set > cold * 2.0,
            "two minutes of lying to it took the hold from {cold:.0} N to {set:.0}"
        );
    }

    /// The tackle is a boat's tackle: an anchor somebody can lift and chain somebody can
    /// veer, and enough of it for the water this game is sailed in.
    #[test]
    fn she_carries_what_a_boat_this_size_carries() {
        let tackle = Tackle::default();
        assert!(tackle.weight > 8.0 && tackle.weight < 30.0);
        // Five to one in ten metres is fifty, which is why fifty is what is aboard.
        assert!(tackle.chain >= GOOD_SCOPE * 10.0);
        let _ = GRAVITY;
    }
}
