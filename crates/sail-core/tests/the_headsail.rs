//! The five seconds of a tack the model used to skip.

use sail_core::prelude::*;

/// Through the wind, and the headsail has to make the crossing with her.
///
/// A tack is not the wind crossing the bow. It is what comes after: the jib backs and
/// stops her, somebody lets the sheet fly, the sail thrashes across the foretriangle, and
/// the new sheet is ground on until it fills. The model had the first instant and none of
/// the rest — the sail was simply on the other side, in the same frame, drawing.
#[test]
fn the_headsail_crosses_rather_than_appearing_on_the_other_side() {
    let crossing = |tws_kn: f64| -> (f64, f64, f64) {
        let cfg = BoatConfig::archipelago_packet();
        let tws = kn_to_ms(tws_kn);
        // With way on. From rest with her sheets in she sits in irons, which is correct
        // and is a different test.
        let mut start = BoatState::at_rest(wrap_tau(0.0 - rad(45.0)));
        start.surge = kn_to_ms(5.5);
        let mut sim = Simulation::new(
            cfg,
            start,
            Box::new(SteadyWind::new(0.0, tws)) as Box<dyn WindField>,
        );
        let mut controls = Controls {
            main_sheet: 0.12,
            jib_sheet: 0.1,
            ..Default::default()
        };
        for _ in 0..(60.0 / SUBSTEP) as u64 {
            let off = deg(sim.telemetry.twa) - 45.0;
            controls.rudder = rad((off * 0.5).clamp(-15.0, 15.0));
            sim.run(SUBSTEP, &controls);
        }
        let driving = sim.telemetry.jib.drive;
        // Helm down and through the wind.
        controls.rudder = rad(25.0);
        let (mut t, mut began, mut done) = (0.0, None, None);
        let (mut was, mut biggest_step, mut least) = (sim.telemetry.jib_side, 0.0f64, f64::MAX);
        while t < 25.0 {
            sim.run(SUBSTEP, &controls);
            t += SUBSTEP;
            let now = sim.telemetry.jib_side;
            if began.is_none() && (now - was).abs() > 1e-9 {
                began = Some(t);
            }
            if let (Some(start), None) = (began, done) {
                biggest_step = biggest_step.max((now - was).abs());
                least = least.min(sim.telemetry.jib.drive);
                if now * was < 0.0 || (now.abs() > 0.995 && t - start > 0.3) {
                    done = Some(t - start);
                }
            }
            was = now;
        }
        (
            done.unwrap_or(f64::NAN),
            biggest_step,
            least / driving.max(1.0),
        )
    };

    let (light, light_step, light_drive) = crossing(8.0);
    let (fresh, _, _) = crossing(14.0);
    println!("headsail across: {light:.2} s in eight knots, {fresh:.2} s in fourteen");

    // It takes time, and it is quicker in more wind — the air blows it across and a crew
    // in a breeze is in more of a hurry.
    assert!(
        (0.4..4.0).contains(&light),
        "the sail should take a second or two to get across in eight knots: {light:.2}"
    );
    assert!(
        fresh < light,
        "and less in fourteen: {fresh:.2} against {light:.2}"
    );
    // And it gets there by moving. A single frame from one side to the other is the
    // fault this is here for: at two hundred substeps a second, crossing in two seconds
    // is a hundredth of the span per step.
    assert!(
        light_step < 0.05,
        "the clew jumped {light_step:.3} of the foretriangle in one substep"
    );
    // While it is across the middle it is cloth, not a sail.
    assert!(
        light_drive < 0.25,
        "the headsail should make nothing while it thrashes across: {light_drive:.2} of \
         what it was pulling"
    );
}
