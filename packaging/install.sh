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

# The bar plugin. Defined up here rather than beside the install step
# because BOTH paths need it: an uninstall that cannot see this variable
# is an uninstall that leaves the plugin behind, which is exactly the bug
# this placement fixes.
PLUGIN_ID="ridgetopai.omarcade"
PLUGIN_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/omarchy/plugins/$PLUGIN_ID"

# Every shipped title, by binary name. This list is the ONE place a new
# game is registered: the cabinet and the marquee discover games from
# what is installed and what has written a score, never from a list of
# their own.
GAMES=(omarcade-pixel-break omarcade-volley omarcade-racer)

# Every id this suite has shipped under and then renamed away from.
#
# Declared beside GAMES, and above the uninstall branch, because BOTH
# paths iterate it: install removes a retired name so it cannot haunt the
# cabinet, and uninstall removes it so the suite takes every name it has
# ever used with it. A name that only the install path knows about is a
# name the uninstall path leaves behind.
#
# ⚠️ SCORE FILES ARE NOT LISTED HERE. Breakout's are migrated below
# because that rename predates the support; Pong's are carried by
# ScoreFile::load_or_migrate in core, which is typed, tested, and runs
# however the game was installed. Two migrations racing for one file is a
# bug waiting to happen — if a future rename needs one, add it in core.
RETIRED=(omarcade-breakout omarcade-pong)

die() { echo "install.sh: $*" >&2; exit 1; }
say() { printf '  %s\n' "$*"; }

# Remove a file and report it ONLY if it was actually there.
#
# ⚠️ `rm -f X && say "removed X"` does NOT do this: rm -f exits 0 on a
# missing file, so the report fires either way and an uninstall of a
# partial install cheerfully lists things that never existed. Uninstall
# is the moment a wary user is reading most closely; it must not lie.
drop() {
  [[ -e $1 || -L $1 ]] || return 0
  rm -f "$1" && say "removed $1"
}

if [[ ${1:-} == --uninstall ]]; then
  echo "Removing Omarcade..."
  for game in "${GAMES[@]}"; do
    drop "$BINDIR/$game"
  done
  for game in "${GAMES[@]}"; do
    drop "$APPDIR/$game.desktop"
  done
  # Retired binaries and launchers, so removing the suite removes every
  # name it has ever shipped under rather than only the current ones.
  for old in "${RETIRED[@]}"; do
    drop "$BINDIR/$old"
    drop "$APPDIR/$old.desktop"
  done
  # The pre-suite entry, from when Breakout was the only title.
  drop "$APPDIR/omarcade.desktop"
  drop "$ICONDIR/omarcade.svg"

  # ---------------------------------------------------------------------
  # The bar plugin.
  #
  # ⚠️ THIS STEP DID NOT EXIST UNTIL THE SHIP REVIEW, and its absence was
  # the one bug in this script that reached the user. The install path
  # grew a plugin step; the uninstall path did not, so removing Omarcade
  # left the marquee and cabinet installed in the bar. It did not even
  # error — the cabinet shows "No games found" — which is worse than a
  # failure: it is permanent, polite litter in the bar of someone who has
  # already decided they do not want this.
  #
  # rm -rf is safe on exactly this path and no other: the directory is
  # namespaced to our plugin id and we own every file in it. The parent
  # plugins/ directory belongs to Omarchy and may house other people's
  # plugins, so it is only removed when empty, and never forced.
  # ---------------------------------------------------------------------
  if [[ -d $PLUGIN_DIR ]]; then
    rm -rf "$PLUGIN_DIR"
    say "removed $PLUGIN_DIR"
    rmdir "$(dirname "$PLUGIN_DIR")" 2>/dev/null || true
  fi

  command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q "$APPDIR" || true
  command -v gtk-update-icon-cache   >/dev/null 2>&1 && gtk-update-icon-cache -qtf "${ICONDIR%/scalable/apps}" 2>/dev/null || true
  echo
  echo "Done. Two things were deliberately left alone:"
  echo "  - your high scores, in ${XDG_STATE_HOME:-\$HOME/.local/state}/omarcade/"
  echo "  - Hyprland rules, if you added them to ~/.config/hypr/omarcade.lua"
  echo "The bar needs a restart to drop the widget: omarchy-restart-shell"
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
# Clear any name this suite has shipped under before (the list itself is
# declared at the top, beside GAMES, because uninstall iterates it too).
#
# A left-behind binary and launcher entry are a second game on the cabinet and
# a second entry in the app menu -- both launching a build that no longer
# matches the game it claims to be.
#
# The check that catches a name missed from the list (find ~/.local/bin
# ~/.local/share/applications ~/.local/state/omarcade -iname "*<oldname>*")
# only ever runs on the machine doing the renaming. On everyone else's it is
# this loop or nothing.
for old in "${RETIRED[@]}"; do
  drop "$BINDIR/$old"
  drop "$APPDIR/$old.desktop"
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

# ---------------------------------------------------------------------
# The bar plugin: the marquee and the cabinet.
#
# ⚠️ THIS STEP DID NOT EXIST UNTIL NOW. The plugin directory was created
# by hand in August and then drifted: every QML change since lived only
# in the repo, so the running shell was months behind the source and no
# amount of reinstalling the games would have fixed it. A plugin that
# cannot be installed by the installer is not shippable.
#
# Idempotent like the rest, and L036-clean: the QML is REPLACED rather
# than merged, so a file deleted from the repo does not survive in the
# installed copy.
# ---------------------------------------------------------------------
# ⚠️ ARE WE ALREADY *INSIDE* THE PLUGIN DIRECTORY?
#
# `omarchy plugin add` clones this repo straight into
# ~/.config/omarchy/plugins/ridgetopai.omarcade — which is $PLUGIN_DIR.
# So on the marketplace path the source and the destination are THE SAME
# DIRECTORY, and the copy below is not a copy at all:
#
#   rm -f "$PLUGIN_DIR"/*.qml   deletes Cabinet.qml, Marquee.qml and
#                               ScoreRecord.qml — the running plugin
#   install "$REPO_ROOT/$f" …   then fails, "are the same file"
#
# Measured, not guessed: cloning into a plugin dir and running this left
# `git status` showing ` D Cabinet.qml  D Marquee.qml  D ScoreRecord.qml`
# and no QML on disk. The cabinet deletes itself halfway through its own
# install, and every marketplace user takes this path.
#
# Nothing needs doing in that case: the files are already exactly where
# they belong, because git put them there.
if [[ $REPO_ROOT -ef $PLUGIN_DIR ]]; then
  say "plugin already in place (running from $PLUGIN_DIR)"
else
  mkdir -p "$PLUGIN_DIR"
  # Clear the QML the plugin owns before writing it, so a renamed or
  # removed component cannot linger and shadow its replacement.
  rm -f "$PLUGIN_DIR"/*.qml
  for f in manifest.json Marquee.qml Cabinet.qml ScoreRecord.qml; do
    [[ -f $REPO_ROOT/$f ]] || die "missing plugin file: $REPO_ROOT/$f"
    install -m 644 "$REPO_ROOT/$f" "$PLUGIN_DIR/$f"
    say "$PLUGIN_DIR/$f"
  done

  # The cabinet's game art. Generated by tools/make-cabinet-art.sh and
  # committed, so a clone has it without needing a Rust toolchain run.
  if [[ -d $REPO_ROOT/docs/cabinet ]]; then
    rm -rf "$PLUGIN_DIR/docs/cabinet"
    mkdir -p "$PLUGIN_DIR/docs/cabinet"
    for art in "$REPO_ROOT"/docs/cabinet/*.png; do
      [[ -f $art ]] || continue
      install -m 644 "$art" "$PLUGIN_DIR/docs/cabinet/$(basename "$art")"
      say "$PLUGIN_DIR/docs/cabinet/$(basename "$art")"
    done
  fi
fi

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
