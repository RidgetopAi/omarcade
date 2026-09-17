//! Render candidate laser sounds to .wav files so Brian can HEAR them.
//!
//!   cargo run -p omarcade-defender --example sound_lab -- <outdir>
//!
//! ★ THIS IS THE VECTOR PLAYGROUND MOVE, FOR EARS. Brian built the ship
//! and the Mutant himself once a tool existed that let him see what he
//! was doing. Sound has exactly the same problem and worse: I cannot
//! hear ANY of this. Tuning a constant, asking him to rebuild and fly,
//! and waiting for a verdict is a minutes-long loop for a judgement that
//! should take two seconds of listening. So: render every candidate to a
//! file, let him play them back to back, and let him point at one.
//!
//! ⚠️ THE BRIEF, IN HIS WORDS: "our laser is higher pitched and original
//! is more white noise". That is a diagnosis of METHOD, not of tuning —
//! our laser puts its energy in a line (an oscillator) and the original
//! does not. No value of LASER_TOP_HZ fixes that; a lower-pitched tone
//! is still a tone.
//!
//! ★ THE PRECEDENT, AND IT IS THE SAME FINDING RUNNING BACKWARDS:
//! the racer's tyre squeal was filtered noise, and Brian's verdict was
//! "sounds like static, and not high pitched static either." It needed
//! an oscillator, because a pitch needs energy in a LINE. Defender's
//! laser is the mirror image — it needs to stop being a line. See the
//! "Why it is an oscillator and not filtered noise" block in
//! games/omarcade-racer/src/sound.rs.
//!
//! ⚠️ WHAT THE HARDWARE ACTUALLY DID, AND WHAT IS NOT KNOWN. Defender's
//! sound board is a 6802/6808 writing 8-bit samples straight to a DAC
//! through a PIA, so it was never constrained to an oscillator chip's
//! waveforms — it could emit anything. Reverse-engineers of the sound
//! ROM describe variable-duty-cycle square waves, tables that modulate
//! pitch and volume, and "something akin to granular synthesis" built
//! from dynamically generated delay loops. ★ NOBODY HAS PUBLISHED THE
//! ACTUAL LASER ROUTINE — the annotated disassembly is withheld for
//! copyright. So these candidates are informed guesses at a mechanism,
//! and BRIAN'S EAR IS THE AUTHORITY, not the archaeology.

use std::f32::consts::TAU;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

const SR: f32 = 48_000.0;

/// How long each candidate runs, in seconds. Long enough to hear the
/// tail, short enough that five of them back to back is not a chore.
const LEN: f32 = 0.16;

fn main() {
    let dir: PathBuf = std::env::args()
        .nth(1)
        .unwrap_or_else(|| ".".to_string())
        .into();

    let candidates: Vec<(&str, fn(f32) -> f32)> = vec![
        ("0-current", current),
        ("1-noise-swept", noise_swept),
        ("2-duty-square", duty_square),
        ("3-noise-plus-tone", noise_plus_tone),
        ("4-grain", grain),
    ];

    for (name, f) in &candidates {
        let samples: Vec<f32> = (0..(LEN * SR) as usize)
            .map(|i| f(i as f32 / SR))
            .collect();
        let path = dir.join(format!("laser-{name}.wav"));
        write_wav(&path, &samples);
        let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        println!("wrote {} (peak {:.2})", path.display(), peak);
    }

    println!();
    println!("Play them in order, e.g.:");
    println!("  for f in {}/laser-*.wav; do echo \"$f\"; paplay \"$f\"; sleep 0.4; done", dir.display());
    println!();
    println!("0-current          what the game ships today — the pitched sweep");
    println!("1-noise-swept      white noise through a swept bandpass. The literal");
    println!("                   reading of \"more white noise\".");
    println!("2-duty-square      a square whose DUTY CYCLE and period both sweep.");
    println!("                   Closest to what the ROM is documented to do.");
    println!("3-noise-plus-tone  the current sweep with noise layered over it —");
    println!("                   keeps some pitch, adds the hash.");
    println!("4-grain            short repeating grains at a falling rate, the");
    println!("                   \"granular\" reading of the delay-loop trick.");
}

// ---------------------------------------------------------------------
// The candidates
//
// ⚠️ EACH IS A PLAIN FUNCTION OF TIME, NOT A `Voice`. A Voice carries
// per-sample state across render blocks, which is right for the game and
// pure friction here — the lab renders one contiguous buffer and throws
// it away. Whichever candidate Brian picks gets written as a proper
// phase-integrating Voice when it lands in sound.rs.
//
// ⚠️ BUT THE PHASE IS STILL INTEGRATED INSIDE EACH ONE, via a `static`-
// free running accumulator passed through the closure state below. A
// sweep that recomputes sin(TAU*hz*t) clicks on every sample, and a
// candidate that clicks would be rejected for the wrong reason.
// ---------------------------------------------------------------------

/// What ships today: an exponential downward sweep, sine plus a little
/// square. Included so the comparison has a baseline — a candidate that
/// sounds better in isolation may sound worse next to what he knows.
fn current(t: f32) -> f32 {
    let u = t / LEN;
    if u >= 1.0 {
        return 0.0;
    }
    let top = 1850.0;
    let ratio: f32 = 180.0 / 1850.0;
    // Integrated analytically: the integral of top*ratio^(u^c) dt.
    // Approximated by stepping, which is what the real Voice does.
    let phase = integrate(t, |tt| {
        let uu = tt / LEN;
        top * ratio.powf(uu.powf(0.62))
    });
    let sine = (TAU * phase).sin();
    let edge = if phase.fract() < 0.5 { 1.0 } else { -1.0 };
    let wave = sine * 0.72 + edge * 0.28;
    let env = if u < 0.06 { u / 0.06 } else { (-(u - 0.06) * 4.5).exp() };
    wave * env * 0.30
}

/// ★ THE LITERAL READING OF "MORE WHITE NOISE": white noise pushed
/// through a bandpass whose centre sweeps down hard.
///
/// ⚠️ THE RACER PROVED A BAND OF NOISE READS AS NOISE WHEREVER IT SITS —
/// which is a problem when you want a pitch and exactly what you want
/// when you do not. The sweep is heard as a whoosh moving downward
/// rather than as a falling note, and that may be precisely right.
fn noise_swept(t: f32) -> f32 {
    let u = t / LEN;
    if u >= 1.0 {
        return 0.0;
    }
    bandpass_noise(t, 2600.0, 260.0, 0.55, 3.2) * envelope(u, 0.02, 5.0) * 0.55
}

/// ★ CLOSEST TO THE DOCUMENTED HARDWARE: a square wave whose PERIOD and
/// DUTY CYCLE both sweep, fast.
///
/// A 50% square is a clean pitch. Sweep the duty toward the extremes and
/// the harmonic series lurches around — the ear stops tracking a
/// fundamental and starts hearing hash. That is the "variable duty cycle
/// square waves" the ROM disassembly describes, and it is a mechanism
/// that lands between "tone" and "noise" rather than at either end.
fn duty_square(t: f32) -> f32 {
    let u = t / LEN;
    if u >= 1.0 {
        return 0.0;
    }
    let phase = integrate(t, |tt| {
        let uu = tt / LEN;
        2200.0 * (150.0f32 / 2200.0).powf(uu.powf(0.5))
    });
    // Duty swings from very narrow to near half over the sweep, with a
    // fast wobble on top so it never settles into a stable timbre.
    let wobble = (TAU * 190.0 * t).sin() * 0.18;
    let duty = (0.08 + 0.34 * u + wobble).clamp(0.03, 0.97);
    let sq = if phase.fract() < duty { 1.0 } else { -1.0 };
    sq * envelope(u, 0.01, 5.5) * 0.28
}

/// The current sweep with noise layered over it.
///
/// The conservative option: keeps the pitched zip Brian already has and
/// adds hash on top, rather than replacing the mechanism. Worth hearing
/// because "more white noise" might mean "add some", not "make it noise".
fn noise_plus_tone(t: f32) -> f32 {
    let u = t / LEN;
    if u >= 1.0 {
        return 0.0;
    }
    let tone = current(t) * 0.55;
    let hash = bandpass_noise(t, 3000.0, 400.0, 0.5, 1.6) * envelope(u, 0.01, 6.0) * 0.5;
    tone + hash
}

/// The "granular" reading: a very short grain repeated at a rate that
/// falls over the sound.
///
/// The ROM is described as using "dynamically generated delay loops",
/// which is a period-modulated repeat of a small buffer. Repeating a
/// grain faster than about 30 Hz fuses into a pitch; slower and it reads
/// as a rattle. Sweeping through that boundary is a distinctive sound
/// and a plausible reading of the description.
fn grain(t: f32) -> f32 {
    let u = t / LEN;
    if u >= 1.0 {
        return 0.0;
    }
    // Grain rate falls from very fast to a rattle.
    let rate = 1400.0 * (90.0f32 / 1400.0).powf(u.powf(0.6));
    let period = 1.0 / rate;
    let within = (t % period) / period;
    // Each grain is a decaying burst of the same noise seed, so
    // successive grains are correlated — that correlation is what makes
    // it a texture rather than plain noise.
    let g = noise_at(within * 0.004) * (-within * 6.0).exp();
    g * envelope(u, 0.01, 4.2) * 0.6
}

// ---------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------

/// Deterministic white noise at a time, from a hash rather than a
/// running PRNG, so a candidate can be a pure function of `t`.
fn noise_at(t: f32) -> f32 {
    let n = (t * SR) as u32;
    let mut h = n.wrapping_mul(0x9E37_79B9);
    h ^= h >> 15;
    h = h.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 13;
    (h & 0x00FF_FFFF) as f32 / 0x0080_0000 as f32 - 1.0
}

/// Integrate a frequency function from 0 to `t`, returning phase in
/// cycles.
///
/// ⚠️ THE ONLY CORRECT WAY TO SWEEP AN OSCILLATOR. `sin(TAU*hz*t)` with a
/// changing `hz` jumps the phase whenever `hz` moves, which is a click
/// on every sample of a fast sweep. Stepping the integral at the sample
/// rate is what the real Voice does per-sample; here it is recomputed
/// from zero because the candidates are pure functions of time. That is
/// O(n^2) over the whole render and completely irrelevant at 7680
/// samples.
fn integrate(t: f32, hz: impl Fn(f32) -> f32) -> f32 {
    let steps = (t * SR) as usize;
    let dt = 1.0 / SR;
    let mut phase = 0.0f32;
    for i in 0..steps {
        phase += hz(i as f32 * dt) * dt;
    }
    phase
}

/// A resonant bandpass over noise, swept between two corners.
///
/// ⚠️ RUN FORWARD FROM t=0 EVERY CALL. A filter has state and cannot be
/// evaluated at a point, so this reconstructs it. That is O(n^2) across
/// the render and entirely irrelevant at 7680 samples — but it is the
/// reason the candidates are pure functions of `t`, which is what keeps
/// them readable as recipes rather than as machines.
///
/// `top`/`bottom` are the sweep's endpoints in Hz, so the caller states
/// the sweep once instead of the filter having to infer it from a
/// single sampled value — an earlier version tried to back the sweep out
/// of `centre_now` and was unreadable arithmetic that could only be
/// wrong quietly.
fn bandpass_noise(t: f32, top: f32, bottom: f32, curve: f32, q: f32) -> f32 {
    let steps = (t * SR) as usize;
    let dt = 1.0 / SR;
    let damp = (1.0 / q).clamp(0.0, 1.0);
    let mut low = 0.0f32;
    let mut band = 0.0f32;
    for i in 0..steps {
        let tt = i as f32 * dt;
        let u = (tt / LEN).min(1.0);
        let c = top * (bottom / top).powf(u.powf(curve));
        // The state-variable filter is stable while f stays below ~1.4;
        // clamped rather than trusted, because a sweep that starts near
        // Nyquist would otherwise blow up into a full-scale click.
        let f = (2.0 * (std::f32::consts::PI * c / SR).sin()).clamp(0.0, 1.4);
        let x = noise_at(tt);
        let high = x - low - damp * band;
        band += f * high;
        low += f * band;
    }
    band
}

/// Attack then exponential decay, in normalised time.
fn envelope(u: f32, attack: f32, decay: f32) -> f32 {
    if u < attack {
        u / attack
    } else {
        (-(u - attack) * decay).exp()
    }
}

// ---------------------------------------------------------------------
// WAV output
// ---------------------------------------------------------------------

/// Write 16-bit mono PCM.
///
/// ⚠️ HAND-ROLLED, NO CRATE. core has no dev-dependencies and the game
/// crates depend on core and nothing else — dump_frame writes its own
/// PNG for the same reason. A WAV header is 44 bytes; adding a
/// dependency to avoid writing them would cost more than it saves.
fn write_wav(path: &Path, samples: &[f32]) {
    let mut w = BufWriter::new(File::create(path).expect("create wav"));
    let n = samples.len() as u32;
    let data_bytes = n * 2;
    let sr = SR as u32;

    w.write_all(b"RIFF").unwrap();
    w.write_all(&(36 + data_bytes).to_le_bytes()).unwrap();
    w.write_all(b"WAVEfmt ").unwrap();
    w.write_all(&16u32.to_le_bytes()).unwrap(); // fmt chunk size
    w.write_all(&1u16.to_le_bytes()).unwrap(); // PCM
    w.write_all(&1u16.to_le_bytes()).unwrap(); // mono
    w.write_all(&sr.to_le_bytes()).unwrap();
    w.write_all(&(sr * 2).to_le_bytes()).unwrap(); // byte rate
    w.write_all(&2u16.to_le_bytes()).unwrap(); // block align
    w.write_all(&16u16.to_le_bytes()).unwrap(); // bits
    w.write_all(b"data").unwrap();
    w.write_all(&data_bytes.to_le_bytes()).unwrap();

    for s in samples {
        // ⚠️ CLAMP BEFORE SCALING. A candidate that overshoots would wrap
        // around to full-scale negative and read as a vicious click,
        // which would be judged as the sound rather than as an error.
        let v = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        w.write_all(&v.to_le_bytes()).unwrap();
    }
}
