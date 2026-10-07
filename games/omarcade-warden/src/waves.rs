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
//! ★ W3 ADDED THE BOMBER COLUMN AND THE BAITER TIMER; W4 THE POD COLUMN.
//! Swarmers have no column: they only ever come out of a Pod.

/// Landers per wave, waves 1–4. Wave 5 on repeats the last column.
///
/// ★ THE ORIGINAL'S OWN COUNTS (`WVTAB`), kept as-is. These set how LONG
/// a wave is; how CROWDED it is at any moment is [`SQUAD_ALIVE_CAP`]'s
/// job, and that is the number the smaller world changes.
pub const LANDERS: [usize; 4] = [15, 20, 20, 20];

/// Bombers per wave, waves 1–4 (the original's table: none in wave 1).
/// Placed when the wave starts, in squads of up to [`BOMBER_SQUAD`].
pub const BOMBERS: [usize; 4] = [0, 3, 4, 5];
pub const BOMBER_SQUAD: usize = 3;

/// ★ W4. Pods per wave, waves 1–4: the original's table, and Brian's spec
/// ("never more than 4"). Placed when the wave starts, like the Bombers.
pub const PODS: [usize; 4] = [0, 1, 3, 4];

/// A Bomber's constant drift, world units per second, waves 1–4: the
/// original's `TIEXV` $20/$28/$2C/$30 (1–1.5 px a frame) × 60 × 3.16.
pub const BOMBER_SPEED: [f32; 4] = [190.0, 237.0, 261.0, 284.0];

/// ★ THE BAITER TIMER — seconds until the first one, waves 1–4: the
/// original's `UFOTIM` with its starting difficulty applied (≈48/44/36/32
/// s). The Baiter exists to stop a player camping, so the clock is the
/// whole design: it tightens per wave, tightens again the longer a wave
/// runs, and panics when only a few enemies are left.
pub const BAITER_SECONDS: [f32; 4] = [48.0, 44.0, 36.0, 32.0];
/// Faster per wave after the fourth, and per [`RAMP_SECONDS`] a wave runs
/// on (the original's −4 and −12 ticks).
pub const BAITER_FASTER_PER_WAVE: f32 = 1.0;
pub const BAITER_FASTER_PER_RAMP: f32 = 3.0;
/// ★ HOW KEEN A BAITER IS TO RE-AIM (`UFOSK`). Each roll re-aims if a
/// random byte is above this — so 200 is a 55-in-256 chance (~21%). The
/// original's row, with its starting difficulty applied: 200/180/160/160
/// in waves 1–4, 8 lower per wave after (to wave 14), 12 lower every
/// [`RAMP_SECONDS`] a wave runs on, and never below 40 (~84%).
pub const BAITER_SKIP: [f32; 4] = [200.0, 180.0, 160.0, 160.0];
pub const BAITER_SKIP_PER_WAVE: f32 = 8.0;
pub const BAITER_SKIP_PER_RAMP: f32 = 12.0;
pub const BAITER_SKIP_FLOOR: f32 = 40.0;

/// The timer never goes below this.
pub const BAITER_FLOOR: f32 = 6.0;
/// With this few enemies left the timer is capped at half, then a quarter.
pub const BAITER_PANIC_HALF: usize = 8;
pub const BAITER_PANIC_QUARTER: usize = 3;
/// Never more Baiters than this at once.
pub const MAX_BAITERS: usize = 12;

/// Seconds between squads, waves 1–4 (`WAVTIM`: 30/25/20/16 ticks of
/// 0.25 s).
pub const SQUAD_SECONDS: [f32; 4] = [7.5, 6.25, 5.0, 4.0];

/// Landers per squad. Brian's spec: "Groups of 4-5". The original's
/// `WAVSIZ` is 5 in every wave.
pub const SQUAD_SIZE: usize = 5;

/// A squad arrives on its timer only while FEWER than this many Landers
/// are alive.
///
/// ★ THE ORIGINAL'S 8, COPIED — BRIAN'S CALL (2026-10-06, "A"). It was 5,
/// scaled down so our 4-screen world held the original's Landers per
/// SCREEN. But abduction PACE is set by how many Landers are hunting, not
/// by how crowded a screen is, and with 5 the second squad waited for a
/// kill. Measured, passive player, people taken by 30 s / 60 s: original
/// 3.8 / 7.0, cap 5 3.0 / 6.4, cap 8 4.0 / 7.3. The cost, accepted: ~1.7×
/// the original's Landers per screen.
pub const SQUAD_ALIVE_CAP: usize = 8;

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

/// Bombers in `wave`, and how fast they drift.
pub fn bombers(wave: u32) -> usize {
    BOMBERS[column(wave)]
}
/// Pods in `wave`.
pub fn pods(wave: u32) -> usize {
    PODS[column(wave)]
}
pub fn bomber_speed(wave: u32) -> f32 {
    BOMBER_SPEED[column(wave)]
}

/// The Baiter timer for `wave`, `seconds` into it.
pub fn baiter_seconds(wave: u32, seconds: f32) -> f32 {
    let later = wave.saturating_sub(LANDERS.len() as u32) as f32 * BAITER_FASTER_PER_WAVE;
    let ramps = (seconds.max(0.0) / RAMP_SECONDS).floor() * BAITER_FASTER_PER_RAMP;
    (BAITER_SECONDS[column(wave)] - later - ramps).max(BAITER_FLOOR)
}

/// The chance a Baiter's re-aim roll succeeds, `seconds` into `wave`.
pub fn baiter_seek(wave: u32, seconds: f32) -> f32 {
    let later = wave.clamp(LANDERS.len() as u32, PRESSURE_LAST_WAVE) - LANDERS.len() as u32;
    let ramps = (seconds.max(0.0) / RAMP_SECONDS).floor();
    let skip = (BAITER_SKIP[column(wave)]
        - later as f32 * BAITER_SKIP_PER_WAVE
        - ramps * BAITER_SKIP_PER_RAMP)
        .max(BAITER_SKIP_FLOOR);
    (255.0 - skip) / 256.0
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

/// What the director is told about the world each step.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Census {
    /// Landers alive — what the squad cap counts (the original's LNDCNT).
    pub landers: usize,
    /// Every hostile that must die for the wave to end: Landers, Mutants,
    /// Bombers, Pods, Swarmers. Not Baiters — they leave when the wave is
    /// won.
    pub hostiles: usize,
    /// Baiters alive.
    pub baiters: usize,
}

/// What the game should do this step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// Bring in this many Landers.
    Squad(usize),
    /// ★ W3. Place this many Bombers (once, at the start of a wave).
    Bombers(usize),
    /// ★ W4. Place this many Pods (once, at the start of a wave).
    Pods(usize),
    /// ★ W3. A Baiter arrives: the wave has gone on long enough.
    Baiter,
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
    /// Bombers still to place this wave.
    bombers_pending: usize,
    /// Pods still to place this wave.
    pods_pending: usize,
    /// Until the next Baiter.
    baiter_timer: f32,
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
        Self {
            wave,
            phase: Phase::Fighting,
            reserve: landers(wave),
            squad_timer: 0.0,
            elapsed: 0.0,
            bombers_pending: bombers(wave),
            pods_pending: pods(wave),
            baiter_timer: baiter_seconds(wave, 0.0),
        }
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

    /// The current [`baiter_seek`].
    pub fn baiter_seek(&self) -> f32 {
        baiter_seek(self.wave, self.elapsed)
    }

    /// Advance, told what is alive and how many Humanoids are left. At
    /// most one event per step.
    pub fn step(&mut self, dt: f32, census: Census, people: usize) -> Option<Event> {
        match self.phase {
            Phase::Fighting => {
                self.elapsed += dt;
                self.squad_timer -= dt;

                // ★ THE ORIGINAL'S TWO TRIGGERS: the timer, while the
                // world is not already crowded with Landers — or AT ONCE
                // when none are left, so a quick player is never left
                // flying an empty world waiting on a clock.
                let due = self.squad_timer <= 0.0 && census.landers < SQUAD_ALIVE_CAP;
                if self.reserve > 0 && (census.landers == 0 || due) {
                    let n = SQUAD_SIZE.min(self.reserve);
                    self.reserve -= n;
                    self.squad_timer = squad_seconds(self.wave);
                    return Some(Event::Squad(n));
                }

                if self.bombers_pending > 0 {
                    let n = std::mem::take(&mut self.bombers_pending);
                    return Some(Event::Bombers(n));
                }
                if self.pods_pending > 0 {
                    let n = std::mem::take(&mut self.pods_pending);
                    return Some(Event::Pods(n));
                }

                let left = census.hostiles + self.reserve;
                if left == 0 {
                    self.phase = Phase::Tally { counted: 0, survivors: people, timer: TALLY_LEAD };
                    return Some(Event::Cleared);
                }

                // ★ THE BAITER CLOCK: shortened by the wave and by how long
                // it has run, and capped harder as the last few enemies
                // are hunted down — the original's panic, which is exactly
                // when a player is tempted to slow down.
                let base = baiter_seconds(self.wave, self.elapsed);
                self.baiter_timer -= dt;
                if left <= BAITER_PANIC_QUARTER {
                    self.baiter_timer = self.baiter_timer.min(base / 4.0);
                } else if left <= BAITER_PANIC_HALF {
                    self.baiter_timer = self.baiter_timer.min(base / 2.0);
                }
                if self.baiter_timer <= 0.0 && census.baiters < MAX_BAITERS {
                    self.baiter_timer = if left < 4 { base / 4.0 } else { base };
                    return Some(Event::Baiter);
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

    /// A world with `n` Landers in it and nothing else.
    fn c(n: usize) -> Census {
        Census { landers: n, hostiles: n, baiters: 0 }
    }

    /// Run until an event, with `alive` and `people` fixed.
    fn until_event(d: &mut Director, alive: usize, people: usize, max_seconds: f32) -> Option<Event> {
        let mut t = 0.0;
        while t < max_seconds {
            if let Some(e) = d.step(DT, c(alive), people) {
                return Some(e);
            }
            t += DT;
        }
        None
    }

    #[test]
    fn the_first_squad_arrives_on_the_first_step() {
        let mut d = Director::new();
        assert_eq!(d.step(DT, c(0), 10), Some(Event::Squad(SQUAD_SIZE)));
        assert_eq!(d.reserve(), LANDERS[0] - SQUAD_SIZE);
    }

    /// ★ THE TIMER WAITS FOR ROOM. A crowded world gets no reinforcements
    /// however long the wave runs; one that thins out gets them on time.
    #[test]
    fn a_squad_waits_for_its_timer_and_for_room() {
        let mut d = Director::new();
        d.step(DT, c(0), 10);

        // Crowded: never.
        assert_eq!(until_event(&mut d, SQUAD_ALIVE_CAP, 10, 30.0), None);

        // Room, and the timer long since run: at once.
        assert_eq!(d.step(DT, c(SQUAD_ALIVE_CAP - 1), 10), Some(Event::Squad(SQUAD_SIZE)));

        // Room, but the timer was just reset: not before it runs out.
        let mut t = 0.0;
        loop {
            if d.step(DT, c(SQUAD_ALIVE_CAP - 1), 10).is_some() {
                break;
            }
            t += DT;
        }
        assert!((t - SQUAD_SECONDS[0]).abs() < 0.05, "squad came after {t:.2} s");
    }

    #[test]
    fn an_empty_world_brings_the_next_squad_at_once() {
        let mut d = Director::new();
        d.step(DT, c(0), 10);
        assert_eq!(d.step(DT, c(0), 10), Some(Event::Squad(SQUAD_SIZE)));
    }

    /// The whole of wave 1, start to finish, counting every Lander.
    #[test]
    fn a_wave_delivers_exactly_its_landers_then_clears_and_counts() {
        let mut d = Director::new();
        let mut arrived = 0;
        let mut events = Vec::new();
        for _ in 0..(240 * 120) {
            // The player kills everything the moment it arrives, so the
            // director always sees an empty world.
            match d.step(DT, c(0), 7) {
                Some(Event::Squad(n)) => arrived += n,
                Some(e) => events.push(e),
                None => {}
            }
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
            d.step(DT, c(0), 4);
        }
        assert_eq!(d.step(DT, c(0), 4), Some(Event::Cleared));
        let mut ticks = 0;
        for _ in 0..(240 * 10) {
            if d.step(DT, c(0), 0) == Some(Event::BonusTick) {
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
        d.step(DT, c(0), 10);
        until_event(&mut d, SQUAD_ALIVE_CAP, 10, 40.0);
        assert!(d.pressure() > 1.0);
        while d.wave() == 1 {
            d.step(DT, c(0), 0);
        }
        assert_eq!(d.pressure(), pressure(2, 0.0));
    }

    // ----- W3: Bombers and the Baiter clock -----

    /// Run until an event that is not a squad, with a fixed census.
    fn until(d: &mut Director, census: Census, max_seconds: f32, want: Event) -> Option<f32> {
        let mut t = 0.0;
        while t < max_seconds {
            if d.step(DT, census, 10) == Some(want) {
                return Some(t);
            }
            t += DT;
        }
        None
    }

    /// Wave 1 has no Bombers; wave 2 places its three exactly once.
    #[test]
    fn bombers_arrive_once_at_the_start_of_their_wave() {
        let mut d = Director::new();
        let mut seen = Vec::new();
        for _ in 0..(240 * 120) {
            match d.step(DT, c(0), 5) {
                Some(Event::Bombers(n)) => seen.push((d.wave(), n)),
                Some(Event::NextWave) if d.wave() == 3 => break,
                _ => {}
            }
        }
        assert_eq!(seen, vec![(2, BOMBERS[1])]);
    }

    /// ★ THE BAITER'S RE-AIM CHANCE IS THE ORIGINAL'S: 55 in 256 at the
    /// start of wave 1, keener the longer a wave runs and the later the
    /// wave, and never past the floor.
    #[test]
    fn a_baiter_grows_keener_to_re_aim() {
        assert_eq!(baiter_seek(1, 0.0), 55.0 / 256.0);
        assert_eq!(baiter_seek(1, 10.0), 67.0 / 256.0);
        assert_eq!(baiter_seek(4, 0.0), 95.0 / 256.0);
        assert_eq!(baiter_seek(6, 0.0), 111.0 / 256.0);
        assert_eq!(baiter_seek(30, 0.0), baiter_seek(14, 0.0));
        assert_eq!(baiter_seek(14, 1000.0), (255.0 - BAITER_SKIP_FLOOR) / 256.0);
    }

    /// ★ W4: wave 1 has no Pods; waves 2, 3 and 4 place 1, 3 and 4 —
    /// each exactly once, at the start.
    #[test]
    fn pods_arrive_once_at_the_start_of_their_wave() {
        let mut d = Director::new();
        let mut seen = Vec::new();
        for _ in 0..(240 * 240) {
            match d.step(DT, c(0), 5) {
                Some(Event::Pods(n)) => seen.push((d.wave(), n)),
                Some(Event::NextWave) if d.wave() == 5 => break,
                _ => {}
            }
        }
        assert_eq!(seen, vec![(2, 1), (3, 3), (4, 4)]);
    }

    /// ★ THE SQUAD CAP COUNTS LANDERS ONLY. Five Bombers drifting about
    /// must not hold back the Landers' reinforcements.
    #[test]
    fn bombers_do_not_block_lander_squads() {
        let mut d = Director::new();
        d.step(DT, c(0), 10);
        let busy = Census { landers: 2, hostiles: 7, baiters: 0 };
        let t = until(&mut d, busy, 20.0, Event::Squad(SQUAD_SIZE));
        assert!(t.is_some(), "Bombers blocked the next squad");
    }

    /// ★ THE FIRST BAITER COMES AT ~48 s in wave 1 if the wave drags on.
    #[test]
    fn the_first_baiter_comes_when_the_clock_runs_out() {
        let mut d = Director::new();
        d.step(DT, c(0), 10);
        let crowded = Census { landers: 5, hostiles: 15, baiters: 0 };
        let t = until(&mut d, crowded, 60.0, Event::Baiter).expect("no Baiter in a minute");
        assert!((t - BAITER_SECONDS[0]).abs() < 0.5, "first Baiter at {t:.1} s");
    }

    /// ★ PANIC: with three enemies left the clock is a quarter — the
    /// moment a player is tempted to slow down is the moment it bites.
    #[test]
    fn the_baiter_clock_panics_when_few_enemies_remain() {
        let mut d = Director::new();
        while d.reserve() > 0 {
            d.step(DT, c(0), 10);
        }
        let nearly = Census { landers: 3, hostiles: 3, baiters: 0 };
        let t = until(&mut d, nearly, 60.0, Event::Baiter).expect("no Baiter");
        assert!(t <= BAITER_SECONDS[0] / 4.0 + 0.1, "panic Baiter only after {t:.1} s");
    }

    #[test]
    fn never_more_than_twelve_baiters() {
        let mut d = Director::new();
        d.step(DT, c(0), 10);
        let full = Census { landers: 5, hostiles: 15, baiters: MAX_BAITERS };
        assert_eq!(until(&mut d, full, 120.0, Event::Baiter), None);
    }

    /// ★ BAITERS NEVER HOLD A WAVE OPEN: with only Baiters left, it is won.
    #[test]
    fn a_wave_with_only_baiters_left_is_cleared() {
        let mut d = Director::new();
        while d.reserve() > 0 {
            d.step(DT, c(0), 10);
        }
        let only_baiters = Census { landers: 0, hostiles: 0, baiters: 4 };
        assert_eq!(d.step(DT, only_baiters, 10), Some(Event::Cleared));
    }

    #[test]
    fn the_baiter_clock_tightens_by_wave_and_by_time() {
        assert!(baiter_seconds(2, 0.0) < baiter_seconds(1, 0.0));
        assert!(baiter_seconds(6, 0.0) < baiter_seconds(4, 0.0));
        assert!(baiter_seconds(1, 30.0) < baiter_seconds(1, 0.0));
        assert_eq!(baiter_seconds(40, 1000.0), BAITER_FLOOR);
    }
}
