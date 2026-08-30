//! Painting the road.
//!
//! Split out of the debug example on purpose. The renderer used to live
//! in `examples/dump_art.rs`, which would have made a *debug harness* the
//! source of truth for how the game looks — so the still and the running
//! game could drift apart and each would look correct on its own terms.
//!
//! One implementation, two callers: the game draws a full window, the
//! example draws panels of a filmstrip. Both go through here.
//!
//! This module decides COLOURS and nothing else. Every coordinate comes
//! from `road.rs`; if the geometry looks wrong, it is wrong somewhere that
//! has tests.

use omarcade_core::{Canvas, Color, Pose, Theme};

use crate::art::Art;
use crate::drive::{Drive, Tuning};
use crate::road::{Camera, Road, Segment};

pub fn demo_track() -> Road {
    let mut segs = Vec::new();
    // Only the first ~15 segments occupy real screen height; past that a
    // band is under a pixel tall (measured). So the bend has to start
    // close, or it renders into a sub-pixel sliver and looks straight.
    segs.extend(std::iter::repeat_n(Segment::STRAIGHT, 4));
    // Eased in and out, because a curve that starts at full strength
    // reads as a kink. This is the shape a real track section has.
    for i in 0..6 {
        segs.push(Segment::curving(90.0 * (i as f32 / 6.0)));
    }
    segs.extend(std::iter::repeat_n(Segment::curving(90.0), 40));
    for i in 0..6 {
        segs.push(Segment::curving(90.0 * (1.0 - i as f32 / 6.0)));
    }
    // A lap, not a loop: at A's top speed 400 segments is a two-second
    // lap. Sized for driving rather than for looking at.
    segs.extend(std::iter::repeat_n(Segment::STRAIGHT, 3_944));
    Road::new(segs, 200.0, 2200.0)
}

/// `draw_road` for an arbitrary sub-rectangle, driven by a live car.
///
/// The full-screen scene is the special case of this; keeping one
/// implementation is what stops the filmstrip and the real view drifting
/// apart and showing different things.
#[allow(clippy::too_many_arguments)]
pub fn draw_road_into(
    c: &mut Canvas<'_>,
    art: &Art,
    theme: &Theme,
    road: &Road,
    tuning: &Tuning,
    car: &Drive,
    roll: f32,
    rivals: &[(f32, f32, usize)],
    ox: u32,
    oy: u32,
    w: u32,
    h: u32,
) {
    let sky = theme.background.lerp(theme.blue, 0.30);

    // Grass and road stripe INDEPENDENTLY, and by different amounts.
    //
    // Measured on the shipped theme, the old pairs had grass alternating
    // by 16.0 luminance and road by only 6.9 — so the road read as static
    // while the grass flickered hard beside it. Worse, both were driven by
    // the same band index, which put a strong light/dark boundary straight
    // across the full width of the screen in one unbroken line. Moving,
    // that reads as a rolling scanline: a bad video feed rather than
    // ground going past.
    //
    // So: the grass stripe is softened, the road stripe is strengthened,
    // and the two are offset from each other (see `road_phase`) so their
    // boundaries never coincide. Ground texture should read as two
    // surfaces moving, not as one horizontal bar sweeping the screen.
    // Mixed well toward the theme's own green rather than barely away
    // from the background. At 0.36 the grass landed at chroma 0.184
    // against the theme green's 0.333 — a muddy grey-green sitting close
    // enough to the grey road that the two blended into each other, which
    // is a large part of why the ground read as one wash rather than as
    // two surfaces. Mixing further both restores the hue and widens the
    // road-to-grass edge.
    let grass_a = theme.background.lerp(theme.green, 0.68);
    let grass_b = theme.background.lerp(theme.green, 0.46);
    // Wider apart than the hard-band version could afford. A step of this
    // size would have flashed; a gradient of it just reads as a stronger
    // sense of ground going past, because the change per frame stays
    // proportional to speed.
    let road_a = theme.dark_background.lerp(theme.foreground, 0.26);
    let road_b = theme.dark_background.lerp(theme.foreground, 0.06);
    let rumble_a = theme.red.lerp(Color::WHITE, 0.15);
    let rumble_b = theme.foreground.lerp(Color::WHITE, 0.4);
    let line = theme.foreground.lerp(Color::WHITE, 0.5);

    let camera = Camera::for_road(road, 0.85);
    let fx = ox as f32;
    let fy = oy as f32;
    let fw = w as f32;
    let fh = h as f32;
    let horizon = fh / 2.0;

    let x_offset = car.x_offset(road);

    for y in 0..horizon as u32 {
        let t = y as f32 / horizon;
        c.fill_rect(ox as i32, (oy + y) as i32, w, 1, sky.lerp(theme.background, 1.0 - t * 0.7));
    }
    c.fill_rect(ox as i32, (oy + horizon as u32) as i32, w, h - horizon as u32, grass_b);

    let bands = road.visible(&camera, car.z, x_offset, fw, fh);

    for pair in bands.windows(2).rev() {
        let (near, far) = (pair[0], pair[1]);
        let y0 = far.y.max(horizon);
        let y1 = near.y.min(fh);
        if y1 <= y0 {
            continue;
        }
        let span = near.y - far.y;
        let mut y = y0;
        while y < y1 {
            let bh = (1.0f32).min(y1 - y);
            let t = if span > 0.001 { ((y - far.y) / span).clamp(0.0, 1.0) } else { 1.0 };
            let cx = fx + far.x + (near.x - far.x) * t;
            let hw = far.half_width + (near.half_width - far.half_width) * t;
            let dist = far.distance + (near.distance - far.distance) * t;

            // Two separate groupings, because the ground and the markings
            // have different jobs. The ground is coarse so it cannot
            // appear to run backwards; the markings stay fine so the near
            // road keeps the texture that reads as speed. Neither is a
            // raw divide by segment_length — that is what aliased.
            let z_here = dist + car.z;

            // Ground shade is a CONTINUOUS function of distance, not two
            // flat shades toggling at a hard boundary.
            //
            // This is the fix for the whole surface appearing to flash.
            // A band is 800 world units, and the nearest band alone covers
            // ~270 screen rows — so at most one or two road bands are
            // visible at a time. A pattern you can only see one period of
            // cannot scroll; it can only toggle, and a single brief press
            // of the throttle flipped the entire road between two shades.
            // That reads as flashing, not as travel.
            //
            // The band could not simply be made finer: the Nyquist floor
            // is 533 units at 30fps and the band is already 800.
            //
            // A cosine of distance has no step to jump across. The surface
            // shades smoothly as it approaches, so the eye reads motion
            // from a gradient sliding rather than from a boundary
            // snapping — and there is no sharp edge left to alias into
            // running backwards.
            let cycle = road.segment_length() * Road::SEGMENTS_PER_BAND * 2.0;
            let wave = (z_here / cycle * std::f32::consts::TAU).cos() * 0.5 + 0.5;
            // The markings stay a hard alternation on purpose: they are
            // thin high-contrast detail where a crisp edge is the point,
            // and they are what carries the fine sense of speed.
            let mark = road.marking_index(z_here) % 2 == 0;
            let sy = fy + y;

            // Grass and road are offset a quarter cycle from each other,
            // so no single shade boundary ever runs across the full width
            // of the screen — the rolling-scanline artefact.
            let grass_wave =
                ((z_here / cycle + 0.25) * std::f32::consts::TAU).cos() * 0.5 + 0.5;
            let grass = grass_b.lerp(grass_a, grass_wave);
            c.fill_rect_f(fx, sy, fw, bh, grass);

            let surface = road_b.lerp(road_a, wave);
            c.fill_rect_f(cx - hw, sy, hw * 2.0, bh, surface);

            let rumble = (hw * 0.13).max(0.7);
            let rc = if mark { rumble_a } else { rumble_b };
            c.fill_rect_f(cx - hw, sy, rumble, bh, rc);
            c.fill_rect_f(cx + hw - rumble, sy, rumble, bh, rc);

            if mark {
                let lw = (hw * 0.035).max(0.5);
                c.fill_rect_f(cx - lw / 2.0, sy, lw, bh, line);
            }

            let ht = (dist / 60_000.0).clamp(0.0, 1.0);
            let a = (ht.powf(0.9) * 210.0) as u8;
            if a > 2 {
                c.fill_rect_f(fx, sy, fw, bh, sky.with_alpha(a));
            }
            y += bh;
        }
    }

    // The player, leaning into whatever bend it is actually in.
    //
    // Scale follows the ROAD's width under the bumper, not the panel
    // height. Tying it to height made the car swamp a narrow panel while
    // the road (which comes from panel width) stayed thin — the two
    // disagreed about how big the world was.
    // Rivals, placed by track position and lane, drawn far-to-near so a
    // nearer car occludes a further one.
    let mut traffic: Vec<&(f32, f32, usize)> = rivals.iter().collect();
    traffic.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (rz, lane, livery) in traffic {
        let Some(p) = road.project(&camera, car.z, x_offset, *rz, fw, fh) else {
            continue;
        };
        if p.y <= horizon + 1.0 || p.y > fh + 200.0 {
            continue;
        }
        let s = p.half_width / 105.0;
        let haze = (p.distance / (road.draw_distance() as f32 * road.segment_length()))
            .clamp(0.0, 1.0)
            * 0.8;
        let rival = art.rival(*livery);
        let w = rival.width() as f32 * s;
        let h = rival.height() as f32 * s;
        rival.draw_tinted(
            c,
            fx + p.x + lane * p.half_width - w / 2.0,
            fy + p.y - h,
            s,
            Some((sky, haze)),
        );
    }

    // Scale from the road at a FIXED distance ahead, never from
    // `bands.first()`: the nearest band changes identity as the camera
    // crosses a segment boundary — whole segment one frame, sliver the
    // next — so anything sized from it pulses. A fixed probe distance is
    // continuous, which is what a sprite scale has to be.
    let pose = Pose::cornering(car.cornering(road, tuning));
    let probe = road
        .project(&camera, car.z, x_offset, car.z + road.segment_length(), fw, fh)
        .map(|p| p.half_width)
        .unwrap_or(fw * 0.4);
    let scale = probe / 105.0 * 1.5;

    // The player sits where the camera is — dead centre — because the
    // camera IS the car. Steering moves the world, not the sprite.
    art.player.draw_ground_rolling(
        c,
        fx + fw / 2.0,
        fy + fh * 0.98,
        scale,
        pose,
        roll,
        None,
    );
}
