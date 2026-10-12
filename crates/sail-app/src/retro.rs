//! The old-games look: render small, show big, and let the pixels show.
//!
//! Asked for right at the start of this project as the fallback if the water never
//! came good, and worth having now that it has — not as a retreat but because the two
//! read completely differently and the choice belongs to whoever is sailing.
//!
//! The method is the one those games used because they had no choice: draw the world
//! at a few hundred pixels across and put it on the screen at whatever size the screen
//! is, with no filtering between. Everything else follows from that. The sea's
//! glitter becomes a dance of individual lit pixels rather than a sheen; the rigging
//! becomes a stepped line; the horizon becomes a hard edge. None of it is a filter
//! painted over a modern image — it is a genuinely small image, which is why it does
//! not look like a modern image with an effect on it.
//!
//! The console and the chart stay sharp. They are drawn afterwards, in screen space,
//! and a pixelated instrument is a nostalgic idea rather than a readable one: those
//! games put their panels in crisp text for the same reason.
//!
//! The grading is the other half. Period hardware had a small palette and a contrast
//! curve nothing like a linear one, so the picture is pushed harder and pulled a
//! little off saturation — which is what the memory of those games actually looks
//! like, as opposed to what they looked like.

use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};

use crate::camera::MainCamera;

/// How many pixels across the world is drawn, at each step.
///
/// Four hundred and eighty is a VGA mode and is where this looks most like what it is
/// remembering; 720 keeps more of the rigging legible; 960 is a nod rather than a
/// commitment.
const WIDTHS: [u32; 3] = [480, 720, 960];

#[derive(Resource, Default)]
pub struct Retro {
    /// `None` is the modern picture.
    pub step: Option<usize>,
    target: Option<Handle<Image>>,
    screen: Option<Entity>,
    sized: (u32, u32),
}

impl Retro {
    pub fn width(&self) -> Option<u32> {
        self.step.map(|s| WIDTHS[s])
    }

    /// Start in the pixelated picture, from `SAIL_PIXELS` in the environment.
    ///
    /// Takes the width you want and picks the nearest step, so `SAIL_PIXELS=480` means
    /// what it says and anything else lands somewhere sensible.
    pub fn from_env() -> Self {
        let Some(want) = std::env::var("SAIL_PIXELS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
        else {
            return Self::default();
        };
        let step = WIDTHS
            .iter()
            .enumerate()
            .min_by_key(|(_, w)| w.abs_diff(want))
            .map(|(i, _)| i);
        Self {
            step,
            ..Self::default()
        }
    }
}

/// `V` steps through sharp, 960, 720, 480 and back.
pub fn toggle(keys: Res<ButtonInput<KeyCode>>, mut retro: ResMut<Retro>) {
    if !keys.just_pressed(KeyCode::KeyV) {
        return;
    }
    retro.step = match retro.step {
        None => Some(2),
        Some(2) => Some(1),
        Some(1) => Some(0),
        _ => None,
    };
}

/// Point the camera at a small image, and put that image on the screen.
pub fn apply(
    mut commands: Commands,
    mut retro: ResMut<Retro>,
    mut images: ResMut<Assets<Image>>,
    windows: Query<&Window>,
    cameras: Query<Entity, With<MainCamera>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    // In Bevy 0.20 the render target is its own component rather than a field on the
    // camera, so it is inserted and removed rather than assigned.
    let Ok(camera) = cameras.single() else {
        return;
    };
    let physical = (window.physical_width(), window.physical_height());
    if physical.0 == 0 || physical.1 == 0 {
        return;
    }

    let wanted = retro.width().map(|w| {
        let h = (w as f32 * physical.1 as f32 / physical.0 as f32)
            .round()
            .max(1.0) as u32;
        (w, h)
    });

    match wanted {
        None => {
            if retro.target.take().is_some() {
                commands.entity(camera).remove::<RenderTarget>();
                if let Some(screen) = retro.screen.take() {
                    commands.entity(screen).despawn();
                }
                retro.sized = (0, 0);
                info!("picture: sharp");
            }
        }
        Some(size) => {
            if retro.sized == size && retro.target.is_some() {
                return;
            }
            // A new image, because the size changed or there was none. Nearest
            // filtering on the way out is the whole point: a linear filter turns this
            // back into a blurry modern picture, which is the one thing it must not be.
            let mut image = Image::new_fill(
                Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                &[0, 0, 0, 255],
                TextureFormat::Rgba8UnormSrgb,
                bevy::asset::RenderAssetUsages::RENDER_WORLD,
            );
            image.texture_descriptor.usage =
                TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING;
            image.sampler = bevy::image::ImageSampler::nearest();
            let handle = images.add(image);

            commands
                .entity(camera)
                .insert(RenderTarget::Image(handle.clone().into()));

            if let Some(screen) = retro.screen.take() {
                commands.entity(screen).despawn();
            }
            // A second camera, drawn after the first, showing the small image blown up
            // to fill the window. `Sprite::sized` with `ScalingMode` would letterbox;
            // stretching to the window keeps the pixels square because the image's
            // aspect was taken from the window in the first place.
            let screen = commands
                .spawn((
                    Camera2d,
                    Camera {
                        order: 1,
                        ..default()
                    },
                ))
                .with_children(|c| {
                    // A child of the camera, so switching away takes it with it. It
                    // was a loose entity, which meant every change of picture left
                    // another full-screen sprite behind it — invisible, stacked, and
                    // the sort of leak that is only ever found by someone wondering
                    // why the frame rate fell over an hour.
                    c.spawn((
                        Sprite {
                            image: handle.clone(),
                            custom_size: Some(Vec2::new(window.width(), window.height())),
                            ..default()
                        },
                        Transform::default(),
                    ));
                })
                .id();
            retro.screen = Some(screen);
            retro.target = Some(handle);
            retro.sized = size;
            info!(
                "picture: {} by {} pixels, scaled up {:.1} times",
                size.0,
                size.1,
                physical.0 as f32 / size.0 as f32
            );
        }
    }
}

/// Lay the instruments out as if the screen were its usual size.
///
/// They are drawn into the same small image as the world, which is what gives them
/// their blocky look — and, left alone, also makes one gauge fill a quarter of the
/// window, because egui was laying out in a four-hundred-and-eighty-point screen and
/// the result was then blown up six times.
///
/// Telling egui how many of its points go to a pixel fixes the size without touching
/// the look: the panels come out the same fraction of the screen they always were,
/// drawn at a third of the density. Chunky text at a readable size, which is what the
/// old games had — they were not laid out small, they were laid out normally on a
/// screen that had few pixels.
pub fn scale_instruments(
    retro: Res<Retro>,
    windows: Query<&Window>,
    mut contexts: bevy_egui::EguiContexts,
) -> Result {
    let Ok(window) = windows.single() else {
        return Ok(());
    };
    let logical = window.width().max(1.0);
    let ppp = match retro.width() {
        Some(w) => w as f32 / logical,
        None => 1.0,
    };
    let ctx = contexts.ctx_mut()?;
    if (ctx.pixels_per_point() - ppp).abs() > 1e-4 {
        ctx.set_pixels_per_point(ppp);
    }
    Ok(())
}
