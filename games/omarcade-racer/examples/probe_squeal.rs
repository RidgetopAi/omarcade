//! What lean does the player actually reach, and does the squeal fire?
//!
//! Brian drove the track and heard no squeal anywhere, including bends
//! that cannot be taken flat. This walks the real course at the real
//! pace and prints the lean at every segment, so the answer comes from
//! the track rather than from reading the threshold and hoping.

#[path = "../src/track.rs"]
mod track;
#[path = "../src/road.rs"]
mod road;
#[path = "../src/drive.rs"]
mod drive;
#[path = "../src/pace.rs"]
mod pace;

fn main() {
    let course = track::grand_prix();
    let r = course.build();
    let tuning = drive::Tuning::from_corner(&r, 2.6);

    println!("top_speed {:.0}  grip threshold 0.75 / full 1.05 (1.0 = the limit)", tuning.top_speed);
    println!();

    // Walk the whole lap at a few fractions of top speed.
    for frac in [1.0f32, 0.85, 0.7] {
        let speed = tuning.top_speed * frac;
        let mut worst: f32 = 0.0;
        let mut hist = [0usize; 5];
        let step = r.length() / 400.0;
        let mut z = 0.0;
        while z < r.length() {
            let authority = (speed / tuning.top_speed).clamp(0.0, 1.0);
            let lean = (r.curve_at(z).abs() * authority / 1.0).min(4.0);
            worst = worst.max(lean);
            let b = if lean < 0.4 { 0 } else if lean < 0.75 { 1 }
                    else if lean < 0.9 { 2 } else if lean < 1.05 { 3 } else { 4 };
            hist[b] += 1;
            z += step;
        }
        println!("at {:.0}% of top speed:  worst lean {:.3}", frac * 100.0, worst);
        println!("   grip <0.40 (silent)      {:3} samples", hist[0]);
        println!("   0.40-0.75 (still silent) {:3}", hist[1]);
        println!("   0.75-0.90 (squeal starts){:3}  <-- audible", hist[2]);
        println!("   0.90-1.05 (loud)         {:3}", hist[3]);
        println!("   >1.05 (full)             {:3}", hist[4]);
        println!();
    }

    // What the bends CLAIM they are.
    println!("grip used per bend, by the speed you carry through it:");
    println!("   {:10} {:>7} {:>7} {:>7} {:>7}", "bend", "@100%", "@85%", "@70%", "@55%");
    for (name, c) in [("Gentle", 0.55f32), ("Firm", 0.95), ("MustBrake", 1.30), ("Hard", 1.80)] {
        println!("   {name:10} {:7.2} {:7.2} {:7.2} {:7.2}",
                 c, c * 0.85, c * 0.70, c * 0.55);
    }
}
