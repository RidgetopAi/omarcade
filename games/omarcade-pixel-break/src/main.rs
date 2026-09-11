//! Pixel Break — the Omarcade release title.
//!
//! Shipped as `Breakout` through session 13; renamed here, in a commit
//! that changed nothing else, because the identity is public surface:
//! `GAME_ID` names the score file and the cabinet discovers games by
//! scanning for installed binaries.
//!
//! This file is only wiring: input to intent, time to the simulation,
//! state to the renderer. The game lives in `state`/`physics`, the
//! pixels in `render`.
//!
//! Note what is still absent, as in session 1: no winit, no softbuffer,
//! not in this file and not in this crate's Cargo.toml. Everything
//! crosses through `omarcade_core`'s seam.

/// Geometry now lives in the shared core: Pong needed every line of
/// it, which is the test of whether a shared crate earns its keep.
/// Re-exported under the old path so nothing else in the crate moved.
use omarcade_core::geom;

mod art;
mod effects;
mod items;
mod physics;
mod render;
mod sound;
mod state;

use omarcade_core::backend::winit_soft::{Idle, WinitBackend};
use omarcade_core::scores::ScoreFile;
use omarcade_core::{
    Audio, AudioSystem, Backend, Canvas, Game, InputEvent, Key, Pause, Theme, VolumeIndicator,
};

use physics::Accumulator;
use state::{GameState, Phase};

const TITLE: &str = "Pixel Break";
const WIDTH: u32 = 960;
const HEIGHT: u32 = 720;

/// Score-file id. Matches the binary name and the file the marquee reads,
/// so it is public surface: renaming it orphans everyone's high scores.
const GAME_ID: &str = "omarcade-pixel-break";
const GAME_NAME: &str = "Pixel Break";

struct PixelBreak {
    theme: Theme,
    state: GameState,
    accumulator: Accumulator,
    /// Both directions tracked independently, rather than a single
    /// `dir` that the last key wins. Holding Left and tapping Right
    /// would otherwise release the paddle when Right lifts, leaving it
    /// stuck while Left is still physically held.
    left_held: bool,
    right_held: bool,
    scores: ScoreFile,
    /// Whether the current run's score has already been banked. `Phase`
    /// stays Lost for every frame after the last life, so recording on
    /// the phase alone would rewrite the file sixty times a second;
    /// this makes it an edge, not a level.
    recorded: bool,
    /// Every registered voice, and whose turn it is.
    ///
    /// ⚠️ Built before the stream starts and carried here, because
    /// registration is startup-only: `AudioSystem` drops the pending list
    /// when `start` opens the device, and a voice registered after that
    /// would hand a boxed trait object to a thread that may not allocate.
    sound: sound::Bank,
    /// The volume readout, shown when the volume keys move.
    ///
    /// ⚠️ Lives here and not in `render`, because it needs the `Audio`
    /// handle and `render` deliberately never sees one. The type carries
    /// its own copy of the volume for exactly that reason.
    volume: VolumeIndicator,
    /// Whether the game is held still, and the PAUSED overlay.
    ///
    /// ⚠️ A flag beside the game rather than a `Phase`, which is the one
    /// place this game departs from "everything is a phase". Pause is
    /// ORTHOGONAL to the phases: you pause Playing, or Ready, or the
    /// title, and resuming must land back in the one you left. See
    /// `omarcade_core::pause` for the full argument.
    pause: Pause,
    /// ★ **Whether this run used the developer skip.** Debug builds only.
    ///
    /// A skipped run is never banked: Brian's real BEST must not be
    /// overwritten by a run that did not earn it. The tally still DRAWS
    /// in full, NEW BEST included — the screen tells the truth about the
    /// run in front of it — but the score file is left alone.
    #[cfg(debug_assertions)]
    skipped: bool,
}

impl PixelBreak {
    /// ⚠️ Takes the `AudioSystem` so voices can be registered while it is
    /// still accepting them. The alternative — registering inside `main`
    /// and passing the `Bank` in — splits one decision across two files.
    fn new(theme: Theme, audio: &mut AudioSystem) -> Self {
        let scores = ScoreFile::load_or_new(GAME_ID, GAME_NAME);
        let mut state = GameState::on_title();
        // Effects can fire before the first frame renders; without this
        // they would throw grey chips until one does.
        state.set_palette(render::palette(&theme));
        state.best = scores.best().unwrap_or(0);

        PixelBreak {
            theme,
            state,
            accumulator: Accumulator::new(),
            left_held: false,
            right_held: false,
            scores,
            recorded: false,
            sound: sound::Bank::register(audio),
            volume: VolumeIndicator::new(),
            pause: Pause::new(),
            #[cfg(debug_assertions)]
            skipped: false,
        }
    }

    /// Bank the run's score the first time a game ends.
    ///
    /// Save failures are swallowed on purpose: a scoreboard that cannot be
    /// written is not a reason to interrupt someone's game.
    fn bank_score(&mut self) {
        if self.recorded {
            return;
        }
        // ★ A skipped run is watched, not recorded. Debug builds only —
        // in release the flag does not exist and neither does the key
        // that sets it.
        #[cfg(debug_assertions)]
        if self.skipped {
            self.recorded = true;
            return;
        }
        self.recorded = true;
        // ⚠️ The FINAL score, not the running one — the life bonus is
        // part of what the player earned and the marquee should show it.
        self.scores.record(self.state.final_score());
        self.state.best = self.scores.best().unwrap_or(0);
        let _ = self.scores.save();
    }

    /// Resolve held keys into a paddle direction. Both held cancels out,
    /// which is what a player expects.
    fn apply_direction(&mut self) {
        self.state.paddle.dir = match (self.left_held, self.right_held) {
            (true, false) => -1.0,
            (false, true) => 1.0,
            _ => 0.0,
        };
    }
}

impl Game for PixelBreak {
    fn on_input(&mut self, event: InputEvent) -> bool {
        match event {
            InputEvent::KeyDown(Key::Escape) => return false,

            // ⚠️ **P, and only P.** Escape is quit, above — binding pause
            // to the key a player might guess would throw away their run.
            // The title screen says so, next to the volume keys.
            //
            // Refused on the title screen: there is nothing running to
            // freeze there, and a player who paused it would be looking at
            // a PAUSED overlay on a menu with no way to read what it is
            // covering. Every other phase pauses, including the end
            // screens — reading a final score in peace is a fair use of it.
            InputEvent::KeyDown(Key::P) => {
                if self.state.phase != Phase::Title {
                    self.pause.toggle();
                }
            }

            // ⚠️ **Everything else is swallowed while paused**, and this
            // has to be a gate rather than a check inside each arm.
            //
            // Space is the reason. Hold it for the magnet, pause, then let
            // go: without this the release would FIRE the held ball at a
            // field the player cannot see, aimed by a paddle they cannot
            // watch. It stays held instead, and its own three-second
            // auto-release takes it from there once play resumes.
            //
            // Enter is the other: a paused game-over screen is a fair
            // place to sit, and a stray Enter there would wipe the run
            // before the player had finished reading it.
            //
            // ⚠️ Only KEY PRESSES are swallowed. A RELEASE is never an
            // action — it only ever clears state — and swallowing the
            // arrow KeyUps would leave `left_held` true after the player
            // let go, so the paddle would set off on its own the moment
            // play resumed with no key held. Releases fall through to
            // their own arms and to `apply_direction`.
            //
            // The one release that IS an action — Space firing a held
            // ball — checks the pause in its own arm below, because the
            // ball must stay HELD rather than merely not-fired.
            InputEvent::KeyDown(_) if self.pause.is_paused() => return true,

            // ★ **THE DEVELOPER SKIP. DEBUG BUILDS ONLY.**
            //
            // Built because the ending takes 68.9 minutes to reach at the
            // FLOOR and Brian had tried three times without seeing the
            // thing he commissioned. `cargo run -p omarcade-pixel-break`,
            // tap to level 10, play it for real.
            //
            // ⚠️ It kills the remaining bricks and lets `check_win` fire
            // on the next physics tick, rather than calling
            // `advance_level` from here. That keeps ONE entry point into
            // the level transition: the skip takes the genuine path, with
            // the real cue, the real clear bonus, and the real
            // `begin_victory` on level 10. A second caller would be a
            // second thing to keep in step, and the ending is exactly the
            // thing that must not be a special case of itself.
            //
            // ⚠️ Only while `Playing`. During `Clearing` or `Victory` a
            // press would skip a level the cascade has not finished
            // handing over.
            //
            // ⚠️ No per-brick score is awarded, because no bricks were
            // broken. The tally shows what the run actually earned.
            #[cfg(debug_assertions)]
            InputEvent::KeyDown(Key::F8) => {
                if self.state.phase == Phase::Playing {
                    self.skipped = true;
                    for brick in &mut self.state.bricks {
                        brick.hits = 0;
                    }
                }
            }

            InputEvent::KeyDown(Key::Left) => self.left_held = true,
            InputEvent::KeyUp(Key::Left) => self.left_held = false,
            InputEvent::KeyDown(Key::Right) => self.right_held = true,
            InputEvent::KeyUp(Key::Right) => self.right_held = false,

            // ⚠️ **Space carries two meanings and the PHASE separates
            // them.** In `Ready` a press launches; while the magnet holds a
            // ball, the RELEASE fires it. The plan flagged this as S6's
            // real risk — that a press arriving mid-hold might re-launch
            // something — and the guard is in `launch` itself, which
            // returns immediately unless the phase is `Ready`. A held ball
            // only ever exists during `Playing`, so the two cannot overlap.
            // Tested in `magnet_tests`, not merely reasoned about.
            // ⚠️ Space means two things and the PHASE decides which: on
            // the title it opens the door, in play it launches the ball.
            // Two calls rather than one overloaded one — see
            // `start_from_title`.
            InputEvent::KeyDown(Key::Space) => {
                if self.state.phase == Phase::Title {
                    self.state.start_from_title();
                } else {
                    self.state.launch();
                }
            }
            // Releasing fires whatever the magnet is holding. A no-op with
            // nothing held, which is every press outside a magnet.
            //
            // ⚠️ Not while paused: firing at a field the player cannot see,
            // aimed by a paddle they cannot watch, is the opposite of what
            // the magnet is for. The ball stays held and its own
            // three-second auto-release takes over once play resumes.
            InputEvent::KeyUp(Key::Space) => {
                if !self.pause.is_paused() {
                    self.state.release_held_balls();
                }
            }

            // Enter restarts, but only once the game has actually
            // ended — otherwise a stray press wipes a game in progress.
            InputEvent::KeyDown(Key::Enter) => {
                if matches!(self.state.phase, Phase::Won | Phase::Lost) {
                    self.state.restart();
                    // Arm the next run, or its score would never be banked.
                    self.recorded = false;
                    // ⚠️ A restart must never inherit a pause. Enter is
                    // unreachable while paused today, but a fresh game
                    // frozen behind a PAUSED overlay reads as a hang, and
                    // that must not depend on the input gate above.
                    self.pause.resume();
                }
            }

            _ => {}
        }

        self.apply_direction();
        true
    }

    fn update(&mut self, dt: f32, audio: &mut Audio<'_>) {
        // ⚠️ BEFORE the phase-dependent work and outside any match on it.
        // The volume keys answer on the Ready screen and after a game
        // over, so this must advance in every phase — the S7 rule that
        // left a shake stuck at 100% when it ticked inside `Playing`.
        self.volume.update(audio, dt);

        // ⚠️ AFTER the volume tick, and that order is the whole point.
        // The volume keys are handled by the backend and answer in every
        // phase; pausing must not be the one state where they stop being
        // answered, or a mute pressed while paused would leave the
        // indicator frozen on screen with nothing to decay it. Everything
        // below this line is the simulation, and the simulation stops.
        if self.pause.is_paused() {
            return;
        }

        physics::step(&mut self.state, &mut self.accumulator, dt);

        // Everything the simulation thought was worth hearing, played once
        // per frame rather than once per fixed tick.
        //
        // ⚠️ `physics::step` runs up to `MAX_STEPS_PER_FRAME` ticks, so a
        // frame can hold several ticks' worth of cues. Draining here — after
        // the whole step, not inside it — is what keeps a slow frame from
        // playing the same brick twice.
        let bank = &mut self.sound;
        self.state.drain_cues(|cue| bank.play(audio, cue));

        if matches!(self.state.phase, Phase::Won | Phase::Lost) {
            self.bank_score();
        }
    }

    fn render(&mut self, canvas: &mut Canvas<'_>) {
        render::draw(&mut self.state, canvas, &self.theme);
        // ⚠️ The scrim goes down BEFORE the volume indicator, so a volume
        // change made while paused stays legible instead of being dimmed
        // along with the field it is sitting on. Both are drawn by `main`
        // rather than by `render`, because neither is part of the game's
        // own picture.
        // ⚠️ Centred on the PLAYFIELD, not the window. The window centre
        // (y=360) sits just under the brick field's fixed bottom edge at
        // y=288 — right in the airspace the ball occupies, which a render
        // showed immediately and no test could. The clear band runs from
        // the bricks (288) to the paddle (660); its middle is 474.
        let band_mid = ((state::BRICK_TOP
            + (state::BRICK_ROWS as f32 - 1.0) * (state::BRICK_H + state::BRICK_GAP)
            + state::BRICK_H)
            + state::PADDLE_Y)
            / 2.0;
        self.pause
            .draw_centred_at(canvas, &self.theme, band_mid as i32);
        self.volume.draw(canvas, &self.theme);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let theme = Theme::load();

    // Registration is startup-only, so the system is built here and the
    // voices go in before `run` opens the device.
    let mut audio = AudioSystem::new();
    let game = PixelBreak::new(theme, &mut audio);

    WinitBackend::new(TITLE, WIDTH, HEIGHT)
        // Session 1 shipped Idle::Wait, which costs nothing but never
        // redraws on its own. There is a ball to move now, so the loop
        // paces itself with WaitUntil — still never Poll.
        .idle(Idle::Animate { fps: 60 })
        .run(game, audio)?;

    Ok(())
}

#[cfg(test)]
mod pause_tests {
    use super::*;
    use omarcade_core::geom::Vec2;

    /// ⚠️ `AudioSystem::new` opens no device — it is pure until `start`,
    /// so the whole game harness is constructible in a test and the input
    /// wiring can be driven with real events rather than reasoned about.
    fn game() -> PixelBreak {
        let mut audio = AudioSystem::new();
        PixelBreak::new(Theme::default(), &mut audio)
    }

    /// A game past the title, mid-play, which is where a pause matters.
    fn playing() -> PixelBreak {
        let mut g = game();
        g.state.start_from_title();
        g.state.launch();
        assert_eq!(g.state.phase, Phase::Playing);
        g
    }

    #[test]
    fn p_pauses_and_p_resumes() {
        let mut g = playing();
        assert!(!g.pause.is_paused(), "a game must not start paused");
        g.on_input(InputEvent::KeyDown(Key::P));
        assert!(g.pause.is_paused(), "P must pause");
        g.on_input(InputEvent::KeyDown(Key::P));
        assert!(!g.pause.is_paused(), "P again must resume");
    }

    /// ⚠️ Escape is QUIT, in all three games. If pause ever starts
    /// answering it, a player reaching for a pause loses their run.
    #[test]
    fn escape_still_quits_and_does_not_pause() {
        let mut g = playing();
        assert!(!g.on_input(InputEvent::KeyDown(Key::Escape)), "Escape must quit");
        assert!(!g.pause.is_paused(), "Escape must never pause");
    }

    #[test]
    fn the_title_screen_does_not_pause() {
        let mut g = game();
        assert_eq!(g.state.phase, Phase::Title);
        g.on_input(InputEvent::KeyDown(Key::P));
        assert!(
            !g.pause.is_paused(),
            "there is nothing running to freeze on the title screen"
        );
    }

    /// ★ **The bug this nearly shipped with.** Pause while holding Left,
    /// let go while paused, resume: if the KeyUp is swallowed by the pause
    /// gate then `left_held` stays true and the paddle sets off on its own
    /// with no key held. A release is never an action — it only ever
    /// clears state — so only key PRESSES may be swallowed.
    #[test]
    fn releasing_an_arrow_while_paused_does_not_strand_the_paddle() {
        let mut g = playing();
        g.on_input(InputEvent::KeyDown(Key::Left));
        assert!(g.left_held);

        g.on_input(InputEvent::KeyDown(Key::P));
        assert!(g.pause.is_paused());

        g.on_input(InputEvent::KeyUp(Key::Left));
        assert!(!g.left_held, "the release was swallowed by the pause gate");

        g.on_input(InputEvent::KeyDown(Key::P));
        assert_eq!(
            g.state.paddle.dir, 0.0,
            "the paddle resumed moving with no key held"
        );
    }

    /// ⚠️ A press IS swallowed, which is the other half of the same rule.
    #[test]
    fn a_pause_swallows_gameplay_presses() {
        let mut g = playing();
        g.on_input(InputEvent::KeyDown(Key::P));
        g.on_input(InputEvent::KeyDown(Key::Left));
        assert!(!g.left_held, "a press while paused must not reach the game");
    }

    /// ⚠️ Hold Space for the magnet, pause, release: the ball must stay
    /// HELD rather than firing at a field the player cannot see, aimed by
    /// a paddle they cannot watch.
    #[test]
    fn releasing_space_while_paused_keeps_the_ball_held() {
        let mut g = playing();
        // Put a ball in the magnet's grip directly — this is about the
        // input wiring, not about how a catch happens.
        g.state.balls[0].held_for = Some(1.0);
        g.state.balls[0].vel = Vec2::ZERO;

        g.on_input(InputEvent::KeyDown(Key::P));
        g.on_input(InputEvent::KeyUp(Key::Space));
        assert!(
            g.state.balls[0].is_held(),
            "a paused release fired the ball blind"
        );

        // And once resumed, the same release does fire it.
        g.on_input(InputEvent::KeyDown(Key::P));
        g.on_input(InputEvent::KeyUp(Key::Space));
        assert!(!g.state.balls[0].is_held(), "resuming must restore the release");
    }

    /// ⚠️ A paused game-over screen is a fair place to sit and read a
    /// final score. A stray Enter must not wipe the run first.
    #[test]
    fn enter_cannot_restart_a_paused_game_over() {
        let mut g = playing();
        g.state.phase = Phase::Lost;
        g.state.score = 4321;

        g.on_input(InputEvent::KeyDown(Key::P));
        g.on_input(InputEvent::KeyDown(Key::Enter));
        assert_eq!(g.state.phase, Phase::Lost, "Enter restarted a paused game");
        assert_eq!(g.state.score, 4321, "the run was wiped while paused");
    }

    /// ⚠️ And a restart must never inherit a pause: a fresh game frozen
    /// behind a PAUSED overlay reads as a hang.
    #[test]
    fn restarting_clears_the_pause() {
        let mut g = playing();
        g.state.phase = Phase::Lost;
        g.on_input(InputEvent::KeyDown(Key::Enter));
        assert!(!g.pause.is_paused());

        // Even if a pause somehow survived to the restart itself.
        g.pause.toggle();
        g.pause.resume();
        assert!(!g.pause.is_paused());
    }

    // ---- the developer skip (debug builds only) ----

    /// ★ The skip clears the field, and the level advances through the
    /// REAL path — `check_win` on the next physics tick, not a second
    /// caller of `advance_level`.
    #[cfg(debug_assertions)]
    #[test]
    fn the_skip_clears_the_field_and_advances() {
        let mut g = playing();
        assert!(g.state.bricks_remaining() > 0);
        let level = g.state.level;

        g.on_input(InputEvent::KeyDown(Key::F8));
        assert_eq!(g.state.bricks_remaining(), 0, "the field was not cleared");

        // One frame of real physics is what actually advances it.
        let mut audio = AudioSystem::new();
        g.update(1.0 / 60.0, &mut audio.handle());
        assert_ne!(g.state.phase, Phase::Playing, "the level did not transition");
        assert!(
            g.state.score > 0,
            "the level-clear bonus was not awarded — the skip bypassed advance_level"
        );
        let _ = level;
    }

    /// ⚠️ Only while Playing. During the cascade a press would skip a
    /// level the transition has not finished handing over.
    #[cfg(debug_assertions)]
    #[test]
    fn the_skip_is_refused_outside_play() {
        let mut g = playing();
        g.state.phase = Phase::Clearing;
        let before = g.state.bricks_remaining();
        g.on_input(InputEvent::KeyDown(Key::F8));
        assert_eq!(g.state.bricks_remaining(), before, "the skip fired mid-cascade");
        assert!(!g.skipped, "a refused skip must not taint the run");
    }

    /// ⚠️ And it is swallowed while paused, like every other press.
    #[cfg(debug_assertions)]
    #[test]
    fn the_skip_is_swallowed_while_paused() {
        let mut g = playing();
        g.on_input(InputEvent::KeyDown(Key::P));
        let before = g.state.bricks_remaining();
        g.on_input(InputEvent::KeyDown(Key::F8));
        assert_eq!(g.state.bricks_remaining(), before, "the skip fired while paused");
    }

    /// ★★ **The one that protects Brian's marquee.** A skipped run is
    /// watched, never recorded.
    #[cfg(debug_assertions)]
    #[test]
    fn a_skipped_run_is_never_banked() {
        let mut g = playing();
        g.on_input(InputEvent::KeyDown(Key::F8));
        assert!(g.skipped);

        let best_before = g.state.best;
        g.state.phase = Phase::Lost;
        g.state.score = 999_999;
        g.bank_score();

        assert_eq!(
            g.state.best, best_before,
            "a skipped run overwrote the best score"
        );
    }

    /// ⚠️ An untouched run still banks — the taint must be the skip, not
    /// the mere existence of the flag.
    #[cfg(debug_assertions)]
    #[test]
    fn an_ordinary_run_still_banks() {
        let g = playing();
        assert!(!g.skipped, "a fresh run must not be marked skipped");
    }

}
