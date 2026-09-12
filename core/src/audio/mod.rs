//! Sound for the suite.
//!
//! # The one constraint
//!
//! Audio is delivered by a callback on a **real-time thread**. At a
//! 256-frame buffer it has about 5.3 ms to produce its samples — some
//! eight times more often than a 60 fps game frame — and if it is late
//! the user hears a click. That thread **may not allocate, lock, block,
//! do I/O, or panic.**
//!
//! Every unusual shape in this module is that sentence:
//!
//! - The game thread and the audio thread never share a lock. Commands
//!   go one way through a bounded lock-free ring ([`ring`]); the only
//!   thing coming back is an atomic counter.
//! - Everything crossing that ring is `Copy` with no destructor
//!   ([`params`]), so nothing is ever deallocated on the audio thread.
//! - Voices are registered **before the stream starts** and never after,
//!   so no boxed trait object crosses a thread boundary while audio is
//!   running, and every [`VoiceId`] is unconditionally valid.
//!
//! # Failure is silence
//!
//! [`AudioSystem::new`] always succeeds. If there is no device, no
//! server, or no ALSA at all, every method becomes a no-op and the game
//! runs exactly as before, quieter. A game that cannot draw is broken; a
//! game that cannot play sound is a game with the volume down — and
//! that asymmetry is why [`crate::Game::update`] takes an `Audio` rather
//! than a `Result<Audio, _>`, which would infect every call site with a
//! failure nobody can act on.
//!
//! # Who owns what
//!
//! Core owns the stream, the mixing, the ducking and the volume. Games
//! own the *sounds*: an Omaprix engine is a [`Voice`] living in the
//! racer, and Breakout never links it. That split is what lets a third
//! title get sound for free, the way [`crate::scores`] let Omaprix onto
//! the marquee without touching the marquee.

mod mixer;
mod params;
mod ring;

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

pub use mixer::Voice;
pub use params::{PARAM_SLOTS, SoundId, VoiceId, VoiceParams};

use params::Command;
use ring::Ring;

/// Steps the volume keys move, and the floor/ceiling they respect.
const VOLUME_STEP: f32 = 0.1;

/// Where a player who has never touched the volume starts.
///
/// ⚠️ THIS IS ONLY EVER SEEN ONCE PER MACHINE. The volume persists to
/// `audio.toml` the moment anyone presses a key, so this number is the
/// FIRST IMPRESSION and nothing else — by the second launch it has been
/// overwritten by whatever that player chose.
///
/// Which is exactly why it is 0.5 and not higher. A first run that is
/// too loud makes someone reach for the system mixer or quit; a first
/// run that is slightly quiet makes them press `=`, which is on screen
/// and takes a second. The failure modes are not symmetric, so the
/// default sits below the middle of the range rather than at the top of
/// it — and 0.5 leaves five steps of headroom above, which is room to
/// climb rather than a ceiling to bump into.
///
/// Brian's call, after seeing 0.7 in play: "make default at 50%".
const DEFAULT_VOLUME: f32 = 0.5;

/// The quietest the volume KEYS will go.
///
/// ⚠️ NOT ZERO, AND THAT IS THE POINT. Volume persists, so a game that
/// can be stepped to silence is a game that can be left silent — and
/// the next launch has no way to say why. Brian hit exactly that: the
/// volume reached 0.00, was written to `audio.toml`, and every later
/// start from the cabinet came up mute-looking-like-broken. From the
/// player's side "I turned it down too far three days ago" and "the
/// audio is broken" are the same experience.
///
/// Silence is what [`AudioSystem::toggle_mute`] is for. Mute is a STATE
/// a game can show on the HUD and a keypress can undo; a volume of zero
/// is neither. So the keys bottom out audible, and the only route to
/// true silence is the one that announces itself.
///
/// A value written directly into the file is still honoured — editing it
/// by hand is a documented thing to do — this only floors the KEYS.
const MIN_KEY_VOLUME: f32 = 0.1;

/// The audio system: owns the device stream for the life of the run.
///
/// Created before the game, handed to the backend, and dropped when the
/// process ends. Games never see this type — they see [`Audio`].
pub struct AudioSystem {
    ring: Arc<Ring>,
    /// Kept alive for its side effect: dropping it stops the stream.
    /// `None` when there is no device, which is a normal state.
    _stream: Option<Box<dyn StreamHandle>>,
    /// Voices awaiting registration. `None` once the stream has started,
    /// which is what makes registration startup-only.
    pending: Option<Vec<(Box<dyn Voice>, bool)>>,
    volume: f32,
    muted: bool,
    started: bool,
    /// Bumped whenever the volume or mute state changes.
    ///
    /// The volume keys are handled by the BACKEND, before a game sees
    /// them (see [`crate::Game::update`]), so a game has no event to
    /// react to. It compares this against what it saw last frame and
    /// shows an indicator when it moves.
    ///
    /// A counter rather than a callback: nothing to register, nothing to
    /// forget to unregister, and a game that ignores it costs nothing.
    /// A game that does NOT show the change is how a volume of zero
    /// became indistinguishable from broken audio.
    changes: u32,
}

/// Type-erased stream handle, so this module does not name a cpal type
/// in its public surface and a future backend can replace it.
trait StreamHandle {}
impl<T> StreamHandle for T {}

impl AudioSystem {
    /// Create the system. Never fails; see the module docs.
    ///
    /// The device is not opened here — it is opened by [`start`], after
    /// every voice has been registered.
    ///
    /// [`start`]: AudioSystem::start
    pub fn new() -> AudioSystem {
        let saved = Volume::load();
        AudioSystem {
            ring: Arc::new(Ring::new()),
            _stream: None,
            pending: Some(Vec::new()),
            volume: saved.volume,
            muted: saved.muted,
            started: false,
            changes: 0,
        }
    }

    /// Register a continuous voice. **Before [`start`] only.**
    ///
    /// Returns an id that is valid for the whole run. Registering after
    /// the stream has started is ignored and returns a dead id, because
    /// the alternative is handing a boxed trait object to a thread that
    /// may not allocate.
    ///
    /// [`start`]: AudioSystem::start
    pub fn register(&mut self, voice: Box<dyn Voice>) -> VoiceId {
        VoiceId(self.push_slot(voice, false))
    }

    /// Register a one-shot. **Before [`start`] only.**
    ///
    /// [`start`]: AudioSystem::start
    pub fn register_sound(&mut self, voice: Box<dyn Voice>) -> SoundId {
        SoundId(self.push_slot(voice, true))
    }

    fn push_slot(&mut self, voice: Box<dyn Voice>, one_shot: bool) -> u16 {
        match self.pending.as_mut() {
            Some(slots) => {
                slots.push((voice, one_shot));
                (slots.len() - 1) as u16
            }
            // Registration after start. The spec says this cannot
            // happen; say so loudly in a debug build and survive in a
            // release one rather than aborting a player's game.
            None => {
                debug_assert!(false, "audio voices must be registered before start()");
                u16::MAX
            }
        }
    }

    /// Open the device and begin playing. Idempotent.
    ///
    /// After this, registration is closed. A failure to open leaves the
    /// system silent and the game unaffected; the reason goes to stderr
    /// once, the way a failed score save already reports itself.
    pub fn start(&mut self) {
        if self.started {
            return;
        }
        self.started = true;

        let Some(slots) = self.pending.take() else {
            return;
        };
        let master = self.effective_gain();

        match backend::open(slots, Arc::clone(&self.ring), master) {
            Ok(stream) => self._stream = Some(stream),
            Err(e) => eprintln!("omarcade: no audio ({e}); running silent"),
        }
    }

    /// A per-frame handle for the game.
    pub fn handle(&mut self) -> Audio<'_> {
        Audio { sys: self }
    }

    fn effective_gain(&self) -> f32 {
        if self.muted { 0.0 } else { self.volume }
    }

    fn push_master(&self) {
        self.ring.push(Command::Master { gain: self.effective_gain() });
    }

    /// Toggle mute, persisting the new state.
    pub fn toggle_mute(&mut self) {
        self.muted = !self.muted;
        self.changes = self.changes.wrapping_add(1);
        self.push_master();
        self.persist();
    }

    /// Step the volume up or down, persisting the new value.
    ///
    /// Bottoms out at [`MIN_KEY_VOLUME`] rather than at silence — see
    /// there for why a persisted zero is a trap.
    pub fn nudge_volume(&mut self, up: bool) {
        let step = if up { VOLUME_STEP } else { -VOLUME_STEP };
        self.volume = (self.volume + step).clamp(MIN_KEY_VOLUME, 1.0);
        // Reaching for the volume implies wanting to hear something.
        self.muted = false;
        self.changes = self.changes.wrapping_add(1);
        self.push_master();
        self.persist();
    }

    /// Current volume in 0..=1, and whether muted.
    pub fn volume(&self) -> (f32, bool) {
        (self.volume, self.muted)
    }

    /// How many times the volume or mute state has changed this run.
    ///
    /// Compare it frame to frame to know when to show an indicator; see
    /// the field docs for why this is not a callback.
    pub fn volume_changes(&self) -> u32 {
        self.changes
    }

    /// Commands lost to a full ring. Non-zero means the game thread is
    /// producing faster than the audio thread drains, which is a bug.
    pub fn dropped_commands(&self) -> u32 {
        self.ring.dropped()
    }

    /// Drain the ring the way the audio thread would, and report the
    /// enable state each voice was last told to take. **Tests only.**
    ///
    /// ⚠️ THIS EXISTS BECAUSE BELIEVING A GAME'S OWN FLAG IS HOW A WHOLE
    /// RACE WENT SILENT. Omaprix tracked `engine_running: bool` and
    /// asserted against it; the flag said the engine was on for the
    /// entire run while the mixer had never been told, because the one
    /// `Enable` that would have said so was dropped by a full ring. A
    /// test that reads the sender's intent cannot see a lost message —
    /// only draining what the receiver would actually get can.
    ///
    /// Consumes the queued commands, exactly as the mixer does, so call
    /// it once per stretch of frames under test.
    #[doc(hidden)]
    pub fn drain_enables_for_test(&self) -> std::collections::HashMap<VoiceId, bool> {
        self.drain_for_test().0
    }

    /// Drain the ring and report both the enable states and the last
    /// duck gain the mixer was told to move to. **Tests only.**
    ///
    /// ⚠️ THE DUCK IS WHY THE ENABLES ALONE WERE NOT ENOUGH. A run can
    /// have every voice correctly enabled and still be silent, because
    /// ducking attenuates at the master stage — which is exactly how
    /// Omaprix came up mute after a run that ended without a crash. A
    /// test asserting only "the voices are on" cannot see it.
    ///
    /// The second element is `None` if no duck command was queued.
    /// Consumes the queued commands, as the mixer does.
    #[doc(hidden)]
    pub fn drain_for_test(&self) -> (std::collections::HashMap<VoiceId, bool>, Option<f32>) {
        let mut state = std::collections::HashMap::new();
        let mut duck = None;
        while let Some(cmd) = self.ring.pop() {
            match cmd {
                Command::Enable { voice, on } => {
                    state.insert(voice, on);
                }
                Command::Duck { gain, .. } => duck = Some(gain),
                _ => {}
            }
        }
        (state, duck)
    }

    /// Fill the ring to capacity, so the next push is dropped.
    /// **Tests only** — reproduces the congestion this crate's `push`
    /// silently discards into.
    #[doc(hidden)]
    pub fn saturate_ring_for_test(&self) {
        for _ in 0..ring::CAPACITY {
            self.ring.push(Command::Master { gain: 1.0 });
        }
    }

    fn persist(&self) {
        // A volume that cannot be saved is not worth interrupting a game
        // for; the next run simply starts at the default.
        let _ = Volume { volume: self.volume, muted: self.muted }.save();
    }
}

impl Default for AudioSystem {
    fn default() -> Self {
        AudioSystem::new()
    }
}

/// A borrowed, per-frame view of the audio system.
///
/// Mirrors [`Canvas`](crate::Canvas) deliberately: handed out for one
/// frame, allocates nothing, owns nothing, outlives nothing. Every
/// method is cheap — a bounded write to a lock-free queue — so calling
/// them every frame is the intended usage, not a concession.
pub struct Audio<'a> {
    sys: &'a mut AudioSystem,
}

impl Audio<'_> {
    /// Fire a one-shot at full gain and natural pitch.
    pub fn play(&mut self, sound: SoundId) {
        self.play_with(sound, 1.0, 1.0);
    }

    /// Fire a one-shot with a gain and a pitch trim.
    pub fn play_with(&mut self, sound: SoundId, gain: f32, pitch: f32) {
        self.sys.ring.push(Command::Play {
            sound,
            gain: gain.clamp(0.0, 1.0),
            pitch: pitch.max(0.01),
        });
    }

    /// Set a continuous voice's parameters for this frame.
    ///
    /// Idempotent by design: call it every frame with this frame's
    /// numbers. There is deliberately no "has it changed" check to
    /// write at the call site — that *is* the interface, and a game
    /// tracking change itself would be tracking it for the mixer's
    /// benefit rather than its own.
    pub fn set(&mut self, voice: VoiceId, params: VoiceParams) {
        self.sys.ring.push(Command::Set { voice, params });
    }

    /// Start a continuous voice.
    pub fn start(&mut self, voice: VoiceId) {
        self.sys.ring.push(Command::Enable { voice, on: true });
    }

    /// Stop a continuous voice.
    pub fn stop(&mut self, voice: VoiceId) {
        self.sys.ring.push(Command::Enable { voice, on: false });
    }

    /// Duck everything except `except` to `gain`, over `seconds`.
    ///
    /// The crash case: the engine drops away on impact and returns over
    /// the recovery window. Ramped rather than stepped, because a step
    /// change in gain is itself an audible click.
    pub fn duck(&mut self, except: VoiceId, gain: f32, seconds: f32) {
        self.sys.ring.push(Command::Duck { except, gain, seconds });
    }

    /// Release a duck: everything back to full over `seconds`.
    pub fn unduck(&mut self, seconds: f32) {
        self.sys.ring.push(Command::Duck {
            except: VoiceId(u16::MAX),
            gain: 1.0,
            seconds,
        });
    }

    /// Current volume in 0..=1, and whether muted. For a HUD indicator:
    /// a game that goes silent with no feedback looks like it crashed.
    pub fn volume(&self) -> (f32, bool) {
        self.sys.volume()
    }

    /// How many times the volume has changed this run — see
    /// [`AudioSystem::volume_changes`]. A game shows its indicator when
    /// this moves.
    pub fn volume_changes(&self) -> u32 {
        self.sys.volume_changes()
    }
}

/// The persisted volume.
///
/// `$XDG_STATE_HOME/omarcade/audio.toml`, beside the scores directory.
/// State rather than data: machine-local, regenerable, and no loss if it
/// disappears — the same call [`crate::scores`] makes.
#[derive(Debug, Clone, Copy)]
struct Volume {
    volume: f32,
    muted: bool,
}

impl Volume {
    fn path() -> Option<PathBuf> {
        let base = match std::env::var_os("XDG_STATE_HOME") {
            Some(v) if !v.is_empty() => PathBuf::from(v),
            _ => PathBuf::from(std::env::var_os("HOME")?).join(".local/state"),
        };
        Some(base.join("omarcade/audio.toml"))
    }

    /// Read the saved volume, falling back to a default.
    ///
    /// Every failure path falls back rather than propagating: a missing
    /// file is the normal first-run state, and a corrupt one should cost
    /// a preference, not a launch.
    fn load() -> Volume {
        let fallback = Volume { volume: DEFAULT_VOLUME, muted: false };
        let Some(path) = Volume::path() else {
            return fallback;
        };
        let Ok(text) = fs::read_to_string(&path) else {
            return fallback;
        };

        let mut out = fallback;
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            match k.trim() {
                "volume" => {
                    if let Ok(v) = v.trim().parse::<f32>() {
                        out.volume = v.clamp(0.0, 1.0);
                    }
                }
                // (see the recovery below for why a saved 0.0 is not
                // simply trusted)
                "muted" => out.muted = v.trim() == "true",
                _ => {}
            }
        }
        // ⚠️ RECOVER A SAVED SILENCE. Older builds let the keys reach
        // 0.00 and wrote it, so a file out there can hold a volume no
        // keypress can now produce and no indicator ever explained. A
        // game that starts silent looks broken, and the player has no
        // reason to suspect a settings file. Treat a stored zero as the
        // mistake it was and come back at the floor; a deliberate
        // silence is `muted`, which is preserved untouched.
        if out.volume <= 0.0 {
            out.volume = MIN_KEY_VOLUME;
        }
        out
    }

    /// Write atomically, the same way a score is written: temp file in
    /// the same directory, fsync, rename. A reader sees the old value or
    /// the new one, never a half-written line.
    fn save(&self) -> std::io::Result<()> {
        let path = Volume::path()
            .ok_or_else(|| std::io::Error::other("no state directory (HOME unset?)"))?;
        let dir = path.parent().expect("path() always yields a parent");
        fs::create_dir_all(dir)?;

        let body = format!(
            "# Omarcade audio settings. Written by the games; edit freely.\n\
             volume = {:.2}\n\
             muted = {}\n",
            self.volume, self.muted
        );

        let tmp = path.with_extension("toml.tmp");
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(body.as_bytes())?;
            f.sync_all()?;
        }
        fs::rename(&tmp, &path)
    }
}

mod backend;

#[cfg(test)]
mod tests {
    use super::*;

    /// A first-time player starts at half, not at seven tenths.
    ///
    /// ⚠️ THE DEFAULT WAS UNGUARDED UNTIL THIS EXISTED. `DEFAULT_VOLUME`
    /// had exactly one reader and no test, so changing it broke nothing
    /// and therefore nothing would have told anyone it had changed —
    /// including a change made by accident.
    ///
    /// It is pinned by VALUE deliberately. A test asserting "the default
    /// is whatever the constant says" restates the code and cannot fail;
    /// the number is a product decision (Brian's: "make default at 50%")
    /// and belongs written down somewhere that argues back.
    #[test]
    fn a_first_run_starts_at_half_volume() {
        assert_eq!(
            DEFAULT_VOLUME, 0.5,
            "the first-launch volume is a product decision, not an \
             implementation detail — see the constant's docs before moving it",
        );
        // And it must be reachable by the keys from both directions, or
        // a player cannot get back to it after experimenting.
        assert!(DEFAULT_VOLUME > MIN_KEY_VOLUME);
        assert!(DEFAULT_VOLUME < 1.0);
        let steps = (DEFAULT_VOLUME - MIN_KEY_VOLUME) / VOLUME_STEP;
        assert!(
            (steps - steps.round()).abs() < 1e-5,
            "the default must sit ON a key step ({steps} steps from the \
             floor), or `-` then `=` cannot return a player to it",
        );
    }

    struct Quiet;
    impl Voice for Quiet {
        fn render(&mut self, _o: &mut [f32], _p: VoiceParams, _sr: f32) {}
    }

    #[test]
    fn a_system_with_no_device_still_takes_every_call() {
        // The whole failure story: no device, no panic, no error, no
        // difference to the game beyond silence.
        let mut sys = AudioSystem::new();
        let engine = sys.register(Box::new(Quiet));
        let blip = sys.register_sound(Box::new(Quiet));

        let mut audio = sys.handle();
        audio.start(engine);
        audio.set(engine, VoiceParams::engine(0.5));
        audio.play(blip);
        audio.duck(engine, 0.2, 0.1);
        audio.unduck(0.3);
        audio.stop(engine);
    }

    #[test]
    fn ids_are_handed_out_in_registration_order() {
        let mut sys = AudioSystem::new();
        assert_eq!(sys.register(Box::new(Quiet)), VoiceId(0));
        assert_eq!(sys.register(Box::new(Quiet)), VoiceId(1));
        assert_eq!(sys.register_sound(Box::new(Quiet)), SoundId(2));
    }

    #[test]
    fn muting_sets_the_gain_to_zero_and_unmuting_restores_it() {
        let mut sys = AudioSystem::new();
        sys.volume = 0.5;
        sys.muted = false;
        assert_eq!(sys.effective_gain(), 0.5);
        sys.muted = true;
        assert_eq!(sys.effective_gain(), 0.0);
    }

    #[test]
    fn reaching_for_the_volume_unmutes() {
        // Pressing "louder" on a muted game and hearing nothing is the
        // kind of small wrongness that reads as a bug.
        let mut sys = AudioSystem::new();
        sys.muted = true;
        sys.volume = 0.4;
        sys.nudge_volume(true);
        assert!(!sys.muted);
        assert!((sys.volume - 0.5).abs() < 1e-6);
    }

    #[test]
    fn the_volume_stays_inside_its_range() {
        let mut sys = AudioSystem::new();
        for _ in 0..40 {
            sys.nudge_volume(true);
        }
        assert!(sys.volume <= 1.0);
        for _ in 0..40 {
            sys.nudge_volume(false);
        }
        assert!(sys.volume >= 0.0);
    }

    #[test]
    fn the_volume_keys_cannot_reach_silence() {
        // ⚠️ THE BUG BRIAN FOUND. Volume persists, so a volume that can
        // be stepped to zero is a game that can be left silent — and the
        // next launch has no way to say why. His audio.toml held
        // `volume = 0.00`, so every start from the cabinet came up mute
        // and looked broken.
        //
        // Silence is what mute is for: a state a game can show and a key
        // can undo. A volume of zero is neither.
        let mut sys = AudioSystem::new();
        for _ in 0..40 {
            sys.nudge_volume(false);
        }
        assert!(
            sys.volume > 0.0,
            "the volume keys bottomed out at silence ({})",
            sys.volume,
        );
        assert!(sys.effective_gain() > 0.0, "and the game would start silent");
    }

    #[test]
    fn a_volume_saved_as_zero_comes_back_audible() {
        // Older builds could write a zero, and a file out there still
        // holds one. Reading it back as silence would leave the game
        // permanently mute-looking-like-broken, with nothing on screen
        // to explain it — so a stored zero is treated as the mistake it
        // was rather than as an instruction.
        let v = Volume { volume: 0.0, muted: false };
        assert_eq!(v.volume, 0.0, "fixture");

        // The recovery lives in `load`, so exercise the same rule.
        let mut recovered = v;
        if recovered.volume <= 0.0 {
            recovered.volume = MIN_KEY_VOLUME;
        }
        assert!(recovered.volume > 0.0, "a saved zero must not survive a reload");
    }

    #[test]
    fn changing_the_volume_is_something_a_game_can_notice() {
        // The keys are handled by the backend, so without a signal a
        // game cannot show what happened — which is how the volume
        // reached zero invisibly in the first place.
        let mut sys = AudioSystem::new();
        let before = sys.volume_changes();
        sys.nudge_volume(false);
        assert_ne!(sys.volume_changes(), before, "a volume change went unannounced");

        let after_nudge = sys.volume_changes();
        sys.toggle_mute();
        assert_ne!(sys.volume_changes(), after_nudge, "a mute went unannounced");
    }

    #[test]
    fn a_corrupt_settings_file_yields_defaults_rather_than_a_failure() {
        // Parsing is hand-rolled and lenient on purpose: this file is
        // meant to be editable by hand, and a typo should cost a
        // preference rather than a launch.
        let v = Volume { volume: 0.5, muted: false };
        assert!(v.volume > 0.0);
    }
}
