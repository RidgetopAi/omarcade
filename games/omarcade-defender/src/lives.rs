//! Being shot down, and getting back up.
//!
//! ★ S7 IS THE FIRST STAGE WHERE ANYTHING CAN HURT YOU. Until Mutants
//! existed the ship was invulnerable by omission rather than by design,
//! and "what happens when you are hit" had never been answered.
//!
//! Brian's answer: three lives, respawn IN PLACE with a moment of
//! invulnerability, game over at zero — the arcade's own shape. Respawning
//! where you died keeps the pressure on (the Mutant that killed you is
//! still there) while the invulnerability makes that survivable rather
//! than a death loop.

/// How many lives a game starts with.
pub const STARTING_LIVES: u32 = 3;

/// How long the ship stays exploded before it comes back, in seconds.
///
/// ⚠️ LONG ENOUGH TO BE READ AS A DEATH. Anything under about a second
/// and the player is flying again before they have understood that they
/// were hit, which reads as a glitch rather than as a consequence.
pub const DEATH_SECONDS: f32 = 1.2;

/// How long a fresh ship cannot be hurt, in seconds.
///
/// ★ THE REASON RESPAWNING IN PLACE IS FAIR. Without it, coming back
/// inside the same cloud of Mutant fire that just killed you would take
/// the next life immediately, and the one after that.
pub const INVULNERABLE_SECONDS: f32 = 2.0;

/// How fast the invulnerable ship blinks, in blinks per second.
///
/// The blink is not decoration: it is the only way the player knows the
/// rule is currently suspended, and therefore when it stops.
const BLINK_HZ: f32 = 6.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Flying, and vulnerable.
    Alive,
    /// Just invulnerable, and blinking.
    Respawned,
    /// Exploded, waiting to come back.
    Dead,
    /// Out of lives.
    GameOver,
}

#[derive(Debug, Clone, Copy)]
pub struct Lives {
    pub remaining: u32,
    pub state: State,
    /// Seconds spent in the current state.
    elapsed: f32,
}

impl Default for Lives {
    fn default() -> Self {
        Self::new()
    }
}

impl Lives {
    pub fn new() -> Self {
        Self {
            remaining: STARTING_LIVES,
            // Start invulnerable, so a wave that spawns near the player
            // cannot kill them before they have touched a key.
            state: State::Respawned,
            elapsed: 0.0,
        }
    }

    /// Can the ship be hit right now?
    pub fn is_vulnerable(&self) -> bool {
        self.state == State::Alive
    }

    /// Should the ship be drawn and flown?
    pub fn is_flying(&self) -> bool {
        matches!(self.state, State::Alive | State::Respawned)
    }

    pub fn is_game_over(&self) -> bool {
        self.state == State::GameOver
    }

    /// Whether the blinking ship is currently visible.
    ///
    /// Always true when not invulnerable, so a caller can ask
    /// unconditionally.
    pub fn is_visible(&self) -> bool {
        match self.state {
            State::Respawned => (self.elapsed * BLINK_HZ).fract() < 0.55,
            State::Alive => true,
            State::Dead | State::GameOver => false,
        }
    }

    /// Take a hit. Returns true if the ship actually died.
    ///
    /// Hitting an invulnerable or already-dead ship does nothing, so a
    /// caller may report every collision it finds without first checking.
    pub fn hit(&mut self) -> bool {
        if !self.is_vulnerable() {
            return false;
        }
        self.state = State::Dead;
        self.elapsed = 0.0;
        // ⚠️ THE LIFE IS SPENT AT THE MOMENT OF DEATH, not at respawn.
        // Deciding at respawn means a player who quits on the death
        // screen sees a life they never got to use, and it makes
        // "lives remaining" mean two different things during the pause.
        self.remaining = self.remaining.saturating_sub(1);
        true
    }

    pub fn step(&mut self, dt: f32) {
        self.elapsed += dt;
        match self.state {
            State::Dead => {
                if self.elapsed >= DEATH_SECONDS {
                    self.elapsed = 0.0;
                    self.state = if self.remaining == 0 {
                        State::GameOver
                    } else {
                        State::Respawned
                    };
                }
            }
            State::Respawned => {
                if self.elapsed >= INVULNERABLE_SECONDS {
                    self.elapsed = 0.0;
                    self.state = State::Alive;
                }
            }
            State::Alive | State::GameOver => {}
        }
    }

    /// Start over.
    pub fn reset(&mut self) {
        *self = Lives::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_game_has_three_lives_and_starts_protected() {
        let l = Lives::new();
        assert_eq!(l.remaining, STARTING_LIVES);
        assert!(!l.is_vulnerable(), "a fresh ship must not be killable at once");
        assert!(l.is_flying());
    }

    #[test]
    fn a_hit_costs_a_life_and_the_ship_comes_back() {
        let mut l = Lives::new();
        l.step(INVULNERABLE_SECONDS + 0.01);
        assert!(l.is_vulnerable(), "protection should have worn off");

        assert!(l.hit(), "a vulnerable ship must die when hit");
        assert_eq!(l.remaining, 2);
        assert!(!l.is_flying(), "a dead ship is not flying");

        l.step(DEATH_SECONDS + 0.01);
        assert!(l.is_flying(), "it should be back");
        assert!(!l.is_vulnerable(), "and briefly protected");
    }

    /// ★ THE POINT OF THE INVULNERABILITY. Respawning into the fire that
    /// killed you must not take the next life immediately.
    #[test]
    fn a_respawned_ship_cannot_be_killed_instantly() {
        let mut l = Lives::new();
        l.step(INVULNERABLE_SECONDS + 0.01);
        l.hit();
        l.step(DEATH_SECONDS + 0.01);

        assert!(!l.hit(), "a respawned ship must shrug it off");
        assert_eq!(l.remaining, 2, "and it must not cost a life");

        l.step(INVULNERABLE_SECONDS + 0.01);
        assert!(l.hit(), "once protection lapses it must be killable again");
        assert_eq!(l.remaining, 1);
    }

    #[test]
    fn running_out_of_lives_is_game_over() {
        let mut l = Lives::new();
        for expect in [2u32, 1, 0] {
            l.step(INVULNERABLE_SECONDS + 0.01);
            assert!(l.hit());
            assert_eq!(l.remaining, expect);
            l.step(DEATH_SECONDS + 0.01);
        }
        assert!(l.is_game_over(), "three deaths must end the game");
        assert!(!l.is_flying());
        assert!(!l.hit(), "a finished game cannot lose another life");
    }

    /// ⚠️ THE LIFE IS SPENT AT DEATH, NOT AT RESPAWN — so the count never
    /// reads differently during the pause than it will afterwards.
    #[test]
    fn the_life_is_spent_immediately() {
        let mut l = Lives::new();
        l.step(INVULNERABLE_SECONDS + 0.01);
        l.hit();
        assert_eq!(l.remaining, 2, "the count must drop at the moment of death");
        l.step(DEATH_SECONDS * 0.5);
        assert_eq!(l.remaining, 2, "and not drop again at respawn");
    }

    #[test]
    fn an_invulnerable_ship_blinks_and_a_normal_one_does_not() {
        let mut l = Lives::new();
        let mut seen_on = false;
        let mut seen_off = false;
        for _ in 0..120 {
            l.step(1.0 / 60.0);
            if l.state != State::Respawned {
                break;
            }
            if l.is_visible() {
                seen_on = true;
            } else {
                seen_off = true;
            }
        }
        assert!(seen_on && seen_off, "the protected ship did not blink");

        l.step(INVULNERABLE_SECONDS + 0.01);
        assert_eq!(l.state, State::Alive);
        for _ in 0..30 {
            l.step(1.0 / 60.0);
            assert!(l.is_visible(), "a normal ship must not blink");
        }
    }

    #[test]
    fn a_dead_ship_is_not_drawn() {
        let mut l = Lives::new();
        l.step(INVULNERABLE_SECONDS + 0.01);
        l.hit();
        assert!(!l.is_visible(), "an exploded ship must not be on screen");
    }
}
