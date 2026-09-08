//! A fixed-capacity particle pool.
//!
//! Pre-allocated once, never grown, never allocating per frame. A game
//! constructs one with the budget it wants and spends it; the pool itself
//! sets no policy about how many particles a game should have, because core
//! has no way to know.
//!
//! The pool stores plain data and does two things to it: [`ParticlePool::update`]
//! advances every live particle, [`ParticlePool::draw`] adds them to a canvas
//! as light. Everything about how a particle *looks* — colour, size, how fast
//! it fades — is the caller's, which is what keeps effects theme-reactive:
//! a hardcoded colour in here would be a colour no theme could reach.
//!
//! # Why additive
//!
//! Particles draw through [`Canvas::fill_rect_add_f`], not `fill_rect_f`.
//! Sparks and chips are light, and overlapping ones should get brighter
//! rather than mixing toward a flat colour. The sub-pixel path is the one
//! that matters here: particles move continuously and snapping them to whole
//! pixels turns a smooth arc into a stutter.
//!
//! # Why squares
//!
//! There is no circle primitive, deliberately. In a pixel-art register a
//! small square reads as correct, and the first effect this pool serves is
//! brick shatter — where the chips are fragments of a rectangular brick and
//! being square is not an approximation at all. If a later effect genuinely
//! wants round sparks, that is the moment to add a shape primitive and judge
//! it against something real.

use crate::backend::{Canvas, Color};
use crate::geom::Vec2;

/// One particle. Plain data with no behaviour, `Copy` so the pool can move
/// them around without ceremony.
///
/// `age` and `life` are both seconds. A particle is alive while
/// `age < life`; [`ParticlePool::update`] retires it the moment it is not.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Particle {
    pub pos: Vec2,
    pub vel: Vec2,
    /// Side length in play-field units. Square; see the module docs.
    pub size: f32,
    /// Colour at full intensity. Alpha is the starting intensity, and
    /// [`Particle::fade`] scales it down over the particle's life.
    pub color: Color,
    /// How long this particle lives, in seconds.
    pub life: f32,
    /// How long it has lived so far, in seconds.
    pub age: f32,
}

impl Particle {
    /// A particle at `pos` moving at `vel`, living for `life` seconds.
    pub fn new(pos: Vec2, vel: Vec2, size: f32, color: Color, life: f32) -> Self {
        Particle { pos, vel, size, color, life, age: 0.0 }
    }

    /// How far through its life this particle is, in `0.0..=1.0`.
    ///
    /// A particle with a non-positive `life` reads as fully spent rather
    /// than dividing by zero — that way a caller passing `life: 0.0` gets a
    /// particle that never draws, instead of a NaN that propagates into the
    /// canvas.
    pub fn progress(&self) -> f32 {
        if self.life <= 0.0 {
            return 1.0;
        }
        (self.age / self.life).clamp(0.0, 1.0)
    }

    pub fn is_alive(&self) -> bool {
        self.age < self.life
    }

    /// The colour to draw this particle at right now: its own colour with
    /// intensity scaled by how much life is left, so it fades out linearly.
    pub fn fade(&self) -> Color {
        let left = 1.0 - self.progress();
        let a = (self.color.a as f32 * left).round().clamp(0.0, 255.0) as u8;
        self.color.with_alpha(a)
    }
}

/// A fixed-capacity pool of particles.
///
/// Allocates its whole budget at construction and never allocates again.
/// Live particles occupy `0..len`; the rest of the backing store is stale
/// data that is never read.
///
/// # Overflow
///
/// Spawning into a full pool **recycles the oldest particle**. The effect
/// you just triggered always renders — at ten balls in play a level-clear
/// cascade will hit any sane cap, and the break that just happened is the
/// one the player is looking at. An old chip a few frames from fading out
/// disappearing early is invisible; the new break not appearing at all is
/// not.
#[derive(Debug, Clone)]
pub struct ParticlePool {
    particles: Vec<Particle>,
    /// Live count. Particles at `..len` are live, `len..` is stale storage.
    len: usize,
    /// Applied to every particle's velocity each update, in units/s².
    ///
    /// On the pool rather than on each particle: every effect planned wants
    /// the same gravity, and per-particle storage would spend four bytes
    /// each to say the same number. Defaults to zero so a pool used for
    /// sparks is not forced into an arc it never asked for.
    pub gravity: f32,
}

impl ParticlePool {
    /// A pool that can hold `capacity` particles at once.
    ///
    /// The whole backing store is allocated here and never resized, so this
    /// is the only place a particle pool touches the allocator.
    pub fn with_capacity(capacity: usize) -> Self {
        ParticlePool {
            particles: Vec::with_capacity(capacity),
            len: 0,
            gravity: 0.0,
        }
    }

    /// Downward acceleration in units/s², applied every update.
    pub fn with_gravity(mut self, gravity: f32) -> Self {
        self.gravity = gravity;
        self
    }

    /// How many particles the pool can hold.
    pub fn capacity(&self) -> usize {
        self.particles.capacity()
    }

    /// How many are currently alive.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Retire every particle. Keeps the allocation.
    ///
    /// For a level change or a restart, where last level's chips should not
    /// still be falling through the new one.
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Add a particle, recycling the oldest if the pool is full.
    ///
    /// A particle that is already dead on arrival — non-positive `life` — is
    /// rejected rather than stored, so it cannot occupy a slot for a frame
    /// or displace a live one.
    pub fn spawn(&mut self, p: Particle) {
        if p.life <= 0.0 {
            return;
        }

        if self.len < self.particles.capacity() {
            if self.len < self.particles.len() {
                // Reuse a slot left behind by a retired particle.
                self.particles[self.len] = p;
            } else {
                // Still filling the backing store for the first time. This
                // pushes within the reserved capacity, so it never reallocates.
                self.particles.push(p);
            }
            self.len += 1;
            return;
        }

        // Full: overwrite whichever live particle has the least life left.
        //
        // ⚠️ Found by scanning, not by index. `update` compacts the live
        // range with a swap, which reorders particles — so position in the
        // vec says nothing about age, and an index-based "oldest" would
        // silently evict an arbitrary particle instead.
        if let Some(oldest) = self.oldest_index() {
            self.particles[oldest] = p;
        }
    }

    /// Index of the live particle closest to the end of its life.
    fn oldest_index(&self) -> Option<usize> {
        self.particles[..self.len]
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                a.progress().partial_cmp(&b.progress()).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
    }

    /// Advance every live particle by `dt` seconds and retire the spent ones.
    ///
    /// Retirement compacts the live range by swapping the dead particle with
    /// the last live one, which is O(1) per death and leaves no gaps to skip
    /// over when drawing. It does reorder the pool — see the note in
    /// [`spawn`](Self::spawn).
    pub fn update(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }

        let mut i = 0;
        while i < self.len {
            let p = &mut self.particles[i];
            p.age += dt;

            if !p.is_alive() {
                // Swap the dead one out to the end of the live range.
                self.len -= 1;
                self.particles.swap(i, self.len);
                // Do NOT advance i: the particle just swapped in has not
                // been updated this frame yet.
                continue;
            }

            p.vel.y += self.gravity * dt;
            p.pos.x += p.vel.x * dt;
            p.pos.y += p.vel.y * dt;
            i += 1;
        }
    }

    /// Draw every live particle as added light.
    ///
    /// Coordinates are play-field units; a caller working in a letterboxed
    /// viewport transforms them before calling, exactly as it does for every
    /// other draw.
    pub fn draw(&self, canvas: &mut Canvas<'_>) {
        for p in &self.particles[..self.len] {
            let half = p.size * 0.5;
            canvas.fill_rect_add_f(p.pos.x - half, p.pos.y - half, p.size, p.size, p.fade());
        }
    }

    /// The live particles, for a game that needs to draw them its own way.
    pub fn particles(&self) -> &[Particle] {
        &self.particles[..self.len]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dot(life: f32) -> Particle {
        Particle::new(Vec2::ZERO, Vec2::ZERO, 1.0, Color::WHITE, life)
    }

    #[test]
    fn a_new_pool_is_empty_and_preallocated() {
        let pool = ParticlePool::with_capacity(64);
        assert_eq!(pool.len(), 0);
        assert!(pool.is_empty());
        assert!(pool.capacity() >= 64);
    }

    /// The whole point of a fixed pool: no allocation after construction.
    /// Capacity is the observable proxy — a Vec that reallocated would
    /// report a larger one.
    #[test]
    fn spawning_to_capacity_never_reallocates() {
        let mut pool = ParticlePool::with_capacity(32);
        let cap = pool.capacity();
        for _ in 0..200 {
            pool.spawn(dot(1.0));
        }
        assert_eq!(pool.capacity(), cap, "the pool must never grow");
        assert_eq!(pool.len(), 32, "and must never exceed its capacity");
    }

    #[test]
    fn particles_retire_when_their_life_runs_out() {
        let mut pool = ParticlePool::with_capacity(8);
        pool.spawn(dot(1.0));
        pool.spawn(dot(0.5));
        assert_eq!(pool.len(), 2);

        pool.update(0.6);
        assert_eq!(pool.len(), 1, "the 0.5s particle should be gone");

        pool.update(0.6);
        assert_eq!(pool.len(), 0, "and now both");
    }

    /// The retirement swap must not skip the particle it swaps in. A naive
    /// loop that advances its index after a swap steps straight over the
    /// replacement, leaving dead particles alive for an extra frame each.
    #[test]
    fn retiring_many_at_once_leaves_none_behind() {
        let mut pool = ParticlePool::with_capacity(16);
        for _ in 0..10 {
            pool.spawn(dot(0.1));
        }
        assert_eq!(pool.len(), 10);
        pool.update(0.2);
        assert_eq!(pool.len(), 0, "every particle was spent; none may survive");
    }

    #[test]
    fn a_full_pool_recycles_the_oldest() {
        let mut pool = ParticlePool::with_capacity(2);
        pool.spawn(dot(10.0));
        pool.update(5.0); // this one is now half spent
        pool.spawn(dot(10.0)); // fresh

        // Full. The next spawn must evict the half-spent one, not the fresh one.
        pool.spawn(Particle::new(Vec2::new(9.0, 9.0), Vec2::ZERO, 1.0, Color::WHITE, 10.0));

        assert_eq!(pool.len(), 2);
        let ages: Vec<f32> = pool.particles().iter().map(|p| p.age).collect();
        assert!(!ages.contains(&5.0), "the oldest particle should have been recycled");
        assert!(
            pool.particles().iter().any(|p| p.pos.x == 9.0),
            "the newly spawned particle must be present"
        );
    }

    /// Recycling has to survive `update`'s reordering. After a swap, vec
    /// position says nothing about age, so an index-based oldest would evict
    /// the wrong particle.
    #[test]
    fn recycling_finds_the_oldest_after_a_swap_reorders_the_pool() {
        let mut pool = ParticlePool::with_capacity(3);
        pool.spawn(dot(0.1)); // dies first, forcing a swap
        pool.spawn(dot(100.0));
        pool.spawn(dot(100.0));

        pool.update(0.2); // the short one retires; the pool reorders
        assert_eq!(pool.len(), 2);
        pool.update(10.0); // both survivors now aged 10.2

        // Refill, then age one so there is an unambiguous oldest.
        pool.spawn(dot(100.0));
        assert_eq!(pool.len(), 3);

        let before: Vec<f32> = pool.particles().iter().map(|p| p.age).collect();
        let max_age = before.iter().cloned().fold(f32::MIN, f32::max);

        pool.spawn(Particle::new(Vec2::new(7.0, 7.0), Vec2::ZERO, 1.0, Color::WHITE, 100.0));
        let after: Vec<f32> = pool.particles().iter().map(|p| p.age).collect();

        assert_eq!(
            after.iter().filter(|&&a| a == max_age).count(),
            before.iter().filter(|&&a| a == max_age).count() - 1,
            "exactly one instance of the oldest age should have been evicted"
        );
    }

    #[test]
    fn velocity_moves_a_particle_and_gravity_bends_it() {
        let mut pool = ParticlePool::with_capacity(4).with_gravity(100.0);
        pool.spawn(Particle::new(
            Vec2::ZERO,
            Vec2::new(10.0, 0.0),
            1.0,
            Color::WHITE,
            10.0,
        ));
        pool.update(1.0);

        let p = pool.particles()[0];
        assert!((p.pos.x - 10.0).abs() < 1e-4, "x should track velocity");
        assert!(p.pos.y > 0.0, "gravity should have pulled it down");
        assert!(p.vel.y > 0.0, "and given it downward velocity");
    }

    #[test]
    fn zero_gravity_is_the_default_and_keeps_motion_straight() {
        let mut pool = ParticlePool::with_capacity(4);
        pool.spawn(Particle::new(
            Vec2::ZERO,
            Vec2::new(0.0, -10.0),
            1.0,
            Color::WHITE,
            10.0,
        ));
        pool.update(1.0);
        let p = pool.particles()[0];
        assert!((p.pos.y + 10.0).abs() < 1e-4, "no gravity, no bend");
    }

    #[test]
    fn a_particle_fades_over_its_life() {
        let mut p = dot(1.0);
        assert_eq!(p.fade().a, 255);
        p.age = 0.5;
        assert!((p.fade().a as i32 - 128).abs() <= 2, "half way, half intensity");
        p.age = 1.0;
        assert_eq!(p.fade().a, 0, "spent particles add no light");
    }

    /// A caller passing life: 0.0 must not produce a NaN that reaches the
    /// canvas. The particle is rejected outright.
    #[test]
    fn a_particle_with_no_life_is_never_stored() {
        let mut pool = ParticlePool::with_capacity(4);
        pool.spawn(dot(0.0));
        pool.spawn(dot(-1.0));
        assert_eq!(pool.len(), 0);
        // And the guard on progress() holds even if one is built by hand.
        assert_eq!(dot(0.0).progress(), 1.0);
        assert_eq!(dot(0.0).fade().a, 0);
    }

    #[test]
    fn clear_retires_everything_but_keeps_the_allocation() {
        let mut pool = ParticlePool::with_capacity(16);
        for _ in 0..10 {
            pool.spawn(dot(5.0));
        }
        let cap = pool.capacity();
        pool.clear();
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), cap);
        // And the pool is reusable straight away.
        pool.spawn(dot(5.0));
        assert_eq!(pool.len(), 1);
    }

    /// A stalled or reversed clock must not move anything. `update` is
    /// called from a frame loop, and a backwards dt would age particles
    /// negatively and un-retire them.
    #[test]
    fn a_non_advancing_clock_changes_nothing() {
        let mut pool = ParticlePool::with_capacity(4);
        pool.spawn(Particle::new(
            Vec2::ZERO,
            Vec2::new(10.0, 10.0),
            1.0,
            Color::WHITE,
            5.0,
        ));
        let before = pool.particles()[0];
        pool.update(0.0);
        pool.update(-1.0);
        pool.update(f32::NAN);
        assert_eq!(pool.particles()[0], before);
    }

    /// Drawing must stay inside the canvas whatever the particles do, and a
    /// pool of spent particles must add no light at all.
    #[test]
    fn drawing_clips_and_spent_particles_add_nothing() {
        let mut pool = ParticlePool::with_capacity(8);
        // Deliberately off every edge.
        for (x, y) in [(-50.0, -50.0), (500.0, 500.0), (2.0, 2.0)] {
            pool.spawn(Particle::new(
                Vec2::new(x, y),
                Vec2::ZERO,
                4.0,
                Color::WHITE,
                1.0,
            ));
        }

        let mut buf = vec![0u32; 16 * 16];
        {
            let mut canvas = Canvas::new(&mut buf, 16, 16);
            pool.draw(&mut canvas); // must not panic on the off-canvas ones
        }
        assert!(buf.iter().any(|&v| v != 0), "the on-canvas particle should draw");

        // Age them all out; a spent pool draws nothing.
        pool.update(2.0);
        let mut blank = vec![0u32; 16 * 16];
        {
            let mut canvas = Canvas::new(&mut blank, 16, 16);
            pool.draw(&mut canvas);
        }
        assert!(blank.iter().all(|&v| v == 0), "spent particles must add no light");
    }

    /// Particles are light: two overlapping ones must be brighter than one.
    #[test]
    fn overlapping_particles_accumulate_light() {
        let mut one = ParticlePool::with_capacity(4);
        one.spawn(Particle::new(Vec2::new(8.0, 8.0), Vec2::ZERO, 4.0, Color::rgb(60, 60, 60), 1.0));

        let mut two = ParticlePool::with_capacity(4);
        for _ in 0..2 {
            two.spawn(Particle::new(Vec2::new(8.0, 8.0), Vec2::ZERO, 4.0, Color::rgb(60, 60, 60), 1.0));
        }

        let brightness = |pool: &ParticlePool| {
            let mut buf = vec![0u32; 16 * 16];
            {
                let mut canvas = Canvas::new(&mut buf, 16, 16);
                pool.draw(&mut canvas);
            }
            buf.iter().map(|&v| v & 0xff).sum::<u32>()
        };

        assert!(
            brightness(&two) > brightness(&one),
            "two overlapping particles must read brighter than one"
        );
    }
}
