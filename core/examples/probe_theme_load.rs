//! Is Theme::load() reading the live theme, or silently falling back?
//!
//! `Theme::fallback()` is hardcoded everforest, byte-identical to the
//! everforest colors.toml — so a parse failure is INVISIBLE while that
//! theme is active, and switching themes appears to do nothing.
use omarcade_core::Theme;
fn main() {
    let path = Theme::path();
    println!("path            : {path:?}");
    match &path {
        Some(p) => {
            println!("exists          : {}", p.exists());
            match std::fs::read_to_string(p) {
                Ok(s) => {
                    println!("readable        : yes ({} bytes)", s.len());
                    match Theme::parse(&s) {
                        Ok(t) => println!("PARSE           : OK  background #{:02x}{:02x}{:02x}",
                                          t.background.r, t.background.g, t.background.b),
                        Err(e) => println!("PARSE           : *** FAILED *** {e:?}"),
                    }
                }
                Err(e) => println!("readable        : NO — {e}"),
            }
        }
        None => println!("no HOME set"),
    }
    let loaded = Theme::load();
    let fb = Theme::fallback();
    println!("\nloaded background   #{:02x}{:02x}{:02x}", loaded.background.r, loaded.background.g, loaded.background.b);
    println!("fallback background #{:02x}{:02x}{:02x}", fb.background.r, fb.background.g, fb.background.b);
    let same = loaded.background.r == fb.background.r
        && loaded.background.g == fb.background.g
        && loaded.background.b == fb.background.b
        && loaded.green.g == fb.green.g;
    println!("\nloaded == fallback? {same}");
    if same {
        println!("  ^ INCONCLUSIVE while everforest is active: fallback IS everforest.");
        println!("    Switch to a visually different theme and re-run to tell them apart.");
    }
}
