//! The settings a person wants to reach without reading the source.
//!
//! Two kinds of thing live here and they are not the same kind, which is why the panel
//! separates them with a line rather than a heading.
//!
//! **Sound** is a preference. There is no right master volume; there is the one that
//! suits the room, and until now the only control over it was a mute key. A number that
//! belongs to the listener rather than to the boat is the one number in the audio path
//! that is nobody's measurement, and it is reasonable to put it where it can be reached.
//!
//! **Picture** is a preference too, and the three controls here were keys and nothing
//! else — which breaks the console's own rule that everything the keyboard can do the
//! mouse can do. A look you have to know a letter for is a look most people never find.
//!
//! **Weather** is not a preference, and the panel says so out loud. The whole argument
//! of this simulator is that you are handed conditions and have to deal with them — the
//! forecast is drawn before you cast off, the reef goes in because of what it says, and
//! the skill is in the dealing. A dial that turns the wind down is therefore a *practice
//! aid*, not a setting, in the same way a wave pool is not the sea. It is here because
//! learning to gybe in thirty knots by being repeatedly knocked down in thirty knots is
//! not learning, and because wanting to look at a squall from close to is a legitimate
//! thing to want. It is labelled for what it is and it leaves the drawn day alone until
//! it is touched.

use bevy::prelude::*;
use bevy_egui::egui;

use crate::sim::Sim;
use crate::sound::SoundHandle;

/// The settings panel.
///
/// What this module does in a frame, in one place — see `stages.rs` for why the
/// schedule is named sets rather than one long chain.
pub fn plugin(app: &mut App) {
    use crate::stages::{Deck, Paint};
    app.init_resource::<Settings>();
    app.add_systems(
        Update,
        (toggle, apply, hold_everything).in_set(Deck::Weather),
    );
    app.add_systems(bevy_egui::EguiPrimaryContextPass, draw.in_set(Paint::Cards));
}

/// Which page of the panel is showing.
///
/// Tabs rather than one long column, and the reason is the fourth one: the network page
/// is not a row of sliders, it is a list that changes while you look at it, with buttons
/// that do something irreversible-ish. Underneath three sections of preferences it would
/// be both hard to find and easy to hit by accident. Four short pages also means the
/// panel stops growing downwards every time the game learns something new.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Sound,
    Picture,
    Weather,
    /// The consignment: what she is carrying and how to be rid of it.
    Passage,
    Network,
    /// Practice: the man-overboard drill, which is the one manoeuvre worth a page of its
    /// own because it is the one nobody has practised.
    Drill,
}

#[derive(Resource)]
pub struct Settings {
    pub open: bool,
    pub tab: Tab,
    /// What the listener asked for, which is remembered across a mute.
    pub volume: f32,
    /// And the music separately, because somebody will want the sea without it.
    pub music: f32,
    /// The wind, the water and the rain — what the game sounds like.
    pub sea: f32,
    /// Her rigging, her gear and her sails making a noise.
    pub boat: f32,
    /// Whether the weather has been touched. Until it is, the panel shows the day as
    /// drawn and says so; after it, it stops pretending the forecast still applies.
    pub weather_touched: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            open: false,
            tab: Tab::Sound,
            // Where these sit after an evening of listening, and the sea is much lower
            // than anybody would guess from a mixing desk.
            //
            // It is a continuous noise, and a continuous noise at the level of the things
            // that *happen* is not atmosphere, it is loudness — it masks the sounds you
            // are supposed to be sailing by. Pushed right down it stops being something
            // you hear and becomes something you are in, which is what the sea is, and
            // the gear and the music come up over it where they can be made out.
            volume: 0.70,
            music: 1.00,
            // **Both of them down again, against the band.** Four decibels off the gear
            // and three off the sea, which is a step you hear rather than one you have
            // to be told about — and the music is left where it was, so what changes is
            // the balance and not the loudness of the game.
            //
            // The reasoning under this has not changed, it has only been taken further:
            // a continuous noise at the level of the things that *happen* is not
            // atmosphere, it is loudness, and it masks the sounds you are supposed to be
            // sailing by. The sea is something you are in; the gear is something that
            // speaks when it has something to say.
            sea: 0.055,
            boat: 0.42,
            weather_touched: false,
        }
    }
}

/// Put the mix where the settings say, every frame.
///
/// Its own system because the panel is the wrong place for it: `draw` returns early when
/// the panel is shut, so every level set here only reached the audio thread *once
/// somebody had opened the settings*. Until then the mix ran on whatever the parameter
/// block happened to hold, which is a second set of defaults in a second file — the same
/// fault as a reader guessing at a sentinel, one layer up.
///
/// Four numbers a frame costs nothing and there is now one place where the settings are
/// the settings.
pub fn apply(settings: Res<Settings>, sound: Res<SoundHandle>) {
    sound.0.set_master(settings.volume);
    sound.0.set_sea(settings.sea);
    sound.0.set_boat(settings.boat);
    sound.0.set_music(settings.music);
}

/// Escape, which is the one key everybody tries first — and the only system allowed to
/// have it.
///
/// It was two systems: this one toggled the settings and the help panel closed itself.
/// Both fired on the same press, so Escape with the help up closed the help *and opened
/// the settings* — which from the deck is "sometimes Escape does not close the menu",
/// because what you saw was a menu appearing as the other one went.
///
/// One key, one owner, one rule: Escape closes the topmost thing that is open, and if
/// nothing is open it opens the settings. The chart is not in that stack because it is
/// an instrument rather than a modal — you sail with it up, and a key that put it away
/// every time you reached for Escape would be a nuisance.
pub fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mut settings: ResMut<Settings>,
    mut help: ResMut<crate::help::Help>,
    net: Res<crate::net::Net>,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    if help.open {
        help.open = false;
    } else {
        settings.open = !settings.open;
        // On the way out, remember where the game was. Here rather than on every
        // keystroke in the box: closing the panel is the moment somebody has finished
        // changing their mind, and a file written sixty times a second while a person
        // types an address is a file waiting to be found half-written.
        if !settings.open && !net.borrowed {
            sail_net::remembered::write(&net.name, &net.address, &net.word);
        }
    }
}

// A system's parameters are its dependency list rather than an argument list, and this
// panel reaches four more resources than it did now that the picture is settable from
// somewhere other than a keypress.
#[allow(clippy::too_many_arguments)]
/// **The sea stops while the panel is open.**
///
/// There was no pause at all: the one place a player could step away was this panel, and
/// the boat sailed on behind it — which on this coast means she sails on into something.
/// A menu that stops the game is the oldest convention there is and it costs no key,
/// which matters because the key list is for things you do *to the boat* and pausing is
/// not one of them.
///
/// Bevy's virtual clock does it, so everything that reads a delta stops together: the
/// simulation, the sky, the sound, the sea's own animation. Stopping only the simulation
/// would leave a boat frozen under a sky that was still moving, which is worse than not
/// stopping at all.
///
/// **Except in company.** Two to eight boats are each computed by the person sailing
/// them, so a local pause cannot stop anybody else's sea — it would stop only yours while
/// the fleet sailed on. With a link up the water keeps moving and the panel says so.
pub fn hold_everything(
    settings: Res<Settings>,
    net: Res<crate::net::Net>,
    mut clock: ResMut<Time<Virtual>>,
) {
    let hold = settings.open && net.link.is_none();
    if hold && !clock.is_paused() {
        clock.pause();
        info!("the sea is stopped");
    } else if !hold && clock.is_paused() {
        clock.unpause();
        info!("and on again");
    }
}

/// Everything the PICTURE page can touch, behind one name.
///
/// Six resources that are all answers to the same question — *what does the game look
/// like* — and a system may have sixteen parameters. The same move as `Missions` and
/// `InHand` below, and for the same two reasons: the limit, and the fact that these
/// genuinely belong together.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Picture<'w> {
    pub view: ResMut<'w, crate::camera::View>,
    pub ink: ResMut<'w, crate::world::Ink>,
    pub retro: ResMut<'w, crate::retro::Retro>,
    pub hud: ResMut<'w, crate::hud::HudVisible>,
    pub counter: ResMut<'w, crate::world::Counter>,
    pub screen: ResMut<'w, crate::screen::Screen>,
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut contexts: bevy_egui::EguiContexts,
    mut settings: ResMut<Settings>,
    mut sim: ResMut<Sim>,
    sky: Res<crate::sky::Sky>,
    mut clock: ResMut<crate::sky::Clock>,
    sound: Res<SoundHandle>,
    mut picture: Picture,
    mut net: ResMut<crate::net::Net>,
    fleet: Res<crate::fleet::Fleet>,
    mut missions: Missions,
    mut course: ResMut<crate::course::Course>,
    mut home: ResMut<crate::anchorage::Home>,
    weather: Res<crate::weather::Weather>,
) -> Result {
    let (view, ink, retro, hud, counter, screen) = (
        &mut picture.view,
        &mut picture.ink,
        &mut picture.retro,
        &mut picture.hud,
        &mut picture.counter,
        &mut picture.screen,
    );
    if !settings.open {
        net.scan = None;
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let frame = egui::Frame::new()
        .fill(egui::Color32::from_rgb(16, 20, 26))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 82, 96)))
        .inner_margin(egui::Margin::symmetric(14, 12));

    let mut close = false;
    let mut another_sea = false;
    egui::Area::new("settings".into())
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, -20.0))
        .show(ctx, |ui| {
            frame.show(ui, |ui| {
                ui.set_width(300.0);
                tabs(ui, &mut settings.tab);
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                if settings.tab == Tab::Sound {
                    heading(ui, "SOUND");
                    let mut volume = settings.volume;
                    if ui
                        .add(
                            egui::Slider::new(&mut volume, 0.0..=1.0)
                                .show_value(false)
                                .text("volume"),
                        )
                        .changed()
                    {
                        settings.volume = volume;
                    }
                    // Separate from the master, because the sea and the music are two
                    // different things to want. Somebody who sails with the wind in their
                    // ears and nothing else is not turning the game down, they are turning
                    // one instrument out of the mix — and that is a reasonable thing to do
                    // with a soundtrack that is generated rather than composed for them.
                    // Copied out and back rather than borrowed twice, because the settings
                    // resource is one borrow and these are two sliders in it.
                    let (mut sea, mut boat) = (settings.sea, settings.boat);
                    ui.add(
                        egui::Slider::new(&mut sea, 0.0..=1.0)
                            .show_value(false)
                            .text("sea"),
                    );
                    ui.add(
                        egui::Slider::new(&mut boat, 0.0..=1.0)
                            .show_value(false)
                            .text("boat"),
                    );
                    settings.sea = sea;
                    settings.boat = boat;
                    let mut music = settings.music;
                    if ui
                        .add(
                            egui::Slider::new(&mut music, 0.0..=1.0)
                                .show_value(false)
                                .text("music"),
                        )
                        .changed()
                    {
                        settings.music = music;
                    }
                    let muted = sound.0.muted();
                    if ui
                        .button(if muted { "UNMUTE  (K)" } else { "MUTE  (K)" })
                        .clicked()
                    {
                        sound.0.set_muted(!muted);
                    }
                }

                if settings.tab == Tab::Picture {
                    heading(ui, "PICTURE");
                    // Where you are sitting. Two places on a small boat: at the helm, or
                    // astern of her looking back, which is the one that shows you what the
                    // sails are doing.
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("view")
                                .color(egui::Color32::from_rgb(150, 150, 130))
                                .monospace()
                                .size(11.0),
                        );
                        for (name, which) in [
                            ("at the helm", crate::camera::View::Cockpit),
                            ("from astern", crate::camera::View::Chase),
                        ] {
                            if ui.selectable_label(**view == which, name).clicked() {
                                **view = which;
                            }
                        }
                    });
                    // The drawn sea. Not a quality setting: a different picture, and the one
                    // this boat was always headed for.
                    if ui.selectable_label(ink.0, "the ink drawing").clicked() {
                        ink.0 = !ink.0;
                    }
                    pixels(ui, retro);
                    // Where the picture is drawn. `F11` is the same switch without
                    // opening this, and both of them end up in the same resource —
                    // see `screen.rs`, which also explains why the window can be
                    // dragged about at all now.
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("window")
                                .color(egui::Color32::from_rgb(150, 150, 130))
                                .monospace()
                                .size(11.0),
                        );
                        for which in [crate::screen::Screen::Windowed, crate::screen::Screen::Full]
                        {
                            if ui
                                .selectable_label(**screen == which, which.name())
                                .on_hover_text("F11, or Alt Enter")
                                .clicked()
                            {
                                **screen = which;
                            }
                        }
                    });
                    if ui.selectable_label(hud.0, "the diagnostic panel").clicked() {
                        hud.0 = !hud.0;
                    }
                    // The counter, which is the one number here that is about the
                    // machine rather than the boat — and is off until somebody wants it.
                    if ui
                        .selectable_label(counter.0, "the frame counter")
                        .on_hover_text(
                            "frames a second, and the worst frame of the last two — \
                             which is the one you feel",
                        )
                        .clicked()
                    {
                        counter.0 = !counter.0;
                    }
                }

                if settings.tab == Tab::Weather {
                    heading(ui, "WEATHER");
                    ui.label(
                        egui::RichText::new(if settings.weather_touched {
                            "set by hand — this is practice, not the day you were given"
                        } else {
                            "as drawn for this passage"
                        })
                        .color(egui::Color32::from_rgb(150, 150, 130))
                        .monospace()
                        .size(10.0),
                    );
                    ui.add_space(4.0);

                    // The hour is the app's own, so it is always adjustable: the sun moves
                    // whatever is driving the wind.
                    let mut hour = sky.hour;
                    if ui
                        // The whole clock, not the daylight half of it: night is a
                        // thing you can sail now, and a slider that stops at eight in
                        // the evening is a slider that says it is not.
                        .add(egui::Slider::new(&mut hour, 0.0..=23.9).text("hour"))
                        .changed()
                    {
                        // Through the clock's offset: the hour itself is read off the
                        // passage, which is shared with everybody else in the game and
                        // is not this machine's to move.
                        clock.put_at(hour, sky.hour);
                        settings.weather_touched = true;
                    }

                    // The sea. One dial and only one, because wave height is a *consequence*
                    // — of the wind, the fetch and how long it has blown — and a control that
                    // set it directly could make three metres at six knots or flat water at
                    // thirty, either of which a sailor spots instantly. Fetch is the honest
                    // one: it is a real property of where you are, and the height follows
                    // from the same relations the waves themselves use.
                    let tws = sim.sim.telemetry.tws;
                    let t = sim.sim.state.time;
                    if let Some(sea) = sim.sim.water.adjustable() {
                        let mut km = (sea.fetch() / 1000.0) as f32;
                        if ui
                            .add(
                                egui::Slider::new(&mut km, 3.0..=320.0)
                                    .logarithmic(true)
                                    .text("fetch, km"),
                            )
                            .changed()
                        {
                            sea.set_fetch(km as f64 * 1000.0);
                            settings.weather_touched = true;
                        }
                        let (hs, tp) = sail_core::water::fetch_limited_sea(tws, sea.fetch());
                        ui.horizontal(|ui| {
                            ui.label(
                                // Named as well as measured. Douglas is the *sea* scale and
                                // it is not Beaufort — the same force over twenty miles of
                                // fetch and over two hundred is the same wind and quite a
                                // different afternoon, and naming the sea separately is the
                                // only honest way to say which one you have asked for.
                                egui::RichText::new(format!(
                                    "building towards {} — {hs:.1} m at {tp:.1} s",
                                    crate::weather::sea_state(hs)
                                ))
                                .color(egui::Color32::from_rgb(150, 150, 130))
                                .monospace()
                                .size(10.0),
                            );
                            // Because the real thing takes two and a half hours to build and
                            // five to lie down, which is right and is exactly what you do not
                            // want when you have just moved a slider to look at something.
                            if ui.button("SETTLE NOW").clicked() {
                                sea.settle(tws, t);
                                settings.weather_touched = true;
                            }
                        });
                    }

                    // And the wind, if whatever is behind the field has dials on it. A real
                    // forecast would not, and would say so here rather than pretending.
                    let Some(wind) = sim.sim.wind.adjustable() else {
                        ui.label(
                            egui::RichText::new(
                                "the wind is coming from a forecast and cannot be set",
                            )
                            .color(egui::Color32::from_rgb(150, 150, 130))
                            .monospace()
                            .size(10.0),
                        );
                        footer(ui, &mut close);
                        return;
                    };
                    // In Beaufort, because that is what a sailor thinks in: nobody says
                    // "seventeen knots", they say "a fresh breeze" or "a five". The knots are
                    // printed beside it because the instruments are in knots and the two have
                    // to be connectable, and the slider lands on the *middle* of a force
                    // rather than its edge — a dial that gives you 21.9 knots when you asked
                    // for a six is a dial you will say is broken.
                    let mut force =
                        crate::weather::force(sail_core::units::ms_to_kn(wind.synoptic.base_speed));
                    if ui
                        .add(egui::Slider::new(&mut force, 1..=9).text("wind, force"))
                        .changed()
                    {
                        // `blows` rather than the base on its own: the base is the day the
                        // trends wander about, and setting it alone leaves the wind where
                        // it was for the next half hour. A slider moved to force 8 has to
                        // give force 8.
                        wind.synoptic.blows(sail_core::units::kn_to_ms(
                            crate::weather::force_wind(force),
                        ));
                        settings.weather_touched = true;
                    }
                    // And the line under it reads the wind rather than the day, so that
                    // when the two have drifted apart again it says which one you are in.
                    let knots = sail_core::units::ms_to_kn(wind.synoptic.speed_now());
                    ui.label(
                        egui::RichText::new(format!(
                            "{} — {:.0} kn",
                            crate::weather::beaufort(knots),
                            knots
                        ))
                        .color(egui::Color32::from_rgb(150, 150, 130))
                        .monospace()
                        .size(10.0),
                    );
                    let mut oktas = (wind.synoptic.cover() * 8.0).round() as u8;
                    if ui
                        .add(egui::Slider::new(&mut oktas, 0..=8).text("cloud, oktas"))
                        .changed()
                    {
                        wind.synoptic.start_cover(oktas as f64 / 8.0);
                        settings.weather_touched = true;
                    }
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("squalls")
                                .color(egui::Color32::from_rgb(150, 150, 130))
                                .monospace()
                                .size(11.0),
                        );
                        for (name, interval) in [
                            ("none", f64::INFINITY),
                            ("odd one", 2_400.0),
                            ("squally", 900.0),
                            ("constant", 180.0),
                        ] {
                            let on = (wind.squall_interval - interval).abs() < 1.0
                                || (wind.squall_interval.is_infinite() && interval.is_infinite());
                            if ui.selectable_label(on, name).clicked() {
                                wind.squall_interval = interval;
                                // So the change is felt rather than waited for.
                                wind.next_squall = wind.next_squall.min(interval * 0.25);
                                settings.weather_touched = true;
                            }
                        }
                    });
                }

                if settings.tab == Tab::Passage {
                    passage(ui, &mut course);
                    a_new_sea(ui, &mut another_sea, &home.stores);
                }
                if settings.tab == Tab::Drill {
                    drill(ui, &mut missions);
                }
                if settings.tab == Tab::Network {
                    together(ui, &mut net, &fleet, sim.sim.state.pos);
                } else {
                    // Stop looking the moment the page is not showing. The comment on
                    // `scan` said it only ran while the page was open and that was not
                    // true of the first version: nothing ever dropped it, so one visit to
                    // the page left the game broadcasting at everybody's switch once a
                    // second for the rest of the passage.
                    //
                    // And this is a separate `if` rather than the tail of a chain for the
                    // same reason, learnt twice: hanging a third page off the front of it
                    // put the clear behind *that* page's arm, so opening TOGETHER and then
                    // stepping to DRILL left the scan running. Found in review.
                    net.scan = None;
                }
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(if net.link.is_some() {
                        "the fleet is sailing — this does not stop the sea"
                    } else {
                        "the sea is stopped while this is open"
                    })
                    .monospace()
                    .size(10.0)
                    .color(egui::Color32::from_rgb(130, 136, 146)),
                );
                footer(ui, &mut close);
            });
        });
    if close {
        settings.open = false;
    }
    if another_sea {
        // The same handover joining a game uses, pointed at a seed nobody has sailed:
        // `net::rebuild` throws the archipelago away and builds the new one through the
        // systems that built the first, so the solo world and the joined one cannot
        // drift apart. The platform's berth goes with the old islands and is picked
        // again when she opens in the new ones; the store is untouched.
        let drawn = crate::knob::seed() ^ 0x9E37_79B9;
        home.sea = drawn;
        home.moored = None;
        home.harbour.clear();
        home.unsaved = true;
        net.rebuild = Some(sail_net::World {
            seed: drawn,
            tws: weather.tws as f32,
            wind_from: weather.wind_from as f32,
            fetch: weather.fetch as f32,
            start_hour: weather.start_hour as f32,
            cover: weather.cover as f32,
        });
        settings.open = false;
        info!("a new sea: seed {drawn}");
    }
    Ok(())
}

/// The drill page: start it, stop it, and what it is for.
///
/// The passage: what she is carrying, and the one way out of it.
///
/// **A page rather than a key.** Asking for a new consignment used to be `L`, which put
/// a thing that throws away the passage you are on next to the keys that trim the boat —
/// and `L` is a neighbour of nothing in particular, which is exactly the problem: a key
/// with no reason to be that key is a key somebody presses by mistake. Taken off the
/// keyboard from the deck. It is here and on the console button, both of which take a
/// deliberate hand.
fn passage(ui: &mut egui::Ui, course: &mut crate::course::Course) {
    use crate::course::Leg;
    heading(ui, "THE PASSAGE");
    ui.label(
        egui::RichText::new(match course.leg {
            Leg::Carrying => "carrying a consignment",
            Leg::Idle => "nothing aboard — the yard has work whenever you want it",
        })
        .color(egui::Color32::from_rgb(176, 182, 190))
        .monospace()
        .size(11.0),
    );
    ui.add_space(8.0);
    if ui
        .button(
            egui::RichText::new(match course.leg {
                Leg::Carrying => "GIVE UP THIS ONE, TAKE ANOTHER",
                Leg::Idle => "TAKE A JOB",
            })
            .monospace()
            .size(12.0),
        )
        .on_hover_text("the yard will have another the moment you put this one down")
        .clicked()
    {
        course.wants_new = true;
    }
}

/// **A new sea**, which until now could only be asked for with an environment variable.
///
/// The world is a pure function of its seed and the seed is in the save, so a player who
/// does not like the archipelago they were dealt had no way to be dealt another — short
/// of finding the file and deleting it. This is that, with the two things it has to say
/// out loud: the islands and the names go, and **the platform and everything on it come
/// with you**, because the store is yours and not the sea's.
fn a_new_sea(ui: &mut egui::Ui, wants: &mut bool, stores: &crate::anchorage::Stores) {
    ui.add_space(10.0);
    ui.label(
        egui::RichText::new("A NEW SEA")
            .monospace()
            .size(11.0)
            .color(egui::Color32::from_rgb(150, 150, 130)),
    );
    ui.label(
        egui::RichText::new(format!(
            "another archipelago, drawn fresh. {} casks and {} fish come with you — the \
             platform is moored again at the nearest town to wherever she opens.",
            stores.casks, stores.fish
        ))
        .color(egui::Color32::from_rgb(176, 182, 190))
        .monospace()
        .size(10.0),
    );
    ui.add_space(4.0);
    if ui
        .button(
            egui::RichText::new("DRAW ANOTHER ARCHIPELAGO")
                .monospace()
                .size(12.0),
        )
        .on_hover_text("this water, these islands and these names are gone for good")
        .clicked()
    {
        *wants = true;
    }
}

/// **A race, which is the one thing sailing has that nothing else does.**
///
/// Not because winning matters — there is nobody to beat yet — but because a start is a
/// manoeuvre and a good one is the hardest thing in this game that does not involve
/// somebody going over the side. You cannot stop a sailing boat: the five minutes before
/// the gun are spent making her arrive at a line she must not touch early, at speed, at
/// the end you have decided is favoured.
fn regatta(ui: &mut egui::Ui, race: &mut crate::race::Race) {
    heading(ui, "RACE — FIVE MINUTES AND A WINDWARD-LEEWARD");
    ui.label(
        egui::RichText::new(
            "A line laid across the wind two cables to leeward of you, a gun in five \
             minutes, and twice round a buoy eight hundred metres upwind.\n\nOver the \
             line early and you come back and cross again, which is the rule that makes \
             a start a start. The committee end or the pin: the panel says which is \
             favoured and by how many metres, and ten seconds of that is the whole first \
             beat.",
        )
        .color(egui::Color32::from_rgb(176, 182, 190))
        .monospace()
        .size(11.0),
    );
    ui.add_space(8.0);
    if race.running() {
        if let Some(took) = race.time() {
            ui.label(
                egui::RichText::new(format!(
                    "finished in {:.0}:{:02.0}",
                    took / 60.0,
                    took % 60.0
                ))
                .color(egui::Color32::from_rgb(230, 198, 120))
                .monospace()
                .size(11.0),
            );
            ui.add_space(6.0);
        }
        if ui
            .button(egui::RichText::new("ABANDON").monospace().size(12.0))
            .clicked()
        {
            race.wants_stop = true;
        }
    } else if ui
        .button(
            egui::RichText::new("START THE SEQUENCE — FIVE MINUTES")
                .monospace()
                .size(12.0),
        )
        .clicked()
    {
        race.wants_start = true;
    }
}

/// **The job, offered the way a job is offered**: here is a ship, here is what is wrong,
/// and here is what it will take. Not a difficulty setting and not a list of objectives —
/// the whole of it fits in four lines because the whole of it is seamanship you already
/// have the instruments for.
fn pilotage(
    ui: &mut egui::Ui,
    wants: &mut crate::mission::WantsPilotage,
    job: &crate::mission::Pilotage,
) {
    heading(ui, "PILOTAGE — A BRIG OFF THE HEADLAND");
    ui.label(
        egui::RichText::new(
            "A loaded brig stands in from seaward wanting a pilot for the inner \
             passage. She draws four metres where you draw two, she will not lie closer \
             than sixty-five degrees to the wind, and she cannot stop.\n\nCatch her, get \
             ahead of her, and go aboard with Enter. Then she follows your wake, and \
             every sounding you take is for her. Her deck cargo has been working loose \
             since the gale — the casks in the water are hers.",
        )
        .color(egui::Color32::from_rgb(176, 182, 190))
        .monospace()
        .size(11.0),
    );
    ui.add_space(8.0);
    if job.running {
        if let Some(over) = &job.over {
            ui.label(
                egui::RichText::new(over.words())
                    .color(egui::Color32::from_rgb(230, 198, 120))
                    .monospace()
                    .size(11.0),
            );
            ui.add_space(6.0);
        }
        if ui
            .button(egui::RichText::new("LET HER GO").monospace().size(12.0))
            .clicked()
        {
            wants.stop = true;
        }
    } else if ui
        .button(
            egui::RichText::new("TAKE THE JOB — SHE IS STANDING IN")
                .monospace()
                .size(12.0),
        )
        .clicked()
    {
        wants.start = true;
    }
}

/// A page rather than a key, because starting a man-overboard drill by accident in the
/// middle of a delivery would be a poor joke, and because this is where somebody who has
/// just opened the game looks for the thing that teaches them to sail it.
fn drill(ui: &mut egui::Ui, missions: &mut Missions) {
    sailing_101(ui, missions);
    ui.add_space(14.0);
    ui.separator();
    ui.add_space(10.0);
    regatta(ui, &mut missions.race);
    ui.add_space(14.0);
    ui.separator();
    ui.add_space(10.0);
    pilotage(ui, &mut missions.wants, &missions.job);
    ui.add_space(14.0);
    ui.separator();
    ui.add_space(10.0);
    heading(ui, "MAN OVERBOARD DRILL");
    let lesson = &mut missions.drill;
    ui.label(
        egui::RichText::new(
            "Getting her sailing is three keys. Getting her back to one spot and stopped \
             there is the whole of boat handling, and nobody practises it until the \
             afternoon they need it.\n\nThe drill talks you through making her go, and \
             then the dan buoy goes over the side.",
        )
        .color(egui::Color32::from_rgb(176, 182, 190))
        .monospace()
        .size(11.0),
    );
    ui.add_space(8.0);
    if lesson.running() {
        if ui
            .button(egui::RichText::new("STOP THE DRILL").monospace().size(12.0))
            .clicked()
        {
            lesson.wants_stop = true;
        }
    } else if ui
        .button(
            egui::RichText::new("START — PUT HER ON A REACH")
                .monospace()
                .size(12.0),
        )
        .clicked()
    {
        lesson.wants_start = true;
    }
}

/// **SAILING 101**: the walk round the boat, for somebody who has just arrived.
///
/// First on the page because it is the first thing a new reader needs and the last
/// thing they would think to look for — the key list is behind `H`, and a list of
/// forty-four keys is a reference rather than an introduction.
fn sailing_101(ui: &mut egui::Ui, missions: &mut Missions) {
    heading(ui, "SAILING 101");
    ui.label(
        egui::RichText::new(
            "Fifteen steps round the deck: the tiller, the sheets, the rope bank, the \
             dump, the preventer, the pilot, the watch, the chart, the glasses, the \
             hand bearing compass, the log and the rod. One instruction at a time, each \
             about one control and the instrument that answers it — and nothing moves \
             on until you have done it.\n\nIt stands the watch down and takes the \
             pilot off first, because a lesson about the mainsheet is nonsense while \
             somebody else is trimming it.",
        )
        .color(egui::Color32::from_rgb(176, 182, 190))
        .monospace()
        .size(11.0),
    );
    ui.add_space(8.0);
    if missions.basics.running() {
        if ui
            .button(
                egui::RichText::new("STOP THE LESSON")
                    .monospace()
                    .size(12.0),
            )
            .clicked()
        {
            missions.basics.wants_stop = true;
        }
    } else {
        let busy = missions.drill.running();
        let button = ui.add_enabled(
            !busy,
            egui::Button::new(
                egui::RichText::new("START — WALK ME ROUND HER")
                    .monospace()
                    .size(12.0),
            ),
        );
        if busy {
            button.on_hover_text("the drill is running: one card at a time");
        } else if button.clicked() {
            missions.basics.wants_start = true;
        }
    }
}

/// What the missions page works on.
///
/// Four resources behind one name, because a system may have sixteen parameters and
/// this panel wanted seventeen — and because *these four are the page*: the lesson, the
/// drill, the regatta and the pilot job.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Missions<'w> {
    pub basics: ResMut<'w, crate::basics::Basics>,
    pub drill: ResMut<'w, crate::tutorial::Tutorial>,
    pub wants: ResMut<'w, crate::mission::WantsPilotage>,
    pub job: Res<'w, crate::mission::Pilotage>,
    pub race: ResMut<'w, crate::race::Race>,
}

/// How many pixels across the world is drawn, as a row of choices.
///
/// Its own function for the reason `tabs` is: it can then be read without a window, and
/// what this row needs checking for is exactly what a person reads off it — which widths
/// are on offer and whether the one in use is the one lit up. The widths come from
/// `retro` rather than being listed again here, because two lists of the same thing is
/// how a settings panel comes to offer a size the renderer does not have.
pub fn pixels(ui: &mut egui::Ui, retro: &mut crate::retro::Retro) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("pixels")
                .color(egui::Color32::from_rgb(150, 150, 130))
                .monospace()
                .size(11.0),
        );
        // Named by their number. "drawn" alone was honest while there was one of them;
        // with two, a name that does not say which is which makes the player click both
        // to find out.
        for (i, w) in crate::retro::Retro::widths().iter().enumerate() {
            let step = Some(i);
            if ui
                .selectable_label(retro.step == step, format!("{w}"))
                .on_hover_text(if i == 0 {
                    "the drawn picture — pixels you can count"
                } else {
                    "the same picture, finer: for a large screen, where the coarse one \
                     reads as low resolution rather than as drawn"
                })
                .clicked()
            {
                retro.step = step;
            }
        }
        if ui
            .selectable_label(retro.step.is_none(), "sharp")
            .on_hover_text("no pixel grid at all — what is actually on the deck")
            .clicked()
        {
            retro.step = None;
        }
    });
}

/// The pages, as a row of labels.
///
/// Its own function so that it can be looked at without a window. Everything else in
/// this panel is a slider over a resource and would need half the game to test; this is
/// the one piece whose correctness is "the page is there and you can get to it", which is
/// exactly the thing that was reported missing and exactly the thing a test can hold.
pub fn tabs(ui: &mut egui::Ui, current: &mut Tab) {
    ui.horizontal(|ui| {
        for (tab, name) in PAGES {
            let on = *current == tab;
            if ui
                .selectable_label(
                    on,
                    egui::RichText::new(name)
                        .monospace()
                        .size(11.0)
                        .color(if on {
                            egui::Color32::from_rgb(230, 198, 120)
                        } else {
                            egui::Color32::from_rgb(150, 150, 130)
                        }),
                )
                .clicked()
            {
                *current = tab;
            }
        }
    });
}

/// In the order they are read, which is the order they were added in and also the order
/// of how often anybody touches them.
pub const PAGES: [(Tab, &str); 6] = [
    (Tab::Sound, "SOUND"),
    (Tab::Picture, "PICTURE"),
    (Tab::Weather, "WEATHER"),
    (Tab::Passage, "PASSAGE"),
    (Tab::Network, "TOGETHER"),
    (Tab::Drill, "MISSIONS"),
];

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .color(egui::Color32::from_rgb(200, 190, 150))
            .monospace()
            .size(12.0),
    );
    ui.add_space(4.0);
}

fn footer(ui: &mut egui::Ui, close: &mut bool) {
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if ui.button("CLOSE  (Esc)").clicked() {
            *close = true;
        }
    });
}

/// The fourth page: who else is out there.
///
/// Written as one function rather than inline with the sliders because it is the only
/// part of this panel that is about *the world* rather than about preferences, and
/// because it is the one page that changes while you are looking at it.
///
/// The address box is still here under the list, and it is not a fallback nobody needs:
/// the list only finds games on the same network, and the whole point of the internet
/// milestone is a game that is not. An address from a Tailscale tailnet goes in there and
/// works exactly the same.
fn together(
    ui: &mut egui::Ui,
    net: &mut crate::net::Net,
    fleet: &crate::fleet::Fleet,
    here: glam::DVec2,
) {
    heading(ui, "SAILING TOGETHER");

    // Whatever the knock has turned up, taken out of the thread and kept. The probe is
    // dropped with it: a question that has been answered is not a question any more.
    if let Some(answer) = net.probe.as_ref().and_then(|p| p.answer()) {
        net.knocked = Some(answer);
        net.probe = None;
    }

    match (&net.link, net.joined) {
        (Some(_), true) => {
            let n = fleet.boats.len();
            ui.label(
                egui::RichText::new(match n {
                    0 => "in — nobody else on the water yet".to_string(),
                    1 => "in — one other boat".to_string(),
                    n => format!("in — {n} other boats"),
                })
                .color(egui::Color32::from_rgb(150, 200, 150))
                .monospace()
                .size(11.0),
            );
            for boat in &fleet.boats {
                // With how far off she is, because in a list of names the only thing a
                // sailor actually wants to know is whether they can see her. Under a mile
                // on a clear day you can; over two you are looking at empty water.
                //
                // The comment here said exactly this while the code printed the name
                // alone, which is the defect this project keeps producing: a sentence
                // describing the intention rather than the program.
                let away = (glam::DVec2::new(boat.drawn.pos[0] as f64, boat.drawn.pos[1] as f64)
                    - here)
                    .length()
                    / 1852.0;
                ui.label(
                    egui::RichText::new(format!("   {}   {away:.2} nm", boat.name))
                        .color(egui::Color32::from_rgb(186, 192, 198))
                        .monospace()
                        .size(11.0),
                );
            }
            ui.add_space(6.0);
            if ui.button("LEAVE").clicked() {
                // Dropping the link is how you leave: the thread sees the channel go and
                // says goodbye on its way out, so the others lose the boat at once rather
                // than waiting six seconds for her to go quiet.
                net.link = None;
                net.joined = false;
            }
        }
        (Some(_), false) => {
            ui.label(
                egui::RichText::new("knocking...")
                    .color(egui::Color32::from_rgb(200, 190, 150))
                    .monospace()
                    .size(11.0),
            );
            if ui.button("STOP").clicked() {
                net.link = None;
            }
        }
        (None, _) => {
            // Looking only while this page is open. A broadcast once a second is nothing
            // on a network and it is still rude to shout at everybody's switch for the
            // whole of a four-hour passage.
            if net.scan.is_none() {
                match sail_net::discovery::begin() {
                    Ok(scan) => net.scan = Some(scan),
                    Err(e) => net.trouble = e.to_string(),
                }
            }
            let games = net.scan.as_ref().map(|s| s.games()).unwrap_or_default();
            if games.is_empty() {
                ui.label(
                    egui::RichText::new("no games on this network")
                        .color(egui::Color32::from_rgb(150, 150, 130))
                        .monospace()
                        .size(11.0),
                );
            }
            for game in games {
                // Marked as a thing to press. A row of text that happens to be clickable
                // is a row of text as far as anybody is concerned, and this one was read
                // as a label with the join button living underneath it — which is how
                // somebody ends up pressing JOIN with nothing typed in the box.
                let label = format!("  ▸  {}   {} aboard", game.game, game.players);
                if ui
                    .selectable_label(
                        false,
                        egui::RichText::new(label)
                            .monospace()
                            .size(11.0)
                            .color(egui::Color32::from_rgb(230, 198, 120)),
                    )
                    .on_hover_text("join this game")
                    .clicked()
                {
                    net.address = game.at.clone();
                    join(net);
                }
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("or")
                        .color(egui::Color32::from_rgb(150, 150, 130))
                        .monospace()
                        .size(11.0),
                );
                // A knock describes the address it was asked of. Change the address and
                // the answer on the screen is about somewhere else — which is a small
                // lie and the kind that costs an evening, because it is the one piece of
                // evidence a person has that they typed the right thing.
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut net.address)
                            .desired_width(150.0)
                            .hint_text("address:7714")
                            .font(egui::TextStyle::Monospace),
                    )
                    .changed()
                {
                    net.knocked = None;
                    // And typing in the box makes it yours, whatever put it there: an
                    // address edited by hand is a decision and gets remembered.
                    net.borrowed = false;
                }
                // Nothing to join when the box is empty, so the button says so by being
                // grey rather than by failing. Pressed empty it used to build ":7714",
                // hand that to the resolver and put "nodename nor servname provided" on
                // the screen — an operating system's words for "you typed nothing",
                // which is a poor way for a game to say it.
                let typed = !net.address.trim().is_empty();
                if ui
                    .add_enabled(typed, egui::Button::new("JOIN"))
                    .on_disabled_hover_text("type an address, or pick a game above")
                    .clicked()
                {
                    join(net);
                }
                // **Knock first.**
                //
                // A game on the wifi is in the list above and needs none of this. A game
                // on a server somewhere is an address somebody read out, and pressing
                // join on a wrong one used to do nothing whatever — no telling a shut
                // port from a wrong word from a machine that is switched off. Three
                // different evenings and the same silence for all of them.
                if ui
                    .add_enabled(typed, egui::Button::new("KNOCK"))
                    .on_hover_text("ask what is there, without joining it")
                    .clicked()
                {
                    net.knocked = None;
                    net.probe = Some(sail_net::discovery::Probe::begin(
                        &with_port(&net.address),
                        &net.word,
                    ));
                }
            });
            // And the word, for a game that is not on this network.
            //
            // A game on the wifi has no door and needs none; a game on a public address
            // has one, or the first port scan that reaches it is somebody's boat in your
            // archipelago. It is a doorbell and not a password — it goes over the wire in
            // the clear, like everything else here — so it is typed in a plain box
            // beside the address rather than dressed up as a credential.
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("word")
                        .color(egui::Color32::from_rgb(150, 150, 130))
                        .monospace()
                        .size(11.0),
                );
                let box_ = ui.add(
                    egui::TextEdit::singleline(&mut net.word)
                        .desired_width(150.0)
                        .hint_text("only if the game has one")
                        .font(egui::TextStyle::Monospace),
                );
                if box_.changed() {
                    // The same again: a knock that got in with the old word says nothing
                    // about this one.
                    net.knocked = None;
                }
                box_.on_hover_text(
                    "whoever is hosting will tell you this; a game on your own network \
                     will not have one",
                );
            });
            // Whatever the knock found, or that it is still knocking.
            if net.probe.is_some() && net.knocked.is_none() {
                ui.label(
                    egui::RichText::new("knocking…")
                        .color(egui::Color32::from_rgb(150, 150, 130))
                        .monospace()
                        .size(10.0),
                );
            }
            match &net.knocked {
                Some(Ok(found)) => {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} — {} aboard, sea {}",
                            found.game, found.players, found.seed
                        ))
                        .color(egui::Color32::from_rgb(150, 200, 150))
                        .monospace()
                        .size(10.0),
                    );
                }
                Some(Err(why)) => {
                    ui.label(
                        egui::RichText::new(why)
                            .color(egui::Color32::from_rgb(210, 140, 120))
                            .monospace()
                            .size(10.0),
                    );
                }
                None => {}
            }
        }
    }

    if !net.trouble.is_empty() {
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(&net.trouble)
                .color(egui::Color32::from_rgb(210, 140, 120))
                .monospace()
                .size(10.0),
        );
    }
}

/// What a typed address means, in the one place that decides it.
///
/// A bare name or number is the commonest thing to type and the port is fixed anyway,
/// so it gets one. Here rather than inside `join` because the knock has to ask the same
/// address the join would use — two readings of the same box is how a game answers a
/// knock and then refuses the join.
fn with_port(typed: &str) -> String {
    let typed = typed.trim();
    if typed.contains(':') {
        typed.to_string()
    } else {
        format!("{typed}:{}", sail_net::PORT)
    }
}

fn join(net: &mut crate::net::Net) {
    if net.address.trim().is_empty() {
        net.trouble = "nothing to join: pick a game above, or type an address".into();
        return;
    }
    let address = with_port(&net.address);
    net.trouble.clear();
    match sail_net::link::Link::join(&address, &net.name, &net.word.clone()) {
        Ok(link) => {
            net.link = Some(link);
            // Nothing to look for while we are in one.
            net.scan = None;
        }
        Err(e) => net.trouble = format!("{address}: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fleet::{Fleet, Ghost};
    use crate::net::Net;

    /// The picture row offers every width the renderer has, and the sharp picture.
    ///
    /// Reported as the thing that was missing: one drawn size, and a large screen has
    /// no way to ask for a finer one. The failure this guards is the quiet half of that
    /// — a panel listing sizes of its own, which then drift from the renderer's.
    #[test]
    fn the_picture_row_offers_every_width() {
        let mut retro = crate::retro::Retro::default();
        let said = words(|ui| super::pixels(ui, &mut retro));
        for w in crate::retro::Retro::widths() {
            assert!(
                said.contains(&w.to_string()),
                "{w} is not on the row: {said}"
            );
        }
        assert!(
            said.contains("sharp"),
            "no sharp picture on the row: {said}"
        );
        assert!(
            said.contains("pixels"),
            "the row has lost its label: {said}"
        );
    }

    /// Clicking is not tested — this is: whatever step is set, the row can draw it.
    ///
    /// A step the panel cannot render is how a remembered setting turns into a blank
    /// row, and the index is a plain `usize` that several things write.
    #[test]
    fn every_step_can_be_drawn() {
        for step in (0..crate::retro::Retro::widths().len())
            .map(Some)
            .chain([None])
        {
            // Built and then set, rather than with struct update syntax: the rest of
            // `Retro` is render state and rightly private to it.
            let mut retro = crate::retro::Retro::default();
            retro.step = step;
            let said = words(|ui| super::pixels(ui, &mut retro));
            assert!(
                said.contains("pixels"),
                "step {step:?} drew nothing readable: {said}"
            );
        }
    }

    /// Every word the panel actually draws, in one string.
    ///
    /// egui needs no window: it lays out into shapes and hands them back, and the text
    /// shapes carry the text. So a test can ask the question a person asks — *what does
    /// it say* — instead of the question a test usually settles for, which is whether the
    /// function ran without panicking.
    fn words(add: impl FnOnce(&mut egui::Ui)) -> String {
        let ctx = egui::Context::default();
        // `run_ui` rather than `run`: this egui gives a bare `Ui` straight out, which is
        // what these functions take.
        let mut add = Some(add);
        let output = ctx.run_ui(egui::RawInput::default(), move |ui| {
            if let Some(add) = add.take() {
                add(ui);
            }
        });
        // epaint hands back a list of texture uploads with the frame and panics on drop
        // if nobody took them — a real guard in an application and a nuisance here, where
        // there is no renderer to upload a font atlas to. Taken and dropped on purpose.
        let mut output = output;
        output.textures_delta.clear();
        let mut said = String::new();
        fn walk(shape: &egui::epaint::Shape, into: &mut String) {
            match shape {
                egui::epaint::Shape::Text(text) => {
                    into.push_str(text.galley.text());
                    into.push('\n');
                }
                egui::epaint::Shape::Vec(many) => {
                    for shape in many {
                        walk(shape, into);
                    }
                }
                _ => {}
            }
        }
        for clipped in &output.shapes {
            walk(&clipped.shape, &mut said);
        }
        said
    }

    /// Reported from the deck as "I cannot see the network menu", and the answer that
    /// time was that the game being looked at was a release older than the menu. Which is
    /// the right answer and a poor one to have to give: nothing in this repository could
    /// have told anybody whether the page was there, in any version, without opening a
    /// window and pressing Escape.
    #[test]
    fn there_is_a_page_for_sailing_together_and_you_can_get_to_it() {
        assert!(
            PAGES.iter().any(|(tab, _)| *tab == Tab::Network),
            "the panel has no network page at all"
        );
        let mut current = Tab::Sound;
        let said = words(|ui| tabs(ui, &mut current));
        for (_, name) in PAGES {
            assert!(
                said.contains(name),
                "the tab bar does not show {name}: {said:?}"
            );
        }
        assert!(
            said.contains("TOGETHER"),
            "the page is found by its name, by a person: {said:?}"
        );
        assert_eq!(
            current,
            Tab::Sound,
            "drawing a tab bar must not switch pages"
        );
    }

    /// The three states the page can be in, in the words it uses for them.
    #[test]
    fn the_page_says_which_of_the_three_things_is_happening() {
        let fleet = Fleet::default();
        let here = glam::DVec2::ZERO;

        // Not in a game: it looks for one, and says so when there is nothing to find.
        let mut net = Net::default();
        let said = words(|ui| together(ui, &mut net, &fleet, here));
        assert!(
            said.contains("no games on this network"),
            "an empty list has to say it is empty: {said:?}"
        );
        assert!(
            net.scan.is_some(),
            "opening the page should start looking for games"
        );

        // Trouble, which the panel must not swallow: a person who cannot join needs the
        // reason on the screen and not in a log file they will never open.
        net.trouble = "no route to that address".into();
        let said = words(|ui| together(ui, &mut net, &fleet, here));
        assert!(
            said.contains("no route to that address"),
            "the reason it failed has to be visible: {said:?}"
        );
    }

    /// Pressing JOIN with an empty box.
    ///
    /// Reported from the deck as a screenful of ":7714: failed to lookup address
    /// information: nodename nor servname provided, or not known" — which is an
    /// operating system's way of saying "you typed nothing", and a poor way for a game
    /// to say it. The button is grey now, and the function refuses before it resolves
    /// anything, because a disabled button can still be reached by other paths.
    #[test]
    fn joining_nothing_says_so_in_words_a_person_uses() {
        let mut net = Net::default();
        assert!(net.address.is_empty());
        join(&mut net);
        assert!(net.link.is_none(), "it should not have opened a socket");
        assert!(
            net.trouble.contains("pick a game above"),
            "it should say what to do instead: {:?}",
            net.trouble
        );
        assert!(
            !net.trouble.contains("servname"),
            "and not hand the resolver's words to a sailor: {:?}",
            net.trouble
        );

        // And the button is not offered while there is nothing in the box.
        let fleet = Fleet::default();
        let said = words(|ui| together(ui, &mut net, &fleet, glam::DVec2::ZERO));
        assert!(
            said.contains("JOIN"),
            "the button is still there, just grey"
        );
    }

    /// A boat in the list comes with how far off she is — the thing a sailor actually
    /// wants from a list of names, and the thing this panel once promised in a comment
    /// while printing the name alone. That lie survived a review; this does not let it
    /// come back.
    #[test]
    fn a_boat_in_the_list_comes_with_how_far_off_she_is() {
        let mut fleet = Fleet::default();
        let her = sail_net::Drawn {
            pos: [0.0, 2778.0], // a mile and a half north
            ..Default::default()
        };
        fleet.boats.push(Ghost {
            id: 2,
            name: "miki".into(),
            drawn: her,
            age: 0.0,
        });
        // The third state needs a real link, because the page reads the link and the
        // joined flag together — a flag on its own is the "not in a game" state, which is
        // what this test found when it tried to fake it. Nobody has to be listening on
        // the other end: a link is a socket and a thread, and it knocks into the dark
        // quite happily.
        let mut net = Net {
            link: Some(sail_net::link::Link::join("127.0.0.1:1", "test", "").expect("a socket")),
            joined: true,
            ..Default::default()
        };
        let said = words(|ui| together(ui, &mut net, &fleet, glam::DVec2::ZERO));
        assert!(
            said.contains("miki"),
            "her name is not in the list: {said:?}"
        );
        assert!(
            said.contains("1.50 nm"),
            "and nor is how far off she is: {said:?}"
        );
    }
}
