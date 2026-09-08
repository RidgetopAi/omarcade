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

mod effects;
mod items;
mod physics;
mod render;
mod state;

use omarcade_core::backend::winit_soft::{Idle, WinitBackend};
use omarcade_core::scores::ScoreFile;
use omarcade_core::{Audio, AudioSystem, Backend, Canvas, Game, InputEvent, Key, Theme};

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
}

impl PixelBreak {
    fn new(theme: Theme) -> Self {
        let scores = ScoreFile::load_or_new(GAME_ID, GAME_NAME);
        let mut state = GameState::new();
        state.best = scores.best().unwrap_or(0);

        PixelBreak {
            theme,
            state,
            accumulator: Accumulator::new(),
            left_held: false,
            right_held: false,
            scores,
            recorded: false,
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
        self.recorded = true;
        self.scores.record(self.state.score);
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
            InputEvent::KeyDown(Key::Space) => self.state.launch(),
            // Releasing fires whatever the magnet is holding. A no-op with
            // nothing held, which is every press outside a magnet.
            InputEvent::KeyUp(Key::Space) => {
                self.state.release_held_balls();
            }

            // Enter restarts, but only once the game has actually
            // ended — otherwise a stray press wipes a game in progress.
            InputEvent::KeyDown(Key::Enter) => {
                if matches!(self.state.phase, Phase::Won | Phase::Lost) {
                    self.state.restart();
                    // Arm the next run, or its score would never be banked.
                    self.recorded = false;
                }
            }

            _ => {}
        }

        self.apply_direction();
        true
    }

    fn update(&mut self, dt: f32, _audio: &mut Audio<'_>) {
        physics::step(&mut self.state, &mut self.accumulator, dt);

        if matches!(self.state.phase, Phase::Won | Phase::Lost) {
            self.bank_score();
        }
    }

    fn render(&mut self, canvas: &mut Canvas<'_>) {
        render::draw(&mut self.state, canvas, &self.theme);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let theme = Theme::load();

    WinitBackend::new(TITLE, WIDTH, HEIGHT)
        // Session 1 shipped Idle::Wait, which costs nothing but never
        // redraws on its own. There is a ball to move now, so the loop
        // paces itself with WaitUntil — still never Poll.
        .idle(Idle::Animate { fps: 60 })
        .run(PixelBreak::new(theme), AudioSystem::new())?;

    Ok(())
}
