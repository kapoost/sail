//! The tide: how much water there is, and which way it is going.
//!
//! Two things, and a sailor cares about them for two different reasons. The **height**
//! decides whether the passage over the bar exists at all this afternoon, and it is why
//! a chart gives soundings at the lowest the water ever gets rather than at the water
//! you will find. The **stream** decides whether you get there, and it is the larger of
//! the two in this water: two knots through the sound against a boat that makes five is
//! most of an afternoon, and the same two knots the other way is the reason everybody
//! leaves at the same hour.
//!
//! # Where the numbers come from
//!
//! The height is one constituent, M2 — the lunar semidiurnal, period 12 h 25 m — and
//! that is the whole of it. Real tides are a sum of a hundred of them, and the second
//! one, S2, is what turns the fortnight from springs to neaps and back.
//!
//! **There are no springs and no neaps here, and that is the same decision the moon
//! already made.** See `sky::Sky::moon`: this game has no calendar. Days do not
//! accumulate, every passage is its own day, and so a spring-neap cycle would be a phase
//! pulled out of nothing — a world that was at springs this morning and at neaps this
//! afternoon because you started the game twice. What a world gets instead is *a* tide,
//! drawn once from its seed: some worlds are lively and some are sleepy, and inside one
//! world the tide is the same tide all the way through.
//!
//! The twelve-hour-twenty-five-minute period is the one thing that is not drawn. It is
//! the moon's, it is the same everywhere on earth, and a sailor who has waited out one
//! tide knows exactly how long the next one is.
//!
//! # Why the stream peaks when the height does not
//!
//! Slack water at high and low, and the hardest stream halfway between. That is the
//! standing-wave case — a channel with a closed end, which is what an archipelago is —
//! and it is the arrangement the rule of twelfths describes: a twelfth of the range in
//! the first hour, two in the second, three in the third. The other case, where the
//! stream runs hardest *at* high water, belongs to an open coast with the wave sweeping
//! along it, and it is the one that catches people out on a strange shore. One of the
//! two had to be picked; this is the one the local knowledge in this game is about.
//!
//! # And it crosses the wire for nothing
//!
//! Height and stream are functions of the seed and the clock, and both ends of a game
//! already share those. The same argument as the gust field: a thing both of us can work
//! out does not need a packet, and the trap is the same one — it is only true as long as
//! nothing else is allowed to move it.

use crate::rng::Pcg32;
use glam::DVec2;

/// The lunar semidiurnal period, seconds: 12 h 25 m 14 s.
///
/// 12.4206012 hours. Worth the digits — an hour's error over a passage is the difference
/// between arriving at slack water and arriving at the worst of it.
pub const M2: f64 = 12.420_601_2 * 3_600.0;

/// A world's tide.
///
/// `Copy`, like the weather, because it is read in a dozen places a substep and none of
/// them should be taking a borrow out on a thing this small.
#[derive(Clone, Copy, Debug)]
pub struct Tide {
    /// Half the range, metres: the height swings between plus and minus this.
    amplitude: f64,
    /// Seconds from the start of the passage to the first high water.
    high_at: f64,
    /// How hard the stream runs at its hardest, in open water of ordinary depth, m/s.
    rate: f64,
    /// Which way the flood sets, radians clockwise from north — the direction it runs
    /// *towards*, which is the convention for a stream and the opposite of the wind's.
    set: f64,
}

/// The depth at which the stream is the rate it is named for, metres.
///
/// Continuity, not a fudge: the same water forced through a shallower section has to go
/// faster, which is why a stream that is a knot in the deep part of a sound is three
/// over the bank beside it. The gain is capped — the model is a cross-section, not a
/// coastline, and over two metres of sand it would otherwise claim a river.
const ORDINARY_DEPTH: f64 = 30.0;
/// The most the shallows may multiply it by.
const MOST_IT_RUNS: f64 = 1.9;

impl Tide {
    /// Still water: no height, no stream. What the open-ocean tests and the polar run in.
    pub fn none() -> Self {
        Self {
            amplitude: 0.0,
            high_at: 0.0,
            rate: 0.0,
            set: 0.0,
        }
    }

    /// The tide this world has: **the range belongs to the place and the hour belongs to
    /// the day**, which is two seeds for the same reason the islands and the weather are.
    ///
    /// A coast has the tide it has. The range between its springs and its neaps, the
    /// direction the flood sets through its sounds — those are facts about the water,
    /// they are in the pilot book, and local knowledge is worth having precisely because
    /// they do not change between visits. What *does* change is the hour: casting off at
    /// nine on one morning and at two on another is the whole difference between a fair
    /// tide and a foul one, and a world where high water always fell the same length of
    /// time after you pressed go would be a world with no tidal planning in it at all.
    ///
    /// The range is a metre and a half to three and three quarters, which is an ordinary
    /// European shelf coast and not the Severn. The rate is drawn with it rather than
    /// separately, because the two are the same water: a world with a big range moves
    /// more of it through the same gaps in the same six hours, and a tide that rose four
    /// metres without a stream to show for it would be the one thing in the model a
    /// sailor would not believe.
    pub fn of(place: u64, day: u64) -> Self {
        let mut coast = Pcg32::new(place ^ 0x71_DE_11);
        let amplitude = 0.75 + 1.1 * coast.next_f64();
        let set = coast.next_f64() * std::f64::consts::TAU;
        let mut clock = Pcg32::new(day ^ 0x71_DE_C1);
        Self {
            amplitude,
            // Anywhere in the cycle. A passage that always opened at slack water would
            // be a passage with no tide in it.
            high_at: clock.next_f64() * M2,
            // A metre a second — near enough two knots — at the top of the range, and
            // proportionally less below it.
            rate: 0.95 * (amplitude / 1.3),
            set,
        }
    }

    /// Height above mean level, metres. Positive is more water than the chart promises.
    pub fn height(&self, t: f64) -> f64 {
        self.amplitude * self.angle(t).cos()
    }

    /// The whole range, low to high, metres. What a tide table would print.
    pub fn range(&self) -> f64 {
        2.0 * self.amplitude
    }

    /// The stream, metres per second, as a vector over the ground.
    ///
    /// Depth is asked for because the shallows speed it up — see `ORDINARY_DEPTH`. Pass
    /// the charted depth or the depth now; at these gains the difference is noise
    /// against a model this coarse.
    pub fn stream(&self, t: f64, depth: f64) -> DVec2 {
        if self.rate <= 0.0 {
            return DVec2::ZERO;
        }
        // Ninety degrees out of phase with the height: hardest at mid-tide, slack at the
        // top and the bottom. Negative sine so that the flood — the stream that fills
        // the place — is the one running while the water is rising.
        let strength = -self.rate * self.angle(t).sin();
        let gain = (ORDINARY_DEPTH / depth.max(1.0)).sqrt().min(MOST_IT_RUNS);
        DVec2::new(self.set.sin(), self.set.cos()) * strength * gain
    }

    /// Put the tide at a given height *now*, and let the cycle run on from there.
    ///
    /// The same seam as `Synoptic::start_cover` and for the same reason: the dial that
    /// asks for a state of tide and the process that moves it live in different places,
    /// and the honest way to join them is to solve for the phase rather than to hold the
    /// height. Move the clock, not the water.
    ///
    /// **Which half of the cycle it lands in is kept**, so that a hand on the slider
    /// does not turn the flood into the ebb halfway through a drag. A tide run backwards
    /// by a dial is a dial that cannot be used to find out what a falling tide looks
    /// like.
    pub fn put_at(&mut self, height: f64, t: f64) {
        if self.amplitude <= 0.0 {
            return;
        }
        let want = (height / self.amplitude).clamp(-1.0, 1.0).acos();
        let now = self.angle(t).rem_euclid(std::f64::consts::TAU);
        // `acos` gives the rising answer; the falling one is the same angle the other
        // side of high water.
        let want = if now > std::f64::consts::PI {
            std::f64::consts::TAU - want
        } else {
            want
        };
        self.high_at = t - want * M2 / std::f64::consts::TAU;
    }

    /// Half the range: how far from mean level the water gets, metres. What a slider
    /// that asks for a state of tide has to span.
    pub fn amplitude(&self) -> f64 {
        self.amplitude
    }

    /// Where in the cycle it is, radians: zero at high water.
    fn angle(&self, t: f64) -> f64 {
        std::f64::consts::TAU * (t - self.high_at) / M2
    }

    /// Seconds until the next high water, and until the next low.
    ///
    /// What the almanac line on the panel is made of, and the one number anybody plans a
    /// day around.
    pub fn until_high(&self, t: f64) -> f64 {
        let since = (t - self.high_at).rem_euclid(M2);
        if since <= f64::EPSILON {
            0.0
        } else {
            M2 - since
        }
    }

    pub fn until_low(&self, t: f64) -> f64 {
        let since = (t - self.high_at + M2 / 2.0).rem_euclid(M2);
        if since <= f64::EPSILON {
            0.0
        } else {
            M2 - since
        }
    }

    /// Flooding or ebbing, in the word a forecast uses.
    pub fn making(&self, t: f64) -> &'static str {
        let since = (t - self.high_at).rem_euclid(M2);
        // An hour either side of the turn is slack enough to call it that: the stream in
        // that hour is under a quarter of its peak, which is below what you can steer a
        // course to allow for anyway.
        let hour = 3_600.0;
        if since < hour || since > M2 - hour {
            "slack, high water"
        } else if (since - M2 / 2.0).abs() < hour {
            "slack, low water"
        } else if since < M2 / 2.0 {
            "ebbing"
        } else {
            "flooding"
        }
    }
}

impl Default for Tide {
    fn default() -> Self {
        Self::none()
    }
}

#[cfg(test)]
mod the_tide {
    use super::*;

    /// **Twelve hours and twenty-five minutes, and the second high is as high as the
    /// first.** The one property every other claim in this module rests on.
    #[test]
    fn it_comes_round_in_a_lunar_half_day() {
        let tide = Tide::of(1, 4);
        let hw = tide.high_at;
        assert!(
            (tide.height(hw) - tide.height(hw + M2)).abs() < 1e-9,
            "the tide did not come back to where it was a cycle later"
        );
        assert!(
            tide.height(hw) > tide.height(hw + M2 / 2.0),
            "high water was not higher than the low water after it"
        );
        // Twelve and a half hours, to the minute a sailor would use.
        assert!((M2 / 3600.0 - 12.42).abs() < 0.01);
    }

    /// **Slack at the top, hardest halfway down.** The standing-wave case — see the
    /// module header for why that one and not the other.
    #[test]
    fn the_stream_is_slack_when_the_height_is_not_moving() {
        let tide = Tide::of(1, 9);
        let hw = tide.high_at;
        let at_hw = tide.stream(hw, ORDINARY_DEPTH).length();
        let halfway = tide.stream(hw + M2 / 4.0, ORDINARY_DEPTH).length();
        assert!(
            at_hw < 0.02,
            "high water ran at {at_hw:.2} m/s, which is not slack water"
        );
        assert!(
            halfway > 0.5,
            "mid-tide ran at {halfway:.2} m/s, which is not a stream"
        );
    }

    /// **The flood fills, the ebb empties**, and they run opposite ways. Getting this
    /// backwards is the kind of error that only shows up when somebody plans a day
    /// around it, so it is asserted rather than left to the sign of a sine.
    #[test]
    fn the_flood_runs_while_the_water_rises() {
        let tide = Tide::of(1, 11);
        let low = tide.high_at + M2 / 2.0;
        let rising = tide.height(low + 60.0) - tide.height(low);
        assert!(rising > 0.0, "the water was not rising after low water");
        let flood = tide.stream(low + M2 / 4.0, ORDINARY_DEPTH);
        let ebb = tide.stream(low - M2 / 4.0, ORDINARY_DEPTH);
        assert!(
            flood.dot(ebb) < 0.0,
            "the flood and the ebb ran the same way"
        );
        let set = DVec2::new(tide.set.sin(), tide.set.cos());
        assert!(
            flood.dot(set) > 0.0,
            "the flood did not run the way the flood is meant to set"
        );
    }

    /// **Shallow water runs faster**, because the same water has to get through a
    /// smaller gap — and not indefinitely, because this is a cross-section and not a
    /// coastline.
    #[test]
    fn the_shallows_speed_it_up_but_only_so_far() {
        let tide = Tide::of(1, 3);
        let mid = tide.high_at + M2 / 4.0;
        let deep = tide.stream(mid, 60.0).length();
        let bank = tide.stream(mid, 6.0).length();
        let puddle = tide.stream(mid, 0.2).length();
        assert!(bank > deep * 1.5, "the bank ran no faster than the deep");
        assert!(
            puddle <= deep * (MOST_IT_RUNS / (ORDINARY_DEPTH / 60.0f64).sqrt()) + 1e-9,
            "two inches of water claimed a river"
        );
    }

    /// Still water is still: the open-ocean tests and the polar have no tide in them and
    /// must not acquire one.
    #[test]
    fn nothing_is_nothing() {
        let tide = Tide::none();
        for t in [0.0, 1_000.0, 50_000.0] {
            assert_eq!(tide.height(t), 0.0);
            assert_eq!(tide.stream(t, 20.0), DVec2::ZERO);
        }
    }

    /// Two worlds, two tides; the same world twice, the same tide. The network rests on
    /// the second half of that.
    #[test]
    fn a_world_has_one_tide_and_keeps_it() {
        let a = Tide::of(17, 17);
        let b = Tide::of(17, 17);
        let other = Tide::of(18, 18);
        assert_eq!(a.height(4_000.0), b.height(4_000.0));
        assert!(
            (a.height(4_000.0) - other.height(4_000.0)).abs() > 1e-6
                || (a.range() - other.range()).abs() > 1e-6,
            "two seeds drew the same tide"
        );
    }

    /// **The coast keeps its range and the day brings its own hour.** Two seeds, and
    /// the failure this guards against is the quiet one: wire them to the same number
    /// and every passage in a world opens at the same state of tide, which looks like a
    /// working tide right up until somebody notices they never have to plan one.
    #[test]
    fn the_place_sets_the_range_and_the_day_sets_the_hour() {
        let monday = Tide::of(42, 1);
        let tuesday = Tide::of(42, 2);
        let elsewhere = Tide::of(43, 1);
        assert_eq!(
            monday.range(),
            tuesday.range(),
            "the same coast had two different ranges on two days"
        );
        assert!(
            (monday.until_high(0.0) - tuesday.until_high(0.0)).abs() > 600.0,
            "two passages opened at the same state of tide"
        );
        assert!(
            (monday.range() - elsewhere.range()).abs() > 1e-6,
            "two coasts had the same tide"
        );
    }

    /// **The dial moves the clock and not the water**, and it does not turn the tide
    /// round while you are dragging it.
    #[test]
    fn the_dial_sets_the_state_of_tide() {
        let mut tide = Tide::of(6, 2);
        let now = 3_000.0;
        for want in [-0.5, 0.0, 0.4] {
            let height = want * tide.amplitude();
            let falling_before = tide.height(now + 1.0) < tide.height(now);
            tide.put_at(height, now);
            assert!(
                (tide.height(now) - height).abs() < 1e-6,
                "asked for {height:.2} m and got {:.2}",
                tide.height(now)
            );
            let falling_after = tide.height(now + 1.0) < tide.height(now);
            assert_eq!(
                falling_before, falling_after,
                "the dial turned the flood into the ebb"
            );
        }
    }

    /// The next high water is in the future and inside one cycle, which is the whole of
    /// what the almanac line promises.
    #[test]
    fn the_almanac_counts_forward() {
        let tide = Tide::of(1, 5);
        for t in [0.0, 1_234.0, M2 * 3.7] {
            let hw = tide.until_high(t);
            assert!((0.0..=M2).contains(&hw), "next high water was {hw} s away");
            assert!(
                tide.height(t + hw) > tide.height(t + hw + M2 / 4.0) - 1e-9,
                "the moment it called high water was not the top of the tide"
            );
        }
    }
}
