//! Warden, Omarcade's fourth title: a horizontally scrolling shooter in
//! the line of Williams' Defender (1981).
//!
//! ★ STAGE ONE built the world and the flying, and Brian confirmed it:
//! "wow, you kind of nailed that. I honestly can't find anyhing wrong."
//! Those constants are settled and are not retuned here.
//!
//! ★ S5 IS THE STAGE THAT MAKES IT A GAME: you can fire, there are
//! Landers in the world, and they can be destroyed — with particles and
//! with sound.
//!
//! ★ S6 GAVE IT STAKES: there are people on the surface, and the Landers
//! come for them. A Lander drifts, picks the nearest Humanoid, drops on
//! it, and carries it upward — and you can shoot the carrier and catch
//! the person as they fall.
//!
//! ★★ S7 MADE IT POSSIBLE TO LOSE. A Humanoid carried off the top fuses
//! with its captor into a MUTANT, which does not want your civilians —
//! it wants you, and it never shoots straight. You have three lives.
//! And if every person is taken, THE WORLD EXPLODES: the mountains are
//! gone and every surviving Lander mutates. The difficulty does not fall
//! when there is nothing left to protect; it spikes.
//!
//! ★ S8 added the scanner: the whole world in a strip, with the view box.
//!
//! ⚠️ STILL MISSING (docs/warden-plan.md): waves, scoring and the score
//! file (W1); smart bomb and hyperspace (W2); Baiters, Bombers, Pods and
//! Swarmers (W3, W4); the sound pass (W5); the title screen and the
//! cabinet registration in packaging/install.sh (W6). Until W6 the game
//! is deliberately absent from the cabinet — an unfinished title has no
//! business in front of anyone who installed the suite.

mod art;
mod effects;
mod enemy;
mod flight;
mod humanoid;
mod lives;
mod mine;
mod popup;
mod render;
mod scanner;
mod shot;
mod sound;
mod waves;
mod world;

use omarcade_core::audio::{SoundId, VoiceId, VoiceParams};
use omarcade_core::backend::winit_soft::{Idle, WinitBackend};
use omarcade_core::scores::ScoreFile;
use omarcade_core::{Audio, AudioSystem, Backend, Canvas, Game, InputEvent, Key, Pause, Theme};

use effects::Effects;
use enemy::Enemies;
use flight::{Camera, Facing, Input, Ship};
use humanoid::Humanoids;
use lives::Lives;
use mine::Mines;
use popup::Popups;
use shot::Shots;
use waves::{Census, Director, Event};
use world::Terrain;

/// Names the score file, and the cabinet discovers games by it — so it
/// is public surface: renaming it orphans everyone's high scores.
const GAME_ID: &str = "omarcade-warden";
const GAME_NAME: &str = "Warden";
const TITLE: &str = GAME_NAME;
const WIDTH: u32 = world::VIEW_W as u32;
const HEIGHT: u32 = world::VIEW_H as u32;

/// How many ridge samples across the whole world.
///
/// One sample every ~15 world units at four screens wide: fine enough
/// that interpolation is invisible, coarse enough that the ridge has
/// shape rather than noise.
const RIDGE_SAMPLES: usize = 1024;

/// The physics timestep.
///
/// ⚠️ FIXED, AND THE SAME 240Hz THE OTHER THREE USE. Physics that runs
/// on the frame duration changes behaviour with the frame rate, and the
/// reversal timing in `flight` is exactly the kind of thing that would
/// drift between a 60Hz and a 144Hz machine.
const FIXED_DT: f32 = 1.0 / 240.0;

/// How many people live on the surface.
///
/// Defender's own count. Enough that losing one hurts without ending the
/// game, which is what makes the middle of a wave tense rather than
/// merely lost.
const POPULATION: usize = 10;

/// Where the gun sits relative to the ship's origin, in world units.
///
/// ⚠️ THE ART DECIDES THIS, NOT A GUESS. The ship's gun reaches to about
/// +20 units in art space at [`art::SCALE`], so a bolt leaving from the
/// centre would appear to emerge from the cockpit. Mirrored with facing,
/// because the ship is mirrored with facing.
const MUZZLE_FORWARD: f32 = 20.0 * art::SCALE;

/// How low the ship must fly to put a rescued Humanoid back down.
///
/// ★ THE RISK/REWARD THE ARCADE IS BUILT ON. You are safest high up, and
/// the only way to return someone is to go low — so a rescue is not
/// finished at the catch, it is finished at the delivery.
const DROP_OFF_HEIGHT: f32 = 90.0;

/// How fast the hull flare comes up when thrust is pressed, per second.
///
/// Fast: an engine that took a visible moment to light would feel like
/// input lag on the one control the whole game is about.
const EXHAUST_ATTACK: f32 = 14.0;

/// How fast the hull flare dies when thrust is released, per second.
///
/// ★ SLOWER THAN THE ATTACK, and that asymmetry is the point — fire
/// catches instantly and dies away. Equal rates read as a light switch.
const EXHAUST_RELEASE: f32 = 6.0;

/// The ship's hitbox, in world units.
///
/// ★ FROM THE ART, and deliberately TIGHTER than it. The ship draws 70
/// wide, and a hitbox that matched would make the long tail count as a
/// hit — a shot that visibly misses the cockpit but clips the fin reads
/// as unfair, and the arcade was generous here too.
const SHIP_HALF_W: f32 = 22.0;
const SHIP_HALF_H: f32 = 10.0;

/// Smart bombs at the start of a game (the original's default; W2 makes
/// them do something).
const STARTING_SMART_BOMBS: u32 = 3;

/// A ship and a smart bomb every this many points.
const AWARD_EVERY: u32 = 10_000;

/// Catching a falling Humanoid in the air.
const CATCH_POINTS: u32 = 500;
/// Setting a caught Humanoid back down — a whole rescue is 1000.
const SET_DOWN_POINTS: u32 = 500;
/// A Humanoid that survives its own short fall.
const SAFE_LANDING_POINTS: u32 = 250;

/// How long hyperspace takes to put the ship back together: the
/// original's 40 frames.
const HYPERSPACE_SECONDS: f32 = 40.0 / 60.0;

/// The chance of exploding on arrival from hyperspace.
///
/// ★ THE ORIGINAL'S OWN ODDS: a random byte over 192 kills you, 64 in
/// 256. Brian, asked whether to keep it: "keep the hyperspace death
/// chance". It is what makes hyperspace a gamble rather than a free
/// escape.
const HYPERSPACE_DEATH: f32 = 64.0 / 256.0;

/// How long the smart bomb's flashes run: 4 white flashes, each 2 frames
/// on and 2 off (the original inverts the background 8 times at 2-frame
/// intervals).
const BOMB_FLASH_SECONDS: f32 = 16.0 / 60.0;
const BOMB_FLASH_PERIOD: f32 = 4.0 / 60.0;

/// How long the smart bomb's shockwave takes to sweep out past the edges
/// of the screen.
const BLAST_SECONDS: f32 = 0.55;

/// A smart bomb going off: where (world), and how long ago.
#[derive(Debug, Clone, Copy)]
struct Blast {
    x: f32,
    y: f32,
    age: f32,
}

struct Warden {
    theme: Theme,
    terrain: Terrain,
    ship: Ship,
    camera: Camera,
    pause: Pause,

    shots: Shots,
    enemies: Enemies,
    people: Humanoids,
    effects: Effects,
    lives: Lives,
    score: u32,
    /// ★ Whether the surface has already been destroyed, so the world
    /// ends exactly once rather than every frame the population is zero.
    world_ended: bool,

    /// ★ W1. Who arrives when, and when a wave is over. See `waves`.
    director: Director,
    /// ★ W3. What the Bombers leave behind.
    mines: Mines,
    popups: Popups,
    /// Held but not yet usable — W2 gives them a key. Shown in the HUD
    /// now because the 10,000-point award already grants them.
    smart_bombs: u32,
    /// The score at which the next ship and bomb are awarded.
    next_award: u32,
    /// ★ W2. Seconds into a hyperspace jump, while the ship is between
    /// places. `None` when it is not jumping.
    hyperspace: Option<f32>,
    /// The last smart bomb, while its flash and shockwave are still
    /// playing out.
    blast: Option<Blast>,
    /// A bomb went off since the last frame's sounds: play its voice.
    ///
    /// ⚠️ NOT A `*_this_frame` FLAG. The bomb fires from `on_input`,
    /// between frames, and `update` clears those flags before it plays
    /// anything — which is exactly why the bomb's kills were silent when
    /// Brian flew it. This one is cleared only by being played.
    bomb_pending: bool,
    /// A hyperspace jump began since the last frame's sounds — between
    /// frames, like the bomb, and for the same reason not a frame flag.
    hyper_pending: bool,
    /// Noise for hyperspace: where it lands and whether it survives.
    rng: u32,
    /// This game's seed; every random placement in it derives from this.
    seed: u32,
    /// Squads called so far this game, so each lands somewhere new.
    squads: u32,

    scores: ScoreFile,
    /// Whether game over writes the score file. Off in tests, which
    /// must never touch a player's real high scores.
    persist: bool,
    /// Whether this game's score has been banked, so game over records
    /// it once rather than every frame it is on screen.
    recorded: bool,
    best: u32,

    laser: SoundId,

    /// ★ A `VoiceId`, NOT a `SoundId`, and the type is the whole

    /// distinction: one-shots are `play`ed and retire themselves,

    /// continuous voices are `start`ed and `set` and never die.

    thrust: VoiceId,
    /// A Lander materialising. ★ One of the six silent events, and the
    /// first of them to get a voice.
    warp: SoundId,
    boom: SoundId,
    mutant_boom: SoundId,
    /// ★ W5: every other enemy dies its own way, as in the original.
    bomber_hit: SoundId,
    pod_hit: SoundId,
    /// The Baiter's and the Swarmer's — the original shares one (PROTV).
    baiter_hit: SoundId,
    ship_boom: SoundId,
    person_boom: SoundId,
    bomb_sound: SoundId,
    hyper_sound: SoundId,
    grab_sound: SoundId,
    catch_sound: SoundId,
    set_down_sound: SoundId,
    fusion_sound: SoundId,
    lander_shot_sound: SoundId,
    mutant_shot_sound: SoundId,
    swarmer_shot_sound: SoundId,
    extra_life_sound: SoundId,
    scream: VoiceId,
    /// Falls begun so far — the scream restarts when this moves.
    falls: u32,
    /// How many were falling last frame, to notice a new fall.
    falling: usize,

    // Held keys, resolved into an `Input` each step.
    thrust_held: bool,
    /// ★ Eased engine intensity, 0.0 at rest. Drives the hull flare only.
    exhaust: f32,
    up_held: bool,
    down_held: bool,
    fire_held: bool,

    /// Left over from the last frame, so a frame longer than the
    /// timestep does not silently drop simulation time.
    accumulator: f32,

    /// Seconds of simulated time since the game began.
    ///
    /// ★ ADDED BY S8 FOR THE SCANNER'S MUTANT PULSE. Advanced inside the
    /// FIXED step rather than from the frame's `dt`, so it counts the
    /// same simulated time everything else does — a pulse driven by
    /// wall-clock would drift away from the game it is describing the
    /// moment a frame ran long, and would keep running while paused.
    elapsed: f32,

    /// Sounds asked for during the fixed-step loop, played once the
    /// stepping is done.
    ///
    /// ⚠️ THE LOOP CANNOT PLAY THEM DIRECTLY. `update` may run several
    /// physics steps for one frame, and a burst that kills two Landers
    /// inside one frame would otherwise fire two identical one-shots a
    /// few microseconds apart — which is not twice as loud, it is a
    /// flam. Counting and playing once per frame is both correct and
    /// cheaper.
    fired_this_frame: bool,
    // ★ FOUR DEATHS, FOUR FLAGS. This was one `killed_this_frame` that
    // fired the same boom for a Lander, a Mutant, a shot Humanoid and
    // YOUR OWN SHIP. Four events with different meanings and identical
    // feedback — the game said the same thing for "you scored" and "you
    // lost a life".
    lander_killed_this_frame: bool,
    mutant_killed_this_frame: bool,
    bomber_killed_this_frame: bool,
    pod_killed_this_frame: bool,
    baiter_killed_this_frame: bool,
    person_killed_this_frame: bool,
    rescued_this_frame: bool,
    set_down_this_frame: bool,
    lander_fired_this_frame: bool,
    mutant_fired_this_frame: bool,
    swarmer_fired_this_frame: bool,
    /// ★ W5: a ship (and a bomb) awarded at 10,000.
    awarded_this_frame: bool,
    died_this_frame: bool,
    world_ended_this_frame: bool,
}

/// Which enemy gun fired — each has its own voice.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Gun {
    /// A Lander's or a Baiter's (DP1V).
    Lander,
    /// A Mutant's (CLDWN).
    Mutant,
    /// A Swarmer's (ED12).
    Swarmer,
}

/// The event voices — the four deaths, the smart bomb, the hyperspace
/// jump, the people's sounds and the enemy's guns — grouped so
/// `Warden::new` takes one argument rather than fourteen.
///
/// ⚠️ NOT a tuple. Same-typed `SoundId`s positionally would silently
/// swap if anyone reordered them, and the compiler would never say a
/// word — you would just hear a Lander die like a ship once and never
/// work out why.
struct Voices {
    lander: SoundId,
    mutant: SoundId,
    bomber_hit: SoundId,
    pod_hit: SoundId,
    baiter_hit: SoundId,
    ship: SoundId,
    person: SoundId,
    /// ★ The smart bomb: everything on screen going at once.
    smart_bomb: SoundId,
    /// ★ The hyperspace jump: out, then back in.
    hyperspace: SoundId,
    /// ★ The people's sounds (Williams GWAVE / VARI / SCREAM): a Lander
    /// taking someone, catching them, setting them down, and a Mutant
    /// forming.
    grab: SoundId,
    catch: SoundId,
    set_down: SoundId,
    fusion: SoundId,
    /// ★ The enemy's guns: a Lander's or Baiter's, and a Mutant's.
    lander_shot: SoundId,
    mutant_shot: SoundId,
    swarmer_shot: SoundId,
    /// ★ An extra ship, every 10,000.
    extra_life: SoundId,
    /// ★ The scream of someone falling — CONTINUOUS, so it can stop the
    /// moment they are caught or land (see `sound::Scream`).
    scream: VoiceId,
}

impl Warden {
    fn new(
        theme: Theme,
        laser: SoundId,
        thrust: VoiceId,
        warp: SoundId,
        voices: Voices,
        scores: ScoreFile,
        seed: u32,
    ) -> Self {
        let terrain = Terrain::generate(RIDGE_SAMPLES, seed);
        let ship = Ship::new(0.0);
        let camera = Camera::new(ship.x);
        let best = scores.best().unwrap_or(0);

        let mut g = Self {
            theme,
            terrain,
            ship,
            camera,
            pause: Pause::new(),
            shots: Shots::new(),
            enemies: Enemies::new(),
            people: Humanoids::new(),
            effects: Effects::new(),
            lives: Lives::new(),
            world_ended: false,
            score: 0,
            director: Director::new(),
            mines: Mines::new(),
            popups: Popups::new(),
            smart_bombs: STARTING_SMART_BOMBS,
            next_award: AWARD_EVERY,
            hyperspace: None,
            blast: None,
            bomb_pending: false,
            hyper_pending: false,
            rng: 1,
            seed,
            squads: 0,
            scores,
            persist: false,
            recorded: false,
            best,
            laser,
            thrust,
            warp,
            boom: voices.lander,
            mutant_boom: voices.mutant,
            bomber_hit: voices.bomber_hit,
            pod_hit: voices.pod_hit,
            baiter_hit: voices.baiter_hit,
            ship_boom: voices.ship,
            person_boom: voices.person,
            bomb_sound: voices.smart_bomb,
            hyper_sound: voices.hyperspace,
            grab_sound: voices.grab,
            catch_sound: voices.catch,
            set_down_sound: voices.set_down,
            fusion_sound: voices.fusion,
            lander_shot_sound: voices.lander_shot,
            mutant_shot_sound: voices.mutant_shot,
            swarmer_shot_sound: voices.swarmer_shot,
            extra_life_sound: voices.extra_life,
            scream: voices.scream,
            falls: 0,
            falling: 0,
            thrust_held: false,
            exhaust: 0.0,
            up_held: false,
            down_held: false,
            fire_held: false,
            accumulator: 0.0,
            elapsed: 0.0,
            fired_this_frame: false,
            lander_killed_this_frame: false,
            mutant_killed_this_frame: false,
            bomber_killed_this_frame: false,
            pod_killed_this_frame: false,
            baiter_killed_this_frame: false,
            person_killed_this_frame: false,
            rescued_this_frame: false,
            set_down_this_frame: false,
            lander_fired_this_frame: false,
            mutant_fired_this_frame: false,
            swarmer_fired_this_frame: false,
            awarded_this_frame: false,
            died_this_frame: false,
            world_ended_this_frame: false,
        };
        g.new_game(seed);
        g
    }

    /// Everything a fresh game needs, from `seed`.
    ///
    /// ★ ONE PATH FOR THE FIRST GAME AND EVERY RESTART, so a restart
    /// cannot quietly inherit something the first game never had — a
    /// mutated enemy list, a destroyed surface, a pause.
    ///
    /// ⚠️ NO ENEMIES ARE PLACED HERE. The director brings the first squad
    /// on the first step, through `spawn`, so it arrives the way every
    /// later squad does — warping in, and audibly. The old opening five
    /// were placed silently before frame one and needed a flag to keep
    /// their arrival quiet; a wave the player watches arrive should sound.
    fn new_game(&mut self, seed: u32) {
        self.seed = seed;
        self.squads = 0;
        self.terrain = Terrain::generate(RIDGE_SAMPLES, seed);
        self.ship = Ship::new(0.0);
        self.camera = Camera::new(self.ship.x);
        // Snap rather than ease on the first frame: easing in from a
        // default position would read as an opening swoop nobody asked
        // for.
        self.camera.snap_to(&self.ship);
        self.shots.clear();
        self.enemies = Enemies::new();
        self.enemies.reseed(mix(seed, 0x4D07_A17E));
        self.people = Humanoids::new();
        self.people.scatter(POPULATION, &self.terrain, mix(seed, 0x50C1_A15E));
        self.effects.clear();
        self.lives.reset();
        self.score = 0;
        self.world_ended = false;
        self.director = Director::new();
        self.mines.clear();
        self.popups.clear();
        self.smart_bombs = STARTING_SMART_BOMBS;
        self.next_award = AWARD_EVERY;
        self.hyperspace = None;
        self.blast = None;
        self.bomb_pending = false;
        self.hyper_pending = false;
        self.rng = mix(seed, 0x4859_5045);
        self.recorded = false;
        self.accumulator = 0.0;
        self.elapsed = 0.0;
        // ⚠️ A restart must never inherit a pause: a fresh game frozen
        // behind a PAUSED overlay reads as a hang.
        self.pause.resume();
    }

    /// Add `points`, showing them at `(x, y)` when `at` is given, and pay
    /// out the 10,000-point award as many times as it was crossed.
    fn add_score(&mut self, points: u32, at: Option<(f32, f32)>) {
        self.score += points;
        if let Some((x, y)) = at {
            self.popups.add(x, y, points);
        }
        // ⚠️ `while`, NOT `if`: a single big award — a wave bonus late in
        // the game — can cross two thresholds at once.
        while self.score >= self.next_award {
            self.next_award += AWARD_EVERY;
            // ★ W5: the original's "free ship" (FOSHIT). Not for a game
            // that is already over — `award` grants nothing then either.
            if !self.lives.is_game_over() {
                self.awarded_this_frame = true;
            }
            self.lives.award();
            self.smart_bombs += 1;
        }
    }

    /// Is the ship in play — flying, and not between places in
    /// hyperspace? Everything that moves, fires, catches or is hit asks
    /// this rather than `lives.is_flying()`, so a jump cannot be shot,
    /// steered or chased.
    fn flying(&self) -> bool {
        self.lives.is_flying() && self.hyperspace.is_none()
    }

    /// A 0..1 from this game's noise.
    fn roll(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
    }

    /// ★ THE SMART BOMB (B). Everything hostile on screen dies, at full
    /// points. People are untouched — the bomb is for aliens, and a
    /// carrier that dies drops its passenger exactly as a laser kill does.
    ///
    /// ⚠️ ON SCREEN MEANS DRAWN: inside the view horizontally AND
    /// vertically. A Lander carrying someone off the top is above the
    /// screen and survives the bomb, as it did in the original.
    fn smart_bomb(&mut self) {
        if !self.flying() || self.smart_bombs == 0 {
            return;
        }
        self.smart_bombs -= 1;
        self.blast = Some(Blast { x: self.ship.x, y: self.ship.y, age: 0.0 });
        self.bomb_pending = true;
        // ★ BRIAN: "particles explode on screen and go out from center of
        // craft". The burst starts AT the ship, so the bomb reads as
        // something the ship did rather than something that happened.
        self.effects.smart_bomb(self.ship.x, self.ship.y);
        for i in 0..self.enemies.len() {
            let on_screen = self.enemies.get(i).is_some_and(|e| {
                let sx = self.camera.to_screen(e.x);
                e.is_target()
                    && (0.0..=world::VIEW_W).contains(&sx)
                    && (0.0..=world::VIEW_H).contains(&e.y)
            });
            if on_screen {
                self.destroy_enemy(i);
            }
        }
    }

    /// Is this frame one of the bomb's white flashes?
    fn flash_on(&self) -> bool {
        self.blast.is_some_and(|b| {
            b.age < BOMB_FLASH_SECONDS && (b.age / (BOMB_FLASH_PERIOD * 0.5)) as u32 % 2 == 0
        })
    }

    /// ★ HYPERSPACE (H). Out of here, to anywhere: a random place in the
    /// world, stopped dead, with every enemy bolt gone. The ship takes
    /// [`HYPERSPACE_SECONDS`] to come back together, and may not survive
    /// it ([`HYPERSPACE_DEATH`]).
    fn hyperspace(&mut self) {
        if !self.flying() {
            return;
        }
        // ⚠️ NEVER ONTO THE SCREEN YOU LEFT. Brian flew it: "not taking
        // you to another part of the map, just respawning in same map
        // area just lower or higher". A uniform draw over a world only
        // four screens round lands within one screen of the start about
        // half the time, and similar mountains make that read as not
        // having moved. So the jump is at least one screen and at most
        // three, either way round — always somewhere else.
        let away = world::VIEW_W + self.roll() * (world::WORLD_W - 2.0 * world::VIEW_W);
        let x = world::wrap(self.ship.x + away);
        let y = world::VIEW_H * (0.25 + 0.55 * self.roll());
        self.ship.x = world::wrap(x);
        self.ship.y = y.max(self.terrain.height_at(x) + 60.0);
        self.ship.vx = 0.0;
        // The original also lands you facing a random way.
        self.ship.facing = if self.roll() < 0.5 { Facing::East } else { Facing::West };
        self.camera.snap_to(&self.ship);
        self.shots.clear_enemy();
        self.hyperspace = Some(0.0);
        self.hyper_pending = true;
    }

    /// Advance a jump in progress, and settle the death roll on arrival.
    fn step_hyperspace(&mut self, dt: f32) {
        let Some(t) = self.hyperspace else { return };
        let t = t + dt;
        if t < HYPERSPACE_SECONDS {
            self.hyperspace = Some(t);
            return;
        }
        self.hyperspace = None;
        if self.roll() < HYPERSPACE_DEATH && self.lives.destroy() {
            self.ship_lost();
        }
    }

    /// The ship has just been destroyed: the explosion, the sound, and
    /// the passenger who goes with it.
    fn ship_lost(&mut self) {
        self.effects.explode_ship(self.ship.x, self.ship.y, self.ship.vx);
        self.died_this_frame = true;
        // ⚠️ THE PASSENGER GOES WITH YOU. A rescued Humanoid riding
        // under a ship that explodes cannot simply carry on hovering
        // in mid-air, and silently deleting it would be worse.
        for i in 0..self.people.len() {
            if let Some(h) = self.people.get_mut(i) {
                if h.state == humanoid::State::Rescued {
                    h.kill();
                }
            }
        }
    }

    /// Bank the score the first time a game ends.
    ///
    /// Save failures are swallowed on purpose (Pixel Break's rule): a
    /// scoreboard that cannot be written is not a reason to interrupt
    /// anyone's game.
    fn bank_score(&mut self) {
        if self.recorded {
            return;
        }
        self.recorded = true;
        self.scores.record(self.score);
        self.best = self.scores.best().unwrap_or(0);
        if self.persist {
            let _ = self.scores.save();
        }
    }

    /// What is alive, as the director counts it. Baiters are counted
    /// apart: they hurry a wave along and leave when it is won, so they
    /// never hold one open.
    fn census(&self) -> Census {
        let baiters = self.enemies.count(enemy::Kind::Baiter);
        Census {
            landers: self.enemies.count(enemy::Kind::Lander),
            hostiles: self.enemies.remaining() - baiters,
            baiters,
        }
    }

    /// ★ W3: this wave's Bombers, placed around the ship in squads of up
    /// to three. Each squad drifts the opposite way to the last (the
    /// original's `COM TFLG`), and none starts on screen.
    fn spawn_bombers(&mut self, count: usize) {
        let speed = waves::bomber_speed(self.director.wave());
        let mut placed = 0;
        let mut squad = 0u32;
        while placed < count {
            let n = waves::BOMBER_SQUAD.min(count - placed);
            let dir = if squad % 2 == 0 { 1.0 } else { -1.0 };
            let side = if self.roll() < 0.5 { 1.0 } else { -1.0 };
            let base = self.ship.x + side * world::VIEW_W * (1.1 + 0.5 * squad as f32);
            let cruise = world::VIEW_H * (0.5 + 0.25 * self.roll());
            for i in 0..n {
                let x = base + i as f32 * 70.0;
                let y = cruise + (i as f32 - 1.0) * 24.0;
                self.enemies.spawn(enemy::Enemy::bomber(x, y, speed * dir, cruise));
            }
            placed += n;
            squad += 1;
        }
    }

    /// ★ W4: this wave's Pods, each at a random place at least most of a
    /// screen from the ship, drifting its own random way (PRBST). They
    /// warp in through `spawn`, as the Bombers do.
    fn spawn_pods(&mut self, count: usize) {
        for _ in 0..count {
            let away = world::VIEW_W * 0.8 + self.roll() * (world::WORLD_W - 1.6 * world::VIEW_W);
            let x = world::wrap(self.ship.x + away);
            let y = world::VIEW_H * (0.4 + 0.45 * self.roll());
            let vx = (self.roll() - 0.5) * 2.0 * enemy::POD_DRIFT_MAX;
            let mut vy = enemy::POD_VY_MIN + self.roll() * (enemy::POD_VY_MAX - enemy::POD_VY_MIN);
            if self.roll() < 0.5 {
                vy = -vy;
            }
            self.enemies.spawn(enemy::Enemy::pod(x, y, vx, vy));
        }
    }

    /// ★ W3: a Baiter, warping in just off one edge of the screen — near
    /// enough to arrive in seconds, never in your lap.
    fn spawn_baiter(&mut self) {
        let side = if self.roll() < 0.5 { 1.0 } else { -1.0 };
        let x = self.ship.x + side * world::VIEW_W * 0.65;
        let y = (self.ship.y + (self.roll() - 0.5) * 200.0).clamp(80.0, world::VIEW_H * 0.9);
        self.enemies.spawn(enemy::Enemy::baiter(x, y));
    }

    /// Run the wave director for one step and apply what it asks for.
    fn direct(&mut self, dt: f32) {
        self.enemies.set_pressure(self.director.pressure());
        let event = self.director.step(dt, self.census(), self.people.alive());
        match event {
            None => {}
            Some(Event::Squad(n)) => {
                self.squads += 1;
                let seed = mix(self.seed, 0x5EED_0000 ^ self.squads);
                // ★ After the world has ended they come through as Mutants.
                self.enemies.squad(n, self.ship.x, seed, self.world_ended);
            }
            Some(Event::Bombers(n)) => self.spawn_bombers(n),
            Some(Event::Pods(n)) => self.spawn_pods(n),
            Some(Event::Baiter) => self.spawn_baiter(),
            Some(Event::Cleared) => {
                // The wave is held. Nothing in flight may still kill the
                // player while the survivors are being counted — and the
                // Baiters, with nothing left to hurry, leave (Brian's
                // spec: "Vanish when all Landers die").
                self.shots.clear();
                self.mines.clear();
                self.enemies.dismiss_baiters();
            }
            Some(Event::BonusTick) => {
                let per = waves::bonus_per_humanoid(self.director.wave());
                self.add_score(per, None);
            }
            Some(Event::NextWave) if waves::restores(self.director.wave()) => {
                self.restore_planet();
            }
            Some(Event::NextWave) => {}
        }
    }

    /// ★ EVERY FIFTH WAVE THE PLANET IS REBUILT: the surface comes back
    /// and ten people with it.
    ///
    /// ⚠️ ONLY AT A WAVE BOUNDARY, and that is what keeps the humanoid
    /// index invariant: Landers hold indices into the people list, and
    /// replacing the list under a live carrier would re-point it at a
    /// stranger. Between waves there are no enemies at all, so there is
    /// no index to break — and the claims are cleared regardless.
    fn restore_planet(&mut self) {
        debug_assert_eq!(self.enemies.remaining(), 0, "restore with enemies alive");
        self.enemies.clear();
        self.terrain.restore();
        self.world_ended = false;
        self.people.clear();
        let seed = mix(self.seed, 0x50C1_A15E ^ self.director.wave());
        self.people.scatter(POPULATION, &self.terrain, seed);
    }

    /// Where a bolt leaves the ship, in world coordinates.
    fn muzzle(&self) -> (f32, f32) {
        (
            world::wrap(self.ship.x + MUZZLE_FORWARD * self.ship.facing.sign()),
            self.ship.y,
        )
    }

    /// One physics step: the ship, then everything it can interact with.
    fn step(&mut self, input: Input, dt: f32) {
        self.elapsed += dt;
        self.lives.step(dt);
        self.step_hyperspace(dt);
        if let Some(b) = &mut self.blast {
            b.age += dt;
            if b.age >= BLAST_SECONDS.max(BOMB_FLASH_SECONDS) {
                self.blast = None;
            }
        }

        // ⚠️ A DEAD SHIP DOES NOT FLY, AND MUTANTS MUST NOT TRACK IT.
        // Handing them a position while the player cannot move means
        // they converge on the respawn point and kill the next life
        // instantly — the exact death loop the invulnerability exists to
        // prevent, reintroduced by an oversight.
        let was_y = self.ship.y;
        let ship_pos = if self.flying() {
            self.ship.step(input, &self.terrain, dt);
            self.camera.follow(&self.ship, dt);

            // ★ THE PLUME. Emitted per frame while the key is held, and
            // only while the ship is actually flying — a dead ship that
            // still trailed fire would advertise a position the player
            // does not have.
            if input.thrust {
                self.effects.thrust_plume(
                    self.ship.x,
                    self.ship.y,
                    self.ship.vx,
                    self.ship.facing.sign(),
                );
            }

            Some((self.ship.x, self.ship.y))
        } else {
            None
        };

        // ⚠️ EASED, NOT SNAPPED. The hull flare jumping to full and back
        // on the key edge reads as a light switch; the engine should
        // catch and die away. The plume itself is instant — the cloud is
        // made of particles that already outlive the keypress.
        let want = if input.thrust && self.flying() { 1.0 } else { 0.0 };
        let rate = if want > self.exhaust { EXHAUST_ATTACK } else { EXHAUST_RELEASE };
        self.exhaust += (want - self.exhaust).clamp(-rate * dt, rate * dt);
        self.exhaust = self.exhaust.clamp(0.0, 1.0);

        if self.fire_held && self.flying() && self.shots.ready() {
            let (mx, my) = self.muzzle();
            if self.shots.fire(mx, my, self.ship.facing.sign()) {
                self.fired_this_frame = true;
            }
        }

        self.shots.step(dt);

        // Enemies that want to shoot say so; the bolts are built here,
        // because `Enemies` does not know what a Shot is.
        // ★ THE BAITER'S CHASE: the ship's velocity — vertical measured,
        // since the climb is direct — and how keen it is to re-aim.
        let (ship_vx, ship_vy) = (self.ship.vx, (self.ship.y - was_y) / dt.max(1e-6));
        self.enemies.set_chase(enemy::Chase {
            vx: ship_vx,
            vy: ship_vy,
            seek: self.director.baiter_seek(),
        });
        let wants = self.enemies.step(&self.terrain, &mut self.people, ship_pos, dt);
        // ★ W3: and Bombers say where they dropped a mine.
        for (x, y) in self.enemies.take_mines() {
            self.mines.lay(x, y);
        }
        self.mines.step(dt);
        if let Some((sx, sy)) = ship_pos {
            for (index, mx, my) in wants {
                let mut noise = self.enemies.next_noise();
                let (dx, dy, speed, gun) = match self.enemies.get(index) {
                    // ⚠️ A MUTANT NEVER SHOOTS STRAIGHT (Brian's rule,
                    // enforced in `aim_at`); a Lander's slow shot may.
                    // ★ W4: a Swarmer shoots FORWARD, not at you.
                    Some(s) if s.kind == enemy::Kind::Swarmer => {
                        let (dx, dy) = s.swarmer_aim(sy);
                        (dx, dy, s.shot_speed(), Gun::Swarmer)
                    }
                    Some(m) if m.is_mutant() => {
                        let (dx, dy) = m.aim_at(sx, sy, &mut noise);
                        (dx, dy, m.shot_speed(), Gun::Mutant)
                    }
                    Some(l) => {
                        let (dx, dy) = l.lander_aim(sx, sy, ship_vx, ship_vy, &mut noise);
                        (dx, dy, l.shot_speed(), Gun::Lander)
                    }
                    None => continue,
                };
                self.shots.fire_enemy_at(mx, my, dx, dy, speed);
                // ★ Brian: "there is a different sound when they shoot".
                match gun {
                    Gun::Lander => self.lander_fired_this_frame = true,
                    Gun::Mutant => self.mutant_fired_this_frame = true,
                    Gun::Swarmer => self.swarmer_fired_this_frame = true,
                }
            }
        }

        // ★ A SHORT FALL SURVIVED ON ITS OWN is worth 250.
        for i in self.people.step(&self.terrain, dt) {
            if let Some(h) = self.people.get(i) {
                let (x, y) = (h.x, h.y);
                self.add_score(SAFE_LANDING_POINTS, Some((x, y)));
            }
        }
        self.effects.update(dt);
        self.popups.step(dt);

        self.resolve_hits();
        self.resolve_catches();
        self.resolve_ship_hit();
        self.check_world_end();

        // ⚠️ NO WAVES AFTER THE LAST LIFE. A game that is over keeps
        // drawing the world, but nothing new arrives in it — and the
        // score is banked the moment it ends, not when the player
        // presses a key, so quitting from the game-over card keeps it.
        if self.lives.is_game_over() {
            self.bank_score();
        } else {
            self.direct(dt);
        }
    }

    /// An enemy bolt finding the ship.
    fn resolve_ship_hit(&mut self) {
        // ⚠️ NOT WHILE JUMPING. A ship between places is nowhere.
        if !self.lives.is_vulnerable() || self.hyperspace.is_some() {
            return;
        }
        let shot = self.shots.enemy_hit(self.ship.x, self.ship.y, SHIP_HALF_W, SHIP_HALF_H).is_some();
        // ★ FLYING INTO ONE KILLS YOU TOO, and takes it with you — the
        // original's rule, and what makes a Mutant at point-blank range a
        // threat rather than an easy kill. The wreck still scores: it is
        // dead, and the player paid for it.
        let body = if shot {
            None
        } else {
            self.enemies.body_hit(self.ship.x, self.ship.y, SHIP_HALF_W, SHIP_HALF_H)
        };
        // ★ W3: A MINE CANNOT BE SHOT, ONLY AVOIDED. Flying into one costs
        // a life, and the mine goes with it.
        let mine = !shot
            && body.is_none()
            && self.mines.hit(self.ship.x, self.ship.y, SHIP_HALF_W, SHIP_HALF_H);
        if let Some(i) = body {
            if self.lives.is_vulnerable() {
                self.destroy_enemy(i);
            }
        }
        if (shot || mine || body.is_some()) && self.lives.hit() {
            self.ship_lost();
        }
    }

    /// ★★ THE WORLD ENDING. Every person gone means the surface goes too.
    fn check_world_end(&mut self) {
        if self.world_ended || self.people.alive() > 0 {
            return;
        }
        self.world_ended = true;
        self.terrain.destroy();
        // Every surviving Lander turns — the arcade's own spike in
        // difficulty at the exact moment there is nothing left to save.
        self.enemies.mutate_all();
        self.effects.explode_world(&self.terrain, self.camera.x);
        self.world_ended_this_frame = true;
    }

    /// The ship flying into a falling Humanoid.
    ///
    /// ★ THE RESCUE. A caught person rides under the ship until it flies
    /// low enough to set them down, which is the loop the arcade built
    /// its whole risk/reward around: you are safest high up and you can
    /// only return someone by going low.
    fn resolve_catches(&mut self) {
        // ⚠️ A DEAD SHIP CATCHES NO ONE. It is not there to catch with.
        if !self.flying() {
            return;
        }
        if let Some(i) = self.people.catch_test(self.ship.x, self.ship.y) {
            if let Some(h) = self.people.get_mut(i) {
                h.rescued();
                self.rescued_this_frame = true;
                let (x, y) = (h.x, h.y);
                self.add_score(CATCH_POINTS, Some((x, y)));
            }
        }

        // Carry passengers along, and put them down near the ground.
        self.people.carry_with_ship(self.ship.x, self.ship.y);
        let ground = self.terrain.height_at(self.ship.x);
        if self.ship.y - ground < DROP_OFF_HEIGHT {
            let mut set_down = Vec::new();
            for i in 0..self.people.len() {
                if let Some(h) = self.people.get_mut(i) {
                    if h.released_to_ground(&self.terrain) {
                        set_down.push((h.x, h.y));
                    }
                }
            }
            for at in set_down {
                self.add_score(SET_DOWN_POINTS, Some(at));
                self.set_down_this_frame = true;
            }
        }
    }

    /// Kill the enemy at `index`: drop its passenger, score it, blow it
    /// up, and say which death it was. Shared by your laser and by your
    /// hull, so the two can never disagree about what a kill means.
    fn destroy_enemy(&mut self, target: usize) {
        let (lx, ly, lvx) = match self.enemies.get(target) {
            Some(l) => (l.x, l.y, l.vx),
            None => return,
        };
        // ⚠️ DROP THE PASSENGER BEFORE KILLING THE CARRIER.
        // `kill` clears the target, so doing this after would
        // take the Humanoid with it silently and the whole
        // catch-and-rescue loop would never fire.
        self.enemies.release_passenger(target, &mut self.people);
        // ⚠️ ASK WHAT IT IS *BEFORE* KILLING IT. `kill` sets
        // Phase::Dying, and a Mutant that has started dying is
        // still a Mutant — but reading the kind afterwards means
        // reaching back into a list this line just mutated, and
        // the next person to touch it would have to prove that
        // still works. Read it first; it is one bool.
        let kind = match self.enemies.get(target) {
            Some(l) => l.kind,
            None => return,
        };
        let points = self.enemies.kill(target);
        self.add_score(points, None);
        // Each dies in its own colours, and (W5) in its own voice.
        match kind {
            enemy::Kind::Lander | enemy::Kind::Mutant => self.effects.explode_lander(lx, ly, lvx),
            enemy::Kind::Baiter => self.effects.explode_baiter(lx, ly, lvx),
            enemy::Kind::Bomber => self.effects.explode_bomber(lx, ly, lvx),
            enemy::Kind::Pod => self.effects.explode_pod(lx, ly, lvx),
            enemy::Kind::Swarmer => self.effects.explode_swarmer(lx, ly, lvx),
        }
        match kind {
            enemy::Kind::Lander => self.lander_killed_this_frame = true,
            enemy::Kind::Mutant => self.mutant_killed_this_frame = true,
            enemy::Kind::Bomber => self.bomber_killed_this_frame = true,
            enemy::Kind::Pod => self.pod_killed_this_frame = true,
            enemy::Kind::Baiter | enemy::Kind::Swarmer => self.baiter_killed_this_frame = true,
        }
    }

    /// Shots meeting Landers.
    ///
    /// ⚠️ ITERATED BACKWARDS because a hit removes a shot by
    /// `swap_remove`, which moves the LAST element into the current
    /// index. Walking forward would skip whatever got swapped in — a
    /// bug that only shows up when two shots are in flight at once, and
    /// then only sometimes.
    fn resolve_hits(&mut self) {
        let mut i = self.shots.len();
        while i > 0 {
            i -= 1;
            let (sx, sy, from_enemy) = {
                let s = match self.shots.iter().nth(i) {
                    Some(s) => *s,
                    None => continue,
                };
                (s.x, s.y, s.is_enemy())
            };

            // ⚠️⚠️ AN ENEMY BOLT MUST NOT BE TESTED AGAINST ENEMIES.
            // A Mutant's shot spawns INSIDE the Mutant's own hitbox, so
            // without this the very next frame finds the shooter and
            // kills it — Brian saw Mutants "just self destruct" about a
            // fire-interval after appearing, and this was why. The
            // owner check existed for shots hitting the SHIP and was
            // never mirrored for shots hitting ENEMIES.
            //
            // Enemy bolts also pass harmlessly through Humanoids: the
            // aliens are not here to shoot their own cargo.
            if from_enemy {
                continue;
            }

            if let Some(target) = self.enemies.hit_test(sx, sy) {
                self.destroy_enemy(target);
                self.shots.consume(i);
                continue;
            }

            // ⚠️ AND YOUR OWN LASER KILLS PEOPLE. Brian's spec says
            // DON'T SHOOT THEM, and a rule the game quietly refuses to
            // let you break is not a rule anyone ever feels.
            if let Some(victim) = self.people.hit_test(sx, sy) {
                let (hx, hy) = {
                    let h = self.people.get(victim).unwrap();
                    (h.x, h.y)
                };
                if let Some(h) = self.people.get_mut(victim) {
                    h.kill();
                }
                self.effects.explode_humanoid(hx, hy);
                self.shots.consume(i);
                self.person_killed_this_frame = true;
            }
        }
    }

    /// Resolve held keys into this step's request.
    ///
    /// Both vertical keys held cancels out, which is what a player
    /// expects from two keys — the same rule the other three games use.
    fn input(&self) -> Input {
        Input {
            thrust: self.thrust_held,
            vertical: match (self.up_held, self.down_held) {
                (true, false) => -1.0,
                (false, true) => 1.0,
                _ => 0.0,
            },
            face: None,
        }
    }

    /// Whether the engine should be sounding at all this frame.
    ///
    /// ⚠️ DERIVED, NEVER CACHED — the racer's rule, and it cost it a
    /// whole race of silence to learn. A `thrust_sounding: bool` field
    /// can disagree with what the mixer was actually told, and nothing
    /// ever reconciles the two. A predicate over the state that decides
    /// it cannot drift from that state.
    ///
    /// ★ A PAUSED SHIP GOES QUIET like a dead one, reusing this switch
    /// rather than adding a parallel one.
    fn thrust_should_run(&self) -> bool {
        !self.pause.is_paused() && self.flying()
    }

    /// ★ THE SCREAM runs while anyone is falling, and only then.
    ///
    /// Fed before the pause guard like the engine, for the same reason: a
    /// paused game must go quiet, not hold the last state. A new fall —
    /// more people falling than last frame — bumps `falls`, which restarts
    /// the voice from the top (`sound::Scream`).
    fn scream_sound(&mut self, audio: &mut Audio<'_>) {
        let falling = self.people.iter().filter(|h| h.state == humanoid::State::Falling).count();
        if falling > self.falling {
            self.falls += 1;
        }
        self.falling = falling;
        if self.scream_should_run() {
            audio.start(self.scream);
        } else {
            audio.stop(self.scream);
        }
        audio.set(self.scream, sound::Scream::params(self.falls));
    }

    /// Whether the scream should sound: someone is falling and the game
    /// is not paused. Derived, never cached (the engine's rule).
    fn scream_should_run(&self) -> bool {
        !self.pause.is_paused() && self.falling > 0
    }

    /// Feed the engine this frame's `exhaust`.
    ///
    /// ★★ THE SAME NUMBER THE PLUME USES. `exhaust` is eased once, in
    /// `step`, with an asymmetric attack and release; the picture and
    /// the sound then both read it. They cannot drift apart, because
    /// there is nothing to drift — it is one value.
    fn thrust_sound(&mut self, audio: &mut Audio<'_>) {
        if self.thrust_should_run() {
            audio.start(self.thrust);
        } else {
            audio.stop(self.thrust);
        }
        // `set` is idempotent, so there is deliberately no change
        // detection here.
        audio.set(self.thrust, VoiceParams::thrust(self.exhaust));
    }
}

impl Game for Warden {
    fn on_input(&mut self, event: InputEvent) -> bool {
        match event {
            // ⚠️ ESC QUITS. Never bound to anything else, in any title.
            InputEvent::KeyDown(Key::Escape) => return false,

            InputEvent::KeyDown(Key::P) => self.pause.toggle(),

            // ⚠️ ONLY KeyDown IS SWALLOWED WHILE PAUSED (the S11 rule).
            // Eating KeyUp too would leave a key "held" across the
            // pause, and the ship would fly off on resume because the
            // release never arrived.
            InputEvent::KeyDown(_) if self.pause.is_paused() => return true,

            // Facing is a key press, not a steering axis: Defender had
            // you face a direction and thrust that way, and the camera
            // starts its slide the moment you ask rather than when the
            // ship comes around.
            InputEvent::KeyDown(Key::Left) => self.ship.facing = Facing::West,
            InputEvent::KeyDown(Key::Right) => self.ship.facing = Facing::East,

            // ★ SPACE FIRES, T THRUSTS — Brian's call, having flown it:
            // "let's make 'T' thrust, space fire, that works pretty well
            // using arrow keys to steer." Space is the fire key every
            // shooter has, and thrust moved rather than fire settling
            // for a key nobody presses. `Key::T` was added to core for
            // this; the enum previously had no free letter at all.
            //
            // ⚠️ FIRE IS A HELD KEY, RATE-LIMITED BY THE GUN, not one
            // shot per KeyDown. Brian's spec says fire is unlimited, and
            // a per-press weapon turns that into a test of how fast the
            // player can tap.
            InputEvent::KeyDown(Key::Space) => self.fire_held = true,
            InputEvent::KeyUp(Key::Space) => self.fire_held = false,

            InputEvent::KeyDown(Key::T) => self.thrust_held = true,
            InputEvent::KeyUp(Key::T) => self.thrust_held = false,

            // ★ ENTER STARTS A NEW GAME, but only once this one is over —
            // otherwise a stray press wipes a game in progress. Not
            // Space: fire is HELD, and a player still holding it as the
            // last ship goes down would restart without meaning to.
            InputEvent::KeyDown(Key::Enter) if self.lives.is_game_over() => {
                self.bank_score();
                let seed = mix(self.seed, 0x9E37_79B9);
                self.new_game(seed);
            }

            // ★ W2, BRIAN'S KEYS: "B and H are fine". Both are presses,
            // not holds — one bomb, one jump, per KeyDown. Autorepeat is
            // dropped by the backend, so holding B cannot empty the stock.
            InputEvent::KeyDown(Key::B) => self.smart_bomb(),
            InputEvent::KeyDown(Key::H) => self.hyperspace(),

            InputEvent::KeyDown(Key::Up) => self.up_held = true,
            InputEvent::KeyUp(Key::Up) => self.up_held = false,
            InputEvent::KeyDown(Key::Down) => self.down_held = true,
            InputEvent::KeyUp(Key::Down) => self.down_held = false,

            _ => {}
        }
        true
    }

    fn update(&mut self, dt: f32, audio: &mut Audio<'_>) {
        // ★ THE ENGINE IS FED BEFORE THE PAUSE GUARD, NOT AFTER.
        // `update` returns early while paused, so a feed placed below
        // this point would leave the mixer holding the last `exhaust`
        // the ship had when the player hit P — a thrust drone under a
        // PAUSED overlay, which is the audio version of a frozen screen
        // with nothing to say why.
        // ⚠️ RESTATED UNCONDITIONALLY EVERY FRAME, never gated on a
        // cached "is it running" flag. `Command::Enable` is a plain
        // assignment in the mixer — idempotent and free — so a dropped
        // command heals on the very next frame instead of lasting the
        // whole run. The racer learned this the expensive way; see the
        // note above its own start/stop block.
        self.thrust_sound(audio);
        self.scream_sound(audio);

        if self.pause.is_paused() {
            return;
        }

        self.fired_this_frame = false;
        self.lander_killed_this_frame = false;
        self.mutant_killed_this_frame = false;
        self.bomber_killed_this_frame = false;
        self.pod_killed_this_frame = false;
        self.baiter_killed_this_frame = false;
        self.person_killed_this_frame = false;
        self.rescued_this_frame = false;
        self.set_down_this_frame = false;
        self.lander_fired_this_frame = false;
        self.mutant_fired_this_frame = false;
        self.swarmer_fired_this_frame = false;
        self.awarded_this_frame = false;
        self.died_this_frame = false;
        self.world_ended_this_frame = false;

        // Fixed-step accumulation. Clamped so a stalled frame — a
        // debugger, a laptop waking — does not spend a second of
        // wall-clock catching up in one go.
        self.accumulator = (self.accumulator + dt).min(0.25);
        let input = self.input();
        while self.accumulator >= FIXED_DT {
            self.step(input, FIXED_DT);
            self.accumulator -= FIXED_DT;
        }

        // See `fired_this_frame`: one play per frame, however many
        // physics steps ran.
        if self.fired_this_frame {
            audio.play(self.laser);
        }
        // ★ EACH DEATH SAYS WHAT IT WAS. A Lander thuds and sours (the
        // original's HBEV — Brian's pick over his Boom), a Mutant bangs, a Humanoid you shot barely registers, and your own
        // ship is the worst sound in the game.
        if self.lander_killed_this_frame {
            audio.play(self.boom);
        }
        if self.mutant_killed_this_frame {
            audio.play(self.mutant_boom);
        }
        // ★ W5, THE ORIGINAL'S OWN DEATHS (vsndrm1.src): a Bomber's
        // falling zip repeated (HBDV), a Pod's bell (BBSV), a Baiter's or
        // Swarmer's torn rising wail (PROTV).
        if self.bomber_killed_this_frame {
            audio.play(self.bomber_hit);
        }
        if self.pod_killed_this_frame {
            audio.play(self.pod_hit);
        }
        if self.baiter_killed_this_frame {
            audio.play(self.baiter_hit);
        }
        if self.person_killed_this_frame {
            audio.play(self.person_boom);
        }
        if self.died_this_frame {
            audio.play(self.ship_boom);
        }
        // ★ THE ENEMY'S GUNS HAVE THEIR OWN VOICES now, not the laser at
        // half gain: a Lander's (and a Baiter's) spitty "bzew", and a
        // Mutant's buzzy descending trill (Brian: "a different sound when
        // they shoot").
        if self.lander_fired_this_frame {
            audio.play(self.lander_shot_sound);
        }
        if self.mutant_fired_this_frame {
            audio.play(self.mutant_shot_sound);
        }
        // ★ W5: a Swarmer's is its own (ED12), a gritty falling chirp —
        // it borrowed the Lander's until now.
        if self.swarmer_fired_this_frame {
            audio.play(self.swarmer_shot_sound);
        }
        // ★ W5: AN EXTRA SHIP — the original's falling-sweep fanfare.
        if self.awarded_this_frame {
            audio.play(self.extra_life_sound);
        }
        // ★ THE PEOPLE'S SOUNDS. A Lander takes someone; the ship catches
        // them; it sets them down.
        if self.enemies.take_grabbed() > 0 {
            audio.play(self.grab_sound);
        }
        if self.rescued_this_frame {
            audio.play(self.catch_sound);
        }
        if self.set_down_this_frame {
            audio.play(self.set_down_sound);
        }
        if self.world_ended_this_frame {
            audio.play_with(self.boom, 1.0, 1.0);
        }
        // ★★ SOMETHING ARRIVED. One voice however many landed on the
        // same frame — the opening wave drops five at once, and five
        // retriggers of a single voice is four sounds cut dead at
        // sample zero and only the last one heard.
        // ⚠️ SCALED BY HOW MANY, so a group reads as bigger than one
        // straggler without ever reaching the laser's level.
        // ★★ A LANDER BECAME A MUTANT — and now it has its own voice.
        //
        // Brian hears this as a different event from an arrival, and he
        // is right that it is one: an arrival is something appearing out
        // of nothing, a fusion is something you failed to stop becoming
        // worse. It stayed silent rather than borrow Warp's voice until
        // its own existed: the original's SP1, the stepped rising buzz
        // ("When mutant spawns in they get sound").
        if self.enemies.take_fused() > 0 {
            audio.play(self.fusion_sound);
        }

        // ★ THE SMART BOMB SPEAKS FOR EVERYTHING IT KILLED. Its kills set
        // no per-death flags (they happen between frames, see
        // `bomb_pending`), and that is the original's behaviour too: the
        // board was monophonic and the bomb's sound was the one you heard.
        if self.bomb_pending {
            self.bomb_pending = false;
            audio.play(self.bomb_sound);
        }
        // ★ HYPERSPACE HAS A VOICE NOW (the original had none — Brian
        // asked). Timed to the jump: it ends as the ship comes together.
        if self.hyper_pending {
            self.hyper_pending = false;
            audio.play(self.hyper_sound);
        }

        // ★ W1: SQUADS ARRIVE, AND EVERY ONE IS HEARD — the first
        // included. The opening five used to be placed before frame one
        // and kept silent by a flag; now the director brings them in on
        // the first step, warping, so the player watches (and hears) the
        // wave begin.
        let arrived = self.enemies.take_spawned();
        if arrived > 0 {
            let gain = (0.55 + 0.12 * (arrived - 1) as f32).min(1.0);
            audio.play_with(self.warp, gain, 1.0);
        }
    }

    fn render(&mut self, canvas: &mut Canvas<'_>) {
        let scene = render::Scene {
            terrain: &self.terrain,
            ship: &self.ship,
            camera: &self.camera,
            shots: &self.shots,
            enemies: &self.enemies,
            people: &self.people,
            effects: &self.effects,
            score: self.score,
            lives: &self.lives,
            time: self.elapsed,
            exhaust: self.exhaust,
            popups: &self.popups,
            mines: &self.mines,
            hud: render::Hud {
                wave: self.director.wave(),
                phase: self.director.phase(),
                smart_bombs: self.smart_bombs,
                best: self.best.max(self.score),
                hyperspace: self.hyperspace.map(|t| t / HYPERSPACE_SECONDS),
                flash: self.flash_on(),
                blast: self
                    .blast
                    .filter(|b| b.age < BLAST_SECONDS)
                    .map(|b| (b.x, b.y, b.age / BLAST_SECONDS)),
            },
        };
        render::draw(canvas, &scene, &self.theme);
        self.pause.draw(canvas, &self.theme);
    }
}

/// Mix two numbers into a seed. Never zero: xorshift's one fixed point.
fn mix(a: u32, b: u32) -> u32 {
    let mut h = a ^ b.rotate_left(16);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846C_A68B);
    h ^= h >> 16;
    h | 1
}

/// ★ A DIFFERENT GAME EVERY LAUNCH. The seeds were fixed constants, so
/// every game had the same mountains, the same people in the same
/// places and the same Landers coming from the same directions.
fn clock_seed() -> u32 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x0DEF_E4DE);
    mix(nanos as u32, (nanos >> 32) as u32)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let theme = Theme::load();
    let mut audio = AudioSystem::new();
    let laser = audio.register_sound(Box::new(sound::Laser::new()));
    // ⚠️ `register`, NOT `register_sound`. The engine is continuous: it
    // is enabled and fed, never played. Registering it as a one-shot
    // would hand it to `play`, which retriggers rather than sustains.
    let thrust = audio.register(Box::new(sound::Thrust::new()));
    let warp = audio.register_sound(Box::new(sound::Warp::new()));
    let voices = Voices {
        lander: audio.register_sound(Box::new(sound::lander_hit())),
        mutant: audio.register_sound(Box::new(sound::MutantBoom::new())),
        bomber_hit: audio.register_sound(Box::new(sound::bomber_hit())),
        pod_hit: audio.register_sound(Box::new(sound::pod_hit())),
        baiter_hit: audio.register_sound(Box::new(sound::baiter_hit())),
        ship: audio.register_sound(Box::new(sound::ShipBoom::new())),
        person: audio.register_sound(Box::new(sound::PersonBoom::new())),
        smart_bomb: audio.register_sound(Box::new(sound::SmartBomb::new())),
        hyperspace: audio.register_sound(Box::new(sound::Hyperspace::new())),
        grab: audio.register_sound(Box::new(sound::grab())),
        catch: audio.register_sound(Box::new(sound::catch())),
        set_down: audio.register_sound(Box::new(sound::set_down())),
        fusion: audio.register_sound(Box::new(sound::fusion())),
        lander_shot: audio.register_sound(Box::new(sound::lander_shot())),
        mutant_shot: audio.register_sound(Box::new(sound::mutant_shot())),
        swarmer_shot: audio.register_sound(Box::new(sound::swarmer_shot())),
        extra_life: audio.register_sound(Box::new(sound::extra_life())),
        scream: audio.register(Box::new(sound::Scream::new())),
    };
    let scores = ScoreFile::load_or_new(GAME_ID, GAME_NAME);
    let mut game = Warden::new(theme, laser, thrust, warp, voices, scores, clock_seed());
    game.persist = true;

    WinitBackend::new(TITLE, WIDTH, HEIGHT)
        .idle(Idle::Animate { fps: 60 })
        .run(game, audio)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> Warden {
        game_seeded(0x0DEF_E4DE)
    }

    fn game_seeded(seed: u32) -> Warden {
        let mut audio = AudioSystem::new();
        let laser = audio.register_sound(Box::new(sound::Laser::new()));
        let thrust = audio.register(Box::new(sound::Thrust::new()));
        let warp = audio.register_sound(Box::new(sound::Warp::new()));
        let voices = Voices {
            lander: audio.register_sound(Box::new(sound::lander_hit())),
            mutant: audio.register_sound(Box::new(sound::MutantBoom::new())),
            bomber_hit: audio.register_sound(Box::new(sound::bomber_hit())),
            pod_hit: audio.register_sound(Box::new(sound::pod_hit())),
            baiter_hit: audio.register_sound(Box::new(sound::baiter_hit())),
            ship: audio.register_sound(Box::new(sound::ShipBoom::new())),
            person: audio.register_sound(Box::new(sound::PersonBoom::new())),
            smart_bomb: audio.register_sound(Box::new(sound::SmartBomb::new())),
            hyperspace: audio.register_sound(Box::new(sound::Hyperspace::new())),
            grab: audio.register_sound(Box::new(sound::grab())),
            catch: audio.register_sound(Box::new(sound::catch())),
            set_down: audio.register_sound(Box::new(sound::set_down())),
            fusion: audio.register_sound(Box::new(sound::fusion())),
            lander_shot: audio.register_sound(Box::new(sound::lander_shot())),
            mutant_shot: audio.register_sound(Box::new(sound::mutant_shot())),
        swarmer_shot: audio.register_sound(Box::new(sound::swarmer_shot())),
        extra_life: audio.register_sound(Box::new(sound::extra_life())),
            scream: audio.register(Box::new(sound::Scream::new())),
        };
        // ⚠️ AN IN-MEMORY SCORE FILE, never loaded or saved: a test that
        // reaches game over must not write a player's real high scores.
        let scores = ScoreFile::new(GAME_ID, GAME_NAME);
        Warden::new(Theme::fallback(), laser, thrust, warp, voices, scores, seed)
    }

    /// Kill every enemy that can be killed right now, through the same
    /// `destroy_enemy` the laser and the hull use.
    fn kill_all_targets(g: &mut Warden) {
        for i in 0..g.enemies.len() {
            if g.enemies.get(i).is_some_and(|e| e.is_target()) {
                g.destroy_enemy(i);
            }
        }
    }

    /// Run real frames through `update`, killing everything as it
    /// becomes killable, until `done` says stop. Returns frames run.
    fn play_until(g: &mut Warden, max_seconds: f32, done: impl Fn(&Warden) -> bool) -> usize {
        let mut audio = AudioSystem::new();
        let frames = (max_seconds * 60.0) as usize;
        for f in 0..frames {
            {
                let mut a = audio.handle();
                g.update(1.0 / 60.0, &mut a);
            }
            kill_all_targets(g);
            if done(g) {
                return f;
            }
        }
        panic!("not done after {max_seconds} s (wave {}, phase {:?})", g.director.wave(), g.director.phase());
    }


    /// ★ THE LINE SWEEPER — Brian: "hardly any landers will pick up a
    /// humanoid if you just go down the line". An immortal player at full
    /// thrust one way, firing nonstop, held `alt` above the ground (0 =
    /// a passive player: no thrust, no fire). Returns (people grabbed,
    /// people lost, seconds to clear wave 1 or None) within `secs`.
    fn sweep(seed: u32, secs: f32, alt: f32) -> (usize, usize, u32, Option<f32>) {
        let mut g = game_seeded(seed);
        immortal(&mut g);
        if alt > 0.0 {
            g.on_input(InputEvent::KeyDown(Key::T));
            g.on_input(InputEvent::KeyDown(Key::Space));
        }
        let mut audio = AudioSystem::new();
        let mut was: Vec<humanoid::State> = g.people.iter().map(|h| h.state).collect();
        let (mut grabs, mut lost) = (0, 0);
        for f in 0..(secs * 60.0) as usize {
            if alt > 0.0 {
                g.ship.y = g.terrain.height_at(g.ship.x) + alt;
            }
            {
                let mut a = audio.handle();
                g.update(1.0 / 60.0, &mut a);
            }
            for (i, h) in g.people.iter().enumerate() {
                if i < was.len() && was[i] != h.state {
                    if h.state == humanoid::State::Carried && was[i] == humanoid::State::Walking {
                        grabs += 1;
                    }
                    if h.state == humanoid::State::Dead {
                        lost += 1;
                    }
                    was[i] = h.state;
                }
            }
            if g.director.wave() > 1 {
                return (grabs, lost, 1_000 - g.lives.remaining, Some(f as f32 / 60.0));
            }
        }
        (grabs, lost, 1_000 - g.lives.remaining, None)
    }

    /// An instrument, not a check: `cargo test --release measure_sweeper
    /// -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn measure_sweeper() {
        for alt in [0.0, 270.0, 285.0, 300.0] {
            let (mut tg, mut tl, mut td, mut cl) = (0, 0, 0, vec![]);
            for s in 0..20u32 {
                let (g, l, d, c) = sweep(0x1000 + s * 7919, 120.0, alt);
                td += d;
                tg += g;
                tl += l;
                cl.extend(c);
            }
            let mean = cl.iter().sum::<f32>() / cl.len().max(1) as f32;
            println!(
                "alt {alt}: mean grabs {:.2} lost {:.2} deaths {:.2}, cleared {}/20 mean {mean:.1}s",
                tg as f32 / 20.0,
                tl as f32 / 20.0,
                td as f32 / 20.0,
                cl.len()
            );
        }
    }

    /// ★★ W5: EVERY ENEMY DIES IN ITS OWN VOICE, through the one
    /// `destroy_enemy` the laser and the hull both use. Exactly one death
    /// flag per kind — the Baiter and Swarmer share the original's PROTV.
    #[test]
    fn every_kind_dies_in_its_own_voice() {
        use enemy::{Enemy, Kind, Phase};
        let cases: [(Kind, Enemy); 6] = [
            (Kind::Lander, Enemy::lander(500.0, 400.0, 0.0)),
            (Kind::Mutant, Enemy::mutant(500.0, 400.0)),
            (Kind::Bomber, Enemy::bomber(500.0, 400.0, 0.0, 400.0)),
            (Kind::Pod, Enemy::pod(500.0, 400.0, 0.0, 0.0)),
            (Kind::Baiter, Enemy::baiter(500.0, 400.0)),
            (Kind::Swarmer, Enemy::swarmer(500.0, 400.0, 0.0, 0.0, 0.0, 0.0)),
        ];
        for (kind, mut e) in cases {
            let mut g = game();
            e.phase = Phase::Hovering;
            g.enemies.spawn(e);
            let i = g.enemies.len() - 1;
            g.destroy_enemy(i);
            let flags = [
                (Kind::Lander, g.lander_killed_this_frame),
                (Kind::Mutant, g.mutant_killed_this_frame),
                (Kind::Bomber, g.bomber_killed_this_frame),
                (Kind::Pod, g.pod_killed_this_frame),
                (Kind::Baiter, g.baiter_killed_this_frame),
            ];
            let want = if kind == Kind::Swarmer { Kind::Baiter } else { kind };
            for (k, set) in flags {
                assert_eq!(set, k == want, "{kind:?} died: the {k:?} voice flag was {set}");
            }
        }
    }

    /// ★★ W5: A SWARMER'S SHOT IS ITS OWN VOICE, through the real loop.
    /// A Swarmer flies at the ship until it fires; every Lander is killed
    /// as it arrives so no other gun can be heard. Before this, a
    /// Swarmer's shot raised the Lander's flag.
    #[test]
    fn a_swarmer_shot_has_its_own_voice() {
        use enemy::{Enemy, Kind, Phase};
        let mut g = game();
        immortal(&mut g);
        let mut audio = AudioSystem::new();
        let (mut swarmer, mut lander) = (false, false);
        for f in 0..(8 * 60) {
            if f % 60 == 0 && g.enemies.count(Kind::Swarmer) == 0 {
                let mut e = Enemy::swarmer(world::wrap(g.ship.x + 500.0), g.ship.y, -250.0, 0.0, 0.0, 0.0);
                e.phase = Phase::Hovering;
                g.enemies.spawn(e);
            }
            for i in 0..g.enemies.len() {
                if g.enemies.get(i).is_some_and(|e| e.kind == Kind::Lander && e.is_target()) {
                    g.enemies.kill(i);
                }
            }
            {
                let mut a = audio.handle();
                g.update(1.0 / 60.0, &mut a);
            }
            swarmer |= g.swarmer_fired_this_frame;
            lander |= g.lander_fired_this_frame;
        }
        assert!(swarmer, "no Swarmer shot was heard in 8 s");
        assert!(!lander, "a Swarmer's shot raised the Lander's flag");
    }

    /// A ship that cannot run out — the wave tests are about waves, and a
    /// stray Lander shot ending the game would end the test with it.
    fn immortal(g: &mut Warden) {
        g.lives.remaining = 1_000;
    }

    /// ★ THE FIRST SQUAD ARRIVES THROUGH `spawn` ON THE FIRST STEP — the
    /// counter Warp is wired to. Before W1 nothing ever reached it in
    /// play (1edd4a7); this is the path that makes the arrival audible.
    #[test]
    fn the_first_squad_arrives_through_spawn_on_the_first_step() {
        let mut g = game();
        assert_eq!(g.enemies.remaining(), 0, "nothing is placed before the game runs");
        g.step(Input::default(), FIXED_DT);
        assert_eq!(g.enemies.take_spawned(), waves::SQUAD_SIZE);
    }

    /// ★★ A WHOLE WAVE, THROUGH THE REAL LOOP (L069). Fifteen Landers
    /// arrive in squads, die, the survivors are counted, the bonus is
    /// paid, and wave 2 begins.
    #[test]
    fn a_whole_wave_plays_through_update_and_pays_its_bonus() {
        let mut g = game();
        immortal(&mut g);
        play_until(&mut g, 120.0, |g| g.director.wave() == 2);
        let arrived = waves::landers(1);
        let people = g.people.alive() as u32;
        assert_eq!(people, POPULATION as u32, "nobody should have been taken");
        assert_eq!(
            g.score,
            arrived as u32 * enemy::LANDER_POINTS + people * waves::bonus_per_humanoid(1),
            "fifteen Landers and the survivors' bonus"
        );
    }

    /// ★ EVERY FIFTH WAVE REBUILDS THE PLANET: the surface comes back and
    /// so do ten people, even after the world has ended.
    #[test]
    fn wave_five_restores_the_planet_and_its_people() {
        let mut g = game();
        immortal(&mut g);
        // End the world by hand: every person gone.
        for i in 0..g.people.len() {
            g.people.get_mut(i).unwrap().kill();
        }
        play_until(&mut g, 5.0, |g| g.world_ended);
        assert!(g.terrain.is_destroyed());

        play_until(&mut g, 600.0, |g| g.director.wave() == 5);
        assert!(!g.terrain.is_destroyed(), "wave 5 should rebuild the surface");
        assert!(!g.world_ended);
        assert_eq!(g.people.alive(), POPULATION, "wave 5 should bring ten people back");
    }

    /// ★ AFTER THE WORLD ENDS, SQUADS COME THROUGH AS MUTANTS — and they
    /// still finish arriving. A Mutant stuck in `Warping` could never be
    /// shot and the wave could never end.
    #[test]
    fn after_the_world_ends_squads_arrive_as_mutants_that_can_be_shot() {
        let mut g = game();
        immortal(&mut g);
        for i in 0..g.people.len() {
            g.people.get_mut(i).unwrap().kill();
        }
        // The first squad is already in; let the world end and the next
        // squad arrive into a dead world.
        play_until(&mut g, 60.0, |g| g.world_ended && g.director.reserve() < waves::landers(1) - waves::SQUAD_SIZE);
        let newest: Vec<_> = g.enemies.iter().filter(|e| e.phase == enemy::Phase::Warping).collect();
        assert!(!newest.is_empty(), "the second squad should be warping in");
        assert!(newest.iter().all(|e| e.is_mutant()), "a squad after the end should be Mutants");

        // In slices: `update` caps one call at 0.25 s.
        let mut audio = AudioSystem::new();
        for _ in 0..((enemy::WARP_SECONDS + 0.05) * 60.0) as usize {
            let mut a = audio.handle();
            g.update(1.0 / 60.0, &mut a);
        }
        assert!(
            g.enemies.iter().filter(|e| e.is_mutant()).all(|e| e.is_target() || e.phase == enemy::Phase::Dying),
            "a Mutant that warped in never finished arriving"
        );
    }

    /// Catch 500, set down 500 — through `step`, the way a rescue is
    /// actually flown.
    #[test]
    fn a_rescue_pays_for_the_catch_and_again_for_the_set_down() {
        let mut g = game();
        g.enemies.clear();
        g.people.clear();
        g.lives.state = lives::State::Alive;
        // High enough that the drop-off height is not reached at once.
        g.ship.y = 400.0;
        let mut h = humanoid::Humanoid::new(g.ship.x, g.ship.y + 10.0, 0.0);
        h.state = humanoid::State::Falling;
        h.fell_from = h.y;
        g.people.spawn(h);

        g.step(Input::default(), FIXED_DT);
        assert_eq!(g.score, CATCH_POINTS, "the catch did not pay");
        assert_eq!(g.people.get(0).unwrap().state, humanoid::State::Rescued);

        // Fly down until they are set down.
        let down = Input { vertical: 1.0, ..Default::default() };
        for _ in 0..(240 * 3) {
            g.step(down, FIXED_DT);
            if g.people.get(0).unwrap().state == humanoid::State::Walking {
                break;
            }
        }
        assert_eq!(g.people.get(0).unwrap().state, humanoid::State::Walking);
        assert_eq!(g.score, CATCH_POINTS + SET_DOWN_POINTS, "the set-down did not pay");
    }

    /// A short fall survived on its own is worth 250; a long one is not.
    #[test]
    fn a_safe_landing_pays_and_a_fatal_one_does_not() {
        let mut g = game();
        g.enemies.clear();
        g.people.clear();
        // Far from the ship, so it cannot be caught on the way down.
        let x = world::wrap(g.ship.x + world::VIEW_W * 1.5);
        let ground = g.terrain.height_at(x);
        let mut short = humanoid::Humanoid::new(x, ground + 40.0, 0.0);
        short.state = humanoid::State::Falling;
        short.fell_from = short.y;
        let mut long = humanoid::Humanoid::new(world::wrap(x + 200.0), 0.0, 0.0);
        long.y = g.terrain.height_at(long.x) + humanoid::SURVIVABLE_FALL * 2.0;
        long.state = humanoid::State::Falling;
        long.fell_from = long.y;
        g.people.spawn(short);
        g.people.spawn(long);

        // Six seconds: at the arcade's gravity the long fall alone takes
        // about 3.4.
        for _ in 0..(240 * 6) {
            g.step(Input::default(), FIXED_DT);
        }
        assert_eq!(g.people.get(0).unwrap().state, humanoid::State::Walking);
        assert_eq!(g.people.get(1).unwrap().state, humanoid::State::Dead);
        assert_eq!(g.score, SAFE_LANDING_POINTS);
    }

    /// Every 10,000: a ship and a smart bomb — twice, if one award
    /// crosses two thresholds.
    #[test]
    fn every_ten_thousand_awards_a_ship_and_a_bomb() {
        let mut g = game();
        let (ships, bombs) = (g.lives.remaining, g.smart_bombs);
        g.add_score(9_990, None);
        assert_eq!((g.lives.remaining, g.smart_bombs), (ships, bombs));
        assert!(!g.awarded_this_frame, "the fanfare played before 10,000");
        g.add_score(10, None);
        assert_eq!((g.lives.remaining, g.smart_bombs), (ships + 1, bombs + 1));
        // ★ W5: and the award is heard.
        assert!(g.awarded_this_frame, "10,000 crossed in silence");
        g.add_score(20_000, None);
        assert_eq!((g.lives.remaining, g.smart_bombs), (ships + 3, bombs + 3));
    }

    /// ★ FLYING INTO AN ENEMY KILLS YOU, and it.
    #[test]
    fn flying_into_an_enemy_costs_a_life_and_kills_it() {
        let mut g = game();
        g.enemies.clear();
        g.people.clear();
        g.lives.state = lives::State::Alive;
        let lives = g.lives.remaining;
        let mut l = enemy::Enemy::lander(g.ship.x, g.ship.y, 0.0);
        l.phase = enemy::Phase::Hovering;
        g.enemies.spawn(l);

        g.step(Input::default(), FIXED_DT);
        assert_eq!(g.lives.remaining, lives - 1, "the collision cost nothing");
        // ⚠️ THAT ONE, not "no enemies": the director brings wave 1's
        // first squad on this same step.
        assert_eq!(
            g.enemies.get(0).unwrap().phase,
            enemy::Phase::Dying,
            "the enemy survived the collision"
        );
        assert_eq!(g.score, enemy::LANDER_POINTS);
    }

    /// But not while the ship is still blinking back in.
    #[test]
    fn a_respawning_ship_passes_through_enemies() {
        let mut g = game();
        g.enemies.clear();
        let lives = g.lives.remaining;
        assert!(!g.lives.is_vulnerable());
        let mut l = enemy::Enemy::lander(g.ship.x, g.ship.y, 0.0);
        l.phase = enemy::Phase::Hovering;
        g.enemies.spawn(l);
        g.step(Input::default(), FIXED_DT);
        assert_eq!(g.lives.remaining, lives);
        assert_eq!(g.enemies.get(0).unwrap().phase, enemy::Phase::Hovering);
    }

    /// ★ LANDERS SHOOT — slowly. Through `step`, so the bolt really is
    /// built and really is the slow kind.
    #[test]
    fn a_lander_fires_a_slow_shot_at_the_ship() {
        let mut g = game();
        g.enemies.clear();
        g.people.clear();
        let mut l = enemy::Enemy::lander(g.ship.x + 300.0, g.ship.y + 100.0, 0.0);
        l.phase = enemy::Phase::Hovering;
        l.fire_cooldown = 0.0;
        g.enemies.spawn(l);
        g.step(Input::default(), FIXED_DT);
        let bolt = g.shots.iter().find(|s| s.is_enemy()).expect("the Lander did not fire");
        let speed = (bolt.vx * bolt.vx + bolt.vy * bolt.vy).sqrt();
        assert!((speed - enemy::LANDER_SHOT_SPEED).abs() < 1.0, "speed {speed}");
        assert!(bolt.vx < 0.0, "it should fire toward the ship");
    }

    /// ★ GAME OVER BANKS THE SCORE ONCE, AND ENTER STARTS AGAIN. A score
    /// file that recorded every frame of the game-over card would fill
    /// the table with one run.
    #[test]
    fn game_over_banks_once_and_enter_starts_a_fresh_game() {
        let mut g = game();
        g.score = 4_321;
        g.lives.remaining = 0;
        g.lives.state = lives::State::GameOver;
        for _ in 0..30 {
            g.step(Input::default(), FIXED_DT);
        }
        assert_eq!(g.scores.entries.len(), 1, "banked more than once");
        assert_eq!(g.scores.best(), Some(4_321));
        assert_eq!(g.best, 4_321);
        assert_eq!(g.director.reserve(), waves::landers(1), "a finished game still ran waves");

        // Mid-game Enter does nothing; on game over it restarts.
        g.on_input(InputEvent::KeyDown(Key::Enter));
        assert_eq!(g.score, 0);
        assert!(!g.lives.is_game_over());
        assert_eq!(g.lives.remaining, lives::STARTING_LIVES);
        assert_eq!(g.director.wave(), 1);
        assert_eq!(g.smart_bombs, STARTING_SMART_BOMBS);
        assert_eq!(g.people.alive(), POPULATION);
        g.score = 77;
        g.on_input(InputEvent::KeyDown(Key::Enter));
        assert_eq!(g.score, 77, "Enter mid-game wiped a game in progress");
        assert_eq!(g.scores.entries.len(), 1, "the restart banked the score again");
    }

    /// ★ A DIFFERENT GAME EACH TIME. Two seeds, two worlds.
    #[test]
    fn different_seeds_give_different_worlds() {
        let a = game();
        let mut b = game();
        b.new_game(mix(0x0DEF_E4DE, 0x9E37_79B9));
        let xs = |g: &Warden| g.people.iter().map(|h| h.x).collect::<Vec<_>>();
        assert_ne!(xs(&a), xs(&b), "the people stood in the same places");
        assert_ne!(a.terrain.height_at(500.0), b.terrain.height_at(500.0));
    }

    /// A Lander parked, hovering and killable, at an offset from the ship.
    fn parked(g: &Warden, dx: f32, y: f32) -> enemy::Enemy {
        let mut l = enemy::Enemy::lander(g.ship.x + dx, y, 0.0);
        l.phase = enemy::Phase::Hovering;
        l
    }

    /// ★ B, THROUGH THE KEY: everything hostile on screen dies at full
    /// points; off-screen and above-screen enemies and the people do not.
    #[test]
    fn the_smart_bomb_clears_the_screen_and_only_the_screen() {
        let mut g = game();
        g.people.clear();
        g.lives.state = lives::State::Alive;
        let y = g.ship.y;
        g.enemies.spawn(parked(&g, 200.0, y)); // on screen
        g.enemies.spawn(parked(&g, 400.0, y + 120.0)); // on screen
        g.enemies.spawn(parked(&g, world::VIEW_W * 2.0, y)); // far away
        g.enemies.spawn(parked(&g, 300.0, world::VIEW_H * 1.3)); // above the screen
        let hx = world::wrap(g.ship.x + 250.0);
        g.people.spawn(humanoid::Humanoid::new(hx, g.terrain.height_at(hx), 0.0));
        let bombs = g.smart_bombs;

        g.on_input(InputEvent::KeyDown(Key::B));

        let phase = |g: &Warden, i| g.enemies.get(i).unwrap().phase;
        assert_eq!(phase(&g, 0), enemy::Phase::Dying);
        assert_eq!(phase(&g, 1), enemy::Phase::Dying);
        assert_eq!(phase(&g, 2), enemy::Phase::Hovering, "the bomb reached past the screen");
        assert_eq!(phase(&g, 3), enemy::Phase::Hovering, "the bomb reached above the screen");
        assert_eq!(g.people.alive(), 1, "the bomb killed a person");
        assert_eq!(g.score, 2 * enemy::LANDER_POINTS);
        assert_eq!(g.smart_bombs, bombs - 1);
        assert!(g.blast.is_some(), "no blast");
        assert!(g.bomb_pending, "the bomb's voice was not queued");
    }

    /// No bombs, no bomb — and not while paused or between places.
    #[test]
    fn the_smart_bomb_needs_a_bomb_and_a_ship() {
        let mut g = game();
        g.lives.state = lives::State::Alive;
        let y = g.ship.y;
        g.enemies.spawn(parked(&g, 200.0, y));

        g.smart_bombs = 0;
        g.on_input(InputEvent::KeyDown(Key::B));
        assert_eq!(g.enemies.get(0).unwrap().phase, enemy::Phase::Hovering, "fired with none left");

        g.smart_bombs = 2;
        g.on_input(InputEvent::KeyDown(Key::P));
        g.on_input(InputEvent::KeyDown(Key::B));
        assert_eq!(g.smart_bombs, 2, "fired while paused");
        g.on_input(InputEvent::KeyDown(Key::P));

        g.hyperspace = Some(0.0);
        g.on_input(InputEvent::KeyDown(Key::B));
        assert_eq!(g.smart_bombs, 2, "fired from hyperspace");
    }

    /// The flash is four flashes and then it is over.
    #[test]
    fn the_flash_flashes_and_ends() {
        let mut g = game();
        g.lives.state = lives::State::Alive;
        g.on_input(InputEvent::KeyDown(Key::B));
        let mut lit = Vec::new();
        let mut t = 0.0;
        while t < BLAST_SECONDS + 0.1 {
            g.step(Input::default(), FIXED_DT);
            lit.push(g.flash_on());
            t += FIXED_DT;
        }
        let rises = lit.windows(2).filter(|w| !w[0] && w[1]).count() + usize::from(lit[0]);
        assert_eq!(rises, 4, "expected four flashes");
        assert!(g.blast.is_none(), "the blast never ended");
    }

    /// ★ H, THROUGH THE KEY: somewhere else, stopped dead, enemy bolts
    /// gone, your own still flying — and untouchable until it arrives.
    #[test]
    fn hyperspace_jumps_stops_and_clears_enemy_fire() {
        let mut g = game();
        g.enemies.clear();
        g.lives.state = lives::State::Alive;
        g.ship.vx = 400.0;
        let from = g.ship.x;
        g.shots.fire(g.ship.x, g.ship.y, 1.0);
        g.shots.fire_enemy(g.ship.x + 300.0, g.ship.y, -1.0, 0.3);

        g.on_input(InputEvent::KeyDown(Key::H));

        assert!(g.hyperspace.is_some());
        assert!(g.hyper_pending, "the jump's voice was not queued");
        assert!(world::delta(from, g.ship.x).abs() >= world::VIEW_W, "it stayed on the same screen");
        assert_eq!(g.ship.vx, 0.0, "it kept its speed");
        assert!(g.shots.iter().all(|s| !s.is_enemy()), "enemy bolts survived the jump");
        assert_eq!(g.shots.iter().count(), 1, "your own bolt was taken too");

        // An enemy bolt right on the ship mid-jump does nothing.
        let lives = g.lives.remaining;
        g.shots.fire_enemy(g.ship.x, g.ship.y, 1.0, 0.0);
        g.step(Input::default(), FIXED_DT);
        assert_eq!(g.lives.remaining, lives, "shot down while between places");
        assert!(!g.flying(), "a jumping ship should not fly");

        let mut t = 0.0;
        while g.hyperspace.is_some() && t < 2.0 {
            g.step(Input::default(), FIXED_DT);
            t += FIXED_DT;
        }
        assert!((t - HYPERSPACE_SECONDS).abs() < 0.02, "arrived after {t:.3} s");
    }

    /// ★ BRIAN'S NOTE, AS A TEST: a jump always lands somewhere else —
    /// at least a screen from where it started, over many jumps.
    #[test]
    fn hyperspace_always_leaves_the_screen_you_were_on() {
        let mut g = game();
        g.enemies.clear();
        let mut nearest = f32::MAX;
        for _ in 0..500 {
            g.lives.reset();
            let from = g.ship.x;
            g.on_input(InputEvent::KeyDown(Key::H));
            nearest = nearest.min(world::delta(from, g.ship.x).abs());
            g.hyperspace = None;
        }
        assert!(nearest >= world::VIEW_W, "one jump landed only {nearest:.0} away");
    }

    /// ★ THE JUMP SOUNDS AS LONG AS THE JUMP: the voice ends as the ship
    /// finishes coming back together, measured from a real render.
    #[test]
    fn the_hyperspace_voice_lasts_exactly_the_jump() {
        use omarcade_core::Voice;
        let mut v = sound::Hyperspace::new();
        v.retrigger(1.0, 1.0);
        let sr = 48_000.0;
        let mut out = Vec::new();
        let mut block = [0.0f32; 256];
        while v.alive() && out.len() < 96_000 {
            v.render(&mut block, VoiceParams::SILENT, sr);
            out.extend_from_slice(&block);
        }
        let end = out.iter().rposition(|x| x.abs() > 1e-4).unwrap() as f32 / sr;
        assert!((end - HYPERSPACE_SECONDS).abs() < 0.03, "voice {end:.3} s, jump {HYPERSPACE_SECONDS:.3} s");
    }

    /// ★ THE GAMBLE, MEASURED: about one jump in four ends in an
    /// explosion — Brian's "keep the hyperspace death chance" — and the
    /// respawn blink does not protect against it.
    #[test]
    fn hyperspace_kills_about_one_jump_in_four_even_while_blinking() {
        let mut g = game();
        g.enemies.clear();
        let trials = 4000;
        let mut deaths = 0;
        for _ in 0..trials {
            g.lives.reset();
            g.lives.remaining = 1_000;
            assert!(!g.lives.is_vulnerable(), "should start blinking");
            let before = g.lives.remaining;
            g.on_input(InputEvent::KeyDown(Key::H));
            while g.hyperspace.is_some() {
                g.step(Input::default(), FIXED_DT);
            }
            if g.lives.remaining < before {
                deaths += 1;
            }
        }
        let rate = deaths as f32 / trials as f32;
        assert!((rate - HYPERSPACE_DEATH).abs() < 0.03, "died on {rate:.3} of jumps");
    }

    /// ★ W3 THROUGH THE REAL LOOP: wave 2 opens with its Bombers.
    #[test]
    fn wave_two_brings_its_bombers() {
        let mut g = game();
        immortal(&mut g);
        // Wave 1 has none — checked every frame of it, not just at the end.
        play_until(&mut g, 200.0, |g| {
            if g.director.wave() == 1 {
                assert_eq!(g.enemies.count(enemy::Kind::Bomber), 0, "a Bomber in wave 1");
            }
            g.director.wave() == 2
        });
        let mut audio = AudioSystem::new();
        for _ in 0..30 {
            let mut a = audio.handle();
            g.update(1.0 / 60.0, &mut a);
        }
        assert_eq!(g.enemies.count(enemy::Kind::Bomber), waves::BOMBERS[1]);
    }

    /// ★ A MINE CANNOT BE SHOT, ONLY AVOIDED: flying into one costs a life.
    #[test]
    fn flying_into_a_mine_costs_a_life() {
        let mut g = game();
        g.enemies.clear();
        g.lives.state = lives::State::Alive;
        let lives = g.lives.remaining;
        g.mines.lay(g.ship.x + 10.0, g.ship.y);
        g.step(Input::default(), FIXED_DT);
        assert_eq!(g.lives.remaining, lives - 1, "the mine did nothing");
        assert_eq!(g.mines.len(), 0, "the mine that went off is still there");
    }

    /// And your laser passes straight through one.
    #[test]
    fn a_mine_cannot_be_shot() {
        let mut g = game();
        g.enemies.clear();
        g.people.clear();
        g.mines.lay(g.ship.x + 200.0, g.ship.y);
        g.shots.fire(g.ship.x + 200.0, g.ship.y, 1.0);
        g.resolve_hits();
        assert_eq!(g.mines.len(), 1);
        assert_eq!(g.score, 0);
    }

    /// ★ THE BAITERS LEAVE WHEN THE WAVE IS WON — Brian's "Vanish when all
    /// Landers die" — so one left alive cannot hold the wave open.
    #[test]
    fn a_won_wave_sends_its_baiters_away() {
        let mut g = game();
        immortal(&mut g);
        let mut b = enemy::Enemy::baiter(g.ship.x + 300.0, g.ship.y);
        b.phase = enemy::Phase::Hovering;
        g.enemies.spawn(b);
        let mut audio = AudioSystem::new();
        let mut t = 0.0;
        while g.director.wave() == 1 && t < 200.0 {
            {
                let mut a = audio.handle();
                g.update(1.0 / 60.0, &mut a);
            }
            // ⚠️ KEEP THE SHIP OUT OF REACH: a Baiter that rams the ship
            // dies in the collision, which makes it "gone" for the wrong
            // reason — it did, and hid a missing dismissal from this
            // test. Holding the ship mid-hyperspace takes it out of play
            // entirely (holding the respawn blink did not: the blink
            // expires partway through a frame).
            g.hyperspace = Some(0.0);
            // Kill everything EXCEPT Baiters.
            for i in 0..g.enemies.len() {
                if g.enemies.get(i).is_some_and(|e| e.is_target() && e.kind != enemy::Kind::Baiter) {
                    g.destroy_enemy(i);
                }
            }
            t += 1.0 / 60.0;
        }
        assert_eq!(g.director.wave(), 2, "a Baiter held the wave open");
        assert_eq!(g.enemies.count(enemy::Kind::Baiter), 0, "the Baiter stayed");
    }

    /// The smart bomb takes Baiters and Bombers too, at their own points.
    #[test]
    fn the_smart_bomb_scores_baiters_and_bombers() {
        let mut g = game();
        g.lives.state = lives::State::Alive;
        let mut b = enemy::Enemy::baiter(g.ship.x + 200.0, g.ship.y);
        b.phase = enemy::Phase::Hovering;
        let mut m = enemy::Enemy::bomber(g.ship.x + 400.0, g.ship.y, 0.0, 400.0);
        m.phase = enemy::Phase::Hovering;
        g.enemies.spawn(b);
        g.enemies.spawn(m);
        g.on_input(InputEvent::KeyDown(Key::B));
        assert_eq!(g.score, enemy::BAITER_POINTS + enemy::BOMBER_POINTS);
    }

    /// ★ THE SCREAM RUNS WHILE SOMEONE FALLS, AND STOPS WHEN THEY ARE
    /// CAUGHT — through the real update, and each new fall is a new one.
    #[test]
    fn the_scream_runs_only_while_someone_is_falling() {
        let mut g = game();
        g.enemies.clear();
        g.people.clear();
        g.lives.state = lives::State::Alive;
        g.ship.y = 400.0;
        let mut audio = AudioSystem::new();
        let mut frame = |g: &mut Warden| {
            let mut a = audio.handle();
            g.update(1.0 / 60.0, &mut a);
        };
        frame(&mut g);
        assert!(!g.scream_should_run(), "screaming with nobody falling");

        // Someone dropped well away from the ship.
        let x = world::wrap(g.ship.x + 900.0);
        let mut h = humanoid::Humanoid::new(x, g.terrain.height_at(x) + 400.0, 0.0);
        h.state = humanoid::State::Falling;
        h.fell_from = h.y;
        g.people.spawn(h);
        frame(&mut g);
        assert!(g.scream_should_run(), "no scream for a fall");
        let first = g.falls;

        // Caught: the scream stops.
        g.people.get_mut(0).unwrap().rescued();
        frame(&mut g);
        assert!(!g.scream_should_run(), "still screaming after the catch");

        // A second drop is a new fall.
        let mut h2 = humanoid::Humanoid::new(x, g.terrain.height_at(x) + 400.0, 0.0);
        h2.state = humanoid::State::Falling;
        h2.fell_from = h2.y;
        g.people.spawn(h2);
        frame(&mut g);
        assert!(g.falls > first, "a second fall did not restart the scream");
    }

    /// ★ A MUTANT'S SHOT AND A LANDER'S ARE DIFFERENT SOUNDS — "a
    /// different sound when they shoot" — through `step`.
    #[test]
    fn a_mutant_and_a_lander_fire_with_different_voices() {
        let mut g = game();
        g.enemies.clear();
        g.people.clear();
        let mut l = enemy::Enemy::lander(g.ship.x + 300.0, g.ship.y + 100.0, 0.0);
        l.phase = enemy::Phase::Hovering;
        l.fire_cooldown = 0.0;
        g.enemies.spawn(l);
        g.step(Input::default(), FIXED_DT);
        assert!(g.lander_fired_this_frame && !g.mutant_fired_this_frame);

        g.lander_fired_this_frame = false;
        g.enemies.clear();
        let mut m = enemy::Enemy::mutant(g.ship.x + 300.0, g.ship.y + 100.0);
        m.fire_cooldown = 0.0;
        g.enemies.spawn(m);
        g.step(Input::default(), FIXED_DT);
        assert!(g.mutant_fired_this_frame && !g.lander_fired_this_frame);
    }

    #[test]
    fn escape_quits_and_nothing_else_does() {
        let mut g = game();
        assert!(!g.on_input(InputEvent::KeyDown(Key::Escape)), "Esc must quit");

        let mut g = game();
        for k in [Key::Space, Key::P, Key::Left, Key::Right, Key::Up, Key::Down, Key::T] {
            assert!(g.on_input(InputEvent::KeyDown(k)), "{k:?} must not quit");
            assert!(g.on_input(InputEvent::KeyUp(k)), "{k:?} release must not quit");
        }
    }

    #[test]
    fn pause_stops_the_ship_and_resume_releases_it() {
        let mut g = game();
        g.on_input(InputEvent::KeyDown(Key::T));
        let mut audio = AudioSystem::new();
        {
            let mut a = audio.handle();
            g.update(0.5, &mut a);
        }
        let moving = g.ship.vx;
        assert!(moving > 0.0, "thrust should have accelerated the ship");

        g.on_input(InputEvent::KeyDown(Key::P));
        let at_pause = g.ship.vx;
        {
            let mut a = audio.handle();
            g.update(0.5, &mut a);
        }
        assert_eq!(g.ship.vx, at_pause, "a paused ship must not move");

        g.on_input(InputEvent::KeyDown(Key::P));
        {
            let mut a = audio.handle();
            g.update(0.1, &mut a);
        }
        assert_ne!(g.ship.vx, at_pause, "resuming must let it fly again");
    }

    /// ★ BRIAN'S FIRST-FLIGHT LOCK-UP: "locked up firing, continuous
    /// firing and couldn't move". Wayland drops focus without sending a
    /// release, so this drives the game through `HeldKeys` exactly as
    /// the backend does — observe each delivered event, then on focus
    /// loss deliver whatever `release_all` hands back — and checks the
    /// laser actually stops.
    #[test]
    fn losing_focus_while_firing_stops_the_laser() {
        let mut g = game();
        let mut held = omarcade_core::backend::HeldKeys::default();
        let mut deliver = |g: &mut Warden, e: InputEvent| {
            held.observe(e);
            g.on_input(e);
        };
        deliver(&mut g, InputEvent::KeyDown(Key::Space));
        deliver(&mut g, InputEvent::KeyDown(Key::T));
        let mut audio = AudioSystem::new();
        {
            let mut a = audio.handle();
            g.update(0.5, &mut a);
        }
        let mine = |g: &Warden| g.shots.iter().filter(|s| !s.is_enemy()).count();
        assert!(mine(&g) > 0, "holding Space should have fired");

        // Focus leaves. No KeyUp comes from the platform.
        for e in held.release_all() {
            g.on_input(e);
        }
        assert!(!g.fire_held && !g.thrust_held, "focus loss must release held keys");

        // Long enough for every shot in flight to expire, so any shot
        // left is one fired after focus went. In slices: `update` caps
        // one call at 0.25 s.
        for _ in 0..10 {
            let mut a = audio.handle();
            g.update(0.2, &mut a);
        }
        assert_eq!(mine(&g), 0, "the laser kept firing after focus was lost");
    }

    /// ⚠️ THE S11 RULE, AS A TEST. Swallowing KeyUp while paused leaves
    /// the key held across the pause, and the ship flies off on resume
    /// because the release was eaten.
    #[test]
    fn a_key_released_during_a_pause_is_still_released() {
        let mut g = game();
        g.on_input(InputEvent::KeyDown(Key::T));
        assert!(g.thrust_held);

        g.on_input(InputEvent::KeyDown(Key::P));
        g.on_input(InputEvent::KeyUp(Key::T));
        assert!(!g.thrust_held, "the release must land even while paused");

        g.on_input(InputEvent::KeyDown(Key::P));
        let mut audio = AudioSystem::new();
        let mut a = audio.handle();
        g.update(0.5, &mut a);
        assert_eq!(g.ship.vx, 0.0, "the ship must not thrust after a released key");
    }

    /// ⚠️⚠️ REGRESSION: MUTANTS USED TO SHOOT THEMSELVES DEAD.
    ///
    /// Brian: "mutants appear and after about 2 seconds they just self
    /// destruct." An enemy bolt spawns inside its shooter's own hitbox,
    /// and `resolve_hits` tested EVERY shot against the enemy list
    /// regardless of who fired it — so the frame after a Mutant fired,
    /// its own bolt found it and killed it.
    ///
    /// ⚠️ THE TESTS IN `enemy` COULD NOT CATCH THIS. They step the
    /// Landers directly and never build a Shot, so the whole collision
    /// path was outside them. The bug lived in the one file that only
    /// the real game exercises, which is exactly where it hid.
    #[test]
    fn an_enemy_bolt_does_not_kill_the_enemy_that_fired_it() {
        let mut g = game();
        g.people.clear();
        g.enemies.clear();

        // A Mutant sitting next to the ship, and its own bolt right on
        // top of it — the exact geometry of the frame after it fires.
        let (mx, my) = (g.ship.x + 200.0, g.ship.y);
        g.enemies.spawn(enemy::Enemy::mutant(mx, my));
        assert_eq!(g.enemies.len(), 1);

        g.shots.fire_enemy(mx, my, 1.0, 0.3);
        g.resolve_hits();

        assert_eq!(
            g.enemies.len(),
            1,
            "a Mutant shot itself dead with its own bolt"
        );
        assert_eq!(g.score, 0, "and scored the player points for it");
    }

    /// The other half of the same rule: YOUR bolt must still kill them.
    #[test]
    fn your_own_bolt_still_kills_a_mutant() {
        let mut g = game();
        g.people.clear();
        g.enemies.clear();

        let (mx, my) = (g.ship.x + 200.0, g.ship.y);
        g.enemies.spawn(enemy::Enemy::mutant(mx, my));
        g.shots.fire(mx, my, 1.0);
        g.resolve_hits();

        assert_eq!(g.score, enemy::MUTANT_POINTS, "your shot did not connect");
    }

    /// And an enemy bolt must not shoot the civilians either.
    #[test]
    fn an_enemy_bolt_does_not_kill_humanoids() {
        let mut g = game();
        g.enemies.clear();
        g.people.clear();
        let (hx, hy) = (g.ship.x + 300.0, g.terrain.height_at(g.ship.x + 300.0));
        g.people.spawn(humanoid::Humanoid::new(hx, hy, 0.0));

        g.shots.fire_enemy(hx, hy, 1.0, 0.3);
        g.resolve_hits();

        assert_eq!(g.people.alive(), 1, "an alien shot its own cargo");
    }

    #[test]
    fn both_vertical_keys_cancel() {
        let mut g = game();
        g.on_input(InputEvent::KeyDown(Key::Up));
        assert_eq!(g.input().vertical, -1.0);
        g.on_input(InputEvent::KeyDown(Key::Down));
        assert_eq!(g.input().vertical, 0.0, "both held should cancel");
        g.on_input(InputEvent::KeyUp(Key::Up));
        assert_eq!(g.input().vertical, 1.0);
    }

    #[test]
    fn the_arrows_set_facing_without_moving_the_ship() {
        let mut g = game();
        assert_eq!(g.ship.facing, Facing::East);
        g.on_input(InputEvent::KeyDown(Key::Left));
        assert_eq!(g.ship.facing, Facing::West);
        assert_eq!(g.ship.vx, 0.0, "facing is not thrust");
    }

    /// A long frame must not be simulated in one enormous step.
    #[test]
    fn a_stalled_frame_is_clamped() {
        let mut g = game();
        g.on_input(InputEvent::KeyDown(Key::T));
        let mut audio = AudioSystem::new();
        let mut a = audio.handle();
        g.update(30.0, &mut a);
        // 30 seconds of thrust would have lapped the world many times;
        // the clamp means at most a quarter second was simulated.
        assert!(
            g.ship.vx <= flight::TOP_SPEED + 0.001,
            "speed escaped the clamp: {}",
            g.ship.vx
        );
    }

    // ----- W4 -----

    /// ★ THE SMART BOMB AND THE POD: the Pod dies at full points, and the
    /// Swarmers it lets out SURVIVE the bomb that freed them (the
    /// original's "they aren't drawn yet").
    #[test]
    fn swarmers_from_a_bombed_pod_survive_the_bomb() {
        let mut g = game();
        g.lives.state = lives::State::Alive;
        g.enemies.clear();
        let mut p = enemy::Enemy::pod(g.ship.x + 200.0, g.ship.y, 0.0, 30.0);
        p.phase = enemy::Phase::Hovering;
        g.enemies.spawn(p);
        g.on_input(InputEvent::KeyDown(Key::B));
        assert_eq!(g.score, enemy::POD_POINTS, "only the Pod scores");
        assert!(g.enemies.count(enemy::Kind::Swarmer) > 0, "the bomb took the Swarmers too");
    }

    /// ★ THROUGH THE REAL LOOP: wave 2 opens with its one Pod, and wave 1
    /// never has one.
    #[test]
    fn wave_two_brings_its_pod() {
        let mut g = game();
        immortal(&mut g);
        play_until(&mut g, 200.0, |g| {
            if g.director.wave() == 1 {
                assert_eq!(g.enemies.count(enemy::Kind::Pod), 0, "a Pod in wave 1");
            }
            g.director.wave() == 2
        });
        let mut audio = AudioSystem::new();
        for _ in 0..30 {
            let mut a = audio.handle();
            g.update(1.0 / 60.0, &mut a);
        }
        assert_eq!(g.enemies.count(enemy::Kind::Pod), waves::PODS[1]);
    }

    /// ★ THE LASER BURSTS A POD, through the real update: a beam fired
    /// at it kills it, and its Swarmers are in the sky.
    #[test]
    fn shooting_a_pod_lets_out_its_swarmers() {
        let mut g = game();
        immortal(&mut g);
        g.enemies.clear();
        let ahead = g.ship.facing.sign() * 150.0;
        let mut p = enemy::Enemy::pod(world::wrap(g.ship.x + ahead), g.ship.y, 0.0, 0.0);
        p.phase = enemy::Phase::Hovering;
        g.enemies.spawn(p);
        g.on_input(InputEvent::KeyDown(Key::Space));
        let mut audio = AudioSystem::new();
        for _ in 0..30 {
            let mut a = audio.handle();
            g.update(1.0 / 60.0, &mut a);
            if g.enemies.count(enemy::Kind::Swarmer) > 0 {
                break;
            }
        }
        assert_eq!(g.enemies.count(enemy::Kind::Pod), 0, "the Pod survived the beam");
        assert!(g.score >= enemy::POD_POINTS);
        assert!(g.enemies.count(enemy::Kind::Swarmer) > 0, "no Swarmers came out");
    }

    /// ★ THE BAITERS ARE TOLD THE TRUTH, through the real update: the
    /// wave's re-aim chance, and the ship's climb as it actually happens.
    #[test]
    fn baiters_chase_the_real_ship_and_the_waves_keenness() {
        let mut g = game();
        immortal(&mut g);
        g.on_input(InputEvent::KeyDown(Key::Up));
        let mut audio = AudioSystem::new();
        for _ in 0..10 {
            let mut a = audio.handle();
            g.update(1.0 / 60.0, &mut a);
        }
        let c = g.enemies.chase();
        assert_eq!(c.seek, g.director.baiter_seek());
        assert!(c.seek < 0.5, "seek {} is not wave 1's", c.seek);
        assert!(c.vy > flight::CLIMB_SPEED * 0.5, "climbing ship reported vy {}", c.vy);
    }

    /// ★ THE ORIGINAL'S PACE, THROUGH THE REAL LOOP: with nothing killed,
    /// wave 1's second squad still arrives on its 7.5 s timer, because the
    /// cap is the original's 8 — ten Landers hunting by 9 s, not five.
    #[test]
    fn the_second_squad_comes_on_its_timer_without_a_kill() {
        let mut g = game();
        immortal(&mut g);
        let mut audio = AudioSystem::new();
        for _ in 0..(9 * 60) {
            let mut a = audio.handle();
            g.update(1.0 / 60.0, &mut a);
        }
        assert_eq!(g.enemies.count(enemy::Kind::Lander), 2 * waves::SQUAD_SIZE);
    }
}
