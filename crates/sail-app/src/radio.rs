//! The VHF set: a channel, a watch on it, and whoever else is within the horizon.
//!
//! # Why a radio belongs in this game and not in a menu
//!
//! Everything else aboard is a thing you operate badly until you learn to operate it
//! well, and a VHF is the purest example on the boat: it is *procedure*, nothing else.
//! There is no skill in pressing the button. The skill is knowing which channel, in what
//! order the names go, when to shut up and listen, and that "over" and "out" are not the
//! same word. That is the same shape as reefing early and taking a bearing while you
//! still can — a thing a person does right or does late.
//!
//! And it is the one instrument aboard that brings *other people* into a single-handed
//! passage. The flags in `signals.rs` already do that for the brig, with a hoist, a
//! delay while she reads it and an answer; the radio is the same protocol a hundred and
//! fifty years later, and the module it joins already has a field called `said`.
//!
//! # Range is the horizon, which this game already computes
//!
//! A VHF is line of sight. Not approximately — *that is the whole of its range*: the
//! aerial at the masthead sees the other aerial or it does not, and the sea curves away
//! in between. So the range of a transmission is the same arithmetic as the range a
//! lighthouse is raised at, with one number changed: radio waves bend a little more in
//! the atmosphere than light does, so the constant is 2.5 rather than 2.08.
//!
//! What falls out of that is the thing a sailor knows and nobody tells you: **a coast
//! station is enormously further than another yacht**. Her aerial is fifteen metres up
//! and so is yours, which is ten miles between you; the station's is a hundred and
//! eighty metres up on a hill, which is forty. You can hear the harbour and not the boat
//! three miles the other side of the island — and that is not a rule written down here,
//! it is two square roots.
//!
//! # No voice, on purpose
//!
//! What comes out of the speaker is the squelch breaking, the carrier, and the squelch
//! closing. The words are read. That is what a radio with nobody speaking actually is,
//! and it is a great deal more honest than a murmur pretending to be speech — see
//! `music.rs` for the same argument about a recording that cannot know what the wind is
//! doing. Transmitting is a menu of the phrases the procedure allows, which is not a
//! simplification: VHF procedure *is* a closed grammar, and a menu of it is a syllabus.

use bevy::prelude::*;

use crate::sim::Sim;

/// The set, and what is on it.
pub fn plugin(app: &mut App) {
    use crate::stages::Deck;
    app.init_resource::<Radio>();
    app.init_resource::<Coastguard>();
    app.init_resource::<PortControl>();
    app.add_systems(
        Update,
        (broadcast, harbour, listen).chain().in_set(Deck::Weather),
    );
}

/// How far a transmission carries, metres, between two aerials at these heights.
///
/// Two point five rather than the 2.08 of `Landmark::visible_from`: radio refracts a
/// little more than light in the same air, which is why the radio horizon is about a
/// fifth further than the visible one. The same two square roots otherwise.
pub fn carries(ours: f64, theirs: f64) -> f64 {
    const NM: f64 = 1_852.0;
    2.5 * (ours.max(0.0).sqrt() + theirs.max(0.0).sqrt()) * NM
}

/// Where this boat's aerial is: the masthead, which is where it always is.
pub const AERIAL: f64 = 16.0;

/// And a coast station's, on a hill behind the harbour.
///
/// A hundred and eighty metres is an ordinary site for one. It is why the harbour can
/// hear you from forty miles and the yacht two miles behind the island cannot.
pub const STATION_AERIAL: f64 = 180.0;

/// A harbour office's aerial, on the roof above the fairway.
///
/// Forty metres rather than the coastguard's hundred and eighty: a port talks to the
/// water it can see, and the difference shows — you hear the harbour when you are in its
/// approach and the coastguard from half the sea away.
pub const HARBOUR_AERIAL: f64 = 40.0;

/// And a square-rigger's, at the main truck.
pub const BRIG_AERIAL: f64 = 28.0;

/// The calling and distress channel, which is watched and not worked.
pub const CALLING: u8 = 16;

/// Port operations, which is where a harbour works.
pub const PORT: u8 = 12;

/// Ship to ship — the one a pilot and the vessel he is leading go to.
pub const WORKING: u8 = 72;

/// The channels this set has. Sixteen, the two that harbours work, and a ship-to-ship.
///
/// A real set has fifty-odd and the difference matters to nobody: what a sailor uses is
/// sixteen to call, a working channel to talk, and one between boats. A list of four is
/// the syllabus rather than the catalogue.
pub const CHANNELS: [u8; 4] = [16, 6, 12, 72];

/// What each one is for, which is the half of it anybody actually has to learn.
pub fn what_for(channel: u8) -> &'static str {
    match channel {
        16 => "calling and distress — watch it, do not work it",
        6 => "ship to ship, safety",
        12 => "port operations",
        72 => "ship to ship",
        _ => "",
    }
}

/// Somebody saying something, on a channel, from somewhere.
#[derive(Clone, Debug)]
pub struct Transmission {
    /// Who is speaking, as they identify themselves.
    pub from: String,
    pub channel: u8,
    pub words: String,
    /// Where the aerial is, and how high — so the range can be worked out rather than
    /// assumed. `None` for our own set.
    pub at: Option<(glam::DVec2, f64)>,
    /// When it began and how long it takes to say.
    pub began: f64,
    pub takes: f64,
    /// Whether it is worth writing in the book.
    ///
    /// **A navigator copies the forecast down and does not copy down the harbour saying
    /// come alongside the fuel berth.** The set keeps everything it heard, because that
    /// is what a set does; the deck log keeps what somebody decided to write, because
    /// that is what a book is. Flagged by whoever puts it on the air rather than worked
    /// out at the page, since the station knows what it is sending and the page does
    /// not.
    pub worth_writing: bool,
}

impl Transmission {
    /// How long a line takes to say at the measured pace of radio procedure.
    ///
    /// **A hundred and forty words a minute**, which is slower than conversation and is
    /// the pace anybody falls into on a radio because the other end is writing it down.
    /// It matters because the channel is *occupied* while it is being said: two stations
    /// transmitting at once is two stations heard by nobody, and that is the one rule of
    /// a shared channel.
    pub fn saying(
        from: &str,
        channel: u8,
        words: &str,
        at: Option<(glam::DVec2, f64)>,
        now: f64,
    ) -> Self {
        let count = words.split_whitespace().count().max(1) as f64;
        Self {
            from: from.to_string(),
            channel,
            words: words.to_string(),
            at,
            began: now,
            takes: 1.2 + count * 60.0 / 140.0,
            worth_writing: false,
        }
    }

    /// Mark it as one for the book.
    pub fn for_the_book(mut self) -> Self {
        self.worth_writing = true;
        self
    }

    pub fn over_at(&self) -> f64 {
        self.began + self.takes
    }
}

/// One line in the log, as it was heard.
#[derive(Clone, Debug)]
pub struct Heard {
    pub channel: u8,
    pub from: String,
    pub words: String,
    /// How strong it was, 0 to 1 — a transmission at the edge of the horizon is broken
    /// and a strong one is not.
    pub strength: f32,
    /// Ship's time when it was heard, in hours.
    ///
    /// **A radio log is a list of times**, which is the whole reason anybody keeps one:
    /// a position report, a securité and a call you missed are only useful against the
    /// clock — and the clock is the one on the bulkhead, not a stopwatch started at the
    /// dock. It was seconds into the passage at first, and the console printed 0000
    /// against a five-twenty broadcast.
    pub hour: f32,
}

#[derive(Resource)]
pub struct Radio {
    pub on: bool,
    /// The channel being worked. Sixteen is watched separately — see `dual`.
    pub channel: u8,
    /// **Dual watch**, which is the one feature of a VHF nobody explains and everybody
    /// uses: the set listens to your working channel and flicks to sixteen between
    /// words, so you can be talking to the harbour and still hear a distress call.
    pub dual: bool,
    /// Where the squelch is set, 0 wide open to 1 shut right down.
    ///
    /// Open it and you hear the hiss of an empty channel; shut it and you hear nothing
    /// until something is strong enough to break it. Set too far shut and you will not
    /// hear the weak station you are waiting for, which is the mistake everybody makes
    /// once.
    pub squelch: f32,
    /// What is coming out of the speaker now.
    pub traffic: Option<Transmission>,
    /// And what we are saying, which stops us hearing anything else.
    pub transmitting: Option<Transmission>,
    /// The last few things heard, newest last.
    pub log: Vec<Heard>,
    /// Waiting to be put on the air by whoever generates it.
    pub air: Vec<Transmission>,
}

impl Default for Radio {
    fn default() -> Self {
        Self {
            // On, and on sixteen. A set that opens switched off is a set the player
            // never learns exists, and a boat at sea with the radio off is a boat whose
            // skipper has made a decision nobody asked them to make.
            on: true,
            channel: CALLING,
            dual: true,
            // Where a set is left: shut just far enough that an empty channel is quiet.
            squelch: 0.45,
            traffic: None,
            transmitting: None,
            log: Vec::new(),
            air: Vec::new(),
        }
    }
}

/// The most the log keeps. A radio log is a page, not an archive.
const KEEPS: usize = 24;

impl Radio {
    /// Put something on the air, from wherever it is being sent.
    pub fn broadcast(&mut self, transmission: Transmission) {
        self.air.push(transmission);
    }

    /// Whether this set would hear that, and how strongly.
    ///
    /// Nought if it is out of range, on another channel, or while we are transmitting —
    /// a set cannot hear while its own carrier is up, which is why "over" exists.
    pub fn hears(&self, t: &Transmission, from_here: f64) -> f32 {
        if !self.on || self.transmitting.is_some() {
            return 0.0;
        }
        if t.channel != self.channel && !(self.dual && t.channel == CALLING) {
            return 0.0;
        }
        let Some((_, height)) = t.at else {
            return 1.0;
        };
        let range = carries(AERIAL, height);
        if from_here > range {
            return 0.0;
        }
        // Strong until the last fifth of the range, then falling away: a VHF does not
        // fade gently with distance the way an eye does, it works and then it breaks up.
        let edge = (1.0 - (from_here / range).powf(6.0)).clamp(0.0, 1.0) as f32;
        // **And then the squelch, which is the point of having one.** It is a gate on
        // signal strength: shut it down and the hiss goes, and so does the weak station
        // you were waiting for. Everybody makes that mistake once, and a squelch that
        // did not gate anything would be a knob that only changed the hiss — which is
        // the half of it that is decoration.
        if edge < self.squelch * 0.9 {
            return 0.0;
        }
        edge
    }
}

/// Work the set: put what is on the air into the speaker, and remember it.
pub fn listen(
    sim: Res<Sim>,
    sky: Res<crate::sky::Sky>,
    mut log: ResMut<crate::chart::DeckLog>,
    mut radio: ResMut<Radio>,
) {
    let now = sim.sim.state.time;
    let here = sim.sim.state.pos;

    // Our own transmission holds the channel until it is finished.
    if radio
        .transmitting
        .as_ref()
        .is_some_and(|t| now >= t.over_at())
    {
        radio.transmitting = None;
    }
    if radio.traffic.as_ref().is_some_and(|t| now >= t.over_at()) {
        radio.traffic = None;
    }
    if radio.traffic.is_some() || radio.transmitting.is_some() {
        // The channel is busy. Anything sent now is lost, which is what happens.
        radio.air.retain(|t| now < t.began + 0.5);
        return;
    }
    // **The first one whose time has come**, which is not the same as the first one in
    // the queue. A station that calls on sixteen and reads the bulletin on six twelve
    // seconds later puts both on the air in the same frame; taking the queue in order
    // regardless of the clock played the bulletin the instant the call was *not* heard —
    // twelve seconds before the station said it, and holding the channel for the whole
    // of that. A transmission begins when it begins.
    let Some(which) = radio.air.iter().position(|t| t.began <= now) else {
        return;
    };
    let next = radio.air.remove(which);
    let off = next.at.map(|(at, _)| (at - here).length()).unwrap_or(0.0);
    let strength = radio.hears(&next, off);
    if strength <= 0.0 {
        return;
    }
    radio.log.push(Heard {
        channel: next.channel,
        from: next.from.clone(),
        words: next.words.clone(),
        strength,
        hour: sky.hour,
    });
    if radio.log.len() > KEEPS {
        radio.log.remove(0);
    }
    // And a line in the deck log for the ones worth a line, which is how a book records
    // that you had the weather at the hour it went out.
    if next.worth_writing {
        log.write(
            sky.hour,
            crate::chart::Wrote::Heard {
                channel: next.channel,
                from: next.from.clone(),
            },
        );
    }
    radio.traffic = Some(next);
}

#[cfg(test)]
mod the_set {
    use super::*;

    /// **A coast station is enormously further off than another yacht**, and the
    /// difference is two square roots rather than a rule anybody wrote down.
    #[test]
    fn the_harbour_carries_further_than_the_boat_does() {
        let boat = carries(AERIAL, AERIAL) / 1_852.0;
        let station = carries(AERIAL, STATION_AERIAL) / 1_852.0;
        assert!(
            (18.0..24.0).contains(&boat),
            "boat to boat came out at {boat:.0} miles"
        );
        assert!(
            (40.0..50.0).contains(&station),
            "the coast station came out at {station:.0} miles"
        );
    }

    /// **A set cannot hear while it is transmitting**, which is the whole reason the
    /// word "over" exists and the first thing anybody gets wrong.
    #[test]
    fn it_is_deaf_while_it_talks() {
        let mut radio = Radio::default();
        let call = Transmission::saying("Fuglhavn Radio", CALLING, "this is Fuglhavn", None, 0.0);
        assert!(radio.hears(&call, 0.0) > 0.0);
        radio.transmitting = Some(Transmission::saying("us", CALLING, "over", None, 0.0));
        assert_eq!(radio.hears(&call, 0.0), 0.0, "it heard itself talking over");
    }

    /// **Dual watch is what makes sixteen a watch rather than a channel.** Working the
    /// harbour and still hearing a distress call is the one thing it is for.
    #[test]
    fn dual_watch_hears_sixteen_while_working_another_channel() {
        let mut radio = Radio {
            channel: 12,
            ..Default::default()
        };
        let distress = Transmission::saying("Vatney", CALLING, "mayday", None, 0.0);
        assert!(
            radio.hears(&distress, 0.0) > 0.0,
            "dual watch heard nothing"
        );
        radio.dual = false;
        assert_eq!(
            radio.hears(&distress, 0.0),
            0.0,
            "it heard sixteen with the dual watch off"
        );
    }

    /// And a station over the horizon is not heard at all — not faintly, not broken.
    /// A VHF works and then it stops.
    #[test]
    fn over_the_horizon_is_silence() {
        let radio = Radio::default();
        let her = Transmission::saying(
            "Tern",
            CALLING,
            "this is Tern",
            Some((glam::DVec2::ZERO, AERIAL)),
            0.0,
        );
        let range = carries(AERIAL, AERIAL);
        assert!(radio.hears(&her, range * 0.5) > 0.9, "she was not strong");
        assert!(
            radio.hears(&her, range * 0.97) < 0.3,
            "she was clear at the very edge"
        );
        assert_eq!(
            radio.hears(&her, range * 1.01),
            0.0,
            "she was heard beyond it"
        );
    }

    /// **Two stations at once is two stations heard by nobody.** The channel is
    /// occupied while somebody is speaking, and that is the one rule of a shared one.
    #[test]
    fn a_busy_channel_loses_what_is_sent_into_it() {
        let mut radio = Radio {
            traffic: None,
            ..Default::default()
        };
        radio.traffic = Some(Transmission::saying(
            "Fuglhavn Radio",
            CALLING,
            "all ships all ships all ships",
            None,
            100.0,
        ));
        assert!(radio.log.is_empty());
        assert!(
            radio.traffic.as_ref().unwrap().over_at() > 102.0,
            "five words took no time to say"
        );
    }
}

/// The forecast, in the words a shipping forecast uses.
///
/// **Generated out of this world's own weather, not played from a recording**, and the
/// argument is `music.rs`'s word for word: a recording cannot know what the wind is
/// doing. A canned forecast would be somebody else's weather read over yours, which is
/// the one thing that would break a radio's reason for existing — you would learn
/// within two passages that it never matches, and after that it is furniture.
///
/// What it is made of is already in the model and was not there a week ago: the mean
/// wind and the cover come off the synoptic, the visibility off the fog, and the whole
/// second half of it — *veering northwesterly 6 to gale 8 later* — off the front, which
/// has a time and an air mass behind it. The forecast is a reading of the same numbers
/// the barometer is a reading of.
///
/// The form is the real one and it is in a fixed order, because that is what makes a
/// shipping forecast possible to copy down: wind, then weather, then visibility.
pub fn forecast(sim: &Sim, sea_area: &str) -> String {
    let wind = &sim.sim.wind;
    let knots = sail_core::units::ms_to_kn(sim.sim.telemetry.tws);
    let from = sail_core::units::deg(sim.sim.telemetry.twd).rem_euclid(360.0);
    let (building, turning) = wind.outlook();
    let cover = wind.cover();
    let fog = wind.fog();

    // A forecast gives a range rather than a number, because it is a forecast.
    let force = crate::weather::force(knots);
    let wind_words = format!(
        "{} {} to {}",
        compass(from),
        force.saturating_sub(1).max(1),
        (force + 1).min(12)
    );
    // And what it is going to do, which is the half worth having. Said as "later" when
    // it is an hour or more off, which is the convention and is also the truth here:
    // the outlook looks over an hour.
    let shift = sail_core::units::deg(turning);
    let then = match (sail_core::units::ms_to_kn(building), shift) {
        (k, s) if s > 10.0 => Some(format!(
            "veering {} {} later",
            compass((from + s).rem_euclid(360.0)),
            if k > 3.0 { "and increasing" } else { "" }
        )),
        (_, s) if s < -10.0 => Some(format!(
            "backing {} later",
            compass((from + s).rem_euclid(360.0))
        )),
        (k, _) if k > 3.0 => Some("increasing later".to_string()),
        (k, _) if k < -3.0 => Some("decreasing later".to_string()),
        _ => None,
    };
    let weather = match (fog, cover) {
        (f, _) if f > 0.45 => "fog patches",
        (_, c) if c > 0.80 => "rain at times",
        (_, c) if c > 0.55 => "showers",
        (_, c) if c > 0.25 => "fair",
        _ => "fair",
    };
    // Through the sky's own function, with the sun left out of it: a forecast is for
    // the day rather than for this minute, and the low-sun term is a dusk effect.
    let visibility = match crate::sky::how_far(cover as f32, 0.0, fog as f32) {
        v if v < 1_000.0 => "very poor",
        v if v < 3_700.0 => "poor",
        v if v < 9_260.0 => "moderate",
        _ => "good",
    };
    let mut out = format!("{sea_area}: {wind_words}");
    if let Some(then) = then {
        out.push_str(", ");
        out.push_str(then.trim());
    }
    out.push_str(&format!(". {weather}. {visibility}."));
    out
}

/// A bearing in the words a forecast uses, which are eight and never sixteen.
///
/// Nobody has ever said "west-northwesterly 6" on the radio. The coarseness is the
/// point: a forecast covers a sea area the size of a county and a sixteenth of the
/// compass is a precision it does not have.
fn compass(degrees: f64) -> &'static str {
    const POINTS: [&str; 8] = [
        "northerly",
        "northeasterly",
        "easterly",
        "southeasterly",
        "southerly",
        "southwesterly",
        "westerly",
        "northwesterly",
    ];
    POINTS[(((degrees + 22.5).rem_euclid(360.0)) / 45.0) as usize % 8]
}

/// When the coast station reads the forecast, in hours.
///
/// **Fixed times, and that is the content.** A forecast you can ask for at any moment is
/// a weather page with a different font; a forecast at 0520 and 1720 is a thing you plan
/// a departure around — you either heard it or you are sailing on what you heard eight
/// hours ago. Every shipping forecast in the world works this way and it is not an
/// accident of broadcasting: a schedule is what makes the information worth writing
/// down.
pub const BROADCASTS: [f32; 4] = [5.33, 11.33, 17.33, 23.33];

/// The station that reads it, and how it is announced.
///
/// Called on sixteen and read on a working channel, which is the procedure and is also
/// the first thing a radio teaches: sixteen is where you are told *where to listen*, not
/// where you listen.
#[derive(Resource)]
pub struct Coastguard {
    /// Which broadcast was last put out, so one goes out once.
    said: Option<usize>,
    /// The station's name, taken from the harbour the course starts at.
    pub name: String,
}

impl Default for Coastguard {
    fn default() -> Self {
        Self {
            said: None,
            name: "Coastguard".to_string(),
        }
    }
}

/// Put the forecast on the air when its hour comes round.
pub fn broadcast(
    sim: Res<Sim>,
    sky: Res<crate::sky::Sky>,
    course: Res<crate::course::Course>,
    mut station: ResMut<Coastguard>,
    mut radio: ResMut<Radio>,
) {
    // Which slot the clock is in, if any: a broadcast goes out in the two minutes after
    // its hour, because a station reads it at the time and not at the instant.
    let hour = sky.hour;
    let Some(slot) = BROADCASTS
        .iter()
        .position(|at| hour >= *at && hour < at + 0.034)
    else {
        return;
    };
    if station.said == Some(slot) {
        return;
    }
    station.said = Some(slot);
    if station.name == "Coastguard" {
        if let Some(harbour) = course.harbours.first() {
            station.name = format!("{} Coastguard", harbour.name);
        }
    }
    let now = sim.sim.state.time;
    let here = sim.sim.state.pos;
    // The station is ashore and high up; the call goes out from the harbour it is named
    // after, so whether you hear it depends on where you are — see `carries`.
    let at = course.harbours.first().map(|h| h.at).unwrap_or(here);
    radio.broadcast(Transmission::saying(
        &station.name,
        CALLING,
        // **Short, because a call on sixteen is short.** Sixteen is a channel everybody
        // in forty miles is listening to, and the procedure for a safety call is three
        // words of attention, who you are, where to listen, out. The first version of
        // this read like a letter, and it did not fit on the set's own display — which
        // is the same complaint a coastguard would make about it.
        &format!(
            "All ships, all ships, all ships. This is {}. \
             Weather bulletin on channel {}. Out.",
            station.name, 6
        ),
        Some((at, STATION_AERIAL)),
        now,
    ));
    radio.broadcast(Transmission::saying(
        &station.name,
        6,
        &format!("Weather bulletin. {}", forecast(&sim, "The archipelago")),
        Some((at, STATION_AERIAL)),
        // Read on the working channel after the call on sixteen is finished.
        now + 12.0,
    )
    .for_the_book());
}

#[cfg(test)]
mod what_the_station_says {
    use super::*;

    /// **The forecast is in the order a shipping forecast is in**, because that order is
    /// what makes one possible to copy down: wind, then weather, then visibility. A
    /// sailor reaching for a pencil knows what is coming next.
    #[test]
    fn it_reads_in_the_order_a_forecast_reads() {
        let said = "Archipelago: southwesterly 5 to 7, veering northwesterly and increasing later. showers. moderate.";
        let wind = said.find("southwesterly").expect("a wind");
        let weather = said.find("showers").expect("the weather");
        let visibility = said.find("moderate").expect("the visibility");
        assert!(wind < weather && weather < visibility);
    }

    /// **Eight points and never sixteen.** Nobody has ever said "west-northwesterly 6"
    /// on the radio: a forecast covers a sea area the size of a county and a sixteenth
    /// of the compass is a precision it does not have.
    #[test]
    fn the_compass_is_the_forecasters_compass() {
        assert_eq!(compass(0.0), "northerly");
        assert_eq!(compass(44.0), "northeasterly");
        // Two hundred is still southerly: the boundary is at 202 and a half, which is
        // the kind of thing a test is for.
        assert_eq!(compass(200.0), "southerly");
        assert_eq!(compass(210.0), "southwesterly");
        assert_eq!(compass(359.0), "northerly");
        for bearing in 0..360 {
            let said = compass(bearing as f64);
            assert!(
                said.ends_with("erly"),
                "{bearing} degrees came out as {said}"
            );
        }
    }

    /// **Four a day at fixed times**, which is the content rather than the schedule. A
    /// forecast you can ask for at any moment is a weather page with a different font.
    #[test]
    fn it_goes_out_four_times_a_day_at_stated_hours() {
        assert_eq!(BROADCASTS.len(), 4);
        for pair in BROADCASTS.windows(2) {
            assert!(pair[1] > pair[0], "the schedule runs backwards");
            assert!(
                pair[1] - pair[0] > 3.0,
                "two broadcasts {:.1} hours apart",
                pair[1] - pair[0]
            );
        }
    }

    /// And it takes a while to read, because somebody is reading it. A bulletin that
    /// arrives instantly is a notification.
    #[test]
    fn a_bulletin_takes_time_to_read() {
        let bulletin = Transmission::saying(
            "Fuglhavn Coastguard",
            6,
            "Weather bulletin. The archipelago: southwesterly 5 to 7, veering \
             northwesterly and increasing later. showers. moderate.",
            None,
            0.0,
        );
        assert!(
            (5.0..25.0).contains(&bulletin.takes),
            "it took {:.1} seconds to read",
            bulletin.takes
        );
    }
}

// -----------------------------------------------------------------------------------
// The harbour, when you come into its water.
// -----------------------------------------------------------------------------------

/// How far off a harbour its port control takes an interest, metres.
///
/// Two miles. Inside that you are in the approach, committed to it, and in the water the
/// office can see — which is the only water it has anything useful to say about.
pub const APPROACH: f64 = 3_700.0;

/// How often it repeats the traffic and tidal broadcast while you are in there, seconds.
///
/// Ten minutes. A port does repeat its traffic information on the working channel, and
/// the repeat is also the forgiveness: the call on sixteen tells you where to listen,
/// and if you were slow to change channel you get the next one rather than nothing.
const REPEATS_EVERY: f64 = 600.0;

/// The harbour that is currently taking an interest.
#[derive(Resource, Default)]
pub struct PortControl {
    /// Which one, so that it calls when you come into its water and not every frame.
    working: Option<usize>,
    /// And which one it called last, kept after you leave — otherwise a boat working
    /// along the edge of the approach crosses the line four times in a watch and gets
    /// the all-ships call four times, which is a station nobody would leave on.
    called: Option<usize>,
    said_at: f64,
}

impl PortControl {
    /// Whether to say anything to a vessel in this harbour's water, and whether it is an
    /// arrival worth calling all ships about.
    ///
    /// `None` is silence. `Some(true)` is the call on sixteen and then the information;
    /// `Some(false)` is the information again on the working channel, which is what a
    /// port repeats while you are in there.
    fn coming_in(&mut self, which: usize, now: f64) -> Option<bool> {
        let arriving = self.working != Some(which);
        self.working = Some(which);
        if now - self.said_at < REPEATS_EVERY && (!arriving || self.called == Some(which)) {
            return None;
        }
        let announce =
            arriving && (self.called != Some(which) || now - self.said_at > REPEATS_EVERY);
        self.called = Some(which);
        self.said_at = now;
        Some(announce)
    }
}

/// What the harbour has to say, which is only what it actually knows.
///
/// **The one rule the forecast is written under holds here too**: nothing is invented.
/// There is no ferry in this model, so no ferry is reported. What a port office really
/// does know is the state of its own tide, the stream through its own fairway, and what
/// is moving in it — and all three of those are in the simulation and none of them had
/// anywhere to be said out loud. The tide especially: it has been under the keel since
/// the spring and the only place it showed was the depth sounder.
pub fn traffic(sim: &Sim, name: &str, at: glam::DVec2, hour: f32, others: usize) -> String {
    let tide = &sim.sim.tide;
    let now = sim.sim.state.time;
    // High water, in ship's time, which is the figure anybody plans an entry around.
    let high = (hour as f64 + tide.until_high(now) / 3_600.0).rem_euclid(24.0);
    let depth = sim
        .sim
        .land
        .as_ref()
        .map(|l| l.depth(at))
        .unwrap_or(10.0)
        .max(2.0);
    let stream = tide.stream(now, depth);
    let rate = sail_core::units::ms_to_kn(stream.length());
    let mut out = format!(
        "Traffic and tidal information, the {name} approach. \
         High water {:02.0}{:02.0}, height {:.1} metres. ",
        high.floor(),
        (high.fract() * 60.0).floor(),
        tide.range()
    );
    // A stream is named by where it sets *to*, which is the opposite convention to the
    // wind and the one mistake everybody makes reading a tidal atlas for the first time.
    if rate > 0.15 {
        out.push_str(&format!(
            "The stream is setting {} at {:.1} knots. ",
            points_to(sail_core::units::deg(stream.x.atan2(stream.y))),
            rate
        ));
    } else {
        out.push_str("The stream is slack. ");
    }
    out.push_str(match others {
        0 => "No reported traffic. Out.",
        1 => "One vessel under way in the approach. Out.",
        _ => "Several vessels under way in the approach. Out.",
    });
    out
}

/// Where a stream sets, in the eight points a harbour would use.
fn points_to(degrees: f64) -> &'static str {
    const POINTS: [&str; 8] = [
        "north",
        "northeast",
        "east",
        "southeast",
        "south",
        "southwest",
        "west",
        "northwest",
    ];
    POINTS[(((degrees + 22.5).rem_euclid(360.0)) / 45.0) as usize % 8]
}

/// The harbour, calling a vessel that has come into its approach.
pub fn harbour(
    sim: Res<Sim>,
    sky: Res<crate::sky::Sky>,
    course: Res<crate::course::Course>,
    fleet: Res<crate::fleet::Fleet>,
    mut port: ResMut<PortControl>,
    mut radio: ResMut<Radio>,
) {
    let here = sim.sim.state.pos;
    let now = sim.sim.state.time;
    let near = course
        .harbours
        .iter()
        .enumerate()
        .map(|(i, h)| (i, (h.at - here).length()))
        .filter(|(_, off)| *off < APPROACH)
        .min_by(|a, b| a.1.total_cmp(&b.1));
    let Some((which, _)) = near else {
        // Out of its water: the next harbour you come into calls you again, and the one
        // you have left stops talking.
        port.working = None;
        return;
    };
    // It calls you up when you come into a harbour's water, and reads the information
    // again every so often while you are in it — but the call on sixteen is only for an
    // arrival it has not already called.
    let Some(announce) = port.coming_in(which, now) else {
        return;
    };
    let place = &course.harbours[which];
    let station = format!("{} Port Control", place.name);
    // What is actually moving in there, which in this game is whoever else is sailing.
    let others = fleet
        .boats
        .iter()
        .filter(|b| {
            let at = glam::DVec2::new(b.drawn.pos[0] as f64, b.drawn.pos[1] as f64);
            (at - place.at).length() < APPROACH
        })
        .count();
    if announce {
        radio.broadcast(Transmission::saying(
            &station,
            CALLING,
            &format!(
                "All ships, all ships. This is {station}. \
                 Traffic information on channel {PORT}. Out."
            ),
            Some((place.at, HARBOUR_AERIAL)),
            now,
        ));
    }
    let said = Transmission::saying(
        &station,
        PORT,
        &traffic(&sim, &place.name, place.at, sky.hour, others),
        Some((place.at, HARBOUR_AERIAL)),
        // After the call on sixteen is finished, when there was one.
        now + if announce { 11.0 } else { 0.0 },
    );
    // **The first one goes in the book and the repeats do not.** A navigator writes down
    // that he had the harbour's information coming in; he does not write a line every
    // ten minutes because the station reads it again, and a page of those would bury the
    // bearings it is kept for.
    radio.broadcast(if announce { said.for_the_book() } else { said });
}

/// Break a transmission into lines of a width, on the words.
///
/// **Here rather than on the page, because both places that show a transmission need
/// it.** The console had a hard cut at thirty columns and a station with a long name ate
/// all thirty of them: *1000 16 Sandholm Port Control* and not one word of what it
/// actually said, which on a call whose entire purpose is to tell you *listen channel
/// twelve* is a display that shows the envelope and throws away the letter.
///
/// Indented by two, because a continuation line that starts in the time column is a line
/// the eye reads as a new entry.
pub fn across(words: &str, width: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut line = String::from("  ");
    for word in words.split_whitespace() {
        if line.chars().count() + 1 + word.chars().count() > width && !line.trim().is_empty() {
            out.push(std::mem::take(&mut line));
            line.push_str("  ");
        }
        if line.trim().is_empty() {
            line.push_str(word);
        } else {
            line.push(' ');
            line.push_str(word);
        }
    }
    if !line.trim().is_empty() {
        out.push(line);
    }
    out
}


#[cfg(test)]
mod what_the_harbour_says {
    use super::*;

    fn a_world() -> Sim {
        Sim::new(crate::weather::Weather::draw(42), 42)
    }

    /// **The tide is in it, and it is this world's tide.** The one number that has been
    /// under the keel since the spring and had nowhere to be said: a port office knows
    /// when its own high water is, and now so do you, in words, without opening a page.
    #[test]
    fn it_gives_the_tide_in_the_words_a_harbour_gives_it_in() {
        let sim = a_world();
        let said = traffic(&sim, "Nordholm", glam::DVec2::ZERO, 9.0, 0);
        assert!(said.contains("High water"), "{said}");
        assert!(said.contains("metres"), "{said}");
        // The hour it names is the hour the model says, read back off the string.
        let hhmm: String = said
            .split("High water ")
            .nth(1)
            .and_then(|rest| rest.split(',').next())
            .expect("a time")
            .to_string();
        let hours: f64 = hhmm[..2].parse::<f64>().expect("hours")
            + hhmm[2..4].parse::<f64>().expect("minutes") / 60.0;
        let want = (9.0 + sim.sim.tide.until_high(0.0) / 3_600.0).rem_euclid(24.0);
        assert!(
            (hours - want).abs() < 0.02 || (hours - want).abs() > 23.98,
            "it said high water at {hhmm} and the tide says {want:.2}"
        );
    }

    /// **A stream is named by where it sets to**, which is the opposite of the wind and
    /// the first mistake anybody makes with a tidal atlas. And it is slack at the top and
    /// the bottom of the tide, which the words have to be able to say.
    #[test]
    fn the_stream_is_named_the_way_a_stream_is_named() {
        let mut sim = a_world();
        // Mid-tide, where it runs hardest.
        sim.sim.tide.put_at(0.0, 0.0);
        let running = traffic(&sim, "Nordholm", glam::DVec2::ZERO, 9.0, 0);
        assert!(running.contains("setting"), "{running}");
        assert!(running.contains("knots"), "{running}");
        // High water, where it is slack.
        sim.sim.tide.put_at(sim.sim.tide.amplitude(), 0.0);
        let slack = traffic(&sim, "Nordholm", glam::DVec2::ZERO, 9.0, 0);
        assert!(slack.contains("slack"), "{slack}");
        assert_eq!(points_to(0.0), "north");
        assert_eq!(points_to(90.0), "east");
        assert_eq!(points_to(200.0), "south");
    }

    /// **It calls you up once, not once a tack.** The approach is a circle and a boat
    /// working along the edge of it crosses the line four times in a watch; calling all
    /// ships on sixteen each time is a station nobody would leave on. It reads the
    /// information again every so often while you are in there, which is what a port
    /// does, and it calls up again only after it has been quiet for a good while.
    #[test]
    fn it_does_not_call_all_ships_every_time_you_cross_the_line() {
        let mut port = PortControl::default();
        // In, at the start of the world.
        assert_eq!(
            port.coming_in(0, 0.0),
            Some(true),
            "it never called all ships at all"
        );
        // Out and straight back in: no second all-ships call.
        port.working = None;
        assert_eq!(
            port.coming_in(0, 60.0),
            None,
            "it called all ships again a minute later"
        );
        // And after a long while away it does call up again, because by then you have
        // been somewhere else.
        port.working = None;
        assert_eq!(
            port.coming_in(0, REPEATS_EVERY * 2.0),
            Some(true),
            "it never called again after an hour away"
        );
        // And while you stay in there it goes on reading the information, without the
        // all-ships call in front of it.
        assert_eq!(
            port.coming_in(0, REPEATS_EVERY * 4.0),
            Some(false),
            "it stopped reading the information to a vessel still in the approach"
        );
    }

    /// **It reports the traffic there is and not the traffic a harbour usually has.**
    /// The same rule the forecast is written under: there is no ferry in this model, so
    /// there is no ferry on the air. What it counts is whoever else is actually sailing.
    #[test]
    fn it_does_not_invent_a_ferry() {
        let sim = a_world();
        let empty = traffic(&sim, "Nordholm", glam::DVec2::ZERO, 9.0, 0);
        assert!(empty.contains("No reported traffic"), "{empty}");
        let one = traffic(&sim, "Nordholm", glam::DVec2::ZERO, 9.0, 1);
        assert!(one.contains("One vessel"), "{one}");
        let crowd = traffic(&sim, "Nordholm", glam::DVec2::ZERO, 9.0, 4);
        assert!(crowd.contains("Several vessels"), "{crowd}");
    }
}

#[cfg(test)]
mod working_the_set {
    use super::*;

    /// A set in a world, with a book to write in.
    fn aboard() -> App {
        let mut app = App::new();
        app.insert_resource(Sim::new(crate::weather::Weather::draw(42), 42));
        app.insert_resource(crate::sky::Sky::at(12.0, 0.2));
        app.init_resource::<crate::chart::DeckLog>();
        app.init_resource::<Radio>();
        app.add_systems(Update, listen);
        app
    }

    fn at(app: &mut App, seconds: f64) {
        app.world_mut().resource_mut::<Sim>().sim.state.time = seconds;
        app.update();
    }

    /// **A transmission begins when it begins.**
    ///
    /// A station calls on sixteen and reads the bulletin on its working channel twelve
    /// seconds later, and both go on the air in the same frame. Taking the queue in
    /// order regardless of the clock played the second one the instant the first was not
    /// heard — twelve seconds before the station said it, and holding the channel for
    /// the whole of that. Found by reading, which is the only way this one shows: on
    /// screen it looks like a radio working.
    #[test]
    fn nothing_is_heard_before_it_is_said() {
        let mut app = aboard();
        let here = app.world().resource::<Sim>().sim.state.pos;
        {
            let mut radio = app.world_mut().resource_mut::<Radio>();
            // The call, on a channel this set is not on, so it is dropped the way a
            // transmission out of earshot is.
            radio.broadcast(Transmission::saying(
                "Station",
                PORT,
                "listen channel 6",
                Some((here, HARBOUR_AERIAL)),
                0.0,
            ));
            // And the bulletin, twelve seconds behind it, on a channel it is on.
            radio.broadcast(Transmission::saying(
                "Station",
                CALLING,
                "the bulletin",
                Some((here, HARBOUR_AERIAL)),
                12.0,
            ));
        }
        // Twice, because one frame can only put one thing in the speaker: the first
        // update spends itself on the call that is not heard, and it is the one after
        // it that used to play the bulletin early.
        at(&mut app, 1.0);
        at(&mut app, 1.1);
        assert!(
            app.world().resource::<Radio>().traffic.is_none(),
            "the bulletin came out of the speaker eleven seconds before it was read"
        );
        at(&mut app, 12.5);
        assert!(
            app.world().resource::<Radio>().traffic.is_some(),
            "and then it never came at all"
        );
    }

    /// **What is worth writing is written, and the rest is not.** The set keeps
    /// everything it heard; the deck log keeps what somebody decided to write.
    #[test]
    fn only_the_bulletin_goes_in_the_book() {
        let mut app = aboard();
        let here = app.world().resource::<Sim>().sim.state.pos;
        {
            let mut radio = app.world_mut().resource_mut::<Radio>();
            radio.broadcast(Transmission::saying(
                "Station",
                CALLING,
                "all ships",
                Some((here, HARBOUR_AERIAL)),
                0.0,
            ));
        }
        at(&mut app, 1.0);
        assert_eq!(app.world().resource::<Radio>().log.len(), 1);
        assert!(
            app.world().resource::<crate::chart::DeckLog>().lines.is_empty(),
            "a securité was written up in the deck log"
        );
        // The clock is the bulkhead clock, not a stopwatch started at the dock.
        assert_eq!(app.world().resource::<Radio>().log[0].hour, 12.0);
        {
            let mut radio = app.world_mut().resource_mut::<Radio>();
            radio.traffic = None;
            radio.broadcast(
                Transmission::saying(
                    "Station",
                    CALLING,
                    "weather bulletin",
                    Some((here, HARBOUR_AERIAL)),
                    20.0,
                )
                .for_the_book(),
            );
        }
        at(&mut app, 21.0);
        assert_eq!(
            app.world().resource::<crate::chart::DeckLog>().lines.len(),
            1,
            "the bulletin was not written up"
        );
    }
}
