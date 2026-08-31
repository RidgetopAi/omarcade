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
/// 64 wide by 40 tall. A closed-cockpit GT rather than an open-wheeler:
/// roof and rear glass at the top, a full-width wing on two posts, then
/// the tail panel with lights, and a diffuser under it between covered
/// rear wheels.
///
/// The wing is the part that took the longest to get right. It works
/// here because there is SKY UNDER IT and the roof line stops well
/// above — body rows immediately under a bar read as a roof, whatever
/// the gap. See the session-6 notes.
///
/// Legend:
///   B body (primary livery)   D body shadow / lower panels
///   A accent stripe           G glass / rear window
///   S lit upper surface       C mid tone, canopy surround + fins
///   T tyre                    H highlight (diffuser fins)
///   R tread — same colour as H, animated by roll
///   L brake light             W wing
///   E vent band under the wing
///   F near-black, the diffuser recesses
pub const PLAYER_CAR: &[&str] = &[
    "................................................................",
    "................................................................",
    "................................................................",
    "................................................................",
    "................................................................",
    "................................................................",
    "................................................................",
    "................................................................",
    "................................................................",
    "................................................................",
    "................................................................",
    "................................................................",
    "................................................................",
    "................................................................",
    "................................................................",
    "................................................................",
    "................................................................",
    "............................SSSSSSSS............................",
    ".........................SSSBBBBBBBBSSS.........................",
    ".......................SBBBCCCCCCCCCCBBBS.......................",
    "......................SBBCCGGGGGGGGGGCCBBS......................",
    "..............L......SBBCGGGGGGGGGGGGGGCBBS......L..............",
    "..............BSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSB..............",
    "..............BWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWB..............",
    "..............BWWWWWWDDWWWWWWWWAAWWWWWWWWDDWWWWWWB..............",
    "..............BEEEEEETTEEEEEEEEEEEEEEEEEETTEEEEEEB..............",
    "...................SBTTBBBDDDDDDDDDDDDBBBTTBS...................",
    "................SSBBDTTDBBBDDDDDDDDDDBBBDTTDBBSS................",
    "...............BSBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBSB...............",
    "..............BSBBBFFFFFFFFFFFFFFFFFFFFFFFFFFBBBSB..............",
    "...........TTTTTBBFFFFFFFDDDDDSSSSDDDDDFFFFFFFBBTTTTT...........",
    "..........TRRTRTBBFSSSFSSFDDDDLLLLDDDDFSSFSSSFBBTRTRRT..........",
    "..........TTTTTTTBFLLLFLLFFDDDDDDDDDDFFLLFLLLFBTTTTTTT..........",
    "..........TRRTRTRBFFFFFFFFFFFFFFFFFFFFFFFFFFFFBRTRTRRT..........",
    "..........TTTTTTTBDDDCHHHHHHCCCHHCCCHHHHHHCDDDBTTTTTTT..........",
    "..........TRRTRTRTDCCCFFCFHCFFFCCFFFCHFCFFCCCDTRTRTRRT..........",
    "..........TTTTTTTTTCCCHFCFHCFFFCCFFFCHFCFHCCCTTTTTTTTT..........",
    "..........TRRTRTRTFFFFFFCFFFCCCHHCCCFFFCFFFFFFTRTRTRRT..........",
    "...........TTTTTTT......FF............FF......TTTTTTT...........",
    "................................................................",
];


/// The start/finish gantry — the structure that spans the road.
///
/// 160 wide by 60 tall, drawn in the sprite playground. Unlike a prop
/// this does not stand *beside* the road, it stands *over* it: two
/// lattice legs at the verges carrying a banner across the full width,
/// with a chequered band and a signal bar on it.
///
/// ⚠️ The ink does not fill the grid. It occupies columns 17..=142 and
/// rows 0..=56 — 17 blank columns each side and 3 blank rows below. A
/// caller that scales this by GRID width to span the road will draw a
/// structure only ~79% of the road wide. Scale by the ink, or trim the
/// padding, but do not scale the raw width and assume it spans.
///
/// Legend (these letters are NOT the car's — see `gantry_palette`):
///   E leg lattice, main     C leg lattice, shadow
///   J leg upright, darker   A banner field
///   F banner red            I banner red, shadowed
///   K chequer white         T chequer dark
pub const GANTRY: &[&str] = &[
    ".................EEEEEEEFFFFFFFFFFFFFTTKKTTKKTTKKAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAKKTTKKTTKKTTFFFFFFFFFFFFFEEEEEEE.................",
    ".................EC...EEFFF........FFTTKKTTKKTTKKFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFKKTTKKTTKKTTFF........FFFEE...CE.................",
    ".................E.C.E.EFIFIIIIIIIFFFKKTTKKTTKKTTFAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFTTKKTTKKTTKKFFFIIIIIIIFIFE.E.C.E.................",
    ".................E..E..EFI.F.....FFIFKKTTKKTTKKTTFAAAAAFFFFFFAAAAFFFFFFFFAAAAAFFFFFAAAAAFFFFFFFAAAFFFFFFFFAAAAFTTKKTTKKTTKKFIFF.....F.IFE..E..E.................",
    ".................E.E.C.EFI..F...FF.IFTTKKTTKKTTKKFAAAAAFAAAAFFAAAFFFFFFFFAAAAAFAAFFAAAAAFAAAAFFAAAFFFFFFFFAAAAFKKTTKKTTKKTTFI.FF...F..IFE.C.E.E.................",
    ".................EE...CEFI...F.FFII.FTTKKTTKKTTKKFAAAAAFAAAAAFAAAAAAFFAAAAAAAAFAAAFAAAAAFAAAAAFAAAAAAFFAAAAAAAFKKTTKKTTKKTTF.IIFF.F...IFEC...EE.................",
    ".................EEC..CEFI...FFFII..FKKTTKKTTKKTTFAAAAAFFAAAAAAAAAAAFFAAAAAAAFFAAAFAAAAAFAAAAAFAAAAAAFFAAAAAAAFTTKKTTKKTTKKF..IIFFF...IFEC..CEE.................",
    ".................EEEEEEEFI..FF.FI...FKKTTKKTTKKTTFAAAAAFFFFFFFAAAAAAFFAAAAAAAFFFFFFAAAAAFFFFFFFAAAAAAFFAAAAAAAFTTKKTTKKTTKKF...IF.FF..IFEEEEEEE.................",
    ".................EC...EEFI.FF..IF...FTTKKTTKKTTKKFAAAAAAAAAAAFAAAAAAFFAAAAAAFFFFFFFFAAAAFAAAFFAAAAAAAFFAAAAAAAFKKTTKKTTKKTTF...FI..FF.IFEE...CE.................",
    ".................E.C.E.EFI.F..II.F..FTTKKTTKKTTKKFAAAAAFFAAAAFAAAAAAFFAAAAAAFFFFFFFFAAAAFAAAAFAAAAAAAFFAAAAAAAFKKTTKKTTKKTTF..F.II..F.IFE.E.C.E.................",
    ".................E..E..EFIF..II...F.FKKTTKKTTKKTTFAAAAAFFFFFFFAAAAAAFFAAAAAFFAAAAAAFFAAAFAAAAFFAAAAAAFFAAAAAAAFTTKKTTKKTTKKF.F...II..FIFE..E..E.................",
    ".................E.E.C.EFFF.I......FFKKTTKKTTKKTTFAAAAAAFFFFFFAAAAAAFFAAAAAFFAAAAAAAFAAAFAAAAFFAAAAAAFFAAAAAAAFTTKKTTKKTTKKFF......I.FFFE.C.E.E.................",
    ".................EE...CEFFFFFFFFFFFFFTTKKTTKKTTKKFAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFKKTTKKTTKKTTFFFFFFFFFFFFFEC...EE.................",
    ".................EE....EJJIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIJJE....EE.................",
    ".................EEEEEEEJJ............................................................................................................JJEEEEEEE.................",
    ".................EC...EEJJ............................................................................................................JJEE...CE.................",
    ".................E.C.E.EJJ............................................................................................................JJE.E.C.E.................",
    ".................E..E..EJJ............................................................................................................JJE..E..E.................",
    ".................E.E.C.EJJ............................................................................................................JJE.C.E.E.................",
    ".................EE...CEJJ............................................................................................................JJEC...EE.................",
    ".................EE....EJC............................................................................................................CJE....EE.................",
    ".................EEEEEEEJJ............................................................................................................JJEEEEEEE.................",
    ".................EC...EEJJ............................................................................................................JJEE...CE.................",
    ".................ECC.E.EJJ............................................................................................................JJE.E.CCE.................",
    ".................ECCE..EJJ............................................................................................................JJE..ECCE.................",
    ".................E.E.C.EJJ............................................................................................................JJE.C.E.E.................",
    ".................EE...CEJJ............................................................................................................JJEC...EE.................",
    ".................EE....EJC............................................................................................................CJE....EE.................",
    ".................EEEEEEEJJ............................................................................................................JJEEEEEEE.................",
    ".................EC...EEJJ............................................................................................................JJEE...CE.................",
    ".................ECC.E.EJJ............................................................................................................JJE.E.CCE.................",
    ".................E..E..EJJ............................................................................................................JJE..E..E.................",
    ".................E.E.C.EJJ............................................................................................................JJE.C.E.E.................",
    ".................EE...CEJJ............................................................................................................CJEC...EE.................",
    ".................EEEEEEEJC............................................................................................................JJEEEEEEE.................",
    ".................EC...EEJJ............................................................................................................JJEE...CE.................",
    ".................ECC.E.EJJ............................................................................................................JJE.E.CCE.................",
    ".................EC.E..EJJ............................................................................................................JJE..E.CE.................",
    ".................E.E.C.EJJ............................................................................................................JJE.C.E.E.................",
    ".................EE...CEJJ............................................................................................................JJEC...EE.................",
    ".................EEEEEEEJC............................................................................................................CJEEEEEEE.................",
    ".................EC...EEJJ............................................................................................................JJEE...CE.................",
    ".................ECC.E.EJJ............................................................................................................JJE.E.CCE.................",
    ".................E..E..EJJ............................................................................................................JJE..E..E.................",
    ".................E.E.C.EJJ............................................................................................................JJE.C.E.E.................",
    ".................EE...CEJJ............................................................................................................JJEC...EE.................",
    ".................EEEEEEEJC............................................................................................................CJEEEEEEE.................",
    ".................ECC..EEJJ............................................................................................................JJEE...CE.................",
    ".................ECCCE.EJJ............................................................................................................JJEEE.C.E.................",
    ".................E.EE..EJJ............................................................................................................JJE..E..E.................",
    ".................E.E.C.EJJ............................................................................................................JJE.C.E.E.................",
    ".................EE...CEJJ............................................................................................................JJEC...EE.................",
    ".................EEEEEEEJJ............................................................................................................JJEEEEEEE.................",
    ".................EC....EJJ............................................................................................................JJE.....E.................",
    ".................EC....EJJ............................................................................................................JJE.....E.................",
    ".................EC....EJJ............................................................................................................JJE.....E.................",
    ".................E.....EJ..............................................................................................................JE.....E.................",
    "................................................................................................................................................................",
    "................................................................................................................................................................",
    "................................................................................................................................................................",
];

/// A palette for the gantry.
///
/// Separate from [`car_palette`] on purpose, and not merged into it:
/// this art re-uses letters the car already owns with different
/// meanings — `E` is the car's vent red but the gantry's lattice, `F`
/// is the car's near-black but the gantry's banner red, `T` is a tyre
/// but here a chequer square. One shared palette would silently repaint
/// one of the two.
///
/// The structure is a fixed livery, like a car body: a start gantry that
/// changed colour with the desktop would stop reading as the same
/// landmark lap after lap. Only the lattice takes the theme, and only
/// through `theme.green`, so it sits in the scene's light.
pub fn gantry_palette(theme: &Theme) -> Vec<PaletteEntry> {
    let lattice_shadow = Color::rgb(0x2a, 0x49, 0xe5);
    let lattice = lattice_shadow.lerp(theme.green, 0.59);
    let red = Color::rgb(0xaf, 0x12, 0x12);

    vec![
        ('E', lattice),                          // lattice, lit
        ('C', lattice_shadow),                   // lattice, shadowed
        ('J', lattice.lerp(Color::BLACK, 0.21)), // upright, darker still
        ('A', theme.foreground),                 // banner field
        ('F', red),                              // banner red
        ('I', red.lerp(Color::BLACK, 0.35)),     // banner red, shadowed
        ('K', Color::WHITE),                     // chequer, light
        ('T', Color::rgb(0x1a, 0x1a, 0x1a)),     // chequer, dark
    ]
}

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

/// A taller roadside pole. Same language as the marker post, twice the
/// height, so the two read as a set rather than as one thing repeated.
pub const TALL_POLE: &[&str] = &[
    "..LL..",
    "..LL..",
    "..HH..",
    "..DD..",
    "..DD..",
    "..DD..",
    "..DD..",
    "..DD..",
    "..DD..",
    "..DD..",
    "..DD..",
    "..DD..",
];

/// A low marker block — a kerb stone or bollard.
///
/// Deliberately short. A field of props all the same height reads as a
/// fence; mixed heights read as scenery.
pub const MARKER_BLOCK: &[&str] = &[
    "LLLL",
    "HHHH",
    "DDDD",
    "DDDD",
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
        // Tread. The SAME colour as 'H', deliberately — it is a separate
        // letter only so the roll animation can find the wheels without
        // also strobing the diffuser highlights, which were 'H' too.
        // Nothing about the still image changes.
        ('R', tyre_top),
        // Front tyres: the same rubber a little further away, so they
        // read as forward of the rears rather than as a second pair of
        // the same thing.
        ('t', tyre.lerp(theme.darker_background, 0.30)),
        ('h', tyre_top.lerp(theme.darker_background, 0.45)),
        ('L', theme.red.lerp(Color::rgb(255, 90, 70), 0.5)),
        ('W', shadow.lerp(theme.foreground, 0.18)),
        // A mid tone between the body and the scene, for the canopy
        // surround and the diffuser fins — the places that need to read
        // as neither lit bodywork nor a hole.
        ('C', shadow.lerp(theme.muted, 0.37)),
        // The vent band under the wing. Pinned rather than derived: it
        // is a deep interior red that has to stay the same regardless of
        // theme, or the vent stops reading as a recess.
        ('E', Color::rgb(0x3f, 0x13, 0x13)),
        // Near-black with just enough of the scene's light in it to
        // avoid a dead flat hole where the diffuser sits.
        ('F', Color::BLACK.lerp(theme.foreground, 0.12)),
    ]
}

/// Palette for the roadside markers.
pub fn post_palette(theme: &Theme) -> Vec<PaletteEntry> {
    let dark = theme.red.lerp(theme.darker_background, 0.25);
    vec![
        ('L', theme.foreground),
        ('D', dark),
        // A mid-tone between the two, derived from `dark` rather than from
        // a theme slot directly — the same chaining the car palette uses,
        // which is what keeps the shades related to each other when the
        // theme changes rather than merely each related to the theme.
        ('H', dark.lerp(theme.foreground, 0.30)),
    ]
}

/// Every sprite the game ships, built once.
///
/// Built eagerly on purpose: `Sprite::new` panics on malformed art, so
/// constructing the whole set is what turns a typo in a grid into an
/// immediate failure rather than a hole in a screenshot.
pub struct Art {
    pub player: Sprite,
    /// The traffic. Same chassis as the player, different liveries.
    pub rivals: Vec<Sprite>,
    pub post: Sprite,
    /// The start/finish gantry. Not a prop: it spans the road and wants a
    /// FIXED track position, so `scenery.rs` must never pick it up.
    pub gantry: Sprite,
    /// Roadside scenery — what actually carries the sense of motion.
    ///
    /// A list rather than named fields so adding a shape drawn in the
    /// sprite playground is one entry in `Art::load` and nothing else.
    /// `scenery.rs` picks from it by index and never needs to know what
    /// is in it.
    pub props: Vec<Sprite>,
}

/// The grid characters that ROLL.
///
/// Only the tyre tread. 'H' is the same colour but sits in the diffuser
/// as well as the wheels, and animating by colour would strobe the
/// bodywork — which is exactly why the tread got its own letter.
pub const TREAD: &[char] = &['R'];

/// The player's livery. The one car on the road that is fully saturated.
pub const PLAYER_BODY: Color = Color::rgb(214, 78, 62);
pub const PLAYER_ACCENT: Color = Color::rgb(240, 214, 120);

/// Rival liveries, before muting.
///
/// Written saturated and dulled at load, rather than hand-picking dull
/// values: [`mute`] pulls each toward the theme's own muted tone, so the
/// traffic stays theme-reactive and stays *behind* the player's red on
/// any palette. Hand-picked greys would be right on Everforest and
/// muddy or garish everywhere else.
const RIVAL_LIVERIES: &[(Color, Color)] = &[
    (Color::rgb(74, 128, 200), Color::rgb(210, 220, 235)),  // blue
    (Color::rgb(96, 160, 96), Color::rgb(214, 228, 200)),   // green
    (Color::rgb(206, 168, 72), Color::rgb(240, 232, 200)),  // ochre
    (Color::rgb(150, 110, 190), Color::rgb(226, 214, 240)), // violet
    (Color::rgb(120, 132, 148), Color::rgb(214, 220, 228)), // gunmetal
];

/// How far a rival's livery is pulled toward the theme's muted tone.
///
/// The player has to be findable at a glance in a field of traffic, and
/// on a screen this size that read is carried by SATURATION long before
/// hue — a bright blue rival and a bright red player are equally loud,
/// so the eye has to compare shapes to find itself. Dulling the field
/// makes the player pop without changing a single pixel of the player's
/// own art.
///
/// 0.42 by measurement, not taste: below about 0.3 the brighter rivals
/// still competed with the player in the road scene, and past about 0.55
/// the traffic started sinking into the asphalt and stopped reading as
/// cars at distance.
const RIVAL_MUTE: f32 = 0.42;

/// Pull a colour toward the theme's muted tone.
fn mute(theme: &Theme, c: Color, amount: f32) -> Color {
    c.lerp(theme.muted, amount)
}

impl Art {
    pub fn load(theme: &Theme) -> Art {
        let player_pal = car_palette(theme, PLAYER_BODY, PLAYER_ACCENT);

        let rivals = RIVAL_LIVERIES
            .iter()
            .map(|&(body, accent)| {
                let pal = car_palette(
                    theme,
                    mute(theme, body, RIVAL_MUTE),
                    // The accent is dulled harder. It is a small bright
                    // area, and small bright areas are exactly what the
                    // eye locks onto when hunting for its own car.
                    mute(theme, accent, RIVAL_MUTE + 0.18),
                );
                Sprite::new_with_tread(PLAYER_CAR, &pal, TREAD)
            })
            .collect();

        Art {
            player: Sprite::new_with_tread(PLAYER_CAR, &player_pal, TREAD),
            rivals,
            post: Sprite::new(MARKER_POST, &post_palette(theme)),
            gantry: Sprite::new(GANTRY, &gantry_palette(theme)),
            props: vec![
                Sprite::new(MARKER_POST, &post_palette(theme)),
                Sprite::new(TALL_POLE, &post_palette(theme)),
                Sprite::new(MARKER_BLOCK, &post_palette(theme)),
            ],
        }
    }

    /// The rival livery for a given traffic slot.
    ///
    /// Wraps, so callers can index by lane, by car id, or by anything
    /// else without bounds-checking. Deterministic on purpose: the same
    /// car keeps the same colour from frame to frame, which a random
    /// pick per frame would not.
    pub fn rival(&self, n: usize) -> &Sprite {
        &self.rivals[n % self.rivals.len()]
    }

    /// A roadside prop by index, wrapping.
    pub fn prop(&self, n: usize) -> &Sprite {
        &self.props[n % self.props.len()]
    }

    /// How many kinds of roadside art exist. `scenery.rs` spreads its
    /// placement across this, so a new sprite appears on the track with
    /// no other change.
    pub fn prop_kinds(&self) -> usize {
        self.props.len()
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
        // 64x40 now. 32x20 had no room for curved bodywork or a wing
        // with thickness; 48x30 fixed that but still could not hold a
        // diffuser, tail lights and a canopy surround as distinct
        // shapes. The renderer was never the constraint here — measured
        // at 1.34ms for a full field — authoring effort was.
        assert_eq!(art.player.width(), 64);
        assert_eq!(art.player.height(), 40);
        assert!(art.player.ink() > 400, "the car should have real substance");
        assert!(!art.rivals.is_empty(), "there has to be traffic");
        for r in &art.rivals {
            assert_eq!(r.width(), 64);
            assert_eq!(r.height(), 40);
        }
    }

    #[test]
    fn the_cars_are_the_same_size() {
        // They share a road and a scale factor; different dimensions
        // would make one sit wrong relative to the other.
        let art = Art::load(&Theme::fallback());
        for r in &art.rivals {
            assert_eq!(art.player.width(), r.width());
            assert_eq!(art.player.height(), r.height());
            assert_eq!(art.player.ink(), r.ink(), "same chassis, so same ink");
        }
    }

    #[test]
    fn asking_for_a_rival_past_the_end_wraps() {
        // Callers index by lane or car id without bounds-checking.
        let art = Art::load(&Theme::fallback());
        let n = art.rivals.len();
        assert_eq!(art.rival(0), art.rival(n));
        assert_eq!(art.rival(1), art.rival(n + 1));
    }

    #[test]
    fn every_rival_has_its_own_livery() {
        // Muting must not collapse the field into one colour — five
        // identical cars would be a bug that looks like a design choice.
        let art = Art::load(&Theme::fallback());
        let bodies: Vec<Color> = RIVAL_LIVERIES
            .iter()
            .map(|&(b, _)| mute(&Theme::fallback(), b, RIVAL_MUTE))
            .collect();
        for (i, a) in bodies.iter().enumerate() {
            for b in &bodies[i + 1..] {
                assert_ne!(a, b, "two rivals share a livery");
            }
        }
        assert_eq!(art.rivals.len(), RIVAL_LIVERIES.len());
    }

    /// The point of muting, stated as arithmetic rather than as a
    /// comment: every rival must be less saturated than the player, or
    /// the player cannot be found at a glance in traffic.
    #[test]
    fn the_player_is_the_most_saturated_car_on_the_road() {
        let theme = Theme::fallback();
        // Chroma as max-channel minus min-channel: crude, but it is
        // exactly the "how colourful is this" the eye is doing here, and
        // it needs no colour-space conversion.
        let chroma = |c: Color| {
            let hi = c.r.max(c.g).max(c.b) as i32;
            let lo = c.r.min(c.g).min(c.b) as i32;
            hi - lo
        };
        let player = chroma(PLAYER_BODY);
        for &(body, _) in RIVAL_LIVERIES {
            let muted = mute(&theme, body, RIVAL_MUTE);
            assert!(
                chroma(muted) < player,
                "rival {body:?} -> {muted:?} is not duller than the player",
            );
            // And muting must actually do something to each one.
            assert!(chroma(muted) < chroma(body), "muting did nothing to {body:?}");
        }
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
