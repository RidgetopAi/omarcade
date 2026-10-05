# Warden — codebase inventory at develop 1edd4a7 (2026-10-05)

> Snapshot of what exists in games/omarcade-defender before the Warden completion work. Line refs drift — re-grep before trusting one.

## Defender (Warden) inventory, `games/omarcade-defender` at `1edd4a7`

### 1. Architecture

The crate is a binary only (no lib target) and depends only on `omarcade-core` (`Cargo.toml:14-15`). It has 12 source files, about 8,900 lines, and 122 `#[test]` functions. Line refs are relative to `games/omarcade-defender/src/`.

| File | Responsibility |
|---|---|
| `main.rs` | Wiring. The `Defender` struct (`:124`), a fixed 240Hz step (`FIXED_DT :73`, accumulator capped at 0.25s, `:604`), `step` (`:288`) and the collision resolvers (`resolve_hits :435`, `resolve_catches :407`, `resolve_ship_hit :362`, `check_world_end :388`). Sound is decided once per frame through `*_this_frame` flags so two kills in one frame don't double-play. Input at `:557`. |
| `world.rs` | The world loops: `WORLD_SCREENS=4`, `VIEW 960x720`, `WORLD_W=3840` (`:20-30`). `wrap` uses `rem_euclid` (`:212`), `delta` measures the short way round (`:227`). Terrain is 3-octave value noise, blended where the ends meet (`:64-109`). `destroy`/`restore` (`:121-128`); once destroyed, `height_at` returns 0. Terrain is fly-through, with no collision (`:40`). |
| `flight.rs` | Horizontal motion has momentum: `LAP_SECONDS=6` → `TOP_SPEED`, 1s spin-up, exponential drag with a 1.6s half-life, and `REVERSE_BITE=2.2` (extra thrust when reversing). Vertical is direct at `VIEW_H/1.5` per second (`:36-96`, `step :179`). The camera leads by 22% in the facing direction and eases with `exp` (`:109-119`, `follow :270`, `to_screen :307`). Brian approved the feel ("can't find anything wrong"); don't retune it. |
| `shot.rs` | Shots live in a pool in world space. Player shots are horizontal: 2400 u/s, range 1.15 screens, `FIRE_INTERVAL` 0.16s. A shot is drawn as a beam that grows as it flies (`SHOT_TAIL_FRACTION`, `beam_len :169`), with 6 colours cycling per shot. Enemy shots are 900 u/s, angled, owner-tagged. There is a separate enemy cap of 40. |
| `enemy.rs` | One `Lander` struct with `Kind {Lander, Mutant}` (`:158`) and `Phase {Warping, Hovering, Hunting, Grabbing, Carrying, Dying}` (`:133`). Each step returns an `Outcome` that the owning `Landers` applies (`step :652`). There are `spawned`/`fused` counters for sound (`take_* :591-604`). |
| `humanoid.rs` | `State {Walking, Carried, Falling, Rescued, Dead}` (`:67`). Falls have gravity; a fall longer than `VIEW_H/3` (measured from where it started) kills. There's a generous catch box, carry under the ship, and setting down below 90u (`main.rs:101`). The dead stay in the list on purpose, because Landers hold indices into it. |
| `lives.rs` | Three lives. `Alive/Respawned/Dead/GameOver`. Respawn is in place after 1.2s, followed by 2s of blinking invulnerability. |
| `scanner.rs` | Complete. The whole world in a strip 52% of the screen wide. Mutants pulse. The view box is drawn in two pieces when it crosses the wrap point (`:355`). Read-only. |
| `effects.rs` | Particles in world space, drawn through the camera: lander, humanoid, ship and world explosions, plus the thrust plume (`:187-352`). |
| `render.rs` | Draw order (`draw :50`), tractor beams, beam laser with dash pattern (`SHOT_PALETTE :235`, `SHOT_DASHES :272`), lives drawn as small ships, placeholder score and GAME OVER, speed bar and position strip (`draw_hud :520`). |
| `art.rs` | Vector shapes for the ship (`SCALE 2.0`, `draw_ship :218`, flare driven by `exhaust`), Lander, Mutant and Humanoid. |
| `sound.rs` | 7 synthesised voices (section 3). |

**Implemented gameplay:**
- Landers warp in, drift for 2.4s, then pick the nearest unclaimed Humanoid, descend, beam it for 0.9s and carry it up.
- At `y ≥ 1.6×VIEW_H` the Lander mutates in place (`enemy.rs:414, :717`).
- Shooting a carrier drops the passenger (`release_passenger`). You can catch them and set them down.
- Your own laser kills Humanoids, deliberately.
- Mutants chase with jitter at 290 u/s and never fire level (`MUTANT_MIN_ANGLE` is enforced in `aim_at :479`).
- When the population hits 0, the terrain is destroyed and `mutate_all` turns every Lander into a Mutant.
- Pause is on P, and only KeyDown is swallowed while paused.

**Compared with Defender (1981):**

| Feature | Status |
|---|---|
| Lander | **Complete**. Points 100 (chart-verified). |
| Mutant | **Complete**. 150 points. |
| Baiter, Bomber, Pod, Swarmer | **Missing**. No stubs. `Kind` has only two variants, and no art exists. |
| Landers firing at you | **Missing**. Only Mutants fire. |
| Ship colliding with an enemy body | **Missing**. Only enemy shots can hit you (`resolve_ship_hit`). |
| Abduction, carry, fall, catch, set-down | **Complete**. |
| Rescue scoring (catch / set-down / safe landing bonuses) | **Missing**. Score only comes from kills. |
| Planet destruction when all Humanoids are lost | **Complete**. |
| Planet restore every 5 waves | **Missing**. `Terrain::restore` exists but nothing calls it. |
| Waves, attack waves, end-of-wave bonus per surviving Humanoid | **Missing**. 5 Landers are spawned once (`OPENING_LANDERS`); nothing happens when they're all dead. `Landers::remaining()` exists "for S9". |
| Smart bombs, hyperspace | **Missing**. Core `Key` only has `Left/Right/Up/Down/Space/Enter/Escape/P/T/M/Minus/Equals` (+F8 in debug). Enter is free; a hyperspace key needs a new core `Key` variant, the way T was added. |
| Extra life / smart bomb every 10,000 | **Missing**. |
| Game over | **Partial**. Text only, and it's terminal: no restart, and score isn't saved (`render.rs:399`). |
| Title screen, attract mode | **Missing**. |
| Score file, high scores | **Missing**. No `ScoreFile`. The on-screen score is a placeholder (`render.rs:413`). |
| Volume indicator | **Missing**. M/−/= are handled in the backend, but there's no `VolumeIndicator`, unlike the other three games. |

Code comments describe a stage plan:
- S1: world and flight
- S5: firing and Landers
- S6: Humanoids
- S7: losing
- S8: scanner
- **S9: scoring, waves and the score file. This is the next stage.**
- S11: the pause rule
- S13: sound pass
- S14: presentation (title and game-over screens)

### 2. Art pipeline

The Defender art is vector, not pixel. `core::Shape` holds a const point list with even-odd fill. `Transform::at(x,y).scaled(s).flipped(b)` positions it; `scaled` replaces the scale rather than multiplying it (`art.rs:243`). Shapes are drawn back to front and deliberately overlap so no hairlines appear.

**`tools/vector-playground.html`** (1,190 lines, one file, no build):
- Draw and edit points with snap, mirror-X and zoom.
- Layers, each with its own colour.
- A preview at game scale (`gscale`).
- Export (`emit() :615`) writes Rust: `pub const <NAME>_<LAYER>: Shape` for each layer, plus `pub fn draw_<name>()` that fills the layers in order. The name comes from the `artname` field; it defaults to `UNNAMED_SHAPE` so it can't overwrite an existing const.
- Import (`parseExport :710`) reads a pasted `.rs` back in. Brian made the Mutant this way: he imported the Lander and drew a Humanoid inside it.

**`tools/sprite-playground.html`** is the pixel-art tool for the racer's palette sprites (`car_palette` / `Sprite::from_rows`). Defender doesn't use it.

**How the existing art landed** (`assets-inbox/README.md`; commits `1dc60e8`, `d6310ac`, `3924451`):
1. Brian drew small and exported unnamed. `lander.rs`, `humanoid.rs` and `mutant.rs` all have `UNNAMED_SHAPE_*` consts and `draw_unnamed_shape`.
2. Claude scaled the coordinates only (Lander ×2, Mutant ×2, Humanoid ×2.5, origin unchanged) and renamed the prefix. The results are the `*_scaled.rs` files; a numeric-masked diff shows only the names differ.
3. `examples/size_check.rs` `#[path]`-includes the raw and scaled files and renders them beside the ship at the real `SCALE`, as a size check. The file marks itself "Temporary".
4. The scaled code was pasted into `art.rs` (`:257-670`).
5. Hitboxes are written by hand from the art's size: `LANDER_HALF_W = 6.0*SCALE` (`enemy.rs:66`), `humanoid.rs:21`.
6. `tools/zoom-png.py` magnifies small renders for checking.

**Adding a sprite (Baiter, Bomber, Pod, Swarmer):**
1. Brian draws it in the vector playground, in ship units (ship is about 35 units long). Optionally import `lander_scaled.rs` for size reference. Set the Export name (`BAITER`), copy, and save as `assets-inbox/baiter.rs`.
2. Check it for pieces that float free, outlines that cross themselves, and detail under about 2 square units.
3. Scale the coordinates if it was drawn small; save as `baiter_scaled.rs`.
4. Add it to `size_check.rs` and render.
5. Paste into `art.rs` under THE ENEMIES with a `draw_baiter`.
6. Add `Kind::Baiter` and extend `draw_enemy` (`render.rs:160`).
7. Add a scanner blip colour (`scanner.rs:76`, `draw_landers :283`).
8. Add `BAITER_HALF_W/H` derived from the shape's size, and points.
9. Add a `dump_frame` scene and look at it (`examples/dump_frame.rs` scenes: `rest, mutants, apocalypse, rescue, combat, laser, cruise, thrust, thrust-west, west, turn, low, seam`).

### 3. Sound pipeline

**Core seam** (`core/src/audio/`; spec in `docs/audio-api.md`):
- Built on cpal. Game → audio goes through a lock-free ring buffer; audio → game uses atomics only.
- `trait Voice { render(out, params, sr); alive() -> bool (default true); retrigger(gain, pitch) }` (`mixer.rs:23-43`).
- `AudioSystem::register_sound` gives a `SoundId` (one-shots, `play`/`play_with`). `register` gives a `VoiceId` (continuous, `start`/`stop`/`set(VoiceParams)`) (`mod.rs:157-164, :340-384`).
- `VoiceParams` is 4 f32 slots with named constructors, including the new `thrust(exhaust)` (`params.rs`).
- Every voice is registered at startup. Voices may not allocate, lock or panic.
- If audio fails, the game is silent rather than crashing.
- `docs/audio-api.md:2` still says "proposed, not built", which is wrong: it's built.

**Voices present** (`sound.rs`):

| Voice | Recipe | Status |
|---|---|---|
| `Zap` = `Laser` (`:76`, `:185`) | Noise through a bandpass swept 1940→260 Hz, Q 3.6, 0.58s | Brian's, by ear. On held fire it cuts off at 160ms, and he chose that. |
| `Boom` (`:236`) | 40Hz saw through a lowpass swept 1060→260 | Brian's (Lander death) |
| `MutantBoom` (`:367`) | Pure crackle, density 960→480 | Brian's; level raised from 0.30 to 0.45 |
| `ShipBoom` (`:501`) | — | Placeholder, not built by ear |
| `PersonBoom` (`:596`) | — | Placeholder, not built by ear |
| `Warp` (`:715`) | Noise + crackle through a bandpass rising 240→500 Hz, attack 38% | Brian's. Currently can't be heard in play (see below). |
| `Thrust` (`:992`) | Continuous, no envelope, driven by `exhaust`, LP 180→250 Hz with an 8Hz wobble | Brian's. Designed in `tools/thrust-probe.py`, not the playground, and deliberately has no `PLAYGROUND:` line. |

Enemy fire reuses the laser at 0.55 gain, and the world exploding reuses `Boom`.

**Why Warp can't sound yet:** `take_spawned` is fed only by `spawn`, and `spawn` is only reached by the silent opening wave until S9 adds spawns (commit `1edd4a7`).

**Sounds missing:**
- The fusion (Lander → Mutant) sound. It's counted (`main.rs:694 let _fused`) and deliberately silent until Brian builds it.
- A rescue or catch sound. The flag is set at `main.rs:411` and never played, so it's dead code.
- Setting a Humanoid down, an abduction cue, a Humanoid death from falling.
- A distinct enemy shot and a distinct planet explosion.
- Smart bomb, hyperspace, extra life, wave start/end/bonus, title/game-over.
- Voices for Baiter, Bomber, Pod and Swarmer. Brian has said the Warp will also cover Baiter spawns.

**Tools:**
- **`tools/sound-playground.html`**: envelope (len, attack, decay, level), filter (off/lp/bp/hp, from→to, Q), stacked oscillators (sine, square, saw, triangle, noise, crackle with sweep, curve, duty and snap). Space plays, and it can save a `.wav`. `exportRust() :512` writes a real `Voice` plus a `// PLAYGROUND: {json}` line, which the import (`:870`) reads back.
  - There is no `vname` input in the HTML, so every export is named `Zap` and has to be renamed by hand.
  - `ATTACK`/`DECAY` are fractions of the clip's length, not seconds.
  - It can only design one-shot clips.
- **`tools/sound-scope.py`**: `find`/`scope` modes take a YouTube URL or a file and report sweep direction, tonal vs noisy, and band energy. It's how sounds get measured against the real machine.
- **`examples/sound_lab.rs`**: renders candidate laser recipes to WAV.
- **`examples/laser_check.rs`**: `#[path]`-includes the real `sound.rs` to render the shipped voice, including retriggering.
- **`tools/thrust-probe.py`**: audition ladder for sustained sounds.

**Landing a new one-shot SFX:**
1. Brian designs it in the playground and copies the export.
2. Paste it into `sound.rs`, rename `Zap`→`X`, and keep the `PLAYGROUND:` line plus a "★ BRIAN'S, BUILT BY EAR" or "⚠️ PLACEHOLDER" note.
3. In `main()`, `audio.register_sound(Box::new(sound::X::new()))`. Add a `SoundId` field, a `*_this_frame` flag (or a counter for group events), and play it in `update` after the step loop.
4. Add tests in the existing style (`sound.rs` tests from `:1124`): it retires itself, never clips at full gain, never emits a non-finite sample, is distinguishable from the others, and has a test for its character (for example `the_warp_sweeps_upward`).
5. Render a WAV and run it through `sound-scope`.

For a continuous voice: `register` it, add a `VoiceParams` constructor in core, and call `start`/`stop`/`set` every frame *before* the pause guard (`main.rs:544`, `:604`).

### 4. Conventions, cabinet wiring, rename

**Tests:**
- Every game uses inline `#[cfg(test)]` unit tests: Defender 122, Pixel Break 253, Racer 243, Volley 109 (counted with `grep -c '#[test]'` per `src`).
- The last commit body reports "948 tests, 0 failed" for the workspace; README `:263` still says 804.
- Probes and examples (`probe_*`, `simulate`, `dump_frame`) are run by hand. `cargo test` compiles them but doesn't run them.
- There's no CI in the repo.
- Because the crate has no lib, examples `#[path]`-include `src/` files.
- `dump_frame` takes `<scene> <out.png>` in every game, and scenes have to be looked at by eye.
- Each commit body notes "Clippy unchanged: only the known pre-existing flight.rs:71" — the `DRAG` literal `0.693_147_2`, which would be fixed by using `LN_2`.

**Commit style** (`git log develop`):
- Subjects are plain-English outcome sentences with no prefix and no full stop ("Landers announce themselves", "Now you can lose").
- Bodies are prose with ★/⚠️/⇒ markers, Brian quoted directly, measured numbers, the test total, a clippy note, and `Co-Authored-By: Claude Opus 5 (1M context)` plus a `Claude-Session:` trailer.

**What Brian was doing last** (09-15 → 09-19):
- S5–S8 gameplay, then a long sound pass.
- Laser changed from a tone to swept noise.
- Four distinct death sounds.
- Crackle was added to the playground and used for the Mutant death.
- Thrust got a plume and an engine sound.
- The laser became a growing, colour-cycling, dashed beam, measured from 4 reference frames.
- Warp voice added.
- Last commit: the fix that found Warp can't be reached in play and added a separate fusion counter.
- Next pending: S9 (waves, scoring, score file), then the remaining enemy types.

**How the cabinet finds games:**
- `Cabinet.qml` looks for `omarcade-*` binaries in `~/.local/bin` with `find` (`:118-128`).
- Cabinet art is `docs/cabinet/<id without the omarcade- prefix>.png` (`:1162`).
- Names come from the score file's name, falling back to `prettify` (`:325`).
- `Marquee.qml` only shows games that have written a score JSON (`:118-128`), so Warden needs a `ScoreFile` to appear there.
- `ScoreRecord.qml` is generic.
- The Hyprland rule matches the `omarcade` app_id that core sets (`winit_soft.rs:24`), so nothing changes there.

**To ship game 4:**
1. Name it in `main.rs` and add a score file:
   - `GAME_ID`/`GAME_NAME` constants.
   - `ScoreFile::load_or_new`, recording at game over.
   - Change `TITLE` from the bare `"Omarcade"` (`:56`).
   - Delete the "NOT YET REGISTERED" note (`:27-30`) and the stale "scanner missing" header (`:23-25`).
2. Add the in-game pieces: `VolumeIndicator`, a title screen, and restart after game over.
3. Update the packaging and art scripts:
   - `packaging/install.sh:27` `GAMES=(…)`. Install and uninstall both loop over it.
   - A new `packaging/omarcade-warden.desktop`, following the volley file.
   - `tools/make-cabinet-art.sh:50-63`: add `-p` and a `render` line with a scene chosen for the thumbnail, which writes `docs/cabinet/warden.png`.
   - `tools/make-preview.sh`: it hard-codes **three** panels (`:12`, `:71-85`, `PANEL=W/3`), so going to four is a layout decision for Brian, followed by regenerating `preview.png`.
4. Update the text that names three games:
   - `manifest.json` description ("Three original arcade games…").
   - `Cabinet.qml:707` install prompt ("Pixel Break, Volley and Omaprix aren't built").
   - README: `:9`, `:32`, `:58`, a new game section, the architecture tree (`:225-245`), the dump_frame scenes, and the test count.

**Rename defender → Warden** (`git grep -i defender`, excluding Cargo.lock):
- **Has to change:**
  - Directory `games/omarcade-defender/` and `Cargo.toml:2`, which regenerates `Cargo.lock:1138`.
  - The `struct Defender` and `Defender::new` identifiers in `main.rs` (`:124, :210, :224, :556, :751, :764-787`).
  - `-p omarcade-defender` in the usage docs at `examples/dump_frame.rs:3`, `laser_check.rs:16,26-28`, `size_check.rs:3`, `sound_lab.rs:3`.
  - `tools/sound-playground.html:509` (path in a comment).
- **Optional (historical comments in core):** `core/src/audio/params.rs:126`, `core/src/backend/mod.rs:474,477`.
- **Should stay**, because they mean the 1981 arcade:
  - `flight.rs:19,41,104,148`, `enemy.rs:6,26,83`, `humanoid.rs:3`, `scanner.rs:7,32`, `shot.rs:17`, `world.rs:17`, `art.rs:600`, `render.rs:424,516`, `sound.rs:58,854,874`, `main.rs:83,570`, `assets-inbox/README.md:36`, `sound_lab.rs:22,27`, `tools/sound-scope.py:4,184`.
  - The Mandrel refs `ref:defender-points` and `ref:defender-thrust-bed`.
- **Not related:** `games/omarcade-volley/src/state.rs:50` ("a defender", the ordinary word).

### 5. TODO / known issues

There are no TODO, FIXME or `todo!` markers. The known issues are in prose:
- The deferred S9 / S13 / S14 work.
- ShipBoom and PersonBoom are placeholders waiting for Brian.
- Warp is now the loudest voice (peak 0.86) and no one has listened to it against the others (`f1b8fa1`).
- The fusion sound hasn't been built.
- `size_check` and the inbox are marked temporary.
- Terrain collision isn't decided.
- `WORLD_SCREENS=4` is described as a starting value.

### 6. Recommendations

1. **Fix stale docs:** `main.rs:23-25`, the `render.rs:516` comment saying "scanner belongs to a later stage", `audio-api.md:2`, and README `:263`. Drop the old HUD position strip in `draw_hud` (`render.rs:520`), which the scanner's view box now does.
2. **Delete or wire up the dead pieces:** `rescued_this_frame` (`main.rs:411`, never played), `let _ = w` in `draw_score`, and `let _ = theme` in `draw_shots`.
3. **Before adding four enemy types**, rename `Landers`/`Lander` to `Enemies`/`Enemy`, extend `Kind`, and give each kind its own step function (as with `step_mutant`).
   - A Pod bursting into Swarmers will go through `spawn()` and bump the arrival counter. Decide whether a burst should play Warp.
4. **Waves have to respect the Humanoid-index rule** (the list never shrinks, `enemy.rs:180`). Reset both lists and `claimed` at wave boundaries; restore the terrain and repopulate every 5th wave.
5. **Vary the seeds.** They're fixed (`0x0DEF_E4DE`, `0x5EED_1234`, `0x50C1_A15E`), so every game is identical. Mix in the wave number or start time.
6. **Fill the gameplay gaps:** Landers firing, ship-to-enemy body collision, and rescue scoring.
7. **Beam hit detection:** only the head of a shot is hit-tested (`resolve_hits`) while it's drawn up to about 60% of a screen. An enemy that moves into the drawn beam isn't hit. Test whether that reads as a bug.
8. **Hitbox checks:** `art.rs` has 0 tests. Hitboxes are said to come from the art but are typed in by hand; a test comparing the `LANDER_HALF_W` / `HUMANOID` values to each shape's actual size would stop them drifting apart.
9. **Sound playground:** add the missing name input so exports stop all coming out as `Zap`. Every voice ignores `pitch` — either document that in core or implement it.
10. **Inbox cleanup:** when the inbox is retired, delete `size_check.rs` in the same commit. `cargo test` compiles examples, so it would break the build.
11. **Clippy:** fix the `flight.rs:71` warning with `std::f32::consts::LN_2`.
