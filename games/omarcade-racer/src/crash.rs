//! The crash fireball: one drawing, animated by transform.
//!
//! There is no frame sequence here and that is deliberate. The art is a
//! single 160x60 grid (`art::EXPLOSION`); everything that makes it move
//! is derived from one number, the fraction of its life that has
//! elapsed. This is the same choice the car's cornering pose makes
//! (decision d966a129): a transform is authored once and stays correct
//! at every size, where a hand-drawn sequence is drawn N times and the
//! N drawings drift apart.
//!
//! Three transforms stack, and each is doing a job the others cannot:
//!
//! - **FLIP** mirrors the sprite left-to-right. The art is asymmetric by
//!   17% of its own mass, so mirroring visibly changes the silhouette
//!   without moving the fireball. On its own this is a two-state loop
//!   and reads as one within about half a second.
//! - **SCALE** grows the fireball over its life. This is what makes it
//!   an explosion rather than a campfire, and — because it is
//!   continuous — it means no two frames are ever the same size, which
//!   is what hides the flip's two-ness.
//! - **FADE** takes it out toward smoke and then to nothing, so it ends
//!   rather than vanishing mid-burn.
//!
//! ⚠️ THE FLIP IS TIMED, NOT PER-FRAME. Alternating every frame at 60fps
//! is a 30Hz square wave on the brightest sprite in the game: it reads
//! as electrical buzz rather than flame, and high-contrast flicker in
//! that band is exactly what accessibility guidance warns about. It is
//! also the L023 trap in a new costume — a per-frame alternation changes
//! character the moment the framerate does. Driving it from elapsed time
//! means a dropped frame slows nothing.

use omarcade_core::backend::Canvas;
use omarcade_core::sprite::Sprite;
use omarcade_core::Color;

/// How long a fireball burns, in seconds.
///
/// Long enough to read as an event rather than a blink, short enough
/// that the player is not held out of the game. The crash STATE may
/// outlast this — that is the game's business, not the sprite's.
pub const BURN_TIME: f32 = 1.4;

/// Seconds between mirror flips.
///
/// 0.06s is about 8Hz of alternation. Fast enough to read as turbulence,
/// well clear of the 30Hz buzz that flipping every frame at 60fps would
/// produce, and — being a time, not a frame count — unchanged by a
/// dropped frame.
pub const FLIP_INTERVAL: f32 = 0.06;

/// The scale the fireball starts and ends at, as a multiple of the size
/// a CAR occupies at the same distance.
///
/// ⚠️ MEASURED, NOT PICKED. The explosion art's ink is 22 rows tall and
/// the car's ink is 22 rows tall — identical — so a fireball drawn at
/// the car's own scale rule comes out exactly car-sized, and a ramp
/// through 1.0 spans "smaller than the wreck" to "barely larger". That
/// reads as a puff, not a blast.
///
/// An explosion must be BIGGER than the thing that exploded. It starts
/// at the wreck's own size, because at the instant of impact that is
/// what it replaces, and ends at nearly three times it.
///
/// This is a third scaling rule, alongside the two the structures needed
/// (a road-spanning thing scales by its INK WIDTH, a roadside thing by
/// its PANEL HEIGHT). A fireball scales against THE OBJECT IT REPLACES.
pub const SCALE_START: f32 = 1.0;
pub const SCALE_END: f32 = 2.8;

/// The fraction of the burn spent at full brightness before the fade
/// begins.
///
/// The fade is not linear over the whole life: an explosion is bright
/// almost immediately and dies slowly. Fading from t=0 makes the first
/// frames — the ones carrying the impact — the dimmest ones.
pub const HOLD_FRACTION: f32 = 0.35;

/// How far the fade is allowed to go.
///
/// ⚠️ NOT 1.0, AND THIS IS NOT A TASTE CALL. `Canvas` composites opaque
/// pixels — there is no alpha to fade into — so the fade is a tint
/// toward the smoke colour. At amount 1.0 EVERY pixel becomes exactly
/// the smoke colour, and the fireball's last frame is a solid
/// silhouette: a black block sitting on the road, which is worse than
/// no effect at all. Seen in the road scene, not in a test.
///
/// Stopping at 0.72 keeps some of the fire's own variation in the final
/// frame, so it reads as thinning smoke. The sprite still has to be
/// REMOVED when it burns out — the fade takes it most of the way, and
/// `is_alive` does the rest.
pub const FADE_LIMIT: f32 = 0.72;

/// One crash fireball, burning at a fixed place on the track.
///
/// It holds a track position rather than a screen position: the world
/// keeps moving underneath it, so a fireball left behind recedes and
/// shrinks with the road exactly as any other object does. A
/// screen-anchored explosion would slide along with the camera and read
/// as a decal stuck to the glass. (This is the same rule the roadside
/// props follow — an object owns a `z`; a scanline never owns an
/// object.)
#[derive(Debug, Clone, Copy)]
pub struct Explosion {
    /// Track position of the blast, in world units.
    pub z: f32,
    /// Lateral position, in the same units the car's `x` uses.
    pub x: f32,
    /// Seconds burned so far.
    elapsed: f32,
}

impl Explosion {
    /// Light one at a track position.
    pub fn start(z: f32, x: f32) -> Explosion {
        Explosion {
            z,
            x,
            elapsed: 0.0,
        }
    }

    /// Age it. Returns `false` once it has burned out.
    pub fn advance(&mut self, dt: f32) -> bool {
        // A negative or non-finite dt would run the fireball backwards
        // or park it at NaN, and it is reached from the same clamped
        // frame clock the physics uses.
        if dt.is_finite() && dt > 0.0 {
            self.elapsed += dt;
        }
        self.is_alive()
    }

    /// Has it finished burning?
    pub fn is_alive(&self) -> bool {
        self.elapsed < BURN_TIME
    }

    /// How far through its life, 0.0 at ignition to 1.0 at burnout.
    pub fn life(&self) -> f32 {
        (self.elapsed / BURN_TIME).clamp(0.0, 1.0)
    }

    /// The size multiplier at the current moment.
    ///
    /// Grows on a decelerating curve rather than linearly: a blast
    /// expands fastest at the instant it happens and then slows. A
    /// linear ramp reads as something inflating.
    pub fn scale_factor(&self) -> f32 {
        let t = self.life();
        let eased = 1.0 - (1.0 - t) * (1.0 - t);
        SCALE_START + (SCALE_END - SCALE_START) * eased
    }

    /// Whether this moment draws mirrored.
    ///
    /// Timed, not per-frame — see the module note. Integer division of
    /// elapsed time by the interval, so the alternation is identical at
    /// 30fps and 60fps.
    pub fn flipped(&self) -> bool {
        (self.elapsed / FLIP_INTERVAL) as u32 % 2 == 1
    }

    /// How far the fireball has faded toward smoke, 0.0 (full heat) to
    /// 1.0 (gone).
    ///
    /// Holds at full brightness for the first [`HOLD_FRACTION`] of the
    /// burn, then fades over the remainder.
    pub fn fade(&self) -> f32 {
        let t = self.life();
        if t <= HOLD_FRACTION {
            0.0
        } else {
            ((t - HOLD_FRACTION) / (1.0 - HOLD_FRACTION)) * FADE_LIMIT
        }
    }

    /// Draw it, anchored on the ground at a screen position.
    ///
    /// `base_scale` is the scale the sprite would draw at if it were a
    /// car at this distance — the caller owns the projection, this owns
    /// only the animation on top of it.
    ///
    /// The fade is applied as a tint toward the smoke colour rather than
    /// as alpha, because `Canvas` composites opaque pixels: there is no
    /// blend to fade into. Tinting toward the scene's own dark tone is
    /// what "burning out" looks like when every pixel is solid.
    pub fn draw(
        &self,
        canvas: &mut Canvas<'_>,
        sprite: &Sprite,
        cx: f32,
        ground_y: f32,
        base_scale: f32,
        smoke: Color,
    ) {
        let scale = base_scale * self.scale_factor();
        if !scale.is_finite() || scale <= 0.0 {
            return;
        }

        // Anchor on the ground under the fireball's INK, not under the
        // grid: the art sits low in a 160x60 field with blank rows on
        // every side, and anchoring by the grid would both offset it
        // sideways and hang it in the air.
        //
        // ⚠️ BOTH HELPERS RETURN PIXELS IN THE SPRITE'S OWN GRID, so
        // both scale by `scale` — NOT by the sprite's scaled width. The
        // first version of this multiplied the bias by `width * scale`,
        // which made the offset grow with the fireball: it walked
        // sideways across the screen as it expanded, which looks exactly
        // like a physics bug and is not one. This is the same form
        // `structures.rs` uses, and it is the reason that form exists.
        let cx = cx - sprite.ink_centre_bias() * scale;
        let ground_y = ground_y + sprite.ink_foot_gap() * scale;
        let w = sprite.width() as f32 * scale;
        let h = sprite.height() as f32 * scale;
        let x = cx - w / 2.0;
        let y = ground_y - h;

        let fade = self.fade();
        if self.flipped() {
            // `draw_flipped` carries no tint, so a fading fireball draws
            // unmirrored for its last frames rather than snapping back
            // to full heat on alternate frames. Below the threshold the
            // tint is invisible anyway.
            if fade < 0.05 {
                sprite.draw_flipped(canvas, x, y, scale);
                return;
            }
        }
        sprite.draw_tinted(canvas, x, y, scale, Some((smoke, fade)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fireball_burns_out() {
        let mut e = Explosion::start(1000.0, 0.0);
        assert!(e.is_alive());
        assert!(e.advance(BURN_TIME * 0.5));
        assert!(e.is_alive());
        assert!(!e.advance(BURN_TIME));
        assert!(!e.is_alive());
    }

    #[test]
    fn it_grows_over_its_life() {
        // The blast expands. Without this the sprite reads as a lit
        // decal rather than an explosion, and the flip's two-ness
        // becomes visible because every frame is the same size.
        let mut e = Explosion::start(0.0, 0.0);
        let first = e.scale_factor();
        e.advance(BURN_TIME * 0.5);
        let mid = e.scale_factor();
        e.advance(BURN_TIME * 0.49);
        let last = e.scale_factor();

        assert!(first < mid, "{first} !< {mid}");
        assert!(mid < last, "{mid} !< {last}");
        assert!((first - SCALE_START).abs() < 1e-5);
        assert!(last < SCALE_END);
    }

    #[test]
    fn a_blast_outgrows_the_wreck() {
        // The art's ink is the same height as the car's, so scale 1.0 is
        // exactly car-sized. A fireball that never passes 1.0 reads as a
        // puff. This is the guard on the measurement in SCALE_END's
        // docs, and it fails against the first values tried (0.65/1.55),
        // which topped out at 0.9x the car in the road scene.
        let mut e = Explosion::start(0.0, 0.0);
        assert!(e.scale_factor() >= 1.0, "starts smaller than the wreck");
        e.advance(BURN_TIME * 0.9);
        assert!(
            e.scale_factor() > 2.0,
            "peaks at {:.2}x the car — too small to read as a blast",
            e.scale_factor()
        );
    }

    #[test]
    fn growth_decelerates() {
        // A blast expands fastest at the instant it happens. A linear
        // ramp reads as inflation, so the first half must cover more
        // ground than the second — this fails against `eased = t`.
        let mut e = Explosion::start(0.0, 0.0);
        let a = e.scale_factor();
        e.advance(BURN_TIME * 0.5);
        let b = e.scale_factor();
        e.advance(BURN_TIME * 0.5);
        let c = e.scale_factor();
        assert!(b - a > c - b, "first half {} !> second {}", b - a, c - b);
    }

    #[test]
    fn the_flip_alternates_and_is_driven_by_time_not_frames() {
        // The whole point of timing the flip: 60fps and 30fps must show
        // the same alternation, or a dropped frame changes what the
        // effect looks like. Stepping one clock at half the other's rate
        // must land on the same mirror state at the same instants.
        let mut fast = Explosion::start(0.0, 0.0);
        let mut slow = Explosion::start(0.0, 0.0);

        let mut seen_both = (false, false);
        for _ in 0..20 {
            // two 60fps steps == one 30fps step
            fast.advance(1.0 / 60.0);
            fast.advance(1.0 / 60.0);
            slow.advance(1.0 / 30.0);
            assert_eq!(
                fast.flipped(),
                slow.flipped(),
                "mirror state diverged between 60fps and 30fps at t={}",
                fast.elapsed
            );
            if fast.flipped() {
                seen_both.0 = true;
            } else {
                seen_both.1 = true;
            }
        }
        assert!(
            seen_both.0 && seen_both.1,
            "the flip never alternated — a constant is not an animation"
        );
    }

    #[test]
    fn the_flip_is_well_clear_of_the_buzz_band() {
        // Flipping every frame at 60fps is a 30Hz square wave on the
        // brightest sprite in the game. Guard the interval so a future
        // "make it flicker faster" cannot walk it back into that band
        // without this failing.
        let hz = 1.0 / (FLIP_INTERVAL * 2.0);
        assert!(hz < 12.0, "flip runs at {hz}Hz, into the buzz band");
        assert!(hz > 4.0, "flip runs at {hz}Hz, slow enough to read as two images");
    }

    #[test]
    fn it_holds_full_heat_before_fading() {
        // Fading from t=0 dims exactly the frames that carry the impact.
        let mut e = Explosion::start(0.0, 0.0);
        assert_eq!(e.fade(), 0.0);
        e.advance(BURN_TIME * HOLD_FRACTION * 0.9);
        assert_eq!(e.fade(), 0.0, "faded during the hold");

        e.advance(BURN_TIME * 0.5);
        assert!(e.fade() > 0.0, "never started fading");

        e.advance(BURN_TIME);
        assert!(
            (e.fade() - FADE_LIMIT).abs() < 1e-5,
            "did not fade to the limit"
        );
    }

    #[test]
    fn the_fade_never_reaches_a_solid_silhouette() {
        // `Canvas` has no alpha: a tint at 1.0 paints every pixel the
        // same colour, so a fully faded fireball is a black block on the
        // road. Guard the ceiling — this fails against `FADE_LIMIT =
        // 1.0`, which is what the first version shipped and what the
        // road scene caught.
        let mut e = Explosion::start(0.0, 0.0);
        e.advance(BURN_TIME * 10.0);
        assert!(
            e.fade() < 0.85,
            "fade reaches {:.2} — the last frame is a silhouette",
            e.fade()
        );
    }

    /// The fireball must not WALK as it grows.
    ///
    /// This is the guard on the anchor arithmetic. `ink_centre_bias`
    /// returns pixels in the sprite's grid; multiplying it by the
    /// sprite's SCALED WIDTH instead of by `scale` makes the horizontal
    /// offset grow with the fireball, so it slides sideways across the
    /// screen over its life. In the road scene it crossed most of a
    /// panel. It looks like a physics bug and it is a units bug.
    ///
    /// Measured on the drawn output rather than on the intermediate,
    /// per L022: the question is where the ink LANDS, not what the
    /// helper returns.
    #[test]
    fn a_growing_fireball_stays_where_it_was_lit() {
        use omarcade_core::sprite::PaletteEntry;

        // ⚠️ THE INK MUST BE GENUINELY OFF-CENTRE IN ITS GRID, and this
        // fixture is measured, not eyeballed. The first attempt used a
        // 4-wide grid with ink in columns 1..=2 — which is dead centre,
        // so `ink_centre_bias` was 0.0, and zero multiplied by a wrong
        // factor is still zero. The test passed against the very bug it
        // was written for. A 6-wide grid with the same ink gives a bias
        // of -1.0, which a wrong scaling actually moves. (L017: ask what
        // change would make it fail, then verify that it does.)
        let rows = &["......", ".XX...", ".XX...", "......"];
        let pal: Vec<PaletteEntry> = vec![('X', Color::rgb(0xff, 0, 0))];
        let sprite = Sprite::new(rows, &pal);

        let centre_of_ink = |scale: f32, e: &Explosion| -> f32 {
            let mut buf = vec![0u32; 256 * 256];
            {
                let mut c = Canvas::new(&mut buf, 256, 256);
                e.draw(&mut c, &sprite, 128.0, 200.0, scale, Color::BLACK);
            }
            let xs: Vec<f32> = (0..256)
                .filter(|&x| (0..256).any(|y| buf[y * 256 + x] != 0))
                .map(|x| x as f32)
                .collect();
            assert!(!xs.is_empty(), "nothing drew");
            (xs[0] + xs[xs.len() - 1] + 1.0) / 2.0
        };

        let young = Explosion::start(0.0, 0.0);
        let mut old = Explosion::start(0.0, 0.0);
        old.advance(BURN_TIME * 0.95);
        assert!(old.scale_factor() > young.scale_factor() * 2.0);

        let a = centre_of_ink(4.0, &young);
        let b = centre_of_ink(4.0, &old);
        assert!(
            (a - b).abs() < 2.0,
            "fireball walked {:.1}px sideways as it grew ({a} -> {b})",
            (a - b).abs()
        );
    }

    #[test]
    fn a_stalled_frame_cannot_run_it_backwards() {
        let mut e = Explosion::start(0.0, 0.0);
        e.advance(0.2);
        let t = e.life();
        e.advance(-1.0);
        e.advance(f32::NAN);
        assert_eq!(e.life(), t, "a bad dt moved the clock");
    }
}
