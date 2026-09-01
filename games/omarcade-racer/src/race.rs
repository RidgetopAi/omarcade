//! The shape of a race: qualifying, the grid, the clock, the laps.
//!
//! This is the Pole Position structure, deliberately. One qualifying lap
//! against the clock; make the cut and your time decides where you start;
//! then a race of [`RACE_LAPS`] laps in which the clock, not the traffic,
//! is what ends you. Crashing costs time and nothing else — `crash` owns
//! the fireball and never touches this module — and that division is the
//! whole design: a crash is survivable, an empty clock is not.
//!
//! Every time limit here is DERIVED. The reference driver in `pace` is
//! driven over the course at start-up and the limits are its times plus an
//! allowance. Change the course or the car and the limits follow; there is
//! no number in this file that was picked by looking at a lap and thinking
//! "about ninety seconds". The allowances themselves are the one thing a
//! person chooses, and they are ratios, so they survive a retune (L019).
//!
//! Progress is counted by DISTANCE, never by `z` wrapping. `z` wraps once
//! a lap wherever the road decides, the grid sits behind the line, and a
//! crash rewinds the car; a signed step in `z` handles all three, where a
//! wrap counter handles none of them.

use crate::drive::{Drive, Tuning};
use crate::pace::{self, Pacer};
use crate::road::Road;

/// Laps in the race proper. The plan's number.
pub const RACE_LAPS: u32 = 3;

/// How many times a lap the clock is topped up. The start line is one of
/// them; the rest divide the lap evenly by distance.
///
/// Two puts a checkpoint on the back straight before the Hard right, so
/// the clock is a live question twice a lap rather than once — and so
/// a lap's worth of banked time cannot be spent on one corner.
pub const CHECKPOINTS_PER_LAP: usize = 2;

/// How much slower than the reference driver a lap may be and still
/// qualify. The reference driver is the physics-exact one; a person
/// steering with two keys and reading corners for the first time is not.
///
/// ⚠️ CHOSEN, and only driving settles it (L023). It is a ratio over a
/// measured lap so the course or the car can be retuned without this
/// silently turning into an impossible cut or a free pass. At 1.25 the
/// cut on the grand prix sits near 112s against the reference's ~90s
/// from a standing start.
pub const QUAL_ALLOWANCE: f32 = 1.25;

/// The same allowance, per checkpoint window, during the race. Stated
/// separately from [`QUAL_ALLOWANCE`] because the two are different
/// questions — "are you good enough to start?" and "are you keeping
/// pace?" — even though today they have the same answer.
pub const CHECKPOINT_ALLOWANCE: f32 = 1.25;

/// Seconds from "ready" to the green light.
pub const COUNTDOWN_SECONDS: f32 = 3.0;

/// The derived time limits for a course and a car.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Windows {
    /// The reference driver's flying lap, in seconds.
    pub reference_lap: f32,
    /// The reference driver's lap from a standing start on the grid to
    /// the line after one full lap. Slower than the flying lap by the
    /// run-up to speed.
    pub reference_standing: f32,
    /// Seconds allowed for the qualifying lap, from the green light.
    pub qualify: f32,
    /// Seconds granted per checkpoint during the race.
    pub checkpoint: f32,
}

impl Windows {
    /// Derive the limits by driving the course with [`Pacer::EXACT`].
    ///
    /// `grid_z` is where the car sits at the green light — behind the
    /// line, so the standing lap is the run to the line plus a lap.
    pub fn derive(road: &Road, tuning: &Tuning, grid_z: f32) -> Windows {
        let flying = pace::lap(Pacer::EXACT, road, tuning);
        let start = Drive { z: grid_z, ..Drive::new() };
        let to_line = line_from(grid_z, road);
        let standing = pace::drive_distance(
            Pacer::EXACT,
            road,
            tuning,
            start,
            to_line + road.length(),
        );
        Windows {
            reference_lap: flying.time,
            reference_standing: standing.time,
            qualify: standing.time * QUAL_ALLOWANCE,
            checkpoint: flying.time / CHECKPOINTS_PER_LAP as f32 * CHECKPOINT_ALLOWANCE,
        }
    }
}

/// Distance from `z` forward to the start line at `z = 0`.
fn line_from(z: f32, road: &Road) -> f32 {
    let z = road.wrap(z);
    if z == 0.0 {
        0.0
    } else {
        road.length() - z
    }
}

/// Why a run ended without a finish.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Out {
    /// The qualifying lap took longer than [`Windows::qualify`].
    DidNotQualify,
    /// The clock reached zero during the race.
    OutOfTime,
}

/// What the countdown leads into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Session {
    Qualifying,
    Race,
}

/// Where the run is. One value, so "may the player drive" and "is it
/// over" are the same question asked of the same thing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// Lights. The car is held; the clock has not started.
    Countdown { remaining: f32, then: Session },
    /// One lap against [`Windows::qualify`].
    Qualifying,
    /// Made the cut. Holding here until the race is started.
    Qualified { time: f32, position: usize },
    /// The race proper. `lap` is the lap being driven, from 1.
    Racing { lap: u32 },
    /// Every lap done. `time` is the race time.
    Finished { time: f32 },
    Over(Out),
}

/// Something that happened this frame that the caller may want to react
/// to — a HUD flash, a sound, points in S12.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    GreenLight,
    /// Made the cut, with the lap time and the grid position it earned.
    Qualified { time: f32, position: usize },
    /// A checkpoint crossed with `remaining` seconds still on the clock
    /// before the top-up. The number the original paid points for.
    Checkpoint { remaining: f32 },
    /// A lap of the race completed. `lap` is the lap just finished and
    /// `remaining` what was on the clock at the line — the line is a
    /// checkpoint too, and S12 pays for it the same way.
    LapDone { lap: u32, remaining: f32 },
    /// Every lap done, in `time` seconds, with `remaining` on the clock.
    Finished { time: f32, remaining: f32 },
    Over(Out),
}

/// The run: which phase, how much clock, how far along.
#[derive(Clone, Debug)]
pub struct Race {
    pub phase: Phase,
    /// Seconds left before the run ends, while the clock is running.
    pub clock: f32,
    /// Seconds since the green light of the current session.
    pub elapsed: f32,
    /// Laps of the race completed.
    pub laps_done: u32,
    pub windows: Windows,
    /// Grid positions available, so qualifying can be bracketed into
    /// them. The player's slot plus the traffic's.
    grid_size: usize,
    length: f32,
    /// Distance travelled since the green light, by signed `z` steps.
    travelled: f32,
    last_z: f32,
    /// `travelled` at which the next mark — a checkpoint or the line —
    /// is crossed.
    next_mark: f32,
    /// Distance between marks during the race.
    mark_spacing: f32,
    /// Distance from the grid slot to the start line.
    to_line: f32,
}

impl Race {
    /// A run about to qualify, on the grid at `grid_z`, lights on.
    pub fn new(windows: Windows, road: &Road, grid_z: f32, grid_size: usize) -> Race {
        Race {
            phase: Phase::Countdown { remaining: COUNTDOWN_SECONDS, then: Session::Qualifying },
            clock: windows.qualify,
            elapsed: 0.0,
            laps_done: 0,
            windows,
            grid_size: grid_size.max(1),
            length: road.length(),
            travelled: 0.0,
            last_z: road.wrap(grid_z),
            next_mark: f32::INFINITY,
            mark_spacing: road.length() / CHECKPOINTS_PER_LAP as f32,
            to_line: line_from(grid_z, road),
        }
    }

    /// May the player's input reach the car?
    pub fn driving(&self) -> bool {
        matches!(self.phase, Phase::Qualifying | Phase::Racing { .. })
    }

    /// Grid slots, the player's included.
    pub fn grid_size(&self) -> usize {
        self.grid_size
    }

    /// Is the run finished, one way or the other?
    pub fn is_over(&self) -> bool {
        matches!(self.phase, Phase::Finished { .. } | Phase::Over(_))
    }

    /// The grid position a qualifying time earns, from 1.
    ///
    /// Bracketed linearly between the reference standing lap (pole) and
    /// the cut (last). Faster than the reference is still pole: the
    /// reference driver is exact, not optimal, and a wider line beats it.
    pub fn grid_position(&self, time: f32) -> usize {
        let pole = self.windows.reference_standing;
        let cut = self.windows.qualify;
        if time <= pole || cut <= pole {
            return 1;
        }
        let frac = (time - pole) / (cut - pole);
        let slot = (frac * self.grid_size as f32).floor() as usize + 1;
        slot.clamp(1, self.grid_size)
    }

    /// Put the car back on the grid for the race. Called once the caller
    /// has moved the car and the traffic; `grid_z` is where the car is.
    pub fn start_race(&mut self, grid_z: f32, road: &Road) {
        self.phase = Phase::Countdown { remaining: COUNTDOWN_SECONDS, then: Session::Race };
        self.clock = self.windows.checkpoint;
        self.elapsed = 0.0;
        self.laps_done = 0;
        self.travelled = 0.0;
        self.last_z = road.wrap(grid_z);
        self.to_line = line_from(grid_z, road);
        self.next_mark = f32::INFINITY;
    }

    /// Advance the run by `dt` with the car now at `car_z`.
    pub fn advance(&mut self, dt: f32, car_z: f32, road: &Road) -> Option<Event> {
        self.track_distance(car_z, road);

        match self.phase {
            Phase::Countdown { remaining, then } => {
                let remaining = remaining - dt;
                if remaining > 0.0 {
                    self.phase = Phase::Countdown { remaining, then };
                    return None;
                }
                // Green. Distance starts counting from here, and the
                // first mark is the line after one lap (qualifying) or
                // the first checkpoint (race).
                self.travelled = 0.0;
                self.elapsed = 0.0;
                match then {
                    Session::Qualifying => {
                        self.phase = Phase::Qualifying;
                        self.clock = self.windows.qualify;
                        self.next_mark = self.to_line + self.length;
                    }
                    Session::Race => {
                        self.phase = Phase::Racing { lap: 1 };
                        self.clock = self.windows.checkpoint;
                        self.next_mark = self.to_line + self.mark_spacing;
                    }
                }
                Some(Event::GreenLight)
            }

            Phase::Qualifying => {
                self.elapsed += dt;
                self.clock -= dt;
                if self.travelled >= self.next_mark {
                    let time = self.elapsed;
                    let position = self.grid_position(time);
                    self.phase = Phase::Qualified { time, position };
                    return Some(Event::Qualified { time, position });
                }
                if self.clock <= 0.0 {
                    self.clock = 0.0;
                    self.phase = Phase::Over(Out::DidNotQualify);
                    return Some(Event::Over(Out::DidNotQualify));
                }
                None
            }

            Phase::Racing { lap } => {
                self.elapsed += dt;
                self.clock -= dt;
                if self.travelled >= self.next_mark {
                    // Checkpoint. What is left on the clock carries over,
                    // as in the original: a fast driver banks time, and
                    // that bank is what S12 pays for. It is also why the
                    // window is per checkpoint rather than per lap — a
                    // lap's worth of bank would make the clock a formality.
                    let remaining = self.clock.max(0.0);
                    self.clock = remaining + self.windows.checkpoint;
                    self.next_mark += self.mark_spacing;

                    // Distance from the line, not from the grid, decides
                    // whether this mark is the line.
                    let marks_from_line =
                        ((self.next_mark - self.to_line) / self.mark_spacing).round() as usize;
                    let at_line = (marks_from_line - 1) % CHECKPOINTS_PER_LAP == 0;
                    if at_line {
                        self.laps_done += 1;
                        if self.laps_done >= RACE_LAPS {
                            let time = self.elapsed;
                            self.phase = Phase::Finished { time };
                            return Some(Event::Finished { time, remaining });
                        }
                        self.phase = Phase::Racing { lap: lap + 1 };
                        return Some(Event::LapDone { lap, remaining });
                    }
                    return Some(Event::Checkpoint { remaining });
                }
                if self.clock <= 0.0 {
                    self.clock = 0.0;
                    self.phase = Phase::Over(Out::OutOfTime);
                    return Some(Event::Over(Out::OutOfTime));
                }
                None
            }

            Phase::Qualified { .. } | Phase::Finished { .. } | Phase::Over(_) => None,
        }
    }

    /// Fold this frame's movement into `travelled`, signed.
    ///
    /// A wrap is a jump of about a whole lap backwards and is undone; a
    /// crash rewind is a small step backwards and is kept, so a rewound
    /// car has to re-drive the ground it lost.
    fn track_distance(&mut self, car_z: f32, road: &Road) {
        let z = road.wrap(car_z);
        let mut step = z - self.last_z;
        if step < -self.length / 2.0 {
            step += self.length;
        } else if step > self.length / 2.0 {
            step -= self.length;
        }
        self.travelled += step;
        self.last_z = z;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pace::DT;
    use crate::track::grand_prix;

    const GRID: usize = 6;

    fn course() -> (Road, Tuning, f32) {
        let road = grand_prix().build();
        let tuning = Tuning::from_corner(&road, 1.5);
        let visible = road.draw_distance() as f32 * road.segment_length();
        let grid_z = road.wrap(-0.1 * visible);
        (road, tuning, grid_z)
    }

    /// Drive a race with a pacer until it ends or `limit` seconds pass.
    /// Returns every event, in order.
    fn drive(race: &mut Race, car: &mut Drive, pacer: Pacer, road: &Road, tuning: &Tuning, limit: f32) -> Vec<Event> {
        let mut events = Vec::new();
        let mut t = 0.0;
        while !race.is_over() && t < limit {
            if race.driving() {
                pacer.step(car, road, tuning, DT);
            }
            if let Some(e) = race.advance(DT, car.z, road) {
                events.push(e);
            }
            t += DT;
        }
        events
    }

    /// A driver who never exceeds a fraction of top speed: the pacer with
    /// its margin scaled down does that in corners only, so this one
    /// wraps it with a ceiling on the straights too.
    fn drive_capped(race: &mut Race, car: &mut Drive, cap: f32, road: &Road, tuning: &Tuning, limit: f32) -> Vec<Event> {
        let mut events = Vec::new();
        let mut t = 0.0;
        while !race.is_over() && t < limit {
            if race.driving() {
                let inputs = Pacer::EXACT.inputs(car, road, tuning);
                let throttle = if car.speed > tuning.top_speed * cap { 0.0 } else { inputs.throttle };
                car.update(DT, throttle, inputs.brake, inputs.steer, road, tuning);
            }
            if let Some(e) = race.advance(DT, car.z, road) {
                events.push(e);
            }
            t += DT;
        }
        events
    }

    /// The limits sit where the derivation says relative to a number
    /// the derivation does not use: the flat-out floor of the course.
    #[test]
    fn the_windows_are_derived_from_a_driven_lap() {
        let (road, tuning, grid_z) = course();
        let w = Windows::derive(&road, &tuning, grid_z);
        let floor = road.length() / tuning.top_speed;
        assert!(w.reference_lap > floor && w.reference_lap < floor * 1.15, "{w:?}");
        assert!(w.reference_standing > w.reference_lap, "a standing start is not slower: {w:?}");
        assert!(
            w.reference_standing < w.reference_lap + tuning.accel_time,
            "the run-up cost more than the whole acceleration: {w:?}",
        );
        assert!(w.qualify > w.reference_standing * 1.1, "the cut leaves no room for a person: {w:?}");
        assert!(
            (w.checkpoint * CHECKPOINTS_PER_LAP as f32) > w.reference_lap,
            "a lap of checkpoint windows is less than the reference lap: {w:?}",
        );
    }

    /// The reference driver qualifies, on pole, in about its own time.
    #[test]
    fn the_reference_driver_qualifies_on_pole() {
        let (road, tuning, grid_z) = course();
        let w = Windows::derive(&road, &tuning, grid_z);
        let mut race = Race::new(w, &road, grid_z, GRID);
        let mut car = Drive { z: grid_z, ..Drive::new() };
        let events = drive(&mut race, &mut car, Pacer::EXACT, &road, &tuning, 200.0);

        assert_eq!(events.first(), Some(&Event::GreenLight));
        let Some(Event::Qualified { time, position }) = events.get(1).copied() else {
            panic!("did not qualify: {events:?}");
        };
        assert_eq!(position, 1);
        assert!(
            (time - w.reference_standing).abs() < 0.5,
            "qualified in {time} against a reference of {}",
            w.reference_standing,
        );
        assert!(matches!(race.phase, Phase::Qualified { .. }));
        assert!(!race.driving(), "the car should be held after the flag");
    }

    /// A driver too slow for the cut is put out when the clock runs
    /// out, not when the lap ends — and before the lap ends.
    #[test]
    fn too_slow_does_not_qualify() {
        let (road, tuning, grid_z) = course();
        let w = Windows::derive(&road, &tuning, grid_z);
        let mut race = Race::new(w, &road, grid_z, GRID);
        let mut car = Drive { z: grid_z, ..Drive::new() };
        // Half of top speed everywhere is roughly twice the reference lap.
        let events = drive_capped(&mut race, &mut car, 0.5, &road, &tuning, 400.0);
        assert_eq!(events.last(), Some(&Event::Over(Out::DidNotQualify)), "{events:?}");
        assert_eq!(race.phase, Phase::Over(Out::DidNotQualify));
        assert!(
            race.travelled < race.to_line + road.length(),
            "the clock should have run out BEFORE the lap was done",
        );
        assert!((race.elapsed - w.qualify).abs() < 0.01, "put out at {} against a cut of {}", race.elapsed, w.qualify);
    }

    /// Grid positions bracket the span from reference to cut.
    #[test]
    fn grid_position_brackets_the_qualifying_time() {
        let (road, tuning, grid_z) = course();
        let w = Windows::derive(&road, &tuning, grid_z);
        let race = Race::new(w, &road, grid_z, GRID);
        let (pole, cut) = (w.reference_standing, w.qualify);
        assert_eq!(race.grid_position(pole - 5.0), 1, "faster than the reference is still pole");
        assert_eq!(race.grid_position(pole), 1);
        assert_eq!(race.grid_position(pole + (cut - pole) * 0.5), GRID / 2 + 1);
        assert_eq!(race.grid_position(cut - 0.01), GRID);
        assert_eq!(race.grid_position(cut + 10.0), GRID, "clamped, not off the end");
        // Every slot is reachable.
        let mut seen = std::collections::BTreeSet::new();
        let mut t = pole;
        while t < cut {
            seen.insert(race.grid_position(t));
            t += 0.1;
        }
        assert_eq!(seen.len(), GRID, "unreachable grid slots: {seen:?}");
    }

    /// The reference driver finishes the race with the right number of
    /// checkpoints and laps, in about three of its laps.
    #[test]
    fn the_reference_driver_finishes_three_laps() {
        let (road, tuning, grid_z) = course();
        let w = Windows::derive(&road, &tuning, grid_z);
        let mut race = Race::new(w, &road, grid_z, GRID);
        race.start_race(grid_z, &road);
        let mut car = Drive { z: grid_z, ..Drive::new() };
        let events = drive(&mut race, &mut car, Pacer::EXACT, &road, &tuning, 600.0);

        let checkpoints = events.iter().filter(|e| matches!(e, Event::Checkpoint { .. })).count();
        let laps: Vec<u32> = events
            .iter()
            .filter_map(|e| if let Event::LapDone { lap, .. } = e { Some(*lap) } else { None })
            .collect();
        assert_eq!(checkpoints, (CHECKPOINTS_PER_LAP - 1) * RACE_LAPS as usize, "{events:?}");
        assert_eq!(laps, (1..RACE_LAPS).collect::<Vec<_>>(), "{events:?}");
        let Some(Event::Finished { time, .. }) = events.last().copied() else {
            panic!("did not finish: {events:?}");
        };
        let expected = w.reference_standing + w.reference_lap * (RACE_LAPS - 1) as f32;
        assert!((time - expected).abs() < 1.0, "finished in {time}, expected about {expected}");
        assert_eq!(race.laps_done, RACE_LAPS);
    }

    /// Time left at a checkpoint carries over. Without this the bank
    /// that S12 pays for does not exist and a fast lap is worth nothing.
    #[test]
    fn remaining_time_carries_over_at_a_checkpoint() {
        let (road, tuning, grid_z) = course();
        let w = Windows::derive(&road, &tuning, grid_z);
        let mut race = Race::new(w, &road, grid_z, GRID);
        race.start_race(grid_z, &road);
        let mut car = Drive { z: grid_z, ..Drive::new() };
        let mut events = Vec::new();
        let mut t = 0.0;
        while t < 300.0 {
            if race.driving() {
                Pacer::EXACT.step(&mut car, &road, &tuning, DT);
            }
            if let Some(e) = race.advance(DT, car.z, &road) {
                events.push(e);
                if let Event::Checkpoint { remaining } = e {
                    assert!(remaining > 0.0, "the reference driver arrived with nothing left");
                    assert!(
                        (race.clock - (remaining + w.checkpoint)).abs() < 0.01,
                        "clock after the checkpoint is {} — {remaining} left plus {} granted",
                        race.clock, w.checkpoint,
                    );
                    return;
                }
            }
            t += DT;
        }
        panic!("no checkpoint reached: {events:?}");
    }

    /// The clock kills at the FIRST window a slow driver misses, not at
    /// the end of the race.
    #[test]
    fn a_slow_driver_runs_out_of_time_at_the_first_window() {
        let (road, tuning, grid_z) = course();
        let w = Windows::derive(&road, &tuning, grid_z);
        let mut race = Race::new(w, &road, grid_z, GRID);
        race.start_race(grid_z, &road);
        let mut car = Drive { z: grid_z, ..Drive::new() };
        let events = drive_capped(&mut race, &mut car, 0.5, &road, &tuning, 600.0);
        assert_eq!(events.last(), Some(&Event::Over(Out::OutOfTime)), "{events:?}");
        assert!(
            !events.iter().any(|e| matches!(e, Event::Checkpoint { .. } | Event::LapDone { .. })),
            "reached a checkpoint at half speed: {events:?}",
        );
        assert!((race.elapsed - w.checkpoint).abs() < 0.01);
    }

    /// The car is held before the green light, and released on it.
    #[test]
    fn the_countdown_holds_the_car_then_releases_it() {
        let (road, tuning, grid_z) = course();
        let w = Windows::derive(&road, &tuning, grid_z);
        let mut race = Race::new(w, &road, grid_z, GRID);
        assert!(!race.driving());
        let mut t = 0.0;
        let mut green_at = None;
        while t < COUNTDOWN_SECONDS + 1.0 {
            if race.advance(DT, grid_z, &road) == Some(Event::GreenLight) {
                green_at = Some(t);
                assert_eq!(race.clock, w.qualify, "the clock started before the green light");
            } else if green_at.is_none() {
                assert!(!race.driving(), "released before the green light at {t}");
                assert_eq!(race.clock, w.qualify, "the HUD shows the wrong clock during the lights");
            }
            t += DT;
        }
        let green_at = green_at.expect("no green light");
        assert!((green_at - COUNTDOWN_SECONDS).abs() < 0.02, "green at {green_at}");
        assert!(race.driving());
        assert!(race.clock < w.qualify, "the clock is not running after the green light");
    }

    /// A wrap of `z` is a lap; a crash rewind is lost ground. Both are
    /// what a signed step gives and neither is what a wrap counter gives.
    #[test]
    fn distance_survives_the_wrap_and_counts_a_rewind() {
        let (road, tuning, grid_z) = course();
        let w = Windows::derive(&road, &tuning, grid_z);
        let length = road.length();
        let mut race = Race::new(w, &road, length - 100.0, GRID);
        race.track_distance(50.0, &road);
        assert!((race.travelled - 150.0).abs() < 0.01, "across the wrap: {}", race.travelled);
        race.track_distance(50.0 - 1000.0 + length, &road);
        assert!((race.travelled + 850.0).abs() < 0.01, "after a rewind: {}", race.travelled);
        let _ = tuning;
    }
}
