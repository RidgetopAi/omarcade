//! How big is a Lander next to the ship, really?
//!
//!   cargo run -p omarcade-defender --example size_check -- out.png
//!
//! A size decision made from unit counts is a guess. The ship is 35 units
//! long and draws at SCALE 2.0, which is 70px on a 960px screen — but
//! knowing that does not tell you whether a 6-unit Lander reads as an
//! enemy or as a speck. This draws all three at the size the game will
//! actually draw them, against the game's own background, so the
//! question is answered by looking.
//!
//! ⚠️ Temporary. This exists to settle enemy scale before the art moves
//! into src/art.rs; delete it once the sizes are locked.

use std::path::Path;

use omarcade_core::{text::text, Canvas, Color, Theme, Transform};

#[path = "../src/art.rs"]
mod art;

#[path = "../assets-inbox/lander.rs"]
mod lander_small;
#[path = "../assets-inbox/lander_scaled.rs"]
mod lander_big;

#[path = "../assets-inbox/humanoid.rs"]
mod humanoid_small;
#[path = "../assets-inbox/humanoid_scaled.rs"]
mod humanoid_big;

#[path = "../assets-inbox/mutant_scaled.rs"]
mod mutant_big;

const W: u32 = 960;
const H: u32 = 720;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "size_check.png".to_string());
    let theme = Theme::load();

    let mut buf = vec![0u32; (W * H) as usize];
    let mut canvas = Canvas::new(&mut buf, W, H);
    canvas.clear(theme.background);

    let fg = theme.foreground;
    let muted = fg.lerp(theme.background, 0.55);

    // A horizon, so the pieces are read against the same ground the game
    // gives them rather than floating in a void.
    let horizon = (H as f32 * 0.78) as i32;
    canvas.fill_rect(0, horizon, W, 2, muted);

    // ---- the ship, the reference everything else is judged against ----
    let ship_y = H as f32 * 0.30;
    art::draw_ship(&mut canvas, &Transform::at(200.0, ship_y).scaled(art::SCALE));
    label(&mut canvas, "SHIP  35 x 11 units  ->  70 x 22 px", 40, (ship_y + 40.0) as i32, fg);

    // ---- landers: as drawn, and at 2x ----
    let row = H as f32 * 0.52;
    lander_small::draw_unnamed_shape(&mut canvas, &Transform::at(160.0, row).scaled(art::SCALE));
    label(&mut canvas, "as drawn", 118, (row + 46.0) as i32, muted);
    label(&mut canvas, "12x15", 122, (row + 62.0) as i32, muted);

    lander_big::draw_lander(&mut canvas, &Transform::at(300.0, row).scaled(art::SCALE));
    label(&mut canvas, "2x", 288, (row + 46.0) as i32, fg);
    label(&mut canvas, "24x30", 274, (row + 62.0) as i32, fg);

    label(&mut canvas, "LANDER", 40, (row - 60.0) as i32, fg);

    // ---- humanoids: as drawn, and at 2.5x ----
    humanoid_small::draw_unnamed_shape(&mut canvas, &Transform::at(520.0, row).scaled(art::SCALE));
    label(&mut canvas, "as drawn", 478, (row + 46.0) as i32, muted);
    label(&mut canvas, "4x9", 498, (row + 62.0) as i32, muted);

    humanoid_big::draw_humanoid(&mut canvas, &Transform::at(640.0, row).scaled(art::SCALE));
    label(&mut canvas, "2.5x", 622, (row + 46.0) as i32, fg);
    label(&mut canvas, "10x22", 614, (row + 62.0) as i32, fg);

    label(&mut canvas, "HUMANOID", 470, (row - 60.0) as i32, fg);

    // ---- the mutant, which must sit exactly where a lander sits ----
    lander_big::draw_lander(&mut canvas, &Transform::at(790.0, row).scaled(art::SCALE));
    mutant_big::draw_mutant(&mut canvas, &Transform::at(880.0, row).scaled(art::SCALE));
    label(&mut canvas, "MUTANT", 770, (row - 60.0) as i32, fg);
    label(&mut canvas, "vs lander", 762, (row + 46.0) as i32, muted);
    label(&mut canvas, "both 24x30", 758, (row + 62.0) as i32, muted);

    // ---- the real question: all three together, at the proposed sizes ----
    let scene = H as f32 * 0.88;
    label(&mut canvas, "TOGETHER, AT THE PROPOSED SIZES", 40, (scene - 118.0) as i32, fg);
    art::draw_ship(&mut canvas, &Transform::at(180.0, scene - 70.0).scaled(art::SCALE));
    lander_big::draw_lander(&mut canvas, &Transform::at(430.0, scene - 78.0).scaled(art::SCALE));
    lander_big::draw_lander(&mut canvas, &Transform::at(530.0, scene - 62.0).scaled(art::SCALE));
    humanoid_big::draw_humanoid(&mut canvas, &Transform::at(430.0, scene - 12.0).scaled(art::SCALE));
    humanoid_big::draw_humanoid(&mut canvas, &Transform::at(700.0, scene - 12.0).scaled(art::SCALE));
    mutant_big::draw_mutant(&mut canvas, &Transform::at(640.0, scene - 70.0).scaled(art::SCALE));
    mutant_big::draw_mutant(&mut canvas, &Transform::at(760.0, scene - 84.0).scaled(art::SCALE));

    write_png(Path::new(&path), &buf, W, H);
    println!("wrote {path}");
}

fn label(canvas: &mut Canvas<'_>, s: &str, x: i32, y: i32, color: Color) {
    text(canvas, s, x, y, 2, color);
}

/// Minimal PNG writer: stored-mode deflate, no compression.
///
/// The same tradeoff the other games make — it keeps the crate free of
/// an image dependency, at the cost of a ~2 MB file. Fine for a render
/// nobody commits; anything that lives in the repo gets compressed by
/// the tool that generates it.
fn write_png(path: &Path, buf: &[u32], w: u32, h: u32) {
    let mut raw = Vec::with_capacity(((w * 3 + 1) * h) as usize);
    for y in 0..h {
        raw.push(0); // filter: none
        for x in 0..w {
            let px = buf[(y * w + x) as usize];
            raw.push((px >> 16) as u8);
            raw.push((px >> 8) as u8);
            raw.push(px as u8);
        }
    }

    let mut png = Vec::new();
    png.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit RGB
    chunk(&mut png, b"IHDR", &ihdr);

    chunk(&mut png, b"IDAT", &zlib_stored(&raw));
    chunk(&mut png, b"IEND", &[]);

    std::fs::write(path, png).expect("write png");
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

fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    for (i, block) in data.chunks(65_535).enumerate() {
        let last = (i + 1) * 65_535 >= data.len();
        out.push(if last { 1 } else { 0 });
        out.extend_from_slice(&(block.len() as u16).to_le_bytes());
        out.extend_from_slice(&(!(block.len() as u16)).to_le_bytes());
        out.extend_from_slice(block);
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + byte as u32) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}
