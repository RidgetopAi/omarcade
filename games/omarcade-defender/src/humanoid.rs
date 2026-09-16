//! The people on the surface.
//!
//! ★ THESE ARE WHY THE GAME IS CALLED DEFENDER. Without them, killing a
//! Lander is target practice: the enemy takes nothing from you, so there
//! is nothing to defend and no reason to fly anywhere in particular.
//! With them, every Lander you ignore is a person being carried away.
//!
//! ⚠️ YOUR OWN LASER CAN KILL THEM, AND THAT IS DELIBERATE. Brian's spec
//! says DON'T SHOOT THEM, and a rule the game quietly enforces is not a
//! rule the player ever feels. The arcade let you do it, and the moment
//! you did it by accident was the moment you learned to aim.

use crate::world::{self, Terrain};

/// How wide a Humanoid is for hit testing, in world units.
///
/// ★ FROM THE ART, like every other hitbox here: 5 x 11.25 units at
/// [`crate::art::SCALE`]. Widened slightly in x because a figure this
/// narrow is otherwise almost impossible to hit deliberately — and
/// because catching one is meant to feel generous.
pub const HALF_W: f32 = 2.5 * crate::art::SCALE;
pub const HALF_H: f32 = 5.625 * crate::art::SCALE;

/// How fast a Humanoid walks, in world units per second.
///
/// Slow enough to read as walking rather than fleeing. They do not know
/// they are in danger, which is most of the pathos of the thing.
pub const WALK_SPEED: f32 = 18.0;

/// How long, on average, before one turns around.
const TURN_SECONDS: f32 = 3.2;

/// How fast a dropped Humanoid falls, in world units per second squared.
pub const FALL_GRAVITY: f32 = 620.0;

/// The terminal speed of a fall, so a long drop does not become
/// uncatchable.
pub const FALL_TERMINAL: f32 = 420.0;

/// How far a Humanoid can fall and survive, in world units.
///
/// ⚠️ MEASURED FROM WHERE THE FALL STARTED, not from an absolute height.
/// The surface is mountains — an absolute threshold would make a drop
/// into a valley lethal and the same drop onto a peak survivable, which
/// is not a rule a player could ever learn.
///
/// About a third of the screen. The arcade is forgiving of a short drop
/// and fatal from altitude, and the forgiving part is what makes
/// catching feel like a rescue rather than a formality.
pub const SURVIVABLE_FALL: f32 = world::VIEW_H / 3.0;

/// How close the ship must be to catch a falling Humanoid.
///
/// ★ GENEROUS ON PURPOSE. The fun is the rescue, not the precision, and
/// a catch box that demands pixel accuracy converts a heroic moment into
/// a fumble.
pub const CATCH_HALF_W: f32 = 46.0;
pub const CATCH_HALF_H: f32 = 40.0;

/// Where a rescued Humanoid rides, below the ship.
///
/// Enough clearance that the passenger reads as being carried rather
/// than as overlapping the hull — seen in a frame, not reasoned out.
pub const CARRY_OFFSET_Y: f32 = 34.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// On the surface, walking. Can be grabbed, and can be shot.
    Walking,
    /// Held by a Lander and being carried upward.
    Carried,
    /// Dropped, and falling. Can be caught.
    Falling,
    /// Riding under the ship after a catch.
    Rescued,
    /// Gone: shot, or fell too far.
    Dead,
}

#[derive(Debug, Clone, Copy)]
pub struct Humanoid {
    pub x: f32,
    pub y: f32,
    pub state: State,
    /// Walk direction, signed. Also used to face the art.
    pub vx: f32,
    /// Downward speed while falling, positive means falling.
    pub fall_speed: f32,
    /// The height a fall began at, so survival is judged by DISTANCE.
    pub fell_from: f32,
    /// Seconds until the next turn while walking.
    turn_in: f32,
}

impl Humanoid {
    pub fn new(x: f32, y: f32, vx: f32) -> Self {
        Self {
            x: world::wrap(x),
            y,
            state: State::Walking,
            vx,
            fall_speed: 0.0,
            fell_from: 0.0,
            turn_in: TURN_SECONDS,
        }
    }

    /// Can a Lander take this one?
    pub fn is_grabbable(&self) -> bool {
        self.state == State::Walking
    }

    /// Is this one still part of the world's population?
    ///
    /// ★ A RESCUED HUMANOID STILL COUNTS. It is riding under the ship,
    /// not saved-and-filed — the loss condition (S7) asks how many
    /// people are left, and a passenger is still a person.
    pub fn is_alive(&self) -> bool {
        self.state != State::Dead
    }

    /// Can your laser hit this one right now?
    ///
    /// Not while it rides under your own ship — otherwise firing forward
    /// from a rescue would shoot your own passenger, which is a cruelty
    /// the arcade did not commit either.
    pub fn is_shootable(&self) -> bool {
        matches!(self.state, State::Walking | State::Carried | State::Falling)
    }

    /// Taken by a Lander.
    pub fn grabbed(&mut self) {
        self.state = State::Carried;
    }

    /// Released — because the carrier died, or it was let go.
    pub fn dropped(&mut self) {
        if self.state == State::Carried {
            self.state = State::Falling;
            self.fall_speed = 0.0;
            self.fell_from = self.y;
        }
    }

    /// Caught by the ship.
    pub fn rescued(&mut self) {
        if self.state == State::Falling {
            self.state = State::Rescued;
            self.fall_speed = 0.0;
        }
    }

    /// Put back down on the surface after a rescue.
    pub fn released_to_ground(&mut self, terrain: &Terrain) {
        if self.state == State::Rescued {
            self.state = State::Walking;
            self.y = terrain.height_at(self.x);
        }
    }

    pub fn kill(&mut self) {
        self.state = State::Dead;
    }

    fn step(&mut self, terrain: &Terrain, dt: f32, seed: &mut u32) {
        match self.state {
            State::Walking => {
                self.x = world::wrap(self.x + self.vx * dt);
                // Stand ON the ground, which moves under them.
                self.y = terrain.height_at(self.x);

                self.turn_in -= dt;
                if self.turn_in <= 0.0 {
                    self.vx = -self.vx;
                    self.turn_in = TURN_SECONDS * (0.5 + next01(seed));
                }
            }
            State::Falling => {
                self.fall_speed = (self.fall_speed + FALL_GRAVITY * dt).min(FALL_TERMINAL);
                self.y -= self.fall_speed * dt;

                let ground = terrain.height_at(self.x);
                if self.y <= ground {
                    self.y = ground;
                    // ⚠️ SURVIVAL IS JUDGED BY HOW FAR IT FELL, not by
                    // how fast it was going or where it landed.
                    if self.fell_from - ground > SURVIVABLE_FALL {
                        self.state = State::Dead;
                    } else {
                        self.state = State::Walking;
                        self.fall_speed = 0.0;
                    }
                }
            }
            // Carried and Rescued are both driven by whoever holds them.
            State::Carried | State::Rescued | State::Dead => {}
        }
    }
}

/// Everyone on the surface.
#[derive(Debug, Default)]
pub struct Humanoids {
    live: Vec<Humanoid>,
    seed: u32,
}

impl Humanoids {
    pub fn new() -> Self {
        Self { live: Vec::new(), seed: 0x7A11_1CE5 }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Humanoid> {
        self.live.iter()
    }

    pub fn get(&self, index: usize) -> Option<&Humanoid> {
        self.live.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut Humanoid> {
        self.live.get_mut(index)
    }

    pub fn len(&self) -> usize {
        self.live.len()
    }

    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }

    /// How many people are left — the number the loss condition watches.
    pub fn alive(&self) -> usize {
        self.live.iter().filter(|h| h.is_alive()).count()
    }

    pub fn spawn(&mut self, h: Humanoid) {
        self.live.push(h);
    }

    pub fn clear(&mut self) {
        self.live.clear();
    }

    /// Put `count` Humanoids on the surface, spread across the world.
    pub fn scatter(&mut self, count: usize, terrain: &Terrain, seed: u32) {
        let spacing = world::WORLD_W / count.max(1) as f32;
        let mut s = seed | 1;
        for i in 0..count {
            let jitter = next01(&mut s);
            let x = world::wrap(spacing * (i as f32 + 0.15 + jitter * 0.7));
            let dir = if next01(&mut s) < 0.5 { -1.0 } else { 1.0 };
            self.spawn(Humanoid::new(x, terrain.height_at(x), WALK_SPEED * dir));
        }
    }

    pub fn step(&mut self, terrain: &Terrain, dt: f32) {
        let mut seed = self.seed;
        for h in &mut self.live {
            h.step(terrain, dt, &mut seed);
        }
        self.seed = seed;
        // ⚠️ THE DEAD ARE NOT REMOVED HERE. S7's loss condition counts
        // the population, and a Vec that silently shrinks would make
        // "how many did I lose" unanswerable. They are simply skipped.
    }

    /// The nearest grabbable Humanoid to `x`, with its index.
    ///
    /// ⚠️ DISTANCE THROUGH [`world::delta`]. A Lander at x = 5 and a
    /// Humanoid at WORLD_W - 5 are ten units apart; raw subtraction says
    /// nearly four screens, so every Lander near the seam would ignore
    /// the person standing next to it and fly the long way round.
    pub fn nearest_grabbable(&self, x: f32) -> Option<usize> {
        self.live
            .iter()
            .enumerate()
            .filter(|(_, h)| h.is_grabbable())
            .min_by(|(_, a), (_, b)| {
                let da = world::delta(x, a.x).abs();
                let db = world::delta(x, b.x).abs();
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
    }

    /// The first shootable Humanoid overlapping `(x, y)`.
    pub fn hit_test(&self, x: f32, y: f32) -> Option<usize> {
        self.live.iter().position(|h| {
            h.is_shootable()
                && world::delta(h.x, x).abs() <= HALF_W
                && (h.y - y).abs() <= HALF_H
        })
    }

    /// The first falling Humanoid the ship at `(x, y)` would catch.
    pub fn catch_test(&self, x: f32, y: f32) -> Option<usize> {
        self.live.iter().position(|h| {
            h.state == State::Falling
                && world::delta(h.x, x).abs() <= CATCH_HALF_W
                && (h.y - y).abs() <= CATCH_HALF_H
        })
    }

    /// Move every rescued Humanoid along with the ship.
    pub fn carry_with_ship(&mut self, ship_x: f32, ship_y: f32) {
        for h in &mut self.live {
            if h.state == State::Rescued {
                h.x = world::wrap(ship_x);
                h.y = ship_y - CARRY_OFFSET_Y;
            }
        }
    }
}

/// A deterministic 0..1, advancing the seed.
fn next01(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terrain() -> Terrain {
        Terrain::generate(256, 0x0DEF_E4DE)
    }

    #[test]
    fn humanoids_walk_along_the_ground() {
        let t = terrain();
        let mut hs = Humanoids::new();
        hs.spawn(Humanoid::new(500.0, 0.0, WALK_SPEED));
        hs.step(&t, 0.5);
        let h = hs.iter().next().unwrap();
        assert!(h.x > 500.0, "should have walked east");
        assert!(
            (h.y - t.height_at(h.x)).abs() < 0.01,
            "must stand on the ground, not float: {} vs {}",
            h.y,
            t.height_at(h.x)
        );
    }

    #[test]
    fn a_walker_turns_around_eventually() {
        let t = terrain();
        let mut hs = Humanoids::new();
        hs.spawn(Humanoid::new(500.0, 0.0, WALK_SPEED));
        let start = hs.iter().next().unwrap().vx;
        for _ in 0..600 {
            hs.step(&t, 1.0 / 60.0);
        }
        // Ten seconds is several turn intervals; it must have flipped at
        // least once, which we detect by having seen the other sign.
        let mut seen_other = false;
        for _ in 0..600 {
            hs.step(&t, 1.0 / 60.0);
            if hs.iter().next().unwrap().vx.signum() != start.signum() {
                seen_other = true;
                break;
            }
        }
        assert!(seen_other, "a walker never turned around");
    }

    /// ★ THE STAGE'S WHOLE POINT, AS A TEST: a carried Humanoid that is
    /// released falls, and can be caught.
    #[test]
    fn a_dropped_humanoid_falls_and_can_be_caught() {
        let t = terrain();
        let mut hs = Humanoids::new();
        let ground = t.height_at(500.0);
        hs.spawn(Humanoid::new(500.0, ground + 200.0, 0.0));
        hs.get_mut(0).unwrap().grabbed();
        hs.get_mut(0).unwrap().dropped();
        assert_eq!(hs.get(0).unwrap().state, State::Falling);

        let before = hs.get(0).unwrap().y;
        hs.step(&t, 0.1);
        assert!(hs.get(0).unwrap().y < before, "it did not fall");

        let (hx, hy) = (hs.get(0).unwrap().x, hs.get(0).unwrap().y);
        let caught = hs.catch_test(hx, hy).expect("the ship is right on it");
        hs.get_mut(caught).unwrap().rescued();
        assert_eq!(hs.get(0).unwrap().state, State::Rescued);
    }

    /// ⚠️ SURVIVAL IS BY DISTANCE FALLEN. A short drop is survivable and
    /// a long one is not, and neither answer may depend on whether the
    /// landing happened to be on a peak or in a valley.
    #[test]
    fn a_short_fall_is_survived_and_a_long_one_is_not() {
        let t = terrain();

        let mut hs = Humanoids::new();
        let ground = t.height_at(500.0);
        hs.spawn(Humanoid::new(500.0, ground + SURVIVABLE_FALL * 0.5, 0.0));
        hs.get_mut(0).unwrap().grabbed();
        hs.get_mut(0).unwrap().dropped();
        for _ in 0..600 {
            hs.step(&t, 1.0 / 120.0);
        }
        assert_eq!(
            hs.get(0).unwrap().state,
            State::Walking,
            "a short fall must be survivable"
        );

        let mut hs = Humanoids::new();
        hs.spawn(Humanoid::new(500.0, ground + SURVIVABLE_FALL * 2.5, 0.0));
        hs.get_mut(0).unwrap().grabbed();
        hs.get_mut(0).unwrap().dropped();
        for _ in 0..900 {
            hs.step(&t, 1.0 / 120.0);
        }
        assert_eq!(hs.get(0).unwrap().state, State::Dead, "a long fall must kill");
    }

    /// The same drop must give the same answer wherever in the world it
    /// happens — the reason survival is measured from the start of the
    /// fall rather than against an absolute height.
    #[test]
    fn survival_does_not_depend_on_the_terrain_underneath() {
        let t = terrain();
        // Find a high sample and a low one.
        let (mut lo_x, mut hi_x) = (0.0f32, 0.0f32);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for i in 0..400 {
            let x = i as f32 * (world::WORLD_W / 400.0);
            let h = t.height_at(x);
            if h < lo {
                lo = h;
                lo_x = x;
            }
            if h > hi {
                hi = h;
                hi_x = x;
            }
        }
        assert!(hi - lo > 40.0, "the terrain is too flat to test this");

        for x in [lo_x, hi_x] {
            let mut hs = Humanoids::new();
            hs.spawn(Humanoid::new(x, t.height_at(x) + SURVIVABLE_FALL * 0.5, 0.0));
            hs.get_mut(0).unwrap().grabbed();
            hs.get_mut(0).unwrap().dropped();
            for _ in 0..900 {
                hs.step(&t, 1.0 / 120.0);
            }
            assert_eq!(
                hs.get(0).unwrap().state,
                State::Walking,
                "the same short fall must survive at x={x}"
            );
        }
    }

    /// ⚠️ YOU CAN SHOOT THEM. The rule "don't shoot the humanoids" is
    /// only a rule if the game lets you break it.
    #[test]
    fn your_own_laser_can_kill_a_humanoid() {
        let t = terrain();
        let mut hs = Humanoids::new();
        hs.spawn(Humanoid::new(500.0, t.height_at(500.0), 0.0));
        let h = *hs.get(0).unwrap();
        let hit = hs.hit_test(h.x, h.y).expect("a walker must be shootable");
        hs.get_mut(hit).unwrap().kill();
        assert_eq!(hs.alive(), 0);
    }

    /// ...but not the one riding under your own ship.
    #[test]
    fn a_rescued_passenger_cannot_be_shot_by_its_rescuer() {
        let t = terrain();
        let mut hs = Humanoids::new();
        hs.spawn(Humanoid::new(500.0, t.height_at(500.0) + 300.0, 0.0));
        hs.get_mut(0).unwrap().grabbed();
        hs.get_mut(0).unwrap().dropped();
        hs.get_mut(0).unwrap().rescued();

        let h = *hs.get(0).unwrap();
        assert!(hs.hit_test(h.x, h.y).is_none(), "shot my own passenger");
    }

    /// ⚠️ THE SEAM. A Lander at the very start of the world must notice
    /// the Humanoid a few units west of it, which is at the very end.
    #[test]
    fn the_nearest_humanoid_is_found_across_the_seam() {
        let t = terrain();
        let mut hs = Humanoids::new();
        // One very close, across the seam; one far away but with a
        // smaller raw coordinate difference.
        hs.spawn(Humanoid::new(world::WORLD_W - 12.0, t.height_at(0.0), 0.0));
        hs.spawn(Humanoid::new(900.0, t.height_at(900.0), 0.0));

        let near = hs.nearest_grabbable(4.0).expect("something must be nearest");
        assert_eq!(near, 0, "it picked the far one, so delta was not used");
    }

    #[test]
    fn a_rescued_humanoid_rides_with_the_ship() {
        let mut hs = Humanoids::new();
        hs.spawn(Humanoid::new(100.0, 500.0, 0.0));
        hs.get_mut(0).unwrap().grabbed();
        hs.get_mut(0).unwrap().dropped();
        hs.get_mut(0).unwrap().rescued();

        hs.carry_with_ship(742.0, 400.0);
        let h = hs.get(0).unwrap();
        assert!((h.x - 742.0).abs() < 0.01, "did not follow the ship in x");
        assert!((h.y - (400.0 - CARRY_OFFSET_Y)).abs() < 0.01, "wrong carry height");
    }

    #[test]
    fn a_scattered_population_stands_on_the_ground_and_is_deterministic() {
        let t = terrain();
        let mut a = Humanoids::new();
        let mut b = Humanoids::new();
        a.scatter(6, &t, 4242);
        b.scatter(6, &t, 4242);
        assert_eq!(a.len(), 6);
        assert_eq!(a.alive(), 6);

        for h in a.iter() {
            assert!((h.y - t.height_at(h.x)).abs() < 0.01, "floating at {}", h.x);
            assert!(h.x >= 0.0 && h.x < world::WORLD_W);
        }
        let xa: Vec<f32> = a.iter().map(|h| h.x).collect();
        let xb: Vec<f32> = b.iter().map(|h| h.x).collect();
        assert_eq!(xa, xb, "the same seed must give the same people");
    }

    /// The dead stay in the list so the population can be counted, but
    /// they are not alive and cannot be grabbed.
    #[test]
    fn the_dead_are_counted_out_but_not_removed() {
        let t = terrain();
        let mut hs = Humanoids::new();
        hs.scatter(3, &t, 7);
        hs.get_mut(1).unwrap().kill();

        assert_eq!(hs.len(), 3, "the list must not silently shrink");
        assert_eq!(hs.alive(), 2);
        hs.step(&t, 0.1);
        assert_eq!(hs.len(), 3, "still three, one of them dead");
    }
}
