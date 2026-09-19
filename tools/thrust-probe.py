#!/usr/bin/env python3
"""Audition a SUSTAINED noise bed — the thrust engine, before building it.

★ WHY THIS EXISTS. The sound playground cannot model a held sound. Every
sweep in it runs against `u = i/n`, the position in the CLIP, and its
envelope is attack-then-exponential-decay with no sustain stage. Holding
the button there does not hold a sound; it replays a one-shot every
160 ms, and the restart seam is audible as a wave behind the noise.

⚠️ THIS IS A PROBE, NOT A DESIGN. It renders five 3-second beds that each
isolate ONE variable, so the question "which controls does the playground
actually need?" gets answered by ear before a session is spent building
them. None of these is a proposed voice. Brian designs the voice.

  ./thrust-probe.py [outdir]     # default /tmp/thrust

★ 48 kHz MONO to match the game engine (sound.rs SR = 48_000.0), so the
filter behaves identically here and there — no resampling in between.
"""

import math
import os
import struct
import sys
import wave

SR = 48_000.0
SECONDS = 3.0
# ★ Below the laser's measured 0.76 peak ON PURPOSE. A HELD sound fatigues
# far faster than a one-shot, so the bed starts quiet and earns its level.
PEAK = 0.45


def noise(seed):
    """xorshift32 — the same generator shape the playground and sound.rs use."""
    n = seed & 0xFFFFFFFF

    def next_sample():
        nonlocal n
        n ^= (n << 13) & 0xFFFFFFFF
        n ^= n >> 17
        n ^= (n << 5) & 0xFFFFFFFF
        return (n / 0xFFFFFFFF) * 2.0 - 1.0

    return next_sample


def render(corner, q, wobble_depth=0.0, wobble_hz=0.0, seed=0x1234_5678):
    """A state-variable lowpass over white noise, held at a FIXED corner.

    ⚠️ THE CORNER DOES NOT SWEEP. That is the whole point of the probe —
    a sustained engine holds its timbre; a sweep is a one-shot's
    signature. `wobble` is a slow, shallow WANDER, not a sweep.

    ★ `wobble_depth` IS IN OCTAVES and means it. A one-pole smoother fed
    unit-variance noise outputs a std of only sqrt(k/(2-k)) — at 8 Hz
    against 48 kHz that is 0.023, so an un-normalised depth of 0.6 moves
    the corner by about a hundredth of an octave. Inaudible. The first
    version of this probe did exactly that and the scope caught it: the
    wobbled file measured identical to the flat one to three decimals.
    ⇒ DIVIDE BY THAT STD. Then depth is octaves, and it stays octaves
    when the rate changes.
    """
    n = int(SECONDS * SR)
    rnd = noise(seed)
    # A second, independent noise stream for the wobble, so the breathing
    # is not correlated with the grain of the bed itself.
    wob = noise(seed ^ 0xA5A5_A5A5)

    low = 0.0
    band = 0.0
    out = []
    # A one-pole smoother turns white noise into a slow random wander.
    # ★ RANDOM, NOT A SINE. A sine LFO reads as a machine pulsing; an
    # engine wanders and never repeats.
    smooth = 0.0
    if wobble_hz > 0.0:
        k = 1.0 - math.exp(-2.0 * math.pi * wobble_hz / SR)
        # ⚠️ THE 3.0: sqrt(k/(2-k)) is the output std for UNIT-VARIANCE
        # input, and `noise()` is uniform on (-1,1) whose variance is
        # 1/3. Without it every depth means 58% of what it says — the
        # ladder Brian heard was really +/-0.087 octaves at rung 1.
        norm = 1.0 / math.sqrt(k / (2.0 - k) / 3.0)
    else:
        k = 0.0
        norm = 0.0

    damp = min(1.0, 1.0 / q)

    for i in range(n):
        v = rnd()

        if wobble_depth > 0.0 and k > 0.0:
            smooth += k * (wob() - smooth)
            w = smooth * norm
            # Wander the corner MULTIPLICATIVELY — timbre is heard
            # logarithmically, so a fixed number of Hz is a far bigger
            # move down low than up high. `w` is now ~unit variance, so
            # depth reads directly as octaves.
            c = corner * math.pow(2.0, w * wobble_depth)
            # And breathe the level with it, gently. A corner that moves
            # while the level sits perfectly still still reads as static.
            amp = 1.0 + w * wobble_depth * 0.25
        else:
            c = corner
            amp = 1.0

        g = min(1.4, 2.0 * math.sin(math.pi * min(c, SR * 0.45) / SR))
        high = v - low - damp * band
        band += g * high
        low += g * band
        out.append(low * amp)

    # ★ A short fade at each end ONLY so the file does not click on the
    # boundary of the clip. This is not an envelope — the body is flat,
    # because that is what "held" means.
    fade = int(0.01 * SR)
    for i in range(fade):
        wf = i / fade
        out[i] *= wf
        out[n - 1 - i] *= wf

    peak = max(abs(s) for s in out) or 1.0
    scale = PEAK / peak
    return [s * scale for s in out]


def write_wav(path, samples):
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(int(SR))
        w.writeframes(b"".join(
            struct.pack("<h", max(-32768, min(32767, int(s * 32767))))
            for s in samples
        ))


# ★ BRIAN CHOSE THE BED: 250 Hz, Q 0.7. "you kind of nailed on #2 ...
# on og a little wobble in there to make is sound like rushing".
# ⇒ The corner and Q are now FIXED and this ladder varies ONE thing:
# how far the wobble moves, in octaves. He picks the amount by ear.
BED_CORNER = 250.0
BED_Q = 0.7
WOBBLE_HZ = 8.0

PROBES = [
    ("0-flat", "the chosen bed, no wobble — the reference to beat",
     BED_CORNER, BED_Q, 0.0, 0.0),
    ("1-wobble-015", "barely there: +/- 0.15 octaves",
     BED_CORNER, BED_Q, 0.15, WOBBLE_HZ),
    ("2-wobble-030", "a little rush: +/- 0.30 octaves",
     BED_CORNER, BED_Q, 0.30, WOBBLE_HZ),
    ("3-wobble-050", "clearly moving: +/- 0.50 octaves",
     BED_CORNER, BED_Q, 0.50, WOBBLE_HZ),
    ("4-wobble-080", "too much, probably — the far end of the ladder",
     BED_CORNER, BED_Q, 0.80, WOBBLE_HZ),
]


def main():
    outdir = sys.argv[1] if len(sys.argv) > 1 else "/tmp/thrust"
    os.makedirs(outdir, exist_ok=True)

    for name, desc, corner, q, wd, wh in PROBES:
        path = os.path.join(outdir, name + ".wav")
        write_wav(path, render(corner, q, wd, wh))
        print(f"{path}\n    {desc}")

    print(f"\n★ 0 is the bed you approved. Walk UP the ladder until it")
    print(f"  starts rushing, then one more to hear it go too far.")
    print(f"★ The number you land on is the depth, in octaves.")


if __name__ == "__main__":
    main()
