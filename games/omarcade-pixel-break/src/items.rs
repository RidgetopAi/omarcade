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
/// All four the plan calls for. S5 shipped the two that resize the paddle;
/// S6 adds the two that do not — which is the reason the magnet gets a
/// timer of its own rather than riding `paddle_effect_left`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Grow(Strength),
    /// Narrows the paddle. The only bad drop.
    Bomb,
    /// A caught ball sticks to the paddle until Space is released, or
    /// three seconds pass. Lasts 20 s.
    Magnet,
    /// One more ball, up to the level's cap. The only item with no
    /// duration: it changes the field, not the paddle.
    Omarchy,
}

impl ItemKind {
    pub fn is_bad(self) -> bool {
        matches!(self, ItemKind::Bomb)
    }

    /// Seconds the effect lasts once caught.
    ///
    /// ⚠️ Omarchy is 0.0 and that is not a placeholder — it spends itself
    /// the instant it is caught. Routing it through a timer would make
    /// catching one silently cancel a running grow.
    pub fn seconds(self) -> f32 {
        match self {
            ItemKind::Grow(s) => s.seconds(),
            ItemKind::Bomb => BOMB_SECONDS,
            ItemKind::Magnet => MAGNET_SECONDS,
            ItemKind::Omarchy => 0.0,
        }
    }

    /// How wide this item is, in field units.
    ///
    /// Size is a property of the KIND, like `seconds` and `is_bad` above,
    /// so catching, culling and drawing cannot disagree about it — they
    /// all go through `Item::rect`.
    pub fn width(self) -> f32 {
        match self {
            ItemKind::Omarchy => OMARCHY_SIZE,
            _ => ITEM_W,
        }
    }

    /// How tall this item is, in field units.
    pub fn height(self) -> f32 {
        match self {
            ItemKind::Omarchy => OMARCHY_SIZE,
            _ => ITEM_H,
        }
    }

    /// Whether this item resizes the paddle.
    ///
    /// The grow/bomb axis is one scale and one timer (see `GameState`);
    /// the magnet and the Omarchy item deliberately sit outside it, so a
    /// caught bomb must not clear a magnet the player is still holding.
    pub fn resizes_paddle(self) -> bool {
        matches!(self, ItemKind::Grow(_) | ItemKind::Bomb)
    }
}

/// How long a caught bomb keeps the paddle narrow.
pub const BOMB_SECONDS: f32 = 15.0;

/// How long the magnet stays armed once caught.
///
/// ⚠️ This is the powerup's life, NOT the hold. A ball sticks for at most
/// `MAGNET_HOLD_SECONDS`; the magnet itself keeps catching balls for this
/// long. Confusing the two makes a 20 s weld instead of a 20 s ability.
pub const MAGNET_SECONDS: f32 = 20.0;

/// How long one ball may stay stuck before it fires itself.
///
/// ⚠️ **The auto-release is the feature, not a limitation.** Brian settled
/// this. Without it the optimal play at level 9 is catch-aim-release on
/// every single bounce: strictly better, much slower, and it replaces the
/// skill in the game with patience. Do not make the hold unlimited.
pub const MAGNET_HOLD_SECONDS: f32 = 3.0;
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

/// The Omarchy item is drawn — and caught — as a large square.
///
/// ⚠️ **This is a deliberate gameplay change, not just art.** The mark is
/// a 15x15 grid, and in the default 34x16 box it scaled to 16 units: about
/// one screen pixel per logo cell, which turned the whole glyph into a
/// smudge. Brian's verdict on seeing it in play was that you cannot tell
/// what it is. At 44 each cell gets nearly three pixels and the mark
/// actually reads.
///
/// The cost is honest: this item's catch box is 2.75x taller and 1.3x
/// wider than the bars', so the rarest and most wanted drop is also the
/// easiest to catch. That is a reasonable thing for a reward to be, but it
/// IS a change — `probe_balance` moves because of it, and it should.
pub const OMARCHY_SIZE: f32 = 44.0;

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
///
/// ⚠️ **The ratio is load-bearing and the bag width is not.** S6 widened
/// this from five to ten so the magnet and the Omarchy item could join
/// without diluting the bombs: two in ten is the same 1:4 as one in five.
/// Adding a good item without adding its share of bombs would quietly make
/// the game easier, and the plan calls this ratio the single most-felt
/// tuning decision in the feature.
///
/// The Omarchy item is deliberately ONE in ten. It is the +1 ball, and the
/// endgame tail is the problem it exists to solve — but a field that is
/// always full of balls is not a treat, it is a different game.
const BAG: [ItemKind; 10] = [
    ItemKind::Grow(Strength::Small),
    ItemKind::Grow(Strength::Medium),
    ItemKind::Bomb,
    ItemKind::Grow(Strength::Small),
    ItemKind::Grow(Strength::Large),
    ItemKind::Grow(Strength::Small),
    ItemKind::Grow(Strength::Medium),
    ItemKind::Bomb,
    ItemKind::Magnet,
    ItemKind::Omarchy,
];

/// How many times a bag may be reshuffled before falling back to a
/// legal-by-construction arrangement. Measured worst case is 6.
const MAX_SHUFFLE_ATTEMPTS: usize = 50;

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
    ///
    /// ⚠️ Sized from the KIND, so the Omarchy item's larger box applies to
    /// catching and culling too — not just to how it is drawn. A rect that
    /// disagreed with the art would mean catching thin air, or missing
    /// something the player clearly touched.
    pub fn rect(&self) -> crate::geom::Rect {
        crate::geom::Rect::from_center(self.pos, self.kind.width() / 2.0, self.kind.height() / 2.0)
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

    /// Refill the bag with one of each entry, shuffled — and reshuffle
    /// until the result is legal.
    ///
    /// ⚠️ **The guarantee is stated ONCE, in `is_legal`, and enforced by
    /// rejection.** The obvious implementation — shuffle, then swap
    /// offending items apart — was tried first and was wrong in a way that
    /// took 210 draws to show: there are two repairs (one for a pair
    /// *inside* the bag, one for the *seam* between bags), and each could
    /// undo the other. Every within-bag assertion passed while the seam
    /// quietly broke. Rejection cannot fight itself: a bag either
    /// satisfies both properties or it is discarded.
    ///
    /// Two ways two bombs end up back to back, both covered by `is_legal`:
    ///
    /// 1. **Inside a bag.** With one bomb per bag this was impossible and
    ///    the shuffle needed no help. S6's ten-item bag holds TWO, so the
    ///    shuffle can now deal them adjacent by itself — the property the
    ///    bag exists for stopped being free the moment the bag widened.
    /// 2. **Across the seam.** Bag N can end with a bomb and bag N+1 begin
    ///    with one. Drawing is from the BACK, so "first out of the next
    ///    bag" is its LAST element.
    ///
    /// Reshuffling only reorders, so every bag stays a true permutation of
    /// `BAG` and the 1:4 ratio is exact regardless of how many attempts it
    /// takes. Measured over 500 seeds: mean 1.7 attempts, worst case 6.
    fn refill(&mut self) {
        let last_was_bomb = self.last_was_bad;
        for _ in 0..MAX_SHUFFLE_ATTEMPTS {
            self.bag.clear();
            self.bag.extend_from_slice(&BAG);
            // Fisher-Yates, back to front.
            for i in (1..self.bag.len()).rev() {
                let j = self.next_below(i + 1);
                self.bag.swap(i, j);
            }
            if Self::is_legal(&self.bag, last_was_bomb) {
                return;
            }
        }
        // Ratios too tight to reject are not reachable with the shipped
        // bag, but a future one could be. Spreading the bad items evenly
        // is legal by construction, so the guarantee holds even here.
        self.bag = Self::spread_evenly(last_was_bomb);
    }

    /// Both properties the bag promises, in one place.
    fn is_legal(bag: &[ItemKind], last_was_bomb: bool) -> bool {
        if bag.windows(2).any(|w| w[0].is_bad() && w[1].is_bad()) {
            return false;
        }
        // The next item drawn is the LAST element.
        !(last_was_bomb && bag.last().is_some_and(|k| k.is_bad()))
    }

    /// A legal-by-construction arrangement: bad items spaced out among the
    /// good ones. The fallback when rejection cannot find one.
    fn spread_evenly(last_was_bomb: bool) -> Vec<ItemKind> {
        let (bad, good): (Vec<ItemKind>, Vec<ItemKind>) =
            BAG.iter().partition(|k| k.is_bad());
        let step = (good.len() / bad.len().max(1)).max(1);
        let mut out: Vec<ItemKind> = Vec::with_capacity(BAG.len());
        let mut taken = 0;
        for b in &bad {
            let upto = (taken + step).min(good.len());
            out.extend_from_slice(&good[taken..upto]);
            taken = upto;
            out.push(*b);
        }
        out.extend_from_slice(&good[taken..]);
        if last_was_bomb && out.last().is_some_and(|k| k.is_bad()) {
            out.rotate_right(1);
        }
        out
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

    /// One bad item for every four good ones, held exactly over any whole
    /// number of bags.
    ///
    /// ⚠️ Asserted as a RATIO, not as a count. S6 widened the bag from five
    /// to ten and a hardcoded "one bomb per bag" would have passed straight
    /// through the change while the game got measurably easier.
    #[test]
    fn the_bag_holds_the_one_in_four_ratio() {
        let bombs_per_bag = BAG.iter().filter(|k| k.is_bad()).count();
        assert_eq!(
            bombs_per_bag * 4,
            BAG.len() - bombs_per_bag,
            "the bag itself must be one bad item to four good"
        );

        let mut d = Dropper::new(7);
        let bags = 400;
        let mut bombs = 0;
        for _ in 0..bags * BAG.len() {
            if d.draw().is_bad() {
                bombs += 1;
            }
        }
        assert_eq!(
            bombs,
            bags * bombs_per_bag,
            "exactly {bombs_per_bag} bombs per bag of {}",
            BAG.len()
        );
    }

    /// ⚠️ The reason `separate_bad` exists. With ONE bomb per bag the
    /// no-two-in-a-row property was free — the shuffle could not violate
    /// it. With TWO it can, so the guarantee now has to be enforced rather
    /// than assumed. This test fails on the plain Fisher-Yates the
    /// five-item bag used.
    #[test]
    fn a_widened_bag_still_cannot_deal_two_bombs_in_a_row() {
        assert!(
            BAG.iter().filter(|k| k.is_bad()).count() >= 2,
            "this test only means something once a bag holds several bombs"
        );
        // Many seeds, because an unguarded shuffle fails only sometimes and
        // a single seed can get lucky for a long time.
        for seed in 0..200 {
            let mut d = Dropper::new(seed);
            let mut last_was_bomb = false;
            for i in 0..400 {
                let bomb = d.draw().is_bad();
                assert!(
                    !(bomb && last_was_bomb),
                    "seed {seed} dealt two bombs in a row at draw {i}"
                );
                last_was_bomb = bomb;
            }
        }
    }

    /// The magnet and the Omarchy item actually appear — a bag entry that
    /// never comes out is a feature nobody can reach.
    #[test]
    fn every_kind_in_the_bag_is_actually_dealt() {
        let mut d = Dropper::new(4242);
        let mut saw_magnet = false;
        let mut saw_omarchy = false;
        for _ in 0..500 {
            match d.draw() {
                ItemKind::Magnet => saw_magnet = true,
                ItemKind::Omarchy => saw_omarchy = true,
                _ => {}
            }
        }
        assert!(saw_magnet, "the magnet never dropped");
        assert!(saw_omarchy, "the Omarchy item never dropped");
    }

    /// ⚠️ Only grow and bomb share the paddle axis. If the magnet or the
    /// Omarchy item ever reported that it resizes the paddle, catching one
    /// would clear a running grow for no reason a player could guess.
    #[test]
    fn only_grow_and_bomb_touch_the_paddle_axis() {
        assert!(ItemKind::Grow(Strength::Small).resizes_paddle());
        assert!(ItemKind::Bomb.resizes_paddle());
        assert!(!ItemKind::Magnet.resizes_paddle());
        assert!(!ItemKind::Omarchy.resizes_paddle());
    }

    /// The Omarchy item spends itself immediately; everything else lasts.
    #[test]
    fn only_the_omarchy_item_has_no_duration() {
        assert_eq!(ItemKind::Omarchy.seconds(), 0.0);
        for kind in [
            ItemKind::Grow(Strength::Small),
            ItemKind::Bomb,
            ItemKind::Magnet,
        ] {
            assert!(kind.seconds() > 0.0, "{kind:?} should last");
        }
    }

    /// ⚠️ The hold is far shorter than the powerup. Swapping these two
    /// constants would weld a ball to the paddle for twenty seconds
    /// instead of arming the ability for twenty.
    #[test]
    fn the_hold_is_much_shorter_than_the_magnet() {
        assert!(
            MAGNET_HOLD_SECONDS < MAGNET_SECONDS / 4.0,
            "the hold ({MAGNET_HOLD_SECONDS}s) must be a brief moment inside \
             the magnet's life ({MAGNET_SECONDS}s), not a weld"
        );
        assert!(MAGNET_HOLD_SECONDS > 0.0, "an instant release is no hold at all");
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

    /// The Omarchy item is bigger than the bars, and SQUARE.
    ///
    /// Both halves matter: square is what lets the 15x15 mark scale without
    /// distortion, and bigger is what makes it legible at all.
    #[test]
    fn the_omarchy_item_is_a_larger_square() {
        assert_eq!(ItemKind::Omarchy.width(), ItemKind::Omarchy.height());
        assert!(
            ItemKind::Omarchy.height() > ITEM_H * 2.0,
            "the mark needs real pixels: {} is not much better than {ITEM_H}",
            ItemKind::Omarchy.height()
        );
        for kind in [ItemKind::Bomb, ItemKind::Magnet, ItemKind::Grow(Strength::Small)] {
            assert_eq!(kind.width(), ITEM_W, "{kind:?} should be a standard bar");
            assert_eq!(kind.height(), ITEM_H, "{kind:?} should be a standard bar");
        }
    }

    /// The catch box follows the KIND, not a shared constant.
    ///
    /// ⚠️ This is the assertion that would have caught the bug if `rect()`
    /// had been left on `ITEM_W`/`ITEM_H`: the art would have grown while
    /// catching stayed small, so the player would visibly touch the mark
    /// and not catch it.
    #[test]
    fn the_catch_box_matches_what_is_drawn() {
        let at = Vec2::new(300.0, 300.0);
        let omarchy = Item::new(at, ItemKind::Omarchy).rect();
        let bar = Item::new(at, ItemKind::Bomb).rect();

        assert_eq!(omarchy.w, OMARCHY_SIZE);
        assert_eq!(omarchy.h, OMARCHY_SIZE);
        assert_eq!(bar.w, ITEM_W);
        assert_eq!(bar.h, ITEM_H);

        // Same centre, so the bigger box grows in every direction rather
        // than hanging off one edge.
        assert_eq!(omarchy.center(), bar.center());
    }

    /// A bigger item must still fit the field wherever a brick can drop it.
    ///
    /// ⚠️ Bricks are laid out centred, so the outermost columns sit closest
    /// to the walls. If the widest item overhung from there it would be
    /// drawn clipped, and the glow around it more so.
    #[test]
    fn the_widest_item_fits_at_the_outermost_brick() {
        let total_w = crate::state::BRICK_COLS as f32 * crate::state::BRICK_W
            + (crate::state::BRICK_COLS - 1) as f32 * crate::state::BRICK_GAP;
        let x0 = (crate::state::FIELD_W - total_w) / 2.0;
        let leftmost = x0 + crate::state::BRICK_W / 2.0;
        let rightmost = x0 + (crate::state::BRICK_COLS - 1) as f32
            * (crate::state::BRICK_W + crate::state::BRICK_GAP)
            + crate::state::BRICK_W / 2.0;

        let widest = OMARCHY_SIZE.max(ITEM_W);
        assert!(leftmost - widest / 2.0 >= 0.0, "an item would overhang the left wall");
        assert!(
            rightmost + widest / 2.0 <= crate::state::FIELD_W,
            "an item would overhang the right wall"
        );
    }
}
