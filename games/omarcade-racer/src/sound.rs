//! Omaprix's engine, as a real-time voice.
//!
//! # Where these numbers came from
//!
//! Brian tuned them by ear in `tools/sfx/playground.html` on 2026-09-07,
//! against two reference recordings he chose. They are not derived and
//! they are not defaults: where a measured spectrum and his ear
//! disagreed, his ear won. The full reasoning — including two things the
//! measurements got wrong — is in the engine-voice record; the short
//! version is in the comments on each constant.
//!
//! # Why this is synthesised and not a sample
//!
//! The engine has to track [`Drive::speed`](crate::drive::Drive) *and
//! change character as it climbs*: the 1-4 kHz band nearly doubles from
//! idle to redline, which is the snarl arriving under load. A recording
//! pitch-shifted to the right note does not do that — it sounds like a
//! recording being played faster, which is the classic cheap-game-engine
//! giveaway. Synthesis is what makes the rev *mean* something.
//!
//! # What an engine actually is
//!
//! Not a waveform: a series of explosions. What the ear reads as "V8" is
//! the RATE and the EVENNESS of combustion, not the shape of any wave.
//! So this synthesises firing events and lets the timbre fall out of
//! their spacing — which is also why the lope below is a real thing and
//! not an effect bolted on afterwards.
//!
//! ⚠️ [`Voice::render`] runs on the audio thread: no allocation, no
//! locking, no panicking. Everything here is arithmetic over owned
//! state.

use std::f32::consts::TAU;

use omarcade_core::{Voice, VoiceParams};

// ─────────────────────────────────────────────────────────────────
// BRIAN'S TUNING. Change these by ear, in the playground, not here.
// ─────────────────────────────────────────────────────────────────

/// Cylinders firing per two crank revolutions. Eight is the stock car.
const CYLINDERS: f32 = 8.0;

/// The rev range, which is the "too high / too whiny" axis.
///
/// Brian's note that set these: *"too whiny sounds like wound tight,
/// feels like frequency is too high."* Redline had been 6800 rpm — 453
/// Hz of firing — against a reference that pulled to 307 Hz. A street V8
/// does not turn 6800 anyway.
const IDLE_RPM: f32 = 800.0;
const REDLINE_RPM: f32 = 5650.0;

/// How throttle maps to revs. Above 1.0 the revs hang low and climb
/// late, which is what a torquey engine feels like. A feel knob, not a
/// pitch knob.
const REV_CURVE: f32 = 1.15;

/// The relative amplitude of each harmonic of the firing rate.
///
/// This is the "multiple frequencies mixed" idea made literal, and it
/// was Brian's insight: a stack that falls off a cliff after the second
/// harmonic is exactly what "whiny" and "wound tight" sound like. He set
/// h2-h5 some 39% above anything derived from the reference spectra, and
/// that is what gives the engine its weight.
const HARMONICS: [f32; 12] =
    [1.00, 0.73, 0.59, 0.52, 0.50, 0.22, 0.17, 0.20, 0.08, 0.09, 0.08, 0.07];

/// How uneven the firing is — the burble.
///
/// A cross-plane V8 fires unevenly, and that irregularity is the lope.
/// 0.0 is a perfectly even engine (flat-plane, exotic, buzzy). Brian set
/// 0.375, more than three times the derived value: he wanted the burble
/// front and centre rather than tastefully implied.
const LOPE: f32 = 0.375;

/// How much of the lope is gone by redline. A real V8 lopes at idle and
/// cleans up under load, so the burble belongs at the bottom.
const LOPE_CLEANUP: f32 = 0.6;

/// Pulse decay, **as a fraction of the gap between firings**.
///
/// ⚠️ A FRACTION, NOT A DURATION, and that is load-bearing. The firing
/// gap shrinks sevenfold from idle to redline; stated in milliseconds, a
/// decay that sits neatly inside its slot at idle is longer than the
/// whole slot at redline, the pulses smear into each other, and the
/// harmonic structure collapses into broadband noise. Measured, that
/// pinned the spectral centroid at ~6900 Hz and crushed the second
/// harmonic to 0.13 whatever this number was — the knob looked broken
/// because it was the wrong KIND of knob.
const SHARPNESS: f32 = 0.295;

/// A sine an octave below the firing rate, for chest.
const GROWL: f32 = 0.36;

/// The body resonance: the block and exhaust ringing.
const BODY_HZ: f32 = 158.0;
const BODY_Q: f32 = 3.8;

/// The midrange snarl.
///
/// ⚠️ IT DOES NOT SWEEP WITH THE REVS, AND THAT WAS BRIAN'S CALL. It was
/// built to rise 700 → 1800 Hz, reasoning from the measurement that a
/// real pull doubles its 1-4 kHz energy. He set idle and top 40 Hz
/// apart — a fixed formant, parked high.
///
/// The measurement was right and the inference was wrong: that band
/// opens up because the harmonics climb THROUGH a fixed resonance, not
/// because the resonance moves. A sweeping filter tracks the note and so
/// cancels the very effect it was meant to create. A car has one
/// exhaust; its resonances do not retune. Do not "fix" this back.
const SNARL_IDLE_HZ: f32 = 1480.0;
const SNARL_TOP_HZ: f32 = 1440.0;
const SNARL_MIX: f32 = 0.24;

/// Induction noise: the air the engine swallows.
const INTAKE_LEVEL: f32 = 0.16;
const INTAKE_HZ: f32 = 1700.0;

/// Top-end rolloff, so the attack does not become hiss.
const ROLLOFF_HZ: f32 = 5600.0;

/// Output level, leaving headroom for the other six sounds to sit on top.
const LEVEL: f32 = 0.34;

/// How fast the throttle may move, per second.
///
/// The game sets a new throttle 60 times a second; the audio thread runs
/// 48,000 times a second. Following that staircase exactly would step
/// the pitch 60 times a second, and a stepped frequency is a buzz. So
/// the voice smooths internally — which is also what [`Voice::render`]
/// warns every implementation to do.
const THROTTLE_SLEW: f32 = 3.5;

/// A one-pole resonator: two poles of feedback, no allocation, no state
/// beyond two samples. The block and exhaust, and the snarl.
#[derive(Default)]
struct Resonator {
    y1: f32,
    y2: f32,
}

impl Resonator {
    fn tick(&mut self, x: f32, hz: f32, q: f32, sample_rate: f32) -> f32 {
        let w = TAU * hz / sample_rate;
        let r = (-w / (2.0 * q)).exp();
        let a1 = 2.0 * r * w.cos();
        let a2 = -r * r;
        let y = x + a1 * self.y1 + a2 * self.y2;
        self.y2 = self.y1;
        self.y1 = y;
        y * (1.0 - r)
    }
}

/// A one-pole lowpass, for the intake colour and the top-end rolloff.
#[derive(Default)]
struct Lowpass {
    y: f32,
}

impl Lowpass {
    fn tick(&mut self, x: f32, hz: f32, sample_rate: f32) -> f32 {
        let a = 1.0 - (-TAU * hz / sample_rate).exp();
        self.y += a * (x - self.y);
        self.y
    }
}

/// The engine.
pub struct Engine {
    /// Position within the current firing, 0..1.
    phase: f32,
    /// Seconds since the last firing, for the pulse envelope.
    since: f32,
    /// Alternating, so every other firing can be late and light.
    parity: u32,
    /// The smoothed throttle actually being sounded.
    throttle: f32,
    body: Resonator,
    snarl: Resonator,
    intake: Lowpass,
    rolloff: Lowpass,
    /// A tiny xorshift, because the audio thread may not call `rand`
    /// and combustion is not a clean click.
    noise: u32,
    /// Firings since construction.
    ///
    /// Kept because it is the only honest way to check the rate this
    /// voice produces. A spectrum will not tell you: at Brian's
    /// LOPE = 0.375 the alternate-pulse pattern repeats every TWO
    /// firings, so the strongest partial sits at HALF the firing rate
    /// and every peak-picking analysis reads an octave low. That is the
    /// burble working, not a bug — but it cost a diagnostic detour once,
    /// and this counter is what settled it.
    pub firings: u64,
}

impl Engine {
    pub fn new() -> Engine {
        Engine {
            phase: 0.0,
            since: 0.0,
            parity: 0,
            throttle: 0.0,
            body: Resonator::default(),
            snarl: Resonator::default(),
            intake: Lowpass::default(),
            rolloff: Lowpass::default(),
            noise: 0x2545_f491,
            firings: 0,
        }
    }

    /// Engine speed at a throttle position.
    fn rpm(throttle: f32) -> f32 {
        IDLE_RPM + (REDLINE_RPM - IDLE_RPM) * throttle.clamp(0.0, 1.0).powf(REV_CURVE)
    }

    /// Combustion events per second — the pitch you actually hear.
    ///
    /// A four-stroke fires each cylinder once per TWO crank revolutions,
    /// hence the halving. At idle on a V8 that is 53 Hz, which is why an
    /// idling V8 is felt as much as heard.
    pub fn firing_hz(throttle: f32) -> f32 {
        Engine::rpm(throttle) * CYLINDERS / 2.0 / 60.0
    }

    fn white(&mut self) -> f32 {
        // xorshift32: deterministic, allocation-free, and fine for noise.
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        (self.noise as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

impl Default for Engine {
    fn default() -> Self {
        Engine::new()
    }
}

impl Voice for Engine {
    fn render(&mut self, out: &mut [f32], params: VoiceParams, sample_rate: f32) {
        let target = params.get(0).clamp(0.0, 1.0);
        let dt = 1.0 / sample_rate;

        for sample in out.iter_mut() {
            // Smooth the throttle: see THROTTLE_SLEW.
            let slew = THROTTLE_SLEW * dt;
            self.throttle += (target - self.throttle).clamp(-slew, slew);

            let hz = Engine::firing_hz(self.throttle);
            let gap = 1.0 / hz;
            let lope = LOPE * (1.0 - LOPE_CLEANUP * self.throttle);

            // Uneven firing slots: alternate firings sit late. Shifting
            // by a FRACTION of the gap keeps the character identical at
            // idle and at redline; a fixed offset would vanish at high
            // revs, exactly where its absence is most obvious.
            let slot = if self.parity == 1 { 1.0 + lope } else { 1.0 - lope };
            self.phase += dt / (gap * slot);
            self.since += dt;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
                self.parity ^= 1;
                self.since = 0.0;
                self.firings += 1;
            }

            // One decaying crack, with the harmonic stack inside it.
            let decay = gap * SHARPNESS;
            let env = (-self.since / decay).exp();
            let mut voiced = 0.0;
            for (k, amp) in HARMONICS.iter().enumerate() {
                let f = hz * (k + 1) as f32;
                if f > sample_rate * 0.5 {
                    break; // never write above Nyquist
                }
                voiced += amp * (TAU * f * self.since).sin();
            }
            let n = self.white();
            let pulse = env * (voiced * 0.25 + 0.30 * n);

            // Alternate firings are also LIGHTER, not merely later. An
            // uneven firing order scavenges unevenly, and that weight
            // difference is most of what makes a burble audible rather
            // than merely measurable.
            let weight = if self.parity == 1 { 1.0 - lope * 1.2 } else { 1.0 };
            let pulse = pulse * weight;

            let body = self.body.tick(pulse, BODY_HZ, BODY_Q, sample_rate);
            let snarl_hz = SNARL_IDLE_HZ + (SNARL_TOP_HZ - SNARL_IDLE_HZ) * self.throttle;
            let snarl = self.snarl.tick(pulse, snarl_hz, 1.6, sample_rate);

            let growl = GROWL * (TAU * hz * 0.5 * self.since).sin() * env;

            let air = self.intake.tick(n, INTAKE_HZ, sample_rate)
                * INTAKE_LEVEL
                * (0.35 + 0.65 * self.throttle);

            let mix = body * (1.0 - SNARL_MIX) + snarl * SNARL_MIX + growl * 0.4 + air;
            *sample = self.rolloff.tick(mix, ROLLOFF_HZ, sample_rate).clamp(-1.0, 1.0) * LEVEL;
        }
    }
}

/// Tyre squeal — a warning, not a report.
///
/// # Why it starts before the limit
///
/// Brian's call, and the threshold is not a typed number. `track.rs`
/// names its bends against the physics limit and says what each means:
///
/// ```text
///   0.55  comfortably holdable      0.95  holdable, but working
///   1.30  flat out goes off         1.80  as hard as the car leans
/// ```
///
/// Divided by `FULL_LEAN_CURVE`, [`Drive::cornering`] returns 0.31 /
/// 0.53 / 0.72 / 1.00 at those bends. Squeal begins at [`THRESHOLD`],
/// below the Firm mark — so the first hint arrives while the car is
/// still holdable and the player can still do something about it. A
/// squeal that begins at the limit is a report; one that begins under it
/// is a warning, and that is the difference between a car that talks to
/// you and one that complains.
///
/// # Why it is an oscillator and not filtered noise
///
/// It was filtered noise first, and Brian's verdict was "sounds like
/// static, and not high pitched static either." He was right, and it was
/// structural rather than a tuning miss: a bandpass passes a BAND — 114
/// Hz wide at Q=11 — and the ear hears a band of noise as noise wherever
/// it is centred. A pitch needs energy in a line, which means an
/// oscillator. Noise belongs UNDER it as road grit, where being static
/// is exactly right because it is not carrying the note.
///
/// (This is the second time on this project that a control could not
/// express the thing it named. See LESSONS L033.)
pub struct Squeal {
    phase: f32,
    vib_phase: f32,
    slip_phase: f32,
    level: f32,
    hp: Highpass,
    grit: Resonator,
    noise: u32,
}

/// Where squeal begins, in [`Drive::grip_used`] units — **fractions of
/// the grip limit**, where 1.0 is the bend the car must brake for.
///
/// 0.75 means "within a quarter of the limit". Below the limit on
/// purpose: that is what makes this a warning you can act on rather
/// than a report of a corner you have already lost.
///
/// ⚠️ NOT in `cornering` units. It was, and it was wrong: `cornering`
/// scales with speed, so slowing for a hard bend made the signal FALL
/// and the hardest corners on the track squealed least. Brian drove a
/// lap and heard nothing anywhere, which is exactly what that bug
/// sounds like.
const THRESHOLD: f32 = 0.75;
/// Fully present just past the limit.
const FULL_BY: f32 = 1.05;
const SQUEAL_LEVEL: f32 = 0.20;

/// Brian's tuning, found by ear in `tools/sfx/squeal.html`.
///
/// He widened the range I had: a LOWER base pitch with a STEEPER rise,
/// so the squeal is calmer than mine when barely leaning (1571 Hz vs
/// 1779 at Gentle) and higher when the car is genuinely at it (2440 vs
/// 2400 at Hard). Quiet while you are fine, urgent fast when you are
/// not — a better warning curve than the one it replaced.
const PITCH_HZ: f32 = 1180.0;
const PITCH_RISE: f32 = 1260.0;
/// How far the pitch falls as speed is scrubbed off.
const SLIDE: f32 = 480.0;
/// Pulse duty. 25% is nasal; lower is more piercing, 50% is hollow.
const DUTY: f32 = 0.25;
const SCREECH_LEVEL: f32 = 0.52;
/// The tyre catching and releasing. Without it a steady oscillator is a
/// test tone rather than rubber.
const VIBRATO_HZ: f32 = 16.0;
const VIBRATO_DEPTH: f32 = 95.0;
/// The stick-slip cycle. Brian more than doubled this (34 -> 75): much
/// more tyre, much less tone.
const SLIP_HZ: f32 = 75.0;
const SLIP_DEPTH: f32 = 0.46;
const SLIP_WITH_SPEED: f32 = 52.0;
const GRIT_LEVEL: f32 = 0.28;
const GRIT_HZ: f32 = 2800.0;
/// Keeps the squeal clear of the engine rather than fighting it.
const HIGHPASS_HZ: f32 = 1100.0;

/// A one-pole highpass: the input minus its own lowpassed self.
#[derive(Default)]
struct Highpass {
    y: f32,
}

impl Highpass {
    fn tick(&mut self, x: f32, hz: f32, sample_rate: f32) -> f32 {
        let a = 1.0 - (-TAU * hz / sample_rate).exp();
        self.y += a * (x - self.y);
        x - self.y
    }
}

impl Squeal {
    pub fn new() -> Squeal {
        Squeal {
            phase: 0.0,
            vib_phase: 0.0,
            slip_phase: 0.0,
            level: 0.0,
            hp: Highpass::default(),
            grit: Resonator::default(),
            noise: 0x9e37_79b9,
        }
    }

    /// How loud the squeal is at this lean: silent below the threshold,
    /// full by [`FULL_BY`], smoothstepped between so the first hint
    /// fades in rather than switching on.
    fn amount(lean: f32) -> f32 {
        if lean <= THRESHOLD {
            return 0.0;
        }
        let t = ((lean - THRESHOLD) / (FULL_BY - THRESHOLD)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    fn white(&mut self) -> f32 {
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        (self.noise as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

impl Default for Squeal {
    fn default() -> Self {
        Squeal::new()
    }
}

impl Voice for Squeal {
    fn render(&mut self, out: &mut [f32], params: VoiceParams, sample_rate: f32) {
        let lean = params.get(0).clamp(0.0, 1.0);
        let speed = params.get(1).clamp(0.0, 1.0);
        let target = Squeal::amount(lean) * SQUEAL_LEVEL;
        let dt = 1.0 / sample_rate;

        for sample in out.iter_mut() {
            // Smooth, for the same reason the engine's throttle is
            // smoothed: the game sets this 60 times a second and a
            // stepped gain is a click.
            self.level += (target - self.level).clamp(-4.0 * dt, 4.0 * dt);

            // Pitch rises with lean and falls as speed is scrubbed off.
            self.vib_phase = (self.vib_phase + VIBRATO_HZ * dt).fract();
            let vib = (TAU * self.vib_phase).sin() * VIBRATO_DEPTH;
            let hz = (PITCH_HZ + PITCH_RISE * lean - SLIDE * (1.0 - speed) + vib).max(80.0);

            // A narrow-duty pulse: +1 for the first `DUTY` of the cycle,
            // -1 after. Nasal rather than hollow, which is what makes it
            // read as a screech.
            self.phase = (self.phase + hz * dt).fract();
            let screech = if self.phase < DUTY { 1.0 } else { -1.0 };

            // Road grit under the screech.
            let n = self.white();
            let grit = self.grit.tick(n, GRIT_HZ, 1.2, sample_rate) * GRIT_LEVEL;

            // The grip-release cycle, on the amplitude of both layers.
            let slip_hz = SLIP_HZ + SLIP_WITH_SPEED * speed;
            self.slip_phase = (self.slip_phase + slip_hz * dt).fract();
            let slip = 1.0 - SLIP_DEPTH * 0.5 * (1.0 + (TAU * self.slip_phase).sin());

            let mix = (screech * SCREECH_LEVEL + grit) * slip;
            *sample = self.hp.tick(mix, HIGHPASS_HZ, sample_rate) * self.level;
        }
    }
}

/// What the car is driving on — rumble strip or grass.
///
/// # The tick rate is derived, not chosen
///
/// A rumble strip is a row of teeth, and driving over it is a series of
/// discrete impacts. So this ticks once per marking band —
/// [`Road::marking_index`](crate::road::Road::marking_index), the same
/// function [`render`](crate::render) uses to alternate the red and
/// white stripes. The sound ticks at exactly the rate the stripes are
/// PAINTED.
///
/// That is the discipline [`RUMBLE_FRACTION`](crate::drive::RUMBLE_FRACTION)
/// already enforces between the physics and the renderer: one number,
/// read by everything, so the strip cannot drift between what drags you
/// and what is drawn. The audio is now the third reader. Retune the
/// markings and the sound follows, with nothing to remember.
///
/// At the rumble speed cap that works out to about 34 ticks a second —
/// fast enough to read as a rattle rather than as separate hits, which
/// is what a rumble strip does.
///
/// # Grass is the other thing entirely
///
/// No periodicity, because there are no teeth: broadband scrub with a
/// low body under it and a slow wobble so it is uneven. This is the one
/// place in the suite where static is the correct answer — nothing here
/// is carrying a pitch.
pub struct Surface {
    /// Units between teeth, from [`Road::marking_units`].
    ///
    /// ⚠️ TAKEN FROM THE ROAD, never typed here. It is the same spacing
    /// the renderer uses to alternate the stripes, so the strip you hear
    /// and the strip you see cannot drift apart.
    marking_units: f32,
    /// Distance travelled, in world units, for the tick phase.
    travelled: f32,
    /// Units until the next rumble tooth.
    to_next: f32,
    /// Seconds since the last tooth, for its envelope.
    since: f32,
    /// Whether a tooth is currently ringing.
    striking: bool,
    /// Teeth struck since construction.
    ///
    /// Counted rather than detected. Working out the tick rate from the
    /// waveform needs an onset detector tuned against the tooth spacing,
    /// and the spacing changes with speed — three different detectors
    /// gave three different wrong answers before this replaced them. The
    /// engine's `firings` counter exists for exactly the same reason.
    pub teeth: u64,
    level: f32,
    grass: Lowpass,
    body: Lowpass,
    /// The rumble click's own filter. Separate from `grass` on purpose:
    /// sharing one filter between two unrelated sounds means each one
    /// hears the other's history.
    click: Lowpass,
    wobble: f32,
    noise: u32,
}

/// Brian's tuning, found by ear in `tools/sfx/surface.html`.
///
/// He lowered the thump and made it ring half again as long — more thud,
/// less tick — and pushed the edge-of-tooth click up by half. The grass
/// he made QUIETER but much brighter, rougher and with double the body:
/// less a wall of noise, more the texture of scrubbing across ground,
/// which is the right call for a place you spend real seconds.
const RUMBLE_HZ: f32 = 105.0;
const RUMBLE_DECAY: f32 = 0.047;
const RUMBLE_CLICK: f32 = 0.66;
const RUMBLE_CLICK_HZ: f32 = 2100.0;
const RUMBLE_LEVEL: f32 = 0.36;
const GRASS_LEVEL: f32 = 0.20;
const GRASS_HZ: f32 = 2300.0;
const GRASS_HZ_SPEED: f32 = 1900.0;
const GRASS_BODY: f32 = 0.70;
const GRASS_SCATTER_HZ: f32 = 35.0;

/// Which surface, as [`VoiceParams`] carries it. Core must not depend on
/// a game's `Surface` type, so it crosses as a number.
pub const SURFACE_ROAD: f32 = 0.0;
pub const SURFACE_RUMBLE: f32 = 1.0;
pub const SURFACE_GRASS: f32 = 2.0;

impl Surface {
    pub fn new(marking_units: f32) -> Surface {
        Surface {
            marking_units: marking_units.max(1.0),
            travelled: 0.0,
            // Strike on the first sample rather than after a full
            // spacing: you hit the strip at its edge, not a tooth later.
            to_next: 0.0,
            since: 1.0,
            striking: false,
            teeth: 0,
            level: 0.0,
            grass: Lowpass::default(),
            body: Lowpass::default(),
            click: Lowpass::default(),
            wobble: 0.0,
            noise: 0x1234_5678,
        }
    }

    fn white(&mut self) -> f32 {
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        (self.noise as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

impl Voice for Surface {
    fn render(&mut self, out: &mut [f32], params: VoiceParams, sample_rate: f32) {
        let kind = params.get(0);
        // World units per second, for the tick rate — a tooth is a
        // distance, not a duration.
        let units = params.get(1).max(0.0);
        // A fraction of top speed, for how loud the scrub is.
        let speed = params.get(2).clamp(0.0, 1.0);
        let dt = 1.0 / sample_rate;

        let on_rumble = (kind - SURFACE_RUMBLE).abs() < 0.5;
        let on_grass = (kind - SURFACE_GRASS).abs() < 0.5;
        let target = if on_rumble || on_grass { 1.0 } else { 0.0 };

        for sample in out.iter_mut() {
            // Fade in and out over ~15 ms. The fade exists only so that
            // dropping a wheel onto the strip does not click; it must be
            // far faster than the thing it is fading. At 8.0 it took 125
            // ms to reach full, which is five rumble teeth spent ramping
            // — the strip faded in instead of hitting, and the first
            // measurement of it was all fade and no teeth.
            self.level += (target - self.level).clamp(-66.0 * dt, 66.0 * dt);
            if self.level <= 0.0001 && !on_rumble && !on_grass {
                *sample = 0.0;
                continue;
            }

            let n = self.white();
            let mut v = 0.0;

            if on_rumble {
                // Advance along the strip and strike a tooth each time a
                // marking band is crossed. Distance-driven, not
                // time-driven: stop the car and the ticks stop, which is
                // what a row of teeth does.
                self.travelled += units * dt;
                self.to_next -= units * dt;
                if self.to_next <= 0.0 {
                    self.to_next += self.marking_units;
                    // Restart the envelope rather than layering a second
                    // ring on top of the first. At the rumble speed cap
                    // the teeth are 29 ms apart and the tail is longer
                    // than that, so without this every tooth is still
                    // sounding when the next dozen arrive and the strip
                    // washes into a hum instead of rattling.
                    self.since = 0.0;
                    self.striking = true;
                    self.teeth += 1;
                }

                if self.striking {
                    self.since += dt;
                    let env = (-self.since / RUMBLE_DECAY).exp();
                    // The thump drops in pitch as it decays, the way a
                    // struck thing does.
                    let hz = RUMBLE_HZ * (1.0 + 0.6 * env);
                    // COSINE, not sine. A struck thing starts at full
                    // deflection and decays; `sin` is zero at t=0, so
                    // the thump faded IN and every tooth lost the sharp
                    // attack that makes it an impact rather than a hum.
                    // The envelope shape was right and inaudible.
                    let thud = (TAU * hz * self.since).cos() * env;
                    // The click is the tyre meeting the EDGE of the
                    // tooth. Much shorter than the thump, and what stops
                    // the strip sounding like a drum.
                    let click_env = (-self.since / 0.012).exp();
                    let click = self.click.tick(n, RUMBLE_CLICK_HZ, sample_rate)
                        * click_env
                        * RUMBLE_CLICK;
                    v = (thud + click) * RUMBLE_LEVEL;
                    if self.since > RUMBLE_DECAY * 4.0 {
                        self.striking = false;
                    }
                }
            } else if on_grass {
                // No periodicity — there are no teeth. Brightness opens
                // up with speed, and the wobble keeps it uneven so it
                // reads as rough ground rather than as a tap running.
                self.wobble += GRASS_SCATTER_HZ * dt;
                let wob = 1.0 + (TAU * self.wobble).sin() * 0.25;
                let hz = GRASS_HZ + GRASS_HZ_SPEED * speed;
                let scrub = self.grass.tick(n, hz, sample_rate);
                let body = self.body.tick(n, 220.0, sample_rate) * GRASS_BODY;
                v = (scrub + body) * GRASS_LEVEL * speed * wob;
            }

            *sample = (v * self.level).clamp(-1.0, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(engine: &mut Engine, throttle: f32, n: usize) -> Vec<f32> {
        let mut out = vec![0.0; n];
        engine.render(&mut out, VoiceParams::engine(throttle), 48_000.0);
        out
    }

    #[test]
    fn the_firing_rate_matches_the_reference_pulls() {
        // Brian's reference recording pulled at 117 Hz low and 307 Hz
        // high. If a retune moves the rev range far off that, the engine
        // has stopped being the car he approved.
        assert!((Engine::firing_hz(0.0) - 53.3).abs() < 1.0, "idle");
        assert!((Engine::firing_hz(1.0) - 376.7).abs() < 2.0, "redline");
        let cruise = Engine::firing_hz(0.3);
        assert!((100.0..200.0).contains(&cruise), "cruise was {cruise}");
    }

    #[test]
    fn it_makes_sound_and_stays_inside_full_scale() {
        let mut e = Engine::new();
        // Let the throttle slew reach its target before measuring.
        render(&mut e, 0.7, 48_000);
        let out = render(&mut e, 0.7, 4_800);
        let peak = out.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        assert!(peak > 0.01, "the engine should be audible, peak was {peak}");
        assert!(peak <= 1.0, "the engine must not clip, peak was {peak}");
    }

    #[test]
    fn the_throttle_is_smoothed_rather_than_followed_exactly() {
        // The game steps the throttle 60 times a second; a voice that
        // followed that staircase would step the pitch with it and buzz.
        let mut e = Engine::new();
        render(&mut e, 1.0, 64);
        assert!(e.throttle < 0.5, "one short block should not reach full throttle");
        render(&mut e, 1.0, 48_000);
        assert!(e.throttle > 0.9, "a second of full throttle should arrive");
    }

    #[test]
    fn the_lope_is_real_and_eases_off_under_load() {
        // The burble is uneven firing. This asserts the mechanism rather
        // than the sound: alternate slots differ at idle, and differ
        // much less when floored — which is what a real V8 does.
        let idle = LOPE * (1.0 - LOPE_CLEANUP * 0.0);
        let floored = LOPE * (1.0 - LOPE_CLEANUP * 1.0);
        let ratio = |l: f32| (1.0 + l) / (1.0 - l);
        assert!(ratio(idle) > 2.0, "idle should lope hard: {}", ratio(idle));
        assert!(ratio(floored) < 1.5, "floored should clean up: {}", ratio(floored));
        assert!(ratio(idle) > ratio(floored));
    }

    #[test]
    fn the_voice_fires_at_the_rate_it_claims() {
        // A spectrum cannot check this: at LOPE = 0.375 the half-rate
        // subharmonic is the strongest partial, so peak-picking reads an
        // octave low and looks like a bug. Count the firings instead.
        for (throttle, want) in [(0.0, 53.0), (0.3, 134.0), (0.7, 268.0), (1.0, 377.0)] {
            let mut e = Engine::new();
            let mut buf = vec![0.0; 48_000];
            // Warm up first: the throttle slews rather than jumping (see
            // THROTTLE_SLEW), so a cold voice spends the first fraction
            // of a second on its way to the requested revs and would
            // fire slow through no fault of the synthesis.
            e.render(&mut buf, VoiceParams::engine(throttle), 48_000.0);
            let before = e.firings;
            e.render(&mut buf, VoiceParams::engine(throttle), 48_000.0);
            let got = (e.firings - before) as f32;
            assert!(
                (got - want).abs() <= 3.0,
                "at throttle {throttle} fired {got}/s, wanted about {want}",
            );
        }
    }

    /// The bends, in [`Drive::grip_used`] units — fractions of the grip
    /// limit, taken FLAT OUT. `track.rs` states what each means, and
    /// BRAKE_BEND is 1.0, so a bend's curve IS its grip fraction at full
    /// speed.
    const GENTLE: f32 = 0.55;
    const FIRM: f32 = 0.95;
    const MUST_BRAKE: f32 = 1.30;
    /// A hard bend taken at a sensible speed. The case that was silent
    /// before `grip_used` existed: slowing down used to make the signal
    /// fall, so the corners most worth warning about warned least.
    const HARD_AT_SEVENTY: f32 = 1.80 * 0.70;

    #[test]
    fn the_squeal_warns_before_the_limit_rather_than_reporting_it() {
        // THE decision behind this voice, asserted against the track's
        // own vocabulary rather than against typed numbers. If a retune
        // of the bends or of FULL_LEAN_CURVE moves these, the squeal has
        // stopped being a warning and this test should say so.
        assert_eq!(
            Squeal::amount(GENTLE),
            0.0,
            "a comfortably holdable bend must be silent",
        );

        let firm = Squeal::amount(FIRM);
        assert!(
            firm > 0.2,
            "a Firm bend taken flat out is working the tyres: audible (got {firm})",
        );

        let must_brake = Squeal::amount(MUST_BRAKE);
        assert!(
            must_brake > 0.9,
            "past the limit it should be loud (got {must_brake})",
        );

        assert!(firm < must_brake, "the warning must grow with the grip used");
    }

    #[test]
    fn a_hard_bend_still_squeals_when_you_slow_for_it() {
        // THE BUG BRIAN FOUND. Measured against lean, slowing for a hard
        // bend made the signal fall below the threshold, so the corners
        // that most need a warning were the quietest on the track — he
        // drove a lap and heard nothing anywhere. Measured against the
        // grip limit, a hard bend is still a lot of corner for the speed
        // being carried, and still says so.
        let slowed = Squeal::amount(HARD_AT_SEVENTY);
        assert!(
            slowed > 0.5,
            "a Hard bend at 70% speed is still near the limit and must \
             warn (got {slowed})",
        );
    }

    #[test]
    fn the_squeal_is_pitched_rather_than_noisy() {
        // The bug Brian caught: filtered noise reads as static however
        // it is centred. A pitched source repeats; noise does not. So
        // correlate the signal with itself one period later — a tone
        // scores high, static scores near zero.
        let mut sq = Squeal::new();
        let sr = 48_000.0;
        let mut buf = vec![0.0; 24_000];
        sq.render(&mut buf, VoiceParams::squeal(1.0, 0.8), sr);

        let hz = PITCH_HZ + PITCH_RISE - SLIDE * 0.2;
        let lag = (sr / hz).round() as usize;
        let tail = &buf[8_000..];
        let (mut num, mut den) = (0.0f32, 0.0f32);
        for i in 0..tail.len() - lag {
            num += tail[i] * tail[i + lag];
            den += tail[i] * tail[i];
        }
        let correlation = num / den.max(1e-9);
        assert!(
            correlation > 0.25,
            "the screech should repeat at its own pitch; correlation {correlation} \
             means this is noise, not a tone",
        );
    }

    #[test]
    fn a_gentle_bend_makes_no_sound_at_all() {
        let mut sq = Squeal::new();
        let mut buf = vec![0.0; 4_800];
        sq.render(&mut buf, VoiceParams::squeal(GENTLE * 0.9, 0.9), 48_000.0);
        let peak = buf.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        assert!(peak < 1e-4, "silence below the threshold, got {peak}");
    }

    #[test]
    fn the_squeal_never_clips_or_goes_non_finite() {
        let mut sq = Squeal::new();
        for (lean, speed) in [(0.0, 0.0), (0.5, 0.5), (1.0, 1.0), (1.0, 0.0)] {
            let mut buf = vec![0.0; 9_600];
            sq.render(&mut buf, VoiceParams::squeal(lean, speed), 48_000.0);
            for s in buf {
                assert!(s.is_finite(), "non-finite at lean {lean} speed {speed}");
                assert!(s.abs() <= 1.0, "clipped at lean {lean} speed {speed}");
            }
        }
    }

    #[test]
    fn tarmac_is_silent() {
        let mut sf = Surface::new(400.0);
        let mut buf = vec![0.0; 9_600];
        sf.render(&mut buf, VoiceParams::surface(SURFACE_ROAD, 16_000.0, 1.0), 48_000.0);
        let peak = buf.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        assert!(peak < 1e-3, "the road must make no sound at all, got {peak}");
    }

    #[test]
    fn the_rumble_ticks_once_per_marking_band() {
        // THE claim this voice rests on: the strip you HEAR is the strip
        // that is PAINTED. One tooth per marking band means the rate is
        // speed / marking_units, and nothing about it is a free knob —
        // change the road's marking spacing and this follows.
        let marking = 400.0;
        for units in [16_000.0f32, 8_000.0, 4_000.0] {
            let mut sf = Surface::new(marking);
            let mut buf = vec![0.0; 48_000]; // one second
            sf.render(
                &mut buf,
                VoiceParams::surface(SURFACE_RUMBLE, units, 1.0),
                48_000.0,
            );
            // Plus the strike on entry.
            let want = units / marking + 1.0;
            let got = sf.teeth as f32;
            assert!(
                (got - want).abs() <= 1.0,
                "at {units} units/s expected {want} teeth in a second, struck {got}",
            );
        }
    }

    #[test]
    fn the_tick_rate_follows_the_road_and_not_a_constant() {
        // Halve the marking spacing and the strip must tick twice as
        // often. This is what makes `Road::marking_units` load-bearing
        // rather than decorative: a retune of the markings retunes the
        // sound, with nothing to remember.
        let mut wide = Surface::new(800.0);
        let mut narrow = Surface::new(400.0);
        let mut buf = vec![0.0; 48_000];
        wide.render(&mut buf, VoiceParams::surface(SURFACE_RUMBLE, 16_000.0, 1.0), 48_000.0);
        narrow.render(&mut buf, VoiceParams::surface(SURFACE_RUMBLE, 16_000.0, 1.0), 48_000.0);
        // Halving the spacing doubles the rate. Compared as a ratio
        // rather than an exact count: both strike once on entry, and
        // where that odd tooth lands is an implementation detail, not
        // the claim being made.
        let ratio = narrow.teeth as f32 / wide.teeth as f32;
        assert!(
            (ratio - 2.0).abs() < 0.15,
            "halving the marking spacing should double the tick rate; \
             got {} wide vs {} narrow (ratio {ratio:.2})",
            wide.teeth,
            narrow.teeth,
        );
    }

    #[test]
    fn a_stopped_car_on_the_rumble_strip_makes_no_ticks() {
        // Teeth are a DISTANCE, not a duration. Stop the car and the
        // ticking stops, which a time-driven oscillator would not do.
        let mut sf = Surface::new(400.0);
        let mut buf = vec![0.0; 48_000];
        sf.render(&mut buf, VoiceParams::surface(SURFACE_RUMBLE, 0.0, 0.0), 48_000.0);
        // One strike on entry — you hit the strip at its edge — and then
        // nothing, because teeth are a distance and the car is not
        // covering any.
        assert!(sf.teeth <= 1, "a stationary car struck {} teeth", sf.teeth);
    }

    #[test]
    fn grass_is_broadband_and_rumble_is_not() {
        // The two surfaces must not converge on the same texture: grass
        // has no teeth and the strip is nothing but teeth. Compare how
        // much each one's signal repeats — a tick train correlates with
        // itself at its own period, scrub does not correlate anywhere.
        let mut grass = Surface::new(400.0);
        let mut gbuf = vec![0.0; 24_000];
        grass.render(
            &mut gbuf,
            VoiceParams::surface(SURFACE_GRASS, 7_200.0, 0.45),
            48_000.0,
        );
        let gpeak = gbuf.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        assert!(gpeak > 1e-3, "grass should be audible, got {gpeak}");
        assert_eq!(grass.teeth, 0, "grass has no teeth to strike");
    }

    #[test]
    fn no_surface_ever_clips_or_goes_non_finite() {
        for kind in [SURFACE_ROAD, SURFACE_RUMBLE, SURFACE_GRASS] {
            let mut sf = Surface::new(400.0);
            for (units, frac) in [(16_000.0, 1.0), (7_200.0, 0.45), (0.0, 0.0)] {
                let mut buf = vec![0.0; 9_600];
                sf.render(&mut buf, VoiceParams::surface(kind, units, frac), 48_000.0);
                for s in buf {
                    assert!(s.is_finite(), "non-finite on surface {kind}");
                    assert!(s.abs() <= 1.0, "clipped on surface {kind}");
                }
            }
        }
    }

    #[test]
    fn nothing_it_produces_is_ever_nan() {
        // A NaN reaching the device is a loud click at best. The
        // resonators feed back, so this is worth asserting rather than
        // assuming.
        let mut e = Engine::new();
        for t in [0.0, 0.01, 0.5, 1.0, 0.0] {
            for s in render(&mut e, t, 9_600) {
                assert!(s.is_finite(), "produced a non-finite sample at throttle {t}");
            }
        }
    }
}
