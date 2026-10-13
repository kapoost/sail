//! The horizon, drawn as a line.
//!
//! Because that is what it is. Every draughtsman, every printmaker and every manga
//! artist draws the horizon with a stroke, and none of them arrives at it by shading
//! the sea down until it meets the sky — the line comes first and the tones hang off
//! it. Up to now it was the second thing: the edge where the sea's last fogged tone
//! happened to differ from the sky's first, which at four hundred and eighty pixels
//! across is a row of pixels that moves in whole rows as the boat heaves.
//!
//! Where to put it is the one decision worth writing down. The sea here is a flat
//! plane, so its vanishing line is at eye level however far you can see — the apparent
//! horizon is not at any particular distance, it is *at infinity*, and a line drawn at
//! any range sits a fraction of a degree below it. So the range is chosen for occlusion
//! rather than for geometry: just inside the far plane, which puts every island in
//! front of it. Draw it nearer and it cuts across the far ones, which no drawing of a
//! coast has ever done.
//!
//! And it is not fogged. A drawn line is not atmospheric perspective — a pen does not
//! fade because the thing is far away, it stops or it does not. Fogging this would hide
//! it completely, since it sits three times the visibility away.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

/// Segments round the ring. Enough that the straight chords are shorter than a pixel at
/// this range: at seven kilometres and a forty-one degree lens, a hundred and twenty
/// gives a chord of about a tenth of a degree.
const SEGMENTS: usize = 120;
/// How far out the ring sits, metres. Inside the camera's eight-kilometre far plane
/// with room to spare, and beyond every island the generator places.
const RANGE: f32 = 7_600.0;

#[derive(Component)]
pub struct HorizonLine;

/// The ring, excluding the camera it follows.
type Ring<'w, 's> = Query<
    'w,
    's,
    (&'static mut Transform, &'static mut Visibility),
    (With<HorizonLine>, Without<crate::camera::MainCamera>),
>;

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut positions: Vec<Vec3> = Vec::with_capacity(SEGMENTS + 1);
    for i in 0..=SEGMENTS {
        let a = std::f32::consts::TAU * i as f32 / SEGMENTS as f32;
        positions.push(Vec3::new(a.sin() * RANGE, 0.0, -a.cos() * RANGE));
    }
    let mut indices = Vec::with_capacity(SEGMENTS * 2);
    for i in 0..SEGMENTS {
        indices.push(i as u32);
        indices.push(i as u32 + 1);
    }
    let mut mesh = Mesh::new(PrimitiveTopology::LineList, RenderAssetUsages::RENDER_WORLD);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![Vec3::Y; positions.len()]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_indices(Indices::U32(indices));

    commands.spawn((
        HorizonLine,
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            // The same ink as everything else, and unlit for the same reason: a lit
            // dark line picks up the sky and becomes a grey rim.
            base_color: Color::linear_rgb(0.035, 0.045, 0.062),
            unlit: true,
            fog_enabled: false,
            ..default()
        })),
        Transform::default(),
        Visibility::Hidden,
        bevy::light::NotShadowCaster,
        bevy::light::NotShadowReceiver,
    ));
}

/// Keep it centred on the eye, and only draw it when the picture is drawn.
///
/// Centred on the camera rather than on the boat: the ring has to stay concentric with
/// the eye or one side of it comes closer than the other and the line tilts off level,
/// which is the one thing a horizon may never do.
pub fn follow(
    ink: Res<crate::world::Ink>,
    camera: Query<&Transform, With<crate::camera::MainCamera>>,
    mut ring: Ring,
) {
    let Ok(view) = camera.single() else {
        return;
    };
    let Ok((mut t, mut seen)) = ring.single_mut() else {
        return;
    };
    // At the water, under the eye. Not at the eye's own height: the ring is the edge of
    // the sea, and the sea is at zero.
    t.translation = Vec3::new(view.translation.x, 0.0, view.translation.z);
    *seen = if ink.0 {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
}
