//! The reel: the game playing itself through its own scenes, for filming.
//!
//! **A trailer wants footage this program cannot take of itself.** Its own screenshot
//! dials can photograph a scene and can even record one frame at a time — but not faster
//! than about twelve a second, because past that the capture is served a picture that has
//! not been drawn yet and every frame comes back black. Twelve interpolated up to
//! twenty-four is a cut; it is not sixty frames of real motion with the rigging in it.
//!
//! So this is the other way round: instead of the game filming itself, something else
//! films the game — a screen recorder, which takes sixty real frames a second and the
//! sound with them — and the game's job is to **put on the show without a hand on the
//! keyboard**. `SAIL_REEL` starts a timetable: the weather, the hour, the camera, the
//! course and the chart are each changed at a stated second, in an order that builds.
//!
//! There are two of them, because they are for two different things.
//!
//! * `SAIL_REEL=1` — **the passage.** A minute and three quarters of what the game *is*:
//!   the boat, the deck, the paper, the gale, the drawn sea, home, a flag, the night. It
//!   has the instruments in it, because they are half the game.
//! * `SAIL_REEL=short` — **under way.** Eighty-odd seconds of nothing but a boat sailing:
//!   dawn, the wardrobe changing as she bears away, the breeze making, a reef, sunset,
//!   moonrise, night. No panels, no mission, no text. It is the one to put in front of
//!   somebody who has never seen the game, and it is the one to cut down to sixty.
//!
//! What neither of them is, is a cutscene. Every beat below is the same resource the
//! settings panel, the compass buttons or the view key writes to, changed the same way a
//! hand would change it. The single exception is the sky's clock, which runs a sunrise at
//! five hundred times so that ninety minutes of light fits in ten seconds — see
//! [`crate::sky::Clock`], which says so in its own words.
//!
//! Press record, run it, and cut the result up afterwards: each scene is long enough to
//! take six or seven seconds out of the middle, away from the moment the weather jumps.

use bevy::prelude::*;

use crate::camera::View;
use crate::sim::Sim;

/// One thing the reel does, at one moment.
#[derive(Clone, Copy, Debug)]
pub enum Act {
    /// Force, oktas, the hour, and the fetch in kilometres — the four dials the weather
    /// page has, set together because a scene is all four at once.
    ///
    /// The hour is optional, and the `None` is the whole reason: a reel that is running
    /// its own clock through a sunrise must be able to change the wind without dragging
    /// the sun back to where the last beat left it.
    Weather {
        force: u8,
        oktas: u8,
        hour: Option<f32>,
        fetch: f64,
    },
    /// Where the camera sits.
    Seat(View),
    /// And whether it is sweeping round her.
    Orbit(f32),
    /// The console and the chart: off for the sailing shots, on where the instruments
    /// are the subject.
    Furniture { console: bool, chart: Chart },
    /// The drawn sea rather than the lit one.
    Ink(bool),
    /// A flag at the masthead, for the scene with the brig in it.
    Hoist(crate::signals::Flag),
    /// Moor the platform fine on the bow, so that home comes into the picture.
    Home,
    /// How fast the sun goes. One is a sailing day; five hundred is a sunrise in ten
    /// seconds.
    Clock(f32),
    /// Send the foredeck for a sail.
    ///
    /// **The console has a button for each of these and this presses it.** The watch
    /// chooses its own wardrobe off a minute's average of the wind, which is right at sea
    /// and far too slow to film: measured here, an order given by bearing away took
    /// thirty-odd seconds to come out as cloth, by which time the shot it belonged to was
    /// over. A skipper who wants the kite up now says so, and so does the reel.
    Headsail(sail_core::state::Headsail),
    /// Reef the main: nought for full, one, two, three, four for handed.
    ///
    /// The same as selecting the reef line and hauling it to the next cringle. It runs at
    /// its own pace and stops at the stop, because that is what a slab reef does.
    Reef(usize),
    /// Put the pilot on a course that gives this true wind angle, signed degrees —
    /// positive with the wind to starboard, so the sign is the tack.
    ///
    /// **This is how a sail change gets filmed.** There is no dial for "set the Code 0":
    /// the watch chooses the wardrobe off the angle and the breeze, the way a watch does,
    /// so the way to film the kite going up is to bear away in light air and let them.
    /// The pilot is handed a compass course — the same thing its own four buttons nudge —
    /// worked out from the angle wanted and the angle she is on.
    Steer(f64),
    /// Where the four sliders on the sound page are put.
    ///
    /// **A reel is filmed with a microphone as well as a camera**, and the mix a demo
    /// wants is not the mix a game wants: the sea is the loudest thing in this program
    /// and the gear is the thing worth hearing — a winch, a sheet, a batten going over,
    /// water against the topsides. So the sea goes down to almost nothing and the boat
    /// comes up, and the music goes off entirely because it is written to the cut
    /// afterwards and not to the take.
    Mix {
        master: f32,
        sea: f32,
        boat: f32,
        crew: f32,
        music: f32,
    },
    /// How thick the fog is, nought to one — the weather page's own dial.
    ///
    /// Asked for rather than set: the wind blows fog away, so a bank asked for in
    /// fifteen knots arrives as almost nothing. Which is why the fog scene is a calm
    /// one, and that is weather and not a workaround.
    Fog(f64),
    /// A swell: height, period, and the bearing it comes *from*. Height nought takes it
    /// away again.
    ///
    /// Separate from the wind because it is weather from somewhere else — the thing that
    /// makes an ocean an ocean rather than a windy afternoon.
    Swell { hs: f64, tp: f64, from: f64 },
    /// Lay a mark that many metres dead ahead, with a line astern and three boats
    /// racing for it. Nought takes the course up again.
    ///
    /// **Dead ahead and not dead to windward, which is the same thing on the layline.**
    /// A windward mark is rounded close-hauled with the buoy fine on the bow — that is
    /// what a layline *is* — so a mark laid ahead of a boat that is already close-hauled
    /// is in the right place and can be reached in twenty seconds instead of eight
    /// minutes. A trailer cannot sail a leg.
    Buoy(f64),
    /// Put the brig that many metres off the port bow, under sail and sailing her own
    /// course. Nought takes her away.
    Brig(f64),
    /// The furling line, nought set to one rolled away. The same line the rack hauls.
    ///
    /// **This is how canvas comes off before a gale rather than during one.** A change
    /// of headsail is twenty-four seconds of drum; rolling the one that is up away is
    /// twelve, and in a reel that has to put force eight on the screen at a stated
    /// second, twelve is the difference between storm canvas and a blown-out sail.
    Furl(f64),
    /// The staysail: up or down. Storm canvas, and the reason a deep-reefed boat still
    /// has something forward to balance her.
    Staysail(bool),
}

/// What the chart is doing in a beat.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Chart {
    Away,
    Corner,
    Table,
}

/// Which reel.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Which {
    #[default]
    Passage,
    UnderWay,
    Demo,
}

/// The passage: a minute and three quarters of what the game is.
///
/// Ordered, and the order is the cut: it opens on her sailing, goes round her once, sits
/// the viewer at the helm, puts the paper on the table, builds a gale, turns the sea into
/// a drawing, brings the evening down over the platform and finishes at night with a moon
/// on the water.
pub const PASSAGE: &[(f64, Act)] = &[
    (
        0.0,
        Act::Weather {
            force: 4,
            oktas: 3,
            hour: Some(8.0),
            fetch: 60.0,
        },
    ),
    (
        0.5,
        Act::Furniture {
            console: false,
            chart: Chart::Away,
        },
    ),
    (1.0, Act::Seat(View::Chase)),
    // Round her once: rig, heel, wake, hull.
    (9.0, Act::Orbit(12.0)),
    (20.0, Act::Orbit(0.0)),
    // The deck as a place.
    (21.0, Act::Seat(View::Cockpit)),
    // The paper, which is the half of this game that is not the water.
    (
        31.0,
        Act::Furniture {
            console: true,
            chart: Chart::Table,
        },
    ),
    // A gale, from the same seat she was just sailing quietly in.
    (
        43.0,
        Act::Weather {
            force: 8,
            oktas: 6,
            hour: Some(11.0),
            fetch: 200.0,
        },
    ),
    (
        43.5,
        Act::Furniture {
            console: false,
            chart: Chart::Away,
        },
    ),
    (44.0, Act::Seat(View::Chase)),
    // The drawn sea: the same water, told differently.
    (57.0, Act::Ink(true)),
    // Evening, and home.
    (
        68.0,
        Act::Weather {
            force: 4,
            oktas: 2,
            hour: Some(19.3),
            fetch: 40.0,
        },
    ),
    (68.5, Act::Home),
    (69.0, Act::Ink(false)),
    // A flag at the masthead. Worth filming on its own — the cloth is on the shroud
    // either way — and it is the scene with the brig in it when the reel is run with
    // one: `SAIL_BRIG=600 SAIL_BRIG_SHOW=170` puts her where she can be seen, and she
    // answers this hoist with her pendant.
    (78.0, Act::Hoist(crate::signals::Flag::Kilo)),
    // And the night, with a moon on the water and the chart in the corner under its own
    // dim lamp — which is the one place the red torch can be filmed.
    (
        88.0,
        Act::Weather {
            force: 4,
            oktas: 1,
            hour: Some(1.6),
            fetch: 40.0,
        },
    ),
    (
        88.5,
        Act::Furniture {
            console: true,
            chart: Chart::Corner,
        },
    ),
];

/// Under way: a boat sailing, and nothing else.
///
/// **A sail change takes twenty-four seconds and that is what shapes this.** The furling
/// line runs at 0.085 a second and the change is two full turns of the drum — roll the old
/// one away, swap at the bow, set the new one — so an order given at the shot it belongs
/// to is a bare forestay for the whole shot. Measured, not guessed: the first cut of this
/// reel ordered the gennaker ten seconds before it wanted it and filmed a boat with no
/// headsail. Every order below is therefore given a shot early, and the rolling away is
/// part of the shot before.
///
/// The second thing it shapes is the order of the weather. The kite comes in **half a
/// minute before the breeze arrives**, because a gennaker in force seven is over its
/// cloth and a sail that is over its cloth has about five seconds of dignity left — the
/// twelve seconds it takes to roll away would have blown it out. Which is also how it
/// works at sea: nobody hands a kite because it is already blowing.
///
/// **No panels and no story**, which is the point of having a second one. The passage
/// reel is for somebody deciding whether to buy it; this is for somebody deciding whether
/// to look at it, and what makes that decision is water, light and cloth.
///
/// The sun does the work. Sunrise here is **05:29** and sunset **18:31** — fifty-seven
/// north, five degrees of declination, both read off the same two sines the sky itself
/// uses — so the dawn beat opens at a quarter to five with the clock at five hundred and
/// twenty, which puts the sun on the horizon about five seconds in and gives four more of
/// it climbing. The evening beat opens at ten to six and runs on past sunset without
/// stopping, which is how the moonrise is free: a full moon is at the antisolar point, so
/// it comes up on the opposite bearing as the sun goes down, and the camera only has to
/// turn round.
///
/// The two long scenes are long because somebody else is doing the work in them and they
/// take their own time: the watch chooses the wardrobe off a minute's average of the wind,
/// so bearing away from close-hauled to a broad reach in light air walks up the ladder —
/// genoa, Code 0, gennaker — at the watch's pace and not the reel's. Same at the other
/// end, where it comes on to blow and the kite has to come in before the reef goes down.
/// Both are marked in `docs/store.md` as the scenes to cut from.
pub const UNDER_WAY: &[(f64, Act)] = &[
    (
        0.0,
        Act::Furniture {
            console: false,
            chart: Chart::Away,
        },
    ),
    (
        0.1,
        Act::Weather {
            force: 3,
            oktas: 2,
            hour: Some(4.75),
            fetch: 25.0,
        },
    ),
    (0.2, Act::Seat(View::Chase)),
    (0.4, Act::Steer(50.0)),
    (0.5, Act::Clock(520.0)),
    // SUNRISE, 0.5 to 10 — 04:45 to 06:09. The sun breaks the horizon about five seconds
    // in and climbs for the rest of it; before that the sky is doing the thing it only
    // does for twenty minutes a day.
    (10.0, Act::Clock(1.0)),
    (10.2, Act::Seat(View::Cockpit)),
    // FIRST LIGHT, CLOSE-HAULED, 10 to 18 — the deck as a place, heeled, low sun through
    // the genoa. A texture shot.
    (18.0, Act::Seat(View::Chase)),
    (18.2, Act::Orbit(6.0)),
    (18.4, Act::Steer(135.0)),
    (18.6, Act::Headsail(sail_core::state::Headsail::Gennaker)),
    // BEARING AWAY, 18 to 30 — fifty degrees to a hundred and thirty-five, the camera
    // walking round her the other way, and the genoa rolling away on the stay as she
    // goes. The order is given a shot early on purpose: see the note above.
    (30.0, Act::Orbit(0.0)),
    // THE KITE, 30 to 42 — the bow clear at about thirty, the gennaker going up off the
    // sprit and full and drawing by forty-two, which is the whole twenty-four seconds the
    // drum takes and the reason this shot is twelve and not ten.
    (42.0, Act::Headsail(sail_core::state::Headsail::Genoa)),
    (42.2, Act::Orbit(5.0)),
    // AND IN AGAIN, 42 to 54 — twelve seconds before the breeze arrives, which is when a
    // kite actually comes down. Nobody hands one because it is already blowing.
    (54.0, Act::Orbit(0.0)),
    (
        54.2,
        Act::Weather {
            force: 7,
            oktas: 6,
            hour: None,
            fetch: 150.0,
        },
    ),
    (54.4, Act::Seat(View::Astern)),
    (55.4, Act::Reef(2)),
    // IT COMES ON TO BLOW, 54 to 66 — two slabs into the main and the genoa unrolling
    // into it, seen from aft where the wake tilts as she lies over. The hour is left
    // alone on purpose: the morning carries on.
    (66.0, Act::Seat(View::Chase)),
    (66.2, Act::Steer(115.0)),
    (
        66.4,
        Act::Weather {
            force: 5,
            oktas: 3,
            hour: Some(17.83),
            fetch: 100.0,
        },
    ),
    (66.6, Act::Clock(520.0)),
    // SUNSET, 66 to 76 — 17:50 onwards, the sun on the horizon at about seventy-one.
    (76.0, Act::Orbit(8.0)),
    // MOONRISE, 76 to 85 — the clock still running and the camera coming round to the
    // opposite bearing, which is where a full moon always is. Nothing stages this: it is
    // the same two sines as the sunrise, read backwards.
    (85.0, Act::Orbit(0.0)),
    (85.2, Act::Clock(1.0)),
    // NIGHT UNDER THE MOON, 85 to 93 — half past eight, the moon fourteen degrees up and
    // its path on the water.
];

/// The demo: five scenes, one take, and a mix made for a microphone.
///
/// **Asked for as material to cut, not as a film.** Two minutes and twenty of take for
/// forty-odd seconds of demo, in eight shots — which is the ratio that lets a cut be made out of
/// the middles, away from every moment the weather jumps. Where to cut each one is in
/// `dist/demo.sh`, which prints the list after the run, and in `docs/store.md`.
///
/// # How much material each scene needs, and why that much
///
/// None of these numbers is a taste. Each is the length of something the program
/// actually takes to do:
///
/// * **Sunrise, twenty-four seconds.** Sunrise here is 05:29 — fifty-seven north, five
///   degrees of declination, the same two sines the sky itself uses — and the clock at
///   five hundred and twenty puts 8.8 minutes of it into every second. Opening at 03:45
///   therefore breaks the sun over the horizon at about twelve seconds, and the twelve
///   before it are night going to twilight, which is the half of a sunrise nobody films.
///   Twelve is also when her headsail is set: she is handed whatever day the seed drew,
///   which can be force seven with three slabs tied in and the genoa rolled away, and
///   shaking that out takes the reef line ten seconds and the drum twelve. So the sail
///   is up exactly as the sun is, and both were timed rather than hoped for.
///
/// * **The rounding, forty-two seconds, and it is the long one for one reason: the
///   furling drum.** A headsail change is two full turns at 0.085 a second — twelve
///   seconds to roll the old one away and twelve to set the new — so the kite ordered at
///   thirty-two is not drawing until fifty-six whatever anybody does. Laid against that:
///   sixteen seconds of approach on the layline with the mark coming up from forty
///   metres, the bear-away at forty with the genoa half rolled, the kite going up out of
///   the turn the way it does in a race, full and pulling at fifty-six. That is **three
///   shots** — the approach, the rounding, and the kite from the cockpit — which is why
///   the scene is worth its length.
///
/// * **The brig in fog, and then snugging down: twenty-four seconds.** Fog is weather
///   and will not survive a breeze, so this is a calm: force two, where none of it is
///   blown away. At six oktas the visibility works out at about four hundred and fifty
///   metres and she is laid at two hundred and sixty — close enough to be a ship, far
///   enough to be a shape. Eight seconds of that is the shot. The sixteen after it are
///   canvas coming off, and they are sixteen because eight was tried and measured: the
///   drum takes twelve to roll a kite away and the reef line nine to run three slabs
///   out, and force eight arrived with a third of a gennaker still flying. It held that
///   time. A take that depends on holding is a take with a retake in it.
///
/// * **The gale, twenty-four seconds.** The wind sea is settled rather than built, so it
///   is there the moment it is asked for — but the shot wants a boat *working*, and that
///   is three or four waves end to end. Two seats, eight and sixteen: the chase, which
///   pulls back as the sea gets up and is the only view that shows a six-metre swell as
///   six metres, and astern, where the wake tilts as she lies over.
///
/// * **Sunset at the platform, twenty-four seconds.** Sunset is 18:31, so 17:50 at five
///   hundred and twenty puts the sun on the horizon about four and a half seconds in.
///   Then the clock drops to a hundred and twenty, because the colour after the sun has
///   gone is half an hour of real evening and running that at five hundred throws it
///   away in two seconds. The last eight of the twenty-four are hers rather than the
///   sky's: the gale's three slabs and the rolled headsail are shaken out and unrolled
///   at the top of the scene and take until about a hundred and twenty-seven to run.
///
/// # And the sound
///
/// The mix is set in the first beat and never touched again: the sea at a twentieth, the
/// boat at full, the music off. **The sea is this program's loudest voice and the worst
/// thing to put under a cut** — it is broadband, it never stops, and it buries the winch,
/// the sheet and the water along the topsides, which are the sounds that say a boat is a
/// machine being worked. The music is off because it is written to the cut afterwards,
/// against these scenes' own shape.
pub const DEMO: &[(f64, Act)] = &[
    // ---- CALM SUNRISE, 0 to 24 ----------------------------------------------------
    (
        0.0,
        Act::Mix {
            master: 0.9,
            sea: 0.05,
            boat: 1.0,
            // Up, because a demo is where the watch is worth hearing: a call is the
            // one sound in the film with a person in it.
            crew: 0.9,
            music: 0.0,
        },
    ),
    (
        0.1,
        Act::Furniture {
            console: false,
            chart: Chart::Away,
        },
    ),
    (
        0.2,
        Act::Weather {
            force: 2,
            oktas: 1,
            hour: Some(3.75),
            fetch: 6.0,
        },
    ),
    (0.3, Act::Seat(View::Chase)),
    // Close reach, so the low sun comes through the sail from the side rather than
    // behind it: a backlit genoa is a white shape and a side-lit one has cloth in it.
    (0.4, Act::Steer(60.0)),
    (0.5, Act::Clock(520.0)),
    // **The day she was given is not the day this is.** A drawn morning can open with
    // three slabs already tied in and the genoa rolled away — which is right for force
    // seven and is a boat with no sails up in force two. Shaken out and unrolled in the
    // first second, both at the pace of their own lines: the reef takes ten seconds and
    // the drum twelve, which is why they are said here and not at the shot that wants
    // them.
    (0.6, Act::Reef(0)),
    (0.7, Act::Furl(0.0)),
    // **And the fog off, which has to be said.** A drawn day can have a bank of it in
    // already — the synoptic draws fog the way it draws cloud — and seed 19 opens with
    // half an okta of visibility and a white sunrise. The fog is a scene in this reel
    // and therefore has to be a thing the reel owns from the first second.
    (0.8, Act::Fog(0.0)),
    // ---- THE ROUNDING, FORCE 4, 24 to 66 ------------------------------------------
    (24.0, Act::Clock(1.0)),
    (
        24.1,
        Act::Weather {
            force: 4,
            oktas: 3,
            // Mid-morning rather than the hour the sunrise left behind. The demo walks
            // through one day — dawn, morning, fog, noon, evening — and the alternative
            // is five scenes in the same flat early light, which is what the first take
            // came back as.
            hour: Some(9.5),
            fetch: 40.0,
        },
    ),
    (24.2, Act::Steer(45.0)),
    (24.3, Act::Buoy(40.0)),
    // The kite is ordered a shot early, as every order in every reel here is: the drum
    // takes twenty-four seconds and the shot it belongs to is eight long. She is under
    // genoa by now, so this is a real change and not a bare stay being dressed.
    (32.0, Act::Headsail(sail_core::state::Headsail::Gennaker)),
    // The bear-away. She has had sixteen seconds of a freshening breeze to cover the
    // forty metres, which puts the mark close aboard and about to go astern.
    (40.0, Act::Steer(135.0)),
    (40.1, Act::Orbit(4.0)),
    (48.0, Act::Orbit(0.0)),
    // The course comes up as the rounding shot ends, not at the fog: the fleet keeps a
    // boat for six seconds after it stops hearing from her, so taking the buoys away at
    // the fog scene left two yachts ghosting about in it next to a brig.
    (48.1, Act::Buoy(0.0)),
    // And the kite from the cockpit, which is where somebody standing on this boat
    // would be looking at it from.
    (56.0, Act::Seat(View::Cockpit)),
    // ---- THE BRIG IN FOG, THEN SNUGGING DOWN, 66 to 90 ----------------------------
    (
        66.0,
        Act::Weather {
            force: 2,
            oktas: 6,
            hour: None,
            fetch: 10.0,
        },
    ),
    (66.1, Act::Fog(0.62)),
    // **No change of course in this scene, and that is not laziness.** The brig is laid
    // on a bearing off the bow at the moment she is placed; swing the boat afterwards
    // and the bearing swings with it, which put her out of the frame entirely the first
    // time this was filmed. She keeps the heading she came out of the rounding on.
    (66.3, Act::Brig(260.0)),
    (66.5, Act::Seat(View::Chase)),
    // Making ready, and the gale is sixteen seconds away. Canvas comes off *before* it
    // arrives, which is both how it is done and the only way a reel gets storm canvas
    // instead of a blown-out sail — and sixteen is not eight because eight was measured
    // and was not enough: the drum takes twelve to roll a kite away and the reef line
    // nine to run out three slabs, and force eight arrived with a third of a gennaker
    // still flying. It held, that time.
    (74.0, Act::Furl(1.0)),
    (74.1, Act::Reef(3)),
    // ---- THE GALE ON THE OCEAN, 90 to 114 -----------------------------------------
    (90.0, Act::Fog(0.0)),
    (
        90.1,
        Act::Weather {
            force: 8,
            oktas: 7,
            // Noon, and high cloud rather than a dark one: a gale an hour after dawn is
            // a grey rectangle. The light is what makes a sea look big.
            hour: Some(11.5),
            fetch: 400.0,
        },
    ),
    // Thirty degrees off the wind, which is where an ocean swell sits relative to the
    // gale that is blowing now: it was made by the same system a day and a thousand
    // miles ago, and it has not turned with it.
    (
        90.2,
        Act::Swell {
            hs: 6.5,
            tp: 14.0,
            from: 30.0,
        },
    ),
    // Running off, which is what she is doing out here and what keeps her on her feet.
    (90.3, Act::Steer(145.0)),
    (90.4, Act::Staysail(true)),
    // The brig goes: a square-rigger in a force eight ocean is a different film and a
    // promise this game has not made.
    (90.5, Act::Brig(0.0)),
    // **The cockpit and not the seat facing aft**, which was tried and filmed the
    // inside of a wave: from the after end of the cockpit looking down the wake, a
    // following sea at force eight is directly in front of the lens and the shot is
    // forty feet of green water with no boat in it. True, and unreadable. From the
    // cockpit looking forward you get the thing instead — the bow going in, the spray
    // coming aft, the mast against a moving horizon.
    (98.0, Act::Seat(View::Cockpit)),
    (106.0, Act::Seat(View::Chase)),
    // ---- SUNSET AT THE PLATFORM, 114 to 138 ---------------------------------------
    (
        114.0,
        Act::Weather {
            force: 3,
            oktas: 2,
            hour: Some(17.83),
            fetch: 15.0,
        },
    ),
    (
        114.1,
        Act::Swell {
            hs: 0.0,
            tp: 12.0,
            from: 0.0,
        },
    ),
    (114.2, Act::Staysail(false)),
    (114.3, Act::Home),
    (114.4, Act::Steer(120.0)),
    // Her evening clothes back on: the gale's three slabs shaken out and the headsail
    // unrolled. Both run at the pace of their own lines — nine seconds and twelve — so
    // they are said at the top of the scene and she is dressed by about a hundred and
    // twenty-seven, which is why the shot to cut is the one after the sun has gone and
    // not the one it goes in.
    (114.5, Act::Furl(0.0)),
    (114.6, Act::Reef(0)),
    (114.7, Act::Clock(520.0)),
    // The sun is down at about a hundred and nineteen. What follows is the part worth
    // having, so the clock comes off five hundred and on to a hundred and twenty —
    // half an hour of afterglow across the last sixteen seconds instead of three hours
    // of night. Sixteen and not eight because she is still dressing: the drum and the
    // reef line were started at the top of the scene and she is not in her evening
    // clothes until about a hundred and twenty-seven.
    (122.0, Act::Clock(120.0)),
];

/// How long each runs before it says so.
pub const PASSAGE_RUNS_FOR: f64 = 102.0;
pub const UNDER_WAY_RUNS_FOR: f64 = 93.0;
pub const DEMO_RUNS_FOR: f64 = 138.0;

impl Which {
    pub fn beats(self) -> &'static [(f64, Act)] {
        match self {
            Which::Passage => PASSAGE,
            Which::UnderWay => UNDER_WAY,
            Which::Demo => DEMO,
        }
    }

    pub fn runs_for(self) -> f64 {
        match self {
            Which::Passage => PASSAGE_RUNS_FOR,
            Which::UnderWay => UNDER_WAY_RUNS_FOR,
            Which::Demo => DEMO_RUNS_FOR,
        }
    }
}

/// Whether a reel was asked for, and which.
#[derive(Resource, Clone, Copy, Default)]
pub struct Reel {
    pub on: bool,
    pub which: Which,
    /// Seconds of quiet before the first beat — `SAIL_REEL_AFTER`.
    ///
    /// **For the hand that is holding the stopwatch.** Somebody recording this has a
    /// window to find, a screen recorder to point at it and a mouse to get out of the
    /// frame, and all of that happens while the reel is already playing its opening shot
    /// at them. Thirty seconds of her simply sailing, before anything is staged, is the
    /// difference between a take and a retake.
    ///
    /// It also buys the only way to photograph the early scenes from inside the program:
    /// the capture cannot produce a picture for the first forty seconds or so of a run —
    /// the same limitation `dist/store.sh` works around by waiting seventy — so a reel
    /// that starts at zero has its first four scenes in a part of the run the game cannot
    /// photograph. Delayed, they can be checked like any other.
    pub from: f64,
    done: usize,
    said: bool,
}

impl Reel {
    pub fn asked() -> Self {
        let (on, which) = match crate::knob::var("SAIL_REEL").as_deref() {
            Ok("0") | Err(_) => (false, Which::Passage),
            Ok("short") | Ok("2") | Ok("under-way") => (true, Which::UnderWay),
            Ok("demo") | Ok("3") => (true, Which::Demo),
            Ok(_) => (true, Which::Passage),
        };
        Self {
            on,
            which,
            from: crate::knob::var("SAIL_REEL_AFTER")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0),
            done: 0,
            said: false,
        }
    }
}

/// Everything a beat can reach, behind one name: a system may have sixteen parameters
/// and this one wanted more.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Stage<'w> {
    pub sky: ResMut<'w, crate::sky::Sky>,
    pub clock: ResMut<'w, crate::sky::Clock>,
    pub view: ResMut<'w, View>,
    pub sweep: ResMut<'w, crate::camera::Sweep>,
    pub ink: ResMut<'w, crate::world::Ink>,
    pub hud: ResMut<'w, crate::hud::HudVisible>,
    pub console: ResMut<'w, crate::panel::Console>,
    pub chart: ResMut<'w, crate::chart::Chart>,
    pub hoist: ResMut<'w, crate::signals::Hoist>,
    pub berth: ResMut<'w, crate::anchorage::Berth>,
    pub pilot: ResMut<'w, crate::input::Pilot>,
    pub orders: ResMut<'w, crate::orders::Orders>,
    pub bank: ResMut<'w, crate::clutch::Bank>,
    pub props: Props<'w>,
}

/// The rest of the stage, behind a second name.
///
/// Not a second idea, a second *box*: a Bevy system may have sixteen parameters, a
/// `SystemParam` built out of fields is counted the same way, and `Stage` was at
/// thirteen before the demo wanted three more. Nesting one inside the other costs one
/// field and nothing else.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Props<'w> {
    pub mix: ResMut<'w, crate::settings::Settings>,
    pub race: ResMut<'w, crate::race::Race>,
    pub brig: ResMut<'w, crate::ship::Brig>,
}

/// Run the timetable.
pub fn run(mut reel: ResMut<Reel>, mut sim: ResMut<Sim>, mut stage: Stage) {
    if !reel.on {
        return;
    }
    let beats = reel.which.beats();
    // Everything below is said in reel time, which is the clock the cut is made against;
    // `from` is the only place it and the boat's clock are allowed to differ.
    let now = sim.sim.state.time - reel.from;
    while reel.done < beats.len() && beats[reel.done].0 <= now {
        let (at, act) = beats[reel.done];
        reel.done += 1;
        play(act, &mut sim, &mut stage);
        // The cloth that is actually up, said at every beat. Whoever is cutting the film
        // needs to know what was on the bow in each shot, and a reel that *orders* a
        // gennaker is not the same statement as a boat that *has* one — the order takes
        // twenty-four seconds and the log is the only place the difference shows.
        info!(
            "reel: {at:>5.1} s — {act:?}  [{} {:.0}% out, reef {:.0}%]",
            sim.controls.headsail.name(),
            (1.0 - sim.controls.furl) * 100.0,
            sim.controls.reef * 100.0,
        );
    }
    let runs_for = reel.which.runs_for();
    if !reel.said && now > runs_for {
        reel.said = true;
        info!("reel: that is the lot — {runs_for:.0} s");
    }
}

fn play(act: Act, sim: &mut Sim, stage: &mut Stage) {
    match act {
        Act::Weather {
            force,
            oktas,
            hour,
            fetch,
        } => {
            // The same two resources the weather page writes, and in the same way: this
            // is a hand on the sliders, not a back door into the model.
            //
            // **Worked out once and used twice**, which is the whole of the fix. The sea
            // used to be settled against `telemetry.tws` — the wind as it was *last*
            // frame, because telemetry is a frame behind the resource it is read from.
            // So a beat that put force two on the water and then force eight settled the
            // gale's sea at the calm's wind and left it to build the rest by itself,
            // which takes hours. Filmed, that is a boat lying over to a gale on water
            // out of a different afternoon: the one shot in the demo that was supposed
            // to be about the size of the sea, and the sea was not in it.
            let wanted = sail_core::units::kn_to_ms(crate::weather::force_wind(force));
            if let Some(wind) = sim.sim.wind.adjustable() {
                // `blows` rather than the base alone — see its own note. A reel that sets
                // force 7 and gets force 4 for the next half hour films the wrong day.
                wind.synoptic.blows(wanted);
                wind.synoptic.start_cover(oktas as f64 / 8.0);
            }
            if let Some(sea) = sim.sim.water.adjustable() {
                sea.set_fetch(fetch * 1000.0);
                // Settled rather than built from nothing: a trailer cannot wait half an
                // hour for a gale to raise its own sea.
                sea.settle(wanted, 0.0);
            }
            // The hour moves the clock's own offset rather than the sky, because the sky
            // is derived from the passage now and would be back where it was next frame.
            // Asking for no hour leaves it where the passage has it, which is what lets a
            // beat change the wind in the middle of a sunrise without dragging the sun
            // back to where the last beat put it.
            if let Some(want) = hour {
                stage.clock.put_at(want, stage.sky.hour);
            }
            *stage.sky = crate::sky::Sky::at(stage.sky.hour, oktas as f32 / 8.0);
        }
        Act::Seat(seat) => {
            *stage.view = seat;
        }
        Act::Orbit(rate) => {
            stage.sweep.0 = (rate > 0.0).then_some(rate.to_radians());
        }
        Act::Furniture { console, chart } => {
            stage.console.hidden = !console;
            stage.hud.0 = false;
            stage.chart.open = chart != Chart::Away;
            stage.chart.spread = chart == Chart::Table;
        }
        Act::Ink(on) => stage.ink.0 = on,
        Act::Hoist(flag) => {
            stage.hoist.flying = Some(flag);
            stage.hoist.since = sim.sim.state.time;
        }
        Act::Home => {
            // Fine on the port bow, where the drawing knob puts it — see `anchorage`.
            let st = &sim.sim.state;
            let heading = st.heading - 0.6;
            stage.berth.at = Some(st.pos + glam::DVec2::new(heading.sin(), heading.cos()) * 70.0);
            stage.berth.harbour = "alongside".into();
        }
        Act::Clock(rate) => stage.clock.rate = rate,
        Act::Headsail(which) => stage.orders.set(sim, which),
        Act::Reef(slab) => {
            use crate::clutch::Line;
            let which = Line::Reef.on_the_bank();
            if let Some(stop) = Line::Reef
                .detents(&sim.controls)
                .and_then(|stops| stops.get(slab))
            {
                stage.bank.target[which] = Some(*stop);
            }
        }
        Act::Mix {
            master,
            sea,
            boat,
            crew,
            music,
        } => {
            // The same resource the sound page writes, which is read into the mixer
            // every frame by `settings::apply`. A reel that reached past it into the
            // audio thread would be overwritten before the next beat.
            stage.props.mix.volume = master;
            stage.props.mix.sea = sea;
            stage.props.mix.boat = boat;
            stage.props.mix.crew = crew;
            stage.props.mix.music = music;
        }
        Act::Fog(thickness) => {
            if let Some(wind) = sim.sim.wind.adjustable() {
                wind.synoptic.start_fog(thickness);
            }
        }
        Act::Swell { hs, tp, from } => {
            if let Some(sea) = sim.sim.water.adjustable() {
                // **Relative to the wind, not to north.** A reel is written before the
                // day is drawn and has no idea which way the wind will be blowing, so a
                // compass bearing here would put the swell dead ahead in one world and
                // dead astern in the next. Off the wind by so many degrees is the thing
                // that was meant anyway: a swell made by yesterday's system lies across
                // today's gale, and *across* is the picture.
                let twd = sim.sim.telemetry.twd;
                let run = twd + sail_core::units::rad(from) + std::f64::consts::PI;
                sea.set_swell(
                    (hs > 0.0).then_some(sail_core::water::Swell { hs, tp, run }),
                    sim.sim.state.time,
                );
            }
        }
        Act::Buoy(range) => {
            let race = &mut stage.props.race;
            if range <= 0.0 {
                **race = crate::race::Race::default();
                return;
            }
            let st = &sim.sim.state;
            let ahead = glam::DVec2::new(st.heading.sin(), st.heading.cos());
            let across = glam::DVec2::new(ahead.y, -ahead.x);
            let mark = st.pos + ahead * range;
            // A line astern, where a start line is, so that the wide shots have one in
            // them. Nobody crosses it inside the scene; it is scenery, and it is the
            // thing that says this is a race and not a navigation exercise.
            race.line = (
                st.pos - ahead * 300.0 - across * 70.0,
                st.pos - ahead * 300.0 + across * 70.0,
            );
            race.marks = vec![mark];
            race.at = 0;
            race.stage = crate::race::Stage::Racing;
            // And two boats already racing for the same mark, laid where they are
            // converging on it rather than where a fleet would be before a gun: both
            // ahead and across, inside the camera's field. A buoy on its own is a
            // training exercise.
            //
            // **There were three, and the third was astern of us.** Reported from the
            // deck in one line: the boat that comes at us during the sail change. It
            // was fifty-five metres out on the quarter and sailing for the same water,
            // so it closed through the whole shot and arrived in the middle of the one
            // moment the camera is pointed somewhere else. Two boats ahead read as a
            // fleet; one astern reads as a collision about to happen.
            race.rivals = ["Varg", "Tern"]
                .into_iter()
                .zip([(38.0, -46.0), (62.0, -78.0)])
                .map(|(name, (abeam, astern))| crate::race::Rival {
                    name,
                    at: st.pos + across * abeam - ahead * astern,
                    heading: st.heading,
                    speed: sim.sim.telemetry.stw.max(2.0),
                    way: glam::DVec2::ZERO,
                    at_mark: 0,
                    started: true,
                    finished: None,
                    skill: 0.94,
                })
                .collect();
        }
        Act::Brig(range) => {
            let brig = &mut stage.props.brig;
            if range <= 0.0 {
                brig.afloat = false;
                return;
            }
            let st = &sim.sim.state;
            // Twenty-five degrees off the bow: far enough out that our own mainsail is
            // not standing in front of her, near enough in that she is inside the
            // camera's field, which is narrower than it looks. The same argument as
            // `SAIL_BRIG_SHOW`, which learned it the hard way at thirty-eight.
            let off = st.heading - sail_core::units::rad(25.0);
            brig.at = st.pos + glam::DVec2::new(off.sin(), off.cos()) * range;
            // Sailing our way rather than pinned to our bearing: she is handed a mark
            // five miles ahead and left to get on with it, which is why she is not
            // sliding sideways across the horizon the way a pinned ship does.
            brig.heading = st.heading;
            brig.bound_for =
                brig.at + glam::DVec2::new(st.heading.sin(), st.heading.cos()) * 9_000.0;
            brig.speed = sail_core::units::kn_to_ms(4.5);
            brig.way = glam::DVec2::new(st.heading.sin(), st.heading.cos()) * brig.speed;
            brig.set = [1.0, 1.0, 1.0];
            brig.afloat = true;
        }
        Act::Furl(how_far) => {
            stage.bank.target[crate::clutch::Line::Furl.on_the_bank()] =
                Some(how_far.clamp(0.0, 1.0));
        }
        Act::Staysail(up) => {
            stage.bank.target[crate::clutch::Line::Staysail.on_the_bank()] =
                Some(if up { 1.0 } else { 0.0 });
        }
        Act::Steer(twa) => {
            // Turning to starboard by δ swings the wind δ further aft on the port side,
            // which is to say the true wind angle *falls* by δ. So the course that gives
            // the angle wanted is the course she is on plus the difference the other way
            // round. Said once here rather than guessed at each beat.
            let st = &sim.sim.state;
            let want = twa.to_radians();
            let turn = sim.sim.telemetry.twa - want;
            stage.pilot.inner.mode = sail_core::autopilot::PilotMode::Heading(
                sail_core::units::wrap_tau(st.heading + turn),
            );
        }
    }
}

pub fn plugin(app: &mut App) {
    use crate::stages::Deck;
    app.insert_resource(Reel::asked());
    app.add_systems(Update, run.in_set(Deck::Weather));
}

#[cfg(test)]
mod the_running_order {
    use super::*;

    /// **A timetable runs forwards.** Out of order it would play the gale before the
    /// breeze and the beats would fire in one lump, because the loop that runs them takes
    /// everything whose moment has passed.
    #[test]
    fn the_beats_are_in_order_and_inside_the_run() {
        for which in [Which::Passage, Which::UnderWay, Which::Demo] {
            let mut last = -1.0;
            for (at, _) in which.beats() {
                assert!(
                    *at >= last,
                    "{which:?}: a beat at {at} s comes after {last}"
                );
                assert!(
                    *at < which.runs_for(),
                    "{which:?}: a beat at {at} s is after the reel has ended"
                );
                last = *at;
            }
        }
    }

    /// And every scene has room to be cut out of: six seconds in the middle, away from
    /// the moment the weather jumps.
    #[test]
    fn every_scene_is_long_enough_to_cut_from() {
        for which in [Which::Passage, Which::UnderWay, Which::Demo] {
            let beats = which.beats();
            // Beats within a second and a half of an earlier one are that one's scene
            // being staged — the weather, the furniture, the seat and the course all
            // change together — so only the gaps that are a *scene* are measured. By
            // index rather than by value, so that two beats at the same moment are one
            // scene and not a scene of no length.
            let moments: Vec<f64> = beats
                .iter()
                .enumerate()
                .filter(|(i, (at, _))| {
                    beats[..*i]
                        .iter()
                        .all(|(other, _)| (at - other).abs() > 1.5)
                })
                .map(|(_, (at, _))| *at)
                .collect();
            for pair in moments.windows(2) {
                assert!(
                    pair[1] - pair[0] >= 8.0,
                    "{which:?}: a scene from {:.1} to {:.1} is {:.1} s, which is not a shot",
                    pair[0],
                    pair[1],
                    pair[1] - pair[0]
                );
            }
            // And the last scene gets the same room before the reel says it is done.
            let last = *moments.last().expect("a reel has beats in it");
            assert!(
                which.runs_for() - last >= 8.0,
                "{which:?}: the last scene gets {:.1} s",
                which.runs_for() - last
            );
        }
    }

    /// **The demo is filmed with a microphone as well as a camera**, so the mix is part
    /// of the timetable and not part of whatever the machine happened to be set to. It
    /// has to be the first thing said — a demo whose first eight seconds are at the
    /// player's own volume is eight seconds that cannot be used — and the sea has to
    /// stay down, because the sea buries every sound a cut is made of.
    #[test]
    fn the_demo_sets_its_own_mix_before_anything_else() {
        let (at, first) = DEMO[0];
        let Act::Mix { sea, music, .. } = first else {
            panic!("the demo opens with {first:?} rather than with the mix");
        };
        assert_eq!(
            at, 0.0,
            "the mix is set at {at} s, after the take has begun"
        );
        assert!(sea <= 0.1, "the sea is at {sea}, which will bury the gear");
        assert_eq!(
            music, 0.0,
            "the music is written to the cut, not to the take"
        );
        for (at, act) in &DEMO[1..] {
            assert!(
                !matches!(act, Act::Mix { .. }),
                "{at} s moves the mix in the middle of a take"
            );
        }
    }

    /// **Canvas comes off before the gale and not during it.** The reel can put force
    /// eight on the screen at a stated second; the drum and the reef line cannot be
    /// hurried, so the order to shorten has to be given with the wind still light. Get
    /// this backwards and the take ends with a blown-out sail, which is a retake.
    #[test]
    fn the_demo_shortens_sail_before_the_wind_arrives() {
        let gale = DEMO
            .iter()
            .filter(|(_, act)| matches!(act, Act::Weather { force, .. } if *force >= 7))
            .map(|(at, _)| *at)
            .min_by(f64::total_cmp)
            .expect("the demo has a gale in it");
        // Taking canvas *off*, and before the wind: a reef shaken out in the evening is
        // the opposite instruction and is not what this is about.
        let shortened = DEMO
            .iter()
            .filter(|(at, act)| {
                *at <= gale
                    && (matches!(act, Act::Furl(x) if *x > 0.5)
                        || matches!(act, Act::Reef(slab) if *slab > 0))
            })
            .map(|(at, _)| *at)
            .max_by(f64::total_cmp)
            .expect("the demo takes canvas off her somewhere before the gale");
        assert!(
            gale - shortened >= 6.0,
            "she is shortened down at {shortened} s and the gale is at {gale}"
        );
    }

    /// **The short one is the one with no furniture in it**, which is the whole reason it
    /// exists: it is a boat sailing, for somebody who has never seen the game. A panel
    /// that crept back in would not fail anything else here.
    #[test]
    fn nothing_is_on_the_screen_in_the_short_one() {
        for (at, act) in UNDER_WAY {
            match act {
                Act::Furniture { console, chart } => assert!(
                    !console && *chart == Chart::Away,
                    "{at} s puts the furniture back on screen"
                ),
                Act::Hoist(_) | Act::Home | Act::Ink(_) => {
                    panic!("{at} s is story, and this reel has none")
                }
                _ => {}
            }
        }
    }
}
