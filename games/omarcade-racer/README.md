# The racer

A pseudo-3D driving game in the Pole Position lineage: a scanline road, sprites
scaled by distance, traffic to overtake. The *technique* is borrowed and openly
so; none of the art is.

**Status: art, rendering and the road model. No game yet.** There is no physics,
no input, no collision, no scoring. What exists is the car, the traffic, the
machinery that draws them convincingly, and the track they sit on. This document
is the map of that machinery and, more usefully, the record of *why* each piece
is shaped the way it is.

## The files

| File | What it holds |
|---|---|
| `src/art.rs` | The sprite grids as text, the palette recipes, and `Art::load` |
| `src/road.rs` | The track as segments, and the projection from track-z to screen |
| `src/main.rs` | Reports the sprite set. Exists so `art.rs`'s tests have a target to run in |
| `examples/dump_art.rs` | Renders the scenes below to PNG. The whole feedback loop |
| `examples/bench_sprites.rs` | What the real sprites cost at real resolution |
| `../../core/src/sprite.rs` | `Sprite`, `Pose`, `Roll` — art-as-data, and the transforms |
| `../../tools/sprite-playground.html` | Browser grid editor. Emits paste-ready Rust |

### Looking at it

```sh
cargo run -p omarcade-racer --example dump_art -- out.png sheet   # every sprite, several scales
cargo run -p omarcade-racer --example dump_art -- out.png road    # the cars in their real setting
cargo run -p omarcade-racer --example dump_art -- out.png curve   # a bend, which a straight road cannot prove
cargo run -p omarcade-racer --example dump_art -- out.png lean    # the pose range, three rows
cargo run -p omarcade-racer --example dump_art -- out.png roll    # consecutive frames of tread
cargo run --release -p omarcade-racer --example bench_sprites     # settles any "this is cheap" claim
```

`road` is the scene that decides things. On the sheet a car sits in isolation; on
the road it sits against grass, gets haze mixed over it, and is surrounded by
traffic, so contrast that reads in the sheet can wash out in context.

**Never screenshot a window for these.** The render is deterministic and the
window is not.

## Why the art is text

A sprite is a grid of characters plus a palette. `.` is transparent; every other
character indexes a colour.

The reason is the correction loop. Pixel art as data is reviewable row by row,
diffable, and correctable *in words* — "the rear wing is one row too high" is a
change anyone can make without opening an image editor, and a wrong pixel shows
up in the diff.

`Sprite::new` **panics** on a ragged row or an unpalletted character, deliberately.
The alternative — skipping quietly — is how a font once shipped "BEST" as "EST".

### Colours are recipes, not hex

Nearly every tone is *derived* from the live Omarchy theme:

```rust
let shadow   = body.lerp(theme.darker_background, 0.45);
let tyre     = theme.darker_background.lerp(Color::BLACK, 0.35);
let tyre_top = tyre.lerp(theme.foreground, 0.14);   // ← from another derived tone
```

That third line matters. Tones chain off other tones, which is *why* the four
tyre shades stay related to each other when the theme changes. A palette where
everything derived only from a theme slot would look the same at rest and fall
apart on a theme switch.

What is **pinned** is pinned on purpose: `body` and `accent` (a red car that turns
green with the desktop stops being a recognisable object) and `E`, the vent band
under the wing (a deep interior red that must hold across themes or the vent stops
reading as a recess).

There is **no palette size limit**. `Sprite::from_rows` resolves the palette once
at build time and never consults it again while drawing; cost scales with a
sprite's *ink*, not its palette. Roughly 93 characters are usable; 15 are in use.

### The playground

`tools/sprite-playground.html` authors in the same vocabulary the Rust uses —
base, mix-toward, amount — so what you pick translates back with no loss. It emits
the grid **and** its palette together, because they are the pair that has to stay
in sync: a grid carrying a letter the palette does not know is a `Sprite::new`
panic, and that is a failure worth catching while drawing rather than at
`cargo run`.

Paste a whole emitted block back into the tool to keep editing. A partial paste —
the `vec!` block without the `let` bindings above it — is reported rather than
silently ignored.

## Why the cars move without extra art

Three transforms, all arithmetic on where each source pixel lands. One grid covers
the whole range; there is no sprite-per-angle sheet and there should not be one.

```rust
Pose { lean, turn, squat }        // -1..1, -1..1, 0..1
Pose::cornering(t)                // all three from one number
car.draw_ground_posed(c, x, ground_y, scale, pose, tint);
```

- **`lean`** shears horizontally by height above the base. Wheels stay planted,
  body tilts over them. Shearing about the *base* rather than the centre is what
  makes it read as banking instead of sliding.
- **`turn`** squashes width, so the car reads as angled away from the camera.
- **`squat`** compresses height about the contact patch. Small, but it is what
  makes a lean read as weight rather than as a slider.

Measured at **0.95–0.97×** the cost of drawing plain — free, marginally cheaper,
because squashing covers fewer pixels.

**This deliberately does not rotate.** A car seen from behind and rotated shows
its *side*, and there is no side in a rear-view grid; no transform can invent one.
Squash is the honest approximation and it is what the arcade originals used. If
the cars ever need to genuinely turn, that is drawn art and a real decision, not a
tweak.

### Rolling tread

```rust
roll.advance(speed, pixels_per_unit, dt);
car.draw_ground_rolling(c, x, ground_y, scale, pose, roll.phase(), None);
```

Pixels are marked as tread **by character, never by colour** —
`Sprite::new_with_tread(grid, palette, &['R'])`. In this car `H` was doing two
jobs, tread on the wheels *and* highlights in the diffuser, so animating by colour
would have strobed the bodywork. `R` is the same colour as `H`; it exists only so
the animation has an unambiguous target.

Two properties worth knowing before changing anything here:

- The tread wraps inside the **wheel's own row span**, not the sprite's, so rubber
  never climbs out of the tyre onto the body.
- The rate is **capped** at `MAX_ROLL_PER_FRAME`. Past about one row per frame a
  scrolling pattern aliases and visually *reverses* — the wagon-wheel effect — and
  no amount of speed fixes it. It pins and reads as "very fast" instead.

`pixels_per_unit` is the tuning knob, and it is **not calibrated yet** because
there is no speed model to calibrate it against.

## The traffic

Rivals are the player's chassis in five liveries, written saturated and dulled at
load by mixing toward `theme.muted`. Derived rather than hand-picked so the field
stays theme-reactive and stays *behind* the player on any palette.

The player is the only fully saturated car on the road, and that is the whole
mechanism for finding your own car at a glance — at this screen size saturation
carries that read long before hue does. A test asserts every rival's chroma is
strictly below the player's, so the property cannot erode quietly.

**A known tradeoff, not an oversight:** an earlier decision held that rivals should
have a *simpler silhouette*, so "me" versus "traffic" reads by shape at speed.
Colour-only was chosen instead, and it is what Pole Position did. If it stops
working once the cars are actually moving, the fix is a simpler rival shape —
**not** a brighter player.

## The road

`src/road.rs` holds the track and the projection. It draws nothing — it answers
*where on screen is this point of track?*, and the renderer decides colours.

The direction matters. The old sketch walked screen rows and asked "how far is
this row?" (`z = 1/t`). That cannot become a game, because curvature is a
property of *track distance*, not of screen row — the same row is a different
piece of track next frame. So the model inverts it: segments sit at fixed
track-z and project *forward* to a screen-y.

```rust
let road = Road::straight(400);
let camera = Camera::for_road(&road, 0.85);   // 0.85 = fraction of screen the road fills
for band in road.visible(&camera, camera_z, x_offset, w, h) { … }
```

Three things here were got wrong first, found by *looking at the render*, and are
now pinned by tests that fail without the fix:

- **The camera is derived, not chosen.** The first version picked
  `height: 1000.0, fov: 0.5·π` and the road covered 110% of the screen — a
  featureless slab with the rumble strips off both edges. `Camera::for_road`
  solves the camera from the road's own dimensions and a fill fraction, so it
  holds at any road width and any resolution.
- **Curvature is not scaled by distance; steering is.** They are different
  quantities. A steering offset is a fixed lateral distance and must shrink with
  distance — real perspective. Accumulated curve already grows with distance by
  construction, so scaling it again cancels the growth exactly, and *every bend
  rendered as a straight road*. The unit tests passed the whole time, because
  they checked the accumulator and never checked the screen.
- **A curve value is a ratio.** Raw double integration grows as n² — at 100
  segments a curve of 2.0 accumulated to 10,100, putting the road four million
  pixels off centre. Normalised over the draw distance, `curve: 1.0` now means
  "displace the road by half a screen over the full visible distance", which is
  a number a track author can actually pick.

Bands are drawn by **interpolating across each band per scanline**, not as one
rect. Near the camera a single segment is ~180px tall (measured), so one rect
per band is the "banded" approach `bench_road` warned about, and the edges
stair-step in slabs. The model costs 0.15ms/frame — 0.9% of a 60fps budget, and
curvature costs nothing extra.

Hills are deliberately not built. `Segment` carries a `pitch` field that stays
0.0, so adding them later is a change to `project` and not a migration of every
track authored by then.

## What is not built

- Physics, input, collision, lap timing, scoring.
- A name. "Pole Position" is Namco's; ours is unchosen.
- Sound.

## If you change something here

Read `~/projects/LESSONS.md` first — L016 through L020 were all earned in this
file and the one next to it. The short version:

- Run generated code; do not read it and assume it compiles.
- Ask what code change would make a new test fail. If nothing would, the fixture
  is wrong.
- A tuning constant is a ratio or an absolute. Absolutes calibrated at one sprite
  size break at every other.
- When a debug scene looks wrong, measure the scene before suspecting the maths.
