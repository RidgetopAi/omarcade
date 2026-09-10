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
    Audio, AudioSystem, Backend, Canvas, Game, InputEvent, Key, Theme, VolumeIndicator,
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
}

impl PixelBreak {
    /// ⚠️ Takes the `AudioSystem` so voices can be registered while it is
    /// still accepting them. The alternative — registering inside `main`
    /// and passing the `Bank` in — splits one decision across two files.
    fn new(theme: Theme, audio: &mut AudioSystem) -> Self {
        let scores = ScoreFile::load_or_new(GAME_ID, GAME_NAME);
        let mut state = GameState::new();
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

    fn update(&mut self, dt: f32, audio: &mut Audio<'_>) {
        // ⚠️ BEFORE the phase-dependent work and outside any match on it.
        // The volume keys answer on the Ready screen and after a game
        // over, so this must advance in every phase — the S7 rule that
        // left a shake stuck at 100% when it ticked inside `Playing`.
        self.volume.update(audio, dt);

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
        // Drawn last so it sits above the field, and by `main` rather than
        // by `render` because it is not part of the game's own picture.
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
