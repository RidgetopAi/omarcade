//! Render the engine voice offline and write a WAV.
//!
//! The audio path has a device at one end and a real-time thread in the
//! middle, neither of which a test can inspect. This renders the same
//! [`Voice`] the game plays, straight to a file, so the synthesis can be
//! heard and measured on its own:
//!
//! ```text
//! cargo run -p omarcade-racer --example probe_engine
//! pw-play /tmp/omaprix-engine.wav
//! ```

#[path = "../src/sound.rs"]
mod sound;

use std::fs::File;
use std::io::{BufWriter, Write};

use omarcade_core::{Voice, VoiceParams};

const SR: f32 = 48_000.0;

fn write_wav(path: &str, samples: &[f32]) -> std::io::Result<()> {
    let mut w = BufWriter::new(File::create(path)?);
    let data_len = (samples.len() * 2) as u32;
    w.write_all(b"RIFF")?;
    w.write_all(&(36 + data_len).to_le_bytes())?;
    w.write_all(b"WAVEfmt ")?;
    w.write_all(&16u32.to_le_bytes())?;
    w.write_all(&1u16.to_le_bytes())?;
    w.write_all(&1u16.to_le_bytes())?;
    w.write_all(&(SR as u32).to_le_bytes())?;
    w.write_all(&((SR as u32) * 2).to_le_bytes())?;
    w.write_all(&2u16.to_le_bytes())?;
    w.write_all(&16u16.to_le_bytes())?;
    w.write_all(b"data")?;
    w.write_all(&data_len.to_le_bytes())?;
    for s in samples {
        w.write_all(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())?;
    }
    Ok(())
}

fn main() -> std::io::Result<()> {
    let mut engine = sound::Engine::new();
    let mut all = Vec::new();

    // Steady points, then a rev — the same auditions engine.py writes.
    for (throttle, secs) in [(0.0, 1.0), (0.3, 1.0), (0.7, 1.0), (1.0, 1.0)] {
        let n = (SR * secs) as usize;
        let mut buf = vec![0.0; n];
        engine.render(&mut buf, VoiceParams::engine(throttle), SR);
        println!("throttle {:3.0}%  peak {:.3}", throttle * 100.0,
                 buf.iter().fold(0.0f32, |a, b| a.max(b.abs())));
        all.extend_from_slice(&buf);
    }

    // The rev: the audition that matters, and the one thing a steady
    // note cannot show.
    let block = (SR * 0.02) as usize;
    for i in 0..150 {
        let f = i as f32 / 150.0;
        let t = if f < 0.5 { f * 2.0 } else { (1.0 - f) * 2.0 };
        let mut buf = vec![0.0; block];
        engine.render(&mut buf, VoiceParams::engine(t), SR);
        all.extend_from_slice(&buf);
    }

    // Diagnostic: what throttle does the voice actually reach, and how
    // many firings happen in a second at each step?
    {
        let mut e = sound::Engine::new();
        for t in [0.0f32, 0.3, 0.7, 1.0] {
            let mut buf = vec![0.0; SR as usize];
            e.render(&mut buf, VoiceParams::engine(t), SR);
            // Count zero crossings of the pulse train as a proxy for the
            // firing rate actually produced.
            let before = e.firings;
            e.render(&mut buf, VoiceParams::engine(t), SR);
            let fired = e.firings - before;
            let want = sound::Engine::firing_hz(t);
            println!("  requested {t:.1} -> {fired} firings/s (want {want:.0})");
        }
    }

    write_wav("/tmp/omaprix-engine.wav", &all)?;
    println!("\nwrote /tmp/omaprix-engine.wav ({:.1}s)", all.len() as f32 / SR);
    Ok(())
}

#[cfg(test)]
mod probe_tests {}
