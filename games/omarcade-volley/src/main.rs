//! Volley — the second Omarcade title.
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
mod sound;
mod state;

use omarcade_core::backend::winit_soft::{Idle, WinitBackend};
use omarcade_core::scores::ScoreFile;
use omarcade_core::{
    Audio, AudioSystem, Backend, Canvas, Game, InputEvent, Key, Pause, Theme, VolumeIndicator,
};

use ai::Opponent;
use physics::Accumulator;
use state::{GameState, Phase, Side};

const TITLE: &str = "Omarcade Volley";
const WIDTH: u32 = 960;
const HEIGHT: u32 = 720;

/// Score-file id. Matches the binary name and the file the marquee
/// reads, so it is public surface.
///
/// This game shipped as Pong and was renamed. Renaming the id renames the
/// score file, which is why [`LEGACY_GAME_ID`] exists rather than this
/// being a one-line change: the records under the old name are real runs
/// on a real machine and they are carried across on first launch.
const GAME_ID: &str = "omarcade-volley";
const GAME_NAME: &str = "Volley";

/// The id this game's scores were written under before the rename.
///
/// Kept as a constant rather than inlined because it is a historical
/// fact with an expiry: once every install has launched once, no
/// `omarcade-pong.json` exists anywhere and this can go. It is harmless
/// until then — a missing legacy file is the ordinary case, not an error.
const LEGACY_GAME_ID: &str = "omarcade-pong";

struct Volley {
    theme: Theme,
    /// The volume readout.
    volume: VolumeIndicator,
    /// Volley's five voices. See `sound`.
    sound: sound::Bank,
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
    /// Whether the match is held still, and the PAUSED overlay.
    ///
    /// ⚠️ A flag rather than a `Phase`: pause is orthogonal to Select /
    /// Serve / Playing / Over, and resuming must land back in the one it
    /// left. See `omarcade_core::pause`.
    pause: Pause,
}

impl Volley {
    /// ⚠️ Takes the `AudioSystem` so voices can be registered while it is
    /// still accepting them. The alternative — registering inside `main`
    /// and passing the `Bank` in — splits one decision across two files.
    fn new(theme: Theme, audio: &mut AudioSystem) -> Self {
        // Longest rally is higher-is-better, so the default ranking is
        // the right one and no `lower_is_better()` is needed here.
        let scores = ScoreFile::load_or_migrate(GAME_ID, GAME_NAME, LEGACY_GAME_ID);
        let state = GameState::new();
        let opponent = Opponent::new(Side::Right, state.difficulty);

        let mut game = Volley {
            theme,
            volume: VolumeIndicator::new(),
            sound: sound::Bank::register(audio),
            state,
            accumulator: Accumulator::new(),
            opponent,
            up_held: false,
            down_held: false,
            scores,
            recorded: false,
            pause: Pause::new(),
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

impl Game for Volley {
    fn on_input(&mut self, event: InputEvent) -> bool {
        match event {
            InputEvent::KeyDown(Key::Escape) => return false,

            // ⚠️ **P, and only P.** Escape is quit, above.
            //
            // Refused on the select screen: nothing is running to freeze
            // there, and a PAUSED overlay over a menu covers the very
            // thing it would need the player to read.
            InputEvent::KeyDown(Key::P) => {
                if !matches!(self.state.phase, Phase::Select) {
                    self.pause.toggle();
                }
            }

            // ⚠️ Only key PRESSES are swallowed. A RELEASE only ever
            // clears state, and swallowing the arrow KeyUps would leave
            // `up_held` set after the player let go, so the paddle would
            // set off on its own the moment the match resumed. Releases
            // fall through to their own arms and to `apply_direction`.
            InputEvent::KeyDown(_) if self.pause.is_paused() => return true,

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
                // ⚠️ A restart must never inherit a pause: a fresh match
                // frozen behind a PAUSED overlay reads as a hang.
                self.pause.resume();
            }

            _ => {}
        }

        self.apply_direction();
        true
    }

    fn update(&mut self, dt: f32, audio: &mut Audio<'_>) {
        // ⚠️ Outside any phase match — the volume keys answer between
        // points, not only during them.
        self.volume.update(audio, dt);

        // ⚠️ AFTER the volume tick: the volume keys answer in every phase
        // and pausing must not be the one state where they stop, or a
        // mute pressed while paused would leave the indicator frozen on
        // screen. Everything below is the simulation, and it stops.
        if self.pause.is_paused() {
            return;
        }

        // The opponent decides before time advances, so its choice is
        // acted on by the same physics step the player's input is.
        self.opponent.update(&mut self.state, dt);
        physics::step(&mut self.state, &mut self.accumulator, dt);

        // Everything the simulation thought was worth hearing, played once
        // per frame rather than once per fixed tick.
        //
        // ⚠️ `physics::step` runs several fixed ticks in a frame, so
        // draining HERE — after the whole step rather than inside it — is
        // what keeps a slow frame from playing the same bounce twice.
        let bank = &mut self.sound;
        self.state.drain_cues(|cue| bank.play(audio, cue));

        // ⚠️ The end screen's clock, driven from the FRAME loop. Nothing
        // in `physics::step` reaches Phase::Over — it runs no simulation
        // — so the staged reveal would never advance if this lived there.
        self.state.tick_over(dt);

        if self.state.is_over() {
            self.bank_score();
        }
    }

    fn render(&mut self, canvas: &mut Canvas<'_>) {
        render::draw(&self.state, canvas, &self.theme);
        // ⚠️ The scrim goes down BEFORE the volume indicator, so a volume
        // change made while paused stays legible instead of being dimmed
        // with the field under it.
        //
        // ⚠️ Centred on the WINDOW here, unlike Pixel Break. Volley's ball
        // crosses the whole field, so no band is permanently clear — but
        // the scores sit at the top (y=46) and the hint at the bottom
        // (y=694), and the middle is the one place the overlay collides
        // with neither. Verified by rendering it.
        self.pause.draw(canvas, &self.theme);
        self.volume.draw(canvas, &self.theme);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let theme = Theme::load();

    // Registration is startup-only, so the system is built here and the
    // voices go in before `run` opens the device.
    let mut audio = AudioSystem::new();
    let game = Volley::new(theme, &mut audio);

    WinitBackend::new(TITLE, WIDTH, HEIGHT)
        .idle(Idle::Animate { fps: 60 })
        .run(game, audio)?;

    Ok(())
}

#[cfg(test)]
mod pause_tests {
    use super::*;

    fn game() -> Volley {
        // A registered-but-never-started AudioSystem: the voices go in,
        // no device is ever opened, and nothing renders. Exactly what a
        // headless test wants.
        let mut audio = AudioSystem::new();
        Volley::new(Theme::default(), &mut audio)
    }

    /// A match in progress, which is where a pause matters.
    fn playing() -> Volley {
        let mut g = game();
        g.state.phase = Phase::Playing;
        g
    }

    #[test]
    fn p_pauses_and_p_resumes() {
        let mut g = playing();
        assert!(!g.pause.is_paused(), "a match must not start paused");
        g.on_input(InputEvent::KeyDown(Key::P));
        assert!(g.pause.is_paused(), "P must pause");
        g.on_input(InputEvent::KeyDown(Key::P));
        assert!(!g.pause.is_paused(), "P again must resume");
    }

    /// ⚠️ Escape is QUIT. If pause ever starts answering it, a player
    /// reaching for a pause loses their match.
    #[test]
    fn escape_still_quits_and_does_not_pause() {
        let mut g = playing();
        assert!(
            !g.on_input(InputEvent::KeyDown(Key::Escape)),
            "Escape must quit"
        );
        assert!(!g.pause.is_paused(), "Escape must never pause");
    }

    #[test]
    fn the_select_screen_does_not_pause() {
        let mut g = game();
        assert!(matches!(g.state.phase, Phase::Select));
        g.on_input(InputEvent::KeyDown(Key::P));
        assert!(
            !g.pause.is_paused(),
            "there is nothing running to freeze on the select screen"
        );
    }

    /// ★ Pause while holding Up, let go while paused, resume: if the
    /// KeyUp is swallowed then `up_held` stays true and the paddle sets
    /// off on its own with no key held.
    #[test]
    fn releasing_an_arrow_while_paused_does_not_strand_the_paddle() {
        let mut g = playing();
        g.on_input(InputEvent::KeyDown(Key::Up));
        assert!(g.up_held);

        g.on_input(InputEvent::KeyDown(Key::P));
        g.on_input(InputEvent::KeyUp(Key::Up));
        assert!(!g.up_held, "the release was swallowed by the pause gate");
    }

    #[test]
    fn a_pause_swallows_gameplay_presses() {
        let mut g = playing();
        g.on_input(InputEvent::KeyDown(Key::P));
        g.on_input(InputEvent::KeyDown(Key::Up));
        assert!(!g.up_held, "a press while paused must not reach the game");
    }
}
