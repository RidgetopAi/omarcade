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

GAMES=(omarcade-breakout)

die() { echo "install.sh: $*" >&2; exit 1; }
say() { printf '  %s\n' "$*"; }

if [[ ${1:-} == --uninstall ]]; then
  echo "Removing Omarcade..."
  for game in "${GAMES[@]}"; do
    rm -f "$BINDIR/$game" && say "removed $BINDIR/$game"
  done
  rm -f "$APPDIR/omarcade.desktop" && say "removed $APPDIR/omarcade.desktop"
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

# @BINDIR@ is a placeholder because .desktop Exec= needs an absolute path
# and does not expand ~ or $HOME.
sed "s|@BINDIR@|$BINDIR|g" "$REPO_ROOT/packaging/omarcade.desktop" > "$APPDIR/omarcade.desktop"
chmod 644 "$APPDIR/omarcade.desktop"
say "$APPDIR/omarcade.desktop"

install -m 644 "$REPO_ROOT/packaging/omarcade.svg" "$ICONDIR/omarcade.svg"
say "$ICONDIR/omarcade.svg"

command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q "$APPDIR" || true
command -v gtk-update-icon-cache   >/dev/null 2>&1 && gtk-update-icon-cache -qtf "${ICONDIR%/scalable/apps}" 2>/dev/null || true

echo
echo "Installed. Launch from your app menu, or run: ${GAMES[0]}"
case ":$PATH:" in
  *":$BINDIR:"*) ;;
  *) echo "NOTE: $BINDIR is not on your PATH; the app-menu entry still works." ;;
esac
echo
echo "Optional -- Hyprland window rules (float, center, full opacity):"
echo "  cp packaging/hyprland/omarcade.lua ~/.config/hypr/omarcade.lua"
echo "  echo 'require(\"hypr.omarcade\")' >> ~/.config/hypr/hyprland.lua"
