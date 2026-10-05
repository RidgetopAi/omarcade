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

/// How fast the BEAM'S TAIL travels, as a fraction of the head's speed.
///
/// ★★ MEASURED OFF THE ORIGINAL MACHINE, not chosen. Brian supplied four
/// frames; one of them (687x213) happened to catch four player beams at
/// four different ages, stacked. Their spans were 148, 214, 304 and 411
/// pixels — 22%, 31%, 44% and 60% of the frame — and both ENDS moved
/// between them: the head advanced 187, 137, 148 px while the tail
/// advanced 121, 47, 41. The tail travels at about 44% of the head's
/// speed, so the beam STRETCHES as it flies.
///
/// ⇒ THIS IS THE MECHANISM, AND IT IS NOT A LONGER STREAK. Brian put it
/// plainly: "they start on fire shorter and grow in length as they
/// travel out." The old `SHOT_LENGTH` drew a fixed 34-unit tail behind
/// the head, which is 3.5% of [`world::VIEW_W`] and never changed — the
/// arcade's beam reaches 60% and gets there by growing. No value of a
/// fixed length reproduces that, which is why the constant is gone
/// rather than retuned.
///
/// ★ THE EXISTING PHYSICS ALREADY LANDS IT. At 0.45 against
/// [`SHOT_SPEED`] and [`SHOT_RANGE`], a beam reaches 63% of a screen at
/// the end of its life; the reference frames top out at 60%. Neither
/// [`SHOT_SPEED`] nor [`SHOT_RANGE`] needed touching — only the drawing
/// was ever wrong.
pub const SHOT_TAIL_FRACTION: f32 = 0.45;

/// How many distinct colours a beam can be fired in.
///
/// ★ ONE SHOT IS ONE COLOUR, PICKED WHEN IT IS FIRED. Measured across
/// all four reference frames, every beam is dominated by a single hue
/// and different beams differ: one frame is green throughout (856 green
/// pixels against 15 white), another is purple-and-blue (242/151), a
/// third mixes white, green and yellow beams on screen at once.
/// ⇒ Brian read this as cycling — "might start a purpleish color and
/// then change to green" — and the pixels agree, with the refinement
/// that the change happens BETWEEN shots rather than along one beam.
/// Same shape as the Mutant explosion, where colour is chosen per batch.
pub const SHOT_TINTS: u8 = 6;

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
    /// How far this shot HAS travelled, in world units.
    ///
    /// ★ THE BEAM'S LENGTH IS A FUNCTION OF THIS, not of a constant. See
    /// [`SHOT_TAIL_FRACTION`] and [`Shot::beam_len`].
    pub travelled: f32,
    /// Which colour this beam was fired in: `0..SHOT_TINTS`.
    ///
    /// ⚠️ CHOSEN AT FIRE TIME AND THEN FIXED. A beam that changed colour
    /// along its own length would be a different mechanism, and the
    /// reference frames rule it out — each beam is one hue.
    pub tint: u8,
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
        self.travelled += travel;
        self.remaining > 0.0
    }

    /// How long the beam is right now, in world units.
    ///
    /// ★ ZERO AT THE MUZZLE, GROWING AS IT FLIES. The head moves at
    /// [`SHOT_SPEED`] and the tail at [`SHOT_TAIL_FRACTION`] of it, so
    /// the gap between them opens at the difference. That is the whole
    /// formula: distance travelled times the speed difference.
    ///
    /// ⚠️ VISUAL ONLY. Collision tests the HEAD POINT (`main.rs` reads
    /// `(s.x, s.y)`), so a beam that stretches across most of the screen
    /// is not a hitbox that does. `the_beam_is_not_a_hitbox` pins that.
    pub fn beam_len(&self) -> f32 {
        self.travelled * (1.0 - SHOT_TAIL_FRACTION)
    }

    /// The tail of the drawn streak, behind the head.
    ///
    /// A bolt is drawn as a segment rather than a dot because a dot
    /// moving at 2400 units/second is, at 60 frames, forty units further
    /// along each frame — it reads as a blinking speck rather than as
    /// something travelling.
    #[cfg(test)]
    pub fn tail(&self) -> f32 {
        world::wrap(self.x - self.vx.signum() * self.beam_len())
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
    /// Which colour the NEXT player shot will be fired in.
    ///
    /// ★ A ROTATING COUNTER, NOT A RANDOM DRAW, and that is the closer
    /// reading of the hardware: a 1981 board cycling a colour register
    /// steps it, it does not roll dice. It also makes the sequence
    /// reproducible, which is the difference between a test that pins
    /// the behaviour and one that hopes.
    next_tint: u8,
}

impl Shots {
    pub fn new() -> Self {
        Self { live: Vec::with_capacity(MAX_SHOTS), cooldown: 0.0, next_tint: 0 }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Shot> {
        self.live.iter()
    }

    pub fn len(&self) -> usize {
        self.live.len()
    }

    #[cfg(test)]
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
            travelled: 0.0,
            tint: self.next_tint,
            owner: Owner::Player,
        });
        // ★ ADVANCE ONLY ON A SHOT THAT ACTUALLY LEFT THE BARREL, which
        // is why this sits after the early return rather than at the top.
        // Stepping it on refused shots would cycle the colour while the
        // gun was merely cooling.
        self.next_tint = (self.next_tint + 1) % SHOT_TINTS;
        self.cooldown = FIRE_INTERVAL;
        true
    }

    /// An enemy bolt, travelling along `(dx, dy)`.
    ///
    /// ⚠️ NOT RATE-LIMITED HERE. Each Mutant owns its own cooldown, and
    /// running enemy fire through the PLAYER's gun timer would mean one
    /// Mutant shooting stopped the others — a bug that would look like
    /// the enemies politely taking turns.
    /// An enemy bolt at the Mutant's speed. The game fires through
    /// [`fire_enemy_at`](Self::fire_enemy_at); this stages one for the
    /// tests and dump_frame.
    #[allow(dead_code)]
    pub fn fire_enemy(&mut self, x: f32, y: f32, dx: f32, dy: f32) {
        self.fire_enemy_at(x, y, dx, dy, ENEMY_SHOT_SPEED);
    }

    /// An enemy bolt at a speed of the shooter's choosing — a Lander's
    /// is slow enough to step out of, a Mutant's is not.
    pub fn fire_enemy_at(&mut self, x: f32, y: f32, dx: f32, dy: f32, speed: f32) {
        if self.live.len() >= MAX_SHOTS + MAX_ENEMY_SHOTS {
            return;
        }
        let len = (dx * dx + dy * dy).sqrt().max(0.001);
        self.live.push(Shot {
            x: world::wrap(x),
            y,
            vx: speed * dx / len,
            vy: speed * dy / len,
            remaining: SHOT_RANGE,
            travelled: 0.0,
            // ⚠️ ENEMY BOLTS DO NOT CYCLE. Threat colour is game
            // information and is held fixed on purpose (L065) — a bolt
            // that might arrive in any colour is a bolt the eye has to
            // identify before it can dodge.
            tint: 0,
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

    /// Take an already-built [`Shot`] into this set, bypassing the gun.
    ///
    /// ⚠️ FOR DIAGNOSTIC SCENES ONLY — `examples/dump_frame.rs` uses it
    /// to place beams at chosen ages, which `fire` cannot do because it
    /// is rate-limited and `step` ages the cooldown and the shots
    /// together. Nothing in the game calls this; the gun is the only way
    /// a shot enters play.
    #[allow(dead_code)] // dump_frame's `laser` scene stages beams with it
    pub fn adopt(&mut self, shot: Shot) {
        self.live.push(shot);
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
        // ⚠️ STEPPED FIRST, AND THAT IS NOT A WORKAROUND. A beam now has
        // ZERO length at the muzzle and grows as it flies, so at fire
        // time the tail sits exactly ON the head and neither direction
        // is "behind". This test used to pass without stepping because
        // the old fixed `SHOT_LENGTH` made a shot born full-size.
        let mut shots = Shots::new();
        shots.fire(500.0, 300.0, 1.0);
        shots.step(0.05);
        let s = *shots.iter().next().unwrap();
        // Eastward: the tail is west of the head.
        assert!(world::delta(s.tail(), s.x) > 0.0, "tail should be behind");

        let mut shots = Shots::new();
        shots.fire(500.0, 300.0, -1.0);
        shots.step(0.05);
        let s = *shots.iter().next().unwrap();
        assert!(world::delta(s.tail(), s.x) < 0.0, "tail should be behind");
    }

    /// ★★ THE MECHANISM BRIAN DESCRIBED: "they start on fire shorter and
    /// grow in length as they travel out."
    #[test]
    fn the_beam_grows_as_it_travels() {
        let mut shots = Shots::new();
        shots.fire(0.0, 300.0, 1.0);

        // Born with no length at all — the beam leaves the muzzle as a
        // point, which is why `the_tail_trails_behind_the_head` has to
        // step before it can ask which end is which.
        let born = shots.iter().next().unwrap().beam_len();
        assert!(born < 1.0, "a beam should leave the muzzle as a point, got {born}");

        let mut last = born;
        for _ in 0..8 {
            shots.step(0.02);
            let Some(s) = shots.iter().next() else { break };
            let now = s.beam_len();
            assert!(now > last, "beam shrank: {last} -> {now}");
            last = now;
        }

        // ★ AND IT REACHES THE SIZE THE ORIGINAL DOES. The reference
        // frames top out at 60% of a screen; the physics lands at ~63%
        // without either SHOT_SPEED or SHOT_RANGE being touched.
        let mut shots = Shots::new();
        shots.fire(0.0, 300.0, 1.0);
        shots.step(SHOT_RANGE / SHOT_SPEED * 0.98);
        let s = shots.iter().next().expect("should still be alive");
        let frac = s.beam_len() / world::VIEW_W;
        assert!(
            (0.55..=0.70).contains(&frac),
            "a full-grown beam spans {:.0}% of a screen; the reference is ~60%",
            frac * 100.0
        );
    }

    /// ★★ SUCCESSIVE SHOTS ARE DIFFERENT COLOURS — the cycling Brian
    /// read off the machine ("might start a purpleish color and then
    /// change to green"), which the pixels refined to: the change
    /// happens BETWEEN shots, not along one beam.
    #[test]
    fn successive_shots_cycle_colour() {
        let mut shots = Shots::new();
        let mut seen = Vec::new();
        for _ in 0..SHOT_TINTS {
            shots.fire(0.0, 300.0, 1.0);
            seen.push(shots.iter().last().unwrap().tint);
            // Clear the cooldown without ageing anything into oblivion.
            shots.step(FIRE_INTERVAL);
        }
        let mut sorted = seen.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            SHOT_TINTS as usize,
            "{SHOT_TINTS} shots produced {} distinct colours: {seen:?}",
            sorted.len()
        );

        // ⚠️ AND IT WRAPS rather than running off the end of the palette.
        shots.fire(0.0, 300.0, 1.0);
        let wrapped = shots.iter().last().unwrap().tint;
        assert_eq!(wrapped, seen[0], "the cycle did not wrap");
    }

    /// ⚠️ A REFUSED SHOT MUST NOT ADVANCE THE COLOUR. The gun is asked on
    /// every frame the key is held and says no most of them; cycling on
    /// the refusals would race the colour forward while nothing fired.
    #[test]
    fn a_refused_shot_does_not_cycle_colour() {
        let mut shots = Shots::new();
        shots.fire(0.0, 300.0, 1.0);
        let first = shots.iter().last().unwrap().tint;
        // Still cooling: these all refuse.
        for _ in 0..10 {
            assert!(!shots.fire(0.0, 300.0, 1.0), "fired while cooling");
        }
        shots.step(FIRE_INTERVAL);
        shots.fire(0.0, 300.0, 1.0);
        let second = shots.iter().last().unwrap().tint;
        assert_eq!(
            second,
            (first + 1) % SHOT_TINTS,
            "ten refused shots moved the colour from {first} to {second}"
        );
    }

    /// ⚠️ THE BEAM IS A PICTURE, NOT A WEAPON. It now stretches across
    /// most of a screen, and if that length ever reached the collision
    /// path a shot would kill things it visibly flew past — including
    /// Humanoids, where the punishment is losing the rescue.
    ///
    /// Collision reads the HEAD POINT (`main.rs` passes `(s.x, s.y)` to
    /// the hit tests). This pins that the two stay separate.
    #[test]
    fn the_beam_is_not_a_hitbox() {
        let mut shots = Shots::new();
        shots.fire(0.0, 300.0, 1.0);
        shots.step(0.2);
        let s = *shots.iter().next().unwrap();

        // A long beam behind the head...
        assert!(s.beam_len() > 200.0, "expected a grown beam, got {}", s.beam_len());

        // ...and a target sitting squarely inside it, well behind the
        // head, is NOT hit: `enemy_hit` and the player hit path both ask
        // about the head only.
        let mut enemy = Shots::new();
        enemy.fire_enemy(0.0, 300.0, 1.0, 0.0);
        enemy.step(0.4);
        let e = *enemy.iter().next().unwrap();
        let behind = world::wrap(e.x - e.vx.signum() * e.beam_len() * 0.5);
        assert!(
            enemy.enemy_hit(behind, 300.0, 4.0, 4.0).is_none(),
            "a target inside the beam but behind the head was hit — \
             the drawn length has reached collision"
        );
    }
}
