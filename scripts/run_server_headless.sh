#!/usr/bin/env bash
# Run the headless LAN Audio server on Linux, capturing system audio
# (default sink monitor) through PulseAudio / PipeWire.
#
# Usage: scripts/run_server_headless.sh [--source synthetic|linux_loopback] [extra desktop_headless args...]
# Env:   LAN_AUDIO_LINUX_SOURCE=<pulse source>  (default @DEFAULT_MONITOR@)
set -euo pipefail
cd "$(dirname "$0")/.."

SOURCE=linux_loopback
if [[ "${1:-}" == "--source" ]]; then
  SOURCE="$2"; shift 2
fi

command -v cargo >/dev/null || { echo "cargo not found; install Rust first" >&2; exit 1; }
command -v parec >/dev/null || echo "warning: parec not found (install pulseaudio-utils); falling back to synthetic" >&2

exec cargo run --release -p lan_audio_server --bin desktop_headless -- \
  --audio-source "$SOURCE" "$@"
