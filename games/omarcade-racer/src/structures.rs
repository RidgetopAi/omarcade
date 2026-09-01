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
    /// A billboard beside the road.
    Billboard { side: Side },
}

/// One structure at one place on the track.
#[derive(Clone, Copy, Debug)]
pub struct Placement {
    /// Where along the track, in world units. Fixed — that is the whole
    /// point of this module.
    pub z: f32,
    pub kind: Structure,
}

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

/// How far off the centre line a billboard stands, in half-widths.
///
/// Past 1.0 by enough to clear the verge and the rumble strip. Further out
/// than about 1.4 and it drifts toward the horizon fast enough to be
/// unreadable before it is close.
const BILLBOARD_OFFSET_HALF_WIDTHS: f32 = 1.25;

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
            let d = ahead_of(road, camera_z, p.z);
            let reach = road.draw_distance() as f32 * road.segment_length();
            // `d >= 0.0`, NOT `> 0.0`. A structure the camera is standing
            // exactly on is at distance zero, and excluding it means the
            // start gantry blinks out at the precise moment you cross the
            // line — the one frame it most needs to be there. `project`
            // rejects a non-positive distance on its own, so the zero case
            // is handled without a special case here.
            (d >= 0.0 && d < reach).then_some((d, p))
        })
        .collect();
    visible.sort_by(|a, b| b.0.total_cmp(&a.0));

    for (_, placement) in visible {
        let Some(p) = road.project(
            camera,
            camera_z,
            x_offset,
            camera_z + ahead_of(road, camera_z, placement.z),
            fw,
            fh,
        ) else {
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
        }
    }
}

/// How far ahead of the camera a track position is, accounting for the
/// track looping.
///
/// A structure at z=0 is not behind you on lap two; it is a whole lap
/// ahead. Without this, the start gantry vanishes the moment it is passed
/// and never returns.
fn ahead_of(road: &Road, camera_z: f32, z: f32) -> f32 {
    let lap = road.segment_count() as f32 * road.segment_length();
    (z - camera_z).rem_euclid(lap)
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

    let x = fx + p.x + side.sign() * p.half_width * BILLBOARD_OFFSET_HALF_WIDTHS
        - sprite.ink_centre_bias() * scale;
    let y = fy + p.y + sprite.ink_foot_gap() * scale;
    sprite.draw_ground(c, x, y, scale);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::{grand_prix, UNITS_PER_MILE};

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
            if !matches!(p.kind, Structure::Billboard { .. }) {
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
        Placement { z: 1.1 * reach, kind: Structure::Billboard { side: Side::Right } },
        Placement { z: 2.2 * reach, kind: Structure::Billboard { side: Side::Left } },
        Placement { z: 3.3 * reach, kind: Structure::Billboard { side: Side::Right } },
        // The long back straight before the Hard right — the one place on
        // the lap with real time to look around. Mile 1.70 in course
        // terms; expressed here as the reach count that lands there.
        Placement { z: 34.0 * reach, kind: Structure::Billboard { side: Side::Left } },
        Placement { z: 35.1 * reach, kind: Structure::Billboard { side: Side::Right } },
        Placement { z: 36.2 * reach, kind: Structure::Billboard { side: Side::Left } },
    ]
}
