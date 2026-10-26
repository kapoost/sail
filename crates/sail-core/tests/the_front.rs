//! A day that becomes a different day, measured.
//!
//! The weather was drawn once and then wandered about what it had been drawn as. An
//! Ornstein-Uhlenbeck process is a good model of an afternoon and a poor model of a
//! *passage*: over four hours it gives you a knot either way and twenty degrees of
//! wander, and the barometer is a readout of the wind rather than a thing that tells you
//! something you cannot see yet.
//!
//! A front is the other half of weather — the day's air mass being replaced by another
//! one's — and what came out of running it is the sequence every pilot book describes:
//!
//! * **The glass falls ahead of it, bottoms out at the line, and rises behind.** The
//!   tendency reverses about half an hour before the wind does, which is the whole
//!   reason anybody ever looked at a barometer.
//! * **The wind veers through it**, by twenty to seventy degrees, every time — because
//!   that is what the circulation round a northern low does, not because a coin was
//!   tossed.
//! * **It blows harder behind a cold one and the sky clears**; a warm one eases and
//!   leaves it thick.
//! * And the outlook knows about it an hour out, because the thing has a time and a
//!   speed, which is the difference between a forecast and a readout.

use sail_core::units::{deg, kn_to_ms, ms_to_kn, rad, wrap_pi};
use sail_core::wind::{Front, SyntheticWind, WindField};

/// A day with a known front in it, stepped minute by minute.
fn crossing(front: Front) -> Vec<(f64, f64, f64, f64, f64)> {
    let mut wind = SyntheticWind::new(rad(200.0), kn_to_ms(14.0), 11);
    // The trends off, so what is measured is the front and not the noise it arrives in.
    wind.synoptic.blows(kn_to_ms(14.0));
    wind.synoptic.hold_steady();
    wind.synoptic.front = Some(front);
    let mut out = Vec::new();
    for minute in 0..300 {
        let (glass, tendency) = wind.synoptic.glass();
        out.push((
            minute as f64,
            ms_to_kn(wind.synoptic.speed_now()),
            deg(wind.synoptic.direction_now()),
            glass,
            tendency,
        ));
        wind.step(60.0);
    }
    out
}

fn cold() -> Front {
    Front {
        through_at: 7_200.0,
        takes: 1_200.0,
        behind_speed: kn_to_ms(22.0),
        behind_direction: rad(250.0),
        behind_cover: 0.2,
        trough: 6.0,
    }
}

/// **The wind veers through it**, and it is the same forty degrees the air masses
/// differ by. Nothing else in the model can turn the mean wind that far.
#[test]
fn the_wind_veers_through_the_front() {
    let day = crossing(cold());
    let before = day[30].2;
    let after = day[200].2;
    let shift = deg(wrap_pi(rad(after) - rad(before)));
    assert!(
        (shift - 50.0).abs() < 12.0,
        "it went from {before:.0} to {after:.0}, a shift of {shift:.0}"
    );
    assert!(shift > 0.0, "it backed through a front instead of veering");
}

/// **And it blows harder behind a cold one.**
#[test]
fn it_builds_behind_a_cold_front() {
    let day = crossing(cold());
    assert!(
        day[200].1 > day[30].1 + 5.0,
        "{:.0} knots before and {:.0} after",
        day[30].1,
        day[200].1
    );
}

/// **The glass falls, bottoms out at the line and rises behind it** — which is the one
/// instrument reading on this boat that is ahead of the weather. Without the trough the
/// barometer is a wind gauge with different units: it would fall as the wind built and
/// stay down, and nothing about the passing would show in it at all.
#[test]
fn the_glass_bottoms_out_at_the_line() {
    let day = crossing(cold());
    let lowest = day
        .iter()
        .enumerate()
        .min_by(|a, b| a.1 .3.total_cmp(&b.1 .3))
        .map(|(i, _)| i)
        .expect("a day has minutes in it");
    // The front goes through at two hours, and the lowest glass is within a quarter of
    // an hour of it.
    assert!(
        (lowest as i64 - 120).abs() < 15,
        "the glass bottomed out at minute {lowest}, not at the front"
    );
    // Eleven hectopascals of it: a fall nobody would miss on the dial.
    assert!(
        day[30].3 - day[lowest].3 > 8.0,
        "the glass fell only {:.1} hPa",
        day[30].3 - day[lowest].3
    );
    // **And it comes back up behind the line**, which is the half that tells you the
    // worst of it is past. It shows in the glass rather than in the tendency: the
    // three-hour tendency still has the whole fall inside its window, and saying
    // otherwise would be an instrument that lies to flatter a rhyme.
    assert!(
        day[160].3 > day[lowest].3 + 1.0,
        "it did not rise behind the line: {:.1} at the bottom, {:.1} after",
        day[lowest].3,
        day[160].3
    );
    // Falling, hard, as it comes — and never at a rate no barometer has shown. The
    // deepest three-hourly fall on record outside a hurricane is about twenty.
    assert!(day[110].4 < -1.0, "it was not falling ahead of it");
    assert!(
        day.iter().all(|m| m.4 > -20.0),
        "the tendency reached {:.0} hPa in three hours",
        day.iter().map(|m| m.4).fold(f64::MAX, f64::min)
    );
}

/// **The forecast knows an hour before the wind does.** The trends are noise relaxing
/// to a mean and an hour of them is worth a knot; the front is the day becoming another
/// day, and it is knowable because the thing has a time and a speed.
#[test]
fn the_outlook_sees_it_coming() {
    let mut wind = SyntheticWind::new(rad(200.0), kn_to_ms(14.0), 11);
    wind.synoptic.blows(kn_to_ms(14.0));
    wind.synoptic.hold_steady();
    wind.synoptic.front = Some(cold());
    // Seventy minutes out — the front is twenty minutes wide and two hours away, so
    // this is the moment it comes inside the hour the outlook looks over. The wind has
    // not moved at all yet.
    for _ in 0..70 {
        wind.step(60.0);
    }
    let (building, turning) = wind.synoptic.expects();
    assert!(
        ms_to_kn(building) > 2.0,
        "it saw {:.1} knots of building coming",
        ms_to_kn(building)
    );
    assert!(
        deg(turning) > 10.0,
        "it saw {:.0} degrees of veer coming",
        deg(turning)
    );
}

/// **Most days have none**, and the ones that do are veering days. A model that backed
/// half the time would be teaching the wrong lesson to the one person this is for.
#[test]
fn about_a_third_of_days_bring_one_and_all_of_them_veer() {
    let mut days = 0;
    let mut fronts = 0;
    for seed in 0..600u64 {
        let mut wind = SyntheticWind::new(rad(140.0), kn_to_ms(16.0), seed);
        wind.synoptic.may_bring_a_front(seed);
        days += 1;
        if let Some(front) = wind.synoptic.front {
            fronts += 1;
            let veer = deg(wrap_pi(front.behind_direction - rad(140.0)));
            assert!(
                (15.0..=75.0).contains(&veer),
                "a front that shifted the wind {veer:.0} degrees"
            );
        }
    }
    let share = fronts as f64 / days as f64;
    assert!(
        (0.28..0.40).contains(&share),
        "{:.0}% of days brought a front",
        share * 100.0
    );
}

/// **And adding it must not have moved anybody's weather.** The same fault the fog was
/// caught by: a draw taken out of the shared stream shifts every gust in every world
/// that ever existed.
#[test]
fn a_front_draws_from_its_own_dice() {
    let gusts = |front: bool| {
        let mut wind = SyntheticWind::new(rad(90.0), kn_to_ms(12.0), 5);
        if front {
            wind.synoptic.may_bring_a_front(5);
            wind.synoptic.front = None;
        }
        for _ in 0..30 {
            wind.step(10.0);
        }
        wind.sample(glam::DVec2::new(40.0, 10.0), 300.0)
    };
    let before = gusts(false);
    let after = gusts(true);
    assert_eq!(
        before, after,
        "the front took a number out of the shared stream"
    );
}
