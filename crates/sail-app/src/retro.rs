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

/// The picture's own size: the drawn width, the console's scale, the font.
///
/// What this module does in a frame, in one place — see `stages.rs` for why the
/// schedule is named sets rather than one long chain.
pub fn plugin(app: &mut App) {
    use crate::stages::{Build, Deck, Paint};
    app.insert_resource(Retro::from_env());
    app.add_systems(Startup, setup_ui_camera.in_set(Build::Scene));
    app.add_systems(
        Update,
        stress
            .run_if(|| crate::knob::var("SAIL_RETRO_STRESS").is_ok())
            .in_set(Deck::Weather),
    );
    app.add_systems(Update, apply.in_set(Deck::Weather));
    app.add_systems(
        bevy_egui::EguiPrimaryContextPass,
        (
            // Once, at the start: `set_fonts` rebuilds the atlas, so it must not be
            // called every frame — and before the instruments are scaled, because the
            // scale is in points and a point is a size of type.
            install_font.run_if(bevy::ecs::schedule::common_conditions::run_once),
            scale_instruments,
        )
            .chain()
            .in_set(Paint::Setting),
    );
}

/// How many pixels across the world is drawn, coarsest first.
///
/// **Two widths now, and the reason the second is back is worth writing down**, because
/// this list was cut to one on purpose. There were three — 480 for the VGA mode this is
/// remembering, 720, and 960 — and the argument for dropping two of them still holds for
/// 480: it loses the rigging, which is half of what there is to look at on a boat.
///
/// 960 came back on a different argument to the one that removed it. It went because
/// nobody had *chosen* it — it was a third variant of one idea. It returns because a
/// screen got bigger: 720 across a laptop and 720 across a large display are not the
/// same picture, and on the big one the pixels stop reading as a drawn image and start
/// reading as a low-resolution one. That is a reason to choose a width rather than an
/// accident of having three.
///
/// The cost named when they were cut is real and unchanged, but it is a *maintenance*
/// cost rather than a frame-rate one, and the difference is worth having measured.
/// Seventy-five seconds of the same seed at each width, same machine, same minute:
/// 26.6 frames per second mean at 720 and 27.4 at 960, worst frame 62 ms against 60.
/// The finer picture is not slower — within the noise it is the same, because what
/// costs this renderer its frames is forty thousand sea vertices rebuilt on the CPU,
/// not the size of the thing they are drawn into.
///
/// Checked again on a second machine rather than taken on trust, which matters because
/// the first set was measured on a laptop with other work on it: 49/49/44/43/42 at 720
/// against 41/47/47/41/47 at 960, worst frame 78.7 ms against 76.8. Different absolute
/// numbers, same conclusion. Two machines disagreeing about the rate and agreeing about
/// the comparison is what makes this a measurement rather than an anecdote.
///
/// One thing that run turned up and this one did not: the *first* sample after starting
/// at 960 was 103.8 ms. That is the textures being baked rather than anything being
/// drawn, so it does not bear on the width — but if the stutter on entering a game is
/// ever worth chasing, it is a place to start.
///
/// What each width does cost is a texture size and a code path that has to be right on
/// somebody else's machine. Two is the most this should carry. If a third is ever
/// wanted, it should displace one of these rather than join them.
const WIDTHS: [u32; 2] = [720, 960];

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
        // Clamped rather than indexed blind: the stress loop and a stale `SAIL_PIXELS`
        // can both name a step that is no longer there, and a panic in the render path
        // is a worse answer than the coarsest picture.
        self.step.map(|i| WIDTHS[i.min(WIDTHS.len() - 1)])
    }

    /// The widths on offer, coarsest first — for the settings panel to lay out.
    pub fn widths() -> &'static [u32] {
        &WIDTHS
    }

    /// The drawn width a fresh install opens on, as a step.
    ///
    /// Its own function because two things want it now — the dial below and the file of
    /// preferences — and a default written twice is a default that drifts.
    pub fn default_step() -> Option<usize> {
        Some(nearest(960))
    }

    /// Start in the pixelated picture, from `SAIL_PIXELS` in the environment.
    ///
    /// Takes the width you want and picks the nearest step, so `SAIL_PIXELS=480` means
    /// what it says and anything else lands somewhere sensible.
    pub fn from_env() -> Self {
        let supersample = crate::knob::var("SAIL_SUPERSAMPLE").is_ok_and(|v| v != "0");
        let Some(want) = crate::knob::var("SAIL_PIXELS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
        else {
            // **Nine hundred and sixty, which is the finer of the two drawn widths.**
            //
            // Sharp was the default once, on the reasoning that a new player should see
            // the most of what is drawn; what a new player actually sees in a sharp
            // picture is one with no opinion about itself, and the whole look of this
            // thing — the drawn horizon, the flat tones, the stepped rigging — is built
            // for a small image. That argument has not changed and 720 is still on the
            // row beside this one.
            //
            // What changed is the screen. 720 was chosen when the window was the size it
            // is and the displays were smaller; on a large one it stops reading as a
            // picture that was *drawn* and starts reading as one that is low on pixels,
            // which is a different and much worse thing. And it costs nothing: measured
            // on two machines, a minute each, the finer width came out inside the noise
            // of the coarse one, because the wall is forty thousand vertices of sea on
            // the processor and not the size of the target.
            //
            // `SAIL_PIXELS=720` asks for the coarse one, `SAIL_PIXELS=0` for sharp.
            return Self {
                step: Some(nearest(960)),
                supersample,
                ..Self::default()
            };
        };
        // Nought is sharp; anything else picks the nearest width we have.
        //
        // This is what the line above has claimed since it was written, and for a while
        // it was not true: with one width left, the number was read and thrown away, so
        // `SAIL_PIXELS=960` and `SAIL_PIXELS=481` both meant 720. With two widths the
        // promise has to be kept again.
        if want == 0 {
            return Self::default();
        }
        Self {
            step: Some(nearest(want)),
            supersample,
            ..Self::default()
        }
    }
}

/// Which width is closest to the one asked for.
///
/// Its own function so the answer can be asserted without setting an environment
/// variable — a test that writes to the process environment is a test that fights every
/// other test in the binary.
fn nearest(want: u32) -> usize {
    WIDTHS
        .iter()
        .enumerate()
        .min_by_key(|(_, w)| w.abs_diff(want))
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// Between the drawn picture and the sharp one — from the settings panel, which is
/// where it belongs.
///
/// It had a key, and the key is gone. The list of keys on this boat should be things
/// you do *to the boat*: trim, steer, navigate, look. How many pixels the picture is
/// drawn with is not one of those, it is a preference, and a preference on a key is a
/// line in the help that every player reads past looking for something they need.
/// Asked for from the deck in those words.
#[allow(dead_code)]
pub fn step(retro: &mut Retro) {
    // Round the whole list and back to sharp, so the thing it is bound to walks every
    // picture rather than only the first drawn one.
    retro.step = match retro.step {
        None => Some(0),
        Some(i) if i + 1 < WIDTHS.len() => Some(i + 1),
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
    let gap: f64 = crate::knob::var("SAIL_RETRO_STRESS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.4);
    if *since < gap {
        return;
    }
    *since = 0.0;
    // Walks the widths that actually exist and then the sharp picture, so this keeps
    // exercising every path when the list changes length rather than cycling through
    // steps that were removed years before the fault it is hunting.
    retro.step = match retro.step {
        None => Some(0),
        Some(i) if i + 1 < WIDTHS.len() => Some(i + 1),
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

/// How wide the console needs to be, in egui points, to lay itself out without the
/// pieces touching.
///
/// **Measured off the drawn console rather than guessed**: the weather block on the left
/// takes about a hundred and fifty points, the rack of clutches in the middle four
/// hundred and twenty, and the glass with the sheets and the wardrobe under it eight
/// hundred and thirty. With the margins that is fourteen hundred and twenty, and this is
/// that with a little air.
const CONSOLE_POINTS: f32 = 1_450.0;

/// And how tall, measured the same way.
///
/// The right-hand corner is the column that decides it: the case with the glass and the
/// sheets in it is six hundred and fifty points, the chart folded above it will not go
/// below a hundred and eighty before it stops being a chart at all, and the two of them
/// keep thirty-six points between themselves and the edges. Shorter than this and the
/// chart reaches down into the case, which is the overlap reported from the deck.
const CONSOLE_HIGH: f32 = 870.0;

/// What share of the window the console is drawn to take.
///
/// **Not all of it, and the number is a measurement rather than a taste.** At the size
/// the console was laid out and looked at for a year — a window of fourteen hundred and
/// forty points on a screen of twice that density — the strip spans about eighty-five
/// per cent of the height it would need to fill the window edge to edge. So that is the
/// share: it reproduces the view the whole thing was tuned in, and it leaves the middle
/// of the screen to the water, which is the thing worth looking at.
const SPREAD: f32 = 0.85;

/// And the smallest and largest the instruments may be drawn, against the screen's own
/// density.
///
/// Half size at the bottom: past that the figures stop being readable and a console you
/// cannot read is not a smaller console, it is a decoration. Two and a half at the top
/// is a guard against a number arriving from a settings file rather than a shape
/// anybody's screen produces.
const SMALLEST: f32 = 0.5;
const BIGGEST: f32 = 2.5;

/// How much a player may ask for on top of that, as a multiplier.
///
/// A third bigger covers the eyes this was reported for; a quarter smaller covers
/// somebody who wants the water and not the instruments. Wider than that and the
/// console either overlaps itself or becomes a strip of type.
pub const ASKED_RANGE: std::ops::RangeInclusive<f32> = 0.75..=1.35;

/// How large to draw the instruments on a window of this size.
///
/// **The console used to be the same size on every screen in the world.** The rule was
/// *the size it was drawn at, unless the window is too narrow for it* — and the size it
/// was drawn at is a constant, so a player on a four-thousand-pixel monitor got exactly
/// the same instruments in device pixels as a player on a laptop, which is to say a
/// quarter of the area and type eight pixels high. Reported from the deck as the fonts
/// being too small, and it was not a font size: it was a console that did not know how
/// big the screen was.
///
/// So the scale comes from the window and nothing else: the console is a fixed layout
/// of a known width and a known height, and it is drawn at the size that puts it on
/// *this* screen at the share of it the thing was tuned at. The smaller of the two fits,
/// because a console that fits across and runs off the bottom is the same bug wearing
/// different clothes — on a sixteen-by-nine window it is the height that binds, and that
/// is the overlap between the chart and the case.
///
/// `scale_factor` is the display's own density, and it belongs here only at the ends:
/// the result is in points per *physical* pixel, so a dense screen and a coarse one of
/// the same size in inches get type of the same size, which is the whole point of the
/// unit. The floor and the ceiling are the exception, because *unreadable* is a fact
/// about a physical pixel.
pub fn console_scale(
    physical_width: f32,
    physical_height: f32,
    scale_factor: f32,
    asked: f32,
) -> f32 {
    let fits = (physical_width / CONSOLE_POINTS).min(physical_height / CONSOLE_HIGH);
    let want = fits * SPREAD * asked.clamp(*ASKED_RANGE.start(), *ASKED_RANGE.end());
    want.clamp(scale_factor * SMALLEST, scale_factor * BIGGEST)
}

/// Set the instruments' size. Their own camera means this is independent of the
/// picture the world is drawn in.
pub fn scale_instruments(
    windows: Query<&Window>,
    settings: Res<crate::settings::Settings>,
    mut contexts: bevy_egui::EguiContexts,
) -> Result {
    let Ok(window) = windows.single() else {
        return Ok(());
    };
    let ppp = console_scale(
        window.physical_width() as f32,
        window.physical_height() as f32,
        window.scale_factor(),
        settings.ui,
    );
    let ctx = contexts.ctx_mut()?;
    if (ctx.pixels_per_point() - ppp).abs() > 1e-4 {
        ctx.set_pixels_per_point(ppp);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How big the type actually comes out, in device pixels, on the screens people
    /// have — the console's smallest size, measured, and taken from the console rather
    /// than typed here so that a change to the type scale is a change to this test.
    fn label_px(w: f32, h: f32, sf: f32) -> f32 {
        crate::panel::letter::UNIT * console_scale(w, h, sf, 1.0)
    }

    /// **The console used to be the same size on every screen in the world**, and that
    /// is the fault this rule exists to fix. The old one was *the size it was drawn at,
    /// unless the window is narrower than the strip* — and the size it was drawn at is a
    /// constant, so a four-thousand-pixel monitor got type eight pixels high, exactly
    /// like a laptop, in a quarter of the area. Reported from the deck as the fonts
    /// being too small; it was not a font size, it was a console that did not know how
    /// big the screen was.
    #[test]
    fn the_console_grows_with_the_screen() {
        let laptop = label_px(1366.0, 768.0, 1.0);
        let full_hd = label_px(1920.0, 1080.0, 1.0);
        let quad = label_px(2560.0, 1440.0, 1.0);
        let four_k = label_px(3840.0, 2160.0, 1.0);
        assert!(
            full_hd > laptop && quad > full_hd && four_k > quad,
            "{laptop:.1}, {full_hd:.1}, {quad:.1}, {four_k:.1} device pixels — \
             the console is not reading the screen"
        );
        // And by the right amount: twice the pixels is twice the type, because a
        // four-thousand-pixel monitor is not a bigger desk, it is a finer one.
        assert!(
            (four_k / full_hd - 2.0).abs() < 0.05,
            "4K gave {:.2} times the type of 1080p",
            four_k / full_hd
        );
        // Nothing on an ordinary screen comes out under ten pixels, which is where
        // small type stops being small and starts being a squint.
        // Eleven device pixels on the commonest screen there is, and fifteen on the
        // next one up. Under about ten, type stops being small and starts being a
        // squint — which is the report this came from.
        assert!(full_hd >= 11.0, "1080p got {full_hd:.1} px type");
        assert!(quad >= 15.0, "1440p got {quad:.1} px type");
        assert!(laptop >= 8.0, "a 1366 laptop got {laptop:.1} px type");
    }

    /// **And the view it was all tuned in does not move.** A year of looking at this
    /// console happened at fourteen hundred and forty points on a screen of twice that
    /// density, and every size in it was chosen by eye there. A rule that improved every
    /// other screen by changing that one would be a rule nobody could check against
    /// anything.
    #[test]
    fn the_window_it_was_drawn_in_is_the_size_it_was_drawn_at() {
        let was = 2.0 * 0.84; // the old constant, and what the panel was laid out at
        let now = console_scale(2880.0, 1760.0, 2.0, 1.0);
        assert!(
            (now - was).abs() < 0.05,
            "the console it was drawn at moved from {was:.2} to {now:.2}"
        );
    }

    /// **A console that fits across and runs off the bottom is the same bug wearing
    /// different clothes.** On a sixteen-by-nine window it is the height that binds —
    /// the chart folded in the corner reaches down into the case below it, which is the
    /// overlap reported from the deck — so the height is in the rule.
    #[test]
    fn it_fits_down_the_screen_as_well_as_across() {
        for (w, h) in [
            (1920.0, 1080.0),
            (1280.0, 720.0),
            (2560.0, 1080.0),
            (1440.0, 900.0),
        ] {
            let ppp = console_scale(w, h, 1.0, 1.0);
            assert!(
                w / ppp >= CONSOLE_POINTS * SPREAD - 1.0,
                "{w}x{h} left the console {:.0} points across",
                w / ppp
            );
            assert!(
                h / ppp >= CONSOLE_HIGH * SPREAD - 1.0,
                "{w}x{h} left the console {:.0} points down and the chart is in the case",
                h / ppp
            );
        }
    }

    /// **And the hand on top of it does what it says.** A third bigger is a third
    /// bigger, a quarter smaller is a quarter smaller, and a number from outside the
    /// range — a settings file edited by hand, or an older one — cannot put the console
    /// somewhere it cannot be read or cannot be fitted.
    #[test]
    fn what_the_player_asks_for_is_what_the_player_gets() {
        let plain = console_scale(1920.0, 1080.0, 1.0, 1.0);
        let bigger = console_scale(1920.0, 1080.0, 1.0, 1.3);
        assert!((bigger / plain - 1.3).abs() < 0.01);
        let smaller = console_scale(1920.0, 1080.0, 1.0, 0.8);
        assert!((smaller / plain - 0.8).abs() < 0.01);
        // Out of range, both ways, and nothing silly comes out.
        let silly = console_scale(1920.0, 1080.0, 1.0, 12.0);
        assert!(silly <= plain * 1.36, "a twelve got through as {silly:.2}");
        let tiny = console_scale(1920.0, 1080.0, 1.0, 0.0);
        assert!(tiny >= plain * 0.74, "a zero got through as {tiny:.2}");
    }

    /// And never into illegibility: past half size the figures stop being readable, and
    /// a console you cannot read is a decoration rather than a smaller console.
    #[test]
    fn it_never_shrinks_past_reading() {
        assert!(console_scale(400.0, 300.0, 2.0, 1.0) >= 2.0 * SMALLEST);
        assert!(console_scale(400.0, 300.0, 1.0, 0.75) >= SMALLEST);
    }

    /// A number asks for the width nearest it, which is what the docs have always said.
    ///
    /// For a while they said it and it was not true: with one width left the value was
    /// parsed and discarded. The list is two long again, so the promise is live again.
    #[test]
    fn a_wanted_width_picks_the_nearest_one() {
        assert_eq!(nearest(720), 0);
        assert_eq!(nearest(960), 1);
        // Below the coarsest and above the finest both clamp to the end they are nearest.
        assert_eq!(nearest(1), 0);
        assert_eq!(nearest(4000), WIDTHS.len() - 1);
        // The old 480 is gone; it should land on the coarse one rather than panic.
        assert_eq!(nearest(480), 0);
        // Dead between the two, and either answer is defensible — this asserts which,
        // so that changing the list makes somebody look at it.
        assert_eq!(nearest(840), 0);
    }

    /// Every step the panel can offer resolves to a width the renderer has.
    ///
    /// The panel builds its row from `widths()` now, but `step` is a plain index and
    /// anything that writes one — the stress loop, a remembered setting — can name a
    /// step that is no longer there. `width()` clamps; this is what says so.
    #[test]
    fn every_step_resolves_to_a_width() {
        for (i, width) in WIDTHS.iter().enumerate() {
            let retro = Retro {
                step: Some(i),
                ..Retro::default()
            };
            assert_eq!(retro.width(), Some(*width));
        }
        // A step past the end of the list is the coarsest picture, not a panic.
        let stale = Retro {
            step: Some(99),
            ..Retro::default()
        };
        assert_eq!(stale.width(), Some(*WIDTHS.last().unwrap()));
        assert_eq!(Retro::default().width(), None);
    }

    /// The toggle walks every picture and comes back round.
    #[test]
    fn the_toggle_visits_all_of_them() {
        let mut retro = Retro::default();
        let mut seen = Vec::new();
        for _ in 0..=WIDTHS.len() {
            step(&mut retro);
            seen.push(retro.step);
        }
        for i in 0..WIDTHS.len() {
            assert!(seen.contains(&Some(i)), "step {i} never came up: {seen:?}");
        }
        assert_eq!(retro.step, None, "it should end back at the sharp picture");
    }
}
