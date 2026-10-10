//! Minimal deterministic PRNG (PCG32). Hand-rolled rather than pulling `rand`, because
//! the validation suite must reproduce bit-for-bit across machines and `rand`'s default
//! generator is explicitly not a stability guarantee.

#[derive(Clone, Debug)]
pub struct Pcg32 {
    state: u64,
    inc: u64,
}

impl Pcg32 {
    pub fn new(seed: u64) -> Self {
        let mut r = Self {
            state: 0,
            inc: (seed << 1) | 1,
        };
        r.next_u32();
        r.state = r.state.wrapping_add(seed);
        r.next_u32();
        r
    }

    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Uniform in [0, 1).
    pub fn next_f64(&mut self) -> f64 {
        // 53 significant bits, the full mantissa.
        let hi = self.next_u32() as u64;
        let lo = self.next_u32() as u64;
        (((hi << 21) ^ lo) & ((1u64 << 53) - 1)) as f64 / (1u64 << 53) as f64
    }

    /// Standard normal via Box-Muller. Returns one sample; the paired value is discarded,
    /// which costs a little entropy and buys statelessness.
    pub fn next_normal(&mut self) -> f64 {
        let u1 = self.next_f64().max(1e-12);
        let u2 = self.next_f64();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
}
