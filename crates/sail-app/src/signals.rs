//! Talking to a ship with flags.
//!
//! **A pilot boat and the ship she is leading have to be able to say things to each
//! other, and they cannot shout.** Half a cable of water and two sets of canvas swallow a
//! voice; a hundred and fifty years before anybody had a radio, the answer was a set of
//! flags that every ship in the world carried and every officer could read. It is still
//! carried and still read, and the single-letter signals of the International Code are
//! the part of it that means something on its own — a ship with one flag up is making one
//! statement, and the statements are chosen so that the ones you need in a hurry are one
//! flag.
//!
//! The ones in this game are real and mean what the code says they mean:
//!
//! * **K** — *I wish to communicate with you.* Here it is the question a pilot actually
//!   wants answered: are you with me? She replies with what she is doing.
//! * **L** — *You should stop your vessel instantly.*
//! * **X** — *Stop carrying out your intentions and watch for my signals.* Hold where you
//!   are; I am not ready for you.
//! * **P** — the Blue Peter, *about to proceed to sea*. Carry on; make sail again.
//! * **U** — *You are running into danger.* The one signal in the set that is about the
//!   water rather than about her.
//!
//! And she answers the way a ship answers: the **answering pendant**, hoisted *to the
//! dip* when the hoist has been seen and *close up* when it has been read and will be
//! acted on. That is the real procedure and it is also exactly the feedback this mission
//! was missing — the question "is she following me" had no answer on deck, and now it is
//! a flag you can see from the helm.
//!
//! **She has to be able to see it.** A hoist is cloth: inside the distance a flag can be
//! read from her deck, and not through fog or dark. A signal she never saw does nothing,
//! which is honest and is why the pendant matters.

use bevy::prelude::*;
use bevy_egui::egui;
use glam::DVec2;

use crate::mission::Pilotage;
use crate::ship::Brig;
use crate::sim::Sim;

/// How far a single flag can be read from her deck, metres.
///
/// Six cables. A flag is about half a metre across and the eye resolves a minute of arc,
/// so the *letter* is gone long before the colours are; what is being read at this range
/// is a pattern somebody already expects. Shorter than [`crate::mission::IN_SIGHT`] on
/// purpose: she can see your boat from further off than she can read your hoist, and the
/// difference between those two is a thing a pilot feels.
pub const READABLE: f64 = 1_100.0;

/// The signals this boat carries, in the order they are bent on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flag {
    /// K — *I wish to communicate with you.*
    Kilo,
    /// L — *You should stop your vessel instantly.*
    Lima,
    /// X — *Stop carrying out your intentions and watch for my signals.*
    Xray,
    /// P — the Blue Peter: *about to proceed.*
    Papa,
    /// U — *You are running into danger.*
    Uniform,
}

impl Flag {
    pub const ALL: [Flag; 5] = [
        Flag::Kilo,
        Flag::Lima,
        Flag::Xray,
        Flag::Papa,
        Flag::Uniform,
    ];

    /// The letter, which is what anybody on a bridge would call it.
    pub fn letter(self) -> &'static str {
        match self {
            Flag::Kilo => "K",
            Flag::Lima => "L",
            Flag::Xray => "X",
            Flag::Papa => "P",
            Flag::Uniform => "U",
        }
    }

    /// What the code says it means. Quoted rather than paraphrased: this is the one place
    /// in the game where the player is learning something they can take off the screen.
    pub fn means(self) -> &'static str {
        match self {
            Flag::Kilo => "I wish to communicate with you",
            Flag::Lima => "you should stop your vessel instantly",
            Flag::Xray => "stop what you are doing and watch for my signals",
            Flag::Papa => "about to proceed — the Blue Peter",
            Flag::Uniform => "you are running into danger",
        }
    }

    /// And what it asks of her, in this trade.
    pub fn asks(self) -> &'static str {
        match self {
            Flag::Kilo => "answer me: what are you doing?",
            Flag::Lima => "take the way off her",
            Flag::Xray => "hold where you are and wait for me",
            Flag::Papa => "carry on — make sail and follow",
            Flag::Uniform => "sheer off, there is no water that way",
        }
    }

    /// The flag itself, as the rectangles it is sewn from.
    ///
    /// `(across, down, wide, tall, colour)` in fractions of the flag, with *across*
    /// measured from the halyard and *down* from the head. Pieces later in the list are
    /// sewn on top of the ones before them, which is how a cross and a centre panel are
    /// made — and how they are made in cloth, too.
    ///
    /// These are the standard depictions. Getting them right is not decoration in a game
    /// whose whole claim is that the detail is real: somebody who learns K and L here is
    /// learning the flags that are on every bridge in the world, and a blue bar where the
    /// code has a blue cross would be teaching them wrong.
    pub fn cloth(self) -> Vec<(f32, f32, f32, f32, Color)> {
        let yellow = Color::srgb(0.93, 0.78, 0.12);
        let blue = Color::srgb(0.07, 0.29, 0.62);
        let black = Color::srgb(0.08, 0.08, 0.09);
        let white = Color::srgb(0.93, 0.92, 0.89);
        let red = Color::srgb(0.78, 0.13, 0.13);
        match self {
            // Yellow at the hoist, blue flying.
            Flag::Kilo => vec![(0.0, 0.0, 0.5, 1.0, yellow), (0.5, 0.0, 0.5, 1.0, blue)],
            // Quartered yellow and black, yellow in the upper hoist.
            Flag::Lima => vec![
                (0.0, 0.0, 0.5, 0.5, yellow),
                (0.5, 0.0, 0.5, 0.5, black),
                (0.0, 0.5, 0.5, 0.5, black),
                (0.5, 0.5, 0.5, 0.5, yellow),
            ],
            // White with a blue **cross** — both arms, which is the whole of telling X
            // apart from a flag with a stripe down it.
            Flag::Xray => vec![
                (0.0, 0.0, 1.0, 1.0, white),
                (0.38, 0.0, 0.24, 1.0, blue),
                (0.0, 0.38, 1.0, 0.24, blue),
            ],
            // The Blue Peter: blue with a white rectangle in the middle of it, not a
            // band across it.
            Flag::Papa => vec![(0.0, 0.0, 1.0, 1.0, blue), (0.3, 0.3, 0.4, 0.4, white)],
            // Quartered white and red, white in the upper hoist.
            Flag::Uniform => vec![
                (0.0, 0.0, 0.5, 0.5, white),
                (0.5, 0.0, 0.5, 0.5, red),
                (0.0, 0.5, 0.5, 0.5, red),
                (0.5, 0.5, 0.5, 0.5, white),
            ],
        }
    }

    /// Whether what she is told takes the helm out of the signal book and puts it on the
    /// wheel.
    ///
    /// **A question is not an order.** K asks what she is doing and P tells her to carry
    /// on; neither of them is a reason to stop following the boat ahead, and gating her
    /// steering on *any* signal meant that asking her whether she was following made her
    /// stop — the exact opposite of what the flag was added for. L, X and U are the three
    /// that take the helm, and they take it because each of them is an order about where
    /// she is to be.
    pub fn takes_the_helm(self) -> bool {
        matches!(self, Flag::Lima | Flag::Xray | Flag::Uniform)
    }
}

impl Flag {
    /// The flag of that letter, for `SAIL_HOIST=L`.
    pub fn of(letter: &str) -> Option<Self> {
        let letter = letter.trim().to_ascii_uppercase();
        Self::ALL.into_iter().find(|f| f.letter() == letter)
    }
}

/// What is at our masthead, and since when.
///
/// `SAIL_HOIST=L` opens with one bent on, which is how the cloth and the answer get
/// looked at without sailing out to find a ship first.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Hoist {
    pub flying: Option<Flag>,
    /// Simulation time it went up, so that her answer can take the few seconds a lookout
    /// and a signal book actually take.
    pub since: f64,
}

impl Default for Hoist {
    fn default() -> Self {
        Self {
            flying: crate::knob::var("SAIL_HOIST")
                .ok()
                .and_then(|v| Flag::of(&v)),
            since: 0.0,
        }
    }
}

/// How the answering pendant is flying, which is her whole half of the conversation.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Pendant {
    /// Nothing flying: either nothing has been said, or she cannot see it.
    #[default]
    Down,
    /// *Seen.* Hoisted halfway — a lookout has it and the officer is reading it.
    Dip,
    /// *Read and understood.* Close up to the block, and she is doing it.
    CloseUp,
}

/// Her side of the conversation.
#[derive(Resource, Default, Clone, Debug)]
pub struct Answer {
    pub pendant: Pendant,
    /// What she has been told to do, once she has read it — which is what the rest of
    /// the mission acts on rather than the hoist itself, because an order she never saw
    /// is not an order.
    pub reading: Option<Flag>,
    /// What she called back, if the signal asked for words.
    pub said: Option<String>,
}

/// How long a ship takes to see a hoist, read it and answer, seconds.
///
/// **Not instant, and this is not friction for its own sake.** There is a lookout, a man
/// at the signal halyard and an officer with the book, and the three of them take a few
/// seconds on a good day. It also makes the pendant worth watching: a signal that is
/// obeyed in the same frame it is hoisted has no moment of *has she seen it*, which is
/// the moment the whole mechanic is about.
const SEEING: f64 = 2.5;
const READING: f64 = 4.5;

/// Whether she can read what is at our masthead.
///
/// Distance and daylight. Fog is not modelled as a signal-blocker yet — the visibility
/// the haze is drawn from is the honest number to use the day it is.
pub fn can_read(brig: &Brig, boat: DVec2, visibility: f64) -> bool {
    let range = (boat - brig.at).length();
    range < READABLE.min(visibility)
}

/// Hoist, read, answer.
pub fn signalling(
    hoist: Res<Hoist>,
    mut answer: ResMut<Answer>,
    brig: Res<Brig>,
    job: Res<Pilotage>,
    sim: Res<Sim>,
    sky: Res<crate::sky::Sky>,
) {
    let running = job.running && job.over.is_none() && brig.afloat;
    let Some(flag) = hoist.flying.filter(|_| running) else {
        *answer = Answer::default();
        return;
    };
    // Daylight and distance. A flag at night is a rag: there is no night in this game
    // yet, but the sun is modelled and the rule costs one line now rather than a
    // rewrite later.
    let daylight = sky.elevation().to_degrees() > -2.0;
    if !daylight
        || !can_read(
            &brig,
            sim.sim.state.pos,
            crate::sky::visibility(&sky) as f64,
        )
    {
        answer.pendant = Pendant::Down;
        answer.reading = None;
        return;
    }
    let since = sim.sim.state.time - hoist.since;
    answer.pendant = match since {
        t if t < SEEING => Pendant::Down,
        t if t < READING => Pendant::Dip,
        _ => Pendant::CloseUp,
    };
    let reading = (answer.pendant == Pendant::CloseUp).then_some(flag);
    // A new order is a new answer: what she called back about the last one is not an
    // answer to this one, and leaving it up would have her saying "making sail" with L
    // flying.
    if reading != answer.reading {
        answer.said = None;
    }
    answer.reading = reading;
}

/// The hoist, as a panel — the halyard you actually pull.
///
/// Only while there is a ship to talk to, like every other panel in this game that is
/// about a place rather than about the boat. What is on it is five flags and a line to
/// haul them down, and each one says what the code says it means as well as what it asks
/// of her: the player who reads this panel twice has learnt five real signals.
pub fn draw(
    mut contexts: bevy_egui::EguiContexts,
    mut hoist: ResMut<Hoist>,
    answer: Res<Answer>,
    job: Res<Pilotage>,
    brig: Res<Brig>,
    sim: Res<Sim>,
) -> Result {
    if !job.running || job.over.is_some() || !brig.afloat {
        return Ok(());
    }
    let range = (brig.at - sim.sim.state.pos).length();
    if range > crate::mission::IN_SIGHT * 1.5 {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let ink = egui::Color32::from_rgb(230, 198, 120);
    let quiet = egui::Color32::from_rgb(150, 150, 130);
    let frame = egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(14, 18, 24, 232))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 82, 96)))
        .inner_margin(egui::Margin::symmetric(14, 10));
    let mut hoisting: Option<Option<Flag>> = None;
    egui::Area::new("signals".into())
        // Bottom left, above the frame counter: the chandler's list is anchored in the
        // middle of the same side, and standing alongside the platform with a ship in
        // sight put one panel over the other.
        .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(12.0, -34.0))
        .show(ctx, |ui| {
            frame.show(ui, |ui| {
                ui.set_width(292.0);
                ui.label(
                    egui::RichText::new("THE SIGNAL HALYARD")
                        .monospace()
                        .size(12.0)
                        .color(ink),
                );
                ui.label(
                    egui::RichText::new(match (hoist.flying, answer.pendant) {
                        (None, _) => "nothing flying".to_string(),
                        (Some(f), Pendant::Down) => {
                            format!("{} flying — she has not seen it", f.letter())
                        }
                        (Some(f), Pendant::Dip) => {
                            format!("{} flying — answered at the dip: seen", f.letter())
                        }
                        (Some(f), Pendant::CloseUp) => {
                            format!("{} flying — answered close up: understood", f.letter())
                        }
                    })
                    .monospace()
                    .size(11.0)
                    .color(if answer.pendant == Pendant::CloseUp {
                        ink
                    } else {
                        quiet
                    }),
                );
                ui.add_space(6.0);
                for flag in Flag::ALL {
                    let up = hoist.flying == Some(flag);
                    if ui
                        .selectable_label(
                            up,
                            egui::RichText::new(format!("{} — {}", flag.letter(), flag.asks()))
                                .monospace()
                                .size(11.0),
                        )
                        .on_hover_text(flag.means())
                        .clicked()
                    {
                        hoisting = Some((!up).then_some(flag));
                    }
                }
                if hoist.flying.is_some() {
                    ui.add_space(4.0);
                    if ui
                        .button(
                            egui::RichText::new("haul down")
                                .monospace()
                                .size(11.0)
                                .color(quiet),
                        )
                        .clicked()
                    {
                        hoisting = Some(None);
                    }
                }
            });
        });
    if let Some(flying) = hoisting {
        hoist.flying = flying;
        hoist.since = sim.sim.state.time;
        if let Some(flag) = flying {
            info!("hoisted {} — {}", flag.letter(), flag.means());
        }
    }
    Ok(())
}

/// The cloth of her answering pendant, at the main masthead.
#[derive(Component)]
pub struct Answering;

/// Fly it, dip it, or haul it down.
///
/// **At the dip means seen; close up means understood**, and the difference has to be
/// visible from another boat or it is not telling anybody anything. *At the dip* is
/// halfway up the halyard — not a hand's breadth below the block, which is what it was
/// and which at three cables is no difference at all. Five metres of mast between the two
/// states, which reads as a flag that has gone *up* when she understands.
///
/// The panel says it in words as well, because the signal book is the thing being taught
/// here and the cloth is the reminder that it is a thing on a rope.
pub fn fly_the_pendant(
    answer: Res<Answer>,
    mut cloth: Query<(&mut Visibility, &mut Transform), With<Answering>>,
) {
    for (mut show, mut at) in &mut cloth {
        *show = match answer.pendant {
            Pendant::Down => Visibility::Hidden,
            _ => Visibility::Inherited,
        };
        // Close up to the topmast cap, or halfway down the halyard.
        at.translation.y = match answer.pendant {
            Pendant::CloseUp => 26.5,
            _ => 21.0,
        };
    }
}

/// The cloth itself, on our own signal halyard.
#[derive(Component)]
pub struct Flying;

/// Keep what is drawn in step with what is hoisted.
///
/// Rebuilt whole when the flag changes, which is a handful of times in a passage — the
/// same rule as the pile on the platform, and for the same reason: a thing that changes
/// rarely should not be a thing checked every frame.
pub fn dress(
    mut commands: Commands,
    hoist: Res<Hoist>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    boat: Query<Entity, With<crate::boat::BoatRoot>>,
    flying: Query<Entity, With<Flying>>,
    mut shown: Local<Option<Option<Flag>>>,
) {
    if *shown == Some(hoist.flying) {
        return;
    }
    let Ok(root) = boat.single() else {
        return;
    };
    *shown = Some(hoist.flying);
    for entity in &flying {
        commands.entity(entity).despawn();
    }
    let Some(flag) = hoist.flying else {
        return;
    };
    // Half a metre of bunting on the port shroud — the starboard one has H on it while
    // the job is on, which is where a pilot boat's own flag belongs.
    const WIDE: f32 = 0.46;
    const TALL: f32 = 0.38;
    for (n, (across, down, wide, tall, colour)) in flag.cloth().into_iter().enumerate() {
        let mesh = meshes.add(Cuboid::new(0.02, TALL * tall, WIDE * wide));
        commands.entity(root).with_children(|b| {
            b.spawn((
                Flying,
                Mesh3d(mesh.clone()),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: colour,
                    perceptual_roughness: 0.92,
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                })),
                Transform::from_xyz(
                    // A piece sewn on top of another stands a millimetre proud of it,
                    // which is both true of cloth and the only way two quads in the same
                    // plane do not fight over which is in front.
                    -1.35 - n as f32 * 0.002,
                    3.6 - TALL * (down + tall * 0.5),
                    -0.2 + WIDE * (across + wide * 0.5) - WIDE * 0.5,
                ),
            ));
        });
    }
}

/// What this module does in a frame.
pub fn plugin(app: &mut App) {
    use crate::stages::{Deck, Paint};
    app.init_resource::<Hoist>();
    app.init_resource::<Answer>();
    app.add_systems(
        Update,
        (signalling, dress, fly_the_pendant)
            .chain()
            .in_set(Deck::Weather),
    );
    // Furniture rather than a card: it is a thing on the edge of the screen that is
    // there while a ship is, the same as the console and the chandler's list.
    app.add_systems(
        bevy_egui::EguiPrimaryContextPass,
        draw.in_set(Paint::Instruments),
    );
}

#[cfg(test)]
mod what_a_ship_can_read {
    use super::*;

    /// **A hoist is cloth, and cloth has a range.** Six cables is a flag you can read;
    /// a mile and a half is a boat you can see with nothing legible on it.
    #[test]
    fn a_flag_is_read_nearer_than_a_boat_is_seen() {
        // A constant against a constant, so the compiler can see it is true — which is
        // the point: this is a statement about the design that must not quietly stop
        // being true when somebody tunes one of the two numbers.
        const _: () = assert!(READABLE < crate::mission::IN_SIGHT);
        let brig = Brig {
            at: DVec2::ZERO,
            ..Default::default()
        };
        assert!(can_read(&brig, DVec2::new(400.0, 0.0), 8_000.0));
        assert!(
            !can_read(&brig, DVec2::new(1_300.0, 0.0), 8_000.0),
            "a flag was read from seven cables off"
        );
    }

    /// **X is a cross and P is a panel**, which is the whole of telling either of them
    /// from a flag with a stripe down it — and the reason this is a test rather than a
    /// glance at a screenshot is that a wrong flag teaches somebody a wrong flag.
    #[test]
    fn the_cross_has_two_arms_and_the_blue_peter_has_a_middle() {
        let x = Flag::Xray.cloth();
        let upright = x.iter().any(|(across, down, wide, tall, _)| {
            *down == 0.0 && *tall == 1.0 && *wide < 0.5 && *across > 0.0
        });
        let arm = x
            .iter()
            .any(|(across, _, wide, tall, _)| *across == 0.0 && *wide == 1.0 && *tall < 0.5);
        assert!(upright && arm, "X came out as a stripe rather than a cross");

        let p = Flag::Papa.cloth();
        let panel = p.iter().any(|(across, down, wide, tall, _)| {
            *across > 0.0 && *down > 0.0 && across + wide < 1.0 && down + tall < 1.0
        });
        assert!(
            panel,
            "the Blue Peter came out as a band rather than a panel"
        );
    }

    /// **A question is not an order.** K and P leave the helm where it was; the three
    /// that say where she is to be take it.
    #[test]
    fn only_three_of_them_take_the_helm() {
        assert!(!Flag::Kilo.takes_the_helm(), "asking her stopped her");
        assert!(
            !Flag::Papa.takes_the_helm(),
            "telling her to carry on stopped her"
        );
        for flag in [Flag::Lima, Flag::Xray, Flag::Uniform] {
            assert!(flag.takes_the_helm(), "{} did nothing", flag.letter());
        }
    }

    /// And in thick weather the visibility wins over the distance.
    #[test]
    fn she_cannot_read_what_she_cannot_see() {
        let brig = Brig {
            at: DVec2::ZERO,
            ..Default::default()
        };
        assert!(
            !can_read(&brig, DVec2::new(600.0, 0.0), 300.0),
            "a flag was read through three cables of fog"
        );
    }

    /// Every flag in the set says what the code says, and asks for something different.
    #[test]
    fn the_set_is_five_distinct_signals() {
        let mut letters: Vec<&str> = Flag::ALL.iter().map(|f| f.letter()).collect();
        letters.sort_unstable();
        letters.dedup();
        assert_eq!(letters.len(), Flag::ALL.len(), "two flags share a letter");
        for flag in Flag::ALL {
            assert!(!flag.means().is_empty() && !flag.asks().is_empty());
            // Every piece is inside the flag, and between them they cover it: the first
            // rectangle of each is the ground, and a ground with a hole in it is a flag
            // you can see the shroud through.
            let cloth = flag.cloth();
            assert!(!cloth.is_empty(), "{} is bare rope", flag.letter());
            for (across, down, wide, tall, _) in &cloth {
                assert!(
                    *across >= 0.0
                        && *down >= 0.0
                        && across + wide <= 1.001
                        && down + tall <= 1.001,
                    "{} has cloth hanging off the edge of it",
                    flag.letter()
                );
            }
            let ground: f32 = cloth
                .iter()
                .take_while(|(_, down, _, tall, _)| *down == 0.0 && *tall == 1.0 || cloth.len() > 2)
                .map(|(_, _, wide, tall, _)| wide * tall)
                .sum();
            assert!(
                ground >= 0.999,
                "{} covers {:.2} of its own area",
                flag.letter(),
                ground
            );
        }
    }
}
