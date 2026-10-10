//! Render a stretch of play as the mixer hears it, for the W5 LEVEL PASS.
//!
//!   cargo run --release -p omarcade-warden --example mix_scene -- <outdir>
//!
//! ★ WHY A SCENE AND NOT A TABLE. Every voice was judged alone, by A/B
//! against its ROM render. Balance is a different question — it is about
//! what sits on top of what — and it can only be heard with the sounds
//! overlapping the way the game overlaps them (Brian, 2026-10-09: "don't
//! cut sounds off...it sounds really good" — Warden mixes, the board did
//! not).
//!
//! It writes two mixes of the SAME ~14 s of scripted play:
//!   · scene-current.wav  — every voice at its shipped level;
//!   · scene-original.wav — every voice moved to where it sat RELATIVE TO
//!     THE LASER on the real board, measured from the ROM renders (all of
//!     the original's sounds drove one full-scale DAC, so their relative
//!     loudness IS its mix). ⚠️ Both are scaled to the same overall
//!     loudness, so the louder one does not win the A/B by being louder.
//! And it prints, for each, the RAW summed peak at the default volume
//! (0.5) and at full volume. ⚠️ This is the sum BEFORE the core mixer's
//! limiter (LIMIT_CEILING 0.95, added in the same pass): in the game,
//! anything listed as over 1.0 is held at the ceiling, not clipped.
//!
//! ★ Since the W5 level pass (Brian: "balance use original") the shipped
//! levels ARE the original balance: the "move" column should read ~0.
//!
//! One voice per sound, retriggered, exactly as `register_sound` gives
//! the game: a second Lander dying restarts the Lander voice.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use omarcade_core::audio::{Voice, VoiceParams};

/// ★ THE REAL sound.rs, included rather than copied (see laser_check).
#[allow(dead_code)]
#[path = "../src/sound.rs"]
mod sound;

const SR: f32 = 48_000.0;
const LEN: f32 = 14.5;
/// The game's default master volume (core audio DEFAULT_VOLUME).
const DEFAULT_MASTER: f32 = 0.5;
const FIRE_INTERVAL: f32 = 0.16;

/// One sound in the scene: its name, a fresh voice, when it fires, and
/// where the ORIGINAL put it relative to the laser, in dB (measured from
/// the ROM renders — W5 level pass, 2026-10-09). `None`: no original to
/// follow (ours, or Brian's own), so it keeps its current balance.
struct Track {
    name: &'static str,
    voice: Box<dyn Voice>,
    fires: Vec<f32>,
    original_vs_laser_db: Option<f32>,
}

fn burst(from: f32, to: f32) -> Vec<f32> {
    let mut v = Vec::new();
    let mut t = from;
    while t < to {
        v.push(t);
        t += FIRE_INTERVAL;
    }
    v
}

fn tracks() -> Vec<Track> {
    let mut laser = burst(1.0, 2.6);
    laser.extend(burst(4.0, 5.1));
    laser.extend(burst(7.0, 8.6));
    laser.extend(burst(9.5, 10.6));
    let t = |name, voice: Box<dyn Voice>, fires: &[f32], db| Track {
        name,
        voice,
        fires: fires.to_vec(),
        original_vs_laser_db: db,
    };
    vec![
        t("laser", Box::new(sound::turbo_laser()), &laser, Some(0.0)),
        t("game start", Box::new(sound::game_start()), &[0.0], Some(-3.4)),
        // Brian's Warp; the original's arrival is APPEAR.
        t("warp", Box::<sound::Warp>::default(), &[0.2, 6.8], Some(-0.4)),
        t("lander hit", Box::new(sound::lander_hit()), &[1.5, 2.2, 4.5, 7.6], Some(-3.8)),
        // Brian's own; the original's Mutant dies on CANNON.
        t("mutant boom", Box::<sound::MutantBoom>::default(), &[8.2], None),
        t("bomber hit", Box::new(sound::bomber_hit()), &[4.8], Some(-0.3)),
        t("pod hit", Box::new(sound::pod_hit()), &[7.3], Some(-2.1)),
        t("baiter hit", Box::new(sound::baiter_hit()), &[10.2], Some(-1.8)),
        t("lander shot", Box::new(sound::lander_shot()), &[3.0, 6.5], Some(-3.5)),
        t("mutant shot", Box::new(sound::mutant_shot()), &[8.0], Some(-4.8)),
        t("swarmer shot", Box::new(sound::swarmer_shot()), &[9.6, 9.9], Some(-5.0)),
        t("grab", Box::new(sound::grab()), &[3.4], Some(-3.6)),
        t("catch", Box::new(sound::catch()), &[5.5], Some(-4.6)),
        t("set down", Box::new(sound::set_down()), &[6.0], Some(-0.3)),
        t("fusion", Box::new(sound::fusion()), &[8.8], Some(-1.1)),
        t("extra life", Box::new(sound::extra_life_short()), &[6.6], Some(0.0)),
        // ★ Brian's rule outranks the board: a shot person stays under a
        // Lander's death (the original's LITE was louder than its HBEV).
        t("person crackle", Box::new(sound::person_crackle()), &[2.9], Some(-4.8)),
        // Crackle first, so the loudest stretch is the LITE's.
        t("smart bomb", Box::<sound::SmartBomb>::default(), &[11.2], Some(-0.4)),
        t("hyperspace", Box::<sound::Hyperspace>::default(), &[5.9], None),
    ]
}

/// Loudness the way the W5 table measures it: the loudest 300 ms RMS.
fn loudness(x: &[f32]) -> f32 {
    let w = (0.3 * SR) as usize;
    let hop = (0.05 * SR) as usize;
    let mut best = 0.0f32;
    let mut k = 0;
    while k + w <= x.len().max(w) {
        let end = (k + w).min(x.len());
        let seg = &x[k..end];
        let r = (seg.iter().map(|v| v * v).sum::<f32>() / w as f32).sqrt();
        best = best.max(r);
        k += hop;
    }
    best
}

fn db(x: f32) -> f32 {
    20.0 * x.max(1e-9).log10()
}

fn main() {
    let dir: PathBuf = std::env::args().nth(1).unwrap_or_else(|| ".".into()).into();

    // Each sound alone, once, for its current loudness.
    let alone: Vec<f32> = tracks()
        .into_iter()
        .map(|mut t| loudness(&render(t.voice.as_mut(), 3.5, &[0.0])))
        .collect();
    let laser_now = alone[0];

    // Each track rendered on its own timeline.
    let mut rendered: Vec<(Track, Vec<f32>)> = Vec::new();
    for mut t in tracks() {
        let s = render(t.voice.as_mut(), LEN, &t.fires);
        rendered.push((t, s));
    }
    let mut thrust = render_thrust(LEN, &[(0.5, 6.0), (8.0, 11.0)]);

    println!("{:16} {:>9} {:>9} {:>8}", "sound", "now vs laser", "original", "move");
    let mut gains = Vec::new();
    for ((t, _), now) in rendered.iter().zip(&alone) {
        let now_db = db(*now) - db(laser_now);
        let g = match t.original_vs_laser_db {
            Some(want) => 10f32.powf((want - now_db) / 20.0),
            None => 1.0,
        };
        let want = t.original_vs_laser_db.map_or("(keep)".to_string(), |w| format!("{w:+.1}"));
        println!("{:16} {:>+9.1} {:>9} {:>+8.1}", t.name, now_db, want, db(g));
        gains.push(g);
    }
    // Thrust: the original's sat 7.3 dB under its laser.
    let thrust_now = db(loudness(&thrust)) - db(laser_now);
    let thrust_gain = 10f32.powf((-7.3 - thrust_now) / 20.0);
    println!("{:16} {:>+9.1} {:>9} {:>+8.1}", "thrust", thrust_now, "-7.3", db(thrust_gain));

    let mix = |gains: &[f32], thrust_gain: f32, thrust: &[f32]| {
        let n = (LEN * SR) as usize;
        let mut out = vec![0.0f32; n];
        for ((_, s), g) in rendered.iter().zip(gains) {
            for (o, v) in out.iter_mut().zip(s) {
                *o += v * g;
            }
        }
        for (o, v) in out.iter_mut().zip(thrust) {
            *o += v * thrust_gain;
        }
        out
    };
    let current = mix(&vec![1.0; gains.len()], 1.0, &thrust);
    let mut original = mix(&gains, thrust_gain, &thrust);
    thrust.clear();

    // Same overall loudness, so the A/B is about balance, not volume.
    let rms = |x: &[f32]| (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt();
    let k = rms(&current) / rms(&original);
    println!("equal-loudness scale k = {k:.4} ({:+.2} dB); final factor per voice = move × k:", db(k));
    for ((t, _), g) in rendered.iter().zip(&gains) {
        println!("    {:16} × {:.4}", t.name, g * k);
    }
    println!("    {:16} × {:.4}", "thrust", thrust_gain * k);
    for v in original.iter_mut() {
        *v *= k;
    }

    for (name, m) in [("scene-current", &current), ("scene-original", &original)] {
        let peak = m.iter().fold(0.0f32, |a, v| a.max(v.abs()));
        let clipped = m.iter().filter(|v| v.abs() > 1.0).count() as f32 / m.len() as f32;
        // Where it clips, in 0.5 s bins, so the culprit can be named.
        let mut bins: Vec<(f32, usize)> = Vec::new();
        for (i, v) in m.iter().enumerate() {
            if v.abs() > 1.0 {
                let b = (i as f32 / SR * 2.0).floor() / 2.0;
                match bins.last_mut() {
                    Some((t, n)) if *t == b => *n += 1,
                    _ => bins.push((b, 1)),
                }
            }
        }
        println!(
            "{name}: raw peak {:.2} at default volume, {:.2} at full ({:.2}% of samples over 1.0 — the core limiter holds these at 0.95)",
            peak * DEFAULT_MASTER,
            peak,
            clipped * 100.0
        );
        for (t, n) in bins {
            println!("    over 1.0 at full volume (limited): {t:.1}–{:.1} s, {n} samples", t + 0.5);
        }
        // Written at the default volume, as a first launch would play it.
        let at_default: Vec<f32> = m.iter().map(|v| v * DEFAULT_MASTER).collect();
        write_wav(&dir.join(format!("{name}.wav")), &at_default);
    }
}

/// Render a voice for `seconds`, retriggering it at each time in `fires`.
fn render(v: &mut dyn Voice, seconds: f32, fires: &[f32]) -> Vec<f32> {
    let total = (seconds * SR) as usize;
    let mut out = Vec::with_capacity(total);
    let mut block = [0.0f32; 256];
    let mut next = 0usize;
    while out.len() < total {
        let t = out.len() as f32 / SR;
        while next < fires.len() && fires[next] <= t {
            v.retrigger(1.0, 1.0);
            next += 1;
        }
        v.render(&mut block, VoiceParams::SILENT, SR);
        out.extend_from_slice(&block);
    }
    out.truncate(total);
    out
}

/// The engine, driven the way the game eases it (see laser_check).
fn render_thrust(seconds: f32, holds: &[(f32, f32)]) -> Vec<f32> {
    const EXHAUST_ATTACK: f32 = 14.0;
    const EXHAUST_RELEASE: f32 = 6.0;
    const FIXED_DT: f32 = 1.0 / 120.0;
    let mut v = sound::Thrust::new();
    let total = (seconds * SR) as usize;
    let mut out = Vec::with_capacity(total);
    let mut block = [0.0f32; 256];
    let (mut exhaust, mut stepped) = (0.0f32, 0.0f32);
    while out.len() < total {
        let t = out.len() as f32 / SR;
        while stepped < t {
            let want = if holds.iter().any(|&(a, b)| stepped >= a && stepped < b) { 1.0 } else { 0.0 };
            let rate = if want > exhaust { EXHAUST_ATTACK } else { EXHAUST_RELEASE };
            exhaust = (exhaust + (want - exhaust).clamp(-rate * FIXED_DT, rate * FIXED_DT)).clamp(0.0, 1.0);
            stepped += FIXED_DT;
        }
        v.render(&mut block, VoiceParams::thrust(exhaust), SR);
        out.extend_from_slice(&block);
    }
    out.truncate(total);
    out
}

/// 16-bit mono PCM, hand-rolled (core has no dev-dependencies).
fn write_wav(path: &Path, samples: &[f32]) {
    let mut w = BufWriter::new(File::create(path).expect("create wav"));
    let n = samples.len() as u32;
    let sr = SR as u32;
    w.write_all(b"RIFF").unwrap();
    w.write_all(&(36 + n * 2).to_le_bytes()).unwrap();
    w.write_all(b"WAVEfmt ").unwrap();
    w.write_all(&16u32.to_le_bytes()).unwrap();
    w.write_all(&1u16.to_le_bytes()).unwrap();
    w.write_all(&1u16.to_le_bytes()).unwrap();
    w.write_all(&sr.to_le_bytes()).unwrap();
    w.write_all(&(sr * 2).to_le_bytes()).unwrap();
    w.write_all(&2u16.to_le_bytes()).unwrap();
    w.write_all(&16u16.to_le_bytes()).unwrap();
    w.write_all(b"data").unwrap();
    w.write_all(&(n * 2).to_le_bytes()).unwrap();
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        w.write_all(&v.to_le_bytes()).unwrap();
    }
    println!("wrote {}", path.display());
}
