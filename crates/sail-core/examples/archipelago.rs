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
    // The same square the game uses, so what is printed here is what somebody sails in.
    let count: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(34);
    let land = Archipelago::generate_around(0x15_1A_4D ^ 31, 5_600.0, count, 900.0);
    chart(&land, 5_600.0);
    grain(&land);
    println!(
        "{} islands over an eleven-kilometre square\n",
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

/// The archipelago as a page of text.
///
/// **Because the shape of a scatter is not a number.** Counts, radii and nearest
/// neighbours all say the generator did something; whether what it did looks like a coast
/// is a question for an eye, and an eye needs a picture. This is the cheapest picture
/// that can be read in a terminal, in a test log, or in a diff — which is also the point:
/// a change to the placement shows up here as a different shape rather than as a
/// different number nobody can interpret.
///
/// `#` is land, `:` its shoal water, `.` open sea, `+` where the boat starts.
fn chart(land: &Archipelago, half_width: f64) {
    const COLUMNS: usize = 96;
    const ROWS: usize = 40;
    println!();
    for row in 0..ROWS {
        let mut line = String::with_capacity(COLUMNS);
        // North at the top: the first row is the top of the square.
        let y = half_width - (row as f64 + 0.5) / ROWS as f64 * 2.0 * half_width;
        for col in 0..COLUMNS {
            let x = -half_width + (col as f64 + 0.5) / COLUMNS as f64 * 2.0 * half_width;
            let at = glam::DVec2::new(x, y);
            let depth = land.depth(at);
            line.push(if at.length() < half_width / 28.0 {
                '+'
            } else if depth <= 0.0 {
                '#'
            } else if depth < 12.0 {
                ':'
            } else {
                '.'
            });
        }
        println!("{line}");
    }
    println!();
}

/// How much of a chain it is, as two numbers.
///
/// The spread of the islands along the grain against their spread across it. A scatter
/// comes out near one; a coast comes out at two or three, and the eye reads that as *a
/// line of islands* before it reads anything else. Measured by the principal axis of the
/// centres rather than against the strike the generator used, so it answers the question
/// a reader of the chart would ask rather than the one the code already knows.
fn grain(land: &Archipelago) {
    // In the generator's own frame first: how far they lie along the grain and how far
    // off it. This is the number the placement is actually trying to move.
    let strike = land.strike();
    let along = glam::DVec2::new(strike.sin(), strike.cos());
    let across = glam::DVec2::new(along.y, -along.x);
    let (mut sa, mut sc) = (0.0_f64, 0.0_f64);
    for i in land.islands() {
        sa += i.centre.dot(along).powi(2);
        sc += i.centre.dot(across).powi(2);
    }
    let n = land.islands().len().max(1) as f64;
    // One line an island, if asked: `--example archipelago -- 34 all`. The chart above
    // says what the shape is; this says where every rock in it went, which is what you
    // want open beside the chart when a placement rule is being changed.
    if std::env::args().any(|a| a == "all") {
        let mut rows: Vec<(f64, f64, f64)> = land
            .islands()
            .iter()
            .map(|i| {
                (
                    i.centre.dot(along) / 1000.0,
                    i.centre.dot(across) / 1000.0,
                    i.radius,
                )
            })
            .collect();
        rows.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        for (a, c, r) in &rows {
            println!("    along {a:+6.2} km   off {c:+6.2} km   r {r:4.0} m");
        }
    }
    println!(
        "grain {:03.0}°: {:.1} km along it, {:.1} km off it — {:.1} to one",
        deg(strike),
        (sa / n).sqrt() / 1000.0,
        (sc / n).sqrt() / 1000.0,
        (sa / sc).sqrt()
    );
    let centres: Vec<glam::DVec2> = land.islands().iter().map(|i| i.centre).collect();
    let n = centres.len() as f64;
    let mean = centres.iter().fold(glam::DVec2::ZERO, |a, b| a + *b) / n;
    let (mut xx, mut xy, mut yy) = (0.0, 0.0, 0.0);
    for c in &centres {
        let d = *c - mean;
        xx += d.x * d.x;
        xy += d.x * d.y;
        yy += d.y * d.y;
    }
    xx /= n;
    xy /= n;
    yy /= n;
    // The two eigenvalues of a symmetric 2x2, which are the variances along and across.
    let mid = 0.5 * (xx + yy);
    let gap = (0.25 * (xx - yy) * (xx - yy) + xy * xy).sqrt();
    let (big, small) = (mid + gap, (mid - gap).max(1.0));
    let axis = 0.5 * (2.0 * xy).atan2(xx - yy);
    println!(
        "{} islands, grain {:03.0}°, spread {:.1} km along and {:.1} km across — {:.1} to one",
        centres.len(),
        deg(wrap(std::f64::consts::FRAC_PI_2 - axis)),
        big.sqrt() / 1000.0,
        small.sqrt() / 1000.0,
        (big / small).sqrt(),
    );
}

fn wrap(a: f64) -> f64 {
    a.rem_euclid(std::f64::consts::PI)
}
