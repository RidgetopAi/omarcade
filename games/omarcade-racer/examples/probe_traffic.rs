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
#[path = "../src/collide.rs"]
mod collide;
#[path = "../src/traffic.rs"]
mod traffic;
#[path = "../src/pace.rs"]
mod pace;

use drive::{Drive, Tuning};
use pace::Pacer;
use track::grand_prix;
use traffic::{Field, CRUISE_MAX, CRUISE_MIN};

const DT: f32 = 1.0 / 240.0;
const CARS: usize = 5;

/// The player is the reference driver from `pace`: brake only when the
/// bend ahead demands it, otherwise flat out. It is not a perfect human
/// — it is the physics-exact one, which is the bar the traffic has to
/// be judged against. It used to be a local copy with no lookahead and
/// a 0.9 margin; the lap it drives is within two seconds of that one,
/// so the overtake arithmetic below still holds.
const PACER: Pacer = Pacer::EXACT;

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
    let mut last_pass: Vec<f32> = vec![-99.0; CARS];

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
        PACER.step(&mut player, &road, &tuning, DT);
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
            // ⚠️ ONE PASS PER CAR PER COOLDOWN. A bare sign change
            // double-counts: a car sitting near the boundary oscillates
            // across it and logs four "overtakes" in 0.2s at identical
            // closing speed, which is what the raw numbers showed. The
            // traffic was fine; the instrument was counting wrong.
            if prev[i] > 0.0
                && g <= 0.0
                && (prev[i] - g).abs() < length * 0.1
                && t - last_pass[i] > 1.0
            {
                passes.push((t, player.speed - car.speed, (player.x - car.x).abs()));
                last_pass[i] = t;
            }
            prev[i] = g;
        }
    }

    println!(
        "\n  {:.2} laps in {t:.1}s with {CARS} cars on track  ({:.1}s per lap, pace::Pacer margin {:.2})\n",
        travelled / length,
        t / (travelled / length).max(0.01),
        PACER.margin,
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
    println!("    · ~12 passes a lap with 5 cars and recycling at 0.33 laps. That");
    println!("      number is DERIVED, not a preference: a car is recycled after the");
    println!("      player travels 0.33 laps (~30s), reappears ~1.5 draw distances");
    println!("      ahead and is re-passed ~7s later, so each car cycles about every");
    println!("      36s — 2.5 passes per car per lap, 12 across the field.");
    println!("      ⚠️ An earlier version of this note said 'one pass per car per");
    println!("      lap'. That described a FIXED FIELD and was written before");
    println!("      recycling existed; against a stream it is simply the wrong bar.");
    println!("      If you change RECYCLE_BEHIND_LAPS, redo this arithmetic.");
    println!("    · passes on the same line are the ones that become crashes once");
    println!("      collision lands; they should exist, but not be most of them");
    println!("    · an overtake much longer than a straight means the cruise band's");
    println!("      top end is too high\n");
}
