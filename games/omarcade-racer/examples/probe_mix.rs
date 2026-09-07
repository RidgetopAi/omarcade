//! Drive the real game loop and report what the tyre voice receives and
//! produces. Brian heard no squeal on the track; the lean signal is fine
//! (probe_squeal), so this checks the rest of the path.

#[path = "../src/track.rs"] mod track;
#[path = "../src/road.rs"] mod road;
#[path = "../src/drive.rs"] mod drive;
#[path = "../src/pace.rs"] mod pace;
#[path = "../src/sound.rs"] mod sound;

use omarcade_core::{Voice, VoiceParams};

fn main() {
    let course = track::grand_prix();
    let r = course.build();
    let tuning = drive::Tuning::from_corner(&r, 2.6);

    // Walk the lap and render the voice with the params the game sends.
    let mut sq = sound::Squeal::new();
    let mut loudest: (f32, f32, f32) = (0.0, 0.0, 0.0);
    let mut audible_frames = 0;

    let frames = 600;
    let step = r.length() / frames as f32;
    let mut z = 0.0;
    for _ in 0..frames {
        let speed = tuning.top_speed;                 // flat out
        let authority = (speed / tuning.top_speed).clamp(0.0, 1.0);
        let lean = (r.curve_at(z) / 1.8 * authority).abs().min(1.0);
        let throttle = speed / tuning.top_speed;

        // One video frame of audio at 60fps.
        let mut buf = vec![0.0; 800];
        sq.render(&mut buf, VoiceParams::squeal(lean, throttle), 48_000.0);
        let peak = buf.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        if peak > 1e-4 { audible_frames += 1; }
        if peak > loudest.2 { loudest = (lean, throttle, peak); }
        z += step;
    }

    println!("over one flat-out lap ({frames} frames of audio):");
    println!("  frames with any output : {audible_frames} / {frames}");
    println!("  loudest frame          : lean {:.2} throttle {:.2} -> peak {:.4}",
             loudest.0, loudest.1, loudest.2);
    println!();

    // And a direct check at a known-loud lean, held long enough to slew.
    let mut sq2 = sound::Squeal::new();
    for _ in 0..60 {
        let mut buf = vec![0.0; 800];
        sq2.render(&mut buf, VoiceParams::squeal(1.0, 1.0), 48_000.0);
    }
    let mut buf = vec![0.0; 4800];
    sq2.render(&mut buf, VoiceParams::squeal(1.0, 1.0), 48_000.0);
    let peak = buf.iter().fold(0.0f32, |a, b| a.max(b.abs()));
    println!("held at full lean for a second: peak {peak:.4}");
    println!("(the engine, for comparison, peaks around 0.34)");
}
