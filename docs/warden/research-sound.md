# Warden — sound reference (Williams sound board, Defender 1981)

> Research for Warden, 2026-10-05. Numbers measured by running Sam Dicker's sound ROM (assembled from the released source, md5-checked) in an MC6800 emulator. Port the TECHNIQUES with our own parameters. The emulator, the source and 25 reference renders for A/B listening live OUTSIDE this repo in ~/projects/omarcade-reference/ (never commit them).


**How I got these numbers.** I didn't rely on blog posts. I took Sam Dicker's original sound ROM source (`vsndrm1.src`, "DEFENDER SOUNDS REV. 1.0 BY SAM D 10/80") from the mwenge/defender repo and assembled it. The output is byte-identical to the arcade `defend.snd`: md5 `ec5b36f8…` matches the repo's check. I then ran it in a small MC6800 emulator I wrote, which logs every DAC write with its cycle time. Durations, sample rates and pitch tracks below come from that run unless marked "analytic". Where I also worked a number out by hand, the two agree within about 5%: bomber hit 1.22 s both ways, pod hit 5.26 vs 5.27 s, laser 9.79 vs about 9.74 s.

I mapped game events to sound codes from the game's own sound table (`defa7.src` lines 660–691, `SNDLD` call sites). The mapping is confirmed three ways: code $13 "KILSND" lands on BGEND, $16 lands on THRUST, and $1E lands on the VARI vector the source names **FOSHIT** (free ship).

---

## 1. How the Williams sound board works

- **CPU:** a Motorola 6808/6802 on a 3.579545 MHz crystal, divided by 4 internally, so it runs at **894,886 cycles/s**. It has 128 bytes of RAM ($00–$7F) and a 2 KB ROM at $F800 (MAME `williams.cpp`: `SOUND_CLOCK = XTAL(3'579'545)`, "effective frequency is 894.886kHz").
- **DAC:** an **MC1408 8-bit DAC** on PIA port A ($0400). There is no sample clock. A sample exists only when the code executes `STAA $0400`, so **the sample rate is whatever the loop's cycle count works out to**: about 28 kHz in tight loops, a few hundred Hz in slow ones. The DAC holds its last value between writes. Stair-steps, aliasing and pitch-dependent sample rate are core to the sound. MAME models no output filter. Output is unipolar (0–255), and the real amplifier is AC-coupled.
- **Commands:** the game CPU writes a 5-bit code through a 6821 PIA, which raises an IRQ. The IRQ handler **resets the stack, which kills whatever sound is playing**, then dispatches. Codes $01–$0D go to GWAVE presets, $0E–$1C to special routines, $1D–$20 to VARI presets. **The board is strictly monophonic: every new sound cuts off the old one.** The game decides which sound wins with a priority byte, and each sound has a "hold" timer in 16 ms frames. When the hold expires and you're holding thrust, the game re-sends THRUST, which **truncates the previous sound's tail**. That cut-off behaviour is a big part of how Defender feels.
- **Random source** (shared by every noise routine): a 16-bit shift register in HI:LO, shifted right. The new top bit is bit 0 XOR bit 3 of LO, and the output bit is the old bit 0. The seed is HI=$3C, LO=0.

### The synthesis routines (from the source)

| Routine | What it does |
|---|---|
| **GWAVE** | Wavetable oscillator. It copies one of 7 ROM waves to RAM and plays it at a period taken from a **frequency pattern table**: each entry is a period P, played for GCCNT cycles. Per-sample cost is **26 + 6P cycles** (P=0 counts as 256), so sample rate = 894886/(26+6P) and tone = 894886/(N·(26+6P)+49) for an N-sample wave. **Echo**: replay the whole pattern GECHO times, subtracting `GECDEC × (rom_sample>>4)` from the RAM table each time. That subtraction **wraps modulo 256** (it isn't clamped), so heavy decay turns into harsh distortion. **Pre-decay** is the same operation applied once at the start. **Frequency modulation**: after the echoes, add GDFINC to every period and replay, dropping entries that overflow, up to GDCNT times. |
| **VARI** | Variable-duty square wave. Low half = LO counts, high half = HI counts, **14 cycles per count**, so f = 894886/(14·(LO+HI)). Every SWPDT counts it adds HIDT to HI until HI equals HIEN, then adds LOMOD to LO and repeats until LO wraps to 0. |
| **NOISE** (used by TURBO) | Each random bit outputs 0 or the current amplitude. Every CYCNT samples the amplitude drops by DECAY and the hold delay grows by 1. |
| **LITE / APPEAR** | 1-bit full-scale noise: the DAC is inverted on each random 1 bit. The delay L steps by DFREQ every CYCNT samples. Sample period = 42 + 6L cycles. |
| **FNOISE** (THRUST, BG1, CANNON) | Slew-limited random walk. Pick a random 8-bit target and ramp the output toward it at a slope of FHI.FLO DAC units per sample (about 28.4 kHz). When it gets there, pick a new target. The "distortion" flag ANDs the slope with a random byte. The decay flag scales the slope by ×7/8 every SAMPC samples. |
| **SCREAM** | Four 8-bit phase accumulators producing square waves, summed with amplitudes 128/64/32/16. Output is about 4.6 kHz (218 µs per sample). |
| **HYPER** | Pulse-width sweep at a fixed 56.8 Hz. |
| **RADIO** | 16-step table oscillator with a rising frequency. Not used by Defender: the lander-grab entry `LGSND` is defined but never referenced. |
| **ORGAN** | Tunes and notes. Not used by Defender. |

---

## 2. Each Defender sound (code, routine, measured behaviour)

| Event | Code to routine | What it actually is (measured) |
|---|---|---|
| **Player laser** | $14 TURBO, hold 768 ms, pri $C0 | **Noise, not a tone.** Random bits × amplitude. Amplitude starts at 255 and drops 1 every 32 samples. The per-sample hold grows by 8 cycles every 32 samples, so the noise clock falls from **~15.8 kHz to ~430 Hz** while the level fades linearly. It runs **9.79 s** if nothing interrupts. Analytically, at 0.1 s the clock is ~4.4 kHz at 92% level; at 0.25 s ~2.6 kHz at 86%; at the 768 ms hold end ~1.5 kHz at 74%. So the "pshew" is mostly the falling noise clock, with little loudness change. Each shot restarts it, and holding thrust cuts it at 768 ms. |
| **Thrust on** | $16 THRUST, loops forever | FNOISE, slope 3, no decay. A rumble at about 28.3k samples/s (35.3 µs per sample), RMS about 55/128, energy mostly below ~500 Hz. |
| **Thrust release** | $0F BG1, loops forever | Same routine with slope 1, so darker and lower (about 47/128 RMS). The code says it doesn't go silent. Confirm by ear against MAME before copying this. |
| **Enemy explosions** | Lander $06 GWAVE "HBEV"; bomber $01 "HBDV"; baiter and swarmer $07 "PROTV"; pod $05 "BBSV"; mutant $17 CANNON | Each enemy type has its own sound (details in section 3). |
| **CANNON** (big explosion) | $17 | FNOISE with distortion. Starts at slope 255 and falls ×7/8 every 1000 samples (~35 ms, so τ ≈ 260 ms). Sample interval goes from 39 to 34 µs. Ends at **2.58 s**. |
| **Player death** | $11 LITE ×2 (128 ms each), then $17 CANNON | Two retriggered crackle bursts, then the full 2.6 s explosion tail. About 2.8 s total. |
| **Smart bomb** | LITE retriggered 6× every 64 ms, then CANNON | A stuttering 384 ms of crackle, then the explosion tail. |
| **Planet explodes** | TURBO 64 ms, LITE 2×96 ms, CANNON 2×160 ms, then tail | Laser noise, then crackle, then the explosion restarted once, then the tail. |
| **Humanoid killed** | $11 LITE | 1-bit noise. Clock sweeps 18.6 kHz down to 573 Hz over **0.70 s** (L = 1→255, 3 samples per step). |
| **Lander grabs humanoid** | $0B GWAVE "ED10" | Same as the 2-player start sound (see below). |
| **Lander carrying, mutant conversion** | $0E SP1 retriggered 10× at 16 ms | VARI. Each retrigger lowers LO by 14 (254, 240, 226, …), so the buzz rises in steps from **~250 Hz**. HI is a sawtooth (+24 mod 256 every 18 ms, 433 ms cycle). The last setting **drones until another sound replaces it**. |
| **Humanoid falling / scream** | $1A SCREAM | Voice 1 starts at ~1150 Hz and its frequency step drops by 1 every 256 samples (55.6 ms). It reaches 0 after **3.56 s**. When a voice passes step 55, the next voice starts at step 65, so the voices are staggered by about 0.5 s and get quieter by half each. Total **5.29 s**. Measured pitch: 1168, 927, 766, 569, 379, 175 Hz… then the lower voices fade out. |
| **Humanoid caught** | $08 "SPNRV" ×3 at 160 ms | 8-sample sine starting at 269 Hz. The period drops by 3 every 5 cycles, so it would reach ~3.5 kHz at 228 ms. The 160 ms retrigger cuts each one at **~474 Hz**: three rising "bloops". |
| **Humanoid set down** | $1F VARI QUASAR | Rising sweeps **378 → 1530 Hz** of about 260 ms each, 10 of them. LO steps 40, 36, … 4, so each sweep is a little higher. Total **2.64 s**. |
| **Lander, baiter shot** | $03 GWAVE "DP1V" | Wrapped 72-sample sine. Fundamental falls **166 → 79 Hz** over 15 single cycles, **168 ms**. A spitty "bzew". |
| **Mutant shot** | $09 "CLDWNV" | 3-note figure (873 / 1396 / 2934 Hz) on an odd 8-sample wave, 1 cycle each, 3 echoes (about 7 ms), then all periods +1 and repeat. A descending buzzy trill: measured 4.2 kHz → 1.0 kHz at 1.2 s → 600 Hz at 3.6 s. It would run about 17 s if never interrupted. |
| **Swarmer shot** | $0C "ED12" | 8-sample sine at only **~5 kHz sample rate** (heavily stair-stepped). Six falling steps 658 → 559 Hz, 10 cycles each (99.6 ms), 6 echoes, **0.60 s**. |
| **Enemies materialize** (wave start) | $15 APPEAR | 1-bit noise, clock *rising* **760 Hz → 16.9 kHz** (L = 192→0 in steps of −2, 16 samples per step), **1.07 s**. |
| **1-player start** | $0A "SV3" | 166 Hz, 72-sample sine, 256 passes. After each pass the table is eroded by `rom>>4` (wrapping), so the timbre cycles about every 16 passes. Each recompute leaves **DAC gaps of up to 10.6 ms**, which chop the tone. **3.14 s**. |
| **2-player start** | $0B "ED10" | Same sound as "lander grabs humanoid". 6-note pattern 787 / 726 / 673 / 628 / 553 / 726 Hz on a harmonic-rich 16-sample wave, 6 cycles each, 15 echoes at 5/16 decay (wraps after about 3 echoes). **0.83 s**. |
| **Coin** | $19 HYPER | **56.8 Hz pulse** whose duty sweeps 0 → 100% over 128 periods (**2.26 s**). Timbre goes thin → hollow (loudest at 50%, ~1.2 s) → thin. |
| **Extra life** | $1E VARI FOSHIT | Falling square sweeps **~1530 → 378 Hz in 128 ms** (HI 1→129 in steps of +8 every 8 ms, LO=40). Then LO−1 and repeat, 40 times, each sweep higher and faster. **5.32 s**. |
| **Wave complete** | $13 BGEND | **Silence.** The game kills sound before the bonus tally. |
| **Hyperspace** | none | The `HYPER` routine in `defa7.src` calls no sound. You get only what's already playing or the thrust state. |

---

## 3. Recipes for a Rust synthesizer

**Engine design, which is where the authenticity comes from.** Run each voice on a **virtual 894,886 Hz cycle clock**. Each routine is a state machine that emits `(dac_value: u8, hold_cycles)`. Render with zero-order hold, box-filtered (cycle-integrated) down to 48 kHz, so the hold-and-step aliasing survives. Map the output as `(v − 128)/128`, then apply a DC-blocking high-pass around 20 Hz to imitate the AC-coupled amplifier. Keep the game-side rules too:

- one voice
- priority byte, and an equal priority interrupts
- hold timers in 16 ms frames
- thrust re-asserts when the hold expires

Do all of this without ROM data. The tables are generic shapes (sine, square, a two-harmonic wave), so define your own.

1. **Laser.** 16-bit shift-register noise. Per sample: `out = bit ? amp : 0`; `period_cycles = 48.5 + 8k`. Every 32 samples: `k += 1`, `amp -= 1` (starting at k=1, amp=255). Hold 768 ms and retrigger on each shot. For a snappier original sound, try a 24-sample step or amplitude decay of 2.
2. **Thrust.** Slew noise. `target = rand_u8()`; each sample moves `out` toward the target by 3 (BG1 idle uses 1); at the target pick a new one. Sample interval 35 µs. Loop while held, fade 5–10 ms on release.
3. **Explosion (CANNON).** Slew noise with `slope = (slope_max & rand_u8())`. Every 1000 samples (~35 ms), `slope_max_fixed16 -= slope_max_fixed16 >> 3`, starting from $FF00. Stop when it reaches 0, about 2.6 s.
4. **Crackle (LITE, humanoid death).** 1-bit full-scale toggle noise. Period = 42 + 6L cycles; L goes 1→255, +1 every 3 samples; 0.70 s.
5. **Materialize (APPEAR).** As LITE, but L goes 192→0 in steps of −2 every 16 samples; 1.07 s, rising.
6. **Bomber hit.** 16-sample square holding 2 cycles per table (tone = 2× the table rate). 22-step period list 1,1,2,2,4,4,8,8,16,32,40…192, giving **~3190 → 94 Hz in 152 ms**. Repeat 8× with level 1, 14/16, 12/16, … (subtract from the peak). 1.22 s.
7. **Lander hit.** 72-sample sine. Periods 1,2,4,8,9,10,11,12,14,15,16,18,20,22 then 64, giving **380 → 78 → 30 Hz** over 142 ms. 4 echoes at 100/75/50/25%. About 0.6 s.
8. **Pod hit.** 16-sample sine alternating **726 Hz × 4 cycles / 135 Hz × 4 cycles**, 10 pairs = 351 ms. 15 echoes, each −1/16. About 5.3 s, though in practice the 256 ms hold and the next sound cut it.
9. **Baiter / swarmer hit.** The "wrap distortion" sound. Build the wave as `w[i] = (sine[i] − 17*(sine[i]>>4)) mod 256`, which gives spiky near-0 / near-255 values. Play periods 1…12 (380 → 126 Hz), 2 echoes, then shorten every period by 1 per round (pitch rises), dropping entries that hit 0. 1.09 s.
10. **Enemy shot (lander/baiter).** The same wrapped wave, one cycle per pass, period +1 each pass for 15 passes: 166 → 79 Hz in 168 ms.
11. **Mutant shot.** Odd 8-sample wave [0,64,128,0,255,0,128,64]. Arpeggio of periods 16/8/1, one cycle each, ×3, then all periods +1. Cut at 768 ms.
12. **Swarmer shot.** 8-sample sine (render at only ~5 kHz for grit). Six steps 658 → 559 Hz, 10 cycles each, ×6 echoes −1/16. 0.6 s.
13. **Catch.** 8-sample sine starting at period 64 (269 Hz); period −3 every 5 cycles. Retrigger 3× at 160 ms intervals.
14. **Set down.** Square with LO=40; HI goes 129→1 in steps of −4 every 8 ms (378 → 1530 Hz); then LO −= 4, 10 sweeps, 2.64 s.
15. **Extra life.** Square with LO=40; HI goes 1→129 in steps of +8 every 8 ms (1530 → 378 Hz); then LO −= 1, 40 sweeps, 5.3 s. A shorter 10–12-sweep version also reads well.
16. **Scream.** 4 square voices from 8-bit phase accumulators at fs ≈ 4.6 kHz, f = step × fs/256. Voice 1 starts at step 64 (~1150 Hz), and every voice's step drops by 1 every 256 samples (55.6 ms). When a voice reaches 55, start the next at 65. Amplitudes 128/64/32/16, summed. 5.3 s.
17. **Coin.** 56.8 Hz pulse, duty 0 → 127/128 over 128 periods, 2.26 s.
18. **Mutant-conversion drone.** Square with LO stepping 254 → 128 by −14 per 16 ms; HI a sawtooth 1+24n mod 256 every 18 ms; hold until replaced.
19. **Start jingles.** A 72-sample sine at 166 Hz with cumulative wrapping erosion and **deliberate 4–10 ms silent gaps** between passes (1P). For 2P, a 6-note harmonic-wave arpeggio, 15 passes, decaying 5/16 with wrap.

**General rules for the character:**
- Sample rate tracks pitch (26+6P cycles per sample), so low notes become audibly stair-stepped.
- Decay wraps instead of clamping.
- Leave the CPU-gap silences in.
- Keep everything one voice that cuts the last one.
- Noise always comes from a shift register, either toggled at full scale or as random-target slews. Never use filtered white noise.

---

### Things to know
- **The laser is noise, not a tonal sweep**, unlike most recreations. It's a fading noise burst whose clock drops; nothing in it is a pitched oscillator.
- **The idle rumble on thrust release is unconfirmed.** It comes straight from the code (BG1 loops forever) and I didn't confirm it by ear. Compare with a MAME recording before relying on it.
- **Copyright.** The source and tables are Williams code. Port the techniques and use your own parameters rather than copying the tables wholesale.

### Sources
- Defender source including `vsndrm1.src` and the sound tables in `defa7.src` / `defb6.src`: https://github.com/mwenge/defender
- Original source dumps: https://github.com/historicalsource/defender and https://github.com/historicalsource/williams-soundroms
- MAME driver (clock, M6808, MC1408 DAC, command path): https://github.com/mamedev/mame/blob/master/src/mame/williams/williams.cpp and `williams_m.cpp`
- Chris Lomont's Williams sound board notes (128 B RAM, 2 KB ROM, DAC at $400, IRQ kills the running sound): https://lomont.org/software/misc/robotron
- Background: https://gigazine.net/gsc_news/en/20210722-williams-defender and https://forums.parallax.com/discussion/98973/defender-sound

I couldn't find a Sam Dicker interview with technical sound-design detail, so the source code is the primary authority here.
