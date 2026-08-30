//! The game's pixel art, authored as text.
//!
//! Every sprite is a grid of characters plus a palette. `.` is
//! transparent. The point of writing art this way is that it is
//! reviewable and correctable in words — "the rear wing is one row too
//! high", "the cockpit should be two pixels narrower" — without opening
//! an image editor, and a wrong pixel is visible in the diff.
//!
//! **These are our own designs.** The technique is Pole Position's; the
//! shapes are not. Nothing here is traced from or measured against
//! Namco's art.
//!
//! Colours come from the live Omarchy theme wherever the shape allows
//! it, so the suite stays theme-reactive. A car's own livery is fixed —
//! a red car that turns green with the desktop theme stops being a
//! recognisable object — but the shadow, glass and tyre tones are
//! derived from the theme's background so the car sits in its scene
//! rather than on top of it.

use omarcade_core::sprite::{PaletteEntry, Sprite};
use omarcade_core::{Color, Theme};

/// The player's car, seen from behind.
///
/// 48 wide by 30 tall. Read the silhouette top to bottom: rear wing,
/// engine cover and roll hoop, the body with side pods, then the four
/// tyres with the rear pair widest. Open-wheel, so the tyres stand
/// clear of the body — that is what makes the shape read as a race car
/// at 8 pixels tall from a distance.
///
/// Legend:
///   B body (primary livery)   D body shadow / lower panels
///   A accent stripe           G glass / intake
///   T tyre                    H tyre highlight (top curve)
///   L brake light             W wing
pub const PLAYER_CAR: &[&str] = &[
    "................................................",
    "................................................",
    "................................................",
    "..............WWWWWWWWWWWWWWWWWWWW..............",
    "..............WSSSSSSSSSSSSSSSSSSW..............",
    "..............WDDDDDDDDDDDDDDDDDDW..............",
    "..................DD........DD..................",
    "..................DD........DD..................",
    "............SSSSSSSSSSSSSSSSSSSSSSSS............",
    "...........SSBBBBBBBBBBBBBBBBBBBBBBSS...........",
    ".......th..SBBGGGGGGGGGGGGGGGGGGGGBBS..ht.......",
    "......thht.SBBGGGGGGGGGGGGGGGGGGGGBBS.thht......",
    "......thht.SBBBBBBBBBBBBBBBBBBBBBBBBS.thht......",
    ".......th..SBBBBBBBBBBBBBBBBBBBBBBBBS..ht.......",
    ".........SSBBBBBBBAAAAAAAAAAAABBBBBBBSS.........",
    ".......SSBBBBBBBBBAAAAAAAAAAAABBBBBBBBBSS.......",
    ".....SSBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBSS.....",
    "....SBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBS....",
    "....SBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBS....",
    "....SBDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDBS....",
    "..TTSBDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDBSTT..",
    ".THHHTDDDDDDLLLLDDDDDDDDDDDDDDDDLLLLDDDDDDTHHHT.",
    "THHHHTDDDDDDLLLLDDDDDDDDDDDDDDDDLLLLDDDDDDTHHHHT",
    "THHHHTDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDTHHHHT",
    "THHHHTDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDTHHHHT",
    "THHHTTDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDTTHHHT",
    "TTTTTT....................................TTTTTT",
    ".TTTT......................................TTTT.",
    "................................................",
    "................................................",
];

/// An opponent car. Same chassis, plainer read.
///
/// Deliberately a simpler silhouette than the player's: at speed the
/// player needs to tell "that is me" from "that is traffic" instantly,
/// and shape does that faster than colour.
pub const RIVAL_CAR: &[&str] = &[
    "................................................",
    "................................................",
    "................................................",
    "................................................",
    "...............WWWWWWWWWWWWWWWWWW...............",
    "...............WDDDDDDDDDDDDDDDDW...............",
    "..................DD........DD..................",
    "..................DD........DD..................",
    "............SSSSSSSSSSSSSSSSSSSSSSSS............",
    "...........SSBBBBBBBBBBBBBBBBBBBBBBSS...........",
    ".......th..SBBGGGGGGGGGGGGGGGGGGGGBBS..ht.......",
    "......thht.SBBGGGGGGGGGGGGGGGGGGGGBBS.thht......",
    "......thht.SBBBBBBBBBBBBBBBBBBBBBBBBS.thht......",
    ".......th..SBBBBBBBBBBBBBBBBBBBBBBBBS..ht.......",
    ".........SSBBBBBBBBBBBBBBBBBBBBBBBBBBSS.........",
    ".......SSBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBSS.......",
    ".....SSBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBSS.....",
    "....SBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBS....",
    "....SBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBS....",
    "....SBDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDBS....",
    "..TTSBDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDBSTT..",
    ".THHHTDDDDDDLLLLDDDDDDDDDDDDDDDDLLLLDDDDDDTHHHT.",
    "THHHHTDDDDDDLLLLDDDDDDDDDDDDDDDDLLLLDDDDDDTHHHHT",
    "THHHHTDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDTHHHHT",
    "THHHHTDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDTHHHHT",
    "THHHTTDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDTTHHHT",
    "TTTTTT....................................TTTTTT",
    ".TTTT......................................TTTT.",
    "................................................",
    "................................................",
];

/// A roadside marker post — the cheapest thing that sells speed.
///
/// Small, high-contrast, and passing constantly. The eye reads speed
/// from things streaming past at the edge of the road far more than
/// from the road surface itself.
pub const MARKER_POST: &[&str] = &[
    "..LL..",
    "..LL..",
    "..DD..",
    "..DD..",
    "..DD..",
    "..DD..",
    "..DD..",
    "..DD..",
];

/// A palette for one car livery.
///
/// `body` is the car's own colour and does NOT follow the theme: a car
/// that changes colour with the desktop stops being a recognisable
/// object. Everything else is derived, so the car sits in the scene's
/// light rather than on top of it.
pub fn car_palette(theme: &Theme, body: Color, accent: Color) -> Vec<PaletteEntry> {
    // The shadowed underside: the body colour pushed toward the scene's
    // darkest tone, so lighting reads as lighting rather than as a
    // second flat colour.
    let shadow = body.lerp(theme.darker_background, 0.45);
    // The cockpit opening, not glass: an open-wheel car seen from
    // behind shows a dark hole with the driver in it. Reading it as
    // tinted glass made a murky band that looked like a mistake.
    let glass = theme.darker_background.lerp(body, 0.15);
    let tyre = theme.darker_background.lerp(Color::BLACK, 0.35);
    // A narrow catch-light on the top of the tyre, not a broad grey
    // slab. Too much of it and the wheels stop reading as round rubber
    // and start reading as painted panels.
    let tyre_top = tyre.lerp(theme.foreground, 0.14);

    vec![
        ('B', body),
        ('D', shadow),
        // The lit upper surface. At 32x20 there was no room for a third
        // body tone; at 48x30 a top-lit gradient is what stops the car
        // reading as a flat cutout.
        ('S', body.lerp(theme.foreground, 0.22)),
        ('A', accent),
        ('G', glass),
        ('T', tyre),
        ('H', tyre_top),
        // Front tyres: the same rubber a little further away, so they
        // read as forward of the rears rather than as a second pair of
        // the same thing.
        ('t', tyre.lerp(theme.darker_background, 0.30)),
        ('h', tyre_top.lerp(theme.darker_background, 0.45)),
        ('L', theme.red.lerp(Color::rgb(255, 90, 70), 0.5)),
        ('W', shadow.lerp(theme.foreground, 0.18)),
    ]
}

/// Palette for the roadside markers.
pub fn post_palette(theme: &Theme) -> Vec<PaletteEntry> {
    vec![
        ('L', theme.foreground),
        ('D', theme.red.lerp(theme.darker_background, 0.25)),
    ]
}

/// Every sprite the game ships, built once.
///
/// Built eagerly on purpose: `Sprite::new` panics on malformed art, so
/// constructing the whole set is what turns a typo in a grid into an
/// immediate failure rather than a hole in a screenshot.
pub struct Art {
    pub player: Sprite,
    pub rival: Sprite,
    pub post: Sprite,
}

impl Art {
    pub fn load(theme: &Theme) -> Art {
        let player_pal = car_palette(theme, Color::rgb(214, 78, 62), Color::rgb(240, 214, 120));
        let rival_pal = car_palette(theme, Color::rgb(74, 128, 200), Color::rgb(210, 220, 235));

        Art {
            player: Sprite::new(PLAYER_CAR, &player_pal),
            rival: Sprite::new(RIVAL_CAR, &rival_pal),
            post: Sprite::new(MARKER_POST, &post_palette(theme)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every sprite must parse. `Sprite::new` panics on a ragged row or
    /// an unpalletted character, so this is the test that catches a typo
    /// in the art before it reaches a screenshot.
    #[test]
    fn all_art_parses() {
        let art = Art::load(&Theme::fallback());
        // 48x30 rather than the original 32x20: at the coarser grid there
        // was no room for curved bodywork, a wing with thickness, or a
        // third body tone, and the result read as blocky next to any
        // real arcade sprite.
        assert_eq!(art.player.width(), 48);
        assert_eq!(art.player.height(), 30);
        assert_eq!(art.rival.width(), 48);
        assert!(art.player.ink() > 400, "the car should have real substance");
    }

    #[test]
    fn the_cars_are_the_same_size() {
        // They share a road and a scale factor; different dimensions
        // would make one sit wrong relative to the other.
        let art = Art::load(&Theme::fallback());
        assert_eq!(art.player.width(), art.rival.width());
        assert_eq!(art.player.height(), art.rival.height());
    }

    #[test]
    fn art_follows_the_theme_without_changing_its_livery() {
        // A car's own colour must be stable across themes — a red car
        // that turns green with the desktop stops being an object. But
        // its shadow and glass SHOULD move, or it floats above the scene.
        let light = Theme::fallback();
        let mut dark = Theme::fallback();
        dark.darker_background = Color::rgb(0, 0, 0);
        dark.cyan = Color::rgb(0, 255, 255);

        let body = Color::rgb(214, 78, 62);
        let a = car_palette(&light, body, Color::WHITE);
        let b = car_palette(&dark, body, Color::WHITE);

        let find = |p: &[PaletteEntry], c: char| p.iter().find(|(k, _)| *k == c).unwrap().1;
        assert_eq!(find(&a, 'B'), find(&b, 'B'), "livery must not follow the theme");
        assert_ne!(find(&a, 'D'), find(&b, 'D'), "shadow must follow the theme");
    }
}
