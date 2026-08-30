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
//!
//! Never screenshot a window for this — the render is deterministic and
//! the window is not.

#[path = "../src/art.rs"]
mod art;
#[path = "../src/road.rs"]
mod road;
#[path = "../src/drive.rs"]
mod drive;

use std::io::Write;

use art::Art;
use omarcade_core::{Canvas, Color, Pose, Theme};
use drive::{Drive, Tuning};
use road::{Camera, Road, Segment};

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
            "curve" => draw_road(&mut c, &art, &theme, bendy_track()),
            "lean" => draw_lean(&mut c, &art, &theme),
            "roll" => draw_roll(&mut c, &art, &theme),
            "drive" => draw_drive(&mut c, &art, &theme),
            other => {
                eprintln!("unknown scene {other:?} — try: sheet | road | curve | lean | roll | drive");
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
fn bendy_track() -> Road {
    let mut segs = Vec::new();
    // Only the first ~15 segments occupy real screen height; past that a
    // band is under a pixel tall (measured). So the bend has to start
    // close, or it renders into a sub-pixel sliver and looks straight.
    segs.extend(std::iter::repeat_n(Segment::STRAIGHT, 4));
    // Eased in and out, because a curve that starts at full strength
    // reads as a kink. This is the shape a real track section has.
    for i in 0..6 {
        segs.push(Segment::curving(90.0 * (i as f32 / 6.0)));
    }
    segs.extend(std::iter::repeat_n(Segment::curving(90.0), 40));
    for i in 0..6 {
        segs.push(Segment::curving(90.0 * (1.0 - i as f32 / 6.0)));
    }
    // A lap, not a loop: at A's top speed 400 segments is a two-second
    // lap. Sized for driving rather than for looking at.
    segs.extend(std::iter::repeat_n(Segment::STRAIGHT, 3_944));
    Road::new(segs, 200.0, 2200.0)
}

/// The cars on a real road, which is the only view that says whether
/// they read at speed.
///
/// Every coordinate here now comes from `road.rs` — this function decides
/// only what colour things are. That is the point of the split: if the
/// road looks wrong, the projection is wrong, and it is wrong somewhere
/// that has tests.
fn draw_road(c: &mut Canvas<'_>, art: &Art, theme: &Theme, road: Road) {
    let sky = theme.background.lerp(theme.blue, 0.30);
    let grass_a = theme.background.lerp(theme.green, 0.40);
    let grass_b = theme.background.lerp(theme.green, 0.28);
    let road_a = theme.dark_background.lerp(theme.foreground, 0.15);
    let road_b = theme.dark_background.lerp(theme.foreground, 0.11);
    let rumble_a = theme.red.lerp(Color::WHITE, 0.15);
    let rumble_b = theme.foreground.lerp(Color::WHITE, 0.4);
    let line = theme.foreground.lerp(Color::WHITE, 0.5);

    let camera = Camera::default();
    // A little way in, so the eased curve is ahead of us in the `curve`
    // scene rather than under the bumper.
    let camera_z = 3_000.0;
    let x_offset = 0.0;

    // The horizon is where the projection puts it, not a constant. The
    // sketch had its own HORIZON and the two could drift apart.
    let horizon = H as f32 / 2.0;

    for y in 0..horizon as u32 {
        let t = y as f32 / horizon;
        c.fill_rect(0, y as i32, W, 1, sky.lerp(theme.background, 1.0 - t * 0.7));
    }
    // Grass fills everything below the horizon; road is painted over it.
    c.fill_rect(0, horizon as i32, W, H - horizon as u32, grass_b);

    let bands = road.visible(&camera, camera_z, x_offset, W as f32, H as f32);

    // Painted far-to-near, and INTERPOLATED ACROSS EACH BAND rather than
    // filled as one rect.
    //
    // Near the camera a single segment is ~180px tall (measured), so one
    // rect per band is the "banded" approach the road bench warned about:
    // cheapest, but the edges stair-step in slabs. Walking scanlines
    // inside the band and lerping the edge across it is the classic
    // scanline road, and the bench put it at 0.51ms — 3% of a frame.
    for pair in bands.windows(2).rev() {
        let (near, far) = (pair[0], pair[1]);
        let y0 = far.y.max(horizon);
        let y1 = near.y.min(H as f32);
        if y1 <= y0 {
            continue;
        }

        let span = near.y - far.y;
        let mut y = y0;
        while y < y1 {
            let h = (1.0f32).min(y1 - y);
            // Where in the band this scanline sits: 0 at the far edge,
            // 1 at the near edge.
            let t = if span > 0.001 { ((y - far.y) / span).clamp(0.0, 1.0) } else { 1.0 };
            let cx = far.x + (near.x - far.x) * t;
            let hw = far.half_width + (near.half_width - far.half_width) * t;
            let dist = far.distance + (near.distance - far.distance) * t;

            // Bands alternate by TRACK position, not by screen row — that
            // is what makes them bunch toward the horizon like real ground
            // instead of striping the screen evenly.
            let seg = (dist + camera_z) / road.segment_length();
            let phase = (seg as u32) % 2 == 0;

            c.fill_rect_f(0.0, y, W as f32, h, if phase { grass_a } else { grass_b });
            c.fill_rect_f(cx - hw, y, hw * 2.0, h, if phase { road_a } else { road_b });

            // Rumble strips, as a fraction of the road — a ratio, so they
            // stay proportionate at every distance.
            let rumble = (hw * 0.13).max(0.7);
            let rc = if phase { rumble_a } else { rumble_b };
            c.fill_rect_f(cx - hw, y, rumble, h, rc);
            c.fill_rect_f(cx + hw - rumble, y, rumble, h, rc);

            if phase {
                let lw = (hw * 0.035).max(0.5);
                c.fill_rect_f(cx - lw / 2.0, y, lw, h, line);
            }

            // Distance haze, keyed off real distance rather than screen row.
            let ht = (dist / 60_000.0).clamp(0.0, 1.0);
            let a = (ht.powf(0.9) * 210.0) as u8;
            if a > 2 {
                c.fill_rect_f(0.0, y, W as f32, h, sky.with_alpha(a));
            }
            y += h;
        }
    }

    // Roadside posts, placed at track positions so they stream past
    // rather than sitting in an even ladder.
    let first_post = ((camera_z / 2000.0).floor() + 1.0) * 2000.0;
    for i in 0..30 {
        let z = first_post + i as f32 * 2000.0;
        let Some(p) = road.project(&camera, camera_z, x_offset, z, W as f32, H as f32)
        else { continue };
        if p.y <= horizon + 1.0 {
            break;
        }
        // Scale follows the road, so a post is the same real size always.
        let s = p.half_width / 105.0 * 1.5;
        let haze = (p.distance / 60_000.0).clamp(0.0, 1.0) * 0.8;
        for side in [-1.0f32, 1.0] {
            let px = p.x + side * p.half_width * 1.28;
            let w = art.post.width() as f32 * s;
            let h = art.post.height() as f32 * s;
            art.post.draw_tinted(c, px - w / 2.0, p.y - h, s, Some((sky, haze)));
        }
    }

    // Rivals, placed by track position and lane, far-to-near so nearer
    // cars occlude further ones.
    let rivals: [(f32, f32); 4] = [
        (camera_z + 26_000.0, -0.55),
        (camera_z + 15_000.0, 0.50),
        (camera_z + 8_000.0, -0.25),
        (camera_z + 4_200.0, 0.42),
    ];
    for (i, (z, lane)) in rivals.iter().enumerate().rev() {
        let Some(p) = road.project(&camera, camera_z, x_offset, *z, W as f32, H as f32)
        else { continue };
        // Sprite scale follows road width, so a car always covers the
        // same fraction of the lane at any distance.
        let s = p.half_width / 105.0;
        let haze = (p.distance / 60_000.0).clamp(0.0, 1.0) * 0.8;
        let rival = art.rival(i);
        let w = rival.width() as f32 * s;
        let h = rival.height() as f32 * s;
        rival.draw_tinted(c, p.x + lane * p.half_width - w / 2.0, p.y - h, s,
                          Some((sky, haze)));
    }

    // The player, where the camera actually sits: dead centre, at the
    // bottom, at the scale the road has right under the bumper.
    art.player.draw_ground(c, W as f32 / 2.0, 706.0, 7.0);
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

    let road = bendy_track();
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
            car.update(DT, 1.0, correction, &road, tuning);
        }
        car.z = 0.0;

        let mut elapsed = 0.0f32;
        for (panel, mark) in marks.iter().enumerate() {
            // Drive to this mark on the track, timing how long it takes.
            while car.z < *mark && elapsed < 60.0 {
                let correction = (-car.x * 3.0).clamp(-1.0, 1.0);
                car.update(DT, 1.0, correction, &road, tuning);
                elapsed += DT;
            }
            times[row][panel] = elapsed;

            let x0 = panel as u32 * pw;
            let y0 = row as u32 * ph;
            draw_road_into(c, art, theme, &road, tuning, &car, x0, y0, pw, ph);

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

/// `draw_road` for an arbitrary sub-rectangle, driven by a live car.
///
/// The full-screen scene is the special case of this; keeping one
/// implementation is what stops the filmstrip and the real view drifting
/// apart and showing different things.
#[allow(clippy::too_many_arguments)]
fn draw_road_into(
    c: &mut Canvas<'_>,
    art: &Art,
    theme: &Theme,
    road: &Road,
    tuning: &Tuning,
    car: &Drive,
    ox: u32,
    oy: u32,
    w: u32,
    h: u32,
) {
    let sky = theme.background.lerp(theme.blue, 0.30);
    let grass_a = theme.background.lerp(theme.green, 0.40);
    let grass_b = theme.background.lerp(theme.green, 0.28);
    let road_a = theme.dark_background.lerp(theme.foreground, 0.15);
    let road_b = theme.dark_background.lerp(theme.foreground, 0.11);
    let rumble_a = theme.red.lerp(Color::WHITE, 0.15);
    let rumble_b = theme.foreground.lerp(Color::WHITE, 0.4);
    let line = theme.foreground.lerp(Color::WHITE, 0.5);

    let camera = Camera::for_road(road, 0.85);
    let fx = ox as f32;
    let fy = oy as f32;
    let fw = w as f32;
    let fh = h as f32;
    let horizon = fh / 2.0;

    let x_offset = car.x_offset(road);

    for y in 0..horizon as u32 {
        let t = y as f32 / horizon;
        c.fill_rect(ox as i32, (oy + y) as i32, w, 1, sky.lerp(theme.background, 1.0 - t * 0.7));
    }
    c.fill_rect(ox as i32, (oy + horizon as u32) as i32, w, h - horizon as u32, grass_b);

    let bands = road.visible(&camera, car.z, x_offset, fw, fh);

    for pair in bands.windows(2).rev() {
        let (near, far) = (pair[0], pair[1]);
        let y0 = far.y.max(horizon);
        let y1 = near.y.min(fh);
        if y1 <= y0 {
            continue;
        }
        let span = near.y - far.y;
        let mut y = y0;
        while y < y1 {
            let bh = (1.0f32).min(y1 - y);
            let t = if span > 0.001 { ((y - far.y) / span).clamp(0.0, 1.0) } else { 1.0 };
            let cx = fx + far.x + (near.x - far.x) * t;
            let hw = far.half_width + (near.half_width - far.half_width) * t;
            let dist = far.distance + (near.distance - far.distance) * t;

            let seg = (dist + car.z) / road.segment_length();
            let phase = (seg as u32) % 2 == 0;
            let sy = fy + y;

            c.fill_rect_f(fx, sy, fw, bh, if phase { grass_a } else { grass_b });
            c.fill_rect_f(cx - hw, sy, hw * 2.0, bh, if phase { road_a } else { road_b });

            let rumble = (hw * 0.13).max(0.7);
            let rc = if phase { rumble_a } else { rumble_b };
            c.fill_rect_f(cx - hw, sy, rumble, bh, rc);
            c.fill_rect_f(cx + hw - rumble, sy, rumble, bh, rc);

            if phase {
                let lw = (hw * 0.035).max(0.5);
                c.fill_rect_f(cx - lw / 2.0, sy, lw, bh, line);
            }

            let ht = (dist / 60_000.0).clamp(0.0, 1.0);
            let a = (ht.powf(0.9) * 210.0) as u8;
            if a > 2 {
                c.fill_rect_f(fx, sy, fw, bh, sky.with_alpha(a));
            }
            y += bh;
        }
    }

    // The player, leaning into whatever bend it is actually in.
    //
    // Scale follows the ROAD's width under the bumper, not the panel
    // height. Tying it to height made the car swamp a narrow panel while
    // the road (which comes from panel width) stayed thin — the two
    // disagreed about how big the world was.
    // Scale from the road at a FIXED distance ahead, never from
    // `bands.first()`: the nearest band changes identity as the camera
    // crosses a segment boundary — whole segment one frame, sliver the
    // next — so anything sized from it pulses. A fixed probe distance is
    // continuous, which is what a sprite scale has to be.
    let pose = Pose::cornering(car.cornering(road, tuning));
    let probe = road
        .project(&camera, car.z, x_offset, car.z + road.segment_length(), fw, fh)
        .map(|p| p.half_width)
        .unwrap_or(fw * 0.4);
    let scale = probe / 105.0 * 1.5;
    art.player.draw_ground_posed(
        c,
        fx + fw / 2.0,
        fy + fh * 0.98,
        scale,
        pose,
        None,
    );
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
