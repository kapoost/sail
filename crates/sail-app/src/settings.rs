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
            sea: 0.08,
            boat: 0.70,
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
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    if help.open {
        help.open = false;
    } else {
        settings.open = !settings.open;
    }
}

// A system's parameters are its dependency list rather than an argument list, and this
// panel reaches four more resources than it did now that the picture is settable from
// somewhere other than a keypress.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut contexts: bevy_egui::EguiContexts,
    mut settings: ResMut<Settings>,
    mut sim: ResMut<Sim>,
    mut sky: ResMut<crate::sky::Sky>,
    sound: Res<SoundHandle>,
    mut view: ResMut<crate::camera::View>,
    mut ink: ResMut<crate::world::Ink>,
    mut retro: ResMut<crate::retro::Retro>,
    mut hud: ResMut<crate::hud::HudVisible>,
    mut net: ResMut<crate::net::Net>,
    fleet: Res<crate::fleet::Fleet>,
    mut lesson: ResMut<crate::tutorial::Tutorial>,
) -> Result {
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
                            if ui.selectable_label(*view == which, name).clicked() {
                                *view = which;
                            }
                        }
                    });
                    // The drawn sea. Not a quality setting: a different picture, and the one
                    // this boat was always headed for.
                    if ui.selectable_label(ink.0, "the ink drawing").clicked() {
                        ink.0 = !ink.0;
                    }
                    // Resolution. Named in pixels of height, because that is what the old
                    // machines were sold as and it is what the look is.
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("pixels")
                                .color(egui::Color32::from_rgb(150, 150, 130))
                                .monospace()
                                .size(11.0),
                        );
                        for (name, step) in [
                            ("sharp", None),
                            ("960", Some(2usize)),
                            ("720", Some(1)),
                            ("480", Some(0)),
                        ] {
                            if ui.selectable_label(retro.step == step, name).clicked() {
                                retro.step = step;
                            }
                        }
                    });
                    if ui
                        .selectable_label(hud.0, "the diagnostic panel  (Tab)")
                        .clicked()
                    {
                        hud.0 = !hud.0;
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
                        .add(egui::Slider::new(&mut hour, 4.0..=20.0).text("hour"))
                        .changed()
                    {
                        *sky = crate::sky::Sky::at(hour, sky.cover);
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
                    let knots = sail_core::units::ms_to_kn(wind.synoptic.base_speed);
                    let mut force = crate::weather::force(knots);
                    if ui
                        .add(egui::Slider::new(&mut force, 1..=9).text("wind, force"))
                        .changed()
                    {
                        wind.synoptic.base_speed =
                            sail_core::units::kn_to_ms(crate::weather::force_wind(force));
                        settings.weather_touched = true;
                    }
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

                if settings.tab == Tab::Drill {
                    drill(ui, &mut lesson);
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
                footer(ui, &mut close);
            });
        });
    if close {
        settings.open = false;
    }
    Ok(())
}

/// The drill page: start it, stop it, and what it is for.
///
/// A page rather than a key, because starting a man-overboard drill by accident in the
/// middle of a delivery would be a poor joke, and because this is where somebody who has
/// just opened the game looks for the thing that teaches them to sail it.
fn drill(ui: &mut egui::Ui, lesson: &mut crate::tutorial::Tutorial) {
    heading(ui, "MAN OVERBOARD DRILL");
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
pub const PAGES: [(Tab, &str); 5] = [
    (Tab::Sound, "SOUND"),
    (Tab::Picture, "PICTURE"),
    (Tab::Weather, "WEATHER"),
    (Tab::Network, "TOGETHER"),
    (Tab::Drill, "DRILL"),
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
                ui.add(
                    egui::TextEdit::singleline(&mut net.address)
                        .desired_width(150.0)
                        .hint_text("address:7714")
                        .font(egui::TextStyle::Monospace),
                );
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
                ui.add(
                    egui::TextEdit::singleline(&mut net.word)
                        .desired_width(150.0)
                        .hint_text("only if the game has one")
                        .font(egui::TextStyle::Monospace),
                )
                .on_hover_text(
                    "whoever is hosting will tell you this; a game on your own network \
                     will not have one",
                );
            });
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

fn join(net: &mut crate::net::Net) {
    if net.address.trim().is_empty() {
        net.trouble = "nothing to join: pick a game above, or type an address".into();
        return;
    }
    let address = if net.address.contains(':') {
        net.address.trim().to_string()
    } else {
        // A bare address is the commonest thing to type and the port is fixed anyway.
        format!("{}:{}", net.address.trim(), sail_net::PORT)
    };
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
