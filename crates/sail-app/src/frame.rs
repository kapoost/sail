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

/// Sim world position to Bevy translation. Height comes from the caller.
///
/// Note the cast to f32. For a proof-of-concept area of a few kilometres this is
/// harmless — f32 holds millimetre precision out to 10 km. A full-size coastline will
/// need a floating origin; this function is where that change lands.
#[inline]
pub fn to_bevy(pos: glam::DVec2, height: f32) -> Vec3 {
    Vec3::new(pos.x as f32, height, -pos.y as f32)
}

/// Where the hull actually sits, including how far the sea has lifted it.
#[inline]
pub fn hull_translation(st: &BoatState) -> Vec3 {
    to_bevy(st.pos, st.heave as f32)
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
