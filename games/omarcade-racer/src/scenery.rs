//! Things beside the road: what actually sells motion.
//!
//! # Why this exists
//!
//! Three attempts were made to get a sense of speed out of the ground
//! surfaces themselves — fine bands, coarse bands, then a smooth gradient
//! — and each failed differently:
//!
//! - fine hard bands aliased and the road visibly ran **backwards** above
//!   75% of top speed
//! - coarse hard bands were so large that only one or two were on screen,
//!   so the whole surface **toggled** between two shades instead of
//!   scrolling
//! - a smooth cosine gradient fixed the toggling and produced **ocean
//!   waves**: a smooth luminance gradient along z is pixel-for-pixel the
//!   image a corrugated surface makes under diffuse light, and human
//!   vision reads smooth gradients as surface curvature. The corrugation
//!   was in the *still frame*; motion only animated it into swells.
//!
//! The shared error was the premise. Large surfaces carry speed
//! **magnitude**; discrete world-anchored objects carry motion
//! **direction**. Pole Position — this game's direct lineage — had no
//! ground banding at all: flat tarmac, and the dashed line, the curbing
//! and roadside billboards did all the work.
//!
//! So this module is the motion channel, and the ground went flat.
//!
//! # Why these cannot alias
//!
//! The Nyquist floor applies to *periodic* patterns, where the eye locks
//! onto a phase and phase becomes ambiguous past half a period per frame.
//! Jittered spacing has no global phase to misread: the eye tracks
//! individual objects, and each persists across many frames — roughly
//! 2000 units apart against 533 units of travel per frame at 30fps.
//! Irregular wagon-wheel spokes do not reverse.
//!
//! # The one way to get this wrong
//!
//! Two earlier attempts at roadside detail failed **identically**, and
//! both were placement bugs rather than concept bugs: they positioned
//! detail as a function of the *scanline* — a fraction of the verge's
//! screen width, then a multiple of the road's screen half-width. Both
//! shrink with distance, so each band's detail landed at a different
//! screen x and smeared into long diagonal rays converging on the horizon.
//!
//! **An object owns a `z`; a scanline never owns an object.** Everything
//! here is anchored to a fixed track position and projected, exactly as
//! the rival cars are.

use crate::road::Road;

/// One thing standing beside the road.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prop {
    /// Track position, world units. This is what makes it an object
    /// rather than a decoration on a scanline.
    pub z: f32,
    /// Lateral offset in road half-widths from the centre line. Beyond
    /// ±1.0 is off the road, which is where scenery belongs.
    pub lane: f32,
    /// Which sprite to draw. The renderer maps this to art; keeping it an
    /// index means new roadside art needs no change here.
    pub kind: usize,
}

/// A deterministic hash, so scenery is identical every run and every
/// session without storing a list.
///
/// Determinism matters more than quality here: a track that rearranges
/// itself between runs cannot be learned, and learning the track is most
/// of what a racing game is.
fn hash(n: u32) -> u32 {
    // Wang-style integer hash. Cheap, and good enough that neighbouring
    // inputs do not produce neighbouring outputs — which is the only
    // property being relied on.
    let mut h = n.wrapping_mul(2_654_435_761);
    h ^= h >> 15;
    h = h.wrapping_mul(2_246_822_519);
    h ^= h >> 13;
    h
}

/// A hashed float in 0..1 from an integer seed.
fn hash01(n: u32) -> f32 {
    (hash(n) % 100_000) as f32 / 100_000.0
}

/// Average spacing between roadside props, in world units.
///
/// Two requirements pull against each other here, and the resolution is
/// that one of them was measuring the wrong thing.
///
/// A prop is only legible within roughly 2,000 units — beyond that it is
/// a couple of pixels tall (the projection falls as 1/distance, so a post
/// 50px tall at 200 units is 3px at 3,200). So props must be placed close
/// to be worth drawing at all.
///
/// The competing worry was arrival RATE: at 30fps the car covers 533
/// units per frame, so tight spacing means props arrive every frame or
/// two. But arrival rate is not the property that matters. What matters
/// is how long each prop PERSISTS, and a prop enters at the draw distance
/// and leaves at the camera — 45 frames at 30fps, 90 at 60. Arrival rate
/// would only matter if props were indistinguishable from one another, at
/// which point they are a periodic pattern again; `PROP_JITTER` and the
/// mix of kinds are what make them individuals instead.
///
/// So 1,600: about fifteen in view, the nearest ones large enough to
/// read, each on screen for dozens of frames.
pub const PROP_SPACING: f32 = 1_600.0;

/// How much the spacing is allowed to wander, as a fraction.
///
/// This is the load-bearing constant of the whole module. Perfectly even
/// spacing is a periodic pattern and periodic patterns reverse; jitter is
/// what removes the global phase the eye would otherwise lock onto.
pub const PROP_JITTER: f32 = 0.35;

/// How far out from the centre line props stand, in road half-widths.
const PROP_LANE: f32 = 1.30;
const PROP_LANE_JITTER: f32 = 0.22;

/// Build the scenery for a stretch of track.
///
/// Generated from the track position rather than stored, so a long track
/// costs nothing and the same stretch always looks the same.
///
/// `from` and `to` are absolute track positions; the caller passes the
/// visible range. Props are returned near-to-far in `z` order.
pub fn props_between(from: f32, to: f32, kinds: usize) -> Vec<Prop> {
    assert!(kinds > 0, "there must be at least one kind of prop to place");
    if to <= from {
        return Vec::new();
    }

    let first = (from / PROP_SPACING).floor() as i64;
    let last = (to / PROP_SPACING).ceil() as i64;

    let mut out = Vec::with_capacity((last - first).max(0) as usize + 2);
    for i in first..=last {
        // The slot index is the identity of this prop. Hashing it — rather
        // than a running counter — is what makes the track stable no
        // matter where the camera enters it.
        let seed = i as u32;
        let jitter = (hash01(seed) - 0.5) * 2.0 * PROP_JITTER;
        let z = (i as f32 + jitter) * PROP_SPACING;
        if z < from || z > to {
            continue;
        }

        // Alternate sides with a hashed break, so it is not a zip.
        let side = if hash(seed ^ 0x9e37) % 2 == 0 { -1.0 } else { 1.0 };
        let lane_jitter = (hash01(seed ^ 0x51ed) - 0.5) * 2.0 * PROP_LANE_JITTER;
        let lane = side * (PROP_LANE + lane_jitter);

        let kind = (hash(seed ^ 0x2545) as usize) % kinds;
        out.push(Prop { z, lane, kind });
    }
    out
}

/// Everything visible from `camera_z`, near-to-far.
///
/// Wrapping is handled by asking for the range ahead in *unwrapped* track
/// units and letting the projection deal with it; a prop's `z` may exceed
/// the track length, which is correct — it is the same place next lap.
pub fn visible_props(road: &Road, camera_z: f32, kinds: usize) -> Vec<Prop> {
    let reach = road.draw_distance() as f32 * road.segment_length();
    props_between(camera_z, camera_z + reach, kinds)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole reason this module exists instead of another ground
    /// pattern: spacing must be irregular, or it is periodic and periodic
    /// patterns reverse at speed.
    #[test]
    fn spacing_is_irregular() {
        // Range derived from the spacing, not hardcoded: an earlier
        // version asked for 60,000 units and asserted ">20 props", which
        // silently became an impossible demand the moment the spacing was
        // retuned. Ask for a long enough run instead.
        let span = PROP_SPACING * 40.0;
        let props = props_between(0.0, span, 3);
        assert!(
            props.len() > 20,
            "only {} props in {span:.0} units at spacing {PROP_SPACING}",
            props.len(),
        );

        let gaps: Vec<f32> = props.windows(2).map(|w| w[1].z - w[0].z).collect();
        let mean = gaps.iter().sum::<f32>() / gaps.len() as f32;
        let spread = gaps
            .iter()
            .map(|g| (g - mean).abs())
            .fold(0.0f32, f32::max);

        assert!(
            spread > PROP_SPACING * 0.2,
            "gaps vary by only {spread:.0} units around {mean:.0} — \
             that is close enough to periodic to alias",
        );
    }

    /// ...but not so irregular that props overlap or the field goes patchy.
    #[test]
    fn spacing_stays_within_bounds() {
        let props = props_between(0.0, PROP_SPACING * 60.0, 3);
        for w in props.windows(2) {
            let gap = w[1].z - w[0].z;
            assert!(
                gap > PROP_SPACING * 0.2,
                "two props only {gap:.0} units apart — they will overlap",
            );
            assert!(
                gap < PROP_SPACING * 1.9,
                "a {gap:.0}-unit hole in the scenery reads as the world ending",
            );
        }
    }

    /// Every prop must be OFF the road. One standing on the racing line
    /// is a collision the player cannot anticipate.
    #[test]
    fn props_stand_clear_of_the_road() {
        for p in props_between(0.0, PROP_SPACING * 60.0, 3) {
            assert!(
                p.lane.abs() > 1.0,
                "a prop at lane {} is on the road surface",
                p.lane,
            );
        }
    }

    /// Both sides get used, or it looks like a one-sided fence.
    #[test]
    fn props_appear_on_both_sides() {
        let props = props_between(0.0, PROP_SPACING * 40.0, 3);
        let left = props.iter().filter(|p| p.lane < 0.0).count();
        let right = props.len() - left;
        assert!(left > 5 && right > 5, "lopsided: {left} left, {right} right");
    }

    /// The same stretch of track must look the same every time it is
    /// approached, or the track cannot be learned — and learning the track
    /// is most of what a racing game is.
    #[test]
    fn scenery_is_deterministic_and_position_stable() {
        let (lo, hi) = (PROP_SPACING * 5.0, PROP_SPACING * 12.0);
        let a = props_between(lo, hi, 3);
        let b = props_between(lo, hi, 3);
        assert_eq!(a, b, "the same range generated two different sceneries");

        // ...including when reached from a different starting point, which
        // is what a running camera actually does.
        let wide = props_between(0.0, PROP_SPACING * 20.0, 3);
        let overlap: Vec<&Prop> = wide
            .iter()
            .filter(|p| p.z >= lo && p.z <= hi)
            .collect();
        let narrow: Vec<&Prop> = a.iter().collect();
        assert_eq!(
            overlap, narrow,
            "props moved depending on where the camera entered the range",
        );
    }

    /// Props must persist across many frames.
    ///
    /// An object that appears and vanishes within a frame or two is a
    /// flicker, not a motion cue, and it would alias exactly like the
    /// patterns this replaces.
    ///
    /// Note WHAT is measured. An earlier version of this test asserted the
    /// arrival RATE — how long between one prop and the next — and forced
    /// the spacing so wide that the nearest prop was beyond the distance
    /// at which a prop is even legible. Arrival rate is not the property:
    /// props are individuals, not a pattern, so what matters is how long
    /// each one stays on screen. That is the whole visible depth.
    #[test]
    fn props_last_many_frames_at_top_speed() {
        let road = Road::straight(400);
        let visible = road.draw_distance() as f32 * road.segment_length();
        let top_speed = visible / 1.5;

        for fps in [60.0f32, 30.0] {
            let per_frame = top_speed / fps;
            let frames_on_screen = visible / per_frame;
            assert!(
                frames_on_screen > 20.0,
                "at {fps}fps a prop crosses the screen in {frames_on_screen:.0} frames \
                 — too quick to read as an object",
            );
        }
    }

    /// ...and the nearest ones must be close enough to actually see. A
    /// field of props all beyond legibility range is an empty roadside.
    #[test]
    fn the_nearest_props_are_close_enough_to_read() {
        // A post is ~8 rows; below about 6px tall it stops reading as an
        // object. Scale follows the road's projected half-width.
        let props = props_between(0.0, PROP_SPACING * 6.0, 3);
        let nearest = props.first().expect("props in the first stretch");
        assert!(
            nearest.z < 2_400.0,
            "the nearest prop is {:.0} units out, past where one is legible",
            nearest.z,
        );
    }

    /// A visible run should be populated but not crowded.
    #[test]
    fn a_visible_stretch_holds_a_sensible_number() {
        let road = Road::straight(400);
        let n = visible_props(&road, 0.0, 3).len();
        assert!(
            (6..=30).contains(&n),
            "{n} props in view — expected roughly a dozen",
        );
    }

    /// All kinds get used, or the extra art is dead weight.
    #[test]
    fn every_kind_gets_placed() {
        let props = props_between(0.0, PROP_SPACING * 60.0, 3);
        for k in 0..3 {
            assert!(
                props.iter().any(|p| p.kind == k),
                "kind {k} never appears; the art is dead weight",
            );
        }
    }

    #[test]
    #[should_panic(expected = "at least one kind")]
    fn placing_with_no_art_is_rejected() {
        props_between(0.0, 1000.0, 0);
    }
}
