//! The plain data that crosses from the game thread to the audio thread.
//!
//! # Why everything here is `Copy` and owns nothing
//!
//! These types are read inside the audio callback, which runs on a
//! real-time thread with a deadline of about 5 ms and **may not
//! allocate, lock, or block** (see [`crate::audio`]). A type with a
//! destructor — a `String`, a `Box`, an `Arc` — reaching that thread
//! means a deallocation happens there, on a thread that must never
//! touch the allocator.
//!
//! So nothing in this module has a destructor, and that is a
//! correctness property rather than an optimisation. If a future field
//! wants to be a `String`, it wants to be an id into a table the audio
//! thread already owns instead.

/// A continuous voice, registered at startup.
///
/// Registration is startup-only, so this is always valid: there is no
/// "not registered yet" state to handle, because the stream does not
/// start until every voice exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VoiceId(pub(crate) u16);

impl VoiceId {
    /// A voice that is registered nowhere.
    ///
    /// For tests and for a game constructing its state before it has a
    /// real id. Every command naming it is silently ignored by the
    /// mixer — the slot lookup simply misses — so it is safe to use and
    /// silent by construction rather than by special-casing.
    pub const NONE: VoiceId = VoiceId(u16::MAX);
}

/// A one-shot sound, registered at startup alongside the voices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SoundId(pub(crate) u16);

/// How many parameters one voice carries.
///
/// Four covers every sound in the suite today (the engine uses one, the
/// surface two) with room to spare. A fixed width is what keeps
/// [`VoiceParams`] `Copy` and keeps [`Command`] one size forever.
pub const PARAM_SLOTS: usize = 4;

/// The parameters of a continuous voice, as of the last frame that set
/// them.
///
/// # Why an array and not a struct per sound
///
/// An enum with a variant per sound would read better at the call site,
/// and it would mean [`Command`] grows every time the suite gains a
/// sound — including sounds in *other games*, since this type lives in
/// core. A fixed array keeps the wire format stable: a new sound is a
/// new constructor here and a new [`Voice`](super::Voice) in the game,
/// and nothing that crosses a thread boundary changes shape.
///
/// The constructors are the readable layer. Games call
/// `VoiceParams::engine(throttle)`, never `VoiceParams([x, 0.0, ..])`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoiceParams([f32; PARAM_SLOTS]);

impl VoiceParams {
    /// All slots zero. What a voice sees before its first `set`.
    pub const SILENT: VoiceParams = VoiceParams([0.0; PARAM_SLOTS]);

    /// Build from raw slots. Prefer a named constructor.
    pub const fn new(slots: [f32; PARAM_SLOTS]) -> VoiceParams {
        VoiceParams(slots)
    }

    /// Read one slot. Out of range reads as `0.0` rather than panicking:
    /// a panic on the audio thread aborts the process, and a voice
    /// reading a slot it never set is a bug worth surviving.
    pub fn get(&self, slot: usize) -> f32 {
        match self.0.get(slot) {
            Some(v) => *v,
            None => 0.0,
        }
    }

    /// An engine at `throttle` in 0..=1 — normally
    /// `Drive::speed / Tuning::top_speed`.
    pub fn engine(throttle: f32) -> VoiceParams {
        VoiceParams([throttle.clamp(0.0, 1.0), 0.0, 0.0, 0.0])
    }

    /// Tyre squeal at `lean` in 0..=1 — normally
    /// `Drive::cornering().abs()` — and how fast the tyres are being
    /// dragged over the road.
    ///
    /// Speed is a second parameter rather than folded into `lean`
    /// because the two do different jobs: lean decides whether the tyre
    /// is complaining at all, and speed decides how fast it grips and
    /// releases while it does.
    pub fn squeal(lean: f32, speed: f32) -> VoiceParams {
        VoiceParams([lean.clamp(0.0, 1.0), speed.clamp(0.0, 1.0), 0.0, 0.0])
    }

    /// Surface noise: which surface, and how fast we are crossing it.
    ///
    /// The surface arrives as a number because this crate's audio layer
    /// must not depend on any game's `Surface` type — the same reason
    /// core knows nothing about cars.
    pub fn surface(kind: f32, speed: f32) -> VoiceParams {
        VoiceParams([kind, speed.max(0.0), 0.0, 0.0])
    }
}

impl Default for VoiceParams {
    fn default() -> Self {
        VoiceParams::SILENT
    }
}

/// One instruction from the game thread to the mixer.
///
/// Fixed size, `Copy`, no heap: this is what travels through the ring in
/// [`super::ring`]. See the module docs for why that matters.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Command {
    /// Fire a one-shot, with a gain and a pitch trim.
    Play { sound: SoundId, gain: f32, pitch: f32 },
    /// This frame's parameters for a continuous voice.
    Set { voice: VoiceId, params: VoiceParams },
    /// Start or stop a continuous voice.
    Enable { voice: VoiceId, on: bool },
    /// Duck everything except one voice, over a ramp.
    Duck { except: VoiceId, gain: f32, seconds: f32 },
    /// Master gain, 0..=1.
    Master { gain: f32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_are_plain_data() {
        // The property this module exists to guarantee: nothing here has
        // a destructor, so nothing deallocates on the audio thread.
        // `needs_drop` is the compiler's own answer to that question, so
        // this test fails the moment someone adds a String or a Box.
        assert!(!std::mem::needs_drop::<VoiceParams>());
        assert!(!std::mem::needs_drop::<Command>());
        assert!(!std::mem::needs_drop::<VoiceId>());
        assert!(!std::mem::needs_drop::<SoundId>());
    }

    #[test]
    fn a_throttle_outside_the_range_is_clamped_not_trusted() {
        // A game reading a speed while paused, or one frame of a bad
        // dt, should not hand the synthesiser a negative rev count.
        assert_eq!(VoiceParams::engine(-1.0).get(0), 0.0);
        assert_eq!(VoiceParams::engine(2.0).get(0), 1.0);
        assert_eq!(VoiceParams::engine(0.5).get(0), 0.5);
    }

    #[test]
    fn reading_an_unset_slot_yields_silence_rather_than_panicking() {
        // A panic here would abort the process from the audio thread.
        let p = VoiceParams::engine(0.5);
        assert_eq!(p.get(3), 0.0);
        assert_eq!(p.get(99), 0.0);
    }
}
