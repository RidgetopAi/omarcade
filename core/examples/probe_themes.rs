//! How much hue does the road pick up, theme by theme?
use omarcade_core::{Color, Theme};
fn luma(c: Color) -> f32 {
    0.2126 * c.r as f32 + 0.7152 * c.g as f32 + 0.0722 * c.b as f32
}
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
    println!("{:<20} {:>8} {:>8} {:>8} {:>9}", "theme", "roadC", "grassC", "LUMA GAP", "grassLum");
    println!("{:-<54}", "");
    for n in &names {
        let p = format!("{dir}/{n}/colors.toml");
        let Ok(s) = std::fs::read_to_string(&p) else { continue };
        let Ok(t) = Theme::parse(&s) else { continue };
        // Mirrors render.rs. If ROAD_DESATURATION moves there, move it here.
        let road = t.dark_background.lerp(t.foreground, 0.16).desaturated(0.75);
        // Mirrors grass_for() in render.rs.
        let road_luma = luma(road);
        let mut grass = t.background.lerp(t.green, 0.0);
        let mut mix = 0.0f32;
        while mix <= 0.62 {
            let c = t.background.lerp(t.green, mix);
            if (luma(c) - road_luma).abs() <= 34.0 { grass = c; }
            mix += 0.01;
        }
        println!("{:<20} {:>8.3} {:>8.3} {:>8.1} {:>9.1}",
                 n, chroma(road), chroma(grass), (luma(grass) - luma(road)).abs(), luma(grass));
    }
    println!("\nLUMA GAP is what decides whether the road edge reads as a hard stripe.");
    println!("Above ~40 it does. This is what was 62 on everforest and 56 on gruvbox");
    println!("while being only 31 on flexoki-light — which is why the striping showed");
    println!("on dark themes and not on light ones.");
}
