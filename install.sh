#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

# --- Helpers ---------------------------------------------------------------

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
RESET='\033[0m'

info()  { printf "${BOLD}==> %s${RESET}\n" "$*"; }
ok()    { printf "${GREEN}==> %s${RESET}\n" "$*"; }
warn()  { printf "${YELLOW}==> %s${RESET}\n" "$*"; }
fail()  { printf "${RED}Error: %s${RESET}\n" "$*" >&2; exit 1; }

usage() {
  cat <<EOF
Usage: ./install.sh [OPTIONS] [TARGETS...]

Build and package Envibe for production.

Options:
  --all       Build all package formats (slow — compresses ~200MB Electron app)
  --help      Show this help message

Linux targets (pass one or more):
  dir         Unpacked directory — run directly (default, instant)
  AppImage    Universal Linux binary (slow — squashfs compression)
  tar.gz      Compressed archive (slow — gzip compression)
  deb         Debian/Ubuntu package (requires fpm)
  rpm         Fedora/RHEL package (requires fpm + rpm-build)
  pacman      Arch/CachyOS package (requires fpm)

macOS targets:
  dir         Unpacked .app bundle (default, instant)
  dmg         Disk image (slow — compression)

With no targets specified, builds an unpacked directory. This is fast and
produces a directly-runnable binary at dist/linux-unpacked/envibe (Linux)
or dist/mac/Envibe.app (macOS).

Examples:
  ./install.sh                    # Unpacked build (instant)
  ./install.sh AppImage           # AppImage only (~2-5 min for compression)
  ./install.sh tar.gz pacman      # tar.gz + pacman
  ./install.sh --all              # All formats
EOF
  exit 0
}

# --- Parse arguments -------------------------------------------------------

BUILD_ALL=false
TARGETS=()

for arg in "$@"; do
  case "$arg" in
    --all)  BUILD_ALL=true ;;
    --help) usage ;;
    *)      TARGETS+=("$arg") ;;
  esac
done

# --- Platform detection ----------------------------------------------------

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin) PLATFORM="mac" ;;
  Linux)  PLATFORM="linux" ;;
  *)      fail "Unsupported platform: $OS. This script supports macOS and Linux." ;;
esac

info "Platform: $OS ($ARCH)"

# Resolve targets
if [ "$BUILD_ALL" = true ]; then
  if [ "$PLATFORM" = "linux" ]; then
    TARGETS=("dir" "AppImage" "tar.gz" "deb" "rpm" "pacman")
  else
    TARGETS=("dir" "dmg")
  fi
elif [ ${#TARGETS[@]} -eq 0 ]; then
  TARGETS=("dir")
fi

TARGETS_STR="${TARGETS[*]}"
info "Targets: $TARGETS_STR"

# Warn if any slow target is selected
SLOW_TARGETS="AppImage|tar.gz|dmg|deb|rpm|pacman"
for t in "${TARGETS[@]}"; do
  if [[ "$t" =~ ^($SLOW_TARGETS)$ ]]; then
    warn "Packaged targets selected — compression of ~200MB Electron app will take a few minutes."
    warn "electron-builder produces no output during compression. This is normal."
    break
  fi
done

# --- Prerequisite checks ---------------------------------------------------

info "Checking prerequisites..."

command -v node  >/dev/null 2>&1 || fail "Node.js is not installed. Install it from https://nodejs.org/"
command -v npm   >/dev/null 2>&1 || fail "npm is not installed. It ships with Node.js: https://nodejs.org/"
command -v rustc >/dev/null 2>&1 || fail "Rust is not installed. Install it from https://rustup.rs/"
command -v cargo >/dev/null 2>&1 || fail "Cargo is not installed. Install Rust from https://rustup.rs/"

NODE_VER="$(node -v)"
RUST_VER="$(rustc --version)"
ok "Node.js $NODE_VER"
ok "Rust    $RUST_VER"

# --- Install npm dependencies ---------------------------------------------

info "Installing npm dependencies..."
npm ci --prefer-offline 2>/dev/null || npm install
ok "npm dependencies installed"

# --- Build Rust backend (release) ------------------------------------------

info "Building Rust backend (release)..."
(cd backend && cargo build --release)

BINARY="backend/target/release/envibe"
[ -f "$BINARY" ] || fail "Rust binary not found at $BINARY"
ok "Rust backend built: $BINARY"

# --- Build frontend + Electron ---------------------------------------------

info "Compiling Electron TypeScript..."
npx tsc -p tsconfig.electron.json

info "Bundling React frontend..."
npx vite build

# --- Package with electron-builder ----------------------------------------

info "Packaging Electron app ($TARGETS_STR)..."
npx electron-builder --"$PLATFORM" "${TARGETS[@]}" --publish=never

# --- Done ------------------------------------------------------------------

echo ""
ok "Build complete!"
echo ""

# Show output
if [ "$PLATFORM" = "linux" ]; then
  UNPACKED="dist/linux-unpacked"
  if [ -d "$UNPACKED" ]; then
    info "Unpacked app: $UNPACKED/envibe"
  fi
elif [ "$PLATFORM" = "mac" ]; then
  MAC_APP="dist/mac/Envibe.app"
  if [ -d "$MAC_APP" ]; then
    info "App bundle: $MAC_APP"
  fi
fi

PACKAGES=$(find dist -maxdepth 1 \( \
  -name '*.dmg' -o \
  -name '*.AppImage' -o \
  -name '*.deb' -o \
  -name '*.rpm' -o \
  -name '*.pacman' -o \
  -name '*.tar.gz' \
\) 2>/dev/null | sort)

if [ -n "$PACKAGES" ]; then
  info "Packages:"
  echo "$PACKAGES" | while read -r f; do
    SIZE=$(du -h "$f" | cut -f1)
    echo "  $f  ($SIZE)"
  done
fi
