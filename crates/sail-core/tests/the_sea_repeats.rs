//! Does the sea come back to itself?
//!
//! The fault this file exists for was visible from the deck and invisible to every other
//! test: the water was laid out in a regular pattern, big enough to see across the whole
//! field of view and perfectly obvious once noticed. Nothing was wrong with the height,
//! the period or the slope — the sea was the right sea, five times over.
//!
//! Five sinusoids was the cause. A sum of a few components is not a sea, it is an
//! interference pattern, and an interference pattern repeats. So the measurement is
//! correlation against itself at a distance: take a long line of water, slide a copy of
//! it along, and see how strongly the two agree. A real wind sea agrees with itself over
//! a wave or two and then forgets; a lattice agrees again every time the pattern comes
//! round.
//!
//! ```text
//! cargo test -p sail-core --test the_sea_repeats -- --nocapture
//! ```

use glam::DVec2;
use sail_core::water::{WaterSurface, WindSea};

/// How much of the sea to sample, and how finely.
const LINE: f64 = 3_000.0;
const STEP: f64 = 2.0;

/// Correlation of the surface with itself, offset by `lag` metres along `dir`.
fn echoes(sea: &WindSea, dir: DVec2) -> Vec<(f64, f64)> {
    let n = (LINE / STEP) as usize;
    let h: Vec<f64> = (0..n)
        .map(|i| sea.height(dir * (i as f64 * STEP), 0.0))
        .collect();
    let mean = h.iter().sum::<f64>() / n as f64;
    let var = h.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / n as f64;
    (1..n / 2)
        .map(|lag| {
            let m = n - lag;
            let c = (0..m)
                .map(|i| (h[i] - mean) * (h[i + lag] - mean))
                .sum::<f64>()
                / m as f64;
            (lag as f64 * STEP, c / var)
        })
        .collect()
}

/// **The sea must not come back.**
///
/// Beyond a few wavelengths there is no physical reason for the water a quarter of a mile
/// away to resemble the water here, and a correlation that says otherwise is a pattern
/// the eye will find. The old sea scored 0.92 at 1164 m; this asks for less than two
/// thirds of that, measured well outside the range where waves may legitimately look
/// like themselves.
#[test]
fn the_water_a_quarter_of_a_mile_off_is_different_water() {
    for seed in [3, 17, 61] {
        let sea = WindSea::new(10.0, 40_000.0, 0.0, seed);
        for (name, dir) in [
            ("downwind", DVec2::new(0.0, -1.0)),
            ("across", DVec2::new(1.0, 0.0)),
        ] {
            let far = echoes(&sea, dir)
                .into_iter()
                .filter(|(d, _)| *d > 250.0)
                .fold(
                    (0.0, 0.0),
                    |worst, (d, c)| if c > worst.1 { (d, c) } else { worst },
                );
            println!(
                "seed {seed} {name}: strongest echo beyond 250 m is {:.2} at {:.0} m",
                far.1, far.0
            );
            assert!(
                far.1 < 0.62,
                "the sea correlates {:.2} with itself {:.0} m {name}, which is a pattern",
                far.1,
                far.0
            );
        }
    }
}

/// The sea is still the sea it says it is: height, period and the slope a sailor would
/// recognise. A spectrum broad enough not to repeat is also a spectrum that can quietly
/// stop being the requested sea, and these are the numbers the rest of the boat is
/// written against.
#[test]
fn and_it_is_still_the_sea_it_reports() {
    let sea = WindSea::new(10.0, 40_000.0, 0.0, 7);
    let hs = sea.significant_height();
    let tp = sea.peak_period_or(0.0);
    // Four times the root-mean-square elevation is the definition of Hs, so measuring it
    // back off the surface is the one check that the amplitudes were normalised right.
    let n = 4_000;
    let mut sum = 0.0;
    for i in 0..n {
        let at = DVec2::new(i as f64 * 1.7, i as f64 * 0.9);
        let eta = sea.height(at, 0.0);
        sum += eta * eta;
    }
    let measured = 4.0 * (sum / n as f64).sqrt();
    println!(
        "reported hs {hs:.2} m, measured {measured:.2} m, tp {tp:.2} s, rms slope {:.3}",
        sea.characteristic_slope()
    );
    assert!(
        (measured - hs).abs() < 0.12 * hs,
        "a sea that reports {hs:.2} m measures {measured:.2} m"
    );
    // Ten knots over a long fetch: a metre of sea at four seconds, and an rms slope of
    // 0.098 — six degrees. The five-component sea this replaced measured 0.084 on the
    // same wind, so broadening the spectrum and taking the root two out of the heights
    // very nearly cancelled: the water is the same steepness and no longer a lattice.
    // The band is wide enough to allow a different spectrum and narrow enough that a
    // sea of milk or a millpond fails it.
    assert!((3.5..5.0).contains(&tp), "tp {tp:.2}");
    assert!(
        (0.07..0.14).contains(&sea.characteristic_slope()),
        "rms slope {:.3}",
        sea.characteristic_slope()
    );
}
