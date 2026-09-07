//! Omaprix's engine, as a real-time voice.
//!
//! # Where these numbers came from
//!
//! Brian tuned them by ear in `tools/sfx/playground.html` on 2026-09-07,
//! against two reference recordings he chose. They are not derived and
//! they are not defaults: where a measured spectrum and his ear
//! disagreed, his ear won. The full reasoning — including two things the
//! measurements got wrong — is in the engine-voice record; the short
//! version is in the comments on each constant.
//!
//! # Why this is synthesised and not a sample
//!
//! The engine has to track [`Drive::speed`](crate::drive::Drive) *and
//! change character as it climbs*: the 1-4 kHz band nearly doubles from
//! idle to redline, which is the snarl arriving under load. A recording
//! pitch-shifted to the right note does not do that — it sounds like a
//! recording being played faster, which is the classic cheap-game-engine
//! giveaway. Synthesis is what makes the rev *mean* something.
//!
//! # What an engine actually is
//!
//! Not a waveform: a series of explosions. What the ear reads as "V8" is
//! the RATE and the EVENNESS of combustion, not the shape of any wave.
//! So this synthesises firing events and lets the timbre fall out of
//! their spacing — which is also why the lope below is a real thing and
//! not an effect bolted on afterwards.
//!
//! ⚠️ [`Voice::render`] runs on the audio thread: no allocation, no
//! locking, no panicking. Everything here is arithmetic over owned
//! state.

use std::f32::consts::TAU;

use omarcade_core::{Voice, VoiceParams};

// ─────────────────────────────────────────────────────────────────
// BRIAN'S TUNING. Change these by ear, in the playground, not here.
// ─────────────────────────────────────────────────────────────────

/// Cylinders firing per two crank revolutions. Eight is the stock car.
const CYLINDERS: f32 = 8.0;

/// The rev range, which is the "too high / too whiny" axis.
///
/// Brian's note that set these: *"too whiny sounds like wound tight,
/// feels like frequency is too high."* Redline had been 6800 rpm — 453
/// Hz of firing — against a reference that pulled to 307 Hz. A street V8
/// does not turn 6800 anyway.
const IDLE_RPM: f32 = 800.0;
const REDLINE_RPM: f32 = 5650.0;

/// How throttle maps to revs. Above 1.0 the revs hang low and climb
/// late, which is what a torquey engine feels like. A feel knob, not a
/// pitch knob.
const REV_CURVE: f32 = 1.15;

/// The relative amplitude of each harmonic of the firing rate.
///
/// This is the "multiple frequencies mixed" idea made literal, and it
/// was Brian's insight: a stack that falls off a cliff after the second
/// harmonic is exactly what "whiny" and "wound tight" sound like. He set
/// h2-h5 some 39% above anything derived from the reference spectra, and
/// that is what gives the engine its weight.
const HARMONICS: [f32; 12] =
    [1.00, 0.73, 0.59, 0.52, 0.50, 0.22, 0.17, 0.20, 0.08, 0.09, 0.08, 0.07];

/// How uneven the firing is — the burble.
///
/// A cross-plane V8 fires unevenly, and that irregularity is the lope.
/// 0.0 is a perfectly even engine (flat-plane, exotic, buzzy). Brian set
/// 0.375, more than three times the derived value: he wanted the burble
/// front and centre rather than tastefully implied.
const LOPE: f32 = 0.375;

/// How much of the lope is gone by redline. A real V8 lopes at idle and
/// cleans up under load, so the burble belongs at the bottom.
const LOPE_CLEANUP: f32 = 0.6;

/// Pulse decay, **as a fraction of the gap between firings**.
///
/// ⚠️ A FRACTION, NOT A DURATION, and that is load-bearing. The firing
/// gap shrinks sevenfold from idle to redline; stated in milliseconds, a
/// decay that sits neatly inside its slot at idle is longer than the
/// whole slot at redline, the pulses smear into each other, and the
/// harmonic structure collapses into broadband noise. Measured, that
/// pinned the spectral centroid at ~6900 Hz and crushed the second
/// harmonic to 0.13 whatever this number was — the knob looked broken
/// because it was the wrong KIND of knob.
const SHARPNESS: f32 = 0.295;

/// A sine an octave below the firing rate, for chest.
const GROWL: f32 = 0.36;

/// The body resonance: the block and exhaust ringing.
const BODY_HZ: f32 = 158.0;
const BODY_Q: f32 = 3.8;

/// The midrange snarl.
///
/// ⚠️ IT DOES NOT SWEEP WITH THE REVS, AND THAT WAS BRIAN'S CALL. It was
/// built to rise 700 → 1800 Hz, reasoning from the measurement that a
/// real pull doubles its 1-4 kHz energy. He set idle and top 40 Hz
/// apart — a fixed formant, parked high.
///
/// The measurement was right and the inference was wrong: that band
/// opens up because the harmonics climb THROUGH a fixed resonance, not
/// because the resonance moves. A sweeping filter tracks the note and so
/// cancels the very effect it was meant to create. A car has one
/// exhaust; its resonances do not retune. Do not "fix" this back.
const SNARL_IDLE_HZ: f32 = 1480.0;
const SNARL_TOP_HZ: f32 = 1440.0;
const SNARL_MIX: f32 = 0.24;

/// Induction noise: the air the engine swallows.
const INTAKE_LEVEL: f32 = 0.16;
const INTAKE_HZ: f32 = 1700.0;

/// Top-end rolloff, so the attack does not become hiss.
const ROLLOFF_HZ: f32 = 5600.0;

/// Output level, leaving headroom for the other six sounds to sit on top.
const LEVEL: f32 = 0.34;

/// How fast the throttle may move, per second.
///
/// The game sets a new throttle 60 times a second; the audio thread runs
/// 48,000 times a second. Following that staircase exactly would step
/// the pitch 60 times a second, and a stepped frequency is a buzz. So
/// the voice smooths internally — which is also what [`Voice::render`]
/// warns every implementation to do.
const THROTTLE_SLEW: f32 = 3.5;

/// A one-pole resonator: two poles of feedback, no allocation, no state
/// beyond two samples. The block and exhaust, and the snarl.
#[derive(Default)]
struct Resonator {
    y1: f32,
    y2: f32,
}

impl Resonator {
    fn tick(&mut self, x: f32, hz: f32, q: f32, sample_rate: f32) -> f32 {
        let w = TAU * hz / sample_rate;
        let r = (-w / (2.0 * q)).exp();
        let a1 = 2.0 * r * w.cos();
        let a2 = -r * r;
        let y = x + a1 * self.y1 + a2 * self.y2;
        self.y2 = self.y1;
        self.y1 = y;
        y * (1.0 - r)
    }
}

/// A one-pole lowpass, for the intake colour and the top-end rolloff.
#[derive(Default)]
struct Lowpass {
    y: f32,
}

impl Lowpass {
    fn tick(&mut self, x: f32, hz: f32, sample_rate: f32) -> f32 {
        let a = 1.0 - (-TAU * hz / sample_rate).exp();
        self.y += a * (x - self.y);
        self.y
    }
}

/// The engine.
pub struct Engine {
    /// Position within the current firing, 0..1.
    phase: f32,
    /// Seconds since the last firing, for the pulse envelope.
    since: f32,
    /// Alternating, so every other firing can be late and light.
    parity: u32,
    /// The smoothed throttle actually being sounded.
    throttle: f32,
    body: Resonator,
    snarl: Resonator,
    intake: Lowpass,
    rolloff: Lowpass,
    /// A tiny xorshift, because the audio thread may not call `rand`
    /// and combustion is not a clean click.
    noise: u32,
    /// Firings since construction.
    ///
    /// Kept because it is the only honest way to check the rate this
    /// voice produces. A spectrum will not tell you: at Brian's
    /// LOPE = 0.375 the alternate-pulse pattern repeats every TWO
    /// firings, so the strongest partial sits at HALF the firing rate
    /// and every peak-picking analysis reads an octave low. That is the
    /// burble working, not a bug — but it cost a diagnostic detour once,
    /// and this counter is what settled it.
    pub firings: u64,
}

impl Engine {
    pub fn new() -> Engine {
        Engine {
            phase: 0.0,
            since: 0.0,
            parity: 0,
            throttle: 0.0,
            body: Resonator::default(),
            snarl: Resonator::default(),
            intake: Lowpass::default(),
            rolloff: Lowpass::default(),
            noise: 0x2545_f491,
            firings: 0,
        }
    }

    /// Engine speed at a throttle position.
    fn rpm(throttle: f32) -> f32 {
        IDLE_RPM + (REDLINE_RPM - IDLE_RPM) * throttle.clamp(0.0, 1.0).powf(REV_CURVE)
    }

    /// Combustion events per second — the pitch you actually hear.
    ///
    /// A four-stroke fires each cylinder once per TWO crank revolutions,
    /// hence the halving. At idle on a V8 that is 53 Hz, which is why an
    /// idling V8 is felt as much as heard.
    pub fn firing_hz(throttle: f32) -> f32 {
        Engine::rpm(throttle) * CYLINDERS / 2.0 / 60.0
    }

    fn white(&mut self) -> f32 {
        // xorshift32: deterministic, allocation-free, and fine for noise.
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        (self.noise as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

impl Default for Engine {
    fn default() -> Self {
        Engine::new()
    }
}

impl Voice for Engine {
    fn render(&mut self, out: &mut [f32], params: VoiceParams, sample_rate: f32) {
        let target = params.get(0).clamp(0.0, 1.0);
        let dt = 1.0 / sample_rate;

        for sample in out.iter_mut() {
            // Smooth the throttle: see THROTTLE_SLEW.
            let slew = THROTTLE_SLEW * dt;
            self.throttle += (target - self.throttle).clamp(-slew, slew);

            let hz = Engine::firing_hz(self.throttle);
            let gap = 1.0 / hz;
            let lope = LOPE * (1.0 - LOPE_CLEANUP * self.throttle);

            // Uneven firing slots: alternate firings sit late. Shifting
            // by a FRACTION of the gap keeps the character identical at
            // idle and at redline; a fixed offset would vanish at high
            // revs, exactly where its absence is most obvious.
            let slot = if self.parity == 1 { 1.0 + lope } else { 1.0 - lope };
            self.phase += dt / (gap * slot);
            self.since += dt;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
                self.parity ^= 1;
                self.since = 0.0;
                self.firings += 1;
            }

            // One decaying crack, with the harmonic stack inside it.
            let decay = gap * SHARPNESS;
            let env = (-self.since / decay).exp();
            let mut voiced = 0.0;
            for (k, amp) in HARMONICS.iter().enumerate() {
                let f = hz * (k + 1) as f32;
                if f > sample_rate * 0.5 {
                    break; // never write above Nyquist
                }
                voiced += amp * (TAU * f * self.since).sin();
            }
            let n = self.white();
            let pulse = env * (voiced * 0.25 + 0.30 * n);

            // Alternate firings are also LIGHTER, not merely later. An
            // uneven firing order scavenges unevenly, and that weight
            // difference is most of what makes a burble audible rather
            // than merely measurable.
            let weight = if self.parity == 1 { 1.0 - lope * 1.2 } else { 1.0 };
            let pulse = pulse * weight;

            let body = self.body.tick(pulse, BODY_HZ, BODY_Q, sample_rate);
            let snarl_hz = SNARL_IDLE_HZ + (SNARL_TOP_HZ - SNARL_IDLE_HZ) * self.throttle;
            let snarl = self.snarl.tick(pulse, snarl_hz, 1.6, sample_rate);

            let growl = GROWL * (TAU * hz * 0.5 * self.since).sin() * env;

            let air = self.intake.tick(n, INTAKE_HZ, sample_rate)
                * INTAKE_LEVEL
                * (0.35 + 0.65 * self.throttle);

            let mix = body * (1.0 - SNARL_MIX) + snarl * SNARL_MIX + growl * 0.4 + air;
            *sample = self.rolloff.tick(mix, ROLLOFF_HZ, sample_rate).clamp(-1.0, 1.0) * LEVEL;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(engine: &mut Engine, throttle: f32, n: usize) -> Vec<f32> {
        let mut out = vec![0.0; n];
        engine.render(&mut out, VoiceParams::engine(throttle), 48_000.0);
        out
    }

    #[test]
    fn the_firing_rate_matches_the_reference_pulls() {
        // Brian's reference recording pulled at 117 Hz low and 307 Hz
        // high. If a retune moves the rev range far off that, the engine
        // has stopped being the car he approved.
        assert!((Engine::firing_hz(0.0) - 53.3).abs() < 1.0, "idle");
        assert!((Engine::firing_hz(1.0) - 376.7).abs() < 2.0, "redline");
        let cruise = Engine::firing_hz(0.3);
        assert!((100.0..200.0).contains(&cruise), "cruise was {cruise}");
    }

    #[test]
    fn it_makes_sound_and_stays_inside_full_scale() {
        let mut e = Engine::new();
        // Let the throttle slew reach its target before measuring.
        render(&mut e, 0.7, 48_000);
        let out = render(&mut e, 0.7, 4_800);
        let peak = out.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        assert!(peak > 0.01, "the engine should be audible, peak was {peak}");
        assert!(peak <= 1.0, "the engine must not clip, peak was {peak}");
    }

    #[test]
    fn the_throttle_is_smoothed_rather_than_followed_exactly() {
        // The game steps the throttle 60 times a second; a voice that
        // followed that staircase would step the pitch with it and buzz.
        let mut e = Engine::new();
        render(&mut e, 1.0, 64);
        assert!(e.throttle < 0.5, "one short block should not reach full throttle");
        render(&mut e, 1.0, 48_000);
        assert!(e.throttle > 0.9, "a second of full throttle should arrive");
    }

    #[test]
    fn the_lope_is_real_and_eases_off_under_load() {
        // The burble is uneven firing. This asserts the mechanism rather
        // than the sound: alternate slots differ at idle, and differ
        // much less when floored — which is what a real V8 does.
        let idle = LOPE * (1.0 - LOPE_CLEANUP * 0.0);
        let floored = LOPE * (1.0 - LOPE_CLEANUP * 1.0);
        let ratio = |l: f32| (1.0 + l) / (1.0 - l);
        assert!(ratio(idle) > 2.0, "idle should lope hard: {}", ratio(idle));
        assert!(ratio(floored) < 1.5, "floored should clean up: {}", ratio(floored));
        assert!(ratio(idle) > ratio(floored));
    }

    #[test]
    fn the_voice_fires_at_the_rate_it_claims() {
        // A spectrum cannot check this: at LOPE = 0.375 the half-rate
        // subharmonic is the strongest partial, so peak-picking reads an
        // octave low and looks like a bug. Count the firings instead.
        for (throttle, want) in [(0.0, 53.0), (0.3, 134.0), (0.7, 268.0), (1.0, 377.0)] {
            let mut e = Engine::new();
            let mut buf = vec![0.0; 48_000];
            // Warm up first: the throttle slews rather than jumping (see
            // THROTTLE_SLEW), so a cold voice spends the first fraction
            // of a second on its way to the requested revs and would
            // fire slow through no fault of the synthesis.
            e.render(&mut buf, VoiceParams::engine(throttle), 48_000.0);
            let before = e.firings;
            e.render(&mut buf, VoiceParams::engine(throttle), 48_000.0);
            let got = (e.firings - before) as f32;
            assert!(
                (got - want).abs() <= 3.0,
                "at throttle {throttle} fired {got}/s, wanted about {want}",
            );
        }
    }

    #[test]
    fn nothing_it_produces_is_ever_nan() {
        // A NaN reaching the device is a loud click at best. The
        // resonators feed back, so this is worth asserting rather than
        // assuming.
        let mut e = Engine::new();
        for t in [0.0, 0.01, 0.5, 1.0, 0.0] {
            for s in render(&mut e, t, 9_600) {
                assert!(s.is_finite(), "produced a non-finite sample at throttle {t}");
            }
        }
    }
}
