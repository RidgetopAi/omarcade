//! Hear the SHIPPED laser, exactly as the game plays it.
//!
//! ★ WHY THIS IS NOT sound_lab. The lab renders the five *candidate*
//! recipes as plain `fn(f32) -> f32`, which is right for choosing a
//! direction but cannot exercise the thing that actually ships: a real
//! `Voice`, with its envelope, its filter state, and — the part that
//! matters here — its `retrigger()`.
//!
//! ⚠️ THE TRUNCATION IS THE POINT. `ZAP_LEN` is 0.580 s and
//! `shot::FIRE_INTERVAL` is 0.16 s, and the game registers exactly ONE
//! `Laser` voice. So on held fire each shot cuts the previous one dead
//! after 160 ms and you hear only the top of the sweep. Brian was shown
//! that trade and chose it deliberately — this example is how that choice
//! stays audible instead of becoming a surprise later.
//!
//! cargo run --release -p omarcade-warden --example laser_check -- <outdir>

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use omarcade_core::audio::{Voice, VoiceParams};

/// ★ THE REAL sound.rs, INCLUDED RATHER THAN COPIED.
///
/// ⚠️ warden is a BINARY crate with no library target (which is also
/// why clippy wants `--bins` and not `--lib`), so an example cannot
/// `use omarcade_warden::sound`. sound_lab works around that by
/// restating its candidates standalone — fine for throwaway recipes,
/// WRONG here, because the whole point of this example is to hear the
/// code that actually ships. A copy would drift silently and then lie.
// ⚠️ `dead_code` off for the include only: this example uses `Laser` and
// not `Boom`, so every Boom constant reads as unused here. Silencing it
// at the module keeps a REAL warning visible instead of burying it in
// eleven false ones.
#[allow(dead_code)]
#[path = "../src/sound.rs"]
mod sound;

const SR: f32 = 48_000.0;

/// The real fire interval, from shot.rs. ⚠️ Kept in sync by hand; this is
/// an example, not a test, and the game does not link against it.
const FIRE_INTERVAL: f32 = 0.16;

fn main() {
    let dir: PathBuf = std::env::args().nth(1).unwrap_or_else(|| ".".into()).into();

    // One shot, rendered past its own length so the retire is included.
    let single = render(&mut sound::Laser::new(), 0.8, &[0.0]);
    emit(&dir.join("laser-single.wav"), &single, "one shot, full tail");

    // Five shots at the real fire interval through ONE voice — what a
    // held trigger actually sounds like.
    let fires: Vec<f32> = (0..5).map(|i| i as f32 * FIRE_INTERVAL).collect();
    let rapid = render(&mut sound::Laser::new(), 1.6, &fires);
    emit(&dir.join("laser-rapid.wav"), &rapid, "5 shots @ 0.16s, one voice");

    // ★ THE FOUR DEATHS, each on its own. Only the Lander is Brian's;
    // the other three are placeholders waiting for his ear.
    let deaths: [(&str, &mut dyn Voice, f32, &str); 4] = [
        ("lander", &mut sound::Boom::new(), 0.9, "BRIAN'S — subtle, like the original"),
        ("mutant", &mut sound::MutantBoom::new(), 0.8, "★ BRIAN'S — pure crackle, no body"),
        ("ship", &mut sound::ShipBoom::new(), 1.5, "placeholder — your own death"),
        ("person", &mut sound::PersonBoom::new(), 0.5, "placeholder — you broke the rule"),
    ];
    for (name, v, secs, what) in deaths {
        let s = render(v, secs, &[0.0]);
        emit(&dir.join(format!("boom-{name}.wav")), &s, what);
    }

    // ★ THE WARP-IN. Rendered past its own length so the retire lands.
    let warp = render(&mut sound::Warp::new(), 0.8, &[0.0]);
    emit(&dir.join("warp.wav"), &warp, "★ BRIAN'S — a Lander materialising");

    // ★★ THE SMART BOMB — built from the Williams mechanism, not by ear.
    // A/B against the emulated ROM: LITE ×6 at 64 ms, then CANNON.
    let bomb = render(&mut sound::SmartBomb::new(), 3.2, &[0.0]);
    emit(&dir.join("smartbomb.wav"), &bomb, "Williams LITE x6 + CANNON");

    // ★ HYPERSPACE — ours (the original had none): LITE out, APPEAR in.
    let hyper = render(&mut sound::Hyperspace::new(), 1.0, &[0.0]);
    emit(&dir.join("hyperspace.wav"), &hyper, "LITE out + APPEAR in, ours");

    // ★★ THE ENGINE, DRIVEN THE WAY THE GAME DRIVES IT.
    // ⚠️ NOT at a held constant. `exhaust` is EASED in main.rs — attack
    // 14.0/s against release 6.0/s — and the asymmetry is most of what
    // the sound is: fire catches instantly and dies away. Rendering it
    // at a flat 1.0 would measure a voice nobody ever hears.
    let held = render_thrust(1.2, &[(0.1, 0.9)]);
    emit(&dir.join("thrust-held.wav"), &held, "one long burn, eased in and out");

    // What flying actually sounds like: short taps, the way a player
    // feathers it. ★ THE RELEASE NEVER FINISHES between taps, which is
    // the whole reason the release is slower than the attack.
    let taps = render_thrust(2.0, &[(0.1, 0.25), (0.5, 0.65), (0.9, 1.1), (1.4, 1.5)]);
    emit(&dir.join("thrust-taps.wav"), &taps, "four taps — feathering it");
}

/// Render the thrust voice over a timeline of held intervals, easing
/// `exhaust` exactly the way `Warden::step` does.
///
/// ⚠️ THE EASING CONSTANTS ARE DUPLICATED FROM main.rs AND THAT IS A
/// KNOWN COST. This example cannot `include!` main.rs the way it does
/// sound.rs — main.rs is a binary root with a `fn main`. If the easing
/// is ever retuned, retune it here too or this file starts lying about
/// what the game sounds like.
fn render_thrust(seconds: f32, holds: &[(f32, f32)]) -> Vec<f32> {
    const EXHAUST_ATTACK: f32 = 14.0;
    const EXHAUST_RELEASE: f32 = 6.0;
    // The game eases once per FIXED_DT step, not once per sample.
    const FIXED_DT: f32 = 1.0 / 120.0;

    let mut v = sound::Thrust::new();
    let total = (seconds * SR) as usize;
    let mut out: Vec<f32> = Vec::with_capacity(total);
    let mut block = [0.0f32; 256];
    let mut exhaust = 0.0f32;
    let mut stepped = 0.0f32;

    while out.len() < total {
        let t = out.len() as f32 / SR;
        // Catch the simulation up to this block, one fixed step at a
        // time, so the easing sees the same dt the game gives it.
        while stepped < t {
            let want = if holds.iter().any(|&(a, b)| stepped >= a && stepped < b) {
                1.0
            } else {
                0.0
            };
            let rate = if want > exhaust { EXHAUST_ATTACK } else { EXHAUST_RELEASE };
            exhaust += (want - exhaust).clamp(-rate * FIXED_DT, rate * FIXED_DT);
            exhaust = exhaust.clamp(0.0, 1.0);
            stepped += FIXED_DT;
        }
        v.render(&mut block, VoiceParams::thrust(exhaust), SR);
        out.extend_from_slice(&block);
    }
    out.truncate(total);
    out
}

/// Render a voice for `seconds`, retriggering it at each time in `fires`,
/// a 256-sample block at a time the way the mixer does.
fn render(v: &mut dyn Voice, seconds: f32, fires: &[f32]) -> Vec<f32> {
    let total = (seconds * SR) as usize;
    let mut out = Vec::with_capacity(total);
    let mut block = [0.0f32; 256];
    let mut next = 0usize;

    while out.len() < total {
        // Fire on the block boundary it falls in. The game's own timing
        // is frame-quantised too, so this is not a simplification that
        // changes what you hear.
        let t = out.len() as f32 / SR;
        if next < fires.len() && fires[next] <= t {
            v.retrigger(1.0, 1.0);
            next += 1;
        }
        v.render(&mut block, VoiceParams::SILENT, SR);
        out.extend_from_slice(&block);
    }
    out.truncate(total);
    out
}

fn emit(path: &Path, samples: &[f32], what: &str) {
    write_wav(path, samples);
    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    println!("wrote {} — {what} (peak {peak:.2})", path.display());
}

/// Write 16-bit mono PCM. ⚠️ Hand-rolled, no crate — same reason as
/// sound_lab and dump_frame: core has no dev-dependencies.
fn write_wav(path: &Path, samples: &[f32]) {
    let mut w = BufWriter::new(File::create(path).expect("create wav"));
    let n = samples.len() as u32;
    let data_bytes = n * 2;
    let sr = SR as u32;

    w.write_all(b"RIFF").unwrap();
    w.write_all(&(36 + data_bytes).to_le_bytes()).unwrap();
    w.write_all(b"WAVEfmt ").unwrap();
    w.write_all(&16u32.to_le_bytes()).unwrap();
    w.write_all(&1u16.to_le_bytes()).unwrap();
    w.write_all(&1u16.to_le_bytes()).unwrap();
    w.write_all(&sr.to_le_bytes()).unwrap();
    w.write_all(&(sr * 2).to_le_bytes()).unwrap();
    w.write_all(&2u16.to_le_bytes()).unwrap();
    w.write_all(&16u16.to_le_bytes()).unwrap();
    w.write_all(b"data").unwrap();
    w.write_all(&data_bytes.to_le_bytes()).unwrap();

    for s in samples {
        // ⚠️ CLAMP BEFORE SCALING — an overshoot would wrap to full-scale
        // negative and read as a vicious click, i.e. as the sound.
        let v = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        w.write_all(&v.to_le_bytes()).unwrap();
    }
}
