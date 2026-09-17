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
//! ★ S6 GAVE IT STAKES: there are people on the surface, and the Landers
//! come for them. A Lander drifts, picks the nearest Humanoid, drops on
//! it, and carries it upward — and you can shoot the carrier and catch
//! the person as they fall.
//!
//! ★★ S7 MADE IT POSSIBLE TO LOSE. A Humanoid carried off the top fuses
//! with its captor into a MUTANT, which does not want your civilians —
//! it wants you, and it never shoots straight. You have three lives.
//! And if every person is taken, THE WORLD EXPLODES: the mountains are
//! gone and every surviving Lander mutates. The difficulty does not fall
//! when there is nothing left to protect; it spikes.
//!
//! ⚠️ STILL DELIBERATELY MISSING: waves, a score file, the scanner, and
//! the other five enemy types. S9 owns scoring and waves; the score on
//! screen exists only so a kill is visible.
//!
//! ⚠️ NOT YET REGISTERED IN packaging/install.sh. A game with no name,
//! no score file and no title screen has no business in the cabinet, and
//! adding it there would put an unfinished title in front of anyone who
//! installed the suite.

mod art;
mod effects;
mod enemy;
mod flight;
mod humanoid;
mod lives;
mod render;
mod scanner;
mod shot;
mod sound;
mod world;

use omarcade_core::audio::SoundId;
use omarcade_core::backend::winit_soft::{Idle, WinitBackend};
use omarcade_core::{Audio, AudioSystem, Backend, Canvas, Game, InputEvent, Key, Pause, Theme};

use effects::Effects;
use enemy::Landers;
use flight::{Camera, Facing, Input, Ship};
use humanoid::Humanoids;
use lives::Lives;
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

/// How many people live on the surface.
///
/// Defender's own count. Enough that losing one hurts without ending the
/// game, which is what makes the middle of a wave tense rather than
/// merely lost.
const POPULATION: usize = 10;

/// Where the gun sits relative to the ship's origin, in world units.
///
/// ⚠️ THE ART DECIDES THIS, NOT A GUESS. The ship's gun reaches to about
/// +20 units in art space at [`art::SCALE`], so a bolt leaving from the
/// centre would appear to emerge from the cockpit. Mirrored with facing,
/// because the ship is mirrored with facing.
const MUZZLE_FORWARD: f32 = 20.0 * art::SCALE;

/// How low the ship must fly to put a rescued Humanoid back down.
///
/// ★ THE RISK/REWARD THE ARCADE IS BUILT ON. You are safest high up, and
/// the only way to return someone is to go low — so a rescue is not
/// finished at the catch, it is finished at the delivery.
const DROP_OFF_HEIGHT: f32 = 90.0;

/// The ship's hitbox, in world units.
///
/// ★ FROM THE ART, and deliberately TIGHTER than it. The ship draws 70
/// wide, and a hitbox that matched would make the long tail count as a
/// hit — a shot that visibly misses the cockpit but clips the fin reads
/// as unfair, and the arcade was generous here too.
const SHIP_HALF_W: f32 = 22.0;
const SHIP_HALF_H: f32 = 10.0;

struct Defender {
    theme: Theme,
    terrain: Terrain,
    ship: Ship,
    camera: Camera,
    pause: Pause,

    shots: Shots,
    landers: Landers,
    people: Humanoids,
    effects: Effects,
    lives: Lives,
    score: u32,
    /// ★ Whether the surface has already been destroyed, so the world
    /// ends exactly once rather than every frame the population is zero.
    world_ended: bool,

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

    /// Seconds of simulated time since the game began.
    ///
    /// ★ ADDED BY S8 FOR THE SCANNER'S MUTANT PULSE. Advanced inside the
    /// FIXED step rather than from the frame's `dt`, so it counts the
    /// same simulated time everything else does — a pulse driven by
    /// wall-clock would drift away from the game it is describing the
    /// moment a frame ran long, and would keep running while paused.
    elapsed: f32,

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
    rescued_this_frame: bool,
    enemy_fired_this_frame: bool,
    died_this_frame: bool,
    world_ended_this_frame: bool,
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

        let mut people = Humanoids::new();
        people.scatter(POPULATION, &terrain, 0x50C1_A15E);

        Self {
            theme,
            terrain,
            ship,
            camera,
            pause: Pause::new(),
            shots: Shots::new(),
            landers,
            people,
            effects: Effects::new(),
            lives: Lives::new(),
            world_ended: false,
            score: 0,
            laser,
            boom,
            thrust_held: false,
            up_held: false,
            down_held: false,
            fire_held: false,
            accumulator: 0.0,
            elapsed: 0.0,
            fired_this_frame: false,
            killed_this_frame: false,
            rescued_this_frame: false,
            enemy_fired_this_frame: false,
            died_this_frame: false,
            world_ended_this_frame: false,
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
        self.elapsed += dt;
        self.lives.step(dt);

        // ⚠️ A DEAD SHIP DOES NOT FLY, AND MUTANTS MUST NOT TRACK IT.
        // Handing them a position while the player cannot move means
        // they converge on the respawn point and kill the next life
        // instantly — the exact death loop the invulnerability exists to
        // prevent, reintroduced by an oversight.
        let ship_pos = if self.lives.is_flying() {
            self.ship.step(input, &self.terrain, dt);
            self.camera.follow(&self.ship, dt);
            Some((self.ship.x, self.ship.y))
        } else {
            None
        };

        if self.fire_held && self.lives.is_flying() && self.shots.ready() {
            let (mx, my) = self.muzzle();
            if self.shots.fire(mx, my, self.ship.facing.sign()) {
                self.fired_this_frame = true;
            }
        }

        self.shots.step(dt);

        // Mutants that want to shoot say so; the bolts are built here,
        // because `Landers` does not know what a Shot is.
        let wants = self.landers.step(&self.terrain, &mut self.people, ship_pos, dt);
        if let Some((sx, sy)) = ship_pos {
            for (index, mx, my) in wants {
                let mut noise = self.landers.next_noise();
                let (dx, dy) = match self.landers.get(index) {
                    Some(m) => m.aim_at(sx, sy, &mut noise),
                    None => continue,
                };
                self.shots.fire_enemy(mx, my, dx, dy);
                self.enemy_fired_this_frame = true;
            }
        }

        self.people.step(&self.terrain, dt);
        self.effects.update(dt);

        self.resolve_hits();
        self.resolve_catches();
        self.resolve_ship_hit();
        self.check_world_end();
    }

    /// An enemy bolt finding the ship.
    fn resolve_ship_hit(&mut self) {
        if !self.lives.is_vulnerable() {
            return;
        }
        if self
            .shots
            .enemy_hit(self.ship.x, self.ship.y, SHIP_HALF_W, SHIP_HALF_H)
            .is_some()
            && self.lives.hit()
        {
            self.effects.explode_ship(self.ship.x, self.ship.y, self.ship.vx);
            self.died_this_frame = true;
            // ⚠️ THE PASSENGER GOES WITH YOU. A rescued Humanoid riding
            // under a ship that explodes cannot simply carry on hovering
            // in mid-air, and silently deleting it would be worse.
            for i in 0..self.people.len() {
                if let Some(h) = self.people.get_mut(i) {
                    if h.state == humanoid::State::Rescued {
                        h.kill();
                    }
                }
            }
        }
    }

    /// ★★ THE WORLD ENDING. Every person gone means the surface goes too.
    fn check_world_end(&mut self) {
        if self.world_ended || self.people.alive() > 0 {
            return;
        }
        self.world_ended = true;
        self.terrain.destroy();
        // Every surviving Lander turns — the arcade's own spike in
        // difficulty at the exact moment there is nothing left to save.
        self.landers.mutate_all();
        self.effects.explode_world(&self.terrain, self.camera.x);
        self.world_ended_this_frame = true;
    }

    /// The ship flying into a falling Humanoid.
    ///
    /// ★ THE RESCUE. A caught person rides under the ship until it flies
    /// low enough to set them down, which is the loop the arcade built
    /// its whole risk/reward around: you are safest high up and you can
    /// only return someone by going low.
    fn resolve_catches(&mut self) {
        if let Some(i) = self.people.catch_test(self.ship.x, self.ship.y) {
            if let Some(h) = self.people.get_mut(i) {
                h.rescued();
                self.rescued_this_frame = true;
            }
        }

        // Carry passengers along, and put them down near the ground.
        self.people.carry_with_ship(self.ship.x, self.ship.y);
        let ground = self.terrain.height_at(self.ship.x);
        if self.ship.y - ground < DROP_OFF_HEIGHT {
            let terrain = &self.terrain;
            for i in 0..self.people.len() {
                if let Some(h) = self.people.get_mut(i) {
                    h.released_to_ground(terrain);
                }
            }
        }
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
            let (sx, sy, from_enemy) = {
                let s = match self.shots.iter().nth(i) {
                    Some(s) => *s,
                    None => continue,
                };
                (s.x, s.y, s.is_enemy())
            };

            // ⚠️⚠️ AN ENEMY BOLT MUST NOT BE TESTED AGAINST ENEMIES.
            // A Mutant's shot spawns INSIDE the Mutant's own hitbox, so
            // without this the very next frame finds the shooter and
            // kills it — Brian saw Mutants "just self destruct" about a
            // fire-interval after appearing, and this was why. The
            // owner check existed for shots hitting the SHIP and was
            // never mirrored for shots hitting ENEMIES.
            //
            // Enemy bolts also pass harmlessly through Humanoids: the
            // aliens are not here to shoot their own cargo.
            if from_enemy {
                continue;
            }

            if let Some(target) = self.landers.hit_test(sx, sy) {
                let (lx, ly, lvx) = {
                    let l = self.landers.iter().nth(target).unwrap();
                    (l.x, l.y, l.vx)
                };
                // ⚠️ DROP THE PASSENGER BEFORE KILLING THE CARRIER.
                // `kill` clears the target, so doing this after would
                // take the Humanoid with it silently and the whole
                // catch-and-rescue loop would never fire.
                self.landers.release_passenger(target, &mut self.people);
                self.score += self.landers.kill(target);
                self.effects.explode_lander(lx, ly, lvx);
                self.shots.consume(i);
                self.killed_this_frame = true;
                continue;
            }

            // ⚠️ AND YOUR OWN LASER KILLS PEOPLE. Brian's spec says
            // DON'T SHOOT THEM, and a rule the game quietly refuses to
            // let you break is not a rule anyone ever feels.
            if let Some(victim) = self.people.hit_test(sx, sy) {
                let (hx, hy) = {
                    let h = self.people.get(victim).unwrap();
                    (h.x, h.y)
                };
                if let Some(h) = self.people.get_mut(victim) {
                    h.kill();
                }
                self.effects.explode_humanoid(hx, hy);
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

            // ★ SPACE FIRES, T THRUSTS — Brian's call, having flown it:
            // "let's make 'T' thrust, space fire, that works pretty well
            // using arrow keys to steer." Space is the fire key every
            // shooter has, and thrust moved rather than fire settling
            // for a key nobody presses. `Key::T` was added to core for
            // this; the enum previously had no free letter at all.
            //
            // ⚠️ FIRE IS A HELD KEY, RATE-LIMITED BY THE GUN, not one
            // shot per KeyDown. Brian's spec says fire is unlimited, and
            // a per-press weapon turns that into a test of how fast the
            // player can tap.
            InputEvent::KeyDown(Key::Space) => self.fire_held = true,
            InputEvent::KeyUp(Key::Space) => self.fire_held = false,

            InputEvent::KeyDown(Key::T) => self.thrust_held = true,
            InputEvent::KeyUp(Key::T) => self.thrust_held = false,

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
        self.rescued_this_frame = false;
        self.enemy_fired_this_frame = false;
        self.died_this_frame = false;
        self.world_ended_this_frame = false;

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
        if self.killed_this_frame || self.died_this_frame {
            audio.play(self.boom);
        }
        if self.enemy_fired_this_frame {
            // Quieter than your own gun, so a busy screen does not drown
            // out the shot you actually fired.
            audio.play_with(self.laser, 0.55, 1.0);
        }
        if self.world_ended_this_frame {
            audio.play_with(self.boom, 1.0, 1.0);
        }
    }

    fn render(&mut self, canvas: &mut Canvas<'_>) {
        let scene = render::Scene {
            terrain: &self.terrain,
            ship: &self.ship,
            camera: &self.camera,
            shots: &self.shots,
            landers: &self.landers,
            people: &self.people,
            effects: &self.effects,
            score: self.score,
            lives: &self.lives,
            time: self.elapsed,
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
        for k in [Key::Space, Key::P, Key::Left, Key::Right, Key::Up, Key::Down, Key::T] {
            assert!(g.on_input(InputEvent::KeyDown(k)), "{k:?} must not quit");
            assert!(g.on_input(InputEvent::KeyUp(k)), "{k:?} release must not quit");
        }
    }

    #[test]
    fn pause_stops_the_ship_and_resume_releases_it() {
        let mut g = game();
        g.on_input(InputEvent::KeyDown(Key::T));
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
        g.on_input(InputEvent::KeyDown(Key::T));
        assert!(g.thrust_held);

        g.on_input(InputEvent::KeyDown(Key::P));
        g.on_input(InputEvent::KeyUp(Key::T));
        assert!(!g.thrust_held, "the release must land even while paused");

        g.on_input(InputEvent::KeyDown(Key::P));
        let mut audio = AudioSystem::new();
        let mut a = audio.handle();
        g.update(0.5, &mut a);
        assert_eq!(g.ship.vx, 0.0, "the ship must not thrust after a released key");
    }

    /// ⚠️⚠️ REGRESSION: MUTANTS USED TO SHOOT THEMSELVES DEAD.
    ///
    /// Brian: "mutants appear and after about 2 seconds they just self
    /// destruct." An enemy bolt spawns inside its shooter's own hitbox,
    /// and `resolve_hits` tested EVERY shot against the enemy list
    /// regardless of who fired it — so the frame after a Mutant fired,
    /// its own bolt found it and killed it.
    ///
    /// ⚠️ THE TESTS IN `enemy` COULD NOT CATCH THIS. They step the
    /// Landers directly and never build a Shot, so the whole collision
    /// path was outside them. The bug lived in the one file that only
    /// the real game exercises, which is exactly where it hid.
    #[test]
    fn an_enemy_bolt_does_not_kill_the_enemy_that_fired_it() {
        let mut g = game();
        g.people.clear();
        g.landers.clear();

        // A Mutant sitting next to the ship, and its own bolt right on
        // top of it — the exact geometry of the frame after it fires.
        let (mx, my) = (g.ship.x + 200.0, g.ship.y);
        g.landers.spawn(enemy::Lander::mutant(mx, my));
        assert_eq!(g.landers.len(), 1);

        g.shots.fire_enemy(mx, my, 1.0, 0.3);
        g.resolve_hits();

        assert_eq!(
            g.landers.len(),
            1,
            "a Mutant shot itself dead with its own bolt"
        );
        assert_eq!(g.score, 0, "and scored the player points for it");
    }

    /// The other half of the same rule: YOUR bolt must still kill them.
    #[test]
    fn your_own_bolt_still_kills_a_mutant() {
        let mut g = game();
        g.people.clear();
        g.landers.clear();

        let (mx, my) = (g.ship.x + 200.0, g.ship.y);
        g.landers.spawn(enemy::Lander::mutant(mx, my));
        g.shots.fire(mx, my, 1.0);
        g.resolve_hits();

        assert_eq!(g.score, enemy::MUTANT_POINTS, "your shot did not connect");
    }

    /// And an enemy bolt must not shoot the civilians either.
    #[test]
    fn an_enemy_bolt_does_not_kill_humanoids() {
        let mut g = game();
        g.landers.clear();
        g.people.clear();
        let (hx, hy) = (g.ship.x + 300.0, g.terrain.height_at(g.ship.x + 300.0));
        g.people.spawn(humanoid::Humanoid::new(hx, hy, 0.0));

        g.shots.fire_enemy(hx, hy, 1.0, 0.3);
        g.resolve_hits();

        assert_eq!(g.people.alive(), 1, "an alien shot its own cargo");
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
        g.on_input(InputEvent::KeyDown(Key::T));
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
