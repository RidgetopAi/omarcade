#!/usr/bin/env python3
"""Omaprix engine tone — a stock-car V8, synthesized from firing events.

Run it, listen, change a number in TUNING, run it again. That loop is the
whole point of this file: every complaint you can say out loud ("too
harsh", "too high", "doesn't burble") is one named constant here.

    python3 engine.py              # writes the four audition WAVs
    pw-play out/engine_70.wav      # listen

WHY PULSES AND NOT A WAVEFORM
An engine is not a tone. It is a series of explosions, and what your ear
identifies as "V8" is the RATE and the EVENNESS of those explosions, not
the shape of any wave. So this synthesizes combustion events and lets the
timbre fall out of them, rather than picking an oscillator and hoping.
That is also why the low end sounds like an engine instead of a hum: the
fundamental you hear IS the firing rate, not something we tuned to.

NOT REAL-TIME (yet). This writes files so you can judge the timbre with
no Rust and no backend decision. The math is deliberately portable: if we
go real-time in Rust later, it is this, per frame, with `throttle` read
from `Drive::speed / Tuning::top_speed`.
"""

import math
import os
import struct
import wave

import numpy as np

SAMPLE_RATE = 44_100

# ─────────────────────────────────────────────────────────────────────
# TUNING — the whole conversation lives in this block.
# ─────────────────────────────────────────────────────────────────────

# How many cylinders fire per two crank revolutions. Eight is the stock
# car. Four would make it a hot hatch, twelve an exotic. This changes the
# firing rate for a given RPM, so it moves the pitch as well as the
# character — expect to re-check the RPMs below if you change it.
CYLINDERS = 8

# The rev range. These two decide PITCH, which is the "too high / too
# low" axis. A stock car idles low and lazy and does not scream: 750 to
# 6200 is a street V8. An F1 would be 4000 to 18000 and is exactly the
# sound we are avoiding.
IDLE_RPM = 800.0
REDLINE_RPM = 5650.0

# How the throttle maps to RPM. 1.0 is linear. Above 1.0 the revs hang
# low and then climb late, which is what a torquey engine feels like;
# below 1.0 it leaps off idle. This is a FEEL knob, not a pitch knob.
RPM_CURVE = 1.15

# ── THE HARMONIC STACK ───────────────────────────────────────────────
#
# One amplitude per partial, as a multiple of the firing rate. This is
# the control Brian actually tuned with (playground.html), and these are
# HIS numbers, found by ear on 2026-09-07.
#
# It is the "multiple frequencies mixed" idea made literal: harmonic 1 is
# the firing rate itself, and everything above it is what turns a bare
# pulse rate into an engine. The low partials are WEIGHT — a stack that
# falls off a cliff after h2 is precisely what "whiny" and "wound tight"
# sound like, which is how this got here.
#
# Read them as a shape, not as twelve independent numbers: a slow, even
# decline through h5 is thickness; a cliff is thinness; a bump at one
# partial is a resonance that will follow the revs and sound like a
# whistle.
HARMONICS = [1.00, 0.73, 0.59, 0.52, 0.50, 0.22, 0.17, 0.20, 0.08, 0.09, 0.08, 0.07]

# ── The four layers ──────────────────────────────────────────────────

# 1. LOPE — the reason a cross-plane V8 sounds American.
# Its firing order is uneven: the gap between some pairs of combustion
# events is longer than others, and that irregular pulse is the burble.
# 0.0 is a perfectly even engine (flat-plane, exotic, buzzy). 0.30 is a
# pronounced lope. Raise this if it sounds too clean or too European.
LOPE = 0.375

# How much of the lope is gone by redline. 1.0 means perfectly even at
# full throttle, 0.0 means it lopes just as hard flat-out. See
# `pulse_positions` for why this is not a constant.
LOPE_CLEANUP = 0.6

# 2. HARMONIC_ROLLOFF — brightness, i.e. the "harsh" axis.
# How fast each combustion thump decays, AS A FRACTION OF THE GAP BETWEEN
# FIRINGS. A short decay is a sharp crack with lots of high harmonics
# (harsh, F1, 8-bit); a long one is a round thump (muffled, muscular).
# LOWER = HARSHER.
#
# ⚠️ A FRACTION, NOT A DURATION, and that is load-bearing (L019). The
# firing gap shrinks SEVEN TIMES from idle to redline. Stated in
# milliseconds, a decay that sits neatly inside its slot at idle is
# LONGER THAN THE WHOLE SLOT at redline: pulses smear into each other,
# the pulse train stops being a pulse train, and the harmonic structure
# collapses into broadband noise. Measured, that pinned the centroid at
# ~6900 Hz and crushed harmonic 2 to 0.13 no matter what this number was
# set to — the knob looked broken because it was the wrong KIND of knob.
# As a ratio the pulse keeps the same shape relative to its slot at every
# rev, which is what makes the timbre hold together across the range.
HARMONIC_ROLLOFF = 0.295

# How much the thump sharpens as the revs rise. Real engines get brighter
# and angrier at the top end and this is what carries that. 0.0 means the
# timbre never changes with speed, which sounds like a recording being
# pitch-shifted — the classic cheap-game-engine giveaway.
ROLLOFF_TIGHTENING = 0.34

# 3. BODY — the resonant thump of the block and exhaust.
# Combustion pulses alone are thin and clicky. Ringing them through a
# low resonance is the difference between "engine" and "typewriter".
BODY_HZ = 158.0
BODY_Q = 3.8          # higher = more tuned/hollow, lower = more diffuse

# How much of the sound is the resonant body versus the raw pulse crack.
# Body-led on purpose: the resonator is what makes this an engine rather
# than a noise gate, and the dry pulses are only there to keep the attack
# defined. Raise toward 1.0 for a more distant/muffled car, lower for a
# more percussive one with the induction right in your ear.
BODY_MIX = 0.72

# A SECOND resonance an octave up, which is what puts real weight into
# harmonic 2. Measured, the reference runs h2 at 0.29-0.68 of the
# fundamental; a single resonator at BODY_HZ produced 0.06-0.20, because
# nothing in the chain was ringing up there at all. An exhaust system has
# more than one resonant mode, so this is the physics, not a fudge.
OCTAVE_HZ_RATIO = 2.0
OCTAVE_Q = 3.0
OCTAVE_MIX = 0.42

# A third resonance, in the MIDRANGE — the snarl. This is the band the
# reference actually lives in (23% of its energy at low revs, 37% on the
# pull) and the one a two-resonator engine leaves empty: measured, the
# body modes sat at 70-234 Hz and the raw pulses at 4500-9400 Hz, with a
# hole between them that read as "thin and hissy at the same time".
#
# ⚠️ IT DOES NOT SWEEP ANY MORE, AND THAT WAS BRIAN'S CALL. I built this
# to rise 700 -> 1800 Hz with the revs, reasoning from the reference that
# the 1-4 kHz band doubles on a pull. He set idle and top 40 Hz apart —
# a FIXED formant, parked high — and that is what sounded right to him.
#
# The measurement was not wrong; the inference was. That band does open
# up on a real pull, but it opens because the harmonics climb THROUGH a
# fixed resonance, not because the resonance itself moves. A sweeping
# filter tracks the note and so cancels the very effect it was meant to
# create. The car has one exhaust; its resonances do not retune.
#
# The two constants are kept separate rather than collapsed into one so
# the sweep stays available if a later ear wants it.
SNARL_HZ_IDLE = 1480.0
SNARL_HZ_REDLINE = 1440.0
SNARL_Q = 1.6
SNARL_MIX = 0.24

# 4. INTAKE — air moving. Filtered noise, grows with throttle.
# Without this the engine sounds synthetic no matter how good the pulses
# are, because a real engine is also a very large air pump.
INTAKE_AT_IDLE = 0.02
INTAKE_AT_REDLINE = 0.16
INTAKE_BRIGHTNESS = 0.10   # 0 = dark rush, 1 = hissy

# ── Overall ──────────────────────────────────────────────────────────

# Peak normalization target. Leaves headroom for the other six sounds to
# sit on top without clipping the mix.
PEAK = 0.72


# ─────────────────────────────────────────────────────────────────────
# The synthesis
# ─────────────────────────────────────────────────────────────────────

def rpm_at(throttle: float) -> float:
    """Engine speed at a throttle position in 0..1."""
    t = min(max(throttle, 0.0), 1.0) ** RPM_CURVE
    return IDLE_RPM + (REDLINE_RPM - IDLE_RPM) * t


def firing_hz(throttle: float) -> float:
    """Combustion events per second.

    A four-stroke fires every cylinder once per TWO crank revolutions,
    hence the /2. This is the frequency you actually hear as the pitch
    of the engine — at idle on a V8 it is 50Hz, which is why an idling
    V8 is felt as much as heard.
    """
    return rpm_at(throttle) * CYLINDERS / 2.0 / 60.0


def pulse_positions(throttle: float, seconds: float) -> np.ndarray:
    """Sample indices where a cylinder fires, with LOPE applied.

    The lope shifts every other pulse later in its slot. Pairs of events
    end up unevenly spaced — long-short-long-short — which is the burble.
    Shifting by a FRACTION OF THE GAP rather than a fixed number of
    milliseconds keeps the character identical at idle and at redline;
    a fixed offset would vanish at high revs, exactly where you can hear
    it least but expect it most.
    """
    # The lope EASES OFF as the revs rise. Measured on the reference, the
    # odd/even harmonic balance stays inside 1.4-2.5 at every rev; a
    # constant lope put mine at 28 in the midrange, which reads as a
    # chuggy, uneven cruise. A real V8 lopes at idle and cleans up under
    # load, so the burble belongs at the bottom of the range.
    lope = LOPE * (1.0 - LOPE_CLEANUP * min(max(throttle, 0.0), 1.0))

    hz = firing_hz(throttle)
    gap = SAMPLE_RATE / hz
    count = int(seconds * hz)
    idx = np.arange(count, dtype=np.float64)
    shift = np.where(idx % 2 == 1, gap * lope * 0.5, 0.0)
    return idx * gap + shift


def combustion(throttle: float, seconds: float) -> np.ndarray:
    """The pulse train: one decaying crack per cylinder firing."""
    n = int(seconds * SAMPLE_RATE)
    out = np.zeros(n, dtype=np.float64)

    # Brighter (shorter) thumps at high revs. See ROLLOFF_TIGHTENING.
    #
    # The decay is a FRACTION OF THE FIRING GAP, so the pulse keeps its
    # shape relative to its slot at every rev. See HARMONIC_ROLLOFF for
    # what happens when this is an absolute duration instead.
    gap = 1.0 / firing_hz(throttle)
    decay = gap * HARMONIC_ROLLOFF * (1.0 - ROLLOFF_TIGHTENING * throttle)
    decay = max(decay, 0.00015)
    tail = int(decay * 6.0 * SAMPLE_RATE)

    # One pulse shape, stamped repeatedly — the shape is identical for
    # every cylinder, so any character in the result comes from the
    # SPACING, which is the claim this whole file rests on.
    t = np.arange(tail) / SAMPLE_RATE
    env = np.exp(-t / decay)

    # A little noise inside the pulse: combustion is not a clean click.
    # It MUST be dark noise. White noise here is flat to 22kHz, and no
    # amount of resonator downstream rescues it — measured, the dry path
    # put more energy at 6kHz than at the firing rate, which is the exact
    # hiss we are trying not to make. Two poles of lowpass at birth.
    # ONE short smoothing pass, not two long ones. White noise here is
    # wrong (it hisses; measured, it beat the firing rate at 6kHz), but
    # over-filtering is equally wrong in the other direction: the
    # reference keeps 22-43% of its energy in 1-4kHz and that band IS
    # combustion texture. This is the middle the two references agree on.
    rng = np.random.default_rng(0xC0FFEE)
    noise = rng.standard_normal(tail)
    noise = np.convolve(noise, np.ones(5) / 5.0, mode="same")
    noise /= max(np.abs(noise).max(), 1e-9)

    # The voiced part of the pulse is the HARMONIC STACK summed at the
    # firing rate — the same construction as playground.html's makePulse,
    # so what Brian tuned by ear is what this generator produces. It
    # replaces a single typed tone under the crack: one sine could never
    # express h2 0.73 / h3 0.59 / h4 0.52, and measured, the old shape
    # was delivering those partials at 0.01-0.07.
    hz = firing_hz(throttle)
    voiced = np.zeros(tail)
    for k, amp in enumerate(HARMONICS, start=1):
        if amp <= 0.0:
            continue
        if hz * k > SAMPLE_RATE / 2:      # never write above Nyquist
            break
        voiced += amp * np.sin(2 * math.pi * hz * k * t)
    voiced /= max(np.abs(voiced).max(), 1e-9)

    shape = env * (voiced + 0.30 * noise)

    # Alternate firings are also QUIETER, not merely later. See LOPE.
    lope = LOPE * (1.0 - LOPE_CLEANUP * min(max(throttle, 0.0), 1.0))
    for j, pos in enumerate(pulse_positions(throttle, seconds)):
        i = int(pos)
        if i >= n:
            break
        end = min(i + tail, n)
        weight = 1.0 - lope * 1.2 if j % 2 else 1.0
        out[i:end] += shape[: end - i] * weight

    return out


def resonate(sig: np.ndarray, hz: float, q: float) -> np.ndarray:
    """A one-pole-pair resonant bandpass — the block and exhaust ringing.

    Written out by hand rather than pulled from scipy: it is six lines,
    and it keeps this file runnable with nothing but numpy.
    """
    w = 2 * math.pi * hz / SAMPLE_RATE
    r = math.exp(-w / (2 * q))
    a1, a2 = 2 * r * math.cos(w), -r * r
    out = np.zeros_like(sig)
    y1 = y2 = 0.0
    for i, x in enumerate(sig):
        y = x + a1 * y1 + a2 * y2
        out[i] = y
        y2, y1 = y1, y
    return out * (1 - r)


def intake(throttle: float, seconds: float) -> np.ndarray:
    """Filtered noise: the air the engine is swallowing."""
    n = int(seconds * SAMPLE_RATE)
    rng = np.random.default_rng(0x5EED)
    noise = rng.standard_normal(n)

    # A one-pole lowpass. INTAKE_BRIGHTNESS 0 is a dark rush, 1 is hiss.
    #
    # The cutoff OPENS WITH THE REVS. A fixed colour was the last bright
    # thing in the mix: at redline it laid flat hiss over a good engine,
    # because only its level tracked throttle and never its tone. A real
    # induction gets both louder and brighter as it swallows more air,
    # and tying the two together is what stops the noise layer reading as
    # a separate hiss sitting on top of the car.
    a = (0.02 + 0.55 * INTAKE_BRIGHTNESS) * (0.45 + 0.55 * throttle)
    out = np.zeros(n)
    y = 0.0
    for i, x in enumerate(noise):
        y += a * (x - y)
        out[i] = y

    level = INTAKE_AT_IDLE + (INTAKE_AT_REDLINE - INTAKE_AT_IDLE) * throttle
    return out * level / max(np.abs(out).max(), 1e-9)


# How far above the firing rate the raw pulse attack is allowed to reach,
# as a multiple of it. The dry layer exists for the leading edge of each
# combustion event; measured, it ran to a 9.4 kHz centroid at redline and
# was the single largest thing pulling the mix bright. Above this it is
# only hiss, and the snarl resonance already covers the band that matters.
ATTACK_REACH = 17.0


def render(throttle: float, seconds: float = 2.0) -> np.ndarray:
    """The whole engine at one throttle position."""
    pulses = combustion(throttle, seconds)

    # Tame the dry attack's top end. See ATTACK_REACH.
    # Clamped at BOTH ends. Without the floor, idle's 60 Hz firing rate
    # put the cutoff at 840 Hz and filtered the attack out of existence;
    # without the ceiling, redline reaches into pure hiss.
    cutoff = min(max(firing_hz(throttle) * ATTACK_REACH, 2600.0), 7000.0)
    a_lp = 1.0 - math.exp(-2 * math.pi * cutoff / SAMPLE_RATE)
    smoothed = np.zeros_like(pulses)
    y = 0.0
    for i, x in enumerate(pulses):
        y += a_lp * (x - y)
        smoothed[i] = y
    pulses = smoothed

    # Two resonant modes, not one: the fundamental body and an octave-up
    # mode that carries harmonic 2. See OCTAVE_MIX.
    body = resonate(pulses, BODY_HZ, BODY_Q)
    oct_ = resonate(pulses, BODY_HZ * OCTAVE_HZ_RATIO, OCTAVE_Q)

    # The midrange snarl, whose centre rises with the revs.
    snarl_hz = SNARL_HZ_IDLE + (SNARL_HZ_REDLINE - SNARL_HZ_IDLE) * throttle
    snarl = resonate(pulses, snarl_hz, SNARL_Q)

    dry = pulses / max(np.abs(pulses).max(), 1e-9)
    wet = body / max(np.abs(body).max(), 1e-9)
    hi = oct_ / max(np.abs(oct_).max(), 1e-9)
    mid = snarl / max(np.abs(snarl).max(), 1e-9)

    voiced = (1.0 - OCTAVE_MIX) * wet + OCTAVE_MIX * hi
    voiced = (1.0 - SNARL_MIX) * voiced + SNARL_MIX * mid
    mix = (1.0 - BODY_MIX) * dry + BODY_MIX * voiced
    mix += intake(throttle, seconds)

    # The resonator leaves a small DC drift. Harmless to the ear but it
    # eats headroom and shows up in every spectrum, so remove it here
    # rather than explain it every time we measure.
    mix -= mix.mean()

    # Fade the ends so an audition does not start and stop with a click.
    # A real-time port would loop instead and never need this.
    edge = int(0.01 * SAMPLE_RATE)
    ramp = np.linspace(0.0, 1.0, edge)
    mix[:edge] *= ramp
    mix[-edge:] *= ramp[::-1]

    return mix / max(np.abs(mix).max(), 1e-9) * PEAK


def write_wav(path: str, sig: np.ndarray) -> None:
    data = np.clip(sig, -1.0, 1.0)
    pcm = (data * 32767.0).astype(np.int16)
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SAMPLE_RATE)
        w.writeframes(pcm.tobytes())


def sweep(seconds: float = 6.0) -> np.ndarray:
    """Idle to redline and back — the one that tells you if it is alive.

    Built by rendering short slices at rising throttle and butting them
    together. Crude next to a real-time port (the pulse phase resets at
    each slice) but it answers the only question a sweep is for: does the
    CHARACTER change with the revs, or is it just a pitch-shifted sample?
    """
    slices = []
    steps = 60
    for i in range(steps):
        t = i / (steps - 1)
        ramp = t if t < 0.5 else (1.0 - t)
        slices.append(render(min(ramp * 2.0, 1.0), seconds / steps))
    return np.concatenate(slices)


def rev(low: float = 0.15, high: float = 1.0, up: float = 1.1,
        hold: float = 0.5, down: float = 1.4) -> np.ndarray:
    """One rev: off idle, up to the top, hold, and back down.

    THE audition. A steady note tells you whether one rev sounds right;
    a rev tells you whether the engine is alive — whether the timbre
    opens up as it climbs, or whether it is just one sound being played
    faster. That opening-up is the measured signature of the reference
    (1-4 kHz energy nearly doubles, harmonic 3 goes four times stronger)
    and it is the whole reason this is synthesized rather than sampled.

    Crossfaded rather than butt-joined: the pulse phase resets at every
    slice boundary, and without the fade each join is an audible click
    that a real-time port would never produce.
    """
    step = 0.05
    fade = int(step * 0.5 * SAMPLE_RATE)
    pieces = []
    for phase, dur in (("up", up), ("hold", hold), ("down", down)):
        n = max(int(dur / step), 1)
        for i in range(n):
            f = i / max(n - 1, 1)
            if phase == "up":
                t = low + (high - low) * (f ** 0.75)
            elif phase == "hold":
                t = high
            else:
                t = high + (low - high) * (f ** 0.55)
            pieces.append(render(t, step + step * 0.5))

    out = pieces[0]
    for nxt in pieces[1:]:
        head, tail = out[:-fade], out[-fade:]
        ramp = np.linspace(0.0, 1.0, fade)
        joined = tail * (1 - ramp) + nxt[:fade] * ramp
        out = np.concatenate([head, joined, nxt[fade:]])
    return out / max(np.abs(out).max(), 1e-9) * PEAK


def main() -> None:
    here = os.path.dirname(os.path.abspath(__file__))
    out = os.path.join(here, "out")
    os.makedirs(out, exist_ok=True)

    # The four audition points. Named by throttle percentage so you can
    # say "the 70 one is too harsh" and we both know what that means.
    for pct in (0, 30, 70, 100):
        sig = render(pct / 100.0)
        path = os.path.join(out, f"engine_{pct:02d}.wav")
        write_wav(path, sig)
        print(f"{path}  throttle {pct:3d}%  "
              f"{rpm_at(pct / 100.0):6.0f} rpm  "
              f"{firing_hz(pct / 100.0):6.1f} Hz firing")

    path = os.path.join(out, "engine_sweep.wav")
    write_wav(path, sweep())
    print(f"{path}  idle → redline → idle")

    path = os.path.join(out, "engine_rev.wav")
    write_wav(path, rev())
    print(f"{path}  ★ ONE REV — the audition that matters")

    path = os.path.join(out, "engine_rev_double.wav")
    write_wav(path, np.concatenate([rev(), np.zeros(int(0.25 * SAMPLE_RATE)), rev()]))
    print(f"{path}  two revs, as the reference does it")


if __name__ == "__main__":
    main()
