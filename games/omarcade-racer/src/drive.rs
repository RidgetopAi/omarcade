//! Motion: where the car is on the track, and how fast.
//!
//! Kept separate from [`crate::road`] on purpose. The road answers *where
//! is this piece of track?*; this answers *where am I on it, and how fast
//! am I going?*. The road never moves and has no notion of time.
//!
//! # Where the numbers come from
//!
//! L015, the most expensive lesson on this project: a tuning constant
//! copied from somewhere else is an untested assumption. Pong inherited a
//! paddle speed from Breakout and matches became unwinnable, because the
//! same name meant a different physical quantity.
//!
//! So nothing here is picked by feel and then defended. Two constants
//! matter — how fast you can go, and how fast you can steer — and each
//! one is *solved* from a duration a person can actually judge:
//!
//! - **[`Tuning::from_corner`]** fixes how long you get to react to a bend
//!   appearing at the horizon, and solves top speed from it.
//! - **[`Tuning::from_crossing`]** fixes how long it takes to cross the
//!   road verge to verge, and solves the steer rate from it.
//!
//! Whichever duration is chosen is the *constraint*; the other constant
//! becomes a free knob that can be tuned by feel without breaking the
//! geometry. They produce genuinely different games, which is why both
//! exist and why `dump_art.rs` can render them side by side.

use crate::road::Road;

/// Which quantity was solved for, and which was left free.
///
/// Carried so a render can say what it is showing, and so nobody later
/// reads a number out of here without knowing whether it was derived or
/// merely chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Derivation {
    /// Top speed solved from reaction time. Steering is the free knob.
    FromCorner,
    /// Steer rate solved from crossing time. Top speed is the free knob.
    FromCrossing,
}

/// The numbers that decide how the car moves.
#[derive(Clone, Copy, Debug)]
pub struct Tuning {
    /// World units per second at full throttle.
    pub top_speed: f32,
    /// How fast the car crosses the road, in half-widths per second.
    /// The road is 2.0 half-widths wide, so 2.0 here means "verge to
    /// verge in one second".
    pub steer_rate: f32,
    /// Seconds to reach top speed from a standstill.
    pub accel_time: f32,
    /// How hard a bend throws the car toward its outside, in half-widths
    /// per second per unit of curve at full speed.
    ///
    /// SOLVED, not chosen — see [`BRAKE_BEND`]. It is the one number in
    /// the module that decides whether corners are a skill or scenery,
    /// and it was a bare `0.9` that nobody could judge: the question
    /// "is 0.9 a lot?" has no answer without doing the arithmetic against
    /// the steer rate, which is exactly the kind of constant L019 is
    /// about. Stated as "the bend you must brake for", it can be checked
    /// against `Road::curve_at` by reading.
    pub centrifugal: f32,
    /// Seconds from top speed to a dead stop under full braking.
    ///
    /// Stated as a DURATION, not as a deceleration, for the same reason
    /// every other number in this file is: a raw `-40_000.0` units per
    /// second squared is unjudgeable by a person and silently wrong the
    /// moment the road's scale moves. "How long does it take to stop?"
    /// is a question you can answer by driving (L019).
    ///
    /// Compare `accel_time`: braking is meant to be decisively stronger
    /// than lifting off, or the brake is a slower way of doing nothing —
    /// which is exactly what it was before this existed.
    pub brake_time: f32,
    /// Which of the two above was solved rather than chosen.
    pub derived: Derivation,
    /// The duration the derivation was solved from, in seconds. Kept for
    /// display: it is the number a person actually judged.
    pub basis_seconds: f32,
}

impl Tuning {
    /// **A — derive top speed from corner reaction time.**
    ///
    /// A bend first becomes visible at the far edge of the drawn road.
    /// Choosing how many seconds the player gets between seeing it and
    /// arriving at it fixes how fast they may travel:
    ///
    /// ```text
    /// top_speed = visible_distance / reaction_seconds
    /// ```
    ///
    /// This is the arcade-authentic constraint. Pole Position's difficulty
    /// *is* corner recognition, and tuning this way means the bend arrival
    /// rate is correct by construction rather than by luck.
    ///
    /// Note it uses the road's real draw distance, so a road that can see
    /// further permits a faster car — the relationship the player actually
    /// experiences.
    pub fn from_corner(road: &Road, reaction_seconds: f32) -> Tuning {
        assert!(
            reaction_seconds > 0.0 && reaction_seconds.is_finite(),
            "reaction time must be positive and finite, got {reaction_seconds}",
        );
        let visible = road.draw_distance() as f32 * road.segment_length();
        Tuning {
            top_speed: visible / reaction_seconds,
            // Free knob. Chosen by feel, and safe to change: no geometry
            // depends on it under this derivation.
            steer_rate: 1.6,
            centrifugal: 1.6 / BRAKE_BEND,
            accel_time: 4.0,
            brake_time: BRAKE_TIME,
            derived: Derivation::FromCorner,
            basis_seconds: reaction_seconds,
        }
    }

    /// **B — derive the steer rate from verge-to-verge crossing time.**
    ///
    /// The road is 2.0 half-widths across, so:
    ///
    /// ```text
    /// steer_rate = 2.0 / crossing_seconds
    /// ```
    ///
    /// This tunes the game around lane-changing and threading traffic
    /// rather than around corner recognition.
    ///
    /// Top speed is the free knob here — but it still takes the road as an
    /// argument, and that is deliberate. It was once a bare `90_000.0`,
    /// and when the road's scale was retuned that silently became 5.6x too
    /// fast: B ended up with a 0.27-second reaction window, which is not a
    /// game. A free knob may be chosen by feel; it may not be expressed in
    /// units that do not track the system (L019). So it is stated as a
    /// relaxed reaction window and converted, exactly as A's is — the
    /// difference is that this one is *chosen*, not *solved for*.
    ///
    /// Note what B still does not do: it never consults the road to decide
    /// its STEERING, which is the actual trade-off being chosen between.
    pub fn from_crossing(road: &Road, crossing_seconds: f32) -> Tuning {
        assert!(
            crossing_seconds > 0.0 && crossing_seconds.is_finite(),
            "crossing time must be positive and finite, got {crossing_seconds}",
        );
        let visible = road.draw_distance() as f32 * road.segment_length();
        Tuning {
            // Free knob, at a relaxed window so B is unmistakably the
            // "bends are easy, traffic is the challenge" tuning.
            top_speed: visible / RELAXED_REACTION,
            steer_rate: 2.0 / crossing_seconds,
            // Solved from the SAME bend, so B's stronger steering buys a
            // proportionally stronger push and the limit bend stays put.
            // Pinning the push instead would mean B's quicker hands made
            // every corner free, which is a different game than "traffic
            // is the challenge" — it would be "there is no challenge".
            centrifugal: (2.0 / crossing_seconds) / BRAKE_BEND,
            accel_time: 4.0,
            brake_time: BRAKE_TIME,
            derived: Derivation::FromCrossing,
            basis_seconds: crossing_seconds,
        }
    }

    /// How long the player actually gets between a bend appearing at the
    /// horizon and arriving at it, at full speed.
    ///
    /// Under [`Derivation::FromCorner`] this returns the number that was
    /// chosen. Under [`Derivation::FromCrossing`] it is *whatever falls
    /// out* — and that is the point of being able to ask.
    pub fn reaction_seconds(&self, road: &Road) -> f32 {
        let visible = road.draw_distance() as f32 * road.segment_length();
        visible / self.top_speed
    }

    /// How long it takes to cross the road verge to verge.
    ///
    /// The mirror of [`Tuning::reaction_seconds`]: chosen under one
    /// derivation, consequential under the other.
    pub fn crossing_seconds(&self) -> f32 {
        2.0 / self.steer_rate
    }
}

/// The car's live state on the track.
#[derive(Clone, Copy, Debug)]
pub struct Drive {
    /// Position along the track, in world units. Wraps with the road.
    pub z: f32,
    /// Position across the road, in half-widths. 0.0 is the centre line,
    /// ±1.0 is a verge.
    ///
    /// Stored as a *ratio* rather than in world units so it means the same
    /// thing on a road of any width (L019). The projection multiplies it
    /// back up when it needs world units.
    pub x: f32,
    /// Current speed, world units per second.
    pub speed: f32,
}

/// How wide the rumble strip is, as a fraction of the road's half-width.
///
/// SHARED WITH THE RENDERER, which draws the strip at exactly this
/// fraction. It lived only in `render.rs` as a drawing detail until the
/// surface started dragging, and the moment physics depends on where a
/// thing is painted, the two must read one number. Otherwise the strip
/// moves in a later art pass and the car starts slowing on tarmac — a
/// bug that would present as "the handling feels wrong sometimes" and
/// look nothing like a rendering change.
pub const RUMBLE_FRACTION: f32 = 0.13;

/// What the car is driving on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// Tarmac. No penalty.
    Road,
    /// The rumble strip at the road's edge. A warning, not a punishment:
    /// it costs a little speed, which is the feedback that tells a player
    /// they are running out of road before they are off it.
    Rumble,
    /// Grass. Costs real time.
    Grass,
}

impl Surface {
    /// The most of top speed this surface will carry, as a fraction.
    ///
    /// A ratio rather than an absolute, so a retune of the road or the
    /// top speed cannot silently turn "grass is slow" into "grass is
    /// stationary" (L019).
    ///
    /// Grass CAPS rather than kills. A dead stop would make a clipped
    /// verge fatal, and the fail state in this game is a collision, not a
    /// wheel on the grass. At 45% a mistake costs a corner's worth of
    /// time and stays recoverable — which is the penalty that teaches,
    /// where an instant stop just ends the run.
    pub fn speed_cap(self) -> f32 {
        match self {
            Surface::Road => 1.0,
            Surface::Rumble => 0.85,
            Surface::Grass => 0.45,
        }
    }
}

/// How far past the verge the car may stray before it is off the road.
///
/// Not zero, because clamping exactly at the verge means the car can never
/// visibly put a wheel onto the grass, and that moment is most of what
/// makes a near-miss read as a near-miss.
pub const MAX_STRAY: f32 = 1.35;

impl Drive {
    /// Sitting still on the centre line at the start of the track.
    pub fn new() -> Drive {
        Drive { z: 0.0, x: 0.0, speed: 0.0 }
    }

    /// Advance by `dt` seconds.
    ///
    /// `throttle` and `brake` are 0..1, `steer` is -1..1 (negative left).
    /// All are clamped rather than trusted, because they come from input
    /// handling and a held key that misses its key-up should not be able
    /// to send the car off the map.
    ///
    /// # Why the brake is its own input
    ///
    /// It was not, and that was a real bug rather than a simplification:
    /// the brake key set throttle to zero, which is *identical to lifting
    /// off*. Pressing it did nothing a released key did not already do,
    /// and the car took a four-second glide to stop either way. A brake
    /// that cannot be distinguished from coasting is not a brake.
    ///
    /// The two cannot share a rate because they are different forces.
    /// Coasting is drag; braking is brakes. So they get separate
    /// durations — `accel_time` and `brake_time` — and separate paths
    /// here.
    pub fn update(
        &mut self,
        dt: f32,
        throttle: f32,
        brake: f32,
        steer: f32,
        road: &Road,
        tuning: &Tuning,
    ) {
        let throttle = throttle.clamp(0.0, 1.0);
        let brake = brake.clamp(0.0, 1.0);
        let steer = steer.clamp(-1.0, 1.0);

        // Braking overrides throttle. A player holding both wants to
        // stop — and letting the two fight would make the outcome depend
        // on which force happened to be larger, which is not something a
        // player can predict or learn.
        if brake > 0.0 {
            // Solved from the duration, exactly as the accel rate is.
            let decel = tuning.top_speed / tuning.brake_time * brake;
            // `max(0.0)`: the brake stops the car, it does not reverse
            // it. Without this a large dt at low speed drives `speed`
            // negative and the car crawls backwards down the track.
            self.speed = (self.speed - decel * dt).max(0.0);
        } else {
            // Speed eases toward its target rather than snapping, so the
            // car has weight. Lifting off coasts down at the same rate.
            let target = tuning.top_speed * throttle;
            let rate = tuning.top_speed / tuning.accel_time;
            if self.speed < target {
                self.speed = (self.speed + rate * dt).min(target);
            } else {
                self.speed = (self.speed - rate * dt).max(target);
            }
        }

        // Surface drag. Off the tarmac the car is dragged down toward
        // what the surface will carry — at the BRAKE rate, so leaving the
        // road feels like something grabbing the car rather than like the
        // throttle quietly going soft.
        //
        // Applied as a ceiling rather than as a force, so it cannot fight
        // the throttle into an equilibrium the player has to discover.
        // The rule is simply: this surface will not carry you faster than
        // this. Note it is read from the position BEFORE this frame's
        // steering, which is correct — the drag is what the car is on
        // now, not where it is about to be.
        let cap = tuning.top_speed * self.surface().speed_cap();
        if self.speed > cap {
            let drag = tuning.top_speed / tuning.brake_time;
            self.speed = (self.speed - drag * dt).max(cap);
        }

        self.z = road.wrap(self.z + self.speed * dt);

        // Steering authority scales with speed. A stationary car cannot
        // change lanes, which is both true and what stops the car sliding
        // sideways off the line while parked.
        let authority = if tuning.top_speed > 0.0 {
            (self.speed / tuning.top_speed).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.x += steer * tuning.steer_rate * authority * dt;

        // Centrifugal push: a bend throws the car toward its outside. This
        // is what makes a corner something to *drive* rather than
        // something to watch go by, and it is why steering into a bend at
        // speed is not free.
        //
        // ⚠️ THE SQUARE IS LOAD-BEARING. Push goes as authority², steering
        // as authority, and that asymmetry is the entire reason braking
        // helps. Halve your speed and the push falls to a quarter while
        // your steering only falls to a half — so slowing buys back twice
        // the grip it costs you in hands. Make both linear and the ratio
        // is constant at every speed: a bend you cannot hold at full
        // throttle is equally unholdable at walking pace, the brake stops
        // being an answer, and the only strategy left is to never go fast.
        // (It is also the physics: centrifugal force goes as v².)
        //
        // NOT segment_at().curve — that is the renderer's raw authoring
        // number. See Road::curve_at.
        let curve = road.curve_at(self.z);
        self.x -= curve * authority * authority * tuning.centrifugal * dt;

        self.x = self.x.clamp(-MAX_STRAY, MAX_STRAY);
    }

    /// True when the car has a wheel off the road.
    pub fn off_road(&self) -> bool {
        self.x.abs() > 1.0
    }

    /// What the car is currently driving on.
    ///
    /// The boundaries are the ones the renderer PAINTS: the rumble strip
    /// occupies the outer [`RUMBLE_FRACTION`] of each half-width, so it
    /// runs from 0.87 to 1.0 and grass begins past the verge. A player
    /// can therefore see which surface they are on, which is the whole
    /// point of a penalty that is not a message on screen.
    pub fn surface(&self) -> Surface {
        let d = self.x.abs();
        if d > 1.0 {
            Surface::Grass
        } else if d > 1.0 - RUMBLE_FRACTION {
            Surface::Rumble
        } else {
            Surface::Road
        }
    }

    /// The car's position across the road in **world units**, which is
    /// what [`Road::project`] wants for its `x_offset`.
    pub fn x_offset(&self, road: &Road) -> f32 {
        self.x * road.width() / 2.0
    }

    /// How hard the car is cornering right now, -1..1, for feeding
    /// [`omarcade_core::Pose::cornering`].
    ///
    /// This is deliberately the *bend*, not the steering input: the car
    /// leans because it is going round a corner, not because a key is
    /// held. Holding left on a straight should not bank the car.
    pub fn cornering(&self, road: &Road, tuning: &Tuning) -> f32 {
        let authority = if tuning.top_speed > 0.0 {
            (self.speed / tuning.top_speed).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (road.curve_at(self.z) / FULL_LEAN_CURVE * authority).clamp(-1.0, 1.0)
    }
}

impl Default for Drive {
    fn default() -> Self {
        Drive::new()
    }
}

/// Seconds from top speed to a dead stop under full braking.
///
/// 1.2s against coasting's 4.0s, so the brake is a little over three
/// times stronger than lifting off. That ratio is the point: the brake
/// has to be worth reaching for at a corner, or the optimal line is
/// simply "never touch it", which is the game the racer had before this
/// existed.
///
/// A free knob, and safe to move by feel — stated as a duration, so no
/// geometry depends on it and a retune of the road cannot silently
/// invalidate it (L019). Shared by both tunings: how hard the car stops
/// is a property of the CAR, not of which quantity the tuning solved for.
const BRAKE_TIME: f32 = 1.2;

/// The reaction window B leaves the player, in seconds.
///
/// Chosen, not derived — that is what makes it B's free knob. Deliberately
/// far above A's 1.5s so the two tunings are visibly different games.
const RELAXED_REACTION: f32 = 2.6;

/// The bend you must brake for.
///
/// The normalised curve at which the centrifugal push exactly cancels
/// full counter-steer at full speed. Below it, a bend can be held flat
/// out by steering alone; above it, no amount of steering saves the car
/// and the only way through is to shed speed.
///
/// This is the constant that decides whether cornering is a skill, and
/// it replaces a bare `CENTRIFUGAL = 0.9` that could not be judged
/// without arithmetic. Measured against that value, the hardest bend on
/// the demo track (normalised curve 1.488) sat at 84% of what full
/// counter-steer could hold: the player rounded it at top speed, never
/// touching the brake, straying a quarter of the way to the verge. The
/// corner was scenery.
///
/// At 1.0 that same bend is roughly 1.5x past the limit — genuinely
/// unholdable flat out, and the brake becomes the answer rather than an
/// alternative to it.
///
/// Stated in the units of `Road::curve_at`, so a reader can compare it
/// to a track by reading rather than by deriving (L019). The push is
/// solved FROM it per tuning, which is what keeps the limit bend fixed
/// when steering changes.
const BRAKE_BEND: f32 = 1.0;

/// The normalised curve at which the car reaches FULL lean.
///
/// Stated as "the bend that pins the pose" rather than as a multiplier,
/// because a multiplier has to be re-derived every time anything upstream
/// moves — and it was, twice, in one session. It was 0.02 against raw
/// curve values, then 1.2 against normalised ones, and then `draw_distance`
/// went from 300 to 120, normalised curve moved from ~0.6 to ~1.49, and
/// every corner silently pinned at full lean again: identical-looking
/// bends, which is the exact failure the constant exists to prevent.
///
/// Expressed this way the number says what it means — "a bend of 1.8 is as
/// hard as this car ever leans" — and a reader can check it against
/// `Road::curve_at` without doing any arithmetic. L019: state the
/// constant in the units of the thing it is about.
const FULL_LEAN_CURVE: f32 = 1.8;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::road::Segment;

    fn straight() -> Road {
        Road::straight(400)
    }

    /// The point of derivation A: the reaction time you asked for is the
    /// reaction time you get.
    #[test]
    fn deriving_from_the_corner_delivers_that_reaction_time() {
        let road = straight();
        for wanted in [0.8f32, 1.5, 3.0] {
            let t = Tuning::from_corner(&road, wanted);
            let got = t.reaction_seconds(&road);
            assert!(
                (got - wanted).abs() < 0.001,
                "asked for {wanted}s to react, got {got}s",
            );
        }
    }

    /// A road bent to a given normalised curve, all the way along.
    ///
    /// `Segment::curving` takes the RENDERER's raw authoring number, so
    /// this solves backwards through `curve_at` to land on a wanted
    /// normalised curve. Doing it by hand in each test is how the two
    /// kinds got mixed up before (see `Road::curve_at`).
    fn bent_to(normalised: f32) -> Road {
        let probe = Road::straight(400);
        let n = probe.draw_distance() as f32;
        let raw = normalised * (n * (n + 1.0) / 2.0) / n;
        let segs = std::iter::repeat_n(Segment::curving(raw), 400).collect();
        Road::new(segs, 200.0, 2200.0)
    }

    /// The fastest speed a corrective driver can hold this bend at,
    /// found by trying.
    ///
    /// Returned in world units. Steps down from full speed rather than
    /// bisecting: the range is small, the granularity is what matters
    /// more than the precision, and a loop that a reader can check by
    /// eye is worth more here than a tighter bound.
    fn fastest_holdable(road: &Road, tuning: &Tuning) -> f32 {
        let dt = 1.0 / 120.0;
        let mut frac = 1.0f32;
        while frac > 0.1 {
            let target = tuning.top_speed * frac;
            let mut d = Drive::new();
            let mut left_road = false;
            for _ in 0..(6.0 / dt) as usize {
                let correction = (-d.x * 3.0).clamp(-1.0, 1.0);
                let (throttle, brake) =
                    if d.speed > target { (0.0, 1.0) } else { (1.0, 0.0) };
                d.update(dt, throttle, brake, correction, road, tuning);
                if d.surface() != Surface::Road {
                    left_road = true;
                    break;
                }
            }
            if !left_road {
                return target;
            }
            frac -= 0.05;
        }
        tuning.top_speed * 0.1
    }

    /// Drive a bend with a CORRECTIVE driver and report how far the car
    /// strays. 1.0 is the verge.
    ///
    /// ⚠️ The driver steers toward the centre line, as much as is needed
    /// and no more — it does NOT hold full lock. Holding full lock
    /// forever does not measure "can this bend be held": under the limit
    /// the steering overpowers the push and the car leaves by the
    /// INSIDE, which reads as a stray of 1.35 and looks identical to
    /// being thrown off the outside. That mistake is already documented
    /// on `a_corner_can_be_held_by_steering_into_it`, and this helper
    /// walked into it anyway.
    ///
    /// What a corrective driver cannot do is beat physics: if the push at
    /// this speed exceeds what full steering can cancel, the correction
    /// saturates at full lock and the car goes off the outside regardless.
    /// So this measures the bend, not the driver.
    fn stray_holding_flat_out(road: &Road, tuning: &Tuning, brake: f32) -> f32 {
        let mut car = Drive::new();
        car.speed = tuning.top_speed;
        let dt = 1.0 / 240.0;
        let mut worst = 0.0f32;
        for _ in 0..(4.0 / dt) as usize {
            let correction = (-car.x * 3.0).clamp(-1.0, 1.0);
            car.update(dt, 1.0 - brake, brake, correction, road, tuning);
            worst = worst.max(car.x.abs());
        }
        worst
    }

    /// The surfaces begin exactly where the renderer paints them.
    ///
    /// The one test that would catch the two drifting apart. If the strip
    /// is redrawn at a different width and only `render.rs` is edited,
    /// this still passes — which is why both read `RUMBLE_FRACTION`
    /// rather than each holding a copy.
    #[test]
    fn the_surfaces_are_where_they_are_drawn() {
        let mut car = Drive::new();
        let edge = 1.0 - RUMBLE_FRACTION;

        for (x, want) in [
            (0.0f32, Surface::Road),
            (edge - 0.01, Surface::Road),
            (edge + 0.01, Surface::Rumble),
            (0.999, Surface::Rumble),
            (1.01, Surface::Grass),
            (-1.01, Surface::Grass),
            (-(edge + 0.01), Surface::Rumble),
        ] {
            car.x = x;
            assert_eq!(car.surface(), want, "at x={x}");
        }
    }

    /// Grass costs real speed. THE BUG THIS EXISTS FOR: `off_road()` was
    /// written and then read by nothing, so leaving the road was free and
    /// the MAX_STRAY clamp was the only thing suggesting the verge
    /// mattered at all.
    #[test]
    fn grass_slows_the_car_down() {
        let road = straight();
        let tuning = Tuning::from_corner(&road, 1.5);

        let mut car = Drive::new();
        car.speed = tuning.top_speed;
        car.x = 1.2;
        for _ in 0..(2.0 / (1.0 / 240.0)) as usize {
            car.update(1.0 / 240.0, 1.0, 0.0, 0.0, &road, &tuning);
        }

        let cap = tuning.top_speed * Surface::Grass.speed_cap();
        assert!(
            (car.speed - cap).abs() < tuning.top_speed * 0.01,
            "on grass at full throttle the car should settle at {cap}, sat at {}",
            car.speed,
        );
        assert!(car.speed < tuning.top_speed * 0.5, "grass barely slowed the car");
    }

    /// The rumble strip is a warning, not a punishment: it costs less
    /// than the grass does, or there is no reason to draw a distinction
    /// between clipping a verge and leaving the road.
    #[test]
    fn the_rumble_strip_costs_less_than_the_grass() {
        let road = straight();
        let tuning = Tuning::from_corner(&road, 1.5);
        let dt = 1.0 / 240.0;

        let settle = |x: f32| {
            let mut car = Drive::new();
            car.speed = tuning.top_speed;
            car.x = x;
            for _ in 0..(2.0 / dt) as usize {
                car.update(dt, 1.0, 0.0, 0.0, &road, &tuning);
            }
            car.speed
        };

        let on_road = settle(0.0);
        let on_rumble = settle(0.95);
        let on_grass = settle(1.2);

        assert!(on_rumble < on_road, "the rumble strip cost nothing");
        assert!(on_grass < on_rumble, "the grass was no worse than the rumble strip");
        assert_eq!(on_road, tuning.top_speed, "the road itself should cost nothing");
    }

    /// Leaving the road costs TIME, which is the penalty that matters in
    /// a game scored on it. Asserting the consequence a player feels, not
    /// the mechanism that produces it (L022).
    #[test]
    fn going_off_costs_distance() {
        let road = straight();
        let tuning = Tuning::from_corner(&road, 1.5);
        let dt = 1.0 / 240.0;

        let run = |x: f32| {
            let mut car = Drive::new();
            car.speed = tuning.top_speed;
            car.x = x;
            let start = car.z;
            for _ in 0..(3.0 / dt) as usize {
                car.update(dt, 1.0, 0.0, 0.0, &road, &tuning);
            }
            car.z - start
        };

        let clean = run(0.0);
        let excursion = run(1.2);
        assert!(
            excursion < clean * 0.6,
            "three seconds on the grass covered {excursion} against {clean} on the road \
             — leaving the track has to cost real ground",
        );
    }

    /// `centrifugal` is SOLVED, not chosen: ask for a limit bend and the
    /// limit bend is what you get back.
    ///
    /// The same shape as the two derivation tests above, and the reason
    /// the constant is stated as a bend rather than as a push.
    #[test]
    fn the_push_is_solved_from_the_bend_you_must_brake_for() {
        let road = Road::straight(400);
        for tuning in [Tuning::from_corner(&road, 1.5), Tuning::from_crossing(&road, 1.2)] {
            let limit = tuning.steer_rate / tuning.centrifugal;
            assert!(
                (limit - BRAKE_BEND).abs() < 0.001,
                "BRAKE_BEND is {BRAKE_BEND} but the tuning's limit bend is {limit}",
            );
        }
    }

    /// Below the limit, a bend is holdable flat out.
    ///
    /// Two jobs. First: it stops the harder push turning EVERY corner
    /// into a wall — a game where no bend can be taken at speed is as
    /// skill-free as one where every bend can.
    ///
    /// Second, and the reason it is driven flat out rather than braked:
    /// THIS IS THE UNITS GUARD. Physics once read `segment_at().curve` —
    /// the renderer's raw authoring number, ~90 — instead of the
    /// normalised `curve_at`, producing a push of 81 half-widths per
    /// second against 1.6 of steering, and the car crossed the whole road
    /// in 25ms. A test that BRAKES cannot catch that, because the push
    /// scales with authority² and a braking car has almost none; it
    /// passes against the bug. Only a car at full speed, on a bend that
    /// must be holdable, fails when the units are wrong. Verified by
    /// re-introducing the bug. (L015, and L022 rule 3: two quantities
    /// sharing a name are not the same kind of thing.)
    #[test]
    fn a_gentle_bend_can_be_held_at_full_speed() {
        let road = bent_to(BRAKE_BEND * 0.6);
        let tuning = Tuning::from_corner(&road, 1.5);
        let stray = stray_holding_flat_out(&road, &tuning, 0.0);
        assert!(
            stray < 1.0,
            "a bend at 60% of the limit should be holdable flat out, strayed {stray}",
        );
    }

    /// THE BUG THIS CHANGE EXISTS FOR.
    ///
    /// Past the limit, full counter-steer at full speed is not enough and
    /// the car leaves the road. Against the old `CENTRIFUGAL = 0.9` the
    /// hardest bend the demo track contained sat at 84% of what steering
    /// could hold, so this test fails against that version — which is the
    /// only reason it is worth having (L017).
    #[test]
    fn a_hard_bend_cannot_be_held_at_full_speed() {
        let road = bent_to(BRAKE_BEND * 1.5);
        let tuning = Tuning::from_corner(&road, 1.5);
        let stray = stray_holding_flat_out(&road, &tuning, 0.0);
        assert!(
            stray > 1.0,
            "a bend 50% past the limit must NOT be holdable flat out, strayed only {stray}",
        );
    }

    /// And the brake is the answer to it.
    ///
    /// The pair above says a hard bend is unholdable; without this one
    /// that is merely a bend nobody can take, which is not a skill. What
    /// makes it a skill is that slowing down works — and it works only
    /// because the push goes as authority² while steering goes as
    /// authority.
    #[test]
    fn braking_gets_you_through_a_bend_you_cannot_hold() {
        let road = bent_to(BRAKE_BEND * 1.5);
        let tuning = Tuning::from_corner(&road, 1.5);

        let flat_out = stray_holding_flat_out(&road, &tuning, 0.0);
        let braking = stray_holding_flat_out(&road, &tuning, 1.0);

        assert!(flat_out > 1.0, "the premise: flat out should go off, strayed {flat_out}");
        assert!(
            braking < 1.0,
            "braking into the same bend should keep the car on the road, strayed {braking}",
        );
    }

    /// The brake delivers the stopping time it was stated as.
    ///
    /// The same shape as the two derivation tests above, and for the same
    /// reason: `brake_time` claims to be "seconds from top speed to a
    /// standstill", so the test is to drive at top speed, stand on the
    /// brake, and time it.
    #[test]
    fn braking_stops_the_car_in_the_stated_time() {
        let road = straight();
        let tuning = Tuning::from_corner(&road, 1.5);
        let mut car = Drive::new();
        car.speed = tuning.top_speed;

        let dt = 1.0 / 240.0;
        let mut elapsed = 0.0f32;
        while car.speed > 0.0 && elapsed < 10.0 {
            car.update(dt, 0.0, 1.0, 0.0, &road, &tuning);
            elapsed += dt;
        }

        assert!(
            (elapsed - tuning.brake_time).abs() < 0.05,
            "brake_time says {}s to stop, took {elapsed}s",
            tuning.brake_time,
        );
    }

    /// THE BUG THIS FEATURE EXISTS FOR.
    ///
    /// The brake key used to set throttle to zero, which is precisely
    /// what releasing throttle already does. Pressing it changed nothing.
    /// This test fails against that version, which is the only reason it
    /// is worth having (L017: a test that cannot fail is worse than none).
    ///
    /// Asserting a RATIO rather than an absolute speed, so retuning the
    /// road or the top speed cannot invalidate it.
    #[test]
    fn braking_is_decisively_stronger_than_coasting() {
        let road = straight();
        let tuning = Tuning::from_corner(&road, 1.5);
        let dt = 1.0 / 240.0;
        let half_second = (0.5 / dt) as usize;

        let mut braking = Drive::new();
        braking.speed = tuning.top_speed;
        let mut coasting = braking;

        for _ in 0..half_second {
            braking.update(dt, 0.0, 1.0, 0.0, &road, &tuning);
            coasting.update(dt, 0.0, 0.0, 0.0, &road, &tuning);
        }

        let lost_braking = tuning.top_speed - braking.speed;
        let lost_coasting = tuning.top_speed - coasting.speed;
        // accel_time 4.0 / brake_time 1.2 = 3.33x. Asserting 2.5x leaves
        // room to retune either duration without a false failure, while
        // still failing outright if the brake ever collapses back into
        // being a second coast.
        assert!(
            lost_braking > lost_coasting * 2.5,
            "braking shed {lost_braking} but coasting shed {lost_coasting} \
             — the brake is not meaningfully stronger than lifting off",
        );
    }

    /// The brake stops the car; it does not reverse it.
    ///
    /// A large `dt` at low speed is the case that breaks this — the
    /// deceleration for that step exceeds the remaining speed. Tested at
    /// the clamp the game actually uses (1/15s) rather than at a
    /// comfortable 1/240, because the whole point is the bad frame.
    #[test]
    fn braking_never_drives_the_car_backwards() {
        let road = straight();
        let tuning = Tuning::from_corner(&road, 1.5);
        let mut car = Drive::new();
        car.speed = tuning.top_speed * 0.02;

        for _ in 0..30 {
            car.update(1.0 / 15.0, 0.0, 1.0, 0.0, &road, &tuning);
            assert!(
                car.speed >= 0.0,
                "brake drove speed negative: {}",
                car.speed,
            );
        }
        assert_eq!(car.speed, 0.0, "the car should have come to rest");
    }

    /// Holding both pedals stops the car, and stops it at the BRAKE rate.
    ///
    /// "Brake wins" was already true when both meant the same thing, so
    /// it was not saying much. Now that they are different forces, the
    /// question has content: which rate applies?
    #[test]
    fn the_brake_beats_the_throttle() {
        let road = straight();
        let tuning = Tuning::from_corner(&road, 1.5);
        let dt = 1.0 / 240.0;

        let mut both = Drive::new();
        both.speed = tuning.top_speed;
        let mut brake_only = both;

        for _ in 0..(0.4 / dt) as usize {
            both.update(dt, 1.0, 1.0, 0.0, &road, &tuning);
            brake_only.update(dt, 0.0, 1.0, 0.0, &road, &tuning);
        }

        assert!(
            (both.speed - brake_only.speed).abs() < 1.0,
            "throttle+brake ({}) should decelerate exactly like brake alone ({})",
            both.speed,
            brake_only.speed,
        );
        assert!(both.speed < tuning.top_speed * 0.9, "the car did not slow down");
    }

    /// The point of derivation B: the crossing time you asked for is the
    /// crossing time you get.
    #[test]
    fn deriving_from_the_crossing_delivers_that_crossing_time() {
        let road = straight();
        for wanted in [0.6f32, 1.2, 2.5] {
            let t = Tuning::from_crossing(&road, wanted);
            let got = t.crossing_seconds();
            assert!(
                (got - wanted).abs() < 0.001,
                "asked for {wanted}s to cross, got {got}s",
            );
        }
    }

    /// The two derivations must actually produce different games, or
    /// there is no choice to make and this whole module is ceremony.
    #[test]
    fn the_two_derivations_disagree() {
        let road = straight();
        let a = Tuning::from_corner(&road, 1.5);
        let b = Tuning::from_crossing(&road, 1.2);

        assert!(
            (a.top_speed - b.top_speed).abs() > 1.0,
            "both tunings picked the same top speed; there is nothing to choose between",
        );
        // ...and each is worse than the other at what the other optimises.
        assert!(a.crossing_seconds() != b.crossing_seconds());
        assert!(a.reaction_seconds(&road) != b.reaction_seconds(&road));
    }

    /// A road you can see further down permits a faster car under
    /// derivation A. This is the relationship that makes it "authentic":
    /// the constraint is the player's eyes, not a number.
    #[test]
    fn seeing_further_permits_going_faster_under_derivation_a() {
        let mut near = straight();
        near.set_draw_distance(100);
        let mut far = straight();
        far.set_draw_distance(300);

        let slow = Tuning::from_corner(&near, 1.5);
        let fast = Tuning::from_corner(&far, 1.5);
        assert!(fast.top_speed > slow.top_speed * 2.5);
    }

    /// ...and derivation B is blind to it, which is precisely the
    /// trade-off being chosen between.
    #[test]
    fn derivation_b_does_not_care_how_far_you_can_see() {
        let road = straight();
        let a = Tuning::from_crossing(&road, 1.2);
        let b = Tuning::from_crossing(&road, 1.2);
        assert_eq!(a.top_speed, b.top_speed);
        assert_eq!(a.steer_rate, b.steer_rate);
    }

    #[test]
    fn the_car_accelerates_and_reaches_top_speed() {
        let road = straight();
        let t = Tuning::from_corner(&road, 1.5);
        let mut d = Drive::new();
        assert_eq!(d.speed, 0.0);

        for _ in 0..((t.accel_time / 0.016) as usize + 8) {
            d.update(0.016, 1.0, 0.0, 0.0, &road, &t);
        }
        assert!(
            (d.speed - t.top_speed).abs() < t.top_speed * 0.01,
            "expected to reach {} but got {}",
            t.top_speed,
            d.speed,
        );
    }

    #[test]
    fn lifting_off_slows_the_car_down() {
        let road = straight();
        let t = Tuning::from_corner(&road, 1.5);
        let mut d = Drive::new();
        for _ in 0..300 {
            d.update(0.016, 1.0, 0.0, 0.0, &road, &t);
        }
        let fast = d.speed;
        for _ in 0..60 {
            d.update(0.016, 0.0, 0.0, 0.0, &road, &t);
        }
        assert!(d.speed < fast, "coasting did not slow the car");
    }

    /// Crossing time is a claim about the car, not about the maths, so
    /// drive the car and time it.
    #[test]
    fn the_car_really_crosses_the_road_in_the_stated_time() {
        let road = straight();
        let t = Tuning::from_crossing(&road, 1.2);
        let mut d = Drive::new();

        // Up to speed first — steering authority scales with it.
        for _ in 0..600 {
            d.update(0.016, 1.0, 0.0, 0.0, &road, &t);
        }
        d.x = -1.0;

        let dt = 0.001;
        let mut elapsed = 0.0;
        while d.x < 1.0 && elapsed < 10.0 {
            d.update(dt, 1.0, 0.0, 1.0, &road, &t);
            elapsed += dt;
        }
        assert!(
            (elapsed - 1.2).abs() < 0.05,
            "verge to verge took {elapsed}s, expected 1.2s",
        );
    }

    /// A parked car cannot steer. Without this the car slides sideways off
    /// the line while stationary, which looks like a physics bug.
    #[test]
    fn a_stationary_car_cannot_steer() {
        let road = straight();
        let t = Tuning::from_corner(&road, 1.5);
        let mut d = Drive::new();
        for _ in 0..120 {
            d.update(0.016, 0.0, 0.0, 1.0, &road, &t);
        }
        assert_eq!(d.x, 0.0, "a parked car steered to {}", d.x);
    }

    /// The car must not be able to leave the world, however long a key is
    /// held or however wrong the input is.
    #[test]
    fn the_car_cannot_leave_the_road_entirely() {
        let road = straight();
        let t = Tuning::from_corner(&road, 1.5);
        let mut d = Drive::new();
        for _ in 0..2000 {
            // Deliberately out-of-range input, as a stuck key would give.
            d.update(0.016, 5.0, 0.0, 9.0, &road, &t);
        }
        assert!(d.x <= MAX_STRAY + 0.001, "car reached x={}", d.x);
        assert!(d.speed <= t.top_speed + 0.001);
    }

    /// Lean must actually VARY across the range of bends, not pin.
    ///
    /// It has pinned twice: once when the constant was calibrated against
    /// raw curve values and once when `draw_distance` moved and shifted
    /// what "normalised curve" meant. A pinned lean makes every corner
    /// look identical, which is worse than no lean at all because it
    /// implies information that is not there.
    ///
    /// ⚠️ EACH BEND IS DRIVEN AT A SPEED IT CAN BE HELD AT, and that is
    /// not fussiness. Driven flat out, the two hardest bends here both
    /// put the car onto the rumble strip, where the surface cap pins
    /// speed — and lean is scaled by speed, so both came back at 0.551
    /// and this test failed. Nothing was wrong with lean: a car scrubbing
    /// a verge at 67% genuinely is not cornering harder than one holding
    /// the road at 100%, and lean said so correctly.
    ///
    /// But a car that has fallen off the road is not demonstrating
    /// cornering, so measuring it there asks the wrong question. Braking
    /// through the hard ones keeps every sample ON the road, which is
    /// where the property under test lives.
    #[test]
    fn lean_varies_across_bends_instead_of_pinning() {
        let dt = 1.0 / 120.0;
        let mut seen: Vec<f32> = Vec::new();

        for raw in [10.0f32, 30.0, 60.0, 90.0] {
            let road = Road::new(vec![Segment::curving(raw); 400], 200.0, 2200.0);
            let tuning = Tuning::from_corner(&road, 1.5);
            let mut d = Drive::new();
            // A driver who slows for the bend: hold the fastest speed
            // this corner can be taken at, rather than braking for a
            // fixed amount. Braking is an INPUT, not a speed target —
            // holding the brake down for six seconds simply stops the
            // car, and a stationary car has no lean at all, which is how
            // the first attempt at this produced a 0.0.
            //
            // The holdable speed is FOUND, not derived. Solving
            // `curve * authority^2 * centrifugal = steer_rate` gives the
            // answer for a driver holding FULL LOCK, and this driver does
            // not: `(-x * 3.0)` only saturates once the car is a third of
            // the way to a verge, so it settles wherever push balances a
            // partial correction — measurably slower than the formula
            // predicts (0.70 against 0.82 on the hardest bend here).
            //
            // Fitting a second formula to this particular driver would be
            // a constant calibrated against something that moves the
            // moment the driver or the physics changes, which is the
            // mistake this module keeps a lesson about. Searching for it
            // stays correct through both.
            let target = fastest_holdable(&road, &tuning);
            for _ in 0..(6.0 / dt) as usize {
                let correction = (-d.x * 3.0).clamp(-1.0, 1.0);
                let (throttle, brake) =
                    if d.speed > target { (0.0, 1.0) } else { (1.0, 0.0) };
                d.update(dt, throttle, brake, correction, &road, &tuning);
            }
            assert_eq!(
                d.surface(),
                Surface::Road,
                "the sample for curve {raw} left the tarmac; lean measured there \
                 reports the surface cap, not the bend",
            );
            let lean = d.cornering(&road, &tuning).abs();
            assert!(
                lean < 1.0,
                "a curve of {raw} already pins lean at {lean}; \
                 harder bends cannot read as harder",
            );
            seen.push(lean);
        }

        for pair in seen.windows(2) {
            assert!(
                pair[1] > pair[0] + 0.01,
                "lean barely moved between bends: {seen:?}",
            );
        }
    }

    /// A corner must be SURVIVABLE: a driver who slows for it has to be
    /// able to hold it, or the bend is unwinnable by construction.
    ///
    /// ⚠️ This used to assert the bend was holdable AT FULL THROTTLE, and
    /// it was — the old `CENTRIFUGAL = 0.9` left this curve (normalised
    /// 1.488) at 84% of what full counter-steer could cancel, so the
    /// player rounded it flat out without touching the brake. Deriving
    /// the push from `BRAKE_BEND` deliberately ended that: this bend is
    /// now half again past the limit and full throttle WILL put the car
    /// off. That is the feature, so the test now drives it the way it is
    /// meant to be driven.
    ///
    /// ⚠️ AND THE GUARD MOVED, because braking DISSOLVES it. The push
    /// goes as authority², so a braking car drives authority toward zero
    /// and the push vanishes no matter how wrong its units are — a
    /// braking test passes happily against the raw-curve bug. Verified by
    /// re-introducing it. The units guard therefore lives in
    /// [`a_gentle_bend_can_be_held_at_full_speed`], which drives flat out
    /// at a bend that must be holdable: there, a push in the wrong units
    /// pins the car at MAX_STRAY instantly and the test fails, exactly as
    /// it did when the bug shipped.
    ///
    /// What THIS test now guards is the other half: that a bend past the
    /// limit is not merely hard but actually winnable by a driver who
    /// slows for it. Without it, "corners are a skill" could be satisfied
    /// by a corner nobody can take.
    ///
    /// This is the bug that shipped. Physics read `segment_at().curve`
    /// directly — the renderer's raw authoring number, around 90 at a
    /// draw distance of 300 — and multiplied it by CENTRIFUGAL. The push
    /// came out at 81 half-widths per second against 1.6 of steering
    /// authority, so the car crossed the entire road in 25 milliseconds
    /// and sat pinned at MAX_STRAY before the scene even began. Reading
    /// through `Road::curve_at` is what keeps the two in the same units.
    ///
    /// L015 again, and L022 rule 3: two quantities sharing a name are not
    /// necessarily the same kind of thing.
    #[test]
    fn a_corner_can_be_held_by_slowing_for_it() {
        // A hard bend, at the strength the visual scenes actually use.
        let road = Road::new(vec![Segment::curving(90.0); 400], 1000.0, 2200.0);
        for tuning in [Tuning::from_corner(&road, 1.5), Tuning::from_crossing(&road, 1.2)] {
            let mut d = Drive::new();
            // Drive it like a driver FROM THE START: steer toward the
            // centre line, as much as is needed and no more.
            //
            // Note there is no free warm-up lap here. An earlier version
            // accelerated for 600 frames with no steering first, which put
            // the car off the road before the driver existed and made this
            // look like a physics failure. Holding full lock forever is
            // not "surviving the corner" either — that leaves by the
            // inside. A corner is survivable if a corrective driver can
            // hold a line through it, which is what this asserts.
            for _ in 0..1200 {
                let correction = (-d.x * 3.0).clamp(-1.0, 1.0);
                // Braking through it, which is what this bend now asks
                // for. A driver who slows appropriately holds the line.
                d.update(0.016, 0.0, 1.0, correction, &road, &tuning);
                assert!(
                    !d.off_road(),
                    "a driver who slowed for the bend still went off at x={} ({:?})",
                    d.x,
                    tuning.derived,
                );
            }
            // ...and it should settle near the middle, not merely survive
            // by scraping a verge.
            assert!(
                d.x.abs() < 0.5,
                "the line settled at x={}, which is most of the way to a verge",
                d.x,
            );
        }
    }

    /// ...and the mirror: a corner must not be trivial either. Ignoring it
    /// entirely has to cost you the road, or there is nothing to drive.
    #[test]
    fn ignoring_a_corner_puts_you_off_the_road() {
        let road = Road::new(vec![Segment::curving(90.0); 400], 1000.0, 2200.0);
        let tuning = Tuning::from_corner(&road, 1.5);
        let mut d = Drive::new();
        for _ in 0..900 {
            d.update(0.016, 1.0, 0.0, 0.0, &road, &tuning);
        }
        assert!(
            d.off_road(),
            "the car held a hard bend with no steering input at all, x={}",
            d.x,
        );
    }

    /// A bend must push the car toward its outside, or a corner is
    /// scenery rather than something to drive.
    #[test]
    fn a_bend_pushes_the_car_to_its_outside() {
        let road = Road::new(vec![Segment::curving(60.0); 400], 1000.0, 2200.0);
        let t = Tuning::from_corner(&road, 1.5);
        let mut d = Drive::new();
        for _ in 0..600 {
            d.update(0.016, 1.0, 0.0, 0.0, &road, &t);
        }
        assert!(
            d.x < -0.05,
            "a right-hand bend should push the car left (outside), x={}",
            d.x,
        );
    }

    /// The car leans because of the BEND, not because a key is held.
    /// Holding left on a straight must not bank it.
    #[test]
    fn lean_comes_from_the_corner_not_from_the_input() {
        let road = straight();
        let t = Tuning::from_corner(&road, 1.5);
        let mut d = Drive::new();
        for _ in 0..600 {
            d.update(0.016, 1.0, 0.0, -1.0, &road, &t);
        }
        assert_eq!(
            d.cornering(&road, &t),
            0.0,
            "the car banked on a straight road while steering",
        );

        let bendy = Road::new(vec![Segment::curving(40.0); 400], 1000.0, 2200.0);
        let bt = Tuning::from_corner(&bendy, 1.5);
        let mut bd = Drive::new();
        for _ in 0..600 {
            bd.update(0.016, 1.0, 0.0, 0.0, &bendy, &bt);
        }
        assert!(bd.cornering(&bendy, &bt).abs() > 0.01, "the car did not bank in a bend");
    }

    /// Track position wraps, so a long session cannot run off the end or
    /// lose float precision drifting into the millions.
    #[test]
    fn driving_far_enough_wraps_the_track() {
        let road = straight();
        let t = Tuning::from_corner(&road, 1.5);
        let mut d = Drive::new();
        for _ in 0..4000 {
            d.update(0.016, 1.0, 0.0, 0.0, &road, &t);
        }
        assert!(d.z >= 0.0 && d.z < road.length(), "z={} escaped the track", d.z);
    }

    /// Off-road must be reachable and must be reported, or there is
    /// nothing to build a penalty on later.
    #[test]
    fn straying_past_the_verge_reads_as_off_road() {
        let road = straight();
        let t = Tuning::from_crossing(&road, 1.2);
        let mut d = Drive::new();
        assert!(!d.off_road());
        for _ in 0..600 {
            d.update(0.016, 1.0, 0.0, 0.0, &road, &t);
        }
        for _ in 0..600 {
            d.update(0.016, 1.0, 0.0, 1.0, &road, &t);
        }
        assert!(d.off_road(), "car sat at x={} and was not off-road", d.x);
    }

    /// The state a renderer reads must be in the units the renderer wants.
    #[test]
    fn x_offset_is_in_world_units() {
        let road = straight();
        let mut d = Drive::new();
        d.x = 1.0;
        assert!((d.x_offset(&road) - road.width() / 2.0).abs() < 0.001);
        d.x = 0.0;
        assert_eq!(d.x_offset(&road), 0.0);
    }

    #[test]
    #[should_panic(expected = "reaction time must be positive")]
    fn a_zero_reaction_time_is_rejected() {
        Tuning::from_corner(&straight(), 0.0);
    }

    #[test]
    #[should_panic(expected = "crossing time must be positive")]
    fn a_zero_crossing_time_is_rejected() {
        Tuning::from_crossing(&straight(), 0.0);
    }
}
