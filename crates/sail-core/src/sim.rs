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
use crate::sail::{headsail_blanketing, sail_forces};
use crate::state::{BoatState, Controls, Telemetry};
use crate::units::{rad, wrap_pi, wrap_tau, GRAVITY, RHO_SEA};
use crate::wind::{direction_of, shear_factor, WindField};
use glam::DVec2;

pub const SUBSTEP: f64 = 1.0 / 200.0;

pub struct Simulation {
    pub config: BoatConfig,
    pub state: BoatState,
    pub wind: Box<dyn WindField>,
    pub telemetry: Telemetry,
    accumulator: f64,
}

impl Simulation {
    pub fn new(config: BoatConfig, state: BoatState, wind: Box<dyn WindField>) -> Self {
        Self {
            config,
            state,
            wind,
            telemetry: Telemetry::default(),
            accumulator: 0.0,
        }
    }

    /// Advance by `dt` seconds of wall-clock time, in fixed substeps. Leftover time is
    /// carried, so the physics never sees a variable step regardless of frame rate.
    pub fn step(&mut self, dt: f64, controls: &Controls) {
        self.accumulator += dt;
        // Cap the catch-up so a stall in the presentation layer cannot spiral.
        let max_substeps = 20;
        let mut n = 0;
        while self.accumulator >= SUBSTEP && n < max_substeps {
            self.substep(SUBSTEP, controls);
            self.accumulator -= SUBSTEP;
            n += 1;
        }
        if n == max_substeps {
            self.accumulator = 0.0;
        }
    }

    /// Run for a fixed duration with constant controls. Used heavily by the validation
    /// suite to reach steady state.
    pub fn run(&mut self, seconds: f64, controls: &Controls) {
        let steps = (seconds / SUBSTEP).round() as u64;
        for _ in 0..steps {
            self.substep(SUBSTEP, controls);
        }
    }

    fn substep(&mut self, dt: f64, controls: &Controls) {
        let cfg = self.config;
        let st = self.state;

        self.wind.step(dt);
        let wind_ref = self.wind.sample(st.pos, st.time);
        let tws = wind_ref.length();
        let twd = direction_of(wind_ref);

        let velocity = st.velocity_world();
        let speed = st.speed();
        let heel_cos = st.heel.cos().abs();
        // Past roughly 30 degrees the rudder starts to emerge and the keel loses grip.
        // This is why a knocked-down boat stops answering the helm — and it is the
        // mechanism that makes reefing a decision rather than a cosmetic option.
        let immersion = 1.0 - 0.55 * crate::units::smoothstep(rad(30.0), rad(65.0), st.heel.abs());
        let appendage_factor = heel_cos * immersion;

        // --- Apparent wind, sampled at each sail's centre of effort ---------------
        let (main_area, main_z_ce) = cfg.reefed_main(controls.reef);
        let (jib_area, jib_z_ce) = cfg.furled_genoa(controls.furl);
        let (main_awa, main_aws) = apparent_wind(&st, wind_ref, main_z_ce * heel_cos, velocity);
        let (jib_awa, jib_aws) = apparent_wind(&st, wind_ref, jib_z_ce * heel_cos, velocity);

        let main = sail_forces(
            &cfg.main.profile,
            main_awa,
            main_aws,
            controls.main_sheet,
            main_area,
            st.heel,
            0.0,
        );
        let jib = sail_forces(
            &cfg.genoa.profile,
            jib_awa,
            jib_aws,
            controls.jib_sheet,
            jib_area,
            st.heel,
            headsail_blanketing(jib_awa),
        );

        let drive = main.drive + jib.drive;
        let sail_side = main.side + jib.side;

        // --- Hull velocity direction in the body frame ----------------------------
        // Everything hydrodynamic resolves against this. Getting it wrong means
        // braking the boat's forward motion with a force that physically opposes its
        // sideways motion, which stalls the boat upwind and looks like bad trim.
        let flow_angle = if speed < 1e-3 {
            0.0
        } else {
            st.sway.atan2(st.surge)
        };

        // --- Lateral plane: the keel and canoe body earning their leeway -----------
        let lp_sway = st.sway + st.yaw_rate * cfg.x_clr;
        let lp_speed = (st.surge * st.surge + lp_sway * lp_sway).sqrt();
        let lp_flow_angle = if lp_speed < 1e-3 {
            0.0
        } else {
            lp_sway.atan2(st.surge)
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
        let rud_sway = st.sway + st.yaw_rate * cfg.x_rudder;
        let rud_speed = (st.surge * st.surge + rud_sway * rud_sway).sqrt();
        let rud_flow_angle = if rud_speed < 1e-3 {
            0.0
        } else {
            rud_sway.atan2(st.surge)
        };
        let rud = cfg.rudder.forces(-rud_flow_angle - rudder_angle, rud_speed);
        let rud_lift = rud.lift * appendage_factor;
        // The rudder's force is normal to its own chord, not to the hull axis.
        let (rud_fx, rud_fy) = resolve(rud_lift, rud.drag, rud_flow_angle + rudder_angle);

        // --- Bare-hull resistance --------------------------------------------------
        let r_friction = frictional_resistance(speed, cfg.lwl, cfg.wetted_surface);
        let r_residuary =
            residuary_resistance(speed, cfg.lwl, cfg.displacement, cfg.residuary_coefficient);
        let r_heel = (r_friction + r_residuary) * cfg.heel_drag_factor * st.heel * st.heel;
        let (hull_fx, hull_fy) = resolve(0.0, r_friction + r_residuary + r_heel, flow_angle);

        let resistance = r_friction + r_residuary + r_heel + lp.drag + lp_induced + rud.drag;

        // --- Engine ---------------------------------------------------------------
        let thrust = controls.throttle.clamp(-1.0, 1.0) * cfg.max_thrust;

        // --- Body-frame forces ----------------------------------------------------
        let fx = drive + thrust + hull_fx + lp_fx + rud_fx;
        let fy = sail_side + hull_fy + lp_fy + rud_fy;

        // --- Yaw moment -----------------------------------------------------------
        // Rig side force acting at its fore-and-aft centre of effort. The lead of the
        // centre of effort ahead of the centre of lateral resistance is a lee-helm
        // term; heel pays it back with interest.
        let mut mz = cfg.main.x_ce * main.side + cfg.genoa.x_ce * jib.side;
        // Heel swings the centre of effort out to leeward, so the driving force now
        // acts on a lever arm and luffs the boat up. This is weather helm.
        let y_ce_main = main_z_ce * st.heel.sin();
        let y_ce_jib = jib_z_ce * st.heel.sin();
        mz += -(y_ce_main * main.drive + y_ce_jib * jib.drive);
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

        // --- Roll moment ----------------------------------------------------------
        // Heeling arm is the centre of effort above the waterline plus the centre of
        // lateral resistance below it.
        let mut mx = main.side * (main_z_ce * heel_cos + cfg.z_clr)
            + jib.side * (jib_z_ce * heel_cos + cfg.z_clr);
        mx -= cfg.displacement * GRAVITY * righting_arm(st.heel, cfg.gm, cfg.gz_peak_angle);
        let roll_stiffness = cfg.displacement * GRAVITY * cfg.gm;
        let critical = 2.0 * (cfg.roll_inertia * roll_stiffness).sqrt();
        mx -= cfg.roll_damping * critical * st.roll_rate;
        mx -= 0.06 * critical * st.roll_rate * st.roll_rate.abs();

        // --- Integrate ------------------------------------------------------------
        let m_x = cfg.displacement + cfg.surge_added_mass;
        let m_y = cfg.displacement + cfg.sway_added_mass;

        let du = fx / m_x + st.sway * st.yaw_rate;
        let dv = fy / m_y - st.surge * st.yaw_rate;
        let dr = mz / cfg.yaw_inertia;
        let dp = mx / cfg.roll_inertia;

        let s = &mut self.state;
        s.surge += du * dt;
        s.sway += dv * dt;
        s.yaw_rate += dr * dt;
        s.roll_rate += dp * dt;
        s.heading = wrap_tau(s.heading + s.yaw_rate * dt);
        s.heel += s.roll_rate * dt;
        s.heel = s.heel.clamp(-1.4, 1.4);
        s.pos += s.velocity_world() * dt;
        s.time += dt;

        // --- Telemetry ------------------------------------------------------------
        let to_wind = DVec2::new(twd.sin(), twd.cos());
        let wind_body = self.state.to_body(to_wind);
        let v = self.state.velocity_world();
        self.telemetry = Telemetry {
            tws,
            twd,
            twa: wrap_pi(wind_body.y.atan2(wind_body.x)),
            aws: main_aws,
            awa: main_awa,
            stw: self.state.speed(),
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
            jib,
            drive,
            side: sail_side,
            resistance,
            resistance_friction: r_friction,
            resistance_residuary: r_residuary,
            resistance_induced: lp_induced,
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
