# Warden — gameplay reference (Williams Defender, 1981)

> Research for Warden, 2026-10-05. Numbers read from the released 1981 source (github.com/mwenge/defender). These are MECHANICS to re-implement with our own art, names and parameters — not assets to copy. Where Brian has already decided a value (e.g. Lander = 100 points, Mandrel a7f355dd), his call stands; see docs/warden-plan.md §Open questions.


Most numbers below come from the original 6809 source (Red Label revision, dated 1981) at https://github.com/mwenge/defender. I cloned it and read the code directly. File and label names are given so anyone can re-check a value.

Two caveats apply throughout:
- **Time units.** The game's process scheduler sleeps in units I've treated as 60 Hz video frames, so "1 tick ≈ 16.7 ms". The main game loop wakes every 15 frames (0.25 s).
- **Speed units.** World X is 16-bit with 32 sub-units per pixel. Enemy velocities are in 1/32 px per frame. Y is in scanlines, with 1/256-line fractions.

Where the code and printed guides disagree, I've said so.

---

## 0. Coordinate system and world (`phr6.src`, `defa7.src`)

- **Play field:** Y runs from `YMIN=42` to `YMAX=240`. The top 42 lines hold the scanner and HUD. The visible width is 152 byte-columns × 2 px = **304 px** (the hardware is 320×256).
- **World:** X is 16-bit and wraps (`OX16`), at 32 units per pixel (`SUBD #100*32 ;100 PIXEL LEFT BUFFER`). That makes the world **2048 px around, about 6.7 screen widths**, with seamless wrap.
- **Off-screen objects:** anything more than about 100 px left or 400 px right of the view goes on an "inactive" list. That list is moved 8× velocity every 8 frames, which is cheap off-screen simulation (`ISCAN`/`OSCAN`).
- **Terrain:** a height table `ALTBL` with one entry per 2 px (1024 entries). It is drawn as a **single-pixel-wide mountain line**, which was a hardware limit per Wikipedia.
- **Stars:** up to 16 stars scroll slower than the ship, giving parallax.

## 1. Enemy roster

Sprite sizes come from the picture descriptors in `defb6.src`, given as bytes × lines, with 1 byte = 2 px. Points come from the `KILO`/`KILP` macros, encoded as a BCD mantissa plus an exponent (e.g. `$0115` = 15×10 = 150).

| Enemy (source name) | Size (px) | Points | Kill code |
|---|---|---|---|
| Lander (`LND`) | 10×8 | 150 | `KILP $0115` |
| Mutant ("Schitzo", `SCZ`) | 10×8 | 150 | `KILP $0115` |
| Baiter (`UFO`) | 12×4 | 200 | `KILP $0120` |
| Bomber ("Tie", `TIE`) | 8×8 | 250 | `KILO $0125` |
| Pod ("Probe", `PRB`) | 8×8 | 1000 | `KILO $0210` |
| Swarmer (`SW`) | 6×4 | 150 | `MSWKIL: LDD #$0115` |
| Humanoid (`AST`) | 4×8 | — | — |
| Bomber mine (`BMBP1`) | 4×3 | — | — |
| Player ship | 16×6 | — | — |

**Swarmer points:** several guides say 200 or 500. The arcade code says **150**.

**How enemies appear and die (Sam Dicker's routines, `samexap7.src`):**
- Enemies teleport in with an *appear* effect (`APVCT`): the sprite's pixels converge from a scattered cloud into the shape.
- They die with the reverse (`XSVCT`): the sprite's own pixels fly outward in an expanding grid.
- Each type has its own hit sound (`LHSND`, `SCHSND`, `UFHSND`, `TIHSND`, `PRHSND`, `SWHSND`).

**Lander.** Rough look: a round, dome-topped saucer with dangling legs, commonly rendered green and yellow.
- **Spawn:** appears at the top (`YMIN+2`) at a random world X. It drifts horizontally at a random speed from 1 to `LNDXV` in a random direction, and descends at `LNDYV`.
- **Hunting:** it is assigned a humanoid round-robin (`GTARG`). It hovers about 50 lines above the terrain, oscillating, until its X lines up with its target (within about 8 px).
- **Grab:** it stops, slides 1 px per frame to align exactly, and descends to 12 lines above the humanoid. Then lander and humanoid rise together at `LNDYV`.
- **Mutation:** at `YMIN+8` the humanoid is "pulled inside" (explosion plus sound), and the lander turns into a mutant in place (`SCZ00`).
- **Shooting:** fires every `rand(1..LDSTIM)` × 6 frames (`LSHOT`).
- **Cap:** at most **8 landers alive at once** (`CMPA #8` in `GEX`).

**Mutant.** Rough look: a jittery, blocky, checkered sprite, commonly rendered purple and green.
- **Chase:** constant X seek toward the player at `SZXV`.
- **Laser-line dodge:** while horizontally far away (outside roughly −12 to +56 px), it deliberately **avoids your Y line**. If it is within 8 lines of your altitude it moves away vertically.
- **Charge:** once horizontally close, it seeks your Y directly. This is the classic "hangs above or below you, then charges".
- **Jitter:** a random Y hop of ±`SZRY` lines every 3 frames.
- **Shooting:** fires every `rand(1..SZSTIM)` × 3 frames.
- **Spawn:** created by landers, or directly as reserves (`SCZST`, which keeps at least 300 px away from the player).

**Baiter** (the "hurry-up" enemy). Rough look: a flat, wide, thin saucer; a 3-frame animation cycle gives the iridescent look.
- **Spawn:** near the current view.
- **Chase:** X velocity = **player's X velocity ± 2 px/frame toward you**, unless already within ±20 px. You therefore cannot outrun it.
- **Y:** approaches at half your vertical speed plus a bias.
- **Re-aim:** re-aims only on a random roll (`UFOSK`, "seek probability").
- **Shooting:** fires every `rand(1..UFSTIM)` × 6 frames, which is frequent.
- **Cap:** at most **12** alive (`UFOCNT < 12`).

**Bomber.** Rough look: a small box with **palette-cycling colour** (`TIECOL` rotates colours every 6 frames).
- **Spawn:** in squads of up to 3 (`TIER`), placed relative to the player. Each squad moves at constant X velocity `TIEXV`, alternating direction per squad (`COM TFLG`).
- **Off-screen:** wanders a "cruise altitude" between lines `$40` and `$68`.
- **On-screen:** adjusts Y to sit 16–32 lines above or below you (it stays near you vertically, not on top of you).
- **Mines:** drops a mine on a 1-in-8 roll each frame (`BOMBST`), with at most **10 mines** live. Mines are stationary, cannot be shot, and live for a random 1–32 ticks.
- **Shooting:** bombers do **not** fire aimed shots.

**Pod.** Rough look: star or starburst shaped, commonly purple with red.
- **Movement:** drifts with a random X/Y velocity and does not shoot.
- **On death:** worth 1000 and releases **`RMAX(6)` swarmers, i.e. a random 1–6** biased toward low counts (`PRBKIL`). Total swarmers are capped at **20** (`MMSW`).

**Swarmer.** Rough look: a tiny teardrop, commonly red or orange.
- **Spawn:** bursts from a pod's position with a random velocity.
- **Chase:** X velocity `SWXV` toward you. Y uses a random acceleration masked by `SWAC`, which produces the undulating sine-like motion.
- **Shooting:** fires every `rand(1..SWSTIM)`.
- **Persistence:** reserves persist across lives and are respawned in clumps of up to 6 (`RSW0`).

**Enemy shots** (`SHOOT`, `GETSHL`):
- **Aim:** at the player with a random ±16 px X and ±16 line Y error.
- **Lead:** about **53% of the time** (`SEED > 120`) the shot adds your velocity as a lead.
- **Speed:** proportional to distance, so roughly a fixed flight time.
- **Limits:** at most **20 shells** on screen. Shells only fire from objects that are on or near the screen. Lifetime is 20 ticks.
- **Collision:** a shell or mine that hits you scores 25 (`BKIL: LDD #$25`), which is where printed guides' "25 points" comes from.

## 2. Wave structure (`blk71.src` `WVTAB`, `defa7.src` `GETWV`/`GEXEC`/`WDELT`)

**How the wave table works.** Each parameter is an 8-byte row: `MAX, MIN, INTRA-delta, INTER-delta, W1, W2, W3, W4`.
- Waves 1–4 take the W1–W4 value.
- Every wave also gets the **inter-wave delta applied N times**, where N = (wave − 4, minimum 0) + **initial difficulty (operator adjust GA1, default 5)**, capped at the **difficulty ceiling (GA2, default 15)**. So even wave 1 gets 5 deltas applied, and growth stops around wave 14.
- The **intra-wave delta** is applied to the live parameters every 40 game-loop ticks (**about 10 s**), so a wave gets harder the longer you take.
- MAX and MIN only clamp the deltas.

| Parameter | W1 | W2 | W3 | W4 | Inter Δ | Intra Δ | Effective wave 1 (difficulty 5) |
|---|---|---|---|---|---|---|---|
| Landers | 15 | 20 | 20 | 20 | 0 | 0 | 15 |
| Bombers | 0 | 3 | 4 | 5 | 0 | 0 | 0 |
| Pods | 0 | 1 | 3 | 4 | 0 | 0 | 0 |
| Squad interval (`WAVTIM`, ticks of 0.25 s) | 30 | 25 | 20 | 16 | 0 | 0 | 7.5 s |
| Squad size (`WAVSIZ`) | 5 | 5 | 5 | 5 | — | — | 5 |
| Lander X speed max (`LNDXV`) | $16 | $1E | $26 | $2E | +2 | +3 | $20 = 1 px/frame |
| Lander Y speed (`LNDYV`) | $70 | $B0 | $100 | $100 | 0 | +$10 | 0.44 line/frame |
| Lander shot timer (`LDSTIM`) | $4A | $3A | $2A | $2A | −2 | −4 | 64 (min 16) |
| Bomber X speed (`TIEXV`) | $20 | $28 | $2C | $30 | 0 | 0 | 1 px/frame |
| Mutant X speed (`SZXV`) | $0C | $1C | $24 | $28 | +4 | +8 | $20 |
| Mutant shot timer (`SZSTIM`) | $2A | $22 | $1E | $1C | −2 | −2 | 32 (min 8) |
| Swarmer X speed (`SWXV`) | $16 | $1E | $20 | $22 | +2 | +8 | $20 |
| Swarmer shot timer (`SWSTIM`) | 25 | 25 | 25 | 25 | −1 | −2 | 20 (min 10) |
| **Baiter timer (`UFOTIM`)** | $D4 | $C4 | $A4 | $94 | −4 | −12 | 192 ticks ≈ 48 s (min 24 ≈ 6 s) |
| Baiter shot timer (`UFSTIM`) | 15 | 13 | 12 | 10 | −1 | — | 10 (min 3) |

**Wave composition (my reading of the code).** Wave 1 is **15 landers and nothing else**. Wave 2 is 20 landers, 3 bombers, 1 pod. Wave 3 is 20 / 4 / 3. Wave 4 and later are 20 / 5 / 4. From wave 5 on, the counts stay the same and only speeds and fire rates ramp.

**This conflicts with some player recollections, so check it in MAME before relying on it.** One strategy snippet mentions meeting a bomber followed by two pods at the start of a wave, but it doesn't say which wave.

**Landers arrive in squads.** The first squad comes immediately. Another squad of 5 arrives every `WAVTIM` ticks, or **immediately if no landers are alive**, but only while fewer than 8 are alive. Bombers and pods are all placed at wave start.

**Baiter schedule (`GEXEC`):**
- The first baiter arrives after `UFOTIM` ticks: wave 1 ≈ 48 s, wave 2 ≈ 44 s, wave 3 ≈ 36 s, wave 4 ≈ 32 s, then about 1 s faster per wave down to the floor.
- While enemies remain, `UFOTIM` itself shrinks by 12 ticks (3 s) every 10 s.
- **Panic acceleration:** once **8 or fewer** enemies remain, the countdown is capped at `UFOTIM/2`. Once **3 or fewer** remain, it is capped at `UFOTIM/4`. After a baiter spawns with fewer than 4 enemies left, the next one comes after a random wait of at most `UFOTIM/4`.
- There are never more than 12 baiters.

**End of wave.** A wave ends when landers, bombers, pods, mutants, swarmers and baiters (alive plus reserve) all reach 0 (`WVCHK`). Then:
- Everything is killed (`GNCIDE`).
- The screen shows "**ATTACK WAVE n COMPLETED**", then "**BONUS X** m".
- Each surviving humanoid icon is drawn one at a time, every 4 frames, each adding **100 × min(wave, 5)** points. So the bonus is 100/200/300/400/500 per humanoid, capped at 500 from wave 5 on.
- A 128-frame (about 2 s) hang, then the next wave starts.

**Death mid-wave.** Live enemies are saved to reserve (`PLSAV`) and re-placed when you respawn. Progress is kept but positions are reset.

**Humanoid restore.** In `GETWV` the humanoid count is reset to **10** when the *new* wave number is a multiple of **GA4 (default 5)**. That means waves 5, 10, 15 start with fresh humanoids. Wikipedia-style sources say 6, 11, 16, which is the same rule counted as "after completing 5 waves". Either way, the planet is rebuilt every 5th wave. Humanoids are placed spread across the four quadrants of the world (`ASTST`).

**Planet destruction.** When the last humanoid dies (`ASTCLR`, count reaches 0), `TERBLO` runs:
- The terrain is erased from both the screen and the scanner.
- Then 16 rounds of random full-screen colour flashes, explosions along the old terrain, and a "lightning" sound.
- After that, every new lander squad spawns as **mutants** (`LNDST0: TST ASTCNT / JMP SCZS0`), and any lander that loses its target "freaks" into a mutant (`GTARG` fails, so `SCZ00`).
- The game is pure space combat until the restore wave.

## 3. Humanoids (`defb6.src` `AKIL1`, `AFALL`, `P250`, `P500`)

- **Count and look:** 10 per planet. Tiny 4×8 figures that walk on the ground. Your ship **cannot collide** with them (`OTYP=$10`), and **smart bombs don't kill them** (the bomb only kills `OTYP<2`).
- **If you shoot a lander carrying one** (`LKIL1`), you get 150 for the lander. The humanoid starts falling with a scream sound.
- **Falling:** acceleration is +8/256 lines per frame every 4 frames, with terminal speed $300.
- **Landing on its own:** if impact speed is **≤ $E0** it survives, scoring **250** (with a "250" sprite popping up). If faster, it dies. In practice that means "shot low enough = safe, shot high = splat".
- **Catching it in the air:** you get **500** (`P500`, with a "500" pop-up), and it rides beneath your ship.
- **Setting it down** (when the ground reaches it): another **500**. A full rescue is therefore 1000.
- **Shooting a humanoid yourself** kills it. If you lose it, its lander becomes a mutant at the top of the screen.

## 4. Player

**Horizontal physics** (`PLAYER` routine, runs every frame):
- **Thrust** adds 3 velocity units per frame (`PLADIR=$0300` added into the fraction).
- **Drag** removes velocity/64 every frame (exponential damping).
- **Terminal velocity** is therefore 192 units ≈ **6 px/frame ≈ 360 px/s**, about 5.7 s per world lap. There is a hard clamp at ±256.
- The time constant is 64 frames (about 1.07 s): roughly 63% of top speed after 1 s of thrust. After releasing thrust it coasts with the same decay. This "heavy" inertia is a core part of how the game feels.

**Camera / ship screen position:**
- Facing right, the ship rests at about **x=64 px** (column $20). Facing left, it rests at about **x=224 px** (column $70).
- The resting position is offset forward by velocity/8 columns (up to about 48 px at speed).
- The ship's screen X can only change by **1 column (2 px) per frame**, with the scroll compensating (`BGDELT`). So on **Reverse**, the ship flips instantly but then **glides across the screen** to the far side over about 1–1.5 s while the world keeps scrolling with your old momentum. Reverse doesn't zero your velocity; thrust has to fight it.

**Vertical movement:**
- No inertia: the ship starts moving at 1 line/frame and ramps by 8/256 per frame up to **2 lines/frame**.
- It stops instantly on release.
- It is clamped between `YMIN+1` and line 238.

**Laser** (`LFIRE`, `LASR`/`LASL`):
- At most **4 beams** on screen at once.
- The beam spawns at the nose. The head extends **8 px per frame**. A broken, "fizzling" trailing segment follows at 6 px per frame using a random pattern table (`FISTAB`), so the beam stretches as it travels.
- Each beam is one horizontal line, colour-cycled (`COLR`).
- It stops at the first object it hits (one kill per beam) or at the screen edge.

**Smart bomb** (`SBOMB`):
- **3 at start.** P1SBC is loaded with the ship count, so it equals the ship setting.
- It kills every enemy **currently drawn on screen** and awards full points.
- Pods killed this way still release swarmers, which survive because they aren't drawn yet.
- Effect: the background is inverted 8 times at 2-frame intervals, i.e. **4 white flashes**, plus `SBSND`.
- It needs a button release before it can fire again.

**Hyperspace** (`HYPER`):
- Clears the screen and all enemy shots.
- Puts you at a **random world X**, random facing, random Y, with zero velocity.
- Materialises over 40 frames.
- Then **you explode if a random byte > 192: about a 24.6% chance** (`LSEED`).

**Lives and bonus:**
- **3 ships** by default (adjustable; one Taito build defaulted to 5).
- **Every 10,000 points** (`REPLAY` default `$0100`): +1 ship **and +1 smart bomb** (`INC PLAS` / `INC PSBC`), with a jingle.

**Death sequence** (`PDEATH`, `PLEX`):
1. The screen goes **black** and the ship is redrawn in monochrome, flashing through palette steps **red, red, red, orange, yellow, light yellow, white, white** every 2 frames.
2. Then one **full-screen white flash**.
3. All enemies are wiped.
4. The ship bursts into **128 particles**. Velocities are random within a diamond/ellipse shape (corner values are rejected). Particles are drawn as 2-pixel "flavours" and fade **white → yellow → orange → red → dark** over a long tail: 56 ticks on the first colour, then 4 ticks per step.
5. Death sound. Then the wave-clear check, then the next ship (or the other player's turn).

## 5. Complete scoring table (from code)

| Event | Points |
|---|---|
| Lander | 150 |
| Mutant | 150 |
| Swarmer | 150 |
| Baiter | 200 |
| Bomber | 250 |
| Pod | 1000 |
| Catch a falling humanoid | 500 |
| Set a carried humanoid down | 500 (1000 total rescue) |
| Humanoid lands safely by itself | 250 |
| Enemy shell or mine collides with you | 25 |
| End-of-wave bonus, per surviving humanoid | 100 × wave, capped at 500 from wave 5 |
| Bonus ship + smart bomb | every 10,000 |

Factory high-score table (`DEFALT`): DRJ 21,270; SAM 18,315; LED 15,920; PGD 14,285; CRB 12,520; MRS 11,035; SSR 8,265; TMH 6,010.

## 6. Attract mode, HUD, and messages

**HUD** (`BORDER`, `phr6.src` constants):
- A full-width horizontal line at about line 40 separates the HUD from the play field.
- The **scanner** is a framed box in the top centre, about 128 px wide × 32 lines tall, showing the whole 2048 px world at about 16:1.
- A pair of bracket marks in the scanner shows the current screen window. The scanner is centred on the player and draws the terrain profile plus coloured enemy and humanoid blips.
- **P1** score and reserve ships with smart bombs sit at the top left. **P2** is at the top right.

**Attract sequence** (`amode1.src`, coded by DeMar overnight before the AMOA show):
1. "WILLIAMS" logo page.
2. "ELECTRONICS INC. PRESENTS".
3. The **DEFENDER** logo materialises piece by piece using the appear effect.
4. Copyright line.
5. Hall of Fame tables: "TODAYS GREATEST" and "ALL TIME GREATEST" (`HALL OF FAME`).
6. **Instruction page:** a scanner and terrain are drawn. A lander descends onto a man and lifts him. The ship lasers the lander, then flies to catch the man. Next, each enemy appears one at a time, gets lasered, then re-appears parked in a table with its name and points ("LANDER", "MUTANT", "BOMBER", "POD", "SWARMER", "BAITER").
7. Loop.

**In-game messages:**
- "PLAYER 1/2" prompt at the start of each turn.
- "ATTACK WAVE n COMPLETED" and "BONUS X n" at wave end.
- "PLAYER n GAME OVER" in 2-player games, otherwise "GAME OVER".
- High-score initial entry uses the joystick up/down to change a letter and fire to advance, with a blinking underline (`HALL OF FAME ENTRY`).

## 7. What makes Defender feel like Defender

**What the designers said:**
- Jarvis on the premise: "if you're defending something, you're being attacked, and you can do whatever you wanted." He added the humanoids specifically to combine **offence and defence** into "a rich tactical and strategic experience", where you constantly re-prioritise between killing aliens and saving people. The loss is visible (mutants, an exploding planet), and that makes neglect self-punishing.
- **A world bigger than the screen, plus the scanner.** The big decision was a scrolling world. Steve Ritchie convinced Jarvis it had to scroll **both ways**. The scanner turns the game into constant triage across the whole planet.
- **The controls are deliberately hard.** A 2-way stick plus Thrust, Reverse, Fire, Smart Bomb and Hyperspace mixes the Space Invaders stick with the Asteroids buttons. The team was "proud that it intimidated everyone". Jesper Juul argues that this "unnecessary" difficulty *is* the game.
- **Sam Dicker's particle explosions and appear effects** re-energised the project. Death and materialisation are spectacle, not just events.
- **Baiters exist to stop camping.** The source confirms this mechanically: the timer tightens as the wave drags on and goes into panic mode when few enemies are left.

**Feel levers from the code that matter most for Warden:**
1. Heavy horizontal inertia (drag 1/64 per frame, about 1 s time constant) against twitchy, inertia-free vertical movement.
2. Reverse is instant, but the ship's slow 2 px/frame glide across the screen creates the "whip-around".
3. The camera leads further as you speed up.
4. Long, fast, stretching laser beams, capped at 4.
5. Enemy fire is aimed, jittered, and often leads you.
6. Mutants dodge your firing line before they charge.
7. Baiters match your speed, so running never works.
8. Difficulty ramps both within a wave (every 10 s) and across waves (capped around wave 14).
9. Screen flashes and full palette effects for smart bombs, death and planet loss.

**Things to keep original in Warden:**
- Don't copy these sprites, sounds, names, or the "ATTACK WAVE" text and the logo.
- The numbers and system rules above are mechanics, and those are fine to re-implement with your own art and naming.

## Sources

- Original source (primary for all numbers): https://github.com/mwenge/defender. The files I read were `src/blk71.src` (`WVTAB`, player explosion), `src/defa7.src` (`GEXEC`, `GETWV`, `WDELT`, `BONUS`, `PLAYER`, laser, `SBOMB`, `HYPER`, `PDEATH`, replay), `src/defb6.src` (enemy AI, kill/score values, humanoid fall, picture sizes), `src/romc8.src` (CMOS defaults: 3 ships, 10,000 replay, difficulty 5/15, restore wave 5, high-score table), `src/amode1.src`, `src/mess0.src`, `src/phr6.src`, and `src/info.src` (ROM revision notes).
- Wikipedia (development history, Jarvis "Defenders" quote, Ritchie bidirectional scrolling, Dicker explosions, hardware, controls rationale): https://en.wikipedia.org/wiki/Defender_(video_game)
- Mental Floss ("richest elements… were bugs" quote, scanner as an innovation, 990,000 bug): https://www.mentalfloss.com/fun/video-games/facts-about-defender
- Jesper Juul, "Defender: Unnecessarily Hard": https://www.jesperjuul.net/text/defender/
- Creative Computing Video & Arcade Games, Fall 1983 (point values, strategy; some values are from home ports): https://www.atarimagazines.com/cva/v1n2/defender.php
- Atari 400/800 manual transcription (home version, used only for cross-checks): https://ctrl-alt-rees.com/archive/www.cyou.com-~richard/DEFENDER.TXT
- Interviews checked that had no Defender-specific design detail: https://dadgum.com/halcyon/BOOK/JARVIS.HTM and https://www.gamedeveloper.com/production/eugeneology-an-interview-with-eugene-jarvis
- Couldn't fetch (403/402/401 errors): StrategyWiki, the shmup and Gamicus fandom wikis, arcade-history.com, and the 1982 Joystik article on vgpavilion.

**Open items, in order of importance:**
- **Wave 1 composition.** The code reads as 0 bombers and 0 pods in wave 1, which is the main conflict with player recollection. Check it in MAME.
- **Timing scale.** All timings assume one scheduler tick = one 60 Hz frame. Check it in MAME by timing the first baiter in wave 1 (the code predicts about 48 s).
- **Enemy colours are not verified.** They are set by palette registers at runtime, so the colour descriptions above are common-knowledge approximations, not values decoded from the source.
