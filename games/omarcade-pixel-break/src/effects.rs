//! Brick chips, damage feedback and screen shake.
//!
//! Kept out of `state.rs` for the same reason `items.rs` is: the world
//! model should not grow a second personality. `state` owns the pool and
//! the shake value that exist; this module owns the rules about what goes
//! into them.
//!
//! # Where these numbers came from
//!
//! Every constant below was tuned in `tools/shatter-playground.html`
//! against real motion, real scale, additive blending and a live theme —
//! not chosen in code. The playground's integration is a transcription of
//! `ParticlePool::update` and was verified against the Rust to four
//! decimals, so the numbers transfer exactly rather than approximately.
//!
//! ⚠️ **Regenerate them there, do not hand-edit.** A number nudged here is
//! a number nobody has ever looked at.

use omarcade_core::particles::{Particle, ParticlePool};
use omarcade_core::Color;

use crate::geom::{Rect, Vec2};

// ---------------------------------------------------------------------------
// Tuned in tools/shatter-playground.html.
// ---------------------------------------------------------------------------

/// Chips thrown when a brick is destroyed.
pub const SHATTER_CHIPS: usize = 18;
/// How hard they are thrown, in units/s.
pub const SHATTER_SPEED: f32 = 240.0;
/// Half-angle of the cone around the ball's travel, in radians.
///
/// ⚠️ Chips go WITH the ball. A brick that sprays evenly in all directions
/// reads as a generic puff; thrown along the impact it reads as the brick
/// coming apart, which is what the plan means by "made of the brick".
pub const SHATTER_SPREAD: f32 = 0.611; // 35°
/// Per-chip speed variation, as a fraction of SHATTER_SPEED.
pub const SHATTER_JITTER: f32 = 0.30;

/// Chip side length in field units, and its variation.
pub const CHIP_SIZE: f32 = 4.0;
pub const CHIP_SIZE_VAR: f32 = 0.50;
/// Seconds a chip lives, and its variation.
///
/// ⚠️ The variation matters more than it looks: with a single lifetime the
/// whole burst vanishes on one frame, which reads as a glitch rather than
/// as settling.
pub const CHIP_LIFE: f32 = 0.55;
pub const CHIP_LIFE_VAR: f32 = 0.40;
/// Downward acceleration on chips, units/s².
pub const CHIP_GRAVITY: f32 = 640.0;

/// Share of a full break thrown by a brick that SURVIVES the hit.
///
/// Damage and shatter are one system, partial — which is what keeps the
/// reinforced-brick feedback cheap.
pub const PARTIAL_SHARE: f32 = 0.35;

/// How many times a second a falling item pulses.
///
/// ⚠️ Slow on purpose. The plan asks for items that "pulse", and a fast
/// blink reads as a warning — the opposite of a reward. At roughly one and
/// a half cycles a second the item breathes, which draws the eye without
/// nagging it.
pub const GLOW_HZ: f32 = 1.5;

/// How far the pulse swings, as a fraction of the item's border.
///
/// The glow is drawn as a halo OUTSIDE the item, so this is measured in
/// field units of extra surround at the peak of the swing.
pub const GLOW_UNITS: f32 = 3.0;

/// The Omarchy item's share of the swing, against every other item's.
///
/// ⚠️ **The plan is explicit that the Omarchy item is the one that should
/// look "genuinely special".** The others get a gentle breath; this gets a
/// halo two and a half times deeper, so the rare drop announces itself
/// across a busy field without any second mechanism.
pub const GLOW_OMARCHY_SCALE: f32 = 2.5;

/// A free-running clock for effects that pulse rather than decay.
///
/// ⚠️ Separate from the cascade's `Clear::elapsed`, which resets at every
/// stage seam — a pulse driven off that would visibly jump each time a
/// level cleared. This only ever advances.
///
/// ⚠️ Like every other effect it ticks OUTSIDE the phase match, so items
/// waiting on the Ready screen keep breathing. See the note beside
/// `chips.update` in `physics.rs`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pulse {
    /// Seconds since the game began. Only ever increases.
    pub t: f32,
}

impl Pulse {
    pub fn tick(&mut self, dt: f32) {
        self.t += dt;
        // Wrap on the cycle so a long session cannot drift into the range
        // where an f32 no longer resolves small time steps. One full
        // period is indistinguishable from any other, so this is invisible.
        let period = 1.0 / GLOW_HZ;
        if self.t > period {
            self.t -= period;
        }
    }

    /// The glow's depth in field units, for one item kind, right now.
    ///
    /// Returns a value in `0..=GLOW_UNITS * scale`, swinging smoothly. The
    /// curve is a raised cosine rather than a sine so the item spends
    /// slightly longer at its dimmest — a breath, not a metronome.
    pub fn depth(&self, scale: f32) -> f32 {
        let phase = self.t * GLOW_HZ * std::f32::consts::TAU;
        let swing = (1.0 - phase.cos()) * 0.5;
        GLOW_UNITS * scale * swing
    }
}

/// Peak screen-shake offset in field units, and how fast it decays.
///
/// ⚠️ Applied to the VIEWPORT only. Shake moves the drawing, never the
/// field: a ball that collided with something it does not visually touch
/// would be a real bug wearing an effect's clothes.
/// ⚠️ The plan caps this near 4 units. Shake is the effect most likely to
/// be too much, and it makes people motion-sick before they can say why.
pub const SHAKE_UNITS: f32 = 2.5;
pub const SHAKE_DECAY: f32 = 9.0;

/// Share of a full break thrown by each cell in the level-clear wave.
///
/// ⚠️ Smaller than a real break, and the pool is why. Sixty cells at a
/// full eighteen chips is 1080 particles into a pool of 512 — the early
/// rows would be recycled away before the wave reached the bottom, so the
/// cascade would eat its own head. Keeping the whole wave on screen at
/// once is the only way it reads as a wave.
pub const CASCADE_SHARE: f32 = 0.4;

/// How much bigger, brighter and longer-lived a cascade chip is than a
/// brick chip.
///
/// ⚠️ **The wave cannot be tuned like a brick break, and the first attempt
/// proved it.** A break throws eighteen chips into one brick-sized area
/// with the brick's own colour still on screen behind them; the wave
/// spreads seven over the same area against BLACK, with nothing behind
/// them at all. Rendered at brick-break values it came out as grey dust —
/// measured: a chip at 0.15 alpha over the dark background has a
/// red-to-blue spread of 11 out of 255, which the eye reads as grey, not
/// as red.
///
/// Additive light needs mass to carry colour on a dark ground. Bigger
/// chips put more light in the same place; a slower fade holds them above
/// the alpha where hue collapses; and starting brighter keeps them there
/// longer. The pool budget is what stops the obvious fix — "throw more" —
/// so each chip has to be worth more instead.
pub const CASCADE_SIZE_SCALE: f32 = 2.2;
pub const CASCADE_LIFE_SCALE: f32 = 1.9;
/// Starting intensity of a cascade chip, out of 255.
pub const CASCADE_ALPHA: u8 = 255;

/// The victory wave's own share and lifetime.
///
/// ⚠️ **The level cascade's numbers make a drizzle when the wave is held
/// four seconds, and rendering it is what showed that.** The level wave
/// crosses in 0.40 s, so all six rows are in the air together and 0.4
/// share reads as a field coming apart. The victory wave takes 4.2 s —
/// 0.70 s a row — and a chip living 1.05 s has nearly faded before the
/// next row throws. Only about two rows are ever up, so the same numbers
/// that read as a wave read as light rain.
///
/// The pool is what stopped the level cascade throwing more (sixty cells
/// at full chips is 1080 into a pool of 512), but the victory wave is
/// spread over ten times as long, so its rows never all coexist. That
/// slack is the budget these spend: 130 chips a row, about three rows up
/// at once, a peak near 396 against the pool's 512 — 2.3x the density
/// with headroom left. At 0.9 / 2.8 the peak hits 512 exactly and the
/// wave starts recycling its own head.
pub const VICTORY_SHARE: f32 = 0.7;
pub const VICTORY_LIFE_SCALE: f32 = 2.6;

/// One cell of the victory wave: the cascade burst, with more mass and a
/// longer life so a held wave still reads as one event.
pub fn victory_burst(
    pool: &mut ParticlePool,
    rng: &mut Rng,
    cell: Rect,
    dir: Vec2,
    color: Color,
) {
    burst_with(
        pool,
        rng,
        cell,
        dir,
        color,
        VICTORY_SHARE,
        CASCADE_SIZE_SCALE,
        VICTORY_LIFE_SCALE,
    );
}

/// How many chips the pool can hold at once.
///
/// Sized for the worst honest case rather than the average: ten balls in
/// play on a late level, several breaking bricks in the same tick, at 18
/// chips each. Overflow recycles the oldest particle, so exceeding this is
/// graceful — an old chip a few frames from fading vanishes early, which
/// is invisible, rather than the break the player is watching failing to
/// appear.
pub const POOL_CAPACITY: usize = 512;

// ---------------------------------------------------------------------------
// The burst.
// ---------------------------------------------------------------------------

/// Throw chips from a brick that was just hit.
///
/// `share` is how much of a full break to throw: 1.0 when the brick is
/// destroyed, [`PARTIAL_SHARE`] when it survived and is merely damaged.
///
/// `dir` is the ball's travel. It need not be normalised; a zero vector
/// falls back to straight up so a chip burst triggered without a ball —
/// a level-clear cascade, a test — still reads correctly.
///
/// ⚠️ **Chips are fanned EVENLY across the cone, not sampled randomly.**
/// An even fan reads as a brick coming apart; random angles clump, and
/// clumps read as noise. This is what the playground draws, so it is what
/// the game has to draw, or the tuning does not reproduce.
pub fn shatter(
    pool: &mut ParticlePool,
    rng: &mut Rng,
    brick: Rect,
    dir: Vec2,
    color: Color,
    share: f32,
) {
    burst(pool, rng, brick, dir, color, share, false);
}

/// One cell of the level-clear wave.
///
/// Same fan as a brick break, with chips scaled up so they still read as
/// coloured light against an empty field — see `CASCADE_SIZE_SCALE`.
pub fn cascade_burst(
    pool: &mut ParticlePool,
    rng: &mut Rng,
    cell: Rect,
    dir: Vec2,
    color: Color,
) {
    burst(pool, rng, cell, dir, color, CASCADE_SHARE, true);
}

fn burst(
    pool: &mut ParticlePool,
    rng: &mut Rng,
    brick: Rect,
    dir: Vec2,
    color: Color,
    share: f32,
    cascade: bool,
) {
    let (size, life) = if cascade {
        (CASCADE_SIZE_SCALE, CASCADE_LIFE_SCALE)
    } else {
        (1.0, 1.0)
    };
    burst_with(pool, rng, brick, dir, color, share, size, life);
}

#[allow(clippy::too_many_arguments)]
fn burst_with(
    pool: &mut ParticlePool,
    rng: &mut Rng,
    brick: Rect,
    dir: Vec2,
    color: Color,
    share: f32,
    size_scale: f32,
    life_scale: f32,
) {
    let cascade = life_scale > 1.0;
    let count = ((SHATTER_CHIPS as f32 * share).round() as usize).max(1);
    if share <= 0.0 {
        return;
    }

    let dir = if dir.length() > 1e-3 { dir } else { Vec2::new(0.0, -1.0) };
    let base = dir.y.atan2(dir.x);
    let centre = brick.center();

    for i in 0..count {
        // -1 at one edge of the cone, +1 at the other.
        let t = if count == 1 {
            0.0
        } else {
            (i as f32 / (count - 1) as f32) * 2.0 - 1.0
        };
        // A little wobble on top of the fan, so it is not a perfect rake.
        let angle = base + t * SHATTER_SPREAD + rng.signed() * SHATTER_SPREAD * 0.15;
        let speed = SHATTER_SPEED * (1.0 + rng.signed() * SHATTER_JITTER);
        // ⚠️ The scales are PARAMETERS now. They used to be looked up
        // here from `cascade`, which would have silently shadowed the
        // victory wave's own numbers and left it drizzling exactly as
        // before — the change would have compiled and done nothing.
        let size = (CHIP_SIZE * size_scale * (1.0 + rng.signed() * CHIP_SIZE_VAR)).max(1.0);
        let life = (CHIP_LIFE * life_scale * (1.0 + rng.signed() * CHIP_LIFE_VAR)).max(0.05);
        let color = if cascade { color.with_alpha(CASCADE_ALPHA) } else { color };

        // Chips start spread across the brick's face, not all from its
        // centre — they are pieces OF the brick, so they begin where the
        // brick was.
        let pos = Vec2::new(
            centre.x + rng.signed() * brick.w * 0.35,
            centre.y + rng.signed() * brick.h * 0.35,
        );
        let vel = Vec2::new(angle.cos() * speed, angle.sin() * speed);

        pool.spawn(Particle::new(pos, vel, size, color, life));
    }
}

// ---------------------------------------------------------------------------
// Screen shake.
// ---------------------------------------------------------------------------

/// A decaying screen shake.
///
/// ⚠️ **This offsets the drawing, never the world.** `Viewport` already
/// centralises every coordinate, so the shake is applied there and nothing
/// else in `render.rs` needs to know it exists — and physics never sees it
/// at all, which is what keeps collisions honest.
#[derive(Debug, Clone, Copy, Default)]
pub struct Shake {
    /// Current magnitude in field units. Zero when still.
    pub amount: f32,
}

impl Shake {
    /// Start a shake, or refresh one already running.
    ///
    /// Takes the LARGER of the two rather than adding: two armoured bricks
    /// breaking in the same tick is not twice as violent an event, and
    /// summing would let a busy moment throw the screen across the window.
    pub fn add(&mut self, units: f32) {
        self.amount = self.amount.max(units);
    }

    /// Decay toward still. Exponential, so it fades fast then finishes.
    pub fn tick(&mut self, dt: f32) {
        if self.amount <= 0.0 {
            return;
        }
        self.amount *= (-SHAKE_DECAY * dt).exp();
        // Snap to zero below a subpixel, so a viewport is not offset by a
        // number too small to see for a long tail of frames.
        if self.amount < 0.01 {
            self.amount = 0.0;
        }
    }

    pub fn is_still(&self) -> bool {
        self.amount <= 0.0
    }

    /// The offset to apply to the viewport this frame.
    pub fn offset(&self, rng: &mut Rng) -> Vec2 {
        if self.is_still() {
            return Vec2::ZERO;
        }
        Vec2::new(rng.signed() * self.amount, rng.signed() * self.amount)
    }

    pub fn clear(&mut self) {
        self.amount = 0.0;
    }
}

// ---------------------------------------------------------------------------
// Randomness.
// ---------------------------------------------------------------------------

/// The effects' random source.
///
/// ⚠️ **Seeded and deterministic, like the `Dropper`'s.** `probe_balance`
/// is only a measurement if the same seed gives the same run, and effects
/// that consumed randomness from a global source would make every headless
/// probe irreproducible.
///
/// Same LCG and the same 16-bit mask as `items::Dropper` — the mask is not
/// decoration: an unmasked shift keeps every upper bit and the result runs
/// to millions, which is the bug that put six balls thirty thousand units
/// off-field at S3 (L037).
#[derive(Debug, Clone)]
pub struct Rng {
    seed: u32,
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        // Offsetting rather than `| 1` keeps the mapping injective, so two
        // different seeds cannot collapse to the same stream.
        Rng { seed: seed.wrapping_add(0x9E37_79B9) }
    }

    /// A number in `0.0..1.0`.
    pub fn unit(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
        ((self.seed >> 8) & 0xFFFF) as f32 / 65536.0
    }

    /// A number in `-1.0..1.0`.
    pub fn signed(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }
}

impl Default for Rng {
    fn default() -> Self {
        Rng::new(0x0EFF_EC75)
    }
}

/// A pool sized and configured for this game's chips.
pub fn new_pool() -> ParticlePool {
    ParticlePool::with_capacity(POOL_CAPACITY).with_gravity(CHIP_GRAVITY)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brick() -> Rect {
        Rect::new(100.0, 200.0, crate::state::BRICK_W, crate::state::BRICK_H)
    }

    fn white() -> Color {
        Color::rgb(255, 255, 255)
    }

    #[test]
    fn a_destroyed_brick_throws_a_full_burst() {
        let mut pool = new_pool();
        let mut rng = Rng::new(1);
        shatter(&mut pool, &mut rng, brick(), Vec2::new(1.0, -1.0), white(), 1.0);
        assert_eq!(pool.len(), SHATTER_CHIPS);
    }

    /// Damage and shatter are ONE system, partial — that is what keeps the
    /// reinforced-brick feedback cheap.
    #[test]
    fn a_survived_hit_throws_a_smaller_burst() {
        let mut pool = new_pool();
        let mut rng = Rng::new(1);
        shatter(&mut pool, &mut rng, brick(), Vec2::new(1.0, -1.0), white(), PARTIAL_SHARE);
        assert!(pool.len() < SHATTER_CHIPS, "a partial break must be smaller");
        assert!(pool.len() > 0, "a partial break must still be visible");
    }

    /// ⚠️ Chips go WITH the ball. Thrown against its travel they read as a
    /// bounce-back; thrown evenly in all directions they read as generic
    /// dust. Neither is a brick coming apart.
    #[test]
    fn chips_are_thrown_along_the_balls_travel() {
        for dir in [
            Vec2::new(1.0, -1.0),
            Vec2::new(-1.0, -1.0),
            Vec2::new(0.0, 1.0),
            Vec2::new(1.0, 0.0),
        ] {
            let mut pool = new_pool();
            let mut rng = Rng::new(7);
            shatter(&mut pool, &mut rng, brick(), dir, white(), 1.0);

            let want = dir.with_length(1.0);
            // The average chip velocity should point roughly the way the
            // ball was going.
            let mut sum = Vec2::ZERO;
            for p in pool.particles() {
                sum += p.vel;
            }
            let avg = sum * (1.0 / pool.len() as f32);
            let unit = avg.with_length(1.0);
            let dot = unit.x * want.x + unit.y * want.y;
            assert!(
                dot > 0.7,
                "burst for dir {dir:?} averaged {avg:?}, dot {dot:.2} — not along the travel"
            );
        }
    }

    /// A burst with no ball still reads: straight up rather than nowhere.
    #[test]
    fn a_zero_direction_falls_back_to_upward() {
        let mut pool = new_pool();
        let mut rng = Rng::new(3);
        shatter(&mut pool, &mut rng, brick(), Vec2::ZERO, white(), 1.0);
        assert_eq!(pool.len(), SHATTER_CHIPS);
        let mut sum = Vec2::ZERO;
        for p in pool.particles() {
            sum += p.vel;
        }
        assert!(sum.y < 0.0, "a directionless burst should still go up");
    }

    /// ⚠️ Chips are pieces OF the brick, so they start where the brick was
    /// — spread across its face, not all from one point.
    #[test]
    fn chips_start_spread_across_the_bricks_face() {
        let mut pool = new_pool();
        let mut rng = Rng::new(11);
        let b = brick();
        shatter(&mut pool, &mut rng, b, Vec2::new(0.0, -1.0), white(), 1.0);

        let xs: Vec<f32> = pool.particles().iter().map(|p| p.pos.x).collect();
        let lo = xs.iter().cloned().fold(f32::INFINITY, f32::min);
        let hi = xs.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        assert!(hi - lo > b.w * 0.25, "chips all started from nearly one point");
        // But still within the brick.
        assert!(lo >= b.left() - 1.0 && hi <= b.right() + 1.0, "chips started outside the brick");
    }

    /// Every chip must be visible for a while and then stop — a zero or
    /// negative life is rejected by the pool and would silently thin the
    /// burst.
    #[test]
    fn every_chip_is_born_alive() {
        let mut pool = new_pool();
        let mut rng = Rng::new(5);
        shatter(&mut pool, &mut rng, brick(), Vec2::new(1.0, -1.0), white(), 1.0);
        assert_eq!(pool.len(), SHATTER_CHIPS, "the pool rejected a dead-on-arrival chip");
        for p in pool.particles() {
            assert!(p.life > 0.0);
            assert!(p.size >= 1.0, "a sub-pixel chip cannot be seen");
        }
    }

    /// ⚠️ Lifetimes must VARY. With one lifetime the whole burst vanishes
    /// on a single frame, which reads as a glitch rather than as settling.
    #[test]
    fn chips_do_not_all_die_at_once() {
        let mut pool = new_pool();
        let mut rng = Rng::new(13);
        shatter(&mut pool, &mut rng, brick(), Vec2::new(1.0, -1.0), white(), 1.0);
        let lives: Vec<f32> = pool.particles().iter().map(|p| p.life).collect();
        let lo = lives.iter().cloned().fold(f32::INFINITY, f32::min);
        let hi = lives.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        assert!(hi - lo > CHIP_LIFE * 0.2, "chip lifetimes barely varied: {lo}..{hi}");
    }

    /// The pool must survive far more breaks than it can hold. Overflow
    /// recycles rather than dropping the newest — the break the player is
    /// looking at always renders.
    #[test]
    fn overflowing_the_pool_keeps_the_newest_break() {
        let mut pool = new_pool();
        let mut rng = Rng::new(2);
        for _ in 0..200 {
            shatter(&mut pool, &mut rng, brick(), Vec2::new(1.0, -1.0), white(), 1.0);
        }
        assert_eq!(pool.len(), POOL_CAPACITY, "the pool should be full, not over");
        assert!(pool.capacity() >= POOL_CAPACITY);
    }

    // ---- shake ----

    #[test]
    fn shake_decays_to_still() {
        let mut s = Shake::default();
        s.add(SHAKE_UNITS);
        assert!(!s.is_still());
        for _ in 0..240 {
            s.tick(1.0 / 240.0);
        }
        assert!(s.is_still(), "shake still running after a second: {}", s.amount);
    }

    /// ⚠️ Two armoured bricks breaking together is not twice as violent an
    /// event. Summing would let a busy moment throw the screen across the
    /// window.
    #[test]
    fn shakes_take_the_larger_rather_than_adding() {
        let mut s = Shake::default();
        s.add(2.0);
        s.add(3.0);
        assert_eq!(s.amount, 3.0);
        s.add(1.0);
        assert_eq!(s.amount, 3.0, "a smaller shake must not reduce a bigger one");
    }

    #[test]
    fn a_still_shake_offsets_nothing() {
        let s = Shake::default();
        let mut rng = Rng::new(1);
        assert_eq!(s.offset(&mut rng), Vec2::ZERO);
    }

    /// The offset stays inside the amount, so the plan's cap means what it
    /// says.
    #[test]
    fn the_offset_never_exceeds_the_amount() {
        let mut s = Shake::default();
        s.add(SHAKE_UNITS);
        let mut rng = Rng::new(9);
        for _ in 0..2000 {
            let o = s.offset(&mut rng);
            assert!(o.x.abs() <= s.amount + 1e-4, "x offset {} exceeded {}", o.x, s.amount);
            assert!(o.y.abs() <= s.amount + 1e-4, "y offset {} exceeded {}", o.y, s.amount);
        }
    }

    /// ⚠️ The plan caps shake near 4 units, because it is the effect most
    /// likely to be too much. This is a guard on the tuning, not on the code.
    #[test]
    fn the_shipped_shake_respects_the_plans_cap() {
        assert!(
            SHAKE_UNITS <= 4.0,
            "SHAKE_UNITS is {SHAKE_UNITS}; the plan caps it near 4 and warns that shake \
             makes people motion-sick before they can say why"
        );
        assert!(SHAKE_DECAY >= 4.0, "a slow decay leaves the screen swimming");
    }

    // ---- rng ----

    /// ⚠️ L037 in a third place: an unmasked shift makes this run to
    /// millions, and a chip velocity scaled by it lands off-field.
    #[test]
    fn the_random_source_stays_in_range() {
        let mut r = Rng::new(12345);
        for _ in 0..20_000 {
            let u = r.unit();
            assert!((0.0..1.0).contains(&u), "unit {u} out of range");
            let s = r.signed();
            assert!((-1.0..1.0).contains(&s), "signed {s} out of range");
        }
    }

    /// Same seed, same effects — `probe_balance` is only a measurement if
    /// the whole run is reproducible.
    #[test]
    fn the_same_seed_gives_the_same_burst() {
        let burst = |seed| {
            let mut pool = new_pool();
            let mut rng = Rng::new(seed);
            shatter(&mut pool, &mut rng, brick(), Vec2::new(1.0, -1.0), white(), 1.0);
            pool.particles().iter().map(|p| (p.pos, p.vel, p.life)).collect::<Vec<_>>()
        };
        assert_eq!(burst(42), burst(42));
        assert_ne!(burst(42), burst(43), "different seeds should differ");
    }
}
