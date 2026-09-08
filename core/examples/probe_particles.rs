//! Look at the primitives. Tests prove they are correct; only a picture
//! proves they look like light.
//!
//!   cargo run -p omarcade-core --example probe_particles -- /tmp/fx.png
//!
//! Four panels, left to right:
//!   1. additive vs alpha over a mid background — the pair that shows why
//!      additive exists at all
//!   2. saturation — overlapping bright squares climbing to white, never
//!      wrapping to a dark hole
//!   3. a shatter burst frozen a third of the way through its life
//!   4. the same burst late, showing the fade and the gravity arc

use omarcade_core::geom::Vec2;
use omarcade_core::particles::{Particle, ParticlePool};
use omarcade_core::{Canvas, Color};

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

/// Deterministic pseudo-random in 0..1. A real effect will use the game's
/// own source; this only has to be repeatable so the picture is stable.
fn rnd(seed: &mut u32) -> f32 {
    *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    ((*seed >> 8) & 0xFFFF) as f32 / 65535.0
}

/// A brick-shatter burst: chips in the brick's own colour, thrown along the
/// ball's direction of travel, pulled down by gravity.
fn shatter(pool: &mut ParticlePool, at: Vec2, dir: Vec2, color: Color, seed: &mut u32) {
    for _ in 0..24 {
        let spread = (rnd(seed) - 0.5) * 1.6;
        let speed = 40.0 + rnd(seed) * 90.0;
        let vel = Vec2::new(
            (dir.x + spread) * speed,
            (dir.y + (rnd(seed) - 0.5) * 0.8) * speed,
        );
        let size = 2.0 + rnd(seed) * 3.0;
        let life = 0.5 + rnd(seed) * 0.6;
        pool.spawn(Particle::new(at, vel, size, color, life));
    }
}

fn main() {
    let out = std::env::args().nth(2).unwrap_or_else(|| "/tmp/fx.png".into());
    let (w, h) = (960u32, 260u32);
    let mut buf = vec![0u32; (w * h) as usize];

    // A mid-dark ground, the sort of thing an Omarchy theme actually gives.
    let bg = Color::rgb(28, 32, 40);
    let panel_w = w / 4;

    {
        let mut c = Canvas::new(&mut buf, w, h);
        c.clear(bg);

        // --- 1. additive vs alpha, same source colour, same alpha --------
        let src = Color::rgb(90, 140, 200).with_alpha(170);
        let base = Color::rgb(150, 150, 150);
        c.fill_rect(30, 40, 120, 60, base);
        c.fill_rect(30, 40, 120, 60, src);
        c.fill_rect(30, 140, 120, 60, base);
        c.fill_rect_add(30, 140, 120, 60, src);

        // --- 2. saturation: overlapping bright squares -------------------
        let x0 = panel_w as i32 + 30;
        let hot = Color::rgb(120, 90, 40);
        for i in 0..5 {
            let off = i * 22;
            c.fill_rect_add(x0 + off, 60 + off / 2, 70, 70, hot);
        }

        // --- 3 and 4. a shatter burst, early and late --------------------
        let mut seed = 0x5EED_u32;
        let brick = Color::rgb(190, 120, 70);

        for (panel, elapsed) in [(2u32, 0.22f32), (3, 0.62)] {
            let mut pool = ParticlePool::with_capacity(256).with_gravity(260.0);
            let origin = Vec2::new(
                (panel * panel_w + panel_w / 2) as f32,
                90.0,
            );
            let mut s = seed;
            shatter(&mut pool, origin, Vec2::new(0.35, -0.9), brick, &mut s);

            // Step it in fixed slices, the way a frame loop would.
            let mut t = 0.0;
            while t < elapsed {
                pool.update(1.0 / 240.0);
                t += 1.0 / 240.0;
            }
            pool.draw(&mut c);
            seed = seed.wrapping_add(1);
        }
    }

    write_png(&out, w, h, &buf).expect("write png");
    println!("wrote {out}");
    println!();
    println!("panel 1  top: fill_rect (alpha) over grey — mixes toward the source, DIMS");
    println!("         bottom: fill_rect_add over grey — adds light, BRIGHTENS");
    println!("panel 2  five overlapping additive squares — climbs to white and STOPS");
    println!("         (a wrapping add would show dark holes at the overlaps)");
    println!("panel 3  shatter burst at 0.22s — chips thrown up-right, still bright");
    println!("panel 4  the same burst at 0.62s — faded, pulled into an arc by gravity");
}
