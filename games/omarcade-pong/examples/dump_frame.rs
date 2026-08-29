//! Render any game state straight to a PNG, with no window involved.
//!
//! This is how the README screenshot is generated — never by
//! screenshotting a live window, which depends on the compositor, the
//! active theme, and whatever the game happened to be doing.
//!
//!   cargo run -p omarcade-pong --example dump_frame -- <scene> <out.png> [w] [h]
//!
//! Scenes: select | serve | rally | matchpoint | won | lost
//!
//! Writes a minimal uncompressed PNG (stored-mode deflate) so the crate
//! stays dependency-free. Bigger than a real encoder's output and
//! perfectly valid.

#[path = "../src/ai.rs"]
mod ai;
#[path = "../src/physics.rs"]
mod physics;
#[path = "../src/render.rs"]
mod render;
#[path = "../src/state.rs"]
mod state;

use std::io::Write;

use omarcade_core::{Canvas, Theme};
use physics::{serve, step_fixed};
use state::{Difficulty, GameState, Phase, Side};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: dump_frame <scene> <out.png> [w] [h]");
        eprintln!("scenes: select | serve | rally | matchpoint | won | lost");
        std::process::exit(2);
    }

    let scene = args[1].as_str();
    let out = &args[2];
    let w: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(960);
    let h: u32 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(720);

    let state = build(scene);

    let mut buf = vec![0u32; (w * h) as usize];
    {
        let mut canvas = Canvas::new(&mut buf, w, h);
        render::draw(&state, &mut canvas, &Theme::load());
    }

    match write_png(out, &buf, w, h) {
        Ok(()) => println!("wrote {out} ({w}x{h}, scene: {scene})"),
        Err(e) => {
            eprintln!("dump_frame: {e}");
            std::process::exit(1);
        }
    }
}

/// Build a state directly, rather than playing until it happens.
fn build(scene: &str) -> GameState {
    let mut s = GameState::with_difficulty(Difficulty::Normal);

    match scene {
        "select" => return s,

        "serve" => {
            s.begin();
            s.score_left = 3;
            s.score_right = 2;
        }

        // Mid-rally, which is the interesting frame: ball in flight
        // with a trail, paddles committed, rally counter showing.
        "rally" => {
            s.begin();
            s.score_left = 4;
            s.score_right = 6;
            serve(&mut s);
            // Let it play a while so the paddles are somewhere other
            // than centred and a real exchange is under way.
            let mut opponent = ai::Opponent::new(Side::Right, s.difficulty);
            for _ in 0..900 {
                // A simple tracker on the player side.
                let target = s.ball.pos.y;
                let p = s.paddle_mut(Side::Left);
                let delta = target - p.center_y();
                p.dir = if delta.abs() < 6.0 {
                    0.0
                } else if delta > 0.0 {
                    1.0
                } else {
                    -1.0
                };
                opponent.update(&mut s, physics::FIXED_DT);
                step_fixed(&mut s);
                if s.phase == Phase::Serve {
                    serve(&mut s);
                }
            }
            s.rally = s.rally.max(9);

            // The trail is sampled once per FRAME by physics::step, not
            // per fixed tick — and this harness drives step_fixed
            // directly, so nothing has populated it. Sample it here at
            // the same cadence the game would, or the ball renders as a
            // lone square with no sense of motion.
            s.trail.clear();
            for _ in 0..state::TRAIL_LEN {
                for _ in 0..4 {
                    step_fixed(&mut s);
                }
                // NEWEST FIRST, matching physics::record_trail. The
                // renderer fades from index 0 outward, so pushing and
                // reversing puts the solid end behind the ball and the
                // faint end at its leading edge — a comet flying
                // backwards.
                s.trail.insert(0, s.ball.pos);
            }
        }

        "matchpoint" => {
            s.begin();
            s.score_left = 10;
            s.score_right = 9;
            serve(&mut s);
            for _ in 0..200 {
                step_fixed(&mut s);
            }
            s.rally = 14;
        }

        "won" => {
            s.begin();
            s.score_left = state::MATCH_POINT;
            s.score_right = 8;
            s.longest_rally = 31;
            s.best = 44;
            s.phase = Phase::Over { winner: Side::Left };
        }

        "lost" => {
            s.begin();
            s.score_left = 6;
            s.score_right = state::MATCH_POINT;
            s.longest_rally = 22;
            s.best = 44;
            s.phase = Phase::Over { winner: Side::Right };
        }

        other => {
            eprintln!("unknown scene {other:?}");
            std::process::exit(2);
        }
    }

    s
}

// ----------------------------------------------------------------------
// A minimal PNG writer.
// ----------------------------------------------------------------------

fn write_png(path: &str, buf: &[u32], w: u32, h: u32) -> std::io::Result<()> {
    let mut raw = Vec::with_capacity(((w * 3 + 1) * h) as usize);
    for y in 0..h {
        raw.push(0); // filter type 0 (None) per scanline
        for x in 0..w {
            let px = buf[(y * w + x) as usize];
            raw.push((px >> 16) as u8);
            raw.push((px >> 8) as u8);
            raw.push(px as u8);
        }
    }

    let mut png = Vec::new();
    png.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit, truecolour RGB
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"IDAT", &zlib_stored(&raw));
    chunk(&mut png, b"IEND", &[]);

    let mut f = std::fs::File::create(path)?;
    f.write_all(&png)
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

/// zlib stream using stored (uncompressed) deflate blocks.
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01]; // CMF/FLG: deflate, 32K window, no dict
    // Stored blocks cap at 65535 bytes each.
    for (i, block) in data.chunks(65535).enumerate() {
        let last = if (i + 1) * 65535 >= data.len() { 1 } else { 0 };
        out.push(last);
        let n = block.len() as u16;
        out.extend_from_slice(&n.to_le_bytes());
        out.extend_from_slice(&(!n).to_le_bytes());
        out.extend_from_slice(block);
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}
