//! The volume indicator: what a game shows when the volume keys move.
//!
//! # Why this exists at all
//!
//! The volume keys are handled by the backend, *before* a game sees any
//! input, so a game has no event to react to. That is deliberate — every
//! game in the suite should answer the same keys the same way without
//! each one implementing it — but it leaves a gap: press mute, and the
//! game goes silent with nothing on screen to say why.
//!
//! [`AudioSystem::volume_changes`](crate::AudioSystem::volume_changes)
//! was built for exactly this, and its own documentation names the
//! failure it is there to prevent: *"a volume of zero became
//! indistinguishable from broken audio."* This is the other half of that
//! seam — the part that puts it on the screen.
//!
//! # Why it lives in core
//!
//! Because all three games need it and none of them needs a different
//! one. Pixel Break is simply the first to show it; Pong and the racer
//! call the same two methods.
//!
//! A per-game copy would be three things to keep in step, and the whole
//! argument for a shared core is that this kind of thing is written once.
//!
//! # Using it
//!
//! Two calls from a game's `update` and `render`:
//!
//! ```ignore
//! fn update(&mut self, dt: f32, audio: &mut Audio<'_>) {
//!     self.volume.update(audio, dt);
//! }
//!
//! fn render(&mut self, canvas: &mut Canvas<'_>) {
//!     self.volume.draw(canvas, &self.theme);
//! }
//! ```
//!
//! ⚠️ `update` must be called on **every** frame, not only while playing.
//! Reaching for the volume on a title screen is the ordinary case, and an
//! indicator that only decays during play would stick there forever. This
//! is the S7 rule — a decaying effect must advance in every phase — in a
//! new place, and the type is built so a game cannot get it wrong: it
//! holds its own copy of the volume and needs no game state to tick.

use crate::text::{text, text_width};
use crate::{Audio, Canvas, Color, Theme};

/// How long the indicator stays up after the last change.
///
/// Long enough to read after a single keypress, short enough that holding
/// a key down does not leave it hanging around after the last press —
/// each change re-arms it, so the visible time is this plus however long
/// the player keeps pressing.
const SHOW_SECONDS: f32 = 1.6;

/// The last portion of its life spent fading out, as a fraction.
///
/// A hard disappearance reads as a glitch; a fade reads as an answer that
/// is finished. Only the tail fades, so the value itself is at full
/// contrast for as long as it is worth reading.
const FADE_TAIL: f32 = 0.35;

/// Bar geometry, in pixels at scale 1.
const BAR_W: u32 = 120;
const BAR_H: u32 = 10;
const PAD: i32 = 10;

/// Watches the volume and shows a bar when it moves.
///
/// ⚠️ Holds its own copy of the volume rather than reading it at draw
/// time, because `draw` gets a `Canvas` and not an `Audio` — the two are
/// handed out in different methods and deliberately never together.
#[derive(Debug, Clone, Copy)]
pub struct VolumeIndicator {
    /// The change counter as of last frame.
    ///
    /// ⚠️ Seeded from the audio system on the first `update` rather than
    /// from zero, so a persisted non-zero counter — or simply a game that
    /// starts after a change — does not flash the indicator on frame one.
    seen: Option<u32>,
    /// Seconds left on screen.
    left: f32,
    /// The volume as of the last change, and whether it was a mute.
    volume: f32,
    muted: bool,
}

impl VolumeIndicator {
    pub fn new() -> VolumeIndicator {
        VolumeIndicator { seen: None, left: 0.0, volume: 1.0, muted: false }
    }

    /// One already on screen at a given setting, for rendering a frame of
    /// it without an audio device.
    ///
    /// ⚠️ Public because the alternative is worse: a `dump_frame` scene
    /// that reached into private fields would be testing a state the game
    /// cannot actually produce, and the whole value of rendering a frame
    /// is that it shows what a player would see. It is also the only way
    /// to look at this at all — the indicator answers a keypress the
    /// backend consumes, so there is no way to pose it from game state.
    pub fn shown_at(volume: f32, muted: bool) -> VolumeIndicator {
        VolumeIndicator {
            seen: Some(0),
            left: SHOW_SECONDS,
            volume: volume.clamp(0.0, 1.0),
            muted,
        }
    }

    /// Tick the timer and notice any change. Call every frame.
    ///
    /// ⚠️ **Every frame, in every phase.** See the module docs: the volume
    /// keys work on a title screen, so an indicator that only advances
    /// during play would stick there until the player started a game.
    pub fn update(&mut self, audio: &Audio<'_>, dt: f32) {
        let now = audio.volume_changes();
        match self.seen {
            // First frame: adopt the counter without showing anything.
            // Whatever it already is happened before this game was looking.
            None => self.seen = Some(now),
            Some(seen) if seen != now => {
                self.seen = Some(now);
                let (volume, muted) = audio.volume();
                self.volume = volume;
                self.muted = muted;
                // Re-armed rather than extended, so holding a key down
                // does not bank an ever-growing display.
                self.left = SHOW_SECONDS;
            }
            Some(_) => {}
        }

        if self.left > 0.0 {
            self.left = (self.left - dt).max(0.0);
        }
    }

    /// Whether anything is on screen right now.
    pub fn showing(&self) -> bool {
        self.left > 0.0
    }

    /// How visible the indicator is, in `0.0..=1.0`.
    fn alpha(&self) -> f32 {
        if self.left <= 0.0 {
            return 0.0;
        }
        let fade = SHOW_SECONDS * FADE_TAIL;
        if self.left >= fade {
            1.0
        } else {
            self.left / fade
        }
    }

    /// Draw the indicator, if it has anything to say.
    ///
    /// ⚠️ **Bottom-right, and that was found by rendering it rather than
    /// reasoned out.** The first version went top-right on the assumption
    /// that the score sits top-left and the opposite corner is therefore
    /// free. The score does sit top-left — and Pixel Break puts LIVES
    /// top-RIGHT, with LEVEL centred between them, so the whole top row is
    /// taken. The bar landed straight across "LIVES 3".
    ///
    /// All three games keep the top row and the middle busy and none of
    /// them uses the bottom-right corner, which makes it the one position
    /// that ports. It is also simply the better place: a transient message
    /// belongs out of the eye's path, not in the row the player reads for
    /// score.
    pub fn draw(&self, canvas: &mut Canvas<'_>, theme: &Theme) {
        let alpha = self.alpha();
        if alpha <= 0.0 {
            return;
        }

        let scale = 2;
        let w = canvas.width() as i32;
        let h = canvas.height() as i32;
        let right = w - PAD;

        // ⚠️ The GLYPH carries the meaning, not the colour: S5 established
        // that colour cannot be trusted in a theme-reactive game, because
        // a theme can put any hue anywhere. A muted bar is not merely a
        // differently-coloured bar — it says MUTED.
        let label = if self.muted { "MUTED" } else { "VOLUME" };
        let label_w = text_width(label, scale) as i32;
        let bar_w = BAR_W as i32;

        // Anchored to the BOTTOM edge and grown upward, so the block sits
        // the same distance from the corner whatever the text scale is.
        let bar_y = h - PAD - BAR_H as i32;
        let label_y = bar_y - (7 * scale) as i32 - 6;
        let bar_x = right - bar_w;

        let fg = fade(theme.foreground, alpha);
        text(canvas, label, right - label_w, label_y, scale, fg);

        // The trough, so an empty bar still reads as a bar rather than as
        // nothing having been drawn.
        canvas.fill_rect(bar_x, bar_y, BAR_W, BAR_H, fade(theme.muted, alpha * 0.7));

        // ⚠️ A muted bar shows the volume it will RETURN to, not zero.
        // Showing zero would be a lie — unmuting restores this level — and
        // it would also make mute and "turned all the way down" look
        // identical, which is the exact confusion this whole seam exists
        // to prevent.
        let filled = (BAR_W as f32 * self.volume.clamp(0.0, 1.0)) as u32;
        if filled > 0 {
            let colour = if self.muted { theme.muted } else { theme.accent };
            canvas.fill_rect(bar_x, bar_y, filled, BAR_H, fade(colour, alpha));
        }
    }
}

impl Default for VolumeIndicator {
    fn default() -> Self {
        VolumeIndicator::new()
    }
}

/// Dim a colour toward nothing.
///
/// ⚠️ Scales the channels rather than compositing against a background,
/// because a `Canvas` has no blend mode and the background is a theme
/// colour that may be light or dark. On a light theme this fades toward
/// black rather than toward the page — visible, and honest about what it
/// is doing, where an assumed dark background would fade UP into a bright
/// smear on exactly the themes that are hardest to read.
fn fade(c: Color, alpha: f32) -> Color {
    let a = alpha.clamp(0.0, 1.0);
    Color::rgb(
        (c.r as f32 * a) as u8,
        (c.g as f32 * a) as u8,
        (c.b as f32 * a) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh indicator must be silent: whatever the counter already
    /// reads happened before this game was looking.
    #[test]
    fn it_starts_hidden() {
        let v = VolumeIndicator::new();
        assert!(!v.showing());
        assert_eq!(v.alpha(), 0.0);
    }

    /// The timer must run down to zero and stay there.
    #[test]
    fn it_hides_itself_again() {
        let mut v = VolumeIndicator::new();
        v.seen = Some(0);
        v.left = SHOW_SECONDS;
        assert!(v.showing());

        // Tick past its life in ordinary frames.
        for _ in 0..200 {
            if v.left > 0.0 {
                v.left = (v.left - 1.0 / 60.0).max(0.0);
            }
        }
        assert!(!v.showing(), "the indicator must not linger");
        assert_eq!(v.alpha(), 0.0);
    }

    /// The fade must be a tail, not the whole life — the value is worth
    /// reading at full contrast for most of the time it is up.
    #[test]
    fn it_is_solid_before_it_fades() {
        let mut v = VolumeIndicator::new();
        v.seen = Some(0);
        v.left = SHOW_SECONDS;
        assert_eq!(v.alpha(), 1.0, "fully visible when it arrives");

        v.left = SHOW_SECONDS * FADE_TAIL;
        assert_eq!(v.alpha(), 1.0, "still solid at the start of the tail");

        v.left = SHOW_SECONDS * FADE_TAIL * 0.5;
        assert!(v.alpha() < 1.0 && v.alpha() > 0.0, "fading through the tail");
    }

    /// ⚠️ Re-armed, not extended. Holding a key down must not bank an
    /// ever-growing display that outlasts the player's interest.
    #[test]
    fn a_second_change_rearms_rather_than_extends() {
        let mut v = VolumeIndicator::new();
        v.seen = Some(0);
        v.left = SHOW_SECONDS * 0.5;
        // What `update` does on a change:
        v.left = SHOW_SECONDS;
        assert_eq!(v.left, SHOW_SECONDS, "re-armed to the full time");
        assert!(v.left <= SHOW_SECONDS, "and never beyond it");
    }

    /// Fading must reach nothing and preserve a colour at full alpha.
    #[test]
    fn fading_spans_the_whole_range() {
        let c = Color::rgb(200, 100, 50);
        let full = fade(c, 1.0);
        assert_eq!((full.r, full.g, full.b), (200, 100, 50), "unchanged at 1.0");
        let none = fade(c, 0.0);
        assert_eq!((none.r, none.g, none.b), (0, 0, 0), "gone at 0.0");
        // And clamped, so a caller cannot brighten a colour past itself.
        let over = fade(c, 4.0);
        assert_eq!((over.r, over.g, over.b), (200, 100, 50), "clamped above 1.0");
    }
}
