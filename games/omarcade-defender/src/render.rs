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
use crate::enemy::{Kind, Landers, Phase};
use crate::flight::{Camera, Ship};
use crate::humanoid::{self, Humanoids, State};
use crate::lives::Lives;
use crate::shot::Owner;
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
    pub people: &'a Humanoids,
    pub effects: &'a Effects,
    pub score: u32,
    pub lives: &'a Lives,
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
    draw_people(canvas, scene.people, scene.camera);
    draw_beams(canvas, scene.landers, scene.people, scene.camera);
    draw_landers(canvas, scene.landers, scene.camera);
    draw_shots(canvas, scene.shots, scene.camera, theme);
    scene.effects.draw(canvas, scene.camera);
    // ⚠️ A DEAD OR BLINKING SHIP IS NOT ALWAYS DRAWN. `is_visible`
    // answers both questions, so this does not need to know which state
    // the ship is in.
    if scene.lives.is_visible() {
        draw_ship(canvas, scene.ship, scene.camera, theme);
    }
    draw_hud(canvas, scene.ship, scene.camera, theme);
    draw_score(canvas, scene.score, theme);
    draw_lives(canvas, scene.lives, theme);
    if scene.lives.is_game_over() {
        draw_game_over(canvas, theme);
    }
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
                draw_enemy(canvas, l.kind, sx, sy, art::SCALE * (0.15 + 0.85 * p));
            }
            // Hunting and carrying look the same as hovering — the
            // TRACTOR BEAM is what tells you which is which, and it is
            // drawn separately so it sits under the Lander.
            Phase::Hovering | Phase::Hunting | Phase::Grabbing | Phase::Carrying => {
                draw_enemy(canvas, l.kind, sx, sy, art::SCALE);
            }
            // Dying: one bright frame of the shape blowing outward,
            // under the particles. Short enough that it reads as the
            // instant of destruction rather than as an animation.
            Phase::Dying => {
                let p = l.progress();
                draw_enemy(canvas, l.kind, sx, sy, art::SCALE * (1.0 + p * 0.9));
            }
        }
    }
}

/// The people.
///
/// Drawn UNDER the Landers and the beams, so an abduction reads as
/// something happening TO them.
fn draw_people(canvas: &mut Canvas<'_>, people: &Humanoids, camera: &Camera) {
    let h = canvas.height() as f32;
    let margin = humanoid::HALF_W + 4.0;

    for p in people.iter() {
        if p.state == State::Dead {
            continue;
        }
        let sx = camera.to_screen(p.x);
        if sx < -margin || sx > canvas.width() as f32 + margin {
            continue;
        }
        // The art's origin is its middle; y is the ground they stand on.
        let sy = h - p.y - humanoid::HALF_H;
        art::draw_humanoid(
            canvas,
            &Transform::at(sx, sy).scaled(art::SCALE).flipped(p.vx < 0.0),
        );
    }
}

/// One enemy, of whichever kind.
///
/// ★ THE MUTANT ART IS THE LANDER'S, WEARING A PERSON. Brian built it by
/// importing the Lander into the playground and drawing a Humanoid into
/// the pod, so the two occupy exactly the same space on screen — which
/// is what makes the fusion read as a fusion rather than as a swap.
fn draw_enemy(canvas: &mut Canvas<'_>, kind: Kind, sx: f32, sy: f32, scale: f32) {
    let t = Transform::at(sx, sy).scaled(scale);
    match kind {
        Kind::Lander => art::draw_lander(canvas, &t),
        Kind::Mutant => art::draw_mutant(canvas, &t),
    }
}

/// The tractor beams.
///
/// ★ THE BEAM IS THE WARNING, and it is the only thing that tells a
/// player an abduction is under way in time to stop it. A Lander that
/// simply descended and rose again would give no signal at all, and the
/// first time you noticed would be when someone was already gone.
fn draw_beams(canvas: &mut Canvas<'_>, landers: &Landers, people: &Humanoids, camera: &Camera) {
    let h = canvas.height() as f32;

    for l in landers.iter() {
        let Some(who) = l.carrying() else { continue };
        let Some(p) = people.get(who) else { continue };

        let sx = camera.to_screen(l.x);
        if sx < -40.0 || sx > canvas.width() as f32 + 40.0 {
            continue;
        }

        let top = h - l.y;
        let bottom = h - p.y;
        if bottom <= top {
            continue;
        }

        // ⚠️ WIDER AND BRIGHTER AT THE BOTTOM, WHICH IS THE OPPOSITE OF
        // the first version and only obvious at 8x. A beam that fades as
        // it widens has its widest part at its dimmest, so it reads as a
        // rod tapering to a point at the victim — the eye follows
        // brightness, not geometry. Light pouring DOWN means the pool of
        // it is on the person, which is also where the player needs to
        // be looking.
        let steps = 16;
        for i in 0..steps {
            let t0 = i as f32 / steps as f32;
            let t1 = (i + 1) as f32 / steps as f32;
            let y0 = top + (bottom - top) * t0;
            let y1 = top + (bottom - top) * t1;
            let half = 2.5 + 6.0 * t0 * t0;
            let glow = 0.30 + 0.55 * t0;
            let c = Color::rgb(
                (70.0 * glow) as u8,
                (235.0 * glow) as u8,
                (120.0 * glow) as u8,
            );
            canvas.fill_rect_add_f(sx - half, y0, half * 2.0, y1 - y0, c);
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

        // ⚠️ AN ENEMY BOLT IS DRAWN ALONG ITS OWN VELOCITY, not
        // horizontally. Mutants are required never to fire level, so a
        // horizontal streak would draw every angled shot as if it were
        // flat and the one rule that makes them dangerous would be
        // invisible.
        let speed = (s.vx * s.vx + s.vy * s.vy).sqrt().max(1.0);
        let len = crate::shot::SHOT_LENGTH;
        let tail_x = head - (s.vx / speed) * len;
        let y_head = h - s.y;
        // Screen y grows DOWN while world y grows UP, so the tail's
        // screen offset is the opposite sign of the world velocity.
        let tail_y = y_head + (s.vy / speed) * len;

        let (x0, x1) = if tail_x < head { (tail_x, head) } else { (head, tail_x) };
        if x1 < 0.0 || x0 > canvas.width() as f32 {
            continue;
        }

        let color = if s.owner == Owner::Enemy {
            Color::rgb(255, 120, 60)
        } else {
            hot
        };

        if s.vy.abs() < 0.001 {
            canvas.fill_rect_add_f(x0, y_head - 1.5, x1 - x0, 3.0, color);
        } else {
            canvas.line_add_f(tail_x, tail_y, head, y_head, 3.0, color);
        }
    }
}

/// The lives left, as ships.
///
/// ★ SHIPS, NOT A NUMBER. A count has to be read; a row of ships is
/// understood at a glance, which is the only kind of reading a player
/// does mid-flight.
fn draw_lives(canvas: &mut Canvas<'_>, lives: &Lives, _theme: &Theme) {
    for i in 0..lives.remaining {
        let x = 28.0 + i as f32 * 42.0;
        art::draw_ship(canvas, &Transform::at(x, 44.0).scaled(0.55));
    }
}

/// ⚠️ NOT A REAL GAME-OVER SCREEN. S14 owns presentation; this exists so
/// that running out of lives is unmistakable rather than a ship that
/// silently stops coming back.
fn draw_game_over(canvas: &mut Canvas<'_>, theme: &Theme) {
    let msg = "GAME OVER";
    let w = omarcade_core::text::text_width(msg, 5) as i32;
    omarcade_core::text::text(
        canvas,
        msg,
        (canvas.width() as i32 - w) / 2,
        canvas.height() as i32 / 2 - 20,
        5,
        theme.foreground,
    );
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
    // ★ THE WORLD ENDED: there is nothing down there any more. Returning
    // early rather than drawing a flat line at zero, because a line
    // would read as ground you could still land on.
    if terrain.is_destroyed() {
        return;
    }
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
            let people = Humanoids::new();
            let lives = Lives::new();
            let scene = bare_scene(&t, &ship, &cam, &shots, &landers, &people, &fx, &lives);
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
            let people = Humanoids::new();
            let lives = Lives::new();
            let scene = bare_scene(&t, &ship, &cam, &shots, &landers, &people, &fx, &lives);
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
    #[allow(clippy::too_many_arguments)]
    fn bare_scene<'a>(
        terrain: &'a Terrain,
        ship: &'a Ship,
        camera: &'a Camera,
        shots: &'a Shots,
        landers: &'a Landers,
        people: &'a Humanoids,
        effects: &'a Effects,
        lives: &'a Lives,
    ) -> Scene<'a> {
        Scene { terrain, ship, camera, shots, landers, people, effects, score: 0, lives }
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
            let people = Humanoids::new();
            let lives = Lives::new();
            let scene = bare_scene(&t, &ship, &cam, &shots, &landers, &people, &fx, &lives);
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
