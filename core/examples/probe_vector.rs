//! Look at the vector primitives. Tests prove the coverage maths; only a
//! picture proves a ship does not look like a hexagon.
//!
//!   cargo run --release -p omarcade-core --example probe_vector -- /tmp/vec.png
//!
//! Panels are aimed at the things most likely to be wrong, not at the
//! things easiest to draw:
//!   1. SMALL CIRCLES — radius 2 to 10. The faceting a polygon
//!      approximation would show is only visible when the circle is small.
//!   2. THE SEAM — two polygons sharing an edge exactly. Each contributes
//!      half coverage there, so a hairline of background can show through.
//!      This is the classic anti-aliasing artefact and the one most likely
//!      to bite a ship built from several coloured pieces.
//!   3. ROTATION — one shape at twenty-four angles. A shape that gains or
//!      loses weight as it turns makes a flying ship pulse.
//!   4. CONCAVE + STROKE — a chevron and a star, filled and outlined, plus
//!      thin lines fanned through every angle.
//!   5. ADDITIVE — a vector glow over a mid ground, the language the
//!      particle system already speaks.

use omarcade_core::backend::vector::{Shape, Transform};
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


const W: u32 = 1000;
const H: u32 = 560;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "/tmp/vec.png".into());
    let mut px = vec![0u32; (W * H) as usize];
    let mut c = Canvas::new(&mut px, W, H);

    let ink = Color::rgb(235, 238, 245);
    let dim = Color::rgb(120, 130, 150);
    let hot = Color::rgb(255, 170, 60);
    let cool = Color::rgb(90, 200, 255);

    c.clear(Color::rgb(16, 18, 24));

    // ---- 1. small circles, where faceting would show -------------------
    let mut x = 30.0;
    for i in 0..7 {
        let r = 2.0 + i as f32 * 1.5;
        c.circle_f(x + r, 60.0, r, ink);
        x += r * 2.0 + 12.0;
    }
    // The same radii as rings, which is harder: a thin band has two rims.
    let mut x = 30.0;
    for i in 0..7 {
        let r = 2.0 + i as f32 * 1.5;
        c.ring_f(x + r, 110.0, r, 1.5, cool);
        x += r * 2.0 + 12.0;
    }

    // ---- 2. the seam: two shapes sharing an edge exactly ----------------
    // If a hairline shows down the join, a multi-coloured ship will show it
    // too, and that is worth knowing before the art is built.
    let seam_x = 260.0;
    c.polygon_f(&[(seam_x, 30.0), (seam_x + 60.0, 30.0), (seam_x + 60.0, 130.0), (seam_x, 130.0)], hot);
    c.polygon_f(&[(seam_x + 60.0, 30.0), (seam_x + 120.0, 30.0), (seam_x + 120.0, 130.0), (seam_x + 60.0, 130.0)], hot);
    // A diagonal seam, which is the harder case.
    c.polygon_f(&[(seam_x + 140.0, 30.0), (seam_x + 230.0, 30.0), (seam_x + 140.0, 130.0)], cool);
    c.polygon_f(&[(seam_x + 230.0, 30.0), (seam_x + 230.0, 130.0), (seam_x + 140.0, 130.0)], cool);

    // ---- 3. rotation: does the shape keep its weight? -------------------
    const SHIP: Shape = Shape::new(&[
        (14.0, 0.0), (-6.0, -7.0), (-2.0, 0.0), (-6.0, 7.0),
    ]);
    for i in 0..24 {
        let a = i as f32 * std::f32::consts::TAU / 24.0;
        let (cx, cy) = (110.0 + (i % 12) as f32 * 70.0, 210.0 + (i / 12) as f32 * 70.0);
        SHIP.fill(&mut c, &Transform::at(cx, cy).facing(a).scaled(1.6), ink);
    }

    // ---- 4. concave, stroked, and thin lines at every angle -------------
    const CHEVRON: Shape = Shape::new(&[
        (0.0, -26.0), (26.0, 26.0), (0.0, 10.0), (-26.0, 26.0),
    ]);
    CHEVRON.fill(&mut c, &Transform::at(90.0, 400.0), hot);
    CHEVRON.stroke(&mut c, &Transform::at(200.0, 400.0), 2.0, hot);

    // A five-pointed star: concave, self-crossing in the naive winding, and
    // the shape most likely to expose a fill-rule mistake.
    let mut star = [(0.0f32, 0.0f32); 10];
    for (i, p) in star.iter_mut().enumerate() {
        let r = if i % 2 == 0 { 34.0 } else { 14.0 };
        let a = i as f32 * std::f32::consts::TAU / 10.0 - std::f32::consts::FRAC_PI_2;
        *p = (a.cos() * r, a.sin() * r);
    }
    let star = Shape::new(&star);
    star.fill(&mut c, &Transform::at(320.0, 400.0), cool);
    star.stroke(&mut c, &Transform::at(430.0, 400.0), 1.5, cool);

    // Thin lines fanned through a half turn: the test of whether a line at
    // an awkward angle stays a consistent weight.
    for i in 0..16 {
        let a = i as f32 * std::f32::consts::PI / 16.0;
        let (dx, dy) = (a.cos() * 46.0, a.sin() * 46.0);
        c.line_f(560.0 - dx, 400.0 - dy, 560.0 + dx, 400.0 + dy, 1.0, dim);
    }
    // The same fan at 3px, to compare weight consistency across widths.
    for i in 0..16 {
        let a = i as f32 * std::f32::consts::PI / 16.0;
        let (dx, dy) = (a.cos() * 46.0, a.sin() * 46.0);
        c.line_f(680.0 - dx, 400.0 - dy, 680.0 + dx, 400.0 + dy, 3.0, dim);
    }

    // ---- 5. additive: vector art speaking the particle language ---------
    c.fill_rect(770, 340, 200, 120, Color::rgb(40, 44, 58));
    for i in 0..5 {
        let r = 46.0 - i as f32 * 9.0;
        c.circle_add_f(840.0, 400.0, r, Color::rgb(30, 22, 10));
    }
    SHIP.fill_add(&mut c, &Transform::at(920.0, 400.0).scaled(2.6), Color::rgb(70, 110, 150));

    // A label strip so the panels are identifiable at a glance.
    for (i, _) in (0..5).enumerate() {
        c.fill_rect(8 + i as i32 * 4, 8, 2, 2, dim);
    }

    write_png(&path, W, H, &px).expect("write png");
    println!("wrote {path}");
}
