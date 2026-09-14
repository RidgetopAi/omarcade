//! Drawing the world.
//!
//! ⚠️ NO EFFECTS THIS STAGE. Brian: "no special effects now". The core
//! already has a particle pool and additive blending, and both stay
//! unused until the flying is settled — a ship that handles badly does
//! not handle better with a glow on it.
//!
//! The ship is vector art now, drawn through core's antialiased
//! primitives and authored in `tools/vector-playground.html`. The shape
//! lives in [`crate::art`]; this module only places it.

use omarcade_core::{Canvas, Color, Theme, Transform};

use crate::art;
use crate::flight::{Camera, Ship};
use crate::world::{self, Terrain};

/// Draw a frame.
pub fn draw(canvas: &mut Canvas<'_>, terrain: &Terrain, ship: &Ship, camera: &Camera, theme: &Theme) {
    canvas.clear(sky(theme));
    draw_terrain(canvas, terrain, camera, theme);
    draw_ship(canvas, ship, camera, theme);
    draw_hud(canvas, ship, camera, theme);
}

fn sky(theme: &Theme) -> Color {
    // Darker than the theme's background, so the mountains read against
    // it without needing a colour of their own.
    theme.background.lerp(Color::rgb(0, 0, 0), 0.35)
}

/// The mountains.
///
/// ⚠️ THE VISIBLE STRIP CAN SPAN THE SEAM. With the camera near x = 0 the
/// left of the screen is the far EAST end of the world and the right is
/// the west end. Walking world x from `camera.left()` forward and
/// wrapping each sample handles that without a special case — which is
/// the point of doing it this way rather than slicing the height array.
fn draw_terrain(canvas: &mut Canvas<'_>, terrain: &Terrain, camera: &Camera, theme: &Theme) {
    let w = canvas.width() as i32;
    let h = canvas.height() as f32;

    // World units per screen pixel. The canvas is a fixed 960x720 and
    // the view is one screen wide, so this is 1 — but deriving it means
    // a future zoom does not silently break the terrain.
    let scale = world::VIEW_W / canvas.width() as f32;

    // ⚠️ THE FILL HAS TO READ AS MASS. A first version tinted the
    // background 15% toward black, which against an already dark sky was
    // invisible — rendered, the mountains were a thin wire hanging in
    // space rather than terrain. The body is now lifted TOWARD the
    // foreground so it is solid ground with a lit edge on top.
    let ridge = theme.foreground.lerp(theme.background, 0.25);
    let fill = theme.background.lerp(theme.foreground, 0.13);

    let left = camera.left();
    for sx in 0..w {
        let wx = left + sx as f32 * scale;
        let height = terrain.height_at(wx);

        // `y` is measured UP from the world floor; the canvas measures
        // DOWN from the top. One subtraction, in one place.
        let top = (h - height).round() as i32;
        let depth = (h as i32 - top).max(0) as u32;
        if depth == 0 {
            continue;
        }

        // The body of the mountain, then a brighter line along its top
        // edge so the ridge reads as a silhouette rather than a mass.
        canvas.fill_rect(sx, top, 1, depth, fill);
        canvas.fill_rect(sx, top, 1, RIDGE_LINE_PX, ridge);
    }
}

const RIDGE_LINE_PX: u32 = 2;

/// The ship.
///
/// The art is vector, authored in `tools/vector-playground.html` and
/// living in [`crate::art`]. Nothing about the shape is decided here —
/// this only says where it goes and which way it faces.
fn draw_ship(canvas: &mut Canvas<'_>, ship: &Ship, camera: &Camera, _theme: &Theme) {
    let sx = camera.to_screen(ship.x);
    let sy = canvas.height() as f32 - ship.y;

    // ⚠️ FACING IS A MIRROR, NOT A HALF TURN. `.facing(PI)` would roll the
    // ship inverted — canopy underneath, fin pointing down. The old
    // placeholder was symmetric about its long axis and so could not show
    // the difference; this art can.
    let t = Transform::at(sx, sy)
        .scaled(art::SCALE)
        .flipped(ship.facing.sign() < 0.0);

    art::draw_ship(canvas, &t);
}

/// The minimum a pilot needs: how fast, and where in the world.
///
/// Not the real HUD — Defender's scanner belongs to a later stage, when
/// there is something on it worth scanning for. This exists so the
/// flying can be judged: without a position read-out it is genuinely
/// hard to tell a wrapping world from a treadmill.
fn draw_hud(canvas: &mut Canvas<'_>, ship: &Ship, camera: &Camera, theme: &Theme) {
    let w = canvas.width() as f32;
    let muted = theme.foreground.lerp(theme.background, 0.6);

    // A speed bar, because a number cannot be read at a glance while
    // flying and a bar can.
    let bar_w = 160.0;
    let bar_x = w - bar_w - 18.0;
    canvas.fill_rect_f(bar_x, 18.0, bar_w, 6.0, muted);
    let filled = bar_w * ship.speed_fraction();
    if filled > 0.0 {
        let colour = if ship.vx < 0.0 { theme.accent } else { theme.foreground };
        // Fills from the side the ship is travelling toward, so the bar
        // reads as direction as well as magnitude.
        let x = if ship.vx < 0.0 { bar_x + bar_w - filled } else { bar_x };
        canvas.fill_rect_f(x, 18.0, filled, 6.0, colour);
    }

    // A strip showing where in the loop the view is. One screen's worth
    // of the world, marked on a line representing the whole lap.
    let strip_w = 160.0;
    let strip_x = 18.0;
    canvas.fill_rect_f(strip_x, 18.0, strip_w, 6.0, muted);
    let here = world::wrap(camera.x) / world::WORLD_W;
    let window = strip_w / world::WORLD_SCREENS;
    canvas.fill_rect_f(
        strip_x + here * strip_w - window * 0.5,
        18.0,
        window,
        6.0,
        theme.foreground,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flight::Ship;

    fn canvas_of(buf: &mut [u32]) -> Canvas<'_> {
        Canvas::new(buf, 960, 720)
    }

    /// ⚠️ COUNTING NON-BACKGROUND PIXELS IS NOT A RENDER TEST — the
    /// project has said so since session one. This asserts something
    /// specific instead: that the terrain reaches the BOTTOM of the
    /// screen in every column, because a mountain with sky under it is
    /// the failure mode of getting the y flip wrong.
    #[test]
    fn the_terrain_is_solid_to_the_bottom_of_the_screen() {
        let mut buf = vec![0u32; 960 * 720];
        let t = Terrain::generate(256, 5);
        let ship = Ship::new(0.0);
        let cam = Camera::new(ship.x);
        let theme = Theme::fallback();
        {
            let mut c = canvas_of(&mut buf);
            draw(&mut c, &t, &ship, &cam, &theme);
        }

        let sky_px = sky(&theme).to_u32();
        for x in (0..960).step_by(37) {
            let bottom = buf[719 * 960 + x];
            assert_ne!(
                bottom, sky_px,
                "column {x} has sky at the very bottom — the ridge is drawn upside down"
            );
        }
    }

    /// The ridge follows the terrain rather than sitting at a fixed
    /// height: sample where the solid part starts in each column and
    /// check it varies.
    #[test]
    fn the_drawn_ridge_actually_undulates() {
        let mut buf = vec![0u32; 960 * 720];
        let t = Terrain::generate(256, 5);
        let ship = Ship::new(0.0);
        let cam = Camera::new(ship.x);
        let theme = Theme::fallback();
        {
            let mut c = canvas_of(&mut buf);
            draw(&mut c, &t, &ship, &cam, &theme);
        }

        let sky_px = sky(&theme).to_u32();
        let mut tops = Vec::new();
        for x in (0..960).step_by(11) {
            let top = (0..720).find(|y| buf[y * 960 + x] != sky_px).unwrap_or(720);
            tops.push(top);
        }
        let lo = *tops.iter().min().unwrap();
        let hi = *tops.iter().max().unwrap();
        assert!(
            hi - lo > 20,
            "the drawn ridge only varies by {} pixels across the screen — it is flat",
            hi - lo
        );
    }

    /// ⚠️ THE SEAM, IN THE RENDERER. With the camera at x = 0 the left
    /// half of the screen is the east end of the world. If the terrain
    /// were sliced from the array rather than sampled through `wrap`,
    /// this is where it would tear.
    #[test]
    fn the_terrain_draws_across_the_seam() {
        let mut buf = vec![0u32; 960 * 720];
        let t = Terrain::generate(256, 5);
        let ship = Ship::new(0.0);
        let cam = Camera::new(0.0);
        let theme = Theme::fallback();
        {
            let mut c = canvas_of(&mut buf);
            draw(&mut c, &t, &ship, &cam, &theme);
        }

        let sky_px = sky(&theme).to_u32();
        // Every column must still have ground in it, including the ones
        // that read from the far end of the height array.
        for x in 0..960 {
            let bottom = buf[719 * 960 + x];
            assert_ne!(bottom, sky_px, "column {x} has no ground across the seam");
        }
    }
}
