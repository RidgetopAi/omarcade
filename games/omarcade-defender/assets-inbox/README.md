# Asset inbox

Where playground exports land before they are wired into the game.

## How to hand one over

1. Draw it in `tools/vector-playground.html`.
2. Name it in the Export panel — **the name matters**, it becomes the Rust
   const prefix. `LANDER`, `HUMANOID`, `MUTANT`, and so on.
3. Press **copy**.
4. Paste into a file here named after the thing: `lander.rs`, `humanoid.rs`.
5. Say it is ready.

Saving as `.rs` rather than `.txt` because the export is already valid
Rust — nothing has to be retyped or unmangled, and an editor will syntax
highlight it so a truncated paste is visible immediately.

## What happens next

The art moves into `src/art.rs` beside the ship, with three things checked
first, because all three have already bitten once:

- **Pieces that float free.** The ship's gun sat below the hull in empty
  space. Anything meant to be attached must OVERLAP what it attaches to.
- **Outlines that cross themselves.** An even-odd fill renders the crossed
  part HOLLOW, so a self-crossing shape comes out as a smear. The
  playground will draw it the same way, so it is visible there too.
- **Detail too small to survive.** Below about 2 square units a piece is
  a few pixels at game scale. Check it in the playground's preview at the
  size it will actually be drawn, not on the editor grid.

## Sizes

The ship is ~34 units nose to tail and draws at `SCALE = 2.0`, about 68px
on a 960x720 screen. Enemies should be authored in the same units so they
sit at a sensible size beside it. Defender's landers were roughly a third
of the ship's width; humanoids smaller still.
