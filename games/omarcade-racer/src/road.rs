//! The road model: the track as data, and the projection that puts it on
//! screen.
//!
//! This file draws nothing. It answers one question — *where on screen is
//! this point of track?* — and everything visual is built on top of the
//! answer.
//!
//! # Why this is not the sketch in `dump_art.rs`
//!
//! That sketch walks screen rows and asks "how far away is this row?"
//! (`z = 1/t`). For a still image that is enough. It cannot become a game,
//! because a road that **curves and scrolls** has curvature as a property
//! of *track distance*, not of screen row — the same screen row is a
//! different piece of track on the next frame.
//!
//! So the direction is inverted here. The track is a list of segments at
//! fixed positions in track-z; each one projects *forward* to a screen-y.
//! That inversion is the whole reason this is real code and the sketch was
//! a sketch.
//!
//! # Everything here is a ratio
//!
//! L015 and L019, both earned on this project: a constant tuned at one
//! scale is an untested assumption at every other. The sketch's `1150.0`
//! (road half-width) and `105.0` (sprite scale divisor) are exactly that —
//! absolutes calibrated against 960x720 that mean nothing at another size.
//!
//! Nothing here is pinned to a resolution. `camera_depth` comes from a
//! field-of-view *angle*; widths are in world units divided by distance.
//! Change the window size and the road looks the same, because the only
//! screen-space number in the projection is the height it scales into.


/// One slice of track.
///
/// `curve` is how much the road's direction changes *through* this segment.
/// It is not an angle and not a screen offset — it accumulates (see
/// [`Road::project`]), and that double-integration is what makes a bend
/// read as a curve rather than as a diagonal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    /// Direction change through this segment. Positive bends right.
    pub curve: f32,
    /// Height change through this segment.
    ///
    /// **Always 0.0 for now.** The lineage this game sits in was flat, and
    /// pitch is a materially harder projection — a deliberate decision, not
    /// an omission. The field exists so that adding hills later is a change
    /// to [`Road::project`] alone, and not a migration of every track that
    /// has been authored by then.
    pub pitch: f32,
}

impl Segment {
    /// A flat, straight slice — the default piece of road.
    pub const STRAIGHT: Segment = Segment { curve: 0.0, pitch: 0.0 };

    /// A flat slice bending by `curve`. Positive bends right.
    pub const fn curving(curve: f32) -> Segment {
        Segment { curve, pitch: 0.0 }
    }
}

/// How the camera sees the road.
///
/// Only two numbers, and neither is in pixels.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    /// Height above the road surface, in world units.
    pub height: f32,
    /// Horizontal field of view, in radians.
    ///
    /// Stored as an angle rather than as a projection-plane distance so it
    /// stays a ratio (L019). `depth()` turns it into the number the
    /// projection actually multiplies by.
    pub fov: f32,
}

impl Camera {
    /// The projection-plane distance implied by the field of view.
    ///
    /// This is the classic `1 / tan(fov / 2)`: the distance at which a
    /// world unit subtends the half-screen. A narrower FOV pushes the plane
    /// further out, which magnifies everything and flattens the road's
    /// apparent curvature — it is the "zoom" of the whole scene.
    pub fn depth(&self) -> f32 {
        1.0 / (self.fov / 2.0).tan()
    }

    /// Derive a camera from the road it is looking at, rather than picking
    /// numbers and hoping.
    ///
    /// This exists because the first version of this file did pick numbers
    /// — `height: 1000.0, fov: 0.5 * PI` — and the render showed the road
    /// covering 110% of the screen at the nearest band, a solid slab with
    /// its rumble strips off both edges. That is L015 exactly: a constant
    /// chosen before there was a system to measure it against.
    ///
    /// So the camera is solved from two things that can actually be
    /// *judged by eye*:
    ///
    /// - `fill`: how much of the screen width the road covers at the
    ///   nearest drawn band. Under 1.0 by definition — at 1.0 the verges
    ///   sit exactly on the screen edges and the rumble strips vanish.
    /// - the requirement that the nearest band meets the bottom of the
    ///   screen, so there is no gap under the road.
    ///
    /// Both inputs are ratios, so this holds at any resolution and any
    /// road width.
    pub fn for_road(road: &Road, fill: f32) -> Camera {
        assert!(
            fill > 0.0 && fill < 1.0,
            "fill is a fraction of screen width and must be inside (0,1), got {fill}",
        );
        // scale at the nearest band, from: fill = scale * road_width / 2
        let scale_near = fill * 2.0 / road.width();
        // scale = depth / distance, and the nearest band is one segment out
        let depth = scale_near * road.segment_length();
        Camera {
            // Solved so the nearest band lands on the bottom edge.
            height: 1.0 / scale_near,
            fov: 2.0 * (1.0 / depth).atan(),
        }
    }
}

impl Default for Camera {
    /// The camera for a default-shaped road, at the fill that reads best.
    ///
    /// 0.85 leaves the verges and rumble strips on screen at the bumper
    /// while still filling the frame.
    fn default() -> Self {
        Camera::for_road(&Road::straight(1), 0.85)
    }
}

/// Where a point of track lands on screen.
///
/// Screen coordinates stay **floating point** on purpose. Rounding to a
/// pixel is the caller's business, and doing it in exactly one place is
/// what keeps the half-pixel error in L018 from getting in — a sprite
/// offset by half a pixel is invisible in a still and looks exactly like a
/// physics bug once things move.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Projected {
    /// Screen row. Smaller is further away; the horizon is the limit.
    pub y: f32,
    /// Screen column of the road's centre line, curvature included.
    pub x: f32,
    /// Road half-width in screen pixels.
    pub half_width: f32,
    /// World-to-screen scale at this distance. A sprite `w` world units
    /// wide covers `w * scale` pixels.
    pub scale: f32,
    /// Distance ahead of the camera, in world units. Useful for haze and
    /// for deciding what is worth drawing at all.
    pub distance: f32,
}

/// A track: segments, and the projection that puts them on screen.
pub struct Road {
    segments: Vec<Segment>,
    /// Length of one segment in world units.
    segment_length: f32,
    /// Full road width in world units, centre line to centre line of the
    /// verges. Every other width in the game is a fraction of this.
    width: f32,
    /// How far ahead the camera can see, in segments. Beyond this, road is
    /// not drawn — it is under a pixel tall and costs more than it shows.
    draw_distance: usize,
}

impl Road {
    /// Build a track from segments.
    ///
    /// # Panics
    /// On an empty segment list. A road with no segments has no geometry to
    /// project against, and every method here would have to invent an
    /// answer — better to fail at construction than to return quiet
    /// nonsense from `project`. This matches `Sprite::new`, which panics on
    /// a ragged row for the same reason.
    pub fn new(segments: Vec<Segment>, segment_length: f32, width: f32) -> Road {
        assert!(
            !segments.is_empty(),
            "a road needs at least one segment; got none",
        );
        assert!(
            segment_length > 0.0 && segment_length.is_finite(),
            "segment_length must be positive and finite, got {segment_length}",
        );
        assert!(
            width > 0.0 && width.is_finite(),
            "road width must be positive and finite, got {width}",
        );
        Road { segments, segment_length, width, draw_distance: 300 }
    }

    /// A straight test track of `n` segments.
    ///
    /// The dimensions are the ones the rest of the game derives from, and
    /// they are chosen relative to each other rather than to any screen: a
    /// segment is about half a road-width long, which is what makes rumble
    /// banding land at a believable rate as it streams past.
    pub fn straight(n: usize) -> Road {
        Road::new(vec![Segment::STRAIGHT; n], 1000.0, 2200.0)
    }

    /// Total track length in world units. The track loops at this point.
    pub fn length(&self) -> f32 {
        self.segments.len() as f32 * self.segment_length
    }

    pub fn width(&self) -> f32 {
        self.width
    }

    pub fn segment_length(&self) -> f32 {
        self.segment_length
    }

    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    pub fn draw_distance(&self) -> usize {
        self.draw_distance
    }

    pub fn set_draw_distance(&mut self, segments: usize) {
        self.draw_distance = segments;
    }

    /// Wrap a track position into the track, so the road loops.
    ///
    /// `rem_euclid`, never `%`: a camera that reverses past the start line
    /// would get a negative index out of `%` and panic. That is in this
    /// project's gotchas because it has already been paid for once.
    pub fn wrap(&self, z: f32) -> f32 {
        z.rem_euclid(self.length())
    }

    /// The segment containing track position `z`, wrapping.
    pub fn segment_at(&self, z: f32) -> Segment {
        self.segments[self.segment_index_at(z)]
    }

    /// Index of the segment containing `z`, wrapping.
    pub fn segment_index_at(&self, z: f32) -> usize {
        let i = (self.wrap(z) / self.segment_length) as usize;
        // `wrap` guarantees `z < length`, so the division is in range — but
        // float division at the very top of the range can round up to
        // exactly `len()`. Clamping costs nothing and turns a rare panic
        // into a correct answer.
        i.min(self.segments.len() - 1)
    }

    /// Project a point on the track to the screen.
    ///
    /// `camera_z` is where the camera sits along the track; `z` is the
    /// point being projected; `x_offset` is how far the camera is from the
    /// road's centre line, in world units, positive to the right.
    ///
    /// `screen_w` and `screen_h` are the only screen-space numbers in the
    /// whole model, and they appear exactly here.
    ///
    /// Returns `None` when the point is at or behind the camera plane,
    /// where the projection has no meaning — dividing by that distance
    /// would produce an infinity and paint a road across the sky.
    pub fn project(
        &self,
        camera: &Camera,
        camera_z: f32,
        x_offset: f32,
        z: f32,
        screen_w: f32,
        screen_h: f32,
    ) -> Option<Projected> {
        let distance = z - camera_z;
        if distance <= 0.0 || !distance.is_finite() {
            return None;
        }

        // The projection proper. Everything an arcade racer draws follows
        // from this one ratio: things `depth / distance` times smaller the
        // further away they are.
        let scale = camera.depth() / distance;

        // Screen y. The camera looks at the horizon, so a point at the
        // road surface (camera.height below the camera) falls *below* the
        // horizon by its projected height, and approaches the horizon as
        // distance grows. The horizon sits at the vertical centre.
        //
        // Scaling by `screen_h / 2` — half the screen, matching the
        // half-angle in `depth()` — is what keeps this resolution
        // independent.
        let horizon = screen_h / 2.0;
        let y = horizon + scale * camera.height * (screen_h / 2.0);

        // Road half-width, and the centre line displaced by however much
        // curvature has accumulated between the camera and this point.
        let half_width = scale * (self.width / 2.0) * (screen_w / 2.0);
        // Curvature and steering are NOT the same kind of quantity, and
        // multiplying both by `scale` is the bug this comment exists to
        // prevent coming back. The first version of this file did exactly
        // that, and every curve rendered as a dead-straight road.
        //
        // - `x_offset` is a fixed lateral distance in world units. It is
        //   real perspective: standing 500 units left of the centre line
        //   shifts the road a lot up close and almost nothing at the
        //   horizon, so it MUST be divided by distance — i.e. scaled.
        //
        // - `curve_x` is accumulated one step per segment walked, so it
        //   already grows with distance by construction. Scaling it again
        //   divides by the same distance it was just multiplied by; the
        //   two cancel and the bend flattens out to nothing.
        //
        // So the curve is applied in screen space directly, and only the
        // steering offset goes through the projection.
        let curve_x = self.accumulated_curve(camera_z, z);
        let x = screen_w / 2.0 + curve_x * (screen_w / 2.0)
            - scale * x_offset * (screen_w / 2.0);

        Some(Projected { y, x, half_width, scale, distance })
    }

    /// How far the road's centre has wandered from the camera's line of
    /// sight by track position `z`, in world units.
    ///
    /// Curvature is applied **twice**: each segment's curve adds to a
    /// running direction, and that direction adds to a running offset. A
    /// single integration would give a road that changes angle at each
    /// segment — a polyline. The second integration is what makes it a
    /// curve, and it is the piece the `dump_art.rs` sketch faked with a
    /// hand-tuned `z * z` term.
    ///
    /// Walking segment by segment is O(distance), which is bounded by
    /// `draw_distance` and therefore constant per frame in practice.
    ///
    /// # What a curve value means
    ///
    /// The result is a fraction of half the screen: 1.0 pushes the road's
    /// centre from the middle of the screen to its edge.
    ///
    /// That normalisation is not cosmetic. Raw double integration grows as
    /// n² in the number of segments walked — at 100 segments a curve of
    /// 2.0 accumulates to 10,100, which put the road four million pixels
    /// off centre and was how this was found. A `curve` with no unit
    /// cannot be tuned, because no value of it is right.
    ///
    /// So the accumulation is divided by the same n² it grows by, taken
    /// over the draw distance. `curve: 1.0` then means "displace the road
    /// by one half-screen over the full visible distance", which is a
    /// number a person can reason about and a track author can pick.
    /// L019: a ratio, not an absolute.
    fn accumulated_curve(&self, camera_z: f32, z: f32) -> f32 {
        let span = z - camera_z;
        if span <= 0.0 {
            return 0.0;
        }

        let mut direction = 0.0f32;
        let mut offset = 0.0f32;
        let mut walked = 0.0f32;

        while walked < span {
            // The last step is a partial segment. Weighting by how much of
            // the segment is actually covered is what stops the road
            // twitching sideways as the camera crosses a segment boundary.
            let step = (span - walked).min(self.segment_length) / self.segment_length;
            let seg = self.segment_at(camera_z + walked);

            direction += seg.curve * step;
            offset += direction * step;
            walked += self.segment_length;
        }

        // Normalise by the n² the double integration grows by, measured
        // over the draw distance, so the value means the same thing
        // regardless of how far the camera can see.
        let n = self.draw_distance as f32;
        offset / (n * (n + 1.0) / 2.0)
    }

    /// Project every visible segment boundary, nearest first.
    ///
    /// This is what a renderer walks: each entry is the far edge of one
    /// band of road, and drawing between consecutive entries fills the
    /// screen with no gaps and no overdraw.
    ///
    /// Segments past the horizon are dropped rather than drawn at sub-pixel
    /// height — they cost a scanline each and show nothing.
    pub fn visible(
        &self,
        camera: &Camera,
        camera_z: f32,
        x_offset: f32,
        screen_w: f32,
        screen_h: f32,
    ) -> Vec<Projected> {
        let mut out = Vec::with_capacity(self.draw_distance);
        // Start at the boundary of the segment the camera is in, so the
        // nearest band is a whole segment rather than a sliver that
        // changes size every frame.
        let first = (camera_z / self.segment_length).floor() * self.segment_length;

        for i in 0..self.draw_distance {
            let z = first + (i as f32 + 1.0) * self.segment_length;
            match self.project(camera, camera_z, x_offset, z, screen_w, screen_h) {
                // Above the horizon line is behind the camera's view of
                // the world; nothing further can be visible either.
                Some(p) if p.y <= screen_h / 2.0 => break,
                Some(p) => out.push(p),
                None => continue,
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    const W: f32 = 960.0;
    const H: f32 = 720.0;

    fn cam() -> Camera {
        Camera::default()
    }

    /// The single most obvious way a pseudo-3D racer looks wrong, called
    /// out by name in the racer's README: a road that narrows toward the
    /// viewer. Worth a test that fails loudly if the projection is ever
    /// inverted.
    #[test]
    fn road_widens_toward_the_viewer() {
        let road = Road::straight(64);
        let near = road.project(&cam(), 0.0, 0.0, 1_000.0, W, H).unwrap();
        let far = road.project(&cam(), 0.0, 0.0, 20_000.0, W, H).unwrap();

        assert!(
            near.half_width > far.half_width,
            "near road ({}) must be wider than far road ({})",
            near.half_width,
            far.half_width,
        );
    }

    /// Nearer track must land lower on screen, always. A projection that
    /// is not monotonic paints bands out of order and tears the road.
    #[test]
    fn nearer_track_lands_lower_on_screen() {
        let road = Road::straight(64);
        let mut last = f32::INFINITY;
        for i in 1..40 {
            let z = i as f32 * 1_000.0;
            let p = road.project(&cam(), 0.0, 0.0, z, W, H).unwrap();
            assert!(
                p.y < last,
                "z={z} projected to y={} which is not above the previous {last}",
                p.y,
            );
            last = p.y;
        }
    }

    /// Distant road approaches the horizon without ever crossing it.
    /// Crossing would paint road into the sky.
    #[test]
    fn distant_road_approaches_but_never_crosses_the_horizon() {
        let road = Road::straight(64);
        let horizon = H / 2.0;
        let far = road.project(&cam(), 0.0, 0.0, 10_000_000.0, W, H).unwrap();

        assert!(far.y > horizon, "road at y={} crossed the horizon {horizon}", far.y);
        assert!(
            far.y - horizon < 1.0,
            "road at 10M units should be hard against the horizon, was {} above it",
            far.y - horizon,
        );
    }

    /// The camera plane itself has no projection. Returning a number here
    /// would be an infinity dressed up as a coordinate.
    #[test]
    fn nothing_at_or_behind_the_camera_projects() {
        let road = Road::straight(64);
        assert!(road.project(&cam(), 5_000.0, 0.0, 5_000.0, W, H).is_none());
        assert!(road.project(&cam(), 5_000.0, 0.0, 4_999.0, W, H).is_none());
        assert!(road.project(&cam(), 5_000.0, 0.0, 5_001.0, W, H).is_some());
    }

    /// A straight road stays centred no matter how far down it you look.
    #[test]
    fn a_straight_road_does_not_wander() {
        let road = Road::straight(64);
        for i in 1..30 {
            let p = road.project(&cam(), 0.0, 0.0, i as f32 * 1_000.0, W, H).unwrap();
            assert!(
                (p.x - W / 2.0).abs() < 0.001,
                "straight road drifted to x={} at segment {i}",
                p.x,
            );
        }
    }

    /// A bend must displace the road, must displace it *more* with
    /// distance, and must displace it in the direction of its sign.
    #[test]
    fn a_bend_displaces_the_road_and_grows_with_distance() {
        let road = Road::new(vec![Segment::curving(3.0); 64], 1000.0, 2200.0);
        let near = road.accumulated_curve(0.0, 2_000.0);
        let far = road.accumulated_curve(0.0, 10_000.0);

        assert!(near > 0.0, "a right bend must displace right, got {near}");
        assert!(
            far > near,
            "displacement must grow with distance: {far} at 10k vs {near} at 2k",
        );

        let left = Road::new(vec![Segment::curving(-3.0); 64], 1000.0, 2200.0);
        assert!(
            left.accumulated_curve(0.0, 10_000.0) < 0.0,
            "a left bend must displace left",
        );
    }

    /// Curvature is integrated twice, so displacement grows faster than
    /// linearly. A single integration — the bug this guards — would give a
    /// straight diagonal, where doubling distance exactly doubles offset.
    #[test]
    fn curvature_is_integrated_twice_not_once() {
        let road = Road::new(vec![Segment::curving(2.0); 200], 1000.0, 2200.0);
        let d1 = road.accumulated_curve(0.0, 10_000.0);
        let d2 = road.accumulated_curve(0.0, 20_000.0);

        assert!(
            d2 > d1 * 2.5,
            "doubling distance should more than double offset \
             (quadratic, not linear): {d1} then {d2}",
        );
    }

    /// A road with no curve accumulates nothing, at any distance.
    #[test]
    fn a_straight_accumulates_no_curve() {
        let road = Road::straight(64);
        assert_eq!(road.accumulated_curve(0.0, 30_000.0), 0.0);
    }

    /// The track loops, including backwards past the start line. `%` would
    /// give a negative index here and panic.
    #[test]
    fn the_track_wraps_in_both_directions() {
        let road = Road::straight(10);
        let len = road.length();
        assert_eq!(len, 10_000.0);

        assert!((road.wrap(len + 250.0) - 250.0).abs() < 0.001);
        assert!((road.wrap(-250.0) - (len - 250.0)).abs() < 0.001);
        assert_eq!(road.segment_index_at(-1.0), 9);
        assert_eq!(road.segment_index_at(len), 0);
        assert_eq!(road.segment_index_at(len - 1.0), 9);
    }

    /// Segment lookup must land on the right segment, not one either side.
    #[test]
    fn segment_lookup_lands_on_the_right_segment() {
        let segments: Vec<Segment> =
            (0..8).map(|i| Segment::curving(i as f32)).collect();
        let road = Road::new(segments, 100.0, 2200.0);

        assert_eq!(road.segment_at(0.0).curve, 0.0);
        assert_eq!(road.segment_at(99.9).curve, 0.0);
        assert_eq!(road.segment_at(100.0).curve, 1.0);
        assert_eq!(road.segment_at(750.0).curve, 7.0);
        // ...and wraps back to the first.
        assert_eq!(road.segment_at(800.0).curve, 0.0);
    }

    /// The visible list must be ordered near-to-far and stop at the
    /// horizon, which is what lets a renderer walk it without sorting.
    #[test]
    fn visible_segments_are_ordered_near_to_far() {
        let road = Road::straight(400);
        let bands = road.visible(&cam(), 0.0, 0.0, W, H);

        assert!(!bands.is_empty(), "a straight road should have visible bands");
        for pair in bands.windows(2) {
            assert!(
                pair[1].y < pair[0].y,
                "bands out of order: {} then {}",
                pair[0].y,
                pair[1].y,
            );
            assert!(pair[1].distance > pair[0].distance);
        }
        let horizon = H / 2.0;
        assert!(bands.last().unwrap().y > horizon);
    }

    /// Steering off-centre must slide the road the other way — the camera
    /// moving right is the road moving left — and by more when nearer.
    #[test]
    fn steering_offset_slides_the_road_the_other_way() {
        let road = Road::straight(64);
        let centred = road.project(&cam(), 0.0, 0.0, 2_000.0, W, H).unwrap();
        let right = road.project(&cam(), 0.0, 500.0, 2_000.0, W, H).unwrap();

        assert!(
            right.x < centred.x,
            "moving the camera right must move the road left: {} vs {}",
            right.x,
            centred.x,
        );

        let far = road.project(&cam(), 0.0, 500.0, 20_000.0, W, H).unwrap();
        assert!(
            (far.x - W / 2.0).abs() < (right.x - W / 2.0).abs(),
            "a steering offset must matter less at distance",
        );
    }

    /// Resolution independence, which is the whole point of expressing the
    /// camera as an angle (L019). The road must cover the same *fraction*
    /// of the screen at any size.
    #[test]
    fn the_projection_is_resolution_independent() {
        let road = Road::straight(64);
        let small = road.project(&cam(), 0.0, 0.0, 3_000.0, 480.0, 360.0).unwrap();
        let large = road.project(&cam(), 0.0, 0.0, 3_000.0, 1920.0, 1440.0).unwrap();

        let small_frac = small.half_width / 480.0;
        let large_frac = large.half_width / 1920.0;
        assert!(
            (small_frac - large_frac).abs() < 0.0001,
            "road covers {small_frac} of a small screen but {large_frac} of a large one",
        );

        let small_y = (small.y - 180.0) / 360.0;
        let large_y = (large.y - 720.0) / 1440.0;
        assert!(
            (small_y - large_y).abs() < 0.0001,
            "road sits at {small_y} down a small screen but {large_y} down a large one",
        );
    }

    /// A narrower lens magnifies. This is the knob that decides how much
    /// road is on screen, and it must move in the direction it reads as.
    #[test]
    fn a_narrower_lens_magnifies() {
        let road = Road::straight(64);
        let wide = Camera { height: 1000.0, fov: 0.8 * PI };
        let narrow = Camera { height: 1000.0, fov: 0.3 * PI };

        let w = road.project(&wide, 0.0, 0.0, 5_000.0, W, H).unwrap();
        let n = road.project(&narrow, 0.0, 0.0, 5_000.0, W, H).unwrap();
        assert!(n.half_width > w.half_width);
    }

    /// A bend must actually reach the SCREEN, not merely accumulate.
    ///
    /// This is the test that was missing. `a_bend_displaces_the_road_and_
    /// grows_with_distance` checked `accumulated_curve` in isolation and
    /// passed, while every rendered curve came out dead straight — because
    /// `project` multiplied the accumulated curve by `scale`, which falls
    /// as 1/distance and cancelled the growth almost exactly.
    ///
    /// Testing the intermediate value proved nothing about the output.
    /// L017: ask what change would make the test fail. The old one would
    /// not have failed on the bug that shipped.
    #[test]
    fn a_bend_visibly_moves_the_road_on_screen() {
        let road = Road::new(vec![Segment::curving(2.0); 300], 1000.0, 2200.0);
        let camera = Camera::for_road(&road, 0.85);
        let bands = road.visible(&camera, 0.0, 0.0, W, H);

        let nearest = bands.first().unwrap();
        let furthest = bands.last().unwrap();

        // The road under the bumper is still ahead of us, so it stays put.
        assert!(
            (nearest.x - W / 2.0).abs() < W * 0.05,
            "the road at the bumper should be roughly centred, was {}",
            nearest.x,
        );
        // ...and by the horizon the bend must have gone somewhere visible.
        assert!(
            furthest.x - W / 2.0 > W * 0.15,
            "a sustained right bend moved the far road only {} px from centre \
             on a {W}px screen — it will render as a straight line",
            furthest.x - W / 2.0,
        );
    }

    /// The mirror of the above: a left bend must go left, on screen.
    #[test]
    fn a_left_bend_moves_the_road_left_on_screen() {
        let road = Road::new(vec![Segment::curving(-2.0); 300], 1000.0, 2200.0);
        let camera = Camera::for_road(&road, 0.85);
        let bands = road.visible(&camera, 0.0, 0.0, W, H);
        assert!(bands.last().unwrap().x < W / 2.0 - W * 0.15);
    }

    /// Steering must keep behaving like perspective even though curvature
    /// no longer does: a fixed lateral offset matters less with distance.
    /// If someone "fixes" the curve bug by unscaling both, this fails.
    #[test]
    fn steering_still_falls_away_with_distance() {
        let road = Road::straight(300);
        let camera = Camera::for_road(&road, 0.85);

        let near = road.project(&camera, 0.0, 400.0, 2_000.0, W, H).unwrap();
        let far = road.project(&camera, 0.0, 400.0, 40_000.0, W, H).unwrap();

        let near_shift = (near.x - W / 2.0).abs();
        let far_shift = (far.x - W / 2.0).abs();
        assert!(
            near_shift > far_shift * 4.0,
            "steering should fall away with distance: {near_shift} near vs {far_shift} far",
        );
    }

    /// The DEFAULT camera must obey the same rule, not only an explicitly
    /// derived one.
    ///
    /// Found by mutation: restoring the original guessed camera
    /// (`height: 1000.0, fov: 0.5 * PI`) — the one whose render was a
    /// featureless slab — left every test passing, because they all called
    /// `for_road` directly and nothing exercised `Default`. A guard that
    /// cannot fail on the bug it exists to catch is not a guard (L017).
    #[test]
    fn the_default_camera_also_fits_the_screen() {
        let road = Road::straight(400);
        let bands = road.visible(&Camera::default(), 0.0, 0.0, W, H);
        let nearest = bands.first().unwrap();

        let covered = nearest.half_width * 2.0 / W;
        assert!(
            (0.5..=1.0).contains(&covered),
            "the default camera puts {covered:.2} of the screen under road",
        );
        assert!(
            (nearest.y - H).abs() < 1.0,
            "the default camera leaves the road ending at y={}, not {H}",
            nearest.y,
        );
    }

    /// The nearest band must fit ON the screen, verges included.
    ///
    /// The first camera in this file failed this at 110% of screen width:
    /// the bottom third of the render was a featureless slab with its
    /// rumble strips off both edges. Measured, not assumed.
    #[test]
    fn the_nearest_band_fits_on_screen() {
        let road = Road::straight(400);
        let camera = Camera::for_road(&road, 0.85);
        let bands = road.visible(&camera, 0.0, 0.0, W, H);

        let nearest = bands.first().expect("a road should have visible bands");
        let covered = nearest.half_width * 2.0 / W;
        assert!(
            covered <= 1.0,
            "the nearest band covers {covered:.2} of the screen — the verges are off it",
        );
        assert!(
            covered > 0.5,
            "the nearest band covers only {covered:.2} of the screen — the road looks distant",
        );
    }

    /// ...and it must reach the bottom edge, or there is a gap under the
    /// road that the player's own car floats above.
    #[test]
    fn the_nearest_band_reaches_the_bottom_of_the_screen() {
        let road = Road::straight(400);
        let camera = Camera::for_road(&road, 0.85);
        let bands = road.visible(&camera, 0.0, 0.0, W, H);

        let nearest = bands.first().unwrap();
        assert!(
            (nearest.y - H).abs() < 1.0,
            "the nearest band ends at y={} but the screen is {H} tall",
            nearest.y,
        );
    }

    /// The camera must follow the road's dimensions, not carry numbers
    /// that only suit one road. This is the L019 guard on the camera
    /// itself: a wider road needs a differently-placed camera to look the
    /// same, and `for_road` is what supplies it.
    #[test]
    fn the_camera_adapts_to_the_road_it_looks_at() {
        let narrow = Road::new(vec![Segment::STRAIGHT; 400], 1000.0, 1100.0);
        let wide = Road::new(vec![Segment::STRAIGHT; 400], 1000.0, 4400.0);

        let cn = Camera::for_road(&narrow, 0.85);
        let cw = Camera::for_road(&wide, 0.85);

        let n = narrow.visible(&cn, 0.0, 0.0, W, H)[0];
        let w = wide.visible(&cw, 0.0, 0.0, W, H)[0];

        // Different roads, same *look*: both fill the frame the same way.
        assert!(
            (n.half_width - w.half_width).abs() < 0.5,
            "a 4x wider road should still fill the screen the same: {} vs {}",
            n.half_width,
            w.half_width,
        );
        // ...which it can only do by standing further back.
        assert!(
            cw.height > cn.height * 3.0,
            "a wider road needs a higher camera: {} vs {}",
            cw.height,
            cn.height,
        );
    }

    #[test]
    #[should_panic(expected = "inside (0,1)")]
    fn a_fill_that_overflows_the_screen_is_rejected() {
        Camera::for_road(&Road::straight(8), 1.4);
    }

    /// Pitch is carried but deliberately unused. If this ever fails,
    /// someone has started building hills, and the decision to stay flat
    /// needs revisiting rather than the test being edited.
    #[test]
    fn the_track_is_flat_for_now() {
        let road = Road::straight(64);
        assert!(
            (0..road.segment_count()).all(|i| road.segments[i].pitch == 0.0),
            "the road model is flat by decision; pitch must stay zero",
        );
    }

    #[test]
    #[should_panic(expected = "at least one segment")]
    fn an_empty_road_is_rejected() {
        Road::new(vec![], 1000.0, 2200.0);
    }
}
