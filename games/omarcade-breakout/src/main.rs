//! Breakout — the first Omarcade title.
//!
//! Session 1 is the chassis, not the game: this opens a window in the
//! user's live theme colours and closes on Esc. The paddle, ball and
//! bricks arrive in session 2.
//!
//! Worth noticing what this file does *not* import. There is no winit
//! and no softbuffer here, and none in this crate's Cargo.toml either.
//! Everything comes through `omarcade_core`'s seam, which is what makes
//! the later layer-shell backend a drop-in rather than a rewrite.

use omarcade_core::backend::winit_soft::{Idle, WinitBackend};
use omarcade_core::{Backend, Canvas, Game, InputEvent, Key, Theme};

const TITLE: &str = "Omarcade Breakout";
const WIDTH: u32 = 960;
const HEIGHT: u32 = 720;

struct Breakout {
    theme: Theme,
    /// Set once a resize is seen, so the test pattern can be drawn
    /// relative to the real surface rather than the requested size.
    size: (u32, u32),
}

impl Breakout {
    fn new(theme: Theme) -> Self {
        Breakout { theme, size: (WIDTH, HEIGHT) }
    }
}

impl Game for Breakout {
    fn on_input(&mut self, event: InputEvent) -> bool {
        match event {
            // Esc quits. Session 2 will want this to open a pause menu
            // instead, which is exactly why the seam lets the game
            // decide rather than the backend.
            InputEvent::KeyDown(Key::Escape) => false,

            InputEvent::Resized { width, height } => {
                self.size = (width, height);
                true
            }

            // The compositor is closing us; nothing to save yet.
            InputEvent::CloseRequested => true,

            _ => true,
        }
    }

    fn update(&mut self, _dt: f32) {
        // Nothing moves yet. When it does, this is where it moves — and
        // the backend switches from Idle::Wait to Idle::Animate.
    }

    fn render(&mut self, canvas: &mut Canvas<'_>) {
        let t = &self.theme;
        canvas.clear(t.background);

        let (w, h) = (canvas.width() as i32, canvas.height() as i32);

        // A row of theme swatches. This is a check, not decoration: if
        // these are the colours in colors.toml then the theme really is
        // live, and if the last one is not cut off at the right edge
        // then fill_rect is clipping correctly on a real surface.
        let swatches = [t.red, t.orange, t.yellow, t.green, t.cyan, t.blue, t.magenta];
        let sw = 96;
        let sh = 96;
        let total = sw * swatches.len() as i32;
        let x0 = (w - total) / 2;
        let y0 = (h - sh) / 2;

        for (i, colour) in swatches.iter().enumerate() {
            canvas.fill_rect(x0 + i as i32 * sw, y0, sw as u32, sh as u32, *colour);
        }

        // Foreground bar above the swatches, and an accent underline
        // deliberately drawn wider than the window so it must clip.
        canvas.fill_rect(x0, y0 - 48, total as u32, 16, t.foreground);
        canvas.fill_rect(-40, y0 + sh + 32, (w + 200) as u32, 8, t.accent);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Read the palette once at startup. Reacting to a live theme change
    // is a theme-set.d hook, and belongs with the marquee work.
    let theme = Theme::load();

    WinitBackend::new(TITLE, WIDTH, HEIGHT)
        // Nothing animates in session 1, so block until the compositor
        // has something for us. This is the ~0% idle CPU requirement.
        .idle(Idle::Wait)
        .run(Breakout::new(theme))?;

    Ok(())
}
