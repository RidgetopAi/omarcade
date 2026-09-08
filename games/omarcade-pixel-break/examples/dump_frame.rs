//! Render a GameState to a PNG with no window, no compositor.
//! Possible because Canvas wraps any &mut [u32].
use omarcade_core::geom;
#[path = "../src/physics.rs"]
mod physics;
#[path = "../src/render.rs"]
mod render;
#[path = "../src/items.rs"]
mod items;
#[path = "../src/effects.rs"]
mod effects;
#[path = "../src/state.rs"]
mod state;

use omarcade_core::{Canvas, Color, Theme};
use state::{GameState, FIELD_H, FIELD_W};

/// Minimal PNG writer: no image crate, so no new dependency.
fn write_png(path: &str, w: u32, h: u32, px: &[u32]) -> std::io::Result<()> {
    use std::io::Write;
    fn crc32(data: &[u8]) -> u32 {
        let mut table = [0u32; 256];
        for (i, e) in table.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 { c = if c & 1 != 0 { 0xEDB88320 ^ (c >> 1) } else { c >> 1 }; }
            *e = c;
        }
        let mut c = 0xFFFF_FFFFu32;
        for &b in data { c = table[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8); }
        c ^ 0xFFFF_FFFF
    }
    fn adler32(data: &[u8]) -> u32 {
        let (mut a, mut b) = (1u32, 0u32);
        for &x in data { a = (a + x as u32) % 65521; b = (b + a) % 65521; }
        (b << 16) | a
    }
    fn chunk(out: &mut Vec<u8>, tag: &[u8], body: &[u8]) {
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        let mut full = tag.to_vec(); full.extend_from_slice(body);
        out.extend_from_slice(&full);
        out.extend_from_slice(&crc32(&full).to_be_bytes());
    }
    // raw scanlines, filter byte 0 per row
    let mut raw = Vec::with_capacity((w * h * 3 + h) as usize);
    for y in 0..h {
        raw.push(0);
        for x in 0..w {
            let p = px[(y * w + x) as usize];
            raw.push((p >> 16) as u8); raw.push((p >> 8) as u8); raw.push(p as u8);
        }
    }
    // zlib stored blocks
    let mut z = vec![0x78, 0x01];
    for (i, block) in raw.chunks(65535).enumerate() {
        let last = if (i + 1) * 65535 >= raw.len() { 1u8 } else { 0 };
        z.push(last);
        z.extend_from_slice(&(block.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(block.len() as u16)).to_le_bytes());
        z.extend_from_slice(block);
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit RGB
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    std::fs::File::create(path)?.write_all(&out)
}

fn main() {
    let scene = std::env::args().nth(1).unwrap_or_else(|| "ready".into());
    let out = std::env::args().nth(2).unwrap_or_else(|| "/tmp/frame.png".into());
    let w: u32 = std::env::args().nth(3).and_then(|v| v.parse().ok()).unwrap_or(FIELD_W as u32);
    let h: u32 = std::env::args().nth(4).and_then(|v| v.parse().ok()).unwrap_or(FIELD_H as u32);

    let theme = Theme::load();
    let mut s = GameState::new();
    // ⚠️ This harness runs its whole simulation and renders ONCE at the
    // end, so every effect fires before render's per-frame palette refresh
    // ever happens. Without this the cascade draws in grey.
    s.set_palette(render::palette(&theme));

    // These scenes drive step_fixed directly, which is the simulation and
    // nothing else. The trail is sampled once per FRAME by physics::step,
    // so a scene built this way has an empty one. Advance a few frames'
    // worth to populate it, exactly as the running game would.
    fn fill_trail(s: &mut state::GameState) {
        let mut acc = physics::Accumulator::new();
        for _ in 0..state::TRAIL_LEN {
            physics::step(s, &mut acc, 1.0 / 60.0);
        }
    }

    // Build the requested situation directly — no need to play to it.
    match scene.as_str() {
        "ready" => {}
        "playing" => {
            s.launch();
            for _ in 0..1500 { physics::step_fixed(&mut s); }
            fill_trail(&mut s);
        }
        // ⚠️ Chips frozen partway through their flight. This is the scene
        // that decides whether the shatter tuning survived the port from
        // tools/shatter-playground.html — the numbers were judged there
        // against real motion, and only a rendered frame proves the game
        // is drawing what the playground drew.
        "shatter" => {
            s.launch();
            // Break a row of bricks with a ball travelling up and right, so
            // the chips fan the way the playground showed them.
            let targets: Vec<usize> = s
                .bricks
                .iter()
                .enumerate()
                .filter(|(_, b)| b.alive())
                .map(|(i, _)| i)
                .skip(22)
                .take(5)
                .collect();
            for (n, i) in targets.iter().enumerate() {
                let r = s.bricks[*i].rect;
                s.balls[0].pos = geom::Vec2::new(r.center().x, r.bottom() + 8.0);
                s.balls[0].vel = geom::Vec2::new(140.0, -300.0);
                physics::step_fixed(&mut s);
                // Stagger them, so the burst is caught at several ages at
                // once — one frame showing early, middle and late chips.
                for _ in 0..(n * 14) {
                    physics::step_fixed(&mut s);
                }
            }
            fill_trail(&mut s);
        }
        // The same armoured break with shake FORCED OFF, so a rendered
        // pair isolates what the shake is actually doing.
        "armoured-noshake" => {
            s.level = 9;
            s.bricks = state::build_bricks(9);
            s.launch();
            let i = s
                .bricks
                .iter()
                .position(|b| b.alive() && b.tier == state::Tier::Armoured)
                .unwrap_or(0);
            s.bricks[i].hits = 1;
            let r = s.bricks[i].rect;
            s.balls[0].pos = geom::Vec2::new(r.center().x, r.bottom() + 8.0);
            s.balls[0].vel = geom::Vec2::new(-120.0, -320.0);
            for _ in 0..10 {
                physics::step_fixed(&mut s);
            }
            s.shake.clear();
            fill_trail(&mut s);
        }
        // An armoured brick breaking: the case that also shakes.
        "armoured" => {
            s.level = 9;
            s.bricks = state::build_bricks(9);
            s.launch();
            let i = s
                .bricks
                .iter()
                .position(|b| b.alive() && b.tier == state::Tier::Armoured)
                .unwrap_or(0);
            s.bricks[i].hits = 1;
            let r = s.bricks[i].rect;
            s.balls[0].pos = geom::Vec2::new(r.center().x, r.bottom() + 8.0);
            s.balls[0].vel = geom::Vec2::new(-120.0, -320.0);
            for _ in 0..10 {
                physics::step_fixed(&mut s);
            }
            fill_trail(&mut s);
        }
        // The level-clear cascade, caught at three points. ⚠️ These are
        // the frames that prove the wave is not invisible: the field is
        // ALREADY EMPTY when it fires, so the effect has to come from the
        // field's geometry rather than from live bricks.
        "cascade-early" | "cascade-late" | "cascade-build" => {
            s.launch();
            for b in &mut s.bricks {
                b.hits = 0;
            }
            physics::step_fixed(&mut s);
            let secs = match scene.as_str() {
                "cascade-early" => state::CLEAR_WAVE_SECONDS * 0.45,
                "cascade-late" => state::CLEAR_WAVE_SECONDS * 0.98,
                _ => state::CLEAR_WAVE_SECONDS + state::CLEAR_BUILD_SECONDS * 0.55,
            };
            for _ in 0..(secs / physics::FIXED_DT) as u32 {
                physics::step_fixed(&mut s);
            }
        }
        // The same ball at level 1 and level 10, so the speed-reactive
        // trail can be compared rather than admired.
        "trail-slow" | "trail-fast" => {
            s.level = if scene == "trail-slow" { 1 } else { state::LEVELS };
            // ⚠️ Keep ONE brick alive. Clearing them all makes check_win
            // fire on the first tick, and the scene cascades to a win
            // instead of showing a ball with a trail.
            s.bricks = state::build_bricks(s.level);
            let keep = s.bricks.len() - 1;
            for b in s.bricks.iter_mut().take(keep) {
                b.hits = 0;
            }
            s.launch();
            s.balls[0].pos = geom::Vec2::new(200.0, 420.0);
            s.balls[0].vel = geom::Vec2::new(0.62, -0.42).with_length(s.ball_speed());
            // Sample per FRAME, the way the real loop does — the trail's
            // whole character depends on that spacing.
            for _ in 0..40 {
                physics::step(&mut s, &mut physics::Accumulator::default(), 1.0 / 60.0);
            }
        }
        "midgame" => {
            s.launch();
            for _ in 0..40_000 {
                let t = s.balls[0].pos.x; let c = s.paddle.center_x();
                s.paddle.dir = if (t - c).abs() < 4.0 { 0.0 } else if t > c { 1.0 } else { -1.0 };
                physics::step_fixed(&mut s);
                if s.phase == state::Phase::Ready { s.launch(); }
            }
            fill_trail(&mut s);
        }
        // Several balls in play at once, each with its own trail.
        // ⚠️ This is the scene that shows the per-ball trail is right: a
        // shared trail would draw one line whipping between the balls.
        "multiball" => {
            s.launch();
            s.level = 6; // the high cap
            let mut seed = 11u32;
            while s.balls.len() < 6 {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                // ⚠️ Mask to 16 bits before dividing. Without the mask the
                // shift keeps every upper bit and `r` runs to millions, so
                // balls spawn tens of thousands of units off-field and drain
                // on the first tick.
                let r = ((seed >> 8) & 0xFFFF) as f32 / 65535.0;
                s.spawn_ball(
                    geom::Vec2::new(160.0 + r * 620.0, 300.0 + r * 200.0),
                    geom::Vec2::new(r * 300.0 - 150.0, -state::BALL_SPEED),
                );
            }
            // Long enough for every ball to build a trail and diverge,
            // short enough that they are all still in play.
            for _ in 0..150 {
                physics::step_fixed(&mut s);
            }
            fill_trail(&mut s);
        }
        // Every tier at every damage stage, so the "hits are readable from
        // the SHAPE" claim can actually be looked at.
        "tiers" => {
            s.level = 10;
            s.bricks = state::build_bricks(10);
            // Row 0-2 are armoured, 3-5 reinforced at L10. Walk each row's
            // columns through progressively more damage.
            for (i, b) in s.bricks.iter_mut().enumerate() {
                let col = i % state::BRICK_COLS;
                let max = b.tier.hits();
                // Column n has taken n hits, clamped to one short of death.
                let taken = (col as u32).min(max - 1);
                b.hits = max - taken;
            }
            s.phase = state::Phase::Playing;
        }
        // Items falling beside balls, which is the comparison that matters:
        // ⚠️ an uncaught item must never be mistaken for a ball.
        "items" => {
            s.launch();
            s.level = 6;
            // Two balls, so there is something to confuse them with.
            s.spawn_ball(geom::Vec2::new(300.0, 380.0), geom::Vec2::new(120.0, -260.0));
            for _ in 0..90 {
                physics::step_fixed(&mut s);
            }
            fill_trail(&mut s);
            // A spread of items at different heights, ALL FOUR kinds.
            //
            // ⚠️ The magnet and the Omarchy item are here so they can be
            // LOOKED AT beside the two that already shipped. S5 killed two
            // colour choices that read fine in code and failed on screen,
            // and this scene is where that was caught.
            let kinds = [
                items::ItemKind::Grow(items::Strength::Small),
                items::ItemKind::Bomb,
                items::ItemKind::Magnet,
                items::ItemKind::Omarchy,
                items::ItemKind::Grow(items::Strength::Medium),
            ];
            for (i, kind) in kinds.into_iter().enumerate() {
                s.items.push(items::Item::new(
                    geom::Vec2::new(150.0 + i as f32 * 150.0, 300.0 + i as f32 * 70.0),
                    kind,
                ));
            }
            // And show a grown paddle, since that is what catching one does.
            s.apply_item(items::ItemKind::Grow(items::Strength::Large));
        }
        // What the player sees the moment a level is cleared. The bug this
        // scene exists for: before it, this frame said "PRESS SPACE" and was
        // indistinguishable from losing a ball.
        "advanced" => {
            s.launch();
            s.score = 1240;
            for b in &mut s.bricks { b.hits = 0; }
            s.advance_level();
        }
        "won" => { for b in &mut s.bricks { b.hits = 0; } s.phase = state::Phase::Won; s.score = 600; s.best = 600; }
        "lost" => { s.lives = 0; s.phase = state::Phase::Lost; s.score = 250; s.best = 980; }
        other => { eprintln!("unknown scene {other}"); std::process::exit(2); }
    }

    let mut buf = vec![0u32; (w * h) as usize];
    {
        let mut c = Canvas::new(&mut buf, w, h);
        render::draw(&mut s, &mut c, &theme);
    }
    write_png(&out, w, h, &buf).expect("write png");
    println!("{out}: scene={scene} {w}x{h} phase={:?} bricks={} score={} lives={}",
        s.phase, s.bricks_remaining(), s.score, s.lives);
    let _ = Color::BLACK;
}
