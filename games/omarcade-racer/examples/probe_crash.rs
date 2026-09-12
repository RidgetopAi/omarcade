//! What does the crash actually sound like?
//!
//! Brian: "the crash does sound like a blip blop". The playground was
//! the first suspect — `chimes.html` is known to be miscalibrated
//! against its Rust (ref:s9-synthesis-mismatch) — but the crash's
//! numbers went the other way: the commit that landed it records his
//! tuning ("the impact twice as long and a third lower") and the RUST
//! holds those values. crash.html's sliders were never updated from
//! their originals. So the tool is stale and the game is right, and the
//! sound has to be measured rather than blamed on a transfer.
//!
//!   cargo run -p omarcade-racer --example probe_crash

#[path = "../src/sound.rs"]
mod sound;

use omarcade_core::{Voice, VoiceParams};

const SR: f32 = 48_000.0;

fn db(x: f32) -> f32 {
    if x > 0.0 { 20.0 * x.log10() } else { -99.0 }
}

fn main() {
    println!();
    println!("OMAPRIX — THE CRASH, MEASURED");
    println!("=============================");
    println!();

    for force in [1.0f32, 0.5, 0.25] {
        let mut c = sound::Crash::new();
        c.retrigger(force, 1.0);
        let n = (SR * 1.6) as usize;
        let mut buf = vec![0.0f32; n];
        c.render(&mut buf, VoiceParams::SILENT, SR);

        let peak = buf.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        println!("  force {force:.2}   peak {peak:.3} ({:.1} dBFS)", db(peak));

        // The envelope, in 50 ms slices — this is the SHAPE, and the
        // shape is what "blip blop" is a description of.
        print!("    envelope  ");
        let slice = (SR * 0.02) as usize;
        for w in buf.chunks(slice).take(70) {
            let p = w.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            let bar = (p / peak.max(1e-9) * 8.0) as usize;
            print!("{}", ["·", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"][bar.min(8)]);
        }
        println!("   (each cell = 20 ms, 1.4 s shown)");

        // Where the energy stops. A crash that is over in 200 ms is a
        // blip however loud its first sample is.
        let audible = buf
            .iter()
            .rposition(|s| s.abs() > peak * 0.05)
            .map(|i| i as f32 / SR)
            .unwrap_or(0.0);
        println!("    audible for {audible:.2} s (to -26 dB of its own peak)");
        println!();
    }

    // How much is being lost to the clamp? Count samples pinned at
    // full scale — a transient that spends real time at ±1.0 is not
    // loud, it is CHOPPED, and a chopped transient is a click.
    {
        let mut c = sound::Crash::new();
        c.retrigger(1.0, 1.0);
        let n = (SR * 1.6) as usize;
        let mut buf = vec![0.0f32; n];
        c.render(&mut buf, VoiceParams::SILENT, SR);
        let pinned = buf.iter().filter(|s| s.abs() >= 0.999).count();
        println!("  CLIPPING");
        println!("  --------");
        println!(
            "    {pinned} samples pinned at full scale ({:.1} ms of the impact)",
            pinned as f32 / SR * 1000.0,
        );
        println!("    ⚠️ The voice clamps to ±1.0 INTERNALLY, so this is lost");
        println!("       before the master volume or the mixer ever sees it.");
        println!();
    }

    // The burn on its own, against the impact. This is the ratio that
    // decided everything: at -24 dB the fire was inaudible under the
    // hit and the crash read as a blip.
    {
        let mut c = sound::Crash::new();
        c.retrigger(1.0, 1.0);
        let n = (SR * 1.6) as usize;
        let mut buf = vec![0.0f32; n];
        c.render(&mut buf, VoiceParams::SILENT, SR);
        let impact_end = (SR * 0.20) as usize;
        let burn_from = (SR * 0.40) as usize;
        let ip = buf[..impact_end].iter().fold(0.0f32, |m, s| m.max(s.abs()));
        let bp = buf[burn_from..].iter().fold(0.0f32, |m, s| m.max(s.abs()));
        println!("  THE BURN AGAINST THE IMPACT");
        println!("  ---------------------------");
        println!("    impact peak (0-200 ms)   {ip:.3} ({:.1} dBFS)", db(ip));
        println!("    burn peak   (400 ms+)    {bp:.3} ({:.1} dBFS)", db(bp));
        println!("    the burn sits {:.1} dB under the impact", db(bp) - db(ip));
        println!("    ⚠️ It was -24.1 dB before the port. Under about");
        println!("       -20 dB the fire is inaudible beside the hit.");
        println!();
    }

    println!("  ⚠️ The fireball sprite burns for 1.40 s. A sound much");
    println!("     shorter than that is a blip under a fire that is");
    println!("     still on screen.");
    println!();
}
