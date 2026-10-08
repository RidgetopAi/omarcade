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

/// What a Baiter and a Bomber are worth (`a7f355dd`, Brian's chart).
pub const BAITER_POINTS: u32 = 200;
pub const BOMBER_POINTS: u32 = 250;
/// ★ W4. A Pod is the most valuable thing in the sky (`PRBKIL: KILO
/// $0210`), and a Swarmer is worth a Mutant (`MSWKIL: LDD #$0115`).
pub const POD_POINTS: u32 = 1000;
pub const SWARMER_POINTS: u32 = 150;

// ---------------------------------------------------------------------
// W3: the Baiter
// ---------------------------------------------------------------------

/// How much faster than the SHIP a Baiter closes, world units per second.
///
/// ★ THE ORIGINAL'S RULE: X velocity = the player's own X velocity ± 2 px
/// per frame toward the player (research-gameplay.md §1). At 3.16 units
/// per original pixel and 60 Hz that is ~380 u/s on top of whatever the
/// ship is doing — so it cannot be outrun, which is the point of it.
pub const BAITER_MARGIN: f32 = 380.0;

/// Inside this horizontal distance a Baiter stops re-aiming sideways and
/// keeps the speed it has (the original's ±20 px) — so it overshoots.
pub const BAITER_CLOSE: f32 = 63.0;

/// ★ THE ORIGINAL'S WINDOW: A BAITER ONLY RECONSIDERS NOW AND THEN.
/// Every 18 frames (its 3-image cycle at `NAP 6`) it rolls a byte, and
/// re-aims only if the byte beats `UFOSK` — about 1 time in 5 at the
/// start of wave 1 ([`crate::waves::baiter_seek`]). Between re-aims it
/// flies on its old heading: it overshoots, drifts, and that drift is
/// where a player lines up a shot or slips away. (Ours used to re-aim
/// every ~0.4 s, and at once on any overshoot — glued to you, Brian found.)
pub const BAITER_SEEK_PERIOD: f32 = 18.0 / 60.0;

/// When it re-aims and is more than 10 lines off your altitude, its
/// vertical speed becomes (your vertical speed + 1 line a frame toward
/// you) ÷ 2 (UFONV). Holding still, that closes at 90 u/s.
pub const BAITER_VSEEK: f32 = 3.0 * 60.0;
/// Inside this altitude gap it leaves its vertical speed alone (10 lines).
pub const BAITER_Y_CLOSE: f32 = 30.0;

/// How often a Baiter fires, seconds at pressure 1.0 — "fires often".
/// Brian's spec: "shoot faster THAN YOU". Its shots are faster than a
/// Lander's and slower than a Mutant's.
pub const BAITER_FIRE_INTERVAL: f32 = 0.9;
pub const BAITER_SHOT_SPEED: f32 = 640.0;

// ---------------------------------------------------------------------
// W3: the Bomber
// ---------------------------------------------------------------------

/// How far above or below the ship a Bomber holds itself while on screen
/// (the original's 16–32 lines, at 3 units a line).
pub const BOMBER_NEAR: f32 = 48.0;
pub const BOMBER_FAR: f32 = 96.0;

/// A Bomber's vertical push and drag — the original adds ±$10/256 line
/// per frame and bleeds 1/32 of its speed a frame (TIE, defb6.src):
/// 675 u/s² against a 1.9/s drag.
pub const BOMBER_ACCEL: f32 = 675.0;
pub const BOMBER_DRAG: f32 = 1.9;

/// Mines laid per second by one Bomber while it is on screen.
///
/// ⚠️ OURS. The original rolls 1 in 8 each time it visits a squad member
/// (TIE31) and only in the on-screen branch — that much is certain, and
/// kept: a Bomber lays only where you can see it. The tick rate behind
/// "1 in 8" cannot be pinned without tracing the scheduler, so the rate
/// is set by feel: a few mines a second makes the FIELD Brian's spec
/// names ("LEAVE MINE FIELDS behind… kill bombers before they lay").
pub const BOMBER_MINE_RATE: f32 = 1.4;

/// Horizontal distance within which a Bomber counts as on screen.
pub const BOMBER_ON_SCREEN: f32 = world::VIEW_W * 0.55;

// ---------------------------------------------------------------------
// W4: the Pod and the Swarmer (PRBST, PRBKIL, MMSW, MSWM, SWBMB in
// defb6.src)
// ---------------------------------------------------------------------

/// One original pixel a frame, measured SCREEN-relatively: 3.16 units per
/// original pixel (the Baiter's and the Bomber's scale) at 60 Hz. The Pod
/// and the Swarmer are things you meet on screen, so their speeds are
/// read against the screen, not against a lap of the world.
const SCREEN_PX_PER_FRAME: f32 = 3.16 * 60.0;

/// One original line a frame, vertically: 3 units a line (the Lander's
/// scale, [`LANDER_VSPEED`]) at 60 Hz.
const LINE_PER_FRAME: f32 = 3.0 * 60.0;

/// A Pod drifts at a random X speed up to 1 px a frame either way (PRBST:
/// `SEED & $3F - $20`, in 32nds)…
pub const POD_DRIFT_MAX: f32 = SCREEN_PX_PER_FRAME;
/// …and a random Y speed of 32–64 256ths of a line a frame, never less
/// (`ORB #$20` / `ANDB #$DF` force the floor): 22.5–45 u/s.
pub const POD_VY_MIN: f32 = 32.0 / 256.0 * LINE_PER_FRAME;
pub const POD_VY_MAX: f32 = 64.0 / 256.0 * LINE_PER_FRAME;

/// Swarmers alive at once, at most (MMSW: `CMPA #20`). A Pod that bursts
/// with the sky already full lets out only what fits.
pub const MAX_SWARMERS: usize = 20;

/// How many Swarmers a Pod lets out: the original's `RMAX(6)`, which is
/// NOT an even 1–6. RMAX halves a random byte until it is ≤ 6 and adds
/// one, so it lands on 4–7 for 253 of 256 bytes — about 5.5 a Pod.
pub fn burst_count(byte: u8) -> usize {
    let mut r = byte;
    while r > 6 {
        r >>= 1;
    }
    r as usize + 1
}

/// The burst: each Swarmer flies out at a random velocity (RANDV) — up to
/// 1 px a frame sideways and 1 line a frame up or down…
pub const SWARMER_BURST_VX: f32 = SCREEN_PX_PER_FRAME;
pub const SWARMER_BURST_VY: f32 = LINE_PER_FRAME;
/// …for a random 0–31 frames before it turns on you (`PTIME = HSEED &
/// $1F`). That stagger is what makes a burst a scatter rather than a
/// formation.
pub const SWARMER_SCATTER_MAX: f32 = 31.0 / 60.0;

/// A Swarmer's chase speed at pressure 1.0: `SWXV`, $20 in wave 1 with
/// the starting difficulty applied, which is 1 px a frame.
pub const SWARMER_SPEED: f32 = SCREEN_PX_PER_FRAME;

/// ★ BRIAN'S TIP, AND THE ORIGINAL'S RULE: FLY CLOSE BEHIND THEM AND THEY
/// DO NOT TURN. A Swarmer picks its direction toward you and keeps it
/// until it is more than 150 px past you (MSWM: `ADDD #150*32`, `CMPD
/// #300*32`). Inside that, it flies on.
pub const SWARMER_TURN_GAP: f32 = 150.0 * 3.16;

/// The undulation. Every 3 frames (`NAP 3`) a Swarmer's vertical speed
/// is pushed toward your altitude by its OWN fixed acceleration — a
/// random 0–31 256ths of a line a frame (`HSEED & SWAC`) — then damped by
/// 1/64, nudged by a random ±16 256ths, and capped at 2 lines a frame.
/// Bang-bang steering with that little damping overshoots, which is the
/// sine-like weave; a Swarmer that rolled a small acceleration barely
/// weaves at all.
pub const SWARMER_TICK: f32 = 3.0 / 60.0;
pub const SWARMER_ACCEL_MAX: f32 = 31.0 / 256.0 * LINE_PER_FRAME;
pub const SWARMER_NUDGE: f32 = 16.0 / 256.0 * LINE_PER_FRAME;
pub const SWARMER_VY_MAX: f32 = 2.0 * LINE_PER_FRAME;
pub const SWARMER_DAMPING: f32 = 1.0 / 64.0;

/// How often a Swarmer tries to fire, seconds, at pressure 1.0.
///
/// ⚠️ OURS, NOT THE ORIGINAL'S. SWSTIM re-arms every 0.55–1.05 s, which
/// with a pack of six is a hail; Brian's spec is "fires sometimes". Like
/// the original it only fires while HEADING FOR YOU and on screen, so a
/// Swarmer you are behind never shoots back.
pub const SWARMER_FIRE_INTERVAL: f32 = 2.4;

/// ★ A SWARMER SHOOTS FORWARD, NOT AT YOU (SWBMB): the shot flies the way
/// the Swarmer is going and drops to your altitude over the time it takes
/// to cover 256 original pixels. So its angle is set by your height
/// difference, and a ship that is behind the pack is never in its line.
pub const SWARMER_SHOT_REACH: f32 = 256.0 * 3.16;
/// ⚠️ OURS: the original's is 8× the Swarmer's speed, ~1500 u/s here.
/// The Baiter's speed keeps it in the family of dodgeable pellets.
pub const SWARMER_SHOT_SPEED: f32 = 640.0;

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

/// How often a Lander fires, in seconds, at pressure 1.0.
///
/// ★ BRIAN'S SPEC: Landers "fire at you but NOT aggressively — easily
/// avoided". Twice the Mutant's interval, and each shot also flies at
/// barely half a Mutant shot's speed ([`LANDER_SHOT_SPEED`]). A Lander
/// shot is a thing to notice and step out of; a Mutant shot is a thing
/// to fear.
pub const LANDER_FIRE_INTERVAL: f32 = 3.2;

/// How far away a Lander will bother shooting from — roughly the screen
/// it is on. The original only fired from objects on or near the screen.
pub const LANDER_FIRE_RANGE: f32 = world::VIEW_W * 0.6;

/// How fast a Lander's shot flies, world units per second. The Mutant's
/// is [`crate::shot::ENEMY_SHOT_SPEED`] (900).
pub const LANDER_SHOT_SPEED: f32 = 480.0;

/// How far off a Lander's aim is, at most, in radians either way.
///
/// The original adds ±16 px of error to every shot. Aimed but not
/// sniping: standing still is punished, moving is enough.
pub const LANDER_AIM_ERROR: f32 = 0.12;

/// How often a Lander's shot leads a moving ship rather than aiming
/// where it is: the original's SHOOT leads when SEED > 120, 135 of 256.
pub const LANDER_LEAD_CHANCE: f32 = 135.0 / 256.0;

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

/// The Baiter's and Bomber's hitboxes, DERIVED from their art's bounds so
/// the two cannot drift apart (docs/warden-plan.md recommendation 2).
pub const BAITER_HALF: (f32, f32) = scaled(crate::art::half_extents(&crate::art::BAITER_LAYERS));
pub const BOMBER_HALF: (f32, f32) = scaled(crate::art::half_extents(&crate::art::BOMBER_LAYERS));
pub const POD_HALF: (f32, f32) = scaled(crate::art::half_extents(&crate::art::POD_LAYERS));
pub const SWARMER_HALF: (f32, f32) = scaled(crate::art::half_extents(&crate::art::SWARMER_LAYERS));

const fn scaled(e: (f32, f32)) -> (f32, f32) {
    (e.0 * crate::art::SCALE, e.1 * crate::art::SCALE)
}

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

// ---------------------------------------------------------------------
// ★★ HOW A LANDER HUNTS — THE ORIGINAL'S WAY (defb6.src LANDST, LANDS0,
// LANDG, LANDF, GTARG), after Brian flew wave 1: "wave 1 seems really
// hard… ability to even get to the captured humanoids".
//
// MEASURED, before this change: every Lander hunted the NEAREST person
// and steered to them, so a whole squad grabbed at once, 4 s in, all
// round the world, and all five were gone by 13–15 s — on every seed.
// The original, simulated from its source (omarcade-reference/emulator/
// og_landers.py): first grab 7–24 s in, the rest one at a time, seconds
// apart. Its Landers:
//  · arrive at a random place at the TOP and sink to their hunting
//    height (LANDST: YMIN+2, LNDYV);
//  · drift at their OWN random speed and direction (RMAX(LNDXV));
//  · are handed the NEXT person in a list, wherever they are (GTARG);
//  · and do NOT steer toward them — they drift until they happen to
//    pass over them, and only then swoop (LANDS0).
// That last is the whole difference: an abduction is something that
// builds where you can see it coming, not something that happens
// everywhere at once.
// ---------------------------------------------------------------------

/// One original pixel-per-frame, in world units per second, measured
/// WORLD-RELATIVELY: the original's world is 2048 px round, ours 3840
/// units, so a speed that laps theirs in a given time laps ours in the
/// same time. Lap time is what decides when a drifting Lander reaches its
/// target, so it is the scale that keeps the original's timing.
const PX_PER_FRAME: f32 = world::WORLD_W / 2048.0 * 60.0;

/// A Lander's drift is a random 1..=32 thirty-seconds of a pixel a frame
/// (RMAX(LNDXV), LNDXV = $20 in wave 1): ~3.5 to 112.5 u/s, either way.
pub const DRIFT_MAX: f32 = PX_PER_FRAME;
pub const DRIFT_STEPS: u32 = 32;

/// The drift used where a single representative speed is wanted (tests,
/// diagnostic scenes): the middle of the range.
pub const DRIFT_SPEED: f32 = DRIFT_MAX * 0.5;

/// How fast a Lander sinks to its hunting height, swoops, and climbs
/// away with its captive: LNDYV, $70/256 of a line a frame in wave 1,
/// at 3 world units a line — 78.75 u/s. One speed for all three, as in
/// the original. (Was 150 down and 96 up.)
pub const LANDER_VSPEED: f32 = 0x70 as f32 / 256.0 * 60.0 * 3.0;

/// The swoop's slide onto its target: 1 px a frame (LANDG).
pub const GRAB_SLIDE: f32 = PX_PER_FRAME;

/// How close a drifting Lander must pass over its target to swoop: the
/// original compares 32 px buckets (LANDS0 `ANDA #$FC`).
pub const ALIGN_X: f32 = 32.0 * world::WORLD_W / 2048.0;

/// Where a Lander appears: near the top of the sky, as the original's
/// do (YMIN+2), so its arrival and its sink are both in view.
pub const ARRIVE_HEIGHT: f32 = world::VIEW_H * 0.9;

/// No Lander arrives closer than this to the player, either way.
pub const ARRIVE_CLEAR: f32 = world::VIEW_W * 0.35;

/// Where in its own slot of the world a Lander arrives: anywhere in the
/// middle 70%, so neighbours are at least 30% of a slot apart (~190 u
/// for a squad of 5) yet the gaps between them still vary.
pub const ARRIVE_JITTER: (f32, f32) = (0.15, 0.85);

/// The original's hover is a BAND, not a line (LANDS0: sink while above
/// GETALT−50, climb only once more than 20 lines below it, else hold).
/// 20 lines at 3 u a line. Over rising ground a Lander rides the bottom
/// of the band, over falling ground the top, so a squad hovers at
/// different heights and one altitude does not line them all up for the
/// laser — Brian: "hardly any landers will pick up a humanoid if you
/// just go down the line".
pub const HOVER_BAND: f32 = 20.0 * 3.0;

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
/// ⚠️ ONE STRUCT, MANY KINDS. Every enemy shares a position, a phase,
/// a death and a place in the scanner; what differs is how it moves and
/// what it is worth. A Mutant is the case that settled it: it IS a
/// Lander that has fused with a person, transformed in place, so the
/// two could never have been separate structs.
///
/// ★ EACH KIND OWNS ONE `step_*`, dispatched in [`Enemy::step`]. A new
/// enemy (Baiter, Bomber, Pod, Swarmer — docs/warden-plan.md W3, W4) is
/// a variant here, a `step_*`, a `points` arm and a `draw_*`. The
/// variants arrive with their behaviour rather than ahead of it: a kind
/// that exists but cannot move would be a match arm that pretends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Lander,
    Mutant,
    /// ★ W3. The hurry-up: matches your speed so it cannot be outrun,
    /// arrives on a timer that tightens the longer a wave drags on.
    Baiter,
    /// ★ W3. Drifts at a constant speed, holds near your altitude, never
    /// fires — and lays mines where you can see it.
    Bomber,
    /// ★ W4. Drifts, never fires, worth 1000 — and full of Swarmers.
    Pod,
    /// ★ W4. What a Pod lets out: a small, weaving chaser that comes in
    /// packs and only turns round once it is well past you.
    Swarmer,
}

#[derive(Debug, Clone, Copy)]
pub struct Enemy {
    pub kind: Kind,
    /// Seconds until this one can fire again.
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
    /// Vertical speed, world units per second. Baiters and Bombers only;
    /// Landers and Mutants move by phase.
    pub vy: f32,
    /// Baiter: seconds to its next re-aim. Bomber: its cruise altitude
    /// while off screen. Swarmer: time banked toward its next 3-frame
    /// tick, and before that, how long its scatter lasts.
    pub aux: f32,
    /// Baiter: the altitude offset it is holding from the ship.
    /// Swarmer: its own fixed vertical acceleration (how hard it weaves).
    pub bias: f32,
    /// Index of the Humanoid being hunted or carried, if any.
    ///
    /// ⚠️ AN INDEX, AND THE HUMANOID LIST MUST THEREFORE NEVER SHRINK.
    /// `Humanoids::step` deliberately keeps the dead in place for
    /// exactly this reason — a Vec that compacted on death would
    /// silently re-point every carrier at the wrong person.
    pub target: Option<usize>,
}

impl Enemy {
    /// A Lander, materialising.
    pub fn lander(x: f32, y: f32, vx: f32) -> Self {
        Self {
            kind: Kind::Lander,
            fire_cooldown: LANDER_FIRE_INTERVAL,
            x: world::wrap(x),
            y,
            vx,
            vy: 0.0,
            aux: 0.0,
            bias: 0.0,
            phase: Phase::Warping,
            elapsed: 0.0,
            target: None,
        }
    }

    /// A Baiter, warping in. Announced through `spawn` like every arrival,
    /// so Brian's Warp plays for it — his call: "the sound I just made we
    /// will use for spawns of landers and baiters".
    ///
    /// ⚠️ `aux` and `bias` start at zero ON PURPOSE: the first step aims.
    pub fn baiter(x: f32, y: f32) -> Self {
        Self { kind: Kind::Baiter, fire_cooldown: BAITER_FIRE_INTERVAL, ..Self::lander(x, y, 0.0) }
    }

    /// A Bomber, warping in, drifting at `vx` and cruising at `cruise`
    /// while off screen.
    pub fn bomber(x: f32, y: f32, vx: f32, cruise: f32) -> Self {
        Self { kind: Kind::Bomber, aux: cruise, ..Self::lander(x, y, vx) }
    }

    /// ★ W4. A Pod, warping in, drifting at `(vx, vy)`. Placed at the
    /// start of its wave, like the Bombers, through `spawn`.
    pub fn pod(x: f32, y: f32, vx: f32, vy: f32) -> Self {
        Self { kind: Kind::Pod, vy, ..Self::lander(x, y, vx) }
    }

    /// ★ W4. A Swarmer, already out — bursting from a Pod, it does not
    /// warp. It flies at `(vx, vy)` for `scatter` seconds, then turns on
    /// the ship; `accel` is how hard it weaves.
    pub fn swarmer(x: f32, y: f32, vx: f32, vy: f32, scatter: f32, accel: f32) -> Self {
        Self {
            kind: Kind::Swarmer,
            fire_cooldown: SWARMER_FIRE_INTERVAL,
            vy,
            aux: scatter,
            bias: accel,
            phase: Phase::Hovering,
            ..Self::lander(x, y, vx)
        }
    }

    /// A Mutant, already formed, outside the arrival machinery.
    ///
    /// ⚠️ NOT USED BY THE GAME ITSELF — every Mutant in play is a Lander
    /// that fused, or a squad member that warped in (`Enemies::squad`).
    /// It exists to stage one directly: the tests and dump_frame's
    /// `mutants` scene.
    #[allow(dead_code)]
    pub fn mutant(x: f32, y: f32) -> Self {
        Self {
            kind: Kind::Mutant,
            fire_cooldown: MUTANT_FIRE_INTERVAL,
            x: world::wrap(x),
            y,
            vx: 0.0,
            vy: 0.0,
            aux: 0.0,
            bias: 0.0,
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
            Kind::Baiter => BAITER_POINTS,
            Kind::Bomber => BOMBER_POINTS,
            Kind::Pod => POD_POINTS,
            Kind::Swarmer => SWARMER_POINTS,
        }
    }

    /// Half-width and half-height of this one's hitbox, world units.
    pub fn half_extents(&self) -> (f32, f32) {
        match self.kind {
            Kind::Lander | Kind::Mutant => (LANDER_HALF_W, LANDER_HALF_H),
            Kind::Baiter => BAITER_HALF,
            Kind::Bomber => BOMBER_HALF,
            Kind::Pod => POD_HALF,
            Kind::Swarmer => SWARMER_HALF,
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

    /// Advance this enemy. `prey` is where its target is, if it has one
    /// and that target is still grabbable.
    ///
    /// ⚠️ THE HUMANOID IS PASSED IN RATHER THAN REACHED FOR. A Lander
    /// that held a reference to the Humanoid list could not be stepped
    /// while that list was being mutated, and the borrow checker would
    /// push the whole thing into a shared-mutable shape it does not need.
    /// The owner resolves the target and hands over a position.
    ///
    /// `pressure` is the wave's [`crate::waves::pressure`]: speeds are
    /// multiplied by it and fire intervals divided by it. 1.0 is the
    /// tuning S5–S7 settled.
    #[allow(clippy::too_many_arguments)]
    fn step(
        &mut self,
        terrain: &Terrain,
        prey: Option<(f32, f32)>,
        ship: Option<(f32, f32)>,
        chase: Chase,
        noise: &mut u32,
        pressure: f32,
        dt: f32,
    ) -> Outcome {
        self.elapsed += dt;

        // ★ ARRIVING AND DYING ARE THE SAME FOR EVERY KIND, so they are
        // handled here once and the kinds below never have to remember
        // to stand still. Warping lives here rather than in the Lander's
        // own step because a squad arriving after the world has ended is
        // MUTANTS, and a Mutant that skipped this would never finish
        // arriving — untargetable for the rest of the wave.
        match self.phase {
            Phase::Dying => return Outcome::None,
            Phase::Warping => {
                if self.elapsed >= WARP_SECONDS {
                    self.phase = Phase::Hovering;
                    self.elapsed = 0.0;
                }
                return Outcome::None;
            }
            _ => {}
        }

        // ★ WHICH CREATURE, THEN HOW FAR THROUGH ITS BEHAVIOUR. A Mutant
        // ignores the abduction machine entirely — it already ate its
        // Humanoid and its only goal is the ship — so "which kind am I"
        // is answered here, before any kind looks at its phase.
        match self.kind {
            Kind::Lander => self.step_lander(terrain, prey, ship, noise, pressure, dt),
            Kind::Mutant => self.step_mutant(ship, noise, pressure, dt),
            Kind::Baiter => self.step_baiter(ship, chase, noise, pressure, dt),
            Kind::Bomber => self.step_bomber(terrain, ship, noise, dt),
            Kind::Pod => self.step_pod(terrain, dt),
            Kind::Swarmer => self.step_swarmer(ship, noise, pressure, dt),
        }
    }

    /// A Lander's whole behaviour: drift, hunt, grab, carry — and take
    /// the odd unhurried shot at the ship.
    fn step_lander(
        &mut self,
        terrain: &Terrain,
        prey: Option<(f32, f32)>,
        ship: Option<(f32, f32)>,
        noise: &mut u32,
        pressure: f32,
        dt: f32,
    ) -> Outcome {
        let mut outcome = Outcome::None;

        match self.phase {
            Phase::Hovering => {
                // ★ ITS OWN DRIFT, NOT A CHASE. It goes where it was going.
                self.x = world::wrap(self.x + self.vx * pressure * dt);

                // Follow the ridge rather than holding an absolute
                // height: a Lander at a fixed y would sink into a peak
                // and float high over a valley, and the terrain here has
                // real peaks. ★ At LNDYV, so a new arrival visibly SINKS
                // from the top to its hunting height.
                let want = terrain.height_at(self.x) + HOVER_HEIGHT;
                let v = LANDER_VSPEED * pressure * dt;
                if self.y > want {
                    self.y = (self.y - v).max(want);
                } else if self.y < want - HOVER_BAND {
                    self.y = (self.y + v).min(want - HOVER_BAND);
                }

                // Its person is handed out by the owner before this step
                // (`Enemies::step`, GTARG). ★ PASSING OVER THEM is the only
                // thing that turns a drift into a swoop.
                if let Some((px, _)) = prey {
                    if world::delta(self.x, px).abs() <= ALIGN_X {
                        self.phase = Phase::Hunting;
                        self.elapsed = 0.0;
                    }
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
                // ★ THE SWOOP (LANDG): slide onto them at a pixel a frame
                // and come down at LNDYV. Its own drift is remembered for
                // when it is shot off them and goes back to drifting.
                let gap = world::delta(self.x, px);
                if gap.abs() > GRAB_REACH_X {
                    let step = GRAB_SLIDE * pressure * dt;
                    self.x = world::wrap(self.x + gap.signum() * step.min(gap.abs()));
                }

                let want_y = py + GRAB_HEIGHT;
                if self.y > want_y {
                    self.y = (self.y - LANDER_VSPEED * pressure * dt).max(want_y);
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
                self.y += LANDER_VSPEED * pressure * dt;
                // Drift a little while climbing so the ascent is not a
                // dead vertical line.
                self.x = world::wrap(self.x + self.vx.signum() * DRIFT_SPEED * 0.4 * pressure * dt);

                // ⚠️ S7 TAKES OVER HERE. At the top of the world this
                // should become a Mutant and, if it was the last person,
                // end the world. Reported rather than acted on, so the
                // stage boundary is visible in the code.
                if self.y >= world::VIEW_H * 1.6 {
                    outcome = Outcome::ReachedTop;
                }
            }

            // Handled once, for every kind, in `step`.
            Phase::Warping | Phase::Dying => {}
        }

        // ★ A LANDER SHOOTS ONLY WHEN IT HAS NOTHING BETTER TO REPORT,
        // and only while free-flying. One that is beaming someone up has
        // its hands full — and a shot fired from the beam would land on
        // the very player trying to shoot the carrier, punishing the
        // rescue the whole game is built around.
        if outcome == Outcome::None && matches!(self.phase, Phase::Hovering | Phase::Hunting) {
            self.fire_cooldown -= dt;
            if let Some((sx, _)) = ship {
                if self.fire_cooldown <= 0.0 && world::delta(self.x, sx).abs() < LANDER_FIRE_RANGE {
                    self.fire_cooldown =
                        LANDER_FIRE_INTERVAL * (0.7 + 0.6 * next01(noise)) / pressure;
                    outcome = Outcome::Fires;
                }
            }
        }

        outcome
    }
}

impl Enemy {
    /// A Mutant's whole behaviour: chase the ship, and shoot at it.
    fn step_mutant(
        &mut self,
        ship: Option<(f32, f32)>,
        noise: &mut u32,
        pressure: f32,
        dt: f32,
    ) -> Outcome {
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

        let speed = MUTANT_SPEED * pressure;
        self.x = world::wrap(self.x + (dir_x / dl) * speed * dt);
        self.y += (dir_y / dl) * speed * dt;
        self.vx = (dir_x / dl) * speed;

        // Stay in the world vertically — a Mutant that chased a climbing
        // ship forever would leave the playfield and never come back.
        self.y = self.y.clamp(20.0, world::VIEW_H * 1.5);

        self.fire_cooldown -= dt;
        if self.fire_cooldown <= 0.0 && gap_x.abs() < MUTANT_FIRE_RANGE {
            self.fire_cooldown = MUTANT_FIRE_INTERVAL * (0.7 + 0.6 * next01(noise)) / pressure;
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

    /// Where a Lander's shot should go: at the ship at `(sx, sy)`, give
    /// or take [`LANDER_AIM_ERROR`] — and, [`LANDER_LEAD_CHANCE`] of the
    /// time, at where a ship moving at `(svx, svy)` WILL be when the shot
    /// gets there. Unlike a Mutant's it may fly level — the never-straight
    /// rule is Brian's for Mutants, and a slow level shot is the easiest
    /// thing in the game to step out of.
    ///
    /// ★ THE ORIGINAL'S ANSWER TO FLYING THE LINE (SHOOT, defb6.src: when
    /// SEED > 120 the player's velocity PLAXV is added to the shot's).
    /// Brian: "hardly any landers will pick up a humanoid if you just go
    /// down the line". A ship at top speed outruns a shot aimed where it
    /// is; one aimed where it is going meets it. A ship that is not
    /// racing along has no lead to take, so to it nothing has changed.
    pub fn lander_aim(&self, sx: f32, sy: f32, svx: f32, svy: f32, noise: &mut u32) -> (f32, f32) {
        let mut dx = world::delta(self.x, sx);
        let mut dy = sy - self.y;
        if next01(noise) < LANDER_LEAD_CHANCE {
            if let Some(t) = intercept_time(dx, dy, svx, svy, LANDER_SHOT_SPEED) {
                dx += svx * t;
                dy += svy * t;
            }
        }
        let angle = dy.atan2(dx) + (next01(noise) - 0.5) * 2.0 * LANDER_AIM_ERROR;
        (angle.cos(), angle.sin())
    }

    /// How fast this one's shots fly. A Bomber never fires; its entry is
    /// only there so the match stays total.
    pub fn shot_speed(&self) -> f32 {
        match self.kind {
            // A Bomber and a Pod never fire; their entries keep the match total.
            Kind::Lander | Kind::Bomber | Kind::Pod => LANDER_SHOT_SPEED,
            Kind::Swarmer => SWARMER_SHOT_SPEED,
            Kind::Mutant => crate::shot::ENEMY_SHOT_SPEED,
            Kind::Baiter => BAITER_SHOT_SPEED,
        }
    }

    /// ★ THE BAITER (UFOST/UFOLP/UFONV): it cannot be outrun, but it
    /// does not steer every moment either.
    ///
    /// It aims once as it arrives. After that, every
    /// [`BAITER_SEEK_PERIOD`] it re-aims only on a roll that succeeds
    /// `chase.seek` of the time. A re-aim sets X to the SHIP's speed plus
    /// [`BAITER_MARGIN`] toward you (unless already within
    /// [`BAITER_CLOSE`], when X is left alone) and, unless within
    /// [`BAITER_Y_CLOSE`], Y to half of your vertical speed plus
    /// [`BAITER_VSEEK`] toward you. Between re-aims it simply flies on.
    fn step_baiter(
        &mut self,
        ship: Option<(f32, f32)>,
        chase: Chase,
        noise: &mut u32,
        pressure: f32,
        dt: f32,
    ) -> Outcome {
        let Some((sx, sy)) = ship else {
            // No ship to bait: keep drifting.
            self.x = world::wrap(self.x + self.vx * dt);
            return Outcome::None;
        };
        let dx = world::delta(self.x, sx);
        let dy = sy - self.y;

        // ⚠️ `bias` IS "HAS AIMED YET": a Baiter always aims as it arrives
        // (UFOST calls UFONV0 unconditionally); only later re-aims roll.
        self.aux -= dt;
        if self.aux <= 0.0 {
            self.aux += BAITER_SEEK_PERIOD;
            let first = self.bias == 0.0;
            if first || next01(noise) < chase.seek {
                self.bias = 1.0;
                if dx.abs() > BAITER_CLOSE {
                    self.vx = chase.vx + BAITER_MARGIN * dx.signum();
                }
                if dy.abs() > BAITER_Y_CLOSE {
                    self.vy = (chase.vy + BAITER_VSEEK * dy.signum()) * 0.5;
                }
            }
        }
        self.x = world::wrap(self.x + self.vx * dt);
        self.y += self.vy * dt;
        let (low, high) = (40.0, world::VIEW_H * 0.95);
        if self.y < low || self.y > high {
            self.y = self.y.clamp(low, high);
            self.vy = 0.0;
        }

        self.fire_cooldown -= dt;
        if self.fire_cooldown <= 0.0 && dx.abs() < world::VIEW_W * 0.6 {
            self.fire_cooldown = BAITER_FIRE_INTERVAL * (0.6 + 0.8 * next01(noise)) / pressure;
            return Outcome::Fires;
        }
        Outcome::None
    }

    /// ★ THE BOMBER: constant drift, never fires, and on screen it holds
    /// [`BOMBER_NEAR`]..[`BOMBER_FAR`] above or below you and lays mines.
    /// Off screen it wanders a cruise altitude. (TIE, defb6.src.)
    fn step_bomber(
        &mut self,
        terrain: &Terrain,
        ship: Option<(f32, f32)>,
        noise: &mut u32,
        dt: f32,
    ) -> Outcome {
        self.x = world::wrap(self.x + self.vx * dt);

        let on_screen = ship.filter(|(sx, _)| world::delta(self.x, *sx).abs() < BOMBER_ON_SCREEN);
        let push = match on_screen {
            Some((_, sy)) => {
                // The original's bands: too far above → come down, too close
                // above → go up; mirrored below. Inside the band, coast.
                let rel = self.y - sy;
                let r = rel.abs();
                if r > BOMBER_FAR {
                    -rel.signum()
                } else if r < BOMBER_NEAR {
                    if rel == 0.0 { 1.0 } else { rel.signum() }
                } else {
                    0.0
                }
            }
            None => {
                // Wander the cruise altitude a little, now and then.
                if next01(noise) < 0.5 * dt {
                    self.aux = (self.aux + (next01(noise) - 0.5) * 60.0)
                        .clamp(world::VIEW_H * 0.45, world::VIEW_H * 0.8);
                }
                let gap = self.aux - self.y;
                if gap.abs() > 16.0 { gap.signum() } else { 0.0 }
            }
        };
        self.vy += push * BOMBER_ACCEL * dt;
        self.vy -= self.vy * BOMBER_DRAG * dt;
        self.y += self.vy * dt;
        // Never into the mountains, never off the top.
        let floor = terrain.height_at(self.x) + 40.0;
        self.y = self.y.clamp(floor.min(world::VIEW_H * 0.9), world::VIEW_H * 0.92);

        if on_screen.is_some() && next01(noise) < BOMBER_MINE_RATE * dt {
            return Outcome::LaysMine;
        }
        Outcome::None
    }
}

impl Enemy {
    /// ★ THE POD: drifts and never fires. It bounces between the
    /// mountains and the top of the sky rather than leaving the playfield.
    /// (The bounce is ours; the drift is PRBST's.)
    fn step_pod(&mut self, terrain: &Terrain, dt: f32) -> Outcome {
        self.x = world::wrap(self.x + self.vx * dt);
        self.y += self.vy * dt;
        let floor = (terrain.height_at(self.x) + 60.0).min(world::VIEW_H * 0.5);
        let ceiling = world::VIEW_H * 0.92;
        if self.y < floor {
            self.y = floor;
            self.vy = self.vy.abs();
        } else if self.y > ceiling {
            self.y = ceiling;
            self.vy = -self.vy.abs();
        }
        Outcome::None
    }

    /// ★ THE SWARMER (MSWM). Scatter first, then chase: X at its fixed
    /// speed toward you, re-chosen only once it is [`SWARMER_TURN_GAP`]
    /// past you; Y weaving toward your altitude on a 3-frame tick.
    fn step_swarmer(
        &mut self,
        ship: Option<(f32, f32)>,
        noise: &mut u32,
        pressure: f32,
        dt: f32,
    ) -> Outcome {
        // The scatter: flying out of the Pod on the burst's velocity.
        if self.phase == Phase::Hovering {
            self.x = world::wrap(self.x + self.vx * dt);
            self.y += self.vy * dt;
            self.y = self.y.clamp(20.0, world::VIEW_H * 0.95);
            if self.elapsed < self.aux {
                return Outcome::None;
            }
            self.phase = Phase::Hunting;
            self.aux = 0.0;
            // MSWM's entry: pick a side and go.
            if let Some((sx, _)) = ship {
                let dir = if world::delta(self.x, sx) < 0.0 { -1.0 } else { 1.0 };
                self.vx = dir * SWARMER_SPEED * pressure;
            }
        }

        let Some((sx, sy)) = ship else {
            // No ship: carry on the way it was going.
            self.x = world::wrap(self.x + self.vx * dt);
            return Outcome::None;
        };
        let dx = world::delta(self.x, sx);
        let speed = SWARMER_SPEED * pressure;

        // ⚠️ ONLY WHEN WELL PAST. Inside the gap the direction is kept, so
        // a ship that slips in behind a pack is never turned on.
        if dx.abs() > SWARMER_TURN_GAP {
            self.vx = dx.signum() * speed;
        } else {
            self.vx = self.vx.signum() * speed;
        }

        // The weave, on the original's 3-frame tick.
        self.aux += dt;
        while self.aux >= SWARMER_TICK {
            self.aux -= SWARMER_TICK;
            let toward = if sy > self.y { 1.0 } else { -1.0 };
            self.vy = (self.vy + toward * self.bias).clamp(-SWARMER_VY_MAX, SWARMER_VY_MAX);
            self.vy -= self.vy * SWARMER_DAMPING;
            self.vy += (next01(noise) - 0.5) * 2.0 * SWARMER_NUDGE;
        }

        self.x = world::wrap(self.x + self.vx * dt);
        self.y += self.vy * dt;
        self.y = self.y.clamp(20.0, world::VIEW_H * 0.95);

        // ★ IT FIRES ONLY WHILE HEADING FOR YOU, and only on screen
        // (SWBMB: `EORA OXV,X` / `BMI SWBX`). The timer re-arms either way.
        self.fire_cooldown -= dt;
        if self.fire_cooldown <= 0.0 {
            self.fire_cooldown =
                SWARMER_FIRE_INTERVAL * (0.6 + 0.8 * next01(noise)) / pressure;
            let heading_for_you = self.vx * dx > 0.0;
            if heading_for_you && dx.abs() < world::VIEW_W * 0.55 {
                return Outcome::Fires;
            }
        }
        Outcome::None
    }

    /// Where a Swarmer's shot goes: FORWARD, the way it is flying, dropping
    /// to `sy` over [`SWARMER_SHOT_REACH`]. Never back at a ship behind it.
    pub fn swarmer_aim(&self, sy: f32) -> (f32, f32) {
        let dir = if self.vx < 0.0 { -1.0 } else { 1.0 };
        (dir * SWARMER_SHOT_REACH, sy - self.y)
    }
}

/// What a Baiter chases with: the ship's velocity (its vertical speed is
/// measured, since its climb is direct), and the chance a re-aim roll
/// succeeds ([`crate::waves::baiter_seek`]).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Chase {
    pub vx: f32,
    pub vy: f32,
    pub seek: f32,
}

/// What a Lander's step needs its owner to do.
///
/// The Lander cannot reach the Humanoid list, so it says what happened
/// and the owner applies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    None,
    /// The beam has taken hold of its target.
    Grabbed,
    /// Carried a Humanoid off the top of the world — time to fuse into
    /// a Mutant.
    ReachedTop,
    /// A Mutant wants to shoot at the ship.
    Fires,
    /// A Bomber drops a mine where it is.
    LaysMine,
}

/// Every enemy in the world, of every kind.
#[derive(Debug, Default)]
pub struct Enemies {
    live: Vec<Enemy>,
    /// How many Landers have been added since this was last read.
    ///
    /// ★ A COUNT, NOT A FLAG, and the difference matters at the opening
    /// wave: five arrive on the same frame, and a bool could only say
    /// "something spawned". The count lets the caller play ONE arrival
    /// scaled to how big it was, rather than retriggering one voice five
    /// times and hearing only the last.
    ///
    /// ⚠️ CLEARED BY READING IT (`take_spawned`). A caller that forgets
    /// to read it gets a number that grows forever, which is a louder
    /// failure than a flag that silently stays true.
    spawned: usize,
    /// How many Landers have FUSED into Mutants since this was last read.
    ///
    /// ★★ A SEPARATE COUNT FROM `spawned`, BECAUSE BRIAN HEARS THEM AS
    /// DIFFERENT EVENTS: "we have a different sound for when the lander
    /// merges with the humanoid and spawns in. but the sound I just made
    /// we will use for spawns of landers and baiters."
    /// ⇒ An arrival is something appearing out of nothing. A fusion is a
    /// thing you failed to stop becoming worse — the arcade's cruellest
    /// arithmetic — and the game should not say the same word for both.
    ///
    /// ⚠️ FUSION DOES NOT GO THROUGH `spawn`. A Lander reaching the top
    /// calls `mutate()` and transforms IN PLACE; nothing is ever pushed.
    /// That is exactly why the arrival voice was silent in play.
    fused: usize,
    /// Humanoid indices already spoken for, so two Landers never hunt
    /// the same person and end up stacked on the same spot.
    claimed: Vec<usize>,
    /// Noise for Mutant jitter and fire timing. On the collection so a
    /// whole world of Mutants shares one deterministic stream.
    noise: u32,
    /// The wave's [`crate::waves::pressure`], set by the owner each step.
    pressure: f32,
    /// Where the round-robin hand-out of targets got to (GTARG's TPTR).
    next_target: usize,
    /// What a Baiter needs that a position does not carry, set by the
    /// owner each step.
    chase: Chase,
    /// Mines Bombers have dropped since this was last read.
    mines: Vec<(f32, f32)>,
    /// People a Lander has taken hold of since this was last read — the
    /// grab's voice (ED10) plays on it.
    grabbed: usize,
}

impl Enemies {
    pub fn new() -> Self {
        Self {
            live: Vec::new(),
            claimed: Vec::new(),
            noise: 0x4D07_A17E,
            spawned: 0,
            fused: 0,
            pressure: 1.0,
            next_target: 0,
            chase: Chase { vx: 0.0, vy: 0.0, seek: 1.0 },
            mines: Vec::new(),
            grabbed: 0,
        }
    }

    /// How many people Landers have taken hold of since this was last
    /// asked, and reset the count (the take-pattern of `take_spawned`).
    pub fn take_grabbed(&mut self) -> usize {
        std::mem::replace(&mut self.grabbed, 0)
    }

    /// The ship's velocity, and how keen a Baiter is to re-aim
    /// ([`crate::waves::baiter_seek`]).
    pub fn set_chase(&mut self, chase: Chase) {
        self.chase = chase;
    }

    #[cfg(test)]
    pub fn chase(&self) -> Chase {
        self.chase
    }

    /// Mines dropped since this was last asked, and forget them — the
    /// owner lays them. (The same take-pattern as `take_spawned`.)
    pub fn take_mines(&mut self) -> Vec<(f32, f32)> {
        std::mem::take(&mut self.mines)
    }

    /// Live enemies of `kind`, not yet dying.
    pub fn count(&self, kind: Kind) -> usize {
        self.live.iter().filter(|e| e.kind == kind && e.phase != Phase::Dying).count()
    }

    /// ★ THE BAITERS LEAVE WHEN THE WAVE IS WON. Brian's spec: "Vanish
    /// when all Landers die." They are the hurry-up, and with nothing left
    /// to hurry there is no Baiter left to fight — removed, unscored.
    pub fn dismiss_baiters(&mut self) {
        self.live.retain(|e| e.kind != Kind::Baiter);
    }

    /// Seed the shared noise, so two games do not fight the same fight.
    pub fn reseed(&mut self, seed: u32) {
        // xorshift has one fixed point, and it is zero.
        self.noise = seed | 1;
    }

    /// How hard the wave is pushing. See [`crate::waves::pressure`].
    pub fn set_pressure(&mut self, pressure: f32) {
        self.pressure = pressure;
    }

    pub fn iter(&self) -> impl Iterator<Item = &Enemy> {
        self.live.iter()
    }

    pub fn len(&self) -> usize {
        self.live.len()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }

    /// How many are still killable — the number that matters for "is the
    /// wave over", which S9 will ask.
    pub fn remaining(&self) -> usize {
        self.live.iter().filter(|l| l.phase != Phase::Dying).count()
    }

    pub fn spawn(&mut self, enemy: Enemy) {
        self.live.push(enemy);
        self.spawned += 1;
    }

    /// How many Landers have fused into Mutants since this was last
    /// asked, and reset the count.
    ///
    /// ★ THE EVENT IS THE TRANSFORMATION, not an appearance. See
    /// [`Landers::take_spawned`] for the other half of the pair.
    pub fn take_fused(&mut self) -> usize {
        std::mem::replace(&mut self.fused, 0)
    }

    /// How many Landers have arrived since this was last asked, and
    /// reset the count.
    ///
    /// ★ EVERY ARRIVAL GOES THROUGH `spawn`, including `scatter`'s
    /// opening wave and whatever S9's reinforcements turn out to be, so
    /// a caller wired to this needs no change when waves land.
    /// ⚠️ MUTANTS COUNT TOO — `Lander::mutant` is spawned through here
    /// when one fuses. That is correct: a Mutant appearing is also
    /// something arriving, and it is one of the six silent events.
    pub fn take_spawned(&mut self) -> usize {
        std::mem::replace(&mut self.spawned, 0)
    }

    pub fn clear(&mut self) {
        self.live.clear();
        self.claimed.clear();
    }

    /// Bring `count` Landers in near the top of the sky, spread round the
    /// world, each drifting its own random way at its own random speed.
    ///
    /// ★ THE ORIGINAL'S ARRIVAL (LANDST): the top of the playfield,
    /// RMAX(LNDXV) for the speed and a coin for the direction. Its X is
    /// plain random, but in a world a quarter the size in screens that
    /// put two or three of a squad in one spot — Brian, flying it: "the
    /// landers seem to be spawning in groups too close together". So the
    /// world away from the player is cut into one slot per Lander and
    /// each lands at a random place in the middle of its own slot:
    /// never stacked, and still not evenly spaced (evenly spaced arrivals
    /// grabbed in unison all round the world). ⚠️ Never in the player's
    /// lap: the slots start and end [`ARRIVE_CLEAR`] either side of them.
    pub fn arrive(&mut self, count: usize, avoid_x: f32, seed: u32) {
        let span = world::WORLD_W - 2.0 * ARRIVE_CLEAR;
        let slot = span / count.max(1) as f32;
        for i in 0..count {
            // ⚠️ wrapping_mul, NOT `*`: a plain multiply panics in a debug
            // build from i = 2 onward.
            let k = seed.wrapping_add((i as u32).wrapping_mul(2_654_435_761));
            let within = ARRIVE_JITTER.0 + hash01(k) * (ARRIVE_JITTER.1 - ARRIVE_JITTER.0);
            let x = world::wrap(avoid_x + ARRIVE_CLEAR + slot * (i as f32 + within));
            let steps = 1 + (hash01(k ^ 0x5851_F42D) * DRIFT_STEPS as f32) as u32;
            let speed = DRIFT_MAX * steps.min(DRIFT_STEPS) as f32 / DRIFT_STEPS as f32;
            let dir = if hash01(k ^ 0x9E37_79B9) < 0.5 { -1.0 } else { 1.0 };
            self.spawn(Enemy::lander(x, ARRIVE_HEIGHT, speed * dir));
        }
    }

    /// Bring in a squad of `count`, spread round the world away from
    /// `avoid_x`, announcing itself through `spawn` like every arrival.
    ///
    /// ★ AFTER THE WORLD HAS ENDED THEY ARRIVE AS MUTANTS — the
    /// original's own rule (`LNDST0`). With no one left to abduct, a
    /// Lander has no job; what comes through the warp is the thing that
    /// wants the pilot. They still warp in: [`Enemy::step`] finishes the
    /// arrival for every kind.
    pub fn squad(&mut self, count: usize, avoid_x: f32, seed: u32, mutants: bool) {
        let first = self.live.len();
        self.arrive(count, avoid_x, seed);
        if mutants {
            for e in &mut self.live[first..] {
                e.kind = Kind::Mutant;
                e.fire_cooldown = MUTANT_FIRE_INTERVAL;
            }
        }
    }

    /// The first live enemy whose body overlaps a box at `(x, y)` with
    /// these half-extents — the ship flying into one.
    pub fn body_hit(&self, x: f32, y: f32, half_w: f32, half_h: f32) -> Option<usize> {
        self.live.iter().position(|e| {
            let (hw, hh) = e.half_extents();
            e.is_target()
                && world::delta(e.x, x).abs() <= hw + half_w
                && (e.y - y).abs() <= hh + half_h
        })
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
        let pressure = self.pressure;
        let chase = self.chase;
        let mut mines = Vec::new();
        // ⚠️ ACCUMULATED LOCALLY, not written straight to `self.fused`.
        // The loop below holds `&mut` borrows of `self.live`, so the
        // field cannot be touched from inside it.
        let mut fused = 0usize;
        let mut grabbed = 0usize;
        for (index, l) in self.live.iter_mut().enumerate() {
            // ★ A DRIFTING LANDER WITHOUT A PERSON IS HANDED THE NEXT ONE ON
            // THE LIST (GTARG) — wherever they are, not the nearest. Done
            // here, before its step, rather than as an outcome of it: an
            // outcome would take the step's one slot, and a Lander asking
            // for a target every step would never get to fire (a test
            // caught exactly that).
            if l.kind == Kind::Lander && l.phase == Phase::Hovering {
                let lost = l.target.is_some_and(|i| !people.get(i).is_some_and(|h| h.is_grabbable()));
                if lost {
                    l.target = None;
                }
                if l.target.is_none() {
                    l.target = hand_out(&mut self.next_target, &self.claimed, people);
                    if let Some(i) = l.target {
                        self.claimed.push(i);
                    }
                }
            }
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

            match l.step(terrain, prey, ship, chase, &mut noise, pressure, dt) {
                Outcome::None => {}

                Outcome::LaysMine => mines.push((l.x, l.y)),

                Outcome::Fires => {
                    // The owner builds the bolt — this type does not
                    // know what a Shot is, and should not learn.
                    shots_wanted.push((index, l.x, l.y));
                }

                Outcome::Grabbed => {
                    if let Some(i) = l.target {
                        if let Some(h) = people.get_mut(i) {
                            h.grabbed();
                            grabbed += 1;
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
                    fused += 1;
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
        self.fused += fused;
        self.grabbed += grabbed;
        self.mines.extend(mines);
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
            // ⚠️ LANDERS ONLY. A Baiter or a Bomber has no Humanoid to
            // fuse with and no Mutant form.
            if l.phase != Phase::Dying && l.kind == Kind::Lander {
                l.mutate();
                l.y = l.y.min(world::VIEW_H * 0.9);
            }
        }
        self.claimed.clear();
        // ⚠️ DELIBERATELY DOES NOT TOUCH `fused`. The world ending is
        // ONE event with its own voice (`world_ended_this_frame` plays
        // the Lander boom at full gain); a dozen fusion sounds stacked
        // on top of it would bury the thing they were announcing. The
        // per-Lander fusion voice is for the ordinary case — one you
        // failed to stop — where it carries information.
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
            let (hw, hh) = l.half_extents();
            l.is_target() && world::delta(l.x, x).abs() <= hw && (l.y - y).abs() <= hh
        })
    }

    /// Kill the one at `index` and report its points.
    ///
    /// ★ A POD BURSTS. Its Swarmers are pushed straight into the list,
    /// NOT through `spawn`: they do not warp in, and the original plays
    /// no arrival for them — the Pod's own death is the sound (PRBKIL).
    /// ⚠️ APPENDED, SO EVERY EXISTING INDEX STAYS PUT. The smart bomb
    /// walks indices fixed before it started, so Swarmers born from a Pod
    /// it killed are past the end of its walk and survive it — the
    /// original's rule ("they aren't drawn yet").
    pub fn kill(&mut self, index: usize) -> u32 {
        let worth = self.live[index].kill();
        if self.live[index].kind == Kind::Pod {
            let (x, y) = (self.live[index].x, self.live[index].y);
            self.burst(x, y);
        }
        worth
    }

    /// Let a Pod's Swarmers out at `(x, y)`, as many as [`burst_count`]
    /// rolls and [`MAX_SWARMERS`] allows.
    fn burst(&mut self, x: f32, y: f32) {
        let want = burst_count((self.next_noise() >> 24) as u8);
        let room = MAX_SWARMERS.saturating_sub(self.count(Kind::Swarmer));
        for _ in 0..want.min(room) {
            let mut n = self.next_noise();
            let vx = (next01(&mut n) - 0.5) * 2.0 * SWARMER_BURST_VX;
            let vy = (next01(&mut n) - 0.5) * 2.0 * SWARMER_BURST_VY;
            let scatter = next01(&mut n) * SWARMER_SCATTER_MAX;
            let accel = next01(&mut n) * SWARMER_ACCEL_MAX;
            self.live.push(Enemy::swarmer(x, y, vx, vy, scatter, accel));
        }
    }

    /// The enemy at `index`, for a caller that needs to aim from it.
    pub fn get(&self, index: usize) -> Option<&Enemy> {
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
    #[cfg(test)]
    pub fn mutants(&self) -> usize {
        self.live.iter().filter(|l| l.is_mutant() && l.phase != Phase::Dying).count()
    }
}

/// The next free person on the list after `next`, round-robin (GTARG).
/// ⚠️ Never one already claimed: two Landers on one Humanoid stack in the
/// same place, which looks like a rendering bug rather than a race.
fn hand_out(next: &mut usize, claimed: &[usize], people: &Humanoids) -> Option<usize> {
    let n = people.len();
    for k in 1..=n {
        let i = (*next + k) % n;
        if people.get(i).is_some_and(|h| h.is_grabbable()) && !claimed.contains(&i) {
            *next = i;
            return Some(i);
        }
    }
    None
}

/// Advance a noise state and return 0..1.
fn next01(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}

/// When a shot fired now at `speed` meets a target `(dx, dy)` away moving
/// at `(vx, vy)`: the first t > 0 with |d + v·t| = speed·t. None when the
/// target is outrunning the shot, which then aims where the target is.
fn intercept_time(dx: f32, dy: f32, vx: f32, vy: f32, speed: f32) -> Option<f32> {
    let a = vx * vx + vy * vy - speed * speed;
    let b = 2.0 * (dx * vx + dy * vy);
    let c = dx * dx + dy * dy;
    if a.abs() < 1e-3 {
        return (b < 0.0).then(|| -c / b);
    }
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return None;
    }
    let r = disc.sqrt();
    [(-b - r) / (2.0 * a), (-b + r) / (2.0 * a)]
        .into_iter()
        .filter(|t| *t > 0.0)
        .reduce(f32::min)
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

    /// ★ EVERY ARRIVAL IS COUNTED, whatever path it came in by.
    #[test]
    fn spawning_is_counted_and_the_count_clears() {
        let t = terrain();
        let mut ls = Enemies::new();
        assert_eq!(ls.take_spawned(), 0, "a fresh world has not spawned anything");

        ls.spawn(Enemy::lander(500.0, 300.0, 0.0));
        ls.spawn(Enemy::lander(900.0, 300.0, 0.0));
        assert_eq!(ls.take_spawned(), 2, "two arrivals should count as two");
        assert_eq!(ls.take_spawned(), 0, "reading the count must clear it");
        // ⚠️ AND AN ARRIVAL IS NOT A FUSION. The two counters feed two
        // different voices; a spawn that also bumped `fused` would play
        // the Mutant-forming sound every time a Lander appeared.
        assert_eq!(
            ls.take_fused(),
            0,
            "an arrival leaked into the fusion count — they must stay distinct"
        );

        // ★ A squad's arrival goes through `spawn`, so Warp hears it.
        let mut ls = Enemies::new();
        ls.arrive(5, 0.0, 99);
        assert_eq!(ls.take_spawned(), 5, "an arrival must be reported");
        let _ = &t;

        // ⚠️⚠️ A MUTANT FUSING IS *NOT* COUNTED, AND THIS TEST USED TO
        // CLAIM IT WAS. The old version called `spawn(Lander::mutant(…))`
        // and asserted the count went up — which is true, and proves
        // nothing, because THE GAME NEVER TAKES THAT PATH. A Lander that
        // reaches the top calls `l.mutate()` and transforms IN PLACE
        // (see `Outcome::ReachedTop`); no Lander is ever pushed. The test
        // was exercising my assumption instead of the code.
        // ⇒ Brian heard the consequence before any test did: "I didn't
        // hear it at all". With `scatter` called once at startup and
        // fusion mutating in place, `spawn` is never reached during play
        // and the arrival voice could not sound.
        let mut ls = Enemies::new();
        ls.spawn(Enemy::lander(500.0, 300.0, 0.0));
        ls.take_spawned();
        let before = ls.len();
        if let Some(l) = ls.live.first_mut() {
            l.mutate();
        }
        assert_eq!(ls.len(), before, "fusion must transform, not add");
        assert_eq!(
            ls.take_spawned(),
            0,
            "fusion counted as an arrival — it mutates in place, so if this \
             ever becomes true it is a real change and not a refactor"
        );
    }

    #[test]
    fn a_lander_cannot_be_shot_until_it_has_arrived() {
        let t = terrain();
        let mut ls = Enemies::new();
        ls.spawn(Enemy::lander(500.0, 300.0, 0.0));

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
        let mut ls = Enemies::new();
        // Sitting a few units west of the seam, i.e. at the very top of
        // the coordinate range.
        ls.spawn(Enemy::lander(world::WORLD_W - 4.0, 300.0, 0.0));
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
        let mut ls = Enemies::new();
        ls.spawn(Enemy::lander(500.0, 300.0, 0.0));
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
        let mut ls = Enemies::new();
        ls.spawn(Enemy::lander(500.0, 300.0, 0.0));
        ls.step(&t, &mut nobody(), None, WARP_SECONDS + 0.01);
        let y = ls.iter().next().unwrap().y;
        let hit = ls.hit_test(500.0, y).unwrap();
        ls.kill(hit);

        ls.step(&t, &mut nobody(), None, DEATH_SECONDS * 0.5);
        assert_eq!(ls.len(), 1, "the corpse should still be drawn");
        ls.step(&t, &mut nobody(), None, DEATH_SECONDS);
        assert!(ls.is_empty(), "the corpse should be gone");
    }

    /// Where a still Lander at x = 500 settles, starting at height `y`.
    fn settle_from(y: f32) -> (f32, f32) {
        let t = terrain();
        let mut ls = Enemies::new();
        ls.spawn(Enemy::lander(500.0, y, 0.0));
        ls.step(&t, &mut nobody(), None, WARP_SECONDS + 0.01);
        // Eight seconds: it moves at LNDYV now, not by easing.
        for _ in 0..480 {
            ls.step(&t, &mut nobody(), None, 1.0 / 60.0);
        }
        let l = ls.iter().next().unwrap();
        (l.y, t.height_at(l.x) + HOVER_HEIGHT)
    }

    #[test]
    fn landers_follow_the_ridge_rather_than_a_fixed_height() {
        // From below, it climbs to the bottom of the band and holds.
        let (y, want) = settle_from(10.0);
        assert!((y - (want - HOVER_BAND)).abs() < 4.0, "settled at {y} not {}", want - HOVER_BAND);
    }

    /// ★ THE HOVER IS A BAND (LANDS0): from above it sinks to the top of
    /// it, from below it climbs only to the bottom — so two Landers over
    /// the same ground can hunt a band's height apart, and no one
    /// altitude lines a squad up for the laser.
    #[test]
    fn the_hover_is_a_band_not_a_line() {
        let (high, want) = settle_from(world::VIEW_H);
        let (low, _) = settle_from(10.0);
        assert!((high - want).abs() < 4.0, "from above settled at {high} not {want}");
        assert!(high - low > HOVER_BAND * 0.9, "both settled on one line: {high} vs {low}");
    }

    /// ★ THE ORIGINAL'S ARRIVAL: at the top, never on the player, each
    /// at its own speed within RMAX(LNDXV), both ways.
    #[test]
    fn landers_arrive_at_the_top_each_their_own_way() {
        let mut ls = Enemies::new();
        ls.arrive(40, 0.0, 12345);
        assert_eq!(ls.len(), 40);
        let mut speeds = Vec::new();
        let (mut east, mut west) = (0, 0);
        for l in ls.iter() {
            assert_eq!(l.y, ARRIVE_HEIGHT, "not at the top");
            assert!(world::delta(0.0, l.x).abs() > world::VIEW_W * 0.3, "in the player's lap at {}", l.x);
            assert!(l.x >= 0.0 && l.x < world::WORLD_W, "outside the world: {}", l.x);
            let v = l.vx.abs();
            assert!(v > 0.0 && v <= DRIFT_MAX + 1e-3, "drift {v} out of range");
            speeds.push(v);
            if l.vx > 0.0 { east += 1 } else { west += 1 }
        }
        speeds.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!(speeds[speeds.len() - 1] - speeds[0] > DRIFT_MAX * 0.5, "speeds barely vary");

        assert!(east > 5 && west > 5, "all one way: {east} east, {west} west");
    }

    /// Where a slow Lander shot fired at a ship racing past below it
    /// ends up relative to the ship: the closest it comes, in units.
    fn closest_pass(aim: (f32, f32), ship: (f32, f32), svx: f32) -> f32 {
        let l = Enemy::lander(1000.0, 500.0, 0.0);
        let (mut bx, mut by) = (l.x, l.y);
        let (mut sx, sy) = ship;
        let mut best = f32::MAX;
        for _ in 0..240 {
            let dt = 1.0 / 120.0;
            bx += aim.0 * LANDER_SHOT_SPEED * dt;
            by += aim.1 * LANDER_SHOT_SPEED * dt;
            sx += svx * dt;
            best = best.min(((bx - sx).powi(2) + (by - sy).powi(2)).sqrt());
        }
        best
    }

    /// ★★ A LEADING SHOT MEETS A SHIP RACING THE LINE; A PLAIN ONE
    /// FALLS BEHIND IT (SHOOT, defb6.src). Over many shots from a Lander
    /// above a ship flying toward and under it at top speed: the shots
    /// that lead it pass close, and without the lead none do.
    #[test]
    fn some_lander_shots_lead_a_moving_ship() {
        let l = Enemy::lander(1000.0, 500.0, 0.0);
        let ship = (1400.0, 250.0);
        let svx = -crate::flight::TOP_SPEED;
        let (mut close, mut still_close) = (0, 0);
        for seed in 1..400u32 {
            let mut n = seed.wrapping_mul(0x9E37_79B9) | 1;
            if closest_pass(l.lander_aim(ship.0, ship.1, svx, 0.0, &mut n), ship, svx) < 40.0 {
                close += 1;
            }
            let mut n = seed.wrapping_mul(0x9E37_79B9) | 1;
            if closest_pass(l.lander_aim(ship.0, ship.1, 0.0, 0.0, &mut n), ship, svx) < 40.0 {
                still_close += 1;
            }
        }
        assert!(close > 100, "only {close} of 399 leading shots met the ship");
        assert_eq!(still_close, 0, "a shot that does not lead met the ship");
    }

    /// A ship that is not moving gets the same shot it always did.
    #[test]
    fn a_still_ship_gets_no_lead() {
        let l = Enemy::lander(1000.0, 500.0, 0.0);
        for seed in 1..50u32 {
            let mut a = seed;
            let lead = l.lander_aim(1300.0, 300.0, 0.0, 0.0, &mut a);
            // The same noise draws by hand — the lead coin, then the
            // error — with no lead applied: it must be the same shot.
            let mut n = seed;
            let _ = next01(&mut n);
            let angle = (300.0f32 - l.y).atan2(world::delta(l.x, 1300.0))
                + (next01(&mut n) - 0.5) * 2.0 * LANDER_AIM_ERROR;
            let plain = (angle.cos(), angle.sin());
            assert!((lead.0 - plain.0).abs() < 1e-5 && (lead.1 - plain.1).abs() < 1e-5);
        }
    }

    /// ★★ A SQUAD IS SPREAD, NOT CLUMPED, AND NOT EVENLY SPACED EITHER.
    /// Clumped was Brian's complaint ("spawning in groups too close
    /// together"); evenly spaced was half of why a whole squad once
    /// grabbed at once. Over many seeds: no two of a squad of 5 closer
    /// than 30% of a slot, and the gaps between them vary.
    #[test]
    fn a_squad_arrives_spread_round_the_world() {
        let slot = (world::WORLD_W - 2.0 * ARRIVE_CLEAR) / SQUAD as f32;
        const SQUAD: usize = 5;
        let (mut lo, mut hi) = (f32::MAX, 0.0f32);
        for seed in 0..200u32 {
            let mut ls = Enemies::new();
            ls.arrive(SQUAD, 1234.0, seed.wrapping_mul(0x9E37_79B9));
            let mut xs: Vec<f32> = ls.iter().map(|l| world::delta(1234.0, l.x).rem_euclid(world::WORLD_W)).collect();
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            for w in xs.windows(2) {
                lo = lo.min(w[1] - w[0]);
                hi = hi.max(w[1] - w[0]);
            }
        }
        assert!(lo >= slot * 0.3 - 0.5, "two of a squad {lo:.0} apart (slot {slot:.0})");
        assert!(hi - lo > slot * 0.8, "squads are evenly spaced: gaps {lo:.0}..{hi:.0}");
    }

    /// The same seed must give the same wave, or a screenshot cannot be
    /// reproduced and a bug cannot be chased.
    #[test]
    fn arrivals_are_deterministic() {
        let mut a = Enemies::new();
        let mut b = Enemies::new();
        a.arrive(4, 300.0, 99);
        b.arrive(4, 300.0, 99);
        let xs: Vec<(f32, f32)> = a.iter().map(|l| (l.x, l.vx)).collect();
        let ys: Vec<(f32, f32)> = b.iter().map(|l| (l.x, l.vx)).collect();
        assert_eq!(xs, ys);
    }

    /// ★★ IT DOES NOT STEER. A Lander drifting away from its person keeps
    /// going — the abduction comes only when its drift carries it over
    /// them. This is the rule that spreads wave 1's abductions out.
    #[test]
    fn a_lander_drifts_until_it_passes_over_its_target() {
        let t = terrain();
        let mut people = Humanoids::new();
        people.spawn(crate::humanoid::Humanoid::new(1000.0, t.height_at(1000.0), 0.0));
        let mut ls = Enemies::new();
        let mut l = Enemy::lander(900.0, t.height_at(900.0) + HOVER_HEIGHT, -DRIFT_SPEED);
        l.phase = Phase::Hovering;
        ls.spawn(l);
        for _ in 0..(120 * 3) {
            ls.step(&t, &mut people, None, 1.0 / 120.0);
        }
        let l = ls.get(0).unwrap();
        assert_eq!(l.target, Some(0), "it was never given its person");
        assert_eq!(l.phase, Phase::Hovering, "it went for them instead of drifting");
        assert!(world::delta(900.0, l.x) < -100.0, "it turned toward them: at {}", l.x);
    }

    /// ⚠️ THE SEAM. A Lander drifting west from x = 30 must swoop on the
    /// person at WORLD_W - 20, fifty units away across the seam — through
    /// `delta`, not a subtraction that calls them a world apart.
    #[test]
    fn a_lander_swoops_across_the_seam() {
        let t = terrain();
        let mut people = Humanoids::new();
        let px = world::WORLD_W - 20.0;
        people.spawn(crate::humanoid::Humanoid::new(px, t.height_at(px), 0.0));
        let mut ls = Enemies::new();
        // 65 units east of them across the seam — just outside ALIGN_X.
        let mut l = Enemy::lander(45.0, t.height_at(45.0) + HOVER_HEIGHT, -DRIFT_SPEED);
        l.phase = Phase::Hovering;
        ls.spawn(l);
        for _ in 0..(120 * 2) {
            ls.step(&t, &mut people, None, 1.0 / 120.0);
            if ls.get(0).unwrap().phase != Phase::Hovering {
                break;
            }
        }
        let l = ls.get(0).unwrap();
        assert_eq!(l.phase, Phase::Hunting, "it drifted past them at the seam");
        // ⚠️ AND IT SAW THEM BEFORE IT CROSSED: once it has wrapped to the
        // far end a plain subtraction would also say "close", so only a
        // swoop that starts on THIS side proves `delta` is doing the work.
        assert!(l.x < world::VIEW_W, "it only saw them after wrapping, at {}", l.x);
    }

    /// ★ TARGETS ARE HANDED OUT IN TURN (GTARG), NOT BY DISTANCE: a Lander
    /// right above one person is given the next on the list.
    #[test]
    fn targets_are_handed_out_in_turn_not_by_distance() {
        let t = terrain();
        let mut people = Humanoids::new();
        for x in [500.0, 2000.0, 3000.0] {
            people.spawn(crate::humanoid::Humanoid::new(x, t.height_at(x), 0.0));
        }
        let mut ls = Enemies::new();
        for _ in 0..2 {
            let mut l = Enemy::lander(500.0, t.height_at(500.0) + HOVER_HEIGHT, DRIFT_SPEED);
            l.phase = Phase::Hovering;
            ls.spawn(l);
        }
        ls.step(&t, &mut people, None, 1.0 / 120.0);
        let targets: Vec<Option<usize>> = ls.iter().map(|l| l.target).collect();
        assert_eq!(targets, vec![Some(1), Some(2)], "handed out by distance, not in turn");
    }

    /// ★★ THE WHOLE STAGE, END TO END. A Lander drifts over its person,
    /// swoops, lowers a beam, and carries them up.
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

        let mut ls = Enemies::new();
        ls.spawn(Enemy::lander(500.0, t.height_at(500.0) + HOVER_HEIGHT, DRIFT_SPEED));

        // Long enough to warp, drift over them, swoop, and grab.
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
        // ★ AND THE GRAB IS COUNTED, through the real step — its voice
        // (ED10) plays on this.
        assert_eq!(ls.take_grabbed(), 1, "the grab was not counted");
        assert_eq!(ls.take_grabbed(), 0, "reading the count must clear it");
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

        let mut ls = Enemies::new();
        ls.spawn(Enemy::lander(590.0, t.height_at(600.0) + HOVER_HEIGHT, DRIFT_SPEED));

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

        let mut ls = Enemies::new();
        ls.spawn(Enemy::lander(560.0, t.height_at(560.0) + HOVER_HEIGHT, DRIFT_SPEED));
        ls.spawn(Enemy::lander(640.0, t.height_at(640.0) + HOVER_HEIGHT, -DRIFT_SPEED));

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

        let mut ls = Enemies::new();
        ls.spawn(Enemy::lander(500.0, t.height_at(500.0) + HOVER_HEIGHT, DRIFT_SPEED));

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

        let mut ls = Enemies::new();
        ls.spawn(Enemy::lander(590.0, t.height_at(600.0) + HOVER_HEIGHT, DRIFT_SPEED));
        // Drain the test's own setup arrival, so the counts asserted
        // below are about the FUSION and nothing else.
        assert_eq!(ls.take_spawned(), 1, "the setup spawn should count once");

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

        // ★★ AND THE FUSION IS COUNTED, THROUGH THE REAL PATH. This
        // assertion rides on the test that already drives an actual
        // Lander to the top rather than calling `mutate()` by hand —
        // which matters, because the FIRST version of the arrival test
        // did call a method directly, asserted the count moved, passed,
        // and proved nothing. Brian found the consequence by ear: "I
        // didn't hear it at all."
        assert_eq!(ls.take_fused(), 1, "a fusion must be counted");
        assert_eq!(ls.take_fused(), 0, "reading the count must clear it");
        // ⚠️ AND IT IS NOT AN ARRIVAL. The two are separate events with
        // separate voices; a fusion that also counted as a spawn would
        // play the arrival sound for a Mutant forming.
        assert_eq!(
            ls.take_spawned(),
            0,
            "fusion counted as an arrival — they must stay distinct"
        );
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
        let m = Enemy::mutant(500.0, 400.0);
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
        let m = Enemy::mutant(500.0, 400.0);
        let mut noise = 0x9999u32;

        let (vx, _) = m.aim_at(900.0, 400.0, &mut noise);
        assert!(vx > 0.0, "should fire east at a ship to the east");

        let (vx, _) = m.aim_at(100.0, 400.0, &mut noise);
        assert!(vx < 0.0, "should fire west at a ship to the west");

        // ⚠️ AND ACROSS THE SEAM. A ship just east of x=0 is WEST of a
        // Mutant near the end of the world, by the short way round.
        let m = Enemy::mutant(world::WORLD_W - 50.0, 400.0);
        let (vx, _) = m.aim_at(20.0, 400.0, &mut noise);
        assert!(vx > 0.0, "across the seam it must still fire the short way");
    }

    #[test]
    fn a_mutant_chases_the_ship_and_asks_to_fire() {
        let t = terrain();
        let mut people = Humanoids::new();
        let mut ls = Enemies::new();
        ls.spawn(Enemy::mutant(500.0, 400.0));

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
        let mut ls = Enemies::new();
        ls.spawn(Enemy::mutant(500.0, 400.0));

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
        let mut ls = Enemies::new();
        ls.arrive(4, 0.0, 99);
        let _ = &t;
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
        let mut ls = Enemies::new();
        ls.spawn(Enemy::lander(5.0, 300.0, -DRIFT_SPEED));
        ls.step(&t, &mut nobody(), None, WARP_SECONDS + 0.01);
        for _ in 0..600 {
            ls.step(&t, &mut nobody(), None, 1.0 / 60.0);
        }
        let l = ls.iter().next().unwrap();
        assert!(l.x >= 0.0 && l.x < world::WORLD_W, "drifted out of the world: {}", l.x);
        assert!(l.x > world::WORLD_W * 0.5, "should have wrapped west, at {}", l.x);
    }

    // ----- W3: the Baiter and the Bomber -----

    fn flying(mut e: Enemy) -> Enemy {
        e.phase = Phase::Hovering;
        e
    }

    /// The chase at the start of wave 1: re-aims ~21% of rolls.
    fn wave_one(vx: f32, vy: f32) -> Chase {
        Chase { vx, vy, seek: crate::waves::baiter_seek(1, 0.0) }
    }

    /// ★ A BAITER CANNOT BE OUTRUN: the ship flat out, the Baiter a screen
    /// behind. With the original's lazy re-aiming it does not sit on you —
    /// it overshoots, runs on, and comes back (measured: swings of ±500–
    /// 1100 u) — but it keeps CROSSING you, and never strays much beyond a
    /// screen. Five seeds, so one lucky roll cannot pass it.
    #[test]
    fn a_baiter_cannot_be_outrun() {
        let t = Terrain::generate(256, 0x0DEF_E4DE);
        let top = 640.0;
        let dt = 1.0 / 240.0;
        for seed in 1..=5u32 {
            let mut people = Humanoids::new();
            let mut es = Enemies::new();
            es.reseed(seed.wrapping_mul(0x9E37_79B9));
            let mut sx = 1000.0f32;
            es.spawn(flying(Enemy::baiter(sx - world::VIEW_W, 300.0)));
            es.set_chase(wave_one(top, 0.0));
            let (mut worst, mut crossings, mut ahead) = (0.0f32, 0, false);
            for i in 0..(240 * 12) {
                sx = world::wrap(sx + top * dt);
                es.step(&t, &mut people, Some((sx, 300.0)), dt);
                let d = world::delta(sx, es.get(0).unwrap().x);
                if i > 240 * 4 {
                    worst = worst.max(d.abs());
                    if (d > 0.0) != ahead {
                        crossings += 1;
                    }
                }
                ahead = d > 0.0;
            }
            assert!(crossings >= 2, "seed {seed}: crossed the ship only {crossings} times in 8 s");
            assert!(worst < world::VIEW_W * 1.3, "seed {seed}: the ship got {worst:.0} away");
        }
    }

    /// ★ IT FLIES ON BETWEEN RE-AIMS. With re-aiming switched off it aims
    /// once as it arrives — and then overshoots the ship and keeps going,
    /// rather than snapping back (the window Brian's flight was missing).
    #[test]
    fn a_baiter_aims_on_arrival_then_flies_on() {
        let t = Terrain::generate(256, 0x0DEF_E4DE);
        let mut people = Humanoids::new();
        let mut es = Enemies::new();
        let sx = 1000.0;
        es.spawn(flying(Enemy::baiter(sx - 300.0, 300.0)));
        es.set_chase(Chase { vx: 0.0, vy: 0.0, seek: 0.0 });
        let dt = 1.0 / 240.0;
        es.step(&t, &mut people, Some((sx, 300.0)), dt);
        let aimed = es.get(0).unwrap().vx;
        assert_eq!(aimed, BAITER_MARGIN, "it did not aim on arrival");
        for _ in 0..(240 * 3) {
            es.step(&t, &mut people, Some((sx, 300.0)), dt);
        }
        let b = es.get(0).unwrap();
        assert_eq!(b.vx, aimed, "it re-aimed with re-aiming off");
        assert!(world::delta(sx, b.x) > world::VIEW_W * 0.5, "it did not overshoot");
    }

    /// Already within 20 px of the ship, a re-aim leaves X alone (UFONV1).
    #[test]
    fn a_baiter_level_with_you_does_not_re_aim_sideways() {
        let t = Terrain::generate(256, 0x0DEF_E4DE);
        let mut people = Humanoids::new();
        let mut es = Enemies::new();
        es.spawn(flying(Enemy::baiter(1010.0, 300.0)));
        es.set_chase(Chase { vx: 0.0, vy: 0.0, seek: 0.0 });
        es.step(&t, &mut people, Some((1000.0, 300.0)), 1.0 / 240.0);
        assert_eq!(es.get(0).unwrap().vx, 0.0);
    }

    /// ★ ITS CLIMB IS THE ORIGINAL'S: on a re-aim, half of (your vertical
    /// speed + 1 line a frame toward you) — 90 u/s at a ship holding still,
    /// and it closes faster on a ship climbing toward it than away.
    #[test]
    fn a_baiter_closes_altitude_at_the_originals_rate() {
        let t = Terrain::generate(256, 0x0DEF_E4DE);
        let dt = 1.0 / 240.0;
        let climb = |ship_vy: f32| {
            let mut people = Humanoids::new();
            let mut es = Enemies::new();
            es.spawn(flying(Enemy::baiter(1000.0, 200.0)));
            es.set_chase(Chase { vx: 0.0, vy: ship_vy, seek: 0.0 });
            es.step(&t, &mut people, Some((1000.0, 500.0)), dt);
            es.get(0).unwrap().vy
        };
        assert_eq!(climb(0.0), BAITER_VSEEK * 0.5);
        assert_eq!(climb(-100.0), (BAITER_VSEEK - 100.0) * 0.5);
    }

    /// ★ A BOMBER HOLDS NEAR YOUR ALTITUDE, NOT ON IT — and never fires.
    #[test]
    fn a_bomber_holds_its_band_and_never_fires() {
        let t = Terrain::generate(256, 0x0DEF_E4DE);
        let mut people = Humanoids::new();
        let mut es = Enemies::new();
        let (sx, sy) = (1000.0, 300.0);
        es.spawn(flying(Enemy::bomber(sx + 100.0, sy + 220.0, 0.0, 400.0)));
        let dt = 1.0 / 240.0;
        for _ in 0..(240 * 6) {
            let wants = es.step(&t, &mut people, Some((sx, sy)), dt);
            assert!(wants.is_empty(), "a Bomber fired");
        }
        let rel = (es.get(0).unwrap().y - sy).abs();
        assert!(
            (BOMBER_NEAR - 12.0..=BOMBER_FAR + 12.0).contains(&rel),
            "it sat {rel:.0} from the ship's altitude"
        );
    }

    /// ★ MINES ONLY WHERE YOU CAN SEE THEM: a Bomber two screens away lays
    /// nothing; one on screen lays a field.
    #[test]
    fn a_bomber_lays_mines_only_on_screen() {
        let t = Terrain::generate(256, 0x0DEF_E4DE);
        let mut people = Humanoids::new();
        let dt = 1.0 / 240.0;
        let run = |dx: f32| {
            let mut es = Enemies::new();
            es.spawn(flying(Enemy::bomber(1000.0 + dx, 400.0, 0.0, 400.0)));
            let mut people = Humanoids::new();
            for _ in 0..(240 * 5) {
                es.step(&t, &mut people, Some((1000.0, 300.0)), dt);
            }
            es.take_mines().len()
        };
        let _ = &mut people;
        assert_eq!(run(world::VIEW_W * 2.0), 0, "a Bomber off screen laid mines");
        assert!(run(100.0) >= 3, "a Bomber on screen laid no field");
    }

    /// The world ending turns Landers, and only Landers.
    #[test]
    fn the_world_ending_does_not_touch_baiters_or_bombers() {
        let mut es = Enemies::new();
        es.spawn(flying(Enemy::baiter(500.0, 300.0)));
        es.spawn(flying(Enemy::bomber(900.0, 300.0, 100.0, 400.0)));
        es.mutate_all();
        assert_eq!(es.get(0).unwrap().kind, Kind::Baiter);
        assert_eq!(es.get(1).unwrap().kind, Kind::Bomber);
    }

    /// ★ HITBOXES COME FROM THE ART (recommendation 2): a shot just inside
    /// the Baiter's drawn hull hits, one just outside misses — and the
    /// half-width is the hull's own 12.15 art units.
    #[test]
    fn the_baiter_and_bomber_are_hit_where_they_are_drawn() {
        let s = crate::art::SCALE;
        assert!((BAITER_HALF.0 - 12.15 * s).abs() < 1e-3, "Baiter half-width {}", BAITER_HALF.0);
        assert!((BOMBER_HALF.0 - 7.0 * s).abs() < 1e-3, "Bomber half-width {}", BOMBER_HALF.0);
        let mut es = Enemies::new();
        es.spawn(flying(Enemy::baiter(1000.0, 300.0)));
        assert_eq!(es.hit_test(1000.0 + 11.5 * s, 300.0), Some(0), "missed inside the hull");
        assert_eq!(es.hit_test(1000.0 + 13.0 * s, 300.0), None, "hit outside the hull");
    }

    // ----- W4: the Pod and the Swarmer -----

    /// ★ RMAX(6) IS 4–7, NOT 1–6: every byte, counted.
    #[test]
    fn a_pod_holds_four_to_seven_swarmers_almost_always() {
        let counts: Vec<usize> = (0..=255u8).map(burst_count).collect();
        assert!(counts.iter().all(|n| (1..=7).contains(n)));
        let high = counts.iter().filter(|&&n| n >= 4).count();
        assert_eq!(high, 253, "RMAX(6) lands on 4–7 for 253 of 256 bytes");
        let mean = counts.iter().sum::<usize>() as f32 / 256.0;
        assert!((mean - 5.45).abs() < 0.01, "mean {mean}");
    }

    /// ★ A SHOT POD BURSTS where it died — and the Swarmers do NOT count as
    /// arrivals, so no Warp plays for them (the Pod's death is the sound).
    #[test]
    fn a_pod_bursts_into_swarmers_where_it_dies() {
        let mut es = Enemies::new();
        es.spawn(flying(Enemy::pod(1200.0, 400.0, 0.0, 30.0)));
        es.take_spawned();
        assert_eq!(es.kill(0), POD_POINTS);
        let n = es.count(Kind::Swarmer);
        assert!((1..=7).contains(&n), "{n} swarmers");
        for e in es.iter().filter(|e| e.kind == Kind::Swarmer) {
            assert_eq!((e.x, e.y), (1200.0, 400.0));
            assert!(e.is_target(), "a Swarmer out of a Pod can be shot at once");
        }
        assert_eq!(es.take_spawned(), 0, "the burst announced itself as an arrival");
    }

    /// ★ NEVER MORE THAN TWENTY: a Pod that bursts into a full sky lets
    /// out only what fits.
    #[test]
    fn swarmers_are_capped_at_twenty() {
        let mut es = Enemies::new();
        for _ in 0..8 {
            es.spawn(flying(Enemy::pod(1200.0, 400.0, 0.0, 30.0)));
        }
        for i in 0..8 {
            es.kill(i);
        }
        assert_eq!(es.count(Kind::Swarmer), MAX_SWARMERS);
    }

    /// A Swarmer already chasing at `vx`, with no weave. ⚠️ ALREADY
    /// HUNTING: one straight out of a Pod picks its side toward you on its
    /// first step (MSWM's entry), which is not what these tests are about.
    fn swarmer_at(x: f32, y: f32, vx: f32) -> Enemy {
        Enemy { phase: Phase::Hunting, ..Enemy::swarmer(x, y, vx, 0.0, 0.0, 0.0) }
    }

    /// ★ BRIAN'S TIP: FLY CLOSE BEHIND THEM AND THEY DO NOT TURN. A
    /// Swarmer heading east, with the ship just behind it to the west,
    /// keeps going east — until it is more than the turn gap past.
    #[test]
    fn a_swarmer_keeps_its_heading_until_well_past_you() {
        let t = terrain();
        let mut people = nobody();
        let mut es = Enemies::new();
        let sx = 1000.0;
        es.spawn(swarmer_at(sx + 40.0, 300.0, SWARMER_SPEED));
        let dt = 1.0 / 240.0;
        let mut turned_at = None;
        for i in 0..(240 * 6) {
            es.step(&t, &mut people, Some((sx, 300.0)), dt);
            let e = es.get(0).unwrap();
            if e.vx < 0.0 {
                turned_at = Some((i, world::delta(sx, e.x)));
                break;
            }
        }
        let (_, gap) = turned_at.expect("it never turned back");
        // Measured after the turning step has already moved it back one
        // step's travel (under a unit at 240 Hz).
        assert!(gap > SWARMER_TURN_GAP - 1.0, "it turned only {gap:.0} past the ship");
        assert!(gap < SWARMER_TURN_GAP + 10.0, "it overshot to {gap:.0} before turning");
    }

    /// ★ IT ONLY SHOOTS WHILE HEADING FOR YOU. With the ship behind it, a
    /// Swarmer never fires; with the ship ahead, it does.
    #[test]
    fn a_swarmer_never_fires_back_at_a_ship_behind_it() {
        let t = terrain();
        let mut people = nobody();
        let dt = 1.0 / 240.0;

        let mut es = Enemies::new();
        es.spawn(swarmer_at(1040.0, 300.0, SWARMER_SPEED));
        // ⚠️ LONGER THAN THE FIRST FIRE INTERVAL (2.4 s), or "it never
        // fired" would be true of any Swarmer, heading or not.
        for _ in 0..(240 * 8) {
            // Keep it just ahead of the ship, both moving east.
            let sx = world::delta(0.0, es.get(0).unwrap().x) - 40.0;
            let wants = es.step(&t, &mut people, Some((sx, 300.0)), dt);
            assert!(wants.is_empty(), "it fired back at a ship behind it");
        }

        let mut es = Enemies::new();
        es.spawn(swarmer_at(800.0, 300.0, SWARMER_SPEED));
        let mut fired = 0;
        for _ in 0..(240 * 6) {
            let sx = world::wrap(es.get(0).unwrap().x + 300.0);
            fired += es.step(&t, &mut people, Some((sx, 300.0)), dt).len();
        }
        assert!(fired > 0, "it never fired at a ship ahead of it");
    }

    /// ★ ITS SHOT GOES FORWARD: the way it flies, dropping toward you.
    #[test]
    fn a_swarmer_shoots_forward_not_at_you() {
        let s = swarmer_at(1000.0, 300.0, -SWARMER_SPEED);
        let (dx, dy) = s.swarmer_aim(200.0);
        assert!(dx < 0.0, "a westbound Swarmer shot east");
        assert!(dy < 0.0, "a shot aimed above a ship below");
    }

    /// ★ THE WEAVE: a Swarmer that rolled a strong acceleration crosses
    /// your altitude again and again rather than settling on it.
    #[test]
    fn a_swarmer_weaves_across_your_altitude() {
        let t = terrain();
        let mut people = nobody();
        let mut es = Enemies::new();
        es.spawn(Enemy::swarmer(1000.0, 450.0, 0.0, 0.0, 0.0, SWARMER_ACCEL_MAX));
        let dt = 1.0 / 240.0;
        let sy = 300.0;
        let (mut crossings, mut above) = (0, true);
        for _ in 0..(240 * 8) {
            let sx = es.get(0).unwrap().x + 100.0;
            es.step(&t, &mut people, Some((sx, sy)), dt);
            let now = es.get(0).unwrap().y > sy;
            if now != above {
                crossings += 1;
                above = now;
            }
        }
        assert!(crossings >= 4, "only {crossings} crossings in 8 s: it settled, not weaved");
    }

    /// ★ A POD NEVER FIRES, and stays in the sky.
    #[test]
    fn a_pod_drifts_and_never_fires() {
        let t = terrain();
        let mut people = nobody();
        let mut es = Enemies::new();
        es.spawn(flying(Enemy::pod(1000.0, 400.0, POD_DRIFT_MAX, -POD_VY_MAX)));
        let dt = 1.0 / 240.0;
        let mut highest_late = 0.0f32;
        for i in 0..(240 * 30) {
            let wants = es.step(&t, &mut people, Some((1000.0, 400.0)), dt);
            assert!(wants.is_empty(), "a Pod fired");
            let p = es.get(0).unwrap();
            assert!(p.y > t.height_at(p.x) && p.y < world::VIEW_H, "left the sky at {}", p.y);
            if i > 240 * 10 {
                highest_late = highest_late.max(p.y);
            }
        }
        // ★ IT BOUNCES: sent downward, it reaches the floor within seconds
        // and must come back up into the sky, not sit pinned to the floor.
        assert!(highest_late > world::VIEW_H * 0.6, "pinned low: never above {highest_late:.0}");
    }
}
