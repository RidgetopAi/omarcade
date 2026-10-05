# Warden — the plan to finish game four

Game four has been called `defender` while it was being built. Its name is **Warden**.
This plan covers everything between develop `1edd4a7` and shipping it.

Written 2026-10-05 by Ridge, from three research passes:

| Doc | What it holds |
|---|---|
| `docs/warden/codebase-inventory.md` | What exists today, file by file. What is complete, partial or missing compared with the 1981 game. How art and sound get from the tools into the code. Every place a rename or ship has to touch. |
| `docs/warden/research-gameplay.md` | The original's enemy AI, wave table, scoring, humanoid rules, player physics, HUD and attract mode. Numbers come from the released 6809 source. |
| `docs/warden/research-sound.md` | How the Williams sound board works. Every Defender sound with measured numbers, plus a synthesis recipe for each one. |
| `~/projects/omarcade-reference/` | **Outside the repo, never commit it.** The original source, a sound-ROM emulator, and 25 reference renders to A/B against. |

Mandrel (brian tenant, project `omarcade`) has the history. The original plan is
`98ace083` (ref:defender-plan). Lander points were amended in `a7f355dd`. The last handoff
before this plan is `44f3d6b9`.

---

## How the work changes from here

Until now Brian drew every piece of art and built every sound by ear in the playgrounds.
Claude wired them in. On 2026-10-05 he handed over the build:

> "time is an issue for me and your capabilities are off the chart now … finish our 4th game …
> develop the rest of the assets, and sound effects. Sound effects are important … especially
> the phaser type sound … complete game play with waves."

**So Claude now drafts the art and the sounds as well.** Brian approves by eye and by ear.

- **Art.** Draft each piece as a vector-playground export (`pub const …: Shape` + `draw_*`,
  the exact shape `tools/vector-playground.html` emits and re-imports). That way Brian can
  open any piece in the playground and move a point.
- **Sound.** Build each voice from the Williams mechanism in `research-sound.md`. Render it
  to WAV and put it next to the matching ROM render in `omarcade-reference/rom-renders/`.
  Measure both with `tools/sound-scope.py`. Then Brian listens.
- **Brian's existing voices stay as they are.** These are Zap, Boom, MutantBoom, Warp and
  Thrust. He settled each one by ear. Change one only if he hears an A/B and picks the
  alternative.

**What stays with Brian:** whether something looks right, whether something sounds right,
the controls, and anything that reaches `main` or the marketplace listing.

---

## Rules carried forward (each one cost time once)

- **Work on `develop` only.** Every push to `main` changes the live listing at
  omarchyplugins.com. Merging to `main` is Brian's release call.
- **Do not retune these:**
  - the flight model (`bb715b33`)
  - the fire rate
  - the S5/S6/S7 numbers
  - Brian's voices
- **No terrain collision.** Brian: "can't commit to it yet".
- **Points:** LANDER 100 · MUTANT 150 · BAITER 200 · BOMBER 250 · POD 1000 · SWARMER 150
  (`a7f355dd`, Brian's call from the arcade chart).
- **World geometry:**
  - Use `world::delta` for every measurement between two world positions.
  - Facing is `.flipped()`, never `.facing(PI)`.
  - Art pieces overlap. They never share an edge.
  - `Lander.target` is an index, and `Humanoids` never compacts.
- **Threat colours are hardcoded**, never taken from the theme (L065).
- **Test the path the game actually takes (L069).** A test that drives a method gameplay
  never calls is not a test: Warp passed its tests while it could never play. Mutation-check
  every new assertion.
- **Sound levels:**
  - A `_LEVEL` constant is not the peak. Render the voice and measure it.
  - `register` (continuous) and `register_sound` (one-shot) are different calls.
  - Feed continuous voices above the pause guard.
- **Build constraints:**
  - core has no dev-dependencies, and no crate gets added.
  - The defender crate is a binary, so run clippy with `--bins`.

## The gate, every commit

```bash
cargo test --workspace            # 948 at 1edd4a7 — the number only goes up
cargo clippy --workspace --bins   # nothing new (flight.rs:71 is pre-existing, W0 fixes it)
cargo run --release -p omarcade-warden --example dump_frame -- <scene> /tmp/x.png   # look at it
```

Commits follow the house style:
- The subject is a plain-English sentence describing the outcome.
- The body gives measured numbers, the test total and a clippy note.

---

## The stages

Each stage gets its own Mandrel task under `tree:omarcade branch:warden`. A stage is done
when every check under "Done when" passes **and Brian has flown it**.

### W0. Rename it, clean the ground (small; do it first)

The rename is free until the game is registered in `install.sh`. Doing it first means
every later commit already uses the final names.

1. Rename `games/omarcade-defender` to `games/omarcade-warden`.
   - Package and binary become `omarcade-warden`. `Cargo.lock` regenerates.
   - `struct Defender` becomes `Warden`.
   - Update the `-p omarcade-defender` usage lines in `examples/*`, and the path in
     `tools/sound-playground.html`.
   - **Keep every comment where "Defender" means the 1981 arcade game.** The inventory
     lists which those are.
2. Fix the stale docs:
   - `main.rs` header ("scanner missing", "NOT YET REGISTERED")
   - `render.rs` "scanner belongs to a later stage"
   - `docs/audio-api.md:2` ("proposed, not built")
3. Remove the old HUD position strip. The scanner's view box replaced it.
4. Fix clippy at `flight.rs:71`: `DRAG` should use `std::f32::consts::LN_2`. The value
   does not change.
5. Make the enemy model general:
   - `Landers`/`Lander` become `Enemies`/`Enemy`.
   - `Kind` grows one variant per enemy type, each with its own `step_*`.
   - Keep the `take_spawned`/`take_fused` counters. Keep the humanoid-index invariant.

**Done when:**
- `cargo test --workspace` passes at ≥948.
- clippy is clean.
- `git grep -i omarcade-defender` returns nothing outside `docs/warden*`, which record the
  before-state and the rename itself.

### W1. Waves and scoring (was S9; Brian's own next step)

This stage unblocks Brian's `Warp` voice, which is waiting on mid-game spawns.

**The director.** Add `src/waves.rs`. Every tunable is a named constant in one table, the
same shape as the original's wave table:

| | Wave 1 | Wave 2 | Wave 3 | Wave 4+ |
|---|---|---|---|---|
| Landers | 15 | 20 | 20 | 20 |
| Bombers | 0 | 3 | 4 | 5 |
| Pods | 0 | 1 | 3 | 4 |

- **Lander arrival:**
  - Landers arrive in **squads of 5**.
  - The next squad comes every ~7.5 s, or immediately if no Landers are alive.
  - Never more than **8 Landers alive** at once.
- **Pods** follow Brian's spec of 1, 3 and 4, which matches the original code.
- **Bombers and pods** are placed when the wave starts.
- **Difficulty:**
  - Within a wave, speeds and fire rates ramp every ~10 s.
  - Across waves they ramp, and the ramp stops growing around wave 14.
  - ⚠️ **Density:** our world is 4 screens wide; the original's is about 6.7. Measure enemy
    density per screen against the original before copying counts as-is.
- **Spawning goes through `spawn()`**, so `take_spawned` fires and Warp plays.
  - Decide whether the opening squad should be audible. A wave the player watches arrive
    probably should be.

**Wave end** happens when every hostile is dead (alive and reserve).
1. Show a banner. Write our own text, not "ATTACK WAVE".
2. Tally the bonus one humanoid at a time: **100 × min(wave, 5)** each.
3. Pause about 2 s, then start the next wave.
4. **Every 5th wave:**
   - Humanoids reset to 10, spread across the world.
   - `Terrain::restore()` runs. Nothing calls it yet.
   - Humanoids are reset at wave boundaries only, so the index invariant holds.

**Scoring:**
- Rescue scoring:
  - catching a falling humanoid **500**
  - setting it down **500**
  - a humanoid that lands safely on its own **250**
- Add a small score pop-up for each.
- The real score HUD replaces the placeholder.
- `core::scores` `ScoreFile` with `GAME_ID`/`GAME_NAME`. This is also what makes the
  Marquee list the game.
- **Every 10,000 points:** one life and one smart bomb. The smart-bomb counter shows in
  the HUD now, even though bombs arrive in W2.

**Gameplay gaps to close here:**
- Landers fire too. Make it slow and easy to dodge, per Brian's spec.
- Ship-to-enemy body collision.
- Game-over leads to restart, and the score is saved.
- Vary the seeds per game. They are fixed today, so every game plays the same.

**Done when:**
- Waves 1→6 play through: bonus tally, restore on wave 5, Warp audible on squad arrival.
- Score persists.
- Tests cover the real paths: a full wave completion driven through `step`, and the
  restore on wave 5.

### W2. Smart bomb and hyperspace (was S10)

- **New keys.** `core` `Key` has no spare keys, so add variants the way `T` was added.
  Proposed: **B** or **Shift** for smart bomb, **H** for hyperspace (Enter is free too).
  **Brian picks.**
- **Smart bomb:**
  - Kills every hostile drawn on screen and awards full points.
  - Humanoids are untouched.
  - Swarmers from a pod killed this way survive.
  - Effect: 4 quick full-screen flashes.
  - Start with 3 bombs. Needs a key release before the next one fires.
- **Hyperspace:**
  - Random world X and Y, zero velocity.
  - Clears enemy shots.
  - About a 40-frame materialise.
  - **~25% chance of dying** on arrival (the original's 64/256).
- **Recommendation:** keep the death risk. It is what makes hyperspace a gamble rather
  than a free escape.

### W3. Baiters, Bombers and mines (was S11)

- **Baiter:**
  - Flat, wide, thin saucer.
  - X velocity = the player's velocity ± a margin toward the player, so it can't be outrun.
  - Re-aims on a random roll, and fires often.
  - **Timer:** the first one arrives ~48 s into wave 1, shrinking per wave and through the
    wave. Panic mode at ≤8 and ≤3 enemies left. At most 12.
  - Arrives with Brian's Warp.
- **Bomber:**
  - Small box, palette-cycling colour.
  - Squads of up to 3, constant X velocity, alternating direction per squad.
  - Keeps 16–32 px above or below the player when on screen.
  - **Never fires.**
  - **Mine:** drops one on a 1-in-8 roll, at most 10 live. Mines are stationary, cannot be
    shot, and live 1–32 ticks.
- **Art needed:** Baiter, Bomber, Mine.

### W4. Pods and Swarmers (was S12)

- **Pod:**
  - Starburst shape. Drifts at random. Never shoots.
  - Worth 1000. Bursts into **1–6 Swarmers**, weighted toward fewer.
  - At most 20 Swarmers alive at once.
  - **Brian's spec:** Pods never number more than 4.
- **Swarmer:**
  - Tiny teardrop. Undulating chase (random Y acceleration).
  - Fires sometimes.
  - Brian's tip: **flying close behind them, they don't turn** — keep that exploitable.
  - Leftover Swarmers carry over to your next life.
- **Decide whether a Pod burst plays Warp.** Swarmers arrive through `spawn()`, so by
  default it would.
- **Art needed:** Pod, Swarmer.

### W5. The sound pass (was S13) — "the phaser" (Brian named this as the most important part)

**Engine** (recommended). A small `williams` voice kit in the game crate. Each primitive is
a state machine emitting `(dac_u8, hold_cycles)` on a **virtual 894,886 Hz clock**. It is
rendered with zero-order hold, box-filtered down to the output rate, then a ~20 Hz DC block.
This is what gives the stair-stepping and aliasing the original has.

**Primitives** (all detailed in `research-sound.md` §1 and §3):

| Primitive | Original routine | What it is |
|---|---|---|
| clock-noise | TURBO | 16-bit shift-register noise whose clock falls while the level fades. |
| slew-noise | FNOISE | Random-target ramp, optionally decaying. |
| toggle-noise | LITE/APPEAR | 1-bit noise with a sweeping clock. |
| vari-square | VARI | Duty sweep. |
| wavetable | GWAVE | Period patterns, echoes, and erosion that wraps around instead of clamping. |
| scream | SCREAM | 4 phase accumulators. |

**The laser — the "phaser" Brian means.**
- In the original it is not a tone. It is **TURBO**: shift-register noise whose clock falls
  from ~15.8 kHz to ~430 Hz while the level fades slowly. Each shot restarts it.
- Brian's Zap (a bandpass swept 1940→260 Hz over white noise) is an ear-match of that.
  Ear-wise the two are close relatives.
- **Build a clock-noise candidate and A/B it** against Zap and `14_laser_TURBO.wav`. Brian
  picks.

**Voices still to build** (event → original mechanism → status):

| Event | Original | Status |
|---|---|---|
| Fusion (Lander+Humanoid → Mutant) | SP1 vari-square buzz rising in steps | ⚠️ counter live (`take_fused`), voice missing |
| Lander grabs humanoid / tractor beam | GWAVE 6-note arpeggio | missing |
| Humanoid falling (scream) | SCREAM, 4 staggered falling squares | missing |
| Catch | GWAVE 3 rising "bloops" | missing. ⚠️ `rescued_this_frame` is set and never played — cheapest win in the project |
| Set down | VARI 378→1530 Hz rising sweeps | missing |
| Humanoid killed | LITE 1-bit crackle sweeping down | placeholder PersonBoom |
| Player death | LITE×2 then CANNON tail, ~2.8 s | placeholder ShipBoom |
| Smart bomb | LITE stutter ×6 then CANNON | missing |
| Planet explodes | TURBO→LITE→CANNON chain | reuses Boom |
| Enemy shot: Lander/Baiter, Mutant, Swarmer | three distinct GWAVE voices | all reuse the laser at 0.55 |
| Hits: Bomber, Pod, Baiter/Swarmer | three distinct GWAVE voices | missing |
| Extra life | VARI falling-sweep fanfare | missing |
| Wave start / materialise | APPEAR rising toggle-noise | Brian's Warp covers this |
| Wave complete | **silence** in the original — then a bonus tally tick per humanoid (ours) | missing |
| Game start | SV3 chopped 166 Hz tone | missing |
| Hyperspace | **no sound** in the original | decide |

**Character rules:**
- **Choke vs polyphony.** The original is **monophonic**: every new sound cuts off the last
  one, chosen by priority. Our mixer is polyphonic. Recommend **choke groups**: shots choke
  shots, and big events choke the bed. Full mono would bury the laser under every explosion.
  **Brian decides.**
- **Level.** Warp at peak 0.86 is the loudest sound in the game and nobody has judged it in
  play. It becomes audible in W1. Do a **level pass** once every voice exists: render all of
  them and set peaks as a set, not one at a time.
- **Pitch.** Every voice ignores the `pitch` argument. Either implement it in the kit or
  document it in core.
- **Playground.** `tools/sound-playground.html` names every export `Zap`. Add the name field.

### W6. Presentation and ship (was S14)

- **Game screens:**
  - Title/attract: logo, an enemy roster card with points, the high-score table.
  - "Press Space".
  - Wave banners, game-over, restart.
- `VolumeIndicator` (the other three games have one).
- **Cabinet and packaging:**
  - `packaging/install.sh` `GAMES=(…)`.
  - `packaging/omarcade-warden.desktop`.
  - `tools/make-cabinet-art.sh` → `docs/cabinet/warden.png`.
  - `tools/make-preview.sh` is hard-coded to **3 panels**, so four is a layout call
    (Brian).
- **Text:**
  - `manifest.json` description ("Three original…" becomes four).
  - `Cabinet.qml:707` install prompt.
  - README: a Warden section, the architecture tree, the test count (it says 804, actual 948+).
- Retire `assets-inbox/` and delete `examples/size_check.rs` **in the same commit**:
  `cargo test` compiles examples, so removing one without the other breaks the build.
- **Release:** merge `develop` → `main` and bump the version. **Brian's call**, because it
  changes the live listing.

---

## The art to draft (vector-playground format, overlap rule, ship ≈ 35 units long)

| Piece | Read | Size vs Lander (original px) | Colour (threat colours are hardcoded) |
|---|---|---|---|
| Baiter | flat, wide, thin saucer; 3-frame shimmer | 12×4 vs 10×8 → wider, half height | iridescent cycle |
| Bomber | small box | 8×8 | palette-cycling |
| Mine | tiny cross/dot | 4×3 | pulsing |
| Pod | starburst | 8×8 | purple/red |
| Swarmer | tiny teardrop | 6×4 | red/orange |
| Score pop-ups | 250 / 500 | — | — |
| Smart-bomb HUD icon | — | — | — |
| Title logo "WARDEN" | — | — | — |

The colours listed are common knowledge, not decoded from the ROM. Ours can differ.

For each piece:
1. Write the export file.
2. Add it to `size_check` and render it next to the ship.
3. Add a `dump_frame` scene.
4. Show Brian before wiring.

Steps 2–9 of the inventory's "adding a sprite" are the wiring checklist.

---

## Recommendations beyond the original (Brian to accept or drop)

1. **Laser hit detection.** It only tests the beam's head, but the beam is drawn up to ~60%
   of the screen. An enemy that drifts into the visible beam isn't hit. The original stops
   the beam at the first object it touches, so test the whole lit segment.
2. **Hitbox drift test.** Hitboxes are typed in by hand. Add a test that derives each
   `*_HALF_W/H` from the shape's real bounds, so art and collision can't drift apart.
3. **Mutant dodge.** In the original, Mutants avoid your firing line until they're close,
   then charge. Ours chase with jitter. The dodge is most of why Mutants feel smart.
4. **Reverse glide.** The original ship glides across the screen at 2 px/frame on reverse.
   That doesn't change physics, so it isn't covered by the "do not retune the flight model"
   rule. Compare in play before touching anything — it may already read right.
5. **A test-only wave skip** (debug build only, like F8) to fly wave 5 and 10 without
   playing through.

## Open questions for Brian

1. **Lander points.** The released source scores a Lander at 150. Your arcade-chart
   screenshot said 100, and 100 is what we use. Keep it, or did that chart come from a
   different revision?
2. **Keys** for smart bomb and hyperspace.
3. **Hyperspace death chance:** keep ~25%?
4. **Choke groups** vs full polyphony.
5. **Cabinet preview:** four panels, or swap the layout?
6. **Wave 1 composition:** the source gives Landers only, no Bombers or Pods. That is
   different from some memories of the game — check it in MAME if it matters.
