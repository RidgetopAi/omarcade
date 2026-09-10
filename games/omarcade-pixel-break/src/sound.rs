//! Pixel Break's voice: twelve struck sounds, clean-synth arcade.
//!
//! # Where these numbers came from
//!
//! Brian tuned them by ear in `tools/sfx/pixel-break.html` on 2026-09-09.
//! They are not defaults and they are not derived — where my starting
//! guess and his ear disagreed, his ear won, and the comments below say
//! what he moved rather than what I proposed.
//!
//! The shape of his tuning is worth recording, because it is a single
//! idea applied twelve times: **he lowered everything except the plain
//! brick.** The paddle fell 42%, the bomb 47%, the reinforced break 24%,
//! while the brick — the sound heard more than any other — rose 9% and
//! grew half again as long. That is not twelve independent choices; it
//! is carving a hole in the middle of the register and putting the
//! most-heard sound in it.
//!
//! # Why the playground's numbers can be trusted here
//!
//! Because [`strike`] below and the playground's `strike()` are the same
//! function, verified sample-for-sample rather than by ear: identical
//! notes through both agree to 6-7 significant figures, the residual
//! being `f32` against `f64`.
//!
//! That check exists because the racer's tools do **not** hold: its
//! `chimes.html` builds notes from a `PeriodicWave` and an exponential
//! gain ramp, which is a different function from its Rust — different
//! harmonic series, normalised on one side only, an exponential attack
//! where the Rust is linear, and a decay ending at 0.0001 where the Rust
//! ends at 0.057. A level of 0.32 there was never a level of 0.32 in the
//! game. It never mattered because chimes play alone; it would matter
//! here, because §7 asks for every sound to survive a ten-ball mix, and
//! that is a claim about *relative* levels.
//!
//! # Why twelve sounds share one voice
//!
//! A registered one-shot is a single slot, and playing it again calls
//! [`Voice::retrigger`] — it restarts, it does not layer. Twelve slots
//! would therefore give twelve simultaneous sounds but only one of each
//! *kind*, which is exactly backwards for this game: nothing ever plays
//! two game-overs at once, and everything plays a shower of brick hits.
//!
//! So the sounds are grouped by how often they collide with themselves
//! ([`Bank`]) and selected within a group by the pitch argument — the
//! idiom the racer's `Chime` established for its five countdown sounds.
//!
//! ⚠️ [`Voice::render`] runs on the audio thread: no allocation, no
//! locking, no panicking. Everything here is arithmetic over owned state.

use std::f32::consts::TAU;

use omarcade_core::{Audio, AudioSystem, SoundId, Voice, VoiceParams};

use crate::state::Cue;

// ─────────────────────────────────────────────────────────────────
// BRIAN'S TUNING. Change these by ear, in the playground, not here.
// ─────────────────────────────────────────────────────────────────

// ── paddle hit ─────────────────────────────────────────────
/// Down from 420: the most-heard sound after the brick, and at 420 it sat
/// in the same register as the brick and fought it. Low and quiet is what
/// keeps it from becoming a nag by level three.
const PADDLE_HZ: f32 = 245.0;
const PADDLE_LEN: f32 = 0.075;
const PADDLE_LEVEL: f32 = 0.34;
const PADDLE_TONE: f32 = 0.22;
/// Pitch added per hit of a rally, and the ceiling it climbs to.
///
/// §7 asks for the paddle hit to rise with rally length. `play_with`'s
/// pitch argument already carries it, so there is no mechanism here — only
/// the curve. The cap matters more than the step: without one, a long
/// rally walks the paddle up into the brick's register and the two stop
/// being distinguishable.
const RALLY_STEP: f32 = 0.036;
const RALLY_CAP: f32 = 1.80;

// ── plain brick ────────────────────────────────────────────
/// The only sound Brian raised, and the one he lengthened most in
/// proportion (70 → 110 ms). It is heard more than everything else
/// combined, so it gets the clear space above the paddle and enough
/// length to ring rather than click.
const BRICK_HZ: f32 = 960.0;
const BRICK_LEN: f32 = 0.110;
const BRICK_LEVEL: f32 = 0.30;
const BRICK_TONE: f32 = 0.26;

// ── reinforced chip (survived a hit) ───────────────────────
/// Kept exactly as proposed. Duller and lower than a break, because the
/// job is to read as *not yet* — if a chip and a break sound alike, the
/// player cannot hear the difference between damage and destruction.
const CHIP_HZ: f32 = 500.0;
const CHIP_LEN: f32 = 0.055;
const CHIP_LEVEL: f32 = 0.24;
const CHIP_TONE: f32 = 0.46;

// ── reinforced break ───────────────────────────────────────
/// Down from 620 and nearly doubled in length. The reward for a second
/// hit, and long enough to be heard as a small event rather than a
/// louder chip.
const RBREAK_HZ: f32 = 470.0;
/// A perfect fifth. Rising, because breaking through should sound like it.
const RBREAK_INTERVAL: f32 = 1.50;
const RBREAK_GAP: f32 = 0.045;
const RBREAK_LEN: f32 = 0.220;
const RBREAK_LEVEL: f32 = 0.32;
const RBREAK_TONE: f32 = 0.34;

// ── armoured break (the heavy one) ─────────────────────────
/// §7 calls this "the heavy one". In a palette with no noise in it,
/// weight is a low fundamental and a long tail — not volume.
const ARMOUR_HZ: f32 = 195.0;
/// The octave above, and deliberately shorter than the fundamental, so
/// what rings on is the LOW note. That asymmetry is the weight.
const ARMOUR_INTERVAL: f32 = 2.00;
const ARMOUR_GAP: f32 = 0.030;
const ARMOUR_LEN: f32 = 0.300;
const ARMOUR_LEVEL: f32 = 0.40;
const ARMOUR_TONE: f32 = 0.52;

// ── power-up catch ─────────────────────────────────────────
/// Kept as proposed. Rising, because catching something good should
/// sound like it.
const ITEM_HZ: f32 = 660.0;
const ITEM_INTERVAL: f32 = 1.50;
const ITEM_GAP: f32 = 0.070;
const ITEM_LEN: f32 = 0.180;
const ITEM_LEVEL: f32 = 0.34;
const ITEM_TONE: f32 = 0.30;

// ── BOMB catch — the only warning the player gets ──────────
/// Down from 330, the largest move Brian made.
///
/// ⚠️ **The bomb does not glow** — S8b settled that a glow is an
/// invitation — so this sound is the whole warning. It is the loudest
/// thing in the game, the second longest, and the only one built on a
/// dissonance.
///
/// ⚠️ Known and accepted: `BOMB_HZ * BOMB_DISSONANCE` is 246.8 Hz, which
/// sits 1.8 Hz from `PADDLE_HZ`. There is no beating — the overlap window
/// is ~79 ms against a 556 ms beat period — but a bomb caught in the same
/// instant as a paddle hit can fuse with it. What saves it is the second
/// half: the fallen pair at 117/165 Hz is below anything the paddle makes
/// and rings ~200 ms after the paddle is gone. Moving this to ~165 would
/// clear the collision entirely at the cost of a semitone.
const BOMB_HZ: f32 = 175.0;
/// Falling, not rising: the shape is the opposite of the power-up's.
const BOMB_FALL: f32 = 0.67;
/// A tritone — the classic wrong-sounding interval. Sounded *with* each
/// note rather than after it, because simultaneous dissonance is what the
/// ear reads as wrong; a sequence would just read as a tune.
const BOMB_DISSONANCE: f32 = 1.41;
const BOMB_GAP: f32 = 0.055;
const BOMB_LEN: f32 = 0.260;
const BOMB_LEVEL: f32 = 0.42;
const BOMB_TONE: f32 = 0.62;

// ── magnet catch and release ───────────────────────────────
/// Two halves of one gesture: the catch holds, the release lets go.
/// Kept as proposed.
const MAGNET_HZ: f32 = 440.0;
const MAGNET_LEN: f32 = 0.220;
/// The release is the same note a fifth up and much shorter.
///
/// ⚠️ It also fires on the 3 s auto-release, when the player did nothing.
/// That is why it is a lift rather than a fall: an auto-release must not
/// sound like a mistake.
const MAGNET_LIFT: f32 = 1.50;
const MAGNET_RELEASE_LEN: f32 = 0.090;
const MAGNET_LEVEL: f32 = 0.30;
const MAGNET_TONE: f32 = 0.26;

// ── ball lost ──────────────────────────────────────────────
/// Brian more than doubled the timbre, 0.34 → 0.76 — the largest
/// proportional change in the whole set — and left the level alone.
///
/// That is the interesting choice here: a loss needs to *land*, and the
/// obvious way is to make it louder. Reediness instead means it cuts
/// through without being punishing, which matters for a sound the player
/// hears at least three times a game.
const LOST_HZ: f32 = 390.0;
const LOST_FALL: f32 = 0.67;
const LOST_GAP: f32 = 0.080;
const LOST_LEN: f32 = 0.240;
const LOST_LEVEL: f32 = 0.34;
const LOST_TONE: f32 = 0.76;

// ── level clear ────────────────────────────────────────────
/// Rises and resolves to the octave. The racer's finish proved that the
/// resolution is what makes an ending sound finished rather than cut off.
/// It plays under the cascade, so it can afford to be the longest sound
/// in the game.
const CLEAR_HZ: f32 = 520.0;
const CLEAR_STEP: f32 = 1.26;
const CLEAR_GAP: f32 = 0.105;
const CLEAR_LEN: f32 = 0.540;
const CLEAR_LEVEL: f32 = 0.34;
const CLEAR_TONE: f32 = 0.34;

// ── game over ──────────────────────────────────────────────
/// The mirror of level clear: the same shape descending, and it does
/// **not** resolve. Ending on the unresolved note is what makes it feel
/// like a stop rather than a finish.
const OVER_HZ: f32 = 330.0;
const OVER_STEP: f32 = 0.79;
const OVER_GAP: f32 = 0.150;
const OVER_LEN: f32 = 0.720;
const OVER_LEVEL: f32 = 0.36;
const OVER_TONE: f32 = 0.44;

// ── ★ VICTORY FANFARE — level 10 complete ──────────────────
/// The one piece of actual music in the game: I-IV-V-I, four chords,
/// the last resolving to the octave and held.
///
/// ⚠️ That progression is not a flourish — it is the most resolved
/// sequence in common practice, which is why every arcade fanfare since
/// the 70s reaches for it, and it is the same idea the level-clear sound
/// already ends on, given room to breathe.
///
/// ⚠️ Held to about four seconds and no longer. 70s arcade "music" was
/// fanfares, not songs — Space Invaders was four descending notes — and
/// a tune long enough to hum is a tune too long to hear twice.
const VICTORY_HZ: f32 = 262.0;
const VICTORY_GAP: f32 = 0.62;
const VICTORY_LEN: f32 = 0.72;
/// The last chord, held far longer than the three that set it up. This
/// is what makes the sequence an ENDING rather than a fourth chord.
const VICTORY_HOLD: f32 = 2.20;
/// ⚠️ Low because THIRTEEN notes sound nearly together here — a level
/// that suits one struck note would clip badly across a chord stack.
const VICTORY_LEVEL: f32 = 0.17;
const VICTORY_TONE: f32 = 0.38;
/// Two octaves above the resolution, quieter: the shine on top.
const VICTORY_SPARKLE: f32 = 0.45;

// ── the end-of-game tally ──────────────────────────────────
/// One line of the tally landing. Small, bright, and quick — it punctuates
/// a number rather than announcing one.
const TALLY_HZ: f32 = 1046.0;
const TALLY_LEN: f32 = 0.090;
const TALLY_LEVEL: f32 = 0.26;
const TALLY_TONE: f32 = 0.20;

/// How long the whole fanfare runs — three chords then the held
/// resolution. ⚠️ `state::VICTORY_WAVE_SECONDS` is matched to this, so
/// the cascade and the music finish together.
pub const VICTORY_SECONDS: f32 = VICTORY_GAP * 3.0 + VICTORY_HOLD;

/// The most notes any one sound uses. The victory fanfare's four triads
/// plus its sparkle.
const MAX_NOTES: usize = 13;

// ─────────────────────────────────────────────────────────────────
// The synthesis.
// ─────────────────────────────────────────────────────────────────

/// One note of a sound: when it starts, what it plays, how it sounds.
///
/// Grouped rather than passed as five loose floats, for the reason the
/// racer's `Note` gives: a call site reading `strike(out, sr, t, 0.05,
/// 470.0, 0.22, 0.32, 0.34)` tells a reader nothing about which number is
/// which.
#[derive(Clone, Copy)]
struct Note {
    /// Seconds after the sound began.
    start: f32,
    hz: f32,
    len: f32,
    level: f32,
    /// Harmonic content: 0 is a pure sine, 1 a reedy tone.
    tone: f32,
}

/// One struck note, summed into `out` starting at `t0` seconds.
///
/// Struck, not switched on: a few milliseconds of attack so there is no
/// click, then an exponential decay.
///
/// ⚠️ **This function and `strike()` in `tools/sfx/pixel-break.html` are
/// one function written twice.** Changing either without the other
/// silently decalibrates the tuning tool, and every number it produces
/// afterwards inherits the error. The playground's copy carries the same
/// warning.
fn strike(out: &mut [f32], sample_rate: f32, t0: f32, note: Note) {
    let Note { start, hz, len, level, tone } = note;
    let dt = 1.0 / sample_rate;
    for (i, sample) in out.iter_mut().enumerate() {
        let t = t0 + i as f32 * dt - start;
        if t < 0.0 || t >= len {
            continue;
        }
        // A short attack so the note is struck rather than clicked on.
        let attack = (t / 0.008).min(1.0);
        let env = attack * (-t / (len * 0.35)).exp();

        // A small harmonic stack: `tone` decides how reedy it is.
        let mut v = (TAU * hz * t).sin();
        let mut amp = tone;
        for k in 2..6 {
            if hz * k as f32 > sample_rate * 0.5 {
                break;
            }
            v += amp * (TAU * hz * k as f32 * t).sin() / k as f32;
            amp *= tone;
        }

        *sample += v * env * level;
    }
}

// ─────────────────────────────────────────────────────────────────
// The twelve sounds.
// ─────────────────────────────────────────────────────────────────

/// Which sound a voice is currently playing.
///
/// ⚠️ This is *not* [`Cue`]. A cue is what happened in the game; a
/// `Shot` is what that sounds like. They are nearly one-to-one today and
/// deliberately separate anyway, so that re-tuning what a bomb sounds
/// like never reaches into physics.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Shot {
    Paddle,
    Brick,
    Chip,
    ReinforcedBreak,
    ArmouredBreak,
    Item,
    Bomb,
    MagnetCatch,
    MagnetRelease,
    Lost,
    Clear,
    Over,
    Victory,
    Tally,
}

impl Shot {
    /// How this arrives through the mixer, which carries floats.
    ///
    /// The pitch argument does double duty: it selects the sound *and*,
    /// for the paddle, trims it. That works because the selector is an
    /// integer and the trim is a fraction — see [`Shot::from_pitch`].
    fn as_pitch(self) -> f32 {
        match self {
            Shot::Paddle => 1.0,
            Shot::Brick => 2.0,
            Shot::Chip => 3.0,
            Shot::ReinforcedBreak => 4.0,
            Shot::ArmouredBreak => 5.0,
            Shot::Item => 6.0,
            Shot::Bomb => 7.0,
            Shot::MagnetCatch => 8.0,
            Shot::MagnetRelease => 9.0,
            Shot::Lost => 10.0,
            Shot::Clear => 11.0,
            Shot::Over => 12.0,
            Shot::Victory => 13.0,
            Shot::Tally => 14.0,
        }
    }

    fn from_pitch(p: f32) -> Shot {
        match p.trunc() as i32 {
            2 => Shot::Brick,
            3 => Shot::Chip,
            4 => Shot::ReinforcedBreak,
            5 => Shot::ArmouredBreak,
            6 => Shot::Item,
            7 => Shot::Bomb,
            8 => Shot::MagnetCatch,
            9 => Shot::MagnetRelease,
            10 => Shot::Lost,
            11 => Shot::Clear,
            12 => Shot::Over,
            13 => Shot::Victory,
            14 => Shot::Tally,
            _ => Shot::Paddle,
        }
    }

    /// How long this sound lasts, so the mixer can retire it.
    fn length(self) -> f32 {
        match self {
            Shot::Paddle => PADDLE_LEN,
            Shot::Brick => BRICK_LEN,
            Shot::Chip => CHIP_LEN,
            Shot::ReinforcedBreak => RBREAK_GAP + RBREAK_LEN,
            Shot::ArmouredBreak => ARMOUR_LEN,
            Shot::Item => ITEM_GAP + ITEM_LEN,
            Shot::Bomb => BOMB_GAP + BOMB_LEN,
            Shot::MagnetCatch => MAGNET_LEN,
            Shot::MagnetRelease => MAGNET_RELEASE_LEN,
            Shot::Lost => LOST_GAP + LOST_LEN,
            Shot::Clear => CLEAR_GAP * 3.0 + CLEAR_LEN,
            Shot::Over => OVER_GAP * 2.0 + OVER_LEN,
            Shot::Victory => VICTORY_SECONDS,
            Shot::Tally => TALLY_LEN,
        }
    }
}

/// The pitch trim for the nth paddle hit of a rally.
///
/// ⚠️ Must stay identical to `rallyPitch()` in the playground, or the
/// climb Brian tuned is not the climb that ships. Pinned by a test.
fn rally_pitch(rally: u32) -> f32 {
    (1.0 + rally as f32 * RALLY_STEP).min(RALLY_CAP)
}

/// Pixel Break's voice: any one of the twelve sounds, one at a time.
pub struct Pixel {
    what: Shot,
    /// Extra pitch multiplier, used only by the paddle's rally climb.
    trim: f32,
    t: f32,
    alive: bool,
}

impl Pixel {
    pub fn new() -> Pixel {
        Pixel { what: Shot::Paddle, trim: 1.0, t: 0.0, alive: false }
    }

    /// The notes that make up one sound.
    ///
    /// Returned into a fixed-size buffer rather than a `Vec` because this
    /// is called from [`Voice::render`], on the audio thread, where
    /// allocation is forbidden.
    ///
    /// ⚠️ Sized for the VICTORY FANFARE at thirteen — four triads plus a
    /// sparkle. It was five until S10 (the level clear's ceiling), and
    /// growing it deliberately with `no_sound_overflows_the_note_buffer`
    /// updated is the point: a fourteenth note would otherwise be dropped
    /// silently, which on a chord means one voice quietly missing.
    fn notes(what: Shot, trim: f32) -> ([Note; MAX_NOTES], usize) {
        let mut n =
            [Note { start: 0.0, hz: 0.0, len: 0.0, level: 0.0, tone: 0.0 }; MAX_NOTES];
        let count = match what {
            Shot::Paddle => {
                n[0] = Note {
                    start: 0.0,
                    hz: PADDLE_HZ * trim,
                    len: PADDLE_LEN,
                    level: PADDLE_LEVEL,
                    tone: PADDLE_TONE,
                };
                1
            }
            Shot::Brick => {
                n[0] = Note {
                    start: 0.0,
                    hz: BRICK_HZ,
                    len: BRICK_LEN,
                    level: BRICK_LEVEL,
                    tone: BRICK_TONE,
                };
                1
            }
            Shot::Chip => {
                n[0] = Note {
                    start: 0.0,
                    hz: CHIP_HZ,
                    len: CHIP_LEN,
                    level: CHIP_LEVEL,
                    tone: CHIP_TONE,
                };
                1
            }
            Shot::ReinforcedBreak => {
                n[0] = Note {
                    start: 0.0,
                    hz: RBREAK_HZ,
                    len: RBREAK_LEN,
                    level: RBREAK_LEVEL,
                    tone: RBREAK_TONE,
                };
                n[1] = Note {
                    start: RBREAK_GAP,
                    hz: RBREAK_HZ * RBREAK_INTERVAL,
                    len: RBREAK_LEN,
                    level: RBREAK_LEVEL,
                    tone: RBREAK_TONE,
                };
                2
            }
            Shot::ArmouredBreak => {
                n[0] = Note {
                    start: 0.0,
                    hz: ARMOUR_HZ,
                    len: ARMOUR_LEN,
                    level: ARMOUR_LEVEL,
                    tone: ARMOUR_TONE,
                };
                // Shorter and quieter than the fundamental, so the LOW
                // note is what rings on. That asymmetry is the weight.
                n[1] = Note {
                    start: ARMOUR_GAP,
                    hz: ARMOUR_HZ * ARMOUR_INTERVAL,
                    len: ARMOUR_LEN * 0.55,
                    level: ARMOUR_LEVEL * 0.7,
                    tone: ARMOUR_TONE,
                };
                2
            }
            Shot::Item => {
                n[0] = Note {
                    start: 0.0,
                    hz: ITEM_HZ,
                    len: ITEM_LEN * 0.6,
                    level: ITEM_LEVEL,
                    tone: ITEM_TONE,
                };
                n[1] = Note {
                    start: ITEM_GAP,
                    hz: ITEM_HZ * ITEM_INTERVAL,
                    len: ITEM_LEN,
                    level: ITEM_LEVEL,
                    tone: ITEM_TONE,
                };
                2
            }
            Shot::Bomb => {
                // Each note sounded WITH its dissonant partner, not after
                // it: simultaneous dissonance is what the ear reads as
                // wrong, where a sequence would read as a tune.
                n[0] = Note {
                    start: 0.0,
                    hz: BOMB_HZ,
                    len: BOMB_LEN,
                    level: BOMB_LEVEL,
                    tone: BOMB_TONE,
                };
                n[1] = Note {
                    start: 0.0,
                    hz: BOMB_HZ * BOMB_DISSONANCE,
                    len: BOMB_LEN * 0.8,
                    level: BOMB_LEVEL * 0.75,
                    tone: BOMB_TONE,
                };
                n[2] = Note {
                    start: BOMB_GAP,
                    hz: BOMB_HZ * BOMB_FALL,
                    len: BOMB_LEN,
                    level: BOMB_LEVEL,
                    tone: BOMB_TONE,
                };
                n[3] = Note {
                    start: BOMB_GAP,
                    hz: BOMB_HZ * BOMB_FALL * BOMB_DISSONANCE,
                    len: BOMB_LEN * 0.8,
                    level: BOMB_LEVEL * 0.75,
                    tone: BOMB_TONE,
                };
                4
            }
            Shot::MagnetCatch => {
                n[0] = Note {
                    start: 0.0,
                    hz: MAGNET_HZ,
                    len: MAGNET_LEN,
                    level: MAGNET_LEVEL,
                    tone: MAGNET_TONE,
                };
                1
            }
            Shot::MagnetRelease => {
                n[0] = Note {
                    start: 0.0,
                    hz: MAGNET_HZ * MAGNET_LIFT,
                    len: MAGNET_RELEASE_LEN,
                    level: MAGNET_LEVEL,
                    tone: MAGNET_TONE,
                };
                1
            }
            Shot::Lost => {
                n[0] = Note {
                    start: 0.0,
                    hz: LOST_HZ,
                    len: LOST_LEN * 0.5,
                    level: LOST_LEVEL,
                    tone: LOST_TONE,
                };
                n[1] = Note {
                    start: LOST_GAP,
                    hz: LOST_HZ * LOST_FALL,
                    len: LOST_LEN,
                    level: LOST_LEVEL,
                    tone: LOST_TONE,
                };
                2
            }
            Shot::Clear => {
                n[0] = Note {
                    start: 0.0,
                    hz: CLEAR_HZ,
                    len: CLEAR_LEN * 0.4,
                    level: CLEAR_LEVEL,
                    tone: CLEAR_TONE,
                };
                n[1] = Note {
                    start: CLEAR_GAP,
                    hz: CLEAR_HZ * CLEAR_STEP,
                    len: CLEAR_LEN * 0.4,
                    level: CLEAR_LEVEL,
                    tone: CLEAR_TONE,
                };
                n[2] = Note {
                    start: CLEAR_GAP * 2.0,
                    hz: CLEAR_HZ * CLEAR_STEP * CLEAR_STEP,
                    len: CLEAR_LEN * 0.4,
                    level: CLEAR_LEVEL,
                    tone: CLEAR_TONE,
                };
                // Resolving to the octave is what makes it sound finished
                // rather than cut off.
                n[3] = Note {
                    start: CLEAR_GAP * 3.0,
                    hz: CLEAR_HZ * 2.0,
                    len: CLEAR_LEN,
                    level: CLEAR_LEVEL,
                    tone: CLEAR_TONE,
                };
                n[4] = Note {
                    start: CLEAR_GAP * 3.0,
                    hz: CLEAR_HZ * 3.0,
                    len: CLEAR_LEN * 0.8,
                    level: CLEAR_LEVEL * 0.5,
                    tone: CLEAR_TONE,
                };
                5
            }
            Shot::Over => {
                n[0] = Note {
                    start: 0.0,
                    hz: OVER_HZ,
                    len: OVER_LEN * 0.45,
                    level: OVER_LEVEL,
                    tone: OVER_TONE,
                };
                n[1] = Note {
                    start: OVER_GAP,
                    hz: OVER_HZ * OVER_STEP,
                    len: OVER_LEN * 0.45,
                    level: OVER_LEVEL,
                    tone: OVER_TONE,
                };
                // No octave, no resolution: it stops rather than finishes.
                n[2] = Note {
                    start: OVER_GAP * 2.0,
                    hz: OVER_HZ * OVER_STEP * OVER_STEP,
                    len: OVER_LEN,
                    level: OVER_LEVEL,
                    tone: OVER_TONE,
                };
                3
            }
            Shot::Victory => {
                // Equal temperament: a semitone is the twelfth root of two.
                let st = |semi: f32| VICTORY_HZ * 2.0f32.powf(semi / 12.0);
                let mut i = 0;
                // I - IV - V, each a triad, each landing on the beat.
                for (c, chord) in [[0.0, 4.0, 7.0], [5.0, 9.0, 12.0], [7.0, 11.0, 14.0]]
                    .iter()
                    .enumerate()
                {
                    for semi in chord {
                        n[i] = Note {
                            start: VICTORY_GAP * c as f32,
                            hz: st(*semi),
                            len: VICTORY_LEN,
                            level: VICTORY_LEVEL,
                            tone: VICTORY_TONE,
                        };
                        i += 1;
                    }
                }
                // The resolution: the octave, held.
                let at = VICTORY_GAP * 3.0;
                for semi in [12.0, 16.0, 19.0] {
                    n[i] = Note {
                        start: at,
                        hz: st(semi),
                        len: VICTORY_HOLD,
                        level: VICTORY_LEVEL,
                        tone: VICTORY_TONE,
                    };
                    i += 1;
                }
                n[i] = Note {
                    start: at,
                    hz: st(24.0),
                    len: VICTORY_HOLD * 0.7,
                    level: VICTORY_LEVEL * VICTORY_SPARKLE,
                    tone: VICTORY_TONE,
                };
                i += 1;
                i
            }
            Shot::Tally => {
                n[0] = Note {
                    start: 0.0,
                    hz: TALLY_HZ,
                    len: TALLY_LEN,
                    level: TALLY_LEVEL,
                    tone: TALLY_TONE,
                };
                1
            }
        };
        (n, count)
    }
}

impl Default for Pixel {
    fn default() -> Self {
        Pixel::new()
    }
}

impl Voice for Pixel {
    fn render(&mut self, out: &mut [f32], _params: VoiceParams, sample_rate: f32) {
        for s in out.iter_mut() {
            *s = 0.0;
        }
        if !self.alive {
            return;
        }

        let (notes, count) = Pixel::notes(self.what, self.trim);
        for note in &notes[..count] {
            strike(out, sample_rate, self.t, *note);
        }

        for s in out.iter_mut() {
            *s = s.clamp(-1.0, 1.0);
        }

        self.t += out.len() as f32 / sample_rate;
        if self.t > self.what.length() {
            self.alive = false;
        }
    }

    fn alive(&self) -> bool {
        self.alive
    }

    /// `pitch` selects the sound by its integer part and trims it by the
    /// fraction — see [`Shot::as_pitch`].
    fn retrigger(&mut self, _gain: f32, pitch: f32) {
        self.what = Shot::from_pitch(pitch);
        self.trim = pitch.fract() + 1.0;
        self.t = 0.0;
        self.alive = true;
    }
}

// ─────────────────────────────────────────────────────────────────
// The bank.
// ─────────────────────────────────────────────────────────────────

/// How many slots each group gets.
///
/// Sized by how often a group collides *with itself*, which is not the
/// same as how often it plays:
///
/// - **Hits** are the shower. At ten balls, brick hits land milliseconds
///   apart; with one slot each would cut the last one off and the effect
///   would be a stuttering machine gun exactly when the action is best.
/// - **Breaks** are rarer — a tiered brick takes two or four hits to get
///   here — but two balls can finish two bricks in the same tick.
/// - **Items** need two only because the magnet's catch and release are
///   separate sounds that can nearly touch.
/// - **Events** need one: they are mutually exclusive by construction.
///   Losing a ball, clearing a field and running out of lives cannot
///   happen together, so a second slot could never sound.
const HIT_SLOTS: usize = 4;
const BREAK_SLOTS: usize = 2;
const ITEM_SLOTS: usize = 2;
const EVENT_SLOTS: usize = 1;

/// Every voice Pixel Break registers, and whose turn it is.
///
/// ⚠️ Registration is startup-only — `AudioSystem::pending` is dropped
/// when the stream starts — so this is built once, before `run`.
pub struct Bank {
    hits: [SoundId; HIT_SLOTS],
    breaks: [SoundId; BREAK_SLOTS],
    items: [SoundId; ITEM_SLOTS],
    events: [SoundId; EVENT_SLOTS],
    next_hit: usize,
    next_break: usize,
    next_item: usize,
}

impl Bank {
    /// Register every voice. Call before [`AudioSystem::start`].
    pub fn register(audio: &mut AudioSystem) -> Bank {
        let mut one = || audio.register_sound(Box::new(Pixel::new()));
        Bank {
            hits: [one(), one(), one(), one()],
            breaks: [one(), one()],
            items: [one(), one()],
            events: [one()],
            next_hit: 0,
            next_break: 0,
            next_item: 0,
        }
    }

    /// Play what a cue sounds like.
    ///
    /// This is the whole translation layer: physics says *a brick broke*,
    /// and everything about which slot, which sound and which pitch is
    /// decided here.
    pub fn play(&mut self, audio: &mut Audio<'_>, cue: Cue) {
        let (slot, shot, trim) = match cue {
            Cue::PaddleHit { rally } => {
                (self.next_hit_slot(), Shot::Paddle, rally_pitch(rally))
            }
            Cue::PlainBreak => (self.next_hit_slot(), Shot::Brick, 1.0),
            Cue::ReinforcedChip => (self.next_hit_slot(), Shot::Chip, 1.0),
            Cue::ReinforcedBreak => {
                (self.next_break_slot(), Shot::ReinforcedBreak, 1.0)
            }
            Cue::ArmouredBreak => (self.next_break_slot(), Shot::ArmouredBreak, 1.0),
            Cue::ItemCaught => (self.next_item_slot(), Shot::Item, 1.0),
            Cue::BombCaught => (self.next_item_slot(), Shot::Bomb, 1.0),
            Cue::MagnetCatch => (self.next_item_slot(), Shot::MagnetCatch, 1.0),
            Cue::MagnetRelease => (self.next_item_slot(), Shot::MagnetRelease, 1.0),
            Cue::BallLost => (self.events[0], Shot::Lost, 1.0),
            Cue::LevelClear => (self.events[0], Shot::Clear, 1.0),
            Cue::GameOver => (self.events[0], Shot::Over, 1.0),
            // ⚠️ The fanfare takes the EVENT slot and the tally takes an
            // ITEM slot, deliberately: the tally chimes land WHILE the
            // fanfare's last chord is still ringing, and sharing one slot
            // would have each chime cut the music off.
            Cue::Victory => (self.events[0], Shot::Victory, 1.0),
            Cue::TallyLine => (self.next_item_slot(), Shot::Tally, 1.0),
        };
        // The selector is the integer part and the trim the fraction, so
        // one float carries both. `trim - 1.0` because a trim of 1.0 must
        // leave the integer alone.
        audio.play_with(slot, 1.0, shot.as_pitch() + (trim - 1.0));
    }

    fn next_hit_slot(&mut self) -> SoundId {
        let id = self.hits[self.next_hit];
        self.next_hit = (self.next_hit + 1) % HIT_SLOTS;
        id
    }

    fn next_break_slot(&mut self) -> SoundId {
        let id = self.breaks[self.next_break];
        self.next_break = (self.next_break + 1) % BREAK_SLOTS;
        id
    }

    fn next_item_slot(&mut self) -> SoundId {
        let id = self.items[self.next_item];
        self.next_item = (self.next_item + 1) % ITEM_SLOTS;
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(what: Shot, trim: f32, n: usize) -> Vec<f32> {
        let mut v = Pixel::new();
        v.retrigger(1.0, what.as_pitch() + (trim - 1.0));
        let mut out = vec![0.0; n];
        v.render(&mut out, VoiceParams::default(), 48_000.0);
        out
    }

    fn peak(out: &[f32]) -> f32 {
        out.iter().fold(0.0f32, |a, b| a.max(b.abs()))
    }

    const ALL: [Shot; 14] = [
        Shot::Paddle,
        Shot::Brick,
        Shot::Chip,
        Shot::ReinforcedBreak,
        Shot::ArmouredBreak,
        Shot::Item,
        Shot::Bomb,
        Shot::MagnetCatch,
        Shot::MagnetRelease,
        Shot::Lost,
        Shot::Clear,
        Shot::Over,
        Shot::Victory,
        Shot::Tally,
    ];

    #[test]
    fn every_sound_makes_sound() {
        for what in ALL {
            let out = render(what, 1.0, 48_000);
            assert!(peak(&out) > 0.01, "{what:?} was silent");
        }
    }

    #[test]
    fn every_sound_stays_inside_full_scale() {
        for what in ALL {
            let out = render(what, 1.0, 48_000);
            assert!(peak(&out) <= 1.0, "{what:?} clipped at {}", peak(&out));
        }
    }

    /// The selector survives the round trip, including the paddle's
    /// fractional trim riding on the same float.
    #[test]
    fn the_pitch_argument_selects_the_right_sound() {
        for what in ALL {
            assert_eq!(Shot::from_pitch(what.as_pitch()), what, "{what:?} plain");
        }
        // A paddle at full rally cap is 1.0 + 0.8 = 1.8, which must still
        // read as Paddle and not round up into Brick.
        assert_eq!(Shot::from_pitch(1.0 + (RALLY_CAP - 1.0)), Shot::Paddle);
    }

    /// ⚠️ Pins the climb against the playground's `rallyPitch()`. If this
    /// fails after a retune, the tool and the game have drifted apart.
    #[test]
    fn the_rally_climbs_and_then_stops() {
        assert!((rally_pitch(0) - 1.0).abs() < 1e-6, "a first hit is untrimmed");
        assert!((rally_pitch(1) - (1.0 + RALLY_STEP)).abs() < 1e-6);
        assert!((rally_pitch(10) - (1.0 + 10.0 * RALLY_STEP)).abs() < 1e-6);
        // The cap is what stops a long rally walking up into the brick's
        // register, so it matters more than the step.
        assert!((rally_pitch(1000) - RALLY_CAP).abs() < 1e-6, "the climb must cap");
    }

    #[test]
    fn a_paddle_hit_actually_rises() {
        let low = render(Shot::Paddle, rally_pitch(0), 4_800);
        let high = render(Shot::Paddle, rally_pitch(20), 4_800);
        // Count zero crossings as a cheap pitch proxy: a higher note
        // crosses zero more often over the same window.
        let cross = |v: &[f32]| {
            v.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count()
        };
        assert!(
            cross(&high) > cross(&low),
            "a long rally must sound higher: {} vs {}",
            cross(&high),
            cross(&low)
        );
    }

    #[test]
    fn a_one_shot_retires_itself() {
        let mut v = Pixel::new();
        v.retrigger(1.0, Shot::Paddle.as_pitch());
        assert!(v.alive(), "a triggered sound is alive");
        let mut out = vec![0.0; 48_000];
        v.render(&mut out, VoiceParams::default(), 48_000.0);
        assert!(!v.alive(), "a spent one-shot must retire");
        // And must then cost nothing.
        let mut again = vec![0.0; 128];
        v.render(&mut again, VoiceParams::default(), 48_000.0);
        assert!(again.iter().all(|s| *s == 0.0), "a retired voice must be silent");
    }

    /// ⚠️ §7's named test, and the one that cost the most in Omaprix.
    ///
    /// A one-shot must be sized against whatever it plays over, not
    /// against silence. Ten balls is genuinely reachable — S6 added the
    /// Omarchy item and S8b made it easier to catch — so this sums a
    /// plausible ten-ball frame and checks that no sound is buried.
    ///
    /// "Can be heard" is measured as: the sound's own peak must be at
    /// least a fifth of the loudest thing in the mix. Below that it is
    /// masked in practice, whatever a spectrum says.
    #[test]
    fn every_sound_can_be_heard_at_ten_balls() {
        // The mix a dense ten-ball field actually makes: a shower of
        // brick hits with paddle hits underneath.
        let mut bed = vec![0.0f32; 24_000];
        for i in 0..26 {
            let at = i as f32 * 0.02;
            let (notes, count) = Pixel::notes(Shot::Brick, 1.0);
            for note in &notes[..count] {
                strike(&mut bed, 48_000.0, -at, *note);
            }
        }
        for i in 0..5 {
            let at = 0.05 + i as f32 * 0.09;
            let (notes, count) = Pixel::notes(Shot::Paddle, rally_pitch(8));
            for note in &notes[..count] {
                strike(&mut bed, 48_000.0, -at, *note);
            }
        }
        let bed_peak = peak(&bed);

        for what in ALL {
            let out = render(what, 1.0, 24_000);
            let ratio = peak(&out) / bed_peak;
            assert!(
                ratio >= 0.2,
                "{what:?} peaks at {:.3} against a {:.3} bed (ratio {ratio:.3}) \
                 — it would be buried at ten balls",
                peak(&out),
                bed_peak
            );
        }
    }

    /// The bomb is the one sound §7 says must carry meaning the visual
    /// might miss, because a bomb does not glow. It must therefore be
    /// louder than the good catch it could be confused with.
    #[test]
    fn the_bomb_is_louder_than_a_good_catch() {
        let bomb = peak(&render(Shot::Bomb, 1.0, 24_000));
        let item = peak(&render(Shot::Item, 1.0, 24_000));
        assert!(bomb > item, "the bomb ({bomb:.3}) must cut through the item ({item:.3})");
    }

    /// A chip must not be mistakable for a break, or the player cannot
    /// hear the difference between damaging a brick and destroying it.
    #[test]
    fn a_chip_is_quieter_and_shorter_than_a_break() {
        assert!(CHIP_LEVEL < RBREAK_LEVEL, "a chip must be the quieter of the two");
        assert!(
            Shot::Chip.length() < Shot::ReinforcedBreak.length(),
            "a chip must be the shorter of the two"
        );
    }

    /// Every sound must fit the buffer `notes` returns, or a future sound
    /// with six notes would silently lose one.
    #[test]
    fn no_sound_overflows_the_note_buffer() {
        for what in ALL {
            let (_, count) = Pixel::notes(what, 1.0);
            assert!(
                count <= MAX_NOTES,
                "{what:?} needs {count} notes, the buffer holds {MAX_NOTES}"
            );
            assert!(count >= 1, "{what:?} has no notes at all");
        }
    }

    /// `length()` must cover every note, or the mixer retires a voice
    /// while it is still sounding and the tail is cut off.
    #[test]
    fn every_sound_outlives_its_longest_note() {
        for what in ALL {
            let (notes, count) = Pixel::notes(what, 1.0);
            let last = notes[..count]
                .iter()
                .fold(0.0f32, |m, n| m.max(n.start + n.len));
            assert!(
                what.length() >= last - 1e-6,
                "{what:?} lasts {} but its notes run to {last}",
                what.length()
            );
        }
    }
}
