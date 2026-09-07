//! Run BOTH voices through the real core mixer, the way the game does.
//! Every layer has checked out in isolation, so the fault is in how they
//! combine — which is the one thing nothing has exercised yet.

#[path = "../src/track.rs"] mod track;
#[path = "../src/road.rs"] mod road;
#[path = "../src/drive.rs"] mod drive;
#[path = "../src/pace.rs"] mod pace;
#[path = "../src/sound.rs"] mod sound;

use omarcade_core::{AudioSystem, VoiceParams};

fn main() {
    let course = track::grand_prix();
    let r = course.build();
    let tuning = drive::Tuning::from_corner(&r, 2.6);

    let mut sys = AudioSystem::new();
    let engine = sys.register(Box::new(sound::Engine::new()));
    let tyres = sys.register(Box::new(sound::Squeal::new()));
    println!("engine = {engine:?}   tyres = {tyres:?}");

    // Feed it the same commands the game does, at a hard corner.
    let mut a = sys.handle();
    a.start(engine);
    a.start(tyres);
    a.set(engine, VoiceParams::engine(1.0));
    a.set(tyres, VoiceParams::squeal(1.0, 1.0));
    println!("dropped commands: {}", sys.dropped_commands());

    // Find the hardest bend on the real track and report its lean.
    let mut worst = 0.0f32;
    let mut z = 0.0;
    while z < r.length() {
        worst = worst.max((r.curve_at(z) / 1.8).abs());
        z += r.length() / 500.0;
    }
    println!("hardest bend on the course -> lean {worst:.3}");
    println!("squeal threshold 0.42, full by 0.78");
}
