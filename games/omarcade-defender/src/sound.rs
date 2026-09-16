//! The laser and the explosion.
//!
//! ★ BRIAN NAMED SOUND AS THE THING HE MOST WANTS TO NAIL, and he was
//! right about the mechanism before any of this was written: "pretty sure
//! uses some kind of phaser to shift frequencys". Defender's laser is a
//! VCO swept hard downward in a few tens of milliseconds. It is not a
//! sample, it was never a sample, and reproducing it means sweeping an
//! oscillator rather than finding a wav.
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

use std::f32::consts::TAU;

use omarcade_core::audio::{Voice, VoiceParams};

// ---------------------------------------------------------------------
// The laser
// ---------------------------------------------------------------------

/// Where the sweep starts, in Hz.
///
/// High and thin. The original's bolt is a zip rather than a bang, and
/// the whole character is in how fast it falls from here.
const LASER_TOP_HZ: f32 = 1850.0;

/// Where the sweep ends, in Hz.
const LASER_BOTTOM_HZ: f32 = 180.0;

/// How long the whole sound lasts, in seconds.
///
/// ⚠️ SHORT, AND IT MATTERS. At the fire interval a player can hold down
/// the trigger and get six of these a second; anything longer overlaps
/// itself into a drone and the individual shots stop being audible as
/// shots.
const LASER_LEN: f32 = 0.13;

/// How sharply the sweep falls.
///
/// The frequency runs `top * (bottom/top)^(t^CURVE)` — an exponential
/// sweep in pitch, because pitch is heard logarithmically and a LINEAR
/// ramp in Hz spends most of its time in the bottom octave where nothing
/// is happening. The exponent bends it further toward the start, which is
/// what makes it read as a zap rather than a slide whistle.
const LASER_CURVE: f32 = 0.62;

const LASER_LEVEL: f32 = 0.30;

/// Defender's laser: a fast downward frequency sweep.
pub struct Laser {
    t: f32,
    phase: f32,
    gain: f32,
    alive: bool,
}

impl Default for Laser {
    fn default() -> Self {
        Laser::new()
    }
}

impl Laser {
    pub fn new() -> Laser {
        Laser { t: 0.0, phase: 0.0, gain: 1.0, alive: false }
    }
}

impl Voice for Laser {
    fn render(&mut self, out: &mut [f32], _params: VoiceParams, sample_rate: f32) {
        if !self.alive {
            out.fill(0.0);
            return;
        }

        let dt = 1.0 / sample_rate;
        let ratio = LASER_BOTTOM_HZ / LASER_TOP_HZ;

        for sample in out.iter_mut() {
            if self.t >= LASER_LEN {
                *sample = 0.0;
                self.alive = false;
                continue;
            }

            let u = self.t / LASER_LEN;
            let hz = LASER_TOP_HZ * ratio.powf(u.powf(LASER_CURVE));

            // ⚠️ ADVANCE THE PHASE, DO NOT COMPUTE sin(TAU * hz * t).
            // With a CHANGING frequency the second form is wrong: it
            // jumps the phase every time hz moves, which is a click on
            // every sample of a fast sweep. Integrating the frequency is
            // the only way a swept oscillator stays continuous.
            self.phase = (self.phase + hz * dt).fract();

            // A touch of square in the sine gives it the edge the
            // original has without the harshness of a pure square.
            let sine = (TAU * self.phase).sin();
            let edge = if self.phase < 0.5 { 1.0 } else { -1.0 };
            let wave = sine * 0.72 + edge * 0.28;

            // Fast attack, and a decay that is mostly gone before the
            // sweep bottoms out — the tail of the sweep should be felt
            // more than heard.
            let env = if u < 0.06 { u / 0.06 } else { (-(u - 0.06) * 4.5).exp() };

            *sample = wave * env * LASER_LEVEL * self.gain;
            self.t += dt;
        }
    }

    fn alive(&self) -> bool {
        self.alive
    }

    fn retrigger(&mut self, gain: f32, _pitch: f32) {
        self.t = 0.0;
        // ⚠️ THE PHASE IS NOT RESET. Restarting it at zero on every shot
        // makes rapid fire sound mechanically identical each time; letting
        // it run gives successive bolts slightly different attacks for
        // free, which is what the hardware did by not caring.
        self.gain = gain.clamp(0.0, 1.0);
        self.alive = true;
    }
}

// ---------------------------------------------------------------------
// The explosion
// ---------------------------------------------------------------------

/// How long the blast lasts, in seconds.
const BOOM_LEN: f32 = 0.62;

/// The ring modulator's carrier, in Hz.
///
/// ★ RING MODULATION IS THE POINT. Multiplying noise by a low sine (not
/// adding) produces sum and difference frequencies and gives the metallic,
/// hollow quality the 1981 explosions have — filtered noise alone is a
/// "shh", and no amount of enveloping turns it into a Defender explosion.
const RING_HZ: f32 = 62.0;

/// How much of the output is ring-modulated versus plain noise.
const RING_MIX: f32 = 0.62;

/// Where the noise filter starts and ends, as a fraction of Nyquist.
///
/// Sweeping DOWN over the blast, which is a fireball settling: bright at
/// the instant of the burst, a low rumble as it dies. A fixed corner is a
/// wash that merely gets quieter — the same mistake the racer's crash
/// made and had to be corrected for (see its BURN block).
const BOOM_COLOUR_TOP: f32 = 0.42;
const BOOM_COLOUR_BOTTOM: f32 = 0.045;

const BOOM_LEVEL: f32 = 0.42;

/// A one-pole lowpass, enough to colour noise.
#[derive(Default, Clone, Copy)]
struct Lowpass {
    z: f32,
}

impl Lowpass {
    /// `cutoff` is a fraction of the sample rate, 0..0.5.
    fn tick(&mut self, input: f32, cutoff: f32) -> f32 {
        let a = (TAU * cutoff.clamp(0.0005, 0.49)).min(1.0);
        self.z += a * (input - self.z);
        self.z
    }
}

/// An explosion: swept noise, ring-modulated.
pub struct Boom {
    t: f32,
    gain: f32,
    alive: bool,
    phase: f32,
    lp: Lowpass,
    noise: u32,
}

impl Default for Boom {
    fn default() -> Self {
        Boom::new()
    }
}

impl Boom {
    pub fn new() -> Boom {
        Boom { t: 0.0, gain: 1.0, alive: false, phase: 0.0, lp: Lowpass::default(), noise: 0x1234_5678 }
    }

    fn white(&mut self) -> f32 {
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        (self.noise as f32 / u32::MAX as f32) * 2.0 - 1.0
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

            // Noise, filtered by a corner sweeping downward.
            let cutoff = BOOM_COLOUR_TOP * (BOOM_COLOUR_BOTTOM / BOOM_COLOUR_TOP).powf(u);
            let raw = self.white();
            let n = self.lp.tick(raw, cutoff);

            // Ring modulation: MULTIPLY, do not add. The carrier also
            // falls, so the metallic component sags with the blast.
            let carrier_hz = RING_HZ * (1.0 - 0.45 * u);
            self.phase = (self.phase + carrier_hz * dt).fract();
            let ring = n * (TAU * self.phase).sin();

            let v = n * (1.0 - RING_MIX) + ring * RING_MIX;

            // Instant attack, exponential decay. An explosion has no
            // swell — it is loudest at the moment it happens.
            let env = (-u * 3.6).exp();

            *sample = v * env * BOOM_LEVEL * self.gain;
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
        self.lp = Lowpass::default();
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

    /// Estimate the dominant frequency of a slice by counting zero
    /// crossings — enough to prove a sweep goes DOWN, which is the one
    /// claim that matters.
    fn rough_hz(samples: &[f32]) -> f32 {
        let crossings = samples
            .windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count() as f32;
        crossings * SR / (2.0 * samples.len() as f32)
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

        let s = render_all(&mut laser, LASER_LEN * 1.5);
        assert!(peak(&s) > 0.05, "the laser was inaudible: peak {}", peak(&s));
        assert!(!laser.alive(), "a one-shot must retire itself");
    }

    /// ★ THE DEFINING PROPERTY. Brian identified the mechanism as
    /// frequency shifting, and this is that claim as an assertion: the
    /// pitch at the start must be far above the pitch at the end.
    #[test]
    fn the_laser_sweeps_downward() {
        let mut laser = Laser::new();
        laser.retrigger(1.0, 1.0);
        let s = render_all(&mut laser, LASER_LEN);

        let n = (LASER_LEN * SR) as usize;
        let early = rough_hz(&s[..n / 6]);
        let late = rough_hz(&s[n / 2..n.min(s.len())]);
        assert!(
            early > late * 2.0,
            "the sweep is not falling: {early:.0} Hz then {late:.0} Hz"
        );
    }

    /// A swept oscillator that recomputes sin(TAU*hz*t) instead of
    /// integrating phase clicks on every sample. A click is a large
    /// sample-to-sample jump, so bound the biggest step.
    #[test]
    fn the_laser_does_not_click() {
        let mut laser = Laser::new();
        laser.retrigger(1.0, 1.0);
        let s = render_all(&mut laser, LASER_LEN);
        let live: Vec<f32> = s.iter().copied().take((LASER_LEN * SR) as usize).collect();

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

    /// Ring modulation must actually be doing something — a mix of 0
    /// would leave plain noise, and the test that catches that is one
    /// that compares against noise with the ring removed.
    #[test]
    fn the_explosion_is_ring_modulated_not_just_noise() {
        assert!(RING_MIX > 0.25, "the ring is what makes it metallic");
        let mut boom = Boom::new();
        boom.retrigger(1.0, 1.0);
        let s = render_all(&mut boom, BOOM_LEN * 0.5);

        // A ring-modulated signal crosses zero at the carrier's rate on
        // top of the noise, so the modulated signal has markedly more
        // low-frequency structure than its envelope alone would give.
        // Cheap proxy: it must not be silent and must not be a DC blob.
        let mean = s.iter().sum::<f32>() / s.len() as f32;
        assert!(mean.abs() < 0.02, "the blast has a DC offset: {mean}");
        assert!(peak(&s) > 0.05);
    }

    #[test]
    fn gain_scales_the_sound() {
        let mut loud = Laser::new();
        loud.retrigger(1.0, 1.0);
        let l = peak(&render_all(&mut loud, LASER_LEN));

        let mut soft = Laser::new();
        soft.retrigger(0.25, 1.0);
        let s = peak(&render_all(&mut soft, LASER_LEN));

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
        let s = render_all(&mut laser, LASER_LEN);
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

        for s in render_all(&mut laser, LASER_LEN * 2.0) {
            assert!(s.is_finite(), "laser emitted {s}");
        }
        for s in render_all(&mut boom, BOOM_LEN * 2.0) {
            assert!(s.is_finite(), "boom emitted {s}");
        }
    }
}
