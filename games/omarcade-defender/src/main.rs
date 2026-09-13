//! Omarcade's fourth title: a horizontally scrolling shooter.
//!
//! ★ STAGE ONE, AND ONLY STAGE ONE. Brian: "first we prove we can build
//! the world horizontal scrolling, with the mountain terrain below, no
//! special effects now then put me something that represents a ship and
//! we can fly it smoothly and it feels good... If we can get this we can
//! build everything around it."
//!
//! So there is no score, no enemies, no shooting, no sound. There is a
//! world that loops, mountains under it, and a ship that flies. The
//! flight model either feels right or it does not, and nothing else is
//! worth building until that is settled — content cannot rescue bad
//! handling.
//!
//! ⚠️ NOT YET REGISTERED IN packaging/install.sh. A game with no name,
//! no score file and no title screen has no business in the cabinet, and
//! adding it there would put an unfinished title in front of anyone who
//! installed the suite.

mod flight;
mod render;
mod world;

use omarcade_core::backend::winit_soft::{Idle, WinitBackend};
use omarcade_core::{Audio, AudioSystem, Backend, Canvas, Game, InputEvent, Key, Pause, Theme};

use flight::{Camera, Facing, Input, Ship};
use world::Terrain;

const TITLE: &str = "Omarcade";
const WIDTH: u32 = world::VIEW_W as u32;
const HEIGHT: u32 = world::VIEW_H as u32;

/// How many ridge samples across the whole world.
///
/// One sample every ~15 world units at four screens wide: fine enough
/// that interpolation is invisible, coarse enough that the ridge has
/// shape rather than noise.
const RIDGE_SAMPLES: usize = 1024;

/// The physics timestep.
///
/// ⚠️ FIXED, AND THE SAME 240Hz THE OTHER THREE USE. Physics that runs
/// on the frame duration changes behaviour with the frame rate, and the
/// reversal timing in `flight` is exactly the kind of thing that would
/// drift between a 60Hz and a 144Hz machine.
const FIXED_DT: f32 = 1.0 / 240.0;

struct Defender {
    theme: Theme,
    terrain: Terrain,
    ship: Ship,
    camera: Camera,
    pause: Pause,

    // Held keys, resolved into an `Input` each step.
    thrust_held: bool,
    up_held: bool,
    down_held: bool,

    /// Left over from the last frame, so a frame longer than the
    /// timestep does not silently drop simulation time.
    accumulator: f32,
}

impl Defender {
    fn new(theme: Theme) -> Self {
        let terrain = Terrain::generate(RIDGE_SAMPLES, 0x0DEF_E4DE);
        let ship = Ship::new(0.0);
        let mut camera = Camera::new(ship.x);
        // Snap rather than ease on the first frame: easing in from a
        // default position would read as an opening swoop nobody asked
        // for.
        camera.snap_to(&ship);

        Self {
            theme,
            terrain,
            ship,
            camera,
            pause: Pause::new(),
            thrust_held: false,
            up_held: false,
            down_held: false,
            accumulator: 0.0,
        }
    }

    /// Resolve held keys into this step's request.
    ///
    /// Both vertical keys held cancels out, which is what a player
    /// expects from two keys — the same rule the other three games use.
    fn input(&self) -> Input {
        Input {
            thrust: self.thrust_held,
            vertical: match (self.up_held, self.down_held) {
                (true, false) => -1.0,
                (false, true) => 1.0,
                _ => 0.0,
            },
            face: None,
        }
    }
}

impl Game for Defender {
    fn on_input(&mut self, event: InputEvent) -> bool {
        match event {
            // ⚠️ ESC QUITS. Never bound to anything else, in any title.
            InputEvent::KeyDown(Key::Escape) => return false,

            InputEvent::KeyDown(Key::P) => self.pause.toggle(),

            // ⚠️ ONLY KeyDown IS SWALLOWED WHILE PAUSED (the S11 rule).
            // Eating KeyUp too would leave a key "held" across the
            // pause, and the ship would fly off on resume because the
            // release never arrived.
            InputEvent::KeyDown(_) if self.pause.is_paused() => return true,

            // Facing is a key press, not a steering axis: Defender had
            // you face a direction and thrust that way, and the camera
            // starts its slide the moment you ask rather than when the
            // ship comes around.
            InputEvent::KeyDown(Key::Left) => self.ship.facing = Facing::West,
            InputEvent::KeyDown(Key::Right) => self.ship.facing = Facing::East,

            InputEvent::KeyDown(Key::Space) => self.thrust_held = true,
            InputEvent::KeyUp(Key::Space) => self.thrust_held = false,

            InputEvent::KeyDown(Key::Up) => self.up_held = true,
            InputEvent::KeyUp(Key::Up) => self.up_held = false,
            InputEvent::KeyDown(Key::Down) => self.down_held = true,
            InputEvent::KeyUp(Key::Down) => self.down_held = false,

            _ => {}
        }
        true
    }

    fn update(&mut self, dt: f32, _audio: &mut Audio<'_>) {
        if self.pause.is_paused() {
            return;
        }

        // Fixed-step accumulation. Clamped so a stalled frame — a
        // debugger, a laptop waking — does not spend a second of
        // wall-clock catching up in one go.
        self.accumulator = (self.accumulator + dt).min(0.25);
        let input = self.input();
        while self.accumulator >= FIXED_DT {
            self.ship.step(input, &self.terrain, FIXED_DT);
            self.camera.follow(&self.ship, FIXED_DT);
            self.accumulator -= FIXED_DT;
        }
    }

    fn render(&mut self, canvas: &mut Canvas<'_>) {
        render::draw(canvas, &self.terrain, &self.ship, &self.camera, &self.theme);
        self.pause.draw(canvas, &self.theme);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let theme = Theme::load();
    let audio = AudioSystem::new();
    let game = Defender::new(theme);

    WinitBackend::new(TITLE, WIDTH, HEIGHT)
        .idle(Idle::Animate { fps: 60 })
        .run(game, audio)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> Defender {
        Defender::new(Theme::fallback())
    }

    #[test]
    fn escape_quits_and_nothing_else_does() {
        let mut g = game();
        assert!(!g.on_input(InputEvent::KeyDown(Key::Escape)), "Esc must quit");

        let mut g = game();
        for k in [Key::Space, Key::P, Key::Left, Key::Right, Key::Up, Key::Down] {
            assert!(g.on_input(InputEvent::KeyDown(k)), "{k:?} must not quit");
            assert!(g.on_input(InputEvent::KeyUp(k)), "{k:?} release must not quit");
        }
    }

    #[test]
    fn pause_stops_the_ship_and_resume_releases_it() {
        let mut g = game();
        g.on_input(InputEvent::KeyDown(Key::Space));
        let mut audio = AudioSystem::new();
        {
            let mut a = audio.handle();
            g.update(0.5, &mut a);
        }
        let moving = g.ship.vx;
        assert!(moving > 0.0, "thrust should have accelerated the ship");

        g.on_input(InputEvent::KeyDown(Key::P));
        let at_pause = g.ship.vx;
        {
            let mut a = audio.handle();
            g.update(0.5, &mut a);
        }
        assert_eq!(g.ship.vx, at_pause, "a paused ship must not move");

        g.on_input(InputEvent::KeyDown(Key::P));
        {
            let mut a = audio.handle();
            g.update(0.1, &mut a);
        }
        assert_ne!(g.ship.vx, at_pause, "resuming must let it fly again");
    }

    /// ⚠️ THE S11 RULE, AS A TEST. Swallowing KeyUp while paused leaves
    /// the key held across the pause, and the ship flies off on resume
    /// because the release was eaten.
    #[test]
    fn a_key_released_during_a_pause_is_still_released() {
        let mut g = game();
        g.on_input(InputEvent::KeyDown(Key::Space));
        assert!(g.thrust_held);

        g.on_input(InputEvent::KeyDown(Key::P));
        g.on_input(InputEvent::KeyUp(Key::Space));
        assert!(!g.thrust_held, "the release must land even while paused");

        g.on_input(InputEvent::KeyDown(Key::P));
        let mut audio = AudioSystem::new();
        let mut a = audio.handle();
        g.update(0.5, &mut a);
        assert_eq!(g.ship.vx, 0.0, "the ship must not thrust after a released key");
    }

    #[test]
    fn both_vertical_keys_cancel() {
        let mut g = game();
        g.on_input(InputEvent::KeyDown(Key::Up));
        assert_eq!(g.input().vertical, -1.0);
        g.on_input(InputEvent::KeyDown(Key::Down));
        assert_eq!(g.input().vertical, 0.0, "both held should cancel");
        g.on_input(InputEvent::KeyUp(Key::Up));
        assert_eq!(g.input().vertical, 1.0);
    }

    #[test]
    fn the_arrows_set_facing_without_moving_the_ship() {
        let mut g = game();
        assert_eq!(g.ship.facing, Facing::East);
        g.on_input(InputEvent::KeyDown(Key::Left));
        assert_eq!(g.ship.facing, Facing::West);
        assert_eq!(g.ship.vx, 0.0, "facing is not thrust");
    }

    /// A long frame must not be simulated in one enormous step.
    #[test]
    fn a_stalled_frame_is_clamped() {
        let mut g = game();
        g.on_input(InputEvent::KeyDown(Key::Space));
        let mut audio = AudioSystem::new();
        let mut a = audio.handle();
        g.update(30.0, &mut a);
        // 30 seconds of thrust would have lapped the world many times;
        // the clamp means at most a quarter second was simulated.
        assert!(
            g.ship.vx <= flight::TOP_SPEED + 0.001,
            "speed escaped the clamp: {}",
            g.ship.vx
        );
    }
}
