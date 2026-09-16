//! Explosions.
//!
//! ★ THE FIRST USE OF [`omarcade_core::ParticlePool`] BY ANY GAME. It has
//! existed, with gravity, since before Pixel Break shipped and nothing has
//! ever spawned into it.
//!
//! ⚠️ AND THE POOL CANNOT SIMPLY BE USED AS-IS, FOR ONE REASON: it stores
//! positions and draws them where they are. Our world WRAPS and SCROLLS,
//! so a particle's world x is not its screen x, and an explosion at the
//! seam has debris whose coordinates are a whole world apart while the
//! debris itself is a few pixels wide.
//!
//! So the pool is kept in WORLD space — particles are spawned with world
//! coordinates and the pool's own `update` does the physics — and this
//! module does the drawing itself, one particle at a time, through the
//! camera. [`ParticlePool::draw`] is deliberately NOT called: it would
//! draw world coordinates straight onto the screen, which is correct only
//! when the camera is at zero, i.e. exactly where a careless test looks.

use omarcade_core::{Canvas, Color, Particle, ParticlePool, Vec2};

use crate::flight::Camera;

/// How many particles can be in the air at once.
///
/// Enough for several overlapping explosions; the pool recycles its
/// oldest rather than dropping new ones, so a busy moment degrades by
/// shortening old debris rather than by swallowing a new blast.
pub const CAPACITY: usize = 512;

/// Downward acceleration on debris, in world units per second squared.
///
/// ⚠️ POSITIVE IS DOWN IN PARTICLE SPACE BUT UP IN WORLD SPACE. The
/// pool adds `gravity` to `vel.y`, and the game's y measures height ABOVE
/// the floor, so debris must be pulled toward SMALLER y — a negative
/// number here. Getting this backwards makes explosions rise like
/// balloons, which is the kind of thing that looks almost deliberate.
pub const GRAVITY: f32 = -520.0;

/// How many pieces a Lander comes apart into.
pub const LANDER_PIECES: usize = 26;

/// How many pieces a Humanoid comes apart into. Fewer, and smaller.
pub const HUMANOID_PIECES: usize = 12;

/// How many pieces the ship comes apart into. More, and bigger.
pub const SHIP_PIECES: usize = 40;

/// How many pieces the world throws up when it ends.
pub const WORLD_PIECES: usize = 150;

/// How fast the debris leaves, in world units per second.
const BURST_SPEED: f32 = 210.0;

/// How long a piece lives, in seconds, before and after jitter.
const PIECE_LIFE: f32 = 0.55;

/// Explosions, in world space.
pub struct Effects {
    pool: ParticlePool,
    /// Advanced on every spawn so two explosions never throw identical
    /// debris. Deterministic, so a render can be reproduced.
    seed: u32,
}

impl Default for Effects {
    fn default() -> Self {
        Self::new()
    }
}

impl Effects {
    pub fn new() -> Self {
        Self {
            pool: ParticlePool::with_capacity(CAPACITY).with_gravity(GRAVITY),
            seed: 0x51ED_2701,
        }
    }

    pub fn len(&self) -> usize {
        self.pool.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pool.is_empty()
    }

    pub fn clear(&mut self) {
        self.pool.clear();
    }

    pub fn update(&mut self, dt: f32) {
        self.pool.update(dt);
    }

    fn rand(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
    }

    /// Blow a Lander apart at `(x, y)` in world coordinates.
    ///
    /// Debris carries the dead thing's own velocity, so a Lander shot
    /// while drifting east throws its wreckage east — an explosion that
    /// bursts symmetrically out of a moving object reads as a sprite
    /// being replaced rather than a thing being destroyed.
    pub fn explode_lander(&mut self, x: f32, y: f32, vx: f32) {
        // The Lander's own greens, plus the hot core a blast has for the
        // first instant. Pieces are drawn additively, so these read as
        // light rather than as paint.
        const COLORS: [Color; 4] = [
            Color::rgb(30, 218, 16),
            Color::rgb(0, 143, 2),
            Color::rgb(255, 230, 66),
            Color::rgb(255, 174, 0),
        ];

        for i in 0..LANDER_PIECES {
            // Spread evenly around the circle and then jitter, rather
            // than drawing the angle at random: pure random angles clump
            // visibly at this count and leave bald patches in the burst.
            let spread = (i as f32 + self.rand()) / LANDER_PIECES as f32;
            let angle = spread * std::f32::consts::TAU;

            // ⚠️ SPEED IS SQUARED, AND THAT IS WHAT MAKES IT A BURST.
            // A uniform speed put every piece at nearly the same radius,
            // so the explosion rendered as an expanding RING with a hole
            // in the middle — visible immediately in a frame and not at
            // all in the code. Squaring biases the draw toward the slow
            // end, which fills the centre while a few pieces still throw
            // clear. Found by looking, like everything else on this list.
            let r = self.rand();
            let speed = BURST_SPEED * (0.12 + 1.15 * r * r);

            let color = COLORS[i % COLORS.len()];
            let life = PIECE_LIFE * (0.6 + 0.7 * self.rand());
            let size = 2.0 + 3.0 * self.rand();

            self.pool.spawn(Particle::new(
                Vec2::new(x, y),
                // ⚠️ The sin term is NEGATED because world y grows UP
                // while the angle is measured the usual screen way. Left
                // alone, every burst would be vertically mirrored — which
                // is invisible in a symmetric burst and would only show
                // up once gravity was added.
                Vec2::new(angle.cos() * speed + vx * 0.35, -angle.sin() * speed),
                size,
                color,
                life,
            ));
        }
    }

    /// A Humanoid killed by your own laser.
    ///
    /// ⚠️ DELIBERATELY SMALLER AND IN ITS OWN COLOURS. A person dying
    /// must not look like a kill you meant to make — the same big green
    /// burst a Lander gives would read as a score, and this is the
    /// opposite of a score.
    pub fn explode_humanoid(&mut self, x: f32, y: f32) {
        const COLORS: [Color; 3] = [
            Color::rgb(255, 0, 234),
            Color::rgb(0, 255, 0),
            Color::rgb(255, 221, 0),
        ];

        for i in 0..HUMANOID_PIECES {
            let spread = (i as f32 + self.rand()) / HUMANOID_PIECES as f32;
            let angle = spread * std::f32::consts::TAU;
            let r = self.rand();
            let speed = BURST_SPEED * 0.55 * (0.12 + 1.15 * r * r);

            // Hoisted: `self.rand()` inside a `self.pool` call borrows
            // self twice.
            let size = 1.5 + 2.0 * self.rand();
            let life = PIECE_LIFE * (0.5 + 0.6 * self.rand());
            self.pool.spawn(Particle::new(
                Vec2::new(x, y),
                Vec2::new(angle.cos() * speed, -angle.sin() * speed),
                size,
                COLORS[i % COLORS.len()],
                life,
            ));
        }
    }

    /// The player's ship, destroyed.
    ///
    /// Bigger and in the ship's own blues, so dying does not look like
    /// another kill you scored.
    pub fn explode_ship(&mut self, x: f32, y: f32, vx: f32) {
        const COLORS: [Color; 4] = [
            Color::rgb(45, 81, 225),
            Color::rgb(140, 146, 222),
            Color::rgb(255, 214, 120),
            Color::rgb(255, 255, 255),
        ];

        for i in 0..SHIP_PIECES {
            let spread = (i as f32 + self.rand()) / SHIP_PIECES as f32;
            let angle = spread * std::f32::consts::TAU;
            let r = self.rand();
            let speed = BURST_SPEED * 1.35 * (0.12 + 1.15 * r * r);
            let size = 2.5 + 4.0 * self.rand();
            let life = PIECE_LIFE * (0.9 + 0.9 * self.rand());

            self.pool.spawn(Particle::new(
                Vec2::new(x, y),
                Vec2::new(angle.cos() * speed + vx * 0.3, -angle.sin() * speed),
                size,
                COLORS[i % COLORS.len()],
                life,
            ));
        }
    }

    /// ★★ THE WORLD ENDING. The surface blows apart along its whole
    /// visible length.
    ///
    /// ⚠️ SEEDED FROM THE RIDGE THE WORLD USED TO HAVE, via
    /// `height_at_raw` — by the time this is called the terrain is
    /// already destroyed and `height_at` returns zero, so asking it
    /// where the mountains were would put the entire explosion on the
    /// floor. The shape being lost is the shape worth showing.
    pub fn explode_world(&mut self, terrain: &crate::world::Terrain, camera_x: f32) {
        let span = crate::world::VIEW_W * 1.4;
        for i in 0..WORLD_PIECES {
            let t = i as f32 / WORLD_PIECES as f32;
            let x = crate::world::wrap(camera_x - span * 0.2 + span * t);
            let y = terrain.height_at_raw(x);

            let r = self.rand();
            let angle = (self.rand() - 0.5) * std::f32::consts::PI * 0.8;
            let speed = 340.0 * (0.25 + r);
            let size = 3.0 + 5.0 * self.rand();
            let life = 1.4 * (0.6 + 0.8 * self.rand());

            // Thrown UPWARD and outward — the ground itself coming apart.
            self.pool.spawn(Particle::new(
                Vec2::new(x, y),
                Vec2::new(angle.sin() * speed * 0.6, angle.cos() * speed),
                size,
                if i % 3 == 0 {
                    Color::rgb(255, 180, 60)
                } else {
                    Color::rgb(220, 110, 40)
                },
                life,
            ));
        }
    }

    /// Draw every particle through the camera.
    ///
    /// ⚠️ NOT `ParticlePool::draw`. See the module note: the pool draws
    /// where a particle IS, and in this game where it is and where it
    /// appears are different questions.
    pub fn draw(&self, canvas: &mut Canvas<'_>, camera: &Camera) {
        let h = canvas.height() as f32;
        for p in self.pool.particles() {
            let sx = camera.to_screen(p.pos.x);
            // Off-screen in x is common — the world is four screens wide
            // and most debris is somewhere else.
            if sx < -8.0 || sx > canvas.width() as f32 + 8.0 {
                continue;
            }
            let sy = h - p.pos.y;
            let half = p.size * 0.5;
            canvas.fill_rect_add_f(sx - half, sy - half, p.size, p.size, p.fade());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world;

    #[test]
    fn an_explosion_fills_the_pool_and_then_empties_it() {
        let mut fx = Effects::new();
        assert!(fx.is_empty());
        fx.explode_lander(500.0, 300.0, 0.0);
        assert_eq!(fx.len(), LANDER_PIECES);

        // Every piece must die within the longest possible life.
        fx.update(PIECE_LIFE * 1.4);
        assert!(fx.is_empty(), "{} pieces outlived their life", fx.len());
    }

    /// ⚠️ GRAVITY MUST PULL DEBRIS DOWN, AND "DOWN" IS TOWARD SMALLER y
    /// in this game. A sign error here makes wreckage float upward.
    #[test]
    fn debris_falls() {
        let mut fx = Effects::new();
        fx.explode_lander(500.0, 1000.0, 0.0);
        // Average height, so the test is about the field and not about
        // one piece that happened to be thrown upward.
        let before: f32 =
            fx.pool.particles().iter().map(|p| p.pos.y).sum::<f32>() / LANDER_PIECES as f32;
        for _ in 0..12 {
            fx.update(1.0 / 60.0);
        }
        let after: f32 =
            fx.pool.particles().iter().map(|p| p.pos.y).sum::<f32>() / fx.len().max(1) as f32;
        assert!(after < before, "debris rose: {before} -> {after}");
    }

    #[test]
    fn debris_inherits_the_dead_things_motion() {
        let mut still = Effects::new();
        still.explode_lander(500.0, 300.0, 0.0);
        let still_x: f32 =
            still.pool.particles().iter().map(|p| p.vel.x).sum::<f32>() / LANDER_PIECES as f32;

        let mut moving = Effects::new();
        moving.explode_lander(500.0, 300.0, 400.0);
        let moving_x: f32 =
            moving.pool.particles().iter().map(|p| p.vel.x).sum::<f32>() / LANDER_PIECES as f32;

        assert!(
            moving_x > still_x + 50.0,
            "wreckage did not carry the motion: {still_x} vs {moving_x}"
        );
    }

    #[test]
    fn two_explosions_do_not_throw_identical_debris() {
        let mut fx = Effects::new();
        fx.explode_lander(500.0, 300.0, 0.0);
        let first: Vec<f32> = fx.pool.particles().iter().map(|p| p.vel.x).collect();
        fx.clear();
        fx.explode_lander(500.0, 300.0, 0.0);
        let second: Vec<f32> = fx.pool.particles().iter().map(|p| p.vel.x).collect();
        assert_ne!(first, second, "every explosion would look the same");
    }

    /// The same Effects, fed the same calls, must produce the same
    /// debris — a render has to be reproducible.
    #[test]
    fn explosions_are_deterministic_from_a_fresh_pool() {
        let mut a = Effects::new();
        let mut b = Effects::new();
        a.explode_lander(500.0, 300.0, 0.0);
        b.explode_lander(500.0, 300.0, 0.0);
        let xa: Vec<f32> = a.pool.particles().iter().map(|p| p.vel.x).collect();
        let xb: Vec<f32> = b.pool.particles().iter().map(|p| p.vel.x).collect();
        assert_eq!(xa, xb);
    }

    #[test]
    fn the_pool_never_grows_past_its_capacity() {
        let mut fx = Effects::new();
        for _ in 0..200 {
            fx.explode_lander(500.0, 300.0, 0.0);
        }
        assert!(fx.len() <= CAPACITY, "pool grew to {}", fx.len());
    }

    /// Debris spawned at the seam must still be drawn: its world x is
    /// near WORLD_W while the camera sits near 0, and only the camera's
    /// own wrapping transform connects them.
    #[test]
    fn debris_at_the_seam_is_still_on_screen() {
        let mut fx = Effects::new();
        fx.explode_lander(world::WORLD_W - 2.0, 300.0, 0.0);

        let camera = Camera::new(4.0);
        let drawn = fx
            .pool
            .particles()
            .iter()
            .filter(|p| {
                let sx = camera.to_screen(p.pos.x);
                (-8.0..world::VIEW_W + 8.0).contains(&sx)
            })
            .count();
        assert!(drawn > 0, "debris across the seam vanished from the screen");
    }
}
