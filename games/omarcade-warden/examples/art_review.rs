//! W3 art review: the Baiter, Bomber and Mine drafts beside the ship,
//! the Lander and the Mutant, at the size the game draws them.
//!
//!   cargo run -p omarcade-warden --example art_review -- out.png
//!
//! ⚠️ Temporary, like size_check: it exists so Brian can approve the
//! drafts before they move into src/art.rs. Delete it with the inbox
//! (same commit — `cargo test` compiles examples).

use std::path::Path;

use omarcade_core::{text::text, Canvas, Color, Theme, Transform};

#[allow(dead_code)]
#[path = "../src/art.rs"]
mod art;
#[path = "../assets-inbox/baiter.rs"]
mod baiter;
#[path = "../assets-inbox/bomber.rs"]
mod bomber;
#[path = "../assets-inbox/mine.rs"]
mod mine;

const W: u32 = 960;
const H: u32 = 720;

type Draw = fn(&mut Canvas<'_>, &Transform);

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "art_review.png".to_string());
    let theme = Theme::load();
    let mut buf = vec![0u32; (W * H) as usize];
    let mut canvas = Canvas::new(&mut buf, W, H);
    // The game's own sky and ground, so the pieces are judged where they live.
    let sky = theme.background.lerp(Color::rgb(0, 0, 0), 0.35);
    canvas.clear(sky);
    let ground = theme.background.lerp(theme.foreground, 0.13);
    canvas.fill_rect(0, 620, W, 100, ground);
    canvas.fill_rect(0, 620, W, 2, theme.foreground.lerp(theme.background, 0.25));
    let fg = theme.foreground;
    let muted = fg.lerp(theme.background, 0.45);

    let ship = |c: &mut Canvas<'_>, t: &Transform| art::draw_ship(c, t, 0.0);
    let pieces: [(&str, &str, Draw); 6] = [
        ("SHIP", "35x11", ship),
        ("LANDER", "12x15", art::draw_lander),
        ("MUTANT", "12x15", art::draw_mutant),
        ("BAITER", "24x10 NEW", baiter::draw_baiter),
        ("BOMBER", "14x10 NEW", bomber::draw_bomber),
        ("MINE", "5x5 NEW", mine::draw_mine),
    ];

    text(&mut canvas, "W3 ART DRAFTS - GAME SIZE", 24, 24, 2, fg);
    let xs = [90.0, 240.0, 360.0, 500.0, 650.0, 800.0];
    for (i, (name, size, draw)) in pieces.iter().enumerate() {
        draw(&mut canvas, &Transform::at(xs[i], 130.0).scaled(art::SCALE));
        let w = omarcade_core::text::text_width(name, 2) as i32;
        text(&mut canvas, name, xs[i] as i32 - w / 2, 175, 2, fg);
        let w = omarcade_core::text::text_width(size, 1) as i32;
        text(&mut canvas, size, xs[i] as i32 - w / 2, 197, 1, muted);
    }

    text(&mut canvas, "4X CLOSE-UP", 24, 250, 2, fg);
    let close: [(f32, Draw); 4] = [
        (150.0, art::draw_lander),
        (380.0, baiter::draw_baiter),
        (620.0, bomber::draw_bomber),
        (820.0, mine::draw_mine),
    ];
    for (x, draw) in close {
        draw(&mut canvas, &Transform::at(x, 400.0).scaled(art::SCALE * 4.0));
    }

    // In context: a scene the way play will look.
    text(&mut canvas, "IN PLAY", 24, 520, 2, fg);
    art::draw_ship(&mut canvas, &Transform::at(140.0, 575.0).scaled(art::SCALE), 0.0);
    baiter::draw_baiter(&mut canvas, &Transform::at(300.0, 545.0).scaled(art::SCALE));
    bomber::draw_bomber(&mut canvas, &Transform::at(520.0, 590.0).scaled(art::SCALE));
    bomber::draw_bomber(&mut canvas, &Transform::at(570.0, 590.0).scaled(art::SCALE));
    for (x, y) in [(470.0, 600.0), (430.0, 585.0), (395.0, 605.0)] {
        mine::draw_mine(&mut canvas, &Transform::at(x, y).scaled(art::SCALE));
    }
    art::draw_lander(&mut canvas, &Transform::at(720.0, 560.0).scaled(art::SCALE));
    art::draw_mutant(&mut canvas, &Transform::at(840.0, 580.0).scaled(art::SCALE));

    write_png(Path::new(&path), &buf, W, H);
    println!("wrote {path}");
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
