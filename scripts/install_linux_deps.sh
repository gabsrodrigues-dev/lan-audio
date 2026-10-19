#!/usr/bin/env bash
# Installs build + runtime dependencies for LAN Audio on Linux.
# Supports Debian/Ubuntu (apt), Fedora (dnf) and Arch (pacman).
set -euo pipefail

if command -v apt-get >/dev/null 2>&1; then
  sudo apt-get update
  sudo apt-get install -y build-essential cmake pkg-config pulseaudio-utils \
    libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev
elif command -v dnf >/dev/null 2>&1; then
  sudo dnf install -y gcc gcc-c++ cmake pkgconf-pkg-config pulseaudio-utils \
    webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel libxdo-devel openssl-devel
elif command -v pacman >/dev/null 2>&1; then
  sudo pacman -S --needed base-devel cmake pkgconf libpulse \
    webkit2gtk-4.1 libappindicator-gtk3 librsvg xdotool openssl
else
  echo "Unsupported distro: install cmake, pkg-config, pulseaudio-utils (parec/pacat/pactl)" >&2
  echo "and the Tauri v2 Linux prerequisites (webkit2gtk-4.1, appindicator, librsvg)." >&2
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "Rust not found. Install it with: curl https://sh.rustup.rs -sSf | sh" >&2
fi
echo "Done. PipeWire users: make sure pipewire-pulse is running (pactl info)."
