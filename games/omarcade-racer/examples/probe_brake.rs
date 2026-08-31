//! What the brake actually does, in seconds and in distance.
//!
//! The test suite proves the brake matches its stated duration. That is
//! not the same question as whether it is USEFUL — a brake that stops the
//! car in the stated time can still be worthless if the car travels
//! further than the corner it was trying to make.
//!
//! So this measures the two numbers a driver actually experiences:
//! how long it takes to stop, and HOW FAR the car travels doing it —
//! the second expressed in the units the player reads the road in
//! (segments, and fractions of the visible distance).
//!
//!   cargo run -p omarcade-racer --example probe_brake

#[path = "../src/road.rs"]
mod road;
#[path = "../src/drive.rs"]
mod drive;

use drive::{Drive, Tuning};
use road::Road;

const DT: f32 = 1.0 / 240.0;

fn main() {
    let track = Road::straight(400);
    let tuning = Tuning::from_corner(&track, 1.5);
    let visible = track.draw_distance() as f32 * track.segment_length();

    println!("\n  the brake, measured\n");
    println!("    top speed        {:>10.0} units/s", tuning.top_speed);
    println!("    accel time       {:>10.2} s   (standstill to top)", tuning.accel_time);
    println!("    brake time       {:>10.2} s   (top to standstill)", tuning.brake_time);
    println!("    visible ahead    {:>10.0} units = {} segments",
        visible, track.draw_distance());
    println!();

    // How each input sheds speed, from top, to a stop or a floor.
    println!("    {:<12} {:>9} {:>12} {:>10} {:>12}",
        "input", "to stop", "distance", "segments", "of visible");
    for (label, throttle, brake) in [
        ("brake", 0.0f32, 1.0f32),
        ("coast", 0.0, 0.0),
    ] {
        let mut car = Drive::new();
        car.speed = tuning.top_speed;
        let start = car.z;
        let mut t = 0.0f32;
        while car.speed > tuning.top_speed * 0.001 && t < 30.0 {
            car.update(DT, throttle, brake, 0.0, &track, &tuning);
            t += DT;
        }
        let dist = car.z - start;
        println!("    {label:<12} {t:>8.2}s {dist:>12.0} {:>10.1} {:>11.0}%",
            dist / track.segment_length(), dist / visible * 100.0);
    }

    println!();
    println!("    THE QUESTION THIS ANSWERS: can you stop for a corner you can see?");
    println!("    A bend appears at 100% of visible. If braking distance is under");
    println!("    that, the brake is a real option. If it is over, the only strategy");
    println!("    is to never reach top speed — which is not a game.\n");

    // Partial braking: is there a reason to feather it?
    println!("    partial braking, 0.5s of input from top speed:\n");
    println!("    {:<10} {:>12} {:>14}", "brake", "speed after", "% of top");
    for b in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
        let mut car = Drive::new();
        car.speed = tuning.top_speed;
        for _ in 0..(0.5 / DT) as usize {
            car.update(DT, 0.0, b, 0.0, &track, &tuning);
        }
        println!("    {b:<10.2} {:>12.0} {:>13.0}%",
            car.speed, car.speed / tuning.top_speed * 100.0);
    }
    println!();
}
