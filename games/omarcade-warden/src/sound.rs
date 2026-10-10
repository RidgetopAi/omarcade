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
#[allow(dead_code)] // Brian's Boom, retired from play — see `Boom`.
const BOOM_LEN: f32 = 0.580;

/// ★ SUBTLE, AND DELIBERATELY SO. Brian checked the original machine:
/// a Lander going up is not a spectacle, it is a soft crump. The whole
/// character is a slow saw under a lowpass corner falling 1060 -> 260 Hz,
/// with a long lazy decay that lets it settle rather than snap.
#[allow(dead_code)] // Brian's Boom, retired from play — see `Boom`.
const BOOM_ATTACK: f32 = 0.090;
#[allow(dead_code)] // Brian's Boom, retired from play — see `Boom`.
const BOOM_DECAY: f32 = 2.100;
#[allow(dead_code)] // Brian's Boom, retired from play — see `Boom`.
const BOOM_LEVEL: f32 = 0.250;

/// A Lander dying. ★ BRIAN'S, BUILT BY EAR AGAINST THE ORIGINAL.
///
/// ⚠️ RETIRED FROM PLAY 2026-10-09: Brian A/B'd it against the original's
/// HBEV ([`lander_hit`]) and chose HBEV. Kept, unwired and still tested,
/// as his reference and so laser_check can render the comparison.
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
#[allow(dead_code)]
pub struct Boom {
    t: f32,
    gain: f32,
    alive: bool,
    phase: [f32; 1],
    low: f32,
    band: f32,
}

#[allow(dead_code)]
impl Default for Boom {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
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

/// ★★ A MUTANT DYING. BRIAN'S, BUILT BY EAR AGAINST THE ORIGINAL.
///
/// ⚠️⚠️ THIS IS NOT A BOOM. THERE IS NO OSCILLATOR AND NO FILTER.
/// It is PURE CRACKLE — short impulses at random times, nothing else.
/// Every other voice in this file is a waveform shaped by a filter;
/// this one is a scatter of pops, and that is the entire sound.
///
/// ★ THE NAME IS NOW A LIE and is kept only so main.rs needs no edit.
/// A Mutant does not go "boom", it comes apart.
///
/// ⚠️ DO NOT "FIX" THIS BY ADDING A BODY. The placeholder it replaced
/// was a saw under a falling lowpass — the Lander's shape bent toward
/// violence — and I proposed layering crackle ON TOP of exactly that.
/// Brian threw the body out entirely and the result is closer to the
/// machine. That is the third time in three sessions that a confident
/// claim in this file about HOW a sound works was overturned by him
/// going and listening (L067).
///
/// How it works, since nothing else here does this:
/// · DENSITY 960 -> 480 pops/sec, with curve 2.43 — so it holds dense
///   and collapses LATE, rather than thinning evenly.
/// · SNAP FALLS 25ms -> 9ms. The pops start with a tail and end dry,
///   which is what makes it come apart rather than fade.
/// · No filter at all. `mode: off`.
const MUTANT_BOOM_LEN: f32 = 0.575;
const MUTANT_BOOM_ATTACK: f32 = 0.100;
const MUTANT_BOOM_DECAY: f32 = 2.000;
/// ⚠️ 0.45, NOT the 0.300 Brian exported. His level rendered a peak of
/// 0.263 — QUIETER THAN SHOOTING A HUMANOID (0.336), which inverts a
/// design decision the tests encode: killing a Mutant is an achievement
/// and shooting a person is a mistake, so the mistake must never be the
/// more satisfying sound. 0.45 renders 0.395 and changes nothing else
/// about the voice. ⇒ `the_four_deaths_are_distinguishable` caught this.
const MUTANT_BOOM_LEVEL: f32 = 0.450;

pub struct MutantBoom {
    t: f32,
    gain: f32,
    alive: bool,
    noise: u32,
    /// Per-crackle: seconds until the next pop, the amplitude of
    /// the pop ringing down now, and its sign.
    pop_wait: [f32; 1],
    pop_amp: [f32; 1],
    pop_sign: [f32; 1],
}

impl Default for MutantBoom {
    fn default() -> Self {
        Self::new()
    }
}

impl MutantBoom {
    pub fn new() -> Self {
        Self {
            t: 0.0,
            gain: 1.0,
            alive: false,
            noise: 0x1234_5678,
            pop_wait: [0.0; 1],
            pop_amp: [0.0; 1],
            pop_sign: [1.0; 1],
        }
    }

    /// xorshift32 — deterministic, allocation-free, audio-thread safe.
    fn white(&mut self) -> f32 {
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        (self.noise as f32 / u32::MAX as f32) * 2.0 - 1.0
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

            // crackle, mix 0.880
            // ★ Pops, not a waveform. Short impulses at random
            // times, thinning as the sound dies — a continuous
            // oscillator cannot make this shape at any setting.
            // ⚠️ The `u >= 0.000` guard and `/ 1.000` are DEAD (delay is
            // zero) and are kept verbatim so the playground's export
            // regenerates this file byte-for-byte. Do not simplify them
            // or the round-trip stops being checkable.
            if u >= 0.000 {
                let cu = (u - 0.000) / 1.000;
                let rate = 960.0 * 0.5000_f32.powf(cu.powf(2.430));
                self.pop_wait[0] -= dt;
                if self.pop_wait[0] <= 0.0 {
                    // Exponential gaps. Evenly spaced pops sound
                    // like a machine, not like fire.
                    let r = (self.white() + 1.0) * 0.5;
                    self.pop_wait[0] += -r.max(1e-6).ln() / rate.max(1e-6);
                    self.pop_amp[0] = 1.0;
                    self.pop_sign[0] = if self.white() < 0.0 { -1.0 } else { 1.0 };
                }
                let snap = 0.025 + (0.009 - 0.025) * cu;
                v += self.pop_sign[0] * self.pop_amp[0] * 0.880;
                self.pop_amp[0] *= (-dt / snap.max(1e-5)).exp();
            }

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
        // Pop state IS cleared — unlike phase. A half-decayed pop
        // carried into the next explosion is an audible click at
        // sample zero, before the attack has opened.
        self.pop_wait = [0.0; 1];
        self.pop_amp = [0.0; 1];
    }
}

// ⚠️ LEVEL BELOW IS 0.3, THE EXPORT'S. The shipped constant is 0.45 —
// see MUTANT_BOOM_LEVEL. Re-importing this line gives Brian's voice as
// he built it; the level is a mix decision made after, not part of it.
// PLAYGROUND: {"len":0.575,"attack":0.1,"decay":2,"level":0.3,"filter":{"mode":"off","from":1840,"to":260,"q":5.5},"oscs":[{"on":true,"wave":"crackle","from":960,"to":480,"curve":2.43,"duty":0.5,"dutyTo":0.5,"mix":0.88,"delay":0,"snap":0.0255,"snapTo":0.009}]}

// ---------------------------------------------------------------------


// ---------------------------------------------------------------------
// The warp-in
// ---------------------------------------------------------------------

// ★★ BRIAN'S, BUILT BY EAR in tools/sound-playground.html and exported
// verbatim. The constants below are his; the `PLAYGROUND:` line at the
// end of this block round-trips back into the tool.
// ⚠️ DO NOT "improve" these numbers by reasoning about them. They are a
// listening result. The way to change this sound is to open the tool.

/// How long the whole sound lasts, in seconds.
///
/// ★ THIS ALL BUT MATCHES `enemy::WARP_SECONDS` (0.55) AND THAT IS THE
/// POINT. The Lander spends exactly that long materialising and
/// unhittable; a voice that outlasted it would still be arriving after
/// the thing had arrived, and one that finished early would leave the
/// last of the animation silent. They land together.
const WARP_LEN: f32 = 0.575;

/// ⚠️ A LONG ATTACK — 38% of the whole sound, where every other voice in
/// this file opens in 2-10%. That is what makes this a warp-in rather
/// than an event: it BUILDS. An explosion hits and decays; something
/// arriving swells until it is there.
const WARP_ATTACK: f32 = 0.380;
const WARP_DECAY: f32 = 1.000;
const WARP_LEVEL: f32 = 0.510;

/// A Lander materialising: noise and crackle through a bandpass
/// sweeping UP.
///
/// ★★ IT SWEEPS THE WRONG WAY ON PURPOSE. Every other filtered voice
/// here falls — the laser 1940 → 260, the Lander boom 1060 → 260. This
/// one climbs, 240 → 500 Hz. A falling sweep is something collapsing;
/// a rising one is something assembling, and the ear reads the
/// direction before it reads anything else.
///
/// ★ AND IT IS NOISE *PLUS* CRACKLE, the only voice in the file that
/// layers the two. The Mutant explosion is pure crackle with no body;
/// the laser is pure swept noise. This one has both: a bed that builds
/// and pops that thicken over it as the shape resolves.
pub struct Warp {
    t: f32,
    gain: f32,
    alive: bool,
    low: f32,
    band: f32,
    noise: u32,
    /// Per-crackle: seconds until the next pop, the amplitude of
    /// the pop ringing down now, and its sign.
    pop_wait: [f32; 1],
    pop_amp: [f32; 1],
    pop_sign: [f32; 1],
}

impl Default for Warp {
    fn default() -> Self {
        Self::new()
    }
}

impl Warp {
    pub fn new() -> Self {
        Self {
            t: 0.0,
            gain: 1.0,
            alive: false,
            low: 0.0,
            band: 0.0,
            noise: 0x1234_5678,
            pop_wait: [0.0; 1],
            pop_amp: [0.0; 1],
            pop_sign: [1.0; 1],
        }
    }

    /// xorshift32 — deterministic, allocation-free, audio-thread safe.
    fn white(&mut self) -> f32 {
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        (self.noise as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

impl Voice for Warp {
    fn render(&mut self, out: &mut [f32], _params: VoiceParams, sample_rate: f32) {
        if !self.alive {
            out.fill(0.0);
            return;
        }
        let dt = 1.0 / sample_rate;

        for sample in out.iter_mut() {
            if self.t >= WARP_LEN {
                *sample = 0.0;
                self.alive = false;
                continue;
            }
            let u = self.t / WARP_LEN;
            let mut v = 0.0f32;

            // noise, mix 0.920
            v += self.white() * 0.920;

            // crackle, mix 0.500
            // ★ Pops, not a waveform. Short impulses at random
            // times, thickening as the shape resolves — a continuous
            // oscillator cannot make this at any setting.
            if u >= 0.010 {
                let cu = (u - 0.010) / 0.990;
                let rate = 330.0 * 3.3939_f32.powf(cu.powf(0.580));
                self.pop_wait[0] -= dt;
                if self.pop_wait[0] <= 0.0 {
                    // Exponential gaps. Evenly spaced pops sound
                    // like a machine, not like something arriving.
                    let r = (self.white() + 1.0) * 0.5;
                    self.pop_wait[0] += -r.max(1e-6).ln() / rate.max(1e-6);
                    self.pop_amp[0] = 1.0;
                    self.pop_sign[0] = if self.white() < 0.0 { -1.0 } else { 1.0 };
                }
                let snap = 0.003 + (0.038 - 0.003) * cu;
                v += self.pop_sign[0] * self.pop_amp[0] * 0.500;
                self.pop_amp[0] *= (-dt / snap.max(1e-5)).exp();
            }

            // Normalised: the mixes sum above 1 and would clip.
            v /= 1.420;

            // A two-pole state-variable filter, corner sweeping UP.
            let c = 240.0 * 2.0833_f32.powf(u);
            // ⚠️ CLAMPED: a corner near Nyquist makes this blow up.
            let g = (2.0 * (PI * c.min(sample_rate * 0.45) / sample_rate).sin()).min(1.4);
            let damp = (1.0 / 7.000_f32).min(1.0);
            let high = v - self.low - damp * self.band;
            self.band += g * high;
            self.low += g * self.band;
            v = self.band;

            let env = if u < WARP_ATTACK {
                u / WARP_ATTACK
            } else {
                (-(u - WARP_ATTACK) * WARP_DECAY).exp()
            };

            *sample = v * env * WARP_LEVEL * self.gain;
            self.t += dt;
        }
    }

    fn alive(&self) -> bool {
        self.alive
    }

    fn retrigger(&mut self, gain: f32, _pitch: f32) {
        self.t = 0.0;
        // ⚠️ THE NOISE SEQUENCE IS NOT RESET — successive arrivals draw
        // from where the last one left off, so each gets a slightly
        // different texture for free.
        self.gain = gain.clamp(0.0, 1.0);
        self.alive = true;
        self.low = 0.0;
        self.band = 0.0;
        // Pop state IS cleared — unlike the noise. A half-decayed pop
        // carried into the next arrival is an audible click at sample
        // zero, before the attack has opened.
        self.pop_wait = [0.0; 1];
        self.pop_amp = [0.0; 1];
    }
}

// ── playground state, for round-tripping ──
// Paste this line back into tools/sound-playground.html to keep tweaking
// this sound by ear.
// PLAYGROUND: {"len":0.575,"attack":0.38,"decay":1,"level":0.51,"filter":{"mode":"bp","from":240,"to":500,"q":7},"oscs":[{"on":true,"wave":"noise","from":960,"to":480,"curve":2.43,"duty":0.5,"dutyTo":0.5,"mix":0.92,"delay":0,"snap":0.0255,"snapTo":0.009},{"on":true,"wave":"crackle","from":330,"to":1120,"curve":0.58,"duty":0.5,"dutyTo":0.5,"mix":0.5,"delay":0.01,"snap":0.0025,"snapTo":0.0385}]}

// ---------------------------------------------------------------------
// The thrust
// ---------------------------------------------------------------------

// ★★ THE FIRST CONTINUOUS VOICE IN DEFENDER, and it is a different KIND
// of thing from the five above. Those are one-shots: an event fires, a
// clip plays, `alive()` goes false, the mixer retires it. Thrust is HELD
// — it starts when the key goes down, lasts as long as the player holds
// it, and has no natural length at all.
//
// ⇒ SO IT HAS NO ENVELOPE. There is no attack, no decay, no LEN. The
// loudness comes from `exhaust`, read live out of `VoiceParams` every
// sample. THE PARAMETER IS THE ENVELOPE.
//
// ⚠️ IT ALSO NEVER IMPLEMENTS `alive()`. The trait defaults to `true`
// (core/src/audio/mixer.rs:35, "Continuous voices leave this at the
// default") and that default is exactly right here — a thrust voice that
// retired itself would have to be re-registered on every keypress.
//
// ★ THE PATTERN IS THE RACER'S ENGINE (games/omarcade-racer/src/sound.rs
// :243), which has shipped this shape all along: read a 0..=1 parameter
// from slot 0, render continuously, never die. Thrust is that with
// `exhaust` where the racer has `throttle`.

// ★★ BUILT BY EAR — BRIAN'S, 2026-09-19. See ref:defender-thrust-bed.
// ⚠️ NOT built in tools/sound-playground.html, and that is deliberate
// rather than an oversight. The playground designs CLIPS: every sweep in
// it runs against `u = i/n`, the position in a fixed buffer, and its
// envelope is attack-then-exponential-decay with no sustain stage. It
// cannot express a voice whose shape comes from outside itself. Brian
// auditioned this one from rendered WAVs instead (tools/thrust-probe.py)
// and picked the bed and the wobble depth off a ladder.
// ⇒ THERE IS NO `PLAYGROUND:` LINE ON THIS VOICE. Do not add one; it
// would round-trip into a tool that cannot represent what this is.

/// Filter corner at rest, in Hz — the engine idling.
///
/// ★ BRIGHTNESS TRACKS LOAD, which is why this is not simply
/// [`THRUST_CORNER_FULL`] at a lower level. A real engine opening up
/// gets BRIGHTER as well as louder; level alone reads as the same sound
/// through a volume knob. The corner rides up with `exhaust`.
const THRUST_CORNER_IDLE: f32 = 180.0;

/// Filter corner at full thrust, in Hz.
///
/// ★★ THIS IS THE NUMBER BRIAN APPROVED. He heard a five-file ladder and
/// picked 250 Hz over 400: "you kind of nailed on #2". Do not drift it
/// while tuning something else.
const THRUST_CORNER_FULL: f32 = 250.0;

/// Filter resonance. ★★★ LOW, AND THAT IS THE WHOLE TRICK.
///
/// ⚠️⚠️ EVERY OTHER VOICE IN THIS FILE USES Q 3.0–6.4 AND THIS ONE MUST
/// NOT. A two-pole state-variable filter at high Q is a near-oscillator:
/// it RINGS at its corner, and over noise that ring is heard as a PITCH
/// sitting behind the hiss. Brian reported exactly that while trying to
/// build this in the playground — "can hear the wave behind it" — and it
/// measured out as the cause: at the same 400 Hz corner, Q 4.0 collapsed
/// every harmonic above the fundamental to ~4% and pulled centroid/peak
/// to 1.90, which is the signature of a tone, not of noise. At Q 0.7 the
/// harmonics stay up (0.55, 0.35, 0.24) and centroid/peak sits at 3.93.
/// ⇒ HIGH Q GIVES A PITCH WEARING A NOISE COAT. LOW Q GIVES A WALL.
/// A held sound needs the wall. Raising this is not "more character",
/// it is the bug he spent a session hearing.
const THRUST_Q: f32 = 0.7;

/// How far the corner wanders, in OCTAVES.
///
/// ★ BRIAN'S RUNG. Offered ±0.15 / 0.30 / 0.50 / 0.80 against a flat
/// reference, he chose the quietest: "it's number 1". That matches what
/// he asked for — "a little wobble in there to make is sound like
/// rushing" — and matches the original machine, where thrust is a BED
/// under the game rather than a feature on top of it.
///
/// ⚠️ THE UNITS ARE OCTAVES AND THEY ONLY STAY OCTAVES BECAUSE OF THE
/// NORMALISATION IN `wobble()`. The first version of the probe wobbled
/// by a raw smoother output and moved the corner about a HUNDREDTH of an
/// octave — the scope measured the wobbled file as identical to the flat
/// one to three decimal places. A control with no authority is worse
/// than no control, because it reads as "tried it, did not help".
const THRUST_WOBBLE_OCTAVES: f32 = 0.15;

/// How fast the corner wanders, in Hz.
///
/// ★ THE RUSH RATE. Slower reads as swelling, faster as fluttering.
/// This is independent of [`THRUST_WOBBLE_OCTAVES`] — depth is how far,
/// this is how often — so it can be tuned without re-auditioning depth.
const THRUST_WOBBLE_HZ: f32 = 8.0;

/// How much the wobble moves the LEVEL, as a fraction of how much it
/// moves the corner.
///
/// ★ A corner that wanders while the level sits perfectly still still
/// reads as static — the ear takes the steady loudness as the truth and
/// hears the timbre change as an artefact. Breathing them together is
/// what makes it one moving thing. Kept well under 1.0 so the level
/// follows rather than leads.
const THRUST_WOBBLE_LEVEL: f32 = 0.25;

/// Output gain, chosen to land the rendered peak near 0.45.
///
/// ⚠️⚠️ THIS IS A GAIN, NOT A PEAK, AND THE TWO ARE NOWHERE NEAR EACH
/// OTHER HERE. A lowpass at Q 0.7 REMOVES energy: the raw filter output
/// peaks around 0.276, so a gain of 0.45 renders at 0.124 — less than a
/// third of what the name suggests. It takes ~1.63 to reach 0.45.
/// ★ THIS IS THE OLD SHIP-DEATH LESSON RUNNING BACKWARDS. There, a RESONANT
/// filter at Q 6.4 ADDED gain the level constant said nothing about and
/// the voice clipped at 1.03. Here a gentle filter subtracts it. Either
/// direction, the rule is the same: A `_LEVEL` CONSTANT IS NOT THE PEAK.
/// Render and measure. `thrust_does_not_clip` guards the top end.
///
/// ⚠️ THE TARGET 0.45 IS NOT A LISTENING RESULT — it is my choice,
/// pending Brian flying it. Set below the laser's measured 0.76 on
/// purpose: a HELD sound fatigues far faster than a one-shot, and
/// thrust is held for most of a game. ⇒ Revisit it BY FLYING IT.
const THRUST_LEVEL: f32 = 1.632;

/// Below this much exhaust the voice writes silence.
///
/// ★ WHY A FLOOR AT ALL: `exhaust` eases to zero asymptotically
/// (main.rs EXHAUST_RELEASE), so without a floor the engine would
/// whisper forever at an amplitude too small to hear but large enough to
/// keep the filter working. The floor is low enough to be inaudible and
/// high enough to actually reach.
const THRUST_GATE: f32 = 0.002;

/// The ship's engine: a wall of low noise that rides `exhaust`.
///
/// ★ WHAT THIS IS, PHYSICALLY: white noise through a lowpass held at a
/// FIXED corner — fixed, not swept. A sweep is a one-shot's signature,
/// the sound of an event with a beginning and an end. An engine holds
/// its timbre and changes only with load. What moves here moves for a
/// reason: the corner rides `exhaust` (load), and wanders slightly on
/// its own (the rush).
///
/// ⚠️ THERE IS NO SLEW ON `exhaust` AND THAT IS DELIBERATE. The racer's
/// engine slews its throttle internally (THROTTLE_SLEW) because raw
/// speed is jumpy. `exhaust` is ALREADY eased, in main.rs:303, with an
/// asymmetric attack of 14.0/s against a release of 6.0/s — fire catches
/// instantly and dies away. Slewing it twice would soften the attack the
/// visual was tuned for and put picture and sound out of step.
/// ⇒ SOUND AND PLUME MOVE ON THE SAME NUMBER. That is the point.
pub struct Thrust {
    noise: u32,
    /// Smoothed noise driving the wobble. Its own stream, so the
    /// breathing is not correlated with the grain of the bed itself.
    wobble_noise: u32,
    wobble: f32,
    /// Two-pole state-variable filter state.
    low: f32,
    band: f32,
}

impl Thrust {
    pub fn new() -> Thrust {
        Thrust {
            noise: 0x1234_5678,
            wobble_noise: 0xB7E1_5163,
            wobble: 0.0,
            low: 0.0,
            band: 0.0,
        }
    }

    fn white(&mut self) -> f32 {
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        (self.noise as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    fn white_wobble(&mut self) -> f32 {
        self.wobble_noise ^= self.wobble_noise << 13;
        self.wobble_noise ^= self.wobble_noise >> 17;
        self.wobble_noise ^= self.wobble_noise << 5;
        (self.wobble_noise as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    /// Advance the wobble one sample and return it, normalised to roughly
    /// unit variance.
    ///
    /// ★★ THE NORMALISATION IS THE WHOLE FUNCTION. A one-pole smoother
    /// fed unit-variance noise outputs a standard deviation of only
    /// `sqrt(k / (2 - k))` — at 8 Hz against 48 kHz that is 0.023. Using
    /// it raw means a "depth" of 0.15 moves the corner by about three
    /// THOUSANDTHS of an octave: inaudible, and indistinguishable from
    /// the control being broken.
    /// ⇒ Dividing by that standard deviation makes the output ~unit
    /// variance, so depth reads directly as octaves AND STAYS octaves
    /// if [`THRUST_WOBBLE_HZ`] is ever retuned. Deriving it from
    /// `sample_rate` rather than hardcoding keeps that true at any rate.
    fn wobble(&mut self, sample_rate: f32) -> f32 {
        let k = 1.0 - (-2.0 * PI * THRUST_WOBBLE_HZ / sample_rate).exp();
        let n = self.white_wobble();
        self.wobble += k * (n - self.wobble);
        // ⚠️ `k` is tiny, so `k / (2 - k)` is tiny and its square root is
        // still safely above zero — but guard anyway, because this runs
        // on the audio thread and a division by zero here would spray
        // NaN through every voice in the mixer.
        // ⚠️ THE `/ 3.0` IS NOT A FUDGE FACTOR. `sqrt(k / (2 - k))` is
        // the output std of a one-pole smoother fed UNIT-VARIANCE noise,
        // and `white_wobble` is not that: it is uniform on (-1, 1),
        // whose variance is 1/3. The smoother scales the input std, so
        // the real output std is `sqrt(k / (2 - k)) * sqrt(1/3)` — which
        // is `sqrt(k / (2 - k) / 3)`, the divisor written here.
        // ⇒ Without it the result lands at std 0.577 and every depth
        // silently means 58% of what it says.
        let std = (k / (2.0 - k) / 3.0).sqrt().max(1e-6);
        self.wobble / std
    }
}

impl Default for Thrust {
    fn default() -> Thrust {
        Thrust::new()
    }
}

impl Voice for Thrust {
    fn render(&mut self, out: &mut [f32], params: VoiceParams, sample_rate: f32) {
        // Slot 0 is `exhaust`, straight off the ship. See
        // `VoiceParams::thrust`.
        let exhaust = params.get(0).clamp(0.0, 1.0);

        if exhaust < THRUST_GATE {
            out.fill(0.0);
            // ⚠️ THE FILTER IS NOT CLEARED HERE. The gate is crossed
            // every time the player lifts off, and clearing state on
            // each crossing would put a click at the start of every
            // re-ignition. The noise keeps running for the same reason
            // the laser's does: continuity is free character.
            return;
        }

        for sample in out.iter_mut() {
            let w = self.wobble(sample_rate);

            // Brightness rides load, then wanders. Both are
            // MULTIPLICATIVE on the corner: timbre is heard
            // logarithmically, so a fixed number of Hz is a far bigger
            // move down low than it is up high.
            let c = (THRUST_CORNER_IDLE
                + (THRUST_CORNER_FULL - THRUST_CORNER_IDLE) * exhaust)
                * (w * THRUST_WOBBLE_OCTAVES).exp2();

            let v = self.white();

            // ⚠️ CLAMPED, for the same reason as every other filter in
            // this file: a corner near Nyquist walks the SVF past its
            // stability limit and it self-oscillates to NaN, which the
            // mixer then spreads across every voice. The corner here is
            // nowhere near it, but the clamp costs nothing and the
            // failure is catastrophic.
            let g = (2.0 * (PI * c.min(sample_rate * 0.45) / sample_rate).sin()).min(1.4);
            let damp = (1.0 / THRUST_Q).min(1.0);
            let high = v - self.low - damp * self.band;
            self.band += g * high;
            self.low += g * self.band;

            // The level breathes with the corner, and rides exhaust.
            let breath = 1.0 + w * THRUST_WOBBLE_OCTAVES * THRUST_WOBBLE_LEVEL;
            *sample = self.low * breath * exhaust * THRUST_LEVEL;
        }
    }

    // ★ NO `alive()`. The trait default is `true` and a continuous voice
    // wants exactly that — see the block comment at the top of this
    // section.

    // ★ NO `retrigger()` EITHER. Nothing "fires" a held sound; it is
    // driven by `set`, not by `play`. The trait default is a no-op,
    // which is the correct behaviour if one is ever called by mistake.
}

// ---------------------------------------------------------------------
// The smart bomb
// ---------------------------------------------------------------------

// ★★ THE FIRST VOICE BUILT FROM THE WILLIAMS MECHANISM RATHER THAN BY EAR
// (docs/warden-plan.md W5, pulled forward because Brian flew the bomb and
// heard nothing: "the bomb when pressed has no sound effect").
//
// The original (docs/warden/research-sound.md §2): LITE retriggered 6×
// every 64 ms, then CANNON. Read from Sam Dicker's own source
// (vsndrm1.src, `LITEN` and `FNOISE`) and rebuilt as the board ran it:
// a state machine writing 8-bit DAC values on a virtual 894,886 Hz cycle
// clock, held between writes, integrated down to the output rate. The
// stair-steps and the aliasing ARE the sound; a modern noise generator
// through a filter would be a different instrument.
//
// ⚠️ OUR PARAMETERS, THE ORIGINAL'S TECHNIQUE. No ROM table is used.
// The numbers below are mechanism (a clock, a step, a count), measured
// against the emulated ROM renders in ~/projects/omarcade-reference.

/// The sound board's CPU clock: 3.579545 MHz ÷ 4.
const WILLIAMS_CLOCK: f32 = 894_886.0;

/// LITE bursts in the smart bomb's stutter, and how far apart (64 ms):
/// SBSND, repeat 6, timer 4 frames.
const BOMB_STUTTERS: u8 = 6;
const BOMB_STUTTER_CYCLES: f32 = 0.064 * WILLIAMS_CLOCK;

/// ★ W5: THE SHIP'S DEATH IS THE SAME CHAIN, SLOWER (PDSND: LITE repeat
/// 2, timer 8 frames, then CANNON): two crackles 128 ms apart, then the
/// explosion's whole 2.6 s tail. ~2.8 s. ROM: 11_LITE.wav + 17_CANNON.wav.
const DEATH_STUTTERS: u8 = 2;
const DEATH_STUTTER_CYCLES: f32 = 0.128 * WILLIAMS_CLOCK;

/// LITE and APPEAR share one routine (`LITEN`): cycles per sample are
/// `LITE_BASE + LITE_PER_STEP × L`, and L moves by a step every few
/// samples. LITE climbs L from 1 (clock FALLING from ~18.6 kHz: something
/// breaking up); APPEAR walks it down to 0 (clock RISING: something
/// arriving).
const LITE_BASE: f32 = 42.0;
const LITE_PER_STEP: f32 = 6.0;
const LITE_SAMPLES_PER_STEP: u8 = 3;

/// CANNON: cycles per sample (measured: 72,000 samples in the 2.58 s the
/// emulator times it at → 35.8 µs), samples between slope decays, and the
/// slope it starts from (8.8 fixed point).
const CANNON_CYCLES: f32 = 32.0;
const CANNON_DECAY_SAMPLES: u16 = 1000;
const CANNON_START_SLOPE: u16 = 0xFF00;

/// The AC coupling of the real amplifier: a one-pole high-pass at ~20 Hz.
const DC_BLOCK_HZ: f32 = 20.0;

/// Output level. ⚠️ NOT THE PEAK — the crackle is full-scale 1-bit noise
/// and the DC block overshoots its edges. Rendered and measured: see
/// `no_voice_clips_at_full_gain`.
const BOMB_LEVEL: f32 = 0.42;
const SHIP_DEATH_LEVEL: f32 = 0.42;

/// ★ W5: A PERSON YOU SHOT (AHSND → LITE): one crackle, its clock falling
/// ~18.6 kHz → 573 Hz over 0.70 s. Replaced a placeholder (W5).
/// ⚠️ Brian's spec for this event is that it should NOT be satisfying —
/// it is the sound of a mistake — so it must stay QUIETER THAN A LANDER
/// DYING (`the_four_deaths_are_distinguishable`). At 0.25 it measured
/// RMS 0.24 against HBEV's 0.17 and broke that; 0.16 puts it under.
const PERSON_CRACKLE_LEVEL: f32 = 0.16;

// ---------------------------------------------------------------------
// The board: the parts every Williams-mechanism voice shares
// ---------------------------------------------------------------------

/// One Williams sound board's worth of state: the random shift register,
/// the DAC and how long it holds, and the amplifier's AC coupling.
///
/// ★ A VOICE IS A SEQUENCE OF ROUTINES ON THIS. The smart bomb is LITE
/// then CANNON; hyperspace is LITE then APPEAR. Each routine is a small
/// generator that writes the DAC; the board turns writes into samples.
pub(crate) struct Board {
    hi: u8,
    lo: u8,
    dac: u8,
    /// Cycles the DAC still holds its value.
    hold: f32,
    dc_x: f32,
    dc_y: f32,
}

impl Board {
    const fn new() -> Self {
        // The original's seed: HI = $3C, LO = 0.
        Self { hi: 0x3C, lo: 0, dac: 0x80, hold: 0.0, dc_x: 0.0, dc_y: 0.0 }
    }

    /// Restart the output path. The shift register keeps running between
    /// sounds, as the board's did: no two are the same noise.
    fn restart(&mut self) {
        self.hold = 0.0;
        self.dc_x = 0.0;
        self.dc_y = 0.0;
    }

    /// One step of the shift register, with `feed` mixed into the new top
    /// bit. Returns the bit shifted out.
    fn shift(&mut self, feed: u8) -> bool {
        let new_top = ((feed >> 3) ^ self.lo) & 1;
        let out_hi = self.hi & 1;
        self.hi = (self.hi >> 1) | (new_top << 7);
        let out = self.lo & 1 == 1;
        self.lo = (self.lo >> 1) | (out_hi << 7);
        out
    }

    /// One output sample. `write` is called whenever the DAC's hold runs
    /// out, and must set `dac` and `hold`.
    ///
    /// ★ ZERO-ORDER HOLD, INTEGRATED: the DAC is averaged over exactly the
    /// cycles this output sample spans, so every write lands with its true
    /// weight and nothing is resampled away. Then the AC coupling.
    fn sample(&mut self, per_sample: f32, r: f32, mut write: impl FnMut(&mut Board)) -> f32 {
        let mut need = per_sample;
        let mut acc = 0.0f32;
        while need > 0.0 {
            if self.hold <= 0.0 {
                write(self);
            }
            let take = need.min(self.hold);
            acc += self.dac as f32 * take;
            need -= take;
            self.hold -= take;
        }
        let x = (acc / per_sample - 128.0) / 128.0;
        let y = x - self.dc_x + r * self.dc_y;
        self.dc_x = x;
        self.dc_y = y;
        y
    }
}

/// The DC-block coefficient at `sample_rate`.
fn dc_block(sample_rate: f32) -> f32 {
    1.0 - (2.0 * PI * DC_BLOCK_HZ / sample_rate)
}

/// LITE / APPEAR: 1-bit full-scale noise, the DAC inverting on each
/// random 1, with its clock walked by `step` every `per_step` samples.
#[derive(Debug, Clone, Copy)]
struct Liten {
    l: u8,
    step: i8,
    per_step: u8,
    count: u8,
    /// Cycles since this one started, for a caller that cuts it short.
    cycles: f32,
}

impl Liten {
    /// The original's LITE: L from 1, +1 every 3 samples — falling.
    fn lite() -> Self {
        Self { l: 1, step: 1, per_step: LITE_SAMPLES_PER_STEP, count: LITE_SAMPLES_PER_STEP, cycles: 0.0 }
    }

    /// APPEAR's shape — L walked DOWN, so the clock rises — from `l` in
    /// steps of `step`, `per_step` samples each.
    fn appear(l: u8, step: i8, per_step: u8) -> Self {
        Self { l, step, per_step, count: per_step, cycles: 0.0 }
    }

    /// Starting a LITE/APPEAR puts the DAC at full scale (`LITEN`).
    fn begin(b: &mut Board) {
        b.dac = 0xFF;
    }

    /// One DAC write. Returns false once L has run off its end (the
    /// original's `BNE LITE0` falling through).
    fn write(&mut self, b: &mut Board) -> bool {
        let lo = b.lo;
        if b.shift(lo) {
            b.dac = !b.dac;
        }
        b.hold = LITE_BASE + LITE_PER_STEP * self.l as f32;
        self.cycles += b.hold;
        self.count -= 1;
        if self.count == 0 {
            self.count = self.per_step;
            let next = self.l as i16 + self.step as i16;
            if !(1..=255).contains(&next) {
                return false;
            }
            self.l = next as u8;
        }
        true
    }
}

/// CANNON (`FNOISE` with distortion): slew toward a random target at a
/// random slope no steeper than a ceiling that decays ×7/8 every
/// [`CANNON_DECAY_SAMPLES`].
#[derive(Debug, Clone, Copy)]
struct Cannon {
    fmax: u16,
    frac: u8,
    target: u8,
    decay_in: u16,
}

impl Cannon {
    fn new(b: &Board) -> Self {
        Self { fmax: CANNON_START_SLOPE, frac: 0, target: b.dac, decay_in: CANNON_DECAY_SAMPLES }
    }

    /// One DAC write. Returns false once the ceiling has settled.
    fn write(&mut self, b: &mut Board) -> bool {
        if b.dac == self.target {
            let dac = b.dac;
            b.shift(dac);
            self.target = b.lo;
        }
        let slope_hi = ((self.fmax >> 8) as u8) & b.hi;
        let step = ((slope_hi as u16) << 8) | (self.fmax & 0xFF);
        let pos = ((b.dac as u16) << 8) | self.frac as u16;
        let t = (self.target as u16) << 8;
        let next = if pos < t { pos.saturating_add(step).min(t) } else { pos.saturating_sub(step).max(t) };
        b.dac = (next >> 8) as u8;
        self.frac = next as u8;
        b.hold = CANNON_CYCLES;

        self.decay_in -= 1;
        if self.decay_in == 0 {
            self.decay_in = CANNON_DECAY_SAMPLES;
            // ×7/8, truncating — it settles at 7/256 and stops, exactly
            // where the original's routine returns.
            let next = self.fmax - (self.fmax >> 3);
            let settled = next == self.fmax || self.fmax <= 7;
            self.fmax = next;
            if settled {
                return false;
            }
        }
        true
    }
}

/// TURBO (`NOISE` with frequency decay): each sample is the shift
/// register's bit times the amplitude; every 32 samples the amplitude
/// drops by one and the delay loop grows by one count (8 cycles). So the
/// noise CLOCK falls ~15.8 kHz → ~430 Hz while the level fades slowly,
/// over 9.79 s if nothing cuts it. The original's laser — and the first
/// beat of the planet going.
#[derive(Debug, Clone, Copy)]
struct Turbo {
    amp: u8,
    delay: u16,
    count: u8,
}

/// TURBO's per-sample cost: the bit-picking (~48.5 cycles, the BCC
/// splitting it by a load) plus 8 a delay count; and the bookkeeping
/// between each run of 32.
const TURBO_BASE: f32 = 48.5;
const TURBO_PER_DELAY: f32 = 8.0;
const TURBO_SAMPLES: u8 = 32;
const TURBO_CYCLE_END: f32 = 30.0;

impl Turbo {
    fn new() -> Self {
        Self { amp: 0xFF, delay: 1, count: TURBO_SAMPLES }
    }

    /// One DAC write. False once the amplitude has decayed away.
    fn write(&mut self, b: &mut Board) -> bool {
        let lo = b.lo;
        b.dac = if b.shift(lo) { self.amp } else { 0 };
        b.hold = TURBO_BASE + TURBO_PER_DELAY * self.delay as f32;
        self.count -= 1;
        if self.count == 0 {
            self.count = TURBO_SAMPLES;
            self.amp = self.amp.saturating_sub(1);
            if self.amp == 0 {
                return false;
            }
            self.delay += 1;
            b.hold += TURBO_CYCLE_END;
        }
        true
    }
}

/// Park the board silent once a voice has finished.
fn park(b: &mut Board) {
    b.dac = 0x80;
    b.hold = f32::MAX;
}

#[derive(Debug, Clone, Copy)]
enum BombStage {
    /// The crackle, on burst `n` of the stutter.
    Lite { burst: u8, routine: Liten },
    /// The explosion tail.
    Cannon(Cannon),
    Done,
}

/// A stuttering crackle, then the big explosion (LITE × n, CANNON) —
/// the original's smart bomb and, slower, its ship's death.
pub struct SmartBomb {
    gain: f32,
    stage: BombStage,
    board: Board,
    stutters: u8,
    stutter_cycles: f32,
    level: f32,
}

impl Default for SmartBomb {
    fn default() -> Self {
        Self::new()
    }
}

impl SmartBomb {
    pub fn new() -> Self {
        Self::chain(BOMB_STUTTERS, BOMB_STUTTER_CYCLES, BOMB_LEVEL)
    }

    /// ★ W5: the ship's death (PDSND) — replaced a placeholder.
    pub fn ship_death() -> Self {
        Self::chain(DEATH_STUTTERS, DEATH_STUTTER_CYCLES, SHIP_DEATH_LEVEL)
    }

    fn chain(stutters: u8, stutter_cycles: f32, level: f32) -> Self {
        Self { gain: 1.0, stage: BombStage::Done, board: Board::new(), stutters, stutter_cycles, level }
    }
}

impl Voice for SmartBomb {
    fn render(&mut self, out: &mut [f32], _params: VoiceParams, sample_rate: f32) {
        let per_sample = WILLIAMS_CLOCK / sample_rate;
        let r = dc_block(sample_rate);
        for sample in out.iter_mut() {
            if matches!(self.stage, BombStage::Done) {
                *sample = 0.0;
                continue;
            }
            let stage = &mut self.stage;
            let (stutters, stutter_cycles) = (self.stutters, self.stutter_cycles);
            let y = self.board.sample(per_sample, r, |b| loop {
                match stage {
                    BombStage::Lite { burst, routine } => {
                        // ★ THE STUTTER: every 64 ms the crackle restarts
                        // from its fastest clock, six times, before the
                        // explosion takes over.
                        if routine.cycles >= stutter_cycles {
                            if *burst + 1 < stutters {
                                *stage = BombStage::Lite { burst: *burst + 1, routine: Liten::lite() };
                                Liten::begin(b);
                            } else {
                                *stage = BombStage::Cannon(Cannon::new(b));
                            }
                            continue;
                        }
                        routine.write(b);
                        return;
                    }
                    BombStage::Cannon(c) => {
                        if !c.write(b) {
                            *stage = BombStage::Done;
                        }
                        return;
                    }
                    BombStage::Done => return park(b),
                }
            });
            *sample = y * self.level * self.gain;
        }
    }

    fn alive(&self) -> bool {
        !matches!(self.stage, BombStage::Done)
    }

    fn retrigger(&mut self, gain: f32, _pitch: f32) {
        self.gain = gain.clamp(0.0, 1.0);
        self.board.restart();
        self.stage = BombStage::Lite { burst: 0, routine: Liten::lite() };
        Liten::begin(&mut self.board);
    }
}

// ---------------------------------------------------------------------
// Hyperspace
// ---------------------------------------------------------------------

// ★ OURS, NOT THE ORIGINAL'S — AND THE ORIGINAL HAD NONE. Defender's
// HYPER routine (defa7.src) calls no sound at all; you heard whatever was
// already playing. Brian, having flown the jump: "Hyper Space has no sound
// FX either. Can we add that". So this is built in the board's own
// vocabulary rather than recalled: OUT, then IN.
// · Out: one LITE burst (64 ms, the smart bomb's stutter unit), its
//   clock falling — the ship coming apart.
// · In: APPEAR, the board's own "something is materialising" routine,
//   its clock RISING from ~760 Hz to the top, time-compressed so it
//   lands as the ship finishes coming back together (main.rs
//   HYPERSPACE_SECONDS, 40 frames).
// ⚠️ NOT THE WARP. Brian's Warp also rises, but it is swept filtered
// noise; this is 1-bit crackle. Same direction, different instrument —
// a Lander arriving and you arriving must not be the same word.

/// The departure: one LITE burst this long.
const HYPER_OUT_CYCLES: f32 = 0.064 * WILLIAMS_CLOCK;

/// The arrival: APPEAR's L from 192 down in steps of 2 — the original's
/// own start and step — with samples per step compressed from its 16 to
/// fit the materialise (measured: 9 → ~0.6 s, against 1.07 s at 16).
const HYPER_IN_FROM: u8 = 0xC0;
const HYPER_IN_STEP: i8 = -2;
const HYPER_IN_PER_STEP: u8 = 9;

/// ⚠️ NOT THE PEAK; rendered and measured in the clip test. Under the
/// bomb's: a jump is a manoeuvre, not a detonation.
const HYPER_LEVEL: f32 = 0.30;

#[derive(Debug, Clone, Copy)]
enum HyperStage {
    Out(Liten),
    In(Liten),
    Done,
}

/// Hyperspace: the ship breaking up, then coming back together.
pub struct Hyperspace {
    gain: f32,
    stage: HyperStage,
    board: Board,
}

impl Default for Hyperspace {
    fn default() -> Self {
        Self::new()
    }
}

impl Hyperspace {
    pub fn new() -> Self {
        Self { gain: 1.0, stage: HyperStage::Done, board: Board::new() }
    }
}

impl Voice for Hyperspace {
    fn render(&mut self, out: &mut [f32], _params: VoiceParams, sample_rate: f32) {
        let per_sample = WILLIAMS_CLOCK / sample_rate;
        let r = dc_block(sample_rate);
        for sample in out.iter_mut() {
            if matches!(self.stage, HyperStage::Done) {
                *sample = 0.0;
                continue;
            }
            let stage = &mut self.stage;
            let y = self.board.sample(per_sample, r, |b| loop {
                match stage {
                    HyperStage::Out(routine) => {
                        if routine.cycles >= HYPER_OUT_CYCLES {
                            *stage = HyperStage::In(Liten::appear(
                                HYPER_IN_FROM,
                                HYPER_IN_STEP,
                                HYPER_IN_PER_STEP,
                            ));
                            Liten::begin(b);
                            continue;
                        }
                        routine.write(b);
                        return;
                    }
                    HyperStage::In(routine) => {
                        if !routine.write(b) {
                            *stage = HyperStage::Done;
                        }
                        return;
                    }
                    HyperStage::Done => return park(b),
                }
            });
            *sample = y * HYPER_LEVEL * self.gain;
        }
    }

    fn alive(&self) -> bool {
        !matches!(self.stage, HyperStage::Done)
    }

    fn retrigger(&mut self, gain: f32, _pitch: f32) {
        self.gain = gain.clamp(0.0, 1.0);
        self.board.restart();
        self.stage = HyperStage::Out(Liten::lite());
        Liten::begin(&mut self.board);
    }
}

// ---------------------------------------------------------------------
// THE PEOPLE'S SOUNDS, AND THE ENEMY'S GUNS — GWAVE, VARI AND SCREAM
// ---------------------------------------------------------------------

// ★ BRIAN, having flown it with these silent: "when a humanoid is picked
// up we get a higher pitched [sound] … shoots lander with humanoid,
// sound fx like sliding down, and picks them up sound fx woo woo woop and
// lands them we get that classic phaser sound when dropped to ground.
// When mutant spawns in they get sound and there is a different sound
// when they shoot."
//
// Each is the original's own routine (vsndrm1.src), mapped through the
// game's sound table (research-sound.md §2) and checked against the
// emulated ROM render named beside it:
//
// | Event                      | Routine (preset)   | ROM render                      |
// |----------------------------|--------------------|---------------------------------|
// | Lander grabs a person      | GWAVE (ED10)       | 0b_G11_ED10_start2_pickup.wav   |
// | A dropped person falls     | SCREAM             | 1a_SCREAM.wav                   |
// | The ship catches them      | GWAVE (SPNRV) ×3   | 08_G8_SPNRV_catch.wav           |
// | Set down on the ground     | VARI (QUASAR)      | 1f_QUASAR_astroland.wav         |
// | A Lander becomes a Mutant  | VARI (SP1)         | 0e_SP1_landersuck.wav           |
// | A Lander or Baiter fires   | GWAVE (DP1V)       | 03_G3_DP1V_shoot.wav            |
// | A Mutant fires             | GWAVE (CLDWN)      | 09_G9_CLDWN_mutantshoot.wav     |
//
// ⚠️ OUR TABLES, THE ORIGINAL'S MECHANISM. Wave shapes are generated from
// formulas, not copied from the ROM; each period pattern is the one the
// MEASURED pitches imply through the board's own timing (a GWAVE sample
// costs 26 + 6P cycles, plus 49 at the end of each pass of the wave —
// research-sound.md §1).

/// A GWAVE sample costs this many cycles plus 6 per period count…
const GWAVE_SAMPLE_BASE: f32 = 26.0;
const GWAVE_PER_COUNT: f32 = 6.0;
/// …and each completed pass of the wave costs this much more.
const GWAVE_WAVE_END: f32 = 49.0;

/// ★ THE CPU-GAP SILENCES. The board has one CPU, so while it rewrites the
/// wave in RAM no sample goes out and the DAC just holds. WVDECA costs
/// 65 cycles a sample plus 12 per sixteenth of decay (its SBA / DEC /
/// BNE loop); WVTRAN (re-copying the ROM wave) 43 a sample; the GW0 scan
/// that trims the pattern ~25 an entry. Counted from vsndrm1.src and
/// checked against the emulator: an echo of PROTV (decay 3) goes silent
/// for 7.4k cycles, a frequency step (decay 3 + re-copy + predecay 17)
/// for 30.6k — this model gives 7.4k and 30.1k. Twelve of those are
/// half of PROTV's 1.09 s. research-sound.md: "leave the CPU-gap
/// silences in".
const GWAVE_DECAY_PER_SAMPLE: f32 = 65.0;
const GWAVE_DECAY_PER_FACTOR: f32 = 12.0;
const GWAVE_COPY_PER_SAMPLE: f32 = 43.0;
const GWAVE_SCAN_PER_ENTRY: f32 = 25.0;

/// One count of a VARI half-cycle (DEX / BEQ / DECA / BNE).
const VARI_COUNT_CYCLES: f32 = 14.0;
/// The sweep's own bookkeeping between half-cycles.
const VARI_SWEEP_CYCLES: f32 = 40.0;

/// SCREAM's sample period: 218 µs (measured), 195 board cycles.
const SCREAM_CYCLES: f32 = 195.0;

/// A wavetable, at most 72 samples (the longest GWAVE uses).
#[derive(Clone, Copy)]
struct WaveTable {
    data: [u8; 72],
    len: usize,
}

impl WaveTable {
    fn from_fn(len: usize, f: impl Fn(f32) -> f32) -> Self {
        let mut data = [0u8; 72];
        for (i, d) in data.iter_mut().enumerate().take(len) {
            let theta = std::f32::consts::TAU * i as f32 / len as f32;
            *d = f(theta).round().clamp(0.0, 255.0) as u8;
        }
        Self { data, len }
    }

    /// A plain sine, full scale.
    fn sine(len: usize) -> Self {
        Self::from_fn(len, |t| 127.5 + 127.5 * t.sin())
    }

    /// A sine with its second harmonic — the bright, reedy one.
    fn two_harmonic(len: usize) -> Self {
        Self::from_fn(len, |t| 127.5 + 82.0 * t.sin() + 46.0 * (2.0 * t).sin())
    }

    /// A full-scale square, two cycles to the table — so each pass of the
    /// wave sounds an octave above the table rate (the Bomber's hit).
    fn square_twice(len: usize) -> Self {
        Self::from_fn(len, |t| if (2.0 * t).sin() >= 0.0 { 255.0 } else { 0.0 })
    }

    /// The odd eight-step wave the Mutant's gun is built on: a lopsided
    /// near-square, harsh by design (research-sound.md §3, recipe 11).
    fn odd8() -> Self {
        let mut data = [0u8; 72];
        data[..8].copy_from_slice(&[0, 64, 128, 0, 255, 0, 128, 64]);
        Self { data, len: 8 }
    }
}

/// A GWAVE preset: the wave, and how the board walks it.
#[derive(Clone, Copy)]
struct GwaveSpec {
    wave: WaveTable,
    /// Passes of the wave per pattern entry (GCCNT).
    cycles: u8,
    /// Times the whole pattern is played (GECHO)…
    echoes: u8,
    /// …and how much the wave decays after each (GECDEC, sixteenths).
    echo_decay: u8,
    /// Decay applied once, before anything plays.
    predecay: u8,
    /// Added to every period after the echoes (GDFINC), and how many
    /// times (GDCNT; 0 means 255, as on the board).
    freq_inc: i8,
    freq_count: u8,
    /// The period pattern (P in 26 + 6P).
    pattern: &'static [u8],
}

/// GWAVE, running.
#[derive(Clone, Copy)]
struct Gwave {
    spec: GwaveSpec,
    ram: [u8; 72],
    start: usize,
    end: usize,
    entry: usize,
    period: u8,
    passes_left: u8,
    sample: usize,
    echoes_left: u8,
    offset: u8,
    count_left: u8,
    /// Cycles the CPU spends rewriting the wave before the next sample.
    gap: f32,
}

impl Gwave {
    fn new(spec: GwaveSpec) -> Self {
        let mut g = Self {
            spec,
            ram: spec.wave.data,
            start: 0,
            end: spec.pattern.len(),
            entry: 0,
            period: spec.pattern[0],
            passes_left: spec.cycles,
            sample: 0,
            echoes_left: spec.echoes,
            offset: 0,
            count_left: spec.freq_count,
            gap: 0.0,
        };
        g.decay(spec.predecay);
        // Before the first sample: silent anyway, so it costs nothing.
        g.gap = 0.0;
        g
    }

    /// ★ THE ORIGINAL'S DECAY WRAPS, IT DOES NOT CLAMP: each sample loses
    /// a sixteenth of its ROM value `factor` times, modulo 256. Heavy
    /// decay turns a sine into spikes — that is the character.
    fn decay(&mut self, factor: u8) {
        if factor != 0 {
            self.gap += self.spec.wave.len as f32
                * (GWAVE_DECAY_PER_SAMPLE + GWAVE_DECAY_PER_FACTOR * factor as f32);
        }
        for i in 0..self.spec.wave.len {
            let step = (self.spec.wave.data[i] >> 4).wrapping_mul(factor);
            self.ram[i] = self.ram[i].wrapping_sub(step);
        }
    }

    fn load_entry(&mut self) {
        self.period = self.spec.pattern[self.entry].wrapping_add(self.offset);
        self.passes_left = self.spec.cycles;
    }

    /// The pattern is done: echo, then frequency-shift (GEND), or finish.
    fn end_of_pattern(&mut self) -> bool {
        self.decay(self.spec.echo_decay);
        // DEC GECNT: an echo count of 0 plays 256 times (SV3 does).
        self.echoes_left = self.echoes_left.wrapping_sub(1);
        if self.echoes_left > 0 {
            self.entry = self.start;
            self.load_entry();
            return true;
        }
        if self.spec.freq_inc == 0 {
            return false;
        }
        self.count_left = self.count_left.wrapping_sub(1);
        if self.count_left == 0 {
            return false;
        }
        self.offset = self.offset.wrapping_add(self.spec.freq_inc as u8);
        // Keep only the entries the shift has not pushed past the end of
        // the counter (GW0): a rising pattern loses its top, a falling
        // one its bottom, until nothing is left.
        self.gap += GWAVE_SCAN_PER_ENTRY * (self.end - self.start) as f32;
        let (mut found, mut new_start, mut new_end) = (false, self.start, self.end);
        for i in self.start..self.end {
            let (sum, carry) = self.offset.overflowing_add(self.spec.pattern[i]);
            let valid = if self.spec.freq_inc > 0 { !carry } else { carry && sum != 0 };
            if valid && !found {
                found = true;
                new_start = i;
            } else if !valid && found {
                new_end = i;
                break;
            }
        }
        if !found {
            return false;
        }
        self.start = new_start;
        self.end = new_end;
        if self.spec.echo_decay != 0 {
            self.ram = self.spec.wave.data;
            self.gap += GWAVE_COPY_PER_SAMPLE * self.spec.wave.len as f32;
            self.decay(self.spec.predecay);
        }
        self.echoes_left = self.spec.echoes;
        self.entry = self.start;
        self.load_entry();
        true
    }

    /// One DAC write. False once the routine has run out.
    fn write(&mut self, b: &mut Board) -> bool {
        b.dac = self.ram[self.sample];
        let p = if self.period == 0 { 256.0 } else { self.period as f32 };
        b.hold = GWAVE_SAMPLE_BASE + GWAVE_PER_COUNT * p;
        self.sample += 1;
        if self.sample < self.spec.wave.len {
            return true;
        }
        self.sample = 0;
        b.hold += GWAVE_WAVE_END;
        self.passes_left -= 1;
        if self.passes_left > 0 {
            return true;
        }
        self.entry += 1;
        if self.entry < self.end {
            self.load_entry();
            true
        } else {
            let more = self.end_of_pattern();
            // The DAC holds this last sample while the CPU works.
            b.hold += std::mem::take(&mut self.gap);
            more
        }
    }
}

/// A VARI preset: a square whose two halves are counted separately.
#[derive(Clone, Copy)]
struct VariSpec {
    lo: u8,
    hi: u8,
    lo_step: u8,
    hi_step: u8,
    hi_end: u8,
    /// Counts between sweeps (SWPDT).
    sweep: u16,
    /// Added to the low half after each full sweep (LOMOD); 0 ends it.
    lo_mod: u8,
    amp: u8,
}

/// VARI, running.
#[derive(Clone, Copy)]
struct Vari {
    spec: VariSpec,
    lo_base: u8,
    lo: u8,
    hi: u8,
    in_hi: bool,
    remaining: u32,
    to_sweep: u32,
    at_edge: bool,
    sweep_due: bool,
}

impl Vari {
    fn new(spec: VariSpec, b: &mut Board) -> Self {
        b.dac = spec.amp;
        Self {
            spec,
            lo_base: spec.lo,
            lo: spec.lo,
            hi: spec.hi,
            in_hi: false,
            remaining: 0,
            to_sweep: spec.sweep as u32,
            at_edge: true,
            sweep_due: false,
        }
    }

    fn counts(c: u8) -> u32 {
        if c == 0 { 256 } else { c as u32 }
    }

    fn write(&mut self, b: &mut Board) -> bool {
        if self.sweep_due {
            self.sweep_due = false;
            // VSWEEP: settle the DAC on its high half, then step both
            // halves; a finished sweep steps the low half's base (LOMOD).
            if b.dac < 0x80 {
                b.dac = !b.dac;
            }
            self.lo = self.lo.wrapping_add(self.spec.lo_step);
            self.hi = self.hi.wrapping_add(self.spec.hi_step);
            if self.hi == self.spec.hi_end {
                if self.spec.lo_mod == 0 {
                    return false;
                }
                self.lo_base = self.lo_base.wrapping_add(self.spec.lo_mod);
                if self.lo_base == 0 {
                    return false;
                }
                self.lo = self.lo_base;
                self.hi = self.spec.hi;
            }
            self.to_sweep = self.spec.sweep as u32;
            self.in_hi = false;
            self.at_edge = true;
            b.hold = VARI_SWEEP_CYCLES;
            return true;
        }
        if self.at_edge {
            b.dac = !b.dac;
            self.remaining = Self::counts(if self.in_hi { self.hi } else { self.lo });
            self.at_edge = false;
        }
        let n = self.remaining.min(self.to_sweep);
        b.hold = n as f32 * VARI_COUNT_CYCLES;
        self.remaining -= n;
        self.to_sweep -= n;
        if self.to_sweep == 0 {
            self.sweep_due = true;
        } else if self.remaining == 0 {
            self.in_hi = !self.in_hi;
            self.at_edge = true;
        }
        true
    }
}

/// SCREAM: four square voices from 8-bit phase accumulators, each half
/// the level of the last, their pitches falling together; as one passes
/// step $37 the next starts at $41, so the fall is staggered echoes.
#[derive(Clone, Copy)]
struct ScreamRoutine {
    timer: [u8; 4],
    freq: [u8; 4],
    tick: u8,
}

impl ScreamRoutine {
    fn new() -> Self {
        Self { timer: [0; 4], freq: [0x40, 0, 0, 0], tick: 0 }
    }

    fn write(&mut self, b: &mut Board) -> bool {
        let mut amp = 128u8;
        let mut out = 0u8;
        for i in 0..4 {
            self.timer[i] = self.timer[i].wrapping_add(self.freq[i]);
            if self.timer[i] & 0x80 != 0 {
                out = out.wrapping_add(amp);
            }
            amp >>= 1;
        }
        b.dac = out;
        b.hold = SCREAM_CYCLES;
        self.tick = self.tick.wrapping_add(1);
        if self.tick == 0 {
            let mut any = false;
            for i in 0..4 {
                if self.freq[i] != 0 {
                    if self.freq[i] == 0x37 && i + 1 < 4 {
                        self.freq[i + 1] = 0x41;
                    }
                    self.freq[i] -= 1;
                    any = true;
                }
            }
            if !any {
                return false;
            }
        }
        true
    }
}

/// What a one-shot board voice does: begin, then write until done.
/// `t` is seconds since the trigger, for scripts that restart or cut.
pub(crate) trait Script: Send {
    fn begin(&mut self, b: &mut Board);
    fn write(&mut self, b: &mut Board, t: f32) -> bool;
    /// A gain over time, for the one voice that fades (the fusion drone).
    fn envelope(&self, _t: f32) -> f32 {
        1.0
    }
}

/// A one-shot voice that runs a [`Script`] on a [`Board`].
pub struct BoardVoice<S: Script> {
    board: Board,
    script: S,
    level: f32,
    gain: f32,
    alive: bool,
    t: f32,
}

impl<S: Script> BoardVoice<S> {
    fn with(script: S, level: f32) -> Self {
        Self { board: Board::new(), script, level, gain: 1.0, alive: false, t: 0.0 }
    }
}

impl<S: Script> Voice for BoardVoice<S> {
    fn render(&mut self, out: &mut [f32], _params: VoiceParams, sample_rate: f32) {
        let per_sample = WILLIAMS_CLOCK / sample_rate;
        let r = dc_block(sample_rate);
        let dt = 1.0 / sample_rate;
        for sample in out.iter_mut() {
            if !self.alive {
                *sample = 0.0;
                continue;
            }
            let (script, t, alive) = (&mut self.script, self.t, &mut self.alive);
            let y = self.board.sample(per_sample, r, |b| {
                if !*alive || !script.write(b, t) {
                    *alive = false;
                    park(b);
                }
            });
            *sample = y * self.level * self.gain * self.script.envelope(self.t);
            self.t += dt;
        }
    }

    fn alive(&self) -> bool {
        self.alive
    }

    fn retrigger(&mut self, gain: f32, _pitch: f32) {
        self.gain = gain.clamp(0.0, 1.0);
        self.board.restart();
        self.t = 0.0;
        self.alive = true;
        self.script.begin(&mut self.board);
    }
}

// ----- the presets: ours, from the measured behaviour -----

/// ★ A LANDER TAKES SOMEONE (ED10): a six-note falling-then-back figure,
/// 787 / 726 / 673 / 628 / 553 / 726 Hz on a bright 16-step wave, six
/// passes a note, fifteen echoes decaying 5/16 with wrap — so it starts
/// sweet and goes ragged. ~0.83 s. Brian: "higher pitched".
fn grab_spec() -> GwaveSpec {
    GwaveSpec {
        wave: WaveTable::two_harmonic(16),
        cycles: 6,
        echoes: 15,
        echo_decay: 5,
        predecay: 3,
        freq_inc: 0,
        freq_count: 2,
        pattern: &[7, 8, 9, 10, 12, 8],
    }
}

/// ★ THE SHIP CATCHES THEM (SPNRV): an 8-step sine from 269 Hz whose
/// period shortens by 3 a pass — a rising bloop. Brian: "woo woo woop".
fn catch_spec() -> GwaveSpec {
    GwaveSpec {
        wave: WaveTable::sine(8),
        cycles: 5,
        echoes: 1,
        echo_decay: 0,
        predecay: 0,
        freq_inc: -3,
        freq_count: 0,
        pattern: &[64],
    }
}

/// ★ A LANDER OR BAITER FIRES (DP1V): a 72-step sine pre-decayed into
/// spikes, one pass at a time, each a period longer — 166 → 79 Hz in
/// 168 ms. A spitty "bzew".
fn lander_shot_spec() -> GwaveSpec {
    GwaveSpec {
        wave: WaveTable::sine(72),
        cycles: 1,
        echoes: 1,
        echo_decay: 0,
        predecay: 17,
        freq_inc: 1,
        freq_count: 15,
        pattern: &[8],
    }
}

/// ★ A MUTANT FIRES (CLDWN): a three-note trill (873 / 1396 / 2934 Hz) on
/// the odd 8-step wave, three quick echoes, then every period one longer:
/// a buzzy descending trill. Cut at 768 ms, the game's hold for it.
fn mutant_shot_spec() -> GwaveSpec {
    GwaveSpec {
        wave: WaveTable::odd8(),
        cycles: 1,
        echoes: 3,
        echo_decay: 1,
        predecay: 0,
        freq_inc: 1,
        freq_count: 0,
        pattern: &[16, 8, 1],
    }
}
const MUTANT_SHOT_CUT: f32 = 0.768;

// ----- the enemy dying: every kind its own (W5) -----
//
// The original gives every enemy its own death (defa7 sound table):
//
// | Kind              | Code  | Preset | Hold   | Full length | ROM render                     |
// |-------------------|-------|--------|--------|-------------|--------------------------------|
// | Bomber            | TIHSND | HBDV  | 160 ms | 1.22 s      | 01_G1_HBDV_bomberhit.wav       |
// | Pod               | PRHSND | BBSV  | 256 ms | 5.3 s       | 05_G5_BBSV_podhit.wav          |
// | Baiter, Swarmer   | UFHSND/SWHSND | PROTV | 128 ms | 1.09 s | 07_G7_PROTV_baiter_swarmhit.wav |
// | Lander            | LHSND  | HBEV  | 160 ms | 0.6 s       | 06_G6_HBEV_landerhit.wav       |
//
// ★ THE LANDER'S IS HBEV NOW. It was built as an A/B against Brian's
// `Boom`, and on 2026-10-09 he picked it: "let's use original Lander
// death". `Boom` stays in the file, unwired, as his reference.

/// ★ A BOMBER DIES (HBDV, "heartbeat distorto"): a square two cycles to
/// the table, one pass a period, periods doubling 1 → 192 — ~3.2 kHz down
/// to ~94 Hz in 152 ms — eight times, each echo 2/16 quieter with wrap.
/// A falling zip, repeated, souring as it goes. 1.22 s.
fn bomber_hit_spec() -> GwaveSpec {
    GwaveSpec {
        wave: WaveTable::square_twice(16),
        cycles: 1,
        echoes: 8,
        echo_decay: 2,
        predecay: 0,
        freq_inc: 0,
        freq_count: 0,
        pattern: &[1, 1, 2, 2, 4, 4, 8, 8, 16, 32, 40, 48, 56, 64, 72, 80, 96, 112, 128, 160, 176, 192],
    }
}

/// ★ A POD DIES (BBSV, "big ben"): a 16-step sine tolling between ~726 Hz
/// and ~135 Hz, four passes each, ten pairs; fifteen echoes, each 1/16
/// quieter. A bell. The full toll is 5.3 s; the game held it 256 ms and
/// the next sound cut it, so ours is cut at [`POD_HIT_CUT`].
fn pod_hit_spec() -> GwaveSpec {
    GwaveSpec {
        wave: WaveTable::sine(16),
        cycles: 4,
        echoes: 15,
        echo_decay: 1,
        predecay: 0,
        freq_inc: 0,
        freq_count: 0,
        pattern: &[8, 64, 8, 64, 8, 64, 8, 64, 8, 64, 8, 64, 8, 64, 8, 64, 8, 64, 8, 64],
    }
}
const POD_HIT_CUT: f32 = 0.768;

/// ★ A BAITER OR A SWARMER DIES (PROTV): the 72-step sine pre-decayed
/// into wrapped spikes, periods 1 … 12 (~380 → 126 Hz), twice, then every
/// period one SHORTER a round — so it climbs back up as the pattern
/// shrinks to nothing. A torn, rising wail. 1.09 s.
fn baiter_hit_spec() -> GwaveSpec {
    GwaveSpec {
        wave: WaveTable::sine(72),
        cycles: 1,
        echoes: 2,
        echo_decay: 3,
        predecay: 17,
        freq_inc: -1,
        freq_count: 0,
        pattern: &[1, 1, 2, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12],
    }
}

/// ★ A LANDER DIES (HBEV, "heartbeat echo") — Brian's pick over his own
/// `Boom` (2026-10-09). The
/// 72-step sine, periods 1 … 22 then 64 — ~380 → 78 → 30 Hz in 142 ms —
/// four echoes at 4/16 decay with wrap. A thud that sours. ~0.6 s.
fn lander_hit_spec() -> GwaveSpec {
    GwaveSpec {
        wave: WaveTable::sine(72),
        cycles: 1,
        echoes: 4,
        echo_decay: 4,
        predecay: 0,
        freq_inc: 0,
        freq_count: 0,
        pattern: &[1, 2, 4, 8, 9, 10, 11, 12, 14, 15, 16, 18, 20, 22, 64],
    }
}

/// ★ A SWARMER FIRES (ED12): the 8-step sine at periods 23 … 28 —
/// so few samples, so slowly (~5.5k a second), that it is all stair-
/// steps — six falling notes 658 → 559 Hz, ten passes each, six echoes
/// at 1/16. A gritty falling chirp. 0.6 s. ROM render
/// 0c_G12_ED12_swarmshoot.wav.
fn swarmer_shot_spec() -> GwaveSpec {
    GwaveSpec {
        wave: WaveTable::sine(8),
        cycles: 10,
        echoes: 6,
        echo_decay: 1,
        predecay: 2,
        freq_inc: 0,
        freq_count: 2,
        pattern: &[23, 24, 25, 26, 27, 28],
    }
}

/// ★ A GAME BEGINS (SV3, the 1-player start): the 72-step sine at
/// ~166 Hz, one pass at a time, 256 times — and after every pass the
/// table erodes by a sixteenth of the ROM wave, WITH WRAP, so the timbre
/// cycles about every 16 passes; each erosion leaves the DAC holding for
/// ~6 ms, which chops the tone. 3.14 s. ROM render 0a_G10_SV3_start1.wav.
fn game_start_spec() -> GwaveSpec {
    GwaveSpec {
        wave: WaveTable::sine(72),
        cycles: 1,
        echoes: 0,
        echo_decay: 1,
        predecay: 1,
        freq_inc: 1,
        freq_count: 1,
        pattern: &[8],
    }
}

/// ★ SET DOWN SAFE (QUASAR): rising sweeps, 378 → 1530 Hz, each ~0.26 s,
/// ten of them, each a little higher. Brian: "that classic phaser sound
/// when dropped to ground".
const SET_DOWN: VariSpec = VariSpec {
    lo: 40,
    hi: 129,
    lo_step: 0,
    hi_step: 0xFC,
    hi_end: 1,
    sweep: 512,
    lo_mod: 0xFC,
    amp: 0xFF,
};

/// ★ AN EXTRA SHIP (FOSHIT, "free ship"): QUASAR run the other way — a
/// square whose high half GROWS 1 → 129 by 8 counts every 512, so each
/// sweep FALLS ~1530 → 378 Hz in ~128 ms; then the low half shortens by
/// one and it sweeps again, forty times, each higher and quicker. 5.3 s.
/// The game held it 512 ms at top priority. ROM render
/// 1e_FOSHIT_freeship.wav.
const EXTRA_LIFE: VariSpec = VariSpec {
    lo: 40,
    hi: 1,
    lo_step: 0,
    hi_step: 8,
    hi_end: 129,
    sweep: 512,
    lo_mod: 0xFF,
    amp: 0xFF,
};

/// The short version's cut: about a dozen sweeps (research-sound.md:
/// "a shorter 10–12-sweep version also reads well").
const EXTRA_LIFE_SHORT: f32 = 1.5;

/// ★ A LANDER BECOMES A MUTANT (SP1): a buzz whose low half shortens by
/// 14 counts every 16 ms for ten steps (254 → 128), so it rises in steps
/// from ~250 Hz, over a high half sawing up 24 counts every 18 ms. The
/// original drones until another sound replaces it; ours drones for
/// [`FUSION_SECONDS`] and fades.
const FUSION: VariSpec = VariSpec {
    lo: 254,
    hi: 1,
    lo_step: 0,
    hi_step: 24,
    hi_end: 65,
    sweep: 1152,
    lo_mod: 0,
    amp: 0xFF,
};
const FUSION_STEPS: u8 = 10;
const FUSION_STEP_SECONDS: f32 = 0.016;
const FUSION_SECONDS: f32 = 1.1;
const FUSION_FADE: f32 = 0.3;

/// The SP1 low half for step `n` (1-based): the original's own
/// arithmetic — 254, 240, 226 … down by 14.
fn fusion_lo(n: u8) -> u8 {
    let mut a = 32 - n as i32;
    let mut lo = 0i32;
    while a > 20 {
        lo += 14;
        a -= 1;
    }
    lo += 5 * a;
    lo.clamp(1, 255) as u8
}

/// Levels. ⚠️ NOT PEAKS — each is rendered and measured in
/// `no_voice_clips_at_full_gain`.
const GRAB_LEVEL: f32 = 0.34;
const CATCH_LEVEL: f32 = 0.40;
const LANDER_SHOT_LEVEL: f32 = 0.26;
const MUTANT_SHOT_LEVEL: f32 = 0.24;
const SWARMER_SHOT_LEVEL: f32 = 0.24;
const GAME_START_LEVEL: f32 = 0.30;
const SET_DOWN_LEVEL: f32 = 0.30;
const EXTRA_LIFE_LEVEL: f32 = 0.30;
const FUSION_LEVEL: f32 = 0.26;
const SCREAM_LEVEL: f32 = 0.30;
const BOMBER_HIT_LEVEL: f32 = 0.30;
const POD_HIT_LEVEL: f32 = 0.30;
const BAITER_HIT_LEVEL: f32 = 0.30;
const LANDER_HIT_LEVEL: f32 = 0.30;

/// A single GWAVE preset, played once.
pub struct GwaveOnce {
    spec: GwaveSpec,
    run: Gwave,
    cut: f32,
}

impl Script for GwaveOnce {
    fn begin(&mut self, _b: &mut Board) {
        self.run = Gwave::new(self.spec);
    }
    fn write(&mut self, b: &mut Board, t: f32) -> bool {
        t < self.cut && self.run.write(b)
    }
}

fn gwave_once(spec: GwaveSpec, cut: f32, level: f32) -> BoardVoice<GwaveOnce> {
    BoardVoice::with(GwaveOnce { spec, run: Gwave::new(spec), cut }, level)
}

/// The grab (ED10).
pub fn grab() -> BoardVoice<GwaveOnce> {
    gwave_once(grab_spec(), f32::MAX, GRAB_LEVEL)
}

/// A Lander's or Baiter's shot (DP1V).
pub fn lander_shot() -> BoardVoice<GwaveOnce> {
    gwave_once(lander_shot_spec(), f32::MAX, LANDER_SHOT_LEVEL)
}

/// A Mutant's shot (CLDWN), cut where the game cuts it.
pub fn mutant_shot() -> BoardVoice<GwaveOnce> {
    gwave_once(mutant_shot_spec(), MUTANT_SHOT_CUT, MUTANT_SHOT_LEVEL)
}

/// One LITE, run out.
pub struct LiteOnce {
    run: Liten,
}

impl Script for LiteOnce {
    fn begin(&mut self, b: &mut Board) {
        self.run = Liten::lite();
        Liten::begin(b);
    }
    fn write(&mut self, b: &mut Board, _t: f32) -> bool {
        self.run.write(b)
    }
}

/// A person you shot (LITE).
pub fn person_crackle() -> BoardVoice<LiteOnce> {
    BoardVoice::with(LiteOnce { run: Liten::lite() }, PERSON_CRACKLE_LEVEL)
}

/// A game beginning (SV3).
pub fn game_start() -> BoardVoice<GwaveOnce> {
    gwave_once(game_start_spec(), f32::MAX, GAME_START_LEVEL)
}

/// A Swarmer's shot (ED12).
pub fn swarmer_shot() -> BoardVoice<GwaveOnce> {
    gwave_once(swarmer_shot_spec(), f32::MAX, SWARMER_SHOT_LEVEL)
}

/// A Bomber's death (HBDV).
pub fn bomber_hit() -> BoardVoice<GwaveOnce> {
    gwave_once(bomber_hit_spec(), f32::MAX, BOMBER_HIT_LEVEL)
}

/// A Pod's death (BBSV), cut where the game's next sound would cut it.
pub fn pod_hit() -> BoardVoice<GwaveOnce> {
    gwave_once(pod_hit_spec(), POD_HIT_CUT, POD_HIT_LEVEL)
}

/// A Baiter's or a Swarmer's death (PROTV).
pub fn baiter_hit() -> BoardVoice<GwaveOnce> {
    gwave_once(baiter_hit_spec(), f32::MAX, BAITER_HIT_LEVEL)
}

/// A Lander's death (HBEV).
pub fn lander_hit() -> BoardVoice<GwaveOnce> {
    gwave_once(lander_hit_spec(), f32::MAX, LANDER_HIT_LEVEL)
}

/// The catch: SPNRV three times, each restart 160 ms after the last —
/// the game re-sends it, and each send cuts the previous bloop off.
pub struct CatchScript {
    run: Gwave,
    sends: u8,
}
const CATCH_SENDS: u8 = 3;
const CATCH_INTERVAL: f32 = 0.160;

impl Script for CatchScript {
    fn begin(&mut self, _b: &mut Board) {
        self.run = Gwave::new(catch_spec());
        self.sends = 1;
    }
    fn write(&mut self, b: &mut Board, t: f32) -> bool {
        if self.sends < CATCH_SENDS && t >= self.sends as f32 * CATCH_INTERVAL {
            self.sends += 1;
            self.run = Gwave::new(catch_spec());
        }
        self.run.write(b)
    }
}

pub fn catch() -> BoardVoice<CatchScript> {
    BoardVoice::with(CatchScript { run: Gwave::new(catch_spec()), sends: 1 }, CATCH_LEVEL)
}

/// A routine the game's sound table can send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Routine {
    Turbo,
    Lite,
    Cannon,
}

/// One running routine.
#[derive(Debug, Clone, Copy)]
enum Running {
    Turbo(Turbo),
    Lite(Liten),
    Cannon(Cannon),
    Done,
}

impl Running {
    fn start(send: Routine, b: &mut Board) -> Self {
        match send {
            Routine::Turbo => Running::Turbo(Turbo::new()),
            Routine::Lite => {
                Liten::begin(b);
                Running::Lite(Liten::lite())
            }
            Routine::Cannon => Running::Cannon(Cannon::new(b)),
        }
    }

    fn write(&mut self, b: &mut Board) -> bool {
        let more = match self {
            Running::Turbo(r) => r.write(b),
            Running::Lite(r) => r.write(b),
            Running::Cannon(r) => r.write(b),
            Running::Done => false,
        };
        if !more {
            *self = Running::Done;
        }
        more
    }
}

/// ★ THE GAME'S SOUND TABLE, PLAYED: a list of (routine, repeats, timer
/// in 16 ms frames). Each send cuts whatever was playing (the board's
/// IRQ resets its stack); after the last timer the last routine runs out
/// on its own. This is how defa7's multi-part sounds — the planet going,
/// the ship dying, the smart bomb — were made from single routines.
pub struct Sequence {
    steps: &'static [(Routine, u8, u8)],
    running: Running,
    /// Index into `steps`, and how many sends of it have gone.
    step: usize,
    sent: u8,
    /// When the next send is due, in seconds since the trigger.
    next_at: f32,
}

/// One frame of the game's sound timer.
const SOUND_FRAME: f32 = 0.016;

impl Sequence {
    fn new(steps: &'static [(Routine, u8, u8)]) -> Self {
        Self { steps, running: Running::Done, step: 0, sent: 0, next_at: 0.0 }
    }

    fn send(&mut self, b: &mut Board) {
        let (what, _, frames) = self.steps[self.step];
        self.running = Running::start(what, b);
        self.sent += 1;
        self.next_at += frames as f32 * SOUND_FRAME;
    }
}

impl Script for Sequence {
    fn begin(&mut self, b: &mut Board) {
        self.step = 0;
        self.sent = 0;
        self.next_at = 0.0;
        self.send(b);
    }

    fn write(&mut self, b: &mut Board, t: f32) -> bool {
        if t >= self.next_at {
            let (_, repeats, _) = self.steps[self.step];
            if self.sent < repeats {
                self.send(b);
            } else if self.step + 1 < self.steps.len() {
                self.step += 1;
                self.sent = 0;
                self.send(b);
            } else {
                // The last send runs out on its own.
                self.next_at = f32::MAX;
            }
        }
        self.running.write(b)
    }
}

/// ★ THE PLANET GOES (TBSND, defa7): TURBO once for 64 ms, LITE twice
/// 96 ms apart, CANNON twice 160 ms apart, then that CANNON's whole
/// tail. Laser noise, crackle, the explosion restarted once — ~3 s.
/// research-sound.md §2, "Planet explodes".
const PLANET: &[(Routine, u8, u8)] = &[(Routine::Turbo, 1, 4), (Routine::Lite, 2, 6), (Routine::Cannon, 2, 10)];
const PLANET_LEVEL: f32 = 0.42;

/// The planet exploding (TBSND).
pub fn planet() -> BoardVoice<Sequence> {
    BoardVoice::with(Sequence::new(PLANET), PLANET_LEVEL)
}

/// One VARI preset, played once.
pub struct VariOnce {
    spec: VariSpec,
    run: Option<Vari>,
    /// Seconds after which the sound is cut, as the game's next sound
    /// would cut it. `f32::MAX` runs it out.
    cut: f32,
}

impl Script for VariOnce {
    fn begin(&mut self, b: &mut Board) {
        self.run = Some(Vari::new(self.spec, b));
    }
    fn write(&mut self, b: &mut Board, t: f32) -> bool {
        t < self.cut && self.run.as_mut().is_some_and(|v| v.write(b))
    }
}

/// The set-down (QUASAR).
pub fn set_down() -> BoardVoice<VariOnce> {
    BoardVoice::with(VariOnce { spec: SET_DOWN, run: None, cut: f32::MAX }, SET_DOWN_LEVEL)
}

/// The extra ship (FOSHIT), run out in full — the A/B alternative Brian
/// did not pick (2026-10-09); kept for laser_check and the length test.
#[allow(dead_code)]
pub fn extra_life() -> BoardVoice<VariOnce> {
    BoardVoice::with(VariOnce { spec: EXTRA_LIFE, run: None, cut: f32::MAX }, EXTRA_LIFE_LEVEL)
}

/// The extra ship (FOSHIT), cut after [`EXTRA_LIFE_SHORT`] so it does
/// not sit over the next fight. ★ Brian's pick: "short version def."
pub fn extra_life_short() -> BoardVoice<VariOnce> {
    BoardVoice::with(VariOnce { spec: EXTRA_LIFE, run: None, cut: EXTRA_LIFE_SHORT }, EXTRA_LIFE_LEVEL)
}

/// The fusion (SP1): ten stepped restarts, then the drone, then a fade.
pub struct FusionScript {
    run: Option<Vari>,
    step: u8,
}

impl FusionScript {
    fn restart(&mut self, b: &mut Board) {
        self.step += 1;
        let spec = VariSpec { lo: fusion_lo(self.step), ..FUSION };
        self.run = Some(Vari::new(spec, b));
    }
}

impl Script for FusionScript {
    fn begin(&mut self, b: &mut Board) {
        self.step = 0;
        self.restart(b);
    }
    fn write(&mut self, b: &mut Board, t: f32) -> bool {
        if t >= FUSION_SECONDS {
            return false;
        }
        if self.step < FUSION_STEPS && t >= self.step as f32 * FUSION_STEP_SECONDS {
            self.restart(b);
        }
        // SP1 loops VARI forever: when one run ends, start it again.
        if !self.run.as_mut().is_some_and(|v| v.write(b)) {
            let spec = VariSpec { lo: fusion_lo(self.step), ..FUSION };
            self.run = Some(Vari::new(spec, b));
        }
        true
    }
    fn envelope(&self, t: f32) -> f32 {
        ((FUSION_SECONDS - t) / FUSION_FADE).clamp(0.0, 1.0)
    }
}

pub fn fusion() -> BoardVoice<FusionScript> {
    BoardVoice::with(FusionScript { run: None, step: 0 }, FUSION_LEVEL)
}

/// ★ THE SCREAM — a CONTINUOUS voice, not a one-shot.
///
/// It runs while someone is falling and stops the moment they are caught
/// or land: the original's board was monophonic and the next sound cut
/// it; a one-shot here could not be cut and would scream on over the
/// catch. Slot 0 of its params is the count of falls begun — when that
/// changes, a new fall has started and the scream starts over.
pub struct Scream {
    board: Board,
    run: ScreamRoutine,
    fall: f32,
    alive: bool,
}

impl Default for Scream {
    fn default() -> Self {
        Self::new()
    }
}

impl Scream {
    pub fn new() -> Self {
        Self { board: Board::new(), run: ScreamRoutine::new(), fall: -1.0, alive: false }
    }

    /// The params that drive it: which fall this is.
    pub fn params(falls: u32) -> VoiceParams {
        VoiceParams::new([falls as f32, 0.0, 0.0, 0.0])
    }
}

impl Voice for Scream {
    fn render(&mut self, out: &mut [f32], params: VoiceParams, sample_rate: f32) {
        let fall = params.get(0);
        if fall != self.fall {
            self.fall = fall;
            self.board.restart();
            self.run = ScreamRoutine::new();
            self.alive = true;
        }
        let per_sample = WILLIAMS_CLOCK / sample_rate;
        let r = dc_block(sample_rate);
        for sample in out.iter_mut() {
            if !self.alive {
                *sample = 0.0;
                continue;
            }
            let (run, alive) = (&mut self.run, &mut self.alive);
            let y = self.board.sample(per_sample, r, |b| {
                if !*alive || !run.write(b) {
                    *alive = false;
                    park(b);
                }
            });
            *sample = y * SCREAM_LEVEL;
        }
    }
}

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
    /// A Lander, a Mutant, your ship and a shot Humanoid once shared one
    /// boom, and the game said the same thing for "you scored" and "you
    /// lost a life". Since W5 three of them are the original's (HBEV,
    /// PDSND, LITE) and the Mutant's is Brian's; this pins the properties
    /// the split buys, on the voices that actually play:
    /// · your own death outlasts every other;
    /// · a Mutant bites brighter than your death rumbles;
    /// · ★ BRIAN'S RULE: shooting a person is QUIETER than killing a
    ///   Lander or a Mutant — a mistake, not an achievement.
    #[test]
    fn the_four_deaths_are_distinguishable() {
        let ship_len = run_length(&mut SmartBomb::ship_death());
        for (name, len) in [
            ("lander", run_length(&mut lander_hit())),
            ("mutant", run_length(&mut MutantBoom::new())),
            ("person", run_length(&mut person_crackle())),
        ] {
            assert!(ship_len > len, "your own death ({ship_len:.2} s) must outlast a {name}'s ({len:.2} s)");
        }

        let render = |v: &mut dyn Voice, secs: f32| {
            v.retrigger(1.0, 1.0);
            render_all(v, secs)
        };
        let l = render(&mut lander_hit(), 0.3);
        let m = render(&mut MutantBoom::new(), 0.3);
        let s = render(&mut SmartBomb::ship_death(), 1.5);
        let p = render(&mut person_crackle(), 0.3);

        // Brightness, over the explosion tail of the death (past the
        // crackles), measured rather than read from constants.
        let bright = |v: &[f32]| band_rms(v, 1200.0) / band_rms(v, 200.0).max(1e-9);
        let tail = &s[(0.4 * SR) as usize..];
        assert!(
            bright(&m) > bright(tail),
            "a Mutant must be brighter than your own death: {:.3} vs {:.3}",
            bright(&m),
            bright(tail)
        );

        let rms = |v: &[f32]| (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
        assert!(rms(&p) < rms(&l), "shooting a person ({:.3}) must be quieter than killing a Lander ({:.3})", rms(&p), rms(&l));
        assert!(rms(&p) < rms(&m), "shooting a person ({:.3}) must be quieter than killing a Mutant ({:.3})", rms(&p), rms(&m));
    }

    /// ⚠️ NOTHING MAY CLIP. ★ THIS TEST EXISTS BECAUSE IT HAPPENED:
    /// a placeholder ship death (since replaced) shipped at LEVEL 0.42 and rendered a peak of 1.03,
    /// because a resonant lowpass at Q 6.4 adds gain the level constant
    /// says nothing about. The WAV writer clamps, so the rendered file
    /// sounded plausible while the real mixer would have hard-clipped.
    /// ⇒ A `_LEVEL` CONSTANT IS NOT THE PEAK. Measure the render.
    #[test]
    fn no_voice_clips_at_full_gain() {
        let cases: [(&str, &mut dyn Voice, f32); 22] = [
            ("laser", &mut Laser::new(), ZAP_LEN),
            ("bomber hit", &mut bomber_hit(), 1.4),
            ("swarmer shot", &mut swarmer_shot(), 0.8),
            ("ship death", &mut SmartBomb::ship_death(), 3.0),
            ("planet", &mut planet(), 3.2),
            ("game start", &mut game_start(), 3.3),
            ("person crackle", &mut person_crackle(), 0.8),
            ("extra life", &mut extra_life(), 5.6),
            ("pod hit", &mut pod_hit(), 1.0),
            ("baiter hit", &mut baiter_hit(), 1.3),
            ("lander hit", &mut lander_hit(), 0.8),
            ("smart bomb", &mut SmartBomb::new(), 3.0),
            ("hyperspace", &mut Hyperspace::new(), 0.7),
            ("grab", &mut grab(), 1.0),
            ("catch", &mut catch(), 0.7),
            ("set-down", &mut set_down(), 2.8),
            ("fusion", &mut fusion(), 1.2),
            ("lander shot", &mut lander_shot(), 0.3),
            ("mutant shot", &mut mutant_shot(), 0.9),
            ("lander", &mut Boom::new(), BOOM_LEN),
            ("mutant", &mut MutantBoom::new(), MUTANT_BOOM_LEN),
            ("warp", &mut Warp::new(), WARP_LEN),
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
            ("ship", &mut SmartBomb::ship_death(), 2.2),
            ("person", &mut person_crackle(), 0.55),
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

    /// How long a one-shot runs before it retires itself, in seconds.
    fn run_length(v: &mut dyn Voice) -> f32 {
        v.retrigger(1.0, 1.0);
        let mut block = [0.0f32; 256];
        let mut n = 0usize;
        while v.alive() && n < (10.0 * SR) as usize {
            v.render(&mut block, VoiceParams::SILENT, SR);
            n += block.len();
        }
        n as f32 / SR
    }

    /// ★★ EVERY HIT RUNS AS LONG AS THE ORIGINAL'S (W5), measured by
    /// running the sound ROM in the emulator: HBDV 1.22 s, PROTV 1.085 s,
    /// HBEV ~0.6 s; the Pod's 5.3 s toll is cut at the game's 768 ms.
    /// ⚠️ PROTV IS THE CPU-GAP GUARD: without the silences while the
    /// board rewrites its wave it runs 0.59 s, and this fails.
    #[test]
    fn every_hit_runs_as_long_as_the_original() {
        let cases: [(&str, &mut dyn Voice, f32, f32); 4] = [
            ("bomber", &mut bomber_hit(), 1.15, 1.27),
            ("baiter/swarmer", &mut baiter_hit(), 1.03, 1.13),
            ("lander", &mut lander_hit(), 0.50, 0.66),
            ("pod", &mut pod_hit(), 0.76, 0.78),
        ];
        for (name, v, lo, hi) in cases {
            let t = run_length(v);
            assert!((lo..=hi).contains(&t), "{name} hit ran {t:.3} s, want {lo}..{hi}");
        }
    }

    /// ★ THE EXTRA SHIP RUNS AS LONG AS FOSHIT (5.32 s by the emulator):
    /// forty sweeps, each a count shorter. The short A/B cuts at 1.5 s.
    #[test]
    fn the_extra_life_runs_as_long_as_the_original() {
        let t = run_length(&mut extra_life());
        assert!((5.1..=5.5).contains(&t), "extra life ran {t:.3} s");
        let t = run_length(&mut extra_life_short());
        assert!((1.49..=1.52).contains(&t), "short extra life ran {t:.3} s");
    }

    /// ★ W5: THE SHIP'S DEATH IS TWO CRACKLES 128 MS APART, THEN THE
    /// WHOLE CANNON TAIL (PDSND): 0.256 + 2.58 s. The crackle you get for
    /// shooting a person is one LITE, 0.70 s.
    #[test]
    fn the_ship_death_and_the_person_crackle_run_as_long_as_the_original() {
        let t = run_length(&mut SmartBomb::ship_death());
        assert!((2.7..=2.95).contains(&t), "ship death ran {t:.3} s");
        let t = run_length(&mut person_crackle());
        assert!((0.62..=0.75).contains(&t), "person crackle ran {t:.3} s");
        // And the smart bomb is still six stutters: 0.384 + 2.58 s.
        let t = run_length(&mut SmartBomb::new());
        assert!((2.85..=3.1).contains(&t), "smart bomb ran {t:.3} s");
    }

    /// ★ W5: THE PLANET'S SEQUENCE KEEPS THE GAME'S TIMERS (TBSND):
    /// 64 ms of TURBO, two LITEs 96 ms apart, a CANNON cut at 160 ms, then
    /// a second CANNON run out (2.58 s) — ~3.0 s. And TURBO alone runs the
    /// 9.79 s the emulator times the original's laser at.
    #[test]
    fn the_planet_keeps_the_sound_tables_timing() {
        let t = run_length(&mut planet());
        assert!((2.9..=3.1).contains(&t), "planet ran {t:.3} s");
        const TURBO_ALONE: &[(Routine, u8, u8)] = &[(Routine::Turbo, 1, 1)];
        let t = run_length(&mut BoardVoice::with(Sequence::new(TURBO_ALONE), 0.3));
        assert!((9.6..=9.95).contains(&t), "TURBO ran {t:.3} s");
    }

    /// ★ W5: A GAME BEGINS WITH SV3, 256 passes (an echo count of 0 is
    /// 256 on the board) — 3.14 s by the emulator. With the echo count
    /// read as 0 it would play once (~6 ms); with it panicking on
    /// underflow, not at all.
    #[test]
    fn the_game_start_runs_as_long_as_the_original() {
        let t = run_length(&mut game_start());
        assert!((3.0..=3.25).contains(&t), "game start ran {t:.3} s");
    }

    /// ★ THE SWARMER'S SHOT RUNS AS LONG AS ED12 (0.60 s by the emulator).
    #[test]
    fn the_swarmer_shot_runs_as_long_as_the_original() {
        let t = run_length(&mut swarmer_shot());
        assert!((0.55..=0.65).contains(&t), "swarmer shot ran {t:.3} s");
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

    /// Brightness of a slice: RMS of the first difference over RMS. High
    /// for crackle, near zero for a slow rumble.
    fn brightness(samples: &[f32]) -> f32 {
        let d: Vec<f32> = samples.windows(2).map(|w| w[1] - w[0]).collect();
        let r = |s: &[f32]| (s.iter().map(|x| x * x).sum::<f32>() / s.len().max(1) as f32).sqrt();
        r(&d) / r(samples).max(1e-9)
    }

    /// ★ THE SMART BOMB RUNS AS LONG AS THE ORIGINAL'S: the stutter
    /// (384 ms) plus CANNON's 72 decays (~2.58 s), audible to ~2.9 s in
    /// the emulated ROM render. Then it retires.
    #[test]
    fn the_smart_bomb_lasts_as_long_as_the_original_and_retires() {
        let mut b = SmartBomb::new();
        assert!(!b.alive());
        b.retrigger(1.0, 1.0);
        let s = render_all(&mut b, 3.4);
        assert!(!b.alive(), "the bomb never retired");
        let end = s.iter().rposition(|x| x.abs() > 1e-4).unwrap() as f32 / SR;
        assert!((2.8..=3.1).contains(&end), "it ended at {end:.2} s");
        assert!(s.iter().all(|x| x.is_finite()));
    }

    /// ★ CRACKLE, THEN EXPLOSION. The stutter is 1-bit noise and bright;
    /// the CANNON tail darkens as its slope decays. Measured on the ROM
    /// render: brightness 0.32 in the stutter, ~0.01 by 2 s.
    #[test]
    fn the_smart_bomb_crackles_and_then_rumbles() {
        let mut b = SmartBomb::new();
        b.retrigger(1.0, 1.0);
        let s = render_all(&mut b, 3.0);
        let at = |t: f32| (t * SR) as usize;
        let crackle = brightness(&s[at(0.05)..at(0.35)]);
        let tail = brightness(&s[at(1.8)..at(2.2)]);
        assert!(crackle > 0.2, "the stutter is not crackle: {crackle:.3}");
        assert!(tail < crackle * 0.15, "the tail did not darken: {crackle:.3} → {tail:.3}");
    }

    /// ★ THE STUTTER IS SIX RESTARTS. Each one snaps LITE back to its
    /// fastest clock, so the crackle is brighter just after every 64 ms
    /// boundary than just before it.
    #[test]
    fn the_smart_bomb_stutters_six_times() {
        let mut b = SmartBomb::new();
        b.retrigger(1.0, 1.0);
        let s = render_all(&mut b, 0.5);
        let win = (0.006 * SR) as usize;
        for k in 1..6 {
            let edge = (k as f32 * 0.064 * SR) as usize;
            let before = brightness(&s[edge - win..edge]);
            let after = brightness(&s[edge + win / 4..edge + win]);
            assert!(after > before * 1.2, "no restart at {} ms: {before:.3} → {after:.3}", k * 64);
        }
    }

    // ----- the people's sounds and the enemy's guns -----

    /// Zero crossings per second of a slice: a pitch proxy that needs no
    /// FFT and does not care about level.
    fn crossings(s: &[f32]) -> f32 {
        let n = s.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count();
        n as f32 / (s.len() as f32 / SR)
    }

    fn end_of(v: &mut dyn Voice, seconds: f32) -> (Vec<f32>, f32) {
        v.retrigger(1.0, 1.0);
        let s = render_all(v, seconds);
        let end = s.iter().rposition(|x| x.abs() > 1e-4).unwrap_or(0) as f32 / SR;
        (s, end)
    }

    /// ★ EACH LASTS AS LONG AS THE ORIGINAL'S (measured from the ROM
    /// renders: grab 0.83 s, set-down 2.64 s, Lander shot 0.17 s; the
    /// catch is three bloops 160 ms apart; the Mutant's shot is cut at the
    /// game's 768 ms; the fusion is ours at 1.1 s) — and each retires.
    #[test]
    fn the_new_voices_last_as_long_as_the_originals() {
        let cases: [(&str, &mut dyn Voice, f32, f32, f32); 6] = [
            ("grab", &mut grab(), 1.5, 0.72, 0.92),
            ("catch", &mut catch(), 1.0, 0.45, 0.65),
            ("set-down", &mut set_down(), 3.5, 2.45, 2.75),
            ("lander shot", &mut lander_shot(), 0.6, 0.11, 0.21),
            ("mutant shot", &mut mutant_shot(), 1.2, 0.70, 0.78),
            ("fusion", &mut fusion(), 1.6, 0.9, FUSION_SECONDS + 0.02),
        ];
        for (name, v, secs, lo, hi) in cases {
            let (s, end) = end_of(v, secs);
            assert!((lo..=hi).contains(&end), "{name} ended at {end:.2} s");
            assert!(!v.alive(), "{name} never retired");
            assert!(s.iter().all(|x| x.is_finite()), "{name} emitted a non-finite sample");
        }
    }

    /// ★ THE SET-DOWN RISES: each sweep climbs 378 → 1530 Hz — "that
    /// classic phaser sound".
    #[test]
    fn the_set_down_sweeps_upward() {
        let (s, _) = end_of(&mut set_down(), 0.3);
        let at = |t: f32| (t * SR) as usize;
        let low = crossings(&s[at(0.01)..at(0.07)]);
        let high = crossings(&s[at(0.18)..at(0.24)]);
        assert!(high > low * 1.5, "the sweep did not rise: {low:.0} → {high:.0} crossings/s");
    }

    /// ★ AND THE SCREAM, AND BOTH GUNS, FALL.
    #[test]
    fn the_scream_and_the_shots_fall() {
        let at = |t: f32| (t * SR) as usize;

        let mut scream = Scream::new();
        let mut s = Vec::new();
        let mut block = [0.0f32; 256];
        while s.len() < at(2.2) {
            scream.render(&mut block, Scream::params(1), SR);
            s.extend_from_slice(&block);
        }
        let (early, late) = (crossings(&s[at(0.1)..at(0.5)]), crossings(&s[at(1.6)..at(2.0)]));
        assert!(early > late * 1.3, "the scream did not fall: {early:.0} → {late:.0}");

        let (s, _) = end_of(&mut lander_shot(), 0.3);
        let (early, late) = (crossings(&s[at(0.0)..at(0.04)]), crossings(&s[at(0.10)..at(0.14)]));
        assert!(early > late, "the Lander's shot did not fall: {early:.0} → {late:.0}");

        let (s, _) = end_of(&mut mutant_shot(), 0.9);
        let (early, late) = (crossings(&s[at(0.02)..at(0.15)]), crossings(&s[at(0.55)..at(0.74)]));
        assert!(early > late * 1.5, "the Mutant's shot did not fall: {early:.0} → {late:.0}");
    }

    /// The scream starts over when a new fall begins, and is silent once
    /// its own course has run.
    #[test]
    fn the_scream_restarts_for_each_fall() {
        let mut scream = Scream::new();
        let mut block = [0.0f32; 256];
        for _ in 0..(6.0 * SR / 256.0) as usize {
            scream.render(&mut block, Scream::params(1), SR);
        }
        assert!(peak(&block) < 1e-3, "the scream outlived its own course");
        let mut fresh = Vec::new();
        for _ in 0..40 {
            scream.render(&mut block, Scream::params(2), SR);
            fresh.extend_from_slice(&block);
        }
        assert!(peak(&fresh) > 0.05, "a new fall did not restart the scream");
    }

    /// ★ THE ORIGINAL'S DECAY WRAPS, IT DOES NOT CLAMP — a sample of 10
    /// losing 15 goes to 251, not 0. The wrap IS the GWAVE's grit.
    #[test]
    fn gwave_decay_wraps_rather_than_clamping() {
        let mut wave = WaveTable::sine(8);
        wave.data[0] = 255; // rom >> 4 = 15
        let spec = GwaveSpec { wave, ..catch_spec() };
        let mut g = Gwave::new(spec);
        g.ram[0] = 10;
        g.decay(1);
        assert_eq!(g.ram[0], 251);
    }

    /// The fusion's steps are the original's own arithmetic: 254, 240,
    /// 226 … down by 14 to 128 over ten sends.
    #[test]
    fn the_fusion_buzz_steps_up_by_fourteen() {
        let steps: Vec<u8> = (1..=10).map(fusion_lo).collect();
        assert_eq!(steps, vec![254, 240, 226, 212, 198, 184, 170, 156, 142, 128]);
    }

    /// ★ HYPERSPACE GOES OUT AND COMES BACK IN: the departure's crackle
    /// falls (LITE, clock slowing), the arrival's rises (APPEAR, clock
    /// quickening) — measured: 0.37 → 0.15, then 0.14 → 0.36. It ends,
    /// and retires.
    #[test]
    fn hyperspace_breaks_up_and_comes_back_together() {
        let mut h = Hyperspace::new();
        h.retrigger(1.0, 1.0);
        let s = render_all(&mut h, 0.9);
        assert!(!h.alive(), "the jump never retired");
        assert!(s.iter().all(|x| x.is_finite()));
        let at = |t: f32| (t * SR) as usize;
        let leaving = (brightness(&s[at(0.0)..at(0.016)]), brightness(&s[at(0.048)..at(0.064)]));
        assert!(leaving.1 < leaving.0, "the departure did not fall: {leaving:?}");
        let arriving = (brightness(&s[at(0.08)..at(0.16)]), brightness(&s[at(0.58)..at(0.65)]));
        assert!(arriving.1 > arriving.0 * 1.5, "the arrival did not rise: {arriving:?}");
    }

    /// ★★ THE WARP-IN SWEEPS UP, where every other filtered voice in
    /// this file sweeps DOWN.
    ///
    /// ⚠️ THIS IS THE CHARACTER, NOT A TUNING DETAIL. A falling sweep is
    /// something collapsing; a rising one is something assembling, and
    /// the ear reads the direction before it reads anything else. If
    /// this ever inverts, the Lander will sound like it is leaving.
    #[test]
    fn the_warp_sweeps_upward() {
        let mut w = Warp::new();
        w.retrigger(1.0, 1.0);
        let s = render_all(&mut w, WARP_LEN);
        let n = s.len();
        // Compare the first third against the last third, by the energy
        // above and below the filter's own midpoint.
        let early = &s[n / 8..n / 3];
        let late = &s[n * 2 / 3..n * 7 / 8];
        let lift = |x: &[f32]| {
            let hi: f32 = x.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum::<f32>()
                / (x.len() - 1) as f32;
            let r: f32 = x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32;
            (hi / r.max(1e-12)).sqrt()
        };
        let (a, b) = (lift(early), lift(late));
        assert!(
            b > a * 1.10,
            "the warp does not brighten as it arrives: early {a:.4} vs late {b:.4}"
        );
    }

    /// ★ IT BUILDS RATHER THAN HITS. A 38% attack is what separates this
    /// from every explosion in the file, which open in 2-10%.
    #[test]
    fn the_warp_builds_rather_than_hits() {
        assert!(
            WARP_ATTACK > 0.25,
            "WARP_ATTACK {WARP_ATTACK} is short enough to read as an impact"
        );
        let mut w = Warp::new();
        w.retrigger(1.0, 1.0);
        let s = render_all(&mut w, WARP_LEN);
        let n = s.len();
        // The loudest moment is NOT at the start.
        let first = peak(&s[..n / 8]);
        let mid = peak(&s[n / 3..n * 2 / 3]);
        assert!(
            mid > first * 1.5,
            "the warp peaks at its onset like an explosion: {first:.4} then {mid:.4}"
        );
    }

    /// ★ IT LANDS WITH THE ANIMATION. `enemy::WARP_SECONDS` is 0.55 and
    /// the voice is 0.575 — a voice that outlasted the materialising
    /// would still be arriving after the Lander had arrived.
    #[test]
    fn the_warp_matches_the_animation() {
        let diff = (WARP_LEN - crate::enemy::WARP_SECONDS).abs();
        assert!(
            diff < 0.08,
            "the warp voice ({WARP_LEN}) and the warp animation ({}) have \
             drifted apart by {diff:.3}s",
            crate::enemy::WARP_SECONDS
        );
    }

    /// Render a continuous voice at a HELD parameter value.
    ///
    /// ⚠️ `render_all` feeds `VoiceParams::SILENT`, which is exactly
    /// right for the five one-shots and useless for a voice whose whole
    /// shape comes from its parameter — it would measure `Thrust` at
    /// zero exhaust and conclude the engine is silent.
    fn render_held(v: &mut dyn Voice, params: VoiceParams, seconds: f32) -> Vec<f32> {
        let mut out = Vec::new();
        let mut block = [0.0f32; 256];
        let blocks = (seconds * SR / 256.0) as usize + 1;
        for _ in 0..blocks {
            v.render(&mut block, params, SR);
            out.extend_from_slice(&block);
        }
        out
    }

    fn rms(samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    /// ★★ THE DEFINING PROPERTY OF A CONTINUOUS VOICE, AND THE ONE THAT
    /// SEPARATES IT FROM EVERY OTHER VOICE IN THIS FILE.
    ///
    /// A one-shot is over when it is over. Thrust is over when the
    /// player says so. If this ever retires itself, the engine goes
    /// permanently silent mid-flight and nothing re-registers it.
    #[test]
    fn thrust_never_retires() {
        let mut t = Thrust::new();
        let held = VoiceParams::thrust(1.0);
        // Far longer than the longest one-shot in the file.
        let s = render_held(&mut t, held, 5.0);
        assert!(t.alive(), "thrust retired itself — the engine would cut out");
        // And it is still making sound at the end, not merely "alive".
        let tail = &s[s.len() - 4096..];
        assert!(
            rms(tail) > 0.01,
            "thrust went quiet while held: tail rms {:.5}",
            rms(tail)
        );
    }

    /// ★ LOUDNESS TRACKS `exhaust`. The parameter IS the envelope, so
    /// this is the test that the voice is driven at all rather than
    /// free-running.
    #[test]
    fn thrust_follows_exhaust() {
        let mut quiet = Thrust::new();
        let mut loud = Thrust::new();
        let a = rms(&render_held(&mut quiet, VoiceParams::thrust(0.25), 0.5));
        let b = rms(&render_held(&mut loud, VoiceParams::thrust(1.0), 0.5));
        assert!(
            b > a * 2.0,
            "full thrust ({b:.4}) is not meaningfully louder than quarter ({a:.4})"
        );
    }

    /// ★ AND SO DOES BRIGHTNESS — the corner rides load, which is what
    /// makes it read as an engine opening up rather than a volume knob.
    ///
    /// Measured as high-band energy against low, so it cannot be passed
    /// by simply getting louder: both signals are normalised by their
    /// own RMS first.
    #[test]
    fn thrust_brightens_under_load() {
        fn brightness(exhaust: f32) -> f32 {
            let mut t = Thrust::new();
            let s = render_held(&mut t, VoiceParams::thrust(exhaust), 0.5);
            // Skip the filter's settling transient.
            let s = &s[4096..];
            let r = rms(s).max(1e-9);
            // A crude one-pole highpass: the difference between
            // successive samples is energy that survived the lowpass.
            let hi: f32 = s.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum::<f32>()
                / (s.len() - 1) as f32;
            hi.sqrt() / r
        }
        let idle = brightness(0.15);
        let full = brightness(1.0);
        assert!(
            full > idle * 1.15,
            "brightness does not track load: idle {idle:.4} vs full {full:.4}"
        );
    }

    /// ★★ THE WOBBLE HAS AUTHORITY — measured on the CORNER, not on
    /// the output.
    ///
    /// ⚠️⚠️ THIS TEST EXISTS BECAUSE A DEAD CONTROL ONCE SHIPPED. An
    /// un-normalised one-pole smoother moved the corner by about a
    /// hundredth of an octave, and the wobbled render measured identical
    /// to the flat one to three decimal places: wired, shipped, and
    /// doing nothing.
    ///
    /// ⚠️ AND THE OBVIOUS TEST DOES NOT CATCH THAT. Window RMS over a
    /// 2-second render drifts 0.118 with the wobble DEAD and 0.129 at
    /// the shipped depth — white noise is lumpy enough at 50 ms to
    /// swallow the signal whole. Brightness-per-window is no better
    /// (0.073 dead, 0.079 live). ±0.15 octaves is genuinely subtle;
    /// that is why Brian chose it, and it means no statistic over the
    /// OUTPUT separates it from noise reliably.
    /// ⇒ SO MEASURE THE MECHANISM. `wobble()` is deterministic given the
    /// seed, and what it must deliver is a stated number of octaves.
    #[test]
    fn the_wobble_actually_moves() {
        let mut t = Thrust::new();
        // One second of the modulator alone.
        let n = SR as usize;
        let mut w = Vec::with_capacity(n);
        for _ in 0..n {
            w.push(t.wobble(SR));
        }

        let mean = w.iter().sum::<f32>() / n as f32;
        let std = (w.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / n as f32).sqrt();

        // ★ THE NORMALISATION IS THE CLAIM: `wobble()` returns roughly
        // unit variance, so `depth` reads directly as octaves. Without
        // it this lands near 0.023 and the whole control is inert.
        assert!(
            (0.7..=1.4).contains(&std),
            "wobble is not unit-variance: std {std:.4}. Depth no longer means \
             octaves — this is exactly the dead-control bug."
        );

        // And what that buys, in the units the constant is written in:
        // the corner actually swings by something close to the stated
        // depth rather than a rounding error.
        let swing = w.iter().fold(0.0f32, |m, x| m.max(x.abs())) * THRUST_WOBBLE_OCTAVES;
        assert!(
            swing > 0.10,
            "the corner barely moves: peak swing {swing:.4} octaves against a \
             depth of {THRUST_WOBBLE_OCTAVES}"
        );
    }

    /// ★★ THE RESONANCE STAYS LOW, and this is a design constraint
    /// rather than a tuning preference.
    ///
    /// ⚠️ At high Q the state-variable filter rings at its corner, and
    /// that ring is heard as a PITCH behind the noise — the exact fault
    /// Brian reported while trying to build this in the playground:
    /// "can hear the wave behind it".
    ///
    /// ⚠️⚠️ MEASURED BY AUTOCORRELATION AT THE CORNER PERIOD, and the
    /// first version of this test did NOT. It compared high-band energy
    /// against total, which sounds like it should detect a ring and does
    /// not: that ratio tracks the CORNER and is flat across Q (0.0324 at
    /// Q 0.5, 0.0327 at Q 6.0). It would have passed at Q 6.0 — the very
    /// bug it claimed to guard. Ringing is PERIODICITY, so the measure
    /// has to be periodicity: 0.018 flat through Q 1.0, then 0.19 at
    /// Q 2.0 and 0.58 at Q 6.0.
    #[test]
    fn thrust_is_a_wall_not_a_tone() {
        let mut t = Thrust::new();
        let s = render_held(&mut t, VoiceParams::thrust(1.0), 1.0);
        // Skip the filter's settling transient.
        let s = &s[4096..];

        let mean = s.iter().sum::<f32>() / s.len() as f32;
        let d: Vec<f32> = s.iter().map(|x| x - mean).collect();

        // A filter ringing at its corner repeats at that period. Sweep
        // lags around it rather than assuming one exactly, because the
        // wobble moves the corner while this renders.
        let period = SR / THRUST_CORNER_FULL;
        let mut best = 0.0f32;
        for lag in (period * 0.6) as usize..(period * 1.8) as usize {
            let mut num = 0.0f32;
            let mut den = 0.0f32;
            // Every 7th sample: this is O(n * lags) and the estimate is
            // identical to three decimals at full density.
            let mut i = 0;
            while i + lag < d.len() {
                num += d[i] * d[i + lag];
                den += d[i] * d[i];
                i += 7;
            }
            if den > 0.0 {
                best = best.max(num / den);
            }
        }
        assert!(
            best < 0.10,
            "thrust RINGS — autocorrelation {best:.4} at the corner period. \
             THRUST_Q is {THRUST_Q}; anything from about 2.0 up reads as a \
             pitch behind the noise."
        );
    }

    /// The engine shares a mixer with five one-shots and must not clip.
    #[test]
    fn thrust_does_not_clip() {
        let mut t = Thrust::new();
        let p = peak(&render_held(&mut t, VoiceParams::thrust(1.0), 2.0));
        assert!(p <= 1.0, "thrust CLIPS at full exhaust: peak {p:.3}");
        assert!(p < 0.95, "thrust has no headroom: peak {p:.3}");
    }

    /// ★ SILENT AT REST. `exhaust` eases to zero asymptotically, so
    /// without the gate the engine whispers forever.
    #[test]
    fn thrust_is_silent_at_rest() {
        let mut t = Thrust::new();
        let s = render_held(&mut t, VoiceParams::thrust(0.0), 0.2);
        assert!(
            peak(&s) == 0.0,
            "thrust is audible at zero exhaust: peak {:.6}",
            peak(&s)
        );
    }

    /// No NaN reaches the mixer. ⚠️ One NaN in one voice is spread across
    /// every other voice by the summing mixer, so this is not a local
    /// failure.
    #[test]
    fn thrust_never_emits_nan() {
        for exhaust in [0.0, 0.001, 0.5, 1.0] {
            let mut t = Thrust::new();
            for s in render_held(&mut t, VoiceParams::thrust(exhaust), 1.0) {
                assert!(s.is_finite(), "thrust emitted {s} at exhaust {exhaust}");
            }
        }
    }
}
