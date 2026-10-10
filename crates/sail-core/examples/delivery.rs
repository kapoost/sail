//! Is this boat pleasant to carry parcels around an archipelago in?
//!
//! A speed polar answers "how fast at this angle", which is the wrong question for a
//! game of short legs between islands. What is actually felt there is different:
//!
//! * **Holes.** Every point of sail gets used within the hour, so an angle the boat
//!   is bad at is met constantly rather than avoided. One number for the worst angle
//!   beats fifteen numbers for all of them.
//! * **Tacking tax.** In a channel, the beat angle decides how many times you work
//!   the boat to get somewhere. Miles sailed per mile made good is the honest cost.
//!   The same applies running, if the boat is slow enough dead downwind to be worth
//!   gybing instead.
//! * **The loop.** A delivery round is a closed circuit that visits every quarter of
//!   the compass and comes home. Its elapsed time is the only figure that integrates
//!   all of the above, and it is the one the player experiences.
//!
//! Compared against the charter sloop throughout, because "better" is a comparison
//! and a boat tuned in isolation is a boat tuned to nothing.
//!
//!   cargo run -p sail-core --release --example delivery

use sail_core::prelude::*;
use sail_core::water::WindSea;
use sail_core::wind::SteadyWind;

const TARGET_HEEL_DEG: f64 = 22.0;
const SETTLE: f64 = 420.0;
/// Legs of a delivery round, nautical miles each, laid out as a closed polygon.
const LEG_MILES: f64 = 4.0;
/// How many legs the round has. Twelve gives a bearing every thirty degrees, so the
/// circuit genuinely visits every point of sail rather than sampling four of them.
const LEGS: usize = 12;

fn main() {
    let boats = [
        BoatConfig::archipelago_packet(),
        BoatConfig::charter_sloop_34(),
    ];
    println!(
        "A delivery round: {LEGS} legs of {LEG_MILES:.0} nm, bearings evenly round the compass,"
    );
    println!("sailed the fast way — tacking or gybing wherever the rhumb line is slower.\n");

    for cfg in boats {
        println!(
            "{} — LOA {:.2} m, beam {:.2} m (L/B {:.2}), disp {:.0} kg, sail {:.0} m2, hull speed {:.2} kn",
            cfg.name,
            cfg.loa,
            cfg.beam,
            cfg.loa / cfg.beam,
            cfg.displacement,
            cfg.sail_area(),
            ms_to_kn(cfg.hull_speed())
        );
        for (tws, sea) in [
            (7.0, false),
            (7.0, true),
            (12.0, false),
            (12.0, true),
            (18.0, false),
            (18.0, true),
        ] {
            let p = polar(cfg, tws, sea);
            let round = round_hours(&p);
            println!(
                "  {tws:>2.0} kn {:<5}  beat {:>4.0}° at {:>4.2} kn VMG ({:.2} miles sailed per mile won)  \
                 run {:>4.0}° {:>4.2} kn  best {:>4.2} kn  worst usable {:>4.0}° {:>4.2} kn  \
                 round {:>5.2} h",
                if sea { "sea" } else { "flat" },
                deg(p.beat_angle),
                ms_to_kn(p.beat_vmg),
                1.0 / p.beat_angle.cos().max(1e-3),
                deg(p.run_angle),
                ms_to_kn(p.run_vmg),
                ms_to_kn(p.best),
                deg(p.worst_angle),
                ms_to_kn(p.worst),
                round
            );
        }
        println!();
    }
    println!("`worst usable` is the slowest angle outside the no-go zone, as a check for holes.");
    println!("`round` counts only sailing time — no manoeuvres, no harbours, no tide.");
}

struct Polar {
    /// TWA and speed at each sampled angle.
    points: Vec<(f64, f64)>,
    beat_angle: f64,
    beat_vmg: f64,
    run_angle: f64,
    run_vmg: f64,
    best: f64,
    worst_angle: f64,
    worst: f64,
}

fn polar(cfg: BoatConfig, tws_knots: f64, sea: bool) -> Polar {
    let mut points = Vec::new();
    let mut twa = 25.0;
    while twa <= 180.5 {
        points.push((rad(twa), sail_at(cfg, tws_knots, rad(twa), sea)));
        twa += 5.0;
    }

    // Upwind: the angle whose speed times cos is greatest. Downwind the same with the
    // sign flipped, which is what makes gybing downwind worth doing at all.
    let (mut beat_angle, mut beat_vmg) = (rad(45.0), 0.0);
    let (mut run_angle, mut run_vmg) = (rad(180.0), 0.0);
    for &(a, v) in &points {
        if v * a.cos() > beat_vmg {
            beat_vmg = v * a.cos();
            beat_angle = a;
        }
        if -v * a.cos() > run_vmg {
            run_vmg = -v * a.cos();
            run_angle = a;
        }
    }
    let best = points.iter().fold(0.0_f64, |m, &(_, v)| m.max(v));
    // "Usable" starts at the beat angle: inside it the boat is not slow, it is simply
    // not going that way, and counting the no-go zone as a hole would be dishonest.
    let (mut worst_angle, mut worst) = (rad(90.0), f64::INFINITY);
    for &(a, v) in &points {
        if a >= beat_angle && v < worst {
            worst = v;
            worst_angle = a;
        }
    }
    Polar {
        points,
        beat_angle,
        beat_vmg,
        run_angle,
        run_vmg,
        best,
        worst_angle,
        worst,
    }
}

/// Settle the boat at one wind angle and report her speed.
///
/// `reef_for_heel` would be the obvious tool and it is the wrong one here: it only
/// takes a wind field, and the whole question is what the sea does. So the reef is
/// chosen the way a crew chooses it — try full sail, and shorten down a step at a
/// time until she stands up — and the boat is then sailed in the real conditions.
fn sail_at(cfg: BoatConfig, tws_knots: f64, twa: f64, sea: bool) -> f64 {
    let mut best = 0.0_f64;
    for step in 0..=4 {
        let reef = step as f64 / 4.0;
        let (stw, heel, held) = settle(cfg, tws_knots, twa, reef, sea);
        if !held {
            continue;
        }
        if heel.abs() <= rad(TARGET_HEEL_DEG) || step == 4 {
            return stw;
        }
        best = best.max(stw);
    }
    best
}

fn settle(cfg: BoatConfig, tws_knots: f64, twa: f64, reef: f64, sea: bool) -> (f64, f64, bool) {
    let heading = wrap_tau(-twa);
    let mut state = BoatState::at_rest(heading);
    state.surge = 0.5 * cfg.hull_speed();
    let mut sim = Simulation::new(
        cfg,
        state,
        Box::new(SteadyWind::new(0.0, kn_to_ms(tws_knots))) as Box<dyn WindField>,
    );
    if sea {
        // A working archipelago fetch: islands to windward most of the time.
        sim = sim.with_water(Box::new(WindSea::new(
            kn_to_ms(tws_knots),
            60_000.0,
            0.0,
            0x5EA,
        )));
    }
    let mut pilot = TillerPilot::new();
    let mut c = Controls {
        reef,
        ..Default::default()
    };
    // Averaged over the last two minutes, not sampled at the end. In a seaway the
    // instantaneous speed swings with the wave period, so one sample is a coin toss
    // worth most of a knot — which is how a boat came out *faster* in waves than in
    // flat water and briefly looked like a physics bug.
    let steps = (SETTLE / SUBSTEP) as u64;
    let average_from = steps - (120.0 / SUBSTEP) as u64;
    let (mut sum, mut heel_sum, mut n) = (0.0, 0.0, 0.0);
    for i in 0..steps {
        let (m, j) = optimal_sheets(&cfg, sim.telemetry.awa, c.outhaul);
        c.main_sheet = m;
        c.jib_sheet = j;
        c.rudder = pilot.rudder(&cfg, &sim.state, heading, SUBSTEP);
        sim.run(SUBSTEP, &c);
        if i >= average_from {
            sum += sim.telemetry.stw;
            heel_sum += sim.state.heel;
            n += 1.0;
        }
    }
    let held = wrap_pi(sim.state.heading - heading).abs() < rad(5.0);
    (sum / n, heel_sum / n, held)
}

/// Hours to sail the closed round.
///
/// For each leg, the boat takes whichever is quicker: the rhumb line at the speed the
/// polar gives, or working to windward (or downwind) at the best VMG. That is what a
/// skipper does, so it is what the figure of merit should assume.
fn round_hours(p: &Polar) -> f64 {
    let mut hours = 0.0;
    for leg in 0..LEGS {
        let twa = (leg as f64) * std::f64::consts::TAU / (LEGS as f64);
        // Fold onto 0..180: a bearing either side of the wind is the same sailing.
        let twa = wrap_pi(twa).abs();
        let direct = interpolate(&p.points, twa);
        let made_good = if twa < p.beat_angle {
            p.beat_vmg / twa.cos().max(1e-3)
        } else if twa > p.run_angle {
            p.run_vmg / (-twa.cos()).max(1e-3)
        } else {
            direct
        }
        .max(direct);
        hours += LEG_MILES * 1852.0 / made_good.max(0.05) / 3600.0;
    }
    hours
}

fn interpolate(points: &[(f64, f64)], twa: f64) -> f64 {
    if twa <= points[0].0 {
        return points[0].1;
    }
    for w in points.windows(2) {
        if twa <= w[1].0 {
            let t = (twa - w[0].0) / (w[1].0 - w[0].0);
            return w[0].1 + (w[1].1 - w[0].1) * t;
        }
    }
    points[points.len() - 1].1
}
