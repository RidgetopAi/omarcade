//! Does the course ask real questions, and can it be driven?
//!
//! A track definition can be correct — right length, right bends — and
//! still be a bad course: every corner the same, no straight long enough
//! to reach top speed, a bend that arrives before the last one has been
//! recovered from. Those are properties of the SEQUENCE, and no unit test
//! on a single section can see them.
//!
//! So this drives the whole lap with a driver that brakes only when it
//! must, and reports what the lap actually demands.
//!
//!   cargo run -p omarcade-racer --example probe_track

#[path = "../src/road.rs"]
mod road;
#[path = "../src/drive.rs"]
mod drive;
#[path = "../src/track.rs"]
mod track;

use drive::{Drive, Surface, Tuning};
use track::{grand_prix, UNITS_PER_MILE};

const DT: f32 = 1.0 / 240.0;

fn main() {
    let course = grand_prix();
    let road = course.build();
    let tuning = Tuning::from_corner(&road, 1.5);
    let limit = tuning.steer_rate / tuning.centrifugal;

    println!("\n  THE COURSE\n");
    println!("    length        {:.2} miles ({} segments)",
        course.length_miles(), road.segment_count());
    println!("    top speed     {:.0} units/s", tuning.top_speed);
    println!("    limit bend    {limit:.2}");
    println!();

    // What the lap contains, corner by corner, walking the curve profile.
    println!("  what the lap contains\n");
    println!("    {:<10} {:>8} {:>10} {:>12}", "at mile", "curve", "x limit", "demands");
    let mut i = 0;
    while i < road.segment_count() {
        let c = road.curve_at(i as f32 * road.segment_length());
        if c.abs() > limit * 0.3 {
            // Walk to the peak of this corner.
            let start = i;
            let mut peak = 0.0f32;
            while i < road.segment_count() {
                let cc = road.curve_at(i as f32 * road.segment_length());
                if cc.abs() <= limit * 0.3 { break; }
                if cc.abs() > peak.abs() { peak = cc; }
                i += 1;
            }
            let mile = start as f32 * road.segment_length() / UNITS_PER_MILE;
            let demands = if peak.abs() > limit { "BRAKE" } else { "hold it" };
            let dir = if peak > 0.0 { "right" } else { "left " };
            println!("    {mile:<10.2} {:>8.2} {:>9.2}x {dir} {demands}",
                peak.abs(), peak.abs() / limit);
        } else {
            i += 1;
        }
    }

    // Drive it. The driver brakes only when the bend ahead demands it —
    // which is the strategy the game is asking the player to learn.
    println!("\n  driving the lap\n");
    let trace = std::env::args().any(|a| a == "--trace");
    let trace_off = std::env::args().any(|a| a == "--off");
    let mut car = Drive::new();
    car.speed = tuning.top_speed;
    let mut t = 0.0f32;
    let mut off = 0.0f32;
    let mut braking = 0.0f32;
    let mut slowest = tuning.top_speed;
    let lap = road.segment_count() as f32 * road.segment_length();

    // ⚠️ `car.z` WRAPS — `Road::wrap` keeps it inside the track length, so
    // `while car.z < lap` never terminates and the car simply laps forever.
    // Distance travelled has to be accumulated separately. This probe
    // reported "stuck at 0.92 miles" for exactly this reason, which looked
    // like a physics failure and was a loop condition.
    let mut travelled = 0.0f32;
    while travelled < lap && t < 600.0 {
        // Look ahead and slow for the WORST bend inside braking range.
        //
        // ⚠️ Sampling the curve at one point `speed * brake_time` ahead is
        // wrong and cost real time here: that is where the car would come
        // to a STOP, not where it needs to already be slow, and a single
        // sample steps straight over the entry of a corner that begins
        // sooner. The driver under-braked and spent five seconds a lap on
        // the grass, which read as "the course is too hard" when it was
        // the driver being short-sighted.
        //
        // Braking distance is the AVERAGE speed over the stop, so half of
        // `speed * brake_time` — and every bend within it matters, not
        // just the one at the far end.
        let look = car.speed * tuning.brake_time * 0.5;
        let mut ahead = 0.0f32;
        let steps = 12;
        for k in 0..=steps {
            let z = car.z + look * k as f32 / steps as f32;
            ahead = ahead.max(road.curve_at(z).abs());
        }
        let holdable = if ahead > 0.001 {
            (tuning.steer_rate / (tuning.centrifugal * ahead)).sqrt().min(1.0)
        } else { 1.0 };
        // ⚠️ AND THE MARGIN IS NOT COSMETIC. `holdable` is the algebra for a
        // driver at FULL LOCK, and this one steers `(-x * 3.0)` — it does
        // not reach full lock until the car is a third of the way to a
        // verge, so it genuinely cannot hold the speed the formula says.
        // At 0.9 it ran wide on the Hard corner every lap and the probe
        // reported "the course asks for more than the car can give", which
        // was a statement about the DRIVER. Same trap as the lean test:
        // a closed form solves for the driver you assumed.
        let target = tuning.top_speed * holdable * 0.78;

        let (throttle, brake) = if car.speed > target { (0.0, 1.0) } else { (1.0, 0.0) };
        if brake > 0.0 { braking += DT; }
        let correction = (-car.x * 3.0).clamp(-1.0, 1.0);
        let before = car.speed;
        car.update(DT, throttle, brake, correction, &road, &tuning);
        travelled += (before + car.speed) * 0.5 * DT;

        if car.surface() != Surface::Road {
            off += DT;
            if trace_off {
                println!("      OFF at mile {:.2}: x={:+.3} speed={:.0}% curve_here={:.2} target={:.0}%",
                    travelled/UNITS_PER_MILE, car.x, car.speed/tuning.top_speed*100.0,
                    road.curve_at(car.z).abs(), target/tuning.top_speed*100.0);
            }
        }
        slowest = slowest.min(car.speed);
        t += DT;
        if trace && (t * 240.0) as usize % 240 == 0 {
            println!("      t={t:5.1} mile={:5.2} speed={:6.0} x={:+.2} {:?}",
                travelled/UNITS_PER_MILE, car.speed, car.x, car.surface());
        }
    }

    println!("    distance covered  {:.2} of {:.2} miles", travelled/UNITS_PER_MILE, lap/UNITS_PER_MILE);
    println!("    lap time          {t:.2} s");
    println!("    time braking      {braking:.2} s  ({:.0}% of the lap)", braking/t*100.0);
    println!("    time off the road {off:.2} s");
    println!("    slowest point     {:.0} ({:.0}% of top)", slowest, slowest/tuning.top_speed*100.0);
    println!();
    if off > 0.5 {
        println!("    ⚠️  a driver who slows for every corner still spent {off:.1}s off");
        println!("       the road — the course asks for more than the car can give.");
    } else if braking < 0.5 {
        println!("    ⚠️  barely any braking. The course does not use the brake, so");
        println!("       there is no decision in it.");
    } else {
        println!("    A driver who reads the corners gets round clean, and has to");
        println!("    brake to do it. That is the lap having a question in it.");
    }
    println!();
}
