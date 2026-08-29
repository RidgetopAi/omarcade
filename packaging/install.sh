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

GAMES=(omarcade-breakout omarcade-pong)

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
