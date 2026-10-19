#!/usr/bin/env bash
# Build the Tauri desktop client on Linux (.deb + AppImage bundles).
# Run scripts/install_linux_deps.sh first.
set -euo pipefail
cd "$(dirname "$0")/../apps/desktop/src-tauri"

if ! cargo tauri --version >/dev/null 2>&1; then
  echo "[desktop] installing tauri-cli ..."
  cargo install tauri-cli --version "^2" --locked
fi
echo "[desktop] running cargo tauri build ..."
cargo tauri build --bundles deb,appimage "$@"
echo "[desktop] bundles in <repo>/target/release/bundle/ (deb, appimage)"
