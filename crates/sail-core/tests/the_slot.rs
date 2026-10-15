//! What the mainsail does to the headsail's wind, and what it must not do when it is not
//! there.

use sail_core::prelude::*;

/// Put her close-hauled with way on, hold her there for half a second, and read what the
/// headsail is doing.
///
/// Half a second rather than a minute on purpose: the question is what the *boom* does
/// to the headsail's wind, and a boat sailed for a minute under two different mainsheets
/// is in two different places at two different speeds, so the headsail's drive differs
/// for a dozen honest reasons. Held at one state, only the slot is left.
fn at_this_moment(main_sheet: f64, reef: f64, furl: f64) -> (f64, f64, f64) {
    let cfg = BoatConfig::archipelago_packet();
    let mut start = BoatState::at_rest(wrap_tau(0.0 - rad(45.0)));
    start.surge = kn_to_ms(5.0);
    let mut sim = Simulation::new(
        cfg,
        start,
        Box::new(SteadyWind::new(0.0, kn_to_ms(14.0))) as Box<dyn WindField>,
    );
    let controls = Controls {
        main_sheet,
        jib_sheet: 0.16,
        reef,
        furl,
        ..Default::default()
    };
    for _ in 0..(0.5 / SUBSTEP) as u64 {
        sim.run(SUBSTEP, &controls);
    }
    (
        sim.telemetry.jib.drive,
        sim.telemetry.jib.coeffs.cl,
        sim.telemetry.bend_jib,
    )
}

/// **A mainsail that is not there does not bend the headsail's wind.**
///
/// Reported from the deck: with no main set, hauling the boom to the centreline made the
/// genoa go soft, and the cure was to trim the boom *as if the main were there* — which
/// is a boat taking orders from a sail that is rolled up on the boom.
///
/// The mechanism is real and the bookkeeping was not. The slot works on the lift each
/// sail makes, and a lift coefficient knows nothing about whether the sail exists: a
/// main rolled away to a scrap still reports a perfectly good `cl`, so its upwash went
/// on turning the headsail's wind by three degrees. The same mistake was found and
/// fixed once before, a dozen lines further down, for whether the slot is open at all.
#[test]
fn a_main_that_is_not_set_does_not_touch_the_headsail() {
    let eased = at_this_moment(0.9, 1.0, 0.0);
    let hard = at_this_moment(0.05, 1.0, 0.0);
    let bend = (eased.2 - hard.2).abs();
    assert!(
        deg(bend) < 0.5,
        "with the main rolled away the boom must not bend the headsail's wind: \
         {:.1}° of difference between a boom eased and a boom amidships",
        deg(bend)
    );
    let drive = (eased.0 - hard.0).abs() / eased.0.abs().max(1.0);
    assert!(
        drive < 0.05,
        "and the headsail must not care where the boom is: {:.0}% of its drive changed",
        drive * 100.0
    );
}

/// **With the main set, hauling it in helps the headsail rather than killing it.**
///
/// The other half, and the half that must keep working: the mainsail's circulation turns
/// the flow arriving at the headsail to windward, so the headsail meets it at a larger
/// angle. That is the thing a trimmer feels — haul the main and the headsail works
/// harder without anybody touching its sheet — and it is why the upwash is in the model
/// at all.
#[test]
fn a_main_that_is_set_upwashes_the_headsail() {
    let eased = at_this_moment(0.9, 0.0, 0.0);
    let hard = at_this_moment(0.08, 0.0, 0.0);
    assert!(
        hard.2 > eased.2,
        "a main sheeted in bends more wind into the headsail: {:.1}° against {:.1}°",
        deg(hard.2),
        deg(eased.2)
    );
    assert!(
        hard.1 > eased.1 * 0.9,
        "so the headsail is not what gives when the main comes in: cl {:.2} against {:.2}",
        hard.1,
        eased.1
    );
}

/// **Three slabs are three different mainsails, and then she is handed.**
///
/// The step from the second reef to no main at all is the one you need in the forties,
/// which is where a cruising main has a third row of points sewn into it. Each slab has
/// to take a real bite out of her — a reef that changes the area by a few per cent is a
/// reef nobody would bother going forward for.
#[test]
fn each_slab_takes_a_real_bite_out_of_her() {
    let cfg = BoatConfig::archipelago_packet();
    let full = cfg.reefed_main(0.0).0;
    let areas: Vec<f64> = [0.25, 0.5, 0.75]
        .iter()
        .map(|reef| cfg.reefed_main(*reef).0)
        .collect();
    assert!(
        cfg.reefed_main(1.0).0 < 1e-9,
        "the last position on the halyard is the sail coming down"
    );
    let mut was = full;
    for (slab, area) in areas.iter().enumerate() {
        let off = (was - area) / full;
        assert!(
            (0.10..0.25).contains(&off),
            "slab {} takes {:.0}% of her off, which is not a reef",
            slab + 1,
            off * 100.0
        );
        was = *area;
    }
    assert!(
        areas[2] / full < 0.55,
        "down to the third she is half the sail she was: {:.0}%",
        areas[2] / full * 100.0
    );
}
