//! The laser and the explosion.
//!
//! ★ BRIAN NAMED SOUND AS THE THING HE MOST WANTS TO NAIL, and he was
//! right about the mechanism before any of this was written: "pretty sure
//! uses some kind of phaser to shift frequencys". Something is swept hard
//! downward in both of these. It is not a sample, it was never a sample,
//! and reproducing it means sweeping rather than finding a wav.
//!
//! ⚠️ WHAT gets swept is the part that took two attempts. The laser was
//! first built as a swept OSCILLATOR, and Brian's verdict after listening
//! was "our laser is higher pitched and original is more white noise" —
//! so it is now a swept FILTER over noise, built by ear in the playground
//! rather than derived. The `Zap` doc below has the whole story.
//!
//! These two land in S5 rather than waiting for the S13 sound pass for a
//! simple reason: a shooter with silent guns does not read as a game, so
//! judging whether S5 "feels right" without them would be judging
//! something nobody would ship.
//!
//! ⚠️ BOTH ARE ONE-SHOTS, following the pattern proven by the racer's
//! `Crash` (games/omarcade-racer/src/sound.rs:806): `alive()` reports
//! false when finished so the mixer retires them, and `retrigger()`
//! restarts from the beginning when the same sound fires again while
//! still ringing. Voices render on the AUDIO THREAD — no allocation, no
//! `rand`, no panics.

use std::f32::consts::PI;

use omarcade_core::audio::{Voice, VoiceParams};

// ---------------------------------------------------------------------
// The laser
// ---------------------------------------------------------------------

// ★★ BUILT BY EAR, NOT DERIVED. Brian designed this voice in
// tools/sound-playground.html and exported it; the constants below are
// his, verbatim, and the playground state rides in a comment at the end
// of this block so the design can be pasted back in and tweaked further.
// ⚠️ DO NOT "improve" these numbers by reasoning about them. They are a
// listening result. The playground round-trips, so the way to change
// this sound is to open the tool, not to edit the file.

/// How long the whole sound lasts, in seconds.
///
/// ⚠️ THIS IS 3.6x THE FIRE INTERVAL (shot.rs FIRE_INTERVAL = 0.16) AND
/// THAT IS DELIBERATE. One `Laser` voice is registered, so a new shot
/// `retrigger()`s it and cuts the previous one dead at 160 ms — on held
/// fire you hear only the top ~28% of the sweep, and the fall to 260 Hz
/// never arrives. Brian was shown exactly that trade and chose it: the
/// truncation IS the rapid-fire texture, and the full tail is what a
/// single shot is for. ⇒ Do not "fix" this by shortening the sound or by
/// adding voices without asking him.
const ZAP_LEN: f32 = 0.580;
const ZAP_ATTACK: f32 = 0.020;
const ZAP_DECAY: f32 = 0.700;
const ZAP_LEVEL: f32 = 0.550;

/// Defender's laser: white noise through a hard downward bandpass sweep.
///
/// ★ THE MECHANISM CHANGED HERE, NOT THE TUNING. This voice used to be a
/// pitched oscillator (sine+square swept 1850→180 Hz) and Brian's verdict
/// on it was "our laser is higher pitched and original is more white
/// noise" — a judgement about METHOD. No value of a top-frequency
/// constant fixes a tone that should not be a tone, so the oscillator is
/// gone and what sweeps now is a FILTER over noise.
///
/// ★★ AND IT IS THE RACER'S TYRE-SQUEAL FINDING RUNNING BACKWARDS. That
/// one was filtered noise and needed an oscillator, because a pitch needs
/// energy in a LINE (see games/omarcade-racer/src/sound.rs:328). This is
/// the mirror image: the laser needed to STOP being a line.
///
/// ⚠️ The real hardware is a 6802 writing 8-bit samples straight to a DAC
/// — it could emit anything, and nobody has published the actual laser
/// routine. This is an informed reading confirmed by Brian's ear, which
/// is the authority the analysis does not have.
pub struct Zap {
    t: f32,
    gain: f32,
    alive: bool,
    low: f32,
    band: f32,
    noise: u32,
}

impl Default for Zap {
    fn default() -> Self {
        Self::new()
    }
}

impl Zap {
    pub fn new() -> Self {
        Self {
            t: 0.0,
            gain: 1.0,
            alive: false,
            low: 0.0,
            band: 0.0,
            noise: 0x1234_5678,
        }
    }

    /// xorshift32 — deterministic, allocation-free, audio-thread safe.
    ///
    /// ⚠️ NOT `rand`. Voices render on the audio thread, where an
    /// allocation or a lock is a dropout.
    fn white(&mut self) -> f32 {
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        (self.noise as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

impl Voice for Zap {
    fn render(&mut self, out: &mut [f32], _params: VoiceParams, sample_rate: f32) {
        if !self.alive {
            out.fill(0.0);
            return;
        }
        let dt = 1.0 / sample_rate;

        for sample in out.iter_mut() {
            if self.t >= ZAP_LEN {
                *sample = 0.0;
                self.alive = false;
                continue;
            }
            let u = self.t / ZAP_LEN;
            let mut v = 0.0f32;

            // noise, mix 1.000
            v += self.white() * 1.000;

            // A two-pole state-variable filter, corner sweeping
            // 1940 Hz → 260 Hz across the life of the sound. The band
            // output is the one taken: a bandpass moving down is what
            // turns a flat hiss into a zap.
            let c = 1940.0 * 0.1340_f32.powf(u);
            // ⚠️ CLAMPED: a corner near Nyquist makes this blow up. The
            // SVF is only stable while g stays well below 2, and an
            // unclamped corner at a low sample rate walks straight past
            // it — the filter self-oscillates to NaN and the mixer
            // spreads the NaN across every voice.
            let g = (2.0 * (PI * c.min(sample_rate * 0.45) / sample_rate).sin()).min(1.4);
            let damp = (1.0 / 3.600_f32).min(1.0);
            let high = v - self.low - damp * self.band;
            self.band += g * high;
            self.low += g * self.band;
            v = self.band;

            let env = if u < ZAP_ATTACK {
                u / ZAP_ATTACK
            } else {
                (-(u - ZAP_ATTACK) * ZAP_DECAY).exp()
            };

            *sample = v * env * ZAP_LEVEL * self.gain;
            self.t += dt;
        }
    }

    fn alive(&self) -> bool {
        self.alive
    }

    fn retrigger(&mut self, gain: f32, _pitch: f32) {
        self.t = 0.0;
        // ⚠️ THE NOISE SEQUENCE IS NOT RESET — successive shots draw from
        // where the last one left off, so each gets a slightly different
        // attack for free, which is what the hardware did by not caring.
        // ★ THE FILTER STATE *IS* CLEARED, and that is not the same
        // decision: stale `low`/`band` would make the first millisecond
        // of a shot depend on the one before it, which is a click, not
        // character.
        self.gain = gain.clamp(0.0, 1.0);
        self.alive = true;
        self.low = 0.0;
        self.band = 0.0;
    }
}

/// The shipped laser. ★ Named `Laser` so the game wires up unchanged;
/// `Zap` is the name the playground exported it under.
pub type Laser = Zap;

// ── playground state, for round-tripping ──
// Paste this line back into tools/sound-playground.html to keep tweaking
// this sound by ear.
// PLAYGROUND: {"len":0.58,"attack":0.02,"decay":0.7,"level":0.55,"filter":{"mode":"bp","from":1940,"to":260,"q":3.6},"oscs":[{"on":true,"wave":"noise","from":1,"to":1,"curve":1,"duty":0.5,"dutyTo":0.5,"mix":1}]}

// ---------------------------------------------------------------------
// The explosions
// ---------------------------------------------------------------------

// ★★ FOUR DEATHS, FOUR SOUNDS — AND UNTIL NOW THERE WAS ONE.
//
// `killed_this_frame` fired the same boom for a Lander dying, a MUTANT
// dying, a Humanoid you shot by mistake, and YOUR OWN SHIP exploding.
// Four events with completely different meanings, one noise. Brian's
// note is what split them: "this is the explosion (it's subtle like og)
// for landers...the mutants get a more intense explosion".
//
// ⚠️ ONLY `Boom` IS BRIAN'S. He built the Lander explosion by ear in the
// playground against the original machine and its constants are his,
// verbatim. THE OTHER THREE ARE PLACEHOLDERS I SHAPED, NOT SOUNDS HE HAS
// APPROVED — each is the Lander recipe bent toward what its event MEANS,
// and each is meant to be replaced the same way the laser was: open the
// playground, build it by ear, paste the export back.
// ⇒ DO NOT treat the three placeholder constant sets as settled the way
// Brian's are. They are a starting point for his ear, and the PLAYGROUND
// comment under each one is how he picks it up.

/// How long the blast lasts, in seconds.
const BOOM_LEN: f32 = 0.580;

/// ★ SUBTLE, AND DELIBERATELY SO. Brian checked the original machine:
/// a Lander going up is not a spectacle, it is a soft crump. The whole
/// character is a slow saw under a lowpass corner falling 1060 -> 260 Hz,
/// with a long lazy decay that lets it settle rather than snap.
const BOOM_ATTACK: f32 = 0.090;
const BOOM_DECAY: f32 = 2.100;
const BOOM_LEVEL: f32 = 0.250;

/// A Lander dying. ★ BRIAN'S, BUILT BY EAR AGAINST THE ORIGINAL.
///
/// ⚠️ THIS IS A SAW THROUGH A SWEEPING LOWPASS, NOT RING-MODULATED NOISE.
/// The previous Boom was noise multiplied by a 62 Hz carrier, on the
/// theory that ring modulation gave the 1981 explosions their metallic
/// quality. That theory is gone — Brian went and listened to the machine.
/// A 40 Hz saw is almost all harmonics, and dragging a resonant lowpass
/// down across them is what actually produces the crump.
/// ⇒ THE `low` OUTPUT IS TAKEN, not `band`. The laser wants the bandpass
/// (energy in a moving sliver); an explosion wants everything BELOW the
/// corner, which is what makes it a body rather than a whistle.
pub struct Boom {
    t: f32,
    gain: f32,
    alive: bool,
    phase: [f32; 1],
    low: f32,
    band: f32,
}

impl Default for Boom {
    fn default() -> Self {
        Self::new()
    }
}

impl Boom {
    pub fn new() -> Self {
        Self { t: 0.0, gain: 1.0, alive: false, phase: [0.0; 1], low: 0.0, band: 0.0 }
    }
}

impl Voice for Boom {
    fn render(&mut self, out: &mut [f32], _params: VoiceParams, sample_rate: f32) {
        if !self.alive {
            out.fill(0.0);
            return;
        }
        let dt = 1.0 / sample_rate;

        for sample in out.iter_mut() {
            if self.t >= BOOM_LEN {
                *sample = 0.0;
                self.alive = false;
                continue;
            }
            let u = self.t / BOOM_LEN;
            let mut v = 0.0f32;

            // saw, mix 1.000
            {
                let hz = 40.0 * 1.0000_f32.powf(u.powf(0.920));
                // ⚠️ INTEGRATE the phase. Recomputing sin(TAU*hz*t)
                // with a changing hz clicks on every sample.
                self.phase[0] = (self.phase[0] + hz * dt).fract();
                let p = self.phase[0];
                v += (2.0 * p - 1.0) * 1.000;
            }

            // A two-pole state-variable filter, corner sweeping
            // 1060 Hz -> 260 Hz.
            let c = 1060.0 * 0.2453_f32.powf(u);
            // ⚠️ CLAMPED: a corner near Nyquist makes this blow up.
            let g = (2.0 * (PI * c.min(sample_rate * 0.45) / sample_rate).sin()).min(1.4);
            let damp = (1.0 / 5.700_f32).min(1.0);
            let high = v - self.low - damp * self.band;
            self.band += g * high;
            self.low += g * self.band;
            v = self.low;

            let env = if u < BOOM_ATTACK {
                u / BOOM_ATTACK
            } else {
                (-(u - BOOM_ATTACK) * BOOM_DECAY).exp()
            };

            *sample = v * env * BOOM_LEVEL * self.gain;
            self.t += dt;
        }
    }

    fn alive(&self) -> bool {
        self.alive
    }

    fn retrigger(&mut self, gain: f32, _pitch: f32) {
        self.t = 0.0;
        // ⚠️ THE PHASE IS NOT RESET — successive blasts get slightly
        // different attacks for free, which is what the hardware did
        // by not caring.
        self.gain = gain.clamp(0.0, 1.0);
        self.alive = true;
        // ★ THE FILTER STATE *IS* CLEARED. Stale low/band would make the
        // first milliseconds of a blast depend on the previous one,
        // which is a click, not character.
        self.low = 0.0;
        self.band = 0.0;
    }
}

// ── playground state, for round-tripping ──
// Paste this line back into tools/sound-playground.html to keep tweaking
// this sound by ear.
// PLAYGROUND: {"len":0.58,"attack":0.09,"decay":2.1,"level":0.25,"filter":{"mode":"lp","from":1060,"to":260,"q":5.7},"oscs":[{"on":true,"wave":"saw","from":40,"to":40,"curve":0.92,"duty":0.04,"dutyTo":0.08,"mix":1}]}

// ---------------------------------------------------------------------

/// ⚠️ PLACEHOLDER — NOT BUILT BY EAR. Brian asked for "a more intense
/// explosion" for Mutants and has not yet designed it.
///
/// The shape is the Lander's, bent toward violence rather than settling:
/// the corner starts nearly twice as high and stays open longer (a
/// brighter, harsher body), the saw sits higher, the decay is faster so
/// it BITES instead of sagging, and the level is up. A Mutant is the
/// thing that hunts you; its death should be a bang, not a crump.
const MUTANT_BOOM_LEN: f32 = 0.480;
const MUTANT_BOOM_ATTACK: f32 = 0.020;
const MUTANT_BOOM_DECAY: f32 = 3.400;
const MUTANT_BOOM_LEVEL: f32 = 0.360;

pub struct MutantBoom {
    t: f32,
    gain: f32,
    alive: bool,
    phase: [f32; 1],
    low: f32,
    band: f32,
}

impl Default for MutantBoom {
    fn default() -> Self {
        Self::new()
    }
}

impl MutantBoom {
    pub fn new() -> Self {
        Self { t: 0.0, gain: 1.0, alive: false, phase: [0.0; 1], low: 0.0, band: 0.0 }
    }
}

impl Voice for MutantBoom {
    fn render(&mut self, out: &mut [f32], _params: VoiceParams, sample_rate: f32) {
        if !self.alive {
            out.fill(0.0);
            return;
        }
        let dt = 1.0 / sample_rate;

        for sample in out.iter_mut() {
            if self.t >= MUTANT_BOOM_LEN {
                *sample = 0.0;
                self.alive = false;
                continue;
            }
            let u = self.t / MUTANT_BOOM_LEN;
            let mut v = 0.0f32;

            {
                let hz = 66.0 * 1.0000_f32.powf(u.powf(0.920));
                self.phase[0] = (self.phase[0] + hz * dt).fract();
                let p = self.phase[0];
                v += (2.0 * p - 1.0) * 1.000;
            }

            let c = 1900.0 * 0.2100_f32.powf(u);
            let g = (2.0 * (PI * c.min(sample_rate * 0.45) / sample_rate).sin()).min(1.4);
            let damp = (1.0 / 4.200_f32).min(1.0);
            let high = v - self.low - damp * self.band;
            self.band += g * high;
            self.low += g * self.band;
            v = self.low;

            let env = if u < MUTANT_BOOM_ATTACK {
                u / MUTANT_BOOM_ATTACK
            } else {
                (-(u - MUTANT_BOOM_ATTACK) * MUTANT_BOOM_DECAY).exp()
            };

            *sample = v * env * MUTANT_BOOM_LEVEL * self.gain;
            self.t += dt;
        }
    }

    fn alive(&self) -> bool {
        self.alive
    }

    fn retrigger(&mut self, gain: f32, _pitch: f32) {
        self.t = 0.0;
        self.gain = gain.clamp(0.0, 1.0);
        self.alive = true;
        self.low = 0.0;
        self.band = 0.0;
    }
}

// PLAYGROUND: {"len":0.48,"attack":0.02,"decay":3.4,"level":0.36,"filter":{"mode":"lp","from":1900,"to":399,"q":4.2},"oscs":[{"on":true,"wave":"saw","from":66,"to":66,"curve":0.92,"duty":0.04,"dutyTo":0.08,"mix":1}]}

// ---------------------------------------------------------------------

/// ⚠️ PLACEHOLDER — NOT BUILT BY EAR. ★ YOUR OWN DEATH, and it used to
/// be the SAME SOUND as a Lander's.
///
/// That was the worst of the four collisions: the game gave identical
/// feedback for "you scored" and "you lost a life". This is shaped to be
/// the worst sound in the game — the longest, the lowest, a slow decay
/// that outlasts the others so it hangs there. An explosion you hear
/// happening TO you rather than one you caused.
const SHIP_BOOM_LEN: f32 = 1.100;
const SHIP_BOOM_ATTACK: f32 = 0.010;
const SHIP_BOOM_DECAY: f32 = 1.200;
/// ⚠️ 0.30, NOT the 0.42 this started at. At 0.42 the rendered peak was
/// 1.03 — CLIPPING, because a resonant lowpass at Q 6.4 adds gain the
/// level constant does not account for. The WAV writer clamps and would
/// have hidden it; the mixer would not have.
/// ⇒ ★ A `_LEVEL` CONSTANT IS NOT THE PEAK. Render and measure.
const SHIP_BOOM_LEVEL: f32 = 0.300;

pub struct ShipBoom {
    t: f32,
    gain: f32,
    alive: bool,
    phase: [f32; 1],
    low: f32,
    band: f32,
}

impl Default for ShipBoom {
    fn default() -> Self {
        Self::new()
    }
}

impl ShipBoom {
    pub fn new() -> Self {
        Self { t: 0.0, gain: 1.0, alive: false, phase: [0.0; 1], low: 0.0, band: 0.0 }
    }
}

impl Voice for ShipBoom {
    fn render(&mut self, out: &mut [f32], _params: VoiceParams, sample_rate: f32) {
        if !self.alive {
            out.fill(0.0);
            return;
        }
        let dt = 1.0 / sample_rate;

        for sample in out.iter_mut() {
            if self.t >= SHIP_BOOM_LEN {
                *sample = 0.0;
                self.alive = false;
                continue;
            }
            let u = self.t / SHIP_BOOM_LEN;
            let mut v = 0.0f32;

            {
                let hz = 28.0 * 1.0000_f32.powf(u.powf(0.920));
                self.phase[0] = (self.phase[0] + hz * dt).fract();
                let p = self.phase[0];
                v += (2.0 * p - 1.0) * 1.000;
            }

            let c = 820.0 * 0.1400_f32.powf(u);
            let g = (2.0 * (PI * c.min(sample_rate * 0.45) / sample_rate).sin()).min(1.4);
            let damp = (1.0 / 6.400_f32).min(1.0);
            let high = v - self.low - damp * self.band;
            self.band += g * high;
            self.low += g * self.band;
            v = self.low;

            let env = if u < SHIP_BOOM_ATTACK {
                u / SHIP_BOOM_ATTACK
            } else {
                (-(u - SHIP_BOOM_ATTACK) * SHIP_BOOM_DECAY).exp()
            };

            *sample = v * env * SHIP_BOOM_LEVEL * self.gain;
            self.t += dt;
        }
    }

    fn alive(&self) -> bool {
        self.alive
    }

    fn retrigger(&mut self, gain: f32, _pitch: f32) {
        self.t = 0.0;
        self.gain = gain.clamp(0.0, 1.0);
        self.alive = true;
        self.low = 0.0;
        self.band = 0.0;
    }
}

// PLAYGROUND: {"len":1.1,"attack":0.01,"decay":1.2,"level":0.3,"filter":{"mode":"lp","from":820,"to":115,"q":6.4},"oscs":[{"on":true,"wave":"saw","from":28,"to":28,"curve":0.92,"duty":0.04,"dutyTo":0.08,"mix":1}]}

// ---------------------------------------------------------------------

/// ⚠️ PLACEHOLDER — NOT BUILT BY EAR. Shooting a Humanoid.
///
/// ★ AND IT SHOULD NOT BE SATISFYING. Brian's spec is DON'T SHOOT THEM,
/// and the game deliberately lets you — "a rule the game quietly refuses
/// to let you break is not a rule anyone ever feels" (main.rs). Giving
/// that the same meaty boom as killing a Lander REWARDS it, which is
/// exactly backwards.
/// ⇒ So: short, thin, high, and over almost before it starts. A mistake
/// noise, not an achievement noise.
const PERSON_BOOM_LEN: f32 = 0.260;
const PERSON_BOOM_ATTACK: f32 = 0.006;
const PERSON_BOOM_DECAY: f32 = 5.200;
const PERSON_BOOM_LEVEL: f32 = 0.180;

pub struct PersonBoom {
    t: f32,
    gain: f32,
    alive: bool,
    phase: [f32; 1],
    low: f32,
    band: f32,
}

impl Default for PersonBoom {
    fn default() -> Self {
        Self::new()
    }
}

impl PersonBoom {
    pub fn new() -> Self {
        Self { t: 0.0, gain: 1.0, alive: false, phase: [0.0; 1], low: 0.0, band: 0.0 }
    }
}

impl Voice for PersonBoom {
    fn render(&mut self, out: &mut [f32], _params: VoiceParams, sample_rate: f32) {
        if !self.alive {
            out.fill(0.0);
            return;
        }
        let dt = 1.0 / sample_rate;

        for sample in out.iter_mut() {
            if self.t >= PERSON_BOOM_LEN {
                *sample = 0.0;
                self.alive = false;
                continue;
            }
            let u = self.t / PERSON_BOOM_LEN;
            let mut v = 0.0f32;

            {
                let hz = 150.0 * 1.0000_f32.powf(u.powf(0.920));
                self.phase[0] = (self.phase[0] + hz * dt).fract();
                let p = self.phase[0];
                v += (2.0 * p - 1.0) * 1.000;
            }

            let c = 1400.0 * 0.5000_f32.powf(u);
            let g = (2.0 * (PI * c.min(sample_rate * 0.45) / sample_rate).sin()).min(1.4);
            let damp = (1.0 / 3.000_f32).min(1.0);
            let high = v - self.low - damp * self.band;
            self.band += g * high;
            self.low += g * self.band;
            v = self.low;

            let env = if u < PERSON_BOOM_ATTACK {
                u / PERSON_BOOM_ATTACK
            } else {
                (-(u - PERSON_BOOM_ATTACK) * PERSON_BOOM_DECAY).exp()
            };

            *sample = v * env * PERSON_BOOM_LEVEL * self.gain;
            self.t += dt;
        }
    }

    fn alive(&self) -> bool {
        self.alive
    }

    fn retrigger(&mut self, gain: f32, _pitch: f32) {
        self.t = 0.0;
        self.gain = gain.clamp(0.0, 1.0);
        self.alive = true;
        self.low = 0.0;
        self.band = 0.0;
    }
}

// PLAYGROUND: {"len":0.26,"attack":0.006,"decay":5.2,"level":0.18,"filter":{"mode":"lp","from":1400,"to":700,"q":3},"oscs":[{"on":true,"wave":"saw","from":150,"to":150,"curve":0.92,"duty":0.04,"dutyTo":0.08,"mix":1}]}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    /// Render a whole voice to a buffer, a block at a time, the way the
    /// mixer does.
    fn render_all(v: &mut dyn Voice, seconds: f32) -> Vec<f32> {
        let mut out = Vec::new();
        let mut block = [0.0f32; 256];
        let blocks = (seconds * SR / 256.0) as usize + 1;
        for _ in 0..blocks {
            v.render(&mut block, VoiceParams::SILENT, SR);
            out.extend_from_slice(&block);
        }
        out
    }

    fn peak(samples: &[f32]) -> f32 {
        samples.iter().fold(0.0f32, |m, s| m.max(s.abs()))
    }

    /// RMS of a slice after a one-pole bandpass at `hz`, used to ask
    /// "how much energy is around here" without pulling in an FFT.
    ///
    /// ⚠️ This replaces a zero-crossing pitch estimator. The old voice
    /// was an oscillator and had a countable pitch; this one is noise,
    /// where only a band measurement says anything true. (Same finding
    /// as the sound scope's: a naive per-window statistic lies about
    /// noise unless you ask it about a BAND.)
    fn band_rms(samples: &[f32], hz: f32) -> f32 {
        let g = 2.0 * (PI * hz / SR).sin();
        let damp = 0.4f32;
        let (mut low, mut band) = (0.0f32, 0.0f32);
        let mut sum = 0.0f64;
        for &s in samples {
            let high = s - low - damp * band;
            band += g * high;
            low += g * band;
            sum += (band as f64) * (band as f64);
        }
        (sum / samples.len().max(1) as f64).sqrt() as f32
    }

    #[test]
    fn a_voice_is_silent_until_it_is_triggered() {
        let mut laser = Laser::new();
        assert!(!laser.alive(), "must not start alive");
        let s = render_all(&mut laser, 0.2);
        assert_eq!(peak(&s), 0.0, "an untriggered voice must be silent");

        let mut boom = Boom::new();
        assert!(!boom.alive());
        let s = render_all(&mut boom, 0.2);
        assert_eq!(peak(&s), 0.0);
    }

    #[test]
    fn the_laser_makes_a_sound_and_then_retires() {
        let mut laser = Laser::new();
        laser.retrigger(1.0, 1.0);
        assert!(laser.alive());

        let s = render_all(&mut laser, ZAP_LEN * 1.5);
        assert!(peak(&s) > 0.05, "the laser was inaudible: peak {}", peak(&s));
        assert!(!laser.alive(), "a one-shot must retire itself");
    }

    /// ★★ THE DEFINING PROPERTY, AND IT IS NOT THE OLD ONE.
    ///
    /// This used to assert that the PITCH falls, measured by counting
    /// zero crossings. That test is gone with the oscillator it was
    /// written for: bandpassed noise has no stable pitch to count, and
    /// a crossing count over noise measures the filter's ringing rate
    /// mixed with hash — it would pass or fail for the wrong reasons.
    ///
    /// ⚠️ THE CLAIM STILL HAS TO BE DEFENDED, so it is restated in the
    /// terms the new mechanism actually has: the ENERGY moves from a
    /// high band to a low band. Early in the sound there must be more
    /// energy up around the top of the sweep than down at the bottom,
    /// and late in the sound that relationship must have INVERTED.
    #[test]
    fn the_laser_sweeps_downward() {
        let mut laser = Laser::new();
        laser.retrigger(1.0, 1.0);
        let s = render_all(&mut laser, ZAP_LEN);
        let n = ((ZAP_LEN * SR) as usize).min(s.len());

        // The sweep runs 1940 Hz -> 260 Hz. Probe well inside each end
        // so the test does not depend on the exact corner values.
        let early = &s[..n / 8];
        let late = &s[n * 3 / 4..n];

        let early_ratio = band_rms(early, 1500.0) / band_rms(early, 300.0).max(1e-9);
        let late_ratio = band_rms(late, 1500.0) / band_rms(late, 300.0).max(1e-9);

        assert!(
            early_ratio > 1.0,
            "the sound does not START high: high/low energy was {early_ratio:.2}"
        );
        assert!(
            late_ratio < early_ratio * 0.5,
            "the energy is not FALLING: high/low went {early_ratio:.2} -> {late_ratio:.2}"
        );
    }

    /// A swept oscillator that recomputes sin(TAU*hz*t) instead of
    /// integrating phase clicks on every sample. A click is a large
    /// sample-to-sample jump, so bound the biggest step.
    #[test]
    fn the_laser_does_not_click() {
        let mut laser = Laser::new();
        laser.retrigger(1.0, 1.0);
        let s = render_all(&mut laser, ZAP_LEN);
        let live: Vec<f32> = s.iter().copied().take((ZAP_LEN * SR) as usize).collect();

        let biggest = live
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max);
        // The square component makes legitimate jumps at the half-phase,
        // so this bounds gross discontinuity rather than demanding
        // perfect smoothness.
        assert!(biggest < 0.5, "sample-to-sample jump of {biggest} suggests a click");
    }

    #[test]
    fn the_explosion_makes_a_sound_and_then_retires() {
        let mut boom = Boom::new();
        boom.retrigger(1.0, 1.0);
        let s = render_all(&mut boom, BOOM_LEN * 1.4);
        assert!(peak(&s) > 0.05, "the explosion was inaudible: peak {}", peak(&s));
        assert!(!boom.alive(), "a one-shot must retire itself");
    }

    /// The blast must be loudest at the start and clearly dying by the
    /// end, or it is a wash rather than an explosion.
    #[test]
    fn the_explosion_decays() {
        let mut boom = Boom::new();
        boom.retrigger(1.0, 1.0);
        let s = render_all(&mut boom, BOOM_LEN);
        let n = (BOOM_LEN * SR) as usize;
        let first = peak(&s[..n / 5]);
        let last = peak(&s[n * 3 / 5..n.min(s.len())]);
        assert!(first > last * 2.0, "did not decay: {first} then {last}");
    }

    /// ★ THE FOUR DEATHS MUST NOT SOUND THE SAME.
    ///
    /// ⚠️ THIS REPLACES `the_explosion_is_ring_modulated_not_just_noise`,
    /// which asserted `RING_MIX > 0.25` — a claim about a mechanism that
    /// no longer exists. The old Boom was noise times a 62 Hz carrier on
    /// the theory that ring modulation gave the 1981 explosions their
    /// metallic quality. Brian went and listened to the actual machine
    /// and the answer was a saw under a falling lowpass. The theory was
    /// wrong, so the test defending it had to go rather than be renamed.
    ///
    /// What replaces it is the property the split actually buys: a
    /// Lander, a Mutant, your ship and a shot Humanoid must be TELLABLE
    /// APART. If a future edit collapses two of them onto the same
    /// numbers, this fails.
    #[test]
    fn the_four_deaths_are_distinguishable() {
        let mut lander = Boom::new();
        let mut mutant = MutantBoom::new();
        let mut ship = ShipBoom::new();
        let mut person = PersonBoom::new();

        lander.retrigger(1.0, 1.0);
        mutant.retrigger(1.0, 1.0);
        ship.retrigger(1.0, 1.0);
        person.retrigger(1.0, 1.0);

        let l = render_all(&mut lander, BOOM_LEN);
        let m = render_all(&mut mutant, MUTANT_BOOM_LEN);
        let s = render_all(&mut ship, SHIP_BOOM_LEN);
        let p = render_all(&mut person, PERSON_BOOM_LEN);

        // Length: the ship's death outlasts everything, the Humanoid is
        // the briefest. This is the loss/mistake ordering, in seconds.
        assert!(
            SHIP_BOOM_LEN > BOOM_LEN,
            "your own death must outlast a Lander's"
        );
        assert!(
            PERSON_BOOM_LEN < BOOM_LEN,
            "shooting a person must be the briefest of them"
        );

        // Brightness: a Mutant bites, a ship rumbles. Measured, not
        // asserted from the constants, so a broken filter is caught too.
        let bright = |v: &[f32]| {
            let n = v.len() / 3;
            band_rms(&v[..n], 1200.0) / band_rms(&v[..n], 200.0).max(1e-9)
        };
        assert!(
            bright(&m) > bright(&s),
            "a Mutant must be brighter than your own death: {:.3} vs {:.3}",
            bright(&m),
            bright(&s)
        );

        // Level: shooting a Humanoid must not be the most satisfying
        // sound in the game. It is a mistake, not an achievement.
        assert!(
            peak(&p) < peak(&l),
            "shooting a person must be quieter than killing a Lander"
        );
        assert!(
            peak(&p) < peak(&m),
            "shooting a person must be quieter than killing a Mutant"
        );
    }

    /// ⚠️ NOTHING MAY CLIP. ★ THIS TEST EXISTS BECAUSE IT HAPPENED:
    /// ShipBoom shipped at LEVEL 0.42 and rendered a peak of 1.03,
    /// because a resonant lowpass at Q 6.4 adds gain the level constant
    /// says nothing about. The WAV writer clamps, so the rendered file
    /// sounded plausible while the real mixer would have hard-clipped.
    /// ⇒ A `_LEVEL` CONSTANT IS NOT THE PEAK. Measure the render.
    #[test]
    fn no_voice_clips_at_full_gain() {
        let cases: [(&str, &mut dyn Voice, f32); 5] = [
            ("laser", &mut Laser::new(), ZAP_LEN),
            ("lander", &mut Boom::new(), BOOM_LEN),
            ("mutant", &mut MutantBoom::new(), MUTANT_BOOM_LEN),
            ("ship", &mut ShipBoom::new(), SHIP_BOOM_LEN),
            ("person", &mut PersonBoom::new(), PERSON_BOOM_LEN),
        ];
        for (name, v, len) in cases {
            v.retrigger(1.0, 1.0);
            let p = peak(&render_all(v, len * 1.1));
            assert!(p <= 1.0, "{name} CLIPS at full gain: peak {p:.3}");
            // And leave headroom, because these share a mixer.
            assert!(p < 0.95, "{name} has no headroom: peak {p:.3}");
        }
    }

    /// Every explosion voice is a one-shot and must retire itself, or the
    /// mixer keeps rendering silence forever.
    #[test]
    fn every_explosion_retires_itself() {
        let cases: [(&str, &mut dyn Voice, f32); 4] = [
            ("lander", &mut Boom::new(), BOOM_LEN),
            ("mutant", &mut MutantBoom::new(), MUTANT_BOOM_LEN),
            ("ship", &mut ShipBoom::new(), SHIP_BOOM_LEN),
            ("person", &mut PersonBoom::new(), PERSON_BOOM_LEN),
        ];
        for (name, v, len) in cases {
            v.retrigger(1.0, 1.0);
            let s = render_all(v, len * 1.4);
            assert!(peak(&s) > 0.02, "{name} was inaudible: peak {}", peak(&s));
            assert!(!v.alive(), "{name} must retire itself");
            for x in s {
                assert!(x.is_finite(), "{name} emitted {x}");
            }
        }
    }

    #[test]
    fn gain_scales_the_sound() {
        let mut loud = Laser::new();
        loud.retrigger(1.0, 1.0);
        let l = peak(&render_all(&mut loud, ZAP_LEN));

        let mut soft = Laser::new();
        soft.retrigger(0.25, 1.0);
        let s = peak(&render_all(&mut soft, ZAP_LEN));

        assert!(s < l * 0.6, "gain did not reduce the level: {l} vs {s}");
    }

    /// Firing again while still ringing must restart, not stack or stall.
    #[test]
    fn retriggering_restarts_the_sound() {
        let mut laser = Laser::new();
        laser.retrigger(1.0, 1.0);
        let mut block = [0.0f32; 256];
        laser.render(&mut block, VoiceParams::SILENT, SR);

        laser.retrigger(1.0, 1.0);
        assert!(laser.alive(), "a retrigger must revive the voice");
        let s = render_all(&mut laser, ZAP_LEN);
        assert!(peak(&s) > 0.05, "a retriggered laser must be audible");
    }

    /// ⚠️ NOTHING ON THE AUDIO THREAD MAY PRODUCE A NaN. One reaches the
    /// output device and the result is a click at best.
    #[test]
    fn no_voice_ever_emits_a_non_finite_sample() {
        let mut laser = Laser::new();
        laser.retrigger(1.0, 1.0);
        let mut boom = Boom::new();
        boom.retrigger(1.0, 1.0);

        for s in render_all(&mut laser, ZAP_LEN * 2.0) {
            assert!(s.is_finite(), "laser emitted {s}");
        }
        for s in render_all(&mut boom, BOOM_LEN * 2.0) {
            assert!(s.is_finite(), "boom emitted {s}");
        }
    }
}
