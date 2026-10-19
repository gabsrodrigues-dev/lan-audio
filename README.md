<p align="center">
  <h1 align="center">LAN Audio</h1>
  <p align="center">
    Stream Windows or Linux PC audio to Android phones over Wi-Fi or USB.<br/>
    Turn any Android device into a wireless speaker.
  </p>
</p>

<p align="center">
  <a href="https://github.com/MengBad/lan-audio/releases"><img alt="Release" src="https://img.shields.io/github/v/release/MengBad/lan-audio?color=6366f1" /></a>
  <a href="https://github.com/MengBad/lan-audio/blob/main/LICENSE"><img alt="License" src="https://img.shields.io/github/license/MengBad/lan-audio?color=22c55e" /></a>
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20Android-8b5cf6" />
</p>

<p align="center">
  <a href="#quick-start">Quick Start</a> &nbsp;|&nbsp;
  <a href="https://github.com/MengBad/lan-audio/releases">Download</a> &nbsp;|&nbsp;
  <a href="CHANGELOG.md">Changelog</a> &nbsp;|&nbsp;
  <a href="README.zh-CN.md">中文文档</a>
</p>

---

## Overview

LAN Audio captures system audio from a Windows PC (WASAPI Loopback) or a Linux PC (PulseAudio / PipeWire monitor) and streams it in real-time to Android devices on the same local network. Supports Wi-Fi and USB (adb) connections with automatic mDNS discovery.

**Use cases:**
- PC speakers are weak, but your phone/tablet sounds better
- Listen to PC audio from another room
- Quick wireless speaker setup without extra hardware

## Features

### Audio Pipeline
- **Opus codec** — 48kHz VBR encoding with sinc resampler for non-48kHz sources
- **PCM16 lossless** — uncompressed fallback for maximum compatibility
- **Four playback modes** — ultra_low_latency (≤30ms) / low_latency (~64ms) / balanced / high_quality
- **5ms frame duration** — ultra-low-latency mode uses 5ms capture and transport frames for minimal pipeline delay
- **Native 3-band EQ** — biquad peaking filter on Oboe path (60Hz / 1kHz / 10kHz) with presets
- **Loudness normalization** — auto gain control in balanced/high_quality modes
- **Adaptive runtime** — server-side CPU + queue watchdog auto-downgrades codec under pressure
- **Auto-degradation** — ultra-low-latency mode automatically falls back to low_latency when network jitter is too high

### Connectivity
- **Auto-discovery** — mDNS scan finds nearby servers, no manual IP needed
- **USB mode** — stable wired connection via adb, no Wi-Fi required
- **Auto-reconnect** — exponential backoff on network interruptions
- **Multi-device** — up to 4 phones receiving simultaneously
- **Remote volume** — control phone volume from PC tray menu

### Android Client
- **Codec picker** — choose Auto / Opus / PCM 16 from the UI
- **Latency chart** — real-time smooth curve with dashed baseline reference and live ms readout
- **Audio quality strip** — shows negotiated codec, sample rate, channels
- **Reverse mic** — stream Android microphone back to PC (port 7878)
- **Background playback** — ForegroundService with MediaSession, survives screen-off

### Windows Desktop
- **System tray** — left-click to show window, right-click for quick menu (volume, updates, quit)
- **Compact UI** — Audio Console Dark theme, all controls visible without scrolling
- **Auto-start streaming** — one-click start/stop with QR code for phone connection
- **Safe mode** — one-click rollback to legacy path for troubleshooting

## Quick Start

### Download

**Android:** Grab the APK from [Releases](https://github.com/MengBad/lan-audio/releases). Most phones need the `arm64-v8a` build.

**Linux:** see [Linux](#linux) below (`.deb` / AppImage from the *Build Linux Client* workflow, or build from source).

**Windows:** Download the `.exe` from Releases, or build from source:

```powershell
git clone https://github.com/MengBad/lan-audio.git
cd lan-audio
cargo run -p lan_audio_server --bin desktop_headless -- --audio-source windows_loopback
```

### Usage

1. Connect PC and phone to the same Wi-Fi network
2. Start LAN Audio on Windows or Linux
3. Open the app on Android — it auto-discovers the PC
4. Tap connect and start streaming

**USB mode** (more stable, no Wi-Fi needed):
```powershell
cargo run -p lan_audio_server --bin desktop_headless -- --transport usb --adb-serial <device-serial> --audio-source windows_loopback
```

## Linux

The server captures "what you hear" from the monitor of the default output
sink using `parec`, so it works on both PulseAudio and PipeWire
(`pipewire-pulse`). No virtual cable is needed.

```bash
# 1. Dependencies (apt/dnf/pacman are detected automatically)
./scripts/install_linux_deps.sh

# 2a. Headless server (system audio, Opus over Wi-Fi)
./scripts/run_server_headless.sh
#     or: cargo run --release -p lan_audio_server --bin desktop_headless -- --audio-source linux_loopback

# 2b. Desktop app with tray icon (.deb + AppImage)
./scripts/build_desktop_client.sh
```

- Capture a different source: `LAN_AUDIO_LINUX_SOURCE=<name>` (list with `pactl list short sources`).
  Default is `@DEFAULT_MONITOR@`, which follows the current default output.
- `--audio-source system_loopback` picks the native loopback for the OS; on Linux
  `windows_loopback` is also accepted and mapped to `linux_loopback`.
- **Reverse mic:** when the phone streams its microphone, the server creates a
  virtual input called **LAN Audio Mic** (`lan_audio_mic`) that any app can select.
  It is removed when the phone disconnects.
- **Firewall (ufw):** `sudo ufw allow 39990:39992/udp && sudo ufw allow 39991/tcp && sudo ufw allow 7878:7879/tcp`
- **USB mode** works the same as on Windows (`adb` in `PATH`).

## Tech Stack

| Component | Technology |
| :--- | :--- |
| Audio capture | WASAPI Loopback (Windows), PulseAudio/PipeWire monitor via `parec` (Linux) |
| Desktop GUI | Tauri 2 + Rust |
| Transport | UDP data + WebSocket control, Protocol v2/v3 |
| Codec | Opus 48kHz VBR, PCM16 |
| Android client | Flutter + Kotlin |
| Audio output | Oboe (NDK), AudioTrack fallback |
| Discovery | mDNS |

## Project Structure

```
apps/
  android_flutter/       Android client (Flutter + Kotlin Native)
  desktop/               Windows / Linux desktop (Tauri + Rust)

crates/
  lan_audio_protocol/    Protocol definitions & wire format parsing
  lan_audio_server/      Audio capture, encoding, transport, adaptive runtime
  lan_audio_domain/      Shared types & constants

docs/                    Protocol specs, architecture, design docs
scripts/                 Build, validation & release scripts
```

## Development

### Requirements

- Rust 1.75+
- Flutter 3.x
- Android SDK + NDK (for Oboe)
- Windows 10+, or Linux with PulseAudio or PipeWire (`pipewire-pulse`)

### Build & Test

```powershell
# One-step validation (format + check + test)
powershell -ExecutionPolicy Bypass -File .\scripts\validate_local.ps1

# Or manually
cargo fmt --all -- --check
cargo check
cargo test -p lan_audio_protocol -p lan_audio_server
```

### Build Release

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\package_release.ps1 -Clean
```

## Documentation

| Document | Description |
| :--- | :--- |
| [Protocol Spec](docs/protocol.md) | Wire format, packet structure, negotiation |
| [Protocol v2 Migration](docs/protocol_v2_migration.md) | Migration guide from v1 to v2 |
| [Architecture](docs/architecture.md) | System architecture overview |
| [Desktop UI Design](docs/desktop_ui.md) | Desktop client UI spec |
| [Dev Setup](docs/dev_setup.md) | Development environment setup |
| [Known Issues](docs/known_issues.md) | Known bugs and limitations |
| [Roadmap](docs/roadmap.md) | Feature roadmap |
| [Release Policy](docs/RELEASE_POLICY.md) | Versioning & release process |

> For Chinese documentation, see [README.zh-CN.md](README.zh-CN.md)

## License

[MIT](LICENSE)
