//! The live Omarchy palette.
//!
//! Games take their colours from whatever theme the user is running, so
//! Omarcade looks like part of the desktop rather than a foreign window.
//!
//! # Why this reads on demand
//!
//! `omarchy theme set` does not retarget a symlink. It builds the new
//! theme in `current/next-theme`, then:
//!
//! ```text
//! rm -rf  ~/.local/state/omarchy/current/theme
//! mv      ~/.local/state/omarchy/current/next-theme  →  current/theme
//! ```
//!
//! The directory is destroyed and replaced. Two things follow, and both
//! shape this module:
//!
//! 1. An inotify watch on the file or its directory goes stale the first
//!    time the user switches themes — the watch still refers to the
//!    deleted inode, and no further events ever arrive. So there is no
//!    watcher here: [`Theme::load`] re-reads, and it is cheap enough
//!    (well under a kilobyte) to call whenever the answer might matter.
//! 2. Between the `rm -rf` and the `mv` the path does not exist at all.
//!    A read landing in that window is normal, not an error, which is
//!    why every failure path falls back instead of propagating.
//!
//! Reacting *promptly* to a theme change is a `theme-set.d` hook, which
//! `omarchy theme set` runs after the swap. That belongs to the marquee
//! work later; a game reads its palette at startup.

use std::path::PathBuf;

use serde::Deserialize;

use crate::backend::Color;

/// Whether the active theme is light or dark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Dark,
    Light,
}

/// A resolved Omarchy palette.
///
/// Every field is populated: missing entries fall back to
/// [`Theme::fallback`], so a game can index any colour unconditionally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub mode: Mode,

    pub accent: Color,
    pub selection: Color,
    pub muted: Color,

    pub background: Color,
    pub dark_background: Color,
    pub darker_background: Color,
    pub lighter_background: Color,

    pub foreground: Color,
    pub dark_foreground: Color,
    pub light_foreground: Color,

    pub red: Color,
    pub yellow: Color,
    pub orange: Color,
    pub green: Color,
    pub cyan: Color,
    pub blue: Color,
    pub magenta: Color,
}

impl Theme {
    /// Where Omarchy keeps the active theme's palette.
    ///
    /// Resolved from `$HOME` at call time rather than baked in, so this
    /// still works for any user and under a test harness.
    pub fn path() -> Option<PathBuf> {
        let home = std::env::var_os("HOME")?;
        Some(
            PathBuf::from(home)
                .join(".local/state/omarchy/current/theme/colors.toml"),
        )
    }

    /// Read the live palette, falling back on any failure.
    ///
    /// Never returns an error. A game refusing to start because a theme
    /// file was mid-swap, absent, or hand-edited into invalid TOML would
    /// be a worse outcome than showing the fallback palette.
    pub fn load() -> Theme {
        Theme::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| Theme::parse(&s).ok())
            .unwrap_or_else(Theme::fallback)
    }

    /// Parse a `colors.toml`. Exposed so tests can feed it real files.
    pub fn parse(source: &str) -> Result<Theme, toml::de::Error> {
        let raw: RawTheme = toml::from_str(source)?;
        Ok(raw.resolve())
    }

    /// The palette used when no theme can be read.
    ///
    /// These are Everforest Dark's values — a real, coherent palette
    /// rather than primary colours, so the fallback reads as a deliberate
    /// look instead of a failure state.
    pub const fn fallback() -> Theme {
        Theme {
            mode: Mode::Dark,

            accent: Color::rgb(0x7f, 0xbb, 0xb3),
            selection: Color::rgb(0x3d, 0x48, 0x4d),
            muted: Color::rgb(0x47, 0x52, 0x58),

            background: Color::rgb(0x2d, 0x35, 0x3b),
            dark_background: Color::rgb(0x21, 0x27, 0x2c),
            darker_background: Color::rgb(0x18, 0x1d, 0x20),
            lighter_background: Color::rgb(0x34, 0x3f, 0x44),

            foreground: Color::rgb(0xd3, 0xc6, 0xaa),
            dark_foreground: Color::rgb(0x4f, 0x58, 0x5e),
            light_foreground: Color::rgb(0x9d, 0xa9, 0xa0),

            red: Color::rgb(0xe6, 0x7e, 0x80),
            yellow: Color::rgb(0xdb, 0xbc, 0x7f),
            orange: Color::rgb(0xe0, 0x9d, 0x7f),
            green: Color::rgb(0xa7, 0xc0, 0x80),
            cyan: Color::rgb(0x83, 0xc0, 0x92),
            blue: Color::rgb(0x7f, 0xbb, 0xb3),
            magenta: Color::rgb(0xd6, 0x99, 0xb6),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Theme::fallback()
    }
}

/// The file as written: every key optional, all colours still strings.
///
/// Themes are user-editable and third-party themes are installable from
/// git, so a file missing `orange` or carrying keys we do not know about
/// must load rather than fail. Unknown keys are ignored by serde;
/// missing ones come from the fallback.
#[derive(Debug, Default, Deserialize)]
struct RawTheme {
    mode: Option<String>,

    accent: Option<String>,
    selection: Option<String>,
    muted: Option<String>,

    background: Option<String>,
    dark_background: Option<String>,
    darker_background: Option<String>,
    lighter_background: Option<String>,

    foreground: Option<String>,
    dark_foreground: Option<String>,
    light_foreground: Option<String>,

    red: Option<String>,
    yellow: Option<String>,
    orange: Option<String>,
    green: Option<String>,
    cyan: Option<String>,
    blue: Option<String>,
    magenta: Option<String>,
}

impl RawTheme {
    fn resolve(self) -> Theme {
        let d = Theme::fallback();

        // A field that is absent, or present but unparseable, takes the
        // fallback value. One bad hex string should cost that one colour,
        // not the whole theme.
        fn pick(raw: &Option<String>, default: Color) -> Color {
            raw.as_deref().and_then(parse_hex).unwrap_or(default)
        }

        Theme {
            mode: match self.mode.as_deref() {
                Some("light") => Mode::Light,
                Some("dark") => Mode::Dark,
                _ => d.mode,
            },

            accent: pick(&self.accent, d.accent),
            selection: pick(&self.selection, d.selection),
            muted: pick(&self.muted, d.muted),

            background: pick(&self.background, d.background),
            dark_background: pick(&self.dark_background, d.dark_background),
            darker_background: pick(&self.darker_background, d.darker_background),
            lighter_background: pick(&self.lighter_background, d.lighter_background),

            foreground: pick(&self.foreground, d.foreground),
            dark_foreground: pick(&self.dark_foreground, d.dark_foreground),
            light_foreground: pick(&self.light_foreground, d.light_foreground),

            red: pick(&self.red, d.red),
            yellow: pick(&self.yellow, d.yellow),
            orange: pick(&self.orange, d.orange),
            green: pick(&self.green, d.green),
            cyan: pick(&self.cyan, d.cyan),
            blue: pick(&self.blue, d.blue),
            magenta: pick(&self.magenta, d.magenta),
        }
    }
}

/// Parse `#rrggbb` or `#rgb` (with or without the `#`).
///
/// Returns `None` rather than a default so the caller decides what a bad
/// value means — here, "keep the fallback for this one colour".
fn parse_hex(s: &str) -> Option<Color> {
    let s = s.trim().trim_start_matches('#');

    match s.len() {
        // #rgb shorthand: each digit doubled, so f0a -> ff00aa.
        3 => {
            let mut it = s.chars().map(|c| c.to_digit(16));
            let (r, g, b) = (it.next()??, it.next()??, it.next()??);
            Some(Color::rgb(
                (r * 17) as u8,
                (g * 17) as u8,
                (b * 17) as u8,
            ))
        }
        6 => Some(Color::rgb(
            u8::from_str_radix(&s[0..2], 16).ok()?,
            u8::from_str_radix(&s[2..4], 16).ok()?,
            u8::from_str_radix(&s[4..6], 16).ok()?,
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_six_digit_hex() {
        assert_eq!(parse_hex("#7fbbb3"), Some(Color::rgb(0x7f, 0xbb, 0xb3)));
        assert_eq!(parse_hex("7fbbb3"), Some(Color::rgb(0x7f, 0xbb, 0xb3)));
        assert_eq!(parse_hex("  #2d353b  "), Some(Color::rgb(0x2d, 0x35, 0x3b)));
    }

    #[test]
    fn parses_three_digit_shorthand() {
        assert_eq!(parse_hex("#f0a"), Some(Color::rgb(0xff, 0x00, 0xaa)));
        assert_eq!(parse_hex("#fff"), Some(Color::WHITE));
        assert_eq!(parse_hex("#000"), Some(Color::BLACK));
    }

    #[test]
    fn rejects_nonsense() {
        assert_eq!(parse_hex(""), None);
        assert_eq!(parse_hex("#12345"), None);
        assert_eq!(parse_hex("#gggggg"), None);
        assert_eq!(parse_hex("rebeccapurple"), None);
    }

    /// The palette actually on this machine, verbatim.
    const EVERFOREST: &str = r##"
mode = "dark"

accent = "#7fbbb3"
selection = "#3d484d"
muted = "#475258"

background = "#2d353b"
dark_background = "#21272c"
darker_background = "#181d20"
lighter_background = "#343f44"

foreground = "#d3c6aa"
dark_foreground = "#4f585e"
light_foreground = "#9da9a0"
bright_foreground = "#d3c6aa"

red = "#e67e80"
yellow = "#dbbc7f"
orange = "#e09d7f"
green = "#a7c080"
cyan = "#83c092"
blue = "#7fbbb3"
magenta = "#d699b6"
brown = "#704e3f"

bright_red = "#e67e80"
bright_yellow = "#dbbc7f"
"##;

    #[test]
    fn parses_a_real_theme_file() {
        let t = Theme::parse(EVERFOREST).expect("real colors.toml must parse");
        assert_eq!(t.mode, Mode::Dark);
        assert_eq!(t.background, Color::rgb(0x2d, 0x35, 0x3b));
        assert_eq!(t.foreground, Color::rgb(0xd3, 0xc6, 0xaa));
        assert_eq!(t.accent, Color::rgb(0x7f, 0xbb, 0xb3));
        assert_eq!(t.magenta, Color::rgb(0xd6, 0x99, 0xb6));
    }

    /// `bright_*` and `brown` are in the file but not in our struct.
    /// Third-party themes may add anything; unknown keys must be ignored.
    #[test]
    fn unknown_keys_are_ignored() {
        let t = Theme::parse(EVERFOREST);
        assert!(t.is_ok(), "unknown keys must not fail the parse");
    }

    #[test]
    fn missing_keys_take_the_fallback() {
        let t = Theme::parse(r##"background = "#123456""##).unwrap();
        assert_eq!(t.background, Color::rgb(0x12, 0x34, 0x56));
        assert_eq!(t.orange, Theme::fallback().orange);
        assert_eq!(t.mode, Mode::Dark);
    }

    #[test]
    fn one_bad_colour_does_not_poison_the_rest() {
        let t = Theme::parse(
            r##"
            background = "not-a-colour"
            foreground = "#ffffff"
            "##,
        )
        .unwrap();
        assert_eq!(t.background, Theme::fallback().background);
        assert_eq!(t.foreground, Color::WHITE);
    }

    #[test]
    fn light_mode_is_read() {
        let t = Theme::parse(r##"mode = "light""##).unwrap();
        assert_eq!(t.mode, Mode::Light);
    }

    #[test]
    fn empty_file_is_the_fallback() {
        assert_eq!(Theme::parse("").unwrap(), Theme::fallback());
    }

    /// Malformed TOML is the one case parse reports — load() swallows it.
    #[test]
    fn broken_toml_is_an_error_but_load_still_works() {
        assert!(Theme::parse("this is not = = toml").is_err());
        let _ = Theme::load(); // must not panic whatever is on disk
    }
}
