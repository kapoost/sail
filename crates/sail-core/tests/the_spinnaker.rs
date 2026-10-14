//! The symmetric kite, and what it is for.

use sail_core::prelude::*;
use sail_core::state::Headsail;

fn run_at(twa: f64, sail: Headsail, tws_kn: f64) -> f64 {
    let cfg = BoatConfig::archipelago_packet();
    let mut start = BoatState::at_rest(wrap_tau(0.0 - rad(twa)));
    start.surge = kn_to_ms(4.0);
    let mut sim = Simulation::new(
        cfg,
        start,
        Box::new(SteadyWind::new(0.0, kn_to_ms(tws_kn))) as Box<dyn WindField>,
    );
    let mut c = Controls {
        headsail: sail,
        main_sheet: 0.9,
        jib_sheet: 0.75,
        furl: 0.0,
        ..Default::default()
    };
    for _ in 0..(90.0 / SUBSTEP) as u64 {
        let off = deg(sim.telemetry.twa) - twa;
        c.rudder = rad((off * 0.5).clamp(-15.0, 15.0));
        sim.run(SUBSTEP, &c);
    }
    ms_to_kn(sim.telemetry.stw)
}

/// A spinnaker is for running deep, and that is a measurable claim rather than a
/// description: against the asymmetric it should be *worse* on a reach and *better* on
/// a run, because the pole takes it out of the mainsail's shadow and the asymmetric has
/// no pole.
///
/// If this ever fails in the reaching direction, the sail has been given the gennaker's
/// strengths as well as its own and is simply a bigger sail rather than a different one.
#[test]
fn the_pole_is_what_the_spinnaker_is_for() {
    let deep_kite = run_at(165.0, Headsail::Spinnaker, 12.0);
    let deep_gen = run_at(165.0, Headsail::Gennaker, 12.0);
    // Measured: 7.83 against 7.02 knots in twelve of true wind.
    assert!(
        deep_kite > deep_gen,
        "dead downwind the symmetric should beat the asymmetric: {deep_kite:.2} against \
         {deep_gen:.2} kn"
    );

    // And on a reach it is the asymmetric's water. A symmetric cannot be sheeted in that
    // far and has no luff to hold a leading edge with.
    let reach_kite = run_at(100.0, Headsail::Spinnaker, 12.0);
    let reach_gen = run_at(100.0, Headsail::Gennaker, 12.0);
    // Measured: 5.82 against 5.10 the other way round.
    assert!(
        reach_gen > reach_kite,
        "on a reach the asymmetric should beat the symmetric: {reach_gen:.2} against \
         {reach_kite:.2} kn"
    );
}

/// And it is the first thing to go. Half-ounce cloth with no luff tension in it: the
/// spinnaker's number is the lowest on the boat, below even the gennaker's.
#[test]
fn the_kite_is_the_most_fragile_thing_aboard() {
    let cfg = BoatConfig::archipelago_packet();
    let kite = cfg.headsail(Headsail::Spinnaker).limit_aws;
    let gennaker = cfg.headsail(Headsail::Gennaker).limit_aws;
    let genoa = cfg.headsail(Headsail::Genoa).limit_aws;
    assert!(
        kite < gennaker && gennaker < genoa,
        "spinnaker {:.0} kn, gennaker {:.0}, genoa {:.0}",
        ms_to_kn(kite),
        ms_to_kn(gennaker),
        ms_to_kn(genoa)
    );
    // And it is the biggest.
    assert!(
        cfg.headsail(Headsail::Spinnaker).area > cfg.headsail(Headsail::Gennaker).area,
        "a symmetric kite is the largest sail in the wardrobe"
    );
}
