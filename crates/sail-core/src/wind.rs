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

    /// What the glass reads, as (hectopascals, change over the last three hours).
    ///
    /// There is no pressure field in this model and there does not need to be one: a
    /// barometer on a boat is not an instrument for measuring air, it is an instrument
    /// for inferring the system you are in, and the system here *is* the synoptic state —
    /// the mean wind and which way it is going. So the reading is derived from those, by
    /// the relation every pilot book states the other way round: deep low, strong wind;
    /// high, calm.
    ///
    /// A flat calm under a ridge reads about 1030, a fresh breeze about 1010, a gale
    /// about 985, a storm about 970. That is an empirical fit to mid-latitude weather and
    /// it is stated as one — it is not claiming a pressure field, it is reading the
    /// weather the game actually has in the units a sailor keeps his own log in.
    ///
    /// The tendency is the number that matters. Nobody acts on 1008; everybody acts on
    /// *1008 and falling three in three hours*, which is a gale warning written on a dial.
    fn barometer(&self) -> (f64, f64) {
        (1013.0, 0.0)
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

    /// The synthetic weather behind this field, if there is one.
    ///
    /// The seam a settings dial needs, and it is deliberately the only one: a real
    /// forecast read off a GRIB file has no dials on it and answers `None`, at which
    /// point the panel says the weather is not adjustable, which is the truth. Giving
    /// the trait a set of setters instead would have put a promise on every field that
    /// only one of them can keep.
    fn adjustable(&mut self) -> Option<&mut SyntheticWind> {
        None
    }

    /// Cloud cover, 0 clear to 1 solid. Weather, so it lives with the rest of the slow
    /// weather rather than in the renderer: a front brings cloud and wind together, and
    /// a sky that clears while the breeze builds is a sky nobody believes.
    fn cover(&self) -> f64 {
        0.55
    }

    /// How thick the fog is, 0 clear to 1 blind. Here with the cloud and for the same
    /// reason: it is weather, it comes and goes on the weather's own clock, and a field
    /// with no weather in it has none.
    fn fog(&self) -> f64 {
        0.0
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

    /// Which planetary belt a point is in, in the words a sailor uses, or `None` for a
    /// world with one wind in it — which is every world this engine had until the belts
    /// went in, and still the default.
    fn band_at(&self, _pos: DVec2) -> Option<&'static str> {
        None
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
    /// How thick the fog is, 0 to 1, as a slow process of its own.
    ///
    /// **Not cloud.** Cover is what is above you and fog is what you are inside, and the
    /// two are different weather: a clear night with a cold sea under warm air is the
    /// classic way to get fog and there is no cloud in it at all. So it gets its own
    /// mean and its own trend rather than being read off the sky.
    fog_mean: f64,
    fog_trend: f64,
    /// **Fog's own dice.**
    ///
    /// Not the shared generator, and that is the whole of why this field exists. The
    /// synoptic process, the squalls and the gust field all draw from one stream, so a
    /// new quantity taking a number out of it shifts every number after it — and every
    /// seed in the game then means a different day. Caught by a crew scenario that had
    /// nothing to do with fog and failed the moment this was added.
    ///
    /// Kept as a seed and a counter rather than as a generator, because `Synoptic` is
    /// `Copy` and a generator that can be copied is a generator that quietly rewinds —
    /// which this crate decided once already by giving `Pcg32` only `Clone`.
    fog_seed: u64,
    fog_step: u64,
    /// The front crossing today, if one is. Most days have none.
    pub front: Option<Front>,
    /// Seconds since the passage began, which is what the front is read against.
    ///
    /// Kept here rather than taken as an argument because every reader of this struct
    /// asks it for a scalar — `speed()`, `cover()`, `barometer()` — and threading a
    /// time through all of them to serve one optional feature is how a model acquires a
    /// parameter nobody can explain.
    since: f64,
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
/// How far the fog wanders from the day it was drawn.
///
/// Wider than the cloud's, because fog is more nearly a yes-or-no than cover is: a coast
/// either has it or it has not, and the days in between are the unusual ones.
const FOG_SWING: f64 = 0.35;

/// A front crossing: the day's weather becoming a different day's.
///
/// # Why this is a *time* and not a line on the water
///
/// A front is a boundary between two air masses, five hundred kilometres long and
/// moving at twenty-five knots. This archipelago is five and a half kilometres across.
/// Every boat in the game is through the transition within about seven minutes of every
/// other, so a front modelled as a line in the water would be a line nobody can see
/// both ends of, costing a dot product per sample to express a difference of minutes.
///
/// The honest simplification is the opposite of the squall's. A squall is modelled as a
/// *thing in the water* because it is four hundred metres across and you can watch it
/// coming; a front is modelled as a thing on the clock because at this scale that is
/// what it is. What you see of a front is never its edge — it is the sequence over your
/// own head.
///
/// # What a sailor gets out of it
///
/// All of it falls out of two air masses and a transition, with nothing written down
/// separately:
///
/// * **The wind veers through it**, by twenty to seventy degrees, which is what the
///   circulation round a northern low does — so the tack you are on matters hours
///   before anything happens.
/// * **The glass falls, bottoms out at the line and rises behind.** A front is a trough
///   of low pressure travelling with the boundary, so the barometer stops being a
///   readout of the wind and becomes the one instrument aboard that is ahead of the
///   weather.
/// * **It blows harder behind a cold one and clears.** The cover belongs to the air
///   mass, so the sky changes *with* the wind rather than being correlated with it by a
///   coefficient.
#[derive(Clone, Copy, Debug)]
pub struct Front {
    /// Seconds from the start of the passage to the middle of the transition.
    pub through_at: f64,
    /// How long the change takes, seconds.
    ///
    /// A sharp cold front is twenty minutes of wind and done; a warm front is half a
    /// day of thickening cloud and a slow back. The difference between the two is this
    /// number and what is behind it, and nothing else.
    pub takes: f64,
    /// The air mass behind it.
    pub behind_speed: f64,
    pub behind_direction: f64,
    pub behind_cover: f64,
    /// How deep the pressure trough at the line is, hectopascals. Four to eight on an
    /// ordinary front.
    pub trough: f64,
}

impl Front {
    /// How far through it the day is: 0 fully ahead, 1 fully behind.
    ///
    /// Smoothstepped rather than linear, because a front has no edge: it begins as a
    /// thickening and ends as a clearance, and the steep part is in the middle.
    pub fn through(&self, since: f64) -> f64 {
        let half = self.takes.max(1.0) * 0.5;
        crate::units::smoothstep(-half, half, since - self.through_at)
    }

    /// The trough, in hectopascals below the air masses' own, at this moment. A bell
    /// centred on the line and as wide as the transition.
    pub fn dip(&self, since: f64) -> f64 {
        let half = self.takes.max(1.0) * 0.5;
        let off = (since - self.through_at) / half;
        self.trough * (-off * off).exp()
    }
}

const SYNOPTIC_FLOOR: f64 = 0.55;
const SYNOPTIC_CEILING: f64 = 1.75;

impl Synoptic {
    pub fn new(direction: f64, speed: f64, seed: u64, rng: &mut Pcg32) -> Self {
        let fog_seed = seed ^ 0xF0_6F_06;
        let mut fog_rng = Pcg32::new(fog_seed);
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
            // Most days have none. A mean at a fifth with this swing puts thick fog on
            // about one day in six and a bit of haze on a good many more — a coast
            // rather than a climate, and the shape anybody who sails one would know:
            // fog is not rare, it is just not today.
            fog_mean: 0.20,
            fog_trend: FOG_SWING * fog_rng.next_normal(),
            fog_seed,
            fog_step: 1,
            front: None,
            since: 0.0,
        }
    }

    fn step(&mut self, dt: f64, rng: &mut Pcg32) {
        self.since += dt;
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
        // Fog comes and goes over a watch rather than over a tack, like the cloud, and
        // a little slower: a bank rolls in over an hour and takes most of a day to lift.
        // Its own dice here too, for the same reason: stepping the fog off the shared
        // stream would shift the gusts by one draw every step, which is the same fault
        // spread thinner.
        //
        let mut fog_rng = Pcg32::new(self.fog_seed ^ self.fog_step);
        self.fog_step = self.fog_step.wrapping_add(1);
        ou_step(
            &mut self.fog_trend,
            FOG_SWING,
            self.cover_timescale * 1.6,
            dt,
            &mut fog_rng,
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

    /// Give the day a front to cross, from its own dice.
    ///
    /// **Its own seed, like the fog's, and for the same reason**: a number taken out of
    /// the shared stream shifts every gust in every world that ever existed. That was
    /// learned once already, by a test that noticed the watch changing sail a minute
    /// later after fog was added.
    ///
    /// Most days have none. One in three is about right for a coast in the half of the
    /// year anybody sails in — often enough that the glass is worth watching, rare
    /// enough that a steady day is still a steady day.
    pub fn may_bring_a_front(&mut self, seed: u64) {
        let mut rng = Pcg32::new(seed ^ 0xF2_0A_71);
        if rng.next_f64() > 0.34 {
            return;
        }
        // **Which way it shifts is not a coin.** In the northern hemisphere a front
        // veers the wind as it goes through, every time, because that is what the
        // circulation round a low does — and the amount is twenty to seventy degrees. A
        // model that backed half the time would be teaching the wrong lesson to the one
        // person this is for.
        let veer = crate::units::rad(20.0 + 50.0 * rng.next_f64());
        // Two thirds of the fronts crossing a coast like this are cold: sharp, and clear
        // behind. A warm one is slow and leaves it thick.
        let cold = rng.next_f64() < 0.66;
        self.front = Some(Front {
            // Somewhere in the first few hours, so a passage of an hour or two has a
            // real chance of being in it rather than a certainty of missing it.
            through_at: 600.0 + 9_000.0 * rng.next_f64(),
            takes: if cold {
                900.0 + 1_500.0 * rng.next_f64()
            } else {
                7_000.0 + 9_000.0 * rng.next_f64()
            },
            behind_speed: self.base_speed
                * if cold {
                    1.25 + 0.5 * rng.next_f64()
                } else {
                    0.75
                },
            behind_direction: wrap_tau(self.base_direction + veer),
            behind_cover: if cold {
                0.15 + 0.25 * rng.next_f64()
            } else {
                0.80 + 0.19 * rng.next_f64()
            },
            trough: 4.0 + 4.0 * rng.next_f64(),
        });
    }

    /// Put the weather's own clock where the passage is.
    ///
    /// **For a boat that joined late.** The front is read against seconds since the
    /// passage began, and that number is accumulated a step at a time — so a client
    /// that joins an hour in would start its front an hour behind the host's and sail
    /// into a veer nobody else got. Everything else about the front needs no packet:
    /// both machines draw the same one from the same seed, like the islands. This is
    /// the one number that has to be told.
    pub fn at_time(&mut self, t: f64) {
        self.since = t;
    }

    /// Stop the trends, leaving the front free to do its work. For measuring one thing
    /// at a time: the day's noise is a knot either way and enough to hide a front's
    /// first hour.
    pub fn hold_steady(&mut self) {
        self.settled();
    }

    /// The glass and its tendency, and the hour's outlook — the trait's own, said here
    /// so a panel or a test can ask the weather rather than the field.
    pub fn glass(&self) -> (f64, f64) {
        self.barometer()
    }

    pub fn expects(&self) -> (f64, f64) {
        self.outlook()
    }

    /// Put the fog the air is carrying where it is asked for, and let it wander after.
    ///
    /// **It sets what is there, not what you will see**, and the difference is the one
    /// honest thing about this dial. The first version solved through the thinning so
    /// that asking for nine tenths gave nine tenths — which meant a dial that could
    /// produce thick fog in a gale, and stored a trend of several hundred to do it, so
    /// that the moment the wind eased the fog went to one and stayed there.
    ///
    /// Asking for fog in twenty knots now gives you almost none, because that is what
    /// twenty knots does to fog. A dial that can lie about its own model is worse than
    /// no dial.
    pub fn start_fog(&mut self, fog: f64) {
        self.fog_trend = fog.clamp(0.0, 1.0) - self.fog_mean;
    }

    /// What the air is carrying, before the wind gets at it — which is what a dial
    /// setting it should read back, or it would snap away from wherever it was put.
    pub fn fog_aloft(&self) -> f64 {
        (self.fog_mean + self.fog_trend).clamp(0.0, 1.0)
    }

    /// **How much of the fog the wind has not yet blown away.**
    ///
    /// The relation this model has to get right, because it is the one a sailor knows
    /// without thinking: fog does not survive a breeze. Below about eight knots it sits;
    /// by twenty it has gone. Mixing is what destroys it — the wind stirs the cold air at
    /// the surface into the warmer air above and the droplets evaporate — so this is a
    /// mechanism rather than a fudge, and it is why a foggy morning burns off the moment
    /// the sea breeze fills in.
    fn thinning(&self) -> f64 {
        let knots = crate::units::ms_to_kn(self.speed());
        (1.0 - (knots - 8.0) / 12.0).clamp(0.0, 1.0)
    }

    /// How thick it is here and now: 0 is clear and 1 is not seeing your own bow.
    pub fn fog(&self) -> f64 {
        ((self.fog_mean + self.fog_trend) * self.thinning()).clamp(0.0, 1.0)
    }

    /// Cover now. Correlated with the wind, because they are correlated in the weather:
    /// the cloud comes with the front that brings the breeze, so a building wind brings
    /// a thickening sky and a dying one lets it open up.
    pub fn cover(&self) -> f64 {
        (self.air_mass().2 + self.cover_trend + COVER_FROM_WIND * self.speed_trend).clamp(0.0, 1.0)
    }

    /// Take the weather from somewhere else rather than from the dice.
    ///
    /// For a game with more than one boat in it. Everything else about that world is a
    /// pure function of the seed and therefore needs no agreement — the islands, the
    /// depths, the gust *pattern* — but the synoptic wind is a process with state, and
    /// two machines rolling their own cannot be relied on to stay in the same breeze.
    /// Being in different winds is the one divergence that would actually spoil sailing
    /// together: you would be watching somebody sail beautifully in air that is not
    /// there.
    ///
    /// Written as the inverse of `speed`, `direction` and `cover` rather than as a
    /// separate "override" path, so there is one definition of what the weather is and
    /// following somebody else's is a matter of setting the same three trends the dice
    /// would have set. The process then carries on from there under its own relaxation,
    /// which is what makes a once-a-second correction smooth rather than a step: between
    /// packets the weather here evolves plausibly, and every second it is told where it
    /// actually is.
    pub fn follow(&mut self, speed: f64, direction: f64, cover: f64) {
        self.speed_trend = speed / self.base_speed.max(0.5) - 1.0;
        self.direction_trend = crate::units::wrap_pi(direction - self.base_direction);
        self.start_cover(cover);
    }

    /// Set the day's wind and mean it **now**.
    ///
    /// `base_speed` on its own is the day the trends wander about, not the wind on the
    /// water: the trend that is in it when the base changes stays there and relaxes over
    /// forty minutes, and it is allowed to be three quarters high or nearly half low. So
    /// a base moved from a gale to a fresh breeze leaves a wind that is still most of a
    /// gale for the next half hour — which is what the trailer's reel found the hard way,
    /// with the watch setting a Code 0 into the easing it could read on the glass and the
    /// cloth going out of her in a gust that had not eased at all.
    ///
    /// That lag is right when the weather is making its own mind up and wrong when
    /// somebody has reached in and said what it is. This is the second case: a slider, a
    /// reel, a scenario. The swings stay, so it is still weather and not a number.
    pub fn blows(&mut self, speed: f64) {
        self.base_speed = speed;
        self.speed_trend = 0.0;
    }

    /// The two the rest of the world is allowed to read, because a game with more than
    /// one boat in it has to be able to say what the weather is without sampling it
    /// somewhere in particular.
    pub fn speed_now(&self) -> f64 {
        self.speed()
    }

    pub fn direction_now(&self) -> f64 {
        self.direction()
    }

    /// **The base is the air mass she is in, which is not always the one she started
    /// in.** With no front this is the day as drawn and nothing has changed; with one it
    /// walks from the air mass ahead to the one behind as the front goes through, and
    /// the trends wander about whatever it has reached.
    ///
    /// That is the whole mechanism. Everything a frontal passage does — the veer, the
    /// building, the clearance — is this one lerp, read through functions that already
    /// existed.
    fn air_mass_at(&self, then: f64) -> (f64, f64, f64) {
        let Some(front) = self.front else {
            return (self.base_speed, self.base_direction, self.cover_mean);
        };
        let through = front.through(then);
        (
            self.base_speed + (front.behind_speed - self.base_speed) * through,
            // The short way round, so a front that veers forty degrees does not back
            // three hundred and twenty instead.
            wrap_tau(
                self.base_direction
                    + crate::units::wrap_pi(front.behind_direction - self.base_direction) * through,
            ),
            self.cover_mean + (front.behind_cover - self.cover_mean) * through,
        )
    }

    fn air_mass(&self) -> (f64, f64, f64) {
        self.air_mass_at(self.since)
    }

    fn speed(&self) -> f64 {
        let base = self.air_mass().0;
        (base * (1.0 + self.speed_trend)).clamp(base * SYNOPTIC_FLOOR, base * SYNOPTIC_CEILING)
    }

    fn direction(&self) -> f64 {
        wrap_tau(self.air_mass().1 + self.direction_trend)
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

    /// What a barometer in this weather would be reading, and what it has done over the
    /// last three hours. See `WindField::barometer` for why a model with no pressure
    /// field can still have an honest glass.
    ///
    /// Read off the *synoptic* wind rather than the gust: a barometer has an hour of lag
    /// in it and no interest at all in the puff that has just come over the mast. The
    /// tendency is the hourly outlook taken over three hours, with the sign the right way
    /// round — building wind is falling glass, and getting that backwards would be a
    /// barometer that tells you to relax as the gale arrives.
    fn barometer(&self) -> (f64, f64) {
        /// What a barometer's tendency *is*: how far the glass has moved in the last
        /// three hours. Not a forecast — the forecast is the `outlook`, and this is the
        /// reading it is taken off.
        const TENDENCY_OVER: f64 = 3.0 * 3_600.0;
        let now = self.pressure_at(0.0);
        (now, now - self.pressure_at(-TENDENCY_OVER))
    }

    /// The glass this many seconds from now — forwards or back.
    ///
    /// **One formula and one tense**, which it was not. The tendency used to be the
    /// hour's outlook multiplied by three, and that is an extrapolation: fine for a
    /// trend relaxing to a mean, absurd for a front. Measured, a six-hectopascal trough
    /// crossing in twenty minutes came out as *minus thirty-seven hectopascals in three
    /// hours* — a reading no barometer has ever shown, and about four times the fastest
    /// fall recorded outside a hurricane.
    ///
    /// Evaluating the same pressure at two times cannot do that: the answer is bounded
    /// by what the pressure actually is. The trend's past is its expected past, which
    /// for an Ornstein-Uhlenbeck process is the same exponential as its future, because
    /// the process is time-reversible about its mean.
    fn pressure_at(&self, when: f64) -> f64 {
        /// Hectopascals per knot of mean wind: a flat calm under a ridge at 1030, a
        /// fresh breeze at 1010, a gale at 985, a storm at 970.
        const PER_KNOT: f64 = 1.30;
        const CALM: f64 = 1030.0;
        let then = self.since + when;
        let base = self.air_mass_at(then).0;
        let trend = self.speed_trend * (-when.abs() / self.speed_timescale).exp();
        let mean = crate::units::ms_to_kn(base * (1.0 + trend));
        let dip = self.front.map_or(0.0, |f| f.dip(then));
        CALM - PER_KNOT * mean - dip
    }

    /// Expected change over the next hour. For `dX = −X/tau dt + noise` the mean path
    /// is `X exp(−t/tau)`, so this is exact rather than an estimate.
    fn outlook(&self) -> (f64, f64) {
        let decay = |tau: f64| 1.0 - (-3_600.0 / tau).exp();
        let (speed, direction) = (
            -self.base_speed * self.speed_trend * decay(self.speed_timescale),
            -self.direction_trend * decay(self.direction_timescale),
        );
        // **Plus the front, which is the part worth forecasting.** The trends are noise
        // relaxing to a mean and an hour of them is worth a knot either way; a front is
        // the day becoming another day, and it is knowable an hour out because the thing
        // has a time and a speed. Said as the same two numbers, so every reader of the
        // outlook — the console, the fishing, the watch — gets the front without being
        // told about fronts.
        let Some(front) = self.front else {
            return (speed, direction);
        };
        let moved = front.through(self.since + 3_600.0) - front.through(self.since);
        (
            speed + (front.behind_speed - self.base_speed) * moved,
            direction + crate::units::wrap_pi(front.behind_direction - self.base_direction) * moved,
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
    /// **A life of zero is a division that returns NaN, and `clamp` does not catch it.**
    /// `0.0 / 0.0` is NaN, and Rust's `f64::clamp` propagates a NaN input rather than
    /// clamping it — so one squall with no life in it put NaN through the maturity curve,
    /// the depth, the outflow and finally the wind at *every* boat including the reader's
    /// own, with nothing anywhere to stop it or to say what had happened.
    ///
    /// The wire refuses such a squall before it gets here (`Sane` in `sail-net`), and the
    /// model has never made one. This is the floor under both: a quantity about to go
    /// underneath a division is held above zero where the division is, not where the
    /// value came from.
    pub fn maturity(&self) -> f64 {
        let u = (self.age / self.life.max(f64::EPSILON)).clamp(0.0, 1.0);
        (std::f64::consts::PI * u).sin().powf(0.6)
    }

    /// How deep inside it a point is: 1 at the centre of the core, 0 at the edge and
    /// beyond. This is what the rain and the visibility are drawn from.
    pub fn depth(&self, pos: DVec2) -> f64 {
        // Held above zero for the same reason as `maturity`, and in the same place.
        let r = (pos - self.at).length() / self.radius.max(f64::EPSILON);
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

/// The planetary wind bands, laid across a flat sea.
///
/// **The cheapest half of a round-the-world game.** The fantasy is a real one and it is
/// the oldest in sailing: leave, never beat once, and come back from the other side.
/// What that needs is not a globe — a globe costs the horizon, and the arithmetic is
/// brutal, `√(2Rh)` — it needs *the trades*: wind that depends on where you are rather
/// than only on what the weather is doing. A sailor crossing from one belt into the next
/// has the wind go light, hang about, and then fill in from the opposite quarter, and
/// that is a passage rather than a weather report.
///
/// So the bands go in first, in the flat world, where they cost nothing and can be
/// sailed through this afternoon. If they read well, wrapping the world east to west is
/// mechanical. If they do not, a sphere was never going to save them.
///
/// The model is one cosine and it is not a simplification of the real thing so much as
/// the shape of it:
///
/// * the trade belt, wind from the north-east, steady;
/// * the horse latitudes, where the two systems meet and the wind goes light and
///   variable — and which fall out of the arithmetic rather than being written in,
///   because a flow vector interpolating between two opposite flows passes through
///   nothing on the way;
/// * the westerly belt, wind from the south-west, stronger;
/// * and then the pattern repeats, which is what a planet does.
///
/// It is the northern hemisphere, because the rest of the model is: the gusts veer
/// right, the squalls track the way they track here, and the sun is drawn for fifty-
/// seven north.
#[derive(Clone, Copy, Debug)]
pub struct Bands {
    /// Metres from one trade belt to the next: a whole cycle, trades to westerlies and
    /// back. Earth's is about five thousand kilometres; a world you can sail round in an
    /// afternoon wants tens of them, and this is the knob that decides which.
    pub spacing: f64,
    /// Which way the belts run, as a bearing. North-south belts are what a planet has;
    /// the knob exists because a test that can turn the pattern sideways is a test that
    /// can tell a band apart from a coincidence.
    pub along: f64,
}

/// How much of the wind is the *day* rather than the belt it is blowing in.
///
/// A quarter. Not a fudge factor and not taste: without it the horse latitudes are a
/// line of dead calm, which is both wrong — they are light and variable, not empty —
/// and the one regime this model is known to handle badly. A quarter of the day's own
/// wind, blowing the day's own way, is what keeps the boat moving across the gap and is
/// what makes crossing it a decision rather than a wall.
const THE_DAY_ITSELF: f64 = 0.25;

/// How square the belts are.
///
/// A pure cosine makes the calms as wide as the trades, which is not what a planet
/// looks like: the belts are broad and the gaps between them are narrow. Raising the
/// cosine to a power below one flattens the tops and tightens the crossings, and
/// four-tenths puts about a fifth of the cycle in the light stuff — which is the Atlantic
/// in about the right proportion.
const HOW_SQUARE: f64 = 0.4;

impl Bands {
    /// The mean flow the belts want at a point, as a vector, given the day's own wind.
    ///
    /// A vector rather than a bearing and a speed on purpose. Interpolating a *bearing*
    /// between the trades and the westerlies swings it through a hundred and eighty
    /// degrees of wind that never blows and leaves the speed up at full strength all the
    /// way across; interpolating the flow takes it through nothing, which is the calm
    /// belt, and the direction change happens where the wind is too light to care about.
    /// The thing that is hard to model is the thing the arithmetic does for free.
    pub fn flow_at(&self, pos: DVec2, day_direction: f64, day_speed: f64) -> DVec2 {
        // Which belt, as an angle round the cycle. Measured along whichever way the
        // belts are laid, which is north-south on a planet.
        let across = pos.x * self.along.sin() + pos.y * self.along.cos();
        let phase = std::f64::consts::TAU * across / self.spacing.max(1.0);
        // Minus one deep in the trades, plus one deep in the westerlies, nothing
        // between.
        let swing = -phase.cos();
        let shaped = swing.signum() * swing.abs().powf(HOW_SQUARE);
        // The westerly flow runs towards the north-east, the trade flow towards the
        // south-west, and `shaped` picks which and how hard.
        let toward = self.along + crate::units::rad(60.0);
        let belt = DVec2::new(toward.sin(), toward.cos()) * shaped * day_speed;
        // And the day underneath it, which is what blows in the gaps.
        flow_from(day_direction, day_speed) * THE_DAY_ITSELF + belt * (1.0 - THE_DAY_ITSELF)
    }

    /// What a sailor would call where he is, for the instruments to say out loud.
    ///
    /// Worth having as words rather than as a number: "the trades" is a thing you plan a
    /// passage around and 0.37 of a cycle is not.
    pub fn name(&self, pos: DVec2) -> &'static str {
        let across = pos.x * self.along.sin() + pos.y * self.along.cos();
        let phase = std::f64::consts::TAU * across / self.spacing.max(1.0);
        let swing = -phase.cos();
        if swing.abs() < 0.25 {
            "the horse latitudes"
        } else if swing < 0.0 {
            "the trades"
        } else {
            "the westerlies"
        }
    }
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
    /// The planetary belts, when the world has them. `None` is one wind everywhere,
    /// which is what this has always been and what every validated number was measured
    /// in.
    pub bands: Option<Bands>,

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
        let synoptic = Synoptic::new(mean_direction, mean_speed, seed, &mut rng);
        Self {
            mean_direction: synoptic.direction(),
            mean_speed: synoptic.speed(),
            synoptic,
            gusts: GustField::new(seed ^ 0x6057),
            direction_spread: 6.0_f64.to_radians(),
            shift_timescale: 90.0,
            veer_coupling: 7.0_f64.to_radians(),
            // One wind everywhere unless somebody asks for a planet.
            bands: None,
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
        (self.mean_at(pos).1 * (1.0 + self.gust_at(pos, t))).max(0.0)
    }

    /// The mean wind at a point before the gusts: the day's own, or what the belt it is
    /// blowing in makes of it.
    ///
    /// One function because the speed and the direction have to come from the same
    /// vector — working them out separately is how a wind ends up blowing at one speed
    /// and from a direction a quarter of a mile away.
    fn mean_at(&self, pos: DVec2) -> (f64, f64) {
        match self.bands {
            None => (self.mean_direction, self.mean_speed),
            Some(bands) => {
                let flow = bands.flow_at(pos, self.mean_direction, self.mean_speed);
                (direction_of(flow), flow.length())
            }
        }
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
            self.mean_at(pos).0
                + self.direction_anomaly
                + self.veer_coupling * self.gust_at(pos, t),
        )
    }

    /// The weather as somebody else has it. See [`Synoptic::follow`].
    ///
    /// The squalls come over with it, because a squall is the opposite of a pure function
    /// of the seed: it is born at a time, it is carried downwind, and it dies. One of
    /// those arriving for you and not for the boat half a mile away would be the most
    /// visible possible way for two people to be in different weather — you would be
    /// reefing in the sunshine.
    pub fn follow(&mut self, speed: f64, direction: f64, cover: f64, squalls: Vec<Squall>) {
        self.synoptic.follow(speed, direction, cover);
        self.mean_speed = self.synoptic.speed();
        self.mean_direction = self.synoptic.direction();
        self.squalls = squalls;
        // And stop making our own, which would be weather nobody else has.
        self.next_squall = f64::INFINITY;
    }

    /// The squalls as they stand, with their ages, for handing to somebody else.
    pub fn squall_state(&self) -> &[Squall] {
        &self.squalls
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
    fn adjustable(&mut self) -> Option<&mut SyntheticWind> {
        Some(self)
    }

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

    fn band_at(&self, pos: DVec2) -> Option<&'static str> {
        self.bands.map(|b| b.name(pos))
    }

    fn outlook(&self) -> (f64, f64) {
        self.synoptic.outlook()
    }

    fn barometer(&self) -> (f64, f64) {
        self.synoptic.barometer()
    }

    fn cover(&self) -> f64 {
        self.synoptic.cover()
    }

    fn fog(&self) -> f64 {
        self.synoptic.fog()
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
    use crate::units::{deg, kn_to_ms, ms_to_kn, rad};

    /// **A wind asked for is a wind blowing.** The base speed on its own is the day the
    /// trends wander about, so changing it leaves whatever trend was in it — up to three
    /// quarters high, nearly half low, relaxing over forty minutes. A gale wound down to
    /// a fresh breeze would stay most of a gale for the next half hour while the glass
    /// and the forecast both said it had eased, which is how the trailer's reel lost a
    /// Code 0: the watch set light cloth on the easing it could read and the wind that
    /// had not eased took it out of her.
    #[test]
    fn a_wind_set_by_hand_is_the_wind_on_the_water() {
        let mut rng = Pcg32::new(0xDECADE);
        let mut day = Synoptic::new(deg(240.0), kn_to_ms(34.0), 7, &mut rng);
        // Wind it up to the ceiling, the way a gale's own trend would.
        day.follow(kn_to_ms(34.0) * SYNOPTIC_CEILING, deg(240.0), 0.75);
        let before = ms_to_kn(day.speed_now());
        assert!(
            before > 50.0,
            "the trend should be high, and it is {before:.0} kn"
        );

        day.blows(kn_to_ms(13.5));
        let after = ms_to_kn(day.speed_now());
        assert!(
            (after - 13.5).abs() < 0.1,
            "asked for 13.5 kn and got {after:.1} kn"
        );

        // And setting the base alone is the lag this exists to avoid — kept as a
        // measurement rather than a memory, so the day it stops being true somebody is
        // told.
        let mut lagging = Synoptic::new(deg(240.0), kn_to_ms(34.0), 7, &mut rng);
        lagging.follow(kn_to_ms(34.0) * SYNOPTIC_CEILING, deg(240.0), 0.75);
        lagging.base_speed = kn_to_ms(13.5);
        let still = ms_to_kn(lagging.speed_now());
        assert!(
            still > 20.0,
            "the base alone should leave it blowing hard, and it is {still:.0} kn"
        );
    }

    /// **Fog does not survive a breeze**, which is the one relation about it a sailor
    /// knows without being told — and the reason a foggy morning burns off the moment
    /// the sea breeze fills in. Below about eight knots it sits; by twenty it has gone.
    #[test]
    fn the_wind_blows_it_away() {
        let thick = |knots: f64| {
            let mut rng = Pcg32::new(0xF0660);
            let mut day = Synoptic::new(deg(180.0), kn_to_ms(knots), 11, &mut rng);
            // `blows` rather than the base alone, or the wind in this test is the wind
            // asked for times whatever trend the dice handed out — which is up to three
            // quarters either way and makes a test about wind speed a test about luck.
            day.blows(kn_to_ms(knots));
            day.start_fog(0.9);
            day.fog()
        };
        let calm = thick(5.0);
        assert!(calm > 0.85, "a calm morning kept only {calm:.2} of its fog");
        assert!(thick(14.0) < calm, "a working breeze thinned nothing");
        assert!(
            thick(22.0) < 0.01,
            "twenty-two knots left {:.2} of it",
            thick(22.0)
        );
        // And it is a slope rather than a switch: the middle is the middle.
        let middle = thick(14.0);
        assert!(
            (0.1..0.75).contains(&middle),
            "fourteen knots gave {middle:.2}"
        );
    }

    /// **Adding fog must not change anybody's day.** The synoptic process, the squalls
    /// and the gusts all draw from one stream, so a new quantity taking a number out of
    /// it shifts every number after — and every seed in the game then means a different
    /// afternoon. That is not hypothetical: it happened, and what caught it was a crew
    /// scenario with nothing to do with weather at all.
    #[test]
    fn fog_draws_from_its_own_dice() {
        let mut rng = Pcg32::new(0xDEC1DE);
        let day = Synoptic::new(deg(200.0), kn_to_ms(16.0), 3, &mut rng);
        // Whatever the shared stream had left after building the day is what the squalls
        // and the gust field get. Two days built the same way must leave it in the same
        // place — and must leave it where a day with no fog in it would have.
        let after_fog = rng.next_f64();

        let mut plain = Pcg32::new(0xDEC1DE);
        // The three draws a synoptic makes of the shared stream: speed, direction, cover.
        for _ in 0..3 {
            let _ = plain.next_normal();
        }
        assert_eq!(
            after_fog,
            plain.next_f64(),
            "fog took a number out of the shared stream and moved the weather"
        );
        assert!(day.fog() >= 0.0);
    }

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

    /// Following somebody else's weather has to survive the next step.
    ///
    /// This is the test the first attempt needed. The game wrote the server's wind
    /// straight into `mean_speed` and `mean_direction`, which looked right and did
    /// nothing: `step` recomputes both from the synoptic trend two hundred times a
    /// second, so the number handed over lived for five milliseconds and was then
    /// replaced by the weather that machine was having anyway. Both boats sailed on in
    /// their own breeze while the code claimed they were in one.
    #[test]
    fn a_wind_told_to_follow_another_one_keeps_following_it() {
        let mut wind = SyntheticWind::new(rad(200.0), kn_to_ms(12.0), 7);
        let asked_speed = kn_to_ms(21.0);
        let asked_from = rad(250.0);
        wind.follow(asked_speed, asked_from, 0.8, Vec::new());
        assert!(
            (wind.synoptic.speed_now() - asked_speed).abs() < 0.05,
            "it should arrive at once: {:.2} against {asked_speed:.2}",
            wind.synoptic.speed_now()
        );

        // And still be there a minute later, having been stepped the way the simulation
        // steps it.
        for _ in 0..12_000 {
            wind.step(0.005);
        }
        let drift = ms_to_kn(wind.synoptic.speed_now() - asked_speed).abs();
        assert!(
            drift < 0.6,
            "a minute on it had wandered {drift:.2} kn from the wind it was given"
        );
        let veered = deg(crate::units::wrap_pi(
            wind.synoptic.direction_now() - asked_from,
        ))
        .abs();
        assert!(veered < 4.0, "and {veered:.1} degrees");

        // The direct route, for the record: written to the mean, gone on the first step.
        let mut naive = SyntheticWind::new(rad(200.0), kn_to_ms(12.0), 7);
        naive.mean_speed = asked_speed;
        naive.step(0.005);
        assert!(
            (naive.mean_speed - asked_speed).abs() > 1.0,
            "writing the mean directly should not survive a step — if this ever passes, \
             the weather sync could have been four lines after all"
        );
    }

    /// Squalls travel with their ages, because a squall is the one piece of weather that
    /// is not a function of the seed.
    #[test]
    fn squalls_come_across_with_their_lives() {
        let mut wind = SyntheticWind::new(rad(200.0), kn_to_ms(18.0), 3);
        let theirs = vec![Squall {
            at: DVec2::new(400.0, -250.0),
            radius: 320.0,
            strength: 0.55,
            age: 90.0,
            life: 300.0,
        }];
        wind.follow(kn_to_ms(18.0), rad(200.0), 0.5, theirs);
        let here = wind.squall_state();
        assert_eq!(here.len(), 1);
        // Half-grown rather than new-born: the age is what says so.
        assert!(
            here[0].maturity() > 0.5,
            "a squall copied across without its age arrives as a baby: {:.2}",
            here[0].maturity()
        );
        // And this machine stops inventing its own, which nobody else would have.
        for _ in 0..200_000 {
            wind.step(0.005);
        }
        assert!(
            wind.squall_state().len() <= 1,
            "a client following somebody else's weather should not be making squalls"
        );
    }
}
