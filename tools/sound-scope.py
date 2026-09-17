#!/usr/bin/env python3
"""Measure what a real arcade sound is actually made of.

★ WHY THIS EXISTS. The Defender laser was being rebuilt from my guesses
at 1981 hardware, because I cannot hear. Brian has gameplay footage of
the real machine. This closes that gap: he points it at a moment where a
laser fires, and it reports — in numbers — what the sound is doing. That
turns "I think the ROM used variable-duty squares" into a measurement.

⚠️ THIS DOES NOT LET ME HEAR ANYTHING. It converts audio into numbers I
can read. Brian still owns every judgement about whether something
sounds right; this only makes my starting point an informed one.

USAGE

  Find the candidate sounds in a clip:
    ./sound-scope.py find <url-or-file> --from 30 --to 90

  Measure one of them:
    ./sound-scope.py scope <url-or-file> --at 47.3 --len 0.2

  Compare the real thing against one of our candidates:
    ./sound-scope.py scope laser-1-noise-swept.wav --at 0 --len 0.16

⚠️ ARCADE FOOTAGE IS A RECORDING OF A ROOM, NOT A DIRECT FEED. Expect
YouTube compression, cabinet speaker colouration, mic placement and
possibly other sounds layered on top. ⇒ TRUST THE BIG SHAPES — sweep
direction, tonal vs noisy, rough frequency range — and DO NOT trust
precise amplitudes or anything above ~10 kHz. A conclusion that only
holds if the recording is clean is not a conclusion.
"""

import argparse
import json
import math
import os
import subprocess
import sys
import tempfile
import wave

import numpy as np

SR = 48000


# ---------------------------------------------------------------------
# Getting audio in
# ---------------------------------------------------------------------

def fetch_audio(source, start=None, duration=None):
    """Return mono float32 samples at SR from a URL or a local file.

    ⚠️ DOWNLOADS ONLY THE SECTION ASKED FOR when given a time range and a
    URL — a full gameplay video is hundreds of megabytes and we want two
    seconds of it.
    """
    tmp = tempfile.mkdtemp(prefix="sound-scope-")
    wav = os.path.join(tmp, "audio.wav")

    if os.path.exists(source):
        src = source
    else:
        # yt-dlp writes the bestaudio stream; ffmpeg does the slicing
        # afterwards because seeking a partial download is unreliable.
        src = os.path.join(tmp, "dl.m4a")
        cmd = ["yt-dlp", "-q", "--no-warnings", "-f", "bestaudio", "-o", src, source]
        if start is not None and duration is not None:
            # Only fetch the section we need, with a little padding.
            lo = max(0.0, start - 1.0)
            hi = start + duration + 1.0
            cmd += ["--download-sections", f"*{lo}-{hi}", "--force-keyframes-at-cuts"]
        run(cmd, "yt-dlp failed — is the URL right, and is the video public?")
        # yt-dlp may pick its own extension.
        if not os.path.exists(src):
            cands = [f for f in os.listdir(tmp) if f.startswith("dl.")]
            if not cands:
                die("yt-dlp produced no file")
            src = os.path.join(tmp, cands[0])

    ff = ["ffmpeg", "-v", "error", "-y"]
    # ⚠️ -ss BEFORE -i seeks fast but lands on a keyframe; for a 200ms
    # window that imprecision is the whole measurement. Seek AFTER -i so
    # ffmpeg decodes to the exact sample.
    ff += ["-i", src]
    if start is not None:
        ff += ["-ss", str(start)]
    if duration is not None:
        ff += ["-t", str(duration)]
    ff += ["-ac", "1", "-ar", str(SR), "-c:a", "pcm_s16le", wav]
    run(ff, "ffmpeg failed to decode the audio")

    with wave.open(wav, "rb") as w:
        n = w.getnframes()
        raw = w.readframes(n)
    x = np.frombuffer(raw, dtype="<i2").astype(np.float32) / 32768.0
    return x


def run(cmd, msg):
    p = subprocess.run(cmd, capture_output=True, text=True)
    if p.returncode != 0:
        die(f"{msg}\n\n$ {' '.join(cmd)}\n{p.stderr.strip()[:2000]}")


def die(msg):
    print(f"error: {msg}", file=sys.stderr)
    sys.exit(1)


# ---------------------------------------------------------------------
# Finding candidate sounds
# ---------------------------------------------------------------------

def find(x, top=25):
    """List the sharpest onsets — the moments most likely to be a shot.

    ★ A LASER IS AN ONSET, NOT A LOUD PATCH. Gameplay audio has music and
    a constant drone; what distinguishes a zap is a fast RISE. So this
    ranks by increase in energy rather than by energy, which is what
    keeps it from returning twenty hits inside one explosion.
    """
    hop = 256
    win = 1024
    frames = (len(x) - win) // hop
    if frames < 2:
        die("clip is too short to search")

    # Spectral flux: how much energy APPEARED since the previous frame.
    # Only rises count — a sound ending is not an onset.
    window = np.hanning(win)
    prev = None
    flux = np.zeros(frames)
    for i in range(frames):
        seg = x[i * hop:i * hop + win] * window
        mag = np.abs(np.fft.rfft(seg))
        if prev is not None:
            flux[i] = np.sum(np.maximum(mag - prev, 0.0))
        prev = mag

    # Local peaks only, and spaced apart, so one event reports once.
    order = np.argsort(flux)[::-1]
    picked = []
    min_gap = int(0.08 * SR / hop)
    for i in order:
        if flux[i] <= 0:
            break
        if all(abs(i - j) >= min_gap for j in picked):
            picked.append(i)
        if len(picked) >= top:
            break

    return sorted((i * hop / SR, flux[i]) for i in picked)


# ---------------------------------------------------------------------
# Measuring one sound
# ---------------------------------------------------------------------

def scope(x, label=""):
    """Describe a short sound in numbers a synthesis decision can use."""
    n = len(x)
    if n < 512:
        die("slice too short to analyse")

    out = {"label": label, "seconds": round(n / SR, 4)}

    # ── Envelope ────────────────────────────────────────────────────
    # Where the energy is in time: a zap is front-loaded, a drone is not.
    env = np.abs(x)
    k = max(1, int(0.002 * SR))
    env = np.convolve(env, np.ones(k) / k, mode="same")
    peak_at = int(np.argmax(env)) / SR
    out["peak"] = round(float(np.max(np.abs(x))), 4)
    out["peak_at_s"] = round(peak_at, 4)

    # Decay: how long from the peak to a tenth of it.
    p = int(peak_at * SR)
    thresh = env[p] * 0.1 if env[p] > 0 else 0
    tail = np.where(env[p:] < thresh)[0]
    out["decay_to_10pct_s"] = round(float(tail[0] / SR), 4) if len(tail) else None

    # ── Sweep: dominant frequency over time ─────────────────────────
    # ★ THE HEADLINE MEASUREMENT. Defender's laser is claimed to sweep;
    # this is what proves it, in which direction, and how far.
    win = 512
    hop = 128
    track = []
    # ⚠️ GATE ON ENERGY. The tail of a one-shot is near-silence, and
    # silence measures as perfectly flat noise and as an arbitrary
    # centroid. Measuring it drags every summary statistic toward
    # nonsense — the first version of this tool reported our own pitched
    # oscillator as NOISE for exactly this reason. Only windows carrying
    # real signal get a vote.
    gate = float(np.max(np.abs(x))) * 0.08
    for i in range(0, n - win, hop):
        seg = x[i:i + win] * np.hanning(win)
        if np.max(np.abs(seg)) < gate:
            continue
        mag = np.abs(np.fft.rfft(seg))
        if mag.sum() <= 0:
            continue
        freqs = np.fft.rfftfreq(win, 1 / SR)
        # Spectral CENTROID rather than the single loudest bin: a noisy
        # sound has no single peak, and the loudest bin of noise jumps
        # around meaninglessly. The centroid is where the energy sits on
        # average, which is defined for both tonal and noisy signals.
        centroid = float((freqs * mag).sum() / mag.sum())
        # ⚠️ SKIP DC AND THE FIRST FEW BINS. A narrow-duty square has a
        # large DC offset, so bin 0 is its loudest bin and the "pitch"
        # reads as 0 Hz — which is what the duty-cycle candidate did
        # before this line. The fundamental of anything we care about is
        # well above 90 Hz.
        lo_bin = max(1, int(90.0 * win / SR))
        peak_hz = float(freqs[lo_bin + int(np.argmax(mag[lo_bin:]))])
        track.append((round(i / SR, 4), round(centroid), round(peak_hz)))

    out["track"] = track
    if track:
        early = track[:max(1, len(track) // 5)]
        late = track[-max(1, len(track) // 5):]
        # ★ THE SWEEP IS MEASURED ON THE PEAK BIN, NOT THE CENTROID.
        # The centroid is pulled upward by any harmonic content — a
        # square sweeping 1850→180 Hz has a centroid near 5 kHz because
        # its harmonics run to Nyquist, which says nothing about the
        # pitch. The peak bin follows the fundamental, which is the
        # thing that is actually sweeping. The centroid is reported too,
        # as BRIGHTNESS, because that is a real and separate question.
        ep = sum(t[2] for t in early) / len(early)
        lp = sum(t[2] for t in late) / len(late)
        ec = sum(t[1] for t in early) / len(early)
        lc = sum(t[1] for t in late) / len(late)
        out["peak_start_hz"] = round(ep)
        out["peak_end_hz"] = round(lp)
        out["centroid_start_hz"] = round(ec)
        out["centroid_end_hz"] = round(lc)
        out["sweep"] = ("DOWN" if lp < ep * 0.8 else
                        "UP" if lp > ep * 1.25 else "flat")
        out["sweep_ratio"] = round(lp / ep, 3) if ep else None
        # ⚠️ IF THESE TWO DISAGREE THE SOUND IS NOT A CLEAN TONE. A pure
        # sine puts its centroid on its fundamental; a ratio far above 1
        # means most of the energy is elsewhere — harmonics, or noise.
        out["centroid_over_peak"] = round(ec / ep, 2) if ep else None

    # ── Tonal or noisy ──────────────────────────────────────────────
    # ★ THE QUESTION BRIAN'S VERDICT TURNS ON. "More white noise" is a
    # claim about this and nothing else.
    #
    # ⚠️ MEASURED ON SHORT WINDOWS, DELIBERATELY. A swept tone smears
    # across bins over a long window and measures as noise — an earlier
    # attempt at this made exactly that mistake and reported our own
    # pitched sweep as noise. Over 512 samples (~11ms) the sweep barely
    # moves, so the window sees a nearly-steady tone if there is one.
    flats = []
    for i in range(0, n - win, hop):
        seg = x[i:i + win] * np.hanning(win)
        # Same energy gate as the track, and for the same reason:
        # silence is perfectly flat and would read as pure noise.
        if np.max(np.abs(seg)) < gate:
            continue
        mag = np.abs(np.fft.rfft(seg))[1:]
        if mag.sum() <= 0 or np.max(mag) <= 0:
            continue
        mag = mag + 1e-10
        gm = math.exp(float(np.mean(np.log(mag))))
        am = float(np.mean(mag))
        flats.append(gm / am)
    if flats:
        f = float(np.median(flats))
        out["flatness"] = round(f, 4)
        # ⚠️ THRESHOLDS CALIBRATED AGAINST SIGNALS OF KNOWN CHARACTER,
        # NOT PICKED FROM A TEXTBOOK. Measured on our own candidates,
        # where the answer is known because I wrote them:
        #   pitched sine+square sweep  0.35
        #   swept-bandpass noise       0.47
        #   variable-duty square       0.55
        # ★ A TEXTBOOK "NOISE IS ABOVE 0.3" WOULD CALL ALL THREE NOISE
        # AND BE USELESS HERE. The absolute value is inflated for every
        # one of them because these are SWEEPS — the fundamental moves
        # during even a short window, smearing the spectrum — and
        # because 512 samples is only 94 Hz of resolution. What the
        # number can do is RANK: higher is noisier, and the gaps above
        # are real. What it cannot do is answer "is this noise" on its
        # own, so it is reported as a comparison, not a verdict.
        out["flatness_note"] = "compare against a reference, do not read absolutely"
        out["character"] = ("noisier than a swept square" if f > 0.55 else
                            "noise-like" if f > 0.42 else
                            "mixed / harmonic-rich" if f > 0.25 else
                            "tonal")

    # ── Harmonic structure ──────────────────────────────────────────
    # A square has odd harmonics; a saw has all of them; noise has none.
    # Measured at the loudest moment, where the structure is clearest.
    seg = x[max(0, p - win // 2):max(0, p - win // 2) + 2048]
    if len(seg) >= 2048:
        mag = np.abs(np.fft.rfft(seg * np.hanning(2048)))
        freqs = np.fft.rfftfreq(2048, 1 / SR)
        f0_bin = int(np.argmax(mag[1:])) + 1
        f0 = float(freqs[f0_bin])
        if f0 > 20:
            out["f0_hz"] = round(f0)
            hs = []
            for h in range(1, 9):
                target = f0 * h
                if target >= SR / 2:
                    break
                b = int(round(target * 2048 / SR))
                lo, hi = max(0, b - 2), min(len(mag), b + 3)
                hs.append(round(float(np.max(mag[lo:hi]) / np.max(mag)), 3))
            out["harmonics_rel"] = hs
            odd = sum(hs[0::2])
            even = sum(hs[1::2])
            out["odd_vs_even"] = round(odd / even, 2) if even > 0 else None

    # ── Where the energy sits ───────────────────────────────────────
    mag = np.abs(np.fft.rfft(x * np.hanning(n)))
    freqs = np.fft.rfftfreq(n, 1 / SR)
    total = mag.sum()
    if total > 0:
        bands = [(0, 200), (200, 800), (800, 2000), (2000, 5000),
                 (5000, 10000), (10000, 20000)]
        out["bands_pct"] = {
            f"{a}-{b}": round(float(mag[(freqs >= a) & (freqs < b)].sum() / total * 100), 1)
            for a, b in bands
        }

    return out


def report(d):
    """Print a measurement in a form a synthesis decision can use."""
    print(f"\n=== {d.get('label','')} ({d['seconds']}s) ===")
    print(f"peak {d['peak']} at {d['peak_at_s']}s"
          + (f", down to 10% after {d['decay_to_10pct_s']}s" if d.get('decay_to_10pct_s') else ""))
    if "sweep" in d:
        print(f"SWEEP: {d['sweep']}  {d['peak_start_hz']} Hz -> {d['peak_end_hz']} Hz"
              f"  (ratio {d['sweep_ratio']})   [peak bin = the fundamental]")
        print(f"BRIGHTNESS: centroid {d['centroid_start_hz']} -> {d['centroid_end_hz']} Hz"
              f"  (centroid/peak {d.get('centroid_over_peak')}; "
              f"~1 = pure tone, >3 = lots of harmonics or noise)")
    if "character" in d:
        print(f"CHARACTER: {d['character']}  (flatness {d['flatness']})")
        print("  reference points, measured on our own candidates:")
        print("    0.35 = pitched sine+square sweep   0.47 = swept-noise")
        print("    0.55 = variable-duty square")
        print("  ⚠️ RANK these, do not read the absolute value — a sweep")
        print("     smears its own spectrum and inflates all of them.")
    if "f0_hz" in d:
        print(f"f0 {d['f0_hz']} Hz, harmonics {d['harmonics_rel']}")
        if d.get("odd_vs_even"):
            hint = ("square-like (odd-dominant)" if d["odd_vs_even"] > 1.6
                    else "saw-like (odd and even)" if d["odd_vs_even"] > 0.7
                    else "even-dominant")
            print(f"odd/even {d['odd_vs_even']} — {hint}")
    if "bands_pct" in d:
        print("energy by band (%):")
        for k, v in d["bands_pct"].items():
            bar = "#" * int(v / 2)
            print(f"  {k:>12} Hz {v:5.1f} {bar}")
    if d.get("track"):
        print("over time (s, centroid Hz, PEAK-BIN Hz <- the pitch):")
        step = max(1, len(d["track"]) // 12)
        for row in d["track"][::step]:
            print(f"  {row[0]:.3f}  {row[1]:6d}  {row[2]:6d}")


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)

    f = sub.add_parser("find", help="list the sharpest onsets in a clip")
    f.add_argument("source")
    f.add_argument("--from", dest="start", type=float, default=0.0)
    f.add_argument("--to", dest="end", type=float, default=60.0)
    f.add_argument("--top", type=int, default=25)

    s = sub.add_parser("scope", help="measure one sound")
    s.add_argument("source")
    s.add_argument("--at", type=float, default=0.0)
    s.add_argument("--len", dest="length", type=float, default=0.2)
    s.add_argument("--json", action="store_true")

    a = ap.parse_args()

    if a.cmd == "find":
        dur = a.end - a.start
        if dur <= 0:
            die("--to must be after --from")
        x = fetch_audio(a.source, a.start, dur)
        print(f"searching {dur:.1f}s from {a.start:.1f}s\n")
        print("  time in clip   absolute   strength")
        for t, v in find(x, a.top):
            print(f"  {t:8.3f}s   {a.start + t:8.3f}s   {v:9.1f}")
        print("\n⇒ measure one with:")
        print(f"   ./sound-scope.py scope {a.source} --at <absolute> --len 0.2")
        print("\n⚠️ These are ONSETS, not identified sounds. A laser, an")
        print("   explosion and a UI blip all look like onsets — listen to")
        print("   the timestamps and pick the ones that are actually shots.")

    else:
        x = fetch_audio(a.source, a.at, a.length)
        d = scope(x, f"{a.source} @ {a.at}s")
        if a.json:
            print(json.dumps(d, indent=2))
        else:
            report(d)


if __name__ == "__main__":
    main()
