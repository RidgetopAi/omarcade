//! Score pop-ups: a number where something was earned.
//!
//! ★ THE ORIGINAL'S "250" AND "500" SPRITES. A rescue scores away from
//! the score display — down at the surface, under the ship — and without
//! a number at the spot the player cannot tell a catch that paid from
//! one that did not. Drawn in the world, so they scroll with it.

use omarcade_core::{Canvas, Color};

use crate::flight::Camera;
use crate::world;

/// How long one stays up, seconds.
pub const LIFE: f32 = 1.1;
/// How far it rises over its life, world units.
const RISE: f32 = 40.0;
/// At most this many at once; the oldest goes first.
const MAX: usize = 12;

#[derive(Debug, Clone, Copy)]
struct Popup {
    x: f32,
    y: f32,
    points: u32,
    age: f32,
}

#[derive(Debug, Default)]
pub struct Popups {
    live: Vec<Popup>,
}

impl Popups {
    pub const fn new() -> Self {
        Self { live: Vec::new() }
    }

    /// Show `points` at world `(x, y)`.
    pub fn add(&mut self, x: f32, y: f32, points: u32) {
        if self.live.len() >= MAX {
            self.live.remove(0);
        }
        self.live.push(Popup { x: world::wrap(x), y, points, age: 0.0 });
    }

    pub fn step(&mut self, dt: f32) {
        for p in &mut self.live {
            p.age += dt;
        }
        self.live.retain(|p| p.age < LIFE);
    }

    pub fn clear(&mut self) {
        self.live.clear();
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.live.len()
    }

    /// ⚠️ A FIXED COLOUR, NOT THE THEME'S. A score is game information
    /// and holds still under every theme (L065), exactly as threat
    /// colours do.
    pub fn draw(&self, canvas: &mut Canvas<'_>, camera: &Camera) {
        let h = canvas.height() as f32;
        for p in &self.live {
            let t = p.age / LIFE;
            let sx = camera.to_screen(p.x);
            if !(-60.0..=canvas.width() as f32 + 60.0).contains(&sx) {
                continue;
            }
            // `y` is up from the world floor; the canvas counts down.
            let sy = h - (p.y + RISE * t);
            let fade = if t < 0.7 { 1.0 } else { 1.0 - (t - 0.7) / 0.3 };
            let colour = Color::rgb(255, 224, 96).lerp(Color::rgb(0, 0, 0), 1.0 - fade);
            let text = p.points.to_string();
            let w = omarcade_core::text::text_width(&text, 2) as i32;
            omarcade_core::text::text(canvas, &text, sx as i32 - w / 2, sy as i32, 2, colour);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_popup_expires_after_its_life() {
        let mut p = Popups::new();
        p.add(100.0, 200.0, 500);
        p.step(LIFE * 0.5);
        assert_eq!(p.len(), 1);
        p.step(LIFE * 0.6);
        assert_eq!(p.len(), 0);
    }

    #[test]
    fn the_oldest_goes_when_full() {
        let mut p = Popups::new();
        for i in 0..(MAX as u32 + 3) {
            p.add(0.0, 0.0, i);
        }
        assert_eq!(p.len(), MAX);
        assert_eq!(p.live[0].points, 3);
    }
}
