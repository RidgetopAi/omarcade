//! The racer — third Omarcade title.
//!
//! This file is only wiring: input to intent, time to the simulation,
//! state to the renderer. The game lives in `road`/`drive`, the pixels in
//! `render`.
//!
//! Note what is absent, as in Breakout and Pong: no winit, no softbuffer,
//! not in this file and not in this crate's `Cargo.toml`. Everything
//! crosses through `omarcade_core`'s seam.
//!
//! Controls: ← → steer · ↑ throttle · ↓ brake · Enter start/restart ·
//! Escape quit.

mod collide;
mod crash;
mod traffic;
mod art;
mod drive;
mod hud;
mod pace;
mod race;
mod render;
mod road;
mod scenery;
mod structures;
mod track;

use omarcade_core::backend::winit_soft::{Idle, WinitBackend};
use omarcade_core::{Backend, Canvas, Game, InputEvent, Key, Roll, Theme};

use art::Art;
use drive::{Drive, Tuning};
use hud::Flash;
use race::{Event, Phase, Race, Windows};
use road::Road;

const TITLE: &str = "Omarcade Racer";
const WIDTH: u32 = 960;
const HEIGHT: u32 = 720;

/// How long the player gets between a bend appearing at the horizon and
/// arriving at it. **This is the constant the whole game is tuned from**
/// — top speed is solved from it (decision e515f892, tuning A). Change
/// this and the car's speed follows; do not set a speed directly.
const REACTION_SECONDS: f32 = 1.5;

/// How many cars share the track with the player.
///
/// Five is what the static field carried and it reads well at this draw
/// distance: enough that a lap is a series of overtakes rather than one,
/// few enough that the road never looks like a car park. It is a
/// starting point to drive against, not a derived number.
const TRAFFIC_CARS: usize = 5;

/// How fast the tread scrolls, in source pixels per world unit travelled.
///
/// Uncalibrated since the tread landed, for want of a speed model to
/// measure it against. There is one now, so it is derived rather than
/// guessed: the wheel is ~9 source pixels of tread and should turn about
/// three times a second at full speed, which is fast enough to read as
/// motion and slow enough to stay under `MAX_ROLL_PER_FRAME` (past about
/// one row per frame a scrolling pattern aliases and visually reverses —
/// the wagon-wheel effect, which no amount of speed fixes).
///
///   pixels_per_unit = wheel_pixels * turns_per_second / top_speed
///
/// A ratio against top speed, so retuning the reaction window carries it
/// along instead of silently breaking it (L019).
const WHEEL_PIXELS: f32 = 9.0;
const WHEEL_TURNS_PER_SECOND: f32 = 3.0;

struct Racer {
    theme: Theme,
    art: Art,
    road: Road,
    tuning: Tuning,
    car: Drive,
    roll: Roll,
    pixels_per_unit: f32,
    /// The traffic. It DRIVES (decision 4a0707a3) and it never sees the
    /// player — `Field::advance` takes no player at all, which is how
    /// that rule is enforced rather than merely intended.
    traffic: traffic::Field,
    /// The fireball, while one is burning. `None` means racing.
    ///
    /// A crash does NOT end the run — the plan's fail state is a missed
    /// checkpoint, which is S11's work. A crash costs TIME: the car
    /// stops, it burns, and the clock keeps going. That is the whole
    /// punishment, and it is enough because the timer is what ends you.
    crash: Option<crash::Explosion>,
    /// The run: qualifying, the grid, the clock, the laps. Every limit
    /// in it is derived from the reference driver at start-up.
    race: Race,
    /// Where the car sits at the green light, for putting it back there.
    grid_z: f32,
    /// A line flashed mid-screen: GO, a checkpoint's banked time, a lap.
    flash: Option<Flash>,
    /// Steering tracked as two independent flags rather than one
    /// direction, exactly as Pong does. With a single `dir`, holding Left
    /// and tapping Right releases the wheel when Right lifts, leaving the
    /// car straight while Left is still physically held.
    left_held: bool,
    right_held: bool,
    throttle_held: bool,
    brake_held: bool,
}

impl Racer {
    fn new(theme: Theme) -> Self {
        let art = Art::load(&theme);
        // The shipped course. `render::demo_track()` is still there and is
        // still what the visual scenes use — it is one bend, sized to be
        // LOOKED at, and every dump_art render is calibrated against it.
        // This is the one that is meant to be driven.
        let road = track::grand_prix().build();
        let tuning = Tuning::from_corner(&road, REACTION_SECONDS);

        let pixels_per_unit = WHEEL_PIXELS * WHEEL_TURNS_PER_SECOND / tuning.top_speed;

        // The traffic field. Spacing, lanes and per-car cruise speeds all
        // live in `traffic::Field::grid` now — the reasoning that used to
        // sit here (space by a FRACTION of the visible depth, never in
        // segments, or the cars arrive three pixels tall) moved with it.
        let traffic = traffic::Field::grid(&road, TRAFFIC_CARS);

        let visible = road.draw_distance() as f32 * road.segment_length();

        // Where the car sits at the green light. Computed before `road`
        // moves into the struct.
        let start_z = road.wrap(structures::GRID_SETBACK * visible);

        // The time limits, derived by driving the course with the
        // reference driver rather than typed in. Two simulated laps at
        // 240Hz, a few milliseconds, once.
        let windows = Windows::derive(&road, &tuning, start_z);
        let race = Race::new(windows, &road, start_z, TRAFFIC_CARS + 1);

        Racer {
            theme,
            art,
            road,
            tuning,
            // On the grid, BEHIND the line. See `structures::GRID_SETBACK`.
            car: Drive { z: start_z, ..Drive::new() },
            roll: Roll::new(),
            pixels_per_unit,
            traffic,
            crash: None,
            race,
            grid_z: start_z,
            flash: None,
            left_held: false,
            right_held: false,
            throttle_held: false,
            brake_held: false,
        }
    }

    /// Put the car on the grid in the slot qualifying earned and start
    /// the race. `position` is from 1; the cars ahead on the grid are the
    /// slots in front of it.
    fn start_race(&mut self, position: usize) {
        self.car = Drive { z: self.grid_z, ..Drive::new() };
        self.roll = Roll::new();
        self.crash = None;
        self.flash = None;
        let ahead = position.saturating_sub(1).min(TRAFFIC_CARS);
        self.traffic = traffic::Field::grid_split(&self.road, TRAFFIC_CARS, ahead, self.grid_z);
        self.race.start_race(self.grid_z, &self.road);
    }

    /// Back to the grid for a fresh qualifying lap.
    fn restart(&mut self) {
        *self = Racer::new(self.theme);
    }

    /// React to what the race reported this frame.
    fn on_event(&mut self, event: Option<Event>) {
        if let Some(event) = event {
            if let Some(flash) = flash_for(event) {
                self.flash = Some(flash);
            }
        }
    }

    /// Steering input as -1..1, from the two held flags.
    fn steer(&self) -> f32 {
        match (self.left_held, self.right_held) {
            (true, false) => -1.0,
            (false, true) => 1.0,
            // Both or neither: straight. Both-held meaning straight is
            // the honest reading of the input, not a special case.
            _ => 0.0,
        }
    }

    /// Throttle as 0..1.
    ///
    /// Zero when braking: the two are separate inputs to `Drive::update`
    /// now, but there is no sense in feeding it both at once, and
    /// `Drive` resolves the conflict in the brake's favour anyway.
    ///
    /// Zero with nothing held is coasting, not free-wheeling: lifting off
    /// decelerates. A racer that holds its speed with no input has no
    /// throttle.
    fn throttle(&self) -> f32 {
        if self.brake_held || !self.throttle_held {
            0.0
        } else {
            1.0
        }
    }

    /// Brake as 0..1.
    ///
    /// Its own input rather than "throttle, but zero". Those were the
    /// same thing until this existed, which meant the brake key produced
    /// a four-second coast — indistinguishable from releasing throttle.
    fn brake(&self) -> f32 {
        if self.brake_held {
            1.0
        } else {
            0.0
        }
    }
}

impl Game for Racer {
    fn on_input(&mut self, event: InputEvent) -> bool {
        match event {
            InputEvent::CloseRequested => return false,
            InputEvent::KeyDown(Key::Escape) => return false,

            InputEvent::KeyDown(Key::Left) => self.left_held = true,
            InputEvent::KeyUp(Key::Left) => self.left_held = false,
            InputEvent::KeyDown(Key::Right) => self.right_held = true,
            InputEvent::KeyUp(Key::Right) => self.right_held = false,
            InputEvent::KeyDown(Key::Up) => self.throttle_held = true,
            InputEvent::KeyUp(Key::Up) => self.throttle_held = false,
            InputEvent::KeyDown(Key::Down) => self.brake_held = true,
            InputEvent::KeyUp(Key::Down) => self.brake_held = false,

            // Enter moves between sessions and nothing else: it starts
            // the race once qualified, and starts over once the run has
            // ended. Mid-lap it does nothing, so a stray press cannot
            // throw away a lap.
            InputEvent::KeyDown(Key::Enter) => match self.race.phase {
                Phase::Qualified { position, .. } => self.start_race(position),
                Phase::Finished { .. } | Phase::Over(_) => self.restart(),
                _ => {}
            },

            _ => {}
        }
        true
    }

    fn update(&mut self, dt: f32) {
        // A dt spike — a dragged window, a stalled compositor — must not
        // teleport the car through a corner or past a rival. Clamping to
        // roughly four frames keeps a hitch as a hitch.
        let dt = dt.min(1.0 / 15.0);

        if let Some(flash) = &mut self.flash {
            flash.remaining -= dt;
            if flash.remaining <= 0.0 {
                self.flash = None;
            }
        }

        // The lights. EVERYTHING holds — the car, the traffic, the clock
        // — so the field a session starts from is the one on the grid,
        // and the first second of the clock is the first second of
        // driving.
        if matches!(self.race.phase, Phase::Countdown { .. }) {
            let event = self.race.advance(dt, self.car.z, &self.road);
            self.on_event(event);
            return;
        }

        // THE TRAFFIC DRIVES WHATEVER THE PLAYER IS DOING, crash
        // included. A field that freezes while you burn would make the
        // restart a different race from the one you crashed out of.
        self.traffic.advance(dt, &self.road, &self.tuning);

        if let Some(fire) = &mut self.crash {
            // Burning. The car is stopped and THE CLOCK KEEPS RUNNING —
            // that IS the punishment, and the whole of it. A crash never
            // ends the run; an empty clock does. Input is ignored so a
            // held key cannot drive a wreck.
            self.car.speed = 0.0;
            if !fire.advance(dt) {
                self.crash = None;
            }
            let event = self.race.advance(dt, self.car.z, &self.road);
            self.on_event(event);
            // Deliberately NOT recycling while burning: the player is
            // not moving, so nothing has been overtaken, and recycling
            // reads distance travelled since a pass.
            return;
        }

        // Where the car was before this frame's move, for the swept
        // collision check below. A frame at 30fps covers more ground than
        // a car occupies, so testing only the new position steps clean
        // over traffic.
        let prev_z = self.car.z;

        if self.race.driving() {
            self.car.update(
                dt,
                self.throttle(),
                self.brake(),
                self.steer(),
                &self.road,
                &self.tuning,
            );
        } else {
            // Past the flag, or out. The car brakes itself to a stop and
            // the keys do nothing; the banner has the next move.
            self.car.update(dt, 0.0, 1.0, 0.0, &self.road, &self.tuning);
        }
        self.roll
            .advance(self.car.speed, self.pixels_per_unit, dt);

        // Supply, not driving: cars that have fallen well behind come
        // back out at the horizon so there is always something to
        // overtake. Five cars on a 2.7-mile loop cannot do that on their
        // own — measured, see `probe_traffic`. Kept a SEPARATE call so
        // `advance` stays provably blind.
        self.traffic.recycle(self.car.z, &self.road);

        // Did that step put us into anything? Checked AFTER the move,
        // so the frame the player drives into a car is the frame it
        // registers rather than the one after. Only while driving: a car
        // rolling to a stop after the flag cannot crash.
        if self.race.driving() {
            if let Some(hit) = collide::check(&self.car, prev_z, &self.traffic, &self.road) {
                // ⚠️ REWIND THE PLAYER TO THE POINT OF CONTACT. The check
                // is swept, so `car.update` has already carried the car
                // PAST where the impact happened — up to a frame's travel,
                // which is 267 units at 60fps and 1067 at the clamped
                // 15fps. Left there, the wreck comes to rest beyond its
                // own fireball and the fire renders behind the car. Brian
                // saw exactly that. The race counts distance by signed z
                // steps, so the rewind is ground to re-drive, as it should
                // be.
                self.car.z = self.road.wrap(hit.player_z);
                self.car.speed = 0.0;
                self.crash = Some(crash::Explosion::start(hit.z, hit.x));
            }
        }

        // The race sees the car where it ended up, rewind included.
        let event = self.race.advance(dt, self.car.z, &self.road);
        self.on_event(event);
    }

    fn render(&mut self, canvas: &mut Canvas<'_>) {
        render::draw_road_into(
            canvas,
            &self.art,
            &self.theme,
            &self.road,
            &self.tuning,
            &self.car,
            self.roll.phase(),
            &self.traffic.as_rendered(),
            0,
            0,
            WIDTH,
            HEIGHT,
        );

        // The fireball goes on top of the road, after it. It is an
        // overlay rather than part of the scene graph — there is one at
        // most, and only while a crash is burning.
        if let Some(fire) = &self.crash {
            render::draw_explosion_into(
                canvas,
                &self.art.explosion,
                &self.theme,
                &self.road,
                &self.car,
                fire,
                0,
                0,
                WIDTH,
                HEIGHT,
            );
        }

        // The HUD goes over everything, fireball included: the clock is
        // the one thing that must never be hidden, because it is the
        // thing that ends you.
        let layout = hud::compose(&self.race, self.flash.as_ref());
        hud::draw(canvas, &self.theme, &layout, WIDTH, HEIGHT);
    }
}

/// The line flashed for an event, if it gets one. The banner speaks
/// for qualifying, finishing and going out; these are the moments that
/// pass while you are still driving.
///
/// A free function rather than a method so the HUD's font-coverage test
/// can ask for exactly the strings the game will produce.
pub fn flash_for(event: Event) -> Option<Flash> {
    let line = match event {
        Event::GreenLight => "GO".to_string(),
        Event::Checkpoint { remaining } => format!("+{remaining:.1}"),
        Event::LapDone { lap, remaining } => format!("LAP {lap} DONE  +{remaining:.1}"),
        Event::Qualified { .. } | Event::Finished { .. } | Event::Over(_) => return None,
    };
    Some(Flash::new(line))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let theme = Theme::load();

    WinitBackend::new(TITLE, WIDTH, HEIGHT)
        .idle(Idle::Animate { fps: 60 })
        .run(Racer::new(theme))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn racer() -> Racer {
        Racer::new(Theme::load())
    }

    /// A racer past the lights, on its qualifying lap, so a test about
    /// driving is not silently a test about the countdown holding the car.
    fn on_track() -> Racer {
        let mut g = racer();
        g.race.phase = Phase::Qualifying;
        g
    }

    /// The gotcha Pong documented and this file inherits: with a single
    /// direction rather than two flags, tapping one way while holding the
    /// other leaves the car straight while a key is still physically
    /// down.
    #[test]
    fn releasing_one_key_leaves_the_other_still_steering() {
        let mut g = racer();
        g.on_input(InputEvent::KeyDown(Key::Left));
        assert_eq!(g.steer(), -1.0);

        // Tap right while left is still held.
        g.on_input(InputEvent::KeyDown(Key::Right));
        assert_eq!(g.steer(), 0.0, "both held should read as straight");
        g.on_input(InputEvent::KeyUp(Key::Right));

        assert_eq!(g.steer(), -1.0, "left was still held and must resume");
    }

    /// Brake beats throttle. A player holding both wants to stop.
    #[test]
    fn braking_overrides_throttle() {
        let mut g = racer();
        g.on_input(InputEvent::KeyDown(Key::Up));
        assert_eq!(g.throttle(), 1.0);
        g.on_input(InputEvent::KeyDown(Key::Down));
        assert_eq!(g.throttle(), 0.0);
    }

    #[test]
    fn escape_and_close_quit_but_driving_does_not() {
        let mut g = racer();
        assert!(g.on_input(InputEvent::KeyDown(Key::Left)));
        assert!(g.on_input(InputEvent::KeyDown(Key::Up)));
        assert!(!g.on_input(InputEvent::KeyDown(Key::Escape)));
        assert!(!g.on_input(InputEvent::CloseRequested));
    }

    /// Holding throttle must actually move the car down the track.
    #[test]
    fn holding_throttle_drives_the_car() {
        let mut g = on_track();
        g.on_input(InputEvent::KeyDown(Key::Up));
        for _ in 0..120 {
            g.update(1.0 / 60.0);
        }
        assert!(g.car.speed > 0.0, "the car never moved");
        assert!(g.car.z > 0.0, "the car never advanced down the track");
    }

    /// A stalled frame must not teleport the car. Without the clamp a
    /// two-second hitch would jump it two seconds down the road, through
    /// any corner or rival in between.
    #[test]
    fn a_stalled_frame_does_not_teleport_the_car() {
        let mut g = on_track();
        g.on_input(InputEvent::KeyDown(Key::Up));
        for _ in 0..600 {
            g.update(1.0 / 60.0);
        }
        let before = g.car.z;

        g.update(2.0);
        let jumped = g.car.z - before;
        let one_frame = g.tuning.top_speed / 60.0;
        assert!(
            jumped < one_frame * 5.0,
            "a 2s stall moved the car {jumped} units, over five frames' worth",
        );
    }

    /// The tread must roll when moving and must never exceed the cap
    /// where a scrolling pattern aliases into running backwards.
    #[test]
    fn the_tread_rolls_without_aliasing() {
        let mut g = on_track();
        g.on_input(InputEvent::KeyDown(Key::Up));

        let mut last = g.roll.phase();
        let mut moved = false;
        for _ in 0..600 {
            g.update(1.0 / 60.0);
            let step = (g.roll.phase() - last).abs();
            assert!(
                step <= omarcade_core::sprite::MAX_ROLL_PER_FRAME + 1e-6,
                "tread advanced {step} in one frame, past the aliasing cap",
            );
            if step > 0.0 {
                moved = true;
            }
            last = g.roll.phase();
        }
        assert!(moved, "the tread never turned while driving");
    }

    /// Driving into a car lights a fireball, and the fireball ends.
    ///
    /// The full crash cycle in one test, because the pieces are correct
    /// individually and it is the SEQUENCE that can be wrong: hit ->
    /// burn -> back to racing.
    #[test]
    fn hitting_a_car_starts_a_crash_that_ends_by_itself() {
        let mut g = on_track();

        // Put a car directly in front of the player, in the same lane.
        g.traffic.cars[0].z = g.road.wrap(g.car.z + 100.0);
        g.traffic.cars[0].x = g.car.x;
        g.car.speed = g.tuning.top_speed * 0.5;

        assert!(g.crash.is_none(), "started already crashed");

        g.update(1.0 / 60.0);
        assert!(g.crash.is_some(), "drove into a car and nothing happened");

        // The car is stopped while it burns — that is the cost, since the
        // plan's fail state is a missed checkpoint rather than the crash.
        g.update(1.0 / 60.0);
        assert_eq!(g.car.speed, 0.0, "a burning wreck is still moving");

        // And it ends on its own.
        for _ in 0..(crash::BURN_TIME * 120.0) as usize {
            g.update(1.0 / 60.0);
        }
        assert!(g.crash.is_none(), "the fireball never burned out");
    }

    /// Input must not drive a wreck.
    #[test]
    fn holding_the_throttle_through_a_crash_does_nothing() {
        let mut g = on_track();
        g.traffic.cars[0].z = g.road.wrap(g.car.z + 100.0);
        g.traffic.cars[0].x = g.car.x;
        g.car.speed = g.tuning.top_speed * 0.5;
        g.update(1.0 / 60.0);
        assert!(g.crash.is_some());

        g.throttle_held = true;
        for _ in 0..30 {
            g.update(1.0 / 60.0);
            assert_eq!(
                g.car.speed, 0.0,
                "the throttle moved the car while it was a fireball"
            );
        }
    }

    /// The traffic keeps driving while the player burns.
    ///
    /// A field that freezes during a crash would make the restart a
    /// different race from the one that was crashed out of.
    #[test]
    fn traffic_keeps_driving_through_a_crash() {
        let mut g = on_track();
        g.traffic.cars[0].z = g.road.wrap(g.car.z + 100.0);
        g.traffic.cars[0].x = g.car.x;
        g.car.speed = g.tuning.top_speed * 0.5;
        g.update(1.0 / 60.0);
        assert!(g.crash.is_some());

        // Watch a car that is NOT the one that was hit.
        let watched = 2;
        let before = g.traffic.cars[watched].z;
        for _ in 0..60 {
            g.update(1.0 / 60.0);
        }
        assert!(
            (g.traffic.cars[watched].z - before).abs() > 1.0,
            "the field froze while the player was burning"
        );
    }

    /// Traffic must be on the road, not in the scenery.
    ///
    /// ⚠️ CHECKED AFTER DRIVING, not only at construction. The old
    /// version of this test ran against a field that had never moved,
    /// which was adequate when the cars were static and is worthless now
    /// that they drive: every interesting way for a car to end up in the
    /// scenery happens during `advance`, not during `grid`. Twenty
    /// seconds of simulation is several corners of the shipped course.
    #[test]
    fn every_rival_sits_on_the_track() {
        let mut g = racer();
        assert!(!g.traffic.cars.is_empty());

        for step in 0..(20.0 * 60.0) as usize {
            g.traffic.advance(1.0 / 60.0, &g.road, &g.tuning);
            for car in &g.traffic.cars {
                assert!(
                    car.z >= 0.0 && car.z < g.road.length(),
                    "after {step} steps a rival sits at {}, off a track {} long",
                    car.z,
                    g.road.length(),
                );
                assert!(
                    car.x.abs() <= 1.0,
                    "after {step} steps a rival sits at lane {}, off the road",
                    car.x
                );
                assert!(
                    car.livery < 5,
                    "livery {} is past the five that exist",
                    car.livery
                );
            }
        }
    }

    /// The tuning must be the one that was chosen, not whatever a
    /// refactor leaves behind. Decision e515f892.
    #[test]
    fn the_game_ships_the_tuning_that_was_chosen() {
        let g = racer();
        assert_eq!(g.tuning.derived, drive::Derivation::FromCorner);
        assert!(
            (g.tuning.reaction_seconds(&g.road) - REACTION_SECONDS).abs() < 0.001,
            "the shipped reaction window is {}, not {REACTION_SECONDS}",
            g.tuning.reaction_seconds(&g.road),
        );
    }

    /// Nothing moves during the lights: not the car under a held key,
    /// not the traffic, not the clock.
    #[test]
    fn the_lights_hold_everything() {
        let mut g = racer();
        assert!(matches!(g.race.phase, Phase::Countdown { .. }));
        let car_z = g.car.z;
        let traffic_z: Vec<f32> = g.traffic.cars.iter().map(|c| c.z).collect();
        let clock = g.race.clock;
        g.on_input(InputEvent::KeyDown(Key::Up));
        for _ in 0..30 {
            g.update(1.0 / 60.0);
        }
        assert_eq!(g.car.z, car_z, "the car moved during the countdown");
        assert_eq!(g.car.speed, 0.0);
        let now: Vec<f32> = g.traffic.cars.iter().map(|c| c.z).collect();
        assert_eq!(now, traffic_z, "the traffic moved during the countdown");
        assert_eq!(g.race.clock, clock, "the clock ran during the countdown");
    }

    /// After the green light the same held key drives the car.
    #[test]
    fn the_green_light_releases_the_car() {
        let mut g = racer();
        g.on_input(InputEvent::KeyDown(Key::Up));
        for _ in 0..((race::COUNTDOWN_SECONDS + 1.0) * 60.0) as usize {
            g.update(1.0 / 60.0);
        }
        assert_eq!(g.race.phase, Phase::Qualifying);
        assert!(g.car.speed > 0.0, "the car never moved after the green light");
        assert!(g.race.clock < g.race.windows.qualify, "the clock never started");
    }

    /// The clock runs while the wreck burns. That is the entire cost of
    /// a crash, so if it stopped, crashing would be free.
    #[test]
    fn the_clock_runs_while_burning() {
        let mut g = racer();
        g.race.phase = Phase::Racing { lap: 1 };
        g.race.clock = 30.0;
        g.crash = Some(crash::Explosion::start(g.car.z + 500.0, 0.0));
        for _ in 0..12 {
            g.update(1.0 / 60.0);
        }
        assert!(g.crash.is_some(), "fixture: the fire should still be burning");
        assert!(
            (g.race.clock - (30.0 - 12.0 / 60.0)).abs() < 1e-3,
            "the clock did not run while burning: {}",
            g.race.clock,
        );
    }

    /// Enter after qualifying starts the race from the earned slot: the
    /// grid has that many cars ahead and the rest behind.
    #[test]
    fn enter_starts_the_race_from_the_grid_slot() {
        let mut g = racer();
        g.race.phase = Phase::Qualified { time: 100.0, position: 3 };
        g.on_input(InputEvent::KeyDown(Key::Enter));
        assert!(
            matches!(g.race.phase, Phase::Countdown { then: race::Session::Race, .. }),
            "{:?}",
            g.race.phase,
        );
        assert_eq!(g.car.z, g.grid_z);
        assert_eq!(g.car.speed, 0.0);
        let length = g.road.length();
        let ahead = g
            .traffic
            .cars
            .iter()
            .filter(|c| (c.z - g.car.z).rem_euclid(length) < length / 2.0)
            .count();
        assert_eq!(ahead, 2, "grid slot 3 means two cars ahead");
        assert_eq!(g.traffic.cars.len(), TRAFFIC_CARS);
    }

    /// Enter mid-lap does nothing; a stray press cannot throw a lap away.
    #[test]
    fn enter_mid_lap_is_ignored() {
        let mut g = racer();
        g.race.phase = Phase::Qualifying;
        g.race.clock = 42.0;
        g.on_input(InputEvent::KeyDown(Key::Enter));
        assert_eq!(g.race.phase, Phase::Qualifying);
        assert_eq!(g.race.clock, 42.0);
    }

    /// Enter after the end starts a fresh run on the grid.
    #[test]
    fn enter_after_the_end_restarts() {
        let mut g = racer();
        g.race.phase = Phase::Over(race::Out::OutOfTime);
        g.car.z = 12345.0;
        g.on_input(InputEvent::KeyDown(Key::Enter));
        assert!(matches!(g.race.phase, Phase::Countdown { then: race::Session::Qualifying, .. }));
        assert_eq!(g.car.z, g.grid_z);
    }

    /// Past the flag the keys are dead and the car stops by itself.
    #[test]
    fn after_the_flag_the_car_stops_and_ignores_the_keys() {
        let mut g = racer();
        g.race.phase = Phase::Finished { time: 270.0 };
        g.car.speed = g.tuning.top_speed;
        g.on_input(InputEvent::KeyDown(Key::Up));
        for _ in 0..(3.0 * 60.0) as usize {
            g.update(1.0 / 60.0);
        }
        assert_eq!(g.car.speed, 0.0, "the car kept going after the flag");
    }
}
