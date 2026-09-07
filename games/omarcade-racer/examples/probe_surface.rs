//! What does the rumble strip actually produce? Print the gaps between
//! strikes so the count comes from the signal rather than from a
//! detector I have already got wrong twice.

#[path = "../src/sound.rs"] mod sound;

use omarcade_core::{Voice, VoiceParams};

fn main() {
    let sr = 48_000.0;
    for units in [16_000.0f32, 8_000.0] {
        let mut sf = sound::Surface::new(400.0);
        let mut buf = vec![0.0; sr as usize];
        sf.render(&mut buf, VoiceParams::surface(sound::SURFACE_RUMBLE, units, 1.0), sr);

        let peak = buf.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        let expect = units / 400.0;
        println!("{units:8.0} units/s: expect {expect:.0} teeth, {:.1} ms apart. peak {peak:.3}",
                 1000.0 / expect);

        // Print the envelope over the first 120 ms so the shape is
        // visible instead of inferred.
        let win = (sr * 0.001) as usize;   // 1 ms buckets
        print!("           ");
        for b in 0..120 {
            let a = &buf[b * win..(b + 1) * win];
            let e = a.iter().fold(0.0f32, |x, y| x.max(y.abs())) / peak;
            print!("{}", match (e * 9.0) as u32 { 0 => '.', 1..=2 => '_',
                   3..=5 => '-', 6..=7 => '=', _ => '#' });
        }
        println!("  (1 ms per char, 120 ms)");
    }
}
