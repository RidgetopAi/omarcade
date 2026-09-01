//! Can you see a car before you hit it?
//!
//! Brian, driving S11: "the crash detection I think is too tight,
//! especially in turns, you can't see cars quick enough." It was not the
//! detection. The projection is hyperbolic and contact is 1.6 segments,
//! so at the old closing speed a car in your lane drew EIGHT PIXELS tall
//! one second before contact, and in a Firm bend it was off the frame
//! edge until half a second out. Two knobs answer it and this probe
//! reports both: how big a rival draws by seconds-to-contact
//! (`render::RIVAL_SCALE_EXPONENT`) and how early it enters the frame in a
//! bend (the traffic band, `traffic::CRUISE_MIN/MAX`).
//!
//! Part A drives the reference driver — blind to traffic — through three
//! laps against the real field and, at each contact, reports how long the
//! struck car had been on screen and how tall it was drawn a second and
//! half a second before. Those are the numbers a person had to react to.
//!
//!   cargo run -p omarcade-racer --example probe_warning

#[path = "../src/road.rs"]
mod road;
#[path = "../src/drive.rs"]
mod drive;
#[path = "../src/track.rs"]
mod track;
#[path = "../src/traffic.rs"]
mod traffic;
#[path = "../src/collide.rs"]
mod collide;
#[path = "../src/pace.rs"]
mod pace;
#[path = "../src/art.rs"]
mod art;
#[path = "../src/crash.rs"]
mod crash;
#[path = "../src/structures.rs"]
mod structures;
#[path = "../src/scenery.rs"]
mod scenery;
#[path = "../src/render.rs"]
mod render;

use drive::{Drive, Tuning};
use pace::Pacer;
use render::{rival_scale, CAMERA_FILL, CAR_ART_PIXELS_PER_HALF_WIDTH};
use road::{Camera, Road, Segment};
use track::grand_prix;
use traffic::{Field, CORNER_MARGIN, CRUISE_MAX, CRUISE_MIN};

const DT: f32 = 1.0 / 60.0;
const W: f32 = 960.0;
const H: f32 = 720.0;
/// The car sprite's ink, in source pixels — see `probe_contact`.
const CAR_INK_ROWS: f32 = 22.0;
const CAR_INK_COLS: f32 = 44.0;

/// The player's own drawn scale, from the road one segment ahead — the
/// renderer's rule.
fn player_scale(road: &Road, cam: &Camera, player: &Drive) -> f32 {
    let x_off = player.x * road.width() / 2.0;
    road.project(cam, player.z, x_off, player.z + road.segment_length(), W, H)
        .map(|p| p.half_width)
        .unwrap_or(W * 0.4)
        / CAR_ART_PIXELS_PER_HALF_WIDTH
}

/// `(on_screen, height_px)` of a rival as the renderer draws it: culled
/// by distance as `render` culls, and on screen if any of its width is
/// inside the frame — a bend pushes far cars off the edge.
fn seen(road: &Road, cam: &Camera, player: &Drive, rz: f32, lane: f32) -> (bool, f32) {
    let ahead = (rz - player.z).rem_euclid(road.length());
    let x_off = player.x * road.width() / 2.0;
    let Some(p) = road.project(cam, player.z, x_off, player.z + ahead, W, H) else {
        return (false, 0.0);
    };
    if p.y <= H / 2.0 + 1.0 || p.y > H + 200.0 {
        return (false, 0.0);
    }
    let s = rival_scale(p.half_width / CAR_ART_PIXELS_PER_HALF_WIDTH, player_scale(road, cam, player));
    let cx = p.x + lane * p.half_width;
    let half_w = CAR_INK_COLS * s / 2.0;
    (cx + half_w > 0.0 && cx - half_w < W, CAR_INK_ROWS * s)
}

fn main() {
    let road = grand_prix().build();
    let tuning = Tuning::from_corner(&road, 1.5);
    let cam = Camera::for_road(&road, CAMERA_FILL);
    let length = road.length();
    let visible = road.draw_distance() as f32 * road.segment_length();

    println!("\n  CAN YOU SEE A CAR BEFORE YOU HIT IT?\n");
    println!(
        "  rival scale exponent {} · cruise band {:.0}-{:.0}% · closing on a straight {:.0}-{:.0} u/s\n",
        render::RIVAL_SCALE_EXPONENT,
        CRUISE_MIN * 100.0,
        CRUISE_MAX * 100.0,
        tuning.top_speed * (1.0 - CRUISE_MAX),
        tuning.top_speed * (1.0 - CRUISE_MIN),
    );

    // A. The census.
    println!("  A. EVERY CONTACT THE BLIND REFERENCE DRIVER MAKES IN THREE LAPS\n");
    println!(
        "  {:>6} {:>6} {:>8} {:>8} {:>9} {:>8} {:>8} {:>7}",
        "t", "curve", "closing", "lat gap", "seen(s)", "px@1s", "px@0.5s", "px@hit"
    );
    let mut field = Field::grid(&road, 5);
    let mut player = Drive { z: road.wrap(-0.1 * visible), ..Drive::new() };
    let mut hist: Vec<Vec<(bool, f32)>> = vec![Vec::new(); field.cars.len()];
    let (mut t, mut travelled, mut last_z, mut burn) = (0.0f32, 0.0f32, player.z, 0.0f32);
    let mut contacts = 0;
    let mut seen_total = 0.0f32;
    let mut px_1s_total = 0.0f32;
    let mut hidden_in_bends = 0;
    while travelled < length * 3.0 && t < 900.0 {
        if burn > 0.0 {
            burn -= DT;
            player.speed = 0.0;
            field.advance(DT, &road, &tuning);
            t += DT;
            continue;
        }
        let prev_z = player.z;
        Pacer::EXACT.step(&mut player, &road, &tuning, DT);
        field.advance(DT, &road, &tuning);
        field.recycle(player.z, &road);
        for (i, c) in field.cars.iter().enumerate() {
            hist[i].push(seen(&road, &cam, &player, c.z, c.x));
            if hist[i].len() > 180 {
                hist[i].remove(0);
            }
        }
        if let Some(hit) = collide::check(&player, prev_z, &field, &road) {
            contacts += 1;
            let h = &hist[hit.car];
            let n = h.len();
            let seen_s = h.iter().rev().take_while(|(on, _)| *on).count() as f32 * DT;
            let at = |secs: f32| {
                h.get(n.saturating_sub(1 + (secs / DT) as usize)).map(|v| v.1).unwrap_or(0.0)
            };
            let curve = road.curve_at(player.z).abs();
            if curve > 0.3 && seen_s < 1.0 {
                hidden_in_bends += 1;
            }
            seen_total += seen_s.min(3.0);
            px_1s_total += at(1.0);
            println!(
                "  {t:>6.1} {curve:>6.2} {:>8.0} {:>8.2} {:>9.2} {:>8.0} {:>8.0} {:>7.0}",
                hit.closing,
                (player.x - hit.x).abs(),
                seen_s,
                at(1.0),
                at(0.5),
                at(0.0)
            );
            player.z = road.wrap(hit.player_z);
            player.speed = 0.0;
            burn = crash::BURN_TIME;
            for hh in hist.iter_mut() {
                hh.clear();
            }
        }
        let mut step = player.z - last_z;
        if step < -length / 2.0 {
            step += length;
        }
        travelled += step.max(0.0);
        last_z = player.z;
        t += DT;
    }
    let n = contacts.max(1) as f32;
    println!(
        "\n  {contacts} contacts in 3 laps · mean on-screen {:.2}s (capped at 3) · mean {:.0}px a second out · {hidden_in_bends} in bends with under a second on screen\n",
        seen_total / n,
        px_1s_total / n
    );

    // B. Size by seconds-to-contact.
    let straight = Road::straight(400);
    let cam_s = Camera::for_road(&straight, CAMERA_FILL);
    let p0 = Drive { z: straight.segment_length() * 3.0, x: 0.0, speed: tuning.top_speed };
    let ps = player_scale(&straight, &cam_s, &p0);
    println!("  B. HOW TALL A CAR IN YOUR LANE DRAWS (player is {:.0}px) BY SECONDS TO CONTACT\n", CAR_INK_ROWS * ps);
    println!("  {:>10} {:>7} {:>7} {:>7} {:>7}", "closing", "1.5s", "1.0s", "0.5s", "0.25s");
    for cruise in [CRUISE_MIN, (CRUISE_MIN + CRUISE_MAX) / 2.0, CRUISE_MAX] {
        let closing = tuning.top_speed * (1.0 - cruise);
        let mut row = format!("  {:>4.0}% {:>4.0}", (1.0 - cruise) * 100.0, closing);
        for secs in [1.5f32, 1.0, 0.5, 0.25] {
            let d = closing * secs + collide::contact_distance(&straight);
            let (_, h) = seen(&straight, &cam_s, &p0, p0.z + d, 0.0);
            row += &format!(" {h:>7.0}");
        }
        println!("{row}");
    }

    // C. Frame entry in each bend the course has.
    println!("\n  C. IN A BEND, HOW MANY SECONDS BEFORE CONTACT A CAR IN YOUR LANE ENTERS THE FRAME\n");
    println!("  {:>6} {:>10} {:>10} {:>10}", "curve", "closing", "enters at", "seconds");
    for raw in [30.0f32, 45.0, 57.0, 78.0, 108.0] {
        let bend = Road::new(vec![Segment::curving(raw); 400], 200.0, 2200.0);
        let cam_b = Camera::for_road(&bend, CAMERA_FILL);
        let pb = Drive { z: bend.segment_length() * 3.0, x: 0.0, speed: tuning.top_speed };
        let curve = bend.curve_at(pb.z).abs();
        // The player's own corner speed and the traffic's, from the same balance.
        let player_v = tuning.top_speed * pace::holdable(curve, &tuning).min(1.0);
        let mut first = None;
        let mut d = 30000.0f32;
        while d > 0.0 {
            if seen(&bend, &cam_b, &pb, pb.z + d, 0.0).0 && first.is_none() {
                first = Some(d);
            }
            d -= 25.0;
        }
        let enters = first.unwrap_or(0.0) - collide::contact_distance(&bend);
        for cruise in [CRUISE_MIN, CRUISE_MAX] {
            // The traffic's own corner rule: cruise, capped inside the limit.
            let traffic_v = (tuning.top_speed * cruise)
                .min(tuning.top_speed * pace::holdable(curve, &tuning) * CORNER_MARGIN);
            let closing = (player_v - traffic_v).max(1.0);
            println!(
                "  {curve:>6.2} {:>9.0} {:>10.0} {:>10.2}",
                closing, enters, enters / closing
            );
        }
    }
    println!("\n  Rows come in pairs: the slowest car in the band, then the fastest.\n");
}
