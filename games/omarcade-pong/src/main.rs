//! Pong — the second Omarcade title.
//!
//! This file is only wiring: input to intent, time to the simulation,
//! state to the renderer. The game lives in `state`/`physics`/`ai`, the
//! pixels in `render`.
//!
//! Note what is still absent, as in Breakout: no winit, no softbuffer,
//! not in this file and not in this crate's Cargo.toml. Everything
//! crosses through `omarcade_core`'s seam.

mod ai;
mod physics;
mod render;
mod state;

use omarcade_core::backend::winit_soft::{Idle, WinitBackend};
use omarcade_core::scores::ScoreFile;
use omarcade_core::{Backend, Canvas, Game, InputEvent, Key, Theme};

use ai::Opponent;
use physics::Accumulator;
use state::{GameState, Phase, Side};

const TITLE: &str = "Omarcade Pong";
const WIDTH: u32 = 960;
const HEIGHT: u32 = 720;

/// Score-file id. Matches the binary name and the file the marquee
/// reads, so it is public surface: renaming it orphans everyone's
/// scores.
const GAME_ID: &str = "omarcade-pong";
const GAME_NAME: &str = "Pong";

struct Pong {
    theme: Theme,
    state: GameState,
    accumulator: Accumulator,
    opponent: Opponent,
    /// Both directions tracked independently rather than a single `dir`
    /// that the last key wins. Holding Up and tapping Down would
    /// otherwise release the paddle when Down lifts, leaving it stuck
    /// while Up is still physically held.
    up_held: bool,
    down_held: bool,
    scores: ScoreFile,
    /// Whether this match's result has been banked. `Phase::Over` stays
    /// set for every frame after the last point, so recording on the
    /// phase alone would rewrite the file sixty times a second; this
    /// makes it an edge, not a level.
    recorded: bool,
}

impl Pong {
    fn new(theme: Theme) -> Self {
        // Longest rally is higher-is-better, so the default ranking is
        // the right one and no `lower_is_better()` is needed here.
        let scores = ScoreFile::load_or_new(GAME_ID, GAME_NAME);
        let state = GameState::new();
        let opponent = Opponent::new(Side::Right, state.difficulty);

        let mut game = Pong {
            theme,
            state,
            accumulator: Accumulator::new(),
            opponent,
            up_held: false,
            down_held: false,
            scores,
            recorded: false,
        };
        game.refresh_best();
        game
    }

    /// Pull the record for the difficulty currently selected.
    ///
    /// Per difficulty, not overall: an easy run and a hard run are
    /// different games, so showing one as the other's target would be
    /// meaningless.
    fn refresh_best(&mut self) {
        self.state.best = self
            .scores
            .best_for(self.state.difficulty.id())
            .unwrap_or(0);
    }

    /// Bank the match's longest rally the first time it ends.
    ///
    /// **Longest rally, not the score.** First-to-11 means the score is
    /// won-or-lost rather than a measure of how well it went — 11-9 and
    /// 11-0 are the same "11". The longest rally describes the play
    /// itself, is comparable within a difficulty, and is
    /// higher-is-better, so it threads through the existing contract
    /// without asking the marquee to rank backwards.
    ///
    /// Save failures are swallowed on purpose: a scoreboard that cannot
    /// be written is not a reason to interrupt someone's game.
    fn bank_score(&mut self) {
        if self.recorded {
            return;
        }
        self.recorded = true;
        self.scores
            .record_at(self.state.longest_rally, self.state.difficulty.id());
        self.refresh_best();
        let _ = self.scores.save();
    }

    /// Resolve held keys into a paddle direction. Both held cancels
    /// out, which is what a player expects.
    fn apply_direction(&mut self) {
        self.state.left.dir = match (self.up_held, self.down_held) {
            (true, false) => -1.0,
            (false, true) => 1.0,
            _ => 0.0,
        };
    }

    /// Space: choose, serve, or nothing.
    fn on_confirm(&mut self) {
        match self.state.phase {
            Phase::Select => {
                self.state.begin();
                self.opponent.reset(self.state.difficulty);
                self.refresh_best();
            }
            Phase::Serve => physics::serve(&mut self.state),
            _ => {}
        }
    }

    /// Up/Down on the select screen change difficulty rather than
    /// moving a paddle.
    fn on_select_move(&mut self, down: bool) {
        if self.state.phase != Phase::Select {
            return;
        }
        self.state.difficulty = if down {
            self.state.difficulty.next()
        } else {
            self.state.difficulty.prev()
        };
        self.state.apply_difficulty();
        self.opponent.reset(self.state.difficulty);
        self.refresh_best();
    }
}

impl Game for Pong {
    fn on_input(&mut self, event: InputEvent) -> bool {
        match event {
            InputEvent::KeyDown(Key::Escape) => return false,

            InputEvent::KeyDown(Key::Up) => {
                self.up_held = true;
                self.on_select_move(false);
            }
            InputEvent::KeyUp(Key::Up) => self.up_held = false,
            InputEvent::KeyDown(Key::Down) => {
                self.down_held = true;
                self.on_select_move(true);
            }
            InputEvent::KeyUp(Key::Down) => self.down_held = false,

            InputEvent::KeyDown(Key::Space) => self.on_confirm(),

            // Enter restarts, but only once the match has actually
            // ended — otherwise a stray press wipes a game in progress.
            InputEvent::KeyDown(Key::Enter) if self.state.is_over() => {
                self.state.restart();
                self.opponent.reset(self.state.difficulty);
                // Arm the next match, or its result is never banked.
                self.recorded = false;
            }

            _ => {}
        }

        self.apply_direction();
        true
    }

    fn update(&mut self, dt: f32) {
        // The opponent decides before time advances, so its choice is
        // acted on by the same physics step the player's input is.
        self.opponent.update(&mut self.state, dt);
        physics::step(&mut self.state, &mut self.accumulator, dt);

        if self.state.is_over() {
            self.bank_score();
        }
    }

    fn render(&mut self, canvas: &mut Canvas<'_>) {
        render::draw(&self.state, canvas, &self.theme);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let theme = Theme::load();

    WinitBackend::new(TITLE, WIDTH, HEIGHT)
        .idle(Idle::Animate { fps: 60 })
        .run(Pong::new(theme))?;

    Ok(())
}
