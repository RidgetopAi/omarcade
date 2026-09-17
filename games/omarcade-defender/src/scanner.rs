//! The scanner: the whole world, at a glance.
//!
//! ★ THIS IS THE FILE THAT MAKES MUTANTS FAIR. S7 gave them the ability
//! to hunt you across a world four screens wide, which meant the first
//! warning you got was one arriving on top of you. That was read as the
//! stage being unfair. It was not the stage — it was the missing half of
//! it. Defender's scanner is not a convenience; the game is unplayable
//! without it, and that has been true since 1981.
//!
//! Everything here is READ-ONLY presentation over state that already
//! exists. The scanner adds no gameplay, owns no state, and decides
//! nothing. If it were deleted the game would play identically and
//! merely be impossible.
//!
//! ⚠️ THE WHOLE FILE IS ABOUT ONE MAPPING, AND THE SEAM RUNS THROUGH IT.
//! The scanner shows the entire looping world compressed into a strip,
//! so unlike the main view — which shows one screen and can mostly
//! pretend the world is flat — there is nowhere here for the wrap to
//! hide. Every world x goes through [`Scanner::plot`], which wraps; the
//! view box, which is the one thing wide enough to straddle x = 0, is
//! drawn in TWO PIECES when it does. See `draw_view_box`.

use omarcade_core::{Canvas, Color, Theme};

use crate::enemy::{Kind, Landers, Phase};
use crate::humanoid::{Humanoids, State};
use crate::flight::{Camera, Ship};
use crate::world::{self, Terrain};

/// How wide the scanner is, as a fraction of the screen.
///
/// ★ THE ARCADE'S PROPORTION, ROUGHLY. Defender's scanner spans the
/// middle of the display with the score either side of it, and the
/// reason is legibility rather than taste: the strip has to hold four
/// screens of world, so a narrow one compresses two Landers a full
/// screen apart into the same pixel and stops answering the question it
/// exists to answer.
pub const WIDTH_FRACTION: f32 = 0.52;

/// Height of the scanner face, in pixels.
///
/// ⚠️ NOT PROPORTIONAL TO THE WIDTH, AND DELIBERATELY SO. The world is
/// 3840x720; drawn to scale in a 500px strip it would be 94px tall and
/// eat a seventh of the screen. The scanner squashes vertically — which
/// is what the arcade did — because the vertical axis only has to
/// separate "on the ground" from "up where the Mutants are", and that
/// needs far less room than a faithful aspect ratio would give it.
const HEIGHT: f32 = 56.0;

/// Gap between the top of the screen and the scanner face.
const TOP: f32 = 30.0;

/// How far outside the face the glow bleeds.
const GLOW_PAD: f32 = 3.0;

// ★★ THE BLIP COLOURS ARE THE ARCADE'S, NOT THE THEME'S — AND THIS IS A
// DELIBERATE EXCEPTION TO A RULE THE REST OF THE SUITE FOLLOWS.
//
// The scanner was rendered against Brian's live Omarchy theme and every
// blip came out white. The theme is near-monochrome: its `magenta` is
// #c4d8e2, a pale blue-grey, and its `yellow` is #6b5e73. Nothing was
// broken — the scanner drew exactly what it was given.
//
// ⚠️ BUT A MUTANT THAT READS AS GREY ON A GREY THEME DEFEATS THE ONLY
// REASON THIS FILE EXISTS. The whole point of S8 is that a Mutant
// hunting you from across the world must be unmistakable BEFORE it
// arrives. Danger is game information, not decoration, and information
// cannot be delegated to a palette the player chose for their terminal.
//
// ⇒ THE FURNITURE STAYS THEMED (the rim, the ground, the view box, the
// ship) because that is presentation and should feel native. THE
// THREATS ARE FIXED. The same split already exists in this game and was
// not invented here: effects.rs hardcodes every explosion colour, the
// tractor beam in render.rs is a fixed green, and enemy bolts are a
// fixed orange. Volley uses no fixed colours at all; the racer uses 29.
const LANDER_COLOUR: Color = Color::rgb(255, 96, 40);
const MUTANT_COLOUR: Color = Color::rgb(255, 40, 200);
const PERSON_COLOUR: Color = Color::rgb(90, 255, 140);

/// The scanner's coordinate frame: the whole world, squashed to fit.
///
/// ★ ONE PLACE WHERE WORLD BECOMES SCANNER, AND EVERY CALLER GOES
/// THROUGH IT. The seam and the vertical squash are both handled here,
/// once. A blip that computed its own x would be the exact shape of bug
/// this project keeps finding: correct everywhere except within one
/// hitbox of x = 0.
struct Scanner {
    /// Left edge of the face, in screen pixels.
    left: f32,
    /// Top edge of the face, in screen pixels.
    top: f32,
    /// Width of the face, in screen pixels.
    width: f32,
    /// Height of the face, in screen pixels.
    height: f32,
}

impl Scanner {
    fn new(canvas_w: f32) -> Self {
        let width = canvas_w * WIDTH_FRACTION;
        Self {
            left: (canvas_w - width) * 0.5,
            top: TOP,
            width,
            height: HEIGHT,
        }
    }

    /// Screen position for a world position.
    ///
    /// ⚠️ `world::wrap` FIRST, ALWAYS. A Lander at x = -3 is three units
    /// west of the seam and belongs at the far RIGHT of the scanner; the
    /// raw coordinate would put it three pixels off the left edge, where
    /// it would be clipped away and the player would be told there is
    /// nothing there.
    fn plot(&self, world_x: f32, world_y: f32) -> (f32, f32) {
        let t = world::wrap(world_x) / world::WORLD_W;
        let x = self.left + t * self.width;

        // World y measures UP from the floor; the face measures DOWN
        // from its top edge. Clamped rather than allowed to run off,
        // because a Mutant riding the ceiling still has to appear ON the
        // scanner — off the top edge is indistinguishable from absent,
        // and absent is the one thing it must never look like.
        let v = (world_y / world::VIEW_H).clamp(0.0, 1.0);
        let y = self.top + (1.0 - v) * self.height;
        (x, y)
    }

    /// The scanner x for a world x, without the vertical.
    fn plot_x(&self, world_x: f32) -> f32 {
        self.left + (world::wrap(world_x) / world::WORLD_W) * self.width
    }

    fn right(&self) -> f32 {
        self.left + self.width
    }

    fn bottom(&self) -> f32 {
        self.top + self.height
    }
}

/// Everything the scanner draws, gathered so the signature does not grow
/// a parameter per enemy type the way `render::draw` nearly did.
pub struct View<'a> {
    pub terrain: &'a Terrain,
    pub ship: &'a Ship,
    pub camera: &'a Camera,
    pub landers: &'a Landers,
    pub people: &'a Humanoids,
    /// Seconds since the game began, for the Mutant pulse.
    ///
    /// ⚠️ A CLOCK, NOT A FRAME COUNT. The pulse has to run at the same
    /// rate however fast the machine draws, for the same reason the
    /// camera easing does.
    pub time: f32,
}

/// Draw the scanner.
///
/// Order is depth, and it is the same order the main view uses: the face
/// it all sits on, the ground, then the things standing on the ground,
/// then the things in the air, then the frame marking where you are
/// looking — which goes ON TOP because it is the one element the eye
/// has to find first.
pub fn draw(canvas: &mut Canvas<'_>, view: &View<'_>, theme: &Theme) {
    let s = Scanner::new(canvas.width() as f32);

    draw_face(canvas, &s, theme);
    draw_terrain(canvas, &s, view.terrain, theme);
    draw_people(canvas, &s, view.people);
    draw_landers(canvas, &s, view.landers, view.time);
    draw_ship(canvas, &s, view.ship);
    draw_view_box(canvas, &s, view.camera, theme);
}

/// The face the scanner is drawn on.
///
/// ★ THE "HUD LOOK" BRIAN ASKED FOR STARTS HERE. A flat outlined
/// rectangle is the 1981 original and reads as a hole cut in the screen.
/// What makes this read as a lit instrument instead is that the face is
/// DARKER than the sky behind it while its border GLOWS — the contrast
/// between a dark well and a bright rim is what the eye interprets as
/// something emitting light rather than something drawn on.
fn draw_face(canvas: &mut Canvas<'_>, s: &Scanner, theme: &Theme) {
    // The well. Darker than the sky so blips have something to be
    // bright against — the sky alone is not dark enough to carry a dim
    // Humanoid dot.
    canvas.fill_rect_f(
        s.left,
        s.top,
        s.width,
        s.height,
        theme.background.lerp(Color::rgb(0, 0, 0), 0.62),
    );

    // The rim, three passes from wide-and-dim to tight-and-bright. One
    // bright line reads as a border; a gradient off the edge of a bright
    // line reads as a border that is LIT, and that difference is the
    // whole of the modernisation.
    let rim = theme.cyan.lerp(theme.foreground, 0.25);
    for (pad, strength) in [(GLOW_PAD, 0.10), (1.5, 0.22), (0.0, 0.55)] {
        let c = scaled(rim, strength);
        let (x, y) = (s.left - pad, s.top - pad);
        let (w, h) = (s.width + pad * 2.0, s.height + pad * 2.0);
        // Four bars rather than a stroked rect: the corners double up,
        // which is exactly where a rim highlight wants to be brightest.
        canvas.fill_rect_add_f(x, y, w, 1.0 + pad * 0.5, c);
        canvas.fill_rect_add_f(x, y + h - (1.0 + pad * 0.5), w, 1.0 + pad * 0.5, c);
        canvas.fill_rect_add_f(x, y, 1.0 + pad * 0.5, h, c);
        canvas.fill_rect_add_f(x + w - (1.0 + pad * 0.5), y, 1.0 + pad * 0.5, h, c);
    }
}

/// The ground, as a ridge line across the whole world.
///
/// ★ WITHOUT THIS THE BLIPS FLOAT. A Humanoid on the surface and a
/// Humanoid being carried away are only distinguishable if you can see
/// where the surface IS — the abduction reads as a dot lifting OFF
/// something, and with no ground drawn there is nothing to lift off.
///
/// ⚠️ DRAWN AS A LINE, NOT A FILL. Filling it solid would give the
/// scanner a bright lower half that drowns the Humanoid dots sitting on
/// it, which is the one population of blips already hardest to see.
fn draw_terrain(canvas: &mut Canvas<'_>, s: &Scanner, terrain: &Terrain, theme: &Theme) {
    // ★ THE WORLD ENDED — there is no ground to draw. Same stance as the
    // main renderer: return rather than draw a flat line, because a line
    // would say there is still a surface down there.
    if terrain.is_destroyed() {
        return;
    }

    let ground = scaled(theme.green.lerp(theme.foreground, 0.2), 0.34);
    let columns = s.width as usize;

    for i in 0..columns {
        let t = i as f32 / columns as f32;
        let wx = t * world::WORLD_W;
        let (_, y) = s.plot(wx, terrain.height_at(wx));
        canvas.fill_rect_add_f(s.left + i as f32, y, 1.0, 1.5, ground);
    }
}

/// The people.
///
/// Dim, small, and low on the face. They are not a threat and must not
/// compete with the things that are — but they are the whole POINT of
/// the game, so a player scanning for "is anyone being carried up" has
/// to be able to find them.
fn draw_people(canvas: &mut Canvas<'_>, s: &Scanner, people: &Humanoids) {
    let colour = PERSON_COLOUR;

    for p in people.iter() {
        if p.state == State::Dead {
            continue;
        }
        let (x, y) = s.plot(p.x, p.y);
        // ★ A BEING-CARRIED PERSON IS BRIGHTER. The blip rising off the
        // ground already reads as an abduction; brightening it means the
        // player's eye is pulled there rather than having to notice.
        // ⚠️ A BLIP, NOT A 2x3 RECT. The first version drew a small
        // dim rectangle and the render showed it as very nearly
        // nothing — a flat dot has only its size to be seen by, while
        // the same dot with a halo reads as a light. These are the
        // dimmest population on the face by design and so are the ones
        // that most need the halo to be findable at all.
        let lifted = matches!(p.state, State::Carried | State::Falling);
        blip(canvas, x, y, if lifted { 1.9 } else { 1.5 },
             scaled(colour, if lifted { 1.0 } else { 0.72 }));
    }
}

/// The enemies.
///
/// ★★ THIS IS WHY THE SCANNER EXISTS, AND MUTANTS ARE WHY IT EXISTS NOW.
/// A Lander is something you go and find; a Mutant is something that
/// finds YOU, from anywhere in the world, and the player needs to see it
/// coming. So Mutants get the danger colour, a larger blip and a PULSE —
/// motion is the only channel on a crowded HUD that survives peripheral
/// vision, and a player mid-dogfight is using nothing but peripheral
/// vision on this strip.
fn draw_landers(canvas: &mut Canvas<'_>, s: &Scanner, landers: &Landers, time: f32) {
    for l in landers.iter() {
        // A dying Lander is already gone as far as the player's planning
        // is concerned — leaving it on the scanner for its death frames
        // would say there is still something there to deal with.
        if l.phase == Phase::Dying {
            continue;
        }

        let (x, y) = s.plot(l.x, l.y);

        match l.kind {
            Kind::Lander => {
                let c = LANDER_COLOUR;
                // Warping in: dimmer, so an arrival reads as an arrival
                // rather than as something that was always there.
                let strength = if l.phase == Phase::Warping { 0.45 } else { 0.9 };
                blip(canvas, x, y, 2.0, scaled(c, strength));
            }
            Kind::Mutant => {
                // ★ THE PULSE. Runs on the clock, not the frame, and is
                // offset by world position so a cluster of Mutants does
                // not flash in unison — a synchronised row reads as one
                // wide object, and knowing there are FOUR is the thing
                // that changes what a player does.
                let phase = time * 4.2 + world::wrap(l.x) * 0.02;
                let pulse = 0.65 + 0.35 * phase.sin();
                blip(canvas, x, y, 2.6 + pulse * 1.2, scaled(MUTANT_COLOUR, pulse));
            }
        }
    }
}

/// The ship.
///
/// The brightest thing on the face, and white rather than a theme
/// colour. On a strip carrying four other populations of coloured dot,
/// "which one is me" has to be answerable without a legend.
fn draw_ship(canvas: &mut Canvas<'_>, s: &Scanner, ship: &Ship) {
    let (x, y) = s.plot(ship.x, ship.y);
    // ⚠️ PURE WHITE AND THE LARGEST BLIP ON THE FACE. The first version
    // lerped 70% toward white from the theme foreground and drew at the
    // same radius as a Lander — rendered, the ship was DIMMER than the
    // Mutants hunting it. That is backwards: "where am I" is the fastest
    // read a player makes on this strip, and everything else is measured
    // relative to it.
    let white = Color::rgb(255, 255, 255);
    blip(canvas, x, y, 3.1, white);

    // A short tick in the direction faced. The ship blip is the one the
    // player looks at to answer "which way am I pointing" when the main
    // view has scrolled somewhere confusing.
    let dir = ship.facing.sign();
    canvas.fill_rect_add_f(x + dir * 2.0, y - 0.5, dir * 4.0, 1.0, scaled(white, 0.8));
}

/// The frame marking the screen you are actually looking at.
///
/// ★★ THE SINGLE MOST IMPORTANT ELEMENT ON THE SCANNER. Without it the
/// strip is a field of blips with no "you are here", and it cannot
/// answer the only question a player asks of it mid-flight: is that
/// Mutant to my left or my right? The ship blip alone is not enough —
/// knowing where you are does not tell you how much of the world you can
/// already see.
///
/// ⚠️⚠️ THIS IS THE ONE THING WIDE ENOUGH TO STRADDLE THE SEAM, AND IT
/// IS DRAWN IN TWO PIECES WHEN IT DOES. With the camera near x = 0 the
/// box runs off the right edge of the face and continues at the left.
/// Computing a single left-and-width from the camera would give a box
/// that is nearly the whole world wide and wrong in exactly the place
/// the player flies through every lap — the same shape as the seam bug
/// in `world::delta`'s own doc comment.
fn draw_view_box(canvas: &mut Canvas<'_>, s: &Scanner, camera: &Camera, theme: &Theme) {
    let edge = theme.cyan.lerp(Color::rgb(255, 255, 255), 0.45);

    // The visible slice, in scanner pixels. One screen of a world that
    // is WORLD_SCREENS wide.
    let span = s.width / world::WORLD_SCREENS;
    let start = s.plot_x(camera.left());

    // ⚠️ SPLIT AT THE RIGHT EDGE. `start + span` running past `right()`
    // is not a clipping problem to ignore — that part of the box is a
    // real part of the world the player can see, and it lives at the
    // other end of the face.
    let overflow = (start + span) - s.right();
    let pieces: [(f32, f32); 2] = if overflow > 0.0 {
        [(start, span - overflow), (s.left, overflow)]
    } else {
        [(start, span), (0.0, 0.0)]
    };

    for (x, w) in pieces {
        if w <= 0.0 {
            continue;
        }
        // A tinted wash so the slice reads as lit, then the brackets.
        canvas.fill_rect_add_f(x, s.top, w, s.height, scaled(edge, 0.07));
        canvas.fill_rect_add_f(x, s.top - 1.0, w, 1.0, scaled(edge, 0.5));
        canvas.fill_rect_add_f(x, s.bottom(), w, 1.0, scaled(edge, 0.5));
    }

    // ★ THE VERTICAL EDGES GO ON THE REAL ENDS OF THE SLICE, not on each
    // piece. A split box drawing four verticals would show two bright
    // bars in the middle of the world that correspond to nothing — the
    // seam is not a boundary the player can perceive, and drawing it as
    // one would invent a landmark that is not there.
    let end = s.plot_x(camera.left() + world::VIEW_W);
    for x in [start, end] {
        canvas.fill_rect_add_f(x - 0.5, s.top - 2.0, 1.5, s.height + 4.0, scaled(edge, 0.85));
    }
}

/// One blip: a bright core inside a soft halo.
///
/// ★ THE HALO IS THE WHOLE "LIGHTING" ASK. A flat dot of a given size is
/// as visible as its size allows; the same dot with additive falloff
/// around it reads as a light source and is findable at the edge of
/// vision at a much smaller core size. That matters here because the
/// scanner has to carry a dozen blips in 56 pixels of height without
/// becoming a smear.
fn blip(canvas: &mut Canvas<'_>, x: f32, y: f32, r: f32, color: Color) {
    canvas.circle_add_f(x, y, r * 2.1, scaled(color, 0.18));
    canvas.circle_add_f(x, y, r * 1.3, scaled(color, 0.32));
    canvas.circle_add_f(x, y, r * 0.6, color);
}

/// A colour at a fraction of its brightness.
///
/// ⚠️ SCALES THE CHANNELS, NOT THE ALPHA. These all draw additively, and
/// an additive draw ignores alpha — dimming one by making it transparent
/// would change nothing at all, silently.
fn scaled(c: Color, k: f32) -> Color {
    let k = k.clamp(0.0, 1.0);
    Color::rgb(
        (c.r as f32 * k) as u8,
        (c.g as f32 * k) as u8,
        (c.b as f32 * k) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: f32 = 960.0;

    #[test]
    fn the_whole_world_maps_onto_the_face() {
        let s = Scanner::new(W);
        // The west end of the world sits at the left edge, the east end
        // arrives back at it, and the middle is the middle.
        assert!((s.plot_x(0.0) - s.left).abs() < 0.01);
        assert!((s.plot_x(world::WORLD_W) - s.left).abs() < 0.01, "the world must close on itself");
        assert!((s.plot_x(world::WORLD_W * 0.5) - (s.left + s.width * 0.5)).abs() < 0.01);

        // ⚠️ EVERY world x lands ON the face, including ones nobody
        // normalised first. A blip clipped away is a blip that lies.
        for wx in [-9999.0, -1.0, 0.0, 12345.6, world::WORLD_W * 3.7] {
            let x = s.plot_x(wx);
            assert!(
                (s.left - 0.01..=s.right() + 0.01).contains(&x),
                "world x {wx} plotted to {x}, off a face spanning {}..{}",
                s.left,
                s.right()
            );
        }
    }

    #[test]
    fn the_vertical_is_flipped_and_clamped() {
        let s = Scanner::new(W);
        // World y grows UP; the face grows DOWN. Ground at the bottom.
        let (_, ground) = s.plot(0.0, 0.0);
        let (_, sky) = s.plot(0.0, world::VIEW_H);
        assert!(ground > sky, "the ground must be BELOW the sky on the face");
        assert!((ground - s.bottom()).abs() < 0.01);
        assert!((sky - s.top).abs() < 0.01);

        // ★ A MUTANT ABOVE THE CEILING STILL APPEARS. Off the top edge
        // is indistinguishable from absent.
        let (_, high) = s.plot(0.0, world::VIEW_H * 3.0);
        assert!((s.top..=s.bottom()).contains(&high), "a high blip fell off the face: {high}");
    }

    /// ⚠️ THE SEAM, WHICH IS THE BUG THIS FILE IS MOST LIKELY TO HAVE.
    /// Two points a few units apart across x = 0 must be a few PIXELS
    /// apart on the face — not a whole face apart.
    #[test]
    fn the_seam_is_not_a_cliff_on_the_scanner() {
        let s = Scanner::new(W);
        let px_per_unit = s.width / world::WORLD_W;

        let east = s.plot_x(4.0);
        let west = s.plot_x(world::WORLD_W - 4.0);
        // They are 8 world units apart, so they sit at opposite ENDS of
        // the face — that is correct and expected. What must hold is
        // that each is where it belongs.
        assert!(east - s.left < 8.0 * px_per_unit + 0.01, "just east of the seam is not at the left edge");
        assert!(s.right() - west < 8.0 * px_per_unit + 0.01, "just west of the seam is not at the right edge");
    }

    /// ★★ THE TEST THAT L064 IS ABOUT. The view box is the one element
    /// wide enough to straddle the seam, and the failure mode is a box
    /// that spans nearly the whole face. Assert the DRAWN WIDTH, by
    /// counting lit columns in a real buffer, rather than asserting the
    /// arithmetic in isolation — the arithmetic was never the part that
    /// was going to be wrong.
    #[test]
    fn the_view_box_stays_one_screen_wide_across_the_seam() {
        let span_px = (W * WIDTH_FRACTION) / world::WORLD_SCREENS;

        for camera_x in [
            0.0,
            world::VIEW_W * 0.5,
            world::WORLD_W * 0.5,
            world::WORLD_W - 1.0,
            world::WORLD_W - world::VIEW_W * 0.25,
        ] {
            let mut buf = vec![0u32; 960 * 720];
            let theme = Theme::fallback();
            let s = Scanner::new(W);
            {
                let mut c = Canvas::new(&mut buf, 960, 720);
                let camera = Camera::new(camera_x);
                draw_view_box(&mut c, &s, &camera, &theme);
            }

            // Count the columns the wash lit, along a row inside the box.
            let row = (s.top + s.height * 0.5) as usize;
            let lit = (s.left as usize..s.right() as usize)
                .filter(|&x| buf[row * 960 + x] != 0)
                .count() as f32;

            assert!(
                (lit - span_px).abs() < 4.0,
                "camera at {camera_x}: the view box lit {lit} columns, expected about {span_px} \
                 — a box spanning much more than one screen means the seam split failed"
            );
        }
    }

    /// The box must be where the camera is, not merely the right size.
    /// A correctly-sized box in the wrong place is the more dangerous
    /// bug, because it looks right.
    ///
    /// ⚠️ THE CENTRE IS MEASURED IN WORLD SPACE AND THEN PLOTTED, NOT BY
    /// ADDING HALF A SPAN TO THE PLOTTED START. The first version of
    /// this test did the latter and failed at camera x = 100 — where
    /// `camera.left()` is -380, which wraps to the far EAST end of the
    /// face, so the box legitimately starts at pixel 742 and continues
    /// past the seam at the left edge. Adding half a span to 742 walks
    /// off the face and lands nowhere. ★ THE CODE WAS RIGHT AND THE
    /// ASSERTION WAS NAIVE — the same shape as the sign-convention
    /// finding in the stage-1 notes. Scanner pixels cannot be added
    /// across the seam any more than world coordinates can be
    /// subtracted across it.
    #[test]
    fn the_view_box_is_centred_on_the_camera() {
        let s = Scanner::new(W);
        for camera_x in [100.0, 0.0, world::WORLD_W * 0.25, world::WORLD_W - 50.0] {
            let camera = Camera::new(camera_x);
            // Half a screen east of the left edge, in WORLD units — the
            // one place this arithmetic is safe.
            let centre_world = camera.left() + world::VIEW_W * 0.5;
            let centre = s.plot_x(centre_world);
            let expected = s.plot_x(camera_x);
            assert!(
                (centre - expected).abs() < 1.0,
                "camera at {camera_x}: box centre plots to {centre}, camera plots to {expected}"
            );
        }
    }

    #[test]
    fn dimming_scales_the_channels_rather_than_the_alpha() {
        // ⚠️ THE SILENT ONE. Everything here draws additively, and an
        // additive draw ignores alpha — dimming via alpha would compile,
        // run, and change nothing.
        let c = Color::rgb(200, 100, 50);
        let half = scaled(c, 0.5);
        assert_eq!((half.r, half.g, half.b), (100, 50, 25));
        assert_eq!(scaled(c, 0.0), Color::rgb(0, 0, 0));
        assert_eq!(scaled(c, 1.0), c);
        // Out-of-range input must not wrap a channel around.
        assert_eq!(scaled(c, 2.0), c);
    }
}
