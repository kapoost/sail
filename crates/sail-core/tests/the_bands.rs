//! The planetary wind belts, laid across a flat sea.
//!
//! The question this file answers is the one the idea stands on: **does sailing from one
//! belt into the next behave the way a planet does** — the trades steady from the
//! north-east, the wind going light and hanging about, and then filling in from the
//! opposite quarter — or is it a sine wave with a sailing boat on it.
//!
//! Printed as well as asserted, because the thing that decides whether this is worth
//! wrapping the world for is a passage across the belts and you cannot assert a passage.
//!
//! ```text
//! cargo test -p sail-core --test the_bands -- --nocapture
//! ```

use glam::DVec2;
use sail_core::units::{deg, kn_to_ms, ms_to_kn, rad, wrap_pi};
use sail_core::wind::{Bands, SyntheticWind, WindField};

/// A cycle twenty kilometres across: half an hour of sailing from the middle of the
/// trades to the middle of the westerlies at six knots, which is the scale this is for.
const SPACING: f64 = 20_000.0;

fn planet() -> SyntheticWind {
    // Wind from the north-west as the day's own, so that nothing in the belt pattern can
    // be mistaken for the day underneath it.
    let mut wind = SyntheticWind::new(rad(315.0), kn_to_ms(14.0), 5);
    wind.bands = Some(Bands {
        spacing: SPACING,
        along: 0.0,
    });
    wind
}

/// A section across the belts, which is the picture the whole idea lives or dies by.
#[test]
fn a_section_across_the_belts() {
    let wind = planet();
    println!(
        "\n{:>8} {:>10} {:>8}   what it is",
        "north", "from", "knots"
    );
    let mut north = -SPACING * 0.5;
    while north <= SPACING * 0.5 + 1.0 {
        let at = DVec2::new(0.0, north);
        let flow = wind.sample(at, 0.0);
        println!(
            "{:>7.1}k {:>9.0}° {:>8.1}   {}",
            north / 1000.0,
            deg(sail_core::wind::direction_of(flow)),
            ms_to_kn(flow.length()),
            wind.band_at(at).unwrap_or("—")
        );
        north += SPACING / 16.0;
    }
}

/// **The trades blow from the north-east and the westerlies from the south-west**, which
/// is the whole claim. Taken in the middle of each belt, where a belt is what it is.
#[test]
fn the_two_belts_blow_from_opposite_quarters() {
    let wind = planet();
    // Deep in the trades is where the cosine is at its most negative, which is the
    // middle of the cycle as this lays it out; the westerlies are half a cycle away.
    let trades = DVec2::new(0.0, 0.0);
    let westerlies = DVec2::new(0.0, SPACING * 0.5);

    let from_trades = sail_core::wind::direction_of(wind.sample(trades, 0.0));
    let from_westerlies = sail_core::wind::direction_of(wind.sample(westerlies, 0.0));

    assert!(
        deg(wrap_pi(from_trades - rad(60.0))).abs() < 30.0,
        "the trades are blowing from {:.0} rather than from the north-east",
        deg(from_trades)
    );
    assert!(
        deg(wrap_pi(from_westerlies - rad(240.0))).abs() < 30.0,
        "the westerlies are blowing from {:.0} rather than from the south-west",
        deg(from_westerlies)
    );
    // And they are opposite, which is the part that makes a circumnavigation possible:
    // one belt takes you out and the other brings you back.
    assert!(
        deg(wrap_pi(from_trades - from_westerlies)).abs() > 120.0,
        "the two belts are only {:.0} degrees apart",
        deg(wrap_pi(from_trades - from_westerlies)).abs()
    );
}

/// And between them it goes light, which is what makes crossing one a decision.
///
/// Not dead: the horse latitudes are light and *variable*, not empty, and a belt of flat
/// calm across the world would be a wall rather than a passage — as well as putting the
/// boat in the one regime this model is known to handle badly.
#[test]
fn the_horse_latitudes_are_light_but_not_empty() {
    let wind = planet();
    let belt = ms_to_kn(wind.sample(DVec2::new(0.0, 0.0), 0.0).length());
    let gap = ms_to_kn(wind.sample(DVec2::new(0.0, SPACING * 0.25), 0.0).length());
    assert!(
        gap < belt * 0.6,
        "the gap between the belts is {gap:.1} knots against {belt:.1} in the trades — \
         it is not a gap at all"
    );
    assert!(
        gap > 1.5,
        "the gap is {gap:.1} knots, which is a wall rather than the horse latitudes"
    );
}

/// A world with no belts is the world every validated number was measured in.
///
/// The belts are an option and this is the test that they stay one: with `bands` unset
/// the field must be exactly what it was, or every polar, every VMG and every reef
/// recommendation in the validation suite is quietly measuring something else.
#[test]
fn without_them_nothing_moves() {
    let plain = SyntheticWind::new(rad(315.0), kn_to_ms(14.0), 5);
    assert!(plain.band_at(DVec2::ZERO).is_none());
    // Averaged along each half of a section, because a single pair of points differs by
    // the gusts and the gusts are the loudest thing in the field at this scale. What a
    // belt does and a gust does not is move the *mean* a mile at a time.
    let mean_over = |from: f64, to: f64| {
        let n = 64;
        (0..n)
            .map(|i| {
                let north = from + (to - from) * i as f64 / n as f64;
                ms_to_kn(plain.sample(DVec2::new(0.0, north), 0.0).length())
            })
            .sum::<f64>()
            / n as f64
    };
    let south = mean_over(-SPACING * 0.5, 0.0);
    let north = mean_over(0.0, SPACING * 0.5);
    assert!(
        (south - north).abs() < 1.5,
        "the plain world has a pattern in it: {south:.1} knots over one half and \
         {north:.1} over the other"
    );
}
