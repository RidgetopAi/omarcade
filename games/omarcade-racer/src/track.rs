//! Authoring a course: sections a person can read, segments the road can drive.
//!
//! [`crate::road`] holds a track as a flat list of [`Segment`]s, each with
//! a raw `curve` number. That is the right thing for the projection to
//! consume and the wrong thing for a human to write. The demo track was
//! authored by hand:
//!
//! ```text
//! segs.extend(std::iter::repeat_n(Segment::STRAIGHT, 4));
//! for i in 0..6 { segs.push(Segment::curving(90.0 * (i as f32 / 6.0))); }
//! segs.extend(std::iter::repeat_n(Segment::curving(90.0), 40));
//! ```
//!
//! Three problems with writing a 2.7-mile course that way, and this module
//! is one fix for each.
//!
//! # 1. Lengths were segment counts
//!
//! `40` is a length only if you know what a segment is worth. Change
//! `segment_length` and every course silently changes shape — the same
//! failure as the top speed that was once a bare `90_000.0` and became
//! 5.6x too fast when the road was retuned (L019). Sections are stated in
//! **distance**, and the segment count is computed.
//!
//! # 2. Curve strength was a raw number
//!
//! `90.0` means nothing on its own. Whether it is a gentle sweep or an
//! unholdable hairpin depends on `draw_distance`, on the steer rate and on
//! the centrifugal push — and when `draw_distance` moved from 300 to 120,
//! what `90.0` meant moved with it. Bends are named against **the physics
//! limit**: [`Bend::MustBrake`] is defined as past the point where full
//! counter-steer holds, so it stays true when the physics is retuned. The
//! author says what a corner should *demand*, not what number to store.
//!
//! # 3. Easing was retyped per corner
//!
//! The hand-written track eases its bend in and out over six segments, and
//! the comment explains why: a curve that starts at full strength reads as
//! a kink rather than as a corner. That is knowledge about how corners
//! look, and it belongs in the thing that builds corners — not in the
//! memory of whoever writes the next one.

use crate::drive::BRAKE_BEND;
use crate::road::{Road, Segment};

/// One mile in world units.
///
/// The road is 2200 units wide. A real two-lane road is about 7.3m across,
/// so a world unit is roughly 3.3mm and a mile is about 485,000 units.
///
/// This exists so a course can be authored in a unit a person has an
/// intuition for — "a quarter-mile straight" is a picture, "121,000 units"
/// is not. It is a conversion, not a tuning constant: nothing about how
/// the game feels depends on it, only what the numbers in a course
/// definition look like.
pub const UNITS_PER_MILE: f32 = 485_000.0;

/// How long a section is.
///
/// Stated as a distance rather than as a segment count so a course
/// survives a change to `segment_length`. The two constructors exist
/// because both are natural to author in: a straight is naturally
/// "a third of a mile", a corner is naturally "a hundred and fifty
/// metres".
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Length {
    Miles(f32),
    Metres(f32),
}

impl Length {
    /// One metre, in world units. Derived from the mile so the two units
    /// cannot drift apart.
    const UNITS_PER_METRE: f32 = UNITS_PER_MILE / 1609.344;

    pub fn units(self) -> f32 {
        match self {
            Length::Miles(m) => m * UNITS_PER_MILE,
            Length::Metres(m) => m * Self::UNITS_PER_METRE,
        }
    }
}

/// Which way a corner goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
}

impl Dir {
    fn sign(self) -> f32 {
        match self {
            Dir::Left => -1.0,
            Dir::Right => 1.0,
        }
    }
}

/// How hard a corner is, stated against what the car can actually do.
///
/// Each variant resolves to a multiple of [`BRAKE_BEND`] — the bend at
/// which centrifugal push exactly cancels full counter-steer at full
/// speed. So the names are claims about *the driving*, and they stay true
/// when the physics is retuned, which a stored curve number would not.
///
/// The point of the set is that a course needs corners on both sides of
/// the limit. A track of nothing but `MustBrake` is one decision repeated;
/// a track of nothing but `Gentle` has no decisions in it at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bend {
    /// A sweep. Barely asks anything — the road bends, the driver does not
    /// have to respond.
    Gentle,
    /// Holdable flat out, but only just. The car runs wide and the driver
    /// feels it without having to lift.
    Firm,
    /// Past the limit: flat out puts you off. This is the corner the brake
    /// exists for.
    MustBrake,
    /// Well past the limit. Wants real slowing, not a dab.
    Hard,
}

impl Bend {
    /// The normalised curve this bend asks for.
    ///
    /// Multiples of the limit rather than absolutes:
    ///   0.55  comfortably holdable        0.95  holdable, but working
    ///   1.30  flat out goes off           1.80  wants real slowing
    pub fn curve(self) -> f32 {
        BRAKE_BEND
            * match self {
                Bend::Gentle => 0.55,
                Bend::Firm => 0.95,
                Bend::MustBrake => 1.30,
                Bend::Hard => 1.80,
            }
    }
}

/// One authored piece of course.
#[derive(Clone, Copy, Debug)]
enum Section {
    Straight { length: Length },
    Corner { bend: Bend, dir: Dir, length: Length },
}

/// A course, authored as sections and built into a [`Road`].
///
/// ```ignore
/// let road = Track::new()
///     .straight(Length::Miles(0.3))
///     .corner(Bend::Firm, Dir::Right, Length::Metres(180.0))
///     .corner(Bend::MustBrake, Dir::Left, Length::Metres(140.0))
///     .build();
/// ```
#[derive(Clone, Debug, Default)]
pub struct Track {
    sections: Vec<Section>,
    segment_length: f32,
    width: f32,
}

/// What fraction of a corner is spent easing in, and again easing out.
///
/// A curve that starts at full strength reads as a kink rather than as a
/// corner — the hand-written demo track eased over six of its fifty-two
/// segments and said so in a comment. Held here as a fraction of the
/// corner's own length rather than as a segment count, so a long sweeping
/// bend eases over a longer distance than a short one and both read the
/// same way (L019: an angle, not a distance).
///
/// Capped at 0.5 by construction below, since easing in and out cannot
/// together exceed the corner.
const EASE_FRACTION: f32 = 0.18;

impl Track {
    /// A course with the road dimensions the rest of the game derives
    /// from.
    ///
    /// These are the numbers `Road::straight` uses, and they are chosen
    /// relative to each other: a segment is about half a road-width long,
    /// which is what makes rumble banding stream past at a believable
    /// rate.
    pub fn new() -> Track {
        Track { sections: Vec::new(), segment_length: 200.0, width: 2200.0 }
    }

    /// Straight road.
    pub fn straight(mut self, length: Length) -> Track {
        self.sections.push(Section::Straight { length });
        self
    }

    /// A corner, eased in and out.
    pub fn corner(mut self, bend: Bend, dir: Dir, length: Length) -> Track {
        self.sections.push(Section::Corner { bend, dir, length });
        self
    }

    /// How long the course is, in world units.
    pub fn length_units(&self) -> f32 {
        self.sections
            .iter()
            .map(|s| match s {
                Section::Straight { length } => length.units(),
                Section::Corner { length, .. } => length.units(),
            })
            .sum()
    }

    /// How long the course is, in miles.
    pub fn length_miles(&self) -> f32 {
        self.length_units() / UNITS_PER_MILE
    }

    /// Build the road.
    ///
    /// # Panics
    /// On an empty course, and on a section shorter than one segment. The
    /// second is worth failing on rather than rounding away: a corner
    /// asked for in metres that quietly becomes zero segments is a corner
    /// that vanishes from the track while the course definition still says
    /// it is there — which would be debugged by staring at the driving.
    pub fn build(&self) -> Road {
        assert!(!self.sections.is_empty(), "a course needs at least one section");

        let mut segs: Vec<Segment> = Vec::new();
        for (i, section) in self.sections.iter().enumerate() {
            match *section {
                Section::Straight { length } => {
                    let n = self.segments_for(length, i);
                    segs.extend(std::iter::repeat_n(Segment::STRAIGHT, n));
                }
                Section::Corner { bend, dir, length } => {
                    let n = self.segments_for(length, i);
                    // Raw curve is what `Road` stores; the author's number
                    // is normalised. Converting here, in one place, is what
                    // keeps the two kinds from being mixed up at the call
                    // site — the bug that once threw the car across the
                    // road in 25 milliseconds.
                    let raw = self.raw_curve(bend.curve()) * dir.sign();
                    let ease = ((n as f32 * EASE_FRACTION) as usize).max(1).min(n / 2);
                    for j in 0..n {
                        // Ramp up over the first `ease` segments, down over
                        // the last, full strength between.
                        let t = if j < ease {
                            (j + 1) as f32 / ease as f32
                        } else if j >= n - ease {
                            (n - j) as f32 / ease as f32
                        } else {
                            1.0
                        };
                        segs.push(Segment::curving(raw * t));
                    }
                }
            }
        }

        Road::new(segs, self.segment_length, self.width)
    }

    /// How many segments a length comes to.
    fn segments_for(&self, length: Length, index: usize) -> usize {
        let n = (length.units() / self.segment_length).round() as usize;
        assert!(
            n > 0,
            "section {index} is {:.0} units, shorter than one {:.0}-unit segment — \
             it would vanish from the track while the course still claims it is there",
            length.units(),
            self.segment_length,
        );
        n
    }

    /// Convert a normalised curve into the raw number `Segment` stores.
    ///
    /// The inverse of [`Road::curve_at`], which divides by the triangular
    /// number of the draw distance. Done here so a course is authored in
    /// the units the PHYSICS reads, and the renderer's authoring number
    /// never appears in a track definition at all.
    ///
    /// ⚠️ This depends on `draw_distance`, which `Road::new` sets. Read
    /// from a probe road built with the same constructor, so the two
    /// cannot disagree.
    fn raw_curve(&self, normalised: f32) -> f32 {
        let probe = Road::new(vec![Segment::STRAIGHT], self.segment_length, self.width);
        let n = probe.draw_distance() as f32;
        normalised * (n * (n + 1.0) / 2.0) / n
    }
}

/// The score-file label for the grand prix. It rides in the score entry's
/// difficulty slot (decision 729d1f0e): Omaprix has tracks, not tiers, and
/// the marquee already keeps a best per label, so a second course shows up
/// beside this one with nothing else changed.
pub const GRAND_PRIX_ID: &str = "grand-prix";

/// **The course.** 2.7 miles, and the only track the game ships for now.
///
/// Authored to exercise what the car can do rather than to look like a
/// place: there is a brake, a bend past which steering cannot save you,
/// and a surface that costs you for getting it wrong. A course that never
/// asks for any of those is a corridor.
///
/// The shape, in order:
///   - a long start straight, so the grid and the gantry have somewhere to
///     sit and the field is at speed before the first corner
///   - a Firm right, which teaches what running wide feels like without
///     punishing it
///   - a MustBrake left arriving off a short straight — the first corner
///     that actually requires the brake, placed where it can be seen
///   - a fast esse, Gentle both ways, that rewards carrying speed
///   - a Hard right at the end of the longest straight, which is the
///     hardest single decision on the lap: the most speed to shed, and the
///     most to lose by shedding too much
///   - a Firm left onto the start straight, so the lap joins smoothly
///     rather than at a kink
///
/// ⚠️ NOT YET TUNED BY DRIVING. The lengths are chosen so the sections
/// read at speed and the mileage lands at 2.7; whether the *rhythm* is
/// right is a question only playing it can answer (L023). The format is
/// what makes retuning it cheap — change a `Length` or a `Bend`, not a
/// loop that pushes segments.
pub fn grand_prix() -> Track {
    Track::new()
        .straight(Length::Miles(0.45))
        .corner(Bend::Firm, Dir::Right, Length::Miles(0.25))
        .straight(Length::Miles(0.15))
        .corner(Bend::MustBrake, Dir::Left, Length::Miles(0.22))
        .straight(Length::Miles(0.20))
        .corner(Bend::Gentle, Dir::Right, Length::Miles(0.18))
        .corner(Bend::Gentle, Dir::Left, Length::Miles(0.18))
        .straight(Length::Miles(0.42))
        .corner(Bend::Hard, Dir::Right, Length::Miles(0.20))
        .straight(Length::Miles(0.20))
        .corner(Bend::Firm, Dir::Left, Length::Miles(0.25))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A course's length is what it was authored to be.
    ///
    /// The whole reason sections are stated in distance: this must hold
    /// whatever `segment_length` is, or the format has bought nothing over
    /// counting segments by hand.
    #[test]
    fn a_course_is_as_long_as_it_says() {
        let track = Track::new()
            .straight(Length::Miles(1.0))
            .corner(Bend::Firm, Dir::Right, Length::Miles(0.5))
            .straight(Length::Miles(0.5));

        assert!(
            (track.length_miles() - 2.0).abs() < 0.001,
            "authored 2.0 miles, measured {}",
            track.length_miles(),
        );

        let road = track.build();
        let built = road.segment_count() as f32 * road.segment_length() / UNITS_PER_MILE;
        assert!(
            (built - 2.0).abs() < 0.01,
            "the built road is {built} miles, not the 2.0 authored",
        );
    }

    /// Bend names are claims about the DRIVING, and this is the test that
    /// makes them true rather than decorative.
    ///
    /// `MustBrake` must actually be past the limit and `Firm` must
    /// actually be under it. If the physics is retuned and these names
    /// stop describing the car, this fails — which is the entire reason
    /// bends are named against `BRAKE_BEND` instead of storing curve
    /// numbers.
    #[test]
    fn the_bend_names_describe_what_the_car_can_do() {
        assert!(
            Bend::Gentle.curve() < Bend::Firm.curve(),
            "Gentle must be gentler than Firm",
        );
        assert!(
            Bend::Firm.curve() < BRAKE_BEND,
            "Firm claims to be holdable flat out, but it is past the limit",
        );
        assert!(
            Bend::MustBrake.curve() > BRAKE_BEND,
            "MustBrake claims to need the brake, but it is holdable flat out",
        );
        assert!(
            Bend::Hard.curve() > Bend::MustBrake.curve(),
            "Hard must be harder than MustBrake",
        );
    }

    /// A corner reaches the strength it asked for, in the units the
    /// PHYSICS reads.
    ///
    /// Checking through `curve_at` rather than against the stored raw
    /// number is the point: the two are different kinds, and a course
    /// authored in one but read in the other is the bug that put the car
    /// across the road in 25ms.
    #[test]
    fn a_corner_reaches_the_curve_it_asked_for() {
        for bend in [Bend::Gentle, Bend::Firm, Bend::MustBrake, Bend::Hard] {
            let road = Track::new()
                .corner(bend, Dir::Right, Length::Miles(0.4))
                .build();

            let mut peak = 0.0f32;
            for i in 0..road.segment_count() {
                peak = peak.max(road.curve_at(i as f32 * road.segment_length()).abs());
            }
            assert!(
                (peak - bend.curve()).abs() < 0.01,
                "{bend:?} asked for {} and the road peaks at {peak}",
                bend.curve(),
            );
        }
    }

    /// Corners ease rather than starting at full strength, and the ease
    /// scales with the corner's own length.
    ///
    /// A long sweep easing over the same six segments as a tight bend
    /// reads differently, which is why the fraction is a fraction.
    #[test]
    fn corners_ease_in_proportion_to_their_length() {
        for miles in [0.2f32, 0.8] {
            let road = Track::new()
                .corner(Bend::Firm, Dir::Right, Length::Miles(miles))
                .build();

            let first = road.curve_at(0.0).abs();
            let peak = Bend::Firm.curve();
            assert!(
                first < peak * 0.5,
                "a {miles}-mile corner opens at {first}, most of its {peak} peak — \
                 that reads as a kink, not a corner",
            );

            // The ease should span a fraction of the corner, so a longer
            // corner takes proportionally longer to reach full strength.
            let n = road.segment_count();
            let expected_ease = (n as f32 * EASE_FRACTION) as usize;
            let at_ease_end =
                road.curve_at(expected_ease as f32 * road.segment_length()).abs();
            assert!(
                (at_ease_end - peak).abs() < peak * 0.15,
                "a {miles}-mile corner should be at full strength by segment \
                 {expected_ease} of {n}, but reads {at_ease_end} against {peak}",
            );
        }
    }

    /// Direction is respected, and the two are mirror images.
    #[test]
    fn corners_go_the_way_they_are_told() {
        let right = Track::new()
            .corner(Bend::Firm, Dir::Right, Length::Miles(0.3))
            .build();
        let left = Track::new()
            .corner(Bend::Firm, Dir::Left, Length::Miles(0.3))
            .build();

        let mid = right.segment_count() as f32 / 2.0 * right.segment_length();
        assert!(right.curve_at(mid) > 0.0, "Right should bend positive");
        assert!(left.curve_at(mid) < 0.0, "Left should bend negative");
        assert!(
            (right.curve_at(mid) + left.curve_at(mid)).abs() < 0.001,
            "the two directions should be mirror images",
        );
    }

    /// A section too short to render is a failure, not a silent rounding.
    #[test]
    #[should_panic(expected = "shorter than one")]
    fn a_section_shorter_than_a_segment_is_rejected() {
        Track::new().straight(Length::Metres(0.1)).build();
    }

    /// An empty course is a failure, matching `Road::new`.
    #[test]
    #[should_panic(expected = "at least one section")]
    fn an_empty_course_is_rejected() {
        Track::new().build();
    }

    /// The shipped course is the length it claims, and contains corners
    /// on BOTH sides of the limit.
    ///
    /// The second half is the one that matters: a track of nothing but
    /// holdable bends has no decisions in it, and a track of nothing but
    /// unholdable ones is the same decision repeated. Asserting the mix
    /// is asserting that the course is a course.
    #[test]
    fn the_shipped_course_asks_real_questions() {
        let track = grand_prix();
        assert!(
            (track.length_miles() - 2.7).abs() < 0.05,
            "the course is {} miles, not the 2.7 it is described as",
            track.length_miles(),
        );

        let road = track.build();
        let mut peak = 0.0f32;
        let mut holdable_corner = false;
        for i in 0..road.segment_count() {
            let c = road.curve_at(i as f32 * road.segment_length()).abs();
            peak = peak.max(c);
            // A real corner that is under the limit — not merely a
            // straight, which is also under it.
            if c > BRAKE_BEND * 0.4 && c < BRAKE_BEND {
                holdable_corner = true;
            }
        }
        assert!(
            peak > BRAKE_BEND,
            "no corner on the course needs the brake: peak {peak} against a limit of {BRAKE_BEND}",
        );
        assert!(
            holdable_corner,
            "every corner on the course needs the brake, so there is no choice to make",
        );
    }

    /// Miles and metres agree.
    #[test]
    fn the_two_length_units_are_consistent() {
        let a = Length::Miles(1.0).units();
        let b = Length::Metres(1609.344).units();
        assert!((a - b).abs() < 1.0, "a mile is {a} but 1609.344m is {b}");
    }
}
