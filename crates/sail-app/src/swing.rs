//! Things hung over the side on a lanyard, and what they fetch up against.
//!
//! A boat carries gear tied to her rails — a creel of fish, a lifebuoy, a fender, a
//! bucket on a line — and all of it does the same thing, which is the reason this is one
//! file rather than the same forty lines written out beside each object.
//!
//! **It is a pendulum and not a filter.** Lagging the hull's roll is the obvious thing to
//! reach for and it is wrong in a way you can see: a filtered angle has no period of its
//! own, so it cannot keep swinging after the boat has stopped, and it cannot swing
//! *against* her either — which is most of what hanging gear actually does and the whole
//! reason it is worth drawing. A real one has `T = 2π√(l/g)`, which on half a metre of
//! lanyard is about a second and a half, and it hangs plumb while the boat rolls
//! underneath it.
//!
//! **It fetches up against two things, and neither of them is a clamp.** Inboard of
//! anything hung on a quarter is the boat, and below it is the sea. Stopping the angle at
//! a limit is what a clamp does and it reads as a thing hitting a wall in a vacuum: the
//! swing simply ends. What a fender does against topsides is *bounce*, losing most but
//! not all of its speed, and what a buoy does when it reaches the water is slow down and
//! lose its weight — the two are different and the difference is visible from across a
//! harbour.
//!
//! The water is the more interesting of the two. A thing floating does not swing at all:
//! buoyancy cancels the gravity that was restoring it, so the restoring stiffness goes
//! away in proportion to how deep it is and the damping goes up. A lifebuoy that dips a
//! wave and *carries on swinging* is the tell that nobody modelled the water; one that
//! dips, checks, and comes out lagging is the thing itself.

/// Where a hanging thing is in its swing.
///
/// The angle is measured in the **boat's** frame, because that is the frame the thing is
/// drawn in: a child of the hull with a rotation on it. Plumb is therefore not zero, it
/// is minus the heel, and that is what makes the whole thing work without any
/// world-space bookkeeping.
#[derive(Default, Debug, Clone, Copy)]
pub struct Swing {
    pub angle: f32,
    pub rate: f32,
}

/// What is hung, and what it is hung on.
#[derive(Debug, Clone, Copy)]
pub struct Hung {
    /// Lanyard, metres. It sets the period and nothing else.
    pub length: f32,
    /// How far the thing's own body reaches below the end of the lanyard, metres — which
    /// is what actually meets the water.
    pub reach: f32,
    /// How far it can swing before it is against her topsides, radians. Positive is
    /// inboard for something hung to port and the sign is the caller's business.
    pub inboard: f32,
    pub outboard: f32,
    /// How much of its speed it keeps off the planking. Nought is a bag of sand and one
    /// is a squash ball; a fender or a wicker basket is down at the bottom of that range.
    pub bounce: f32,
    /// How much of its weight the water takes when it is fully in, as a fraction. A
    /// lifebuoy is one and a bit — it is *trying* to float. A creel with the sea through
    /// it is nearly nothing.
    pub buoyant: f32,
    /// How quickly it dies away in air, and again in the water, per second.
    pub damp: f32,
    pub damp_wet: f32,
}

/// Gravity, for the restoring term.
const G: f32 = 9.81;

/// How hard the hull's roll throws it.
///
/// A boat rolling under a hanging weight accelerates the hanging point sideways, and that
/// is the forcing term — it is why gear swings hardest in the moment she comes back up
/// rather than at the end of the roll, which is the part a filter can never produce.
const THROWN: f32 = 2.4;

impl Swing {
    /// One step. `plumb` is the angle that is vertical in the boat's frame — minus the
    /// heel — and `clearance` is how far the hanging point is above the water.
    pub fn step(&mut self, hung: &Hung, plumb: f32, roll_rate: f32, clearance: f32, dt: f32) {
        // How much of it is in the water, nought to one. The bottom of the thing hangs
        // `(length + reach)·cos θ` below the pivot, measured from vertical, so a swing
        // outwards lifts it clear — which is why a buoy skips along the surface rather
        // than ploughing it.
        let hangs = (hung.length + hung.reach) * (self.angle - plumb).cos();
        let immersion = ((hangs - clearance) / hung.reach.max(0.05)).clamp(0.0, 1.0);

        // Buoyancy does not push it about; it takes its weight away. A thing floating has
        // nothing restoring it to plumb, so the stiffness goes with the immersion rather
        // than a force being added — which is the same statement and a far better
        // behaved one, since a force fighting gravity at the bottom of a swing is how an
        // integrator is made to ring.
        let afloat = (1.0 - hung.buoyant * immersion).max(-0.2);
        let stiffness = G / hung.length * afloat;
        let damp = hung.damp + (hung.damp_wet - hung.damp) * immersion;

        self.rate += (stiffness * (plumb - self.angle) - damp * self.rate) * dt;
        self.rate -= roll_rate * THROWN * dt;
        self.angle += self.rate * dt;

        // And the topsides. A bounce rather than a stop: it arrives, most of the speed
        // goes into the noise it makes, and what is left sends it back out.
        if self.angle > hung.inboard {
            self.angle = hung.inboard;
            if self.rate > 0.0 {
                self.rate = -self.rate * hung.bounce;
            }
        }
        if self.angle < -hung.outboard {
            self.angle = -hung.outboard;
            if self.rate < 0.0 {
                self.rate = -self.rate * hung.bounce;
            }
        }
    }

    /// How wet it is, for anything that wants to draw a splash or darken a colour.
    pub fn immersion(&self, hung: &Hung, plumb: f32, clearance: f32) -> f32 {
        let hangs = (hung.length + hung.reach) * (self.angle - plumb).cos();
        ((hangs - clearance) / hung.reach.max(0.05)).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn creel() -> Hung {
        Hung {
            length: 0.62,
            reach: 0.26,
            inboard: 0.11,
            outboard: 0.85,
            bounce: 0.25,
            buoyant: 0.2,
            damp: 1.9,
            damp_wet: 4.5,
        }
    }

    /// It has a period of its own, which is the whole reason it is not a filter: pushed
    /// once and left alone, it comes back and goes past.
    #[test]
    fn it_swings_rather_than_settling() {
        let mut s = Swing {
            angle: -0.4,
            rate: 0.0,
        };
        let hung = Hung {
            damp: 0.05,
            damp_wet: 0.05,
            ..creel()
        };
        let mut crossed = 0;
        let mut side = s.angle < 0.0;
        for _ in 0..600 {
            s.step(&hung, 0.0, 0.0, 10.0, 1.0 / 120.0);
            if (s.angle < 0.0) != side {
                side = !side;
                crossed += 1;
            }
        }
        // Five seconds on a lanyard this long is three or four swings either way.
        assert!(crossed >= 4, "it crossed plumb {crossed} times");
    }

    /// And a boat rolling under it throws it, which a thing that merely followed the
    /// heel could not do: at the moment she is level and rolling hardest, it is not.
    #[test]
    fn the_roll_throws_it() {
        let mut s = Swing::default();
        s.step(&creel(), 0.0, 1.2, 10.0, 1.0 / 60.0);
        assert!(s.rate < 0.0, "a roll should have thrown it");
    }

    /// It bounces off her topsides rather than stopping against them.
    #[test]
    fn it_comes_off_the_planking() {
        let hung = creel();
        let mut s = Swing {
            angle: hung.inboard - 0.01,
            rate: 1.5,
        };
        s.step(&hung, 0.0, 0.0, 10.0, 1.0 / 60.0);
        assert_eq!(s.angle, hung.inboard);
        assert!(s.rate < 0.0, "it should have come back off");
        assert!(s.rate.abs() < 1.5, "and left most of its speed in the bang");
    }

    /// In the water it loses its weight and its swing. A buoy that dips a wave and
    /// carries on swinging is the tell that nobody modelled the water.
    #[test]
    fn the_water_takes_the_swing_out_of_it() {
        let dry = Hung {
            buoyant: 1.1,
            ..creel()
        };
        let settle = |clearance: f32| {
            let mut s = Swing {
                angle: -0.5,
                rate: 0.0,
            };
            for _ in 0..240 {
                s.step(&dry, 0.0, 0.0, clearance, 1.0 / 120.0);
            }
            s.rate.abs()
        };
        // Hung clear of the surface, and hung with its bottom half a metre into it.
        let in_air = settle(10.0);
        let in_water = settle(0.55);
        assert!(
            in_water < in_air * 0.5,
            "wet {in_water} against dry {in_air}"
        );
    }

    /// Swinging out lifts it clear, which is why a buoy skips along a surface instead of
    /// ploughing through it.
    #[test]
    fn swinging_out_lifts_it_out_of_the_water() {
        let hung = creel();
        let hanging = Swing {
            angle: 0.0,
            rate: 0.0,
        };
        let swung = Swing {
            angle: -0.7,
            rate: 0.0,
        };
        assert!(hanging.immersion(&hung, 0.0, 0.75) > swung.immersion(&hung, 0.0, 0.75));
    }
}
