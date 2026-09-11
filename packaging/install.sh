#!/usr/bin/env bash
# Install Omarcade for the current user. No root, no system directories.
#
#   ./packaging/install.sh            build and install
#   ./packaging/install.sh --uninstall  remove everything this installed
#
# Idempotent: re-running upgrades in place.

set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
BINDIR="${XDG_BIN_HOME:-$HOME/.local/bin}"
APPDIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
ICONDIR="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps"

# Every shipped title, by binary name. This list is the ONE place a new
# game is registered: the cabinet and the marquee discover games from
# what is installed and what has written a score, never from a list of
# their own.
GAMES=(omarcade-pixel-break omarcade-volley omarcade-racer)

die() { echo "install.sh: $*" >&2; exit 1; }
say() { printf '  %s\n' "$*"; }

if [[ ${1:-} == --uninstall ]]; then
  echo "Removing Omarcade..."
  for game in "${GAMES[@]}"; do
    rm -f "$BINDIR/$game" && say "removed $BINDIR/$game"
  done
  for game in "${GAMES[@]}"; do
    rm -f "$APPDIR/$game.desktop" && say "removed $APPDIR/$game.desktop"
  done
  # The pre-suite entry, from when Breakout was the only title.
  rm -f "$APPDIR/omarcade.desktop"
  rm -f "$ICONDIR/omarcade.svg"    && say "removed $ICONDIR/omarcade.svg"
  command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q "$APPDIR" || true
  echo "Done. (Hyprland rules in ~/.config/hypr/omarcade.lua were left alone.)"
  exit 0
fi

command -v cargo >/dev/null 2>&1 || die "cargo not found. Install Rust: https://rustup.rs"

# --- Breakout -> Pixel Break -------------------------------------------------
# Session 14 renamed the title. GAME_ID names the score file AND the cabinet
# discovers games by scanning $BINDIR, so a rename with no migration does not
# error and does not vanish: it leaves a permanent, unplayable "Breakout" on
# the cabinet next to the game that replaced it.
#
# Carried entries are stamped difficulty "legacy". Old scores came from a
# single 60-brick field; a Pixel Break run is ten levels with tiered bricks and
# level bonuses. ScoreFile groups by difficulty, so the history survives without
# ever being ranked against a run of the game it is not.
#
# Runs at most once: it acts only when the old file exists and the new one does
# not, so it can never overwrite a newer file with an older one.
SCOREDIR="${XDG_STATE_HOME:-$HOME/.local/state}/omarcade/scores"
old_scores="$SCOREDIR/omarcade-breakout.json"
new_scores="$SCOREDIR/omarcade-pixel-break.json"
if [[ -f $old_scores && ! -f $new_scores ]]; then
  mv "$old_scores" "$new_scores"
  sed -i \
    -e 's|"id": *"omarcade-breakout"|"id": "omarcade-pixel-break"|' \
    -e 's|"name": *"Breakout"|"name": "Pixel Break"|' \
    -e 's|"difficulty": *"[^"]*"|"difficulty": "legacy"|' \
    "$new_scores"
  # A v1 entry may carry no difficulty field at all (ScoreFile predates it).
  # Give those one, so nothing carried lands in the current table.
  sed -i -E '/"at": *"[^"]*"$/ s|$|,\n      "difficulty": "legacy"|' "$new_scores"
  say "migrated Breakout scores -> $new_scores (entries marked legacy)"
fi

# --- Retired binary names ----------------------------------------------------
# Every id this suite has shipped under and then renamed away from.
#
# A left-behind binary and launcher entry are a second game on the cabinet and
# a second entry in the app menu -- both launching a build that no longer
# matches the game it claims to be. The uninstall loop cannot reach them: it
# iterates GAMES, which by definition names only current titles. Same reason as
# omarcade.desktop below.
#
# This is a LIST because it is now the second rename rather than the first, and
# the check that catches a missed one (find ~/.local/bin ~/.local/share/
# applications ~/.local/state/omarcade -iname "*<oldname>*") only ever runs on
# the machine doing the renaming. On everyone else's it is this loop or nothing.
#
# ⚠️ SCORE FILES ARE NOT LISTED HERE. Breakout's are migrated above because that
# rename predates the support; Pong's are carried by ScoreFile::load_or_migrate
# in core, which is typed, tested, and runs however the game was installed. Two
# migrations racing for one file is a bug waiting to happen -- if a future
# rename needs one, add it in core, not here.
RETIRED=(omarcade-breakout omarcade-pong)
for old in "${RETIRED[@]}"; do
  [[ -e $BINDIR/$old ]] && say "removing retired $BINDIR/$old"
  rm -f "$BINDIR/$old"
  [[ -e $APPDIR/$old.desktop ]] && say "removing retired $APPDIR/$old.desktop"
  rm -f "$APPDIR/$old.desktop"
done

echo "Building Omarcade (release)..."
# Build one -p flag per game as separate argv entries. A pattern-substitution
# expansion here would collapse "-p name" into a single argument.
cargo_args=()
for game in "${GAMES[@]}"; do
  cargo_args+=(-p "$game")
done
cargo build --release --manifest-path "$REPO_ROOT/Cargo.toml" "${cargo_args[@]}"

mkdir -p "$BINDIR" "$APPDIR" "$ICONDIR"

echo "Installing..."
for game in "${GAMES[@]}"; do
  built="$REPO_ROOT/target/release/$game"
  # Fail loudly rather than installing a stale binary from a previous build.
  [[ -x $built ]] || die "expected binary not found after build: $built"
  install -m 755 "$built" "$BINDIR/$game"
  say "$BINDIR/$game"
done

# One launcher entry per game. @BINDIR@ is a placeholder because
# .desktop Exec= needs an absolute path and expands neither ~ nor $HOME.
for game in "${GAMES[@]}"; do
  src="$REPO_ROOT/packaging/$game.desktop"
  [[ -f $src ]] || die "no launcher entry for $game: $src"
  sed "s|@BINDIR@|$BINDIR|g" "$src" > "$APPDIR/$game.desktop"
  chmod 644 "$APPDIR/$game.desktop"
  say "$APPDIR/$game.desktop"
done

# Session 3 installed a single 'omarcade.desktop' back when Breakout was
# the only title. Left behind it becomes a duplicate Breakout entry in
# the app menu, so an upgrade clears it.
rm -f "$APPDIR/omarcade.desktop"

install -m 644 "$REPO_ROOT/packaging/omarcade.svg" "$ICONDIR/omarcade.svg"
say "$ICONDIR/omarcade.svg"

command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q "$APPDIR" || true
command -v gtk-update-icon-cache   >/dev/null 2>&1 && gtk-update-icon-cache -qtf "${ICONDIR%/scalable/apps}" 2>/dev/null || true

echo
echo "Installed ${#GAMES[@]} games. Launch from your app menu, or run: ${GAMES[*]}"
case ":$PATH:" in
  *":$BINDIR:"*) ;;
  *) echo "NOTE: $BINDIR is not on your PATH; the app-menu entry still works." ;;
esac
echo
echo "Optional -- Hyprland window rules (float, center, full opacity):"
echo "  cp packaging/hyprland/omarcade.lua ~/.config/hypr/omarcade.lua"
echo "  echo 'require(\"hypr.omarcade\")' >> ~/.config/hypr/hyprland.lua"
