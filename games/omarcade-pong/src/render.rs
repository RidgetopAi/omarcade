//! Drawing. The only file that turns state into pixels.
//!
//! Gameplay happens in a fixed 960x720 play field; the window is
//! whatever size Hyprland decides. This module bridges the two with a
//! **letterbox**: one uniform scale for both axes, centred, with bars
//! on whichever pair of edges has slack. A non-uniform stretch would be
//! easier and would mean the ball moves faster horizontally than
//! vertically at the same speed — which players feel immediately even
//! if they cannot name it.
//!
//! The 5x7 font is `omarcade_core::text`. It was a copy of Breakout's here
//! until the racer became its third consumer, which is the rule `geom`
//! moved on. Each game still owns its coverage test, because each game
//! owns the strings it can display.

use omarcade_core::ease;
use omarcade_core::geom::Rect;
use omarcade_core::text::{text, text_width, GLYPH_H};
use omarcade_core::{Canvas, Color, Theme};

use crate::state::{Difficulty, GameState, Phase, Side, FIELD_H, FIELD_W};

/// Maps play-field coordinates onto the window.
#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    scale: f32,
    off_x: f32,
    off_y: f32,
}

impl Viewport {
    /// Fit the play field inside `(w, h)`, preserving aspect ratio.
    pub fn fit(w: u32, h: u32) -> Self {
        // A zero-sized window would give a zero or NaN scale; clamp to
        // something harmless since a frame may still be requested.
        let w = w.max(1) as f32;
        let h = h.max(1) as f32;
        let scale = (w / FIELD_W).min(h / FIELD_H);
        Viewport {
            scale,
            off_x: (w - FIELD_W * scale) / 2.0,
            off_y: (h - FIELD_H * scale) / 2.0,
        }
    }

    fn x(&self, x: f32) -> i32 {
        (self.off_x + x * self.scale).round() as i32
    }

    fn y(&self, y: f32) -> i32 {
        (self.off_y + y * self.scale).round() as i32
    }

    /// Float variants, for things that move. The integer ones round to
    /// the nearest pixel, which is right for static geometry and wrong
    /// for anything animating.
    fn fx(&self, x: f32) -> f32 {
        self.off_x + x * self.scale
    }

    fn fy(&self, y: f32) -> f32 {
        self.off_y + y * self.scale
    }

    fn flen(&self, v: f32) -> f32 {
        (v * self.scale).max(1.0)
    }

    fn len(&self, v: f32) -> u32 {
        // At least one pixel: a thin object that rounds to zero would
        // vanish entirely at small window sizes.
        ((v * self.scale).round() as i32).max(1) as u32
    }

    fn rect(&self, r: Rect, canvas: &mut Canvas<'_>, color: Color) {
        canvas.fill_rect(self.x(r.x), self.y(r.y), self.len(r.w), self.len(r.h), color);
    }

    /// Text scale that keeps the HUD proportional to the window rather
    /// than fixed in pixels.
    fn text_scale(&self, base: f32) -> u32 {
        ((base * self.scale).round() as i32).max(1) as u32
    }
}

/// Draw the whole frame.
pub fn draw(state: &GameState, canvas: &mut Canvas<'_>, theme: &Theme) {
    let vp = Viewport::fit(canvas.width(), canvas.height());

    // The letterbox bars are darker than the field, so the play area
    // reads as a distinct surface rather than the window just being an
    // odd shape.
    canvas.clear(theme.darker_background);
    canvas.fill_rect(
        vp.x(0.0),
        vp.y(0.0),
        vp.len(FIELD_W),
        vp.len(FIELD_H),
        theme.background,
    );

    if state.phase == Phase::Select {
        draw_select(state, canvas, theme, &vp);
        return;
    }

    draw_net(canvas, theme, &vp);
    draw_score(state, canvas, theme, &vp);

    vp.rect(state.left.rect(), canvas, theme.foreground);
    vp.rect(state.right.rect(), canvas, theme.foreground);

    // No ball between points or after the match — nothing is in play.
    if state.phase == Phase::Playing {
        draw_trail(state, canvas, theme, &vp);

        // Sub-pixel, unlike the paddles: this is the one thing on
        // screen that moves every frame, and snapping it to whole
        // pixels is exactly what makes 60fps motion look like 30.
        let r = state.ball.rect();
        canvas.fill_rect_f(
            vp.fx(r.x),
            vp.fy(r.y),
            vp.flen(r.w),
            vp.flen(r.h),
            theme.accent,
        );
    }

    draw_rally(state, canvas, theme, &vp);
    draw_phase_message(state, canvas, theme, &vp);
}

/// The dashed centre line.
///
/// Static geometry, so integer rects: it never moves, and sub-pixel
/// coverage would only make it blurrier than the paddles beside it.
fn draw_net(canvas: &mut Canvas<'_>, theme: &Theme, vp: &Viewport) {
    const DASH_H: f32 = 18.0;
    const GAP: f32 = 14.0;
    const W: f32 = 4.0;

    let x = FIELD_W / 2.0 - W / 2.0;
    let mut y = GAP / 2.0;
    while y < FIELD_H {
        let h = DASH_H.min(FIELD_H - y);
        vp.rect(Rect::new(x, y, W, h), canvas, theme.dark_foreground);
        y += DASH_H + GAP;
    }
}

/// The ball's recent path, fading out behind it.
///
/// Cheap on purpose: ten alpha quads, far under a tenth of a frame at
/// 60fps. Sampled once per frame by `physics::step`, never per fixed
/// tick, so its length does not change with frame rate.
fn draw_trail(state: &GameState, canvas: &mut Canvas<'_>, theme: &Theme, vp: &Viewport) {
    // Skip the newest sample: physics records the ball's CURRENT
    // position, so trail[0] sits exactly under the ball. Drawing it
    // costs a blend for a quad nothing can see and slightly muddies the
    // leading edge, where the ball should be at its most solid.
    let trail = state.trail.iter().skip(1);
    let n = state.trail.len().saturating_sub(1) as f32;
    for (i, p) in trail.enumerate() {
        // Newest is nearly solid, oldest nearly gone.
        let t = 1.0 - (i as f32 / n.max(1.0));
        let alpha = (ease::out_quad(t) * 110.0) as u8;
        if alpha == 0 {
            continue;
        }
        // Shrinks as it fades, which reads as speed rather than as a
        // smear of identical squares.
        let r = state.ball.radius * (0.45 + 0.55 * t);
        canvas.fill_rect_f(
            vp.fx(p.x - r),
            vp.fy(p.y - r),
            vp.flen(r * 2.0),
            vp.flen(r * 2.0),
            theme.accent.with_alpha(alpha),
        );
    }
}

/// The two big score digits, either side of the net.
fn draw_score(state: &GameState, canvas: &mut Canvas<'_>, theme: &Theme, vp: &Viewport) {
    let scale = vp.text_scale(6.0);
    let y = vp.y(46.0);

    let left = state.score_left.to_string();
    let right = state.score_right.to_string();

    // Mirrored around the net rather than centred in each half, so the
    // two numbers stay a matched pair as they gain digits.
    let gap = 70.0;
    let lx = vp.x(FIELD_W / 2.0 - gap) - text_width(&left, scale) as i32;
    let rx = vp.x(FIELD_W / 2.0 + gap);

    text(canvas, &left, lx, y, scale, theme.foreground);
    text(canvas, &right, rx, y, scale, theme.foreground);
}

/// Current rally length, once it is worth remarking on.
///
/// Hidden below a threshold: a "1" appearing on every serve is noise,
/// and the number only becomes interesting once a rally is going
/// somewhere.
fn draw_rally(state: &GameState, canvas: &mut Canvas<'_>, theme: &Theme, vp: &Viewport) {
    const SHOW_FROM: u32 = 5;
    if state.phase != Phase::Playing || state.rally < SHOW_FROM {
        return;
    }
    let scale = vp.text_scale(2.0);
    let s = format!("RALLY {}", state.rally);
    // Below the net's last dash rather than centred on it — the net
    // runs down the middle of the field, so a centred label sits
    // directly on top of it and both become unreadable.
    let x = vp.x(FIELD_W / 2.0) - (text_width(&s, scale) / 2) as i32;
    let y = vp.y(FIELD_H - 26.0);
    // A short backing bar in the field colour, so the label reads even
    // where it overlaps the net.
    canvas.fill_rect(
        x - (4.0 * vp.scale) as i32,
        y - (3.0 * vp.scale) as i32,
        text_width(&s, scale) + (8.0 * vp.scale) as u32,
        (GLYPH_H * scale) + (6.0 * vp.scale) as u32,
        theme.background,
    );
    text(canvas, &s, x, y, scale, theme.muted);
}

/// The difficulty select.
fn draw_select(state: &GameState, canvas: &mut Canvas<'_>, theme: &Theme, vp: &Viewport) {
    let title_scale = vp.text_scale(5.0);
    let title = "PONG";
    text(
        canvas,
        title,
        vp.x(FIELD_W / 2.0) - (text_width(title, title_scale) / 2) as i32,
        vp.y(120.0),
        title_scale,
        theme.foreground,
    );

    let scale = vp.text_scale(3.0);
    let mut y = 300.0;
    for d in Difficulty::ALL {
        let selected = d == state.difficulty;
        let label = d.label();
        let w = text_width(label, scale) as i32;
        let x = vp.x(FIELD_W / 2.0) - w / 2;

        if selected {
            // A filled bar behind the choice, sized to the text, so the
            // selection reads at a glance rather than by colour alone.
            let pad = 16.0;
            let bar = Rect::new(
                FIELD_W / 2.0 - (w as f32 / vp.scale) / 2.0 - pad,
                y - 10.0,
                (w as f32 / vp.scale) + pad * 2.0,
                46.0,
            );
            vp.rect(bar, canvas, theme.selection);
        }

        text(
            canvas,
            label,
            x,
            vp.y(y),
            scale,
            if selected { theme.background } else { theme.muted },
        );
        y += 70.0;
    }

    let hint_scale = vp.text_scale(2.0);
    for (i, line) in ["UP DOWN TO CHOOSE", "SPACE TO START"].iter().enumerate() {
        let x = vp.x(FIELD_W / 2.0) - (text_width(line, hint_scale) / 2) as i32;
        text(
            canvas,
            line,
            x,
            vp.y(560.0 + i as f32 * 34.0),
            hint_scale,
            theme.muted,
        );
    }
}

/// Serve prompt and the end-of-match screen.
fn draw_phase_message(
    state: &GameState,
    canvas: &mut Canvas<'_>,
    theme: &Theme,
    vp: &Viewport,
) {
    match state.phase {
        Phase::Serve => {
            let scale = vp.text_scale(2.0);
            let msg = "SPACE TO SERVE";
            let x = vp.x(FIELD_W / 2.0) - (text_width(msg, scale) / 2) as i32;
            text(canvas, msg, x, vp.y(FIELD_H / 2.0 + 60.0), scale, theme.muted);
        }
        Phase::Over { winner } => {
            // One veil for the whole state. It is the most expensive
            // call on the canvas — roughly 7x an opaque frame — so it
            // is used for a STATE, never as a per-frame effect.
            canvas.veil(theme.darker_background.with_alpha(190));

            let scale = vp.text_scale(4.0);
            let msg = match winner {
                Side::Left => "YOU WIN",
                Side::Right => "YOU LOSE",
            };
            let x = vp.x(FIELD_W / 2.0) - (text_width(msg, scale) / 2) as i32;
            text(
                canvas,
                msg,
                x,
                vp.y(260.0),
                scale,
                if winner == Side::Left { theme.green } else { theme.red },
            );

            let sub = vp.text_scale(2.0);
            let detail = format!(
                "{} {}-{}   LONGEST RALLY {}",
                state.difficulty.label(),
                state.score_left,
                state.score_right,
                state.longest_rally
            );
            let dx = vp.x(FIELD_W / 2.0) - (text_width(&detail, sub) / 2) as i32;
            text(canvas, &detail, dx, vp.y(360.0), sub, theme.foreground);

            if state.best > 0 {
                let best = format!("BEST RALLY {}", state.best);
                let bx = vp.x(FIELD_W / 2.0) - (text_width(&best, sub) / 2) as i32;
                text(canvas, &best, bx, vp.y(400.0), sub, theme.accent);
            }

            for (i, line) in ["ENTER TO PLAY AGAIN", "ESC TO QUIT"].iter().enumerate() {
                let lx = vp.x(FIELD_W / 2.0) - (text_width(line, sub) / 2) as i32;
                text(
                    canvas,
                    line,
                    lx,
                    vp.y(480.0 + i as f32 * 34.0),
                    sub,
                    theme.muted,
                );
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omarcade_core::text::glyph;
    use crate::state::MATCH_POINT;

    /// Pixels differing from a reference frame.
    ///
    /// NOT "non-zero pixels": the theme background is itself a non-zero
    /// colour, so counting those counts the whole canvas and every
    /// comparison comes out equal.
    fn differing(a: &[u32], b: &[u32]) -> usize {
        a.iter().zip(b).filter(|(x, y)| x != y).count()
    }

    /// Render one frame and hand back its buffer.
    fn frame_of(state: &GameState, w: u32, h: u32) -> Vec<u32> {
        let mut buf = vec![0; (w * h) as usize];
        {
            let mut canvas = Canvas::new(&mut buf, w, h);
            draw(state, &mut canvas, &Theme::fallback());
        }
        buf
    }

    /// A blank frame at the same size, to compare against.
    fn blank(w: u32, h: u32) -> Vec<u32> {
        let mut buf = vec![0; (w * h) as usize];
        {
            let mut canvas = Canvas::new(&mut buf, w, h);
            canvas.clear(Theme::fallback().darker_background);
            canvas.fill_rect(0, 0, w, h, Theme::fallback().background);
        }
        buf
    }

    /// EVERY string this game can put on screen must be renderable.
    ///
    /// The 5x7 font skips unknown glyphs silently, which is how Breakout
    /// once shipped "BEST" as "EST". Requiring the whole printable set
    /// rather than a list of today's strings means a new message cannot
    /// reintroduce it.
    #[test]
    fn the_font_covers_everything_the_game_can_display() {
        for ch in ('A'..='Z').chain('0'..='9').chain([' ', '-']) {
            assert!(glyph(ch).is_some(), "font is missing {ch:?}");
        }

        // The literal strings, including every generated one.
        let mut strings = vec![
            "PONG".to_string(),
            "UP DOWN TO CHOOSE".to_string(),
            "SPACE TO START".to_string(),
            "SPACE TO SERVE".to_string(),
            "YOU WIN".to_string(),
            "YOU LOSE".to_string(),
            "ENTER TO PLAY AGAIN".to_string(),
            "ESC TO QUIT".to_string(),
        ];
        for d in Difficulty::ALL {
            strings.push(d.label().to_string());
            strings.push(format!("{} {}-{}   LONGEST RALLY {}", d.label(), MATCH_POINT, 9, 42));
        }
        strings.push(format!("BEST RALLY {}", 137));
        strings.push(format!("RALLY {}", 23));

        for s in strings {
            for ch in s.chars() {
                assert!(
                    glyph(ch).is_some(),
                    "{s:?} contains {ch:?}, which the font would skip silently"
                );
            }
        }
    }

    #[test]
    fn the_viewport_letterboxes_rather_than_stretching() {
        // A wide window: bars left and right, scale set by height.
        let vp = Viewport::fit(1920, 720);
        assert!((vp.scale - 1.0).abs() < 1e-5);
        assert!(vp.off_x > 0.0, "expected horizontal bars");
        assert!((vp.off_y).abs() < 1e-5);

        // A tall one: bars top and bottom.
        let vp = Viewport::fit(960, 1440);
        assert!((vp.scale - 1.0).abs() < 1e-5);
        assert!(vp.off_y > 0.0, "expected vertical bars");
    }

    #[test]
    fn a_zero_sized_window_does_not_produce_nan() {
        let vp = Viewport::fit(0, 0);
        assert!(vp.scale.is_finite());
        assert!(vp.x(100.0).is_positive() || vp.x(100.0) <= 0);
        assert!(vp.len(10.0) >= 1, "a thin object must never vanish entirely");
    }

    #[test]
    fn a_thin_rect_still_draws_at_a_tiny_scale() {
        let vp = Viewport::fit(96, 72); // one tenth scale
        assert!(vp.len(4.0) >= 1, "the net must survive a small window");
    }

    #[test]
    fn every_phase_renders_without_panicking() {
        // Includes the odd sizes: a game that panics mid-frame because
        // the window is 1px wide is worse than one that draws nothing.
        for (w, h) in [(960, 720), (1261, 701), (1, 1), (2536, 1416)] {
            for phase in [
                Phase::Select,
                Phase::Serve,
                Phase::Playing,
                Phase::Over { winner: Side::Left },
                Phase::Over { winner: Side::Right },
            ] {
                let mut s = GameState::new();
                s.begin();
                s.phase = phase;
                s.score_left = 7;
                s.score_right = MATCH_POINT;
                s.rally = 12;
                s.longest_rally = 44;
                s.best = 51;
                s.trail = vec![s.ball.pos; 10];

                let _ = frame_of(&s, w, h);
            }
        }
    }

    #[test]
    fn the_select_screen_actually_draws_something() {
        let (w, h) = (960, 720);
        let s = GameState::new();
        let drawn = frame_of(&s, w, h);
        assert!(
            differing(&drawn, &blank(w, h)) > 1000,
            "the select screen rendered nearly nothing"
        );
    }

    #[test]
    fn the_ball_is_hidden_when_it_is_not_in_play() {
        // Count pixels with and without the ball, holding everything
        // else fixed. Serve and Over must not show one.
        let (w, h) = (960, 720);

        let frame_for = |phase: Phase| {
            let mut s = GameState::new();
            s.begin();
            s.phase = phase;
            s.trail.clear();
            frame_of(&s, w, h)
        };

        // Compare the two frames directly: the only difference between
        // them should be the ball itself.
        let playing = frame_for(Phase::Playing);
        let serving = frame_for(Phase::Serve);
        let diff = differing(&playing, &serving);
        assert!(
            diff > 40,
            "expected a visible ball while playing and none while serving, diff was {diff}"
        );
    }

    #[test]
    fn the_rally_counter_stays_hidden_until_it_is_worth_showing() {
        let (w, h) = (960, 720);
        let render = |rally: u32| {
            let mut s = GameState::new();
            s.begin();
            s.phase = Phase::Playing;
            s.rally = rally;
            s.trail.clear();
            frame_of(&s, w, h)
        };
        assert_eq!(
            differing(&render(1), &render(4)),
            0,
            "a short rally must not draw a counter"
        );
        assert!(
            differing(&render(9), &render(4)) > 20,
            "a rally worth remarking on should show its length"
        );
    }
}
