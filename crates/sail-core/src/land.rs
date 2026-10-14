//! The archipelago: where the land is, how deep it is beside it, and what it does to
//! the wind.
//!
//! Islands are not scenery in a sailing game. They are the whole of what makes
//! sailing among them different from sailing in open water, and all of it comes from
//! three things:
//!
//! * **They stop the water being deep.** Running aground has to be possible or the
//!   land is a painted backdrop, and the shoal has to reach further out than the beach
//!   does, because that is how it catches people.
//! * **They stop the wind.** Behind an island the breeze dies, lifts, heads and comes
//!   back in gusts, and reading that is most of the skill of coastal sailing. It is
//!   also the one thing an open-sea simulator can never teach.
//! * **They stop the sea.** Waves need a fetch to build, and the fetch to windward of
//!   a boat lying under a lee shore is the distance to that shore. This model already
//!   takes fetch as its parameter, so sheltered water is not a special case here —
//!   it is the same equation with a smaller number in it.
//!
//! Generated rather than authored, from a seed, like the weather and the sea and the
//! sky. A hand-drawn archipelago is a better archipelago and a worse *rule*; this one
//! can be regenerated, swept over, and measured, which is what the rest of the project
//! is built to do.

use crate::rng::Pcg32;
use crate::units::wrap_tau;
use glam::DVec2;

/// Depth of open water, metres. Not a real sounding — deep enough that the keel never
/// knows about it, which is all the model needs away from land.
pub const OPEN_WATER: f64 = 40.0;

/// How far out from the shore the bottom shelves up, metres.
///
/// Four hundred metres, and the first attempt at eighty was measured and thrown away:
/// it put the two-metre contour fifteen metres off the beach, so the only way to run
/// aground was to sail into the rocks, and a shoal you cannot be caught by is not a
/// hazard, it is a wall.
///
/// The interesting version is the one you are caught by while still well off the land,
/// watching the sounder come up and deciding whether to tack. That needs a bank wide
/// enough to be a decision, which on a real coast it always is.
const SHELF: f64 = 400.0;

/// One island: a lobed blob rather than a circle.
///
/// The outline is a mean radius with three harmonics on it. Three is enough to read as
/// a coastline with headlands and bays, and few enough that the distance to the shore
/// stays cheap to evaluate — which matters, because the wind shadow asks for it along
/// a ray several times per substep.
#[derive(Clone, Copy, Debug)]
pub struct Island {
    pub centre: DVec2,
    /// Mean radius, metres.
    pub radius: f64,
    /// How high it stands, metres. Sets the length of its wind shadow.
    pub height: f64,
    lobes: [f64; 3],
    phases: [f64; 3],
    /// Seed for this island's own relief, so two islands of the same size are not the
    /// same hill.
    relief_seed: u64,
}

/// How far in from the shore the cliff face reaches, as a share of the mean radius.
const CLIFF_SHARE: f64 = 0.20;
/// And how much of the island's height it climbs in that distance.
const CLIFF_RISE: f64 = 0.62;
/// Size of a ridge, metres. A few hundred: smaller and the island is corrugated,
/// larger and a whole island is one slope.
const RIDGE_SCALE: f64 = 260.0;
/// The lowest the relief may take the ground, as a share of the mean profile. Valleys,
/// not canyons — a valley that cuts to nothing would flood.
const RELIEF_FLOOR: f64 = 0.55;
/// How far inland the relief is held back, as a share of the mean radius. The coastline
/// belongs to the distance field and the relief may not move it.
const SHORE_HOLD: f64 = 0.10;

impl Island {
    /// Distance from the centre to the shore on a given bearing, metres.
    pub fn shore_radius(&self, bearing: f64) -> f64 {
        let mut r = 1.0;
        for (i, (amp, phase)) in self.lobes.iter().zip(&self.phases).enumerate() {
            let harmonic = (i + 2) as f64;
            r += amp * (harmonic * bearing + phase).sin();
        }
        self.radius * r.max(0.25)
    }

    /// Height of the land above the water at a point, metres. Zero at the shore and
    /// anywhere seaward of it.
    ///
    /// **The one definition of how high the ground is**, and it had to become one
    /// because there were three. `depth` carried `-height * sqrt(inland)`, a dome; the
    /// renderer carried a cliff-and-roll curve with two harmonics of bearing on it; and
    /// the wind shadow keyed off `height` alone. A boat could therefore sound six metres
    /// over ground the eye was looking at the top of, and the relief the renderer drew
    /// existed nowhere else — which is also why it read as a dish-cover: a function of
    /// *bearing* scales the whole radial line together, so an island could have lobes
    /// but never a ridge running inland, never a valley, never two summits.
    ///
    /// Three parts, in the order they matter:
    ///
    /// * **The profile.** How the ground climbs from the shore. Not a dome: these are
    ///   basalt islands the Atlantic has been taking the edge off for ten thousand
    ///   years, so the first fifth of the way inland does three fifths of the climbing
    ///   and the rest is a long roll to the top. That is a cliff from a mile off, which
    ///   is the distance you sail past them at.
    /// * **The ridges**, from a ridged noise field in *world* coordinates — so they run
    ///   inland, fork, and die out, which is what erosion leaves. This is the part that
    ///   could not exist before.
    /// * **A taper to nothing at the water**, because the coastline belongs to the
    ///   signed distance field and nothing here may move it. Relief that reaches the
    ///   shore would lift the land out of the water in some places and drown it in
    ///   others, and the depth sounder and the drawn coast would part company again.
    pub fn elevation(&self, pos: DVec2) -> f64 {
        let d = self.signed_distance(pos);
        if d >= 0.0 {
            return 0.0;
        }
        let inland = (-d / self.radius.max(1.0)).clamp(0.0, 1.0);
        // The climb. Steep at the shore, easing off inland.
        let profile = if inland < CLIFF_SHARE {
            let e = inland / CLIFF_SHARE;
            CLIFF_RISE * e * e * (3.0 - 2.0 * e)
        } else {
            CLIFF_RISE
                + (1.0 - CLIFF_RISE) * ((inland - CLIFF_SHARE) / (1.0 - CLIFF_SHARE)).powf(0.75)
        };
        // The relief, in world metres so it does not stretch with the island's size: a
        // ridge is a few hundred metres across whether it is on a skerry or on a
        // headland, and scaling it to the island would make every island the same
        // picture at a different zoom.
        let ridge = crate::noise::ridged(pos / RIDGE_SCALE, self.relief_seed);
        let relief = RELIEF_FLOOR + (1.0 - RELIEF_FLOOR) * ridge;
        // Held down at the water's edge, released inland.
        let shore = crate::units::smoothstep(0.0, SHORE_HOLD, inland);
        self.height * profile * (1.0 - shore + shore * relief)
    }

    /// Metres to the shore, negative inside.
    ///
    /// Approximate, and the approximation is named: for a star-shaped outline the true
    /// distance to the boundary is a minimisation along it, and this measures along
    /// the radius instead. The two agree exactly on a circle and differ by a few per
    /// cent where the coast is steeply curved — which is a bigger error than it sounds
    /// only if something downstream needs a true distance field. Nothing here does:
    /// the depth wants a smooth shoal and the wind shadow wants a ray hit.
    pub fn signed_distance(&self, pos: DVec2) -> f64 {
        let d = pos - self.centre;
        let range = d.length();
        if range < 1e-6 {
            return -self.radius;
        }
        range - self.shore_radius(wrap_tau(d.x.atan2(d.y)))
    }
}

/// A scatter of islands, and the water between them.
pub struct Archipelago {
    islands: Vec<Island>,
}

impl Archipelago {
    /// Scatter islands over a square of the given half-width, metres.
    ///
    /// Rejection-sampled so that no two overlap and none is closer to another than a
    /// navigable channel. A generator that is allowed to merge islands produces one
    /// continent and a lot of dead ends; keeping a gap is what produces an archipelago
    /// you can sail *through* rather than around.
    pub fn generate(seed: u64, half_width: f64, count: usize) -> Self {
        Self::generate_around(seed, half_width, count, 0.0)
    }

    /// The same, with a patch of open water kept clear at the origin.
    ///
    /// The boat starts at the origin, and a generator that is free to put a skerry
    /// there starts her aground about one run in twenty — which is not a hard sailing
    /// problem, it is a broken game. The clearing is also where a harbour would go.
    pub fn generate_around(seed: u64, half_width: f64, count: usize, clear_radius: f64) -> Self {
        /// Narrowest channel the generator will leave between two islands, metres.
        const CHANNEL: f64 = 420.0;
        let mut rng = Pcg32::new(seed);
        let mut islands: Vec<Island> = Vec::with_capacity(count);
        let mut attempts = 0;
        while islands.len() < count && attempts < count * 200 {
            attempts += 1;
            let centre = DVec2::new(
                (rng.next_f64() * 2.0 - 1.0) * half_width,
                (rng.next_f64() * 2.0 - 1.0) * half_width,
            );
            // A spread of sizes: mostly skerries, a few proper islands to hide behind.
            let t = rng.next_f64();
            let radius = 90.0 + t * t * t * 1500.0;
            let candidate = Island {
                centre,
                radius,
                // Low islands shelter badly, which is true and is why a fifty-metre
                // hill matters more to a sailor than a mile of sand.
                height: 8.0 + radius * (0.03 + 0.05 * rng.next_f64()),
                lobes: [
                    (rng.next_f64() - 0.5) * 0.34,
                    (rng.next_f64() - 0.5) * 0.22,
                    (rng.next_f64() - 0.5) * 0.14,
                ],
                phases: [
                    rng.next_f64() * std::f64::consts::TAU,
                    rng.next_f64() * std::f64::consts::TAU,
                    rng.next_f64() * std::f64::consts::TAU,
                ],
                relief_seed: rng.next_u32() as u64 | ((rng.next_u32() as u64) << 32),
            };
            if candidate.centre.length() < clear_radius + radius {
                continue;
            }
            let clear = islands.iter().all(|other| {
                (other.centre - candidate.centre).length()
                    > other.radius * 1.35 + candidate.radius * 1.35 + CHANNEL
            });
            if clear {
                islands.push(candidate);
            }
        }
        Self { islands }
    }

    /// No land at all, for the validation suite and the open-water polar.
    pub fn empty() -> Self {
        Self {
            islands: Vec::new(),
        }
    }

    pub fn islands(&self) -> &[Island] {
        &self.islands
    }

    /// Metres to the nearest shore, negative on land.
    pub fn signed_distance(&self, pos: DVec2) -> f64 {
        self.islands
            .iter()
            .map(|i| i.signed_distance(pos))
            .fold(f64::INFINITY, f64::min)
    }

    /// Depth of water, metres. Negative is dry land.
    ///
    /// The shelf is a smoothstep rather than a ramp so the bottom has no crease in it,
    /// which matters because a depth sounder reading is something a player watches
    /// continuously and a kink in it is visible.
    pub fn depth(&self, pos: DVec2) -> f64 {
        let d = self.signed_distance(pos);
        if d <= 0.0 {
            // Inland, and the height comes from the island's own `elevation` — the same
            // function the land is drawn from. It used to be a separate square-root dome
            // here, which meant the sounder and the eye could disagree about the shape
            // of the ground they were both looking at.
            let island = self
                .islands
                .iter()
                .min_by(|a, b| {
                    a.signed_distance(pos)
                        .partial_cmp(&b.signed_distance(pos))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .copied();
            return -island.map(|i| i.elevation(pos)).unwrap_or(10.0);
        }
        // Steep close in, flattening out to the open depth: a real bank, not a ramp.
        let t = (d / SHELF).min(1.0);
        OPEN_WATER * t.sqrt() * t.sqrt() * t
    }

    /// Is a boat of this draft on the bottom?
    pub fn aground(&self, pos: DVec2, draft: f64) -> bool {
        self.depth(pos) < draft
    }

    /// Distance to the first land upwind, metres, capped at `open`.
    ///
    /// Sphere-traced: step along the ray by whatever the distance field says is safe,
    /// which converges in a handful of steps for a clear ray and lands exactly on the
    /// shore for one that hits. The alternative — fixed steps — either misses thin
    /// islands or costs ten times as much.
    pub fn fetch(&self, pos: DVec2, from_direction: f64, open: f64) -> f64 {
        if self.islands.is_empty() {
            return open;
        }
        let step = DVec2::new(from_direction.sin(), from_direction.cos());
        let mut travelled = 0.0;
        for _ in 0..64 {
            let d = self.signed_distance(pos + step * travelled);
            if d < 5.0 {
                return travelled.min(open);
            }
            travelled += d.max(20.0);
            if travelled >= open {
                return open;
            }
        }
        open
    }

    /// How much of the open-water wind reaches this spot: 1 in clear air, less in a lee.
    ///
    /// A wind shadow is not a cone with an edge. Air spills round an island and down
    /// over it, so the lee is a soft, gusty, lifting mess that fills in over a distance
    /// of several times the island's *height* — not its width, which is why a long low
    /// sandbank barely shelters anything and a small steep one shelters a lot.
    ///
    /// Three rays rather than one, fanned either side of the wind. A single ray gives
    /// a shadow with a hard edge you can sail across in one boat length; averaging a
    /// fan softens it into something you feel arriving, which is what it is.
    pub fn shelter(&self, pos: DVec2, wind_from: f64) -> f64 {
        if self.islands.is_empty() {
            return 1.0;
        }
        /// How many island heights downwind the shadow takes to fill in.
        const RECOVERY: f64 = 14.0;
        /// How far upwind to bother looking, metres.
        const REACH: f64 = 3000.0;
        let mut total = 0.0;
        let fan = [-0.10, 0.0, 0.10];
        for offset in fan {
            let bearing = wind_from + offset;
            let hit = self.fetch(pos, bearing, REACH);
            total += if hit >= REACH {
                1.0
            } else {
                // Whose shadow is it? The island the ray stopped at.
                let at = pos + DVec2::new(bearing.sin(), bearing.cos()) * hit;
                let height = self
                    .islands
                    .iter()
                    .min_by(|a, b| {
                        a.signed_distance(at)
                            .partial_cmp(&b.signed_distance(at))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|i| i.height)
                    .unwrap_or(10.0);
                1.0 - (-hit / (RECOVERY * height.max(1.0))).exp()
            };
        }
        (total / fan.len() as f64).clamp(0.0, 1.0)
    }
}

/// A wind field with the land in it.
///
/// Wraps any other field and takes out of it what the islands take out of the real
/// thing: strength in the lee, and direction too, because air that spills round an
/// island arrives bent. Both are read from the same shelter figure, so they cannot
/// disagree with each other or with the water's fetch.
///
/// A decorator rather than a new field, so every wind the game can produce — steady
/// for the polar, synthetic with its gusts and shifts for sailing — gets a coastline
/// for free and the two concerns stay separable.
pub struct ShelteredWind {
    inner: Box<dyn crate::wind::WindField>,
    land: std::sync::Arc<Archipelago>,
}

impl ShelteredWind {
    pub fn new(inner: Box<dyn crate::wind::WindField>, land: std::sync::Arc<Archipelago>) -> Self {
        Self { inner, land }
    }
}

impl crate::wind::WindField for ShelteredWind {
    /// Forwarded, and reduced by the shelter: in the lee of an island there is less
    /// wind, and the gusts that do get there are what is left of the ones outside.
    fn gust_at(&self, pos: DVec2, t: f64) -> f64 {
        let from = crate::wind::direction_of(self.inner.sample(pos, t));
        self.inner.gust_at(pos, t) * (0.2 + 0.8 * self.land.shelter(pos, from))
    }

    fn outlook(&self) -> (f64, f64) {
        self.inner.outlook()
    }

    fn squalls(&self) -> Vec<(DVec2, f64, f64)> {
        self.inner.squalls()
    }

    fn cover(&self) -> f64 {
        self.inner.cover()
    }

    fn adjustable(&mut self) -> Option<&mut crate::wind::SyntheticWind> {
        self.inner.adjustable()
    }

    fn squall_at(&self, pos: DVec2) -> f64 {
        // A squall is a column of air a kilometre and more across coming down out of a
        // cloud. An island does not shelter you from one; it only takes the gradient
        // wind out from underneath it.
        self.inner.squall_at(pos)
    }

    fn sample(&self, pos: DVec2, t: f64) -> DVec2 {
        let flow = self.inner.sample(pos, t);
        let speed = flow.length();
        if speed < 1e-6 {
            return flow;
        }
        let from = crate::wind::direction_of(flow);
        let shelter = self.land.shelter(pos, from);

        // Strength first. A lee never goes completely still — air pours down over the
        // top and round the ends — so the floor is a fifth rather than nothing, which
        // is also what stops a boat being permanently trapped by sailing too close.
        let reduced = 0.2 + 0.8 * shelter;

        // And then the bend. Air going round an island leaves the lee turned towards
        // the shadow, which is why the breeze behind a headland comes in off the land
        // rather than along the true wind. The sign is taken from which side has more
        // shelter, measured rather than assumed, so it bends the right way whichever
        // side you pass on.
        const BEND: f64 = 0.35;
        let probe = 0.22;
        let left = self.land.shelter(pos, from - probe);
        let right = self.land.shelter(pos, from + probe);
        let bend = (right - left) * BEND * (1.0 - shelter);

        crate::wind::flow_from(wrap_tau(from + bend), speed * reduced)
    }

    fn step(&mut self, dt: f64) {
        self.inner.step(dt);
    }
}

/// Somewhere to deliver to.
///
/// Not a built harbour — an anchorage, which is what most of an archipelago has. The
/// siting is the same judgement a skipper makes off a chart: find the deepest bite out
/// of the coast, because an indented shore is one with land either side of it, and
/// land either side of you is shelter from everything except the one direction the bay
/// opens to.
#[derive(Clone, Debug)]
pub struct Harbour {
    pub at: DVec2,
    pub name: String,
    /// The bearing the bay opens to — the one direction it gives no shelter from.
    pub exposed_to: f64,
}

/// How far off the beach the anchorage lies, metres.
///
/// Far enough to float: the bank gives about five and a half metres here, which is an
/// anchorage rather than a stranding, and it is still close enough that arriving means
/// threading the shoal rather than stopping somewhere near an island.
const ANCHORAGE_OFF: f64 = 150.0;

impl Island {
    /// The deepest bay on this coast, as a bearing from the centre.
    fn best_bay(&self) -> f64 {
        let mut best = (0.0, f64::INFINITY);
        for i in 0..180 {
            let bearing = i as f64 / 180.0 * std::f64::consts::TAU;
            let r = self.shore_radius(bearing);
            if r < best.1 {
                best = (bearing, r);
            }
        }
        best.0
    }

    /// Where a boat would lie off this island.
    pub fn anchorage(&self) -> DVec2 {
        let bearing = self.best_bay();
        let out = self.shore_radius(bearing) + ANCHORAGE_OFF;
        self.centre + DVec2::new(bearing.sin(), bearing.cos()) * out
    }
}

/// Something you can take a bearing on.
///
/// The whole of terrestrial navigation is this: a thing you can see, that is drawn on the
/// chart, that you can name. Without marks a chart is a picture of some islands; with
/// them it is an instrument, because two bearings and a straight edge put you somewhere
/// and three put you somewhere and tell you how well you did it.
///
/// They are derived from the islands rather than scattered separately, for the same
/// reason the harbours are: the archipelago is a pure function of the seed, so a passage
/// can be sailed twice and the mark that was on the headland is still on the headland.
#[derive(Clone, Debug, PartialEq)]
pub struct Landmark {
    pub at: DVec2,
    pub name: String,
    pub kind: Mark,
    /// How high the ground under it stands, metres. A tower is built on what is there.
    pub ground: f64,
    /// And how high its light or its top is above the *sea*, which is the number a pilot
    /// book gives and the one that decides how far off it can be seen. The two together
    /// are what the structure's own height is, which is what gets drawn.
    pub height: f64,
}

/// What kind of thing it is, which is also how it is drawn on a chart and recognised
/// through a pair of binoculars.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// A lighthouse on an outer skerry: the one that is there because the rock is
    /// dangerous, which is why it is where it is and why it is the tallest thing about.
    Lighthouse,
    /// A church spire over a settlement. The oldest navigation mark there is, and still
    /// the one every pilot book leads with, because a village builds the tallest thing
    /// it can afford and then paints it white.
    Spire,
    /// A beacon: a pole on a rock with a topmark, unlit, which exists only to be seen.
    Beacon,
    /// A cairn on a summit. Stones piled by somebody who wanted to be found again.
    Cairn,
}

impl Mark {
    /// How it is written on a chart.
    pub fn abbreviation(&self) -> &'static str {
        match self {
            Mark::Lighthouse => "Lt",
            Mark::Spire => "Spire",
            Mark::Beacon => "Bn",
            Mark::Cairn => "Cairn",
        }
    }

    /// Whether it shows a light after dark, and so whether it is any use at night.
    pub fn lit(&self) -> bool {
        matches!(self, Mark::Lighthouse)
    }
}

impl Landmark {
    /// How far off it can be seen, metres, from an eye this high.
    ///
    /// The geometric range, which is the one a pilot book gives: the horizon from the
    /// mark plus the horizon from the eye. Two point zero eight times the square root of
    /// the height in metres gives nautical miles, and the rest is arithmetic.
    ///
    /// It is not clairvoyance about weather — rain and haze cut it and the game cuts the
    /// visibility separately — it is the curve of the earth, which nothing cuts.
    pub fn visible_from(&self, eye: f64) -> f64 {
        const NM: f64 = 1_852.0;
        2.08 * (self.height.max(0.0).sqrt() + eye.max(0.0).sqrt()) * NM
    }
}

/// Place names, assembled rather than listed.
///
/// Norse, because these islands are shaped like Norse ones and because a delivery run
/// reads as a voyage when its ends have names and as a test harness when they do not.
/// Deterministic in the island's own position, so the same archipelago always has the
/// same place names and a run can be described to somebody else.
fn place_name(seed: u64) -> String {
    const HEAD: [&str; 16] = [
        "Haug", "Vatn", "Skarv", "Grim", "Brei", "Nord", "Sand", "Fugl", "Stein", "Hval", "Kirk",
        "Mel", "Ulv", "Orm", "Sel", "Berg",
    ];
    const TAIL: [&str; 10] = [
        "vik", "sund", "nes", "vog", "holm", "ey", "stad", "havn", "voe", "skerry",
    ];
    let mut rng = Pcg32::new(seed);
    let head = HEAD[(rng.next_u32() as usize) % HEAD.len()];
    let tail = TAIL[(rng.next_u32() as usize) % TAIL.len()];
    format!("{head}{tail}")
}

impl Archipelago {
    /// Everywhere worth delivering to.
    ///
    /// Only islands big enough to have somebody living on them. A skerry is a hazard,
    /// not a destination, and a run between two rocks a hundred metres apart is not a
    /// passage.
    /// The marks, one to an island that is worth marking.
    ///
    /// Which kind goes where is not decoration — it is the rule a coast is actually
    /// built by, and following it means the chart teaches something true:
    ///
    /// * **Lighthouses go on the outermost dangerous rocks**, because that is what a
    ///   light is for. A skerry in the middle of an archipelago with islands all round
    ///   it drowns in the clutter; the one on the edge, that a boat meets first and in
    ///   the dark, is the one worth lighting.
    /// * **Spires stand over settlements**, which on these islands means the ones big
    ///   enough to have a harbour. A village builds the tallest thing it can afford and
    ///   whitewashes it, and every pilot book in the world leads with it.
    /// * **Beacons go on middling rocks** near a channel: unlit, cheap, and there only
    ///   to be seen.
    /// * **Cairns go on summits** of anything else big enough to have one.
    pub fn landmarks(&self) -> Vec<Landmark> {
        /// A harbour's island is settled, and a settled island has a church.
        const SETTLED: f64 = 240.0;
        /// Below this an island is a rock rather than a place.
        const WORTH_MARKING: f64 = 70.0;
        // How far out a rock has to be to be an outer one: beyond this from the middle
        // of the whole scatter, where a boat meets it before it meets anything else.
        let furthest = self
            .islands
            .iter()
            .map(|i| i.centre.length())
            .fold(0.0_f64, f64::max);
        self.islands
            .iter()
            .enumerate()
            .filter(|(_, i)| i.radius >= WORTH_MARKING)
            .map(|(index, i)| {
                let seed = (i.centre.x.abs() as u64).wrapping_mul(0x9E37_79B9)
                    ^ (i.centre.y.abs() as u64).wrapping_mul(0x85EB_CA6B)
                    ^ (index as u64).wrapping_mul(0xC2B2_AE35);
                let outer = i.centre.length() > furthest * 0.66;
                let kind = if i.radius >= SETTLED {
                    Mark::Spire
                } else if outer {
                    Mark::Lighthouse
                } else if i.radius >= 140.0 {
                    Mark::Beacon
                } else {
                    Mark::Cairn
                };
                // Where on the island it stands. A spire is in the village, which is
                // over the anchorage; everything else is on the summit, which is the
                // middle, because that is where you can see it from and see from.
                let at = if kind == Mark::Spire {
                    let bay = i.anchorage();
                    i.centre + (bay - i.centre) * 0.55
                } else {
                    i.centre
                };
                let ground = i.elevation(at);
                // How tall the thing itself is. The light is on the rock and the rock is
                // low, so its tower is the tallest of them — which is not a flourish,
                // it is why lighthouses look like they do.
                let structure = match kind {
                    Mark::Spire => 22.0,
                    Mark::Lighthouse => 26.0,
                    Mark::Beacon => 7.0,
                    Mark::Cairn => 3.0,
                };
                Landmark {
                    at,
                    name: place_name(seed ^ 0x5BF0_3635),
                    kind,
                    ground,
                    height: ground + structure,
                }
            })
            .collect()
    }

    pub fn harbours(&self) -> Vec<Harbour> {
        // Two hundred and forty metres rather than three hundred and twenty. A smaller
        // archipelago has fewer islands over any given size, and the harbours are what
        // the deliveries are *between* — six of them on a coast is a shuttle service.
        // This is still comfortably above a skerry: a quarter-kilometre island has a
        // beach, a bay to lie in and somewhere for somebody to live.
        const SETTLED: f64 = 240.0;
        self.islands
            .iter()
            .enumerate()
            .filter(|(_, i)| i.radius >= SETTLED)
            .map(|(index, i)| {
                let at = i.anchorage();
                let d = at - i.centre;
                Harbour {
                    at,
                    // The index goes into the seed as well as the position. Position
                    // alone collided — two islands eight kilometres apart both came
                    // out "Fuglstad", which on a chart is an error rather than a
                    // coincidence.
                    name: place_name(
                        (i.centre.x.abs() as u64).wrapping_mul(0x9E37_79B9)
                            ^ (i.centre.y.abs() as u64).wrapping_mul(0x85EB_CA6B)
                            ^ (index as u64).wrapping_mul(0xC2B2_AE35),
                    ),
                    exposed_to: wrap_tau(d.x.atan2(d.y)),
                }
            })
            .collect()
    }
}
