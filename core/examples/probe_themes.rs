//! How much hue does the road pick up, theme by theme?
use omarcade_core::{Color, Theme};
fn chroma(c: Color) -> f32 {
    let (r, g, b) = (c.r as f32, c.g as f32, c.b as f32);
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    if mx == 0.0 { 0.0 } else { (mx - mn) / mx }
}
fn main() {
    let dir = std::env::var("HOME").unwrap() + "/.local/share/omarchy/themes";
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .map(|d| d.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    names.sort();
    println!("road chroma AFTER the 0.75 desaturation cap:\n");
    println!("{:<20} {:>10} {:>10} {:>10}", "theme", "road", "grass", "delta");
    println!("{:-<54}", "");
    for n in &names {
        let p = format!("{dir}/{n}/colors.toml");
        let Ok(s) = std::fs::read_to_string(&p) else { continue };
        let Ok(t) = Theme::parse(&s) else { continue };
        // Mirrors render.rs. If ROAD_DESATURATION moves there, move it here.
        let road = t.dark_background.lerp(t.foreground, 0.16).desaturated(0.75);
        let grass = t.background.lerp(t.green, 0.57);
        println!("{:<20} {:>9.3} {:>10.3} {:>10.3}", n, chroma(road), chroma(grass), chroma(grass) - chroma(road));
    }
    println!("\nroad chroma is how much HUE the tarmac carries. 0 = pure grey.");
    println!("Anything above ~0.10 reads as a coloured road rather than tarmac.");
}
