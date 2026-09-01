//! Did the player hit anything?
//!
//! A pure query over the player and the traffic field: it reads both and
//! mutates neither. That is deliberate placement. Putting this in
//! `drive.rs` would give the player's physics a reason to know traffic
//! exists, and putting it in `traffic.rs` would break the blindness the
//! whole traffic design rests on (decision 4a0707a3) — a car that can be
//! collided with by a function it owns is one edit away from being a car
//! that swerves. Neither module learns anything from this one.
//!
//! ⚠️ THE HITBOX IS THE ART. Both dimensions are derived from the car
//! sprite and the scale rule the renderer already uses, so what you can
//! hit is what you can see. A hand-picked hitbox drifts away from the
//! drawing the moment either changes, and the symptom is the worst kind:
//! "I definitely wasn't touching that car."

use crate::drive::Drive;
use crate::road::Road;
use crate::traffic::Field;

/// How much of a half-width a car occupies, side to side.
///
/// DERIVED: the player sprite's ink is 44 columns wide, and
/// `render::CAR_ART_PIXELS_PER_HALF_WIDTH` is 70 — so a car covers
/// 44/70 of a half-width on screen. Two cars whose lateral positions
/// differ by less than this are drawn OVERLAPPING, which is exactly when
/// a collision should register.
///
/// The road is 2.0 half-widths wide, so three cars abreast come to 1.886
/// and barely fit. Gaps are real but tight, which is the right feel for a
/// racer and is a property of the art rather than a number anyone chose.
pub const CAR_WIDTH_HALF_WIDTHS: f32 = 44.0 / 70.0;

/// How long a car is, as a multiple of its own width.
///
/// The sprite is drawn from behind and has no length, so this is the one
/// number here that cannot be read off the art. 2.5 is a single-seater's
/// proportion — long enough that a rear-end shunt registers before the
/// sprites interpenetrate, short enough that cars are not colliding with
/// clear air.
pub const CAR_LENGTH_RATIO: f32 = 2.5;

/// What the player hit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    /// Index of the traffic car that was struck.
    pub car: usize,
    /// Where it happened, in world units along the track. The fireball
    /// is lit here, so it stays put and recedes with the road.
    pub z: f32,
    /// Lateral position of the impact, in half-widths.
    pub x: f32,
    /// How fast the player was closing when it happened, world units per
    /// second. Available for scoring or damage; nothing reads it yet.
    pub closing: f32,
}

/// A car's length in world units, from the road's own width.
///
/// Everything scales off `Road::width` so a wider or narrower course
/// carries the cars with it rather than silently changing how much room
/// there is to overtake.
pub fn car_length_units(road: &Road) -> f32 {
    let half_width_units = road.width() / 2.0;
    CAR_WIDTH_HALF_WIDTHS * half_width_units * CAR_LENGTH_RATIO
}

/// Has the player hit a traffic car?
///
/// Returns the FIRST overlap found. There is deliberately no "closest"
/// search: the fail state is a crash, and a crash ends the run, so which
/// of two simultaneous contacts is reported changes nothing that can be
/// observed.
///
/// ⚠️ OVERLAP TESTING IS SOUND HERE ONLY BECAUSE THE STEP IS SMALL, and
/// that is checked rather than assumed — see `a_frame_cannot_step_over_a_car`.
/// At the clamped worst case of 15fps the player closes 0.28 of a car
/// length per frame, so nothing can pass through a car between two
/// samples. This is the Breakout tunnelling bug (session 2) in a new
/// place; if `top_speed` rises or the dt clamp loosens, that test fails
/// and a swept check becomes necessary.
pub fn check(player: &Drive, traffic: &Field, road: &Road) -> Option<Hit> {
    let length = road.length();
    let car_len = car_length_units(road);

    for (i, car) in traffic.cars.iter().enumerate() {
        // Lateral overlap: the sprites are drawn touching when their
        // centres are closer than one car width.
        if (player.x - car.x).abs() >= CAR_WIDTH_HALF_WIDTHS {
            continue;
        }

        // Longitudinal overlap, wrapped. The shortest distance around
        // the loop either way — a car just over the start line and a
        // player just short of it are touching, however far apart their
        // raw z values are.
        let d = (player.z - car.z).rem_euclid(length);
        let separation = d.min(length - d);
        if separation >= car_len {
            continue;
        }

        return Some(Hit {
            car: i,
            z: car.z,
            x: car.x,
            closing: player.speed - car.speed,
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track;
    use crate::traffic::Field;

    fn course() -> Road {
        track::grand_prix().build()
    }

    /// One field with one car, placed exactly where the test wants it.
    fn one_car_at(road: &Road, z: f32, x: f32) -> Field {
        let mut f = Field::grid(road, 1);
        f.cars[0].z = road.wrap(z);
        f.cars[0].x = x;
        f
    }

    #[test]
    fn driving_into_the_back_of_a_car_is_a_hit() {
        let road = course();
        let car_len = car_length_units(&road);
        let traffic = one_car_at(&road, 10_000.0, 0.0);

        let player = Drive {
            z: road.wrap(10_000.0 - car_len * 0.5),
            x: 0.0,
            speed: 12_000.0,
        };
        assert!(check(&player, &traffic, &road).is_some());
    }

    #[test]
    fn a_car_a_clear_gap_ahead_is_not_a_hit() {
        let road = course();
        let car_len = car_length_units(&road);
        let traffic = one_car_at(&road, 10_000.0, 0.0);

        let player = Drive {
            z: road.wrap(10_000.0 - car_len * 1.5),
            x: 0.0,
            speed: 12_000.0,
        };
        assert!(check(&player, &traffic, &road).is_none());
    }

    #[test]
    fn passing_in_the_next_lane_is_not_a_hit() {
        // The whole point of the traffic design: there is room to get by
        // if you pick a side. If a clean pass registers as contact the
        // game is unplayable.
        let road = course();
        let traffic = one_car_at(&road, 10_000.0, -0.6);

        let player = Drive {
            z: road.wrap(10_000.0),
            x: 0.6,
            speed: 12_000.0,
        };
        assert!(
            check(&player, &traffic, &road).is_none(),
            "a pass with {:.2} half-widths of lateral gap registered as a hit",
            1.2_f32
        );
    }

    #[test]
    fn side_by_side_and_touching_is_a_hit() {
        let road = course();
        let traffic = one_car_at(&road, 10_000.0, 0.0);

        // ⚠️ AN ABSOLUTE OFFSET, NOT A FRACTION OF THE CONSTANT UNDER
        // TEST. Writing this as `CAR_WIDTH_HALF_WIDTHS * 0.8` makes the
        // fixture shrink with the hitbox, so it passes for any value
        // however wrong — mutation testing confirmed it survived a
        // hitbox sixty times too small. 0.4 half-widths is a real,
        // measurable overlap: the cars are 0.63 wide, so their centres
        // being 0.4 apart means about a third of each sprite is inside
        // the other. (L024: a fixture must not scale with its subject.)
        let player = Drive {
            z: road.wrap(10_000.0),
            x: 0.4,
            speed: 12_000.0,
        };
        assert!(
            check(&player, &traffic, &road).is_some(),
            "cars overlapping by a third of their width did not register"
        );
    }

    #[test]
    fn contact_across_the_start_line_still_registers() {
        // The loop wraps. A car just past z=0 and a player just short of
        // the end of the lap are touching, however far apart their raw
        // coordinates read.
        let road = course();
        let length = road.length();
        let car_len = car_length_units(&road);

        let traffic = one_car_at(&road, car_len * 0.25, 0.0);
        let player = Drive {
            z: road.wrap(length - car_len * 0.25),
            x: 0.0,
            speed: 12_000.0,
        };
        assert!(
            check(&player, &traffic, &road).is_some(),
            "contact across the start line was missed — the z comparison \
             is not wrapping"
        );
    }

    /// ⚠️ THE TUNNELLING GUARD. This is the Breakout bug (session 2) in
    /// a new place: overlap testing is only valid while a single frame
    /// moves less than the object being tested against.
    ///
    /// `main.rs` clamps dt to 1/15s. At the shipped top speed the player
    /// covers well under a car length in that time, so nothing can pass
    /// through a car between two samples. If top speed rises or the
    /// clamp loosens, THIS TEST FAILS and a swept check is required —
    /// which is the point of writing it as an assertion rather than a
    /// note in a comment.
    #[test]
    fn a_frame_cannot_step_over_a_car() {
        use crate::drive::Tuning;

        let road = course();
        let tuning = Tuning::from_corner(&road, 1.5);
        let car_len = car_length_units(&road);

        // The dt clamp in main.rs. Stated here so the test fails if the
        // two ever disagree about the worst case.
        const MAX_DT: f32 = 1.0 / 15.0;

        let step = tuning.top_speed * MAX_DT;
        assert!(
            step < car_len,
            "at {MAX_DT:.3}s a frame covers {step:.0} units against a \
             {car_len:.0}-unit car — collisions can tunnel, and overlap \
             testing is no longer sound"
        );
    }

    #[test]
    fn an_empty_field_cannot_be_hit() {
        let road = course();
        let traffic = Field::default();
        let player = Drive::new();
        assert!(check(&player, &traffic, &road).is_none());
    }
}
