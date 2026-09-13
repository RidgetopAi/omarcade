//! How the ship flies, and how the camera follows it.
//!
//! This file is the game. Everything else in this crate — the terrain,
//! the renderer, eventually the enemies — exists to give the flying
//! somewhere to happen. Brian: "If we can get this we can build
//! everything around it."
//!
//! Two axes that deliberately do NOT behave alike:
//!
//! **Horizontal is momentum.** Thrust accelerates, releasing coasts, and
//! turning around has to be eased through rather than snapped. The ship
//! has weight.
//!
//! **Vertical is direct.** Brian: "vertical goes where you tell it and
//! let off." The ship goes where you push and stops when you stop.
//!
//! The contrast IS the feel. A ship with momentum on both axes is
//! Asteroids in a tube: it floats and it fights you. A ship with
//! momentum on neither is a cursor. Defender put weight on the axis you
//! travel along and left the other one instant, so you can place
//! yourself vertically with total precision while the horizontal carries
//! you — and that is the thing worth reproducing.

use crate::world;

// ---------------------------------------------------------------------
// The numbers.
//
// ⚠️ EVERY ONE IS DERIVED FROM THE WORLD'S ARITHMETIC, NOT PICKED. L015
// is explicit about why: Volley inherited a paddle speed from Breakout
// where it meant something different, and the game was unplayable in a
// way no unit test caught. A constant that cannot be explained cannot be
// tuned, so each of these says what question it answers.
// ---------------------------------------------------------------------

/// Seconds to fly the whole loop at top speed.
///
/// THE ANCHOR CONSTANT: top speed falls out of this, and everything else
/// falls out of top speed. Six seconds for four screens means a screen
/// width every 1.5s, which is the pace contemporary accounts give for
/// Defender — fast enough that the world feels small when you are moving
/// and large when you are looking for something.
pub const LAP_SECONDS: f32 = 6.0;

/// Top horizontal speed, in world units per second.
pub const TOP_SPEED: f32 = world::WORLD_W / LAP_SECONDS;

/// Seconds of held thrust to reach top speed from rest.
///
/// This is the ship's WEIGHT. Much below a second and the momentum is
/// invisible — it may as well be a cursor. Much above and the ship feels
/// like it is asking permission.
pub const SPIN_UP_SECONDS: f32 = 1.0;

/// Horizontal acceleration under thrust.
pub const THRUST: f32 = TOP_SPEED / SPIN_UP_SECONDS;

/// How long coasting takes to halve your speed.
///
/// The glide. Long enough that releasing thrust is a decision with
/// consequences rather than a brake.
pub const COAST_HALF_LIFE: f32 = 1.6;

/// Exponential drag coefficient, per second.
///
/// ⚠️ EXPONENTIAL, NOT LINEAR. Linear drag subtracts a fixed amount per
/// second, which means it takes the same time to shed the last 10 u/s as
/// the first — the ship stops dead at the end of a glide. Decay bleeds
/// off proportionally, so a fast ship loses a lot and a slow one drifts,
/// which is what coasting feels like.
pub const DRAG: f32 = 0.693_147_2 / COAST_HALF_LIFE; // ln 2 / half-life

/// Extra acceleration available when thrusting AGAINST your motion.
///
/// ★ THIS IS THE ONE BRIAN ASKED FOR BY NAME: "eas takeoff if going
/// opposite". Turning around with only [`THRUST`] means flying a long
/// way backwards before you make headway, which reads as the ship
/// ignoring you. This multiplier applies only while thrust opposes
/// velocity, so a reversal bites immediately and then blends into
/// ordinary acceleration as the ship comes through zero.
///
/// ⚠️ It is a MULTIPLIER ON THRUST, not a separate force: keeping it
/// relative means retuning THRUST cannot silently leave the turnaround
/// feeling wrong.
pub const REVERSE_BITE: f32 = 2.2;

/// Vertical speed, in world units per second.
///
/// Direct, so this is a speed and not an acceleration. Derived from the
/// view: crossing the visible height takes about a second and a half,
/// which is quick enough to dodge with and slow enough to aim with.
pub const CLIMB_SPEED: f32 = world::VIEW_H / 1.5;

/// How far above the terrain and below the ceiling the ship may fly.
const FLOOR_CLEARANCE: f32 = 6.0;
const CEILING: f32 = world::VIEW_H * 0.94;

// ---------------------------------------------------------------------
// The camera.
// ---------------------------------------------------------------------

/// How far ahead of the ship the camera sits, as a fraction of the view.
///
/// ⚠️ DEFENDER DOES NOT CENTRE THE SHIP, and this is a large part of why
/// it plays well. The ship sits back in its own direction of travel so
/// most of the screen is the space you are flying INTO. Centred, you get
/// equal warning in a direction you are leaving and the one you are
/// entering, which wastes half the screen.
pub const CAMERA_LEAD: f32 = 0.22;

/// Seconds for the camera to cover most of the distance to where it
/// wants to be.
///
/// ⚠️ THE SLIDE ON REVERSAL. When the ship turns around the lead flips
/// to the other side, and the camera has to travel a chunk of a screen
/// to get there. Snapping is disorienting; this makes it a glide. Slow
/// enough to read as deliberate, fast enough not to lag behind a ship at
/// top speed.
pub const CAMERA_EASE_SECONDS: f32 = 0.42;

/// Which way the ship is pointing. Independent of velocity: a ship can
/// coast east while facing west, and that is exactly what a reversal
/// looks like mid-turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Facing {
    West,
    East,
}

impl Facing {
    /// -1.0 for west, +1.0 for east.
    pub fn sign(self) -> f32 {
        match self {
            Facing::West => -1.0,
            Facing::East => 1.0,
        }
    }
}

/// What the player is asking for this frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct Input {
    /// Thrust held.
    pub thrust: bool,
    /// -1 up, +1 down, 0 neither. Both held cancels, which is what a
    /// player expects from two keys.
    pub vertical: f32,
    /// A reversal request. Defender had a dedicated key for this rather
    /// than steering — you face a direction and thrust that way.
    pub face: Option<Facing>,
}

/// The ship.
#[derive(Debug, Clone)]
pub struct Ship {
    /// World position. x is wrapped; y is measured up from the floor.
    pub x: f32,
    pub y: f32,
    /// Horizontal velocity, signed. Vertical has none — it is direct.
    pub vx: f32,
    pub facing: Facing,
}

impl Ship {
    /// A ship at rest, facing east, at a comfortable altitude.
    pub fn new(x: f32) -> Self {
        Self {
            x: world::wrap(x),
            y: world::VIEW_H * 0.55,
            vx: 0.0,
            facing: Facing::East,
        }
    }

    /// Advance one step.
    ///
    /// `dt` is a fixed timestep from the caller, not a frame duration —
    /// the physics must not change with the frame rate.
    pub fn step(&mut self, input: Input, terrain: &crate::world::Terrain, dt: f32) {
        if let Some(f) = input.face {
            self.facing = f;
        }

        // ---- horizontal: momentum ------------------------------------
        if input.thrust {
            let want = self.facing.sign();

            // ★ THE EASED TURNAROUND. Thrusting against your own motion
            // is worth more than thrusting with it, so a reversal bites
            // at once instead of spending a second going the wrong way.
            // The test is the SIGN of the product: opposing means the
            // ship is still travelling the other way.
            let opposing = self.vx * want < 0.0;
            let a = if opposing { THRUST * REVERSE_BITE } else { THRUST };

            self.vx += want * a * dt;
        }

        // Drag always applies, thrusting or not.
        //
        // ⚠️ `exp(-k dt)` rather than `1 - k*dt`. The linear form is an
        // approximation that only holds for small dt, and it goes
        // NEGATIVE for dt > 1/k — at which point drag reverses the ship.
        // The exact form is correct at any timestep, including the large
        // one a debugger or a stalled frame produces.
        self.vx *= (-DRAG * dt).exp();

        // Clamp after both, so thrust cannot outrun the top speed and
        // drag cannot be what enforces it.
        self.vx = self.vx.clamp(-TOP_SPEED, TOP_SPEED);

        // A ship that is barely moving should be still. Without this it
        // creeps forever, because exponential decay never quite reaches
        // zero, and a "stationary" ship drifts a pixel every few seconds.
        if self.vx.abs() < 0.5 {
            self.vx = 0.0;
        }

        self.x = world::wrap(self.x + self.vx * dt);

        // ---- vertical: direct ----------------------------------------
        //
        // No velocity, no easing: the ship is where you put it. Brian:
        // "vertical goes where you tell it and let off."
        // ⚠️ `y` IS MEASURED UP FROM THE WORLD FLOOR, not down from the
        // top of a screen. `input.vertical` is -1 for "up", following
        // the screen convention every other game in this suite uses for
        // input — so the two disagree by a sign, and this is the one
        // line where that has to be resolved. Subtracting here (the
        // screen-space habit) flew the ship DOWN when the player pressed
        // up, and the first test written against it caught it.
        if input.vertical != 0.0 {
            self.y += -input.vertical * CLIMB_SPEED * dt;
        }

        // Held inside the world. The terrain is fly-THROUGH for now, so
        // this is a soft floor that stops the ship leaving the world
        // rather than a crash — Brian has not committed to collision.
        let floor = terrain.height_at(self.x) + FLOOR_CLEARANCE;
        self.y = self.y.clamp(floor.min(CEILING), CEILING);
    }

    /// How fast, as a fraction of top speed, ignoring direction.
    pub fn speed_fraction(&self) -> f32 {
        (self.vx / TOP_SPEED).abs().min(1.0)
    }
}

/// Where the view is centred, and how it chases the ship.
#[derive(Debug, Clone)]
pub struct Camera {
    /// The world x at the centre of the screen.
    pub x: f32,
}

impl Camera {
    pub fn new(x: f32) -> Self {
        Self { x: world::wrap(x) }
    }

    /// Snap to where the ship wants it, with no glide.
    ///
    /// For a fresh start or a respawn — easing in from wherever the
    /// camera happened to be would read as a swoop nobody asked for.
    pub fn snap_to(&mut self, ship: &Ship) {
        self.x = world::wrap(Self::target_for(ship));
    }

    /// Ease toward the ship's lead position.
    pub fn follow(&mut self, ship: &Ship, dt: f32) {
        let target = Self::target_for(ship);

        // ⚠️ MEASURE THE GAP WITH `delta`, NOT SUBTRACTION. Near the
        // seam the raw difference is nearly a whole world, and the
        // camera would take the long way round — a full reverse sweep
        // of the entire level every time the player crosses zero.
        let gap = world::delta(self.x, target);

        // Frame-rate independent easing. `1 - exp(-k dt)` covers the
        // same FRACTION of the remaining distance per unit time at any
        // timestep; the naive `gap * 0.1` per frame does not, and the
        // camera would be twice as fast at 120fps.
        let k = 1.0 / CAMERA_EASE_SECONDS;
        let t = 1.0 - (-k * dt).exp();

        self.x = world::wrap(self.x + gap * t);
    }

    /// Where the camera would sit if it had already caught up.
    ///
    /// The lead is in the direction the ship FACES, not the direction it
    /// is moving. Facing is what the player just asked for, so the
    /// camera starts its slide the instant they hit reverse rather than
    /// waiting for the ship to actually come around.
    fn target_for(ship: &Ship) -> f32 {
        ship.x + ship.facing.sign() * world::VIEW_W * CAMERA_LEAD
    }

    /// World x at the left edge of the screen.
    pub fn left(&self) -> f32 {
        self.x - world::VIEW_W * 0.5
    }

    /// Screen x for a world x, accounting for the wrap.
    ///
    /// Returns a value that may be off-screen; the renderer clips.
    pub fn to_screen(&self, world_x: f32) -> f32 {
        world::VIEW_W * 0.5 + world::delta(self.x, world_x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Terrain;

    const DT: f32 = 1.0 / 240.0;

    fn flat() -> Terrain {
        Terrain::generate(64, 1)
    }

    fn run(ship: &mut Ship, input: Input, seconds: f32, t: &Terrain) {
        let steps = (seconds / DT).round() as usize;
        for _ in 0..steps {
            ship.step(input, t, DT);
        }
    }

    #[test]
    fn thrust_reaches_top_speed_in_about_the_spin_up_time() {
        let t = flat();
        let mut s = Ship::new(0.0);
        run(&mut s, Input { thrust: true, ..Default::default() }, SPIN_UP_SECONDS, &t);

        // Drag is fighting the whole way, so it will not be exactly top
        // speed — but it must be most of the way there, or SPIN_UP_SECONDS
        // is describing something that does not happen.
        let f = s.speed_fraction();
        assert!(f > 0.55, "after {SPIN_UP_SECONDS}s of thrust the ship is only at {f:.2} of top speed");
        assert!(s.vx > 0.0, "facing east, it should be going east");
    }

    #[test]
    fn thrust_never_exceeds_top_speed() {
        let t = flat();
        let mut s = Ship::new(0.0);
        run(&mut s, Input { thrust: true, ..Default::default() }, 30.0, &t);
        assert!(s.vx <= TOP_SPEED + 0.001, "{} exceeds {TOP_SPEED}", s.vx);
    }

    #[test]
    fn releasing_thrust_coasts_rather_than_stopping() {
        let t = flat();
        let mut s = Ship::new(0.0);
        run(&mut s, Input { thrust: true, ..Default::default() }, 3.0, &t);
        let flying = s.vx;

        // One half-life of coasting should leave about half the speed.
        run(&mut s, Input::default(), COAST_HALF_LIFE, &t);
        let ratio = s.vx / flying;
        assert!(
            (0.4..0.6).contains(&ratio),
            "after one half-life the ship holds {ratio:.2} of its speed, not about half"
        );
        assert!(s.vx > 0.0, "it should still be gliding, not stopped");
    }

    #[test]
    fn a_coasting_ship_eventually_comes_to_rest() {
        let t = flat();
        let mut s = Ship::new(0.0);
        run(&mut s, Input { thrust: true, ..Default::default() }, 3.0, &t);
        run(&mut s, Input::default(), 20.0, &t);
        assert_eq!(s.vx, 0.0, "exponential decay must be floored, or the ship creeps forever");
    }

    /// ★ THE ONE BRIAN ASKED FOR: "eas takeoff if going opposite".
    #[test]
    fn reversing_bites_sooner_than_accelerating_from_rest() {
        let t = flat();

        // A ship at top speed east, told to go west.
        let mut turning = Ship::new(0.0);
        run(&mut turning, Input { thrust: true, ..Default::default() }, 4.0, &t);
        let entry_speed = turning.vx;
        assert!(entry_speed > TOP_SPEED * 0.9, "set up: should be near top speed");

        // How long until it is actually moving west?
        let mut elapsed = 0.0;
        let reverse = Input { thrust: true, face: Some(Facing::West), ..Default::default() };
        while turning.vx > 0.0 && elapsed < 5.0 {
            turning.step(reverse, &t, DT);
            elapsed += DT;
        }

        assert!(turning.vx <= 0.0, "the ship never came around in 5s");

        // Without the bite this would take TOP_SPEED / THRUST = SPIN_UP
        // seconds just to reach zero. With it, meaningfully less.
        let unaided = entry_speed / THRUST;
        assert!(
            elapsed < unaided * 0.75,
            "turnaround took {elapsed:.2}s; plain thrust alone would take {unaided:.2}s — \
             the reverse bite is not doing anything"
        );
    }

    #[test]
    fn the_bite_only_applies_against_the_motion() {
        let t = flat();
        // Thrusting east from rest, then continuing east, must not get
        // the bonus — otherwise the ship accelerates at REVERSE_BITE
        // times thrust all the time and TOP_SPEED arrives far too early.
        let mut s = Ship::new(0.0);
        let east = Input { thrust: true, face: Some(Facing::East), ..Default::default() };
        s.step(east, &t, DT);
        let first = s.vx;
        // One step of plain thrust, less drag. Compare against the
        // unaided figure rather than the boosted one.
        let expected = THRUST * DT;
        assert!(
            (first - expected).abs() < expected * 0.05,
            "first step gained {first}, expected about {expected} — the bite is firing when it should not"
        );
    }

    #[test]
    fn vertical_is_direct_and_stops_dead() {
        let t = flat();
        let mut s = Ship::new(0.0);
        let start = s.y;

        // ⚠️ `vertical: -1.0` IS "UP" (the screen convention the input
        // layer uses) but `y` MEASURES UP FROM THE FLOOR, so climbing
        // makes y LARGER. The first version of this assertion subtracted
        // the other way round out of screen-space habit and reported
        // "moved -240, expected 240" — the physics was right and the
        // test was reading the world upside down. Worth the comment,
        // because the same confusion is one sign away in every file that
        // touches altitude.
        run(&mut s, Input { vertical: -1.0, ..Default::default() }, 0.5, &t);
        let climbed = s.y - start;
        assert!(
            (climbed - CLIMB_SPEED * 0.5).abs() < CLIMB_SPEED * 0.05,
            "half a second of climb moved {climbed}, expected about {}",
            CLIMB_SPEED * 0.5
        );

        // ⚠️ AND IT STOPS. No coasting, no easing — that is the whole
        // point of the axis being direct.
        let held = s.y;
        run(&mut s, Input::default(), 1.0, &t);
        assert_eq!(s.y, held, "vertical must not drift after the key is released");
    }

    #[test]
    fn the_ship_stays_inside_the_world() {
        let t = flat();
        let mut s = Ship::new(0.0);
        run(&mut s, Input { vertical: -1.0, ..Default::default() }, 10.0, &t);
        assert!(s.y <= CEILING + 0.001, "climbed through the ceiling to {}", s.y);

        run(&mut s, Input { vertical: 1.0, ..Default::default() }, 10.0, &t);
        let floor = t.height_at(s.x);
        assert!(s.y >= floor, "sank below the ridge at {} (ridge is {floor})", s.y);
    }

    #[test]
    fn flying_west_from_zero_wraps_to_the_far_end() {
        let t = flat();
        let mut s = Ship::new(5.0);
        run(&mut s, Input { thrust: true, face: Some(Facing::West), ..Default::default() }, 2.0, &t);
        assert!(
            s.x > world::WORLD_W * 0.5,
            "flying west past zero should arrive at the east end, not a negative x: {}",
            s.x
        );
    }

    #[test]
    fn the_camera_leads_in_the_direction_the_ship_faces() {
        let mut s = Ship::new(world::WORLD_W * 0.5);
        let mut c = Camera::new(s.x);
        c.snap_to(&s);
        assert!(
            world::delta(s.x, c.x) > 0.0,
            "facing east, the camera should sit east of the ship"
        );

        s.facing = Facing::West;
        c.snap_to(&s);
        assert!(
            world::delta(s.x, c.x) < 0.0,
            "facing west, the camera should sit west of the ship"
        );
    }

    /// ⚠️ THE BUG THAT WOULD ONLY APPEAR AT THE SEAM, and would be a
    /// whole-level sweep when it did.
    #[test]
    fn the_camera_never_takes_the_long_way_round() {
        let t = flat();
        let mut s = Ship::new(20.0);
        let mut c = Camera::new(20.0);
        c.snap_to(&s);

        // Fly west across zero, easing the camera every step, and track
        // how far it moves in any single step.
        let west = Input { thrust: true, face: Some(Facing::West), ..Default::default() };
        let mut worst: f32 = 0.0;
        for _ in 0..(3.0 / DT) as usize {
            s.step(west, &t, DT);
            let before = c.x;
            c.follow(&s, DT);
            worst = worst.max(world::delta(before, c.x).abs());
        }

        // A step that moves the camera more than a screen means it went
        // round the world rather than across the seam.
        assert!(
            worst < world::VIEW_W,
            "the camera jumped {worst:.0} units in one step — it took the long way round the seam"
        );
    }

    #[test]
    fn the_camera_catches_up_but_does_not_snap() {
        let t = flat();
        let mut s = Ship::new(0.0);
        run(&mut s, Input { thrust: true, ..Default::default() }, 3.0, &t);

        // Camera starts on the ship, which is not where it wants to be.
        let mut c = Camera::new(s.x);
        let gap_before = world::delta(c.x, Camera::target_for(&s)).abs();
        c.follow(&s, DT);
        let gap_after = world::delta(c.x, Camera::target_for(&s)).abs();

        assert!(gap_after < gap_before, "the camera must close the gap");
        assert!(
            gap_after > gap_before * 0.5,
            "one 240Hz step closed more than half the gap — that is a snap, not a glide"
        );
    }

    #[test]
    fn physics_does_not_depend_on_the_timestep() {
        // ⚠️ The whole reason drag and camera easing are exponential.
        // Run the same two seconds at two rates and land in the same place.
        let t = flat();
        let thrust = Input { thrust: true, ..Default::default() };

        let mut fast = Ship::new(0.0);
        for _ in 0..(2.0 / (1.0 / 480.0)) as usize {
            fast.step(thrust, &t, 1.0 / 480.0);
        }

        let mut slow = Ship::new(0.0);
        for _ in 0..(2.0 / (1.0 / 60.0)) as usize {
            slow.step(thrust, &t, 1.0 / 60.0);
        }

        let drift = (fast.vx - slow.vx).abs() / TOP_SPEED;
        assert!(drift < 0.02, "480Hz and 60Hz disagree by {:.1}% of top speed", drift * 100.0);
    }
}
