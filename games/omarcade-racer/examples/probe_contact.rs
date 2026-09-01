//! At what separation do two cars actually TOUCH on screen?
//!
//! Collision has to agree with the picture. Brian crashed into a car that
//! was visibly most of the way up the road — the threshold was 1729 world
//! units, derived as 2.5x the car's width in the road's lateral units.
//!
//! ⚠️ THAT DERIVATION WAS INVALID, and this probe exists because of it.
//! The road's WIDTH (2200 units) and the track's SEGMENT LENGTH (200
//! units) are independent authoring numbers in different visual scales.
//! "2.5 times the car's width" is a sentence about lateral units and
//! means nothing along z. A car 1729 units ahead draws at 11.6% of the
//! player's size — a small sprite well up the road, not a car you are
//! touching.
//!
//! So the number is MEASURED here instead: walk a car toward the player
//! through the real projection, and report the separation at which the
//! drawn sprites first overlap.
//!
//!   cargo run -p omarcade-racer --example probe_contact

#[path = "../src/road.rs"]
mod road;
#[path = "../src/drive.rs"]
mod drive;
#[path = "../src/track.rs"]
mod track;

use drive::{Drive, Tuning};
use omarcade_core::Theme;
use road::Camera;
use track::grand_prix;

/// Mirrors `render.rs`. If these move, this probe is measuring a scene
/// the game does not draw.
const CAMERA_FILL: f32 = 0.85;
const CAR_ART_PIXELS_PER_HALF_WIDTH: f32 = 70.0;
const CAR_INK_ROWS: f32 = 22.0;
const CAR_INK_COLS: f32 = 44.0;

const W: f32 = 960.0;
const H: f32 = 720.0;

fn main() {
    let _ = Theme::load();
    let road = grand_prix().build();
    let tuning = Tuning::from_corner(&road, 1.5);
    let camera = Camera::for_road(&road, CAMERA_FILL);

    let player = Drive {
        z: road.wrap(road.segment_length() * 40.0),
        x: 0.0,
        speed: tuning.top_speed * 0.6,
    };

    // The player's own drawn size, exactly as render.rs computes it.
    let probe = road
        .project(
            &camera,
            player.z,
            0.0,
            player.z + road.segment_length(),
            W,
            H,
        )
        .map(|p| p.half_width)
        .unwrap_or(W * 0.4);
    let player_scale = probe / CAR_ART_PIXELS_PER_HALF_WIDTH;
    let player_h = CAR_INK_ROWS * player_scale;
    let player_w = CAR_INK_COLS * player_scale;

    // The player is drawn with its BOTTOM at 0.98 of the frame.
    let player_bottom = H * 0.98;
    let player_top = player_bottom - player_h;

    println!("\n  WHERE DO TWO CARS ACTUALLY TOUCH?\n");
    println!("  the player draws {player_w:.0} x {player_h:.0} px, bottom at y={player_bottom:.0}, top at y={player_top:.0}");
    println!("  road width {:.0} units, segment {:.0} units — DIFFERENT SCALES\n", road.width(), road.segment_length());

    println!(
        "    {:>10}{:>12}{:>10}{:>12}{:>10}",
        "ahead", "segments", "draws px", "its bottom", "overlap?"
    );

    let mut touch_at = None;
    let mut d = 4000.0f32;
    while d > 20.0 {
        if let Some(p) = road.project(&camera, player.z, 0.0, player.z + d, W, H) {
            let scale = p.half_width / CAR_ART_PIXELS_PER_HALF_WIDTH;
            let h = CAR_INK_ROWS * scale;
            // A car ahead is drawn standing on the road at p.y.
            let bottom = p.y;
            let overlaps = bottom > player_top;

            if overlaps && touch_at.is_none() {
                touch_at = Some(d);
            }

            if (d as i32) % 200 == 0 || overlaps && touch_at == Some(d) {
                println!(
                    "    {d:>10.0}{:>12.1}{h:>10.0}{bottom:>12.0}{:>10}",
                    d / road.segment_length(),
                    if overlaps { "YES" } else { "" }
                );
            }
        }
        d -= 20.0;
    }

    match touch_at {
        Some(d) => {
            println!("\n  ⇒ SPRITES FIRST OVERLAP AT {d:.0} UNITS ({:.1} segments)", d / road.segment_length());
            println!("    The shipped threshold was 1729 units — {:.1}x too far.", 1729.0 / d);
            println!("    At 1729 units a car draws {:.0}% of the player's height.",
                     (CAR_INK_ROWS
                        * (road.project(&camera, player.z, 0.0, player.z + 1729.0, W, H)
                            .map(|p| p.half_width).unwrap_or(0.0)
                            / CAR_ART_PIXELS_PER_HALF_WIDTH))
                        / player_h * 100.0);
        }
        None => println!("\n  ⚠️  never overlapped — the projection or the anchors are wrong"),
    }
    println!();
}
