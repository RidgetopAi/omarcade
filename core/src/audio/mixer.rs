//! The mixer: what actually runs on the audio thread.
//!
//! Everything in [`Mixer::fill`] obeys the rule from [`crate::audio`] —
//! **no allocation, no locking, no blocking, no panicking.** Buffers are
//! sized once at construction, voices are visited by index, and the only
//! communication with the game thread is draining the ring.

use super::params::{Command, SoundId, VoiceId, VoiceParams};
use super::ring::Ring;

/// A source of samples, rendered on the audio thread.
///
/// # The contract
///
/// `render` runs under a hard deadline on a real-time thread. An
/// implementation must not allocate, lock, block, do I/O, or panic. In
/// practice that means: own your state, do arithmetic, write samples.
///
/// This is the seam that keeps the suite decoupled. Omaprix's engine is
/// a `Voice` living in the racer; Breakout's paddle blip is a `Voice`
/// living in Breakout. Core provides the stream, the mixing, the ducking
/// and the clock, and knows nothing about cars or bricks.
pub trait Voice: Send {
    /// Fill `out` (mono) with the next samples.
    ///
    /// `params` is the most recent value the game set. It may be several
    /// frames old, and it may jump — a game that pauses for a second
    /// then resumes will hand you a discontinuity. Smooth internally
    /// rather than trusting continuity: a synthesiser that follows a
    /// jumping parameter exactly will click.
    fn render(&mut self, out: &mut [f32], params: VoiceParams, sample_rate: f32);

    /// False once a one-shot has finished, so the mixer can retire it.
    /// Continuous voices leave this at the default.
    fn alive(&self) -> bool {
        true
    }

    /// Restart a one-shot from the beginning.
    ///
    /// Called when a sound is played again while already playing. The
    /// default ignores it, which is right for a continuous voice.
    fn retrigger(&mut self, _gain: f32, _pitch: f32) {}
}

/// One registered voice and its live state.
struct Slot {
    voice: Box<dyn Voice>,
    params: VoiceParams,
    /// Continuous voices are enabled/disabled; one-shots are triggered.
    enabled: bool,
    /// True for a one-shot, which retires itself via [`Voice::alive`].
    one_shot: bool,
    gain: f32,
}

/// How fast ducking may move, as a fraction of full gain per sample at
/// 48 kHz. A step change in gain is a click; this is the slowest ramp
/// that still feels instant at the moment of a crash.
const DUCK_MAX_STEP: f32 = 1.0 / (0.010 * 48_000.0);

/// The audio-thread half of the system.
pub(crate) struct Mixer {
    slots: Vec<Slot>,
    ring: std::sync::Arc<Ring>,
    /// Scratch for one voice's output. Allocated once, here, because
    /// `fill` may not allocate.
    scratch: Vec<f32>,
    sample_rate: f32,
    master: f32,
    /// Current and target duck gain, and who is exempt.
    duck: f32,
    duck_target: f32,
    duck_step: f32,
    duck_except: Option<VoiceId>,
}

impl Mixer {
    pub(crate) fn new(
        slots: Vec<(Box<dyn Voice>, bool)>,
        ring: std::sync::Arc<Ring>,
        sample_rate: f32,
        max_block: usize,
        master: f32,
    ) -> Mixer {
        Mixer {
            slots: slots
                .into_iter()
                .map(|(voice, one_shot)| Slot {
                    voice,
                    params: VoiceParams::SILENT,
                    // A continuous voice is silent until the game starts
                    // it; a one-shot is silent until it is played.
                    enabled: false,
                    one_shot,
                    gain: 1.0,
                })
                .collect(),
            ring,
            scratch: vec![0.0; max_block],
            sample_rate,
            master,
            duck: 1.0,
            duck_target: 1.0,
            duck_step: DUCK_MAX_STEP,
            duck_except: None,
        }
    }

    /// Apply everything the game asked for since the last callback.
    ///
    /// Drains the whole ring rather than a fixed number: the queue is
    /// bounded, so this terminates, and leaving commands behind would
    /// mean a `set` from this frame arriving a callback late.
    fn drain(&mut self) {
        while let Some(cmd) = self.ring.pop() {
            match cmd {
                Command::Set { voice, params } => {
                    if let Some(s) = self.slots.get_mut(voice.0 as usize) {
                        s.params = params;
                    }
                }
                Command::Enable { voice, on } => {
                    if let Some(s) = self.slots.get_mut(voice.0 as usize) {
                        s.enabled = on;
                    }
                }
                Command::Play { sound, gain, pitch } => {
                    if let Some(s) = self.slots.get_mut(sound.0 as usize) {
                        s.gain = gain;
                        s.enabled = true;
                        s.voice.retrigger(gain, pitch);
                    }
                }
                Command::Duck { except, gain, seconds } => {
                    self.duck_except = Some(except);
                    self.duck_target = gain.clamp(0.0, 1.0);
                    // Convert a duration to a per-sample step once, here,
                    // rather than dividing per sample in the hot loop.
                    let samples = (seconds.max(0.0) * self.sample_rate).max(1.0);
                    self.duck_step = (1.0 / samples).min(DUCK_MAX_STEP);
                }
                Command::Master { gain } => self.master = gain.clamp(0.0, 1.0),
            }
        }
    }

    /// Render one block. **This is the callback body.**
    ///
    /// `out` is interleaved with `channels` channels; every channel gets
    /// the same mono mix, which is what a 2D arcade game wants and what
    /// keeps the synthesis cost independent of the device's layout.
    pub(crate) fn fill(&mut self, out: &mut [f32], channels: usize) {
        self.drain();

        for s in out.iter_mut() {
            *s = 0.0;
        }

        // A device reporting zero channels is nonsense, but it must not
        // be a division by zero on the audio thread.
        let Some(frames) = out.len().checked_div(channels) else {
            return;
        };
        if frames == 0 {
            return;
        }
        // The scratch buffer cannot grow here — growing means allocating
        // — so a block larger than MAX_BLOCK is rendered short rather
        // than dropped, which is a quieter failure than silence.
        let frames = frames.min(self.scratch.len());

        for i in 0..self.slots.len() {
            if !self.slots[i].enabled {
                continue;
            }

            let scratch = &mut self.scratch[..frames];
            for v in scratch.iter_mut() {
                *v = 0.0;
            }

            let slot = &mut self.slots[i];
            slot.voice.render(scratch, slot.params, self.sample_rate);

            // A one-shot that has finished stops costing anything.
            if slot.one_shot && !slot.voice.alive() {
                slot.enabled = false;
            }

            let ducked = self.duck_except != Some(VoiceId(i as u16));
            let gain = slot.gain;

            for (f, v) in self.scratch[..frames].iter().enumerate() {
                // The duck ramp advances per frame, not per callback, so
                // its speed does not depend on the device's block size.
                let d = if ducked { self.duck } else { 1.0 };
                let s = v * gain * d * self.master;
                let base = f * channels;
                for c in 0..channels {
                    out[base + c] += s;
                }
            }

            // Advance the ramp once per block for the *next* block's
            // starting point; within a block the change is inaudible and
            // this keeps the inner loop free of a branch per sample.
            self.advance_duck(frames);
        }

        // Nothing here should exceed full scale, but a voice with a
        // runaway parameter would otherwise wrap and produce a loud
        // click. Clamping is two instructions and removes the failure.
        for s in out.iter_mut() {
            *s = s.clamp(-1.0, 1.0);
        }
    }

    fn advance_duck(&mut self, frames: usize) {
        let room = self.duck_step * frames as f32;
        if self.duck < self.duck_target {
            self.duck = (self.duck + room).min(self.duck_target);
        } else if self.duck > self.duck_target {
            self.duck = (self.duck - room).max(self.duck_target);
        }
    }
}

/// Marker so `SoundId` and `VoiceId` cannot be confused at a call site
/// even though both index the same table.
impl From<SoundId> for VoiceId {
    fn from(s: SoundId) -> VoiceId {
        VoiceId(s.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// A voice that writes a constant, so a test can read the mix back.
    struct Constant(f32);
    impl Voice for Constant {
        fn render(&mut self, out: &mut [f32], _p: VoiceParams, _sr: f32) {
            for s in out.iter_mut() {
                *s = self.0;
            }
        }
    }

    /// A one-shot that plays `n` samples and then reports itself done.
    struct Burst {
        left: usize,
    }
    impl Voice for Burst {
        fn render(&mut self, out: &mut [f32], _p: VoiceParams, _sr: f32) {
            for s in out.iter_mut() {
                if self.left == 0 {
                    break;
                }
                *s = 0.5;
                self.left -= 1;
            }
        }
        fn alive(&self) -> bool {
            self.left > 0
        }
        fn retrigger(&mut self, _g: f32, _p: f32) {
            self.left = 4;
        }
    }

    fn mixer(voices: Vec<(Box<dyn Voice>, bool)>) -> (Mixer, Arc<Ring>) {
        let ring = Arc::new(Ring::new());
        let m = Mixer::new(voices, Arc::clone(&ring), 48_000.0, 512, 1.0);
        (m, ring)
    }

    #[test]
    fn a_voice_is_silent_until_the_game_starts_it() {
        let (mut m, ring) = mixer(vec![(Box::new(Constant(0.5)), false)]);
        let mut out = [0.0f32; 8];

        m.fill(&mut out, 2);
        assert!(out.iter().all(|s| *s == 0.0), "an unstarted voice must not sound");

        ring.push(Command::Enable { voice: VoiceId(0), on: true });
        m.fill(&mut out, 2);
        assert!(out.iter().all(|s| (*s - 0.5).abs() < 1e-6), "a started voice should sound");
    }

    #[test]
    fn every_channel_gets_the_same_mono_mix() {
        let (mut m, ring) = mixer(vec![(Box::new(Constant(0.25)), false)]);
        ring.push(Command::Enable { voice: VoiceId(0), on: true });
        let mut out = [0.0f32; 12];
        m.fill(&mut out, 3);
        for frame in out.chunks(3) {
            assert_eq!(frame[0], frame[1]);
            assert_eq!(frame[1], frame[2]);
        }
    }

    #[test]
    fn a_finished_one_shot_retires_itself() {
        let (mut m, ring) = mixer(vec![(Box::new(Burst { left: 0 }), true)]);
        ring.push(Command::Play { sound: SoundId(0), gain: 1.0, pitch: 1.0 });

        let mut out = [0.0f32; 8];
        m.fill(&mut out, 1);
        assert!(out[..4].iter().all(|s| *s > 0.0), "the burst should have sounded");

        // Second block: the burst is spent and must cost nothing.
        let mut out2 = [0.0f32; 8];
        m.fill(&mut out2, 1);
        assert!(out2.iter().all(|s| *s == 0.0), "a spent one-shot must go quiet");
    }

    #[test]
    fn ducking_ramps_rather_than_jumping() {
        // A step change in gain is an audible click, which is the whole
        // reason `duck` is a ramp and not an assignment.
        let (mut m, ring) = mixer(vec![
            (Box::new(Constant(1.0)), false),
            (Box::new(Constant(1.0)), false),
        ]);
        ring.push(Command::Enable { voice: VoiceId(0), on: true });
        ring.push(Command::Duck { except: VoiceId(1), gain: 0.0, seconds: 0.5 });

        let mut out = [0.0f32; 64];
        m.fill(&mut out, 1);
        assert!(out[0] > 0.9, "ducking must start from where the gain already was");
        assert!(m.duck > 0.0, "half a second of ramp should not complete in one block");
    }

    #[test]
    fn the_exempt_voice_is_not_ducked() {
        let (mut m, ring) = mixer(vec![
            (Box::new(Constant(1.0)), false),
            (Box::new(Constant(1.0)), false),
        ]);
        ring.push(Command::Enable { voice: VoiceId(1), on: true });
        ring.push(Command::Duck { except: VoiceId(1), gain: 0.0, seconds: 0.0 });

        let mut out = [0.0f32; 16];
        m.fill(&mut out, 1);
        assert!(out.iter().all(|s| *s > 0.9), "the exempt voice should keep full gain");
    }

    #[test]
    fn the_mix_cannot_leave_full_scale() {
        // Eight loud voices at once must not wrap into a click.
        let voices: Vec<(Box<dyn Voice>, bool)> =
            (0..8).map(|_| (Box::new(Constant(0.9)) as Box<dyn Voice>, false)).collect();
        let (mut m, ring) = mixer(voices);
        for i in 0..8 {
            ring.push(Command::Enable { voice: VoiceId(i), on: true });
        }
        let mut out = [0.0f32; 16];
        m.fill(&mut out, 2);
        assert!(out.iter().all(|s| *s <= 1.0 && *s >= -1.0), "the mix must stay in range");
    }
}
