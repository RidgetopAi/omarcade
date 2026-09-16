//! Omarcade's fourth title: a horizontally scrolling shooter.
//!
//! ★ STAGE ONE built the world and the flying, and Brian confirmed it:
//! "wow, you kind of nailed that. I honestly can't find anyhing wrong."
//! Those constants are settled and are not retuned here.
//!
//! ★ S5 IS THE STAGE THAT MAKES IT A GAME: you can fire, there are
//! Landers in the world, and they can be destroyed — with particles and
//! with sound.
//!
//! ⚠️ WHAT S5 DELIBERATELY DOES NOT HAVE, so the stage stays honest:
//! Landers do NOT hunt, do NOT abduct, and do NOT shoot back. All of
//! that is S6/S7, where there are Humanoids for them to hunt; building
//! it now would mean building it against imaginary Humanoids. There is
//! also no wave structure and no score file — a score is drawn so a kill
//! is visible, and S9 owns the real thing.
//!
//! ⚠️ NOT YET REGISTERED IN packaging/install.sh. A game with no name,
//! no score file and no title screen has no business in the cabinet, and
//! adding it there would put an unfinished title in front of anyone who
//! installed the suite.

mod art;
mod effects;
mod enemy;
mod flight;
mod render;
mod shot;
mod sound;
mod world;

use omarcade_core::audio::SoundId;
use omarcade_core::backend::winit_soft::{Idle, WinitBackend};
use omarcade_core::{Audio, AudioSystem, Backend, Canvas, Game, InputEvent, Key, Pause, Theme};

use effects::Effects;
use enemy::Landers;
use flight::{Camera, Facing, Input, Ship};
use shot::Shots;
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

/// How many Landers arrive at the start.
///
/// Brian's spec says Landers come in groups of 4-5. S9 owns waves; this
/// is one group, so that S5 has something to shoot.
const OPENING_LANDERS: usize = 5;

/// Where the gun sits relative to the ship's origin, in world units.
///
/// ⚠️ THE ART DECIDES THIS, NOT A GUESS. The ship's gun reaches to about
/// +20 units in art space at [`art::SCALE`], so a bolt leaving from the
/// centre would appear to emerge from the cockpit. Mirrored with facing,
/// because the ship is mirrored with facing.
const MUZZLE_FORWARD: f32 = 20.0 * art::SCALE;

struct Defender {
    theme: Theme,
    terrain: Terrain,
    ship: Ship,
    camera: Camera,
    pause: Pause,

    shots: Shots,
    landers: Landers,
    effects: Effects,
    score: u32,

    laser: SoundId,
    boom: SoundId,

    // Held keys, resolved into an `Input` each step.
    thrust_held: bool,
    up_held: bool,
    down_held: bool,
    fire_held: bool,

    /// Left over from the last frame, so a frame longer than the
    /// timestep does not silently drop simulation time.
    accumulator: f32,

    /// Sounds asked for during the fixed-step loop, played once the
    /// stepping is done.
    ///
    /// ⚠️ THE LOOP CANNOT PLAY THEM DIRECTLY. `update` may run several
    /// physics steps for one frame, and a burst that kills two Landers
    /// inside one frame would otherwise fire two identical one-shots a
    /// few microseconds apart — which is not twice as loud, it is a
    /// flam. Counting and playing once per frame is both correct and
    /// cheaper.
    fired_this_frame: bool,
    killed_this_frame: bool,
}

impl Defender {
    fn new(theme: Theme, laser: SoundId, boom: SoundId) -> Self {
        let terrain = Terrain::generate(RIDGE_SAMPLES, 0x0DEF_E4DE);
        let ship = Ship::new(0.0);
        let mut camera = Camera::new(ship.x);
        // Snap rather than ease on the first frame: easing in from a
        // default position would read as an opening swoop nobody asked
        // for.
        camera.snap_to(&ship);

        let mut landers = Landers::new();
        landers.scatter(OPENING_LANDERS, ship.x, &terrain, 0x5EED_1234);

        Self {
            theme,
            terrain,
            ship,
            camera,
            pause: Pause::new(),
            shots: Shots::new(),
            landers,
            effects: Effects::new(),
            score: 0,
            laser,
            boom,
            thrust_held: false,
            up_held: false,
            down_held: false,
            fire_held: false,
            accumulator: 0.0,
            fired_this_frame: false,
            killed_this_frame: false,
        }
    }

    /// Where a bolt leaves the ship, in world coordinates.
    fn muzzle(&self) -> (f32, f32) {
        (
            world::wrap(self.ship.x + MUZZLE_FORWARD * self.ship.facing.sign()),
            self.ship.y,
        )
    }

    /// One physics step: the ship, then everything it can interact with.
    fn step(&mut self, input: Input, dt: f32) {
        self.ship.step(input, &self.terrain, dt);
        self.camera.follow(&self.ship, dt);

        if self.fire_held && self.shots.ready() {
            let (mx, my) = self.muzzle();
            if self.shots.fire(mx, my, self.ship.facing.sign()) {
                self.fired_this_frame = true;
            }
        }

        self.shots.step(dt);
        self.landers.step(&self.terrain, dt);
        self.effects.update(dt);

        self.resolve_hits();
    }

    /// Shots meeting Landers.
    ///
    /// ⚠️ ITERATED BACKWARDS because a hit removes a shot by
    /// `swap_remove`, which moves the LAST element into the current
    /// index. Walking forward would skip whatever got swapped in — a
    /// bug that only shows up when two shots are in flight at once, and
    /// then only sometimes.
    fn resolve_hits(&mut self) {
        let mut i = self.shots.len();
        while i > 0 {
            i -= 1;
            let (sx, sy) = {
                let s = match self.shots.iter().nth(i) {
                    Some(s) => *s,
                    None => continue,
                };
                (s.x, s.y)
            };

            if let Some(target) = self.landers.hit_test(sx, sy) {
                let (lx, ly, lvx) = {
                    let l = self.landers.iter().nth(target).unwrap();
                    (l.x, l.y, l.vx)
                };
                self.score += self.landers.kill(target);
                self.effects.explode_lander(lx, ly, lvx);
                self.shots.consume(i);
                self.killed_this_frame = true;
            }
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

            // ⚠️ FIRE IS A HELD KEY, RATE-LIMITED BY THE GUN, not one
            // shot per KeyDown. Brian's spec says fire is unlimited, and
            // a per-press weapon turns that into a test of how fast the
            // player can tap.
            //
            // ⚠️⚠️ ENTER IS A PLACEHOLDER AND IT IS THE WRONG KEY.
            // core's `Key` has no letters but P and M (both taken), so
            // the natural binding — SPACE to fire, a letter to thrust —
            // is not expressible today. Space is THRUST here and the
            // flight model is approved, so it was not reassigned
            // unasked. ⇒ ASK BRIAN, then either add the keys core is
            // missing or move thrust deliberately. Do not ship on Enter.
            InputEvent::KeyDown(Key::Enter) => self.fire_held = true,
            InputEvent::KeyUp(Key::Enter) => self.fire_held = false,

            InputEvent::KeyDown(Key::Up) => self.up_held = true,
            InputEvent::KeyUp(Key::Up) => self.up_held = false,
            InputEvent::KeyDown(Key::Down) => self.down_held = true,
            InputEvent::KeyUp(Key::Down) => self.down_held = false,

            _ => {}
        }
        true
    }

    fn update(&mut self, dt: f32, audio: &mut Audio<'_>) {
        if self.pause.is_paused() {
            return;
        }

        self.fired_this_frame = false;
        self.killed_this_frame = false;

        // Fixed-step accumulation. Clamped so a stalled frame — a
        // debugger, a laptop waking — does not spend a second of
        // wall-clock catching up in one go.
        self.accumulator = (self.accumulator + dt).min(0.25);
        let input = self.input();
        while self.accumulator >= FIXED_DT {
            self.step(input, FIXED_DT);
            self.accumulator -= FIXED_DT;
        }

        // See `fired_this_frame`: one play per frame, however many
        // physics steps ran.
        if self.fired_this_frame {
            audio.play(self.laser);
        }
        if self.killed_this_frame {
            audio.play(self.boom);
        }
    }

    fn render(&mut self, canvas: &mut Canvas<'_>) {
        let scene = render::Scene {
            terrain: &self.terrain,
            ship: &self.ship,
            camera: &self.camera,
            shots: &self.shots,
            landers: &self.landers,
            effects: &self.effects,
            score: self.score,
        };
        render::draw(canvas, &scene, &self.theme);
        self.pause.draw(canvas, &self.theme);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let theme = Theme::load();
    let mut audio = AudioSystem::new();
    let laser = audio.register_sound(Box::new(sound::Laser::new()));
    let boom = audio.register_sound(Box::new(sound::Boom::new()));
    let game = Defender::new(theme, laser, boom);

    WinitBackend::new(TITLE, WIDTH, HEIGHT)
        .idle(Idle::Animate { fps: 60 })
        .run(game, audio)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> Defender {
        let mut audio = AudioSystem::new();
        let laser = audio.register_sound(Box::new(sound::Laser::new()));
        let boom = audio.register_sound(Box::new(sound::Boom::new()));
        Defender::new(Theme::fallback(), laser, boom)
    }

    #[test]
    fn escape_quits_and_nothing_else_does() {
        let mut g = game();
        assert!(!g.on_input(InputEvent::KeyDown(Key::Escape)), "Esc must quit");

        let mut g = game();
        for k in [Key::Space, Key::P, Key::Left, Key::Right, Key::Up, Key::Down, Key::Enter] {
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
