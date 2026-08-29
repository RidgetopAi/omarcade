# Omarcade

Original retro arcade games, native to [Omarchy](https://omarchy.org/).

Not emulation and not clones of anyone's ROMs — small games written from
scratch in Rust, drawing straight into a pixel buffer, that read your active
Omarchy theme and idle at roughly nothing when you're not playing.

**Status:** early, but real. Two games (Breakout and Pong) are playable and
tested, the shared engine underneath them is the actual work, and the
cross-game marquee is live in the Omarchy bar.

![Omarcade Breakout](docs/breakout.png)

*Breakout, drawing itself in the active Omarchy theme.*

![Omarcade Pong](docs/pong.png)

*Pong, mid-rally. Both images were produced by the games' own headless
renderers, not screenshotted from a window.*

---

## Install

Requires [Rust](https://rustup.rs) and a Wayland compositor (Hyprland on
Omarchy).

```bash
git clone https://github.com/RidgetopAi/omarcade.git
cd omarcade
./packaging/install.sh
```

That builds the release binaries, installs them to `~/.local/bin`, and adds
a desktop entry per game so they show up in your app launcher. No root, no
system directories. `./packaging/install.sh --uninstall` removes them again.

### Hyprland window rules (recommended)

Omarcade renders a fixed 4:3 canvas, so a tiling layout stretches the window
and grows the letterbox bars. These rules float it at its native size and
opt out of Omarchy's default translucency, which would otherwise wash out
the theme colours:

```bash
cp packaging/hyprland/omarcade.lua ~/.config/hypr/omarcade.lua
echo 'require("hypr.omarcade")' >> ~/.config/hypr/hyprland.lua
```

---

## Breakout

```bash
omarcade-breakout
```

| Key | Action |
| --- | --- |
| `←` `→` or `A` `D` | Move the paddle |
| `Space` | Launch the ball / start |
| `Enter` | Play again, once you've won or lost |
| `Esc` | Quit |

Three lives, sixty bricks. The ball's angle depends on where it hits the
paddle, so you steer it rather than just blocking it.

---

## Pong

```bash
omarcade-pong
```

| Key | Action |
| --- | --- |
| `↑` `↓` | Move the paddle / choose a difficulty |
| `Space` | Start / serve |
| `Enter` | Play again, once the match is over |
| `Esc` | Quit |

First to eleven. Three difficulties, and they change what you can see: a
smaller paddle and a faster ball, not a hidden number. The rally itself
speeds up the longer it runs.

The opponent predicts where the ball will arrive — simulating it forward
through wall bounces — and then gets that prediction wrong on purpose. It
aims off, it only re-decides every so often, and on the lower settings it
cannot read a shot that banks twice. The point is that it misses because it
**committed to the wrong place**, not because it was too slow, so a miss is
something you can watch happen and feel you caused.

How good it actually is, measured over sixty matches per cell against
scripted players of a fixed standard:

| | vs poor | vs fair | vs good |
| --- | --- | --- | --- |
| **Easy** | 97% | 15% | 0% |
| **Normal** | 100% | 65% | 0% |
| **Hard** | 100% | 97% | 25% |

Run it yourself: `cargo run --release -p omarcade-pong --example probe_ai`.

---

## Why a suite

Most small arcade projects ship one game in one repo and stop. Omarcade is
built the other way round: a shared engine (`omarcade-core`) with games as
thin crates on top, and — once it lands — a **marquee** in the Omarchy bar
showing high scores across every title.

That's the part nobody else in the ecosystem has. Theme-reactivity and
native Wayland are table stakes here; a coherent suite is not.

---

## Architecture

Games never touch Wayland, GPU, or windowing types. They see two things:

```rust
trait Game {
    fn on_input(&mut self, event: InputEvent) -> bool;
    fn update(&mut self, dt: f32);
    fn render(&mut self, canvas: &mut Canvas<'_>);
}
```

...and a `Canvas` to draw into. The backend behind that seam is swappable —
today it's winit + softbuffer; a layer-shell backend (games rendered onto
the desktop surface, behind your windows) is the planned deep dive.

The seam is enforced by the build, not by discipline: a game crate that
tries `use winit::window::Window;` fails to compile.

```
core/src/backend/mod.rs         the seam: Canvas, Color, Key, InputEvent, Game, Backend
core/src/backend/winit_soft.rs  winit + softbuffer implementation
core/src/geom.rs                Vec2, Rect, AABB overlap, collision axis
core/src/ease.rs                easing curves, lerp, Decay
core/src/scores.rs              the score contract the marquee reads
core/src/theme.rs               reads your live Omarchy palette, always falls back
games/omarcade-breakout/src/
  state.rs     world model, no behaviour
  physics.rs   fixed 240Hz timestep + collision — the only file that advances time
  render.rs    letterboxed viewport, bitmap-font HUD
  main.rs      wiring
games/omarcade-pong/src/
  state.rs     world model, difficulty tiers, no behaviour
  physics.rs   fixed 240Hz timestep, paddle steering, the rally ramp
  ai.rs        the opponent: predict, then get it wrong on purpose
  render.rs    letterboxed viewport, centre net, difficulty select
  main.rs      wiring
Marquee.qml / Cabinet.qml / ScoreRecord.qml    the bar widget and picker
```

`geom.rs` started inside the Breakout crate. Pong needed every line of it
unchanged, which is the test a shared crate has to pass to deserve
existing — so it moved to `core` rather than being copied.

Each game is its own process and its own window. That's forced rather than
chosen: Quickshell can't embed an external Wayland surface, so a game can't
live inside the bar — only the marquee can.

---

## Development

```bash
cargo test --workspace     # 67 tests
cargo clippy --workspace
cargo run -p omarcade-breakout
```

The game is also driven headlessly, which is how it gets tested without a
compositor. `Canvas` wraps any `&mut [u32]`, so the same rendering and
physics run against a plain buffer:

```bash
# Simulate 200,000 physics ticks and report the outcome
cargo run -p omarcade-breakout --example simulate -- 200000

# Adversarial collision probes
cargo run -p omarcade-breakout --example probe_tunnel
cargo run -p omarcade-breakout --example probe_shallow

# Measure how good the Pong opponent actually is, per difficulty
cargo run --release -p omarcade-pong --example probe_ai

# Render any game state straight to a PNG
cargo run -p omarcade-breakout --example dump_frame -- midgame out.png
#   scenes: ready | playing | midgame | won | lost
cargo run -p omarcade-pong --example dump_frame -- rally out.png
#   scenes: select | serve | rally | matchpoint | won | lost
```

These are deterministic and run in milliseconds. They're the reason
gameplay changes can be verified without opening a window.

---

## Licence

MIT. See [LICENSE](LICENSE).
