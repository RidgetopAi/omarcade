//! Is the traffic passable, and does the field behave like traffic?
//!
//! The tests in `traffic.rs` guard properties: cars move, cars stay on
//! the road, cars are slower than the player, the field does not hold
//! formation. All of that can be true of traffic that is still no fun to
//! drive through — five cars nose to tail on the racing line pass every
//! one of those checks.
//!
//! What actually matters is a question no unit test asks: **over a real
//! lap, how many cars does a competent driver pass, how fast are they
//! closing when it happens, and is there room to get by?** This drives
//! the shipped course and reports exactly that.
//!
//!   cargo run -p omarcade-racer --example probe_traffic

#[path = "../src/road.rs"]
mod road;
#[path = "../src/drive.rs"]
mod drive;
#[path = "../src/track.rs"]
mod track;
#[path = "../src/traffic.rs"]
mod traffic;

use drive::{Drive, Tuning};
use road::Road;
use track::grand_prix;
use traffic::{Field, CRUISE_MAX, CRUISE_MIN};

const DT: f32 = 1.0 / 240.0;
const CARS: usize = 5;

/// A driver that goes as fast as the corner allows.
///
/// The same shape `probe_track` uses: brake only when the bend demands
/// it, otherwise flat out, and steer back toward the centre. It is not a
/// perfect driver — it is a competent one, which is the bar the traffic
/// has to be judged against.
fn drive_step(car: &mut Drive, road: &Road, tuning: &Tuning) {
    let curve = road.curve_at(car.z).abs();

    // The fastest this bend can be held, from the same balance the
    // traffic uses: steer_rate * a == curve * a² * centrifugal.
    let holdable = if curve > f32::EPSILON {
        tuning.top_speed * tuning.steer_rate / (curve * tuning.centrifugal)
    } else {
        f32::INFINITY
    };

    let brake = if car.speed > holdable * 0.9 { 1.0 } else { 0.0 };
    let throttle = if brake > 0.0 { 0.0 } else { 1.0 };
    let steer = (-car.x * 3.0).clamp(-1.0, 1.0);
    car.update(DT, throttle, brake, steer, road, tuning);
}

fn main() {
    let road = grand_prix().build();
    let tuning = Tuning::from_corner(&road, 1.5);
    let length = road.length();

    let mut field = Field::grid(&road, CARS);
    let mut player = Drive::new();

    println!("\n  TRAFFIC — is it passable?\n");
    println!(
        "  player top speed {:.0} u/s · cruise band {:.0}-{:.0} u/s ({:.0}-{:.0}%)",
        tuning.top_speed,
        tuning.top_speed * CRUISE_MIN,
        tuning.top_speed * CRUISE_MAX,
        CRUISE_MIN * 100.0,
        CRUISE_MAX * 100.0,
    );

    // Track each car's gap to the player, signed and unwrapped, so a
    // pass is a sign change rather than a coordinate comparison — the
    // course wraps, and a naive `player.z > car.z` fires once a lap for
    // free.
    let gap_to = |p: &Drive, z: f32| -> f32 {
        let mut d = z - p.z;
        while d > length / 2.0 {
            d -= length;
        }
        while d < -length / 2.0 {
            d += length;
        }
        d
    };

    let mut prev: Vec<f32> = field.cars.iter().map(|c| gap_to(&player, c.z)).collect();
    let mut passes: Vec<(f32, f32, f32)> = Vec::new(); // (lap time, closing speed, lateral gap)

    let mut t = 0.0f32;

    // ⚠️ LAPS ARE COUNTED FROM DISTANCE TRAVELLED, NOT FROM z WRAPPING.
    // `Drive` already wraps z, and the player starts at a negative
    // GRID_SETBACK offset, so a "z went backwards" test trips on the
    // first wrap and every lap after it is short. The first version of
    // this probe reported 269.8s for three laps against a measured
    // 108.1s reference (probe_track) — 2.5 laps counted as 3. Distance
    // is unambiguous and does not care where the start line is.
    let mut travelled = 0.0f32;
    let mut last_z = player.z;

    // Three laps, the race distance the plan specifies.
    while travelled < length * 3.0 && t < 900.0 {
        drive_step(&mut player, &road, &tuning);
        field.advance(DT, &road, &tuning);
        field.recycle(player.z, &road);
        t += DT;

        let mut step = player.z - last_z;
        if step < -length / 2.0 {
            step += length;
        }
        travelled += step.max(0.0);
        last_z = player.z;

        for (i, car) in field.cars.iter().enumerate() {
            let g = gap_to(&player, car.z);
            // A pass: the car was ahead and is now behind. Guard against
            // the wrap by ignoring jumps of more than a fraction of the
            // course.
            if prev[i] > 0.0 && g <= 0.0 && (prev[i] - g).abs() < length * 0.1 {
                passes.push((t, player.speed - car.speed, (player.x - car.x).abs()));
            }
            prev[i] = g;
        }
    }

    println!(
        "\n  {:.2} laps in {t:.1}s with {CARS} cars on track  ({:.1}s per lap)\n",
        travelled / length,
        t / (travelled / length).max(0.01),
    );
    println!("  {} overtakes", passes.len());

    if passes.is_empty() {
        println!("\n  ⚠️  NO OVERTAKES. Traffic is either too fast to catch or the");
        println!("      pass detector is wrong. Both are bugs.\n");
        return;
    }

    println!("\n    {:>8}{:>14}{:>16}", "at", "closing u/s", "lateral gap");
    for (at, closing, lateral) in &passes {
        let flag = if *lateral < 0.25 {
            "  ← same line"
        } else {
            ""
        };
        println!("    {at:>7.1}s{closing:>14.0}{lateral:>16.2}{flag}");
    }

    let mean_close =
        passes.iter().map(|p| p.1).sum::<f32>() / passes.len() as f32;
    let tight = passes.iter().filter(|p| p.2 < 0.25).count();

    println!("\n  mean closing speed {mean_close:.0} u/s");
    println!("  {tight} of {} passes were on the same line", passes.len());

    // How long an overtake takes to complete, against how much straight
    // there is to do it in. This is the number that decides whether the
    // cruise band's upper end is right.
    let car_length = road.segment_length();
    let overtake_seconds = car_length * 3.0 / mean_close.max(1.0);
    println!(
        "\n  an overtake takes about {overtake_seconds:.1}s at that closing speed"
    );

    // Where the field actually ended up, which is what decides whether
    // seven passes is "traffic works" or "traffic ran out".
    println!("\n  WHERE THE FIELD ENDED UP");
    println!("    {:>4}{:>12}{:>14}{:>12}", "car", "cruise", "gap ahead", "laps done");
    let mut rows: Vec<(usize, f32, f32, f32)> = field
        .cars
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let mut d = c.z - player.z;
            while d < 0.0 {
                d += length;
            }
            (i, c.speed / tuning.top_speed, d / length, 0.0)
        })
        .collect();
    rows.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());
    for (i, frac, ahead, _) in &rows {
        println!("    {i:>4}{:>11.0}%{:>13.2}L{:>12}", frac * 100.0, ahead, "");
    }

    println!("\n  WHAT TO LOOK FOR");
    println!("    · roughly one pass per car per lap — many fewer means traffic is");
    println!("      too fast to catch, many more means it is parked");
    println!("    · passes on the same line are the ones that become crashes once");
    println!("      collision lands; they should exist, but not be most of them");
    println!("    · an overtake much longer than a straight means the cruise band's");
    println!("      top end is too high\n");
}
