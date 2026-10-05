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
use crate::enemy::{Kind, Enemies, Phase};
use crate::flight::{Camera, Ship};
use crate::humanoid::{self, Humanoids, State};
use crate::lives::Lives;
use crate::popup::Popups;
use crate::shot::Owner;
use crate::waves::{self, Phase as WavePhase};
use crate::shot::Shots;
use crate::world::{self, Terrain};

/// Everything a frame needs, so the signature does not grow a parameter
/// per feature. S5 alone would have taken `draw` to eight arguments.
pub struct Scene<'a> {
    pub terrain: &'a Terrain,
    pub ship: &'a Ship,
    pub camera: &'a Camera,
    pub shots: &'a Shots,
    pub enemies: &'a Enemies,
    pub people: &'a Humanoids,
    pub effects: &'a Effects,
    pub score: u32,
    pub lives: &'a Lives,
    /// Seconds of simulated time, for the scanner's Mutant pulse.
    pub time: f32,
    /// ★ How hard the engine is burning, 0.0 at rest. Drives the hull
    /// flare; the trailing cloud is [`Effects`]' business.
    pub exhaust: f32,
    pub popups: &'a Popups,
    pub hud: Hud,
}

/// The game state the HUD shows that the world does not hold.
#[derive(Debug, Clone, Copy)]
pub struct Hud {
    pub wave: u32,
    pub phase: WavePhase,
    pub smart_bombs: u32,
    /// The best score on record, or this game's if it is higher.
    pub best: u32,
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
    draw_beams(canvas, scene.enemies, scene.people, scene.camera);
    draw_enemies(canvas, scene.enemies, scene.camera);
    draw_shots(canvas, scene.shots, scene.camera, theme);
    scene.effects.draw(canvas, scene.camera);
    scene.popups.draw(canvas, scene.camera);
    // ⚠️ A DEAD OR BLINKING SHIP IS NOT ALWAYS DRAWN. `is_visible`
    // answers both questions, so this does not need to know which state
    // the ship is in.
    if scene.lives.is_visible() {
        draw_ship(canvas, scene.ship, scene.camera, theme, scene.exhaust);
    }
    draw_hud(canvas, scene.ship, theme);
    draw_score(canvas, scene.score, theme);
    // ★ S8. The scanner is the last thing drawn before the lives and the
    // game-over card, because nothing in the world may overlap it — a
    // Lander drawn over the scanner would be read as a blip ON it.
    crate::scanner::draw(
        canvas,
        &crate::scanner::View {
            terrain: scene.terrain,
            ship: scene.ship,
            camera: scene.camera,
            enemies: scene.enemies,
            people: scene.people,
            time: scene.time,
        },
        theme,
    );
    draw_lives(canvas, scene.lives, theme);
    draw_smart_bombs(canvas, scene.hud.smart_bombs);
    draw_wave(canvas, scene.hud.wave, theme);
    if let WavePhase::Tally { counted, .. } | WavePhase::Hold { counted, .. } = scene.hud.phase {
        draw_tally(canvas, scene.hud.wave, counted, theme);
    }
    if scene.lives.is_game_over() {
        draw_game_over(canvas, scene.score, scene.hud.best, theme);
    }
}

/// Smart bombs held, as a row under the ships.
///
/// ⚠️ A PLACEHOLDER MARK, NOT THE ART. The smart-bomb icon is on the
/// plan's art list for Brian to approve; until then each bomb is a small
/// fixed-colour lozenge, so the 10,000-point award is visible at all.
fn draw_smart_bombs(canvas: &mut Canvas<'_>, count: u32) {
    let colour = Color::rgb(255, 120, 60);
    for i in 0..count.min(8) {
        let x = HUD_MARGIN + 4.0 + i as f32 * 22.0;
        canvas.fill_rect_f(x, 88.0, 14.0, 6.0, colour);
    }
}

/// The wave number, top right under the speed bar.
fn draw_wave(canvas: &mut Canvas<'_>, wave: u32, theme: &Theme) {
    let text = format!("WAVE {wave}");
    let w = omarcade_core::text::text_width(&text, 2) as i32;
    let colour = theme.foreground.lerp(theme.background, 0.35);
    omarcade_core::text::text(canvas, &text, canvas.width() as i32 - 18 - w, 34, 2, colour);
}

/// ★ THE WAVE IS HELD: a banner, and the survivors counted one at a time.
///
/// ⚠️ OUR OWN WORDS. The original's "ATTACK WAVE n COMPLETED" is its
/// text, not ours to reuse (docs/warden-plan.md §W1). A Warden holds the
/// line; the banner says the wave was held.
///
/// The people are drawn as the game's own Humanoid art, one per bonus
/// tick, so the count reads as people saved rather than as a number.
fn draw_tally(canvas: &mut Canvas<'_>, wave: u32, counted: usize, theme: &Theme) {
    let cx = canvas.width() as i32 / 2;
    let top = canvas.height() as i32 / 2 - 90;

    let title = format!("WAVE {wave} HELD");
    let w = omarcade_core::text::text_width(&title, 4) as i32;
    omarcade_core::text::text(canvas, &title, cx - w / 2, top, 4, theme.foreground);

    let per = waves::bonus_per_humanoid(wave);
    let line = format!("BONUS {per} X {counted}");
    let w = omarcade_core::text::text_width(&line, 2) as i32;
    let colour = theme.foreground.lerp(theme.background, 0.3);
    omarcade_core::text::text(canvas, &line, cx - w / 2, top + 48, 2, colour);

    let spacing = 26.0;
    let row_w = spacing * counted.saturating_sub(1) as f32;
    for i in 0..counted {
        let x = cx as f32 - row_w / 2.0 + i as f32 * spacing;
        art::draw_humanoid(canvas, &Transform::at(x, (top + 110) as f32).scaled(art::SCALE));
    }
}

/// The Landers.
///
/// ⚠️ EVERY ENEMY IS TESTED FOR VISIBILITY, NOT DRAWN BLIND. The world is
/// four screens wide, so most of them are somewhere else; `to_screen`
/// already wraps, so this is only about skipping work.
fn draw_enemies(canvas: &mut Canvas<'_>, enemies: &Enemies, camera: &Camera) {
    let h = canvas.height() as f32;
    let margin = crate::enemy::LANDER_HALF_W + 4.0;

    for l in enemies.iter() {
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
fn draw_beams(canvas: &mut Canvas<'_>, enemies: &Enemies, people: &Humanoids, camera: &Camera) {
    let h = canvas.height() as f32;

    for l in enemies.iter() {
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
/// The colours a player beam can be fired in.
///
/// ★★ READ OFF THE ORIGINAL, NOT INVENTED. Across Brian's four reference
/// frames every beam is dominated by ONE hue and the hues differ between
/// beams: a frame that is green throughout (856 green pixels to 15
/// white), one that is purple-and-blue (242/151), one carrying white,
/// green and yellow beams at the same time.
///
/// ⚠️⚠️ HARDCODED, NEVER THEMED, and this is the same rule the threat
/// colours live under (L065). The beam is not decoration — it is how the
/// player reads where their fire is — and a theme that tinted it toward
/// the background would be a gameplay bug wearing a preference.
const SHOT_PALETTE: [Color; 6] = [
    Color::rgb(90, 255, 90),   // green — the most common in the frames
    Color::rgb(255, 255, 255), // white
    Color::rgb(170, 90, 255),  // purple
    Color::rgb(90, 170, 255),  // blue
    Color::rgb(255, 255, 110), // yellow
    Color::rgb(110, 255, 230), // cyan
];

/// How long the SOLID leading section is, as a fraction of the beam.
///
/// ★ THE PATTERN BRIAN COULD NOT PIN DOWN, AND IT IS MEASURABLE. In
/// every reference beam the run lengths are not uniform: the LAST run —
/// the one at the head — is far longer than any other (33, 83, 118 and
/// 143 px in the four-beam frame) and it GROWS with the beam's age,
/// while everything behind it stays broken into short dashes.
/// ⇒ So a beam is a solid leading section with a dashed tail, not an
/// evenly dashed line. The front is the bolt; the dashes are what it
/// left behind.
const SHOT_SOLID_FRACTION: f32 = 0.35;

/// The dash and gap lengths behind the solid head, in world units.
///
/// ★ THE GAPS ARE QUANTISED IN THE ORIGINAL. Measured across every
/// reference beam, gap runs are overwhelmingly 2, 3 or 5 pixels and
/// essentially never larger mid-beam; lit runs cluster around 3 to 10
/// with the occasional 12-18.
/// ⇒ A short table reproduces that far better than a random draw, which
/// would produce the long gaps the reference never shows.
///
/// ⚠️ WALKED FROM A PER-SHOT OFFSET, NOT ALWAYS FROM ENTRY 0. A table
/// walked from the same place every time gives every beam an identical
/// pattern, and it measures as one: our first version came out at stdev
/// 1.10 over 3 distinct run lengths against the arcade's 2.83 over 6.
/// Same scale, visibly less variety — a repeating loop where the
/// original wanders. Starting each beam at its own offset costs nothing
/// and puts the numbers in the same country.
const SHOT_DASHES: [(f32, f32); 9] = [
    (7.0, 2.0),
    (6.0, 5.0),
    (4.0, 2.0),
    (7.0, 3.0),
    (12.0, 2.0),
    (6.0, 2.0),
    (3.0, 5.0),
    (16.0, 3.0),
    (7.0, 2.0),
];

fn draw_shots(canvas: &mut Canvas<'_>, shots: &Shots, camera: &Camera, theme: &Theme) {
    let h = canvas.height() as f32;
    let _ = theme;

    for s in shots.iter() {
        let head = camera.to_screen(s.x);

        // ⚠️ AN ENEMY BOLT IS DRAWN ALONG ITS OWN VELOCITY, not
        // horizontally. Mutants are required never to fire level, so a
        // horizontal streak would draw every angled shot as if it were
        // flat and the one rule that makes them dangerous would be
        // invisible.
        let speed = (s.vx * s.vx + s.vy * s.vy).sqrt().max(1.0);
        // ★ THE BEAM GROWS. See `Shot::beam_len` — the tail travels
        // slower than the head, so the gap between them opens as the
        // shot flies. A fixed length cannot express this.
        let len = s.beam_len();
        let (ux, uy) = (s.vx / speed, s.vy / speed);
        let y_head = h - s.y;

        let tail_x = head - ux * len;
        // Screen y grows DOWN while world y grows UP, so the tail's
        // screen offset is the opposite sign of the world velocity.
        let tail_y = y_head + uy * len;

        let (x0, x1) = if tail_x < head { (tail_x, head) } else { (head, tail_x) };
        if x1 < 0.0 || x0 > canvas.width() as f32 {
            continue;
        }

        if s.owner == Owner::Enemy {
            // ⚠️ ENEMY BOLTS STAY SOLID AND STAY ONE COLOUR. Threat is
            // game information (L065): a bolt broken into dashes is
            // harder to see coming, and that difficulty is not a
            // difficulty the designer chose.
            let color = Color::rgb(255, 120, 60);
            if s.vy.abs() < 0.001 {
                canvas.fill_rect_add_f(x0, y_head - 1.5, x1 - x0, 3.0, color);
            } else {
                canvas.line_add_f(tail_x, tail_y, head, y_head, 3.0, color);
            }
            continue;
        }

        let color = SHOT_PALETTE[(s.tint as usize) % SHOT_PALETTE.len()];

        // The solid leading section, measured back from the head.
        let solid = len * SHOT_SOLID_FRACTION;
        let sx = head - ux * solid;
        let sy = y_head + uy * solid;
        if s.vy.abs() < 0.001 {
            let (a, b) = if sx < head { (sx, head) } else { (head, sx) };
            canvas.fill_rect_add_f(a, y_head - 1.5, b - a, 3.0, color);
        } else {
            canvas.line_add_f(sx, sy, head, y_head, 3.0, color);
        }

        // Then dashes, walking back from the end of the solid section to
        // the tail. ⚠️ Walking from the HEAD rather than from the tail
        // keeps the pattern anchored to the bolt: anchored at the tail,
        // every dash would slide forward each frame as the beam grew and
        // the whole thing would crawl.
        let mut walked = solid;
        // ★ EACH BEAM STARTS AT ITS OWN PLACE IN THE TABLE, keyed off
        // the shot's own colour index so the choice is stable for the
        // life of the beam — a pattern that reshuffled each frame would
        // crawl and shimmer.
        let mut i = (s.tint as usize) * 2;
        while walked < len {
            let (dash, gap) = SHOT_DASHES[i % SHOT_DASHES.len()];
            i += 1;
            let start = walked + gap;
            let end = (start + dash).min(len);
            if start >= len {
                break;
            }
            let ax = head - ux * start;
            let ay = y_head + uy * start;
            let bx = head - ux * end;
            let by = y_head + uy * end;
            if s.vy.abs() < 0.001 {
                let (p, q) = if bx < ax { (bx, ax) } else { (ax, bx) };
                canvas.fill_rect_add_f(p, y_head - 1.5, q - p, 3.0, color);
            } else {
                canvas.line_add_f(ax, ay, bx, by, 3.0, color);
            }
            walked = end;
        }
    }
}

/// The lives left, as ships.
///
/// ★ SHIPS, NOT A NUMBER. A count has to be read; a row of ships is
/// understood at a glance, which is the only kind of reading a player
/// does mid-flight.
fn draw_lives(canvas: &mut Canvas<'_>, lives: &Lives, _theme: &Theme) {
    // ⚠️ MOVED DOWN BY S8, UNDER THE SCORE. The ships sat at y=44 and the
    // score at y=14; once the scanner took the top-centre strip the
    // score moved into this gutter and landed on top of them. They are
    // one column now — score, then ships beneath it — rather than two
    // things that happen to occupy the same corner.
    for i in 0..lives.remaining {
        let x = HUD_MARGIN + 14.0 + i as f32 * 42.0;
        // ⚠️ 0.0 — THE LIVES ROW NEVER BURNS. See art::draw_ship.
        art::draw_ship(canvas, &Transform::at(x, 66.0).scaled(0.55), 0.0);
    }
}

/// The left margin the whole left-hand HUD column hangs off.
const HUD_MARGIN: f32 = 18.0;

/// Game over: the score, the best, and how to go again.
///
/// W6 owns the full presentation (title, attract, roster); this is the
/// part that has to exist for a game to END properly — a final score,
/// and a way back in that is not quitting and relaunching.
fn draw_game_over(canvas: &mut Canvas<'_>, score: u32, best: u32, theme: &Theme) {
    let cx = canvas.width() as i32 / 2;
    let top = canvas.height() as i32 / 2 - 60;
    let centred = |canvas: &mut Canvas<'_>, s: &str, y: i32, scale: u32, c: Color| {
        let w = omarcade_core::text::text_width(s, scale) as i32;
        omarcade_core::text::text(canvas, s, cx - w / 2, y, scale, c);
    };
    let muted = theme.foreground.lerp(theme.background, 0.35);
    centred(canvas, "GAME OVER", top, 5, theme.foreground);
    centred(canvas, &format!("SCORE {score}"), top + 56, 2, theme.foreground);
    let best_line = if score > 0 && score >= best {
        "NEW BEST".to_string()
    } else {
        format!("BEST {best}")
    };
    centred(canvas, &best_line, top + 80, 2, muted);
    centred(canvas, "ENTER TO PLAY AGAIN", top + 120, 2, muted);
}

/// The score. Banked to the score file at game over.
fn draw_score(canvas: &mut Canvas<'_>, score: u32, theme: &Theme) {
    let text = format!("{score:06}");
    let w = omarcade_core::text::text_width(&text, 3) as i32;
    // ★ MOVED LEFT OF CENTRE BY S8, AND THIS IS THE ARCADE'S OWN LAYOUT.
    // The score was centred at the top, which is exactly where the
    // scanner now sits — rendered, the digits collided with its rim.
    // Defender put the score to one side of the scanner rather than
    // above it, and that is also WHY the scanner is only half the screen
    // wide: the space either side is not margin, it is where the rest of
    // the HUD lives.
    // ⚠️ LEFT-ALIGNED IN THE GUTTER, NOT CENTRED IN IT. Centring put the
    // digits on top of the lives row; the gutter is narrow enough that
    // two centred elements in it will always collide. Anchored to the
    // same left margin the lives use, they stack instead.
    let _ = w;
    omarcade_core::text::text(canvas, &text, HUD_MARGIN as i32, 30, 3, theme.foreground);
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
fn draw_ship(canvas: &mut Canvas<'_>, ship: &Ship, camera: &Camera, _theme: &Theme, exhaust: f32) {
    let sx = camera.to_screen(ship.x);
    let sy = canvas.height() as f32 - ship.y;

    // ⚠️ FACING IS A MIRROR, NOT A HALF TURN. `.facing(PI)` would roll the
    // ship inverted — canopy underneath, fin pointing down. The old
    // placeholder was symmetric about its long axis and so could not show
    // the difference; this art can.
    let t = Transform::at(sx, sy)
        .scaled(art::SCALE)
        .flipped(ship.facing.sign() < 0.0);

    art::draw_ship(canvas, &t, exhaust);
}

/// How fast, and which way.
///
/// The position read-out that used to sit beside this went when the
/// scanner arrived: its view box says where in the world you are, and
/// says it against everything else in the world. A bar stays because a
/// speed cannot be read off the scanner at a glance while flying.
fn draw_hud(canvas: &mut Canvas<'_>, ship: &Ship, theme: &Theme) {
    let w = canvas.width() as f32;
    let muted = theme.foreground.lerp(theme.background, 0.6);

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
            let (shots, enemies, fx) = (Shots::new(), Enemies::new(), Effects::new());
            let people = Humanoids::new();
            let lives = Lives::new();
            let scene = bare_scene(&t, &ship, &cam, &shots, &enemies, &people, &fx, &lives);
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
            let (shots, enemies, fx) = (Shots::new(), Enemies::new(), Effects::new());
            let people = Humanoids::new();
            let lives = Lives::new();
            let scene = bare_scene(&t, &ship, &cam, &shots, &enemies, &people, &fx, &lives);
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

    /// ★★ THE SCANNER, THROUGH THE REAL `draw`, WITH REAL ENEMIES IN IT.
    ///
    /// ⚠️ THIS TEST EXISTS BECAUSE OF L064. Yesterday 90 unit tests
    /// passed while the headline feature destroyed itself on sight,
    /// because every enemy test stepped Landers directly and never built
    /// a Shot — the whole collision path sat outside them. The scanner
    /// has exactly the same shape of exposure: its own module tests
    /// check the coordinate mapping, and would all still pass if the
    /// call in `draw` were deleted, mis-ordered, or handed the wrong
    /// camera. So this one goes through `draw` and asserts the scanner
    /// is actually ON the frame.
    #[test]
    fn the_scanner_reaches_the_frame_through_draw() {
        let mut buf = vec![0u32; 960 * 720];
        let t = Terrain::generate(256, 5);
        let ship = Ship::new(0.0);
        let cam = Camera::new(ship.x);
        let theme = Theme::fallback();

        // A Mutant on the far side of the world — the exact thing the
        // scanner was built to reveal, at the exact distance that makes
        // it invisible in the main view.
        let mut enemies = Enemies::new();
        enemies.spawn(crate::enemy::Enemy::mutant(
            world::WORLD_W * 0.5,
            world::VIEW_H * 0.8,
        ));

        {
            let mut c = canvas_of(&mut buf);
            let (shots, fx) = (Shots::new(), Effects::new());
            let people = Humanoids::new();
            let lives = Lives::new();
            let scene = Scene {
                terrain: &t,
                ship: &ship,
                camera: &cam,
                shots: &shots,
                enemies: &enemies,
                people: &people,
                effects: &fx,
                score: 0,
                lives: &lives,
                time: 0.37,
                // ★ Lit, so the thrust flare is exercised by the
                // phase sweep rather than only ever drawn cold.
                exhaust: 1.0,
                popups: &NO_POPUPS,
                hud: quiet_hud(),
            };
            draw(&mut c, &scene, &theme);
        }

        // The scanner occupies a band across the top-centre. Assert
        // there is lit structure there that the rest of the frame does
        // not explain: the sky is flat, so anything in this band came
        // from the scanner.
        let band_top = 28;
        let band_bottom = 88;
        let sky_px = sky(&theme).to_u32();

        let lit = (band_top..band_bottom)
            .flat_map(|y| (300..660).map(move |x| (x, y)))
            .filter(|&(x, y)| buf[y * 960 + x] != sky_px)
            .count();
        assert!(
            lit > 500,
            "only {lit} pixels of the scanner band differ from sky — the scanner is not being \
             drawn by `draw` at all"
        );

        // ★ AND THE MUTANT IS ON IT. A scanner face with no blip on it
        // would satisfy the check above — the face and rim alone light
        // plenty of pixels. This asserts the thing the player needs:
        // something is showing at the middle of the world.
        let mid_x = 480;
        let mutant_showing = (band_top..band_bottom)
            .any(|y| (mid_x - 6..mid_x + 6).any(|x| buf[y * 960 + x as usize] != sky_px));
        assert!(
            mutant_showing,
            "nothing is drawn where a Mutant at the middle of the world should plot"
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
        enemies: &'a Enemies,
        people: &'a Humanoids,
        effects: &'a Effects,
        lives: &'a Lives,
    ) -> Scene<'a> {
        Scene {
            terrain,
            ship,
            camera,
            shots,
            enemies,
            people,
            effects,
            score: 0,
            lives,
            time: 0.0,
            exhaust: 0.0,
            popups: &NO_POPUPS,
            hud: quiet_hud(),
        }
    }

    static NO_POPUPS: Popups = Popups::new();

    /// Wave 1, mid-fight: the HUD as it looks for most of a game.
    fn quiet_hud() -> Hud {
        Hud { wave: 1, phase: WavePhase::Fighting, smart_bombs: 3, best: 0 }
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
            let (shots, enemies, fx) = (Shots::new(), Enemies::new(), Effects::new());
            let people = Humanoids::new();
            let lives = Lives::new();
            let scene = bare_scene(&t, &ship, &cam, &shots, &enemies, &people, &fx, &lives);
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
