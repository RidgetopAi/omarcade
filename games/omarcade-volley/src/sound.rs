//! Volley's voice: five struck sounds, clean-synth arcade.
//!
//! # Where these numbers came from
//!
//! ⚠️ **They are a starting point, not Brian's ear.** Pixel Break's twelve
//! sounds were tuned by hand in `tools/sfx/pixel-break.html` on 2026-09-09
//! and its comments record what he moved. These five have not had that
//! pass yet. They are derived from his tuning rather than guessed — the
//! paddle hit is his paddle hit, and the rest are placed around the hole
//! he carved in the middle of the register — but a derived number is not
//! an approved one. `tools/sfx/volley.html` exists so they can be moved.
//!
//! # Why the playground's numbers can be trusted here
//!
//! Because [`strike`] below and `strike()` in `tools/sfx/volley.html` are
//! the same function, ported unchanged from Pixel Break's pair, which was
//! verified sample-for-sample rather than by ear.
//!
//! That check is not ceremony. The racer's `chimes.html` does **not** hold
//! against its Rust — different harmonic series, an exponential attack
//! where the Rust is linear, a decay ending at 0.0001 against 0.057 — so a
//! level of 0.32 there was never a level of 0.32 in the game. It never
//! mattered because chimes play alone. It would matter here, because a
//! long rally puts the paddle hit against the wall bounce repeatedly, and
//! that is a claim about *relative* levels.
//!
//! # Why five sounds share one voice
//!
//! A registered one-shot is a single slot, and playing it again calls
//! [`Voice::retrigger`] — it restarts, it does not layer. So slots are
//! grouped by how often a sound collides *with itself*, and selected
//! within a group by the pitch argument: the idiom the racer's `Chime`
//! established and Pixel Break's `Bank` generalised.
//!
//! # What is different about Volley
//!
//! Breakout's soundscape is a shower — ten balls, dozens of bricks. This
//! one is a **conversation**: two sounds alternating, for a long time. A
//! 59-return rally plays the paddle hit 59 times with nothing else between
//! but the wall. That changes what matters:
//!
//! - The paddle hit must stay interesting across a very long rally, which
//!   is what [`rally_pitch`] is for. Pixel Break has the same climb, but a
//!   Breakout rally is short and rarely reaches the cap. Here the cap is
//!   the sound most of a good rally is spent at, so it is the number most
//!   worth Brian's ear.
//! - The wall bounce must sit *under* the paddle hit. It is the more
//!   frequent of the two and carries less information — the player already
//!   knows the ball hit a wall, because they watched it.
//!
//! ⚠️ [`Voice::render`] runs on the audio thread: no allocation, no
//! locking, no panicking. Everything here is arithmetic over owned state.

use std::f32::consts::TAU;

use omarcade_core::{Audio, AudioSystem, SoundId, Voice, VoiceParams};

use crate::state::Cue;

// ─────────────────────────────────────────────────────────────────
// THE TUNING. Change these by ear, in the playground, not here.
// ─────────────────────────────────────────────────────────────────

// ── paddle hit ─────────────────────────────────────────────
/// Brian's tuned Pixel Break paddle hit, unchanged: 245 Hz, low and quiet
/// so the most-heard sound in the game cannot become a nag.
///
/// It carries over exactly rather than being re-derived because it is the
/// same event — a ball coming off a paddle — and it is the one number here
/// that has already survived his ear over several play sessions.
const PADDLE_HZ: f32 = 245.0;
const PADDLE_LEN: f32 = 0.075;
const PADDLE_LEVEL: f32 = 0.34;
const PADDLE_TONE: f32 = 0.22;

/// Pitch added per return of a rally, and the ceiling it climbs to.
///
/// ⚠️ **The cap matters more here than it does in Pixel Break.** A Breakout
/// rally is a handful of hits and rarely approaches the ceiling; a Volley
/// rally of 59 sits at it for most of its length, so the cap is not an
/// edge case but the sound of a good rally.
///
/// The step is gentler than Breakout's 0.036 for the same reason: over 59
/// returns that would hit the ceiling by return 22 and spend the other 37
/// flat. At 0.012 the climb is still arriving at the cap around return 50,
/// so the rally keeps rising for as long as a very good one lasts.
const RALLY_STEP: f32 = 0.012;
/// 1.7 rather than Breakout's 1.8: a touch lower because the climb here is
/// heard in isolation against the wall bounce rather than buried in a
/// brick shower, and the top of Breakout's range is bright when exposed.
const RALLY_CAP: f32 = 1.70;

// ── wall bounce ────────────────────────────────────────────
/// Below the paddle hit and quieter. The rule Brian's tuning established
/// is that the most-heard sound gets the clear middle and everything else
/// keeps out of its way — and in a rally the wall is heard *more* than the
/// paddle while saying less.
///
/// 165 Hz is deliberately the number he moved the bomb to, which is the
/// one pitch in this register already known to sit clear of 245.
const WALL_HZ: f32 = 165.0;
/// Short. A wall bounce is a tick, not a note: at the ramped speeds a long
/// rally reaches, two bounces can land within 120 ms of each other.
const WALL_LEN: f32 = 0.045;
/// Well under the paddle's 0.34. It is the ball's footstep, not an event.
const WALL_LEVEL: f32 = 0.17;
/// Nearly pure. Harmonics here would reach up into the paddle's register,
/// which is the one place this sound must not go.
const WALL_TONE: f32 = 0.10;

// ── serve ──────────────────────────────────────────────────
/// Two notes rising: the ball is in play. Above the rally's register so it
/// reads as a beginning rather than as another return.
const SERVE_HZ: f32 = 392.0;
const SERVE_LEN: f32 = 0.09;
const SERVE_LEVEL: f32 = 0.24;
const SERVE_TONE: f32 = 0.18;
/// The second note, a fourth above, and when it lands.
const SERVE_UP: f32 = 4.0 / 3.0;
const SERVE_GAP: f32 = 0.055;

// ── point won / lost ───────────────────────────────────────
/// A point is two notes. Won rises, lost falls — the oldest convention in
/// the medium, and the one thing here that needs no explanation.
///
/// ⚠️ One `Shot` with a direction rather than two sounds: they are the
/// same gesture inverted, and writing them separately invites them to
/// drift apart under tuning until winning and losing stop being a matched
/// pair.
const POINT_HZ: f32 = 330.0;
const POINT_LEN: f32 = 0.13;
const POINT_LEVEL: f32 = 0.30;
const POINT_TONE: f32 = 0.20;
/// The interval the second note moves by — up when won, down when lost.
const POINT_INTERVAL: f32 = 3.0 / 2.0;
const POINT_GAP: f32 = 0.10;

// ── ★ THE MATCH WON — the fanfare ──────────────────────────
/// I-IV-V-I: four chords, the last resolving to the octave and held.
///
/// ⚠️ This replaced a three-note rising triad that ran 0.58 s. Brian
/// played it and said the win was "too short and not enough reward" —
/// and the comparison he reached for was Pixel Break, whose victory runs
/// 4.06 s. He was right, and the fix is not "make it louder": it is that
/// a sequence needs a RESOLUTION to read as an ending, and three notes
/// climbing away from the root never resolve. They stop.
///
/// ⚠️ The progression is not a flourish — it is the most resolved
/// sequence in common practice, which is why every arcade fanfare since
/// the 70s reaches for it. Pixel Break's victory is the same four chords
/// and this is deliberately the same music, because the suite should
/// sound like one cabinet rather than three unrelated games.
///
/// ⚠️ Held to about four seconds and no longer. 70s arcade "music" was
/// fanfares, not songs — Space Invaders was four descending notes — and
/// a tune long enough to hum is a tune too long to hear twice. That
/// matters more in Volley than in Breakout: a match takes a couple of
/// minutes and can be replayed with ENTER, so this is heard far more
/// often than a ten-level playthrough's ending.
const MATCH_HZ: f32 = 262.0;
/// One beat. Three of these set the progression up.
///
/// ⚠️ Public because the win screen's staged reveal is timed to it — the
/// lines land ON the chords. Deriving the screen from the music means
/// retuning the fanfare moves both together rather than leaving them to
/// drift apart silently.
pub const MATCH_BEAT: f32 = 0.52;
const MATCH_GAP: f32 = MATCH_BEAT;
const MATCH_LEN: f32 = 0.60;
/// The last chord, held far longer than the three that set it up. ★ THIS
/// IS THE "TA-DA" — the held resolution is the entire difference between
/// a flourish and an ending, and it is what the old three-note version
/// had no room for.
const MATCH_HOLD: f32 = 2.00;
/// ⚠️ Low because THIRTEEN notes sound nearly together here. A level that
/// suits one struck note clips badly across a chord stack — the same
/// reason Pixel Break's fanfare sits at 0.17 while its paddle hit is at
/// 0.34. Pinned by `a_rendered_sound_is_audible_and_never_clips`.
const MATCH_LEVEL: f32 = 0.17;
const MATCH_TONE: f32 = 0.38;
/// Two octaves above the resolution, quieter: the shine on top.
const MATCH_SPARKLE: f32 = 0.45;

/// How long the whole fanfare runs — three chords then the held
/// resolution. ⚠️ `state::VICTORY_SECONDS` is matched to this so the
/// screen and the music finish together.
pub const MATCH_WON_SECONDS: f32 = MATCH_GAP * 3.0 + MATCH_HOLD;

// ── the match lost ─────────────────────────────────────────
/// ⚠️ Deliberately still SHORT, and left as it was.
///
/// Losing does not get a ceremony. Stretching this to match the win
/// would not be generous, it would be dwelling — the player wants to hit
/// ENTER and go again, and a four-second sequence standing between them
/// and that is a punishment rather than an ending.
const LOST_HZ: f32 = 262.0;
const LOST_LEN: f32 = 0.26;
const LOST_LEVEL: f32 = 0.34;
const LOST_TONE: f32 = 0.24;
const LOST_GAP: f32 = 0.16;
/// A major triad, read downwards. It keeps the major third rather than
/// going minor: this is an arcade cabinet, not a requiem, and the falling
/// direction already carries the meaning.
const LOST_STEPS: [f32; 3] = [1.0, 5.0 / 4.0, 3.0 / 2.0];

/// The most notes any one sound uses — the fanfare's four triads plus
/// its sparkle.
///
/// ⚠️ Grown from 3 with the fanfare. A fourteenth note would be dropped
/// SILENTLY, which on a chord means one voice quietly missing rather than
/// an error, so `no_sound_overflows_the_note_buffer` pins this against
/// every shot rather than trusting the comment.
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
/// ⚠️ **This function, `strike()` in `tools/sfx/volley.html` and Pixel
/// Break's copy are one function written three times.** Changing any one
/// without the others silently decalibrates the tuning tool, and every
/// number it produces afterwards inherits the error. Both playgrounds
/// carry the same warning. Pinned against Pixel Break's by a test.
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
// The five sounds.
// ─────────────────────────────────────────────────────────────────

/// Which sound a voice is currently playing.
///
/// ⚠️ This is *not* [`Cue`]. A cue is what happened in the game; a `Shot`
/// is what that sounds like. They are nearly one-to-one today and
/// deliberately separate anyway, so that re-tuning what a lost point
/// sounds like never reaches into physics.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Shot {
    Paddle,
    Wall,
    Serve,
    PointWon,
    PointLost,
    MatchWon,
    MatchLost,
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
            Shot::Wall => 2.0,
            Shot::Serve => 3.0,
            Shot::PointWon => 4.0,
            Shot::PointLost => 5.0,
            Shot::MatchWon => 6.0,
            Shot::MatchLost => 7.0,
        }
    }

    fn from_pitch(p: f32) -> Shot {
        match p.trunc() as i32 {
            2 => Shot::Wall,
            3 => Shot::Serve,
            4 => Shot::PointWon,
            5 => Shot::PointLost,
            6 => Shot::MatchWon,
            7 => Shot::MatchLost,
            _ => Shot::Paddle,
        }
    }

    /// How long this sound lasts, so the mixer can retire it.
    fn length(self) -> f32 {
        match self {
            Shot::Paddle => PADDLE_LEN,
            Shot::Wall => WALL_LEN,
            Shot::Serve => SERVE_GAP + SERVE_LEN,
            Shot::PointWon | Shot::PointLost => POINT_GAP + POINT_LEN,
            Shot::MatchWon => MATCH_WON_SECONDS,
            Shot::MatchLost => LOST_GAP * 2.0 + LOST_LEN,
        }
    }
}

/// The pitch trim for the nth return of a rally.
///
/// ⚠️ Must stay identical to `rallyPitch()` in the playground, or the
/// climb that gets tuned is not the climb that ships. Pinned by a test.
fn rally_pitch(rally: u32) -> f32 {
    (1.0 + rally as f32 * RALLY_STEP).min(RALLY_CAP)
}

/// Volley's voice: any one of the five sounds, one at a time.
pub struct Struck {
    what: Shot,
    /// Extra pitch multiplier, used only by the paddle's rally climb.
    trim: f32,
    t: f32,
    alive: bool,
}

impl Struck {
    pub fn new() -> Struck {
        Struck { what: Shot::Paddle, trim: 1.0, t: 0.0, alive: false }
    }

    /// The notes that make up one sound.
    ///
    /// Returned into a fixed-size buffer rather than a `Vec` because this
    /// is called from [`Voice::render`], on the audio thread, where
    /// allocation is forbidden.
    ///
    /// ⚠️ Sized at [`MAX_NOTES`] for the match-over triad. A fourth note
    /// would be dropped SILENTLY, which on a chord means one voice quietly
    /// missing — so `no_sound_overflows_the_note_buffer` pins the size
    /// against every shot rather than trusting this comment.
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
            Shot::Wall => {
                n[0] = Note {
                    start: 0.0,
                    hz: WALL_HZ,
                    len: WALL_LEN,
                    level: WALL_LEVEL,
                    tone: WALL_TONE,
                };
                1
            }
            Shot::Serve => {
                n[0] = Note {
                    start: 0.0,
                    hz: SERVE_HZ,
                    len: SERVE_LEN,
                    level: SERVE_LEVEL,
                    tone: SERVE_TONE,
                };
                n[1] = Note {
                    start: SERVE_GAP,
                    hz: SERVE_HZ * SERVE_UP,
                    len: SERVE_LEN,
                    level: SERVE_LEVEL,
                    tone: SERVE_TONE,
                };
                2
            }
            Shot::PointWon | Shot::PointLost => {
                // Same gesture, inverted. The second note goes up for a
                // point won and down for one lost.
                let second = if what == Shot::PointWon {
                    POINT_HZ * POINT_INTERVAL
                } else {
                    POINT_HZ / POINT_INTERVAL
                };
                n[0] = Note {
                    start: 0.0,
                    hz: POINT_HZ,
                    len: POINT_LEN,
                    level: POINT_LEVEL,
                    tone: POINT_TONE,
                };
                n[1] = Note {
                    start: POINT_GAP,
                    hz: second,
                    len: POINT_LEN,
                    level: POINT_LEVEL,
                    tone: POINT_TONE,
                };
                2
            }
            Shot::MatchWon => {
                // Equal temperament: a semitone is the twelfth root of two.
                let st = |semi: f32| MATCH_HZ * 2.0f32.powf(semi / 12.0);
                let mut i = 0;
                // I - IV - V, each a triad, each landing on the beat.
                for (c, chord) in [[0.0, 4.0, 7.0], [5.0, 9.0, 12.0], [7.0, 11.0, 14.0]]
                    .iter()
                    .enumerate()
                {
                    for semi in chord {
                        n[i] = Note {
                            start: MATCH_GAP * c as f32,
                            hz: st(*semi),
                            len: MATCH_LEN,
                            level: MATCH_LEVEL,
                            tone: MATCH_TONE,
                        };
                        i += 1;
                    }
                }
                // ★ The resolution: the octave, held. This is the ending.
                let at = MATCH_GAP * 3.0;
                for semi in [12.0, 16.0, 19.0] {
                    n[i] = Note {
                        start: at,
                        hz: st(semi),
                        len: MATCH_HOLD,
                        level: MATCH_LEVEL,
                        tone: MATCH_TONE,
                    };
                    i += 1;
                }
                // The shine on top, two octaves above and quieter.
                n[i] = Note {
                    start: at,
                    hz: st(24.0),
                    len: MATCH_HOLD * 0.7,
                    level: MATCH_LEVEL * MATCH_SPARKLE,
                    tone: MATCH_TONE,
                };
                i += 1;
                i
            }
            Shot::MatchLost => {
                // The triad, read downwards. Short on purpose — see
                // LOST_STEPS.
                for (i, step) in LOST_STEPS.iter().rev().enumerate() {
                    n[i] = Note {
                        start: LOST_GAP * i as f32,
                        hz: LOST_HZ * step,
                        len: LOST_LEN,
                        level: LOST_LEVEL,
                        tone: LOST_TONE,
                    };
                }
                LOST_STEPS.len()
            }
        };
        (n, count)
    }
}

impl Default for Struck {
    fn default() -> Self {
        Struck::new()
    }
}

impl Voice for Struck {
    fn render(&mut self, out: &mut [f32], _params: VoiceParams, sample_rate: f32) {
        for s in out.iter_mut() {
            *s = 0.0;
        }
        if !self.alive {
            return;
        }

        let (notes, count) = Struck::notes(self.what, self.trim);
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
/// - **Rally** covers the paddle hit and the wall bounce — the two sounds
///   a long rally is made of. Two slots, because at the speeds the
///   overtime ramp reaches, a wall bounce and a paddle hit can land within
///   a few milliseconds of each other, and one slot would have the second
///   cut the first off. That is the stutter this game would notice most,
///   since it happens during the best rallies rather than the worst.
/// - **Events** need one: a serve, a point and a match end are mutually
///   exclusive by construction. A point moves the phase to `Serve`, and a
///   serve cannot happen in the same tick as the point that caused it, so
///   a second slot could never sound.
const RALLY_SLOTS: usize = 2;
const EVENT_SLOTS: usize = 1;

/// Every voice Volley registers, and whose turn it is.
///
/// ⚠️ Registration is startup-only — `AudioSystem::pending` is dropped
/// when the stream starts — so this is built once, before `run`.
pub struct Bank {
    rally: [SoundId; RALLY_SLOTS],
    events: [SoundId; EVENT_SLOTS],
    next_rally: usize,
}

impl Bank {
    /// Register every voice. Call before [`AudioSystem::start`].
    pub fn register(audio: &mut AudioSystem) -> Bank {
        let mut one = || audio.register_sound(Box::new(Struck::new()));
        Bank { rally: [one(), one()], events: [one()], next_rally: 0 }
    }

    /// Play what a cue sounds like.
    ///
    /// This is the whole translation layer: physics says *the ball came
    /// off a paddle*, and everything about which slot, which sound and
    /// which pitch is decided here.
    pub fn play(&mut self, audio: &mut Audio<'_>, cue: Cue) {
        let (slot, shot, trim) = match cue {
            Cue::PaddleHit { rally } => {
                (self.next_rally_slot(), Shot::Paddle, rally_pitch(rally))
            }
            Cue::WallBounce => (self.next_rally_slot(), Shot::Wall, 1.0),
            Cue::Serve => (self.events[0], Shot::Serve, 1.0),
            Cue::Point { to_player: true } => (self.events[0], Shot::PointWon, 1.0),
            Cue::Point { to_player: false } => (self.events[0], Shot::PointLost, 1.0),
            Cue::MatchOver { won: true } => (self.events[0], Shot::MatchWon, 1.0),
            Cue::MatchOver { won: false } => (self.events[0], Shot::MatchLost, 1.0),
        };
        // The selector is the integer part and the trim the fraction, so
        // one float carries both. `trim - 1.0` because a trim of 1.0 must
        // leave the integer alone.
        audio.play_with(slot, 1.0, shot.as_pitch() + (trim - 1.0));
    }

    fn next_rally_slot(&mut self) -> SoundId {
        let id = self.rally[self.next_rally];
        self.next_rally = (self.next_rally + 1) % RALLY_SLOTS;
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Shot; 7] = [
        Shot::Paddle,
        Shot::Wall,
        Shot::Serve,
        Shot::PointWon,
        Shot::PointLost,
        Shot::MatchWon,
        Shot::MatchLost,
    ];

    #[test]
    fn no_sound_overflows_the_note_buffer() {
        // The buffer is fixed because render runs on the audio thread. An
        // extra note would be dropped silently — on a chord, one voice
        // quietly missing rather than an error.
        for shot in ALL {
            let (_, count) = Struck::notes(shot, 1.0);
            assert!(count <= MAX_NOTES, "{shot:?} wants {count} notes");
            assert!(count > 0, "{shot:?} makes no sound at all");
        }
    }

    #[test]
    fn every_shot_survives_the_round_trip_through_the_mixer() {
        // The mixer carries a float, not an enum. A shot that does not
        // come back is a sound that plays as something else.
        for shot in ALL {
            assert_eq!(Shot::from_pitch(shot.as_pitch()), shot, "{shot:?}");
        }
    }

    #[test]
    fn a_rally_trim_does_not_select_a_different_sound() {
        // The paddle's climb rides in the FRACTION of the same float that
        // selects the sound. If a trim ever reached 1.0 it would roll the
        // selector into the next shot — the paddle hit would turn into a
        // wall bounce partway through a long rally.
        for rally in [0u32, 1, 10, 50, 100, 10_000] {
            let trim = rally_pitch(rally);
            let pitch = Shot::Paddle.as_pitch() + (trim - 1.0);
            assert_eq!(
                Shot::from_pitch(pitch),
                Shot::Paddle,
                "rally {rally} selected the wrong sound at pitch {pitch}"
            );
        }
    }

    #[test]
    fn the_rally_climb_rises_and_then_stops() {
        assert_eq!(rally_pitch(0), 1.0, "an unreturned serve is untrimmed");
        assert!(rally_pitch(10) > rally_pitch(1), "it climbs");
        assert!(rally_pitch(50) > rally_pitch(10), "still climbing at 50");
        assert_eq!(rally_pitch(10_000), RALLY_CAP, "and it is bounded");
    }

    #[test]
    fn the_climb_is_still_rising_through_a_long_rally() {
        // Brian's best rally is 59. The whole point of the gentler step is
        // that a rally that good should still be going up near its end
        // rather than having flattened out early — if this fails, the step
        // is too coarse and most of a great rally sounds identical.
        assert!(
            rally_pitch(50) > rally_pitch(40),
            "a 50-return rally must not already be at the ceiling"
        );
    }

    #[test]
    fn the_wall_sits_below_the_paddle() {
        // The rule Brian's Pixel Break tuning established: the sound heard
        // most gets the clear middle, everything else keeps out of its
        // way. In a rally the wall is heard more than the paddle while
        // saying less, so it must be lower AND quieter.
        assert!(WALL_HZ < PADDLE_HZ, "the wall must not crowd the paddle");
        assert!(WALL_LEVEL < PADDLE_LEVEL, "and must sit under it");
    }

    #[test]
    fn the_climbing_paddle_never_reaches_the_wall_from_above() {
        // The paddle climbs; the wall does not. They approach from
        // opposite directions and must not meet — a return that sounded
        // like a wall bounce would be actively misleading.
        let top = PADDLE_HZ * RALLY_CAP;
        assert!(top > WALL_HZ, "sanity: the paddle starts above the wall");
        assert!(
            PADDLE_HZ * rally_pitch(0) > WALL_HZ,
            "even untrimmed, the paddle stays above the wall"
        );
    }

    #[test]
    fn winning_and_losing_are_mirror_images() {
        let (won, wc) = Struck::notes(Shot::PointWon, 1.0);
        let (lost, lc) = Struck::notes(Shot::PointLost, 1.0);
        assert_eq!(wc, lc, "the same gesture, inverted");
        assert_eq!(won[0].hz, lost[0].hz, "both open on the same note");
        assert!(won[1].hz > won[0].hz, "a point won rises");
        assert!(lost[1].hz < lost[0].hz, "a point lost falls");
    }

    #[test]
    fn a_lost_match_falls() {
        let (lost, n) = Struck::notes(Shot::MatchLost, 1.0);
        for i in 1..n {
            assert!(lost[i].hz < lost[i - 1].hz, "the losing triad falls");
        }
    }

    #[test]
    fn the_fanfare_resolves_rather_than_just_stopping() {
        // ★ The reason the win was rebuilt. Brian played the old
        // three-note version and said it was "too short and not enough
        // reward". The fix is not volume — it is that a sequence needs a
        // RESOLUTION to read as an ending. Three notes climbing away from
        // the root never resolve; they stop.
        //
        // So: the last chord must RETURN to the root (up an octave), and
        // it must be HELD far longer than the chords that set it up.
        let (n, count) = Struck::notes(Shot::MatchWon, 1.0);
        let notes = &n[..count];

        let last_start = notes.iter().map(|x| x.start).fold(0.0f32, f32::max);
        let opening: Vec<&Note> = notes.iter().filter(|x| x.start == 0.0).collect();
        let closing: Vec<&Note> = notes.iter().filter(|x| x.start == last_start).collect();

        assert!(opening.len() >= 3, "the fanfare opens on a chord");
        assert!(closing.len() >= 3, "and closes on one");

        // The resolution is the opening root, an octave up.
        let root = opening.iter().map(|x| x.hz).fold(f32::MAX, f32::min);
        let resolved = closing.iter().map(|x| x.hz).fold(f32::MAX, f32::min);
        let ratio = resolved / root;
        assert!(
            (ratio - 2.0).abs() < 0.02,
            "the last chord must resolve to the octave, got {ratio:.3}x"
        );

        // ★ And it must be HELD. This is the "ta-da" — without it the
        // fourth chord is just a fourth chord.
        let held = closing.iter().map(|x| x.len).fold(0.0f32, f32::max);
        let setup = opening.iter().map(|x| x.len).fold(0.0f32, f32::max);
        assert!(
            held > setup * 2.0,
            "the resolution must be held: {held}s against a {setup}s chord"
        );
    }

    #[test]
    fn the_win_is_a_real_ending_and_the_loss_is_not() {
        // The asymmetry is the point, and it is easy to "tidy up" by
        // accident later. Losing does not get a ceremony: the player
        // wants to hit ENTER and go again, and a four-second sequence
        // between them and that is a punishment rather than an ending.
        let won = Shot::MatchWon.length();
        let lost = Shot::MatchLost.length();
        assert!(
            won > lost * 3.0,
            "the win should dwarf the loss: {won}s against {lost}s"
        );
        // Long enough to be an ending, short enough to hear twice. 70s
        // arcade music was fanfares, not songs.
        assert!((3.0..=5.0).contains(&won), "the fanfare runs {won}s");
        assert!(lost < 1.0, "the loss runs {lost}s");
    }

    #[test]
    fn every_sound_ends() {
        // `length` retires the voice. A shot whose notes outlast it would
        // be cut off; one that outlasts its notes holds a slot silently,
        // and that slot is how the NEXT sound gets stolen.
        for shot in ALL {
            let (notes, count) = Struck::notes(shot, 1.0);
            let last = notes[..count]
                .iter()
                .map(|n| n.start + n.len)
                .fold(0.0f32, f32::max);
            assert!(
                shot.length() >= last - 1e-6,
                "{shot:?} is retired at {} but plays until {last}",
                shot.length()
            );
        }
    }

    #[test]
    fn a_struck_voice_goes_quiet_on_its_own() {
        let mut v = Struck::new();
        assert!(!v.alive(), "silent until triggered");
        v.retrigger(1.0, Shot::Paddle.as_pitch());
        assert!(v.alive());

        // Render past the end of the longest sound.
        let mut buf = [0.0f32; 512];
        let sr = 48_000.0;
        for _ in 0..200 {
            v.render(&mut buf, VoiceParams::default(), sr);
        }
        assert!(!v.alive(), "a one-shot must retire itself");
    }

    #[test]
    fn a_rendered_sound_is_audible_and_never_clips() {
        let sr = 48_000.0;
        for shot in ALL {
            let mut v = Struck::new();
            v.retrigger(1.0, shot.as_pitch());

            let mut peak = 0.0f32;
            let mut buf = [0.0f32; 256];
            for _ in 0..120 {
                v.render(&mut buf, VoiceParams::default(), sr);
                for s in buf {
                    assert!(s.is_finite(), "{shot:?} produced a non-finite sample");
                    peak = peak.max(s.abs());
                }
            }
            assert!(peak > 0.01, "{shot:?} is inaudible (peak {peak})");
            assert!(peak <= 1.0, "{shot:?} clips (peak {peak})");
        }
    }
    /// ⚠️ **The calibration check.** `strike()` here and `strike()` in
    /// `tools/sfx/volley.html` are one function written twice, and the
    /// whole value of the playground rests on them staying that way: a
    /// level of 0.30 there must BE a level of 0.30 here.
    ///
    /// The racer's `chimes.html` is the counter-example this guards
    /// against. It was built from a different synthesis — an exponential
    /// attack against a linear one, a different harmonic series, a decay
    /// ending at 0.0001 against 0.057 — so every number ever tuned in it
    /// described a sound the game did not make. Nobody noticed, because
    /// chimes play alone and nothing contradicted them.
    ///
    /// These expectations were produced by RUNNING the playground's
    /// JavaScript (node, f64) and pasting its output. They are not
    /// derived from the Rust, so agreement means the two implementations
    /// genuinely match rather than that one was fitted to the other. The
    /// tolerance is f32-against-f64 rounding, nothing more.
    #[test]
    fn the_playground_and_the_rust_are_the_same_function() {
        // (note, samples at 0, 1, 7, 31, 63, 127, 255, 511) at 48 kHz.
        #[allow(clippy::excessive_precision)]
        let cases: [(Note, [f32; 8]); 5] = [
            (
                Note { start: 0.0, hz: 245.0, len: 0.075, level: 0.34, tone: 0.22 },
                [0.0, 0.0000363, 0.0017438, 0.0251470, 0.0431509, -0.0715299,
                 0.1613683, -0.1211256],
            ),
            (
                Note { start: 0.0, hz: 165.0, len: 0.045, level: 0.17, tone: 0.10 },
                [0.0, 0.0000106, 0.0005131, 0.0088576, 0.0255461, 0.0168835,
                 -0.0606319, -0.0864712],
            ),
            (
                Note { start: 0.0, hz: 522.67, len: 0.09, level: 0.24, tone: 0.18 },
                [0.0, 0.0000520, 0.0023806, 0.0146933, -0.0322138, 0.0430507,
                 -0.1353631, -0.0576298],
            ),
            (
                Note { start: 0.0, hz: 495.0, len: 0.13, level: 0.30, tone: 0.20 },
                [0.0, 0.0000631, 0.0029010, 0.0197526, -0.0343186, 0.0803656,
                 -0.1127997, 0.2271239],
            ),
            (
                Note { start: 0.0, hz: 393.0, len: 0.26, level: 0.34, tone: 0.24 },
                [0.0, 0.0000598, 0.0028002, 0.0265944, -0.0044051, 0.0351427,
                 0.1391735, 0.3005259],
            ),
        ];

        const PICKS: [usize; 8] = [0, 1, 7, 31, 63, 127, 255, 511];
        for (note, want) in cases {
            let mut out = [0.0f32; 512];
            strike(&mut out, 48_000.0, 0.0, note);
            for (k, &i) in PICKS.iter().enumerate() {
                let got = out[i];
                assert!(
                    (got - want[k]).abs() < 2e-6,
                    "{} Hz sample {i}: rust {got:.7} vs playground {:.7}",
                    note.hz,
                    want[k]
                );
            }
        }
    }
}
