//! The active Omarchy wallpaper, downsampled for use behind a game.
//!
//! Omarchy keeps a symlink at `~/.local/state/omarchy/current/background`
//! pointing at whichever image the current theme is showing. Reading it
//! lets a game put the *desktop's own* picture into its scene — the
//! racer blends it faintly into the sky, so the horizon carries a
//! silhouette of wherever the player actually is.
//!
//! # Why this is not simply "load a PNG"
//!
//! Two constraints shape everything here.
//!
//! **It must never be on the frame path.** The images are enormous — the
//! shipped Omarchy wallpapers run to 7680x3215 — and decoding one costs
//! tens to hundreds of milliseconds. So a [`Backdrop`] is built ONCE at
//! start-up, beside the theme read that already happens there, and what
//! it holds is a small pre-scaled buffer, never the source image.
//!
//! **It must never stop a game starting.** A missing symlink, an
//! unreadable file, a format nobody expected, a theme mid-swap: every
//! one of those returns `None` and the game draws the sky it drew
//! before. This follows [`Theme::load`](crate::Theme::load) exactly, and
//! for the same reason — a game that refuses to run because a wallpaper
//! moved is a worse outcome than a plain sky.

use std::io::Cursor;
use std::path::PathBuf;

use crate::backend::Color;

/// A decoded wallpaper, scaled to the size a game asked for.
///
/// Small by construction: the source is downsampled during load and the
/// original is dropped, so what survives is the few hundred kilobytes a
/// game actually draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backdrop {
    w: u32,
    h: u32,
    /// Row-major RGB, `w * h` entries.
    px: Vec<Color>,
}

impl Backdrop {
    /// Where Omarchy points at the live wallpaper.
    ///
    /// A symlink the theme switcher repoints, so reading it fresh is how
    /// a game sees the current image rather than the one installed.
    pub fn path() -> Option<PathBuf> {
        // ⚠️ An override, for LOOKING at this against wallpapers the
        // machine is not currently showing. Judging a theme-reactive
        // effect from the one theme that happens to be installed is how
        // the green-road bug survived four fixes (see `probe_themes`).
        if let Some(p) = std::env::var_os("OMARCADE_BACKGROUND") {
            return Some(PathBuf::from(p));
        }
        let home = std::env::var_os("HOME")?;
        Some(PathBuf::from(home).join(".local/state/omarchy/current/background"))
    }

    /// Load the live wallpaper, scaled to `w` x `h`. `None` on any
    /// failure, which is a normal state and never an error.
    pub fn load(w: u32, h: u32) -> Option<Backdrop> {
        Backdrop::load_from(&Backdrop::path()?, w, h)
    }

    /// Load a specific file. Exposed so tests can use a fixture instead
    /// of whatever the machine happens to be showing.
    pub fn load_from(path: &std::path::Path, w: u32, h: u32) -> Option<Backdrop> {
        if w == 0 || h == 0 {
            return None;
        }
        let bytes = std::fs::read(path).ok()?;
        let (src, sw, sh) = decode(&bytes)?;
        if sw == 0 || sh == 0 {
            return None;
        }
        Some(Backdrop { w, h, px: scale(&src, sw, sh, w, h) })
    }

    pub fn width(&self) -> u32 {
        self.w
    }

    pub fn height(&self) -> u32 {
        self.h
    }

    /// The colour at a pixel. Out of range reads clamp to the edge
    /// rather than panicking — a caller walking a scanline should not
    /// have to bounds-check every step.
    pub fn at(&self, x: u32, y: u32) -> Color {
        let x = x.min(self.w.saturating_sub(1));
        let y = y.min(self.h.saturating_sub(1));
        self.px[(y * self.w + x) as usize]
    }
}

/// Decode a JPEG or PNG into RGB triples.
///
/// ⚠️ SNIFFED FROM THE BYTES, NOT FROM THE EXTENSION. The symlink's name
/// is `background` with no suffix at all, and the file it points at may
/// be either format — the shipped themes contain both. Trusting a
/// filename here would mean never loading anything.
fn decode(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    // PNG's 8-byte signature.
    const PNG_MAGIC: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

    if bytes.starts_with(PNG_MAGIC) {
        let mut d = zune_png::PngDecoder::new(Cursor::new(bytes));
        let pixels = d.decode().ok()?;
        let (w, h) = d.dimensions()?;
        let comps = d.colorspace()?.num_components();
        let raw = pixels.u8()?;
        return Some((to_rgb(&raw, comps)?, w as u32, h as u32));
    }

    // Everything else is tried as JPEG. A file that is neither simply
    // fails to decode, which is the same `None` as a missing one.
    let mut d = zune_jpeg::JpegDecoder::new(Cursor::new(bytes));
    let raw = d.decode().ok()?;
    let (w, h) = d.dimensions()?;
    let comps = d.output_colorspace()?.num_components();
    Some((to_rgb(&raw, comps)?, w as u32, h as u32))
}

/// Normalise whatever the decoder produced into RGB triples.
///
/// Greyscale wallpapers exist and RGBA ones are common; both would
/// otherwise be read as RGB and come out as diagonal colour garbage,
/// because the stride would be wrong by a channel.
fn to_rgb(raw: &[u8], components: usize) -> Option<Vec<u8>> {
    match components {
        3 => Some(raw.to_vec()),
        4 => Some(
            raw.chunks_exact(4)
                .flat_map(|p| [p[0], p[1], p[2]])
                .collect(),
        ),
        1 => Some(raw.iter().flat_map(|&v| [v, v, v]).collect()),
        2 => Some(
            raw.chunks_exact(2)
                .flat_map(|p| [p[0], p[0], p[0]])
                .collect(),
        ),
        _ => None,
    }
}

/// Box-filter down to the target size.
///
/// ⚠️ AVERAGED, NOT SAMPLED. Nearest-neighbour from 6016 px to 960 is a
/// 6:1 decimation that keeps one pixel in thirty-six and throws the rest
/// away — which aliases hard on exactly the fine detail a photograph is
/// full of, and makes a landscape read as speckle. Averaging each source
/// block is what turns a photograph into the soft distant shape this is
/// wanted for.
///
/// Upscaling is not a case that arises (every wallpaper is far larger
/// than any game window), but a block of one pixel is still correct if
/// it ever did.
fn scale(src: &[u8], sw: u32, sh: u32, dw: u32, dh: u32) -> Vec<Color> {
    let mut out = Vec::with_capacity((dw * dh) as usize);
    for y in 0..dh {
        // The source rows this destination row covers.
        let y0 = (y as u64 * sh as u64 / dh as u64) as u32;
        let y1 = (((y + 1) as u64 * sh as u64 / dh as u64) as u32).max(y0 + 1).min(sh);
        for x in 0..dw {
            let x0 = (x as u64 * sw as u64 / dw as u64) as u32;
            let x1 = (((x + 1) as u64 * sw as u64 / dw as u64) as u32).max(x0 + 1).min(sw);

            let (mut r, mut g, mut b, mut n) = (0u64, 0u64, 0u64, 0u64);
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let i = ((sy as usize * sw as usize) + sx as usize) * 3;
                    if i + 2 >= src.len() {
                        continue;
                    }
                    r += src[i] as u64;
                    g += src[i + 1] as u64;
                    b += src[i + 2] as u64;
                    n += 1;
                }
            }
            out.push(if n == 0 {
                Color::rgb(0, 0, 0)
            } else {
                Color::rgb((r / n) as u8, (g / n) as u8, (b / n) as u8)
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 2x2 RGB PNG, spelled out byte by byte.
    ///
    /// ⚠️ Hand-built rather than encoded, because core has a DECODER and
    /// no encoder — pulling one in as a dev-dependency to test the
    /// decoder would double the dependency this whole module was kept
    /// small to avoid. The bytes below are a minimal valid PNG:
    /// signature, IHDR, a single IDAT holding one uncompressed deflate
    /// block, and IEND.
    fn png_2x2() -> Vec<u8> {
        fn crc32(data: &[u8]) -> u32 {
            let mut c: u32 = 0xffff_ffff;
            for &b in data {
                c ^= b as u32;
                for _ in 0..8 {
                    c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
                }
            }
            !c
        }
        fn chunk(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
            let mut v = Vec::new();
            v.extend_from_slice(&(body.len() as u32).to_be_bytes());
            v.extend_from_slice(kind);
            v.extend_from_slice(body);
            let mut crc_in = kind.to_vec();
            crc_in.extend_from_slice(body);
            v.extend_from_slice(&crc32(&crc_in).to_be_bytes());
            v
        }

        // Two rows, each a filter byte then 2 RGB pixels.
        let raw: Vec<u8> = vec![
            0, 255, 0, 0, 0, 255, 0, //
            0, 0, 0, 255, 255, 255, 255,
        ];
        // zlib: header, one stored (uncompressed) deflate block, adler32.
        let mut z = vec![0x78, 0x01];
        z.push(0x01); // final, stored
        z.extend_from_slice(&(raw.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(raw.len() as u16)).to_le_bytes());
        z.extend_from_slice(&raw);
        let (mut a, mut b) = (1u32, 0u32);
        for &byte in &raw {
            a = (a + byte as u32) % 65521;
            b = (b + a) % 65521;
        }
        z.extend_from_slice(&((b << 16) | a).to_be_bytes());

        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&2u32.to_be_bytes()); // width
        ihdr.extend_from_slice(&2u32.to_be_bytes()); // height
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit, RGB

        let mut out = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        out.extend(chunk(b"IHDR", &ihdr));
        out.extend(chunk(b"IDAT", &z));
        out.extend(chunk(b"IEND", &[]));
        out
    }

    #[test]
    fn a_missing_wallpaper_is_not_an_error() {
        let p = std::path::Path::new("/nonexistent/omarcade/test/background");
        assert_eq!(Backdrop::load_from(p, 64, 32), None);
    }

    /// ⚠️ THE FORMAT IS SNIFFED, NOT TAKEN FROM THE NAME. Omarchy's
    /// symlink is called `background` with no extension, so a decoder
    /// chosen by filename would never load anything at all.
    #[test]
    fn a_png_is_recognised_without_an_extension() {
        let dir = std::env::temp_dir().join("omarcade-backdrop-test");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("background");
        std::fs::write(&p, png_2x2()).unwrap();

        let b = Backdrop::load_from(&p, 2, 2).expect("a PNG with no suffix must still load");
        assert_eq!((b.width(), b.height()), (2, 2));
        assert_eq!(b.at(0, 0), Color::rgb(255, 0, 0));
        assert_eq!(b.at(1, 1), Color::rgb(255, 255, 255));
        let _ = std::fs::remove_file(&p);
    }

    /// ⚠️ DOWNSCALING AVERAGES. Sampling one pixel in thirty-six turns a
    /// photograph into speckle; this is the property that makes it read
    /// as a distant shape instead.
    #[test]
    fn downscaling_averages_rather_than_sampling() {
        // A 2x2 of pure red, green, blue, white averages to (128,128,128)
        // at 1x1 — a sampler would return whichever corner it picked.
        let src = vec![
            255, 0, 0, 0, 255, 0, //
            0, 0, 255, 255, 255, 255,
        ];
        let out = scale(&src, 2, 2, 1, 1);
        assert_eq!(out.len(), 1);
        let c = out[0];
        assert_eq!(
            (c.r, c.g, c.b),
            (127, 127, 127),
            "a downscale must average its source block, not pick from it",
        );
    }

    /// Out-of-range reads clamp rather than panic, so a caller walking a
    /// scanline never has to bounds-check.
    #[test]
    fn reading_past_the_edge_clamps() {
        let b = Backdrop { w: 2, h: 2, px: vec![Color::rgb(1, 2, 3); 4] };
        assert_eq!(b.at(99, 99), Color::rgb(1, 2, 3));
    }
}
