//! A sea made somewhere else.
//!
//! **The wave on the ocean need not be the wind at the boat**, and until this existed
//! the model said it was: every component came out of `(tws, fetch)`, so the sea could
//! only run where the wind ran and could only be as big as the wind here justified. That
//! is a shelf sea on a windy afternoon. It is not an ocean — a six-metre swell at sixteen
//! seconds arrives under a fifteen-knot breeze, from a storm that stopped two days ago a
//! thousand miles away, and it is the thing the boat is actually riding.
//!
//! What this file checks is that the two trains stay two trains: that they add the way
//! independent seas add, that the swell keeps its own bearing and period rather than
//! being pulled towards the wind's, and — the one that matters most — that a sea with no
//! swell in it is exactly the sea it always was.

use sail_core::units::{kn_to_ms, rad, wrap_pi};
use sail_core::water::{Swell, WaterSurface, WindSea};

/// Which way a component runs, radians. The model holds it as a unit vector built
/// `(sin, cos)` from the bearing, so this is that read backwards.
fn bearing(c: &sail_core::water::WaveComponent) -> f64 {
    c.direction.x.atan2(c.direction.y)
}

fn ocean() -> WindSea {
    // A light breeze from the north, which is the whole point: whatever we see of the
    // swell below cannot have come from this.
    let mut sea = WindSea::new(kn_to_ms(14.0), 60_000.0, 0.0, 7);
    sea.settle(kn_to_ms(14.0), 0.0);
    sea
}

/// **Nothing changes when there is no swell.** The bands were re-cut to make room for a
/// second train, and the re-cutting must be invisible to every sea that does not have
/// one — which is nearly all of them, and all of the ones every other test in this
/// workspace is about.
#[test]
fn a_sea_with_no_swell_is_the_sea_it_always_was() {
    let sea = ocean();
    assert!(sea.swell().is_none());
    let components = sea.components();
    assert_eq!(components.len(), 21, "all twenty-one are the wind sea's");

    // The wind sea is broad in direction — that is what makes it a wind sea rather than
    // a swell — and it is still broad.
    let spread = components
        .iter()
        .map(|c| wrap_pi(bearing(c) - sea.running_towards()).abs())
        .fold(0.0_f64, f64::max);
    assert!(
        spread > rad(30.0),
        "the wind sea has narrowed to {:.0} degrees",
        sail_core::units::deg(spread)
    );
    // And the height it reports is the height it has.
    assert!((sea.significant_height() - sea.wind_sea_height()).abs() < 1e-9);
}

/// Two seas add by energy, not by height: a seven-metre swell under a one-metre wind sea
/// is seven point one, and a boat told eight would be a boat told a lie.
#[test]
fn two_seas_add_the_way_two_seas_add() {
    let mut sea = ocean();
    let wind_sea = sea.wind_sea_height();
    assert!(
        wind_sea > 0.3 && wind_sea < 2.0,
        "a fourteen-knot sea is {wind_sea:.1} m"
    );

    sea.set_swell(
        Some(Swell {
            hs: 7.0,
            tp: 17.0,
            run: rad(70.0),
        }),
        0.0,
    );
    let both = (wind_sea * wind_sea + 49.0).sqrt();
    assert!(
        (sea.significant_height() - both).abs() < 1e-9,
        "{:.2} m reported against {both:.2} m of energy",
        sea.significant_height()
    );
    // The wind sea underneath is untouched: a swell arriving does not raise the chop.
    assert!((sea.wind_sea_height() - wind_sea).abs() < 1e-9);
    // And the period that gets reported is the one carrying the energy.
    assert!((sea.dominant_period() - 17.0).abs() < 1e-9);
}

/// **It keeps its own bearing**, which is the fact the whole thing exists for. The swell
/// runs where the storm that made it sent it; the wind here has nothing to say about it.
#[test]
fn the_swell_runs_its_own_way_and_at_its_own_period() {
    let mut sea = ocean();
    let wind_way = sea.running_towards();
    let swell_way = rad(70.0);
    assert!(
        wrap_pi(swell_way - wind_way).abs() > rad(45.0),
        "the test is pointless unless the two disagree"
    );
    sea.set_swell(
        Some(Swell {
            hs: 7.0,
            tp: 17.0,
            run: swell_way,
        }),
        0.0,
    );

    // The swell's components are the long ones, and they are the first six: the list is
    // built long to short and something downstream depends on that.
    let components = sea.components();
    assert_eq!(components.len(), 21);
    let (swell_part, wind_part) = components.split_at(6);

    for c in swell_part {
        assert!(
            wrap_pi(bearing(c) - swell_way).abs() < rad(8.0),
            "a swell component is {:.0} degrees off its train",
            sail_core::units::deg(wrap_pi(bearing(c) - swell_way).abs())
        );
        let period = std::f64::consts::TAU / c.omega;
        assert!(
            (14.0..21.0).contains(&period),
            "a seventeen-second swell has a {period:.1} s component in it"
        );
    }
    // And the wind sea is still around the wind.
    let wind_off = wind_part
        .iter()
        .map(|c| wrap_pi(bearing(c) - wind_way).abs())
        .fold(0.0_f64, f64::max);
    assert!(
        wind_off < rad(80.0),
        "the wind sea has wandered {:.0} degrees from the wind",
        sail_core::units::deg(wind_off)
    );

    // The longest thing in the sea is the swell, and the shortest is still the wind sea's
    // shortest — which is the ordering `carries_everything` reads when it asks the mesh
    // whether it can draw this.
    let longest = components
        .iter()
        .map(|c| c.wavelength())
        .fold(0.0_f64, f64::max);
    assert!(
        (swell_part[0].wavelength() - longest).abs() < 1.0,
        "the longest wave is not the swell's"
    );
    let shortest = components
        .iter()
        .map(|c| c.wavelength())
        .fold(f64::INFINITY, f64::min);
    assert!(
        (components.last().unwrap().wavelength() - shortest).abs() < 1e-9,
        "the list is no longer ordered long to short"
    );
}

/// And the water actually moves with it: a swell under a flat calm is still a swell.
#[test]
fn the_surface_feels_it() {
    let mut calm = WindSea::new(kn_to_ms(9.0), 20_000.0, 0.0, 3);
    calm.settle(kn_to_ms(9.0), 0.0);
    let was = (0..400)
        .map(|n| calm.height(glam::DVec2::new(n as f64 * 7.3, 0.0), 0.0))
        .fold(0.0_f64, |m, h| m.max(h.abs()));

    calm.set_swell(
        Some(Swell {
            hs: 6.0,
            tp: 16.0,
            run: rad(90.0),
        }),
        0.0,
    );
    let now = (0..400)
        .map(|n| calm.height(glam::DVec2::new(n as f64 * 7.3, 0.0), 0.0))
        .fold(0.0_f64, |m, h| m.max(h.abs()));
    assert!(
        now > was * 2.5,
        "the surface barely moved: {was:.2} m before, {now:.2} m with six metres of \
         swell running through it"
    );
}
