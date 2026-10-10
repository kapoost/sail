//! The one place that knows both coordinate conventions.
//!
//! `sail-core` works in metres with x east, y north, z up, and headings measured from
//! north clockwise. Bevy works in y-up with -Z as forward. Every conversion goes
//! through here; nothing else in the app is allowed to mix the two.
//!
//! Mapping: `bevy.x = east`, `bevy.y = up`, `bevy.z = -north`. North is therefore -Z,
//! which lines up with Bevy's own idea of forward.

use bevy::prelude::*;
use sail_core::state::BoatState;

/// Where the rendered world is pinned, in sim coordinates.
///
/// The simulation sails away from its origin without limit; a rendered frame must not.
/// Single-precision holds about a millimetre at ten kilometres and degrades linearly
/// from there, and the first thing to break is not the geometry but the **texture
/// coordinates**: the sea's detail is addressed in world metres, so after twenty
/// minutes of sailing its UVs are in the hundreds, the GPU interpolates them in f32,
/// and the quantisation shows up as the water's surface detail stuttering and racing.
/// That is what a long session actually looks like, and it arrives long before
/// anything moves visibly in the wrong place.
///
/// So the world is drawn relative to a mark that follows the boat in whole
/// kilometre-and-a-bit steps. Everything rendered subtracts the same offset in the
/// same frame, so a step moves the entire scene together and is invisible; between
/// steps nothing moves at all, and the boat is never more than half a step from the
/// floating-point sweet spot.
#[derive(Resource, Default, Clone, Copy)]
pub struct RenderOrigin {
    pub offset: glam::DVec2,
}

/// How far the boat may stray before the mark is moved, metres.
///
/// A power of two so the step lands exactly on a float, and a whole number of sea
/// texture tiles so the water's UVs can be rebased by the same arithmetic without
/// shifting the pattern by a fraction of a repeat.
pub const ORIGIN_STEP: f64 = 1024.0;

impl RenderOrigin {
    /// Sim world position to Bevy translation. Height comes from the caller.
    #[inline]
    pub fn to_bevy(self, pos: glam::DVec2, height: f32) -> Vec3 {
        let p = pos - self.offset;
        Vec3::new(p.x as f32, height, -p.y as f32)
    }

    /// Where the hull actually sits, including how far the sea has lifted it.
    #[inline]
    pub fn hull_translation(self, st: &BoatState) -> Vec3 {
        self.to_bevy(st.pos, st.heave as f32)
    }

    /// Move the mark if the boat has wandered, returning the shift that was applied.
    ///
    /// Anything holding a position in *rendered* space — the chase camera is the only
    /// one — has to be shifted by the same amount, or it is left a kilometre behind
    /// and the view snaps. Everything else stores sim coordinates and converts each
    /// frame, so it follows for nothing.
    pub fn rebase(&mut self, boat: glam::DVec2) -> Vec3 {
        let wanted = glam::DVec2::new(
            (boat.x / ORIGIN_STEP).round() * ORIGIN_STEP,
            (boat.y / ORIGIN_STEP).round() * ORIGIN_STEP,
        );
        if wanted == self.offset {
            return Vec3::ZERO;
        }
        let shift = self.offset - wanted;
        self.offset = wanted;
        Vec3::new(shift.x as f32, 0.0, -shift.y as f32)
    }
}

/// Hull attitude: heading about the vertical, then heel about the hull axis.
///
/// Derivation, written down because it is pure sign-error territory. Heading 0 must
/// point at -Z, and a rotation about +Y by `t` sends (0,0,-1) to (-sin t, 0, -cos t);
/// matching that against (sin psi, 0, -cos psi) gives `t = -psi`. Heel positive to
/// starboard must tip the mast, which is local +Y, towards local +X; a rotation about
/// +Z by `a` sends (0,1,0) to (-sin a, cos a, 0), so `a = -heel`.
/// Pitch is positive bow-up. The bow is local -Z, so lifting it is a rotation about
/// local +X by the pitch angle: that sends -Z towards +Y.
#[inline]
pub fn attitude(st: &BoatState) -> Quat {
    Quat::from_rotation_y(-st.heading as f32)
        * Quat::from_rotation_z(-st.heel as f32)
        * Quat::from_rotation_x(st.pitch as f32)
}

/// Heading only — for things that stay upright, like the masthead wind indicator.
#[inline]
pub fn heading_only(st: &BoatState) -> Quat {
    Quat::from_rotation_y(-st.heading as f32)
}

/// Move the render origin when the boat has wandered far enough from it.
///
/// Runs before anything that places an object, so the whole frame is drawn against one
/// mark. The chase camera is shifted by hand because it is the only thing that keeps a
/// position in rendered space rather than converting from sim coordinates each frame.
pub fn rebase_origin(
    sim: Res<crate::sim::Sim>,
    mut origin: ResMut<RenderOrigin>,
    mut chase: ResMut<crate::camera::ChaseState>,
) {
    let shift = origin.rebase(sim.sim.state.pos);
    if shift != Vec3::ZERO {
        chase.shift(shift);
    }
}
