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

/// How far ahead a car must be before its sprite stops overlapping the
/// player's, in SEGMENTS.
///
/// ⚠️ MEASURED THROUGH THE PROJECTION, NOT DERIVED FROM THE ROAD'S WIDTH.
/// The first version of this computed a car's length as 2.5x its width in
/// the road's LATERAL units and got 1729 — which is 5.4x too far. Brian
/// crashed into a car that was visibly most of the way up the road and
/// sent a screenshot; at 1729 units a car draws 12% of the player's
/// height, a small sprite near the horizon.
///
/// THE MISTAKE WAS TREATING TWO AUTHORING NUMBERS AS ONE SCALE. The
/// road's width (2200 units) and the track's segment length (200 units)
/// are independent — nothing ever required them to agree — so "2.5 times
/// the car's width" is a sentence about lateral units and means nothing
/// along z. There is no conversion between them to get right; the
/// relationship only exists in the projection.
///
/// So it is measured there: `probe_contact` walks a car toward the
/// player through the real camera and reports where the drawn sprites
/// first touch. The answer is 1.6 segments. Re-run that probe if the
/// camera fill, the draw distance, or the car art changes.
pub const CONTACT_SEGMENTS: f32 = 1.6;

/// What the player hit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    /// Index of the traffic car that was struck.
    pub car: usize,
    /// Where it happened, in world units along the track. The fireball
    /// is lit here, so it stays put and recedes with the road.
    pub z: f32,
    /// Where the PLAYER was at the moment of contact.
    ///
    /// ⚠️ NOT where the player ended the frame. The check is swept, so
    /// by the time it runs the car has already been moved past the point
    /// of impact — by up to a full frame's travel (267 units at 60fps,
    /// 1067 at the clamped 15fps). Lighting the fireball and then leaving
    /// the player at the frame-end position put the fire BEHIND the car,
    /// which is what Brian saw: "it's like it's behind you slightly".
    ///
    /// The caller stops the player here, so the wreck comes to rest at
    /// the point of contact with the fire in front of it.
    pub player_z: f32,
    /// Lateral position of the impact, in half-widths.
    pub x: f32,
    /// How fast the player was closing when it happened, world units per
    /// second. Available for scoring or damage; nothing reads it yet.
    pub closing: f32,
}

/// The separation at which two cars are touching, in world units.
///
/// In SEGMENTS, because that is the unit the projection actually works
/// in — the player's own sprite is sized from a probe one segment ahead
/// (see `render.rs`), so segments are what the drawn scene is calibrated
/// against. The road's lateral width is a different scale entirely and
/// cannot be converted to this one.
pub fn contact_distance(road: &Road) -> f32 {
    CONTACT_SEGMENTS * road.segment_length()
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
pub fn check(
    player: &Drive,
    prev_z: f32,
    traffic: &Field,
    road: &Road,
) -> Option<Hit> {
    let length = road.length();
    let contact = contact_distance(road);

    // ⚠️ SWEPT, NOT AN OVERLAP TEST. This is the Breakout tunnelling bug
    // (session 2) and it is REAL here, not theoretical: once the contact
    // range was measured properly at 320 units, a single frame at 30fps
    // covers 533 and at the clamped 15fps covers 1067. A car can sit
    // entirely between two samples.
    //
    // The first version tested the player's CURRENT position only. It
    // passed its own tunnelling guard because the threshold was 1729
    // units — five times too large — so the bug was hidden by another
    // bug. Fixing the threshold made the guard fail immediately, which
    // is exactly why it was written as an assertion.
    //
    // The sweep is over the segment the player travelled this frame:
    // anything whose contact band that segment crosses was hit, however
    // briefly.
    let travelled = (player.z - prev_z).rem_euclid(length);
    // A wrap or a reset contributes nothing rather than a near-full-lap
    // sweep that would collide with the entire field.
    let travelled = if travelled > length / 2.0 { 0.0 } else { travelled };

    for (i, car) in traffic.cars.iter().enumerate() {
        // ⚠️ YOU CRASH INTO CARS; CARS DO NOT CRASH INTO YOU. Only a car
        // the player is CATCHING can be hit. The traffic is blind
        // (decision 4a0707a3) and faster than a stopped car, so a car
        // that drives through the wreck while it burns is sitting inside
        // the contact range the moment the burn ends — and without this
        // rule the first frame of throttle was a second crash. Brian:
        // "before I can move a few pixels I get crashed into from ai
        // car". Pole Position had the same rule for the same reason:
        // every collision in it is the player arriving at a car.
        if player.speed <= car.speed {
            continue;
        }

        // Lateral overlap: the sprites are drawn touching when their
        // centres are closer than one car width.
        if (player.x - car.x).abs() >= CAR_WIDTH_HALF_WIDTHS {
            continue;
        }

        // How far ahead of where the player STARTED this frame the car
        // is. Everything is measured forward from there.
        let ahead_of_start = (car.z - prev_z).rem_euclid(length);

        // The car's contact band, as an interval forward from prev_z.
        // The sweep covers [0, travelled]; the band is the car's
        // position plus or minus the contact range.
        let band_near = ahead_of_start - contact;
        let band_far = ahead_of_start + contact;

        // Do the swept interval and the contact band overlap?
        if band_far < 0.0 || band_near > travelled {
            continue;
        }

        // Where along the sweep the contact happened. The car's contact
        // band starts at `band_near` measured forward from `prev_z`; if
        // that is behind the start of the sweep the player was already
        // touching, so contact is at the sweep's start.
        let travelled_to_contact = band_near.max(0.0);
        let player_z = prev_z + travelled_to_contact;

        return Some(Hit {
            car: i,
            z: car.z,
            x: car.x,
            player_z,
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
        let contact = contact_distance(&road);
        let traffic = one_car_at(&road, 10_000.0, 0.0);

        let player = Drive {
            z: road.wrap(10_000.0 - contact * 0.5),
            x: 0.0,
            speed: 12_000.0,
        };
        assert!(check(&player, road.wrap(player.z - road.segment_length()), &traffic, &road).is_some());
    }

    #[test]
    fn a_car_a_clear_gap_ahead_is_not_a_hit() {
        let road = course();
        let contact = contact_distance(&road);
        let traffic = one_car_at(&road, 10_000.0, 0.0);

        let player = Drive {
            z: road.wrap(10_000.0 - contact * 1.5),
            x: 0.0,
            speed: 12_000.0,
        };
        assert!(check(&player, road.wrap(player.z - road.segment_length()), &traffic, &road).is_none());
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
            check(&player, road.wrap(player.z - road.segment_length()), &traffic, &road).is_none(),
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
            check(&player, road.wrap(player.z - road.segment_length()), &traffic, &road).is_some(),
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
        let contact = contact_distance(&road);

        let traffic = one_car_at(&road, contact * 0.25, 0.0);
        let player = Drive {
            z: road.wrap(length - contact * 0.25),
            x: 0.0,
            speed: 12_000.0,
        };
        assert!(
            check(&player, road.wrap(player.z - road.segment_length()), &traffic, &road).is_some(),
            "contact across the start line was missed — the z comparison \
             is not wrapping"
        );
    }

    /// ⚠️ THE TUNNELLING GUARD, and it EARNED ITS KEEP.
    ///
    /// Written first as "a frame must move less than the contact range",
    /// which passed only because the contact range was 1729 units — five
    /// times too large. Brian crashed into a car most of the way up the
    /// road and sent a screenshot; measuring the real contact distance
    /// (320 units, `probe_contact`) made this test fail immediately,
    /// because at 30fps a frame covers 533 units and at the clamped
    /// 15fps it covers 1067. One bug was hiding the other.
    ///
    /// So `check` is SWEPT now, and this asserts the property that
    /// actually matters: a car sitting entirely between two samples is
    /// still hit. The old formulation is deliberately not restored —
    /// requiring the frame to be shorter than a car would cap top speed
    /// at a third of its current value.
    #[test]
    fn a_frame_cannot_step_over_a_car() {
        use crate::drive::Tuning;

        let road = course();
        let tuning = Tuning::from_corner(&road, 1.5);
        let contact = contact_distance(&road);

        // The dt clamp in main.rs — the worst case a frame can be.
        const MAX_DT: f32 = 1.0 / 15.0;
        let step = tuning.top_speed * MAX_DT;
        assert!(
            step > contact,
            "this test is vacuous: a frame ({step:.0}) no longer outruns the \
             contact range ({contact:.0}), so there is nothing to tunnel"
        );

        // A car placed squarely in the middle of one frame's travel:
        // present at neither the start position nor the end position.
        let start_z = road.wrap(10_000.0);
        let end_z = road.wrap(start_z + step);
        let traffic = one_car_at(&road, start_z + step * 0.5, 0.0);

        let player = Drive {
            z: end_z,
            x: 0.0,
            speed: tuning.top_speed,
        };

        // Prove the setup: neither endpoint alone would find it.
        let at_end = (player.z - traffic.cars[0].z).rem_euclid(road.length());
        let sep_end = at_end.min(road.length() - at_end);
        assert!(
            sep_end > contact,
            "fixture is wrong — the car is within contact of the END position, \
             so this would pass without any sweep"
        );

        assert!(
            check(&player, start_z, &traffic, &road).is_some(),
            "a car sitting between two frames was stepped clean over"
        );
    }

    /// The fireball must end up IN FRONT of the stopped wreck.
    ///
    /// ⚠️ THE SWEEP CREATED THIS PROBLEM AND THE REWIND SOLVES IT. By the
    /// time `check` runs, `car.update` has already carried the player
    /// past the point of impact — up to a full frame's travel, 267 units
    /// at 60fps and 1067 at the clamped 15fps. Lighting the fire at the
    /// struck car's position while leaving the player at the frame-end
    /// position put the fire BEHIND the wreck. Brian drove it and
    /// reported exactly that: "it's like it's behind you slightly."
    ///
    /// Checked at several frame rates because the error scales with the
    /// frame: a 60fps-only test would show the smallest version of it.
    #[test]
    fn the_fireball_lands_in_front_of_the_stopped_wreck() {
        use crate::drive::Tuning;

        let road = course();
        let tuning = Tuning::from_corner(&road, 1.5);
        let contact = contact_distance(&road);
        let length = road.length();

        for fps in [60.0f32, 30.0, 15.0] {
            let step = tuning.top_speed / fps;
            let prev = road.wrap(10_000.0);
            let end = road.wrap(prev + step);

            // ⚠️ THE CAR MUST SIT AT THE START OF THE SWEEP, not near its
            // end. Placed ahead of the frame-end position the fire is in
            // front whether or not the rewind happens, and this test
            // passes against the very bug it guards — confirmed by
            // mutation. A car the player drives ENTIRELY PAST in one
            // frame is the case that fails without the rewind, and it is
            // also the case Brian hit: at speed, contact is detected
            // after the car is already behind you.
            let traffic = one_car_at(&road, prev + contact * 0.5, 0.0);
            let moved = Drive {
                z: end,
                x: 0.0,
                speed: tuning.top_speed,
            };

            let hit = check(&moved, prev, &traffic, &road)
                .unwrap_or_else(|| panic!("no contact detected at {fps}fps"));

            // Forward distance from where the player comes to rest to
            // where the fire is lit. Wrapped, and it must be a SHORT
            // forward distance rather than nearly a full lap.
            let ahead = (hit.z - hit.player_z).rem_euclid(length);
            assert!(
                ahead < contact * 2.0,
                "at {fps}fps the fire is {ahead:.0} units from the wreck — \
                 that is behind it, or most of a lap away"
            );
            assert!(
                ahead > 0.0,
                "at {fps}fps the fire is exactly on the wreck"
            );
        }
    }

    #[test]
    fn an_empty_field_cannot_be_hit() {
        let road = course();
        let traffic = Field::default();
        let player = Drive::new();
        assert!(check(&player, road.wrap(player.z - road.segment_length()), &traffic, &road).is_none());
    }

    /// A faster car passing through a slower player is not a crash —
    /// only a car the player is catching can be hit. Without this, the
    /// blind traffic drove through a burning wreck and the restart was
    /// a second crash on the first frame of throttle.
    #[test]
    fn a_car_you_are_not_catching_cannot_be_hit() {
        let road = course();
        let contact = contact_distance(&road);
        let mut field = one_car_at(&road, 1000.0 + contact * 0.5, 0.0);
        // The car is faster than the player and inside contact range.
        field.cars[0].speed = 9000.0;
        let player = Drive { z: 1000.0, x: 0.0, speed: 4000.0 };
        assert_eq!(check(&player, 950.0, &field, &road), None, "hit by a car that was overtaking");
        // A stopped player, a car driving through it: nothing.
        let stopped = Drive { z: 1000.0, x: 0.0, speed: 0.0 };
        assert_eq!(check(&stopped, 1000.0, &field, &road), None, "hit while stationary");
        // The same geometry with the player faster IS a hit.
        let catching = Drive { z: 1000.0, x: 0.0, speed: 9500.0 };
        assert!(check(&catching, 950.0, &field, &road).is_some(), "catching the car should hit it");
    }
}
