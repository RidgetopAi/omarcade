//! What does scaling do to the sky?
//!
//! The backend is about to grow a fixed-resolution mode: the racer keeps
//! rendering 960x720 and the backend scales that up to whatever the
//! window is. The open question is HOW to scale, and it is not a
//! performance question — it is a question about ONE surface.
//!
//! ⚠️ THE SKY IS A PHOTOGRAPH. Everything else the racer draws is flat
//! colour — road, grass, cars, signs — and flat colour survives any
//! resampling. `backdrop.rs` puts the desktop wallpaper behind the
//! horizon at SKY_IMAGE = 0.30, and a photograph is exactly the content
//! where nearest-neighbour stairsteps a smooth gradient and eats the
//! ridgelines the fade direction was tuned to keep.
//!
//! So this renders a REAL frame through the game's own renderer, scales
//! it both ways, and writes the sky band cropped and stacked so the two
//! can be judged side by side rather than argued about.
//!
//!   cargo run --release -p omarcade-racer --example probe_scale -- <outdir> [dw dh]
//!
//! ⚠️ A MEASUREMENT TOOL, NOT SHIPPED CODE. It duplicates the scaler so
//! the real one can be written against evidence; when the backend has
//! its own, this stays as the thing that justified the choice.
//!
//! ⚠️ OMARCADE_BACKGROUND=<path> selects which wallpaper is in the sky,
//! the same as the game. Without a wallpaper the sky is plain and this
//! probe cannot answer anything — it says so rather than pretending.

#[path = "../src/collide.rs"]
mod collide;
#[path = "../src/traffic.rs"]
mod traffic;
#[path = "../src/crash.rs"]
mod crash;
#[path = "../src/art.rs"]
mod art;
#[path = "../src/road.rs"]
mod road;
#[path = "../src/drive.rs"]
mod drive;
#[path = "../src/structures.rs"]
mod structures;
#[path = "../src/track.rs"]
mod track;
#[path = "../src/render.rs"]
mod render;
#[path = "../src/scenery.rs"]
mod scenery;

use std::io::Write;
use std::time::Instant;

use art::Art;
use drive::{Drive, Tuning};
use omarcade_core::{Canvas, Theme};

const W: u32 = 960;
const H: u32 = 720;

// ---------------------------------------------------------------------
// The two candidate scalers.
// ---------------------------------------------------------------------

/// Per-axis sample plan: for each destination index, which source index
/// it starts at and how far between that one and the next it sits, as a
/// 0..=256 fixed-point weight.
///
/// Centre-sampled (`+0.5 ... -0.5`) rather than corner-sampled, so the
/// image does not drift half a source pixel toward the origin as it is
/// scaled up — a corner-sampled ramp puts the whole picture visibly
/// off-centre at large scales.
fn axis(src: usize, dst: usize) -> Vec<(usize, u32)> {
    (0..dst)
        .map(|i| {
            let f = ((i as f64 + 0.5) * src as f64 / dst as f64 - 0.5).max(0.0);
            let i0 = (f.floor() as usize).min(src.saturating_sub(1));
            let w = ((f - i0 as f64) * 256.0).round().clamp(0.0, 256.0) as u32;
            (i0, w)
        })
        .collect()
}

/// Blend two packed 0x00RRGGBB pixels, `w` in 0..=256.
///
/// ⚠️ THE OBVIOUS PACKED-LANE TRICK IS WRONG AND IT LOOKS LIKE CONFETTI.
/// Riding red and blue in one u32 and doing `(q - p) * w >> 8` breaks the
/// moment a channel DECREASES: the subtraction underflows, borrows across
/// into the neighbouring lane, and no amount of masking afterwards undoes
/// a borrow that already happened. A first version did exactly that; the
/// TIMING looked plausible and the pixel-difference statistics looked like
/// a real quality tradeoff, and only rendering the crop showed the output
/// was noise. See L0xx.
///
/// Weighted-sum form instead: `p*(256-w) + q*w >> 8`. Both terms are
/// non-negative, so nothing can borrow, and the lanes stay independent.
#[inline(always)]
fn lerp2(p: u32, q: u32, w: u32) -> u32 {
    let iw = 256 - w;
    let prb = p & 0x00FF_00FF;
    let qrb = q & 0x00FF_00FF;
    let rb = ((prb * iw + qrb * w) >> 8) & 0x00FF_00FF;
    let pg = (p >> 8) & 0xFF;
    let qg = (q >> 8) & 0xFF;
    let g = ((pg * iw + qg * w) >> 8) & 0xFF;
    rb | (g << 8)
}

fn scale_nearest(src: &[u32], sw: usize, sh: usize, dst: &mut [u32], dw: usize, dh: usize) {
    let cols: Vec<usize> = (0..dw).map(|x| x * sw / dw).collect();
    for dy in 0..dh {
        let srow = &src[(dy * sh / dh) * sw..][..sw];
        let drow = &mut dst[dy * dw..][..dw];
        for dx in 0..dw {
            drow[dx] = srow[cols[dx]];
        }
    }
}

fn scale_bilinear(src: &[u32], sw: usize, sh: usize, dst: &mut [u32], dw: usize, dh: usize) {
    let cx = axis(sw, dw);
    let cy = axis(sh, dh);
    for dy in 0..dh {
        let (y0, wy) = cy[dy];
        let y1 = (y0 + 1).min(sh - 1);
        let r0 = &src[y0 * sw..][..sw];
        let r1 = &src[y1 * sw..][..sw];
        let drow = &mut dst[dy * dw..][..dw];
        for dx in 0..dw {
            let (x0, wx) = cx[dx];
            let x1 = (x0 + 1).min(sw - 1);
            let top = lerp2(r0[x0], r0[x1], wx);
            let bot = lerp2(r1[x0], r1[x1], wx);
            drow[dx] = lerp2(top, bot, wy);
        }
    }
}

// ---------------------------------------------------------------------

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = args.get(1).map(|s| s.as_str()).unwrap_or("/tmp");
    let dw: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1920);
    let dh: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1440);

    let theme = Theme::load();
    let art = Art::load(&theme);

    // ⚠️ Say so rather than producing two identical crops and calling it
    // a result: with no wallpaper the sky is flat colour and flat colour
    // scales identically either way. The probe would "pass" meaninglessly.
    match &art.sky {
        Some(_) => println!("  sky: wallpaper loaded — the comparison is meaningful"),
        None => println!(
            "  ⚠️ SKY: NO WALLPAPER. The sky is plain colour, so both scalers will\n     \
             agree and this probe proves NOTHING. Set OMARCADE_BACKGROUND=<path>."
        ),
    }

    // A real frame, through the game's own renderer, on the real course.
    let mut buf = vec![0u32; (W * H) as usize];
    // The same construction main.rs uses, so this is the real course
    // with the real handling rather than a fixture that only resembles it.
    let road = track::grand_prix().build();
    let tuning = Tuning::from_corner(&road, 1.5);
    let mut car = Drive::new();
    car.z = 120.0;
    car.speed = tuning.top_speed * 0.8;
    {
        let mut c = Canvas::new(&mut buf, W, H);
        render::draw_road_into_with(
            &mut c, &art, &theme, &road, &tuning, &car, 0.0, &[], 0, 0, W, H, true,
        );
    }

    let (sw, sh) = (W as usize, H as usize);
    let mut nn = vec![0u32; dw * dh];
    let mut bl = vec![0u32; dw * dh];

    // Time them on the real frame, not on synthetic content.
    let iters = 40;
    scale_nearest(&buf, sw, sh, &mut nn, dw, dh);
    let t = Instant::now();
    for _ in 0..iters {
        scale_nearest(&buf, sw, sh, &mut nn, dw, dh);
    }
    let ms_nn = t.elapsed().as_secs_f64() * 1000.0 / iters as f64;

    scale_bilinear(&buf, sw, sh, &mut bl, dw, dh);
    let t = Instant::now();
    for _ in 0..iters {
        scale_bilinear(&buf, sw, sh, &mut bl, dw, dh);
    }
    let ms_bl = t.elapsed().as_secs_f64() * 1000.0 / iters as f64;

    println!("\n  960x720 -> {dw}x{dh}");
    println!("    nearest   {ms_nn:6.2} ms");
    println!("    bilinear  {ms_bl:6.2} ms   ({:.1}x the cost)", ms_bl / ms_nn);

    // How different are they, really? A count plus the worst channel
    // delta says whether the eye has anything to find.
    let mut differing = 0usize;
    let mut worst = 0u32;
    for i in 0..dw * dh {
        if nn[i] != bl[i] {
            differing += 1;
            for s in [0u32, 8, 16] {
                let d = (((nn[i] >> s) & 0xFF) as i32 - ((bl[i] >> s) & 0xFF) as i32).unsigned_abs();
                worst = worst.max(d);
            }
        }
    }
    println!(
        "    differ on {:.1}% of pixels, worst channel delta {worst}",
        differing as f64 * 100.0 / (dw * dh) as f64
    );

    let _ = write_png(&format!("{dir}/scale-nearest.png"), &nn, dw as u32, dh as u32);
    let _ = write_png(&format!("{dir}/scale-bilinear.png"), &bl, dw as u32, dh as u32);

    // The crop that answers the question: the SKY BAND, nearest above
    // bilinear, split by a red rule. Cropped 1:1 out of each scaled
    // image so what is on screen is what the scaler produced — resizing
    // the comparison would be measuring this tool's own resampling.
    let cw = (dw * 3 / 4).min(1100);
    let ch = (dh / 6).min(200);
    let ox = (dw - cw) / 2;
    // The horizon sits at 300/720 of the frame; sample just above it,
    // where the wallpaper is strongest and the ridgelines live.
    let oy = (dh as f64 * 0.20) as usize;
    const RULE: usize = 6;
    let mut cmp = vec![0u32; cw * (ch * 2 + RULE)];
    for y in 0..ch {
        for x in 0..cw {
            cmp[y * cw + x] = nn[(oy + y) * dw + ox + x];
            cmp[(y + ch + RULE) * cw + x] = bl[(oy + y) * dw + ox + x];
        }
    }
    for y in ch..ch + RULE {
        for x in 0..cw {
            cmp[y * cw + x] = 0x00FF_2020;
        }
    }
    let _ = write_png(&format!("{dir}/scale-sky-compare.png"), &cmp, cw as u32, (ch * 2 + RULE) as u32);

    println!("\n  wrote scale-nearest.png, scale-bilinear.png");
    println!("  wrote scale-sky-compare.png  (NEAREST above the red rule, BILINEAR below)");
}

// ---------------------------------------------------------------------
// PNG out. Lifted from dump_art: stored-deflate, no compression, no
// dependency. Big files, but these are looked at once and deleted.
// ---------------------------------------------------------------------

fn write_png(path: &str, buf: &[u32], w: u32, h: u32) -> std::io::Result<()> {
    let mut raw = Vec::with_capacity(((w * 3 + 1) * h) as usize);
    for y in 0..h {
        raw.push(0);
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
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
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

fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    for (i, block) in data.chunks(65535).enumerate() {
        let last = if (i + 1) * 65535 >= data.len() { 1 } else { 0 };
        out.push(last);
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
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}
