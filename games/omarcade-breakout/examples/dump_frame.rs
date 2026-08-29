//! Render a GameState to a PNG with no window, no compositor.
//! Possible because Canvas wraps any &mut [u32].
#[path = "../src/geom.rs"]
mod geom;
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
    let out = std::env::args().nth(1).unwrap_or_else(|| "/tmp/frame.png".into());
    let (w, h) = (FIELD_W as u32, FIELD_H as u32);
    let theme = Theme::load();
    let s = GameState::new();

    let mut buf = vec![0u32; (w * h) as usize];
    let mut c = Canvas::new(&mut buf, w, h);

    // Provisional rendering, inline: render.rs is file 4.
    c.clear(theme.background);
    let palette = [theme.red, theme.orange, theme.yellow, theme.green, theme.cyan, theme.blue];
    for b in &s.bricks {
        if !b.alive { continue; }
        c.fill_rect(b.rect.x as i32, b.rect.y as i32, b.rect.w as u32, b.rect.h as u32,
                    palette[b.color_index % palette.len()]);
    }
    let p = s.paddle.rect();
    c.fill_rect(p.x as i32, p.y as i32, p.w as u32, p.h as u32, theme.foreground);
    let ball = s.ball.rect();
    c.fill_rect(ball.x as i32, ball.y as i32, ball.w as u32, ball.h as u32, theme.accent);
    // field border, so we can see the play area bounds
    c.fill_rect(0, 0, w, 2, theme.muted);
    c.fill_rect(0, (h - 2) as i32, w, 2, theme.muted);
    c.fill_rect(0, 0, 2, h, theme.muted);
    c.fill_rect((w - 2) as i32, 0, 2, h, theme.muted);

    write_png(&out, w, h, &buf).expect("write png");
    println!("wrote {out} ({w}x{h}), {} bricks", s.bricks_remaining());
    let _ = Color::BLACK;
}
