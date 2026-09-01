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
//!   lean    one car swept through the full range of poses
//!   roll    consecutive frames of the tread scrolling
//!   drive   the two speed tunings side by side, as a filmstrip over time
//!   gantry  the start gantry: raw pixels, over the road, and a size ladder
//!
//! Never screenshot a window for this — the render is deterministic and
//! the window is not.

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

use art::Art;
use drive::{Drive, Tuning};
use omarcade_core::sprite::Sprite;
use omarcade_core::{Canvas, Pose, Theme};
use road::{Camera, Road};

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
            "road" => draw_road(&mut c, &art, &theme, Road::straight(400)),
            "curve" => draw_road(&mut c, &art, &theme, render::demo_track()),
            "lean" => draw_lean(&mut c, &art, &theme),
            "roll" => draw_roll(&mut c, &art, &theme),
            "drive" => draw_drive(&mut c, &art, &theme),
            "gantry" => draw_gantry(&mut c, &art, &theme),
            "heights" => draw_gantry_heights(&mut c, &theme),
            "structures" => draw_structures(&mut c, &art, &theme),
            "proportion" => draw_proportion(&mut c, &art, &theme),
            "lap" => draw_lap(&mut c, &art, &theme),
            other => {
                eprintln!(
                    "unknown scene {other:?} — try: sheet | road | curve | lean | roll | drive | gantry | heights | structures | proportion | lap"
                );
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

/// A track that bends, so a curve can actually be judged. A straight
/// road looks identical whether curvature works or not.
/// The cars on a real road, which is the only view that says whether
/// they read at speed.
///
/// This is the game's own renderer at full-window size — not a second
/// implementation. An earlier version kept its own copy here, which meant
/// the still and the running game could drift apart while each looked
/// correct on its own terms.
fn draw_road(c: &mut Canvas<'_>, art: &Art, theme: &Theme, road: Road) {
    let tuning = Tuning::from_corner(&road, 1.5);
    let mut car = Drive::new();
    // Up to speed with a driver holding the line, a little way in, so the
    // scene shows the bend rather than the start line.
    let dt = 1.0 / 120.0;
    for _ in 0..(6.0 / dt) as usize {
        let correction = (-car.x * 3.0).clamp(-1.0, 1.0);
        car.update(dt, 1.0, 0.0, correction, &road, &tuning);
    }
    // Placed so the roadside props are mid-stream rather than all bunched
    // at the horizon — this scene exists to judge them.
    car.z = road.segment_length() * 3.0;

    // Placed as a FRACTION of the visible road, not in segments. At a
    // draw distance of 120 segments, "nine segments ahead" is still 7% of
    // the way to the horizon — the cars rendered correctly and were three
    // pixels tall. What matters is how far up the visible depth a car
    // sits, so that is what is written.
    let visible = road.draw_distance() as f32 * road.segment_length();
    let rivals = [
        (car.z + visible * 0.30, -0.45, 0usize),
        (car.z + visible * 0.16, 0.40, 1),
        (car.z + visible * 0.075, -0.25, 2),
        (car.z + visible * 0.03, 0.42, 3),
    ];

    render::draw_road_into(
        c, art, theme, &road, &tuning, &car, 0.0, &rivals, 0, 0, W, H,
    );
}

/// The two speed tunings, side by side, as a filmstrip over time.
///
/// A still cannot show a feel difference and a single frame cannot show
/// motion, so this renders the SAME track from the SAME start, sampled at
/// the same elapsed seconds, under both derivations. The only thing that
/// differs between the two rows is the tuning, which is what makes the
/// comparison honest.
///
/// Top row: A, top speed derived from corner reaction time.
/// Bottom row: B, steer rate derived from verge-to-verge crossing time.
///
/// The numbers behind it are printed to stdout — a filmstrip shows you
/// how it feels, not what it measures.
fn draw_drive(c: &mut Canvas<'_>, art: &Art, theme: &Theme) {
    const PANELS: usize = 4;
    // A fixed simulation step, so both rows are integrated identically and
    // any difference is the tuning rather than the arithmetic.
    const DT: f32 = 1.0 / 120.0;

    let pw = W / PANELS as u32;
    let ph = H / 2;

    let road = render::demo_track();
    let tunings = [
        ("A  corner 1.5s", Tuning::from_corner(&road, 1.5)),
        ("B  crossing 1.2s", Tuning::from_crossing(&road, 1.2)),
    ];

    // Sample by POSITION ON THE TRACK, not by wall-clock.
    //
    // Sampling every N seconds sounds like the fair comparison and is not:
    // the faster tuning clears the whole bend inside one step, so both
    // rows show the same two pictures and the strip proves nothing. What
    // actually differs between the tunings is HOW LONG the same piece of
    // road takes, so the road is the constant and time is what is
    // measured. Each column is the same place on the track for both rows;
    // the elapsed seconds printed under each are the answer.
    let marks = [1_500.0f32, 3_500.0, 6_000.0, 40_000.0];

    println!("\n  the two tunings, on the same track:\n");
    println!(
        "  {:<18} {:>10} {:>10} {:>9} {:>9}",
        "", "top speed", "steer", "react s", "cross s",
    );

    let mut times = [[0.0f32; 4]; 2];

    for (row, (label, tuning)) in tunings.iter().enumerate() {
        println!(
            "  {label:<18} {:>10.0} {:>10.2} {:>9.2} {:>9.2}",
            tuning.top_speed,
            tuning.steer_rate,
            tuning.reaction_seconds(&road),
            tuning.crossing_seconds(),
        );

        let mut car = Drive::new();
        // Up to speed with a driver already holding the line, so the strip
        // compares cornering rather than launching.
        for _ in 0..(6.0 / DT) as usize {
            let correction = (-car.x * 3.0).clamp(-1.0, 1.0);
            car.update(DT, 1.0, 0.0, correction, &road, tuning);
        }
        car.z = 0.0;

        let mut elapsed = 0.0f32;
        for (panel, mark) in marks.iter().enumerate() {
            // Drive to this mark on the track, timing how long it takes.
            while car.z < *mark && elapsed < 60.0 {
                let correction = (-car.x * 3.0).clamp(-1.0, 1.0);
                car.update(DT, 1.0, 0.0, correction, &road, tuning);
                elapsed += DT;
            }
            times[row][panel] = elapsed;

            let x0 = panel as u32 * pw;
            let y0 = row as u32 * ph;
            render::draw_road_into(
                c, art, theme, &road, tuning, &car, 0.0, &[], x0, y0, pw, ph,
            );

            // Bar length is ELAPSED TIME to reach this mark, scaled to the
            // slowest row, so a longer bar reads directly as "took longer".
            let bar = (pw as f32 * 0.85 * (elapsed / 12.0).min(1.0)) as u32;
            let tint = if row == 0 { theme.red } else { theme.blue };
            c.fill_rect(x0 as i32 + 8, (y0 + ph - 12) as i32, bar.max(2), 5, tint);
        }
    }

    println!();
    for (row, (label, _)) in tunings.iter().enumerate() {
        print!("  {label:<18} reached each mark at:");
        for t in times[row] {
            print!(" {t:>6.2}s");
        }
        println!();
    }

    // Divider between the rows, so the two are not read as one strip.
    c.fill_rect(0, ph as i32 - 1, W, 2, theme.foreground);
    for p in 1..PANELS as u32 {
        c.fill_rect((p * pw) as i32 - 1, 0, 2, H, theme.foreground);
    }
    println!(
        "\n  columns are the SAME PLACE on the track; bar length = time taken to get there"
    );
    println!("  top row A (red) · bottom row B (blue)");
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

/// One car swept through the full cornering range.
///
/// The row exists to answer a question a still frame cannot: does the
/// car stay a car at full lock, or does it shear into a parallelogram?
/// Poses are generated, not authored, so this is also the check that
/// the transform degrades gracefully at the extremes rather than only
/// looking right in the middle.
fn draw_lean(c: &mut Canvas<'_>, art: &Art, theme: &Theme) {
    c.clear(theme.background);

    let steps = 5;
    // Scale chosen so the cars do NOT overlap: a leaning sprite is
    // wider than its grid, and cars running into each other reads as
    // the transform smearing when it is only the layout being too
    // tight.
    let span = W as f32 / steps as f32;
    let s = (span * 0.78) / art.player.width() as f32;

    let rows: [(&str, f32, fn(f32) -> Pose); 3] = [
        ("cornering", 220.0, |t| Pose::cornering(t)),
        ("lean only", 430.0, |t| Pose { lean: t, turn: 0.0, squat: 0.0 }),
        ("turn only", 640.0, |t| Pose { lean: 0.0, turn: t, squat: 0.0 }),
    ];

    for (_, ground, pose_of) in rows {
        // A ground line per row: the wheels must sit ON it at every
        // pose, which is the property that separates banking from
        // sliding sideways.
        c.fill_rect(0, ground as i32, W, 1, theme.muted);
        for i in 0..steps {
            let t = i as f32 / (steps - 1) as f32 * 2.0 - 1.0;
            let x = span * (i as f32 + 0.5);
            art.player.draw_ground_posed(c, x, ground, s, pose_of(t), None);
        }
    }
}

/// Consecutive frames of the tread rolling.
///
/// A still cannot show motion, so this lays successive frames side by
/// side: the wheels must differ between them while everything else — the
/// wing, the lights, the diffuser — stays pinned. That second half is
/// the real check, because the tread shares its colour with the
/// diffuser highlights and an animation keyed on colour rather than on
/// the tread letter would strobe both.
fn draw_roll(c: &mut Canvas<'_>, art: &Art, theme: &Theme) {
    c.clear(theme.background);

    let steps = 5;
    let span = W as f32 / steps as f32;
    let s = (span * 0.80) / art.player.width() as f32;

    // Two rows at different speeds, so the difference between a gentle
    // roll and a fast one is visible rather than asserted.
    for (row, (ground, per_frame)) in [(250.0f32, 0.35f32), (620.0, 0.9)].iter().enumerate() {
        c.fill_rect(0, *ground as i32, W, 1, theme.muted);
        for i in 0..steps {
            let roll = i as f32 * per_frame;
            let x = span * (i as f32 + 0.5);
            art.player.draw_ground_rolling(
                c,
                x,
                *ground,
                s,
                Pose::UPRIGHT,
                roll,
                None,
            );
        }
        let _ = row;
    }
}

/// The start/finish gantry, three ways.
///
/// This exists to answer "what does it look like" before any placement
/// code is written, so the art's real needs are visible rather than
/// guessed at. Three panels, because a road-spanning structure fails in
/// three different ways and one view hides two of them:
///
///   TOP LEFT   the raw pixels, big, on neutral ground — are the shapes
///              right at all
///   TOP RIGHT  the measured facts, printed to stdout, not drawn
///   BOTTOM     over the real road at the real projection, then a size
///              ladder as it approaches
///
/// ⚠️ The grid is 160 wide but the INK spans columns 17..=142. Scaling
/// the grid width to the road would leave the structure ~79% of the road
/// wide, floating clear of both verges. Everything here scales by ink.
fn draw_gantry(c: &mut Canvas<'_>, art: &Art, theme: &Theme) {
    c.clear(theme.darker_background);

    let g = &art.gantry;

    // The ink bounds, measured from the art rather than hardcoded — if
    // Brian redraws the gantry with different padding this follows it.
    let (ink_x0, ink_x1) = ink_columns(art::GANTRY);
    let ink_w = (ink_x1 - ink_x0 + 1) as f32;
    let grid_w = g.width() as f32;
    // Where the ink's centre sits relative to the grid's centre, in grid
    // pixels. Nonzero if the padding is lopsided, and then every "centre
    // it on the road" would be off by exactly this much.
    let ink_cx = (ink_x0 + ink_x1 + 1) as f32 / 2.0;
    let centre_bias = ink_cx - grid_w / 2.0;

    println!("\n  the gantry, measured:\n");
    println!("    grid            {} x {}", g.width(), g.height());
    println!("    ink columns     {ink_x0}..={ink_x1}  ({ink_w} wide)");
    println!("    padding         {ink_x0} left, {} right", grid_w as usize - 1 - ink_x1);
    println!("    ink / grid      {:.3}", ink_w / grid_w);
    println!("    centre bias     {centre_bias:+.1} px  (0 = ink is centred in the grid)");
    println!("    lit pixels      {}", g.ink());

    // ---- panel 1: the raw pixels, on neutral ground -------------------
    //
    // Big enough that individual pixels are visible. This is the panel
    // that answers "is the lattice right", which the road view cannot.
    let top_h = 300u32;
    c.fill_rect(0, 0, W, top_h, theme.background);
    c.fill_rect(0, top_h as i32 - 2, W, 2, theme.dark_foreground);

    let flat_s = ((W as f32 - 40.0) / grid_w).min((top_h as f32 - 40.0) / g.height() as f32);
    g.draw_ground(
        c,
        W as f32 / 2.0 - centre_bias * flat_s,
        top_h as f32 - 20.0,
        flat_s,
    );

    // ---- panel 2: over the real road ----------------------------------
    //
    // The game's own renderer draws the road, then the gantry is placed
    // through the SAME projection the renderer uses. Placing it by eye
    // here would prove nothing about whether it can be placed for real.
    let road = Road::straight(400);
    let tuning = Tuning::from_corner(&road, 1.5);
    let mut car = Drive::new();
    let dt = 1.0 / 120.0;
    for _ in 0..(6.0 / dt) as usize {
        let correction = (-car.x * 3.0).clamp(-1.0, 1.0);
        car.update(dt, 1.0, 0.0, correction, &road, &tuning);
    }
    car.z = road.segment_length() * 3.0;

    let road_y = top_h;
    let road_h = H - top_h;
    render::draw_road_into(
        c, &art, theme, &road, &tuning, &car, 0.0, &[], 0, road_y, W, road_h,
    );

    // A size ladder: the same structure at four distances up the visible
    // depth, so how fast it grows on approach is visible. A single
    // placement cannot show that, and growth rate is the thing most
    // likely to feel wrong.
    // 0.85 — the SAME fill the renderer uses (render.rs:196). A different
    // value here would project the gantry against a camera the drawn road
    // was never rendered with, and it would sit convincingly in the wrong
    // place.
    let camera = Camera::for_road(&road, 0.85);
    let visible = road.draw_distance() as f32 * road.segment_length();
    // Nearest last, so closer structures paint over further ones — the
    // same order the renderer draws traffic in.
    for frac in [0.42f32, 0.26, 0.15, 0.075] {
        let z = car.z + visible * frac;
        let Some(p) = road.project(
            &camera,
            car.z,
            car.x * road.width() / 2.0,
            z,
            W as f32,
            road_h as f32,
        ) else {
            continue;
        };

        // THE SIZE RULE: the gantry spans the road, so its scale comes
        // from the projected road WIDTH — not from a height in
        // half-widths the way a roadside prop does. A prop is beside the
        // road and its height is the free parameter; this thing's width
        // is pinned by the thing it straddles.
        let span_half_widths = 2.6;
        let scale = p.half_width * span_half_widths / ink_w;

        // The legs must land ON the verges, so the sprite is centred on
        // the road's centre line corrected for the ink's own offset.
        let x = p.x - centre_bias * scale;
        // And it must stand ON the road surface: the projected y IS the
        // road at this distance, so that is the sprite's ground line.
        let y = road_y as f32 + p.y;

        g.draw_ground(c, x, y, scale);
    }

    println!("\n  drawn over the road at 2.6 half-widths of span, scaled by INK width");
    println!("  four distances, 7.5% to 42% of the visible depth\n");
}

/// The first and last columns of a grid that contain any ink.
///
/// Measured rather than hardcoded so a redraw with different padding is
/// followed automatically — a hardcoded 17 would silently go wrong the
/// first time the art changed.
fn ink_columns(rows: &[&str]) -> (usize, usize) {
    let w = rows[0].len();
    let mut lo = w;
    let mut hi = 0usize;
    for row in rows {
        for (i, ch) in row.chars().enumerate() {
            if ch != '.' {
                lo = lo.min(i);
                hi = hi.max(i);
            }
        }
    }
    (lo, hi)
}

/// Gantry with the leg cut to 11 rows — a candidate height.
const GANTRY_SHORT: &[&str] = &[
    ".................EEEEEEEFFFFFFFFFFFFFTTKKTTKKTTKKAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAKKTTKKTTKKTTFFFFFFFFFFFFFEEEEEEE.................",
    ".................EC...EEFFF........FFTTKKTTKKTTKKFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFKKTTKKTTKKTTFF........FFFEE...CE.................",
    ".................E.C.E.EFIFIIIIIIIFFFKKTTKKTTKKTTFAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFTTKKTTKKTTKKFFFIIIIIIIFIFE.E.C.E.................",
    ".................E..E..EFI.F.....FFIFKKTTKKTTKKTTFAAAAAFFFFFFAAAAFFFFFFFFAAAAAFFFFFAAAAAFFFFFFFAAAFFFFFFFFAAAAFTTKKTTKKTTKKFIFF.....F.IFE..E..E.................",
    ".................E.E.C.EFI..F...FF.IFTTKKTTKKTTKKFAAAAAFAAAAFFAAAFFFFFFFFAAAAAFAAFFAAAAAFAAAAFFAAAFFFFFFFFAAAAFKKTTKKTTKKTTFI.FF...F..IFE.C.E.E.................",
    ".................EE...CEFI...F.FFII.FTTKKTTKKTTKKFAAAAAFAAAAAFAAAAAAFFAAAAAAAAFAAAFAAAAAFAAAAAFAAAAAAFFAAAAAAAFKKTTKKTTKKTTF.IIFF.F...IFEC...EE.................",
    ".................EEC..CEFI...FFFII..FKKTTKKTTKKTTFAAAAAFFAAAAAAAAAAAFFAAAAAAAFFAAAFAAAAAFAAAAAFAAAAAAFFAAAAAAAFTTKKTTKKTTKKF..IIFFF...IFEC..CEE.................",
    ".................EEEEEEEFI..FF.FI...FKKTTKKTTKKTTFAAAAAFFFFFFFAAAAAAFFAAAAAAAFFFFFFAAAAAFFFFFFFAAAAAAFFAAAAAAAFTTKKTTKKTTKKF...IF.FF..IFEEEEEEE.................",
    ".................EC...EEFI.FF..IF...FTTKKTTKKTTKKFAAAAAAAAAAAFAAAAAAFFAAAAAAFFFFFFFFAAAAFAAAFFAAAAAAAFFAAAAAAAFKKTTKKTTKKTTF...FI..FF.IFEE...CE.................",
    ".................E.C.E.EFI.F..II.F..FTTKKTTKKTTKKFAAAAAFFAAAAFAAAAAAFFAAAAAAFFFFFFFFAAAAFAAAAFAAAAAAAFFAAAAAAAFKKTTKKTTKKTTF..F.II..F.IFE.E.C.E.................",
    ".................E..E..EFIF..II...F.FKKTTKKTTKKTTFAAAAAFFFFFFFAAAAAAFFAAAAAFFAAAAAAFFAAAFAAAAFFAAAAAAFFAAAAAAAFTTKKTTKKTTKKF.F...II..FIFE..E..E.................",
    ".................E.E.C.EFFF.I......FFKKTTKKTTKKTTFAAAAAAFFFFFFAAAAAAFFAAAAAFFAAAAAAAFAAAFAAAAFFAAAAAAFFAAAAAAAFTTKKTTKKTTKKFF......I.FFFE.C.E.E.................",
    ".................EE...CEFFFFFFFFFFFFFTTKKTTKKTTKKFAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFKKTTKKTTKKTTFFFFFFFFFFFFFEC...EE.................",
    ".................EE....EJJIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIJJE....EE.................",
    ".................EEEEEEEJJ............................................................................................................JJEEEEEEE.................",
    ".................EC...EEJJ............................................................................................................JJEE...CE.................",
    ".................E.C.E.EJJ............................................................................................................JJE.E.C.E.................",
    ".................E..E..EJJ............................................................................................................JJE..E..E.................",
    ".................E.E.C.EJJ............................................................................................................JJE.C.E.E.................",
    ".................EE...CEJJ............................................................................................................JJEC...EE.................",
    ".................EE....EJC............................................................................................................CJE....EE.................",
    ".................EC....EJJ............................................................................................................JJE.....E.................",
    ".................EC....EJJ............................................................................................................JJE.....E.................",
    ".................EC....EJJ............................................................................................................JJE.....E.................",
    ".................E.....EJ..............................................................................................................JE.....E.................",
];

/// Gantry with the leg cut to 18 rows — a candidate height.
const GANTRY_MID: &[&str] = &[
    ".................EEEEEEEFFFFFFFFFFFFFTTKKTTKKTTKKAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAKKTTKKTTKKTTFFFFFFFFFFFFFEEEEEEE.................",
    ".................EC...EEFFF........FFTTKKTTKKTTKKFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFKKTTKKTTKKTTFF........FFFEE...CE.................",
    ".................E.C.E.EFIFIIIIIIIFFFKKTTKKTTKKTTFAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFTTKKTTKKTTKKFFFIIIIIIIFIFE.E.C.E.................",
    ".................E..E..EFI.F.....FFIFKKTTKKTTKKTTFAAAAAFFFFFFAAAAFFFFFFFFAAAAAFFFFFAAAAAFFFFFFFAAAFFFFFFFFAAAAFTTKKTTKKTTKKFIFF.....F.IFE..E..E.................",
    ".................E.E.C.EFI..F...FF.IFTTKKTTKKTTKKFAAAAAFAAAAFFAAAFFFFFFFFAAAAAFAAFFAAAAAFAAAAFFAAAFFFFFFFFAAAAFKKTTKKTTKKTTFI.FF...F..IFE.C.E.E.................",
    ".................EE...CEFI...F.FFII.FTTKKTTKKTTKKFAAAAAFAAAAAFAAAAAAFFAAAAAAAAFAAAFAAAAAFAAAAAFAAAAAAFFAAAAAAAFKKTTKKTTKKTTF.IIFF.F...IFEC...EE.................",
    ".................EEC..CEFI...FFFII..FKKTTKKTTKKTTFAAAAAFFAAAAAAAAAAAFFAAAAAAAFFAAAFAAAAAFAAAAAFAAAAAAFFAAAAAAAFTTKKTTKKTTKKF..IIFFF...IFEC..CEE.................",
    ".................EEEEEEEFI..FF.FI...FKKTTKKTTKKTTFAAAAAFFFFFFFAAAAAAFFAAAAAAAFFFFFFAAAAAFFFFFFFAAAAAAFFAAAAAAAFTTKKTTKKTTKKF...IF.FF..IFEEEEEEE.................",
    ".................EC...EEFI.FF..IF...FTTKKTTKKTTKKFAAAAAAAAAAAFAAAAAAFFAAAAAAFFFFFFFFAAAAFAAAFFAAAAAAAFFAAAAAAAFKKTTKKTTKKTTF...FI..FF.IFEE...CE.................",
    ".................E.C.E.EFI.F..II.F..FTTKKTTKKTTKKFAAAAAFFAAAAFAAAAAAFFAAAAAAFFFFFFFFAAAAFAAAAFAAAAAAAFFAAAAAAAFKKTTKKTTKKTTF..F.II..F.IFE.E.C.E.................",
    ".................E..E..EFIF..II...F.FKKTTKKTTKKTTFAAAAAFFFFFFFAAAAAAFFAAAAAFFAAAAAAFFAAAFAAAAFFAAAAAAFFAAAAAAAFTTKKTTKKTTKKF.F...II..FIFE..E..E.................",
    ".................E.E.C.EFFF.I......FFKKTTKKTTKKTTFAAAAAAFFFFFFAAAAAAFFAAAAAFFAAAAAAAFAAAFAAAAFFAAAAAAFFAAAAAAAFTTKKTTKKTTKKFF......I.FFFE.C.E.E.................",
    ".................EE...CEFFFFFFFFFFFFFTTKKTTKKTTKKFAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFKKTTKKTTKKTTFFFFFFFFFFFFFEC...EE.................",
    ".................EE....EJJIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIJJE....EE.................",
    ".................EEEEEEEJJ............................................................................................................JJEEEEEEE.................",
    ".................EC...EEJJ............................................................................................................JJEE...CE.................",
    ".................E.C.E.EJJ............................................................................................................JJE.E.C.E.................",
    ".................E..E..EJJ............................................................................................................JJE..E..E.................",
    ".................E.E.C.EJJ............................................................................................................JJE.C.E.E.................",
    ".................EE...CEJJ............................................................................................................JJEC...EE.................",
    ".................EE....EJC............................................................................................................CJE....EE.................",
    ".................EEEEEEEJJ............................................................................................................JJEEEEEEE.................",
    ".................EC...EEJJ............................................................................................................JJEE...CE.................",
    ".................ECC.E.EJJ............................................................................................................JJE.E.CCE.................",
    ".................ECCE..EJJ............................................................................................................JJE..ECCE.................",
    ".................E.E.C.EJJ............................................................................................................JJE.C.E.E.................",
    ".................EE...CEJJ............................................................................................................JJEC...EE.................",
    ".................EE....EJC............................................................................................................CJE....EE.................",
    ".................EC....EJJ............................................................................................................JJE.....E.................",
    ".................EC....EJJ............................................................................................................JJE.....E.................",
    ".................EC....EJJ............................................................................................................JJE.....E.................",
    ".................E.....EJ..............................................................................................................JE.....E.................",
];

/// Gantry with the leg cut to 25 rows — a candidate height.
const GANTRY_TALL: &[&str] = &[
    ".................EEEEEEEFFFFFFFFFFFFFTTKKTTKKTTKKAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAKKTTKKTTKKTTFFFFFFFFFFFFFEEEEEEE.................",
    ".................EC...EEFFF........FFTTKKTTKKTTKKFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFKKTTKKTTKKTTFF........FFFEE...CE.................",
    ".................E.C.E.EFIFIIIIIIIFFFKKTTKKTTKKTTFAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFTTKKTTKKTTKKFFFIIIIIIIFIFE.E.C.E.................",
    ".................E..E..EFI.F.....FFIFKKTTKKTTKKTTFAAAAAFFFFFFAAAAFFFFFFFFAAAAAFFFFFAAAAAFFFFFFFAAAFFFFFFFFAAAAFTTKKTTKKTTKKFIFF.....F.IFE..E..E.................",
    ".................E.E.C.EFI..F...FF.IFTTKKTTKKTTKKFAAAAAFAAAAFFAAAFFFFFFFFAAAAAFAAFFAAAAAFAAAAFFAAAFFFFFFFFAAAAFKKTTKKTTKKTTFI.FF...F..IFE.C.E.E.................",
    ".................EE...CEFI...F.FFII.FTTKKTTKKTTKKFAAAAAFAAAAAFAAAAAAFFAAAAAAAAFAAAFAAAAAFAAAAAFAAAAAAFFAAAAAAAFKKTTKKTTKKTTF.IIFF.F...IFEC...EE.................",
    ".................EEC..CEFI...FFFII..FKKTTKKTTKKTTFAAAAAFFAAAAAAAAAAAFFAAAAAAAFFAAAFAAAAAFAAAAAFAAAAAAFFAAAAAAAFTTKKTTKKTTKKF..IIFFF...IFEC..CEE.................",
    ".................EEEEEEEFI..FF.FI...FKKTTKKTTKKTTFAAAAAFFFFFFFAAAAAAFFAAAAAAAFFFFFFAAAAAFFFFFFFAAAAAAFFAAAAAAAFTTKKTTKKTTKKF...IF.FF..IFEEEEEEE.................",
    ".................EC...EEFI.FF..IF...FTTKKTTKKTTKKFAAAAAAAAAAAFAAAAAAFFAAAAAAFFFFFFFFAAAAFAAAFFAAAAAAAFFAAAAAAAFKKTTKKTTKKTTF...FI..FF.IFEE...CE.................",
    ".................E.C.E.EFI.F..II.F..FTTKKTTKKTTKKFAAAAAFFAAAAFAAAAAAFFAAAAAAFFFFFFFFAAAAFAAAAFAAAAAAAFFAAAAAAAFKKTTKKTTKKTTF..F.II..F.IFE.E.C.E.................",
    ".................E..E..EFIF..II...F.FKKTTKKTTKKTTFAAAAAFFFFFFFAAAAAAFFAAAAAFFAAAAAAFFAAAFAAAAFFAAAAAAFFAAAAAAAFTTKKTTKKTTKKF.F...II..FIFE..E..E.................",
    ".................E.E.C.EFFF.I......FFKKTTKKTTKKTTFAAAAAAFFFFFFAAAAAAFFAAAAAFFAAAAAAAFAAAFAAAAFFAAAAAAFFAAAAAAAFTTKKTTKKTTKKFF......I.FFFE.C.E.E.................",
    ".................EE...CEFFFFFFFFFFFFFTTKKTTKKTTKKFAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFKKTTKKTTKKTTFFFFFFFFFFFFFEC...EE.................",
    ".................EE....EJJIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIJJE....EE.................",
    ".................EEEEEEEJJ............................................................................................................JJEEEEEEE.................",
    ".................EC...EEJJ............................................................................................................JJEE...CE.................",
    ".................E.C.E.EJJ............................................................................................................JJE.E.C.E.................",
    ".................E..E..EJJ............................................................................................................JJE..E..E.................",
    ".................E.E.C.EJJ............................................................................................................JJE.C.E.E.................",
    ".................EE...CEJJ............................................................................................................JJEC...EE.................",
    ".................EE....EJC............................................................................................................CJE....EE.................",
    ".................EEEEEEEJJ............................................................................................................JJEEEEEEE.................",
    ".................EC...EEJJ............................................................................................................JJEE...CE.................",
    ".................ECC.E.EJJ............................................................................................................JJE.E.CCE.................",
    ".................ECCE..EJJ............................................................................................................JJE..ECCE.................",
    ".................E.E.C.EJJ............................................................................................................JJE.C.E.E.................",
    ".................EE...CEJJ............................................................................................................JJEC...EE.................",
    ".................EE....EJC............................................................................................................CJE....EE.................",
    ".................EEEEEEEJJ............................................................................................................JJEEEEEEE.................",
    ".................EC...EEJJ............................................................................................................JJEE...CE.................",
    ".................ECC.E.EJJ............................................................................................................JJE.E.CCE.................",
    ".................E..E..EJJ............................................................................................................JJE..E..E.................",
    ".................E.E.C.EJJ............................................................................................................JJE.C.E.E.................",
    ".................EE...CEJJ............................................................................................................CJEC...EE.................",
    ".................EEEEEEEJC............................................................................................................JJEEEEEEE.................",
    ".................EC....EJJ............................................................................................................JJE.....E.................",
    ".................EC....EJJ............................................................................................................JJE.....E.................",
    ".................EC....EJJ............................................................................................................JJE.....E.................",
    ".................E.....EJ..............................................................................................................JE.....E.................",
];

/// The gantry at four leg heights, over the road.
///
/// ⚠️ ASKED AND ANSWERED: the art keeps its 43 rows of leg. This scene is
/// kept because it is the evidence, not because the question is open.
///
/// The question was one a flat sprite sheet cannot answer: how tall
/// should the legs be? On the sheet the gantry is drawn five times larger
/// than it ever appears in play, with nothing beside it — and read there
/// it looks like a radio mast. Read HERE, over the road, all four heights
/// are near indistinguishable, because the extra leg sits below the
/// banner where perspective swallows it.
///
/// The banner is identical in every one. Only the leg differs, and that
/// is the point: if the four panels look the same, the leg was never the
/// thing worth changing.
fn draw_gantry_heights(c: &mut Canvas<'_>, theme: &Theme) {
    let road = Road::straight(400);
    let tuning = Tuning::from_corner(&road, 1.5);
    let mut car = Drive::new();
    let dt = 1.0 / 120.0;
    for _ in 0..(6.0 / dt) as usize {
        let correction = (-car.x * 3.0).clamp(-1.0, 1.0);
        car.update(dt, 1.0, 0.0, correction, &road, &tuning);
    }
    car.z = road.segment_length() * 3.0;

    let options: [(&str, &[&str]); 4] = [
        ("short  11", GANTRY_SHORT),
        ("mid    18", GANTRY_MID),
        ("tall   25", GANTRY_TALL),
        ("as drawn 43", art::GANTRY),
    ];

    let pw = W / 2;
    let ph = H / 2;
    let camera = Camera::for_road(&road, 0.85);
    let visible = road.draw_distance() as f32 * road.segment_length();

    println!("\n  gantry leg heights, all with the same banner\n");

    for (i, (label, rows)) in options.iter().enumerate() {
        let x0 = (i as u32 % 2) * pw;
        let y0 = (i as u32 / 2) * ph;

        render::draw_road_into(
            c, &Art::load(theme), theme, &road, &tuning, &car, 0.0, &[], x0, y0, pw, ph,
        );

        let sprite = Sprite::new(rows, &art::gantry_palette(theme));
        let (ix0, ix1) = ink_columns(rows);
        let ink_w = (ix1 - ix0 + 1) as f32;
        let bias = (ix0 + ix1 + 1) as f32 / 2.0 - sprite.width() as f32 / 2.0;

        // ONE distance, chosen near enough that the leg is actually
        // legible. An earlier version drew 30% and 10% of the visible
        // depth: at 30% the whole structure is a smudge a few pixels
        // tall and all four heights look identical, and at 10% the
        // ground line falls below the panel and it is clipped away
        // entirely. Neither showed the thing being chosen between.
        for frac in [0.045f32] {
            let z = car.z + visible * frac;
            let Some(p) = road.project(
                &camera, car.z, car.x * road.width() / 2.0, z, pw as f32, ph as f32,
            ) else { continue };
            let scale = p.half_width * 2.6 / ink_w;
            sprite.draw_ground(
                c,
                x0 as f32 + p.x - bias * scale,
                y0 as f32 + p.y,
                scale,
            );
        }

        println!("    {label} leg rows");
    }

    // Dividers, so the four read as four.
    c.fill_rect(pw as i32 - 1, 0, 2, H, theme.foreground);
    c.fill_rect(0, ph as i32 - 1, W, 2, theme.foreground);
    println!("\n    top-left short · top-right mid · bottom-left tall · bottom-right as drawn\n");
}

/// The roadside structures together, in the scene.
///
/// The gantry spans the road; the billboard stands beside it. They are
/// different KINDS — one is scaled by the road's width, the other by its
/// own height — and the only way to know whether they read as belonging
/// to the same world is to put them in one picture.
fn draw_structures(c: &mut Canvas<'_>, art: &Art, theme: &Theme) {
    let road = Road::straight(400);
    let tuning = Tuning::from_corner(&road, 1.5);
    let mut car = Drive::new();
    let dt = 1.0 / 120.0;
    for _ in 0..(6.0 / dt) as usize {
        let correction = (-car.x * 3.0).clamp(-1.0, 1.0);
        car.update(dt, 1.0, 0.0, correction, &road, &tuning);
    }
    car.z = road.segment_length() * 3.0;

    render::draw_road_into(c, art, theme, &road, &tuning, &car, 0.0, &[], 0, 0, W, H);

    let camera = Camera::for_road(&road, 0.85);
    let visible = road.draw_distance() as f32 * road.segment_length();

    // Billboards first and furthest, so nearer things paint over them.
    let (bx0, bx1) = ink_columns(art::BILLBOARD);
    let (_, by1) = ink_rows(art::BILLBOARD);
    let (_, y1) = ink_rows(art::BILLBOARD);
    let b_ink_w = (bx1 - bx0 + 1) as f32;
    let b_bias = (bx0 + bx1 + 1) as f32 / 2.0 - art.billboard.width() as f32 / 2.0;
    // The sprite's own ground line: the billboard's ink stops before the
    // bottom of its grid, so `draw_ground` would hang it in the air by
    // the leftover rows. Correcting by the gap is what stands it on the
    // verge rather than above it.
    let b_foot_gap = (art.billboard.height() as usize - 1 - by1) as f32;
    let _ = y1;

    println!("\n  the roadside structures, in the scene\n");
    println!("    billboard ink   cols {bx0}..={bx1} ({b_ink_w} wide), {b_foot_gap} blank rows below");

    // ⚠️ NEAR, and this is the correction that mattered most. At 34% of
    // the visible depth the projected road is TEN PIXELS of half-width,
    // so a billboard there is about twenty pixels of anything — it read
    // as a white speck hanging above the horizon, and I diagnosed that
    // as a broken ground line twice before doing the arithmetic (L020:
    // measure the layout before suspecting the transform).
    //
    // Nothing was wrong with the placement. It was simply very far away.
    // The depths here are where a billboard is actually a billboard.
    for (frac, side) in [(0.085f32, 1.0f32), (0.035, -1.0)] {
        let z = car.z + visible * frac;
        let Some(p) = road.project(
            &camera, car.z, car.x * road.width() / 2.0, z, W as f32, H as f32,
        ) else { continue };

        // A BILLBOARD IS SCALED BY ITS OWN HEIGHT, not by the road's
        // width — it does not span anything, so nothing pins its width.
        //
        // ⚠️ BY THE PANEL, NOT BY THE WHOLE SPRITE. Scaling so the full
        // ink height (panel + posts, 45 rows) came to one half-width made
        // the panel itself only two thirds of that and the posts under
        // four pixels tall — which read as a sign FLOATING above the
        // ground rather than standing on it. What a viewer judges the
        // size of is the sign, so the sign is what the size is stated
        // about.
        let (by0, _) = ink_rows(art::BILLBOARD);
        let panel_h = (art::BILLBOARD_PANEL_ROWS.1 - art::BILLBOARD_PANEL_ROWS.0 + 1) as f32;
        let _ = by0;
        let scale = p.half_width * 0.95 / panel_h;
        // Placed BESIDE the road, just off the verge. Far enough not to
        // be something you can hit, near enough to be readable — at 1.55
        // out they sat almost on the horizon and were too small to be
        // signs at all.
        let x = p.x + side * p.half_width * 1.25 - b_bias * scale;
        let y = p.y + b_foot_gap * scale;
        art.billboard.draw_ground(c, x, y, scale);
    }

    // The gantry, nearer, spanning the road.
    let (gx0, gx1) = ink_columns(art::GANTRY);
    let g_ink_w = (gx1 - gx0 + 1) as f32;
    let g_bias = (gx0 + gx1 + 1) as f32 / 2.0 - art.gantry.width() as f32 / 2.0;
    let z = car.z + visible * 0.075;
    if let Some(p) = road.project(
        &camera, car.z, car.x * road.width() / 2.0, z, W as f32, H as f32,
    ) {
        let scale = p.half_width * 2.6 / g_ink_w;
        art.gantry.draw_ground(c, p.x - g_bias * scale, p.y, scale);
    }

    println!("    gantry ink      cols {gx0}..={gx1} ({g_ink_w} wide)");
    println!("\n    gantry scaled by the ROAD (2.6 half-widths of span)");
    println!("    billboard scaled by ITSELF (~1 half-width tall), set 1.55 out\n");
}

/// The first and last rows of a grid that contain any ink.
fn ink_rows(rows: &[&str]) -> (usize, usize) {
    let mut lo = rows.len();
    let mut hi = 0usize;
    for (i, row) in rows.iter().enumerate() {
        if row.chars().any(|c| c != '.') {
            lo = lo.min(i);
            hi = hi.max(i);
        }
    }
    (lo, hi)
}

/// The gantry exactly as Brian drew it, 43 rows of leg. Kept here so the
/// cut version can be compared against it in the scene rather than flat.
const GANTRY_AS_DRAWN: &[&str] = &[
    ".................EEEEEEEFFFFFFFFFFFFFTTKKTTKKTTKKAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAKKTTKKTTKKTTFFFFFFFFFFFFFEEEEEEE.................",
    ".................EC...EEFFF........FFTTKKTTKKTTKKFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFKKTTKKTTKKTTFF........FFFEE...CE.................",
    ".................E.C.E.EFIFIIIIIIIFFFKKTTKKTTKKTTFAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFTTKKTTKKTTKKFFFIIIIIIIFIFE.E.C.E.................",
    ".................E..E..EFI.F.....FFIFKKTTKKTTKKTTFAAAAAFFFFFFAAAAFFFFFFFFAAAAAFFFFFAAAAAFFFFFFFAAAFFFFFFFFAAAAFTTKKTTKKTTKKFIFF.....F.IFE..E..E.................",
    ".................E.E.C.EFI..F...FF.IFTTKKTTKKTTKKFAAAAAFAAAAFFAAAFFFFFFFFAAAAAFAAFFAAAAAFAAAAFFAAAFFFFFFFFAAAAFKKTTKKTTKKTTFI.FF...F..IFE.C.E.E.................",
    ".................EE...CEFI...F.FFII.FTTKKTTKKTTKKFAAAAAFAAAAAFAAAAAAFFAAAAAAAAFAAAFAAAAAFAAAAAFAAAAAAFFAAAAAAAFKKTTKKTTKKTTF.IIFF.F...IFEC...EE.................",
    ".................EEC..CEFI...FFFII..FKKTTKKTTKKTTFAAAAAFFAAAAAAAAAAAFFAAAAAAAFFAAAFAAAAAFAAAAAFAAAAAAFFAAAAAAAFTTKKTTKKTTKKF..IIFFF...IFEC..CEE.................",
    ".................EEEEEEEFI..FF.FI...FKKTTKKTTKKTTFAAAAAFFFFFFFAAAAAAFFAAAAAAAFFFFFFAAAAAFFFFFFFAAAAAAFFAAAAAAAFTTKKTTKKTTKKF...IF.FF..IFEEEEEEE.................",
    ".................EC...EEFI.FF..IF...FTTKKTTKKTTKKFAAAAAAAAAAAFAAAAAAFFAAAAAAFFFFFFFFAAAAFAAAFFAAAAAAAFFAAAAAAAFKKTTKKTTKKTTF...FI..FF.IFEE...CE.................",
    ".................E.C.E.EFI.F..II.F..FTTKKTTKKTTKKFAAAAAFFAAAAFAAAAAAFFAAAAAAFFFFFFFFAAAAFAAAAFAAAAAAAFFAAAAAAAFKKTTKKTTKKTTF..F.II..F.IFE.E.C.E.................",
    ".................E..E..EFIF..II...F.FKKTTKKTTKKTTFAAAAAFFFFFFFAAAAAAFFAAAAAFFAAAAAAFFAAAFAAAAFFAAAAAAFFAAAAAAAFTTKKTTKKTTKKF.F...II..FIFE..E..E.................",
    ".................E.E.C.EFFF.I......FFKKTTKKTTKKTTFAAAAAAFFFFFFAAAAAAFFAAAAAFFAAAAAAAFAAAFAAAAFFAAAAAAFFAAAAAAAFTTKKTTKKTTKKFF......I.FFFE.C.E.E.................",
    ".................EE...CEFFFFFFFFFFFFFTTKKTTKKTTKKFAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFKKTTKKTTKKTTFFFFFFFFFFFFFEC...EE.................",
    ".................EE....EJJIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIJJE....EE.................",
    ".................EEEEEEEJJ............................................................................................................JJEEEEEEE.................",
    ".................EC...EEJJ............................................................................................................JJEE...CE.................",
    ".................E.C.E.EJJ............................................................................................................JJE.E.C.E.................",
    ".................E..E..EJJ............................................................................................................JJE..E..E.................",
    ".................E.E.C.EJJ............................................................................................................JJE.C.E.E.................",
    ".................EE...CEJJ............................................................................................................JJEC...EE.................",
    ".................EE....EJC............................................................................................................CJE....EE.................",
    ".................EEEEEEEJJ............................................................................................................JJEEEEEEE.................",
    ".................EC...EEJJ............................................................................................................JJEE...CE.................",
    ".................ECC.E.EJJ............................................................................................................JJE.E.CCE.................",
    ".................ECCE..EJJ............................................................................................................JJE..ECCE.................",
    ".................E.E.C.EJJ............................................................................................................JJE.C.E.E.................",
    ".................EE...CEJJ............................................................................................................JJEC...EE.................",
    ".................EE....EJC............................................................................................................CJE....EE.................",
    ".................EEEEEEEJJ............................................................................................................JJEEEEEEE.................",
    ".................EC...EEJJ............................................................................................................JJEE...CE.................",
    ".................ECC.E.EJJ............................................................................................................JJE.E.CCE.................",
    ".................E..E..EJJ............................................................................................................JJE..E..E.................",
    ".................E.E.C.EJJ............................................................................................................JJE.C.E.E.................",
    ".................EE...CEJJ............................................................................................................CJEC...EE.................",
    ".................EEEEEEEJC............................................................................................................JJEEEEEEE.................",
    ".................EC...EEJJ............................................................................................................JJEE...CE.................",
    ".................ECC.E.EJJ............................................................................................................JJE.E.CCE.................",
    ".................EC.E..EJJ............................................................................................................JJE..E.CE.................",
    ".................E.E.C.EJJ............................................................................................................JJE.C.E.E.................",
    ".................EE...CEJJ............................................................................................................JJEC...EE.................",
    ".................EEEEEEEJC............................................................................................................CJEEEEEEE.................",
    ".................EC...EEJJ............................................................................................................JJEE...CE.................",
    ".................ECC.E.EJJ............................................................................................................JJE.E.CCE.................",
    ".................E..E..EJJ............................................................................................................JJE..E..E.................",
    ".................E.E.C.EJJ............................................................................................................JJE.C.E.E.................",
    ".................EE...CEJJ............................................................................................................JJEC...EE.................",
    ".................EEEEEEEJC............................................................................................................CJEEEEEEE.................",
    ".................ECC..EEJJ............................................................................................................JJEE...CE.................",
    ".................ECCCE.EJJ............................................................................................................JJEEE.C.E.................",
    ".................E.EE..EJJ............................................................................................................JJE..E..E.................",
    ".................E.E.C.EJJ............................................................................................................JJE.C.E.E.................",
    ".................EE...CEJJ............................................................................................................JJEC...EE.................",
    ".................EEEEEEEJJ............................................................................................................JJEEEEEEE.................",
    ".................EC....EJJ............................................................................................................JJE.....E.................",
    ".................EC....EJJ............................................................................................................JJE.....E.................",
    ".................EC....EJJ............................................................................................................JJE.....E.................",
    ".................E.....EJ..............................................................................................................JE.....E.................",
    "................................................................................................................................................................",
    "................................................................................................................................................................",
    "................................................................................................................................................................",
];

/// The car and the gantry together, so PROPORTION can be judged.
///
/// The sprite sheet cannot answer this. It draws art on a neutral ground
/// at whatever scale fits the panel, which says nothing about how big a
/// thing is relative to the car that drives under it — and reading height
/// off that view is how "the gantry is too tall" got decided without the
/// scene ever being consulted.
///
/// Two panels, same road, same distance, same car. Only the leg differs.
fn draw_proportion(c: &mut Canvas<'_>, art: &Art, theme: &Theme) {
    let road = Road::straight(400);
    let tuning = Tuning::from_corner(&road, 1.5);
    let mut car = Drive::new();
    let dt = 1.0 / 120.0;
    for _ in 0..(6.0 / dt) as usize {
        let correction = (-car.x * 3.0).clamp(-1.0, 1.0);
        car.update(dt, 1.0, 0.0, correction, &road, &tuning);
    }
    car.z = road.segment_length() * 3.0;

    let camera = Camera::for_road(&road, 0.85);
    let visible = road.draw_distance() as f32 * road.segment_length();
    let pw = W / 2;

    let options: [(&str, &[&str]); 2] = [
        ("as drawn, 43 leg rows", GANTRY_AS_DRAWN),
        ("cut to 18 leg rows", art::GANTRY),
    ];

    println!("\n  the car and the gantry, same scene, same distance\n");

    for (i, (label, rows)) in options.iter().enumerate() {
        let x0 = i as u32 * pw;
        render::draw_road_into(
            c, art, theme, &road, &tuning, &car, 0.0, &[], x0, 0, pw, H,
        );

        let sprite = Sprite::new(rows, &art::gantry_palette(theme));
        let (ix0, ix1) = ink_columns(rows);
        let ink_w = (ix1 - ix0 + 1) as f32;
        let bias = (ix0 + ix1 + 1) as f32 / 2.0 - sprite.width() as f32 / 2.0;

        // Close enough that the car and the structure are in the same
        // picture at a size you can compare. Further out and both are
        // specks; the question is about their RATIO, so both have to be
        // big enough to have a shape.
        let z = car.z + visible * 0.055;
        if let Some(p) = road.project(
            &camera, car.z, car.x * road.width() / 2.0, z, pw as f32, H as f32,
        ) {
            let scale = p.half_width * 2.6 / ink_w;
            sprite.draw_ground(c, x0 as f32 + p.x - bias * scale, p.y, scale);

            // How tall it comes out, in car-heights — the number the
            // picture is showing.
            let (iy0, iy1) = ink_rows(rows);
            let g_px = (iy1 - iy0 + 1) as f32 * scale;
            let car_px = art.player.height() as f32
                * (p.half_width / 70.0);
            println!("    {label:<24} {:.0}px tall here, about {:.1}x the car",
                g_px, g_px / car_px.max(1.0));
        }
    }

    c.fill_rect(pw as i32 - 1, 0, 2, H, theme.foreground);
    println!("\n    left: as drawn · right: cut. Same road, same distance, same car.\n");
}

/// The real course, at several points around the lap.
///
/// Not a debug arrangement: this renders the SHIPPED placements on the
/// SHIPPED track through the game's own renderer, so what it shows is
/// what is going to be driven. The `structures` scene next to it places
/// things by hand to judge the art; this one judges the PLACEMENT.
fn draw_lap(c: &mut Canvas<'_>, art: &Art, theme: &Theme) {
    let road = track::grand_prix().build();
    let tuning = Tuning::from_corner(&road, 1.5);
    let mile = track::UNITS_PER_MILE;

    // Four points chosen to show what the lap contains: the start line
    // itself, the first pair of billboards, the back straight pair, and
    // the approach to the Hard right.
    // ⚠️ Stops picked in REACHES, not miles. The car sees 24,000 units —
    // a twentieth of a mile — so "mile 0.10" and "mile 0.12" are two
    // completely different, non-overlapping views, and a stop chosen in
    // miles lands nowhere near what it was meant to show.
    // ⚠️ Stops picked so a structure is NEAR, and that is a much tighter
    // window than it sounds. The projection is hyperbolic: a thing at half
    // the draw distance is not half the size, it is a fiftieth. At 0.5
    // reach a billboard is three pixels of half-width. Only the nearest
    // few percent of the visible road holds anything big enough to judge,
    // so each stop sits just BEHIND a placement rather than a long way
    // back from it.
    let reach = road.draw_distance() as f32 * road.segment_length();
    let placements = structures::shipped();
    let stops = [
        // Exactly where the game starts you, so the top-left panel is the
        // first frame of a run rather than an arrangement near it.
        ("the starting grid", structures::GRID_SETBACK * reach),
        ("first billboard", placements[1].z - 0.05 * reach),
        ("second billboard", placements[2].z - 0.05 * reach),
        ("back straight", placements[4].z - 0.05 * reach),
    ];
    let _ = mile;

    let pw = W / 2;
    let ph = H / 2;

    println!("\n  the shipped placements on the shipped course\n");

    for (i, (label, z)) in stops.iter().enumerate() {
        let x0 = (i as u32 % 2) * pw;
        let y0 = (i as u32 / 2) * ph;

        let mut car = Drive::new();
        car.speed = tuning.top_speed * 0.8;
        car.z = road.wrap(*z);

        render::draw_road_into(
            c, art, theme, &road, &tuning, &car, 0.0, &[], x0, y0, pw, ph,
        );
        println!("    {label:<22} mile {:.3}  (reach {:.1})", z / mile, z / reach);
    }

    c.fill_rect(pw as i32 - 1, 0, 2, H, theme.foreground);
    c.fill_rect(0, ph as i32 - 1, W, 2, theme.foreground);
    println!("\n    top-left THE STARTING GRID · top-right first billboards");
    println!("    bottom-left down the straight · bottom-right back straight pair\n");
}
