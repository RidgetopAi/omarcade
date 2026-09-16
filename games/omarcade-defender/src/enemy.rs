//! Landers, and what it takes to kill one.
//!
//! ⚠️ THIS STAGE IS DELIBERATELY INCOMPLETE. A Defender Lander hunts a
//! Humanoid, tractor-beams it upward and becomes a Mutant at the top.
//! None of that is here — S5 is "there is something in the world and it
//! can die", and the hunting is S6 where there are Humanoids to hunt.
//! Building the abduction now would mean building it against imaginary
//! Humanoids and rewriting it when the real ones arrive.
//!
//! What IS here: they arrive, they drift over the ridge, they can be
//! shot, and they are worth points.

use crate::world::{self, Terrain};

/// What a Lander is worth.
///
/// ★ 100, FROM THE ARCADE'S OWN ENEMY CHART. The build plan said 150;
/// Brian checked the machine and corrected it (ref:defender-points).
/// Where the plan and the chart disagree, the chart wins.
pub const LANDER_POINTS: u32 = 100;

/// How wide a Lander is for the purposes of being hit, in world units.
///
/// ★ DERIVED FROM THE ART, NOT PICKED. The Lander is 12 units wide in
/// art space and draws at [`crate::art::SCALE`], so this is the real
/// drawn half-width — the same rule Omaprix's posts follow, where the
/// hitbox IS the art rather than a number that drifts away from it.
pub const LANDER_HALF_W: f32 = 6.0 * crate::art::SCALE;
pub const LANDER_HALF_H: f32 = 7.5 * crate::art::SCALE;

/// How high above the ridge a Lander settles.
///
/// ⚠️ RAISED FROM 120 AFTER LOOKING AT A FRAME. At 120 the Landers sat
/// in the bottom quarter of the screen, tucked against the ridge — the
/// fight happened along the floor and the whole middle of the playfield
/// was empty. Defender's Landers occupy the airspace you fly through.
/// The ridge still governs the height, so they rise and fall with it.
pub const HOVER_HEIGHT: f32 = 300.0;

/// How fast a Lander drifts along the world, in units per second.
///
/// Slow. A Lander in the original is not chasing you in this phase — it
/// is looking for a Humanoid, and the menace is that it is ignoring you.
pub const DRIFT_SPEED: f32 = 46.0;

/// How long the warp-in takes, in seconds.
pub const WARP_SECONDS: f32 = 0.55;

/// How long the death animation holds the corpse before it is removed.
///
/// The particles outlive this; it only governs how long the Lander's own
/// shape keeps being drawn, flashing, before it goes.
pub const DEATH_SECONDS: f32 = 0.18;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Materialising. Cannot be hit yet — a Lander that could be killed
    /// before it finished arriving would let a player farm the spawn
    /// point, and would look like a bug besides.
    Warping,
    /// Alive and drifting. The only phase that can be shot.
    Hovering,
    /// Hit, and playing out its death. No longer collidable.
    Dying,
}

#[derive(Debug, Clone, Copy)]
pub struct Lander {
    /// World x, always wrapped.
    pub x: f32,
    /// Height above the world floor, matching the ship's convention.
    pub y: f32,
    /// Signed drift, world units per second.
    pub vx: f32,
    pub phase: Phase,
    /// Seconds spent in the current phase.
    pub elapsed: f32,
}

impl Lander {
    pub fn new(x: f32, y: f32, vx: f32) -> Self {
        Self { x: world::wrap(x), y, vx, phase: Phase::Warping, elapsed: 0.0 }
    }

    /// Can a shot hit this one right now?
    pub fn is_target(&self) -> bool {
        self.phase == Phase::Hovering
    }

    /// Should this one still be drawn?
    pub fn is_alive(&self) -> bool {
        !matches!(self.phase, Phase::Dying if self.elapsed >= DEATH_SECONDS)
    }

    /// How far through the current phase, 0..=1.
    pub fn progress(&self) -> f32 {
        let span = match self.phase {
            Phase::Warping => WARP_SECONDS,
            Phase::Dying => DEATH_SECONDS,
            Phase::Hovering => return 1.0,
        };
        (self.elapsed / span).clamp(0.0, 1.0)
    }

    /// Mark this one as hit. Returns the points it is worth.
    pub fn kill(&mut self) -> u32 {
        self.phase = Phase::Dying;
        self.elapsed = 0.0;
        LANDER_POINTS
    }

    fn step(&mut self, terrain: &Terrain, dt: f32) {
        self.elapsed += dt;

        match self.phase {
            Phase::Warping => {
                if self.elapsed >= WARP_SECONDS {
                    self.phase = Phase::Hovering;
                    self.elapsed = 0.0;
                }
            }
            Phase::Hovering => {
                self.x = world::wrap(self.x + self.vx * dt);

                // Follow the ridge rather than holding an absolute
                // height: a Lander at a fixed y would sink into a peak
                // and float high over a valley, and the terrain here has
                // real peaks. Eased rather than snapped, or it jitters
                // over the ridge's fine detail.
                let want = terrain.height_at(self.x) + HOVER_HEIGHT;
                self.y += (want - self.y) * (1.0 - (-4.0 * dt).exp());
            }
            Phase::Dying => {}
        }
    }
}

/// Every Lander in the world.
#[derive(Debug, Default)]
pub struct Landers {
    live: Vec<Lander>,
}

impl Landers {
    pub fn new() -> Self {
        Self { live: Vec::new() }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Lander> {
        self.live.iter()
    }

    pub fn len(&self) -> usize {
        self.live.len()
    }

    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }

    /// How many are still killable — the number that matters for "is the
    /// wave over", which S9 will ask.
    pub fn remaining(&self) -> usize {
        self.live.iter().filter(|l| l.phase != Phase::Dying).count()
    }

    pub fn spawn(&mut self, lander: Lander) {
        self.live.push(lander);
    }

    pub fn clear(&mut self) {
        self.live.clear();
    }

    /// Scatter `count` Landers across the world, away from `avoid_x`.
    ///
    /// ⚠️ NOT UNIFORMLY RANDOM ACROSS THE WORLD — spread evenly and then
    /// jittered. Four uniform draws over four screens clump often enough
    /// that a wave regularly arrives as "three in one place and one on
    /// the far side", which reads as a bug rather than as variety.
    pub fn scatter(&mut self, count: usize, avoid_x: f32, terrain: &Terrain, seed: u32) {
        let spacing = world::WORLD_W / count.max(1) as f32;
        for i in 0..count {
            // ⚠️ wrapping_mul, NOT `*`. A plain multiply here PANICS in a
            // debug build from i = 2 onward — an overflow that release
            // mode would have silently wrapped, so the bug would have
            // shipped working and crashed only under a debugger.
            let jitter = hash01(seed.wrapping_add((i as u32).wrapping_mul(2_654_435_761)));
            let mut x = world::wrap(avoid_x + spacing * (i as f32 + 0.5 + (jitter - 0.5) * 0.6));

            // Never directly on top of the player: arriving in the
            // player's lap gives them no chance to react to something
            // they could not have seen coming.
            if world::delta(avoid_x, x).abs() < world::VIEW_W * 0.35 {
                x = world::wrap(x + world::VIEW_W * 0.5);
            }

            let dir = if hash01(seed ^ (i as u32).wrapping_mul(0x9E37_79B9)) < 0.5 {
                -1.0
            } else {
                1.0
            };
            let y = terrain.height_at(x) + HOVER_HEIGHT;
            self.spawn(Lander::new(x, y, DRIFT_SPEED * dir));
        }
    }

    pub fn step(&mut self, terrain: &Terrain, dt: f32) {
        for l in &mut self.live {
            l.step(terrain, dt);
        }
        self.live.retain(|l| l.is_alive());
    }

    /// The first Lander overlapping `(x, y)`, if any.
    ///
    /// ⚠️ THE X TEST GOES THROUGH [`world::delta`]. A shot at x = 5 and a
    /// Lander at x = WORLD_W - 5 are ten units apart, and subtracting
    /// their coordinates says they are nearly a whole world apart. Every
    /// enemy sitting on the seam would be unkillable, and only from one
    /// side — a bug that would survive any test written near the origin.
    pub fn hit_test(&self, x: f32, y: f32) -> Option<usize> {
        self.live.iter().position(|l| {
            l.is_target()
                && world::delta(l.x, x).abs() <= LANDER_HALF_W
                && (l.y - y).abs() <= LANDER_HALF_H
        })
    }

    /// Kill the one at `index` and report its points.
    pub fn kill(&mut self, index: usize) -> u32 {
        self.live[index].kill()
    }
}

/// A deterministic 0..1 from an integer, for placement that repeats.
fn hash01(mut h: u32) -> f32 {
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846C_A68B);
    h ^= h >> 16;
    (h & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terrain() -> Terrain {
        Terrain::generate(256, 0x0DEF_E4DE)
    }

    #[test]
    fn a_lander_cannot_be_shot_until_it_has_arrived() {
        let t = terrain();
        let mut ls = Landers::new();
        ls.spawn(Lander::new(500.0, 300.0, 0.0));

        assert!(ls.hit_test(500.0, 300.0).is_none(), "warping must not be hittable");
        ls.step(&t, WARP_SECONDS + 0.01);
        assert!(ls.hit_test(500.0, 300.0).is_some(), "it should be hittable now");
    }

    /// ⚠️ THE SEAM, AS A TEST. This is the bug the module exists to
    /// prevent, and it is invisible anywhere except within one hitbox of
    /// x = 0.
    #[test]
    fn a_lander_on_the_seam_can_be_shot_from_both_sides() {
        let t = terrain();
        let mut ls = Landers::new();
        // Sitting a few units west of the seam, i.e. at the very top of
        // the coordinate range.
        ls.spawn(Lander::new(world::WORLD_W - 4.0, 300.0, 0.0));
        ls.step(&t, WARP_SECONDS + 0.01);
        let y = ls.iter().next().unwrap().y;

        assert!(
            ls.hit_test(world::WORLD_W - 6.0, y).is_some(),
            "a shot just west of it must connect"
        );
        assert!(
            ls.hit_test(2.0, y).is_some(),
            "a shot just EAST of it — across the seam — must also connect"
        );
        assert!(
            ls.hit_test(world::VIEW_W, y).is_none(),
            "something a screen away must not connect"
        );
    }

    #[test]
    fn a_dead_lander_stops_being_a_target_immediately() {
        let t = terrain();
        let mut ls = Landers::new();
        ls.spawn(Lander::new(500.0, 300.0, 0.0));
        ls.step(&t, WARP_SECONDS + 0.01);

        let y = ls.iter().next().unwrap().y;
        let hit = ls.hit_test(500.0, y).expect("should be hittable");
        assert_eq!(ls.kill(hit), LANDER_POINTS);
        assert!(ls.hit_test(500.0, y).is_none(), "a dying lander is not a target");
        assert_eq!(ls.remaining(), 0, "it must not count toward the wave");
    }

    #[test]
    fn a_dead_lander_is_removed_once_its_death_has_played() {
        let t = terrain();
        let mut ls = Landers::new();
        ls.spawn(Lander::new(500.0, 300.0, 0.0));
        ls.step(&t, WARP_SECONDS + 0.01);
        let y = ls.iter().next().unwrap().y;
        let hit = ls.hit_test(500.0, y).unwrap();
        ls.kill(hit);

        ls.step(&t, DEATH_SECONDS * 0.5);
        assert_eq!(ls.len(), 1, "the corpse should still be drawn");
        ls.step(&t, DEATH_SECONDS);
        assert!(ls.is_empty(), "the corpse should be gone");
    }

    #[test]
    fn landers_follow_the_ridge_rather_than_a_fixed_height() {
        let t = terrain();
        let mut ls = Landers::new();
        // Start at a wrong height and let it settle.
        ls.spawn(Lander::new(500.0, 10.0, 0.0));
        ls.step(&t, WARP_SECONDS + 0.01);
        for _ in 0..240 {
            ls.step(&t, 1.0 / 60.0);
        }
        let l = ls.iter().next().unwrap();
        let want = t.height_at(l.x) + HOVER_HEIGHT;
        assert!((l.y - want).abs() < 4.0, "settled at {} not {want}", l.y);
    }

    #[test]
    fn a_scattered_wave_is_spread_out_and_not_on_the_player() {
        let t = terrain();
        let mut ls = Landers::new();
        ls.scatter(5, 0.0, &t, 12345);
        assert_eq!(ls.len(), 5);
        assert_eq!(ls.remaining(), 5);

        for l in ls.iter() {
            assert!(
                world::delta(0.0, l.x).abs() > world::VIEW_W * 0.3,
                "spawned too close to the player at {}",
                l.x
            );
            assert!(l.x >= 0.0 && l.x < world::WORLD_W, "outside the world: {}", l.x);
        }
    }

    /// The same seed must give the same wave, or a screenshot cannot be
    /// reproduced and a bug cannot be chased.
    #[test]
    fn scattering_is_deterministic() {
        let t = terrain();
        let mut a = Landers::new();
        let mut b = Landers::new();
        a.scatter(4, 300.0, &t, 99);
        b.scatter(4, 300.0, &t, 99);
        let xs: Vec<f32> = a.iter().map(|l| l.x).collect();
        let ys: Vec<f32> = b.iter().map(|l| l.x).collect();
        assert_eq!(xs, ys);
    }

    #[test]
    fn a_drifting_lander_wraps_with_the_world() {
        let t = terrain();
        let mut ls = Landers::new();
        ls.spawn(Lander::new(5.0, 300.0, -DRIFT_SPEED));
        ls.step(&t, WARP_SECONDS + 0.01);
        for _ in 0..600 {
            ls.step(&t, 1.0 / 60.0);
        }
        let l = ls.iter().next().unwrap();
        assert!(l.x >= 0.0 && l.x < world::WORLD_W, "drifted out of the world: {}", l.x);
        assert!(l.x > world::WORLD_W * 0.5, "should have wrapped west, at {}", l.x);
    }
}
