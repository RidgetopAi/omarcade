//! Pause: the overlay every game in the suite shows when P is pressed.
//!
//! # Why this exists
//!
//! Brian played Pixel Break to level 10 and asked for a pause — in that
//! game "and actually all games". A run of Pixel Break is over an hour at
//! the floor, the racer wants to be put down mid-course, and Pong is
//! played against an opponent that never needs a break. Every one of them
//! needs the same thing, and none of them needs a different one.
//!
//! # Why it lives in core
//!
//! For the reason [`VolumeIndicator`](crate::VolumeIndicator) does: three
//! games need identical behaviour, and a per-game copy is three things to
//! keep in step. That indicator is the precedent this file follows all the
//! way down to its shape — state the game owns, ticked or drawn from the
//! game's own `update`/`render`, knowing nothing about the game's rules.
//!
//! # Why it is NOT a `Phase`
//!
//! Pixel Break models everything else as a phase, and the comment on
//! `Phase::Title` argues for that directly: *"A phase rather than a flag,
//! like every other state here."* Pause is the exception, and the reason
//! is that a phase answers **what the game is doing** — Title, Ready,
//! Playing, Clearing, Lost, Victory, Won — while pause is **orthogonal to
//! all of them**. You pause *a phase*, and on resume you must land back in
//! the one you left.
//!
//! As a phase it would have to store the phase it came from inside the
//! phase enum, and every `match` in `state.rs`, `render.rs` and both
//! probes would need a `Paused` arm meaning "whatever the previous one
//! meant". As a flag beside the game it is three lines per game and no
//! rule of the game changes at all.
//!
//! That is also why it is here and not in any game's `state.rs`: pausing
//! is not a rule of Breakout, it is a property of the cabinet.
//!
//! # Using it
//!
//! Three calls, in the three places a game already has:
//!
//! ```ignore
//! fn on_input(&mut self, event: InputEvent) -> bool {
//!     match event {
//!         InputEvent::KeyDown(Key::P) => self.pause.toggle(),
//!         ...
//!     }
//! }
//!
//! fn update(&mut self, dt: f32, audio: &mut Audio<'_>) {
//!     self.volume.update(audio, dt);   // ⚠️ BEFORE the pause gate
//!     if self.pause.is_paused() {
//!         return;
//!     }
//!     ...the simulation...
//! }
//!
//! fn render(&mut self, canvas: &mut Canvas<'_>) {
//!     ...the game's own picture...
//!     self.pause.draw(canvas, &self.theme);
//! }
//! ```
//!
//! ⚠️ **The volume indicator must tick BEFORE the pause gate**, or muting
//! a paused game would leave the indicator frozen on screen forever. The
//! volume keys are handled by the backend and answer in every phase; a
//! pause must not be the one state where they stop being answered. This is
//! the S7 rule — a decaying effect advances in every phase — meeting a new
//! way to get it wrong.
//!
//! ⚠️ **P only, never Escape.** Escape quits, in all three games
//! (`Key::Escape` returns `false` from every `on_input` in the suite).
//! Binding pause to it would silently break quit three times over.

use crate::text::{text, text_width, GLYPH_H};
use crate::{Canvas, Theme};

/// How far the field is dimmed behind the overlay, as an alpha.
///
/// ⚠️ Chosen so the board stays **readable**, not hidden. Brian picked
/// "freeze + dim + PAUSED" over a full-screen card for exactly this
/// reason: pausing to think means wanting to look at the board. At 150
/// the bricks still read clearly; past about 200 the field turns into a
/// texture and the pause stops being useful for the thing it is for.
const DIM_ALPHA: u8 = 150;

/// Gap between the PAUSED line and the hint under it, in pixels.
const GAP: i32 = 18;

/// Whether the game is paused, and the overlay that says so.
///
/// Holds no game state and needs none: a game keeps one of these beside
/// its own state exactly as it keeps a `VolumeIndicator`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pause {
    paused: bool,
}

impl Pause {
    /// A game that is running.
    pub const fn new() -> Self {
        Pause { paused: false }
    }

    /// Flip between paused and running. This is what P does.
    ///
    /// ⚠️ A toggle rather than `set(bool)`: one key with one meaning, so
    /// there is no way for a caller to get the two out of step.
    pub fn toggle(&mut self) {
        self.paused = !self.paused;
    }

    /// Whether the simulation should be held still this frame.
    ///
    /// ⚠️ The **game** decides what freezing means — core must not know
    /// that Pixel Break advances a `physics::step` and the racer does its
    /// own integration. All this type promises is the answer to "should
    /// time pass?".
    pub const fn is_paused(&self) -> bool {
        self.paused
    }

    /// Resume, whatever state the pause was in.
    ///
    /// For the cases a game must not stay paused through — a restart, or
    /// returning to a title screen — where leaving the flag set would
    /// strand the player on a frozen fresh game.
    pub fn resume(&mut self) {
        self.paused = false;
    }

    /// Dim the field and say PAUSED across the middle of the window.
    ///
    /// ⚠️ Centring on the WINDOW is wrong for most games, and Pixel Break
    /// is the proof: its window centre (y=360) lands just under the brick
    /// field, in the airspace the ball actually occupies. Prefer
    /// [`draw_centred_at`] and give it the middle of whatever band is
    /// genuinely clear. This is kept for the games whose window centre IS
    /// their clear space.
    pub fn draw(&self, canvas: &mut Canvas<'_>, theme: &Theme) {
        let mid = canvas.height() as i32 / 2;
        self.draw_centred_at(canvas, theme, mid);
    }

    /// Dim the field and say PAUSED centred on a given line.
    ///
    /// A no-op while running, so a game calls it unconditionally from
    /// `render` and cannot forget the `if`.
    ///
    /// ⚠️ Drawn LAST, over the game's own picture — it is a scrim, and a
    /// scrim under the field would dim nothing.
    ///
    /// ⚠️ **`centre_y` is the caller's job because core cannot know where
    /// a given game's clear space is**, and L051 is the standing warning
    /// against inferring a layout from a convention: the volume indicator
    /// was put in "the free corner" by reasoning rather than by reading
    /// `draw_hud`, and it landed on top of LIVES. Each game passes the
    /// middle of a band it has actually measured.
    pub fn draw_centred_at(&self, canvas: &mut Canvas<'_>, theme: &Theme, centre_y: i32) {
        if !self.paused {
            return;
        }

        let w = canvas.width();
        let h = canvas.height();

        // ⚠️ A NORMAL alpha blend toward the theme's own background, never
        // `fill_rect_add`. S7 established that additive blending saturates
        // on light surfaces — on a light theme an additive scrim would
        // wash the field out to white rather than dimming it. Blending
        // toward the background darkens a dark theme and lightens a light
        // one, which is "recede" in both.
        canvas.fill_rect(0, 0, w, h, theme.background.with_alpha(DIM_ALPHA));

        let w = w as i32;
        let h = h as i32;

        // ⚠️ The WORD carries the meaning, not the dimming — the S5 rule
        // that colour cannot be trusted in a theme-reactive game. A dimmed
        // field on its own is indistinguishable from a game that has hung,
        // which is the whole reason Brian rejected a bare freeze.
        let label = "PAUSED";
        let label_scale = 4;
        let label_w = text_width(label, label_scale) as i32;
        let label_h = (GLYPH_H * label_scale) as i32;

        // ⚠️ The hint NAMES THE KEY. P is not discoverable — nothing else
        // in the suite uses it, and the one key a player might guess,
        // Escape, quits. A pause nobody can leave is worse than none.
        let hint = "PRESS P TO RESUME";
        let hint_scale = 2;
        let hint_w = text_width(hint, hint_scale) as i32;
        let hint_h = (GLYPH_H * hint_scale) as i32;

        // Centred as one BLOCK: the two lines and the gap are measured
        // together, so the pair straddles `centre_y` rather than the first
        // line sitting on it and the second hanging below.
        //
        // Clamped into the window so a caller who passes a centre near an
        // edge gets a readable overlay rather than text off the screen.
        let block_h = label_h + GAP + hint_h;
        let top = (centre_y - block_h / 2).clamp(0, (h - block_h).max(0));

        text(
            canvas,
            label,
            (w - label_w) / 2,
            top,
            label_scale,
            theme.foreground,
        );
        text(
            canvas,
            hint,
            (w - hint_w) / 2,
            top + label_h + GAP,
            hint_scale,
            theme.muted,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_pause_is_running() {
        assert!(!Pause::new().is_paused(), "a game must not start paused");
    }

    #[test]
    fn toggling_pauses_and_toggling_again_resumes() {
        let mut p = Pause::new();
        p.toggle();
        assert!(p.is_paused(), "the first press must pause");
        p.toggle();
        assert!(!p.is_paused(), "the second press must resume");
    }

    /// ⚠️ A restart or a return to the title must never leave the player
    /// looking at a frozen fresh game.
    #[test]
    fn resume_clears_a_pause_however_it_was_set() {
        let mut p = Pause::new();
        p.toggle();
        p.resume();
        assert!(!p.is_paused());
        // And resuming an already-running game is harmless.
        p.resume();
        assert!(!p.is_paused());
    }

    #[test]
    fn the_default_matches_new() {
        assert_eq!(Pause::default().is_paused(), Pause::new().is_paused());
    }
}
