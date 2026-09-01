//! Things that stand at a KNOWN place on the track.
//!
//! Deliberately not [`crate::scenery`], which is the opposite problem.
//! Roadside props are scattered — hashed, jittered, spread across whatever
//! art exists — and that is right for them, because a marker post means
//! nothing in particular and only has to stream past. A structure means
//! something *where it is*: the gantry is the start line, and moving it
//! moves the start line.
//!
//! So props are placed by an algorithm and structures are placed by hand,
//! and the two never share a code path.
//!
//! # The two ways a structure is sized
//!
//! This is the part that is easy to get wrong, and both ways were got
//! wrong before they were got right.
//!
//! A **road-spanning** structure — a gantry — has its width pinned by the
//! thing it straddles. It is scaled so its INK spans a stated number of
//! road half-widths. Scaling by the grid instead leaves it floating clear
//! of both verges, because the art has blank columns either side.
//!
//! A **roadside** structure — a billboard — spans nothing, so nothing pins
//! its width. It is scaled by the height of the part a viewer judges the
//! size of: the PANEL, not the whole sprite. Scaling by the whole thing
//! makes the sign two thirds of the intended size and the posts too short
//! to see, which reads as a sign hanging in the air.

use omarcade_core::sprite::Sprite;
use omarcade_core::Canvas;

use crate::art::Art;
use crate::road::{Camera, Road};

/// Which side of the road something stands on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    fn sign(self) -> f32 {
        match self {
            Side::Left => -1.0,
            Side::Right => 1.0,
        }
    }
}

/// What is standing there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Structure {
    /// The start/finish gantry. Spans the road.
    Gantry,
    /// A blank billboard beside the road.
    Billboard { side: Side },
    /// A billboard carrying the Omarchy wordmark.
    ///
    /// A separate variant rather than a field on `Billboard`, because it
    /// is a different SPRITE — wider, so the mark fits at native
    /// resolution — and placement has to scale the one it is actually
    /// drawing.
    BillboardOmarchy { side: Side },
}

/// One structure at one place on the track.
#[derive(Clone, Copy, Debug)]
pub struct Placement {
    /// Where along the track, in world units. Fixed — that is the whole
    /// point of this module.
    pub z: f32,
    pub kind: Structure,
}

/// Where the car starts, as a fraction of the draw distance BEFORE the
/// start line. Negative: it is a setback.
///
/// ⚠️ A start line you cannot see at the start is not a start line. The
/// car began at track zero — the same place as the gantry — so it began
/// standing inside it, the line was a whole lap ahead, and it appeared
/// only in the last twentieth of a mile of each lap. Brian found this in
/// a screenshot; five tests here missed it, because each asked whether
/// the structure was placed correctly and none asked what you can see
/// from where you start.
///
/// ⚠️ AND IT IS A TENTH, not most of a draw distance. The projection is
/// hyperbolic, so distance and apparent size are nothing like
/// proportional: at 0.8 the gantry is FOUR PIXELS of half-width —
/// technically visible, practically not there. At 0.1 it is around
/// thirty, which is a structure you are lined up under.
///
/// Lives here rather than in `main.rs` so the test that checks the line
/// is visible from the grid reads the same number the game starts at. It
/// was briefly two numbers, and the test passed against a value the game
/// did not use (L022).
pub const GRID_SETBACK: f32 = -0.1;

/// How much of the road a gantry spans, in half-widths.
///
/// The road is 2.0 half-widths across, so 2.6 means the legs land outside
/// the verges — which is where the legs of a real gantry are. At exactly
/// 2.0 they would sit ON the rumble strips and read as an obstacle.
const GANTRY_SPAN_HALF_WIDTHS: f32 = 2.6;

/// How tall a billboard's PANEL is, in road half-widths.
///
/// Stated about the panel rather than the whole sprite — see the module
/// docs. Just under a half-width is the size at which the sign is
/// comfortably readable without dominating the frame.
const BILLBOARD_PANEL_HALF_WIDTHS: f32 = 0.95;

/// Where a billboard's NEAR EDGE stands, in half-widths from the centre
/// line.
///
/// Stated about the edge of the drawing nearest the road, not about the
/// sprite's centre — see `draw_roadside`. Past 1.0 by enough to clear the
/// verge and the rumble strip and read as standing off the track rather
/// than on its shoulder.
const BILLBOARD_OFFSET_HALF_WIDTHS: f32 = 1.15;

/// Draw every structure that is currently visible.
///
/// Called after the road and before the traffic, so a car can pass in
/// front of a billboard. Sorted far-to-near so nearer structures paint
/// over further ones, which is the same order the traffic is drawn in and
/// for the same reason.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    c: &mut Canvas<'_>,
    art: &Art,
    road: &Road,
    camera: &Camera,
    camera_z: f32,
    x_offset: f32,
    placements: &[Placement],
    fx: f32,
    fy: f32,
    fw: f32,
    fh: f32,
) {
    // Far to near. Collected rather than sorted in place because the
    // placement list belongs to the caller and is shared across frames.
    let mut visible: Vec<(f32, &Placement)> = placements
        .iter()
        .filter_map(|p| {
            // A structure is somewhere on a looping track, so the one
            // ahead may be at a smaller z than the camera. `ahead_of`
            // resolves that to a distance rather than a position.
            visible_at(road, camera_z, p.z).map(|d| (d, p))
        })
        .collect();
    visible.sort_by(|a, b| b.0.total_cmp(&a.0));

    for (d, placement) in visible {
        let Some(p) = road.project(camera, camera_z, x_offset, camera_z + d, fw, fh)
        else {
            continue;
        };

        // Off the top of the frame, or past the bottom. The same guard the
        // road's own sprite pass uses.
        if p.y <= fh / 2.0 + 1.0 || p.y > fh + 200.0 {
            continue;
        }

        match placement.kind {
            Structure::Gantry => draw_spanning(c, &art.gantry, &p, fx, fy),
            Structure::Billboard { side } => {
                draw_roadside(c, &art.billboard, art.billboard_panel_rows(), &p, side, fx, fy)
            }
            Structure::BillboardOmarchy { side } => draw_roadside(
                c,
                &art.billboard_omarchy,
                art.billboard_omarchy_panel_rows(),
                &p,
                side,
                fx,
                fy,
            ),
        }
    }
}

/// How far ahead of the camera a track position is, accounting for the
/// track looping.
///
/// A structure at z=0 is not behind you on lap two; it is a whole lap
/// ahead. Without this the start gantry would go NEGATIVE the moment it
/// was passed and be filtered out for the rest of the lap.
///
/// ⚠️ WRAPPING IS NECESSARY AND NOT SUFFICIENT, which is what the first
/// version of this got wrong. It returned a correct distance — a whole
/// lap, 1,309,799 units — and a structure that far away projects to a
/// half-width of ZERO. The gantry was never drawn anywhere on the lap,
/// and the test that was supposed to catch it asserted the DISTANCE was
/// right without ever asking whether anything could be seen at it.
/// Callers must check the result against the draw reach, and
/// [`visible_at`] is the thing that does both.
fn ahead_of(road: &Road, camera_z: f32, z: f32) -> f32 {
    let lap = road.segment_count() as f32 * road.segment_length();
    (z - camera_z).rem_euclid(lap)
}

/// How far ahead a structure is, if it is close enough to draw at all.
///
/// One place that knows both halves of the rule — wrapped for the lap,
/// and inside the draw distance — so a caller cannot get one without the
/// other.
///
/// The near bound is `> 0.0` rather than `>= 0.0`: at exactly zero the
/// camera is standing in the structure, the projection divides by that
/// distance, and there is nothing sensible to draw. A structure the car
/// is inside is a structure it has passed.
fn visible_at(road: &Road, camera_z: f32, z: f32) -> Option<f32> {
    let d = ahead_of(road, camera_z, z);
    let reach = road.draw_distance() as f32 * road.segment_length();
    (d > 0.0 && d < reach).then_some(d)
}

/// A structure that spans the road: scaled so its INK covers a stated
/// number of half-widths, and centred on the road's centre line.
fn draw_spanning(
    c: &mut Canvas<'_>,
    sprite: &Sprite,
    p: &crate::road::Projected,
    fx: f32,
    fy: f32,
) {
    let Some((x0, _, x1, _)) = sprite.ink_bounds() else { return };
    let ink_w = (x1 - x0 + 1) as f32;
    let scale = p.half_width * GANTRY_SPAN_HALF_WIDTHS / ink_w;

    // Centred on the road, corrected for however the ink sits in its grid,
    // and stood on the road surface rather than on the grid's bottom edge.
    let x = fx + p.x - sprite.ink_centre_bias() * scale;
    let y = fy + p.y + sprite.ink_foot_gap() * scale;
    sprite.draw_ground(c, x, y, scale);
}

/// A structure that stands beside the road: scaled by the height of its
/// readable part, and set out past the verge.
fn draw_roadside(
    c: &mut Canvas<'_>,
    sprite: &Sprite,
    panel_rows: (usize, usize),
    p: &crate::road::Projected,
    side: Side,
    fx: f32,
    fy: f32,
) {
    let panel_h = (panel_rows.1 - panel_rows.0 + 1) as f32;
    let scale = p.half_width * BILLBOARD_PANEL_HALF_WIDTHS / panel_h;

    let Some((x0, _, x1, _)) = sprite.ink_bounds() else { return };

    // ⚠️ THE OFFSET IS TO THE INK'S NEAR EDGE, NOT TO THE SPRITE CENTRE.
    //
    // Offsetting the sprite's centre by 1.25 half-widths sounds like it
    // stands the sign a quarter of a half-width past the verge. It does
    // not: the ink is 66 columns of a 160-wide grid, so half the SPRITE
    // reaches far beyond half the DRAWING, and the sign overlapped the
    // tarmac by 86 pixels at every distance — posts planted on the road.
    //
    // What has to clear the verge is the edge of the drawing nearest the
    // road. So the near edge is placed, and the sprite's origin is solved
    // backwards from it.
    let ink_w = (x1 - x0 + 1) as f32 * scale;
    // Where the ink's near edge must land, in screen pixels.
    let near_edge = p.x + side.sign() * p.half_width * BILLBOARD_OFFSET_HALF_WIDTHS;
    // `draw_ground` centres the SPRITE on x, so the ink's left edge sits
    // at `x - width/2 + x0`. Solve for x given where the near edge goes.
    let ink_left = match side {
        Side::Right => near_edge,
        Side::Left => near_edge - ink_w,
    };
    let x = fx + ink_left + sprite.width() as f32 * scale / 2.0 - x0 as f32 * scale;
    let y = fy + p.y + sprite.ink_foot_gap() * scale;
    sprite.draw_ground(c, x, y, scale);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::{grand_prix, UNITS_PER_MILE};

    /// Every structure is actually SEEN at some point on the lap.
    ///
    /// ⚠️ THE TEST THAT SHOULD HAVE EXISTED FIRST. Its neighbour below
    /// asserts a passed structure comes round again — and it did, at a
    /// correct distance of a whole lap, which projects to a half-width of
    /// zero. The gantry was never drawn anywhere, the test passed, and it
    /// took a screenshot from Brian to find it.
    ///
    /// Asserting a DISTANCE is asserting an intermediate. This drives the
    /// whole lap and asserts the OUTPUT: that each structure spends real
    /// time on screen at a size worth drawing (L022).
    #[test]
    fn every_structure_is_seen_on_the_lap() {
        let road = grand_prix().build();
        let lap = road.segment_count() as f32 * road.segment_length();
        let camera = Camera::for_road(&road, 0.85);

        for placement in shipped() {
            let mut seen_frames = 0;
            let mut biggest = 0.0f32;

            // Walk the whole lap in draw-distance-sized steps, sampling
            // finely enough that a structure cannot slip between samples.
            let step = road.segment_length();
            let mut camera_z = 0.0f32;
            while camera_z < lap {
                if let Some(d) = visible_at(&road, camera_z, placement.z) {
                    if let Some(p) =
                        road.project(&camera, camera_z, 0.0, camera_z + d, 960.0, 720.0)
                    {
                        seen_frames += 1;
                        biggest = biggest.max(p.half_width);
                    }
                }
                camera_z += step;
            }

            assert!(
                seen_frames > 0,
                "{:?} at z={} is never visible anywhere on the lap",
                placement.kind,
                placement.z,
            );
            assert!(
                biggest > 20.0,
                "{:?} never gets bigger than {biggest:.1}px of half-width — \
                 it is on the track but never big enough to see",
                placement.kind,
            );
        }
    }

    /// THE START LINE IS VISIBLE FROM THE STARTING GRID.
    ///
    /// ⚠️ THE BUG BRIAN FOUND IN A SCREENSHOT, and the one every other
    /// test here missed. The gantry was correctly placed at track zero and
    /// correctly drawn — and the car also began at track zero, standing
    /// inside it. The line was then a whole lap ahead, fifty-five draw
    /// distances away, and appeared only in the last twentieth of a mile
    /// of each lap. Every test passed: it was on the track, it was on a
    /// straight, it came round again, and walking the whole lap did see
    /// it. None of them asked the actual question, which is whether you
    /// can see the start line AT THE START.
    ///
    /// The lesson is the reusable part: a test that samples the whole
    /// space cannot catch a bug about ONE POSITION in it. Test the
    /// position the player is actually in.
    #[test]
    fn the_start_line_is_visible_from_the_grid() {
        let road = grand_prix().build();
        let camera = Camera::for_road(&road, 0.85);
        let visible = road.draw_distance() as f32 * road.segment_length();

        // Where main.rs puts the car at the green light.
        let grid_z = road.wrap(GRID_SETBACK * visible);

        let gantry = shipped()
            .into_iter()
            .find(|p| p.kind == Structure::Gantry)
            .expect("there is a start line");

        let d = visible_at(&road, grid_z, gantry.z)
            .expect("the start line is not visible from the starting grid");
        let p = road
            .project(&camera, grid_z, 0.0, grid_z + d, 960.0, 720.0)
            .expect("the start line does not project from the starting grid");

        assert!(
            p.half_width > 20.0,
            "the start line is only {:.1}px of half-width from the grid — \
             technically visible, practically not there",
            p.half_width,
        );
    }

    /// A structure ahead stays ahead, and one just passed is a whole lap
    /// away rather than behind.
    ///
    /// THE BUG THIS GUARDS: without the wrap, the start gantry disappears
    /// the instant the car crosses it and never comes back, so laps two
    /// and three have no start line.
    #[test]
    fn a_passed_structure_comes_round_again() {
        let road = grand_prix().build();
        let lap = road.segment_count() as f32 * road.segment_length();

        // Just before the line: nearly there.
        let d = ahead_of(&road, lap - 1000.0, 0.0);
        assert!((d - 1000.0).abs() < 1.0, "expected 1000 units to go, got {d}");

        // Just after it: almost a whole lap away, not negative.
        let d = ahead_of(&road, 1000.0, 0.0);
        assert!(d > 0.0, "a just-passed structure came out at {d}, behind the camera");
        assert!(
            (d - (lap - 1000.0)).abs() < 1.0,
            "expected nearly a full lap, got {d}",
        );
    }

    /// Every shipped placement is on the track.
    ///
    /// A z past the end of the lap wraps to somewhere arbitrary, so it
    /// would still draw — just not where the definition says. That is the
    /// kind of wrong that looks right.
    #[test]
    fn the_shipped_placements_are_on_the_track() {
        let road = grand_prix().build();
        let lap = road.segment_count() as f32 * road.segment_length();

        for p in shipped() {
            assert!(
                p.z >= 0.0 && p.z < lap,
                "{:?} at z={} is off a {lap}-unit lap",
                p.kind,
                p.z,
            );
        }
    }

    /// The gantry is the start line, so it sits at the start.
    #[test]
    fn the_gantry_marks_the_start() {
        let gantries: Vec<_> = shipped()
            .into_iter()
            .filter(|p| p.kind == Structure::Gantry)
            .collect();
        assert_eq!(gantries.len(), 1, "there should be exactly one start line");
        assert_eq!(gantries[0].z, 0.0, "the start line is not at the start");
    }

    /// Billboards stand on straights, not in corners.
    ///
    /// Not decoration: a sign is something to read, and a corner is when
    /// the player has the least attention to spare. Placing them where
    /// nothing else is happening is what makes them readable at all.
    #[test]
    fn billboards_stand_where_there_is_time_to_read_them() {
        let road = grand_prix().build();
        for p in shipped() {
            if !matches!(
                p.kind,
                Structure::Billboard { .. } | Structure::BillboardOmarchy { .. }
            ) {
                continue;
            }
            // Straight where it stands, and still straight a little way
            // further on — a sign at the very entry of a bend is a sign
            // you read while turning in.
            for ahead in [0.0f32, 0.05, 0.1] {
                let z = p.z + ahead * UNITS_PER_MILE;
                let curve = road.curve_at(z).abs();
                assert!(
                    curve < 0.1,
                    "a billboard at mile {:.2} has a bend of {curve} {:.2} miles ahead",
                    p.z / UNITS_PER_MILE,
                    ahead,
                );
            }
        }
    }
}

/// Where the structures stand on the shipped course.
///
/// Hand-placed, because that is what a structure is. The gantry is the
/// start line and belongs at zero; the billboards go on straights, where
/// there is time to look at them — a sign at a corner entry is a sign read
/// while turning in.
///
/// # ⚠️ SPACED IN DRAW REACHES, NOT IN MILES
///
/// This is the correction that mattered, and it is L019 in yet another
/// costume. The track format authors in miles because a course is a place
/// and miles are how a person pictures one. But the CAR cannot see a mile
/// — the draw distance is 120 segments of 200 units, which is 24,000
/// units, or **one twentieth of a mile**.
///
/// The first version of this list spaced billboards 0.12 miles apart,
/// which sounded close together and is two and a half times further than
/// the car can see. Every structure was correctly placed and every one sat
/// beyond the horizon; the render came back empty and looked like the
/// drawing code was broken.
///
/// So spacing is stated in reaches. One reach apart means the next sign
/// appears at the horizon as the last one passes. Miles are right for
/// authoring a course and wrong for authoring what stands beside it,
/// because visibility is what these are for.
///
/// ⚠️ These are tied to [`crate::track::grand_prix`]. Change the course
/// and they move to the wrong places on it; the tests check they are on
/// the track and on straights, which is what catches that.
pub fn shipped() -> Vec<Placement> {
    // One draw reach: how far the car can see. Read from the road rather
    // than written down, so a change to the draw distance moves these
    // rather than silently pushing them over the horizon.
    let road = crate::track::grand_prix().build();
    let reach = road.draw_distance() as f32 * road.segment_length();

    vec![
        // The start line.
        Placement { z: 0.0, kind: Structure::Gantry },
        // Down the start straight: the first appears as the gantry is
        // passed, then alternating sides a reach apart, so they arrive one
        // at a time rather than as a wall.
        // The wordmark gets the first slot after the line, where a real
        // circuit puts the sponsor that paid the most.
        Placement { z: 1.1 * reach, kind: Structure::BillboardOmarchy { side: Side::Right } },
        Placement { z: 2.2 * reach, kind: Structure::Billboard { side: Side::Left } },
        Placement { z: 3.3 * reach, kind: Structure::Billboard { side: Side::Right } },
        // The long back straight before the Hard right — the one place on
        // the lap with real time to look around. Mile 1.70 in course
        // terms; expressed here as the reach count that lands there.
        Placement { z: 34.0 * reach, kind: Structure::Billboard { side: Side::Left } },
        Placement { z: 35.1 * reach, kind: Structure::BillboardOmarchy { side: Side::Right } },
        Placement { z: 36.2 * reach, kind: Structure::Billboard { side: Side::Left } },
    ]
}
