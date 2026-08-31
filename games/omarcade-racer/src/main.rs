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
//! Controls: ← → steer · ↑ throttle · ↓ brake · Escape quit.

mod art;
mod drive;
mod render;
mod road;
mod scenery;

use omarcade_core::backend::winit_soft::{Idle, WinitBackend};
use omarcade_core::{Backend, Canvas, Game, InputEvent, Key, Roll, Theme};

use art::Art;
use drive::{Drive, Tuning};
use road::Road;

const TITLE: &str = "Omarcade Racer";
const WIDTH: u32 = 960;
const HEIGHT: u32 = 720;

/// How long the player gets between a bend appearing at the horizon and
/// arriving at it. **This is the constant the whole game is tuned from**
/// — top speed is solved from it (decision e515f892, tuning A). Change
/// this and the car's speed follows; do not set a speed directly.
const REACTION_SECONDS: f32 = 1.5;

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
    /// Traffic as (track position, lane in half-widths, livery index).
    ///
    /// Static for now — the cars sit where they are put. Making them
    /// drive is its own piece of work and wants its own decision, because
    /// "traffic that moves" is an AI question, not a rendering one.
    rivals: Vec<(f32, f32, usize)>,
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
        let road = render::demo_track();
        let tuning = Tuning::from_corner(&road, REACTION_SECONDS);

        let pixels_per_unit = WHEEL_PIXELS * WHEEL_TURNS_PER_SECOND / tuning.top_speed;

        // Spread traffic down the track at staggered lanes, so there is
        // something to judge distance against.
        //
        // Spaced by a FRACTION of the visible road rather than in
        // segments: at a draw distance of 120, a car nine segments ahead
        // is still only 7% of the way to the horizon and renders three
        // pixels tall. Spacing them across the visible depth is what makes
        // them arrive at a usable rate.
        let visible = road.draw_distance() as f32 * road.segment_length();
        let rivals = vec![
            (visible * 0.25, -0.45, 0),
            (visible * 0.60, 0.40, 1),
            (visible * 1.10, -0.20, 2),
            (visible * 1.80, 0.45, 3),
            (visible * 2.60, -0.40, 4),
        ];

        Racer {
            theme,
            art,
            road,
            tuning,
            car: Drive::new(),
            roll: Roll::new(),
            pixels_per_unit,
            rivals,
            left_held: false,
            right_held: false,
            throttle_held: false,
            brake_held: false,
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

    /// Throttle as 0..1. Brake wins over throttle when both are held,
    /// because a player pressing both wants to stop.
    fn throttle(&self) -> f32 {
        if self.brake_held {
            0.0
        } else if self.throttle_held {
            1.0
        } else {
            // Coasting, not free-wheeling: lifting off decelerates. A
            // racer that holds its speed with no input has no throttle.
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

            _ => {}
        }
        true
    }

    fn update(&mut self, dt: f32) {
        // A dt spike — a dragged window, a stalled compositor — must not
        // teleport the car through a corner or past a rival. Clamping to
        // roughly four frames keeps a hitch as a hitch.
        let dt = dt.min(1.0 / 15.0);

        self.car
            .update(dt, self.throttle(), self.steer(), &self.road, &self.tuning);
        self.roll
            .advance(self.car.speed, self.pixels_per_unit, dt);
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
            &self.rivals,
            0,
            0,
            WIDTH,
            HEIGHT,
        );
    }
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
        let mut g = racer();
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
        let mut g = racer();
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
        let mut g = racer();
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

    /// Traffic must be on the road, not in the scenery.
    #[test]
    fn every_rival_sits_on_the_track() {
        let g = racer();
        assert!(!g.rivals.is_empty());
        for (z, lane, livery) in &g.rivals {
            assert!(
                *z >= 0.0 && *z < g.road.length(),
                "a rival sits at {z}, off a track {} long",
                g.road.length(),
            );
            assert!(lane.abs() <= 1.0, "a rival sits at lane {lane}, off the road");
            assert!(*livery < 5, "livery {livery} is past the five that exist");
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
}
