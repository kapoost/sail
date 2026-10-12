//! What does the land actually do?
//!
//! An archipelago that does not change the numbers is scenery, and scenery is not what
//! this is for. Three things have to be true before it earns its place: you can run
//! out of water, the wind dies in a lee, and the sea goes down there too. This measures
//! all three, because each of them is easy to write and easy to get silently wrong.
//!
//!   cargo run -p sail-core --release --example archipelago

use sail_core::land::{Archipelago, OPEN_WATER};
use sail_core::prelude::*;
use sail_core::water::fetch_limited_sea;

fn main() {
    let land = Archipelago::generate(0xA12C, 9_000.0, 26);
    println!(
        "{} islands over an eighteen-kilometre square\n",
        land.islands().len()
    );
    println!("{:>8}{:>9}{:>9}{:>8}", "radius", "height", "x km", "y km");
    let mut sorted: Vec<_> = land.islands().to_vec();
    sorted.sort_by(|a, b| b.radius.partial_cmp(&a.radius).unwrap());
    for i in sorted.iter().take(6) {
        println!(
            "{:>8.0}{:>9.0}{:>9.2}{:>8.2}",
            i.radius,
            i.height,
            i.centre.x / 1000.0,
            i.centre.y / 1000.0
        );
    }

    // Take the largest island and walk away from it, straight downwind.
    let big = sorted[0];
    let wind_from = rad(0.0);
    println!(
        "\nsailing downwind away from the lee of the {:.0} m island ({:.0} m high)",
        big.radius, big.height
    );
    println!(
        "{:>9}{:>9}{:>9}{:>10}{:>9}{:>9}",
        "m to lee", "depth", "shelter", "wind kn", "fetch km", "Hs m"
    );
    for step in 0..12 {
        let out = big.radius + 40.0 * 1.9_f64.powi(step) - 40.0;
        // Downwind of the island means to the south of it, for a northerly.
        let pos = big.centre - glam::DVec2::new(0.0, out);
        let shelter = land.shelter(pos, wind_from);
        let fetch = land.fetch(pos, wind_from, 120_000.0);
        let open = kn_to_ms(18.0);
        let (hs, _) = fetch_limited_sea(open * (0.2 + 0.8 * shelter), fetch);
        println!(
            "{:>9.0}{:>9.1}{:>9.2}{:>10.1}{:>9.1}{:>9.2}",
            out - big.radius,
            land.depth(pos),
            shelter,
            ms_to_kn(open * (0.2 + 0.8 * shelter)),
            fetch / 1000.0,
            hs
        );
    }

    // And the bottom, coming in from seaward.
    println!("\ncoming ashore: depth against distance off");
    println!(
        "{:>10}{:>9}{:>26}",
        "m off", "depth", "aground at 1.95 m draft?"
    );
    for off in [-20.0, 0.0, 10.0, 25.0, 40.0, 60.0, 80.0, 120.0, 200.0] {
        let pos = big.centre + glam::DVec2::new(0.0, big.shore_radius(0.0) + off);
        println!(
            "{:>10.0}{:>9.1}{:>26}",
            off,
            land.depth(pos),
            if land.aground(pos, 1.95) { "yes" } else { "no" }
        );
    }
    println!(
        "\nopen water is {OPEN_WATER:.0} m, and the shelf reaches four hundred metres offshore."
    );

    let harbours = land.harbours();
    println!("\n{} anchorages worth delivering to", harbours.len());
    println!(
        "{:>12}{:>9}{:>9}{:>9}{:>12}",
        "name", "x km", "y km", "depth", "open to"
    );
    for h in &harbours {
        println!(
            "{:>12}{:>9.2}{:>9.2}{:>9.1}{:>11.0}°",
            h.name,
            h.at.x / 1000.0,
            h.at.y / 1000.0,
            land.depth(h.at),
            deg(h.exposed_to)
        );
    }
    if harbours.len() > 1 {
        let (a, b) = (&harbours[0], &harbours[1]);
        println!(
            "\na run from {} to {} is {:.1} nautical miles",
            a.name,
            b.name,
            (b.at - a.at).length() / 1852.0
        );
    }
}
