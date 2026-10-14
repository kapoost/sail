//! The one thing the deck noticed twice.

use sail_core::prelude::*;

/// The drawn boom must never jump, however fast the wind line is crossed.
///
/// Reported from the deck as "it still jumps", and then — the observation that found it —
/// "maybe it depends on how fast you cross the wind line; by hand on the helm I can catch
/// the transit I want". Which was exactly right. The simulation flips the boom's side ten
/// degrees *past* dead astern, with hysteresis so that a boat squared off downwind does
/// not flicker; the drawing was re-deriving the side from the raw apparent wind, which
/// flips at a hundred and eighty. For as long as the apparent wind spent between those
/// two angles the two disagreed and the drawn spar was on the wrong side of the boat.
/// Crossed slowly that is a second of something odd; crossed hard it is a teleport.
///
/// So this measures what the renderer sees, substep by substep, through the hardest gybe
/// the boat can be made to do: helm hard over, in a seaway, at two wind speeds.
#[test]
fn the_drawn_boom_never_jumps() {
    for (tws_kn, rudder) in [(10.0, 35.0), (16.0, 35.0)] {
        let cfg = BoatConfig::archipelago_packet();
        let tws = kn_to_ms(tws_kn);
        let sea = sail_core::water::WindSea::new(tws, 14_000.0, 0.0, 0xC0FFEE);
        let mut sim = Simulation::new(
            cfg,
            BoatState::at_rest(wrap_tau(0.0 - rad(165.0))),
            Box::new(SteadyWind::new(0.0, tws)) as Box<dyn WindField>,
        )
        .with_water(Box::new(sea));
        let mut controls = Controls {
            main_sheet: 0.95,
            jib_sheet: 0.85,
            ..Default::default()
        };
        for _ in 0..(60.0 / SUBSTEP) as u64 {
            let off = deg(sim.telemetry.twa) - 165.0;
            controls.rudder = rad((off * 0.5).clamp(-25.0, 25.0));
            sim.run(SUBSTEP, &controls);
        }
        controls.rudder = rad(-rudder);
        let (mut worst, mut at, mut was, mut t) = (0.0f64, 0.0, sim.telemetry.boom, 0.0);
        while t < 12.0 {
            sim.run(SUBSTEP, &controls);
            t += SUBSTEP;
            let now = sim.telemetry.boom;
            let step = deg(now - was).abs();
            if step > worst {
                worst = step;
                at = t;
            }
            was = now;
        }
        // At two hundred substeps a second, half a degree a step is a hundred degrees a
        // second — the fastest part of a real crossing. Two degrees a step would be four
        // hundred, which no spar does; a hundred and thirty-six in one step is the fault
        // this is here for.
        assert!(
            worst < 2.0,
            "{tws_kn:.0} kn: the drawn boom moved {worst:.1} degrees in one substep at \
             {at:.2}s, which is a jump and not a swing"
        );
    }
}
