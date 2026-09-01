//! The traffic: cars that drive, and that never see you.
//!
//! Decision 4a0707a3. A traffic car holds its own line at its own speed
//! and has NO awareness of the player — it does not dodge, does not
//! block, and does not brake because someone is closing. A car sitting
//! in your line will be hit unless you move.
//!
//! **That is enforced by the signature, not by discipline.**
//! [`Field::advance`] takes no player, no camera and no input; there is
//! nothing in scope for a future edit to react to without changing the
//! function's shape and the test that guards it. The reasoning is in the
//! decision, but the short version is that the fail state is a collision:
//! when contact ends the run, the obstacle field has to be predictable,
//! or every crash is arguable. Blind traffic makes every crash the
//! player's own error.
//!
//! ⚠️ THIS IS NOT [`Drive`](crate::drive::Drive) AND MUST NOT BECOME IT.
//! `Drive` is the player's physics: throttle, brake, steering authority,
//! the centrifugal push you fight through a bend. A traffic car has no
//! inputs at all — it has a target speed and a lane it holds. Driving it
//! through `Drive` would mean synthesising fake key presses for an AI,
//! which is how an AI ends up fighting its own controller.

use crate::drive::{Surface, Tuning};
use crate::road::Road;

/// The slowest and fastest a traffic car cruises, as a fraction of the
/// player's top speed.
///
/// ⚠️ A BAND, NOT A NUMBER, and both ends are load-bearing. Above about
/// 0.75 an overtake takes longer than the straights are long, so passing
/// becomes impossible anywhere but a corner. Below about 0.55 the
/// traffic reads as parked cars rather than as a race, and the plan's
/// points-per-car-passed scoring becomes automatic.
///
/// Varied per car rather than shared, so the field SPREADS over a lap
/// and cars close on each other into groups. A single speed makes the
/// whole lap one overtake, learned once and then repeated.
///
/// Expressed against `tuning.top_speed` rather than in world units so a
/// retune of the car carries the traffic with it (L015).
pub const CRUISE_MIN: f32 = 0.55;
pub const CRUISE_MAX: f32 = 0.75;

/// How much of the road's half-width the traffic will use.
///
/// Kept inside the rumble strip: a traffic car riding the verge would be
/// taking the surface penalty for its whole life, and it would look like
/// the AI could not hold a line. `1.0 - RUMBLE_FRACTION` is the tarmac's
/// own edge, and this sits inside that again so a car has somewhere to
/// be without touching the strip.
pub const MAX_LANE: f32 = 0.72;

/// How quickly a traffic car settles onto its lane, in half-widths per
/// second.
///
/// Deliberately slow. Pole Position's traffic drifted lazily across the
/// road rather than snapping to a rail, and that laziness is most of what
/// makes it read as traffic rather than as scenery on a track. It is also
/// what makes an overtake a judgement: the gap you aim at is still moving
/// when you get there.
pub const LANE_DRIFT_RATE: f32 = 0.18;


/// How far behind the player a car must fall before it is recycled
/// ahead, as a fraction of **the lap**.
///
/// ⚠️ A FRACTION OF THE LAP, NOT OF THE VISIBLE ROAD, and the difference
/// is not cosmetic. THE VISIBLE ROAD IS 1.8% OF A LAP on the shipped
/// course — 24,000 units against 1,309,800. Expressed in visible roads
/// this was 1.2, which reads like a comfortable margin and is actually
/// 2.2% of a lap: a car was recycled almost the instant it was passed.
/// The probe showed it immediately — 112 overtakes over three laps
/// against five cars, every car passed about seven times a lap, and the
/// whole field collapsed onto the recycle distance instead of spreading.
///
/// This is the scale trap the course notes already warn about in another
/// costume: miles are the right unit for authoring a course and the
/// wrong one for authoring what stands beside it. Recycling is a PACING
/// question, so it belongs in lap fractions; the respawn distance below
/// is a VISIBILITY question, so it belongs in visible roads. Two
/// different questions, two different units, and using one unit for both
/// is what produced the bug.
///
/// A third of a lap means a car passed at the start line reappears
/// somewhere after the second corner — long enough that it reads as new
/// traffic rather than as the same car coming round again.
pub const RECYCLE_BEHIND_LAPS: f32 = 0.33;

/// Where a recycled car reappears, as a fraction of the visible road
/// ahead of the player.
///
/// ⚠️ DERIVED, NOT PICKED, and the derivation is a safety argument.
/// A recycled car must never appear close enough to be unavoidable —
/// that is the one way this mechanism can turn a fair game unfair, and
/// it lands right before collision does.
///
/// The worst case is closing on the SLOWEST traffic: at a cruise floor
/// of 0.55 the closing speed is 0.45 of top speed, 7200 u/s on the
/// shipped tuning. The game already has a definition of "enough time to
/// react" — `REACTION_SECONDS`, 1.5, the same constant
/// `Tuning::from_corner` derives top speed from — and 1.5s at that
/// closing speed is 10800 units, or 0.45 of the 24000-unit visible road.
///
/// Respawning at the far edge of the visible road (1.0) is further again
/// and has a property the bare safety margin does not: the car is
/// ALWAYS SEEN ARRIVING. It fades in at the horizon like every other car
/// rather than appearing in the middle distance, so there is nothing to
/// notice. A shorter distance would be safe and would still look like a
/// pop-in.
pub const RECYCLE_AHEAD: f32 = 1.0;

/// One traffic car.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Car {
    /// Position along the track, world units. Wraps with the road.
    pub z: f32,
    /// Position across the road in half-widths, matching `Drive::x`.
    pub x: f32,
    /// Current speed, world units per second.
    pub speed: f32,
    /// Which livery to draw it in.
    pub livery: usize,
    /// This car's own cruise speed as a fraction of the player's top
    /// speed. Fixed for the car's life — it is the car's character.
    cruise: f32,
    /// The lane it is heading for, in half-widths.
    lane: f32,
    /// How many times this car has been recycled ahead. Drives the
    /// fresh lane and cruise speed it comes back with.
    recycled: u32,
    /// How far the player has travelled since overtaking this car, in
    /// world units. `None` until the car has actually been passed.
    ///
    /// ⚠️ THIS IS WHY RECYCLING NEEDS STATE AND NOT A POSITION TEST.
    /// On a closed loop "behind me" and "ahead of me" are THE SAME SET
    /// of positions — a car 0.99 of a lap behind is 0.01 of a lap in
    /// front, and both descriptions are true at once. No comparison of
    /// two `z` values can tell them apart.
    ///
    /// Two attempts proved it. A signed gap unwrapped onto
    /// (-length/2, length/2] gave a recycle window only 0.33..0.50 of a
    /// lap wide — everything past halfway read as "ahead" and was never
    /// recycled, which Brian found by driving: pass the field, then see
    /// nothing for a lap and a half. Switching to distance-ahead closed
    /// that hole and opened the opposite one, recycling cars that were
    /// genuinely in front and about to be reached: zero overtakes.
    ///
    /// Being overtaken is an EVENT, so it is recorded when it happens.
    since_passed: Option<f32>,
}

impl Car {
    /// The fastest this car will go where it currently is.
    ///
    /// ⚠️ THE CORNER SPEED IS SOLVED, NOT PICKED. Steering moves the car
    /// by `steer_rate * authority`; a bend pushes it out by
    /// `curve * authority² * centrifugal`. A car holds its line at full
    /// lock exactly when those balance:
    ///
    /// ```text
    ///     steer_rate * a = curve * a² * centrifugal
    ///                  a = steer_rate / (curve * centrifugal)
    /// ```
    ///
    /// and since `authority = speed / top_speed`, the fastest a bend of
    /// curve `c` can be held is `top_speed * steer_rate / (c *
    /// centrifugal)`. That is the same arithmetic the player is subject
    /// to, read from the same two tuning numbers — so traffic corners on
    /// the physics you corner on, rather than on a second set that would
    /// drift away from it the moment either is retuned (L015).
    ///
    /// A margin is taken off because full lock is the theoretical edge:
    /// a car cornering at exactly the limit is holding the wheel hard
    /// over with nothing left, and any curve at all in the next segment
    /// puts it on the grass.
    pub fn target_speed(&self, road: &Road, tuning: &Tuning) -> f32 {
        let cruise = tuning.top_speed * self.cruise;

        let curve = road.curve_at(self.z).abs();
        if curve <= f32::EPSILON || tuning.centrifugal <= 0.0 {
            return cruise;
        }

        // The full-lock limit, then backed off so the car is not riding
        // the edge of its own grip through every bend.
        const CORNER_MARGIN: f32 = 0.85;
        let limit = tuning.top_speed * tuning.steer_rate / (curve * tuning.centrifugal);

        cruise.min(limit * CORNER_MARGIN)
    }

    /// What this car is driving on, from its lateral position.
    ///
    /// Reuses the player's own surface rule so "where the rumble strip
    /// is" has exactly one answer in the game.
    pub fn surface(&self) -> Surface {
        Surface::at(self.x)
    }
}

/// The whole traffic field.
///
/// A struct rather than a bare `Vec` so the no-player rule has somewhere
/// to live and something to test.
#[derive(Debug, Clone, Default)]
pub struct Field {
    pub cars: Vec<Car>,
    /// Where the player was on the previous `recycle` call, so the
    /// distance they have travelled can be accumulated. `None` before
    /// the first call.
    last_player_z: Option<f32>,
}

impl Field {
    /// Build a starting field spread down the road ahead.
    ///
    /// Spacing is a FRACTION OF THE VISIBLE DEPTH, not a segment count:
    /// at a draw distance of 120, a car nine segments ahead is 7% of the
    /// way to the horizon and renders three pixels tall. This is the same
    /// reasoning the static placement used, kept because it was right.
    ///
    /// Speeds and lanes are spread deterministically across the field
    /// rather than randomly. The same lap must present the same traffic
    /// twice — a player learning a course is learning where the cars are,
    /// and a random field makes that learning worthless. It also means a
    /// probe measures the real game and not one sample of it.
    pub fn grid(road: &Road, n: usize) -> Field {
        let visible = road.draw_distance() as f32 * road.segment_length();

        let cars = (0..n)
            .map(|i| {
                let t = if n > 1 {
                    i as f32 / (n - 1) as f32
                } else {
                    0.0
                };

                // Spread down the road, widening as it goes: the far
                // cars matter less, and bunching the near ones gives the
                // player something to do immediately.
                let z = road.wrap(visible * (0.25 + 2.35 * t * t));

                // Alternate sides, at varying distance from the centre,
                // so the field does not read as a single file.
                let side = if i % 2 == 0 { -1.0 } else { 1.0 };
                let lane = side * MAX_LANE * (0.35 + 0.65 * ((i % 3) as f32 / 2.0));

                // Walk the cruise band across the field. Not random:
                // repeatable traffic is what makes a course learnable.
                let cruise = CRUISE_MIN + (CRUISE_MAX - CRUISE_MIN) * t;

                Car {
                    z,
                    x: lane,
                    speed: 0.0,
                    livery: i,
                    cruise,
                    lane,
                    recycled: 0,
                    since_passed: None,
                }
            })
            .collect();

        Field {
            cars,
            last_player_z: None,
        }
    }

    /// Drive every car one step.
    ///
    /// ⚠️ TAKES NO PLAYER, AND THAT IS THE POINT. There is deliberately
    /// nothing in scope to react to. `traffic_is_blind` guards this by
    /// running the same field twice — once alone, once with a player
    /// driving through it — and asserting the cars end up in identical
    /// places. If a future change threads the player in here, that test
    /// is where the argument has to be had.
    pub fn advance(&mut self, dt: f32, road: &Road, tuning: &Tuning) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }

        for car in &mut self.cars {
            // Approach the target rather than snapping to it, so a car
            // entering a corner slows into it and accelerates out. The
            // rates are the player's own: traffic that could brake harder
            // than you can would be a different vehicle.
            let target = car.target_speed(road, tuning) * car.surface().speed_cap();

            if car.speed < target {
                let accel = tuning.top_speed / tuning.accel_time.max(f32::EPSILON);
                car.speed = (car.speed + accel * dt).min(target);
            } else {
                let decel = tuning.top_speed / tuning.brake_time.max(f32::EPSILON);
                car.speed = (car.speed - decel * dt).max(target);
            }

            car.z = road.wrap(car.z + car.speed * dt);

            // Drift onto the lane rather than tracking it exactly. The
            // gap you aim at is still moving when you get there, which is
            // what makes an overtake a judgement rather than a formality.
            let gap = car.lane - car.x;
            let step = LANE_DRIFT_RATE * dt;
            car.x += gap.clamp(-step, step);
        }
    }

    /// Move cars that have fallen far behind back out in front.
    ///
    /// ⚠️ THIS IS THE ONE PLACE THE PLAYER'S POSITION IS ALLOWED IN, and
    /// it is deliberately NOT part of [`Field::advance`]. Recycling is a
    /// supply question — "is there anything left to overtake" — not a
    /// driving one, and keeping it in its own call means `advance` stays
    /// provably blind and its guard test stays meaningful.
    ///
    /// Five cars on a 2.7-mile loop cannot produce continuous traffic:
    /// measured over three laps, the field ended up spread from 0.11 to
    /// 0.95 of a lap apart, and 5 of 7 overtakes happened in the first
    /// twenty seconds. You clear the grid and then drive alone. Pole
    /// Position's traffic was an obstacle STREAM rather than a simulated
    /// field, and this is that: the same five cars, recycled.
    ///
    /// A recycled car gets a fresh lane and cruise speed so the stream
    /// does not become the same five cars in the same order forever.
    pub fn recycle(&mut self, player_z: f32, road: &Road) {
        let visible = road.draw_distance() as f32 * road.segment_length();
        let length = road.length();

        // How far the player moved since the last call, unwrapped. This
        // is what "distance since the pass" accumulates.
        let step = match self.last_player_z {
            Some(prev) => {
                let d = (player_z - prev).rem_euclid(length);
                // A backwards or teleporting player (a reset, a lap
                // rollover on a stalled frame) contributes nothing
                // rather than a spurious near-full-lap step.
                if d > length / 2.0 {
                    0.0
                } else {
                    d
                }
            }
            None => 0.0,
        };
        self.last_player_z = Some(player_z);

        for (i, car) in self.cars.iter_mut().enumerate() {
            // Distance from the player forward to this car.
            let ahead = (car.z - player_z).rem_euclid(length);

            match car.since_passed {
                None => {
                    // ⚠️ A PASS IS A SIGN CHANGE, DETECTED PER FRAME, and
                    // it must be measured over the SMALL step the player
                    // actually moved — not inferred from a position.
                    //
                    // The car was in front last frame and is behind now.
                    // In "distance ahead" terms that is a value which was
                    // small and has wrapped to near a full lap, and the
                    // only honest tolerance is how far the player moved
                    // this frame. Anything looser fires on cars that were
                    // never passed; anything based on a fixed fraction of
                    // the lap fires every time round.
                    let was_ahead = (car.z - (player_z - step)).rem_euclid(length);
                    if was_ahead <= step && ahead > length / 2.0 {
                        car.since_passed = Some(0.0);
                    }
                }
                Some(travelled) => {
                    let travelled = travelled + step;
                    if travelled < length * RECYCLE_BEHIND_LAPS {
                        car.since_passed = Some(travelled);
                        continue;
                    }

                    car.since_passed = None;
                    car.recycled = car.recycled.wrapping_add(1);

                    // ⚠️ VARY BY THE CAR AS WELL AS BY THE COUNT, or the
                    // field COLLAPSES INTO ONE CLUMP. Cars are passed at
                    // similar times, so they are recycled a similar
                    // number of times; keying the fresh character off
                    // that count alone gave every car the same cruise
                    // (measured: all five at 62%) and every car the same
                    // reappearance point, so they came back as a single
                    // block that arrives together and is passed together.
                    // Seeding with the car's own index breaks the tie.
                    let n = car.recycled as f32 + i as f32 * 0.37;
                    let t = ((n * 0.618_034) % 1.0).abs();

                    // Stagger where they reappear, too. Landing every car
                    // on exactly the horizon stacks them nose to tail
                    // even when their speeds differ.
                    let spread = 1.0 + (i as f32 * 0.23 + t * 0.4);
                    car.z = road.wrap(player_z + visible * RECYCLE_AHEAD * spread);

                    car.cruise = CRUISE_MIN + (CRUISE_MAX - CRUISE_MIN) * t;
                    let side = if (car.recycled as usize + i) % 2 == 0 {
                        -1.0
                    } else {
                        1.0
                    };
                    car.lane = side * MAX_LANE * (0.35 + 0.65 * t);
                }
            }
        }
    }

    /// The field as the renderer wants it: `(z, lane, livery)`.
    ///
    /// Keeps the drawing code unaware that traffic gained a speed and a
    /// target lane — it draws cars at positions, exactly as before.
    pub fn as_rendered(&self) -> Vec<(f32, f32, usize)> {
        self.cars.iter().map(|c| (c.z, c.x, c.livery)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drive::Drive;
    use crate::track;

    fn course() -> (Road, Tuning) {
        let road = track::grand_prix().build();
        let tuning = Tuning::from_corner(&road, 1.5);
        (road, tuning)
    }

    /// The rule the whole design rests on, guarded mechanically.
    ///
    /// Runs the same field twice for the same duration — once with
    /// nothing else on track, once with a player driving flat out through
    /// it — and asserts every car ends in an identical place. This is
    /// what stops "just a small nudge to avoid the player" being added
    /// later without the decision (4a0707a3) being re-opened.
    #[test]
    fn traffic_is_blind() {
        let (road, tuning) = course();
        let dt = 1.0 / 60.0;

        let mut alone = Field::grid(&road, 5);
        let mut watched = alone.clone();
        let mut player = Drive::new();

        for _ in 0..(20.0 / dt) as usize {
            alone.advance(dt, &road, &tuning);

            let correction = (-player.x * 3.0).clamp(-1.0, 1.0);
            player.update(dt, 1.0, 0.0, correction, &road, &tuning);
            watched.advance(dt, &road, &tuning);
        }

        for (a, b) in alone.cars.iter().zip(watched.cars.iter()) {
            assert_eq!(
                (a.z, a.x, a.speed),
                (b.z, b.x, b.speed),
                "a traffic car moved differently with a player on track"
            );
        }
    }

    #[test]
    fn traffic_actually_drives() {
        // The bug this replaces: cars sat where they were put. A field
        // that does not move is the thing this module exists to end.
        let (road, tuning) = course();
        let mut field = Field::grid(&road, 5);
        let start: Vec<f32> = field.cars.iter().map(|c| c.z).collect();

        for _ in 0..600 {
            field.advance(1.0 / 60.0, &road, &tuning);
        }

        for (car, z0) in field.cars.iter().zip(start.iter()) {
            assert!(car.speed > 0.0, "a car never moved off");
            assert!(
                (car.z - z0).abs() > road.segment_length(),
                "a car covered less than one segment in ten seconds"
            );
        }
    }

    #[test]
    fn traffic_is_slower_than_the_player() {
        // If traffic can run with the player it cannot be overtaken, and
        // the plan scores 50 points per car passed.
        let (road, tuning) = course();
        let mut field = Field::grid(&road, 5);
        for _ in 0..900 {
            field.advance(1.0 / 60.0, &road, &tuning);
        }
        for car in &field.cars {
            assert!(
                car.speed < tuning.top_speed * CRUISE_MAX + 1.0,
                "a car at {} is running past the cruise band on a top speed of {}",
                car.speed,
                tuning.top_speed
            );
        }
    }

    /// Varied speeds exist so the field does not hold formation.
    ///
    /// ⚠️ THE FIRST VERSION OF THIS TEST ASSERTED `after != before` AND
    /// PASSED AGAINST UNIFORM SPEEDS — the exact bug it existed to catch
    /// (L024). Float inequality is not a measurement: the cars are on a
    /// curved course at different positions, so they corner at different
    /// moments and the gaps wobble by a hair even when every car runs at
    /// an identical cruise. Any non-zero difference satisfied it.
    ///
    /// MEASURED instead. Over one simulated minute on the shipped
    /// course, the standard deviation of the gaps between cars moves to
    /// 0.68x its starting value with varied cruise speeds, and to
    /// 1.000x with uniform ones. The threshold sits between two numbers
    /// that were measured, not guessed at.
    ///
    /// It CONVERGES rather than diverging because the starting grid is
    /// spaced quadratically and the faster far cars close those wide
    /// gaps. The direction is incidental; what the guard cares about is
    /// that uniform speeds hold formation EXACTLY and varied ones do not.
    #[test]
    fn the_field_spreads() {
        let (road, tuning) = course();
        let mut field = Field::grid(&road, 5);

        let spread_of = |f: &Field| -> f32 {
            let mut zs: Vec<f32> = f.cars.iter().map(|c| c.z).collect();
            zs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let gaps: Vec<f32> = zs.windows(2).map(|w| w[1] - w[0]).collect();
            let mean = gaps.iter().sum::<f32>() / gaps.len() as f32;
            (gaps.iter().map(|g| (g - mean).powi(2)).sum::<f32>() / gaps.len() as f32).sqrt()
        };

        let before = spread_of(&field);
        for _ in 0..(60.0 * 60.0) as usize {
            field.advance(1.0 / 60.0, &road, &tuning);
        }
        let after = spread_of(&field);
        let ratio = after / before.max(1.0);

        assert!(
            (ratio - 1.0).abs() > 0.10,
            "the field held formation (gap spread moved {ratio:.3}x over a \
             minute) — the cruise band is not varying across the cars"
        );
    }

    #[test]
    fn no_car_leaves_the_road() {
        // A traffic car on the grass is taking a permanent speed penalty
        // and looks like an AI that cannot hold a line.
        let (road, tuning) = course();
        let mut field = Field::grid(&road, 6);
        for _ in 0..(120.0 * 60.0) as usize {
            field.advance(1.0 / 60.0, &road, &tuning);
            for car in &field.cars {
                assert!(
                    car.x.abs() <= MAX_LANE + 1e-3,
                    "a car reached lane {}, outside the traffic's own limit",
                    car.x
                );
            }
        }
    }

    #[test]
    fn cars_slow_for_corners() {
        // The derived corner speed, exercised. A car that takes the
        // course's hardest bend at its cruise speed is cornering on
        // different physics than the player, who must brake for it.
        let (road, tuning) = course();
        let field = Field::grid(&road, 5);
        let car = field.cars[0];

        // Find the sharpest point on the shipped course.
        let steps = 400;
        let hardest = (0..steps)
            .map(|i| road.length() * i as f32 / steps as f32)
            .max_by(|a, b| {
                road.curve_at(*a)
                    .abs()
                    .partial_cmp(&road.curve_at(*b).abs())
                    .unwrap()
            })
            .unwrap();
        assert!(road.curve_at(hardest).abs() > 0.0, "course has no bends");

        let straight = Car { z: 0.0, ..car };
        let cornering = Car { z: hardest, ..car };

        // Pick a z on the course that is genuinely straight to compare
        // against, or the assertion is vacuous.
        let flat = (0..steps)
            .map(|i| road.length() * i as f32 / steps as f32)
            .min_by(|a, b| {
                road.curve_at(*a)
                    .abs()
                    .partial_cmp(&road.curve_at(*b).abs())
                    .unwrap()
            })
            .unwrap();
        let on_straight = Car { z: flat, ..straight };

        assert!(
            cornering.target_speed(&road, &tuning) < on_straight.target_speed(&road, &tuning),
            "a car does not slow for the course's hardest bend"
        );
    }



    /// THE SAFETY GUARD ON RECYCLING.
    ///
    /// A car moved back out in front must always appear far enough away
    /// that the player can see it and react. This is the one way the
    /// recycling mechanism can make the game unfair, and it lands
    /// immediately before collision does — so it is checked against the
    /// worst case: closing on the SLOWEST traffic at full speed, with
    /// the game's own `REACTION_SECONDS` as the bar.
    #[test]
    fn a_recycled_car_never_appears_inside_reaction_distance() {
        const REACTION_SECONDS: f32 = 1.5;

        let (road, tuning) = course();
        let mut field = Field::grid(&road, 5);

        // Worst case closing speed: player flat out, traffic at the floor.
        let closing = tuning.top_speed * (1.0 - CRUISE_MIN);
        let needed = closing * REACTION_SECONDS;

        // Put every car well past the recycle threshold, then recycle.
        let player_z = road.wrap(road.length() * 0.5);
        for car in &mut field.cars {
            car.z = road.wrap(player_z - road.length() * (RECYCLE_BEHIND_LAPS + 0.05));
        }
        field.recycle(player_z, &road);

        for car in &field.cars {
            let mut gap = car.z - player_z;
            while gap < 0.0 {
                gap += road.length();
            }
            assert!(
                gap >= needed,
                "a car reappeared {gap:.0} units ahead; {needed:.0} is the \
                 distance {REACTION_SECONDS}s of reaction needs at a closing \
                 speed of {closing:.0} u/s"
            );
        }
    }

    /// Recycling must not fire on a car the player has merely passed.
    ///
    /// ⚠️ THE FIRST VERSION OF THIS TEST STOPPED GUARDING ANYTHING when
    /// recycling moved from a position check to a pass event: it placed
    /// a car behind the player by fiat, and a car that was never
    /// overtaken is never a recycle candidate, so it passed for the
    /// wrong reason. Mutation testing found it — removing the 0.33-lap
    /// wait entirely left the whole suite green.
    ///
    /// This drives a real pass and then asserts the car is NOT snatched
    /// back immediately: the player must cover the design's distance
    /// first, or traffic pops back in front the instant it is overtaken.
    #[test]
    fn a_freshly_passed_car_is_left_alone() {
        let (road, tuning) = course();
        let length = road.length();
        let mut field = Field::grid(&road, 1);

        let mut player = Drive::new();
        player.z = road.wrap(length * 0.25);
        player.speed = tuning.top_speed;
        field.cars[0].z = road.wrap(player.z + road.segment_length());
        field.cars[0].speed = tuning.top_speed * CRUISE_MIN;

        let dt = 1.0 / 60.0;
        let start_player_z = player.z;

        // Drive only a tenth of a lap past the pass — well short of the
        // 0.33 the design promises.
        while (player.z - start_player_z).rem_euclid(length) < length * 0.10 {
            player.z = road.wrap(player.z + player.speed * dt);
            field.advance(dt, &road, &tuning);
            field.recycle(player.z, &road);

            assert_eq!(
                field.cars[0].recycled, 0,
                "a car was recycled after only {:.3} of a lap, short of the \
                 {RECYCLE_BEHIND_LAPS} the design promises — traffic will pop \
                 back in front the moment it is passed",
                (player.z - start_player_z).rem_euclid(length) / length,
            );
        }
    }

    /// `advance` must stay blind even though `recycle` is not.
    ///
    /// The two are separate calls precisely so this stays provable. If a
    /// future change folds recycling into `advance`, `traffic_is_blind`
    /// starts failing, and that is the intended alarm.
    #[test]
    fn recycling_is_not_part_of_advancing() {
        let (road, tuning) = course();
        let visible = road.draw_distance() as f32 * road.segment_length();
        let mut field = Field::grid(&road, 3);

        let player_z = road.wrap(road.length() * 0.5);
        for car in &mut field.cars {
            car.z = road.wrap(player_z - road.length() * (RECYCLE_BEHIND_LAPS + 0.05));
        }
        let before: Vec<f32> = field.cars.iter().map(|c| c.z).collect();

        // Advancing alone must not move anything back out in front,
        // however far behind the cars are.
        for _ in 0..10 {
            field.advance(1.0 / 60.0, &road, &tuning);
        }
        for (car, z0) in field.cars.iter().zip(before.iter()) {
            let moved = (car.z - z0).abs();
            assert!(
                moved < visible,
                "advance() jumped a car {moved:.0} units — recycling leaked into it"
            );
        }
    }

    /// Sweep the whole loop: no position anywhere on the course may
    /// produce a recycle that lands a car unavoidably close.
    ///
    /// ⚠️ THE SINGLE-POSITION TEST WAS NOT ENOUGH. This mechanism is a
    /// wrap-around calculation, and wrap-around bugs live at the
    /// boundaries — Brian drove the first version and found cars that
    /// were never recycled at all, because the old signed-gap check had
    /// a window only 0.33..0.50 of a lap wide and anything past halfway
    /// round read as "ahead". A test that checks one placement cannot
    /// see a hole in the OTHER 83% of the loop. Sweep it.
    #[test]
    fn no_position_on_the_loop_recycles_a_car_into_the_players_lap() {
        const REACTION_SECONDS: f32 = 1.5;

        let (road, tuning) = course();
        let length = road.length();
        let closing = tuning.top_speed * (1.0 - CRUISE_MIN);
        let needed = closing * REACTION_SECONDS;

        // Every hundredth of the loop, at every hundredth of an offset.
        for p in 0..100 {
            let player_z = road.wrap(length * p as f32 / 100.0);
            for c in 0..100 {
                let mut field = Field::grid(&road, 1);
                field.cars[0].z = road.wrap(player_z + length * c as f32 / 100.0);
                let before = field.cars[0].z;

                field.recycle(player_z, &road);
                let after = field.cars[0].z;

                if (after - before).abs() < 1.0 {
                    continue; // not recycled
                }

                let ahead = (after - player_z).rem_euclid(length);
                assert!(
                    ahead >= needed,
                    "player at {p}% of the loop, car at +{c}%: recycled to \
                     {ahead:.0} units ahead, inside the {needed:.0} that \
                     {REACTION_SECONDS}s of reaction needs"
                );
            }
        }
    }

    /// Every car must eventually come back.
    ///
    /// ⚠️ THE PASS IS DRIVEN, NOT TELEPORTED. Recycling is event-based:
    /// a car becomes a candidate only once the player has actually
    /// overtaken it, so a fixture that drops a car behind by fiat has
    /// never been passed and correctly is not recycled. The earlier
    /// version of this test did exactly that and had to be rewritten
    /// when the design moved from a position test to a pass event —
    /// which is the honest signal that the two are different mechanisms,
    /// not the same one spelled differently.
    ///
    /// This drives a player past a car and then keeps driving, and
    /// asserts the car comes back out in front within the distance the
    /// design promises.
    #[test]
    fn a_car_that_falls_behind_is_always_eventually_recycled() {
        let (road, tuning) = course();
        let length = road.length();
        let mut field = Field::grid(&road, 1);

        // Put the car just in front of a player who is about to pass it.
        let mut player = Drive::new();
        player.z = road.wrap(length * 0.25);
        player.speed = tuning.top_speed;
        field.cars[0].z = road.wrap(player.z + road.segment_length());
        field.cars[0].speed = tuning.top_speed * CRUISE_MIN;

        let start_z = field.cars[0].z;
        let dt = 1.0 / 60.0;

        // Drive well past the recycle threshold.
        let mut recycled = false;
        for _ in 0..(120.0 / dt) as usize {
            player.z = road.wrap(player.z + player.speed * dt);
            field.advance(dt, &road, &tuning);
            field.recycle(player.z, &road);

            // A recycle is a jump: the car moves much further in one
            // step than it could have driven.
            if (field.cars[0].z - start_z).abs() > road.segment_length()
                && field.cars[0].recycled > 0
            {
                recycled = true;
                break;
            }
        }

        assert!(
            recycled,
            "a car the player drove past was never recycled — it will only \
             be seen again by lapping it, which is the bug Brian found by \
             driving"
        );
    }

    #[test]
    fn a_stalled_frame_cannot_move_the_field() {
        let (road, tuning) = course();
        let mut field = Field::grid(&road, 3);
        field.advance(0.1, &road, &tuning);
        let before: Vec<Car> = field.cars.clone();

        field.advance(-1.0, &road, &tuning);
        field.advance(f32::NAN, &road, &tuning);

        assert_eq!(before, field.cars, "a bad dt moved the traffic");
    }
}
