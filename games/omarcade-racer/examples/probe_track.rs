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
#[path = "../src/pace.rs"]
mod pace;

use drive::{Drive, Surface, Tuning};
use pace::Pacer;
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

    // Drive it with the reference driver: brake only when the bend ahead
    // demands it, otherwise flat out. That is the strategy the game asks
    // the player to learn, and `pace` is the one place it is defined —
    // this probe used to carry its own copy, which clamped the holdable
    // speed before applying its margin and so never exceeded 78% of top
    // speed anywhere. It reported that as "23% of the lap braking".
    let pacer = Pacer::EXACT;
    println!("\n  driving the lap  (pace::Pacer, margin {:.2})\n", pacer.margin);
    let trace = std::env::args().any(|a| a == "--trace");
    let trace_off = std::env::args().any(|a| a == "--off");
    let mut car = Drive::new();
    car.speed = tuning.top_speed;
    let mut t = 0.0f32;
    let mut off = 0.0f32;
    let mut braking = 0.0f32;
    let mut slowest = tuning.top_speed;
    let lap = road.segment_count() as f32 * road.segment_length();

    let mut travelled = 0.0f32;
    while travelled < lap {
        let target = pacer.target(&car, &road, &tuning);
        let before = car.speed;
        let inputs = pacer.step(&mut car, &road, &tuning, DT);
        if inputs.brake > 0.0 { braking += DT; }
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
