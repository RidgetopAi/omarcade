//! Mines: what a Bomber leaves behind.
//!
//! ★ BRIAN'S SPEC: Bombers "LEAVE MINE FIELDS behind. ⚠️ MINES CANNOT BE
//! SHOT — avoid. Kill bombers before they lay." So a mine is not an enemy:
//! it scores nothing, it does not hold a wave open, your laser passes
//! through it, and flying into one costs a life. It sits still and, after
//! a while, burns out.

use crate::world;

/// At most this many at once — the original's own cap (`BMBCNT < 10`).
pub const MAX: usize = 10;

/// How long a mine lives, seconds: between these.
///
/// ⚠️ OURS. Long enough that a Bomber's trail is a FIELD you have to fly
/// around, short enough that the screen clears behind it.
pub const LIFE_MIN: f32 = 4.0;
pub const LIFE_MAX: f32 = 8.0;

/// A mine's hitbox, world units — derived from its art like the enemies'.
pub const HALF: (f32, f32) = {
    let (w, h) = crate::art::half_extents(&crate::art::MINE_LAYERS);
    (w * crate::art::SCALE, h * crate::art::SCALE)
};

#[derive(Debug, Clone, Copy)]
pub struct Mine {
    pub x: f32,
    pub y: f32,
    pub age: f32,
    pub life: f32,
}

impl Mine {
    /// 0..1, for the pulse: a mine that is about to go out flickers.
    pub fn pulse(&self) -> f32 {
        let fading = self.age > self.life - 1.0;
        let rate = if fading { 14.0 } else { 4.0 };
        0.5 + 0.5 * (self.age * rate).sin()
    }
}

#[derive(Debug, Default)]
pub struct Mines {
    live: Vec<Mine>,
    seed: u32,
}

impl Mines {
    pub const fn new() -> Self {
        Self { live: Vec::new(), seed: 0x0B0B_1E55 }
    }

    /// No mines, as a constant — for a render test's empty scene.
    #[cfg(test)]
    pub const fn empty() -> Self {
        Self::new()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Mine> {
        self.live.iter()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.live.len()
    }

    pub fn clear(&mut self) {
        self.live.clear();
    }

    /// Drop a mine at `(x, y)`, unless the field is full.
    pub fn lay(&mut self, x: f32, y: f32) {
        if self.live.len() >= MAX {
            return;
        }
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        let r = (self.seed & 0xFFFF) as f32 / 65_536.0;
        self.live.push(Mine { x: world::wrap(x), y, age: 0.0, life: LIFE_MIN + (LIFE_MAX - LIFE_MIN) * r });
    }

    pub fn step(&mut self, dt: f32) {
        for m in &mut self.live {
            m.age += dt;
        }
        self.live.retain(|m| m.age < m.life);
    }

    /// Does a box at `(x, y)` with these half-extents touch a mine? The
    /// mine it touches goes off and is gone.
    pub fn hit(&mut self, x: f32, y: f32, half_w: f32, half_h: f32) -> bool {
        let found = self.live.iter().position(|m| {
            world::delta(m.x, x).abs() <= HALF.0 + half_w && (m.y - y).abs() <= HALF.1 + half_h
        });
        if let Some(i) = found {
            self.live.swap_remove(i);
        }
        found.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_field_is_capped_at_ten() {
        let mut m = Mines::new();
        for i in 0..20 {
            m.lay(i as f32 * 10.0, 300.0);
        }
        assert_eq!(m.len(), MAX);
    }

    #[test]
    fn mines_burn_out_within_their_life() {
        let mut m = Mines::new();
        m.lay(0.0, 300.0);
        m.step(LIFE_MIN * 0.9);
        assert_eq!(m.len(), 1, "gone too soon");
        m.step(LIFE_MAX);
        assert_eq!(m.len(), 0, "outlived its life");
    }

    /// Through the seam: a mine at x = 5 touches a ship at WORLD_W - 5.
    #[test]
    fn a_mine_on_the_seam_still_hits() {
        let mut m = Mines::new();
        m.lay(5.0, 300.0);
        assert!(m.hit(world::WORLD_W - 5.0, 300.0, 22.0, 10.0));
        assert_eq!(m.len(), 0, "the mine that went off should be gone");
    }
}
