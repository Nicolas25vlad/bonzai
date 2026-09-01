#!/usr/bin/env bash
set -euo pipefail

REPO="https://github.com/Nicolas25vlad/bonzai.git"
PREFIX="${BONZAI_PREFIX:-$HOME/.local}"
BIN_DIR="$PREFIX/bin"
SYSTEMD_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
ENABLE_SYSTEMD=0

for arg in "$@"; do
  case "$arg" in
    --systemd) ENABLE_SYSTEMD=1 ;;
    -h|--help)
      cat <<'EOF'
Bonzai installer

Usage:
  ./install.sh [--systemd]
  curl -fsSL https://raw.githubusercontent.com/Nicolas25vlad/bonzai/main/install.sh | bash

Options:
  --systemd   Install and enable the user service after building.

Environment:
  BONZAI_PREFIX   Installation prefix. Defaults to ~/.local
EOF
      exit 0
      ;;
    *) echo "Unknown option: $arg" >&2; exit 2 ;;
  esac
done

command -v cargo >/dev/null 2>&1 || {
  echo "error: cargo is required. Install Rust with rustup or your package manager." >&2
  exit 1
}
command -v stty >/dev/null 2>&1 || {
  echo "error: stty is required for interactive watch mode." >&2
  exit 1
}

SOURCE_DIR=""
CLEANUP_DIR=""
if [[ -f Cargo.toml && -f src/main.rs ]]; then
  SOURCE_DIR="$PWD"
else
  command -v git >/dev/null 2>&1 || {
    echo "error: git is required when running the remote installer." >&2
    exit 1
  }
  CLEANUP_DIR="$(mktemp -d)"
  trap 'rm -rf "$CLEANUP_DIR"' EXIT
  git clone --depth 1 "$REPO" "$CLEANUP_DIR/bonzai"
  SOURCE_DIR="$CLEANUP_DIR/bonzai"
fi

printf '\n\033[1;32m🌱 Building Bonzai...\033[0m\n'
cd "$SOURCE_DIR"
cargo build --release --locked

mkdir -p "$BIN_DIR"
install -m755 target/release/bonzai "$BIN_DIR/bonzai"

if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
  printf '\nNote: add %s to your PATH.\n' "$BIN_DIR"
fi

if (( ENABLE_SYSTEMD )); then
  command -v systemctl >/dev/null 2>&1 || {
    echo "error: systemctl was not found." >&2
    exit 1
  }
  mkdir -p "$SYSTEMD_DIR"
  sed "s|%h/.local/bin/bonzai|$BIN_DIR/bonzai|" systemd/bonzai.service > "$SYSTEMD_DIR/bonzai.service"
  systemctl --user daemon-reload
  systemctl --user enable --now bonzai.service
fi

printf '\n\033[1;32m✓ Bonzai installed to %s/bonzai\033[0m\n' "$BIN_DIR"
printf '  Run: bonzai init && bonzai start && bonzai watch\n\n'
