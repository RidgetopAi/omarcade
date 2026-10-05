//! Waves: who arrives when, when a wave is over, and what surviving it
//! is worth.
//!
//! ★ W1 OF docs/warden-plan.md (was S9). Until this existed, five Landers
//! were placed once and nothing happened when they were all dead — the
//! game had no second minute. The director turns it into the arcade's
//! loop: squads arrive, the wave is held or lost, the people who
//! survived are counted, and the next wave is harder.
//!
//! ★ EVERY TUNABLE IS A NAMED CONSTANT IN THIS FILE, in the same shape
//! as the original's wave table (`WVTAB`, docs/warden/research-gameplay.md
//! §2). Retuning a wave should never mean reading the code that runs it.
//!
//! ⚠️ THE DIRECTOR IS PURE. It is told how many hostiles are alive and
//! how many people are left, and it answers with an [`Event`]; it never
//! touches an enemy, a Humanoid or the score. That keeps every rule here
//! testable without a world, and keeps the one place that CAN see the
//! world — `main` — the one place that changes it.
//!
//! ⚠️ BOMBERS AND PODS ARE NOT IN THE TABLE YET. The original's wave 2 is
//! 20 Landers + 3 Bombers + 1 Pod; those columns arrive with the enemies
//! themselves in W3/W4, for the same reason `Kind` has no empty variants:
//! a count for a thing that cannot exist is a number that pretends.

/// Landers per wave, waves 1–4. Wave 5 on repeats the last column.
///
/// ★ THE ORIGINAL'S OWN COUNTS (`WVTAB`), kept as-is. These set how LONG
/// a wave is; how CROWDED it is at any moment is [`SQUAD_ALIVE_CAP`]'s
/// job, and that is the number the smaller world changes.
pub const LANDERS: [usize; 4] = [15, 20, 20, 20];

/// Seconds between squads, waves 1–4 (`WAVTIM`: 30/25/20/16 ticks of
/// 0.25 s).
pub const SQUAD_SECONDS: [f32; 4] = [7.5, 6.25, 5.0, 4.0];

/// Landers per squad. Brian's spec: "Groups of 4-5". The original's
/// `WAVSIZ` is 5 in every wave.
pub const SQUAD_SIZE: usize = 5;

/// A squad arrives on its timer only while FEWER than this many hostiles
/// are alive.
///
/// ⚠️ SCALED FOR OUR WORLD, NOT COPIED. The original caps at 8 in a world
/// ~6.7 screens round; ours is 4 (`world::WORLD_SCREENS`). Same lap time
/// (6 s here, ~5.7 s there), so the same count would be ~1.7× as many
/// enemies per screen. 8 × 4 / 6.7 = 4.8 → 5, which keeps the per-screen
/// density the original had (1.19 → 1.25). Brian's to retune by feel.
pub const SQUAD_ALIVE_CAP: usize = 5;

/// How often the pressure ramps within a wave, in seconds (the
/// original's intra-wave delta: every 40 ticks ≈ 10 s).
pub const RAMP_SECONDS: f32 = 10.0;

/// Pressure added per wave after the first.
///
/// From the original's Lander X speed: +2 per wave against an effective
/// wave-1 value of 32 → 1/16 per wave.
pub const PRESSURE_PER_WAVE: f32 = 0.0625;

/// The last wave that adds pressure. The original's difficulty ceiling
/// (15 deltas, 5 of them already spent on wave 1) stops growth here.
pub const PRESSURE_LAST_WAVE: u32 = 14;

/// Pressure added each [`RAMP_SECONDS`] a wave runs on.
pub const PRESSURE_PER_RAMP: f32 = 0.04;

/// The most a long wave can add on top of its starting pressure.
///
/// ⚠️ CAPPED HERE AND NOT IN THE ORIGINAL'S WAY (per-parameter MAX/MIN),
/// because one shared multiplier needs one ceiling. 0.2 is five ramps:
/// a wave dragged out past 50 s is as hard as it gets.
pub const PRESSURE_RAMP_CAP: f32 = 0.2;

/// The ceiling on pressure overall.
///
/// ★ CHOSEN SO A MUTANT NEVER OUTRUNS THE SHIP: 290 u/s × 2.0 = 580,
/// under the ship's 640 top speed. Fleeing must stay an answer.
pub const MAX_PRESSURE: f32 = 2.0;

/// Bonus per surviving Humanoid is this × the wave number…
pub const BONUS_STEP: u32 = 100;
/// …up to this wave, after which it stays at 500.
pub const BONUS_CAP_WAVE: u32 = 5;

/// Every this-many waves the planet is rebuilt and repopulated.
pub const RESTORE_EVERY: u32 = 5;

/// Silence between the last kill and the first bonus tick, seconds.
pub const TALLY_LEAD: f32 = 1.0;
/// Between bonus ticks: one Humanoid counted per tick. The original
/// draws one every 4 frames; at 60 Hz that is too fast to read on a
/// screen this size, so it is slowed to something you can count along.
pub const TALLY_TICK: f32 = 0.18;
/// The hold after the tally, before the next wave (the original's 128
/// frames ≈ 2 s).
pub const TALLY_HOLD: f32 = 2.0;

/// Landers in `wave`.
pub fn landers(wave: u32) -> usize {
    LANDERS[column(wave)]
}

/// Seconds between squads in `wave`.
pub fn squad_seconds(wave: u32) -> f32 {
    SQUAD_SECONDS[column(wave)]
}

/// What each surviving Humanoid is worth at the end of `wave`.
pub fn bonus_per_humanoid(wave: u32) -> u32 {
    BONUS_STEP * wave.clamp(1, BONUS_CAP_WAVE)
}

/// Does `wave` start with the planet rebuilt?
pub fn restores(wave: u32) -> bool {
    wave > 1 && wave % RESTORE_EVERY == 0
}

/// How hard the enemy is pushing, `seconds` into `wave`. Exactly 1.0 at
/// the start of wave 1.
///
/// ★ ONE MULTIPLIER, NOT A TABLE PER PARAMETER. Enemy speeds are
/// multiplied by it and fire intervals divided by it. The original ramps
/// a dozen parameters separately; one shared number keeps their
/// relationships — a Mutant stays faster than a Lander, a Lander stays
/// gentler to dodge than a Mutant — at every wave.
///
/// ⚠️ 1.0 AT WAVE 1, SECOND 0 IS LOAD-BEARING: the S5–S7 numbers Brian
/// tuned are what wave 1 opens with, unchanged.
pub fn pressure(wave: u32, seconds: f32) -> f32 {
    let waves = wave.clamp(1, PRESSURE_LAST_WAVE) - 1;
    let ramps = (seconds.max(0.0) / RAMP_SECONDS).floor();
    let within = (ramps * PRESSURE_PER_RAMP).min(PRESSURE_RAMP_CAP);
    (1.0 + waves as f32 * PRESSURE_PER_WAVE + within).min(MAX_PRESSURE)
}

fn column(wave: u32) -> usize {
    (wave.max(1) as usize - 1).min(LANDERS.len() - 1)
}

/// Where in a wave the director is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase {
    /// Squads are arriving and being fought.
    Fighting,
    /// The wave is held: the survivors are being counted, one per tick.
    Tally {
        /// How many have been counted so far.
        counted: usize,
        /// How many there are to count — fixed at the moment the wave
        /// was cleared, so a person dying in the pause cannot shrink a
        /// bonus that is already on screen.
        survivors: usize,
        /// Until the next tick.
        timer: f32,
    },
    /// Counted; holding before the next wave.
    Hold { counted: usize, timer: f32 },
}

/// What the game should do this step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// Bring in this many Landers.
    Squad(usize),
    /// Every hostile is dead and none are left to come. The wave is held.
    Cleared,
    /// One more survivor counted: award [`bonus_per_humanoid`].
    BonusTick,
    /// The next wave begins. [`Director::wave`] is already the new one.
    NextWave,
}

#[derive(Debug, Clone)]
pub struct Director {
    wave: u32,
    phase: Phase,
    /// Landers of this wave still to arrive.
    reserve: usize,
    /// Until the next squad is due.
    squad_timer: f32,
    /// Seconds this wave has been fought, for the pressure ramp.
    elapsed: f32,
}

impl Default for Director {
    fn default() -> Self {
        Self::new()
    }
}

impl Director {
    /// Wave 1, about to begin. The first squad arrives on the first step.
    pub fn new() -> Self {
        Self::at_wave(1)
    }

    fn at_wave(wave: u32) -> Self {
        Self { wave, phase: Phase::Fighting, reserve: landers(wave), squad_timer: 0.0, elapsed: 0.0 }
    }

    pub fn wave(&self) -> u32 {
        self.wave
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// Landers of this wave not yet arrived.
    #[cfg(test)]
    pub fn reserve(&self) -> usize {
        self.reserve
    }

    /// The current [`pressure`].
    pub fn pressure(&self) -> f32 {
        pressure(self.wave, self.elapsed)
    }

    /// Advance. `alive` is every hostile still in the world, `people`
    /// how many Humanoids are left. At most one event per step.
    pub fn step(&mut self, dt: f32, alive: usize, people: usize) -> Option<Event> {
        match self.phase {
            Phase::Fighting => {
                self.elapsed += dt;
                self.squad_timer -= dt;

                // ★ THE ORIGINAL'S TWO TRIGGERS: the timer, while the
                // world is not already crowded — or AT ONCE when nobody is
                // left, so a quick player is never left flying an empty
                // world waiting on a clock.
                let due = self.squad_timer <= 0.0 && alive < SQUAD_ALIVE_CAP;
                if self.reserve > 0 && (alive == 0 || due) {
                    let n = SQUAD_SIZE.min(self.reserve);
                    self.reserve -= n;
                    self.squad_timer = squad_seconds(self.wave);
                    return Some(Event::Squad(n));
                }

                if self.reserve == 0 && alive == 0 {
                    self.phase = Phase::Tally { counted: 0, survivors: people, timer: TALLY_LEAD };
                    return Some(Event::Cleared);
                }
                None
            }

            Phase::Tally { counted, survivors, timer } => {
                let timer = timer - dt;
                if timer > 0.0 {
                    self.phase = Phase::Tally { counted, survivors, timer };
                    return None;
                }
                if counted < survivors {
                    self.phase = Phase::Tally { counted: counted + 1, survivors, timer: TALLY_TICK };
                    return Some(Event::BonusTick);
                }
                self.phase = Phase::Hold { counted, timer: TALLY_HOLD };
                None
            }

            Phase::Hold { counted, timer } => {
                let timer = timer - dt;
                if timer > 0.0 {
                    self.phase = Phase::Hold { counted, timer };
                    return None;
                }
                *self = Self::at_wave(self.wave + 1);
                Some(Event::NextWave)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 240.0;

    /// Run until an event, with `alive` and `people` fixed.
    fn until_event(d: &mut Director, alive: usize, people: usize, max_seconds: f32) -> Option<Event> {
        let mut t = 0.0;
        while t < max_seconds {
            if let Some(e) = d.step(DT, alive, people) {
                return Some(e);
            }
            t += DT;
        }
        None
    }

    #[test]
    fn the_first_squad_arrives_on_the_first_step() {
        let mut d = Director::new();
        assert_eq!(d.step(DT, 0, 10), Some(Event::Squad(SQUAD_SIZE)));
        assert_eq!(d.reserve(), LANDERS[0] - SQUAD_SIZE);
    }

    /// ★ THE TIMER WAITS FOR ROOM. A crowded world gets no reinforcements
    /// however long the wave runs; one that thins out gets them on time.
    #[test]
    fn a_squad_waits_for_its_timer_and_for_room() {
        let mut d = Director::new();
        d.step(DT, 0, 10);

        // Crowded: never.
        assert_eq!(until_event(&mut d, SQUAD_ALIVE_CAP, 10, 30.0), None);

        // Room, and the timer long since run: at once.
        assert_eq!(d.step(DT, SQUAD_ALIVE_CAP - 1, 10), Some(Event::Squad(SQUAD_SIZE)));

        // Room, but the timer was just reset: not before it runs out.
        let mut t = 0.0;
        loop {
            if d.step(DT, SQUAD_ALIVE_CAP - 1, 10).is_some() {
                break;
            }
            t += DT;
        }
        assert!((t - SQUAD_SECONDS[0]).abs() < 0.05, "squad came after {t:.2} s");
    }

    #[test]
    fn an_empty_world_brings_the_next_squad_at_once() {
        let mut d = Director::new();
        d.step(DT, 0, 10);
        assert_eq!(d.step(DT, 0, 10), Some(Event::Squad(SQUAD_SIZE)));
    }

    /// The whole of wave 1, start to finish, counting every Lander.
    #[test]
    fn a_wave_delivers_exactly_its_landers_then_clears_and_counts() {
        let mut d = Director::new();
        let mut arrived = 0;
        let mut alive = 0usize;
        let mut events = Vec::new();
        for _ in 0..(240 * 120) {
            match d.step(DT, alive, 7) {
                Some(Event::Squad(n)) => {
                    arrived += n;
                    alive += n;
                }
                Some(e) => events.push(e),
                None => {}
            }
            // The player kills everything the moment it arrives.
            alive = 0;
            if events.contains(&Event::NextWave) {
                break;
            }
        }
        assert_eq!(arrived, LANDERS[0]);
        let ticks = events.iter().filter(|e| **e == Event::BonusTick).count();
        assert_eq!(events.first(), Some(&Event::Cleared));
        assert_eq!(ticks, 7, "one bonus tick per survivor");
        assert_eq!(events.last(), Some(&Event::NextWave));
        assert_eq!(d.wave(), 2);
        assert_eq!(d.reserve(), LANDERS[1]);
    }

    /// A survivor lost during the pause does not take back a bonus the
    /// screen has already started counting.
    #[test]
    fn survivors_are_fixed_when_the_wave_is_cleared() {
        let mut d = Director::new();
        while d.reserve() > 0 {
            d.step(DT, 0, 4);
        }
        assert_eq!(d.step(DT, 0, 4), Some(Event::Cleared));
        let mut ticks = 0;
        for _ in 0..(240 * 10) {
            if d.step(DT, 0, 0) == Some(Event::BonusTick) {
                ticks += 1;
            }
        }
        assert_eq!(ticks, 4);
    }

    #[test]
    fn the_bonus_climbs_and_then_holds_at_500() {
        let per: Vec<u32> = (1..=7).map(bonus_per_humanoid).collect();
        assert_eq!(per, vec![100, 200, 300, 400, 500, 500, 500]);
    }

    #[test]
    fn the_planet_is_rebuilt_every_fifth_wave_only() {
        let r: Vec<u32> = (1..=16).filter(|&w| restores(w)).collect();
        assert_eq!(r, vec![5, 10, 15]);
    }

    #[test]
    fn later_waves_repeat_the_last_column() {
        assert_eq!(landers(1), 15);
        assert_eq!(landers(2), 20);
        assert_eq!(landers(9), 20);
        assert_eq!(squad_seconds(9), SQUAD_SECONDS[3]);
    }

    /// ★ WAVE 1 OPENS ON BRIAN'S NUMBERS: pressure 1.0 exactly, so every
    /// S5–S7 constant is what the player meets first.
    #[test]
    fn pressure_starts_at_one_and_ramps_within_and_across_waves() {
        assert_eq!(pressure(1, 0.0), 1.0);
        assert_eq!(pressure(1, 9.9), 1.0, "nothing before the first ramp");
        assert!(pressure(1, 10.0) > 1.0);
        assert!(pressure(2, 0.0) > pressure(1, 0.0));
        assert_eq!(
            pressure(1, 1000.0),
            1.0 + PRESSURE_RAMP_CAP,
            "a long wave is capped, not endless"
        );
        assert_eq!(pressure(PRESSURE_LAST_WAVE, 0.0), pressure(40, 0.0), "growth stops");
        assert!(pressure(40, 1000.0) <= MAX_PRESSURE);
    }

    /// The ramp resets with each wave rather than carrying a long wave's
    /// within-wave pressure into the next one.
    #[test]
    fn the_within_wave_ramp_resets_on_the_next_wave() {
        let mut d = Director::new();
        d.step(DT, 0, 10);
        until_event(&mut d, SQUAD_ALIVE_CAP, 10, 40.0);
        assert!(d.pressure() > 1.0);
        while d.wave() == 1 {
            d.step(DT, 0, 0);
        }
        assert_eq!(d.pressure(), pressure(2, 0.0));
    }
}
