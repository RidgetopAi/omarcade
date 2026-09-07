# The Omarcade audio seam — specification

Status: **proposed, not built**. Nothing in this document exists in code yet.
Written to be argued with before any of it is written.

Decisions already made with Brian (2026-09-07):

- **Backend: cpal.** 9 crates against rodio's 57 and kira's 39, on a workspace
  that is 5 crates today. Proven to synthesize the Omaprix engine per-sample in
  real time with the pitch driven from outside the audio thread.
- **Audio is passed into `update`**, not a fourth trait method. Sound is caused
  by simulation, not painted after it.
- **The mixer and the stream live in core.** Games hand core sound *sources*;
  they never touch cpal, a device, or a sample buffer.

---

## 1. The constraint everything else follows from

The audio callback runs on a **separate, real-time thread**, and at a 256-frame
buffer it must finish inside **5.33 ms** — about eight times more often than a
60 fps video frame.

It **cannot allocate. It cannot lock. It cannot block.**

Every awkward-looking part of this design is that sentence. A `Mutex` shared
between the game thread and the audio callback is the classic way to get audio
that crackles under load and is fine on the developer's machine — the game
thread holds the lock for one frame too long and the callback misses its
deadline. So the two threads never share a lock. They share a **lock-free
channel** in one direction and **atomics** in the other.

This is also why `Voice` (§4) is a trait the *audio thread* calls, not a closure
the game supplies: a game closure could capture anything, and "anything" includes
allocation.

---

## 2. What a game sees

```rust
pub trait Game {
    fn on_input(&mut self, event: InputEvent) -> bool;
    fn update(&mut self, dt: f32, audio: &mut Audio<'_>);   // ← changed
    fn render(&mut self, canvas: &mut Canvas<'_>);
}
```

`Audio<'_>` mirrors [`Canvas`] deliberately: a borrowed view handed out per
frame, allocating nothing, owning nothing, outliving nothing. If you know how
`Canvas` behaves you already know how `Audio` behaves.

```rust
impl<'a> Audio<'a> {
    /// Fire a one-shot. Cheap; safe to call every frame.
    pub fn play(&mut self, sound: SoundId);

    /// Fire a one-shot with a gain and pitch trim.
    pub fn play_with(&mut self, sound: SoundId, gain: f32, pitch: f32);

    /// Set a continuous voice's parameters. Idempotent — call it every
    /// frame with this frame's numbers; that IS the interface.
    pub fn set(&mut self, voice: VoiceId, params: VoiceParams);

    /// Start/stop a continuous voice.
    pub fn start(&mut self, voice: VoiceId);
    pub fn stop(&mut self, voice: VoiceId);

    /// Duck everything except `voice` to `gain` over `seconds`.
    pub fn duck(&mut self, except: VoiceId, gain: f32, seconds: f32);

    /// Master volume, 0.0..=1.0. Persisted by the game, not by core.
    pub fn set_master(&mut self, gain: f32);
}
```

### What Omaprix's `update` would look like

```rust
fn update(&mut self, dt: f32, audio: &mut Audio<'_>) {
    // ... existing simulation, unchanged ...

    // Continuous: this frame's numbers, every frame. No edge detection,
    // no "if changed" — set() is idempotent and that is the point.
    audio.set(self.engine, VoiceParams::engine(
        self.car.speed / self.tuning.top_speed,
    ));
    audio.set(self.tires, VoiceParams::squeal(
        self.car.cornering(&self.road, &self.tuning).abs(),
    ));
    audio.set(self.surface, VoiceParams::surface(
        self.car.surface(), self.car.speed,
    ));

    // One-shots, off the event stream that already drives flash_for.
    if let Some(event) = event {
        match event {
            Event::GreenLight => audio.play(SoundId::GreenLight),
            Event::Checkpoint { .. } | Event::LapDone { .. } =>
                audio.play(SoundId::Checkpoint),
            Event::Finished { .. } => audio.play(SoundId::Finish),
            _ => {}
        }
    }
    for _ in 0..self.traffic.take_passes() {
        audio.play(SoundId::Pass);
    }
}
```

Note what is absent: no `Option<Sound>`, no error handling, no "is audio
available" check. **Audio that fails is silent, never fatal.** A missing device,
a busy server, a build without audio — the game runs, quieter. See §6.

---

## 3. How the two threads talk

```
GAME THREAD                          AUDIO THREAD (real-time)
────────────                         ────────────────────────
update(dt, audio)
  audio.play(..)   ──┐
  audio.set(..)      ├─ ring buffer ─►  drain all pending commands
  audio.duck(..)   ──┘  (lock-free)     (bounded, no allocation)
                                            │
                                            ▼
                                        for each active voice:
                                          voice.render(&mut scratch)
                                          mix into the output
                                            │
                                            ▼
                                        apply duck + master gain
```

- **Game → audio: a bounded lock-free ring of commands**, drained once per
  callback. Bounded because unbounded means allocation. If it ever fills, the
  oldest commands are dropped and a counter increments — a dropped sound effect
  is a far better failure than a blocked game thread.
- **Audio → game: atomics only** (an `is_playing` flag, the drop counter).
  Nothing richer is needed and anything richer invites a lock.
- **`VoiceParams` is `Copy` and plain data** — no `String`, no `Box`, no `Arc`.
  It has to cross into a context that cannot deallocate it.

---

## 4. Where sound actually comes from

Core owns the stream and the mixing. It does **not** know what an engine sounds
like — that is Omaprix's business, and Breakout must not link it.

```rust
/// A source of samples, rendered on the audio thread.
///
/// ⚠️ `render` runs under the real-time constraint in §1: no allocation,
/// no locking, no I/O, no panicking. Implementations are pure DSP over
/// their own state.
pub trait Voice: Send {
    /// Fill `out` (mono) with the next samples. `params` is the most
    /// recent value the game set; it may be several frames old and may
    /// jump — interpolate internally rather than trusting continuity.
    fn render(&mut self, out: &mut [f32], params: VoiceParams, sample_rate: f32);

    /// False once a one-shot has finished, so the mixer can retire it.
    fn alive(&self) -> bool { true }
}
```

### Voices are registered at STARTUP ONLY

Every voice a game will ever use is registered before the first frame, and the
set is fixed for the run. This is a real constraint and it buys three things:

- The mixer's voice table is built **before the stream is hot**, so no boxed
  trait object ever crosses a thread boundary while audio is running.
- The audio thread never has to install anything, which means it never has to
  allocate — see §1.
- Every `VoiceId` is unconditionally valid. There is no "not registered yet"
  case for a game to handle, because there cannot be one.

Nothing in the suite wants more: all seven Omaprix sounds are known at compile
time, and Breakout and Pong are one-shots. Dynamic registration would only be
needed by something that loads sounds it did not know about when it was built,
which Omarcade is not. If a future title genuinely needs it, adding it is
purely additive and no existing game changes.

Games register voices at startup, before the stream is hot:

```rust
// In Omaprix's main, once.
let engine = audio.register(Box::new(racer::sound::Engine::new()));
let tires  = audio.register(Box::new(racer::sound::Squeal::new()));
```

`Box::new` here is fine — it happens once, on the game thread, at startup.
Registration after the stream is running hands the box across via the same
command ring, and the audio thread only ever *uses* it.

**This is the seam that keeps the suite decoupled.** Omaprix's `Engine` voice
lives in `games/omarcade-racer/src/sound.rs`. Breakout's paddle blip lives in
Breakout. Core provides the stream, the mixer, the ducking and the clock, and
knows nothing about cars.

### Why not "load a WAV"

Because the engine has to track `Drive::speed` continuously, and a sample
cannot. That single requirement is what ruled out sample-playback backends and
what shapes this trait. Core will also want a plain `Sample` voice (a baked WAV,
for Breakout's one-shots) — that is just another `Voice` implementation, and the
mixer cannot tell them apart.

---

## 5. Which sounds are which

From the seven agreed with Brian, and what already drives each:

| Sound | Kind | Driven by |
|---|---|---|
| Engine | continuous | `Drive::speed / Tuning::top_speed` |
| Tire squeal | continuous | `Drive::cornering()` — already 0..1 |
| Surface | continuous | `Drive::surface()` → Road/Rumble/Grass |
| Crash | one-shot | `crash::Explosion::start` (1.4 s `BURN_TIME`) |
| Car pass | one-shot | `traffic::take_passes()` — **close passes only**, see below |
| Checkpoint / lap | one-shot | `Event::Checkpoint` / `LapDone` |
| Start / qualify / finish | one-shot | `Event::GreenLight` / `Qualified` / `Finished` |

Every one of these signals exists today. **No new plumbing is needed to decide
what plays** — only to play it.

The crash is the one case that needs `duck`: the engine must drop out on impact
and return over `crash::recovery_time`, which is derived from the tuning and so
already the right length.

---

## 6. Failure is silence

`Audio::new` returns an `Audio` whatever happens. If the device is missing, the
server is busy, or the build has audio disabled, every method becomes a no-op
and the game is unchanged apart from being quiet.

This is a deliberate asymmetry with rendering: a game that cannot draw is
broken, and a game that cannot play sound is a game with the volume down. It
also keeps `Game::update` free of `Result`, which would otherwise infect every
call site for a failure nobody can act on.

Diagnostics go to stderr once, at startup — the same way a failed score save
already reports itself.

---

## 6a. Volume and mute

**A key in-game, and a file that remembers it.** Both, because they answer
different questions: the key is how you turn it down *now*, the file is why it
is still down tomorrow.

| Key | Does |
|---|---|
| `M` | toggle mute |
| `-` | quieter, in steps |
| `=` | louder, in steps |

⚠️ **These three keys do not exist yet.** `Key` is a closed enum of eight
variants — `Left`, `Right`, `Up`, `Down`, `Space`, `Enter`, `Escape`, `P` — so
`M`, `Minus` and `Equals` have to be ADDED to core before any of this works.
They are unbound in all three games precisely because core cannot deliver them
today. Adding a variant is additive and breaks nothing (games match on the
variants they care about), but it is a core change and belongs in the same
commit as the mixer, not discovered halfway through.

**And they must be handled where a game cannot swallow them.** Input reaches a
game through `on_input`, which owns the event and may return `false` to quit.
If volume were routed through there, every game would have to remember to
forward three keys it does not care about, and the first one to forget has a
title where mute silently does nothing. So the backend intercepts `M`, `-` and
`=` BEFORE `on_input` sees them, applies them to the mixer, and does not pass
them on. That is a deliberate exception to "the backend does not interpret
input", and the reason is that volume is a property of the SUITE rather than of
any game — the same argument that puts the mixer in core at all.

A game that wants its own meaning for `M` therefore cannot have one. That is
the cost, it is small, and it is worth naming out loud rather than discovering
in Breakout.

The value lives in **`$XDG_STATE_HOME/omarcade/audio.toml`**, beside the scores
directory core already owns, and follows the same rules as [`scores`]: state
rather than data, machine-local, written atomically through a temp file and a
rename, and a read that fails falls back to a default instead of propagating.

```toml
# $XDG_STATE_HOME/omarcade/audio.toml
volume = 0.7
muted = false
```

**This is suite-wide by construction.** Volume lives in core's mixer, so
Breakout and Pong inherit the keys, the file and the behaviour without writing
any of it. That is the whole reason to decide it once here rather than three
times later.

Two consequences worth stating:

- **Muting must be visible.** A game that goes silent on a keypress with no
  feedback looks like it crashed. The HUD needs an indicator — this is a
  rendering job each game owns, not something core can do.
- **The system mixer still works.** `pavucontrol` and `wpctl` see each game as
  its own stream regardless, and nothing here interferes with that. This is a
  convenience on top, not a replacement.

## 7. What this costs

- `core` gains **one dependency, cpal** (9 crates, several already shared with
  winit). Workspace goes 5 → ~12.
- `Game::update` changes signature. **Three call sites**, all in this repo.
- Games that want no sound ignore the parameter. Breakout and Pong compile
  unchanged apart from the signature.

## 8. Answered by Brian (2026-09-07)

1. **Car pass: CLOSE PASSES ONLY.** A pass fires roughly 11 times a lap, ~35 a
   race; a whoosh for every one of them is wallpaper, and wallpaper is worse
   than silence because it trains the ear to ignore the channel. The lateral
   distance is already computed in `collide`, so the gate costs nothing new —
   only a threshold, which is a tuning constant and belongs with the other
   ones in the racer, not in core.
2. **Volume: a key AND a file** — see §6a.
3. **The marquee makes no sound.** "No sound on marquee is fine for now." It
   stays a silent bar widget; nothing in this design gives it a voice, and the
   audio stream belongs to game processes only.
4. **`register` is startup-only** — see §4.

## 9. Still open, not blocking

- The engine's idle is its weakest state (thin fundamental). Least important in
  a racing game, but a grid start-up sequence would expose it.
- Brian's verdict on the engine is **provisional on hearing it in the game**:
  "won't know for sure until I play it and see how it sounds in realtime."
  Everything here is what makes that hearing possible.
- The remaining six sounds have signals and a category each (§5) but no design.
  The method that worked for the engine — reference files, measurement, then a
  real-time playground with a scope — is the one to reuse.
