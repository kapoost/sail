//! What the day turns out to be.
//!
//! A passage simulator whose weather is a constant is a tech demo. The point of the
//! genre is that you are handed conditions and have to deal with them, and the skill
//! being exercised — reef early or carry on, which sail plan, which point of sail is
//! actually tolerable — only exists if you did not choose the weather yourself.
//!
//! Two rules kept this honest.
//!
//! **The sea is not an independent dial.** Wave height is a consequence of wind
//! speed, fetch and how long the wind has been blowing, and drawing them separately
//! would let the generator produce three metres at six knots or flat water at thirty,
//! either of which a sailor spots instantly. So the wind is drawn, and the *sea* is
//! drawn as a fetch — how much open water is upwind of you — which is a real property
//! of where you are rather than a fudge factor. `fetch_limited_sea` then decides the
//! height and period, and they come out consistent because they are computed from the
//! same physics the waves themselves use.
//!
//! **A skipper leaves harbour having looked at the forecast.** Drawing twenty-eight
//! knots and handing over a boat under full main is not a challenge, it is a broken
//! starting position: she is overpowered before the first frame, rounds up, and the
//! autopilot gives the helm back within half a minute. So the reef that goes in before
//! the mooring lines are off is part of the draw.

use bevy::prelude::*;
use sail_core::autopilot::{reef_for_heel, OVERPOWERED_HEEL};
use sail_core::prelude::*;
use sail_core::rng::Pcg32;
use sail_core::water::fetch_limited_sea;

/// Mean wind speed of the draw, knots.
///
/// Seventeen, up from fourteen. The spread around it does the work: the median comes
/// out at sixteen knots, a solid force five, with a force seven in the top decile and
/// a metre of sea at the median against three quarters before.
const MEAN_WIND_KN: f64 = 17.0;
/// Weibull shape. Two is the Rayleigh case, which is the standard description of wind
/// speed statistics and gives the long tail that makes a hard day feel earned.
const WIND_SHAPE: f64 = 2.0;
/// Sailable band, knots.
///
/// The floor is nine rather than five, and that is half taste and half a measurement.
/// A ghosting day is a real day, but it is also the one regime where the model is
/// known to be wrong: `examples/delivery.rs` shows the sea costing far too much speed
/// in light air — thirty-one percent at eight knots where ten would be generous — so
/// a drawn calm spends the player's time in the only conditions the physics mishandles
/// badly. The floor comes back down when frequency-dependent radiation damping does.
///
/// The ceiling is where a boat this size is out by mistake rather than by choice.
const WIND_MIN_KN: f64 = 9.0;
const WIND_MAX_KN: f64 = 34.0;

/// And how far `SAIL_WIND` may go, which is a different question.
///
/// **A drawn day is capped; a dial is a question.** The ceiling above is about what the
/// game hands somebody who did not ask — thirty-four knots is where a boat this size is
/// out by mistake rather than by choice, and no draw should exceed it. But a dial exists
/// to ask the model what something looks like, and "what does a fifteen-metre sea
/// actually look like" is a question the water can answer and the clamp was refusing on
/// the boat's behalf.
///
/// Ninety knots is a hurricane and is where the answers stop being about sailing.
/// Nothing is promised about the boat up there — she is a cork — but the sea is the
/// same spectrum it always was, and that is the thing being asked about.
const WIND_ASKED_MAX_KN: f64 = 90.0;

/// Fetch band, metres: a sheltered bay at one end, a long open coast at the other.
///
/// Twelve kilometres is a sheltered bay with a decent run across it, and the floor is
/// there for that reason and no other.
///
/// It was nearly set much higher, to steer around the pitch-resonance trap the sim
/// notes used to warn about: twelve knots over twenty kilometres builds half a metre
/// at a 2.9 second period, which sits on this hull's 2.8 second pitch resonance. The
/// sweep says that trap is gone — bounding the added resistance in waves closed it,
/// and the speed loss is now roughly flat across fetch rather than spiking on
/// resonance. Steering the draw away from it would have been a retreat from a
/// weakness that no longer exists.
///
/// What the same sweep *did* find is a real one, and it is not about fetch at all: in
/// light air the sea costs far too much. Eight knots with a 0.42 m sea loses 31% of
/// boat speed where ten would be generous, and the penalty is worst exactly where the
/// boat has least power to spare. Since the draw puts the player under eight knots
/// about one launch in five, this is now a calibration that matters rather than a
/// footnote. The fix is the one already named: frequency-dependent radiation damping
/// instead of a fitted curve.
///
/// Drawn logarithmically, because the difference between eight and twenty kilometres
/// is a different sea and the difference between two hundred and three hundred is not.
const FETCH_MIN: f64 = 12_000.0;
const FETCH_MAX: f64 = 320_000.0;

/// How long the reef decision lets the boat settle, seconds. Shorter than the polar's
/// four hundred: this only has to separate "she stands up" from "she is on her ear".
const REEF_SETTLE: f64 = 90.0;

/// Points of sail to start on. Never head to wind, never dead downwind: both are
/// states you sail *into*, and starting there is a puzzle rather than a passage.
const TWA_MIN_DEG: f64 = 35.0;
const TWA_MAX_DEG: f64 = 150.0;

/// The conditions for this run, and the seed that produced them.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Weather {
    pub seed: u64,
    /// True wind speed, m/s.
    pub tws: f64,
    /// Direction the wind comes *from*, radians from north.
    pub wind_from: f64,
    /// Fetch, metres.
    pub fetch: f64,
    /// Significant wave height, metres, and peak period, seconds — consequences of
    /// the three above, kept here so the forecast can be stated without recomputing.
    pub hs: f64,
    pub tp: f64,
    /// Local time at the start, hours since midnight.
    ///
    /// **Morning, mostly — and one day in five, the dark.** It used to be morning always,
    /// and the reasoning was that a delivery leg starts at a civilised hour and the sun
    /// should climb through the session rather than set in it; night sailing was called a
    /// different game, needing instrument lights, navigation lights and a sea you cannot
    /// read, and it was not written.
    ///
    /// It is written now. She carries her side lights and a stern light, the coast
    /// flashes its own characteristics, the chart goes under a dim lamp, and there is a
    /// moon at the antisolar point with a path under it on the water. So the draw opens
    /// the other end of the day: four runs in five still start between half past six and
    /// half past eight, and the fifth starts in the dark somewhere between ten at night
    /// and three in the morning.
    ///
    /// One in five rather than half, because a passage you cannot see is the *special*
    /// one. A game that handed out night every other session would be a game about
    /// night, and this one is about sailing.
    pub start_hour: f64,
    /// Cloud cover the day settles about, 0 clear to 1 solid.
    pub cover: f64,
    /// Mean interval between squalls, seconds, or infinite for a day with none.
    ///
    /// A property of the day rather than of the wind model. Some days are a procession
    /// of them and most are not, and which kind of day it is, is exactly the thing a
    /// skipper wants out of a forecast — it decides whether you keep the reef handy.
    pub squall_interval: f64,
    /// True wind angle to start on, radians, signed.
    pub start_twa: f64,
    /// Reef already in before casting off.
    pub start_reef: f64,
}

impl Weather {
    /// Draw a day. The seed is reported so a bad one can be sailed again on purpose.
    pub fn draw(seed: u64) -> Self {
        let mut rng = Pcg32::new(seed);

        // Weibull by inverse transform. The scale is set so the mean comes out where
        // intended: for shape 2 the mean is scale * gamma(1.5) = scale * 0.8862.
        let scale = MEAN_WIND_KN / 0.886_227;
        let u = rng.next_f64().clamp(1e-9, 1.0 - 1e-9);
        let knots =
            (scale * (-(1.0 - u).ln()).powf(1.0 / WIND_SHAPE)).clamp(WIND_MIN_KN, WIND_MAX_KN);
        // `SAIL_WIND=9` for a day with nine knots in it, whatever the draw said.
        //
        // The same family as `SAIL_SQUALL` and `SAIL_SEA`: every question about what the
        // water looks like in a given breeze begins with hunting for a seed that happens
        // to have that breeze in it, which is a slow way to ask a question with a number
        // in it.
        let knots = crate::knob::var("SAIL_WIND")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .map_or(knots, |k| k.clamp(WIND_MIN_KN, WIND_ASKED_MAX_KN));
        let tws = kn_to_ms(knots);

        let wind_from = rng.next_f64() * std::f64::consts::TAU;

        // Either tack, any sensible point of sail.
        let magnitude = rad(TWA_MIN_DEG + rng.next_f64() * (TWA_MAX_DEG - TWA_MIN_DEG));
        let start_twa = if rng.next_f64() < 0.5 {
            magnitude
        } else {
            -magnitude
        };

        // Log-uniform fetch — or `SAIL_FETCH`, in kilometres, for the same reason
        // `SAIL_WIND` exists.
        //
        // **Fetch is what makes a sea big, more than wind is.** Thirty-four knots over
        // the three hundred kilometres this archipelago can offer is three and a half
        // metres; the same wind over the fifteen hundred a Southern Ocean low has behind
        // it is nearly twice that, and at fifty knots fully developed it is fifteen. The
        // draw stays inside what a shelf sea has, because that is where this game is
        // sailed. The dial does not, because the question is about the water.
        let fetch = crate::knob::var("SAIL_FETCH")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .map(|km| (km * 1_000.0).clamp(1_000.0, 4_000_000.0))
            .unwrap_or_else(|| {
                let f = rng.next_f64();
                FETCH_MIN * (FETCH_MAX / FETCH_MIN).powf(f)
            });
        let (hs, tp) = fetch_limited_sea(tws, fetch);

        // What a skipper would already have tied in. Asked of the model rather than
        // guessed from a table: `reef_for_heel` sails the boat at this wind angle and
        // reports the smallest reef that keeps her inside her righting moment. It
        // costs a fraction of a second once, at launch, which is the right place to
        // spend it — the alternative is handing over a boat that is overpowered in
        // frame one, rounds up, and loses the autopilot inside half a minute.
        let wind_speed = tws;
        let direction = wind_from;
        let make = move || -> Box<dyn sail_core::wind::WindField> {
            Box::new(sail_core::wind::SteadyWind::new(direction, wind_speed))
        };
        let (start_reef, _) = reef_for_heel(
            BoatConfig::archipelago_packet(),
            &make,
            start_twa.abs(),
            REEF_SETTLE,
            OVERPOWERED_HEEL,
        );

        // The hour and the cloud. Both drawn last, with the squalls, for the same
        // reason: a new call earlier in the sequence shifts every draw after it.
        // Six thirty to eight thirty, which at this latitude in spring is a sun between
        // eight and twenty-three degrees up. Deliberately the low end: a low sun is what
        // gives the long cloud shadows, the shafts under the squalls and the warm light
        // the whole look was built on, and it climbs through the session anyway.
        // Four days in five a morning; the fifth, a night watch. `SAIL_HOUR` overrides
        // either, which is how both are looked at.
        let start_hour = if rng.next_f64() < 0.2 {
            // Ten at night to three in the morning, wrapped round midnight: the hours
            // when there is nothing but the lights, and the ones a pilot earns his fee
            // in.
            (22.0 + rng.next_f64() * 5.0) % 24.0
        } else {
            6.5 + rng.next_f64() * 2.0
        };
        // `SAIL_HOUR=20.5` starts her at half past eight in the evening.
        //
        // The same kind of knob as `SAIL_SQUALL`: the sky's best hours are the two at
        // either end of the day, and waiting twelve hours of real time to look at a
        // sunset is not a way to work on one. Read here rather than where the sky is
        // drawn so that everything downstream of the hour — the light, the cloud
        // tones, the haze, the sea's own glints — agrees about what time it is.
        let start_hour = crate::knob::var("SAIL_HOUR")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .map(|h| h.rem_euclid(24.0))
            .unwrap_or(start_hour);
        // Beta-ish: most days are broken cloud and the extremes are rarer, which is
        // what a coastal sky does. Two draws averaged is the cheapest way to get that
        // shape and it is exact, not an approximation of one.
        let cover = 0.5 * (rng.next_f64() + rng.next_f64()) * 0.9 + 0.08;
        // `SAIL_COVER=0.2` for a nearly clear day. The companion to `SAIL_HOUR`: the
        // sky and the cloud in front of it are two things to look at and they have to
        // be separable, or every judgement about one is really a judgement about how
        // much of the other happened to be drawn.
        let cover = crate::knob::var("SAIL_COVER")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .map(|c| c.clamp(0.0, 1.0))
            .unwrap_or(cover);

        // How squally. Three kinds of day, in roughly the proportion a coast hands them
        // out: most are clear of them, some have one or two go by, and a few are one
        // after another. Drawn last on purpose — a new call anywhere earlier shifts
        // every draw after it, and every seeded day anyone has sailed becomes a
        // different day. Which it did once, between writing this and reading it back.
        let squall_interval = match rng.next_f64() {
            u if u < 0.55 => f64::INFINITY,
            u if u < 0.88 => 2_400.0,
            _ => 900.0,
        };

        Self {
            seed,
            start_hour,
            cover,
            tws,
            wind_from,
            fetch,
            hs,
            tp,
            squall_interval,
            start_twa,
            start_reef,
        }
    }

    /// The forecast, as a watch leader would hand it over.
    pub fn report(&self) -> String {
        format!(
            "{:02.0}{:02.0}, {} oktas — {:.0} kn from {:03.0}, {}; sea {:.1} m at {:.1} s \
             over {:.0} km of fetch ({}); starting {} on {} tack{}{} [seed {}]",
            self.start_hour.floor(),
            self.start_hour.fract() * 60.0,
            (self.cover * 8.0).round(),
            ms_to_kn(self.tws),
            deg(self.wind_from),
            beaufort(ms_to_kn(self.tws)),
            self.hs,
            self.tp,
            self.fetch / 1000.0,
            sea_state(self.hs),
            point_of_sail(self.start_twa),
            if self.start_twa > 0.0 {
                "starboard"
            } else {
                "port"
            },
            if self.start_reef > 0.01 {
                format!(", {:.0}% reef already tied in", self.start_reef * 100.0)
            } else {
                String::new()
            },
            match self.squall_interval {
                i if i.is_infinite() => "",
                i if i <= 1_200.0 => ", squally",
                _ => ", the odd squall about",
            },
            self.seed,
        )
    }
}

/// Which force this wind is.
///
/// Beaufort is a *wind* scale and it is the one every sailor thinks in — nobody says
/// "seventeen knots", they say "a fresh breeze" or "a five". The bands are the real
/// ones, and the boundaries matter: force six is where a cruising boat puts a reef in,
/// force seven is where she puts the second one, and force eight is where she stops
/// going where she wanted to go.
pub fn force(knots: f64) -> u8 {
    match knots {
        k if k < 1.0 => 0,
        k if k < 4.0 => 1,
        k if k < 7.0 => 2,
        k if k < 11.0 => 3,
        k if k < 17.0 => 4,
        k if k < 22.0 => 5,
        k if k < 28.0 => 6,
        k if k < 34.0 => 7,
        k if k < 41.0 => 8,
        k if k < 48.0 => 9,
        k if k < 56.0 => 10,
        k if k < 64.0 => 11,
        _ => 12,
    }
}

/// What each force is called.
///
/// Its own table so that [`force`] and [`beaufort`] cannot disagree, which they did:
/// the number came from one list of thresholds and the name from a second list beside
/// it, and the second was shifted by a whole force. Thirty knots was reported as
/// "force 8, gale" by the words and as 7 by the number, in the same program, off the
/// same wind. One table, two readers.
fn named(force: u8) -> &'static str {
    match force {
        0 => "calm",
        1 => "light air",
        2 => "light breeze",
        3 => "gentle breeze",
        4 => "moderate breeze",
        5 => "fresh breeze",
        6 => "strong breeze",
        7 => "near gale",
        8 => "gale",
        9 => "severe gale",
        10 => "storm",
        11 => "violent storm",
        _ => "hurricane",
    }
}

/// A representative wind for a force, knots: the middle of its band.
///
/// For a dial, so that asking for a six gives you a six rather than its edge — a slider
/// that lands on 21.9 knots is a five, and the person who moved it will say the dial is
/// broken rather than that they are a knot short.
pub fn force_wind(force: u8) -> f64 {
    match force.min(12) {
        0 => 0.5,
        1 => 2.0,
        2 => 5.0,
        3 => 9.0,
        4 => 13.5,
        5 => 19.0,
        6 => 24.5,
        7 => 30.5,
        8 => 37.0,
        9 => 44.0,
        10 => 52.0,
        11 => 60.0,
        _ => 70.0,
    }
}

/// What a visibility is called, in the words a forecast uses.
///
/// The international scale, because it is the one on every shipping forecast and the
/// numbers in it already mean something to anybody who has listened to one. "Poor" is
/// where a prudent skipper starts sounding a signal; "fog" is where the chart table
/// stops being optional.
pub fn how_far(metres: f32) -> &'static str {
    match metres {
        m if m < 200.0 => "under a cable",
        m if m < 1_000.0 => "a few hundred metres",
        m if m < 2_000.0 => "a mile at most",
        m if m < 5_000.0 => "two or three miles",
        m if m < 10_000.0 => "a good few miles",
        _ => "all the way to the horizon",
    }
}

/// And what the fog itself is doing, as opposed to how far you can see through it.
///
/// Two different sentences on purpose: a tenth of fog on a clear day still leaves ten
/// kilometres, and the same tenth under a thick overcast leaves a mile. The dial moves
/// the first and the line underneath reports the second.
pub fn how_thick(fog: f64) -> &'static str {
    match fog {
        f if f < 0.05 => "clear",
        f if f < 0.25 => "a haze on the water",
        f if f < 0.55 => "coming and going",
        f if f < 0.85 => "shut in",
        _ => "you hear more than you see",
    }
}

/// What a sea of this height is called.
///
/// Douglas, which is the *sea* scale and is not Beaufort — a point worth keeping
/// straight on a panel that carries both. A force six over twenty miles of fetch and a
/// force six over two hundred are the same wind and quite different afternoons, and the
/// only honest way to say which is to name the sea separately.
pub fn sea_state(hs: f64) -> &'static str {
    match hs {
        h if h < 0.1 => "glassy",
        h if h < 0.5 => "smooth",
        h if h < 1.25 => "slight",
        h if h < 2.5 => "moderate",
        h if h < 4.0 => "rough",
        h if h < 6.0 => "very rough",
        h if h < 9.0 => "high",
        _ => "very high",
    }
}

/// The scale a sailor actually thinks in.
pub fn beaufort(knots: f64) -> String {
    let f = force(knots);
    format!("force {f}, {}", named(f))
}

fn point_of_sail(twa: f64) -> &'static str {
    match deg(twa).abs() {
        a if a < 50.0 => "close hauled",
        a if a < 70.0 => "close reaching",
        a if a < 100.0 => "on a beam reach",
        a if a < 140.0 => "on a broad reach",
        _ => "running",
    }
}

#[cfg(test)]
mod what_the_wind_is_called {
    use super::*;

    /// **The number and the name came from two tables and the tables disagreed.**
    /// Thirty knots was `force(30) == 7` and `beaufort(30) == "force 8, gale"` — the
    /// same wind, in the same program, named a whole force apart, because the words
    /// list had been written beside the numbers list rather than out of it.
    #[test]
    fn the_number_and_the_name_are_the_same_wind() {
        for knots in 0..100 {
            let said = beaufort(knots as f64);
            let number = force(knots as f64);
            assert!(
                said.starts_with(&format!("force {number},")),
                "{knots} kn is force {number} and is called {said:?}"
            );
        }
    }

    /// And the thresholds are the scale as it is actually written down, including the
    /// top end — which only became reachable when `SAIL_WIND` stopped being clamped to
    /// what a drawn day may contain.
    #[test]
    fn the_scale_is_the_one_in_the_almanac() {
        for (knots, expect) in [
            (0.0, 0),
            (3.9, 1),
            (10.0, 3),
            (27.9, 6),
            (30.0, 7),
            (34.0, 8),
            (41.0, 9),
            (48.0, 10),
            (56.0, 11),
            (64.0, 12),
            (90.0, 12),
        ] {
            assert_eq!(force(knots), expect, "{knots} kn");
        }
        assert!(beaufort(80.0).ends_with("hurricane"));
        // And a force asked for comes back as a wind inside that force.
        for f in 0..=12u8 {
            let kn = force_wind(f);
            assert_eq!(force(kn), f, "force {f} asked for gave {kn} kn");
        }
    }
}

#[cfg(test)]
mod scales {
    use super::*;

    /// The force bands have to be the real ones, and the dial has to land inside the
    /// band it says. A slider that gives you 21.9 knots when you asked for a six is a
    /// slider you will say is broken — and it would be right, because 21.9 is a five.
    #[test]
    fn asking_for_a_force_gives_you_that_force() {
        for f in 0..=10u8 {
            let knots = force_wind(f);
            assert_eq!(
                force(knots),
                f,
                "force {f} is quoted as {knots:.1} kn, which reads back as a {}",
                force(knots)
            );
        }
    }

    /// And the boundaries are where the sailing decisions are.
    #[test]
    fn the_bands_are_where_a_sailor_expects_them() {
        assert_eq!(force(16.9), 4, "a fresh breeze starts at seventeen");
        assert_eq!(force(17.0), 5);
        assert_eq!(
            force(21.9),
            5,
            "and a strong breeze at twenty-two — the first reef"
        );
        assert_eq!(force(22.0), 6);
        assert_eq!(force(27.9), 6);
        assert_eq!(force(28.0), 7, "a near gale, and the second reef");
        assert_eq!(
            force(34.0),
            8,
            "a gale, and you are not going where you wanted"
        );
    }

    /// Douglas is the *sea* scale and is not Beaufort, which is a distinction worth
    /// keeping on a panel that carries both: the same force over twenty miles of fetch
    /// and over two hundred is the same wind and quite a different afternoon.
    #[test]
    fn the_sea_is_named_by_its_height() {
        assert_eq!(sea_state(0.05), "glassy");
        assert_eq!(sea_state(0.4), "smooth");
        assert_eq!(sea_state(1.0), "slight");
        assert_eq!(sea_state(2.0), "moderate");
        assert_eq!(sea_state(3.0), "rough");
        assert_eq!(sea_state(5.0), "very rough");
        // A force six over a short fetch is a slight sea; over a long one it is rough.
        let short = sail_core::water::fetch_limited_sea(kn_to_ms(24.5), 15_000.0).0;
        let long = sail_core::water::fetch_limited_sea(kn_to_ms(24.5), 300_000.0).0;
        assert_ne!(
            sea_state(short),
            sea_state(long),
            "the same wind over 15 and 300 km of fetch should not be the same sea              ({short:.1} m against {long:.1} m)"
        );
    }
}
