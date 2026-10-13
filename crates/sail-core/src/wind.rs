//! Wind field.
//!
//! Meteorological convention throughout: a wind *direction* is where the wind comes
//! FROM. The flow vector for direction `theta` (compass radians, from north, clockwise)
//! and speed `s` is `(-s*sin(theta), -s*cos(theta))` in world ENU — verified in
//! `tests/conventions.rs`.
//!
//! `WindField` is the seam where a real GRIB2 forecast replaces the synthetic model
//! without touching anything downstream.

use crate::rng::Pcg32;
use crate::units::{wrap_tau, RHO_AIR};
use glam::DVec2;

/// Reference height for a quoted wind speed, by convention 10 m above the surface.
pub const WIND_REF_HEIGHT: f64 = 10.0;

/// Power-law exponent for the marine boundary layer. 0.11 is the standard open-water
/// value; it means a 12 kn wind at 10 m is ~11.3 kn at the 6 m centre of effort.
pub const SHEAR_EXPONENT: f64 = 0.11;

/// Scale a reference wind speed to an arbitrary height. Sampling the rig at its centre
/// of effort rather than at 10 m matters: it is a few percent of drive force, and more
/// importantly it is the mechanism behind sail twist.
#[inline]
pub fn shear_factor(height: f64) -> f64 {
    (height.max(0.5) / WIND_REF_HEIGHT).powf(SHEAR_EXPONENT)
}

/// Dynamic pressure of air at a given speed.
#[inline]
pub fn q_air(speed: f64) -> f64 {
    0.5 * RHO_AIR * speed * speed
}

pub trait WindField: Send + Sync {
    /// Where the weather is going: expected change in the mean wind over the next hour,
    /// as (metres per second, radians). Zero for a field with no trend in it.
    ///
    /// This is the barometer. A skipper does not sail the wind that is blowing, he sails
    /// the one he thinks will be blowing in an hour — whether to put the reef in now
    /// while it is easy, whether this shift is worth tacking on or will come back. The
    /// number is honest rather than clairvoyant: for a relaxing process the expected
    /// change is just how far the trend currently is from zero, so this says what the
    /// weather is *doing*, not what it will do.
    fn outlook(&self) -> (f64, f64) {
        (0.0, 0.0)
    }

    /// True wind flow vector in world ENU at the reference height, in m/s.
    /// `pos` is in metres, `t` in seconds since simulation start.
    fn sample(&self, pos: DVec2, t: f64) -> DVec2;

    /// Advance any internal stochastic state. A pure/analytic field may ignore this.
    fn step(&mut self, _dt: f64) {}

    /// The squalls that are about, as (centre, radius, how developed). Empty for a field
    /// without any.
    ///
    /// On the trait so that what you *see* is what hits you: the rain, the dark cloud
    /// over it and the extra twenty knots are one object, and a squall drawn in one
    /// place and felt in another is worse than no squall at all.
    fn squalls(&self) -> Vec<(DVec2, f64, f64)> {
        Vec::new()
    }

    /// How deep inside a squall a point is: 0 in the clear, 1 in the heart of one.
    /// Drives the rain and the visibility.
    fn squall_at(&self, _pos: DVec2) -> f64 {
        0.0
    }

    /// Cloud cover, 0 clear to 1 solid. Weather, so it lives with the rest of the slow
    /// weather rather than in the renderer: a front brings cloud and wind together, and
    /// a sky that clears while the breeze builds is a sky nobody believes.
    fn cover(&self) -> f64 {
        0.55
    }

    /// How much harder than the mean it is blowing at a point, as a fraction: 0.2 is a
    /// twenty percent gust and −0.2 a lull. Zero for a field with no structure in it.
    ///
    /// Exposed on the trait rather than kept inside the gust model because other things
    /// want to know where the gusts are. A cumulus sits over a downdraught, so the
    /// cloud layer places its cells by asking this — which makes the dark patch on the
    /// water and the extra pressure in the sails the same event rather than two
    /// decorations that happen to be on screen together.
    fn gust_at(&self, _pos: DVec2, _t: f64) -> f64 {
        0.0
    }
}

/// The slow weather: the mean wind is not a constant either.
///
/// Without this the simulator hands you a wind and that is the wind, for ever. Every
/// decision the genre is about — reef now or carry on, go out at all, which side of the
/// island — needs the weather to be *going* somewhere, and a day that cannot change is
/// one long trim exercise.
///
/// Two relaxing processes on top of the drawn day, at the scale real synoptic
/// variability runs at: the speed on forty minutes and the direction on an hour. They
/// start from their stationary distribution rather than from zero, so there is a trend
/// in the first minute instead of forty minutes of artificial calm before the weather
/// remembers to start — which also means the forecast handed over at the dock can
/// honestly say "building, veering", because it is reading a trend that already exists.
#[derive(Clone, Copy, Debug)]
pub struct Synoptic {
    /// The day as drawn: the mean the trends wander about.
    pub base_speed: f64,
    pub base_direction: f64,
    /// Standard deviation of the speed trend, as a fraction of the base.
    pub speed_swing: f64,
    /// Relaxation time of the speed trend, seconds.
    pub speed_timescale: f64,
    /// Standard deviation of the direction trend, radians.
    pub direction_swing: f64,
    /// Relaxation time of the direction trend, seconds.
    pub direction_timescale: f64,
    /// Cloud cover the day settles about, and how far it wanders from it.
    pub cover_mean: f64,
    pub cover_swing: f64,
    /// Relaxation time of the cover, seconds. Slower than the wind: cloud comes and
    /// goes over a watch, not over a tack.
    pub cover_timescale: f64,
    speed_trend: f64,
    direction_trend: f64,
    cover_trend: f64,
}

/// How much of the wind's trend shows up in the cloud cover.
///
/// They are correlated in the weather and not independent dials: the cloud comes with
/// the front that brings the breeze, so a building wind thickens the sky and a dying one
/// lets it open up. A fifth of the trend, which at the usual spread is about a tenth of
/// the sky.
const COVER_FROM_WIND: f64 = 0.45;

/// How far the mean wind is allowed to wander from the drawn day, as a multiple.
///
/// Not a taste decision in either direction. The floor keeps the player out of the
/// light-air regime the model is known to handle badly (see `weather.rs`), and the
/// ceiling keeps a drawn gale from becoming a storm the boat has no answer to.
const SYNOPTIC_FLOOR: f64 = 0.55;
const SYNOPTIC_CEILING: f64 = 1.75;

impl Synoptic {
    pub fn new(direction: f64, speed: f64, rng: &mut Pcg32) -> Self {
        Self {
            base_speed: speed,
            base_direction: direction,
            speed_swing: 0.22,
            speed_timescale: 2_400.0,
            direction_swing: 20.0_f64.to_radians(),
            direction_timescale: 3_600.0,
            cover_mean: 0.55,
            cover_swing: 0.22,
            cover_timescale: 9_000.0,
            // From the stationary distribution, so the day is already under way.
            speed_trend: 0.22 * rng.next_normal(),
            direction_trend: 20.0_f64.to_radians() * rng.next_normal(),
            cover_trend: 0.22 * rng.next_normal(),
        }
    }

    fn step(&mut self, dt: f64, rng: &mut Pcg32) {
        ou_step(
            &mut self.speed_trend,
            self.speed_swing,
            self.speed_timescale,
            dt,
            rng,
        );
        ou_step(
            &mut self.direction_trend,
            self.direction_swing,
            self.direction_timescale,
            dt,
            rng,
        );
        ou_step(
            &mut self.cover_trend,
            self.cover_swing,
            self.cover_timescale,
            dt,
            rng,
        );
    }

    /// Set the cover to a given value *now*, leaving it free to wander afterwards.
    ///
    /// Needed because the day's cover is drawn in one place and the process that moves
    /// it lives in another. Setting the mean alone does not do it: the trend is drawn
    /// from its stationary distribution so the weather is already under way, and the
    /// wind coupling adds its own slice — so a day forecast at two oktas opened at
    /// eight, and the forecast on the dock was contradicted by the sky above it inside
    /// two minutes. Solve for the trend instead, and the two agree at t=0 and diverge
    /// honestly after it.
    pub fn start_cover(&mut self, cover: f64) {
        self.cover_trend = cover - self.cover_mean - COVER_FROM_WIND * self.speed_trend;
    }

    /// Cover now. Correlated with the wind, because they are correlated in the weather:
    /// the cloud comes with the front that brings the breeze, so a building wind brings
    /// a thickening sky and a dying one lets it open up.
    fn cover(&self) -> f64 {
        (self.cover_mean + self.cover_trend + COVER_FROM_WIND * self.speed_trend).clamp(0.0, 1.0)
    }

    fn speed(&self) -> f64 {
        (self.base_speed * (1.0 + self.speed_trend)).clamp(
            self.base_speed * SYNOPTIC_FLOOR,
            self.base_speed * SYNOPTIC_CEILING,
        )
    }

    fn direction(&self) -> f64 {
        wrap_tau(self.base_direction + self.direction_trend)
    }

    /// Stop the weather where it is. Zeroing the swings is not enough on its own: the
    /// trends are *drawn* at construction so the day is already under way, and a field
    /// with the noise turned off but a trend still in it relaxes towards the base over
    /// the next hour — so a wind asked to hold steady quietly changed by two knots and
    /// twenty degrees on its first step, which is the sort of thing that makes a
    /// validation suite measure the wrong boat. Found by a squall test that kept
    /// missing: the squall was being aimed up a wind that was no longer there.
    fn settled(&mut self) {
        self.speed_swing = 0.0;
        self.direction_swing = 0.0;
        self.speed_trend = 0.0;
        self.direction_trend = 0.0;
        self.cover_swing = 0.0;
        self.cover_trend = 0.0;
    }

    /// Expected change over the next hour. For `dX = −X/tau dt + noise` the mean path
    /// is `X exp(−t/tau)`, so this is exact rather than an estimate.
    fn outlook(&self) -> (f64, f64) {
        let decay = |tau: f64| 1.0 - (-3_600.0 / tau).exp();
        (
            -self.base_speed * self.speed_trend * decay(self.speed_timescale),
            -self.direction_trend * decay(self.direction_timescale),
        )
    }
}

/// A squall: a travelling cell of weather with its own wind in it.
///
/// Modelled as what one physically *is*, which turns out to be cheaper than modelling
/// what one feels like. A squall is a column of air coming down out of a cloud and
/// spreading out when it hits the water — so the wind in it is a radial **outflow** from
/// the centre, added to whatever was blowing anyway. Everything a sailor knows about
/// squalls falls out of that one vector and none of it has to be written down
/// separately:
///
/// * On the leading edge the outflow runs the same way as the wind, so the gust front
///   is where the extra twenty knots are.
/// * On the flanks they are at right angles, so the sum is both stronger *and* shifted
///   forty-odd degrees — the big header or lift that comes with one.
/// * Behind it they oppose, which is the hole. The calm after a squall is not a lull in
///   the weather, it is the outflow cancelling the gradient wind, and this gets it for
///   nothing.
/// * Outside the core there is a ring of weak inflow, because the air coming down has
///   to come from somewhere. That is the lull *before* it arrives, which is the oldest
///   warning there is and the one that makes the whole thing fair: the boat is told.
///
/// Sizes are at the small end of real. A squall two to four kilometres across takes five
/// to ten minutes to pass over at these speeds, where a true ten-kilometre line would
/// take most of an hour and be the whole session.
#[derive(Clone, Copy, Debug)]
pub struct Squall {
    /// Centre, in world metres.
    pub at: DVec2,
    /// Radius of the core, metres.
    pub radius: f64,
    /// Peak outflow as a fraction of the mean wind speed.
    pub strength: f64,
    pub age: f64,
    pub life: f64,
}

impl Squall {
    /// How developed it is: nothing at birth, full in the middle of its life, gone at
    /// the end. A squall that arrived at full strength would be a wall, and one that
    /// vanished at full strength would leave a hole in the wind.
    pub fn maturity(&self) -> f64 {
        let u = (self.age / self.life).clamp(0.0, 1.0);
        (std::f64::consts::PI * u).sin().powf(0.6)
    }

    /// How deep inside it a point is: 1 at the centre of the core, 0 at the edge and
    /// beyond. This is what the rain and the visibility are drawn from.
    pub fn depth(&self, pos: DVec2) -> f64 {
        let r = (pos - self.at).length() / self.radius;
        (1.0 - crate::units::smoothstep(0.55, 1.05, r)) * self.maturity()
    }

    /// The radial profile of the outflow, as a fraction of `strength`. Positive is air
    /// coming out of the cell; negative is the ring of inflow feeding it.
    fn radial(&self, r: f64) -> f64 {
        if r < 1.0 {
            // Strongest a little inside the edge rather than at the dead centre, which
            // is where the downdraught actually spreads hardest.
            let x = r.max(0.08);
            x.min(0.75) / 0.75 * (1.0 - crate::units::smoothstep(0.75, 1.05, r))
        } else {
            // The inflow that feeds it: weak, wide, and the reason it goes light before
            // a squall hits.
            -0.22 * (1.0 - crate::units::smoothstep(1.0, 2.4, r))
        }
    }

    /// What this squall adds to the flow at a point, in m/s.
    pub fn outflow(&self, pos: DVec2, mean_speed: f64) -> DVec2 {
        let offset = pos - self.at;
        let r = offset.length() / self.radius;
        if r > 2.4 {
            return DVec2::ZERO;
        }
        let outward = if offset.length_squared() < 1.0 {
            DVec2::ZERO
        } else {
            offset.normalize()
        };
        outward * (self.strength * mean_speed * self.radial(r) * self.maturity())
    }
}

/// Mean interval between squalls, seconds. One every twenty-five minutes on a squally
/// day, which is frequent for real weather and about right for a watch at the helm.
const SQUALL_INTERVAL: f64 = 1_500.0;
/// How many can be about at once. Two is a busy sky.
const SQUALLS_AT_ONCE: usize = 2;
/// How far upwind one is born, metres. Far enough that the rain and the dark cloud are
/// a warning you can act on rather than something that happens to you.
const SQUALL_BIRTH: f64 = 5_000.0;
/// And how far downwind one is forgotten.
const SQUALL_FORGET: f64 = 7_000.0;

/// Gustiness as a field in space, carried over the water by the wind that makes it.
///
/// This replaced a purely temporal model, and the replacement is the point. The old
/// gust was an Ornstein-Uhlenbeck process on the speed: correct statistics at the boat,
/// and the *same* wind everywhere at once. So a gust did not arrive — it switched on, on
/// the whole ocean, which is the one thing about wind that a sailor can see is false. You
/// watch the water to windward because a gust is a patch travelling towards you, and
/// reading that patch is a real skill with a real pay-off.
///
/// What makes it cheap is that it is analytic: smooth value noise in two octaves,
/// evaluated at `pos` offset by how far the pattern has been carried. No state, no cells
/// to recycle, no synchronisation — the same function answers for the boat, for the wind
/// streaks on the water and for the cloud layer, and it answers for a point a kilometre
/// to windward just as happily as for one under the keel.
///
/// The old calibration is what sets the cell size, which is a pleasing way round. Twelve
/// percent intensity on a twenty-five second timescale was measured over open water; a
/// patch of size `L` carried past at `U` gives a timescale of `L/U`, so twenty-five
/// seconds at eight metres a second *is* a two hundred metre cell. That is the right
/// number for boundary-layer gust cells, and it was not chosen — it fell out.
#[derive(Clone, Copy, Debug)]
pub struct GustField {
    /// Size of a gust cell, metres.
    pub cell: f64,
    /// Standard deviation of the speed anomaly, as a fraction of the mean.
    pub intensity: f64,
    /// How fast the pattern travels compared with the wind carrying it. A gust patch
    /// runs a little slower than the air it is in, because it is a structure in the
    /// boundary layer rather than a parcel of it.
    pub advection: f64,
    seed: u64,
}

impl GustField {
    pub fn new(seed: u64) -> Self {
        Self {
            cell: 200.0,
            intensity: 0.12,
            advection: 0.85,
            seed,
        }
    }

    /// The anomaly at a point, as a fraction of the mean wind speed.
    ///
    /// `carried` is how far the pattern has been moved downwind so far — an integral,
    /// not a speed times a time, and the difference is a bug this cost before it was a
    /// comment. Written as `flow * advection * t` it seemed equivalent and was not: the
    /// moment the mean wind changes, *every* past second is re-advected at the new
    /// speed, so the whole pattern teleports. At an hour in, a hundredth of a knot of
    /// change jumps the field metres per frame, and the gusts turn into a buzz. Caught
    /// by `gusts_are_gusty_but_not_noise`, which counts exactly that.
    pub fn at(&self, pos: DVec2, carried: DVec2) -> f64 {
        if self.intensity <= 0.0 || self.cell <= 0.0 {
            return 0.0;
        }
        // Scaled so `intensity` is the standard deviation of the result, which is
        // what lets the twelve percent measured over open water mean twelve percent
        // here.
        crate::noise::two_octaves((pos - carried) / self.cell, self.seed)
            * (self.intensity / crate::noise::TWO_OCTAVE_SIGMA)
    }
}

/// Flow vector from a meteorological direction and speed.
#[inline]
pub fn flow_from(direction: f64, speed: f64) -> DVec2 {
    DVec2::new(-speed * direction.sin(), -speed * direction.cos())
}

/// Meteorological direction (where it comes from) of a flow vector.
#[inline]
pub fn direction_of(flow: DVec2) -> f64 {
    wrap_tau((-flow.x).atan2(-flow.y))
}

/// Steady wind. Useful as a baseline and for isolating a bug to the gust model.
#[derive(Clone, Debug)]
pub struct SteadyWind {
    pub direction: f64,
    pub speed: f64,
}

impl SteadyWind {
    pub fn new(direction: f64, speed: f64) -> Self {
        Self { direction, speed }
    }
}

impl WindField for SteadyWind {
    fn sample(&self, _pos: DVec2, _t: f64) -> DVec2 {
        flow_from(self.direction, self.speed)
    }
}

/// Synthetic but physical wind: an Ornstein-Uhlenbeck process on both speed and
/// direction.
///
/// Three properties make this feel like real wind rather than noise:
///   * turbulence intensity ~10-15% of the mean over open water,
///   * gust timescales of tens of seconds, not frames,
///   * gusts veer. In the northern hemisphere a gust brings down air with more
///     momentum from aloft, which arrives backed to the right of the surface wind.
///     Sailors steer on this, so an unveering gust reads as fake immediately.
#[derive(Clone, Debug)]
pub struct SyntheticWind {
    /// The mean the gusts are measured against, which the slow weather moves.
    pub mean_direction: f64,
    pub mean_speed: f64,
    /// The slow weather that moves them.
    pub synoptic: Synoptic,
    /// The gusts, as a field in space rather than a wobble in time.
    pub gusts: GustField,
    /// Standard deviation of direction, radians.
    pub direction_spread: f64,
    /// Relaxation time of the direction process, seconds.
    pub shift_timescale: f64,
    /// How strongly a gust veers right, radians per unit normalised speed anomaly.
    pub veer_coupling: f64,

    direction_anomaly: f64,
    /// How far the gust pattern has been carried downwind, metres. Integrated, so a
    /// change in the wind changes where the pattern is *going* and not where it has
    /// been.
    carried: DVec2,
    /// The squalls about, and the seconds until the next one is born.
    squalls: Vec<Squall>,
    /// Seconds until the next one is born.
    pub next_squall: f64,
    /// Mean interval between squalls, seconds. Public because how squally a day is, is
    /// a property of the day: some are a procession of them and some have none, and the
    /// draw in `weather.rs` is where that belongs rather than here.
    pub squall_interval: f64,
    rng: Pcg32,
}

impl SyntheticWind {
    /// A plausible open-water day: moderate breeze, moderate gustiness.
    pub fn new(mean_direction: f64, mean_speed: f64, seed: u64) -> Self {
        let mut rng = Pcg32::new(seed);
        let synoptic = Synoptic::new(mean_direction, mean_speed, &mut rng);
        Self {
            mean_direction: synoptic.direction(),
            mean_speed: synoptic.speed(),
            synoptic,
            gusts: GustField::new(seed ^ 0x6057),
            direction_spread: 6.0_f64.to_radians(),
            shift_timescale: 90.0,
            veer_coupling: 7.0_f64.to_radians(),
            direction_anomaly: 0.0,
            carried: DVec2::ZERO,
            squalls: Vec::new(),
            // Drawn from the exponential the interval implies, so the first one is not
            // always due at the same moment of the passage.
            next_squall: -SQUALL_INTERVAL * (1.0 - rng.next_f64().max(1e-9)).ln(),
            squall_interval: SQUALL_INTERVAL,
            rng,
        }
    }

    /// A squall, placed upwind of the origin and across it by a random offset.
    ///
    /// Upwind of the *world origin* rather than of the boat, which is deliberate: the
    /// wind field does not know where the boat is and should not, or the weather starts
    /// following the player about. The offsets are wide enough that most of them miss,
    /// which is also how it is — you watch three go by to leeward for every one that
    /// gets you.
    fn born_upwind(&mut self) -> Squall {
        let upwind = -flow_from(self.mean_direction, self.mean_speed).normalize_or_zero();
        let across = DVec2::new(-upwind.y, upwind.x);
        let offset = (self.rng.next_f64() * 2.0 - 1.0) * SQUALL_FORGET;
        Squall {
            at: upwind * SQUALL_BIRTH + across * offset,
            radius: 700.0 + self.rng.next_f64() * 1_100.0,
            // Half again to two and a half times the gradient wind at the core. The top
            // of that range is a squall you reef for and the bottom is one you shrug at,
            // and not knowing which it is until it is close is the whole decision.
            strength: 0.5 + self.rng.next_f64() * 2.0,
            age: 0.0,
            life: 900.0 + self.rng.next_f64() * 900.0,
        }
    }

    /// Put a squall in, where you want it. For the validation suite: a weather feature
    /// that happens every twenty-five minutes at a random bearing cannot be asserted
    /// about unless it can also be placed.
    pub fn add_squall(&mut self, squall: Squall) {
        self.squalls.push(squall);
    }

    /// Speed at a point and an instant, at the reference height.
    pub fn speed_at(&self, pos: DVec2, t: f64) -> f64 {
        (self.mean_speed * (1.0 + self.gust_at(pos, t))).max(0.0)
    }

    /// Direction at a point and an instant, including the veer that goes with a gust.
    ///
    /// A gust over water arrives from the right of the mean in the northern hemisphere,
    /// because the air in it has come down from a height where it was turning less with
    /// friction. It is the shift you can set a tack up for, so it is modelled: the veer
    /// is tied to the local gust strength, which means it now arrives *with the patch*
    /// rather than happening everywhere at once.
    pub fn direction_at(&self, pos: DVec2, t: f64) -> f64 {
        wrap_tau(
            self.mean_direction
                + self.direction_anomaly
                + self.veer_coupling * self.gust_at(pos, t),
        )
    }

    /// Freeze the stochastic state. Handy for reproducing a moment while tuning.
    pub fn frozen(mut self) -> Self {
        self.squalls.clear();
        self.next_squall = f64::INFINITY;
        self.gusts.intensity = 0.0;
        self.direction_spread = 0.0;
        self.direction_anomaly = 0.0;
        self.synoptic.settled();
        self.mean_speed = self.synoptic.speed();
        self.mean_direction = self.synoptic.direction();
        self
    }
}

impl WindField for SyntheticWind {
    fn sample(&self, pos: DVec2, t: f64) -> DVec2 {
        let mut flow = flow_from(self.direction_at(pos, t), self.speed_at(pos, t));
        for squall in &self.squalls {
            // Inside the core the air is turbulent and windy all through, not only where
            // the outflow happens to line up with the gradient wind. A third again over
            // the whole footprint, and the outflow on top of it.
            flow *= 1.0 + 0.35 * squall.depth(pos);
            flow += squall.outflow(pos, self.mean_speed);
        }
        flow
    }

    fn squalls(&self) -> Vec<(DVec2, f64, f64)> {
        self.squalls
            .iter()
            .map(|s| (s.at, s.radius, s.maturity()))
            .collect()
    }

    fn squall_at(&self, pos: DVec2) -> f64 {
        self.squalls
            .iter()
            .map(|s| s.depth(pos))
            .fold(0.0, f64::max)
    }

    fn gust_at(&self, pos: DVec2, _t: f64) -> f64 {
        self.gusts.at(pos, self.carried)
    }

    fn outlook(&self) -> (f64, f64) {
        self.synoptic.outlook()
    }

    fn cover(&self) -> f64 {
        self.synoptic.cover()
    }

    fn step(&mut self, dt: f64) {
        // The slow weather first, because it moves the mean everything else is measured
        // against — including where the gusts are carried to.
        self.synoptic.step(dt, &mut self.rng);
        self.mean_speed = self.synoptic.speed();
        self.mean_direction = self.synoptic.direction();

        // Carry the gust pattern downwind. A gust patch travels a little slower than
        // the air it is a structure in.
        let mean_flow = flow_from(self.mean_direction, self.mean_speed);
        self.carried += mean_flow * (self.gusts.advection * dt);

        // The squalls. They are carried by the wind above them, which is faster than the
        // one on the water — so a squall overtakes you, and the fact that it catches up
        // from astern-of-upwind is why you get a look at it first.
        for squall in &mut self.squalls {
            squall.age += dt;
            squall.at += mean_flow * (1.25 * dt);
        }
        self.squalls.retain(|s| s.age < s.life);

        self.next_squall -= dt;
        if self.next_squall <= 0.0 {
            self.next_squall = -self.squall_interval * (1.0 - self.rng.next_f64().max(1e-9)).ln();
            if self.squalls.len() < SQUALLS_AT_ONCE {
                let born = self.born_upwind();
                self.squalls.push(born);
            }
        }

        // Only the fast wander of the direction is a process in time now. The
        // gusts are a pattern in space and need no stepping — they arrive because the
        // wind carries them past, which is both cheaper and the actual mechanism.
        //
        // Exact discretisation of dX = -X/tau dt + sigma sqrt(2/tau) dW, so the step
        // size cannot change the statistics of the process.
        ou_step(
            &mut self.direction_anomaly,
            self.direction_spread,
            self.shift_timescale,
            dt,
            &mut self.rng,
        );
    }
}

fn ou_step(x: &mut f64, sigma: f64, tau: f64, dt: f64, rng: &mut Pcg32) {
    if sigma <= 0.0 || tau <= 0.0 {
        return;
    }
    let decay = (-dt / tau).exp();
    let noise = sigma * (1.0 - decay * decay).sqrt();
    *x = *x * decay + noise * rng.next_normal();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `intensity` has to mean what it says, or every wind speed in the simulator is
    /// quietly a few percent wrong and nothing else says so. The shape function's own
    /// standard deviation is a measured constant, so this is the measurement.
    #[test]
    fn the_gust_field_has_the_intensity_it_claims() {
        let field = GustField::new(0xA11CE);
        // Sampled over a few square kilometres on an irrational stride, so the points
        // never fall into step with the lattice.
        let mut sum = 0.0;
        let mut sum_sq = 0.0;
        let mut n = 0.0;
        for i in 0..400 {
            for j in 0..400 {
                let p = DVec2::new(i as f64 * 17.37, j as f64 * 13.11);
                let x = field.at(p, DVec2::ZERO);
                sum += x;
                sum_sq += x * x;
                n += 1.0;
            }
        }
        let mean = sum / n;
        let sigma = (sum_sq / n - mean * mean).sqrt();
        assert!(
            mean.abs() < 0.005,
            "a gust field with a mean in it is a wind speed error, got {mean:.4}"
        );
        assert!(
            (sigma - field.intensity).abs() < 0.004,
            "intensity {:.3} should be the standard deviation, got {sigma:.4} — \
             so the scaling is wrong",
            field.intensity
        );
    }

    /// The pattern travels with the wind, and that is the whole reason it exists: a gust
    /// you can see coming is one that is somewhere else first.
    #[test]
    fn a_gust_arrives_from_upwind() {
        let field = GustField::new(7);
        let flow = flow_from(0.0, 8.0); // from the north, so flowing south
        let carried = flow * (field.advection * 60.0);
        let here = DVec2::ZERO;
        // What is upwind of us now is what we will be in a minute.
        let now = field.at(here - carried, DVec2::ZERO);
        let later = field.at(here, carried);
        assert!(
            (now - later).abs() < 1e-9,
            "what is upwind now should be here later: {now:.4} then {later:.4}"
        );
    }
}
