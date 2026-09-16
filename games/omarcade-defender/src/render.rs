//! Drawing the world.
//!
//! ⚠️ THE STAGE-1 NOTE SAID "NO EFFECTS": the particle pool and additive
//! blending stayed unused until the flying was settled, because a ship
//! that handles badly does not handle better with a glow on it. The
//! flying IS settled now (bb715b33, confirmed by play), so S5 spends
//! them — see [`crate::effects`].
//!
//! The ship is vector art now, drawn through core's antialiased
//! primitives and authored in `tools/vector-playground.html`. The shape
//! lives in [`crate::art`]; this module only places it.

use omarcade_core::{Canvas, Color, Theme, Transform};

use crate::art;
use crate::effects::Effects;
use crate::enemy::{Landers, Phase};
use crate::flight::{Camera, Ship};
use crate::shot::Shots;
use crate::world::{self, Terrain};

/// Everything a frame needs, so the signature does not grow a parameter
/// per feature. S5 alone would have taken `draw` to eight arguments.
pub struct Scene<'a> {
    pub terrain: &'a Terrain,
    pub ship: &'a Ship,
    pub camera: &'a Camera,
    pub shots: &'a Shots,
    pub landers: &'a Landers,
    pub effects: &'a Effects,
    pub score: u32,
}

/// Draw a frame.
///
/// Order is depth: sky, mountains, then the things in the air, then the
/// HUD over everything. Explosions draw AFTER the enemies that made them
/// so debris passes in front of a neighbouring Lander rather than
/// mysteriously behind it.
pub fn draw(canvas: &mut Canvas<'_>, scene: &Scene<'_>, theme: &Theme) {
    canvas.clear(sky(theme));
    draw_terrain(canvas, scene.terrain, scene.camera, theme);
    draw_landers(canvas, scene.landers, scene.camera);
    draw_shots(canvas, scene.shots, scene.camera, theme);
    scene.effects.draw(canvas, scene.camera);
    draw_ship(canvas, scene.ship, scene.camera, theme);
    draw_hud(canvas, scene.ship, scene.camera, theme);
    draw_score(canvas, scene.score, theme);
}

/// The Landers.
///
/// ⚠️ EVERY ENEMY IS TESTED FOR VISIBILITY, NOT DRAWN BLIND. The world is
/// four screens wide, so most of them are somewhere else; `to_screen`
/// already wraps, so this is only about skipping work.
fn draw_landers(canvas: &mut Canvas<'_>, landers: &Landers, camera: &Camera) {
    let h = canvas.height() as f32;
    let margin = crate::enemy::LANDER_HALF_W + 4.0;

    for l in landers.iter() {
        let sx = camera.to_screen(l.x);
        if sx < -margin || sx > canvas.width() as f32 + margin {
            continue;
        }
        let sy = h - l.y;

        match l.phase {
            // Arriving: a vertical streak that collapses into the shape,
            // which is what a warp-in reads as. Drawn as a scale rather
            // than a fade — a Lander fading in could be mistaken for one
            // that is merely far away.
            Phase::Warping => {
                let p = l.progress();
                let t = Transform::at(sx, sy).scaled(art::SCALE * (0.15 + 0.85 * p));
                art::draw_lander(canvas, &t);
            }
            Phase::Hovering => {
                art::draw_lander(canvas, &Transform::at(sx, sy).scaled(art::SCALE));
            }
            // Dying: one bright frame of the shape blowing outward,
            // under the particles. Short enough that it reads as the
            // instant of destruction rather than as an animation.
            Phase::Dying => {
                let p = l.progress();
                let t = Transform::at(sx, sy).scaled(art::SCALE * (1.0 + p * 0.9));
                art::draw_lander(canvas, &t);
            }
        }
    }
}

/// The laser bolts.
///
/// ⚠️ DRAWN FROM THE TAIL TO THE HEAD THROUGH THE CAMERA, NOT AS A RECT
/// BETWEEN TWO WORLD COORDINATES. A bolt straddling the seam has a head
/// near 0 and a tail near WORLD_W; subtracting those gives a streak
/// almost four screens long across the whole window.
fn draw_shots(canvas: &mut Canvas<'_>, shots: &Shots, camera: &Camera, theme: &Theme) {
    let h = canvas.height() as f32;
    let hot = theme.foreground.lerp(Color::rgb(255, 255, 255), 0.6);

    for s in shots.iter() {
        let head = camera.to_screen(s.x);
        // The tail is placed by measuring BACK from the head on screen,
        // which cannot straddle anything.
        let tail = head - s.vx.signum() * crate::shot::SHOT_LENGTH;
        let (x0, x1) = if tail < head { (tail, head) } else { (head, tail) };

        if x1 < 0.0 || x0 > canvas.width() as f32 {
            continue;
        }
        let y = h - s.y;
        canvas.fill_rect_add_f(x0, y - 1.5, x1 - x0, 3.0, hot);
    }
}

/// The score.
///
/// ⚠️ NOT THE REAL HUD, AND NOT PERSISTED. S9 owns scoring, waves and the
/// score file. This exists so that shooting something has a visible
/// consequence while S5 is being judged — a kill that changes nothing on
/// screen is hard to tell from a miss.
fn draw_score(canvas: &mut Canvas<'_>, score: u32, theme: &Theme) {
    let text = format!("{score:06}");
    let w = omarcade_core::text::text_width(&text, 3) as i32;
    omarcade_core::text::text(
        canvas,
        &text,
        (canvas.width() as i32 - w) / 2,
        14,
        3,
        theme.foreground,
    );
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
            let (shots, landers, fx) = (Shots::new(), Landers::new(), Effects::new());
            let scene = bare_scene(&t, &ship, &cam, &shots, &landers, &fx);
            draw(&mut c, &scene, &theme);
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
            let (shots, landers, fx) = (Shots::new(), Landers::new(), Effects::new());
            let scene = bare_scene(&t, &ship, &cam, &shots, &landers, &fx);
            draw(&mut c, &scene, &theme);
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

    /// A scene with nothing in it but the world and the ship — what the
    /// terrain tests are actually about.
    fn bare_scene<'a>(
        terrain: &'a Terrain,
        ship: &'a Ship,
        camera: &'a Camera,
        shots: &'a Shots,
        landers: &'a Landers,
        effects: &'a Effects,
    ) -> Scene<'a> {
        Scene { terrain, ship, camera, shots, landers, effects, score: 0 }
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
            let (shots, landers, fx) = (Shots::new(), Landers::new(), Effects::new());
            let scene = bare_scene(&t, &ship, &cam, &shots, &landers, &fx);
            draw(&mut c, &scene, &theme);
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
