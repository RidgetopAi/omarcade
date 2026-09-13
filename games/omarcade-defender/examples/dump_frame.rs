//! Render one frame to a PNG, headless.
//!
//!   cargo run -p omarcade-defender --example dump_frame -- <scene> out.png
//!
//! ⚠️ SCENE THEN PATH, matching the other three games. `dump_art` in the
//! racer takes them the other way round and that inconsistency has cost
//! time before.
//!
//! ⚠️ AND A SCENE CANNOT ASSERT ITS OWN CONTENT. A frame with nothing in
//! it renders, writes a valid PNG and reports success — the racer's
//! crash scene did exactly that twice. LOOK AT EVERY SCENE ONCE.

use std::path::Path;

use omarcade_core::{Canvas, Theme};

#[path = "../src/flight.rs"]
mod flight;
#[path = "../src/render.rs"]
mod render;
#[path = "../src/world.rs"]
mod world;

use flight::{Camera, Facing, Input, Ship};
use world::Terrain;

const W: u32 = 960;
const H: u32 = 720;

fn main() {
    let mut args = std::env::args().skip(1);
    let scene = args.next().unwrap_or_else(|| "rest".to_string());
    let path = args.next().unwrap_or_else(|| "out.png".to_string());

    let terrain = Terrain::generate(1024, 0x0DEF_E4DE);
    let mut ship = Ship::new(0.0);
    let mut camera = Camera::new(ship.x);

    // Fly the ship into the state the scene names, using the real
    // physics rather than posing it by hand — a posed frame can show a
    // configuration the game cannot actually produce.
    let dt = 1.0 / 240.0;
    let mut run = |ship: &mut Ship, camera: &mut Camera, input: Input, seconds: f32| {
        for _ in 0..(seconds / dt) as usize {
            ship.step(input, &terrain, dt);
            camera.follow(ship, dt);
        }
    };

    let thrust_east = Input { thrust: true, face: Some(Facing::East), ..Default::default() };
    let thrust_west = Input { thrust: true, face: Some(Facing::West), ..Default::default() };

    match scene.as_str() {
        // Sitting still, facing east. The baseline the others are read
        // against.
        "rest" => camera.snap_to(&ship),

        // At speed, east. What most of the game looks like.
        "cruise" => run(&mut ship, &mut camera, thrust_east, 4.0),

        // At speed, west — the mirrored ship and the camera leading the
        // other way.
        "west" => run(&mut ship, &mut camera, thrust_west, 4.0),

        // ★ MID-TURNAROUND: travelling east, facing west, camera sliding
        // across. The single most important frame in this stage, because
        // it is the moment the eased reversal is visible.
        "turn" => {
            run(&mut ship, &mut camera, thrust_east, 4.0);
            run(&mut ship, &mut camera, thrust_west, 0.25);
        }

        // Low over the ridge, to check the clearance reads as flying
        // rather than clipping.
        "low" => {
            run(&mut ship, &mut camera, thrust_east, 2.5);
            run(
                &mut ship,
                &mut camera,
                Input { vertical: 1.0, ..Default::default() },
                3.0,
            );
        }

        // ⚠️ THE SEAM. The camera sits at x = 0, so the left of the
        // screen is the far east end of the world and the right is the
        // west end. If the terrain tore anywhere, it would be here.
        "seam" => {
            ship.x = 0.0;
            camera.snap_to(&ship);
            camera.x = 0.0;
        }

        other => {
            eprintln!("unknown scene: {other}");
            eprintln!("scenes: rest | cruise | west | turn | low | seam");
            std::process::exit(2);
        }
    }

    let mut buf = vec![0u32; (W * H) as usize];
    {
        let mut canvas = Canvas::new(&mut buf, W, H);
        render::draw(&mut canvas, &terrain, &ship, &camera, &Theme::load());
    }

    write_png(Path::new(&path), &buf, W, H);
    println!(
        "wrote {path} ({W}x{H}, scene: {scene})  ship x={:.0} vx={:.0} facing={:?}",
        ship.x, ship.vx, ship.facing
    );
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
