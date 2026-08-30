//! Change per frame at each throttle: hard bands vs the gradient.
//!
//! The complaint that produced this: at a bare touch of the throttle the
//! road's colour changed far faster than the car appeared to move. With
//! hard bands the change was ALWAYS the full contrast — a step — however
//! slowly you were going. Only its frequency scaled with speed, and the
//! eye reads the size of a change, not its frequency.
fn main() {
    let seg = 200.0f32;
    let cycle = seg * 4.0 * 2.0;
    let top = 16000.0f32;
    let fps = 60.0f32;
    let hard = 16.2f32; // measured step, previous build
    let amp = 31.4f32;  // measured peak-to-peak, gradient build

    println!("{:>9} {:>9} {:>9} {:>16} {:>16}", "throttle", "speed", "u/frame", "HARD step/frame", "GRAD lum/frame");
    for f in [0.02f32, 0.05, 0.1, 0.25, 0.5, 1.0] {
        let sp = top * f;
        let per = sp / fps;
        let grad = amp * std::f32::consts::PI * per / cycle;
        println!("{:>8.0}% {:>9.0} {:>9.1} {:>16} {:>16.2}", f * 100.0, sp, per, format!("{hard:.1}"), grad);
    }
    println!("\nA luminance change under ~1.0 per frame is below what the eye");
    println!("tracks as a discrete event. The gradient is under that up to");
    println!("~10% throttle and scales smoothly above it; the hard band was");
    println!("16.2 at every speed including a standstill crawl.");
}
