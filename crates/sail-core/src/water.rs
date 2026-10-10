//! The water surface.
//!
//! Gerstner waves rather than a sum of sines, because Gerstner gets the two things
//! that matter to a boat: crests are sharper than troughs, and surface particles move
//! in orbits rather than up and down. That orbital motion is not decoration — it is
//! the water the hull is actually moving through, and it is why a boat surges on a
//! wave instead of merely rising on one.
//!
//! Deep-water dispersion throughout: `omega^2 = g k`. Long waves therefore run fast
//! and short ones crawl, which is what makes a real sea look layered.
//!
//! `WaterSurface` is the seam. Flat water keeps every existing flat-water result
//! valid, and a measured spectrum or a swell forecast can replace the synthetic sea
//! without anything downstream noticing.

use crate::units::GRAVITY;
use glam::{DVec2, DVec3};

/// What the hull needs to know about the water underneath it.
pub trait WaterSurface: Send + Sync {
    /// Surface elevation above mean level, metres.
    fn height(&self, pos: DVec2, t: f64) -> f64;

    /// Velocity of the water itself at the surface, metres per second. The vertical
    /// component drives heave; the horizontal component is the orbital flow the hull
    /// and appendages move through.
    fn velocity(&self, pos: DVec2, t: f64) -> DVec3;

    /// Surface slope as (d/dx, d/dy). A boat rolls and pitches towards the local
    /// surface normal, so this is what tilts it in a seaway.
    fn slope(&self, pos: DVec2, t: f64) -> DVec2;

    /// Surface elevation with everything shorter than `min_wavelength` taken out.
    ///
    /// For *drawing*, never for physics. A surface mesh can only carry wavelengths
    /// longer than twice its vertex spacing; anything shorter does not disappear, it
    /// aliases, and the aliases crawl as the mesh slides along under the boat. On a
    /// grid that is deliberately coarse in the distance — which is the right way to
    /// spend vertices — that crawl is the whole far field shimmering, and no amount of
    /// smoothing downstream recovers it, because the information was destroyed at the
    /// sample. The fix has to be at the sample: do not ask for detail the mesh cannot
    /// hold.
    ///
    /// The default ignores the limit, which is correct for a flat sea and for any
    /// surface whose content is already long.
    fn height_resolved(&self, pos: DVec2, t: f64, min_wavelength: f64) -> f64 {
        let _ = min_wavelength;
        self.height(pos, t)
    }

    /// Slope with the same band limit, so the shading matches the geometry.
    fn slope_resolved(&self, pos: DVec2, t: f64, min_wavelength: f64) -> DVec2 {
        let _ = min_wavelength;
        self.slope(pos, t)
    }

    /// Water velocity at a depth below the surface, metres per second.
    ///
    /// Orbital motion decays as `exp(-k z)`, and steeply: each component dies off on
    /// the scale of its own wavelength, so short waves are a surface phenomenon and
    /// long ones reach well down. This matters more than it looks. A keel two metres
    /// down sees a quarter to a half less orbital flow than the hull at the surface,
    /// and feeding it the surface figure puts it at twenty-odd degrees of angle of
    /// attack — stalled, and braking the boat with kilonewtons that are not there.
    fn velocity_at_depth(&self, pos: DVec2, depth: f64, t: f64) -> DVec3 {
        let _ = depth;
        self.velocity(pos, t)
    }

    /// Typical surface slope of this sea, radians — the root-mean-square of its
    /// component slopes.
    ///
    /// What foam and whitecaps have to be judged against. A fixed threshold cannot
    /// work: a long swell of a given height is far gentler than a short chop of the
    /// same height, so a number tuned on one produces either no foam at all or a sea
    /// of milk on the other.
    fn characteristic_slope(&self) -> f64 {
        0.0
    }

    /// Horizontal displacement of a surface parcel from its mean position, metres.
    ///
    /// The integral of the orbital velocity, and therefore bounded: in deep water a
    /// surface particle travels a circle of radius equal to the wave amplitude. This
    /// is what the fine surface detail rides on — and the reason it matters is that
    /// multiplying the instantaneous orbital *velocity* by elapsed time instead gives
    /// an oscillating quantity scaled by an ever-growing one, so the detail slides
    /// about in every direction with amplitude increasing without bound. Which reads,
    /// from the deck, exactly like the boat periodically going backwards.
    fn horizontal_displacement(&self, _pos: DVec2, _t: f64) -> DVec2 {
        DVec2::ZERO
    }

    /// How much of the surface pressure gradient a hull of this draft actually feels,
    /// averaged over its immersed depth.
    ///
    /// The gradient decays as `exp(-k z)`, so a hull does not feel the surface value —
    /// it feels the average over its own draft, which is always less. One for a
    /// surface particle, and smaller the shorter the waves and the deeper the hull.
    fn depth_attenuation(&self, _draft: f64) -> f64 {
        1.0
    }

    /// Significant wave height, metres. Zero for flat water.
    fn significant_height(&self) -> f64 {
        0.0
    }
}

/// Dead flat. Every flat-water test and the whole measured polar rest on this.
#[derive(Clone, Copy, Debug, Default)]
pub struct FlatWater;

impl WaterSurface for FlatWater {
    fn height(&self, _pos: DVec2, _t: f64) -> f64 {
        0.0
    }
    fn velocity(&self, _pos: DVec2, _t: f64) -> DVec3 {
        DVec3::ZERO
    }
    fn slope(&self, _pos: DVec2, _t: f64) -> DVec2 {
        DVec2::ZERO
    }
    fn velocity_at_depth(&self, _pos: DVec2, _depth: f64, _t: f64) -> DVec3 {
        DVec3::ZERO
    }
    fn horizontal_displacement(&self, _pos: DVec2, _t: f64) -> DVec2 {
        DVec2::ZERO
    }
}

/// One Gerstner component.
#[derive(Clone, Copy, Debug)]
pub struct WaveComponent {
    /// Amplitude, metres. Half the crest-to-trough height.
    pub amplitude: f64,
    /// Wavenumber, radians per metre.
    pub k: f64,
    /// Angular frequency, radians per second. Tied to `k` by deep-water dispersion.
    pub omega: f64,
    /// Unit direction of travel.
    pub direction: DVec2,
    pub phase: f64,
}

impl WaveComponent {
    /// How much of this component a mesh of the given resolution can actually carry.
    ///
    /// One at twice the limit and above, rolled smoothly to nothing as the wavelength
    /// approaches it. Smooth rather than a hard cut because a component switching off
    /// between one vertex ring and the next would draw a visible seam in the water —
    /// trading a shimmer for a scar.
    fn resolved(&self, min_wavelength: f64) -> f64 {
        if min_wavelength <= 0.0 {
            return 1.0;
        }
        let wavelength = std::f64::consts::TAU / self.k.max(1e-9);
        let t = ((wavelength / min_wavelength) - 1.0).clamp(0.0, 1.0);
        // Smoothstep, so the roll-off has no corner in it.
        t * t * (3.0 - 2.0 * t)
    }

    /// Build from a period and a direction, letting dispersion fix the wavelength.
    pub fn from_period(amplitude: f64, period: f64, direction: f64, phase: f64) -> Self {
        let omega = std::f64::consts::TAU / period.max(0.3);
        // Deep water: omega^2 = g k.
        let k = omega * omega / GRAVITY;
        Self {
            amplitude,
            k,
            omega,
            direction: DVec2::new(direction.sin(), direction.cos()),
            phase,
        }
    }

    pub fn wavelength(&self) -> f64 {
        std::f64::consts::TAU / self.k
    }

    /// Phase speed, metres per second.
    pub fn celerity(&self) -> f64 {
        self.omega / self.k
    }

    #[inline]
    fn theta(&self, pos: DVec2, t: f64) -> f64 {
        self.k * self.direction.dot(pos) - self.omega * t + self.phase
    }
}

/// A short-crested wind sea: several components spread about the wind direction.
///
/// Heights and periods come from the usual fetch-limited relations rather than being
/// dialled in, so a given wind produces a plausible sea on its own. A long fetch in
/// twenty knots builds a very different sea from the same wind across a bay, and the
/// `fetch` term is what expresses that.
#[derive(Clone, Debug)]
pub struct WindSea {
    components: Vec<WaveComponent>,
    hs: f64,
}

impl WindSea {
    /// `tws` in m/s, `fetch` in metres, `direction` the wind's compass direction (the
    /// one it comes *from*; the sea runs the other way).
    pub fn new(tws: f64, fetch: f64, direction: f64, seed: u64) -> Self {
        let (hs, tp) = fetch_limited_sea(tws, fetch);
        // Waves travel downwind, which is the reciprocal of the meteorological
        // direction the wind is quoted in.
        let run = direction + std::f64::consts::PI;

        // Five components: the peak, two longer, two shorter, fanned either side of
        // the mean direction. Enough to stop the surface looking like corduroy,
        // cheap enough to sample a dozen times a frame.
        let mut rng = crate::rng::Pcg32::new(seed);
        let spread = [0.0_f64, -0.42, 0.42, -0.72, 0.72];
        let period_ratio = [1.0_f64, 1.26, 0.82, 1.52, 0.66];
        // Energy-weighted so the peak dominates and the sum lands on the right Hs.
        let weight = [1.0_f64, 0.62, 0.55, 0.34, 0.26];
        let norm: f64 = weight.iter().map(|w| w * w).sum::<f64>().sqrt();

        let components = (0..5)
            .map(|i| {
                // Hs is four times the root-mean-square elevation, and for a sum of
                // sinusoids the rms is the quadrature sum of the amplitudes over
                // root two — so scale the set to land on the requested Hs.
                let amplitude = (hs / 2.0) * weight[i] / norm;
                WaveComponent::from_period(
                    amplitude,
                    tp * period_ratio[i],
                    run + spread[i],
                    rng.next_f64() * std::f64::consts::TAU,
                )
            })
            .collect();

        Self { components, hs }
    }

    pub fn components(&self) -> &[WaveComponent] {
        &self.components
    }

    /// Period of the dominant component, seconds.
    pub fn peak_period(&self) -> f64 {
        std::f64::consts::TAU / self.components[0].omega
    }
}

impl WaterSurface for WindSea {
    fn height(&self, pos: DVec2, t: f64) -> f64 {
        self.components
            .iter()
            .map(|c| c.amplitude * c.theta(pos, t).cos())
            .sum()
    }

    fn height_resolved(&self, pos: DVec2, t: f64, min_wavelength: f64) -> f64 {
        self.components
            .iter()
            .map(|c| c.amplitude * c.resolved(min_wavelength) * c.theta(pos, t).cos())
            .sum()
    }

    fn slope_resolved(&self, pos: DVec2, t: f64, min_wavelength: f64) -> DVec2 {
        let mut s = DVec2::ZERO;
        for c in &self.components {
            let d = -c.amplitude * c.resolved(min_wavelength) * c.k * c.theta(pos, t).sin();
            s += c.direction * d;
        }
        s
    }

    fn velocity(&self, pos: DVec2, t: f64) -> DVec3 {
        let mut v = DVec3::ZERO;
        for c in &self.components {
            let theta = c.theta(pos, t);
            // Deep-water orbital motion at the surface: a circle of radius equal to
            // the amplitude, travelled at the wave's own angular rate.
            let orbital = c.amplitude * c.omega;
            v.x += orbital * theta.cos() * c.direction.x;
            v.y += orbital * theta.cos() * c.direction.y;
            v.z += orbital * theta.sin();
        }
        v
    }

    fn slope(&self, pos: DVec2, t: f64) -> DVec2 {
        let mut s = DVec2::ZERO;
        for c in &self.components {
            let d = -c.amplitude * c.k * c.theta(pos, t).sin();
            s += c.direction * d;
        }
        s
    }

    fn velocity_at_depth(&self, pos: DVec2, depth: f64, t: f64) -> DVec3 {
        let d = depth.max(0.0);
        let mut v = DVec3::ZERO;
        for c in &self.components {
            let theta = c.theta(pos, t);
            // Each component decays on the scale of its own wavenumber.
            let decay = (-c.k * d).exp();
            let orbital = c.amplitude * c.omega * decay;
            v.x += orbital * theta.cos() * c.direction.x;
            v.y += orbital * theta.cos() * c.direction.y;
            v.z += orbital * theta.sin();
        }
        v
    }

    fn characteristic_slope(&self) -> f64 {
        let sum: f64 = self
            .components
            .iter()
            .map(|c| {
                let slope = c.amplitude * c.k;
                slope * slope * 0.5
            })
            .sum();
        sum.sqrt()
    }

    fn horizontal_displacement(&self, pos: DVec2, t: f64) -> DVec2 {
        let mut d = DVec2::ZERO;
        for c in &self.components {
            // Velocity is A omega cos(theta) and dtheta/dt is -omega, so the integral
            // is *minus* A sin(theta). The plus sign was caught immediately by
            // differentiating this numerically and comparing against the velocity —
            // which is the only defence that works, since wave maths is thick with
            // sign conventions and this is the fourth such error this session.
            d -= c.direction * (c.amplitude * c.theta(pos, t).sin());
        }
        d
    }

    fn depth_attenuation(&self, draft: f64) -> f64 {
        let d = draft.max(0.01);
        // Amplitude-weighted mean of the exact per-component average,
        // (1 - exp(-k d)) / (k d). Weighted rather than exact per component, because
        // the force is assembled from one combined slope; short waves are attenuated
        // hardest, which is right — they are a surface phenomenon.
        let mut weight = 0.0;
        let mut total = 0.0;
        for c in &self.components {
            let kd = c.k * d;
            let mean = if kd < 1e-6 {
                1.0
            } else {
                (1.0 - (-kd).exp()) / kd
            };
            total += c.amplitude * mean;
            weight += c.amplitude;
        }
        if weight <= 0.0 {
            1.0
        } else {
            total / weight
        }
    }

    fn significant_height(&self) -> f64 {
        self.hs
    }
}

/// Significant height and peak period for a wind over a given fetch.
///
/// Fully developed deep water gives `Hs ~= 0.0248 U^2` and `Tp ~= 0.73 U`, the
/// Pierson-Moskowitz relations. Short fetch caps both, which is why the Baltic in
/// twenty knots is a steep, awkward one-metre sea and the Atlantic in the same wind is
/// a long three-metre one. The cap here is the usual JONSWAP-style fetch scaling,
/// simplified to one expression.
pub fn fetch_limited_sea(tws: f64, fetch: f64) -> (f64, f64) {
    let u = tws.max(0.0);
    if u < 0.5 {
        return (0.0, 3.0);
    }
    let developed_hs = 0.0248 * u * u;
    let developed_tp = 0.73 * u;

    // Dimensionless fetch. Both height and period saturate as it grows.
    let x = (GRAVITY * fetch.max(1.0) / (u * u)).min(2.0e4);
    let hs_factor = (x / 2.0e4).powf(0.5).min(1.0);
    let tp_factor = (x / 2.0e4).powf(0.33).min(1.0);

    (
        developed_hs * hs_factor,
        (developed_tp * tp_factor).max(1.6),
    )
}
