//! Engine-agnostic sailing yacht dynamics.
//!
//! The crate deliberately contains no rendering, no input handling and no dependency
//! on any game engine. That is what keeps the presentation layer — Bevy, Fyrox, Godot
//! via gdext — a reversible decision, and it is what makes the physics testable
//! without opening a window.
//!
//! The core feedback loop, and the whole reason the crate exists:
//!
//! ```text
//!   true wind  ->  trim  ->  drive + leeway  ->  apparent wind shifts  ->  trim
//! ```
//!
//! Everything else — anchoring, the auxiliary engine, winches, reefing — extends that
//! loop rather than standing beside it.
//!
//! ```
//! use sail_core::prelude::*;
//! use sail_core::wind::{SteadyWind, WindField};
//!
//! let cfg = BoatConfig::charter_sloop_34();
//! let make = || -> Box<dyn WindField> { Box::new(SteadyWind::new(rad(0.0), kn_to_ms(12.0))) };
//!
//! // Beat at 42 degrees to the true wind on starboard tack, trimmed and steered.
//! let s = steady_state(cfg, &make, rad(42.0), 420.0, 0.0);
//!
//! assert!(s.converged);
//! assert!(ms_to_kn(s.telemetry.stw) > 5.0);   // a touch under six knots
//! assert!(s.state.heel < 0.0);                // heeled to port, as starboard tack must be
//! assert!(s.controls.rudder < 0.0);           // carrying a few degrees of weather helm
//! ```
//!
//! Note what the example does *not* do: start from rest with the sheets hard in. That
//! puts the boat in irons, where it has no steerage and goes nowhere — correct physics,
//! and a trap worth knowing about before you wonder why your boat will not sail.

pub mod autopilot;
pub mod boat;
pub mod hull;
pub mod hydrostatics;
pub mod land;
pub mod rng;
pub mod sail;
pub mod sim;
pub mod state;
pub mod units;
pub mod water;
pub mod wind;

pub mod prelude {
    pub use crate::autopilot::{
        heading_rudder, optimal_sheets, reef_for_heel, steady_state, Autopilot, PilotMode, Steady,
        TillerPilot,
    };
    pub use crate::boat::{BoatConfig, RiggedSail};
    pub use crate::hull::Foil;
    pub use crate::hydrostatics::HullForm;
    pub use crate::sail::{camber_for_outhaul, outhaul_for_heel, SailForces, SailProfile};
    pub use crate::sim::{Simulation, SUBSTEP};
    pub use crate::state::{BoatState, Controls, Telemetry};
    pub use crate::units::{deg, kn_to_ms, ms_to_kn, rad, wrap_pi, wrap_tau};
    pub use crate::water::{FlatWater, WaterSurface, WindSea};
    pub use crate::wind::{SteadyWind, SyntheticWind, WindField};
}
