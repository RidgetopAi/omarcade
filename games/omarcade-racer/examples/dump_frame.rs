//! Render a real playing frame straight to a PNG, with no window.
//!
//! ⚠️ THIS IS NOT `dump_art`. They answer different questions and the
//! difference matters:
//!
//! - `dump_art` draws CONTACT SHEETS — eight variants of a car in a 4x2
//!   grid, a size ladder, every sign side by side. It is a developer's
//!   diagnostic, calibrated against `render::demo_track()`, which is one
//!   bend sized to be LOOKED at.
//! - This draws ONE FRAME AS THE PLAYER SEES IT, on the real grand-prix
//!   course, with the HUD over it. It is what a screenshot would be if
//!   screenshots were deterministic.
//!
//! The racer had only the first, which is why the cabinet could not show
//! a picture of it: a contact sheet is not cabinet art, and the README
//! image cannot be a diagnostic grid either.
//!
//!   cargo run -p omarcade-racer --example dump_frame -- <scene> <out.png> [w] [h]
//!
//! ⚠️ ARGUMENT ORDER IS SCENE THEN PATH, matching pixel-break and volley.
//! `dump_art` takes them the other way round — a real trip hazard, and
//! the reason this one follows the majority.
//!
//! Scenes:
//!   drive    the car at speed on a straight, traffic ahead     ← cabinet art
//!   race     mid-race, the HUD carrying a lap and a score
//!   qualify  the qualifying lap, clock running
//!   grid     the countdown, lights over the grid
//!   crash    a fireball, through the real collision path
//!   clean    `drive` with NO HUD, for use as a backdrop
//!
//! ⚠️ The sky comes from the live Omarchy wallpaper via `backdrop.rs`.
//! Set OMARCADE_BACKGROUND=<path> to render against a different one.

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
#[path = "../src/hud.rs"]
mod hud;
#[path = "../src/race.rs"]
mod race;
#[path = "../src/pace.rs"]
mod pace;
#[path = "../src/score.rs"]
mod score;

use std::io::Write;

use art::Art;
use drive::{Drive, Tuning};
use hud::{Flash, Scoreboard};
use omarcade_core::{Canvas, Theme};
use race::{Phase, Race, Session, Windows};

const W: u32 = 960;
const H: u32 = 720;

/// The reaction window `main.rs` tunes the car with. Kept in step with
/// it deliberately: a frame drawn with different handling is not a
/// picture of the shipped game.
const REACTION_SECONDS: f32 = 1.5;
const TRAFFIC_CARS: usize = 8;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: dump_frame <scene> <out.png> [w] [h]");
        eprintln!("scenes: drive | race | qualify | grid | crash | clean");
        std::process::exit(2);
    }

    let scene = args[1].as_str();
    let out = &args[2];
    let w: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(W);
    let h: u32 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(H);

    let theme = Theme::load();
    let art = Art::load(&theme);

    // The real course and the real handling, exactly as `Racer::new`
    // builds them. A frame drawn from `demo_track()` would be a picture
    // of the test fixture, not of the game.
    let road = track::grand_prix().build();
    let tuning = Tuning::from_corner(&road, REACTION_SECONDS);
    let visible = road.draw_distance() as f32 * road.segment_length();
    let start_z = road.wrap(structures::GRID_SETBACK * visible);
    let windows = Windows::derive(&road, &tuning, start_z);

    let mut traffic = traffic::Field::grid(&road, TRAFFIC_CARS);
    let mut car = Drive::new();
    let mut race = Race::new(windows, &road, start_z, TRAFFIC_CARS + 1);
    let mut board = Scoreboard::default();
    let mut flash: Option<Flash> = None;
    let mut fire: Option<crash::Explosion> = None;
    let mut show_player = true;

    match scene {
        // A straight at speed, traffic up the road. The frame that says
        // "this is a driving game" in one look — which is what a cabinet
        // screen and a README image both need.
        "drive" | "clean" => {
            car.z = road.wrap(start_z + visible * 0.35);
            car.speed = tuning.top_speed * 0.86;
            // ⚠️ SHORT SETTLE ON PURPOSE. The rivals start in grid
            // formation and spread as they drive; two seconds put them
            // over the horizon and the first render of this scene had an
            // empty road, which is the one thing a racing game's cabinet
            // art must not show. Half a second breaks the formation up
            // without losing them.
            settle(&mut traffic, &road, &tuning, 0.5);
            race.phase = Phase::Racing { lap: 1 };
            board.score = 41_200;
            board.best = Some(52_800);
        }
        "race" => {
            car.z = road.wrap(start_z + visible * 1.6);
            car.speed = tuning.top_speed * 0.78;
            car.x = -0.28;
            settle(&mut traffic, &road, &tuning, 4.0);
            race.phase = Phase::Racing { lap: 2 };
            board.score = 128_450;
            board.best = Some(140_000);
            flash = Some(Flash::new("LAP 2"));
        }
        "qualify" => {
            car.z = road.wrap(start_z + visible * 0.8);
            car.speed = tuning.top_speed * 0.92;
            settle(&mut traffic, &road, &tuning, 1.0);
            race.phase = Phase::Qualifying;
            board.score = 9_600;
        }
        // The lights, with the car held on the grid.
        "grid" => {
            car.z = start_z;
            car.speed = 0.0;
            race.phase = Phase::Countdown { remaining: 1.6, then: Session::Qualifying };
        }
        // A real fireball, built the way a collision builds one, so the
        // picture cannot drift from what a crash actually looks like.
        "crash" => {
            car.z = road.wrap(start_z + visible * 0.5);
            car.speed = tuning.top_speed * 0.70;
            settle(&mut traffic, &road, &tuning, 2.0);
            race.phase = Phase::Racing { lap: 1 };
            board.score = 64_100;
            // ⚠️ HOW FAR UP THE ROAD THE FIRE SITS IS THE WHOLE SCENE.
            // Two earlier attempts put it at `car.z` and at `car.z + 26`
            // and BOTH rendered a frame with no visible fireball — the
            // first because the camera sits AT the car so the projection
            // was degenerate, the second because 26 units up a road whose
            // visible depth is thousands puts the fire ON THE HORIZON,
            // a few pixels tall and lost in the tarmac.
            //
            // ⚠️ NEITHER FAILED. Both wrote a valid PNG and printed
            // success. Only looking at the image showed the scene was
            // empty. A `dump_frame` scene cannot assert its own content,
            // so LOOK AT EVERY SCENE ONCE.
            //
            // A real collision happens within a car's length or two, so
            // the fireball fills the lower frame the way it does in play.
            let mut c = crash::Explosion::start(road.wrap(car.z + road.segment_length() * 1.5), car.x);
            // Part-burned: a fireball at its first frame is a dot, and at
            // its last it is smoke. This is the one worth looking at.
            c.advance(0.45);
            fire = Some(c);
            show_player = false;
            flash = Some(Flash::new("CRASH"));
        }
        other => {
            eprintln!("unknown scene {other:?} — try: drive | race | qualify | grid | crash | clean");
            std::process::exit(2);
        }
    }

    let mut buf = vec![0u32; (w * h) as usize];
    {
        let mut c = Canvas::new(&mut buf, w, h);
        render::draw_road_into_with(
            &mut c,
            &art,
            &theme,
            &road,
            &tuning,
            &car,
            0.0,
            &traffic.as_rendered(),
            0,
            0,
            w,
            h,
            show_player,
        );

        if let Some(f) = &fire {
            render::draw_explosion_into(
                &mut c, &art.explosion, &theme, &road, &car, f, 0, 0, w, h,
            );
        }

        // ⚠️ `clean` is the CONTROLLED PAIR (L044) for `drive`: the same
        // frame with the HUD withheld. Judging a backdrop with the clock
        // and the score sitting on it is judging two things at once.
        if scene != "clean" {
            let layout = hud::compose(&race, flash.as_ref(), &board);
            hud::draw(&mut c, &theme, &layout, w, h);
        }
    }

    if let Err(e) = write_png(out, &buf, w, h) {
        eprintln!("could not write {out}: {e}");
        std::process::exit(1);
    }
    println!("wrote {out} ({w}x{h}, scene: {scene})");
}

/// Let the traffic drive for a while so the cars are spread the way a
/// race spreads them.
///
/// Straight off `Field::grid` they sit in starting formation, which is a
/// picture of the first second of a race and of nothing else.
fn settle(field: &mut traffic::Field, road: &road::Road, tuning: &Tuning, seconds: f32) {
    let dt = 1.0 / 60.0;
    let steps = (seconds / dt) as u32;
    for _ in 0..steps {
        field.advance(dt, road, tuning);
    }
}

// ---------------------------------------------------------------------
// PNG out. Stored-mode deflate, no dependency — the same writer
// `dump_art` uses. Larger files than a real encoder, perfectly valid.
// ---------------------------------------------------------------------

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
