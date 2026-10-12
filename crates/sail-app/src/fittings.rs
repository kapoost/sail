//! What a cockpit has in it.
//!
//! The deck had a surface and no furniture: no toerail along the sheer, no coaming
//! round the cockpit, no way into the cabin, no winches for the sheets that are
//! visibly led to them, and — the one that matters — nothing in the helmsman's hands.
//!
//! The tiller is not decoration. It is the only instrument on this boat that reads
//! continuously and needs no numbers: how far over it is says how hard she is
//! griping, and watching it creep to windward as the breeze builds is how a sailor
//! knows to reef before the heel gauge does. A boat steered by an invisible rudder is
//! being driven, not sailed.

use bevy::prelude::*;

use crate::deck;
use crate::sim::Sim;

/// The tiller, so it can be swung with the rudder.
#[derive(Component)]
pub struct Tiller;

/// Where the rudder stock comes through the deck, metres aft of the centre of gravity.
const STOCK_Z: f32 = 4.60;
/// Length of the tiller forward of the stock.
const TILLER_LENGTH: f32 = 1.70;

pub fn spawn(
    b: &mut ChildSpawnerCommands,
    meshes: &mut Assets<Mesh>,
    varnish: &Handle<StandardMaterial>,
    metal: &Handle<StandardMaterial>,
    painted: &Handle<StandardMaterial>,
) {
    // --- The tiller ----------------------------------------------------------
    //
    // Pivoted at the rudder head and reaching forward into the cockpit, where a hand
    // can be on it. The shaft is offset forward by half its length so the transform's
    // rotation happens about the stock rather than about the middle of the stick.
    b.spawn((
        Tiller,
        Transform::from_xyz(0.0, deck::deck_height(STOCK_Z) + 0.18, STOCK_Z),
        Visibility::default(),
    ))
    .with_children(|t| {
        t.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.055, 0.075, TILLER_LENGTH))),
            MeshMaterial3d(varnish.clone()),
            // Forward of the pivot, and rising slightly, the way a tiller does so it
            // clears the coaming when it is hard over.
            Transform::from_xyz(0.0, 0.10, -TILLER_LENGTH * 0.5)
                .with_rotation(Quat::from_rotation_x(0.10)),
        ));
        // The head of the stock it is clamped to.
        t.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.065, 0.24))),
            MeshMaterial3d(metal.clone()),
            Transform::from_xyz(0.0, 0.0, 0.0),
        ));
    });

    // --- Winches -------------------------------------------------------------
    //
    // Two for the sheets, one each side on the coaming, and one on the coachroof for
    // the bank of slow lines. They are drums rather than models of winches, which is
    // the right level of detail for something seen from two metres away, and they
    // matter because the sheets are already drawn running to them.
    let drum = meshes.add(Cylinder::new(0.105, 0.20));
    let top = meshes.add(Cylinder::new(0.125, 0.035));
    for side in [-1.0_f32, 1.0] {
        let z = 1.9;
        let x = side * deck::deck_half_beam(z) * 0.62;
        let y = deck::deck_height(z) + 0.10;
        b.spawn((
            Mesh3d(drum.clone()),
            MeshMaterial3d(metal.clone()),
            Transform::from_xyz(x, y, z),
        ));
        b.spawn((
            Mesh3d(top.clone()),
            MeshMaterial3d(metal.clone()),
            Transform::from_xyz(x, y + 0.11, z),
        ));
    }
    // The clutch-bank winch, on the coachroof where the lines come aft to.
    b.spawn((
        Mesh3d(drum.clone()),
        MeshMaterial3d(metal.clone()),
        Transform::from_xyz(0.42, deck::cabin_top(0.2) + 0.10, 0.2),
    ));

    // --- Companionway --------------------------------------------------------
    //
    // A cabin with no way into it is a box. The sliding hatch and the washboards under
    // it are the most recognisable thing on a small yacht's coachroof, and they also
    // tell you which end of the cabin is the back.
    let aft = deck::cabin_aft();
    b.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.62, 0.52, 0.04))),
        MeshMaterial3d(varnish.clone()),
        Transform::from_xyz(0.0, deck::cabin_top(aft) - 0.26, aft + 0.02),
    ));
    // The hatch garage, slid shut.
    b.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.70, 0.06, 0.78))),
        MeshMaterial3d(painted.clone()),
        Transform::from_xyz(0.0, deck::cabin_top(aft - 0.5) + 0.04, aft - 0.45),
    ));

    // --- Grabrails -----------------------------------------------------------
    //
    // Along the coachroof, which is what you hold on the way forward. Cheap, and their
    // absence is felt the moment anyone looks at the cabin top and wonders how you are
    // supposed to cross it in a seaway.
    let rail = meshes.add(Cuboid::new(0.05, 0.09, 2.6));
    for side in [-1.0_f32, 1.0] {
        let z = -1.1;
        b.spawn((
            Mesh3d(rail.clone()),
            MeshMaterial3d(varnish.clone()),
            Transform::from_xyz(
                side * deck::cabin_half_beam(z) * 0.72,
                deck::cabin_top(z) + 0.05,
                z,
            ),
        ));
    }
}

/// Swing the tiller with the rudder.
pub fn update_tiller(sim: Res<Sim>, mut q: Query<&mut Transform, With<Tiller>>) {
    let angle = sim.controls.rudder as f32;
    for mut t in &mut q {
        // Opposite to the blade. Positive rudder turns the bow to starboard, which
        // means the blade's trailing edge goes to starboard and the tiller — being
        // forward of the stock rather than aft of it — goes to port. Pushing the
        // tiller away from the wind to bear away is the oldest muscle memory in
        // sailing, and getting it backwards would be felt immediately.
        t.rotation = Quat::from_rotation_y(angle);
    }
}
