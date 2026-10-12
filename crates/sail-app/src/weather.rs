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
        let tws = kn_to_ms(knots);

        let wind_from = rng.next_f64() * std::f64::consts::TAU;

        // Either tack, any sensible point of sail.
        let magnitude = rad(TWA_MIN_DEG + rng.next_f64() * (TWA_MAX_DEG - TWA_MIN_DEG));
        let start_twa = if rng.next_f64() < 0.5 {
            magnitude
        } else {
            -magnitude
        };

        // Log-uniform fetch.
        let f = rng.next_f64();
        let fetch = FETCH_MIN * (FETCH_MAX / FETCH_MIN).powf(f);
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

        Self {
            seed,
            tws,
            wind_from,
            fetch,
            hs,
            tp,
            start_twa,
            start_reef,
        }
    }

    /// The forecast, as a watch leader would hand it over.
    pub fn report(&self) -> String {
        format!(
            "{:.0} kn from {:03.0}, {} — sea {:.1} m at {:.1} s over {:.0} km of fetch; \
             starting {} on {} tack{} [seed {}]",
            ms_to_kn(self.tws),
            deg(self.wind_from),
            beaufort(ms_to_kn(self.tws)),
            self.hs,
            self.tp,
            self.fetch / 1000.0,
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
            self.seed,
        )
    }
}

/// The scale a sailor actually thinks in.
fn beaufort(knots: f64) -> &'static str {
    match knots {
        k if k < 4.0 => "force 2, light breeze",
        k if k < 7.0 => "force 3, gentle breeze",
        k if k < 11.0 => "force 4, moderate",
        k if k < 17.0 => "force 5, fresh",
        k if k < 22.0 => "force 6, strong",
        k if k < 28.0 => "force 7, near gale",
        k if k < 34.0 => "force 8, gale",
        _ => "force 9, severe gale",
    }
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
