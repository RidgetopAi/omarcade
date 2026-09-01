//! The points ledger: three kinds of race fact folded into one number.
//!
//! Pole Position paid for three things and so does this — ground
//! covered, cars passed, and seconds still on the clock at each mark.
//! The rates are Brian's call from the plan (ce1c8740): 50 a foot, 50 a
//! car, 200 a second. Over three laps of the grand prix that is a little
//! over two million points, nearly all of it distance, which is how the
//! arcade read too: the score mostly says how far you got, and the cars
//! and the clock are what separates two drivers who both finished.
//!
//! The ledger knows nothing about frames, files, or the marquee. `main`
//! feeds it what the race and the traffic report and reads `total` back
//! for the HUD and, at the end of a run, for the score file.
//!
//! Every constant here is stated in the unit of the thing it describes
//! (L022): feet are feet, and the conversion to world units is derived
//! from the one place a mile is defined.

use crate::track::UNITS_PER_MILE;

/// World units in one foot, from the track's own mile.
pub const UNITS_PER_FOOT: f32 = UNITS_PER_MILE / 5280.0;

/// Points for every foot of ground covered.
pub const POINTS_PER_FOOT: f32 = 50.0;
/// Points for every car passed. A recycled car passed again is a car
/// passed again — traffic is a stream, not a roster.
pub const POINTS_PER_CAR: u32 = 50;
/// Points for every second still on the clock when a mark is crossed.
pub const POINTS_PER_SECOND: f32 = 200.0;

/// The running score of one run: qualifying and the race together.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ledger {
    /// The furthest `Race::travelled` has reached this session. Ground is
    /// paid for once, the first time it is covered.
    ///
    /// A crash rewinds `travelled` a little; the car then re-drives that
    /// stretch. Paying on the high-water mark means the rewind neither
    /// refunds points already earned nor pays twice for the same road.
    high_water: f32,
    /// Feet paid for, across every session of the run.
    feet: f32,
    /// Cars passed.
    cars: u32,
    /// Seconds banked at marks.
    seconds: f32,
}

/// The ledger's three columns, in points, for a probe or a test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Breakdown {
    pub distance: u32,
    pub cars: u32,
    pub time: u32,
}

impl Ledger {
    pub fn new() -> Ledger {
        Ledger::default()
    }

    /// A green light. `Race::travelled` restarts from zero at every one,
    /// so the high-water mark must too; without this the whole first lap
    /// of the race would sit under qualifying's mark and pay nothing.
    pub fn new_session(&mut self) {
        self.high_water = 0.0;
    }

    /// The race's distance since the green light, signed, every frame.
    /// Only new ground pays.
    pub fn distance(&mut self, travelled: f32) {
        if travelled > self.high_water {
            self.feet += (travelled - self.high_water) / UNITS_PER_FOOT;
            self.high_water = travelled;
        }
    }

    /// Cars overtaken this frame.
    pub fn passed(&mut self, cars: u32) {
        self.cars = self.cars.saturating_add(cars);
    }

    /// A mark crossed with `remaining` seconds on the clock.
    pub fn checkpoint(&mut self, remaining: f32) {
        self.seconds += remaining.max(0.0);
    }

    pub fn breakdown(&self) -> Breakdown {
        Breakdown {
            distance: (self.feet * POINTS_PER_FOOT) as u32,
            cars: self.cars.saturating_mul(POINTS_PER_CAR),
            time: (self.seconds * POINTS_PER_SECOND).round() as u32,
        }
    }

    pub fn total(&self) -> u32 {
        let b = self.breakdown();
        b.distance.saturating_add(b.cars).saturating_add(b.time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::race::RACE_LAPS;
    use crate::track::grand_prix;

    #[test]
    fn a_foot_is_derived_from_the_mile() {
        // Relative, not absolute: f32 at 485,000 steps by ~0.03.
        let rebuilt = UNITS_PER_FOOT * 5280.0;
        assert!((rebuilt - UNITS_PER_MILE).abs() / UNITS_PER_MILE < 1e-6, "{rebuilt}");
    }

    #[test]
    fn three_laps_of_the_grand_prix_pay_what_the_plan_promised() {
        // The plan (ce1c8740): 2.7 miles ≈ 14,256 ft, ~2.1M points over
        // three laps. The course as built is the authority; the plan's
        // figure is the sanity check.
        let road = grand_prix().build();
        let mut ledger = Ledger::new();
        ledger.distance(road.length() * RACE_LAPS as f32);
        let distance = ledger.breakdown().distance;
        assert!(
            (2_000_000..=2_300_000).contains(&distance),
            "three laps paid {distance}, not the two-million-odd the plan expects"
        );
    }

    #[test]
    fn ground_is_paid_for_once() {
        let mut ledger = Ledger::new();
        ledger.distance(1000.0);
        let once = ledger.total();

        // A crash rewind and the re-drive back to where the car was.
        ledger.distance(600.0);
        assert_eq!(ledger.total(), once, "a rewind refunded points");
        ledger.distance(1000.0);
        assert_eq!(ledger.total(), once, "re-driving lost ground paid twice");

        ledger.distance(1200.0);
        assert!(ledger.total() > once, "new ground did not pay");
    }

    #[test]
    fn a_stationary_car_earns_nothing() {
        let mut ledger = Ledger::new();
        ledger.distance(0.0);
        ledger.distance(0.0);
        assert_eq!(ledger.total(), 0);
    }

    #[test]
    fn a_green_light_starts_the_mark_over_but_keeps_the_points() {
        let mut ledger = Ledger::new();
        ledger.distance(5000.0);
        let qualifying = ledger.total();

        // The race: travelled restarts from zero.
        ledger.new_session();
        ledger.distance(10.0);
        assert!(ledger.total() > qualifying, "the first ground of the race paid nothing");
        ledger.distance(5000.0);
        assert!(
            (ledger.total() as f32 - 2.0 * qualifying as f32).abs() <= 1.0,
            "the same distance in two sessions should pay twice: {} vs 2x{}",
            ledger.total(),
            qualifying
        );
    }

    #[test]
    fn many_small_frames_pay_the_same_as_one_step() {
        // At 30fps a frame covers a few feet. Rounding per frame would
        // bleed points; summing feet and rounding once must not.
        let mut whole = Ledger::new();
        whole.distance(30_000.0);

        let mut frames = Ledger::new();
        let mut z = 0.0;
        for _ in 0..1000 {
            z += 30.0;
            frames.distance(z);
        }
        let (a, b) = (whole.total(), frames.total());
        assert!(a.abs_diff(b) <= 1, "one step paid {a}, a thousand frames paid {b}");
    }

    #[test]
    fn every_car_passed_pays_the_same() {
        let mut ledger = Ledger::new();
        ledger.passed(1);
        ledger.passed(2);
        assert_eq!(ledger.breakdown().cars, 3 * POINTS_PER_CAR);
        assert_eq!(ledger.total(), 3 * POINTS_PER_CAR);
    }

    #[test]
    fn banked_seconds_pay_by_the_tenth() {
        let mut ledger = Ledger::new();
        ledger.checkpoint(11.3);
        assert_eq!(ledger.breakdown().time, 2260);
        // A mark reached with the clock already out banks nothing, and
        // never goes negative.
        ledger.checkpoint(-0.2);
        assert_eq!(ledger.breakdown().time, 2260);
    }

    #[test]
    fn the_total_is_the_sum_of_the_columns() {
        let mut ledger = Ledger::new();
        ledger.distance(UNITS_PER_FOOT * 100.0);
        ledger.passed(4);
        ledger.checkpoint(10.0);
        let b = ledger.breakdown();
        assert_eq!(b.distance, 5000);
        assert_eq!(b.cars, 200);
        assert_eq!(b.time, 2000);
        assert_eq!(ledger.total(), 7200);
    }
}
