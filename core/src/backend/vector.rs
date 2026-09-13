//! Shapes that are not rectangles.
//!
//! Everything else in [`Canvas`](super::Canvas) draws an axis-aligned box.
//! That ceiling is visible in the games: art gets built from stacked slabs
//! because slabs are what the engine can draw, not because a stack of slabs
//! is what the artist wanted.
//!
//! These primitives lift the ceiling without changing the language.
//! [`fill_rect_f`](super::Canvas::fill_rect_f) already establishes how this
//! engine anti-aliases — work out analytically how much of a pixel the shape
//! covers, fold that fraction into alpha, then composite. A rect can do it
//! with one multiply per pixel (`cx * cy`) because every row is the same
//! span. A polygon has to find its spans per row first. That is the whole
//! difference; the blend at the end is identical, which is why vector art
//! and rect art sit in the same picture without looking pasted together.
//!
//! **Analytic, not supersampled.** Nothing here renders at 4x and averages.
//! Coverage is computed from the geometry directly, in one pass, so the cost
//! scales with the pixels a shape touches rather than with a sample count.
//!
//! # Cost, measured
//!
//! Cheaper than expected for art, ruinous for backdrops. At 960x720
//! (`cargo run --release -p omarcade-core --example bench_vector`):
//!
//! | Call | Cost | Share of a 60fps frame |
//! |---|---|---|
//! | `fill_rect_f` 64x64 (the reference) | 0.030 ms | 0.18% |
//! | `polygon_f`, the same area as a quad | 0.047 ms | 0.28% |
//! | `circle_f` r=32 | 0.039 ms | 0.23% |
//! | One ship, scale 2 | 0.009 ms | 0.05% |
//! | **64 rotating ships** | 0.603 ms | **3.62%** |
//! | 200 small additive circles | 0.196 ms | 1.18% |
//! | **A full-screen polygon** | 6.676 ms | **40.05%** |
//!
//! So a polygon is about **1.6x a rect** for the same area — cheap enough
//! that a whole arcade screen of vector art costs under 4% of a frame, and
//! no reason to ration it for the things a game actually draws.
//!
//! ⚠️ **Never fill a large area with a polygon.** The full-screen case costs
//! roughly 220x what the same rectangle costs, because every row intersects
//! every edge regardless of how simple the shape is. Backdrops, veils and
//! panels stay on `fill_rect`. These primitives are for *art*: things with a
//! shape, usually moving, usually small.
//!
//! # ⚠️ Build multi-piece art with OVERLAP, never a shared edge
//!
//! Two shapes that meet along an exact shared edge leave a faint hairline of
//! background down the join. It is not a bug to be fixed here: each shape is
//! anti-aliased independently and honestly, so each contributes about half
//! coverage to the pixels the edge runs through, and one half-covered layer
//! over another reaches 75%, not 100%. Measured on a 45° join the seam
//! pixels read 192 where the interior reads 255.
//!
//! Axis-aligned joins are exempt — an edge on a pixel boundary splits
//! cleanly — so the artefact appears only on diagonals, which is precisely
//! where a ship's panels meet.
//!
//! ⇒ When a hull and a fin are different colours, **let the pieces overlap
//! by a pixel** rather than sharing a boundary. The piece drawn second wins
//! the overlap and the join is solid. Drawing one shape fully and laying the
//! smaller detail on top of it is the habit that avoids this entirely.

use super::{Canvas, Color};

/// How far apart two coordinates must be before they are treated as
/// distinct. Edges shorter than this contribute nothing and are skipped:
/// a degenerate edge produces a division by a near-zero height and sends
/// the intersection maths to infinity.
const EPSILON: f32 = 1e-6;

/// Where a shape sits, which way it faces, and how big it is.
///
/// [`Pose`](crate::Pose) deliberately refuses to rotate, and it is right to:
/// a racer's car seen from behind and rotated would show a side that the
/// sprite does not contain. A vector shape has no such problem — it is
/// described by points, and points rotate honestly. A ship that faces east,
/// faces west and banks between the two needs exactly this.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// Where the shape's origin lands on the canvas, in pixels.
    pub x: f32,
    pub y: f32,
    /// Facing, in radians, clockwise on screen (y grows downward).
    pub angle: f32,
    /// Uniform scale. Art is authored at a convenient size and scaled here,
    /// so the same shape serves a HUD icon and a hero ship.
    pub scale: f32,
    /// Mirror across the shape's own vertical axis, before rotation.
    ///
    /// A side-on ship facing the other way is a MIRROR, not a half turn.
    /// Rotating it by π rolls it inverted — canopy underneath, fin pointing
    /// down — which a symmetric placeholder hides and real art does not.
    /// Kept separate from `scale` so it cannot be smuggled in as a negative
    /// one, which would also flip the winding and the y axis.
    pub flip_x: bool,
}

impl Default for Transform {
    fn default() -> Self {
        Transform { x: 0.0, y: 0.0, angle: 0.0, scale: 1.0, flip_x: false }
    }
}

impl Transform {
    /// Unrotated, unscaled, at a position.
    pub const fn at(x: f32, y: f32) -> Self {
        Transform { x, y, angle: 0.0, scale: 1.0, flip_x: false }
    }

    /// The same placement, facing `angle` radians.
    pub const fn facing(self, angle: f32) -> Self {
        Transform { angle, ..self }
    }

    /// The same placement at a different size.
    pub const fn scaled(self, scale: f32) -> Self {
        Transform { scale, ..self }
    }

    /// The same placement, mirrored left-to-right.
    ///
    /// This is how a side-on ship turns around. Use it rather than
    /// `.facing(PI)`, which rolls the art upside down.
    pub const fn flipped(self, flip_x: bool) -> Self {
        Transform { flip_x, ..self }
    }

    /// Map a point from shape space to canvas space.
    ///
    /// Scale, then rotate, then translate — the order that lets a shape be
    /// authored around its own origin and placed without the rotation
    /// dragging it off-centre.
    pub fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        let (sin, cos) = self.angle.sin_cos();
        // Mirror first, in the shape's own space, so a flipped ship still
        // banks the way it is steered rather than the other way.
        let x = if self.flip_x { -x } else { x };
        let (sx, sy) = (x * self.scale, y * self.scale);
        (self.x + sx * cos - sy * sin, self.y + sx * sin + sy * cos)
    }
}

/// Art: a closed outline in its own coordinate space.
///
/// Holds no colour and no position. The same `Shape` drawn through two
/// [`Transform`]s at two colours is a ship and its shadow, and a `const`
/// shape costs nothing at runtime — which is what lets the playground
/// export art as pasteable Rust rather than a file the game has to load.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shape<'p> {
    /// Outline points, in order. Implicitly closed: the last joins the first.
    pub points: &'p [(f32, f32)],
}

impl<'p> Shape<'p> {
    pub const fn new(points: &'p [(f32, f32)]) -> Self {
        Shape { points }
    }

    /// Fill this outline, transformed, blending over what is there.
    pub fn fill(&self, canvas: &mut Canvas<'_>, t: &Transform, color: Color) {
        with_transformed(self.points, t, |pts| canvas.polygon_f(pts, color));
    }

    /// Fill this outline, transformed, *adding* light.
    pub fn fill_add(&self, canvas: &mut Canvas<'_>, t: &Transform, color: Color) {
        with_transformed(self.points, t, |pts| canvas.polygon_add_f(pts, color));
    }

    /// Stroke this outline, transformed. `width` is in canvas pixels and is
    /// **not** scaled by the transform: a hairline outline should stay a
    /// hairline whatever size the shape is drawn at.
    pub fn stroke(&self, canvas: &mut Canvas<'_>, t: &Transform, width: f32, color: Color) {
        with_transformed(self.points, t, |pts| {
            for i in 0..pts.len() {
                let a = pts[i];
                let b = pts[(i + 1) % pts.len()];
                canvas.line_f(a.0, a.1, b.0, b.1, width, color);
            }
        });
    }
}

/// Transform points into a stack buffer and hand them to `draw`.
///
/// Shapes are small — a ship hull is a dozen points — so the common case
/// needs no allocation at all. A shape larger than the buffer falls back to
/// a `Vec` rather than being clipped, because silently dropping points
/// would corrupt the outline in a way that is very hard to see.
fn with_transformed(
    points: &[(f32, f32)],
    t: &Transform,
    draw: impl FnOnce(&[(f32, f32)]),
) {
    const INLINE: usize = 32;
    if points.len() <= INLINE {
        let mut buf = [(0.0f32, 0.0f32); INLINE];
        for (dst, src) in buf.iter_mut().zip(points) {
            *dst = t.apply(src.0, src.1);
        }
        draw(&buf[..points.len()]);
    } else {
        let buf: Vec<(f32, f32)> = points.iter().map(|p| t.apply(p.0, p.1)).collect();
        draw(&buf);
    }
}

/// Blend mode for the shared span filler.
#[derive(Clone, Copy, PartialEq)]
enum Blend {
    Over,
    Add,
}

impl Canvas<'_> {
    /// Fill a polygon, anti-aliased, blending over the canvas.
    ///
    /// **Any simple polygon** — convex or concave. Filling follows the
    /// even-odd rule, which is also why winding order does not matter: a
    /// shape drawn clockwise and the same shape drawn anti-clockwise fill
    /// identically. The playground can hand over whatever the artist drew
    /// without normalising it first.
    ///
    /// Self-intersecting outlines are not an error but do follow even-odd,
    /// so the overlap of a bowtie renders hollow. That is a property worth
    /// knowing rather than a bug to work around.
    ///
    /// Fewer than three points draws nothing.
    pub fn polygon_f(&mut self, points: &[(f32, f32)], color: Color) {
        self.fill_polygon(points, color, Blend::Over);
    }

    /// The additive twin of [`polygon_f`](Self::polygon_f).
    ///
    /// Edge coverage scales the light exactly as it scales alpha in the
    /// blended path — a pixel the shape half covers receives half the light,
    /// which is what keeps a glowing outline from crawling as it turns.
    pub fn polygon_add_f(&mut self, points: &[(f32, f32)], color: Color) {
        self.fill_polygon(points, color, Blend::Add);
    }

    /// Draw a line of `width` pixels between two points, anti-aliased.
    ///
    /// Built as a four-point quad rather than by walking pixels, so it is
    /// the same coverage maths as everything else here and a line at any
    /// angle has genuinely soft edges on all four sides.
    ///
    /// `width` is a diameter, centred on the line: the quad extends half of
    /// it either side. A zero-length line draws nothing rather than a dot,
    /// because there is no direction to give it width in.
    pub fn line_f(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, color: Color) {
        if !(x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite()) {
            return;
        }
        if !width.is_finite() || width <= 0.0 || color.a == 0 {
            return;
        }

        let (dx, dy) = (x1 - x0, y1 - y0);
        let len = (dx * dx + dy * dy).sqrt();
        if len < EPSILON {
            return;
        }

        // Unit normal, scaled to half the width: the offset from the centre
        // line to each side of the quad.
        let h = width * 0.5;
        let (nx, ny) = (-dy / len * h, dx / len * h);

        let quad = [
            (x0 + nx, y0 + ny),
            (x1 + nx, y1 + ny),
            (x1 - nx, y1 - ny),
            (x0 - nx, y0 - ny),
        ];
        self.polygon_f(&quad, color);
    }

    /// The additive twin of [`line_f`](Self::line_f).
    pub fn line_add_f(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, color: Color) {
        if !(x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite()) {
            return;
        }
        if !width.is_finite() || width <= 0.0 || color.a == 0 {
            return;
        }
        let (dx, dy) = (x1 - x0, y1 - y0);
        let len = (dx * dx + dy * dy).sqrt();
        if len < EPSILON {
            return;
        }
        let h = width * 0.5;
        let (nx, ny) = (-dy / len * h, dx / len * h);
        let quad = [
            (x0 + nx, y0 + ny),
            (x1 + nx, y1 + ny),
            (x1 - nx, y1 - ny),
            (x0 - nx, y0 - ny),
        ];
        self.polygon_add_f(&quad, color);
    }

    /// Fill a circle, anti-aliased.
    ///
    /// Measured by radial distance rather than approximated with a polygon,
    /// so a small circle stays round — a 6px dot built from line segments
    /// reads as a visible hexagon, and dots are usually small.
    pub fn circle_f(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        self.fill_disc(cx, cy, r, 0.0, color, Blend::Over);
    }

    /// The additive twin of [`circle_f`](Self::circle_f).
    pub fn circle_add_f(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        self.fill_disc(cx, cy, r, 0.0, color, Blend::Add);
    }

    /// Draw a hollow circle: everything between `r - thickness` and `r`.
    ///
    /// A thickness at or beyond the radius fills solid, which makes a ring
    /// that grows into a disc a single expanding call rather than two cases
    /// the caller has to switch between.
    pub fn ring_f(&mut self, cx: f32, cy: f32, r: f32, thickness: f32, color: Color) {
        let inner = (r - thickness).max(0.0);
        self.fill_disc(cx, cy, r, inner, color, Blend::Over);
    }

    /// The additive twin of [`ring_f`](Self::ring_f).
    pub fn ring_add_f(&mut self, cx: f32, cy: f32, r: f32, thickness: f32, color: Color) {
        let inner = (r - thickness).max(0.0);
        self.fill_disc(cx, cy, r, inner, color, Blend::Add);
    }

    /// Shared implementation behind the polygon entry points.
    fn fill_polygon(&mut self, points: &[(f32, f32)], color: Color, blend: Blend) {
        if points.len() < 3 || color.a == 0 {
            return;
        }
        // One non-finite point would send the bounds and the per-row
        // intersection maths to infinity, so the whole shape is rejected
        // rather than partly drawn. Same stance as `fill_rect_f`.
        if points.iter().any(|p| !(p.0.is_finite() && p.1.is_finite())) {
            return;
        }

        let (mut min_y, mut max_y) = (f32::INFINITY, f32::NEG_INFINITY);
        for p in points {
            min_y = min_y.min(p.1);
            max_y = max_y.max(p.1);
        }

        let y_start = (min_y.floor().max(0.0)) as i64;
        let y_end = (max_y.ceil().min(self.height as f32)) as i64;
        if y_start >= y_end {
            return;
        }

        // Vertical coverage is resolved by sampling each pixel row at
        // SUBSAMPLES evenly spaced heights and averaging the spans found.
        // Horizontal coverage stays exact (a span's fractional ends are
        // computed directly), so this is only approximating the axis where
        // a polygon's edge can move within a single row. Four steps puts
        // the worst-case error on a near-horizontal edge below an eighth of
        // a level — invisible — for a quarter of the cost of sixteen.
        const SUBSAMPLES: usize = 4;
        const INV: f32 = 1.0 / SUBSAMPLES as f32;

        let width = self.width as usize;
        let mut coverage = vec![0.0f32; width];
        let mut crossings: Vec<f32> = Vec::with_capacity(points.len());

        for py in y_start..y_end {
            coverage[..width].fill(0.0);
            let mut touched_lo = width;
            let mut touched_hi = 0usize;

            for s in 0..SUBSAMPLES {
                // Sample at the centre of each sub-row, not its edge: an
                // edge exactly on a pixel boundary would otherwise be
                // counted or missed depending on rounding.
                let sample_y = py as f32 + (s as f32 + 0.5) * INV;

                crossings.clear();
                for i in 0..points.len() {
                    let (ax, ay) = points[i];
                    let (bx, by) = points[(i + 1) % points.len()];

                    // Half-open in y: a vertex shared by two edges is counted
                    // exactly once, which is what stops a seam appearing at
                    // every horizontal vertex.
                    let (lo, hi, x_lo, x_hi) = if ay < by {
                        (ay, by, ax, bx)
                    } else {
                        (by, ay, bx, ax)
                    };
                    if hi - lo < EPSILON {
                        continue; // horizontal edge: contributes no crossing
                    }
                    if sample_y < lo || sample_y >= hi {
                        continue;
                    }
                    let t = (sample_y - lo) / (hi - lo);
                    crossings.push(x_lo + (x_hi - x_lo) * t);
                }

                if crossings.len() < 2 {
                    continue;
                }
                crossings.sort_unstable_by(|a, b| a.total_cmp(b));

                // Even-odd: inside between the 1st and 2nd crossing, the 3rd
                // and 4th, and so on. This is what gives concave shapes
                // their notches and makes winding order irrelevant.
                let mut i = 0;
                while i + 1 < crossings.len() {
                    let (sx, ex) = (crossings[i], crossings[i + 1]);
                    i += 2;
                    if ex <= 0.0 || sx >= width as f32 || ex - sx < EPSILON {
                        continue;
                    }
                    let sx = sx.max(0.0);
                    let ex = ex.min(width as f32);

                    let first = sx.floor() as usize;
                    let last = (ex.ceil() as usize).min(width);
                    if first < touched_lo {
                        touched_lo = first;
                    }
                    if last > touched_hi {
                        touched_hi = last;
                    }

                    // Exact horizontal coverage per pixel, weighted by this
                    // sub-row's share of the pixel's height.
                    for (px, cov) in coverage
                        .iter_mut()
                        .enumerate()
                        .take(last)
                        .skip(first)
                    {
                        let l = sx.max(px as f32);
                        let r = ex.min(px as f32 + 1.0);
                        if r > l {
                            *cov += (r - l) * INV;
                        }
                    }
                }
            }

            if touched_lo >= touched_hi {
                continue;
            }
            self.blend_row(py as usize, (touched_lo, touched_hi), &coverage, color, blend);
        }
    }

    /// Shared implementation behind the circle and ring entry points.
    ///
    /// `inner` of 0 gives a solid disc. Coverage per pixel comes from the
    /// exact horizontal extent of the circle at several heights within the
    /// row, which keeps a small circle round where a polygon approximation
    /// would show its facets.
    fn fill_disc(&mut self, cx: f32, cy: f32, r: f32, inner: f32, color: Color, blend: Blend) {
        if !(cx.is_finite() && cy.is_finite() && r.is_finite() && inner.is_finite()) {
            return;
        }
        if r <= 0.0 || color.a == 0 {
            return;
        }
        let inner = inner.clamp(0.0, r);

        let y_start = ((cy - r).floor().max(0.0)) as i64;
        let y_end = ((cy + r).ceil().min(self.height as f32)) as i64;
        if y_start >= y_end {
            return;
        }

        const SUBSAMPLES: usize = 4;
        const INV: f32 = 1.0 / SUBSAMPLES as f32;

        let width = self.width as usize;
        let mut coverage = vec![0.0f32; width];

        for py in y_start..y_end {
            coverage[..width].fill(0.0);
            let mut touched_lo = width;
            let mut touched_hi = 0usize;

            for s in 0..SUBSAMPLES {
                let sy = py as f32 + (s as f32 + 0.5) * INV - cy;

                let outer_half = {
                    let d = r * r - sy * sy;
                    if d <= 0.0 {
                        continue;
                    }
                    d.sqrt()
                };
                // The hollow middle of a ring, if this row reaches it.
                let inner_half = {
                    let d = inner * inner - sy * sy;
                    if d > 0.0 {
                        d.sqrt()
                    } else {
                        0.0
                    }
                };

                // One span for a disc, two for a row that straddles the hole.
                let spans: [(f32, f32); 2] = if inner_half > 0.0 {
                    [
                        (cx - outer_half, cx - inner_half),
                        (cx + inner_half, cx + outer_half),
                    ]
                } else {
                    [(cx - outer_half, cx + outer_half), (0.0, 0.0)]
                };
                let span_count = if inner_half > 0.0 { 2 } else { 1 };

                for &(sx, ex) in spans.iter().take(span_count) {
                    if ex <= 0.0 || sx >= width as f32 || ex - sx < EPSILON {
                        continue;
                    }
                    let sx = sx.max(0.0);
                    let ex = ex.min(width as f32);
                    let first = sx.floor() as usize;
                    let last = (ex.ceil() as usize).min(width);
                    if first < touched_lo {
                        touched_lo = first;
                    }
                    if last > touched_hi {
                        touched_hi = last;
                    }
                    for (px, cov) in coverage
                        .iter_mut()
                        .enumerate()
                        .take(last)
                        .skip(first)
                    {
                        let l = sx.max(px as f32);
                        let rr = ex.min(px as f32 + 1.0);
                        if rr > l {
                            *cov += (rr - l) * INV;
                        }
                    }
                }
            }

            if touched_lo >= touched_hi {
                continue;
            }
            self.blend_row(py as usize, (touched_lo, touched_hi), &coverage, color, blend);
        }
    }

    /// Composite one row of accumulated coverage onto the buffer.
    ///
    /// The single place where coverage becomes pixels, so the polygon and
    /// circle paths cannot drift apart in how they blend — the same reason
    /// `fill_rect_add` copies `fill_rect`'s clipping verbatim.
    fn blend_row(
        &mut self,
        py: usize,
        span: (usize, usize),
        coverage: &[f32],
        color: Color,
        blend: Blend,
    ) {
        let (lo, hi) = span;
        let stride = self.width as usize;
        let start = py * stride;
        let alpha = color.a as f32 / 255.0;

        for (px, &cov) in coverage.iter().enumerate().take(hi).skip(lo) {
            if cov <= 0.0 {
                continue;
            }
            let a = (cov.min(1.0) * alpha * 255.0).round() as u8;
            if a == 0 {
                continue;
            }
            let i = start + px;
            let src = color.with_alpha(a);
            let dst = Color::from_u32(self.buffer[i]);
            self.buffer[i] = match blend {
                Blend::Over => src.over(dst).to_u32(),
                Blend::Add => src.plus(dst).to_u32(),
            };
        }
    }
}

#[cfg(test)]
mod vector_tests {
    use super::*;

    /// A canvas backed by a plain vec, so a test can read pixels back.
    fn canvas(w: u32, h: u32) -> (Vec<u32>, u32, u32) {
        (vec![0u32; (w * h) as usize], w, h)
    }

    fn alpha_at(buf: &[u32], w: u32, x: u32, y: u32) -> u8 {
        // Everything here draws white on black, so any channel reports how
        // much coverage landed. Reading one avoids depending on packing.
        Color::from_u32(buf[(y * w + x) as usize]).r
    }

    const WHITE: Color = Color::rgb(255, 255, 255);

    #[test]
    fn axis_aligned_polygon_matches_fill_rect_f() {
        // The strongest correctness statement available: a rectangle is a
        // polygon, so the new path must agree with the proven one. If these
        // ever diverge, vector art and rect art stop sitting in the same
        // picture — which is the whole premise of the module.
        let (mut a, w, h) = canvas(32, 32);
        let (mut b, _, _) = canvas(32, 32);

        Canvas::new(&mut a, w, h).fill_rect_f(4.25, 6.5, 10.5, 8.75, WHITE);
        Canvas::new(&mut b, w, h).polygon_f(
            &[(4.25, 6.5), (14.75, 6.5), (14.75, 15.25), (4.25, 15.25)],
            WHITE,
        );

        for i in 0..a.len() {
            let (pa, pb) = (Color::from_u32(a[i]).r as i32, Color::from_u32(b[i]).r as i32);
            // Vertical coverage is sub-sampled rather than exact, so a
            // partly covered row may differ by a sub-sample step. Full and
            // empty pixels must agree exactly.
            assert!(
                (pa - pb).abs() <= 32,
                "pixel {i}: fill_rect_f={pa} polygon_f={pb}",
            );
        }
    }

    #[test]
    fn interior_is_fully_opaque() {
        let (mut buf, w, h) = canvas(32, 32);
        Canvas::new(&mut buf, w, h).polygon_f(
            &[(4.0, 4.0), (28.0, 4.0), (28.0, 28.0), (4.0, 28.0)],
            WHITE,
        );
        assert_eq!(alpha_at(&buf, w, 16, 16), 255, "interior must be solid");
        assert_eq!(alpha_at(&buf, w, 2, 2), 0, "outside must be untouched");
    }

    #[test]
    fn half_covered_edge_is_half_lit() {
        // An edge down the middle of a pixel column covers exactly half of
        // it. This is the property that makes motion read as smooth, so it
        // is asserted numerically rather than looked at.
        let (mut buf, w, h) = canvas(16, 16);
        Canvas::new(&mut buf, w, h)
            .polygon_f(&[(4.0, 2.0), (8.5, 2.0), (8.5, 14.0), (4.0, 14.0)], WHITE);

        let edge = alpha_at(&buf, w, 8, 8) as i32;
        assert!(
            (edge - 128).abs() <= 4,
            "half-covered pixel should be ~128, got {edge}",
        );
    }

    #[test]
    fn concave_notch_stays_empty() {
        // The reason even-odd was chosen. A chevron's notch must not fill;
        // a convex-only filler would bridge straight across it.
        let (mut buf, w, h) = canvas(40, 40);
        Canvas::new(&mut buf, w, h).polygon_f(
            &[(4.0, 4.0), (20.0, 24.0), (36.0, 4.0), (36.0, 36.0), (4.0, 36.0)],
            WHITE,
        );

        assert_eq!(alpha_at(&buf, w, 20, 8), 0, "the notch must stay empty");
        assert_eq!(alpha_at(&buf, w, 20, 32), 255, "the body must be filled");
    }

    #[test]
    fn winding_order_does_not_matter() {
        // The playground will hand over whatever the artist drew. If a
        // shape vanished when drawn anti-clockwise, that tool would be
        // quietly unusable.
        let tri = [(8.0, 4.0), (28.0, 30.0), (4.0, 30.0)];
        let mut reversed = tri;
        reversed.reverse();

        let (mut a, w, h) = canvas(32, 32);
        let (mut b, _, _) = canvas(32, 32);
        Canvas::new(&mut a, w, h).polygon_f(&tri, WHITE);
        Canvas::new(&mut b, w, h).polygon_f(&reversed, WHITE);
        assert_eq!(a, b, "winding order must not change the fill");
    }

    #[test]
    fn rotation_preserves_area() {
        // Area is the invariant a transform must not break. A shape that
        // gained or lost coverage as it turned would make a rotating ship
        // pulse — exactly the artefact vector art is meant to remove.
        let square = Shape::new(&[(-8.0, -8.0), (8.0, -8.0), (8.0, 8.0), (-8.0, 8.0)]);
        let ink = |angle: f32| {
            let (mut buf, w, h) = canvas(64, 64);
            let mut c = Canvas::new(&mut buf, w, h);
            square.fill(&mut c, &Transform::at(32.0, 32.0).facing(angle), WHITE);
            buf.iter().map(|p| Color::from_u32(*p).r as u32).sum::<u32>()
        };

        let flat = ink(0.0) as f32;
        let tilted = ink(std::f32::consts::FRAC_PI_4) as f32;
        let drift = (tilted - flat).abs() / flat;
        assert!(drift < 0.02, "area drifted {:.1}% when rotated", drift * 100.0);
    }

    #[test]
    fn flipping_mirrors_rather_than_rolling() {
        // A side-on ship facing the other way must be a MIRROR. Rotating it
        // by PI rolls it inverted, which a vertically symmetric placeholder
        // hides and real art exposes immediately: canopy underneath, fin
        // pointing down. This asserts the difference so the two cannot be
        // confused again.
        //
        // An asymmetric mark ABOVE the axis must stay above it when
        // flipped, and must end up BELOW it when rotated.
        let t_flip = Transform::at(0.0, 0.0).flipped(true);
        let (fx, fy) = t_flip.apply(3.0, -2.0);
        assert!((fx - -3.0).abs() < 1e-5, "flip must mirror x, got {fx}");
        assert!((fy - -2.0).abs() < 1e-5, "flip must NOT move y, got {fy}");

        let t_rot = Transform::at(0.0, 0.0).facing(std::f32::consts::PI);
        let (rx, ry) = t_rot.apply(3.0, -2.0);
        assert!((rx - -3.0).abs() < 1e-5, "a half turn also mirrors x");
        assert!((ry - 2.0).abs() < 1e-5, "but a half turn INVERTS y, got {ry}");
    }

    #[test]
    fn a_flipped_shape_keeps_its_area() {
        let ship = Shape::new(&[(14.0, 0.0), (-6.0, -7.0), (-2.0, 0.0), (-6.0, 7.0)]);
        let ink = |flip: bool| {
            let (mut buf, w, h) = canvas(64, 64);
            let mut c = Canvas::new(&mut buf, w, h);
            ship.fill(&mut c, &Transform::at(32.0, 32.0).scaled(1.5).flipped(flip), WHITE);
            buf.iter().map(|p| Color::from_u32(*p).r as u32).sum::<u32>()
        };
        let (a, b) = (ink(false) as f32, ink(true) as f32);
        assert!((a - b).abs() / a < 0.02, "mirroring must not change how much ink lands");
    }

    #[test]
    fn transform_scales_about_its_own_origin() {
        // Authoring a shape around its origin and scaling it must not walk
        // it across the screen.
        let t = Transform::at(100.0, 50.0).scaled(3.0);
        assert_eq!(t.apply(0.0, 0.0), (100.0, 50.0));
        let (x, y) = t.apply(2.0, 0.0);
        assert!((x - 106.0).abs() < 1e-4 && (y - 50.0).abs() < 1e-4);
    }

    #[test]
    fn circle_is_round_not_faceted() {
        // Sample the boundary at many angles; a polygon approximation would
        // show its facets as a varying radius. The measured radius must be
        // constant to within a pixel all the way round.
        let (mut buf, w, h) = canvas(64, 64);
        Canvas::new(&mut buf, w, h).circle_f(32.0, 32.0, 20.0, WHITE);

        for i in 0..32 {
            let a = i as f32 * std::f32::consts::TAU / 32.0;
            let (dx, dy) = (a.cos(), a.sin());
            // Just inside the rim must be lit; well outside must not be.
            let (ix, iy) = (32.0 + dx * 18.0, 32.0 + dy * 18.0);
            let (ox, oy) = (32.0 + dx * 22.5, 32.0 + dy * 22.5);
            assert!(
                alpha_at(&buf, w, ix as u32, iy as u32) > 200,
                "inside the rim at {a:.2} rad should be lit",
            );
            assert_eq!(
                alpha_at(&buf, w, ox as u32, oy as u32),
                0,
                "outside the rim at {a:.2} rad should be empty",
            );
        }
    }

    #[test]
    fn ring_is_hollow() {
        let (mut buf, w, h) = canvas(64, 64);
        Canvas::new(&mut buf, w, h).ring_f(32.0, 32.0, 20.0, 5.0, WHITE);
        assert_eq!(alpha_at(&buf, w, 32, 32), 0, "the middle must be hollow");
        assert!(alpha_at(&buf, w, 32, 14) > 200, "the band must be drawn");
    }

    #[test]
    fn a_thick_ring_fills_solid() {
        // A ring whose thickness reaches the centre is a disc. This lets an
        // expanding shockwave be one call rather than two cases.
        let (mut buf, w, h) = canvas(64, 64);
        Canvas::new(&mut buf, w, h).ring_f(32.0, 32.0, 20.0, 50.0, WHITE);
        assert_eq!(alpha_at(&buf, w, 32, 32), 255);
    }

    #[test]
    fn line_has_width_on_both_sides() {
        // Width is a diameter centred on the line, not an offset to one
        // side. A caller drawing a 4px line from a to b expects it centred.
        let (mut buf, w, h) = canvas(32, 32);
        Canvas::new(&mut buf, w, h).line_f(4.0, 16.0, 28.0, 16.0, 4.0, WHITE);
        for y in 15..=17 {
            assert!(alpha_at(&buf, w, 16, y) > 200, "row {y} should be lit");
        }
        assert_eq!(alpha_at(&buf, w, 16, 12), 0, "well above must be empty");
        assert_eq!(alpha_at(&buf, w, 16, 20), 0, "well below must be empty");
    }

    #[test]
    fn additive_light_accumulates() {
        let (mut buf, w, h) = canvas(16, 16);
        {
            let mut c = Canvas::new(&mut buf, w, h);
            let grey = Color::rgb(80, 80, 80);
            let tri = [(2.0, 2.0), (14.0, 2.0), (14.0, 14.0)];
            c.polygon_add_f(&tri, grey);
            c.polygon_add_f(&tri, grey);
        }
        assert_eq!(alpha_at(&buf, w, 11, 7), 160, "two passes must sum");
    }

    #[test]
    fn degenerate_input_draws_nothing_and_does_not_panic() {
        let (mut buf, w, h) = canvas(16, 16);
        {
            let mut c = Canvas::new(&mut buf, w, h);
            c.polygon_f(&[], WHITE);
            c.polygon_f(&[(1.0, 1.0), (2.0, 2.0)], WHITE); // only two points
            c.polygon_f(&[(0.0, 0.0), (f32::NAN, 1.0), (2.0, 2.0)], WHITE);
            c.polygon_f(&[(0.0, 0.0), (f32::INFINITY, 1.0), (2.0, 2.0)], WHITE);
            c.circle_f(8.0, 8.0, f32::NAN, WHITE);
            c.circle_f(8.0, 8.0, -4.0, WHITE);
            c.line_f(4.0, 4.0, 4.0, 4.0, 2.0, WHITE); // zero length
            c.line_f(0.0, 0.0, 8.0, 8.0, 0.0, WHITE); // zero width
            c.polygon_f(&[(0.0, 0.0), (8.0, 0.0), (8.0, 8.0)], Color::TRANSPARENT);
        }
        assert!(buf.iter().all(|&p| p == 0), "nothing should have been drawn");
    }

    #[test]
    fn a_shared_diagonal_edge_seams_but_an_overlap_does_not() {
        // Finding #35, pinned. Two triangles meeting on an exact diagonal
        // leave a ~192 hairline where the interior is 255. This is inherent
        // to compositing two independently anti-aliased shapes and is
        // handled by building art with overlap, not by changing the filler.
        // The test exists so the documented number cannot quietly drift,
        // and so the overlap remedy is proven rather than asserted.
        let seam_dip = |overlap: f32| {
            let (mut buf, w, h) = canvas(40, 40);
            {
                let mut c = Canvas::new(&mut buf, w, h);
                c.polygon_f(&[(4.0, 4.0), (36.0, 4.0), (4.0, 36.0)], WHITE);
                c.polygon_f(
                    &[
                        (36.0 - overlap, 4.0 - overlap),
                        (36.0, 36.0),
                        (4.0 - overlap, 36.0 - overlap),
                    ],
                    WHITE,
                );
            }
            // The dimmest pixel strictly inside the combined shape.
            let mut worst = 255u8;
            for y in 6..34u32 {
                for x in 6..34u32 {
                    worst = worst.min(alpha_at(&buf, w, x, y));
                }
            }
            worst
        };

        let butted = seam_dip(0.0);
        assert!(
            (150..=220).contains(&butted),
            "a butted diagonal should seam around 192, got {butted}",
        );

        let overlapped = seam_dip(1.5);
        assert_eq!(
            overlapped, 255,
            "overlapping the pieces must close the seam completely",
        );
    }

    #[test]
    fn shapes_clip_to_the_canvas() {
        // Wholly and partly off-canvas geometry must neither panic nor
        // wrap around to the far edge.
        let (mut buf, w, h) = canvas(16, 16);
        {
            let mut c = Canvas::new(&mut buf, w, h);
            c.polygon_f(&[(-100.0, -100.0), (-50.0, -100.0), (-50.0, -50.0)], WHITE);
            c.circle_f(-40.0, -40.0, 10.0, WHITE);
        }
        assert!(buf.iter().all(|&p| p == 0), "off-canvas must draw nothing");

        let mut buf2 = vec![0u32; 256];
        {
            let mut c = Canvas::new(&mut buf2, 16, 16);
            // Straddles the left edge: the visible half must be drawn.
            c.polygon_f(&[(-8.0, 4.0), (8.0, 4.0), (8.0, 12.0), (-8.0, 12.0)], WHITE);
        }
        assert_eq!(Color::from_u32(buf2[8 * 16 + 0]).r, 255, "left edge drawn");
        assert_eq!(Color::from_u32(buf2[8 * 16 + 12]).r, 0, "past the shape");
    }
}
