//! Smooth value noise, in one place.
//!
//! Two things in this crate want a continuous pseudo-random field over the plane — the
//! gusts, which are patches of wind carried over the water, and the land, which is
//! ridges and valleys laid over an island's mean shape. They wanted it for different
//! reasons and would have got two implementations, which in this codebase is the fault
//! that keeps coming back: one thing described in two places, drifting apart until
//! something is quietly wrong in one of them.
//!
//! Value noise rather than gradient noise, and the choice is not arbitrary. Gradient
//! (Perlin) noise is zero at every lattice point, which is right for a texture and
//! wrong for a field you are going to *measure*: it has a visible lattice of zeroes in
//! it, and a wind field with a regular grid of average patches reads as a grid. Value
//! noise's lattice points carry the extreme values instead, so what shows is the blobs
//! rather than the grid.
//!
//! Smoothstep interpolation, not linear, because the derivative matters in both uses: a
//! kink in the wind field is a step in the force on the rig and the boat rings like a
//! bell when it crosses one, and a kink in the land is a crease in the hillside.

use glam::DVec2;

/// Hash of a lattice point to [−1, 1]. Deterministic and stateless, which is what lets
/// a field be sampled anywhere at any time without storing it.
fn lattice(ix: i64, iy: i64, seed: u64) -> f64 {
    let mut h = seed
        ^ (ix as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (iy as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 33;
    h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h ^= h >> 33;
    h = h.wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    h ^= h >> 33;
    (h >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
}

/// One octave, on a unit lattice. Range is [−1, 1]; the standard deviation is about
/// 0.32, which matters to anything scaling it to a stated spread.
pub fn value(p: DVec2, seed: u64) -> f64 {
    let i = p.floor();
    let f = p - i;
    let w = f * f * (DVec2::splat(3.0) - 2.0 * f);
    let (ix, iy) = (i.x as i64, i.y as i64);
    let a = lattice(ix, iy, seed);
    let b = lattice(ix + 1, iy, seed);
    let c = lattice(ix, iy + 1, seed);
    let d = lattice(ix + 1, iy + 1, seed);
    let top = a + (b - a) * w.x;
    let bottom = c + (d - c) * w.x;
    top + (bottom - top) * w.y
}

/// Two octaves, the second small and turned off-lattice so the two never line up and
/// make a grid of it.
pub fn two_octaves(p: DVec2, seed: u64) -> f64 {
    0.74 * value(p, seed) + 0.26 * value(p * 2.7 + DVec2::new(11.3, -7.1), seed ^ 0x9E37)
}

/// Standard deviation of [`two_octaves`], measured rather than derived.
///
/// Pinned by a test, because the whole point of the number is that a caller scaling the
/// field to a stated spread gets that spread. If it drifts, every wind speed in the
/// simulator is quietly wrong by a few percent and nothing else says so.
pub const TWO_OCTAVE_SIGMA: f64 = 0.330_6;

/// Ridged noise: creases rather than blobs, which is what erosion leaves behind.
///
/// The absolute value of noise has a sharp valley wherever the noise crosses zero, and
/// one minus that is a sharp *ridge*. Those lines are what a hillside is made of — a
/// smooth hill is a pudding, and this is the difference between a drawn island and a
/// dish-cover. Range [0, 1].
pub fn ridged(p: DVec2, seed: u64) -> f64 {
    // Three octaves, and the high one is kept small on purpose. At 0.12 the field moved
    // by 0.19 across a fiftieth of a lattice cell, which over a two hundred metre
    // island is a cliff every few metres — spikes, not ridges. The crest is rounded off
    // with a smoothstep for the same reason: the absolute value puts a derivative
    // discontinuity exactly along every ridge line, which is what makes a ridge sharp
    // and also what makes a mesh built on it look chipped.
    let n = 0.70 * value(p, seed)
        + 0.23 * value(p * 2.3 + DVec2::new(5.1, 9.7), seed ^ 0x51ED)
        + 0.07 * value(p * 4.3 - DVec2::new(3.3, 1.9), seed ^ 0xB1A5);
    let crest = (1.0 - (n / 0.40).abs()).clamp(0.0, 1.0);
    crest * crest * (3.0 - 2.0 * crest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_octaves_has_the_spread_it_claims() {
        let mut sum = 0.0;
        let mut sum_sq = 0.0;
        let mut n = 0.0;
        for i in 0..400 {
            for j in 0..400 {
                // An irrational stride, so the samples never fall into step with the
                // lattice and measure only its corners.
                let x = two_octaves(DVec2::new(i as f64 * 0.1737, j as f64 * 0.1311), 0xA11CE);
                sum += x;
                sum_sq += x * x;
                n += 1.0;
            }
        }
        let mean = sum / n;
        let sigma = (sum_sq / n - mean * mean).sqrt();
        assert!(
            mean.abs() < 0.01,
            "a field with a mean in it biases its user: {mean:.4}"
        );
        assert!(
            (sigma - TWO_OCTAVE_SIGMA).abs() < 0.004,
            "TWO_OCTAVE_SIGMA wants to be {sigma:.4}"
        );
    }

    #[test]
    fn ridged_noise_is_in_range_and_continuous() {
        // Two properties, and they are the two that matter for ground. In range,
        // because it is multiplied into a height and a negative one digs a hole in the
        // island. And continuous — not *smooth*: the absolute value puts a derivative
        // discontinuity along every ridge line, which is exactly what makes a ridge a
        // ridge, and a cliff is a legitimate thing for an island to have. What must not
        // happen is a *jump*.
        //
        // Tested by halving the sampling step: a continuous field's largest jump falls
        // with the step and a discontinuous one's does not. Bounding the slope in
        // lattice units instead was the first attempt and it was meaningless, because
        // how steep that is in metres depends on the cell size the caller picks — which
        // is land's business, not noise's.
        let sweep = |step: f64| {
            let mut most = 0.0_f64;
            let mut spread = (1.0_f64, 0.0_f64);
            for i in 0..200 {
                let mut last: Option<f64> = None;
                for j in 0..600 {
                    let v = ridged(DVec2::new(i as f64 * 0.137, j as f64 * step), 7);
                    assert!((0.0..=1.0).contains(&v), "out of range: {v}");
                    spread = (spread.0.min(v), spread.1.max(v));
                    if let Some(was) = last {
                        most = most.max((v - was).abs());
                    }
                    last = Some(v);
                }
            }
            (most, spread)
        };
        let (coarse, spread) = sweep(0.02);
        let (fine, _) = sweep(0.005);
        assert!(
            fine < coarse * 0.45,
            "quartering the step should quarter the jump: {coarse:.3} then {fine:.3}"
        );
        assert!(
            spread.0 < 0.1 && spread.1 > 0.9,
            "the field should use its whole range, got {:.2} to {:.2}",
            spread.0,
            spread.1
        );
    }
}
