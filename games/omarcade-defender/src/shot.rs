//! The laser, and the only thing in the game that travels faster than
//! the ship.
//!
//! Shots live in WORLD space and wrap with it, which is the whole reason
//! this is its own module rather than a Vec in main: a shot fired east
//! near the seam must keep going and come back from the west, and every
//! question anyone asks about it ("has it reached the target?", "is it
//! on screen?") has to be asked through [`world::delta`] rather than by
//! subtracting. In a looping world a raw subtraction is wrong only near
//! the seam, which is the kind of bug that survives every test written
//! at the origin.

use crate::world;

/// How fast a shot travels, in world units per second.
///
/// ★ FASTER THAN THE SHIP'S TOP SPEED, AND DELIBERATELY SO. Defender's
/// laser crosses most of a screen in a blink; a shot that a ship at full
/// throttle could outrun would let you fly into your own fire, which
/// reads as a bug no matter how it is explained.
pub const SHOT_SPEED: f32 = 2400.0;

/// How far a shot travels before it gives up, in world units.
///
/// ⚠️ MEASURED AS DISTANCE, NOT AS TIME ON SCREEN. Killing a shot when
/// it leaves the view would tie weapon range to where the camera happens
/// to be — fire while the camera is still sliding after a reversal and
/// the same shot would have a different range. Distance is a property of
/// the weapon; visibility is a property of the viewer.
///
/// A bit over one screen: long enough to hit something you can see, short
/// enough that the world does not fill with old shots.
pub const SHOT_RANGE: f32 = world::VIEW_W * 1.15;

/// How long the drawn streak is, in world units.
pub const SHOT_LENGTH: f32 = 34.0;

/// The most shots that can be in flight at once.
///
/// ⚠️ A FIXED POOL, NOT A GROWING Vec. Fire is unlimited per Brian's
/// spec, and "unlimited" with an allocating container means a held
/// trigger allocates during play. The cap is high enough that a player
/// cannot reach it at the fire rate below, so it never truncates in
/// practice — it only bounds the memory.
pub const MAX_SHOTS: usize = 24;

/// How many enemy bolts may exist on top of the player's own.
///
/// Separate headroom so a screen full of Mutant fire can never starve
/// the player's gun — running out of ammunition because the enemy is
/// shooting at you would be a maddening bug to diagnose.
pub const MAX_ENEMY_SHOTS: usize = 40;

/// How fast an enemy bolt travels.
///
/// ★ SLOWER THAN YOURS, DELIBERATELY. A Mutant's shot has to be
/// dodgeable on sight; yours has to feel instant. Same speed for both
/// would make one of those two things false.
pub const ENEMY_SHOT_SPEED: f32 = 900.0;

/// The minimum gap between shots, in seconds.
///
/// Unlimited ammunition, not unlimited rate — without this, one frame of
/// a held key would empty the pool and the laser would be a solid beam.
pub const FIRE_INTERVAL: f32 = 0.16;

/// Who fired a bolt.
///
/// ⚠️ A BOLT MUST KNOW WHOSE IT IS, or the collision code has to guess
/// from its direction — and a Mutant firing west at a ship flying west
/// makes that guess wrong. S7 is the first stage where anything shoots
/// back, so this is the first stage where it matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    Player,
    Enemy,
}

/// One laser bolt in flight.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shot {
    /// World x, always wrapped into the world.
    pub x: f32,
    /// Height above the world floor, matching [`crate::flight::Ship::y`].
    pub y: f32,
    /// World units per second, signed: positive is east.
    pub vx: f32,
    /// Vertical speed, positive is UP (world y grows upward).
    ///
    /// ★ ZERO FOR THE PLAYER'S LASER, which is deliberately horizontal
    /// exactly as the arcade's is. It exists for the Mutants, who are
    /// required never to fire level.
    pub vy: f32,
    /// How far this shot still has left to travel.
    pub remaining: f32,
    pub owner: Owner,
}

impl Shot {
    /// Advance by `dt`, and report whether the shot is still alive.
    ///
    /// The shot wraps with the world, so it is never "off the end" —
    /// only ever out of range.
    fn step(&mut self, dt: f32) -> bool {
        let travel = (self.vx * self.vx + self.vy * self.vy).sqrt() * dt;
        self.x = world::wrap(self.x + self.vx * dt);
        self.y += self.vy * dt;
        self.remaining -= travel;
        self.remaining > 0.0
    }

    /// The tail of the drawn streak, behind the head.
    ///
    /// A bolt is drawn as a segment rather than a dot because a dot
    /// moving at 2400 units/second is, at 60 frames, forty units further
    /// along each frame — it reads as a blinking speck rather than as
    /// something travelling.
    pub fn tail(&self) -> f32 {
        world::wrap(self.x - self.vx.signum() * SHOT_LENGTH)
    }

    pub fn is_enemy(&self) -> bool {
        self.owner == Owner::Enemy
    }
}

/// Every shot in flight, and the cooldown between them.
#[derive(Debug, Default)]
pub struct Shots {
    live: Vec<Shot>,
    cooldown: f32,
}

impl Shots {
    pub fn new() -> Self {
        Self { live: Vec::with_capacity(MAX_SHOTS), cooldown: 0.0 }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Shot> {
        self.live.iter()
    }

    pub fn len(&self) -> usize {
        self.live.len()
    }

    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }

    pub fn clear(&mut self) {
        self.live.clear();
        self.cooldown = 0.0;
    }

    /// True when the gun is ready to fire again.
    pub fn ready(&self) -> bool {
        self.cooldown <= 0.0 && self.live.len() < MAX_SHOTS
    }

    /// Fire from `(x, y)` travelling in `direction` (the sign of a
    /// [`crate::flight::Facing`]).
    ///
    /// Does nothing if the gun is still cooling or the pool is full, so
    /// a caller may simply ask on every frame the key is held.
    pub fn fire(&mut self, x: f32, y: f32, direction: f32) -> bool {
        if !self.ready() {
            return false;
        }
        self.live.push(Shot {
            x: world::wrap(x),
            y,
            vx: SHOT_SPEED * direction.signum(),
            vy: 0.0,
            remaining: SHOT_RANGE,
            owner: Owner::Player,
        });
        self.cooldown = FIRE_INTERVAL;
        true
    }

    /// An enemy bolt, travelling along `(dx, dy)`.
    ///
    /// ⚠️ NOT RATE-LIMITED HERE. Each Mutant owns its own cooldown, and
    /// running enemy fire through the PLAYER's gun timer would mean one
    /// Mutant shooting stopped the others — a bug that would look like
    /// the enemies politely taking turns.
    pub fn fire_enemy(&mut self, x: f32, y: f32, dx: f32, dy: f32) {
        if self.live.len() >= MAX_SHOTS + MAX_ENEMY_SHOTS {
            return;
        }
        let len = (dx * dx + dy * dy).sqrt().max(0.001);
        self.live.push(Shot {
            x: world::wrap(x),
            y,
            vx: ENEMY_SHOT_SPEED * dx / len,
            vy: ENEMY_SHOT_SPEED * dy / len,
            remaining: SHOT_RANGE,
            owner: Owner::Enemy,
        });
    }

    /// The first ENEMY bolt overlapping the box at `(x, y)`.
    ///
    /// ⚠️ ENEMY ONLY. Without the owner check your own laser would kill
    /// you the instant it left the muzzle, since it spawns inside your
    /// own hitbox.
    pub fn enemy_hit(&self, x: f32, y: f32, half_w: f32, half_h: f32) -> Option<usize> {
        self.live.iter().position(|s| {
            s.is_enemy()
                && world::delta(s.x, x).abs() <= half_w
                && (s.y - y).abs() <= half_h
        })
    }

    /// Advance every shot and retire the ones that have run out.
    pub fn step(&mut self, dt: f32) {
        self.cooldown = (self.cooldown - dt).max(0.0);
        self.live.retain_mut(|s| s.step(dt));
    }

    /// Remove the shot at `index`, which has hit something.
    pub fn consume(&mut self, index: usize) {
        self.live.swap_remove(index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shot_travels_in_the_direction_it_was_fired() {
        let mut shots = Shots::new();
        assert!(shots.fire(100.0, 300.0, 1.0));
        shots.step(0.1);
        let s = shots.iter().next().unwrap();
        assert!(s.x > 100.0, "an eastward shot must move east, got {}", s.x);

        let mut shots = Shots::new();
        assert!(shots.fire(500.0, 300.0, -1.0));
        shots.step(0.1);
        let s = shots.iter().next().unwrap();
        assert!(s.x < 500.0, "a westward shot must move west, got {}", s.x);
    }

    /// ⚠️ THE SEAM. A shot fired west from just inside the world's start
    /// must reappear at the far end, not stop at zero or go negative.
    #[test]
    fn a_shot_crosses_the_seam() {
        let mut shots = Shots::new();
        shots.fire(10.0, 300.0, -1.0);
        shots.step(0.1); // 240 units west of x=10
        let s = shots.iter().next().unwrap();
        assert!(s.x > world::WORLD_W - 400.0, "should have wrapped, got {}", s.x);
        assert!(s.x < world::WORLD_W, "must stay inside the world, got {}", s.x);
    }

    /// Range is distance travelled, and crossing the seam does not
    /// refund any of it.
    #[test]
    fn range_is_spent_by_travel_not_by_position() {
        let mut shots = Shots::new();
        shots.fire(10.0, 300.0, -1.0);
        // Step in small increments across the seam until it dies.
        let mut travelled = 0.0;
        while !shots.is_empty() {
            shots.step(0.01);
            travelled += SHOT_SPEED * 0.01;
            assert!(travelled < SHOT_RANGE * 1.2, "a shot outlived its range");
        }
        assert!(
            travelled >= SHOT_RANGE * 0.9,
            "a shot died early after {travelled} of {SHOT_RANGE}"
        );
    }

    #[test]
    fn the_gun_cools_between_shots() {
        let mut shots = Shots::new();
        assert!(shots.fire(0.0, 0.0, 1.0), "the first shot must fire");
        assert!(!shots.fire(0.0, 0.0, 1.0), "an immediate second must not");
        shots.step(FIRE_INTERVAL + 0.001);
        assert!(shots.fire(0.0, 0.0, 1.0), "it must fire again once cooled");
    }

    /// Unlimited ammunition must not mean an unbounded pool.
    #[test]
    fn the_pool_is_bounded_however_hard_you_hold_the_trigger() {
        let mut shots = Shots::new();
        for _ in 0..10_000 {
            shots.fire(0.0, 300.0, 1.0);
            shots.step(FIRE_INTERVAL);
        }
        assert!(shots.len() <= MAX_SHOTS, "pool grew to {}", shots.len());
    }

    #[test]
    fn a_shot_expires_at_range() {
        let mut shots = Shots::new();
        shots.fire(0.0, 300.0, 1.0);
        // Just short of the range: still alive.
        shots.step(SHOT_RANGE / SHOT_SPEED * 0.98);
        assert_eq!(shots.len(), 1, "died early");
        shots.step(SHOT_RANGE / SHOT_SPEED * 0.05);
        assert!(shots.is_empty(), "should have expired at range");
    }

    #[test]
    fn the_tail_trails_behind_the_head() {
        let mut shots = Shots::new();
        shots.fire(500.0, 300.0, 1.0);
        let s = *shots.iter().next().unwrap();
        // Eastward: the tail is west of the head.
        assert!(world::delta(s.tail(), s.x) > 0.0, "tail should be behind");

        let mut shots = Shots::new();
        shots.fire(500.0, 300.0, -1.0);
        let s = *shots.iter().next().unwrap();
        assert!(world::delta(s.tail(), s.x) < 0.0, "tail should be behind");
    }
}
