//! The race HUD: the clock, the lap, and the banners between sessions.
//!
//! Split in two on purpose. [`compose`] turns the race state into the
//! strings that will be on screen and says nothing about pixels;
//! [`draw`] paints a [`Layout`] and knows nothing about racing. That is
//! what lets a test check every string the HUD can ever produce against
//! the font — the way Breakout once shipped "BEST" as "EST" was a glyph
//! the font did not have, found by looking at the screen.

use omarcade_core::text::{text, text_width, GLYPH_H};
use omarcade_core::{Canvas, Color, Theme};

use crate::race::{Out, Phase, Race, Session, RACE_LAPS};

/// Font scale for the corner readouts. At 960x720 a glyph is 15px tall.
pub const SCALE: u32 = 3;
/// Font scale for the first line of a banner.
pub const BANNER_SCALE: u32 = 6;
/// Font scale for the lines under it, and for a flash.
pub const SUB_SCALE: u32 = 4;
/// Inset from the window edge, in pixels.
const MARGIN: i32 = 16;
/// Padding inside a banner's backing, in pixels.
const PAD: i32 = 10;

/// Under this many seconds the clock turns red.
pub const URGENT_SECONDS: f32 = 10.0;

/// How long a flash stays up.
pub const FLASH_SECONDS: f32 = 1.5;

/// A line shown briefly in the middle of the screen: the green light, a
/// checkpoint's banked time, a lap.
#[derive(Clone, Debug, PartialEq)]
pub struct Flash {
    pub line: String,
    pub remaining: f32,
}

impl Flash {
    pub fn new(line: impl Into<String>) -> Flash {
        Flash { line: line.into(), remaining: FLASH_SECONDS }
    }
}

/// Everything on screen this frame, as strings.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Layout {
    /// Top-left: the clock.
    pub clock: String,
    /// Whether the clock should shout.
    pub urgent: bool,
    /// Top-right: the lap, or the session.
    pub corner: Option<String>,
    /// Centre: the lights, the grid slot, the end. First line largest.
    pub banner: Vec<String>,
    /// Lower centre: a flash.
    pub flash: Option<String>,
}

/// Seconds with tenths. `max(0)` because a clock that has just run out
/// is shown as 0.0, never as a negative frame.
fn secs(t: f32) -> String {
    format!("{:.1}", t.max(0.0))
}

/// What the HUD says for this state.
pub fn compose(race: &Race, flash: Option<&Flash>) -> Layout {
    let clock = format!("TIME {}", secs(race.clock));
    let urgent = matches!(race.phase, Phase::Qualifying | Phase::Racing { .. })
        && race.clock < URGENT_SECONDS;

    let corner = match race.phase {
        Phase::Qualifying | Phase::Countdown { then: Session::Qualifying, .. } => {
            Some("QUALIFYING".to_string())
        }
        Phase::Qualified { .. } => Some("QUALIFIED".to_string()),
        Phase::Racing { lap } => Some(format!("LAP {lap}/{RACE_LAPS}")),
        Phase::Countdown { then: Session::Race, .. } => Some(format!("LAP 1/{RACE_LAPS}")),
        Phase::Finished { .. } | Phase::Over(_) => None,
    };

    let banner = match race.phase {
        // The lights count whole seconds: 3, 2, 1. GO is a flash.
        Phase::Countdown { remaining, .. } => vec![format!("{}", remaining.ceil().max(1.0) as u32)],
        Phase::Qualified { time, position } => vec![
            format!("QUALIFIED {}", secs(time)),
            format!("GRID {position} OF {}", race.grid_size()),
            "ENTER TO RACE".to_string(),
        ],
        Phase::Finished { time } => vec![
            format!("FINISHED {}", secs(time)),
            "ENTER TO RACE AGAIN".to_string(),
        ],
        Phase::Over(Out::DidNotQualify) => vec![
            "DID NOT QUALIFY".to_string(),
            "ENTER TO TRY AGAIN".to_string(),
        ],
        Phase::Over(Out::OutOfTime) => vec![
            "OUT OF TIME".to_string(),
            "ENTER TO TRY AGAIN".to_string(),
        ],
        Phase::Qualifying | Phase::Racing { .. } => Vec::new(),
    };

    Layout {
        clock,
        urgent,
        corner,
        banner,
        flash: flash.map(|f| f.line.clone()),
    }
}

/// A line of text centred on `cx`, on a backing so it reads over any
/// part of the scene — sky, grass, a fireball.
fn centred(c: &mut Canvas<'_>, s: &str, cx: i32, y: i32, scale: u32, color: Color, backing: Color) {
    let w = text_width(s, scale) as i32;
    let h = (GLYPH_H * scale) as i32;
    let x = cx - w / 2;
    c.fill_rect(x - PAD, y - PAD, (w + 2 * PAD) as u32, (h + 2 * PAD) as u32, backing);
    text(c, s, x, y, scale, color);
}

/// Paint the layout over a `w` x `h` frame.
pub fn draw(c: &mut Canvas<'_>, theme: &Theme, layout: &Layout, w: u32, h: u32) {
    let backing = theme.darker_background;

    // Corners.
    let clock_color = if layout.urgent { theme.red } else { theme.foreground };
    let cw = text_width(&layout.clock, SCALE) as i32;
    let ch = (GLYPH_H * SCALE) as i32;
    c.fill_rect(MARGIN - PAD, MARGIN - PAD, (cw + 2 * PAD) as u32, (ch + 2 * PAD) as u32, backing);
    text(c, &layout.clock, MARGIN, MARGIN, SCALE, clock_color);

    if let Some(corner) = &layout.corner {
        let tw = text_width(corner, SCALE) as i32;
        let x = w as i32 - MARGIN - tw;
        c.fill_rect(x - PAD, MARGIN - PAD, (tw + 2 * PAD) as u32, (ch + 2 * PAD) as u32, backing);
        text(c, corner, x, MARGIN, SCALE, theme.foreground);
    }

    // Banner, from a third of the way down.
    let cx = w as i32 / 2;
    let mut y = h as i32 / 3;
    for (i, line) in layout.banner.iter().enumerate() {
        let scale = if i == 0 { BANNER_SCALE } else { SUB_SCALE };
        let color = if i == 0 { theme.accent } else { theme.foreground };
        centred(c, line, cx, y, scale, color, backing);
        y += (GLYPH_H * scale) as i32 + 2 * PAD + 8;
    }

    // Flash, below the banner region so the two never overlap.
    if let Some(flash) = &layout.flash {
        let y = (h as f32 * 0.62) as i32;
        centred(c, flash, cx, y, SUB_SCALE, theme.yellow, backing);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drive::Tuning;
    use crate::race::{Event, Windows};
    use crate::track::grand_prix;
    use omarcade_core::text::unrenderable;

    fn race() -> Race {
        let road = grand_prix().build();
        let tuning = Tuning::from_corner(&road, 1.5);
        let visible = road.draw_distance() as f32 * road.segment_length();
        let grid_z = road.wrap(-0.1 * visible);
        let windows = Windows::derive(&road, &tuning, grid_z);
        Race::new(windows, &road, grid_z, 6)
    }

    /// Every phase, with awkward numbers, and every flash the game can
    /// raise: all of it must be in the font.
    #[test]
    fn everything_the_hud_can_say_is_in_the_font() {
        let mut r = race();
        let phases = [
            Phase::Countdown { remaining: 2.4, then: Session::Qualifying },
            Phase::Countdown { remaining: 0.01, then: Session::Race },
            Phase::Qualifying,
            Phase::Qualified { time: 103.456, position: 6 },
            Phase::Racing { lap: 3 },
            Phase::Finished { time: 1234.5 },
            Phase::Over(Out::DidNotQualify),
            Phase::Over(Out::OutOfTime),
        ];
        let flashes = [
            flash_for(Event::GreenLight),
            flash_for(Event::Checkpoint { remaining: 12.34 }),
            flash_for(Event::LapDone { lap: 2, remaining: 0.0 }),
        ];
        for phase in phases {
            r.phase = phase;
            for clock in [-0.5, 0.0, 9.99, 112.3] {
                r.clock = clock;
                for flash in flashes.iter().chain([None].iter()) {
                    let layout = compose(&r, flash.as_ref());
                    let mut all = vec![layout.clock.clone()];
                    all.extend(layout.corner.clone());
                    all.extend(layout.banner.clone());
                    all.extend(layout.flash.clone());
                    for s in all {
                        assert_eq!(unrenderable(&s), None, "{phase:?}: {s:?} has a glyph the font lacks");
                    }
                }
            }
        }
    }

    /// The flash text main.rs builds for an event. Kept here so the
    /// coverage test above and the game agree on the strings.
    pub fn flash_for(event: Event) -> Option<Flash> {
        crate::flash_for(event)
    }

    #[test]
    fn the_banner_says_the_grid_slot_and_the_time() {
        let mut r = race();
        r.phase = Phase::Qualified { time: 92.34, position: 2 };
        let l = compose(&r, None);
        assert_eq!(l.banner[0], "QUALIFIED 92.3");
        assert_eq!(l.banner[1], "GRID 2 OF 6");
        assert_eq!(l.corner.as_deref(), Some("QUALIFIED"));
    }

    #[test]
    fn the_lights_count_whole_seconds_down_to_one() {
        let mut r = race();
        for (remaining, shown) in [(3.0, "3"), (2.2, "3"), (2.0, "2"), (0.4, "1"), (0.0, "1")] {
            r.phase = Phase::Countdown { remaining, then: Session::Race };
            assert_eq!(compose(&r, None).banner, vec![shown.to_string()], "at {remaining}");
        }
    }

    #[test]
    fn the_clock_shouts_only_while_driving_and_only_when_low() {
        let mut r = race();
        r.clock = 5.0;
        r.phase = Phase::Racing { lap: 1 };
        assert!(compose(&r, None).urgent);
        r.clock = 30.0;
        assert!(!compose(&r, None).urgent);
        r.clock = 5.0;
        r.phase = Phase::Qualified { time: 90.0, position: 1 };
        assert!(!compose(&r, None).urgent, "nothing to hurry for after the flag");
        assert_eq!(compose(&r, None).clock, "TIME 5.0");
        r.clock = -0.3;
        r.phase = Phase::Over(Out::OutOfTime);
        assert_eq!(compose(&r, None).clock, "TIME 0.0", "a run-out clock never shows negative");
    }

    #[test]
    fn a_flash_is_drawn_and_a_banner_is_drawn() {
        let (w, h) = (320u32, 240u32);
        let theme = Theme::fallback();
        let mut r = race();
        r.phase = Phase::Over(Out::OutOfTime);
        let paint = |layout: &Layout| {
            let mut buf = vec![0u32; (w * h) as usize];
            {
                let mut c = Canvas::new(&mut buf, w, h);
                draw(&mut c, &theme, layout, w, h);
            }
            buf.iter().filter(|&&p| p != 0).count()
        };
        let plain = paint(&compose(&r, None));
        let flashed = paint(&compose(&r, Some(&Flash::new("+12.3"))));
        assert!(plain > 0, "a banner painted nothing");
        assert!(flashed > plain, "the flash painted nothing extra");
    }
}
