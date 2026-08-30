use omarcade_core::{Color, Theme};
fn lum(c: Color) -> f32 {
    0.2126 * c.r as f32 + 0.7152 * c.g as f32 + 0.0722 * c.b as f32
}
fn main() {
    let t = Theme::load();
    println!("theme slots:");
    for (n, c) in [("background", t.background), ("dark_background", t.dark_background),
                   ("darker_background", t.darker_background), ("foreground", t.foreground),
                   ("green", t.green), ("blue", t.blue), ("red", t.red)] {
        println!("  {n:<18} #{:02x}{:02x}{:02x}  lum {:>6.1}", c.r, c.g, c.b, lum(c));
    }
    // Mix amounts mirrored from games/omarcade-racer/src/render.rs.
    // If they drift, this probe lies — so keep them together.
    let g_a = t.background.lerp(t.green, 0.68);
    let g_b = t.background.lerp(t.green, 0.46);
    let r_a = t.dark_background.lerp(t.foreground, 0.26);
    let r_b = t.dark_background.lerp(t.foreground, 0.06);
    println!("\nderived:");
    for (n, c) in [("grass_a", g_a), ("grass_b", g_b), ("road_a", r_a), ("road_b", r_b)] {
        println!("  {n:<8} #{:02x}{:02x}{:02x}  lum {:>6.1}", c.r, c.g, c.b, lum(c));
    }
    println!("\nCONTRAST BETWEEN ALTERNATING BANDS:");
    println!("  grass_a vs grass_b : {:>5.1} lum", (lum(g_a) - lum(g_b)).abs());
    println!("  road_a  vs road_b  : {:>5.1} lum", (lum(r_a) - lum(r_b)).abs());
    println!("\n  grass vs road (edge definition): {:>5.1} lum", (lum(g_a) - lum(r_a)).abs());

    // How green is the grass, really? Saturation as max-min over max.
    let chroma = |c: Color| {
        let (r, g, b) = (c.r as f32, c.g as f32, c.b as f32);
        let mx = r.max(g).max(b);
        let mn = r.min(g).min(b);
        if mx == 0.0 { 0.0 } else { (mx - mn) / mx }
    };
    println!("\n  grass_a chroma: {:.3}   (theme.green is {:.3})", chroma(g_a), chroma(t.green));
    println!("  road_a  chroma: {:.3}", chroma(r_a));
}
