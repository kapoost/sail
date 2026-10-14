//! Fixed-step integrator.
//!
//! Standard ship manoeuvring equations in the body frame, with the rigid-body coupling
//! terms kept: `X = m(u̇ - v·r)` and `Y = m(v̇ + u·r)`. Dropping them is a common
//! shortcut and it costs you the feel of a turn — the outward slide as the boat comes
//! round is exactly that `u·r` term.
//!
//! 200 Hz. The stiffest mode is roll, with a natural period near 3 s, so this is
//! generous; the margin buys headroom for waves later without revisiting the step.

use crate::boat::BoatConfig;
use crate::hull::{frictional_resistance, induced_resistance, residuary_resistance, righting_arm};
use crate::hydrostatics;
use crate::sail::{
    camber_for_lead, camber_for_outhaul, headsail_blanketing, rig_interference, sail_forces,
    twist_for_lead, twist_for_vang,
};
use crate::state::{BoatState, Controls, Telemetry};
use crate::units::{rad, smoothstep, wrap_pi, wrap_tau, GRAVITY, RHO_SEA};
use crate::water::{FlatWater, WaterSurface};
use crate::wind::{direction_of, shear_factor, WindField};
use glam::DVec2;

pub const SUBSTEP: f64 = 1.0 / 200.0;

/// How hard the mainsheet holds the boom once it is tight, newton-metres per radian.
///
/// Stiff, because a tackle is: a few degrees of give under the full load of the sail,
/// no more. Any softer and the boom wanders visibly in steady air, which it does not.
const SHEET_STIFFNESS: f64 = 140_000.0;
/// Damping while the sheet is tight — the tackle's own friction, which is considerable.
const SHEET_DAMPING: f64 = 5_000.0;
/// And while it is slack, when only the air and the gooseneck are resisting.
const BOOM_DAMPING: f64 = 190.0;

/// How much the sheet gives before it is holding anything, radians.
///
/// A mainsheet is not a bar. There is stretch in the rope, slop in four sheaves and a
/// traveller car that moves, and the sum of it is a couple of degrees the boom can wander
/// through before the tackle is taking load at all. In steady air nobody would notice.
/// In a seaway it is most of what the boom *does*: she rolls, the boom swings into the
/// slack, the rope comes up and stops it, and the sail snatches — which is the noise a
/// boat makes rolling downwind in light air and a thing the model had no room for while
/// the sheet was infinitely tight at exactly its set angle.
///
/// Measured rather than chosen, and then taken at the low end of what the measurement
/// allows: a 6:1 tackle with twenty metres of line in it stretching the one per cent
/// polyester stretches under load, worked back through a 2.4 m lever, is between one and
/// four centimetres at the boom — one to two degrees. Two and a half degrees was tried
/// first and cost the beam-sea roll test a tenth of a degree it did not have to spare:
/// slack lets the boom lag instead of transmitting the sail's force rigidly, so the rig
/// damps the roll it is supposed to be driving. That is a real effect and the test is
/// measuring a real thing; the honest response was to take the bottom of the physical
/// range rather than to widen the test.
const SHEET_SLACK: f64 = 1.8 * std::f64::consts::PI / 180.0;

/// The slowest a boom crossing the boat is ever seen to move, radians a second.
///
/// Not a fudge, and worth saying why. The aerodynamic model gives a sail on the wrong
/// side a force that falls almost to nothing as the chord lines up with the wind near the
/// centreline, so a boom in light air would start its crossing at a couple of degrees a
/// second and spend three seconds getting going. Real booms do not do that. What is
/// missing from the force model is everything that is not steady aerodynamics — the
/// leech snapping through, the roll of the boat throwing it, the vang and the topping
/// lift, the man ducking — and the sum of it is that a boom which has decided to come
/// across comes across. It is thrown, not eased.
///
/// Forty-five degrees a second is the floor. Measured against the whole manoeuvre: it
/// puts an unassisted gybe in ten knots at 2.8 seconds and one in twenty-two at 1.2, and
/// the second number is almost untouched by it because there the air is doing the work.
/// So the floor is only ever felt where it was wanted — in light air, where the model
/// would otherwise have the boom creeping — and above it the physics is in charge and the
/// manoeuvre is quicker in more wind, which is the whole character of a gybe.
const BOOM_CROSSING: f64 = 45.0 * std::f64::consts::PI / 180.0;

/// What is left of the boom's speed after it fetches up against the cap shroud.
///
/// Not nothing, which is what it used to be. A spar coming across in a breeze arrives at
/// the wire with real energy and the wire is a spring with a mast at the end of it: the
/// boom rebounds, the sheet snatches it back, and it settles in a swing or two. That
/// bounce is how a gybe *ends*, and it is the difference between a manoeuvre that
/// finishes with a bang and one that finishes by arriving.
///
/// Only on a real impact. Below ten degrees a second the boom is leaning on the wire
/// rather than hitting it, and bouncing there would chatter.
const SHROUD_BOUNCE: f64 = 0.35;

pub struct Simulation {
    pub config: BoatConfig,
    pub state: BoatState,
    pub wind: Box<dyn WindField>,
    pub water: Box<dyn WaterSurface>,
    pub telemetry: Telemetry,
    accumulator: f64,
    /// What ended the passage, and how close each sail is to being the thing that does.
    mishap: Option<crate::state::Mishap>,
    /// Which side the apparent wind is on, with hysteresis. See the gybe.
    wind_side: f64,
    strain_main: f64,
    strain_head: f64,
    strain_stay: f64,
    capsizing: f64,
    /// The state one substep back, kept so a frame landing between two substeps can be
    /// drawn where the boat actually is rather than where it last happened to be.
    prev_state: BoatState,
    /// Last substep's wave slopes along and across the hull, so their rate of change
    /// can be differenced. Motion damping has to be charged on movement *relative to
    /// the water surface*: a boat pitching exactly with the wave is following it, not
    /// radiating waves, and charging it for that made a following sea cost as much as
    /// a head one.
    prev_slope_along: f64,
    prev_slope_athwart: f64,
    /// The land, if there is any. Shared, because the presentation layer draws the
    /// same islands the physics runs aground on.
    pub land: Option<std::sync::Arc<crate::land::Archipelago>>,
    /// Whether the slope history above is real yet.
    ///
    /// It has to be asked. Differencing against an unset zero on the very first
    /// substep reported a slope rate of 16 rad/s where the physical ceiling is 0.24,
    /// and the energy-based wave drag turned that into fifty megawatts and a force of
    /// 7.6 MN — enough to reverse a five-tonne boat from three knots to minus seven in
    /// a single five-millisecond step. Everything after that was the wreckage.
    slopes_primed: bool,
}

impl Simulation {
    /// Flat water. Every flat-water result — the polar, the validation suite — rests
    /// on this being the default, so adding a sea is opt-in rather than a surprise.
    pub fn new(config: BoatConfig, state: BoatState, wind: Box<dyn WindField>) -> Self {
        Self {
            config,
            state,
            wind,
            water: Box::new(FlatWater),
            telemetry: Telemetry::default(),
            accumulator: 0.0,
            mishap: None,
            wind_side: 0.0,
            strain_main: 0.0,
            strain_head: 0.0,
            strain_stay: 0.0,
            capsizing: 0.0,
            prev_state: state,
            land: None,
            prev_slope_along: 0.0,
            prev_slope_athwart: 0.0,
            slopes_primed: false,
        }
    }

    /// Put a sea under it, and float the boat on it.
    ///
    /// The settling is not a nicety. A state built for flat water has `heave` and
    /// `pitch` at zero, and dropping that into a sea leaves the hull half a metre out
    /// of position — which for a twenty-tonne boat with a 48 m^2 waterplane is a
    /// quarter of a meganewton of buoyancy and an acceleration over 12 m/s^2. The
    /// slam destroys the state inside five seconds, the keel ends up broadside, and a
    /// stalled keel at speed is a spiral that never recovers. The boat simply never
    /// sailed.
    pub fn with_water(mut self, water: Box<dyn WaterSurface>) -> Self {
        self.water = water;
        self.float_on_water();
        self
    }

    /// Place the hull on the local surface: heave to the water's height, pitch to its
    /// longitudinal slope.
    pub fn float_on_water(&mut self) {
        let st = self.state;
        let forward = st.forward();
        let half = self.config.lwl * 0.5;
        let bow = self.water.height(st.pos + forward * half, st.time);
        let stern = self.water.height(st.pos - forward * half, st.time);
        self.state.heave = self.water.height(st.pos, st.time);
        self.state.pitch = ((bow - stern) / self.config.lwl).atan();
        self.state.heave_rate = 0.0;
        self.state.pitch_rate = 0.0;
    }

    /// Advance by `dt` seconds of wall-clock time, in fixed substeps. Leftover time is
    /// carried, so the physics never sees a variable step regardless of frame rate.
    pub fn step(&mut self, dt: f64, controls: &Controls) {
        self.accumulator += dt;
        // Cap the catch-up so a stall in the presentation layer cannot spiral.
        let max_substeps = 20;
        let mut n = 0;
        while self.accumulator >= SUBSTEP && n < max_substeps {
            self.prev_state = self.state;
            self.substep(SUBSTEP, controls);
            self.accumulator -= SUBSTEP;
            n += 1;
        }
        if n == max_substeps {
            self.accumulator = 0.0;
        }
    }

    /// Where the boat is *now*, as opposed to at the last completed substep.
    ///
    /// The integrator runs at a fixed 200 Hz and a rendered frame almost never lands on
    /// a substep boundary, so there is always a leftover of up to 5 ms sitting in the
    /// accumulator. Drawing the last completed substep throws that leftover away, and
    /// the error is not a harmless constant: at 120 frames a second each frame consumes
    /// two substeps, then two, then one, so the boat advances 10, 10, 5 milliseconds of
    /// simulated time while the clock advances evenly. That is a twenty percent
    /// velocity modulation at the beat frequency between the frame rate and 200 Hz, and
    /// since the frame rate wanders the beat wanders with it — which is why it reads as
    /// the camera intermittently failing to keep up rather than as steady roughness.
    ///
    /// Interpolating the remainder costs one blend per frame and removes all of it.
    /// Physics still only ever sees whole fixed substeps; this is presentation.
    pub fn render_state(&self) -> BoatState {
        let t = (self.accumulator / SUBSTEP).clamp(0.0, 1.0);
        BoatState::lerp(&self.prev_state, &self.state, t)
    }

    /// Run for a fixed duration with constant controls. Used heavily by the validation
    /// suite to reach steady state.
    pub fn run(&mut self, seconds: f64, controls: &Controls) {
        let steps = (seconds / SUBSTEP).round() as u64;
        for _ in 0..steps {
            self.prev_state = self.state;
            self.substep(SUBSTEP, controls);
        }
    }

    /// Take a fresh boat: the sail is bent on, the keel is clear, nothing has happened
    /// yet.
    ///
    /// The mishap is sticky by design — a sail that has blown out does not come back —
    /// so there has to be one way to say "another boat, another consignment", and this
    /// is it. It does not undo the sailing; it hands you a new one.
    /// What a fresh boat is rigged with: everything shortened right down.
    ///
    /// Because handing somebody the same sail plan that just killed them is not a fresh
    /// boat, it is the same accident with a loading screen. The gennaker that blew out
    /// in twenty-six knots is still set the instant she exists again, and it blows out
    /// again before the words have faded — which is what "L does not reset" looks like
    /// from the deck. She starts under a deep reef with the headsail rolled away, which
    /// is how you leave a dock in weather you do not know yet.
    pub fn safe_rig() -> Controls {
        Controls {
            reef: 1.0,
            furl: 1.0,
            staysail: 0.0,
            rudder: 0.0,
            headsail: crate::state::Headsail::Genoa,
            headsail_aback: false,
            ..Default::default()
        }
    }

    pub fn recover(&mut self, at: BoatState) {
        self.state = at;
        self.prev_state = at;
        self.mishap = None;
        self.strain_main = 0.0;
        self.strain_head = 0.0;
        self.strain_stay = 0.0;
        self.capsizing = 0.0;
    }

    /// Put an archipelago in the water.
    pub fn with_land(mut self, land: std::sync::Arc<crate::land::Archipelago>) -> Self {
        self.land = Some(land);
        self
    }

    fn substep(&mut self, dt: f64, controls: &Controls) {
        let cfg = self.config;
        let st = self.state;

        // What the land is doing here. Both of these vary over a kilometre while the
        // boat is eleven metres long, so one reading at the boat's own position
        // describes the whole of the local water — and the alternative, asking per
        // query, would trace rays a hundred times a substep for an answer that does
        // not change across the hull.
        let (depth, shelter) = match &self.land {
            Some(land) => {
                let from = direction_of(self.wind.sample(st.pos, st.time));
                (land.depth(st.pos), land.shelter(st.pos, from))
            }
            None => (crate::land::OPEN_WATER, 1.0),
        };
        self.water.set_shelter(shelter);

        self.wind.step(dt);
        let wind_ref = self.wind.sample(st.pos, st.time);
        let tws = wind_ref.length();
        let twd = direction_of(wind_ref);
        // And the sea follows the wind, hours behind it. Fed the instantaneous reading
        // rather than a mean, which is correct and costs nothing: the relaxation's time
        // constant is hours, so gusts average out inside it on their own.
        self.water.follow_wind(tws, twd, st.time, dt);

        let velocity = st.velocity_world();
        let heel_cos = st.heel.cos().abs();

        // --- The water the hull is actually in ------------------------------------
        // Orbital flow, not just surface height. A boat in a seaway is moving through
        // water that is itself moving, and that difference is why it surges on a wave
        // rather than only rising on one. Speed through the water and speed over
        // ground stop being the same number here; the hull cares about the former and
        // the apparent wind about the latter.
        let water_velocity = self.water.velocity(st.pos, st.time);
        let water_drift = DVec2::new(water_velocity.x, water_velocity.y);
        let relative = st.to_body(velocity - water_drift);
        let surge_rel = relative.x;
        let sway_rel = relative.y;
        let speed = (surge_rel * surge_rel + sway_rel * sway_rel).sqrt();
        // Past roughly 30 degrees the rudder starts to emerge and the keel loses grip.
        // This is why a knocked-down boat stops answering the helm — and it is the
        // mechanism that makes reefing a decision rather than a cosmetic option.
        let appendage_immersion =
            1.0 - 0.55 * crate::units::smoothstep(rad(30.0), rad(65.0), st.heel.abs());
        let appendage_factor = heel_cos * appendage_immersion;

        // --- Apparent wind, sampled at each sail's centre of effort ---------------
        // The mainsail is in two sections, because the apparent wind aloft is not the
        // apparent wind on deck: the true wind is stronger up there while the boat's
        // speed is the same, so the apparent wind is *freer* higher up. That is why
        // some twist in the leech is correct rather than sloppy, and it is what gives
        // the vang a job it can be done well or badly.
        let ((lower_area, lower_z), (upper_area, upper_z)) = cfg.main_sections(controls.reef);
        let flying = controls.headsail;
        let head = cfg.headsail(flying);
        // A roller genoa is a trim control: half rolled is a smaller genoa and a real
        // way to sail. A sail set flying is not. There is no useful half of a Code 0 —
        // start the furling line and the luff goes slack, the sail folds along its
        // length and flogs, and what is left is doing nothing but wearing itself out.
        // So for the flying sails the line is a hoist, not a reef: the sail is up, or it
        // is away, and the little in between is where it stops working rather than a
        // band to be sailed in.
        let furl = if flying.flying() {
            1.0 - smoothstep(0.10, 0.45, controls.furl)
        } else {
            1.0 - controls.furl.clamp(0.0, 1.0)
        };
        let ((jib_lower_area, jib_lower_z), (jib_upper_area, jib_upper_z)) =
            cfg.headsail_sections(flying, 1.0 - furl);
        // How fast each sail's centre of effort is being swung sideways by the roll.
        // A point `h` above the roll axis moves at `p * h` when the boat rolls at `p`.
        let swing = |h: f64| st.roll_rate * h;
        let (lower_awa, lower_aws) = apparent_wind(
            &st,
            wind_ref,
            lower_z * heel_cos,
            velocity,
            swing(lower_z * heel_cos),
        );
        let (upper_awa, upper_aws) = apparent_wind(
            &st,
            wind_ref,
            upper_z * heel_cos,
            velocity,
            swing(upper_z * heel_cos),
        );
        let (jib_awa, jib_aws) = apparent_wind(
            &st,
            wind_ref,
            jib_lower_z * heel_cos,
            velocity,
            swing(jib_lower_z * heel_cos),
        );
        let (jib_upper_awa, jib_upper_aws) = apparent_wind(
            &st,
            wind_ref,
            jib_upper_z * heel_cos,
            velocity,
            swing(jib_upper_z * heel_cos),
        );

        let main_camber = crate::sail::camber_reefed(
            camber_for_outhaul(controls.outhaul),
            controls.reef,
            cfg.max_reef,
        );
        // Where the sheet says the boom may go, and where the boom actually is.
        //
        // Those are the same thing in flat water and they are not the same thing in a
        // seaway, because the sheet is a rope: it can pull the boom in and it cannot
        // push it out. The boom is sixty kilogrammes of spar and cloth two and a half
        // metres out on a lever, the wind holds it against the sheet, and anything that
        // takes the wind out of it for a moment — a roll to windward, the wave that
        // started the roll — lets it swing in on its own weight. Then the breeze comes
        // back and the sheet snatches.
        //
        // The cost is not the noise. It is that each of those excursions moves the angle
        // of attack by a few degrees, and a sail working at its limit has nowhere to put
        // them: trimmed on the edge of the groove, a sail knocked two degrees further in
        // stalls, and two degrees out luffs, and either way the flow has to rebuild. A
        // boat in a seaway cannot be trimmed as fine as a boat on flat water, and this
        // is where that comes from.
        let boom_set = cfg.main.profile.chord_angle(controls.main_sheet);
        // Before the first step there is nothing to continue from, so the boom starts
        // where the sheet has it rather than slamming out from the centreline.
        if st.boom_angle == 0.0 && st.boom_rate == 0.0 {
            self.state.boom_angle = boom_set;
        }
        // Signed towards leeward: positive is where a drawing sail lives, negative is the
        // boom having come across to windward, where its angle of attack is the apparent
        // wind angle *plus* the swing and the sail does nothing but flog.
        //
        // Which means that when the *wind* changes sides, the sign has to be flipped by
        // hand, and this is the gybe.
        //
        // Without it a gybe cost nothing whatever. Measured before: she was steered from
        // TWA +165 round through dead astern to −151, and the boom sat at +68 degrees the
        // entire time with its rate at zero — because "sixty-eight degrees to leeward"
        // silently became the other side of the boat the instant leeward did. The boom
        // teleported across with the wind, no swing, no noise, no lost drive, and the
        // most dangerous manoeuvre on a small boat was free.
        //
        // The wind going across the stern does not move the boom. It makes the boom
        // *wrong*: the same spar, in the same place over the deck, is now to windward,
        // and the sail is aback until the thing comes across. One sign change says all of
        // that, and everything downstream — the pendulum below, the backed sail's angle
        // of attack, the shadow the main casts — then does what it already knew how to
        // do.
        // With hysteresis, and that is not a detail. The first version flipped whenever
        // the apparent wind changed sign while it was abaft the beam, which near dead
        // astern it does constantly — every wave rolls the boat, the apparent swings
        // across the stern and back, and the boom was teleporting from side to side
        // several times a second. Measured in twenty-two knots: the crossing "took" ten
        // milliseconds, because what was being measured was a spar flickering rather than
        // a spar moving.
        //
        // The wind has to get ten degrees *past* dead astern before the world changes,
        // which is also how it feels from the helm: a boat squared off dead downwind is
        // on neither gybe, and nothing happens until the wind is properly behind the
        // sail.
        if self.wind_side == 0.0 {
            self.wind_side = if lower_awa == 0.0 {
                1.0
            } else {
                lower_awa.signum()
            };
        } else if lower_awa.signum() != self.wind_side && lower_awa.abs() < crate::units::rad(160.0)
        {
            // Twenty degrees past dead astern, not ten.
            //
            // At ten the band was narrower than a boat squared away actually wanders: on
            // a dead run she rolls, the apparent swings five or six degrees either side
            // of the stern with every wave, and anything inside the band was being
            // treated as a gybe. Reported from the deck as not settling at a hundred and
            // eighty — and it was not, because every swing through the stern threw the
            // boom and sent the headsail across the foretriangle again, so the sail spent
            // its whole time crossing and never drawing.
            self.wind_side = lower_awa.signum();
            self.state.boom_angle = -self.state.boom_angle;
            self.state.boom_rate = -self.state.boom_rate;
            // And the headsail starts its own crossing, from where it is: sheeted on
            // what has just become the windward side. For a tack that is the backed jib
            // that pushes her bow round; for a gybe it is the sail having to come round
            // the forestay. Both are the same event to this number.
            // Except for a symmetric kite, which does not cross anything. It is the
            // same shape on both gybes: what changes sides is the *pole*, taken off one
            // corner and clipped on to the other, and the sail goes on drawing
            // throughout. A spinnaker that collapsed and thrashed across the foredeck at
            // every gybe would be a sail nobody would ever set.
            if controls.headsail != crate::state::Headsail::Spinnaker {
                self.state.jib_across = 0.0;
            }
        }
        let boom_angle = if st.boom_angle == 0.0 && st.boom_rate == 0.0 {
            boom_set
        } else {
            self.state.boom_angle
        };
        // Aback: the boom is on the windward side and the sail is being pressed from the
        // front. A sail there makes no drive worth the name whatever its sheet says.
        let boom_aback = boom_angle < 0.0;
        let twist = twist_for_vang(controls.vang, boom_angle.abs());
        // The upper section is eased by the twist, expressed back through the sheet
        // mapping so both sections go through the same force model.
        // The *section's* twist, not the head's: see `UPPER_SECTION_TWIST`.
        let upper_sheet = cfg
            .main
            .profile
            .sheet_for_chord_angle(boom_angle.abs() + twist * crate::sail::UPPER_SECTION_TWIST);
        // And the lower section is set by where the boom *is*, not by where the sheet
        // would have it. In steady air those are the same number — the sheet is what put
        // the boom there — and the whole difference is the second and a half of a gybe,
        // when the spar is somewhere the sheet has not asked for and the sail has to be
        // computed from the spar.
        let lower_sheet = cfg.main.profile.sheet_for_chord_angle(boom_angle.abs());

        // What each sail does to the other's wind. Worked out from the lift each would
        // make on its own — two extra coefficient lookups — and then applied to the
        // angle the other one sees.
        // Taken from the step before, which at two hundred a second is five milliseconds
        // out of date and saves computing every sail twice. The pair is a feedback loop
        // either way; a loop that is one substep behind settles to the same place.
        let (bend_main, bend_jib) = crate::sail::slot_interaction(
            self.telemetry.main.coeffs.cl,
            self.telemetry.jib.coeffs.cl,
        );
        // And whether there is a mainsail behind the headsail at all, which decides
        // something else again: whether the headsail can be sheeted close. A sloop's
        // two sails are one aerofoil, and the headsail's exit flow is received by the
        // slot. Hand the main and it is received by nothing — see `alone_leech_drag`.
        //
        // From the *area* that is actually set, which is the correction to the first
        // version of this and worth writing down: it asked the mainsail's lift
        // coefficient, and a coefficient knows nothing about whether the sail exists. A
        // main rolled away to nothing still reports a perfectly good cl, so the slot
        // stayed open with no mainsail in it and the whole mechanism did nothing. Area
        // cannot lie about that. Both are needed — a main of some size, and that main
        // drawing rather than flogging.
        let main_set = lower_area + upper_area;
        let jib_slot_support = crate::units::smoothstep(
            0.12,
            0.40,
            main_set / (jib_lower_area + jib_upper_area).max(1e-6),
        )
        .min(crate::units::smoothstep(
            0.0,
            0.5,
            self.telemetry.main.coeffs.cl,
        ));
        let bent = |awa: f64, by: f64| awa.signum() * (awa.abs() + by).max(0.0);
        let lower_awa = bent(lower_awa, bend_main);
        let upper_awa = bent(upper_awa, bend_main);
        let jib_awa = bent(jib_awa, bend_jib);
        let jib_upper_awa = bent(jib_upper_awa, bend_jib);

        let main = crate::sail::sail_forces_in_slot(
            &cfg.main.profile,
            lower_awa,
            lower_aws,
            lower_sheet,
            lower_area,
            st.heel,
            0.0,
            main_camber,
            1.0,
            boom_aback,
        );
        let main_upper = crate::sail::sail_forces_in_slot(
            &cfg.main.profile,
            upper_awa,
            upper_aws,
            upper_sheet,
            upper_area,
            st.heel,
            0.0,
            main_camber,
            1.0,
            boom_aback,
        );
        // A headsail's depth and twist both come from its sheet lead car — the genoa's
        // outhaul and vang in one fitting, which is why it has neither separately.
        // Where the headsail's depth comes from, which is not the same question for all
        // three. A genoa's shape is made by its lead car — the car is its outhaul and
        // its vang in one fitting. A sail set flying has no car and no such control: its
        // depth is cut into it by the sailmaker and nothing on board changes it.
        let jib_camber = match flying {
            crate::state::Headsail::Genoa => {
                crate::sail::camber_rolled(camber_for_lead(controls.genoa_car), controls.furl)
            }
            crate::state::Headsail::Code0 => crate::sail::CAMBER_CODE_ZERO,
            crate::state::Headsail::Gennaker => crate::sail::CAMBER_GENNAKER,
            crate::state::Headsail::Spinnaker => crate::sail::CAMBER_SPINNAKER,
        };
        // Twist, and the same question again: a genoa's leech is opened and closed by
        // its lead car, and a sail set flying has no car to lead to. Its sheet goes to a
        // block on the quarter, as far aft as the boat allows, so its leech falls open
        // to whatever its own cut and the sheet tension give it — a fixed, generous
        // amount, and nothing on the bank changes it.
        let jib_twist = if flying.flying() {
            crate::sail::MAX_LEAD_TWIST * 0.75
        } else {
            twist_for_lead(controls.genoa_car)
        };
        let jib_chord = head.profile.chord_angle(controls.jib_sheet);
        let jib_upper_sheet = head
            .profile
            .sheet_for_chord_angle(jib_chord + jib_twist * crate::sail::UPPER_SECTION_TWIST);

        // What the headsail loses from lying on the shrouds and the spreader. Only
        // the headsail: the mainsail's own contact with the rigging is the boom
        // against the cap shroud, which is already a hard limit on its chord angle.
        //
        // And only a headsail on the stay. A sail set flying is tacked out on the sprit
        // ahead of everything, so there is nothing for it to lie against — which is a
        // large part of why it is worth the trouble of setting one.
        let (jib_lift, jib_drag) = if flying.flying() {
            (1.0, 1.0)
        } else {
            rig_interference(jib_chord, cfg.headsail_bears_at)
        };
        // The upper section bears harder: the spreader reaches further inboard than the
        // shroud does at deck level, so the sail meets it sooner going up.
        let (jib_upper_lift, jib_upper_drag) = if flying.flying() {
            (1.0, 1.0)
        } else {
            rig_interference(jib_chord + jib_twist, cfg.headsail_bears_at * 0.8)
        };

        // Whether it is staying full at all.
        //
        // A sail on a stay luffs along the stay and goes quiet. A sail set flying has
        // nothing holding its luff, so when the apparent wind comes too far forward the
        // luff folds in and the sail collapses into the lee of the main — it stops being
        // a sail and becomes a flag. The band is narrow on purpose: it is what makes
        // choosing the sail a decision rather than a menu, and it is why you hand the
        // gennaker before you harden up, not after.
        let fill = if flying.flying() {
            let limit = flying.fills_above();
            smoothstep(limit - rad(8.0), limit + rad(8.0), jib_awa.abs())
        } else {
            1.0
        };
        // Deep downwind the main takes the wind from a headsail on the stay. It does not
        // take it from a sail squared out on the sprit — getting clear of that shadow is
        // the whole reason the sprit is there.
        let shadow = if flying.flying() {
            0.0
        } else {
            headsail_blanketing(jib_awa, boom_angle)
        };
        let shadow_upper = if flying.flying() {
            0.0
        } else {
            headsail_blanketing(jib_upper_awa, boom_angle)
        };
        // Cloth on cloth: the inner sail eased out into the back of the outer one.
        // Computed here, before either sail's forces, because both of them have to know
        // about it — the one being leant on as much as the one doing the leaning.
        let set_inner = controls.staysail.clamp(0.0, 1.0);
        let fouling = if set_inner > 1e-3 {
            crate::sail::cloth_on_cloth(
                cfg.staysail.profile.chord_angle(controls.staysail_sheet),
                jib_chord,
            )
        } else {
            0.0
        };

        // Mid-crossing the sail is not a sail. It is cloth streaming down the middle of
        // the foretriangle with the wind in neither side of it, and the area that is
        // doing anything is a fifth of what is up there. Triangular rather than smooth on
        // purpose: it is full at both ends of the crossing — backed at the start, which is
        // what pushes her bow round, and drawing at the end.
        let thrashing = (1.0 - (2.0 * st.jib_across - 1.0).abs()).clamp(0.0, 1.0);
        let crossing_fill = 1.0 - 0.8 * thrashing;
        // And on the wrong side of the boat for the first half of it.
        let jib_aback = controls.headsail_aback || st.jib_across < 0.5;
        let jib = crate::sail::sail_forces_in_slot(
            &head.profile,
            jib_awa,
            jib_aws,
            controls.jib_sheet,
            jib_lower_area * fill * crossing_fill,
            st.heel,
            shadow,
            jib_camber,
            jib_slot_support,
            jib_aback,
        )
        .scaled(jib_lift, jib_drag);
        // A sail that has collapsed is not a sail quietly making less force: it is cloth
        // flogging. The force model already took its area away through `fill`; this is
        // what says so out loud, so the picture shakes and the instruments read LUFFING
        // rather than showing a serene triangle doing nothing.
        let mut jib = jib;
        // Collapsed, or lying against the sail inside it. Both of those are cloth that
        // has stopped being a wing, and both have to say so where it can be seen: the
        // force model was taking the drive away quietly and leaving two serene triangles
        // pressed into each other. A fouled rig is the noisiest thing on a boat.
        jib.coeffs.luffing = jib.coeffs.luffing.max(1.0 - fill).max(fouling);
        let jib_upper = crate::sail::sail_forces_in_slot(
            &head.profile,
            jib_upper_awa,
            jib_upper_aws,
            jib_upper_sheet,
            jib_upper_area * fill * crossing_fill,
            st.heel,
            shadow_upper,
            jib_camber,
            jib_slot_support,
            jib_aback,
        )
        .scaled(jib_upper_lift, jib_upper_drag);
        let mut jib_upper = jib_upper;
        jib_upper.coeffs.luffing = jib_upper.coeffs.luffing.max(1.0 - fill).max(fouling);

        // --- Staysail: the inner sail of a double head rig ------------------------
        //
        // Three things happen when it goes up, and only one of them is its own area.
        //
        // It works in disturbed air. Behind a genoa it is nearly pointless — a
        // hundred-and-five percent genoa covers it completely and what reaches it is
        // the genoa's wake — which is why on a boat like this the staysail is an
        // *alternative* to the genoa in heavy weather rather than an addition to it.
        // Under a sail set flying from the sprit there is a real gap between the two,
        // and what comes through it is fast, clean air.
        //
        // And it takes a little from the sail outside it, because the slot works both
        // ways. The net is positive — this is why double head rigs exist — but it is
        // much less than the staysail's area suggests, and a model that just added the
        // square metres would make it the obvious choice everywhere, which it is not.
        let stay_set = set_inner;
        let stay = if stay_set > 1e-3 {
            let sail = &cfg.staysail;
            let (stay_awa, stay_aws) = apparent_wind(
                &st,
                wind_ref,
                sail.z_ce * heel_cos,
                velocity,
                st.roll_rate * sail.z_ce * heel_cos,
            );
            // Trimmed by its own tackle. Self-tacking means the boom crosses by itself,
            // not that nobody trims it.
            let sheet = controls.staysail_sheet.clamp(0.0, 1.0);
            let covered = if flying.flying() { 0.80 } else { 0.35 };
            Some(sail_forces(
                &sail.profile,
                stay_awa,
                stay_aws,
                sheet,
                sail.area * stay_set * covered,
                st.heel,
                headsail_blanketing(stay_awa, boom_angle),
                crate::sail::CAMBER_REFERENCE,
            ))
        } else {
            None
        };
        // What the others give up for the company.
        //
        // The outer sail always pays a little. The mainsail pays only when the staysail
        // is set inside a genoa, and that is the case that decides whether this rig is
        // worth having: a genoa already overlaps the staysail completely, so the inner
        // sail adds almost nothing of its own and backwinds the main's luff for it.
        // Under a sail set flying there is a real gap and the main is left alone.
        //
        // Without this the staysail was worth a fraction of a knot everywhere and there
        // was no reason not to carry it always, which is not what a boat does and not
        // what the sail is for. It belongs under the big reaching sails, or on its own
        // with the genoa rolled away in a blow. Upwind, inside the genoa, it costs.
        let slot_cost = 1.0 - 0.04 * stay_set - 0.18 * fouling;
        let main_cost = if flying.flying() {
            1.0
        } else {
            1.0 - 0.055 * stay_set
        };
        // Two sails pressed together are two sails flogging. The inner one takes most
        // of it — it is the one whose leech has nowhere to go — and the outer one loses
        // the back of its slot.
        let stay = stay.map(|f| f.scaled(1.0 - 0.75 * fouling, 1.0 + 0.5 * fouling));
        let stay_drive = stay.map(|f| f.drive).unwrap_or(0.0);
        let stay_side = stay.map(|f| f.side).unwrap_or(0.0);

        // Where the load sits on each sail, which is not a constant: see
        // [`crate::sail::centre_of_pressure`].
        let main_cop = crate::sail::centre_of_pressure(main_camber, main.coeffs.stalled);
        let jib_cop = crate::sail::centre_of_pressure(jib_camber, jib.coeffs.stalled);
        // Windage: everything above water that is not a sail.
        //
        // This was missing entirely, and what it was missing was not a correction — it
        // was the whole of what a boat does with nothing set. Measured before adding it:
        // with the main handed and the genoa rolled away, from rest, in twenty knots of
        // wind, she did not move at all. Not slowly: *at all*. And given way she kept it,
        // because nothing was taking it off her, so she could coast to windward under
        // bare poles for several minutes.
        //
        // A drag coefficient times an area, acting along the apparent wind — which is to
        // say the direction the air is *going*, so it pushes her bodily to leeward and
        // backwards when she is head to wind. More of it beam-on than head-on, because
        // beam-on presents the whole side of the hull and a rolled headsail as well as
        // the spar.
        //
        // Beam-on it is four times what it is head-on, and the difference is not a
        // detail: eighteen square metres against five is the difference between a boat
        // that lies there and a boat that is being carried to leeward at two knots.
        //
        // And it is counted only for what is actually bare. A mast with a mainsail set
        // on it is the leading edge of that sail — the flow stays attached round it and
        // its drag is already in the sail's own profile drag — and a genoa that is
        // unrolled is a sail rather than a sausage on the stay. Counting both twice would
        // put a few hundred newtons of drag on a boat going to windward that nothing is
        // actually doing to her. The topsides are there whatever is set, so slightly
        // under half of it never goes away.
        let dressed = {
            let main_up = cfg.reefed_main(controls.reef).0 / cfg.main.area.max(1e-6);
            0.5 * main_up.clamp(0.0, 1.0) + 0.5 * (1.0 - controls.furl.clamp(0.0, 1.0))
        };
        const TOPSIDES: f64 = 0.45;
        let bare = TOPSIDES + (1.0 - TOPSIDES) * (1.0 - dressed);
        let sideways = lower_awa.sin() * lower_awa.sin();
        let windage = crate::wind::q_air(lower_aws)
            * (cfg.windage_area + (cfg.windage_beam - cfg.windage_area) * sideways)
            * bare;
        let windage_drive = -windage * lower_awa.cos();
        let windage_side = -windage * lower_awa.sin();

        let drive = (main.drive + main_upper.drive) * main_cost
            + (jib.drive + jib_upper.drive) * slot_cost
            + stay_drive
            + windage_drive;
        let sail_side = (main.side + main_upper.side) * main_cost
            + (jib.side + jib_upper.side) * slot_cost
            + stay_side
            + windage_side;

        // --- Hull velocity direction in the body frame ----------------------------
        // Everything hydrodynamic resolves against this. Getting it wrong means
        // braking the boat's forward motion with a force that physically opposes its
        // sideways motion, which stalls the boat upwind and looks like bad trim.
        let flow_angle = if speed < 1e-3 {
            0.0
        } else {
            sway_rel.atan2(surge_rel)
        };

        // --- Lateral plane: the keel and canoe body earning their leeway -----------
        // Sampled at the keel's own depth, not at the surface. The orbital flow down
        // there is a fraction of what the hull feels, and using the surface value puts
        // the keel at twenty-odd degrees of angle of attack in a modest sea — stalled,
        // and braking with kilonewtons of drag that do not exist.
        let lp_water = self
            .water
            .velocity_at_depth(st.pos, cfg.lateral_plane_depth, st.time);
        let lp_rel = st.to_body(velocity - DVec2::new(lp_water.x, lp_water.y));
        let lp_sway = lp_rel.y + st.yaw_rate * cfg.x_clr;
        let lp_speed = (lp_rel.x * lp_rel.x + lp_sway * lp_sway).sqrt();
        let lp_flow_angle = if lp_speed < 1e-3 {
            0.0
        } else {
            lp_sway.atan2(lp_rel.x)
        };
        let lp = cfg.lateral_plane.forces(-lp_flow_angle, lp_speed);
        // A heeled foil tilts part of its lift into the vertical.
        let lp_lift = lp.lift * appendage_factor;
        let lp_induced = induced_resistance(lp_lift, lp_speed, cfg.induced_span);
        let (lp_fx, lp_fy) = resolve(lp_lift, lp.drag + lp_induced, lp_flow_angle);

        // --- Rudder ---------------------------------------------------------------
        let rudder_angle = controls
            .rudder
            .clamp(-cfg.max_rudder_angle, cfg.max_rudder_angle);
        let rud_water = self
            .water
            .velocity_at_depth(st.pos, cfg.rudder_depth, st.time);
        let rud_rel = st.to_body(velocity - DVec2::new(rud_water.x, rud_water.y));
        let rud_sway = rud_rel.y + st.yaw_rate * cfg.x_rudder;
        let rud_speed = (rud_rel.x * rud_rel.x + rud_sway * rud_sway).sqrt();
        let rud_flow_angle = if rud_speed < 1e-3 {
            0.0
        } else {
            rud_sway.atan2(rud_rel.x)
        };
        let rud = cfg.rudder.forces(-rud_flow_angle - rudder_angle, rud_speed);
        let rud_lift = rud.lift * appendage_factor;
        // The rudder's force is normal to its own chord, not to the hull axis.
        let (rud_fx, rud_fy) = resolve(rud_lift, rud.drag, rud_flow_angle + rudder_angle);

        // --- Where the boat sits in the wave --------------------------------------
        // Both slopes are measured as differences over the hull's own dimensions,
        // which averages out waves shorter than the boat — the analytic point slope
        // cannot know that a wave narrower than the beam does not heel her.
        let forward = st.forward();
        let starboard = st.starboard();
        let half_length = cfg.lwl * 0.5;
        let half_beam = cfg.beam * 0.5;
        let h_bow = self.water.height(st.pos + forward * half_length, st.time);
        let h_stern = self.water.height(st.pos - forward * half_length, st.time);
        let h_stbd = self.water.height(st.pos + starboard * half_beam, st.time);
        let h_port = self.water.height(st.pos - starboard * half_beam, st.time);
        let slope_along = (h_bow - h_stern) / cfg.lwl;
        let slope_athwart_raw = (h_stbd - h_port) / cfg.beam;
        // Rates of change, for damping relative to the surface. The first substep has
        // no history to difference against, so it reports no rate rather than a
        // fabricated one. The clamp is a second line of defence: this rate multiplies
        // into a force through the motion damping, and a term that can reach
        // meganewtons from a numerical transient is too sharp an edge to leave
        // unguarded. The physical ceiling for a four-second sea is a quarter of a
        // radian per second, so two is generous.
        const SLOPE_RATE_LIMIT: f64 = 2.0;
        let (slope_along_rate, slope_athwart_rate) = if self.slopes_primed {
            (
                ((slope_along - self.prev_slope_along) / dt)
                    .clamp(-SLOPE_RATE_LIMIT, SLOPE_RATE_LIMIT),
                ((slope_athwart_raw - self.prev_slope_athwart) / dt)
                    .clamp(-SLOPE_RATE_LIMIT, SLOPE_RATE_LIMIT),
            )
        } else {
            (0.0, 0.0)
        };
        self.prev_slope_along = slope_along;
        self.prev_slope_athwart = slope_athwart_raw;
        self.slopes_primed = true;

        // --- Added resistance in waves ---------------------------------------------
        // Charged from the power the motion damping dissipates, on the grounds that a
        // pitching hull radiates waves and that energy has to be paid for. Identically
        // zero in flat water, which is why the polar did not move.
        let heave_c = cfg.heave_damping
            * 2.0
            * (cfg.heave_mass * RHO_SEA * GRAVITY * cfg.form.waterplane_area()).sqrt();
        let pitch_c = cfg.pitch_damping
            * 2.0
            * (cfg.pitch_inertia * RHO_SEA * GRAVITY * cfg.form.longitudinal_moment()).sqrt();
        let heave_rel = st.heave_rate - water_velocity.z;
        // Pitch relative to the surface, for the same reason heave is.
        let pitch_rel = st.pitch_rate - slope_along_rate;
        let motion_power = heave_c * heave_rel * heave_rel + pitch_c * pitch_rel * pitch_rel;
        // Dividing power by speed is only meaningful while the boat is making way. A
        // simple floor on the denominator is not enough: it leaves a *resistance* that
        // grows without bound as the boat stops, which is backwards. A boat lying
        // still in a sea bobs; it does not experience four kilonewtons of drag. This
        // form has the right asymptotics at both ends — it goes to zero with speed and
        // to power-over-speed once she is moving.
        const WAVE_DRAG_KNEE: f64 = 1.2;
        let v = speed.abs();
        let r_waves_uncapped =
            cfg.wave_drag_fraction * motion_power * v / (v * v + WAVE_DRAG_KNEE * WAVE_DRAG_KNEE);

        // --- Bare-hull resistance --------------------------------------------------
        let r_friction = frictional_resistance(speed, cfg.lwl, cfg.wetted_surface);
        let r_residuary =
            residuary_resistance(speed, cfg.lwl, cfg.displacement, cfg.residuary_coefficient);
        // Two regimes. Up to about twenty degrees a boat just pays a quadratic form-drag
        // penalty. Past that it starts burying its leeward rail: the waterplane goes
        // badly asymmetric, the quarter wave climbs the deck, and the price rises much
        // faster. Without the second term the model thinks thirty degrees of heel is
        // the fastest place to be, which is a full nine degrees past where a real
        // cruiser's VMG peaks — and it makes sail depth look free.
        let heel = st.heel.abs();
        let excess = (heel - cfg.rail_down_heel).max(0.0);
        let r_heel = (r_friction + r_residuary)
            * (cfg.heel_drag_factor * heel * heel + cfg.rail_down_factor * excess * excess);

        // A stated bound on the wave drag, not a silent fudge.
        //
        // Charging added resistance from the motion damping's dissipated power is a
        // reasonable argument off resonance and badly wrong on it. Drive this hull at
        // its 2.8 s pitch period — which half a metre of short chop over a twenty
        // kilometre fetch does exactly — and the term grows until it stops the boat
        // dead and pushes her backwards. Hobby-horsing is real and miserable; it does
        // not do that. The honest fix is frequency-dependent radiation damping from a
        // panel solver rather than one lumped coefficient. Until then this is capped
        // against the bare-hull resistance, so a resonant sea still costs a great deal
        // without being able to bring her to a stop.
        let bare_hull = (r_friction + r_residuary + r_heel).abs();
        let r_waves = r_waves_uncapped.min(1.5 * bare_hull.max(60.0));

        let (hull_fx, hull_fy) =
            resolve(0.0, r_friction + r_residuary + r_heel + r_waves, flow_angle);

        let resistance =
            r_friction + r_residuary + r_heel + r_waves + lp.drag + lp_induced + rud.drag;

        // --- Body-frame forces ----------------------------------------------------
        let fx = drive + hull_fx + lp_fx + rud_fx;
        let fy = sail_side + hull_fy + lp_fy + rud_fy;

        // --- Yaw moment -----------------------------------------------------------
        // Rig side force acting at its fore-and-aft centre of effort. The lead of the
        // centre of effort ahead of the centre of lateral resistance is a lee-helm
        // term; heel pays it back with interest.
        // The fore-and-aft lever each sail works on, with the load's own movement along
        // the chord included: a stalling sail carries its force aft, and aft of the
        // centre of lateral resistance is weather helm.
        let main_x = cfg.main.x_ce
            - crate::sail::chordwise_shift(&cfg.main.profile, cfg.main.area, boom_angle, main_cop);
        let jib_x =
            head.x_ce - crate::sail::chordwise_shift(&head.profile, head.area, jib_chord, jib_cop);
        let mut mz = main_x * (main.side + main_upper.side)
            + jib_x * (jib.side + jib_upper.side) * slot_cost
            + cfg.staysail.x_ce * stay_side
            // The windage has a lever too. Small — about sixty newton-metres in twenty
            // knots against the sails' thousands — but under bare poles it is the only
            // yaw moment on her, and a force carried at the origin because nobody said
            // where it acted is an accident rather than a model.
            + cfg.windage_x * windage_side;
        // Heel swings the centre of effort out to leeward, so the driving force now
        // acts on a lever arm and luffs the boat up. This is weather helm.
        // Two things put a sail's centre of effort out to leeward, and the second one
        // is why a boat can be broached while standing upright: heel swings it out, and
        // easing the sheet carries it out. See [`crate::sail::lateral_offset`].
        let to_leeward = -lower_awa.signum();
        let main_out = to_leeward
            * crate::sail::lateral_offset(
                &cfg.main.profile,
                cfg.main.area,
                boom_angle,
                false,
                main_cop,
            );
        let jib_out = to_leeward
            * crate::sail::lateral_offset(
                &head.profile,
                head.area,
                jib_chord,
                flying.flying(),
                jib_cop,
            );
        let y_lower = lower_z * st.heel.sin() + main_out;
        let y_upper = upper_z * st.heel.sin() + main_out;
        let y_jib = jib_lower_z * st.heel.sin() + jib_out;
        let y_jib_upper = jib_upper_z * st.heel.sin() + jib_out;
        mz += -(y_lower * main.drive
            + y_upper * main_upper.drive
            + y_jib * jib.drive
            + y_jib_upper * jib_upper.drive);
        // Everything the rig has contributed so far, kept so the three causes of weather
        // helm can be told apart downstream.
        let yaw_from_rig = mz;
        // The heeled waterplane is asymmetric, which drives the bow to windward too.
        let yaw_from_heel = -cfg.heel_yaw_coefficient
            * st.heel.sin()
            * 0.5
            * RHO_SEA
            * cfg.lateral_plane.area
            * cfg.lwl
            * speed
            * speed;
        mz += yaw_from_heel;
        // Appendages.
        mz += cfg.x_clr * lp_fy + cfg.x_rudder * rud_fy;
        // Residual bare-hull yaw damping, on top of what the appendages already give.
        mz -= cfg.yaw_damping * st.yaw_rate * (speed + 0.5);
        // The hull's share is what is left once the rig and the heel are taken out, with
        // the rudder's own contribution removed the same way `weather_helm` removes it:
        // the question is what the helm has to *fight*, not what it is doing.
        let yaw_from_hull = mz - yaw_from_rig - yaw_from_heel - cfg.x_rudder * rud_fy;

        // --- Heave and pitch, from distributed buoyancy ---------------------------
        // Each station asks how deep it is in the *local* surface. In a seaway they
        // see different wave heights at the same instant, and that difference is the
        // pitching moment — there is no separate wave-excitation term because none is
        // needed.
        let hydro = hydrostatics::integrate(&cfg.form, st.heave, st.pitch, |x| {
            self.water.height(st.pos + forward * x, st.time)
        });

        let weight = cfg.displacement * GRAVITY;
        let heave_stiffness = RHO_SEA * GRAVITY * cfg.form.waterplane_area();
        let heave_critical = 2.0 * (cfg.heave_mass * heave_stiffness).sqrt();
        // Damp against the water, not against the world. A boat riding a swell is
        // barely moving relative to the surface it is on; damping against a fixed
        // frame would fight the whole wave and pound instead of ride.
        let heave_relative = st.heave_rate - water_velocity.z;
        let fz = hydro.lift - weight - cfg.heave_damping * heave_critical * heave_relative;

        let pitch_stiffness = RHO_SEA * GRAVITY * cfg.form.longitudinal_moment();
        let pitch_critical = 2.0 * (cfg.pitch_inertia * pitch_stiffness).sqrt();
        let my = hydro.pitch_moment - cfg.pitch_damping * pitch_critical * pitch_rel;

        // --- Roll moment ----------------------------------------------------------
        // Heeling arm is the centre of effort above the waterline plus the centre of
        // lateral resistance below it.
        // Each section heels on its own arm, which is the whole point: spilling the
        // top of the sail sheds heeling moment out of proportion to the force it
        // gives up, because the top is where the arm is longest.
        // The windage heels her too, and at a storm's wind speeds that is most of why a
        // boat lying a-hull lies over. Its centre is lower than the mainsail's: the spar
        // is up there but so are the topsides, and they are not.
        let windage_z = cfg.main.z_ce * 0.75;
        let mut mx = windage_side * (windage_z * heel_cos + cfg.z_clr)
            + main.side * (lower_z * heel_cos + cfg.z_clr)
            + main_upper.side * (upper_z * heel_cos + cfg.z_clr)
            + jib.side * (jib_lower_z * heel_cos + cfg.z_clr)
            + jib_upper.side * (jib_upper_z * heel_cos + cfg.z_clr)
            + stay_side * (cfg.staysail.z_ce * heel_cos + cfg.z_clr);
        // The righting curve restores towards the local vertical, and in a wave the
        // local vertical is tilted. Evaluating GZ at the heel *relative to the wave
        // slope* is the whole of wave-induced rolling, to first order, and it keeps
        // the calibrated curve rather than replacing it. The coefficient is below one
        // because the keel is deep and the wave's pressure field decays with depth.
        let slope_athwart = slope_athwart_raw * cfg.effective_wave_slope;
        mx -= weight * righting_arm(st.heel - slope_athwart, cfg.gm, cfg.gz_peak_angle);
        let roll_stiffness = weight * cfg.gm;
        let critical = 2.0 * (cfg.roll_inertia * roll_stiffness).sqrt();
        let roll_rel = st.roll_rate - slope_athwart_rate * cfg.effective_wave_slope;
        mx -= cfg.roll_damping * critical * roll_rel;
        mx -= 0.06 * critical * roll_rel * roll_rel.abs();
        // Keel roll damping, and underway it is the dominant term — three times the
        // still-water figure at five knots. A keel rolling while moving forward sees
        // an angle of attack of `p * lever / V` and generates lift against the roll;
        // the speed cancels out of the product, leaving a damping linear in both. This
        // is why a boat under way rolls far less than the same boat drifting, and
        // without it the model rolled thirty-two degrees beating through a one-metre
        // sea. At rest it contributes nothing, so the still-water roll decay — and the
        // whole flat-water polar, where the roll rate is zero anyway — is untouched.
        let lever = cfg.roll_damping_lever;
        mx -= 0.5
            * RHO_SEA
            * cfg.lateral_plane.area
            * cfg.lateral_plane.lift_slope
            * lever
            * lever
            * speed
            * roll_rel;

        // --- Integrate ------------------------------------------------------------
        // Surfing: the wave's inertial force, closed consistently.
        //
        // For a deep-water wave the water's horizontal acceleration is exactly
        // `g * grad(eta)`, since `omega^2 = g k` makes `A omega^2` and `g A k` the same
        // number. So the Froude-Krylov force on a floating body is its displaced mass
        // times the water's own acceleration — and because the body floats, displaced
        // mass *is* its mass.
        //
        // That closure matters. Applying the force with the bare mass while the
        // equation of motion divides by mass-plus-added-mass leaves the boat unable to
        // follow the water it sits in, and applying it only along the hull axis leaves
        // it unable to follow sideways at all. Both were true here, and the result was
        // a boat thrown about *relative to the water* rather than carried along by it:
        // speed through the water swinging by more than a knot in a one-metre sea,
        // when it should have been nearly steady while speed over ground surged.
        //
        // Using the same effective mass on both sides means that with no sails and no
        // drag she rides the orbit exactly, which is what a floating object does.
        let m_x = cfg.displacement + cfg.surge_added_mass;
        let m_y = cfg.displacement + cfg.sway_added_mass;
        // Note the sign, which cost an hour. For eta = A cos(theta) the water's
        // horizontal acceleration is +A omega^2 sin(theta) while g grad(eta) is
        // -A omega^2 sin(theta): they are opposite. Physically the pressure is higher
        // under a crest, so water is pushed *down* the slope — and so is a boat. Get
        // it backwards and the wave drives her up its own face, pumping energy in, and
        // the symptom is a boat flung about rather than carried.
        // Attenuated over the hull's draft, because the pressure gradient decays with
        // depth and the hull feels the average over its own immersion rather than the
        // value at the surface. The length averaging is already exact: for a sinusoid
        // the bow-to-stern height difference *is* the mean gradient over the hull.
        let depth_factor = self.water.depth_attenuation(cfg.form.canoe_draft);
        let water_accel = -self.water.slope(st.pos, st.time) * GRAVITY * depth_factor;
        let accel_body = st.to_body(water_accel);
        let surf = m_x * accel_body.x;
        let sway_wave = m_y * accel_body.y;

        let du = (fx + surf) / m_x + st.sway * st.yaw_rate;
        let dv = (fy + sway_wave) / m_y - st.surge * st.yaw_rate;
        let dr = mz / cfg.yaw_inertia;
        let dp = mx / cfg.roll_inertia;
        let dw = fz / cfg.heave_mass;
        let dq = my / cfg.pitch_inertia;

        let s = &mut self.state;
        s.surge += du * dt;
        s.sway += dv * dt;
        s.yaw_rate += dr * dt;
        s.roll_rate += dp * dt;
        s.heave_rate += dw * dt;
        s.pitch_rate += dq * dt;

        // --- The headsail changing sides ---------------------------------------------
        //
        // A tack is not the wind crossing the bow. It is the next five seconds: the jib
        // backs and stops her, somebody lets the sheet fly, the sail thrashes across the
        // foretriangle, and the new sheet is ground on until it fills. The model had the
        // first instant and none of the rest — the sail was simply on the other side, in
        // the same frame, drawing.
        //
        // Quicker in more wind for the same reason the boom is: it is the air that blows
        // it across, and a crew in a breeze is also in more of a hurry. Three and a half
        // seconds in a drifter, down to a second and a bit when it is blowing — which is
        // about what a cruising crew with a winch handle manages.
        {
            let across =
                1.2 + 2.3 * (1.0 - (crate::units::ms_to_kn(jib_aws) / 22.0).clamp(0.0, 1.0));
            s.jib_across = (s.jib_across + dt / across).min(1.0);
        }

        // --- The boom ---------------------------------------------------------------
        // Its own little pendulum, swinging about the mast. Four things act on it and
        // the first is the only one that is ever large: the sail's own force, which
        // holds it out against the sheet and is why the boom sits still in steady air.
        {
            let inertia = (cfg.boom_mass * cfg.boom_cg * cfg.boom_cg).max(1.0);
            // The sail pressing the boom to leeward, taken at the sail's own lever.
            let push = (main.lift.hypot(main.drag) + main_upper.lift.hypot(main_upper.drag))
                * cfg.boom_cg
                * 0.45;
            // Heeled over, the boom's own weight pulls it downhill — which is to
            // leeward, and which is why a rolling boat's boom works against its sheet.
            let gravity =
                cfg.boom_mass * GRAVITY * cfg.boom_cg * st.heel.abs().sin() * st.boom_angle.cos();
            // The sheet. A rope: it pulls when it is tight and does nothing when it is
            // not, and the whole behaviour here comes out of that one asymmetry.
            //
            // It is made fast on the centreline, so what it limits is how far the boom
            // can get from the centreline — on *either* side. In a breeze that never
            // matters, because the wind holds the boom out to leeward against the sheet
            // and it stays there. In light air with any swell running it is the whole
            // story: there is not enough wind to hold the boom anywhere, the roll throws
            // it across, it fetches up against the sheet on the far side, and the sail
            // slams and flogs. Which is the most recognisable thing a boat does in a
            // calm, and the model could not do it while the boom was pinned to one side.
            let stretch = st.boom_angle.abs() - (boom_set + SHEET_SLACK);
            let (sheet, damping) = if stretch > 0.0 {
                (
                    -SHEET_STIFFNESS * stretch * st.boom_angle.signum(),
                    SHEET_DAMPING,
                )
            } else {
                (0.0, BOOM_DAMPING)
            };
            // And the boat turning underneath it.
            //
            // The damping above is the sail dragging through the air, and until now it
            // was told only how fast the boom was moving *relative to the deck* — which
            // in a turn is the wrong question. Thirty square metres of cloth two metres
            // out from the mast does not care what the deck is doing; it cares what the
            // air is doing, and when the boat yaws the whole sail is swept sideways
            // through it. So the damping sees the boom's rate through the *air*, which
            // is its rate over the deck plus the boat's own, and the result is that a
            // boat put hard into a turn leaves her boom behind for a moment and then
            // takes it with her. Anybody who has looked up during a sharp luff has seen
            // the boom lag and the leech shake.
            //
            // The second term is the same idea for acceleration rather than speed: the
            // boom is a pendulum on a platform that is itself angularly accelerating, and
            // in the deck's frame that is a force. It is what makes the boom snatch when
            // the helm goes over rather than easing into the turn.
            //
            // Both are written in the leeward-signed frame the boom lives in, which is
            // why the boat's yaw rate arrives multiplied by which side the wind is on.
            let turning = self.wind_side * (damping * st.yaw_rate);
            let accel = (push + gravity + sheet - damping * st.boom_rate + turning) / inertia
                + self.wind_side * dr;
            s.boom_rate += accel * dt;
            // Coming across, it comes across. Only while it is on the windward side and
            // already heading the right way: a boom wandering about in the slack of its
            // sheet in a calm is doing the opposite thing and must be left alone to do it.
            if s.boom_angle < 0.0 && s.boom_rate > 0.0 {
                s.boom_rate = s.boom_rate.max(BOOM_CROSSING);
            }
            s.boom_angle += s.boom_rate * dt;
            // The cap shroud stops it on either side — the same limit the force model
            // uses for the sheet, and the thing a boom actually hits.
            let hard_over = cfg.main.profile.max_chord_angle;
            if s.boom_angle.abs() > hard_over {
                s.boom_angle = hard_over * s.boom_angle.signum();
                s.boom_rate = if s.boom_rate.abs() > crate::units::rad(10.0) {
                    -s.boom_rate * SHROUD_BOUNCE
                } else {
                    0.0
                };
            }
        }
        s.heading = wrap_tau(s.heading + s.yaw_rate * dt);
        s.heel += s.roll_rate * dt;
        s.heel = s.heel.clamp(-1.4, 1.4);
        s.heave += s.heave_rate * dt;
        s.pitch += s.pitch_rate * dt;
        s.pitch = s.pitch.clamp(-0.6, 0.6);
        // Aground. Not a collision model — a keel in the mud is a boat that stops
        // going forwards and starts going sideways with the wind on her, which is
        // exactly how it feels and is enough to make the sounder worth watching.
        // Steerage goes first, because a rudder needs flow over it and there is none.
        if depth < cfg.draft {
            let bite = ((cfg.draft - depth) / cfg.draft).clamp(0.0, 1.0);
            s.surge *= 1.0 - bite * 0.92 * (dt / SUBSTEP);
            s.sway *= 1.0 - bite * 0.55 * (dt / SUBSTEP);
            s.yaw_rate *= 1.0 - bite * 0.80 * (dt / SUBSTEP);
        }

        s.pos += s.velocity_world() * dt;
        s.time += dt;

        // --- Telemetry ------------------------------------------------------------
        let to_wind = DVec2::new(twd.sin(), twd.cos());
        let wind_body = self.state.to_body(to_wind);
        let v = self.state.velocity_world();
        // --- What ends a passage -------------------------------------------------
        //
        // Three things, and each of them is a decision somebody made ten minutes
        // earlier. Aground is instant, because it is: a keel at six knots finding rock
        // is not a process. The other two take a little while, which is the point —
        // there is time to do something, and the whole game is in whether you do.
        let depth_now = depth - cfg.draft;
        if self.mishap.is_none() {
            if depth_now <= 0.0 {
                self.mishap = Some(crate::state::Mishap::Aground);
            } else if st.heel.abs() > cfg.gz_peak_angle + crate::units::rad(25.0) {
                // Past the peak of the righting arm by a good margin she is not coming
                // back, and the model has no business pretending she might.
                self.capsizing += dt;
                if self.capsizing > 1.5 {
                    self.mishap = Some(crate::state::Mishap::Capsized);
                }
            } else {
                self.capsizing = 0.0;
            }
        }
        // Cloth. Every sail is cut for a wind and past it the cloth goes — not at once,
        // because sailcloth is not a fuse, but a sail carried a fifth over its number
        // has about five seconds of dignity left.
        if self.mishap.is_none() {
            let set_main = cfg.reefed_main(controls.reef).0 > 0.5;
            let overload = |aws: f64, limit: f64| (aws / limit - 1.0).max(0.0);
            for (strain, over, which) in [
                (
                    &mut self.strain_main,
                    if set_main {
                        overload(lower_aws, cfg.main.limit_aws)
                    } else {
                        0.0
                    },
                    crate::state::Blown::Main,
                ),
                (
                    &mut self.strain_head,
                    if controls.furl < 0.985 {
                        overload(jib_aws, head.limit_aws)
                    } else {
                        0.0
                    },
                    crate::state::Blown::Headsail(flying),
                ),
                (
                    &mut self.strain_stay,
                    if controls.staysail > 0.02 {
                        overload(jib_aws, cfg.staysail.limit_aws)
                    } else {
                        0.0
                    },
                    crate::state::Blown::Staysail,
                ),
            ] {
                if over > 0.0 {
                    *strain += over * dt;
                    if *strain > 1.0 {
                        self.mishap = Some(crate::state::Mishap::BlownOut(which));
                    }
                } else {
                    // Cloth recovers when the load comes off, but not instantly: a sail
                    // that has been over its number and come back is a sail somebody
                    // frightened, and frightening it twice in a minute is how it goes.
                    *strain = (*strain - 0.25 * dt).max(0.0);
                }
            }
        }

        self.telemetry = Telemetry {
            mishap: self.mishap,
            // From `wind_side` and not from the apparent wind's sign, and that is the
            // whole of it.
            //
            // The stored angle changes sign when `wind_side` does — ten degrees *past*
            // dead astern, with hysteresis, so that a boat squared off downwind does not
            // flicker. Dividing by the raw `awa.signum()` instead meant the two
            // disagreed for exactly as long as the apparent wind spent between 180 and
            // 170 degrees: the drawn boom jumped to the new side the moment the wind
            // touched dead astern, jumped back when the simulation flipped, and only
            // then began its swing.
            //
            // Which is why it looked like a teleport when the helm was put over hard and
            // looked right when the wind line was crossed slowly — reported from the deck
            // exactly that way, and that observation is what found it.
            boom: -self.wind_side * self.state.boom_angle,
            // Where the clew has got to: on the old side at the start of the crossing,
            // streaming down the middle halfway through, hard on the new one at the end.
            // And held to windward for as long as somebody is holding it there, which is
            // a different thing from being caught there by a tack.
            jib_side: if controls.headsail_aback {
                self.wind_side
            } else {
                self.wind_side * (1.0 - 2.0 * self.state.jib_across.clamp(0.0, 1.0))
            },
            strain: self.strain_main.max(self.strain_head).max(self.strain_stay),
            bend_main,
            bend_jib,
            depth,
            under_keel: depth - cfg.draft,
            tws,
            twd,
            twa: wrap_pi(wind_body.y.atan2(wind_body.x)),
            // Read at the lower section, which is where a masthead-free instrument and
            // the helmsman's own face both sit.
            aws: lower_aws,
            awa: lower_awa,
            wind_height: lower_z * heel_cos,
            // Speed through the water, which is what a log reads and what the hull
            // feels. In a seaway it differs from speed over ground by the orbital flow.
            stw: speed,
            sog: v.length(),
            cog: if v.length() > 1e-3 {
                wrap_tau(v.x.atan2(v.y))
            } else {
                self.state.heading
            },
            vmg: v.dot(to_wind),
            heel: self.state.heel,
            leeway: self.state.leeway(),
            yaw_rate: self.state.yaw_rate,
            main,
            main_upper,
            twist,
            jib,
            jib_upper,
            jib_twist,

            significant_height: self.water.significant_height(),
            heave: self.state.heave,
            pitch: self.state.pitch,
            immersion: hydro.mean_immersion,
            heave_accel: dw,
            resistance_waves: r_waves,
            surf_force: surf,
            drive,
            side: sail_side,
            resistance,
            resistance_friction: r_friction,
            resistance_residuary: r_residuary,
            resistance_induced: lp_induced,
            keel_alpha: lp.alpha,
            keel_stalled: lp.stalled,
            keel_surge: lp_rel.x,
            keel_sway: lp_sway,
            rudder_alpha: rud.alpha,
            rudder_stalled: rud.stalled,
            // Measured off the same three factors the force above is built from, so the
            // instrument cannot disagree with the boat. Referenced to five knots over
            // the blade, which is a cruising boat with way on and the speed at which a
            // tiller feels like a tiller.
            rudder_bite: (appendage_factor
                * (rud_speed / crate::units::kn_to_ms(5.0)).powi(2)
                * (1.0 - 0.85 * rud.stalled))
                .clamp(0.0, 1.0),
            weather_helm: mz - cfg.x_rudder * rud_fy,
            surge_from_sails: drive,
            surge_from_hull: hull_fx,
            surge_from_foils: lp_fx + rud_fx,
            yaw_from_rig,
            yaw_from_heel,
            yaw_from_hull,
            righting_arm: righting_arm(self.state.heel, cfg.gm, cfg.gz_peak_angle),
        };
    }
}

/// Resolve a lift/drag pair from the flow frame into the body frame.
///
/// `flow_angle` is the direction the surface is moving through the water, measured
/// from the bow. Drag opposes that direction; lift is normal to it. The forward
/// component of keel lift that this produces is how a boat makes progress to windward.
#[inline]
fn resolve(lift: f64, drag: f64, flow_angle: f64) -> (f64, f64) {
    let (s, c) = (flow_angle.sin(), flow_angle.cos());
    (-drag * c - lift * s, -drag * s + lift * c)
}

/// Apparent wind at a given height above the water, in the body frame.
/// Returns the signed angle (positive with the wind on the starboard side) and speed.
/// The wind a sail feels, which is not the wind the boat feels.
///
/// Three things separate them, and the third was missing.
///
/// The wind aloft is stronger — that is the shear. The boat's own motion takes some of
/// it away — that is the apparent wind. And the *sail is moving sideways*, because the
/// boat is rolling and the sail is six metres up: at a third of a radian a second, the
/// head of the mainsail is swinging through the air at nearly two metres a second,
/// which is a quarter of the breeze on a light day. Leaving it out does not merely lose
/// a detail. It removes a feedback loop that works in both directions and decides how
/// the boat behaves at the two ends of the wind range.
///
/// Close-hauled, rolling to leeward swings the sail downwind, the angle of attack drops,
/// the force falls, and the roll is checked. That is aerodynamic roll damping, and it is
/// most of why a boat under sail rolls so much less than the same boat lying ahull.
///
/// Running with the boom squared off, the same geometry has the opposite sign: rolling
/// to windward swings the sail *into* the wind, the angle of attack grows, the force
/// grows, and the roll goes further. Negative damping. That is the death roll, and it is
/// the reason a boat under spinnaker can wind herself up through three rolls into a
/// broach while the helmsman does everything right.
///
/// So this is the boom swinging, written where it belongs: not as a separate oscillator
/// bolted to the rig, but as the sail being told where it actually is.
fn apparent_wind(
    st: &BoatState,
    wind_ref: DVec2,
    height: f64,
    velocity: DVec2,
    lateral: f64,
) -> (f64, f64) {
    let true_flow = wind_ref * shear_factor(height);
    let apparent_flow = true_flow - velocity;
    let mut body = st.to_body(apparent_flow);
    // The sail's own swing, subtracted the same way the hull's motion is: a sail moving
    // to starboard sees the air coming at it from starboard.
    body.y -= lateral;
    // The wind comes FROM the direction opposite to the flow.
    let awa = wrap_pi((-body.y).atan2(-body.x));
    (awa, body.length())
}
