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
//! # Why it is drawn twice the size and then halved
//!
//! Because a small picture and a *badly sampled* small picture are not the same thing,
//! and only one of them is the look. Drawn straight at four hundred and eighty across,
//! the horizon is a step function: the sea/sky boundary falls inside one row of pixels,
//! and that row is six pixels tall on the screen, so as the boat heaves the whole
//! horizon jumps up and down in six-pixel lurches. The rigging does the same thing
//! sideways — a shroud a third of a pixel wide either lands on a pixel or does not, so
//! it flickers on and off as the mast sways. Neither is what the period hardware did:
//! those machines drew *sprites*, hand-placed and hand-shaded, and the one thing a hand
//! never does is drop a line because it fell between two pixels.
//!
//! So the world is rendered at twice the width and twice the height and then averaged
//! two-by-two into the small grid. The grid is identical — same chunky pixels, same
//! nearest-neighbour blow-up, no filtering on the way to the screen. What changes is
//! what is *in* each pixel: the average of the four samples that fell inside it, so the
//! horizon moves through tones instead of jumping rows, and a wire too thin to fill a
//! pixel comes out as a dimmer pixel rather than as nothing. Genuinely antialiased
//! pixel art, which is what a pixel artist does by hand with a brush.
//!
//! **Off by default, and that is a decision rather than caution.** It works and it
//! costs about what it should, but the sharpness of the boat and the horizon is being
//! solved with *line* instead — a drawn edge at this size carries more than a better
//! sampled one, which is the same reason the instruments are text and not pixels. So
//! the machinery stays, switched off, one variable away: `SAIL_SUPERSAMPLE=1`. It is
//! worth keeping because it is the only honest answer to the horizon *jumping*, which
//! is a sampling fault and not a drawing one, and if the lines do not settle that then
//! this is where to come back to.
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

/// How many samples across each pixel of the small picture the world is drawn with.
///
/// Two, and two only: the reduction is done by a linear sampler drawing the big image
/// at exactly half size, which lands each output texel dead between four input texels
/// and therefore averages them exactly. At any other ratio bilinear picks four of the
/// nine and the filter stops being a box, so three would need a shader to be honest.
const SUPERSAMPLE: u32 = 2;

#[derive(Resource, Default)]
pub struct Retro {
    /// `None` is the modern picture.
    pub step: Option<usize>,
    /// On with `SAIL_SUPERSAMPLE=1`. Off by default — see the module docs.
    supersample: bool,
    target: Option<Handle<Image>>,
    /// The cameras that reduce the big picture and then blow the small one up.
    passes: Vec<Entity>,
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
            // Seven hundred and twenty, which is where this ends up every time somebody
            // sits down with it. Sharp was the default on the reasoning that a new
            // player should see the most of what is drawn; what a new player actually
            // sees is a picture with no opinion about itself, and the whole look of this
            // thing — the drawn horizon, the flat tones, the stepped rigging — is built
            // for a small image. `SAIL_PIXELS=0` still asks for sharp.
            return Self {
                step: Some(1),
                ..Self::default()
            };
        };
        if want == 0 {
            return Self::default();
        }
        let step = WIDTHS
            .iter()
            .enumerate()
            .min_by_key(|(_, w)| w.abs_diff(want))
            .map(|(i, _)| i);
        Self {
            step,
            supersample: std::env::var("SAIL_SUPERSAMPLE").is_ok_and(|v| v != "0"),
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

/// Cycle the picture on a timer, to go looking for the freeze on `V`.
///
/// `SAIL_RETRO_STRESS=1`. A fault that happens "sometimes" when a key is pressed is a
/// fault nobody can hold still long enough to look at, and pressing the key by hand a
/// hundred times is not a measurement. This presses it a hundred times and counts the
/// frames between, so if the picture ever stops the log stops with it and says at which
/// step it was standing when it did.
pub fn stress(
    time: Res<Time>,
    mut retro: ResMut<Retro>,
    mut since: Local<f64>,
    mut frames: Local<u64>,
) {
    *frames += 1;
    *since += time.delta_secs_f64();
    let gap: f64 = std::env::var("SAIL_RETRO_STRESS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.4);
    if *since < gap {
        return;
    }
    *since = 0.0;
    retro.step = match retro.step {
        None => Some(2),
        Some(2) => Some(1),
        Some(1) => Some(0),
        _ => None,
    };
    info!("stress: now {:?} after {} frames", retro.step, *frames);
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
                for pass in retro.passes.drain(..) {
                    commands.entity(pass).despawn();
                }
                retro.sized = (0, 0);
                info!("picture: sharp");
            }
        }
        Some(size) => {
            if retro.sized == size && retro.target.is_some() {
                return;
            }
            let ss = if retro.supersample { SUPERSAMPLE } else { 1 };

            // Two images. The world is drawn into the big one and averaged into the
            // small one, and only the small one is ever shown.
            //
            // The samplers are the whole of the method and they are opposites on
            // purpose. Linear on the way *down*, because at exactly half size a
            // bilinear tap is a four-texel box average and that is the filter we want.
            // Nearest on the way *out*, because a linear filter there turns this back
            // into a blurry modern picture, which is the one thing it must not be.
            let big = canvas(
                size.0 * ss,
                size.1 * ss,
                bevy::image::ImageSampler::linear(),
            );
            let small = canvas(size.0, size.1, bevy::image::ImageSampler::nearest());
            let big = images.add(big);
            let small = images.add(small);

            commands
                .entity(camera)
                .insert(RenderTarget::Image(big.clone().into()));

            for pass in retro.passes.drain(..) {
                commands.entity(pass).despawn();
            }

            // The reduction. A camera whose target is the small image, showing the big
            // one at exactly the small one's size. A 2D camera's units are its target's
            // pixels, so a sprite of that size centred on the origin covers it exactly —
            // which is what makes the sample positions land where the box filter needs
            // them rather than half a texel off.
            if ss > 1 {
                let reduce = commands
                    .spawn((
                        Camera2d,
                        Camera {
                            order: 1,
                            ..default()
                        },
                        // In Bevy 0.20 the target is its own component, which is also
                        // how the main camera is pointed at the big image.
                        RenderTarget::Image(small.clone().into()),
                    ))
                    .with_children(|c| {
                        c.spawn((
                            Sprite {
                                image: big.clone(),
                                custom_size: Some(Vec2::new(size.0 as f32, size.1 as f32)),
                                ..default()
                            },
                            Transform::default(),
                        ));
                    })
                    .id();
                retro.passes.push(reduce);
            }

            // And the blow-up, drawn after it, filling the window. `Sprite::sized` with
            // a scaling mode would letterbox; stretching to the window keeps the pixels
            // square because the image's aspect came from the window in the first place.
            let shown = if ss > 1 { small.clone() } else { big.clone() };
            let screen = commands
                .spawn((
                    Camera2d,
                    Camera {
                        order: 2,
                        ..default()
                    },
                ))
                .with_children(|c| {
                    // A child of the camera, so switching away takes it with it. It was
                    // a loose entity, which meant every change of picture left another
                    // full-screen sprite behind it — invisible, stacked, and the sort of
                    // leak only ever found by someone wondering why the frame rate fell
                    // over an hour.
                    c.spawn((
                        Sprite {
                            image: shown,
                            custom_size: Some(Vec2::new(window.width(), window.height())),
                            ..default()
                        },
                        Transform::default(),
                    ));
                })
                .id();
            retro.passes.push(screen);
            retro.target = Some(small);
            retro.sized = size;
            info!(
                "picture: {} by {} pixels from {} by {} samples, scaled up {:.1} times",
                size.0,
                size.1,
                size.0 * ss,
                size.1 * ss,
                physical.0 as f32 / size.0 as f32
            );
        }
    }
}

/// A render target of a given size and sampling.
fn canvas(width: u32, height: u32, sampler: bevy::image::ImageSampler) -> Image {
    let mut image = Image::new_fill(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.usage =
        TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING;
    image.sampler = sampler;
    image
}

/// The camera the instruments are drawn on.
///
/// Its own, always pointed at the window, and therefore always at the full density of
/// the screen. `bevy_egui` otherwise puts its context on the first camera the
/// application makes — the one looking at the sea — and the moment that starts drawing
/// into a 480-pixel image the console goes in with it.
///
/// Both of the obvious ways round that were tried and are in the history. Laying the
/// panels out in the small image makes them six times too big; scaling the layout down
/// to fit puts a nine-point label on three pixels, at which glyphs stop having shapes,
/// and growing the type inside that layout only trades legibility for a console that
/// no longer fits its own boxes. There is no setting that makes a readable instrument
/// out of 480 pixels, because the panel genuinely needs about fourteen hundred points
/// of width.
///
/// So the panel is drawn at the resolution it needs and the sea at the resolution it
/// wants. That is not a compromise between the two looks, it is the one arrangement in
/// which each is what it should be — and the crisp lettering over a coarse sea is the
/// same trick as the sharp wind over it: one deliberate layer of detail on top of a
/// deliberately coarse picture.
#[derive(Component)]
pub struct UiCamera;

pub fn setup_ui_camera(mut commands: Commands) {
    commands.spawn((
        UiCamera,
        Camera2d,
        Camera {
            // After the world and after the upscaled picture, clearing neither.
            order: 3,
            clear_color: bevy::camera::ClearColorConfig::None,
            ..default()
        },
        bevy_egui::PrimaryEguiContext,
    ));
}

/// Put a terminal face on every instrument.
///
/// The last thing that could not be pixelated. The console is drawn at the full
/// resolution of the screen because no arrangement makes a readable panel out of four
/// hundred and eighty pixels — but a *typeface* can be built on a pixel grid and drawn
/// sharply, which gets the look without giving up the legibility. The letters are
/// blocky because their shapes are blocky, not because they are being resampled.
///
/// VT323 is a revival of the DEC VT320's character set. It is not a costume: a boat's
/// instrument panel is a terminal, and this is what terminals looked like. It is also
/// narrow, which is what let it drop into a layout drawn for something else without
/// bursting a single box.
///
/// The tweak is needed because the face is drawn small for its nominal size — a
/// terminal cell is tall and thin — so everything would come out a third smaller than
/// the sizes the panel asks for.
pub fn install_font(mut contexts: bevy_egui::EguiContexts) -> Result {
    use bevy_egui::egui::{FontData, FontDefinitions, FontFamily, FontTweak};
    let ctx = contexts.ctx_mut()?;
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "vt323".into(),
        std::sync::Arc::new(
            FontData::from_static(include_bytes!("../../../assets/VT323-Regular.ttf")).tweak(
                FontTweak {
                    scale: 1.32,
                    y_offset_factor: -0.06,
                    ..default()
                },
            ),
        ),
    );
    // Both families, because the panel asks for monospace and the chart for the
    // proportional one, and a console with two eras of lettering on it is worse than
    // either.
    for family in [FontFamily::Monospace, FontFamily::Proportional] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, "vt323".into());
    }
    ctx.set_fonts(fonts);
    Ok(())
}

/// How large the instruments are drawn, against the screen's own density.
///
/// Below one they shrink. The console was laid out for a window of its own and takes
/// a good share of a cockpit view; a little under is enough to give the sea back the
/// middle of the screen without making anything hard to hit with a mouse.
const UI_SCALE: f32 = 0.84;

/// Set the instruments' size. Their own camera means this is independent of the
/// picture the world is drawn in.
pub fn scale_instruments(windows: Query<&Window>, mut contexts: bevy_egui::EguiContexts) -> Result {
    let Ok(window) = windows.single() else {
        return Ok(());
    };
    let ppp = window.scale_factor() * UI_SCALE;
    let ctx = contexts.ctx_mut()?;
    if (ctx.pixels_per_point() - ppp).abs() > 1e-4 {
        ctx.set_pixels_per_point(ppp);
    }
    Ok(())
}
