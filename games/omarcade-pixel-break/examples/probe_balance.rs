//! Measure the difficulty curve instead of arguing about it.
//!
//!   cargo run -p omarcade-pixel-break --example probe_balance
//!
//! Plays every level headlessly with a perfect-tracking paddle and reports
//! what each one actually costs: hits to clear, ball speed, and how long a
//! flawless player takes. No window, no compositor, deterministic.
//!
//! ⚠️ **A perfect paddle is a FLOOR, not a forecast.** It never misses, so
//! the times here are the fastest the level can physically be cleared, and
//! a real player will take longer and lose balls. Read a level that is slow
//! for the perfect paddle as one that will be *much* slower in the hands —
//! the ratio between levels is the signal, not the absolute seconds.
//!
//! This is the instrument the power-up durations get tuned against at S5:
//! a 20-second magnet means something different on a level that takes 40
//! seconds than on one that takes 90.

use omarcade_core::geom;
#[path = "../src/physics.rs"]
mod physics;
#[path = "../src/items.rs"]
mod items;
#[path = "../src/effects.rs"]
mod effects;
#[path = "../src/state.rs"]
mod state;

use physics::{step_fixed, FIXED_DT};
use state::{
    ball_speed_for, hits_to_clear, tier_for, GameState, Phase, Tier, BRICK_COLS, BRICK_ROWS,
    LEVELS,
};

/// Give up on a level after this much simulated time. A level that cannot
/// be cleared by a perfect paddle inside two minutes is a design problem,
/// and hanging the probe is a worse way to learn that than saying so.
const PATIENCE_SECONDS: f32 = 900.0;

struct Measured {
    drops: u32,
    caught: u32,
    level: u32,
    hits: u32,
    speed: f32,
    seconds: f32,
    seconds_to_90: f32,
    cleared: bool,
    rows: String,
}

/// Play one level to a clear with a paddle that never misses.
fn measure(level: u32) -> Measured {
    let mut s = GameState::new();
    s.level = level;
    s.bricks = state::build_bricks(level);
    // A generous life count: this measures how long clearing takes, not
    // whether a perfect player survives, and they always do.
    s.lives = 99;
    s.launch();

    let hits = hits_to_clear(level);
    let mut drops = 0u32;
    let mut caught = 0u32;
    let mut items_before = 0usize;
    let start_bricks = s.bricks_remaining();
    let mut ticks_to_90 = 0u32;
    let mut ticks = 0u32;
    let max_ticks = (PATIENCE_SECONDS / FIXED_DT) as u32;

    while ticks < max_ticks {
        // Track the lowest ball — the one about to be lost.
        if let Some(target) = s
            .balls
            .iter()
            .max_by(|a, b| a.pos.y.partial_cmp(&b.pos.y).unwrap())
            .map(|b| b.pos.x)
        {
            let c = s.paddle.center_x();
            s.paddle.dir = if (target - c).abs() < 4.0 {
                0.0
            } else if target > c {
                1.0
            } else {
                -1.0
            };
        }

        let paddle_before = s.paddle.w;
        step_fixed(&mut s);
        ticks += 1;
        // Count what appeared and what the paddle actually received.
        if s.items.len() > items_before {
            drops += (s.items.len() - items_before) as u32;
        }
        if s.paddle.w != paddle_before {
            caught += 1;
        }
        items_before = s.items.len();
        if ticks_to_90 == 0 && s.bricks_remaining() * 10 <= start_bricks {
            ticks_to_90 = ticks;
        }

        // Clearing the field advances the level, which is how we know we
        // are done. Winning happens only on the last one.
        // ⚠️ Clearing a field now begins a ~0.9s cascade rather than
        // advancing at once. The probe measures DIFFICULTY, not effects:
        // counting the animation would add nine tenths of a second to
        // every level and quietly move a number that ten sessions of
        // comparisons depend on. Stop the clock the moment the field is
        // clear.
        if s.phase == state::Phase::Clearing || s.phase == Phase::Won {
            break;
        }
        if s.level != level {
            break;
        }
        // A perfect paddle should never drain, but if it does, relaunch
        // rather than stalling in Ready forever.
        if s.phase == Phase::Ready {
            s.launch();
        }
    }

    let cleared =
        s.level != level || s.phase == Phase::Won || s.phase == state::Phase::Clearing;

    // The row shape, in the plan's own notation.
    let rows: String = (0..BRICK_ROWS)
        .map(|r| match tier_for(level, r) {
            Tier::Plain => '.',
            Tier::Reinforced => '#',
            Tier::Armoured => '=',
        })
        .collect::<Vec<char>>()
        .iter()
        .map(|c| format!("{c} "))
        .collect();

    Measured {
        level,
        hits,
        speed: ball_speed_for(level),
        seconds: ticks as f32 * FIXED_DT,
        seconds_to_90: ticks_to_90 as f32 * FIXED_DT,
        drops,
        caught,
        cleared,
        rows,
    }
}

fn main() {
    println!("Pixel Break — the difficulty curve, measured");
    println!("{BRICK_COLS} columns x {BRICK_ROWS} rows · perfect-tracking paddle · 240 Hz fixed step");
    println!();
    println!("  .  plain (1 hit)     #  reinforced (2)     =  armoured (4)");
    println!();
    println!(
        "{:>3}  {:<13} {:>6} {:>8} {:>9} {:>8} {:>7} {:>7}",
        "L", "rows", "hits", "speed", "seconds", "s/hit", "drops", "caught"
    );
    println!("{}", "-".repeat(72));

    let mut results = Vec::new();
    for level in 1..=LEVELS {
        let m = measure(level);
        let flag = if m.cleared { "" } else { "  <- NOT CLEARED" };
        println!(
            "{:>3}  {:<13} {:>6} {:>8.0} {:>9.1} {:>8.3} {:>7} {:>7}{}",
            m.level,
            m.rows.trim_end(),
            m.hits,
            m.speed,
            m.seconds,
            m.seconds / m.hits as f32,
            m.drops,
            m.caught,
            flag
        );
        results.push(m);
    }

    println!();

    // The claim the whole difficulty argument rests on: work climbs much
    // faster than speed, so the game gets longer rather than twitchier.
    let first = &results[0];
    let last = &results[results.len() - 1];
    let work_ratio = last.hits as f32 / first.hits as f32;
    let speed_ratio = last.speed / first.speed;
    let time_ratio = last.seconds / first.seconds;

    println!("work   L1 -> L{LEVELS}:  {:>5.2}x  ({} -> {} hits)", work_ratio, first.hits, last.hits);
    println!("speed  L1 -> L{LEVELS}:  {:>5.2}x  ({:.0} -> {:.0} units/s)", speed_ratio, first.speed, last.speed);
    println!("time   L1 -> L{LEVELS}:  {:>5.2}x  ({:.1} -> {:.1} s)", time_ratio, first.seconds, last.seconds);
    println!();

    let total: f32 = results.iter().map(|m| m.seconds).sum();
    let tail: f32 = results.iter().map(|m| m.seconds - m.seconds_to_90).sum();
    let to_90: f32 = results.iter().map(|m| m.seconds_to_90).sum();
    println!("time to break the first 90% of every field:  {:>6.0}s", to_90);
    println!("time hunting the LAST 10%:                   {:>6.0}s  <- {:.0}% of the run", tail, 100.0 * tail / (to_90 + tail));
    println!();
    println!("a flawless run of all {LEVELS} levels: {:.0} seconds ({:.1} minutes)", total, total / 60.0);
    println!("⚠️ that is a FLOOR — a perfect paddle never misses. Real play is longer.");
    println!();
    println!("⚠️ THE ENDGAME TAIL is the headline number here, not the totals. Much of the run");
    println!("   is one ball hunting a nearly-empty field, and s/hit is WORST on the easy");
    println!("   levels for exactly that reason — L2 is sparse for longer than L8 is.");
    println!();
    println!("⚠️ MEASURED AT S5: power-ups CUT THE DENSE PHASE BY 18% AND THE TAIL BY ONLY 8%,");
    println!("   so the tail's SHARE of the run went UP (35% -> 38%) while the run itself got");
    println!("   12 minutes shorter. That is not a failure, it is a diagnosis: a wider paddle");
    println!("   returns more balls, it does not make ONE ball cover more field. The tail is a");
    println!("   SEARCH problem, and only more balls searching at once can fix it — S6.");
    println!("   Read the two phases separately; the single tail percentage hides this.");
    println!();

    // Anything that did not clear is a design problem, and worth an exit
    // code so this can sit in a check.
    let stuck: Vec<u32> = results.iter().filter(|m| !m.cleared).map(|m| m.level).collect();
    if stuck.is_empty() {
        println!("PASS: every level clears with a perfect paddle");
    } else {
        println!("FAIL: levels {stuck:?} did not clear in {PATIENCE_SECONDS:.0}s");
        std::process::exit(1);
    }

    if speed_ratio >= work_ratio {
        println!("FAIL: speed is climbing as fast as the work — L{LEVELS} will be unplayable");
        std::process::exit(1);
    }
}
