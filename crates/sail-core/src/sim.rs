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
    camber_for_lead, camber_for_outhaul, headsail_blanketing, sail_forces, twist_for_lead,
    twist_for_vang,
};
use crate::state::{BoatState, Controls, Telemetry};
use crate::units::{rad, wrap_pi, wrap_tau, GRAVITY, RHO_SEA};
use crate::water::{FlatWater, WaterSurface};
use crate::wind::{direction_of, shear_factor, WindField};
use glam::DVec2;

pub const SUBSTEP: f64 = 1.0 / 200.0;

pub struct Simulation {
    pub config: BoatConfig,
    pub state: BoatState,
    pub wind: Box<dyn WindField>,
    pub water: Box<dyn WaterSurface>,
    pub telemetry: Telemetry,
    accumulator: f64,
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
        let ((jib_lower_area, jib_lower_z), (jib_upper_area, jib_upper_z)) =
            cfg.genoa_sections(controls.furl);
        let (lower_awa, lower_aws) = apparent_wind(&st, wind_ref, lower_z * heel_cos, velocity);
        let (upper_awa, upper_aws) = apparent_wind(&st, wind_ref, upper_z * heel_cos, velocity);
        let (jib_awa, jib_aws) = apparent_wind(&st, wind_ref, jib_lower_z * heel_cos, velocity);
        let (jib_upper_awa, jib_upper_aws) =
            apparent_wind(&st, wind_ref, jib_upper_z * heel_cos, velocity);

        let main_camber = camber_for_outhaul(controls.outhaul);
        let boom_angle = cfg.main.profile.chord_angle(controls.main_sheet);
        let twist = twist_for_vang(controls.vang, boom_angle);
        // The upper section is eased by the twist, expressed back through the sheet
        // mapping so both sections go through the same force model.
        let upper_sheet = cfg.main.profile.sheet_for_chord_angle(boom_angle + twist);

        let main = sail_forces(
            &cfg.main.profile,
            lower_awa,
            lower_aws,
            controls.main_sheet,
            lower_area,
            st.heel,
            0.0,
            main_camber,
        );
        let main_upper = sail_forces(
            &cfg.main.profile,
            upper_awa,
            upper_aws,
            upper_sheet,
            upper_area,
            st.heel,
            0.0,
            main_camber,
        );
        // A headsail's depth and twist both come from its sheet lead car — the genoa's
        // outhaul and vang in one fitting, which is why it has neither separately.
        let jib_camber = camber_for_lead(controls.genoa_car);
        let jib_twist = twist_for_lead(controls.genoa_car);
        let jib_chord = cfg.genoa.profile.chord_angle(controls.jib_sheet);
        let jib_upper_sheet = cfg
            .genoa
            .profile
            .sheet_for_chord_angle(jib_chord + jib_twist);

        let jib = sail_forces(
            &cfg.genoa.profile,
            jib_awa,
            jib_aws,
            controls.jib_sheet,
            jib_lower_area,
            st.heel,
            headsail_blanketing(jib_awa),
            jib_camber,
        );
        let jib_upper = sail_forces(
            &cfg.genoa.profile,
            jib_upper_awa,
            jib_upper_aws,
            jib_upper_sheet,
            jib_upper_area,
            st.heel,
            headsail_blanketing(jib_upper_awa),
            jib_camber,
        );

        let drive = main.drive + main_upper.drive + jib.drive + jib_upper.drive;
        let sail_side = main.side + main_upper.side + jib.side + jib_upper.side;

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

        // --- Engine ---------------------------------------------------------------
        let thrust = controls.throttle.clamp(-1.0, 1.0) * cfg.max_thrust;

        // --- Body-frame forces ----------------------------------------------------
        let fx = drive + thrust + hull_fx + lp_fx + rud_fx;
        let fy = sail_side + hull_fy + lp_fy + rud_fy;

        // --- Yaw moment -----------------------------------------------------------
        // Rig side force acting at its fore-and-aft centre of effort. The lead of the
        // centre of effort ahead of the centre of lateral resistance is a lee-helm
        // term; heel pays it back with interest.
        let mut mz = cfg.main.x_ce * (main.side + main_upper.side)
            + cfg.genoa.x_ce * (jib.side + jib_upper.side);
        // Heel swings the centre of effort out to leeward, so the driving force now
        // acts on a lever arm and luffs the boat up. This is weather helm.
        let y_lower = lower_z * st.heel.sin();
        let y_upper = upper_z * st.heel.sin();
        let y_jib = jib_lower_z * st.heel.sin();
        let y_jib_upper = jib_upper_z * st.heel.sin();
        mz += -(y_lower * main.drive
            + y_upper * main_upper.drive
            + y_jib * jib.drive
            + y_jib_upper * jib_upper.drive);
        // The heeled waterplane is asymmetric, which drives the bow to windward too.
        mz += -cfg.heel_yaw_coefficient
            * st.heel.sin()
            * 0.5
            * RHO_SEA
            * cfg.lateral_plane.area
            * cfg.lwl
            * speed
            * speed;
        // Appendages.
        mz += cfg.x_clr * lp_fy + cfg.x_rudder * rud_fy;
        // Prop walk: thrust acting off the centreline.
        mz += cfg.prop_walk * thrust;
        // Residual bare-hull yaw damping, on top of what the appendages already give.
        mz -= cfg.yaw_damping * st.yaw_rate * (speed + 0.5);

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
        let mut mx = main.side * (lower_z * heel_cos + cfg.z_clr)
            + main_upper.side * (upper_z * heel_cos + cfg.z_clr)
            + jib.side * (jib_lower_z * heel_cos + cfg.z_clr)
            + jib_upper.side * (jib_upper_z * heel_cos + cfg.z_clr);
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
        self.telemetry = Telemetry {
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
            weather_helm: mz - cfg.x_rudder * rud_fy,
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
fn apparent_wind(st: &BoatState, wind_ref: DVec2, height: f64, velocity: DVec2) -> (f64, f64) {
    let true_flow = wind_ref * shear_factor(height);
    let apparent_flow = true_flow - velocity;
    let body = st.to_body(apparent_flow);
    // The wind comes FROM the direction opposite to the flow.
    let awa = wrap_pi((-body.y).atan2(-body.x));
    (awa, apparent_flow.length())
}
