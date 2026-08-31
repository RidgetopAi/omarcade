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
use crate::scenery;

/// How tall a roadside prop stands, in road half-widths.
///
/// A ratio against the road rather than a pixel size, so props keep their
/// apparent size at any resolution and any road width (L019).
const PROP_HEIGHT_IN_HALF_WIDTHS: f32 = 0.38;

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
    let grass_flat = theme.background.lerp(theme.green, 0.57);
    // One shade each. The midpoint of the pairs these replace, so the
    // scene keeps its overall value while losing the pattern that could
    // only ever alias, toggle or wave.
    let road_flat = theme.dark_background.lerp(theme.foreground, 0.16);
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
    c.fill_rect(ox as i32, (oy + horizon as u32) as i32, w, h - horizon as u32, grass_flat);

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

            // FLAT. No band, no gradient, no wave.
            //
            // Three attempts to get motion out of this surface all failed:
            // fine bands aliased into running backwards, coarse bands
            // toggled the whole screen at once, and a smooth cosine read
            // as ocean swells — because a smooth luminance gradient along
            // z is exactly the image a corrugated surface makes under
            // diffuse light, and vision reads gradients as curvature. The
            // corrugation was in the still frame; motion only animated it.
            //
            // Pole Position, this game's lineage, had no ground banding
            // either. Large surfaces carry speed MAGNITUDE; discrete
            // world-anchored objects carry motion DIRECTION. The markings
            // below and the props in `scenery.rs` are the motion channel.
            let mark = road.marking_index(z_here) % 2 == 0;
            let sy = fy + y;

            c.fill_rect_f(fx, sy, fw, bh, grass_flat);
            c.fill_rect_f(cx - hw, sy, hw * 2.0, bh, road_flat);

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
    // Roadside props. THIS is the motion channel — see scenery.rs for why
    // the ground surfaces are not.
    //
    // Placement follows the rival cars exactly, and that is deliberate:
    // an object owns a `z` and is projected. Two earlier attempts at
    // roadside detail positioned it as a function of the SCANLINE instead
    // — a fraction of verge screen-width, then a multiple of the road's
    // screen half-width — and both smeared into diagonal rays converging
    // on the horizon, because both shrink with distance.
    let mut props = scenery::visible_props(road, car.z, art.prop_kinds());
    props.sort_by(|a, b| b.z.total_cmp(&a.z));
    for prop in &props {
        let Some(p) = road.project(&camera, car.z, x_offset, prop.z, fw, fh) else {
            continue;
        };
        if p.y <= horizon + 1.0 || p.y > fh + 200.0 {
            continue;
        }
        let sprite = art.prop(prop.kind);
        // Scale so a prop stands a sensible fraction of the ROAD's width,
        // rather than at a factor copied from elsewhere.
        //
        // A post should read as roughly waist-high next to a car: the car
        // is 64px of art covering about 0.9 of a half-width, so one unit
        // of half-width is ~71px of art. A post 8 rows tall wants to be
        // about a third of a half-width, which puts its scale at
        // half_width * 0.33 / 8.
        //
        // The previous value (half_width / 105.0 * 1.6) was inherited from
        // the old post-drawing code and derived from nothing; at that
        // scale a post was a couple of pixels and never registered.
        let s = p.half_width * PROP_HEIGHT_IN_HALF_WIDTHS / sprite.height() as f32;
        let w = sprite.width() as f32 * s;
        let h = sprite.height() as f32 * s;
        let haze = (p.distance / (road.draw_distance() as f32 * road.segment_length()))
            .clamp(0.0, 1.0)
            * 0.85;
        sprite.draw_tinted(
            c,
            fx + p.x + prop.lane * p.half_width - w / 2.0,
            fy + p.y - h,
            s,
            Some((sky, haze)),
        );
    }

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
