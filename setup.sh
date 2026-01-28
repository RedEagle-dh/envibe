#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
APP_NAME="envibe"
APP_LABEL="Envibe"
APP_COMMENT="Dev orchestration tool for parallel project management"
UNPACKED_DIR="$SCRIPT_DIR/dist/linux-unpacked"

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
Usage: ./setup.sh [COMMAND] [OPTIONS]

Install or uninstall Envibe on your system.

Commands:
  install       Install Envibe (default)
  uninstall     Remove Envibe from the system

Options:
  --system      System-wide install to /opt (requires sudo)
  --help        Show this help message

User-local install (default, no sudo):
  App:       ~/.local/share/envibe/
  Binary:    ~/.local/bin/envibe
  Desktop:   ~/.local/share/applications/envibe.desktop

System-wide install (--system):
  App:       /opt/envibe/
  Binary:    /usr/local/bin/envibe
  Desktop:   /usr/share/applications/envibe.desktop

Examples:
  ./setup.sh                # Install for current user
  ./setup.sh --system       # Install system-wide (sudo)
  ./setup.sh uninstall      # Uninstall (user-local)
  ./setup.sh uninstall --system
EOF
  exit 0
}

# --- Parse arguments -------------------------------------------------------

COMMAND="install"
SYSTEM_WIDE=false

for arg in "$@"; do
  case "$arg" in
    install)    COMMAND="install" ;;
    uninstall)  COMMAND="uninstall" ;;
    --system)   SYSTEM_WIDE=true ;;
    --help)     usage ;;
    *)          fail "Unknown argument: $arg. Use --help for usage." ;;
  esac
done

# --- Resolve paths ---------------------------------------------------------

if [ "$SYSTEM_WIDE" = true ]; then
  INSTALL_DIR="/opt/$APP_NAME"
  BIN_DIR="/usr/local/bin"
  DESKTOP_DIR="/usr/share/applications"
  SUDO="sudo"
else
  INSTALL_DIR="$HOME/.local/share/$APP_NAME"
  BIN_DIR="$HOME/.local/bin"
  DESKTOP_DIR="$HOME/.local/share/applications"
  SUDO=""
fi

BIN_LINK="$BIN_DIR/$APP_NAME"
DESKTOP_FILE="$DESKTOP_DIR/$APP_NAME.desktop"
ICON_DIR="${INSTALL_DIR}"

# --- Uninstall -------------------------------------------------------------

do_uninstall() {
  info "Uninstalling Envibe..."

  if [ -L "$BIN_LINK" ]; then
    $SUDO rm "$BIN_LINK"
    ok "Removed $BIN_LINK"
  fi

  if [ -f "$DESKTOP_FILE" ]; then
    $SUDO rm "$DESKTOP_FILE"
    ok "Removed $DESKTOP_FILE"

    # Update desktop database
    if command -v update-desktop-database >/dev/null 2>&1; then
      $SUDO update-desktop-database "$(dirname "$DESKTOP_FILE")" 2>/dev/null || true
    fi
  fi

  if [ -d "$INSTALL_DIR" ]; then
    $SUDO rm -rf "$INSTALL_DIR"
    ok "Removed $INSTALL_DIR"
  fi

  echo ""
  ok "Envibe uninstalled."
}

# --- Install ---------------------------------------------------------------

do_install() {
  # Verify unpacked build exists
  [ -d "$UNPACKED_DIR" ] || fail "Unpacked build not found at $UNPACKED_DIR. Run ./install.sh first."
  [ -f "$UNPACKED_DIR/envibe" ] || fail "Executable not found at $UNPACKED_DIR/envibe. Build may be corrupt."

  if [ "$SYSTEM_WIDE" = true ]; then
    info "Installing Envibe system-wide..."
  else
    info "Installing Envibe for current user..."
  fi

  # Create directories
  $SUDO mkdir -p "$INSTALL_DIR"
  mkdir -p "$BIN_DIR"
  $SUDO mkdir -p "$DESKTOP_DIR"

  # Copy app files
  info "Copying application files to $INSTALL_DIR..."
  $SUDO cp -a "$UNPACKED_DIR/." "$INSTALL_DIR/"
  $SUDO chmod +x "$INSTALL_DIR/envibe"
  $SUDO chmod +x "$INSTALL_DIR/chrome-sandbox" 2>/dev/null || true
  ok "Application files installed"

  # Create symlink in PATH
  info "Creating symlink: $BIN_LINK"
  $SUDO ln -sf "$INSTALL_DIR/envibe" "$BIN_LINK"
  ok "Symlink created"

  # Write .desktop file
  info "Creating desktop entry: $DESKTOP_FILE"
  $SUDO tee "$DESKTOP_FILE" > /dev/null <<EOF
[Desktop Entry]
Name=$APP_LABEL
Comment=$APP_COMMENT
Exec=$INSTALL_DIR/envibe %U
Terminal=false
Type=Application
Categories=Development;IDE;
Keywords=docker;compose;dev;orchestration;
StartupWMClass=$APP_NAME
StartupNotify=true
EOF

  # If there's an icon, add it to the desktop entry
  # Electron apps include a default icon in the snapshot; use a generic fallback
  if [ -f "$INSTALL_DIR/resources/app.asar" ]; then
    # Extract icon from asar if possible, otherwise use a system icon
    for icon in \
      "$INSTALL_DIR/resources/icon.png" \
      "$INSTALL_DIR/resources/icons/512x512.png" \
      "$INSTALL_DIR/resources/app-icon.png"; do
      if [ -f "$icon" ]; then
        $SUDO sed -i "/^StartupNotify/a Icon=$icon" "$DESKTOP_FILE"
        ok "Icon set: $icon"
        break
      fi
    done
  fi

  $SUDO chmod 644 "$DESKTOP_FILE"

  # Update desktop database so launchers pick it up immediately
  if command -v update-desktop-database >/dev/null 2>&1; then
    $SUDO update-desktop-database "$(dirname "$DESKTOP_FILE")" 2>/dev/null || true
  fi

  # --- Done ----------------------------------------------------------------

  echo ""
  ok "Envibe installed!"
  echo ""
  info "Installed to:  $INSTALL_DIR"
  info "Command:       $BIN_LINK"
  info "Desktop entry: $DESKTOP_FILE"
  echo ""
  info "Launch from terminal:  envibe"
  info "Launch from Hyprland:  SUPER -> search \"Envibe\""

  # Check if BIN_DIR is in PATH
  if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
    echo ""
    warn "$BIN_DIR is not in your PATH."
    warn "Add this to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
    echo ""
    echo "  export PATH=\"$BIN_DIR:\$PATH\""
  fi
}

# --- Run -------------------------------------------------------------------

case "$COMMAND" in
  install)   do_install ;;
  uninstall) do_uninstall ;;
esac
