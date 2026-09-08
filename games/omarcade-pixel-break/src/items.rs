//! Falling power-ups: what drops, how often, and what it does when caught.
//!
//! Kept out of `state.rs` so the world model does not grow a second
//! personality. `state` owns the items that exist; this module owns the
//! rules about which ones appear.
//!
//! # The bag, and why it is not a dice roll
//!
//! ⚠️ Drops come from a **shuffled bag**, never from an independent random
//! choice per drop. With a 1-in-5 bomb chance, independent rolls give four
//! bombs in a row about once every six hundred drops — which sounds rare
//! until you remember a full run has hundreds of drops, so it happens, and
//! when it happens the player does not think "unlucky", they think the game
//! cheated. A bag of five holding one bomb makes that sequence *impossible*
//! while keeping the same ratio. This is the difference between hard and
//! unfair, and the plan calls it the single most-felt tuning decision in
//! the feature.

use crate::geom::Vec2;

/// How much wider a grow makes the paddle, and for how long.
///
/// Three strengths so a good catch can feel better than an ordinary one.
/// The durations are Brian's stated guesses, kept in one place precisely
/// because `probe_balance` is expected to revise them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strength {
    Small,
    Medium,
    Large,
}

impl Strength {
    /// Seconds of paddle growth this strength grants.
    pub const fn seconds(self) -> f32 {
        match self {
            Strength::Small => 10.0,
            Strength::Medium => 25.0,
            Strength::Large => 45.0,
        }
    }

    /// How much wider the paddle gets, as a multiple of its base width.
    pub const fn scale(self) -> f32 {
        match self {
            Strength::Small => 1.25,
            Strength::Medium => 1.5,
            Strength::Large => 1.8,
        }
    }
}

/// What a falling item does when caught.
///
/// Deliberately only the two S5 ships. The magnet and the Omarchy item are
/// S6: a variant that exists but silently does nothing is worse than one
/// that does not exist, because it looks implemented from the outside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Grow(Strength),
    /// Narrows the paddle. The only bad drop.
    Bomb,
}

impl ItemKind {
    pub fn is_bad(self) -> bool {
        matches!(self, ItemKind::Bomb)
    }

    /// Seconds the effect lasts once caught.
    pub fn seconds(self) -> f32 {
        match self {
            ItemKind::Grow(s) => s.seconds(),
            ItemKind::Bomb => BOMB_SECONDS,
        }
    }
}

/// How long a caught bomb keeps the paddle narrow.
pub const BOMB_SECONDS: f32 = 15.0;
/// How narrow a bomb makes the paddle, as a multiple of its base width.
pub const BOMB_SCALE: f32 = 0.6;

/// How fast items fall, in units/s.
///
/// ⚠️ Deliberately far slower than any ball speed (340-460). An item that
/// falls at ball pace is one more thing moving at ball pace, and the plan
/// is explicit that an uncaught item must never be mistaken for a ball.
/// Motion is the first cue the eye uses; shape is the second.
pub const FALL_SPEED: f32 = 190.0;

pub const ITEM_W: f32 = 34.0;
pub const ITEM_H: f32 = 16.0;

/// Chance that a brick's final hit drops something, at level 1 and at the
/// last level. Linear between.
///
/// ⚠️ These are the numbers `probe_balance` exists to judge. The measured
/// problem they aim at is the endgame tail: 35% of a flawless run is one
/// ball hunting a nearly-empty field, and a wider paddle plus more traffic
/// is what shortens it. Re-run the probe after changing them.
pub const DROP_CHANCE_LOW: f32 = 0.12;
pub const DROP_CHANCE_HIGH: f32 = 0.20;

/// One bomb for every four good items.
const BAG: [ItemKind; 5] = [
    ItemKind::Grow(Strength::Small),
    ItemKind::Grow(Strength::Medium),
    ItemKind::Bomb,
    ItemKind::Grow(Strength::Small),
    ItemKind::Grow(Strength::Large),
];

/// A falling power-up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Item {
    pub pos: Vec2,
    pub kind: ItemKind,
}

impl Item {
    pub fn new(pos: Vec2, kind: ItemKind) -> Self {
        Item { pos, kind }
    }

    /// The item as a rect, which is how catching sees it.
    pub fn rect(&self) -> crate::geom::Rect {
        crate::geom::Rect::from_center(self.pos, ITEM_W / 2.0, ITEM_H / 2.0)
    }
}

/// Decides what drops and when.
///
/// Holds the shuffled bag and the random source. Lives on `GameState` so a
/// test or a probe can seed it and get the same run twice — `probe_balance`
/// is only a measurement if it is reproducible.
#[derive(Debug, Clone)]
pub struct Dropper {
    seed: u32,
    /// The current bag, drawn from the back. Refilled and reshuffled when
    /// it empties.
    bag: Vec<ItemKind>,
    /// Whether the last item drawn was a bomb, so a refill can avoid
    /// putting another one straight after it across the bag seam.
    last_was_bad: bool,
}

impl Dropper {
    pub fn new(seed: u32) -> Self {
        // ⚠️ `seed | 1` here would map 42 and 43 to the SAME state, so two
        // different seeds would produce identical runs. Guard the zero case
        // by offsetting instead, which is injective.
        let mut d = Dropper {
            seed: seed.wrapping_add(0x9E37_79B9),
            bag: Vec::with_capacity(BAG.len()),
            last_was_bad: false,
        };
        d.refill();
        d
    }

    /// A number in `0.0..1.0`.
    ///
    /// ⚠️ Masked to 16 bits before dividing. An unmasked shift keeps every
    /// upper bit and the result runs to millions — which is exactly the bug
    /// that put six balls thirty thousand units off-field at S3 (L037).
    fn next_f32(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
        ((self.seed >> 8) & 0xFFFF) as f32 / 65536.0
    }

    fn next_below(&mut self, n: usize) -> usize {
        (self.next_f32() * n as f32) as usize % n.max(1)
    }

    /// Refill the bag with one of each entry, shuffled.
    ///
    /// ⚠️ **The reshuffle is where the guarantee is actually won or lost.**
    /// A shuffled bag stops two bombs appearing *within* a bag, but says
    /// nothing about the seam: bag N can end with the bomb and bag N+1
    /// begin with it, and the player gets the back-to-back pair the bag
    /// was supposed to make impossible. Drawing is from the BACK, so the
    /// next item out is the last element — if that is a bomb and the
    /// previous draw was too, swap it away from the end.
    fn refill(&mut self) {
        let last_was_bomb = self.last_was_bad;
        self.bag.clear();
        self.bag.extend_from_slice(&BAG);
        // Fisher-Yates, back to front.
        for i in (1..self.bag.len()).rev() {
            let j = self.next_below(i + 1);
            self.bag.swap(i, j);
        }

        // Close the seam. Swapping with the front keeps every bag a true
        // permutation of BAG — the ratio is untouched, only the join moves.
        if last_was_bomb {
            if let Some(end) = self.bag.last() {
                if end.is_bad() {
                    let n = self.bag.len();
                    self.bag.swap(0, n - 1);
                }
            }
        }
    }

    /// Chance a final hit drops something, on this level.
    pub fn drop_chance(&self, level: u32, levels: u32) -> f32 {
        let level = level.clamp(1, levels);
        let t = (level - 1) as f32 / (levels - 1).max(1) as f32;
        DROP_CHANCE_LOW + (DROP_CHANCE_HIGH - DROP_CHANCE_LOW) * t
    }

    /// Roll for a drop at `pos`. Returns the item if one fell.
    ///
    /// ⚠️ Call this ONLY on a brick's **final** hit. A reinforced brick's
    /// first hit must drop nothing, or an armoured brick — four hits — would
    /// roll four times and the late levels would rain power-ups exactly
    /// where they are least needed.
    pub fn roll(&mut self, pos: Vec2, level: u32, levels: u32) -> Option<Item> {
        if self.next_f32() >= self.drop_chance(level, levels) {
            return None;
        }
        Some(Item::new(pos, self.draw()))
    }

    /// Take the next item from the bag, refilling if it is empty.
    fn draw(&mut self) -> ItemKind {
        if self.bag.is_empty() {
            self.refill();
        }
        // `refill` guarantees a non-empty bag.
        let kind = self.bag.pop().unwrap_or(ItemKind::Grow(Strength::Small));
        self.last_was_bad = kind.is_bad();
        kind
    }
}

impl Default for Dropper {
    fn default() -> Self {
        Dropper::new(0x5EED_D00D)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚠️ The property the bag exists for: a run of bombs is IMPOSSIBLE, not
    /// merely unlikely. Independent 1-in-5 rolls would produce two in a row
    /// every twenty-five drops; the bag holds exactly one bomb per five.
    #[test]
    fn the_bag_never_deals_two_bombs_in_a_row() {
        let mut d = Dropper::new(1);
        let mut last_was_bomb = false;
        for i in 0..5000 {
            let kind = d.draw();
            let bomb = kind.is_bad();
            assert!(
                !(bomb && last_was_bomb),
                "two bombs in a row at draw {i} — the bag must make this impossible"
            );
            last_was_bomb = bomb;
        }
    }

    /// One bomb in five, held exactly over any whole number of bags.
    #[test]
    fn the_bag_holds_the_one_in_four_ratio() {
        let mut d = Dropper::new(7);
        let bags = 400;
        let mut bombs = 0;
        for _ in 0..bags * BAG.len() {
            if d.draw().is_bad() {
                bombs += 1;
            }
        }
        assert_eq!(bombs, bags, "exactly one bomb per bag of {}", BAG.len());
    }

    /// Every bag contains the same multiset — shuffling reorders, it does
    /// not resample.
    #[test]
    fn every_bag_holds_the_same_items() {
        let mut d = Dropper::new(99);
        for bag in 0..200 {
            let mut drawn: Vec<ItemKind> = (0..BAG.len()).map(|_| d.draw()).collect();
            let mut want = BAG.to_vec();
            let key = |k: &ItemKind| format!("{k:?}");
            drawn.sort_by_key(key);
            want.sort_by_key(key);
            assert_eq!(drawn, want, "bag {bag} was not a permutation of the bag");
        }
    }

    /// The shuffle must actually shuffle: drawing the bag in its declared
    /// order every time would pass the ratio tests and still feel canned.
    #[test]
    fn the_bag_is_actually_shuffled() {
        let mut d = Dropper::new(3);
        let first: Vec<ItemKind> = (0..BAG.len()).map(|_| d.draw()).collect();
        let mut differed = false;
        for _ in 0..50 {
            let next: Vec<ItemKind> = (0..BAG.len()).map(|_| d.draw()).collect();
            if next != first {
                differed = true;
                break;
            }
        }
        assert!(differed, "every bag came out in the same order");
    }

    /// ⚠️ L037 in a new place: an unmasked shift makes this run to millions.
    #[test]
    fn the_random_source_stays_in_range() {
        let mut d = Dropper::new(12345);
        for _ in 0..20_000 {
            let v = d.next_f32();
            assert!((0.0..1.0).contains(&v), "random value {v} out of range");
        }
    }

    /// Same seed, same run. `probe_balance` is only a measurement if the
    /// drops are reproducible.
    #[test]
    fn the_same_seed_gives_the_same_drops() {
        let draws = |seed| {
            let mut d = Dropper::new(seed);
            (0..200).map(|_| d.draw()).collect::<Vec<_>>()
        };
        assert_eq!(draws(42), draws(42));
        assert_ne!(draws(42), draws(43), "different seeds should differ");
    }

    #[test]
    fn drop_chance_climbs_with_the_level_and_clamps() {
        let d = Dropper::new(1);
        assert!((d.drop_chance(1, 10) - DROP_CHANCE_LOW).abs() < 1e-6);
        assert!((d.drop_chance(10, 10) - DROP_CHANCE_HIGH).abs() < 1e-6);
        for level in 2..=10 {
            assert!(d.drop_chance(level, 10) > d.drop_chance(level - 1, 10));
        }
        assert_eq!(d.drop_chance(0, 10), d.drop_chance(1, 10), "below range clamps");
        assert_eq!(d.drop_chance(99, 10), d.drop_chance(10, 10), "above range clamps");
    }

    /// Rolling produces drops at roughly the stated rate — the bag governs
    /// WHAT falls, this governs WHETHER anything does.
    #[test]
    fn rolls_land_near_the_stated_chance() {
        for (level, want) in [(1u32, DROP_CHANCE_LOW), (10, DROP_CHANCE_HIGH)] {
            let mut d = Dropper::new(2024);
            let n = 20_000;
            let drops = (0..n)
                .filter(|_| d.roll(Vec2::ZERO, level, 10).is_some())
                .count();
            let rate = drops as f32 / n as f32;
            assert!(
                (rate - want).abs() < 0.02,
                "level {level}: rate {rate:.3} vs stated {want:.3}"
            );
        }
    }

    #[test]
    fn grow_strengths_are_ordered_in_both_size_and_time() {
        let s = [Strength::Small, Strength::Medium, Strength::Large];
        for w in s.windows(2) {
            assert!(w[1].seconds() > w[0].seconds());
            assert!(w[1].scale() > w[0].scale());
        }
        assert!(s[0].scale() > 1.0, "a grow must actually grow");
        assert!(BOMB_SCALE < 1.0, "a bomb must actually shrink");
    }

    /// An item must not fall at ball pace — motion is the first cue the eye
    /// uses to tell them apart.
    #[test]
    fn items_fall_far_slower_than_any_ball() {
        assert!(
            FALL_SPEED < crate::state::BALL_SPEED * 0.75,
            "items at {FALL_SPEED} are too close to ball pace"
        );
    }
}
