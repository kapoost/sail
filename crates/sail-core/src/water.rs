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

    /// Tell the surface how much of the open sea reaches here, 0 to 1.
    ///
    /// Waves need a fetch to build, so the water under the lee of an island is flatter
    /// than the water a mile outside it. Set once a substep from the boat's own
    /// position rather than evaluated per query: the shelter varies over a kilometre
    /// and the boat is eleven metres long, so one figure describes the whole of the
    /// local sea and the alternative is tracing rays a hundred times a substep.
    ///
    /// The default ignores it, which is right for flat water and for any sea that has
    /// no land to be sheltered by.
    fn set_shelter(&mut self, _shelter: f64) {}

    /// Peak period if the surface has one, else the fallback. For reporting only.
    fn peak_period_or(&self, fallback: f64) -> f64 {
        fallback
    }

    /// Let the sea follow the wind, which it does slowly and asymmetrically.
    ///
    /// A sea is not a function of the wind now; it is a function of the wind for the
    /// last few hours. That lag is most of what a seaway feels like. A new breeze over
    /// flat water is fast and easy for an hour before it gets lumpy, and a wind that
    /// drops leaves its sea behind — the nastiest motion a small boat sees is a dying
    /// gale, when there is no wind left to steady her and the whole sea is still
    /// running. Build and decay therefore get different time constants, and the slower
    /// one is the decay.
    ///
    /// Default does nothing, which is right for flat water and for any sea handed over
    /// as a fixed spectrum.
    fn follow_wind(&mut self, _tws: f64, _direction: f64, _t: f64, _dt: f64) {}

    /// Longest wavelength present, metres.
    ///
    /// A presentation layer that band-limits by vertex spacing can ask this and skip
    /// the water entirely wherever the mesh is too coarse to carry anything — which on
    /// a grid that reaches the horizon is most of it.
    fn longest_wavelength(&self) -> f64 {
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
    /// Significant height, metres: the sea as it is *now*, which lags the wind.
    hs: f64,
    /// Peak period, seconds, likewise.
    tp: f64,
    /// Direction the waves run towards, radians. Follows the wind, slowly.
    run: f64,
    /// How much open water is upwind of here, metres. A property of the place, so it
    /// does not change as the wind does.
    fetch: f64,
    /// Fixed per component, so a relaxation reshapes the sea without reshuffling it.
    phases: [f64; BANDS],
    /// Seconds owed to the relaxation, so it can be driven from a 200 Hz substep
    /// without evaluating the fetch relations two hundred times a second for a process
    /// whose time constant is hours.
    owed: f64,
    /// How much of the open sea reaches here; 1 in clear water, less in a lee.
    shelter: f64,
}

/// Components in the sea: the peak, a long pair and a short pair.
const BANDS: usize = 5;

/// Direction of each component relative to the mean, radians.
///
/// Both members of a pair sit on the same side of the peak in period and on opposite
/// sides of it in direction, so there is nothing left to correlate. The widths follow
/// the physics they were always meant to: spreading is narrowest at the spectral peak
/// and opens up away from it, more on the short side than the long, which is the
/// Mitsuyasu result.
const SPREAD: [f64; BANDS] = [0.0, -0.38, 0.38, -0.66, 0.66];
/// Period of each component as a multiple of the peak. The two members of a pair are
/// given slightly different periods so they cannot lock into a regular diamond.
const PERIOD_RATIO: [f64; BANDS] = [1.0, 1.26, 1.21, 0.82, 0.78];
/// Energy weighting, so the peak dominates and the sum lands on the right Hs.
const WEIGHT: [f64; BANDS] = [1.0, 0.60, 0.58, 0.32, 0.30];

/// How long a sea takes to build towards the one the wind would make, seconds.
///
/// Two and a half hours. Duration-limited growth is a real relation and this is the
/// right order for a coastal fetch: a fresh breeze over flat water gives you an hour
/// of easy sailing before it gets lumpy, which is a thing every sailor has used.
const SEA_BUILD: f64 = 9_000.0;
/// And how long it takes to lie down, seconds. Twice as long, and the asymmetry is the
/// point: a dying gale is the worst motion a small boat sees, because the wind has gone
/// and the sea has not.
const SEA_DECAY: f64 = 18_000.0;

impl WindSea {
    /// `tws` in m/s, `fetch` in metres, `direction` the wind's compass direction (the
    /// one it comes *from*; the sea runs the other way).
    pub fn new(tws: f64, fetch: f64, direction: f64, seed: u64) -> Self {
        let (hs, tp) = fetch_limited_sea(tws, fetch);
        let mut rng = crate::rng::Pcg32::new(seed);
        let mut phases = [0.0; BANDS];
        for p in &mut phases {
            *p = rng.next_f64() * std::f64::consts::TAU;
        }
        let mut sea = Self {
            components: Vec::new(),
            hs,
            tp,
            // Waves travel downwind, which is the reciprocal of the meteorological
            // direction the wind is quoted in.
            run: direction + std::f64::consts::PI,
            fetch,
            phases,
            owed: 0.0,
            shelter: 1.0,
        };
        sea.reshape(0.0, &[0.0; BANDS]);
        sea
    }

    /// Build the components from the sea state. The single definition of the spectrum's
    /// shape, called by the constructor and by every relaxation — which is the only
    /// reason a sea that changes is not a second sea model.
    ///
    /// `was` is each component's angular frequency before the reshape. A Gerstner phase
    /// is `k·x − ωt + φ`, so changing ω would step the surface by `Δω·t` — at an hour
    /// in, a jump of several wavelengths. Folding that into the stored phase keeps the
    /// water continuous where the boat is, which is the only place anyone can tell.
    fn reshape(&mut self, t: f64, was: &[f64; BANDS]) {
        let norm: f64 = WEIGHT.iter().map(|w| w * w).sum::<f64>().sqrt();
        self.components = (0..BANDS)
            .map(|i| {
                // Hs is four times the root-mean-square elevation, and for a sum of
                // sinusoids the rms is the quadrature sum of the amplitudes over
                // root two — so scale the set to land on the requested Hs.
                let amplitude = (self.hs / 2.0) * WEIGHT[i] / norm;
                let mut c = WaveComponent::from_period(
                    amplitude,
                    self.tp * PERIOD_RATIO[i],
                    self.run + SPREAD[i],
                    self.phases[i],
                );
                c.phase = crate::units::wrap_tau(self.phases[i] + (c.omega - was[i]) * t);
                self.phases[i] = c.phase;
                c
            })
            .collect();
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
        self.shelter
            * self
                .components
                .iter()
                .map(|c| c.amplitude * c.theta(pos, t).cos())
                .sum::<f64>()
    }

    fn height_resolved(&self, pos: DVec2, t: f64, min_wavelength: f64) -> f64 {
        self.shelter
            * self
                .components
                .iter()
                .map(|c| c.amplitude * c.resolved(min_wavelength) * c.theta(pos, t).cos())
                .sum::<f64>()
    }

    fn slope_resolved(&self, pos: DVec2, t: f64, min_wavelength: f64) -> DVec2 {
        let mut s = DVec2::ZERO;
        for c in &self.components {
            let d = -c.amplitude * c.resolved(min_wavelength) * c.k * c.theta(pos, t).sin();
            s += c.direction * d;
        }
        // Scaled whole, which drops the term in the *gradient* of the shelter. That
        // term is bounded by the wave height over the shelter's own length scale —
        // a metre over a kilometre, a thousandth of a radian — against slopes that
        // run to a tenth. Two orders down, and named rather than ignored.
        s * self.shelter
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
        v * self.shelter
    }

    fn slope(&self, pos: DVec2, t: f64) -> DVec2 {
        let mut s = DVec2::ZERO;
        for c in &self.components {
            let d = -c.amplitude * c.k * c.theta(pos, t).sin();
            s += c.direction * d;
        }
        s * self.shelter
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
        v * self.shelter
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

    fn peak_period_or(&self, _fallback: f64) -> f64 {
        self.peak_period()
    }

    fn follow_wind(&mut self, tws: f64, direction: f64, t: f64, dt: f64) {
        // Driven from the substep, acted on once a second. The time constants are
        // hours; evaluating the fetch relations two hundred times a second to move a
        // sea by a ten-thousandth of a millimetre is not a calculation, it is a bill.
        self.owed += dt;
        if self.owed < 1.0 {
            return;
        }
        let step = self.owed;
        self.owed = 0.0;

        let (hs_target, tp_target) = fetch_limited_sea(tws, self.fetch);
        let tau = if hs_target > self.hs {
            SEA_BUILD
        } else {
            SEA_DECAY
        };
        let blend = 1.0 - (-step / tau).exp();
        let was: [f64; BANDS] = std::array::from_fn(|i| self.components[i].omega);
        self.hs += (hs_target - self.hs) * blend;
        self.tp += (tp_target - self.tp) * blend;
        // The sea swings round to a shift on the same clock, and that lag is a thing
        // you sail in: an hour after a front goes through, the wind is in the new
        // quarter and the old sea is still running across it. Shortest way round, so a
        // veer through north does not take the sea the long way.
        let want = direction + std::f64::consts::PI;
        self.run =
            crate::units::wrap_tau(self.run + crate::units::wrap_pi(want - self.run) * blend);
        self.reshape(t, &was);
    }

    fn set_shelter(&mut self, shelter: f64) {
        self.shelter = shelter.clamp(0.0, 1.0);
    }

    fn longest_wavelength(&self) -> f64 {
        self.components
            .iter()
            .map(|c| std::f64::consts::TAU / c.k.max(1e-9))
            .fold(0.0, f64::max)
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
