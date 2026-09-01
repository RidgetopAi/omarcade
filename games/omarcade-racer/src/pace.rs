//! The reference driver: what "a competent lap" *is*, in code.
//!
//! Several things need the same answer to "how fast can this course be
//! driven cleanly?" — the track probe judges the course by it, the traffic
//! probe measures overtakes against it, and the qualifying threshold is
//! *derived* from it (the plan's rule: simulate a competent lap, add a
//! margin; never pick a number). Before this module each probe carried its
//! own driver and they disagreed by eighteen seconds on the same course.
//! Neither difference was a judgement call:
//!
//! - one driver clamped the holdable speed to 1.0 and *then* applied its
//!   corner margin, so the margin bit on straights too. It never exceeded
//!   78% of top speed anywhere, and reported that as "braking";
//! - the same driver solved the corner balance without the speed factor on
//!   steering, giving a square root where the physics is linear. That is
//!   optimistic past the limit bend, and it was patched with the very
//!   margin above.
//!
//! One driver, here, consumed by everything. If the physics in `drive`
//! changes, this is the one place the driver's model of it lives.

use crate::drive::{Drive, Surface, Tuning};
use crate::road::Road;

/// The simulation step the reference lap is driven at.
///
/// 240 Hz, the same as the probes have always used, so a lap time from
/// here is comparable with every number in the project's record. It is
/// well above the game's frame rate on purpose: the reference is a
/// property of the course and the car, not of how fast the screen
/// refreshes.
pub const DT: f32 = 1.0 / 240.0;

/// How hard the driver steers back toward the centre line, in units of
/// full lock per half-width of offset.
///
/// At 3.0 the correction saturates a third of the way to the verge. This
/// is the gain every probe has driven with, kept so the reference lap
/// matches the record, and it is not load-bearing for the corner speed:
/// the balance point below is reached at exactly the offset where the
/// correction saturates, for any gain that saturates before the verge.
pub const CENTRE_GAIN: f32 = 3.0;

/// The fastest a bend can be held at, as a fraction of top speed.
///
/// From the balance in [`Drive::update`]: steering moves the car at
/// `steer_rate × authority` and the bend pushes it back at
/// `curve × authority² × centrifugal`, where `authority` is speed as a
/// fraction of top. At full lock they cancel when
///
/// ```text
/// authority = steer_rate / (curve × centrifugal)
/// ```
///
/// which is `BRAKE_BEND / curve` — **linear** in the bend. Above 1.0 the
/// bend is holdable flat out and the value is simply "more than you have".
/// A straight returns infinity rather than a clamp, so that a margin
/// applied to the result cannot slow the car where there is no corner.
///
/// Measured, not just derived: on a constant bend of normalised curve
/// 1.49 a search for the fastest speed that stays on the tarmac finds
/// 0.68 against 0.67 from this formula. The square-root form that used
/// to live in the track probe gives 0.82 for the same bend, and a driver
/// trusting it spends nine seconds a lap off the road.
pub fn holdable(curve: f32, tuning: &Tuning) -> f32 {
    let curve = curve.abs();
    if curve > f32::EPSILON {
        tuning.steer_rate / (curve * tuning.centrifugal)
    } else {
        f32::INFINITY
    }
}

/// The worst bend inside braking range of the car, in `Road::curve_at`
/// units.
///
/// Braking distance is the *average* speed over the stop, so half of
/// `speed × brake_time`, and every bend inside it matters, not only the
/// one at the far end: sampling a single point ahead steps straight over
/// the entry of a corner that begins sooner, and a driver doing that
/// under-braked and blamed the course.
pub fn bend_ahead(car: &Drive, road: &Road, tuning: &Tuning) -> f32 {
    let look = car.speed * tuning.brake_time * 0.5;
    let steps = 12;
    (0..=steps)
        .map(|k| road.curve_at(car.z + look * k as f32 / steps as f32).abs())
        .fold(0.0, f32::max)
}

/// The inputs the driver chose this step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Inputs {
    pub throttle: f32,
    pub brake: f32,
    pub steer: f32,
}

/// A driver that takes every corner at a fixed fraction of what it can
/// hold, and everything else flat out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pacer {
    /// Fraction of the holdable corner speed to aim for. 1.0 is the
    /// physics-exact driver; below it is a driver leaving something in
    /// hand; above it is a driver who will leave the road.
    ///
    /// It scales the *corner* speed only. The target is clamped to top
    /// speed after the margin is applied, never before, so a margin can
    /// never slow the car on a straight — the bug this module replaced.
    pub margin: f32,
}

impl Pacer {
    /// The driver who goes exactly as fast as the physics allows.
    pub const EXACT: Pacer = Pacer { margin: 1.0 };

    /// The speed this driver wants right now, in world units per second.
    pub fn target(&self, car: &Drive, road: &Road, tuning: &Tuning) -> f32 {
        let corner = holdable(bend_ahead(car, road, tuning), tuning);
        tuning.top_speed * (corner * self.margin).min(1.0)
    }

    /// Choose inputs for this step. Brake if over target, otherwise full
    /// throttle; steer back toward the centre line.
    pub fn inputs(&self, car: &Drive, road: &Road, tuning: &Tuning) -> Inputs {
        let target = self.target(car, road, tuning);
        let (throttle, brake) = if car.speed > target { (0.0, 1.0) } else { (1.0, 0.0) };
        let steer = (-car.x * CENTRE_GAIN).clamp(-1.0, 1.0);
        Inputs { throttle, brake, steer }
    }

    /// Choose inputs and advance the car by `dt`. Returns what was chosen,
    /// so a caller can count braking time or log the decision.
    pub fn step(&self, car: &mut Drive, road: &Road, tuning: &Tuning, dt: f32) -> Inputs {
        let inputs = self.inputs(car, road, tuning);
        car.update(dt, inputs.throttle, inputs.brake, inputs.steer, road, tuning);
        inputs
    }
}

/// What one lap by a [`Pacer`] looked like.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lap {
    /// Seconds from the start line back to it.
    pub time: f32,
    /// Seconds with the brake applied.
    pub braking: f32,
    /// Seconds with any part of the car off the tarmac — rumble or grass.
    pub off_road: f32,
    /// The slowest the car went, in world units per second.
    pub slowest: f32,
    /// The furthest the car got from the centre line, in half-widths.
    pub max_stray: f32,
}

/// Drive one lap of `road` from the start line at top speed and report it.
///
/// From top speed rather than a standing start because the lap is a
/// property of the course: a flying lap is the same whichever lap of a
/// race it is, and the qualifying lap the game asks for is a flying one.
///
/// Laps are counted by distance travelled, not by `z` wrapping — the car
/// wraps once per lap but a start line offset would make that count
/// short, and distance does not care where the line is.
pub fn lap(pacer: Pacer, road: &Road, tuning: &Tuning) -> Lap {
    let mut car = Drive::new();
    car.speed = tuning.top_speed;
    drive_distance(pacer, road, tuning, car, road.length())
}

/// Drive `distance` from `start` and report it.
///
/// The general form of [`lap`]: a standing start from the grid, a lap
/// and a half, whatever the question needs. The qualifying lap the game
/// asks for starts from rest behind the line, so its reference must too.
pub fn drive_distance(
    pacer: Pacer,
    road: &Road,
    tuning: &Tuning,
    start: Drive,
    distance: f32,
) -> Lap {
    let mut car = start;
    let mut report = Lap {
        time: 0.0,
        braking: 0.0,
        off_road: 0.0,
        slowest: car.speed,
        max_stray: 0.0,
    };
    let mut travelled = 0.0f32;
    // A lap at walking pace on a long course would run forever; nothing
    // sensible takes ten times the flat-out time.
    let ceiling = distance / tuning.top_speed * 10.0;
    while travelled < distance && report.time < ceiling {
        let before = car.speed;
        let inputs = pacer.step(&mut car, road, tuning, DT);
        travelled += (before + car.speed) * 0.5 * DT;
        report.time += DT;
        if inputs.brake > 0.0 {
            report.braking += DT;
        }
        if car.surface() != Surface::Road {
            report.off_road += DT;
        }
        report.slowest = report.slowest.min(car.speed);
        report.max_stray = report.max_stray.max(car.x.abs());
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::road::Segment;
    use crate::track::grand_prix;

    fn bend(raw: f32) -> Road {
        Road::new(vec![Segment::curving(raw); 400], 200.0, 2200.0)
    }

    /// The fastest fraction of top speed that keeps the car on the tarmac
    /// for a long time, found by SEARCH rather than by formula. This is
    /// the independent information the formula tests need: it knows
    /// nothing about the algebra, only whether the car left the road.
    fn searched_holdable(road: &Road, tuning: &Tuning) -> f32 {
        let dt = 1.0 / 120.0;
        let mut frac = 1.0f32;
        while frac > 0.05 {
            let target = tuning.top_speed * frac;
            let mut d = Drive::new();
            let mut left = false;
            // Long enough that a car drifting outward at the slow rate a
            // near-balance produces still reaches the verge. A six-second
            // window once made 0.73 look holdable on a bend whose true
            // answer is 0.68.
            for _ in 0..(40.0 / dt) as usize {
                let steer = (-d.x * CENTRE_GAIN).clamp(-1.0, 1.0);
                let (throttle, brake) = if d.speed > target { (0.0, 1.0) } else { (1.0, 0.0) };
                d.update(dt, throttle, brake, steer, road, tuning);
                if d.surface() != Surface::Road {
                    left = true;
                    break;
                }
            }
            if !left {
                return frac;
            }
            frac -= 0.01;
        }
        0.05
    }

    /// The formula is the physics, not the square root of it.
    ///
    /// Both forms agree on holdable bends, so the fixture is one past the
    /// limit, where they part: the search is the arbiter, and the sqrt
    /// form must be visibly wrong or this test could pass with either.
    #[test]
    fn holdable_matches_the_search_and_the_square_root_does_not() {
        let road = bend(90.0);
        let tuning = Tuning::from_corner(&road, 1.5);
        let curve = road.curve_at(0.0).abs();
        assert!(curve > 1.2, "fixture must be past the limit bend, got {curve}");

        let measured = searched_holdable(&road, &tuning);
        let linear = holdable(curve, &tuning);
        let sqrt = (tuning.steer_rate / (curve * tuning.centrifugal)).sqrt();

        assert!(
            (measured - linear).abs() <= 0.02,
            "search found {measured}, formula says {linear}",
        );
        assert!(
            (measured - sqrt).abs() > 0.1,
            "the square-root form ({sqrt}) is too close to the search ({measured}) \
             for this test to tell the formulas apart",
        );
    }

    /// A straight is driven flat out at ANY margin.
    ///
    /// The bug: clamp to 1.0, then multiply by the margin, and a 0.78
    /// margin becomes a 78% speed limit everywhere. A margin under 1.0 is
    /// the case that catches it; 1.0 would pass either way.
    #[test]
    fn a_straight_is_flat_out_whatever_the_margin() {
        let road = Road::straight(400);
        let tuning = Tuning::from_corner(&road, 1.5);
        let mut car = Drive::new();
        car.speed = tuning.top_speed;
        for margin in [0.5, 0.78, 0.9, 1.0] {
            let target = Pacer { margin }.target(&car, &road, &tuning);
            assert_eq!(
                target, tuning.top_speed,
                "margin {margin} slowed the car on a straight to {target}",
            );
        }
    }

    /// On a bend past the limit the margin scales the target exactly.
    #[test]
    fn the_margin_scales_the_corner_speed() {
        let road = bend(90.0);
        let tuning = Tuning::from_corner(&road, 1.5);
        let mut car = Drive::new();
        car.speed = tuning.top_speed;
        let exact = Pacer::EXACT.target(&car, &road, &tuning);
        let eased = Pacer { margin: 0.8 }.target(&car, &road, &tuning);
        assert!(exact < tuning.top_speed, "fixture bend must demand braking");
        assert!(
            (eased - exact * 0.8).abs() < 1.0,
            "margin 0.8 gave {eased} against {exact} exact",
        );
    }

    /// The exact driver laps the grand prix without touching the verge,
    /// in a time between the flat-out floor and a modest amount over it.
    ///
    /// The floor is the independent number here: a lap slower than 15%
    /// over it means the driver is losing time somewhere other than the
    /// corners, which is what the old capped driver did (+32%).
    #[test]
    fn the_exact_pacer_laps_the_grand_prix_on_the_tarmac() {
        let road = grand_prix().build();
        let tuning = Tuning::from_corner(&road, 1.5);
        let floor = road.length() / tuning.top_speed;
        let report = lap(Pacer::EXACT, &road, &tuning);
        assert_eq!(report.off_road, 0.0, "the exact driver left the road: {report:?}");
        assert!(report.max_stray < 0.5, "strayed {} of the way to the verge", report.max_stray);
        assert!(report.time > floor, "faster than flat out is not possible: {report:?}");
        assert!(
            report.time < floor * 1.15,
            "lap {:.1}s against a {:.1}s floor — time lost off the corners",
            report.time, floor,
        );
        assert!(report.braking > 1.0, "the course has bends; nobody braked: {report:?}");
    }

    /// The tarmac assertion above is live: a driver asking for more than
    /// the physics allows DOES leave the road on this course. Without
    /// this, "never left the road" could be a course with no real bends.
    #[test]
    fn a_margin_over_the_limit_leaves_the_road() {
        let road = grand_prix().build();
        let tuning = Tuning::from_corner(&road, 1.5);
        let report = lap(Pacer { margin: 1.15 }, &road, &tuning);
        assert!(report.off_road > 0.5, "an over-the-limit driver stayed on: {report:?}");
    }

    /// Braking starts BEFORE the bend. A driver that reads only the curve
    /// under the car arrives at the corner still at top speed.
    #[test]
    fn the_driver_brakes_on_the_approach() {
        // A long straight, then a bend past the limit.
        let mut segments = vec![Segment::curving(0.0); 300];
        segments.extend(vec![Segment::curving(90.0); 200]);
        let road = Road::new(segments, 200.0, 2200.0);
        let tuning = Tuning::from_corner(&road, 1.5);
        let mut car = Drive::new();
        car.speed = tuning.top_speed;
        // Stand just short of the bend, inside braking range.
        car.z = 300.0 * 200.0 - car.speed * tuning.brake_time * 0.25;
        assert_eq!(road.curve_at(car.z), 0.0, "fixture: the car must still be on the straight");
        let inputs = Pacer::EXACT.inputs(&car, &road, &tuning);
        assert_eq!(inputs.brake, 1.0, "no braking with a hard bend in range: {inputs:?}");
    }
}
