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

#[path = "../src/art.rs"]
mod art;
#[path = "../src/effects.rs"]
mod effects;
#[path = "../src/enemy.rs"]
mod enemy;
#[path = "../src/humanoid.rs"]
mod humanoid;
#[path = "../src/lives.rs"]
mod lives;
#[path = "../src/shot.rs"]
mod shot;
#[path = "../src/flight.rs"]
mod flight;
#[path = "../src/render.rs"]
mod render;
#[path = "../src/scanner.rs"]
mod scanner;
#[path = "../src/world.rs"]
mod world;

use effects::Effects;
use enemy::Landers;
use humanoid::Humanoids;
use lives::Lives;
use flight::{Camera, Facing, Input, Ship};
use shot::Shots;
use world::Terrain;

const W: u32 = 960;
const H: u32 = 720;

fn main() {
    let mut args = std::env::args().skip(1);
    let scene = args.next().unwrap_or_else(|| "rest".to_string());
    let path = args.next().unwrap_or_else(|| "out.png".to_string());

    let mut terrain = Terrain::generate(1024, 0x0DEF_E4DE);
    let mut ship = Ship::new(0.0);
    let mut camera = Camera::new(ship.x);
    // S5's entities. Most scenes are about the flight model and leave
    // these empty; the `combat` scene fills them.
    let mut shots = Shots::new();
    let mut landers = Landers::new();
    let mut effects = Effects::new();
    let mut people = Humanoids::new();
    let lives = Lives::new();

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

        // ★ S7: Mutants hunting, firing at angles, and a life lost.
        "mutants" => {
            camera.snap_to(&ship);
            let base = ship.x;

            for (dx, dy) in [(240.0f32, 60.0f32), (420.0, -90.0), (600.0, 140.0)] {
                let x = world::wrap(base + dx);
                landers.spawn(enemy::Lander::mutant(x, ship.y + dy));
            }
            // One Lander still doing its job, for contrast.
            let lx = world::wrap(base + 780.0);
            let mut l = enemy::Lander::new(lx, terrain.height_at(lx) + enemy::HOVER_HEIGHT, 46.0);
            l.phase = enemy::Phase::Hovering;
            landers.spawn(l);

            // Their fire, at angles, on its way in.
            let mut noise = 0xC0FF_EE01u32;
            for i in 0..3 {
                if let Some(m) = landers.get(i) {
                    let (dx, dy) = m.aim_at(ship.x, ship.y, &mut noise);
                    shots.fire_enemy(m.x, m.y, dx, dy);
                }
            }
            shots.step(0.14);

            // And one of yours going the other way.
            shots.fire(ship.x + 60.0, ship.y, 1.0);
            shots.step(0.03);

            let px = world::wrap(base + 150.0);
            people.spawn(humanoid::Humanoid::new(px, terrain.height_at(px), 18.0));
        }

        // ★ S7: the world after it has ended. No mountains, all Mutants.
        "apocalypse" => {
            camera.snap_to(&ship);
            let base = ship.x;
            terrain.destroy();

            for (dx, dy) in [(180.0f32, 120.0f32), (360.0, -60.0), (560.0, 200.0), (700.0, 20.0)] {
                let x = world::wrap(base + dx);
                landers.spawn(enemy::Lander::mutant(x, ship.y + dy));
            }
            let mut noise = 0xBEEF_0007u32;
            for i in 0..4 {
                if let Some(m) = landers.get(i) {
                    let (dx, dy) = m.aim_at(ship.x, ship.y, &mut noise);
                    shots.fire_enemy(m.x, m.y, dx, dy);
                }
            }
            shots.step(0.18);
            effects.explode_world(&terrain, camera.x);
            effects.update(0.55);
        }

        // ★ S6: the abduction, mid-act. A Lander with its beam on and a
        // person being lifted, another walking, one falling, and one
        // riding under the ship after a catch.
        "rescue" => {
            camera.snap_to(&ship);
            let base = ship.x;

            // Walkers on the surface.
            for dx in [120.0f32, 640.0] {
                let x = world::wrap(base + dx);
                people.spawn(humanoid::Humanoid::new(x, terrain.height_at(x), 18.0));
            }

            // One being taken: Lander overhead, beam on, victim rising.
            let vx = world::wrap(base + 330.0);
            let vground = terrain.height_at(vx);
            people.spawn(humanoid::Humanoid::new(vx, vground + 46.0, 0.0));
            people.get_mut(2).unwrap().grabbed();

            let mut carrier = enemy::Lander::new(vx, vground + 46.0 + enemy::GRAB_HEIGHT, 0.0);
            carrier.phase = enemy::Phase::Grabbing;
            carrier.elapsed = enemy::GRAB_SECONDS * 0.6;
            carrier.target = Some(2);
            landers.spawn(carrier);

            // One falling, having been shot free.
            let fx = world::wrap(base + 470.0);
            let mut faller = humanoid::Humanoid::new(fx, terrain.height_at(fx) + 260.0, 0.0);
            faller.grabbed();
            faller.dropped();
            people.spawn(faller);

            // And one already rescued, riding under the ship.
            let mut saved = humanoid::Humanoid::new(ship.x, ship.y, 0.0);
            saved.grabbed();
            saved.dropped();
            saved.rescued();
            people.spawn(saved);
            people.carry_with_ship(ship.x, ship.y);

            // A Lander watching from further out.
            let ox = world::wrap(base + 700.0);
            let mut idle = enemy::Lander::new(ox, terrain.height_at(ox) + enemy::HOVER_HEIGHT, 46.0);
            idle.phase = enemy::Phase::Hovering;
            landers.spawn(idle);
        }

        // ★ S5, ALL OF IT AT ONCE: Landers in the air, bolts in flight,
        // and an explosion part-way through. Posed deliberately so one
        // frame can be judged — the real game never shows all of this
        // arranged this conveniently.
        "combat" => {
            camera.snap_to(&ship);
            let base = ship.x;
            // Three Landers ahead, at the height they hover.
            for (i, dx) in [180.0f32, 330.0, 470.0].iter().enumerate() {
                let x = world::wrap(base + dx);
                let y = terrain.height_at(x) + enemy::HOVER_HEIGHT;
                let mut l = enemy::Lander::new(x, y, if i == 1 { -46.0 } else { 46.0 });
                // Push two past the warp so they are drawn at full size.
                if i != 2 {
                    l.phase = enemy::Phase::Hovering;
                    l.elapsed = 0.0;
                } else {
                    l.elapsed = enemy::WARP_SECONDS * 0.45;
                }
                landers.spawn(l);
            }
            // Bolts on their way.
            shots.fire(base + 70.0, ship.y, 1.0);
            shots.step(shot::FIRE_INTERVAL);
            shots.fire(base + 40.0, ship.y, 1.0);
            shots.step(0.02);

            // And one Lander already dying, mid-explosion.
            let ex = world::wrap(base + 600.0);
            let ey = terrain.height_at(ex) + enemy::HOVER_HEIGHT;
            effects.explode_lander(ex, ey, 46.0);
            effects.update(0.14);
        }

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
        let scene = render::Scene {
            terrain: &terrain,
            ship: &ship,
            camera: &camera,
            shots: &shots,
            landers: &landers,
            people: &people,
            effects: &effects,
            lives: &lives,
            score: 0,
            // ★ A FIXED, NON-ZERO CLOCK. The scanner pulses its Mutants
            // on this; at 0.0 every dump would freeze the pulse at one
            // arbitrary phase. This value puts it near its peak, so a
            // screenshot shows the Mutant blips at their brightest —
            // which is the state worth checking they are visible in.
            time: 0.37,
        };
        render::draw(&mut canvas, &scene, &Theme::load());
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
