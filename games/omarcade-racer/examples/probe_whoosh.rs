//! Brian drove and heard no pass whoosh. The probe said 2-3 a lap.
//! Trace the whole path: gap recorded -> intensity -> played.

#[path = "../src/track.rs"] mod track;
#[path = "../src/road.rs"] mod road;
#[path = "../src/drive.rs"] mod drive;
#[path = "../src/pace.rs"] mod pace;
#[path = "../src/art.rs"]
mod art;
#[path = "../src/scenery.rs"]
mod scenery;
#[path = "../src/structures.rs"]
mod structures;
#[path = "../src/collide.rs"] mod collide;
#[path = "../src/traffic.rs"] mod traffic;
#[path = "../src/sound.rs"] mod sound;

use pace::Pacer;

fn main() {
    let road = track::grand_prix().build();
    let tuning = drive::Tuning::from_corner(&road, 1.5);
    let length = road.length();

    // TRAFFIC_CARS in main.rs is 5 — probe_traffic uses a different
    // number, and the count changes how often you meet anyone.
    let mut field = traffic::Field::grid(&road, 5);
    let mut player = drive::Drive::new();
    let dt = 1.0 / 60.0;

    let mut gaps = Vec::new();
    let mut fired = 0;
    let mut travelled = 0.0f32;
    let mut last_z = player.z;
    let mut t = 0.0f32;

    while travelled < length * 3.0 && t < 600.0 {
        Pacer::EXACT.step(&mut player, &road, &tuning, dt);
        field.advance(dt, &road, &tuning);
        field.recycle(player.z, player.x, &road);

        for g in field.pass_gaps() {
            gaps.push(*g);
            if sound::Pass::intensity_for(*g).is_some() {
                fired += 1;
            }
        }
        field.take_passes();

        let mut step = player.z - last_z;
        if step < -length / 2.0 { step += length; }
        travelled += step.max(0.0);
        last_z = player.z;
        t += dt;
    }

    println!("\n  WITH {} CARS (the game's TRAFFIC_CARS), 3 laps:\n", 5);
    println!("    {} passes, {fired} close enough to sound", gaps.len());
    println!("    threshold CLOSE_ENOUGH = {}", sound::CLOSE_ENOUGH);
    if !gaps.is_empty() {
        gaps.sort_by(|a: &f32, b| a.partial_cmp(b).unwrap());
        let n = gaps.len();
        let pct = |p: f32| gaps[((n - 1) as f32 * p) as usize];
        println!("    closest {:.3} · 25th {:.3} · median {:.3} · widest {:.3}",
                 gaps[0], pct(0.25), pct(0.5), gaps[n - 1]);
        println!("    -> {:.1} whooshes per lap", fired as f32 / 3.0);
    }

    // ── And now a driver who AVOIDS cars, which is what a player does.
    for (name, avoid) in [("barely reacts", 0.08f32), ("dodges a little", 0.18), ("dodges clearly", 0.35)] {
        let mut field = traffic::Field::grid(&road, 5);
        let mut player = drive::Drive::new();
        let mut gaps = Vec::new();
        let mut fired = 0;
        let mut travelled = 0.0f32;
        let mut last_z = player.z;
        let mut t = 0.0f32;

        while travelled < length * 3.0 && t < 600.0 {
            // Steer away from the nearest car ahead, the way a player
            // threading traffic does.
            // Steer only when a car is close ahead AND roughly in the
            // way — a player aims to clear it, not to run from it.
            let mut steer = 0.0f32;
            let mut nearest = f32::MAX;
            for car in &field.cars {
                let d = (car.z - player.z).rem_euclid(length);
                let across = (car.x - player.x).abs();
                if d < nearest && d < 40_000.0 && across < 1.0 {
                    nearest = d;
                    steer = if car.x > player.x { -avoid } else { avoid };
                }
            }
            // Drift back toward the middle when nothing is in the way,
            // so the car does not end up parked on a verge.
            if steer == 0.0 {
                steer = (-player.x * 0.5).clamp(-0.3, 0.3);
            }
            player.update(dt, 1.0, 0.0, steer, &road, &tuning);
            field.advance(dt, &road, &tuning);
            field.recycle(player.z, player.x, &road);
            for g in field.pass_gaps() {
                gaps.push(*g);
                if sound::Pass::intensity_for(*g).is_some() { fired += 1; }
            }
            field.take_passes();

            let mut step = player.z - last_z;
            if step < -length / 2.0 { step += length; }
            travelled += step.max(0.0);
            last_z = player.z;
            t += dt;
        }

        gaps.sort_by(|a: &f32, b| a.partial_cmp(b).unwrap());
        println!("  A DRIVER WHO {}:", name.to_uppercase());
        if gaps.is_empty() {
            println!("    no passes at all");
        } else {
            let n = gaps.len();
            let pct = |p: f32| gaps[((n - 1) as f32 * p) as usize];
            println!("    {n} passes over 3 laps, {fired} close enough ({:.1}/lap)",
                     fired as f32 / 3.0);
            println!("    closest {:.3} · 25th {:.3} · median {:.3} · widest {:.3}",
                     gaps[0], pct(0.25), pct(0.5), gaps[n - 1]);
            println!("    all gaps: {:?}",
                     gaps.iter().map(|g| (g * 100.0).round() / 100.0).collect::<Vec<_>>());
        }
    }

    println!();
    println!("  ⚠️ Pacer::EXACT drives the RACING LINE. A player STEERS,");
    println!("     and steering round traffic is exactly what stops a pass");
    println!("     being close.");
}
