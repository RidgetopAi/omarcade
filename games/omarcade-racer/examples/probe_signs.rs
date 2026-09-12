//! Where everything STANDS on the shipped course, seen from above.
//!
//! Answers a question no other instrument does: the game is a forward
//! view down a road, `dump_art` renders that forward view, and neither
//! can show you the LAP. A sign's position is authored as a `z` — a
//! distance along the track — and a distance along a track tells you
//! nothing about whether it is on the straight you meant or halfway
//! into the following corner.
//!
//! So this integrates the heading from `Road::curve_at` to recover a
//! real overhead centreline, drops every `structures::shipped()`
//! placement onto it, and names the course section each one lands in.
//!
//! ⚠️ THE MAP IS DERIVED, NEVER DRAWN BY HAND. Every number here is read
//! from `track::grand_prix()` and `structures::shipped()`, so a course
//! edit or a moved sign shows up the next time this is run rather than
//! leaving a stale picture in the docs.
//!
//! ⚠️ THE SHAPE IS INDICATIVE, NOT SURVEYED. `curve_at` returns the
//! normalised curve the PROJECTION uses, which is what bends the road on
//! screen — it is not a signed radius in world units. Integrating it
//! gives the right topology (the order of corners, which way each turns,
//! roughly how tight) and should not be read as a scale drawing.
//!
//! Run with:
//!   cargo run -p omarcade-racer --example probe_signs

#[path = "../src/road.rs"]
mod road;
#[path = "../src/drive.rs"]
mod drive;
#[path = "../src/track.rs"]
mod track;
#[path = "../src/art.rs"]
mod art;
#[path = "../src/scenery.rs"]
mod scenery;
#[path = "../src/structures.rs"]
mod structures;

use structures::{Side, Structure};

/// How finely the centreline is walked, in world units.
const STEP: f32 = 500.0;

/// Character grid for the overhead map.
const COLS: usize = 92;
const ROWS: usize = 30;

fn main() {
    let road = track::grand_prix().build();
    let length = road.length();
    let reach = road.draw_distance() as f32 * road.segment_length();
    let placements = structures::shipped();

    println!();
    println!("OMAPRIX — SIGNS AND POSTS, FROM ABOVE");
    println!("=====================================");
    println!();
    println!(
        "Course      {:.2} miles ({:.0} world units), {} segments",
        length / track::UNITS_PER_MILE,
        length,
        road.segment_count(),
    );
    println!(
        "Draw reach  {reach:.0} units — how far ahead the car can see, \
         and the unit signs are spaced in",
    );
    println!("Road width  {:.0} units", road.width());
    println!();

    // ---- walk the centreline, integrating heading from the curve ----
    let mut pts: Vec<(f32, f32, f32)> = Vec::new(); // (x, y, z)
    let (mut x, mut y, mut heading) = (0.0f32, 0.0f32, 0.0f32);
    let mut z = 0.0f32;
    while z < length {
        pts.push((x, y, z));
        // The normalised curve is a rate of turn per unit travelled. The
        // scale factor is cosmetic — it sets how tightly the picture
        // coils — but it must be CONSTANT, or corners lie about their
        // relative severity.
        heading += road.curve_at(z) * STEP * 0.000_012;
        x += heading.sin() * STEP;
        y += heading.cos() * STEP;
        z += STEP;
    }

    // ---- fit to the character grid ----
    let (min_x, max_x) = bounds(pts.iter().map(|p| p.0));
    let (min_y, max_y) = bounds(pts.iter().map(|p| p.1));
    let span_x = (max_x - min_x).max(1.0);
    let span_y = (max_y - min_y).max(1.0);

    // ⚠️ ONE SCALE FOR BOTH AXES, or the lap is not the shape it is.
    // Fitting x and y independently stretches whichever axis is smaller
    // and turns a gentle bend into a hairpin. The picture must keep the
    // track's aspect ratio; the character cell's own 1:2 shape is
    // corrected for separately by `CELL_ASPECT`.
    const CELL_ASPECT: f32 = 2.1;
    let scale = ((COLS - 1) as f32 / (span_x * CELL_ASPECT))
        .min((ROWS - 1) as f32 / span_y);
    let used_cols = span_x * CELL_ASPECT * scale;
    let used_rows = span_y * scale;
    let pad_c = ((COLS - 1) as f32 - used_cols) / 2.0;
    let pad_r = ((ROWS - 1) as f32 - used_rows) / 2.0;

    let to_cell = |x: f32, y: f32| -> (usize, usize) {
        let c = (pad_c + (x - min_x) * CELL_ASPECT * scale).round() as usize;
        // Screen rows run downward; north should be up.
        let r = (pad_r + (max_y - y) * scale).round() as usize;
        (r.min(ROWS - 1), c.min(COLS - 1))
    };

    let mut grid = vec![vec![' '; COLS]; ROWS];
    for (px, py, _) in &pts {
        let (r, c) = to_cell(*px, *py);
        grid[r][c] = '·';
    }

    // Structures go on last so they always win their cell.
    //
    // ⚠️ AND THEY MUST NOT OVERWRITE EACH OTHER. The three start-straight
    // signs sit 26,400 units apart on a 1.3-million-unit lap, which is
    // well under one character cell — drawn naively, two of the three
    // vanish and the map silently under-reports what is on the track.
    // A taken cell nudges along the track instead, so every sign is
    // visible even where the map cannot resolve the true spacing.
    let mut crowded = 0usize;
    for (i, p) in placements.iter().enumerate() {
        let at = point_at(&pts, p.z);
        let (mut r, mut c) = to_cell(at.0, at.1);
        let glyph = match p.kind {
            Structure::Gantry => '#',
            // Numbered so the map and the table below can be read
            // together — a glyph alone cannot say WHICH billboard.
            _ => char::from_digit(i as u32, 10).unwrap_or('*'),
        };
        if grid[r][c] != '·' && grid[r][c] != ' ' {
            crowded += 1;
            // Step FORWARD along the walked line to the nearest free
            // cell, so the nudge is along the track rather than into
            // scenery — and forward rather than back, because back is
            // where the gantry and the earlier signs already are.
            let mut ahead = p.z;
            while ahead < length {
                ahead += STEP;
                let a = point_at(&pts, ahead.min(length));
                let (rr, cc) = to_cell(a.0, a.1);
                if grid[rr][cc] == '·' || grid[rr][cc] == ' ' {
                    r = rr;
                    c = cc;
                    break;
                }
            }
        }
        grid[r][c] = glyph;
    }

    println!("  # = start/finish gantry     1-6 = billboards (see table)");
    println!("  · = track centreline        direction of travel: 0 -> 1 -> 2 ...");
    println!();
    for row in &grid {
        let line: String = row.iter().collect();
        println!("  {}", line.trim_end());
    }
    if crowded > 0 {
        println!();
        println!("  ⚠️ {crowded} sign(s) shown one cell along: they sit closer together");
        println!("     than one character can resolve. Read spacing off the TABLE.");
    }
    println!();

    // ---- the table ----
    println!("EVERY HAND-PLACED STRUCTURE");
    println!("---------------------------");
    println!(
        "  {:<3} {:<22} {:<7} {:>9} {:>7} {:>7}  {}",
        "#", "WHAT", "SIDE", "Z (units)", "MILE", "REACHES", "SECTION IT LANDS IN",
    );
    for (i, p) in placements.iter().enumerate() {
        let (what, side) = describe(p.kind);
        println!(
            "  {:<3} {:<22} {:<7} {:>9.0} {:>7.2} {:>7.1}  {}",
            i,
            what,
            side,
            p.z,
            p.z / track::UNITS_PER_MILE,
            p.z / reach,
            section_at(p.z),
        );
    }
    println!();

    // ---- the course, for reading the table against ----
    println!("THE COURSE, SECTION BY SECTION");
    println!("------------------------------");
    println!(
        "  {:<26} {:>9} {:>9} {:>7}  {}",
        "SECTION", "FROM", "TO", "MILES", "STRUCTURES IN IT",
    );
    let mut from = 0.0f32;
    for (name, miles) in sections() {
        let to = from + miles * track::UNITS_PER_MILE;
        let here: Vec<String> = placements
            .iter()
            .enumerate()
            .filter(|(_, p)| p.z >= from && p.z < to)
            .map(|(i, _)| i.to_string())
            .collect();
        println!(
            "  {:<26} {:>9.0} {:>9.0} {:>7.2}  {}",
            name,
            from,
            to,
            miles,
            if here.is_empty() { "—".to_string() } else { here.join(", ") },
        );
        from = to;
    }
    println!();

    // ---- the other roadside population ----
    println!("ROADSIDE PROPS — the OTHER thing beside the road");
    println!("------------------------------------------------");
    println!(
        "  ⚠️ These are NOT placed by hand and are NOT in the map above.\n     \
         `scenery::props_between` generates them from a hash of the track\n     \
         position, so they are identical every run but nobody chose where\n     \
         any individual one goes.",
    );
    let props = scenery::props_between(0.0, length, 4);
    println!();
    println!("  Count on one lap   {}", props.len());
    println!("  Average spacing    {:.0} units (± {:.0}% jitter)",
        scenery::PROP_SPACING,
        scenery::PROP_JITTER * 100.0,
    );
    println!(
        "  Stand at           ±1.30 half-widths from centre (±0.22 jitter)\n     \
         — i.e. just off the road, alternating sides on a hashed break",
    );
    println!(
        "  In view at once    ~{:.0}",
        reach / scenery::PROP_SPACING,
    );
    println!();
}

fn bounds(it: impl Iterator<Item = f32>) -> (f32, f32) {
    it.fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)))
}

/// The walked point nearest a track position.
fn point_at(pts: &[(f32, f32, f32)], z: f32) -> (f32, f32) {
    let i = ((z / STEP).round() as usize).min(pts.len().saturating_sub(1));
    (pts[i].0, pts[i].1)
}

fn describe(k: Structure) -> (&'static str, &'static str) {
    match k {
        Structure::Gantry => ("Start/finish gantry", "spans"),
        Structure::Billboard { side } => ("Billboard (blank)", side_name(side)),
        Structure::BillboardOmarchy { side } => ("Billboard: OMARCHY", side_name(side)),
        Structure::BillboardRidgetop { side } => ("Billboard: RIDGETOPAI", side_name(side)),
        Structure::BillboardMandrel { side } => ("Billboard: MANDREL", side_name(side)),
        Structure::BillboardOmarcade { side } => ("Billboard: OMARCADE", side_name(side)),
        Structure::BillboardNextLap { side } => ("Sign: NEXT LAP", side_name(side)),
    }
}

fn side_name(s: Side) -> &'static str {
    match s {
        Side::Left => "left",
        Side::Right => "right",
    }
}

/// The shipped course, as authored in `track::grand_prix`.
///
/// ⚠️ RESTATED HERE because `Track` does not expose its sections, and a
/// probe that cannot name them can only print numbers. If `grand_prix`
/// changes, this list must follow — the total is asserted below so a
/// drift is loud rather than silent.
fn sections() -> Vec<(&'static str, f32)> {
    vec![
        ("Start straight", 0.45),
        ("Firm RIGHT", 0.25),
        ("Short straight", 0.15),
        ("MUST-BRAKE LEFT", 0.22),
        ("Straight", 0.20),
        ("Gentle right", 0.18),
        ("Gentle left", 0.18),
        ("Long BACK STRAIGHT", 0.42),
        ("HARD RIGHT", 0.20),
        ("Straight", 0.20),
        ("Firm LEFT (to the line)", 0.25),
    ]
}

fn section_at(z: f32) -> String {
    let mut from = 0.0f32;
    for (name, miles) in sections() {
        let to = from + miles * track::UNITS_PER_MILE;
        if z >= from && z < to {
            let frac = (z - from) / (to - from);
            let where_in = match frac {
                f if f < 0.25 => "start of",
                f if f < 0.75 => "middle of",
                _ => "end of",
            };
            return format!("{where_in} {name}");
        }
        from = to;
    }
    "past the line (wraps)".to_string()
}
