//! Brian ran a full qualifying lap and heard no whoosh at any volume.
//! The probes say every driving style triggers it. So run the racer's
//! OWN frame logic and find where the signal dies.

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
    let mut field = traffic::Field::grid(&road, 5);
    let mut player = drive::Drive::new();
    let dt = 1.0 / 60.0;

    let mut recycled_calls = 0u32;
    let mut frames_with_gaps = 0u32;
    let mut total_gaps = 0u32;
    let mut near_some = 0u32;
    let mut travelled = 0.0f32;
    let mut last_z = player.z;
    let mut t = 0.0f32;

    // ONE qualifying lap, the way Brian just drove.
    while travelled < length && t < 300.0 {
        Pacer::EXACT.step(&mut player, &road, &tuning, dt);
        field.advance(dt, &road, &tuning);

        // Exactly the racer's order.
        field.recycle(player.z, player.x, &road);
        recycled_calls += 1;

        let gaps = field.pass_gaps();
        if !gaps.is_empty() {
            frames_with_gaps += 1;
            total_gaps += gaps.len() as u32;
            let near = gaps
                .iter()
                .filter_map(|g| sound::Pass::intensity_for(*g))
                .fold(None, |b: Option<f32>, i| Some(b.map_or(i, |x: f32| x.max(i))));
            if let Some(i) = near {
                near_some += 1;
                println!("  frame {recycled_calls}: gaps {:?} -> intensity {i:.3}",
                         gaps.iter().map(|g| (g * 100.0).round() / 100.0)
                             .collect::<Vec<_>>());
            } else {
                println!("  frame {recycled_calls}: gaps {:?} -> NO intensity",
                         gaps.iter().map(|g| (g * 100.0).round() / 100.0)
                             .collect::<Vec<_>>());
            }
        }
        field.take_passes();

        let mut step = player.z - last_z;
        if step < -length / 2.0 { step += length; }
        travelled += step.max(0.0);
        last_z = player.z;
        t += dt;
    }

    println!();
    println!("  one qualifying lap, {t:.0}s, {recycled_calls} frames:");
    println!("    frames with a recorded pass : {frames_with_gaps}");
    println!("    total gaps recorded         : {total_gaps}");
    println!("    frames that produced a sound: {near_some}");

    // What do they actually SOUND like next to the engine?
    println!();
    println!("  RENDERED PEAKS, against the engine's 0.34:");
    let mut eng = sound::Engine::new();
    let mut warm = vec![0.0; 48_000];
    use omarcade_core::{Voice, VoiceParams};
    eng.render(&mut warm, VoiceParams::engine(0.8), 48_000.0);
    eng.render(&mut warm, VoiceParams::engine(0.8), 48_000.0);
    let engine_peak = warm.iter().fold(0.0f32, |a, b| a.max(b.abs()));

    for gap in [0.15f32, 0.36, 0.50, 0.72, 0.88, 1.00] {
        let Some(i) = sound::Pass::intensity_for(gap) else { continue };
        let mut p = sound::Pass::new();
        p.retrigger(i, 1.0);
        let mut buf = vec![0.0; 24_000];
        p.render(&mut buf, VoiceParams::SILENT, 48_000.0);
        let peak = buf.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        println!("    gap {gap:.2} -> peak {peak:.3}  ({:.0}% of the engine)",
                 peak / engine_peak * 100.0);
    }
    println!("    engine at 80% throttle peaks at {engine_peak:.3}");
}
