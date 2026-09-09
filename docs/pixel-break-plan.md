# Pixel Break — the plan

**Status:** proposed, not built. Written 2026-09-07. Nothing in this document has been
implemented.

Breakout becomes **Pixel Break**: ten levels, catchable power-ups, tiered bricks, and an
effects layer good enough to be the thing people screenshot. This is the release title —
the one that has to prove the suite is worth installing.

---

## 0. What is true today

Read this before designing anything on top of it.

| File | Lines | What it is |
|---|---|---|
| `state.rs` | 351 | The world, no behaviour. One field of bricks, 3 lives, `Phase::{Ready,Playing,Lost,Won}` |
| `physics.rs` | 517 | The only file that advances time. 240 Hz fixed step, shallowest-collision-wins, angle clamp |
| `render.rs` | 326 | The only file that draws. Letterboxed viewport, 10-sample ball trail, HUD, phase message |
| `main.rs` | 149 | Wiring only. Input → intent, time → sim, state → render. `update` ignores its `Audio` |

36 tests green. Zero sounds. One level. `GameState::new()` builds a 10x6 grid and that is
the whole game.

**Three invariants in `physics.rs` that were paid for and must survive every change below:**

1. **Fixed 240 Hz timestep.** Per-tick ball movement (~1.75 units) is far smaller than the
   thinnest brick. This is what makes tunnelling impossible rather than unlikely. Ball speed
   rises across levels — §3 caps it against this.
2. **Shallowest collision wins, one per tick.** Two overlapping bricks reflecting twice sends
   the ball back the way it came. Multi-ball multiplies the chances of this; §2 keeps the
   rule per-ball.
3. **`MIN_VERTICAL_FRACTION = 0.25`.** Below it the ball skims and the game stalls.

**Core surfaces this plan spends:** `Canvas::{fill_rect, fill_rect_f, veil, clear}`,
`Sprite`, `text` (5x7, `A-Z0-9 -` only), `Theme`, `ScoreFile`, and the `Audio`/`Voice` seam
that Omaprix proved.

---

## 1. The rename

`Breakout` → **Pixel Break**. Do this FIRST, in its own commit, before any feature work —
it touches identity, and identity is public surface.

| Thing | From | To | Note |
|---|---|---|---|
| Crate | `omarcade-breakout` | `omarcade-pixel-break` | directory move too |
| `GAME_ID` | `omarcade-breakout` | `omarcade-pixel-break` | **names the score file** |
| `GAME_NAME` | `Breakout` | `Pixel Break` | what the marquee shows |
| Window title | `Omarcade Breakout` | `Pixel Break` | |
| `.desktop` | `omarcade-breakout.desktop` | `omarcade-pixel-break.desktop` | |
| `install.sh` | `GAMES=(omarcade-breakout …)` | `omarcade-pixel-break` | |
| README, docs | Breakout | Pixel Break | keep historical mentions in comments |

⚠️ **The score file is keyed by `GAME_ID`.** Renaming it orphans
`~/.local/state/omarcade/scores/omarcade-breakout.json` — one real entry, 450 points, from
2026-08-29. The marquee discovers games by scanning that directory, so an orphan does not
error, it just shows up forever as a game called "Breakout" that nobody can play.

**Migration:** `install.sh` renames the file and rewrites `id` and `name` in place if the old
one exists and the new one does not. Idempotent, and it must never overwrite a new file with
an old one. `install.sh` already has precedent for exactly this — it removes the pre-suite
`omarcade.desktop` for the same reason.

⚠️ Old scores are **not comparable** to new ones — 450 points was a single 60-brick field.
The scoring in §5 is a different game. Migrate the file so the history is not lost, but the
migration writes `difficulty: "legacy"` on those entries so the marquee never ranks a 450
against a ten-level run. `ScoreFile` already groups by difficulty for exactly this reason.

`packaging/install.sh` also removes the stale `omarcade-breakout` binary from `~/.local/bin`,
or the cabinet lists two games — it discovers by `find` over that directory.

---

## 2. Balls, lives and the multi-ball

**The model (settled):** lives persist across all ten levels; balls are what is in play now.

- `lives: u32`, starting at 3. Lost **only when the last ball in play drains.**
- `balls: Vec<Ball>` replaces `ball: Ball`. Ordinary play is a one-element vec.
- Catching **Omarchy** spawns one more ball, up to the cap.
- Cap: **5 balls** on levels 1-5, **10 balls** on levels 6-10.
- Clearing a level keeps lives, score and level progress; balls reset to one.
- Lives gone → game over at whatever level you reached, score banked.
- Clear level 10 → you beat the game.

### What this does to physics

`step_fixed` currently reads `state.ball`. It becomes a loop over `state.balls`, and this is
the single largest source of new bugs in the whole plan. Four rules:

1. **Each ball collides independently, with its own shallowest-wins resolution.** Do NOT
   collect collisions across balls and resolve them together — invariant 2 is per-ball.
2. **A brick killed by ball A is dead for ball B in the same tick.** Since bricks are
   resolved one ball at a time, ball B simply sees `alive: false`. This is correct and free,
   but it means the *order* of balls in the vec is observable. Keep the vec stable —
   push new balls to the back, and remove drained ones by index after the loop, never during.
3. **Draining is per-ball, not per-frame.** A ball past the bottom is removed from the vec.
   `lose_life()` fires only when the vec becomes empty.
4. **`Phase::Ready` holds exactly one ball, on the paddle.** Multi-ball only exists during
   `Playing`.

A new test file's worth of guards, at minimum:
`a_ball_draining_with_others_in_play_costs_no_life`,
`the_last_ball_draining_costs_a_life`,
`two_balls_hitting_the_same_brick_in_one_tick_score_it_once`,
`the_ball_cap_is_five_below_level_six_and_ten_above`,
`a_level_starts_with_exactly_one_ball`.

⚠️ **`record_trail` samples `state.ball`.** With N balls it must keep N trails. A single
shared trail with several balls writing into it produces a line that jumps between balls —
this will look like a rendering bug and will be reported as one. Trail moves onto `Ball`.

---

## 3. The ten levels

### Brick tiers

| Tier | Hits | Reads as |
|---|---|---|
| Plain | 1 | solid brick, theme row colour |
| Reinforced | 2 | brick with a visible seam; first hit breaks a corner off |
| Armoured | 4 | brick with a heavy border; **chips off in four visible stages** |

You asked for the damage to *show* — "breaking off smaller pieces". That is the rule:
**a brick's remaining hits are always readable from its shape**, never from a colour the
player has to learn. §6 says how.

### The curve

```
L1   . . . . . .      all plain
L2   # . . . . .      1 row reinforced
L3   # # . . . .      2
L4   # # # . . .      3
L5   # # # # . .      4
L6   # # # # # .      5
L7   # # # # # #      6   <- reinforced peak
L8   = # # # # #      1 row armoured, 5 reinforced
L9   = = # # # #      2 armoured
L10  = = = # # #      3 armoured   <- final

.  plain (1 hit)     #  reinforced (2)     =  armoured (4)
```

Rows are counted **from the top**, which is the row hardest to reach — the field gets a
harder crust as you climb, and the ball has to get behind it.

**Total hits required:** L1 = 60, L7 = 120, L10 = **180**. That is a 3.0x climb in work, so
**ball speed must not climb anything like that** or L10 is unplayable.

⚠️ *Corrected at S4.* This originally read "L10 = 156", which is arithmetic that was never
checked: 3 armoured rows of 4 hits plus 3 reinforced rows of 2, over 10 columns, is
120 + 60 = 180, and no row combination of a 6x10 field produces 156 at all (150 and 160 are
the nearest). The layout above is unchanged — only the total was wrong. The curve it makes is
+10 per level to L7 as reinforced rows fill in, then +20 per level as armour replaces them.
`hits_to_clear_matches_the_planned_curve` asserts every step of it.

### Ball speed

`BALL_SPEED` is 340 today. Proposal: **340 at L1 rising to 460 at L10**, linearly —
`340 + 13.3 * (level - 1)`.

⚠️ **This is bounded by invariant 1.** At 460 units/s a 240 Hz tick moves the ball 1.92
units against a 28-unit-tall brick — still an order of magnitude of headroom, tunnelling
stays impossible. **A speed above ~1500 would start to threaten it.** State that constant in
the code and put a test on it: `ball_speed_at_every_level_is_far_below_the_tunnelling_limit`.
Derive the limit from `BRICK_H` and `FIXED_DT` rather than typing a number — this is L026,
and Omaprix paid for it twice.

Paddle speed rises with it, 600 → 700, so the paddle can always still get under the ball.
A test should assert the paddle can cross the field faster than the ball can, at every level.

---

## 4. Power-ups

### The four

| | Icon | Effect | Duration |
|---|---|---|---|
| **a** | **OMARCHY** in logo script | +1 ball, up to the cap | permanent for the level |
| **b** | magnet | hold Space to catch and aim | 20 s |
| **c** | grow | paddle wider — three strengths | 10 s / 25 s / 45 s |
| **d** | bomb | paddle **narrower** | 15 s |

Your numbers, as you said, are educated guesses. They are written as named constants in one
place so tuning them is a one-line change, and §9 says how we tune them.

### Drop rules

- A brick's **final** hit may drop one item. A reinforced brick's first hit drops nothing —
  otherwise armoured bricks would rain power-ups.
- **Ratio 1 bomb : 4 good.** Not per-drop random — a **shuffled bag**, so a player never
  gets four bombs in a row by luck. This is the difference between "hard" and "unfair", and
  it is the single most-felt tuning decision in the whole feature.
- Drop *rate* climbs with level (you asked for more power-ups AND more bombs later). Both
  climb; the 1:4 ratio holds.
- Items fall at a constant speed, are caught by the paddle, and are **missed harmlessly** if
  they reach the bottom.
- ⚠️ **An uncaught item must never be mistaken for a ball.** Different shape, different
  motion, no trail. A player who dives for a bomb thinking it is the ball will call it a bug.

### Magnet — the rule that keeps it honest

Active for 20 s. While active, a ball touching the paddle **sticks**. Hold Space to keep it;
release to fire it off at the aim you have lined up. **Auto-releases after 3 s** regardless.

The auto-release is not a limitation, it is the feature. Without it, the optimal play at
level 9 is catch-aim-release on every single bounce, which is slower and strictly better —
it removes the skill from the game and replaces it with patience.

⚠️ **Space is already the launch key.** In `Phase::Ready` Space launches; while magnet-held,
Space-release fires. One key, two meanings, disambiguated by phase — that is fine, but it
must be *tested*, because a `KeyDown(Space)` arriving while a ball is held must not also
re-launch something.

### Grow and bomb — the same axis

Both scale `paddle.w`. They must be one system, not two, or they will contradict each other:

- One `paddle_scale` value, driven by whichever effect is active.
- **A bomb caught while grown cancels the grow** rather than stacking into a
  double-negative. Simplest rule to explain and to feel.
- Grow stacks its *duration*, not its size: catching a second grow extends the timer.
- ⚠️ **The paddle must never grow past the field or shrink below the ball's diameter.**
  Clamp both ends and test both.
- ⚠️ `bounce_off_paddle` divides by `paddle.w / 2.0`. A narrow paddle makes steering
  *more* sensitive, which is a nice emergent difficulty — but check it does not push
  outgoing angles into the `MIN_VERTICAL_FRACTION` clamp so hard that the bomb makes the
  ball behave strangely rather than just making the paddle small.

---

## 5. Scoring

Score persists across levels and is what gets banked. The current rule — 10 per brick — does
not survive ten levels; a L10 armoured brick is 4x the work of a L1 plain one.

| Event | Points |
|---|---|
| Plain brick | 10 |
| Reinforced brick destroyed | 30 |
| Armoured brick destroyed | 80 |
| Power-up caught | 50 |
| Level cleared | 500 × level |
| Life remaining at game end | 1000 each |

Level-clear bonus scales with level, so reaching L10 is worth far more than grinding L1 —
which matters because the marquee shows one number and that number should mean "how far did
you get", not "how long did you play".

⚠️ **Do not pay per-hit on multi-hit bricks.** Paying 10 per hit makes an armoured brick
worth 40 and quietly makes them the best value on the board. Pay on destruction only.

`difficulty` field in the score file: **`"pixel-break"`** — one tier, all ten levels, one
comparable table. (Omaprix uses that slot for its track; Pong uses it for easy/normal/hard.
Precedent is per-game.)

---

## 6. Look and feel

This is the section that decides whether the release lands. Retro shape, modern light.

### Core gains (settled: add the primitives)

These serve every future title, which is the suite thesis paying off:

1. **`Canvas::fill_rect_add`** — additive blend. Glow that brightens whatever is under it
   instead of alpha-blending toward a colour. This is what makes a pixel effect read as
   *light* rather than as a translucent sticker. ⚠️ Must saturate per-channel, not wrap.
2. **A fixed-pool particle system** in core. Pre-allocated, no per-frame allocation, `Copy`
   particles, one `update(dt)` and one `draw`. Sized once. Omaprix wants this too — its
   crash is currently bespoke.
3. **`Canvas::fill_circle_f`** — optional, only if the particle work actually wants round
   sparks. Squares may well look *more* correct here. Decide by looking, not up front.

⚠️ Every one of these is a change to a crate two other shipped games depend on. Core changes
land in their own commit, with their own tests, and the racer and Pong must still build and
pass before the game work starts on top.

### The effects

**Brick shatter.** A destroyed brick throws chips — small rects in the brick's own colour,
with gravity, spinning off in the ball's direction of travel. Not a puff of generic dust:
the chips are *made of the brick*, which is what sells it.

**Damage that reads.** A reinforced brick's first hit visibly breaks a corner off — the
brick is drawn smaller and off-centre, with chips already gone. An armoured brick loses a
quarter of itself per hit. **This is the same system as the shatter**, just partial, and
doing it that way is what keeps it cheap.

**Screen shake.** Small, on armoured breaks and on losing a ball. A viewport offset —
`Viewport` already centralises every coordinate, so shake is a handful of lines there and
nothing else in `render.rs` has to know. ⚠️ Keep it under ~4 units and decay it fast; shake
is the effect most likely to be *too much*, and it makes people motion-sick before they can
say why.

**Speed-reactive trail.** The trail exists and is good. Make its length and brightness track
ball speed, so L10 looks fast. With additive blending it becomes a streak of light.

**Power-up glow.** Falling items pulse. The Omarchy logo item is the one that should look
genuinely special — it is the reward, and it carries the distro's name.

**Level-clear cascade.** On the last brick, everything left on screen resolves into a wave.
Brief — under a second — then the next level builds itself in, row by row, rather than
appearing. Builds are cheap and they make ten levels feel like a journey.

**Theme.** Everything above draws from `Theme`. The suite's whole visual argument is that it
looks like *your* desktop. A hardcoded colour anywhere in this section is a bug.

### The Omarchy logo item — ✅ DONE (S8, 636155f)

⚠️ **Both warnings below were resolved, and the first one was simply wrong.**

The premise was that the 5x7 font cannot draw "Omarchy" in logo script, so the mark would
need authoring as pixel art in `tools/sprite-playground.html` — a real session's work. The
logo Brian supplied made that unnecessary: `docs/omarchy-logo-hackerman.svg` is a 1200-unit
square whose every path coordinate is a multiple of 80, i.e. **a native 15x15 grid**. At
15x15 its alpha is only 0 or 255; at 600px all 225 cells are uniformly full or empty. The
art is *read off* the source, never redrawn. See `games/omarcade-pixel-break/src/art.rs`.

⚠️ **Licensing: asked and cleared.** Brian confirmed the community was asked before the mark
shipped as a game item.

⚠️ **What only rendering caught:** the mark's ink is its walls, one cell wide, which on a
16-unit-tall item lands on ~1 screen pixel. Drawn as-is the glyph collapsed into faint
scratches around a large body-coloured middle — a picture frame, not a logo. It is drawn
**inverted** (`art::inverted_rows`): same grid, same silhouette, but the glyph's mass carries
it rather than its linework. The 15x15 art is the source of truth; the inverse is derived.

⚠️ **The limit, stated honestly:** at 16 pixels the mark reads as a distinctive solid badge —
the only square among wide bars — not as a legible Omarchy wordmark. The one-row notch in the
left wall does not survive the 15-into-16 rounding. Enlarging the item box would fix it but
would move `Rect::from_center` and change the hitbox, which is a gameplay edit; that was
considered and declined.

---

## 7. Sound

Pixel Break has the audio seam and no sounds. It should ship with sound; a silent game next
to Omaprix will read as broken.

The method is proven seven times over and is not negotiable:
**build a playground with a scope, let Brian tune it, port the settings.**

Candidate list — most are one-shots, which is the easy case:
paddle hit (pitch rises with rally length), plain brick, reinforced brick (two distinct
sounds: chip and break), armoured break (the heavy one), power-up catch, bomb catch (must
sound *bad* — this is the one place the sound has to carry meaning the visual might miss),
magnet catch and release, ball lost, level clear, game over.

⚠️ **The lesson that cost the most on Omaprix applies here directly:** a one-shot must be
sized against whatever it plays over, not against silence. Pixel Break's mix is mostly
silence between hits, but at ten balls in play there is a *lot* going on — the same trap in
a new shape. `every_sound_can_be_heard_at_ten_balls` is a real test.

⚠️ **No HUD volume indicator exists in any game.** The handoff calls this the most likely
next audio complaint — M mutes silently and reads as a crash. Pixel Break is a new HUD
anyway (§8); it should be the game that finally shows it, and the pattern then goes back to
the other two.

---

## 8. HUD and screens

```
SCORE 12,480          LEVEL 4/10          LIVES 3
                                          [====   ] MAGNET
```

⚠️ **The font has no comma and no slash-safe layout.** `text_width` covers `A-Z0-9 -`; the
render tests already guard this and will catch it, which is exactly why they exist. Either
add glyphs to core (comma, slash — cheap, and every game benefits) or format without them.
Adding them is better; `LEVEL 4/10` is worth a glyph.

New screens: title (with the logo sprite), level intro, level clear, game complete (a real
ending — someone who clears ten levels deserves more than "YOU WIN"), game over.

---

## 9. How we build it

Ten sessions, each ending green, tested and pushed. Every one is drivable — you play it
before we move on, because every real finding in this project so far has come from you
playing, never from a test.

| # | Session | Lands |
|---|---|---|
| **S1** | **The rename** | Crate, ids, desktop file, install.sh migration. Zero gameplay change. Cabinet still lists it, old score migrated to `legacy`. |
| **S2** | **Core primitives** | Additive blend, particle pool. Core tests. Racer and Pong still green. No Pixel Break change yet. |
| **S3** | **Multi-ball** | `balls: Vec<Ball>`, per-ball trails, lives-vs-balls rule. Ball cap. The riskiest simulation change — it gets a session alone. |
| **S4** | **Levels and tiers** | 10 levels, three brick tiers, the curve, speed ramp with the tunnelling guard. |
| **S5** | **Power-ups: drop and catch** | Falling items, the shuffled bag, catch detection. Grow/bomb on one axis. |
| **S6** | **Magnet and Omarchy** | The stick/aim/release mechanic and the extra-ball item. Both need their own session — they are the two that change how the game is *played*. |
| **S7** | **Shatter and damage** | Chips, partial damage, screen shake. The visual payoff for S2. |
| **S8** | **The logo sprite + item art** | Authored in the sprite playground. Licence question asked before this lands. |
| **S9** | **Sound** | Playground first, then port. The full one-shot set. |
| **S10** | **HUD, screens, scoring, volume indicator** | The frame around it. Then a full ten-level playthrough as the acceptance test. |

**Tuning is not a session.** It is continuous, and it needs a probe. Build
`probe_balance` early (S4) — it plays levels headlessly and reports hits-to-clear, expected
drop counts, and time-to-clear per level. Your numbers in §4 are guesses; this is what turns
them into measurements. The `simulate` example already establishes the pattern.

**Per-file brief before writing, every time.** Path, what it does, key methods, why, what to
watch for. That is how you read this codebase and it has not failed yet.

---

## 10. Risks, honestly

1. **Multi-ball is the real risk.** Every collision routine in `physics.rs` assumes one ball.
   The failure mode is subtle — a ball that occasionally reflects wrong, in a way that looks
   like bad luck rather than a bug. S3 is alone for this reason, and the guards listed in §2
   are the minimum.
2. **Scope.** Ten sessions is the honest estimate, and it is longer than the last three games
   took each. Every one is shippable on its own — if we stop at S7 there is a real, finished,
   better game. Nothing here is load-bearing for anything after it except S2 → S7.
3. **Core changes touch two shipped games.** S2 lands alone, with the racer and Pong green,
   before anything is built on it.
4. **The Omarchy wordmark.** §6. Ask before building on it.
5. **Difficulty.** L10 at **180** hits with three lives may simply be too hard, and we will
   not know until it is played. `probe_balance` gives us numbers, but your hands give the
   verdict. (The 180 is corrected from 156 — see §3.)
6. **Competitor census — still open, now partly answered.** A search for an Omarchy
   brick-breaker turns up nothing. That is *no evidence of one*, not *proof of none*: web
   search is poor at surfacing week-old repos, which is exactly what this ecosystem is made
   of. Before title four, check the plugin lists and the community directly, not a search
   engine.

---

## 11. Not in this release

Deliberately cut, so the release actually ships: boss levels, level editor, two-player,
laser paddle, ball-through (the classic Arkanoid "break" powerup), per-level music,
leaderboard beyond the existing marquee, gamepad.

Cut is not "never". They are written down so we can stop arguing with ourselves about them.
