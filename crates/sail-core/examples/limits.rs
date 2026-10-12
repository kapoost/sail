//! Where the boat stops being manageable: helm, heel, leeway, and whether she can be
//! made to broach and knocked down.
//!
//! The question this answers is not "is the model stable" — a model that cannot be put
//! on its ear is a model that is lying, because a real boat with too much sail up and a
//! squall on the beam goes over. What matters is that she goes over for the right
//! reasons and in the right order: pressed, heeled, the rudder emerging, the hull
//! driving her up into the wind, and the helm gone before the heel is.
//!
//!   cargo run -p sail-core --release --example limits

use sail_core::autopilot::{optimal_sheets_with, TillerPilot};
use sail_core::prelude::*;
use sail_core::state::Headsail;
use sail_core::wind::{flow_from, SteadyWind, WindField};

/// A steady wind with one gust in it, so the same squall can be run at every sail.
struct Squall {
    from: f64,
    base: f64,
    peak: f64,
    start: f64,
    rise: f64,
    hold: f64,
}

impl WindField for Squall {
    fn sample(&self, _pos: glam::DVec2, t: f64) -> glam::DVec2 {
        let since = t - self.start;
        let strength = if since < 0.0 {
            0.0
        } else if since < self.rise {
            since / self.rise
        } else if since < self.rise + self.hold {
            1.0
        } else {
            (1.0 - (since - self.rise - self.hold) / self.rise).max(0.0)
        };
        flow_from(self.from, self.base + (self.peak - self.base) * strength)
    }

}

struct Run {
    max_heel: f64,
    max_rudder: f64,
    max_off_course: f64,
    stalled_for: f64,
    ended_heel: f64,
    ended_speed: f64,
}

/// Hold a course with the pilot and the best trim, and report what she did.
// A squall has a lot of knobs on it and they are all the point.
#[allow(clippy::too_many_arguments)]
fn sail_through(
    cfg: BoatConfig,
    wind: Box<dyn WindField>,
    twa: f64,
    seconds: f64,
    head: Headsail,
    reef: f64,
    trim_for: Option<f64>,
    steered: bool,
) -> Run {
    let sample = wind.sample(glam::DVec2::ZERO, 0.0);
    let heading = wrap_tau(sail_core::wind::direction_of(sample) - twa);
    let mut state = BoatState::at_rest(heading);
    state.surge = 0.6 * cfg.hull_speed();
    // In a seaway, because that is where a boat broaches. On flat water with a steady
    // breeze there is nothing to start her rolling, and the roll is what the rig
    // couples to — a broach test on a millpond tests nothing.
    // A sea to match the squall rather than the lull: a forty-knot gust does not
    // arrive on a twenty-knot sea, and the wave that rolls her is half the story.
    let sea = sail_core::water::WindSea::new(kn_to_ms(32.0), 60_000.0, 0.0, 9);
    let mut sim = Simulation::new(cfg, state, wind).with_water(Box::new(sea));
    let mut pilot = TillerPilot::new();
    let mut controls = Controls {
        reef,
        headsail: head,
        furl: 0.0,
        ..Default::default()
    };
    let mut out = Run {
        max_heel: 0.0,
        max_rudder: 0.0,
        max_off_course: 0.0,
        stalled_for: 0.0,
        ended_heel: 0.0,
        ended_speed: 0.0,
    };
    // Settle, trim for the lull, and then cleat it. This is the whole point of the
    // test: a sheet that re-trims itself to the gust every two hundredth of a second is
    // a crew of eight with the sheets in their hands, and it is why a simulated boat
    // never broaches. A real one has them on the winch with a turn round the cleat and
    // somebody has to get there.
    let settle = (30.0 / SUBSTEP) as u64;
    for i in 0..(seconds / SUBSTEP) as u64 {
        if i <= settle {
            let awa = trim_for.unwrap_or(sim.telemetry.awa);
            let (m, j) = optimal_sheets_with(&cfg, awa, controls.outhaul, head);
            controls.main_sheet = m;
            controls.jib_sheet = j;
        }
        // Steered, or with the helm lashed amidships. The second is the honest test of
        // whether a knockdown is in the model at all: left to herself an overpressed
        // boat rounds up and spills, which saves her, and it is the saving that has to
        // be ruled out before anyone can say she cannot be put over.
        controls.rudder = if steered {
            pilot.correct_under_load(
                &cfg,
                wrap_pi(heading - sim.state.heading),
                sim.state.yaw_rate,
                sim.telemetry.weather_helm,
                SUBSTEP,
            )
        } else {
            0.0
        };
        sim.run(SUBSTEP, &controls);
        out.max_heel = out.max_heel.max(sim.state.heel.abs());
        out.max_rudder = out.max_rudder.max(controls.rudder.abs());
        out.max_off_course = out
            .max_off_course
            .max(wrap_pi(sim.state.heading - heading).abs());
        if sim.telemetry.rudder_stalled > 0.5 {
            out.stalled_for += SUBSTEP;
        }
    }
    out.ended_heel = sim.state.heel.abs();
    out.ended_speed = sim.telemetry.stw;
    out
}

/// Run one squall and print the row.
fn squall_row(cfg: BoatConfig, peak_kn: f64, twa: f64, head: Headsail, reef: f64, steered: bool) {
    let wind = Box::new(Squall {
        from: 0.0,
        base: kn_to_ms(16.0),
        peak: kn_to_ms(peak_kn),
        start: 60.0,
        rise: 12.0,
        hold: 25.0,
    });
    let run = sail_through(cfg, wind, rad(twa), 160.0, head, reef, None, steered);
    println!(
        "{twa:>5.0}{:>10}{:>7.2}{:>9.1}{:>9.1}{:>10.0}{:>9.1}{:>9.2}",
        head.name(),
        reef,
        deg(run.max_heel),
        deg(run.max_rudder),
        deg(run.max_off_course),
        run.stalled_for,
        ms_to_kn(run.ended_speed),
    );
}

fn main() {
    let cfg = BoatConfig::archipelago_packet();

    // --- What the boom does in a seaway --------------------------------------
    println!("-- the boom against its sheet: flat water against a sea --");
    println!(
        "{:>4}{:>6}{:>8}{:>10}{:>12}{:>10}",
        "TWA", "wind", "sea", "STW", "boom swing", "slack for"
    );
    for (twa_deg, tws) in [(42.0, 10.0), (42.0, 16.0), (135.0, 10.0), (135.0, 16.0), (150.0, 16.0)] {
        for (label, hs) in [("flat", 0.0), ("sea", 1.2)] {
            let wind = || -> Box<dyn WindField> { Box::new(SteadyWind::new(0.0, kn_to_ms(tws))) };
            let heading = wrap_tau(sail_core::wind::direction_of(
                wind().sample(glam::DVec2::ZERO, 0.0),
            ) - rad(twa_deg));
            let mut state = BoatState::at_rest(heading);
            state.surge = 0.6 * cfg.hull_speed();
            let mut sim = Simulation::new(cfg, state, wind());
            if hs > 0.0 {
                sim = sim.with_water(Box::new(sail_core::water::WindSea::new(
                    kn_to_ms(tws),
                    60_000.0,
                    0.0,
                    5,
                )));
            }
            let mut pilot = TillerPilot::new();
            let mut controls = Controls::default();
            let mut swing: f64 = 0.0;
            let mut slack = 0.0;
            let steps = (240.0 / SUBSTEP) as u64;
            for i in 0..steps {
                let (m, j) = optimal_sheets_with(
                    &cfg,
                    sim.telemetry.awa,
                    controls.outhaul,
                    Headsail::Genoa,
                );
                controls.main_sheet = m;
                controls.jib_sheet = j;
                controls.rudder = pilot.correct_under_load(
                    &cfg,
                    wrap_pi(heading - sim.state.heading),
                    sim.state.yaw_rate,
                    sim.telemetry.weather_helm,
                    SUBSTEP,
                );
                sim.run(SUBSTEP, &controls);
                if i > steps / 3 {
                    let set = cfg.main.profile.chord_angle(controls.main_sheet);
                    let off = sim.state.boom_angle - set;
                    swing = swing.max(off.abs());
                    if off < -rad(0.5) {
                        slack += SUBSTEP;
                    }
                }
            }
            println!(
                "{twa_deg:>4.0}{tws:>6.0}{label:>8}{:>10.2}{:>11.1}deg{:>9.1}s",
                ms_to_kn(sim.telemetry.stw),
                deg(swing),
                slack,
            );
        }
    }
    println!();

    // --- Helm, heel and leeway, which is the everyday question ---------------
    println!("-- holding a course, full sail, what the helm and the hull are doing --");
    println!(
        "{:>5}{:>6}{:>8}{:>8}{:>9}{:>9}{:>8}",
        "TWA", "wind", "STW", "heel", "leeway", "helm", "stall"
    );
    for twa in [40.0, 60.0, 90.0, 135.0] {
        for tws in [8.0, 14.0, 20.0, 26.0] {
            let wind = Box::new(SteadyWind::new(0.0, kn_to_ms(tws)));
            let sample = wind.sample(glam::DVec2::ZERO, 0.0);
            let heading = wrap_tau(sail_core::wind::direction_of(sample) - rad(twa));
            let mut state = BoatState::at_rest(heading);
            state.surge = 0.6 * cfg.hull_speed();
            let mut sim = Simulation::new(cfg, state, wind);
            let mut pilot = TillerPilot::new();
            let mut controls = Controls::default();
            for _ in 0..(300.0 / SUBSTEP) as u64 {
                let (m, j) = optimal_sheets_with(
                    &cfg,
                    sim.telemetry.awa,
                    controls.outhaul,
                    Headsail::Genoa,
                );
                controls.main_sheet = m;
                controls.jib_sheet = j;
                controls.rudder = pilot.correct_under_load(
                    &cfg,
                    wrap_pi(heading - sim.state.heading),
                    sim.state.yaw_rate,
                    sim.telemetry.weather_helm,
                    SUBSTEP,
                );
                sim.run(SUBSTEP, &controls);
            }
            let t = &sim.telemetry;
            println!(
                "{twa:>5.0}{tws:>6.0}{:>8.2}{:>8.1}{:>9.2}{:>9.1}{:>8.2}  awa {:>4.0}  helm load {:>8.0} Nm",
                ms_to_kn(sim.telemetry.stw),
                deg(sim.state.heel).abs(),
                deg(sim.telemetry.leeway).abs(),
                deg(controls.rudder),
                sim.telemetry.rudder_stalled,
                deg(t.awa).abs(),
                t.weather_helm,
            );
        }
    }

    // --- The squall ----------------------------------------------------------
    println!("\n-- a squall: {} kn rising to {} kn over 12 s, trimmed for the lull --", 16, 34);
    println!(
        "{:>5}{:>10}{:>7}{:>9}{:>9}{:>10}{:>9}{:>9}",
        "TWA", "sail", "reef", "max heel", "max helm", "off course", "stalled", "end spd"
    );
    for (twa, head, reef) in [
        (90.0, Headsail::Genoa, 0.0),
        (90.0, Headsail::Genoa, 1.0),
        (90.0, Headsail::Genoa, 2.0 / 3.0),
        (90.0, Headsail::Code0, 0.0),
        (135.0, Headsail::Gennaker, 0.0),
        (150.0, Headsail::Gennaker, 0.0),
    ] {
        let wind = Box::new(Squall {
            from: 0.0,
            base: kn_to_ms(16.0),
            peak: kn_to_ms(34.0),
            start: 60.0,
            rise: 12.0,
            hold: 25.0,
        });
        // Trimmed for the apparent wind she had in the lull and left there.
        let run = sail_through(cfg, wind, rad(twa), 160.0, head, reef, None, true);
        println!(
            "{twa:>5.0}{:>10}{:>7.2}{:>9.1}{:>9.1}{:>10.0}{:>9.1}{:>9.2}",
            head.name(),
            reef,
            deg(run.max_heel),
            deg(run.max_rudder),
            deg(run.max_off_course),
            run.stalled_for,
            ms_to_kn(run.ended_speed),
        );
    }

    // And the one nobody sails through: a white squall, full sail, no warning.
    println!("\n-- 16 kn to 48 kn, which is a knockdown or it is not a model --");
    println!(
        "{:>5}{:>10}{:>7}{:>9}{:>9}{:>10}{:>9}{:>9}",
        "TWA", "sail", "reef", "max heel", "max helm", "off course", "stalled", "end spd"
    );
    for (twa, head, reef) in [
        (90.0, Headsail::Genoa, 0.0),
        (110.0, Headsail::Code0, 0.0),
        (135.0, Headsail::Gennaker, 0.0),
    ] {
        squall_row(cfg, 48.0, twa, head, reef, true);
    }
    println!("\n-- the same, with the helm lashed amidships: can she be put over at all --");
    println!(
        "{:>5}{:>10}{:>7}{:>9}{:>9}{:>10}{:>9}{:>9}",
        "TWA", "sail", "reef", "max heel", "max helm", "off course", "stalled", "end spd"
    );
    for (peak, twa, head) in [
        (34.0, 90.0, Headsail::Genoa),
        (48.0, 90.0, Headsail::Genoa),
        (62.0, 90.0, Headsail::Genoa),
        (48.0, 135.0, Headsail::Gennaker),
    ] {
        print!("{peak:>3.0}kn ");
        squall_row(cfg, peak, twa, head, 0.0, false);
    }

}
