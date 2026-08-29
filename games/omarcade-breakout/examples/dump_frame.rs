//! Render a GameState to a PNG with no window, no compositor.
//! Possible because Canvas wraps any &mut [u32].
#[path = "../src/geom.rs"]
mod geom;
#[path = "../src/physics.rs"]
mod physics;
#[path = "../src/render.rs"]
mod render;
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
        "midgame" => {
            s.launch();
            for _ in 0..40_000 {
                let t = s.ball.pos.x; let c = s.paddle.center_x();
                s.paddle.dir = if (t - c).abs() < 4.0 { 0.0 } else if t > c { 1.0 } else { -1.0 };
                physics::step_fixed(&mut s);
                if s.phase == state::Phase::Ready { s.launch(); }
            }
            fill_trail(&mut s);
        }
        "won" => { for b in &mut s.bricks { b.alive = false; } s.phase = state::Phase::Won; s.score = 600; s.best = 600; }
        "lost" => { s.lives = 0; s.phase = state::Phase::Lost; s.score = 250; s.best = 980; }
        other => { eprintln!("unknown scene {other}"); std::process::exit(2); }
    }

    let mut buf = vec![0u32; (w * h) as usize];
    {
        let mut c = Canvas::new(&mut buf, w, h);
        render::draw(&s, &mut c, &theme);
    }
    write_png(&out, w, h, &buf).expect("write png");
    println!("{out}: scene={scene} {w}x{h} phase={:?} bricks={} score={} lives={}",
        s.phase, s.bricks_remaining(), s.score, s.lives);
    let _ = Color::BLACK;
}
