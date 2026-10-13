//! The small curves every other module was keeping its own copy of.
//!
//! Found by reviewing a day's work: five implementations of one Hermite step in
//! `sail-app`, under three different names — `step_between` in the clouds, `ease` on the
//! deck, a closure in the deck texture, and two called `smoothstep` that I had written
//! that same afternoon in the sea and in its texture. Plus a sixth inlined in the wave
//! spectrum next door, beside the `units::smoothstep` it duplicates.
//!
//! None of them disagreed, which is the only reason it had cost nothing yet. That is
//! how this particular fault always starts — the masthead, the sun, the shroud rule, the
//! sea colour, the drift rate, the key list — and what it costs is paid later, by
//! somebody changing one of the six.
//!
//! `sail-core` has these in `units`, in `f64`, because that is the arithmetic the
//! physics is in. These are the `f32` ones the renderer wants, which is a real
//! distinction and not a second copy: converting at every call site would be worse than
//! either.

/// Hermite between two edges: flat outside them, smooth across.
#[inline]
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    ease(((x - edge0) / (edge1 - edge0).max(1e-6)).clamp(0.0, 1.0))
}

/// The same curve on a value already in 0 to 1, which is how half the callers want it.
#[inline]
pub fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
