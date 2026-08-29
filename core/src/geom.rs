//! Geometry for a play field.
//!
//! Deliberately free of game concepts — no paddle, no ball, no bricks.
//! Everything here is pure maths over `f32`, which is what lets it be
//! tested exhaustively without constructing a game, and lets a game's
//! `physics.rs` stay about *rules* rather than arithmetic.
//!
//! All coordinates are **play-field** coordinates, not pixels. The
//! window gets tiled to whatever size Hyprland likes; a game plays
//! identically regardless because scaling happens at render time.
//!
//! This started life inside the Breakout crate. Its own first line
//! claimed it was free of game concepts, and that turned out to be
//! true enough that the second title needed every word of it — so it
//! moved here rather than being copied. Anything added below must keep
//! that property: if it names a game, it belongs in that game.

/// A 2D vector: a position, a velocity, or a displacement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Vec2 { x, y }
    }

    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }

    /// Unit vector in the same direction. Returns [`Vec2::ZERO`] for a
    /// zero-length input rather than NaN, so a stalled ball cannot
    /// poison every later calculation.
    pub fn normalized(self) -> Vec2 {
        let len = self.length();
        if len == 0.0 { Vec2::ZERO } else { Vec2::new(self.x / len, self.y / len) }
    }

    /// Same direction, given length. The ball uses this to keep a
    /// constant speed after a bounce changes its angle.
    pub fn with_length(self, len: f32) -> Vec2 {
        self.normalized() * len
    }
}

impl std::ops::Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}

impl std::ops::Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}

impl std::ops::Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, k: f32) -> Vec2 {
        Vec2::new(self.x * k, self.y * k)
    }
}

impl std::ops::AddAssign for Vec2 {
    fn add_assign(&mut self, o: Vec2) {
        *self = *self + o;
    }
}

/// An axis-aligned rectangle, positioned by its top-left corner.
///
/// Top-left origin with **y increasing downward**, matching the pixel
/// buffer. Keeping one convention from physics through to rendering
/// avoids a whole family of sign-flip bugs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Which axis a collision should be resolved along.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Rect { x, y, w, h }
    }

    /// Build from a centre point and half-extents — how the ball, which
    /// thinks of itself as a point with a radius, becomes a rect.
    pub fn from_center(c: Vec2, half_w: f32, half_h: f32) -> Self {
        Rect::new(c.x - half_w, c.y - half_h, half_w * 2.0, half_h * 2.0)
    }

    pub fn left(&self) -> f32 {
        self.x
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn top(&self) -> f32 {
        self.y
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub fn center(&self) -> Vec2 {
        Vec2::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    /// Do these rectangles genuinely overlap?
    ///
    /// **Strict** inequality: rectangles that merely touch edge-to-edge
    /// do not count. A ball resting exactly against a wall would
    /// otherwise re-collide every single frame and jitter in place.
    pub fn overlaps(&self, o: &Rect) -> bool {
        self.left() < o.right()
            && self.right() > o.left()
            && self.top() < o.bottom()
            && self.bottom() > o.top()
    }

    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.left() && p.x <= self.right() && p.y >= self.top() && p.y <= self.bottom()
    }

    /// How deeply `self` has penetrated `o` along each axis, as positive
    /// distances, or `None` if they do not overlap.
    ///
    /// This is the number collision response needs: the shallower axis
    /// is the one the collision actually happened on, and the depth is
    /// how far to push back out.
    pub fn penetration(&self, o: &Rect) -> Option<Vec2> {
        if !self.overlaps(o) {
            return None;
        }
        // Distance to push out in each direction; the smaller of the two
        // per axis is the real overlap.
        let dx = (self.right() - o.left()).min(o.right() - self.left());
        let dy = (self.bottom() - o.top()).min(o.bottom() - self.top());
        Some(Vec2::new(dx, dy))
    }

    /// The axis a collision with `o` should be resolved along: whichever
    /// is penetrated *less*, because that is the face the mover reached
    /// first.
    ///
    /// Resolving both axes at once is the classic corner bug — the ball
    /// reflects twice and comes back out the way it went in.
    pub fn collision_axis(&self, o: &Rect) -> Option<Axis> {
        let p = self.penetration(o)?;
        // An exact tie means a perfect corner strike, where neither face
        // was reached first and no answer is more correct than the other.
        // It resolves to Y, which is only a tie-break — a game that cares
        // which way a corner throws the ball should not be leaning on
        // this, it should test the incoming direction itself.
        Some(if p.x < p.y { Axis::X } else { Axis::Y })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::new(x, y, w, h)
    }

    #[test]
    fn vec_arithmetic() {
        let a = Vec2::new(3.0, 4.0);
        assert_eq!(a.length(), 5.0);
        assert_eq!(a + Vec2::new(1.0, 1.0), Vec2::new(4.0, 5.0));
        assert_eq!(a - Vec2::new(1.0, 1.0), Vec2::new(2.0, 3.0));
        assert_eq!(a * 2.0, Vec2::new(6.0, 8.0));
    }

    #[test]
    fn normalizing_zero_gives_zero_not_nan() {
        let n = Vec2::ZERO.normalized();
        assert_eq!(n, Vec2::ZERO);
        assert!(!n.x.is_nan() && !n.y.is_nan());
    }

    #[test]
    fn with_length_preserves_direction() {
        let v = Vec2::new(3.0, 4.0).with_length(10.0);
        assert!((v.length() - 10.0).abs() < 1e-5);
        assert!((v.x - 6.0).abs() < 1e-5);
        assert!((v.y - 8.0).abs() < 1e-5);
    }

    #[test]
    fn overlapping_rects_overlap() {
        assert!(r(0.0, 0.0, 10.0, 10.0).overlaps(&r(5.0, 5.0, 10.0, 10.0)));
        assert!(r(0.0, 0.0, 10.0, 10.0).overlaps(&r(-5.0, -5.0, 10.0, 10.0)));
    }

    #[test]
    fn separated_rects_do_not_overlap() {
        assert!(!r(0.0, 0.0, 10.0, 10.0).overlaps(&r(20.0, 0.0, 10.0, 10.0)));
        assert!(!r(0.0, 0.0, 10.0, 10.0).overlaps(&r(0.0, 20.0, 10.0, 10.0)));
    }

    /// The jitter bug: a ball resting exactly against a wall must not
    /// register a collision every frame.
    #[test]
    fn touching_edges_do_not_count_as_overlap() {
        let a = r(0.0, 0.0, 10.0, 10.0);
        assert!(!a.overlaps(&r(10.0, 0.0, 10.0, 10.0)), "right edge touch");
        assert!(!a.overlaps(&r(-10.0, 0.0, 10.0, 10.0)), "left edge touch");
        assert!(!a.overlaps(&r(0.0, 10.0, 10.0, 10.0)), "bottom edge touch");
        assert!(!a.overlaps(&r(0.0, -10.0, 10.0, 10.0)), "top edge touch");
    }

    #[test]
    fn from_center_round_trips() {
        let c = Vec2::new(50.0, 30.0);
        let rect = Rect::from_center(c, 5.0, 5.0);
        assert_eq!(rect, r(45.0, 25.0, 10.0, 10.0));
        assert_eq!(rect.center(), c);
    }

    #[test]
    fn penetration_is_none_when_apart() {
        assert_eq!(r(0.0, 0.0, 10.0, 10.0).penetration(&r(50.0, 50.0, 10.0, 10.0)), None);
    }

    #[test]
    fn penetration_measures_overlap_depth() {
        // Mover's right edge is 2 past the other's left edge.
        let p = r(0.0, 0.0, 10.0, 10.0).penetration(&r(8.0, 0.0, 10.0, 10.0)).unwrap();
        assert!((p.x - 2.0).abs() < 1e-5, "x penetration = 2, got {}", p.x);
    }

    /// A shallow side clip resolves on X; a shallow top hit resolves on
    /// Y. This is what stops corner hits reflecting twice.
    #[test]
    fn collision_axis_picks_the_shallower_penetration() {
        let brick = r(100.0, 100.0, 40.0, 20.0);

        // Barely clipping the left face: deep in y, shallow in x.
        let from_side = r(98.0, 105.0, 6.0, 6.0);
        assert_eq!(from_side.collision_axis(&brick), Some(Axis::X));

        // Barely clipping the top face: deep in x, shallow in y.
        let from_above = r(115.0, 98.0, 6.0, 6.0);
        assert_eq!(from_above.collision_axis(&brick), Some(Axis::Y));
    }

    #[test]
    fn contains_point() {
        let rect = r(0.0, 0.0, 10.0, 10.0);
        assert!(rect.contains(Vec2::new(5.0, 5.0)));
        assert!(rect.contains(Vec2::new(0.0, 0.0)));
        assert!(!rect.contains(Vec2::new(11.0, 5.0)));
    }
}
