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
    let grass_a = theme.background.lerp(theme.green, 0.40);
    let grass_b = theme.background.lerp(theme.green, 0.28);
    let road_a = theme.dark_background.lerp(theme.foreground, 0.15);
    let road_b = theme.dark_background.lerp(theme.foreground, 0.11);
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
            let phase = road.band_index(dist + car.z) % 2 == 0;
            let mark = road.marking_index(dist + car.z) % 2 == 0;
            let sy = fy + y;

            c.fill_rect_f(fx, sy, fw, bh, if phase { grass_a } else { grass_b });
            c.fill_rect_f(cx - hw, sy, hw * 2.0, bh, if phase { road_a } else { road_b });

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
