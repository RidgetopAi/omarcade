//! Landers, and what it takes to kill one.
//!
//! ★ S6 GAVE THEM A PURPOSE. In S5 a Lander arrived, drifted and died —
//! it was a target. Now it hunts: it finds the nearest Humanoid, drops
//! to the surface, takes it, and carries it upward. That is the whole
//! difference between a shooting gallery and Defender, because a Lander
//! you ignore now costs you something.
//!
//! ★★ S7 MADE THE THREAT REAL. A Lander that carries a Humanoid off the
//! top of the world FUSES WITH IT into a Mutant, and a Mutant does not
//! want your civilians — it wants you. That is the arcade's cruellest
//! trick: every person you fail to save comes back as something hunting
//! you, so falling behind compounds.
//!
//! ⚠️ AND A MUTANT NEVER SHOOTS STRAIGHT. Brian's spec is explicit about
//! it, and it is the whole reason Mutants are frightening rather than
//! merely fast: a straight shot from a thing you are flying toward is
//! dodged by moving; an angled one has to be read.

use crate::humanoid::Humanoids;
use crate::world::{self, Terrain};

/// What a Lander is worth.
///
/// ★ 100, FROM THE ARCADE'S OWN ENEMY CHART. The build plan said 150;
/// Brian checked the machine and corrected it (ref:defender-points).
/// Where the plan and the chart disagree, the chart wins.
pub const LANDER_POINTS: u32 = 100;

/// What a Mutant is worth. Also from the arcade's chart.
pub const MUTANT_POINTS: u32 = 150;

/// How fast a Mutant chases, in world units per second.
///
/// ★ FAST ENOUGH TO BE FRIGHTENING, SLOWER THAN THE SHIP AT FULL
/// THROTTLE. A Mutant that could outrun you would make fleeing
/// pointless, and fleeing is the only answer to several of them at once.
pub const MUTANT_SPEED: f32 = 290.0;

/// How erratically a Mutant moves, as a fraction of its speed.
///
/// The arcade's Mutants jitter rather than sliding along a clean vector,
/// which is most of why they are hard to shoot.
pub const MUTANT_JITTER: f32 = 0.55;

/// How often a Mutant fires, in seconds.
pub const MUTANT_FIRE_INTERVAL: f32 = 1.6;

/// How far away a Mutant will bother shooting from, in world units.
pub const MUTANT_FIRE_RANGE: f32 = world::VIEW_W * 0.75;

/// ★ THE MINIMUM ANGLE OF A MUTANT'S SHOT, in radians off horizontal.
///
/// ⚠️ BRIAN'S SPEC: "NEVER SHOOT STRAIGHT — always at an angle." This is
/// that rule as a number, and it is enforced rather than hoped for: the
/// fire code pushes the angle out to at least this far from level, so
/// there is no alignment of ship and Mutant that produces a flat shot.
pub const MUTANT_MIN_ANGLE: f32 = 0.30;

/// How wide a Lander is for the purposes of being hit, in world units.
///
/// ★ DERIVED FROM THE ART, NOT PICKED. The Lander is 12 units wide in
/// art space and draws at [`crate::art::SCALE`], so this is the real
/// drawn half-width — the same rule Omaprix's posts follow, where the
/// hitbox IS the art rather than a number that drifts away from it.
pub const LANDER_HALF_W: f32 = 6.0 * crate::art::SCALE;
pub const LANDER_HALF_H: f32 = 7.5 * crate::art::SCALE;

/// How high above a Humanoid a Lander sits while grabbing it.
///
/// ⚠️ RAISED FROM 54 AFTER LOOKING AT A FRAME. At 54 the Lander sat
/// almost on top of its victim and the tractor beam was a stub — it read
/// as two sprites touching rather than as a beam pulling someone up.
/// The beam is the only warning a player gets, so it has to be visible
/// as a beam at a glance.
pub const GRAB_HEIGHT: f32 = 120.0;

/// How high above the ridge a Lander settles.
///
/// ⚠️ RAISED FROM 120 AFTER LOOKING AT A FRAME. At 120 the Landers sat
/// in the bottom quarter of the screen, tucked against the ridge — the
/// fight happened along the floor and the whole middle of the playfield
/// was empty. Defender's Landers occupy the airspace you fly through.
/// The ridge still governs the height, so they rise and fall with it.
pub const HOVER_HEIGHT: f32 = 300.0;

/// How fast a Lander drifts along the world, in units per second.
///
/// Slow. A Lander in the original is not chasing you in this phase — it
/// is looking for a Humanoid, and the menace is that it is ignoring you.
pub const DRIFT_SPEED: f32 = 46.0;

/// How long a Lander drifts before it starts hunting, in seconds.
///
/// ★ A DELIBERATE GRACE PERIOD. Landers that begin hunting the instant
/// they arrive reach the surface before the player has crossed the
/// world once, and the first thing you learn is that you were already
/// too late. Drifting first is also how the arcade reads: they mill
/// about, and then they get to work.
pub const HUNT_AFTER: f32 = 2.4;

/// How fast a hunting Lander moves toward its target, in units/second.
///
/// Faster than the drift — this one has decided — but well under the
/// ship's top speed, so an attentive player always has the option of
/// getting there first.
pub const HUNT_SPEED: f32 = 132.0;

/// How fast a Lander descends and climbs, in units per second.
pub const DESCEND_SPEED: f32 = 150.0;
pub const CLIMB_SPEED: f32 = 96.0;

/// How close, horizontally, a Lander must be to grab.
pub const GRAB_REACH_X: f32 = 16.0;

/// How long the tractor beam holds before the Humanoid is lifted.
///
/// ★ THE WINDOW THE WHOLE STAGE TURNS ON. This is the moment the player
/// is meant to notice and intervene: long enough to see the beam, fly
/// over and shoot, short enough that ignoring it costs you.
pub const GRAB_SECONDS: f32 = 0.9;

/// How long the warp-in takes, in seconds.
pub const WARP_SECONDS: f32 = 0.55;

/// How long the death animation holds the corpse before it is removed.
///
/// The particles outlive this; it only governs how long the Lander's own
/// shape keeps being drawn, flashing, before it goes.
pub const DEATH_SECONDS: f32 = 0.18;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Materialising. Cannot be hit yet — a Lander that could be killed
    /// before it finished arriving would let a player farm the spawn
    /// point, and would look like a bug besides.
    Warping,
    /// Alive and drifting, not yet interested in anyone.
    Hovering,
    /// Has chosen a Humanoid and is closing on it.
    Hunting,
    /// Over its target, beam on, lifting it.
    Grabbing,
    /// Climbing with a Humanoid. Shoot it and the passenger falls.
    Carrying,
    /// Hit, and playing out its death. No longer collidable.
    Dying,
}

/// What kind of enemy this is.
///
/// ⚠️ ONE TYPE, NOT TWO. A Mutant IS a Lander that has fused with a
/// person — it keeps the same position, the same death, the same
/// hitbox, and only its behaviour and its art differ. Splitting them
/// into separate structs would duplicate all of that to express a
/// difference that is really one field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Lander,
    Mutant,
}

#[derive(Debug, Clone, Copy)]
pub struct Lander {
    /// Lander or Mutant.
    pub kind: Kind,
    /// Seconds until this one can fire again. Mutants only.
    pub fire_cooldown: f32,
    /// World x, always wrapped.
    pub x: f32,
    /// Height above the world floor, matching the ship's convention.
    pub y: f32,
    /// Signed drift, world units per second.
    pub vx: f32,
    pub phase: Phase,
    /// Seconds spent in the current phase.
    pub elapsed: f32,
    /// Index of the Humanoid being hunted or carried, if any.
    ///
    /// ⚠️ AN INDEX, AND THE HUMANOID LIST MUST THEREFORE NEVER SHRINK.
    /// `Humanoids::step` deliberately keeps the dead in place for
    /// exactly this reason — a Vec that compacted on death would
    /// silently re-point every carrier at the wrong person.
    pub target: Option<usize>,
}

impl Lander {
    pub fn new(x: f32, y: f32, vx: f32) -> Self {
        Self {
            kind: Kind::Lander,
            fire_cooldown: MUTANT_FIRE_INTERVAL * 0.5,
            x: world::wrap(x),
            y,
            vx,
            phase: Phase::Warping,
            elapsed: 0.0,
            target: None,
        }
    }

    /// A Mutant, already formed — used when the world ends and every
    /// surviving Lander turns.
    pub fn mutant(x: f32, y: f32) -> Self {
        Self {
            kind: Kind::Mutant,
            fire_cooldown: MUTANT_FIRE_INTERVAL,
            x: world::wrap(x),
            y,
            vx: 0.0,
            phase: Phase::Hovering,
            elapsed: 0.0,
            target: None,
        }
    }

    /// Fuse with the Humanoid this one is carrying.
    ///
    /// ★ THE TRANSFORMATION THE WHOLE STAGE IS NAMED FOR. It keeps its
    /// position, so on screen the Lander you failed to stop simply
    /// becomes the thing that is now hunting you.
    pub fn mutate(&mut self) {
        self.kind = Kind::Mutant;
        self.phase = Phase::Hovering;
        self.elapsed = 0.0;
        self.target = None;
        self.fire_cooldown = MUTANT_FIRE_INTERVAL;
    }

    pub fn is_mutant(&self) -> bool {
        self.kind == Kind::Mutant
    }

    /// What killing this one is worth.
    pub fn points(&self) -> u32 {
        match self.kind {
            Kind::Lander => LANDER_POINTS,
            Kind::Mutant => MUTANT_POINTS,
        }
    }

    /// Can a shot hit this one right now?
    ///
    /// ★ EVERY LIVE PHASE, NOT JUST HOVERING. A carrier that could not
    /// be shot would make the abduction unstoppable, which is the one
    /// thing the stage must never be.
    pub fn is_target(&self) -> bool {
        matches!(
            self.phase,
            Phase::Hovering | Phase::Hunting | Phase::Grabbing | Phase::Carrying
        )
    }

    /// Is this one holding someone?
    pub fn carrying(&self) -> Option<usize> {
        match self.phase {
            Phase::Grabbing | Phase::Carrying => self.target,
            _ => None,
        }
    }

    /// Should this one still be drawn?
    pub fn is_alive(&self) -> bool {
        !matches!(self.phase, Phase::Dying if self.elapsed >= DEATH_SECONDS)
    }

    /// How far through the current phase, 0..=1.
    pub fn progress(&self) -> f32 {
        let span = match self.phase {
            Phase::Warping => WARP_SECONDS,
            Phase::Dying => DEATH_SECONDS,
            Phase::Grabbing => GRAB_SECONDS,
            // Open-ended phases have no "through" to be part of.
            Phase::Hovering | Phase::Hunting | Phase::Carrying => return 1.0,
        };
        (self.elapsed / span).clamp(0.0, 1.0)
    }

    /// Mark this one as hit. Returns the points it is worth.
    ///
    /// The caller is responsible for dropping any passenger — this type
    /// does not own the Humanoid list and must not pretend to.
    pub fn kill(&mut self) -> u32 {
        let worth = self.points();
        self.phase = Phase::Dying;
        self.elapsed = 0.0;
        self.target = None;
        worth
    }

    /// Advance this Lander. `prey` is where its target is, if it has one
    /// and that target is still grabbable.
    ///
    /// ⚠️ THE HUMANOID IS PASSED IN RATHER THAN REACHED FOR. A Lander
    /// that held a reference to the Humanoid list could not be stepped
    /// while that list was being mutated, and the borrow checker would
    /// push the whole thing into a shared-mutable shape it does not need.
    /// The owner resolves the target and hands over a position.
    fn step(
        &mut self,
        terrain: &Terrain,
        prey: Option<(f32, f32)>,
        ship: Option<(f32, f32)>,
        noise: &mut u32,
        dt: f32,
    ) -> Outcome {
        self.elapsed += dt;
        let mut outcome = Outcome::None;

        // ★ A MUTANT IGNORES THE ABDUCTION MACHINE ENTIRELY. It has
        // nothing left to abduct with — it already ate its Humanoid —
        // and its only goal is the ship. Handled before the phase match
        // rather than as another phase, because "which of these two
        // creatures am I" is a different question from "how far through
        // an abduction am I".
        if self.kind == Kind::Mutant && self.phase != Phase::Dying {
            return self.step_mutant(ship, noise, dt);
        }

        match self.phase {
            Phase::Warping => {
                if self.elapsed >= WARP_SECONDS {
                    self.phase = Phase::Hovering;
                    self.elapsed = 0.0;
                }
            }

            Phase::Hovering => {
                self.x = world::wrap(self.x + self.vx * dt);

                // Follow the ridge rather than holding an absolute
                // height: a Lander at a fixed y would sink into a peak
                // and float high over a valley, and the terrain here has
                // real peaks. Eased rather than snapped, or it jitters
                // over the ridge's fine detail.
                let want = terrain.height_at(self.x) + HOVER_HEIGHT;
                self.y += (want - self.y) * (1.0 - (-4.0 * dt).exp());

                if self.elapsed >= HUNT_AFTER {
                    outcome = Outcome::WantsTarget;
                }
            }

            Phase::Hunting => {
                let Some((px, py)) = prey else {
                    // The target is gone — shot, taken by someone else,
                    // or fallen. Go back to drifting and look again.
                    self.phase = Phase::Hovering;
                    self.elapsed = 0.0;
                    self.target = None;
                    return Outcome::None;
                };

                // ⚠️ CLOSE THE GAP THROUGH `delta`, NOT BY SUBTRACTING.
                // A Lander at x = 5 hunting someone at WORLD_W - 5 would
                // otherwise fly the entire world eastward to reach
                // something ten units west of it.
                let gap = world::delta(self.x, px);
                if gap.abs() > GRAB_REACH_X {
                    let step = HUNT_SPEED * dt;
                    self.x = world::wrap(self.x + gap.signum() * step.min(gap.abs()));
                    self.vx = HUNT_SPEED * gap.signum();
                }

                // Descend toward the target as it closes, so the arrival
                // is a swoop rather than a drop straight down.
                let want_y = py + GRAB_HEIGHT;
                if self.y > want_y {
                    self.y = (self.y - DESCEND_SPEED * dt).max(want_y);
                }

                if world::delta(self.x, px).abs() <= GRAB_REACH_X
                    && (self.y - want_y).abs() < 6.0
                {
                    self.phase = Phase::Grabbing;
                    self.elapsed = 0.0;
                    outcome = Outcome::Grabbed;
                }
            }

            Phase::Grabbing => {
                let Some((_, py)) = prey else {
                    self.phase = Phase::Hovering;
                    self.elapsed = 0.0;
                    self.target = None;
                    return Outcome::None;
                };

                // Hold station while the beam does its work, and draw
                // the Humanoid up to meet it.
                let _ = py;
                if self.elapsed >= GRAB_SECONDS {
                    self.phase = Phase::Carrying;
                    self.elapsed = 0.0;
                }
            }

            Phase::Carrying => {
                if prey.is_none() {
                    // Passenger gone (shot out of the beam): resume.
                    self.phase = Phase::Hovering;
                    self.elapsed = 0.0;
                    self.target = None;
                    return Outcome::None;
                }
                self.y += CLIMB_SPEED * dt;
                // Drift a little while climbing so the ascent is not a
                // dead vertical line.
                self.x = world::wrap(self.x + self.vx.signum() * DRIFT_SPEED * 0.4 * dt);

                // ⚠️ S7 TAKES OVER HERE. At the top of the world this
                // should become a Mutant and, if it was the last person,
                // end the world. Reported rather than acted on, so the
                // stage boundary is visible in the code.
                if self.y >= world::VIEW_H * 1.6 {
                    outcome = Outcome::ReachedTop;
                }
            }

            Phase::Dying => {}
        }

        outcome
    }
}

impl Lander {
    /// A Mutant's whole behaviour: chase the ship, and shoot at it.
    fn step_mutant(&mut self, ship: Option<(f32, f32)>, noise: &mut u32, dt: f32) -> Outcome {
        let Some((sx, sy)) = ship else {
            // No ship to hunt — it is dead or between lives. Drift, so a
            // respawning player is not instantly surrounded by Mutants
            // that tracked them while they could not move.
            self.x = world::wrap(self.x + self.vx * dt);
            return Outcome::None;
        };

        // ⚠️ delta, NOT SUBTRACTION. A Mutant at x = 5 chasing a ship at
        // WORLD_W - 5 must go ten units west, not four screens east.
        let gap_x = world::delta(self.x, sx);
        let gap_y = sy - self.y;

        // Jitter, so the approach is not a clean interception line. The
        // arcade's Mutants shudder toward you, which is most of why they
        // are hard to hit.
        let jx = (next01(noise) - 0.5) * 2.0 * MUTANT_JITTER;
        let jy = (next01(noise) - 0.5) * 2.0 * MUTANT_JITTER;

        let len = (gap_x * gap_x + gap_y * gap_y).sqrt().max(1.0);
        let dir_x = gap_x / len + jx;
        let dir_y = gap_y / len + jy;
        let dl = (dir_x * dir_x + dir_y * dir_y).sqrt().max(0.001);

        self.x = world::wrap(self.x + (dir_x / dl) * MUTANT_SPEED * dt);
        self.y += (dir_y / dl) * MUTANT_SPEED * dt;
        self.vx = (dir_x / dl) * MUTANT_SPEED;

        // Stay in the world vertically — a Mutant that chased a climbing
        // ship forever would leave the playfield and never come back.
        self.y = self.y.clamp(20.0, world::VIEW_H * 1.5);

        self.fire_cooldown -= dt;
        if self.fire_cooldown <= 0.0 && gap_x.abs() < MUTANT_FIRE_RANGE {
            self.fire_cooldown = MUTANT_FIRE_INTERVAL * (0.7 + 0.6 * next01(noise));
            return Outcome::Fires;
        }

        Outcome::None
    }

    /// Where a Mutant's shot should go, aimed at `(sx, sy)`.
    ///
    /// ⚠️⚠️ NEVER STRAIGHT. Brian's spec is explicit and this is where it
    /// is ENFORCED rather than hoped for: the aim is computed, and then
    /// the angle is PUSHED OUT to at least [`MUTANT_MIN_ANGLE`] off
    /// horizontal. There is no alignment of ship and Mutant that yields
    /// a flat shot, including the obvious one where both sit at exactly
    /// the same altitude — which is precisely the case a naive "aim at
    /// the player" would fire dead level.
    pub fn aim_at(&self, sx: f32, sy: f32, noise: &mut u32) -> (f32, f32) {
        let dx = world::delta(self.x, sx);
        let dy = sy - self.y;

        let mut angle = dy.atan2(dx.abs().max(1.0));

        if angle.abs() < MUTANT_MIN_ANGLE {
            // Level or nearly level: kick it off horizontal. Which way
            // is chosen from the noise so a player cannot learn to sit
            // in one place and always duck the same direction.
            let up = if angle.abs() < 1e-4 {
                next01(noise) < 0.5
            } else {
                angle < 0.0
            };
            angle = if up { -MUTANT_MIN_ANGLE } else { MUTANT_MIN_ANGLE };
        }

        let dir = if dx < 0.0 { -1.0 } else { 1.0 };
        (angle.cos() * dir, angle.sin())
    }
}

/// What a Lander's step needs its owner to do.
///
/// The Lander cannot reach the Humanoid list, so it says what happened
/// and the owner applies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    None,
    /// Drifted long enough; wants a Humanoid to hunt.
    WantsTarget,
    /// The beam has taken hold of its target.
    Grabbed,
    /// Carried a Humanoid off the top of the world — time to fuse into
    /// a Mutant.
    ReachedTop,
    /// A Mutant wants to shoot at the ship.
    Fires,
}

/// Every Lander in the world.
#[derive(Debug, Default)]
pub struct Landers {
    live: Vec<Lander>,
    /// Humanoid indices already spoken for, so two Landers never hunt
    /// the same person and end up stacked on the same spot.
    claimed: Vec<usize>,
    /// Noise for Mutant jitter and fire timing. On the collection so a
    /// whole world of Mutants shares one deterministic stream.
    noise: u32,
}

impl Landers {
    pub fn new() -> Self {
        Self { live: Vec::new(), claimed: Vec::new(), noise: 0x4D07_A17E }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Lander> {
        self.live.iter()
    }

    pub fn len(&self) -> usize {
        self.live.len()
    }

    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }

    /// How many are still killable — the number that matters for "is the
    /// wave over", which S9 will ask.
    pub fn remaining(&self) -> usize {
        self.live.iter().filter(|l| l.phase != Phase::Dying).count()
    }

    pub fn spawn(&mut self, lander: Lander) {
        self.live.push(lander);
    }

    pub fn clear(&mut self) {
        self.live.clear();
        self.claimed.clear();
    }

    /// Scatter `count` Landers across the world, away from `avoid_x`.
    ///
    /// ⚠️ NOT UNIFORMLY RANDOM ACROSS THE WORLD — spread evenly and then
    /// jittered. Four uniform draws over four screens clump often enough
    /// that a wave regularly arrives as "three in one place and one on
    /// the far side", which reads as a bug rather than as variety.
    pub fn scatter(&mut self, count: usize, avoid_x: f32, terrain: &Terrain, seed: u32) {
        let spacing = world::WORLD_W / count.max(1) as f32;
        for i in 0..count {
            // ⚠️ wrapping_mul, NOT `*`. A plain multiply here PANICS in a
            // debug build from i = 2 onward — an overflow that release
            // mode would have silently wrapped, so the bug would have
            // shipped working and crashed only under a debugger.
            let jitter = hash01(seed.wrapping_add((i as u32).wrapping_mul(2_654_435_761)));
            let mut x = world::wrap(avoid_x + spacing * (i as f32 + 0.5 + (jitter - 0.5) * 0.6));

            // Never directly on top of the player: arriving in the
            // player's lap gives them no chance to react to something
            // they could not have seen coming.
            if world::delta(avoid_x, x).abs() < world::VIEW_W * 0.35 {
                x = world::wrap(x + world::VIEW_W * 0.5);
            }

            let dir = if hash01(seed ^ (i as u32).wrapping_mul(0x9E37_79B9)) < 0.5 {
                -1.0
            } else {
                1.0
            };
            let y = terrain.height_at(x) + HOVER_HEIGHT;
            self.spawn(Lander::new(x, y, DRIFT_SPEED * dir));
        }
    }

    /// Advance every Lander, hunting and abducting through `people`.
    ///
    /// ★ THIS IS WHERE THE ABDUCTION ACTUALLY HAPPENS, and it lives here
    /// rather than in `main` because it is the one place that can see
    /// both lists at once. A Lander asks for a target; this resolves it,
    /// hands over a position, and applies whatever the step reports.
    pub fn step(
        &mut self,
        terrain: &Terrain,
        people: &mut Humanoids,
        ship: Option<(f32, f32)>,
        dt: f32,
    ) -> Vec<(usize, f32, f32)> {
        let mut shots_wanted = Vec::new();
        let mut noise = self.noise;
        for (index, l) in self.live.iter_mut().enumerate() {
            // Resolve the target's position, and drop a target that has
            // stopped being valid — shot while carried, or already taken.
            let prey = match l.target {
                Some(i) => match people.get(i) {
                    Some(h) if h.is_alive() => {
                        let held = matches!(l.phase, Phase::Grabbing | Phase::Carrying);
                        // While hunting, the target must still be free.
                        // While holding, it must still be ours.
                        if held || h.is_grabbable() {
                            Some((h.x, h.y))
                        } else {
                            None
                        }
                    }
                    _ => None,
                },
                None => None,
            };

            match l.step(terrain, prey, ship, &mut noise, dt) {
                Outcome::None => {}

                Outcome::Fires => {
                    // The owner builds the bolt — this type does not
                    // know what a Shot is, and should not learn.
                    shots_wanted.push((index, l.x, l.y));
                }

                Outcome::WantsTarget => {
                    if let Some(i) = people.nearest_grabbable(l.x) {
                        // ⚠️ DO NOT LET TWO LANDERS CLAIM THE SAME
                        // PERSON. Without this they converge on one
                        // Humanoid and stack in the same place, which
                        // looks like a rendering bug rather than a race.
                        if !self.claimed.contains(&i) {
                            l.target = Some(i);
                            l.phase = Phase::Hunting;
                            l.elapsed = 0.0;
                            self.claimed.push(i);
                        }
                    }
                }

                Outcome::Grabbed => {
                    if let Some(i) = l.target {
                        if let Some(h) = people.get_mut(i) {
                            h.grabbed();
                        }
                    }
                }

                Outcome::ReachedTop => {
                    // ★★ THE FUSION. The person is gone and the Lander
                    // that took them becomes the thing that now hunts
                    // you — which is the arcade's cruellest arithmetic:
                    // every rescue you miss makes the next minute
                    // harder, not merely poorer.
                    if let Some(i) = l.target.take() {
                        if let Some(h) = people.get_mut(i) {
                            h.kill();
                        }
                        self.claimed.retain(|&c| c != i);
                    }
                    l.mutate();
                    // Come back down into the playfield rather than
                    // hunting from somewhere the player cannot reach.
                    l.y = world::VIEW_H * 0.9;
                }
            }

            // Carry the passenger along, in both the grab and the climb.
            if let Some(i) = l.carrying() {
                if let Some(h) = people.get_mut(i) {
                    h.x = l.x;
                    // Rise to meet the Lander during the grab, then ride
                    // beneath it.
                    let under = l.y - GRAB_HEIGHT;
                    if l.phase == Phase::Grabbing {
                        let t = (l.elapsed / GRAB_SECONDS).clamp(0.0, 1.0);
                        h.y += (under - h.y) * t * 0.35;
                    } else {
                        h.y = under;
                    }
                }
            }
        }

        // Release the claims of everyone who died, so a fresh Lander can
        // hunt a Humanoid whose previous suitor was shot.
        let live_claims: Vec<usize> =
            self.live.iter().filter_map(|l| l.target).collect();
        self.claimed.retain(|c| live_claims.contains(c));

        self.noise = noise;
        self.live.retain(|l| l.is_alive());
        shots_wanted
    }

    /// ★ THE WORLD HAS ENDED: every surviving Lander becomes a Mutant.
    ///
    /// The arcade's own behaviour, and the reason losing every Humanoid
    /// is catastrophic rather than merely sad — the difficulty does not
    /// decline with nothing left to protect, it spikes.
    pub fn mutate_all(&mut self) {
        for l in &mut self.live {
            if l.phase != Phase::Dying && !l.is_mutant() {
                l.mutate();
                l.y = l.y.min(world::VIEW_H * 0.9);
            }
        }
        self.claimed.clear();
    }

    /// Drop whatever the Lander at `index` was carrying, and report who
    /// it was so the caller can react.
    ///
    /// ⚠️ CALLED BEFORE `kill`, because `kill` clears the target. A
    /// carrier shot without this would take its passenger with it
    /// silently, and the whole catch-and-rescue loop would never fire.
    pub fn release_passenger(&mut self, index: usize, people: &mut Humanoids) -> Option<usize> {
        let who = self.live[index].carrying()?;
        if let Some(h) = people.get_mut(who) {
            h.dropped();
        }
        self.claimed.retain(|&c| c != who);
        self.live[index].target = None;
        Some(who)
    }

    /// The first Lander overlapping `(x, y)`, if any.
    ///
    /// ⚠️ THE X TEST GOES THROUGH [`world::delta`]. A shot at x = 5 and a
    /// Lander at x = WORLD_W - 5 are ten units apart, and subtracting
    /// their coordinates says they are nearly a whole world apart. Every
    /// enemy sitting on the seam would be unkillable, and only from one
    /// side — a bug that would survive any test written near the origin.
    pub fn hit_test(&self, x: f32, y: f32) -> Option<usize> {
        self.live.iter().position(|l| {
            l.is_target()
                && world::delta(l.x, x).abs() <= LANDER_HALF_W
                && (l.y - y).abs() <= LANDER_HALF_H
        })
    }

    /// Kill the one at `index` and report its points.
    pub fn kill(&mut self, index: usize) -> u32 {
        self.live[index].kill()
    }

    /// The enemy at `index`, for a caller that needs to aim from it.
    pub fn get(&self, index: usize) -> Option<&Lander> {
        self.live.get(index)
    }

    /// Advance the shared noise, so a caller aiming a Mutant's shot
    /// draws from the same deterministic stream.
    pub fn next_noise(&mut self) -> u32 {
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        self.noise
    }

    /// How many Mutants are alive.
    pub fn mutants(&self) -> usize {
        self.live.iter().filter(|l| l.is_mutant() && l.phase != Phase::Dying).count()
    }
}

/// Advance a noise state and return 0..1.
fn next01(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}

/// A deterministic 0..1 from an integer, for placement that repeats.
fn hash01(mut h: u32) -> f32 {
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846C_A68B);
    h ^= h >> 16;
    (h & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terrain() -> Terrain {
        Terrain::generate(256, 0x0DEF_E4DE)
    }

    /// An empty world of people — the S5 tests are about Landers alone,
    /// and with nobody to hunt a Lander simply drifts, which is exactly
    /// the behaviour those tests were written against.
    fn nobody() -> Humanoids {
        Humanoids::new()
    }

    #[test]
    fn a_lander_cannot_be_shot_until_it_has_arrived() {
        let t = terrain();
        let mut ls = Landers::new();
        ls.spawn(Lander::new(500.0, 300.0, 0.0));

        assert!(ls.hit_test(500.0, 300.0).is_none(), "warping must not be hittable");
        ls.step(&t, &mut nobody(), None, WARP_SECONDS + 0.01);
        assert!(ls.hit_test(500.0, 300.0).is_some(), "it should be hittable now");
    }

    /// ⚠️ THE SEAM, AS A TEST. This is the bug the module exists to
    /// prevent, and it is invisible anywhere except within one hitbox of
    /// x = 0.
    #[test]
    fn a_lander_on_the_seam_can_be_shot_from_both_sides() {
        let t = terrain();
        let mut ls = Landers::new();
        // Sitting a few units west of the seam, i.e. at the very top of
        // the coordinate range.
        ls.spawn(Lander::new(world::WORLD_W - 4.0, 300.0, 0.0));
        ls.step(&t, &mut nobody(), None, WARP_SECONDS + 0.01);
        let y = ls.iter().next().unwrap().y;

        assert!(
            ls.hit_test(world::WORLD_W - 6.0, y).is_some(),
            "a shot just west of it must connect"
        );
        assert!(
            ls.hit_test(2.0, y).is_some(),
            "a shot just EAST of it — across the seam — must also connect"
        );
        assert!(
            ls.hit_test(world::VIEW_W, y).is_none(),
            "something a screen away must not connect"
        );
    }

    #[test]
    fn a_dead_lander_stops_being_a_target_immediately() {
        let t = terrain();
        let mut ls = Landers::new();
        ls.spawn(Lander::new(500.0, 300.0, 0.0));
        ls.step(&t, &mut nobody(), None, WARP_SECONDS + 0.01);

        let y = ls.iter().next().unwrap().y;
        let hit = ls.hit_test(500.0, y).expect("should be hittable");
        assert_eq!(ls.kill(hit), LANDER_POINTS);
        assert!(ls.hit_test(500.0, y).is_none(), "a dying lander is not a target");
        assert_eq!(ls.remaining(), 0, "it must not count toward the wave");
    }

    #[test]
    fn a_dead_lander_is_removed_once_its_death_has_played() {
        let t = terrain();
        let mut ls = Landers::new();
        ls.spawn(Lander::new(500.0, 300.0, 0.0));
        ls.step(&t, &mut nobody(), None, WARP_SECONDS + 0.01);
        let y = ls.iter().next().unwrap().y;
        let hit = ls.hit_test(500.0, y).unwrap();
        ls.kill(hit);

        ls.step(&t, &mut nobody(), None, DEATH_SECONDS * 0.5);
        assert_eq!(ls.len(), 1, "the corpse should still be drawn");
        ls.step(&t, &mut nobody(), None, DEATH_SECONDS);
        assert!(ls.is_empty(), "the corpse should be gone");
    }

    #[test]
    fn landers_follow_the_ridge_rather_than_a_fixed_height() {
        let t = terrain();
        let mut ls = Landers::new();
        // Start at a wrong height and let it settle.
        ls.spawn(Lander::new(500.0, 10.0, 0.0));
        ls.step(&t, &mut nobody(), None, WARP_SECONDS + 0.01);
        for _ in 0..240 {
            ls.step(&t, &mut nobody(), None, 1.0 / 60.0);
        }
        let l = ls.iter().next().unwrap();
        let want = t.height_at(l.x) + HOVER_HEIGHT;
        assert!((l.y - want).abs() < 4.0, "settled at {} not {want}", l.y);
    }

    #[test]
    fn a_scattered_wave_is_spread_out_and_not_on_the_player() {
        let t = terrain();
        let mut ls = Landers::new();
        ls.scatter(5, 0.0, &t, 12345);
        assert_eq!(ls.len(), 5);
        assert_eq!(ls.remaining(), 5);

        for l in ls.iter() {
            assert!(
                world::delta(0.0, l.x).abs() > world::VIEW_W * 0.3,
                "spawned too close to the player at {}",
                l.x
            );
            assert!(l.x >= 0.0 && l.x < world::WORLD_W, "outside the world: {}", l.x);
        }
    }

    /// The same seed must give the same wave, or a screenshot cannot be
    /// reproduced and a bug cannot be chased.
    #[test]
    fn scattering_is_deterministic() {
        let t = terrain();
        let mut a = Landers::new();
        let mut b = Landers::new();
        a.scatter(4, 300.0, &t, 99);
        b.scatter(4, 300.0, &t, 99);
        let xs: Vec<f32> = a.iter().map(|l| l.x).collect();
        let ys: Vec<f32> = b.iter().map(|l| l.x).collect();
        assert_eq!(xs, ys);
    }

    /// ★★ THE WHOLE STAGE, END TO END. A Lander drifts, finds the person
    /// nearest to it, closes, lowers a beam, and carries them up.
    ///
    /// ⚠️ THIS IS THE TEST THAT MATTERS. Every other test here checks one
    /// joint; this one checks that the joints connect, which is exactly
    /// what a state machine assembled from correct parts still gets
    /// wrong.
    #[test]
    fn a_lander_hunts_a_humanoid_grabs_it_and_carries_it_off() {
        let t = terrain();
        let mut people = Humanoids::new();
        people.spawn(crate::humanoid::Humanoid::new(600.0, t.height_at(600.0), 0.0));

        let mut ls = Landers::new();
        ls.spawn(Lander::new(500.0, t.height_at(500.0) + HOVER_HEIGHT, DRIFT_SPEED));

        // Long enough to warp, drift past HUNT_AFTER, close, and grab.
        let mut saw_hunting = false;
        let mut saw_grabbing = false;
        let mut saw_carrying = false;
        for _ in 0..1800 {
            ls.step(&t, &mut people, None, 1.0 / 120.0);
            people.step(&t, 1.0 / 120.0);
            match ls.iter().next().map(|l| l.phase) {
                Some(Phase::Hunting) => saw_hunting = true,
                Some(Phase::Grabbing) => saw_grabbing = true,
                Some(Phase::Carrying) => saw_carrying = true,
                _ => {}
            }
            if saw_carrying {
                break;
            }
        }

        assert!(saw_hunting, "the Lander never went hunting");
        assert!(saw_grabbing, "the Lander never got a grip");
        assert!(saw_carrying, "the Lander never carried anyone off");

        let h = people.get(0).unwrap();
        assert_eq!(h.state, crate::humanoid::State::Carried, "the person is not held");

        // And the victim must be RISING, under the Lander.
        let before = people.get(0).unwrap().y;
        for _ in 0..30 {
            ls.step(&t, &mut people, None, 1.0 / 120.0);
            people.step(&t, 1.0 / 120.0);
        }
        assert!(
            people.get(0).unwrap().y > before,
            "the victim is not being carried upward"
        );
    }

    /// ★ AND THE PLAYER CAN STOP IT. Shooting a carrier must drop the
    /// passenger, or the abduction is unstoppable and the stage is
    /// pointless.
    #[test]
    fn shooting_a_carrier_drops_its_passenger() {
        let t = terrain();
        let mut people = Humanoids::new();
        people.spawn(crate::humanoid::Humanoid::new(600.0, t.height_at(600.0), 0.0));

        let mut ls = Landers::new();
        ls.spawn(Lander::new(590.0, t.height_at(600.0) + HOVER_HEIGHT, DRIFT_SPEED));

        for _ in 0..2400 {
            ls.step(&t, &mut people, None, 1.0 / 120.0);
            people.step(&t, 1.0 / 120.0);
            if ls.iter().next().map(|l| l.phase) == Some(Phase::Carrying) {
                break;
            }
        }
        assert_eq!(
            ls.iter().next().map(|l| l.phase),
            Some(Phase::Carrying),
            "setup failed: nobody was being carried"
        );

        // Shoot it.
        let released = ls.release_passenger(0, &mut people);
        assert_eq!(released, Some(0), "no passenger was released");
        ls.kill(0);

        assert_eq!(
            people.get(0).unwrap().state,
            crate::humanoid::State::Falling,
            "the passenger did not fall"
        );
    }

    /// ⚠️ TWO LANDERS MUST NOT CONVERGE ON THE SAME PERSON. Without the
    /// claim list they stack on one Humanoid and it reads as a rendering
    /// bug rather than as a race.
    #[test]
    fn two_landers_do_not_hunt_the_same_humanoid() {
        let t = terrain();
        let mut people = Humanoids::new();
        people.spawn(crate::humanoid::Humanoid::new(600.0, t.height_at(600.0), 0.0));
        people.spawn(crate::humanoid::Humanoid::new(1400.0, t.height_at(1400.0), 0.0));

        let mut ls = Landers::new();
        ls.spawn(Lander::new(560.0, t.height_at(560.0) + HOVER_HEIGHT, DRIFT_SPEED));
        ls.spawn(Lander::new(640.0, t.height_at(640.0) + HOVER_HEIGHT, -DRIFT_SPEED));

        for _ in 0..900 {
            ls.step(&t, &mut people, None, 1.0 / 120.0);
            people.step(&t, 1.0 / 120.0);
        }

        let targets: Vec<Option<usize>> = ls.iter().map(|l| l.target).collect();
        if let [Some(a), Some(b)] = targets[..] {
            assert_ne!(a, b, "both Landers claimed the same person");
        }
    }

    /// A Lander whose target dies must give up rather than hunting a
    /// corpse forever.
    #[test]
    fn a_lander_gives_up_on_a_dead_target() {
        let t = terrain();
        let mut people = Humanoids::new();
        people.spawn(crate::humanoid::Humanoid::new(600.0, t.height_at(600.0), 0.0));

        let mut ls = Landers::new();
        ls.spawn(Lander::new(500.0, t.height_at(500.0) + HOVER_HEIGHT, DRIFT_SPEED));

        for _ in 0..600 {
            ls.step(&t, &mut people, None, 1.0 / 120.0);
            people.step(&t, 1.0 / 120.0);
            if ls.iter().next().map(|l| l.phase) == Some(Phase::Hunting) {
                break;
            }
        }
        assert_eq!(ls.iter().next().map(|l| l.phase), Some(Phase::Hunting));

        people.get_mut(0).unwrap().kill();
        for _ in 0..30 {
            ls.step(&t, &mut people, None, 1.0 / 120.0);
            people.step(&t, 1.0 / 120.0);
        }
        assert_eq!(
            ls.iter().next().map(|l| l.phase),
            Some(Phase::Hovering),
            "it kept hunting a dead person"
        );
        assert_eq!(ls.iter().next().unwrap().target, None);
    }

    /// ★★ THE STAGE'S HEADLINE: a Lander that gets away with someone
    /// comes back as a Mutant.
    #[test]
    fn a_lander_that_reaches_the_top_becomes_a_mutant() {
        let t = terrain();
        let mut people = Humanoids::new();
        people.spawn(crate::humanoid::Humanoid::new(600.0, t.height_at(600.0), 0.0));

        let mut ls = Landers::new();
        ls.spawn(Lander::new(590.0, t.height_at(600.0) + HOVER_HEIGHT, DRIFT_SPEED));

        // No ship, so the Mutant has nothing to chase once it forms.
        for _ in 0..6000 {
            ls.step(&t, &mut people, None, 1.0 / 120.0);
            people.step(&t, 1.0 / 120.0);
            if ls.mutants() > 0 {
                break;
            }
        }

        assert_eq!(ls.mutants(), 1, "the Lander never mutated");
        assert_eq!(people.alive(), 0, "the person should be gone");
        let m = ls.iter().next().unwrap();
        assert!(m.is_mutant());
        assert_eq!(m.points(), MUTANT_POINTS, "a Mutant is worth 150");
        assert!(
            m.y <= world::VIEW_H * 1.1,
            "it must come back into the playfield, not hunt from orbit: {}",
            m.y
        );
    }

    /// ⚠️⚠️ BRIAN'S SPEC, AS AN ASSERTION: "NEVER SHOOT STRAIGHT — always
    /// at an angle." Tested over many geometries INCLUDING the exact
    /// case a naive aim gets wrong: ship and Mutant at identical
    /// altitude, where "aim at the player" fires dead level.
    #[test]
    fn a_mutant_never_fires_straight() {
        let m = Lander::mutant(500.0, 400.0);
        let mut noise = 0x1234_5678u32;

        for dy in [-400.0f32, -60.0, -1.0, 0.0, 1.0, 60.0, 400.0] {
            for dx in [-600.0f32, -40.0, 40.0, 600.0] {
                let (vx, vy) = m.aim_at(500.0 + dx, 400.0 + dy, &mut noise);
                let angle = vy.atan2(vx.abs());
                assert!(
                    angle.abs() >= MUTANT_MIN_ANGLE - 0.001,
                    "fired at {angle:.3} rad for dx={dx} dy={dy} — too flat"
                );
                assert!(vy.abs() > 0.001, "a perfectly level shot got through");
            }
        }
    }

    /// The shot must still go TOWARD the ship — never-straight must not
    /// become never-accurate.
    #[test]
    fn a_mutant_shoots_toward_the_ship() {
        let m = Lander::mutant(500.0, 400.0);
        let mut noise = 0x9999u32;

        let (vx, _) = m.aim_at(900.0, 400.0, &mut noise);
        assert!(vx > 0.0, "should fire east at a ship to the east");

        let (vx, _) = m.aim_at(100.0, 400.0, &mut noise);
        assert!(vx < 0.0, "should fire west at a ship to the west");

        // ⚠️ AND ACROSS THE SEAM. A ship just east of x=0 is WEST of a
        // Mutant near the end of the world, by the short way round.
        let m = Lander::mutant(world::WORLD_W - 50.0, 400.0);
        let (vx, _) = m.aim_at(20.0, 400.0, &mut noise);
        assert!(vx > 0.0, "across the seam it must still fire the short way");
    }

    #[test]
    fn a_mutant_chases_the_ship_and_asks_to_fire() {
        let t = terrain();
        let mut people = Humanoids::new();
        let mut ls = Landers::new();
        ls.spawn(Lander::mutant(500.0, 400.0));

        let ship = Some((900.0f32, 400.0f32));
        let start = world::delta(500.0, 900.0).abs();

        let mut fired = false;
        for _ in 0..600 {
            let wants = ls.step(&t, &mut people, ship, 1.0 / 120.0);
            if !wants.is_empty() {
                fired = true;
            }
        }

        let m = ls.iter().next().unwrap();
        let now = world::delta(m.x, 900.0).abs();
        assert!(now < start, "the Mutant did not close: {start} -> {now}");
        assert!(fired, "a Mutant in range never asked to fire");
    }

    /// ⚠️ A MUTANT MUST NOT TRACK A DEAD SHIP. Converging on the respawn
    /// point while the player cannot move is exactly the death loop the
    /// invulnerability exists to prevent.
    #[test]
    fn a_mutant_does_not_hunt_a_ship_that_is_not_flying() {
        let t = terrain();
        let mut people = Humanoids::new();
        let mut ls = Landers::new();
        ls.spawn(Lander::mutant(500.0, 400.0));

        for _ in 0..600 {
            let wants = ls.step(&t, &mut people, None, 1.0 / 120.0);
            assert!(wants.is_empty(), "it shot at a ship that is not there");
        }
        let m = ls.iter().next().unwrap();
        assert!(
            (m.y - 400.0).abs() < 1.0,
            "it converged vertically on nothing: {}",
            m.y
        );
    }

    /// ★ THE WORLD ENDING: every surviving Lander turns.
    #[test]
    fn when_the_world_ends_every_lander_mutates() {
        let t = terrain();
        let mut ls = Landers::new();
        ls.scatter(4, 0.0, &t, 99);
        assert_eq!(ls.mutants(), 0);

        ls.mutate_all();
        assert_eq!(ls.mutants(), 4, "some Landers survived the apocalypse");
        for l in ls.iter() {
            assert!(l.is_mutant());
            assert!(l.y <= world::VIEW_H * 0.95, "mutated out of reach at {}", l.y);
        }
    }

    #[test]
    fn a_drifting_lander_wraps_with_the_world() {
        let t = terrain();
        let mut ls = Landers::new();
        ls.spawn(Lander::new(5.0, 300.0, -DRIFT_SPEED));
        ls.step(&t, &mut nobody(), None, WARP_SECONDS + 0.01);
        for _ in 0..600 {
            ls.step(&t, &mut nobody(), None, 1.0 / 60.0);
        }
        let l = ls.iter().next().unwrap();
        assert!(l.x >= 0.0 && l.x < world::WORLD_W, "drifted out of the world: {}", l.x);
        assert!(l.x > world::WORLD_W * 0.5, "should have wrapped west, at {}", l.x);
    }
}
