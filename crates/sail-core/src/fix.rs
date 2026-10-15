//! Working out where you are from what you can see.
//!
//! The oldest instrument in this game and the only one that is pure arithmetic: a
//! bearing to something you can name is a *position line* — you are somewhere on the
//! line that runs from that mark along the reciprocal of the bearing you took — and two
//! position lines cross at a point. Three cross in a small triangle, and the triangle is
//! the thing: it is not an error bar, it is the navigator's own report on how carefully
//! he just did it, and it has been called a cocked hat since before anybody wrote any of
//! this down.
//!
//! Everything a pilot book says about taking a fix falls out of the geometry and is
//! written here as a consequence rather than as a rule:
//!
//! * **Two bearings crossing at a fine angle are a bad fix.** The crossing point slides
//!   along both lines for a degree of error, so a cut of twenty degrees turns a two
//!   degree mistake into a quarter of a mile. Thirty degrees is the usual floor and
//!   ninety is the ideal, and `cut` is the number that says which you have.
//! * **Three is better than two**, not because the third line is more accurate but
//!   because two lines always cross — they cannot tell you that one of them is wrong,
//!   and three can.
//! * **A bearing on something close is worth more than one on something far**, because
//!   the same angular error is fewer metres of position line at the near mark.
//!
//! This knows nothing about charts, keys or screens. It takes bearings and gives back a
//! position and a report, which is all a navigator does.

use crate::units::{wrap_pi, wrap_tau};
use glam::DVec2;

/// A bearing taken on something, and the something it was taken on.
#[derive(Clone, Copy, Debug)]
pub struct Sight {
    /// Where the mark is, which the chart knows.
    pub mark: DVec2,
    /// The bearing *to* it, true, radians clockwise from north — which is what the
    /// compass reads and what the navigator writes down. The position line is plotted
    /// from the mark along the reciprocal of it, and doing that conversion in one place
    /// is the whole reason this type exists: a reciprocal applied twice is a fix on the
    /// wrong side of the island, and it looks perfectly reasonable.
    pub bearing: f64,
    /// How far she has run since this bearing was taken, through the water.
    ///
    /// **This is what turns two bearings into one fix.** A position line is a statement
    /// about where she was at the moment the bearing was taken, and she does not stay
    /// there: take a bearing, sail for twenty minutes, take another, and the two lines
    /// are about two different places. The navigator's answer is four hundred years old
    /// and is the running fix — *transfer* the first line along the course and distance
    /// run and cross it with the second, and the crossing is where she is now. The line
    /// moves; the bearing does not.
    ///
    /// It is the run **through the water**, because that is what the compass and the log
    /// give you, and it is why a running fix is weaker than two marks shot together: a
    /// knot of tide that nobody has measured goes straight into the transfer and comes
    /// out as position. That is not a defect in the method, it is the method being
    /// honest about what it is made of.
    pub ran: DVec2,
}

impl Sight {
    /// The position line through the mark: a point on it and a unit vector along it,
    /// running *away* from the mark towards the observer.
    pub fn line(&self) -> (DVec2, DVec2) {
        let away = wrap_tau(self.bearing + std::f64::consts::PI);
        (self.through(), DVec2::new(away.sin(), away.cos()))
    }

    /// The point the line passes through: the mark, carried along by the run since.
    ///
    /// Through the mark itself while the bearing is fresh, which is where a position
    /// line starts its life — and a hand's breadth off it twenty minutes later, which is
    /// where the fix comes from.
    pub fn through(&self) -> DVec2 {
        self.mark + self.ran
    }
}

/// What a set of sights says about where you are.
#[derive(Clone, Copy, Debug)]
pub struct Fix {
    /// The position itself: where two lines cross, or the middle of the triangle three
    /// of them make.
    pub at: DVec2,
    /// How big the cocked hat is, metres — the distance from the fix to the furthest of
    /// the crossing points. Nought for two sights, which is why two sights can never
    /// tell you that you were careless.
    pub hat: f64,
    /// The angle the fix actually stands on, radians: the **best** crossing among the
    /// pairs rather than the worst.
    ///
    /// Which is what a navigator does with three lines. Two of them may lie nearly along
    /// each other — three marks on a coast are often roughly in a row — and that pair
    /// tells you little; but if the third crosses them squarely, the fix is a good one
    /// and the near-parallel pair is simply not what it rests on. Reporting the worst
    /// pair called a perfectly sound three-line fix *a fine cut*, which is the arithmetic
    /// being stricter than the practice.
    pub cut: f64,
    /// And the worst of them, which is still worth knowing: it is why the cocked hat is
    /// the shape it is.
    pub worst_cut: f64,
}

/// Where the two position lines cross, if they do.
fn crossing(a: &Sight, b: &Sight) -> Option<DVec2> {
    let (pa, da) = a.line();
    let (pb, db) = b.line();
    let denominator = da.x * db.y - da.y * db.x;
    // Parallel, or near enough that the crossing is a rumour.
    if denominator.abs() < 1.0e-9 {
        return None;
    }
    let offset = pb - pa;
    let t = (offset.x * db.y - offset.y * db.x) / denominator;
    Some(pa + da * t)
}

/// The angle at which two position lines cut each other, nought to a right angle.
fn cut_of(a: &Sight, b: &Sight) -> f64 {
    let (_, da) = a.line();
    let (_, db) = b.line();
    let angle = wrap_pi(da.y.atan2(da.x) - db.y.atan2(db.x)).abs();
    // A cut of a hundred and sixty degrees is a cut of twenty: which side the lines lie
    // on does not change how badly they cross.
    angle.min(std::f64::consts::PI - angle)
}

/// Plot the sights and see where they put you.
///
/// Returns nothing for fewer than two, or for a set that does not cross at all — two
/// bearings on the same mark, or on two marks in line, which is a navigator taking the
/// same position line twice and is a mistake worth not hiding.
pub fn plot(sights: &[Sight]) -> Option<Fix> {
    if sights.len() < 2 {
        return None;
    }
    let mut points: Vec<DVec2> = Vec::new();
    let mut cut: f64 = 0.0;
    let mut worst: f64 = std::f64::consts::FRAC_PI_2;
    for (i, a) in sights.iter().enumerate() {
        for b in &sights[i + 1..] {
            let here = cut_of(a, b);
            // A pair that is useless is left out of the position rather than allowed to
            // throw it: two lines crossing at three degrees put the crossing a mile away
            // and would drag the mean of the triangle with it.
            if here < crate::units::rad(6.0) {
                continue;
            }
            if let Some(p) = crossing(a, b) {
                points.push(p);
                cut = cut.max(here);
                worst = worst.min(here);
            }
        }
    }
    if points.is_empty() {
        return None;
    }
    let at = points.iter().fold(DVec2::ZERO, |sum, p| sum + *p) / points.len() as f64;
    let hat = points
        .iter()
        .map(|p| (*p - at).length())
        .fold(0.0_f64, f64::max);
    Some(Fix {
        at,
        hat,
        cut,
        worst_cut: worst,
    })
}

/// What a navigator would say about a fix of this shape.
///
/// Not a score out of ten: the two things that make a fix trustworthy are how finely the
/// lines cut and how far apart they landed, and both have numbers a pilot book would
/// recognise. A cut under thirty degrees is poor whatever the triangle does; a triangle
/// bigger than a cable is a fix nobody would act on near rocks.
pub fn verdict(fix: &Fix, sights: usize) -> &'static str {
    const CABLE: f64 = 185.0;
    if fix.cut < crate::units::rad(30.0) {
        "a fine cut — the lines barely cross"
    } else if sights < 3 {
        "two lines: they cross, and nothing checks them"
    } else if fix.hat < CABLE * 0.35 {
        "a tight hat"
    } else if fix.hat < CABLE {
        "a workable hat"
    } else {
        "a hat you would not stand on"
    }
}

/// Where you must be standing if those three things are *that* far apart from each
/// other — the three-point problem, which a navigator solves with a station pointer and
/// which is the oldest fix there is.
///
/// Nothing about a compass is in it. You measure the horizontal angle between the left
/// mark and the middle one, and between the middle one and the right, and the two angles
/// alone put you somewhere: a chord subtends the same angle from every point of its arc,
/// so each pair of marks puts you on a circle through both of them, and the circles
/// cross at the middle mark and at one other place. That other place is where you are.
///
/// Which is why it is the fix to have when your compass is suspect and why a station
/// pointer — three arms and a protractor — was on every bridge for two hundred years.
///
/// It fails, and says so, in the case every pilot book warns about: when all three marks
/// and the observer lie on one circle there is no second crossing and the angles are the
/// same from everywhere on it. That is the **swinging circle**, and the answer is to pick
/// a different middle mark.
pub fn three_point(a: DVec2, b: DVec2, c: DVec2, left: f64, right: f64) -> Option<DVec2> {
    let arc = |p: DVec2, q: DVec2, angle: f64| -> Option<(DVec2, f64)> {
        let sin = angle.sin();
        if sin.abs() < 1.0e-4 {
            return None;
        }
        let chord = q - p;
        let r = chord.length() / (2.0 * sin);
        let middle = (p + q) * 0.5;
        // The centre sits on the perpendicular bisector, a cotangent of the angle away
        // from the middle of the chord — and the cotangent carries the side with it,
        // which is the whole of why this is written this way. It goes negative past a
        // right angle, which puts the centre across the chord from the observer, and it
        // goes negative again for an angle measured the other way round. Built from
        // `sqrt(r² − chord²/4)` instead, as it was, the sign is lost and the circle comes
        // out on the wrong side of the coast half the time — with a perfectly confident
        // answer on it.
        //
        // Which side, settled on paper rather than in the code: the left-hand normal of
        // the chord with the cotangent *subtracted* puts the circle through the
        // observer, and added puts it through a point four kilometres the other side of
        // the marks — a confident answer reflected across the coast, which is exactly
        // the failure this function exists to avoid. Checked numerically for three
        // standing places before it was written down.
        let out = DVec2::new(-chord.y, chord.x).normalize_or_zero();
        Some((middle - out * (chord.length() * 0.5 / angle.tan()), r.abs()))
    };
    let (first, r1) = arc(a, b, left)?;
    let (second, r2) = arc(b, c, right)?;
    // The two circles cross at b and at the observer. Reflect b in the line of centres
    // and the other crossing falls out — no quadratic, no branch to get wrong.
    let axis = second - first;
    let span = axis.length();
    if span < 1.0e-6 {
        // Concentric: the swinging circle. There is no answer and pretending otherwise
        // is how a station pointer puts a ship on a rock.
        return None;
    }
    let along = axis / span;
    let to_b = b - first;
    let foot = first + along * to_b.dot(along);
    let other = foot * 2.0 - b;
    let _ = (r1, r2);
    Some(other)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::{deg, rad};

    /// The bearing a compass would read from here to there, true, clockwise from north.
    fn bearing_to(from: DVec2, to: DVec2) -> f64 {
        let off = to - from;
        wrap_tau(off.x.atan2(off.y))
    }

    /// Three bearings taken from a known spot put you back on that spot.
    ///
    /// Which sounds circular and is the opposite: it is the one test that catches a
    /// reciprocal applied twice, or not at all, and that fault draws a perfectly
    /// convincing fix on the wrong side of the island.
    #[test]
    fn three_bearings_put_you_where_you_took_them() {
        let me = DVec2::new(1_200.0, -3_400.0);
        let marks = [
            DVec2::new(4_000.0, 1_000.0),
            DVec2::new(-2_500.0, -800.0),
            DVec2::new(900.0, -9_000.0),
        ];
        let sights: Vec<Sight> = marks
            .iter()
            .map(|m| Sight {
                mark: *m,
                bearing: bearing_to(me, *m),
                ran: DVec2::ZERO,
            })
            .collect();
        let fix = plot(&sights).expect("three good bearings");
        assert!(
            (fix.at - me).length() < 1.0,
            "a perfect set of sights put the fix {:.1} m from where they were taken",
            (fix.at - me).length()
        );
        assert!(fix.hat < 1.0, "and opened a {:.1} m cocked hat", fix.hat);
    }

    /// Three lines, two of them nearly in a row, is still a good fix — because it stands
    /// on the pair that crosses squarely and the near-parallel pair is not what it rests
    /// on.
    ///
    /// Reported from the deck with a screenshot: three honest bearings, a cocked hat of
    /// two hundred and sixty metres, and the chart calling it *a fine cut* because two of
    /// the three marks happened to lie in a line. That was the arithmetic being stricter
    /// than the practice.
    #[test]
    fn three_lines_stand_on_the_pair_that_crosses() {
        let me = DVec2::ZERO;
        let marks = [
            // Two nearly in a row from here...
            DVec2::new(0.0, 6_000.0),
            DVec2::new(900.0, 6_000.0),
            // ...and one square on the beam, which is what the fix is built on.
            DVec2::new(6_000.0, 0.0),
        ];
        let sights: Vec<Sight> = marks
            .iter()
            .map(|m| Sight {
                mark: *m,
                bearing: bearing_to(me, *m),
                ran: DVec2::ZERO,
            })
            .collect();
        let fix = plot(&sights).expect("three bearings");
        assert!(
            deg(fix.cut) > 70.0,
            "the fix stands on a {:.0} degree cut and was called {:.0}",
            deg(fix.cut),
            deg(fix.worst_cut)
        );
        assert!(
            deg(fix.worst_cut) < 15.0,
            "and the two in a row should still be reported as the poor pair"
        );
        assert_ne!(verdict(&fix, 3), "a fine cut — the lines barely cross");
    }

    /// A degree of error on each bearing opens the hat rather than moving the fix, and
    /// that is the whole value of the third line.
    #[test]
    fn a_careless_degree_shows_up_as_a_triangle() {
        let me = DVec2::new(0.0, 0.0);
        let marks = [
            DVec2::new(3_000.0, 0.0),
            DVec2::new(-1_500.0, 2_600.0),
            DVec2::new(-1_500.0, -2_600.0),
        ];
        let sights: Vec<Sight> = marks
            .iter()
            .enumerate()
            .map(|(i, m)| Sight {
                mark: *m,
                // A degree and a half each way, which is a careful hand on a hand bearing
                // compass in a seaway.
                bearing: bearing_to(me, *m) + rad(1.5) * if i % 2 == 0 { 1.0 } else { -1.0 },
                ran: DVec2::ZERO,
            })
            .collect();
        let fix = plot(&sights).expect("three bearings");
        assert!(
            fix.hat > 40.0,
            "a degree and a half of error should open a visible hat, and it opened \
             {:.0} m",
            fix.hat
        );
        // And it should still be roughly right: the error is in the shape of the
        // triangle rather than in where it sits.
        assert!(
            (fix.at - me).length() < 160.0,
            "and it should still be near: {:.0} m out",
            (fix.at - me).length()
        );
    }

    /// Two marks nearly in line are one position line taken twice, and the arithmetic
    /// has to say so rather than produce a confident answer from it.
    #[test]
    fn a_fine_cut_is_reported_as_one() {
        let me = DVec2::new(0.0, 0.0);
        let close = [DVec2::new(0.0, 5_000.0), DVec2::new(600.0, 5_000.0)];
        let sights: Vec<Sight> = close
            .iter()
            .map(|m| Sight {
                mark: *m,
                bearing: bearing_to(me, *m),
                ran: DVec2::ZERO,
            })
            .collect();
        let fix = plot(&sights).expect("they do cross");
        assert!(
            deg(fix.cut) < 10.0,
            "two marks nearly in line cut at {:.0} degrees",
            deg(fix.cut)
        );
        assert_eq!(verdict(&fix, 2), "a fine cut — the lines barely cross");
        // And a proper pair says so too.
        let square = [DVec2::new(0.0, 5_000.0), DVec2::new(5_000.0, 0.0)];
        let good: Vec<Sight> = square
            .iter()
            .map(|m| Sight {
                mark: *m,
                bearing: bearing_to(me, *m),
                ran: DVec2::ZERO,
            })
            .collect();
        let fix = plot(&good).expect("a right-angled cut");
        assert!((deg(fix.cut) - 90.0).abs() < 1.0);
        assert_eq!(
            verdict(&fix, 2),
            "two lines: they cross, and nothing checks them"
        );
    }

    /// Three horizontal angles put you where you measured them, with no compass in it.
    ///
    /// The station pointer's own test: stand somewhere, measure the angles between three
    /// marks, and the arithmetic has to give the standing place back. It is the one
    /// check that catches a circle built on the wrong side of its chord, which produces
    /// a confident answer reflected across the coast.
    #[test]
    fn three_angles_put_you_where_you_measured_them() {
        let marks = [
            DVec2::new(-3_000.0, 4_000.0),
            DVec2::new(500.0, 5_200.0),
            DVec2::new(4_100.0, 3_300.0),
        ];
        for me in [
            DVec2::new(0.0, 0.0),
            DVec2::new(-1_500.0, -900.0),
            DVec2::new(2_200.0, 400.0),
        ] {
            let bearing = |m: DVec2| wrap_tau((m - me).x.atan2((m - me).y));
            let left = wrap_pi(bearing(marks[1]) - bearing(marks[0]));
            let right = wrap_pi(bearing(marks[2]) - bearing(marks[1]));
            let got = three_point(marks[0], marks[1], marks[2], left, right)
                .expect("three marks and two angles");
            assert!(
                (got - me).length() < 1.0,
                "standing at {me:?} the angles solved to {got:?}"
            );
        }
    }

    /// And on the swinging circle it says it cannot, rather than inventing somewhere.
    #[test]
    fn the_swinging_circle_has_no_answer() {
        // Four points on one circle: three marks and an observer. Every position on that
        // circle shows the same two angles, so the problem has no single answer — which
        // is the case every pilot book warns about and the one a confident wrong answer
        // would hide.
        let r = 4_000.0;
        let on = |deg: f64| {
            let a = deg.to_radians();
            DVec2::new(r * a.sin(), r * a.cos())
        };
        let (a, b, c, me) = (on(40.0), on(120.0), on(200.0), on(300.0));
        let bearing = |m: DVec2| wrap_tau((m - me).x.atan2((m - me).y));
        let left = wrap_pi(bearing(b) - bearing(a));
        let right = wrap_pi(bearing(c) - bearing(b));
        let got = three_point(a, b, c, left, right);
        // Either it refuses, or it lands somewhere on that circle — what it must not do
        // is claim a position off it.
        if let Some(p) = got {
            assert!(
                (p.length() - r).abs() < 60.0,
                "a swinging circle produced {p:?}, which is not on it"
            );
        }
    }

    /// One sight is not a fix, and the same sight twice is not two.
    #[test]
    fn one_line_is_not_a_position() {
        let only = Sight {
            mark: DVec2::new(1_000.0, 0.0),
            bearing: rad(90.0),
                ran: DVec2::ZERO,
        };
        assert!(plot(&[only]).is_none());
        assert!(
            plot(&[only, only]).is_none(),
            "the same line twice crossed itself"
        );
    }
}

#[cfg(test)]
mod the_running_fix {
    use super::*;
    use crate::units::{rad, wrap_tau};

    fn bearing_to(from: DVec2, mark: DVec2) -> f64 {
        let off = mark - from;
        wrap_tau(off.x.atan2(off.y))
    }

    /// **One mark, two bearings and the run between them is a position.**
    ///
    /// The oldest trick in coastal navigation and the one a single lighthouse on a long
    /// coast is good for: take her, sail on, take her again, transfer the first line by
    /// what the log and the compass say you have run, and cross the two. Without the
    /// transfer both lines go through the same mark and cross at the mark — which is a
    /// fix on the rocks, and is what the chart drew before this existed.
    #[test]
    fn two_bearings_on_one_mark_cross_where_she_is() {
        let light = DVec2::new(0.0, 4_000.0);
        let first = DVec2::new(-3_000.0, 0.0);
        // Half an hour at six knots, due east: five and a half thousand metres.
        let run = DVec2::new(5_556.0, 0.0);
        let now = first + run;
        let sights = [
            Sight {
                mark: light,
                bearing: bearing_to(first, light),
                ran: run,
            },
            Sight {
                mark: light,
                bearing: bearing_to(now, light),
                ran: DVec2::ZERO,
            },
        ];
        let fix = plot(&sights).expect("a transferred line and a fresh one cross");
        assert!(
            (fix.at - now).length() < 1.0,
            "the running fix is {:.0} m from where she is",
            (fix.at - now).length()
        );
        assert!(
            fix.cut > rad(30.0),
            "half an hour of running opens the cut to {:.0}°",
            fix.cut.to_degrees()
        );
    }

    /// Without the run, the same two bearings say she is standing on the lighthouse.
    ///
    /// The test that says why the field exists. Two position lines through one mark
    /// cross at the mark however carefully the bearings were taken, so a chart that does
    /// not transfer is not merely less accurate — it is confidently wrong, and wrong in
    /// the direction of the rocks.
    #[test]
    fn without_the_transfer_she_is_on_the_rock() {
        let light = DVec2::new(0.0, 4_000.0);
        let first = DVec2::new(-3_000.0, 0.0);
        let now = first + DVec2::new(5_556.0, 0.0);
        let sights = [
            Sight {
                mark: light,
                bearing: bearing_to(first, light),
                ran: DVec2::ZERO,
            },
            Sight {
                mark: light,
                bearing: bearing_to(now, light),
                ran: DVec2::ZERO,
            },
        ];
        if let Some(fix) = plot(&sights) {
            assert!(
                (fix.at - light).length() < 1.0,
                "untransferred, the crossing is the mark itself"
            );
        }
    }

    /// A tide nobody measured goes into the transfer and comes out as position.
    ///
    /// Which is the honest limit of the method, and the reason a running fix is the one
    /// you take when you cannot have a better one. Half a knot of set across the track
    /// for half an hour is five hundred metres, and the fix is out by about that — not
    /// because the arithmetic is poor but because the arithmetic was given the run
    /// through the water and she went through the ground.
    #[test]
    fn an_unmeasured_set_comes_out_as_error() {
        let light = DVec2::new(0.0, 4_000.0);
        let first = DVec2::new(-3_000.0, 0.0);
        let through_water = DVec2::new(5_556.0, 0.0);
        // Half a knot northward for half an hour, which nobody aboard knows about.
        let set = DVec2::new(0.0, 463.0);
        let really = first + through_water + set;
        let sights = [
            Sight {
                mark: light,
                bearing: bearing_to(first, light),
                ran: through_water,
            },
            Sight {
                mark: light,
                bearing: bearing_to(really, light),
                ran: DVec2::ZERO,
            },
        ];
        let fix = plot(&sights).expect("they still cross");
        let out = (fix.at - really).length();
        assert!(
            (200.0..900.0).contains(&out),
            "the set should come out as a few hundred metres of error, got {out:.0} m"
        );
    }
}

