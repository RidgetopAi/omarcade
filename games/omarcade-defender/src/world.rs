//! The world: a horizontal loop, and the mountains along the bottom of it.
//!
//! The world is four screens wide and joined end to end, so flying west
//! long enough brings you back from the east. That loop is the single
//! fact every other file has to respect, and it is the one that breaks
//! things quietly when it is forgotten: two points near the seam are
//! CLOSE, even though their raw coordinates are nearly a whole world
//! apart. Subtracting positions without [`World::delta`] gives an answer
//! that is wrong only sometimes, which is the worst kind of wrong.
//!
//! Nothing here knows about the screen. The world is in world units; the
//! camera decides what part of it you can see.

/// How many screens wide the world is.
///
/// ⚠️ FOUR IS A STARTING POINT, NOT A FINDING. Brian: "I don't have a
/// feel but we can start with the 4x and see." It matches Defender's own
/// proportion, and it is the first number to try moving when the flying
/// feels cramped or the lap feels like a commute.
pub const WORLD_SCREENS: f32 = 4.0;

/// The visible width the world is measured against.
///
/// The window can be any size — the backend letterboxes a fixed 960x720
/// canvas — so this is a world-space constant, not a window size.
pub const VIEW_W: f32 = 960.0;
pub const VIEW_H: f32 = 720.0;

/// Total width of the loop, in world units.
pub const WORLD_W: f32 = VIEW_W * WORLD_SCREENS;

/// The mountains.
///
/// A ridge line sampled at fixed intervals and interpolated between, so
/// the terrain is defined by a handful of numbers rather than a bitmap
/// and can be regenerated at any width. Heights are measured UP from the
/// bottom of the world, because that is how a mountain is described; the
/// renderer flips it.
///
/// ⚠️ FLY-THROUGH FOR NOW. Brian: "let's keep it fly through right now,
/// I thought about making it crashable but can't commit to it yet." So
/// there is deliberately NO collision here — only the shape. Adding a
/// floor later is a small change against this; building one now and
/// removing it would not be.
pub struct Terrain {
    /// Height above the world floor at each sample, in world units.
    heights: Vec<f32>,
    /// World-space distance between neighbouring samples.
    step: f32,
}

impl Terrain {
    /// Generate a ridge with `samples` points across the whole world.
    ///
    /// Deterministic from `seed`: the same seed is the same mountains,
    /// so a screenshot can be reproduced and a test can assert a shape.
    /// A value-noise sum rather than true randomness — pure noise gives
    /// spiky static, and two octaves give something that reads as
    /// mountains at a glance.
    pub fn generate(samples: usize, seed: u32) -> Self {
        assert!(samples >= 4, "a ridge needs at least a few samples");

        let mut heights = Vec::with_capacity(samples);
        for i in 0..samples {
            // Two octaves: a slow swell for the big shapes, a faster one
            // for the detail on top. Their periods are deliberately not
            // multiples of each other, or the peaks line up and the
            // ridge repeats visibly.
            let t = i as f32 / samples as f32;
            // ⚠️ THREE OCTAVES, NOT TWO. Two gave a gentle swell that
            // rendered as a rolling hill — correct, and not mountains.
            // The third octave is what puts peaks on it.
            let big = noise(t * 3.0, seed);
            let small = noise(t * 11.0, seed ^ 0x9E37_79B9);
            let fine = noise(t * 29.0, seed ^ 0x85EB_CA6B);

            let h = BASE_HEIGHT
                + big * BIG_AMPLITUDE
                + small * SMALL_AMPLITUDE
                + fine * SMALL_AMPLITUDE * 0.45;
            heights.push(h.clamp(MIN_HEIGHT, MAX_HEIGHT));
        }

        // ⚠️ THE RIDGE HAS TO MEET ITSELF. The world wraps, so the last
        // sample sits next to the first — and without this the seam is a
        // cliff that appears out of nowhere every lap. Blend the final
        // few samples towards the first.
        let blend = (samples / 8).max(2);
        for i in 0..blend {
            let t = i as f32 / blend as f32;
            let idx = samples - blend + i;
            heights[idx] = heights[idx] * (1.0 - t) + heights[0] * t;
        }

        Self {
            heights,
            step: WORLD_W / samples as f32,
        }
    }

    /// Ridge height at a world x, interpolated between samples.
    ///
    /// Takes any x, wrapped or not, so callers never have to normalise
    /// before asking.
    pub fn height_at(&self, x: f32) -> f32 {
        let x = wrap(x);
        let pos = x / self.step;
        let i = pos.floor() as usize % self.heights.len();
        let j = (i + 1) % self.heights.len();
        let t = pos - pos.floor();
        self.heights[i] * (1.0 - t) + self.heights[j] * t
    }

    /// The samples, for a renderer that wants to walk them directly
    /// rather than resample a curve it already has.
    ///
    /// ⚠️ Used by the tests and not (yet) by the binary, which is what
    /// the dead-code warning is about. Kept because the terrain's own
    /// tests need to inspect the ridge, and a generator whose output
    /// cannot be examined cannot be asserted on.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn samples(&self) -> &[f32] {
        &self.heights
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn step(&self) -> f32 {
        self.step
    }
}

/// Where the ridge sits before any noise is added, as a fraction of the
/// view height. Low enough to leave most of the screen as sky, high
/// enough that the mountains are a presence rather than a trim.
const BASE_HEIGHT: f32 = VIEW_H * 0.20;
const BIG_AMPLITUDE: f32 = VIEW_H * 0.14;
const SMALL_AMPLITUDE: f32 = VIEW_H * 0.075;
const MIN_HEIGHT: f32 = VIEW_H * 0.05;
const MAX_HEIGHT: f32 = VIEW_H * 0.36;

/// Smooth value noise in [-1, 1], deterministic in `seed`.
///
/// Hash-based rather than a table: no allocation, no state, and the same
/// input always gives the same output, which is what makes the terrain
/// reproducible.
fn noise(t: f32, seed: u32) -> f32 {
    let i = t.floor();
    let f = t - i;
    // Smoothstep, so the joins between integer samples have no visible
    // corner. Linear interpolation here reads as faceted.
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash_unit(i as i32, seed);
    let b = hash_unit(i as i32 + 1, seed);
    (a * (1.0 - u) + b * u) * 2.0 - 1.0
}

/// A stable pseudo-random value in [0, 1] from an integer and a seed.
fn hash_unit(n: i32, seed: u32) -> f32 {
    let mut h = (n as u32).wrapping_mul(0x27D4_EB2D) ^ seed.wrapping_mul(0x1656_67B1);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2545_F491);
    h ^= h >> 13;
    (h & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}

/// Bring any x into [0, WORLD_W).
///
/// `rem_euclid` rather than `%`: the remainder operator keeps the sign
/// of the left operand, so a ship at x = -10 would land at -10 rather
/// than at the far end of the world, and the seam would only break when
/// flying west.
pub fn wrap(x: f32) -> f32 {
    x.rem_euclid(WORLD_W)
}

/// The signed distance from `from` to `to`, taking the short way round.
///
/// ⚠️ USE THIS INSTEAD OF SUBTRACTING. In a looping world the raw
/// difference between two points near the seam is almost a whole world
/// wide, while the real distance between them is a few units. Anything
/// that measures — the camera chasing the ship, a shot deciding whether
/// it has passed a target, an enemy steering — is wrong in exactly the
/// place a player will fly through constantly.
///
/// Result is in (-WORLD_W/2, WORLD_W/2]: positive means `to` is to the
/// east of `from` by the shorter route.
pub fn delta(from: f32, to: f32) -> f32 {
    let raw = wrap(to) - wrap(from);
    if raw > WORLD_W * 0.5 {
        raw - WORLD_W
    } else if raw <= -WORLD_W * 0.5 {
        raw + WORLD_W
    } else {
        raw
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping_is_euclidean_in_both_directions() {
        assert_eq!(wrap(0.0), 0.0);
        assert_eq!(wrap(WORLD_W), 0.0);
        // ⚠️ THE ONE `%` GETS WRONG. A ship flying west past zero must
        // arrive at the far end of the world, not at a negative x.
        assert!((wrap(-10.0) - (WORLD_W - 10.0)).abs() < 0.001, "{}", wrap(-10.0));
        assert!((wrap(-WORLD_W - 5.0) - (WORLD_W - 5.0)).abs() < 0.001);
        // And anything, however far out, lands inside the world.
        for x in [-99_999.0, -1.0, 0.5, WORLD_W - 0.5, WORLD_W * 7.3] {
            let w = wrap(x);
            assert!((0.0..WORLD_W).contains(&w), "{x} wrapped to {w}");
        }
    }

    #[test]
    fn delta_takes_the_short_way_round() {
        // Ordinary case, nowhere near the seam.
        assert!((delta(100.0, 400.0) - 300.0).abs() < 0.001);
        assert!((delta(400.0, 100.0) + 300.0).abs() < 0.001);

        // ⚠️ THE SEAM. 10 units west of zero and 10 units east of it are
        // 20 apart, not WORLD_W - 20 apart. Subtracting raw coordinates
        // gets this wrong, and gets it wrong only here.
        let east = 10.0;
        let west = WORLD_W - 10.0;
        assert!((delta(west, east) - 20.0).abs() < 0.001, "{}", delta(west, east));
        assert!((delta(east, west) + 20.0).abs() < 0.001, "{}", delta(east, west));

        // Never longer than half a lap, whichever pair you ask about.
        for a in [0.0, 123.0, WORLD_W * 0.5, WORLD_W - 1.0] {
            for b in [0.0, 999.0, WORLD_W * 0.25, WORLD_W - 3.0] {
                assert!(delta(a, b).abs() <= WORLD_W * 0.5 + 0.001);
            }
        }
    }

    #[test]
    fn the_ridge_meets_itself_at_the_seam() {
        let t = Terrain::generate(256, 7);
        // Sampling either side of x = 0 must not step off a cliff. The
        // world loops, so this join is one a player crosses every lap.
        let just_east = t.height_at(1.0);
        let just_west = t.height_at(WORLD_W - 1.0);
        let jump = (just_east - just_west).abs();
        assert!(
            jump < VIEW_H * 0.02,
            "the ridge jumps {jump:.1} units across the seam — that is a visible cliff"
        );
    }

    #[test]
    fn the_ridge_stays_inside_its_bounds() {
        let t = Terrain::generate(512, 99);
        for (i, h) in t.samples().iter().enumerate() {
            assert!(
                (MIN_HEIGHT..=MAX_HEIGHT).contains(h),
                "sample {i} is {h}, outside [{MIN_HEIGHT}, {MAX_HEIGHT}]"
            );
        }
    }

    #[test]
    fn the_ridge_is_not_flat() {
        // ⚠️ A TEST THAT COULD NOT FAIL WOULD BE WORSE THAN NONE: a
        // terrain generator returning a constant would satisfy every
        // bounds check above. Assert there is actually relief.
        let t = Terrain::generate(256, 3);
        let lo = t.samples().iter().copied().fold(f32::MAX, f32::min);
        let hi = t.samples().iter().copied().fold(f32::MIN, f32::max);
        assert!(hi - lo > VIEW_H * 0.05, "the ridge is nearly flat: {lo}..{hi}");
    }

    #[test]
    fn the_same_seed_is_the_same_mountains() {
        let a = Terrain::generate(128, 42);
        let b = Terrain::generate(128, 42);
        let c = Terrain::generate(128, 43);
        assert_eq!(a.samples(), b.samples(), "a seed must reproduce");
        assert_ne!(a.samples(), c.samples(), "different seeds must differ");
    }

    #[test]
    fn height_reads_the_same_wrapped_or_not() {
        let t = Terrain::generate(128, 11);
        for x in [0.0, 37.5, WORLD_W * 0.33, WORLD_W - 2.0] {
            let here = t.height_at(x);
            assert!((t.height_at(x + WORLD_W) - here).abs() < 0.001);
            assert!((t.height_at(x - WORLD_W) - here).abs() < 0.001);
        }
    }
}
