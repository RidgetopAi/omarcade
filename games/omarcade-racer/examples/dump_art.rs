//! Render the sprite sheet to a PNG so the art can be judged.
//!
//! The whole point of pixel-art-as-data is a tight correction loop:
//! look at the render, say "the rear wing is one row too high", change
//! two characters in `art.rs`, look again. This is the "look again"
//! half.
//!
//!   cargo run -p omarcade-racer --example dump_art -- out.png [scene]
//!
//! Scenes:
//!   sheet   every sprite at several scales, on a neutral ground
//!   road    the cars sitting on a real pseudo-3D road
//!
//! Never screenshot a window for this — the render is deterministic and
//! the window is not.

#[path = "../src/art.rs"]
mod art;

use std::io::Write;

use art::Art;
use omarcade_core::{Canvas, Color, Theme};

const W: u32 = 960;
const H: u32 = 720;
const HORIZON: u32 = 300;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args.get(1).map(|s| s.as_str()).unwrap_or("art.png");
    let scene = args.get(2).map(|s| s.as_str()).unwrap_or("sheet");

    let theme = Theme::load();
    let art = Art::load(&theme);

    let mut buf = vec![0u32; (W * H) as usize];
    {
        let mut c = Canvas::new(&mut buf, W, H);
        match scene {
            "sheet" => draw_sheet(&mut c, &art, &theme),
            "road" => draw_road(&mut c, &art, &theme),
            other => {
                eprintln!("unknown scene {other:?} — try: sheet | road");
                std::process::exit(2);
            }
        }
    }

    match write_png(out, &buf, W, H) {
        Ok(()) => println!("wrote {out} ({scene})"),
        Err(e) => {
            eprintln!("dump_art: {e}");
            std::process::exit(1);
        }
    }
}

/// Every sprite, at the range of scales the game will actually use.
fn draw_sheet(c: &mut Canvas<'_>, art: &Art, theme: &Theme) {
    c.clear(theme.darker_background);

    // A ground band so the cars are not floating in void.
    c.fill_rect(0, 250, W, H - 250, theme.background);
    c.fill_rect(0, 248, W, 2, theme.dark_foreground);

    // The player's car at increasing scale, left to right — this is the
    // view that shows whether growth is smooth or steppy.
    let scales = [1.2, 1.8, 2.6, 3.6, 5.0];
    let mut x = 90.0;
    for s in scales {
        art.player.draw_ground(c, x, 430.0, s);
        x += 34.0 * s + 26.0;
    }

    // Every rival livery underneath at one size, so the field can be
    // judged as a SET: they have to be distinguishable from each other
    // and all duller than the player above them.
    let mut x = 90.0;
    for (i, _) in art.rivals.iter().enumerate() {
        art.rival(i).draw_ground(c, x, 640.0, 2.6);
        x += 34.0 * 2.6 + 26.0;
    }

    // Marker posts at the far right.
    for (i, s) in [2.0, 3.0, 4.5].iter().enumerate() {
        art.post.draw_ground(c, 830.0 + i as f32 * 40.0, 640.0, *s);
    }

    // A big player car top-right: the pixels at full size, for judging
    // the shapes themselves rather than the motion.
    art.player.draw_ground(c, 700.0, 220.0, 5.5);
}

/// The cars on a real road, which is the only view that says whether
/// they read at speed.
///
/// The projection here is the real one, not a sketch. Screen row -> a
/// distance down the track, road width falling as 1/distance. Getting
/// this backwards makes the road narrow toward the camera, which is the
/// single most obvious way a pseudo-3D racer looks wrong.
fn draw_road(c: &mut Canvas<'_>, art: &Art, theme: &Theme) {
    let sky = theme.background.lerp(theme.blue, 0.30);
    let grass_a = theme.background.lerp(theme.green, 0.40);
    let grass_b = theme.background.lerp(theme.green, 0.28);
    let road_a = theme.dark_background.lerp(theme.foreground, 0.15);
    let road_b = theme.dark_background.lerp(theme.foreground, 0.11);
    let rumble_a = theme.red.lerp(Color::WHITE, 0.15);
    let rumble_b = theme.foreground.lerp(Color::WHITE, 0.4);
    let line = theme.foreground.lerp(Color::WHITE, 0.5);

    // Sky, with a gradient so the horizon has somewhere to sit.
    for y in 0..HORIZON {
        let t = y as f32 / HORIZON as f32;
        c.fill_rect(0, y as i32, W, 1, sky.lerp(theme.background, 1.0 - t * 0.7));
    }

    // How far down the track each screen row is.
    //
    // Rows just under the horizon are far away; rows at the bottom are
    // right in front of the camera. Distance therefore falls as the row
    // descends, and road width — being inversely proportional to
    // distance — GROWS toward the bottom of the screen.
    let project = |y: f32| -> (f32, f32, f32) {
        // 0 at the horizon, 1 at the bottom.
        let t = ((y - HORIZON as f32) / (H - HORIZON) as f32).clamp(0.0001, 1.0);
        // Distance to this row. Near the horizon t is tiny, so z is huge.
        let z = 1.0 / t;
        // Road half-width in screen pixels: a fixed world width divided
        // by distance.
        let half = (1150.0 / z).min(W as f32 * 0.62);
        // A bend: the further away, the more it has accumulated, which
        // is what makes a curve read as a curve and not a diagonal.
        let centre = W as f32 / 2.0 + 0.55 * z * z * 0.5;
        (centre, half, z)
    };

    for y in HORIZON..H {
        let (centre, half, z) = project(y as f32);

        // Bands scroll with DISTANCE, not with screen row — that is what
        // makes them bunch up toward the horizon the way real ground does
        // instead of striping the screen evenly.
        let phase = (z * 2.2) as u32 % 2 == 0;

        c.fill_rect(0, y as i32, W, 1, if phase { grass_a } else { grass_b });
        c.fill_rect_f(centre - half, y as f32, half * 2.0, 1.0,
                      if phase { road_a } else { road_b });

        // Rumble strips, red/white alternating, scaled with the road.
        let rumble = (half * 0.13).max(1.0);
        let rc = if phase { rumble_a } else { rumble_b };
        c.fill_rect_f(centre - half, y as f32, rumble, 1.0, rc);
        c.fill_rect_f(centre + half - rumble, y as f32, rumble, 1.0, rc);

        // Dashed centre line, only on alternate bands.
        if phase {
            let lw = (half * 0.035).max(1.0);
            c.fill_rect_f(centre - lw / 2.0, y as f32, lw, 1.0, line);
        }

        // Distance haze.
        let t = (y - HORIZON) as f32 / (H - HORIZON) as f32;
        let a = ((1.0 - t).powf(2.2) * 190.0) as u8;
        if a > 2 {
            c.fill_rect(0, y as i32, W, 1, sky.with_alpha(a));
        }
    }

    // Rivals, far to near, hazed by distance so they belong to the scene.
    for (i, y) in [352.0f32, 396.0, 470.0, 580.0].iter().enumerate() {
        let (centre, half, _) = project(*y);
        // Sprite scale follows road width, so a car always covers the
        // same fraction of the lane.
        let s = half / 105.0;
        let lane = (i as f32 - 1.5) * half * 0.42;
        let t = (*y - HORIZON as f32) / (H - HORIZON) as f32;
        let haze = ((1.0 - t) * 0.7).clamp(0.0, 1.0);
        // A different livery per slot — the point of the traffic being
        // a set rather than one sprite.
        let rival = art.rival(i);
        let w = rival.width() as f32 * s;
        let h = rival.height() as f32 * s;
        rival.draw_tinted(c, centre + lane - w / 2.0, *y - h, s, Some((sky, haze)));
    }

    // Roadside posts down both edges, spaced by distance so they stream
    // past rather than sitting in an even ladder.
    for i in 1..14 {
        let z = i as f32 * 0.85;
        let t = 1.0 / z;
        if t > 1.0 { continue; }
        let y = HORIZON as f32 + t * (H - HORIZON) as f32;
        let (centre, half, _) = project(y);
        let s = (half / 105.0) * 1.5;
        art.post.draw_ground(c, centre - half * 1.28, y, s);
        art.post.draw_ground(c, centre + half * 1.28, y, s);
    }

    // The player, big and low, where the camera actually sits.
    art.player.draw_ground(c, W as f32 / 2.0, 706.0, 7.0);
}

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
